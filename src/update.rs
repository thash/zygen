// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use clap::Args;
use log::debug;
use regex::Regex;
use rmp_serde::Serializer;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::iter::once;
use std::path::PathBuf;

use super::core;
use super::discovery;
use super::flavors::update_flavors as flavors;
use super::supported_apis::supported_apis;

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Targets all APIs
    #[arg(long)]
    all: bool,
}

pub async fn main(args: &UpdateArgs) -> Result<(), Box<dyn Error>> {
    debug!("{:?}", args);
    let downloaded_files = download().await?;
    debug!("Downloaded files to process: {:?}", downloaded_files);
    for api_filepath in downloaded_files {
        let api = extract_api(api_filepath)?;
        println!("Extracted API for zg: {}", api.id);
        let path = core::api_dir().join(format!("{}.msgpack", api.id.replace(":", "_")));
        store_zgapi_msgpack(api, &path)?;
    }
    Ok(())
}

/// Serialize and store the ZgApi struct locally using MessagePack format
pub fn store_zgapi_msgpack(api: core::ZgApi, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    api.serialize(&mut Serializer::new(writer))?;
    Ok(())
}

/// Download API definition JSONs found both in DISCOVERY_URL response and core::supported_api_ids().
/// Note that it doesn't remove existing JSON files
async fn download() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let discovered_apis = discovery::ensure_discovered_apis(true).await?;

    // Collect supported API IDs in the format of "name:version" (e.g., "bigquery:v2")
    let supported_api_ids: HashSet<String> = supported_apis(true)
        .iter()
        .flat_map(|api| api.versions.iter().map(|v| format!("{}:{}", api.name, v)))
        .collect();

    // From discovered APIs, select supported API IDs, that will be downloaded
    let apis_to_download: Vec<discovery::DiscoveryDirectoryItem> = discovered_apis
        .items
        .into_iter()
        .filter(|item| supported_api_ids.contains(&item.id))
        .collect();
    debug!("Total APIs to download: {}", apis_to_download.len());

    let mut downloaded_files = Vec::new();

    for item in apis_to_download {
        if let Some(filepath) =
            discovery::download_api_definition(item.id, item.discovery_rest_url).await?
        {
            downloaded_files.push(filepath);
        }
    }

    Ok(downloaded_files)
}

/// Extracts API information from a JSON file and converts it into a `ZgApi` struct.
///
/// Reads a JSON file containing API descriptions, parses it into a `core::ApiDescription`,
/// processes its resources using the `convert_resource` function, and constructs a `ZgApi` struct.
pub fn extract_api(api_filepath: PathBuf) -> Result<core::ZgApi, Box<dyn Error>> {
    let api_description: discovery::ApiDescription =
        serde_json::from_reader(BufReader::new(File::open(api_filepath)?))?;

    let resources = api_description
        .resources
        .unwrap_or_default()
        .into_iter()
        .map(|(resource_name, resource)| {
            convert_resource(
                &api_description.name,
                resource_name,
                resource,
                None,
                &api_description.schemas.clone().unwrap_or_default(),
            )
        })
        .collect(); // Collect the resources into a Vec<ZgResource>

    let api = core::ZgApi {
        id: api_description.id,
        name: api_description.name,
        version: api_description.version,
        revision: api_description.revision,
        base_url: api_description.base_url,
        resources,
        schemas: api_description.schemas.unwrap_or_default(),
    };

    match api.id.as_str() {
        // Several API have somewhat "flat (no nest)" resource hierarchy (e.g., bigquery:v2's resources are all top-level).
        // We need to infer the hierarchy based on the method flat_paths and update the resources accordingly.
        "bigquery:v2" => Ok(rebuild_hierarchy(&mut api.clone())),
        "compute:v1" => Ok(rebuild_hierarchy(&mut api.clone())),
        "sqladmin:v1" | "sqladmin:v1beta4" => Ok(rebuild_hierarchy(&mut api.clone())),
        "storage:v1" => Ok(rebuild_hierarchy(&mut api.clone())),
        _ => Ok(api),
    }
}

/// Converts a `core::Resource` into a `core::ZgResource`, handling resource hierarchy and paths.
///
/// # Arguments
///
/// * `service_name` - The name of the service (e.g., "container", "bigquery").
/// * `resource_name` - The name of the current resource being converted (e.g., "projects", "zones").
/// * `resource` - The `core::Resource` struct containing methods and sub-resources to be converted.
/// * `parent_path` - An optional string representing the parent resource's path.
///
/// # Returns
///
/// A `core::ZgResource` that contains the resource name, methods, and nested sub-resources. The function
/// also calculates and sets the `path` field of the resource, which represents the full path to this
/// resource in a dot-separated format (e.g., "container.projects.locations").
fn convert_resource(
    service_name: &str,
    resource_name: String,
    resource: discovery::Resource,
    parent_path: Option<String>,
    schemas: &HashMap<String, discovery::Schema>,
) -> core::ZgResource {
    let methods: Vec<core::ZgMethod> = resource
        .methods
        .unwrap_or_default()
        .into_iter()
        .map(|(n, m)| convert_method(n, m, schemas))
        .collect();

    let path = methods
        .first()
        .map(|m| {
            let mut seg: Vec<_> = m.id.split('.').collect();
            seg.pop(); // remove the last part, which is the method name
            seg.join(".")
        })
        .or_else(|| {
            match &parent_path {
                Some(pp) => Some(format!("{}.{}", pp, resource_name)),
                None => Some(format!("{}.{}", service_name, resource_name)), // top-level
            }
        });

    debug!("service: {service_name} > resource: {resource_name}\n parent_path: {parent_path:?}\n  (new) path: {path:?}");

    let sub_resources = resource
        .resources
        .unwrap_or_default()
        .into_iter()
        .map(|(sub_resource_name, sub_resource)| {
            convert_resource(
                service_name,
                sub_resource_name,
                sub_resource,
                path.clone(),
                schemas,
            )
        })
        .collect();

    core::ZgResource {
        name: resource_name,
        parent_path,
        path,
        methods,
        resources: Some(sub_resources),
    }
}

/// Converts a `discovery::Method` into a `core::ZgMethod`.
fn convert_method(
    method_name: String,
    method: discovery::Method,
    schemas: &HashMap<String, discovery::Schema>,
) -> core::ZgMethod {
    let request_data_schema = match method.http_method.as_str() {
        "GET" | "DELETE" => None, // No request body for GET/DELETE
        _ => method
            .request
            .as_ref()
            .and_then(|req| req.ref_name.as_deref())
            .and_then(|ref_name| schemas.get(ref_name).cloned()), // Resolve and embed the schema directly
    };

    core::ZgMethod {
        id: method.id.clone(),
        original_id: None,
        name: method_name,
        http_method: method.http_method.clone(),
        flat_path: method
            .flat_path
            .or(Some(method.path)) // If Method.flatPath is blank, use Method.path as ZgMethod.flat_path (only `storage:v1`)
            .unwrap_or_else(|| {
                panic!(
                    "Error: Both flatPath and path not exist in the method: {}",
                    method.id
                )
            }),
        query_params: collect_query_params(&method.parameters),
        // None if http_method is GET or DELETE; otherwise, extract from schema in the API definition
        request_data_schema,
    }
}

/// Collects query parameters from the method's parameters.
fn collect_query_params(
    parameters: &Option<HashMap<String, discovery::Parameter>>,
) -> Vec<core::ZgQueryParam> {
    let required_regex = Regex::new(r"(?i)^\s*required\.").unwrap();
    parameters
        .as_ref()
        .map(|params| {
            params
                .iter()
                // Collect only query parameters; ignore path parameters
                // Also, exclude parameters for nested objects that contain a dot (".") in their names (e.g., https://cloud.google.com/spanner/docs/reference/rest/v1/projects.instances.backups/create)
                .filter(|(name, param)| param.location == "query" && !name.contains('.'))
                .map(|(name, param)| core::ZgQueryParam {
                    name: name.clone(),
                    description: param.description.clone(),
                    // Unlike "path" params, "query" params may not be marked as true in the API definition; instead, description starts with 'Required.'
                    required: param.required.unwrap_or_else(|| {
                        param
                            .description
                            .as_ref()
                            .is_some_and(|desc| required_regex.is_match(desc))
                    }),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Rebuilds the resource hierarchy for the given `ZgApi`.
///
/// Updates the given `ZgApi`'s path/parent_path and method ids by calling `update_resource_paths`.
/// Then, based on these updated paths, rebuild the resource hierarchy and returns new `ZgApi`.
fn rebuild_hierarchy(original_api: &mut core::ZgApi) -> core::ZgApi {
    debug_resource_hierarchy(&original_api.resources, 0);

    // Update resource paths, parent_paths, and method IDs based on methods' flat_paths
    let mut api = update_resource_paths(original_api);

    // Prepare children (resources with parent_path) to insert into the hierarchy
    let mut children_to_insert: Vec<core::ZgResource> = Vec::new();
    for resource in api.resources.iter_mut() {
        // If resource has a parent, it's a child
        if resource.parent_path.is_some() {
            children_to_insert.push(resource.clone());
        }
    }
    debug!(
        "children_to_insert: {:?}",
        &children_to_insert
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>()
    );

    // Remove children from the top-level resources; retain only the top-level resources
    api.resources.retain(|r| r.parent_path.is_none());
    debug!(
        "Initial top-level resources: {:?}",
        &api.resources
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>()
    );

    // Insert children into the resource hierarchy; remove (pop) child from the list.
    // Finally all children should find their parents, so iterate until the list is empty.
    while let Some(child_res) = children_to_insert.pop() {
        // Try to find the parent and insert; insert back to children_to_insert if it fails.
        // Use `insert(0, ...)` instead of `push()` to try another child in the next iteration.
        if !insert_child_resource(&mut api.resources, &child_res) {
            children_to_insert.insert(0, child_res);
        }
        debug!("Remaining children count: {}", children_to_insert.len());
    }
    debug_resource_hierarchy(&api.resources, 0);

    api.clone()
}

/// Updates path/parent_path and method ids for each resource in the `ZgApi` by inspecting the methods' flat paths.
fn update_resource_paths(api: &mut core::ZgApi) -> core::ZgApi {
    let (service_name, version) = api.id.split_once(':').unwrap();

    fn recursive(
        resource: &mut core::ZgResource,
        service_name: &str,
        version: &str,
        inherited_parent_path: Option<String>,
    ) {
        let methods = &resource.methods;
        let parent_resource_names: Vec<String> =
            build_parent_resources(service_name, version, &resource.name, methods);
        debug!("inherited_pareht_path: {:?}", inherited_parent_path);
        debug!(
            "resource: '{}' > parent names: {:?}",
            &resource.name, &parent_resource_names
        );

        // If inherited_parent_path is Some (i.e., nested in a parent), use the inherited_parent_path as the parent_path
        // If inherited_parent_path is None (i.e., top-level), so build parent_path by joining the service name and parent_resource_names
        let parent_path: Option<String> = inherited_parent_path.clone().or_else(|| {
            (!parent_resource_names.is_empty()).then(|| {
                once(service_name) // Start with the service name
                    .chain(parent_resource_names.iter().map(String::as_str)) // Append ancestors
                    .collect::<Vec<_>>()
                    .join(".")
            })
        });

        // Build resource_path by joining the parent_path and resource name
        let resource_path = Some(parent_path.as_ref().map_or_else(
            || format!("{}.{}", service_name, &resource.name), // top-level resource
            |pp| format!("{}.{}", pp, &resource.name),
        ));

        // Update ids for each method, path, and parent_path.
        // Keep the original id in the original_id field.
        for method in resource.methods.iter_mut() {
            method.original_id = Some(method.id.clone());
            method.id = format!("{}.{}", &resource_path.as_ref().unwrap(), &method.name);
        }

        // Recursively update path/parent_paths of sub-resources if any
        if let Some(sub_resources) = &mut resource.resources {
            for r in sub_resources.iter_mut() {
                recursive(r, service_name, version, resource_path.clone());
            }
        };

        resource.path = resource_path;
        resource.parent_path = parent_path;
        debug!(
            "updated resource paths of '{}':\n  path: {:?}\n  parent_path: {:?}",
            &resource.name, &resource.path, &resource.parent_path
        );
    }

    for resource in api.resources.iter_mut() {
        recursive(resource, service_name, version, None);
    }

    api.clone()
}

/// Recursively inserts a child resource into the correct parent resource based on the parent path.
///
/// This function traverses the given resource hierarchy and inserts the child resource into
/// the matching parent, identified by `parent_path`. If the parent is found, the child resource
/// is added to its sub-resources; if not, finally the function returns `false`.
fn insert_child_resource(
    resources: &mut [core::ZgResource],
    child_resource: &core::ZgResource,
) -> bool {
    debug!(
        "trying to insert child_resource ('{}') to its parent '{:?}'",
        &child_resource.name, &child_resource.parent_path
    );
    for resource in resources.iter_mut() {
        debug!("  candidate to be inserted: {:?}", &resource.path);
        if resource.path == child_resource.parent_path {
            let parent_resources_vec = resource.resources.get_or_insert(Vec::new());

            if let Some(existing_child) = parent_resources_vec
                .iter_mut()
                .find(|r| r.path == child_resource.path)
            {
                // rare: only observed in "sqladmin:v1" - if eixisting child resource with the same path found, merge methods.
                existing_child
                    .methods
                    .extend(child_resource.methods.clone());
            } else {
                // common: insert the child resource into the parent's sub-resources.
                parent_resources_vec.push(child_resource.clone());
            }
            debug!(
                "  Successfully inserted child_resource: {:?}",
                &child_resource.path
            );
            return true;
        } else if let Some(ref mut children) = resource.resources {
            // Dive into child search only if the child's parent path starts with the current resource's path
            if let Some(ref resource_path) = resource.path {
                if child_resource
                    .parent_path
                    .as_ref()
                    .is_some_and(|p| p.starts_with(resource_path))
                    && insert_child_resource(children, child_resource)
                {
                    return true;
                }
            }
        }
    }
    debug!(
        "  Failed to insert child_resource: {:?}",
        &child_resource.path
    );
    false
}

/// Recursively prints the hierarchy of resources for debugging purposes.
fn debug_resource_hierarchy(resources: &Vec<core::ZgResource>, indent: usize) {
    for resource in resources {
        // Print the current resource with indentation
        debug!(
            "{:indent$}{} (path: {:?}, parent_path: {:?})",
            "",
            resource.name,
            resource.path,
            resource.parent_path,
            indent = indent
        );

        // Recursively print sub-resources, if any
        if let Some(sub_resources) = &resource.resources {
            debug_resource_hierarchy(sub_resources, indent + 2);
        }
    }
}

/// Builds sets of parent resources from flat_path of the resource's methods by removing placeholders (e.g., `{projectsId}`),
/// omitting the last segment (as it's the resource name or method name), and filtering out paths ending with the resource_name.
/// Finally returns the first set of parent resources (e.g., `["projects", "datasets", "tables"]`). Doesn't include the resource itself.
fn build_parent_resources(
    service_name: &str,
    version: &str,
    resource_name: &str,
    methods: &[core::ZgMethod],
) -> Vec<String> {
    let flat_paths = &methods
        .iter()
        .map(|m| m.flat_path.clone())
        .filter(|p| is_valid_flat_path(service_name, p))
        .collect::<HashSet<String>>(); // use HashSet to remove duplicates
    debug!(
        "resource: {}, flat_paths: {:#?}",
        &resource_name, &flat_paths
    );

    let segments: Vec<String> = flat_paths
        .iter()
        .map(|flat_path| {
            flat_path
                .split('/')
                .filter(|segment| !segment.starts_with('{') && !segment.ends_with('}')) // Remove placeholder items
                .map(|s| s.to_string()) // Convert &str to String
                .collect::<Vec<String>>()
        })
        .map(|mut segments| {
            // Remove the last segment (which is the resource itself or method name)
            segments.pop();
            // Remove a version string (e.g., "v1") from segments if any. ref: https://cloud.google.com/apis/design/resource_names#resource_name_vs_url
            segments.retain(|segment| *segment != version);
            segments
        })
        .find(|segments| {
            // Filter out paths where the last element matches the resource_name
            segments
                .last()
                .map_or(true, |last_segment| *last_segment != resource_name)
        })
        .unwrap_or_default(); // An empty Vec if no parent resources are found

    // Most APIs' segment names are equal to the resource names; deal with exceptions by flavor logics.
    match service_name {
        "storage" => flavors::transform_storage_parents(resource_name, segments),
        "compute" => flavors::transform_compute_parents(resource_name, segments),
        "sqladmin" => flavors::transform_sqladmin_parents(segments),
        _ => segments,
    }
}

// Check if the flat_path is appripriate to be used to build the resource hierarchy.
fn is_valid_flat_path(service_name: &str, flat_path: &str) -> bool {
    // Filter out paths ends_with ":xxxxx"
    // Filter out paths contain "/aggregated/" in compute API
    !(flat_path.contains(':') || service_name == "compute" && flat_path.contains("/aggregated/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_api_nested_resources() -> Result<(), Box<dyn Error>> {
        // Extract ZgApi from a mock JSON file, with nested resources.
        let api: core::ZgApi =
            extract_api(PathBuf::from("tests/test_data/container_v1_nested.json"))?;

        // Check that the API ID and name are parsed correctly
        assert_eq!(api.id, "container:v1");
        assert_eq!(api.name, "Container API");
        assert_eq!(api.base_url, "https://container.googleapis.com/");

        // Check that the top-level resource is parsed correctly
        assert_eq!(api.resources.len(), 1);
        assert_eq!(api.resources[0].name, "projects");

        // Check the nested resource (locations inside projects)
        let nested_resources = api.resources[0].resources.as_ref().unwrap();
        assert_eq!(nested_resources.len(), 1);
        assert_eq!(nested_resources[0].name, "locations");

        // Check deeper nested resource (clusters inside locations)
        let deeper_nested_resources = nested_resources[0].resources.as_ref().unwrap();
        assert_eq!(deeper_nested_resources.len(), 1);
        assert_eq!(deeper_nested_resources[0].name, "clusters");

        // Check the methods inside the clusters resource
        let methods = &deeper_nested_resources[0].methods;
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "get");
        assert_eq!(methods[0].http_method, "GET");
        assert_eq!(
            methods[0].flat_path,
            "v1/projects/{projectId}/locations/{location}/clusters/{clusterId}"
        );

        // Check the next level of nested resource (nodePools inside clusters)
        let node_pools_resources = deeper_nested_resources[0].resources.as_ref().unwrap();
        assert_eq!(node_pools_resources.len(), 1);
        assert_eq!(node_pools_resources[0].name, "nodePools");

        let node_pools_methods = &node_pools_resources[0].methods;
        assert_eq!(node_pools_methods.len(), 1);
        assert_eq!(node_pools_methods[0].name, "get");
        assert_eq!(node_pools_methods[0].http_method, "GET");
        assert_eq!(
            node_pools_methods[0].flat_path,
            "v1/projects/{projectId}/locations/{location}/clusters/{clusterId}/nodePools/{nodePoolId}"
        );

        Ok(())
    }

    #[test]
    fn test_convert_resource() {
        // Prepare a mock core::Resource with methods and sub-resources (from container:v1 API)
        let mock_methods = Some(
            vec![
                (
                    "list".to_string(),
                    discovery::Method {
                        description: "Lists all clusters owned by a project in either the specified zone or all zones.".to_string(),
                        flat_path: Some("v1/projects/{projectsId}/locations/{locationsId}/clusters".to_string()),
                        http_method: "GET".to_string(),
                        id: "container.projects.locations.clusters.list".to_string(),
                        parameter_order: None,
                        parameters: None,
                        path: "v1/{+parent}/clusters".to_string(),
                        request: None,
                        response: None,
                        scopes: None,
                    },
                )
            ]
            .into_iter()
            .collect(),
        );

        let mock_sub_resources = Some(
            vec![(
                "nodePools".to_string(),
                discovery::Resource {
                    methods: Some(
                        vec![(
                            "list".to_string(),
                            discovery::Method {
                                description: "Lists the node pools for a cluster.".to_string(),
                                flat_path: Some("v1/projects/{projectsId}/locations/{locationsId}/clusters/{clustersId}/nodePools".to_string()),
                                http_method: "GET".to_string(),
                                id: "container.projects.locations.clusters.nodePools.list".to_string(),
                                parameter_order: None,
                                parameters: None,
                                path: "v1/{+parent}/nodePools".to_string(),
                                request: None,
                                response: None,
                                scopes: None,
                            },
                        )]
                        .into_iter()
                        .collect(),
                    ),
                    resources: None,
                },
            )]
            .into_iter()
            .collect(),
        );

        // Combine the methods and sub-resources into a discovery::Resource
        let resource = discovery::Resource {
            methods: mock_methods,
            resources: mock_sub_resources,
        };

        // Convert the discovery::Resource into a core::ZgResource
        let zg_resource = convert_resource(
            "container",
            "clusters".to_string(),
            resource,
            Some("container.projects.locations".to_string()),
            &HashMap::new(),
        );

        // Assertions
        assert_eq!(zg_resource.name, "clusters");
        assert_eq!(
            zg_resource.path.unwrap(),
            "container.projects.locations.clusters"
        );
        assert_eq!(
            zg_resource.parent_path.unwrap(),
            "container.projects.locations"
        );

        assert_eq!(zg_resource.methods.len(), 1);
        assert_eq!(zg_resource.methods[0].name, "list");
        assert_eq!(
            zg_resource.methods[0].flat_path,
            "v1/projects/{projectsId}/locations/{locationsId}/clusters"
        );
        assert_eq!(zg_resource.methods[0].http_method, "GET");

        // Check sub-resources
        assert!(zg_resource.resources.is_some());
        let sub_resources = zg_resource.resources.as_ref().unwrap();
        assert_eq!(sub_resources.len(), 1);

        assert_eq!(sub_resources[0].name, "nodePools");
        assert_eq!(
            sub_resources[0].path.as_ref().unwrap(),
            "container.projects.locations.clusters.nodePools"
        );
        assert_eq!(
            sub_resources[0].parent_path.as_ref().unwrap(),
            "container.projects.locations.clusters"
        );

        assert_eq!(sub_resources[0].methods.len(), 1);
        assert_eq!(sub_resources[0].methods[0].name, "list");
        assert_eq!(
            sub_resources[0].methods[0].flat_path,
            "v1/projects/{projectsId}/locations/{locationsId}/clusters/{clustersId}/nodePools"
        );
        assert_eq!(sub_resources[0].methods[0].http_method, "GET");
    }

    #[test]
    fn test_build_parent_resources() {
        let methods: Vec<core::ZgMethod> = vec![core::ZgMethod {
            id: "bigquery.datasets.list".to_string(),
            name: "list".to_string(),
            flat_path: "projects/{projectsId}/datasets".to_string(),
            ..core::ZgMethod::testdata()
        }];

        let parent_resources = build_parent_resources("bigquery", "v2", "datasets", &methods);
        assert_eq!(parent_resources, vec!["projects"]);

        let methods: Vec<core::ZgMethod> = vec![core::ZgMethod {
            id: "bigquery.projects.datasets.tables.rowAccessPolicies.list".to_string(),
            name: "list".to_string(),
            flat_path:
                "projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}/rowAccessPolicies"
                    .to_string(),
            ..core::ZgMethod::testdata()
        }];

        let parent_resources =
            build_parent_resources("bigquery", "v2", "rowAccessPolicies", &methods);
        assert_eq!(parent_resources, vec!["projects", "datasets", "tables"]);
    }

    #[test]
    fn test_update_resource_paths() {
        let mut api = core::ZgApi {
            id: "bigquery:v2".to_string(),
            name: "bigquery".to_string(),
            base_url: "https://bigquery.googleapis.com/bigquery/v2/".to_string(),
            resources: vec![
                core::ZgResource {
                    name: "datasets".to_string(),
                    parent_path: None,
                    path: Some("bigquery.datasets".to_string()), // no "projects"
                    methods: vec![core::ZgMethod {
                        name: "list".to_string(),
                        id: "bigquery.datasets.list".to_string(), // no "projects"
                        flat_path: "projects/{projectsId}/datasets".to_string(),
                        ..core::ZgMethod::testdata()
                    }],
                    resources: None, // no sub-resources
                },
                // bigquery:v2 API definition has all-flat (no nest) resources
                core::ZgResource {
                    name: "tables".to_string(),
                    parent_path: None, // looks like top-level
                    path: Some("bigquery.tables".to_string()), // no "projects.datasets"
                    methods: vec![core::ZgMethod {
                        name: "delete".to_string(),
                        id: "bigquery.tables.delete".to_string(), // no "projects.datasets"
                        http_method: "DELETE".to_string(),
                        flat_path: "projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}"
                            .to_string(),
                        ..core::ZgMethod::testdata()
                    }],
                    resources: None,
                },
            ],
            ..core::ZgApi::testdata()
        };

        let updated_api = update_resource_paths(&mut api);

        let datasets = &updated_api.resources[0];
        assert_eq!(
            datasets.path,
            Some("bigquery.projects.datasets".to_string())
        );
        assert_eq!(
            datasets.methods[0].id,
            "bigquery.projects.datasets.list".to_string()
        );

        let tables = &updated_api.resources[1]; // still flat, but path/parent_path/method.id are updated
        assert_eq!(
            tables.path,
            Some("bigquery.projects.datasets.tables".to_string())
        );
        assert_eq!(
            tables.methods[0].id,
            "bigquery.projects.datasets.tables.delete".to_string()
        );
    }

    #[test]
    fn test_update_resource_paths_recursive() {
        let mut api = core::ZgApi {
            id: "sqladmin:v1".to_string(),
            name: "sqladmin".to_string(),
            base_url: "https://sqladmin.googleapis.com/".to_string(),
            resources: vec![core::ZgResource {
                name: "projects".to_string(),
                parent_path: None,
                path: None,
                methods: vec![],
                resources: Some(vec![core::ZgResource {
                    name: "instances".to_string(),
                    parent_path: None,
                    path: None,
                    methods: vec![core::ZgMethod {
                        id: "sql.projects.instances.getDiskShrinkConfig".to_string(),
                        name: "getDiskShrinkConfig".to_string(),
                        flat_path: "v1/projects/{project}/instances/{instance}/getDiskShrinkConfig"
                            .to_string(),
                        ..core::ZgMethod::testdata()
                    }],
                    resources: None,
                }]),
            }],
            ..core::ZgApi::testdata()
        };

        // Call the function to update resource paths
        let updated_api = update_resource_paths(&mut api);

        // Assert the top-level 'projects' resource
        let projects = &updated_api.resources[0];
        assert_eq!(projects.name, "projects");
        assert_eq!(projects.path.as_ref().unwrap(), "sqladmin.projects");
        assert_eq!(projects.parent_path, None);

        // Assert the 'instances' sub-resource under 'projects'
        let instances = &projects.resources.as_ref().unwrap()[0];
        assert_eq!(instances.name, "instances");
        assert_eq!(
            instances.path.as_ref().unwrap(),
            "sqladmin.projects.instances"
        );
        assert_eq!(instances.parent_path.as_ref().unwrap(), "sqladmin.projects");

        // Assert the method under 'instances'
        assert_eq!(instances.methods[0].name, "getDiskShrinkConfig");
        assert_eq!(
            instances.methods[0].id,
            "sqladmin.projects.instances.getDiskShrinkConfig"
        );
        assert_eq!(instances.methods[0].http_method, "GET");
        assert_eq!(
            instances.methods[0].flat_path,
            "v1/projects/{project}/instances/{instance}/getDiskShrinkConfig"
        );
    }

    #[test]
    fn test_insert_child_resource() {
        // Create a mock parent resource
        let parent_resource = core::ZgResource {
            name: "parent".to_string(),
            path: Some("parent_path".to_string()),
            methods: vec![],
            resources: Some(vec![]),
            parent_path: None,
        };

        // Create a mock child resource
        let child_resource = core::ZgResource {
            name: "child".to_string(),
            path: Some("parent_path.child_path".to_string()),
            methods: vec![],
            resources: None,
            parent_path: Some("parent_path".to_string()),
        };

        let mut resources = vec![parent_resource];
        let inserted = insert_child_resource(&mut resources, &child_resource);

        assert!(inserted); // The child resource should be inserted successfully
        assert_eq!(resources[0].resources.as_ref().unwrap().len(), 1);
        assert_eq!(resources[0].resources.as_ref().unwrap()[0].name, "child");
        assert_eq!(
            resources[0].resources.as_ref().unwrap()[0].path,
            Some("parent_path.child_path".to_string())
        );
    }

    #[test]
    fn test_insert_child_resource_nested() {
        // Create a mock grandparent resource
        let mut grandparent_resource = core::ZgResource {
            name: "grandparent".to_string(),
            path: Some("grandparent_path".to_string()),
            methods: vec![],
            resources: Some(vec![]),
            parent_path: None,
        };

        // Create a mock parent resource
        let parent_resource = core::ZgResource {
            name: "parent".to_string(),
            path: Some("grandparent_path.parent_path".to_string()),
            methods: vec![],
            resources: Some(vec![]),
            parent_path: Some("grandparent_path".to_string()),
        };

        // Add the parent resource to the grandparent resource
        grandparent_resource
            .resources
            .as_mut()
            .unwrap()
            .push(parent_resource);

        // Create a mock child resource
        let child_resource = core::ZgResource {
            name: "child".to_string(),
            path: Some("grandparent_path.parent_path.child_path".to_string()),
            methods: vec![],
            resources: None,
            parent_path: Some("grandparent_path.parent_path".to_string()),
        };

        let mut resources = vec![grandparent_resource];
        let inserted = insert_child_resource(&mut resources, &child_resource);
        assert!(inserted); // The child resource should be inserted successfully
        let parent_resources = &resources[0].resources.as_ref().unwrap();
        assert_eq!(parent_resources[0].resources.as_ref().unwrap().len(), 1);
        assert_eq!(
            parent_resources[0].resources.as_ref().unwrap()[0].name,
            "child"
        );
        assert_eq!(
            parent_resources[0].resources.as_ref().unwrap()[0].path,
            Some("grandparent_path.parent_path.child_path".to_string())
        );
    }

    #[test]
    fn test_insert_child_resource_not_found() {
        // Create a mock resource without any children
        let resource = core::ZgResource {
            name: "resource".to_string(),
            path: Some("resource_path".to_string()),
            methods: vec![],
            resources: Some(vec![]),
            parent_path: None,
        };

        // Create a mock child resource
        let child_resource = core::ZgResource {
            name: "child".to_string(),
            path: Some("resource_path.child_path".to_string()),
            methods: vec![],
            resources: None,
            parent_path: Some("still_unknown_parent_path".to_string()),
        };

        // Attempt to insert the child resource into a non-existent parent resource
        let mut resources = vec![resource];
        let result = insert_child_resource(&mut resources, &child_resource);

        assert!(!result);
        assert!(resources[0].resources.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_insert_child_resource_merge_methods_instances() {
        // Create a mock parent resource "projects"
        let parent_resource = core::ZgResource {
            name: "projects".to_string(),
            path: Some("projects".to_string()),
            methods: vec![],
            resources: Some(vec![]),
            parent_path: None,
        };

        // Create a mock child resource "instances" with the first method "get"
        let instances1 = core::ZgResource {
            name: "instances".to_string(),
            path: Some("projects.instances".to_string()),
            methods: vec![core::ZgMethod {
                id: "sqladmin.projects.instances.get".to_string(),
                name: "get".to_string(),
                flat_path: "v1/projects/{project}/instances/{instance}".to_string(),
                ..core::ZgMethod::testdata()
            }],
            resources: None,
            parent_path: Some("projects".to_string()),
        };

        // Create another mock child resource "instances" with a second method "performDiskShrink"
        let instances2 = core::ZgResource {
            name: "instances".to_string(),
            path: Some("projects.instances".to_string()),
            methods: vec![core::ZgMethod {
                id: "sqladmin.projects.instances.performDiskShrink".to_string(),
                name: "performDiskShrink".to_string(),
                flat_path: "v1/projects/{project}/instances/{instance}/performDiskShrink"
                    .to_string(),
                http_method: "POST".to_string(),
                ..core::ZgMethod::testdata()
            }],
            resources: None,
            parent_path: Some("projects".to_string()),
        };

        let mut resources = vec![parent_resource];

        let inserted_first = insert_child_resource(&mut resources, &instances1);
        assert!(
            inserted_first,
            "First child resource should be inserted successfully"
        );
        assert_eq!(resources[0].resources.as_ref().unwrap().len(), 1);
        assert_eq!(
            resources[0].resources.as_ref().unwrap()[0].name,
            "instances"
        );

        let inserted_second = insert_child_resource(&mut resources, &instances2);
        assert!(
            inserted_second,
            "Second child resource should be inserted successfully"
        );

        // Ensure the methods are merged into the same child resource
        let child_resources = &resources[0].resources.as_ref().unwrap();
        let child = &child_resources[0];
        assert_eq!(
            child.methods.len(),
            2,
            "Methods should be merged for the same resource path"
        );
        assert_eq!(child.methods[0].name, "get", "First method should exist");
        assert_eq!(
            child.methods[1].name, "performDiskShrink",
            "Second method should exist"
        );
    }
}
