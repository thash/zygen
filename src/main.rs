use clap::{Parser, Subcommand};
use std::error::Error;

mod core;
mod desc;
mod discovery;
mod exec;
mod flavors;
mod list;
mod supported_apis;
mod update;

#[derive(Parser)]
#[command(name = "zg")]
#[command(version, about)]
struct Cli {
    /// Activate debug mode to see more detailed logs.
    #[arg(long, global = true)]
    debug: bool,

    /// Only Gemini API (generativelanguage) requires an API key. Other APIs ignore this value as they use gcloud to retrieve credentials
    #[arg(long, global = true)]
    api_key: Option<String>,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Update API definitions stored locally.
    Update(update::UpdateArgs),

    /// List services, resources, or methods (alias: ls).
    #[clap(aliases = &["ls"])]
    List(list::ListArgs),

    /// Describe details of services, resources, or methods (aliases: describe, show).
    ///
    /// Especially, describing methods is useful to understand the required (minimum) parameters/data to send via `zg exec`. Note that the shown minimum is merely a suggestion, you may need to tweak details.
    #[clap(aliases = &["describe", "show"])]
    Desc(desc::DescArgs),

    /// Execute an API method (aliases: ex, execute).
    #[clap(aliases = &["ex", "execute"])]
    Exec(exec::ExecArgs),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let level = if cli.debug { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    match &cli.command {
        Cmd::Update(args) => update::main(args).await,
        Cmd::List(args) => list::main(args, cli.api_key).await,
        Cmd::Desc(args) => desc::main(args, cli.api_key).await,
        Cmd::Exec(args) => exec::main(args, cli.api_key).await,
    }
    .map_err(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
