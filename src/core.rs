use log::{debug, warn};
use rmp_serde::decode::Deserializer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{create_dir_all, File};
use std::io::BufReader;
use std::path::PathBuf;

use super::discovery;
use super::flavors::core_flavors as flavors;
use super::supported_apis::{standalone_apis, supported_apis};
use super::update;

/// Variants of project-related placeholder names appearing in flat_path.
/// Most APIs use "projectsId" but some use "project" or "projectId".
pub static PATH_PLACEHOLDERS_PROJECT: &[&str] = &["projectsId", "project", "projectId"];

/// Variants of region (location) related placeholder names appearing in flat_path.
pub static PATH_PLACEHOLDERS_REGION: &[&str] = &["regionsId", "region", "locationsId", "location"];

/// Variants of zone related placeholder names appearing in flat_path.
pub static PATH_PLACEHOLDERS_ZONE: &[&str] = &["zonesId", "zone"];

// ---------------------- core structs ----------------------------- //
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ZgApi {
    pub id: String,
    pub name: String,
    pub version: String,
    pub revision: String,
    pub base_url: String,
    pub resources: Vec<ZgResource>,
    pub schemas: HashMap<String, discovery::Schema>,
}

impl ZgApi {
    /// Returns a list of all resource paths in the API.
    ///
    /// Sample output:
    /// [
    ///     ("projects", "container.projects"),
    ///     ("aggregated", "container.projects.aggregated"),
    ///     ("locations", "container.projects.locations"),
    ///     ("clusters", "container.projects.locations.clusters"),
    ///      ...
    /// ]
    pub fn all_resource_paths(&self) -> Vec<(String, String)> {
        fn collect_paths(resource: &ZgResource, paths: &mut Vec<(String, String)>) {
            if let Some(ref path) = resource.path {
                paths.push((resource.name.clone(), path.clone()));
            }
            if let Some(ref sub_resources) = resource.resources {
                for sub_resource in sub_resources {
                    collect_paths(sub_resource, paths);
                }
            }
        }

        let mut resource_paths = Vec::new();
        for resource in &self.resources {
            collect_paths(resource, &mut resource_paths);
        }
        resource_paths
    }

    /// Returns a list of resources with duplicated paths.
    ///
    /// Sample output:
    /// [
    ///    ("operations", ["container.projects.locations.operations", "container.projects.zones.operations"]),
    ///    ("nodePools", ["container.projects.locations.clusters.nodePools", "container.projects.zones.clusters.nodePools"]),
    ///    ("clusters", ["container.projects.locations.clusters", "container.projects.zones.clusters"])
    /// ]
    pub fn duplicated_resources(&self) -> Vec<(String, Vec<String>)> {
        self.all_resource_paths()
            .into_iter()
            .fold(
                HashMap::new(),
                |mut map: HashMap<String, Vec<String>>, (name, path)| {
                    map.entry(name).or_default().push(path);
                    map
                },
            )
            .into_iter()
            .filter_map(|(name, paths)| {
                if paths.len() > 1 {
                    Some((name, paths))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ZgResource {
    pub name: String,
    pub parent_path: Option<String>,

    // Used to identify the resource. No method resources have no path (when generated through `convert_resoruce`).
    pub path: Option<String>,

    pub methods: Vec<ZgMethod>,
    pub resources: Option<Vec<ZgResource>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ZgMethod {
    pub id: String,
    pub original_id: Option<String>, // Some() when update::update_resource_paths() is called when importing the API
    pub name: String,
    pub flat_path: String,
    pub http_method: String,
    pub query_params: Vec<ZgQueryParam>,
    // Retrieve the referenced ($ref) object to convert. GET/DELETE: None, other methods: Some(ZgRequestObj).
    // Schema's "Output only (readOnly: true)" properties are filtered out in `update::convert_method()`.
    pub request_data_schema: Option<discovery::Schema>,
}

/// Query parameters for a method. Path parameters are not included here as they are part of the flat_path.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ZgQueryParam {
    pub name: String,
    pub description: Option<String>,

    // Inherited from Parameter, and overwritten to true if description starts with "Required.”
    // Despite that, the required flag isn't perfect - you might not realize that a parameter is required until you execute the method actually.
    // Example of undocumented required parameter: “query” in https://cloud.google.com/bigquery/docs/reference/reservations/rest/v1/projects.locations/searchAllAssignments
    pub required: bool,
}

// ---------------------- Common functions --------------------------- //
/// Returns a directory path to store config and cached data ($HOME/.config/zg).
pub fn config_dir() -> PathBuf {
    let config_dir = dirs::home_dir()
        .expect("Failed to get home directory")
        .join(".config")
        .join("zg");

    // Create the config dir and its subdirs if they don't exist
    create_dir_all(&config_dir).expect("Failed to create configuration directory");
    for subdir in &["api", "discovered"] {
        let subdir_path = config_dir.join(subdir);
        create_dir_all(&subdir_path).expect("Failed to create subdirectory");
    }

    config_dir
}

/// Returns a directory path to store ZgApi in msgpack ($HOME/.config/zg/api).
pub fn api_dir() -> PathBuf {
    config_dir().join("api")
}

/// Load the API description from a serialized MessagePack file
pub async fn load_api_file(
    api_string: &str,
    standalone_key: Option<String>,
) -> Result<ZgApi, Box<dyn Error>> {
    let (cname, version) =
        lookup_api(api_string).ok_or_else(|| format!("Service '{}' not found", api_string))?;

    let path = api_dir().join(format!("{}_{}.msgpack", &cname, &version));
    debug!("API {}:{} is supported. Open {:?}", &cname, &version, &path);

    // Attempt to open the file; if it doesn't exist, perform lazy preparation
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(_) => {
            debug!(
                "File not found. Initiating lazy preparation for {}:{}",
                &cname, &version
            );
            lazy_prep_api_file(&cname, &version, &path, standalone_key).await?
        }
    };

    let reader = BufReader::new(&file);
    Deserialize::deserialize(&mut Deserializer::new(reader))
        .map_err(|e| format!("Error: Failed to deserialize '{:?}': {}", &file, e).into())
}

/// Called when api:version is supported but the API .msgpack file is not found. Possibly `zg update` is not executed.
/// Prepare the API file "lazy" way - downloading the API description and processing it.
async fn lazy_prep_api_file(
    api_name: &str,
    version: &str,
    path: &PathBuf,
    standalone_key: Option<String>,
) -> Result<File, Box<dyn Error>> {
    // Check if a standalone API is requested
    let standalone_api = standalone_apis()
        .into_iter()
        .find(|api| api.name == api_name && api.versions.iter().any(|v| v == version));

    let apidef_path = match standalone_api {
        Some(standalone_api) => {
            // Download the standalone API definition
            let standalone_api_id = format!("{}:{}", api_name, version);
            let key = standalone_key.ok_or_else(|| {
                format!(
                    "--api-key is required for standalone API '{}'",
                    standalone_api_id
                )
            })?;
            debug!(
                "API key '{}' is provided for standalone API '{}'",
                key, standalone_api_id
            );
            let standalone_url = discovery::standalone_discovery_url(standalone_api.clone(), key);
            discovery::download_api_definition(standalone_api_id, standalone_url).await?
        }
        None => {
            // Find the matching item from discovered APIs or raise an error if not found
            let discovered_item = discovery::ensure_discovered_apis(false)
                .await?
                .items
                .into_iter()
                .find(|item| item.name == api_name && item.version == version)
                .ok_or_else(|| {
                    format!("{}:{} not found in the discovered APIs", api_name, version)
                })?;

            discovery::download_api_definition(
                discovered_item.id,
                discovered_item.discovery_rest_url,
            )
            .await?
        }
    };
    debug!("Downloaded API definition: {:?}", apidef_path);

    // Extract the API description to build ZgApi from the downloaded JSON file
    let zg_api = update::extract_api(apidef_path)?;

    // Store the extracted API description to a file (in msgpack format)
    update::store_zgapi_msgpack(zg_api, path)?;

    // Simply try to open the file again (mimicking the previous behavior)
    File::open(path).map_err(|e| format!("(Lazy) Failed to open file '{:?}': {}", path, e).into())
}

/// Finds the canonical service id and version for a given service or its alias.
///
/// For example, to find "container:v1", you have multiple ways:
/// - "container:v1" (explicit version)
/// - "container" (assumes the default version)
/// - "gke" (alias with the default version)
/// - "gke:v1" (alias with version)
fn lookup_api(api_string: &str) -> Option<(String, String)> {
    // Split the api_string into the frist part (name or alias) and the optional second part (version)
    let mut parts = api_string.splitn(2, ':');
    let name_or_alias = parts.next()?;
    let explicit_version = parts.next();

    // Find the matching API by name or alias
    let api = supported_apis(true).into_iter().find(|api| {
        api.name == name_or_alias || api.aliases.contains(&name_or_alias.to_string())
    })?;

    // Determine the version
    let version = match explicit_version {
        Some(ver) if api.versions.contains(&ver.to_string()) => ver,
        Some(_) => return None,        // Invalid version is given
        None => api.default_version(), // Use the default version
    };

    // Return the canonical API name and resolved version
    Some((api.name.to_string(), version.to_string()))
}

/// Find the target resource in the given API
pub fn find_resource<'a>(
    api_id: &str,
    resources: &'a [ZgResource],
    resource_path: &str,
) -> Result<&'a ZgResource, Box<dyn Error>> {
    let mut found = Vec::<&'a ZgResource>::new();

    fn recursive<'a>(
        resource_path: &str,
        resources: &'a [ZgResource],
        found: &mut Vec<&'a ZgResource>,
    ) {
        for resource in resources {
            if let Some(path) = &resource.path {
                if path.ends_with(resource_path) {
                    found.push(resource);
                }
            }

            if let Some(sub_resources) = &resource.resources {
                recursive(resource_path, sub_resources, found);
            }
        }
    }

    recursive(resource_path, resources, &mut found);

    // Early return with an error if no matching resource is found
    if found.is_empty() {
        return Err(format!(
            "Resource '{}' not found for API '{}'.",
            resource_path, api_id
        )
        .into()); // Convert the error message to Box<dyn Error>
    }

    select_resource(api_id, resource_path, found)
        .ok_or_else(|| format!("Failed to select resource '{}'", resource_path).into())
}

/// Selects a resource from a list of found resources based on the API ID and resource path.
///
/// If no resources are found, returns None.
/// If multiple resources are found, resolves ambiguity with service-specific heuristic (flavors).
/// If no service-specific logic is defined, just returns one item without thinking.
///
/// List of services with duplicate resource names, but no specific flavor is defined:
/// - "iam:v1" ... keys x 3, locations x 2, operations x 10, providers x 2, roles x 3
fn select_resource<'a>(
    api_id: &str,
    resource_path: &str, // user-typed resource path
    found: Vec<&'a ZgResource>,
) -> Option<&'a ZgResource> {
    // Return early when only one candidate (length: 1) or no candidate is found (length: 0).
    if found.len() <= 1 {
        return found.first().copied();
    }

    // The below logic would be executed only with multiple choices.
    debug!(
        "Given resource is ambiguous. Candidates: {:#?}",
        found
            .iter()
            .map(|x| x.path.as_ref().unwrap())
            .collect::<Vec<&String>>()
    );

    match api_id {
        "container:v1" => flavors::select_resource_container(found),
        "dataflow:v1b3" => flavors::select_resource_dataflow(resource_path, found),
        "spanner:v1" => flavors::select_resource_spanner(found),
        _ => {
            // Return the last resource as the default choice, with warning
            warn!("Found multiple resources, so returning the last one (--debug for details). Specify more detailed path like 'locations.clusters' instead of 'clsuters' to resolve ambiguity.");
            found.last().copied()
        }
    }
}

/// Find the target method in the resource
pub fn find_method(resource: &ZgResource, method_name: &str) -> Result<ZgMethod, Box<dyn Error>> {
    let method = resource
        .methods
        .iter()
        .find(|m| m.name == method_name)
        .cloned()
        .ok_or_else(|| -> Box<dyn Error> {
            format!(
                "Method '{}' not found in the resource '{}'",
                method_name,
                resource.path.clone().expect("path should exist")
            )
            .into() // Convert the error message to Box<dyn Error>
        })?;

    Ok(method)
}

// ---------------------- macros ----------------------------- //
/// `vecs!` macro that defines Vec<String>
#[macro_export]
macro_rules! vecs {
    ($($str:expr),*) => {
        vec![$($str.to_string()),*]
    };
}

// ---------------------- dummy data for tests ----------------------------- //
#[cfg(test)]
impl ZgApi {
    pub fn testdata() -> Self {
        Self {
            id: "testapi:v1".to_string(),
            name: "Test API".to_string(),
            version: "v1".to_string(),
            revision: "2024-11-11".to_string(),
            base_url: "https://example.com/".to_string(),
            resources: vec![ZgResource::testdata()],
            schemas: HashMap::new(),
        }
    }
}

#[cfg(test)]
impl ZgResource {
    pub fn testdata() -> Self {
        Self {
            name: "testres".to_string(),
            parent_path: Some("testapi.projects".to_string()),
            path: Some("testapi.projects.testres".to_string()),
            methods: vec![ZgMethod::testdata()],
            resources: None, // no sub-resources by default
        }
    }
}

#[cfg(test)]
impl ZgMethod {
    pub fn testdata() -> Self {
        Self {
            id: "testapi.projects.testres.list".to_string(),
            original_id: None,
            name: "list".to_string(),
            flat_path: "v1/projects/{projectsId}/testres/{testresId}".to_string(),
            http_method: "GET".to_string(),
            query_params: vec![],
            request_data_schema: None,
        }
    }
}

// ---------------------- Unit tests ----------------------------- //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_api() {
        // Helper to represent expected answers beiefly in the following test cases.
        fn ans(n: &str, v: &str) -> Option<(String, String)> {
            Some((n.to_string(), v.to_string()))
        }

        // Valid cases
        assert_eq!(lookup_api("container:v1"), ans("container", "v1"));
        assert_eq!(lookup_api("container"), ans("container", "v1"));
        assert_eq!(lookup_api("gke"), ans("container", "v1"));
        assert_eq!(lookup_api("gke:v1"), ans("container", "v1"));

        // Invalid name
        assert_eq!(lookup_api("unknown"), None);
        assert_eq!(lookup_api("unknown:v1"), None);

        // Invalid versions
        assert_eq!(lookup_api("container:v9999"), None);
        assert_eq!(lookup_api("container:heyhey"), None);
    }

    #[test]
    fn test_find_resource_clusters() {
        let top_resources = vec![ZgResource {
            name: "clusters".to_string(),
            path: Some("container.projects.locations.clusters".to_string()),
            ..ZgResource::testdata()
        }];
        let result = find_resource("container", &top_resources, "clusters");
        assert!(result.is_ok(), "Expected to find a 'clusters' resource");
        assert_eq!(result.unwrap().name, "clusters");
    }

    #[test]
    fn test_find_resource_locations_clusters() {
        let top_resources = vec![ZgResource {
            name: "clusters".to_string(),
            path: Some("container.projects.locations.clusters".to_string()),
            ..ZgResource::testdata()
        }];
        let result = find_resource("container", &top_resources, "locations.clusters");
        assert!(
            result.is_ok(),
            "Expected to find a 'locations.clusters' resource"
        );
        assert_eq!(result.unwrap().name, "clusters");
    }

    #[test]
    fn test_select_resource_single_match() {
        let top_resources = [ZgResource {
            name: "projects".to_string(),
            path: Some("container.projects".to_string()),
            ..ZgResource::testdata()
        }];
        let found = vec![&top_resources[0]];
        let result = select_resource("any_api_id", "unused_resource_path", found);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "projects");
    }

    #[test]
    fn test_select_resource_multiple_matches_default() {
        let top_resources = [ZgResource {
            name: "projects".to_string(),
            path: Some("container.projects".to_string()),
            ..ZgResource::testdata()
        }];
        let found = vec![&top_resources[0], &top_resources[0]];
        let result = select_resource("any_api_id", "unused_resource_path", found);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "projects");
    }

    #[test]
    fn test_select_resource_container_v1() {
        let top_resources = vec![
            ZgResource {
                name: "clusters".to_string(),
                path: Some("container.projects.locations.clusters".to_string()),
                ..ZgResource::testdata()
            },
            ZgResource {
                name: "clusters".to_string(),
                path: Some("container.projects.zones.clusters".to_string()),
                ..ZgResource::testdata()
            },
        ];
        let found = vec![
            find_resource("container", &top_resources, "locations.clusters").unwrap(),
            find_resource("container", &top_resources, "zones.clusters").unwrap(),
        ];
        let result = select_resource("container:v1", "unused_resource_path", found);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().path,
            Some("container.projects.locations.clusters".to_string())
        ); // Should prioritize locations.clusters
    }

    #[test]
    fn test_find_method_success() {
        let resource = ZgResource::testdata();
        let method_name = "list";
        let result = find_method(&resource, method_name);

        assert!(result.is_ok(), "Expected to find the method");
        let method = result.unwrap();
        assert_eq!(method.name, method_name);
    }

    #[test]
    fn test_find_method_not_found() {
        let resource = ZgResource::testdata();
        let method_name = "nonexistent_method";
        let result = find_method(&resource, method_name);

        assert!(result.is_err(), "Expected an error");
    }
}
