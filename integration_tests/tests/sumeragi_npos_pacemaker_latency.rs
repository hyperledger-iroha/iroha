//! Validate that the pacemaker keeps `NPoS` pacing near 1s even with ~250ms link delays.

use std::time::{Duration, Instant};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::data_model::{
    Level,
    isi::{InstructionBox, Log, SetParameter},
    parameter::{BlockParameter, Parameter, SumeragiParameter, system::SumeragiNposParameters},
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use nonzero_ext::nonzero;
use tokio::time::sleep;

const BLOCK_TIME_MS: u64 = 1_000;
const BLOCK_SYNC_GOSSIP_PERIOD_MS: u64 = 250;
const SAMPLE_BLOCKS: u64 = 8;
const BLOCK_SPACING_BUDGET_MS: f64 = 1_500.0;
const COLLECTORS_K: u16 = 3;
const REDUNDANT_SEND_R: u8 = 2;
const TIMEOUT_PROPOSE_MS: u64 = 350;
const TIMEOUT_PREVOTE_MS: u64 = 450;
const TIMEOUT_PRECOMMIT_MS: u64 = 550;
const TIMEOUT_COMMIT_MS: u64 = 750;
const TIMEOUT_DA_MS: u64 = 650;
const TIMEOUT_AGG_MS: u64 = 120;
const PROPOSE_EMA_BUDGET_MS: f64 = 1_100.0;
const PREVOTE_EMA_BUDGET_MS: f64 = 1_200.0;
const PRECOMMIT_EMA_BUDGET_MS: f64 = 1_300.0;
const COMMIT_EMA_BUDGET_MS: f64 = 1_400.0;
const METRIC_POLL_INTERVAL: Duration = Duration::from_millis(200);
const METRIC_POLL_TIMEOUT: Duration = Duration::from_secs(20);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::cast_precision_loss)]
async fn npos_pacemaker_targets_one_second_under_250ms_links() -> Result<()> {
    init_instruction_registry();

    let npos_params = SumeragiNposParameters {
        block_time_ms: BLOCK_TIME_MS,
        timeout_propose_ms: TIMEOUT_PROPOSE_MS,
        timeout_prevote_ms: TIMEOUT_PREVOTE_MS,
        timeout_precommit_ms: TIMEOUT_PRECOMMIT_MS,
        timeout_commit_ms: TIMEOUT_COMMIT_MS,
        timeout_da_ms: TIMEOUT_DA_MS,
        timeout_aggregator_ms: TIMEOUT_AGG_MS,
        k_aggregators: COLLECTORS_K,
        redundant_send_r: REDUNDANT_SEND_R,
        ..SumeragiNposParameters::default()
    };

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_base_seed("npos-pacemaker-rtt250ms")
        .with_auto_populated_trusted_peers()
        .with_block_sync_gossip_period(Duration::from_millis(BLOCK_SYNC_GOSSIP_PERIOD_MS))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors_k"], i64::from(COLLECTORS_K))
                .write(
                    ["sumeragi", "collectors_redundant_send_r"],
                    i64::from(REDUNDANT_SEND_R),
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(COLLECTORS_K),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(REDUNDANT_SEND_R),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos_params.into_custom_parameter(),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_pacemaker_targets_one_second_under_250ms_links),
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
            )
            .wrap_err_with(|| format!("submit pacemaker latency tick {idx}"))?;
    }
    network.ensure_blocks(target_height).await?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;
    let produced = target_height - start_status.blocks;
    let avg_spacing_ms = elapsed_ms / produced as f64;
    ensure!(
        avg_spacing_ms <= BLOCK_SPACING_BUDGET_MS,
        "average block spacing {avg_spacing_ms:.1} ms exceeded budget {BLOCK_SPACING_BUDGET_MS} ms over {produced} blocks"
    );

    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;
    let http = reqwest::Client::new();
    let metrics = poll_metrics(&http, &metrics_url).await?;

    let view_target = metrics.get("sumeragi_pacemaker_view_timeout_target_ms");
    ensure!(
        (view_target - BLOCK_TIME_MS as f64).abs() <= 1.0,
        "pacemaker view timeout target {view_target} ms did not match block_time_ms {BLOCK_TIME_MS}"
    );

    for (phase, budget) in [
        ("propose", PROPOSE_EMA_BUDGET_MS),
        ("prevote", PREVOTE_EMA_BUDGET_MS),
        ("precommit", PRECOMMIT_EMA_BUDGET_MS),
        ("commit", COMMIT_EMA_BUDGET_MS),
    ] {
        let key = format!("sumeragi_phase_latency_ema_ms{{phase=\"{phase}\"}}");
        let value = metrics.get(&key);
        ensure!(
            value <= budget,
            "{phase} EMA {value} ms exceeded budget {budget} ms (target 1s, 250ms RTT envelope)"
        );
    }

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
