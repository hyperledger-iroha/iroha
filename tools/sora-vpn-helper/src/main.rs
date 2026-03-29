use std::{
    env, fs, io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode, Stdio},
    sync::Arc,
    time::Duration,
};

use clap::{Parser, Subcommand};
use hex::FromHexError;
use iroha_crypto::{
    Algorithm, KeyPair,
    soranet::handshake::{
        DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
        RuntimeParams, build_client_hello, client_handle_relay_hello,
    },
};
use iroha_data_model::soranet::vpn::{
    VPN_CELL_LEN, VpnCellClassV1, VpnCellError, VpnFlowLabelV1, VpnPaddedCellV1,
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
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::CertificateDer,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
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

#[derive(Debug, Parser)]
#[command(name = "sora-vpn-controller")]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// Ignored compatibility flag used by the Electron wrapper.
    #[arg(long)]
    json: bool,
    /// Optional JSON payload consumed by `connect` and the internal tunnel runner.
    payload: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    InstallCheck,
    Status,
    Connect,
    Disconnect,
    Repair,
    #[command(hide = true)]
    RunTunnel,
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
    #[serde(default)]
    route_pushes: Vec<String>,
    #[serde(default)]
    excluded_routes: Vec<String>,
    #[serde(default)]
    dns_servers: Vec<String>,
    #[serde(default)]
    tunnel_addresses: Vec<String>,
    mtu_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedMultiaddr {
    host: IpAddr,
    port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TunnelShutdown {
    repair_required: bool,
    message: String,
}

#[derive(Debug, Error)]
enum ControllerError {
    #[error("connect payload is required")]
    MissingPayload,
    #[error("invalid connect payload: {0}")]
    InvalidPayload(String),
    #[error("invalid relay multiaddr: {0}")]
    InvalidMultiaddr(String),
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
        Command::Connect => connect_command(cli.payload.as_deref()),
        Command::Disconnect => disconnect_command("idle"),
        Command::Repair => repair_command(),
        Command::RunTunnel => {
            let payload = parse_connect_payload(cli.payload.as_deref())?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(run_tunnel_command(payload))
        }
    }
}

fn connect_command(raw_payload: Option<&str>) -> Result<(), ControllerError> {
    let payload = parse_connect_payload(raw_payload)?;
    if let Some(existing_pid) = current_state().pid {
        terminate_pid(existing_pid)?;
        let _ = wait_for_pid_exit(existing_pid, Duration::from_secs(2));
    }

    let mut state = State::default();
    state.message = "starting".to_owned();
    state.session_id = Some(payload.session_id.clone());
    state.relay_endpoint = Some(payload.relay_endpoint.clone());
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
    persist_state(&state)?;

    let (endpoint, connection) = match connect_and_handshake(&payload).await {
        Ok(result) => result,
        Err(error) => {
            update_terminal_state(
                false,
                true,
                Some(pid),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                error.to_string(),
            )?;
            return Err(error);
        }
    };

    let (send, recv) = match timeout(CONNECT_TIMEOUT, connection.open_bi()).await {
        Ok(Ok(streams)) => streams,
        Ok(Err(error)) => {
            let failure = ControllerError::Connection(error);
            update_terminal_state(
                false,
                true,
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
            update_terminal_state(
                false,
                true,
                Some(pid),
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                failure.to_string(),
            )?;
            return Err(failure);
        }
    };

    state.active = true;
    state.repair_required = false;
    state.message = "connected".to_owned();
    persist_state(&state)?;

    let shutdown = match tunnel_read_loop(recv).await {
        Ok(shutdown) => shutdown,
        Err(error) => {
            let message = error.to_string();
            connection.close(0u32.into(), message.as_bytes());
            endpoint.close(0u32.into(), message.as_bytes());
            endpoint.wait_idle().await;
            update_terminal_state(
                false,
                true,
                None,
                payload.session_id.as_str(),
                payload.relay_endpoint.as_str(),
                message,
            )?;
            return Err(error);
        }
    };
    let _send = send;
    connection.close(0u32.into(), shutdown.message.as_bytes());
    endpoint.close(0u32.into(), shutdown.message.as_bytes());
    endpoint.wait_idle().await;
    update_terminal_state(
        false,
        shutdown.repair_required,
        None,
        payload.session_id.as_str(),
        payload.relay_endpoint.as_str(),
        shutdown.message,
    )?;
    Ok(())
}

async fn connect_and_handshake(
    payload: &ConnectPayload,
) -> Result<(Endpoint, Connection), ControllerError> {
    let helper_ticket = decode_hex(payload.helper_ticket_hex.as_str())?;
    let relay = parse_multiaddr(payload.relay_endpoint.as_str())?;
    let bind_addr = match relay.host {
        IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(build_client_config()?);

    let connect = endpoint.connect(SocketAddr::new(relay.host, relay.port), "soranet.local")?;
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

fn build_client_config() -> Result<ClientConfig, ControllerError> {
    let verifier: Arc<dyn ServerCertVerifier> = Arc::new(NoCertificateVerification);
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

async fn tunnel_read_loop(mut recv: RecvStream) -> Result<TunnelShutdown, ControllerError> {
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut frame = [0u8; VPN_CELL_LEN];

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                return Ok(TunnelShutdown {
                    repair_required: false,
                    message: "idle".to_owned(),
                });
            }
            _ = sigint.recv() => {
                return Ok(TunnelShutdown {
                    repair_required: false,
                    message: "idle".to_owned(),
                });
            }
            result = recv.read_exact(&mut frame) => {
                match result {
                    Ok(()) => {
                        let inbound = classify_ingress_frame(&frame)?;
                        if inbound > 0 {
                            add_inbound_bytes(inbound)?;
                        }
                    }
                    Err(ReadExactError::FinishedEarly(0)) => {
                        return Ok(TunnelShutdown {
                            repair_required: false,
                            message: "relay tunnel closed".to_owned(),
                        });
                    }
                    Err(ReadExactError::FinishedEarly(_)) => {
                        return Ok(TunnelShutdown {
                            repair_required: true,
                            message: "relay tunnel closed mid-frame".to_owned(),
                        });
                    }
                    Err(ReadExactError::ReadError(error)) => {
                        return Ok(TunnelShutdown {
                            repair_required: true,
                            message: format!("relay read failed: {error}"),
                        });
                    }
                }
            }
        }
    }
}

fn classify_ingress_frame(frame: &[u8; VPN_CELL_LEN]) -> Result<u64, ControllerError> {
    let cell = VpnPaddedCellV1::parse_bytes_with_flow_label_bits(frame, VpnFlowLabelV1::MAX_BITS)?;
    Ok(if cell.header.class == VpnCellClassV1::Data {
        cell.payload.len() as u64
    } else {
        0
    })
}

fn add_inbound_bytes(delta: u64) -> Result<(), ControllerError> {
    let mut state = current_state();
    state.bytes_in = state.bytes_in.saturating_add(delta);
    persist_state(&state)?;
    Ok(())
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
    fs::write(path, bytes)?;
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
        .unwrap_or_else(|_| PathBuf::from("/tmp/sora-vpn-controller-state.json"))
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
    state.repair_required = true;
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
    if normalized.is_empty() || normalized.len() % 2 != 0 {
        return Err(ControllerError::InvalidPayload(
            "helper ticket must be an even-length hex string".to_owned(),
        ));
    }
    Ok(hex::decode(normalized)?)
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
            IpAddr::V4(
                raw.parse::<Ipv4Addr>()
                    .map_err(|_| ControllerError::InvalidMultiaddr(addr.to_owned()))?,
            )
        }
        "ip6" => {
            let raw = parts
                .next()
                .ok_or_else(|| ControllerError::InvalidMultiaddr(addr.to_owned()))?;
            IpAddr::V6(
                raw.parse::<Ipv6Addr>()
                    .map_err(|_| ControllerError::InvalidMultiaddr(addr.to_owned()))?,
            )
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
struct NoCertificateVerification;

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_multiaddr_accepts_ipv4_quic() {
        let parsed = parse_multiaddr("/ip4/127.0.0.1/udp/7777/quic").expect("parse");
        assert_eq!(
            parsed,
            ParsedMultiaddr {
                host: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
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
                host: IpAddr::V6(Ipv6Addr::LOCALHOST),
                port: 7777,
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
            r#"{"sessionId":"session-1","relayEndpoint":"/ip4/127.0.0.1/udp/7777/quic","exitClass":"standard","helperTicketHex":"aa","routePushes":[],"excludedRoutes":[],"dnsServers":[],"tunnelAddresses":["10.208.0.2/32"],"mtuBytes":1280}"#,
        ))
        .expect("payload");
        assert_eq!("session-1", payload.session_id);
        assert_eq!("/ip4/127.0.0.1/udp/7777/quic", payload.relay_endpoint);
        assert_eq!(1280, payload.mtu_bytes);
    }

    #[test]
    fn decode_hex_accepts_prefixed_values() {
        let decoded = decode_hex("0x0A0b").expect("hex");
        assert_eq!(decoded, vec![0x0A, 0x0B]);
    }
}
