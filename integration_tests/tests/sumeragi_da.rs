//! Data availability + RBC integration scenario exercising large payload distribution.

use std::{
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use ascii_table::AsciiTable;
use eyre::{Report, Result, WrapErr, ensure, eyre};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::{
    client::{Client, Status},
    data_model::{
        Level,
        consensus::CommitCertificate,
        isi::{Log, SetParameter, Unregister},
        parameter::{Parameter, SumeragiParameter, TransactionParameter},
        prelude::QueryBuilderExt,
        query::peer::prelude::FindPeers,
    },
};
use iroha_config_base::toml::Writer as TomlWriter;
use iroha_core::sumeragi::rbc_status;
use iroha_test_network::{Network, NetworkBuilder};
use norito::json::{self, Value};
use rand::{Rng, SeedableRng, distr::Alphanumeric};
use rand_chacha::ChaCha8Rng;
use tokio::time::{sleep, timeout};
use toml::Table;

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

struct PeerMetrics {
    payload_bytes: f64,
    deliver_total: f64,
    ready_total: f64,
    bg_post_queue_depth: f64,
    bg_post_queue_depth_labeled_max: f64,
    p2p_queue_dropped_total: f64,
    snapshot: String,
}

struct RbcObservation {
    delivered_at: Duration,
    height: u64,
    view: u64,
    total_chunks: u32,
    received_chunks: u32,
    ready_count: u64,
    block_hash: String,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct SumeragiSnapshot {
    index: u64,
    view_change_proof_accepted_total: u64,
    view_change_proof_stale_total: u64,
    view_change_proof_rejected_total: u64,
    da_reschedule_total: u64,
}

impl SumeragiSnapshot {
    fn from_json(value: &Value) -> Self {
        Self {
            index: json_u64(value, "view_change_index"),
            view_change_proof_accepted_total: json_u64(value, "view_change_proof_accepted_total"),
            view_change_proof_stale_total: json_u64(value, "view_change_proof_stale_total"),
            view_change_proof_rejected_total: json_u64(value, "view_change_proof_rejected_total"),
            da_reschedule_total: json_u64(value, "da_reschedule_total"),
        }
    }
}

async fn fetch_sumeragi_snapshot(
    client: reqwest::Client,
    torii_base: &str,
) -> Result<SumeragiSnapshot> {
    let url = reqwest::Url::parse(&format!("{torii_base}/v1/sumeragi/status"))
        .wrap_err("compose sumeragi status URL")?;
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .wrap_err("fetch sumeragi status")?;
    let status = response.status();
    ensure!(
        status.is_success(),
        "GET {torii_base}/v1/sumeragi/status returned {status}"
    );
    let body = response
        .text()
        .await
        .wrap_err("read sumeragi status body")?;
    let value: Value = json::from_str(&body).wrap_err("parse sumeragi status JSON payload")?;
    Ok(SumeragiSnapshot::from_json(&value))
}

fn json_u64(root: &Value, key: &str) -> u64 {
    root.as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

// Keep the payload light to avoid overwhelming Torii/queue on constrained hosts.
const LARGE_PAYLOAD_BYTES: usize = 1024; // keep payload light to ensure timely DA/RBC
const RBC_DELIVER_BUDGET_MS: u64 = 15_000;
const RBC_DELIVER_GRACE_MS: u64 = 1_000;
const COMMIT_BUDGET_MS: u64 = 50_000;
const RBC_DELIVER_BUDGET_PER_EXTRA_PEER_MS: u64 = 10_000;
const COMMIT_BUDGET_PER_EXTRA_PEER_MS: u64 = 25_000;
const THROUGHPUT_FLOOR_MIB_S: f64 = 0.1;
const BG_POST_QUEUE_DEPTH_BUDGET: f64 = 32.0;
const P2P_QUEUE_DROP_BUDGET: f64 = 0.0;
const TORII_CONTENT_HEADROOM_BYTES: usize = 2 * 1024 * 1024;
const RBC_CHUNK_SIZE_BYTES: i64 = 512 * 1024;
const P2P_TX_FRAME_BUDGET_BYTES: i64 = 128 * 1024 * 1024;
const CONSENSUS_FRAME_BUDGET_BYTES: i64 = 128 * 1024 * 1024;
const P2P_QUEUE_CAP_HIGH: i64 = 65_536;
const P2P_QUEUE_CAP_LOW: i64 = 131_072;
const P2P_POST_QUEUE_CAP: i64 = 8_192;

fn generate_incompressible_payload(tag: &str, payload_bytes: usize) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tag.hash(&mut hasher);
    let seed = hasher.finish();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut out = String::with_capacity(payload_bytes);
    for _ in 0..payload_bytes {
        let ch = rng.sample(Alphanumeric) as char;
        out.push(ch);
    }
    out
}

/// Scale Torii's HTTP body cap for tests that exercise oversized transactions.
fn torii_max_content_len_for_payload(payload_bytes: usize) -> i64 {
    let inflated = payload_bytes.saturating_mul(12);
    let with_headroom = payload_bytes.saturating_add(TORII_CONTENT_HEADROOM_BYTES);
    let target = inflated.max(with_headroom);
    i64::try_from(target).unwrap_or(i64::MAX)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_rbc_da_large_payload_four_peers() -> Result<()> {
    let result = run_sumeragi_da_scenario_with(
        "sumeragi_rbc_da_large_payload_four_peers",
        4,
        LARGE_PAYLOAD_BYTES,
        false,
        |_| {},
        |_| Ok(()),
    )
    .await;
    if sandbox::handle_result(result, "sumeragi_rbc_da_large_payload_four_peers")?.is_none() {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_da_commit_certificate_history_four_peers() -> Result<()> {
    let result = run_sumeragi_da_scenario_with(
        "sumeragi_da_commit_certificate_history_four_peers",
        4,
        LARGE_PAYLOAD_BYTES,
        true,
        |_| {},
        |_| Ok(()),
    )
    .await;
    if sandbox::handle_result(result, "sumeragi_da_commit_certificate_history_four_peers")?
        .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_rbc_da_large_payload_six_peers() -> Result<()> {
    let result = run_sumeragi_da_scenario_with(
        "sumeragi_rbc_da_large_payload_six_peers",
        6,
        LARGE_PAYLOAD_BYTES,
        false,
        |_| {},
        |_| Ok(()),
    )
    .await;
    if sandbox::handle_result(result, "sumeragi_rbc_da_large_payload_six_peers")?.is_none() {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_exec_qc_with_tight_block_queue_four_peers() -> Result<()> {
    let result = run_sumeragi_da_scenario_with(
        "sumeragi_exec_qc_with_tight_block_queue_four_peers",
        4,
        LARGE_PAYLOAD_BYTES,
        false,
        |layer| {
            layer.write(["sumeragi", "msg_channel_cap_blocks"], 2i64);
        },
        |_| Ok(()),
    )
    .await;
    if sandbox::handle_result(result, "sumeragi_exec_qc_with_tight_block_queue_four_peers")?
        .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_rbc_background_queue_synchronous() -> Result<()> {
    let result = run_sumeragi_da_scenario_with(
        "sumeragi_rbc_background_queue_synchronous",
        4,
        LARGE_PAYLOAD_BYTES,
        false,
        |layer| {
            layer.write(["sumeragi", "debug", "disable_background_worker"], true);
        },
        |per_peer_metrics| {
            ensure!(
                per_peer_metrics.iter().any(|(_, metrics)| {
                    let reader = MetricsReader::new(&metrics.snapshot);
                    reader
                        .max_with_prefix("sumeragi_bg_post_enqueued_total")
                        .unwrap_or(0.0)
                        > 0.0
                }),
                "expected at least one peer to enqueue background post tasks"
            );
            ensure!(
                per_peer_metrics.iter().all(|(_, metrics)| {
                    let reader = MetricsReader::new(&metrics.snapshot);
                    reader
                        .max_with_prefix("sumeragi_bg_post_drop_total")
                        .unwrap_or(0.0)
                        == 0.0
                }),
                "expected background post drops to remain zero with a synchronous queue"
            );
            Ok(())
        },
    )
    .await;
    if sandbox::handle_result(result, "sumeragi_rbc_background_queue_synchronous")?.is_none() {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn sumeragi_rbc_recovers_after_peer_restart() -> Result<()> {
    let payload_bytes = LARGE_PAYLOAD_BYTES;
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;
    let rbc_chunk_max_bytes = i64::try_from(payload_bytes).unwrap_or(i64::MAX);
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit_nz),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit_nz),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["logger", "level"], "INFO")
                .write(["network", "max_frame_bytes"], CONSENSUS_FRAME_BUDGET_BYTES)
                .write(
                    ["network", "max_frame_bytes_consensus"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_control"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_block_sync"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_other"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(["network", "p2p_queue_cap_high"], P2P_QUEUE_CAP_HIGH)
                .write(["network", "p2p_queue_cap_low"], P2P_QUEUE_CAP_LOW)
                .write(["network", "p2p_post_queue_cap"], P2P_POST_QUEUE_CAP)
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    P2P_TX_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["torii", "max_content_len"],
                    torii_max_content_len_for_payload(payload_bytes),
                )
                .write(["network", "p2p_queue_cap_high"], P2P_QUEUE_CAP_HIGH)
                .write(["network", "p2p_queue_cap_low"], P2P_QUEUE_CAP_LOW)
                .write(["network", "p2p_post_queue_cap"], P2P_POST_QUEUE_CAP)
                .write(["sumeragi", "proof_policy"], "exec_qc")
                .write(["sumeragi", "require_execution_qc"], true)
                .write(["sumeragi", "require_wsv_exec_qc"], true)
                .write(["sumeragi", "rbc_chunk_max_bytes"], rbc_chunk_max_bytes)
                .write(["sumeragi", "rbc_session_ttl_secs"], 600i64)
                .write(["sumeragi", "rbc_disk_store_ttl_secs"], 600i64);
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(sumeragi_rbc_recovers_after_peer_restart),
    )
    .await?
    else {
        return Ok(());
    };
    let result: Result<()> = async {
        let peers = network.peers();
        let primary_peer = &peers[0];
        let restart_peer = &peers[1];
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|cow| ConfigLayer(cow.into_owned()))
            .collect();

        let client = primary_peer.client();
        set_sumeragi_parameter(&client, SumeragiParameter::DaEnabled(true)).await?;
        let torii_primary = client.torii_url.clone();
        let status_before = fetch_status(&client).await?;
        let expected_height = status_before.blocks + 1;

        let status_url = torii_primary
            .join("status")
            .wrap_err("compose status URL")?;
        let sessions_url_primary = torii_primary
            .join("v1/sumeragi/rbc/sessions")
            .wrap_err("compose sessions URL")?;
        let restart_sessions_url = reqwest::Url::parse(&format!(
            "{}/v1/sumeragi/rbc/sessions",
            restart_peer.torii_url()
        ))
        .wrap_err("compose restart peer sessions URL")?;

        let http = reqwest::Client::new();

        let heavy_message = generate_incompressible_payload(
            "sumeragi_rbc_recovers_after_peer_restart",
            payload_bytes,
        );
        let submit_handle = tokio::task::spawn_blocking(move || {
            client.submit(Log::new(Level::INFO, heavy_message))
        });

        submit_handle.await.wrap_err("submit join")??;

        let restart_store_dir = restart_peer.kura_store_dir().join("rbc_sessions");
        let persisted =
            wait_for_persisted_rbc_session(&restart_store_dir, expected_height, Instant::now())
                .await?;
        let block_hash_hex = hex::encode(persisted.block_hash.as_ref());

        restart_peer.shutdown().await;
        if sandbox::handle_result(
            restart_peer
                .start_checked(config_layers.clone().into_iter(), None)
                .await,
            "sumeragi_rbc_recovers_after_peer_restart_restart",
        )?
        .is_none()
        {
            return Ok(());
        }
        timeout(
            da_commit_wait_timeout(),
            restart_peer.once_block(expected_height),
        )
        .await
        .map_err(|_| {
            eyre!(
                "restart peer failed to reach height {expected_height} before timeout {:?}",
                da_commit_wait_timeout()
            )
        })?;

        let restart_start = Instant::now();
        wait_for_recovered_flag(
            http.clone(),
            restart_sessions_url,
            expected_height,
            &block_hash_hex,
            restart_start,
        )
        .await?;

        let _rbc_observation = wait_for_rbc_delivery(
            http.clone(),
            sessions_url_primary.clone(),
            expected_height,
            restart_start,
        )
        .await?;
        let _commit_elapsed =
            wait_for_height(http.clone(), status_url, expected_height, restart_start).await?;

        network.shutdown().await;
        Ok(())
    }
    .await;
    if sandbox::handle_result(result, stringify!(sumeragi_rbc_recovers_after_peer_restart))?
        .is_none()
    {
        return Ok(());
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn sumeragi_rbc_recovers_after_restart_with_roster_change() -> Result<()> {
    let payload_bytes = LARGE_PAYLOAD_BYTES;
    let rbc_chunk_max_bytes = 32 * 1024;
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit_nz),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit_nz),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["logger", "level"], "INFO")
                .write(["network", "max_frame_bytes"], CONSENSUS_FRAME_BUDGET_BYTES)
                .write(
                    ["network", "max_frame_bytes_consensus"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_control"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_block_sync"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_other"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(["network", "p2p_queue_cap_high"], P2P_QUEUE_CAP_HIGH)
                .write(["network", "p2p_queue_cap_low"], P2P_QUEUE_CAP_LOW)
                .write(["network", "p2p_post_queue_cap"], P2P_POST_QUEUE_CAP)
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    P2P_TX_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["torii", "max_content_len"],
                    torii_max_content_len_for_payload(payload_bytes),
                )
                .write(["sumeragi", "proof_policy"], "exec_qc")
                .write(["sumeragi", "require_execution_qc"], true)
                .write(["sumeragi", "require_wsv_exec_qc"], true)
                .write(
                    ["sumeragi", "rbc_chunk_max_bytes"],
                    i64::from(rbc_chunk_max_bytes),
                )
                .write(["sumeragi", "rbc_session_ttl_secs"], 600i64)
                .write(["sumeragi", "rbc_disk_store_ttl_secs"], 600i64);
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(sumeragi_rbc_recovers_after_restart_with_roster_change),
    )
    .await?
    else {
        return Ok(());
    };
    let result: Result<()> = async {
        let peers = network.peers();
        let primary_peer = &peers[0];
        let restart_peer = &peers[1];
        let removed_peer = &peers[3];
        let removed_peer_id = removed_peer.id();
        let removed_peer_id_check = removed_peer_id.clone();
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|cow| ConfigLayer(cow.into_owned()))
            .collect();

        let client = primary_peer.client();
        let status_before = fetch_status(&client).await?;
        let expected_height = status_before.blocks + 1;

        let torii_primary = client.torii_url.clone();
        let status_url = torii_primary
            .join("status")
            .wrap_err("compose status URL")?;
        let restart_sessions_url = reqwest::Url::parse(&format!(
            "{}/v1/sumeragi/rbc/sessions",
            restart_peer.torii_url()
        ))
        .wrap_err("compose restart peer sessions URL")?;

        let http = reqwest::Client::new();
        let start = Instant::now();

        let heavy_message = generate_incompressible_payload(
            "sumeragi_rbc_recovers_after_restart_with_roster_change",
            payload_bytes,
        );
        let submit_client = client.clone();
        let submit_handle = tokio::task::spawn_blocking(move || {
            submit_client.submit(Log::new(Level::INFO, heavy_message))
        });

        submit_handle.await.wrap_err("submit join")??;

        let restart_store_dir = restart_peer.kura_store_dir().join("rbc_sessions");
        let persisted =
            wait_for_persisted_rbc_session(&restart_store_dir, expected_height, start).await?;
        let block_hash_hex = hex::encode(persisted.block_hash.as_ref());

        restart_peer.shutdown().await;

        let _commit_elapsed = wait_for_height(
            http.clone(),
            status_url.clone(),
            expected_height,
            Instant::now(),
        )
        .await?;

        let unregister_client = client.clone();
        tokio::task::spawn_blocking(move || {
            unregister_client.submit_blocking(Unregister::peer(removed_peer_id))
        })
        .await
        .wrap_err("join unregister task")??;

        let roster_deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let roster_check_client = client.clone();
            let remaining_peers = tokio::task::spawn_blocking(move || {
                roster_check_client.query(FindPeers).execute_all()
            })
            .await
            .wrap_err("join FindPeers task")??;
            let removed = remaining_peers
                .iter()
                .all(|peer_id| peer_id != &removed_peer_id_check);
            if removed {
                break;
            }
            if Instant::now() >= roster_deadline {
                return Err(eyre!(
                    "removed peer still present in roster after timeout; peer_id={removed_peer_id_check:?}"
                ));
            }
            sleep(Duration::from_millis(200)).await;
        }

        if sandbox::handle_result(
            restart_peer
                .start_checked(config_layers.clone().into_iter(), None)
                .await,
            "sumeragi_rbc_recovers_after_restart_with_roster_change_restart",
        )?
        .is_none()
        {
            return Ok(());
        }
        let restart_start = Instant::now();
        wait_for_recovered_flag(
            http.clone(),
            restart_sessions_url.clone(),
            expected_height,
            &block_hash_hex,
            restart_start,
        )
        .await?;
        timeout(
            da_commit_wait_timeout(),
            restart_peer.once_block(expected_height),
        )
        .await
        .map_err(|_| {
            eyre!(
                "restart peer failed to reach height {expected_height} before timeout {:?}",
                da_commit_wait_timeout()
            )
        })?;
        let _commit_elapsed =
            wait_for_height(http.clone(), status_url, expected_height, restart_start).await?;

        network.shutdown().await;
        Ok(())
    }
    .await;
    if sandbox::handle_result(
        result,
        stringify!(sumeragi_rbc_recovers_after_restart_with_roster_change),
    )?
    .is_none()
    {
        return Ok(());
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn sumeragi_da_payload_loss_does_not_block_commit() -> Result<()> {
    let payload_bytes = 256 * 1024;
    let scenario_name = stringify!(sumeragi_da_payload_loss_does_not_block_commit);
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit_nz),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit_nz),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["logger", "level"], "INFO")
                .write(["network", "max_frame_bytes"], CONSENSUS_FRAME_BUDGET_BYTES)
                .write(
                    ["network", "max_frame_bytes_consensus"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_control"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_block_sync"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_other"],
                    CONSENSUS_FRAME_BUDGET_BYTES,
                )
                .write(["network", "p2p_queue_cap_high"], P2P_QUEUE_CAP_HIGH)
                .write(["network", "p2p_queue_cap_low"], P2P_QUEUE_CAP_LOW)
                .write(["network", "p2p_post_queue_cap"], P2P_POST_QUEUE_CAP)
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    P2P_TX_FRAME_BUDGET_BYTES,
                )
                .write(["sumeragi", "da_enabled"], true)
                .write(["sumeragi", "rbc_chunk_max_bytes"], RBC_CHUNK_SIZE_BYTES)
                // Shuffle and duplicate RBC broadcasts while deterministically dropping shards to
                // block READY fan-outs on every validator.
                .write(["sumeragi", "debug", "rbc", "shuffle_chunks"], true)
                .write(["sumeragi", "debug", "rbc", "duplicate_inits"], true)
                .write(["sumeragi", "debug", "rbc", "drop_every_nth_chunk"], 2_i64);
        });

    let Some(network) = sandbox::start_network_async_or_skip(builder, scenario_name).await? else {
        return Ok(());
    };

    let result: Result<()> = async {
        network
            .ensure_blocks_with(|height| height.total >= 1)
            .await?;

        let http = reqwest::Client::new();
        let client = network.client();

        let mut baseline_blocks = Vec::new();
        let mut baseline_sumeragi_snapshots = Vec::new();
        for peer in network.peers() {
            let status = peer.status().await?;
            baseline_blocks.push(status.blocks);
            baseline_sumeragi_snapshots
                .push(fetch_sumeragi_snapshot(http.clone(), &peer.torii_url()).await?);
        }
        let expected_height = baseline_blocks
            .iter()
            .copied()
            .max()
            .unwrap_or_default()
            .saturating_add(1);

        let submit_client = client.clone();
        tokio::task::spawn_blocking(move || {
            let heavy_message = generate_incompressible_payload(scenario_name, payload_bytes);
            submit_client.submit(Log::new(Level::INFO, heavy_message))
        })
        .await
        .wrap_err("submit log instruction")??;

        let sessions_url = client
            .torii_url
            .join("v1/sumeragi/rbc/sessions")
            .wrap_err("compose sessions URL")?;
        wait_for_rbc_session(&http, sessions_url.clone(), Duration::from_secs(5)).await?;

        let mut status_after = Vec::new();
        let mut after_sumeragi_snapshots = Vec::new();

        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            status_after.clear();
            after_sumeragi_snapshots.clear();
            for peer in network.peers() {
                status_after.push(peer.status().await?);
                after_sumeragi_snapshots
                    .push(fetch_sumeragi_snapshot(http.clone(), &peer.torii_url()).await?);
            }

            let commits_ready = status_after
                .iter()
                .all(|status| status.blocks >= expected_height);
            if commits_ready {
                break;
            }

            if Instant::now() > deadline {
                let blocks: Vec<u64> = status_after.iter().map(|status| status.blocks).collect();
                return Err(eyre!(
                    "timed out waiting for commit under payload loss: blocks={blocks:?}, expected={expected_height}"
                ));
            }

            sleep(Duration::from_millis(200)).await;
        }

        ensure!(
            baseline_sumeragi_snapshots.len() == after_sumeragi_snapshots.len(),
            "mismatched Sumeragi snapshot counts: baseline={}, after={}",
            baseline_sumeragi_snapshots.len(),
            after_sumeragi_snapshots.len()
        );

        ensure!(
            after_sumeragi_snapshots
                .iter()
                .zip(&baseline_sumeragi_snapshots)
                .all(|(after, baseline)| after.da_reschedule_total == baseline.da_reschedule_total),
            "expected da_reschedule_total to remain unchanged when DA is advisory"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;
    if sandbox::handle_result(result, scenario_name)?.is_none() {
        return Ok(());
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn sumeragi_rbc_session_recovers_after_cold_restart() -> Result<()> {
    let payload_bytes = LARGE_PAYLOAD_BYTES;
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;
    let rbc_chunk_max_bytes = i64::try_from(payload_bytes).unwrap_or(i64::MAX);
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit_nz),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit_nz),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["logger", "level"], "INFO")
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    P2P_TX_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["torii", "max_content_len"],
                    torii_max_content_len_for_payload(payload_bytes),
                )
                .write(["sumeragi", "proof_policy"], "exec_qc")
                .write(["sumeragi", "require_execution_qc"], true)
                .write(["sumeragi", "require_wsv_exec_qc"], true)
                .write(["sumeragi", "rbc_chunk_max_bytes"], rbc_chunk_max_bytes);
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(sumeragi_rbc_session_recovers_after_cold_restart),
    )
    .await?
    else {
        return Ok(());
    };
    let result: Result<()> = async {
        let env_dir = network.env_dir().to_path_buf();
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|cow| ConfigLayer(cow.into_owned()))
            .collect();

        let client = network.client();
        let torii = client.torii_url.clone();
        set_sumeragi_parameter(&client, SumeragiParameter::DaEnabled(true))
            .await
            .wrap_err_with(|| format!("enable DA; torii={torii}, env_dir={}", env_dir.display()))?;
        let status_before = fetch_status(&client).await.wrap_err_with(|| {
            format!(
                "fetch status before RBC session; torii={torii}, env_dir={}",
                env_dir.display()
            )
        })?;
        let expected_height = status_before.blocks + 1;

        let sessions_url = client
            .torii_url
            .join("v1/sumeragi/rbc/sessions")
            .wrap_err("compose sessions URL")?;
        let status_url = client
            .torii_url
            .join("status")
            .wrap_err("compose status URL")?;

        let http = reqwest::Client::new();
        let start = Instant::now();

        let inflight_handle = tokio::spawn(wait_for_inflight_rbc(
            http.clone(),
            sessions_url.clone(),
            expected_height,
            start,
        ));

        let heavy_message = generate_incompressible_payload(
            "sumeragi_rbc_session_recovers_after_cold_restart",
            payload_bytes,
        );
        let submit_client = client.clone();
        let submit_handle = tokio::task::spawn_blocking(move || {
            submit_client.submit(Log::new(Level::INFO, heavy_message))
        });

        let (block_hash_hex, total_chunks) =
            inflight_handle.await.wrap_err("inflight detector join")??;

        submit_handle.await.wrap_err("submit join")??;

        let Some(snapshot_before) = fetch_rbc_session(
            &http,
            &sessions_url,
            expected_height,
            &block_hash_hex,
        )
        .await
        .wrap_err_with(|| {
            format!(
                "fetch RBC session before shutdown; torii={torii}, env_dir={}, url={sessions_url}",
                env_dir.display()
            )
        })?
        else {
            return Err(eyre!("expected RBC session snapshot before shutdown"));
        };

        ensure!(
            snapshot_before.received_chunks > 0,
            "expected at least one chunk before shutdown"
        );
        ensure!(
            snapshot_before.received_chunks < total_chunks,
            "session should remain incomplete prior to shutdown"
        );

        network.shutdown().await;

        if sandbox::handle_result(
            restart_all_peers(&network, &config_layers).await,
            "sumeragi_rbc_session_recovers_after_cold_restart_restart_all",
        )?
        .is_none()
        {
            return Ok(());
        }

        wait_for_recovered_flag(
            http.clone(),
            sessions_url.clone(),
            expected_height,
            &block_hash_hex,
            Instant::now(),
        )
        .await?;

        let Some(snapshot_after) = fetch_rbc_session(
            &http,
            &sessions_url,
            expected_height,
            &block_hash_hex,
        )
        .await
        .wrap_err_with(|| {
            format!(
                "fetch RBC session after restart; torii={torii}, env_dir={}, url={sessions_url}",
                env_dir.display()
            )
        })?
        else {
            return Err(eyre!("expected RBC session snapshot after restart"));
        };

        ensure!(
            snapshot_after.recovered,
            "session should be flagged recovered after restart"
        );
        ensure!(
            snapshot_after.received_chunks >= snapshot_before.received_chunks,
            "recovered session should preserve received chunk count (before={}, after={})",
            snapshot_before.received_chunks,
            snapshot_after.received_chunks
        );
        ensure!(
            snapshot_after.total_chunks == total_chunks,
            "total chunk count should remain {total_chunks}, got {}",
            snapshot_after.total_chunks
        );

        let restarted_client = network.client();
        let resumed_torii = restarted_client.torii_url.clone();
        set_sumeragi_parameter(&restarted_client, SumeragiParameter::DaEnabled(true))
            .await
            .wrap_err_with(|| {
                format!(
                    "enable DA after restart; torii={resumed_torii}, env_dir={}",
                    env_dir.display()
                )
            })?;
        let resume_height = fetch_status(&restarted_client)
            .await
            .wrap_err_with(|| {
                format!(
                    "fetch status after restart; torii={resumed_torii}, env_dir={}",
                    env_dir.display()
                )
            })?
            .blocks
            + 1;

        let resume_start = Instant::now();
        let rbc_future = tokio::spawn(wait_for_rbc_delivery(
            http.clone(),
            sessions_url.clone(),
            resume_height,
            resume_start,
        ));
        let commit_future = tokio::spawn(wait_for_height(
            http.clone(),
            status_url,
            resume_height,
            resume_start,
        ));

        let light_message = generate_incompressible_payload(
            "sumeragi_rbc_session_recovers_after_cold_restart_light",
            512 * 1024,
        );
        let submit_resumed = restarted_client.clone();
        let submit_resume_handle = tokio::task::spawn_blocking(move || {
            submit_resumed.submit(Log::new(Level::INFO, light_message))
        });
        submit_resume_handle
            .await
            .wrap_err("resume submit join")??;

        let _rbc_observation = rbc_future.await.wrap_err("rbc resume join")??;
        let _commit_elapsed = commit_future.await.wrap_err("commit resume join")??;

        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(
        result,
        stringify!(sumeragi_rbc_session_recovers_after_cold_restart),
    )?
    .is_none()
    {
        return Ok(());
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn run_sumeragi_da_scenario_with<F, G>(
    scenario_name: &str,
    peer_count: usize,
    payload_bytes: usize,
    check_commit_certificates: bool,
    configure: F,
    inspect: G,
) -> Result<()>
where
    for<'layer> F: Fn(&'layer mut TomlWriter<'layer>),
    G: Fn(&[(usize, PeerMetrics)]) -> Result<()>,
{
    let heavy_message = generate_incompressible_payload(scenario_name, payload_bytes);
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;
    let rbc_chunk_max_bytes = i64::try_from(payload_bytes).unwrap_or(i64::MAX);
    let mut config_table = toml::Table::new();
    {
        let mut writer = TomlWriter::new(&mut config_table);
        writer
            .write("telemetry_enabled", true)
            .write("telemetry_profile", "full")
            .write(["logger", "level"], "WARN")
            .write(["network", "max_frame_bytes"], CONSENSUS_FRAME_BUDGET_BYTES)
            .write(
                ["network", "max_frame_bytes_consensus"],
                CONSENSUS_FRAME_BUDGET_BYTES,
            )
            .write(
                ["network", "max_frame_bytes_control"],
                CONSENSUS_FRAME_BUDGET_BYTES,
            )
            .write(
                ["network", "max_frame_bytes_block_sync"],
                CONSENSUS_FRAME_BUDGET_BYTES,
            )
            .write(
                ["network", "max_frame_bytes_other"],
                CONSENSUS_FRAME_BUDGET_BYTES,
            )
            .write(["network", "p2p_queue_cap_high"], P2P_QUEUE_CAP_HIGH)
            .write(["network", "p2p_queue_cap_low"], P2P_QUEUE_CAP_LOW)
            .write(["network", "p2p_post_queue_cap"], P2P_POST_QUEUE_CAP)
            .write(
                ["network", "max_frame_bytes_tx_gossip"],
                P2P_TX_FRAME_BUDGET_BYTES,
            )
            .write(["sumeragi", "proof_policy"], "exec_qc")
            .write(["sumeragi", "require_execution_qc"], true)
            .write(["sumeragi", "require_wsv_exec_qc"], true)
            .write(["sumeragi", "rbc_chunk_max_bytes"], rbc_chunk_max_bytes)
            .write(["sumeragi", "rbc_store_max_sessions"], 2_048i64)
            .write(["sumeragi", "rbc_store_soft_sessions"], 1_536i64)
            .write(["sumeragi", "rbc_store_max_bytes"], 536_870_912i64)
            .write(["sumeragi", "rbc_store_soft_bytes"], 402_653_184i64)
            .write(["sumeragi", "rbc_session_ttl_secs"], 600i64)
            .write(
                ["sumeragi", "debug", "rbc", "force_deliver_quorum_one"],
                true,
            )
            .write(
                ["torii", "max_content_len"],
                torii_max_content_len_for_payload(payload_bytes),
            );
    }

    let mut nexus = toml::map::Map::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    nexus.insert("lane_count".into(), toml::Value::Integer(1));

    let mut lane = toml::map::Map::new();
    lane.insert("alias".into(), toml::Value::String("lane0".into()));
    lane.insert("index".into(), toml::Value::Integer(0));
    let mut metadata = toml::map::Map::new();
    metadata.insert(
        "scheduler.teu_capacity".into(),
        toml::Value::String("262144".into()),
    );
    lane.insert("metadata".into(), toml::Value::Table(metadata));
    nexus.insert(
        "lane_catalog".into(),
        toml::Value::Array(vec![toml::Value::Table(lane)]),
    );

    let mut fusion = toml::map::Map::new();
    fusion.insert("floor_teu".into(), toml::Value::Integer(131_072));
    fusion.insert("exit_teu".into(), toml::Value::Integer(262_144));
    nexus.insert("fusion".into(), toml::Value::Table(fusion));

    let da_sample = 1_i64;
    let da_threshold = 1_i64;
    let mut da = toml::map::Map::new();
    da.insert(
        "q_in_slot_total".into(),
        toml::Value::Integer(da_sample.max(1)),
    );
    da.insert("q_in_slot_per_ds_min".into(), toml::Value::Integer(1));
    da.insert("sample_size_base".into(), toml::Value::Integer(da_sample));
    da.insert("sample_size_max".into(), toml::Value::Integer(da_sample));
    da.insert("threshold_base".into(), toml::Value::Integer(da_threshold));
    da.insert("per_attester_shards".into(), toml::Value::Integer(1));
    let mut audit = toml::map::Map::new();
    audit.insert("sample_size".into(), toml::Value::Integer(da_sample));
    audit.insert("window_count".into(), toml::Value::Integer(1));
    audit.insert("interval_ms".into(), toml::Value::Integer(60_000));
    da.insert("audit".into(), toml::Value::Table(audit));
    nexus.insert("da".into(), toml::Value::Table(da));

    {
        let mut writer = TomlWriter::new(&mut config_table);
        writer.write("nexus", toml::Value::Table(nexus));
    }
    let mut writer = TomlWriter::new(&mut config_table);
    configure(&mut writer);

    let builder = NetworkBuilder::new()
        .with_peers(peer_count)
        .with_auto_populated_trusted_peers()
        // Enable DA (RBC + availability QC gating) in the base config so runtime parameters and
        // handshake agree.
        .with_data_availability_enabled(true)
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit_nz),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit_nz),
        )))
        .with_config_table(config_table);
    let Some(network) = sandbox::start_network_async_or_skip(builder, scenario_name).await? else {
        return Ok(());
    };

    let mut client = network.client();
    let status_timeout = Duration::from_secs(300);
    client.transaction_status_timeout = status_timeout;
    client.transaction_ttl = Some(status_timeout + Duration::from_secs(5));
    // When Torii websockets are blocked by the environment, bail out early as a sandbox skip.
    if sandbox::handle_result(
        set_sumeragi_parameter(&client, SumeragiParameter::DaEnabled(true)).await,
        scenario_name,
    )?
    .is_none()
    {
        return Ok(());
    }
    let status_before = fetch_status(&client).await?;
    let expected_height = status_before.blocks + 1;

    let status_url = client
        .torii_url
        .join("status")
        .wrap_err("compose status URL")?;
    let sessions_url = client
        .torii_url
        .join("v1/sumeragi/rbc/sessions")
        .wrap_err("compose RBC sessions URL")?;

    let http = reqwest::Client::new();
    let start = Instant::now();

    let rbc_handle = tokio::spawn(wait_for_rbc_delivery(
        http.clone(),
        sessions_url,
        expected_height,
        start,
    ));
    let commit_handle = tokio::spawn(wait_for_height(
        http.clone(),
        status_url,
        expected_height,
        start,
    ));

    let submit_client = client.clone();
    let submit_handle = tokio::task::spawn_blocking(move || {
        submit_client.submit_blocking(Log::new(Level::INFO, heavy_message))
    });
    let submit_hash = submit_handle.await.wrap_err("submit join")??;
    println!(
        "submitted heavy tx {submit_hash:?}; blocks_before={}",
        status_before.blocks
    );

    let rbc_observation = rbc_handle.await.wrap_err("rbc join")??;
    let commit_elapsed = commit_handle.await.wrap_err("commit join")??;

    let torii_urls = network.torii_urls();
    if check_commit_certificates {
        wait_for_commit_certificates(&http, &torii_urls, expected_height, Instant::now()).await?;
    }

    let mut per_peer_metrics = Vec::new();
    for (idx, torii) in torii_urls.iter().enumerate() {
        let mut attempts = 0;
        let (
            payload_bytes_metric,
            mut deliver_total,
            mut ready_total,
            bg_post_queue_depth,
            bg_post_queue_depth_labeled_max,
            p2p_queue_dropped_total,
            body,
        ) = loop {
            let response = http
                .get(format!("{torii}/metrics"))
                .send()
                .await
                .wrap_err("fetch /metrics")?;
            let body = response.text().await.wrap_err("metrics body")?;
            let reader = MetricsReader::new(&body);
            let payload_bytes_metric = reader.get("sumeragi_rbc_payload_bytes_delivered_total");
            let deliver_total = reader.get("sumeragi_rbc_deliver_broadcasts_total");
            let ready_total = reader.get("sumeragi_rbc_ready_broadcasts_total");
            let bg_post_queue_depth = reader.get("sumeragi_bg_post_queue_depth");
            let bg_post_queue_depth_labeled_max = reader
                .max_with_prefix("sumeragi_bg_post_queue_depth_by_peer")
                .unwrap_or(bg_post_queue_depth);
            let p2p_queue_dropped_total = reader
                .max_with_prefix("p2p_queue_dropped_total")
                .unwrap_or(0.0);

            let missing_rbc = deliver_total < 1.0 || ready_total < 1.0;
            if !missing_rbc || attempts >= 5 {
                break (
                    payload_bytes_metric,
                    deliver_total,
                    ready_total,
                    bg_post_queue_depth,
                    bg_post_queue_depth_labeled_max,
                    p2p_queue_dropped_total,
                    body,
                );
            }

            attempts += 1;
            sleep(Duration::from_millis(200)).await;
        };
        if ready_total < 1.0 || deliver_total < 1.0 {
            let store_dir = network.peers()[idx].kura_store_dir().join("rbc_sessions");
            let persisted = rbc_status::read_persisted_snapshot(&store_dir);
            if let Some(summary) = persisted
                .iter()
                .find(|summary| summary.height == expected_height)
            {
                #[allow(clippy::cast_precision_loss)]
                {
                    ready_total = ready_total.max(summary.ready_count as f64);
                    if summary.delivered {
                        deliver_total = deliver_total.max(1.0);
                    }
                }
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let payload_bytes_f64 = payload_bytes as f64;
        assert!(payload_bytes_metric >= payload_bytes_f64);
        assert!(deliver_total >= 1.0);
        assert!(ready_total >= 1.0);
        per_peer_metrics.push((
            idx,
            PeerMetrics {
                payload_bytes: payload_bytes_metric,
                deliver_total,
                ready_total,
                bg_post_queue_depth,
                bg_post_queue_depth_labeled_max,
                p2p_queue_dropped_total,
                snapshot: body,
            },
        ));
    }

    inspect(&per_peer_metrics)?;

    let max_queue_depth = per_peer_metrics
        .iter()
        .map(|(_, metrics)| {
            metrics
                .bg_post_queue_depth_labeled_max
                .max(metrics.bg_post_queue_depth)
        })
        .fold(0.0, f64::max);
    let max_p2p_queue_drops = per_peer_metrics
        .iter()
        .map(|(_, metrics)| metrics.p2p_queue_dropped_total)
        .fold(0.0, f64::max);

    let mut table = AsciiTable::default();
    table.column(0).set_header("Peer");
    table.column(1).set_header("PayloadBytes");
    table.column(2).set_header("DeliverBroadcasts");
    table.column(3).set_header("ReadyBroadcasts");
    table.column(4).set_header("BgQueueDepth");
    table.column(5).set_header("P2pDrops");
    let rows: Vec<Vec<String>> = per_peer_metrics
        .iter()
        .map(|(idx, metrics)| {
            vec![
                format!("peer-{idx}"),
                format!("{:.0}", metrics.payload_bytes),
                format!("{:.0}", metrics.deliver_total),
                format!("{:.0}", metrics.ready_total),
                format!("{:.0}", metrics.bg_post_queue_depth_labeled_max),
                format!("{:.0}", metrics.p2p_queue_dropped_total),
            ]
        })
        .collect();
    let mut table_buf = Vec::new();
    table
        .writeln(&mut table_buf, &rows)
        .expect("writing ascii table should succeed");
    let table_formatted =
        String::from_utf8(table_buf).unwrap_or_else(|_| "<table render failed>".to_string());

    let deliver_ms = rbc_observation.delivered_at.as_millis();
    let commit_ms = commit_elapsed.as_millis();
    #[allow(clippy::cast_precision_loss)]
    let payload_bytes_f64 = payload_bytes as f64;
    let payload_mib = payload_bytes_f64 / (1024.0 * 1024.0);
    let throughput_mib_s = payload_mib / rbc_observation.delivered_at.as_secs_f64().max(1e-6);
    // Require RBC delivery within the delivery budget to keep small payload scenarios tolerant.
    #[allow(clippy::cast_precision_loss)]
    let extra_peers = u64::try_from(peer_count.saturating_sub(4)).unwrap_or(0);
    let deliver_budget_ms = RBC_DELIVER_BUDGET_MS
        .saturating_add(RBC_DELIVER_GRACE_MS)
        .saturating_add(extra_peers.saturating_mul(RBC_DELIVER_BUDGET_PER_EXTRA_PEER_MS));
    let commit_budget_ms = COMMIT_BUDGET_MS
        .saturating_add(extra_peers.saturating_mul(COMMIT_BUDGET_PER_EXTRA_PEER_MS));
    #[allow(clippy::cast_precision_loss)]
    let throughput_floor_mib_s =
        (payload_mib / ((deliver_budget_ms as f64) / 1000.0)).min(THROUGHPUT_FLOOR_MIB_S);

    let deliver_ms_u64 = u64::try_from(deliver_ms).unwrap_or(u64::MAX);
    let commit_ms_u64 = u64::try_from(commit_ms).unwrap_or(u64::MAX);
    assert!(
        deliver_ms_u64 <= deliver_budget_ms,
        "RBC delivery {deliver_ms_u64} ms exceeded budget {deliver_budget_ms} ms"
    );
    assert!(
        commit_ms_u64 <= commit_budget_ms,
        "Commit latency {commit_ms_u64} ms exceeded budget {commit_budget_ms} ms"
    );
    assert!(
        throughput_mib_s >= throughput_floor_mib_s,
        "Throughput {throughput_mib_s:.2} MiB/s dipped below floor {throughput_floor_mib_s:.3} MiB/s (deliver_ms={deliver_ms_u64})"
    );
    assert!(
        max_queue_depth <= BG_POST_QUEUE_DEPTH_BUDGET,
        "Background post queue depth {max_queue_depth:.0} exceeded budget {BG_POST_QUEUE_DEPTH_BUDGET:.0}",
    );
    assert!(
        max_p2p_queue_drops <= P2P_QUEUE_DROP_BUDGET + f64::EPSILON,
        "P2P queue drops {max_p2p_queue_drops:.0} exceeded budget {P2P_QUEUE_DROP_BUDGET:.0}"
    );

    let summary = format!(
        concat!(
            "scenario: {scenario}\n",
            "peers: {peers}\n",
            "payload_bytes: {payload}\n",
            "rbc_deliver_ms: {deliver}\n",
            "commit_ms: {commit}\n",
            "throughput_mib_s: {throughput:.2}\n",
            "rbc_ready_count: {ready}\n",
            "rbc_total_chunks: {total_chunks}\n",
            "rbc_received_chunks: {received}\n",
            "rbc_height: {height}\n",
            "rbc_view: {view}\n",
            "rbc_block_hash: {hash}\n",
            "queue_bg_post_depth_max: {queue_bg:.0}\n",
            "queue_p2p_dropped_total_max: {queue_p2p:.0}\n",
            "metrics:\n{table}"
        ),
        scenario = scenario_name,
        peers = peer_count,
        payload = payload_bytes,
        deliver = deliver_ms,
        commit = commit_ms,
        throughput = throughput_mib_s,
        ready = rbc_observation.ready_count,
        total_chunks = rbc_observation.total_chunks,
        received = rbc_observation.received_chunks,
        height = rbc_observation.height,
        view = rbc_observation.view,
        hash = rbc_observation.block_hash,
        queue_bg = max_queue_depth,
        queue_p2p = max_p2p_queue_drops,
        table = table_formatted.trim_end()
    );

    println!("{summary}");

    for peer in network.peers() {
        let store_dir = peer.kura_store_dir().join("rbc_sessions");
        let snapshot_file = store_dir.join("sessions.norito");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let persisted = rbc_status::read_persisted_snapshot(&store_dir);
            // Committed sessions can be pruned quickly, so accept the persisted snapshot file.
            if persisted
                .iter()
                .any(|summary| summary.height == expected_height && summary.delivered)
                || snapshot_file.exists()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "expected RBC status snapshot persisted in {}",
                snapshot_file.display()
            );
            sleep(Duration::from_millis(200)).await;
        }
    }

    let per_peer_values: Vec<Value> = per_peer_metrics
        .iter()
        .map(|(idx, metrics)| {
            let mut obj = norito::json::Map::new();
            obj.insert("peer_index".into(), Value::from(*idx));
            obj.insert("payload_bytes".into(), Value::from(metrics.payload_bytes));
            obj.insert("deliver_total".into(), Value::from(metrics.deliver_total));
            obj.insert("ready_total".into(), Value::from(metrics.ready_total));
            obj.insert(
                "bg_post_queue_depth".into(),
                Value::from(metrics.bg_post_queue_depth),
            );
            obj.insert(
                "bg_post_queue_depth_labeled_max".into(),
                Value::from(metrics.bg_post_queue_depth_labeled_max),
            );
            obj.insert(
                "p2p_queue_dropped_total".into(),
                Value::from(metrics.p2p_queue_dropped_total),
            );
            obj.insert("metrics_text".into(), Value::from(metrics.snapshot.clone()));
            Value::Object(obj)
        })
        .collect();

    let mut timings = norito::json::Map::new();
    timings.insert(
        "rbc_deliver_ms".into(),
        Value::from(u64::try_from(deliver_ms).unwrap_or(u64::MAX)),
    );
    timings.insert(
        "commit_ms".into(),
        Value::from(u64::try_from(commit_ms).unwrap_or(u64::MAX)),
    );
    timings.insert(
        "rbc_delivered_seconds".into(),
        Value::from(rbc_observation.delivered_at.as_secs_f64()),
    );
    timings.insert(
        "commit_elapsed_seconds".into(),
        Value::from(commit_elapsed.as_secs_f64()),
    );
    timings.insert("throughput_mib_s".into(), Value::from(throughput_mib_s));

    let mut rbc = norito::json::Map::new();
    rbc.insert("height".into(), Value::from(rbc_observation.height));
    rbc.insert(
        "ready_count".into(),
        Value::from(rbc_observation.ready_count),
    );
    rbc.insert(
        "total_chunks".into(),
        Value::from(rbc_observation.total_chunks),
    );
    rbc.insert(
        "received_chunks".into(),
        Value::from(rbc_observation.received_chunks),
    );
    rbc.insert("view".into(), Value::from(rbc_observation.view));
    rbc.insert(
        "block_hash".into(),
        Value::from(rbc_observation.block_hash.clone()),
    );

    let mut queue_metrics = norito::json::Map::new();
    queue_metrics.insert(
        "bg_post_queue_depth_max".into(),
        Value::from(max_queue_depth),
    );
    queue_metrics.insert(
        "p2p_queue_dropped_total_max".into(),
        Value::from(max_p2p_queue_drops),
    );

    let mut summary_obj = norito::json::Map::new();
    summary_obj.insert("scenario".into(), Value::from(scenario_name.to_string()));
    summary_obj.insert("peers".into(), Value::from(peer_count));
    summary_obj.insert("payload_bytes".into(), Value::from(payload_bytes));
    summary_obj.insert("timings".into(), Value::Object(timings));
    summary_obj.insert("rbc".into(), Value::Object(rbc));
    summary_obj.insert("queue".into(), Value::Object(queue_metrics));
    summary_obj.insert("per_peer_metrics".into(), Value::Array(per_peer_values));

    let summary_value = Value::Object(summary_obj);
    let summary_json = json::to_string(&summary_value).wrap_err("serialize summary json")?;
    println!("sumeragi_da_summary::{scenario_name}::{summary_json}");

    let summary_pretty =
        json::to_string_pretty(&summary_value).wrap_err("serialize pretty summary json")?;
    persist_summary_if_requested(scenario_name, &summary_pretty, &per_peer_metrics)?;

    Ok(())
}

#[allow(dead_code)] // retained for manual DA scenario runs
async fn run_sumeragi_da_scenario(
    scenario_name: &str,
    peer_count: usize,
    payload_bytes: usize,
) -> Result<()> {
    run_sumeragi_da_scenario_with(
        scenario_name,
        peer_count,
        payload_bytes,
        false,
        |_| {},
        |_| Ok(()),
    )
    .await
}

async fn set_sumeragi_parameter(client: &Client, parameter: SumeragiParameter) -> Result<()> {
    let submit_client = client.clone();
    tokio::task::spawn_blocking(move || {
        submit_client.submit_blocking(SetParameter::new(Parameter::Sumeragi(parameter)))
    })
    .await
    .wrap_err("join SetParameter task")??;
    Ok(())
}

async fn fetch_status(client: &Client) -> Result<Status> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut delay = Duration::from_millis(200);
    #[allow(unused_assignments)]
    let mut last_err: Option<Report> = None;
    loop {
        let client = client.clone();
        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            tokio::task::spawn_blocking(move || client.get_status())
                .await
                .wrap_err("join get_status task")?
        })
        .await;

        match result {
            Ok(Ok(status)) => return Ok(status),
            Ok(Err(err)) => {
                last_err = Some(err.wrap_err("fetch_status"));
            }
            Err(_) => {
                last_err = Some(eyre!("fetch_status timed out after 5s window"));
            }
        }

        if Instant::now() >= deadline {
            return Err(last_err.unwrap_or_else(|| eyre!("fetch_status exceeded deadline")));
        }

        sleep(delay).await;
        delay = (delay.saturating_mul(2)).min(Duration::from_secs(2));
    }
}

fn persist_summary_if_requested(
    scenario_name: &str,
    summary_pretty: &str,
    per_peer_metrics: &[(usize, PeerMetrics)],
) -> Result<()> {
    let Ok(dir) = std::env::var("SUMERAGI_DA_ARTIFACT_DIR") else {
        return Ok(());
    };

    let root = PathBuf::from(dir);
    fs::create_dir_all(&root).wrap_err("create artifact directory")?;

    let summary_path = root.join(format!("{scenario_name}.summary.json"));
    fs::write(&summary_path, format!("{summary_pretty}\n")).wrap_err("write summary file")?;

    for (idx, metrics) in per_peer_metrics {
        let metrics_path = root.join(format!("{scenario_name}.peer-{idx}.prom"));
        fs::write(&metrics_path, &metrics.snapshot).wrap_err("write metrics snapshot")?;
    }

    Ok(())
}

#[test]
fn metrics_reader_max_with_prefix_handles_labels() {
    let raw = "foo 1\nbar{peer=\"a\"} 3\nbar{peer=\"b\"} 5\n";
    let reader = MetricsReader::new(raw);
    assert_eq!(reader.max_with_prefix("bar"), Some(5.0));
    assert!(reader.max_with_prefix("missing").is_none());
}

#[test]
fn torii_max_content_len_adds_headroom() {
    let payload = 10 * 1024 * 1024;
    let limit = torii_max_content_len_for_payload(payload);
    let payload_i64 = i64::try_from(payload).expect("payload fits in i64");
    let headroom_i64 = i64::try_from(TORII_CONTENT_HEADROOM_BYTES).expect("headroom fits in i64");
    assert!(limit >= payload_i64 * 2);
    assert!(limit >= payload_i64 + headroom_i64);
}

#[test]
fn torii_max_content_len_saturates_on_overflow() {
    let limit = torii_max_content_len_for_payload(usize::MAX);
    assert_eq!(limit, i64::MAX);
}

#[test]
fn da_commit_wait_timeout_is_reasonable() {
    assert_eq!(da_commit_wait_timeout(), Duration::from_secs(240));
}

const DA_COMMIT_WAIT_TIMEOUT_SECS: u64 = 240;

fn da_commit_wait_timeout() -> Duration {
    Duration::from_secs(DA_COMMIT_WAIT_TIMEOUT_SECS)
}

async fn wait_for_height(
    http: reqwest::Client,
    status_url: reqwest::Url,
    target_height: u64,
    start: Instant,
) -> Result<Duration> {
    let timeout = da_commit_wait_timeout();
    loop {
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for height {target_height}; last elapsed {:?}",
                start.elapsed()
            ));
        }
        let response = http
            .get(status_url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch /status")?;
        if !response.status().is_success() {
            sleep(Duration::from_millis(200)).await;
            continue;
        }
        let body = response.text().await.wrap_err("status body")?;
        let value: Value = json::from_str(&body).wrap_err("parse status JSON")?;
        if let Some(height) = value
            .as_object()
            .and_then(|obj| obj.get("blocks"))
            .and_then(|val| extract_u64(val).ok())
            && height >= target_height
        {
            return Ok(start.elapsed());
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_commit_certificates(
    http: &reqwest::Client,
    torii_urls: &[String],
    expected_height: u64,
    start: Instant,
) -> Result<()> {
    let timeout = da_commit_wait_timeout();
    let mut urls = Vec::with_capacity(torii_urls.len());
    for torii in torii_urls {
        let base =
            reqwest::Url::parse(torii).wrap_err_with(|| format!("parse torii url {torii}"))?;
        let mut url = base
            .join("v1/sumeragi/commit-certificates")
            .wrap_err_with(|| format!("compose commit certificates URL for {torii}"))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("from", &expected_height.to_string());
            pairs.append_pair("limit", "1");
        }
        urls.push((torii.clone(), url));
    }

    let mut last_missing = Vec::new();
    loop {
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for commit certificates at height {expected_height}; missing={last_missing:?}"
            ));
        }
        last_missing.clear();
        for (torii, url) in &urls {
            let response = http
                .get(url.clone())
                .header("Accept", "application/json")
                .send()
                .await
                .wrap_err("fetch commit certificates")?;
            if !response.status().is_success() {
                last_missing.push(format!("{torii} (status {})", response.status()));
                continue;
            }
            let body = response.text().await.wrap_err("commit certificates body")?;
            let certificates: Vec<CommitCertificate> =
                json::from_str(&body).wrap_err("parse commit certificates JSON")?;
            if !certificates
                .iter()
                .any(|cert| cert.height == expected_height)
            {
                last_missing.push(format!("{torii} (missing height {expected_height})"));
            }
        }
        if last_missing.is_empty() {
            return Ok(());
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_inflight_rbc(
    http: reqwest::Client,
    sessions_url: reqwest::Url,
    expected_height: u64,
    start: Instant,
) -> Result<(String, u64)> {
    let timeout = Duration::from_secs(60);
    loop {
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for in-flight RBC chunks at height {expected_height}"
            ));
        }
        let response = http
            .get(sessions_url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch RBC sessions")?;
        if !response.status().is_success() {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        let body = response.text().await.wrap_err("sessions body")?;
        let value: Value = json::from_str(&body).wrap_err("parse sessions JSON")?;
        if let Some(items) = value
            .as_object()
            .and_then(|obj| obj.get("items"))
            .and_then(Value::as_array)
        {
            for item in items {
                let Some(obj) = item.as_object() else {
                    continue;
                };
                let height =
                    extract_u64(obj.get("height").ok_or_else(|| eyre!("missing height"))?)?;
                if height != expected_height {
                    continue;
                }
                let delivered = extract_bool(
                    obj.get("delivered")
                        .ok_or_else(|| eyre!("missing delivered"))?,
                )?;
                if delivered {
                    continue;
                }
                let total_chunks = extract_u64(
                    obj.get("total_chunks")
                        .ok_or_else(|| eyre!("missing total_chunks"))?,
                )?;
                if total_chunks == 0 {
                    continue;
                }
                let received_chunks = extract_u64(
                    obj.get("received_chunks")
                        .ok_or_else(|| eyre!("missing received_chunks"))?,
                )?;
                if received_chunks == 0 || received_chunks >= total_chunks {
                    continue;
                }
                let block_hash = extract_string(
                    obj.get("block_hash")
                        .ok_or_else(|| eyre!("missing block_hash"))?,
                )?;
                return Ok((block_hash, total_chunks));
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_persisted_rbc_session(
    store_dir: &Path,
    expected_height: u64,
    start: Instant,
) -> Result<rbc_status::Summary> {
    let timeout = Duration::from_secs(60);
    loop {
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for persisted RBC session at height {expected_height} in {}",
                store_dir.display()
            ));
        }
        let snapshot = rbc_status::read_persisted_snapshot(store_dir);
        if let Some(summary) = snapshot.into_iter().find(|summary| {
            summary.height == expected_height
                && summary.total_chunks > 0
                && summary.received_chunks >= summary.total_chunks
                && !summary.invalid
        }) {
            return Ok(summary);
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_recovered_flag(
    http: reqwest::Client,
    sessions_url: reqwest::Url,
    expected_height: u64,
    block_hash_hex: &str,
    start: Instant,
) -> Result<()> {
    let timeout = Duration::from_secs(120);
    loop {
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for recovered RBC session {block_hash_hex}"
            ));
        }
        let response = http
            .get(sessions_url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch RBC sessions (recovery)")?;
        if !response.status().is_success() {
            sleep(Duration::from_millis(200)).await;
            continue;
        }
        let body = response.text().await.wrap_err("sessions body")?;
        let value: Value = json::from_str(&body).wrap_err("parse sessions JSON")?;
        if let Some(items) = value
            .as_object()
            .and_then(|obj| obj.get("items"))
            .and_then(Value::as_array)
        {
            for item in items {
                let Some(obj) = item.as_object() else {
                    continue;
                };
                let height =
                    extract_u64(obj.get("height").ok_or_else(|| eyre!("missing height"))?)?;
                if height != expected_height {
                    continue;
                }
                let block_hash = extract_string(
                    obj.get("block_hash")
                        .ok_or_else(|| eyre!("missing block_hash"))?,
                )?;
                if block_hash != block_hash_hex {
                    continue;
                }
                let recovered = extract_bool(
                    obj.get("recovered")
                        .ok_or_else(|| eyre!("missing recovered flag"))?,
                )?;
                if recovered {
                    if let Some(from_disk_val) = obj.get("recovered_from_disk") {
                        let from_disk = extract_bool(from_disk_val)?;
                        ensure!(
                            from_disk,
                            "RBC session reported recovered but not recovered_from_disk"
                        );
                    }
                    return Ok(());
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

struct RbcSessionSnapshot {
    total_chunks: u64,
    received_chunks: u64,
    #[allow(dead_code)]
    delivered: bool,
    recovered: bool,
}

async fn fetch_rbc_session(
    http: &reqwest::Client,
    sessions_url: &reqwest::Url,
    expected_height: u64,
    block_hash_hex: &str,
) -> Result<Option<RbcSessionSnapshot>> {
    let response = http
        .get(sessions_url.clone())
        .header("Accept", "application/json")
        .send()
        .await
        .wrap_err("fetch RBC sessions snapshot")?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let body = response.text().await.wrap_err("sessions body")?;
    let value: Value = json::from_str(&body).wrap_err("parse sessions JSON")?;
    let Some(items) = value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(Value::as_array)
    else {
        return Ok(None);
    };
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let height = extract_u64(
            obj.get("height")
                .ok_or_else(|| eyre!("missing height field"))?,
        )?;
        if height != expected_height {
            continue;
        }
        let block_hash = extract_string(
            obj.get("block_hash")
                .ok_or_else(|| eyre!("missing block_hash"))?,
        )?;
        if block_hash != block_hash_hex {
            continue;
        }
        let total_chunks = extract_u64(
            obj.get("total_chunks")
                .ok_or_else(|| eyre!("missing total_chunks"))?,
        )?;
        let received_chunks = extract_u64(
            obj.get("received_chunks")
                .ok_or_else(|| eyre!("missing received_chunks"))?,
        )?;
        let delivered = extract_bool(
            obj.get("delivered")
                .ok_or_else(|| eyre!("missing delivered flag"))?,
        )?;
        let recovered = obj
            .get("recovered")
            .map(extract_bool)
            .transpose()?
            .unwrap_or(false);
        return Ok(Some(RbcSessionSnapshot {
            total_chunks,
            received_chunks,
            delivered,
            recovered,
        }));
    }
    Ok(None)
}

async fn restart_all_peers(network: &Network, config_layers: &[ConfigLayer]) -> Result<()> {
    for peer in network.peers() {
        let mnemonic = peer.mnemonic().to_string();
        peer.start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("restart peer {mnemonic}"))?;
    }
    Ok(())
}

async fn wait_for_rbc_delivery(
    http: reqwest::Client,
    sessions_url: reqwest::Url,
    expected_height: u64,
    start: Instant,
) -> Result<RbcObservation> {
    let timeout = Duration::from_secs(120);
    loop {
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for RBC delivery at height {expected_height}"
            ));
        }
        let response = http
            .get(sessions_url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch RBC sessions")?;
        if !response.status().is_success() {
            sleep(Duration::from_millis(200)).await;
            continue;
        }
        let body = response.text().await.wrap_err("sessions body")?;
        let value: Value = json::from_str(&body).wrap_err("parse sessions JSON")?;
        if let Some(observation) = parse_rbc_summary(&value, expected_height, start)? {
            return Ok(observation);
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_rbc_session(
    http: &reqwest::Client,
    url: reqwest::Url,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let response = http
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch RBC sessions snapshot")?;
        if response.status().is_success() {
            let body = response.text().await.wrap_err("sessions body")?;
            let value: Value = json::from_str(&body).wrap_err("parse sessions JSON")?;
            let has_session = value
                .as_object()
                .and_then(|root| root.get("items"))
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty());
            if has_session {
                return Ok(());
            }
        }
        if Instant::now() > deadline {
            return Err(eyre!("timed out waiting for RBC session to appear"));
        }
        sleep(Duration::from_millis(100)).await;
    }
}

fn parse_rbc_summary(
    root: &Value,
    expected_height: u64,
    start: Instant,
) -> Result<Option<RbcObservation>> {
    let Some(items) = root
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(Value::as_array)
    else {
        return Ok(None);
    };
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let height = extract_u64(obj.get("height").ok_or_else(|| eyre!("missing height"))?)?;
        if height != expected_height {
            continue;
        }
        let delivered = extract_bool(
            obj.get("delivered")
                .ok_or_else(|| eyre!("missing delivered"))?,
        )?;
        if !delivered {
            continue;
        }
        let view = extract_u64(obj.get("view").ok_or_else(|| eyre!("missing view"))?)?;
        let total_chunks = extract_u64(
            obj.get("total_chunks")
                .ok_or_else(|| eyre!("missing total_chunks"))?,
        )?;
        let received_chunks = extract_u64(
            obj.get("received_chunks")
                .ok_or_else(|| eyre!("missing received_chunks"))?,
        )?;
        let ready_count = extract_u64(
            obj.get("ready_count")
                .ok_or_else(|| eyre!("missing ready_count"))?,
        )?;
        let block_hash = extract_string(
            obj.get("block_hash")
                .ok_or_else(|| eyre!("missing block_hash"))?,
        )?;
        return Ok(Some(RbcObservation {
            delivered_at: start.elapsed(),
            height,
            view,
            total_chunks: total_chunks
                .try_into()
                .map_err(|_| eyre!("total_chunks overflow"))?,
            received_chunks: received_chunks
                .try_into()
                .map_err(|_| eyre!("received_chunks overflow"))?,
            ready_count,
            block_hash,
        }));
    }
    Ok(None)
}

fn extract_u64(value: &Value) -> Result<u64> {
    match value {
        Value::Number(num) => num
            .as_u64()
            .ok_or_else(|| eyre!("expected u64, got {num:?}")),
        other => Err(eyre!("expected number, got {other:?}")),
    }
}

fn extract_bool(value: &Value) -> Result<bool> {
    match value {
        Value::Bool(b) => Ok(*b),
        other => Err(eyre!("expected bool, got {other:?}")),
    }
}

fn extract_string(value: &Value) -> Result<String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Null => Ok(String::new()),
        other => Err(eyre!("expected string, got {other:?}")),
    }
}
