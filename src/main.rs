#[path = "../generated/rust/env.rs"]
mod env;
#[path = "../generated/rust/runtime.rs"]
mod env_runtime;

mod client;

use std::env;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::client::{ApiClient, ApiEndpoint};

#[derive(Debug, Parser)]
#[command(
    name = "act",
    version,
    about = "Operate the AntiCapTrad publishing platform"
)]
struct Cli {
    /// `AntiCapTrad` API origin. Remote endpoints must use HTTPS.
    #[arg(
        long,
        env = "ACT_API_URL",
        default_value = "http://127.0.0.1:8080",
        global = true
    )]
    api_url: String,

    /// Emit compact JSON instead of pretty-printed JSON.
    #[arg(long, global = true)]
    compact: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Query the service liveness endpoint.
    Health,
    /// Query dependency readiness.
    Ready,
    /// Read the authenticated principal from /api/me.
    Me,
    /// Perform a read-only GET on an API path.
    Get {
        /// Absolute API path, such as /api/providers.
        path: String,
        /// Attach `ACT_ACCESS_TOKEN` as a bearer token.
        #[arg(long)]
        authenticated: bool,
    },
    /// Print the validated, non-secret client configuration.
    Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    let env_values = env_runtime::load_from_os();
    let _ = &env_values;
    let cli = Cli::parse();
    let endpoint = ApiEndpoint::parse(&cli.api_url).context("invalid API configuration")?;

    if matches!(&cli.command, Command::Config) {
        let output = serde_json::json!({
            "api_origin": endpoint.display_origin(),
            "access_token_configured": access_token().is_ok(),
            "redirects": "rejected",
            "response_limit_bytes": 1_048_576
        });
        print_json(&output, cli.compact)?;
        return Ok(());
    }

    let client = ApiClient::new(endpoint)?;
    let (path, token) = match &cli.command {
        Command::Health => ("/health", None),
        Command::Ready => ("/ready", None),
        Command::Me => ("/api/me", Some(access_token()?)),
        Command::Get {
            path,
            authenticated,
        } => (
            path.as_str(),
            if *authenticated {
                Some(access_token()?)
            } else {
                None
            },
        ),
        Command::Config => unreachable!("configuration exits before client construction"),
    };

    let response = client.get(path, token.as_deref()).await?;
    let status = response.status_code();
    print_json(&response, cli.compact)?;
    if !status.is_success() {
        bail!("AntiCapTrad API returned HTTP {status}");
    }

    Ok(())
}

fn access_token() -> Result<String> {
    let token = env::var("ACT_ACCESS_TOKEN")
        .context("ACT_ACCESS_TOKEN is required for this authenticated command")?;
    if token.trim().is_empty() {
        bail!("ACT_ACCESS_TOKEN is empty");
    }
    Ok(token)
}

fn print_json(value: &impl serde::Serialize, compact: bool) -> Result<()> {
    if compact {
        println!("{}", serde_json::to_string(value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}
