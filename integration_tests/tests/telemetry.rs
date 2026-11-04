//! Telemetry and metrics surface integration tests.
use eyre::ensure;
use iroha_data_model::{
    Identifiable as _, Level,
    isi::{BurnBox, InstructionBox, Log, MintBox, RegisterBox, TransferBox},
    transaction::Executable,
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::peer::Peer;
use iroha_test_network::NetworkBuilder;
use std::collections::{BTreeSet, HashMap, HashSet};
use tokio::runtime::Runtime;
const TELEMETRY_REQUIRED_KEYS: &[&str] = &[
    "accounts{domain=\"garden_of_live_flowers\"}",
    "accounts{domain=\"genesis\"}",
    "accounts{domain=\"wonderland\"}",
    "block_height",
    "block_height_non_empty",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"10\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"25\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"50\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"100\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"250\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"500\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"750\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"1000\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"1500\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"2000\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"3000\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"5000\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"10000\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"30000\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"60000\"}",
    "sumeragi_commit_latency_ms_bucket{lane=\"global\",le=\"+Inf\"}",
    "sumeragi_commit_latency_ms_count{lane=\"global\"}",
    "sumeragi_commit_latency_ms_sum{lane=\"global\"}",
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
fn expected_genesis_metrics<'a>(
    instructions: impl IntoIterator<Item = &'a InstructionBox>,
) -> (Vec<f64>, BTreeSet<DomainId>) {
    // Native genesis initializes its authority domain before running the signed instructions.
    let mut domains = BTreeSet::from([iroha_genesis::GENESIS_DOMAIN_ID.clone()]);
    let mut amounts = Vec::new();
    for instruction in instructions {
        if let Some(RegisterBox::Domain(register)) =
            instruction.as_any().downcast_ref::<RegisterBox>()
        {
            domains.insert(register.object().id().clone());
        }
        let quantity = if let Some(MintBox::Asset(mint)) =
            instruction.as_any().downcast_ref::<MintBox>()
        {
            Some(mint.object())
        } else if let Some(BurnBox::Asset(burn)) = instruction.as_any().downcast_ref::<BurnBox>() {
            Some(burn.object())
        } else if let Some(TransferBox::Asset(transfer)) =
            instruction.as_any().downcast_ref::<TransferBox>()
        {
            Some(transfer.object())
        } else {
            None
        };
        if let Some(quantity) = quantity {
            amounts.push(quantity.as_numeric().clone().to_f64_lossy());
        }
    }
    (amounts, domains)
}

#[test]
#[allow(clippy::float_cmp)]
fn genesis_metric_expectations_include_registered_domains_and_numeric_movements() {
    use iroha_data_model::prelude::*;
    let domain = DomainId::try_new("telemetry", "universal").unwrap();
    let definition =
        AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse().unwrap());
    let asset = AssetId::new(definition, iroha_test_samples::ALICE_ID.clone());
    let instructions: Vec<InstructionBox> = vec![
        Register::domain(Domain::new(domain.clone())).into(),
        Mint::asset_quantity(13u32, asset.clone()).into(),
        Burn::asset_quantity(2u32, asset.clone()).into(),
        Transfer::asset_quantity(asset, 4u32, iroha_test_samples::BOB_ID.clone()).into(),
    ];
    let (amounts, domains) = expected_genesis_metrics(&instructions);
    assert_eq!(amounts, vec![13.0, 2.0, 4.0]);
    assert_eq!(
        domains,
        BTreeSet::from([iroha_genesis::GENESIS_DOMAIN_ID.clone(), domain])
    );
}

fn builder_with_full_telemetry() -> NetworkBuilder {
    NetworkBuilder::new().with_config_layer(|layer| {
        layer.write("telemetry_profile", "full");
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
#[allow(clippy::float_cmp)]
#[tokio::test]
async fn telemetry_permissioned_smoke() -> eyre::Result<()> {
    let builder = builder_with_full_telemetry()
        .with_peers(4)
        .with_permissioned_consensus()
        .with_block_cadence(std::time::Duration::from_secs(2));
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(telemetry_permissioned_smoke))
            .await?
    else {
        return Ok(());
    };
    // misc_measurements (genesis metrics) before submitting extra transactions
    let metrics = reqwest::get(
        network
            .client()
            .client()
            .endpoint()
            .join("/metrics")
            .unwrap(),
    )
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
    let genesis = network.genesis();
    let (amounts, domains) =
        expected_genesis_metrics(genesis.0.external_transactions().flat_map(|transaction| {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                panic!("the telemetry genesis fixture must contain native instructions");
            };
            instructions.iter()
        }));
    assert_eq!(metrics.get("tx_amount_sum"), amounts.iter().sum::<f64>());
    assert_eq!(
        metrics.get("tx_amount_count"),
        f64::from(u32::try_from(amounts.len())?)
    );
    for upper_bound in [0.0, 1_000.0] {
        assert_eq!(
            metrics.get(format!("tx_amount_bucket{{le=\"{upper_bound:.0}\"}}")),
            f64::from(u32::try_from(
                amounts
                    .iter()
                    .filter(|amount| **amount <= upper_bound)
                    .count()
            )?),
        );
    }
    assert_eq!(
        metrics.get("domains"),
        f64::from(u32::try_from(domains.len())?)
    );
    for domain in domains {
        assert_eq!(
            metrics.get(format!("accounts{{domain=\"{}\"}}", domain.name())),
            0.0
        );
    }
    // fetch_online_peers
    for peer in network.peers() {
        let others: HashSet<_> = network
            .peers()
            .iter()
            .filter(|x| x.id() != peer.id())
            .map(|x| Peer::new(x.p2p_address(), x.id()))
            .collect();
        let peer_client = peer.client();
        let operator_key_pair = peer_client
            .client()
            .operator_key_pair()
            .expect("test-network clients carry the peer operator key");
        let peers_uri: iroha_torii::Uri = "/v1/peers".parse().expect("static peers URI");
        let operator_headers = iroha_torii::operator_signed_request_headers(
            operator_key_pair,
            peer_client.client().network_id(),
            &iroha_torii::Method::GET,
            &peers_uri,
            &[],
        )?;
        let http_client = reqwest::Client::builder()
            .timeout(integration_tests::http::request_timeout())
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = http_client
            .get(peer_client.client().endpoint().join("/v1/peers").unwrap())
            .headers(operator_headers)
            .header("Accept", "application/json")
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "signed /v1/peers returned {}",
            response.status()
        );
        let response_body = response.text().await?;
        let response: HashSet<Peer> = norito::json::from_str(&response_body)
            .map_err(|err| eyre::Report::msg(format!("decode peers response: {err}")))?;
        assert_eq!(response, others);
    }
    // commit_time
    let client = network.client();
    integration_tests::sync::spawn_blocking(move || {
        client.submit(
            Log::new(Level::INFO, "mewo".to_owned()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await??;
    network.ensure_blocks_with(|x| x.non_empty >= 2).await?;
    for peer in network.peers() {
        let status = peer.status().await?;
        assert!(
            status.commit_time_ms > 0,
            "No peer can commit block immediately, even the leader one"
        );
        let metrics_url = reqwest::Url::parse(&peer.torii_url())?.join("metrics")?;
        let body = integration_tests::http::client()
            .get(metrics_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        assert!(
            MetricsReader::new(&body).get("sumeragi_commit_latency_ms_count{lane=\"global\"}")
                >= 1.0,
            "each validator must observe the real post-genesis commit in its canonical histogram"
        );
    }
    Ok(())
}
#[test]
fn status_reports_npos_mode_tag_on_start() -> eyre::Result<()> {
    let builder = builder_with_full_telemetry()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus();
    let Some((network, _rt)) =
        start_network(builder, stringify!(status_reports_npos_mode_tag_on_start))?
    else {
        return Ok(());
    };
    let status = network.client().status().get()?;
    let sumeragi = status
        .sumeragi
        .expect("sumeragi status must be present when telemetry is enabled");
    ensure!(
        sumeragi.mode_tag == "npos",
        "expected /status mode_tag to be npos, got {}",
        sumeragi.mode_tag
    );
    Ok(())
}
