use clap::Args;
use prettytable::{format, row, Cell, Row, Table};
use std::cmp::max;
use std::error::Error;
use std::fmt::Write;
use std::str::FromStr;

use super::core;
use super::supported_apis::{supported_apis, SupportedApi};

#[derive(Args, Debug, Default)]
pub struct ListArgs {
    /// The service (e.g., "compute") for which list underlying resources. If omitted, lists all available services (APIs).
    service: Option<String>,

    /// The resource (e.g., "instances") for which list underlying methods. Requires [SERVICE] argument.
    resource: Option<String>,

    /// The method (e.g., "delete") of a resource. Requires [SERVICE] and [RESOURCE] argument. Typically, listing a single method is not helpful.
    method: Option<String>, // Typically not helpful, but allowed for compatibility with other commands (desc, exec)

    /// List all items.
    #[arg(short = 'A', long)]
    all: bool,

    /// Show aliases of services. Effective only when listing services without --long.
    #[arg(short = 'a', long)]
    aliases: bool,

    /// Show service category with title. Effective only when listing services.
    #[arg(short = 'c', long)]
    category: bool,

    /// Display detailed information in long format.
    #[arg(short, long)]
    long: bool,

    /// Colorize the output.
    #[arg(short = 'C', long)]
    color: bool,

    #[arg(
        short = 'S',
        long,
        help = "Sort services, methods, or resources by the given field.\n\
    \tServices' sortable fields: [id, name, aliases, versions]\n\
    \tResources' sortable fields: [name, depth, path, methods]. Effective only with --long.\n\
    \tMethods' sortable fields: [name, http, path]"
    )]
    sort: Option<String>,

    /// Reverse the sort order. Reversing resources takes effect only with --long.
    #[arg(short, long)]
    reverse: bool,
}

/// Main function to handle listing of services, resources, or methods.
/// standalone_api_key is only used for lazy loading (downloading) the API file through discovery url.
///
/// - If no service, it calls `list_services` to list all available services and returns early.
/// - If a service is specified, it loads the corresponding API file using `core::load_api_file`.
///   - If no resource path, the function lists all resources for the service.
///   - If a resource path is specified, the function lists the methods of the resource.
///     - If a method is specified, it lists only that method (Note: This is not very useful).
pub async fn main(
    args: &ListArgs,
    standalone_api_key: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let output = match (&args.service, &args.resource, &args.method) {
        (None, _, _) => {
            // No service specified; list all services
            list_services(args)
        }
        (Some(svc), None, _) => {
            // Service specified; list resources
            let api = core::load_api_file(svc, standalone_api_key).await?;
            list_resources(&api, args)
        }
        (Some(svc), Some(resource_path), _) => {
            // Service and resource specified; list methods
            let api = core::load_api_file(svc, standalone_api_key).await?;
            list_methods(&api, resource_path, args)
        }
    }?;

    print!("{}", output);
    Ok(())
}

#[rustfmt::skip]
#[allow(clippy::wildcard_in_or_patterns)]
/// Function to list all available services. With the `--all` flag, it lists all services including the SUB_SUPPORTED_APIS.
fn list_services(args: &ListArgs) -> Result<String, Box<dyn Error>> {
    let mut apis = supported_apis(args.all);

    // Sort the services based on the --sort field; default sort key is name.
    let sort_field = &args.sort.as_deref().unwrap_or("name");
    apis.sort_by(|a, b| {
        let sorted = match *sort_field {
            "title" | "api_title" => a.title.cmp(&b.title),
            "category" | "categories" => a.category.cmp(&b.category),
            "aliases" | "alias" => a.aliases.cmp(&b.aliases),
            "versions" | "version" => a.versions.cmp(&b.versions),
            "default_version" => a.default_version().cmp(b.default_version()), // practically same as "versions"
            "name" | "api_name" | _ => a.name.cmp(&b.name), // fallback
        };
        if args.reverse { sorted.reverse() } else { sorted }
    });

    if args.long {
        let mut table = initialize_services_table();
        for api in apis {
            table.add_row(row![
                api.name,
                api.title,
                api.category,
                api.aliases.join(", "),
                api.versions.join(", "),
                api.default_version()
            ]);
        }

        table.print_tty(true)?;
        Ok(String::new()) // Return empty string since --long format is printed directly by print_tty() above
    } else {
        let service_line = |api: &SupportedApi| {
            match (args.aliases && !api.aliases.is_empty(), args.category) {
                (true, true) => format!(
                    "[{}] {} - {} ({})",
                    api.category, api.title, api.name, api.aliases.join(", ")
                ),
                (true, false) => format!("{} ({})", api.name, api.aliases.join(", ")),
                (false, true) => format!("[{}] {} - {}", api.category, api.title, api.name),
                (false, false) => api.name.to_owned(),
            }
        };

        let output = apis.iter().map(service_line).collect::<Vec<_>>().join("\n");

        Ok(format!("{}\n", output)) // Add a newline at the end
    }
}

fn initialize_services_table() -> Table {
    let mut t = Table::new();
    t.set_format(*format::consts::FORMAT_CLEAN);
    t.set_titles(row![bu->"name", b->"title", b-> "category", b->"aliases", b->"versions", b->"default_version"]);
    t
}

/// Returns a string of all resources in the API.
fn list_resources(api: &core::ZgApi, args: &ListArgs) -> Result<String, Box<dyn Error>> {
    let resources = &api.resources;

    if args.long {
        let mut table = initialize_resources_table();

        // With --color option, find duplicated resource names to highlight
        let duplicated_resources = args
            .color
            .then(|| api.duplicated_resources())
            .unwrap_or_default();

        add_resource_rows(&mut table, resources, args, &duplicated_resources);

        // Sorting should happen here, after recursively collected all resources into the table in add_resource_rows()
        if let Some(sort_field) = &args.sort {
            table = sort_resources_table(&table, sort_field, args.reverse)?;
        }

        table.print_tty(true)?;

        Ok(String::new()) // Return empty string since --long format is printed directly by print_tty() above
    } else {
        // Without --long option, print only the resource names in a tree (indented) format
        render_resources_tree(resources, "")
    }
}

/// Initialize a table with headers to store resources.
fn initialize_resources_table() -> Table {
    let mut t = Table::new();
    t.set_format(*format::consts::FORMAT_CLEAN);
    t.set_titles(Row::new(vec![
        Cell::new("name").style_spec("bu"),
        Cell::new("depth").style_spec("b"),
        Cell::new("resource_path").style_spec("b"),
        Cell::new("methods").with_hspan(2).style_spec("b"), // span 2 columns for "method_count" and "method_name"
    ]));
    t
}

/// Helper function to add resources to rows in the table, recursively (used when --long).
fn add_resource_rows(
    table: &mut Table,
    resources: &[core::ZgResource],
    args: &ListArgs,
    duplicated_resources: &Vec<(String, Vec<String>)>,
) {
    for resource in resources {
        let mut method_names: Vec<String> =
            resource.methods.iter().map(|m| m.name.clone()).collect();

        method_names.sort_by_key(|name| (name.len(), name.clone())); // Sort method names by length, then alphabetically

        // Colorize the resource name if it has duplicates (i.e, same name but different paths)
        let resource_name_cell = if duplicated_resources
            .iter()
            .any(|(name, _)| name == &resource.name)
        {
            Cell::new(&resource.name).style_spec("Fb")
        } else {
            Cell::new(&resource.name)
        };

        // Calculate the depth of the resource path - starting from 0
        let depth_cell = Cell::new(
            (max(1, resource.path.as_ref().unwrap().matches('.').count()) - 1)
                .to_string()
                .as_str(),
        );

        // Display only the first 5 methods, unless --all flag is set
        let method_names_cell = if !args.all && method_names.len() > 5 {
            Cell::new(format!("{}, ...", method_names[..5].join(", ")).as_str())
        } else {
            Cell::new(method_names.join(", ").as_str())
        };

        // Add the resource row to the table
        table.add_row(Row::new(vec![
            resource_name_cell,
            depth_cell,
            Cell::new(resource.path.as_ref().unwrap()),
            Cell::new(resource.methods.len().to_string().as_str()),
            method_names_cell,
        ]));

        if let Some(sub_resources) = &resource.resources {
            add_resource_rows(table, sub_resources, args, duplicated_resources);
        }
    }
}

#[allow(clippy::wildcard_in_or_patterns)]
/// Helper function to sort the resources in the table based on the --sort field.
fn sort_resources_table(
    table: &Table,
    sort_field: &str,
    reverse: bool,
) -> Result<Table, Box<dyn Error>> {
    let mut rows: Vec<Row> = table.row_iter().cloned().collect();

    // Internal helper function to fetch cell content and parse it into a specific type
    fn cell<T: FromStr + Default>(row: &Row, index: usize) -> T {
        row.get_cell(index)
            .and_then(|cell| cell.get_content().parse::<T>().ok())
            .unwrap_or_default()
    }

    rows.sort_by(|a, b| {
        match sort_field {
            "name" | "resource_name" => {
                // Primary sort by resource name (column idx: 0), secondary by depth (column idx: 1), then by path (column idx: 2)
                cell::<String>(a, 0)
                    .cmp(&cell::<String>(b, 0))
                    .then_with(|| cell::<usize>(a, 1).cmp(&cell::<usize>(b, 1)))
                    .then_with(|| cell::<String>(a, 2).cmp(&cell::<String>(b, 2)))
            }
            "depth" => {
                // Primary sort by depth (column idx: 1), secondary by resource name (column idx: 0)
                cell::<usize>(a, 1)
                    .cmp(&cell::<usize>(b, 1))
                    .then_with(|| cell::<String>(a, 0).cmp(&cell::<String>(b, 0)))
            }
            "method" | "methods" | "method_count" => {
                // Primary sort by method count (column idx: 3), secondary by path (column idx: 2)
                cell::<usize>(a, 3)
                    .cmp(&cell::<usize>(b, 3))
                    .then_with(|| cell::<String>(a, 2).cmp(&cell::<String>(b, 2)))
            }
            "path" | "resource_path" | _ => cell::<String>(a, 2).cmp(&cell::<String>(b, 2)), // fallback
        }
    });

    if reverse {
        rows.reverse()
    }

    let mut sorted_table = initialize_resources_table();
    for row in rows {
        sorted_table.add_row(row);
    }

    Ok(sorted_table)
}

/// Helper function to render resources in a tree-like indented format (used without --long).
fn render_resources_tree(
    resources: &[core::ZgResource],
    indent: &str,
) -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    for resource in resources {
        writeln!(output, "{}{}", indent, resource.name)?;
        if let Some(sub_resources) = &resource.resources {
            let sub_output = render_resources_tree(sub_resources, &format!("{}  ", indent))?;
            output.push_str(&sub_output);
        }
    }
    Ok(output)
}

#[rustfmt::skip]
#[allow(clippy::wildcard_in_or_patterns)]
/// Function to list methods of a specific resource.
fn list_methods(
    api: &core::ZgApi,
    resource_path: &str,
    args: &ListArgs,
) -> Result<String, Box<dyn Error>> {
    let resource = core::find_resource(&api.id, &api.resources, resource_path)
        .map_err(|e| format!("Error finding resource '{}': {}", resource_path, e))?;

    let mut methods = if let Some(ref method_name) = args.method {
        // When you specify a method, only show that method; return Err if not found.
        vec![resource
            .methods
            .iter()
            .find(|m| m.name == *method_name)
            .ok_or_else(|| {
                format!(
                    "Method '{}' not found for resource '{}'.",
                    method_name, resource_path
                )
            })?]
    } else {
        // When no method is specified, list all methods in the resource.
        resource.methods.iter().collect::<Vec<_>>()
    };

    // Sort the methods based on the specified field; default is by flat_path (`default_value = "path"`)
    let sort_field = args.sort.as_deref().unwrap_or("path");
    methods.sort_by(|a, b| {
        let sorted = match sort_field {
            "name" | "method_name" => a.name.cmp(&b.name),
            "http" | "http_method" => a.http_method.cmp(&b.http_method).then(a.flat_path.cmp(&b.flat_path)),
            "path" | "url" | _ => a.flat_path.cmp(&b.flat_path).then(a.http_method.cmp(&b.http_method)), // fallback
        };
        if args.reverse { sorted.reverse() } else { sorted }
    });

    let output = if args.long {
        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_CLEAN);
        table.set_titles(row![bu->"method_name", b->"http_method", b->"path"]);
        for method in methods {
            let row = if args.color {
                // Colorize based on the HTTP methods (POST: green, PUT/PATCH: blue, DELETE: red).
                match method.http_method.as_str() {
                    "POST" => row![Fg => method.name, method.http_method, method.flat_path],
                    "PUT" | "PATCH" => row![Fb => method.name, method.http_method, method.flat_path],
                    "DELETE" => row![Fr => method.name, method.http_method, method.flat_path],
                    _ => row![method.name, method.http_method, method.flat_path],
                }
            } else {
                row![method.name, method.http_method, method.flat_path]
            };
            table.add_row(row);
        }
        table.print_tty(true)?;
        String::new() // Return empty string since --long format is printed directly here
    } else {
        // Without --long option, return only the method names
        methods
            .iter()
            .fold(String::new(), |mut output, method| {
                let _ = writeln!(output, "{}", method.name);
                output
            })
    };

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_resources() -> Vec<core::ZgResource> {
        vec![core::ZgResource {
            name: "projects".to_string(),
            parent_path: None,
            path: Some("container.projects".to_string()),
            methods: vec![],
            resources: Some(vec![
                core::ZgResource {
                    name: "zones".to_string(),
                    parent_path: Some("container.projects".to_string()),
                    path: Some("container.projects.zones".to_string()),
                    methods: vec![],
                    resources: Some(vec![core::ZgResource {
                        name: "clusters".to_string(),
                        parent_path: Some("container.projects.zones".to_string()),
                        path: Some("container.projects.zones.clusters".to_string()),
                        ..core::ZgResource::testdata()
                    }]),
                },
                core::ZgResource {
                    name: "locations".to_string(),
                    parent_path: Some("container.projects".to_string()),
                    path: Some("container.projects.locations".to_string()),
                    methods: vec![],
                    resources: Some(vec![core::ZgResource {
                        name: "clusters".to_string(),
                        parent_path: Some("container.projects.locations".to_string()),
                        path: Some("container.projects.locations.clusters".to_string()),
                        ..core::ZgResource::testdata()
                    }]),
                },
            ]),
        }]
    }

    #[test]
    fn test_list_services() {
        let output = list_services(&ListArgs {
            ..Default::default()
        })
        .expect("list_services failed");

        let expected_services = vec!["compute", "storage", "container"]; // these services should be active in supported_apis.rs
        for service in expected_services {
            assert!(
                output.contains(service),
                "Expected service '{}' not found in output: {}",
                service,
                output
            );
        }
    }

    #[test]
    fn test_list_resources() {
        let api = core::ZgApi {
            resources: setup_resources(),
            ..core::ZgApi::testdata()
        };

        let output = list_resources(
            &api,
            &ListArgs {
                ..Default::default()
            },
        )
        .expect("list_resources failed");

        let expected = "projects\n  zones\n    clusters\n  locations\n    clusters\n";
        assert_eq!(output, expected)
    }

    #[test]
    fn test_add_resource_rows() {
        let mut table = initialize_resources_table();
        let resources = vec![core::ZgResource {
            name: "projects".to_string(),
            ..core::ZgResource::testdata()
        }];
        let args = ListArgs {
            long: true,
            ..Default::default()
        };

        add_resource_rows(&mut table, &resources, &args, &vec![]);

        assert_eq!(table.len(), 1);
        assert_eq!(
            table.get_row(0).unwrap().get_cell(0).unwrap().get_content(),
            "projects"
        );
    }

    #[test]
    #[rustfmt::skip]
    fn test_sort_resources_table() {
        let mut table = initialize_resources_table();
        table.add_row(row!["projects", "0", "compute.projects", "0", ""]);
        table.add_row(row!["zones", "1", "compute.projects.zones", "2", "get, list"]);
        table.add_row(row!["instances", "2", "compute.projects.zones.instances", "48", "get, list, stop, reset, start, ..." ]);

        let name_sorted_table =
            sort_resources_table(&table, "name", false).expect("sort_resources_table by name failed");

        assert_eq!(name_sorted_table.get_row(0).unwrap().get_cell(0).unwrap().get_content(), "instances");
        assert_eq!(name_sorted_table.get_row(2).unwrap().get_cell(0).unwrap().get_content(), "zones");

        let depth_reverse_sorted_table =
            sort_resources_table(&table, "depth", true).expect("sort_resources_table by depth failed");

        assert_eq!(depth_reverse_sorted_table.get_row(0).unwrap().get_cell(0).unwrap().get_content(), "instances");
        assert_eq!(depth_reverse_sorted_table.get_row(2).unwrap().get_cell(0).unwrap().get_content(), "projects");
    }

    #[test]
    fn test_list_methods_empty() {
        let top_resources = setup_resources();

        // 'projects' resource has no methods
        let output = list_methods(
            &core::ZgApi {
                id: "container:v1".to_string(),
                resources: top_resources,
                ..core::ZgApi::testdata()
            },
            "projects",
            &ListArgs {
                service: Some("container".to_string()),
                resource: Some("projects".to_string()),
                sort: Some("path".to_string()),
                ..Default::default()
            },
        )
        .expect("list_methods failed");

        assert!(
            output.contains(""),
            "Expected '' (blank) output, but got: {}",
            output
        );
    }
}
