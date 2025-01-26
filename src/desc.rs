use clap::Args;
use log::debug;
use regex::Regex;
use serde_json::{json, to_string_pretty, Value};
use std::collections::HashMap;
use std::{error::Error, panic};
use urlencoding::encode;

use crate::discovery;

use super::core;
use super::flavors::desc_flavors as flavors;

#[derive(Args, Debug)]
pub struct DescArgs {
    /// Required. Service that has the resource to execute a method (e.g., 'container').
    service: String,

    /// A Resource to describe (e.g., 'clusters'). Supports resource_path to strictly point an unique resource (e.g., `locations.clusters`)
    resource: Option<String>,

    /// A Method to describe (e.g., 'get).
    method: Option<String>,
}

/// Main function to describe services, resources, or methods.
/// standalone_api_key is only used for lazy loading (downloading) the API file through discovery url.
pub async fn main(
    args: &DescArgs,
    standalone_api_key: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let api = core::load_api_file(&args.service, standalone_api_key).await?;
    match (&args.resource, &args.method) {
        (None, None) => describe_service(&api),
        (Some(resource_path), None) => {
            let resource = core::find_resource(&api.id, &api.resources, resource_path)?;
            describe_resource(resource)
        }
        (Some(resource_path), Some(method_name)) => {
            let resource = core::find_resource(&api.id, &api.resources, resource_path)?;
            let method = core::find_method(resource, method_name)?;
            describe_method(&method, &api)
        }
        (None, Some(_)) => panic!("Fatal: Method cannot be specified without a resource."),
    }
}

/// Describes the service. Prints only the top-level resources (ignore nested resources).
fn describe_service(api: &core::ZgApi) -> Result<(), Box<dyn Error>> {
    println!("service: {}", &api.name);
    println!("version: {}", &api.version);
    println!("revision: {}", &api.revision);
    println!("base_url: {}", api.base_url);
    println!("top_level_resources:");
    for resource in &api.resources {
        println!("- {}", resource.name);
    }
    Ok(())
}

/// Describes the resource. Prints the direct children resources and methods (ignores nested resources).
fn describe_resource(resource: &core::ZgResource) -> Result<(), Box<dyn Error>> {
    println!("resource_name: {}", resource.name);
    println!(
        "resource_path: {}",
        resource.path.as_deref().unwrap_or("N/A")
    );
    println!(
        "parent_path: {}",
        resource.parent_path.as_deref().unwrap_or("N/A")
    );
    if !resource.methods.is_empty() {
        println!("methods:");
        for method in &resource.methods {
            println!("- {}", method.name);
        }
    }
    if let Some(children) = &resource.resources {
        if !children.is_empty() {
            println!("\nchild_resources:");
            for child in resource.resources.as_ref().unwrap() {
                println!("- {}", child.name);
            }
        }
    }
    Ok(())
}

/// Describes the method. Prints information useful for executing the method.
fn describe_method(method: &core::ZgMethod, api: &core::ZgApi) -> Result<(), Box<dyn Error>> {
    println!("method_name: {}", method.name);
    println!("method_id: {}", method.id);
    if let Some(original_id) = &method.original_id {
        println!("original_method_id: {}", original_id);
    }
    println!("http_method: {}", method.http_method);
    println!("request_url: {}{}", &api.base_url, method.flat_path);
    println!("autofill_params: {}", autofill_params(method).join(", "));

    let required_params = build_required_params_string(method)?;
    println!("\nrequired_params: {}", required_params);

    // Only show suggested minimum data for non-GET/DELETE methods
    if !["GET", "DELETE"].contains(&method.http_method.as_str()) {
        println!("{}", payload_suggestion(method, api)?);
    }

    // Generate and display the document search result URL
    if let Some(doc_url) = generate_documentation_link(&method.id) {
        println!("\nFind API Reference: {}", doc_url);
    }

    Ok(())
}

/// Extracts the placeholders that will be autofilled in `zg exec`.
fn autofill_params(method: &core::ZgMethod) -> Vec<String> {
    // Extract all placeholders from the flat_path
    let re = Regex::new(r"\{([^}]+)\}").unwrap();
    let placeholders: Vec<String> = re
        .captures_iter(&method.flat_path)
        .filter_map(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .collect();

    // Combine all placeholders that will be autofilled
    let autofill_placeholders: Vec<&str> = core::PATH_PLACEHOLDERS_PROJECT
        .iter()
        .chain(core::PATH_PLACEHOLDERS_REGION.iter())
        .chain(core::PATH_PLACEHOLDERS_ZONE.iter())
        .cloned()
        .collect();

    // Filter placeholders that will be autofilled in `zg exec`
    placeholders
        .into_iter()
        .filter(|param| autofill_placeholders.contains(&param.as_str()))
        .collect()
}

/// Builds the required parameters string.
fn build_required_params_string(method: &core::ZgMethod) -> Result<String, Box<dyn Error>> {
    let re = Regex::new(r"\{([^}]+)\}")?;

    // Collect required "path" params
    let mut required_params: Vec<&str> = re
        .captures_iter(&method.flat_path)
        .filter_map(|cap| cap.get(1))
        .map(|m| m.as_str())
        .filter(|&param| !autofill_params(method).contains(&param.to_string()))
        .collect();

    // Collect required "query" params
    let required_query_params: Vec<&str> = method
        .query_params
        .iter()
        .filter(|qp| qp.required)
        .map(|qp| qp.name.as_str())
        .collect();
    required_params.extend(required_query_params);

    if required_params.is_empty() {
        Ok("None".to_string())
    } else {
        let params_line = required_params
            .iter()
            .map(|param| format!("-p {}=\"\"", param))
            .collect::<Vec<String>>()
            .join(" ");
        Ok(format!("\n{}", params_line))
    }
}

/// Generates a suggestion for the minimum request data to be sent with the method.
fn payload_suggestion(
    method: &core::ZgMethod,
    api: &core::ZgApi,
) -> Result<String, Box<dyn Error>> {
    match method.id.as_str() {
        "bigquery.projects.jobs.insert" => flavors::bigquery_jobs_insert(),
        "sqladmin.projects.instances.insert" => flavors::sqladmin_instances_insert(),
        "container.projects.locations.clusters.create"
        | "container.projects.zones.clusters.create" => flavors::container_clusters_create(),
        _ => {
            // When no flavored logic is defined for the method, builds the suggested minimum request data string,
            // by generating a JSON template with placeholder values for required fields.
            let request_data_schema = match &method.request_data_schema {
                Some(s) => s,
                None => return Ok("\nminimum_data:\n--data '{}'".to_string()), // Doc says "The request body must be empty"
            };

            let data = minimum_data_suggestion(method, request_data_schema, &api.schemas);
            let output = format!("\nminimum_data:\n--data '{}'", to_string_pretty(&data)?);

            Ok(output)
        }
    }
}

/// Recursively builds a JSON object with placeholder values for required fields,
/// handling nested schemas where necessary.
fn minimum_data_suggestion(
    method: &core::ZgMethod,
    schema: &discovery::Schema,
    schemas: &HashMap<String, discovery::Schema>,
) -> serde_json::Value {
    let properties = match &schema.properties {
        Some(props) => props,
        None => return json!({}),
    };

    let mut min_data = serde_json::Map::new();
    let unsupported_msg = Value::String("<<See API Reference for details>>".to_string());

    // Iterate over the properties and add placeholder values to build template JSON
    for (field, prop) in properties.iter() {
        if !is_required(method, field, prop, properties.len() == 1) {
            continue;
        }

        let placeholder_value = match prop.prop_type.as_deref() {
            Some("string") => Value::String("".to_string()),
            Some("integer") => Value::Number(0.into()),
            Some("boolean") => Value::Bool(false),
            Some(_) => unsupported_msg.clone(),
            None => match &prop.ref_name {
                None => unsupported_msg.clone(), // no prop_type and no "$ref (ref_name)" - expect not to happen
                // no prop_type but Some(ref_name); try to recursively resolve the nested schema
                Some(ref_name) => match schemas.get(ref_name) {
                    None => unsupported_msg.clone(),
                    Some(nested_schema) => minimum_data_suggestion(method, nested_schema, schemas),
                },
            },
        };
        min_data.insert(field.clone(), placeholder_value);
    }

    serde_json::Value::Object(min_data)
}

/// Determines if a property is required based on its description and annotations.
/// If the property is read-only, it is not considered required as users don't send it to call the API.
fn is_required(
    method: &core::ZgMethod,
    field: &String,
    prop: &discovery::SchemaProperty,
    is_only_prop: bool,
) -> bool {
    if prop.read_only {
        return false;
    }
    debug_property(field, prop);

    // If the description suggests the property is optional. Don't immediately return false, rather support other conditions.
    let desc_indicates_optional = prop.description.as_deref().is_some_and(|desc| {
        let desc_lower = desc.to_lowercase();
        desc_lower.starts_with("output only") || desc_lower.starts_with("optional")
    });

    // Required if this is the only property in the schema
    if is_only_prop && !desc_indicates_optional {
        return true;
    }

    // Required if property's description contains "Required" or starts with "Identifier."
    let desc_indicates_requirement = prop
        .description
        .as_deref()
        .is_some_and(|desc| desc.contains("Required") || desc.starts_with("Identifier."));

    // Required if property's annotations contains the method id (a strategy used only in "compute" and "storage")
    let annotated_as_required = method
        .original_id
        .as_ref()
        .and_then(|method_id| {
            prop.annotations
                .as_ref()
                .map(|annotations| annotations.required.as_ref())
                .map(|required_methods: &Vec<String>| required_methods.contains(method_id))
        })
        .unwrap_or(false);

    (desc_indicates_requirement || annotated_as_required) && !desc_indicates_optional
}

/// Generates a link to the method documentation (in reality, a search result page).
fn generate_documentation_link(method_id: &str) -> Option<String> {
    let parts: Vec<&str> = method_id.split('.').collect();
    let (service_name, resource_path, method_name) = match parts.as_slice() {
        [service_name, resource @ .., method_name] => {
            (service_name, resource.join("."), method_name)
        }
        _ => return None,
    };

    let search_query = format!("\"Method:\" {} {}", resource_path, method_name);
    let encoded_query = encode(&search_query);
    let url = format!(
        "https://cloud.google.com/s/results/{}/docs?q={}",
        service_name, encoded_query
    );

    Some(url)
}

fn debug_property(field: &String, prop: &discovery::SchemaProperty) {
    debug!(
        "Property '{}': {:?} (child: {:?}){}",
        &field,
        &prop.description.as_ref().expect("No description"),
        &prop.ref_name,
        if prop.properties.is_some() {
            " (+ found child properties)"
        } else {
            ""
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vecs;
    use std::collections::HashMap;

    #[test]
    fn test_build_required_params_string() {
        let method = core::ZgMethod {
            flat_path: "/resource1/{param1}/method1".to_string(),
            ..core::ZgMethod::testdata()
        };

        let result = build_required_params_string(&method);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "\n-p param1=\"\"");
    }

    #[test]
    fn test_payload_suggestion_default() {
        let mut properties = HashMap::new();

        // read_only: false, and required
        properties.insert(
            "requiredField".to_string(),
            discovery::SchemaProperty {
                description: Some("Required. And something happens.".to_string()),
                ..discovery::SchemaProperty::testdata()
            },
        );

        // read_only: false, but not required
        properties.insert(
            "optionalField".to_string(),
            discovery::SchemaProperty {
                description: Some("Optional. It's up to you.".to_string()),
                ..discovery::SchemaProperty::testdata()
            },
        );

        // read_only: true
        properties.insert(
            "outputField".to_string(),
            discovery::SchemaProperty {
                description: Some(
                    "Output only. You don't specify it when executing API.".to_string(),
                ),
                read_only: true,
                ..discovery::SchemaProperty::testdata()
            },
        );

        let method = core::ZgMethod {
            request_data_schema: Some(discovery::Schema {
                properties: Some(properties),
                ..discovery::Schema::testdata()
            }),
            ..core::ZgMethod::testdata()
        };

        // The result should only contain the required field
        let result = payload_suggestion(&method, &core::ZgApi::testdata());
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            "\nminimum_data:\n--data '{\n  \"requiredField\": \"\"\n}'"
        );
    }

    #[test]
    fn test_is_required_description() {
        // Case where description contains "Required"
        let prop = discovery::SchemaProperty {
            description: Some("Fully qualified name. Required when ...".to_string()),
            read_only: false,
            ..discovery::SchemaProperty::testdata()
        };
        assert!(
            is_required(
                &core::ZgMethod::testdata(),
                &String::from("myfield"),
                &prop,
                false
            ),
            "Expected true due to 'Required' in description."
        );

        let prop2 = discovery::SchemaProperty {
            description: Some("Identifier. The resource name of ...".to_string()),
            read_only: false,
            ..discovery::SchemaProperty::testdata()
        };

        assert!(
            is_required(
                &core::ZgMethod::testdata(),
                &String::from("myfield2"),
                &prop2,
                false
            ),
            "Expected true as description starts with 'Identifier.'"
        );
    }

    #[test]
    fn test_is_required_annotations_match() {
        // Case where annotations contain the method ID "compute.instances.insert"
        let prop = discovery::SchemaProperty {
            description: Some("The name of the resource.".to_string()),
            read_only: false,
            annotations: Some(discovery::SchemaPropertyAnnotation {
                required: vecs!["compute.instances.insert"],
            }),
            ..discovery::SchemaProperty::testdata()
        };

        let meth = &core::ZgMethod {
            id: "compute.projects.zones.instances.insert".to_string(),
            original_id: Some("compute.instances.insert".to_string()),
            ..core::ZgMethod::testdata()
        };

        assert!(
            is_required(meth, &String::from("myfield"), &prop, false),
            "Expected true due to matching annotation method ID."
        );
    }

    #[test]
    fn test_generate_documentation_link() {
        let method_id = "compute.instances.insert";
        let result = generate_documentation_link(method_id);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            "https://cloud.google.com/s/results/compute/docs?q=%22Method%3A%22%20instances%20insert"
        );
    }
}
