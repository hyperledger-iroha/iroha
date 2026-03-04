//! Reference SoraNet relay daemon entrypoint and CLI argument parsing.
use std::path::PathBuf;

use clap::Parser;
use soranet_relay::{
    config::{ComplianceConfig, CongestionConfig, RelayConfig},
    constant_rate::ConstantRateProfileName,
    error::RelayError,
    runtime::RelayRuntime,
};
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "soranet-relay",
    version,
    about = "Reference SoraNet relay daemon"
)]
struct Args {
    /// Path to the relay configuration JSON file.
    #[arg(short, long)]
    config: PathBuf,
    /// Override log level (defaults to `info`).
    #[arg(long, default_value = "info")]
    log_level: String,
    /// Override maximum simultaneous circuits per client (config fallback).
    #[arg(long)]
    max_circuits_per_client: Option<u32>,
    /// Override handshake cooldown in milliseconds between attempts from the same client.
    #[arg(long)]
    handshake_cooldown_millis: Option<u64>,
    /// Override compliance pipeline spool directory. Enables compliance logging if set.
    #[arg(long)]
    compliance_pipeline_spool_dir: Option<PathBuf>,
    /// Override compliance log size rotation threshold in bytes.
    #[arg(long)]
    compliance_max_log_bytes: Option<u64>,
    /// Override number of compliance log backups to retain.
    #[arg(long)]
    compliance_max_backup_files: Option<u8>,
    /// Override the constant-rate preset (core/home) advertised to telemetry.
    #[arg(long, value_name = "PROFILE")]
    constant_rate_profile: Option<ConstantRateProfileName>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = try_main().await {
        eprintln!("soranet-relay error: {error}");
        std::process::exit(1);
    }
}

async fn try_main() -> Result<(), RelayError> {
    let args = Args::parse();
    init_tracing(&args.log_level)?;

    let mut config = RelayConfig::load(&args.config)?;
    if let Some(limit) = args.max_circuits_per_client {
        let congestion = config
            .congestion
            .get_or_insert_with(CongestionConfig::default);
        congestion.max_circuits_per_client = limit;
    }
    if let Some(cooldown) = args.handshake_cooldown_millis {
        let congestion = config
            .congestion
            .get_or_insert_with(CongestionConfig::default);
        congestion.handshake_cooldown_millis = cooldown;
    }
    if args.compliance_pipeline_spool_dir.is_some()
        || args.compliance_max_log_bytes.is_some()
        || args.compliance_max_backup_files.is_some()
    {
        let compliance = config
            .compliance
            .get_or_insert_with(ComplianceConfig::default);
        if let Some(dir) = args.compliance_pipeline_spool_dir {
            compliance.enable = true;
            compliance.pipeline_spool_dir = Some(dir);
        }
        if let Some(bytes) = args.compliance_max_log_bytes {
            compliance.max_log_bytes = bytes;
        }
        if let Some(backups) = args.compliance_max_backup_files {
            compliance.max_backup_files = backups;
        }
    }
    if let Some(profile) = args.constant_rate_profile {
        config.constant_rate_profile = profile;
    }

    let runtime = RelayRuntime::new(config)?;

    info!(
        mode = runtime.mode().as_label(),
        listen = runtime.listen(),
        "starting relay runtime"
    );

    let metrics = runtime.metrics();
    runtime.run().await?;

    let snapshot = metrics.snapshot();
    info!(
        success = snapshot.success,
        failure = snapshot.failure,
        "relay shutdown complete"
    );

    Ok(())
}

fn init_tracing(level: &str) -> Result<(), RelayError> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .or_else(|_| tracing_subscriber::EnvFilter::try_new(level))
        .map_err(|error| RelayError::Logging(error.to_string()))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    Ok(())
}
