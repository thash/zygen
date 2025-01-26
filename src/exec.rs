use clap::Args;
use log::debug;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{from_str, json, Value};
use std::env;
use std::error::Error;
use std::fs;
use std::process::Command;
use url::Url;

use super::core;

#[derive(Args, Debug)]
pub struct ExecArgs {
    /// Required. Service that has the resource to execute a method (e.g., 'spanner').
    service: String,

    /// Required. Resource that has the method to execute (e.g., 'databases'). Supports resource_path to strictly point an unique resource (e.g., `projects.instances.databases`)
    resource: String,

    /// Required. Method to execute (e.g., 'create').
    method: String,

    /// Extra headers to include in requests. For example, you can override the default Authorization header (`gcloud auth print-access-token`).
    #[arg(short = 'H', long, num_args = 1.., value_parser = parse_headers)]
    headers: Option<Vec<(String, String)>>,

    #[arg(short, long, aliases = &["parameters", "parameter", "param"], num_args = 1.., value_parser = parse_params, help = "Parameters to be used in the request. Accept multiple params (e.g., '-p databaseId=xxx -p key1=value1 -p key2=value2')\n\
    \t(1) Path parameters: Replace placeholders in the URL (e.g., 'v1/xxx/{databaseId}/yyy').\n\
    \t(2) Query parameters: Add key-value pairs to the query string (e.g., v1/xxx?key1=value1&key2=value2).")]
    params: Option<Vec<(String, String)>>,

    /// HTTP request Body. Used when executing a method with http_method=POST/PUT/PATCH.
    /// Format should be JSON string (-d '{"name": "foo"}') or a curl-style filename (-d @body.json). When omitted, it defaults to empty JSON (-d '{}').
    #[arg(short, long)]
    data: Option<String>,

    #[arg(long)]
    equivalent_curl: bool,
}

/// Parse the parameters in the form of KEY=value
fn parse_params(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or("No '=' found. Params must '-p Key=Value'")?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

/// Parse the headers in the form of -H "Key: Value"
fn parse_headers(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find(':')
        .ok_or("No ':' found. HTTP headers must be in the form '-H \"Key: Value\"'")?;
    let key = s[..pos].trim().to_string();
    let value = s[pos + 1..].trim().to_string();
    Ok((key, value))
}

/// main function to execute a method.
pub async fn main(
    args: &ExecArgs,
    standalone_api_key: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let api = core::load_api_file(&args.service, standalone_api_key).await?;
    debug!("Loaded API: {:?}", &api.id);

    let resource = core::find_resource(&api.id, &api.resources, &args.resource)?;
    debug!("Found resource.path: {:?}", &resource.path);

    let method = core::find_method(resource, &args.method)?;
    debug!("Found method: {} {}", &method.name, &method.flat_path);

    if args.equivalent_curl {
        println!("{}", generate_curl(&api.base_url, &method, args)?);
        return Ok(());
    }

    let client = build_client(&args.headers)?;
    let url = build_url(&api.base_url, &method, &args.params)?;

    // Execute the method by sending a request to the URL
    let res = match method.http_method.as_str() {
        "GET" => client.get(url).send().await?.text().await?,
        "DELETE" => client.delete(url).send().await?.text().await?,
        "POST" | "PUT" | "PATCH" => {
            debug!("{} request w/ Data: {:?}", &method.http_method, &args.data);

            // If no --data option is provided, assume an empty JSON (= `--data '{}'`).
            let data = args.data.as_deref().unwrap_or("{}");

            let json_string = prepare_json_string(data)?;

            let reqwest_method = method
                .http_method
                .parse::<reqwest::Method>()
                .map_err(|e| format!("Invalid HTTP method '{}': {}", &method.http_method, e))?;

            client
                .request(reqwest_method, url)
                .body(json_string) // Serialized JSON string from args.data
                .send()
                .await?
                .text()
                .await?
        }
        _ => {
            return Err(format!(
                "Method '{}' uses unsupported HTTP method '{}'",
                &method.name, &method.http_method
            )
            .into())
        }
    };

    debug!("Raw Response: {:?}", &res);

    // Print the result to stdout in pretty JSON format
    let json: Value = if res.is_empty() {
        json!({})
    } else {
        from_str(&res)?
    };
    println!("{}", serde_json::to_string_pretty(&json)?);

    Ok(())
}

/// Build the URL to send a request to
fn build_url(
    base_url: &String,
    method: &core::ZgMethod,
    params: &Option<Vec<(String, String)>>,
) -> Result<String, Box<dyn Error>> {
    let mut path = method.flat_path.clone();
    let mut query_params = Vec::new();

    if let Some(params) = params {
        for (key, value) in params {
            if path.contains(&format!("{{{}}}", key)) {
                path = path.replace(&format!("{{{}}}", key), value); // path params
            } else {
                query_params.push((key.as_str(), value.as_str())); // query params
            }
        }
    }

    // Autofill: replace placeholders (project_id, region, and zone) with values stored in gcloud CLI.
    // If these autofill targets are specified with -p explicitly, they are already replaced in the previous loop.
    path = replace_placeholders(&path, core::PATH_PLACEHOLDERS_PROJECT, "core/project")?;
    path = replace_placeholders(&path, core::PATH_PLACEHOLDERS_REGION, "compute/region")?;
    path = replace_placeholders(&path, core::PATH_PLACEHOLDERS_ZONE, "compute/zone")?;

    let mut url = Url::parse(&format!("{}{}", base_url, path)).expect("Failed to parse URL");
    if !query_params.is_empty() {
        url.query_pairs_mut().extend_pairs(&query_params);
    }

    debug!("Built URL: {}", &url);
    Ok(url.to_string())
}

/// Replace placeholders in the path with values from gcloud config.
/// Only calls get_gcloud_config_value when placeholders are found in the path.
fn replace_placeholders(
    path: &str,
    placeholders: &[&str],
    gcloud_key: &str,
) -> Result<String, Box<dyn Error>> {
    if placeholders
        .iter()
        .any(|&ph| path.contains(&format!("{{{}}}", ph)))
    {
        match get_gcloud_config_value(gcloud_key) {
            Ok(value) => {
                let mut new_path = path.to_string();
                for &placeholder in placeholders {
                    let placeholder_fmt = format!("{{{}}}", placeholder);
                    new_path = new_path.replace(&placeholder_fmt, &value);
                }
                Ok(new_path)
            }
            Err(e) => {
                debug!("{}", e);
                Ok(path.to_string())
            }
        }
    } else {
        Ok(path.to_string()) // No placeholders found; return the path as is
    }
}

/// Get the value of the given key from gcloud CLI
fn get_gcloud_config_value(key: &str) -> Result<String, Box<dyn Error>> {
    let output = Command::new("gcloud")
        .arg("config")
        .arg("get")
        .arg(key)
        .env("PATH", env::var("PATH")?)
        .output()?;

    let value = String::from_utf8(output.stdout)?.trim().to_string();
    if value.is_empty() {
        return Err(format!(
            "No '{}' found in gcloud config. Consider: 'gcloud config set {} {}'",
            key,
            key,
            key.split('/').last().unwrap_or("").to_uppercase()
        )
        .into());
    }

    debug!("Retrieved 'gcloud config get {}' => {:?}", key, &value);
    Ok(value)
}

/// Build a reqwest client with the access token from gcloud CLI
fn build_client(
    custom_headers: &Option<Vec<(String, String)>>,
) -> Result<reqwest::Client, Box<dyn Error>> {
    let mut headers = HeaderMap::new();

    // Inject 'Authorization' header with the (Bearer) access token from gcloud CLI
    let output = Command::new("gcloud")
        .arg("auth")
        .arg("print-access-token")
        .env("PATH", env::var("PATH")?)
        .output()?;
    let access_token = String::from_utf8(output.stdout)?;

    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", access_token.trim()))?,
    );

    // Inject 'Content-Type' header with 'application/json'
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/json; charset=utf-8"),
    );

    if let Some(hs) = custom_headers {
        for (key, value) in hs.iter() {
            headers.insert(key.parse::<HeaderName>()?, value.parse::<HeaderValue>()?);
        }
    }
    debug!("Headers: {:?}", headers);

    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .build()?)
}

/// Prepares the JSON string from the given data argument.
/// If the data starts with '@', it reads the content from the file.
/// Otherwise, it treats the data as a JSON string.
fn prepare_json_string(data: &str) -> Result<String, Box<dyn Error>> {
    let json_data: Value = if data.starts_with('@') {
        let filename = data.trim_start_matches('@');
        debug!("Reading data from file: {}", filename);
        let file_content = fs::read_to_string(filename)
            .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;
        serde_json::from_str(&file_content)
            .map_err(|e| format!("Invalid JSON syntax in file '{}': {}", filename, e))?
    } else {
        serde_json::from_str(data).map_err(|e| format!("Invalid JSON syntax: {}", e))?
    };

    let json_string = serde_json::to_string(&json_data)
        .map_err(|e| format!("Failed to serialize JSON data: {}", e))?;
    Ok(json_string)
}

/// Generates an equivalent curl command for the given HTTP method and arguments.
fn generate_curl(
    base_url: &String,
    method: &core::ZgMethod,
    args: &ExecArgs,
) -> Result<String, Box<dyn Error>> {
    let mut curl_command = format!("curl -X {}", method.http_method);

    let mut custom_header_keys = Vec::<String>::new();
    if let Some(headers) = &args.headers {
        for (key, value) in headers {
            curl_command.push_str(&format!(" \\\n  -H \"{}: {}\"", key, value));
            custom_header_keys.push(key.to_lowercase());
        }
    }

    if !custom_header_keys.contains(&"authorization".to_string()) {
        curl_command
            .push_str(" \\\n  -H \"Authorization: Bearer $(gcloud auth print-access-token)\"");
    }

    if !custom_header_keys.contains(&"content-type".to_string()) {
        curl_command.push_str(" \\\n  -H \"Content-Type: application/json; charset=utf-8\"");
    }

    if let Some(data) = &args.data {
        let json_string = prepare_json_string(data)?; // If --data @filename, expand the content here; otherwise, treat as JSON string
        let json_data: Value = serde_json::from_str(&json_string)?;
        let mut json_pretty = serde_json::to_string_pretty(&json_data)?;

        // If the JSON data is not empty, add a newline before the JSON string
        if !(json_data.is_object() && json_data.as_object().unwrap().is_empty()) {
            json_pretty = format!("\n{}", json_pretty);
        }
        curl_command.push_str(&format!(" \\\n  -d '{}'", json_pretty));
    }

    curl_command.push_str(&format!(
        " \\\n  \"{}\"",
        build_url(base_url, method, &args.params)?
    ));

    Ok(curl_command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_with_path_params() {
        let base_url = "https://example.com/".to_string();
        let method = core::ZgMethod {
            flat_path: "v1/{xyzId}/{locationId}/instances".to_string(),
            ..core::ZgMethod::testdata()
        };
        let params = Some(vec![
            ("xyzId".to_string(), "my-project".to_string()),
            ("locationId".to_string(), "us-central1".to_string()),
        ]);
        let url = build_url(&base_url, &method, &params).unwrap();
        assert_eq!(
            url,
            "https://example.com/v1/my-project/us-central1/instances"
        );
    }

    #[test]
    fn test_build_url_with_query_params() {
        let base_url = "https://example.com/".to_string();
        let method = core::ZgMethod {
            flat_path: "v1/instances".to_string(),
            ..core::ZgMethod::testdata()
        };
        let params = Some(vec![
            ("filter".to_string(), "active".to_string()),
            ("pageSize".to_string(), "10".to_string()),
        ]);
        let url = build_url(&base_url, &method, &params).unwrap();
        assert_eq!(
            url,
            "https://example.com/v1/instances?filter=active&pageSize=10"
        );
    }

    #[test]
    fn test_build_url_with_mixed_params() {
        let base_url = "https://example.com/".to_string();
        let method = core::ZgMethod {
            flat_path: "v1/{xyzId}/instances".to_string(),
            ..core::ZgMethod::testdata()
        };
        let params = Some(vec![
            ("xyzId".to_string(), "my-project".to_string()),
            ("filter".to_string(), "active".to_string()),
        ]);
        let url = build_url(&base_url, &method, &params).unwrap();
        assert_eq!(
            url,
            "https://example.com/v1/my-project/instances?filter=active"
        );
    }

    #[test]
    fn test_build_client() {
        let client = build_client(&None);
        assert!(client.is_ok(), "Client should be built successfully");

        let _ = client
            .unwrap()
            .get("http://example.com")
            .build()
            .expect("Failed to build request");
    }

    #[test]
    fn test_prepare_json_string_from_string() {
        let json_str = r#"{"key": "value"}"#;
        let result = prepare_json_string(json_str).unwrap();
        assert_eq!(result, r#"{"key":"value"}"#);
    }

    #[test]
    fn test_prepare_json_string_invalid_json() {
        let invalid_json_str = r#"{"key": "value""#; // Missing closing brace
        let result = prepare_json_string(invalid_json_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_curl() {
        let base_url = "https://example.com/".to_string();
        let method = core::ZgMethod {
            http_method: "PUT".to_string(),
            flat_path: "v1/resources/{resourcesId}".to_string(),
            ..core::ZgMethod::testdata()
        };
        let args = ExecArgs {
            service: "test_service".to_string(),
            resource: "test_resource".to_string(),
            method: "test_method".to_string(),
            headers: Some(vec![(
                "X-Custom-Header".to_string(),
                "CustomValue".to_string(),
            )]),
            params: Some(vec![
                ("resourcesId".to_string(), "myResourceId".to_string()),
                ("qp1".to_string(), "value1".to_string()),
                ("qp2".to_string(), "value2".to_string()),
            ]),
            data: Some("{\"key\":\"value\"}".to_string()),
            equivalent_curl: false,
        };

        let curl_command = generate_curl(&base_url, &method, &args).unwrap();

        let expected_command = concat!(
            "curl -X PUT \\\n",
            "  -H \"X-Custom-Header: CustomValue\" \\\n",
            "  -H \"Authorization: Bearer $(gcloud auth print-access-token)\" \\\n",
            "  -H \"Content-Type: application/json; charset=utf-8\" \\\n",
            "  -d '\n{\n  \"key\": \"value\"\n}' \\\n",
            "  \"https://example.com/v1/resources/myResourceId?qp1=value1&qp2=value2\""
        );

        assert_eq!(curl_command, expected_command);
    }
}
