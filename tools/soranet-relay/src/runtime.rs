//! Runtime orchestration for the relay daemon.
#![allow(unexpected_cfgs)]
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use iroha_crypto::{
    Algorithm, KeyPair, PrivateKey, PublicKey,
    soranet::{
        certificate::{
            RelayCertificateBundleV2, leaf_certificate_spki_sha256, select_vpn_endpoint,
        },
        handshake::{
            ClientHelloMetadata, DEFAULT_TLS_SERVER_NAME, HandshakeSuite,
            HarnessError as NoiseHandshakeError, MAX_HANDSHAKE_FRAME_LEN,
            RuntimeParams as NoiseRuntimeParams, SORANET_QUIC_ALPN, SessionSecrets,
            inspect_client_hello, process_client_hello, update_suite_list,
        },
        pow::{
            self, Parameters as PowParameters, Ticket as PowTicket, TicketRevocationStore,
            TicketRevocationStoreLimits,
        },
        puzzle::{self, ChallengeBinding as PuzzleBinding},
        record::{RecordEndpoint, RecordLayer, RecordStreamContext, RecordStreamKind},
        replay::{PersistentReplayLedger, ReplayInsertStatus, ReplayLedgerLimits},
        token::{self, AdmissionToken, DecodeError as TokenDecodeError},
    },
};
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::{
    collections::HashSet,
    fmt::{self, Write as _},
    fs,
    io::{self, Write as _},
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
iroha_crypto::define_soranet_record_io_adapters!(soranet_record_io);
#[cfg(test)]
use crate::metrics::normalize_downgrade_reason;
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
    config::{
        self, ConfigError, RelayConfig, RelayMode, read_bounded_direct_regular_file,
        read_bounded_private_regular_file,
    },
    congestion::{CongestionController, CongestionError, CongestionLease},
    constant_rate::ConstantRateProfileSpec,
    dos::{DoSControls, ThrottleReason, TokenPolicyError},
    error::RelayError,
    exit::{
        ExitRouting, ExitRoutingState, ExitStreamTag, KaigiStreamState, NoritoStreamState,
        RouteCatalogError, RouteOpenFrame, RouteOpenFrameError, derive_kaigi_room_id,
    },
    guard::{self, GuardDirectoryError},
    incentive_log::IncentiveLogger,
    incentives::{
        BandwidthProofIngest, EpochSummary, INCENTIVE_MAX_ACTIVE_EPOCHS_V1, IncentiveCapacityError,
        RelayPerformanceAccumulator,
    },
    metrics::{Metrics, VpnRuntimeState},
    privacy::{
        PrivacyAggregator, PrivacyEventBuffer, ProxyPolicyEventBuffer, RejectReason, ThrottleScope,
    },
    scheduler::{
        CELL_SIZE_BYTES, Cell, CellClass, CellScheduler, OverflowPolicy, QueueDepths,
        SchedulerConfig,
    },
    vpn::{VpnFrameIoError, VpnOverlay, VpnSessionHandle, VpnSettlementArtifact},
    vpn_adapter::{VpnAdapter, VpnBridge},
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
            VPN_DEFAULT_TUNNEL_MTU_BYTES, VPN_USAGE_VOUCHER_CONTROL_MAGIC, VpnCellClassV1,
            VpnFlowLabelV1, VpnHelperTicketError, VpnHelperTicketV1, VpnSignedSessionReceiptV1,
            VpnTariffV1, VpnUsageVoucherEnvelopeV1, VpnUsageVoucherV1,
            derive_vpn_session_address_plan_v1, vpn_tariff_meter_hash_v1,
        },
    },
};
use iroha_primitives::{json::Json, numeric::Quantity};
use norito::{
    DecodeLimits, NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
    streaming::SoranetAccessKind,
    to_bytes,
};
use quinn::{
    ClosedStream, Connection, Dir, Endpoint, Incoming, RecvStream, SendStream, Side, StreamId,
    TransportConfig, VarInt, crypto::rustls::QuicServerConfig as QuinnRustlsServerConfig,
};
use rand::{SeedableRng, rngs::StdRng};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use sha2::{Digest as _, Sha256};
use soranet_record_io::{RecordReader, RecordWriter};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream, UnixStream},
    sync::{Mutex, Notify, OwnedSemaphorePermit, Semaphore},
    task::JoinHandle,
    time::{Instant as TokioInstant, MissedTickBehavior, interval, interval_at, sleep, timeout},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, protocol::Message},
};
use tracing::{debug, info, warn};
struct AdminRenderContext<'a> {
    metrics: &'a Metrics,
    privacy: &'a PrivacyAggregator,
    privacy_events: &'a PrivacyEventBuffer,
    proxy_policy_events: &'a ProxyPolicyEventBuffer,
    performance: &'a Mutex<RelayPerformanceAccumulator>,
}
struct VpnBackendBridgeContext<'a> {
    bridge: VpnBridge,
    adapter: &'a VpnAdapter,
    vpn_session: &'a VpnSessionHandle,
    voucher_authorization: Arc<Mutex<VpnVoucherAuthorization>>,
    settlement_store: Arc<VpnSettlementStore>,
    mtu: usize,
}
#[derive(Clone)]
struct AdminResources {
    metrics: Arc<Metrics>,
    privacy: Arc<PrivacyAggregator>,
    privacy_events: Arc<PrivacyEventBuffer>,
    proxy_policy_events: Arc<ProxyPolicyEventBuffer>,
    performance: Arc<Mutex<RelayPerformanceAccumulator>>,
}
struct AdminAuthorization {
    token_hash: blake3::Hash,
}
impl fmt::Debug for AdminAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminAuthorization")
            .field("token_hash", &"<redacted>")
            .finish()
    }
}
impl Drop for AdminAuthorization {
    fn drop(&mut self) {
        self.token_hash = blake3::Hash::from([0_u8; 32]);
        std::hint::black_box(&mut self.token_hash);
    }
}
impl AdminAuthorization {
    fn load(path: &Path) -> Result<Self, ConfigError> {
        let mut bytes = read_bounded_private_regular_file(
            path,
            258,
            "SoraNet relay admin authentication token",
        )
        .map_err(|error| {
            ConfigError::Admin(format!(
                "failed to read admin_auth_token_path ({}): {error}",
                path.display()
            ))
        })?;
        let token_hash = Self::hash_token(&bytes);
        bytes.fill(0);
        std::hint::black_box(bytes.as_mut_slice());
        Ok(Self {
            token_hash: token_hash?,
        })
    }
    fn hash_token(bytes: &[u8]) -> Result<blake3::Hash, ConfigError> {
        let token = std::str::from_utf8(bytes).map_err(|_| {
            ConfigError::Admin("admin authentication token must be valid UTF-8".to_string())
        })?;
        let token = token.trim_end_matches(['\r', '\n']);
        if !(32..=256).contains(&token.len()) {
            return Err(ConfigError::Admin(
                "admin authentication token must contain 32 to 256 bytes".to_string(),
            ));
        }
        if !token.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(ConfigError::Admin(
                "admin authentication token must contain only printable non-whitespace ASCII"
                    .to_string(),
            ));
        }
        Ok(blake3::hash(token.as_bytes()))
    }
    fn matches(&self, candidate: &str) -> bool {
        // `blake3::Hash` equality is constant-time, so the secret is never
        // compared byte-by-byte with an attacker-controlled prefix.
        self.token_hash == blake3::hash(candidate.as_bytes())
    }
}
const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4";
const NDJSON_CONTENT_TYPE: &str = "application/x-ndjson";
const PLAIN_TEXT_CONTENT_TYPE: &str = "text/plain; charset=utf-8";
const SORANET_HANDSHAKE_LOG_TARGET: &str = "soranet.handshake";
const HANDSHAKE_STREAM_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_PAYLOAD_TIMEOUT: Duration = Duration::from_secs(5);
const ADMIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const ADMIN_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
const ADMIN_MAX_HEADER_BYTES_V1: usize = 16 * 1024;
const ADMIN_MAX_CONCURRENT_CONNECTIONS_V1: usize = 64;
// A permit is held from validated `Incoming` admission through both the TLS
// and signed SoraNet application handshakes. This bounds unauthenticated work
// even when peers stop advancing after address validation.
const QUIC_MAX_PENDING_HANDSHAKES_V1: usize = 64;
const QUIC_TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const QUIC_APPLICATION_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const QUIC_MAX_INCOMING_V1: usize = QUIC_MAX_PENDING_HANDSHAKES_V1;
const QUIC_INCOMING_BUFFER_BYTES_V1: u64 = 64 * 1024;
const QUIC_TOTAL_INCOMING_BUFFER_BYTES_V1: u64 =
    QUIC_INCOMING_BUFFER_BYTES_V1 * QUIC_MAX_INCOMING_V1 as u64;
const QUIC_MAX_BIDI_STREAMS_V1: u32 = 32;
const QUIC_MAX_UNI_STREAMS_V1: u32 = 8;
const QUIC_STREAM_RECEIVE_WINDOW_BYTES_V1: u32 = 256 * 1024;
const QUIC_CONNECTION_RECEIVE_WINDOW_BYTES_V1: u32 = 4 * 1024 * 1024;
const QUIC_SEND_WINDOW_BYTES_V1: u64 = 4 * 1024 * 1024;
const QUIC_CRYPTO_BUFFER_BYTES_V1: usize = 64 * 1024;
const QUIC_DATAGRAM_BUFFER_BYTES_V1: usize = 64 * 1024;
const QUIC_MAX_IDLE_TIMEOUT_MILLIS_V1: u32 = 30_000;
// First-release bounds for the TLS identity artifacts read during relay
// startup. Sixteen certificates leave ample room for a complete issuer chain,
// while the byte ceilings cover ordinary PEM expansion without allowing a
// corrupt local path to define startup allocation.
const TLS_CERTIFICATE_CHAIN_MAX_BYTES_V1: usize = 1024 * 1024;
const TLS_CERTIFICATE_CHAIN_MAX_ENTRIES_V1: usize = 16;
const TLS_PRIVATE_KEY_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum on-disk JSON size of one first-release VPN settlement spool record.
pub const VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1: usize = 64 * 1024;
const VPN_SETTLEMENT_WAL_PREFIX_V1: &str = ".vpn-settlement-live-";
const VPN_SETTLEMENT_WAL_SUFFIX_V1: &str = ".wal";
const VPN_SETTLEMENT_TEMP_PREFIX_V1: &str = ".vpn-settlement-tmp-";
const VPN_SETTLEMENT_OWNER_LOCK_V1: &str = ".vpn-settlement-owner.lock";
static VPN_SETTLEMENT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const VPN_HELPER_TICKET_REPLAY_NAMESPACE: &[u8] =
    b"iroha.soranet.vpn.helper-ticket.consumptions.v1";
const VPN_HELPER_TICKET_REPLAY_ID_DOMAIN: &[u8] = b"iroha.soranet.vpn.helper-ticket.replay-id.v1";
/// Provisional epoch window for incentive aggregation (1 hour).
const INCENTIVE_EPOCH_WINDOW_SECS: u64 = 60 * 60;
/// Maximum Norito-encoded bandwidth proof payload accepted over the QUIC measurement stream.
const MAX_BANDWIDTH_PROOF_FRAME_LEN: usize = 4 * 1024;
const BANDWIDTH_PROOF_DECODE_LIMITS_V1: DecodeLimits =
    DecodeLimits::new(128, MAX_BANDWIDTH_PROOF_FRAME_LEN, 512, 64 * 1024, 16);
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
fn record_stream_context(stream_id: StreamId) -> RecordStreamContext {
    let initiator = match stream_id.initiator() {
        Side::Client => RecordEndpoint::Client,
        Side::Server => RecordEndpoint::Relay,
    };
    let kind = match stream_id.dir() {
        Dir::Bi => RecordStreamKind::Bidirectional,
        Dir::Uni => RecordStreamKind::Unidirectional,
    };
    RecordStreamContext::new(initiator, kind, stream_id.index())
}
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
    vpn_helper_ticket_replays: Option<Arc<StdMutex<PersistentReplayLedger>>>,
    vpn_settlement_store: Option<Arc<VpnSettlementStore>>,
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
    let (algorithm, payload) = identity_key
        .public_key()
        .try_to_bytes()
        .map_err(|err| RelayError::Crypto(format!("malformed relay identity public key: {err}")))?;
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
struct IncentiveMetricsWriter {
    output: String,
    max_bytes: usize,
}
impl IncentiveMetricsWriter {
    fn new(max_bytes: usize) -> Result<Self, fmt::Error> {
        let mut output = String::new();
        output
            .try_reserve_exact(max_bytes)
            .map_err(|_| fmt::Error)?;
        Ok(Self { output, max_bytes })
    }
}
impl fmt::Write for IncentiveMetricsWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self
            .output
            .len()
            .checked_add(value.len())
            .ok_or(fmt::Error)?;
        if next > self.max_bytes {
            return Err(fmt::Error);
        }
        self.output.push_str(value);
        Ok(())
    }
}
include!("runtime/incentive_metrics.rs");
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
    summary: EpochSummary,
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
        measurement_ids: summary.measurement_ids,
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
struct RelayClientHelloPreflight {
    metadata: ClientHelloMetadata,
    negotiated: NegotiatedCapabilities,
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
    #[error("vpn usage voucher error: {0}")]
    UsageVoucher(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VpnBackendSocketIdentity {
    device: u64,
    inode: u64,
}
#[derive(Debug, Clone)]
struct VpnVoucherAuthorization {
    session_id: [u8; 16],
    quote_id: [u8; 32],
    relay_id: RelayId,
    metering_public_key: PublicKey,
    tariff: VpnTariffV1,
    valid_after_ms: u64,
    expires_at_ms: u64,
    max_credit_bytes: u64,
    max_credit_active_ms: u64,
    service_started_at: Option<TokioInstant>,
    observed_ingress_bytes: u64,
    observed_egress_bytes: u64,
    authorized_ingress_bytes: u64,
    authorized_egress_bytes: u64,
    signed_active_ms: u64,
    authorized_active_ms: u64,
    authorized_fee_ceiling: Quantity,
    highest_sequence: u64,
    last_issued_at_ms: Option<u64>,
    has_voucher: bool,
}
impl VpnVoucherAuthorization {
    fn new(
        helper_ticket: &VpnHelperTicketV1,
        max_credit_bytes: u64,
        max_credit_active_ms: u64,
    ) -> Self {
        Self {
            session_id: helper_ticket.session_id,
            quote_id: helper_ticket.quote_id,
            relay_id: helper_ticket.relay_id,
            metering_public_key: helper_ticket.metering_public_key.clone(),
            tariff: helper_ticket.tariff.clone(),
            valid_after_ms: helper_ticket.valid_after_ms,
            expires_at_ms: helper_ticket.expires_at_ms,
            max_credit_bytes,
            max_credit_active_ms,
            service_started_at: None,
            observed_ingress_bytes: 0,
            observed_egress_bytes: 0,
            authorized_ingress_bytes: 0,
            authorized_egress_bytes: 0,
            signed_active_ms: 0,
            authorized_active_ms: 0,
            authorized_fee_ceiling: Quantity::zero(),
            highest_sequence: 0,
            last_issued_at_ms: None,
            has_voucher: false,
        }
    }
    fn begin_service(&mut self) {
        self.service_started_at
            .get_or_insert_with(TokioInstant::now);
    }
    fn observed_active_ms(&self) -> u64 {
        self.service_started_at.map_or(0, |started_at| {
            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
        })
    }
    fn active_deadline(&self) -> Result<TokioInstant, VpnBackendBridgeError> {
        let started_at = self.service_started_at.ok_or_else(|| {
            VpnBackendBridgeError::UsageVoucher(
                "vpn service cannot start before an initial prepaid voucher".to_owned(),
            )
        })?;
        started_at
            .checked_add(Duration::from_millis(self.authorized_active_ms))
            .ok_or_else(|| {
                VpnBackendBridgeError::UsageVoucher(
                    "voucher active-time authorization exceeds the monotonic clock range"
                        .to_owned(),
                )
            })
    }
    fn authorize_ingress_packet(
        &mut self,
        bytes: u64,
    ) -> Result<TokioInstant, VpnBackendBridgeError> {
        let next = self
            .observed_ingress_bytes
            .checked_add(bytes)
            .ok_or_else(|| {
                VpnBackendBridgeError::UsageVoucher(
                    "vpn ingress usage counter overflowed its prepaid authorization".to_owned(),
                )
            })?;
        self.ensure_authorized(next, self.observed_egress_bytes)?;
        self.observed_ingress_bytes = next;
        self.active_deadline()
    }
    fn authorize_egress_packet(
        &mut self,
        bytes: u64,
    ) -> Result<TokioInstant, VpnBackendBridgeError> {
        let next = self
            .observed_egress_bytes
            .checked_add(bytes)
            .ok_or_else(|| {
                VpnBackendBridgeError::UsageVoucher(
                    "vpn egress usage counter overflowed its prepaid authorization".to_owned(),
                )
            })?;
        self.ensure_authorized(self.observed_ingress_bytes, next)?;
        self.observed_egress_bytes = next;
        self.active_deadline()
    }
    fn accept_envelope(
        &mut self,
        envelope: &VpnUsageVoucherEnvelopeV1,
    ) -> Result<VpnUsageVoucherEnvelopeV1, VpnBackendBridgeError> {
        self.accept_envelope_at(envelope, unix_time_ms(SystemTime::now()))
    }
    fn accept_envelope_at(
        &mut self,
        envelope: &VpnUsageVoucherEnvelopeV1,
        now_ms: u64,
    ) -> Result<VpnUsageVoucherEnvelopeV1, VpnBackendBridgeError> {
        let voucher = &envelope.voucher;
        let body = &voucher.body;
        if body.session_id != self.session_id {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher session id does not match helper ticket".to_string(),
            ));
        }
        if body.quote_id != self.quote_id {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher quote id does not match helper ticket".to_string(),
            ));
        }
        if body.relay_id != self.relay_id {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher relay id does not match helper ticket".to_string(),
            ));
        }
        if voucher.client_public_key != self.metering_public_key {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher public key does not match helper ticket".to_string(),
            ));
        }
        voucher.verify().map_err(|error| {
            VpnBackendBridgeError::UsageVoucher(format!(
                "voucher signature verification failed: {error}"
            ))
        })?;
        if self.has_voucher && body.sequence <= self.highest_sequence {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher sequence must increase".to_string(),
            ));
        }
        if body.ingress_bytes < self.authorized_ingress_bytes
            || body.egress_bytes < self.authorized_egress_bytes
        {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher counters must be cumulative".to_string(),
            ));
        }
        if body.active_ms < self.signed_active_ms {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher active time must be cumulative".to_string(),
            ));
        }
        if body.issued_at_ms < self.valid_after_ms || body.issued_at_ms >= self.expires_at_ms {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher issuance time falls outside the signed helper-ticket window".to_string(),
            ));
        }
        if body.issued_at_ms > now_ms {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher issuance time is ahead of the relay clock".to_string(),
            ));
        }
        if now_ms.saturating_sub(body.issued_at_ms) > self.max_credit_active_ms {
            return Err(VpnBackendBridgeError::UsageVoucher(format!(
                "voucher issuance time is older than {}ms",
                self.max_credit_active_ms
            )));
        }
        if self
            .last_issued_at_ms
            .is_some_and(|last| body.issued_at_ms < last)
        {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher issuance time must not move backwards".to_string(),
            ));
        }
        if body.ingress_bytes < self.observed_ingress_bytes
            || body.egress_bytes < self.observed_egress_bytes
        {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher byte ceilings are already below relay-observed user payload".to_string(),
            ));
        }
        let observed_active_ms = self.observed_active_ms();
        if body.active_ms < observed_active_ms {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher active-time ceiling is already below relay-observed service time"
                    .to_string(),
            ));
        }
        let ingress_credit = body
            .ingress_bytes
            .saturating_sub(self.observed_ingress_bytes);
        let egress_credit = body.egress_bytes.saturating_sub(self.observed_egress_bytes);
        if ingress_credit > self.max_credit_bytes || egress_credit > self.max_credit_bytes {
            return Err(VpnBackendBridgeError::UsageVoucher(format!(
                "voucher byte credit exceeds the configured per-direction limit of {}B",
                self.max_credit_bytes
            )));
        }
        let active_credit = body.active_ms.saturating_sub(observed_active_ms);
        if !self.has_voucher && active_credit > self.max_credit_active_ms {
            return Err(VpnBackendBridgeError::UsageVoucher(format!(
                "initial voucher active-time credit exceeds the configured limit of {}ms",
                self.max_credit_active_ms
            )));
        }
        if !self.has_voucher
            && (body.ingress_bytes == 0 || body.egress_bytes == 0 || body.active_ms == 0)
        {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "initial voucher must preauthorize non-zero ingress, egress, and active-time credit"
                    .to_owned(),
            ));
        }
        if envelope.fee_ceiling > self.tariff.lease_fee {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher fee ceiling exceeds the escrowed helper-ticket lease fee".to_owned(),
            ));
        }
        if envelope.fee_ceiling < self.authorized_fee_ceiling {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher fee ceiling must be cumulative".to_string(),
            ));
        }
        let fee_ceiling = self.tariff.fee_ceiling(body).map_err(|error| {
            VpnBackendBridgeError::UsageVoucher(format!(
                "voucher tariff arithmetic failed: {error}"
            ))
        })?;
        if envelope.fee_ceiling != fee_ceiling {
            return Err(VpnBackendBridgeError::UsageVoucher(
                "voucher fee ceiling does not match helper ticket tariff".to_string(),
            ));
        }
        self.has_voucher = true;
        self.highest_sequence = body.sequence;
        self.authorized_ingress_bytes = body.ingress_bytes;
        self.authorized_egress_bytes = body.egress_bytes;
        self.signed_active_ms = body.active_ms;
        // Helper time starts before backend readiness, so later vouchers can
        // include unused setup time. Honor at most one configured window ahead
        // of relay-observed service while retaining the full signed ceiling for
        // monotonic settlement verification.
        self.authorized_active_ms = body
            .active_ms
            .min(observed_active_ms.saturating_add(self.max_credit_active_ms));
        self.authorized_fee_ceiling = fee_ceiling.clone();
        self.last_issued_at_ms = Some(body.issued_at_ms);
        Ok(VpnUsageVoucherEnvelopeV1 {
            voucher: voucher.clone(),
            fee_ceiling,
        })
    }
    fn ensure_authorized(
        &self,
        ingress_bytes: u64,
        egress_bytes: u64,
    ) -> Result<(), VpnBackendBridgeError> {
        let active_ms = self.observed_active_ms();
        if ingress_bytes > self.authorized_ingress_bytes {
            return Err(VpnBackendBridgeError::UsageVoucher(format!(
                "vpn ingress packet would exceed the signed prepaid ceiling of {}B",
                self.authorized_ingress_bytes
            )));
        }
        if egress_bytes > self.authorized_egress_bytes {
            return Err(VpnBackendBridgeError::UsageVoucher(format!(
                "vpn egress packet would exceed the signed prepaid ceiling of {}B",
                self.authorized_egress_bytes
            )));
        }
        if active_ms >= self.authorized_active_ms {
            return Err(VpnBackendBridgeError::UsageVoucher(format!(
                "vpn service time reached the signed prepaid ceiling of {}ms",
                self.authorized_active_ms
            )));
        }
        Ok(())
    }
}
#[derive(Debug, Default)]
struct VpnPacketStreamDecoder {
    buffer: Vec<u8>,
    expected_len: Option<usize>,
}
impl VpnPacketStreamDecoder {
    fn ingest(
        &mut self,
        framed: &[u8],
        max_packet_len: usize,
    ) -> Result<Vec<Vec<u8>>, VpnBackendBridgeError> {
        self.buffer.extend_from_slice(framed);
        let mut packets = Vec::new();
        loop {
            if self.expected_len.is_none() {
                if self.buffer.len() < 2 {
                    break;
                }
                let packet_len = usize::from(u16::from_be_bytes([self.buffer[0], self.buffer[1]]));
                self.buffer.drain(..2);
                if packet_len == 0 || packet_len > max_packet_len {
                    self.buffer.clear();
                    return Err(VpnBackendBridgeError::BackendControl(format!(
                        "vpn packet-stream frame length {packet_len} is outside 1..={max_packet_len}"
                    )));
                }
                self.expected_len = Some(packet_len);
            }
            let packet_len = self
                .expected_len
                .expect("packet length is established before payload decoding");
            if self.buffer.len() < packet_len {
                break;
            }
            packets.push(self.buffer.drain(..packet_len).collect());
            self.expected_len = None;
        }
        Ok(packets)
    }
}
fn encode_vpn_packet_stream_frame(packet: &[u8]) -> Result<Vec<u8>, VpnBackendBridgeError> {
    let packet_len = u16::try_from(packet.len()).map_err(|_| {
        VpnBackendBridgeError::BackendControl(format!(
            "vpn packet length {} exceeds the first-release framing limit",
            packet.len()
        ))
    })?;
    let mut framed = Vec::with_capacity(2 + packet.len());
    framed.extend_from_slice(&packet_len.to_be_bytes());
    framed.extend_from_slice(packet);
    Ok(framed)
}
fn decode_usage_voucher_control(
    payload: &[u8],
) -> Result<Option<VpnUsageVoucherEnvelopeV1>, VpnBackendBridgeError> {
    if !payload.starts_with(VPN_USAGE_VOUCHER_CONTROL_MAGIC) {
        return Ok(None);
    }
    let mut body = &payload[VPN_USAGE_VOUCHER_CONTROL_MAGIC.len()..];
    let envelope = VpnUsageVoucherEnvelopeV1::decode(&mut body).map_err(|error| {
        VpnBackendBridgeError::UsageVoucher(format!(
            "voucher control payload decode failed: {error}"
        ))
    })?;
    if !body.is_empty() {
        return Err(VpnBackendBridgeError::UsageVoucher(
            "voucher control payload has trailing bytes".to_string(),
        ));
    }
    Ok(Some(envelope))
}
async fn accept_initial_usage_voucher<R: AsyncRead + Unpin>(
    adapter: &VpnAdapter,
    vpn_reader: &mut R,
) -> Result<VpnUsageVoucherEnvelopeV1, VpnBackendBridgeError> {
    let cell = adapter.read_ingress_frame(vpn_reader).await?;
    if cell.header.class != VpnCellClassV1::Control {
        return Err(VpnBackendBridgeError::UsageVoucher(
            "the first VPN tunnel cell must be a prepaid usage voucher".to_owned(),
        ));
    }
    let envelope = decode_usage_voucher_control(&cell.payload)?.ok_or_else(|| {
        VpnBackendBridgeError::UsageVoucher(
            "the first VPN tunnel control cell must carry a prepaid usage voucher".to_owned(),
        )
    })?;
    Ok(envelope)
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
struct VpnBackendBootstrap {
    session_id_hex: String,
    server_tunnel_addresses: Vec<String>,
    client_ipv4_address: [u8; 4],
    client_ipv6_address: [u8; 16],
    session_routes: Vec<String>,
    mtu_bytes: u16,
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
struct VpnBackendBootstrapEnvelope {
    bootstrap: VpnBackendBootstrap,
    timestamp_ms: u64,
    nonce: [u8; 16],
    mac: [u8; 32],
}
#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct VpnSettlementSubmitRequestArtifact {
    relay_receipt_hex: String,
    client_voucher_hex: String,
    lease_id_hex: String,
}
#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct VpnSettlementSpoolRecord {
    version: u8,
    generated_at_ms: u64,
    session_id_hex: String,
    quote_id_hex: String,
    payment_tx_hash_hex: String,
    earned_fee: Quantity,
    torii_receipt_path: String,
    submit_receipt_request: VpnSettlementSubmitRequestArtifact,
}
#[derive(Debug, Clone, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
struct VpnSettlementWalRecord {
    version: u8,
    phase: String,
    helper_ticket_replay_id_hex: String,
    valid_after_ms: u64,
    expires_at_ms: u64,
    tariff: VpnTariffV1,
    reserved_settlement: VpnSettlementSpoolRecord,
}
#[derive(Debug, Clone, Copy)]
enum VpnSettlementWalPhase {
    PreService,
    ServiceReserved,
    Finalizing,
}
impl VpnSettlementWalPhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PreService => "pre_service",
            Self::ServiceReserved => "service_reserved",
            Self::Finalizing => "finalizing",
        }
    }
}
#[derive(Debug)]
struct VpnSettlementStore {
    spool_dir: PathBuf,
    _owner_lock: fs::File,
    operation: StdMutex<()>,
    poisoned: AtomicBool,
}
fn unix_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
fn unix_time_ms_for_artifact() -> u64 {
    unix_time_ms(SystemTime::now())
}
fn vpn_settlement_spool_record(
    artifact: &VpnSettlementArtifact,
    generated_at_ms: u64,
) -> VpnSettlementSpoolRecord {
    let relay_receipt_hex = hex::encode(artifact.receipt.encode());
    let client_voucher_hex = hex::encode(artifact.voucher.encode());
    let lease_id_hex = hex::encode(artifact.lease_id);
    VpnSettlementSpoolRecord {
        version: 1,
        generated_at_ms,
        session_id_hex: hex::encode(artifact.receipt.receipt.session_id),
        quote_id_hex: hex::encode(artifact.receipt.receipt.quote_id),
        payment_tx_hash_hex: hex::encode(artifact.receipt.receipt.payment_tx_hash),
        earned_fee: artifact.earned_fee.clone(),
        torii_receipt_path: "/v1/vpn/receipts".to_owned(),
        submit_receipt_request: VpnSettlementSubmitRequestArtifact {
            relay_receipt_hex,
            client_voucher_hex,
            lease_id_hex,
        },
    }
}
fn vpn_settlement_wal_record(
    session: &VpnSessionHandle,
    artifact: &VpnSettlementArtifact,
    phase: VpnSettlementWalPhase,
) -> Result<VpnSettlementWalRecord, String> {
    let tariff = session
        .tariff()
        .cloned()
        .ok_or_else(|| "vpn settlement WAL is missing the signed session tariff".to_owned())?;
    Ok(VpnSettlementWalRecord {
        version: 1,
        phase: phase.as_str().to_owned(),
        helper_ticket_replay_id_hex: hex::encode(vpn_helper_ticket_replay_id_from_parts(
            &artifact.receipt.receipt.relay_id,
            &artifact.receipt.receipt.session_id,
        )),
        valid_after_ms: session.valid_after_ms(),
        expires_at_ms: session.expires_at_ms(),
        tariff,
        reserved_settlement: vpn_settlement_spool_record(artifact, unix_time_ms_for_artifact()),
    })
}
#[cfg(unix)]
fn create_private_vpn_spool_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(config::O_NOFOLLOW_FLAG)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != config::effective_uid()?
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "VPN settlement artifact must be an owner-owned, single-link regular file with mode 0600",
        ));
    }
    Ok(file)
}
#[cfg(not(unix))]
fn create_private_vpn_spool_file(_path: &Path) -> io::Result<fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "secure VPN settlement spooling requires Unix filesystem custody guarantees",
    ))
}
#[cfg(unix)]
fn open_private_vpn_spool_lock(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(config::O_NOFOLLOW_FLAG)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != config::effective_uid()?
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "VPN settlement owner lock must be an owner-owned, single-link regular file with mode 0600",
        ));
    }
    file.try_lock().map_err(|error| {
        io::Error::other(format!(
            "failed to acquire exclusive VPN settlement spool ownership: {error}"
        ))
    })?;
    let metadata = file.metadata()?;
    let path_metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.uid() != config::effective_uid()?
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o777 != 0o600
        || path_metadata.dev() != metadata.dev()
        || path_metadata.ino() != metadata.ino()
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "VPN settlement owner lock changed identity during acquisition",
        ));
    }
    Ok(file)
}
#[cfg(not(unix))]
fn open_private_vpn_spool_lock(_path: &Path) -> io::Result<fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "secure VPN settlement spool ownership requires Unix file locks",
    ))
}
fn sync_vpn_spool_directory(spool_dir: &Path) -> Result<(), String> {
    fs::File::open(spool_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "failed to sync vpn settlement spool directory `{}`: {error}",
                spool_dir.display()
            )
        })
}
fn private_vpn_spool_file_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
                if !metadata.is_file()
                    || metadata.uid()
                        != config::effective_uid().map_err(|error| error.to_string())?
                    || metadata.nlink() != 1
                    || metadata.permissions().mode() & 0o777 != 0o600
                {
                    return Err(format!(
                        "vpn settlement path `{}` is not an owner-owned, single-link regular file with mode 0600",
                        path.display()
                    ));
                }
            }
            #[cfg(not(unix))]
            {
                let _ = metadata;
                return Err(
                    "secure VPN settlement file validation requires Unix custody guarantees"
                        .to_owned(),
                );
            }
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "failed to inspect vpn settlement path `{}`: {error}",
            path.display()
        )),
    }
}
fn vpn_settlement_json_bytes<T: norito::json::JsonSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = norito::json::to_vec_pretty(value)
        .map_err(|error| format!("failed to encode vpn settlement JSON: {error}"))?;
    bytes.push(b'\n');
    if bytes.len() > VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1 {
        return Err(format!(
            "vpn settlement record is {} bytes; first-release limit is {VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1} bytes",
            bytes.len()
        ));
    }
    Ok(bytes)
}
fn write_private_vpn_spool_file_atomically(
    spool_dir: &Path,
    destination: &Path,
    bytes: &[u8],
) -> Result<(), String> {
    let sequence = VPN_SETTLEMENT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = unix_time_ms_for_artifact();
    let temp_path = spool_dir.join(format!(
        "{VPN_SETTLEMENT_TEMP_PREFIX_V1}{timestamp}-{sequence}.tmp"
    ));
    let write_result = (|| {
        let mut file = create_private_vpn_spool_file(&temp_path).map_err(|error| {
            format!(
                "failed to create vpn settlement temporary file `{}`: {error}",
                temp_path.display()
            )
        })?;
        file.write_all(bytes).map_err(|error| {
            format!(
                "failed to write vpn settlement temporary file `{}`: {error}",
                temp_path.display()
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "failed to sync vpn settlement temporary file `{}`: {error}",
                temp_path.display()
            )
        })?;
        drop(file);
        fs::rename(&temp_path, destination).map_err(|error| {
            format!(
                "failed to atomically install vpn settlement file `{}`: {error}",
                destination.display()
            )
        })?;
        sync_vpn_spool_directory(spool_dir)
    })();
    if let Err(error) = write_result {
        if private_vpn_spool_file_exists(&temp_path).unwrap_or(false) {
            let _ = fs::remove_file(&temp_path);
            let _ = sync_vpn_spool_directory(spool_dir);
        }
        return Err(error);
    }
    Ok(())
}
fn vpn_settlement_wal_path(spool_dir: &Path, record: &VpnSettlementSpoolRecord) -> PathBuf {
    spool_dir.join(format!(
        "{VPN_SETTLEMENT_WAL_PREFIX_V1}{}-{}-{}{VPN_SETTLEMENT_WAL_SUFFIX_V1}",
        record.session_id_hex, record.quote_id_hex, record.submit_receipt_request.lease_id_hex,
    ))
}
fn vpn_settlement_final_path(
    spool_dir: &Path,
    record: &VpnSettlementSpoolRecord,
    voucher_sequence: u64,
) -> PathBuf {
    spool_dir.join(format!(
        "vpn-settlement-{}-{}-{}-seq{voucher_sequence}.json",
        record.session_id_hex, record.quote_id_hex, record.submit_receipt_request.lease_id_hex,
    ))
}
fn decode_vpn_spool_payload(
    record: &VpnSettlementSpoolRecord,
) -> Result<(VpnSignedSessionReceiptV1, VpnUsageVoucherV1, [u8; 32]), String> {
    let receipt_bytes = hex::decode(&record.submit_receipt_request.relay_receipt_hex)
        .map_err(|error| format!("vpn settlement receipt hex is invalid: {error}"))?;
    let voucher_bytes = hex::decode(&record.submit_receipt_request.client_voucher_hex)
        .map_err(|error| format!("vpn settlement voucher hex is invalid: {error}"))?;
    let lease_bytes = hex::decode(&record.submit_receipt_request.lease_id_hex)
        .map_err(|error| format!("vpn settlement lease id hex is invalid: {error}"))?;
    let mut receipt_input = receipt_bytes.as_slice();
    let receipt = VpnSignedSessionReceiptV1::decode(&mut receipt_input)
        .map_err(|error| format!("vpn settlement receipt decode failed: {error}"))?;
    if !receipt_input.is_empty() {
        return Err("vpn settlement receipt has trailing bytes".to_owned());
    }
    let mut voucher_input = voucher_bytes.as_slice();
    let voucher = VpnUsageVoucherV1::decode(&mut voucher_input)
        .map_err(|error| format!("vpn settlement voucher decode failed: {error}"))?;
    if !voucher_input.is_empty() {
        return Err("vpn settlement voucher has trailing bytes".to_owned());
    }
    let lease_id: [u8; 32] = lease_bytes
        .try_into()
        .map_err(|_| "vpn settlement lease id must be exactly 32 bytes".to_owned())?;
    Ok((receipt, voucher, lease_id))
}
fn validate_vpn_settlement_wal(
    wal: &VpnSettlementWalRecord,
) -> Result<(VpnSignedSessionReceiptV1, VpnUsageVoucherV1), String> {
    if wal.version != 1 {
        return Err("vpn settlement WAL version must be exactly 1".to_owned());
    }
    if !matches!(
        wal.phase.as_str(),
        "pre_service" | "service_reserved" | "finalizing"
    ) {
        return Err("vpn settlement WAL phase is not recognized".to_owned());
    }
    if wal.valid_after_ms >= wal.expires_at_ms {
        return Err("vpn settlement WAL has an empty signed time window".to_owned());
    }
    let record = &wal.reserved_settlement;
    if record.version != 1 || record.torii_receipt_path != "/v1/vpn/receipts" {
        return Err("vpn settlement WAL carries an invalid submit target".to_owned());
    }
    let (signed_receipt, voucher, lease_id) = decode_vpn_spool_payload(record)?;
    if record.submit_receipt_request.lease_id_hex != hex::encode(lease_id) {
        return Err("vpn settlement WAL lease id is not canonical".to_owned());
    }
    voucher
        .verify()
        .map_err(|error| format!("vpn settlement WAL voucher signature is invalid: {error}"))?;
    signed_receipt.verify().map_err(|error| {
        format!("vpn settlement WAL relay receipt signature is invalid: {error}")
    })?;
    let receipt = &signed_receipt.receipt;
    let active_ms = receipt
        .ended_at_ms
        .checked_sub(receipt.started_at_ms)
        .ok_or_else(|| "vpn settlement WAL receipt interval is inverted".to_owned())?;
    if receipt.started_at_ms < wal.valid_after_ms || receipt.ended_at_ms > wal.expires_at_ms {
        return Err("vpn settlement WAL receipt falls outside the signed ticket window".to_owned());
    }
    if voucher.body.issued_at_ms < wal.valid_after_ms
        || voucher.body.issued_at_ms >= wal.expires_at_ms
        || voucher.body.issued_at_ms > receipt.ended_at_ms
    {
        return Err("vpn settlement WAL voucher timestamp is not consensus-valid".to_owned());
    }
    if receipt.session_id != voucher.body.session_id
        || receipt.quote_id != voucher.body.quote_id
        || receipt.relay_id != voucher.body.relay_id
    {
        return Err("vpn settlement WAL receipt/voucher identity mismatch".to_owned());
    }
    if record.session_id_hex != hex::encode(receipt.session_id)
        || record.quote_id_hex != hex::encode(receipt.quote_id)
        || record.payment_tx_hash_hex != hex::encode(receipt.payment_tx_hash)
    {
        return Err("vpn settlement WAL metadata does not match its receipt".to_owned());
    }
    if receipt.client_voucher_hash != voucher.hash()
        || receipt.highest_voucher_sequence != voucher.body.sequence
    {
        return Err("vpn settlement WAL does not commit to its embedded voucher".to_owned());
    }
    if !voucher
        .body
        .authorizes(receipt.ingress_bytes, receipt.egress_bytes, active_ms)
    {
        return Err("vpn settlement WAL usage exceeds the signed prepaid ceilings".to_owned());
    }
    let expected_uptime = u32::try_from(active_ms.div_ceil(1_000))
        .map_err(|_| "vpn settlement WAL active time exceeds receipt range".to_owned())?;
    if receipt.uptime_secs != expected_uptime || receipt.cover_bytes != 0 {
        return Err("vpn settlement WAL receipt accounting is not canonical".to_owned());
    }
    if receipt.meter_hash != vpn_tariff_meter_hash_v1(&wal.tariff) {
        return Err("vpn settlement WAL tariff commitment mismatch".to_owned());
    }
    let earned_fee = wal
        .tariff
        .fee_for_usage(receipt.ingress_bytes, receipt.egress_bytes, active_ms)
        .map_err(|error| format!("vpn settlement WAL tariff arithmetic failed: {error}"))?;
    if receipt.earned_fee != earned_fee || record.earned_fee != earned_fee {
        return Err(
            "vpn settlement WAL earned fee does not match actual reserved usage".to_owned(),
        );
    }
    match wal.phase.as_str() {
        "pre_service"
            if receipt.ingress_bytes != 0
                || receipt.egress_bytes != 0
                || active_ms != 0
                || receipt.uptime_secs != 0
                || !receipt.earned_fee.is_zero()
                || receipt.started_at_ms != voucher.body.issued_at_ms =>
        {
            return Err(
                "pre-service VPN settlement WAL must reserve exactly zero usage and fee".to_owned(),
            );
        }
        "service_reserved"
            if receipt.ingress_bytes != voucher.body.ingress_bytes
                || receipt.egress_bytes != voucher.body.egress_bytes
                || active_ms == 0 =>
        {
            return Err(
                "service-reserved VPN settlement WAL must retain the exact signed byte ceilings and non-zero time credit"
                    .to_owned(),
            );
        }
        _ => {}
    }
    let replay_id = vpn_helper_ticket_replay_id_from_parts(&receipt.relay_id, &receipt.session_id);
    if wal.helper_ticket_replay_id_hex != hex::encode(replay_id) {
        return Err("vpn settlement WAL replay identifier mismatch".to_owned());
    }
    Ok((signed_receipt, voucher))
}
impl VpnSettlementStore {
    fn open(
        spool_dir: &Path,
        replay_ledger: &StdMutex<PersistentReplayLedger>,
    ) -> Result<Self, String> {
        let spool_dir = config::trusted_private_directory_path(
            spool_dir,
            "VPN settlement receipt spool directory",
        )
        .map_err(|error| {
            format!(
                "vpn receipt spool directory `{}` is not secure: {error}",
                spool_dir.display()
            )
        })?;
        let owner_lock_path = spool_dir.join(VPN_SETTLEMENT_OWNER_LOCK_V1);
        let owner_lock = open_private_vpn_spool_lock(&owner_lock_path).map_err(|error| {
            format!(
                "failed to open vpn settlement owner lock `{}`: {error}",
                owner_lock_path.display()
            )
        })?;
        owner_lock.sync_all().map_err(|error| {
            format!(
                "failed to sync vpn settlement owner lock `{}`: {error}",
                owner_lock_path.display()
            )
        })?;
        sync_vpn_spool_directory(&spool_dir)?;
        let store = Self {
            spool_dir,
            _owner_lock: owner_lock,
            operation: StdMutex::new(()),
            poisoned: AtomicBool::new(false),
        };
        store.recover(replay_ledger)?;
        Ok(store)
    }
    fn ensure_healthy(&self) -> Result<(), String> {
        if self.poisoned.load(Ordering::Acquire) {
            Err("vpn settlement persistence is poisoned; refusing further service".to_owned())
        } else {
            Ok(())
        }
    }
    fn poison<T>(&self, error: String) -> Result<T, String> {
        self.poisoned.store(true, Ordering::Release);
        Err(error)
    }
    fn operation_guard(&self) -> Result<std::sync::MutexGuard<'_, ()>, String> {
        self.ensure_healthy()?;
        self.operation.lock().map_err(|_| {
            self.poisoned.store(true, Ordering::Release);
            "vpn settlement persistence operation lock is poisoned".to_owned()
        })
    }
    fn write_initial_reservation(
        &self,
        session: &VpnSessionHandle,
        artifact: &VpnSettlementArtifact,
    ) -> Result<PathBuf, String> {
        let _guard = self.operation_guard()?;
        let wal = vpn_settlement_wal_record(session, artifact, VpnSettlementWalPhase::PreService)?;
        let path = vpn_settlement_wal_path(&self.spool_dir, &wal.reserved_settlement);
        if private_vpn_spool_file_exists(&path)? {
            return Err(format!(
                "vpn settlement WAL `{}` already owns this session",
                path.display()
            ));
        }
        let bytes = vpn_settlement_json_bytes(&wal)?;
        if let Err(error) = write_private_vpn_spool_file_atomically(&self.spool_dir, &path, &bytes)
        {
            return self.poison(error);
        }
        Ok(path)
    }
    fn replace_reservation(
        &self,
        session: &VpnSessionHandle,
        artifact: &VpnSettlementArtifact,
        phase: VpnSettlementWalPhase,
    ) -> Result<PathBuf, String> {
        let _guard = self.operation_guard()?;
        let wal = vpn_settlement_wal_record(session, artifact, phase)?;
        let path = vpn_settlement_wal_path(&self.spool_dir, &wal.reserved_settlement);
        match private_vpn_spool_file_exists(&path) {
            Ok(true) => {}
            Ok(false) => {
                return self.poison(format!(
                    "vpn settlement WAL `{}` disappeared before durable advancement",
                    path.display()
                ));
            }
            Err(error) => return self.poison(error),
        }
        let bytes = vpn_settlement_json_bytes(&wal)?;
        if let Err(error) = write_private_vpn_spool_file_atomically(&self.spool_dir, &path, &bytes)
        {
            return self.poison(error);
        }
        Ok(path)
    }
    fn discard_initial_reservation(&self, path: &Path) -> Result<(), String> {
        let _guard = self.operation_guard()?;
        match private_vpn_spool_file_exists(path) {
            Ok(true) => {}
            Ok(false) => return Ok(()),
            Err(error) => return self.poison(error),
        }
        if let Err(error) = fs::remove_file(path) {
            return self.poison(format!(
                "failed to remove unredeemed vpn settlement WAL `{}`: {error}",
                path.display()
            ));
        }
        if let Err(error) = sync_vpn_spool_directory(&self.spool_dir) {
            return self.poison(error);
        }
        Ok(())
    }
    fn finalize(
        &self,
        session: &VpnSessionHandle,
        artifact: &VpnSettlementArtifact,
    ) -> Result<PathBuf, String> {
        let _guard = self.operation_guard()?;
        let wal = vpn_settlement_wal_record(session, artifact, VpnSettlementWalPhase::Finalizing)?;
        let wal_path = vpn_settlement_wal_path(&self.spool_dir, &wal.reserved_settlement);
        match private_vpn_spool_file_exists(&wal_path) {
            Ok(true) => {}
            Ok(false) => {
                return self.poison(format!(
                    "vpn settlement WAL `{}` is missing at finalization",
                    wal_path.display()
                ));
            }
            Err(error) => return self.poison(error),
        }
        let wal_bytes = vpn_settlement_json_bytes(&wal)?;
        if let Err(error) =
            write_private_vpn_spool_file_atomically(&self.spool_dir, &wal_path, &wal_bytes)
        {
            return self.poison(error);
        }
        let final_path = vpn_settlement_final_path(
            &self.spool_dir,
            &wal.reserved_settlement,
            artifact.voucher.body.sequence,
        );
        match private_vpn_spool_file_exists(&final_path) {
            Ok(false) => {
                let final_bytes = vpn_settlement_json_bytes(&wal.reserved_settlement)?;
                if let Err(error) = write_private_vpn_spool_file_atomically(
                    &self.spool_dir,
                    &final_path,
                    &final_bytes,
                ) {
                    return self.poison(error);
                }
            }
            Ok(true) => {
                let existing = read_bounded_private_regular_file(
                    &final_path,
                    VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
                    "VPN settlement final artifact",
                )
                .map_err(|error| error.to_string())?;
                let existing: VpnSettlementSpoolRecord = norito::json::from_slice(&existing)
                    .map_err(|error| {
                        format!("existing vpn settlement final artifact is invalid: {error}")
                    })?;
                if existing != wal.reserved_settlement {
                    return self.poison(format!(
                        "vpn settlement final artifact `{}` conflicts with the WAL",
                        final_path.display()
                    ));
                }
            }
            Err(error) => return self.poison(error),
        }
        if let Err(error) = fs::remove_file(&wal_path) {
            return self.poison(format!(
                "failed to remove finalized vpn settlement WAL `{}`: {error}",
                wal_path.display()
            ));
        }
        if let Err(error) = sync_vpn_spool_directory(&self.spool_dir) {
            return self.poison(error);
        }
        Ok(final_path)
    }
    fn recover(&self, replay_ledger: &StdMutex<PersistentReplayLedger>) -> Result<(), String> {
        let mut wal_paths = Vec::new();
        let mut temp_paths = Vec::new();
        for entry in fs::read_dir(&self.spool_dir).map_err(|error| {
            format!(
                "failed to enumerate vpn settlement spool `{}`: {error}",
                self.spool_dir.display()
            )
        })? {
            let entry = entry.map_err(|error| {
                format!("failed to inspect vpn settlement spool entry: {error}")
            })?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with(VPN_SETTLEMENT_WAL_PREFIX_V1)
                && name.ends_with(VPN_SETTLEMENT_WAL_SUFFIX_V1)
            {
                wal_paths.push(entry.path());
            } else if name.starts_with(VPN_SETTLEMENT_TEMP_PREFIX_V1) && name.ends_with(".tmp") {
                temp_paths.push(entry.path());
            }
        }
        wal_paths.sort();
        temp_paths.sort();
        for path in temp_paths {
            private_vpn_spool_file_exists(&path)?;
            fs::remove_file(&path).map_err(|error| {
                format!(
                    "failed to remove interrupted vpn settlement temporary file `{}`: {error}",
                    path.display()
                )
            })?;
            sync_vpn_spool_directory(&self.spool_dir)?;
        }
        for wal_path in wal_paths {
            let bytes = read_bounded_private_regular_file(
                &wal_path,
                VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
                "VPN settlement recovery WAL",
            )
            .map_err(|error| {
                format!(
                    "failed to read vpn settlement recovery WAL `{}`: {error}",
                    wal_path.display()
                )
            })?;
            let wal: VpnSettlementWalRecord =
                norito::json::from_slice(&bytes).map_err(|error| {
                    format!(
                        "failed to decode vpn settlement recovery WAL `{}`: {error}",
                        wal_path.display()
                    )
                })?;
            let (signed_receipt, voucher) = validate_vpn_settlement_wal(&wal)?;
            let receipt = &signed_receipt.receipt;
            let expected_wal = vpn_settlement_wal_path(&self.spool_dir, &wal.reserved_settlement);
            if wal_path != expected_wal {
                return Err(format!(
                    "vpn settlement WAL filename `{}` does not match its authenticated session metadata",
                    wal_path.display()
                ));
            }
            recover_vpn_helper_ticket_replay(
                replay_ledger,
                vpn_helper_ticket_replay_id_from_parts(&receipt.relay_id, &receipt.session_id),
                wal.expires_at_ms,
                unix_time_ms(SystemTime::now()),
            )?;
            let final_path = vpn_settlement_final_path(
                &self.spool_dir,
                &wal.reserved_settlement,
                voucher.body.sequence,
            );
            match private_vpn_spool_file_exists(&final_path) {
                Ok(false) => {
                    let final_bytes = vpn_settlement_json_bytes(&wal.reserved_settlement)?;
                    write_private_vpn_spool_file_atomically(
                        &self.spool_dir,
                        &final_path,
                        &final_bytes,
                    )?;
                }
                Ok(true) => {
                    let existing = read_bounded_private_regular_file(
                        &final_path,
                        VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
                        "VPN settlement recovered final artifact",
                    )
                    .map_err(|error| error.to_string())?;
                    let existing: VpnSettlementSpoolRecord = norito::json::from_slice(&existing)
                        .map_err(|error| {
                            format!(
                                "existing recovered vpn settlement artifact is invalid: {error}"
                            )
                        })?;
                    if existing != wal.reserved_settlement {
                        return Err(format!(
                            "vpn settlement recovery target `{}` conflicts with its WAL",
                            final_path.display()
                        ));
                    }
                }
                Err(error) => return Err(error),
            }
            fs::remove_file(&wal_path).map_err(|error| {
                format!(
                    "failed to remove recovered vpn settlement WAL `{}`: {error}",
                    wal_path.display()
                )
            })?;
            sync_vpn_spool_directory(&self.spool_dir)?;
        }
        Ok(())
    }
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
fn vpn_flow_label_from_session_id(session_id: [u8; 16]) -> VpnFlowLabelV1 {
    let value = (u32::from(session_id[0]) << 16)
        | (u32::from(session_id[1]) << 8)
        | u32::from(session_id[2]);
    VpnFlowLabelV1::from_u32(value).expect("three-byte flow label should always fit")
}
fn build_vpn_backend_bootstrap(helper_ticket: &VpnHelperTicketV1) -> VpnBackendBootstrap {
    let address_plan = derive_vpn_session_address_plan_v1(helper_ticket.session_id);
    VpnBackendBootstrap {
        session_id_hex: hex::encode(helper_ticket.session_id),
        server_tunnel_addresses: address_plan.server_tunnel_addresses,
        client_ipv4_address: helper_ticket.client_ipv4_address,
        client_ipv6_address: helper_ticket.client_ipv6_address,
        session_routes: address_plan.session_routes,
        mtu_bytes: VPN_DEFAULT_TUNNEL_MTU_BYTES,
    }
}
fn vpn_backend_bootstrap_mac(
    secret: &[u8; 32],
    bootstrap: &VpnBackendBootstrap,
    timestamp_ms: u64,
    nonce: &[u8; 16],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_keyed(secret);
    hasher.update(b"soranet-vpn-backend-bootstrap-v1");
    hasher.update(&bootstrap.encode());
    hasher.update(&timestamp_ms.to_be_bytes());
    hasher.update(nonce);
    *hasher.finalize().as_bytes()
}
fn vpn_backend_bootstrap_envelope(
    bootstrap: &VpnBackendBootstrap,
    secret: &[u8; 32],
) -> VpnBackendBootstrapEnvelope {
    let timestamp_ms = unix_time_ms_for_artifact();
    let nonce = rand::random::<[u8; 16]>();
    let mac = vpn_backend_bootstrap_mac(secret, bootstrap, timestamp_ms, &nonce);
    VpnBackendBootstrapEnvelope {
        bootstrap: bootstrap.clone(),
        timestamp_ms,
        nonce,
        mac,
    }
}
async fn write_vpn_backend_bootstrap<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bootstrap: &VpnBackendBootstrap,
    secret: &[u8; 32],
) -> Result<(), VpnBackendBridgeError> {
    let payload = vpn_backend_bootstrap_envelope(bootstrap, secret).encode();
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
#[cfg(unix)]
fn inspect_vpn_backend_socket(
    path: &Path,
    expected_uid: u32,
    expected_gid: u32,
) -> io::Result<VpnBackendSocketIdentity> {
    let canonical = config::trusted_vpn_backend_socket_path(path, expected_uid)?;
    if canonical != path {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "VPN backend Unix socket path {} is not the startup-pinned canonical path {}",
                path.display(),
                canonical.display()
            ),
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "VPN backend endpoint {} must be a direct Unix socket",
                path.display()
            ),
        ));
    }
    if metadata.uid() != expected_uid || metadata.gid() != expected_gid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "VPN backend Unix socket owner {}/{} does not match pinned backend {expected_uid}/{expected_gid}",
                metadata.uid(),
                metadata.gid()
            ),
        ));
    }
    if metadata.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "VPN backend Unix socket must have exactly one hard link",
        ));
    }
    if metadata.permissions().mode() & 0o007 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "VPN backend Unix socket must have no permissions for other users",
        ));
    }
    Ok(VpnBackendSocketIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
#[cfg(unix)]
fn verify_vpn_backend_peer_credentials(
    backend: &UnixStream,
    expected_uid: u32,
    expected_gid: u32,
) -> io::Result<()> {
    let credentials = backend.peer_cred()?;
    if credentials.uid() != expected_uid || credentials.gid() != expected_gid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "VPN backend peer credentials {}/{} do not match pinned backend {expected_uid}/{expected_gid}",
                credentials.uid(),
                credentials.gid()
            ),
        ));
    }
    Ok(())
}
#[cfg(unix)]
async fn connect_authenticated_vpn_backend(
    path: &Path,
    expected_uid: u32,
    expected_gid: u32,
) -> io::Result<UnixStream> {
    let expected_socket = inspect_vpn_backend_socket(path, expected_uid, expected_gid)?;
    let backend = UnixStream::connect(path).await?;
    // Authenticate the connected process before inspecting protocol data or
    // sending the keyed bootstrap. Filesystem checks alone do not authenticate
    // the process that accepted this particular connection.
    verify_vpn_backend_peer_credentials(&backend, expected_uid, expected_gid)?;
    let connected_socket = inspect_vpn_backend_socket(path, expected_uid, expected_gid)?;
    if connected_socket != expected_socket {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "VPN backend Unix socket inode changed while connecting",
        ));
    }
    Ok(backend)
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
    server_config: quinn::ServerConfig,
    transport_trust: Option<Arc<RelayTransportTrust>>,
    admin_authorization: Option<Arc<AdminAuthorization>>,
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
    vpn_helper_ticket_replays: Option<Arc<StdMutex<PersistentReplayLedger>>>,
    vpn_settlement_store: Option<Arc<VpnSettlementStore>>,
    ticket_replays: Arc<StdMutex<TicketReplayState>>,
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
    transport_trust: Option<Arc<RelayTransportTrust>>,
    exit_routing: Arc<ExitRoutingState>,
    incentives: Option<Arc<IncentiveLogger>>,
    lane_manager: Arc<ConstantRateLaneManager>,
    vpn: Option<Arc<VpnOverlay>>,
    vpn_helper_ticket_replays: Option<Arc<StdMutex<PersistentReplayLedger>>>,
    vpn_settlement_store: Option<Arc<VpnSettlementStore>>,
    ticket_replays: Arc<StdMutex<TicketReplayState>>,
}
#[derive(Debug, Clone)]
struct RelayTransportTrust {
    quic_multiaddr: String,
    tls_server_name: String,
    tls_spki_sha256: [u8; 32],
    relay_certificate_sha256: [u8; 32],
    directory_snapshot_digest: [u8; 32],
    valid_until_ms: u64,
}
#[derive(Debug)]
struct TicketReplayState {
    persisted: TicketRevocationStore,
    pending: HashSet<[u8; 32]>,
    capacity: usize,
}
fn absolute_replay_state_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}
impl TicketReplayState {
    fn load(config: &config::PowConfig, now: SystemTime) -> Result<Self, ConfigError> {
        let capacity = usize::try_from(config.revocation_store_capacity).map_err(|_| {
            ConfigError::TicketReplayStore(format!(
                "pow.revocation_store_capacity {} exceeds this platform's usize",
                config.revocation_store_capacity
            ))
        })?;
        let limits = TicketRevocationStoreLimits::new(
            capacity,
            Duration::from_secs(config.revocation_store_ttl_secs),
        )
        .map_err(|error| ConfigError::TicketReplayStore(error.to_string()))?;
        let replay_path = absolute_replay_state_path(&config.revocation_store_path)
            .map_err(|error| ConfigError::TicketReplayStore(error.to_string()))?;
        let persisted = TicketRevocationStore::load(replay_path, limits, now)
            .map_err(|error| ConfigError::TicketReplayStore(error.to_string()))?;
        Ok(Self {
            persisted,
            pending: HashSet::new(),
            capacity,
        })
    }
}
fn verify_and_consume_ticket(
    ticket: &PowTicket,
    replay_state: &StdMutex<TicketReplayState>,
    verify: impl FnOnce() -> Result<(), HandshakeError>,
) -> Result<(), HandshakeError> {
    let fingerprint = ticket.revocation_fingerprint();
    {
        let mut state = replay_state
            .lock()
            .map_err(|_| HandshakeError::ReplayStore("ticket replay lock poisoned".to_owned()))?;
        let now = SystemTime::now();
        let persisted_revoked = state
            .persisted
            .is_ticket_payload_revoked(ticket, now)
            .map_err(|error| HandshakeError::ReplayStore(error.to_string()))?;
        if persisted_revoked || state.pending.contains(&fingerprint) {
            return Err(HandshakeError::Pow(pow::Error::Replay));
        }
        let persisted_len = state
            .persisted
            .len(now)
            .map_err(|error| HandshakeError::ReplayStore(error.to_string()))?;
        if persisted_len.saturating_add(state.pending.len()) >= state.capacity {
            return Err(HandshakeError::ReplayStore(
                "ticket replay store at capacity".to_owned(),
            ));
        }
        state.pending.try_reserve(1).map_err(|_| {
            HandshakeError::ReplayStore("ticket replay pending-set allocation failed".to_owned())
        })?;
        state.pending.insert(fingerprint);
    }
    if let Err(error) = verify() {
        let mut state = replay_state
            .lock()
            .map_err(|_| HandshakeError::ReplayStore("ticket replay lock poisoned".to_owned()))?;
        state.pending.remove(&fingerprint);
        return Err(error);
    }
    let mut state = replay_state
        .lock()
        .map_err(|_| HandshakeError::ReplayStore("ticket replay lock poisoned".to_owned()))?;
    let consumed = pow::record_revocation(ticket, Some(&mut state.persisted), SystemTime::now())
        .map_err(|error| match error {
            pow::Error::RevocationStore(message) => HandshakeError::ReplayStore(message),
            other => HandshakeError::Pow(other),
        });
    state.pending.remove(&fingerprint);
    consumed
}
fn verify_puzzle_ticket_binding(
    ticket: &PowTicket,
    params: &puzzle::Parameters,
    descriptor_commit: &[u8],
    relay_id: &[u8],
    transcript_hash: &[u8; 32],
    replay_state: &StdMutex<TicketReplayState>,
) -> Result<(), HandshakeError> {
    let binding = PuzzleBinding::new(descriptor_commit, relay_id, transcript_hash);
    verify_and_consume_ticket(ticket, replay_state, || {
        puzzle::verify(ticket, &binding, params).map_err(HandshakeError::Puzzle)
    })
}
fn verify_pow_ticket_binding(
    ticket: &PowTicket,
    params: &PowParameters,
    descriptor_commit: &[u8],
    relay_id: &[u8],
    transcript_hash: &[u8; 32],
    replay_state: &StdMutex<TicketReplayState>,
) -> Result<(), HandshakeError> {
    let binding = pow::ChallengeBinding::new(descriptor_commit, relay_id, transcript_hash);
    verify_and_consume_ticket(ticket, replay_state, || {
        pow::verify(ticket, &binding, params).map_err(HandshakeError::Pow)
    })
}
fn continue_after_admission<T>(
    admission: Result<(), HandshakeError>,
    expensive_handshake: impl FnOnce() -> Result<T, HandshakeError>,
) -> Result<T, HandshakeError> {
    admission?;
    expensive_handshake()
}
fn load_vpn_helper_ticket_replay_ledger(
    config: &config::VpnConfig,
    relay_id: &RelayId,
    now_ms: u64,
) -> Result<PersistentReplayLedger, ConfigError> {
    let max_ttl_ms = u64::from(config.lease_secs)
        .checked_mul(1_000)
        .ok_or_else(|| ConfigError::Vpn("vpn.lease_secs overflows milliseconds".to_owned()))?;
    let limits = ReplayLedgerLimits::new(config.helper_ticket_replay_store_capacity, max_ttl_ms)
        .map_err(|error| {
            ConfigError::Vpn(format!(
                "invalid VPN helper-ticket replay ledger settings: {error}"
            ))
        })?;
    let mut namespace =
        Vec::with_capacity(VPN_HELPER_TICKET_REPLAY_NAMESPACE.len() + relay_id.len());
    namespace.extend_from_slice(VPN_HELPER_TICKET_REPLAY_NAMESPACE);
    namespace.extend_from_slice(relay_id);
    let replay_path = absolute_replay_state_path(&config.helper_ticket_replay_store_path)
        .map_err(|error| ConfigError::Vpn(format!("failed to resolve replay path: {error}")))?;
    PersistentReplayLedger::load(&replay_path, &namespace, limits, now_ms).map_err(|error| {
        ConfigError::Vpn(format!(
            "failed to load VPN helper-ticket replay ledger ({}): {error}",
            replay_path.display()
        ))
    })
}
fn vpn_helper_ticket_replay_id(ticket: &VpnHelperTicketV1) -> [u8; 32] {
    vpn_helper_ticket_replay_id_from_parts(&ticket.relay_id, &ticket.session_id)
}
fn vpn_helper_ticket_replay_id_from_parts(relay_id: &RelayId, session_id: &[u8; 16]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(VPN_HELPER_TICKET_REPLAY_ID_DOMAIN);
    hasher.update(relay_id);
    hasher.update(session_id);
    *hasher.finalize().as_bytes()
}
fn recover_vpn_helper_ticket_replay(
    replay_ledger: &StdMutex<PersistentReplayLedger>,
    replay_id: [u8; 32],
    expires_at_ms: u64,
    now_ms: u64,
) -> Result<(), String> {
    let mut replay_ledger = replay_ledger
        .lock()
        .map_err(|_| "VPN helper-ticket replay ledger lock poisoned during recovery".to_owned())?;
    let status = replay_ledger
        .insert(replay_id, expires_at_ms, now_ms)
        .map_err(|error| {
            format!("VPN helper-ticket replay recovery persistence failed: {error}")
        })?;
    match status {
        ReplayInsertStatus::Accepted
        | ReplayInsertStatus::Duplicate
        | ReplayInsertStatus::Expired => Ok(()),
        ReplayInsertStatus::TtlExceeded => {
            Err("recovered VPN helper ticket exceeds the configured replay lifetime".to_owned())
        }
        ReplayInsertStatus::Capacity => Err(
            "VPN helper-ticket replay ledger is at capacity during settlement recovery".to_owned(),
        ),
    }
}
fn consume_vpn_helper_ticket(
    replay_ledger: &StdMutex<PersistentReplayLedger>,
    replay_id: [u8; 32],
    expires_at_ms: u64,
    now_ms: u64,
) -> Result<(), HandshakeError> {
    let mut replay_ledger = replay_ledger.lock().map_err(|_| {
        HandshakeError::ReplayStore("VPN helper-ticket replay ledger lock poisoned".to_owned())
    })?;
    let status = replay_ledger
        .insert(replay_id, expires_at_ms, now_ms)
        .map_err(|error| {
            HandshakeError::ReplayStore(format!(
                "VPN helper-ticket replay ledger persistence failed: {error}"
            ))
        })?;
    match status {
        ReplayInsertStatus::Accepted => Ok(()),
        ReplayInsertStatus::Duplicate => {
            Err(HandshakeError::HelperTicket(VpnHelperTicketError::Replayed))
        }
        ReplayInsertStatus::Expired => Err(HandshakeError::HelperTicket(
            VpnHelperTicketError::Expired {
                expires_at_ms,
                now_ms,
            },
        )),
        ReplayInsertStatus::TtlExceeded => Err(HandshakeError::ReplayStore(
            "VPN helper ticket lifetime exceeds configured vpn.lease_secs".to_owned(),
        )),
        ReplayInsertStatus::Capacity => Err(HandshakeError::ReplayStore(
            "VPN helper-ticket replay ledger is at capacity".to_owned(),
        )),
    }
}
#[cfg(test)]
async fn redeem_vpn_helper_ticket(
    replay_ledger: Arc<StdMutex<PersistentReplayLedger>>,
    ticket: &VpnHelperTicketV1,
    now_ms: u64,
) -> Result<(), HandshakeError> {
    let replay_id = vpn_helper_ticket_replay_id(ticket);
    let expires_at_ms = ticket.expires_at_ms;
    tokio::task::spawn_blocking(move || {
        consume_vpn_helper_ticket(replay_ledger.as_ref(), replay_id, expires_at_ms, now_ms)
    })
    .await
    .map_err(|error| {
        HandshakeError::ReplayStore(format!(
            "VPN helper-ticket replay ledger worker failed: {error}"
        ))
    })?
}
async fn persist_initial_settlement_and_redeem_ticket(
    settlement_store: Arc<VpnSettlementStore>,
    replay_ledger: Arc<StdMutex<PersistentReplayLedger>>,
    session: VpnSessionHandle,
    ticket: VpnHelperTicketV1,
    artifact: VpnSettlementArtifact,
    now_ms: u64,
) -> Result<(), VpnBackendBridgeError> {
    let worker_store = Arc::clone(&settlement_store);
    tokio::task::spawn_blocking(move || {
        let wal_path = worker_store
            .write_initial_reservation(&session, &artifact)
            .map_err(|error| {
                VpnBackendBridgeError::UsageVoucher(format!(
                    "failed to durably reserve initial VPN settlement: {error}"
                ))
            })?;
        let replay_result = consume_vpn_helper_ticket(
            replay_ledger.as_ref(),
            vpn_helper_ticket_replay_id(&ticket),
            ticket.expires_at_ms,
            now_ms,
        );
        if let Err(replay_error) = replay_result {
            worker_store
                .discard_initial_reservation(&wal_path)
                .map_err(|persistence_error| {
                    VpnBackendBridgeError::UsageVoucher(format!(
                        "VPN helper-ticket redemption failed ({replay_error}) and its settlement WAL could not be discarded: {persistence_error}"
                    ))
                })?;
            return Err(VpnBackendBridgeError::UsageVoucher(format!(
                "VPN helper-ticket redemption failed: {replay_error}"
            )));
        }
        Ok(())
    })
    .await
    .map_err(|error| {
        settlement_store.poisoned.store(true, Ordering::Release);
        VpnBackendBridgeError::UsageVoucher(format!(
            "VPN settlement/replay persistence worker failed: {error}"
        ))
    })?
}
fn ensure_vpn_helper_ticket_within_trust(
    ticket: &VpnHelperTicketV1,
    trust: &RelayTransportTrust,
) -> Result<(), NoiseHandshakeError> {
    if ticket.expires_at_ms > trust.valid_until_ms {
        return Err(NoiseHandshakeError::Validation(
            "VPN helper ticket outlives authenticated relay trust".to_owned(),
        ));
    }
    Ok(())
}
fn vpn_helper_handshake_binding(
    helper_ticket: &[u8],
    relay_id: &RelayId,
    descriptor_commit: &[u8],
    trust: &RelayTransportTrust,
) -> [u8; 32] {
    fn update(hasher: &mut blake3::Hasher, value: &[u8]) {
        hasher.update(&(value.len() as u64).to_be_bytes());
        hasher.update(value);
    }
    let mut hasher = blake3::Hasher::new();
    for value in [
        b"iroha.soranet.vpn.helper-handshake-binding.v2".as_slice(),
        helper_ticket,
        trust.quic_multiaddr.as_bytes(),
        relay_id.as_slice(),
        descriptor_commit,
        trust.tls_spki_sha256.as_slice(),
        trust.relay_certificate_sha256.as_slice(),
        trust.directory_snapshot_digest.as_slice(),
        trust.tls_server_name.as_bytes(),
        SORANET_QUIC_ALPN,
    ] {
        update(&mut hasher, value);
    }
    *hasher.finalize().as_bytes()
}
impl RelayRuntime {
    /// Build a relay runtime from configuration, validating certificates,
    /// descriptor commits, guard snapshots, and identity keys along the way.
    pub fn new(mut config: RelayConfig) -> Result<Self, RelayError> {
        config.validate()?;
        let admin_authorization = config
            .admin_auth_token_path()
            .map(AdminAuthorization::load)
            .transpose()?
            .map(Arc::new);
        let validation_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                RelayError::Config(ConfigError::Handshake(
                    "system clock is before the Unix epoch".to_string(),
                ))
            })
            .and_then(|duration| {
                i64::try_from(duration.as_secs()).map_err(|_| {
                    RelayError::Config(ConfigError::Handshake(
                        "current Unix time exceeds i64::MAX".to_string(),
                    ))
                })
            })?;
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
        let certificate_bundle = policy.load_certificate_bundle_at(validation_time)?;
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
        let mut manifest_secrets = policy.manifest_secrets()?;
        let manifest_identity = manifest_secrets
            .as_ref()
            .and_then(|secrets| secrets.identity_private_key);
        let ml_kem_public = manifest_secrets
            .as_ref()
            .and_then(|secrets| secrets.ml_kem_public_key.clone());
        if let Some(secrets) = manifest_secrets.as_mut() {
            if let Some(identity) = secrets.identity_private_key.as_mut() {
                identity.fill(0);
            }
            if let Some(private) = secrets.ml_kem_private_key.as_mut() {
                private.fill(0);
            }
        }
        drop(manifest_secrets);
        if let (Some(bundle), Some(public)) = (certificate_bundle.as_ref(), ml_kem_public.as_ref())
            && public.as_slice() != bundle.certificate.pq_kem_public.as_slice()
        {
            return Err(RelayError::Config(ConfigError::Handshake(
                "ML-KEM public key from descriptor manifest does not match certificate pq_kem_public"
                    .to_string(),
            )));
        }
        let mut identity_seed = if let Some(seed) = manifest_identity {
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
            return Err(RelayError::Config(ConfigError::Handshake(
                "relay handshake identity key is required; configure a private handshake.descriptor_manifest_path"
                    .to_string(),
            )));
        };
        let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, &identity_seed);
        identity_seed.fill(0);
        let private_key = private_key.map_err(|err| {
            RelayError::Crypto(format!("failed to parse relay identity key: {err}"))
        })?;
        let identity_key = KeyPair::from_private_key(private_key).map_err(|err| {
            RelayError::Crypto(format!("failed to derive relay identity key pair: {err}"))
        })?;
        let relay_id = derive_relay_id(&identity_key)?;
        let identity_key = Arc::new(identity_key);
        if let Some(bundle) = certificate_bundle.as_ref() {
            let (algorithm, public_bytes) =
                identity_key.public_key().try_to_bytes().map_err(|err| {
                    RelayError::Crypto(format!("malformed relay identity public key: {err}"))
                })?;
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
        let mut authenticated_guard_entry = None;
        if let Some(guard_cfg) = config.guard_directory_config() {
            let commit_bytes = descriptor_commit_bytes.expect("checked above");
            match guard::load_guard_entry_at(guard_cfg, &relay_id, &commit_bytes, validation_time) {
                Ok(entry) => {
                    if let Some(configured_bundle) = certificate_bundle.as_ref()
                        && configured_bundle != &entry.bundle
                    {
                        return Err(RelayError::Config(ConfigError::GuardDirectory(
                            "configured relay certificate bundle does not exactly match the authenticated guard-directory entry"
                                .to_owned(),
                        )));
                    }
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
                    authenticated_guard_entry = Some(entry);
                }
                Err(GuardDirectoryError::RelayEntryMissing { .. })
                    if guard_cfg.allow_missing_entry() =>
                {
                    warn!(
                        "guard directory entry missing but allow_missing_entry=true; continuing without pinning proof"
                    );
                }
                Err(err) => return Err(RelayError::Config(err.into())),
            }
        }
        let transport_certificate_bundle = if config.vpn_config().is_some_and(|vpn| vpn.enabled) {
            authenticated_guard_entry
                .as_ref()
                .map(|entry| &entry.bundle)
        } else {
            certificate_bundle.as_ref()
        };
        let directory_valid_until_unix = authenticated_guard_entry
            .as_ref()
            .map(|entry| entry.snapshot_valid_until_unix);
        let (server_config, transport_trust) = Self::prepare_server_transport(
            &config,
            transport_certificate_bundle,
            directory_valid_until_unix,
        )?;
        let incentive_logger = config
            .incentive_log_config()
            .as_logger(&hex::encode(relay_id))
            .map_err(RelayError::from)?
            .map(Arc::new);
        let incentive_max_active_epochs = config.incentive_log_config().max_active_epochs;
        let incentive_max_measurements_per_epoch =
            config.incentive_log_config().max_measurements_per_epoch;
        let pow_config = config.pow_config().clone();
        let base_pow_params = pow_config.parameters()?;
        let puzzle_params = pow_config.puzzle_parameters(&base_pow_params)?;
        let token_policy = pow_config.token_policy().map_err(RelayError::Config)?;
        let ticket_replays = Arc::new(StdMutex::new(TicketReplayState::load(
            &pow_config,
            SystemTime::now(),
        )?));
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
            .map(VpnOverlay::try_from_config)
            .transpose()?
            .map(Arc::new);
        let vpn_helper_ticket_replays = vpn_overlay
            .as_ref()
            .map(|vpn| {
                load_vpn_helper_ticket_replay_ledger(
                    vpn.config(),
                    &relay_id,
                    unix_time_ms(SystemTime::now()),
                )
            })
            .transpose()?
            .map(|ledger| Arc::new(StdMutex::new(ledger)));
        let vpn_settlement_store = match (vpn_overlay.as_ref(), vpn_helper_ticket_replays.as_ref())
        {
            (Some(vpn), Some(replay_ledger)) => {
                let spool_dir = vpn.config().receipt_spool_dir.as_deref().ok_or_else(|| {
                    RelayError::Config(ConfigError::Vpn(
                        "vpn receipt spool directory is unavailable after validation".to_owned(),
                    ))
                })?;
                Some(Arc::new(
                    VpnSettlementStore::open(spool_dir, replay_ledger.as_ref()).map_err(
                        |error| {
                            RelayError::Io(io::Error::other(format!(
                                "failed to initialize VPN settlement persistence: {error}"
                            )))
                        },
                    )?,
                ))
            }
            (None, None) => None,
            _ => {
                return Err(RelayError::Io(io::Error::other(
                    "VPN settlement persistence and helper-ticket replay state must initialize together",
                )));
            }
        };
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
        )?);
        let privacy = Arc::new(PrivacyAggregator::new(config.privacy_config().into()));
        let event_capacity = config.privacy_config().event_buffer_capacity;
        let privacy_events = Arc::new(PrivacyEventBuffer::new(event_capacity));
        let proxy_policy_events = Arc::new(ProxyPolicyEventBuffer::new(event_capacity));
        let certificate_bundle_arc = certificate_bundle
            .as_ref()
            .map(|bundle| Arc::new(bundle.clone()));
        let handshake_suites = Arc::new(resolve_handshake_suites(certificate_bundle.as_ref())?);
        let registry = Arc::new(CircuitRegistry::with_max_entries(
            config.congestion_config().max_active_circuits,
        ));
        let lane_manager = Arc::new(ConstantRateLaneManager::new(
            profile_spec,
            Arc::clone(&registry),
        ));
        Ok(Self {
            config,
            server_config,
            transport_trust: transport_trust.map(Arc::new),
            admin_authorization,
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
            performance: Arc::new(Mutex::new(RelayPerformanceAccumulator::with_limits(
                relay_id,
                incentive_max_active_epochs,
                incentive_max_measurements_per_epoch,
            ))),
            epoch_window_secs: INCENTIVE_EPOCH_WINDOW_SECS,
            relay_id,
            exit_routing,
            incentives: incentive_logger,
            lane_manager,
            vpn: vpn_overlay,
            vpn_helper_ticket_replays,
            vpn_settlement_store,
            ticket_replays,
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
            transport_trust: self.transport_trust.as_ref().map(Arc::clone),
            exit_routing: Arc::clone(&self.exit_routing),
            incentives: self.incentives.clone(),
            lane_manager: Arc::clone(&self.lane_manager),
            vpn: self.vpn.clone(),
            vpn_helper_ticket_replays: self.vpn_helper_ticket_replays.clone(),
            vpn_settlement_store: self.vpn_settlement_store.clone(),
            ticket_replays: Arc::clone(&self.ticket_replays),
        }
    }
    /// Start the relay control/data planes until shutdown is requested.
    pub async fn run(self) -> Result<(), RelayError> {
        let listen_addr = self.config.listen_addr()?;
        let admin_addr = self.config.admin_addr()?;
        let mode = self.config.mode;
        let endpoint = Endpoint::server(self.server_config.clone(), listen_addr)
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
            let authorization = self
                .admin_authorization
                .clone()
                .expect("validated admin listener has authentication");
            let resources = AdminResources {
                metrics: Arc::clone(&self.metrics),
                privacy: Arc::clone(&self.privacy),
                privacy_events: Arc::clone(&self.privacy_events),
                proxy_policy_events: Arc::clone(&self.proxy_policy_events),
                performance: Arc::clone(&self.performance),
            };
            Some(tokio::spawn(async move {
                if let Err(error) =
                    RelayRuntime::serve_admin(resources, relay_id, admin_addr, mode, authorization)
                        .await
                {
                    warn!(%error, "admin server terminated");
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
        let handshake_permits = Arc::new(Semaphore::new(QUIC_MAX_PENDING_HANDSHAKES_V1));
        while let Some(incoming) = endpoint.accept().await {
            let Some(incoming) = Self::require_validated_quic_address(incoming) else {
                continue;
            };
            let Some(handshake_permit) = Self::try_quic_handshake_permit(&handshake_permits) else {
                self.metrics.record_capacity_reject();
                incoming.refuse();
                warn!(
                    limit = QUIC_MAX_PENDING_HANDSHAKES_V1,
                    "refusing QUIC connection: pending-handshake capacity reached"
                );
                continue;
            };
            let context = self.circuit_context();
            tokio::spawn(async move {
                RelayRuntime::handle_incoming(incoming, context, handshake_permit).await;
            });
        }
        Ok(())
    }
    fn require_validated_quic_address(incoming: Incoming) -> Option<Incoming> {
        if incoming.remote_address_validated() {
            return Some(incoming);
        }
        if let Err(error) = incoming.retry() {
            error.into_incoming().refuse();
            warn!("failed to issue mandatory QUIC address-validation retry");
        }
        None
    }
    fn try_quic_handshake_permit(permits: &Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
        Arc::clone(permits).try_acquire_owned().ok()
    }
    async fn handle_incoming(
        incoming: Incoming,
        context: CircuitContext,
        handshake_permit: OwnedSemaphorePermit,
    ) {
        let metrics = Arc::clone(&context.metrics);
        let privacy = Arc::clone(&context.privacy);
        let privacy_events = Arc::clone(&context.privacy_events);
        let mode = context.mode;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();
        match incoming.accept() {
            Ok(connecting) => match timeout(QUIC_TLS_HANDSHAKE_TIMEOUT, connecting).await {
                Ok(Ok(connection)) => {
                    let remote = connection.remote_address();
                    info!(mode = mode.as_label(), "accepted SoraNet connection");
                    Self::establish_circuit(connection, context, remote, handshake_permit).await;
                }
                Ok(Err(error)) => {
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
                Err(_) => {
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
                    warn!(
                        timeout_secs = QUIC_TLS_HANDSHAKE_TIMEOUT.as_secs(),
                        "QUIC TLS handshake timed out"
                    );
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
        handshake_permit: OwnedSemaphorePermit,
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
                Err(CongestionError::GlobalCircuitCapacity { limit }) => {
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
                        mode = mode.as_label(),
                        limit, "rejecting handshake: global active-circuit capacity reached"
                    );
                    let reason = format!("global circuit capacity reached (limit {limit})");
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
                        mode = mode.as_label(),
                        limit, "rejecting handshake: maximum circuits per client reached"
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
        let handshake_result = match timeout(
            QUIC_APPLICATION_HANDSHAKE_TIMEOUT,
            Self::perform_handshake(&connection, &context, remote),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(HandshakeError::Timeout("application handshake")),
        };
        // Established circuits are bounded separately. Release admission
        // capacity as soon as the authenticated application handshake ends.
        drop(handshake_permit);
        match handshake_result {
            Ok(HandshakeOutcome {
                negotiated,
                session,
                handshake_bytes,
                puzzle_verify_micros,
                vpn_session,
                vpn_helper_ticket,
            }) => {
                let record_layer =
                    match RecordLayer::new(&session.session_key, RecordEndpoint::Relay) {
                        Ok(layer) => Arc::new(layer),
                        Err(error) => {
                            metrics.record_failure();
                            warn!(
                                %error,
                                "rejecting handshake with unusable application record key"
                            );
                            connection.close(0u32.into(), b"invalid record key");
                            return;
                        }
                    };
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
                        mode = mode.as_label(),
                        warnings = ?warning_messages,
                        "SoraNet handshake reported capability warnings"
                    );
                }
                if let Some(payload) = session.telemetry_payload.as_ref() {
                    debug!(
                        target: SORANET_HANDSHAKE_LOG_TARGET,
                        mode = mode.as_label(),
                        payload_bytes = payload.len(),
                        "SoraNet handshake telemetry"
                    );
                }
                debug!(
                    target: SORANET_HANDSHAKE_LOG_TARGET,
                    mode = mode.as_label(),
                    key_len = session.session_key.payload().len(),
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
                            mode = mode.as_label(),
                            limit, "rejecting handshake: constant-rate neighbor cap reached"
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
                    Err(CircuitAdmissionError::CircuitCapacity { limit }) => {
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
                            mode = mode.as_label(),
                            limit, "rejecting handshake: circuit registry capacity reached"
                        );
                        connection.close(0u32.into(), b"capacity exceeded");
                        return;
                    }
                    Err(CircuitAdmissionError::MemoryCapacity) => {
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
                            mode = mode.as_label(),
                            "rejecting handshake: circuit registry memory unavailable"
                        );
                        connection.close(0u32.into(), b"capacity exceeded");
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
                    vpn_helper_ticket_replays: context.vpn_helper_ticket_replays.clone(),
                    vpn_settlement_store: context.vpn_settlement_store.clone(),
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
                    record_layer,
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
                let elapsed = attempt.elapsed();
                let millis = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
                let event_time = SystemTime::now();
                privacy.record_circuit_rejected(event_time, RejectReason::Downgrade, Some(millis));
                privacy_events.record_handshake_failure(
                    privacy_mode,
                    event_time,
                    SoranetPrivacyHandshakeFailureV1::Downgrade,
                    None,
                    Some(millis),
                );
                context
                    .proxy_policy_events
                    .record_downgrade(privacy_mode, event_time);
                if let Some(payload) = telemetry.as_ref() {
                    warn!(
                        target: SORANET_HANDSHAKE_LOG_TARGET,
                        mode = mode.as_label(),
                        payload_bytes = payload.len(),
                        "SoraNet handshake downgrade telemetry"
                    );
                }
                let warning_messages = warnings
                    .iter()
                    .map(|warning| warning.message.clone())
                    .collect::<Vec<_>>();
                warn!(
                    target: SORANET_HANDSHAKE_LOG_TARGET,
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
                    mode = mode.as_label(),
                    error = %error,
                    "handshake failed"
                );
                let millis = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
                let pow_detail = match &error {
                    HandshakeError::Pow(pow_error) => Some(pow_failure_reason(pow_error)),
                    HandshakeError::Puzzle(_) => Some(SoranetPowFailureReasonV1::InvalidSolution),
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
                    pow_detail,
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
        record_layer: Arc<RecordLayer>,
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
            Arc::clone(&record_layer),
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
                    Arc::clone(&record_layer),
                    resources.vpn_helper_ticket_replays.clone(),
                    resources.vpn_settlement_store.clone(),
                ))
            }
            _ => tokio::spawn(Self::serve_exit_streams(
                connection.clone(),
                exit_resources,
                remote,
                vpn_session.clone(),
                record_layer,
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
            debug!(%error, "measurement ingestion task join error");
        }
        if let Err(error) = exit_task.await {
            debug!(%error, "exit stream task join error");
        }
        if let Some(vpn_session) = vpn_session {
            let settlement_artifact = match vpn_session.settlement_artifact() {
                Ok(artifact) => artifact,
                Err(error) => {
                    warn!(%error, "failed to authenticate final VPN settlement receipt");
                    None
                }
            };
            let receipt = settlement_artifact
                .as_ref()
                .map(|artifact| artifact.receipt.receipt.clone())
                .unwrap_or_else(|| vpn_session.receipt());
            metrics.record_vpn_receipt(&receipt);
            if let (Some(store), Some(artifact)) = (
                resources.vpn_settlement_store.as_ref(),
                settlement_artifact.as_ref(),
            ) {
                let store = Arc::clone(store);
                let session = vpn_session.clone();
                let artifact = artifact.clone();
                let worker_store = Arc::clone(&store);
                match tokio::task::spawn_blocking(move || {
                    worker_store.finalize(&session, &artifact)
                })
                .await
                {
                    Ok(Ok(_)) => info!("vpn settlement artifact durably finalized"),
                    Ok(Err(error)) => warn!(
                        %error,
                        "failed to finalize vpn settlement artifact; recovery WAL retained"
                    ),
                    Err(error) => {
                        store.poisoned.store(true, Ordering::Release);
                        warn!(
                            %error,
                            "vpn settlement finalization worker failed; recovery WAL retained"
                        );
                    }
                }
            } else if settlement_artifact.is_none() {
                debug!("vpn settlement artifact absent because no client voucher was committed");
            }
            debug!(
                exit_class = receipt.exit_class.as_label(),
                ingress = receipt.ingress_bytes,
                egress = receipt.egress_bytes,
                cover = receipt.cover_bytes,
                uptime_secs = receipt.uptime_secs,
                "vpn session closed; receipt emitted"
            );
        }
        debug!(?reason, "SoraNet connection closed");
    }
    async fn ingest_measurement_streams(
        connection: Connection,
        resources: MonitorCircuitResources,
        remote: SocketAddr,
        record_layer: Arc<RecordLayer>,
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
                    let record_stream =
                        match record_layer.stream(record_stream_context(stream.id())) {
                            Ok(stream) => stream,
                            Err(error) => {
                                warn!(
                                    %error,
                                    "failed to derive measurement record keys"
                                );
                                break;
                            }
                        };
                    let stream = RecordReader::new(stream, record_stream.opener);
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
                        warn!(?error, "failed to ingest blinded bandwidth proof stream");
                    }
                }
                Err(quinn::ConnectionError::ApplicationClosed(_)) => break,
                Err(quinn::ConnectionError::LocallyClosed) => break,
                Err(error) => {
                    debug!(%error, "stopping measurement stream accept loop");
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
        record_layer: Arc<RecordLayer>,
    ) {
        loop {
            match connection.accept_bi().await {
                Ok((mut send, mut recv)) => {
                    let record_stream = match record_layer.stream(record_stream_context(send.id()))
                    {
                        Ok(stream) => stream,
                        Err(error) => {
                            warn!(
                                %error,
                                "failed to derive exit-stream record keys"
                            );
                            let _ = send.reset(VarInt::from_u32(0));
                            let _ = recv.stop(VarInt::from_u32(0));
                            continue;
                        }
                    };
                    let mut protected_send = RecordWriter::new(&mut send, record_stream.sealer);
                    let mut protected_recv = RecordReader::new(&mut recv, record_stream.opener);
                    if let Err(error) = Self::process_exit_stream(
                        &resources,
                        &mut protected_send,
                        &mut protected_recv,
                        remote,
                        vpn_session.clone(),
                    )
                    .await
                    {
                        let (stream_name, channel, reason) = error.compliance_context();
                        if let Some(logger) = resources.compliance.as_ref()
                            && let Err(err) = logger.log_exit_route_reject(
                                remote,
                                resources.mode,
                                stream_name,
                                channel,
                                reason,
                            )
                        {
                            warn!(%err, "failed to write compliance log entry");
                        }
                        warn!(
                            stream = stream_name.unwrap_or("unknown"),
                            reason, "failed to process exit stream"
                        );
                        drop(protected_send);
                        drop(protected_recv);
                        let _ = send.reset(VarInt::from_u32(0));
                        let _ = recv.stop(VarInt::from_u32(0));
                    }
                }
                Err(quinn::ConnectionError::ApplicationClosed(_))
                | Err(quinn::ConnectionError::LocallyClosed) => break,
                Err(error) => {
                    debug!(%error, "stopping exit stream accept loop");
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
        record_layer: Arc<RecordLayer>,
        helper_ticket_replays: Option<Arc<StdMutex<PersistentReplayLedger>>>,
        settlement_store: Option<Arc<VpnSettlementStore>>,
    ) {
        let Some(backend_endpoint) = overlay.config().backend_endpoint() else {
            warn!("vpn helper connection rejected: vpn.backend_endpoint is not configured");
            connection.close(0u32.into(), b"vpn backend unavailable");
            return;
        };
        let (mut send, mut recv) =
            match timeout(HANDSHAKE_STREAM_TIMEOUT, connection.accept_bi()).await {
                Ok(Ok(streams)) => streams,
                Ok(Err(error)) => {
                    warn!(%error, "failed to accept vpn helper tunnel stream");
                    connection.close(0u32.into(), b"vpn tunnel stream failed");
                    return;
                }
                Err(_) => {
                    warn!("timed out waiting for vpn helper tunnel stream");
                    connection.close(0u32.into(), b"vpn tunnel stream timeout");
                    return;
                }
            };
        let now_ms = unix_time_ms(SystemTime::now());
        if helper_ticket.expires_at_ms <= now_ms {
            connection.close(0u32.into(), b"vpn helper ticket expired");
            return;
        }
        let flow_label = vpn_flow_label_from_session_id(helper_ticket.session_id);
        let adapter = VpnAdapter::new(vpn_session.session().clone(), Arc::clone(&overlay));
        let bridge = match VpnBridge::new(adapter.clone(), helper_ticket.session_id, flow_label) {
            Ok(bridge) => bridge,
            Err(error) => {
                warn!(%error, "failed to initialize VPN cover scheduler");
                connection.close(0u32.into(), b"vpn cover scheduler unavailable");
                return;
            }
        };
        let mtu = bridge.max_payload_len();
        let record_stream = match record_layer.stream(record_stream_context(send.id())) {
            Ok(stream) => stream,
            Err(error) => {
                warn!(%error, "failed to derive vpn tunnel record keys");
                connection.close(0u32.into(), b"vpn record key failure");
                return;
            }
        };
        let mut protected_send = RecordWriter::new(&mut send, record_stream.sealer);
        let mut protected_recv = RecordReader::new(&mut recv, record_stream.opener);
        let Some(settlement_store) = settlement_store else {
            warn!("vpn settlement persistence is unavailable before prepaid admission");
            connection.close(0u32.into(), b"vpn settlement persistence unavailable");
            return;
        };
        if let Err(error) = settlement_store.ensure_healthy() {
            warn!(%error, "vpn settlement persistence is poisoned before prepaid admission");
            connection.close(0u32.into(), b"vpn settlement persistence poisoned");
            return;
        }
        let voucher_authorization = Arc::new(Mutex::new(VpnVoucherAuthorization::new(
            &helper_ticket,
            overlay.config().usage_voucher_credit_window_bytes,
            overlay.config().usage_voucher_max_age_ms,
        )));
        let setup_timeout = Duration::from_millis(overlay.config().usage_voucher_setup_timeout_ms);
        let initial_envelope = match timeout(
            setup_timeout,
            accept_initial_usage_voucher(&adapter, &mut protected_recv),
        )
        .await
        {
            Ok(Ok(envelope)) => envelope,
            Ok(Err(error)) => {
                warn!(%error, "vpn helper failed prepaid voucher admission");
                connection.close(0u32.into(), b"vpn prepaid voucher rejected");
                return;
            }
            Err(_) => {
                warn!("timed out waiting for initial prepaid vpn voucher");
                connection.close(0u32.into(), b"vpn prepaid voucher timeout");
                return;
            }
        };
        let Some(helper_ticket_replays) = helper_ticket_replays else {
            warn!("vpn helper-ticket replay ledger is unavailable after prepaid admission");
            connection.close(0u32.into(), b"vpn replay protection unavailable");
            return;
        };
        let (initial_authorization, initial_envelope) = {
            let authorization = voucher_authorization.lock().await;
            let mut candidate = authorization.clone();
            let envelope = match candidate.accept_envelope(&initial_envelope) {
                Ok(envelope) => envelope,
                Err(error) => {
                    warn!(%error, "vpn helper failed initial prepaid voucher validation");
                    connection.close(0u32.into(), b"vpn prepaid voucher rejected");
                    return;
                }
            };
            (candidate, envelope)
        };
        let initial_reservation =
            match vpn_session.settlement_reservation_artifact(&initial_envelope, 0, false) {
                Ok(artifact) => artifact,
                Err(error) => {
                    warn!(%error, "failed to construct initial vpn settlement reservation");
                    connection.close(0u32.into(), b"vpn settlement reservation invalid");
                    return;
                }
            };
        let path = backend_endpoint.path();
        let Some((backend_uid, backend_gid)) = overlay.config().backend_expected_peer_ids() else {
            warn!("vpn helper connection rejected: backend peer identity is not pinned");
            connection.close(0u32.into(), b"vpn backend identity unavailable");
            return;
        };
        let backend_label = format!("unix:{}", path.display());
        // Authenticate and reserve a backend connection before irreversibly
        // redeeming the one-use helper ticket. No backend protocol byte is sent
        // until the WAL and replay reservation below both commit.
        let backend = match timeout(
            HANDSHAKE_STREAM_TIMEOUT,
            connect_authenticated_vpn_backend(path, backend_uid, backend_gid),
        )
        .await
        {
            Ok(Ok(backend)) => backend,
            Ok(Err(error)) => {
                warn!(%error, "failed to authenticate local VPN backend");
                connection.close(0u32.into(), b"vpn backend authentication failed");
                return;
            }
            Err(_) => {
                warn!("timed out authenticating local VPN backend");
                connection.close(0u32.into(), b"vpn backend authentication timeout");
                return;
            }
        };
        if let Err(error) = persist_initial_settlement_and_redeem_ticket(
            Arc::clone(&settlement_store),
            helper_ticket_replays,
            vpn_session.clone(),
            helper_ticket.clone(),
            initial_reservation,
            unix_time_ms(SystemTime::now()),
        )
        .await
        {
            warn!(%error, "vpn settlement reservation or helper-ticket redemption failed");
            connection.close(0u32.into(), b"vpn durable admission failed");
            return;
        }
        *voucher_authorization.lock().await = initial_authorization;
        vpn_session.record_usage_voucher(initial_envelope.clone());
        let backend_result = Self::serve_vpn_backend_tunnel_stream(
            &mut protected_send,
            &mut protected_recv,
            &overlay,
            &vpn_session,
            &helper_ticket,
            backend,
            &backend_label,
            bridge,
            &adapter,
            voucher_authorization,
            settlement_store,
            initial_envelope,
            mtu,
        )
        .await;
        let close_reason = match backend_result {
            Ok(()) => b"vpn tunnel closed".as_slice(),
            Err(error) => {
                warn!(%error, %remote, "vpn helper bridge stopped");
                b"vpn bridge policy failure".as_slice()
            }
        };
        connection.close(0u32.into(), close_reason);
        match timeout(Duration::from_secs(1), protected_send.shutdown()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => debug!(%error, "failed to finish protected vpn tunnel stream"),
            Err(error) => debug!(%error, "timed out finishing protected vpn tunnel stream"),
        }
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "the tunnel handoff binds one authenticated prepaid session and backend"
    )]
    async fn serve_vpn_backend_tunnel_stream<VW, VR, S>(
        vpn_writer: &mut VW,
        vpn_reader: &mut VR,
        overlay: &VpnOverlay,
        vpn_session: &VpnSessionHandle,
        helper_ticket: &VpnHelperTicketV1,
        backend: S,
        _backend_addr: &str,
        bridge: VpnBridge,
        adapter: &VpnAdapter,
        voucher_authorization: Arc<Mutex<VpnVoucherAuthorization>>,
        settlement_store: Arc<VpnSettlementStore>,
        initial_envelope: VpnUsageVoucherEnvelopeV1,
        mtu: usize,
    ) -> Result<(), VpnBackendBridgeError>
    where
        VW: AsyncWrite + Unpin,
        VR: AsyncRead + Unpin,
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let now_ms = unix_time_ms(SystemTime::now());
        if helper_ticket.expires_at_ms <= now_ms {
            return Err(VpnBackendBridgeError::BackendControl(
                "vpn helper ticket expired before backend service started".to_owned(),
            ));
        }
        let (mut backend_read, mut backend_write) = tokio::io::split(backend);
        let bootstrap = build_vpn_backend_bootstrap(helper_ticket);
        info!(
            expires_at_ms = helper_ticket.expires_at_ms,
            "bridging helper-authenticated vpn tunnel to relay backend"
        );
        let bootstrap_secret = overlay.backend_bootstrap_secret().ok_or_else(|| {
            VpnBackendBridgeError::BackendControl(
                "vpn backend bootstrap secret is unavailable".to_owned(),
            )
        })?;
        write_vpn_backend_bootstrap(&mut backend_write, &bootstrap, bootstrap_secret).await?;
        read_vpn_backend_status(&mut backend_read).await?;
        let now_ms = unix_time_ms(SystemTime::now());
        if helper_ticket.expires_at_ms <= now_ms {
            return Err(VpnBackendBridgeError::BackendControl(
                "vpn helper ticket expired before backend service started".to_owned(),
            ));
        }
        let remaining = Duration::from_millis(helper_ticket.expires_at_ms.saturating_sub(now_ms));
        let authorized_active_ms = {
            let mut authorization = voucher_authorization.lock().await;
            authorization.begin_service();
            authorization.authorized_active_ms
        };
        vpn_session.begin_metered_service(now_ms);
        let service_reservation = vpn_session
            .settlement_reservation_artifact(&initial_envelope, authorized_active_ms, true)
            .map_err(VpnBackendBridgeError::UsageVoucher)?;
        let worker_store = Arc::clone(&settlement_store);
        let worker_session = vpn_session.clone();
        let persistence_result = match tokio::task::spawn_blocking(move || {
            worker_store.replace_reservation(
                &worker_session,
                &service_reservation,
                VpnSettlementWalPhase::ServiceReserved,
            )
        })
        .await
        {
            Ok(result) => result.map_err(|error| {
                VpnBackendBridgeError::UsageVoucher(format!(
                    "failed to durably reserve prepaid VPN service before forwarding: {error}"
                ))
            }),
            Err(error) => {
                settlement_store.poisoned.store(true, Ordering::Release);
                Err(VpnBackendBridgeError::UsageVoucher(format!(
                    "vpn service reservation persistence worker failed: {error}"
                )))
            }
        };
        if let Err(error) = persistence_result {
            vpn_session.cancel_metered_service_before_forwarding();
            return Err(error);
        }
        let result = timeout(
            remaining,
            Self::bridge_vpn_backend_streams(
                vpn_writer,
                vpn_reader,
                VpnBackendBridgeContext {
                    bridge,
                    adapter,
                    vpn_session,
                    voucher_authorization,
                    settlement_store,
                    mtu,
                },
                &mut backend_read,
                &mut backend_write,
            ),
        )
        .await;
        vpn_session.end_metered_service(unix_time_ms(SystemTime::now()));
        match result {
            Ok(result) => result,
            Err(_) => Err(VpnBackendBridgeError::UsageVoucher(
                "vpn helper ticket expired".to_owned(),
            )),
        }
    }
    async fn bridge_vpn_backend_streams<VW, VR, BR, BW>(
        vpn_writer: &mut VW,
        vpn_reader: &mut VR,
        context: VpnBackendBridgeContext<'_>,
        backend_reader: &mut BR,
        backend_writer: &mut BW,
    ) -> Result<(), VpnBackendBridgeError>
    where
        VW: AsyncWrite + Unpin,
        VR: AsyncRead + Unpin,
        BR: AsyncRead + Unpin,
        BW: AsyncWrite + Unpin,
    {
        let VpnBackendBridgeContext {
            mut bridge,
            adapter,
            vpn_session,
            voucher_authorization,
            settlement_store,
            mtu,
        } = context;
        let upstream_authorization = Arc::clone(&voucher_authorization);
        let upstream_settlement_store = Arc::clone(&settlement_store);
        let upstream = async {
            let max_payload = mtu.min(bridge.max_payload_len()).max(1);
            let mut buffer = vec![0u8; max_payload];
            let mut packet_decoder = VpnPacketStreamDecoder::default();
            loop {
                let read = backend_reader.read(&mut buffer).await?;
                if read == 0 {
                    break Ok(());
                }
                for packet in packet_decoder.ingest(&buffer[..read], mtu)? {
                    upstream_settlement_store
                        .ensure_healthy()
                        .map_err(VpnBackendBridgeError::UsageVoucher)?;
                    let packet_len = u64::try_from(packet.len())
                        .expect("first-release VPN packets are bounded by u16");
                    let active_deadline = upstream_authorization
                        .lock()
                        .await
                        .authorize_egress_packet(packet_len)?;
                    let framed = encode_vpn_packet_stream_frame(&packet)?;
                    tokio::time::timeout_at(
                        active_deadline,
                        bridge.send_buffer(vpn_writer, &framed),
                    )
                    .await
                    .map_err(|_| {
                        VpnBackendBridgeError::UsageVoucher(
                            "vpn active-time credit expired while forwarding an egress packet"
                                .to_owned(),
                        )
                    })??;
                    vpn_session.record_metered_egress(packet_len);
                }
            }
        };
        let downstream_authorization = Arc::clone(&voucher_authorization);
        let downstream_settlement_store = Arc::clone(&settlement_store);
        let voucher_max_age =
            Duration::from_millis(adapter.overlay().config().usage_voucher_max_age_ms);
        let downstream = async {
            let mut voucher_deadline = TokioInstant::now() + voucher_max_age;
            let mut packet_decoder = VpnPacketStreamDecoder::default();
            loop {
                // Keep the absolute voucher deadline around the complete
                // receive-and-forward step. A blocked local backend must not
                // suspend the payment-liveness watchdog.
                let active_deadline = downstream_authorization.lock().await.active_deadline()?;
                let deadline = voucher_deadline.min(active_deadline);
                let progress = tokio::time::timeout_at(deadline, async {
                    match adapter.read_ingress_frame(vpn_reader).await {
                        Ok(cell) => match cell.header.class {
                            VpnCellClassV1::Data => {
                                for packet in packet_decoder.ingest(&cell.payload, mtu)? {
                                    downstream_settlement_store
                                        .ensure_healthy()
                                        .map_err(VpnBackendBridgeError::UsageVoucher)?;
                                    let packet_len = u64::try_from(packet.len())
                                        .expect("first-release VPN packets are bounded by u16");
                                    let active_deadline = downstream_authorization
                                        .lock()
                                        .await
                                        .authorize_ingress_packet(packet_len)?;
                                    let framed = encode_vpn_packet_stream_frame(&packet)?;
                                    tokio::time::timeout_at(
                                        active_deadline,
                                        backend_writer.write_all(&framed),
                                    )
                                    .await
                                    .map_err(|_| {
                                        VpnBackendBridgeError::UsageVoucher(
                                            "vpn active-time credit expired while forwarding an ingress packet"
                                                .to_owned(),
                                        )
                                    })??;
                                    vpn_session.record_metered_ingress(packet_len);
                                }
                                Ok(Some(false))
                            }
                            VpnCellClassV1::Control => {
                                if let Some(envelope) = decode_usage_voucher_control(&cell.payload)?
                                {
                                    let mut authorization = downstream_authorization.lock().await;
                                    let mut candidate = authorization.clone();
                                    let envelope = candidate.accept_envelope(&envelope)?;
                                    let reservation = vpn_session
                                        .settlement_reservation_artifact(
                                            &envelope,
                                            candidate.authorized_active_ms,
                                            true,
                                        )
                                        .map_err(VpnBackendBridgeError::UsageVoucher)?;
                                    downstream_settlement_store
                                        .replace_reservation(
                                            vpn_session,
                                            &reservation,
                                            VpnSettlementWalPhase::ServiceReserved,
                                        )
                                        .map_err(|error| {
                                            VpnBackendBridgeError::UsageVoucher(format!(
                                                "failed to durably advance prepaid VPN settlement before accepting voucher: {error}"
                                            ))
                                        })?;
                                    *authorization = candidate;
                                    drop(authorization);
                                    vpn_session.record_usage_voucher(envelope);
                                    Ok(Some(true))
                                } else {
                                    Ok(Some(false))
                                }
                            }
                            VpnCellClassV1::Cover | VpnCellClassV1::KeepAlive => Ok(Some(false)),
                        },
                        Err(VpnFrameIoError::FrameLength { actual: 0, .. }) => Ok(None),
                        Err(error) => Err(VpnBackendBridgeError::from(error)),
                    }
                })
                .await
                .map_err(|_| {
                    if TokioInstant::now() >= active_deadline {
                        VpnBackendBridgeError::UsageVoucher(
                            "vpn service reached its signed prepaid active-time ceiling".to_owned(),
                        )
                    } else {
                        VpnBackendBridgeError::UsageVoucher(format!(
                            "no fresh vpn usage voucher arrived within {}ms",
                            voucher_max_age.as_millis()
                        ))
                    }
                })??;
                match progress {
                    Some(true) => {
                        voucher_deadline = TokioInstant::now() + voucher_max_age;
                    }
                    Some(false) => {}
                    None => break Ok(()),
                }
            }
        };
        tokio::select! {
            result = upstream => result,
            result = downstream => result,
        }
    }
    async fn process_exit_stream<W, R>(
        resources: &ExitStreamResources,
        send: &mut W,
        recv: &mut R,
        remote: SocketAddr,
        vpn_session: Option<VpnSessionHandle>,
    ) -> Result<(), ExitStreamError>
    where
        W: AsyncWrite + Unpin,
        R: AsyncRead + Unpin,
    {
        let mut header = [0u8; RouteOpenFrame::length()];
        recv.read_exact(&mut header)
            .await
            .map_err(ExitStreamError::Read)?;
        let frame = RouteOpenFrame::decode(&header)?;
        let vpn_adapter = vpn_session.as_ref().and_then(|session| {
            resources
                .vpn
                .as_ref()
                .map(|overlay| VpnAdapter::new(session.session().clone(), Arc::clone(overlay)))
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
    async fn handle_norito_stream<W, R>(
        resources: &ExitStreamResources,
        send: &mut W,
        recv: &mut R,
        remote: SocketAddr,
        frame: RouteOpenFrame,
        vpn_adapter: Option<VpnAdapter>,
    ) -> Result<(), ExitStreamError>
    where
        W: AsyncWrite + Unpin,
        R: AsyncRead + Unpin,
    {
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
            warn!(authenticated, "no norito-stream exit route configured");
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
            warn!("norito-stream route requires authenticated viewers");
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
            authenticated,
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
    async fn handle_kaigi_stream<W, R>(
        resources: &ExitStreamResources,
        send: &mut W,
        recv: &mut R,
        remote: SocketAddr,
        frame: RouteOpenFrame,
        vpn_adapter: Option<VpnAdapter>,
    ) -> Result<(), ExitStreamError>
    where
        W: AsyncWrite + Unpin,
        R: AsyncRead + Unpin,
    {
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
            warn!("kaigi-stream route requires authenticated viewers");
            return Err(ExitStreamError::RouteRequiresAuthentication {
                stream: "kaigi-stream",
                channel: channel_hex,
            });
        }
        let room_id = derive_kaigi_room_id(&channel_id, &record.route_id, &record.stream_id);
        let target_url = match Self::kaigi_multiaddr_to_websocket(&record.exit_multiaddr) {
            Some(url) => url,
            None => {
                warn!("kaigi-stream exit multiaddr unsupported; falling back to configured hub");
                state.hub_ws_url().to_owned()
            }
        };
        info!(authenticated, "kaigi-stream exit route resolved from spool");
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
            status = %response.status(),
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
    async fn bridge_websocket_stream<W, R>(
        send: &mut W,
        recv: &mut R,
        ws_stream: ToriiWebSocket,
        handshake_bytes: Bytes,
        label: &'static str,
        _remote: SocketAddr,
        padding: Option<PaddingSchedule>,
        vpn_adapter: Option<VpnAdapter>,
    ) -> Result<(), ExitStreamError>
    where
        W: AsyncWrite + Unpin,
        R: AsyncRead + Unpin,
    {
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
                                bytes if bytes > 0 => {
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
                                debug!(stream = label, code, "exit adapter closed");
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
                        Message::Text(_) => {
                            warn!(
                                stream = label,
                                "ignoring unexpected text frame from exit adapter"
                            );
                        }
                        Message::Frame(_) => {}
                    }
                }
                send_stream
                    .shutdown()
                    .await
                    .map_err(ExitStreamError::SendFinish)?;
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
    async fn process_measurement_stream<R>(
        mut stream: R,
        performance: Arc<Mutex<RelayPerformanceAccumulator>>,
        relay_id: RelayId,
        incentives: Option<Arc<IncentiveLogger>>,
        privacy: Arc<PrivacyAggregator>,
        privacy_events: Arc<PrivacyEventBuffer>,
        mode: RelayMode,
        compliance: Option<Arc<ComplianceLogger>>,
        remote: SocketAddr,
    ) -> Result<(), IncentiveStreamError>
    where
        R: AsyncRead + Unpin,
    {
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
    async fn read_measurement_frame<R>(
        stream: &mut R,
        len_buf: &mut [u8; 4],
    ) -> Result<Option<Vec<u8>>, IncentiveStreamError>
    where
        R: AsyncRead + Unpin,
    {
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
        let mut frame = Vec::new();
        frame
            .try_reserve_exact(frame_len)
            .map_err(|_| IncentiveStreamError::Allocation)?;
        frame.resize(frame_len, 0);
        if !Self::read_exact_or_eof(stream, &mut frame).await? {
            return Err(IncentiveStreamError::UnexpectedEof {
                expected: frame_len,
                received: 0,
            });
        }
        Ok(Some(frame))
    }
    async fn read_exact_or_eof<R>(
        stream: &mut R,
        buf: &mut [u8],
    ) -> Result<bool, IncentiveStreamError>
    where
        R: AsyncRead + Unpin,
    {
        if buf.is_empty() {
            return Ok(true);
        }
        let mut received = 0;
        while received < buf.len() {
            let read = stream
                .read(&mut buf[received..])
                .await
                .map_err(IncentiveStreamError::Read)?;
            if read == 0 {
                return if received == 0 {
                    Ok(false)
                } else {
                    Err(IncentiveStreamError::UnexpectedEof {
                        expected: buf.len(),
                        received,
                    })
                };
            }
            received += read;
        }
        Ok(true)
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
        let proof = norito::with_decode_limits_scope(BANDWIDTH_PROOF_DECODE_LIMITS_V1, || {
            RelayBandwidthProofV1::decode(&mut cursor)
        })?;
        if !cursor.is_empty() {
            return Err(IncentiveStreamError::TrailingBytes(cursor.len()));
        }
        let verifier_label = proof.verifier_id.to_string();
        enum ProofOutcome {
            Accepted { summary: Option<EpochSummary> },
            Duplicate,
            ForeignRelay,
            Capacity(IncentiveCapacityError),
        }
        let outcome = {
            let mut guard = performance.lock().await;
            match guard.try_ingest_bandwidth_proof(&proof) {
                Ok(BandwidthProofIngest::Accepted) => {
                    let summary = match guard.summary(proof.epoch) {
                        Ok(summary) => summary,
                        Err(error) => {
                            warn!(epoch = proof.epoch, %error, "failed to snapshot bounded incentive epoch");
                            None
                        }
                    };
                    ProofOutcome::Accepted { summary }
                }
                Ok(BandwidthProofIngest::Duplicate) => ProofOutcome::Duplicate,
                Ok(BandwidthProofIngest::ForeignRelay) => ProofOutcome::ForeignRelay,
                Err(error) => ProofOutcome::Capacity(error),
            }
        };
        if let Some(logger) = compliance.as_ref() {
            let reason = match &outcome {
                ProofOutcome::Accepted { .. } => None,
                ProofOutcome::Duplicate => Some("duplicate_measurement"),
                ProofOutcome::ForeignRelay => Some("foreign_relay"),
                ProofOutcome::Capacity(_) => Some("incentive_capacity"),
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
                matches!(&outcome, ProofOutcome::Accepted { .. }),
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
                    "accepted bandwidth proof"
                );
                if let (Some(summary), Some(logger)) = (summary, incentives) {
                    let metrics =
                        snapshot_from_summary(relay_id, summary, SnapshotKind::Measurement);
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
                    "ignored bandwidth proof (duplicate measurement_id)"
                );
            }
            ProofOutcome::ForeignRelay => {
                debug!(
                    epoch = proof.epoch,
                    "ignored bandwidth proof (foreign relay)"
                );
            }
            ProofOutcome::Capacity(error) => {
                warn!(
                    epoch = proof.epoch,
                    %error,
                    "ignored bandwidth proof at incentive memory capacity"
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
            if let Err(error) = guard.try_record_uptime(epoch, uptime_secs, epoch_window_secs) {
                warn!(epoch, %error, "unable to retain incentive uptime sample");
                continue;
            }
            if let Some(logger) = incentives.as_ref() {
                let summary = match guard.summary(epoch) {
                    Ok(summary) => summary,
                    Err(error) => {
                        warn!(epoch, %error, "failed to snapshot bounded incentive uptime epoch");
                        None
                    }
                };
                drop(guard);
                if let Some(summary) = summary {
                    let metrics = snapshot_from_summary(relay_id, summary, SnapshotKind::Uptime);
                    if let Err(error) = logger.write_snapshot(&metrics) {
                        warn!(epoch, ?error, "failed to persist uptime snapshot");
                    }
                }
            }
        }
    }
    async fn perform_handshake(
        connection: &Connection,
        context: &CircuitContext,
        _remote: SocketAddr,
    ) -> Result<HandshakeOutcome, HandshakeError> {
        let (mut send, mut recv) =
            match timeout(HANDSHAKE_STREAM_TIMEOUT, connection.accept_bi()).await {
                Ok(Ok(streams)) => streams,
                Ok(Err(error)) => return Err(HandshakeError::Connection(error)),
                Err(_) => return Err(HandshakeError::Timeout("handshake stream")),
            };
        let helper_ticket_issuer_public_key = context
            .vpn
            .as_ref()
            .and_then(|overlay| overlay.helper_ticket_issuer_public_key());
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
        let mut vpn_helper_ticket_frame: Option<Vec<u8>> = None;
        if helper_ticket_issuer_public_key.is_some()
            || has_token_policy
            || (pow_required && puzzle_enforced)
        {
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
            if let Some(issuer_public_key) = helper_ticket_issuer_public_key {
                if VpnHelperTicketV1::looks_like(&first_frame) {
                    let now_ms = unix_time_ms(SystemTime::now());
                    let helper_ticket =
                        VpnHelperTicketV1::parse(&first_frame, issuer_public_key, now_ms)?;
                    if helper_ticket.relay_id != context.relay_id {
                        return Err(HandshakeError::HelperTicket(
                            VpnHelperTicketError::InvalidRelay,
                        ));
                    }
                    let trust = context.transport_trust.as_deref().ok_or_else(|| {
                        HandshakeError::Noise(NoiseHandshakeError::Validation(
                            "VPN helper ticket received without immutable relay transport trust"
                                .to_owned(),
                        ))
                    })?;
                    ensure_vpn_helper_ticket_within_trust(&helper_ticket, trust)
                        .map_err(HandshakeError::Noise)?;
                    vpn_helper_ticket = Some(helper_ticket);
                    vpn_helper_ticket_frame = Some(first_frame);
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
        let RelayClientHelloPreflight {
            metadata: client_hello,
            mut negotiated,
        } = preflight_client_hello(&client_frame, context.server_caps.as_ref())?;
        let transcript_binding = pow::derive_admission_transcript(&client_frame);
        let mut response_caps = negotiated.clone();
        response_caps.grease.extend(context.grease.iter().cloned());
        let mut grease_entries = std::mem::take(&mut response_caps.grease);
        grease_entries.sort_by_key(|entry| entry.ty);
        let relay_caps_bytes =
            encode_relay_advertisement(&response_caps, context.server_caps.role_bits)?;
        let relay_caps_bytes =
            update_suite_list(&relay_caps_bytes, context.handshake_suites.as_slice(), true)
                .map_err(HandshakeError::Noise)?;
        let relay_caps_bytes = append_grease_tlvs(relay_caps_bytes, &grease_entries)?;
        let admission = if let (Some(params), Some(ticket)) =
            (puzzle_params.as_ref(), pending_puzzle_ticket.as_ref())
        {
            let verify_start = Instant::now();
            let result = verify_puzzle_ticket_binding(
                ticket,
                params,
                context.descriptor_commit.as_slice(),
                context.relay_id.as_slice(),
                &transcript_binding,
                context.ticket_replays.as_ref(),
            );
            let elapsed = verify_start.elapsed();
            context.metrics.record_puzzle_verify(elapsed);
            let micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
            puzzle_verify_micros = Some(micros);
            result
        } else if let Some(ticket) = pending_pow_ticket.as_ref() {
            verify_pow_ticket_binding(
                ticket,
                &pow_params,
                context.descriptor_commit.as_slice(),
                context.relay_id.as_slice(),
                &transcript_binding,
                context.ticket_replays.as_ref(),
            )
        } else if let Some(token) = admission_token.as_ref() {
            context
                .dos
                .verify_token(
                    token,
                    &context.relay_id,
                    &transcript_binding,
                    SystemTime::now(),
                )
                .map_err(HandshakeError::Token)
        } else {
            Ok(())
        };
        // Admission must complete before the handshake engine validates or
        // encapsulates any client ML-KEM material.
        let helper_resume_binding = match vpn_helper_ticket_frame.as_deref() {
            Some(frame) => {
                let trust = context.transport_trust.as_deref().ok_or_else(|| {
                    HandshakeError::Noise(NoiseHandshakeError::Validation(
                        "VPN helper ticket received without immutable relay transport trust"
                            .to_owned(),
                    ))
                })?;
                Some(vpn_helper_handshake_binding(
                    frame,
                    &context.relay_id,
                    context.descriptor_commit.as_slice(),
                    trust,
                ))
            }
            None => None,
        };
        let transport_tls_server_name = context
            .transport_trust
            .as_deref()
            .map_or(DEFAULT_TLS_SERVER_NAME, |trust| {
                trust.tls_server_name.as_str()
            });
        let (relay_hello, session) = continue_after_admission(admission, || {
            let mut rng = StdRng::from_os_rng();
            let runtime_params = NoiseRuntimeParams {
                descriptor_commit: context.descriptor_commit.as_slice(),
                client_capabilities: client_hello.client_capabilities(),
                relay_capabilities: &relay_caps_bytes,
                kem_id: negotiated.kem.id.code(),
                sig_id: client_hello.sig_id(),
                transport_alpn: SORANET_QUIC_ALPN,
                tls_server_name: transport_tls_server_name,
                resume_hash: helper_resume_binding.as_ref().map_or_else(
                    || client_hello.resume_hash(),
                    |binding| Some(binding.as_slice()),
                ),
            };
            match process_client_hello(
                &client_frame,
                &runtime_params,
                context.identity_key.as_ref(),
                &mut rng,
            ) {
                Ok(result) => Ok(result),
                Err(NoiseHandshakeError::Downgrade {
                    warnings,
                    telemetry,
                }) => Err(HandshakeError::Downgrade {
                    warnings,
                    telemetry,
                }),
                Err(error) => Err(HandshakeError::Noise(error)),
            }
        })?;
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
        negotiated.grease.extend(context.grease.iter().cloned());
        let handshake_bytes = byte_guard.finish();
        let vpn_session = match (vpn_helper_ticket.clone(), context.vpn.as_ref()) {
            (Some(helper_ticket), Some(overlay)) => Some(
                overlay
                    .bind_helper_session(
                        overlay.start_session(Arc::clone(&context.metrics)),
                        helper_ticket,
                        Arc::clone(&context.identity_key),
                    )
                    .map_err(|error| {
                        HandshakeError::Noise(NoiseHandshakeError::Validation(error))
                    })?,
            ),
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
        let len = handshake_frame_len_prefix(payload.len())?;
        send.write_all(&len).await.map_err(HandshakeError::Write)?;
        send.write_all(payload)
            .await
            .map_err(HandshakeError::Write)?;
        Ok(())
    }
    async fn read_admin_request<R>(reader: &mut R, deadline: Duration) -> io::Result<String>
    where
        R: AsyncRead + Unpin,
    {
        let bytes = timeout(deadline, async {
            let mut request = Vec::with_capacity(1024);
            let mut buffer = [0_u8; 1024];
            loop {
                let remaining = ADMIN_MAX_HEADER_BYTES_V1.saturating_sub(request.len());
                if remaining == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "admin request headers exceed the first-release limit",
                    ));
                }
                let read_capacity = remaining.min(buffer.len());
                let read = reader.read(&mut buffer[..read_capacity]).await?;
                if read == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "admin request ended before its headers were complete",
                    ));
                }
                request.extend_from_slice(&buffer[..read]);
                if let Some(header_end) = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|offset| offset + 4)
                {
                    request.truncate(header_end);
                    break;
                }
            }
            Ok::<_, io::Error>(request)
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "admin request timed out"))??;
        String::from_utf8(bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "admin request headers must be valid UTF-8",
            )
        })
    }
    fn try_admin_connection_permit(permits: &Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
        Arc::clone(permits).try_acquire_owned().ok()
    }
    fn admin_http_response(
        status: &str,
        content_type: &str,
        body: &str,
        extra_headers: &str,
    ) -> String {
        format!(
            concat!(
                "HTTP/1.1 {status}\r\n",
                "content-type: {content_type}\r\n",
                "content-length: {length}\r\n",
                "cache-control: no-store\r\n",
                "{extra_headers}",
                "connection: close\r\n",
                "\r\n",
                "{body}"
            ),
            status = status,
            content_type = content_type,
            length = body.len(),
            extra_headers = extra_headers,
            body = body,
        )
    }
    fn admin_bearer_token(request: &str) -> Option<&str> {
        let headers = request.strip_suffix("\r\n\r\n")?;
        let mut token = None;
        let mut host_seen = false;
        for line in headers.split("\r\n").skip(1) {
            if line.is_empty()
                || line.starts_with(' ')
                || line.starts_with('\t')
                || line.contains('\r')
                || line.contains('\n')
                || !line.is_ascii()
            {
                return None;
            }
            let Some((name, value)) = line.split_once(':') else {
                return None;
            };
            if name.is_empty()
                || !name.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(
                            byte,
                            b'!' | b'#'
                                | b'$'
                                | b'%'
                                | b'&'
                                | b'\''
                                | b'*'
                                | b'+'
                                | b'-'
                                | b'.'
                                | b'^'
                                | b'_'
                                | b'`'
                                | b'|'
                                | b'~'
                        )
                })
            {
                return None;
            }
            if name.eq_ignore_ascii_case("content-length")
                || name.eq_ignore_ascii_case("transfer-encoding")
            {
                // The first-release admin protocol is bodyless. Reject framing headers rather
                // than letting a local reverse proxy and this parser disagree about boundaries.
                return None;
            }
            if name.eq_ignore_ascii_case("host") {
                if host_seen || value.trim().is_empty() {
                    return None;
                }
                host_seen = true;
            }
            if !name.eq_ignore_ascii_case("authorization") {
                continue;
            }
            if token.is_some() {
                return None;
            }
            let mut parts = value.split_ascii_whitespace();
            let scheme = parts.next()?;
            let candidate = parts.next()?;
            if !scheme.eq_ignore_ascii_case("bearer")
                || !(32..=256).contains(&candidate.len())
                || !candidate.bytes().all(|byte| byte.is_ascii_graphic())
                || parts.next().is_some()
            {
                return None;
            }
            token = Some(candidate);
        }
        token
    }
    async fn render_admin_request(
        request: &str,
        authorization: &AdminAuthorization,
        context: AdminRenderContext<'_>,
        relay_id: RelayId,
        mode: RelayMode,
    ) -> String {
        let Some(request_line) = request.lines().next() else {
            return Self::admin_http_response(
                "400 Bad Request",
                PLAIN_TEXT_CONTENT_TYPE,
                "malformed request\n",
                "",
            );
        };
        let mut fields = request_line.split_ascii_whitespace();
        let (Some(method), Some(path), Some(version)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Self::admin_http_response(
                "400 Bad Request",
                PLAIN_TEXT_CONTENT_TYPE,
                "malformed request\n",
                "",
            );
        };
        if fields.next().is_some() || !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
            return Self::admin_http_response(
                "400 Bad Request",
                PLAIN_TEXT_CONTENT_TYPE,
                "malformed request\n",
                "",
            );
        }
        if method != "GET" {
            return Self::admin_http_response(
                "405 Method Not Allowed",
                PLAIN_TEXT_CONTENT_TYPE,
                "method not allowed\n",
                "allow: GET\r\n",
            );
        }
        let authorized = Self::admin_bearer_token(request)
            .is_some_and(|candidate| authorization.matches(candidate));
        if !authorized {
            return Self::admin_http_response(
                "401 Unauthorized",
                PLAIN_TEXT_CONTENT_TYPE,
                "authentication required\n",
                "www-authenticate: Bearer realm=\"soranet-relay-admin\"\r\n",
            );
        }
        if path == "/healthz" {
            return Self::admin_http_response("200 OK", PLAIN_TEXT_CONTENT_TYPE, "ok\n", "");
        }
        Self::render_admin_response(path, context, relay_id, mode).await
    }
    async fn render_admin_response(
        path: &str,
        context: AdminRenderContext<'_>,
        relay_id: RelayId,
        mode: RelayMode,
    ) -> String {
        if path == "/privacy/events" {
            let body = context.privacy_events.drain_ndjson();
            return Self::admin_http_response("200 OK", NDJSON_CONTENT_TYPE, &body, "");
        }
        if path == "/policy/proxy-toggle" {
            let body = context.proxy_policy_events.drain_ndjson();
            return Self::admin_http_response("200 OK", NDJSON_CONTENT_TYPE, &body, "");
        }
        if path != "/metrics" {
            return Self::admin_http_response(
                "404 Not Found",
                PLAIN_TEXT_CONTENT_TYPE,
                "not found\n",
                "",
            );
        }
        let proxy_queue_depth = context.proxy_policy_events.queue_depth() as u64;
        let mut body = context.metrics.render_prometheus(mode, proxy_queue_depth);
        let incentive_summary = {
            let guard = context.performance.lock().await;
            guard.try_summaries()
        };
        let incentive_summary = match incentive_summary {
            Ok(summary) => summary,
            Err(error) => {
                warn!(%error, "unable to render bounded incentive metrics snapshot");
                return Self::admin_http_response(
                    "503 Service Unavailable",
                    PLAIN_TEXT_CONTENT_TYPE,
                    "metrics capacity unavailable\n",
                    "",
                );
            }
        };
        let incentives = match render_incentive_prometheus(relay_id, &incentive_summary, mode) {
            Ok(incentives) => incentives,
            Err(_) => {
                warn!("unable to render incentive metrics within its bounded response corridor");
                return Self::admin_http_response(
                    "503 Service Unavailable",
                    PLAIN_TEXT_CONTENT_TYPE,
                    "metrics capacity unavailable\n",
                    "",
                );
            }
        };
        let privacy_metrics = context.privacy.render_prometheus(mode, SystemTime::now());
        if !body.ends_with('\n') && !incentives.is_empty() {
            body.push('\n');
        }
        body.push_str(&incentives);
        if !body.ends_with('\n') && !privacy_metrics.is_empty() {
            body.push('\n');
        }
        body.push_str(&privacy_metrics);
        Self::admin_http_response("200 OK", PROMETHEUS_CONTENT_TYPE, &body, "")
    }
    async fn serve_admin(
        resources: AdminResources,
        relay_id: RelayId,
        addr: SocketAddr,
        mode: RelayMode,
        authorization: Arc<AdminAuthorization>,
    ) -> Result<(), RelayError> {
        let AdminResources {
            metrics,
            privacy,
            privacy_events,
            proxy_policy_events,
            performance,
        } = resources;
        let listener = TcpListener::bind(addr).await?;
        let actual = listener.local_addr()?;
        let connection_permits = Arc::new(Semaphore::new(ADMIN_MAX_CONCURRENT_CONNECTIONS_V1));
        info!(listen = %actual, "authenticated admin server listening");
        loop {
            let (mut stream, _peer) = listener.accept().await?;
            let Some(connection_permit) = Self::try_admin_connection_permit(&connection_permits)
            else {
                debug!("rejecting admin connection at capacity");
                continue;
            };
            let metrics = Arc::clone(&metrics);
            let privacy = Arc::clone(&privacy);
            let privacy_events = Arc::clone(&privacy_events);
            let proxy_policy_events = Arc::clone(&proxy_policy_events);
            let performance = Arc::clone(&performance);
            let authorization = authorization.clone();
            tokio::spawn(async move {
                let _connection_permit = connection_permit;
                debug!("serving admin request");
                let request = match RelayRuntime::read_admin_request(
                    &mut stream,
                    ADMIN_REQUEST_TIMEOUT,
                )
                .await
                {
                    Ok(request) => request,
                    Err(error) => {
                        debug!(%error, "failed to read admin request");
                        let (status, body) = match error.kind() {
                            io::ErrorKind::TimedOut => ("408 Request Timeout", "request timeout\n"),
                            io::ErrorKind::InvalidData => (
                                "431 Request Header Fields Too Large",
                                "invalid request headers\n",
                            ),
                            _ => ("400 Bad Request", "incomplete request\n"),
                        };
                        let response = RelayRuntime::admin_http_response(
                            status,
                            PLAIN_TEXT_CONTENT_TYPE,
                            body,
                            "",
                        );
                        let _ = timeout(
                            ADMIN_RESPONSE_TIMEOUT,
                            stream.write_all(response.as_bytes()),
                        )
                        .await;
                        return;
                    }
                };
                let context = AdminRenderContext {
                    metrics: metrics.as_ref(),
                    privacy: privacy.as_ref(),
                    privacy_events: privacy_events.as_ref(),
                    proxy_policy_events: proxy_policy_events.as_ref(),
                    performance: performance.as_ref(),
                };
                let response = RelayRuntime::render_admin_request(
                    &request,
                    &authorization,
                    context,
                    relay_id,
                    mode,
                )
                .await;
                match timeout(
                    ADMIN_RESPONSE_TIMEOUT,
                    stream.write_all(response.as_bytes()),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => debug!(%error, "failed to send admin response"),
                    Err(_) => debug!("timed out sending admin response"),
                }
                let _ = timeout(ADMIN_RESPONSE_TIMEOUT, stream.shutdown()).await;
            });
        }
    }
    fn prepare_server_transport(
        config: &RelayConfig,
        certificate_bundle: Option<&RelayCertificateBundleV2>,
        directory_valid_until_unix: Option<i64>,
    ) -> Result<(quinn::ServerConfig, Option<RelayTransportTrust>), RelayError> {
        let Some(cert_path) = config.certificate_path() else {
            return Ok((
                Self::self_signed_server_config(config.self_signed_subject())?,
                None,
            ));
        };
        let key_path = config.private_key_path().ok_or_else(|| {
            RelayError::Tls(
                "TLS private key path missing after configuration validation".to_owned(),
            )
        })?;
        let certs = Self::load_certificates(cert_path)?;
        let trust = if config.vpn_config().is_some_and(|vpn| vpn.enabled) {
            let bundle = certificate_bundle.ok_or_else(|| {
                RelayError::Tls("VPN transport requires a verified relay certificate".to_owned())
            })?;
            if !bundle.certificate.roles.exit {
                return Err(RelayError::Tls(
                    "VPN relay certificate must authorize the exit role".to_owned(),
                ));
            }
            let endpoint = select_vpn_endpoint(&bundle.certificate.endpoints).map_err(|error| {
                RelayError::Tls(format!("VPN endpoint selection failed: {error}"))
            })?;
            let leaf = certs
                .first()
                .ok_or_else(|| RelayError::Tls("TLS certificate chain is empty".to_owned()))?;
            let leaf_spki = leaf_certificate_spki_sha256(leaf.as_ref()).map_err(|error| {
                RelayError::Tls(format!("invalid TLS leaf certificate: {error}"))
            })?;
            if leaf_spki != endpoint.tls_spki_sha256 {
                return Err(RelayError::Tls(
                    "TLS leaf SPKI does not match the signed VPN endpoint pin".to_owned(),
                ));
            }
            let certificate_bytes = bundle.try_to_cbor().map_err(|error| {
                RelayError::Tls(format!(
                    "failed to encode verified relay certificate: {error}"
                ))
            })?;
            let relay_certificate_sha256: [u8; 32] = Sha256::digest(certificate_bytes).into();
            let directory_snapshot_digest = config
                .guard_directory_config()
                .ok_or_else(|| {
                    RelayError::Tls(
                        "VPN transport requires an authenticated guard directory".to_owned(),
                    )
                })?
                .expected_snapshot_digest()?;
            let valid_until_unix = directory_valid_until_unix
                .ok_or_else(|| {
                    RelayError::Tls(
                        "VPN transport requires authenticated directory validity".to_owned(),
                    )
                })?
                .min(bundle.certificate.valid_until);
            let valid_until_ms = u64::try_from(valid_until_unix)
                .ok()
                .and_then(|seconds| seconds.checked_mul(1_000))
                .ok_or_else(|| {
                    RelayError::Tls(
                        "VPN authenticated directory validity exceeds Unix milliseconds".to_owned(),
                    )
                })?;
            Some(RelayTransportTrust {
                quic_multiaddr: endpoint.quic_multiaddr.clone(),
                tls_server_name: endpoint.tls_server_name.clone(),
                tls_spki_sha256: leaf_spki,
                relay_certificate_sha256,
                directory_snapshot_digest,
                valid_until_ms,
            })
        } else {
            None
        };
        let key = Self::load_private_key(key_path)?;
        let server_config = Self::server_config(certs, key)?;
        Ok((server_config, trust))
    }
    fn self_signed_server_config(subject: &str) -> Result<quinn::ServerConfig, RelayError> {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec![subject.to_owned()])
                .map_err(|error| RelayError::Tls(error.to_string()))?;
        let cert_der = cert.der().clone();
        let key_der = PrivateKeyDer::try_from(signing_key.serialize_der())
            .map_err(|error| RelayError::Tls(error.to_string()))?;
        Self::server_config(vec![cert_der], key_der)
    }
    fn server_config(
        certs: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    ) -> Result<quinn::ServerConfig, RelayError> {
        let tls = Self::tls_server_config(certs, key)?;
        let crypto = QuinnRustlsServerConfig::try_from(Arc::new(tls))
            .map_err(|error| RelayError::Tls(error.to_string()))?;
        let mut config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        config
            .transport_config(Self::quic_transport_config())
            .max_incoming(QUIC_MAX_INCOMING_V1)
            .incoming_buffer_size(QUIC_INCOMING_BUFFER_BYTES_V1)
            .incoming_buffer_size_total(QUIC_TOTAL_INCOMING_BUFFER_BYTES_V1)
            // The relay's abuse accounting is keyed to the validated peer
            // address. Disabling migration prevents that identity from
            // changing after admission.
            .migration(false);
        Ok(config)
    }
    fn quic_transport_config() -> Arc<TransportConfig> {
        let mut transport = TransportConfig::default();
        transport
            .max_concurrent_bidi_streams(VarInt::from_u32(QUIC_MAX_BIDI_STREAMS_V1))
            .max_concurrent_uni_streams(VarInt::from_u32(QUIC_MAX_UNI_STREAMS_V1))
            .max_idle_timeout(Some(
                VarInt::from_u32(QUIC_MAX_IDLE_TIMEOUT_MILLIS_V1).into(),
            ))
            .stream_receive_window(VarInt::from_u32(QUIC_STREAM_RECEIVE_WINDOW_BYTES_V1))
            .receive_window(VarInt::from_u32(QUIC_CONNECTION_RECEIVE_WINDOW_BYTES_V1))
            .send_window(QUIC_SEND_WINDOW_BYTES_V1)
            .crypto_buffer_size(QUIC_CRYPTO_BUFFER_BYTES_V1)
            // Padding and constant-rate cover traffic use QUIC datagrams, so
            // retain the extension with fixed per-connection queue bounds.
            .datagram_receive_buffer_size(Some(QUIC_DATAGRAM_BUFFER_BYTES_V1))
            .datagram_send_buffer_size(QUIC_DATAGRAM_BUFFER_BYTES_V1)
            // The spin bit leaks an otherwise unnecessary RTT signal.
            .allow_spin(false);
        Arc::new(transport)
    }
    fn tls_server_config(
        certs: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    ) -> Result<rustls::ServerConfig, RelayError> {
        let mut tls =
            rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|error| RelayError::Tls(error.to_string()))?;
        // The signed hybrid handshake and the SNR1 record layer run after QUIC setup.
        // Replayable TLS 0-RTT data must never reach either protocol.
        tls.max_early_data_size = 0;
        tls.alpn_protocols = vec![SORANET_QUIC_ALPN.to_vec()];
        Ok(tls)
    }
    fn load_certificates(
        path: &std::path::Path,
    ) -> Result<Vec<CertificateDer<'static>>, RelayError> {
        let data = read_bounded_direct_regular_file(
            path,
            TLS_CERTIFICATE_CHAIN_MAX_BYTES_V1,
            "relay TLS certificate chain",
        )?;
        let mut certs = Vec::new();
        for entry in CertificateDer::pem_slice_iter(&data) {
            if certs.len() == TLS_CERTIFICATE_CHAIN_MAX_ENTRIES_V1 {
                return Err(RelayError::Tls(format!(
                    "relay TLS certificate chain exceeds the first-release {TLS_CERTIFICATE_CHAIN_MAX_ENTRIES_V1}-certificate limit"
                )));
            }
            certs
                .try_reserve_exact(1)
                .map_err(|_| RelayError::Tls("failed to reserve bounded TLS chain".to_owned()))?;
            let certificate = entry.map_err(|error| RelayError::Tls(error.to_string()))?;
            certs.push(certificate);
        }
        if certs.is_empty() {
            return Err(RelayError::Tls(
                "no certificates found in PEM file".to_string(),
            ));
        }
        Ok(certs)
    }
    fn load_private_key(path: &std::path::Path) -> Result<PrivateKeyDer<'static>, RelayError> {
        let mut data = read_bounded_private_regular_file(
            path,
            TLS_PRIVATE_KEY_MAX_BYTES_V1,
            "relay TLS private key",
        )?;
        let key = PrivateKeyDer::from_pem_slice(&data)
            .map_err(|error| RelayError::Tls(error.to_string()));
        data.fill(0);
        key
    }
}
include!("runtime/stream_errors.rs");
fn role_bits(mode: RelayMode) -> u8 {
    match mode {
        RelayMode::Entry => 0x01,
        RelayMode::Middle => 0x02,
        RelayMode::Exit => 0x04,
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
fn preflight_client_hello(
    client_frame: &[u8],
    server_caps: &ServerCapabilities,
) -> Result<RelayClientHelloPreflight, HandshakeError> {
    // This is deliberately the crypto engine's canonical NK2/NK3 parser. A
    // second relay-local decoder previously diverged from the live wire types
    // and rejected every current ClientHello before admission.
    let metadata = inspect_client_hello(client_frame).map_err(HandshakeError::Noise)?;
    let client_caps = parse_client_advertisement(metadata.client_capabilities())
        .map_err(HandshakeError::Capability)?;
    let negotiated =
        negotiate_capabilities(&client_caps, server_caps).map_err(HandshakeError::Capability)?;
    validate_client_selection(&negotiated, metadata.kem_id(), metadata.sig_id())?;
    Ok(RelayClientHelloPreflight {
        metadata,
        negotiated,
    })
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
fn handshake_frame_len_prefix(payload_len: usize) -> Result<[u8; 2], HandshakeError> {
    if payload_len > MAX_HANDSHAKE_FRAME_LEN {
        return Err(HandshakeError::FrameTooLarge(payload_len));
    }
    let len = u16::try_from(payload_len).map_err(|_| HandshakeError::FrameTooLarge(payload_len))?;
    Ok(len.to_be_bytes())
}
fn append_grease_tlvs(
    mut base: Vec<u8>,
    grease: &[GreaseEntry],
) -> Result<Vec<u8>, CapabilityError> {
    let mut encoded_len = base.len();
    if encoded_len > capability::MAX_CAP_VECTOR_LEN {
        return Err(CapabilityError::CapabilityVectorTooLarge);
    }
    for entry in grease {
        let len = u16::try_from(entry.value.len()).map_err(|_| {
            CapabilityError::CapabilityValueTooLarge {
                ty: entry.ty,
                length: entry.value.len(),
            }
        })?;
        encoded_len = encoded_len
            .checked_add(4)
            .and_then(|total| total.checked_add(usize::from(len)))
            .ok_or(CapabilityError::CapabilityVectorTooLarge)?;
        if encoded_len > capability::MAX_CAP_VECTOR_LEN {
            return Err(CapabilityError::CapabilityVectorTooLarge);
        }
    }
    base.try_reserve(encoded_len.saturating_sub(base.len()))
        .map_err(|_| CapabilityError::CapabilityVectorTooLarge)?;
    for entry in grease {
        base.extend_from_slice(&entry.ty.to_be_bytes());
        let len = u16::try_from(entry.value.len()).map_err(|_| {
            CapabilityError::CapabilityValueTooLarge {
                ty: entry.ty,
                length: entry.value.len(),
            }
        })?;
        base.extend_from_slice(&len.to_be_bytes());
        base.extend_from_slice(&entry.value);
    }
    Ok(base)
}
include!("runtime/handshake_diagnostics.rs");
include!("runtime/tests.rs");
