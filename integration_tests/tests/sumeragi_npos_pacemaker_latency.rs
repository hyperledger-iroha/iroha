#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Validate that Sumeragi v2 stays within its round envelope under ~250ms link delays.
use std::time::{Duration, Instant};
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::data_model::{
    Level,
    isi::{InstructionBox, Log, SetParameter},
    parameter::{BlockParameter, Parameter, system::SumeragiNposParameters},
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use nonzero_ext::nonzero;
use tokio::time::sleep;
const BLOCK_TIME_MS: u64 = 1_000;
const COMMIT_TIME_MS: u64 = 2_000;
const BLOCK_SYNC_GOSSIP_PERIOD_MS: u64 = 250;
const SAMPLE_BLOCKS: u64 = 8;
// DA-enabled consensus waits for RBC/availability QC before changing view:
// commit quorum timeout = block_time + 3 * commit_time.
const COMMIT_QUORUM_TIMEOUT_MS: u64 = BLOCK_TIME_MS + 3 * COMMIT_TIME_MS;
const BLOCK_SPACING_BUDGET_MS: f64 = COMMIT_QUORUM_TIMEOUT_MS as f64 * 2.5;
const PHASE_EMA_BUDGET_MS: f64 = COMMIT_QUORUM_TIMEOUT_MS as f64;
const METRIC_POLL_INTERVAL: Duration = Duration::from_millis(200);
const METRIC_POLL_TIMEOUT: Duration = Duration::from_secs(20);
const LATENCY_SYNC_TIMEOUT: Duration = Duration::from_secs(600);
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::cast_precision_loss)]
async fn npos_v2_stays_within_round_envelope_under_250ms_links() -> Result<()> {
    init_instruction_registry();
    let npos_params = SumeragiNposParameters::default();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_base_seed("npos-pacemaker-rtt250ms")
        .with_auto_populated_trusted_peers()
        .with_sync_timeout(LATENCY_SYNC_TIMEOUT)
        .with_block_sync_gossip_period(Duration::from_millis(BLOCK_SYNC_GOSSIP_PERIOD_MS))
        .with_block_cadence(Duration::from_millis(BLOCK_TIME_MS))
        .with_npos_consensus()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos_params.into_custom_parameter(),
        )));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_v2_stays_within_round_envelope_under_250ms_links),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    let start_status = client.get_status()?;
    let target_height = start_status.blocks + SAMPLE_BLOCKS;
    let start = Instant::now();
    for idx in 0..SAMPLE_BLOCKS {
        client
            .submit::<InstructionBox>(
                Log::new(Level::INFO, format!("pacemaker latency tick {idx}")).into(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .wrap_err_with(|| format!("submit pacemaker latency tick {idx}"))?;
        network
            .ensure_blocks(start_status.blocks + idx + 1)
            .await
            .wrap_err_with(|| format!("wait for pacemaker latency tick {idx}"))?;
    }
    network.ensure_blocks(target_height).await?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;
    let produced = target_height - start_status.blocks;
    let avg_spacing_ms = elapsed_ms / produced as f64;
    ensure!(
        avg_spacing_ms <= BLOCK_SPACING_BUDGET_MS,
        "average block spacing {avg_spacing_ms:.1} ms exceeded budget {BLOCK_SPACING_BUDGET_MS} ms (commit quorum timeout {COMMIT_QUORUM_TIMEOUT_MS} ms) over {produced} blocks"
    );
    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;
    let http = integration_tests::http::client();
    let metrics = poll_metrics(&http, &metrics_url).await?;
    let mut phase_metrics_seen = 0_u32;
    for phase in ["propose", "collect_prevote", "collect_precommit", "commit"] {
        let key = format!("sumeragi_phase_latency_ema_ms{{phase=\"{phase}\"}}");
        let value = metrics
            .get_optional(&key)
            .or_else(|| {
                metrics.max_with_prefix_and_label("sumeragi_phase_latency_ema_ms{", "phase", phase)
            })
            .map(|value| {
                phase_metrics_seen = phase_metrics_seen.saturating_add(1);
                value
            });
        if let Some(value) = value {
            ensure!(
                value <= PHASE_EMA_BUDGET_MS,
                "{phase} EMA {value} ms exceeded budget {PHASE_EMA_BUDGET_MS} ms (commit quorum timeout {COMMIT_QUORUM_TIMEOUT_MS} ms)"
            );
        }
    }
    ensure!(
        phase_metrics_seen > 0,
        "phase EMA metrics were not reported for any phase"
    );
    network.shutdown().await;
    Ok(())
}
#[allow(unused_assignments)]
async fn poll_metrics(http: &reqwest::Client, url: &reqwest::Url) -> Result<MetricsReader> {
    let deadline = Instant::now() + METRIC_POLL_TIMEOUT;
    let mut last_error: eyre::Report = eyre!("metrics endpoint did not respond");
    loop {
        match http.get(url.clone()).send().await {
            Ok(response) if response.status().is_success() => match response.text().await {
                Ok(body) => return Ok(MetricsReader::new(&body)),
                Err(err) => last_error = err.into(),
            },
            Ok(response) => {
                let status = response.status();
                last_error = eyre!("metrics endpoint returned status {status}");
            }
            Err(err) => last_error = err.into(),
        }
        if Instant::now() >= deadline {
            return Err(last_error).wrap_err("metrics unavailable within timeout");
        }
        sleep(METRIC_POLL_INTERVAL).await;
    }
}
