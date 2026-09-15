#[path = "../generated/rust/env.rs"]
mod env;
#[path = "../generated/rust/runtime.rs"]
mod env_runtime;

mod client;

use std::env;
use std::io;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use ores_clis_core::{
    CliPolicy, ColorRole, EmitDisposition, EnvironmentHints, LogLevel, OutputMode,
    ProtocolEmitter, RuntimePolicy, StreamRole, TerminalState, paint, parse_shared_argv,
    top_level_io,
};

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
async fn main() -> ExitCode {
    let mut raw = std::env::args();
    let program = raw.next().unwrap_or_else(|| "act".to_owned());
    let terminals = TerminalState::detect();
    let environment = EnvironmentHints::detect();
    let shared = match parse_shared_argv(raw) {
        Ok(shared) => shared,
        Err(error) => {
            let runtime = CliPolicy::default().resolve(terminals, environment);
            emit_diagnostic(
                runtime,
                LogLevel::Error,
                ColorRole::Error,
                format!("act: {error}"),
            );
            return ExitCode::from(2);
        }
    };
    let runtime = shared.policy.resolve(terminals, environment);
    if shared.output_was_explicit() && matches!(shared.policy.output, OutputMode::Human) {
        emit_diagnostic(
            runtime,
            LogLevel::Error,
            ColorRole::Error,
            "act: human output is unsupported; command results are JSON",
        );
        return ExitCode::from(2);
    }
    let mut argv = Vec::with_capacity(shared.passthrough.len() + 1);
    argv.push(program);
    argv.extend(shared.passthrough);

    let cli = match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(error) => {
            let code = u8::try_from(error.exit_code()).unwrap_or(2);
            if error.use_stderr() {
                emit_diagnostic(runtime, LogLevel::Error, ColorRole::Error, error.to_string());
            } else {
                let stdout = io::stdout();
                let mut emitter = ProtocolEmitter::new(stdout.lock(), StreamRole::Primary);
                let _ = top_level_io(emitter.emit_primary_human_line(&error.to_string()));
            }
            return ExitCode::from(code);
        }
    };

    match execute(&cli, runtime).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            emit_diagnostic(runtime, LogLevel::Error, ColorRole::Error, format!("act: {error:#}"));
            ExitCode::from(1)
        }
    }
}

async fn execute(cli: &Cli, runtime: RuntimePolicy) -> Result<()> {
    let endpoint = ApiEndpoint::parse(&cli.api_url).context("invalid API configuration")?;
    let compact = cli.compact || runtime.json();

    if matches!(&cli.command, Command::Config) {
        let output = serde_json::json!({
            "api_origin": endpoint.display_origin(),
            "access_token_configured": access_token().is_ok(),
            "redirects": "rejected",
            "response_limit_bytes": 1_048_576
        });
        print_json(&output, compact)?;
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
    print_json(&response, compact)?;
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
    let stdout = io::stdout();
    let mut emitter = ProtocolEmitter::new(stdout.lock(), StreamRole::Primary);
    let write = if compact {
        emitter.emit_primary_machine_record(&serde_json::to_string(value)?)
    } else {
        // Pretty JSON is an established terminal-facing document surface. It
        // stays ANSI-free and is never used as the NDJSON/machine record form.
        emitter.emit_primary_human_line(&serde_json::to_string_pretty(value)?)
    };
    match top_level_io(write)? {
        EmitDisposition::Written | EmitDisposition::ConsumerClosed => Ok(()),
    }
}

fn emit_diagnostic(
    runtime: RuntimePolicy,
    level: LogLevel,
    role: ColorRole,
    message: impl std::fmt::Display,
) {
    if !runtime.allows_log(level) {
        return;
    }
    let stderr = io::stderr();
    let mut emitter = ProtocolEmitter::new(stderr.lock(), StreamRole::Diagnostics);
    let line = paint(runtime.color_stderr(), role, message);
    let _ = top_level_io(emitter.emit_diagnostic_line(&line));
}
