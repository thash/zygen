use crate::vecs;
use std::iter::once;

/// Deal with the unique path strategy of "storage:v1" (Google Cloud Storage),
/// which uses abbreviated names in flat_path ("bucets" => "b", "objects" => "o").
pub fn transform_storage_parents(resource_name: &str, segments: Vec<String>) -> Vec<String> {
    // Return fixed parents for "buckets", "objects", "folders", and "managedFolders".
    match resource_name {
        "buckets" => return vecs!["projects"],
        "objects" | "folders" | "managedFolders" => return vecs!["projects", "buckets"],
        // For the "projects" resource, return the given segments as-is.
        "projects" => return segments,
        _ => (),
    };

    // Otherwise, rooting from "projects", treat "b" and "o" in the paths as "buckets" and "objects"
    once("projects")
        .chain(segments.iter().map(String::as_str))
        // .into_iter()
        .map(|name| match name {
            "b" => "buckets".to_string(),
            "o" => "objects".to_string(),
            _ => name.to_string(),
        })
        .collect()
}

/// For compute API, removes unnecessary segments that are not defined as resources in the API definition.
pub fn transform_compute_parents(resource_name: &str, segments: Vec<String>) -> Vec<String> {
    // The following resources cannot identify their hierarchy from the flat_path; so manually set the parents.
    match resource_name {
        "globalOrganizationOperations" => vecs![],
        "globalAddresses"
        | "globalNetworkEndpointGroups"
        | "globalOperations"
        | "globalForwardingRules"
        | "networkFirewallPolicies" => vecs!["projects"],
        "instanceGroupManagerResizeRequests" => {
            vecs!["projects", "zones", "instanceGroupManagers"]
        }
        "zoneOperations" => vecs!["projects", "zones"],
        resource if resource.starts_with("region") && resource != "regions" => {
            vecs!["projects", "regions"]
        }
        _ => segments
            .into_iter()
            .filter(|segment| segment != "global" && segment != "locations")
            .collect(),
    }
}

/// Cloud SQL Admin API v1beta4 contains "sql" at the top of the path; remove it
/// ref: https://cloud.google.com/sql/docs/postgres/admin-api/rest
pub fn transform_sqladmin_parents(segments: Vec<String>) -> Vec<String> {
    segments.into_iter().filter(|seg| seg != "sql").collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_storage_parents_buckets() {
        let resource_name = "buckets";
        let segments = vecs!["any", "segments", "here"];
        let result = transform_storage_parents(resource_name, segments);
        assert_eq!(result, vecs!["projects"]);
    }

    #[test]
    fn test_transform_storage_parents_object_access_controls() {
        let resource_name = "objectAccessControls";
        let segments = vecs!["b", "o"];
        let result = transform_storage_parents(resource_name, segments);
        assert_eq!(result, vecs!["projects", "buckets", "objects"]);
    }
}
