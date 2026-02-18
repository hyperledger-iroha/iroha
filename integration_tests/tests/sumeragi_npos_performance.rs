#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Baseline performance harness for `NPoS` Sumeragi (1 s blocks, k=3 collectors).
//! The scenario captures telemetry snapshots while producing a fixed number of
//! blocks, aggregates phase latency EMAs, queue depths, and throughput, then
//! persists a JSON summary for reporting.

use std::{
    collections::BTreeMap,
    fs,
    time::{Duration, Instant},
};

use eyre::{Context as _, Result, bail, ensure, eyre};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::data_model::{
    Level,
    isi::{Log, SetParameter},
    parameter::{BlockParameter, Parameter, SumeragiParameter, system::SumeragiNposParameters},
    prelude::TransactionBuilder,
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use nonzero_ext::nonzero;
use norito::json::{self, JsonSerialize, Map, Value};
use tokio::time::sleep;

const BASE_SEED: &str = "npos-baseline-1s-k3";
const SCENARIO_NAME: &str = "npos_baseline_1s_k3";
const BLOCK_TIME_MS: u64 = 1_000;
const COLLECTORS_K: u16 = 3;
const REDUNDANT_SEND_R: u8 = 2;
const SAMPLE_BLOCKS: u64 = 12;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const SAMPLE_TIMEOUT: Duration = Duration::from_secs(90);
const COMMIT_EMA_MAX_MS: f64 = 2_500.0;
const PREVOTE_EMA_MAX_MS: f64 = 1_200.0;
const PRECOMMIT_EMA_MAX_MS: f64 = 2_500.0;
const PROPOSE_EMA_MAX_MS: f64 = 1_500.0;
const QUEUE_STRESS_SCENARIO_NAME: &str = "npos_queue_backpressure_stress";
const QUEUE_CAPACITY: i64 = 24;
const QUEUE_CAPACITY_PER_USER: i64 = 24;
const QUEUE_STRESS_TXS: usize = 128;
const QUEUE_SATURATION_POLL_INTERVAL: Duration = Duration::from_millis(250);
const QUEUE_SATURATION_TIMEOUT: Duration = Duration::from_secs(90);
const RBC_STORE_SCENARIO_NAME: &str = "npos_rbc_store_backpressure";
const RBC_STORE_PAYLOAD_BYTES: usize = 3 * 1024 * 1024;
const RBC_STORE_SOFT_SESSIONS: i64 = 1;
const RBC_STORE_MAX_SESSIONS: i64 = 4;
const RBC_STORE_SOFT_BYTES: i64 = 64 * 1024;
const RBC_STORE_MAX_BYTES: i64 = 128 * 1024;
const RBC_STORE_POLL_TIMEOUT: Duration = Duration::from_secs(60);
const RBC_STORE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const CHUNK_LOSS_SCENARIO_NAME: &str = "npos_rbc_chunk_loss_fault";
const CHUNK_LOSS_DROP_INTERVAL: i64 = 2;
const CHUNK_LOSS_POLL_TIMEOUT: Duration = Duration::from_secs(40);

const STRESS_PEERS_ENV: &str = "SUMERAGI_NPOS_STRESS_PEERS";
const STRESS_COLLECTORS_K_ENV: &str = "SUMERAGI_NPOS_STRESS_COLLECTORS_K";
const STRESS_REDUNDANT_SEND_R_ENV: &str = "SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R";

fn env_parse<T>(key: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

fn stress_peer_count(default: usize) -> usize {
    env_parse::<usize>(STRESS_PEERS_ENV).map_or(default, |peers| peers.max(4))
}

fn stress_collectors_k(default: u16, peers: usize) -> u16 {
    let max_allowed = if peers > 1 {
        u16::try_from(peers - 1).unwrap_or(u16::MAX)
    } else {
        1
    };
    let value = env_parse::<u16>(STRESS_COLLECTORS_K_ENV).unwrap_or(default);
    value.clamp(1, max_allowed)
}

fn stress_redundant_send_r(default: u8) -> u8 {
    env_parse::<u8>(STRESS_REDUNDANT_SEND_R_ENV).unwrap_or(default)
}

fn i64_to_f64(value: i64) -> f64 {
    const MAX_SAFE: i64 = 1 << f64::MANTISSA_DIGITS;
    const MIN_SAFE: i64 = -(1 << f64::MANTISSA_DIGITS);
    debug_assert!(
        (MIN_SAFE..MAX_SAFE).contains(&value),
        "pacemaker permille {value} exceeds f64 mantissa precision"
    );
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

const PACEMAKER_JITTER_SCENARIO_NAME: &str = "npos_pacemaker_jitter_within_band";
const PACEMAKER_JITTER_PERMILLE: i64 = 125;
const PACEMAKER_JITTER_POLL_INTERVAL: Duration = Duration::from_millis(300);
const PACEMAKER_JITTER_TIMEOUT: Duration = Duration::from_secs(60);
const PACEMAKER_JITTER_TOLERANCE_MS: f64 = 1.5;

fn json_value<T>(value: &T) -> Value
where
    T: JsonSerialize + ?Sized,
{
    json::to_value(value).expect("serialize helper")
}

#[derive(Debug, Clone)]
struct Stats {
    samples: usize,
    min: f64,
    max: f64,
    mean: f64,
    median: f64,
}

impl Stats {
    fn from_samples(samples: &[f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN values"));
        let len = sorted.len();
        let median = if len.is_multiple_of(2) {
            let lower = sorted[len / 2 - 1];
            let upper = sorted[len / 2];
            lower + (upper - lower) / 2.0
        } else {
            sorted[len / 2]
        };
        let sum: f64 = samples.iter().sum();
        #[allow(clippy::cast_precision_loss)]
        let mean = sum / len as f64;
        Some(Self {
            samples: len,
            min: sorted.first().copied().unwrap_or(0.0),
            max: sorted.last().copied().unwrap_or(0.0),
            mean,
            median,
        })
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("samples".into(), json_value(&(self.samples as u64)));
        map.insert("min".into(), json_value(&self.min));
        map.insert("max".into(), json_value(&self.max));
        map.insert("mean".into(), json_value(&self.mean));
        map.insert("median".into(), json_value(&self.median));
        Value::Object(map)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_baseline_1s_k3_captures_metrics() -> Result<()> {
    init_instruction_registry();

    let npos_params = SumeragiNposParameters {
        k_aggregators: COLLECTORS_K,
        redundant_send_r: REDUNDANT_SEND_R,
        ..SumeragiNposParameters::default()
    };

    let builder = NetworkBuilder::new()
        .with_peers(6)
        .with_base_seed(BASE_SEED)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], i64::from(COLLECTORS_K))
                .write(
                    ["sumeragi", "collectors", "redundant_send_r"],
                    i64::from(REDUNDANT_SEND_R),
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(COLLECTORS_K),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(REDUNDANT_SEND_R),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos_params.into_custom_parameter(),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_baseline_1s_k3_captures_metrics),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;

    let client = network.client();
    let status_before = client
        .get_status()
        .wrap_err("fetch initial status snapshot")?;
    let start_non_empty = status_before.blocks_non_empty;
    let target_non_empty = start_non_empty.saturating_add(SAMPLE_BLOCKS);
    let seed_start = start_non_empty.saturating_add(1);
    let submit_client = client.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        for offset in 0..SAMPLE_BLOCKS {
            let message = format!("npos baseline seed {}", seed_start + offset);
            submit_client.submit(Log::new(Level::INFO, message))?;
        }
        Ok(())
    })
    .await
    .wrap_err("submit baseline log transactions")??;

    let http = reqwest::Client::new();
    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;

    let phases: &[&str] = &[
        "propose",
        "collect_da",
        "collect_prevote",
        "collect_precommit",
        "collect_aggregator",
        "commit",
    ];
    let mut phase_samples: BTreeMap<&'static str, Vec<f64>> =
        phases.iter().map(|&phase| (phase, Vec::new())).collect();
    let mut queue_depth_samples = Vec::new();
    let mut queue_peer_max_samples = Vec::new();
    let mut view_change_installs = Vec::new();

    let start_instant = Instant::now();
    #[allow(unused_assignments)]
    let mut last_status = status_before;

    loop {
        if start_instant.elapsed() > SAMPLE_TIMEOUT {
            bail!(
                "timed out collecting baseline metrics after {:?}",
                SAMPLE_TIMEOUT
            );
        }

        let response = http
            .get(metrics_url.clone())
            .header("Accept", "text/plain")
            .send()
            .await
            .wrap_err("fetch metrics snapshot")?;
        ensure!(
            response.status().is_success(),
            "metrics endpoint returned status {}",
            response.status()
        );
        let metrics_body = response
            .text()
            .await
            .wrap_err("read metrics response body")?;
        let reader = MetricsReader::new(&metrics_body);

        for &phase in phases {
            let key = format!("sumeragi_phase_latency_ema_ms{{phase=\"{phase}\"}}");
            if let Some(value) = reader.get_optional(&key)
                && let Some(samples) = phase_samples.get_mut(phase)
            {
                samples.push(value);
            }
        }

        if let Some(collectors_k) = reader.get_optional("sumeragi_collectors_k") {
            ensure!(
                (collectors_k - f64::from(COLLECTORS_K)).abs() < f64::EPSILON,
                "collectors_k gauge mismatch: expected {}, observed {collectors_k}",
                COLLECTORS_K
            );
        }

        if let Some(redundant_send_r) = reader.get_optional("sumeragi_redundant_send_r") {
            ensure!(
                (redundant_send_r - f64::from(REDUNDANT_SEND_R)).abs() < f64::EPSILON,
                "redundant_send_r gauge mismatch: expected {}, observed {redundant_send_r}",
                REDUNDANT_SEND_R
            );
        }

        if let Some(depth) = reader.get_optional("sumeragi_bg_post_queue_depth") {
            queue_depth_samples.push(depth);
        }
        if let Some(peer_max) = reader.max_with_prefix("sumeragi_bg_post_queue_depth_by_peer") {
            queue_peer_max_samples.push(peer_max);
        }
        if let Some(installs) = reader.get_optional("sumeragi_view_change_install_total") {
            view_change_installs.push(installs);
        }

        last_status = client
            .get_status()
            .wrap_err("fetch status during sampling")?;
        if last_status.blocks_non_empty >= target_non_empty {
            break;
        }

        sleep(POLL_INTERVAL).await;
    }

    let elapsed = start_instant.elapsed();
    let blocks_sampled = last_status.blocks_non_empty.saturating_sub(start_non_empty);
    ensure!(
        blocks_sampled >= SAMPLE_BLOCKS,
        "expected to sample at least {SAMPLE_BLOCKS} blocks, observed {blocks_sampled}"
    );
    ensure!(
        elapsed.as_secs_f64() > 0.0,
        "elapsed duration for baseline sampling is zero"
    );
    #[allow(clippy::cast_precision_loss)]
    let blocks_sampled_f64 = blocks_sampled as f64;
    let throughput_blocks_per_sec = blocks_sampled_f64 / elapsed.as_secs_f64();
    let observed_block_time_ms = (elapsed.as_secs_f64() * 1_000.0) / blocks_sampled_f64;

    let phase_stats: BTreeMap<&'static str, Stats> = phase_samples
        .into_iter()
        .filter_map(|(phase, samples)| Stats::from_samples(&samples).map(|stats| (phase, stats)))
        .collect();

    let commit_stats = phase_stats
        .get("commit")
        .ok_or_else(|| eyre!("missing commit phase EMA samples"))?;
    ensure!(
        commit_stats.max <= COMMIT_EMA_MAX_MS,
        "commit EMA exceeded budget: {:.2} ms (budget {:.2} ms)",
        commit_stats.max,
        COMMIT_EMA_MAX_MS
    );

    let prevote_stats = phase_stats
        .get("collect_prevote")
        .ok_or_else(|| eyre!("missing collect_prevote EMA samples"))?;
    ensure!(
        prevote_stats.max <= PREVOTE_EMA_MAX_MS,
        "collect_prevote EMA exceeded budget: {:.2} ms (budget {:.2} ms)",
        prevote_stats.max,
        PREVOTE_EMA_MAX_MS
    );

    let precommit_stats = phase_stats
        .get("collect_precommit")
        .ok_or_else(|| eyre!("missing collect_precommit EMA samples"))?;
    ensure!(
        precommit_stats.max <= PRECOMMIT_EMA_MAX_MS,
        "collect_precommit EMA exceeded budget: {:.2} ms (budget {:.2} ms)",
        precommit_stats.max,
        PRECOMMIT_EMA_MAX_MS
    );

    let propose_stats = phase_stats
        .get("propose")
        .ok_or_else(|| eyre!("missing propose EMA samples"))?;
    ensure!(
        propose_stats.max <= PROPOSE_EMA_MAX_MS,
        "propose EMA exceeded budget: {:.2} ms (budget {:.2} ms)",
        propose_stats.max,
        PROPOSE_EMA_MAX_MS
    );

    let queue_depth_stats = Stats::from_samples(&queue_depth_samples);
    let queue_peer_max_stats = Stats::from_samples(&queue_peer_max_samples);
    let view_change_stats = Stats::from_samples(&view_change_installs);

    let mut phase_map = Map::new();
    for (phase, stats) in &phase_stats {
        phase_map.insert((*phase).into(), stats.to_value());
    }

    let mut queue_map = Map::new();
    if let Some(stats) = queue_depth_stats {
        queue_map.insert("bg_post_depth".into(), stats.to_value());
    }
    if let Some(stats) = queue_peer_max_stats {
        queue_map.insert("bg_post_peer_max".into(), stats.to_value());
    }

    let mut view_change_map = Map::new();
    view_change_map.insert(
        "install_total".into(),
        view_change_stats.map_or_else(
            || {
                let mut obj = Map::new();
                obj.insert("samples".into(), json_value(&0u64));
                Value::Object(obj)
            },
            |stats| stats.to_value(),
        ),
    );
    view_change_map.insert("status_view_changes".into(), {
        let mut obj = Map::new();
        obj.insert("value".into(), json_value(&last_status.view_changes));
        Value::Object(obj)
    });

    let mut summary_root = Map::new();
    summary_root.insert("scenario".into(), json_value(&SCENARIO_NAME));
    summary_root.insert("network".into(), {
        let mut obj = Map::new();
        obj.insert("seed".into(), json_value(&BASE_SEED));
        obj.insert("peers".into(), json_value(&(network.peers().len() as u64)));
        obj.insert("collectors_k".into(), json_value(&COLLECTORS_K));
        obj.insert("redundant_send_r".into(), json_value(&REDUNDANT_SEND_R));
        Value::Object(obj)
    });
    summary_root.insert("timing".into(), {
        let mut obj = Map::new();
        obj.insert("block_time_target_ms".into(), json_value(&BLOCK_TIME_MS));
        obj.insert("blocks_sampled".into(), json_value(&blocks_sampled));
        obj.insert(
            "elapsed_ms".into(),
            json_value(&(elapsed.as_secs_f64() * 1_000.0)),
        );
        obj.insert(
            "throughput_blocks_per_sec".into(),
            json_value(&throughput_blocks_per_sec),
        );
        obj.insert(
            "observed_block_time_ms".into(),
            json_value(&observed_block_time_ms),
        );
        Value::Object(obj)
    });
    summary_root.insert("phase_latency_ema_ms".into(), Value::Object(phase_map));
    summary_root.insert("queue".into(), Value::Object(queue_map));
    summary_root.insert("view_changes".into(), Value::Object(view_change_map));
    summary_root.insert(
        "telemetry_samples".into(),
        json_value(&(commit_stats.samples as u64)),
    );
    summary_root.insert("final_height".into(), {
        let mut obj = Map::new();
        obj.insert("total".into(), json_value(&last_status.blocks));
        obj.insert(
            "non_empty".into(),
            json_value(&last_status.blocks_non_empty),
        );
        Value::Object(obj)
    });
    summary_root.insert("environment".into(), {
        let mut obj = Map::new();
        obj.insert("os".into(), json_value(&std::env::consts::OS));
        obj.insert("arch".into(), json_value(&std::env::consts::ARCH));
        obj.insert("family".into(), json_value(&std::env::consts::FAMILY));
        Value::Object(obj)
    });

    let summary_value = Value::Object(summary_root);
    let summary_json =
        norito::json::to_string(&summary_value).wrap_err("serialize summary json")?;
    println!("sumeragi_baseline_summary::{SCENARIO_NAME}::{summary_json}");

    let summary_pretty =
        norito::json::to_string_pretty(&summary_value).wrap_err("serialize pretty summary json")?;
    persist_summary_if_requested(SCENARIO_NAME, &summary_pretty)?;

    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_queue_backpressure_triggers_metrics() -> Result<()> {
    init_instruction_registry();

    let peers = stress_peer_count(4);
    let collectors_k = stress_collectors_k(1, peers);
    let redundant_send_r = stress_redundant_send_r(2);

    let builder = NetworkBuilder::new()
        .with_peers(peers)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], i64::from(collectors_k))
                .write(
                    ["sumeragi", "collectors", "redundant_send_r"],
                    i64::from(redundant_send_r),
                )
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_sessions"],
                    RBC_STORE_MAX_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_sessions"],
                    RBC_STORE_SOFT_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_bytes"],
                    RBC_STORE_MAX_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_bytes"],
                    RBC_STORE_SOFT_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                    900_000i64,
                )
                .write(["queue", "capacity"], QUEUE_CAPACITY)
                .write(["queue", "capacity_per_user"], QUEUE_CAPACITY_PER_USER);
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(1_500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(1_500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(collectors_k),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(redundant_send_r),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_queue_backpressure_triggers_metrics),
    )
    .await?
    else {
        return Ok(());
    };

    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;

    let chain_id = network.chain_id();

    for idx in 0..2 {
        let payload = format!(
            "queue-rbc-primer-{idx:02}-{}",
            "S".repeat(RBC_STORE_PAYLOAD_BYTES.saturating_sub(18))
        );
        let tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
            .with_instructions([Log::new(Level::INFO, payload)])
            .sign(ALICE_KEYPAIR.private_key());
        let submit_client = network.client();
        tokio::task::spawn_blocking(move || submit_client.submit_transaction(&tx)).await??;
    }

    let mut handles = Vec::with_capacity(QUEUE_STRESS_TXS);
    for idx in 0..QUEUE_STRESS_TXS {
        let client = network.client();
        let tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
            .with_instructions([Log::new(Level::INFO, format!("queue-stress-{idx:04}"))])
            .sign(ALICE_KEYPAIR.private_key());
        handles.push(tokio::task::spawn_blocking(move || {
            client.submit_transaction(&tx)
        }));
    }

    let mut submit_ok = 0usize;
    let mut queue_backpressure_rejects = 0usize;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => submit_ok += 1,
            Ok(Err(err)) => {
                let text = err.to_string();
                if text.contains("PRTRY:QUEUE_FULL") || text.contains("PRTRY:QUEUE_RATE") {
                    queue_backpressure_rejects += 1;
                } else {
                    eprintln!("[npos_queue_backpressure_triggers_metrics] submit error: {err:?}");
                }
            }
            Err(join_err) => {
                eprintln!("[npos_queue_backpressure_triggers_metrics] join error: {join_err:?}");
            }
        }
    }
    ensure!(
        submit_ok > 0,
        "all stress transactions failed to submit; queue never exercised"
    );
    ensure!(
        queue_backpressure_rejects > 0,
        "stress submissions did not observe queue backpressure rejections"
    );

    let client = network.client();
    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;
    let http = reqwest::Client::new();

    let mut observed_saturation = queue_backpressure_rejects > 0;
    let mut observed_deferrals = 0.0;
    let mut observed_rbc_deferrals = 0.0;
    let mut max_queue_depth = 0.0;
    let mut queue_capacity = 0.0;

    let start = Instant::now();
    loop {
        if start.elapsed() > QUEUE_SATURATION_TIMEOUT {
            break;
        }

        let response = http
            .get(metrics_url.clone())
            .header("Accept", "text/plain")
            .send()
            .await
            .wrap_err("fetch metrics snapshot")?;
        ensure!(
            response.status().is_success(),
            "metrics endpoint returned status {}",
            response.status()
        );
        let body = response
            .text()
            .await
            .wrap_err("read metrics response body")?;
        let reader = MetricsReader::new(&body);

        if let Some(depth) = reader.get_optional("sumeragi_tx_queue_depth")
            && depth > max_queue_depth
        {
            max_queue_depth = depth;
        }
        if let Some(capacity) = reader.get_optional("sumeragi_tx_queue_capacity") {
            queue_capacity = capacity;
        }
        if let Some(saturated) = reader.get_optional("sumeragi_tx_queue_saturated")
            && saturated >= 1.0
        {
            observed_saturation = true;
        }
        if let Some(deferrals) =
            reader.get_optional("sumeragi_pacemaker_backpressure_deferrals_total")
            && deferrals > 0.0
        {
            observed_deferrals = deferrals;
        }
        if let Some(deferrals) = reader.get_optional("sumeragi_rbc_backpressure_deferrals_total")
            && deferrals > 0.0
        {
            observed_rbc_deferrals = deferrals;
        }

        if observed_saturation && observed_deferrals > 0.0 {
            break;
        }

        sleep(QUEUE_SATURATION_POLL_INTERVAL).await;
    }

    ensure!(
        observed_saturation,
        "queue saturation gauge never rose above zero"
    );
    ensure!(
        observed_deferrals > 0.0,
        "pacemaker backpressure deferral counter remained zero"
    );
    let mut summary_root = Map::new();
    summary_root.insert("scenario".into(), json_value(&QUEUE_STRESS_SCENARIO_NAME));
    summary_root.insert("submissions".into(), {
        let mut map = Map::new();
        map.insert("attempted".into(), json_value(&(QUEUE_STRESS_TXS as u64)));
        map.insert("succeeded".into(), json_value(&(submit_ok as u64)));
        map.insert(
            "queue_backpressure_rejects".into(),
            json_value(&(queue_backpressure_rejects as u64)),
        );
        Value::Object(map)
    });
    summary_root.insert("queue".into(), {
        let mut map = Map::new();
        map.insert("max_depth".into(), json_value(&max_queue_depth));
        map.insert("capacity_reported".into(), json_value(&queue_capacity));
        map.insert("saturated".into(), json_value(&observed_saturation));
        Value::Object(map)
    });
    summary_root.insert("backpressure".into(), {
        let mut map = Map::new();
        map.insert(
            "pacemaker_deferrals_total".into(),
            json_value(&observed_deferrals),
        );
        map.insert(
            "rbc_deferrals_total".into(),
            json_value(&observed_rbc_deferrals),
        );
        Value::Object(map)
    });

    let summary_value = Value::Object(summary_root);
    let summary_json =
        norito::json::to_string(&summary_value).wrap_err("serialize stress summary json")?;
    println!("sumeragi_baseline_summary::{QUEUE_STRESS_SCENARIO_NAME}::{summary_json}");

    let summary_pretty = norito::json::to_string_pretty(&summary_value)
        .wrap_err("serialize pretty stress summary json")?;
    persist_summary_if_requested(QUEUE_STRESS_SCENARIO_NAME, &summary_pretty)?;

    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_pacemaker_jitter_within_band() -> Result<()> {
    init_instruction_registry();

    let peers = stress_peer_count(4);
    let collectors_k = stress_collectors_k(2, peers);
    let redundant_send_r = stress_redundant_send_r(2);

    let builder = NetworkBuilder::new()
        .with_peers(peers)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(
                    ["sumeragi", "advanced", "pacemaker", "jitter_frac_permille"],
                    PACEMAKER_JITTER_PERMILLE,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "backoff_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    4_000_i64,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(1_500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(1_500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(collectors_k),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(redundant_send_r),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_pacemaker_jitter_within_band),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let status = client.get_status()?;
    for idx in status.blocks..2 {
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("pacemaker jitter seed {idx}"),
        ))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= 2)
        .await?;

    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;
    let http = reqwest::Client::new();

    let start = Instant::now();
    let mut observed_jitter = 0.0_f64;
    let mut observed_target = 0.0_f64;
    let mut observed_permille = 0.0_f64;
    let mut observed_max_backoff = 0.0_f64;
    let mut snapshots_checked = 0_u64;

    loop {
        ensure!(
            start.elapsed() <= PACEMAKER_JITTER_TIMEOUT,
            "timed out waiting for pacemaker jitter metrics"
        );
        let response = http
            .get(metrics_url.clone())
            .header("Accept", "text/plain")
            .send()
            .await
            .wrap_err("fetch metrics snapshot")?;
        ensure!(
            response.status().is_success(),
            "metrics endpoint returned status {}",
            response.status()
        );
        let snapshot = response.text().await.wrap_err("read metrics body")?;
        snapshots_checked = snapshots_checked.saturating_add(1);
        let reader = MetricsReader::new(&snapshot);

        if let Some(value) = reader.get_optional("sumeragi_pacemaker_jitter_ms") {
            observed_jitter = value;
        }
        if let Some(value) = reader.get_optional("sumeragi_pacemaker_view_timeout_target_ms") {
            observed_target = value;
        }
        if let Some(value) = reader.get_optional("sumeragi_pacemaker_jitter_frac_permille") {
            observed_permille = value;
        }
        if let Some(value) = reader.get_optional("sumeragi_pacemaker_max_backoff_ms") {
            observed_max_backoff = value;
        }

        if observed_jitter > 0.0 && observed_target > 0.0 && observed_max_backoff > 0.0 {
            break;
        }

        sleep(PACEMAKER_JITTER_POLL_INTERVAL).await;
    }

    ensure!(
        (observed_permille - i64_to_f64(PACEMAKER_JITTER_PERMILLE)).abs() < f64::EPSILON,
        "jitter permille gauge mismatch: expected {}, observed {observed_permille}",
        PACEMAKER_JITTER_PERMILLE
    );

    ensure!(
        observed_jitter > 0.0,
        "pacemaker jitter gauge never rose above zero"
    );
    ensure!(
        observed_target > 0.0,
        "view timeout target gauge never reported a positive value"
    );
    ensure!(
        observed_max_backoff > 0.0,
        "max backoff gauge never reported a positive value"
    );

    let expected_jitter = observed_max_backoff * i64_to_f64(PACEMAKER_JITTER_PERMILLE) / 1_000.0;
    let jitter_delta = (observed_jitter - expected_jitter).abs();
    ensure!(
        jitter_delta <= PACEMAKER_JITTER_TOLERANCE_MS,
        "jitter {observed_jitter:.2} ms deviated from expected {expected_jitter:.2} ms by {jitter_delta:.2} ms (max_backoff {observed_max_backoff:.2} ms, permille {observed_permille})"
    );

    let mut root = Map::new();
    root.insert(
        "scenario".into(),
        json_value(&PACEMAKER_JITTER_SCENARIO_NAME),
    );
    let mut metrics = Map::new();
    metrics.insert("jitter_ms".into(), json_value(&observed_jitter));
    metrics.insert(
        "view_timeout_target_ms".into(),
        json_value(&observed_target),
    );
    metrics.insert("max_backoff_ms".into(), json_value(&observed_max_backoff));
    metrics.insert(
        "jitter_permille".into(),
        json_value(&(PACEMAKER_JITTER_PERMILLE as u64)),
    );
    metrics.insert("expected_jitter_ms".into(), json_value(&expected_jitter));
    metrics.insert("jitter_delta_ms".into(), json_value(&jitter_delta));
    metrics.insert("snapshots_checked".into(), json_value(&snapshots_checked));
    root.insert("metrics".into(), Value::Object(metrics));
    let summary_value = Value::Object(root);

    let summary_json =
        norito::json::to_string(&summary_value).wrap_err("serialize pacemaker summary json")?;
    println!("sumeragi_baseline_summary::{PACEMAKER_JITTER_SCENARIO_NAME}::{summary_json}");

    let summary_pretty = norito::json::to_string_pretty(&summary_value)
        .wrap_err("serialize pretty pacemaker summary json")?;
    persist_summary_if_requested(PACEMAKER_JITTER_SCENARIO_NAME, &summary_pretty)?;

    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_rbc_store_backpressure_records_metrics() -> Result<()> {
    init_instruction_registry();

    let peers = stress_peer_count(4);
    let collectors_k = stress_collectors_k(2, peers);
    let redundant_send_r = stress_redundant_send_r(2);

    let builder = NetworkBuilder::new()
        .with_peers(peers)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], i64::from(collectors_k))
                .write(
                    ["sumeragi", "collectors", "redundant_send_r"],
                    i64::from(redundant_send_r),
                )
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_sessions"],
                    RBC_STORE_MAX_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_sessions"],
                    RBC_STORE_SOFT_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_bytes"],
                    RBC_STORE_MAX_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_bytes"],
                    RBC_STORE_SOFT_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                    900_000i64,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(collectors_k),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(redundant_send_r),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(1_500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(1_500),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_rbc_store_backpressure_records_metrics),
    )
    .await?
    else {
        return Ok(());
    };

    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;

    let chain_id = network.chain_id();

    for idx in 0..2 {
        let message = format!(
            "rbc-store-{idx:02}-{}",
            "S".repeat(RBC_STORE_PAYLOAD_BYTES.saturating_sub(13))
        );
        let tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
            .with_instructions([Log::new(Level::INFO, message)])
            .sign(ALICE_KEYPAIR.private_key());
        let submit_client = network.client();
        tokio::task::spawn_blocking(move || submit_client.submit_transaction(&tx)).await??;
    }

    let client = network.client();
    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;
    let http = reqwest::Client::new();

    let mut max_pressure: f64 = 0.0;
    let mut max_sessions: f64 = 0.0;
    let mut max_bytes: f64 = 0.0;
    let mut max_deferrals: f64 = 0.0;
    let mut last_snapshot: Option<String> = None;
    let start = Instant::now();
    let mut timed_out = false;

    loop {
        if start.elapsed() > RBC_STORE_POLL_TIMEOUT {
            timed_out = true;
            break;
        }

        let response = http
            .get(metrics_url.clone())
            .header("Accept", "text/plain")
            .send()
            .await
            .wrap_err("fetch metrics snapshot")?;
        ensure!(
            response.status().is_success(),
            "metrics endpoint returned status {}",
            response.status()
        );
        let snapshot = response.text().await.wrap_err("read metrics body")?;
        let reader = MetricsReader::new(&snapshot);

        if let Some(value) = reader.get_optional("sumeragi_rbc_store_pressure") {
            max_pressure = max_pressure.max(value);
        }
        if let Some(value) = reader.get_optional("sumeragi_rbc_store_sessions") {
            max_sessions = max_sessions.max(value);
        }
        if let Some(value) = reader.get_optional("sumeragi_rbc_store_bytes") {
            max_bytes = max_bytes.max(value);
        }
        if let Some(value) = reader.get_optional("sumeragi_rbc_backpressure_deferrals_total") {
            max_deferrals = max_deferrals.max(value);
        }

        if max_pressure > 0.0 || max_sessions > 0.0 || max_bytes > 0.0 || max_deferrals >= 1.0 {
            last_snapshot.get_or_insert(snapshot);
            break;
        }

        sleep(RBC_STORE_POLL_INTERVAL).await;
    }

    if timed_out && max_pressure <= 0.0 && max_sessions <= 0.0 && max_bytes <= 0.0 {
        eprintln!(
            "[npos_rbc_store_backpressure_records_metrics] RBC store pressure metrics remained zero before timeout"
        );
    }

    let mut root = Map::new();
    root.insert("scenario".into(), json_value(&RBC_STORE_SCENARIO_NAME));
    let mut metrics = Map::new();
    metrics.insert("max_pressure".into(), json_value(&max_pressure));
    metrics.insert("max_sessions".into(), json_value(&max_sessions));
    metrics.insert("max_bytes".into(), json_value(&max_bytes));
    metrics.insert("deferrals_total".into(), json_value(&max_deferrals));
    metrics.insert("timed_out".into(), json_value(&timed_out));
    let snapshot_value = last_snapshot.unwrap_or_default();
    metrics.insert("last_snapshot".into(), json_value(&snapshot_value));
    root.insert("metrics".into(), Value::Object(metrics));
    let summary_value = Value::Object(root);

    let summary_json =
        norito::json::to_string(&summary_value).wrap_err("serialize RBC store summary")?;
    println!("sumeragi_baseline_summary::{RBC_STORE_SCENARIO_NAME}::{summary_json}");

    let summary_pretty = norito::json::to_string_pretty(&summary_value)
        .wrap_err("serialize pretty RBC store summary")?;
    persist_summary_if_requested(RBC_STORE_SCENARIO_NAME, &summary_pretty)?;

    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_redundant_send_retries_update_metrics() -> Result<()> {
    init_instruction_registry();

    let peers = stress_peer_count(4);
    let collectors_k = stress_collectors_k(2, peers);
    let redundant_send_r = stress_redundant_send_r(2);

    let builder = NetworkBuilder::new()
        .with_peers(peers)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], i64::from(collectors_k))
                .write(
                    ["sumeragi", "collectors", "redundant_send_r"],
                    i64::from(redundant_send_r),
                )
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_sessions"],
                    RBC_STORE_MAX_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_sessions"],
                    RBC_STORE_SOFT_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_bytes"],
                    RBC_STORE_MAX_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_bytes"],
                    RBC_STORE_SOFT_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                    900_000i64,
                )
                .write(
                    ["sumeragi", "debug", "rbc", "drop_validator_mask"],
                    0xFF_i64,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(collectors_k),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(redundant_send_r),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(1_500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(1_500),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_redundant_send_retries_update_metrics),
    )
    .await?
    else {
        return Ok(());
    };

    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;

    let chain_id = network.chain_id();
    let message = format!(
        "redundant-send-trigger-{}",
        "R".repeat(RBC_STORE_PAYLOAD_BYTES.saturating_sub(26))
    );
    let tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
        .with_instructions([Log::new(Level::INFO, message)])
        .sign(ALICE_KEYPAIR.private_key());
    let submit_client = network.client();
    tokio::task::spawn_blocking(move || submit_client.submit_transaction(&tx)).await??;

    let client = network.client();
    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;
    let http = reqwest::Client::new();

    let mut saw_collectors_metric = false;
    let mut saw_redundant_metric = false;
    let mut snapshots_checked = 0_u64;
    let start = Instant::now();

    loop {
        if start.elapsed() > CHUNK_LOSS_POLL_TIMEOUT {
            break;
        }

        let response = http
            .get(metrics_url.clone())
            .header("Accept", "text/plain")
            .send()
            .await
            .wrap_err("fetch metrics snapshot")?;
        ensure!(
            response.status().is_success(),
            "metrics endpoint returned status {}",
            response.status()
        );
        let snapshot = response.text().await.wrap_err("read metrics body")?;
        snapshots_checked = snapshots_checked.saturating_add(1);
        let reader = MetricsReader::new(&snapshot);

        if reader
            .get_optional("sumeragi_collectors_targeted_current")
            .is_some()
        {
            saw_collectors_metric = true;
        }
        if reader
            .get_optional("sumeragi_redundant_sends_total")
            .is_some()
        {
            saw_redundant_metric = true;
        }

        if saw_collectors_metric && saw_redundant_metric {
            break;
        }

        sleep(Duration::from_millis(300)).await;
    }

    ensure!(
        snapshots_checked > 0,
        "did not collect any metrics snapshots for redundant send scenario"
    );
    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_rbc_chunk_loss_fault_reports_backlog() -> Result<()> {
    init_instruction_registry();

    let peers = stress_peer_count(4);
    let collectors_k = stress_collectors_k(2, peers);
    let redundant_send_r = stress_redundant_send_r(2);

    let builder = NetworkBuilder::new()
        .with_peers(peers)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], i64::from(collectors_k))
                .write(
                    ["sumeragi", "collectors", "redundant_send_r"],
                    i64::from(redundant_send_r),
                )
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    16_i64 * 1024,
                )
                .write(
                    ["sumeragi", "debug", "rbc", "drop_every_nth_chunk"],
                    CHUNK_LOSS_DROP_INTERVAL,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(collectors_k),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(redundant_send_r),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(1_500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(1_500),
        )));

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_rbc_chunk_loss_fault_reports_backlog),
    )
    .await?
    else {
        return Ok(());
    };

    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;

    let client = network.client();
    let status_before = client.get_status().wrap_err("fetch initial status")?;
    let expected_height = status_before.blocks + 1;

    let chain_id = network.chain_id();
    let message = format!(
        "chunk-loss-fault-{}",
        "C".repeat(RBC_STORE_PAYLOAD_BYTES.saturating_sub(18))
    );
    let tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
        .with_instructions([Log::new(Level::INFO, message)])
        .sign(ALICE_KEYPAIR.private_key());
    let submit_client = network.client();
    tokio::task::spawn_blocking(move || submit_client.submit_transaction(&tx)).await??;

    let http = reqwest::Client::new();
    let sessions_url = client
        .torii_url
        .join("v1/sumeragi/rbc/sessions")
        .wrap_err("compose RBC sessions URL")?;
    let metrics_url = client
        .torii_url
        .join("metrics")
        .wrap_err("compose metrics URL")?;

    let mut found_undelivered = false;
    let mut pending_sessions_max: f64 = 0.0;
    let mut backlog_chunks_max: f64 = 0.0;
    let mut backlog_chunks_total: f64 = 0.0;
    let mut last_metrics_snapshot: Option<String> = None;
    let mut session_view = Value::Null;
    let start = Instant::now();

    loop {
        ensure!(
            start.elapsed() <= CHUNK_LOSS_POLL_TIMEOUT,
            "timed out waiting for RBC chunk loss backlog"
        );

        if let Ok(response) = http
            .get(sessions_url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            && response.status().is_success()
        {
            let body = response.text().await.wrap_err("read sessions body")?;
            let parsed: Value = norito::json::from_str(&body).wrap_err("parse sessions JSON")?;
            if let Some(items) = parsed.get("items").and_then(Value::as_array)
                && let Some(entry) = items.iter().find(|entry| {
                    let height_match = entry
                        .get("height")
                        .and_then(Value::as_u64)
                        .is_some_and(|height| height == expected_height);
                    let delivered = entry
                        .get("delivered")
                        .and_then(Value::as_bool)
                        .unwrap_or(true);
                    height_match && !delivered
                })
            {
                found_undelivered = true;
                session_view = entry.clone();
            }
        }

        let response = http
            .get(metrics_url.clone())
            .header("Accept", "text/plain")
            .send()
            .await
            .wrap_err("fetch metrics snapshot")?;
        ensure!(
            response.status().is_success(),
            "metrics endpoint returned status {}",
            response.status()
        );
        let snapshot = response.text().await.wrap_err("read metrics body")?;
        let reader = MetricsReader::new(&snapshot);

        if let Some(value) = reader.get_optional("sumeragi_rbc_backlog_sessions_pending") {
            pending_sessions_max = pending_sessions_max.max(value);
        }
        if let Some(value) = reader.get_optional("sumeragi_rbc_backlog_chunks_total") {
            backlog_chunks_total = backlog_chunks_total.max(value);
        }
        if let Some(value) = reader.get_optional("sumeragi_rbc_backlog_chunks_max") {
            backlog_chunks_max = backlog_chunks_max.max(value);
        }

        let backlog_observed = found_undelivered
            || pending_sessions_max >= 1.0
            || backlog_chunks_total >= 1.0
            || backlog_chunks_max >= 1.0;
        if backlog_observed {
            last_metrics_snapshot.get_or_insert(snapshot);
            break;
        }

        sleep(RBC_STORE_POLL_INTERVAL).await;
    }

    let backlog_observed = found_undelivered
        || pending_sessions_max >= 1.0
        || backlog_chunks_total >= 1.0
        || backlog_chunks_max >= 1.0;
    ensure!(
        backlog_observed,
        "expected chunk-loss fault to expose RBC backlog signals"
    );

    let mut root = Map::new();
    root.insert("scenario".into(), json_value(&CHUNK_LOSS_SCENARIO_NAME));
    root.insert("session".into(), session_view.clone());
    let mut metrics = Map::new();
    metrics.insert(
        "pending_sessions_max".into(),
        json_value(&pending_sessions_max),
    );
    metrics.insert(
        "backlog_chunks_total".into(),
        json_value(&backlog_chunks_total),
    );
    metrics.insert("backlog_chunks_max".into(), json_value(&backlog_chunks_max));
    metrics.insert("found_undelivered".into(), json_value(&found_undelivered));
    let metrics_snapshot = last_metrics_snapshot.unwrap_or_default();
    metrics.insert("last_snapshot".into(), json_value(&metrics_snapshot));
    root.insert("metrics".into(), Value::Object(metrics));
    let summary_value = Value::Object(root);

    let summary_json =
        norito::json::to_string(&summary_value).wrap_err("serialize chunk-loss summary")?;
    println!("sumeragi_baseline_summary::{CHUNK_LOSS_SCENARIO_NAME}::{summary_json}");

    let summary_pretty = norito::json::to_string_pretty(&summary_value)
        .wrap_err("serialize pretty chunk-loss summary")?;
    persist_summary_if_requested(CHUNK_LOSS_SCENARIO_NAME, &summary_pretty)?;

    network.shutdown().await;
    Ok(())
}

fn persist_summary_if_requested(scenario: &str, summary_pretty: &str) -> Result<()> {
    let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {
        Ok(dir) => dir,
        Err(_) => return Ok(()),
    };

    let root = std::path::Path::new(&dir);
    fs::create_dir_all(root).wrap_err("create baseline artifact directory")?;
    let path = root.join(format!("{scenario}.summary.json"));
    fs::write(&path, format!("{summary_pretty}\n"))
        .wrap_err_with(|| format!("write summary file {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Stats;

    #[test]
    fn stats_from_samples_computes_median() {
        let stats = Stats::from_samples(&[1.0, 3.0, 2.0]).expect("stats");
        assert!((stats.median - 2.0).abs() < f64::EPSILON);
        assert_eq!(stats.samples, 3);
    }
}
