//! Packaged SoraFS Governance DAG service launcher.
//!
//! This generic binary intentionally has no secret provider implementation.
//! Production supervisors must link `sorafs_node` and call
//! `run_governance_dag_service` with deployment-owned runtime providers.

use std::{path::PathBuf, process};

use clap::Parser;
use sorafs_node::{GovernanceDagServiceRuntimeProviders, run_governance_dag_service};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Always-on SoraFS Governance DAG publisher and mirror"
)]
struct Args {
    /// Iroha TOML containing `[sorafs.storage]` Governance DAG service fields.
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
    /// Reconcile exactly once without starting the query listener.
    #[arg(long)]
    once: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    // No file/env fallback exists for production credentials or checkpoint
    // custody. This packaged path is useful for policy validation and fails
    // honestly until a deployment-specific launcher injects its providers.
    if let Err(error) = run_governance_dag_service(
        &args.config,
        args.once,
        GovernanceDagServiceRuntimeProviders::default(),
    )
    .await
    {
        eprintln!("sorafs governance DAG service failed: {error}");
        process::exit(1);
    }
}
