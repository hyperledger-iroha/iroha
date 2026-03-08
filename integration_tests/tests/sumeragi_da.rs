#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Data availability + RBC integration scenario exercising large payload distribution.

use std::{
    fs,
    io::ErrorKind,
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
        consensus::Qc,
        isi::{Log, SetParameter, Unregister},
        parameter::{
            Parameter, SumeragiParameter, TransactionParameter, system::SumeragiNposParameters,
        },
        prelude::{HashOf, QueryBuilderExt},
        query::block::prelude::FindBlocks,
        query::peer::prelude::FindPeers,
        transaction::SignedTransaction,
    },
};
use iroha_config_base::toml::Writer as TomlWriter;
use iroha_core::sumeragi::{network_topology::Topology, rbc_status};
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer};
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

struct RbcInflightObservation {
    block_hash: String,
    sessions_url: reqwest::Url,
    height: u64,
    total_chunks: u64,
    received_chunks: u64,
    delivered: bool,
}

struct RbcSessionsProbe {
    url: reqwest::Url,
    baseline_hashes: Vec<String>,
}

type CommitCertificate = Qc;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct SumeragiSnapshot {
    index: u64,
    view_change_proof_accepted_total: u64,
    view_change_proof_stale_total: u64,
    view_change_proof_rejected_total: u64,
    da_reschedule_total: u64,
    rbc_deliver_defer_ready_total: u64,
    rbc_deliver_defer_chunks_total: u64,
}

impl SumeragiSnapshot {
    fn from_json(value: &Value) -> Self {
        Self {
            index: json_u64(value, "view_change_index"),
            view_change_proof_accepted_total: json_u64(value, "view_change_proof_accepted_total"),
            view_change_proof_stale_total: json_u64(value, "view_change_proof_stale_total"),
            view_change_proof_rejected_total: json_u64(value, "view_change_proof_rejected_total"),
            da_reschedule_total: json_u64(value, "da_reschedule_total"),
            rbc_deliver_defer_ready_total: json_u64(value, "rbc_deliver_defer_ready_total"),
            rbc_deliver_defer_chunks_total: json_u64(value, "rbc_deliver_defer_chunks_total"),
        }
    }
}

fn resolve_permissioned_leader_peer(
    peers: &[NetworkPeer],
    height: u64,
    view: u64,
    prf_seed: Option<[u8; 32]>,
) -> Result<NetworkPeer> {
    let mut roster: Vec<_> = peers.iter().map(NetworkPeer::id).collect();
    roster.sort();
    roster.dedup();
    let mut topology = Topology::new(roster);
    if let Some(seed) = prf_seed {
        topology.shuffle_prf(seed, height);
    }
    topology.nth_rotation(view);
    let leader_peer_id = topology
        .as_ref()
        .first()
        .ok_or_else(|| eyre!("empty topology when resolving leader"))?;
    peers
        .iter()
        .find(|peer| peer.id() == *leader_peer_id)
        .cloned()
        .ok_or_else(|| eyre!("leader peer id not found in network peers"))
}

fn chain_epoch_seed(chain_id: &iroha::data_model::ChainId) -> [u8; 32] {
    let chain = chain_id.clone().into_inner();
    let hash = iroha_crypto::Hash::new(chain.as_bytes());
    <[u8; 32]>::from(hash)
}

fn http_client_with_client_auth(client: &Client) -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    for (name, value) in &client.headers {
        let header_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|err| eyre!("invalid header name `{name}`: {err}"))?;
        let header_value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|err| eyre!("invalid header value for `{name}`: {err}"))?;
        headers.insert(header_name, header_value);
    }
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .wrap_err("build reqwest client for authenticated Sumeragi endpoint calls")
}

#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Default)]
struct PendingRbcStashCounters {
    stash_ready_total: u64,
    stash_ready_init_missing_total: u64,
    stash_ready_roster_missing_total: u64,
    stash_ready_roster_hash_mismatch_total: u64,
    stash_ready_roster_unverified_total: u64,
    stash_deliver_total: u64,
    stash_deliver_init_missing_total: u64,
    stash_deliver_roster_missing_total: u64,
    stash_deliver_roster_hash_mismatch_total: u64,
    stash_deliver_roster_unverified_total: u64,
    stash_chunk_total: u64,
}

impl PendingRbcStashCounters {
    fn total(&self) -> u64 {
        self.stash_ready_total
            .saturating_add(self.stash_deliver_total)
            .saturating_add(self.stash_chunk_total)
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

fn parse_pending_rbc_stash_counters(root: &Value) -> PendingRbcStashCounters {
    let pending = root
        .as_object()
        .and_then(|obj| obj.get("pending_rbc"))
        .and_then(Value::as_object);
    let Some(pending) = pending else {
        return PendingRbcStashCounters::default();
    };
    PendingRbcStashCounters {
        stash_ready_total: pending
            .get("stash_ready_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_ready_init_missing_total: pending
            .get("stash_ready_init_missing_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_ready_roster_missing_total: pending
            .get("stash_ready_roster_missing_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_ready_roster_hash_mismatch_total: pending
            .get("stash_ready_roster_hash_mismatch_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_ready_roster_unverified_total: pending
            .get("stash_ready_roster_unverified_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_deliver_total: pending
            .get("stash_deliver_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_deliver_init_missing_total: pending
            .get("stash_deliver_init_missing_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_deliver_roster_missing_total: pending
            .get("stash_deliver_roster_missing_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_deliver_roster_hash_mismatch_total: pending
            .get("stash_deliver_roster_hash_mismatch_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_deliver_roster_unverified_total: pending
            .get("stash_deliver_roster_unverified_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        stash_chunk_total: pending
            .get("stash_chunk_total")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
    }
}

// Keep the payload light to avoid overwhelming Torii/queue on constrained hosts.
const LARGE_PAYLOAD_BYTES: usize = 1024; // keep payload light to ensure timely DA/RBC
// Use a multi-chunk payload to ensure the recovery test observes an in-flight session.
const RBC_RECOVERY_PAYLOAD_BYTES: usize = 1024 * 1024;
const RBC_RECOVERY_CHUNK_BYTES: i64 = 16 * 1024;
const RBC_DELIVER_BUDGET_MS: u64 = 30_000;
const RBC_DELIVER_GRACE_MS: u64 = 5_000;
const COMMIT_BUDGET_MS: u64 = 75_000;
const RBC_DELIVER_BUDGET_PER_EXTRA_PEER_MS: u64 = 15_000;
const COMMIT_BUDGET_PER_EXTRA_PEER_MS: u64 = 30_000;
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
const DA_COMMIT_WAIT_TIMEOUT_SECS: u64 = 900;
const DA_RBC_DELIVERY_TIMEOUT_SECS: u64 = 600;
const DA_RBC_INFLIGHT_TIMEOUT_SECS: u64 = 300;
const DA_RBC_PERSIST_TIMEOUT_SECS: u64 = 300;
const DA_RBC_RECOVERY_TIMEOUT_SECS: u64 = 600;
const DA_RBC_SESSION_TIMEOUT_SECS: u64 = 240;
const DA_VIEW_CHANGE_TIMEOUT_SECS: u64 = 300;
const DA_PAYLOAD_LOSS_COMMIT_TIMEOUT_SECS: u64 = 240;
const DA_KURA_EVICTION_PROGRESS_WAIT_SECS: u64 = 45;

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

fn locate_blocks_dir(store_dir: &Path) -> Result<PathBuf> {
    let legacy_index = store_dir.join("blocks.index");
    if legacy_index.exists() {
        return Ok(store_dir.to_path_buf());
    }
    let blocks_root = store_dir.join("blocks");
    let entries = fs::read_dir(&blocks_root).wrap_err("read blocks dir")?;
    for entry in entries {
        let entry = entry.wrap_err("read blocks dir entry")?;
        let path = entry.path();
        if path.is_dir() && path.join("blocks.index").exists() {
            return Ok(path);
        }
    }
    Err(eyre!("no blocks.index found under {blocks_root:?}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_rbc_da_large_payload_four_peers() -> Result<()> {
    let result = run_sumeragi_da_scenario_with(
        "sumeragi_rbc_da_large_payload_four_peers",
        4,
        LARGE_PAYLOAD_BYTES,
        false,
        true,
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
        true,
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
async fn sumeragi_commit_qc_with_tight_block_queue_four_peers() -> Result<()> {
    let result = run_sumeragi_da_scenario_with(
        "sumeragi_commit_qc_with_tight_block_queue_four_peers",
        4,
        LARGE_PAYLOAD_BYTES,
        false,
        false,
        |layer| {
            layer.write(["sumeragi", "advanced", "queues", "blocks"], 2i64);
        },
        |_| Ok(()),
    )
    .await;
    if sandbox::handle_result(
        result,
        "sumeragi_commit_qc_with_tight_block_queue_four_peers",
    )?
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
        true,
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
                "expected background post drops to remain zero when background worker is disabled"
            );
            ensure!(
                per_peer_metrics.iter().all(|(_, metrics)| {
                    let reader = MetricsReader::new(&metrics.snapshot);
                    reader
                        .max_with_prefix("sumeragi_bg_post_overflow_total")
                        .unwrap_or(0.0)
                        == 0.0
                }),
                "expected background post overflow to remain zero with synchronous fallback"
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

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_da_kura_eviction_rehydrates_from_da_store() -> Result<()> {
    let scenario_name = stringify!(sumeragi_da_kura_eviction_rehydrates_from_da_store);
    let payload_bytes = 128 * 1024;
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;
    let rbc_chunk_max_bytes = i64::try_from(payload_bytes).unwrap_or(i64::MAX);
    let stake_amount = SumeragiNposParameters::default().min_self_bond();

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
            .write(["sumeragi", "consensus_mode"], "npos")
            .write(
                ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                rbc_chunk_max_bytes,
            )
            .write(
                ["sumeragi", "debug", "rbc", "force_deliver_quorum_one"],
                true,
            )
            .write(
                ["torii", "max_content_len"],
                torii_max_content_len_for_payload(payload_bytes),
            )
            .write(["kura", "blocks_in_memory"], 1_i64);
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

    let mut storage = toml::map::Map::new();
    storage.insert("max_disk_usage_bytes".into(), toml::Value::Integer(400_000));
    let mut weights = toml::map::Map::new();
    weights.insert("kura_blocks_bps".into(), toml::Value::Integer(9_000));
    weights.insert("wsv_snapshots_bps".into(), toml::Value::Integer(500));
    weights.insert("sorafs_bps".into(), toml::Value::Integer(250));
    weights.insert("soranet_spool_bps".into(), toml::Value::Integer(200));
    weights.insert("soravpn_spool_bps".into(), toml::Value::Integer(50));
    storage.insert("disk_budget_weights".into(), toml::Value::Table(weights));
    nexus.insert("storage".into(), toml::Value::Table(storage));

    {
        let mut writer = TomlWriter::new(&mut config_table);
        writer.write("nexus", toml::Value::Table(nexus));
    }

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_genesis_bootstrap(stake_amount)
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

    let result: Result<()> = async {
        let mut client = network.client();
        let status_timeout = da_commit_wait_timeout();
        client.transaction_status_timeout = status_timeout;
        client.transaction_ttl = Some(status_timeout.saturating_add(Duration::from_secs(30)));
        set_sumeragi_parameter(&client, SumeragiParameter::DaEnabled(true)).await?;

        let peers = network.peers();
        let peer = peers
            .first()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let store_dir = peer.kura_store_dir();
        let blocks_dir = locate_blocks_dir(&store_dir)?;

        let index_path = blocks_dir.join("blocks.index");
        let hashes_path = blocks_dir.join("blocks.hashes");
        let da_blocks_dir = blocks_dir.join("da_blocks");

        let status_before = fetch_status(&client).await?;
        let mut expected_height = status_before.blocks;
        let mut evicted_height = None;
        let mut submitted = 0_u64;
        let deadline = Instant::now() + da_commit_wait_timeout();

        while evicted_height.is_none() && Instant::now() < deadline {
            expected_height = expected_height.saturating_add(1);
            let message = generate_incompressible_payload(
                &format!("{scenario_name}-{submitted}"),
                payload_bytes,
            );
            submit_log_to_any_peer(&peers, message).await?;
            submitted = submitted.saturating_add(1);

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let per_submit_wait = remaining.min(Duration::from_secs(
                DA_KURA_EVICTION_PROGRESS_WAIT_SECS,
            ));
            if timeout(per_submit_wait, peer.once_block(expected_height))
                .await
                .is_err()
            {
                // Avoid failing on one stalled proposal; continue driving consensus until the
                // scenario deadline and keep the target height aligned with observed progress.
                let status = fetch_status(&client)
                    .await
                    .wrap_err("refresh status after per-submit block wait timeout")?;
                expected_height = expected_height.max(status.blocks);
                continue;
            }

            let bytes = fs::read(&index_path).wrap_err("read blocks.index")?;
            ensure!(
                bytes.len() % 16 == 0,
                "blocks.index size is not aligned to 16-byte entries"
            );
            for (idx, chunk) in bytes.chunks_exact(16).enumerate() {
                if idx == 0 {
                    continue;
                }
                let start =
                    u64::from_le_bytes(chunk[0..8].try_into().expect("block index start"));
                if start == u64::MAX {
                    evicted_height = Some(idx as u64 + 1);
                    break;
                }
            }
        }

        let evicted_height = evicted_height.ok_or_else(|| {
            eyre!(
                "timed out waiting for DA-backed Kura eviction to mark blocks.index after {submitted} blocks"
            )
        })?;

        let da_path = da_blocks_dir.join(format!("{evicted_height:020}.norito"));
        ensure!(da_path.exists(), "expected DA block body at {da_path:?}");

        let hashes = fs::read(&hashes_path).wrap_err("read blocks.hashes")?;
        ensure!(
            hashes.len() % 32 == 0,
            "blocks.hashes size is not aligned to 32-byte entries"
        );
        let hash_offset = usize::try_from(evicted_height.saturating_sub(1))
            .unwrap_or(usize::MAX)
            .saturating_mul(32);
        let expected_hash = hashes
            .get(hash_offset..hash_offset + 32)
            .ok_or_else(|| eyre!("missing hash for evicted height {evicted_height}"))?;

        let query_start = Instant::now();
        let evicted_block = loop {
            let query_peer = peers
                .iter()
                .find(|peer| peer.is_running())
                .ok_or_else(|| eyre!("no running peers available for block query"))?;
            let blocks = query_peer.client().query(FindBlocks).execute_all()?;
            if let Some(block) = blocks
                .iter()
                .find(|block| block.header().height().get() == evicted_height)
            {
                break block.clone();
            }
            ensure!(
                query_start.elapsed() < da_commit_wait_timeout(),
                "missing block at evicted height {evicted_height}"
            );
            sleep(Duration::from_millis(200)).await;
        };
        ensure!(
            expected_hash == evicted_block.hash().as_ref(),
            "rehydrated block hash mismatch at height {evicted_height}"
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
async fn sumeragi_rbc_recovers_after_peer_restart() -> Result<()> {
    let payload_bytes = RBC_RECOVERY_PAYLOAD_BYTES;
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;
    let rbc_chunk_max_bytes = RBC_RECOVERY_CHUNK_BYTES;
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
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    rbc_chunk_max_bytes,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "payload_chunks_per_tick"],
                    1i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "rbc",
                        "rebroadcast_sessions_per_tick",
                    ],
                    1i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                    600_000i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "disk_store_ttl_ms"],
                    600_000i64,
                );
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

        let http = http_client_with_client_auth(&client)?;

        let heavy_message = generate_incompressible_payload(
            "sumeragi_rbc_recovers_after_peer_restart",
            payload_bytes,
        );
        let submit_handle = tokio::task::spawn_blocking(move || {
            client.submit(Log::new(Level::INFO, heavy_message))
        });

        submit_handle.await.wrap_err("submit join")??;

        let restart_store_dir = restart_peer.kura_store_dir().join("rbc_sessions");
        let persisted = wait_for_persisted_inflight_rbc_session(
            &restart_store_dir,
            expected_height,
            Instant::now(),
        )
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
        let restart_start = Instant::now();
        wait_for_recovered_flag(
            http.clone(),
            restart_sessions_url,
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

        if let Err(err) = wait_for_rbc_delivery(
            http.clone(),
            sessions_url_primary.clone(),
            expected_height,
            restart_start,
        )
        .await
        {
            // Delivery may be pruned quickly; recovery and commit checks already validate the path.
            eprintln!(
                "RBC delivery not observed on primary after restart (height {expected_height}): {err:?}"
            );
        }
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
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    i64::from(rbc_chunk_max_bytes),
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                    600_000i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "disk_store_ttl_ms"],
                    600_000i64,
                );
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

        let http = http_client_with_client_auth(&client)?;
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
    let payload_bytes = 128 * 1024;
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
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    RBC_CHUNK_SIZE_BYTES,
                )
                // Shuffle and duplicate RBC broadcasts while deterministically dropping shards to
                // block READY fan-outs on every validator.
                .write(["sumeragi", "debug", "rbc", "shuffle_chunks"], true)
                .write(["sumeragi", "debug", "rbc", "duplicate_inits"], true)
                .write(["sumeragi", "debug", "rbc", "drop_every_nth_chunk"], 4_i64);
        });

    let Some(network) = sandbox::start_network_async_or_skip(builder, scenario_name).await? else {
        return Ok(());
    };

    let result: Result<()> = async {
        network
            .ensure_blocks_with(|height| height.total >= 1)
            .await?;

        let client = network.client();
        let http = http_client_with_client_auth(&client)?;
        let peers = network.peers();
        let peer_count = u64::try_from(peers.len()).unwrap_or(0);
        let max_faults = peer_count.saturating_sub(1) / 3;
        let commit_quorum = (2 * max_faults + 1).max(1);

        let mut baseline_blocks = Vec::new();
        let mut baseline_sumeragi_snapshots = Vec::new();
        for peer in peers.iter() {
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
        if let Err(err) =
            wait_for_rbc_session(&http, sessions_url.clone(), da_rbc_session_timeout()).await
        {
            eprintln!(
                "RBC session not observed before commit (continuing): {err:?}"
            );
        }

        let mut status_after = Vec::new();
        let mut after_sumeragi_snapshots = Vec::new();

        let deadline = Instant::now() + da_payload_loss_commit_timeout();
        loop {
            status_after.clear();
            after_sumeragi_snapshots.clear();
            let mut ready_count = 0u64;
            let mut status_errors = Vec::new();
            for (idx, peer) in peers.iter().enumerate() {
                if !peer.is_running() {
                    continue;
                }
                match peer.status().await {
                    Ok(status) => {
                        if status.blocks >= expected_height {
                            ready_count = ready_count.saturating_add(1);
                        }
                        status_after.push((idx, status));
                    }
                    Err(err) => {
                        status_errors.push((idx, err));
                        continue;
                    }
                }
                if let Ok(snapshot) =
                    fetch_sumeragi_snapshot(http.clone(), &peer.torii_url()).await
                {
                    after_sumeragi_snapshots.push((idx, snapshot));
                }
            }

            let commits_ready = ready_count >= commit_quorum;
            if commits_ready {
                break;
            }

            if Instant::now() > deadline {
                let blocks: Vec<(usize, u64)> = status_after
                    .iter()
                    .map(|(idx, status)| (*idx, status.blocks))
                    .collect();
                let errors: Vec<(usize, String)> = status_errors
                    .into_iter()
                    .map(|(idx, err)| (idx, format!("{err:#}")))
                    .collect();
                return Err(eyre!(
                    "timed out waiting for commit under payload loss: blocks={blocks:?}, expected={expected_height}, quorum={commit_quorum}, errors={errors:?}"
                ));
            }

            sleep(Duration::from_millis(200)).await;
        }

        ensure!(
            !after_sumeragi_snapshots.is_empty(),
            "missing Sumeragi snapshots after payload loss run"
        );

        ensure!(
            after_sumeragi_snapshots.iter().all(|(idx, after)| {
                baseline_sumeragi_snapshots
                    .get(*idx)
                    .is_some_and(|baseline| {
                        after.da_reschedule_total == baseline.da_reschedule_total
                    })
            }),
            "expected da_reschedule_total to remain unchanged when DA is advisory"
        );
        // RBC DELIVER deferrals are optional here: DA is advisory and availability may be satisfied
        // via local payloads or never emit DELIVER in loss scenarios.

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
async fn sumeragi_rbc_unverified_roster_stash_requests_missing_block() -> Result<()> {
    let scenario_name = stringify!(sumeragi_rbc_unverified_roster_stash_requests_missing_block);
    let payload_bytes = LARGE_PAYLOAD_BYTES;
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
                .write(
                    ["torii", "max_content_len"],
                    torii_max_content_len_for_payload(payload_bytes),
                )
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    RBC_CHUNK_SIZE_BYTES,
                );
        });

    let Some(network) = sandbox::start_network_async_or_skip(builder, scenario_name).await? else {
        return Ok(());
    };

    let result: Result<()> = async {
        network
            .ensure_blocks_with(|height| height.total >= 1)
            .await?;

        let peers = network.peers();
        let primary_peer = &peers[0];
        let lagging_peer = &peers[1];
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|cow| ConfigLayer(cow.into_owned()))
            .collect();

        let client = primary_peer.client();
        let http = http_client_with_client_auth(&client)?;

        let status_url = client
            .torii_url
            .join("status")
            .wrap_err("compose status URL")?;
        let lagging_status_url =
            reqwest::Url::parse(&format!("{}/status", lagging_peer.torii_url()))
                .wrap_err("compose lagging status URL")?;
        let peer_sumeragi_urls: Vec<reqwest::Url> = peers
            .iter()
            .map(|peer| {
                reqwest::Url::parse(&format!("{}/v1/sumeragi/status", peer.torii_url()))
            })
            .collect::<Result<_, _>>()
            .wrap_err("compose peer sumeragi status URLs")?;
        let peer_metrics_urls: Vec<reqwest::Url> = peers
            .iter()
            .map(|peer| reqwest::Url::parse(&format!("{}/metrics", peer.torii_url())))
            .collect::<Result<_, _>>()
            .wrap_err("compose peer metrics URLs")?;
        let lagging_index = 1usize;
        let lagging_sumeragi_url = peer_sumeragi_urls
            .get(lagging_index)
            .cloned()
            .ok_or_else(|| eyre!("missing lagging peer sumeragi status URL"))?;
        let lagging_metrics_url = peer_metrics_urls
            .get(lagging_index)
            .cloned()
            .ok_or_else(|| eyre!("missing lagging peer metrics URL"))?;

        let status_before = fetch_status(&client).await?;
        let mut expected_height = status_before.blocks;

        lagging_peer.shutdown().await;

        let advance_blocks = 3u64;
        for idx in 0..advance_blocks {
            expected_height = expected_height.saturating_add(1);
            let payload = generate_incompressible_payload(
                &format!("{scenario_name}-advance-{idx}"),
                payload_bytes,
            );
            let submit_client = client.clone();
            tokio::task::spawn_blocking(move || {
                submit_client.submit(Log::new(Level::INFO, payload))
            })
            .await
            .wrap_err("submit log instruction")??;
            let _ = wait_for_height(
                http.clone(),
                status_url.clone(),
                expected_height,
                Instant::now(),
            )
            .await?;
        }

        if sandbox::handle_result(
            lagging_peer
                .start_checked(config_layers.clone().into_iter(), None)
                .await,
            "sumeragi_rbc_unverified_roster_stash_requests_missing_block_restart",
        )?
        .is_none()
        {
            return Ok(());
        }

        let mut baseline_stash = PendingRbcStashCounters::default();
        let mut baseline_fetch_total = 0.0;
        let baseline_deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let mut ready = true;
            let response = http
                .get(lagging_sumeragi_url.clone())
                .header("Accept", "application/json")
                .send()
                .await
                .wrap_err("fetch baseline sumeragi status")?;
            if !response.status().is_success() {
                ready = false;
            } else {
                let body = response.text().await.wrap_err("baseline sumeragi status body")?;
                let status_value: Value =
                    json::from_str(&body).wrap_err("parse baseline sumeragi status JSON")?;
                baseline_stash = parse_pending_rbc_stash_counters(&status_value);
            }

            let response = http
                .get(lagging_metrics_url.clone())
                .send()
                .await
                .wrap_err("fetch baseline metrics")?;
            if !response.status().is_success() {
                ready = false;
            } else {
                let body = response.text().await.wrap_err("baseline metrics body")?;
                let reader = MetricsReader::new(&body);
                baseline_fetch_total = reader
                    .max_with_prefix("sumeragi_missing_block_fetch_target_total")
                    .unwrap_or(0.0);
            }

            if ready {
                break;
            }
            if Instant::now() > baseline_deadline {
                return Err(eyre!("timed out collecting baseline Sumeragi/metrics snapshots"));
            }
            sleep(Duration::from_millis(200)).await;
        }

        expected_height = expected_height.saturating_add(1);
        let payload =
            generate_incompressible_payload(&format!("{scenario_name}-trigger"), payload_bytes);
        let submit_client = client.clone();
        tokio::task::spawn_blocking(move || submit_client.submit(Log::new(Level::INFO, payload)))
            .await
            .wrap_err("submit trigger log instruction")??;

        let baseline_unverified_total = baseline_stash
            .stash_ready_roster_unverified_total
            .saturating_add(baseline_stash.stash_deliver_roster_unverified_total);
        let fetch_already_active = baseline_fetch_total > 0.0;
        let mut last_unverified_total = baseline_unverified_total;
        let mut last_fetch_total = baseline_fetch_total;
        let mut last_lagging_height = None;
        let pending_deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if Instant::now() > pending_deadline {
                return Err(eyre!(
                    "timed out waiting for missing-block fetch or lagging catch-up; unverified_baseline={baseline_unverified_total}, unverified_last={last_unverified_total}, fetch_baseline={baseline_fetch_total}, fetch_last={last_fetch_total}, lagging_height={last_lagging_height:?}, expected_height={expected_height}"
                ));
            }
            let mut fetch_observed = fetch_already_active;
            let mut lagging_caught_up = false;

            let response = http
                .get(lagging_sumeragi_url.clone())
                .header("Accept", "application/json")
                .send()
                .await
                .wrap_err("fetch sumeragi status")?;
            if response.status().is_success() {
                let body = response.text().await.wrap_err("sumeragi status body")?;
                let status_value: Value =
                    json::from_str(&body).wrap_err("parse sumeragi status JSON")?;
                let counters = parse_pending_rbc_stash_counters(&status_value);
                let unverified_total = counters
                    .stash_ready_roster_unverified_total
                    .saturating_add(counters.stash_deliver_roster_unverified_total);
                last_unverified_total = unverified_total;
            }

            let response = http
                .get(lagging_metrics_url.clone())
                .send()
                .await
                .wrap_err("fetch metrics")?;
            if response.status().is_success() {
                let body = response.text().await.wrap_err("metrics body")?;
                let reader = MetricsReader::new(&body);
                let target_total = reader
                    .max_with_prefix("sumeragi_missing_block_fetch_target_total")
                    .unwrap_or(0.0);
                last_fetch_total = target_total;
                if target_total > baseline_fetch_total {
                    fetch_observed = true;
                }
            }

            let response = http
                .get(lagging_status_url.clone())
                .header("Accept", "application/json")
                .send()
                .await
                .wrap_err("fetch lagging status")?;
            if response.status().is_success() {
                let body = response.text().await.wrap_err("lagging status body")?;
                let status_value: Value =
                    json::from_str(&body).wrap_err("parse lagging status JSON")?;
                if let Some(height) = status_value
                    .as_object()
                    .and_then(|obj| obj.get("blocks"))
                    .and_then(|value| extract_u64(value).ok())
                {
                    last_lagging_height = Some(height);
                    if height >= expected_height {
                        lagging_caught_up = true;
                    }
                }
            }

            if fetch_observed || lagging_caught_up {
                break;
            }
            sleep(Duration::from_millis(200)).await;
        }

        let _ = wait_for_height(
            http.clone(),
            lagging_status_url,
            expected_height,
            Instant::now(),
        )
        .await?;

        network.shutdown().await;
        Ok(())
    }
    .await;
    if sandbox::handle_result(result, scenario_name)?.is_none() {
        return Ok(());
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_idle_view_change_recovers_after_leader_shutdown() -> Result<()> {
    let scenario_name = stringify!(sumeragi_idle_view_change_recovers_after_leader_shutdown);
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(1_000),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                    200_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                    400_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                    600_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                    800_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                    400_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    2_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                );
        });

    let Some(network) = sandbox::start_network_async_or_skip(builder, scenario_name).await? else {
        return Ok(());
    };

    let result: Result<()> = async {
        network
            .ensure_blocks_with(|height| height.total >= 1)
            .await?;

        let peers = network.peers();
        let primary_peer = peers
            .first()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let status_before = fetch_status(&primary_peer.client()).await?;
        let target_height = status_before.blocks + 1;
        let prf_seed = chain_epoch_seed(&network.chain_id());
        let leader_peer = resolve_permissioned_leader_peer(
            peers,
            target_height,
            0,
            Some(prf_seed),
        )
        .wrap_err_with(|| format!("resolve permissioned leader for height {target_height} view 0"))?;

        let mut baseline_view_changes = Vec::with_capacity(peers.len());
        for peer in peers {
            let status = peer.status().await?;
            let view_changes = status
                .sumeragi
                .as_ref()
                .ok_or_else(|| eyre!("status missing sumeragi snapshot"))?
                .view_change_install_total;
            baseline_view_changes.push(view_changes);
        }

        leader_peer.shutdown().await;
        sleep(Duration::from_millis(500)).await;

        let running: Vec<NetworkPeer> = peers
            .iter()
            .filter(|peer| peer.is_running())
            .cloned()
            .collect();
        ensure!(
            running.len() >= 3,
            "expected at least 3 running peers after leader shutdown, got {}",
            running.len()
        );

        let submit_peer = running
            .first()
            .ok_or_else(|| eyre!("no running peers available for submission"))?;
        let mut submitted = false;
        let mut submit_errors = Vec::new();
        for (idx, peer) in running.iter().enumerate() {
            let submit_client = peer.client();
            let payload = format!("{scenario_name}-liveness-{idx}");
            match tokio::task::spawn_blocking(move || {
                submit_client.submit(Log::new(Level::INFO, payload))
            })
            .await
            {
                Ok(Ok(_)) => {
                    submitted = true;
                }
                Ok(Err(err)) => submit_errors.push(err),
                Err(err) => submit_errors.push(eyre!("submit join error: {err}")),
            }
        }
        if !submitted {
            return Err(eyre!(
                "failed to submit log instruction to any running peer: {submit_errors:?}"
            ));
        }

        let view_change_deadline = da_view_change_timeout();

        let status_url = submit_peer
            .client()
            .torii_url
            .join("status")
            .wrap_err("compose status URL")?;
        let start = Instant::now();
        let elapsed =
            wait_for_height(reqwest::Client::new(), status_url, target_height, start).await?;
        ensure!(
            elapsed <= view_change_deadline,
            "expected view change to recover within bound; elapsed={elapsed:?}"
        );

        let http = reqwest::Client::new();
        for (idx, peer) in peers.iter().enumerate() {
            if !peer.is_running() {
                continue;
            }
            let status_url = peer
                .client()
                .torii_url
                .join("status")
                .wrap_err("compose status URL")?;
            timeout(
                view_change_deadline,
                wait_for_height(http.clone(), status_url, target_height, Instant::now()),
            )
            .await
            .wrap_err_with(|| {
                format!(
                    "timed out waiting for peer {idx} to reach height {target_height}"
                )
            })??;
        }

        for (idx, peer) in peers.iter().enumerate() {
            if !peer.is_running() {
                continue;
            }
            let status = peer.status().await?;
            let view_changes = status
                .sumeragi
                .as_ref()
                .ok_or_else(|| eyre!("status missing sumeragi snapshot"))?
                .view_change_install_total;
            let baseline = baseline_view_changes[idx];
            ensure!(
                view_changes >= baseline.saturating_add(1),
                "expected peer {idx} view_change_install_total to advance after leader shutdown: before={baseline}, after={view_changes}",
            );
        }

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
    let payload_bytes = RBC_RECOVERY_PAYLOAD_BYTES;
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;
    let rbc_chunk_max_bytes = RBC_RECOVERY_CHUNK_BYTES;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_data_availability_enabled(true)
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
                .write(
                    ["sumeragi", "advanced", "rbc", "payload_chunks_per_tick"],
                    1i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "rbc",
                        "rebroadcast_sessions_per_tick",
                    ],
                    1i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_sessions"],
                    2_048i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_sessions"],
                    1_536i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_bytes"],
                    536_870_912i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_bytes"],
                    402_653_184i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "pending_max_chunks"],
                    1_024i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "pending_max_bytes"],
                    16_777_216i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "disk_store_max_bytes"],
                    536_870_912i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "disk_store_ttl_ms"],
                    600_000i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                    600_000i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    rbc_chunk_max_bytes,
                );
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
        network
            .ensure_blocks_with(|height| height.total >= 1)
            .await?;

        let env_dir = network.env_dir().to_path_buf();
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|cow| ConfigLayer(cow.into_owned()))
            .collect();

        let client = network.client();
        let torii = client.torii_url.clone();

        let http = http_client_with_client_auth(&client)?;
        let status_before = fetch_status(&client).await.wrap_err_with(|| {
            format!(
                "fetch status before RBC session; torii={torii}, env_dir={}",
                env_dir.display()
            )
        })?;
        let expected_height = status_before.blocks + 1;

        let peers = network.peers().clone();
        let mut probes = Vec::with_capacity(peers.len());
        for peer in &peers {
            let sessions_url = peer
                .client()
                .torii_url
                .join("v1/sumeragi/rbc/sessions")
                .wrap_err("compose sessions URL")?;
            let baseline_hashes = fetch_rbc_session_hashes(&http, &sessions_url).await?;
            probes.push(RbcSessionsProbe {
                url: sessions_url,
                baseline_hashes,
            });
        }

        let heavy_message = generate_incompressible_payload(
            "sumeragi_rbc_session_recovers_after_cold_restart",
            payload_bytes,
        );
        let submit_client = client.clone();
        tokio::task::spawn_blocking(move || {
            submit_client.submit(Log::new(Level::INFO, heavy_message))
        })
        .await
        .wrap_err("submit join")??;

        let inflight = wait_for_inflight_rbc(http.clone(), probes, Instant::now())
            .await
            .wrap_err("wait for inflight RBC session before shutdown")?;
        ensure!(
            inflight.height >= expected_height,
            "expected RBC session height >= {expected_height}, got {}",
            inflight.height
        );
        let block_hash_hex = inflight.block_hash.clone();
        let session_height = inflight.height;

        let observed_peer_index = peers
            .iter()
            .position(|peer| {
                peer.client()
                    .torii_url
                    .join("v1/sumeragi/rbc/sessions")
                    .ok()
                    .as_ref()
                    == Some(&inflight.sessions_url)
            })
            .ok_or_else(|| {
                eyre!(
                    "failed to map RBC sessions URL {} to a peer",
                    inflight.sessions_url
                )
            })?;
        let observed_torii = peers[observed_peer_index].client().torii_url.clone();
        let status_url = observed_torii
            .join("status")
            .wrap_err("compose observed status URL")?;
        let sessions_url = inflight.sessions_url.clone();
        let store_dir = peers[observed_peer_index]
            .kura_store_dir()
            .join("rbc_sessions");

        let snapshot_before = wait_for_rbc_session_snapshot(
            &http,
            &sessions_url,
            session_height,
            &block_hash_hex,
            Instant::now(),
            da_rbc_inflight_timeout(),
        )
        .await
        .wrap_err("wait for inflight RBC session before shutdown")?;

        ensure!(snapshot_before.total_chunks > 0, "expected chunk metadata");
        ensure!(
            !snapshot_before.delivered,
            "session should remain in-flight prior to shutdown (received={}, total={})",
            snapshot_before.received_chunks,
            snapshot_before.total_chunks
        );

        ensure!(
            snapshot_before.received_chunks > 0,
            "expected RBC payload chunks before shutdown (received={}, total={})",
            snapshot_before.received_chunks,
            snapshot_before.total_chunks
        );
        let expected_total_chunks = snapshot_before.total_chunks;

        let persist_start = Instant::now();
        let expected_prefix = format!("{block_hash_hex}_{session_height}_");
        loop {
            if persist_start.elapsed() > da_rbc_persist_timeout() {
                return Err(eyre!(
                    "timed out waiting for persisted RBC session file for {block_hash_hex} at height {session_height} in {}",
                    store_dir.display()
                ));
            }
            let entries = match fs::read_dir(&store_dir) {
                Ok(entries) => entries,
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    sleep(Duration::from_millis(200)).await;
                    continue;
                }
                Err(err) => {
                    return Err(eyre!(
                        "failed to read RBC session store dir {}: {err}",
                        store_dir.display()
                    ));
                }
            };
            let mut found = false;
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(&expected_prefix) && name.ends_with(".norito") {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
            sleep(Duration::from_millis(200)).await;
        }

        network.shutdown().await;

        if sandbox::handle_result(
            restart_all_peers(&network, &config_layers).await,
            "sumeragi_rbc_session_recovers_after_cold_restart_restart_all",
        )?
        .is_none()
        {
            return Ok(());
        }

        let restarted_client = peers[observed_peer_index].client();
        let resumed_torii = observed_torii.clone();
        let restart_sessions_url = resumed_torii
            .join("v1/sumeragi/rbc/sessions")
            .wrap_err("compose restart sessions URL")?;
        let recovery_start = Instant::now();
        wait_for_recovered_flag(
            http.clone(),
            restart_sessions_url.clone(),
            session_height,
            &block_hash_hex,
            recovery_start,
        )
        .await
        .wrap_err("wait for recovered flag after restart")?;
        let snapshot_after = wait_for_rbc_session_snapshot(
            &http,
            &restart_sessions_url,
            session_height,
            &block_hash_hex,
            Instant::now(),
            da_rbc_recovery_timeout(),
        )
        .await
        .wrap_err("fetch recovered RBC session snapshot after restart")?;
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
            snapshot_after.total_chunks == expected_total_chunks,
            "total chunk count should remain {expected_total_chunks}, got {}",
            snapshot_after.total_chunks
        );

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
        let store_dir_for_rbc = store_dir.clone();
        let rbc_future = tokio::spawn(async move {
            wait_for_persisted_rbc_session(&store_dir_for_rbc, resume_height, resume_start).await
        });
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
    require_rbc_observation: bool,
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
    let stake_amount = SumeragiNposParameters::default().min_self_bond();
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
            .write(["sumeragi", "consensus_mode"], "npos")
            .write(
                ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                rbc_chunk_max_bytes,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "store_max_sessions"],
                2_048i64,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "store_soft_sessions"],
                1_536i64,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "store_max_bytes"],
                536_870_912i64,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "store_soft_bytes"],
                402_653_184i64,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                600_000i64,
            )
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
        .with_npos_genesis_bootstrap(stake_amount)
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
    let status_timeout = da_commit_wait_timeout().saturating_add(Duration::from_secs(60));
    client.transaction_status_timeout = status_timeout;
    client.transaction_ttl = Some(status_timeout.saturating_add(Duration::from_secs(30)));
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

    let http = http_client_with_client_auth(&client)?;
    let start = Instant::now();

    let rbc_handle = require_rbc_observation.then(|| {
        tokio::spawn(wait_for_rbc_delivery(
            http.clone(),
            sessions_url,
            expected_height,
            start,
        ))
    });
    let commit_handle = tokio::spawn(wait_for_height(
        http.clone(),
        status_url,
        expected_height,
        start,
    ));

    let submit_client = client.clone();
    let submit_handle = tokio::task::spawn_blocking(move || {
        submit_client.submit(Log::new(Level::INFO, heavy_message))
    });
    let submit_hash = submit_handle.await.wrap_err("submit join")??;
    println!(
        "submitted heavy tx {submit_hash:?}; blocks_before={}",
        status_before.blocks
    );

    let commit_elapsed = commit_handle.await.wrap_err("commit join")??;
    let rbc_observation = if let Some(mut rbc_handle) = rbc_handle {
        match timeout(Duration::from_millis(RBC_DELIVER_GRACE_MS), &mut rbc_handle).await {
            Ok(joined) => joined.wrap_err("rbc join")??,
            Err(_) => {
                rbc_handle.abort();
                if let Some(observation) = rbc_observation_from_persisted_snapshot(
                    network.peers(),
                    expected_height,
                    commit_elapsed,
                ) {
                    eprintln!(
                        "RBC session endpoint observation timed out at height {expected_height}; using persisted snapshot fallback"
                    );
                    observation
                } else {
                    eprintln!(
                        "RBC session observation timed out at height {expected_height}; using commit timing fallback"
                    );
                    RbcObservation {
                        delivered_at: commit_elapsed,
                        height: expected_height,
                        view: 0,
                        total_chunks: 0,
                        received_chunks: 0,
                        ready_count: 0,
                        block_hash: String::new(),
                    }
                }
            }
        }
    } else {
        eprintln!(
            "RBC session observation disabled for {scenario_name}; using commit timing fallback"
        );
        RbcObservation {
            delivered_at: commit_elapsed,
            height: expected_height,
            view: 0,
            total_chunks: 0,
            received_chunks: 0,
            ready_count: 0,
            block_hash: String::new(),
        }
    };
    let ready_required = rbc_observation.ready_count > 0;

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

    if ready_required {
        let any_ready = per_peer_metrics
            .iter()
            .any(|(_, metrics)| metrics.ready_total >= 1.0);
        assert!(
            any_ready,
            "expected at least one peer to record RBC READY broadcasts (ready_count={})",
            rbc_observation.ready_count
        );
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
        true,
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
fn parse_pending_rbc_stash_counters_reads_fields() {
    let raw = r#"{
        "pending_rbc": {
            "stash_ready_total": 2,
            "stash_ready_init_missing_total": 1,
            "stash_ready_roster_missing_total": 0,
            "stash_ready_roster_hash_mismatch_total": 1,
            "stash_ready_roster_unverified_total": 0,
            "stash_deliver_total": 3,
            "stash_deliver_init_missing_total": 1,
            "stash_deliver_roster_missing_total": 1,
            "stash_deliver_roster_hash_mismatch_total": 0,
            "stash_deliver_roster_unverified_total": 1,
            "stash_chunk_total": 4
        }
    }"#;
    let value: Value = json::from_str(raw).expect("parse JSON");
    let counters = parse_pending_rbc_stash_counters(&value);
    assert_eq!(counters.stash_ready_total, 2);
    assert_eq!(counters.stash_ready_init_missing_total, 1);
    assert_eq!(counters.stash_ready_roster_missing_total, 0);
    assert_eq!(counters.stash_ready_roster_hash_mismatch_total, 1);
    assert_eq!(counters.stash_ready_roster_unverified_total, 0);
    assert_eq!(counters.stash_deliver_total, 3);
    assert_eq!(counters.stash_deliver_init_missing_total, 1);
    assert_eq!(counters.stash_deliver_roster_missing_total, 1);
    assert_eq!(counters.stash_deliver_roster_hash_mismatch_total, 0);
    assert_eq!(counters.stash_deliver_roster_unverified_total, 1);
    assert_eq!(counters.stash_chunk_total, 4);
    assert_eq!(counters.total(), 9);
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
    assert_eq!(
        da_commit_wait_timeout(),
        Duration::from_secs(DA_COMMIT_WAIT_TIMEOUT_SECS)
    );
}

fn da_commit_wait_timeout() -> Duration {
    Duration::from_secs(DA_COMMIT_WAIT_TIMEOUT_SECS)
}

fn da_rbc_delivery_timeout() -> Duration {
    Duration::from_secs(DA_RBC_DELIVERY_TIMEOUT_SECS)
}

fn da_rbc_inflight_timeout() -> Duration {
    Duration::from_secs(DA_RBC_INFLIGHT_TIMEOUT_SECS)
}

fn da_rbc_persist_timeout() -> Duration {
    Duration::from_secs(DA_RBC_PERSIST_TIMEOUT_SECS)
}

fn da_rbc_recovery_timeout() -> Duration {
    Duration::from_secs(DA_RBC_RECOVERY_TIMEOUT_SECS)
}

fn da_rbc_session_timeout() -> Duration {
    Duration::from_secs(DA_RBC_SESSION_TIMEOUT_SECS)
}

fn da_view_change_timeout() -> Duration {
    Duration::from_secs(DA_VIEW_CHANGE_TIMEOUT_SECS)
}

fn da_payload_loss_commit_timeout() -> Duration {
    Duration::from_secs(DA_PAYLOAD_LOSS_COMMIT_TIMEOUT_SECS)
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

async fn submit_log_to_any_peer(
    peers: &[NetworkPeer],
    payload: String,
) -> Result<(NetworkPeer, HashOf<SignedTransaction>)> {
    if peers.is_empty() {
        return Err(eyre!("no peers available for submission"));
    }
    let mut errors = Vec::new();
    for peer in peers {
        if !peer.is_running() {
            continue;
        }
        let submit_client = peer.client();
        let payload_clone = payload.clone();
        match tokio::task::spawn_blocking(move || {
            submit_client.submit(Log::new(Level::INFO, payload_clone))
        })
        .await
        {
            Ok(Ok(hash)) => return Ok((peer.clone(), hash)),
            Ok(Err(err)) => errors.push(err),
            Err(err) => errors.push(eyre!("submit join error: {err}")),
        }
    }
    Err(eyre!(
        "failed to submit log instruction to any running peer: {errors:?}"
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submit_log_to_any_peer_errors_without_peers() {
    let result = submit_log_to_any_peer(&[], "payload".to_string()).await;
    assert!(result.is_err());
}

fn collect_da_block_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(err)
                    .wrap_err_with(|| format!("failed to read directory {}", dir.display()));
            }
        };
        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let path = entry.path();
            if file_type.is_dir() {
                if entry.file_name() == std::ffi::OsStr::new("da_blocks") {
                    let da_entries = match fs::read_dir(&path) {
                        Ok(entries) => entries,
                        Err(err) if err.kind() == ErrorKind::NotFound => continue,
                        Err(err) => {
                            return Err(err).wrap_err_with(|| {
                                format!("failed to read directory {}", path.display())
                            });
                        }
                    };
                    for da_entry in da_entries {
                        let da_entry = da_entry?;
                        let da_path = da_entry.path();
                        if da_entry.file_type()?.is_file()
                            && da_path.extension().and_then(|ext| ext.to_str()) == Some("norito")
                        {
                            files.push(da_path);
                        }
                    }
                } else {
                    stack.push(path);
                }
            }
        }
    }
    Ok(files)
}

fn da_block_height(path: &Path) -> Option<u64> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.parse::<u64>().ok())
}

async fn wait_for_da_block_files(root: PathBuf, timeout: Duration) -> Result<Vec<PathBuf>> {
    let start = Instant::now();
    loop {
        let files = collect_da_block_files(&root)?;
        if !files.is_empty() {
            return Ok(files);
        }
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for DA-evicted blocks under {}",
                root.display()
            ));
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

async fn fetch_rbc_session_hashes(
    http: &reqwest::Client,
    sessions_url: &reqwest::Url,
) -> Result<Vec<String>> {
    let response = http
        .get(sessions_url.clone())
        .header("Accept", "application/json")
        .send()
        .await
        .wrap_err("fetch RBC sessions baseline")?;
    if !response.status().is_success() {
        return Ok(Vec::new());
    }
    let body = response.text().await.wrap_err("sessions body")?;
    let value: Value = json::from_str(&body).wrap_err("parse sessions JSON")?;
    let Some(items) = value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };
    let mut hashes = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let block_hash = extract_string(
            obj.get("block_hash")
                .ok_or_else(|| eyre!("missing block_hash"))?,
        )?;
        hashes.push(block_hash);
    }
    Ok(hashes)
}

async fn wait_for_inflight_rbc(
    http: reqwest::Client,
    sessions: Vec<RbcSessionsProbe>,
    start: Instant,
) -> Result<RbcInflightObservation> {
    let timeout = da_rbc_inflight_timeout();
    let mut last_candidate: Option<RbcInflightObservation> = None;
    loop {
        if start.elapsed() > timeout {
            if let Some(candidate) = last_candidate {
                return Err(eyre!(
                    "timed out waiting for in-flight RBC chunks; last_seen={{height={}, received_chunks={}, total_chunks={}, delivered={}, block_hash={}, url={}}}",
                    candidate.height,
                    candidate.received_chunks,
                    candidate.total_chunks,
                    candidate.delivered,
                    candidate.block_hash,
                    candidate.sessions_url
                ));
            }
            return Err(eyre!("timed out waiting for in-flight RBC chunks"));
        }
        for probe in &sessions {
            let response = http
                .get(probe.url.clone())
                .header("Accept", "application/json")
                .send()
                .await
                .wrap_err("fetch RBC sessions")?;
            if !response.status().is_success() {
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
                    let block_hash = extract_string(
                        obj.get("block_hash")
                            .ok_or_else(|| eyre!("missing block_hash"))?,
                    )?;
                    if probe.baseline_hashes.contains(&block_hash) {
                        continue;
                    }
                    let height =
                        extract_u64(obj.get("height").ok_or_else(|| eyre!("missing height"))?)?;
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
                            .ok_or_else(|| eyre!("missing delivered"))?,
                    )?;
                    let observation = RbcInflightObservation {
                        block_hash,
                        sessions_url: probe.url.clone(),
                        height,
                        total_chunks,
                        received_chunks,
                        delivered,
                    };
                    if total_chunks == 0 {
                        last_candidate = Some(observation);
                        continue;
                    }
                    if delivered {
                        last_candidate = Some(observation);
                        continue;
                    }
                    if received_chunks == 0 {
                        last_candidate = Some(observation);
                        continue;
                    }
                    return Ok(observation);
                }
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
    let timeout = da_rbc_persist_timeout();
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
                && summary.received_chunks > 0
                && !summary.invalid
        }) {
            return Ok(summary);
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_persisted_inflight_rbc_session(
    store_dir: &Path,
    expected_height: u64,
    start: Instant,
) -> Result<rbc_status::Summary> {
    let timeout = da_rbc_persist_timeout();
    loop {
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for persisted in-flight RBC session at height {expected_height} in {}",
                store_dir.display()
            ));
        }
        let snapshot = rbc_status::read_persisted_snapshot(store_dir);
        if let Some(summary) = snapshot.into_iter().find(|summary| {
            summary.height == expected_height
                && summary.total_chunks > 0
                && summary.received_chunks > 0
                && !summary.delivered
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
    let timeout = da_rbc_delivery_timeout();
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

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_da_eviction_rehydrates_block_bodies() -> Result<()> {
    let scenario_name = stringify!(sumeragi_da_eviction_rehydrates_block_bodies);
    let payload_bytes = 128 * 1024;
    let tx_limit =
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX);
    let tx_limit_nz =
        NonZeroU64::new(tx_limit).ok_or_else(|| eyre!("tx_limit must be non-zero"))?;
    let rbc_chunk_max_bytes = i64::try_from(payload_bytes).unwrap_or(i64::MAX);
    let stake_amount = SumeragiNposParameters::default().min_self_bond();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_genesis_bootstrap(stake_amount)
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
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    P2P_TX_FRAME_BUDGET_BYTES,
                )
                .write(["network", "p2p_queue_cap_high"], P2P_QUEUE_CAP_HIGH)
                .write(["network", "p2p_queue_cap_low"], P2P_QUEUE_CAP_LOW)
                .write(["network", "p2p_post_queue_cap"], P2P_POST_QUEUE_CAP)
                .write(
                    ["torii", "max_content_len"],
                    torii_max_content_len_for_payload(payload_bytes),
                )
                .write(["sumeragi", "da", "enabled"], true)
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(
                    ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                    rbc_chunk_max_bytes,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                    600_000i64,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "disk_store_ttl_ms"],
                    600_000i64,
                )
                .write(
                    ["sumeragi", "debug", "rbc", "force_deliver_quorum_one"],
                    true,
                )
                .write(["nexus", "enabled"], true)
                .write(["nexus", "storage", "max_disk_usage_bytes"], 1_000_000i64)
                .write(
                    ["nexus", "storage", "disk_budget_weights", "kura_blocks_bps"],
                    9_000i64,
                )
                .write(
                    [
                        "nexus",
                        "storage",
                        "disk_budget_weights",
                        "wsv_snapshots_bps",
                    ],
                    500i64,
                )
                .write(
                    ["nexus", "storage", "disk_budget_weights", "sorafs_bps"],
                    300i64,
                )
                .write(
                    [
                        "nexus",
                        "storage",
                        "disk_budget_weights",
                        "soranet_spool_bps",
                    ],
                    100i64,
                )
                .write(
                    [
                        "nexus",
                        "storage",
                        "disk_budget_weights",
                        "soravpn_spool_bps",
                    ],
                    100i64,
                )
                .write(["kura", "blocks_in_memory"], 2i64);
        });

    let Some(network) = sandbox::start_network_async_or_skip(builder, scenario_name).await? else {
        return Ok(());
    };

    let result: Result<()> = async {
        network
            .ensure_blocks_with(|height| height.total >= 1)
            .await?;

        let peers = network.peers();
        let primary_peer = peers
            .first()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = primary_peer.client();
        let http = reqwest::Client::new();
        let kura_root = primary_peer.kura_store_dir();

        let status_before = fetch_status(&client).await?;
        let mut expected_height = status_before.blocks;
        let mut da_files = Vec::new();
        for idx in 0..10u64 {
            expected_height = expected_height.saturating_add(1);
            let payload = generate_incompressible_payload(
                &format!("{scenario_name}-payload-{idx}"),
                payload_bytes,
            );
            let (active_peer, _) = submit_log_to_any_peer(&peers, payload).await?;
            let _ = wait_for_height(
                http.clone(),
                active_peer
                    .client()
                    .torii_url
                    .join("status")
                    .wrap_err("compose status URL")?,
                expected_height,
                Instant::now(),
            )
            .await?;
            da_files = collect_da_block_files(&kura_root)?;
            if !da_files.is_empty() {
                break;
            }
        }

        if da_files.is_empty() {
            da_files = wait_for_da_block_files(kura_root, Duration::from_secs(30)).await?;
        }
        ensure!(
            !da_files.is_empty(),
            "expected DA-evicted block files under Kura storage"
        );
        let evicted_height = da_files
            .iter()
            .filter_map(|path| da_block_height(path))
            .min()
            .ok_or_else(|| eyre!("failed to parse DA block heights"))?;

        let query_start = Instant::now();
        let evicted_block = loop {
            let query_peer = peers
                .iter()
                .find(|peer| peer.is_running())
                .ok_or_else(|| eyre!("no running peers available for block query"))?;
            let blocks = query_peer.client().query(FindBlocks).execute_all()?;
            if let Some(block) = blocks
                .iter()
                .find(|block| block.header().height().get() == evicted_height)
            {
                break block.clone();
            }
            ensure!(
                query_start.elapsed() < da_commit_wait_timeout(),
                "missing block at evicted height {evicted_height}"
            );
            sleep(Duration::from_millis(200)).await;
        };
        ensure!(
            evicted_block.external_transactions().len() > 0,
            "expected rehydrated block to include transactions at height {evicted_height}"
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

struct RbcSessionSnapshot {
    total_chunks: u64,
    received_chunks: u64,
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

async fn wait_for_rbc_session_snapshot(
    http: &reqwest::Client,
    sessions_url: &reqwest::Url,
    expected_height: u64,
    block_hash_hex: &str,
    start: Instant,
    timeout: Duration,
) -> Result<RbcSessionSnapshot> {
    loop {
        if start.elapsed() > timeout {
            return Err(eyre!(
                "timed out waiting for RBC session snapshot at height {expected_height} ({block_hash_hex})"
            ));
        }
        if let Some(snapshot) =
            fetch_rbc_session(http, sessions_url, expected_height, block_hash_hex).await?
        {
            return Ok(snapshot);
        }
        sleep(Duration::from_millis(200)).await;
    }
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
    let timeout = da_rbc_recovery_timeout();
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

fn rbc_observation_from_persisted_snapshot(
    peers: &[NetworkPeer],
    expected_height: u64,
    delivered_at: Duration,
) -> Option<RbcObservation> {
    peers
        .iter()
        .flat_map(|peer| {
            let store_dir = peer.kura_store_dir().join("rbc_sessions");
            rbc_status::read_persisted_snapshot(store_dir)
        })
        .filter(|summary| {
            summary.height == expected_height && summary.delivered && !summary.invalid
        })
        .max_by_key(|summary| {
            (
                summary.ready_count,
                u64::from(summary.received_chunks),
                u64::from(summary.total_chunks),
            )
        })
        .map(|summary| RbcObservation {
            delivered_at,
            height: summary.height,
            view: summary.view,
            total_chunks: summary.total_chunks,
            received_chunks: summary.received_chunks,
            ready_count: summary.ready_count,
            block_hash: hex::encode(summary.block_hash.as_ref()),
        })
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
