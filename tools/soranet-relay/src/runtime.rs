//! Runtime orchestration for the relay daemon.

#![allow(unexpected_cfgs)]

use std::{
    fmt::Write as _,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU16, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use iroha_crypto::{
    Algorithm, KeyPair, PrivateKey,
    soranet::{
        certificate::RelayCertificateBundleV2,
        handshake::{
            HandshakeSuite, HarnessError as NoiseHandshakeError,
            RuntimeParams as NoiseRuntimeParams, SessionSecrets, process_client_hello,
            relay_finalize_handshake, update_suite_list,
        },
        pow::{self, Parameters as PowParameters, Ticket as PowTicket},
        puzzle::{self, ChallengeBinding as PuzzleBinding},
        token::{self, AdmissionToken, DecodeError as TokenDecodeError},
    },
};
use iroha_data_model::{
    metadata::Metadata,
    prelude::Name,
    soranet::{
        RelayId,
        incentives::{RelayBandwidthProofV1, RelayComplianceStatusV1, RelayEpochMetricsV1},
        privacy_metrics::{
            SoranetPowFailureReasonV1, SoranetPrivacyHandshakeFailureV1, SoranetPrivacyModeV1,
            SoranetPrivacyThrottleScopeV1,
        },
        vpn::{
            VPN_DEFAULT_TUNNEL_MTU_BYTES, VpnCellClassV1, VpnFlowLabelV1, VpnHelperTicketError,
            VpnHelperTicketV1, derive_vpn_session_address_plan_v1,
        },
    },
};
use iroha_primitives::json::Json;
use norito::{
    NoritoDeserialize, NoritoSerialize, codec::Decode, streaming::SoranetAccessKind, to_bytes,
};
use quinn::{ClosedStream, Connection, Endpoint, Incoming, RecvStream, SendStream, VarInt};
use rand::{SeedableRng, rngs::StdRng};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, Notify},
    task::JoinHandle,
    time::{Instant as TokioInstant, MissedTickBehavior, interval, interval_at, sleep, timeout},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, protocol::Message},
};
use tracing::{debug, info, warn};

use crate::{
    capability::{
        self, CapabilityError, CapabilityWarning, GreaseEntry, NegotiatedCapabilities,
        ServerCapabilities, SignatureId, encode_relay_advertisement, negotiate_capabilities,
        parse_client_advertisement,
    },
    circuit::{
        CircuitAdmissionError, CircuitRegistry, PaddingBudget, abort_padding_task,
        spawn_padding_task,
    },
    compliance::{ComplianceLogger, ThrottleAudit},
    config::{self, ConfigError, RelayConfig, RelayMode},
    congestion::{CongestionController, CongestionError, CongestionLease},
    constant_rate::ConstantRateProfileSpec,
    dos::{DoSControls, ThrottleReason, TokenPolicyError},
    error::RelayError,
    exit::{
        ExitRouting, ExitRoutingState, ExitStreamTag, KaigiStreamState, NoritoStreamState,
        RouteCatalogError, RouteOpenFrame, RouteOpenFrameError, derive_kaigi_room_id,
    },
    guard::{self, GuardDirectoryError},
    handshake::{ClientHello, MAX_CLIENT_HELLO_LEN},
    incentive_log::IncentiveLogger,
    incentives::{EpochSummary, RelayPerformanceAccumulator},
    metrics::{Metrics, VpnRuntimeState, normalize_downgrade_reason},
    privacy::{
        PrivacyAggregator, PrivacyEventBuffer, ProxyPolicyEventBuffer, RejectReason, ThrottleScope,
    },
    scheduler::{
        CELL_SIZE_BYTES, Cell, CellClass, CellScheduler, OverflowPolicy, QueueDepths,
        SchedulerConfig,
    },
    vpn::{VpnFrameIoError, VpnOverlay, VpnSessionHandle},
    vpn_adapter::{VpnAdapter, VpnBridge},
};

struct AdminRenderContext<'a> {
    metrics: &'a Metrics,
    privacy: &'a PrivacyAggregator,
    privacy_events: &'a PrivacyEventBuffer,
    proxy_policy_events: &'a ProxyPolicyEventBuffer,
    performance: &'a Mutex<RelayPerformanceAccumulator>,
}

#[derive(Clone)]
struct MetricsResources {
    metrics: Arc<Metrics>,
    privacy: Arc<PrivacyAggregator>,
    privacy_events: Arc<PrivacyEventBuffer>,
    proxy_policy_events: Arc<ProxyPolicyEventBuffer>,
    performance: Arc<Mutex<RelayPerformanceAccumulator>>,
}

const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4";
const NDJSON_CONTENT_TYPE: &str = "application/x-ndjson";
const SORANET_HANDSHAKE_LOG_TARGET: &str = "soranet.handshake";
const HANDSHAKE_STREAM_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_PAYLOAD_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_HANDSHAKE_FRAME_LEN: usize = MAX_CLIENT_HELLO_LEN;
/// Temporary seed used when the relay configuration does not provide an identity key.
const FALLBACK_IDENTITY_SEED: [u8; 32] = [0x42; 32];
/// Provisional epoch window for incentive aggregation (1 hour).
const INCENTIVE_EPOCH_WINDOW_SECS: u64 = 60 * 60;
/// Maximum Norito-encoded bandwidth proof payload accepted over the QUIC measurement stream.
const MAX_BANDWIDTH_PROOF_FRAME_LEN: usize = 4 * 1024;
/// GAR category recorded when a norito-stream request arrives without a configured route.
const FALLBACK_NORITO_UNSUPPORTED_CATEGORY: &str = "stream.norito.unsupported";
/// GAR category recorded when a kaigi-stream request arrives without a configured route.
const FALLBACK_KAIGI_UNSUPPORTED_CATEGORY: &str = "stream.kaigi.unsupported";
/// Minimum acceptable dummy ratio before alerting operators that cover traffic has fallen.
const LOW_DUMMY_RATIO_THRESHOLD: f64 = 0.20;
/// Default ordering of supported handshake suites.
const DEFAULT_HANDSHAKE_SUITES: [HandshakeSuite; 2] = [
    HandshakeSuite::Nk2Hybrid,
    HandshakeSuite::Nk3PqForwardSecure,
];
const VPN_BACKEND_BOOTSTRAP_MAGIC: &[u8; 8] = b"SVPNBE1\0";
const VPN_BACKEND_STATUS_READY: u8 = 1;

/// Shared context required by `monitor_circuit`.
#[derive(Clone)]
struct MonitorCircuitResources {
    registry: Arc<CircuitRegistry>,
    privacy: Arc<PrivacyAggregator>,
    privacy_events: Arc<PrivacyEventBuffer>,
    performance: Arc<Mutex<RelayPerformanceAccumulator>>,
    relay_id: RelayId,
    incentives: Option<Arc<IncentiveLogger>>,
    mode: RelayMode,
    exit_routing: Arc<ExitRoutingState>,
    compliance: Option<Arc<ComplianceLogger>>,
    metrics: Arc<Metrics>,
    lane_manager: Arc<ConstantRateLaneManager>,
    vpn: Option<Arc<VpnOverlay>>,
}

#[derive(Debug)]
struct ConstantRateLaneManager {
    spec: ConstantRateProfileSpec,
    registry: Arc<CircuitRegistry>,
    current_cap: AtomicU16,
    degraded: AtomicBool,
}

impl ConstantRateLaneManager {
    fn new(spec: ConstantRateProfileSpec, registry: Arc<CircuitRegistry>) -> Self {
        Self {
            spec,
            registry,
            current_cap: AtomicU16::new(spec.neighbor_cap),
            degraded: AtomicBool::new(false),
        }
    }

    fn current_cap(&self) -> u16 {
        self.current_cap.load(Ordering::Relaxed)
    }

    fn profile_spec(&self) -> ConstantRateProfileSpec {
        self.spec
    }

    fn apply_active_sample(&self, active: u64, metrics: &Metrics) {
        metrics.set_constant_rate_active_neighbors(active);
        metrics.set_constant_rate_queue_depth(active);
        let saturation_percent = self.compute_saturation_percent(active);
        metrics.set_constant_rate_saturation_percent(saturation_percent.round() as u64);
        let dummy_floor = u64::from(self.spec.dummy_lane_floor);
        let dummy_lanes = dummy_floor.saturating_sub(active.min(dummy_floor));
        metrics.set_constant_rate_dummy_lanes(dummy_lanes);
        let denom = f64::from(self.spec.neighbor_cap.max(1));
        metrics.set_constant_rate_dummy_ratio(dummy_lanes as f64 / denom);
        if let Some(degraded) = self.maybe_toggle_cap(active, saturation_percent) {
            metrics.set_constant_rate_degraded(degraded);
            if degraded {
                metrics.record_constant_rate_ceiling_hit();
            }
        }
    }

    fn compute_saturation_percent(&self, active: u64) -> f64 {
        if self.spec.neighbor_cap == 0 {
            0.0
        } else {
            (active as f64 / f64::from(self.spec.neighbor_cap)) * 100.0
        }
    }

    fn maybe_toggle_cap(&self, active_neighbors: u64, saturation_percent: f64) -> Option<bool> {
        let currently_degraded = self.degraded.load(Ordering::Relaxed);
        if !currently_degraded && saturation_percent >= self.spec.auto_disable_threshold_percent {
            self.degraded.store(true, Ordering::Relaxed);
            let reduced = self.spec.dummy_lane_floor.max(1);
            self.current_cap.store(reduced, Ordering::Relaxed);
            let neighbors = self.registry.constant_rate_neighbors();
            info!(
                profile = self.spec.name,
                saturation = %format!("{saturation_percent:.2}"),
                active_neighbors,
                neighbor_count = neighbors.len(),
                neighbors = ?neighbors,
                new_cap = reduced,
                "constant-rate neighbor cap reduced due to saturation"
            );
            Some(true)
        } else if currently_degraded
            && saturation_percent <= self.spec.auto_reenable_threshold_percent
        {
            self.degraded.store(false, Ordering::Relaxed);
            self.current_cap
                .store(self.spec.neighbor_cap, Ordering::Relaxed);
            let neighbors = self.registry.constant_rate_neighbors();
            info!(
                profile = self.spec.name,
                saturation = %format!("{saturation_percent:.2}"),
                active_neighbors,
                neighbor_count = neighbors.len(),
                neighbors = ?neighbors,
                new_cap = self.spec.neighbor_cap,
                "constant-rate neighbor cap restored"
            );
            Some(false)
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct ConstantRateEngine {
    scheduler: CellScheduler,
    dummy_sent: u64,
    total_sent: u64,
    tick_duration: Duration,
}

#[derive(Debug)]
struct ConstantRateTick {
    cell: Cell,
    queues: QueueDepths,
    dummy_ratio: f64,
}

#[derive(Debug, Clone, Copy)]
struct CongestionAction {
    buffer_space_bytes: usize,
    dropped_class: Option<CellClass>,
}

impl ConstantRateEngine {
    fn new(spec: ConstantRateProfileSpec) -> Self {
        let tick_duration = milliseconds_to_duration(spec.tick_millis);
        let queue_capacity = usize::from(spec.lane_cap.max(1)).saturating_mul(4);
        let scheduler = CellScheduler::new(SchedulerConfig {
            tick_duration,
            queue_capacity,
            overflow_policy: OverflowPolicy::DropOldest,
        });
        Self {
            scheduler,
            dummy_sent: 0,
            total_sent: 0,
            tick_duration,
        }
    }

    fn tick_duration(&self) -> Duration {
        self.tick_duration
    }

    #[cfg(test)]
    fn enqueue(&mut self, cell: Cell) -> bool {
        self.scheduler.enqueue(cell)
    }

    fn next_cell(&mut self) -> ConstantRateTick {
        let queues = self.scheduler.queue_depths();
        let cell = self.scheduler.force_tick();
        self.total_sent = self.total_sent.saturating_add(1);
        if cell.is_dummy {
            self.dummy_sent = self.dummy_sent.saturating_add(1);
        }
        let dummy_ratio = if self.total_sent == 0 {
            1.0
        } else {
            self.dummy_sent as f64 / self.total_sent as f64
        };
        ConstantRateTick {
            cell,
            queues,
            dummy_ratio,
        }
    }

    fn apply_congestion_hint(&mut self, buffer_space_bytes: usize) -> Option<CongestionAction> {
        if buffer_space_bytes >= CELL_SIZE_BYTES {
            return None;
        }
        let dropped_class = self.scheduler.drop_lowest_priority();
        Some(CongestionAction {
            buffer_space_bytes,
            dropped_class,
        })
    }
}

fn spawn_constant_rate_task(
    connection: Connection,
    spec: ConstantRateProfileSpec,
    metrics: Arc<Metrics>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut engine = ConstantRateEngine::new(spec);
        let mut ticker = interval(engine.tick_duration());
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut low_dummy_active = false;

        loop {
            ticker.tick().await;
            if let Some(congestion) =
                engine.apply_congestion_hint(connection.datagram_send_buffer_space())
            {
                metrics.record_constant_rate_congestion_event(congestion.buffer_space_bytes as u64);
                if let Some(class) = congestion.dropped_class {
                    metrics.record_constant_rate_congestion_drop(class);
                    debug!(
                        buffer_space = congestion.buffer_space_bytes,
                        ?class,
                        "dropped queued constant-rate cell due to datagram congestion"
                    );
                } else {
                    debug!(
                        buffer_space = congestion.buffer_space_bytes,
                        "datagram buffer congested; no queued constant-rate cells to drop"
                    );
                }
            }
            let tick = engine.next_cell();
            metrics.set_constant_rate_queue_depth(tick.queues.total() as u64);
            metrics.set_constant_rate_queue_depths(
                tick.queues.control as u64,
                tick.queues.interactive as u64,
                tick.queues.bulk as u64,
            );
            metrics.set_constant_rate_dummy_ratio(tick.dummy_ratio);
            if tick.dummy_ratio < LOW_DUMMY_RATIO_THRESHOLD {
                if !low_dummy_active {
                    metrics.record_constant_rate_low_dummy();
                    metrics.set_constant_rate_degraded(true);
                    low_dummy_active = true;
                }
            } else {
                low_dummy_active = false;
            }

            let payload = Bytes::from(tick.cell.to_bytes());
            if let Err(error) = connection.send_datagram(payload) {
                warn!(
                    ?error,
                    "failed to send constant-rate cell; stopping constant-rate task"
                );
                break;
            }
        }
    })
}

fn abort_constant_rate_task(task: Option<JoinHandle<()>>) {
    if let Some(handle) = task {
        handle.abort();
    }
}

fn milliseconds_to_duration(millis: f64) -> Duration {
    let clamped = millis.max(1.0);
    let micros = (clamped * 1_000.0).round() as u64;
    Duration::from_micros(micros.max(1))
}

#[derive(Clone)]
struct ExitStreamResources {
    norito: Option<Arc<NoritoStreamState>>,
    kaigi: Option<Arc<KaigiStreamState>>,
    privacy: Arc<PrivacyAggregator>,
    privacy_events: Arc<PrivacyEventBuffer>,
    privacy_mode: SoranetPrivacyModeV1,
    mode: RelayMode,
    compliance: Option<Arc<ComplianceLogger>>,
    vpn: Option<Arc<VpnOverlay>>,
}

#[derive(Clone, Copy)]
struct PaddingSchedule {
    channel_id: [u8; 32],
    period: Duration,
}

type ToriiWebSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

#[allow(unexpected_cfgs)]
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize)]
struct NoritoStreamOpen {
    channel_id: [u8; 32],
    route_id: [u8; 32],
    stream_id: [u8; 32],
    authenticated: bool,
    padding_budget_ms: Option<u16>,
    access_kind: SoranetAccessKind,
    exit_token: Vec<u8>,
}

#[allow(unexpected_cfgs)]
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize)]
struct KaigiStreamOpen {
    channel_id: [u8; 32],
    route_id: [u8; 32],
    stream_id: [u8; 32],
    room_id: [u8; 32],
    authenticated: bool,
    access_kind: SoranetAccessKind,
    exit_token: Vec<u8>,
    exit_multiaddr: String,
}

fn derive_relay_id(identity_key: &KeyPair) -> Result<RelayId, RelayError> {
    let (algorithm, payload) = identity_key.public_key().to_bytes();
    if algorithm != Algorithm::Ed25519 {
        return Err(RelayError::Crypto(format!(
            "unsupported relay identity algorithm `{algorithm:?}`"
        )));
    }
    if payload.len() != 32 {
        return Err(RelayError::Crypto(format!(
            "expected 32-byte Ed25519 public key, found {} bytes",
            payload.len()
        )));
    }
    let mut relay_id = [0u8; 32];
    relay_id.copy_from_slice(payload);
    Ok(relay_id)
}

fn current_epoch(window_secs: u64) -> u32 {
    if window_secs == 0 {
        return 0;
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_secs())
        .unwrap_or_default();
    let epoch = secs / window_secs;
    epoch.min(u32::MAX as u64) as u32
}

fn render_incentive_prometheus(
    relay_id: RelayId,
    summaries: &[EpochSummary],
    mode: RelayMode,
) -> String {
    if summaries.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    let mode_label = mode.as_label();
    let relay_hex = hex::encode(relay_id);

    let _ = writeln!(
        output,
        "# HELP soranet_relay_uptime_seconds_total Relay uptime observed within the incentive epoch."
    );
    let _ = writeln!(output, "# TYPE soranet_relay_uptime_seconds_total counter");
    for summary in summaries {
        let _ = writeln!(
            output,
            "soranet_relay_uptime_seconds_total{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {uptime}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            uptime = summary.uptime_seconds
        );
    }

    let _ = writeln!(
        output,
        "# HELP soranet_relay_scheduled_seconds_total Expected uptime window for the incentive epoch."
    );
    let _ = writeln!(
        output,
        "# TYPE soranet_relay_scheduled_seconds_total counter"
    );
    for summary in summaries {
        let _ = writeln!(
            output,
            "soranet_relay_scheduled_seconds_total{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {scheduled}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            scheduled = summary.scheduled_uptime_seconds
        );
    }

    let _ = writeln!(
        output,
        "# HELP soranet_relay_bandwidth_verified_bytes_total Verified relay bandwidth contribution for the epoch."
    );
    let _ = writeln!(
        output,
        "# TYPE soranet_relay_bandwidth_verified_bytes_total counter"
    );
    for summary in summaries {
        let _ = writeln!(
            output,
            "soranet_relay_bandwidth_verified_bytes_total{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {bytes}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            bytes = summary.verified_bandwidth_bytes
        );
    }

    let _ = writeln!(
        output,
        "# HELP soranet_relay_measurements_total Accepted blinded measurement proofs per epoch."
    );
    let _ = writeln!(output, "# TYPE soranet_relay_measurements_total counter");
    for summary in summaries {
        let _ = writeln!(
            output,
            "soranet_relay_measurements_total{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {count}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            count = summary.measurement_ids.len()
        );
    }

    let _ = writeln!(
        output,
        "# HELP soranet_relay_confidence_floor_per_mille Minimum measurement confidence per epoch."
    );
    let _ = writeln!(
        output,
        "# TYPE soranet_relay_confidence_floor_per_mille gauge"
    );
    for summary in summaries {
        let _ = writeln!(
            output,
            "soranet_relay_confidence_floor_per_mille{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {confidence}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            confidence = u64::from(summary.confidence_floor_per_mille)
        );
    }

    output
}

#[derive(Copy, Clone, Debug)]
enum SnapshotKind {
    Uptime,
    Measurement,
}

impl SnapshotKind {
    fn label(self) -> &'static str {
        match self {
            SnapshotKind::Uptime => "uptime",
            SnapshotKind::Measurement => "measurement",
        }
    }
}

fn snapshot_from_summary(
    relay_id: RelayId,
    summary: &EpochSummary,
    kind: SnapshotKind,
) -> RelayEpochMetricsV1 {
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("snapshot").expect("valid metadata name"),
        Json::from(true),
    );
    metadata.insert(
        Name::from_str("snapshot_reason").expect("valid metadata name"),
        Json::from(kind.label()),
    );
    metadata.insert(
        Name::from_str("measurement_count").expect("valid metadata name"),
        Json::from(summary.measurement_ids.len() as u64),
    );

    RelayEpochMetricsV1 {
        relay_id,
        epoch: summary.epoch,
        uptime_seconds: summary.uptime_seconds,
        scheduled_uptime_seconds: summary.scheduled_uptime_seconds,
        verified_bandwidth_bytes: summary.verified_bandwidth_bytes,
        compliance: RelayComplianceStatusV1::Clean,
        reward_score: 0,
        confidence_floor_per_mille: summary.confidence_floor_per_mille,
        measurement_ids: summary.measurement_ids.clone(),
        metadata,
    }
}

struct HandshakeOutcome {
    negotiated: NegotiatedCapabilities,
    session: SessionSecrets,
    handshake_bytes: u64,
    puzzle_verify_micros: Option<u64>,
    vpn_session: Option<VpnSessionHandle>,
    vpn_helper_ticket: Option<VpnHelperTicketV1>,
}

struct HandshakeByteGuard<'a> {
    metrics: &'a Metrics,
    bytes: u64,
    consumed: bool,
}

#[derive(Debug, Error)]
enum VpnBackendBridgeError {
    #[error(transparent)]
    Vpn(#[from] VpnFrameIoError),
    #[error("backend io error: {0}")]
    BackendIo(#[from] std::io::Error),
    #[error("backend control error: {0}")]
    BackendControl(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct VpnBackendBootstrap {
    session_id_hex: String,
    server_tunnel_addresses: Vec<String>,
    session_routes: Vec<String>,
    mtu_bytes: u16,
}

impl<'a> HandshakeByteGuard<'a> {
    fn new(metrics: &'a Metrics) -> Self {
        Self {
            metrics,
            bytes: 0,
            consumed: false,
        }
    }

    fn add(&mut self, delta: usize) {
        if !self.consumed {
            self.bytes = self.bytes.saturating_add(delta as u64);
        }
    }

    fn finish(mut self) -> u64 {
        if !self.consumed && self.bytes > 0 {
            self.metrics.record_handshake_bytes(self.bytes);
        }
        self.consumed = true;
        self.bytes
    }
}

impl<'a> Drop for HandshakeByteGuard<'a> {
    fn drop(&mut self) {
        if !self.consumed && self.bytes > 0 {
            self.metrics.record_handshake_bytes(self.bytes);
            self.consumed = true;
        }
    }
}

fn derive_vpn_session_id(remote: SocketAddr, transcript_hash: [u8; 32]) -> [u8; 16] {
    let mut hasher = Hasher::new();
    hasher.update(b"soranet-vpn-session-id");
    match remote.ip() {
        IpAddr::V4(v4) => hasher.update(&v4.octets()),
        IpAddr::V6(v6) => hasher.update(&v6.octets()),
    };
    hasher.update(&remote.port().to_be_bytes());
    hasher.update(&transcript_hash);
    let digest = hasher.finalize();
    let mut session_id = [0u8; 16];
    session_id.copy_from_slice(&digest.as_bytes()[..16]);
    session_id
}

fn vpn_flow_label_from_session_id(session_id: [u8; 16]) -> VpnFlowLabelV1 {
    let value = (u32::from(session_id[0]) << 16)
        | (u32::from(session_id[1]) << 8)
        | u32::from(session_id[2]);
    VpnFlowLabelV1::from_u32(value).expect("three-byte flow label should always fit")
}

fn build_vpn_backend_bootstrap(session_id: [u8; 16]) -> VpnBackendBootstrap {
    let address_plan = derive_vpn_session_address_plan_v1(session_id);
    VpnBackendBootstrap {
        session_id_hex: hex::encode(session_id),
        server_tunnel_addresses: address_plan.server_tunnel_addresses,
        session_routes: address_plan.session_routes,
        mtu_bytes: VPN_DEFAULT_TUNNEL_MTU_BYTES,
    }
}

async fn write_vpn_backend_bootstrap<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bootstrap: &VpnBackendBootstrap,
) -> Result<(), VpnBackendBridgeError> {
    let payload = serde_json::to_vec(bootstrap)
        .map_err(|error| VpnBackendBridgeError::BackendControl(error.to_string()))?;
    let len = u16::try_from(payload.len()).map_err(|_| {
        VpnBackendBridgeError::BackendControl(format!(
            "vpn backend bootstrap payload {} exceeds u16 length prefix",
            payload.len()
        ))
    })?;
    writer.write_all(VPN_BACKEND_BOOTSTRAP_MAGIC).await?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&payload).await?;
    Ok(())
}

async fn read_vpn_backend_status<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<(), VpnBackendBridgeError> {
    let mut status = [0u8; 1];
    reader.read_exact(&mut status).await?;
    let mut len = [0u8; 2];
    reader.read_exact(&mut len).await?;
    let len = usize::from(u16::from_be_bytes(len));
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    let message = String::from_utf8_lossy(&payload).into_owned();
    if status[0] == VPN_BACKEND_STATUS_READY {
        Ok(())
    } else {
        Err(VpnBackendBridgeError::BackendControl(
            if message.is_empty() {
                "vpn backend rejected session bootstrap".to_owned()
            } else {
                message
            },
        ))
    }
}

fn record_route_open_ingress_metrics(
    vpn_adapter: Option<&VpnAdapter>,
    vpn_session: Option<&VpnSessionHandle>,
) {
    let bytes = RouteOpenFrame::length() as u64;
    if let Some(adapter) = vpn_adapter {
        adapter
            .session()
            .metrics()
            .record_vpn_control_ingress(bytes);
    } else if let Some(session) = vpn_session {
        session
            .session()
            .metrics()
            .record_vpn_control_ingress(bytes);
    }
}

fn record_route_open_egress_metrics(
    vpn_adapter: Option<&VpnAdapter>,
    vpn_session: Option<&VpnSessionHandle>,
) {
    let bytes = RouteOpenFrame::length() as u64;
    if let Some(adapter) = vpn_adapter {
        adapter.session().metrics().record_vpn_control_egress(bytes);
    } else if let Some(session) = vpn_session {
        session.session().metrics().record_vpn_control_egress(bytes);
    }
}

/// Fully configured relay runtime ready to serve traffic.
pub struct RelayRuntime {
    config: RelayConfig,
    metrics: Arc<Metrics>,
    privacy: Arc<PrivacyAggregator>,
    privacy_events: Arc<PrivacyEventBuffer>,
    proxy_policy_events: Arc<ProxyPolicyEventBuffer>,
    registry: Arc<CircuitRegistry>,
    padding_budget: Option<Arc<PaddingBudget>>,
    server_caps: Arc<ServerCapabilities>,
    handshake_suites: Arc<Vec<HandshakeSuite>>,
    grease: Arc<Vec<GreaseEntry>>,
    descriptor_commit: Arc<Vec<u8>>,
    certificate_bundle: Option<Arc<RelayCertificateBundleV2>>,
    identity_key: Arc<KeyPair>,
    dos: Arc<DoSControls>,
    congestion: Option<CongestionController>,
    compliance: Option<Arc<ComplianceLogger>>,
    performance: Arc<Mutex<RelayPerformanceAccumulator>>,
    epoch_window_secs: u64,
    relay_id: RelayId,
    exit_routing: Arc<ExitRoutingState>,
    incentives: Option<Arc<IncentiveLogger>>,
    lane_manager: Arc<ConstantRateLaneManager>,
    vpn: Option<Arc<VpnOverlay>>,
}

#[derive(Clone)]
struct CircuitContext {
    metrics: Arc<Metrics>,
    privacy: Arc<PrivacyAggregator>,
    privacy_events: Arc<PrivacyEventBuffer>,
    proxy_policy_events: Arc<ProxyPolicyEventBuffer>,
    server_caps: Arc<ServerCapabilities>,
    handshake_suites: Arc<Vec<HandshakeSuite>>,
    grease: Arc<Vec<GreaseEntry>>,
    registry: Arc<CircuitRegistry>,
    padding: config::PaddingConfig,
    padding_budget: Option<Arc<PaddingBudget>>,
    mode: RelayMode,
    descriptor_commit: Arc<Vec<u8>>,
    identity_key: Arc<KeyPair>,
    dos: Arc<DoSControls>,
    congestion: Option<CongestionController>,
    compliance: Option<Arc<ComplianceLogger>>,
    performance: Arc<Mutex<RelayPerformanceAccumulator>>,
    relay_id: RelayId,
    exit_routing: Arc<ExitRoutingState>,
    incentives: Option<Arc<IncentiveLogger>>,
    lane_manager: Arc<ConstantRateLaneManager>,
    vpn: Option<Arc<VpnOverlay>>,
}

fn verify_puzzle_ticket_binding(
    ticket: &PowTicket,
    params: &puzzle::Parameters,
    descriptor_commit: &[u8],
    relay_id: &[u8],
    transcript_hash: Option<&[u8]>,
) -> Result<(), puzzle::Error> {
    let binding = PuzzleBinding::new(descriptor_commit, relay_id, transcript_hash);
    puzzle::verify(ticket, &binding, params)
}

fn verify_pow_ticket_binding(
    ticket: &PowTicket,
    params: &PowParameters,
    descriptor_commit: &[u8],
    relay_id: &[u8],
    transcript_hash: Option<&[u8]>,
) -> Result<(), pow::Error> {
    let binding = pow::ChallengeBinding::new(descriptor_commit, relay_id, transcript_hash);
    pow::verify(ticket, &binding, params)
}

impl RelayRuntime {
    /// Build a relay runtime from configuration, validating certificates,
    /// descriptor commits, guard snapshots, and identity keys along the way.
    pub fn new(mut config: RelayConfig) -> Result<Self, RelayError> {
        config.validate()?;
        if let Some(vpn) = config.vpn_config() {
            vpn.require_runtime_available()?;
        }
        let constant_rate_profile = config.constant_rate_profile();
        let profile_spec = constant_rate_profile.spec();
        info!(
            profile = %constant_rate_profile,
            tick_ms = profile_spec.tick_millis,
            lane_cap = profile_spec.lane_cap,
            neighbor_cap = profile_spec.neighbor_cap,
            dummy_floor = profile_spec.dummy_lane_floor,
            "configured constant-rate profile"
        );
        let padding = config.padding_config().clone();
        let padding_budget = PaddingBudget::from_config(&padding).map(Arc::new);
        if let Some(budget) = padding_budget.as_ref() {
            info!(
                limit_bytes_per_sec = budget.limit_per_sec(),
                burst_bytes = budget.burst_bytes(),
                "enabled global padding budget"
            );
        }
        let policy = config.handshake_policy().clone();
        let certificate_bundle = policy.load_certificate_bundle()?;
        let manual_descriptor = policy.descriptor_commit_bytes()?;
        let descriptor_commit_vec = match (&certificate_bundle, manual_descriptor) {
            (Some(bundle), Some(manual)) if manual != bundle.certificate.descriptor_commit => {
                return Err(RelayError::Config(ConfigError::Handshake(
                    "descriptor_commit_hex does not match certificate descriptor_commit"
                        .to_string(),
                )));
            }
            (Some(bundle), _) => bundle.certificate.descriptor_commit.to_vec(),
            (None, Some(manual)) => manual.to_vec(),
            (None, None) => Vec::new(),
        };
        let descriptor_commit_bytes = if descriptor_commit_vec.is_empty() {
            None
        } else if descriptor_commit_vec.len() == 32 {
            let mut commit = [0u8; 32];
            commit.copy_from_slice(&descriptor_commit_vec);
            Some(commit)
        } else {
            return Err(RelayError::Config(ConfigError::Handshake(format!(
                "descriptor commit must be 32 bytes (got {})",
                descriptor_commit_vec.len()
            ))));
        };
        let descriptor_commit = Arc::new(descriptor_commit_vec);

        let manifest_path = policy
            .descriptor_manifest_path()
            .map(|path| path.to_path_buf());
        let manifest_identity = policy.identity_private_key_from_manifest()?;
        let ml_kem_public = policy.ml_kem_keys_from_manifest()?.map(|keys| keys.public);
        if let (Some(bundle), Some(public)) = (certificate_bundle.as_ref(), ml_kem_public.as_ref())
            && public.as_slice() != bundle.certificate.pq_kem_public.as_slice()
        {
            return Err(RelayError::Config(ConfigError::Handshake(
                "ML-KEM public key from descriptor manifest does not match certificate pq_kem_public"
                    .to_string(),
            )));
        }
        let identity_seed = if let Some(seed) = policy.identity_private_key_bytes()? {
            seed
        } else if let Some(seed) = manifest_identity {
            if let Some(path) = &manifest_path {
                debug!(
                    manifest = %path.display(),
                    "relay identity key loaded from descriptor manifest"
                );
            }
            seed
        } else if let Some(path) = &manifest_path {
            return Err(RelayError::Config(ConfigError::DescriptorManifest {
                path: path.clone(),
                message: "descriptor manifest missing identity private key".to_string(),
            }));
        } else {
            warn!("relay handshake identity key missing from config; using fallback test key");
            FALLBACK_IDENTITY_SEED
        };
        let private_key =
            PrivateKey::from_bytes(Algorithm::Ed25519, &identity_seed).map_err(|err| {
                RelayError::Crypto(format!("failed to parse relay identity key: {err}"))
            })?;
        let identity_key = KeyPair::from_private_key(private_key).map_err(|err| {
            RelayError::Crypto(format!("failed to derive relay identity key pair: {err}"))
        })?;
        let relay_id = derive_relay_id(&identity_key)?;
        let identity_key = Arc::new(identity_key);

        if let Some(bundle) = certificate_bundle.as_ref() {
            let (algorithm, public_bytes) = identity_key.public_key().to_bytes();
            if algorithm != Algorithm::Ed25519
                || public_bytes != bundle.certificate.identity_ed25519
            {
                return Err(RelayError::Config(ConfigError::Handshake(
                    "relay identity key does not match certificate identity_ed25519".to_string(),
                )));
            }
            if relay_id != bundle.certificate.relay_id {
                return Err(RelayError::Config(ConfigError::Handshake(
                    "derived relay identifier does not match certificate relay_id".to_string(),
                )));
            }
        }

        if config.guard_directory_config().is_some() && descriptor_commit_bytes.is_none() {
            return Err(RelayError::Config(ConfigError::GuardDirectory(
                "guard_directory requires descriptor_commit_hex or certificate bundle".to_string(),
            )));
        }

        if let Some(guard_cfg) = config.guard_directory_config() {
            let commit_bytes = descriptor_commit_bytes.expect("checked above");
            match guard::load_guard_entry(guard_cfg, &relay_id, &commit_bytes) {
                Ok(entry) => {
                    if let Some(public) = ml_kem_public.as_ref()
                        && public.as_slice() != entry.bundle.certificate.pq_kem_public.as_slice()
                    {
                        return Err(RelayError::Config(ConfigError::GuardDirectory(
                            "ML-KEM public key from descriptor manifest does not match guard directory entry"
                                .to_string(),
                        )));
                    }
                    info!(
                        directory_hash = %hex::encode(entry.directory_hash),
                        phase = ?entry.validation_phase,
                        "validated guard directory snapshot"
                    );
                    if let Some(proof_path) = guard_cfg.pinning_proof_path()
                        && let Err(error) = guard::persist_guard_pinning_proof(
                            proof_path,
                            guard_cfg.snapshot_path(),
                            &entry,
                            &relay_id,
                            SystemTime::now(),
                        )
                    {
                        warn!(
                            proof = %proof_path.display(),
                            %error,
                            "failed to persist guard pinning proof"
                        );
                    }
                }
                Err(GuardDirectoryError::RelayEntryMissing { identity_hex, .. })
                    if guard_cfg.allow_missing_entry() =>
                {
                    warn!(
                        relay = %identity_hex,
                        "guard directory entry missing but allow_missing_entry=true; continuing without pinning proof"
                    );
                }
                Err(err) => return Err(RelayError::Config(err.into())),
            }
        }

        let incentive_logger = config
            .incentive_log_config()
            .as_logger(&hex::encode(relay_id))
            .map_err(RelayError::from)?
            .map(Arc::new);

        let pow_config = config.pow_config().clone();
        let base_pow_params = PowParameters::new(
            pow_config.difficulty.min(u8::MAX as u32) as u8,
            Duration::from_secs(pow_config.max_future_skew_secs),
            Duration::from_secs(pow_config.min_ticket_ttl_secs),
        );
        let puzzle_params = pow_config.puzzle_parameters(&base_pow_params)?;
        let token_policy = pow_config.token_policy().map_err(RelayError::Config)?;

        let grease_entries = policy.grease_entries()?;

        let congestion_controller = {
            let cfg = config.congestion_config().clone();
            if cfg.max_circuits_per_client == 0 {
                None
            } else {
                Some(CongestionController::new(cfg))
            }
        };

        let compliance_logger = match ComplianceLogger::from_config(config.compliance_config()) {
            Ok(Some(logger)) => Some(Arc::new(logger)),
            Ok(None) => None,
            Err(error) => {
                return Err(RelayError::Logging(format!(
                    "failed to initialise compliance logger: {error}"
                )));
            }
        };

        let kem_caps = policy
            .kem
            .iter()
            .map(|entry| capability::KemAdvertisement {
                id: config::parse_kem_id(&entry.id).expect("handshake configuration validated"),
                required: entry.required,
            })
            .collect::<Vec<_>>();

        let signature_caps = policy
            .signatures
            .iter()
            .map(|entry| capability::SignatureAdvertisement {
                id: config::parse_signature_id(&entry.id)
                    .expect("handshake configuration validated"),
                required: entry.required,
            })
            .collect::<Vec<_>>();

        let constant_rate_capability = config.constant_rate_capability();
        let server_caps = ServerCapabilities::new(
            kem_caps,
            signature_caps,
            padding.cell_size,
            descriptor_commit_bytes,
            role_bits(config.mode),
            constant_rate_capability,
        );

        let metrics = Arc::new(Metrics::new());
        metrics.set_constant_rate_profile(
            constant_rate_profile.as_str(),
            u64::from(profile_spec.neighbor_cap),
            profile_spec.tick_millis,
            u64::from(profile_spec.dummy_lane_floor),
        );
        let vpn_overlay = config
            .vpn_config()
            .cloned()
            .filter(|cfg| cfg.enabled)
            .map(VpnOverlay::from_config)
            .map(Arc::new);
        if let Some(vpn) = vpn_overlay.as_ref() {
            let (session_label, byte_label) = vpn.billing_labels();
            metrics.set_vpn_meter_labels(session_label, byte_label);
            metrics.set_vpn_runtime_state(VpnRuntimeState::Active);
            info!(
                session_meter = session_label,
                byte_meter = byte_label,
                padding_budget_ms = vpn.config().padding_budget_ms,
                flow_label_bits = vpn.config().flow_label_bits,
                "vpn overlay enabled; tunnel runtime active"
            );
        } else {
            metrics.set_vpn_runtime_state(VpnRuntimeState::Disabled);
        }
        let exit_routing_cfg = ExitRouting::from_config(config.exit_routing_config())?;
        let exit_routing = Arc::new(exit_routing_cfg.prepare(relay_id));
        if let Some(commit) = descriptor_commit_bytes {
            metrics.set_descriptor_commit_hex(Some(hex::encode(commit)));
        } else {
            metrics.set_descriptor_commit_hex(None);
        }
        if let Some(public) = ml_kem_public.as_ref() {
            let hex = hex::encode(public);
            metrics.set_ml_kem_public_hex(Some(hex.clone()));
            if let Some(path) = &manifest_path {
                info!(
                    manifest = %path.display(),
                    "relay ML-KEM public key loaded from descriptor manifest"
                );
            } else {
                info!("relay ML-KEM public key loaded from inline configuration");
            }
            debug!(ml_kem_public_hex = %hex, "ML-KEM public key advertised for guard directory generation");
        } else {
            metrics.set_ml_kem_public_hex(None);
        }

        let dos = Arc::new(DoSControls::new(
            base_pow_params,
            &pow_config,
            puzzle_params,
            token_policy,
            Arc::clone(&metrics),
            config.mode,
        ));

        let privacy = Arc::new(PrivacyAggregator::new(config.privacy_config().into()));
        let event_capacity = config.privacy_config().event_buffer_capacity;
        let privacy_events = Arc::new(PrivacyEventBuffer::new(event_capacity));
        let proxy_policy_events = Arc::new(ProxyPolicyEventBuffer::new(event_capacity));
        let certificate_bundle_arc = certificate_bundle
            .as_ref()
            .map(|bundle| Arc::new(bundle.clone()));
        let handshake_suites = Arc::new(resolve_handshake_suites(certificate_bundle.as_ref())?);

        let registry = Arc::new(CircuitRegistry::default());
        let lane_manager = Arc::new(ConstantRateLaneManager::new(
            profile_spec,
            Arc::clone(&registry),
        ));
        Ok(Self {
            config,
            metrics: Arc::clone(&metrics),
            privacy,
            privacy_events,
            proxy_policy_events,
            registry,
            padding_budget,
            server_caps: Arc::new(server_caps),
            handshake_suites,
            grease: Arc::new(grease_entries),
            descriptor_commit,
            certificate_bundle: certificate_bundle_arc,
            identity_key,
            dos,
            congestion: congestion_controller,
            compliance: compliance_logger,
            performance: Arc::new(Mutex::new(RelayPerformanceAccumulator::new(relay_id))),
            epoch_window_secs: INCENTIVE_EPOCH_WINDOW_SECS,
            relay_id,
            exit_routing,
            incentives: incentive_logger,
            lane_manager,
            vpn: vpn_overlay,
        })
    }

    /// Expose the metrics registry used by the runtime.
    pub fn metrics(&self) -> Arc<Metrics> {
        Arc::clone(&self.metrics)
    }

    /// Return the relay operating mode (entry/middle/exit).
    pub fn mode(&self) -> RelayMode {
        self.config.mode
    }

    /// Return the configured QUIC listen address string.
    pub fn listen(&self) -> &str {
        &self.config.listen
    }

    /// Return the descriptor commit used to pin handshakes.
    pub fn descriptor_commit(&self) -> &[u8] {
        self.descriptor_commit.as_slice()
    }

    /// Return the validated certificate bundle, if configured.
    pub fn certificate_bundle(&self) -> Option<Arc<RelayCertificateBundleV2>> {
        self.certificate_bundle.as_ref().map(Arc::clone)
    }

    fn circuit_context(&self) -> CircuitContext {
        CircuitContext {
            metrics: Arc::clone(&self.metrics),
            privacy: Arc::clone(&self.privacy),
            privacy_events: Arc::clone(&self.privacy_events),
            proxy_policy_events: Arc::clone(&self.proxy_policy_events),
            server_caps: Arc::clone(&self.server_caps),
            handshake_suites: Arc::clone(&self.handshake_suites),
            grease: Arc::clone(&self.grease),
            registry: Arc::clone(&self.registry),
            padding: self.config.padding_config().clone(),
            padding_budget: self.padding_budget.clone(),
            mode: self.config.mode,
            descriptor_commit: Arc::clone(&self.descriptor_commit),
            identity_key: Arc::clone(&self.identity_key),
            dos: Arc::clone(&self.dos),
            congestion: self.congestion.clone(),
            compliance: self.compliance.clone(),
            performance: Arc::clone(&self.performance),
            relay_id: self.relay_id,
            exit_routing: Arc::clone(&self.exit_routing),
            incentives: self.incentives.clone(),
            lane_manager: Arc::clone(&self.lane_manager),
            vpn: self.vpn.clone(),
        }
    }

    /// Start the relay control/data planes until shutdown is requested.
    pub async fn run(self) -> Result<(), RelayError> {
        let listen_addr = self.config.listen_addr()?;
        let admin_addr = self.config.admin_addr()?;
        let mode = self.config.mode;

        let server_config = if let (Some(cert), Some(key)) = (
            self.config.certificate_path(),
            self.config.private_key_path(),
        ) {
            Self::load_server_config_from_files(cert, key)?
        } else {
            Self::self_signed_server_config(self.config.self_signed_subject())?
        };

        let endpoint = Endpoint::server(server_config, listen_addr)
            .map_err(|error| RelayError::Quic(error.to_string()))?;

        let actual_addr = endpoint
            .local_addr()
            .map_err(|error| RelayError::Quic(error.to_string()))?;
        info!(
            mode = mode.as_label(),
            listen = %actual_addr,
            "relay listening for SoraNet connections"
        );

        let metrics_task = if let Some(admin_addr) = admin_addr {
            let relay_id = self.relay_id;
            let resources = MetricsResources {
                metrics: Arc::clone(&self.metrics),
                privacy: Arc::clone(&self.privacy),
                privacy_events: Arc::clone(&self.privacy_events),
                proxy_policy_events: Arc::clone(&self.proxy_policy_events),
                performance: Arc::clone(&self.performance),
            };
            Some(tokio::spawn(async move {
                if let Err(error) =
                    RelayRuntime::serve_metrics(resources, relay_id, admin_addr, mode).await
                {
                    warn!(%error, "metrics server terminated");
                }
            }))
        } else {
            None
        };
        let uptime_logger = self.incentives.clone();
        let uptime_task = tokio::spawn(RelayRuntime::track_runtime_uptime(
            Arc::clone(&self.performance),
            self.epoch_window_secs,
            self.relay_id,
            uptime_logger,
        ));

        let accept = self.accept_loop(endpoint.clone());
        tokio::pin!(accept);

        loop {
            tokio::select! {
                res = &mut accept => {
                    if let Err(error) = res {
                        warn!(%error, "accept loop terminated unexpectedly");
                        if let Some(handle) = metrics_task.as_ref() {
                            handle.abort();
                        }
                        uptime_task.abort();
                        return Err(error);
                    }
                    break;
                }
                ctrl_c = tokio::signal::ctrl_c() => {
                    if let Err(error) = ctrl_c {
                        warn!(%error, "failed waiting for ctrl-c");
                        if let Some(handle) = metrics_task.as_ref() {
                            handle.abort();
                        }
                        uptime_task.abort();
                        return Err(RelayError::Io(error));
                    }
                    info!("shutdown signal received; closing endpoints");
                    endpoint.close(0u32.into(), b"shutdown");
                }
            }
        }

        if let Some(handle) = metrics_task {
            handle.abort();
        }
        uptime_task.abort();

        Ok(())
    }

    async fn accept_loop(&self, endpoint: Endpoint) -> Result<(), RelayError> {
        while let Some(incoming) = endpoint.accept().await {
            let context = self.circuit_context();
            tokio::spawn(async move { RelayRuntime::handle_incoming(incoming, context).await });
        }

        Ok(())
    }

    async fn handle_incoming(incoming: Incoming, context: CircuitContext) {
        let metrics = Arc::clone(&context.metrics);
        let privacy = Arc::clone(&context.privacy);
        let privacy_events = Arc::clone(&context.privacy_events);
        let mode = context.mode;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();

        match incoming.accept() {
            Ok(connecting) => match connecting.await {
                Ok(connection) => {
                    let remote = connection.remote_address();
                    info!(
                        remote = %remote,
                        mode = mode.as_label(),
                        "accepted SoraNet connection"
                    );
                    tokio::spawn(Self::establish_circuit(connection, context, remote));
                }
                Err(error) => {
                    metrics.record_failure();
                    let now = SystemTime::now();
                    privacy.record_circuit_rejected(now, RejectReason::Other, None);
                    privacy_events.record_handshake_failure(
                        privacy_mode,
                        now,
                        SoranetPrivacyHandshakeFailureV1::Other,
                        None,
                        None,
                    );
                    warn!(%error, "QUIC handshake failed");
                }
            },
            Err(error) => {
                metrics.record_failure();
                let now = SystemTime::now();
                privacy.record_circuit_rejected(now, RejectReason::Other, None);
                privacy_events.record_handshake_failure(
                    privacy_mode,
                    now,
                    SoranetPrivacyHandshakeFailureV1::Other,
                    None,
                    None,
                );
                warn!(%error, "failed to accept incoming QUIC connection");
            }
        }
    }

    async fn establish_circuit(
        connection: Connection,
        context: CircuitContext,
        remote: SocketAddr,
    ) {
        let metrics = Arc::clone(&context.metrics);
        let privacy = Arc::clone(&context.privacy);
        let privacy_events = Arc::clone(&context.privacy_events);
        let registry = Arc::clone(&context.registry);
        let padding = context.padding.clone();
        let mode = context.mode;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();
        let descriptor_commit = if context.descriptor_commit.is_empty() {
            None
        } else {
            Some(context.descriptor_commit.as_ref().as_slice())
        };
        let mut reservation = match context.congestion.as_ref() {
            Some(controller) => match controller.reserve(remote, Instant::now()) {
                Ok(res) => Some(res),
                Err(CongestionError::TooManyCircuits { limit }) => {
                    metrics.record_capacity_reject();
                    let event_time = SystemTime::now();
                    privacy.record_capacity_reject(event_time);
                    privacy.record_throttle(event_time, ThrottleScope::Congestion);
                    privacy.record_gar_category(event_time, "throttle.congestion");
                    privacy_events.record_throttle(
                        privacy_mode,
                        event_time,
                        SoranetPrivacyThrottleScopeV1::from(ThrottleScope::Congestion),
                    );
                    privacy_events.record_gar_category(
                        privacy_mode,
                        event_time,
                        "throttle.congestion",
                    );
                    warn!(
                        remote = %remote,
                        mode = mode.as_label(),
                        limit,
                        "rejecting handshake: maximum circuits per client reached"
                    );
                    let reason = format!("circuit limit reached (limit {limit})");
                    if let Some(logger) = context.compliance.as_ref()
                        && let Err(error) = logger.log_handshake_reject(
                            remote,
                            mode,
                            descriptor_commit,
                            &reason,
                            None,
                            None,
                            &[],
                        )
                    {
                        warn!(%error, "failed to write compliance log entry");
                    }
                    connection.close(0u32.into(), b"capacity exceeded");
                    return;
                }
                Err(CongestionError::HandshakeCooldown {
                    cooldown_millis,
                    observed_gap_millis,
                }) => {
                    metrics.record_throttled();
                    metrics.record_handshake_cooldown_throttle();
                    let event_time = SystemTime::now();
                    privacy.record_throttle(event_time, ThrottleScope::Cooldown);
                    privacy.record_gar_category(event_time, "throttle.cooldown");
                    privacy_events.record_throttle(
                        privacy_mode,
                        event_time,
                        SoranetPrivacyThrottleScopeV1::from(ThrottleScope::Cooldown),
                    );
                    privacy_events.record_gar_category(
                        privacy_mode,
                        event_time,
                        "throttle.cooldown",
                    );
                    debug!(
                        remote = %remote,
                        mode = mode.as_label(),
                        cooldown_millis,
                        observed_gap_millis,
                        "handshake throttled by cooldown window"
                    );
                    let reason = format!(
                        "handshake throttled (cooldown {cooldown_millis} ms, gap {observed_gap_millis} ms)"
                    );
                    let throttle_meta = ThrottleAudit {
                        scope: "handshake_cooldown",
                        cooldown: Some(Duration::from_millis(cooldown_millis)),
                        window: None,
                        burst_limit: None,
                        max_entries: None,
                        observed_gap: Some(Duration::from_millis(observed_gap_millis)),
                    };
                    if let Some(logger) = context.compliance.as_ref()
                        && let Err(error) = logger.log_handshake_reject(
                            remote,
                            mode,
                            descriptor_commit,
                            &reason,
                            Some(throttle_meta),
                            None,
                            &[],
                        )
                    {
                        warn!(%error, "failed to write compliance log entry");
                    }
                    connection.close(0u32.into(), b"throttled");
                    return;
                }
            },
            None => None,
        };

        let attempt = match context.dos.begin(remote, descriptor_commit) {
            Ok(attempt) => attempt,
            Err(throttle) => {
                metrics.record_throttled();
                let event_time = SystemTime::now();
                let throttle_audit = match throttle.reason {
                    ThrottleReason::RemoteQuota => {
                        metrics.record_remote_quota_throttle();
                        privacy.record_throttle(event_time, ThrottleScope::RemoteQuota);
                        privacy.record_gar_category(event_time, "throttle.remote_quota");
                        privacy.record_throttle_cooldown(event_time, throttle.cooldown);
                        privacy_events.record_throttle(
                            privacy_mode,
                            event_time,
                            SoranetPrivacyThrottleScopeV1::from(ThrottleScope::RemoteQuota),
                        );
                        privacy_events.record_gar_category(
                            privacy_mode,
                            event_time,
                            "throttle.remote_quota",
                        );
                        let limits = context.dos.remote_quota_limits();
                        Some(ThrottleAudit {
                            scope: "per_remote",
                            cooldown: Some(throttle.cooldown),
                            window: Some(limits.window()),
                            burst_limit: Some(limits.burst()),
                            max_entries: Some(limits.max_entries()),
                            observed_gap: None,
                        })
                    }
                    ThrottleReason::DescriptorQuota => {
                        metrics.record_descriptor_quota_throttle();
                        privacy.record_throttle(event_time, ThrottleScope::DescriptorQuota);
                        privacy.record_gar_category(event_time, "throttle.descriptor_quota");
                        privacy.record_throttle_cooldown(event_time, throttle.cooldown);
                        privacy_events.record_throttle(
                            privacy_mode,
                            event_time,
                            SoranetPrivacyThrottleScopeV1::from(ThrottleScope::DescriptorQuota),
                        );
                        privacy_events.record_gar_category(
                            privacy_mode,
                            event_time,
                            "throttle.descriptor_quota",
                        );
                        context
                            .dos
                            .descriptor_quota_limits()
                            .map(|limits| ThrottleAudit {
                                scope: "per_descriptor",
                                cooldown: Some(throttle.cooldown),
                                window: Some(limits.window()),
                                burst_limit: Some(limits.burst()),
                                max_entries: Some(limits.max_entries()),
                                observed_gap: None,
                            })
                    }
                    ThrottleReason::DescriptorReplay => {
                        metrics.record_descriptor_replay_throttle();
                        privacy.record_throttle(event_time, ThrottleScope::DescriptorReplay);
                        privacy.record_throttle_cooldown(event_time, throttle.cooldown);
                        privacy_events.record_throttle(
                            privacy_mode,
                            event_time,
                            SoranetPrivacyThrottleScopeV1::from(ThrottleScope::DescriptorReplay),
                        );
                        Some(ThrottleAudit {
                            scope: "descriptor_replay",
                            cooldown: Some(throttle.cooldown),
                            window: None,
                            burst_limit: None,
                            max_entries: None,
                            observed_gap: None,
                        })
                    }
                    ThrottleReason::Emergency => {
                        metrics.record_emergency_throttle();
                        privacy.record_throttle(event_time, ThrottleScope::Emergency);
                        privacy.record_gar_category(event_time, "throttle.emergency");
                        privacy.record_throttle_cooldown(event_time, throttle.cooldown);
                        privacy_events.record_throttle(
                            privacy_mode,
                            event_time,
                            SoranetPrivacyThrottleScopeV1::from(ThrottleScope::Emergency),
                        );
                        privacy_events.record_gar_category(
                            privacy_mode,
                            event_time,
                            "throttle.emergency",
                        );
                        Some(ThrottleAudit {
                            scope: "emergency_consensus",
                            cooldown: Some(throttle.cooldown),
                            window: None,
                            burst_limit: None,
                            max_entries: None,
                            observed_gap: None,
                        })
                    }
                };
                warn!(
                    remote = %remote,
                    mode = mode.as_label(),
                    reason = %throttle.reason,
                    cooldown_secs = throttle.cooldown.as_secs_f32(),
                    "handshake throttled by abuse controls"
                );
                let reason = format!(
                    "dos throttle ({}, cooldown {}s)",
                    throttle.reason,
                    throttle.cooldown.as_secs()
                );
                if let Some(logger) = context.compliance.as_ref()
                    && let Err(error) = logger.log_handshake_reject(
                        remote,
                        mode,
                        descriptor_commit,
                        &reason,
                        throttle_audit,
                        None,
                        &[],
                    )
                {
                    warn!(%error, "failed to write compliance log entry");
                }
                connection.close(0u32.into(), b"throttled");
                return;
            }
        };

        match Self::perform_handshake(&connection, &context, remote).await {
            Ok(HandshakeOutcome {
                negotiated,
                session,
                handshake_bytes,
                puzzle_verify_micros,
                vpn_session,
                vpn_helper_ticket,
            }) => {
                let elapsed = attempt.elapsed();
                metrics.record_success();
                context.dos.record_success(&attempt, elapsed);
                metrics.record_handshake_mode(session.handshake_suite);
                record_handshake_suite_downgrade(metrics.as_ref(), session.handshake_suite);
                let handshake_millis = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
                let sig_labels: Vec<String> = negotiated
                    .signatures
                    .iter()
                    .map(|sig| sig.id.to_string())
                    .collect();
                info!(
                    target: SORANET_HANDSHAKE_LOG_TARGET,
                    remote = %remote,
                    mode = mode.as_label(),
                    kem = %negotiated.kem.id,
                    padding = negotiated.padding,
                    signatures = ?sig_labels,
                    handshake_bytes,
                    elapsed_millis = handshake_millis,
                    puzzle_verify_micros = puzzle_verify_micros,
                    "handshake negotiated"
                );

                let warning_messages = session
                    .warnings
                    .iter()
                    .map(|warning| warning.message.clone())
                    .collect::<Vec<_>>();
                for warning in session.warnings.iter() {
                    metrics.record_downgrade(&warning.message);
                }
                if !warning_messages.is_empty() {
                    warn!(
                        target: SORANET_HANDSHAKE_LOG_TARGET,
                        remote = %remote,
                        mode = mode.as_label(),
                        warnings = ?warning_messages,
                        "SoraNet handshake reported capability warnings"
                    );
                }
                if let Some(payload) = session.telemetry_payload.as_ref() {
                    debug!(
                        target: SORANET_HANDSHAKE_LOG_TARGET,
                        remote = %remote,
                        mode = mode.as_label(),
                        payload = %String::from_utf8_lossy(payload),
                        "SoraNet handshake telemetry"
                    );
                }
                debug!(
                    target: SORANET_HANDSHAKE_LOG_TARGET,
                    remote = %remote,
                    mode = mode.as_label(),
                    key_len = session.session_key.len(),
                    "derived SoraNet session key"
                );

                if let Some(logger) = context.compliance.as_ref()
                    && let Err(error) = logger.log_handshake_success(
                        remote,
                        mode,
                        descriptor_commit,
                        &negotiated,
                        &warning_messages,
                        session.handshake_suite,
                        handshake_millis,
                        handshake_bytes,
                        puzzle_verify_micros,
                    )
                {
                    warn!(%error, "failed to write compliance log entry");
                }

                let register_outcome = match registry.register(
                    remote,
                    &negotiated,
                    Some(context.lane_manager.current_cap()),
                ) {
                    Ok(outcome) => {
                        if let Some(active) = outcome.constant_rate_active {
                            context
                                .lane_manager
                                .apply_active_sample(active, &context.metrics);
                        }
                        outcome
                    }
                    Err(CircuitAdmissionError::ConstantRateNeighborCap { limit }) => {
                        metrics.record_capacity_reject();
                        let event_time = SystemTime::now();
                        privacy.record_capacity_reject(event_time);
                        privacy.record_throttle(event_time, ThrottleScope::Congestion);
                        privacy.record_gar_category(event_time, "throttle.constant_rate_neighbors");
                        privacy_events.record_throttle(
                            privacy_mode,
                            event_time,
                            SoranetPrivacyThrottleScopeV1::from(ThrottleScope::Congestion),
                        );
                        privacy_events.record_gar_category(
                            privacy_mode,
                            event_time,
                            "throttle.constant_rate_neighbors",
                        );
                        warn!(
                            remote = %remote,
                            mode = mode.as_label(),
                            limit,
                            "rejecting handshake: constant-rate neighbor cap reached"
                        );
                        let reason = format!("constant-rate neighbor cap reached (limit {limit})");
                        if let Some(logger) = context.compliance.as_ref()
                            && let Err(error) = logger.log_handshake_reject(
                                remote,
                                mode,
                                descriptor_commit,
                                &reason,
                                None,
                                None,
                                &[],
                            )
                        {
                            warn!(%error, "failed to write compliance log entry");
                        }
                        connection.close(0u32.into(), b"constant-rate capacity exceeded");
                        return;
                    }
                };
                let circuit_id = register_outcome.circuit_id;
                let active_len = registry.active_len() as u64;
                let accepted_at = SystemTime::now();
                privacy.record_circuit_accepted(
                    accepted_at,
                    Some(handshake_millis),
                    Some(active_len),
                );
                privacy_events.record_handshake_success(
                    privacy_mode,
                    accepted_at,
                    Some(handshake_millis),
                    Some(active_len),
                );
                let lease = reservation.take().map(|res| res.into_lease());
                let padding_task = spawn_padding_task(
                    connection.clone(),
                    negotiated.padding,
                    padding.max_idle_millis,
                    remote,
                    Arc::clone(&context.metrics),
                    context.padding_budget.clone(),
                );
                let constant_rate_task = negotiated.constant_rate.map(|_| {
                    spawn_constant_rate_task(
                        connection.clone(),
                        context.lane_manager.profile_spec(),
                        Arc::clone(&context.metrics),
                    )
                });
                let performance = Arc::clone(&context.performance);
                let incentives = context.incentives.clone();
                let relay_id = context.relay_id;
                let resources = MonitorCircuitResources {
                    registry,
                    privacy: Arc::clone(&privacy),
                    privacy_events: Arc::clone(&privacy_events),
                    performance,
                    relay_id,
                    incentives,
                    mode,
                    exit_routing: Arc::clone(&context.exit_routing),
                    compliance: context.compliance.clone(),
                    metrics: Arc::clone(&context.metrics),
                    lane_manager: Arc::clone(&context.lane_manager),
                    vpn: context.vpn.clone(),
                };
                Self::monitor_circuit(
                    connection,
                    remote,
                    circuit_id,
                    padding_task,
                    constant_rate_task,
                    lease,
                    resources,
                    vpn_session,
                    vpn_helper_ticket,
                )
                .await;
            }
            Err(HandshakeError::Downgrade {
                warnings,
                telemetry,
            }) => {
                metrics.record_failure();
                if warnings.is_empty() {
                    metrics.record_downgrade("downgrade");
                } else {
                    for warning in warnings.iter() {
                        metrics.record_downgrade(&warning.message);
                    }
                }
                let downgrade_detail = downgrade_detail_from_warnings(&warnings);
                let elapsed = attempt.elapsed();
                let millis = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
                let event_time = SystemTime::now();
                privacy.record_circuit_rejected(event_time, RejectReason::Downgrade, Some(millis));
                privacy_events.record_handshake_failure(
                    privacy_mode,
                    event_time,
                    SoranetPrivacyHandshakeFailureV1::Downgrade,
                    downgrade_detail.as_deref(),
                    Some(millis),
                );
                context.proxy_policy_events.record_downgrade(
                    privacy_mode,
                    event_time,
                    downgrade_detail.as_deref(),
                );
                if let Some(payload) = telemetry.as_ref() {
                    warn!(
                        target: SORANET_HANDSHAKE_LOG_TARGET,
                        remote = %remote,
                        mode = mode.as_label(),
                        payload = %String::from_utf8_lossy(payload),
                        "SoraNet handshake downgrade telemetry"
                    );
                }
                let warning_messages = warnings
                    .iter()
                    .map(|warning| warning.message.clone())
                    .collect::<Vec<_>>();
                warn!(
                    target: SORANET_HANDSHAKE_LOG_TARGET,
                    remote = %remote,
                    mode = mode.as_label(),
                    warnings = ?warning_messages,
                    "SoraNet handshake downgrade detected"
                );
                let reason = if warning_messages.is_empty() {
                    "downgrade".to_string()
                } else {
                    format!("downgrade: {}", warning_messages.join("; "))
                };
                if let Some(logger) = context.compliance.as_ref()
                    && let Err(error) = logger.log_handshake_reject(
                        remote,
                        mode,
                        descriptor_commit,
                        &reason,
                        None,
                        Some(millis),
                        &warning_messages,
                    )
                {
                    warn!(%error, "failed to write compliance log entry");
                }
                connection.close(0u32.into(), b"handshake downgrade");
            }
            Err(error) => {
                let elapsed = attempt.elapsed();
                metrics.record_failure();
                warn!(
                    remote = %remote,
                    mode = mode.as_label(),
                    error = %error,
                    "handshake failed"
                );
                let millis = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
                let pow_detail = match &error {
                    HandshakeError::Pow(pow_error) => Some(pow_failure_reason(pow_error)),
                    _ => None,
                };
                let reason = match &error {
                    HandshakeError::Pow(_) | HandshakeError::Puzzle(_) => RejectReason::Pow,
                    HandshakeError::Timeout(_) => RejectReason::Timeout,
                    _ => RejectReason::Other,
                };
                let event_time = SystemTime::now();
                privacy.record_circuit_rejected(event_time, reason, Some(millis));
                privacy_events.record_handshake_failure(
                    privacy_mode,
                    event_time,
                    SoranetPrivacyHandshakeFailureV1::from(reason),
                    pow_detail.as_ref().map(|detail| detail.as_label()),
                    Some(millis),
                );
                match &error {
                    HandshakeError::Pow(pow_error) => {
                        context.dos.record_pow_failure(&attempt, elapsed);
                        let mut reason = format!("pow failure: {pow_error}");
                        if let Some(detail) = pow_detail {
                            let _ = write!(reason, " ({})", detail.as_label());
                        }
                        if let Some(logger) = context.compliance.as_ref()
                            && let Err(err) = logger.log_handshake_reject(
                                remote,
                                mode,
                                descriptor_commit,
                                &reason,
                                None,
                                Some(millis),
                                &[],
                            )
                        {
                            warn!(%err, "failed to write compliance log entry");
                        }
                    }
                    HandshakeError::Puzzle(puzzle_error) => {
                        context.dos.record_pow_failure(&attempt, elapsed);
                        let reason = format!("puzzle failure: {puzzle_error}");
                        if let Some(logger) = context.compliance.as_ref()
                            && let Err(err) = logger.log_handshake_reject(
                                remote,
                                mode,
                                descriptor_commit,
                                &reason,
                                None,
                                Some(millis),
                                &[],
                            )
                        {
                            warn!(%err, "failed to write compliance log entry");
                        }
                    }
                    HandshakeError::Timeout(_) => {
                        context.dos.record_timeout(&attempt, elapsed);
                        if let Some(logger) = context.compliance.as_ref()
                            && let Err(err) = logger.log_handshake_reject(
                                remote,
                                mode,
                                descriptor_commit,
                                &error.to_string(),
                                None,
                                Some(millis),
                                &[],
                            )
                        {
                            warn!(%err, "failed to write compliance log entry");
                        }
                    }
                    _ => {
                        context.dos.record_failure(&attempt, elapsed);
                        if let Some(logger) = context.compliance.as_ref()
                            && let Err(err) = logger.log_handshake_reject(
                                remote,
                                mode,
                                descriptor_commit,
                                &error.to_string(),
                                None,
                                Some(millis),
                                &[],
                            )
                        {
                            warn!(%err, "failed to write compliance log entry");
                        }
                    }
                }
                connection.close(0u32.into(), b"handshake failure");
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn monitor_circuit(
        connection: Connection,
        remote: SocketAddr,
        circuit_id: u64,
        padding_task: Option<JoinHandle<()>>,
        constant_rate_task: Option<JoinHandle<()>>,
        congestion_lease: Option<CongestionLease>,
        resources: MonitorCircuitResources,
        vpn_session: Option<VpnSessionHandle>,
        vpn_helper_ticket: Option<VpnHelperTicketV1>,
    ) {
        let privacy_mode: SoranetPrivacyModeV1 = resources.mode.into();
        let measurement_resources = resources.clone();
        let exit_resources = ExitStreamResources {
            norito: resources.exit_routing.norito_stream(),
            kaigi: resources.exit_routing.kaigi_stream(),
            privacy: Arc::clone(&resources.privacy),
            privacy_events: Arc::clone(&resources.privacy_events),
            privacy_mode,
            mode: resources.mode,
            compliance: resources.compliance.clone(),
            vpn: resources.vpn.clone(),
        };
        let registry = Arc::clone(&resources.registry);
        let privacy = Arc::clone(&resources.privacy);
        let privacy_events = Arc::clone(&resources.privacy_events);
        let metrics = Arc::clone(&resources.metrics);

        let measurement_task = tokio::spawn(Self::ingest_measurement_streams(
            connection.clone(),
            measurement_resources,
            remote,
        ));
        let exit_task = match (
            resources.vpn.clone(),
            vpn_session.clone(),
            vpn_helper_ticket,
        ) {
            (Some(vpn), Some(session), Some(helper_ticket)) => {
                tokio::spawn(Self::serve_vpn_backend_tunnel(
                    connection.clone(),
                    remote,
                    vpn,
                    session,
                    helper_ticket,
                ))
            }
            _ => tokio::spawn(Self::serve_exit_streams(
                connection.clone(),
                exit_resources,
                remote,
                vpn_session.clone(),
            )),
        };

        let reason = connection.closed().await;
        abort_padding_task(padding_task);
        abort_constant_rate_task(constant_rate_task);
        let removed = registry.remove(circuit_id);
        if let Some(active) = removed
            .as_ref()
            .and_then(|removal| removal.constant_rate_active)
        {
            resources.lane_manager.apply_active_sample(active, &metrics);
        }
        let active_len = registry.active_len() as u64;
        let sample_time = SystemTime::now();
        privacy.record_active_sample(sample_time, active_len);
        privacy_events.record_active_sample(privacy_mode, sample_time, active_len);
        drop(congestion_lease);

        if let Some(logger) = resources.compliance.as_ref() {
            let lifetime_ms = removed.as_ref().map(|removal| {
                removal
                    .state
                    .opened_at
                    .elapsed()
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64
            });
            let kem_label = removed
                .as_ref()
                .map(|removal| removal.state.kem.to_string());
            let signature_entries = removed.as_ref().map(|removal| {
                removal
                    .state
                    .signatures
                    .iter()
                    .map(|sig| (sig.id.to_string(), sig.required))
                    .collect::<Vec<_>>()
            });
            let padding = removed.as_ref().map(|removal| removal.state.padding);
            let reason_text = reason.to_string();

            if let Err(error) = logger.log_circuit_closed(
                remote,
                resources.mode,
                circuit_id,
                lifetime_ms,
                kem_label.as_deref(),
                signature_entries.as_deref(),
                padding,
                active_len,
                &reason_text,
            ) {
                warn!(%error, "failed to write compliance log entry");
            }
        }

        if let Err(error) = measurement_task.await {
            debug!(
                remote = %remote,
                %error,
                "measurement ingestion task join error"
            );
        }
        if let Err(error) = exit_task.await {
            debug!(
                remote = %remote,
                %error,
                "exit stream task join error"
            );
        }

        if let Some(vpn_session) = vpn_session {
            let receipt = vpn_session.receipt();
            metrics.record_vpn_receipt(&receipt);
            debug!(
                remote = %remote,
                session_id = %hex::encode(receipt.session_id),
                exit_class = receipt.exit_class.as_label(),
                ingress = receipt.ingress_bytes,
                egress = receipt.egress_bytes,
                cover = receipt.cover_bytes,
                uptime_secs = receipt.uptime_secs,
                "vpn session closed; receipt emitted"
            );
        }

        debug!(remote = %remote, ?reason, "SoraNet connection closed");
    }

    async fn ingest_measurement_streams(
        connection: Connection,
        resources: MonitorCircuitResources,
        remote: SocketAddr,
    ) {
        let MonitorCircuitResources {
            performance,
            relay_id,
            incentives,
            privacy,
            privacy_events,
            mode,
            compliance,
            ..
        } = resources;
        let compliance_logger = compliance.clone();
        loop {
            match connection.accept_uni().await {
                Ok(stream) => {
                    let compliance = compliance_logger.clone();
                    if let Err(error) = Self::process_measurement_stream(
                        stream,
                        Arc::clone(&performance),
                        relay_id,
                        incentives.clone(),
                        Arc::clone(&privacy),
                        Arc::clone(&privacy_events),
                        mode,
                        compliance,
                        remote,
                    )
                    .await
                    {
                        warn!(
                            remote = %remote,
                            ?error,
                            "failed to ingest blinded bandwidth proof stream"
                        );
                    }
                }
                Err(quinn::ConnectionError::ApplicationClosed(_)) => break,
                Err(quinn::ConnectionError::LocallyClosed) => break,
                Err(error) => {
                    debug!(
                        remote = %remote,
                        %error,
                        "stopping measurement stream accept loop"
                    );
                    break;
                }
            }
        }
    }

    async fn serve_exit_streams(
        connection: Connection,
        resources: ExitStreamResources,
        remote: SocketAddr,
        vpn_session: Option<VpnSessionHandle>,
    ) {
        loop {
            match connection.accept_bi().await {
                Ok((mut send, mut recv)) => {
                    if let Err(error) = Self::process_exit_stream(
                        &resources,
                        &mut send,
                        &mut recv,
                        remote,
                        vpn_session.clone(),
                    )
                    .await
                    {
                        if let Some(logger) = resources.compliance.as_ref() {
                            let (stream_name, channel) = match &error {
                                ExitStreamError::StreamDisabled { stream } => (Some(*stream), None),
                                ExitStreamError::RouteNotProvisioned { stream, channel } => {
                                    (Some(*stream), Some(channel.as_str()))
                                }
                                ExitStreamError::RouteRequiresAuthentication {
                                    stream,
                                    channel,
                                } => (Some(*stream), Some(channel.as_str())),
                                ExitStreamError::AdapterTimeout { stream, .. } => {
                                    (Some(*stream), None)
                                }
                                ExitStreamError::AdapterConnect { stream, .. } => {
                                    (Some(*stream), None)
                                }
                                ExitStreamError::AdapterSend { stream, .. } => {
                                    (Some(*stream), None)
                                }
                                ExitStreamError::AdapterReceive { stream, .. } => {
                                    (Some(*stream), None)
                                }
                                ExitStreamError::HandshakeEncode { stream, .. } => {
                                    (Some(*stream), None)
                                }
                                ExitStreamError::RouteLookup(_) => (None, None),
                                ExitStreamError::Truncated { .. }
                                | ExitStreamError::Read(_)
                                | ExitStreamError::Decode(_)
                                | ExitStreamError::RecvRead(_)
                                | ExitStreamError::SendWrite(_)
                                | ExitStreamError::SendFinish(_) => (None, None),
                            };
                            let reason_str = error.to_string();
                            if let Err(err) = logger.log_exit_route_reject(
                                remote,
                                resources.mode,
                                stream_name,
                                channel,
                                &reason_str,
                            ) {
                                warn!(%err, "failed to write compliance log entry");
                            }
                        }
                        match error {
                            ExitStreamError::StreamDisabled { stream } => warn!(
                                remote = %remote,
                                stream,
                                "exit routing disabled; update configuration"
                            ),
                            ExitStreamError::RouteNotProvisioned { stream, channel } => warn!(
                                remote = %remote,
                                stream,
                                channel = %channel,
                                "exit request rejected: channel not provisioned"
                            ),
                            ExitStreamError::RouteRequiresAuthentication { stream, channel } => {
                                warn!(
                                    remote = %remote,
                                    stream,
                                    channel = %channel,
                                    "exit request rejected: authentication required"
                                )
                            }
                            other => warn!(
                                remote = %remote,
                                error = %other,
                                "failed to process exit stream"
                            ),
                        }
                        let _ = send.reset(VarInt::from_u32(0));
                        let _ = recv.stop(VarInt::from_u32(0));
                    }
                }
                Err(quinn::ConnectionError::ApplicationClosed(_))
                | Err(quinn::ConnectionError::LocallyClosed) => break,
                Err(error) => {
                    debug!(remote = %remote, %error, "stopping exit stream accept loop");
                    break;
                }
            }
        }
    }

    async fn serve_vpn_backend_tunnel(
        connection: Connection,
        remote: SocketAddr,
        overlay: Arc<VpnOverlay>,
        vpn_session: VpnSessionHandle,
        helper_ticket: VpnHelperTicketV1,
    ) {
        let Some(backend_addr) = overlay.config().backend_socket_addr() else {
            warn!(
                remote = %remote,
                session_id = %hex::encode(helper_ticket.session_id),
                "vpn helper connection rejected: vpn.backend_addr is not configured"
            );
            connection.close(0u32.into(), b"vpn backend unavailable");
            return;
        };

        let backend =
            match timeout(HANDSHAKE_STREAM_TIMEOUT, TcpStream::connect(backend_addr)).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(error)) => {
                    warn!(
                        remote = %remote,
                        backend = %backend_addr,
                        %error,
                        "failed to connect relay VPN backend"
                    );
                    connection.close(0u32.into(), b"vpn backend connect failed");
                    return;
                }
                Err(_) => {
                    warn!(
                        remote = %remote,
                        backend = %backend_addr,
                        "timed out connecting relay VPN backend"
                    );
                    connection.close(0u32.into(), b"vpn backend connect timeout");
                    return;
                }
            };

        let (mut send, mut recv) =
            match timeout(HANDSHAKE_STREAM_TIMEOUT, connection.accept_bi()).await {
                Ok(Ok(streams)) => streams,
                Ok(Err(error)) => {
                    warn!(
                        remote = %remote,
                        %error,
                        "failed to accept vpn helper tunnel stream"
                    );
                    connection.close(0u32.into(), b"vpn tunnel stream failed");
                    return;
                }
                Err(_) => {
                    warn!(
                        remote = %remote,
                        "timed out waiting for vpn helper tunnel stream"
                    );
                    connection.close(0u32.into(), b"vpn tunnel stream timeout");
                    return;
                }
            };

        let flow_label = vpn_flow_label_from_session_id(helper_ticket.session_id);
        let adapter = VpnAdapter::new(vpn_session.session().clone(), overlay.as_ref().clone());
        let bridge = VpnBridge::new(adapter.clone(), helper_ticket.session_id, flow_label);
        let mtu = bridge.max_payload_len();
        let (mut backend_read, mut backend_write) = tokio::io::split(backend);
        let bootstrap = build_vpn_backend_bootstrap(helper_ticket.session_id);

        info!(
            remote = %remote,
            backend = %backend_addr,
            session_id = %hex::encode(helper_ticket.session_id),
            expires_at_ms = helper_ticket.expires_at_ms,
            "bridging helper-authenticated vpn tunnel to relay backend"
        );
        if let Err(error) = write_vpn_backend_bootstrap(&mut backend_write, &bootstrap).await {
            warn!(
                remote = %remote,
                backend = %backend_addr,
                session_id = %hex::encode(helper_ticket.session_id),
                %error,
                "failed to send vpn backend bootstrap"
            );
            connection.close(0u32.into(), b"vpn backend bootstrap failed");
            return;
        }
        if let Err(error) = read_vpn_backend_status(&mut backend_read).await {
            warn!(
                remote = %remote,
                backend = %backend_addr,
                session_id = %hex::encode(helper_ticket.session_id),
                %error,
                "vpn backend failed session bootstrap"
            );
            connection.close(0u32.into(), b"vpn backend bootstrap rejected");
            return;
        }
        if let Err(error) = Self::bridge_vpn_backend_streams(
            &mut send,
            &mut recv,
            bridge,
            &adapter,
            &mut backend_read,
            &mut backend_write,
            mtu,
        )
        .await
        {
            warn!(
                remote = %remote,
                backend = %backend_addr,
                session_id = %hex::encode(helper_ticket.session_id),
                %error,
                "vpn helper bridge stopped"
            );
        }
    }

    async fn bridge_vpn_backend_streams<VW, VR, BR, BW>(
        vpn_writer: &mut VW,
        vpn_reader: &mut VR,
        mut bridge: VpnBridge,
        adapter: &VpnAdapter,
        backend_reader: &mut BR,
        backend_writer: &mut BW,
        mtu: usize,
    ) -> Result<(), VpnBackendBridgeError>
    where
        VW: AsyncWrite + Unpin,
        VR: AsyncRead + Unpin,
        BR: AsyncRead + Unpin,
        BW: AsyncWrite + Unpin,
    {
        let upstream = async {
            bridge
                .pump_tun_to_vpn(backend_reader, vpn_writer, mtu)
                .await
                .map(|_| ())
                .map_err(VpnBackendBridgeError::from)
        };
        let downstream = async {
            loop {
                match adapter.read_ingress_frame(vpn_reader).await {
                    Ok(cell) => {
                        if cell.header.class == VpnCellClassV1::Data {
                            backend_writer.write_all(&cell.payload).await?;
                        }
                    }
                    Err(VpnFrameIoError::FrameLength { actual: 0, .. }) => break Ok(()),
                    Err(error) => break Err(VpnBackendBridgeError::from(error)),
                }
            }
        };

        tokio::select! {
            result = upstream => result,
            result = downstream => result,
        }
    }

    async fn process_exit_stream(
        resources: &ExitStreamResources,
        send: &mut SendStream,
        recv: &mut RecvStream,
        remote: SocketAddr,
        vpn_session: Option<VpnSessionHandle>,
    ) -> Result<(), ExitStreamError> {
        let mut header = [0u8; RouteOpenFrame::length()];
        match recv.read_exact(&mut header).await {
            Ok(()) => {}
            Err(quinn::ReadExactError::FinishedEarly(received)) => {
                return Err(ExitStreamError::Truncated { received });
            }
            Err(quinn::ReadExactError::ReadError(error)) => {
                return Err(ExitStreamError::Read(error));
            }
        }

        let frame = RouteOpenFrame::decode(&header)?;
        let vpn_adapter = vpn_session.as_ref().and_then(|session| {
            resources
                .vpn
                .as_ref()
                .map(|overlay| VpnAdapter::new(session.session().clone(), overlay.as_ref().clone()))
        });
        record_route_open_ingress_metrics(vpn_adapter.as_ref(), vpn_session.as_ref());

        match frame.tag() {
            ExitStreamTag::NoritoStream => {
                Self::handle_norito_stream(
                    resources,
                    send,
                    recv,
                    remote,
                    frame,
                    vpn_adapter.clone(),
                )
                .await?;
            }
            ExitStreamTag::KaigiStream => {
                Self::handle_kaigi_stream(
                    resources,
                    send,
                    recv,
                    remote,
                    frame,
                    vpn_adapter.clone(),
                )
                .await?;
            }
        }

        record_route_open_egress_metrics(vpn_adapter.as_ref(), vpn_session.as_ref());

        Ok(())
    }

    async fn handle_norito_stream(
        resources: &ExitStreamResources,
        send: &mut SendStream,
        recv: &mut RecvStream,
        remote: SocketAddr,
        frame: RouteOpenFrame,
        vpn_adapter: Option<VpnAdapter>,
    ) -> Result<(), ExitStreamError> {
        let authenticated = frame.flags().is_authenticated();
        let (category, norito_state) = match resources.norito.as_ref() {
            Some(state) => (
                state.gar_category(authenticated).to_owned(),
                Some(Arc::clone(state)),
            ),
            None => (FALLBACK_NORITO_UNSUPPORTED_CATEGORY.to_owned(), None),
        };
        let now = SystemTime::now();
        resources.privacy.record_gar_category(now, &category);
        resources
            .privacy_events
            .record_gar_category(resources.privacy_mode, now, &category);

        let channel_id = *frame.channel_id();
        let channel_hex = hex::encode(channel_id);
        let Some(state) = norito_state else {
            warn!(
                remote = %remote,
                channel = %channel_hex,
                authenticated,
                "no norito-stream exit route configured"
            );
            return Err(ExitStreamError::StreamDisabled {
                stream: "norito-stream",
            });
        };

        let record = state.lookup_channel(&channel_id)?.ok_or_else(|| {
            ExitStreamError::RouteNotProvisioned {
                stream: "norito-stream",
                channel: channel_hex.clone(),
            }
        })?;

        if record.access_kind == SoranetAccessKind::Authenticated && !authenticated {
            warn!(
                remote = %remote,
                channel = %channel_hex,
                "norito-stream route requires authenticated viewers"
            );
            return Err(ExitStreamError::RouteRequiresAuthentication {
                stream: "norito-stream",
                channel: channel_hex,
            });
        }

        let padding_interval = record
            .padding_budget_ms
            .map(|ms| Duration::from_millis(u64::from(ms)))
            .filter(|interval| !interval.is_zero())
            .or_else(|| {
                let default = state.padding_target();
                if default.is_zero() {
                    None
                } else {
                    Some(default)
                }
            });

        info!(
            remote = %remote,
            channel = %hex::encode(channel_id),
            route = %hex::encode(record.route_id),
            stream = %hex::encode(record.stream_id),
            authenticated,
            exit_multiaddr = %record.exit_multiaddr,
            padding_budget_ms = record.padding_budget_ms.unwrap_or_default(),
            "norito-stream exit route resolved from spool"
        );
        if let Some(logger) = resources.compliance.as_ref()
            && let Err(error) = logger.log_exit_route_open(
                remote,
                resources.mode,
                "norito-stream",
                authenticated,
                &channel_id,
                &record.route_id,
                &record.stream_id,
                None,
                &format!("{:?}", record.access_kind),
                record.padding_budget_ms,
                &record.exit_multiaddr,
                state.torii_ws_url(),
            )
        {
            warn!(%error, "failed to write compliance log entry");
        }

        let connect_timeout = state.connect_timeout();
        let (ws_stream, response) = timeout(connect_timeout, connect_async(state.torii_ws_url()))
            .await
            .map_err(|_| ExitStreamError::AdapterTimeout {
                stream: "norito-stream",
                timeout: connect_timeout,
            })?
            .map_err(|error| ExitStreamError::AdapterConnect {
                stream: "norito-stream",
                error,
            })?;

        debug!(
            remote = %remote,
            channel = %hex::encode(channel_id),
            status = %response.status(),
            "connected to norito-stream adapter"
        );

        let handshake = NoritoStreamOpen {
            channel_id,
            route_id: record.route_id,
            stream_id: record.stream_id,
            authenticated,
            padding_budget_ms: record.padding_budget_ms,
            access_kind: record.access_kind,
            exit_token: record.exit_token.clone(),
        };
        let handshake_bytes = Bytes::from(to_bytes(&handshake).map_err(|error| {
            ExitStreamError::HandshakeEncode {
                stream: "norito-stream",
                error,
            }
        })?);
        let padding_schedule =
            padding_interval.map(|period| PaddingSchedule { channel_id, period });

        Self::bridge_websocket_stream(
            send,
            recv,
            ws_stream,
            handshake_bytes,
            "norito-stream",
            remote,
            padding_schedule,
            vpn_adapter,
        )
        .await
    }

    async fn handle_kaigi_stream(
        resources: &ExitStreamResources,
        send: &mut SendStream,
        recv: &mut RecvStream,
        remote: SocketAddr,
        frame: RouteOpenFrame,
        vpn_adapter: Option<VpnAdapter>,
    ) -> Result<(), ExitStreamError> {
        let authenticated = frame.flags().is_authenticated();
        let (category, kaigi_state) = match resources.kaigi.as_ref() {
            Some(state) => (
                state.gar_category(authenticated).to_owned(),
                Some(Arc::clone(state)),
            ),
            None => (FALLBACK_KAIGI_UNSUPPORTED_CATEGORY.to_owned(), None),
        };
        let now = SystemTime::now();
        resources.privacy.record_gar_category(now, &category);
        resources
            .privacy_events
            .record_gar_category(resources.privacy_mode, now, &category);

        let channel_id = *frame.channel_id();
        let channel_hex = hex::encode(channel_id);
        let Some(state) = kaigi_state else {
            warn!(
                remote = %remote,
                channel = %channel_hex,
                authenticated,
                "kaigi-stream exit route disabled in configuration"
            );
            return Err(ExitStreamError::StreamDisabled {
                stream: "kaigi-stream",
            });
        };

        let record = state.lookup_channel(&channel_id)?.ok_or_else(|| {
            ExitStreamError::RouteNotProvisioned {
                stream: "kaigi-stream",
                channel: channel_hex.clone(),
            }
        })?;

        if record.access_kind == SoranetAccessKind::Authenticated && !authenticated {
            warn!(
                remote = %remote,
                channel = %channel_hex,
                "kaigi-stream route requires authenticated viewers"
            );
            return Err(ExitStreamError::RouteRequiresAuthentication {
                stream: "kaigi-stream",
                channel: channel_hex,
            });
        }

        let room_id = derive_kaigi_room_id(&channel_id, &record.route_id, &record.stream_id);
        let target_url = match Self::kaigi_multiaddr_to_websocket(&record.exit_multiaddr) {
            Some(url) => url,
            None => {
                warn!(
                    remote = %remote,
                    channel = %hex::encode(channel_id),
                    exit_multiaddr = %record.exit_multiaddr,
                    fallback = state.hub_ws_url(),
                    "kaigi-stream exit multiaddr unsupported; falling back to configured hub"
                );
                state.hub_ws_url().to_owned()
            }
        };

        info!(
            remote = %remote,
            channel = %hex::encode(channel_id),
            route = %hex::encode(record.route_id),
            stream = %hex::encode(record.stream_id),
            room = %hex::encode(room_id),
            authenticated,
            exit_multiaddr = %record.exit_multiaddr,
            target = %target_url,
            "kaigi-stream exit route resolved from spool"
        );
        if let Some(logger) = resources.compliance.as_ref()
            && let Err(error) = logger.log_exit_route_open(
                remote,
                resources.mode,
                "kaigi-stream",
                authenticated,
                &channel_id,
                &record.route_id,
                &record.stream_id,
                Some(&room_id),
                &format!("{:?}", record.access_kind),
                record.padding_budget_ms,
                &record.exit_multiaddr,
                &target_url,
            )
        {
            warn!(%error, "failed to write compliance log entry");
        }

        let connect_timeout = state.connect_timeout();
        let (ws_stream, response) = timeout(connect_timeout, connect_async(target_url.as_str()))
            .await
            .map_err(|_| ExitStreamError::AdapterTimeout {
                stream: "kaigi-stream",
                timeout: connect_timeout,
            })?
            .map_err(|error| ExitStreamError::AdapterConnect {
                stream: "kaigi-stream",
                error,
            })?;

        debug!(
            remote = %remote,
            channel = %hex::encode(channel_id),
            status = %response.status(),
            target = %target_url,
            "connected to kaigi-stream adapter"
        );

        let handshake = KaigiStreamOpen {
            channel_id,
            route_id: record.route_id,
            stream_id: record.stream_id,
            room_id,
            authenticated,
            access_kind: record.access_kind,
            exit_token: record.exit_token.clone(),
            exit_multiaddr: record.exit_multiaddr.clone(),
        };
        let handshake_bytes = Bytes::from(to_bytes(&handshake).map_err(|error| {
            ExitStreamError::HandshakeEncode {
                stream: "kaigi-stream",
                error,
            }
        })?);

        Self::bridge_websocket_stream(
            send,
            recv,
            ws_stream,
            handshake_bytes,
            "kaigi-stream",
            remote,
            None,
            vpn_adapter,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn bridge_websocket_stream(
        send: &mut SendStream,
        recv: &mut RecvStream,
        ws_stream: ToriiWebSocket,
        handshake_bytes: Bytes,
        label: &'static str,
        remote: SocketAddr,
        padding: Option<PaddingSchedule>,
        vpn_adapter: Option<VpnAdapter>,
    ) -> Result<(), ExitStreamError> {
        let (sink, stream) = ws_stream.split();
        let sink = Arc::new(Mutex::new(sink));
        let last_send = Arc::new(Mutex::new(Instant::now()));
        let shutdown = Arc::new(Notify::new());
        let vpn_adapter = vpn_adapter.map(Arc::new);

        let handshake_len = handshake_bytes.len();
        {
            let mut guard = sink.lock().await;
            guard
                .send(Message::Binary(handshake_bytes))
                .await
                .map_err(|error| ExitStreamError::AdapterSend {
                    stream: label,
                    error,
                })?;
        }
        if let Some(adapter) = vpn_adapter.as_ref() {
            adapter.record_egress_frame_count(handshake_len as u64, false);
        }
        let schedule = padding.map(|schedule| {
            let delay = Self::norito_padding_delay(
                &schedule.channel_id,
                schedule.period,
                SystemTime::now(),
            );
            (schedule.period, delay)
        });
        let now = Instant::now();
        let initial_last_send = if let Some((period, delay)) = schedule.as_ref() {
            if *delay < *period {
                now.checked_sub(*period - *delay).unwrap_or(now)
            } else {
                now
            }
        } else {
            now
        };
        *last_send.lock().await = initial_last_send;

        let to_torii = {
            let sink = Arc::clone(&sink);
            let shutdown = Arc::clone(&shutdown);
            let last_send = Arc::clone(&last_send);
            let recv_stream = recv;
            let vpn_adapter = vpn_adapter.clone();
            async move {
                let mut buf = vec![0u8; 16 * 1024];
                let mut terminated_by_notify = false;
                loop {
                    tokio::select! {
                        _ = shutdown.notified() => {
                            terminated_by_notify = true;
                            break;
                        }
                        result = recv_stream.read(&mut buf) => {
                            match result.map_err(ExitStreamError::RecvRead)? {
                                Some(bytes) if bytes > 0 => {
                                    let payload = Bytes::copy_from_slice(&buf[..bytes]);
                                    if let Some(adapter) = vpn_adapter.as_ref() {
                                        adapter.record_ingress_frame_count(bytes as u64, false);
                                    }
                                    {
                                        let mut guard = sink.lock().await;
                                        guard
                                            .send(Message::Binary(payload))
                                            .await
                                            .map_err(|error| ExitStreamError::AdapterSend {
                                                stream: label,
                                                error,
                                            })?;
                                    }
                                    *last_send.lock().await = Instant::now();
                                }
                                _ => {
                                    let mut guard = sink.lock().await;
                                    guard
                                        .send(Message::Close(None))
                                        .await
                                        .map_err(|error| ExitStreamError::AdapterSend {
                                            stream: label,
                                            error,
                                        })?;
                                    break;
                                }
                            }
                        }
                    }
                }
                if !terminated_by_notify {
                    shutdown.notify_waiters();
                }
                Ok::<(), ExitStreamError>(())
            }
        };

        let from_torii = {
            let sink = Arc::clone(&sink);
            let shutdown = Arc::clone(&shutdown);
            let last_send = Arc::clone(&last_send);
            let send_stream = send;
            let mut ws_stream = stream;
            let vpn_adapter = vpn_adapter.clone();
            async move {
                while let Some(message) = ws_stream.next().await {
                    let message = message.map_err(|error| ExitStreamError::AdapterReceive {
                        stream: label,
                        error,
                    })?;
                    match message {
                        Message::Binary(data) => {
                            send_stream
                                .write_all(&data)
                                .await
                                .map_err(ExitStreamError::SendWrite)?;
                            if let Some(adapter) = vpn_adapter.as_ref() {
                                adapter.record_egress_frame_count(data.len() as u64, false);
                            }
                            *last_send.lock().await = Instant::now();
                        }
                        Message::Close(frame) => {
                            if let Some(frame) = frame {
                                let code = u16::from(frame.code);
                                let reason = frame.reason.to_string();
                                debug!(
                                    remote = %remote,
                                    stream = label,
                                    code,
                                    reason = %reason,
                                    "exit adapter closed"
                                );
                            }
                            break;
                        }
                        Message::Ping(payload) => {
                            let mut guard = sink.lock().await;
                            guard.send(Message::Pong(payload)).await.map_err(|error| {
                                ExitStreamError::AdapterSend {
                                    stream: label,
                                    error,
                                }
                            })?;
                        }
                        Message::Pong(_) => {
                            *last_send.lock().await = Instant::now();
                        }
                        Message::Text(text) => {
                            warn!(
                                remote = %remote,
                                stream = label,
                                "ignoring text frame from exit adapter: {text}"
                            );
                        }
                        Message::Frame(_) => {}
                    }
                }
                send_stream.finish().map_err(ExitStreamError::SendFinish)?;
                shutdown.notify_waiters();
                Ok::<(), ExitStreamError>(())
            }
        };

        let padding_future = schedule.map(|(period, delay)| {
            let sink = Arc::clone(&sink);
            let last_send = Arc::clone(&last_send);
            let shutdown = Arc::clone(&shutdown);
            async move {
                let mut ticker = interval_at(TokioInstant::now() + delay, period);
                ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
                loop {
                    tokio::select! {
                        _ = shutdown.notified() => break,
                        _ = ticker.tick() => {
                            let should_send = {
                                let guard = last_send.lock().await;
                                guard.elapsed() >= period
                            };
                            if should_send {
                                {
                                    let mut guard = sink.lock().await;
                                    guard
                                        .send(Message::Binary(Bytes::new()))
                                        .await
                                        .map_err(|error| ExitStreamError::AdapterSend {
                                            stream: label,
                                            error,
                                        })?;
                                }
                                if let Some(adapter) = vpn_adapter.as_ref() {
                                    adapter.record_egress_frame_count(0, false);
                                }
                                *last_send.lock().await = Instant::now();
                            }
                        }
                    }
                }
                Ok::<(), ExitStreamError>(())
            }
        });

        if let Some(padding) = padding_future {
            tokio::try_join!(to_torii, from_torii, padding)?;
        } else {
            tokio::try_join!(to_torii, from_torii)?;
        }

        shutdown.notify_waiters();
        Ok(())
    }

    fn norito_padding_delay(channel_id: &[u8; 32], period: Duration, now: SystemTime) -> Duration {
        if period.is_zero() {
            return Duration::ZERO;
        }
        let period_millis = period.as_millis();
        if period_millis == 0 {
            return Duration::ZERO;
        }
        let mut seed_bytes = [0u8; 8];
        seed_bytes.copy_from_slice(&channel_id[..8]);
        let seed = u64::from_le_bytes(seed_bytes);
        let offset = u128::from(seed) % period_millis;
        let now_duration = now.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        let now_mod = now_duration.as_millis() % period_millis;
        let delta_millis = (period_millis + offset - now_mod) % period_millis;
        Duration::from_millis(delta_millis as u64)
    }

    fn kaigi_multiaddr_to_websocket(addr: &str) -> Option<String> {
        let trimmed = addr.trim();
        if trimmed.is_empty() {
            return None;
        }
        let segments: Vec<&str> = trimmed.trim_matches('/').split('/').collect();
        if segments.len() < 4 {
            return None;
        }
        let mut idx = 0;
        let proto = segments[idx];
        idx += 1;
        let host_segment = *segments.get(idx)?;
        idx += 1;
        let host = match proto {
            "ip4" | "dns" | "dns4" | "dns6" => host_segment.to_string(),
            "ip6" => format!("[{host_segment}]"),
            _ => return None,
        };
        if segments.get(idx)? != &"tcp" {
            return None;
        }
        idx += 1;
        let port_segment = *segments.get(idx)?;
        idx += 1;
        let port: u16 = port_segment.parse().ok()?;

        let mut scheme = "ws";
        if let Some(proto_segment) = segments.get(idx) {
            match *proto_segment {
                "ws" | "wss" => {
                    scheme = proto_segment;
                    idx += 1;
                }
                _ => return None,
            }
        }

        let path = if idx < segments.len() {
            format!("/{}", segments[idx..].join("/"))
        } else {
            "/".to_string()
        };

        Some(format!("{scheme}://{host}:{port}{path}"))
    }

    #[allow(clippy::too_many_arguments)]
    async fn process_measurement_stream(
        mut stream: RecvStream,
        performance: Arc<Mutex<RelayPerformanceAccumulator>>,
        relay_id: RelayId,
        incentives: Option<Arc<IncentiveLogger>>,
        privacy: Arc<PrivacyAggregator>,
        privacy_events: Arc<PrivacyEventBuffer>,
        mode: RelayMode,
        compliance: Option<Arc<ComplianceLogger>>,
        remote: SocketAddr,
    ) -> Result<(), IncentiveStreamError> {
        let mut len_buf = [0u8; 4];
        while let Some(frame) = Self::read_measurement_frame(&mut stream, &mut len_buf).await? {
            Self::handle_bandwidth_proof(
                &frame,
                &performance,
                relay_id,
                incentives.clone(),
                Arc::clone(&privacy),
                Arc::clone(&privacy_events),
                mode,
                compliance.clone(),
                remote,
            )
            .await?;
        }
        Ok(())
    }

    async fn read_measurement_frame(
        stream: &mut RecvStream,
        len_buf: &mut [u8; 4],
    ) -> Result<Option<Vec<u8>>, IncentiveStreamError> {
        if !Self::read_exact_or_eof(stream, len_buf).await? {
            return Ok(None);
        }
        let frame_len = u32::from_be_bytes(*len_buf) as usize;
        if frame_len == 0 {
            return Err(IncentiveStreamError::EmptyFrame);
        }
        if frame_len > MAX_BANDWIDTH_PROOF_FRAME_LEN {
            return Err(IncentiveStreamError::FrameTooLarge { length: frame_len });
        }
        let mut frame = vec![0u8; frame_len];
        Self::read_exact_or_eof(stream, &mut frame).await?;
        Ok(Some(frame))
    }

    async fn read_exact_or_eof(
        stream: &mut RecvStream,
        buf: &mut [u8],
    ) -> Result<bool, IncentiveStreamError> {
        if buf.is_empty() {
            return Ok(true);
        }
        match stream.read_exact(buf).await {
            Ok(()) => Ok(true),
            Err(quinn::ReadExactError::FinishedEarly(read)) => {
                if read == 0 {
                    Ok(false)
                } else {
                    Err(IncentiveStreamError::UnexpectedEof {
                        expected: buf.len(),
                        received: read,
                    })
                }
            }
            Err(quinn::ReadExactError::ReadError(error)) => Err(IncentiveStreamError::Read(error)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_bandwidth_proof(
        frame: &[u8],
        performance: &Arc<Mutex<RelayPerformanceAccumulator>>,
        relay_id: RelayId,
        incentives: Option<Arc<IncentiveLogger>>,
        privacy: Arc<PrivacyAggregator>,
        privacy_events: Arc<PrivacyEventBuffer>,
        mode: RelayMode,
        compliance: Option<Arc<ComplianceLogger>>,
        remote: SocketAddr,
    ) -> Result<(), IncentiveStreamError> {
        let mut cursor = frame;
        let proof = RelayBandwidthProofV1::decode(&mut cursor)?;
        if !cursor.is_empty() {
            return Err(IncentiveStreamError::TrailingBytes(cursor.len()));
        }

        let measurement_hex = hex::encode(proof.measurement_id);
        let verifier_label = proof.verifier_id.to_string();
        enum ProofOutcome {
            Accepted { summary: Option<EpochSummary> },
            Duplicate,
            ForeignRelay,
        }
        let outcome = if proof.relay_id != relay_id {
            ProofOutcome::ForeignRelay
        } else {
            let mut guard = performance.lock().await;
            if guard.ingest_bandwidth_proof(&proof) {
                let summary = guard
                    .summaries()
                    .into_iter()
                    .find(|snapshot| snapshot.epoch == proof.epoch);
                ProofOutcome::Accepted { summary }
            } else {
                ProofOutcome::Duplicate
            }
        };

        if let Some(logger) = compliance.as_ref() {
            let reason = match outcome {
                ProofOutcome::Accepted { .. } => None,
                ProofOutcome::Duplicate => Some("duplicate_measurement"),
                ProofOutcome::ForeignRelay => Some("foreign_relay"),
            };
            if let Err(error) = logger.log_bandwidth_proof(
                remote,
                mode,
                &proof.measurement_id,
                &proof.relay_id,
                proof.epoch,
                proof.verified_bytes,
                proof.confidence.sample_count,
                proof.confidence.jitter_p95_ms,
                proof.confidence.confidence_per_mille,
                proof.issued_at_unix,
                &verifier_label,
                matches!(outcome, ProofOutcome::Accepted { .. }),
                reason,
            ) {
                warn!(%error, "failed to write compliance log entry");
            }
        }

        match outcome {
            ProofOutcome::Accepted { summary } => {
                debug!(
                    epoch = proof.epoch,
                    verified_bytes = proof.verified_bytes,
                    measurement = %measurement_hex,
                    "accepted bandwidth proof"
                );

                if let (Some(summary), Some(logger)) = (summary, incentives) {
                    let metrics =
                        snapshot_from_summary(relay_id, &summary, SnapshotKind::Measurement);
                    if let Err(error) = logger.write_snapshot(&metrics) {
                        warn!(
                            epoch = proof.epoch,
                            ?error,
                            "failed to persist measurement snapshot"
                        );
                    }
                }

                let now = SystemTime::now();
                privacy.record_verified_bytes(now, proof.verified_bytes);
                privacy_events.record_verified_bytes(mode.into(), now, proof.verified_bytes);
            }
            ProofOutcome::Duplicate => {
                debug!(
                    epoch = proof.epoch,
                    measurement = %measurement_hex,
                    "ignored bandwidth proof (duplicate measurement_id)"
                );
            }
            ProofOutcome::ForeignRelay => {
                let relay_hex = hex::encode(proof.relay_id);
                debug!(
                    epoch = proof.epoch,
                    measurement = %measurement_hex,
                    relay = %relay_hex,
                    "ignored bandwidth proof (foreign relay)"
                );
            }
        }

        Ok(())
    }

    async fn track_runtime_uptime(
        performance: Arc<Mutex<RelayPerformanceAccumulator>>,
        epoch_window_secs: u64,
        relay_id: RelayId,
        incentives: Option<Arc<IncentiveLogger>>,
    ) {
        if epoch_window_secs == 0 {
            warn!("incentive epoch window is zero; uptime tracking disabled");
            return;
        }

        let mut last_tick = Instant::now();
        loop {
            sleep(Duration::from_secs(epoch_window_secs)).await;
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(last_tick);
            last_tick = now;

            let uptime_secs = elapsed.as_secs().min(epoch_window_secs);
            if uptime_secs == 0 {
                continue;
            }

            let epoch = current_epoch(epoch_window_secs);
            let mut guard = performance.lock().await;
            guard.record_uptime(epoch, uptime_secs, epoch_window_secs);

            if let Some(logger) = incentives.as_ref() {
                if let Some(summary) = guard
                    .summaries()
                    .into_iter()
                    .find(|snapshot| snapshot.epoch == epoch)
                {
                    drop(guard);
                    let metrics = snapshot_from_summary(relay_id, &summary, SnapshotKind::Uptime);
                    if let Err(error) = logger.write_snapshot(&metrics) {
                        warn!(epoch, ?error, "failed to persist uptime snapshot");
                    }
                } else {
                    drop(guard);
                }
            }
        }
    }

    async fn perform_handshake(
        connection: &Connection,
        context: &CircuitContext,
        remote: SocketAddr,
    ) -> Result<HandshakeOutcome, HandshakeError> {
        let (mut send, mut recv) =
            match timeout(HANDSHAKE_STREAM_TIMEOUT, connection.accept_bi()).await {
                Ok(Ok(streams)) => streams,
                Ok(Err(error)) => return Err(HandshakeError::Connection(error)),
                Err(_) => return Err(HandshakeError::Timeout("handshake stream")),
            };
        let vpn_session = context
            .vpn
            .as_ref()
            .map(|overlay| overlay.start_session(Arc::clone(&context.metrics)));
        let helper_ticket_secret = context
            .vpn
            .as_ref()
            .and_then(|overlay| overlay.config().helper_ticket_secret_bytes());
        let mut byte_guard = HandshakeByteGuard::new(&context.metrics);
        let mut puzzle_verify_micros: Option<u64> = None;

        let pow_params = context.dos.current_pow_parameters();
        let puzzle_params = context.dos.current_puzzle_parameters();
        let pow_required = context.dos.is_pow_required();
        let puzzle_enforced = pow_params.difficulty() > 0 || puzzle_params.is_some();
        let has_token_policy = context.dos.has_token_policy();

        let mut cached_client_frame: Option<Vec<u8>> = None;
        let mut puzzle_ticket_frame: Option<Vec<u8>> = None;
        let mut pending_puzzle_ticket: Option<PowTicket> = None;
        let mut pending_pow_ticket: Option<PowTicket> = None;
        let mut admission_token: Option<AdmissionToken> = None;
        let mut vpn_helper_ticket: Option<VpnHelperTicketV1> = None;

        if helper_ticket_secret.is_some() || has_token_policy || (pow_required && puzzle_enforced) {
            let first_frame = match timeout(
                HANDSHAKE_PAYLOAD_TIMEOUT,
                Self::read_handshake_frame(&mut recv),
            )
            .await
            {
                Ok(Ok(frame)) => frame,
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(HandshakeError::Timeout("pow token/ticket")),
            };
            byte_guard.add(first_frame.len() + 2);

            if let Some(secret) = helper_ticket_secret.as_ref() {
                if VpnHelperTicketV1::looks_like(&first_frame) {
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("system clock should be after unix epoch")
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64;
                    vpn_helper_ticket =
                        Some(VpnHelperTicketV1::parse(&first_frame, secret, now_ms)?);
                } else if has_token_policy && token::frame_looks_like_token(&first_frame) {
                    let token = AdmissionToken::decode(&first_frame)
                        .map_err(HandshakeError::TokenDecode)?;
                    admission_token = Some(token);
                } else if pow_required && puzzle_enforced {
                    puzzle_ticket_frame = Some(first_frame);
                } else {
                    cached_client_frame = Some(first_frame);
                }
            } else if has_token_policy && token::frame_looks_like_token(&first_frame) {
                let token =
                    AdmissionToken::decode(&first_frame).map_err(HandshakeError::TokenDecode)?;
                admission_token = Some(token);
            } else if pow_required && puzzle_enforced {
                puzzle_ticket_frame = Some(first_frame);
            } else {
                cached_client_frame = Some(first_frame);
            }
        }

        if pow_required
            && puzzle_enforced
            && puzzle_ticket_frame.is_none()
            && admission_token.is_none()
            && vpn_helper_ticket.is_none()
        {
            return Err(HandshakeError::MissingChallenge);
        }

        if let Some(frame) = puzzle_ticket_frame {
            let ticket = PowTicket::parse(&frame).map_err(HandshakeError::Pow)?;
            if puzzle_params.is_some() {
                pending_puzzle_ticket = Some(ticket);
            } else {
                pending_pow_ticket = Some(ticket);
            }
        }

        let client_frame = match cached_client_frame {
            Some(frame) => frame,
            None => match timeout(
                HANDSHAKE_PAYLOAD_TIMEOUT,
                Self::read_handshake_frame(&mut recv),
            )
            .await
            {
                Ok(Ok(frame)) => {
                    byte_guard.add(frame.len() + 2);
                    frame
                }
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(HandshakeError::Timeout("client hello")),
            },
        };

        let client_hello =
            ClientHello::parse(&client_frame).map_err(HandshakeError::ClientHello)?;
        ensure_nonzero("client nonce must not be all zeros", client_hello.nonce())?;
        ensure_nonzero(
            "client ephemeral public key must not be all zeros",
            client_hello.ephemeral_public(),
        )?;
        ensure_nonzero(
            "client KEM public key must not be all zeros",
            client_hello.kem_public(),
        )?;

        let client_caps = parse_client_advertisement(client_hello.raw_capabilities())
            .map_err(HandshakeError::Capability)?;
        let mut negotiated = negotiate_capabilities(&client_caps, context.server_caps.as_ref())
            .map_err(HandshakeError::Capability)?;
        validate_client_selection(&negotiated, client_hello.kem_id(), client_hello.sig_id())?;

        let mut response_caps = negotiated.clone();
        response_caps.grease.extend(context.grease.iter().cloned());
        let grease_entries = std::mem::take(&mut response_caps.grease);
        let relay_caps_bytes =
            encode_relay_advertisement(&response_caps, context.server_caps.role_bits);
        let relay_caps_bytes =
            update_suite_list(&relay_caps_bytes, context.handshake_suites.as_slice(), true)
                .map_err(HandshakeError::Noise)?;
        let relay_caps_bytes = append_grease_tlvs(relay_caps_bytes, &grease_entries);

        let mut rng = StdRng::from_os_rng();
        let runtime_params = NoiseRuntimeParams {
            descriptor_commit: context.descriptor_commit.as_slice(),
            client_capabilities: client_hello.raw_capabilities(),
            relay_capabilities: &relay_caps_bytes,
            kem_id: negotiated.kem.id.code(),
            sig_id: client_hello.sig_id(),
            resume_hash: client_hello.resume_hash(),
        };

        let (relay_hello, relay_state) = match process_client_hello(
            &client_frame,
            &runtime_params,
            context.identity_key.as_ref(),
            &mut rng,
        ) {
            Ok(result) => result,
            Err(NoiseHandshakeError::Downgrade {
                warnings,
                telemetry,
            }) => {
                return Err(HandshakeError::Downgrade {
                    warnings,
                    telemetry,
                });
            }
            Err(error) => return Err(HandshakeError::Noise(error)),
        };

        if let (Some(params), Some(ticket)) =
            (puzzle_params.as_ref(), pending_puzzle_ticket.as_ref())
        {
            let transcript_binding = client_hello
                .resume_hash()
                .map(|_| relay_state.transcript_hash().as_slice());
            let verify_start = Instant::now();
            verify_puzzle_ticket_binding(
                ticket,
                params,
                context.descriptor_commit.as_slice(),
                context.relay_id.as_slice(),
                transcript_binding,
            )
            .map_err(HandshakeError::Puzzle)?;
            let elapsed = verify_start.elapsed();
            context.metrics.record_puzzle_verify(elapsed);
            let micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
            puzzle_verify_micros = Some(micros);
        }

        if let Some(ticket) = pending_pow_ticket.as_ref() {
            let transcript_binding = client_hello
                .resume_hash()
                .map(|_| relay_state.transcript_hash().as_slice());
            verify_pow_ticket_binding(
                ticket,
                &pow_params,
                context.descriptor_commit.as_slice(),
                context.relay_id.as_slice(),
                transcript_binding,
            )
            .map_err(HandshakeError::Pow)?;
        }

        if let Some(token) = admission_token.as_ref() {
            context
                .dos
                .verify_token(
                    token,
                    &context.relay_id,
                    relay_state.transcript_hash(),
                    SystemTime::now(),
                )
                .map_err(HandshakeError::Token)?;
        }

        match timeout(
            HANDSHAKE_PAYLOAD_TIMEOUT,
            Self::write_handshake_frame(&mut send, &relay_hello),
        )
        .await
        {
            Ok(Ok(())) => {
                byte_guard.add(relay_hello.len() + 2);
            }
            Ok(Err(error)) => return Err(error),
            Err(_) => return Err(HandshakeError::Timeout("relay hello")),
        }

        send.finish().map_err(HandshakeError::Finish)?;

        let session = if relay_state.requires_client_finish() {
            let client_finish = match timeout(
                HANDSHAKE_PAYLOAD_TIMEOUT,
                Self::read_handshake_frame(&mut recv),
            )
            .await
            {
                Ok(Ok(frame)) => {
                    byte_guard.add(frame.len() + 2);
                    frame
                }
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(HandshakeError::Timeout("client finish")),
            };
            relay_finalize_handshake(relay_state, &client_finish, context.identity_key.as_ref())
                .map_err(HandshakeError::Noise)?
        } else {
            relay_finalize_handshake(relay_state, &[], context.identity_key.as_ref())
                .map_err(HandshakeError::Noise)?
        };

        negotiated.grease.extend(context.grease.iter().cloned());

        let handshake_bytes = byte_guard.finish();
        let vpn_session = match (vpn_session, context.vpn.as_ref()) {
            (Some(session_handle), Some(overlay)) => {
                let session_id = vpn_helper_ticket
                    .map(|ticket| ticket.session_id)
                    .unwrap_or_else(|| derive_vpn_session_id(remote, session.transcript_hash));
                Some(overlay.bind_session(session_handle, session_id))
            }
            _ => None,
        };
        Ok(HandshakeOutcome {
            negotiated,
            session,
            handshake_bytes,
            puzzle_verify_micros,
            vpn_session,
            vpn_helper_ticket,
        })
    }

    async fn read_handshake_frame(recv: &mut RecvStream) -> Result<Vec<u8>, HandshakeError> {
        let mut len_buf = [0u8; 2];
        recv.read_exact(&mut len_buf)
            .await
            .map_err(HandshakeError::Read)?;
        let len = u16::from_be_bytes(len_buf) as usize;
        if len > MAX_HANDSHAKE_FRAME_LEN {
            return Err(HandshakeError::FrameTooLarge(len));
        }
        let mut payload = vec![0u8; len];
        recv.read_exact(&mut payload)
            .await
            .map_err(HandshakeError::Read)?;
        Ok(payload)
    }

    async fn write_handshake_frame(
        send: &mut SendStream,
        payload: &[u8],
    ) -> Result<(), HandshakeError> {
        if payload.len() > MAX_HANDSHAKE_FRAME_LEN {
            return Err(HandshakeError::FrameTooLarge(payload.len()));
        }
        let len = u16::try_from(payload.len()).expect("handshake payload fits in u16");
        send.write_all(&len.to_be_bytes())
            .await
            .map_err(HandshakeError::Write)?;
        send.write_all(payload)
            .await
            .map_err(HandshakeError::Write)?;
        Ok(())
    }

    async fn render_admin_response(
        path: &str,
        context: AdminRenderContext<'_>,
        relay_id: RelayId,
        mode: RelayMode,
    ) -> String {
        if path == "/privacy/events" {
            let body = context.privacy_events.drain_ndjson();
            return format!(
                concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "content-type: {content_type}\r\n",
                    "content-length: {length}\r\n",
                    "connection: close\r\n",
                    "\r\n",
                    "{body}"
                ),
                content_type = NDJSON_CONTENT_TYPE,
                length = body.len(),
                body = body,
            );
        }
        if path == "/policy/proxy-toggle" {
            let body = context.proxy_policy_events.drain_ndjson();
            return format!(
                concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "content-type: {content_type}\r\n",
                    "content-length: {length}\r\n",
                    "connection: close\r\n",
                    "\r\n",
                    "{body}"
                ),
                content_type = NDJSON_CONTENT_TYPE,
                length = body.len(),
                body = body,
            );
        }

        let proxy_queue_depth = context.proxy_policy_events.queue_depth() as u64;
        let mut body = context.metrics.render_prometheus(mode, proxy_queue_depth);
        let incentive_summary = {
            let guard = context.performance.lock().await;
            guard.summaries()
        };
        let incentives = render_incentive_prometheus(relay_id, &incentive_summary, mode);
        let privacy_metrics = context.privacy.render_prometheus(mode, SystemTime::now());
        if !body.ends_with('\n') && !incentives.is_empty() {
            body.push('\n');
        }
        body.push_str(&incentives);
        if !body.ends_with('\n') && !privacy_metrics.is_empty() {
            body.push('\n');
        }
        body.push_str(&privacy_metrics);
        format!(
            concat!(
                "HTTP/1.1 200 OK\r\n",
                "content-type: {content_type}\r\n",
                "content-length: {length}\r\n",
                "connection: close\r\n",
                "\r\n",
                "{body}"
            ),
            content_type = PROMETHEUS_CONTENT_TYPE,
            length = body.len(),
            body = body,
        )
    }

    async fn serve_metrics(
        resources: MetricsResources,
        relay_id: RelayId,
        addr: SocketAddr,
        mode: RelayMode,
    ) -> Result<(), RelayError> {
        let MetricsResources {
            metrics,
            privacy,
            privacy_events,
            proxy_policy_events,
            performance,
        } = resources;
        let listener = TcpListener::bind(addr).await?;
        let actual = listener.local_addr()?;
        info!(listen = %actual, "metrics server listening");

        loop {
            let (mut stream, peer) = listener.accept().await?;
            let metrics = Arc::clone(&metrics);
            let privacy = Arc::clone(&privacy);
            let privacy_events = Arc::clone(&privacy_events);
            let proxy_policy_events = Arc::clone(&proxy_policy_events);
            let performance = Arc::clone(&performance);
            tokio::spawn(async move {
                debug!(peer = %peer, "serving metrics request");
                let mut request_buf = [0u8; 1024];
                let read = match stream.read(&mut request_buf).await {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        debug!(%error, "failed to read admin request");
                        return;
                    }
                };
                let request = std::str::from_utf8(&request_buf[..read]).unwrap_or_default();
                let mut lines = request.lines();
                let request_line = lines.next().unwrap_or_default();
                let path = request_line.split_whitespace().nth(1).unwrap_or("/metrics");

                let context = AdminRenderContext {
                    metrics: metrics.as_ref(),
                    privacy: privacy.as_ref(),
                    privacy_events: privacy_events.as_ref(),
                    proxy_policy_events: proxy_policy_events.as_ref(),
                    performance: performance.as_ref(),
                };

                let response =
                    RelayRuntime::render_admin_response(path, context, relay_id, mode).await;

                if let Err(error) = stream.write_all(response.as_bytes()).await {
                    debug!(%error, "failed to send metrics response");
                }
                let _ = stream.shutdown().await;
            });
        }
    }

    fn load_server_config_from_files(
        cert_path: &std::path::Path,
        key_path: &std::path::Path,
    ) -> Result<quinn::ServerConfig, RelayError> {
        let certs = Self::load_certificates(cert_path)?;
        let key = Self::load_private_key(key_path)?;
        quinn::ServerConfig::with_single_cert(certs, key)
            .map_err(|error| RelayError::Tls(error.to_string()))
    }

    fn self_signed_server_config(subject: &str) -> Result<quinn::ServerConfig, RelayError> {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec![subject.to_owned()])
                .map_err(|error| RelayError::Tls(error.to_string()))?;
        let cert_der = cert.der().clone();
        let key_der = PrivateKeyDer::try_from(signing_key.serialize_der())
            .map_err(|error| RelayError::Tls(error.to_string()))?;
        quinn::ServerConfig::with_single_cert(vec![cert_der], key_der)
            .map_err(|error| RelayError::Tls(error.to_string()))
    }

    fn load_certificates(
        path: &std::path::Path,
    ) -> Result<Vec<CertificateDer<'static>>, RelayError> {
        let data = std::fs::read(path)?;
        let mut certs = Vec::new();
        for entry in CertificateDer::pem_slice_iter(&data) {
            let cert = entry.map_err(|error| RelayError::Tls(error.to_string()))?;
            certs.push(cert.into_owned());
        }
        if certs.is_empty() {
            return Err(RelayError::Tls(
                "no certificates found in PEM file".to_string(),
            ));
        }
        Ok(certs)
    }

    fn load_private_key(path: &std::path::Path) -> Result<PrivateKeyDer<'static>, RelayError> {
        let data = std::fs::read(path)?;
        let key = PrivateKeyDer::from_pem_slice(&data)
            .map_err(|error| RelayError::Tls(error.to_string()))?;
        Ok(key.clone_key())
    }
}

#[derive(Debug, Error)]
enum ExitStreamError {
    #[error("route open frame truncated (received {received} bytes)")]
    Truncated { received: usize },
    #[error("failed to read route open frame: {0}")]
    Read(quinn::ReadError),
    #[error(transparent)]
    Decode(#[from] RouteOpenFrameError),
    #[error("{stream} exit routing disabled in configuration")]
    StreamDisabled { stream: &'static str },
    #[error("{stream} channel {channel} not provisioned")]
    RouteNotProvisioned {
        stream: &'static str,
        channel: String,
    },
    #[error(transparent)]
    RouteLookup(#[from] RouteCatalogError),
    #[error("{stream} channel {channel} requires authenticated viewers")]
    RouteRequiresAuthentication {
        stream: &'static str,
        channel: String,
    },
    #[error("{stream} adapter connection timed out after {timeout:?}")]
    AdapterTimeout {
        stream: &'static str,
        timeout: Duration,
    },
    #[error("{stream} adapter connection failed: {error}")]
    AdapterConnect {
        stream: &'static str,
        #[source]
        error: tungstenite::Error,
    },
    #[error("failed to encode {stream} handshake: {error}")]
    HandshakeEncode {
        stream: &'static str,
        #[source]
        error: norito::Error,
    },
    #[error("failed to send data to {stream} adapter: {error}")]
    AdapterSend {
        stream: &'static str,
        #[source]
        error: tungstenite::Error,
    },
    #[error("failed to receive data from {stream} adapter: {error}")]
    AdapterReceive {
        stream: &'static str,
        #[source]
        error: tungstenite::Error,
    },
    #[error("exit stream read error: {0}")]
    RecvRead(quinn::ReadError),
    #[error("exit stream write error: {0}")]
    SendWrite(quinn::WriteError),
    #[error("failed to finish exit stream: {0}")]
    SendFinish(ClosedStream),
}

#[derive(Debug, Error)]
enum IncentiveStreamError {
    #[error("measurement frame length must be non-zero")]
    EmptyFrame,
    #[error(
        "measurement frame length {length} exceeds maximum of {MAX_BANDWIDTH_PROOF_FRAME_LEN} bytes"
    )]
    FrameTooLarge { length: usize },
    #[error("measurement frame ended prematurely (received {received} of {expected} bytes)")]
    UnexpectedEof { expected: usize, received: usize },
    #[error("failed to decode relay bandwidth proof: {0}")]
    Decode(#[from] norito::codec::Error),
    #[error("measurement frame contains {0} trailing bytes after decoding proof")]
    TrailingBytes(usize),
    #[error("measurement stream read error: {0}")]
    Read(quinn::ReadError),
}

#[derive(Debug, Error)]
enum HandshakeError {
    #[error("timeout waiting for {0}")]
    Timeout(&'static str),
    #[error("connection error: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error("read error: {0}")]
    Read(quinn::ReadExactError),
    #[error("write error: {0}")]
    Write(quinn::WriteError),
    #[error("failed to close handshake stream: {0}")]
    Finish(ClosedStream),
    #[error("handshake frame exceeded maximum length ({0} bytes)")]
    FrameTooLarge(usize),
    #[error("client hello parse failed: {0}")]
    ClientHello(#[from] crate::handshake::ClientHelloError),
    #[error("capability negotiation failed: {0}")]
    Capability(#[from] CapabilityError),
    #[error("invalid client handshake material: {0}")]
    InvalidClient(&'static str),
    #[error("handshake downgrade detected")]
    Downgrade {
        warnings: Vec<CapabilityWarning>,
        telemetry: Option<Vec<u8>>,
    },
    #[error("noise handshake error: {0}")]
    Noise(NoiseHandshakeError),
    #[error("pow verification failed: {0}")]
    Pow(#[from] pow::Error),
    #[error("puzzle verification failed: {0}")]
    Puzzle(#[from] puzzle::Error),
    #[error("missing admission challenge")]
    MissingChallenge,
    #[error("token decode failed: {0}")]
    TokenDecode(TokenDecodeError),
    #[error("token verification failed: {0}")]
    Token(#[from] TokenPolicyError),
    #[error("vpn helper ticket verification failed: {0}")]
    HelperTicket(#[from] VpnHelperTicketError),
}

fn role_bits(mode: RelayMode) -> u8 {
    match mode {
        RelayMode::Entry => 0x01,
        RelayMode::Middle => 0x02,
        RelayMode::Exit => 0x04,
    }
}

fn ensure_nonzero(field: &'static str, bytes: &[u8]) -> Result<(), HandshakeError> {
    if bytes.iter().any(|&byte| byte != 0) {
        Ok(())
    } else {
        Err(HandshakeError::InvalidClient(field))
    }
}

fn resolve_handshake_suites(
    bundle: Option<&RelayCertificateBundleV2>,
) -> Result<Vec<HandshakeSuite>, ConfigError> {
    let suites = match bundle {
        Some(bundle) => bundle.certificate.handshake_suites.clone(),
        None => DEFAULT_HANDSHAKE_SUITES.to_vec(),
    };
    let mut unique = Vec::new();
    for suite in suites {
        if !unique.contains(&suite) {
            unique.push(suite);
        }
    }
    if unique.is_empty() {
        return Err(ConfigError::Handshake(
            "handshake suite list must not be empty".to_string(),
        ));
    }
    Ok(unique)
}

fn validate_client_selection(
    negotiated: &NegotiatedCapabilities,
    kem_id: u8,
    sig_id: u8,
) -> Result<(), HandshakeError> {
    if negotiated.kem.id.code() != kem_id {
        return Err(HandshakeError::InvalidClient(
            "client kem_id does not match negotiated capability",
        ));
    }
    let Some(sig) = SignatureId::from_code(sig_id) else {
        return Err(HandshakeError::InvalidClient(
            "client sig_id is not a supported signature suite",
        ));
    };
    if !negotiated.signatures.iter().any(|entry| entry.id == sig) {
        return Err(HandshakeError::InvalidClient(
            "client sig_id does not match negotiated capability",
        ));
    }
    Ok(())
}

fn append_grease_tlvs(mut base: Vec<u8>, grease: &[GreaseEntry]) -> Vec<u8> {
    for entry in grease {
        base.extend_from_slice(&entry.ty.to_be_bytes());
        base.extend_from_slice(&(entry.value.len() as u16).to_be_bytes());
        base.extend_from_slice(&entry.value);
    }
    base
}

fn downgrade_detail_from_warnings(warnings: &[CapabilityWarning]) -> Option<String> {
    let slug_source = warnings
        .iter()
        .find_map(|warning| {
            let trimmed = warning.message.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or("downgrade");
    Some(normalize_downgrade_reason(slug_source))
}

fn record_handshake_suite_downgrade(metrics: &Metrics, suite: HandshakeSuite) {
    if matches!(suite, HandshakeSuite::Nk3PqForwardSecure) {
        metrics.record_downgrade("handshake_suite_nk3");
    }
}

fn pow_failure_reason(error: &pow::Error) -> SoranetPowFailureReasonV1 {
    match error {
        pow::Error::UnsupportedVersion(_) => SoranetPowFailureReasonV1::UnsupportedVersion,
        pow::Error::DifficultyMismatch { .. } => SoranetPowFailureReasonV1::DifficultyMismatch,
        pow::Error::Expired(_, _) => SoranetPowFailureReasonV1::Expired,
        pow::Error::FutureSkewExceeded(_) => SoranetPowFailureReasonV1::FutureSkewExceeded,
        pow::Error::ExpiryWindowTooSmall(_) => SoranetPowFailureReasonV1::TtlTooShort,
        pow::Error::InvalidSolution => SoranetPowFailureReasonV1::InvalidSolution,
        pow::Error::RelayMismatch | pow::Error::TranscriptMismatch => {
            SoranetPowFailureReasonV1::RelayMismatch
        }
        pow::Error::Replay => SoranetPowFailureReasonV1::Replay,
        pow::Error::RevocationStore(_) => SoranetPowFailureReasonV1::StoreError,
        pow::Error::InvalidSignature | pow::Error::Signing(_) => {
            SoranetPowFailureReasonV1::SignatureInvalid
        }
        pow::Error::PostQuantum(_) => SoranetPowFailureReasonV1::PostQuantumError,
        pow::Error::Clock(_) => SoranetPowFailureReasonV1::ClockError,
        pow::Error::Malformed(_) => SoranetPowFailureReasonV1::UnsupportedVersion,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::ErrorKind,
        net::TcpListener as StdTcpListener,
        num::NonZeroU32,
        sync::Arc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use ed25519_dalek::SigningKey;
    use iroha_crypto::{
        Signature,
        soranet::{
            certificate::{
                CapabilityToggle, KemRotationModeV1, KemRotationPolicyV1, RelayCapabilityFlagsV1,
                RelayCertificateBundleV2, RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
            },
            handshake::HandshakeSuite,
            pow,
            puzzle::{self, ChallengeBinding as PuzzleBinding, Parameters as PuzzleParameters},
        },
    };
    use iroha_data_model::{
        account::AccountId,
        metadata::Metadata,
        soranet::{
            incentives::{BandwidthConfidenceV1, RelayBandwidthProofV1},
            privacy_metrics::{
                SoranetPowFailureReasonV1, SoranetPrivacyModeV1, SoranetPrivacyThrottleScopeV1,
            },
            vpn::{VPN_CELL_LEN, VpnCellFlagsV1, VpnCellV1},
        },
    };
    use norito::{codec::Encode, decode_from_bytes, to_bytes};
    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};
    use tempfile::NamedTempFile;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt, duplex},
        net::TcpStream,
        time::sleep,
    };

    use super::*;
    use crate::{
        capability::{
            GreaseEntry, KemAdvertisement, KemId, NegotiatedCapabilities, SignatureAdvertisement,
            SignatureId,
        },
        config::VpnConfig,
        constant_rate,
        privacy::{PrivacyAggregator, PrivacyConfig, ProxyPolicyEventBuffer},
        scheduler::CellClass,
    };

    const TEST_RELAY_ID: RelayId = [0xAB; 32];

    #[test]
    fn route_open_metrics_use_adapter_once() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = VpnOverlay::from_config(Default::default());
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xAA; 16]);
        let adapter = VpnAdapter::new(handle.session().clone(), overlay);

        record_route_open_ingress_metrics(Some(&adapter), Some(&handle));
        let snapshot = metrics.snapshot();
        let bytes = RouteOpenFrame::length() as u64;
        assert_eq!(0, snapshot.vpn_frames);
        assert_eq!(0, snapshot.vpn_ingress_frames);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(1, snapshot.vpn_control_frames);
        assert_eq!(1, snapshot.vpn_control_ingress_frames);
        assert_eq!(bytes, snapshot.vpn_control_bytes);
        assert_eq!(bytes, snapshot.vpn_control_ingress_bytes);

        record_route_open_egress_metrics(Some(&adapter), Some(&handle));
        let snapshot = metrics.snapshot();
        assert_eq!(0, snapshot.vpn_frames);
        assert_eq!(0, snapshot.vpn_egress_frames);
        assert_eq!(0, snapshot.vpn_egress_bytes);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(2, snapshot.vpn_control_frames);
        assert_eq!(1, snapshot.vpn_control_ingress_frames);
        assert_eq!(1, snapshot.vpn_control_egress_frames);
        assert_eq!(bytes * 2, snapshot.vpn_control_bytes);
        assert_eq!(bytes, snapshot.vpn_control_egress_bytes);
    }

    #[test]
    fn route_open_metrics_fallback_to_session() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = VpnOverlay::from_config(Default::default());
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xBB; 16]);

        record_route_open_ingress_metrics(None, Some(&handle));
        let snapshot = metrics.snapshot();
        let bytes = RouteOpenFrame::length() as u64;
        assert_eq!(0, snapshot.vpn_frames);
        assert_eq!(0, snapshot.vpn_ingress_frames);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(1, snapshot.vpn_control_frames);
        assert_eq!(1, snapshot.vpn_control_ingress_frames);
        assert_eq!(bytes, snapshot.vpn_control_bytes);
        assert_eq!(bytes, snapshot.vpn_control_ingress_bytes);

        record_route_open_egress_metrics(None, Some(&handle));
        let snapshot = metrics.snapshot();
        assert_eq!(0, snapshot.vpn_frames);
        assert_eq!(0, snapshot.vpn_egress_frames);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(2, snapshot.vpn_control_frames);
        assert_eq!(1, snapshot.vpn_control_ingress_frames);
        assert_eq!(1, snapshot.vpn_control_egress_frames);
        assert_eq!(bytes * 2, snapshot.vpn_control_bytes);
        assert_eq!(bytes, snapshot.vpn_control_egress_bytes);
    }

    #[test]
    fn handshake_byte_guard_does_not_touch_vpn_bytes() {
        let metrics = Arc::new(Metrics::new());
        let overlay = VpnOverlay::from_config(Default::default());
        let _session = overlay.start_session(Arc::clone(&metrics));
        let mut guard = HandshakeByteGuard::new(metrics.as_ref());

        guard.add(128);
        guard.finish();

        let snapshot = metrics.snapshot();
        assert_eq!(128, snapshot.handshake_bytes);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(0, snapshot.vpn_ingress_bytes);
    }

    #[tokio::test]
    async fn vpn_backend_bootstrap_encodes_session_address_plan() {
        let bootstrap = build_vpn_backend_bootstrap([0x5A; 16]);
        let (mut writer, mut reader) = duplex(4096);
        write_vpn_backend_bootstrap(&mut writer, &bootstrap)
            .await
            .expect("bootstrap write");

        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic).await.expect("magic");
        assert_eq!(&magic, VPN_BACKEND_BOOTSTRAP_MAGIC);

        let mut len = [0u8; 2];
        reader.read_exact(&mut len).await.expect("len");
        let len = usize::from(u16::from_be_bytes(len));
        let mut payload = vec![0u8; len];
        reader.read_exact(&mut payload).await.expect("payload");
        let decoded: VpnBackendBootstrap =
            serde_json::from_slice(&payload).expect("bootstrap json");
        assert_eq!(decoded, bootstrap);
        assert_eq!(decoded.server_tunnel_addresses.len(), 2);
        assert_eq!(decoded.session_routes.len(), 2);
    }

    #[tokio::test]
    async fn vpn_backend_status_reports_rejection_message() {
        let (mut writer, mut reader) = duplex(256);
        writer.write_all(&[0u8]).await.expect("status");
        writer.write_all(&4u16.to_be_bytes()).await.expect("len");
        writer.write_all(b"fail").await.expect("payload");
        let error = read_vpn_backend_status(&mut reader)
            .await
            .expect_err("status must reject");
        assert!(error.to_string().contains("fail"));
    }

    #[tokio::test]
    async fn vpn_backend_bridge_forwards_backend_payloads_into_vpn_frames() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = Arc::new(VpnOverlay::from_config(VpnConfig {
            enabled: true,
            ..VpnConfig::default()
        }));
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xA1; 16]);
        let adapter = VpnAdapter::new(handle.session().clone(), overlay.as_ref().clone());
        let bridge = VpnBridge::new(
            adapter.clone(),
            [0xA1; 16],
            vpn_flow_label_from_session_id([0xA1; 16]),
        );
        let (vpn_runtime, mut vpn_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut vpn_read, mut vpn_write) = tokio::io::split(vpn_runtime);
        let (backend_runtime, mut backend_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut backend_read, mut backend_write) = tokio::io::split(backend_runtime);
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];

        let bridge_task = tokio::spawn(async move {
            RelayRuntime::bridge_vpn_backend_streams(
                &mut vpn_write,
                &mut vpn_read,
                bridge,
                &adapter,
                &mut backend_read,
                &mut backend_write,
                VpnCellV1::max_payload_len(),
            )
            .await
            .expect("bridge should forward backend payload");
        });

        backend_peer
            .write_all(&payload)
            .await
            .expect("write backend payload");
        backend_peer
            .shutdown()
            .await
            .expect("shutdown backend peer");

        let parsed = crate::vpn::read_frame(overlay.as_ref(), &mut vpn_peer)
            .await
            .expect("vpn frame");
        assert_eq!(payload, parsed.payload);

        bridge_task.await.expect("bridge task joined");
    }

    #[tokio::test]
    async fn vpn_backend_bridge_forwards_vpn_payloads_into_backend_stream() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = Arc::new(VpnOverlay::from_config(VpnConfig {
            enabled: true,
            ..VpnConfig::default()
        }));
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xB2; 16]);
        let adapter = VpnAdapter::new(handle.session().clone(), overlay.as_ref().clone());
        let bridge = VpnBridge::new(
            adapter.clone(),
            [0xB2; 16],
            vpn_flow_label_from_session_id([0xB2; 16]),
        );
        let (vpn_runtime, mut vpn_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut vpn_read, mut vpn_write) = tokio::io::split(vpn_runtime);
        let (backend_runtime, mut backend_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut backend_read, mut backend_write) = tokio::io::split(backend_runtime);
        let payload = vec![0xFA, 0xCE, 0xB0, 0x0C];

        let bridge_task = tokio::spawn(async move {
            RelayRuntime::bridge_vpn_backend_streams(
                &mut vpn_write,
                &mut vpn_read,
                bridge,
                &adapter,
                &mut backend_read,
                &mut backend_write,
                VpnCellV1::max_payload_len(),
            )
            .await
            .expect("bridge should forward vpn payload");
        });

        let cell = overlay
            .data_cell(
                [0xB2; 16],
                vpn_flow_label_from_session_id([0xB2; 16]),
                0,
                0,
                VpnCellFlagsV1::new(false, false, false, false),
                payload.clone(),
            )
            .expect("vpn cell");
        let frame = overlay.pad_cell(cell).expect("vpn frame");
        crate::vpn::write_frame(&mut vpn_peer, &frame)
            .await
            .expect("write vpn frame");
        vpn_peer.shutdown().await.expect("shutdown vpn peer");

        let mut actual = vec![0u8; payload.len()];
        backend_peer
            .read_exact(&mut actual)
            .await
            .expect("read backend payload");
        assert_eq!(payload, actual);

        bridge_task.await.expect("bridge task joined");
    }

    #[test]
    fn downgrade_detail_prefers_first_non_empty_warning() {
        let warnings = vec![
            CapabilityWarning {
                capability_type: 0x0203,
                message: "   ".to_string(),
            },
            CapabilityWarning {
                capability_type: 0x0102,
                message: "Client omitted suite_list capability despite relay marking it required"
                    .to_string(),
            },
        ];
        let slug = downgrade_detail_from_warnings(&warnings);
        assert_eq!(
            slug.as_deref(),
            Some("client_suite_list_missing"),
            "expected to sanitize suite_list warning"
        );
    }

    #[test]
    fn downgrade_detail_sanitizes_constant_rate_warning() {
        let warnings = vec![CapabilityWarning {
            capability_type: 0x0203,
            message: "Constant-rate capability missing on hop".to_string(),
        }];
        let slug = downgrade_detail_from_warnings(&warnings);
        assert_eq!(
            slug.as_deref(),
            Some("constant_rate_capability_missing_on_hop"),
            "constant-rate warnings should produce deterministic slug"
        );
    }

    #[test]
    fn handshake_suite_downgrade_records_only_nk3() {
        let metrics = Metrics::new();
        record_handshake_suite_downgrade(&metrics, HandshakeSuite::Nk2Hybrid);
        record_handshake_suite_downgrade(&metrics, HandshakeSuite::Nk3PqForwardSecure);

        let snapshot = metrics.snapshot();
        assert_eq!(
            snapshot
                .downgrade_counts
                .get("handshake_suite_nk3")
                .copied(),
            Some(1)
        );
    }

    #[test]
    fn constant_rate_lane_manager_auto_disables_and_restores() {
        let spec = constant_rate::profile_by_name("core").expect("core profile");
        let registry = Arc::new(CircuitRegistry::default());
        let manager = ConstantRateLaneManager::new(spec, Arc::clone(&registry));
        let metrics = Metrics::new();
        metrics.set_constant_rate_profile(
            spec.name,
            u64::from(spec.neighbor_cap),
            spec.tick_millis,
            u64::from(spec.dummy_lane_floor),
        );

        // 7/8 neighbors => 87.5% saturation, exceeding the 85% disable threshold.
        manager.apply_active_sample(7, &metrics);
        assert_eq!(manager.current_cap(), spec.dummy_lane_floor);

        // Drop to 3/8 neighbors (37.5%), which is below the 75% restore threshold.
        manager.apply_active_sample(3, &metrics);
        assert_eq!(manager.current_cap(), spec.neighbor_cap);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.constant_rate_queue_depth, 3);
        assert_eq!(snapshot.constant_rate_active_neighbors, 3);
        assert_eq!(snapshot.constant_rate_dummy_lanes, 1);
        assert!(
            (snapshot.constant_rate_dummy_ratio - (1.0 / f64::from(spec.neighbor_cap))).abs()
                < f64::EPSILON,
            "expected dummy ratio to track remaining cover lanes"
        );
        assert!(
            (snapshot.constant_rate_slot_rate_hz - (1000.0 / spec.tick_millis)).abs()
                < f64::EPSILON,
            "slot rate gauge must reflect the profile tick"
        );
        assert_eq!(
            snapshot.constant_rate_ceiling_hits, 1,
            "saturation event should increment the ceiling counter"
        );
        assert!(
            !snapshot.constant_rate_degraded,
            "restoring the neighbor cap should reset the degraded gauge"
        );
        assert!(
            snapshot.constant_rate_saturation_percent <= 38,
            "expected saturation gauge to reflect reduced utilization"
        );
    }

    #[test]
    fn constant_rate_engine_tracks_dummy_ratio_and_queue_depth() {
        let spec = constant_rate::profile_by_name("null").expect("null profile");
        let mut engine = ConstantRateEngine::new(spec);

        let first = engine.next_cell();
        assert!(first.cell.is_dummy);
        assert_eq!(first.queues.total(), 0);
        assert_eq!(first.dummy_ratio, 1.0);

        assert!(engine.enqueue(Cell::new(CellClass::Interactive, vec![0xAA, 0xBB])));
        assert!(engine.enqueue(Cell::new(CellClass::Bulk, vec![0xCC])));

        let second = engine.next_cell();
        assert!(
            !second.cell.is_dummy,
            "queued payload should be sent ahead of dummy cells"
        );
        assert_eq!(
            second.queues,
            QueueDepths {
                control: 0,
                interactive: 1,
                bulk: 1
            }
        );
        assert!(
            second.dummy_ratio < 1.0,
            "dummy ratio should drop after sending a real cell"
        );
    }

    #[test]
    fn constant_rate_engine_drops_low_priority_on_congestion_signal() {
        let spec = constant_rate::profile_by_name("home").expect("home profile");
        let mut engine = ConstantRateEngine::new(spec);

        assert!(engine.enqueue(Cell::new(CellClass::Control, vec![0x01])));
        assert!(engine.enqueue(Cell::new(CellClass::Interactive, vec![0x02])));
        assert!(engine.enqueue(Cell::new(CellClass::Bulk, vec![0x03])));

        let action = engine
            .apply_congestion_hint(CELL_SIZE_BYTES.saturating_sub(1))
            .expect("buffer pressure should emit a congestion action");
        assert_eq!(action.dropped_class, Some(CellClass::Bulk));

        // If congestion clears, the signal should return None and avoid spurious drops.
        assert!(engine.apply_congestion_hint(CELL_SIZE_BYTES).is_none());
    }

    #[test]
    fn verify_puzzle_ticket_respects_resume_hash_binding() {
        let params = PuzzleParameters::new(
            NonZeroU32::new(1_024).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            8,
            Duration::from_secs(180),
            Duration::from_secs(45),
        );
        let descriptor = vec![0xD4; 32];
        let relay_id = vec![0xC3; 32];
        let resume_hash = [0x9Au8; 32];
        let mut rng = StdRng::from_seed([0x5Au8; 32]);

        let binding = PuzzleBinding::new(&descriptor, &relay_id, Some(&resume_hash));
        let ticket = puzzle::mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng)
            .expect("mint ticket with resume hash");
        verify_puzzle_ticket_binding(&ticket, &params, &descriptor, &relay_id, Some(&resume_hash))
            .expect("ticket should verify with matching resume hash");

        let mismatched = [0x44u8; 32];
        let err = verify_puzzle_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &relay_id,
            Some(&mismatched),
        )
        .expect_err("mismatched resume hash must fail verification");
        match err {
            puzzle::Error::InvalidSolution => {}
            other => panic!("unexpected puzzle verification error: {other:?}"),
        }

        // Ensure tickets without a resume hash continue to verify.
        let base_binding = PuzzleBinding::new(&descriptor, &relay_id, None);
        let base_ticket =
            puzzle::mint_ticket(&params, &base_binding, Duration::from_secs(180), &mut rng)
                .expect("mint ticket without resume hash");
        verify_puzzle_ticket_binding(&base_ticket, &params, &descriptor, &relay_id, None)
            .expect("ticket without resume hash should verify");
    }

    #[test]
    fn verify_puzzle_ticket_rejects_wrong_relay_binding() {
        let params = PuzzleParameters::new(
            NonZeroU32::new(4_096).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            5,
            Duration::from_secs(120),
            Duration::from_secs(30),
        );
        let descriptor = vec![0x51; 32];
        let relay_id = vec![0x42; 32];
        let resume_hash = [0x24u8; 32];
        let mut rng = StdRng::from_seed([0x91u8; 32]);

        let binding = PuzzleBinding::new(&descriptor, &relay_id, Some(&resume_hash));
        let ticket = puzzle::mint_ticket(&params, &binding, Duration::from_secs(50), &mut rng)
            .expect("mint ticket with relay binding");
        let mismatched_relay = vec![0x99; 32];

        let err = verify_puzzle_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &mismatched_relay,
            Some(&resume_hash),
        )
        .expect_err("relay mismatch must fail verification");
        match err {
            puzzle::Error::InvalidSolution => {}
            other => panic!("unexpected puzzle verification error: {other:?}"),
        }
    }

    #[test]
    fn verify_pow_ticket_rejects_wrong_relay_binding() {
        let params = PowParameters::new(16, Duration::from_secs(180), Duration::from_secs(45));
        let descriptor = [0xAA; 32];
        let relay_a = [0x01; 32];
        let relay_b = [0x02; 32];
        let mut rng = StdRng::from_seed([0x22; 32]);

        let binding = pow::ChallengeBinding::new(&descriptor, &relay_a, None);
        let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
            .expect("mint pow ticket");

        verify_pow_ticket_binding(&ticket, &params, &descriptor, &relay_a, None)
            .expect("ticket should verify with matching relay");

        let err = verify_pow_ticket_binding(&ticket, &params, &descriptor, &relay_b, None)
            .expect_err("relay mismatch must fail verification");
        match err {
            pow::Error::InvalidSolution => {}
            other => panic!("unexpected pow verification error: {other:?}"),
        }
    }

    #[test]
    fn verify_pow_ticket_respects_transcript_binding() {
        let params = PowParameters::new(16, Duration::from_secs(120), Duration::from_secs(30));
        let descriptor = [0x0C; 32];
        let relay_id = [0x0D; 32];
        let transcript = [0xFE; 32];
        let mut rng = StdRng::from_seed([0x33; 32]);

        let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, Some(&transcript));
        let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(40), &mut rng)
            .expect("mint pow ticket with transcript");

        verify_pow_ticket_binding(&ticket, &params, &descriptor, &relay_id, Some(&transcript))
            .expect("ticket should verify with matching transcript");

        let err = verify_pow_ticket_binding(&ticket, &params, &descriptor, &relay_id, None)
            .expect_err("missing transcript must fail verification");
        match err {
            pow::Error::InvalidSolution => {}
            other => panic!("unexpected pow verification error: {other:?}"),
        }
    }

    #[test]
    fn pow_failure_reason_labels_signature_and_absent_key_cases() {
        let signature = pow::Error::InvalidSignature;
        assert_eq!(
            pow_failure_reason(&signature),
            SoranetPowFailureReasonV1::SignatureInvalid
        );

        let malformed = pow::Error::Malformed("signed ticket payload".to_string());
        assert_eq!(
            pow_failure_reason(&malformed),
            SoranetPowFailureReasonV1::UnsupportedVersion
        );
    }

    #[test]
    fn norito_stream_open_roundtrip() {
        let open = NoritoStreamOpen {
            channel_id: [0xA1; 32],
            route_id: [0xB2; 32],
            stream_id: [0xC3; 32],
            authenticated: true,
            padding_budget_ms: Some(37),
            access_kind: SoranetAccessKind::Authenticated,
            exit_token: vec![0x45, 0x67, 0x89],
        };
        let bytes = to_bytes(&open).expect("encode handshake");
        let decoded: NoritoStreamOpen = decode_from_bytes(&bytes).expect("decode handshake");
        assert_eq!(decoded.channel_id, open.channel_id);
        assert_eq!(decoded.route_id, open.route_id);
        assert_eq!(decoded.stream_id, open.stream_id);
        assert_eq!(decoded.exit_token, open.exit_token);
        assert_eq!(decoded.authenticated, open.authenticated);
        assert_eq!(decoded.padding_budget_ms, open.padding_budget_ms);
        assert_eq!(decoded.access_kind, open.access_kind);
    }

    #[test]
    fn kaigi_stream_open_roundtrip() {
        let open = KaigiStreamOpen {
            channel_id: [0xAA; 32],
            route_id: [0xBB; 32],
            stream_id: [0xCC; 32],
            room_id: [0xDD; 32],
            authenticated: false,
            access_kind: SoranetAccessKind::ReadOnly,
            exit_token: vec![0x10, 0x20, 0x30],
            exit_multiaddr: "/dns/kaigi.example/tcp/9443/ws".into(),
        };
        let bytes = to_bytes(&open).expect("encode kaigi handshake");
        let decoded: KaigiStreamOpen = decode_from_bytes(&bytes).expect("decode kaigi handshake");
        assert_eq!(decoded.channel_id, open.channel_id);
        assert_eq!(decoded.route_id, open.route_id);
        assert_eq!(decoded.stream_id, open.stream_id);
        assert_eq!(decoded.room_id, open.room_id);
        assert_eq!(decoded.exit_token, open.exit_token);
        assert_eq!(decoded.exit_multiaddr, open.exit_multiaddr);
        assert_eq!(decoded.access_kind, open.access_kind);
        assert_eq!(decoded.authenticated, open.authenticated);
    }

    #[test]
    fn norito_padding_delay_matches_expected_formula() {
        let channel_id = [0x11; 32];
        let period = Duration::from_millis(100);
        let now = UNIX_EPOCH + Duration::from_millis(45);
        let delay = RelayRuntime::norito_padding_delay(&channel_id, period, now);

        let period_millis = period.as_millis();
        let mut seed_bytes = [0u8; 8];
        seed_bytes.copy_from_slice(&channel_id[..8]);
        let seed = u64::from_le_bytes(seed_bytes);
        let offset = u128::from(seed) % period_millis;
        let now_mod = now
            .duration_since(UNIX_EPOCH)
            .expect("time since epoch")
            .as_millis()
            % period_millis;
        let expected = (period_millis + offset - now_mod) % period_millis;
        assert_eq!(delay.as_millis(), expected);
    }

    #[test]
    fn norito_padding_delay_zero_when_on_schedule() {
        let channel_id = [0x42; 32];
        let period = Duration::from_millis(80);
        let period_millis = period.as_millis() as u64;
        let mut seed_bytes = [0u8; 8];
        seed_bytes.copy_from_slice(&channel_id[..8]);
        let seed = u64::from_le_bytes(seed_bytes);
        let offset = seed % period_millis;
        let now = UNIX_EPOCH + Duration::from_millis(2 * period_millis + offset);

        let delay = RelayRuntime::norito_padding_delay(&channel_id, period, now);
        assert_eq!(delay.as_millis(), 0);
    }

    #[test]
    fn kaigi_multiaddr_to_websocket_converts_basic_multiaddr() {
        let url = RelayRuntime::kaigi_multiaddr_to_websocket("/dns/kaigi.test/tcp/9443/ws")
            .expect("convert dns multiaddr");
        assert_eq!(url, "ws://kaigi.test:9443/");

        let ipv6_url = RelayRuntime::kaigi_multiaddr_to_websocket("/ip6/2001:db8::1/tcp/8443/wss")
            .expect("convert ipv6 multiaddr");
        assert_eq!(ipv6_url, "wss://[2001:db8::1]:8443/");
    }

    #[test]
    fn kaigi_multiaddr_to_websocket_rejects_invalid_multiaddr() {
        assert!(RelayRuntime::kaigi_multiaddr_to_websocket("/udp/host/9999").is_none());
        assert!(RelayRuntime::kaigi_multiaddr_to_websocket("").is_none());
    }

    fn load_config(json: &str) -> RelayConfig {
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), json).expect("write config");
        RelayConfig::load(file.path()).expect("load config")
    }

    fn sample_account(seed: u8) -> AccountId {
        let (public_key, _) = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519).into_parts();
        AccountId::new(public_key)
    }

    fn sample_bandwidth_proof(
        epoch: u32,
        measurement_seed: u8,
        verified_bytes: u128,
    ) -> RelayBandwidthProofV1 {
        let mut measurement_id = [0u8; 32];
        measurement_id.fill(measurement_seed);
        RelayBandwidthProofV1 {
            relay_id: TEST_RELAY_ID,
            measurement_id,
            epoch,
            verified_bytes,
            verifier_id: sample_account(measurement_seed),
            issued_at_unix: 1,
            confidence: BandwidthConfidenceV1 {
                sample_count: 16,
                jitter_p95_ms: 4,
                confidence_per_mille: 900,
            },
            signature: Signature::from_bytes(&[0x55; 64]),
            metadata: Metadata::default(),
        }
    }

    struct CertificateTestFixture {
        descriptor_commit: [u8; 32],
        bundle: RelayCertificateBundleV2,
        identity_seed_hex: String,
        bundle_file: NamedTempFile,
        manifest_file: NamedTempFile,
        issuer_ed25519_hex: String,
        issuer_mldsa_hex: String,
    }

    const TEST_ML_KEM_PUBLIC_LEN: usize = 1_184;
    const TEST_ML_KEM_SECRET_LEN: usize = 2_400;

    impl CertificateTestFixture {
        fn new() -> Self {
            let descriptor_commit = [0xAB; 32];
            let identity_seed = [0x11; 32];
            let identity_seed_hex = hex::encode(identity_seed);
            let identity_signing = SigningKey::from_bytes(&identity_seed);
            let identity_public = identity_signing.verifying_key();
            let identity_public_bytes = identity_public.to_bytes();

            let relay_mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
                .expect("ML-DSA keypair generation should succeed");

            let kem_policy = KemRotationPolicyV1 {
                mode: KemRotationModeV1::Static,
                preferred_suite: 0x01,
                fallback_suite: None,
                rotation_interval_hours: 0,
                grace_period_hours: 0,
            };

            let ml_kem_private = vec![0x33; TEST_ML_KEM_SECRET_LEN];
            let ml_kem_public = vec![0x44; TEST_ML_KEM_PUBLIC_LEN];

            let certificate = RelayCertificateV2 {
                relay_id: identity_public_bytes,
                identity_ed25519: identity_public_bytes,
                identity_mldsa65: relay_mldsa_keys.public_key.clone(),
                descriptor_commit,
                roles: RelayRolesV2 {
                    entry: true,
                    middle: true,
                    exit: false,
                },
                guard_weight: 25,
                bandwidth_bytes_per_sec: 1_000_000,
                reputation_weight: 50,
                endpoints: vec![RelayEndpointV2 {
                    url: "https://relay.test".to_string(),
                    priority: 0,
                    tags: vec!["norito".to_string()],
                }],
                capability_flags: RelayCapabilityFlagsV1::new(
                    CapabilityToggle::Enabled,
                    CapabilityToggle::Disabled,
                    CapabilityToggle::Enabled,
                    CapabilityToggle::Disabled,
                ),
                kem_policy,
                handshake_suites: vec![
                    HandshakeSuite::Nk3PqForwardSecure,
                    HandshakeSuite::Nk2Hybrid,
                ],
                published_at: 1,
                valid_after: 1,
                valid_until: 3_600,
                directory_hash: [0x66; 32],
                issuer_fingerprint: [0x77; 32],
                pq_kem_public: ml_kem_public.clone(),
            };

            let issuer_seed = [0x99; 32];
            let issuer_signing = SigningKey::from_bytes(&issuer_seed);
            let issuer_mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
                .expect("ML-DSA keypair generation should succeed");

            let bundle = certificate
                .issue(&issuer_signing, issuer_mldsa_keys.secret_key())
                .expect("issue certificate");

            let bundle_file = NamedTempFile::new().expect("bundle file");
            std::fs::write(bundle_file.path(), bundle.to_cbor()).expect("write bundle");

            let manifest_file = NamedTempFile::new().expect("manifest file");
            let manifest_identity_hex = identity_seed_hex.clone();
            std::fs::write(
                manifest_file.path(),
                format!(
                    r#"{{
                        "version": 1,
                        "identity": {{
                            "ed25519_private_key_hex": "{}",
                            "ml_kem_private_key_hex": "{}",
                            "ml_kem_public_hex": "{}"
                        }}
                    }}"#,
                    manifest_identity_hex,
                    hex::encode(&ml_kem_private),
                    hex::encode(&ml_kem_public)
                ),
            )
            .expect("write manifest");

            let issuer_ed25519_hex = hex::encode(issuer_signing.verifying_key().to_bytes());
            let issuer_mldsa_hex = hex::encode(issuer_mldsa_keys.public_key());

            Self {
                descriptor_commit,
                bundle,
                identity_seed_hex,
                bundle_file,
                manifest_file,
                issuer_ed25519_hex,
                issuer_mldsa_hex,
            }
        }
    }

    #[test]
    fn generates_self_signed_config() {
        let config = RelayRuntime::self_signed_server_config("relay.test");
        assert!(config.is_ok());
    }

    #[test]
    fn runtime_uses_fallback_identity_when_missing() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0"
            }
        "#;
        let config = load_config(json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        let context = runtime.circuit_context();

        let expected_private = PrivateKey::from_bytes(Algorithm::Ed25519, &FALLBACK_IDENTITY_SEED)
            .expect("fallback key to parse");
        let expected_pair =
            KeyPair::from_private_key(expected_private).expect("fallback keypair derive");

        assert_eq!(
            context.identity_key.public_key(),
            expected_pair.public_key()
        );
    }

    #[test]
    fn runtime_loads_descriptor_commit_from_certificate() {
        let fixture = CertificateTestFixture::new();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "identity_private_key_hex": "{identity_hex}",
                    "descriptor_manifest_path": "{manifest}",
                    "certificate": {{
                        "bundle_path": "{bundle}",
                        "issuer_ed25519_hex": "{issuer_ed}",
                        "issuer_mldsa_hex": "{issuer_mldsa}",
                        "validation_phase": "phase3_require_dual"
                    }}
                }}
            }}"#,
            identity_hex = fixture.identity_seed_hex,
            manifest = fixture.manifest_file.path().display(),
            bundle = fixture.bundle_file.path().display(),
            issuer_ed = fixture.issuer_ed25519_hex,
            issuer_mldsa = fixture.issuer_mldsa_hex,
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        assert_eq!(runtime.descriptor_commit(), fixture.descriptor_commit);
        let stored_bundle = runtime
            .certificate_bundle()
            .expect("certificate bundle available");
        assert_eq!(
            stored_bundle.certificate.descriptor_commit,
            fixture.descriptor_commit
        );
    }

    #[test]
    fn resolve_handshake_suites_defaults_without_certificate() {
        let suites = resolve_handshake_suites(None).expect("suites");
        assert_eq!(
            suites,
            vec![
                HandshakeSuite::Nk2Hybrid,
                HandshakeSuite::Nk3PqForwardSecure
            ]
        );
    }

    #[test]
    fn resolve_handshake_suites_uses_certificate_order() {
        let fixture = CertificateTestFixture::new();
        let suites = resolve_handshake_suites(Some(&fixture.bundle)).expect("suites");
        assert_eq!(suites, fixture.bundle.certificate.handshake_suites);
    }

    #[test]
    fn runtime_rejects_descriptor_commit_mismatch() {
        let fixture = CertificateTestFixture::new();
        let mismatch_hex = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "identity_private_key_hex": "{identity_hex}",
                    "descriptor_manifest_path": "{manifest}",
                    "descriptor_commit_hex": "{mismatch}",
                    "certificate": {{
                        "bundle_path": "{bundle}",
                        "issuer_ed25519_hex": "{issuer_ed}",
                        "issuer_mldsa_hex": "{issuer_mldsa}",
                        "validation_phase": "phase3_require_dual"
                    }}
                }}
            }}"#,
            identity_hex = fixture.identity_seed_hex,
            manifest = fixture.manifest_file.path().display(),
            bundle = fixture.bundle_file.path().display(),
            issuer_ed = fixture.issuer_ed25519_hex,
            issuer_mldsa = fixture.issuer_mldsa_hex,
            mismatch = mismatch_hex,
        );
        let config = load_config(&json);
        match RelayRuntime::new(config) {
            Err(RelayError::Config(ConfigError::Handshake(message))) => {
                assert!(
                    message.contains("descriptor_commit_hex"),
                    "unexpected error message: {message}"
                );
            }
            Err(other) => panic!("expected handshake config error, got {other:?}"),
            Ok(_) => panic!("expected mismatch to error"),
        }
    }

    #[test]
    fn runtime_requires_mldsa_key_for_phase3() {
        let fixture = CertificateTestFixture::new();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "identity_private_key_hex": "{identity_hex}",
                    "descriptor_manifest_path": "{manifest}",
                    "certificate": {{
                        "bundle_path": "{bundle}",
                        "issuer_ed25519_hex": "{issuer_ed}",
                        "validation_phase": "phase3_require_dual"
                    }}
                }}
            }}"#,
            identity_hex = fixture.identity_seed_hex,
            manifest = fixture.manifest_file.path().display(),
            bundle = fixture.bundle_file.path().display(),
            issuer_ed = fixture.issuer_ed25519_hex,
        );
        let config = load_config(&json);
        match RelayRuntime::new(config) {
            Err(RelayError::Config(ConfigError::Handshake(message))) => {
                assert!(
                    message.contains("issuer_mldsa_hex"),
                    "unexpected error message: {message}"
                );
            }
            Err(other) => panic!("expected handshake config error, got {other:?}"),
            Ok(_) => panic!("expected certificate verification to fail"),
        }
    }

    fn negotiated_caps_fixture() -> NegotiatedCapabilities {
        NegotiatedCapabilities {
            kem: KemAdvertisement {
                id: KemId::MlKem768,
                required: true,
            },
            signatures: vec![SignatureAdvertisement {
                id: SignatureId::Dilithium3,
                required: true,
            }],
            padding: 1024,
            descriptor_commit: None,
            grease: Vec::new(),
            constant_rate: None,
        }
    }

    #[test]
    fn validate_client_selection_rejects_kem_mismatch() {
        let negotiated = negotiated_caps_fixture();
        let err = validate_client_selection(
            &negotiated,
            KemId::MlKem1024.code(),
            SignatureId::Dilithium3.code(),
        )
        .expect_err("kem mismatch should fail");
        match err {
            HandshakeError::InvalidClient(field) => {
                assert_eq!(field, "client kem_id does not match negotiated capability");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn validate_client_selection_rejects_signature_mismatch() {
        let negotiated = negotiated_caps_fixture();
        let err = validate_client_selection(
            &negotiated,
            KemId::MlKem768.code(),
            SignatureId::Falcon512.code(),
        )
        .expect_err("signature mismatch should fail");
        match err {
            HandshakeError::InvalidClient(field) => {
                assert_eq!(field, "client sig_id does not match negotiated capability");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn validate_client_selection_accepts_matching_ids() {
        let negotiated = negotiated_caps_fixture();
        validate_client_selection(
            &negotiated,
            KemId::MlKem768.code(),
            SignatureId::Dilithium3.code(),
        )
        .expect("matching ids accepted");
    }

    #[test]
    fn append_grease_tlvs_preserves_order() {
        let base = vec![0xAA, 0xBB];
        let grease = vec![
            GreaseEntry {
                ty: 0x7f10,
                value: vec![0x01],
            },
            GreaseEntry {
                ty: 0x7f11,
                value: vec![0x02, 0x03],
            },
        ];
        let appended = append_grease_tlvs(base.clone(), &grease);
        let expected = [
            0xAA, 0xBB, 0x7f, 0x10, 0x00, 0x01, 0x01, 0x7f, 0x11, 0x00, 0x02, 0x02, 0x03,
        ];
        assert_eq!(appended, expected);
    }

    #[test]
    fn runtime_honours_configured_identity_key() {
        let seed_hex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "identity_private_key_hex": "{seed_hex}"
                }}
            }}"#
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        let context = runtime.circuit_context();

        let seed_bytes = hex::decode(seed_hex).expect("valid hex");
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&seed_bytes);
        let expected_private =
            PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("configured key parse");
        let expected_pair =
            KeyPair::from_private_key(expected_private).expect("configured keypair derive");

        assert_eq!(
            context.identity_key.public_key(),
            expected_pair.public_key()
        );
    }

    #[test]
    fn runtime_enables_pow_when_required() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "pow": {
                    "required": true,
                    "difficulty": 6,
                    "max_future_skew_secs": 120,
                    "min_ticket_ttl_secs": 10
                }
            }
        "#;
        let config = load_config(json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        let context = runtime.circuit_context();

        assert!(context.dos.is_pow_required());
        assert_eq!(context.dos.current_pow_parameters().difficulty(), 6);
    }

    #[test]
    fn runtime_loads_identity_from_manifest() {
        let seed_hex = "c1d1c2f493ad2db3fbc5ff0bfb8bb4e0f2c5c2d9e9caa8ffd5d38a1808fa4c55";
        let manifest = NamedTempFile::new().expect("create manifest file");
        std::fs::write(
            manifest.path(),
            format!(
                r#"{{
                    "version": 1,
                    "identity": {{
                        "ed25519_private_key_hex": "{seed_hex}"
                    }}
                }}"#
            ),
        )
        .expect("write manifest");

        let manifest_path = manifest.path().to_str().expect("path to utf-8");
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        let context = runtime.circuit_context();

        let seed_bytes = hex::decode(seed_hex).expect("valid hex");
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&seed_bytes);
        let expected_private =
            PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("manifest key parse");
        let expected_pair =
            KeyPair::from_private_key(expected_private).expect("manifest keypair derive");

        assert_eq!(
            context.identity_key.public_key(),
            expected_pair.public_key()
        );
    }

    #[test]
    fn runtime_fails_when_manifest_missing_key() {
        let manifest = NamedTempFile::new().expect("create manifest file");
        std::fs::write(
            manifest.path(),
            r#"{ "version": 1, "identity": { "note": "no private key yet" } }"#,
        )
        .expect("write manifest");

        let manifest_path = manifest.path().to_str().expect("path to utf-8");
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#
        );
        let config = load_config(&json);
        match RelayRuntime::new(config) {
            Err(RelayError::Config(ConfigError::DescriptorManifest { message, .. })) => {
                assert!(
                    message.contains("missing"),
                    "unexpected manifest error message: {message}"
                );
            }
            Err(other) => panic!("expected manifest error, got {other:?}"),
            Ok(_) => panic!("expected manifest error, got Ok(_)"),
        }
    }

    #[tokio::test]
    async fn bandwidth_proof_populates_accumulator() {
        let accumulator = Arc::new(Mutex::new(RelayPerformanceAccumulator::new(TEST_RELAY_ID)));
        let proof = sample_bandwidth_proof(7, 0x34, 1_024);
        let encoded = proof.encode();
        let config = PrivacyConfig {
            min_handshakes: 0,
            flush_delay_buckets: 1,
            force_flush_buckets: 1,
            ..PrivacyConfig::default()
        };
        let privacy = Arc::new(PrivacyAggregator::new(config));
        let privacy_events = Arc::new(PrivacyEventBuffer::new(64));
        let mode = RelayMode::Entry;
        let remote: SocketAddr = "127.0.0.1:0".parse().expect("socket addr");

        RelayRuntime::handle_bandwidth_proof(
            &encoded,
            &accumulator,
            TEST_RELAY_ID,
            None,
            Arc::clone(&privacy),
            Arc::clone(&privacy_events),
            mode,
            None,
            remote,
        )
        .await
        .expect("proof accepted");

        {
            let guard = accumulator.lock().await;
            let summaries = guard.summaries();
            assert_eq!(summaries.len(), 1);
            let summary = &summaries[0];
            assert_eq!(summary.epoch, proof.epoch);
            assert_eq!(summary.verified_bandwidth_bytes, proof.verified_bytes);
            assert_eq!(summary.measurement_ids, vec![proof.measurement_id]);
        }

        // Duplicate proof must be ignored.
        RelayRuntime::handle_bandwidth_proof(
            &encoded,
            &accumulator,
            TEST_RELAY_ID,
            None,
            Arc::clone(&privacy),
            Arc::clone(&privacy_events),
            mode,
            None,
            remote,
        )
        .await
        .expect("duplicate handled");
        let guard = accumulator.lock().await;
        let summaries = guard.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].measurement_ids.len(), 1);

        let rendered = privacy.render_prometheus(
            RelayMode::Entry,
            SystemTime::now() + Duration::from_secs(600),
        );
        assert!(
            rendered.contains("soranet_privacy_verified_bytes_total"),
            "privacy metrics missing bandwidth line: {rendered}"
        );
    }

    #[test]
    fn incentive_metrics_expose_relay_label() {
        let summary = EpochSummary {
            epoch: 3,
            uptime_seconds: 90,
            scheduled_uptime_seconds: 120,
            verified_bandwidth_bytes: 2_048,
            confidence_floor_per_mille: 875,
            measurement_ids: vec![[0x99; 32]],
        };

        let metrics = render_incentive_prometheus(TEST_RELAY_ID, &[summary], RelayMode::Entry);
        let relay_hex = hex::encode(TEST_RELAY_ID);
        assert!(
            metrics.contains(&format!("relay=\"{relay_hex}\"")),
            "metrics should include relay label: {metrics}"
        );
        assert!(
            metrics.contains("soranet_relay_bandwidth_verified_bytes_total"),
            "bandwidth metric missing: {metrics}"
        );
    }

    #[test]
    fn ensure_nonzero_accepts_non_zero_bytes() {
        let bytes = [0u8, 1, 0, 2];
        assert!(ensure_nonzero("test", &bytes).is_ok());
    }

    #[test]
    fn ensure_nonzero_rejects_all_zero_bytes() {
        let bytes = [0u8; 4];
        let err = ensure_nonzero("all zero rejected", &bytes).expect_err("should fail");
        assert!(matches!(
            err,
            HandshakeError::InvalidClient("all zero rejected")
        ));
    }

    #[tokio::test]
    async fn admin_endpoint_serves_privacy_events() {
        let metrics = Arc::new(Metrics::new());
        let privacy = Arc::new(PrivacyAggregator::new(PrivacyConfig::default()));
        let privacy_events = Arc::new(PrivacyEventBuffer::new(8));
        let proxy_policy_events = Arc::new(ProxyPolicyEventBuffer::new(8));
        let performance = Arc::new(Mutex::new(RelayPerformanceAccumulator::new(TEST_RELAY_ID)));
        let mode = RelayMode::Middle;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();
        let event_time = SystemTime::now();

        privacy_events.record_handshake_success(privacy_mode, event_time, Some(37), Some(5));
        privacy_events.record_throttle(
            privacy_mode,
            event_time,
            SoranetPrivacyThrottleScopeV1::DescriptorQuota,
        );
        proxy_policy_events.record_downgrade(privacy_mode, event_time, Some("downgrade"));

        let listener = match StdTcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping admin_endpoint_serves_privacy_events: {error}");
                return;
            }
            Err(error) => panic!("failed to bind test listener: {error}"),
        };
        let addr = listener
            .local_addr()
            .expect("retrieve listener addr for admin endpoint");
        drop(listener);

        let server = {
            let resources = MetricsResources {
                metrics: Arc::clone(&metrics),
                privacy: Arc::clone(&privacy),
                privacy_events: Arc::clone(&privacy_events),
                proxy_policy_events: Arc::clone(&proxy_policy_events),
                performance: Arc::clone(&performance),
            };
            tokio::spawn(async move {
                let _ = RelayRuntime::serve_metrics(resources, TEST_RELAY_ID, addr, mode).await;
            })
        };

        sleep(Duration::from_millis(25)).await;

        let mut stream = TcpStream::connect(addr)
            .await
            .expect("connect to admin endpoint");
        stream
            .write_all(
                b"GET /privacy/events HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("write HTTP request");

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("read HTTP response");
        let text = String::from_utf8(response).expect("response must be UTF-8");
        assert!(
            text.starts_with("HTTP/1.1 200 OK"),
            "expected 200 OK, got: {text}"
        );
        assert!(
            text.to_ascii_lowercase()
                .contains("content-type: application/x-ndjson"),
            "missing NDJSON content-type: {text}"
        );
        let body = text.split("\r\n\r\n").nth(1).unwrap_or_default();
        assert!(
            body.contains("HandshakeSuccess"),
            "handshake event missing from body: {body}"
        );
        assert!(
            body.contains("Throttle"),
            "throttle event missing from body: {body}"
        );
        assert!(
            privacy_events.drain_ndjson().is_empty(),
            "buffer should drain after serving HTTP response"
        );

        stream.shutdown().await.expect("shutdown admin stream");

        let mut downgrade_stream = TcpStream::connect(addr)
            .await
            .expect("connect to proxy policy endpoint");
        downgrade_stream
            .write_all(
                b"GET /policy/proxy-toggle HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("write downgrade HTTP request");

        let mut downgrade_response = Vec::new();
        downgrade_stream
            .read_to_end(&mut downgrade_response)
            .await
            .expect("read downgrade response");
        let downgrade_text =
            String::from_utf8(downgrade_response).expect("downgrade response must be UTF-8");
        assert!(
            downgrade_text.starts_with("HTTP/1.1 200 OK"),
            "expected downgrade endpoint to return 200 OK: {downgrade_text}"
        );
        assert!(
            downgrade_text
                .to_ascii_lowercase()
                .contains("content-type: application/x-ndjson"),
            "downgrade endpoint missing NDJSON content-type: {downgrade_text}"
        );
        assert!(
            downgrade_text.contains("\"reason\":\"downgrade\""),
            "downgrade payload missing downgrade event: {downgrade_text}"
        );
        assert!(
            downgrade_text.contains("\"detail\":\"downgrade\""),
            "downgrade payload missing slug: {downgrade_text}"
        );
        downgrade_stream
            .shutdown()
            .await
            .expect("shutdown proxy policy stream");

        server.abort();
    }

    #[tokio::test]
    async fn privacy_events_endpoint_drains_buffer() {
        let metrics = Metrics::new();
        let privacy = PrivacyAggregator::new(PrivacyConfig::default());
        let privacy_events = PrivacyEventBuffer::new(8);
        let proxy_policy_events = ProxyPolicyEventBuffer::new(8);
        let performance = Mutex::new(RelayPerformanceAccumulator::new(TEST_RELAY_ID));
        let mode = RelayMode::Entry;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();
        let sample_time = SystemTime::now();

        privacy_events.record_handshake_success(privacy_mode, sample_time, Some(42), Some(7));
        privacy_events.record_throttle(
            privacy_mode,
            sample_time,
            SoranetPrivacyThrottleScopeV1::Congestion,
        );

        let context = AdminRenderContext {
            metrics: &metrics,
            privacy: &privacy,
            privacy_events: &privacy_events,
            proxy_policy_events: &proxy_policy_events,
            performance: &performance,
        };
        let response =
            RelayRuntime::render_admin_response("/privacy/events", context, TEST_RELAY_ID, mode)
                .await;
        let parts: Vec<&str> = response.split("\r\n\r\n").collect();
        assert_eq!(parts.len(), 2, "expected HTTP header/body split");
        let headers = parts[0];
        let body = parts[1];
        assert!(
            headers.contains(NDJSON_CONTENT_TYPE),
            "response should advertize ndjson content-type: {headers}"
        );
        assert!(
            !body.trim().is_empty(),
            "expected privacy events body to contain entries"
        );
        let expected_length = format!("content-length: {}", body.len());
        assert!(
            headers.to_ascii_lowercase().contains(&expected_length),
            "content-length header should match body size: {headers}"
        );

        let drained_context = AdminRenderContext {
            metrics: &metrics,
            privacy: &privacy,
            privacy_events: &privacy_events,
            proxy_policy_events: &proxy_policy_events,
            performance: &performance,
        };
        let drained = RelayRuntime::render_admin_response(
            "/privacy/events",
            drained_context,
            TEST_RELAY_ID,
            mode,
        )
        .await;
        let drained_parts: Vec<&str> = drained.split("\r\n\r\n").collect();
        assert_eq!(drained_parts.len(), 2, "expected HTTP header/body split");
        assert!(
            drained_parts[1].is_empty(),
            "privacy event buffer should be empty after drain"
        );
        assert!(
            drained_parts[0]
                .to_ascii_lowercase()
                .contains("content-length: 0"),
            "empty response should advertise zero content length"
        );
    }

    #[test]
    fn downgrade_events_hit_metrics_and_proxy_queue() {
        let metrics = Metrics::new();
        let proxy_policy_events = ProxyPolicyEventBuffer::new(4);
        let mode = RelayMode::Entry;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();
        let warnings = vec![CapabilityWarning {
            capability_type: 0x0101,
            message: "No overlapping handshake suite between client and relay".to_string(),
        }];
        let detail = downgrade_detail_from_warnings(&warnings).expect("detail slug");
        for warning in &warnings {
            metrics.record_downgrade(&warning.message);
        }

        let event_time = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        proxy_policy_events.record_downgrade(privacy_mode, event_time, Some(&detail));

        let rendered = metrics.render_prometheus(mode, proxy_policy_events.queue_depth() as u64);
        let label_block =
            "mode=\"entry\",constant_rate_profile=\"unknown\",constant_rate_neighbors=\"0\"";
        assert!(
            rendered.contains(&format!(
                "sn16_handshake_downgrade_total{{{label_block},reason=\"{detail}\"}} 1"
            )),
            "downgrade counter missing or mislabeled: {rendered}"
        );
        assert!(
            rendered.contains(&format!(
                "soranet_proxy_policy_queue_depth{{{label_block}}} 1"
            )),
            "proxy queue depth gauge missing: {rendered}"
        );

        let ndjson = proxy_policy_events.drain_ndjson();
        assert!(
            ndjson.contains("\"reason\":\"downgrade\""),
            "proxy policy NDJSON must tag downgrade reason slug: {ndjson}"
        );
        assert!(
            ndjson.contains(&format!("\"detail\":\"{detail}\"")),
            "proxy policy NDJSON should carry the slugged detail: {ndjson}"
        );
    }
}
