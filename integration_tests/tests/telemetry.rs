#![allow(missing_docs)]

use std::collections::{HashMap, HashSet};

use expect_test::expect;
use iroha_data_model::{isi::Log, peer::Peer, Identifiable, Level};
use iroha_test_network::{NetworkBuilder, NetworkPeer};

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

#[test]
fn commit_time() -> eyre::Result<()> {
    let (network, rt) = NetworkBuilder::new()
        .with_peers(4)
        .with_default_pipeline_time()
        .start_blocking()?;

    // empty block; must have non-zero commit time
    rt.block_on(network.ensure_blocks_with(|x| x.total == 2))?;

    network
        .client()
        .submit_blocking(Log::new(Level::INFO, "mewo".to_owned()))?;

    for client in network.peers().iter().map(NetworkPeer::client) {
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
    let network = NetworkBuilder::new().start().await?;

    let metrics = reqwest::get(network.client().torii_url.join("/metrics").unwrap())
        .await?
        .text()
        .await?;
    println!("{metrics}");
    let metrics = MetricsReader::new(&metrics);

    let keys = metrics.keys().collect::<std::collections::BTreeSet<_>>();
    expect![[r#"
        {
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
        }
    "#]]
    .assert_debug_eq(&keys);

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
    let network = NetworkBuilder::new()
        .with_peers(4)
        .with_default_pipeline_time()
        .start()
        .await?;

    for peer in network.peers() {
        let others: HashSet<_> = network
            .peers()
            .iter()
            .map(NetworkPeer::peer)
            .filter(|x| x.id() != &peer.peer_id())
            .collect();

        let response: HashSet<Peer> = reqwest::get(peer.client().torii_url.join("/peers").unwrap())
            .await?
            .json()
            .await?;

        assert_eq!(response, others);
    }

    Ok(())
}
