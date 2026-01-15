//! Telemetry and metrics surface integration tests.

use std::collections::{HashMap, HashSet};

use iroha_data_model::{Level, isi::Log, peer::Peer};
use iroha_test_network::NetworkBuilder;
use tokio::runtime::Runtime;

const TELEMETRY_REQUIRED_KEYS: &[&str] = &[
    "accounts{domain=\"garden_of_live_flowers\"}",
    "accounts{domain=\"genesis\"}",
    "accounts{domain=\"wonderland\"}",
    "block_height",
    "block_height_non_empty",
    "commit_time_ms_bucket{le=\"+Inf\"}",
    "commit_time_ms_bucket{le=\"100\"}",
    "commit_time_ms_bucket{le=\"1600\"}",
    "commit_time_ms_bucket{le=\"25600\"}",
    "commit_time_ms_bucket{le=\"400\"}",
    "commit_time_ms_bucket{le=\"6400\"}",
    "commit_time_ms_count",
    "commit_time_ms_sum",
    "connected_peers",
    "domains",
    "dropped_messages",
    "last_commit_time_ms",
    "queue_size",
    "tx_amount_bucket{le=\"+Inf\"}",
    "tx_amount_bucket{le=\"-10\"}",
    "tx_amount_bucket{le=\"-1000\"}",
    "tx_amount_bucket{le=\"-100000\"}",
    "tx_amount_bucket{le=\"-10000000\"}",
    "tx_amount_bucket{le=\"-1000000000\"}",
    "tx_amount_bucket{le=\"0\"}",
    "tx_amount_bucket{le=\"10\"}",
    "tx_amount_bucket{le=\"1000\"}",
    "tx_amount_bucket{le=\"100000\"}",
    "tx_amount_bucket{le=\"10000000\"}",
    "tx_amount_bucket{le=\"1000000000\"}",
    "tx_amount_count",
    "tx_amount_sum",
    "txs{type=\"accepted\"}",
    "txs{type=\"rejected\"}",
    "txs{type=\"total\"}",
    "uptime_since_genesis_ms",
    "view_changes",
];

fn builder_with_full_telemetry() -> NetworkBuilder {
    NetworkBuilder::new().with_config_layer(|layer| {
        layer
            .write("telemetry_enabled", true)
            .write("telemetry_profile", "full");
    })
}

struct MetricsReader {
    map: HashMap<String, f64>,
}

impl MetricsReader {
    fn new(raw: &str) -> Self {
        let map = raw
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| {
                let mut iter = line.split(' ');
                let key = iter.next().expect("key").to_owned();
                let value = iter.next().expect("value").parse().unwrap();
                assert!(iter.next().is_none());
                (key, value)
            })
            .collect();
        Self { map }
    }

    fn get(&self, key: impl AsRef<str>) -> f64 {
        let Some(value) = self.map.get(key.as_ref()) else {
            panic!("key \"{}\" does not exist", key.as_ref());
        };
        *value
    }

    fn keys(&self) -> impl Iterator<Item = &String> {
        self.map.keys()
    }
}

use integration_tests::sandbox;

fn start_network(
    builder: NetworkBuilder,
    context: &'static str,
) -> eyre::Result<Option<(sandbox::SerializedNetwork, Runtime)>> {
    sandbox::start_network_blocking_or_skip(builder, context)
}

#[test]
fn commit_time() -> eyre::Result<()> {
    let builder = builder_with_full_telemetry()
        .with_peers(4)
        .with_default_pipeline_time();
    let Some((network, rt)) = start_network(builder, stringify!(commit_time))? else {
        return Ok(());
    };

    network
        .client()
        .submit_blocking(Log::new(Level::INFO, "mewo".to_owned()))?;
    rt.block_on(network.ensure_blocks_with(|x| x.non_empty >= 2))?;

    for client in network
        .peers()
        .iter()
        .map(iroha_test_network::NetworkPeer::client)
    {
        let status = client.get_status()?;
        assert!(
            status.commit_time_ms > 0,
            "No peer can commit block immediately, even the leader one"
        );
    }

    Ok(())
}

#[allow(clippy::float_cmp)]
#[tokio::test]
async fn misc_measurements() -> eyre::Result<()> {
    let builder = builder_with_full_telemetry();
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(misc_measurements)).await?
    else {
        return Ok(());
    };

    let metrics = reqwest::get(network.client().torii_url.join("/metrics").unwrap())
        .await?
        .text()
        .await?;
    println!("{metrics}");
    let metrics = MetricsReader::new(&metrics);

    let keys = metrics
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    for key in TELEMETRY_REQUIRED_KEYS {
        assert!(
            keys.contains(key),
            "missing metric key {key}; available keys: {keys:?}"
        );
    }

    // genesis measurements
    assert_eq!(metrics.get("tx_amount_sum"), 57.0);
    assert_eq!(metrics.get("tx_amount_count"), 2.0);
    assert_eq!(metrics.get("tx_amount_bucket{le=\"0\"}"), 0.0);
    assert_eq!(metrics.get("tx_amount_bucket{le=\"1000\"}"), 2.0);
    assert_eq!(metrics.get("domains"), 3.0);
    assert_eq!(metrics.get("accounts{domain=\"genesis\"}"), 1.0);
    assert_eq!(metrics.get("accounts{domain=\"wonderland\"}"), 2.0);
    assert_eq!(
        metrics.get("accounts{domain=\"garden_of_live_flowers\"}"),
        1.0
    );

    Ok(())
}

#[tokio::test]
async fn fetch_online_peers() -> eyre::Result<()> {
    let builder = builder_with_full_telemetry()
        .with_peers(4)
        .with_default_pipeline_time();
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(fetch_online_peers)).await?
    else {
        return Ok(());
    };

    for peer in network.peers() {
        let others: HashSet<_> = network
            .peers()
            .iter()
            .filter(|x| x.id() != peer.id())
            .map(|x| Peer::new(x.p2p_address(), x.id()))
            .collect();

        let response_body = reqwest::get(peer.client().torii_url.join("/peers").unwrap())
            .await?
            .text()
            .await?;
        let response: HashSet<Peer> = norito::json::from_str(&response_body)
            .map_err(|err| eyre::Report::msg(format!("decode peers response: {err}")))?;

        assert_eq!(response, others);
    }

    Ok(())
}
