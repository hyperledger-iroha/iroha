#![allow(unexpected_cfgs)]
//! Runs the privileged Sora VPN helper and its authenticated control protocol.
use blake3::{Hasher as Blake3Hasher, hash as blake3_hash};
use clap::{Parser, Subcommand};
use hex::FromHexError;
use iroha_crypto::{
    Algorithm, KeyPair, PublicKey, Signature,
    soranet::{
        certificate::{
            leaf_certificate_spki_sha256, validate_quic_multiaddr, validate_tls_server_name,
        },
        handshake::{
            DEFAULT_CLIENT_CAPABILITIES, DEFAULT_RELAY_CAPABILITIES, RuntimeParams,
            SORANET_QUIC_ALPN, SessionSecrets, build_client_hello, client_handle_relay_hello,
        },
        record::{RecordEndpoint, RecordLayer, RecordStreamContext, RecordStreamKind},
    },
};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::{
    env,
    ffi::OsStr,
    fs,
    io::{self, Write as _},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[cfg(target_os = "linux")]
use std::{
    ffi::CStr,
    os::fd::{FromRawFd, OwnedFd},
};
iroha_crypto::define_soranet_record_io_adapters!(soranet_record_io);
use iroha_data_model::soranet::vpn::{
    VPN_CELL_LEN, VPN_USAGE_VOUCHER_CONTROL_MAGIC, VpnCellClassV1, VpnCellError, VpnCellFlagsV1,
    VpnCellHeaderV1, VpnCellV1, VpnFlowLabelV1, VpnHelperTicketV1, VpnPaddedCellV1,
    VpnUsageVoucherBodyV1, VpnUsageVoucherEnvelopeV1, VpnUsageVoucherV1,
};
use norito::{
    codec::{Decode, Encode},
    json::{self, Map as JsonMap, Number as JsonNumber, Value as JsonValue},
};
use quinn::{
    self, ClientConfig, ConnectError, Connection, ConnectionError, Dir, Endpoint, IdleTimeout,
    ReadExactError, RecvStream, SendStream, Side, StreamId, TransportConfig, VarInt,
    crypto::rustls::QuicClientConfig as QuinnRustlsClientConfig,
};
use rand::{SeedableRng, rngs::StdRng};
use rustls::{
    RootCertStore,
    client::WebPkiServerVerifier,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::CertificateDer,
};
use soranet_record_io::{RecordReader, RecordWriter};
use thiserror::Error;
use tokio::{
    io::unix::AsyncFd,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::lookup_host,
    signal::unix::{Signal, SignalKind, signal},
    time::timeout,
};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CONNECT_POLL_ATTEMPTS: usize = 50;
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SYSTEM_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const SYSTEM_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);
const MAX_SYSTEM_COMMAND_STDOUT_BYTES: usize = 256 * 1024;
const MAX_SYSTEM_COMMAND_STDERR_BYTES: usize = 64 * 1024;
const CONTROLLER_KIND: &str = "linux-helperd";
const PACKET_LEN_PREFIX_BYTES: usize = 2;
const CONNECT_PAYLOAD_FRAME_MAGIC: &[u8; 8] = b"SVPNCP1\0";
const STATE_FILE_FRAME_MAGIC: &[u8; 8] = b"SVPNST1\0";
const STATE_FILE_NAME: &str = "state.norito";
const CONTROLLER_LOCK_FILE_NAME: &str = "controller.lock";
const RESOLV_CONF_BACKUP_FILE_NAME: &str = "resolv.conf.backup";
// The first-release worker protocol is local-only, but the hidden subcommands can still be
// invoked with an arbitrary pipe. One MiB leaves room for the complete route policy while
// preventing a privileged helper from buffering an unbounded stdin stream.
const MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1: usize = 1024 * 1024;
const MAX_CONNECT_PAYLOAD_FIELD_BYTES_V1: usize = 64 * 1024;
const MAX_CONNECT_PAYLOAD_SEQUENCE_ELEMENTS_V1: usize = 4_096;
const MAX_CONNECT_PAYLOAD_TOTAL_ELEMENTS_V1: usize = 4 * 4_096;
const MAX_CONNECT_PAYLOAD_DECODE_ALLOCATION_BYTES_V1: usize = 4 * 1024 * 1024;
const MAX_CONNECT_PAYLOAD_DECODE_DEPTH_V1: usize = 8;
// Persisted state can additionally contain one captured pre-VPN route for every excluded route.
// Bound it independently so a corrupt state file cannot turn startup/status into an OOM path.
const MAX_STATE_FRAME_BYTES_V1: usize = 8 * 1024 * 1024;
const MAX_STATE_FIELD_BYTES_V1: usize = 64 * 1024;
const MAX_STATE_SEQUENCE_ELEMENTS_V1: usize = 4_096;
const MAX_STATE_TOTAL_ELEMENTS_V1: usize = 8 * 1024 * 1024;
const MAX_STATE_DECODE_ALLOCATION_BYTES_V1: usize = 16 * 1024 * 1024;
const MAX_STATE_DECODE_DEPTH_V1: usize = 16;
const MAX_RESOLV_CONF_BYTES_V1: usize = 1024 * 1024;
const MAX_SESSION_ID_BYTES_V1: usize = 256;
const MAX_RELAY_ENDPOINT_BYTES_V1: usize = 2_048;
const MAX_EXIT_CLASS_BYTES_V1: usize = 64;
const MAX_TLS_SERVER_NAME_BYTES_V1: usize = 253;
const MAX_HELPER_TICKET_HEX_BYTES_V1: usize = 64 * 1024;
const MAX_NETWORK_POLICY_ENTRIES_V1: usize = 4_096;
const MAX_NETWORK_POLICY_ENTRY_BYTES_V1: usize = 256;
const DEFAULT_ROUTE_CMD: &str = "ip";
const DEFAULT_ROUTE_SHOW_PREFIX: [&str; 2] = ["-o", "route"];
const DEFAULT_USAGE_VOUCHER_INTERVAL_MS: u64 = 1_000;
static PENDING_BYTES_IN: AtomicU64 = AtomicU64::new(0);
static PENDING_BYTES_OUT: AtomicU64 = AtomicU64::new(0);
static LAST_TRAFFIC_FLUSH_MS: AtomicU64 = AtomicU64::new(0);
static ATOMIC_FILE_NONCE: AtomicU64 = AtomicU64::new(0);
#[cfg(any(target_os = "linux", test))]
const LINUX_IFF_TUN_BITS: u16 = 0x0001;
#[cfg(any(target_os = "linux", test))]
const LINUX_IFF_NO_PI_BITS: u16 = 0x1000;
#[cfg(any(target_os = "linux", test))]
const LINUX_IFF_TUN_EXCL_BITS: u16 = 0x8000;
#[cfg(target_os = "linux")]
const LINUX_TUNSETIFF: nix::libc::c_ulong = 0x4004_54ca;
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
fn default_usage_voucher_interval_ms() -> u64 {
    DEFAULT_USAGE_VOUCHER_INTERVAL_MS
}
#[derive(Debug, Parser)]
#[command(name = "sora-vpn-controller")]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long, global = true)]
    json: bool,
}
#[derive(Debug, Subcommand)]
enum Command {
    InstallCheck,
    Status,
    #[command(about = "Connect using a bounded JSON payload read from stdin")]
    Connect,
    Disconnect,
    Repair,
    #[command(hide = true)]
    RunTunnel,
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
struct State {
    installed: bool,
    active: bool,
    controller_kind: String,
    interface_name: Option<String>,
    network_service: Option<String>,
    version: String,
    controller_path: Option<String>,
    repair_required: bool,
    bytes_in: u64,
    bytes_out: u64,
    message: String,
    worker_identity: Option<WorkerProcessIdentity>,
    session_id: Option<String>,
    relay_endpoint: Option<String>,
    applied_network: Option<AppliedNetworkState>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            installed: true,
            active: false,
            controller_kind: CONTROLLER_KIND.to_owned(),
            interface_name: None,
            network_service: None,
            version: VERSION.to_owned(),
            controller_path: current_controller_path(),
            repair_required: false,
            bytes_in: 0,
            bytes_out: 0,
            message: "ready".to_owned(),
            worker_identity: None,
            session_id: None,
            relay_endpoint: None,
            applied_network: None,
        }
    }
}
#[derive(Debug, Clone, Copy, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
enum WorkerRole {
    Tunnel,
}
impl WorkerRole {
    #[cfg(target_os = "linux")]
    const fn subcommand(self) -> &'static str {
        match self {
            Self::Tunnel => "run-tunnel",
        }
    }
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
struct WorkerProcessIdentity {
    pid: u32,
    start_time_ticks: u64,
    executable_device: u64,
    executable_inode: u64,
    role: WorkerRole,
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
struct ConnectPayload {
    session_id: String,
    relay_endpoint: String,
    exit_class: String,
    helper_ticket_hex: String,
    relay_id_hex: String,
    descriptor_commit_hex: String,
    tls_server_name: String,
    relay_tls_spki_sha256_hex: String,
    relay_certificate_sha256_hex: String,
    directory_snapshot_digest_hex: String,
    padding_budget_ms: u16,
    route_pushes: Vec<String>,
    excluded_routes: Vec<String>,
    dns_servers: Vec<String>,
    tunnel_addresses: Vec<String>,
    mtu_bytes: u64,
    lease_secs: u64,
    metering_private_key_seed_hex: Option<String>,
    usage_voucher_interval_ms: u64,
}
impl ConnectPayload {
    fn wipe_credentials(&mut self) {
        wipe_secret_string(&mut self.helper_ticket_hex);
        if let Some(seed) = self.metering_private_key_seed_hex.as_mut() {
            wipe_secret_string(seed);
        }
        self.metering_private_key_seed_hex = None;
    }
}
impl Drop for ConnectPayload {
    fn drop(&mut self) {
        self.wipe_credentials();
    }
}
fn wipe_secret_string(secret: &mut String) {
    // SAFETY: overwriting every byte with zero preserves UTF-8 validity and does not change the
    // string length or capacity while the mutable byte view exists.
    wipe_secret_bytes(unsafe { secret.as_mut_vec() });
    secret.clear();
}
fn wipe_secret_bytes(secret: &mut [u8]) {
    for byte in secret {
        // SAFETY: `byte` is a valid unique pointer into the supplied mutable slice.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(Ordering::SeqCst);
}
struct WipeBytes(Vec<u8>);
impl std::ops::Deref for WipeBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for WipeBytes {
    fn drop(&mut self) {
        wipe_secret_bytes(&mut self.0);
    }
}
struct SensitiveConnectJson(JsonValue);
impl Drop for SensitiveConnectJson {
    fn drop(&mut self) {
        let Some(object) = self.0.as_object_mut() else {
            return;
        };
        for key in [
            "helperTicketHex",
            "helper_ticket_hex",
            "meteringPrivateKeySeedHex",
            "metering_private_key_seed_hex",
        ] {
            if let Some(JsonValue::String(secret)) = object.get_mut(key) {
                wipe_secret_string(secret);
            }
        }
    }
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, Default)]
#[norito(decode_from_slice)]
struct AppliedNetworkState {
    interface_name: String,
    dns_backend: Option<DnsBackendState>,
    excluded_route_snapshots: Vec<ExcludedRouteSnapshot>,
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
enum DnsBackendState {
    Resolved { interface_name: String },
    ResolvConf,
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
struct ExcludedRouteSnapshot {
    cidr: String,
    family: IpFamily,
    previous_route: Option<String>,
}
type RouteViaDev = (Option<String>, Option<String>);
#[derive(Debug, Clone, Copy, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
enum IpFamily {
    V4,
    V6,
}
impl IpFamily {
    const fn flag(self) -> &'static str {
        match self {
            Self::V4 => "-4",
            Self::V6 => "-6",
        }
    }
    const fn max_prefix(self) -> u8 {
        match self {
            Self::V4 => 32,
            Self::V6 => 128,
        }
    }
    const fn as_json_label(self) -> &'static str {
        match self {
            Self::V4 => "V4",
            Self::V6 => "V6",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedMultiaddrHost {
    Ip(IpAddr),
    Dns {
        name: String,
        address_family: DnsAddressFamily,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DnsAddressFamily {
    Any,
    V4,
    V6,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedMultiaddr {
    host: ParsedMultiaddrHost,
    port: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedCidr {
    address: IpAddr,
    prefix: u8,
}
impl ParsedCidr {
    const fn family(self) -> IpFamily {
        match self.address {
            IpAddr::V4(_) => IpFamily::V4,
            IpAddr::V6(_) => IpFamily::V6,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct TunnelShutdown {
    repair_required: bool,
    message: String,
}
struct PreparedTunnel {
    device: Arc<LinuxTunDevice>,
    interface_name: String,
    network_service: Option<String>,
    applied_network: AppliedNetworkState,
    packet_read_mtu: usize,
}
#[derive(Clone, Copy)]
struct TunnelTrafficConfig {
    circuit_id: [u8; 16],
    flow_label: VpnFlowLabelV1,
    padding_budget_ms: u16,
    packet_read_mtu: usize,
}
struct LinuxTunDevice {
    file: AsyncFd<fs::File>,
    name: String,
}
#[derive(Debug, Clone, Default)]
struct UsageVoucherCounters {
    ingress_bytes: Arc<AtomicU64>,
    egress_bytes: Arc<AtomicU64>,
}
impl UsageVoucherCounters {
    fn add_ingress(&self, bytes: u64) {
        self.ingress_bytes.fetch_add(bytes, Ordering::Relaxed);
    }
    fn add_egress(&self, bytes: u64) {
        self.egress_bytes.fetch_add(bytes, Ordering::Relaxed);
    }
    fn snapshot(&self) -> (u64, u64) {
        (
            self.ingress_bytes.load(Ordering::Relaxed),
            self.egress_bytes.load(Ordering::Relaxed),
        )
    }
}
struct UsageVoucherSigner {
    key_pair: KeyPair,
    ticket: VpnHelperTicketV1,
    sequence: u64,
    started_at: Instant,
    interval: Duration,
}
#[derive(Debug)]
struct PacketStreamDecoder {
    buffer: Vec<u8>,
    expected_len: Option<usize>,
    max_packet_len: usize,
}

struct TunnelShutdownSignals {
    sigterm: Signal,
    sigint: Signal,
}
#[derive(Debug, Error)]
enum ControllerError {
    #[error("connect payload is required")]
    MissingPayload,
    #[error("invalid connect payload: {0}")]
    InvalidPayload(String),
    #[error("invalid relay multiaddr: {0}")]
    InvalidMultiaddr(String),
    #[error("invalid cidr: {0}")]
    InvalidCidr(String),
    #[error("hex decode failed: {0}")]
    Hex(#[from] FromHexError),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("connect error: {0}")]
    Connect(#[from] ConnectError),
    #[error("connection error: {0}")]
    Connection(#[from] ConnectionError),
    #[error("read error: {0}")]
    ReadExact(#[from] ReadExactError),
    #[error("write error: {0}")]
    Write(#[from] quinn::WriteError),
    #[error("stream closed: {0}")]
    ClosedStream(#[from] quinn::ClosedStream),
    #[error("vpn frame parse error: {0}")]
    VpnCell(#[from] VpnCellError),
    #[error("usage voucher signing failed: {0}")]
    Signing(#[from] iroha_crypto::Error),
    #[error("handshake error: {0}")]
    Handshake(String),
    #[error("state error: {0}")]
    State(String),
}
fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
fn run(cli: Cli) -> Result<(), ControllerError> {
    match cli.command {
        Command::InstallCheck => {
            let mut state = current_state()?;
            state.message = if state.repair_required {
                "repair required".to_owned()
            } else if state.active {
                "connected".to_owned()
            } else {
                "ready".to_owned()
            };
            persist_state(&state)?;
            print_state(&state)?;
            Ok(())
        }
        Command::Status => {
            let state = current_state()?;
            print_state(&state)?;
            Ok(())
        }
        Command::Connect => {
            let _lock = acquire_controller_action_lock()?;
            connect_command()
        }
        Command::Disconnect => {
            let _lock = acquire_controller_action_lock()?;
            disconnect_command("idle")
        }
        Command::Repair => {
            let _lock = acquire_controller_action_lock()?;
            repair_command()
        }
        Command::RunTunnel => {
            let payload = read_connect_payload_from_stdin()?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(run_tunnel_command(payload))
        }
    }
}
fn connect_command() -> Result<(), ControllerError> {
    let raw_payload = read_connect_payload_json_from_stdin()?;
    let parsed_payload = std::str::from_utf8(&raw_payload)
        .map_err(|error| {
            ControllerError::InvalidPayload(format!("connect payload stdin is not UTF-8: {error}"))
        })
        .and_then(|raw| parse_connect_payload(Some(raw)));
    drop(raw_payload);
    let mut payload = parsed_payload?;
    let mut previous_state = current_state()?;
    if let Some(existing_worker) = previous_state.worker_identity.as_ref() {
        terminate_worker(existing_worker)?;
        if !wait_for_worker_exit(existing_worker, Duration::from_secs(2))? {
            return Err(ControllerError::State(format!(
                "VPN worker {} did not exit after termination request",
                existing_worker.pid
            )));
        }
    }
    cleanup_persisted_network(&mut previous_state)?;
    let mut state = State {
        message: "starting".to_owned(),
        session_id: Some(payload.session_id.clone()),
        relay_endpoint: Some(payload.relay_endpoint.clone()),
        ..State::default()
    };
    persist_state(&state)?;
    let current_exe = env::current_exe()?;
    let payload_frame = WipeBytes(encode_connect_payload_frame(&payload)?);
    payload.wipe_credentials();
    let mut child = ProcessCommand::new(current_exe)
        .arg("run-tunnel")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // The worker is still blocked on its empty stdin here, so the PID cannot have exited and
    // been reused before the pidfd-backed identity capture.
    let child_pid = child.id();
    let child_identity = match capture_worker_identity(child_pid, WorkerRole::Tunnel) {
        Ok(identity) => identity,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    // Persist the exact blocked child identity before releasing its credential frame. This keeps
    // a worker from mutating host networking before durable state can identify it.
    state.worker_identity = Some(child_identity.clone());
    if let Err(error) = persist_state(&state) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let payload_write = child
        .stdin
        .as_mut()
        .ok_or_else(|| ControllerError::State("failed to open worker stdin".to_owned()))
        .and_then(|stdin| stdin.write_all(&payload_frame).map_err(Into::into));
    if let Err(error) = payload_write {
        let _ = child.kill();
        let _ = child.wait();
        state.worker_identity = None;
        state.message = "failed to deliver worker credentials".to_owned();
        if let Err(state_error) = persist_state(&state) {
            return Err(ControllerError::State(format!(
                "{error}; failed to clear the blocked worker identity: {state_error}"
            )));
        }
        return Err(error);
    }
    drop(child.stdin.take());
    drop(payload_frame);
    for _ in 0..CONNECT_POLL_ATTEMPTS {
        sleep_blocking(CONNECT_POLL_INTERVAL);
        let state = current_state()?;
        if state.worker_identity.as_ref() == Some(&child_identity) && state.active {
            print_state(&state)?;
            return Ok(());
        }
        if !worker_identity_alive(&child_identity)? {
            return Err(ControllerError::State(state.message));
        }
    }
    terminate_worker(&child_identity)?;
    let _ = wait_for_worker_exit(&child_identity, Duration::from_secs(2))?;
    Err(ControllerError::State(
        "timed out waiting for VPN tunnel worker to report readiness".to_owned(),
    ))
}
fn disconnect_command(message: &str) -> Result<(), ControllerError> {
    let mut state = current_state()?;
    if let Some(worker) = state.worker_identity.as_ref() {
        terminate_worker(worker)?;
        if !wait_for_worker_exit(worker, Duration::from_secs(2))? {
            return Err(ControllerError::State(format!(
                "VPN worker {} did not exit after termination request",
                worker.pid
            )));
        }
    }
    cleanup_persisted_network(&mut state)?;
    state.active = false;
    state.repair_required = false;
    state.worker_identity = None;
    state.message = message.to_owned();
    persist_state(&state)?;
    print_state(&state)?;
    Ok(())
}
fn repair_command() -> Result<(), ControllerError> {
    let mut state = current_state()?;
    if let Some(worker) = state.worker_identity.as_ref() {
        terminate_worker(worker)?;
        if !wait_for_worker_exit(worker, Duration::from_secs(2))? {
            return Err(ControllerError::State(format!(
                "VPN worker {} did not exit after termination request",
                worker.pid
            )));
        }
    }
    cleanup_persisted_network(&mut state)?;
    state.active = false;
    state.repair_required = false;
    state.worker_identity = None;
    state.message = "repaired".to_owned();
    persist_state(&state)?;
    print_state(&state)?;
    Ok(())
}
async fn run_tunnel_command(mut payload: ConnectPayload) -> Result<(), ControllerError> {
    // Install the signal handlers before any privileged network mutation. Signals delivered
    // while the handshake or synchronous setup is in progress remain queued and are consumed by
    // the packet loop, which then runs the normal rollback path.
    let mut shutdown_signals = TunnelShutdownSignals::install()?;
    let pid = std::process::id();
    let worker_identity = capture_worker_identity(pid, WorkerRole::Tunnel)?;
    let mut state = current_state()?;
    authorize_worker_start(&state, &worker_identity, &payload)?;
    state.active = false;
    state.repair_required = false;
    state.worker_identity = Some(worker_identity.clone());
    state.session_id = Some(payload.session_id.clone());
    state.relay_endpoint = Some(payload.relay_endpoint.clone());
    state.bytes_in = 0;
    state.bytes_out = 0;
    state.message = "connecting".to_owned();
    state.applied_network = None;
    persist_state(&state)?;
    let (endpoint, connection, record_layer) = match connect_and_handshake(&payload).await {
        Ok(result) => result,
        Err(error) => {
            update_terminal_state(
                false,
                false,
                Some(worker_identity.clone()),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                error.to_string(),
            )?;
            return Err(error);
        }
    };
    let (mut send, mut recv) = match timeout(CONNECT_TIMEOUT, connection.open_bi()).await {
        Ok(Ok(streams)) => streams,
        Ok(Err(error)) => {
            let failure = ControllerError::Connection(error);
            connection.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.wait_idle().await;
            update_terminal_state(
                false,
                false,
                Some(worker_identity.clone()),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                failure.to_string(),
            )?;
            return Err(failure);
        }
        Err(_) => {
            let failure =
                ControllerError::State("timed out opening relay VPN tunnel stream".to_owned());
            connection.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.wait_idle().await;
            update_terminal_state(
                false,
                false,
                Some(worker_identity.clone()),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                failure.to_string(),
            )?;
            return Err(failure);
        }
    };
    let record_stream = match record_layer.stream(record_stream_context(send.id())) {
        Ok(stream) => stream,
        Err(error) => {
            let failure = ControllerError::Handshake(error.to_string());
            connection.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.wait_idle().await;
            update_terminal_state(
                false,
                false,
                Some(worker_identity.clone()),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                failure.to_string(),
            )?;
            return Err(failure);
        }
    };
    // Validate and derive every credential-dependent runtime object before changing host routes,
    // DNS, or link state. A malformed metering seed must not strand a prepared tunnel.
    let voucher_signer = match UsageVoucherSigner::from_payload(&payload) {
        Ok(signer) => signer,
        Err(error) => {
            connection.close(0u32.into(), error.to_string().as_bytes());
            endpoint.close(0u32.into(), error.to_string().as_bytes());
            endpoint.wait_idle().await;
            update_terminal_state(
                false,
                false,
                Some(worker_identity.clone()),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                error.to_string(),
            )?;
            return Err(error);
        }
    };
    let prepared = match prepare_tunnel(&payload) {
        Ok(prepared) => prepared,
        Err(error) => {
            connection.close(0u32.into(), error.to_string().as_bytes());
            endpoint.close(0u32.into(), error.to_string().as_bytes());
            endpoint.wait_idle().await;
            update_terminal_state(
                false,
                false,
                Some(worker_identity.clone()),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                error.to_string(),
            )?;
            return Err(error);
        }
    };
    state = current_state()?;
    state.active = true;
    state.repair_required = false;
    state.interface_name = Some(prepared.interface_name.clone());
    state.network_service = prepared.network_service.clone();
    state.applied_network = Some(prepared.applied_network.clone());
    state.message = "connected".to_owned();
    if let Err(persist_error) = persist_state(&state) {
        let applied_network = prepared.applied_network.clone();
        let cleanup_result = cleanup_tunnel(prepared);
        connection.close(0u32.into(), persist_error.to_string().as_bytes());
        endpoint.close(0u32.into(), persist_error.to_string().as_bytes());
        endpoint.wait_idle().await;
        state.active = false;
        state.interface_name = None;
        state.network_service = None;
        state.worker_identity = Some(worker_identity);
        let cleanup_error = cleanup_result.err();
        state.repair_required = cleanup_error.is_some();
        state.applied_network = cleanup_error.as_ref().map(|_| applied_network);
        state.message = cleanup_error.as_ref().map_or_else(
            || format!("failed to persist connected state: {persist_error}"),
            |cleanup_error| {
                format!(
                    "failed to persist connected state: {persist_error}; cleanup also failed: {cleanup_error}"
                )
            },
        );
        let recovery_persist = persist_state(&state);
        return match (cleanup_error, recovery_persist) {
            (None, Ok(())) => Err(persist_error),
            (Some(cleanup_error), Ok(())) => Err(ControllerError::State(format!(
                "{persist_error}; cleanup also failed: {cleanup_error}"
            ))),
            (None, Err(recovery_error)) => Err(ControllerError::State(format!(
                "{persist_error}; cleaned up host networking but failed to persist the terminal state: {recovery_error}"
            ))),
            (Some(cleanup_error), Err(recovery_error)) => Err(ControllerError::State(format!(
                "{persist_error}; cleanup also failed: {cleanup_error}; failed to persist repair state: {recovery_error}"
            ))),
        };
    }
    let circuit_id = relay_session_id_from_session_id(payload.session_id.as_str());
    let flow_label = vpn_flow_label_from_session_id(circuit_id)?;
    payload.wipe_credentials();
    let voucher_counters = UsageVoucherCounters::default();
    let mut protected_send = RecordWriter::new(&mut send, record_stream.sealer);
    let mut protected_recv = RecordReader::new(&mut recv, record_stream.opener);
    let shutdown = tunnel_packet_loop(
        Arc::clone(&prepared.device),
        &mut protected_send,
        &mut protected_recv,
        TunnelTrafficConfig {
            circuit_id,
            flow_label,
            padding_budget_ms: payload.padding_budget_ms,
            packet_read_mtu: prepared.packet_read_mtu,
        },
        voucher_signer,
        voucher_counters,
        &mut shutdown_signals,
    )
    .await;
    let cleanup_result = cleanup_tunnel(prepared);
    let (repair_required, message) = match shutdown {
        Ok(exit) => {
            if let Err(error) = cleanup_result {
                (true, format!("{}; cleanup failed: {error}", exit.message))
            } else {
                (exit.repair_required, exit.message)
            }
        }
        Err(error) => {
            if let Err(cleanup_error) = cleanup_result {
                (true, format!("{error}; cleanup failed: {cleanup_error}"))
            } else {
                (false, error.to_string())
            }
        }
    };
    let _ = protected_send.shutdown().await;
    drop(protected_send);
    drop(protected_recv);
    connection.close(0u32.into(), message.as_bytes());
    endpoint.close(0u32.into(), message.as_bytes());
    endpoint.wait_idle().await;
    update_terminal_state(
        false,
        repair_required,
        None,
        payload.session_id.as_str(),
        payload.relay_endpoint.as_str(),
        message,
    )?;
    Ok(())
}
fn authorize_worker_start(
    state: &State,
    worker_identity: &WorkerProcessIdentity,
    payload: &ConnectPayload,
) -> Result<(), ControllerError> {
    let exact_start_record = !state.active
        && !state.repair_required
        && state.message == "starting"
        && state.worker_identity.as_ref() == Some(worker_identity)
        && state.session_id.as_deref() == Some(payload.session_id.as_str())
        && state.relay_endpoint.as_deref() == Some(payload.relay_endpoint.as_str())
        && state.applied_network.is_none();
    if !exact_start_record {
        return Err(ControllerError::State(
            "worker invocation is not bound to the controller's exact persisted start record"
                .to_owned(),
        ));
    }
    Ok(())
}
async fn connect_and_handshake(
    payload: &ConnectPayload,
) -> Result<(Endpoint, Connection, Arc<RecordLayer>), ControllerError> {
    let helper_ticket = WipeBytes(decode_hex(payload.helper_ticket_hex.as_str())?);
    let relay = parse_multiaddr(payload.relay_endpoint.as_str())?;
    let relay_addr = resolve_multiaddr_socket_addr(&relay)
        .await
        .map_err(|error| {
            ControllerError::State(format!(
                "failed to resolve VPN relay address {}: {error}",
                payload.relay_endpoint
            ))
        })?;
    let bind_addr = match relay_addr {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let mut endpoint = Endpoint::client(bind_addr).map_err(|error| {
        ControllerError::State(format!(
            "failed to create VPN QUIC endpoint on {bind_addr}: {error}"
        ))
    })?;
    let relay_tls_pin = parse_canonical_nonzero_hex_32(
        payload.relay_tls_spki_sha256_hex.as_str(),
        "relay TLS SPKI pin",
    )?;
    endpoint.set_default_client_config(build_client_config(relay_tls_pin).map_err(|error| {
        ControllerError::State(format!("failed to build VPN QUIC client config: {error}"))
    })?);
    let connect = endpoint
        .connect(relay_addr, payload.tls_server_name.as_str())
        .map_err(|error| {
            ControllerError::State(format!(
                "failed to start VPN QUIC connect to {relay_addr}: {error}"
            ))
        })?;
    let connection = match timeout(CONNECT_TIMEOUT, connect).await {
        Ok(Ok(connection)) => connection,
        Ok(Err(error)) => return Err(ControllerError::Connection(error)),
        Err(_) => {
            return Err(ControllerError::State(
                "timed out connecting to relay endpoint".to_owned(),
            ));
        }
    };
    let session = perform_helper_handshake(&connection, payload, helper_ticket).await?;
    let record_layer = RecordLayer::new(&session.session_key, RecordEndpoint::Client)
        .map_err(|error| ControllerError::Handshake(error.to_string()))?;
    Ok((endpoint, connection, Arc::new(record_layer)))
}
async fn resolve_multiaddr_socket_addr(
    relay: &ParsedMultiaddr,
) -> Result<SocketAddr, ControllerError> {
    match &relay.host {
        ParsedMultiaddrHost::Ip(host) => Ok(SocketAddr::new(*host, relay.port)),
        ParsedMultiaddrHost::Dns {
            name,
            address_family,
        } => lookup_host((name.as_str(), relay.port))
            .await?
            .find(|address| match *address_family {
                DnsAddressFamily::Any => true,
                DnsAddressFamily::V4 => address.is_ipv4(),
                DnsAddressFamily::V6 => address.is_ipv6(),
            })
            .ok_or_else(|| {
                ControllerError::InvalidMultiaddr(format!(
                    "dns {name} did not resolve to the signed address family"
                ))
            }),
    }
}
fn build_client_config(relay_tls_spki_sha256: [u8; 32]) -> Result<ClientConfig, ControllerError> {
    let tls_config = Arc::new(build_tls_client_config(relay_tls_spki_sha256));
    let crypto = QuinnRustlsClientConfig::try_from(tls_config)
        .map_err(|error| ControllerError::State(format!("TLS client config error: {error}")))?;
    let mut client_config = ClientConfig::new(Arc::new(crypto));
    let mut transport = TransportConfig::default();
    transport.max_concurrent_uni_streams(VarInt::from_u32(8));
    transport.max_concurrent_bidi_streams(VarInt::from_u32(8));
    transport.keep_alive_interval(Some(KEEPALIVE_INTERVAL));
    transport
        .max_idle_timeout(Some(IdleTimeout::try_from(IDLE_TIMEOUT).map_err(
            |error| ControllerError::State(format!("idle timeout error: {error}")),
        )?));
    client_config.transport_config(Arc::new(transport));
    Ok(client_config)
}
fn build_tls_client_config(relay_tls_spki_sha256: [u8; 32]) -> rustls::ClientConfig {
    let verifier: Arc<dyn ServerCertVerifier> = Arc::new(PinnedSpkiVerifier {
        relay_tls_spki_sha256,
    });
    let mut tls_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    // Helper authentication runs after QUIC setup. Keep replayable 0-RTT data
    // disabled so no future stream path can bypass that ordering.
    tls_config.enable_early_data = false;
    tls_config.alpn_protocols = vec![SORANET_QUIC_ALPN.to_vec()];
    tls_config
}
async fn perform_helper_handshake(
    connection: &Connection,
    payload: &ConnectPayload,
    helper_ticket: WipeBytes,
) -> Result<SessionSecrets, ControllerError> {
    let (mut send, mut recv) = match timeout(CONNECT_TIMEOUT, connection.open_bi()).await {
        Ok(Ok(streams)) => streams,
        Ok(Err(error)) => return Err(ControllerError::Connection(error)),
        Err(_) => {
            return Err(ControllerError::State(
                "timed out opening handshake stream".to_owned(),
            ));
        }
    };
    write_handshake_frame(&mut send, &helper_ticket).await?;
    let relay_id = parse_canonical_nonzero_hex_32(payload.relay_id_hex.as_str(), "relayIdHex")?;
    let relay_identity = PublicKey::from_bytes(Algorithm::Ed25519, &relay_id).map_err(|error| {
        ControllerError::InvalidPayload(format!("relayIdHex is not a valid Ed25519 key: {error}"))
    })?;
    let descriptor_commit = parse_canonical_nonzero_hex_32(
        payload.descriptor_commit_hex.as_str(),
        "descriptorCommitHex",
    )?;
    let admission_binding = helper_ticket_handshake_binding(payload, &helper_ticket)?;
    let params = RuntimeParams {
        descriptor_commit: &descriptor_commit,
        client_capabilities: &DEFAULT_CLIENT_CAPABILITIES,
        relay_capabilities: &DEFAULT_RELAY_CAPABILITIES,
        kem_id: 1,
        sig_id: 1,
        transport_alpn: SORANET_QUIC_ALPN,
        tls_server_name: payload.tls_server_name.as_str(),
        resume_hash: Some(&admission_binding),
    };
    let mut rng = StdRng::from_os_rng();
    let (client_hello, client_state) = build_client_hello(&params, &mut rng)
        .map_err(|error| ControllerError::Handshake(error.to_string()))?;
    write_handshake_frame(&mut send, &client_hello).await?;
    let relay_hello = read_handshake_frame(&mut recv).await?;
    let (client_finish, session) = client_handle_relay_hello(
        client_state,
        &relay_hello,
        &relay_identity,
        &params,
        &mut rng,
    )
    .map_err(|error| ControllerError::Handshake(error.to_string()))?;
    if let Some(frame) = client_finish {
        write_handshake_frame(&mut send, &frame).await?;
    }
    send.finish()?;
    Ok(session)
}
fn helper_ticket_handshake_binding(
    payload: &ConnectPayload,
    helper_ticket: &[u8],
) -> Result<[u8; 32], ControllerError> {
    fn update(hasher: &mut Blake3Hasher, value: &[u8]) {
        hasher.update(&(value.len() as u64).to_be_bytes());
        hasher.update(value);
    }
    let relay_id = parse_canonical_nonzero_hex_32(payload.relay_id_hex.as_str(), "relayIdHex")?;
    let descriptor_commit = parse_canonical_nonzero_hex_32(
        payload.descriptor_commit_hex.as_str(),
        "descriptorCommitHex",
    )?;
    let relay_spki = parse_canonical_nonzero_hex_32(
        payload.relay_tls_spki_sha256_hex.as_str(),
        "relayTlsSpkiSha256Hex",
    )?;
    let relay_certificate = parse_canonical_nonzero_hex_32(
        payload.relay_certificate_sha256_hex.as_str(),
        "relayCertificateSha256Hex",
    )?;
    let directory_snapshot = parse_canonical_nonzero_hex_32(
        payload.directory_snapshot_digest_hex.as_str(),
        "directorySnapshotDigestHex",
    )?;
    let mut hasher = Blake3Hasher::new();
    for value in [
        b"iroha.soranet.vpn.helper-handshake-binding.v2".as_slice(),
        helper_ticket,
        payload.relay_endpoint.as_bytes(),
        relay_id.as_slice(),
        descriptor_commit.as_slice(),
        relay_spki.as_slice(),
        relay_certificate.as_slice(),
        directory_snapshot.as_slice(),
        payload.tls_server_name.as_bytes(),
        SORANET_QUIC_ALPN,
    ] {
        update(&mut hasher, value);
    }
    Ok(*hasher.finalize().as_bytes())
}
async fn read_handshake_frame(recv: &mut RecvStream) -> Result<Vec<u8>, ControllerError> {
    let mut len_buf = [0u8; 2];
    recv.read_exact(&mut len_buf).await?;
    let len = usize::from(u16::from_be_bytes(len_buf));
    let mut payload = vec![0u8; len];
    recv.read_exact(&mut payload).await?;
    Ok(payload)
}
async fn write_handshake_frame(
    send: &mut SendStream,
    payload: &[u8],
) -> Result<(), ControllerError> {
    let len = u16::try_from(payload.len()).map_err(|_| {
        ControllerError::State(format!(
            "handshake frame length {} exceeds u16 length prefix",
            payload.len()
        ))
    })?;
    send.write_all(&len.to_be_bytes()).await?;
    send.write_all(payload).await?;
    Ok(())
}
fn prepare_tunnel(payload: &ConnectPayload) -> Result<PreparedTunnel, ControllerError> {
    let interface_name = desired_interface_name(payload.session_id.as_str())?;
    let mtu = normalize_mtu(payload.mtu_bytes)?;
    let tunnel_addresses = parse_tunnel_addresses(&payload.tunnel_addresses)?;
    let device = Arc::new(LinuxTunDevice::create(&interface_name)?);
    let mut applied_network = AppliedNetworkState {
        interface_name: device.name().to_owned(),
        dns_backend: None,
        excluded_route_snapshots: Vec::new(),
    };
    if let Err(error) =
        apply_tunnel_link_config(&applied_network.interface_name, mtu, &tunnel_addresses)
    {
        return Err(tunnel_prepare_error_with_cleanup(error, &applied_network));
    }
    if let Err(error) = apply_route_pushes(&applied_network.interface_name, &payload.route_pushes) {
        return Err(tunnel_prepare_error_with_cleanup(error, &applied_network));
    }
    match apply_excluded_routes(&payload.excluded_routes) {
        Ok(snapshots) => {
            applied_network.excluded_route_snapshots = snapshots;
        }
        Err(error) => {
            return Err(tunnel_prepare_error_with_cleanup(error, &applied_network));
        }
    }
    let dns_backend = match apply_dns(&applied_network.interface_name, &payload.dns_servers) {
        Ok(backend) => backend,
        Err(error) => {
            return Err(tunnel_prepare_error_with_cleanup(error, &applied_network));
        }
    };
    applied_network.dns_backend = dns_backend.clone();
    Ok(PreparedTunnel {
        device,
        interface_name: applied_network.interface_name.clone(),
        network_service: dns_backend.as_ref().map(dns_backend_label),
        applied_network,
        packet_read_mtu: usize::from(mtu),
    })
}
fn tunnel_prepare_error_with_cleanup(
    error: ControllerError,
    applied_network: &AppliedNetworkState,
) -> ControllerError {
    match cleanup_network(applied_network) {
        Ok(()) => error,
        Err(cleanup_error) => ControllerError::State(format!(
            "{error}; tunnel preparation rollback also failed: {cleanup_error}"
        )),
    }
}
fn cleanup_tunnel(prepared: PreparedTunnel) -> Result<(), ControllerError> {
    cleanup_network(&prepared.applied_network)?;
    drop(prepared);
    Ok(())
}
impl TunnelShutdownSignals {
    fn install() -> Result<Self, ControllerError> {
        Ok(Self {
            sigterm: signal(SignalKind::terminate())?,
            sigint: signal(SignalKind::interrupt())?,
        })
    }
}
async fn tunnel_packet_loop<W, R>(
    device: Arc<LinuxTunDevice>,
    send: &mut W,
    recv: &mut R,
    traffic: TunnelTrafficConfig,
    voucher_signer: Option<UsageVoucherSigner>,
    voucher_counters: UsageVoucherCounters,
    shutdown_signals: &mut TunnelShutdownSignals,
) -> Result<TunnelShutdown, ControllerError>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    let upstream = tun_to_vpn_loop(
        Arc::clone(&device),
        send,
        traffic,
        voucher_signer,
        voucher_counters.clone(),
    );
    let downstream = vpn_to_tun_loop(device, recv, voucher_counters, traffic.packet_read_mtu);
    tokio::pin!(upstream);
    tokio::pin!(downstream);
    tokio::select! {
        _ = shutdown_signals.sigterm.recv() => Ok(TunnelShutdown {
            repair_required: false,
            message: "idle".to_owned(),
        }),
        _ = shutdown_signals.sigint.recv() => Ok(TunnelShutdown {
            repair_required: false,
            message: "idle".to_owned(),
        }),
        result = &mut upstream => match result {
            Ok(()) => Ok(TunnelShutdown {
                repair_required: false,
                message: "local tunnel closed".to_owned(),
            }),
            Err(error) => Err(error),
        },
        result = &mut downstream => match result {
            Ok(()) => Ok(TunnelShutdown {
                repair_required: false,
                message: "relay tunnel closed".to_owned(),
            }),
            Err(error) => Err(error),
        },
    }
}
async fn tun_to_vpn_loop<W>(
    device: Arc<LinuxTunDevice>,
    send: &mut W,
    traffic: TunnelTrafficConfig,
    mut voucher_signer: Option<UsageVoucherSigner>,
    voucher_counters: UsageVoucherCounters,
) -> Result<(), ControllerError>
where
    W: AsyncWrite + Unpin,
{
    let TunnelTrafficConfig {
        circuit_id,
        flow_label,
        padding_budget_ms,
        packet_read_mtu,
    } = traffic;
    let mut packet_buf = vec![0u8; packet_read_mtu.max(512)];
    let mut sequence = 0u64;
    if let Some(signer) = voucher_signer.as_mut() {
        send_usage_voucher_control_cell(
            send,
            circuit_id,
            flow_label,
            padding_budget_ms,
            &voucher_counters,
            signer,
            &mut sequence,
        )
        .await?;
    }
    let mut voucher_interval = tokio::time::interval(
        voucher_signer
            .as_ref()
            .map(|signer| signer.interval)
            .unwrap_or(Duration::from_secs(60 * 60)),
    );
    voucher_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            packet = device.recv(&mut packet_buf) => {
                let packet_len = packet?;
                if packet_len == 0 {
                    continue;
                }
                voucher_counters.add_egress(packet_len as u64);
                let encoded = encode_packet_stream_frame(&packet_buf[..packet_len])?;
                for chunk in encoded.chunks(VpnCellV1::max_payload_len()) {
                    let cell = VpnCellV1 {
                        header: VpnCellHeaderV1 {
                            version: 1,
                            class: VpnCellClassV1::Data,
                            flags: VpnCellFlagsV1::new(false, false, false, false),
                            circuit_id,
                            flow_label,
                            sequence,
                            ack: 0,
                            padding_budget_ms,
                            payload_len: 0,
                        },
                        payload: chunk.to_vec(),
                    };
                    let padded = cell.into_padded_frame()?;
                    send.write_all(padded.as_ref()).await?;
                    sequence = sequence.saturating_add(1);
                }
                add_traffic_bytes(0, packet_len as u64)?;
            }
            _ = voucher_interval.tick(), if voucher_signer.is_some() => {
                if let Some(signer) = voucher_signer.as_mut() {
                    send_usage_voucher_control_cell(
                        send,
                        circuit_id,
                        flow_label,
                        padding_budget_ms,
                        &voucher_counters,
                        signer,
                        &mut sequence,
                    ).await?;
                }
            }
        }
    }
}
async fn read_exact_or_eof<R>(reader: &mut R, buffer: &mut [u8]) -> io::Result<bool>
where
    R: AsyncRead + Unpin,
{
    let mut received = 0;
    while received < buffer.len() {
        let read = reader.read(&mut buffer[received..]).await?;
        if read == 0 {
            return if received == 0 {
                Ok(false)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "stream ended after {received} of {} expected bytes",
                        buffer.len()
                    ),
                ))
            };
        }
        received += read;
    }
    Ok(true)
}
async fn vpn_to_tun_loop<R>(
    device: Arc<LinuxTunDevice>,
    recv: &mut R,
    voucher_counters: UsageVoucherCounters,
    packet_read_mtu: usize,
) -> Result<(), ControllerError>
where
    R: AsyncRead + Unpin,
{
    let mut decoder = PacketStreamDecoder::new(packet_read_mtu)?;
    let mut frame = [0u8; VPN_CELL_LEN];
    loop {
        match read_exact_or_eof(recv, &mut frame).await {
            Ok(true) => {
                let cell = VpnPaddedCellV1::parse_bytes_with_flow_label_bits(
                    &frame,
                    VpnFlowLabelV1::MAX_BITS,
                )?;
                if cell.header.class != VpnCellClassV1::Data {
                    continue;
                }
                for packet in decoder.ingest(&cell.payload)? {
                    device.send(&packet).await?;
                    voucher_counters.add_ingress(packet.len() as u64);
                    add_traffic_bytes(packet.len() as u64, 0)?;
                }
            }
            Ok(false) => return Ok(()),
            Err(error) => {
                return Err(ControllerError::State(format!(
                    "relay read failed: {error}"
                )));
            }
        }
    }
}
impl UsageVoucherSigner {
    fn from_payload(payload: &ConnectPayload) -> Result<Option<Self>, ControllerError> {
        let Some(seed_hex) = payload.metering_private_key_seed_hex.as_deref() else {
            return Ok(None);
        };
        let mut seed = parse_fixed_hex_32(seed_hex, "metering private key seed")?;
        let ticket = decode_helper_ticket_metadata(payload.helper_ticket_hex.as_str())?;
        let expected_session_id = relay_session_id_from_session_id(payload.session_id.as_str());
        if ticket.session_id != expected_session_id {
            return Err(ControllerError::InvalidPayload(
                "helper ticket session id does not match connect sessionId".to_owned(),
            ));
        }
        let key_pair_result = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519);
        wipe_secret_bytes(&mut seed);
        let key_pair = key_pair_result.map_err(|err| {
            ControllerError::InvalidPayload(format!(
                "metering private key seed was rejected: {err}"
            ))
        })?;
        if key_pair.public_key() != &ticket.metering_public_key {
            return Err(ControllerError::InvalidPayload(
                "metering private key does not match helper ticket public key".to_owned(),
            ));
        }
        let now = Instant::now();
        Ok(Some(Self {
            key_pair,
            ticket,
            sequence: 0,
            started_at: now,
            interval: Duration::from_millis(payload.usage_voucher_interval_ms.max(1)),
        }))
    }
    fn build_envelope(
        &mut self,
        counters: &UsageVoucherCounters,
    ) -> Result<VpnUsageVoucherEnvelopeV1, ControllerError> {
        let (ingress_bytes, egress_bytes) = counters.snapshot();
        let active_ms = self
            .started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let body = VpnUsageVoucherBodyV1 {
            session_id: self.ticket.session_id,
            quote_id: self.ticket.quote_id,
            relay_id: self.ticket.relay_id,
            sequence: self.sequence,
            ingress_bytes,
            egress_bytes,
            active_ms,
            issued_at_ms: unix_now_ms()?,
        };
        let voucher = VpnUsageVoucherV1 {
            signature: Signature::try_new(self.key_pair.private_key(), &body.encode())?,
            client_public_key: self.key_pair.public_key().clone(),
            body,
        };
        let earned_fee = self
            .ticket
            .tariff
            .earned_fee(&voucher.body)
            .map_err(|error| {
                ControllerError::State(format!("usage voucher tariff arithmetic failed: {error}"))
            })?;
        self.sequence = self.sequence.saturating_add(1);
        Ok(VpnUsageVoucherEnvelopeV1 {
            voucher,
            earned_fee,
        })
    }
}
async fn send_usage_voucher_control_cell<W>(
    send: &mut W,
    circuit_id: [u8; 16],
    flow_label: VpnFlowLabelV1,
    padding_budget_ms: u16,
    counters: &UsageVoucherCounters,
    signer: &mut UsageVoucherSigner,
    sequence: &mut u64,
) -> Result<(), ControllerError>
where
    W: AsyncWrite + Unpin,
{
    let envelope = signer.build_envelope(counters)?;
    let encoded = envelope.encode();
    let mut payload = Vec::with_capacity(
        VPN_USAGE_VOUCHER_CONTROL_MAGIC
            .len()
            .saturating_add(encoded.len()),
    );
    payload.extend_from_slice(VPN_USAGE_VOUCHER_CONTROL_MAGIC);
    payload.extend_from_slice(&encoded);
    if payload.len() > VpnCellV1::max_payload_len() {
        return Err(ControllerError::State(
            "usage voucher control payload exceeds vpn cell payload capacity".to_owned(),
        ));
    }
    let cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Control,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id,
            flow_label,
            sequence: *sequence,
            ack: 0,
            padding_budget_ms,
            payload_len: 0,
        },
        payload,
    };
    let padded = cell.into_padded_frame()?;
    send.write_all(padded.as_ref()).await?;
    *sequence = (*sequence).saturating_add(1);
    Ok(())
}
fn decode_helper_ticket_metadata(hex_ticket: &str) -> Result<VpnHelperTicketV1, ControllerError> {
    let bytes = WipeBytes(decode_hex(hex_ticket)?);
    VpnHelperTicketV1::decode_unverified(&bytes).map_err(|error| {
        ControllerError::InvalidPayload(format!("helperTicketHex has invalid v1 metadata: {error}"))
    })
}
fn unix_now_ms() -> Result<u64, ControllerError> {
    unix_time_ms_at(SystemTime::now())
}
fn unix_time_ms_at(now: SystemTime) -> Result<u64, ControllerError> {
    let elapsed = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ControllerError::State("system clock is before the Unix epoch".to_owned()))?;
    Ok(elapsed.as_millis().min(u128::from(u64::MAX)) as u64)
}
impl PacketStreamDecoder {
    fn new(max_packet_len: usize) -> Result<Self, ControllerError> {
        if max_packet_len == 0 || max_packet_len > usize::from(u16::MAX) {
            return Err(ControllerError::State(format!(
                "packet-stream MTU must be within 1..={}, got {max_packet_len}",
                u16::MAX
            )));
        }
        Ok(Self {
            buffer: Vec::new(),
            expected_len: None,
            max_packet_len,
        })
    }

    fn ingest(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, ControllerError> {
        self.buffer.extend_from_slice(bytes);
        let mut packets = Vec::new();
        loop {
            if self.expected_len.is_none() {
                if self.buffer.len() < PACKET_LEN_PREFIX_BYTES {
                    break;
                }
                let len = usize::from(u16::from_be_bytes([self.buffer[0], self.buffer[1]]));
                if len == 0 || len > self.max_packet_len {
                    return Err(ControllerError::State(format!(
                        "packet-stream frame length {len} is outside 1..={} negotiated MTU bytes",
                        self.max_packet_len
                    )));
                }
                self.buffer.drain(..PACKET_LEN_PREFIX_BYTES);
                self.expected_len = Some(len);
            }
            let Some(expected_len) = self.expected_len else {
                break;
            };
            if self.buffer.len() < expected_len {
                break;
            }
            let packet = self.buffer.drain(..expected_len).collect::<Vec<_>>();
            self.expected_len = None;
            packets.push(packet);
        }
        Ok(packets)
    }
}
impl LinuxTunDevice {
    #[cfg(target_os = "linux")]
    fn create(requested_name: &str) -> Result<Self, ControllerError> {
        let name_bytes = requested_name.as_bytes();
        if name_bytes.is_empty() || name_bytes.len() >= nix::libc::IFNAMSIZ {
            return Err(ControllerError::State(format!(
                "invalid Linux interface name {requested_name}"
            )));
        }
        let fd = unsafe {
            nix::libc::open(
                c"/dev/net/tun".as_ptr() as *const _,
                nix::libc::O_RDWR | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(ControllerError::Io(io::Error::last_os_error()));
        }
        let mut req = unsafe { std::mem::zeroed::<nix::libc::ifreq>() };
        unsafe {
            std::ptr::copy_nonoverlapping(
                name_bytes.as_ptr() as *const nix::libc::c_char,
                req.ifr_name.as_mut_ptr(),
                name_bytes.len(),
            );
            req.ifr_ifru.ifru_flags = linux_tun_creation_flags();
        }
        let ioctl_result = unsafe { nix::libc::ioctl(fd, LINUX_TUNSETIFF as _, &req) };
        if ioctl_result < 0 {
            let error = io::Error::last_os_error();
            unsafe {
                nix::libc::close(fd);
            }
            return Err(ControllerError::Io(error));
        }
        let name = match unsafe { CStr::from_ptr(req.ifr_name.as_ptr()) }.to_str() {
            Ok(name) => name.to_owned(),
            Err(error) => {
                unsafe {
                    nix::libc::close(fd);
                }
                return Err(ControllerError::State(format!(
                    "kernel returned a non-UTF-8 TUN interface name: {error}"
                )));
            }
        };
        if let Err(error) = ensure_exact_tun_interface_name(requested_name, &name) {
            unsafe {
                nix::libc::close(fd);
            }
            return Err(error);
        }
        let file = unsafe { fs::File::from_raw_fd(fd) };
        let file = AsyncFd::new(file)?;
        Ok(Self { file, name })
    }
    #[cfg(not(target_os = "linux"))]
    fn create(_requested_name: &str) -> Result<Self, ControllerError> {
        Err(ControllerError::State(
            "Linux system tunnels can only be created on Linux hosts.".to_owned(),
        ))
    }
    fn name(&self) -> &str {
        &self.name
    }
    async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let mut guard = self.file.readable().await?;
            match guard.try_io(|inner| {
                let mut file = inner.get_ref();
                std::io::Read::read(&mut file, buf)
            }) {
                Ok(result) => return result,
                Err(_) => continue,
            }
        }
    }
    async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        loop {
            let mut guard = self.file.writable().await?;
            match guard.try_io(|inner| {
                let mut file = inner.get_ref();
                std::io::Write::write(&mut file, buf)
            }) {
                Ok(result) => return result,
                Err(_) => continue,
            }
        }
    }
}
#[cfg(target_os = "linux")]
const fn linux_tun_creation_flags() -> nix::libc::c_short {
    linux_tun_creation_flag_bits() as nix::libc::c_short
}
#[cfg(any(target_os = "linux", test))]
const fn linux_tun_creation_flag_bits() -> u16 {
    // `IFF_TUN_EXCL` makes a name collision fail instead of attaching this privileged worker to
    // an existing interface that it would subsequently reconfigure.
    LINUX_IFF_TUN_BITS | LINUX_IFF_NO_PI_BITS | LINUX_IFF_TUN_EXCL_BITS
}
#[cfg(any(target_os = "linux", test))]
fn ensure_exact_tun_interface_name(
    requested_name: &str,
    kernel_name: &str,
) -> Result<(), ControllerError> {
    if kernel_name != requested_name {
        return Err(ControllerError::State(format!(
            "kernel returned TUN interface name {kernel_name} instead of requested {requested_name}"
        )));
    }
    Ok(())
}
fn encode_packet_stream_frame(packet: &[u8]) -> Result<Vec<u8>, ControllerError> {
    let packet_len = u16::try_from(packet.len()).map_err(|_| {
        ControllerError::State(format!(
            "packet length {} exceeds u16 packet-stream limit",
            packet.len()
        ))
    })?;
    let mut encoded = Vec::with_capacity(PACKET_LEN_PREFIX_BYTES + packet.len());
    encoded.extend_from_slice(&packet_len.to_be_bytes());
    encoded.extend_from_slice(packet);
    Ok(encoded)
}
fn add_traffic_bytes(bytes_in: u64, bytes_out: u64) -> Result<(), ControllerError> {
    if bytes_in == 0 && bytes_out == 0 {
        return Ok(());
    }
    PENDING_BYTES_IN.fetch_add(bytes_in, Ordering::Relaxed);
    PENDING_BYTES_OUT.fetch_add(bytes_out, Ordering::Relaxed);
    flush_traffic_bytes_if_due(false)
}
fn flush_traffic_bytes_if_due(force: bool) -> Result<(), ControllerError> {
    let now_ms = unix_now_ms()?;
    let last_ms = LAST_TRAFFIC_FLUSH_MS.load(Ordering::Relaxed);
    if !force && last_ms != 0 && now_ms.saturating_sub(last_ms) < 1_000 {
        return Ok(());
    }
    let bytes_in = PENDING_BYTES_IN.swap(0, Ordering::Relaxed);
    let bytes_out = PENDING_BYTES_OUT.swap(0, Ordering::Relaxed);
    if bytes_in == 0 && bytes_out == 0 {
        LAST_TRAFFIC_FLUSH_MS.store(now_ms, Ordering::Relaxed);
        return Ok(());
    }
    let mut state = current_state()?;
    state.bytes_in = state.bytes_in.saturating_add(bytes_in);
    state.bytes_out = state.bytes_out.saturating_add(bytes_out);
    if let Err(error) = persist_state(&state) {
        PENDING_BYTES_IN.fetch_add(bytes_in, Ordering::Relaxed);
        PENDING_BYTES_OUT.fetch_add(bytes_out, Ordering::Relaxed);
        return Err(error);
    }
    LAST_TRAFFIC_FLUSH_MS.store(now_ms, Ordering::Relaxed);
    Ok(())
}
fn flush_traffic_bytes() -> Result<(), ControllerError> {
    flush_traffic_bytes_if_due(true)
}
fn cleanup_persisted_network(state: &mut State) -> Result<(), ControllerError> {
    let restored_persisted_resolver = state.applied_network.as_ref().is_some_and(|applied| {
        matches!(
            applied.dns_backend.as_ref(),
            Some(DnsBackendState::ResolvConf)
        )
    });
    if let Some(applied) = state.applied_network.take() {
        cleanup_network(&applied)?;
    }
    if !restored_persisted_resolver {
        match fs::symlink_metadata(resolv_conf_backup_path()) {
            Ok(_) => cleanup_dns(&DnsBackendState::ResolvConf)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    state.network_service = None;
    Ok(())
}
fn cleanup_network(applied: &AppliedNetworkState) -> Result<(), ControllerError> {
    let mut failures = Vec::new();
    if let Some(dns_backend) = &applied.dns_backend
        && let Err(error) = cleanup_dns(dns_backend)
    {
        failures.push(format!("DNS cleanup failed: {error}"));
    }
    for snapshot in applied.excluded_route_snapshots.iter().rev() {
        if let Err(error) = restore_excluded_route(snapshot) {
            failures.push(format!(
                "failed to restore excluded route {}: {error}",
                snapshot.cidr
            ));
        }
    }
    if !failures.is_empty() {
        return Err(ControllerError::State(failures.join("; ")));
    }
    Ok(())
}
fn apply_tunnel_link_config(
    interface_name: &str,
    mtu: u16,
    tunnel_addresses: &[ParsedCidr],
) -> Result<(), ControllerError> {
    run_command(
        DEFAULT_ROUTE_CMD,
        vec![
            "link".to_owned(),
            "set".to_owned(),
            "dev".to_owned(),
            interface_name.to_owned(),
            "mtu".to_owned(),
            mtu.to_string(),
            "up".to_owned(),
        ],
    )?;
    for address in tunnel_addresses {
        run_command(
            DEFAULT_ROUTE_CMD,
            tunnel_address_add_args(interface_name, *address),
        )?;
    }
    Ok(())
}
fn apply_route_pushes(interface_name: &str, routes: &[String]) -> Result<(), ControllerError> {
    for route in routes {
        let parsed = parse_cidr(route)?;
        run_command(
            DEFAULT_ROUTE_CMD,
            tunnel_route_add_args(interface_name, parsed),
        )?;
    }
    Ok(())
}
fn tunnel_address_add_args(interface_name: &str, address: ParsedCidr) -> Vec<String> {
    vec![
        address.family().flag().to_owned(),
        "address".to_owned(),
        "add".to_owned(),
        format!("{}/{}", address.address, address.prefix),
        "dev".to_owned(),
        interface_name.to_owned(),
    ]
}
fn tunnel_route_add_args(interface_name: &str, route: ParsedCidr) -> Vec<String> {
    vec![
        route.family().flag().to_owned(),
        "route".to_owned(),
        "add".to_owned(),
        format!("{}/{}", route.address, route.prefix),
        "dev".to_owned(),
        interface_name.to_owned(),
    ]
}
fn apply_excluded_routes(routes: &[String]) -> Result<Vec<ExcludedRouteSnapshot>, ControllerError> {
    let mut snapshots = Vec::with_capacity(routes.len());
    for route in routes {
        let applied = (|| -> Result<ExcludedRouteSnapshot, ControllerError> {
            let normalized = route.trim().to_owned();
            let parsed = parse_cidr(&normalized)?;
            let previous_route = capture_existing_route(parsed.family(), &normalized)?;
            let default_route = capture_default_route(parsed.family())?;
            let Some((via, dev)) = default_route else {
                return Err(ControllerError::State(format!(
                    "cannot install excluded route {normalized}: no system default route for {}",
                    match parsed.family() {
                        IpFamily::V4 => "IPv4",
                        IpFamily::V6 => "IPv6",
                    }
                )));
            };
            let mut args = vec![
                parsed.family().flag().to_owned(),
                "route".to_owned(),
                "replace".to_owned(),
                normalized.clone(),
            ];
            if let Some(via) = via {
                args.push("via".to_owned());
                args.push(via);
            }
            if let Some(dev) = dev {
                args.push("dev".to_owned());
                args.push(dev);
            }
            run_command(DEFAULT_ROUTE_CMD, args)?;
            Ok(ExcludedRouteSnapshot {
                cidr: normalized,
                family: parsed.family(),
                previous_route,
            })
        })();
        match applied {
            Ok(snapshot) => snapshots.push(snapshot),
            Err(error) => {
                let rollback = snapshots
                    .iter()
                    .rev()
                    .filter_map(|snapshot| {
                        restore_excluded_route(snapshot)
                            .err()
                            .map(|rollback_error| format!("{}: {rollback_error}", snapshot.cidr))
                    })
                    .collect::<Vec<_>>();
                if rollback.is_empty() {
                    return Err(error);
                }
                return Err(ControllerError::State(format!(
                    "{error}; excluded-route rollback also failed: {}",
                    rollback.join("; ")
                )));
            }
        }
    }
    Ok(snapshots)
}
fn capture_default_route(family: IpFamily) -> Result<Option<RouteViaDev>, ControllerError> {
    let output = run_command(
        DEFAULT_ROUTE_CMD,
        vec![
            family.flag().to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[0].to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[1].to_owned(),
            "default".to_owned(),
        ],
    )?;
    let Some(line) = output.lines().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };
    Ok(Some(parse_route_via_dev(line)))
}
fn capture_existing_route(family: IpFamily, cidr: &str) -> Result<Option<String>, ControllerError> {
    let output = run_command(
        DEFAULT_ROUTE_CMD,
        vec![
            family.flag().to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[0].to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[1].to_owned(),
            cidr.to_owned(),
        ],
    )?;
    Ok(output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_owned()))
}
fn restore_excluded_route(snapshot: &ExcludedRouteSnapshot) -> Result<(), ControllerError> {
    if let Some(previous_route) = &snapshot.previous_route {
        let mut args = vec![
            snapshot.family.flag().to_owned(),
            "route".to_owned(),
            "replace".to_owned(),
        ];
        args.extend(previous_route.split_whitespace().map(ToOwned::to_owned));
        run_command(DEFAULT_ROUTE_CMD, args)?;
        return Ok(());
    }
    let args = vec![
        snapshot.family.flag().to_owned(),
        "route".to_owned(),
        "del".to_owned(),
        snapshot.cidr.clone(),
    ];
    match run_command(DEFAULT_ROUTE_CMD, args) {
        Ok(_) => Ok(()),
        Err(ControllerError::State(message))
            if message.contains("No such process")
                || message.contains("Cannot find device")
                || message.contains("No such file or directory") =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
fn apply_dns(
    interface_name: &str,
    dns_servers: &[String],
) -> Result<Option<DnsBackendState>, ControllerError> {
    if dns_servers.is_empty() {
        return Ok(None);
    }
    if command_exists("resolvectl") {
        let apply_result = (|| -> Result<(), ControllerError> {
            let mut dns_args = vec!["dns".to_owned(), interface_name.to_owned()];
            dns_args.extend(dns_servers.iter().map(|item| item.trim().to_owned()));
            run_command("resolvectl", dns_args)?;
            run_command(
                "resolvectl",
                vec![
                    "domain".to_owned(),
                    interface_name.to_owned(),
                    "~.".to_owned(),
                ],
            )?;
            run_command(
                "resolvectl",
                vec![
                    "default-route".to_owned(),
                    interface_name.to_owned(),
                    "true".to_owned(),
                ],
            )?;
            Ok(())
        })();
        if let Err(error) = apply_result {
            return match run_command(
                "resolvectl",
                vec!["revert".to_owned(), interface_name.to_owned()],
            ) {
                Ok(_) => Err(error),
                Err(rollback_error) => Err(ControllerError::State(format!(
                    "{error}; resolved DNS rollback also failed: {rollback_error}"
                ))),
            };
        }
        return Ok(Some(DnsBackendState::Resolved {
            interface_name: interface_name.to_owned(),
        }));
    }
    let backup_path = resolv_conf_backup_path();
    let backup_bytes = read_stable_regular_file_bounded(
        Path::new("/etc/resolv.conf"),
        MAX_RESOLV_CONF_BYTES_V1,
        "resolver configuration",
    )?;
    let state_root = backup_path
        .parent()
        .ok_or_else(|| ControllerError::State("resolver backup path has no parent".to_owned()))?;
    prepare_private_state_root(state_root)?;
    match fs::symlink_metadata(&backup_path) {
        Ok(_) => {
            return Err(ControllerError::State(format!(
                "resolver backup {} already exists; repair the prior VPN state before connecting",
                backup_path.display()
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    write_file_atomic(
        &backup_path,
        &backup_bytes,
        0o600,
        true,
        "resolver configuration backup",
    )?;
    let mut rendered = String::from("# sora-vpn-controller managed resolv.conf\n");
    for server in dns_servers {
        rendered.push_str("nameserver ");
        rendered.push_str(server.trim());
        rendered.push('\n');
    }
    if let Err(error) = write_file_atomic(
        Path::new("/etc/resolv.conf"),
        rendered.as_bytes(),
        0o644,
        false,
        "resolver configuration",
    ) {
        return match write_file_atomic(
            Path::new("/etc/resolv.conf"),
            &backup_bytes,
            0o644,
            false,
            "resolver configuration rollback",
        ) {
            Ok(()) => {
                remove_private_file_durable(&backup_path, "resolver configuration backup")?;
                Err(error)
            }
            Err(rollback_error) => Err(ControllerError::State(format!(
                "{error}; resolver rollback also failed: {rollback_error}"
            ))),
        };
    }
    Ok(Some(DnsBackendState::ResolvConf))
}
fn cleanup_dns(backend: &DnsBackendState) -> Result<(), ControllerError> {
    match backend {
        DnsBackendState::Resolved { interface_name } => {
            run_command(
                "resolvectl",
                vec!["revert".to_owned(), interface_name.clone()],
            )?;
        }
        DnsBackendState::ResolvConf => {
            let backup = resolv_conf_backup_path();
            let bytes = read_private_stable_regular_file_bounded(
                &backup,
                MAX_RESOLV_CONF_BYTES_V1,
                "resolver configuration backup",
            )?;
            write_file_atomic(
                Path::new("/etc/resolv.conf"),
                &bytes,
                0o644,
                false,
                "resolver configuration",
            )?;
            remove_private_file_durable(&backup, "resolver configuration backup")?;
        }
    }
    Ok(())
}
fn dns_backend_label(backend: &DnsBackendState) -> String {
    match backend {
        DnsBackendState::Resolved { .. } => "resolvectl".to_owned(),
        DnsBackendState::ResolvConf => "resolv.conf".to_owned(),
    }
}
fn resolv_conf_backup_path() -> PathBuf {
    default_state_root().join(RESOLV_CONF_BACKUP_FILE_NAME)
}
fn command_exists(program: &str) -> bool {
    resolve_trusted_command(program).is_some()
}
fn run_command<I, S>(program: &str, args: I) -> Result<String, ControllerError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let program_path = resolve_trusted_command(program).ok_or_else(|| {
        ControllerError::State(format!("{program} was not found in trusted system paths"))
    })?;
    let collected = args
        .into_iter()
        .map(|item| item.as_ref().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    execute_system_command(program, &program_path, &collected, SYSTEM_COMMAND_TIMEOUT)
}
fn execute_system_command(
    program: &str,
    program_path: &Path,
    collected: &[String],
    command_timeout: Duration,
) -> Result<String, ControllerError> {
    let mut command = ProcessCommand::new(program_path);
    command
        .env_clear()
        .env("PATH", "/usr/sbin:/sbin:/usr/bin:/bin")
        .args(collected)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // A timed-out helper may have forked descendants that inherited the pipe descriptors. Put
    // the command in its own process group so cleanup closes every inherited descriptor instead
    // of blocking forever while joining the drain threads.
    command.process_group(0);
    let mut child = command.spawn()?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = kill_command_process_group(child.id());
            let _ = child.wait();
            return Err(ControllerError::State(format!(
                "failed to capture {program} standard output"
            )));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let _ = kill_command_process_group(child.id());
            let _ = child.wait();
            return Err(ControllerError::State(format!(
                "failed to capture {program} standard error"
            )));
        }
    };
    let stdout_thread = match std::thread::Builder::new()
        .name("sora-vpn-command-stdout".to_owned())
        .spawn(move || drain_bounded_pipe(stdout, MAX_SYSTEM_COMMAND_STDOUT_BYTES))
    {
        Ok(thread) => thread,
        Err(error) => {
            let _ = kill_command_process_group(child.id());
            let _ = child.wait();
            return Err(error.into());
        }
    };
    let stderr_thread = match std::thread::Builder::new()
        .name("sora-vpn-command-stderr".to_owned())
        .spawn(move || drain_bounded_pipe(stderr, MAX_SYSTEM_COMMAND_STDERR_BYTES))
    {
        Ok(thread) => thread,
        Err(error) => {
            let _ = kill_command_process_group(child.id());
            let _ = child.wait();
            let _ = stdout_thread.join();
            return Err(error.into());
        }
    };
    let deadline = Instant::now() + command_timeout;
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // The command contract forbids detached work. Ensure descendants cannot retain
                // pipes or continue privileged mutations after their leader exits.
                kill_command_process_group(child.id())?;
                break (status, false);
            }
            Ok(None) if Instant::now() < deadline => {
                sleep_blocking(SYSTEM_COMMAND_POLL_INTERVAL);
            }
            Ok(None) => {
                kill_command_process_group(child.id())?;
                break (child.wait()?, true);
            }
            Err(error) => {
                let _ = kill_command_process_group(child.id());
                let _ = child.wait();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(error.into());
            }
        }
    };
    let stdout_result = join_bounded_pipe(stdout_thread, "standard output");
    let stderr_result = join_bounded_pipe(stderr_thread, "standard error");
    let stdout = stdout_result?;
    let stderr = stderr_result?;
    if timed_out {
        return Err(ControllerError::State(format!(
            "{program} {} exceeded the {} second command deadline",
            collected.join(" "),
            command_timeout.as_secs_f64()
        )));
    }
    if stdout.overflow || stderr.overflow {
        return Err(ControllerError::State(format!(
            "{program} {} exceeded bounded command output limits",
            collected.join(" ")
        )));
    }
    if status.success() {
        return Ok(String::from_utf8_lossy(&stdout.bytes).into_owned());
    }
    let stderr = String::from_utf8_lossy(&stderr.bytes).trim().to_owned();
    let detail = if stderr.is_empty() {
        format!("exit status {status}")
    } else {
        stderr
    };
    Err(ControllerError::State(format!(
        "{program} {} failed: {detail}",
        collected.join(" ")
    )))
}
fn kill_command_process_group(child_pid: u32) -> io::Result<()> {
    let process_group = i32::try_from(child_pid)
        .map_err(|_| io::Error::other("child PID does not fit a Unix process-group identifier"))?;
    match nix::sys::signal::killpg(
        nix::unistd::Pid::from_raw(process_group),
        nix::sys::signal::Signal::SIGKILL,
    ) {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(error) => Err(io::Error::from_raw_os_error(error as i32)),
    }
}
#[derive(Debug, PartialEq, Eq)]
struct BoundedPipeOutput {
    bytes: Vec<u8>,
    overflow: bool,
}
fn drain_bounded_pipe<R: io::Read>(
    mut reader: R,
    max_bytes: usize,
) -> io::Result<BoundedPipeOutput> {
    let mut bytes = Vec::new();
    let mut overflow = false;
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let count = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        let remaining = max_bytes.saturating_sub(bytes.len());
        let retained = remaining.min(count);
        bytes.extend_from_slice(&chunk[..retained]);
        overflow |= retained != count;
    }
    Ok(BoundedPipeOutput { bytes, overflow })
}
fn join_bounded_pipe(
    thread: std::thread::JoinHandle<io::Result<BoundedPipeOutput>>,
    label: &str,
) -> Result<BoundedPipeOutput, ControllerError> {
    thread
        .join()
        .map_err(|_| ControllerError::State(format!("{label} drain thread panicked")))?
        .map_err(Into::into)
}
fn resolve_trusted_command(program: &str) -> Option<PathBuf> {
    if !matches!(program, "ip" | "resolvectl") {
        return None;
    }
    ["/usr/sbin", "/usr/bin", "/sbin", "/bin"]
        .into_iter()
        .map(|dir| Path::new(dir).join(program))
        .filter_map(|candidate| candidate.canonicalize().ok())
        .find(|candidate| validate_system_executable(candidate).is_ok())
}
fn validate_system_executable(path: &Path) -> Result<(), ControllerError> {
    if !path.is_absolute() {
        return Err(ControllerError::State(format!(
            "system executable {} is not absolute",
            path.display()
        )));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(ControllerError::State(format!(
            "system executable {} is not a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.uid() != 0 {
            return Err(ControllerError::State(format!(
                "system executable {} is not root-owned",
                path.display()
            )));
        }
        if metadata.mode() & 0o022 != 0 {
            return Err(ControllerError::State(format!(
                "system executable {} is group- or other-writable",
                path.display()
            )));
        }
        if metadata.mode() & 0o111 == 0 {
            return Err(ControllerError::State(format!(
                "system executable {} is not executable",
                path.display()
            )));
        }
    }
    let parent = path.parent().ok_or_else(|| {
        ControllerError::State(format!(
            "system executable {} has no parent",
            path.display()
        ))
    })?;
    validate_directory_custody(parent)
}
fn normalize_mtu(value: u64) -> Result<u16, ControllerError> {
    if value == 0 || value > u64::from(u16::MAX) {
        return Err(ControllerError::InvalidPayload(format!(
            "mtuBytes must be within 1..={}",
            u16::MAX
        )));
    }
    u16::try_from(value).map_err(|_| {
        ControllerError::InvalidPayload(format!("mtuBytes {value} does not fit into u16"))
    })
}
fn parse_tunnel_addresses(values: &[String]) -> Result<Vec<ParsedCidr>, ControllerError> {
    values.iter().map(|value| parse_cidr(value)).collect()
}
fn parse_cidr(value: &str) -> Result<ParsedCidr, ControllerError> {
    let trimmed = value.trim();
    let Some((address, prefix)) = trimmed.split_once('/') else {
        return Err(ControllerError::InvalidCidr(trimmed.to_owned()));
    };
    let address = address
        .parse::<IpAddr>()
        .map_err(|_| ControllerError::InvalidCidr(trimmed.to_owned()))?;
    let prefix = prefix
        .parse::<u8>()
        .map_err(|_| ControllerError::InvalidCidr(trimmed.to_owned()))?;
    let family = match address {
        IpAddr::V4(_) => IpFamily::V4,
        IpAddr::V6(_) => IpFamily::V6,
    };
    if prefix > family.max_prefix() {
        return Err(ControllerError::InvalidCidr(trimmed.to_owned()));
    }
    Ok(ParsedCidr { address, prefix })
}
fn desired_interface_name(session_id: &str) -> Result<String, ControllerError> {
    let digest = blake3_hash(session_id.as_bytes());
    let name = format!("srvpn{}", hex::encode(&digest.as_bytes()[..5]));
    if name.len() > 15 {
        return Err(ControllerError::State(format!(
            "derived interface name {name} exceeds Linux IFNAMSIZ"
        )));
    }
    Ok(name)
}
fn relay_session_id_from_session_id(session_id: &str) -> [u8; 16] {
    let digest = blake3_hash(session_id.as_bytes());
    let mut session_key = [0u8; 16];
    session_key.copy_from_slice(&digest.as_bytes()[..16]);
    session_key
}
fn vpn_flow_label_from_session_id(session_id: [u8; 16]) -> Result<VpnFlowLabelV1, ControllerError> {
    let value = (u32::from(session_id[0]) << 16)
        | (u32::from(session_id[1]) << 8)
        | u32::from(session_id[2]);
    VpnFlowLabelV1::from_u32(value).map_err(ControllerError::from)
}
fn parse_route_via_dev(line: &str) -> (Option<String>, Option<String>) {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    let mut via = None;
    let mut dev = None;
    let mut idx = 0usize;
    while idx < tokens.len() {
        match tokens[idx] {
            "via" if idx + 1 < tokens.len() => {
                via = Some(tokens[idx + 1].to_owned());
                idx += 2;
            }
            "dev" if idx + 1 < tokens.len() => {
                dev = Some(tokens[idx + 1].to_owned());
                idx += 2;
            }
            _ => idx += 1,
        }
    }
    (via, dev)
}
fn update_terminal_state(
    active: bool,
    repair_required: bool,
    worker_identity: Option<WorkerProcessIdentity>,
    session_id: &str,
    relay_endpoint: &str,
    message: String,
) -> Result<(), ControllerError> {
    flush_traffic_bytes()?;
    let mut state = current_state()?;
    state.active = active;
    state.repair_required = repair_required;
    state.worker_identity = worker_identity;
    state.session_id = Some(session_id.to_owned());
    state.relay_endpoint = Some(relay_endpoint.to_owned());
    apply_terminal_network_lifecycle(&mut state, active, repair_required);
    state.message = message;
    persist_state(&state)?;
    Ok(())
}
fn apply_terminal_network_lifecycle(state: &mut State, active: bool, repair_required: bool) {
    if !active && !repair_required {
        state.applied_network = None;
        state.network_service = None;
    }
}
fn current_state() -> Result<State, ControllerError> {
    let mut state = load_state()?;
    hydrate_runtime_fields(&mut state);
    scrub_stale_process(&mut state)?;
    Ok(state)
}
fn read_bounded<R: io::Read>(
    reader: &mut R,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, ControllerError> {
    let mut bytes = Vec::new();
    read_bounded_into(reader, max_bytes, label, &mut bytes)?;
    Ok(bytes)
}
fn read_sensitive_bounded<R: io::Read>(
    reader: &mut R,
    max_bytes: usize,
    label: &str,
) -> Result<WipeBytes, ControllerError> {
    let mut bytes = WipeBytes(Vec::new());
    read_bounded_into(reader, max_bytes, label, &mut bytes.0)?;
    Ok(bytes)
}
fn read_bounded_into<R: io::Read>(
    reader: &mut R,
    max_bytes: usize,
    label: &str,
    bytes: &mut Vec<u8>,
) -> Result<(), ControllerError> {
    debug_assert!(bytes.is_empty());
    let mut chunk = [0_u8; 8 * 1024];
    while bytes.len() < max_bytes {
        let remaining = max_bytes - bytes.len();
        let read_len = remaining.min(chunk.len());
        let count = loop {
            match reader.read(&mut chunk[..read_len]) {
                Ok(count) => break count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        };
        if count == 0 {
            return Ok(());
        }
        bytes.try_reserve_exact(count).map_err(|error| {
            ControllerError::InvalidPayload(format!(
                "failed to reserve storage while reading {label}: {error}"
            ))
        })?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    let mut growth_probe = [0_u8; 1];
    let extra = loop {
        match reader.read(&mut growth_probe) {
            Ok(count) => break count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    };
    if extra != 0 {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} exceeds the v1 limit of {max_bytes} bytes"
        )));
    }
    Ok(())
}
fn read_stable_regular_file_bounded(
    path: &Path,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, ControllerError> {
    read_stable_regular_file_bounded_with_policy(path, max_bytes, label, false)
}
fn read_private_stable_regular_file_bounded(
    path: &Path,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, ControllerError> {
    read_stable_regular_file_bounded_with_policy(path, max_bytes, label, true)
}
fn read_stable_regular_file_bounded_with_policy(
    path: &Path,
    max_bytes: usize,
    label: &str,
    private: bool,
) -> Result<Vec<u8>, ControllerError> {
    let before_path = fs::symlink_metadata(path)?;
    validate_regular_file_metadata(path, &before_path, label, private)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NOFOLLOW);
    let mut file = options.open(path)?;
    let before = file.metadata()?;
    validate_regular_file_metadata(path, &before, label, private)?;
    ensure_same_file(&before_path, &before, path, label)?;
    if before.len() > max_bytes as u64 {
        return Err(ControllerError::State(format!(
            "{label} {} exceeds the v1 limit of {max_bytes} bytes",
            path.display()
        )));
    }
    let bytes = read_bounded(&mut file, max_bytes, label).map_err(|error| {
        ControllerError::State(format!(
            "failed to read {label} {}: {error}",
            path.display()
        ))
    })?;
    let after = file.metadata()?;
    let after_path = fs::symlink_metadata(path)?;
    validate_regular_file_metadata(path, &after_path, label, private)?;
    ensure_same_file(&before, &after, path, label)?;
    ensure_same_file(&after, &after_path, path, label)?;
    if before.len() != after.len()
        || after.len() != bytes.len() as u64
        || metadata_modified_tuple(&before) != metadata_modified_tuple(&after)
    {
        return Err(ControllerError::State(format!(
            "{label} {} changed while it was being read",
            path.display()
        )));
    }
    Ok(bytes)
}
fn validate_regular_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    label: &str,
    private: bool,
) -> Result<(), ControllerError> {
    if !metadata.file_type().is_file() {
        return Err(ControllerError::State(format!(
            "{label} {} is not a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(ControllerError::State(format!(
                "{label} {} must have exactly one hard link",
                path.display()
            )));
        }
        if private && metadata.uid() != effective_uid() {
            return Err(ControllerError::State(format!(
                "{label} {} is not owned by the effective user",
                path.display()
            )));
        }
        if private && metadata.mode() & 0o7777 & !0o600_u32 != 0 {
            return Err(ControllerError::State(format!(
                "{label} {} grants permissions beyond owner read/write",
                path.display()
            )));
        }
    }
    Ok(())
}
#[cfg(unix)]
fn ensure_same_file(
    left: &fs::Metadata,
    right: &fs::Metadata,
    path: &Path,
    label: &str,
) -> Result<(), ControllerError> {
    if left.dev() != right.dev() || left.ino() != right.ino() {
        return Err(ControllerError::State(format!(
            "{label} {} changed identity while it was being accessed",
            path.display()
        )));
    }
    Ok(())
}
#[cfg(not(unix))]
fn ensure_same_file(
    _left: &fs::Metadata,
    _right: &fs::Metadata,
    _path: &Path,
    _label: &str,
) -> Result<(), ControllerError> {
    Ok(())
}
#[cfg(unix)]
fn metadata_modified_tuple(metadata: &fs::Metadata) -> (i64, i64, i64, i64) {
    (
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}
#[cfg(not(unix))]
fn metadata_modified_tuple(metadata: &fs::Metadata) -> Option<SystemTime> {
    metadata.modified().ok()
}
fn load_state() -> Result<State, ControllerError> {
    load_state_at(&state_path())
}
fn load_state_at(path: &Path) -> Result<State, ControllerError> {
    let parent = path.parent().ok_or_else(|| {
        ControllerError::State(format!("state path {} has no parent", path.display()))
    })?;
    match fs::symlink_metadata(parent) {
        Ok(metadata) => validate_private_state_root(parent, &metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(State::default()),
        Err(error) => return Err(error.into()),
    }
    let bytes = match read_private_stable_regular_file_bounded(
        path,
        MAX_STATE_FRAME_BYTES_V1,
        "state file",
    ) {
        Ok(bytes) => bytes,
        Err(ControllerError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(State::default());
        }
        Err(error) => return Err(error),
    };
    let state = decode_state_frame(&bytes)?;
    validate_state_invariants(&state)?;
    Ok(state)
}
fn persist_state(state: &State) -> Result<(), ControllerError> {
    let path = state_path();
    persist_state_at(&path, state)
}
fn persist_state_at(path: &Path, state: &State) -> Result<(), ControllerError> {
    validate_state_invariants(state)?;
    let parent = path.parent().ok_or_else(|| {
        ControllerError::State(format!("state path {} has no parent", path.display()))
    })?;
    prepare_private_state_root(parent)?;
    let bytes = encode_state_frame(state)?;
    write_file_atomic(path, &bytes, 0o600, true, "state file")
}
fn validate_state_invariants(state: &State) -> Result<(), ControllerError> {
    match &state.worker_identity {
        None if !state.active => Ok(()),
        Some(identity) if identity.pid > 1 => Ok(()),
        _ => Err(ControllerError::State(
            "active state must have a valid worker process identity".to_owned(),
        )),
    }
}
fn print_state(state: &State) -> Result<(), ControllerError> {
    let rendered = json::to_json(&state_json_value(state))
        .map_err(|error| ControllerError::State(format!("failed to render state: {error}")))?;
    println!("{rendered}");
    Ok(())
}
fn encode_state_frame(state: &State) -> Result<Vec<u8>, ControllerError> {
    let payload_len = state.encoded_len();
    let frame_len = STATE_FILE_FRAME_MAGIC
        .len()
        .checked_add(payload_len)
        .ok_or_else(|| ControllerError::State("state frame length overflow".to_owned()))?;
    if frame_len > MAX_STATE_FRAME_BYTES_V1 {
        return Err(ControllerError::State(format!(
            "state frame exceeds the v1 limit of {MAX_STATE_FRAME_BYTES_V1} bytes"
        )));
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(frame_len).map_err(|error| {
        ControllerError::State(format!("failed to reserve state frame storage: {error}"))
    })?;
    bytes.extend_from_slice(STATE_FILE_FRAME_MAGIC);
    state.encode_to(&mut bytes);
    debug_assert_eq!(bytes.len(), frame_len);
    Ok(bytes)
}
fn decode_state_frame(bytes: &[u8]) -> Result<State, ControllerError> {
    if bytes.len() > MAX_STATE_FRAME_BYTES_V1 {
        return Err(ControllerError::State(format!(
            "state frame exceeds the v1 limit of {MAX_STATE_FRAME_BYTES_V1} bytes"
        )));
    }
    if !bytes.starts_with(STATE_FILE_FRAME_MAGIC) {
        return Err(ControllerError::State(
            "state file is not a v1 Norito state frame".to_owned(),
        ));
    }
    let limits = norito::DecodeLimits::new(
        MAX_STATE_SEQUENCE_ELEMENTS_V1,
        MAX_STATE_FIELD_BYTES_V1,
        MAX_STATE_TOTAL_ELEMENTS_V1,
        MAX_STATE_DECODE_ALLOCATION_BYTES_V1,
        MAX_STATE_DECODE_DEPTH_V1,
    );
    norito::codec::decode_exact_from_slice_with_limits(
        &bytes[STATE_FILE_FRAME_MAGIC.len()..],
        limits,
    )
    .map_err(|error| ControllerError::State(format!("failed to decode state: {error}")))
}
fn encode_connect_payload_frame(payload: &ConnectPayload) -> Result<Vec<u8>, ControllerError> {
    validate_connect_payload_ref(payload)?;
    let payload_len = payload.encoded_len();
    let frame_len = CONNECT_PAYLOAD_FRAME_MAGIC
        .len()
        .checked_add(payload_len)
        .ok_or_else(|| {
            ControllerError::InvalidPayload("connect frame length overflow".to_owned())
        })?;
    if frame_len > MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1 {
        return Err(ControllerError::InvalidPayload(format!(
            "connect frame exceeds the v1 limit of {MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1} bytes"
        )));
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(frame_len).map_err(|error| {
        ControllerError::InvalidPayload(format!("failed to reserve connect frame storage: {error}"))
    })?;
    bytes.extend_from_slice(CONNECT_PAYLOAD_FRAME_MAGIC);
    payload.encode_to(&mut bytes);
    debug_assert_eq!(bytes.len(), frame_len);
    Ok(bytes)
}
fn decode_connect_payload_frame(bytes: &[u8]) -> Result<ConnectPayload, ControllerError> {
    if bytes.len() > MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1 {
        return Err(ControllerError::InvalidPayload(format!(
            "connect frame exceeds the v1 limit of {MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1} bytes"
        )));
    }
    if !bytes.starts_with(CONNECT_PAYLOAD_FRAME_MAGIC) {
        return Err(ControllerError::InvalidPayload(
            "worker stdin is not a v1 Norito connect-payload frame".to_owned(),
        ));
    }
    let limits = norito::DecodeLimits::new(
        MAX_CONNECT_PAYLOAD_SEQUENCE_ELEMENTS_V1,
        MAX_CONNECT_PAYLOAD_FIELD_BYTES_V1,
        MAX_CONNECT_PAYLOAD_TOTAL_ELEMENTS_V1,
        MAX_CONNECT_PAYLOAD_DECODE_ALLOCATION_BYTES_V1,
        MAX_CONNECT_PAYLOAD_DECODE_DEPTH_V1,
    );
    let decoded = norito::codec::decode_exact_from_slice_with_limits(
        &bytes[CONNECT_PAYLOAD_FRAME_MAGIC.len()..],
        limits,
    )
    .map_err(|error| {
        ControllerError::InvalidPayload(format!("failed to decode connect payload: {error}"))
    })?;
    validate_connect_payload(decoded)
}
fn state_json_value(state: &State) -> JsonValue {
    let mut map = JsonMap::new();
    insert_bool(&mut map, "installed", state.installed);
    insert_bool(&mut map, "active", state.active);
    insert_string(&mut map, "controller_kind", &state.controller_kind);
    insert_string_option(&mut map, "interface_name", state.interface_name.as_deref());
    insert_string_option(
        &mut map,
        "network_service",
        state.network_service.as_deref(),
    );
    insert_string(&mut map, "version", &state.version);
    insert_string_option(
        &mut map,
        "controller_path",
        state.controller_path.as_deref(),
    );
    insert_bool(&mut map, "repair_required", state.repair_required);
    insert_u64(&mut map, "bytes_in", state.bytes_in);
    insert_u64(&mut map, "bytes_out", state.bytes_out);
    insert_string(&mut map, "message", &state.message);
    match state.worker_identity.as_ref() {
        Some(identity) => insert_u64(&mut map, "pid", u64::from(identity.pid)),
        None => {
            map.insert("pid".to_owned(), JsonValue::Null);
        }
    }
    insert_string_option(&mut map, "session_id", state.session_id.as_deref());
    insert_string_option(&mut map, "relay_endpoint", state.relay_endpoint.as_deref());
    map.insert(
        "applied_network".to_owned(),
        state
            .applied_network
            .as_ref()
            .map(applied_network_json_value)
            .unwrap_or(JsonValue::Null),
    );
    JsonValue::Object(map)
}
fn applied_network_json_value(state: &AppliedNetworkState) -> JsonValue {
    let mut map = JsonMap::new();
    insert_string(&mut map, "interface_name", &state.interface_name);
    map.insert(
        "dns_backend".to_owned(),
        state
            .dns_backend
            .as_ref()
            .map(dns_backend_json_value)
            .unwrap_or(JsonValue::Null),
    );
    map.insert(
        "excluded_route_snapshots".to_owned(),
        JsonValue::Array(
            state
                .excluded_route_snapshots
                .iter()
                .map(excluded_route_snapshot_json_value)
                .collect(),
        ),
    );
    JsonValue::Object(map)
}
fn dns_backend_json_value(state: &DnsBackendState) -> JsonValue {
    let mut map = JsonMap::new();
    match state {
        DnsBackendState::Resolved { interface_name } => {
            insert_string(&mut map, "kind", "resolved");
            insert_string(&mut map, "interface_name", interface_name);
        }
        DnsBackendState::ResolvConf => {
            insert_string(&mut map, "kind", "resolv-conf");
        }
    }
    JsonValue::Object(map)
}
fn excluded_route_snapshot_json_value(snapshot: &ExcludedRouteSnapshot) -> JsonValue {
    let mut map = JsonMap::new();
    insert_string(&mut map, "cidr", &snapshot.cidr);
    insert_string(&mut map, "family", snapshot.family.as_json_label());
    insert_string_option(
        &mut map,
        "previous_route",
        snapshot.previous_route.as_deref(),
    );
    JsonValue::Object(map)
}
fn insert_string(map: &mut JsonMap, key: &str, value: &str) {
    map.insert(key.to_owned(), JsonValue::String(value.to_owned()));
}
fn insert_string_option(map: &mut JsonMap, key: &str, value: Option<&str>) {
    map.insert(
        key.to_owned(),
        value
            .map(|value| JsonValue::String(value.to_owned()))
            .unwrap_or(JsonValue::Null),
    );
}
fn insert_bool(map: &mut JsonMap, key: &str, value: bool) {
    map.insert(key.to_owned(), JsonValue::Bool(value));
}
fn insert_u64(map: &mut JsonMap, key: &str, value: u64) {
    map.insert(key.to_owned(), JsonValue::Number(JsonNumber::from(value)));
}
#[cfg(unix)]
struct ControllerActionLock {
    file: fs::File,
}
#[cfg(not(unix))]
struct ControllerActionLock;
#[cfg(unix)]
impl Drop for ControllerActionLock {
    fn drop(&mut self) {
        // SAFETY: `file` owns a live descriptor and `LOCK_UN` does not retain it.
        let _ = unsafe { nix::libc::flock(self.file.as_raw_fd(), nix::libc::LOCK_UN) };
    }
}
fn acquire_controller_action_lock() -> Result<ControllerActionLock, ControllerError> {
    acquire_controller_action_lock_at(&default_state_root())
}
#[cfg(unix)]
fn acquire_controller_action_lock_at(root: &Path) -> Result<ControllerActionLock, ControllerError> {
    prepare_private_state_root(root)?;
    let path = root.join(CONTROLLER_LOCK_FILE_NAME);
    let mut options = fs::OpenOptions::new();
    options
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NOFOLLOW);
    let file = options.open(&path)?;
    let metadata = file.metadata()?;
    validate_regular_file_metadata(&path, &metadata, "controller action lock", true)?;
    // SAFETY: `file` owns a live descriptor; `flock` only changes its advisory lock state.
    let result =
        unsafe { nix::libc::flock(file.as_raw_fd(), nix::libc::LOCK_EX | nix::libc::LOCK_NB) };
    if result != 0 {
        let error = io::Error::last_os_error();
        if error
            .raw_os_error()
            .is_some_and(|code| code == nix::libc::EWOULDBLOCK || code == nix::libc::EAGAIN)
        {
            return Err(ControllerError::State(
                "another VPN controller action is already in progress".to_owned(),
            ));
        }
        return Err(error.into());
    }
    Ok(ControllerActionLock { file })
}
#[cfg(not(unix))]
fn acquire_controller_action_lock_at(
    _root: &Path,
) -> Result<ControllerActionLock, ControllerError> {
    Err(ControllerError::State(
        "secure VPN controller locking is unavailable on this platform".to_owned(),
    ))
}
fn state_path() -> PathBuf {
    default_state_root().join(STATE_FILE_NAME)
}
fn default_state_root() -> PathBuf {
    // This process mutates host routing and resolver state. Never let caller-controlled
    // environment select the persistence root, including for non-root capability deployments.
    PathBuf::from("/var/lib/sora-vpn-controller")
}
fn effective_uid() -> u32 {
    #[cfg(unix)]
    {
        // SAFETY: `geteuid` has no preconditions and does not retain pointers.
        unsafe { nix::libc::geteuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}
fn prepare_private_state_root(root: &Path) -> Result<(), ControllerError> {
    if !root.is_absolute() {
        return Err(ControllerError::State(format!(
            "state root {} must be absolute",
            root.display()
        )));
    }
    match fs::symlink_metadata(root) {
        Ok(metadata) => validate_private_state_root(root, &metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = root.parent().ok_or_else(|| {
                ControllerError::State(format!("state root {} has no parent", root.display()))
            })?;
            validate_directory_custody(parent)?;
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            builder.mode(0o700);
            builder.create(root)?;
            #[cfg(unix)]
            fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
            let metadata = fs::symlink_metadata(root)?;
            validate_private_state_root(root, &metadata)
        }
        Err(error) => Err(error.into()),
    }
}
fn validate_private_state_root(
    root: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ControllerError> {
    if !metadata.file_type().is_dir() {
        return Err(ControllerError::State(format!(
            "state root {} is not a directory",
            root.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.uid() != effective_uid() {
            return Err(ControllerError::State(format!(
                "state root {} is not owned by the effective user",
                root.display()
            )));
        }
        if metadata.mode() & 0o7777 != 0o700 {
            return Err(ControllerError::State(format!(
                "state root {} must have mode 0700",
                root.display()
            )));
        }
    }
    validate_directory_custody(root)
}
fn validate_directory_custody(path: &Path) -> Result<(), ControllerError> {
    let canonical = path.canonicalize().map_err(|error| {
        ControllerError::State(format!(
            "failed to resolve directory custody for {}: {error}",
            path.display()
        ))
    })?;
    for ancestor in canonical.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        if !metadata.file_type().is_dir() {
            return Err(ControllerError::State(format!(
                "trusted path component {} is not a directory",
                ancestor.display()
            )));
        }
        #[cfg(unix)]
        {
            let owner = metadata.uid();
            if owner != 0 && owner != effective_uid() {
                return Err(ControllerError::State(format!(
                    "trusted path component {} has an unexpected owner",
                    ancestor.display()
                )));
            }
            if metadata.mode() & 0o022 != 0 {
                let root_owned_sticky = owner == 0 && metadata.mode() & 0o1000 != 0;
                if !root_owned_sticky {
                    return Err(ControllerError::State(format!(
                        "trusted path component {} is group- or other-writable",
                        ancestor.display()
                    )));
                }
            }
        }
    }
    Ok(())
}
fn write_file_atomic(
    path: &Path,
    bytes: &[u8],
    mode: u32,
    private: bool,
    label: &str,
) -> Result<(), ControllerError> {
    if mode & 0o7022 != 0 || (private && mode != 0o600) {
        return Err(ControllerError::State(format!(
            "invalid mode {mode:o} for atomic {label} write"
        )));
    }
    if !path.is_absolute() {
        return Err(ControllerError::State(format!(
            "{label} path {} must be absolute",
            path.display()
        )));
    }
    let parent = path.parent().ok_or_else(|| {
        ControllerError::State(format!("{label} path {} has no parent", path.display()))
    })?;
    validate_directory_custody(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => validate_regular_file_metadata(path, &metadata, label, private)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let file_name = path.file_name().and_then(OsStr::to_str).ok_or_else(|| {
        ControllerError::State(format!("{label} path {} has no valid name", path.display()))
    })?;
    let (temporary_path, mut temporary_file) = (0..128)
        .find_map(|_| {
            let nonce = ATOMIC_FILE_NONCE.fetch_add(1, Ordering::Relaxed);
            let temporary_path =
                parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
            let mut options = fs::OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                options.mode(mode);
                options.custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NOFOLLOW);
            }
            match options.open(&temporary_path) {
                Ok(file) => Some(Ok((temporary_path, file))),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .transpose()?
        .ok_or_else(|| {
            ControllerError::State(format!(
                "failed to allocate a temporary {label} in {}",
                parent.display()
            ))
        })?;
    let write_result = (|| -> io::Result<()> {
        #[cfg(unix)]
        temporary_file.set_permissions(fs::Permissions::from_mode(mode))?;
        temporary_file.write_all(bytes)?;
        temporary_file.sync_all()?;
        drop(temporary_file);
        fs::rename(&temporary_path, path)?;
        let directory = fs::File::open(parent)?;
        directory.sync_all()
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error.into());
    }
    let metadata = fs::symlink_metadata(path)?;
    validate_regular_file_metadata(path, &metadata, label, private)?;
    #[cfg(unix)]
    if private && metadata.permissions().mode() & 0o077 != 0 {
        return Err(ControllerError::State(format!(
            "new {label} {} is not private",
            path.display()
        )));
    }
    Ok(())
}
fn remove_private_file_durable(path: &Path, label: &str) -> Result<(), ControllerError> {
    let parent = path.parent().ok_or_else(|| {
        ControllerError::State(format!("{label} path {} has no parent", path.display()))
    })?;
    validate_directory_custody(parent)?;
    let metadata = fs::symlink_metadata(path)?;
    validate_regular_file_metadata(path, &metadata, label, true)?;
    fs::remove_file(path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
fn hydrate_runtime_fields(state: &mut State) {
    state.installed = true;
    if state.controller_kind.trim().is_empty() {
        state.controller_kind = CONTROLLER_KIND.to_owned();
    }
    if state.version.trim().is_empty() {
        state.version = VERSION.to_owned();
    }
    if state.controller_path.is_none() {
        state.controller_path = current_controller_path();
    }
}
fn current_controller_path() -> Option<String> {
    env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(ToOwned::to_owned))
}
fn scrub_stale_process(state: &mut State) -> Result<(), ControllerError> {
    let Some(identity) = state.worker_identity.as_ref() else {
        return Ok(());
    };
    if worker_identity_alive(identity)? {
        return Ok(());
    }
    state.active = false;
    state.worker_identity = None;
    state.repair_required = state.applied_network.is_some();
    if state.message == "connected" || state.message == "starting" || state.message == "connecting"
    {
        state.message = "tunnel worker exited".to_owned();
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn capture_worker_identity(
    pid: u32,
    role: WorkerRole,
) -> Result<WorkerProcessIdentity, ControllerError> {
    let pidfd = open_pidfd(pid)?.ok_or_else(|| {
        ControllerError::State(format!(
            "worker process {pid} exited before identity capture"
        ))
    })?;
    let identity = observe_linux_worker_identity(pid, role)?.ok_or_else(|| {
        ControllerError::State(format!(
            "worker process {pid} exited before identity capture"
        ))
    })?;
    let current_exe = env::current_exe()?.canonicalize()?;
    let current_metadata = fs::metadata(&current_exe)?;
    if identity.executable_device != current_metadata.dev()
        || identity.executable_inode != current_metadata.ino()
    {
        return Err(ControllerError::State(format!(
            "worker process {pid} is not running the controller executable"
        )));
    }
    if !pidfd_send_signal(&pidfd, 0)? {
        return Err(ControllerError::State(format!(
            "worker process {pid} exited during identity capture"
        )));
    }
    Ok(identity)
}
#[cfg(not(target_os = "linux"))]
fn capture_worker_identity(
    _pid: u32,
    _role: WorkerRole,
) -> Result<WorkerProcessIdentity, ControllerError> {
    Err(ControllerError::State(
        "VPN worker process identity is only supported on Linux".to_owned(),
    ))
}
#[cfg(target_os = "linux")]
fn observe_linux_worker_identity(
    pid: u32,
    role: WorkerRole,
) -> Result<Option<WorkerProcessIdentity>, ControllerError> {
    let stat_path = PathBuf::from(format!("/proc/{pid}/stat"));
    let stat = match read_small_proc_file(&stat_path, 16 * 1024) {
        Ok(stat) => stat,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let stat = std::str::from_utf8(&stat).map_err(|error| {
        ControllerError::State(format!("worker process {pid} stat is not UTF-8: {error}"))
    })?;
    let (process_state, start_time_ticks) = parse_linux_process_stat(stat).map_err(|error| {
        ControllerError::State(format!("worker process {pid} stat is malformed: {error}"))
    })?;
    if process_state == 'Z' {
        return Ok(None);
    }
    let cmdline_path = PathBuf::from(format!("/proc/{pid}/cmdline"));
    let cmdline = match read_small_proc_file(&cmdline_path, 64 * 1024) {
        Ok(cmdline) => cmdline,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut args = cmdline
        .split(|byte| *byte == 0)
        .filter(|arg| !arg.is_empty());
    let _program = args.next();
    if args.next() != Some(role.subcommand().as_bytes()) {
        return Ok(None);
    }
    let executable = match fs::metadata(format!("/proc/{pid}/exe")) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    Ok(Some(WorkerProcessIdentity {
        pid,
        start_time_ticks,
        executable_device: executable.dev(),
        executable_inode: executable.ino(),
        role,
    }))
}
#[cfg(any(target_os = "linux", test))]
fn parse_linux_process_stat(stat: &str) -> Result<(char, u64), &'static str> {
    let fields = stat
        .rsplit_once(") ")
        .ok_or("missing command terminator")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let process_state = fields
        .first()
        .and_then(|value| value.chars().next())
        .ok_or("missing process state")?;
    let start_time = fields
        .get(19)
        .ok_or("truncated before start time")?
        .parse::<u64>()
        .map_err(|_| "invalid start time")?;
    Ok((process_state, start_time))
}
#[cfg(target_os = "linux")]
fn read_small_proc_file(path: &Path, max_bytes: usize) -> io::Result<Vec<u8>> {
    use std::io::Read as _;

    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NOFOLLOW);
    let mut file = options.open(path)?;
    let mut bytes = Vec::new();
    (&mut file)
        .take((max_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} exceeds {max_bytes} bytes", path.display()),
        ));
    }
    Ok(bytes)
}
#[cfg(target_os = "linux")]
fn worker_identity_alive(identity: &WorkerProcessIdentity) -> Result<bool, ControllerError> {
    let Some(pidfd) = open_pidfd(identity.pid)? else {
        return Ok(false);
    };
    let observed = observe_linux_worker_identity(identity.pid, identity.role)?;
    Ok(pidfd_send_signal(&pidfd, 0)? && observed.as_ref() == Some(identity))
}
#[cfg(not(target_os = "linux"))]
fn worker_identity_alive(_identity: &WorkerProcessIdentity) -> Result<bool, ControllerError> {
    Err(ControllerError::State(
        "refusing to inspect a persisted VPN worker without Linux process identity support"
            .to_owned(),
    ))
}
#[cfg(target_os = "linux")]
fn open_pidfd(pid: u32) -> Result<Option<OwnedFd>, ControllerError> {
    let raw_pid = i32::try_from(pid)
        .map_err(|_| ControllerError::State(format!("invalid worker PID {pid}")))?;
    // SAFETY: `pidfd_open` receives only integer arguments and returns a new owned descriptor.
    let fd = unsafe { nix::libc::syscall(nix::libc::SYS_pidfd_open, raw_pid, 0) };
    if fd >= 0 {
        // SAFETY: a successful `pidfd_open` returns a fresh descriptor owned by this process.
        return Ok(Some(unsafe { OwnedFd::from_raw_fd(fd as i32) }));
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(nix::libc::ESRCH) {
        return Ok(None);
    }
    Err(ControllerError::State(format!(
        "failed to open a stable handle for worker process {pid}: {error}"
    )))
}
#[cfg(target_os = "linux")]
fn pidfd_send_signal(pidfd: &OwnedFd, signal: i32) -> Result<bool, ControllerError> {
    // SAFETY: `pidfd` is a live process descriptor and the remaining syscall arguments follow
    // `pidfd_send_signal(2)`; no pointer is dereferenced because `siginfo` is null.
    let result = unsafe {
        nix::libc::syscall(
            nix::libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            signal,
            std::ptr::null::<nix::libc::siginfo_t>(),
            0,
        )
    };
    if result == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(nix::libc::ESRCH) {
        return Ok(false);
    }
    Err(ControllerError::State(format!(
        "pidfd signal operation failed: {error}"
    )))
}
#[cfg(target_os = "linux")]
fn terminate_worker(identity: &WorkerProcessIdentity) -> Result<(), ControllerError> {
    let Some(pidfd) = open_pidfd(identity.pid)? else {
        return Ok(());
    };
    if !pidfd_send_signal(&pidfd, 0)? {
        return Ok(());
    }
    let observed = observe_linux_worker_identity(identity.pid, identity.role)?;
    if !pidfd_send_signal(&pidfd, 0)? {
        return Ok(());
    }
    if observed.as_ref() != Some(identity) {
        return Err(ControllerError::State(format!(
            "refusing to signal PID {} because its process identity does not match persisted VPN state",
            identity.pid
        )));
    }
    let _ = pidfd_send_signal(&pidfd, nix::libc::SIGTERM)?;
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn terminate_worker(_identity: &WorkerProcessIdentity) -> Result<(), ControllerError> {
    Err(ControllerError::State(
        "refusing to signal a persisted VPN worker without Linux pidfd support".to_owned(),
    ))
}
fn wait_for_worker_exit(
    identity: &WorkerProcessIdentity,
    timeout_limit: Duration,
) -> Result<bool, ControllerError> {
    let deadline = std::time::Instant::now() + timeout_limit;
    while worker_identity_alive(identity)? && std::time::Instant::now() < deadline {
        sleep_blocking(Duration::from_millis(50));
    }
    Ok(!worker_identity_alive(identity)?)
}
fn sleep_blocking(duration: Duration) {
    std::thread::sleep(duration);
}
fn decode_hex(value: &str) -> Result<Vec<u8>, ControllerError> {
    let trimmed = value.trim();
    let normalized = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if normalized.is_empty() || !normalized.len().is_multiple_of(2) {
        return Err(ControllerError::InvalidPayload(
            "helper ticket must be an even-length hex string".to_owned(),
        ));
    }
    Ok(hex::decode(normalized)?)
}
fn parse_fixed_hex_32(value: &str, label: &str) -> Result<[u8; 32], ControllerError> {
    let trimmed = value.trim();
    let normalized = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if normalized.len() != 64 {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} must decode to 32 bytes (got {} hexadecimal characters)",
            normalized.len()
        )));
    }
    let mut bytes = [0_u8; 32];
    hex::decode_to_slice(normalized, &mut bytes)?;
    Ok(bytes)
}
fn parse_canonical_nonzero_hex_32(value: &str, label: &str) -> Result<[u8; 32], ControllerError> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} must be exactly 64 lowercase hexadecimal characters"
        )));
    }
    let bytes: [u8; 32] = hex::decode(value)
        .expect("canonical hexadecimal validation makes decoding infallible")
        .try_into()
        .expect("64 hexadecimal characters decode to 32 bytes");
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} must not be all zero"
        )));
    }
    Ok(bytes)
}
fn parse_connect_payload(raw_payload: Option<&str>) -> Result<ConnectPayload, ControllerError> {
    let raw_payload = raw_payload.ok_or(ControllerError::MissingPayload)?;
    let value = SensitiveConnectJson(
        json::from_str(raw_payload)
            .map_err(|error| ControllerError::InvalidPayload(error.to_string()))?,
    );
    let object = value.0.as_object().ok_or_else(|| {
        ControllerError::InvalidPayload("connect payload must be a JSON object".to_owned())
    })?;
    let payload = ConnectPayload {
        session_id: require_json_string(object, &["sessionId", "session_id"], "sessionId")?,
        relay_endpoint: require_json_string(
            object,
            &["relayEndpoint", "relay_endpoint"],
            "relayEndpoint",
        )?,
        exit_class: optional_json_string(object, &["exitClass", "exit_class"])?.unwrap_or_default(),
        helper_ticket_hex: require_json_string(
            object,
            &["helperTicketHex", "helper_ticket_hex"],
            "helperTicketHex",
        )?,
        relay_id_hex: require_json_string(object, &["relayIdHex", "relay_id_hex"], "relayIdHex")?,
        descriptor_commit_hex: require_json_string(
            object,
            &["descriptorCommitHex", "descriptor_commit_hex"],
            "descriptorCommitHex",
        )?,
        tls_server_name: require_json_string(
            object,
            &["tlsServerName", "tls_server_name"],
            "tlsServerName",
        )?,
        relay_tls_spki_sha256_hex: require_json_string(
            object,
            &["relayTlsSpkiSha256Hex", "relay_tls_spki_sha256_hex"],
            "relayTlsSpkiSha256Hex",
        )?,
        relay_certificate_sha256_hex: require_json_string(
            object,
            &["relayCertificateSha256Hex", "relay_certificate_sha256_hex"],
            "relayCertificateSha256Hex",
        )?,
        directory_snapshot_digest_hex: require_json_string(
            object,
            &[
                "directorySnapshotDigestHex",
                "directory_snapshot_digest_hex",
            ],
            "directorySnapshotDigestHex",
        )?,
        padding_budget_ms: require_json_u16(
            object,
            &["paddingBudgetMs", "padding_budget_ms"],
            "paddingBudgetMs",
        )?,
        route_pushes: optional_json_string_array(object, &["routePushes", "route_pushes"])?,
        excluded_routes: optional_json_string_array(
            object,
            &["excludedRoutes", "excluded_routes"],
        )?,
        dns_servers: optional_json_string_array(object, &["dnsServers", "dns_servers"])?,
        tunnel_addresses: optional_json_string_array(
            object,
            &["tunnelAddresses", "tunnel_addresses"],
        )?,
        mtu_bytes: require_json_u64(object, &["mtuBytes", "mtu_bytes"], "mtuBytes")?,
        lease_secs: optional_json_u64(object, &["leaseSecs", "lease_secs"])?.unwrap_or_default(),
        metering_private_key_seed_hex: optional_json_string(
            object,
            &["meteringPrivateKeySeedHex", "metering_private_key_seed_hex"],
        )?,
        usage_voucher_interval_ms: optional_json_u64(
            object,
            &["usageVoucherIntervalMs", "usage_voucher_interval_ms"],
        )?
        .unwrap_or_else(default_usage_voucher_interval_ms),
    };
    validate_connect_payload(payload)
}
fn validate_connect_payload(payload: ConnectPayload) -> Result<ConnectPayload, ControllerError> {
    validate_connect_payload_ref(&payload)?;
    Ok(payload)
}
fn validate_connect_payload_ref(payload: &ConnectPayload) -> Result<(), ControllerError> {
    validate_text_field(
        payload.session_id.as_str(),
        "sessionId",
        MAX_SESSION_ID_BYTES_V1,
    )?;
    validate_text_field(
        payload.relay_endpoint.as_str(),
        "relayEndpoint",
        MAX_RELAY_ENDPOINT_BYTES_V1,
    )?;
    validate_text_field(
        payload.exit_class.as_str(),
        "exitClass",
        MAX_EXIT_CLASS_BYTES_V1,
    )?;
    validate_text_field(
        payload.helper_ticket_hex.as_str(),
        "helperTicketHex",
        MAX_HELPER_TICKET_HEX_BYTES_V1,
    )?;
    validate_text_field(
        payload.tls_server_name.as_str(),
        "tlsServerName",
        MAX_TLS_SERVER_NAME_BYTES_V1,
    )?;
    if let Some(seed) = payload.metering_private_key_seed_hex.as_deref() {
        validate_text_field(seed, "meteringPrivateKeySeedHex", 64)?;
    }
    validate_network_policy_entries(&payload.route_pushes, "routePushes")?;
    validate_network_policy_entries(&payload.excluded_routes, "excludedRoutes")?;
    validate_network_policy_entries(&payload.dns_servers, "dnsServers")?;
    validate_network_policy_entries(&payload.tunnel_addresses, "tunnelAddresses")?;
    validate_canonical_cidr_entries(&payload.route_pushes, "routePushes")?;
    validate_canonical_cidr_entries(&payload.excluded_routes, "excludedRoutes")?;
    validate_canonical_cidr_entries(&payload.tunnel_addresses, "tunnelAddresses")?;
    validate_dns_servers(&payload.dns_servers)?;
    if payload.session_id.trim().is_empty() {
        return Err(ControllerError::InvalidPayload(
            "sessionId must not be empty".to_owned(),
        ));
    }
    validate_quic_multiaddr(payload.relay_endpoint.as_str()).map_err(|error| {
        ControllerError::InvalidPayload(format!("relayEndpoint is not canonical: {error}"))
    })?;
    validate_tls_server_name(payload.tls_server_name.as_str()).map_err(|error| {
        ControllerError::InvalidPayload(format!("tlsServerName is not canonical: {error}"))
    })?;
    if payload.helper_ticket_hex.trim().is_empty() {
        return Err(ControllerError::InvalidPayload(
            "helperTicketHex must not be empty".to_owned(),
        ));
    }
    let relay_id = parse_canonical_nonzero_hex_32(payload.relay_id_hex.as_str(), "relayIdHex")?;
    PublicKey::from_bytes(Algorithm::Ed25519, &relay_id).map_err(|error| {
        ControllerError::InvalidPayload(format!("relayIdHex is not a valid Ed25519 key: {error}"))
    })?;
    let _ = parse_canonical_nonzero_hex_32(
        payload.descriptor_commit_hex.as_str(),
        "descriptorCommitHex",
    )?;
    let _ = parse_canonical_nonzero_hex_32(
        payload.relay_tls_spki_sha256_hex.as_str(),
        "relayTlsSpkiSha256Hex",
    )?;
    let _ = parse_canonical_nonzero_hex_32(
        payload.relay_certificate_sha256_hex.as_str(),
        "relayCertificateSha256Hex",
    )?;
    let _ = parse_canonical_nonzero_hex_32(
        payload.directory_snapshot_digest_hex.as_str(),
        "directorySnapshotDigestHex",
    )?;
    let ticket = decode_helper_ticket_metadata(payload.helper_ticket_hex.as_str())?;
    if ticket.relay_id != relay_id {
        return Err(ControllerError::InvalidPayload(
            "helper ticket relay identity does not match relayIdHex".to_owned(),
        ));
    }
    if ticket.session_id != relay_session_id_from_session_id(payload.session_id.as_str()) {
        return Err(ControllerError::InvalidPayload(
            "helper ticket session id does not match sessionId".to_owned(),
        ));
    }
    if ticket.expires_at_ms <= unix_now_ms()? {
        return Err(ControllerError::InvalidPayload(
            "helper ticket has expired".to_owned(),
        ));
    }
    if payload.padding_budget_ms == 0 {
        return Err(ControllerError::InvalidPayload(
            "paddingBudgetMs must be greater than zero".to_owned(),
        ));
    }
    if payload.mtu_bytes == 0 {
        return Err(ControllerError::InvalidPayload(
            "mtuBytes must be greater than zero".to_owned(),
        ));
    }
    Ok(())
}
fn validate_text_field(value: &str, label: &str, max_bytes: usize) -> Result<(), ControllerError> {
    if value.len() > max_bytes {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} exceeds the v1 limit of {max_bytes} bytes"
        )));
    }
    Ok(())
}
fn validate_network_policy_entries(entries: &[String], label: &str) -> Result<(), ControllerError> {
    if entries.len() > MAX_NETWORK_POLICY_ENTRIES_V1 {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} exceeds the v1 limit of {MAX_NETWORK_POLICY_ENTRIES_V1} entries"
        )));
    }
    for (index, entry) in entries.iter().enumerate() {
        if entry.len() > MAX_NETWORK_POLICY_ENTRY_BYTES_V1 {
            return Err(ControllerError::InvalidPayload(format!(
                "{label}[{index}] exceeds the v1 limit of {MAX_NETWORK_POLICY_ENTRY_BYTES_V1} bytes"
            )));
        }
    }
    Ok(())
}
fn validate_canonical_cidr_entries(entries: &[String], label: &str) -> Result<(), ControllerError> {
    for (index, entry) in entries.iter().enumerate() {
        let parsed = parse_cidr(entry).map_err(|_| {
            ControllerError::InvalidPayload(format!("{label}[{index}] is not a valid CIDR"))
        })?;
        let canonical = format!("{}/{}", parsed.address, parsed.prefix);
        if entry != &canonical {
            return Err(ControllerError::InvalidPayload(format!(
                "{label}[{index}] must use canonical CIDR syntax {canonical}"
            )));
        }
    }
    Ok(())
}
fn validate_dns_servers(entries: &[String]) -> Result<(), ControllerError> {
    for (index, entry) in entries.iter().enumerate() {
        let address = entry.parse::<IpAddr>().map_err(|_| {
            ControllerError::InvalidPayload(format!(
                "dnsServers[{index}] must be a canonical IP address"
            ))
        })?;
        let canonical = address.to_string();
        if entry != &canonical || address.is_unspecified() || address.is_multicast() {
            return Err(ControllerError::InvalidPayload(format!(
                "dnsServers[{index}] must be a canonical unicast IP address"
            )));
        }
    }
    Ok(())
}
fn read_connect_payload_from_stdin() -> Result<ConnectPayload, ControllerError> {
    let mut stdin = io::stdin().lock();
    let raw_payload = read_sensitive_bounded(
        &mut stdin,
        MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1,
        "worker stdin connect frame",
    )?;
    decode_connect_payload_frame(&raw_payload)
}
fn read_connect_payload_json_from_stdin() -> Result<WipeBytes, ControllerError> {
    let mut stdin = io::stdin().lock();
    let raw_payload = read_sensitive_bounded(
        &mut stdin,
        MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1,
        "connect payload stdin",
    )?;
    if raw_payload.is_empty() {
        return Err(ControllerError::MissingPayload);
    }
    Ok(raw_payload)
}
fn json_field<'a>(object: &'a JsonMap, keys: &[&str]) -> Option<&'a JsonValue> {
    keys.iter().find_map(|key| object.get(*key))
}
fn require_json_string(
    object: &JsonMap,
    keys: &[&str],
    label: &str,
) -> Result<String, ControllerError> {
    optional_json_string(object, keys)?.ok_or_else(|| {
        ControllerError::InvalidPayload(format!("{label} must be a string and must be present"))
    })
}
fn optional_json_string(
    object: &JsonMap,
    keys: &[&str],
) -> Result<Option<String>, ControllerError> {
    let Some(value) = json_field(object, keys) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(|value| Some(value.to_owned()))
        .ok_or_else(|| ControllerError::InvalidPayload(format!("{} must be a string", keys[0])))
}
fn require_json_u16(object: &JsonMap, keys: &[&str], label: &str) -> Result<u16, ControllerError> {
    let value = require_json_u64(object, keys, label)?;
    u16::try_from(value)
        .map_err(|_| ControllerError::InvalidPayload(format!("{label} must fit into a u16")))
}
fn require_json_u64(object: &JsonMap, keys: &[&str], label: &str) -> Result<u64, ControllerError> {
    optional_json_u64(object, keys)?.ok_or_else(|| {
        ControllerError::InvalidPayload(format!("{label} must be an unsigned integer and present"))
    })
}
fn optional_json_u64(object: &JsonMap, keys: &[&str]) -> Result<Option<u64>, ControllerError> {
    let Some(value) = json_field(object, keys) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    if let Some(value) = value.as_u64() {
        return Ok(Some(value));
    }
    if let Some(raw) = value.as_str() {
        return raw.parse::<u64>().map(Some).map_err(|error| {
            ControllerError::InvalidPayload(format!(
                "{} must be an unsigned integer: {error}",
                keys[0]
            ))
        });
    }
    Err(ControllerError::InvalidPayload(format!(
        "{} must be an unsigned integer",
        keys[0]
    )))
}
fn optional_json_string_array(
    object: &JsonMap,
    keys: &[&str],
) -> Result<Vec<String>, ControllerError> {
    let Some(value) = json_field(object, keys) else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(values) = value.as_array() else {
        return Err(ControllerError::InvalidPayload(format!(
            "{} must be an array of strings",
            keys[0]
        )));
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                ControllerError::InvalidPayload(format!("{}[{index}] must be a string", keys[0]))
            })
        })
        .collect()
}
fn parse_multiaddr(addr: &str) -> Result<ParsedMultiaddr, ControllerError> {
    validate_quic_multiaddr(addr)
        .map_err(|error| ControllerError::InvalidMultiaddr(error.to_string()))?;
    let mut parts = addr.trim_start_matches('/').split('/');
    let proto = parts
        .next()
        .ok_or_else(|| ControllerError::InvalidMultiaddr(addr.to_owned()))?;
    let host = match proto {
        "ip4" => {
            let raw = parts
                .next()
                .ok_or_else(|| ControllerError::InvalidMultiaddr(addr.to_owned()))?;
            ParsedMultiaddrHost::Ip(IpAddr::V4(
                raw.parse::<Ipv4Addr>()
                    .map_err(|_| ControllerError::InvalidMultiaddr(addr.to_owned()))?,
            ))
        }
        "ip6" => {
            let raw = parts
                .next()
                .ok_or_else(|| ControllerError::InvalidMultiaddr(addr.to_owned()))?;
            ParsedMultiaddrHost::Ip(IpAddr::V6(
                raw.parse::<Ipv6Addr>()
                    .map_err(|_| ControllerError::InvalidMultiaddr(addr.to_owned()))?,
            ))
        }
        "dns" | "dns4" | "dns6" => {
            let name = parts
                .next()
                .ok_or_else(|| ControllerError::InvalidMultiaddr(addr.to_owned()))?
                .to_owned();
            let address_family = match proto {
                "dns" => DnsAddressFamily::Any,
                "dns4" => DnsAddressFamily::V4,
                "dns6" => DnsAddressFamily::V6,
                _ => unreachable!("matched DNS protocol"),
            };
            ParsedMultiaddrHost::Dns {
                name,
                address_family,
            }
        }
        other => return Err(ControllerError::InvalidMultiaddr(other.to_owned())),
    };
    let transport = parts
        .next()
        .ok_or_else(|| ControllerError::InvalidMultiaddr(addr.to_owned()))?;
    if transport != "udp" {
        return Err(ControllerError::InvalidMultiaddr(format!(
            "unsupported transport {transport}"
        )));
    }
    let port = parts
        .next()
        .ok_or_else(|| ControllerError::InvalidMultiaddr(addr.to_owned()))?
        .parse::<u16>()
        .map_err(|_| ControllerError::InvalidMultiaddr(addr.to_owned()))?;
    match parts.next() {
        Some("quic") => {}
        None => return Err(ControllerError::InvalidMultiaddr(addr.to_owned())),
        Some(other) => {
            return Err(ControllerError::InvalidMultiaddr(format!(
                "unsupported protocol {other}"
            )));
        }
    }
    if let Some(extra) = parts.next() {
        return Err(ControllerError::InvalidMultiaddr(format!(
            "unexpected trailing segment {extra}"
        )));
    }
    Ok(ParsedMultiaddr { host, port })
}
#[derive(Debug)]
struct PinnedSpkiVerifier {
    relay_tls_spki_sha256: [u8; 32],
}
impl ServerCertVerifier for PinnedSpkiVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &rustls::pki_types::ServerName<'_>,
        ocsp_response: &[u8],
        now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let digest = leaf_certificate_spki_sha256(end_entity.as_ref()).map_err(|error| {
            rustls::Error::General(format!("invalid relay leaf certificate: {error}"))
        })?;
        if digest != self.relay_tls_spki_sha256 {
            return Err(rustls::Error::General(
                "relay TLS SPKI pin mismatch".to_owned(),
            ));
        }
        verifier_for_signature_cert(end_entity)?.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verifier_for_signature_cert(cert)?.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verifier_for_signature_cert(cert)?.verify_tls13_signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
        ]
    }
}
fn verifier_for_signature_cert(
    cert: &CertificateDer<'_>,
) -> Result<Arc<WebPkiServerVerifier>, rustls::Error> {
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from(cert.as_ref().to_vec()))?;
    WebPkiServerVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|error| rustls::Error::General(format!("TLS verifier config error: {error}")))
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        prelude::{Numeric, Quantity},
        soranet::vpn::{VPN_HELPER_TICKET_MAGIC, VpnTariffV1},
    };
    const HELPER_TICKET_METERING_PUBLIC_KEY_OFFSET: usize =
        VPN_HELPER_TICKET_MAGIC.len() + 16 + 32 + 32 + 32 + 32;
    const SMALL_ORDER_ED25519_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    fn quantity_nanos(value: u64) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, 9))
            .expect("u64 nano-XOR test value fits Quantity")
    }
    fn test_relay_id() -> [u8; 32] {
        let keys = KeyPair::try_from_seed(vec![0x44; 32], Algorithm::Ed25519)
            .expect("derive relay fixture key");
        let (algorithm, bytes) = keys.public_key().try_to_bytes().expect("relay fixture key");
        assert_eq!(algorithm, Algorithm::Ed25519);
        bytes
            .try_into()
            .expect("Ed25519 relay identity is 32 bytes")
    }
    fn test_helper_ticket(session_id: &str) -> VpnHelperTicketV1 {
        let metering_keys = KeyPair::try_from_seed(vec![0x66; 32], Algorithm::Ed25519)
            .expect("derive metering fixture key");
        VpnHelperTicketV1 {
            session_id: relay_session_id_from_session_id(session_id),
            quote_id: [0x22; 32],
            account_hash: [0x33; 32],
            relay_id: test_relay_id(),
            payment_tx_hash: [0x55; 32],
            metering_public_key: metering_keys.public_key().clone(),
            tariff: VpnTariffV1 {
                lease_fee: quantity_nanos(1_000),
                active_fee_per_minute: quantity_nanos(100),
                ingress_fee_per_mib: quantity_nanos(7),
                egress_fee_per_mib: quantity_nanos(11),
            },
            expires_at_ms: unix_now_ms()
                .expect("valid test clock")
                .saturating_add(60_000),
        }
    }
    fn test_connect_payload_json(
        session_id: &str,
        ticket: &VpnHelperTicketV1,
        metering_private_key_seed_hex: Option<&str>,
    ) -> String {
        let metering_seed = metering_private_key_seed_hex.map_or_else(String::new, |seed| {
            format!(r#","meteringPrivateKeySeedHex":"{seed}""#)
        });
        format!(
            r#"{{"sessionId":"{session_id}","relayEndpoint":"/ip4/127.0.0.1/udp/7777/quic","exitClass":"standard","helperTicketHex":"{}","relayIdHex":"{}","descriptorCommitHex":"{}","tlsServerName":"relay.example","relayTlsSpkiSha256Hex":"{}","relayCertificateSha256Hex":"{}","directorySnapshotDigestHex":"{}","paddingBudgetMs":15,"routePushes":[],"excludedRoutes":[],"dnsServers":[],"tunnelAddresses":["10.208.0.2/32"],"mtuBytes":1280,"leaseSecs":600,"usageVoucherIntervalMs":1{metering_seed}}}"#,
            ticket.to_hex(&[0xAA; 32]),
            hex::encode(ticket.relay_id),
            "cd".repeat(32),
            "ab".repeat(32),
            "ef".repeat(32),
            "42".repeat(32),
        )
    }
    fn test_connect_payload(session_id: &str) -> ConnectPayload {
        let ticket = test_helper_ticket(session_id);
        let json = test_connect_payload_json(session_id, &ticket, None);
        parse_connect_payload(Some(&json)).expect("valid connect payload fixture")
    }
    #[test]
    fn parse_multiaddr_accepts_ipv4_quic() {
        let parsed = parse_multiaddr("/ip4/127.0.0.1/udp/7777/quic").expect("parse");
        assert_eq!(
            parsed,
            ParsedMultiaddr {
                host: ParsedMultiaddrHost::Ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                port: 7777,
            }
        );
    }
    #[test]
    fn parse_multiaddr_accepts_ipv6_quic() {
        let parsed = parse_multiaddr("/ip6/::1/udp/7777/quic").expect("parse");
        assert_eq!(
            parsed,
            ParsedMultiaddr {
                host: ParsedMultiaddrHost::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST)),
                port: 7777,
            }
        );
    }
    #[test]
    fn parse_multiaddr_accepts_dns_quic() {
        let parsed = parse_multiaddr("/dns/torii/udp/9443/quic").expect("parse");
        assert_eq!(
            parsed,
            ParsedMultiaddr {
                host: ParsedMultiaddrHost::Dns {
                    name: "torii".to_owned(),
                    address_family: DnsAddressFamily::Any,
                },
                port: 9443,
            }
        );
    }
    #[test]
    fn parse_multiaddr_preserves_dns_address_family() {
        for (protocol, expected) in [
            ("dns4", DnsAddressFamily::V4),
            ("dns6", DnsAddressFamily::V6),
        ] {
            let parsed = parse_multiaddr(&format!("/{protocol}/torii/udp/9443/quic"))
                .expect("parse family-specific DNS endpoint");
            assert_eq!(
                parsed.host,
                ParsedMultiaddrHost::Dns {
                    name: "torii".to_owned(),
                    address_family: expected,
                }
            );
        }
    }
    #[test]
    fn parse_multiaddr_rejects_non_udp_transport() {
        let err = parse_multiaddr("/ip4/127.0.0.1/tcp/7777/quic").expect_err("must fail");
        assert!(err.to_string().contains("transport"));
    }
    #[test]
    fn connect_payload_deserializes_camel_case() {
        let payload = test_connect_payload("session-1");
        assert_eq!("session-1", payload.session_id);
        assert_eq!("/ip4/127.0.0.1/udp/7777/quic", payload.relay_endpoint);
        assert_eq!(1280, payload.mtu_bytes);
        assert_eq!(15, payload.padding_budget_ms);
    }
    #[test]
    fn connect_payload_worker_frame_roundtrips_as_norito() {
        let payload = test_connect_payload("session-1");
        let frame = encode_connect_payload_frame(&payload).expect("encode frame");
        assert!(frame.starts_with(CONNECT_PAYLOAD_FRAME_MAGIC));
        assert_eq!(
            decode_connect_payload_frame(&frame).expect("decode frame"),
            payload
        );
    }
    #[test]
    fn state_frame_roundtrips_as_norito() {
        let state = State {
            active: true,
            session_id: Some("session-1".to_owned()),
            relay_endpoint: Some("/ip4/127.0.0.1/udp/7777/quic".to_owned()),
            bytes_in: 7,
            bytes_out: 9,
            ..State::default()
        };
        let frame = encode_state_frame(&state).expect("encode state");
        assert!(frame.starts_with(STATE_FILE_FRAME_MAGIC));
        assert_eq!(decode_state_frame(&frame).expect("decode state"), state);
    }
    #[test]
    fn bounded_reader_accepts_exact_connect_frame_limit() {
        let input = vec![0xA5; MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1];
        let mut reader = input.as_slice();
        let read = read_bounded(
            &mut reader,
            MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1,
            "test connect frame",
        )
        .expect("exact limit is accepted");
        assert_eq!(read.len(), MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1);
        assert_eq!(read.first(), Some(&0xA5));
        assert_eq!(read.last(), Some(&0xA5));
    }
    #[test]
    fn bounded_reader_rejects_connect_frame_limit_plus_one() {
        let input = vec![0xA5; MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1 + 1];
        let mut reader = input.as_slice();
        let error = read_bounded(
            &mut reader,
            MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1,
            "test connect frame",
        )
        .expect_err("limit plus one must be rejected");
        assert!(error.to_string().contains("exceeds the v1 limit"));
    }
    #[test]
    fn sensitive_bounded_reader_owns_exact_input_and_rejects_growth() {
        let mut exact = b"secret".as_slice();
        let read = read_sensitive_bounded(&mut exact, 6, "test secret").expect("exact limit");
        assert_eq!(&*read, b"secret");

        let mut oversized = b"secret!".as_slice();
        let error = match read_sensitive_bounded(&mut oversized, 6, "test secret") {
            Ok(_) => panic!("growth beyond the secret bound must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("exceeds the v1 limit"));
    }
    #[test]
    fn command_pipe_drain_discards_beyond_the_retention_limit() {
        let exact = drain_bounded_pipe(io::Cursor::new(b"abc"), 3).expect("exact drain");
        assert_eq!(
            exact,
            BoundedPipeOutput {
                bytes: b"abc".to_vec(),
                overflow: false,
            }
        );
        let overflow = drain_bounded_pipe(io::Cursor::new(b"abcdef"), 3).expect("overflow drain");
        assert_eq!(overflow.bytes, b"abc");
        assert!(overflow.overflow);
    }
    #[test]
    fn command_deadline_terminates_descendants_holding_output_pipes() {
        let shell = Path::new("/bin/sh");
        if !shell.exists() {
            return;
        }
        let started = Instant::now();
        let error = execute_system_command(
            "sh",
            shell,
            &["-c".to_owned(), "sleep 30 & wait".to_owned()],
            Duration::from_millis(25),
        )
        .expect_err("timed-out command group must fail");
        assert!(error.to_string().contains("deadline"));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "descendant inherited a pipe after command-group cleanup"
        );
    }
    #[tokio::test]
    async fn tunnel_shutdown_handlers_install_before_network_setup() {
        let signals = TunnelShutdownSignals::install().expect("install Unix signal handlers");
        drop(signals);
    }
    #[test]
    fn connect_payload_rejects_network_policy_count_before_encoding() {
        let mut payload = test_connect_payload("session-1");
        payload.route_pushes = vec!["10.0.0.0/8".to_owned(); MAX_NETWORK_POLICY_ENTRIES_V1 + 1];
        let error = encode_connect_payload_frame(&payload)
            .expect_err("producer must enforce the route-count limit");
        assert!(error.to_string().contains("routePushes"));
        assert!(error.to_string().contains("4096 entries"));
    }
    #[test]
    fn decode_hex_accepts_prefixed_values() {
        let decoded = decode_hex("0x0A0b").expect("hex");
        assert_eq!(decoded, vec![0x0A, 0x0B]);
    }
    #[test]
    fn helper_ticket_handshake_binding_is_nonzero_and_credential_bound() {
        let payload = test_connect_payload("session-1");
        let first = helper_ticket_handshake_binding(&payload, b"first helper ticket")
            .expect("first binding");
        let second = helper_ticket_handshake_binding(&payload, b"second helper ticket")
            .expect("second binding");
        assert_ne!(first, [0; 32]);
        assert_ne!(first, second);
        assert_eq!(
            first,
            helper_ticket_handshake_binding(&payload, b"first helper ticket")
                .expect("repeat binding")
        );
        let mut different_trust = payload;
        different_trust.descriptor_commit_hex = "ce".repeat(32);
        assert_ne!(
            first,
            helper_ticket_handshake_binding(&different_trust, b"first helper ticket")
                .expect("trust-bound binding")
        );
    }
    #[test]
    fn connect_payload_rejects_noncanonical_trust_hex() {
        let mut payload = test_connect_payload("session-1");
        payload.relay_tls_spki_sha256_hex.make_ascii_uppercase();
        let error = validate_connect_payload(payload).expect_err("uppercase pin must fail");
        assert!(
            error
                .to_string()
                .contains("exactly 64 lowercase hexadecimal characters")
        );
    }
    #[test]
    fn connect_payload_rejects_dns_directive_injection() {
        let mut payload = test_connect_payload("session-1");
        payload.dns_servers = vec!["1.1.1.1\noptions trust-ad".to_owned()];
        let error = validate_connect_payload(payload).expect_err("DNS directives must fail");
        assert!(error.to_string().contains("canonical IP address"));
    }
    #[test]
    fn connect_payload_requires_canonical_network_policy() {
        let mut payload = test_connect_payload("session-1");
        payload.route_pushes = vec!["2001:0db8::/64".to_owned()];
        let error = validate_connect_payload(payload).expect_err("non-canonical CIDR must fail");
        assert!(error.to_string().contains("canonical CIDR syntax"));
    }
    #[test]
    fn connect_payload_credentials_can_be_wiped_early() {
        let mut payload = test_connect_payload("session-1");
        payload.metering_private_key_seed_hex = Some("66".repeat(32));
        payload.wipe_credentials();
        assert!(payload.helper_ticket_hex.is_empty());
        assert!(payload.metering_private_key_seed_hex.is_none());
    }
    #[test]
    fn vpn_quic_disables_tls_early_data() {
        let tls_config = build_tls_client_config([0x55; 32]);
        assert!(
            !tls_config.enable_early_data,
            "helper authentication must complete before application data"
        );
    }
    #[test]
    fn helper_ticket_metadata_decodes_without_secret() {
        let metering_keys = KeyPair::try_from_seed(vec![0x66; 32], Algorithm::Ed25519)
            .expect("derive metering fixture key");
        let tariff = VpnTariffV1 {
            lease_fee: quantity_nanos(1_000),
            active_fee_per_minute: quantity_nanos(100),
            ingress_fee_per_mib: quantity_nanos(7),
            egress_fee_per_mib: quantity_nanos(11),
        };
        let ticket = VpnHelperTicketV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            account_hash: [0x33; 32],
            relay_id: [0x44; 32],
            payment_tx_hash: [0x55; 32],
            metering_public_key: metering_keys.public_key().clone(),
            tariff,
            expires_at_ms: 99_000,
        };
        let encoded = ticket.to_hex(&[0xAA; 32]);
        let decoded = decode_helper_ticket_metadata(&encoded).expect("ticket metadata");
        assert_eq!(decoded, ticket);
    }
    #[test]
    fn helper_ticket_metadata_rejects_inert_metering_public_key_material() {
        let metering_keys = KeyPair::try_from_seed(vec![0x66; 32], Algorithm::Ed25519)
            .expect("derive metering fixture key");
        let ticket = VpnHelperTicketV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            account_hash: [0x33; 32],
            relay_id: [0x44; 32],
            payment_tx_hash: [0x55; 32],
            metering_public_key: metering_keys.public_key().clone(),
            tariff: VpnTariffV1 {
                lease_fee: quantity_nanos(1_000),
                active_fee_per_minute: quantity_nanos(100),
                ingress_fee_per_mib: quantity_nanos(7),
                egress_fee_per_mib: quantity_nanos(11),
            },
            expires_at_ms: 99_000,
        };
        for (label, public_key) in [
            ("all-zero", [0u8; 32]),
            ("small-order", SMALL_ORDER_ED25519_POINT),
        ] {
            let mut bytes = ticket.to_bytes(&[0xAA; 32]);
            bytes[HELPER_TICKET_METERING_PUBLIC_KEY_OFFSET
                ..HELPER_TICKET_METERING_PUBLIC_KEY_OFFSET + 32]
                .copy_from_slice(&public_key);
            let encoded = hex::encode(bytes);
            match decode_helper_ticket_metadata(&encoded) {
                Err(ControllerError::InvalidPayload(message)) => {
                    assert!(
                        message.contains("metering public key is invalid"),
                        "unexpected {label} ticket metadata error: {message}"
                    );
                }
                other => panic!("expected invalid {label} ticket metadata, got {other:?}"),
            }
        }
    }
    #[test]
    fn usage_voucher_signer_builds_signed_cumulative_voucher() {
        let session_id = "f69c894aa32726fe586fab520f88ae42d1fbb4ebf3083df057f4e40ca0a11111";
        let tariff = VpnTariffV1 {
            lease_fee: quantity_nanos(1_000),
            active_fee_per_minute: quantity_nanos(6_000),
            ingress_fee_per_mib: quantity_nanos(3),
            egress_fee_per_mib: quantity_nanos(5),
        };
        let mut ticket = test_helper_ticket(session_id);
        ticket.tariff = tariff;
        let metering_seed = "66".repeat(32);
        let raw_payload =
            test_connect_payload_json(session_id, &ticket, Some(metering_seed.as_str()));
        let payload = parse_connect_payload(Some(&raw_payload)).expect("payload");
        let mut signer = UsageVoucherSigner::from_payload(&payload)
            .expect("signer")
            .expect("enabled signer");
        let counters = UsageVoucherCounters::default();
        counters.add_ingress(10);
        counters.add_egress(20);
        let envelope = signer
            .build_envelope(&counters)
            .expect("usage voucher should sign");
        envelope.voucher.verify().expect("voucher signature");
        assert_eq!(envelope.voucher.body.session_id, ticket.session_id);
        assert_eq!(envelope.voucher.body.quote_id, ticket.quote_id);
        assert_eq!(envelope.voucher.body.relay_id, ticket.relay_id);
        assert_eq!(envelope.voucher.body.ingress_bytes, 10);
        assert_eq!(envelope.voucher.body.egress_bytes, 20);
        assert_eq!(
            envelope.earned_fee,
            ticket
                .tariff
                .earned_fee(&envelope.voucher.body)
                .expect("bounded fixture fee")
        );
    }
    #[test]
    fn usage_voucher_signer_rejects_wrong_metering_seed() {
        let session_id = "f69c894aa32726fe586fab520f88ae42d1fbb4ebf3083df057f4e40ca0a11111";
        let ticket = test_helper_ticket(session_id);
        let wrong_metering_seed = "77".repeat(32);
        let raw_payload =
            test_connect_payload_json(session_id, &ticket, Some(wrong_metering_seed.as_str()));
        let payload = parse_connect_payload(Some(&raw_payload)).expect("payload");
        let error = match UsageVoucherSigner::from_payload(&payload) {
            Ok(_) => panic!("wrong seed must fail"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("metering private key does not match helper ticket public key")
        );
    }
    #[test]
    fn parse_cidr_accepts_ipv4() {
        let parsed = parse_cidr("10.208.0.2/32").expect("cidr");
        assert_eq!(
            parsed,
            ParsedCidr {
                address: IpAddr::V4(Ipv4Addr::new(10, 208, 0, 2)),
                prefix: 32,
            }
        );
    }
    #[test]
    fn parse_cidr_accepts_ipv6() {
        let parsed = parse_cidr("2001:db8::2/64").expect("cidr");
        assert_eq!(
            parsed,
            ParsedCidr {
                address: IpAddr::V6("2001:db8::2".parse::<Ipv6Addr>().expect("ipv6")),
                prefix: 64,
            }
        );
    }
    #[test]
    fn parse_cidr_rejects_out_of_range_prefix() {
        let error = parse_cidr("10.0.0.1/40").expect_err("must fail");
        assert!(error.to_string().contains("invalid cidr"));
    }
    #[test]
    fn tunnel_owned_addresses_and_routes_never_replace_host_policy() {
        let address = parse_cidr("10.208.0.2/32").expect("address");
        let route = parse_cidr("10.20.0.0/16").expect("route");
        let address_args = tunnel_address_add_args("srvpn0123456789", address);
        let route_args = tunnel_route_add_args("srvpn0123456789", route);
        assert_eq!(address_args[1..3], ["address", "add"]);
        assert_eq!(route_args[1..3], ["route", "add"]);
        assert!(!address_args.iter().any(|arg| arg == "replace"));
        assert!(!route_args.iter().any(|arg| arg == "replace"));
    }
    #[test]
    fn packet_stream_round_trips_fragmented_payload() {
        let packet = vec![0xAB; 1500];
        let encoded = encode_packet_stream_frame(&packet).expect("encode");
        let mut decoder = PacketStreamDecoder::new(1500).expect("bounded decoder");
        let first = decoder.ingest(&encoded[..700]).expect("first fragment");
        assert!(first.is_empty());
        let second = decoder.ingest(&encoded[700..]).expect("second fragment");
        assert_eq!(second, vec![packet]);
    }
    #[test]
    fn decoder_handles_multiple_packets_in_single_chunk() {
        let first = encode_packet_stream_frame(&[1, 2, 3]).expect("first");
        let second = encode_packet_stream_frame(&[4, 5]).expect("second");
        let mut decoder = PacketStreamDecoder::new(1280).expect("bounded decoder");
        let packets = decoder
            .ingest(&[first.as_slice(), second.as_slice()].concat())
            .expect("decode");
        assert_eq!(packets, vec![vec![1, 2, 3], vec![4, 5]]);
    }
    #[test]
    fn packet_stream_rejects_zero_and_over_mtu_lengths_before_buffering() {
        let mut zero = PacketStreamDecoder::new(1280).expect("bounded decoder");
        let error = zero
            .ingest(&0_u16.to_be_bytes())
            .expect_err("zero-length packets must fail closed");
        assert!(error.to_string().contains("outside 1..=1280"));

        let mut oversized = PacketStreamDecoder::new(1280).expect("bounded decoder");
        let error = oversized
            .ingest(&1281_u16.to_be_bytes())
            .expect_err("over-MTU packet must fail before payload buffering");
        assert!(error.to_string().contains("outside 1..=1280"));
        assert_eq!(oversized.buffer.len(), PACKET_LEN_PREFIX_BYTES);
        assert!(oversized.expected_len.is_none());
    }
    #[test]
    fn parse_route_via_dev_extracts_gateway_and_device() {
        let parsed =
            parse_route_via_dev("default via 192.168.1.1 dev enp0s31f6 proto dhcp metric 100");
        assert_eq!(
            parsed,
            (Some("192.168.1.1".to_owned()), Some("enp0s31f6".to_owned()))
        );
    }
    #[test]
    fn desired_interface_name_is_derived_from_the_authenticated_session() {
        let derived = desired_interface_name("deadbeef").expect("name");
        assert!(derived.starts_with("srvpn"));
        assert_eq!(derived.len(), 15);
        assert_ne!(
            derived,
            desired_interface_name("cafebabe").expect("other session")
        );
    }
    #[test]
    fn tun_creation_is_exclusive_and_pins_the_kernel_returned_name() {
        let flags = linux_tun_creation_flag_bits();
        assert_ne!(flags & LINUX_IFF_TUN_EXCL_BITS, 0);
        ensure_exact_tun_interface_name("srvpn0123456789", "srvpn0123456789")
            .expect("exact kernel name");
        let error = ensure_exact_tun_interface_name("srvpn0123456789", "srvpn9876543210")
            .expect_err("renamed or pre-existing interface must fail closed");
        assert!(error.to_string().contains("instead of requested"));
    }
    #[test]
    fn relay_session_id_matches_torii_derivation() {
        let session_id = "f69c894aa32726fe586fab520f88ae42d1fbb4ebf3083df057f4e40ca0a11111";
        let derived = relay_session_id_from_session_id(session_id);
        assert_eq!(derived.len(), 16);
        assert_ne!(derived, [0u8; 16]);
    }
    #[test]
    fn wall_clock_before_unix_epoch_fails_without_panicking() {
        let before_epoch = UNIX_EPOCH
            .checked_sub(Duration::from_millis(1))
            .expect("representable pre-epoch time");
        let error = unix_time_ms_at(before_epoch).expect_err("clock rollback must fail closed");
        assert!(error.to_string().contains("before the Unix epoch"));
    }
    #[test]
    fn cli_requires_connect_payload_on_stdin() {
        let payload = r#"{"sessionId":"session-1","relayEndpoint":"/ip4/127.0.0.1/udp/7777/quic","exitClass":"standard","helperTicketHex":"aa","relayTlsSpkiSha256Hex":"abababababababababababababababababababababababababababababababab","paddingBudgetMs":15,"routePushes":[],"excludedRoutes":[],"dnsServers":[],"tunnelAddresses":["10.208.0.2/32"],"mtuBytes":1280}"#;
        Cli::try_parse_from(["sora-vpn-controller", "connect", payload])
            .expect_err("connect secrets must not be accepted through argv");
        let cli = Cli::try_parse_from(["sora-vpn-controller", "connect"]).expect("parse");
        assert!(matches!(cli.command, Command::Connect));
    }
    #[cfg(unix)]
    fn private_test_state_root(label: &str) -> PathBuf {
        let nonce = ATOMIC_FILE_NONCE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "sora-vpn-helper-{label}-{}-{nonce}",
            std::process::id()
        ));
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&path).expect("create private test root");
        path
    }
    #[test]
    #[cfg(unix)]
    fn controller_actions_are_serialized_by_a_stable_lock() {
        let root = private_test_state_root("controller-lock");
        let first = acquire_controller_action_lock_at(&root).expect("acquire first lock");
        let error = acquire_controller_action_lock_at(&root)
            .err()
            .expect("concurrent action must fail");
        assert!(error.to_string().contains("already in progress"));
        drop(first);
        drop(acquire_controller_action_lock_at(&root).expect("reacquire released lock"));
        fs::remove_file(root.join(CONTROLLER_LOCK_FILE_NAME)).expect("remove lock file");
        fs::remove_dir(root).expect("remove test root");
    }
    #[test]
    #[cfg(unix)]
    fn state_persistence_is_private_atomic_and_round_trips() {
        let root = private_test_state_root("roundtrip");
        let path = root.join(STATE_FILE_NAME);
        let mut state = State {
            message: "first".to_owned(),
            ..State::default()
        };
        persist_state_at(&path, &state).expect("persist first state");
        state.message = "second".to_owned();
        persist_state_at(&path, &state).expect("replace state");
        let loaded = load_state_at(&path).expect("load state");
        assert_eq!(loaded.message, "second");
        assert_eq!(
            fs::symlink_metadata(&path).expect("metadata").mode() & 0o077,
            0
        );
        fs::remove_file(path).expect("remove state");
        fs::remove_dir(root).expect("remove test root");
    }
    #[test]
    #[cfg(unix)]
    fn malformed_existing_state_fails_closed() {
        let root = private_test_state_root("malformed");
        let path = root.join(STATE_FILE_NAME);
        write_file_atomic(&path, b"not a state frame", 0o600, true, "state file")
            .expect("write malformed fixture");
        let error = load_state_at(&path).expect_err("malformed state must not become defaults");
        assert!(error.to_string().contains("not a v1 Norito state frame"));
        fs::remove_file(path).expect("remove state");
        fs::remove_dir(root).expect("remove test root");
    }
    #[test]
    #[cfg(unix)]
    fn state_reads_and_writes_reject_symlinks() {
        use std::os::unix::fs::symlink;

        let root = private_test_state_root("symlink");
        let target = root.join("target");
        write_file_atomic(&target, b"do not replace", 0o600, true, "test target")
            .expect("write target");
        let path = root.join(STATE_FILE_NAME);
        symlink(&target, &path).expect("create state symlink");
        assert!(load_state_at(&path).is_err());
        assert!(persist_state_at(&path, &State::default()).is_err());
        assert_eq!(
            fs::read(&target).expect("target contents"),
            b"do not replace"
        );
        fs::remove_file(path).expect("remove symlink");
        fs::remove_file(target).expect("remove target");
        fs::remove_dir(root).expect("remove test root");
    }
    #[test]
    #[cfg(unix)]
    fn state_rejects_public_permissions_and_writable_root() {
        let root = private_test_state_root("permissions");
        let path = root.join(STATE_FILE_NAME);
        persist_state_at(&path, &State::default()).expect("persist state");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("relax state mode");
        assert!(load_state_at(&path).is_err());
        fs::remove_file(&path).expect("remove state");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o770)).expect("relax root mode");
        assert!(prepare_private_state_root(&root).is_err());
        fs::remove_dir(root).expect("remove test root");
    }
    #[test]
    fn terminal_state_retains_network_snapshot_until_repair_succeeds() {
        let applied = AppliedNetworkState {
            interface_name: "srvpn0000000000".to_owned(),
            dns_backend: None,
            excluded_route_snapshots: Vec::new(),
        };
        let mut state = State {
            network_service: Some("resolvectl".to_owned()),
            applied_network: Some(applied.clone()),
            ..State::default()
        };
        apply_terminal_network_lifecycle(&mut state, false, true);
        assert_eq!(state.applied_network, Some(applied));
        assert_eq!(state.network_service.as_deref(), Some("resolvectl"));
        apply_terminal_network_lifecycle(&mut state, false, false);
        assert!(state.applied_network.is_none());
        assert!(state.network_service.is_none());
    }
    #[test]
    fn state_rejects_active_or_invalid_unbound_worker_identity() {
        let mut state = State {
            active: true,
            ..State::default()
        };
        assert!(validate_state_invariants(&state).is_err());
        state.worker_identity = Some(WorkerProcessIdentity {
            pid: 0,
            start_time_ticks: 10,
            executable_device: 11,
            executable_inode: 12,
            role: WorkerRole::Tunnel,
        });
        assert!(validate_state_invariants(&state).is_err());
        state.worker_identity.as_mut().expect("identity").pid = 42;
        assert!(validate_state_invariants(&state).is_ok());
    }
    #[test]
    fn worker_start_requires_the_exact_controller_persisted_identity_and_session() {
        let payload = test_connect_payload("session-1");
        let identity = WorkerProcessIdentity {
            pid: 42,
            start_time_ticks: 10,
            executable_device: 11,
            executable_inode: 12,
            role: WorkerRole::Tunnel,
        };
        let mut state = State {
            message: "starting".to_owned(),
            worker_identity: Some(identity.clone()),
            session_id: Some(payload.session_id.clone()),
            relay_endpoint: Some(payload.relay_endpoint.clone()),
            ..State::default()
        };
        authorize_worker_start(&state, &identity, &payload).expect("exact start record");

        state.session_id = Some("different-session".to_owned());
        assert!(authorize_worker_start(&state, &identity, &payload).is_err());
        state.session_id = Some(payload.session_id.clone());
        state.worker_identity.as_mut().expect("identity").pid += 1;
        assert!(authorize_worker_start(&state, &identity, &payload).is_err());
        assert!(authorize_worker_start(&State::default(), &identity, &payload).is_err());
    }
    #[test]
    fn linux_process_stat_parser_handles_parentheses_in_command() {
        let stat = format!(
            "123 (worker) with parens) S {} 4242",
            vec!["0"; 18].join(" ")
        );
        assert_eq!(parse_linux_process_stat(&stat), Ok(('S', 4242)));
        assert!(parse_linux_process_stat("123 malformed").is_err());
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn pidfd_zero_signal_confirms_stable_current_process_handle() {
        let pidfd = open_pidfd(std::process::id())
            .expect("pidfd open")
            .expect("current process is alive");
        assert!(pidfd_send_signal(&pidfd, 0).expect("zero signal"));
    }
    #[test]
    fn trusted_command_resolution_rejects_caller_paths_and_unknown_tools() {
        assert!(resolve_trusted_command("/bin/ip").is_none());
        assert!(resolve_trusted_command("sh").is_none());
        assert!(resolve_trusted_command("../ip").is_none());
    }
    #[test]
    fn production_state_root_is_fixed() {
        assert_eq!(
            default_state_root(),
            PathBuf::from("/var/lib/sora-vpn-controller")
        );
    }
}
