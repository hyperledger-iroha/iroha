use std::collections::{HashMap, HashSet};

use iroha_data_model::{isi::Log, peer::Peer, Identifiable, Level};
use iroha_test_network::NetworkBuilder;

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
}

#[test]
fn commit_time() -> eyre::Result<()> {
    let (network, rt) = NetworkBuilder::new()
        .with_peers(4)
        .with_default_pipeline_time()
        .start_blocking()?;

    // genesis commit time must be zero
    for client in network.peers().iter().map(|x| x.client()) {
        let status = client.get_status()?;
        assert_eq!(status.last_block_commit_time_ms, 0);
    }

    network
        .client()
        .submit_blocking(Log::new(Level::INFO, "mewo".to_owned()))?;
    rt.block_on(network.ensure_blocks(2))?;

    let mut met_producer = false;
    for client in network.peers().iter().map(|x| x.client()) {
        let status = client.get_status()?;

        if status.last_block_commit_time_ms == 0 {
            assert!(
                !met_producer,
                "only one peer can have zero propagation time - the leader one"
            );
            assert!(
                status.last_block_commit_time_ms > 0,
                "leader cannot immediately commit the block"
            );
            met_producer = true;
        } else {
            assert!(
                status.last_block_commit_time_ms > 0,
                "very unlikely the block could be propagated so fast"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn misc_measurements() -> eyre::Result<()> {
    let network = NetworkBuilder::new().start().await?;

    let metrics = reqwest::get(network.client().torii_url.join("/metrics").unwrap())
        .await?
        .text()
        .await?;
    println!("{metrics}");
    let metrics = MetricsReader::new(&metrics);

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
            .map(|x| x.peer())
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
