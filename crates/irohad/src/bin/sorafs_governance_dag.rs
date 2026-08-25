//! Stock broker-backed `SoraFS` Governance DAG service launcher.
//!
//! The standalone config loader admits only the service's public endpoint policy, expected
//! identities, stable provider handles, revisions, bounds, and policy digests. Runtime credentials
//! and private keys remain behind the platform-fixed local provider broker.
use clap::Parser;
use iroha_data_model::{ChainId, NetworkId};
use irohad::StockGovernanceDagServiceRuntimeProviderRegistryV1;
use sorafs_node::{
    GovernanceDagServiceRuntimeProviderRegistryV1, run_governance_dag_service_with_runtime_registry,
};
use std::{path::PathBuf, process, sync::Arc};
#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Always-on SoraFS Governance DAG publisher and mirror"
)]
struct Args {
    /// Self-contained Iroha TOML containing the Governance DAG service fields.
    ///
    /// The standalone launcher deliberately rejects unresolved `extends`.
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
    /// Canonical public chain identity used by the exact broker handshake.
    #[arg(long, value_name = "CHAIN_ID")]
    chain_id: ChainId,
    /// Exact genesis-header-derived identity used by the broker handshake.
    #[arg(long, value_name = "NETWORK_ID")]
    network_id: NetworkId,
    /// Reconcile exactly once without starting the query listener.
    #[arg(long)]
    once: bool,
}
#[tokio::main]
async fn main() {
    let Args {
        config,
        chain_id,
        network_id,
        once,
    } = Args::parse();
    let runtime_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> = Arc::new(
        StockGovernanceDagServiceRuntimeProviderRegistryV1::new(chain_id, network_id),
    );
    if let Err(error) = Box::pin(run_governance_dag_service_with_runtime_registry(
        config,
        once,
        Some(runtime_registry),
    ))
    .await
    {
        eprintln!("sorafs governance DAG service failed: {error}");
        process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_requires_a_canonical_chain_identity() {
        let args = Args::try_parse_from([
            "sorafs_governance_dag",
            "--config",
            "governance.toml",
            "--chain-id",
            "sora.production",
            "--network-id",
            "a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5",
            "--once",
        ])
        .expect("parse canonical launcher arguments");
        assert_eq!(args.chain_id, ChainId::from("sora.production"));
        assert_eq!(
            args.network_id.to_string(),
            "a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5"
        );
        assert!(args.once);
        assert!(
            Args::try_parse_from([
                "sorafs_governance_dag",
                "--config",
                "governance.toml",
                "--chain-id",
                "not canonical",
                "--network-id",
                "a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5",
            ])
            .is_err()
        );
        assert!(
            Args::try_parse_from([
                "sorafs_governance_dag",
                "--config",
                "governance.toml",
                "--chain-id",
                "sora.production",
            ])
            .is_err()
        );
    }
}
