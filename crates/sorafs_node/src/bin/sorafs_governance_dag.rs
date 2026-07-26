//! Packaged SoraFS Governance DAG service launcher.
//!
//! Deployment packages link `sorafs_node` and call the registry-aware launcher
//! with their runtime-only provider registry. This generic package contains no
//! credential or private-key loader and fails with a typed error when no
//! supported registry was injected.

use std::{path::PathBuf, process};

use clap::Parser;
use sorafs_node::run_governance_dag_service_with_runtime_registry;

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
    if let Err(error) =
        run_governance_dag_service_with_runtime_registry(&args.config, args.once, None).await
    {
        eprintln!("sorafs governance DAG service failed: {error}");
        process::exit(1);
    }
}
