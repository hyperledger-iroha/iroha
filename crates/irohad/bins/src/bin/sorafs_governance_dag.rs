//! Stock broker-backed `SoraFS` Governance DAG service launcher.
//!
//! The standalone config loader admits only the service's public endpoint policy, expected
//! identities, stable provider handles, revisions, bounds, and policy digests. Runtime credentials
//! and private keys remain behind the configured authenticated local provider broker.
use clap::Parser;
use iroha_config::parameters::actual::RuntimeProviderBrokerEndpointPath;
use iroha_data_model::NetworkId;
use iroha_model_base::chain::ChainId;
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
    /// Public absolute path of this service's authenticated local broker socket.
    #[arg(long, value_name = "ABSOLUTE_SOCKET_PATH")]
    broker_endpoint: RuntimeProviderBrokerEndpointPath,
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
        broker_endpoint,
        once,
    } = Args::parse();
    let runtime_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> =
        Arc::new(StockGovernanceDagServiceRuntimeProviderRegistryV1::new(
            chain_id,
            network_id,
            broker_endpoint,
        ));
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
    fn cli_requires_canonical_chain_network_and_broker_endpoint() {
        let args = Args::try_parse_from([
            "sorafs_governance_dag",
            "--config",
            "governance.toml",
            "--chain-id",
            "sora.production",
            "--network-id",
            "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7",
            "--broker-endpoint",
            "/var/iroha/run/runtime-provider-broker-v1.sock",
            "--once",
        ])
        .expect("parse canonical launcher arguments");
        assert_eq!(args.chain_id, ChainId::from("sora.production"));
        assert_eq!(
            args.network_id.to_string(),
            "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7"
        );
        assert!(args.once);
        assert_eq!(
            args.broker_endpoint.as_path(),
            std::path::Path::new("/var/iroha/run/runtime-provider-broker-v1.sock")
        );
        assert!(
            Args::try_parse_from([
                "sorafs_governance_dag",
                "--config",
                "governance.toml",
                "--chain-id",
                "not canonical",
                "--broker-endpoint",
                "/var/iroha/run/runtime-provider-broker-v1.sock",
                "--network-id",
                "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7",
            ])
            .is_err()
        );
        let canonical_network = args.network_id.to_string();
        let required = [
            "sorafs_governance_dag",
            "--config",
            "governance.toml",
            "--chain-id",
            "sora.production",
            "--network-id",
            canonical_network.as_str(),
        ];
        assert!(
            Args::try_parse_from(required).is_err(),
            "the public broker endpoint is required"
        );
        assert!(
            Args::try_parse_from(
                required
                    .into_iter()
                    .chain(["--broker-endpoint", "../runtime-provider-broker-v1.sock",])
            )
            .is_err(),
            "a relative broker endpoint is rejected before provider resolution"
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
