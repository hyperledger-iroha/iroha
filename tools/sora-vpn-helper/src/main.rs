#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
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
use std::{ffi::CStr, os::fd::FromRawFd};

use blake3::hash as blake3_hash;
use clap::{Parser, Subcommand};
use hex::FromHexError;
use iroha_crypto::{
    Algorithm, KeyPair, Signature,
    soranet::handshake::{
        DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
        RuntimeParams, build_client_hello, client_handle_relay_hello,
    },
};
use iroha_data_model::{
    Encode,
    soranet::vpn::{
        VPN_CELL_LEN, VPN_HELPER_TICKET_LEN, VPN_HELPER_TICKET_MAGIC,
        VPN_USAGE_VOUCHER_CONTROL_MAGIC, VpnCellClassV1, VpnCellError, VpnCellFlagsV1,
        VpnCellHeaderV1, VpnCellV1, VpnFlowLabelV1, VpnHelperTicketV1, VpnPaddedCellV1,
        VpnUsageVoucherBodyV1, VpnUsageVoucherEnvelopeV1, VpnUsageVoucherV1,
    },
};
use nix::{
    errno::Errno,
    sys::signal::{self, Signal},
    unistd::Pid,
};
use quinn::{
    self, ClientConfig, ConnectError, Connection, ConnectionError, Endpoint, IdleTimeout,
    ReadExactError, RecvStream, SendStream, TransportConfig, VarInt,
    crypto::rustls::QuicClientConfig as QuinnRustlsClientConfig,
};
use rand::{SeedableRng, rngs::StdRng};
use rustls::{
    RootCertStore,
    client::WebPkiServerVerifier,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::CertificateDer,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tokio::{
    io::unix::AsyncFd,
    io::{self as tokio_io, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::lookup_host,
    signal::unix::{SignalKind, signal},
    time::timeout,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CONNECT_POLL_ATTEMPTS: usize = 50;
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const CONTROLLER_KIND: &str = "linux-helperd";
const PACKET_LEN_PREFIX_BYTES: usize = 2;
const DEFAULT_ROUTE_CMD: &str = "ip";
const DEFAULT_ROUTE_SHOW_PREFIX: [&str; 2] = ["-o", "route"];
const DEFAULT_USAGE_VOUCHER_INTERVAL_MS: u64 = 1_000;
#[cfg(target_os = "linux")]
const LINUX_IFF_TUN: nix::libc::c_short = 0x0001;
#[cfg(target_os = "linux")]
const LINUX_IFF_NO_PI: nix::libc::c_short = 0x1000;
#[cfg(target_os = "linux")]
const LINUX_TUNSETIFF: nix::libc::c_ulong = 0x4004_54ca;

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
    Connect {
        payload: Option<String>,
    },
    Disconnect,
    Repair,
    #[command(hide = true)]
    RunTunnel {
        payload: Option<String>,
    },
    #[command(hide = true)]
    RunPacketEngine {
        payload: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
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
    pid: Option<u32>,
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
            interface_name: current_interface_name(),
            network_service: None,
            version: VERSION.to_owned(),
            controller_path: current_controller_path(),
            repair_required: false,
            bytes_in: 0,
            bytes_out: 0,
            message: "ready".to_owned(),
            pid: None,
            session_id: None,
            relay_endpoint: None,
            applied_network: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ConnectPayload {
    session_id: String,
    relay_endpoint: String,
    exit_class: String,
    helper_ticket_hex: String,
    #[serde(alias = "relay_tls_spki_sha256_hex")]
    relay_tls_spki_sha256_hex: String,
    #[serde(alias = "padding_budget_ms")]
    padding_budget_ms: u16,
    #[serde(default)]
    route_pushes: Vec<String>,
    #[serde(default)]
    excluded_routes: Vec<String>,
    #[serde(default)]
    dns_servers: Vec<String>,
    #[serde(default)]
    tunnel_addresses: Vec<String>,
    mtu_bytes: u64,
    #[serde(default)]
    lease_secs: u64,
    #[serde(default)]
    lease_fee_nanos: u64,
    #[serde(default)]
    metering_private_key_seed_hex: Option<String>,
    #[serde(default = "default_usage_voucher_interval_ms")]
    usage_voucher_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
struct AppliedNetworkState {
    interface_name: String,
    dns_backend: Option<DnsBackendState>,
    excluded_route_snapshots: Vec<ExcludedRouteSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum DnsBackendState {
    Resolved { interface_name: String },
    ResolvConf { backup_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ExcludedRouteSnapshot {
    cidr: String,
    family: IpFamily,
    previous_route: Option<String>,
}

type RouteViaDev = (Option<String>, Option<String>);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedMultiaddrHost {
    Ip(IpAddr),
    Dns(String),
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
    last_emitted_at: Instant,
    lease_fee_nanos: u64,
    lease_secs: u64,
    interval: Duration,
}

#[derive(Debug, Default)]
struct PacketStreamDecoder {
    buffer: Vec<u8>,
    expected_len: Option<usize>,
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
            let mut state = current_state();
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
            let state = current_state();
            print_state(&state)?;
            Ok(())
        }
        Command::Connect { payload } => connect_command(payload.as_deref()),
        Command::Disconnect => disconnect_command("idle"),
        Command::Repair => repair_command(),
        Command::RunTunnel { payload } => {
            let payload = parse_connect_payload(payload.as_deref())?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(run_tunnel_command(payload))
        }
        Command::RunPacketEngine { payload } => {
            let payload = parse_connect_payload(payload.as_deref())?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(run_packet_engine_command(payload))
        }
    }
}

fn connect_command(raw_payload: Option<&str>) -> Result<(), ControllerError> {
    let payload = parse_connect_payload(raw_payload)?;
    if let Some(existing_pid) = current_state().pid {
        terminate_pid(existing_pid)?;
        let _ = wait_for_pid_exit(existing_pid, Duration::from_secs(2));
    }

    let mut state = State {
        message: "starting".to_owned(),
        session_id: Some(payload.session_id.clone()),
        relay_endpoint: Some(payload.relay_endpoint.clone()),
        ..State::default()
    };
    persist_state(&state)?;

    let current_exe = env::current_exe()?;
    let child = ProcessCommand::new(current_exe)
        .arg("run-tunnel")
        .arg(
            serde_json::to_string(&payload)
                .map_err(|error| ControllerError::InvalidPayload(error.to_string()))?,
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let child_pid = child.id();
    state.pid = Some(child_pid);
    persist_state(&state)?;

    for _ in 0..CONNECT_POLL_ATTEMPTS {
        sleep_blocking(CONNECT_POLL_INTERVAL);
        let state = current_state();
        if state.pid == Some(child_pid) && state.active {
            print_state(&state)?;
            return Ok(());
        }
        if !process_alive(child_pid) {
            return Err(ControllerError::State(state.message));
        }
    }

    terminate_pid(child_pid)?;
    let _ = wait_for_pid_exit(child_pid, Duration::from_secs(2));
    Err(ControllerError::State(
        "timed out waiting for VPN tunnel worker to report readiness".to_owned(),
    ))
}

fn disconnect_command(message: &str) -> Result<(), ControllerError> {
    let mut state = current_state();
    if let Some(pid) = state.pid {
        terminate_pid(pid)?;
        let _ = wait_for_pid_exit(pid, Duration::from_secs(2));
    }
    cleanup_persisted_network(&mut state)?;
    state.active = false;
    state.repair_required = false;
    state.pid = None;
    state.message = message.to_owned();
    persist_state(&state)?;
    print_state(&state)?;
    Ok(())
}

fn repair_command() -> Result<(), ControllerError> {
    let mut state = current_state();
    if let Some(pid) = state.pid {
        terminate_pid(pid)?;
        let _ = wait_for_pid_exit(pid, Duration::from_secs(2));
    }
    cleanup_persisted_network(&mut state)?;
    state.active = false;
    state.repair_required = false;
    state.pid = None;
    state.message = "repaired".to_owned();
    persist_state(&state)?;
    print_state(&state)?;
    Ok(())
}

async fn run_tunnel_command(payload: ConnectPayload) -> Result<(), ControllerError> {
    let pid = std::process::id();
    let mut state = current_state();
    state.active = false;
    state.repair_required = false;
    state.pid = Some(pid);
    state.session_id = Some(payload.session_id.clone());
    state.relay_endpoint = Some(payload.relay_endpoint.clone());
    state.bytes_in = 0;
    state.bytes_out = 0;
    state.message = "connecting".to_owned();
    state.applied_network = None;
    persist_state(&state)?;

    let (endpoint, connection) = match connect_and_handshake(&payload).await {
        Ok(result) => result,
        Err(error) => {
            update_terminal_state(
                false,
                false,
                Some(pid),
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
                Some(pid),
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
                Some(pid),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                failure.to_string(),
            )?;
            return Err(failure);
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
                Some(pid),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                error.to_string(),
            )?;
            return Err(error);
        }
    };

    state = current_state();
    state.active = true;
    state.repair_required = false;
    state.interface_name = Some(prepared.interface_name.clone());
    state.network_service = prepared.network_service.clone();
    state.applied_network = Some(prepared.applied_network.clone());
    state.message = "connected".to_owned();
    persist_state(&state)?;

    let circuit_id = relay_session_id_from_session_id(payload.session_id.as_str());
    let flow_label = vpn_flow_label_from_session_id(circuit_id)?;
    let voucher_signer = UsageVoucherSigner::from_payload(&payload)?;
    let voucher_counters = UsageVoucherCounters::default();
    let shutdown = tunnel_packet_loop(
        Arc::clone(&prepared.device),
        &mut send,
        &mut recv,
        TunnelTrafficConfig {
            circuit_id,
            flow_label,
            padding_budget_ms: payload.padding_budget_ms,
            packet_read_mtu: prepared.packet_read_mtu,
        },
        voucher_signer,
        voucher_counters,
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

    let _ = send.finish();
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

async fn run_packet_engine_command(payload: ConnectPayload) -> Result<(), ControllerError> {
    let pid = std::process::id();
    let (endpoint, connection) = connect_and_handshake(&payload).await?;
    let (mut send, mut recv) = match timeout(CONNECT_TIMEOUT, connection.open_bi()).await {
        Ok(Ok(streams)) => streams,
        Ok(Err(error)) => {
            let failure = ControllerError::Connection(error);
            connection.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.wait_idle().await;
            return Err(failure);
        }
        Err(_) => {
            let failure =
                ControllerError::State("timed out opening relay VPN tunnel stream".to_owned());
            connection.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.close(0u32.into(), failure.to_string().as_bytes());
            endpoint.wait_idle().await;
            return Err(failure);
        }
    };

    update_terminal_state(
        true,
        false,
        Some(pid),
        payload.session_id.as_str(),
        payload.relay_endpoint.as_str(),
        "connected".to_owned(),
    )
    .map_err(|error| {
        ControllerError::State(format!(
            "failed to persist packet engine connected state: {error}"
        ))
    })?;

    let circuit_id = relay_session_id_from_session_id(payload.session_id.as_str());
    let flow_label = vpn_flow_label_from_session_id(circuit_id)?;
    let voucher_signer = UsageVoucherSigner::from_payload(&payload)?;
    let voucher_counters = UsageVoucherCounters::default();
    let shutdown = packet_engine_loop(
        &mut send,
        &mut recv,
        circuit_id,
        flow_label,
        payload.padding_budget_ms,
        voucher_signer,
        voucher_counters,
    )
    .await;
    let (repair_required, message) = match shutdown {
        Ok(exit) => (exit.repair_required, exit.message),
        Err(error) => (false, error.to_string()),
    };

    let _ = send.finish();
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
    )
    .map_err(|error| {
        ControllerError::State(format!(
            "failed to persist packet engine disconnected state: {error}"
        ))
    })?;
    Ok(())
}

async fn connect_and_handshake(
    payload: &ConnectPayload,
) -> Result<(Endpoint, Connection), ControllerError> {
    let helper_ticket = decode_hex(payload.helper_ticket_hex.as_str())?;
    let relay = parse_multiaddr(payload.relay_endpoint.as_str())?;
    let relay_addr = resolve_multiaddr_socket_addr(&relay)
        .await
        .map_err(|error| {
            ControllerError::State(format!(
                "failed to resolve packet engine relay address {}: {error}",
                payload.relay_endpoint
            ))
        })?;
    let bind_addr = match relay_addr {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let mut endpoint = Endpoint::client(bind_addr).map_err(|error| {
        ControllerError::State(format!(
            "failed to create packet engine QUIC endpoint on {bind_addr}: {error}"
        ))
    })?;
    let relay_tls_pin = parse_fixed_hex_32(
        payload.relay_tls_spki_sha256_hex.as_str(),
        "relay TLS SPKI pin",
    )?;
    endpoint.set_default_client_config(build_client_config(relay_tls_pin).map_err(|error| {
        ControllerError::State(format!(
            "failed to build packet engine QUIC client config: {error}"
        ))
    })?);

    let connect = endpoint
        .connect(relay_addr, "soranet.local")
        .map_err(|error| {
            ControllerError::State(format!(
                "failed to start packet engine QUIC connect to {relay_addr}: {error}"
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

    perform_helper_handshake(&connection, helper_ticket).await?;
    Ok((endpoint, connection))
}

async fn resolve_multiaddr_socket_addr(
    relay: &ParsedMultiaddr,
) -> Result<SocketAddr, ControllerError> {
    match &relay.host {
        ParsedMultiaddrHost::Ip(host) => Ok(SocketAddr::new(*host, relay.port)),
        ParsedMultiaddrHost::Dns(host) => lookup_host((host.as_str(), relay.port))
            .await?
            .next()
            .ok_or_else(|| {
                ControllerError::InvalidMultiaddr(format!("dns {host} did not resolve"))
            }),
    }
}

fn build_client_config(relay_tls_spki_sha256: [u8; 32]) -> Result<ClientConfig, ControllerError> {
    let verifier: Arc<dyn ServerCertVerifier> = Arc::new(PinnedSpkiVerifier {
        relay_tls_spki_sha256,
    });
    let mut tls_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    let tls_config = Arc::new({
        tls_config.enable_early_data = true;
        tls_config
    });
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

async fn perform_helper_handshake(
    connection: &Connection,
    helper_ticket: Vec<u8>,
) -> Result<(), ControllerError> {
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

    let params = RuntimeParams {
        descriptor_commit: &DEFAULT_DESCRIPTOR_COMMIT,
        client_capabilities: &DEFAULT_CLIENT_CAPABILITIES,
        relay_capabilities: &DEFAULT_RELAY_CAPABILITIES,
        kem_id: 1,
        sig_id: 1,
        resume_hash: None,
    };
    let mut rng = StdRng::from_os_rng();
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (client_hello, client_state) = build_client_hello(&params, &mut rng)
        .map_err(|error| ControllerError::Handshake(error.to_string()))?;
    write_handshake_frame(&mut send, &client_hello).await?;

    let relay_hello = read_handshake_frame(&mut recv).await?;
    let (client_finish, _session) =
        client_handle_relay_hello(client_state, &relay_hello, &key_pair, &params, &mut rng)
            .map_err(|error| ControllerError::Handshake(error.to_string()))?;
    if let Some(frame) = client_finish {
        write_handshake_frame(&mut send, &frame).await?;
    }
    send.finish()?;
    Ok(())
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
        let _ = cleanup_network(&applied_network);
        return Err(error);
    }

    if let Err(error) = apply_route_pushes(&applied_network.interface_name, &payload.route_pushes) {
        let _ = cleanup_network(&applied_network);
        return Err(error);
    }

    match apply_excluded_routes(&payload.excluded_routes) {
        Ok(snapshots) => {
            applied_network.excluded_route_snapshots = snapshots;
        }
        Err(error) => {
            let _ = cleanup_network(&applied_network);
            return Err(error);
        }
    }

    let dns_backend = match apply_dns(&applied_network.interface_name, &payload.dns_servers) {
        Ok(backend) => backend,
        Err(error) => {
            let _ = cleanup_network(&applied_network);
            return Err(error);
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

fn cleanup_tunnel(prepared: PreparedTunnel) -> Result<(), ControllerError> {
    cleanup_network(&prepared.applied_network)?;
    drop(prepared);
    Ok(())
}

async fn tunnel_packet_loop(
    device: Arc<LinuxTunDevice>,
    send: &mut SendStream,
    recv: &mut RecvStream,
    traffic: TunnelTrafficConfig,
    voucher_signer: Option<UsageVoucherSigner>,
    voucher_counters: UsageVoucherCounters,
) -> Result<TunnelShutdown, ControllerError> {
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let upstream = tun_to_vpn_loop(
        Arc::clone(&device),
        send,
        traffic,
        voucher_signer,
        voucher_counters.clone(),
    );
    let downstream = vpn_to_tun_loop(device, recv, voucher_counters);
    tokio::pin!(upstream);
    tokio::pin!(downstream);

    tokio::select! {
        _ = sigterm.recv() => Ok(TunnelShutdown {
            repair_required: false,
            message: "idle".to_owned(),
        }),
        _ = sigint.recv() => Ok(TunnelShutdown {
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

async fn packet_engine_loop(
    send: &mut SendStream,
    recv: &mut RecvStream,
    circuit_id: [u8; 16],
    flow_label: VpnFlowLabelV1,
    padding_budget_ms: u16,
    voucher_signer: Option<UsageVoucherSigner>,
    voucher_counters: UsageVoucherCounters,
) -> Result<TunnelShutdown, ControllerError> {
    let mut sigterm = signal(SignalKind::terminate()).map_err(|error| {
        ControllerError::State(format!(
            "failed to register packet engine SIGTERM handler: {error}"
        ))
    })?;
    let mut sigint = signal(SignalKind::interrupt()).map_err(|error| {
        ControllerError::State(format!(
            "failed to register packet engine SIGINT handler: {error}"
        ))
    })?;
    let upstream = packet_engine_to_vpn_loop(
        send,
        circuit_id,
        flow_label,
        padding_budget_ms,
        voucher_signer,
        voucher_counters.clone(),
    );
    let downstream = vpn_to_packet_engine_loop(recv, voucher_counters);
    tokio::pin!(upstream);
    tokio::pin!(downstream);

    tokio::select! {
        _ = sigterm.recv() => Ok(TunnelShutdown {
            repair_required: false,
            message: "idle".to_owned(),
        }),
        _ = sigint.recv() => Ok(TunnelShutdown {
            repair_required: false,
            message: "idle".to_owned(),
        }),
        result = &mut upstream => match result {
            Ok(()) => Ok(TunnelShutdown {
                repair_required: false,
                message: "packet engine input closed".to_owned(),
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

async fn tun_to_vpn_loop(
    device: Arc<LinuxTunDevice>,
    send: &mut SendStream,
    traffic: TunnelTrafficConfig,
    mut voucher_signer: Option<UsageVoucherSigner>,
    voucher_counters: UsageVoucherCounters,
) -> Result<(), ControllerError> {
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

async fn packet_engine_to_vpn_loop(
    send: &mut SendStream,
    circuit_id: [u8; 16],
    flow_label: VpnFlowLabelV1,
    padding_budget_ms: u16,
    mut voucher_signer: Option<UsageVoucherSigner>,
    voucher_counters: UsageVoucherCounters,
) -> Result<(), ControllerError> {
    let mut reader = tokio_io::stdin();
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
    loop {
        let Some(packet) = read_packet_from_stream(&mut reader)
            .await
            .map_err(|error| {
                ControllerError::State(format!(
                    "failed to read packet engine stdin stream: {error}"
                ))
            })?
        else {
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
            send.finish()?;
            return Ok(());
        };
        voucher_counters.add_egress(packet.len() as u64);
        let encoded = encode_packet_stream_frame(&packet)?;
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
        if let Some(signer) = voucher_signer.as_mut()
            && signer.should_emit()
        {
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
        add_traffic_bytes(0, packet.len() as u64)?;
    }
}

async fn vpn_to_tun_loop(
    device: Arc<LinuxTunDevice>,
    recv: &mut RecvStream,
    voucher_counters: UsageVoucherCounters,
) -> Result<(), ControllerError> {
    let mut decoder = PacketStreamDecoder::default();
    let mut frame = [0u8; VPN_CELL_LEN];

    loop {
        match recv.read_exact(&mut frame).await {
            Ok(()) => {
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
            Err(ReadExactError::FinishedEarly(0)) => return Ok(()),
            Err(ReadExactError::FinishedEarly(_)) => {
                return Err(ControllerError::State(
                    "relay tunnel closed mid-frame".to_owned(),
                ));
            }
            Err(ReadExactError::ReadError(error)) => {
                return Err(ControllerError::State(format!(
                    "relay read failed: {error}"
                )));
            }
        }
    }
}

async fn vpn_to_packet_engine_loop(
    recv: &mut RecvStream,
    voucher_counters: UsageVoucherCounters,
) -> Result<(), ControllerError> {
    let mut decoder = PacketStreamDecoder::default();
    let mut frame = [0u8; VPN_CELL_LEN];
    let mut writer = tokio_io::stdout();

    loop {
        match recv.read_exact(&mut frame).await {
            Ok(()) => {
                let cell = VpnPaddedCellV1::parse_bytes_with_flow_label_bits(
                    &frame,
                    VpnFlowLabelV1::MAX_BITS,
                )?;
                if cell.header.class != VpnCellClassV1::Data {
                    continue;
                }
                for packet in decoder.ingest(&cell.payload)? {
                    write_packet_to_stream(&mut writer, &packet)
                        .await
                        .map_err(|error| {
                            ControllerError::State(format!(
                                "failed to write packet engine stdout stream: {error}"
                            ))
                        })?;
                    voucher_counters.add_ingress(packet.len() as u64);
                    add_traffic_bytes(packet.len() as u64, 0)?;
                }
            }
            Err(ReadExactError::FinishedEarly(0)) => return Ok(()),
            Err(ReadExactError::FinishedEarly(_)) => {
                return Err(ControllerError::State(
                    "relay tunnel closed mid-frame".to_owned(),
                ));
            }
            Err(ReadExactError::ReadError(error)) => {
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
        if payload.lease_fee_nanos == 0 || payload.lease_secs == 0 {
            return Err(ControllerError::InvalidPayload(
                "leaseSecs and leaseFeeNanos are required when meteringPrivateKeySeedHex is set"
                    .to_owned(),
            ));
        }
        let seed = parse_fixed_hex_32(seed_hex, "metering private key seed")?;
        let ticket = decode_helper_ticket_metadata(payload.helper_ticket_hex.as_str())?;
        let expected_session_id = relay_session_id_from_session_id(payload.session_id.as_str());
        if ticket.session_id != expected_session_id {
            return Err(ControllerError::InvalidPayload(
                "helper ticket session id does not match connect sessionId".to_owned(),
            ));
        }
        let key_pair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
        let now = Instant::now();
        Ok(Some(Self {
            key_pair,
            ticket,
            sequence: 0,
            started_at: now,
            last_emitted_at: now,
            lease_fee_nanos: payload.lease_fee_nanos,
            lease_secs: payload.lease_secs,
            interval: Duration::from_millis(payload.usage_voucher_interval_ms.max(1)),
        }))
    }

    fn should_emit(&self) -> bool {
        self.last_emitted_at.elapsed() >= self.interval
    }

    fn build_envelope(&mut self, counters: &UsageVoucherCounters) -> VpnUsageVoucherEnvelopeV1 {
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
            issued_at_ms: unix_now_ms(),
        };
        let voucher = VpnUsageVoucherV1 {
            signature: Signature::new(self.key_pair.private_key(), &body.encode()),
            client_public_key: self.key_pair.public_key().clone(),
            body,
        };
        let earned_fee_nanos = tariff_earned_fee_nanos(
            self.lease_fee_nanos,
            self.lease_secs,
            voucher.body.active_ms,
        );
        self.sequence = self.sequence.saturating_add(1);
        self.last_emitted_at = Instant::now();
        VpnUsageVoucherEnvelopeV1 {
            voucher,
            earned_fee_nanos,
        }
    }
}

async fn send_usage_voucher_control_cell(
    send: &mut SendStream,
    circuit_id: [u8; 16],
    flow_label: VpnFlowLabelV1,
    padding_budget_ms: u16,
    counters: &UsageVoucherCounters,
    signer: &mut UsageVoucherSigner,
    sequence: &mut u64,
) -> Result<(), ControllerError> {
    let envelope = signer.build_envelope(counters);
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

fn tariff_active_fee_nanos_per_minute(lease_fee_nanos: u64, lease_secs: u64) -> u64 {
    let numerator = u128::from(lease_fee_nanos).saturating_mul(60);
    let denominator = u128::from(lease_secs.max(1));
    u64::try_from(numerator.div_ceil(denominator)).unwrap_or(u64::MAX)
}

fn tariff_earned_fee_nanos(lease_fee_nanos: u64, lease_secs: u64, active_ms: u64) -> u64 {
    let active_fee_nanos_per_minute =
        tariff_active_fee_nanos_per_minute(lease_fee_nanos, lease_secs);
    let earned = u128::from(active_ms)
        .saturating_mul(u128::from(active_fee_nanos_per_minute))
        .div_ceil(60_000);
    u64::try_from(earned.min(u128::from(lease_fee_nanos))).unwrap_or(u64::MAX)
}

fn decode_helper_ticket_metadata(hex_ticket: &str) -> Result<VpnHelperTicketV1, ControllerError> {
    let bytes = decode_hex(hex_ticket)?;
    if bytes.len() != VPN_HELPER_TICKET_LEN || !bytes.starts_with(VPN_HELPER_TICKET_MAGIC) {
        return Err(ControllerError::InvalidPayload(
            "helperTicketHex is not a v1 helper ticket".to_owned(),
        ));
    }
    let mut cursor = VPN_HELPER_TICKET_MAGIC.len();
    let mut session_id = [0u8; 16];
    session_id.copy_from_slice(&bytes[cursor..cursor + 16]);
    cursor += 16;
    let mut quote_id = [0u8; 32];
    quote_id.copy_from_slice(&bytes[cursor..cursor + 32]);
    cursor += 32;
    let mut account_hash = [0u8; 32];
    account_hash.copy_from_slice(&bytes[cursor..cursor + 32]);
    cursor += 32;
    let mut relay_id = [0u8; 32];
    relay_id.copy_from_slice(&bytes[cursor..cursor + 32]);
    cursor += 32;
    let mut payment_tx_hash = [0u8; 32];
    payment_tx_hash.copy_from_slice(&bytes[cursor..cursor + 32]);
    cursor += 32;
    let mut expires_at = [0u8; 8];
    expires_at.copy_from_slice(&bytes[cursor..cursor + 8]);
    Ok(VpnHelperTicketV1 {
        session_id,
        quote_id,
        account_hash,
        relay_id,
        payment_tx_hash,
        expires_at_ms: u64::from_be_bytes(expires_at),
    })
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

impl PacketStreamDecoder {
    fn ingest(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, ControllerError> {
        self.buffer.extend_from_slice(bytes);
        let mut packets = Vec::new();
        loop {
            if self.expected_len.is_none() {
                if self.buffer.len() < PACKET_LEN_PREFIX_BYTES {
                    break;
                }
                let len = usize::from(u16::from_be_bytes([self.buffer[0], self.buffer[1]]));
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
                nix::libc::O_RDWR | nix::libc::O_NONBLOCK,
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
            req.ifr_ifru.ifru_flags = (LINUX_IFF_TUN | LINUX_IFF_NO_PI) as _;
        }

        let ioctl_result = unsafe { nix::libc::ioctl(fd, LINUX_TUNSETIFF as _, &req) };
        if ioctl_result < 0 {
            let error = io::Error::last_os_error();
            unsafe {
                nix::libc::close(fd);
            }
            return Err(ControllerError::Io(error));
        }

        let name = unsafe { CStr::from_ptr(req.ifr_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
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

async fn read_packet_from_stream<R>(reader: &mut R) -> Result<Option<Vec<u8>>, ControllerError>
where
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; PACKET_LEN_PREFIX_BYTES];
    let first_len = reader.read(&mut len_buf[..1]).await?;
    if first_len == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut len_buf[1..]).await?;
    let packet_len = usize::from(u16::from_be_bytes(len_buf));
    let mut packet = vec![0u8; packet_len];
    reader.read_exact(&mut packet).await?;
    Ok(Some(packet))
}

async fn write_packet_to_stream<W>(writer: &mut W, packet: &[u8]) -> Result<(), ControllerError>
where
    W: AsyncWrite + Unpin,
{
    let packet_len = u16::try_from(packet.len()).map_err(|_| {
        ControllerError::State(format!(
            "packet length {} exceeds u16 packet-stream limit",
            packet.len()
        ))
    })?;
    writer.write_all(&packet_len.to_be_bytes()).await?;
    writer.write_all(packet).await?;
    writer.flush().await?;
    Ok(())
}

fn add_traffic_bytes(bytes_in: u64, bytes_out: u64) -> Result<(), ControllerError> {
    if bytes_in == 0 && bytes_out == 0 {
        return Ok(());
    }
    let mut state = current_state();
    state.bytes_in = state.bytes_in.saturating_add(bytes_in);
    state.bytes_out = state.bytes_out.saturating_add(bytes_out);
    persist_state(&state)?;
    Ok(())
}

fn cleanup_persisted_network(state: &mut State) -> Result<(), ControllerError> {
    if let Some(applied) = state.applied_network.take() {
        cleanup_network(&applied)?;
    }
    state.network_service = None;
    Ok(())
}

fn cleanup_network(applied: &AppliedNetworkState) -> Result<(), ControllerError> {
    if let Some(dns_backend) = &applied.dns_backend {
        cleanup_dns(dns_backend)?;
    }
    for snapshot in &applied.excluded_route_snapshots {
        restore_excluded_route(snapshot)?;
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
            vec![
                address.family().flag().to_owned(),
                "address".to_owned(),
                "replace".to_owned(),
                format!("{}/{}", address.address, address.prefix),
                "dev".to_owned(),
                interface_name.to_owned(),
            ],
        )?;
    }
    Ok(())
}

fn apply_route_pushes(interface_name: &str, routes: &[String]) -> Result<(), ControllerError> {
    for route in routes {
        let parsed = parse_cidr(route)?;
        run_command(
            DEFAULT_ROUTE_CMD,
            vec![
                parsed.family().flag().to_owned(),
                "route".to_owned(),
                "replace".to_owned(),
                route.trim().to_owned(),
                "dev".to_owned(),
                interface_name.to_owned(),
            ],
        )?;
    }
    Ok(())
}

fn apply_excluded_routes(routes: &[String]) -> Result<Vec<ExcludedRouteSnapshot>, ControllerError> {
    let mut snapshots = Vec::with_capacity(routes.len());
    for route in routes {
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
        snapshots.push(ExcludedRouteSnapshot {
            cidr: normalized,
            family: parsed.family(),
            previous_route,
        });
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
        return Ok(Some(DnsBackendState::Resolved {
            interface_name: interface_name.to_owned(),
        }));
    }

    let backup_path = resolv_conf_backup_path();
    let backup_bytes = fs::read("/etc/resolv.conf").unwrap_or_default();
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_private_file(&backup_path, &backup_bytes)?;

    let mut rendered = String::from("# sora-vpn-controller managed resolv.conf\n");
    for server in dns_servers {
        rendered.push_str("nameserver ");
        rendered.push_str(server.trim());
        rendered.push('\n');
    }
    write_private_file(Path::new("/etc/resolv.conf"), rendered.as_bytes())?;
    Ok(Some(DnsBackendState::ResolvConf {
        backup_path: backup_path.to_string_lossy().into_owned(),
    }))
}

fn cleanup_dns(backend: &DnsBackendState) -> Result<(), ControllerError> {
    match backend {
        DnsBackendState::Resolved { interface_name } => {
            run_command(
                "resolvectl",
                vec!["revert".to_owned(), interface_name.clone()],
            )?;
        }
        DnsBackendState::ResolvConf { backup_path } => {
            let backup = PathBuf::from(backup_path);
            let bytes = fs::read(&backup)?;
            write_private_file(Path::new("/etc/resolv.conf"), &bytes)?;
            let _ = fs::remove_file(backup);
        }
    }
    Ok(())
}

fn dns_backend_label(backend: &DnsBackendState) -> String {
    match backend {
        DnsBackendState::Resolved { .. } => "resolvectl".to_owned(),
        DnsBackendState::ResolvConf { .. } => "resolv.conf".to_owned(),
    }
}

fn resolv_conf_backup_path() -> PathBuf {
    state_path()
        .parent()
        .unwrap_or_else(|| Path::new("/tmp"))
        .join("resolv.conf.backup")
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
    let output = ProcessCommand::new(&program_path)
        .env_clear()
        .env("PATH", "/usr/sbin:/sbin:/usr/bin:/bin")
        .args(&collected)
        .output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let detail = if stderr.is_empty() {
        format!("exit status {}", output.status)
    } else {
        stderr
    };
    Err(ControllerError::State(format!(
        "{program} {} failed: {detail}",
        collected.join(" ")
    )))
}

fn resolve_trusted_command(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let path = PathBuf::from(program);
        return path.exists().then_some(path);
    }
    ["/usr/sbin", "/sbin", "/usr/bin", "/bin"]
        .into_iter()
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| candidate.exists())
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
    if let Some(name) = current_interface_name() {
        return Ok(name);
    }
    let suffix = session_id
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .take(10)
        .collect::<String>();
    let name = format!(
        "srvpn{}",
        if suffix.is_empty() {
            "0000000000"
        } else {
            suffix.as_str()
        }
    );
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
    pid: Option<u32>,
    session_id: &str,
    relay_endpoint: &str,
    message: String,
) -> Result<(), ControllerError> {
    let mut state = current_state();
    state.active = active;
    state.repair_required = repair_required;
    state.pid = pid;
    state.session_id = Some(session_id.to_owned());
    state.relay_endpoint = Some(relay_endpoint.to_owned());
    if !active {
        state.applied_network = None;
        state.network_service = None;
    }
    state.message = message;
    persist_state(&state)?;
    Ok(())
}

fn current_state() -> State {
    let mut state = load_state();
    hydrate_runtime_fields(&mut state);
    scrub_stale_process(&mut state);
    state
}

fn load_state() -> State {
    let path = state_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return State::default();
    };
    serde_json::from_str::<State>(&raw).unwrap_or_default()
}

fn persist_state(state: &State) -> Result<(), ControllerError> {
    let path = state_path();
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec(state)
        .map_err(|error| ControllerError::State(format!("failed to encode state: {error}")))?;
    write_private_file(path.as_path(), &bytes)?;
    Ok(())
}

fn print_state(state: &State) -> Result<(), ControllerError> {
    let rendered = serde_json::to_string(state)
        .map_err(|error| ControllerError::State(format!("failed to render state: {error}")))?;
    println!("{rendered}");
    Ok(())
}

fn state_path() -> PathBuf {
    env::var("SORANET_VPN_STATE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_state_path())
}

fn default_state_path() -> PathBuf {
    if let Some(runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("sora-vpn-controller/state.json");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".local/state/sora-vpn-controller/state.json");
    }
    PathBuf::from("/var/lib/sora-vpn-controller/state.json")
}

fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(bytes)
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
    if state.interface_name.is_none() {
        state.interface_name = current_interface_name();
    }
}

fn current_interface_name() -> Option<String> {
    env::var("SORANET_VPN_INTERFACE")
        .ok()
        .and_then(|value| trim_to_option(value.as_str()))
}

fn current_controller_path() -> Option<String> {
    env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(ToOwned::to_owned))
}

fn trim_to_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn scrub_stale_process(state: &mut State) {
    let Some(pid) = state.pid else {
        return;
    };
    if process_alive(pid) {
        return;
    }
    state.active = false;
    state.pid = None;
    state.repair_required = state.applied_network.is_some();
    if state.message == "connected" || state.message == "starting" || state.message == "connecting"
    {
        state.message = "tunnel worker exited".to_owned();
    }
}

fn process_alive(pid: u32) -> bool {
    let Ok(raw_pid) = i32::try_from(pid) else {
        return false;
    };
    match signal::kill(Pid::from_raw(raw_pid), None) {
        Ok(()) => true,
        Err(Errno::EPERM) => true,
        Err(Errno::ESRCH) => false,
        Err(_) => false,
    }
}

fn terminate_pid(pid: u32) -> Result<(), ControllerError> {
    if !process_alive(pid) {
        return Ok(());
    }
    let raw_pid =
        i32::try_from(pid).map_err(|_| ControllerError::State(format!("invalid pid {pid}")))?;
    signal::kill(Pid::from_raw(raw_pid), Signal::SIGTERM)
        .map_err(|error| ControllerError::State(format!("failed to terminate pid {pid}: {error}")))
}

fn wait_for_pid_exit(pid: u32, timeout_limit: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout_limit;
    while process_alive(pid) && std::time::Instant::now() < deadline {
        sleep_blocking(Duration::from_millis(50));
    }
    !process_alive(pid)
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
    let decoded = decode_hex(value)?;
    let bytes: [u8; 32] = decoded.try_into().map_err(|bytes: Vec<u8>| {
        ControllerError::InvalidPayload(format!(
            "{label} must decode to 32 bytes (got {})",
            bytes.len()
        ))
    })?;
    Ok(bytes)
}

fn parse_connect_payload(raw_payload: Option<&str>) -> Result<ConnectPayload, ControllerError> {
    let raw_payload = raw_payload.ok_or(ControllerError::MissingPayload)?;
    let payload = serde_json::from_str::<ConnectPayload>(raw_payload)
        .map_err(|error| ControllerError::InvalidPayload(error.to_string()))?;
    if payload.session_id.trim().is_empty() {
        return Err(ControllerError::InvalidPayload(
            "sessionId must not be empty".to_owned(),
        ));
    }
    if payload.relay_endpoint.trim().is_empty() {
        return Err(ControllerError::InvalidPayload(
            "relayEndpoint must not be empty".to_owned(),
        ));
    }
    if payload.helper_ticket_hex.trim().is_empty() {
        return Err(ControllerError::InvalidPayload(
            "helperTicketHex must not be empty".to_owned(),
        ));
    }
    if payload.relay_tls_spki_sha256_hex.trim().is_empty() {
        return Err(ControllerError::InvalidPayload(
            "relayTlsSpkiSha256Hex must not be empty".to_owned(),
        ));
    }
    let _ = parse_fixed_hex_32(
        payload.relay_tls_spki_sha256_hex.as_str(),
        "relayTlsSpkiSha256Hex",
    )?;
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
    Ok(payload)
}

fn parse_multiaddr(addr: &str) -> Result<ParsedMultiaddr, ControllerError> {
    let trimmed = addr.trim();
    if trimmed.is_empty() {
        return Err(ControllerError::InvalidMultiaddr(addr.to_owned()));
    }
    let mut parts = trimmed.trim_matches('/').split('/');
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
        "dns" | "dns4" | "dns6" => ParsedMultiaddrHost::Dns(
            parts
                .next()
                .ok_or_else(|| ControllerError::InvalidMultiaddr(addr.to_owned()))?
                .to_owned(),
        ),
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
        Some("quic") | None => {}
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
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let spki = extract_spki_der(end_entity.as_ref())?;
        let digest: [u8; 32] = Sha256::digest(spki).into();
        if digest != self.relay_tls_spki_sha256 {
            return Err(rustls::Error::General(
                "relay TLS SPKI pin mismatch".to_owned(),
            ));
        }
        Ok(ServerCertVerified::assertion())
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

fn extract_spki_der(certificate: &[u8]) -> Result<&[u8], rustls::Error> {
    let (_, cert_content, _) = read_der_element(certificate, 0, Some(0x30))?;
    let (tbs_start, tbs_content, _) =
        read_der_element(certificate, cert_content.start, Some(0x30))?;
    let mut cursor = tbs_content.start;
    if certificate
        .get(cursor)
        .copied()
        .is_some_and(|tag| tag == 0xA0)
    {
        let (_, _, next) = read_der_element(certificate, cursor, Some(0xA0))?;
        cursor = next;
    }
    for _ in 0..5 {
        let (_, _, next) = read_der_element(certificate, cursor, None)?;
        cursor = next;
    }
    let (spki_start, _, spki_next) = read_der_element(certificate, cursor, Some(0x30))?;
    if spki_start < tbs_start || spki_next > certificate.len() {
        return Err(rustls::Error::General(
            "invalid relay certificate SPKI bounds".to_owned(),
        ));
    }
    Ok(&certificate[spki_start..spki_next])
}

fn read_der_element(
    bytes: &[u8],
    start: usize,
    expected_tag: Option<u8>,
) -> Result<(usize, std::ops::Range<usize>, usize), rustls::Error> {
    let tag = *bytes
        .get(start)
        .ok_or_else(|| rustls::Error::General("truncated DER element".to_owned()))?;
    if let Some(expected) = expected_tag
        && tag != expected
    {
        return Err(rustls::Error::General(format!(
            "unexpected DER tag {tag:#x}; expected {expected:#x}"
        )));
    }
    let len_start = start
        .checked_add(1)
        .ok_or_else(|| rustls::Error::General("DER offset overflow".to_owned()))?;
    let first_len = *bytes
        .get(len_start)
        .ok_or_else(|| rustls::Error::General("truncated DER length".to_owned()))?;
    let (length, content_start) = if first_len & 0x80 == 0 {
        (
            usize::from(first_len),
            len_start
                .checked_add(1)
                .ok_or_else(|| rustls::Error::General("DER offset overflow".to_owned()))?,
        )
    } else {
        let length_bytes = usize::from(first_len & 0x7F);
        if length_bytes == 0 || length_bytes > std::mem::size_of::<usize>() {
            return Err(rustls::Error::General(
                "unsupported DER length encoding".to_owned(),
            ));
        }
        let length_start = len_start
            .checked_add(1)
            .ok_or_else(|| rustls::Error::General("DER offset overflow".to_owned()))?;
        let length_end = length_start
            .checked_add(length_bytes)
            .ok_or_else(|| rustls::Error::General("DER offset overflow".to_owned()))?;
        let mut length = 0usize;
        for byte in bytes
            .get(length_start..length_end)
            .ok_or_else(|| rustls::Error::General("truncated DER length".to_owned()))?
        {
            length = (length << 8) | usize::from(*byte);
        }
        (length, length_end)
    };
    let content_end = content_start
        .checked_add(length)
        .ok_or_else(|| rustls::Error::General("DER offset overflow".to_owned()))?;
    if content_end > bytes.len() {
        return Err(rustls::Error::General(
            "DER element length exceeds certificate".to_owned(),
        ));
    }
    Ok((start, content_start..content_end, content_end))
}

#[cfg(test)]
mod tests {
    use super::*;

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
                host: ParsedMultiaddrHost::Dns("torii".to_owned()),
                port: 9443,
            }
        );
    }

    #[test]
    fn parse_multiaddr_rejects_non_udp_transport() {
        let err = parse_multiaddr("/ip4/127.0.0.1/tcp/7777/quic").expect_err("must fail");
        assert!(err.to_string().contains("unsupported transport"));
    }

    #[test]
    fn connect_payload_deserializes_camel_case() {
        let payload = parse_connect_payload(Some(
            r#"{"sessionId":"session-1","relayEndpoint":"/ip4/127.0.0.1/udp/7777/quic","exitClass":"standard","helperTicketHex":"aa","relayTlsSpkiSha256Hex":"abababababababababababababababababababababababababababababababab","paddingBudgetMs":15,"routePushes":[],"excludedRoutes":[],"dnsServers":[],"tunnelAddresses":["10.208.0.2/32"],"mtuBytes":1280}"#,
        ))
        .expect("payload");
        assert_eq!("session-1", payload.session_id);
        assert_eq!("/ip4/127.0.0.1/udp/7777/quic", payload.relay_endpoint);
        assert_eq!(1280, payload.mtu_bytes);
        assert_eq!(15, payload.padding_budget_ms);
    }

    #[test]
    fn decode_hex_accepts_prefixed_values() {
        let decoded = decode_hex("0x0A0b").expect("hex");
        assert_eq!(decoded, vec![0x0A, 0x0B]);
    }

    #[test]
    fn helper_ticket_metadata_decodes_without_secret() {
        let ticket = VpnHelperTicketV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            account_hash: [0x33; 32],
            relay_id: [0x44; 32],
            payment_tx_hash: [0x55; 32],
            expires_at_ms: 99_000,
        };
        let encoded = ticket.to_hex(&[0xAA; 32]);
        let decoded = decode_helper_ticket_metadata(&encoded).expect("ticket metadata");

        assert_eq!(decoded, ticket);
    }

    #[test]
    fn usage_voucher_signer_builds_signed_cumulative_voucher() {
        let session_id = "f69c894aa32726fe586fab520f88ae42d1fbb4ebf3083df057f4e40ca0a11111";
        let ticket = VpnHelperTicketV1 {
            session_id: relay_session_id_from_session_id(session_id),
            quote_id: [0x22; 32],
            account_hash: [0x33; 32],
            relay_id: [0x44; 32],
            payment_tx_hash: [0x55; 32],
            expires_at_ms: 99_000,
        };
        let raw_payload = serde_json::json!({
            "sessionId": session_id,
            "relayEndpoint": "/ip4/127.0.0.1/udp/7777/quic",
            "exitClass": "standard",
            "helperTicketHex": ticket.to_hex(&[0xAA; 32]),
            "relayTlsSpkiSha256Hex": "ab".repeat(32),
            "paddingBudgetMs": 15,
            "tunnelAddresses": ["10.208.0.2/32"],
            "mtuBytes": 1280,
            "leaseSecs": 10,
            "leaseFeeNanos": 1_000,
            "meteringPrivateKeySeedHex": "66".repeat(32),
            "usageVoucherIntervalMs": 1
        });
        let payload = parse_connect_payload(Some(&raw_payload.to_string())).expect("payload");
        let mut signer = UsageVoucherSigner::from_payload(&payload)
            .expect("signer")
            .expect("enabled signer");
        let counters = UsageVoucherCounters::default();
        counters.add_ingress(10);
        counters.add_egress(20);

        let envelope = signer.build_envelope(&counters);

        envelope.voucher.verify().expect("voucher signature");
        assert_eq!(envelope.voucher.body.session_id, ticket.session_id);
        assert_eq!(envelope.voucher.body.quote_id, ticket.quote_id);
        assert_eq!(envelope.voucher.body.relay_id, ticket.relay_id);
        assert_eq!(envelope.voucher.body.ingress_bytes, 10);
        assert_eq!(envelope.voucher.body.egress_bytes, 20);
        assert_eq!(
            envelope.earned_fee_nanos,
            tariff_earned_fee_nanos(1_000, 10, envelope.voucher.body.active_ms)
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
    fn packet_stream_round_trips_fragmented_payload() {
        let packet = vec![0xAB; 1500];
        let encoded = encode_packet_stream_frame(&packet).expect("encode");
        let mut decoder = PacketStreamDecoder::default();
        let first = decoder.ingest(&encoded[..700]).expect("first fragment");
        assert!(first.is_empty());
        let second = decoder.ingest(&encoded[700..]).expect("second fragment");
        assert_eq!(second, vec![packet]);
    }

    #[test]
    fn decoder_handles_multiple_packets_in_single_chunk() {
        let first = encode_packet_stream_frame(&[1, 2, 3]).expect("first");
        let second = encode_packet_stream_frame(&[4, 5]).expect("second");
        let mut decoder = PacketStreamDecoder::default();
        let packets = decoder
            .ingest(&[first.as_slice(), second.as_slice()].concat())
            .expect("decode");
        assert_eq!(packets, vec![vec![1, 2, 3], vec![4, 5]]);
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
    fn desired_interface_name_prefers_env_override() {
        let original = env::var_os("SORANET_VPN_INTERFACE");
        // SAFETY: tests run in a controlled process and restore the variable before returning.
        unsafe { env::set_var("SORANET_VPN_INTERFACE", "vpncustom0") };
        let derived = desired_interface_name("deadbeef").expect("name");
        match original {
            Some(value) => {
                // SAFETY: restoring previous test-local environment value.
                unsafe { env::set_var("SORANET_VPN_INTERFACE", value) };
            }
            None => {
                // SAFETY: removing the override set for this test.
                unsafe { env::remove_var("SORANET_VPN_INTERFACE") };
            }
        }
        assert_eq!(derived, "vpncustom0");
    }

    #[test]
    fn relay_session_id_matches_torii_derivation() {
        let session_id = "f69c894aa32726fe586fab520f88ae42d1fbb4ebf3083df057f4e40ca0a11111";
        let derived = relay_session_id_from_session_id(session_id);
        assert_eq!(derived.len(), 16);
        assert_ne!(derived, [0u8; 16]);
    }

    #[test]
    fn cli_accepts_connect_payload_after_subcommand() {
        let payload = r#"{"sessionId":"session-1","relayEndpoint":"/ip4/127.0.0.1/udp/7777/quic","exitClass":"standard","helperTicketHex":"aa","relayTlsSpkiSha256Hex":"abababababababababababababababababababababababababababababababab","paddingBudgetMs":15,"routePushes":[],"excludedRoutes":[],"dnsServers":[],"tunnelAddresses":["10.208.0.2/32"],"mtuBytes":1280}"#;
        let cli = Cli::try_parse_from(["sora-vpn-controller", "connect", payload]).expect("parse");
        match cli.command {
            Command::Connect { payload: parsed } => {
                assert_eq!(parsed.as_deref(), Some(payload));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn cli_accepts_run_packet_engine_payload_after_subcommand() {
        let payload = r#"{"sessionId":"session-1","relayEndpoint":"/ip4/127.0.0.1/udp/7777/quic","exitClass":"standard","helperTicketHex":"aa","relayTlsSpkiSha256Hex":"abababababababababababababababababababababababababababababababab","paddingBudgetMs":15,"routePushes":[],"excludedRoutes":[],"dnsServers":[],"tunnelAddresses":["10.208.0.2/32"],"mtuBytes":1280,"stateFilePath":"/tmp/state.json","packetEnginePath":"/tmp/engine","appGroupId":"group.org.sora.wallet.demo.vpn"}"#;
        let cli = Cli::try_parse_from(["sora-vpn-controller", "run-packet-engine", payload])
            .expect("parse");
        match cli.command {
            Command::RunPacketEngine { payload: parsed } => {
                assert_eq!(parsed.as_deref(), Some(payload));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
