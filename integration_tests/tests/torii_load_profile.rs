#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Ignored Torii HTTP hot-path load profile for focused performance validation.

use std::time::{Duration, Instant};

use eyre::{WrapErr, ensure};
use integration_tests::sandbox;
use iroha_data_model::{
    Level, isi::Log, query::executor::prelude::FindParameters, transaction::TransactionBuilder,
};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

const HTTP_QUERY_SAMPLES: usize = 64;
const HTTP_TX_SAMPLES: usize = 32;

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

fn percentile_permille(sorted_samples: &[Duration], permille: usize) -> Duration {
    assert!(!sorted_samples.is_empty());
    let index = (sorted_samples.len() - 1)
        .saturating_mul(permille)
        .saturating_add(500)
        / 1_000;
    sorted_samples[index.min(sorted_samples.len() - 1)]
}

fn print_http_profile(label: &str, mut samples: Vec<Duration>) {
    samples.sort_unstable();
    let total: f64 = samples.iter().map(|sample| sample.as_secs_f64()).sum();
    let sample_count = u32::try_from(samples.len()).expect("HTTP sample count fits in u32");
    let avg_us = total * 1_000_000.0 / f64::from(sample_count);
    eprintln!(
        "torii_http_load_profile kind={label} samples={} avg_us={avg_us:.3} p50_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3}",
        samples.len(),
        micros(percentile_permille(&samples, 500)),
        micros(percentile_permille(&samples, 950)),
        micros(percentile_permille(&samples, 990)),
        micros(*samples.last().expect("non-empty samples")),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "HTTP load profile; run explicitly with --ignored --nocapture"]
async fn torii_http_hot_path_load_profile() -> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "operator")
                .write(["torii", "query_rate_per_authority_per_sec"], 10_000i64)
                .write(["torii", "query_burst_per_authority"], 10_000i64)
                .write(["torii", "tx_rate_per_authority_per_sec"], 10_000i64)
                .write(["torii", "tx_burst_per_authority"], 10_000i64)
                .write(["torii", "preauth_rate_per_ip_per_sec"], 20_000i64)
                .write(["torii", "preauth_burst_per_ip"], 20_000i64)
                .write(["queue", "capacity"], 4_096i64)
                .write(["queue", "capacity_per_user"], 4_096i64);
        });
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(torii_http_hot_path_load_profile))
            .await?
    else {
        return Ok(());
    };

    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;

    let clients = network
        .peers()
        .iter()
        .map(iroha_test_network::NetworkPeer::client)
        .collect::<Vec<_>>();
    ensure!(
        clients.len() == 4,
        "expected a 4-peer Torii load profile network"
    );

    let mut query_handles = Vec::with_capacity(HTTP_QUERY_SAMPLES);
    for index in 0..HTTP_QUERY_SAMPLES {
        let client = clients[index % clients.len()].clone();
        query_handles.push(tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let params = client.query_single(FindParameters)?;
            std::hint::black_box(params);
            Ok::<Duration, eyre::Report>(start.elapsed())
        }));
    }

    let mut query_samples = Vec::with_capacity(HTTP_QUERY_SAMPLES);
    for handle in query_handles {
        query_samples.push(
            handle
                .await
                .wrap_err("query worker panicked")?
                .wrap_err("query worker failed")?,
        );
    }
    print_http_profile("query_find_parameters", query_samples);

    let chain_id = network.chain_id();
    let mut tx_handles = Vec::with_capacity(HTTP_TX_SAMPLES);
    for index in 0..HTTP_TX_SAMPLES {
        let client = clients[index % clients.len()].clone();
        let chain_id = chain_id.clone();
        tx_handles.push(tokio::task::spawn_blocking(move || {
            let tx = TransactionBuilder::new(chain_id, ALICE_ID.clone())
                .with_instructions([Log::new(
                    Level::INFO,
                    format!("torii-http-load-profile-{index:04}"),
                )])
                .sign(ALICE_KEYPAIR.private_key());
            let start = Instant::now();
            let hash = client.submit_transaction(&tx)?;
            std::hint::black_box(hash);
            Ok::<Duration, eyre::Report>(start.elapsed())
        }));
    }

    let mut tx_samples = Vec::with_capacity(HTTP_TX_SAMPLES);
    for handle in tx_handles {
        tx_samples.push(
            handle
                .await
                .wrap_err("transaction worker panicked")?
                .wrap_err("transaction worker failed")?,
        );
    }
    print_http_profile("transaction_submit", tx_samples);

    Ok(())
}
