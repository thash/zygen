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

use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::{to_writer_pretty, Map};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs::File;
use std::path::PathBuf;

use super::core;
use super::supported_apis::SupportedApi;

const DISCOVERED_APIS_FILE: &str = "_discovered_apis.json";
const DISCOVERY_URL: &str = "https://discovery.googleapis.com/discovery/v1/apis";

// ---------------------- Discovery structs ---------------------------------------- //
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryDirectoryList {
    pub kind: String, // "discovery#directoryList"
    pub discovery_version: String,
    pub items: Vec<DiscoveryDirectoryItem>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryDirectoryItem {
    pub kind: String, // "discovery#directoryItem"
    pub id: String,   // API Name + Version  (e.g., "container:v1" or "cloudbilling:v1beta")
    pub name: String, // e.g., "alloydb" or "container"
    pub version: String,
    pub title: String,
    pub description: String,
    pub discovery_rest_url: String,
    pub documentation_link: Option<String>,
    // Only one version is marked as "preferred: true" at a time for a certain service.
    // However, as the "preferred" flag often doesn't match the real-world use cases, zygen doesn't use it strictly.
    pub preferred: bool,
}

// --------------------- Discovered API description -------------------- //
// https://developers.google.com/discovery/v1/reference/apis

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApiDescription {
    pub kind: String,                   // "discovery#restDescription"
    pub id: String,                     // API Name + Version (e.g., "container:v1")
    pub name: String,                   // e.g., "container"
    pub version: String,                // e.g., "v1"
    pub revision: String,               // e.g., "20241022"
    pub canonical_name: Option<String>, // Typically capitalized name (e.g., "Container")
    pub description: String,
    pub discovery_version: String, // Typically, same as version
    pub base_url: String,
    pub base_path: Option<String>,
    pub documentation_link: String,
    pub parameters: Option<HashMap<String, Parameter>>,
    pub protocol: String, // "rest"
    pub resources: Option<HashMap<String, Resource>>,
    pub schemas: Option<HashMap<String, Schema>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Resource {
    pub methods: Option<HashMap<String, Method>>,
    pub resources: Option<HashMap<String, Resource>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Method {
    pub id: String,
    pub http_method: String,
    pub description: String,
    pub path: String,
    pub flat_path: Option<String>, // though `Option`, all APIs except `storage:v1` have flatPath
    pub parameter_order: Option<Vec<String>>,
    pub parameters: Option<HashMap<String, Parameter>>,
    pub request: Option<Request>,
    pub response: Option<Response>,
    pub scopes: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    pub description: Option<String>, // datastore:v1 has a parameter without description
    pub location: String,            // "path" or "query"
    #[serde(rename = "type")]
    pub param_type: String, // "type" is a reserved keyword in Rust, so renamed
    pub enum_values: Option<Vec<String>>,
    pub enum_descriptions: Option<Vec<String>>,
    pub default: Option<String>,
    pub format: Option<String>,
    pub pattern: Option<String>,
    pub required: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Request {
    #[serde(rename = "$ref")]
    pub ref_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    #[serde(rename = "$ref")]
    pub ref_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    pub id: Option<String>,
    pub description: Option<String>,
    pub properties: Option<HashMap<String, SchemaProperty>>,
    // pub required: Option<Vec<String>>, // Not used - comment out to avoid confusion
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SchemaProperty {
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub prop_type: Option<String>, // "type" is a reserved keyword in Rust, so renamed
    pub format: Option<String>,
    pub items: Option<Box<Schema>>,
    pub properties: Option<HashMap<String, Schema>>,
    #[serde(rename = "$ref")]
    pub ref_name: Option<String>, // Reference to another schema (nested/child properties)
    #[serde(default)]
    pub read_only: bool, // default to false if not present
    pub annotations: Option<SchemaPropertyAnnotation>, // Used in limited services: compute and storage
}

// Used in limited services: compute and storage
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SchemaPropertyAnnotation {
    pub required: Vec<String>,
}

// ---------------------- dummy data for tests ----------------------------- //
#[cfg(test)]
impl Schema {
    pub fn testdata() -> Self {
        Self {
            id: Some("testdata".to_string()),
            description: Some("Test schema".to_string()),
            properties: Some(HashMap::new()),
        }
    }
}

#[cfg(test)]
impl SchemaProperty {
    pub fn testdata() -> Self {
        Self {
            description: Some("Test property".to_string()),
            prop_type: Some("string".to_string()),
            format: Some("date-time".to_string()),
            items: None,
            properties: None,
            ref_name: None,
            read_only: false,
            annotations: None,
        }
    }
}

/// Ensure that the discovered APIs are cached in a file, and return the list of APIs.
/// Set `replace` to true to force re-discovery and overwrite the local DISCOVERED_APIS_FILE.
pub async fn ensure_discovered_apis(
    replace: bool,
) -> Result<DiscoveryDirectoryList, Box<dyn Error>> {
    let discovered_apis_file_path = discovered_dir().join(DISCOVERED_APIS_FILE);

    let discovered_apis_json: Value = if !discovered_apis_file_path.exists() && !replace {
        debug!("Discoverying APIs via: {}", DISCOVERY_URL);
        let discovered_apis_json_text = reqwest::get(DISCOVERY_URL).await?.text().await?;
        let j = sort_json(serde_json::from_str(&discovered_apis_json_text)?);

        // Save the discovered APIs JSON to a file
        to_writer_pretty(&mut File::create(&discovered_apis_file_path)?, &j)?;

        j
    } else {
        debug!(
            "Discovered APIs file found at {}",
            discovered_apis_file_path.display()
        );
        let cached_discovered_apis_json_text = std::fs::read_to_string(&discovered_apis_file_path)?;
        sort_json(serde_json::from_str(&cached_discovered_apis_json_text)?)
    };

    let discovered_apis: DiscoveryDirectoryList = serde_json::from_value(discovered_apis_json)?;
    debug!("Total discovered APIs: {}", discovered_apis.items.len());

    Ok(discovered_apis)
}

pub async fn download_api_definition(
    api_id: String,
    discovery_rest_url: String,
) -> Result<PathBuf, Box<dyn Error>> {
    println!("Downloading API definition: {}", discovery_rest_url);
    let api = reqwest::get(discovery_rest_url).await?.text().await?;
    let json: Value = sort_json(serde_json::from_str(&api)?);

    let filepath = discovered_dir().join(format!("{}.json", api_id.replace(":", "_")));
    debug!("Saving API definition: {}", filepath.display());
    let mut f = File::create(&filepath)?;
    to_writer_pretty(&mut f, &json)?;

    Ok(filepath)
}

/// Currently, only Gemini API (generativelanguage) uses this strategy.
pub fn standalone_discovery_url(standalone_api: SupportedApi, api_key: String) -> String {
    match standalone_api.name.as_str() {
        "generativelanguage" => {
            let version = standalone_api
                .versions
                .first()
                .expect("at least one version");
            format!(
                "https://generativelanguage.googleapis.com/$discovery/rest?version={}&key={}",
                version, api_key
            )
        }
        _ => panic!("Unsupported standalone API: {}", standalone_api.name),
    }
}

/// Returns the path to the directory where discovered API JSON files are stored.
/// The directory would be created if it doesn't exist in core::config_dir().
fn discovered_dir() -> PathBuf {
    core::config_dir().join("discovered")
}

/// Sorts JSON fields before into files, so that we can detect exact changes easily. Doesn't sort arrays.
fn sort_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted_map = BTreeMap::new();
            for (k, v) in map {
                sorted_map.insert(k, sort_json(v));
            }
            Value::Object(Map::from_iter(sorted_map))
        }
        _ => value, // Return other types as-is
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_json() {
        // Test the sorting logic for JSON objects
        let unsorted_json = serde_json::json!({
            "b": "value_b",
            "a": {
                "d": "value_d",
                "c": "value_c"
            }
        });

        let sorted_json = sort_json(unsorted_json);

        let expected_json = serde_json::json!({
            "a": {
                "c": "value_c",
                "d": "value_d"
            },
            "b": "value_b"
        });

        assert_eq!(sorted_json, expected_json);
    }
}
