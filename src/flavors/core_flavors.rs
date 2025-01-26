use crate::core;
use log::debug;

/// Select "container" resources, priorizing regional clusters/nodePools over zonal ones.
///
///     projects
///       aggregated
///         usableSubnetworks
///       locations
///         operations
///         clusters *1 <<<=== select
///           nodePools *2 <<<=== select
///           well-known
///       zones
///         clusters *1
///           nodePools *2
///         operations
pub fn select_resource_container(found: Vec<&core::ZgResource>) -> Option<&core::ZgResource> {
    debug!("Prioritize regional clusters (locations.clusters) over zonal clsuters. Ref: https://cloud.google.com/blog/products/containers-kubernetes/choosing-a-regional-vs-zonal-gke-cluster");
    found
        .iter()
        .find(|r| {
            r.path
                .as_ref()
                .unwrap()
                .contains("container.projects.locations.clusters")
        })
        .copied()
        .or_else(|| found.first().copied()) // Though unlikely to happen, fallback to first if not found.
}

/// Select "dataflow" resources, priorizing regional resources (templates, jobs, etc).
/// - *1: "jobs" and its subresources; "jobs" under projects or locations. Prefer the regional one: "locations.jobs".
/// - *2: "templates" ... under projects or locations. Prefer the regional one: "locations.templates".
/// - *3: "snapshots" ... methods under snapshots are undocumented, but calling [gcloud dataflow snapshots delete](https://cloud.google.com/sdk/gcloud/reference/dataflow/snapshots/delete) with `--log-http` indicates `locations.snapshots` is used.
///
///     projects
///       snapshots *3
///       jobs *1
///         workItems *1
///         messages *1
///         debug *1
///       templates *2
///       locations
///         flexTemplates
///         templates *2 <<<=== select
///         jobs *1 <<<=== select (and its *1 subresources)
///           debug *1
///           stages
///           snapshots *3
///           messages *1
///           workItems *1
///         snapshots *3 <<<=== select
pub fn select_resource_dataflow<'a>(
    resource_path: &str,
    found: Vec<&'a core::ZgResource>,
) -> Option<&'a core::ZgResource> {
    if resource_path.ends_with("templates")
        || resource_path.ends_with("jobs")
        || resource_path.ends_with("debug")
        || resource_path.ends_with("messages")
        || resource_path.ends_with("workItems")
    {
        //templates, jobs, or jobs' sub-resources
        debug!("Recommend using jobs, jobs' sub-resources, and templates with locations (regional endpoint). Ref: https://cloud.google.com/dataflow/docs/reference/rest/v1b3/projects.jobs/create");
        found
            .iter()
            .find(|r| r.path.as_ref().unwrap().contains("locations"))
            .copied()
            .or_else(|| found.last().copied())
    } else {
        // snapshots
        debug!("Prefer 'locations.snapshots' over 'locations.jobs.snapshots' or 'projects.snapshots' as per gcloud dataflow command output.");
        found
            .iter()
            .find(|r| r.path.as_ref().unwrap().contains("locations.snapshots"))
            .copied()
            .or_else(|| found.last().copied())
    }
}

/// Select "spanner" resources, assuming 'instances.operations' as the default choice for 'operations' resource.
///
///     scans
///     projects
///       instanceConfigOperations
///       instanceConfigs
///         ssdCaches
///           operations *
///         operations *
///       instances
///         databaseOperations
///         databases
///           sessions
///           backupSchedules
///           databaseRoles
///           operations *
///         instancePartitions
///           operations *
///         backupOperations
///         operations * <<<=== select
///         backups
///           operations *
///         instancePartitionOperations
pub fn select_resource_spanner(found: Vec<&core::ZgResource>) -> Option<&core::ZgResource> {
    debug!("Spanner has 6 resources named 'operations'. 'instances.operations' and 'databases.operations' are common, and here select one under 'instnaces'. Ref: https://cloud.google.com/spanner/docs/manage-and-observe-long-running-operations");
    found
        .iter()
        .find(|r| r.path.as_ref().unwrap().contains("instances.operations"))
        .copied()
        .or_else(|| found.last().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_resource_container() {
        let res1 = core::ZgResource {
            path: Some("container.projects.locations.clusters".to_string()),
            ..core::ZgResource::testdata()
        };
        let res2 = core::ZgResource {
            path: Some("container.projects.zones.clusters".to_string()),
            ..core::ZgResource::testdata()
        };

        let selected = select_resource_container(vec![&res1, &res2]);
        assert_eq!(
            selected.unwrap().path.as_deref(),
            Some("container.projects.locations.clusters")
        );
    }

    #[test]
    fn test_select_resource_dataflow() {
        let resource_path = "templates";

        let res1 = core::ZgResource {
            path: Some("dataflow.projects.locations.templates".to_string()),
            ..core::ZgResource::testdata()
        };
        let res2 = core::ZgResource {
            path: Some("dataflow.projects.templates".to_string()),
            ..core::ZgResource::testdata()
        };

        let selected = select_resource_dataflow(resource_path, vec![&res1, &res2]);
        assert_eq!(
            selected.unwrap().path.as_deref(),
            Some("dataflow.projects.locations.templates")
        );
    }

    #[test]
    fn test_select_resource_spanner() {
        let op1 = core::ZgResource {
            path: Some("spanner.projects.instances.operations".to_string()),
            ..core::ZgResource::testdata()
        };
        let op2 = core::ZgResource {
            path: Some("spanner.projects.instances.databases.operations".to_string()),
            ..core::ZgResource::testdata()
        };

        let selected = select_resource_spanner(vec![&op1, &op2]);
        assert_eq!(
            selected.unwrap().path.as_deref(),
            Some("spanner.projects.instances.operations")
        );
    }
}
