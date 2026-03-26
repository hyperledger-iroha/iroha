use std::{fs, net::SocketAddr, path::Path, sync::Arc, time::Duration};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use color_eyre::{Result, eyre::eyre};
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{
            SumeragiBlockSyncRosterStatus, SumeragiCommitQuorumStatus, SumeragiDaGateReason,
            SumeragiDaGateSatisfaction, SumeragiDaGateStatus, SumeragiDataspaceCommitment,
            SumeragiKuraStoreStatus, SumeragiLaneCommitment, SumeragiLaneGovernance,
            SumeragiMembershipMismatchStatus, SumeragiMembershipStatus,
            SumeragiMissingBlockFetchStatus, SumeragiPendingRbcStatus, SumeragiRbcStoreStatus,
            SumeragiRuntimeUpgradeHook, SumeragiStatusWire, SumeragiValidationRejectStatus,
            SumeragiViewChangeCauseStatus,
        },
    },
    nexus::{DataSpaceId, LaneId},
    parameter::system::SumeragiConsensusMode,
    prelude::ChainId,
};
use iroha_telemetry::metrics::{
    CryptoStatus, GovernanceManifestAdmissionCounters, GovernanceManifestQuorumCounters,
    GovernanceProposalCounters, GovernanceProtectedNamespaceCounters, GovernanceStatus,
    Halo2Status, Status as TelemetryStatus, TxGossipSnapshot, Uptime,
};
use mochi_core::default_manifest;
use norito::json::{self, Value};
use parking_lot::Mutex;
use tokio::{
    net::TcpListener,
    sync::{broadcast, oneshot},
    task::JoinHandle,
};

/// Binary fixture forwarded on the mocked `/block/stream` endpoint.
pub const CANONICAL_BLOCK_WIRE: &[u8] =
    include_bytes!("../../mochi-core/tests/fixtures/canonical_block_wire.bin");
/// Binary fixture forwarded on the mocked `/events` endpoint.
pub const CANONICAL_EVENT_MESSAGE: &[u8] =
    include_bytes!("../../mochi-core/tests/fixtures/canonical_event_message.bin");

/// Deterministic payloads served by the mock Torii instance.
#[derive(Clone, Debug)]
pub struct MockToriiData {
    /// Snapshot returned from `GET /status`.
    pub status: TelemetryStatus,
    /// Snapshot returned from `GET /v1/sumeragi/status`.
    pub sumeragi: SumeragiStatusWire,
    /// JSON payload returned from `GET /configuration`.
    pub configuration: Value,
    /// Prometheus metrics payload returned from `GET /metrics`.
    pub metrics: String,
    /// Raw bytes returned from `POST /query`.
    pub query_response: Vec<u8>,
    /// Binary WebSocket frame broadcast on `/block/stream`.
    pub block_frame: Vec<u8>,
    /// Binary WebSocket frame broadcast on `/events`.
    pub event_frame: Vec<u8>,
}

impl Default for MockToriiData {
    fn default() -> Self {
        let governance = GovernanceStatus {
            proposals: GovernanceProposalCounters::default(),
            protected_namespace: GovernanceProtectedNamespaceCounters::default(),
            manifest_admission: GovernanceManifestAdmissionCounters::default(),
            manifest_quorum: GovernanceManifestQuorumCounters::default(),
            recent_manifest_activations: Vec::new(),
            sealed_lanes_total: 0,
            sealed_lane_aliases: Vec::new(),
            citizens_total: 0,
        };

        let status = TelemetryStatus {
            peers: 2,
            blocks: 5,
            blocks_non_empty: 3,
            commit_time_ms: 42,
            da_reschedule_total: 0,
            txs_approved: 7,
            txs_rejected: 1,
            uptime: Uptime(Duration::from_secs(123)),
            view_changes: 0,
            queue_size: 4,
            crypto: CryptoStatus {
                sm_helpers_available: true,
                sm_openssl_preview_enabled: false,
                halo2: Halo2Status::default(),
            },
            stack: Default::default(),
            sumeragi: None,
            governance,
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot::default(),
            sorafs_micropayments: Vec::new(),
            taikai_ingest: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };

        let sumeragi = SumeragiStatusWire {
            mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
            staged_mode_tag: None,
            staged_mode_activation_height: None,
            mode_activation_lag_blocks: None,
            mode_flip_kill_switch: true,
            mode_flip_blocked: false,
            mode_flip_success_total: 0,
            mode_flip_fail_total: 0,
            mode_flip_blocked_total: 0,
            last_mode_flip_timestamp_ms: None,
            last_mode_flip_error: None,
            consensus_caps: None,
            leader_index: 0,
            highest_qc_height: 10,
            highest_qc_view: 4,
            highest_qc_subject: None,
            locked_qc_height: 9,
            locked_qc_view: 3,
            locked_qc_subject: None,
            commit_quorum: SumeragiCommitQuorumStatus::default(),
            view_change_proof_accepted_total: 1,
            view_change_proof_stale_total: 0,
            view_change_proof_rejected_total: 0,
            view_change_suggest_total: 1,
            view_change_install_total: 1,
            view_change_causes: SumeragiViewChangeCauseStatus::default(),
            gossip_fallback_total: 0,
            block_created_dropped_by_lock_total: 0,
            block_created_hint_mismatch_total: 0,
            block_created_proposal_mismatch_total: 0,
            validation_reject_total: 0,
            validation_reject_reason: None,
            validation_rejects: SumeragiValidationRejectStatus::default(),
            peer_key_policy: Default::default(),
            block_sync_roster: SumeragiBlockSyncRosterStatus::default(),
            pacemaker_backpressure_deferrals_total: 0,
            commit_pipeline_tick_total: 0,
            da_reschedule_total: 0,
            missing_block_fetch: SumeragiMissingBlockFetchStatus {
                total: 0,
                last_targets: 0,
                last_dwell_ms: 0,
            },
            committed_edge_conflict_obsolete_total: 0,
            da_gate: SumeragiDaGateStatus {
                reason: SumeragiDaGateReason::None,
                last_satisfied: SumeragiDaGateSatisfaction::None,
                missing_local_data_total: 0,
                manifest_guard_total: 0,
            },
            kura_store: SumeragiKuraStoreStatus {
                failures_total: 0,
                abort_total: 0,
                last_retry_attempt: 0,
                last_retry_backoff_ms: 0,
                last_height: 0,
                last_view: 0,
                last_hash: None,
                ..Default::default()
            },
            rbc_store: SumeragiRbcStoreStatus {
                sessions: 0,
                bytes: 0,
                pressure_level: 0,
                backpressure_deferrals_total: 0,
                persist_drops_total: 0,
                evictions_total: 0,
                recent_evictions: Vec::new(),
            },
            pending_rbc: SumeragiPendingRbcStatus::default(),
            tx_queue_depth: 0,
            tx_queue_capacity: 100,
            tx_queue_saturated: false,
            epoch_length_blocks: 0,
            epoch_commit_deadline_offset: 0,
            epoch_reveal_deadline_offset: 0,
            prf_epoch_seed: None,
            prf_height: 9,
            prf_view: 3,
            vrf_penalty_epoch: 0,
            vrf_committed_no_reveal_total: 0,
            vrf_no_participation_total: 0,
            vrf_late_reveals_total: 0,
            consensus_penalties_applied_total: 0,
            consensus_penalties_pending: 0,
            vrf_penalties_applied_total: 0,
            vrf_penalties_pending: 0,
            membership: SumeragiMembershipStatus::default(),
            membership_mismatch: SumeragiMembershipMismatchStatus::default(),
            lane_commitments: vec![SumeragiLaneCommitment {
                block_height: 10,
                lane_id: LaneId::new(0),
                tx_count: 3,
                total_chunks: 5,
                rbc_bytes_total: 512,
                teu_total: 120,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x10; Hash::LENGTH],
                )),
            }],
            dataspace_commitments: vec![SumeragiDataspaceCommitment {
                block_height: 10,
                lane_id: LaneId::new(0),
                dataspace_id: DataSpaceId::new(4),
                tx_count: 2,
                total_chunks: 3,
                rbc_bytes_total: 256,
                teu_total: 64,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x11; Hash::LENGTH],
                )),
            }],
            lane_settlement_commitments: Vec::new(),
            lane_relay_envelopes: Vec::new(),
            lane_governance_sealed_total: 0,
            lane_governance_sealed_aliases: Vec::new(),
            lane_governance: vec![SumeragiLaneGovernance {
                lane_id: LaneId::new(0),
                alias: "alpha".to_owned(),
                governance: Some("parliament".to_owned()),
                manifest_required: true,
                manifest_ready: true,
                manifest_path: Some("/etc/iroha/lanes/alpha.json".to_owned()),
                validator_ids: vec![
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
                        .to_owned(),
                ],
                quorum: Some(2),
                protected_namespaces: vec!["finance".to_owned()],
                runtime_upgrade: Some(SumeragiRuntimeUpgradeHook {
                    allow: true,
                    require_metadata: true,
                    metadata_key: Some("upgrade_id".to_owned()),
                    allowed_ids: vec!["alpha-upgrade".to_owned()],
                }),
            }],
            worker_loop: Default::default(),
            commit_inflight: Default::default(),
            ..Default::default()
        };

        let configuration = norito::json!({
            "torii": {
                "address": "127.0.0.1:5555",
                "public_address": "127.0.0.1:5555"
            },
            "network": {
                "address": "127.0.0.1:1337"
            }
        });

        Self {
            status,
            sumeragi,
            configuration,
            metrics: "iroha_blocks_total 5\n".to_owned(),
            query_response: vec![0x13, 0x37],
            block_frame: CANONICAL_BLOCK_WIRE.to_vec(),
            event_frame: CANONICAL_EVENT_MESSAGE.to_vec(),
        }
    }
}

impl MockToriiData {
    /// Load deterministic fixtures for a mock Torii instance from the provided directory.
    pub fn from_fixture_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();

        let read_bytes = |path: &Path| -> Result<Vec<u8>> {
            fs::read(path).map_err(|err| eyre!("failed to read {}: {err}", path.display()))
        };
        let read_string = |path: &Path| -> Result<String> {
            fs::read_to_string(path)
                .map_err(|err| eyre!("failed to read {}: {err}", path.display()))
        };

        let status_path = dir.join("status.json");
        let status_bytes = read_bytes(&status_path)?;
        let status: TelemetryStatus = norito::json::from_slice(&status_bytes).map_err(|err| {
            eyre!(
                "failed to decode Torii status fixture {}: {err}",
                status_path.display()
            )
        })?;

        let sumeragi_path = dir.join("sumeragi.json");
        let sumeragi_bytes = read_bytes(&sumeragi_path)?;
        let sumeragi: SumeragiStatusWire =
            norito::json::from_slice(&sumeragi_bytes).map_err(|err| {
                eyre!(
                    "failed to decode Sumeragi status fixture {}: {err}",
                    sumeragi_path.display()
                )
            })?;

        let configuration_path = dir.join("configuration.json");
        let configuration_bytes = read_bytes(&configuration_path)?;
        let configuration: Value =
            norito::json::from_slice(&configuration_bytes).map_err(|err| {
                eyre!(
                    "failed to decode configuration fixture {}: {err}",
                    configuration_path.display()
                )
            })?;

        let metrics_path = dir.join("metrics.prom");
        let metrics = read_string(&metrics_path)?;

        let query_path = dir.join("query.bin");
        let query_response = read_bytes(&query_path)?;

        let block_path = dir.join("block.bin");
        let block_frame = read_bytes(&block_path)?;

        let event_path = dir.join("event.bin");
        let event_frame = read_bytes(&event_path)?;

        Ok(Self {
            status,
            sumeragi,
            configuration,
            metrics,
            query_response,
            block_frame,
            event_frame,
        })
    }
}

#[derive(Clone)]
struct MockToriiBytes {
    status_bytes: Vec<u8>,
    sumeragi_bytes: Vec<u8>,
    configuration_bytes: Vec<u8>,
    metrics: String,
    query_response: Vec<u8>,
}

#[derive(Clone)]
struct AppState {
    bytes: Arc<Mutex<MockToriiBytes>>,
    block_tx: broadcast::Sender<MockToriiFrame>,
    event_tx: broadcast::Sender<MockToriiFrame>,
    default_block_frame: Arc<Mutex<Vec<u8>>>,
    default_event_frame: Arc<Mutex<Vec<u8>>>,
}

/// Builder for spawning a [`MockTorii`] server with deterministic fixtures.
#[derive(Debug, Clone)]
pub struct MockToriiBuilder {
    addr: SocketAddr,
    data: MockToriiData,
}

impl MockToriiBuilder {
    /// Create a new builder bound to the provided socket address.
    #[must_use]
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            data: MockToriiData::default(),
        }
    }

    /// Override the initial status payload served from `/status`.
    #[must_use]
    pub fn status(mut self, status: TelemetryStatus) -> Self {
        self.data.status = status;
        self
    }

    /// Override the metrics payload served from `/metrics`.
    #[must_use]
    pub fn metrics(mut self, metrics: impl Into<String>) -> Self {
        self.data.metrics = metrics.into();
        self
    }

    /// Override the initial block WebSocket frame.
    #[must_use]
    pub fn block_frame(mut self, frame: Vec<u8>) -> Self {
        self.data.block_frame = frame;
        self
    }

    /// Override the initial event WebSocket frame.
    #[must_use]
    pub fn event_frame(mut self, frame: Vec<u8>) -> Self {
        self.data.event_frame = frame;
        self
    }

    /// Populate the builder with fixtures loaded from `dir`.
    pub fn fixture_dir(mut self, dir: impl AsRef<Path>) -> Result<Self> {
        self.data = MockToriiData::from_fixture_dir(dir)?;
        Ok(self)
    }

    /// Spawn the mock server and return a handle for driving it.
    pub async fn spawn(self) -> Result<MockTorii> {
        MockTorii::spawn(self.addr, self.data).await
    }
}

/// Frames that can be pushed onto the mock Torii WebSocket feeds.
#[derive(Clone, Debug)]
pub enum MockToriiFrame {
    /// UTF-8 frame delivered as a text message.
    Text(String),
    /// Binary frame delivered verbatim.
    Binary(Vec<u8>),
    /// Close signal propagated to connected clients.
    Close,
}

impl MockToriiFrame {
    fn into_message(self) -> Message {
        match self {
            Self::Text(text) => Message::Text(text.into()),
            Self::Binary(bytes) => Message::Binary(Bytes::from(bytes)),
            Self::Close => Message::Close(None),
        }
    }
}

/// Running mock Torii instance.
pub struct MockTorii {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
    state: AppState,
}

impl MockTorii {
    async fn spawn(addr: SocketAddr, data: MockToriiData) -> Result<Self> {
        let bytes = MockToriiBytes {
            status_bytes: norito::to_bytes(&data.status)?,
            sumeragi_bytes: norito::to_bytes(&data.sumeragi)?,
            configuration_bytes: json::to_vec_pretty(&data.configuration)?,
            metrics: data.metrics,
            query_response: data.query_response,
        };
        let bytes = Arc::new(Mutex::new(bytes));
        let default_block_frame = Arc::new(Mutex::new(data.block_frame));
        let default_event_frame = Arc::new(Mutex::new(data.event_frame));
        let (block_tx, _) = broadcast::channel(32);
        let (event_tx, _) = broadcast::channel(32);
        let state = AppState {
            bytes,
            block_tx,
            event_tx,
            default_block_frame,
            default_event_frame,
        };

        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let router = Router::new()
            .route("/status", get(handle_status))
            .route("/v1/sumeragi/status", get(handle_sumeragi_status))
            .route("/configuration", get(handle_configuration))
            .route("/metrics", get(handle_metrics))
            .route("/transaction", post(handle_transaction))
            .route("/query", post(handle_query))
            .route("/block/stream", get(handle_block_stream))
            .route("/events", get(handle_event_stream))
            .with_state(state.clone());

        let server =
            axum::serve(listener, router.into_make_service()).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });

        let task = tokio::spawn(async move {
            if let Err(err) = server.await {
                eprintln!("mock Torii server error: {err}");
            }
        });

        Ok(Self {
            addr,
            shutdown: Some(shutdown_tx),
            task: Some(task),
            state,
        })
    }

    /// Socket address the server listens on.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Update the status payload returned by `/status`.
    pub fn set_status(&self, status: TelemetryStatus) -> Result<()> {
        let mut guard = self.state.bytes.lock();
        guard.status_bytes = norito::to_bytes(&status)?;
        Ok(())
    }

    /// Update the metrics payload returned by `/metrics`.
    pub fn set_metrics(&self, metrics: impl Into<String>) {
        let mut guard = self.state.bytes.lock();
        guard.metrics = metrics.into();
    }

    /// Broadcast a frame on the `/block/stream` feed.
    pub fn broadcast_block(&self, frame: MockToriiFrame) {
        if let MockToriiFrame::Binary(bytes) = &frame {
            let mut guard = self.state.default_block_frame.lock();
            *guard = bytes.clone();
        }
        let _ = self.state.block_tx.send(frame);
    }

    /// Broadcast a frame on the `/events` feed.
    pub fn broadcast_event(&self, frame: MockToriiFrame) {
        if let MockToriiFrame::Binary(bytes) = &frame {
            let mut guard = self.state.default_event_frame.lock();
            *guard = bytes.clone();
        }
        let _ = self.state.event_tx.send(frame);
    }

    /// Signal the server to shut down and wait for completion.
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.await
                .map_err(|err| eyre!("mock Torii task aborted: {err}"))?;
        }
        Ok(())
    }
}

impl Drop for MockTorii {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

async fn handle_status(State(state): State<AppState>) -> impl IntoResponse {
    let bytes = state.bytes.lock().status_bytes.clone();
    binary_response(bytes, "application/norito")
}

async fn handle_sumeragi_status(State(state): State<AppState>) -> impl IntoResponse {
    let bytes = state.bytes.lock().sumeragi_bytes.clone();
    binary_response(bytes, "application/norito")
}

async fn handle_configuration(State(state): State<AppState>) -> impl IntoResponse {
    let bytes = state.bytes.lock().configuration_bytes.clone();
    binary_response(bytes, "application/json")
}

async fn handle_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let metrics = state.bytes.lock().metrics.clone();
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from(metrics))
        .expect("metrics response")
}

async fn handle_transaction() -> impl IntoResponse {
    StatusCode::ACCEPTED
}

async fn handle_query(State(state): State<AppState>) -> impl IntoResponse {
    let bytes = state.bytes.lock().query_response.clone();
    binary_response(bytes, "application/octet-stream")
}

async fn handle_block_stream(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| block_stream(socket, state))
}

async fn handle_event_stream(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| event_stream(socket, state))
}

async fn block_stream(mut socket: WebSocket, state: AppState) {
    if let Err(err) = send_default_frame(&mut socket, &state.default_block_frame).await {
        eprintln!("failed to send default block frame: {err}");
        return;
    }
    let mut rx = state.block_tx.subscribe();
    while let Ok(frame) = rx.recv().await {
        if send_frame(&mut socket, frame).await.is_err() {
            break;
        }
    }
}

async fn event_stream(mut socket: WebSocket, state: AppState) {
    if let Err(err) = send_default_frame(&mut socket, &state.default_event_frame).await {
        eprintln!("failed to send default event frame: {err}");
        return;
    }
    let mut rx = state.event_tx.subscribe();
    while let Ok(frame) = rx.recv().await {
        if send_frame(&mut socket, frame).await.is_err() {
            break;
        }
    }
}

async fn send_default_frame(
    socket: &mut WebSocket,
    storage: &Arc<Mutex<Vec<u8>>>,
) -> std::result::Result<(), axum::Error> {
    let bytes = storage.lock().clone();
    if bytes.is_empty() {
        return Ok(());
    }
    send_frame(socket, MockToriiFrame::Binary(bytes)).await
}

async fn send_frame(
    socket: &mut WebSocket,
    frame: MockToriiFrame,
) -> std::result::Result<(), axum::Error> {
    socket.send(frame.into_message()).await
}

fn binary_response(body: Vec<u8>, content_type: &'static str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
        .body(Body::from(body))
        .expect("binary response")
}

/// Utility used by the Kagami stub binary to emit default manifests.
pub fn kagami_default_manifest_json(
    genesis_public_key: &PublicKey,
    ivm_dir: impl AsRef<Path>,
) -> Result<String> {
    let manifest = default_manifest(
        ChainId::from("mochi-mock-chain"),
        genesis_public_key,
        ivm_dir.as_ref(),
        SumeragiConsensusMode::Permissioned,
        None,
    )?;
    let value = json::to_value(&manifest)?;
    Ok(json::to_string_pretty(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_data_uses_fixtures() {
        let data = MockToriiData::default();
        assert_eq!(data.block_frame.as_slice(), CANONICAL_BLOCK_WIRE);
        assert_eq!(data.event_frame.as_slice(), CANONICAL_EVENT_MESSAGE);
    }
}
