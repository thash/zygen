/*
    This file is NOT the place to collect `zg desc` output for every method individually.
    A rule of thumb is to enable users to find practical strategies to successfully execute methods,
    which is sometime difficult to read from the official API reference (manually) and API Definition JSON (programatically) without heuristic knowledge.

    For example, without flavors, `zg desc sql instances insert` shows `minimum_data: --data '{}'`.
    However, the API responses indicate this minimum_data is a few steps far from the functional minimum:
        --data '{}'               #=> `Invalid request: Missing parameter: Instance.`
        --data '{"aaa": "aaa"}'   #=> `Invalid request: instance name ().`
        --data '{"name": "foo"}'  #=> `Missing parameter: Tier.`
        --data '{"name": "foo", "settings": {"tier": "aaaaaa"}}' #=> `Invalid request: Invalid Tier (aaaaaa) for (EDITION_UNSPECIFIED) Edition.`
    By filling the valid tier, eventually, you would find the practical minimum payload to send. Importantly, you cannot tell it from API reference of [the method](https://cloud.google.com/sql/docs/postgres/admin-api/rest/v1beta4/instances/insert) and [the resource definition](https://cloud.google.com/sql/docs/postgres/admin-api/rest/v1beta4/instances#resource:-databaseinstance). It's a good signs to implement a flavor in this file.

    Note that we prefer to implement flavors when there is little to no guidance and it's difficult to reach the functional minimum except by fair amount of trial and error.
*/
use serde_json::{json, to_string_pretty, Value};
use std::error::Error;

/// Generate the output for zg desc.
fn generate_minimum_data_and_notes(
    data_patterns: Vec<(Option<&str>, Value)>,
    notes: Vec<&str>,
) -> Result<String, Box<dyn Error>> {
    let mut output = String::from("\nminimum_data:\n");
    for (title_option, data) in data_patterns {
        if let Some(title) = title_option {
            output.push_str(&format!("### {}\n", title));
        }
        output.push_str(&format!("--data '{}'\n\n", to_string_pretty(&data)?));
    }
    if !notes.is_empty() {
        output.push_str("notes:\n");
        for note in notes {
            output.push_str(&format!("- {}\n", note));
        }
    }
    Ok(output)
}

/// A macro to briefly define the zg desc output in each flavor function.
macro_rules! template {
    // Multiple data patterns with titles
    (
        $($title:literal >>> { $($key:tt : $value:tt),* $(,)? }),*
        $(<<notes>>)?
        $(<<notes>> $($note:expr),* $(,)?)?
    ) => {{
        let data_patterns = vec![$((Some($title), json!({ $($key: $value),* }))),*];
        let ns = vec![$($($note),*)?];
        generate_minimum_data_and_notes(data_patterns, ns)
    }};

    // A single data without a title
    (
        {$($key:tt : $value:tt),* $(,)? }
        $(<<notes>> $($note:expr),* $(,)?)?
    ) => {{
        let data = vec![(None, json!({ $($key: $value),* }))];
        let ns = vec![$($($note),*)?];
        generate_minimum_data_and_notes(data, ns)
    }};
}

// ------------------------- Flavor implementations ------------------------- //

/// [Justification]
/// The description text of query, load, copy, and extract fields in JobConfiguration start with "[Pick one]," which is an unique strategy to represent Enum-like requirement, but no other services use such expression.
/// Instead of handling "[Pick one]" in desc.rs which only affects BigQuery Jobs insert, it'd be better to treat it as a flavor logic here.
pub fn bigquery_jobs_insert() -> Result<String, Box<dyn Error>> {
    template!(
        "Pattern (1). Query Job" >>> {
            "configuration": {
                "query": { "query": "" }
            }
        },
        "Pattern (2). Load Job" >>> {
            "configuration": {
                "load": {
                    "sourceUris": [],
                    "destinationTable": { "projectId": "", "datasetId": "", "tableId": "" }
                }
            }
        }
        <<notes>>
        "You have to pick one from desired job type: query, load, copy, or extract. https://cloud.google.com/bigquery/docs/reference/rest/v2/Job#JobConfiguration"
    )
}

/// [Justification]
/// No programmatic way to determine the minimum data required to create an instance. We might be able to assume "name" is required as it's an identifier in general, but not sure this assumption works for other services.
/// Even if we could extract "name" as a required field, we would not know that "tier" is required to create an instance unless we execute the API.
pub fn sqladmin_instances_insert() -> Result<String, Box<dyn Error>> {
    template!(
        {"name": "", "settings": {"tier":""}}
        <<notes>>
        "You can find a valid 'tier' by executing `zg ex sql tiers list`"
    )
}

/// [Justification]
/// When you pass "cluster > name" only, the API response indicates Cluster.initial_node_count must be greater than zero, but the field is deprecated.
/// In reallity, we have two valid patterns: (1) specifying nodePool(s), or (2) enable Autopilot.
pub fn container_clusters_create() -> Result<String, Box<dyn Error>> {
    template!(
        "Pattern (1). Standard Cluster" >>> {"cluster": {"name": "", "nodePools": [{"name": ""}]}},
        "Pattern (2). Autopilot Cluster" >>> {"cluster": {"name": "", "autopilot": {"enabled": true}}}
        <<notes>>
        "https://cloud.google.com/kubernetes-engine/docs/reference/rest/v1/projects.locations.clusters#Cluster"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_single_data_no_title_no_notes() {
        let data_patterns = vec![(None, json!({"key": "value"}))];
        let notes = vec![];
        let result = generate_minimum_data_and_notes(data_patterns, notes).unwrap();
        let expected = "\nminimum_data:\n--data '{\n  \"key\": \"value\"\n}'\n\n";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_multiple_data_with_titles_and_notes() {
        let data_patterns = vec![
            (Some("Title 1"), json!({"key1": "value1"})),
            (Some("Title 2"), json!({"key2": "value2"})),
        ];
        let notes = vec!["Note 1", "Note 2"];
        let result = generate_minimum_data_and_notes(data_patterns, notes).unwrap();
        let expected = "\nminimum_data:\n\
                        ### Title 1\n--data '{\n  \"key1\": \"value1\"\n}'\n\n\
                        ### Title 2\n--data '{\n  \"key2\": \"value2\"\n}'\n\n\
                        notes:\n- Note 1\n- Note 2\n";
        assert_eq!(result, expected);
    }
}
