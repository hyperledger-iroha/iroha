#![allow(unexpected_cfgs)]
//! Runs the privileged Sora VPN helper and its authenticated control protocol.
use blake3::{Hasher as Blake3Hasher, hash as blake3_hash};
use hex::FromHexError;
use iroha_crypto::{
    Algorithm, KeyPair, PublicKey,
    soranet::{
        certificate::{
            is_public_relay_ip, leaf_certificate_spki_sha256, validate_quic_multiaddr,
            validate_tls_server_name,
        },
        handshake::{
            DEFAULT_CLIENT_CAPABILITIES, DEFAULT_RELAY_CAPABILITIES, RelayAuthenticationVerifierV1,
            RuntimeParams, SORANET_QUIC_ALPN, SessionSecrets, build_client_hello,
            client_handle_relay_hello,
        },
        record::{RecordEndpoint, RecordLayer, RecordStreamContext, RecordStreamKind},
    },
};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt as _;
use std::{
    env,
    ffi::OsStr,
    fs,
    future::Future,
    io::{self, Read as _, Write as _},
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
    ffi::{CStr, CString},
    os::fd::{FromRawFd, OwnedFd, RawFd},
    os::unix::ffi::OsStrExt as _,
    process::{Child, ExitStatus},
};
iroha_crypto::define_soranet_record_io_adapters!(soranet_record_io);
use iroha_data_model::soranet::vpn::{
    VPN_DEFAULT_TUNNEL_MTU_BYTES, VPN_HELPER_TICKET_LEN, VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1,
    VPN_USAGE_VOUCHER_CONTROL_MAGIC, VpnCellClassV1, VpnCellError, VpnCellFlagsV1, VpnCellHeaderV1,
    VpnCellV1, VpnFlowLabelV1, VpnHelperTicketV1, VpnPaddedCellV1, VpnUsageVoucherBodyV1,
    VpnUsageVoucherEnvelopeV1, VpnUsageVoucherV1, derive_vpn_session_address_plan_v1,
    vpn_helper_network_policy_hash_v1,
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
#[cfg(target_os = "linux")]
use rand::RngCore;
use rand::{SeedableRng, rngs::StdRng};
use rustls::{
    RootCertStore,
    client::WebPkiServerVerifier,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::CertificateDer,
};
#[cfg(target_os = "linux")]
use soranet_record_io::{RecordReader, RecordWriter};
use thiserror::Error;
use tokio::{
    io::unix::AsyncFd,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::lookup_host,
    signal::unix::{Signal, SignalKind, signal},
    sync::Notify,
    time::timeout,
};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RELAY_DNS_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RELAY_DNS_ANSWERS_V1: usize = 32;
const CONNECT_INPUT_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_POLL_INTERVAL: Duration = Duration::from_millis(100);
// The public process has one finite bound for all worker barriers, privileged preparation, and
// final readiness publication. Privileged preparation has its own shorter absolute bound below;
// route count must never multiply that bound by starting a fresh timeout for each command.
const CONNECT_READY_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
#[cfg(target_os = "linux")]
const VPN_STREAM_FINISH_TIMEOUT: Duration = Duration::from_secs(5);
const SYSTEM_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(target_os = "linux")]
const SYSTEM_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);
#[cfg(target_os = "linux")]
const MAX_SYSTEM_COMMAND_STDOUT_BYTES: usize = 256 * 1024;
#[cfg(target_os = "linux")]
const MAX_SYSTEM_COMMAND_STDERR_BYTES: usize = 64 * 1024;
#[cfg(target_os = "linux")]
const SYSTEM_COMMAND_CGROUP_PATH: &str = "/sys/fs/cgroup/sora-vpn-controller.system-command-v1";
const CONTROLLER_KIND: &str = "linux-helperd";
const PACKET_LEN_PREFIX_BYTES: usize = 2;
const TUNNEL_LAUNCH_FRAME_MAGIC: &[u8; 8] = b"SVPNTUN1";
const TUNNEL_LAUNCH_FRAME_BYTES: usize = 64;
const NETWORK_WORKER_IPC_MAGIC: &[u8; 8] = b"SVPNIPC1";
const NETWORK_WORKER_IPC_VERSION: u8 = 1;
const NETWORK_WORKER_IPC_FRAME_BYTES: usize = 64;
#[cfg(any(target_os = "linux", test))]
const TRAFFIC_ACCOUNTING_PERSIST_INTERVAL: Duration = Duration::from_secs(1);
#[cfg(any(target_os = "linux", test))]
const MAX_TRAFFIC_FRAMES_PER_INTERVAL: u32 = 64;
const NETWORK_WORKER_PLAN_FRAME_BYTES: usize = 8 * 1024;
const NETWORK_WORKER_PLAN_MAGIC: &[u8; 8] = b"SVPNPLN1";
const NETWORK_WORKER_PLAN_VERSION: u8 = 1;
const NETWORK_WORKER_IPC_FD: i32 = 3;
const STATE_FILE_FRAME_MAGIC_V1: &[u8; 8] = b"SVPNST1\0";
const STATE_FILE_FRAME_MAGIC: &[u8; 8] = b"SVPNST2\0";
const STATE_FILE_NAME: &str = "state.norito";
const CONTROLLER_LOCK_FILE_NAME: &str = "controller.lock";
const HELPER_TICKET_ISSUER_PUBLIC_KEY_PATH: &str =
    "/etc/sora-vpn-controller/helper-ticket-issuer-public-key.hex";
#[cfg(target_os = "linux")]
const PINNED_SELF_EXEC_PATH: &str = "/proc/self/exe";
const HELPER_TICKET_ISSUER_PUBLIC_KEY_HEX_BYTES: usize = 64;
// The first-release worker protocol is local-only, but the hidden subcommands can still be
// invoked with an arbitrary pipe. One MiB leaves room for the complete route policy while
// preventing a privileged helper from buffering an unbounded stdin stream.
const MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1: usize = 1024 * 1024;
// Persisted state can additionally contain one captured pre-VPN route for every excluded route.
// Bound it independently so a corrupt state file cannot turn startup/status into an OOM path.
const MAX_STATE_FRAME_BYTES_V1: usize = 8 * 1024 * 1024;
const MAX_STATE_FIELD_BYTES_V1: usize = 64 * 1024;
const MAX_STATE_SEQUENCE_ELEMENTS_V1: usize = 4_096;
const MAX_STATE_TOTAL_ELEMENTS_V1: usize = 8 * 1024 * 1024;
const MAX_STATE_DECODE_ALLOCATION_BYTES_V1: usize = 16 * 1024 * 1024;
const MAX_STATE_DECODE_DEPTH_V1: usize = 16;
const MAX_SESSION_ID_BYTES_V1: usize = 256;
const MAX_RELAY_ENDPOINT_BYTES_V1: usize = 2_048;
const MAX_TLS_SERVER_NAME_BYTES_V1: usize = 253;
const MAX_HELPER_TICKET_HEX_BYTES_V1: usize = 64 * 1024;
const MAX_NETWORK_POLICY_ENTRIES_V1: usize = 4_096;
const MAX_NETWORK_POLICY_ENTRY_BYTES_V1: usize = 256;
const VPN_MAX_ROUTE_ENTRIES_V1: usize = 64;
const VPN_MAX_ROUTE_BYTES_V1: usize = 128;
const VPN_MAX_DNS_ENTRIES_V1: usize = 8;
const VPN_MAX_DNS_BYTES_V1: usize = 64;
const DEFAULT_ROUTE_CMD: &str = "ip";
const DEFAULT_ROUTE_SHOW_PREFIX: [&str; 3] = ["-N", "-o", "route"];
const EXCLUDED_ROUTE_PROTOCOL_V1: &str = "186";
const PLANNED_EXCLUDED_ROUTE_PREFIX_V1: &str = "sora-vpn-planned-route-v1 ";
const USAGE_VOUCHER_INTERVAL: Duration = Duration::from_secs(1);
const USAGE_VOUCHER_BYTE_CREDIT_WINDOW: u64 = 256 * 1024;
const USAGE_VOUCHER_BYTE_REFRESH_THRESHOLD: u64 = USAGE_VOUCHER_BYTE_CREDIT_WINDOW / 2;
const USAGE_VOUCHER_ACTIVE_CREDIT_MS: u64 = 2_000;
const NETWORK_WORKER_READY_TIMEOUT: Duration = Duration::from_secs(30);
const PRIVILEGED_PREPARATION_TIMEOUT: Duration = Duration::from_secs(45);
const NETWORK_WORKER_TUN_TIMEOUT: Duration = Duration::from_secs(60);
const NETWORK_WORKER_STOP_TIMEOUT: Duration = Duration::from_secs(2);
const PROCESS_KILL_REAP_TIMEOUT: Duration = Duration::from_secs(5);
const NETWORK_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(25);
const NETWORK_WORKER_TOKEN_ENV: &str = "SORA_VPN_WORKER_TOKEN_HEX";
const NETWORK_WORKER_ISSUER_ENV: &str = "SORA_VPN_WORKER_ISSUER_KEY_HEX";
static ATOMIC_FILE_NONCE: AtomicU64 = AtomicU64::new(0);
#[cfg(any(target_os = "linux", test))]
const LINUX_IFF_TUN_BITS: u16 = 0x0001;
#[cfg(any(target_os = "linux", test))]
const LINUX_IFF_NO_PI_BITS: u16 = 0x1000;
#[cfg(any(target_os = "linux", test))]
const LINUX_IFF_TUN_EXCL_BITS: u16 = 0x8000;
#[cfg(target_os = "linux")]
const LINUX_TUNSETIFF: nix::libc::c_ulong = 0x4004_54ca;
#[cfg(target_os = "linux")]
const LINUX_TUNGETIFF: nix::libc::c_ulong = 0x8004_54d2;
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
#[derive(Debug)]
struct Cli {
    command: Command,
}
#[derive(Debug)]
enum Command {
    InstallCheck,
    Status,
    Connect,
    Disconnect { session_id: String },
    Repair { session_id: String },
    RunTunnel,
    RunNetworkWorker,
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
    network_worker_identity: Option<WorkerProcessIdentity>,
    owner_uid: Option<u32>,
    session_id: Option<String>,
    relay_endpoint: Option<String>,
    relay_id: Option<[u8; 32]>,
    network_policy_hash: Option<[u8; 32]>,
    ticket_expires_at_ms: Option<u64>,
    applied_network: Option<AppliedNetworkState>,
}

// Decode the original local state layout so an upgrade can still quiesce exact workers and
// restore a privileged network journal. A legacy active state has no authenticated expiry and is
// therefore normalized to repair-required before it can be reported or persisted again.
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
struct StateV1 {
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
    network_worker_identity: Option<WorkerProcessIdentity>,
    owner_uid: Option<u32>,
    session_id: Option<String>,
    relay_endpoint: Option<String>,
    relay_id: Option<[u8; 32]>,
    network_policy_hash: Option<[u8; 32]>,
    applied_network: Option<AppliedNetworkState>,
}

impl From<StateV1> for State {
    fn from(state: StateV1) -> Self {
        Self {
            installed: state.installed,
            active: state.active,
            controller_kind: state.controller_kind,
            interface_name: state.interface_name,
            network_service: state.network_service,
            version: state.version,
            controller_path: state.controller_path,
            repair_required: state.repair_required,
            bytes_in: state.bytes_in,
            bytes_out: state.bytes_out,
            message: state.message,
            worker_identity: state.worker_identity,
            network_worker_identity: state.network_worker_identity,
            owner_uid: state.owner_uid,
            session_id: state.session_id,
            relay_endpoint: state.relay_endpoint,
            relay_id: state.relay_id,
            network_policy_hash: state.network_policy_hash,
            ticket_expires_at_ms: None,
            applied_network: state.applied_network,
        }
    }
}

#[cfg(test)]
impl From<&State> for StateV1 {
    fn from(state: &State) -> Self {
        Self {
            installed: state.installed,
            active: state.active,
            controller_kind: state.controller_kind.clone(),
            interface_name: state.interface_name.clone(),
            network_service: state.network_service.clone(),
            version: state.version.clone(),
            controller_path: state.controller_path.clone(),
            repair_required: state.repair_required,
            bytes_in: state.bytes_in,
            bytes_out: state.bytes_out,
            message: state.message.clone(),
            worker_identity: state.worker_identity.clone(),
            network_worker_identity: state.network_worker_identity.clone(),
            owner_uid: state.owner_uid,
            session_id: state.session_id.clone(),
            relay_endpoint: state.relay_endpoint.clone(),
            relay_id: state.relay_id,
            network_policy_hash: state.network_policy_hash,
            applied_network: state.applied_network.clone(),
        }
    }
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
            network_worker_identity: None,
            owner_uid: None,
            session_id: None,
            relay_endpoint: None,
            relay_id: None,
            network_policy_hash: None,
            ticket_expires_at_ms: None,
            applied_network: None,
        }
    }
}
#[derive(Debug, Clone, Copy, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
enum WorkerRole {
    Tunnel,
    Network,
}
impl WorkerRole {
    #[cfg(any(target_os = "linux", test))]
    const fn subcommand(self) -> &'static str {
        match self {
            Self::Tunnel => "run-tunnel",
            Self::Network => "run-network-worker",
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrivilegedCaller {
    uid: u32,
    gid: u32,
}
#[derive(Clone, Encode, Decode, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
#[norito(decode_from_slice)]
struct ConnectPayload {
    session_id: String,
    relay_endpoint: String,
    helper_ticket_hex: String,
    relay_id_hex: String,
    relay_mldsa65_public_key_hex: String,
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
    metering_private_key_seed_hex: String,
}
#[cfg_attr(test, derive(Debug))]
struct AuthenticatedConnectPayload {
    payload: ConnectPayload,
    ticket: VpnHelperTicketV1,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthenticatedPrivilegedNetworkPlan {
    session_id: String,
    relay_endpoint: String,
    relay_id: [u8; 32],
    network_policy_hash: [u8; 32],
    ticket_expires_at_ms: u64,
    route_pushes: Vec<String>,
    excluded_routes: Vec<String>,
    dns_servers: Vec<String>,
    tunnel_addresses: Vec<String>,
    mtu_bytes: u64,
}
#[cfg(target_os = "linux")]
struct UnprivilegedNetworkWorkerInput {
    token: [u8; 32],
    issuer_public_key: PublicKey,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum NetworkIpcKind {
    WorkerReady = 1,
    TunReady = 2,
    Traffic = 3,
    Stop = 4,
    WorkerExit = 5,
    TunAck = 6,
    Start = 7,
    Started = 8,
    Isolated = 9,
}
impl TryFrom<u8> for NetworkIpcKind {
    type Error = ControllerError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::WorkerReady),
            2 => Ok(Self::TunReady),
            3 => Ok(Self::Traffic),
            4 => Ok(Self::Stop),
            5 => Ok(Self::WorkerExit),
            6 => Ok(Self::TunAck),
            7 => Ok(Self::Start),
            8 => Ok(Self::Started),
            9 => Ok(Self::Isolated),
            _ => Err(ControllerError::State(format!(
                "network-worker IPC frame has unknown kind {value}"
            ))),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NetworkIpcFrame {
    kind: NetworkIpcKind,
    token: [u8; 32],
    value_a: u64,
    value_b: u64,
}
impl NetworkIpcFrame {
    fn new(kind: NetworkIpcKind, token: [u8; 32], value_a: u64, value_b: u64) -> Self {
        Self {
            kind,
            token,
            value_a,
            value_b,
        }
    }

    fn encode(self) -> [u8; NETWORK_WORKER_IPC_FRAME_BYTES] {
        let mut bytes = [0_u8; NETWORK_WORKER_IPC_FRAME_BYTES];
        bytes[..8].copy_from_slice(NETWORK_WORKER_IPC_MAGIC);
        bytes[8] = NETWORK_WORKER_IPC_VERSION;
        bytes[9] = self.kind as u8;
        bytes[16..48].copy_from_slice(&self.token);
        bytes[48..56].copy_from_slice(&self.value_a.to_be_bytes());
        bytes[56..64].copy_from_slice(&self.value_b.to_be_bytes());
        bytes
    }

    fn decode(bytes: &[u8], expected_token: &[u8; 32]) -> Result<Self, ControllerError> {
        if bytes.len() != NETWORK_WORKER_IPC_FRAME_BYTES {
            return Err(ControllerError::State(format!(
                "network-worker IPC datagram must contain exactly {NETWORK_WORKER_IPC_FRAME_BYTES} bytes, got {}",
                bytes.len()
            )));
        }
        if &bytes[..8] != NETWORK_WORKER_IPC_MAGIC
            || bytes[8] != NETWORK_WORKER_IPC_VERSION
            || bytes[10..16] != [0_u8; 6]
        {
            return Err(ControllerError::State(
                "network-worker IPC frame has an invalid magic, version, or reserved field"
                    .to_owned(),
            ));
        }
        let kind = NetworkIpcKind::try_from(bytes[9])?;
        let mut token = [0_u8; 32];
        token.copy_from_slice(&bytes[16..48]);
        if !constant_time_bytes_eq(&token, expected_token) {
            return Err(ControllerError::State(
                "network-worker IPC frame authentication token mismatch".to_owned(),
            ));
        }
        let value_a = u64::from_be_bytes(
            bytes[48..56]
                .try_into()
                .expect("fixed IPC value-a field width"),
        );
        let value_b = u64::from_be_bytes(
            bytes[56..64]
                .try_into()
                .expect("fixed IPC value-b field width"),
        );
        Ok(Self {
            kind,
            token,
            value_a,
            value_b,
        })
    }
}
const PLAN_TOKEN_RANGE: std::ops::Range<usize> = 16..48;
const PLAN_PADDING_OFFSET: usize = 48;
const PLAN_ROUTE_COUNT_OFFSET: usize = 50;
const PLAN_EXCLUDED_COUNT_OFFSET: usize = 51;
const PLAN_DNS_COUNT_OFFSET: usize = 52;
const PLAN_SESSION_RANGE: std::ops::Range<usize> = 64..80;
const PLAN_RELAY_ID_RANGE: std::ops::Range<usize> = 80..112;
const PLAN_DESCRIPTOR_RANGE: std::ops::Range<usize> = 112..144;
const PLAN_TLS_SPKI_RANGE: std::ops::Range<usize> = 144..176;
const PLAN_CERTIFICATE_RANGE: std::ops::Range<usize> = 176..208;
const PLAN_DIRECTORY_RANGE: std::ops::Range<usize> = 208..240;
const PLAN_RELAY_LENGTH_OFFSET: usize = 240;
const PLAN_TLS_NAME_LENGTH_OFFSET: usize = 242;
const PLAN_TICKET_RANGE: std::ops::Range<usize> = 244..244 + VPN_HELPER_TICKET_LEN;
const PLAN_RELAY_RANGE: std::ops::Range<usize> =
    PLAN_TICKET_RANGE.end..PLAN_TICKET_RANGE.end + MAX_RELAY_ENDPOINT_BYTES_V1;
const PLAN_TLS_NAME_RANGE: std::ops::Range<usize> =
    PLAN_RELAY_RANGE.end..PLAN_RELAY_RANGE.end + MAX_TLS_SERVER_NAME_BYTES_V1;
const PLAN_RELAY_MLDSA65_RANGE: std::ops::Range<usize> =
    PLAN_TLS_NAME_RANGE.end..PLAN_TLS_NAME_RANGE.end + VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1;
const PLAN_ROUTE_SLOT_BYTES: usize = 18;
// Keep the variable route region aligned and leave a canonical zero-filled gap after the
// fixed-width ML-DSA-65 identity. The previous hard-coded 4 KiB start overlapped the current
// first-release key width and made the helper fail its compile-time layout assertion.
const PLAN_ROUTE_START: usize = (PLAN_RELAY_MLDSA65_RANGE.end + 63) & !63;
const PLAN_ROUTE_RANGE: std::ops::Range<usize> =
    PLAN_ROUTE_START..PLAN_ROUTE_START + 64 * PLAN_ROUTE_SLOT_BYTES;
const PLAN_EXCLUDED_RANGE: std::ops::Range<usize> =
    PLAN_ROUTE_RANGE.end..PLAN_ROUTE_RANGE.end + 64 * PLAN_ROUTE_SLOT_BYTES;
const PLAN_DNS_SLOT_BYTES: usize = 17;
const PLAN_DNS_RANGE: std::ops::Range<usize> =
    PLAN_EXCLUDED_RANGE.end..PLAN_EXCLUDED_RANGE.end + 8 * PLAN_DNS_SLOT_BYTES;
const _: () = assert!(PLAN_RELAY_MLDSA65_RANGE.end < PLAN_ROUTE_RANGE.start);
const _: () = assert!(PLAN_ROUTE_RANGE.start.is_multiple_of(64));
const _: () = assert!(PLAN_DNS_RANGE.end <= NETWORK_WORKER_PLAN_FRAME_BYTES);

fn encode_plan_cidr(slot: &mut [u8], cidr: ParsedCidr) {
    debug_assert_eq!(slot.len(), PLAN_ROUTE_SLOT_BYTES);
    slot[1] = cidr.prefix;
    match cidr.address {
        IpAddr::V4(address) => {
            slot[0] = 4;
            slot[2..6].copy_from_slice(&address.octets());
        }
        IpAddr::V6(address) => {
            slot[0] = 6;
            slot[2..18].copy_from_slice(&address.octets());
        }
    }
}
fn decode_plan_cidr(slot: &[u8], label: &str, index: usize) -> Result<ParsedCidr, ControllerError> {
    if slot.len() != PLAN_ROUTE_SLOT_BYTES {
        return Err(ControllerError::State(format!(
            "fixed {label}[{index}] slot has the wrong width"
        )));
    }
    let prefix = slot[1];
    let address = match slot[0] {
        4 if prefix <= 32 && slot[6..].iter().all(|byte| *byte == 0) => {
            IpAddr::V4(Ipv4Addr::new(slot[2], slot[3], slot[4], slot[5]))
        }
        6 if prefix <= 128 => {
            let mut octets = [0_u8; 16];
            octets.copy_from_slice(&slot[2..18]);
            IpAddr::V6(Ipv6Addr::from(octets))
        }
        _ => {
            return Err(ControllerError::State(format!(
                "fixed {label}[{index}] has an invalid family, prefix, or reserved field"
            )));
        }
    };
    let parsed = ParsedCidr { address, prefix };
    if network_prefix(parsed) != address {
        return Err(ControllerError::State(format!(
            "fixed {label}[{index}] contains nonzero host bits"
        )));
    }
    Ok(parsed)
}
fn encode_plan_dns(slot: &mut [u8], address: IpAddr) {
    debug_assert_eq!(slot.len(), PLAN_DNS_SLOT_BYTES);
    match address {
        IpAddr::V4(address) => {
            slot[0] = 4;
            slot[1..5].copy_from_slice(&address.octets());
        }
        IpAddr::V6(address) => {
            slot[0] = 6;
            slot[1..17].copy_from_slice(&address.octets());
        }
    }
}
fn decode_plan_dns(slot: &[u8], index: usize) -> Result<IpAddr, ControllerError> {
    if slot.len() != PLAN_DNS_SLOT_BYTES {
        return Err(ControllerError::State(format!(
            "fixed DNS[{index}] slot has the wrong width"
        )));
    }
    let address = match slot[0] {
        4 if slot[5..].iter().all(|byte| *byte == 0) => {
            IpAddr::V4(Ipv4Addr::new(slot[1], slot[2], slot[3], slot[4]))
        }
        6 => {
            let mut octets = [0_u8; 16];
            octets.copy_from_slice(&slot[1..17]);
            IpAddr::V6(Ipv6Addr::from(octets))
        }
        _ => {
            return Err(ControllerError::State(format!(
                "fixed DNS[{index}] has an invalid family or reserved field"
            )));
        }
    };
    if address.is_unspecified()
        || address.is_multicast()
        || matches!(address, IpAddr::V4(address) if address == Ipv4Addr::BROADCAST)
    {
        return Err(ControllerError::State(format!(
            "fixed DNS[{index}] is not a unicast resolver"
        )));
    }
    Ok(address)
}
fn copy_plan_hash(bytes: &[u8], range: std::ops::Range<usize>) -> [u8; 32] {
    bytes[range]
        .try_into()
        .expect("fixed network plan hash width")
}
fn encode_authenticated_network_plan(
    authenticated: &AuthenticatedConnectPayload,
    token: [u8; 32],
) -> Result<WipeArray<NETWORK_WORKER_PLAN_FRAME_BYTES>, ControllerError> {
    let payload = &authenticated.payload;
    if payload.relay_endpoint.is_empty()
        || payload.relay_endpoint.len() > MAX_RELAY_ENDPOINT_BYTES_V1
        || payload.tls_server_name.is_empty()
        || payload.tls_server_name.len() > MAX_TLS_SERVER_NAME_BYTES_V1
        || !(1..=VPN_MAX_ROUTE_ENTRIES_V1).contains(&payload.route_pushes.len())
        || payload.excluded_routes.len() > VPN_MAX_ROUTE_ENTRIES_V1
        || !(1..=VPN_MAX_DNS_ENTRIES_V1).contains(&payload.dns_servers.len())
    {
        return Err(ControllerError::State(
            "authenticated payload does not fit the fixed V1 privileged plan".to_owned(),
        ));
    }
    let mut frame = WipeArray::zeroed();
    frame[..8].copy_from_slice(NETWORK_WORKER_PLAN_MAGIC);
    frame[8] = NETWORK_WORKER_PLAN_VERSION;
    frame[PLAN_TOKEN_RANGE].copy_from_slice(&token);
    frame[PLAN_PADDING_OFFSET..PLAN_PADDING_OFFSET + 2]
        .copy_from_slice(&payload.padding_budget_ms.to_be_bytes());
    frame[PLAN_ROUTE_COUNT_OFFSET] = u8::try_from(payload.route_pushes.len())
        .map_err(|_| ControllerError::State("route count exceeds fixed plan width".to_owned()))?;
    frame[PLAN_EXCLUDED_COUNT_OFFSET] =
        u8::try_from(payload.excluded_routes.len()).map_err(|_| {
            ControllerError::State("excluded-route count exceeds fixed plan width".to_owned())
        })?;
    frame[PLAN_DNS_COUNT_OFFSET] = u8::try_from(payload.dns_servers.len())
        .map_err(|_| ControllerError::State("DNS count exceeds fixed plan width".to_owned()))?;
    let session_id = parse_canonical_session_id(payload.session_id.as_str())?;
    frame[PLAN_SESSION_RANGE].copy_from_slice(&session_id);
    let relay_id = parse_canonical_nonzero_hex_32(payload.relay_id_hex.as_str(), "relayIdHex")?;
    frame[PLAN_RELAY_ID_RANGE].copy_from_slice(&relay_id);
    let relay_mldsa65_public_key = parse_relay_mldsa65_public_key_hex(
        payload.relay_mldsa65_public_key_hex.as_str(),
        "relayMlDsa65PublicKeyHex",
    )?;
    frame[PLAN_RELAY_MLDSA65_RANGE].copy_from_slice(&relay_mldsa65_public_key);
    let descriptor = parse_canonical_nonzero_hex_32(
        payload.descriptor_commit_hex.as_str(),
        "descriptorCommitHex",
    )?;
    frame[PLAN_DESCRIPTOR_RANGE].copy_from_slice(&descriptor);
    let spki = parse_canonical_nonzero_hex_32(
        payload.relay_tls_spki_sha256_hex.as_str(),
        "relayTlsSpkiSha256Hex",
    )?;
    frame[PLAN_TLS_SPKI_RANGE].copy_from_slice(&spki);
    let certificate = parse_canonical_nonzero_hex_32(
        payload.relay_certificate_sha256_hex.as_str(),
        "relayCertificateSha256Hex",
    )?;
    frame[PLAN_CERTIFICATE_RANGE].copy_from_slice(&certificate);
    let directory = parse_canonical_nonzero_hex_32(
        payload.directory_snapshot_digest_hex.as_str(),
        "directorySnapshotDigestHex",
    )?;
    frame[PLAN_DIRECTORY_RANGE].copy_from_slice(&directory);
    let relay_length = u16::try_from(payload.relay_endpoint.len()).map_err(|_| {
        ControllerError::State("relay endpoint exceeds fixed plan length width".to_owned())
    })?;
    let tls_name_length = u16::try_from(payload.tls_server_name.len()).map_err(|_| {
        ControllerError::State("TLS server name exceeds fixed plan length width".to_owned())
    })?;
    frame[PLAN_RELAY_LENGTH_OFFSET..PLAN_RELAY_LENGTH_OFFSET + 2]
        .copy_from_slice(&relay_length.to_be_bytes());
    frame[PLAN_TLS_NAME_LENGTH_OFFSET..PLAN_TLS_NAME_LENGTH_OFFSET + 2]
        .copy_from_slice(&tls_name_length.to_be_bytes());
    frame[PLAN_RELAY_RANGE.start..PLAN_RELAY_RANGE.start + payload.relay_endpoint.len()]
        .copy_from_slice(payload.relay_endpoint.as_bytes());
    frame[PLAN_TLS_NAME_RANGE.start..PLAN_TLS_NAME_RANGE.start + payload.tls_server_name.len()]
        .copy_from_slice(payload.tls_server_name.as_bytes());
    let mut ticket_bytes = WipeArray::<VPN_HELPER_TICKET_LEN>::zeroed();
    hex::decode_to_slice(payload.helper_ticket_hex.as_str(), ticket_bytes.as_mut())?;
    frame[PLAN_TICKET_RANGE].copy_from_slice(ticket_bytes.as_ref());

    for (index, route) in payload.route_pushes.iter().enumerate() {
        let parsed = parse_cidr(route)?;
        let start = PLAN_ROUTE_RANGE.start + index * PLAN_ROUTE_SLOT_BYTES;
        encode_plan_cidr(&mut frame[start..start + PLAN_ROUTE_SLOT_BYTES], parsed);
    }
    for (index, route) in payload.excluded_routes.iter().enumerate() {
        let parsed = parse_cidr(route)?;
        let start = PLAN_EXCLUDED_RANGE.start + index * PLAN_ROUTE_SLOT_BYTES;
        encode_plan_cidr(&mut frame[start..start + PLAN_ROUTE_SLOT_BYTES], parsed);
    }
    for (index, resolver) in payload.dns_servers.iter().enumerate() {
        let address = resolver.parse::<IpAddr>().map_err(|_| {
            ControllerError::State("validated resolver could not be encoded".to_owned())
        })?;
        let start = PLAN_DNS_RANGE.start + index * PLAN_DNS_SLOT_BYTES;
        encode_plan_dns(&mut frame[start..start + PLAN_DNS_SLOT_BYTES], address);
    }
    Ok(frame)
}
fn decode_authenticated_network_plan(
    frame: &[u8],
    expected_token: &[u8; 32],
    issuer_public_key: &PublicKey,
    now_ms: u64,
) -> Result<AuthenticatedPrivilegedNetworkPlan, ControllerError> {
    if frame.len() != NETWORK_WORKER_PLAN_FRAME_BYTES
        || &frame[..8] != NETWORK_WORKER_PLAN_MAGIC
        || frame[8] != NETWORK_WORKER_PLAN_VERSION
        || frame[9..16].iter().any(|byte| *byte != 0)
        || !constant_time_bytes_eq(&frame[PLAN_TOKEN_RANGE], expected_token)
        || frame[53..64].iter().any(|byte| *byte != 0)
    {
        return Err(ControllerError::State(
            "network-worker fixed plan has invalid framing or authentication".to_owned(),
        ));
    }
    let route_count = usize::from(frame[PLAN_ROUTE_COUNT_OFFSET]);
    let excluded_count = usize::from(frame[PLAN_EXCLUDED_COUNT_OFFSET]);
    let dns_count = usize::from(frame[PLAN_DNS_COUNT_OFFSET]);
    if !(1..=VPN_MAX_ROUTE_ENTRIES_V1).contains(&route_count)
        || excluded_count > VPN_MAX_ROUTE_ENTRIES_V1
        || !(1..=VPN_MAX_DNS_ENTRIES_V1).contains(&dns_count)
    {
        return Err(ControllerError::State(
            "network-worker fixed plan violates V1 cardinality".to_owned(),
        ));
    }
    let padding_budget_ms = u16::from_be_bytes(
        frame[PLAN_PADDING_OFFSET..PLAN_PADDING_OFFSET + 2]
            .try_into()
            .expect("fixed padding field"),
    );
    if padding_budget_ms == 0 {
        return Err(ControllerError::State(
            "network-worker fixed plan has zero padding budget".to_owned(),
        ));
    }
    let relay_length = usize::from(u16::from_be_bytes(
        frame[PLAN_RELAY_LENGTH_OFFSET..PLAN_RELAY_LENGTH_OFFSET + 2]
            .try_into()
            .expect("fixed relay length"),
    ));
    let tls_name_length = usize::from(u16::from_be_bytes(
        frame[PLAN_TLS_NAME_LENGTH_OFFSET..PLAN_TLS_NAME_LENGTH_OFFSET + 2]
            .try_into()
            .expect("fixed TLS-name length"),
    ));
    if relay_length == 0
        || relay_length > MAX_RELAY_ENDPOINT_BYTES_V1
        || tls_name_length == 0
        || tls_name_length > MAX_TLS_SERVER_NAME_BYTES_V1
        || frame[PLAN_RELAY_RANGE.start + relay_length..PLAN_RELAY_RANGE.end]
            .iter()
            .any(|byte| *byte != 0)
        || frame[PLAN_TLS_NAME_RANGE.start + tls_name_length..PLAN_TLS_NAME_RANGE.end]
            .iter()
            .any(|byte| *byte != 0)
        || frame[PLAN_RELAY_MLDSA65_RANGE.end..PLAN_ROUTE_RANGE.start]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(ControllerError::State(
            "network-worker fixed plan has invalid string bounds or padding".to_owned(),
        ));
    }
    let relay_endpoint =
        std::str::from_utf8(&frame[PLAN_RELAY_RANGE.start..PLAN_RELAY_RANGE.start + relay_length])
            .map_err(|_| ControllerError::State("fixed relay endpoint is not UTF-8".to_owned()))?
            .to_owned();
    let tls_server_name = std::str::from_utf8(
        &frame[PLAN_TLS_NAME_RANGE.start..PLAN_TLS_NAME_RANGE.start + tls_name_length],
    )
    .map_err(|_| ControllerError::State("fixed TLS server name is not UTF-8".to_owned()))?
    .to_owned();

    let mut route_pushes = Vec::with_capacity(route_count);
    for index in 0..route_count {
        let start = PLAN_ROUTE_RANGE.start + index * PLAN_ROUTE_SLOT_BYTES;
        route_pushes.push(decode_plan_cidr(
            &frame[start..start + PLAN_ROUTE_SLOT_BYTES],
            "route",
            index,
        )?);
    }
    let used_route_end = PLAN_ROUTE_RANGE.start + route_count * PLAN_ROUTE_SLOT_BYTES;
    if frame[used_route_end..PLAN_ROUTE_RANGE.end]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ControllerError::State(
            "network-worker fixed plan has nonzero unused route slots".to_owned(),
        ));
    }
    let mut excluded_routes = Vec::with_capacity(excluded_count);
    for index in 0..excluded_count {
        let start = PLAN_EXCLUDED_RANGE.start + index * PLAN_ROUTE_SLOT_BYTES;
        excluded_routes.push(decode_plan_cidr(
            &frame[start..start + PLAN_ROUTE_SLOT_BYTES],
            "excluded route",
            index,
        )?);
    }
    let used_excluded_end = PLAN_EXCLUDED_RANGE.start + excluded_count * PLAN_ROUTE_SLOT_BYTES;
    if frame[used_excluded_end..PLAN_EXCLUDED_RANGE.end]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ControllerError::State(
            "network-worker fixed plan has nonzero unused excluded-route slots".to_owned(),
        ));
    }
    if route_pushes.iter().enumerate().any(|(index, route)| {
        route_pushes[..index].contains(route) || excluded_routes.contains(route)
    }) || excluded_routes
        .iter()
        .enumerate()
        .any(|(index, route)| excluded_routes[..index].contains(route))
    {
        return Err(ControllerError::State(
            "network-worker fixed plan has duplicate or equal include/exclude routes".to_owned(),
        ));
    }
    let mut dns_servers = Vec::with_capacity(dns_count);
    for index in 0..dns_count {
        let start = PLAN_DNS_RANGE.start + index * PLAN_DNS_SLOT_BYTES;
        let address = decode_plan_dns(&frame[start..start + PLAN_DNS_SLOT_BYTES], index)?;
        if dns_servers.contains(&address) {
            return Err(ControllerError::State(
                "network-worker fixed plan has duplicate DNS resolvers".to_owned(),
            ));
        }
        dns_servers.push(address);
    }
    let used_dns_end = PLAN_DNS_RANGE.start + dns_count * PLAN_DNS_SLOT_BYTES;
    if frame[used_dns_end..].iter().any(|byte| *byte != 0) {
        return Err(ControllerError::State(
            "network-worker fixed plan has nonzero unused DNS or tail bytes".to_owned(),
        ));
    }

    let session_id: [u8; 16] = frame[PLAN_SESSION_RANGE]
        .try_into()
        .expect("fixed plan session width");
    let relay_id = copy_plan_hash(frame, PLAN_RELAY_ID_RANGE);
    let relay_mldsa65_public_key: [u8; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1] = frame
        [PLAN_RELAY_MLDSA65_RANGE]
        .try_into()
        .expect("fixed plan ML-DSA-65 public key width");
    PublicKey::from_bytes(Algorithm::MlDsa, &relay_mldsa65_public_key).map_err(|error| {
        ControllerError::State(format!(
            "network-worker fixed plan has an invalid ML-DSA-65 relay identity: {error}"
        ))
    })?;
    let descriptor_commit = copy_plan_hash(frame, PLAN_DESCRIPTOR_RANGE);
    let relay_tls_spki_sha256 = copy_plan_hash(frame, PLAN_TLS_SPKI_RANGE);
    let relay_certificate_sha256 = copy_plan_hash(frame, PLAN_CERTIFICATE_RANGE);
    let directory_snapshot_digest = copy_plan_hash(frame, PLAN_DIRECTORY_RANGE);
    if [
        relay_id,
        descriptor_commit,
        relay_tls_spki_sha256,
        relay_certificate_sha256,
        directory_snapshot_digest,
    ]
    .iter()
    .any(|value| *value == [0_u8; 32])
    {
        return Err(ControllerError::State(
            "network-worker fixed plan contains an all-zero trust digest".to_owned(),
        ));
    }
    let mut ticket_bytes = WipeArray::<VPN_HELPER_TICKET_LEN>::zeroed();
    ticket_bytes.copy_from_slice(&frame[PLAN_TICKET_RANGE]);
    let ticket = VpnHelperTicketV1::parse(ticket_bytes.as_ref(), issuer_public_key, now_ms)
        .map_err(|error| ControllerError::InvalidPayload(format!("helper ticket: {error}")))?;
    if ticket.session_id != session_id || ticket.relay_id != relay_id {
        return Err(ControllerError::State(
            "network-worker fixed plan identity does not match its signed ticket".to_owned(),
        ));
    }
    let route_pushes = route_pushes
        .into_iter()
        .map(|route| format!("{}/{}", route.address, route.prefix))
        .collect::<Vec<_>>();
    let excluded_routes = excluded_routes
        .into_iter()
        .map(|route| format!("{}/{}", route.address, route.prefix))
        .collect::<Vec<_>>();
    let dns_servers = dns_servers
        .into_iter()
        .map(|address| address.to_string())
        .collect::<Vec<_>>();
    let address_plan = derive_vpn_session_address_plan_v1(session_id);
    let computed_policy_hash = vpn_helper_network_policy_hash_v1(
        relay_endpoint.as_str(),
        &relay_id,
        &relay_mldsa65_public_key,
        &descriptor_commit,
        tls_server_name.as_str(),
        &relay_tls_spki_sha256,
        &relay_certificate_sha256,
        &directory_snapshot_digest,
        padding_budget_ms,
        &route_pushes,
        &excluded_routes,
        &dns_servers,
        &address_plan.client_tunnel_addresses,
        u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES),
    );
    if !constant_time_bytes_eq(&computed_policy_hash, &ticket.network_policy_hash) {
        return Err(ControllerError::State(
            "network-worker fixed plan does not match the signed network policy hash".to_owned(),
        ));
    }
    Ok(AuthenticatedPrivilegedNetworkPlan {
        session_id: hex::encode(session_id),
        relay_endpoint,
        relay_id,
        network_policy_hash: ticket.network_policy_hash,
        ticket_expires_at_ms: ticket.expires_at_ms,
        route_pushes,
        excluded_routes,
        dns_servers,
        tunnel_addresses: address_plan.client_tunnel_addresses,
        mtu_bytes: u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES),
    })
}
fn constant_time_bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupervisorIpcPhase {
    AwaitingReady,
    Ready,
    AwaitingTunAck,
    TunValidated,
    AwaitingStarted,
    Running,
    StopSent,
    Exited,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug)]
struct WorkerTrafficAccounting {
    previous_ingress: u64,
    previous_egress: u64,
    dirty: bool,
    frames_in_interval: u32,
    frame_interval_ends_at: tokio::time::Instant,
    next_persist_at: tokio::time::Instant,
}

#[cfg(any(target_os = "linux", test))]
impl WorkerTrafficAccounting {
    fn new(
        previous_ingress: u64,
        previous_egress: u64,
        now: tokio::time::Instant,
    ) -> Result<Self, ControllerError> {
        let interval_end = now
            .checked_add(TRAFFIC_ACCOUNTING_PERSIST_INTERVAL)
            .ok_or_else(|| {
                ControllerError::State(
                    "traffic-accounting interval exceeds the monotonic clock range".to_owned(),
                )
            })?;
        Ok(Self {
            previous_ingress,
            previous_egress,
            dirty: false,
            frames_in_interval: 0,
            frame_interval_ends_at: interval_end,
            next_persist_at: interval_end,
        })
    }

    fn observe_at(
        &mut self,
        state: &mut State,
        ingress: u64,
        egress: u64,
        now: tokio::time::Instant,
        wall_now_ms: u64,
    ) -> Result<(), ControllerError> {
        if now >= self.frame_interval_ends_at {
            self.frames_in_interval = 0;
            self.frame_interval_ends_at = now
                .checked_add(TRAFFIC_ACCOUNTING_PERSIST_INTERVAL)
                .ok_or_else(|| {
                    ControllerError::State(
                        "traffic-accounting interval exceeds the monotonic clock range".to_owned(),
                    )
                })?;
        }
        if self.frames_in_interval >= MAX_TRAFFIC_FRAMES_PER_INTERVAL {
            return Err(ControllerError::State(format!(
                "network worker exceeded the authenticated TRAFFIC frame ceiling of {MAX_TRAFFIC_FRAMES_PER_INTERVAL} per accounting interval"
            )));
        }
        self.frames_in_interval += 1;
        if ingress < self.previous_ingress || egress < self.previous_egress {
            return Err(ControllerError::State(
                "network-worker traffic counters moved backwards".to_owned(),
            ));
        }
        self.previous_ingress = ingress;
        self.previous_egress = egress;
        let counters_changed = state.bytes_out != ingress || state.bytes_in != egress;
        // Preserve the public state convention: bytes_out is client-to-relay ingress and bytes_in
        // is relay-to-client egress. The root supervisor alone owns this in-memory accumulator.
        state.bytes_out = ingress;
        state.bytes_in = egress;
        self.dirty |= counters_changed;
        self.apply_expiry_at(state, wall_now_ms);
        Ok(())
    }

    fn apply_expiry_at(&mut self, state: &mut State, wall_now_ms: u64) {
        let was_active = state.active;
        let required_repair = state.repair_required;
        demote_expired_active_state_at(state, wall_now_ms);
        self.dirty |= was_active != state.active || required_repair != state.repair_required;
    }

    fn flush_if_due_with<F>(
        &mut self,
        state: &State,
        now: tokio::time::Instant,
        mut persist: F,
    ) -> Result<bool, ControllerError>
    where
        F: FnMut(&State) -> Result<(), ControllerError>,
    {
        if now < self.next_persist_at {
            return Ok(false);
        }
        let next_persist_at = now
            .checked_add(TRAFFIC_ACCOUNTING_PERSIST_INTERVAL)
            .ok_or_else(|| {
                ControllerError::State(
                    "traffic-accounting persistence interval exceeds the monotonic clock range"
                        .to_owned(),
                )
            })?;
        if !self.dirty {
            self.next_persist_at = next_persist_at;
            return Ok(false);
        }
        persist(state)?;
        self.dirty = false;
        self.next_persist_at = next_persist_at;
        Ok(true)
    }

    fn force_flush_with<F>(
        &mut self,
        state: &State,
        mut persist: F,
    ) -> Result<bool, ControllerError>
    where
        F: FnMut(&State) -> Result<(), ControllerError>,
    {
        if !self.dirty {
            return Ok(false);
        }
        persist(state)?;
        self.dirty = false;
        Ok(true)
    }

    #[cfg(target_os = "linux")]
    fn flush_if_due(
        &mut self,
        state: &State,
        now: tokio::time::Instant,
    ) -> Result<bool, ControllerError> {
        self.flush_if_due_with(state, now, persist_state)
    }

    #[cfg(target_os = "linux")]
    fn force_flush(&mut self, state: &State) -> Result<bool, ControllerError> {
        self.force_flush_with(state, persist_state)
    }
}

#[cfg(any(target_os = "linux", test))]
fn finish_worker_traffic_accounting(
    outcome: Result<u64, ControllerError>,
    flush: Result<bool, ControllerError>,
) -> Result<u64, ControllerError> {
    match (outcome, flush) {
        (Ok(code), Ok(_)) => Ok(code),
        (Err(error), Ok(_)) => Err(error),
        (Ok(_), Err(flush_error)) => Err(ControllerError::State(format!(
            "failed to flush final network-worker traffic counters: {flush_error}"
        ))),
        (Err(error), Err(flush_error)) => Err(ControllerError::State(format!(
            "{error}; failed to flush final network-worker traffic counters: {flush_error}"
        ))),
    }
}

#[cfg(target_os = "linux")]
fn force_flush_worker_traffic_accounting(
    accounting: &mut WorkerTrafficAccounting,
    state: &mut State,
) -> Result<bool, ControllerError> {
    let clock_result = unix_now_ms();
    if let Ok(now_ms) = clock_result.as_ref() {
        accounting.apply_expiry_at(state, *now_ms);
    }
    let flush_result = accounting.force_flush(state);
    match (clock_result, flush_result) {
        (Ok(_), result) => result,
        (Err(clock_error), Ok(_)) => Err(clock_error),
        (Err(clock_error), Err(flush_error)) => Err(ControllerError::State(format!(
            "{clock_error}; final traffic-counter persistence also failed: {flush_error}"
        ))),
    }
}

fn validate_supervisor_received_frame(
    phase: SupervisorIpcPhase,
    frame: NetworkIpcFrame,
    fd_count: usize,
) -> Result<SupervisorIpcPhase, ControllerError> {
    let next = match (phase, frame.kind, fd_count) {
        (SupervisorIpcPhase::AwaitingReady, NetworkIpcKind::WorkerReady, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            SupervisorIpcPhase::Ready
        }
        (SupervisorIpcPhase::AwaitingReady, NetworkIpcKind::Isolated, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            phase
        }
        (SupervisorIpcPhase::AwaitingReady, NetworkIpcKind::WorkerExit, 0) => {
            SupervisorIpcPhase::Exited
        }
        (SupervisorIpcPhase::AwaitingTunAck, NetworkIpcKind::TunAck, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            SupervisorIpcPhase::TunValidated
        }
        (SupervisorIpcPhase::AwaitingTunAck, NetworkIpcKind::WorkerExit, 0)
        | (SupervisorIpcPhase::TunValidated, NetworkIpcKind::WorkerExit, 0) => {
            SupervisorIpcPhase::Exited
        }
        (SupervisorIpcPhase::AwaitingStarted, NetworkIpcKind::Started, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            SupervisorIpcPhase::Running
        }
        (SupervisorIpcPhase::AwaitingStarted, NetworkIpcKind::WorkerExit, 0) => {
            SupervisorIpcPhase::Exited
        }
        (SupervisorIpcPhase::Running, NetworkIpcKind::Traffic, 0)
        | (SupervisorIpcPhase::StopSent, NetworkIpcKind::Traffic, 0) => phase,
        (SupervisorIpcPhase::StopSent, NetworkIpcKind::WorkerReady, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            phase
        }
        (SupervisorIpcPhase::StopSent, NetworkIpcKind::TunAck, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            phase
        }
        (SupervisorIpcPhase::StopSent, NetworkIpcKind::Started, 0)
        | (SupervisorIpcPhase::StopSent, NetworkIpcKind::Isolated, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            phase
        }
        (SupervisorIpcPhase::Running, NetworkIpcKind::WorkerExit, 0)
        | (SupervisorIpcPhase::StopSent, NetworkIpcKind::WorkerExit, 0) => {
            SupervisorIpcPhase::Exited
        }
        _ => {
            return Err(ControllerError::State(format!(
                "network-worker IPC frame {:?} with {fd_count} descriptors is invalid in supervisor phase {phase:?}",
                frame.kind
            )));
        }
    };
    Ok(next)
}
fn validate_supervisor_sent_frame(
    phase: SupervisorIpcPhase,
    frame: NetworkIpcFrame,
    fd_count: usize,
) -> Result<SupervisorIpcPhase, ControllerError> {
    match (phase, frame.kind, fd_count) {
        (SupervisorIpcPhase::Ready, NetworkIpcKind::TunReady, 1)
            if frame.value_a > 0 && frame.value_b == 0 =>
        {
            Ok(SupervisorIpcPhase::AwaitingTunAck)
        }
        (SupervisorIpcPhase::AwaitingReady, NetworkIpcKind::Stop, 0)
        | (SupervisorIpcPhase::Ready, NetworkIpcKind::Stop, 0)
        | (SupervisorIpcPhase::AwaitingTunAck, NetworkIpcKind::Stop, 0)
        | (SupervisorIpcPhase::TunValidated, NetworkIpcKind::Stop, 0)
        | (SupervisorIpcPhase::AwaitingStarted, NetworkIpcKind::Stop, 0)
        | (SupervisorIpcPhase::Running, NetworkIpcKind::Stop, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            Ok(SupervisorIpcPhase::StopSent)
        }
        (SupervisorIpcPhase::TunValidated, NetworkIpcKind::Start, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            Ok(SupervisorIpcPhase::AwaitingStarted)
        }
        _ => Err(ControllerError::State(format!(
            "supervisor IPC frame {:?} with {fd_count} descriptors is invalid in phase {phase:?}",
            frame.kind
        ))),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerIpcPhase {
    Connecting,
    Ready,
    ValidatingTun,
    AwaitingStart,
    Starting,
    Running,
    Stopping,
    Exited,
}
fn validate_worker_sent_frame(
    phase: WorkerIpcPhase,
    frame: NetworkIpcFrame,
    fd_count: usize,
) -> Result<WorkerIpcPhase, ControllerError> {
    match (phase, frame.kind, fd_count) {
        (WorkerIpcPhase::Connecting, NetworkIpcKind::WorkerReady, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            Ok(WorkerIpcPhase::Ready)
        }
        (WorkerIpcPhase::Connecting, NetworkIpcKind::Isolated, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            Ok(phase)
        }
        (WorkerIpcPhase::Connecting, NetworkIpcKind::WorkerExit, 0)
        | (WorkerIpcPhase::Ready, NetworkIpcKind::WorkerExit, 0)
        | (WorkerIpcPhase::ValidatingTun, NetworkIpcKind::WorkerExit, 0)
        | (WorkerIpcPhase::AwaitingStart, NetworkIpcKind::WorkerExit, 0)
        | (WorkerIpcPhase::Starting, NetworkIpcKind::WorkerExit, 0)
        | (WorkerIpcPhase::Running, NetworkIpcKind::WorkerExit, 0)
        | (WorkerIpcPhase::Stopping, NetworkIpcKind::WorkerExit, 0) => Ok(WorkerIpcPhase::Exited),
        (WorkerIpcPhase::Running, NetworkIpcKind::Traffic, 0)
        | (WorkerIpcPhase::Stopping, NetworkIpcKind::Traffic, 0) => Ok(phase),
        (WorkerIpcPhase::ValidatingTun, NetworkIpcKind::TunAck, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            Ok(WorkerIpcPhase::AwaitingStart)
        }
        (WorkerIpcPhase::Starting, NetworkIpcKind::Started, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            Ok(WorkerIpcPhase::Running)
        }
        _ => Err(ControllerError::State(format!(
            "network worker IPC frame {:?} with {fd_count} descriptors is invalid in phase {phase:?}",
            frame.kind
        ))),
    }
}
fn validate_worker_received_frame(
    phase: WorkerIpcPhase,
    frame: NetworkIpcFrame,
    fd_count: usize,
) -> Result<WorkerIpcPhase, ControllerError> {
    match (phase, frame.kind, fd_count) {
        (WorkerIpcPhase::Ready, NetworkIpcKind::TunReady, 1)
            if frame.value_a > 0 && frame.value_b == 0 =>
        {
            Ok(WorkerIpcPhase::ValidatingTun)
        }
        (WorkerIpcPhase::Ready, NetworkIpcKind::Stop, 0)
        | (WorkerIpcPhase::ValidatingTun, NetworkIpcKind::Stop, 0)
        | (WorkerIpcPhase::AwaitingStart, NetworkIpcKind::Stop, 0)
        | (WorkerIpcPhase::Running, NetworkIpcKind::Stop, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            Ok(WorkerIpcPhase::Stopping)
        }
        (WorkerIpcPhase::AwaitingStart, NetworkIpcKind::Start, 0)
            if frame.value_a == 0 && frame.value_b == 0 =>
        {
            Ok(WorkerIpcPhase::Starting)
        }
        _ => Err(ControllerError::State(format!(
            "supervisor IPC frame {:?} with {fd_count} descriptors is invalid in worker phase {phase:?}",
            frame.kind
        ))),
    }
}
impl ConnectPayload {
    fn wipe_credentials(&mut self) {
        wipe_secret_string(&mut self.helper_ticket_hex);
        wipe_secret_string(&mut self.metering_private_key_seed_hex);
    }
}
impl Drop for ConnectPayload {
    fn drop(&mut self) {
        self.wipe_credentials();
    }
}
fn wipe_secret_string(secret: &mut String) {
    let mut bytes = core::mem::take(secret).into_bytes();
    wipe_secret_vec(&mut bytes);
}
fn wipe_secret_bytes(secret: &mut [u8]) {
    secret.fill(0);
    std::hint::black_box(secret);
}
fn wipe_secret_vec(secret: &mut Vec<u8>) {
    let allocation_len = secret.capacity();
    secret.resize(allocation_len, 0);
    wipe_secret_bytes(secret);
    secret.clear();
    std::hint::black_box(secret);
}
struct WipeArray<const N: usize>([u8; N]);
impl<const N: usize> WipeArray<N> {
    const fn zeroed() -> Self {
        Self([0; N])
    }

    fn clear(&mut self) {
        wipe_secret_bytes(&mut self.0);
    }
}
#[cfg(test)]
impl<const N: usize> Clone for WipeArray<N> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<const N: usize> std::ops::Deref for WipeArray<N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<const N: usize> std::ops::DerefMut for WipeArray<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<const N: usize> AsRef<[u8]> for WipeArray<N> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl<const N: usize> AsMut<[u8]> for WipeArray<N> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
impl<const N: usize> std::fmt::Debug for WipeArray<N> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "WipeArray(<redacted {N}-byte buffer>)")
    }
}
impl<const N: usize> Drop for WipeArray<N> {
    fn drop(&mut self) {
        self.clear();
    }
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
        wipe_secret_vec(&mut self.0);
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
    journal_phase: NetworkJournalPhase,
    dns_backend: Option<DnsBackendState>,
    excluded_route_snapshots: Vec<ExcludedRouteSnapshot>,
}
#[derive(Debug, Clone, Copy, Encode, Decode, PartialEq, Eq, Default)]
#[norito(decode_from_slice)]
enum NetworkJournalPhase {
    #[default]
    Planned,
    TunCreated,
    LinkConfigured,
    RoutesConfigured,
    ConfiguringExcludedRoutes,
    ExcludedRoutesConfigured,
    DnsPlanned,
    Prepared,
    CleaningDns,
    CleaningRoutes,
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
enum DnsBackendState {
    Resolved { interface_name: String },
    ResolvedReverted { interface_name: String },
}
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
#[norito(decode_from_slice)]
struct ExcludedRouteSnapshot {
    cidr: String,
    family: IpFamily,
    /// Durable ownership proof for the helper-installed exclusion.
    ///
    /// Before mutation this stores a versioned canonical route tuple. After mutation it stores the
    /// exact `ip -o route` readback. Keeping both forms in the existing string field preserves the
    /// v1 state-frame layout while closing the post-add, pre-fsync recovery gap. `None` is accepted
    /// only for legacy state and can be cleaned safely only when the exact prefix is absent.
    installed_route: Option<String>,
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
struct PreparedTunnel<D = Arc<LinuxTunDevice>> {
    device: D,
    interface_name: String,
    network_service: Option<String>,
    applied_network: AppliedNetworkState,
    packet_read_mtu: usize,
}
struct CleanupGuard<T> {
    value: Option<T>,
    cleanup: fn(T) -> Result<(), ControllerError>,
}
impl<T> CleanupGuard<T> {
    fn new(value: T, cleanup: fn(T) -> Result<(), ControllerError>) -> Self {
        Self {
            value: Some(value),
            cleanup,
        }
    }

    fn get(&self) -> &T {
        self.value
            .as_ref()
            .expect("cleanup guard value is present until cleanup")
    }

    fn take(mut self) -> T {
        self.value
            .take()
            .expect("cleanup guard value is present until it is taken")
    }
}
impl<T> Drop for CleanupGuard<T> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            let _ = (self.cleanup)(value);
        }
    }
}
#[cfg(target_os = "linux")]
struct NetworkWorkerProcess {
    child: Child,
    identity: WorkerProcessIdentity,
    pidfd: OwnedFd,
    reaped_status: Option<ExitStatus>,
}
#[cfg(target_os = "linux")]
impl NetworkWorkerProcess {
    fn is_reaped(&self) -> bool {
        self.reaped_status.is_some()
    }

    fn poll_exit(&mut self) -> Result<Option<ExitStatus>, ControllerError> {
        if let Some(status) = self.reaped_status {
            return Ok(Some(status));
        }
        let status = self.child.try_wait()?;
        if let Some(status) = status {
            self.reaped_status = Some(status);
        }
        Ok(status)
    }

    fn exact_identity_alive(&mut self) -> Result<bool, ControllerError> {
        if self.poll_exit()?.is_some() {
            return Ok(false);
        }
        worker_identity_alive(&self.identity)
    }

    async fn stop_and_reap(
        &mut self,
        timeout_limit: Duration,
    ) -> Result<ExitStatus, ControllerError> {
        if let Some(status) = self.poll_exit()? {
            return Ok(status);
        }
        if !pidfd_send_signal(&self.pidfd, nix::libc::SIGTERM)? {
            return Err(ControllerError::State(
                "unprivileged network worker exited before termination signal".to_owned(),
            ));
        }
        let deadline = tokio::time::Instant::now() + timeout_limit;
        loop {
            if let Some(status) = self.poll_exit()? {
                return Ok(status);
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(NETWORK_WORKER_POLL_INTERVAL).await;
        }
        let _ = pidfd_send_signal(&self.pidfd, nix::libc::SIGKILL)?;
        let kill_deadline = tokio::time::Instant::now() + PROCESS_KILL_REAP_TIMEOUT;
        loop {
            if let Some(status) = self.poll_exit()? {
                return Ok(status);
            }
            if tokio::time::Instant::now() >= kill_deadline {
                return Err(ControllerError::State(format!(
                    "unprivileged network worker {} did not exit after exact pidfd SIGKILL; retaining its persisted identity and journal without cleanup",
                    self.identity.pid
                )));
            }
            tokio::time::sleep(NETWORK_WORKER_POLL_INTERVAL).await;
        }
    }

    async fn reap_after_protocol_exit(&mut self) -> Result<ExitStatus, ControllerError> {
        let deadline = tokio::time::Instant::now() + NETWORK_WORKER_STOP_TIMEOUT;
        loop {
            if let Some(status) = self.poll_exit()? {
                return Ok(status);
            }
            if tokio::time::Instant::now() >= deadline {
                return self.stop_and_reap(Duration::ZERO).await;
            }
            tokio::time::sleep(NETWORK_WORKER_POLL_INTERVAL).await;
        }
    }
}
#[cfg(target_os = "linux")]
impl Drop for NetworkWorkerProcess {
    fn drop(&mut self) {
        if self.reaped_status.is_none() {
            let _ = pidfd_send_signal(&self.pidfd, nix::libc::SIGKILL);
            if let Ok(status) =
                kill_and_reap_direct_child_bounded(&mut self.child, "unprivileged network worker")
            {
                self.reaped_status = Some(status);
            }
        }
    }
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
    authorized_ingress_bytes: Arc<AtomicU64>,
    authorized_egress_bytes: Arc<AtomicU64>,
    refresh_notify: Arc<Notify>,
}
impl UsageVoucherCounters {
    fn add_ingress(&self, bytes: u64) {
        let _ = self
            .ingress_bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_add(bytes))
            });
    }
    fn add_egress(&self, bytes: u64) {
        let _ = self
            .egress_bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_add(bytes))
            });
    }
    /// Record an IP packet uploaded by the client toward the relay.
    fn record_client_to_relay(&self, bytes: u64) {
        self.add_ingress(bytes);
    }
    /// Record an IP packet downloaded from the relay toward the client.
    fn record_relay_to_client(&self, bytes: u64) {
        self.add_egress(bytes);
        if self.remaining_egress_credit() <= USAGE_VOUCHER_BYTE_REFRESH_THRESHOLD {
            self.refresh_notify.notify_one();
        }
    }
    fn snapshot(&self) -> (u64, u64) {
        (
            self.ingress_bytes.load(Ordering::Relaxed),
            self.egress_bytes.load(Ordering::Relaxed),
        )
    }
    fn set_authorization(&self, ingress_bytes: u64, egress_bytes: u64) {
        self.authorized_ingress_bytes
            .store(ingress_bytes, Ordering::Release);
        self.authorized_egress_bytes
            .store(egress_bytes, Ordering::Release);
    }
    fn remaining_ingress_credit(&self) -> u64 {
        self.authorized_ingress_bytes
            .load(Ordering::Acquire)
            .saturating_sub(self.ingress_bytes.load(Ordering::Acquire))
    }
    fn remaining_egress_credit(&self) -> u64 {
        self.authorized_egress_bytes
            .load(Ordering::Acquire)
            .saturating_sub(self.egress_bytes.load(Ordering::Acquire))
    }
    fn refresh_before_ingress(&self, packet_bytes: u64) -> bool {
        self.remaining_ingress_credit()
            < USAGE_VOUCHER_BYTE_REFRESH_THRESHOLD.saturating_add(packet_bytes)
    }
    async fn refresh_requested(&self) {
        self.refresh_notify.notified().await;
    }
}
struct UsageVoucherSigner {
    key_pair: KeyPair,
    ticket: VpnHelperTicketV1,
    sequence: u64,
    started_at: Instant,
    interval: Duration,
    authorized_active_ms: u64,
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
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NetworkPeerCredentials {
    pid: u32,
    uid: u32,
    gid: u32,
}
#[cfg(target_os = "linux")]
struct ReceivedNetworkIpc {
    frame: NetworkIpcFrame,
    descriptors: Vec<OwnedFd>,
}
#[cfg(target_os = "linux")]
struct NetworkIpcSocket {
    fd: AsyncFd<OwnedFd>,
}
#[cfg(target_os = "linux")]
impl NetworkIpcSocket {
    fn new(fd: OwnedFd) -> Result<Self, ControllerError> {
        Ok(Self {
            fd: AsyncFd::new(fd)?,
        })
    }

    async fn send(
        &self,
        frame: NetworkIpcFrame,
        descriptor: Option<RawFd>,
    ) -> Result<(), ControllerError> {
        let bytes = frame.encode();
        loop {
            let mut guard = self.fd.writable().await?;
            match guard.try_io(|inner| {
                send_network_ipc_once(inner.get_ref().as_raw_fd(), &bytes, descriptor)
            }) {
                Ok(result) => return result,
                Err(_) => continue,
            }
        }
    }

    async fn receive(
        &self,
        expected_token: &[u8; 32],
        expected_peer: NetworkPeerCredentials,
    ) -> Result<ReceivedNetworkIpc, ControllerError> {
        loop {
            let mut guard = self.fd.readable().await?;
            match guard.try_io(|inner| {
                receive_network_ipc_once(inner.get_ref().as_raw_fd(), expected_token, expected_peer)
            }) {
                Ok(result) => return result,
                Err(_) => continue,
            }
        }
    }

    async fn send_plan(
        &self,
        frame: &WipeArray<NETWORK_WORKER_PLAN_FRAME_BYTES>,
    ) -> Result<(), ControllerError> {
        loop {
            let mut guard = self.fd.writable().await?;
            match guard.try_io(|inner| send_network_plan_once(inner.get_ref().as_raw_fd(), frame)) {
                Ok(result) => return result.map_err(Into::into),
                Err(_) => continue,
            }
        }
    }

    async fn receive_plan(
        &self,
        expected_peer: NetworkPeerCredentials,
    ) -> Result<WipeArray<NETWORK_WORKER_PLAN_FRAME_BYTES>, ControllerError> {
        loop {
            let mut guard = self.fd.readable().await?;
            match guard.try_io(|inner| {
                receive_network_plan_once(inner.get_ref().as_raw_fd(), expected_peer)
            }) {
                Ok(result) => return result.map_err(Into::into),
                Err(_) => continue,
            }
        }
    }
}
#[cfg(target_os = "linux")]
fn create_network_ipc_socketpair() -> Result<(OwnedFd, OwnedFd), ControllerError> {
    let mut raw_fds = [-1; 2];
    // SAFETY: `raw_fds` is writable storage for the two descriptors returned
    // by `socketpair`; all arguments are Linux constants with no pointers to
    // caller-owned data beyond that fixed array.
    let result = unsafe {
        nix::libc::socketpair(
            nix::libc::AF_UNIX,
            nix::libc::SOCK_SEQPACKET | nix::libc::SOCK_CLOEXEC | nix::libc::SOCK_NONBLOCK,
            0,
            raw_fds.as_mut_ptr(),
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: a successful `socketpair` call returns two distinct descriptors
    // owned by this process. Wrapping them immediately gives every later error
    // path deterministic close-on-drop behavior.
    let supervisor = unsafe { OwnedFd::from_raw_fd(raw_fds[0]) };
    // SAFETY: see the ownership argument above for the second returned fd.
    let worker = unsafe { OwnedFd::from_raw_fd(raw_fds[1]) };
    enable_network_ipc_credentials(supervisor.as_raw_fd())?;
    enable_network_ipc_credentials(worker.as_raw_fd())?;
    Ok((supervisor, worker))
}

#[cfg(target_os = "linux")]
fn enable_network_ipc_credentials(fd: RawFd) -> io::Result<()> {
    let enabled: nix::libc::c_int = 1;
    // SAFETY: `enabled` has the exact integer representation and lifetime
    // required by Linux `SO_PASSCRED`; `fd` remains open for this call.
    let result = unsafe {
        nix::libc::setsockopt(
            fd,
            nix::libc::SOL_SOCKET,
            nix::libc::SO_PASSCRED,
            (&raw const enabled).cast(),
            nix::libc::socklen_t::try_from(core::mem::size_of_val(&enabled))
                .expect("SO_PASSCRED option width fits socklen_t"),
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
const NETWORK_IPC_CONTROL_WORDS: usize = 16;

#[cfg(target_os = "linux")]
fn send_network_plan_once(
    fd: RawFd,
    frame: &WipeArray<NETWORK_WORKER_PLAN_FRAME_BYTES>,
) -> io::Result<()> {
    // SAFETY: `frame` is a live fixed-size byte array and `fd` is a connected Unix
    // sequenced-packet socket owned by the caller. No destination pointer is required.
    let written = unsafe {
        nix::libc::send(
            fd,
            frame.as_ptr().cast(),
            frame.len(),
            nix::libc::MSG_NOSIGNAL,
        )
    };
    if written < 0 {
        return Err(io::Error::last_os_error());
    }
    let written = usize::try_from(written).map_err(|_| io::Error::other("negative plan write"))?;
    if written != frame.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            format!(
                "network-worker IPC sent {written} of {} fixed plan bytes",
                frame.len()
            ),
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn receive_network_plan_once(
    fd: RawFd,
    expected_peer: NetworkPeerCredentials,
) -> io::Result<WipeArray<NETWORK_WORKER_PLAN_FRAME_BYTES>> {
    let mut frame = WipeArray::zeroed();
    let mut iov = nix::libc::iovec {
        iov_base: frame.as_mut_ptr().cast(),
        iov_len: frame.len(),
    };
    let mut control = [0_usize; NETWORK_IPC_CONTROL_WORDS];
    // SAFETY: the zeroed header is populated with live writable buffers before `recvmsg`.
    let mut message = unsafe { core::mem::zeroed::<nix::libc::msghdr>() };
    message.msg_iov = &raw mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = core::mem::size_of_val(&control);
    // SAFETY: all pointers in `message` identify live stack storage for this syscall.
    let received = unsafe { nix::libc::recvmsg(fd, &raw mut message, nix::libc::MSG_CMSG_CLOEXEC) };
    if received < 0 {
        return Err(io::Error::last_os_error());
    }
    let received = usize::try_from(received)
        .map_err(|_| io::Error::other("negative fixed-plan receive length"))?;
    let mut error = (received != frame.len()
        || message.msg_flags & (nix::libc::MSG_TRUNC | nix::libc::MSG_CTRUNC) != 0)
        .then(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "network-worker plan must be one complete {}-byte datagram",
                    frame.len()
                ),
            )
        });
    let mut credentials = None;
    let mut received_descriptors = Vec::new();
    // SAFETY: the kernel initialized this ancillary chain and the CMSG macros preserve bounds.
    let mut header = unsafe { nix::libc::CMSG_FIRSTHDR(&message) };
    while !header.is_null() {
        // SAFETY: `header` is an in-bounds header in `message`'s ancillary buffer.
        let control_header = unsafe { &*header };
        // SAFETY: pure Linux CMSG header-size arithmetic.
        let base_len = unsafe { nix::libc::CMSG_LEN(0) as usize };
        if control_header.cmsg_len < base_len {
            error.get_or_insert_with(|| {
                io::Error::new(io::ErrorKind::InvalidData, "malformed plan control header")
            });
            break;
        }
        let payload_len = control_header.cmsg_len - base_len;
        match (control_header.cmsg_level, control_header.cmsg_type) {
            (nix::libc::SOL_SOCKET, nix::libc::SCM_CREDENTIALS)
                if payload_len == core::mem::size_of::<nix::libc::ucred>()
                    && credentials.is_none() =>
            {
                // SAFETY: the checked payload is exactly one possibly-unaligned `ucred`.
                let peer = unsafe {
                    core::ptr::read_unaligned(
                        nix::libc::CMSG_DATA(header).cast::<nix::libc::ucred>(),
                    )
                };
                match u32::try_from(peer.pid) {
                    Ok(pid) => {
                        credentials = Some(NetworkPeerCredentials {
                            pid,
                            uid: peer.uid,
                            gid: peer.gid,
                        });
                    }
                    Err(_) => {
                        error.get_or_insert_with(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "fixed plan carried an invalid peer PID",
                            )
                        });
                    }
                }
            }
            (nix::libc::SOL_SOCKET, nix::libc::SCM_RIGHTS) => {
                for index in 0..payload_len / core::mem::size_of::<RawFd>() {
                    // SAFETY: the complete descriptor slot was validated above and is adopted
                    // immediately so every rejection path closes it.
                    let raw_fd = unsafe {
                        core::ptr::read_unaligned(
                            nix::libc::CMSG_DATA(header)
                                .add(index * core::mem::size_of::<RawFd>())
                                .cast::<RawFd>(),
                        )
                    };
                    if raw_fd >= 0 {
                        // SAFETY: SCM_RIGHTS installed a new descriptor owned by this process.
                        received_descriptors.push(unsafe { OwnedFd::from_raw_fd(raw_fd) });
                    }
                }
                error.get_or_insert_with(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        if payload_len == 0
                            || !payload_len.is_multiple_of(core::mem::size_of::<RawFd>())
                        {
                            "network-worker fixed plan carried malformed descriptor data"
                        } else {
                            "network-worker fixed plan must not carry descriptors"
                        },
                    )
                });
            }
            _ => {
                error.get_or_insert_with(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "network-worker fixed plan carried unsupported ancillary data",
                    )
                });
            }
        }
        // SAFETY: advance within the same kernel-populated ancillary buffer.
        header = unsafe { nix::libc::CMSG_NXTHDR(&message, header) };
    }
    if credentials != Some(expected_peer) {
        error.get_or_insert_with(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "network-worker fixed plan credentials do not match the exact child",
            )
        });
    }
    drop(received_descriptors);
    if let Some(error) = error {
        return Err(error);
    }
    Ok(frame)
}

#[cfg(target_os = "linux")]
fn send_network_ipc_once(
    fd: RawFd,
    bytes: &[u8; NETWORK_WORKER_IPC_FRAME_BYTES],
    descriptor: Option<RawFd>,
) -> io::Result<()> {
    let mut iov = nix::libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: an all-zero `msghdr` is the documented empty message state; the
    // fields used below are populated before the syscall.
    let mut message = unsafe { core::mem::zeroed::<nix::libc::msghdr>() };
    message.msg_iov = &raw mut iov;
    message.msg_iovlen = 1;
    let mut control = [0_usize; NETWORK_IPC_CONTROL_WORDS];
    if let Some(descriptor) = descriptor {
        // SAFETY: Linux CMSG sizing helpers are pure arithmetic for the stated
        // payload width.
        let control_len = unsafe {
            nix::libc::CMSG_SPACE(
                u32::try_from(core::mem::size_of::<RawFd>())
                    .expect("descriptor width fits CMSG_SPACE"),
            ) as usize
        };
        if control_len > core::mem::size_of_val(&control) {
            return Err(io::Error::other(
                "network-worker IPC descriptor control buffer is too small",
            ));
        }
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = control_len;
        // SAFETY: `message` names the aligned `control` buffer sized above, so
        // its first header and one `RawFd` payload are writable in bounds.
        unsafe {
            let header = nix::libc::CMSG_FIRSTHDR(&message);
            if header.is_null() {
                return Err(io::Error::other(
                    "network-worker IPC could not initialize descriptor control data",
                ));
            }
            (*header).cmsg_level = nix::libc::SOL_SOCKET;
            (*header).cmsg_type = nix::libc::SCM_RIGHTS;
            (*header).cmsg_len = nix::libc::CMSG_LEN(
                u32::try_from(core::mem::size_of::<RawFd>())
                    .expect("descriptor width fits CMSG_LEN"),
            ) as usize;
            core::ptr::write_unaligned(nix::libc::CMSG_DATA(header).cast::<RawFd>(), descriptor);
        }
    }
    // SAFETY: every pointer in `message` references live stack storage for the
    // duration of this non-blocking syscall; no destination address is used on
    // the connected Unix socket.
    let written = unsafe { nix::libc::sendmsg(fd, &raw const message, nix::libc::MSG_NOSIGNAL) };
    if written < 0 {
        return Err(io::Error::last_os_error());
    }
    let written = usize::try_from(written).map_err(|_| io::Error::other("negative IPC write"))?;
    if written != NETWORK_WORKER_IPC_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            format!("network-worker IPC sent {written} of {NETWORK_WORKER_IPC_FRAME_BYTES} bytes"),
        ));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn receive_network_ipc_once(
    fd: RawFd,
    expected_token: &[u8; 32],
    expected_peer: NetworkPeerCredentials,
) -> io::Result<ReceivedNetworkIpc> {
    let mut bytes = [0_u8; NETWORK_WORKER_IPC_FRAME_BYTES];
    let mut iov = nix::libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut control = [0_usize; NETWORK_IPC_CONTROL_WORDS];
    // SAFETY: an all-zero `msghdr` is the documented empty message state; all
    // receive buffers are installed immediately below.
    let mut message = unsafe { core::mem::zeroed::<nix::libc::msghdr>() };
    message.msg_iov = &raw mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = core::mem::size_of_val(&control);
    // SAFETY: the message points only at live, writable stack buffers. The
    // close-on-exec flag applies atomically to every received descriptor.
    let received = unsafe { nix::libc::recvmsg(fd, &raw mut message, nix::libc::MSG_CMSG_CLOEXEC) };
    if received < 0 {
        return Err(io::Error::last_os_error());
    }
    if received == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "network-worker IPC peer closed",
        ));
    }
    let mut parse_error = (message.msg_flags & (nix::libc::MSG_TRUNC | nix::libc::MSG_CTRUNC) != 0)
        .then(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "network-worker IPC datagram or ancillary data was truncated",
            )
        });
    let received_bytes =
        usize::try_from(received).map_err(|_| io::Error::other("negative IPC receive length"))?;
    let mut descriptors = Vec::new();
    let mut credentials = None;
    // SAFETY: the kernel initialized the ancillary region described by
    // `message`; CMSG traversal stays within its returned `msg_controllen`.
    let mut header = unsafe { nix::libc::CMSG_FIRSTHDR(&message) };
    while !header.is_null() {
        // SAFETY: `header` is either the first in-bounds control header or the
        // result of `CMSG_NXTHDR` for the same message.
        let control_header = unsafe { &*header };
        // SAFETY: `CMSG_LEN(0)` is pure Linux header-size arithmetic.
        let base_len = unsafe { nix::libc::CMSG_LEN(0) as usize };
        if control_header.cmsg_len < base_len {
            parse_error.get_or_insert_with(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "network-worker IPC carried a malformed control header",
                )
            });
            break;
        }
        let payload_len = control_header.cmsg_len - base_len;
        match (control_header.cmsg_level, control_header.cmsg_type) {
            (nix::libc::SOL_SOCKET, nix::libc::SCM_RIGHTS) => {
                if payload_len == 0 || !payload_len.is_multiple_of(core::mem::size_of::<RawFd>()) {
                    parse_error.get_or_insert_with(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "network-worker IPC carried malformed descriptor control data",
                        )
                    });
                }
                let descriptor_count = payload_len / core::mem::size_of::<RawFd>();
                for index in 0..descriptor_count {
                    // SAFETY: the validated payload contains this complete fd;
                    // `read_unaligned` avoids imposing extra C layout alignment.
                    let raw_fd = unsafe {
                        core::ptr::read_unaligned(
                            nix::libc::CMSG_DATA(header)
                                .add(index * core::mem::size_of::<RawFd>())
                                .cast::<RawFd>(),
                        )
                    };
                    if raw_fd < 0 {
                        parse_error.get_or_insert_with(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "network-worker IPC carried an invalid descriptor",
                            )
                        });
                        continue;
                    }
                    // SAFETY: each SCM_RIGHTS entry is a new descriptor owned
                    // by this process and has not previously been wrapped.
                    descriptors.push(unsafe { OwnedFd::from_raw_fd(raw_fd) });
                }
            }
            (nix::libc::SOL_SOCKET, nix::libc::SCM_CREDENTIALS) => {
                if credentials.is_some() {
                    parse_error.get_or_insert_with(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "network-worker IPC carried duplicate credentials",
                        )
                    });
                }
                if payload_len != core::mem::size_of::<nix::libc::ucred>() {
                    parse_error.get_or_insert_with(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "network-worker IPC carried malformed credentials",
                        )
                    });
                    // SAFETY: advance within the same kernel-populated ancillary buffer.
                    header = unsafe { nix::libc::CMSG_NXTHDR(&message, header) };
                    continue;
                }
                // SAFETY: the payload width is exactly one Linux `ucred`;
                // unaligned access is explicit.
                let peer = unsafe {
                    core::ptr::read_unaligned(
                        nix::libc::CMSG_DATA(header).cast::<nix::libc::ucred>(),
                    )
                };
                match u32::try_from(peer.pid) {
                    Ok(pid) => {
                        credentials = Some(NetworkPeerCredentials {
                            pid,
                            uid: peer.uid,
                            gid: peer.gid,
                        });
                    }
                    Err(_) => {
                        parse_error.get_or_insert_with(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "network-worker IPC carried an invalid peer PID",
                            )
                        });
                    }
                }
            }
            _ => {
                parse_error.get_or_insert_with(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "network-worker IPC carried an unsupported control message",
                    )
                });
            }
        }
        // SAFETY: advance within the same kernel-populated ancillary buffer.
        header = unsafe { nix::libc::CMSG_NXTHDR(&message, header) };
    }
    if descriptors.len() > 1 {
        parse_error.get_or_insert_with(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "network-worker IPC carried more than one descriptor",
            )
        });
    }
    if credentials != Some(expected_peer) {
        parse_error.get_or_insert_with(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "network-worker IPC peer credentials {credentials:?} do not match {expected_peer:?}"
                ),
            )
        });
    }
    if let Some(error) = parse_error {
        // Every complete SCM_RIGHTS entry installed by `recvmsg` has already been adopted into
        // `descriptors`, so this return closes them even for CTRUNC and malformed ancillary data.
        return Err(error);
    }
    let frame = NetworkIpcFrame::decode(&bytes[..received_bytes], expected_token)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    Ok(ReceivedNetworkIpc { frame, descriptors })
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
    #[cfg(any(target_os = "linux", test))]
    #[error("privileged system-command custody is uncertain: {0}")]
    CommandCustody(String),
}
fn main() -> ExitCode {
    let result = match exact_hidden_command_from_argv() {
        Some(Command::RunTunnel) => close_unintended_privileged_fds()
            .map_err(ControllerError::from)
            .and_then(|()| run_tunnel_entry()),
        Some(Command::RunNetworkWorker) => run_network_worker_entry(),
        Some(_) => unreachable!("only hidden commands bypass public dispatch"),
        None => close_unintended_privileged_fds()
            .map_err(ControllerError::from)
            .and_then(|()| parse_fixed_cli_from_argv())
            .and_then(run),
    };
    print_exit_result(result)
}
fn print_exit_result(result: Result<(), ControllerError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
#[cfg(target_os = "linux")]
fn mark_unintended_child_fds_close_on_exec() -> io::Result<()> {
    const CLOSE_RANGE_UNSHARE: u32 = 1 << 1;
    const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;
    // SAFETY: `close_range` receives only integer bounds. `CLOEXEC` preserves Rust's internal
    // exec-error pipe until exec while ensuring that no caller-controlled descriptor reaches the
    // privileged child image. `UNSHARE` prevents a shared descriptor table from being modified.
    if unsafe {
        nix::libc::syscall(
            nix::libc::SYS_close_range,
            3_u32,
            u32::MAX,
            CLOSE_RANGE_UNSHARE | CLOSE_RANGE_CLOEXEC,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn close_unintended_privileged_fds() -> io::Result<()> {
    const CLOSE_RANGE_UNSHARE: u32 = 1 << 1;
    // SAFETY: no public or root-supervisor command accepts a non-stdio inherited descriptor.
    // The network worker is dispatched separately because descriptor 3 is its authenticated IPC.
    if unsafe {
        nix::libc::syscall(
            nix::libc::SYS_close_range,
            3_u32,
            u32::MAX,
            CLOSE_RANGE_UNSHARE,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn close_unintended_privileged_fds() -> io::Result<()> {
    Ok(())
}
fn exact_hidden_command_from_argv() -> Option<Command> {
    let mut arguments = env::args_os();
    let _executable = arguments.next()?;
    let command = arguments.next()?;
    if arguments.next().is_some() {
        return None;
    }
    if command == OsStr::new("run-tunnel") {
        Some(Command::RunTunnel)
    } else if command == OsStr::new("run-network-worker") {
        Some(Command::RunNetworkWorker)
    } else {
        None
    }
}
fn parse_fixed_cli_from_argv() -> Result<Cli, ControllerError> {
    let mut arguments = Vec::with_capacity(4);
    for argument in env::args_os().skip(1) {
        if arguments.len() == 4 {
            return Err(ControllerError::State(
                "VPN helper accepts at most four command arguments".to_owned(),
            ));
        }
        let argument = argument.into_string().map_err(|_| {
            ControllerError::State("VPN helper command arguments must be UTF-8".to_owned())
        })?;
        if argument.len() > MAX_SESSION_ID_BYTES_V1 {
            return Err(ControllerError::State(
                "VPN helper command argument exceeds its fixed bound".to_owned(),
            ));
        }
        arguments.push(argument);
    }
    parse_fixed_cli_arguments(arguments)
}
fn parse_fixed_cli_arguments(mut arguments: Vec<String>) -> Result<Cli, ControllerError> {
    let json_count = arguments
        .iter()
        .filter(|argument| argument.as_str() == "--json")
        .count();
    if json_count > 1 {
        return Err(ControllerError::State(
            "VPN helper accepts --json at most once".to_owned(),
        ));
    }
    arguments.retain(|argument| argument != "--json");
    let command = match arguments.as_slice() {
        [command] if command == "install-check" => Command::InstallCheck,
        [command] if command == "status" => Command::Status,
        [command] if command == "connect" => Command::Connect,
        [command, option, session_id] if command == "disconnect" && option == "--session-id" => {
            let _ = parse_canonical_session_id(session_id)?;
            Command::Disconnect {
                session_id: session_id.clone(),
            }
        }
        [command, option, session_id] if command == "repair" && option == "--session-id" => {
            let _ = parse_canonical_session_id(session_id)?;
            Command::Repair {
                session_id: session_id.clone(),
            }
        }
        [command] if command == "run-tunnel" || command == "run-network-worker" => {
            return Err(ControllerError::State(
                "hidden worker commands require exact internal argv dispatch".to_owned(),
            ));
        }
        _ => {
            return Err(ControllerError::State(
                "usage: sora-vpn-controller [--json] <install-check|status|connect|disconnect --session-id ID|repair --session-id ID>"
                    .to_owned(),
            ));
        }
    };
    Ok(Cli { command })
}
fn write_child_stdin_until(
    stdin: &mut std::process::ChildStdin,
    chunks: &[&[u8]],
    deadline: Instant,
    label: &str,
) -> Result<(), ControllerError> {
    #[cfg(target_os = "linux")]
    {
        let fd = stdin.as_raw_fd();
        // SAFETY: F_GETFL/F_SETFL inspect and update only this live pipe descriptor.
        let flags = unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFL) };
        if flags < 0
            || unsafe { nix::libc::fcntl(fd, nix::libc::F_SETFL, flags | nix::libc::O_NONBLOCK) }
                < 0
        {
            return Err(io::Error::last_os_error().into());
        }
        for chunk in chunks {
            let mut offset = 0;
            while offset < chunk.len() {
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .ok_or_else(|| ControllerError::State(format!("timed out writing {label}")))?;
                let timeout_ms =
                    remaining.as_millis().max(1).min(i32::MAX as u128) as nix::libc::c_int;
                let mut descriptor = nix::libc::pollfd {
                    fd,
                    events: nix::libc::POLLOUT,
                    revents: 0,
                };
                // SAFETY: descriptor names one live pollfd for the duration of the call.
                let ready = unsafe { nix::libc::poll(&raw mut descriptor, 1, timeout_ms) };
                if ready == 0 {
                    return Err(ControllerError::State(format!("timed out writing {label}")));
                }
                if ready < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(error.into());
                }
                if descriptor.revents
                    & (nix::libc::POLLERR | nix::libc::POLLHUP | nix::libc::POLLNVAL)
                    != 0
                {
                    return Err(ControllerError::State(format!(
                        "child pipe closed while writing {label}"
                    )));
                }
                match stdin.write(&chunk[offset..]) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            format!("child pipe made no progress writing {label}"),
                        )
                        .into());
                    }
                    Ok(written) => offset += written,
                    Err(error)
                        if error.kind() == io::ErrorKind::Interrupted
                            || error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        return Ok(());
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = deadline;
        let _ = label;
        for chunk in chunks {
            stdin.write_all(chunk)?;
        }
        Ok(())
    }
}
fn run(cli: Cli) -> Result<(), ControllerError> {
    match cli.command {
        Command::InstallCheck => {
            print_state(&install_check_display_state())?;
            Ok(())
        }
        Command::Status => {
            let caller = current_privileged_caller()?;
            let state = current_state()?;
            authorize_status_access(&state, caller)?;
            print_state(&state)?;
            Ok(())
        }
        Command::Connect => {
            let caller = current_privileged_caller()?;
            let preflight_state = current_state()?;
            authorize_connect_replacement(&preflight_state, caller)?;
            let raw_payload = read_connect_payload_json_from_stdin_with_deadline()?;
            let _lock = acquire_controller_action_lock()?;
            let previous_state = current_state()?;
            authorize_connect_replacement(&previous_state, caller)?;
            connect_command(caller, previous_state, raw_payload)
        }
        Command::Disconnect { session_id } => {
            let caller = current_privileged_caller()?;
            let _lock = acquire_controller_action_lock()?;
            disconnect_command("idle", caller, &session_id)
        }
        Command::Repair { session_id } => {
            let caller = current_privileged_caller()?;
            let _lock = acquire_controller_action_lock()?;
            repair_command(caller, &session_id)
        }
        Command::RunTunnel | Command::RunNetworkWorker => Err(ControllerError::State(
            "hidden worker command escaped exact argv dispatch".to_owned(),
        )),
    }
}
fn run_tunnel_entry() -> Result<(), ControllerError> {
    let caller = current_privileged_caller()?;
    read_and_validate_tunnel_launch_frame(caller)?;
    let supervisor_identity = capture_worker_identity(std::process::id(), WorkerRole::Tunnel)?;
    let state = current_state()?;
    authorize_unvalidated_worker_start(&state, &supervisor_identity, caller)?;
    // The root supervisor treats stdin as an opaque bounded byte string and forwards it without
    // JSON, UTF-8, Norito, ticket, or cryptographic parsing.
    let raw_payload = read_connect_payload_json_from_stdin()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_tunnel_command(
        raw_payload,
        caller,
        supervisor_identity,
        state,
    ))
}
fn encode_tunnel_launch_frame(
    caller: PrivilegedCaller,
    child_pid: u32,
) -> [u8; TUNNEL_LAUNCH_FRAME_BYTES] {
    let mut frame = [0_u8; TUNNEL_LAUNCH_FRAME_BYTES];
    frame[..8].copy_from_slice(TUNNEL_LAUNCH_FRAME_MAGIC);
    frame[8..12].copy_from_slice(&child_pid.to_be_bytes());
    frame[12..16].copy_from_slice(&caller.uid.to_be_bytes());
    frame[16..20].copy_from_slice(&caller.gid.to_be_bytes());
    frame
}
fn read_and_validate_tunnel_launch_frame(caller: PrivilegedCaller) -> Result<(), ControllerError> {
    let mut frame = [0_u8; TUNNEL_LAUNCH_FRAME_BYTES];
    let mut stdin = io::stdin().lock();
    #[cfg(target_os = "linux")]
    let deadline = Instant::now() + CONNECT_INPUT_TIMEOUT;
    let mut offset = 0;
    while offset < frame.len() {
        #[cfg(target_os = "linux")]
        wait_for_stdin_until(deadline, "fixed internal tunnel launch frame")?;
        let count = match stdin.read(&mut frame[offset..]) {
            Ok(0) => {
                return Err(ControllerError::State(
                    "truncated fixed internal tunnel launch frame".to_owned(),
                ));
            }
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        offset += count;
    }
    let child_pid = u32::from_be_bytes(frame[8..12].try_into().expect("fixed child PID"));
    let caller_uid = u32::from_be_bytes(frame[12..16].try_into().expect("fixed caller UID"));
    let caller_gid = u32::from_be_bytes(frame[16..20].try_into().expect("fixed caller GID"));
    if &frame[..8] != TUNNEL_LAUNCH_FRAME_MAGIC
        || frame[20..].iter().any(|byte| *byte != 0)
        || child_pid != std::process::id()
        || caller_uid != caller.uid
        || caller_gid != caller.gid
    {
        return Err(ControllerError::State(
            "invalid fixed internal tunnel launch frame".to_owned(),
        ));
    }
    Ok(())
}
fn install_check_display_state() -> State {
    State::default()
}
fn connect_payload_network_policy_hash(
    payload: &ConnectPayload,
) -> Result<[u8; 32], ControllerError> {
    let relay_id = parse_canonical_nonzero_hex_32(payload.relay_id_hex.as_str(), "relayIdHex")?;
    let relay_mldsa65_public_key = parse_relay_mldsa65_public_key_hex(
        payload.relay_mldsa65_public_key_hex.as_str(),
        "relayMlDsa65PublicKeyHex",
    )?;
    let descriptor_commit = parse_canonical_nonzero_hex_32(
        payload.descriptor_commit_hex.as_str(),
        "descriptorCommitHex",
    )?;
    let relay_tls_spki_sha256 = parse_canonical_nonzero_hex_32(
        payload.relay_tls_spki_sha256_hex.as_str(),
        "relayTlsSpkiSha256Hex",
    )?;
    let relay_certificate_sha256 = parse_canonical_nonzero_hex_32(
        payload.relay_certificate_sha256_hex.as_str(),
        "relayCertificateSha256Hex",
    )?;
    let directory_snapshot_digest = parse_canonical_nonzero_hex_32(
        payload.directory_snapshot_digest_hex.as_str(),
        "directorySnapshotDigestHex",
    )?;
    Ok(vpn_helper_network_policy_hash_v1(
        payload.relay_endpoint.as_str(),
        &relay_id,
        &relay_mldsa65_public_key,
        &descriptor_commit,
        payload.tls_server_name.as_str(),
        &relay_tls_spki_sha256,
        &relay_certificate_sha256,
        &directory_snapshot_digest,
        payload.padding_budget_ms,
        &payload.route_pushes,
        &payload.excluded_routes,
        &payload.dns_servers,
        &payload.tunnel_addresses,
        payload.mtu_bytes,
    ))
}
fn state_has_session_binding(state: &State) -> bool {
    state.owner_uid.is_some()
        || state.session_id.is_some()
        || state.relay_endpoint.is_some()
        || state.relay_id.is_some()
        || state.network_policy_hash.is_some()
        || state.ticket_expires_at_ms.is_some()
}
fn authorize_connect_replacement(
    state: &State,
    caller: PrivilegedCaller,
) -> Result<(), ControllerError> {
    if state_has_session_binding(state) && state.owner_uid != Some(caller.uid) {
        return Err(ControllerError::State(
            "refusing to replace a VPN session owned by a different local UID".to_owned(),
        ));
    }
    Ok(())
}
fn authorize_status_access(state: &State, caller: PrivilegedCaller) -> Result<(), ControllerError> {
    if state_has_session_binding(state) && state.owner_uid != Some(caller.uid) {
        return Err(ControllerError::State(
            "refusing to disclose VPN session state owned by a different local UID".to_owned(),
        ));
    }
    Ok(())
}
fn authorize_session_control(
    state: &State,
    caller: PrivilegedCaller,
    session_id: &str,
) -> Result<(), ControllerError> {
    if state.owner_uid != Some(caller.uid) {
        return Err(ControllerError::State(
            "refusing privileged VPN control for a different local UID".to_owned(),
        ));
    }
    if state.session_id.as_deref() != Some(session_id) {
        return Err(ControllerError::State(
            "refusing privileged VPN control for a different or stale session id".to_owned(),
        ));
    }
    Ok(())
}
fn clear_session_binding(state: &mut State) {
    state.active = false;
    state.repair_required = false;
    state.worker_identity = None;
    state.network_worker_identity = None;
    state.owner_uid = None;
    state.session_id = None;
    state.relay_endpoint = None;
    state.relay_id = None;
    state.network_policy_hash = None;
    state.ticket_expires_at_ms = None;
    state.interface_name = None;
    state.network_service = None;
    state.applied_network = None;
    state.bytes_in = 0;
    state.bytes_out = 0;
    state.message = "ready".to_owned();
}
fn finalize_failed_connect_state_with<O: NetworkCleanupOps>(
    _reaped: &ReapedControllerChild,
    state: &mut State,
    operations: &mut O,
    failure: &str,
) -> Result<(), ControllerError> {
    let cleanup_error = cleanup_persisted_network_with(state, operations).err();
    state.active = false;
    state.worker_identity = None;
    state.repair_required = cleanup_error.is_some();
    state.message = cleanup_error.as_ref().map_or_else(
        || failure.to_owned(),
        |cleanup_error| format!("{failure}; privileged network cleanup failed: {cleanup_error}"),
    );
    if cleanup_error.is_none() {
        state.interface_name = None;
        clear_session_binding(state);
    }
    operations.persist(state)?;
    cleanup_error.map_or(Ok(()), Err)
}
fn finalize_failed_connect_after_reap(
    reaped: &ReapedControllerChild,
    caller: PrivilegedCaller,
    child_identity: &WorkerProcessIdentity,
    failure: &str,
) -> Result<(), ControllerError> {
    let mut state = load_state()?;
    hydrate_runtime_fields(&mut state);
    if state_has_session_binding(&state) {
        if state.owner_uid != Some(caller.uid) {
            return Err(ControllerError::State(
                "refusing failed-connect cleanup for a different local UID".to_owned(),
            ));
        }
        if state
            .worker_identity
            .as_ref()
            .is_some_and(|identity| identity != child_identity)
        {
            return Err(ControllerError::State(
                "refusing failed-connect cleanup for a different tunnel supervisor".to_owned(),
            ));
        }
    }
    state = quiesce_persisted_workers(state)?;
    finalize_failed_connect_state_with(reaped, &mut state, &mut SystemNetworkCleanupOps, failure)
}
struct ReapedControllerChild {
    warning: Option<String>,
}
impl ReapedControllerChild {
    fn observed() -> Self {
        Self { warning: None }
    }
}
fn kill_and_reap_direct_child_bounded(
    child: &mut std::process::Child,
    label: &str,
) -> Result<std::process::ExitStatus, ControllerError> {
    if let Some(status) = child.try_wait()? {
        return Ok(status);
    }
    // An unreaped direct child keeps its PID reserved, so `Child::kill` cannot target a reused
    // process even if the child changed argv or executable identity.
    match child.kill() {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            return Err(error.into());
        }
        Err(error) => return Err(error.into()),
    }
    let deadline = Instant::now() + PROCESS_KILL_REAP_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => {
                sleep_blocking(Duration::from_millis(25));
            }
            Ok(None) => {
                return Err(ControllerError::State(format!(
                    "{label} {} did not exit after exact SIGKILL; retaining persisted identities and journal without cleanup",
                    child.id()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
}
#[cfg(target_os = "linux")]
fn terminate_and_reap_controller_child(
    child: &mut std::process::Child,
    identity: &WorkerProcessIdentity,
    grace: Duration,
) -> Result<ReapedControllerChild, ControllerError> {
    let identity_warning = (child.id() != identity.pid).then(|| {
        format!(
            "direct child PID {} does not match captured worker PID {}",
            child.id(),
            identity.pid
        )
    });
    if !matches!(child.try_wait(), Ok(Some(_))) {
        if identity_warning.is_none() {
            let _ = signal_worker_exact(identity, nix::libc::SIGTERM);
        }
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return Ok(ReapedControllerChild {
                    warning: identity_warning,
                });
            }
            sleep_blocking(Duration::from_millis(25));
        }
        // `Child` is still the unreaped direct child, so its PID cannot be reused. `kill` is an
        // exact fallback even if a compromised child changed argv and no longer matches the
        // persisted role check.
        let _ = kill_and_reap_direct_child_bounded(child, "direct VPN supervisor")?;
        return Ok(ReapedControllerChild {
            warning: identity_warning,
        });
    }
    Ok(ReapedControllerChild {
        warning: identity_warning,
    })
}
#[cfg(not(target_os = "linux"))]
fn terminate_and_reap_controller_child(
    _child: &mut std::process::Child,
    _identity: &WorkerProcessIdentity,
    _grace: Duration,
) -> Result<ReapedControllerChild, ControllerError> {
    Err(ControllerError::State(
        "exact VPN controller child reaping is only available on Linux".to_owned(),
    ))
}
fn format_connect_failure(
    failure: &str,
    reap_warning: Option<&str>,
    cleanup_error: Option<&ControllerError>,
) -> String {
    let mut message = failure.to_owned();
    if let Some(warning) = reap_warning {
        message.push_str("; exact supervisor identity warning: ");
        message.push_str(warning);
    }
    if let Some(error) = cleanup_error {
        message.push_str("; terminal cleanup failed: ");
        message.push_str(&error.to_string());
    }
    message
}
fn failed_connect_error_after_reap(
    reaped: &ReapedControllerChild,
    caller: PrivilegedCaller,
    child_identity: &WorkerProcessIdentity,
    failure: &str,
) -> ControllerError {
    let cleanup_error =
        finalize_failed_connect_after_reap(reaped, caller, child_identity, failure).err();
    ControllerError::State(format_connect_failure(
        failure,
        reaped.warning.as_deref(),
        cleanup_error.as_ref(),
    ))
}
fn quiesce_persisted_workers(mut state: State) -> Result<State, ControllerError> {
    if let Some(identity) = state.worker_identity.as_ref() {
        terminate_and_wait_persisted_worker(identity, Duration::from_secs(2))?;
    }
    #[cfg(target_os = "linux")]
    quiesce_system_command_cgroup_until(Instant::now() + PROCESS_KILL_REAP_TIMEOUT)?;
    // The tunnel supervisor may have durably advanced its mutation journal or published the
    // isolated network-child identity while it was shutting down. Reload only after its pidfd is
    // readable, then stop the exact child before returning any state eligible for global cleanup.
    state = load_state()?;
    hydrate_runtime_fields(&mut state);
    if let Some(identity) = state.network_worker_identity.as_ref() {
        terminate_and_wait_persisted_worker(identity, Duration::from_secs(2))?;
    }
    state = load_state()?;
    hydrate_runtime_fields(&mut state);
    state.worker_identity = None;
    state.network_worker_identity = None;
    Ok(state)
}
fn connect_command(
    caller: PrivilegedCaller,
    mut previous_state: State,
    raw_payload: WipeBytes,
) -> Result<(), ControllerError> {
    previous_state = quiesce_persisted_workers(previous_state)?;
    authorize_connect_replacement(&previous_state, caller)?;
    cleanup_persisted_network(&mut previous_state)?;
    let mut state = State {
        message: "authenticating unprivileged connect payload".to_owned(),
        owner_uid: Some(caller.uid),
        ..State::default()
    };
    let mut command = pinned_controller_command()?;
    #[cfg(target_os = "linux")]
    // SAFETY: the closure performs only one async-signal-safe syscall and does not allocate.
    unsafe {
        command.pre_exec(mark_unintended_child_fds_close_on_exec);
    }
    let mut child = command
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
            drop(child.stdin.take());
            return match kill_and_reap_direct_child_bounded(
                &mut child,
                "blocked VPN tunnel supervisor",
            ) {
                Ok(_) => Err(error),
                Err(reap_error) => Err(ControllerError::State(format!(
                    "{error}; exact child termination/reaping failed: {reap_error}"
                ))),
            };
        }
    };
    // Persist the exact blocked child identity before releasing its credential frame. This keeps
    // a worker from mutating host networking before durable state can identify it.
    state.worker_identity = Some(child_identity.clone());
    if let Err(error) = persist_state(&state) {
        drop(child.stdin.take());
        return match kill_and_reap_direct_child_bounded(&mut child, "blocked VPN tunnel supervisor")
        {
            Ok(_) => Err(error),
            Err(reap_error) => Err(ControllerError::State(format!(
                "{error}; exact child termination/reaping failed: {reap_error}"
            ))),
        };
    }
    let launch_frame = encode_tunnel_launch_frame(caller, child_pid);
    let payload_write = child
        .stdin
        .as_mut()
        .ok_or_else(|| ControllerError::State("failed to open worker stdin".to_owned()))
        .and_then(|stdin| {
            write_child_stdin_until(
                stdin,
                &[&launch_frame, &raw_payload],
                Instant::now() + CONNECT_INPUT_TIMEOUT,
                "fixed tunnel launch and opaque connect payload",
            )
        });
    if let Err(error) = payload_write {
        drop(child.stdin.take());
        let reap_result =
            kill_and_reap_direct_child_bounded(&mut child, "blocked VPN tunnel supervisor");
        if let Err(reap_error) = reap_result {
            return Err(ControllerError::State(format!(
                "{error}; exact child termination/reaping failed: {reap_error}; retained persisted supervisor identity"
            )));
        }
        state.worker_identity = None;
        state.message = "failed to deliver worker credentials".to_owned();
        clear_session_binding(&mut state);
        if let Err(state_error) = persist_state(&state) {
            return Err(ControllerError::State(format!(
                "{error}; failed to clear the blocked worker identity: {state_error}"
            )));
        }
        return Err(error);
    }
    drop(child.stdin.take());
    drop(raw_payload);
    let readiness_deadline = Instant::now() + CONNECT_READY_TIMEOUT;
    while Instant::now() < readiness_deadline {
        sleep_blocking(
            readiness_deadline
                .saturating_duration_since(Instant::now())
                .min(CONNECT_POLL_INTERVAL),
        );
        match child.try_wait() {
            Ok(Some(status)) => {
                let failure = format!("VPN tunnel supervisor exited before readiness: {status}");
                let reaped = ReapedControllerChild::observed();
                return Err(failed_connect_error_after_reap(
                    &reaped,
                    caller,
                    &child_identity,
                    &failure,
                ));
            }
            Ok(None) => {}
            Err(error) => {
                let reaped = terminate_and_reap_controller_child(
                    &mut child,
                    &child_identity,
                    Duration::from_secs(2),
                )?;
                let failure = format!("failed to inspect VPN tunnel supervisor: {error}");
                return Err(failed_connect_error_after_reap(
                    &reaped,
                    caller,
                    &child_identity,
                    &failure,
                ));
            }
        }
        let observed_state = match current_state() {
            Ok(state) => state,
            Err(error) => {
                let reaped = terminate_and_reap_controller_child(
                    &mut child,
                    &child_identity,
                    Duration::from_secs(2),
                )?;
                let failure = format!("failed to read VPN state while awaiting readiness: {error}");
                return Err(failed_connect_error_after_reap(
                    &reaped,
                    caller,
                    &child_identity,
                    &failure,
                ));
            }
        };
        let worker_alive = match worker_identity_alive(&child_identity) {
            Ok(alive) => alive,
            Err(error) => {
                let reaped = terminate_and_reap_controller_child(
                    &mut child,
                    &child_identity,
                    Duration::from_secs(2),
                )?;
                let failure = format!("failed to authenticate VPN tunnel supervisor: {error}");
                return Err(failed_connect_error_after_reap(
                    &reaped,
                    caller,
                    &child_identity,
                    &failure,
                ));
            }
        };
        let ready = match connect_state_ready(&observed_state, &child_identity, worker_alive) {
            Ok(ready) => ready,
            Err(error) => {
                let reaped = terminate_and_reap_controller_child(
                    &mut child,
                    &child_identity,
                    Duration::from_secs(2),
                )?;
                let failure =
                    format!("failed to authenticate VPN network-worker readiness: {error}");
                return Err(failed_connect_error_after_reap(
                    &reaped,
                    caller,
                    &child_identity,
                    &failure,
                ));
            }
        };
        if ready {
            print_state(&observed_state)?;
            return Ok(());
        }
        if !worker_alive {
            let reaped = terminate_and_reap_controller_child(
                &mut child,
                &child_identity,
                Duration::from_secs(2),
            )?;
            let failure = observed_state.message.clone();
            return Err(failed_connect_error_after_reap(
                &reaped,
                caller,
                &child_identity,
                &failure,
            ));
        }
    }
    let failure = "timed out waiting for VPN tunnel worker to report readiness";
    let reaped =
        terminate_and_reap_controller_child(&mut child, &child_identity, Duration::from_secs(2))?;
    Err(failed_connect_error_after_reap(
        &reaped,
        caller,
        &child_identity,
        failure,
    ))
}

fn connect_state_ready_with<F>(
    state: &State,
    expected_supervisor: &WorkerProcessIdentity,
    supervisor_alive: bool,
    now_ms: u64,
    mut network_worker_alive: F,
) -> Result<bool, ControllerError>
where
    F: FnMut(&WorkerProcessIdentity) -> Result<bool, ControllerError>,
{
    if !state.active
        || !active_runtime_state_complete_at(state, now_ms)
        || !supervisor_alive
        || state.worker_identity.as_ref() != Some(expected_supervisor)
    {
        return Ok(false);
    }
    let Some(network_worker) = state.network_worker_identity.as_ref() else {
        return Ok(false);
    };
    network_worker_alive(network_worker)
}

fn connect_state_ready(
    state: &State,
    expected_supervisor: &WorkerProcessIdentity,
    supervisor_alive: bool,
) -> Result<bool, ControllerError> {
    let now_ms = unix_now_ms()?;
    connect_state_ready_with(
        state,
        expected_supervisor,
        supervisor_alive,
        now_ms,
        worker_identity_alive,
    )
}

fn disconnect_command(
    message: &str,
    caller: PrivilegedCaller,
    session_id: &str,
) -> Result<(), ControllerError> {
    let mut state = current_state()?;
    authorize_session_control(&state, caller, session_id)?;
    state = quiesce_persisted_workers(state)?;
    cleanup_persisted_network(&mut state)?;
    state.active = false;
    state.repair_required = false;
    state.worker_identity = None;
    clear_session_binding(&mut state);
    state.message = message.to_owned();
    persist_state(&state)?;
    print_state(&state)?;
    Ok(())
}
fn repair_command(caller: PrivilegedCaller, session_id: &str) -> Result<(), ControllerError> {
    let mut state = current_state()?;
    authorize_session_control(&state, caller, session_id)?;
    state = quiesce_persisted_workers(state)?;
    cleanup_persisted_network(&mut state)?;
    state.active = false;
    state.repair_required = false;
    state.worker_identity = None;
    clear_session_binding(&mut state);
    state.message = "repaired".to_owned();
    persist_state(&state)?;
    print_state(&state)?;
    Ok(())
}
#[cfg(target_os = "linux")]
fn ed25519_public_key_bytes(public_key: &PublicKey) -> Result<[u8; 32], ControllerError> {
    let (algorithm, bytes) = public_key.try_to_bytes().map_err(|error| {
        ControllerError::State(format!(
            "failed to encode helper-ticket issuer key: {error}"
        ))
    })?;
    if algorithm != Algorithm::Ed25519 || bytes.len() != 32 {
        return Err(ControllerError::State(
            "helper-ticket issuer key is not canonical Ed25519".to_owned(),
        ));
    }
    let mut encoded = [0_u8; 32];
    encoded.copy_from_slice(&bytes);
    Ok(encoded)
}
#[cfg(target_os = "linux")]
fn spawn_network_worker(
    issuer_public_key: [u8; 32],
) -> Result<(NetworkWorkerProcess, Arc<NetworkIpcSocket>, [u8; 32]), ControllerError> {
    let (supervisor_fd, worker_fd) = create_network_ipc_socketpair()?;
    let mut token = [0_u8; 32];
    StdRng::from_os_rng().fill_bytes(&mut token);
    if token == [0_u8; 32] {
        return Err(ControllerError::State(
            "operating-system RNG returned an invalid all-zero IPC token".to_owned(),
        ));
    }
    let supervisor_pid = std::process::id();
    let worker_fd_raw = worker_fd.as_raw_fd();
    let mut command = pinned_controller_command()?;
    command
        .arg("run-network-worker")
        .env_clear()
        .env(NETWORK_WORKER_TOKEN_ENV, hex::encode(token))
        .env(NETWORK_WORKER_ISSUER_ENV, hex::encode(issuer_public_key))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: the closure only invokes async-signal-safe syscalls between fork and exec. The
    // socket endpoint remains owned by `worker_fd` in the parent until `spawn` returns.
    unsafe {
        command.pre_exec(move || {
            mark_unintended_child_fds_close_on_exec()?;
            if nix::libc::prctl(nix::libc::PR_SET_PDEATHSIG, nix::libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            if nix::libc::getppid() != supervisor_pid as nix::libc::pid_t {
                return Err(io::Error::from_raw_os_error(nix::libc::ESRCH));
            }
            if worker_fd_raw == NETWORK_WORKER_IPC_FD {
                let flags = nix::libc::fcntl(worker_fd_raw, nix::libc::F_GETFD);
                if flags < 0
                    || nix::libc::fcntl(
                        worker_fd_raw,
                        nix::libc::F_SETFD,
                        flags & !nix::libc::FD_CLOEXEC,
                    ) < 0
                {
                    return Err(io::Error::last_os_error());
                }
            } else {
                if nix::libc::dup3(worker_fd_raw, NETWORK_WORKER_IPC_FD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                nix::libc::close(worker_fd_raw);
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    drop(worker_fd);
    let worker_pid = child.id();
    let (identity, pidfd) =
        match capture_worker_identity_with_pidfd(worker_pid, WorkerRole::Network) {
            Ok(captured) => captured,
            Err(error) => {
                drop(child.stdin.take());
                return match kill_and_reap_direct_child_bounded(
                    &mut child,
                    "blocked unprivileged network worker",
                ) {
                    Ok(_) => Err(error),
                    Err(reap_error) => Err(ControllerError::State(format!(
                        "{error}; exact child termination/reaping failed: {reap_error}"
                    ))),
                };
            }
        };
    let mut process = NetworkWorkerProcess {
        child,
        identity,
        pidfd,
        reaped_status: None,
    };
    let ipc = match NetworkIpcSocket::new(supervisor_fd) {
        Ok(ipc) => Arc::new(ipc),
        Err(error) => {
            if let Ok(status) = kill_and_reap_direct_child_bounded(
                &mut process.child,
                "blocked unprivileged network worker",
            ) {
                process.reaped_status = Some(status);
            }
            return Err(error);
        }
    };
    Ok((process, ipc, token))
}
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetworkWorkerReady {
    Ready,
    StopRequested,
    Exited(u64),
}
#[cfg(target_os = "linux")]
async fn await_network_worker_isolated(
    process: &mut NetworkWorkerProcess,
    ipc: &NetworkIpcSocket,
    token: &[u8; 32],
    expected_worker: NetworkPeerCredentials,
    phase: &mut SupervisorIpcPhase,
    signals: &mut TunnelShutdownSignals,
) -> Result<(), ControllerError> {
    let deadline = tokio::time::sleep(NETWORK_WORKER_READY_TIMEOUT);
    let mut poll = tokio::time::interval(NETWORK_WORKER_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            message = ipc.receive(token, expected_worker) => {
                let message = message?;
                *phase = validate_supervisor_received_frame(
                    *phase,
                    message.frame,
                    message.descriptors.len(),
                )?;
                return match message.frame.kind {
                    NetworkIpcKind::Isolated => Ok(()),
                    NetworkIpcKind::WorkerExit => Err(ControllerError::State(
                        "network worker exited before isolation proof".to_owned(),
                    )),
                    _ => Err(ControllerError::State(
                        "network worker skipped the mandatory isolation barrier".to_owned(),
                    )),
                };
            }
            _ = signals.sigterm.recv() => {
                return Err(ControllerError::State(
                    "stop requested before network-worker isolation".to_owned(),
                ));
            }
            _ = signals.sigint.recv() => {
                return Err(ControllerError::State(
                    "stop requested before network-worker isolation".to_owned(),
                ));
            }
            _ = &mut deadline => {
                return Err(ControllerError::State(
                    "timed out waiting for network-worker isolation".to_owned(),
                ));
            }
            _ = poll.tick() => {
                if let Some(status) = process.poll_exit()? {
                    return Err(ControllerError::State(format!(
                        "network worker exited before isolation proof: {status}"
                    )));
                }
            }
        }
    }
}
#[cfg(target_os = "linux")]
async fn await_authenticated_network_worker_plan(
    process: &mut NetworkWorkerProcess,
    ipc: &NetworkIpcSocket,
    token: &[u8; 32],
    expected_worker: NetworkPeerCredentials,
    issuer_public_key: &PublicKey,
    signals: &mut TunnelShutdownSignals,
) -> Result<AuthenticatedPrivilegedNetworkPlan, ControllerError> {
    let deadline = tokio::time::sleep(NETWORK_WORKER_READY_TIMEOUT);
    let mut poll = tokio::time::interval(NETWORK_WORKER_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            frame = ipc.receive_plan(expected_worker) => {
                let frame = frame?;
                let now_ms = unix_now_ms()?;
                return decode_authenticated_network_plan(
                    frame.as_ref(),
                    token,
                    issuer_public_key,
                    now_ms,
                );
            }
            _ = signals.sigterm.recv() => {
                return Err(ControllerError::State(
                    "stop requested while authenticating the unprivileged connect plan".to_owned(),
                ));
            }
            _ = signals.sigint.recv() => {
                return Err(ControllerError::State(
                    "stop requested while authenticating the unprivileged connect plan".to_owned(),
                ));
            }
            _ = &mut deadline => {
                return Err(ControllerError::State(
                    "timed out waiting for the unprivileged authenticated connect plan".to_owned(),
                ));
            }
            _ = poll.tick() => {
                if let Some(status) = process.poll_exit()? {
                    return Err(ControllerError::State(format!(
                        "unprivileged network worker exited before authenticated plan: {status}"
                    )));
                }
            }
        }
    }
}
#[cfg(target_os = "linux")]
async fn await_network_worker_ready(
    process: &mut NetworkWorkerProcess,
    ipc: &NetworkIpcSocket,
    token: &[u8; 32],
    expected_worker: NetworkPeerCredentials,
    phase: &mut SupervisorIpcPhase,
    signals: &mut TunnelShutdownSignals,
) -> Result<NetworkWorkerReady, ControllerError> {
    let deadline = tokio::time::sleep(NETWORK_WORKER_READY_TIMEOUT);
    let mut poll = tokio::time::interval(NETWORK_WORKER_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            message = ipc.receive(token, expected_worker) => {
                let message = message?;
                *phase = validate_supervisor_received_frame(
                    *phase,
                    message.frame,
                    message.descriptors.len(),
                )?;
                match message.frame.kind {
                    NetworkIpcKind::WorkerReady => return Ok(NetworkWorkerReady::Ready),
                    NetworkIpcKind::WorkerExit => {
                        return Ok(NetworkWorkerReady::Exited(message.frame.value_a));
                    }
                    _ => unreachable!("state-machine validation restricts readiness messages"),
                }
            }
            _ = signals.sigterm.recv() => return Ok(NetworkWorkerReady::StopRequested),
            _ = signals.sigint.recv() => return Ok(NetworkWorkerReady::StopRequested),
            _ = &mut deadline => {
                return Err(ControllerError::State(
                    "timed out waiting for the unprivileged network worker to become ready".to_owned(),
                ));
            }
            _ = poll.tick() => {
                if let Some(status) = process.poll_exit()? {
                    return Err(ControllerError::State(format!(
                        "unprivileged network worker exited before readiness: {status}"
                    )));
                }
            }
        }
    }
}
#[cfg(target_os = "linux")]
async fn await_network_worker_tun_ack(
    process: &mut NetworkWorkerProcess,
    ipc: &NetworkIpcSocket,
    token: &[u8; 32],
    expected_worker: NetworkPeerCredentials,
    phase: &mut SupervisorIpcPhase,
    signals: &mut TunnelShutdownSignals,
) -> Result<NetworkWorkerReady, ControllerError> {
    let deadline = tokio::time::sleep(NETWORK_WORKER_TUN_TIMEOUT);
    let mut poll = tokio::time::interval(NETWORK_WORKER_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            message = ipc.receive(token, expected_worker) => {
                let message = message?;
                *phase = validate_supervisor_received_frame(
                    *phase,
                    message.frame,
                    message.descriptors.len(),
                )?;
                match message.frame.kind {
                    NetworkIpcKind::TunAck => return Ok(NetworkWorkerReady::Ready),
                    NetworkIpcKind::WorkerExit => {
                        return Ok(NetworkWorkerReady::Exited(message.frame.value_a));
                    }
                    _ => unreachable!("state-machine validation restricts TUN acknowledgement messages"),
                }
            }
            _ = signals.sigterm.recv() => return Ok(NetworkWorkerReady::StopRequested),
            _ = signals.sigint.recv() => return Ok(NetworkWorkerReady::StopRequested),
            _ = &mut deadline => {
                return Err(ControllerError::State(
                    "timed out waiting for the unprivileged network worker to validate the TUN descriptor".to_owned(),
                ));
            }
            _ = poll.tick() => {
                if let Some(status) = process.poll_exit()? {
                    return Err(ControllerError::State(format!(
                        "unprivileged network worker exited before TUN acknowledgement: {status}"
                    )));
                }
            }
        }
    }
}
#[cfg(target_os = "linux")]
async fn await_network_worker_started(
    process: &mut NetworkWorkerProcess,
    ipc: &NetworkIpcSocket,
    token: &[u8; 32],
    expected_worker: NetworkPeerCredentials,
    phase: &mut SupervisorIpcPhase,
    signals: &mut TunnelShutdownSignals,
    ticket_expires_at_ms: u64,
) -> Result<NetworkWorkerReady, ControllerError> {
    let deadline = tokio::time::sleep(NETWORK_WORKER_READY_TIMEOUT);
    let mut poll = tokio::time::interval(NETWORK_WORKER_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            message = ipc.receive(token, expected_worker) => {
                let message = message?;
                *phase = validate_supervisor_received_frame(
                    *phase,
                    message.frame,
                    message.descriptors.len(),
                )?;
                match message.frame.kind {
                    NetworkIpcKind::Started => {
                        ensure_authenticated_ticket_unexpired_for_connected_state(
                            ticket_expires_at_ms,
                        )?;
                        if let Some(status) = process.poll_exit()? {
                            return Err(ControllerError::State(format!(
                                "unprivileged network worker exited at the STARTED publication barrier: {status}"
                            )));
                        }
                        return Ok(NetworkWorkerReady::Ready);
                    }
                    NetworkIpcKind::WorkerExit => {
                        return Ok(NetworkWorkerReady::Exited(message.frame.value_a));
                    }
                    _ => unreachable!("state-machine validation restricts STARTED messages"),
                }
            }
            _ = signals.sigterm.recv() => return Ok(NetworkWorkerReady::StopRequested),
            _ = signals.sigint.recv() => return Ok(NetworkWorkerReady::StopRequested),
            _ = &mut deadline => {
                return Err(ControllerError::State(
                    "timed out waiting for the unprivileged network worker STARTED acknowledgement".to_owned(),
                ));
            }
            _ = poll.tick() => {
                if let Some(status) = process.poll_exit()? {
                    return Err(ControllerError::State(format!(
                        "unprivileged network worker exited before STARTED acknowledgement: {status}"
                    )));
                }
            }
        }
    }
}
#[cfg(target_os = "linux")]
fn network_worker_exit_message(code: u64) -> &'static str {
    match code {
        0 => "idle",
        1 => "authenticated helper ticket expired",
        2 => "local tunnel closed",
        3 => "relay tunnel closed",
        _ => "unprivileged network worker failed",
    }
}
#[cfg(target_os = "linux")]
async fn supervisor_send_ipc(
    ipc: &NetworkIpcSocket,
    token: [u8; 32],
    phase: &mut SupervisorIpcPhase,
    kind: NetworkIpcKind,
    value_a: u64,
    value_b: u64,
    descriptor: Option<RawFd>,
) -> Result<(), ControllerError> {
    let frame = NetworkIpcFrame::new(kind, token, value_a, value_b);
    let next = validate_supervisor_sent_frame(*phase, frame, usize::from(descriptor.is_some()))?;
    ipc.send(frame, descriptor).await?;
    *phase = next;
    Ok(())
}
#[cfg(target_os = "linux")]
async fn drain_stopping_network_worker(
    process: &mut NetworkWorkerProcess,
    ipc: &NetworkIpcSocket,
    token: &[u8; 32],
    expected_worker: NetworkPeerCredentials,
    phase: &mut SupervisorIpcPhase,
    state: &mut State,
    accounting: &mut WorkerTrafficAccounting,
) -> Result<u64, ControllerError> {
    let deadline = tokio::time::sleep(NETWORK_WORKER_STOP_TIMEOUT);
    let mut poll = tokio::time::interval(NETWORK_WORKER_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            message = ipc.receive(token, expected_worker) => {
                let message = message?;
                *phase = validate_supervisor_received_frame(
                    *phase,
                    message.frame,
                    message.descriptors.len(),
                )?;
                match message.frame.kind {
                    NetworkIpcKind::Traffic => accounting.observe_at(
                        state,
                        message.frame.value_a,
                        message.frame.value_b,
                        tokio::time::Instant::now(),
                        unix_now_ms()?,
                    )?,
                    NetworkIpcKind::WorkerExit => return Ok(message.frame.value_a),
                    NetworkIpcKind::WorkerReady
                    | NetworkIpcKind::TunAck
                    | NetworkIpcKind::Started => {}
                    _ => unreachable!("state-machine validation restricts stopping messages"),
                }
            }
            _ = poll.tick() => {
                if let Some(status) = process.poll_exit()? {
                    return Err(ControllerError::State(format!(
                        "unprivileged network worker exited without its final IPC frame: {status}"
                    )));
                }
            }
            _ = &mut deadline => {
                let status = process.stop_and_reap(Duration::ZERO).await?;
                return Err(ControllerError::State(format!(
                    "unprivileged network worker did not stop within the bounded grace period: {status}"
                )));
            }
        }
    }
}
#[cfg(target_os = "linux")]
async fn supervise_active_network_worker(
    process: &mut NetworkWorkerProcess,
    ipc: &NetworkIpcSocket,
    token: [u8; 32],
    expected_worker: NetworkPeerCredentials,
    phase: &mut SupervisorIpcPhase,
    state: &mut State,
    signals: &mut TunnelShutdownSignals,
) -> Result<u64, ControllerError> {
    let accounting_started_at = tokio::time::Instant::now();
    let mut accounting =
        WorkerTrafficAccounting::new(state.bytes_out, state.bytes_in, accounting_started_at)?;
    let mut poll = tokio::time::interval(NETWORK_WORKER_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut persist_tick = tokio::time::interval_at(
        accounting.next_persist_at,
        TRAFFIC_ACCOUNTING_PERSIST_INTERVAL,
    );
    persist_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let outcome = loop {
        tokio::select! {
            message = ipc.receive(&token, expected_worker) => {
                let message = match message {
                    Ok(message) => message,
                    Err(error) => break Err(error),
                };
                let next_phase = match validate_supervisor_received_frame(
                    *phase,
                    message.frame,
                    message.descriptors.len(),
                ) {
                    Ok(next_phase) => next_phase,
                    Err(error) => break Err(error),
                };
                *phase = next_phase;
                match message.frame.kind {
                    NetworkIpcKind::Traffic => {
                        let wall_now_ms = match unix_now_ms() {
                            Ok(now_ms) => now_ms,
                            Err(error) => break Err(error),
                        };
                        if let Err(error) = accounting.observe_at(
                            state,
                            message.frame.value_a,
                            message.frame.value_b,
                            tokio::time::Instant::now(),
                            wall_now_ms,
                        ) {
                            break Err(error);
                        }
                    }
                    NetworkIpcKind::WorkerExit => break Ok(message.frame.value_a),
                    _ => unreachable!("state-machine validation restricts active messages"),
                }
            }
            _ = signals.sigterm.recv() => {
                if let Err(error) = supervisor_send_ipc(
                    ipc,
                    token,
                    phase,
                    NetworkIpcKind::Stop,
                    0,
                    0,
                    None,
                ).await {
                    break Err(error);
                }
                break drain_stopping_network_worker(
                    process,
                    ipc,
                    &token,
                    expected_worker,
                    phase,
                    state,
                    &mut accounting,
                ).await;
            }
            _ = signals.sigint.recv() => {
                if let Err(error) = supervisor_send_ipc(
                    ipc,
                    token,
                    phase,
                    NetworkIpcKind::Stop,
                    0,
                    0,
                    None,
                ).await {
                    break Err(error);
                }
                break drain_stopping_network_worker(
                    process,
                    ipc,
                    &token,
                    expected_worker,
                    phase,
                    state,
                    &mut accounting,
                ).await;
            }
            _ = persist_tick.tick() => {
                let wall_now_ms = match unix_now_ms() {
                    Ok(now_ms) => now_ms,
                    Err(error) => break Err(error),
                };
                accounting.apply_expiry_at(state, wall_now_ms);
                if let Err(error) = accounting.flush_if_due(
                    state,
                    tokio::time::Instant::now(),
                ) {
                    break Err(error);
                }
            }
            _ = poll.tick() => {
                match process.poll_exit() {
                    Ok(Some(status)) => {
                        break Err(ControllerError::State(format!(
                            "unprivileged network worker exited without its final IPC frame: {status}"
                        )));
                    }
                    Ok(None) => {}
                    Err(error) => break Err(error),
                }
            }
        }
    };
    let flush = force_flush_worker_traffic_accounting(&mut accounting, state);
    finish_worker_traffic_accounting(outcome, flush)
}
#[cfg(target_os = "linux")]
async fn stop_network_worker_before_tun(
    process: &mut NetworkWorkerProcess,
    ipc: &NetworkIpcSocket,
    token: [u8; 32],
    expected_worker: NetworkPeerCredentials,
    phase: &mut SupervisorIpcPhase,
    state: &mut State,
) -> Result<u64, ControllerError> {
    let accounting_started_at = tokio::time::Instant::now();
    let mut accounting =
        WorkerTrafficAccounting::new(state.bytes_out, state.bytes_in, accounting_started_at)?;
    let protocol_outcome = async {
        supervisor_send_ipc(ipc, token, phase, NetworkIpcKind::Stop, 0, 0, None).await?;
        drain_stopping_network_worker(
            process,
            ipc,
            &token,
            expected_worker,
            phase,
            state,
            &mut accounting,
        )
        .await
    }
    .await;
    let flush = force_flush_worker_traffic_accounting(&mut accounting, state);
    let protocol_result = finish_worker_traffic_accounting(protocol_outcome, flush);
    match protocol_result {
        Ok(exit_code) => {
            let _ = process.reap_after_protocol_exit().await?;
            Ok(exit_code)
        }
        Err(protocol_error) => {
            // A broken/malformed IPC channel must not let a child retain the transferred TUN fd.
            // Exact pidfd-backed termination and reaping completes before the caller is allowed
            // to drop its own descriptor or restore any global host state.
            match process.stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT).await {
                Ok(_) => Err(protocol_error),
                Err(reap_error) => Err(ControllerError::State(format!(
                    "{protocol_error}; exact child termination/reaping also failed: {reap_error}"
                ))),
            }
        }
    }
}
#[cfg(target_os = "linux")]
fn confirm_network_worker_reaped(
    process: &NetworkWorkerProcess,
    state: &mut State,
    context: &str,
) -> Result<(), ControllerError> {
    if process.is_reaped() {
        state.network_worker_identity = None;
        return Ok(());
    }
    state.active = false;
    // A full authenticated binding may enter repair state. The earlier pending-authentication
    // state has a deliberately narrower invariant, but still retains both exact identities and
    // refuses every global cleanup path.
    state.repair_required = state.session_id.is_some();
    state.message = format!(
        "{context}; exact network-worker exit was not proven; retaining both identities and the privileged network journal"
    );
    let persist_result = persist_state(state);
    match persist_result {
        Ok(()) => Err(ControllerError::State(state.message.clone())),
        Err(error) => Err(ControllerError::State(format!(
            "{}; failed to persist fail-closed process custody: {error}",
            state.message
        ))),
    }
}
#[cfg(target_os = "linux")]
async fn force_reap_network_worker(
    process: &mut NetworkWorkerProcess,
    state: &mut State,
    context: &str,
) -> Result<(), ControllerError> {
    let stop_result = process.stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT).await;
    confirm_network_worker_reaped(process, state, context)?;
    stop_result.map(|_| ())
}
#[cfg(target_os = "linux")]
async fn run_tunnel_command(
    raw_payload: WipeBytes,
    caller: PrivilegedCaller,
    supervisor_identity: WorkerProcessIdentity,
    mut state: State,
) -> Result<(), ControllerError> {
    // The root process is only a local supervisor. It owns signals, durable state, TUN creation,
    // routes, DNS, descriptor custody, exact child reaping, and deterministic cleanup.
    let mut shutdown_signals = TunnelShutdownSignals::install()?;
    authorize_unvalidated_worker_start(&state, &supervisor_identity, caller)?;
    let issuer_public_key = load_helper_ticket_issuer_public_key()?;
    let issuer_public_key_bytes = ed25519_public_key_bytes(&issuer_public_key)?;
    let (mut network_worker, ipc, token) = match spawn_network_worker(issuer_public_key_bytes) {
        Ok(result) => result,
        Err(error) => {
            state.worker_identity = None;
            state.message = error.to_string();
            clear_session_binding(&mut state);
            persist_state(&state)?;
            return Err(error);
        }
    };
    state.network_worker_identity = Some(network_worker.identity.clone());
    if let Err(error) = persist_state(&state) {
        let context = error.to_string();
        if let Err(reap_error) =
            force_reap_network_worker(&mut network_worker, &mut state, context.as_str()).await
        {
            return Err(ControllerError::State(format!(
                "{error}; exact unprivileged worker reaping failed: {reap_error}"
            )));
        }
        return Err(error);
    }
    let expected_worker = NetworkPeerCredentials {
        pid: network_worker.identity.pid,
        uid: caller.uid,
        gid: caller.gid,
    };
    let mut phase = SupervisorIpcPhase::AwaitingReady;
    let isolation_result = await_network_worker_isolated(
        &mut network_worker,
        &ipc,
        &token,
        expected_worker,
        &mut phase,
        &mut shutdown_signals,
    )
    .await
    .and_then(|()| verify_network_worker_isolation(&network_worker, caller));
    if let Err(error) = isolation_result {
        force_reap_network_worker(
            &mut network_worker,
            &mut state,
            "network-worker isolation failed",
        )
        .await?;
        clear_session_binding(&mut state);
        persist_state(&state)?;
        return Err(error);
    }
    let payload_write = network_worker
        .child
        .stdin
        .as_mut()
        .ok_or_else(|| ControllerError::State("network-worker stdin is unavailable".to_owned()))
        .and_then(|stdin| {
            write_child_stdin_until(
                stdin,
                &[&raw_payload],
                Instant::now() + CONNECT_INPUT_TIMEOUT,
                "opaque connect payload to isolated worker",
            )
        });
    drop(network_worker.child.stdin.take());
    drop(raw_payload);
    if let Err(error) = payload_write {
        force_reap_network_worker(
            &mut network_worker,
            &mut state,
            "opaque payload delivery to the isolated worker failed",
        )
        .await?;
        clear_session_binding(&mut state);
        persist_state(&state)?;
        return Err(error);
    }
    let payload = match await_authenticated_network_worker_plan(
        &mut network_worker,
        &ipc,
        &token,
        expected_worker,
        &issuer_public_key,
        &mut shutdown_signals,
    )
    .await
    {
        Ok(payload) => payload,
        Err(error) => {
            force_reap_network_worker(
                &mut network_worker,
                &mut state,
                "authenticated fixed-plan admission failed",
            )
            .await?;
            clear_session_binding(&mut state);
            persist_state(&state)?;
            return Err(error);
        }
    };
    state.active = false;
    state.repair_required = false;
    state.worker_identity = Some(supervisor_identity.clone());
    state.session_id = Some(payload.session_id.clone());
    state.relay_endpoint = Some(payload.relay_endpoint.clone());
    state.relay_id = Some(payload.relay_id);
    state.network_policy_hash = Some(payload.network_policy_hash);
    state.ticket_expires_at_ms = Some(payload.ticket_expires_at_ms);
    state.bytes_in = 0;
    state.bytes_out = 0;
    state.message = "starting isolated network worker".to_owned();
    state.applied_network = None;
    if let Err(error) = persist_state(&state) {
        if let Err(reap_error) = network_worker
            .stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT)
            .await
        {
            return Err(ControllerError::State(format!(
                "{error}; exact unprivileged worker reaping failed: {reap_error}"
            )));
        }
        return Err(error);
    }
    // The root process retains only this fixed, independently ticket-verified network plan. The
    // full payload, helper-ticket text, and metering seed exist solely in the isolated worker.
    let readiness = match await_network_worker_ready(
        &mut network_worker,
        &ipc,
        &token,
        expected_worker,
        &mut phase,
        &mut shutdown_signals,
    )
    .await
    {
        Ok(readiness) => readiness,
        Err(error) => {
            force_reap_network_worker(
                &mut network_worker,
                &mut state,
                "network worker failed before readiness",
            )
            .await?;
            clear_session_binding(&mut state);
            persist_state(&state)?;
            return Err(error);
        }
    };
    match readiness {
        NetworkWorkerReady::Ready => {}
        NetworkWorkerReady::StopRequested => {
            let stop_result = stop_network_worker_before_tun(
                &mut network_worker,
                &ipc,
                token,
                expected_worker,
                &mut phase,
                &mut state,
            )
            .await;
            confirm_network_worker_reaped(
                &network_worker,
                &mut state,
                "network worker did not stop before TUN creation",
            )?;
            let message = stop_result
                .map(network_worker_exit_message)
                .unwrap_or("unprivileged network worker failed while stopping")
                .to_owned();
            clear_session_binding(&mut state);
            persist_state(&state)?;
            return stop_result.map(|_| ()).map_err(|error| {
                ControllerError::State(format!("{message}; worker stop protocol failed: {error}"))
            });
        }
        NetworkWorkerReady::Exited(code) => {
            let _ = network_worker.reap_after_protocol_exit().await?;
            confirm_network_worker_reaped(
                &network_worker,
                &mut state,
                "network worker exit before TUN creation was not reaped",
            )?;
            let message = network_worker_exit_message(code).to_owned();
            clear_session_binding(&mut state);
            persist_state(&state)?;
            return Err(ControllerError::State(message));
        }
    }

    // No host-network state is changed until an exact unprivileged child, proven by SCM
    // credentials, has completed DNS/QUIC/TLS/handshake/voucher setup and reported READY.
    let prepared_result =
        match ensure_authenticated_ticket_unexpired_at(payload.ticket_expires_at_ms) {
            Ok(()) => prepare_tunnel(&payload, &mut state, &network_worker),
            Err(error) => Err(error),
        };
    let prepared = match prepared_result {
        Ok(prepared) => CleanupGuard::new(prepared, cleanup_tunnel),
        Err(error) => {
            let stop_result = stop_network_worker_before_tun(
                &mut network_worker,
                &ipc,
                token,
                expected_worker,
                &mut phase,
                &mut state,
            )
            .await;
            confirm_network_worker_reaped(
                &network_worker,
                &mut state,
                "network worker was not reaped after privileged preparation failed",
            )?;
            let cleanup_error = cleanup_persisted_network(&mut state).err();
            state.active = false;
            state.repair_required = cleanup_error.is_some();
            state.message = cleanup_error.as_ref().map_or_else(
                || error.to_string(),
                |cleanup_error| format!("{error}; preparation cleanup failed: {cleanup_error}"),
            );
            if !state.repair_required {
                state.interface_name = None;
                clear_session_binding(&mut state);
            }
            persist_state(&state)?;
            return match stop_result {
                Ok(_) => Err(error),
                Err(stop_error) => Err(ControllerError::State(format!(
                    "{error}; worker stop protocol failed: {stop_error}"
                ))),
            };
        }
    };

    let packet_read_mtu = u64::try_from(prepared.get().packet_read_mtu).map_err(|_| {
        ControllerError::State("prepared TUN MTU exceeds the IPC field width".to_owned())
    })?;
    if let Err(error) = supervisor_send_ipc(
        &ipc,
        token,
        &mut phase,
        NetworkIpcKind::TunReady,
        packet_read_mtu,
        0,
        Some(prepared.get().device.as_raw_fd()),
    )
    .await
    {
        let reap_error = network_worker
            .stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT)
            .await
            .err();
        let mut message = error.to_string();
        if let Some(reap_error) = reap_error {
            message = format!("{message}; exact child reaping failed: {reap_error}");
        }
        return match finish_prepared_tunnel(prepared, &mut state, &network_worker, message) {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(ControllerError::State(format!(
                "{error}; safe tunnel cleanup failed: {cleanup_error}"
            ))),
        };
    }

    let tun_ack = await_network_worker_tun_ack(
        &mut network_worker,
        &ipc,
        &token,
        expected_worker,
        &mut phase,
        &mut shutdown_signals,
    )
    .await;
    match tun_ack {
        Ok(NetworkWorkerReady::Ready) => {}
        Ok(NetworkWorkerReady::StopRequested) => {
            let stop_result = stop_network_worker_before_tun(
                &mut network_worker,
                &ipc,
                token,
                expected_worker,
                &mut phase,
                &mut state,
            )
            .await;
            let message = stop_result
                .as_ref()
                .map(|code| network_worker_exit_message(*code))
                .unwrap_or("unprivileged network worker failed while stopping")
                .to_owned();
            finish_prepared_tunnel(prepared, &mut state, &network_worker, message)?;
            return stop_result.map(|_| ());
        }
        Ok(NetworkWorkerReady::Exited(code)) => {
            let reap_error = network_worker.reap_after_protocol_exit().await.err();
            let mut message = network_worker_exit_message(code).to_owned();
            if let Some(reap_error) = reap_error {
                message = format!("{message}; exact child reaping failed: {reap_error}");
            }
            finish_prepared_tunnel(prepared, &mut state, &network_worker, message.clone())?;
            return Err(ControllerError::State(message));
        }
        Err(error) => {
            let reap_error = network_worker
                .stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT)
                .await
                .err();
            let mut message = error.to_string();
            if let Some(reap_error) = reap_error {
                message = format!("{message}; exact child reaping failed: {reap_error}");
            }
            return match finish_prepared_tunnel(prepared, &mut state, &network_worker, message) {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(ControllerError::State(format!(
                    "{error}; safe tunnel cleanup failed: {cleanup_error}"
                ))),
            };
        }
    }

    if let Err(error) =
        supervisor_send_ipc(&ipc, token, &mut phase, NetworkIpcKind::Start, 0, 0, None).await
    {
        let reap_error = network_worker
            .stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT)
            .await
            .err();
        let mut message = error.to_string();
        if let Some(reap_error) = reap_error {
            message = format!("{message}; exact child reaping failed: {reap_error}");
        }
        return match finish_prepared_tunnel(prepared, &mut state, &network_worker, message) {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(ControllerError::State(format!(
                "{error}; safe tunnel cleanup failed: {cleanup_error}"
            ))),
        };
    }
    let started = await_network_worker_started(
        &mut network_worker,
        &ipc,
        &token,
        expected_worker,
        &mut phase,
        &mut shutdown_signals,
        payload.ticket_expires_at_ms,
    )
    .await;
    match started {
        Ok(NetworkWorkerReady::Ready) => {}
        Ok(NetworkWorkerReady::StopRequested) => {
            let stop_result = stop_network_worker_before_tun(
                &mut network_worker,
                &ipc,
                token,
                expected_worker,
                &mut phase,
                &mut state,
            )
            .await;
            let message = stop_result
                .as_ref()
                .map(|code| network_worker_exit_message(*code))
                .unwrap_or("unprivileged network worker failed while stopping")
                .to_owned();
            finish_prepared_tunnel(prepared, &mut state, &network_worker, message)?;
            return stop_result.map(|_| ());
        }
        Ok(NetworkWorkerReady::Exited(code)) => {
            let reap_error = network_worker.reap_after_protocol_exit().await.err();
            let mut message = network_worker_exit_message(code).to_owned();
            if let Some(reap_error) = reap_error {
                message = format!("{message}; exact child reaping failed: {reap_error}");
            }
            finish_prepared_tunnel(prepared, &mut state, &network_worker, message.clone())?;
            return Err(ControllerError::State(message));
        }
        Err(error) => {
            let reap_error = network_worker
                .stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT)
                .await
                .err();
            let mut message = error.to_string();
            if let Some(reap_error) = reap_error {
                message = format!("{message}; exact child reaping failed: {reap_error}");
            }
            return match finish_prepared_tunnel(prepared, &mut state, &network_worker, message) {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(ControllerError::State(format!(
                    "{error}; safe tunnel cleanup failed: {cleanup_error}"
                ))),
            };
        }
    }

    state.repair_required = false;
    state.interface_name = Some(prepared.get().interface_name.clone());
    state.network_service = prepared.get().network_service.clone();
    state.applied_network = Some(prepared.get().applied_network.clone());
    state.message = "connected".to_owned();

    // This is the final authorization and process-custody barrier. It deliberately has no await
    // or fallible preparation between the exact child/ticket checks and the connected-state write
    // below. Persistence independently rechecks the durable expiry invariant before encoding.
    let publication_check = network_worker
        .exact_identity_alive()
        .and_then(|child_alive| {
            ensure_connected_publication_ready_at(
                payload.ticket_expires_at_ms,
                unix_now_ms()?,
                child_alive,
            )
        });
    if let Err(error) = publication_check {
        let reap_error = network_worker
            .stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT)
            .await
            .err();
        let mut message = error.to_string();
        if let Some(reap_error) = reap_error {
            message = format!("{message}; exact child reaping failed: {reap_error}");
        }
        return match finish_prepared_tunnel(prepared, &mut state, &network_worker, message) {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(ControllerError::State(format!(
                "{error}; safe tunnel cleanup failed: {cleanup_error}"
            ))),
        };
    }

    // The worker has validated the TUN fd and acknowledged that its packet loop is armed. Only
    // now may the controller publish connected state to the waiting caller.
    state.active = true;
    if let Err(persist_error) = persist_state(&state) {
        let stop_error = stop_network_worker_before_tun(
            &mut network_worker,
            &ipc,
            token,
            expected_worker,
            &mut phase,
            &mut state,
        )
        .await
        .err();
        let mut message = format!("failed to persist connected state: {persist_error}");
        if let Some(stop_error) = &stop_error {
            message = format!("{message}; worker shutdown failed: {stop_error}");
        }
        return match finish_prepared_tunnel(prepared, &mut state, &network_worker, message) {
            Ok(()) if stop_error.is_none() => Err(persist_error),
            Ok(()) => Err(ControllerError::State(format!(
                "{persist_error}; worker stop protocol failed"
            ))),
            Err(cleanup_error) => Err(ControllerError::State(format!(
                "{persist_error}; safe tunnel cleanup failed: {cleanup_error}"
            ))),
        };
    }
    let worker_result = supervise_active_network_worker(
        &mut network_worker,
        &ipc,
        token,
        expected_worker,
        &mut phase,
        &mut state,
        &mut shutdown_signals,
    )
    .await;
    let (worker_code, worker_error) = match worker_result {
        Ok(code) => {
            let reap_result = network_worker.reap_after_protocol_exit().await;
            match reap_result {
                Ok(status) if status.success() => (Some(code), None),
                Ok(status) => (
                    None,
                    Some(ControllerError::State(format!(
                        "unprivileged network worker reported exit then failed: {status}"
                    ))),
                ),
                Err(error) => (None, Some(error)),
            }
        }
        Err(error) => {
            let reap_error = network_worker
                .stop_and_reap(NETWORK_WORKER_STOP_TIMEOUT)
                .await
                .err();
            let error = reap_error.map_or(error, |reap_error| {
                ControllerError::State(format!("{error}; exact child reaping failed: {reap_error}"))
            });
            (None, Some(error))
        }
    };
    let mut message = worker_code
        .map(network_worker_exit_message)
        .unwrap_or("unprivileged network worker failed")
        .to_owned();
    let had_worker_error = worker_error.is_some();
    if let Some(error) = worker_error {
        message = error.to_string();
    }
    finish_prepared_tunnel(prepared, &mut state, &network_worker, message.clone())?;
    if had_worker_error {
        Err(ControllerError::State(message))
    } else {
        Ok(())
    }
}
#[cfg(not(target_os = "linux"))]
async fn run_tunnel_command(
    _raw_payload: WipeBytes,
    _caller: PrivilegedCaller,
    _supervisor_identity: WorkerProcessIdentity,
    _state: State,
) -> Result<(), ControllerError> {
    Err(ControllerError::State(
        "the privileged VPN supervisor is only supported on Linux".to_owned(),
    ))
}
fn authorize_unvalidated_worker_start(
    state: &State,
    worker_identity: &WorkerProcessIdentity,
    caller: PrivilegedCaller,
) -> Result<(), ControllerError> {
    let exact_start_record = !state.active
        && !state.repair_required
        && state.message == "authenticating unprivileged connect payload"
        && state.worker_identity.as_ref() == Some(worker_identity)
        && state.owner_uid == Some(caller.uid)
        && state.session_id.is_none()
        && state.relay_endpoint.is_none()
        && state.relay_id.is_none()
        && state.network_policy_hash.is_none()
        && state.applied_network.is_none();
    if !exact_start_record {
        return Err(ControllerError::State(
            "worker invocation is not bound to the controller's exact persisted start record"
                .to_owned(),
        ));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
async fn worker_send_ipc(
    ipc: &NetworkIpcSocket,
    token: [u8; 32],
    phase: &mut WorkerIpcPhase,
    kind: NetworkIpcKind,
    value_a: u64,
    value_b: u64,
) -> Result<(), ControllerError> {
    let frame = NetworkIpcFrame::new(kind, token, value_a, value_b);
    let next = validate_worker_sent_frame(*phase, frame, 0)?;
    ipc.send(frame, None).await?;
    *phase = next;
    Ok(())
}
#[cfg(target_os = "linux")]
async fn receive_worker_control(
    ipc: &NetworkIpcSocket,
    token: &[u8; 32],
    expected_supervisor: NetworkPeerCredentials,
    phase: &mut WorkerIpcPhase,
) -> Result<ReceivedNetworkIpc, ControllerError> {
    let message = timeout(
        NETWORK_WORKER_TUN_TIMEOUT,
        ipc.receive(token, expected_supervisor),
    )
    .await
    .map_err(|_| {
        ControllerError::State("timed out waiting for root-supervisor IPC".to_owned())
    })??;
    *phase = validate_worker_received_frame(*phase, message.frame, message.descriptors.len())?;
    Ok(message)
}
#[cfg(target_os = "linux")]
async fn run_network_worker_command(
    authenticated: AuthenticatedConnectPayload,
    ipc: Arc<NetworkIpcSocket>,
    token: [u8; 32],
    expected_supervisor: NetworkPeerCredentials,
) -> Result<(), ControllerError> {
    let mut phase = WorkerIpcPhase::Connecting;
    let plan = encode_authenticated_network_plan(&authenticated, token)?;
    ipc.send_plan(&plan).await?;
    let result = run_network_worker_session(
        authenticated,
        Arc::clone(&ipc),
        token,
        expected_supervisor,
        &mut phase,
    )
    .await;
    if result.is_err() && phase != WorkerIpcPhase::Exited {
        let _ = worker_send_ipc(
            &ipc,
            token,
            &mut phase,
            NetworkIpcKind::WorkerExit,
            u64::MAX,
            0,
        )
        .await;
    }
    result
}
#[cfg(target_os = "linux")]
async fn run_network_worker_session(
    authenticated: AuthenticatedConnectPayload,
    ipc: Arc<NetworkIpcSocket>,
    token: [u8; 32],
    expected_supervisor: NetworkPeerCredentials,
    phase: &mut WorkerIpcPhase,
) -> Result<(), ControllerError> {
    let AuthenticatedConnectPayload {
        mut payload,
        ticket,
    } = authenticated;
    let (endpoint, connection, record_layer) = connect_and_handshake(&payload).await?;
    let (mut send, mut recv) = match timeout(CONNECT_TIMEOUT, connection.open_bi()).await {
        Ok(Ok(streams)) => streams,
        Ok(Err(error)) => return Err(ControllerError::Connection(error)),
        Err(_) => {
            return Err(ControllerError::State(
                "timed out opening relay VPN tunnel stream".to_owned(),
            ));
        }
    };
    let record_stream = record_layer
        .stream(record_stream_context(send.id()))
        .map_err(|error| ControllerError::Handshake(error.to_string()))?;
    let mut voucher_signer = UsageVoucherSigner::from_payload(&payload, ticket.clone())?;
    let expected_interface_name = desired_interface_name(payload.session_id.as_str())?;
    let expected_mtu = normalize_mtu(payload.mtu_bytes)?;
    payload.wipe_credentials();

    worker_send_ipc(&ipc, token, phase, NetworkIpcKind::WorkerReady, 0, 0).await?;
    let mut tun_message = receive_worker_control(&ipc, &token, expected_supervisor, phase).await?;
    if tun_message.frame.kind == NetworkIpcKind::Stop {
        worker_send_ipc(&ipc, token, phase, NetworkIpcKind::WorkerExit, 0, 0).await?;
        connection.close(0u32.into(), b"idle");
        endpoint.close(0u32.into(), b"idle");
        endpoint.wait_idle().await;
        return Ok(());
    }
    let received_mtu = u16::try_from(tun_message.frame.value_a).map_err(|_| {
        ControllerError::State("root supervisor sent an out-of-range TUN MTU".to_owned())
    })?;
    if received_mtu != expected_mtu {
        return Err(ControllerError::State(format!(
            "root supervisor TUN MTU {received_mtu} does not match signed payload MTU {expected_mtu}"
        )));
    }
    let tun_fd = tun_message.descriptors.pop().ok_or_else(|| {
        ControllerError::State("root supervisor omitted the TUN descriptor".to_owned())
    })?;
    let device = Arc::new(LinuxTunDevice::from_received_fd(
        tun_fd,
        &expected_interface_name,
        expected_mtu,
    )?);
    worker_send_ipc(&ipc, token, phase, NetworkIpcKind::TunAck, 0, 0).await?;
    let start = receive_worker_control(&ipc, &token, expected_supervisor, phase).await?;
    if start.frame.kind == NetworkIpcKind::Stop {
        worker_send_ipc(&ipc, token, phase, NetworkIpcKind::WorkerExit, 0, 0).await?;
        connection.close(0u32.into(), b"idle");
        endpoint.close(0u32.into(), b"idle");
        endpoint.wait_idle().await;
        return Ok(());
    }
    debug_assert_eq!(start.frame.kind, NetworkIpcKind::Start);
    // Arm the monotonic authorization boundary before acknowledging STARTED. The packet loop
    // receives this exact deadline, so scheduling or a later wall-clock rollback cannot extend it.
    let ticket_expiry_deadline = authenticated_ticket_expiry_deadline(ticket.expires_at_ms)?;
    worker_send_ipc(&ipc, token, phase, NetworkIpcKind::Started, 0, 0).await?;

    let circuit_id = ticket.session_id;
    let flow_label = vpn_flow_label_from_session_id(circuit_id)?;
    voucher_signer.begin_service();
    let voucher_counters = UsageVoucherCounters::default();
    let control = network_worker_control_loop(
        Arc::clone(&ipc),
        token,
        expected_supervisor,
        voucher_counters.clone(),
    );
    let mut protected_send = RecordWriter::new(&mut send, record_stream.sealer);
    let mut protected_recv = RecordReader::new(&mut recv, record_stream.opener);
    let shutdown = tunnel_packet_loop(
        device,
        &mut protected_send,
        &mut protected_recv,
        TunnelTrafficConfig {
            circuit_id,
            flow_label,
            padding_budget_ms: payload.padding_budget_ms,
            packet_read_mtu: usize::from(expected_mtu),
        },
        ticket_expiry_deadline,
        voucher_signer,
        voucher_counters.clone(),
        control,
    )
    .await;
    let (ingress, egress) = voucher_counters.snapshot();
    worker_send_ipc(&ipc, token, phase, NetworkIpcKind::Traffic, ingress, egress).await?;
    let (exit_code, close_message) = match &shutdown {
        Ok(exit) if exit.message == "authenticated helper ticket expired" => {
            (1, exit.message.as_str())
        }
        Ok(exit) if exit.message == "local tunnel closed" => (2, exit.message.as_str()),
        Ok(exit) if exit.message == "relay tunnel closed" => (3, exit.message.as_str()),
        Ok(exit) => (0, exit.message.as_str()),
        Err(_) => (u64::MAX, "unprivileged network worker failed"),
    };
    let finish_queued = protected_send.shutdown().await.is_ok();
    drop(protected_send);
    drop(protected_recv);
    if finish_queued {
        let _ = timeout(VPN_STREAM_FINISH_TIMEOUT, send.stopped()).await;
    }
    connection.close(0u32.into(), close_message.as_bytes());
    endpoint.close(0u32.into(), close_message.as_bytes());
    endpoint.wait_idle().await;
    worker_send_ipc(&ipc, token, phase, NetworkIpcKind::WorkerExit, exit_code, 0).await?;
    shutdown.map(|_| ())
}
#[cfg(target_os = "linux")]
async fn network_worker_control_loop(
    ipc: Arc<NetworkIpcSocket>,
    token: [u8; 32],
    expected_supervisor: NetworkPeerCredentials,
    counters: UsageVoucherCounters,
) -> Result<TunnelShutdown, ControllerError> {
    let mut phase = WorkerIpcPhase::Running;
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let (ingress, egress) = counters.snapshot();
                worker_send_ipc(
                    &ipc,
                    token,
                    &mut phase,
                    NetworkIpcKind::Traffic,
                    ingress,
                    egress,
                ).await?;
            }
            message = ipc.receive(&token, expected_supervisor) => {
                let message = message?;
                phase = validate_worker_received_frame(
                    phase,
                    message.frame,
                    message.descriptors.len(),
                )?;
                debug_assert_eq!(message.frame.kind, NetworkIpcKind::Stop);
                let (ingress, egress) = counters.snapshot();
                worker_send_ipc(
                    &ipc,
                    token,
                    &mut phase,
                    NetworkIpcKind::Traffic,
                    ingress,
                    egress,
                ).await?;
                return Ok(TunnelShutdown {
                    repair_required: false,
                    message: "idle".to_owned(),
                });
            }
        }
    }
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
    // `relay_addr` is the exact numeric result of one bounded, public-only
    // lookup. Pass it directly to Quinn so no later layer can re-resolve the
    // signed DNS name or follow a rebinding answer.
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
    ensure_soranet_quic_alpn(&connection)?;
    let session = perform_helper_handshake(&connection, payload, helper_ticket).await?;
    let record_layer = RecordLayer::new(session.session_key, RecordEndpoint::Client)
        .map_err(|error| ControllerError::Handshake(error.to_string()))?;
    Ok((endpoint, connection, Arc::new(record_layer)))
}
async fn resolve_multiaddr_socket_addr(
    relay: &ParsedMultiaddr,
) -> Result<SocketAddr, ControllerError> {
    match &relay.host {
        ParsedMultiaddrHost::Ip(host) => {
            if !is_public_relay_ip(*host) {
                return Err(ControllerError::InvalidMultiaddr(
                    "relay IP is not globally routable".to_owned(),
                ));
            }
            Ok(SocketAddr::new(*host, relay.port))
        }
        ParsedMultiaddrHost::Dns {
            name,
            address_family,
        } => {
            let mut answers = Vec::new();
            let resolved = timeout(
                RELAY_DNS_RESOLUTION_TIMEOUT,
                lookup_host((name.as_str(), relay.port)),
            )
            .await
            .map_err(|_| {
                ControllerError::State(format!(
                    "timed out resolving signed VPN relay DNS name {name}"
                ))
            })??;
            for answer in resolved {
                if answers.len() == MAX_RELAY_DNS_ANSWERS_V1 {
                    return Err(ControllerError::InvalidMultiaddr(format!(
                        "dns {name} returned more than {MAX_RELAY_DNS_ANSWERS_V1} answers"
                    )));
                }
                answers.push(answer);
            }
            select_resolved_relay_addr(name, *address_family, relay.port, answers)
        }
    }
}
fn select_resolved_relay_addr(
    name: &str,
    address_family: DnsAddressFamily,
    port: u16,
    answers: impl IntoIterator<Item = SocketAddr>,
) -> Result<SocketAddr, ControllerError> {
    let mut answers = answers.into_iter().collect::<Vec<_>>();
    if answers.is_empty() {
        return Err(ControllerError::InvalidMultiaddr(format!(
            "dns {name} returned no addresses"
        )));
    }
    if answers.len() > MAX_RELAY_DNS_ANSWERS_V1 {
        return Err(ControllerError::InvalidMultiaddr(format!(
            "dns {name} returned more than {MAX_RELAY_DNS_ANSWERS_V1} answers"
        )));
    }
    if answers
        .iter()
        .any(|answer| !is_public_relay_ip(answer.ip()))
    {
        return Err(ControllerError::InvalidMultiaddr(format!(
            "dns {name} returned a private, local, reserved, or documentation address"
        )));
    }
    answers.retain(|answer| match address_family {
        DnsAddressFamily::Any => true,
        DnsAddressFamily::V4 => answer.is_ipv4(),
        DnsAddressFamily::V6 => answer.is_ipv6(),
    });
    answers.sort_unstable();
    answers.dedup();
    answers
        .first()
        .map(|answer| SocketAddr::new(answer.ip(), port))
        .ok_or_else(|| {
            ControllerError::InvalidMultiaddr(format!(
                "dns {name} did not resolve to the signed address family"
            ))
        })
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
fn validate_soranet_quic_alpn(protocol: Option<&[u8]>) -> Result<(), ControllerError> {
    if protocol != Some(SORANET_QUIC_ALPN) {
        return Err(ControllerError::State(
            "relay TLS did not negotiate the exact SoraNet QUIC ALPN".to_owned(),
        ));
    }
    Ok(())
}
fn ensure_soranet_quic_alpn(connection: &Connection) -> Result<(), ControllerError> {
    let handshake = connection.handshake_data().ok_or_else(|| {
        ControllerError::State("relay QUIC connection has no TLS handshake data".to_owned())
    })?;
    let handshake = handshake
        .downcast::<quinn::crypto::rustls::HandshakeData>()
        .map_err(|_| {
            ControllerError::State(
                "relay QUIC connection returned an unexpected TLS handshake type".to_owned(),
            )
        })?;
    validate_soranet_quic_alpn(handshake.protocol.as_deref())
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
    let relay_mldsa65_public_key = parse_relay_mldsa65_public_key_hex(
        payload.relay_mldsa65_public_key_hex.as_str(),
        "relayMlDsa65PublicKeyHex",
    )?;
    let relay_mldsa65_identity = PublicKey::from_bytes(Algorithm::MlDsa, &relay_mldsa65_public_key)
        .map_err(|error| {
            ControllerError::InvalidPayload(format!(
                "relayMlDsa65PublicKeyHex is not a valid ML-DSA-65 key: {error}"
            ))
        })?;
    let relay_certificate_sha256 = parse_canonical_nonzero_hex_32(
        payload.relay_certificate_sha256_hex.as_str(),
        "relayCertificateSha256Hex",
    )?;
    let relay_authentication = RelayAuthenticationVerifierV1::try_new(
        relay_identity,
        relay_mldsa65_identity,
        relay_certificate_sha256,
    )
    .map_err(|error| ControllerError::Handshake(error.to_string()))?;
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
    let session =
        client_handle_relay_hello(client_state, &relay_hello, &relay_authentication, &params)
            .map_err(|error| ControllerError::Handshake(error.to_string()))?;
    send.finish()?;
    Ok(session)
}
fn helper_ticket_handshake_binding(
    payload: &ConnectPayload,
    helper_ticket: &[u8],
) -> Result<[u8; 32], ControllerError> {
    fn update(hasher: &mut Blake3Hasher, value: &[u8]) {
        let len = u64::try_from(value.len())
            .expect("VPN helper handshake fields are protocol-bounded below u64::MAX");
        hasher.update(&len.to_be_bytes());
        hasher.update(value);
    }
    let relay_id = parse_canonical_nonzero_hex_32(payload.relay_id_hex.as_str(), "relayIdHex")?;
    let relay_mldsa65_public_key = parse_relay_mldsa65_public_key_hex(
        payload.relay_mldsa65_public_key_hex.as_str(),
        "relayMlDsa65PublicKeyHex",
    )?;
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
        b"iroha.soranet.vpn.helper-handshake-dual-auth.v1".as_slice(),
        helper_ticket,
        payload.relay_endpoint.as_bytes(),
        relay_id.as_slice(),
        relay_mldsa65_public_key.as_slice(),
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
#[cfg(any(target_os = "linux", test))]
fn ensure_privileged_preparation_deadline_at(
    deadline: Instant,
    now: Instant,
) -> Result<(), ControllerError> {
    if now >= deadline {
        return Err(ControllerError::State(
            "privileged tunnel preparation exceeded its single absolute deadline".to_owned(),
        ));
    }
    Ok(())
}
#[cfg(any(target_os = "linux", test))]
fn privileged_command_deadlines(
    preparation_deadline: Instant,
) -> Result<(Instant, Instant), ControllerError> {
    let execution_deadline = preparation_deadline
        .checked_sub(PROCESS_KILL_REAP_TIMEOUT)
        .ok_or_else(|| {
            ControllerError::State(
                "privileged preparation deadline cannot reserve command cleanup custody".to_owned(),
            )
        })?;
    Ok((execution_deadline, preparation_deadline))
}
#[cfg(any(target_os = "linux", test))]
fn privileged_preparation_deadline_at(
    started: Instant,
    ticket_remaining: Duration,
) -> Result<Instant, ControllerError> {
    let configured_deadline = started
        .checked_add(PRIVILEGED_PREPARATION_TIMEOUT)
        .ok_or_else(|| {
            ControllerError::State(
                "privileged preparation deadline exceeds the monotonic clock range".to_owned(),
            )
        })?;
    let authorization_deadline = started.checked_add(ticket_remaining).ok_or_else(|| {
        ControllerError::State(
            "authenticated ticket deadline exceeds the monotonic clock range".to_owned(),
        )
    })?;
    Ok(configured_deadline.min(authorization_deadline))
}
#[cfg(any(target_os = "linux", test))]
trait NetworkPrepareOps {
    type Device;
    type ExcludedRouteMutation;

    fn check_preparation(&mut self) -> Result<(), ControllerError>;
    fn persist(&mut self, state: &State) -> Result<(), ControllerError>;
    fn create_tun(&mut self, requested_name: &str) -> Result<Self::Device, ControllerError>;
    fn tun_name<'a>(&self, device: &'a Self::Device) -> &'a str;
    fn apply_link(
        &mut self,
        interface_name: &str,
        mtu: u16,
        tunnel_addresses: &[ParsedCidr],
    ) -> Result<(), ControllerError>;
    fn apply_routes(
        &mut self,
        interface_name: &str,
        routes: &[String],
    ) -> Result<(), ControllerError>;
    fn plan_excluded_route(
        &mut self,
        route: &str,
    ) -> Result<(ExcludedRouteSnapshot, Self::ExcludedRouteMutation), ControllerError>;
    fn apply_excluded_route(
        &mut self,
        snapshot: &ExcludedRouteSnapshot,
        mutation: Self::ExcludedRouteMutation,
    ) -> Result<String, ControllerError>;
    fn plan_dns(
        &mut self,
        interface_name: &str,
        dns_servers: &[String],
    ) -> Result<Option<DnsBackendState>, ControllerError>;
    fn apply_dns(
        &mut self,
        interface_name: &str,
        dns_servers: &[String],
        plan: DnsBackendState,
    ) -> Result<DnsBackendState, ControllerError>;
}
#[cfg(target_os = "linux")]
struct SystemNetworkPrepareOps<'a> {
    network_worker: &'a NetworkWorkerProcess,
    ticket_expires_at_ms: u64,
    deadline: Instant,
}
#[cfg(target_os = "linux")]
impl SystemNetworkPrepareOps<'_> {
    fn run_command<I, S>(&mut self, program: &str, args: I) -> Result<String, ControllerError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.check_preparation()?;
        let output = run_command_until(program, args, self.deadline)?;
        self.check_preparation()?;
        Ok(output)
    }
}
#[cfg(target_os = "linux")]
impl NetworkPrepareOps for SystemNetworkPrepareOps<'_> {
    type Device = Arc<LinuxTunDevice>;
    type ExcludedRouteMutation = Vec<String>;

    fn check_preparation(&mut self) -> Result<(), ControllerError> {
        ensure_privileged_preparation_deadline_at(self.deadline, Instant::now())?;
        ensure_authenticated_ticket_unexpired_at(self.ticket_expires_at_ms)?;
        if pidfd_wait_readable(&self.network_worker.pidfd, Duration::ZERO)?
            || !pidfd_send_signal(&self.network_worker.pidfd, 0)?
        {
            return Err(ControllerError::State(format!(
                "unprivileged network worker {} exited during privileged tunnel preparation",
                self.network_worker.identity.pid
            )));
        }
        Ok(())
    }

    fn persist(&mut self, state: &State) -> Result<(), ControllerError> {
        self.check_preparation()?;
        persist_state(state)?;
        self.check_preparation()
    }

    fn create_tun(&mut self, requested_name: &str) -> Result<Self::Device, ControllerError> {
        self.check_preparation()?;
        let device = Arc::new(LinuxTunDevice::create(requested_name)?);
        ensure_exact_tun_interface_name(requested_name, device.name())?;
        self.check_preparation()?;
        Ok(device)
    }

    fn tun_name<'a>(&self, device: &'a Self::Device) -> &'a str {
        device.name()
    }

    fn apply_link(
        &mut self,
        interface_name: &str,
        mtu: u16,
        tunnel_addresses: &[ParsedCidr],
    ) -> Result<(), ControllerError> {
        apply_tunnel_link_config_with(interface_name, mtu, tunnel_addresses, |program, args| {
            self.run_command(program, args)
        })
    }

    fn apply_routes(
        &mut self,
        interface_name: &str,
        routes: &[String],
    ) -> Result<(), ControllerError> {
        apply_route_pushes_with(interface_name, routes, |program, args| {
            self.run_command(program, args)
        })
    }

    fn plan_excluded_route(
        &mut self,
        route: &str,
    ) -> Result<(ExcludedRouteSnapshot, Self::ExcludedRouteMutation), ControllerError> {
        plan_excluded_route_mutation_with(route, |program, args| self.run_command(program, args))
    }

    fn apply_excluded_route(
        &mut self,
        snapshot: &ExcludedRouteSnapshot,
        mutation: Self::ExcludedRouteMutation,
    ) -> Result<String, ControllerError> {
        self.run_command(DEFAULT_ROUTE_CMD, mutation.clone())?;
        let installed =
            capture_existing_route_with(snapshot.family, &snapshot.cidr, |program, args| {
                self.run_command(program, args)
            })?
            .ok_or_else(|| {
                ControllerError::State(format!(
                    "excluded route {} was absent immediately after successful installation",
                    snapshot.cidr
                ))
            })?;
        validate_installed_excluded_route(snapshot, &mutation, &installed)?;
        Ok(installed)
    }

    fn plan_dns(
        &mut self,
        interface_name: &str,
        dns_servers: &[String],
    ) -> Result<Option<DnsBackendState>, ControllerError> {
        self.check_preparation()?;
        plan_dns_backend(interface_name, dns_servers)
    }

    fn apply_dns(
        &mut self,
        interface_name: &str,
        dns_servers: &[String],
        plan: DnsBackendState,
    ) -> Result<DnsBackendState, ControllerError> {
        apply_dns_plan_with(interface_name, dns_servers, plan, |program, args| {
            self.run_command(program, args)
        })
    }
}
#[cfg(any(target_os = "linux", test))]
fn persist_network_prepare_journal<O: NetworkPrepareOps>(
    state: &mut State,
    applied_network: &AppliedNetworkState,
    message: &str,
    operations: &mut O,
) -> Result<(), ControllerError> {
    // Publish the in-memory phase only after its durable write succeeds. On a write failure the
    // caller and a later process both retain the same last durable repair plan.
    let mut next = state.clone();
    next.active = false;
    next.repair_required = true;
    next.interface_name = Some(applied_network.interface_name.clone());
    next.network_service = applied_network.dns_backend.as_ref().map(dns_backend_label);
    next.applied_network = Some(applied_network.clone());
    next.message = message.to_owned();
    operations.persist(&next)?;
    *state = next;
    Ok(())
}
#[cfg(target_os = "linux")]
fn prepare_tunnel(
    payload: &AuthenticatedPrivilegedNetworkPlan,
    state: &mut State,
    network_worker: &NetworkWorkerProcess,
) -> Result<PreparedTunnel, ControllerError> {
    // Pin the remaining signed lifetime to the monotonic clock once. A later wall-clock rollback
    // must not extend root mutation authority, and no system command may finish after this bound.
    let started = Instant::now();
    let ticket_remaining =
        authenticated_ticket_expiry_remaining_at(payload.ticket_expires_at_ms, unix_now_ms()?)?;
    let deadline = privileged_preparation_deadline_at(started, ticket_remaining)?;
    let mut operations = SystemNetworkPrepareOps {
        network_worker,
        ticket_expires_at_ms: payload.ticket_expires_at_ms,
        deadline,
    };
    prepare_tunnel_with(payload, state, &mut operations)
}
#[cfg(any(target_os = "linux", test))]
fn prepare_tunnel_with<O: NetworkPrepareOps>(
    payload: &AuthenticatedPrivilegedNetworkPlan,
    state: &mut State,
    operations: &mut O,
) -> Result<PreparedTunnel<O::Device>, ControllerError> {
    operations.check_preparation()?;
    let interface_name = desired_interface_name(payload.session_id.as_str())?;
    let mtu = normalize_mtu(payload.mtu_bytes)?;
    let tunnel_addresses = parse_tunnel_addresses(&payload.tunnel_addresses)?;
    let planned_dns = operations.plan_dns(&interface_name, &payload.dns_servers)?;
    operations.check_preparation()?;
    let mut applied_network = AppliedNetworkState {
        interface_name: interface_name.clone(),
        journal_phase: NetworkJournalPhase::Planned,
        dns_backend: None,
        excluded_route_snapshots: Vec::new(),
    };
    // Prove every excluded exact prefix absent against the pre-VPN route table and derive its
    // exclusive add before pushing any tunnel route. In particular, a pushed default must not
    // become its own bypass gateway. The complete cleanup intent is durable before the first TUN
    // or route mutation; ambient exact routes are conflicts rather than state we replace.
    let mut excluded_route_mutations = Vec::with_capacity(payload.excluded_routes.len());
    for route in &payload.excluded_routes {
        operations.check_preparation()?;
        let (snapshot, mutation) = operations.plan_excluded_route(route)?;
        operations.check_preparation()?;
        applied_network.excluded_route_snapshots.push(snapshot);
        excluded_route_mutations.push(mutation);
    }
    // This durable repair plan precedes the first TUN ioctl or host-network mutation. The TUN and
    // its link-local routes disappear when the supervisor fd closes; every host-global mutation
    // already has enough journaled state to be undone after a crash.
    persist_network_prepare_journal(state, &applied_network, "preparing tunnel", operations)?;
    operations.check_preparation()?;

    let device = operations.create_tun(&interface_name)?;
    operations.check_preparation()?;
    applied_network.interface_name = operations.tun_name(&device).to_owned();
    applied_network.journal_phase = NetworkJournalPhase::TunCreated;
    persist_network_prepare_journal(state, &applied_network, "created tunnel device", operations)?;
    operations.check_preparation()?;

    operations.apply_link(&applied_network.interface_name, mtu, &tunnel_addresses)?;
    operations.check_preparation()?;
    applied_network.journal_phase = NetworkJournalPhase::LinkConfigured;
    persist_network_prepare_journal(
        state,
        &applied_network,
        "configured tunnel link",
        operations,
    )?;
    operations.check_preparation()?;

    operations.apply_routes(&applied_network.interface_name, &payload.route_pushes)?;
    operations.check_preparation()?;
    applied_network.journal_phase = NetworkJournalPhase::RoutesConfigured;
    persist_network_prepare_journal(
        state,
        &applied_network,
        "configured tunnel routes",
        operations,
    )?;
    operations.check_preparation()?;

    for (index, mutation) in excluded_route_mutations.into_iter().enumerate() {
        operations.check_preparation()?;
        let snapshot = applied_network.excluded_route_snapshots[index].clone();
        let installed_route = operations.apply_excluded_route(&snapshot, mutation)?;
        operations.check_preparation()?;
        applied_network.excluded_route_snapshots[index].installed_route = Some(installed_route);
        applied_network.journal_phase = NetworkJournalPhase::ConfiguringExcludedRoutes;
        persist_network_prepare_journal(
            state,
            &applied_network,
            "journaled exact installed excluded route",
            operations,
        )?;
        operations.check_preparation()?;
    }
    applied_network.journal_phase = NetworkJournalPhase::ExcludedRoutesConfigured;
    persist_network_prepare_journal(
        state,
        &applied_network,
        "configured excluded routes",
        operations,
    )?;
    operations.check_preparation()?;

    if let Some(planned_dns) = planned_dns {
        applied_network.dns_backend = Some(planned_dns.clone());
        applied_network.journal_phase = NetworkJournalPhase::DnsPlanned;
        persist_network_prepare_journal(
            state,
            &applied_network,
            "journaled DNS mutation",
            operations,
        )?;
        operations.check_preparation()?;
        let applied_dns = operations.apply_dns(
            &applied_network.interface_name,
            &payload.dns_servers,
            planned_dns,
        )?;
        operations.check_preparation()?;
        applied_network.dns_backend = Some(applied_dns);
    }
    applied_network.journal_phase = NetworkJournalPhase::Prepared;
    persist_network_prepare_journal(
        state,
        &applied_network,
        "tunnel prepared; awaiting worker",
        operations,
    )?;
    operations.check_preparation()?;

    Ok(PreparedTunnel {
        device,
        interface_name: applied_network.interface_name.clone(),
        network_service: applied_network.dns_backend.as_ref().map(dns_backend_label),
        applied_network,
        packet_read_mtu: usize::from(mtu),
    })
}
fn cleanup_tunnel(prepared: PreparedTunnel) -> Result<(), ControllerError> {
    drop(prepared);
    Ok(())
}
#[cfg(target_os = "linux")]
fn cleanup_prepared_tunnel(
    prepared: CleanupGuard<PreparedTunnel>,
    state: &mut State,
    network_worker: &NetworkWorkerProcess,
) -> Result<(), ControllerError> {
    if !network_worker.is_reaped() {
        // Never restore global routes/DNS while an unverified child could still hold the passed
        // TUN descriptor. Closing the supervisor copy is safe; the durable journal remains for a
        // later repair after process death is established.
        drop(prepared.take());
        return Err(ControllerError::State(
            "global network cleanup deferred because the network worker was not exactly reaped"
                .to_owned(),
        ));
    }
    // Closing the supervisor copy happens only after the exact network child is reaped, so this
    // drop releases the final TUN custody before any global DNS/route restoration begins.
    state.network_worker_identity = None;
    drop(prepared.take());
    cleanup_persisted_network(state)
}
#[cfg(target_os = "linux")]
fn finish_prepared_tunnel(
    prepared: CleanupGuard<PreparedTunnel>,
    state: &mut State,
    network_worker: &NetworkWorkerProcess,
    message: String,
) -> Result<(), ControllerError> {
    let cleanup_result = cleanup_prepared_tunnel(prepared, state, network_worker);
    state.active = false;
    match cleanup_result {
        Ok(()) => {
            state.worker_identity = None;
            clear_session_binding(state);
            persist_state(state)
        }
        Err(cleanup_error) => {
            // `cleanup_prepared_tunnel` never touches global routes/DNS until the exact child is
            // reaped. Keep the supervisor identity, any live network identity, owner, and journal
            // durable so a later owner-authorized repair can prove process death before retrying.
            state.repair_required = true;
            state.message = format!("{message}; cleanup deferred or failed: {cleanup_error}");
            persist_state(state).map_err(|persist_error| {
                ControllerError::State(format!(
                    "{}; failed to persist repair state: {persist_error}",
                    state.message
                ))
            })?;
            Err(cleanup_error)
        }
    }
}
impl TunnelShutdownSignals {
    fn install() -> Result<Self, ControllerError> {
        Ok(Self {
            sigterm: signal(SignalKind::terminate())?,
            sigint: signal(SignalKind::interrupt())?,
        })
    }
}
async fn tunnel_packet_loop<W, R, C>(
    device: Arc<LinuxTunDevice>,
    send: &mut W,
    recv: &mut R,
    traffic: TunnelTrafficConfig,
    expiry_deadline: tokio::time::Instant,
    voucher_signer: UsageVoucherSigner,
    voucher_counters: UsageVoucherCounters,
    control: C,
) -> Result<TunnelShutdown, ControllerError>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
    C: Future<Output = Result<TunnelShutdown, ControllerError>>,
{
    let expiry = tokio::time::sleep_until(expiry_deadline);
    let upstream = tun_to_vpn_loop(
        Arc::clone(&device),
        send,
        traffic,
        voucher_signer,
        voucher_counters.clone(),
    );
    let downstream = vpn_to_tun_loop(device, recv, voucher_counters, traffic.packet_read_mtu);
    tokio::pin!(expiry);
    tokio::pin!(upstream);
    tokio::pin!(downstream);
    tokio::pin!(control);
    tokio::select! {
        _ = &mut expiry => Ok(TunnelShutdown {
            repair_required: false,
            message: "authenticated helper ticket expired".to_owned(),
        }),
        result = &mut control => result,
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
    mut voucher_signer: UsageVoucherSigner,
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
        ..
    } = traffic;
    let mut packet_buf = vec![0u8; packet_read_mtu.max(512)];
    let mut sequence = 0u64;
    send_usage_voucher_control_cell(
        send,
        circuit_id,
        flow_label,
        padding_budget_ms,
        &voucher_counters,
        &mut voucher_signer,
        &mut sequence,
    )
    .await?;
    let first_voucher_deadline = tokio::time::Instant::now()
        .checked_add(voucher_signer.interval)
        .ok_or_else(|| ControllerError::State("usage voucher deadline overflow".to_owned()))?;
    let mut voucher_interval =
        tokio::time::interval_at(first_voucher_deadline, voucher_signer.interval);
    voucher_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            packet = device.recv(&mut packet_buf) => {
                let packet_len = packet?;
                if packet_len == 0 {
                    continue;
                }
                let packet_len_u64 = packet_len as u64;
                if voucher_counters.refresh_before_ingress(packet_len_u64) {
                    send_usage_voucher_control_cell(
                        send,
                        circuit_id,
                        flow_label,
                        padding_budget_ms,
                        &voucher_counters,
                        &mut voucher_signer,
                        &mut sequence,
                    ).await?;
                }
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
                    write_vpn_frame(send, &padded).await?;
                    sequence = sequence.saturating_add(1);
                }
                voucher_counters.record_client_to_relay(packet_len_u64);
            }
            _ = voucher_interval.tick() => {
                send_usage_voucher_control_cell(
                    send,
                    circuit_id,
                    flow_label,
                    padding_budget_ms,
                    &voucher_counters,
                    &mut voucher_signer,
                    &mut sequence,
                ).await?;
            }
            _ = voucher_counters.refresh_requested() => {
                send_usage_voucher_control_cell(
                    send,
                    circuit_id,
                    flow_label,
                    padding_budget_ms,
                    &voucher_counters,
                    &mut voucher_signer,
                    &mut sequence,
                ).await?;
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
fn record_relay_packet_after_tun_write(
    voucher_counters: &UsageVoucherCounters,
    packet_len: usize,
    written: usize,
) -> Result<u64, ControllerError> {
    if written != packet_len {
        return Err(ControllerError::State(format!(
            "partial TUN packet write: wrote {written} of {packet_len} bytes"
        )));
    }
    let packet_len = u64::try_from(packet_len).map_err(|_| {
        ControllerError::State("TUN packet length exceeds the usage counter range".to_owned())
    })?;
    voucher_counters.record_relay_to_client(packet_len);
    Ok(packet_len)
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
    loop {
        let mut frame = VpnPaddedCellV1::zeroed();
        match read_exact_or_eof(recv, frame.as_mut()).await {
            Ok(true) => {
                let cell = frame.parse_with_flow_label_bits(VpnFlowLabelV1::MAX_BITS)?;
                drop(frame);
                if cell.header.class != VpnCellClassV1::Data {
                    continue;
                }
                for packet in decoder.ingest(&cell.payload)? {
                    let written = device.send(&packet).await?;
                    record_relay_packet_after_tun_write(&voucher_counters, packet.len(), written)?;
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
    fn from_payload(
        payload: &ConnectPayload,
        ticket: VpnHelperTicketV1,
    ) -> Result<Self, ControllerError> {
        let mut seed = parse_canonical_secret_hex_32(
            payload.metering_private_key_seed_hex.as_str(),
            "meteringPrivateKeySeedHex",
        )?;
        let expected_session_id = parse_canonical_session_id(payload.session_id.as_str())?;
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
        Ok(Self {
            key_pair,
            ticket,
            sequence: 0,
            started_at: now,
            interval: USAGE_VOUCHER_INTERVAL,
            authorized_active_ms: 0,
        })
    }
    fn begin_service(&mut self) {
        debug_assert_eq!(self.sequence, 0);
        self.started_at = Instant::now();
    }
    fn build_envelope(
        &mut self,
        counters: &UsageVoucherCounters,
    ) -> Result<VpnUsageVoucherEnvelopeV1, ControllerError> {
        let (ingress_bytes, egress_bytes) = counters.snapshot();
        let observed_active_ms = self
            .started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let ingress_bytes = ingress_bytes.saturating_add(USAGE_VOUCHER_BYTE_CREDIT_WINDOW);
        let egress_bytes = egress_bytes.saturating_add(USAGE_VOUCHER_BYTE_CREDIT_WINDOW);
        let active_ms = observed_active_ms
            .saturating_add(USAGE_VOUCHER_ACTIVE_CREDIT_MS)
            .max(self.authorized_active_ms);
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
        let voucher = VpnUsageVoucherV1::try_sign(body, self.key_pair.private_key())?;
        let fee_ceiling = self
            .ticket
            .tariff
            .fee_ceiling(&voucher.body)
            .map_err(|error| {
                ControllerError::State(format!("usage voucher tariff arithmetic failed: {error}"))
            })?;
        counters.set_authorization(ingress_bytes, egress_bytes);
        self.authorized_active_ms = active_ms;
        self.sequence = self.sequence.saturating_add(1);
        Ok(VpnUsageVoucherEnvelopeV1 {
            voucher,
            fee_ceiling,
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
    write_vpn_frame(send, &padded).await?;
    *sequence = (*sequence).saturating_add(1);
    Ok(())
}
async fn write_vpn_frame<W>(send: &mut W, frame: &VpnPaddedCellV1) -> Result<(), ControllerError>
where
    W: AsyncWrite + Unpin,
{
    send.write_all(frame.as_ref()).await?;
    // `RecordWriter` buffers the last accepted authenticated record. Flush at
    // every cell boundary so the initial voucher and a packet's final cell are
    // visible without waiting for another write or tunnel shutdown.
    send.flush().await?;
    Ok(())
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
    #[cfg(target_os = "linux")]
    fn from_received_fd(
        fd: OwnedFd,
        expected_name: &str,
        expected_mtu: u16,
    ) -> Result<Self, ControllerError> {
        // SAFETY: each fcntl call only inspects descriptor/status flags on the owned fd.
        let descriptor_flags = unsafe { nix::libc::fcntl(fd.as_raw_fd(), nix::libc::F_GETFD) };
        let status_flags = unsafe { nix::libc::fcntl(fd.as_raw_fd(), nix::libc::F_GETFL) };
        if descriptor_flags < 0 || status_flags < 0 {
            return Err(io::Error::last_os_error().into());
        }
        if descriptor_flags & nix::libc::FD_CLOEXEC == 0
            || status_flags & nix::libc::O_NONBLOCK == 0
            || status_flags & nix::libc::O_ACCMODE != nix::libc::O_RDWR
        {
            return Err(ControllerError::State(
                "received TUN descriptor lacks CLOEXEC, nonblocking, or read/write custody"
                    .to_owned(),
            ));
        }
        // SAFETY: `stat` is writable storage for the metadata of the owned descriptor.
        let mut stat = unsafe { std::mem::zeroed::<nix::libc::stat>() };
        if unsafe { nix::libc::fstat(fd.as_raw_fd(), &mut stat) } != 0 {
            return Err(io::Error::last_os_error().into());
        }
        let tun_metadata = fs::metadata("/dev/net/tun")?;
        if stat.st_mode & nix::libc::S_IFMT != nix::libc::S_IFCHR
            || !tun_metadata.file_type().is_char_device()
            || stat.st_rdev != tun_metadata.rdev()
            || stat.st_dev != tun_metadata.dev()
            || stat.st_ino != tun_metadata.ino()
        {
            return Err(ControllerError::State(
                "received descriptor is not the expected /dev/net/tun character device".to_owned(),
            ));
        }
        let mut request = unsafe { std::mem::zeroed::<nix::libc::ifreq>() };
        // SAFETY: `TUNGETIFF` writes one initialized ifreq for this live TUN descriptor.
        if unsafe { nix::libc::ioctl(fd.as_raw_fd(), LINUX_TUNGETIFF as _, &mut request) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: the kernel NUL-terminates the fixed-size interface-name field.
        let kernel_name = unsafe { CStr::from_ptr(request.ifr_name.as_ptr()) }
            .to_str()
            .map_err(|error| {
                ControllerError::State(format!(
                    "received TUN descriptor has a non-UTF-8 interface name: {error}"
                ))
            })?
            .to_owned();
        ensure_exact_tun_interface_name(expected_name, &kernel_name)?;
        // SAFETY: TUNGETIFF initialized the flags member of the ifreq union.
        let flags = unsafe { request.ifr_ifru.ifru_flags } as u16;
        ensure_exact_tun_runtime_flags(&kernel_name, flags)?;
        let kernel_mtu = linux_interface_mtu(&kernel_name)?;
        if kernel_mtu != expected_mtu {
            return Err(ControllerError::State(format!(
                "received interface {kernel_name} MTU {kernel_mtu} does not match signed MTU {expected_mtu}"
            )));
        }
        let file: fs::File = fd.into();
        Ok(Self {
            file: AsyncFd::new(file)?,
            name: kernel_name,
        })
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
fn linux_interface_mtu(interface_name: &str) -> Result<u16, ControllerError> {
    if interface_name.is_empty() || interface_name.len() >= nix::libc::IFNAMSIZ {
        return Err(ControllerError::State(
            "invalid interface name for MTU inspection".to_owned(),
        ));
    }
    // SAFETY: socket returns a fresh descriptor or a negative errno result.
    let raw_socket = unsafe {
        nix::libc::socket(
            nix::libc::AF_INET,
            nix::libc::SOCK_DGRAM | nix::libc::SOCK_CLOEXEC,
            0,
        )
    };
    if raw_socket < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: the successful socket call returned a fresh owned descriptor.
    let socket = unsafe { OwnedFd::from_raw_fd(raw_socket) };
    let mut request = unsafe { std::mem::zeroed::<nix::libc::ifreq>() };
    // SAFETY: both buffers are valid and non-overlapping for the validated interface-name width.
    unsafe {
        std::ptr::copy_nonoverlapping(
            interface_name.as_ptr().cast::<nix::libc::c_char>(),
            request.ifr_name.as_mut_ptr(),
            interface_name.len(),
        );
    }
    // SAFETY: SIOCGIFMTU writes the MTU member for the named interface.
    if unsafe { nix::libc::ioctl(socket.as_raw_fd(), nix::libc::SIOCGIFMTU as _, &mut request) } < 0
    {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: SIOCGIFMTU initialized the MTU union member.
    let mtu = unsafe { request.ifr_ifru.ifru_mtu };
    u16::try_from(mtu).map_err(|_| {
        ControllerError::State(format!(
            "kernel returned out-of-range MTU {mtu} for {interface_name}"
        ))
    })
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
fn ensure_exact_tun_runtime_flags(interface_name: &str, flags: u16) -> Result<(), ControllerError> {
    let expected = LINUX_IFF_TUN_BITS | LINUX_IFF_NO_PI_BITS;
    if flags != expected {
        return Err(ControllerError::State(format!(
            "received interface {interface_name} has TUN flags {flags:#06x}, expected exactly IFF_TUN|IFF_NO_PI ({expected:#06x})"
        )));
    }
    Ok(())
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
trait NetworkCleanupOps {
    fn persist(&mut self, state: &State) -> Result<(), ControllerError>;
    fn revert_resolved(&mut self, interface_name: &str) -> Result<(), ControllerError>;
    fn restore_excluded_route(
        &mut self,
        snapshot: &ExcludedRouteSnapshot,
    ) -> Result<(), ControllerError>;
}
struct SystemNetworkCleanupOps;
impl NetworkCleanupOps for SystemNetworkCleanupOps {
    fn persist(&mut self, state: &State) -> Result<(), ControllerError> {
        persist_state(state)
    }

    fn revert_resolved(&mut self, interface_name: &str) -> Result<(), ControllerError> {
        cleanup_resolved_dns(interface_name)
    }

    fn restore_excluded_route(
        &mut self,
        snapshot: &ExcludedRouteSnapshot,
    ) -> Result<(), ControllerError> {
        restore_excluded_route(snapshot)
    }
}
fn persist_cleanup_transition<O: NetworkCleanupOps>(
    state: &mut State,
    operations: &mut O,
    mutate: impl FnOnce(&mut State),
) -> Result<(), ControllerError> {
    // Do not publish an in-memory transition until its durable write succeeds. A caller can retry
    // this exact state after a persist failure, while a process restart observes the same prior
    // step from disk. Every external cleanup operation below is therefore deliberately
    // idempotent.
    let mut next = state.clone();
    mutate(&mut next);
    operations.persist(&next)?;
    *state = next;
    Ok(())
}
fn cleanup_persisted_network_with<O: NetworkCleanupOps>(
    state: &mut State,
    operations: &mut O,
) -> Result<(), ControllerError> {
    if state.applied_network.is_none() {
        return Ok(());
    }

    persist_cleanup_transition(state, operations, |next| {
        next.active = false;
        next.repair_required = true;
        next.message = "cleaning up privileged network state".to_owned();
        if let Some(applied) = next.applied_network.as_mut() {
            applied.journal_phase = if applied.dns_backend.is_some() {
                NetworkJournalPhase::CleaningDns
            } else {
                NetworkJournalPhase::CleaningRoutes
            };
        }
    })?;

    loop {
        let dns_backend = state
            .applied_network
            .as_ref()
            .and_then(|applied| applied.dns_backend.clone());
        match dns_backend {
            None => break,
            Some(DnsBackendState::Resolved { interface_name }) => {
                operations.revert_resolved(&interface_name)?;
                persist_cleanup_transition(state, operations, |next| {
                    next.applied_network
                        .as_mut()
                        .expect("cleanup journal remains present")
                        .dns_backend = Some(DnsBackendState::ResolvedReverted { interface_name });
                })?;
            }
            Some(DnsBackendState::ResolvedReverted { .. }) => {
                persist_cleanup_transition(state, operations, |next| {
                    next.applied_network
                        .as_mut()
                        .expect("cleanup journal remains present")
                        .dns_backend = None;
                    next.network_service = None;
                })?;
            }
        }
    }

    persist_cleanup_transition(state, operations, |next| {
        next.applied_network
            .as_mut()
            .expect("cleanup journal remains present")
            .journal_phase = NetworkJournalPhase::CleaningRoutes;
    })?;
    while let Some(snapshot) = state
        .applied_network
        .as_ref()
        .and_then(|applied| applied.excluded_route_snapshots.last())
        .cloned()
    {
        operations.restore_excluded_route(&snapshot)?;
        persist_cleanup_transition(state, operations, |next| {
            let removed = next
                .applied_network
                .as_mut()
                .expect("cleanup journal remains present")
                .excluded_route_snapshots
                .pop()
                .expect("cleanup route remains present");
            debug_assert_eq!(removed, snapshot);
        })?;
    }

    persist_cleanup_transition(state, operations, |next| {
        next.applied_network = None;
        next.interface_name = None;
        next.network_service = None;
        next.active = false;
        next.repair_required = false;
        next.message = "privileged network cleanup complete".to_owned();
    })
}
fn cleanup_persisted_network(state: &mut State) -> Result<(), ControllerError> {
    #[cfg(target_os = "linux")]
    quiesce_system_command_cgroup_until(Instant::now() + PROCESS_KILL_REAP_TIMEOUT)?;
    cleanup_persisted_network_with(state, &mut SystemNetworkCleanupOps)
}
fn apply_tunnel_link_config_with<F>(
    interface_name: &str,
    mtu: u16,
    tunnel_addresses: &[ParsedCidr],
    mut run: F,
) -> Result<(), ControllerError>
where
    F: FnMut(&str, Vec<String>) -> Result<String, ControllerError>,
{
    run(
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
        run(
            DEFAULT_ROUTE_CMD,
            tunnel_address_add_args(interface_name, *address),
        )?;
    }
    Ok(())
}
fn apply_route_pushes_with<F>(
    interface_name: &str,
    routes: &[String],
    mut run: F,
) -> Result<(), ControllerError>
where
    F: FnMut(&str, Vec<String>) -> Result<String, ControllerError>,
{
    for route in routes {
        let parsed = parse_cidr(route)?;
        run(
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
fn plan_excluded_route_mutation_with<F>(
    route: &str,
    mut run: F,
) -> Result<(ExcludedRouteSnapshot, Vec<String>), ControllerError>
where
    F: FnMut(&str, Vec<String>) -> Result<String, ControllerError>,
{
    let normalized = route.trim().to_owned();
    let parsed = parse_cidr(&normalized)?;
    if capture_existing_route_with(parsed.family(), &normalized, &mut run)?.is_some() {
        return Err(ControllerError::State(format!(
            "cannot install excluded route {normalized}: an exact ambient route already exists"
        )));
    }
    let default_route = capture_default_route_with(parsed.family(), &mut run)?;
    let Some((via, dev)) = default_route else {
        return Err(ControllerError::State(format!(
            "cannot install excluded route {normalized}: no system default route for {}",
            match parsed.family() {
                IpFamily::V4 => "IPv4",
                IpFamily::V6 => "IPv6",
            }
        )));
    };
    if let Some(gateway) = via.as_deref() {
        let gateway_address = gateway.parse::<IpAddr>().map_err(|_| {
            ControllerError::State(format!(
                "cannot install excluded route {normalized}: default gateway is not an IP address"
            ))
        })?;
        let gateway_family = match gateway_address {
            IpAddr::V4(_) => IpFamily::V4,
            IpAddr::V6(_) => IpFamily::V6,
        };
        if gateway_family != parsed.family() {
            return Err(ControllerError::State(format!(
                "cannot install excluded route {normalized}: default gateway has the wrong address family"
            )));
        }
    }
    if dev
        .as_deref()
        .is_some_and(|device| matches!(device, "via" | "dev" | "proto"))
    {
        return Err(ControllerError::State(format!(
            "cannot install excluded route {normalized}: default device name collides with route syntax"
        )));
    }
    let mut command = vec![
        parsed.family().flag().to_owned(),
        "route".to_owned(),
        "add".to_owned(),
        normalized.clone(),
    ];
    if let Some(via) = via {
        command.push("via".to_owned());
        command.push(via);
    }
    if let Some(dev) = dev {
        command.push("dev".to_owned());
        command.push(dev);
    }
    if !command.iter().any(|argument| argument == "dev") {
        return Err(ControllerError::State(format!(
            "cannot install excluded route {normalized}: default route has no device"
        )));
    }
    // Numeric protocol 186 is reserved by this first-release helper contract as an ownership
    // marker. Cleanup still requires the entire exact readback, not merely this marker.
    command.push("proto".to_owned());
    command.push(EXCLUDED_ROUTE_PROTOCOL_V1.to_owned());
    let planned_ownership = format!(
        "{PLANNED_EXCLUDED_ROUTE_PREFIX_V1}{}",
        command[3..].join(" ")
    );
    Ok((
        ExcludedRouteSnapshot {
            cidr: normalized,
            family: parsed.family(),
            installed_route: Some(planned_ownership),
        },
        command,
    ))
}
fn capture_default_route_with<F>(
    family: IpFamily,
    mut run: F,
) -> Result<Option<RouteViaDev>, ControllerError>
where
    F: FnMut(&str, Vec<String>) -> Result<String, ControllerError>,
{
    let output = run(
        DEFAULT_ROUTE_CMD,
        vec![
            family.flag().to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[0].to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[1].to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[2].to_owned(),
            "default".to_owned(),
        ],
    )?;
    let Some(line) = output.lines().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };
    Ok(Some(parse_route_via_dev(line)))
}
fn capture_existing_route_with<F>(
    family: IpFamily,
    cidr: &str,
    mut run: F,
) -> Result<Option<String>, ControllerError>
where
    F: FnMut(&str, Vec<String>) -> Result<String, ControllerError>,
{
    let output = run(
        DEFAULT_ROUTE_CMD,
        vec![
            family.flag().to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[0].to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[1].to_owned(),
            DEFAULT_ROUTE_SHOW_PREFIX[2].to_owned(),
            cidr.to_owned(),
        ],
    )?;
    exact_route_readback(&output, cidr)
}
fn capture_existing_route(family: IpFamily, cidr: &str) -> Result<Option<String>, ControllerError> {
    capture_existing_route_with(family, cidr, |program, args| run_command(program, args))
}
fn exact_route_readback(output: &str, cidr: &str) -> Result<Option<String>, ControllerError> {
    if output.as_bytes().contains(&0) {
        return Err(ControllerError::State(format!(
            "route readback for {cidr} contains an embedded NUL"
        )));
    }
    let mut routes = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let Some(route) = routes.next() else {
        return Ok(None);
    };
    if routes.next().is_some() {
        return Err(ControllerError::State(format!(
            "route readback for {cidr} is ambiguous"
        )));
    }
    if !route_destination_matches_cidr(route, cidr) {
        return Err(ControllerError::State(format!(
            "route readback for {cidr} returned a different destination"
        )));
    }
    Ok(Some(route.to_owned()))
}
fn route_destination_matches_cidr(route: &str, cidr: &str) -> bool {
    let Ok(expected) = parse_cidr(cidr) else {
        return false;
    };
    let Some(destination) = route.split_ascii_whitespace().next() else {
        return false;
    };
    if destination == "default" {
        return expected.prefix == 0 && expected.address.is_unspecified();
    }
    let actual = if destination.contains('/') {
        parse_cidr(destination).ok()
    } else {
        destination
            .parse::<IpAddr>()
            .ok()
            .map(|address| ParsedCidr {
                prefix: match address {
                    IpAddr::V4(_) => 32,
                    IpAddr::V6(_) => 128,
                },
                address,
            })
    };
    actual == Some(expected)
}
fn route_has_exact_field(route: &str, field: &str, value: &str) -> bool {
    route
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair[0] == field && pair[1] == value)
}
fn unique_route_field_value<'a>(
    route: &'a str,
    field: &str,
) -> Result<Option<&'a str>, ControllerError> {
    let mut tokens = route.split_ascii_whitespace();
    let mut value = None;
    while let Some(token) = tokens.next() {
        if token != field {
            continue;
        }
        let next = tokens.next().ok_or_else(|| {
            ControllerError::State(format!("route has a truncated {field} field"))
        })?;
        if value.replace(next).is_some() {
            return Err(ControllerError::State(format!(
                "route has duplicate {field} fields"
            )));
        }
    }
    Ok(value)
}
fn validate_precommitted_excluded_route(
    snapshot: &ExcludedRouteSnapshot,
    planned_route: &str,
    current_route: &str,
) -> Result<(), ControllerError> {
    let mut tokens = planned_route.split_ascii_whitespace().peekable();
    let Some(destination) = tokens.next() else {
        return Err(ControllerError::State(format!(
            "excluded route {} has an empty precommitted ownership proof",
            snapshot.cidr
        )));
    };
    if !route_destination_matches_cidr(destination, &snapshot.cidr) {
        return Err(ControllerError::State(format!(
            "excluded route {} has a precommit for another destination",
            snapshot.cidr
        )));
    }
    let planned_via = if tokens.peek() == Some(&"via") {
        let _ = tokens.next();
        let value = tokens.next().ok_or_else(|| {
            ControllerError::State("precommitted excluded route has truncated via field".to_owned())
        })?;
        Some(value)
    } else {
        None
    };
    if tokens.next() != Some("dev") {
        return Err(ControllerError::State(format!(
            "excluded route {} precommit lacks its device",
            snapshot.cidr
        )));
    }
    let planned_dev = tokens.next().ok_or_else(|| {
        ControllerError::State("precommitted excluded route has truncated dev field".to_owned())
    })?;
    if tokens.next() != Some("proto")
        || tokens.next() != Some(EXCLUDED_ROUTE_PROTOCOL_V1)
        || tokens.next().is_some()
    {
        return Err(ControllerError::State(format!(
            "excluded route {} precommit lacks the exact protocol-{} ownership marker",
            snapshot.cidr, EXCLUDED_ROUTE_PROTOCOL_V1
        )));
    }
    if !route_destination_matches_cidr(current_route, &snapshot.cidr)
        || unique_route_field_value(current_route, "via")? != planned_via
        || unique_route_field_value(current_route, "dev")? != Some(planned_dev)
        || unique_route_field_value(current_route, "proto")? != Some(EXCLUDED_ROUTE_PROTOCOL_V1)
    {
        return Err(ControllerError::State(format!(
            "refusing to delete excluded route {} because the live route does not match its precommitted ownership tuple",
            snapshot.cidr
        )));
    }
    Ok(())
}
fn validate_installed_excluded_route(
    snapshot: &ExcludedRouteSnapshot,
    mutation: &[String],
    installed_route: &str,
) -> Result<(), ControllerError> {
    if !route_destination_matches_cidr(installed_route, &snapshot.cidr) {
        return Err(ControllerError::State(format!(
            "installed excluded-route readback does not identify exact prefix {}",
            snapshot.cidr
        )));
    }
    for field in ["via", "dev", "proto"] {
        let Some(index) = mutation.iter().position(|argument| argument == field) else {
            continue;
        };
        let value = mutation.get(index + 1).ok_or_else(|| {
            ControllerError::State(format!(
                "planned excluded-route mutation has a truncated {field} field"
            ))
        })?;
        if !route_has_exact_field(installed_route, field, value) {
            return Err(ControllerError::State(format!(
                "installed excluded-route readback for {} does not match planned {field} {value}",
                snapshot.cidr
            )));
        }
    }
    Ok(())
}
#[derive(Debug, PartialEq, Eq)]
enum ExcludedRouteRestoreAction {
    AlreadyAbsent,
    DeleteInstalled(String),
}
fn excluded_route_restore_action(
    snapshot: &ExcludedRouteSnapshot,
    current_route: Option<&str>,
) -> Result<ExcludedRouteRestoreAction, ControllerError> {
    let Some(current_route) = current_route else {
        return Ok(ExcludedRouteRestoreAction::AlreadyAbsent);
    };
    let Some(ownership) = snapshot.installed_route.as_deref() else {
        return Err(ControllerError::State(format!(
            "refusing to delete excluded route {} because it lacks a durable ownership proof",
            snapshot.cidr
        )));
    };
    if let Some(planned_route) = ownership.strip_prefix(PLANNED_EXCLUDED_ROUTE_PREFIX_V1) {
        validate_precommitted_excluded_route(snapshot, planned_route, current_route)?;
    } else if current_route != ownership {
        return Err(ControllerError::State(format!(
            "refusing to delete excluded route {} because live route state drifted from the exact helper-installed readback",
            snapshot.cidr
        )));
    }
    Ok(ExcludedRouteRestoreAction::DeleteInstalled(
        current_route.to_owned(),
    ))
}
fn restore_excluded_route(snapshot: &ExcludedRouteSnapshot) -> Result<(), ControllerError> {
    let current = capture_existing_route(snapshot.family, &snapshot.cidr)?;
    let action = excluded_route_restore_action(snapshot, current.as_deref())?;
    let installed_route = match action {
        ExcludedRouteRestoreAction::AlreadyAbsent => return Ok(()),
        ExcludedRouteRestoreAction::DeleteInstalled(installed_route) => installed_route,
    };
    // Delete the exact installed attributes rather than only the prefix. If another route
    // manager wins the race after the readback check, netlink rejects this deletion instead of
    // removing that manager's replacement.
    let mut delete_args = vec![
        snapshot.family.flag().to_owned(),
        "route".to_owned(),
        "del".to_owned(),
    ];
    delete_args.extend(
        installed_route
            .split_ascii_whitespace()
            .map(ToOwned::to_owned),
    );
    run_command(DEFAULT_ROUTE_CMD, delete_args)?;

    let restored = capture_existing_route(snapshot.family, &snapshot.cidr)?;
    if restored.is_some() {
        return Err(ControllerError::State(format!(
            "excluded route {} remained present after exact helper-route deletion",
            snapshot.cidr
        )));
    }
    Ok(())
}
fn plan_dns_backend(
    interface_name: &str,
    dns_servers: &[String],
) -> Result<Option<DnsBackendState>, ControllerError> {
    plan_dns_backend_for_availability(interface_name, dns_servers, command_exists("resolvectl"))
}
fn plan_dns_backend_for_availability(
    interface_name: &str,
    dns_servers: &[String],
    resolvectl_available: bool,
) -> Result<Option<DnsBackendState>, ControllerError> {
    if dns_servers.is_empty() {
        Ok(None)
    } else if resolvectl_available {
        Ok(Some(DnsBackendState::Resolved {
            interface_name: interface_name.to_owned(),
        }))
    } else {
        Err(ControllerError::State(
            "V1 DNS configuration requires a trusted resolvectl executable; direct /etc/resolv.conf mutation is unsupported"
                .to_owned(),
        ))
    }
}
fn apply_dns_plan_with<F>(
    interface_name: &str,
    dns_servers: &[String],
    plan: DnsBackendState,
    mut run: F,
) -> Result<DnsBackendState, ControllerError>
where
    F: FnMut(&str, Vec<String>) -> Result<String, ControllerError>,
{
    match plan {
        DnsBackendState::Resolved {
            interface_name: planned_interface,
        } => {
            if planned_interface != interface_name {
                return Err(ControllerError::State(
                    "resolved DNS journal is bound to a different interface".to_owned(),
                ));
            }
            let apply_result = (|| -> Result<(), ControllerError> {
                let mut dns_args = vec!["dns".to_owned(), interface_name.to_owned()];
                dns_args.extend(dns_servers.iter().map(|item| item.trim().to_owned()));
                run("resolvectl", dns_args)?;
                run(
                    "resolvectl",
                    vec![
                        "domain".to_owned(),
                        interface_name.to_owned(),
                        "~.".to_owned(),
                    ],
                )?;
                run(
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
                return match run(
                    "resolvectl",
                    vec!["revert".to_owned(), interface_name.to_owned()],
                ) {
                    Ok(_) => Err(error),
                    Err(rollback_error) => Err(ControllerError::State(format!(
                        "{error}; resolved DNS rollback also failed: {rollback_error}"
                    ))),
                };
            }
            Ok(DnsBackendState::Resolved {
                interface_name: interface_name.to_owned(),
            })
        }
        DnsBackendState::ResolvedReverted { .. } => Err(ControllerError::State(
            "refusing to apply a DNS journal that is already active or being cleaned".to_owned(),
        )),
    }
}
fn cleanup_resolved_dns(interface_name: &str) -> Result<(), ControllerError> {
    match run_command(
        "resolvectl",
        vec!["revert".to_owned(), interface_name.to_owned()],
    ) {
        Ok(_) => Ok(()),
        // Closing the last TUN descriptor deliberately precedes global cleanup, so systemd-
        // resolved may have already discarded the vanished link. Treat only its explicit
        // absent-link results as the idempotent success case; all other failures stay fatal.
        Err(ControllerError::State(message))
            if message.contains("No such device")
                || message.contains("No such link")
                || message.contains("Link not found")
                || message.contains("Failed to get link data")
                || message.contains("does not exist") =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
fn dns_backend_label(backend: &DnsBackendState) -> String {
    match backend {
        DnsBackendState::Resolved { .. } | DnsBackendState::ResolvedReverted { .. } => {
            "resolvectl".to_owned()
        }
    }
}
fn command_exists(program: &str) -> bool {
    resolve_trusted_command(program).is_some()
}
#[cfg(target_os = "linux")]
fn validate_system_command_cgroup_directory(
    path: &Path,
    metadata: &fs::Metadata,
    owner_private: bool,
) -> Result<(), ControllerError> {
    let permissions = metadata.mode() & 0o777;
    if !metadata.file_type().is_dir()
        || metadata.uid() != 0
        || metadata.gid() != 0
        || metadata.mode() & 0o022 != 0
        || (owner_private && permissions != 0o700)
    {
        return Err(ControllerError::CommandCustody(format!(
            "fixed command cgroup {} is not a root-custodied directory",
            path.display()
        )));
    }
    Ok(())
}
#[cfg(any(target_os = "linux", test))]
fn system_command_cgroup_control_has_custody(
    uid: u32,
    gid: u32,
    mode: u32,
    required_owner_bits: u32,
) -> bool {
    uid == 0 && gid == 0 && mode & 0o022 == 0 && mode & required_owner_bits == required_owner_bits
}
#[cfg(target_os = "linux")]
fn validate_system_command_cgroup_control(
    path: &Path,
    metadata: &fs::Metadata,
    required_owner_bits: u32,
) -> Result<(), ControllerError> {
    if !metadata.file_type().is_file()
        || !system_command_cgroup_control_has_custody(
            metadata.uid(),
            metadata.gid(),
            metadata.mode(),
            required_owner_bits,
        )
    {
        return Err(ControllerError::CommandCustody(format!(
            "fixed command cgroup control {} does not have root-owned, non-writable custody with required owner access",
            path.display()
        )));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn validate_cgroup2_mount(root: &Path, owner_private: bool) -> Result<fs::File, ControllerError> {
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW);
    let root_directory = options.open(root).map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to open Linux cgroup-v2 custody directory {} without following links: {error}",
            root.display()
        ))
    })?;
    let opened_metadata = root_directory.metadata().map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to inspect opened Linux cgroup-v2 custody directory {}: {error}",
            root.display()
        ))
    })?;
    validate_system_command_cgroup_directory(root, &opened_metadata, owner_private)?;
    let mut filesystem = std::mem::MaybeUninit::<nix::libc::statfs>::zeroed();
    // SAFETY: the descriptor pins the opened directory, and `filesystem` provides writable storage
    // for one statfs result. Checking the filesystem magic prevents a lookalike directory from
    // satisfying the root-custody checks in a hostile mount namespace.
    if unsafe { nix::libc::fstatfs(root_directory.as_raw_fd(), filesystem.as_mut_ptr()) } != 0 {
        return Err(ControllerError::CommandCustody(format!(
            "failed to identify Linux cgroup-v2 custody directory {}: {}",
            root.display(),
            io::Error::last_os_error()
        )));
    }
    // SAFETY: successful fstatfs initializes the result.
    let filesystem = unsafe { filesystem.assume_init() };
    if filesystem.f_type as u64 != nix::libc::CGROUP2_SUPER_MAGIC as u64 {
        return Err(ControllerError::CommandCustody(format!(
            "Linux privileged command custody directory {} is not on a genuine cgroup-v2 filesystem",
            root.display()
        )));
    }
    Ok(root_directory)
}
#[cfg(target_os = "linux")]
fn system_command_cgroup_path() -> PathBuf {
    PathBuf::from(SYSTEM_COMMAND_CGROUP_PATH)
}
#[cfg(target_os = "linux")]
fn ensure_system_command_cgroup_at(path: &Path) -> Result<PathBuf, ControllerError> {
    let root = Path::new("/sys/fs/cgroup");
    if path.parent() != Some(root) {
        return Err(ControllerError::CommandCustody(format!(
            "command cgroup {} is not a direct child of the cgroup-v2 root",
            path.display()
        )));
    }
    let controllers = root.join("cgroup.controllers");
    let root_metadata = fs::symlink_metadata(root).map_err(|error| {
        ControllerError::CommandCustody(format!("Linux cgroup-v2 root is unavailable: {error}"))
    })?;
    validate_system_command_cgroup_directory(root, &root_metadata, false)?;
    // Keep the verified mount root open across child-cgroup creation and validation. Cgroupfs
    // controls are live kernel state rather than persistent files, so fsync is neither supported
    // nor a durability boundary; the fixed path is rediscovered and proven empty on every cleanup.
    let root_directory = validate_cgroup2_mount(root, false)?;
    let controllers_metadata = fs::symlink_metadata(&controllers).map_err(|error| {
        ControllerError::CommandCustody(format!(
            "Linux cgroup-v2 controller marker is unavailable: {error}"
        ))
    })?;
    validate_system_command_cgroup_control(&controllers, &controllers_metadata, 0o400)?;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    let created = match builder.create(path) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => {
            return Err(ControllerError::CommandCustody(format!(
                "failed to create fixed command cgroup {}: {error}",
                path.display()
            )));
        }
    };
    if created && let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(0o700)) {
        // The just-created cgroup is necessarily empty. Remove it on a mode failure so an
        // attacker-controlled umask cannot leave a weaker fixed custody directory behind.
        let _ = fs::remove_dir(path);
        return Err(ControllerError::CommandCustody(format!(
            "failed to make fixed command cgroup {} owner-private: {error}",
            path.display()
        )));
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to inspect fixed command cgroup {}: {error}",
            path.display()
        ))
    })?;
    validate_system_command_cgroup_directory(path, &metadata, true)?;
    let command_directory = validate_cgroup2_mount(path, true)?;
    for (control, required_owner_bits) in [
        ("cgroup.events", 0o400),
        ("cgroup.kill", 0o200),
        ("cgroup.procs", 0o200),
    ] {
        let control_path = path.join(control);
        let metadata = fs::symlink_metadata(&control_path).map_err(|error| {
            ControllerError::CommandCustody(format!(
                "fixed command cgroup control {} is unavailable: {error}",
                control_path.display()
            ))
        })?;
        validate_system_command_cgroup_control(&control_path, &metadata, required_owner_bits)?;
    }
    drop(command_directory);
    drop(root_directory);
    Ok(path.to_path_buf())
}
#[cfg(target_os = "linux")]
fn ensure_system_command_cgroup() -> Result<PathBuf, ControllerError> {
    ensure_system_command_cgroup_at(&system_command_cgroup_path())
}
#[cfg(any(target_os = "linux", test))]
fn parse_system_command_cgroup_populated(events: &str) -> Result<bool, ControllerError> {
    let mut populated = None;
    for line in events.lines() {
        let mut fields = line.split_ascii_whitespace();
        let Some(key) = fields.next() else {
            continue;
        };
        let Some(value) = fields.next() else {
            return Err(ControllerError::CommandCustody(
                "fixed command cgroup events contain a truncated entry".to_owned(),
            ));
        };
        if fields.next().is_some() {
            return Err(ControllerError::CommandCustody(
                "fixed command cgroup events contain a malformed entry".to_owned(),
            ));
        }
        if key == "populated" {
            if populated.is_some() {
                return Err(ControllerError::CommandCustody(
                    "fixed command cgroup events duplicate populated state".to_owned(),
                ));
            }
            populated = Some(match value {
                "0" => false,
                "1" => true,
                _ => {
                    return Err(ControllerError::CommandCustody(
                        "fixed command cgroup populated state is not 0 or 1".to_owned(),
                    ));
                }
            });
        }
    }
    populated.ok_or_else(|| {
        ControllerError::CommandCustody(
            "fixed command cgroup events omit populated state".to_owned(),
        )
    })
}
#[cfg(target_os = "linux")]
fn system_command_cgroup_populated(path: &Path) -> Result<bool, ControllerError> {
    let events_path = path.join("cgroup.events");
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NOFOLLOW);
    let mut events = options.open(&events_path).map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to open fixed command cgroup events: {error}"
        ))
    })?;
    let metadata = events.metadata().map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to inspect opened fixed command cgroup events control: {error}"
        ))
    })?;
    validate_system_command_cgroup_control(&events_path, &metadata, 0o400)?;
    let bytes = read_bounded(&mut events, 4 * 1024, "fixed command cgroup events")
        .map_err(|error| ControllerError::CommandCustody(error.to_string()))?;
    let events = std::str::from_utf8(&bytes).map_err(|error| {
        ControllerError::CommandCustody(format!(
            "fixed command cgroup events are not UTF-8: {error}"
        ))
    })?;
    parse_system_command_cgroup_populated(events)
}
#[cfg(target_os = "linux")]
fn write_system_command_cgroup_control(
    path: &Path,
    control: &str,
    value: &[u8],
) -> Result<(), ControllerError> {
    if control != "cgroup.kill" {
        return Err(ControllerError::CommandCustody(format!(
            "unsupported fixed command cgroup write control {control}"
        )));
    }
    let control_path = path.join(control);
    let mut options = fs::OpenOptions::new();
    options
        .write(true)
        .custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NOFOLLOW);
    let mut file = options.open(&control_path).map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to open fixed command cgroup control {}: {error}",
            control_path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to inspect opened fixed command cgroup control {}: {error}",
            control_path.display()
        ))
    })?;
    validate_system_command_cgroup_control(&control_path, &metadata, 0o200)?;
    file.write_all(value).map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to update fixed command cgroup control {}: {error}",
            control_path.display()
        ))
    })
}
#[cfg(target_os = "linux")]
fn open_system_command_cgroup_procs(path: &Path) -> Result<fs::File, ControllerError> {
    let procs_path = path.join("cgroup.procs");
    let mut options = fs::OpenOptions::new();
    options
        .write(true)
        .custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NOFOLLOW);
    let file = options.open(&procs_path).map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to open fixed command cgroup membership control {}: {error}",
            procs_path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        ControllerError::CommandCustody(format!(
            "failed to inspect opened fixed command cgroup membership control {}: {error}",
            procs_path.display()
        ))
    })?;
    validate_system_command_cgroup_control(&procs_path, &metadata, 0o200)?;
    if file.as_raw_fd() > nix::libc::STDERR_FILENO {
        return Ok(file);
    }
    // A caller may invoke the set-user-ID helper with closed stdio. Pin the membership control
    // above descriptors 0..=2 before Command configures child stdio; otherwise dup2 could replace
    // this fd with /dev/null and make the pre-exec write falsely appear successful.
    // SAFETY: F_DUPFD_CLOEXEC duplicates one live descriptor at or above the supplied lower bound.
    let duplicated = unsafe {
        nix::libc::fcntl(
            file.as_raw_fd(),
            nix::libc::F_DUPFD_CLOEXEC,
            nix::libc::STDERR_FILENO + 1,
        )
    };
    if duplicated < 0 {
        return Err(ControllerError::CommandCustody(format!(
            "failed to pin fixed command cgroup membership control above stdio: {}",
            io::Error::last_os_error()
        )));
    }
    drop(file);
    // SAFETY: F_DUPFD_CLOEXEC returned a fresh descriptor now owned by this File.
    Ok(unsafe { fs::File::from_raw_fd(duplicated) })
}
#[cfg(target_os = "linux")]
fn quiesce_system_command_cgroup_at_until(
    path: &Path,
    deadline: Instant,
) -> Result<(), ControllerError> {
    if !system_command_cgroup_populated(path)? {
        return Ok(());
    }
    write_system_command_cgroup_control(path, "cgroup.kill", b"1\n")?;
    loop {
        if !system_command_cgroup_populated(path)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(ControllerError::CommandCustody(format!(
                "fixed command cgroup {} remained populated after cgroup.kill",
                path.display()
            )));
        }
        sleep_blocking(
            deadline
                .saturating_duration_since(Instant::now())
                .min(SYSTEM_COMMAND_POLL_INTERVAL),
        );
    }
}
#[cfg(target_os = "linux")]
fn quiesce_system_command_cgroup_until(deadline: Instant) -> Result<(), ControllerError> {
    let path = ensure_system_command_cgroup()?;
    quiesce_system_command_cgroup_at_until(&path, deadline)
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
#[cfg(target_os = "linux")]
fn run_command_until<I, S>(
    program: &str,
    args: I,
    preparation_deadline: Instant,
) -> Result<String, ControllerError>
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
    let (execution_deadline, custody_deadline) =
        privileged_command_deadlines(preparation_deadline)?;
    if Instant::now() >= execution_deadline {
        return Err(ControllerError::State(
            "privileged preparation has no remaining time for another system command".to_owned(),
        ));
    }
    let cgroup_path = ensure_system_command_cgroup()?;
    execute_system_command_in_cgroup_until(
        program,
        &program_path,
        &collected,
        execution_deadline,
        custody_deadline,
        &cgroup_path,
    )
}
#[cfg(target_os = "linux")]
fn spawn_system_command_pipe_reader<R>(
    reader: R,
    max_bytes: usize,
    name: &str,
) -> io::Result<std::sync::mpsc::Receiver<io::Result<BoundedPipeOutput>>>
where
    R: io::Read + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let _ = sender.send(drain_bounded_pipe(reader, max_bytes));
        })?;
    Ok(receiver)
}
#[cfg(target_os = "linux")]
fn receive_system_command_pipe_until(
    receiver: &std::sync::mpsc::Receiver<io::Result<BoundedPipeOutput>>,
    deadline: Instant,
    label: &str,
) -> Result<BoundedPipeOutput, ControllerError> {
    match receiver.try_recv() {
        Ok(result) => return result.map_err(Into::into),
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            return Err(ControllerError::State(format!(
                "{label} drain thread terminated without a result"
            )));
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
    }
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| {
            ControllerError::State(format!(
                "timed out draining {label} after the exact command unit exited"
            ))
        })?;
    match receiver.recv_timeout(remaining) {
        Ok(result) => result.map_err(Into::into),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(ControllerError::State(format!(
            "timed out draining {label} after the exact command unit exited"
        ))),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(ControllerError::State(
            format!("{label} drain thread terminated without a result"),
        )),
    }
}
#[cfg(target_os = "linux")]
fn system_command_leader_exited_unreaped(child_pid: u32) -> io::Result<bool> {
    let mut information = std::mem::MaybeUninit::<nix::libc::siginfo_t>::zeroed();
    // SAFETY: `information` points to writable storage for one siginfo_t. WNOWAIT observes only
    // this direct child and leaves it unreaped, pinning both its PID and process-group identifier.
    let result = unsafe {
        nix::libc::waitid(
            nix::libc::P_PID,
            child_pid,
            information.as_mut_ptr(),
            nix::libc::WEXITED | nix::libc::WNOHANG | nix::libc::WNOWAIT,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful waitid initializes siginfo_t; WNOHANG reports si_pid == 0 while live.
    let information = unsafe { information.assume_init() };
    // SAFETY: waitid(WEXITED) populates the SIGCHLD variant of siginfo_t.
    let observed_pid = unsafe { information.si_pid() };
    Ok(observed_pid > 0 && u32::try_from(observed_pid).ok() == Some(child_pid))
}
#[cfg(target_os = "linux")]
fn terminate_system_command_unit_until(
    child: &mut Child,
    cgroup_path: &Path,
    deadline: Instant,
) -> Result<ExitStatus, ControllerError> {
    let mut custody_failures = Vec::new();
    match system_command_cgroup_populated(cgroup_path) {
        Ok(true) => {
            if let Err(error) =
                write_system_command_cgroup_control(cgroup_path, "cgroup.kill", b"1\n")
            {
                custody_failures.push(error.to_string());
            }
        }
        Ok(false) => {}
        Err(error) => custody_failures.push(error.to_string()),
    }
    // The leader is deliberately still waitable here, so its PID/PGID cannot be reused before
    // this group signal. The fixed cgroup independently covers descendants that changed sessions.
    if let Err(error) = kill_command_process_group(child.id()) {
        custody_failures.push(format!(
            "failed to kill exact command process group: {error}"
        ));
        let _ = child.kill();
    }

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => sleep_blocking(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(SYSTEM_COMMAND_POLL_INTERVAL),
            ),
            Ok(None) => {
                custody_failures.push(format!(
                    "direct command child {} was not reaped before the absolute custody deadline",
                    child.id()
                ));
                break None;
            }
            Err(error) => {
                custody_failures.push(format!(
                    "failed to reap direct command child {}: {error}",
                    child.id()
                ));
                break None;
            }
        }
    };
    if let Err(error) = quiesce_system_command_cgroup_at_until(cgroup_path, deadline) {
        custody_failures.push(error.to_string());
    }
    if !custody_failures.is_empty() {
        return Err(ControllerError::CommandCustody(custody_failures.join("; ")));
    }
    status.ok_or_else(|| {
        ControllerError::CommandCustody(
            "direct command child status is unavailable after unit termination".to_owned(),
        )
    })
}
#[cfg(target_os = "linux")]
fn setup_system_command_failure(
    child: &mut Child,
    cgroup_path: &Path,
    error: impl std::fmt::Display,
    custody_deadline: Instant,
) -> ControllerError {
    let message = error.to_string();
    match terminate_system_command_unit_until(child, cgroup_path, custody_deadline) {
        Ok(_) => ControllerError::State(message),
        Err(custody_error) => ControllerError::CommandCustody(format!(
            "{message}; command launch cleanup failed: {custody_error}"
        )),
    }
}
#[cfg(target_os = "linux")]
fn execute_system_command(
    program: &str,
    program_path: &Path,
    collected: &[String],
    command_timeout: Duration,
) -> Result<String, ControllerError> {
    let cgroup_path = ensure_system_command_cgroup()?;
    execute_system_command_in_cgroup(
        program,
        program_path,
        collected,
        command_timeout,
        &cgroup_path,
    )
}
#[cfg(target_os = "linux")]
fn execute_system_command_in_cgroup(
    program: &str,
    program_path: &Path,
    collected: &[String],
    command_timeout: Duration,
    cgroup_path: &Path,
) -> Result<String, ControllerError> {
    // Derive both phase cutoffs once, before the first custody operation. No error, signal, reap,
    // cgroup, pipe, or setup path can extend the final deadline with a fresh relative timeout.
    let started = Instant::now();
    let execution_deadline = started + command_timeout;
    let custody_deadline = execution_deadline + PROCESS_KILL_REAP_TIMEOUT;
    execute_system_command_in_cgroup_until(
        program,
        program_path,
        collected,
        execution_deadline,
        custody_deadline,
        cgroup_path,
    )
}
#[cfg(target_os = "linux")]
fn execute_system_command_in_cgroup_until(
    program: &str,
    program_path: &Path,
    collected: &[String],
    execution_deadline: Instant,
    custody_deadline: Instant,
    cgroup_path: &Path,
) -> Result<String, ControllerError> {
    if execution_deadline > custody_deadline || Instant::now() >= execution_deadline {
        return Err(ControllerError::State(format!(
            "{program} command has no remaining absolute execution budget"
        )));
    }
    quiesce_system_command_cgroup_at_until(cgroup_path, custody_deadline)?;
    if Instant::now() >= execution_deadline {
        return Err(ControllerError::State(format!(
            "{program} command exhausted its absolute execution budget while quiescing prior custody"
        )));
    }
    let membership = open_system_command_cgroup_procs(cgroup_path)?;
    let membership_fd = membership.as_raw_fd();
    let supervisor_pid = i32::try_from(std::process::id()).map_err(|_| {
        ControllerError::CommandCustody(
            "VPN supervisor PID does not fit Linux pid_t for command custody".to_owned(),
        )
    })?;
    let mut command = ProcessCommand::new(program_path);
    command
        .env_clear()
        .env("PATH", "/usr/sbin:/sbin:/usr/bin:/bin")
        .args(collected)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.process_group(0);
    // SAFETY: the closure uses only async-signal-safe integer syscalls between fork and exec. The
    // membership descriptor remains owned by `membership` until `spawn` returns and is CLOEXEC.
    unsafe {
        command.pre_exec(move || {
            mark_unintended_child_fds_close_on_exec()?;
            if nix::libc::prctl(nix::libc::PR_SET_PDEATHSIG, nix::libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            if nix::libc::getppid() != supervisor_pid {
                return Err(io::Error::from_raw_os_error(nix::libc::ESRCH));
            }
            let membership_value = b"0\n";
            let written = nix::libc::write(
                membership_fd,
                membership_value.as_ptr().cast(),
                membership_value.len(),
            );
            if written != membership_value.len() as isize {
                return Err(if written < 0 {
                    io::Error::last_os_error()
                } else {
                    io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to enter the fixed command cgroup atomically",
                    )
                });
            }
            if nix::libc::getppid() != supervisor_pid {
                return Err(io::Error::from_raw_os_error(nix::libc::ESRCH));
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    drop(membership);
    let stdout = child.stdout.take().ok_or_else(|| {
        setup_system_command_failure(
            &mut child,
            cgroup_path,
            format!("failed to capture {program} standard output"),
            custody_deadline,
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        setup_system_command_failure(
            &mut child,
            cgroup_path,
            format!("failed to capture {program} standard error"),
            custody_deadline,
        )
    })?;
    let stdout_receiver = spawn_system_command_pipe_reader(
        stdout,
        MAX_SYSTEM_COMMAND_STDOUT_BYTES,
        "sora-vpn-command-stdout",
    )
    .map_err(|error| {
        setup_system_command_failure(&mut child, cgroup_path, error, custody_deadline)
    })?;
    let stderr_receiver = spawn_system_command_pipe_reader(
        stderr,
        MAX_SYSTEM_COMMAND_STDERR_BYTES,
        "sora-vpn-command-stderr",
    )
    .map_err(|error| {
        setup_system_command_failure(&mut child, cgroup_path, error, custody_deadline)
    })?;
    let (timed_out, observation_error) = loop {
        match system_command_leader_exited_unreaped(child.id()) {
            Ok(true) => break (false, None),
            Ok(false) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => break (false, Some(error)),
        }
        if Instant::now() >= execution_deadline {
            break (true, None);
        }
        sleep_blocking(
            execution_deadline
                .saturating_duration_since(Instant::now())
                .min(SYSTEM_COMMAND_POLL_INTERVAL),
        );
    };
    let status = terminate_system_command_unit_until(&mut child, cgroup_path, custody_deadline)?;
    let stdout = receive_system_command_pipe_until(
        &stdout_receiver,
        custody_deadline,
        "system command standard output",
    )?;
    let stderr = receive_system_command_pipe_until(
        &stderr_receiver,
        custody_deadline,
        "system command standard error",
    )?;
    if let Some(error) = observation_error {
        return Err(error.into());
    }
    if timed_out {
        return Err(ControllerError::State(format!(
            "{program} {} exceeded its absolute command execution deadline",
            collected.join(" ")
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
#[cfg(not(target_os = "linux"))]
fn execute_system_command(
    _program: &str,
    _program_path: &Path,
    _collected: &[String],
    _command_timeout: Duration,
) -> Result<String, ControllerError> {
    Err(ControllerError::State(
        "privileged system commands are supported only on Linux".to_owned(),
    ))
}
#[cfg(target_os = "linux")]
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
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, PartialEq, Eq)]
struct BoundedPipeOutput {
    bytes: Vec<u8>,
    overflow: bool,
}
#[cfg(any(target_os = "linux", test))]
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
#[cfg(target_os = "linux")]
fn system_executable_has_file_capabilities(path: &Path) -> Result<bool, ControllerError> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        ControllerError::State(format!(
            "system executable {} contains an embedded NUL",
            path.display()
        ))
    })?;
    let attribute = b"security.capability\0";
    // SAFETY: both C strings are NUL-terminated and live for the call. A null value with size zero
    // asks only whether the capability xattr exists, so no output storage is required.
    let size = unsafe {
        nix::libc::getxattr(
            path.as_ptr(),
            attribute.as_ptr().cast(),
            std::ptr::null_mut(),
            0,
        )
    };
    if size >= 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error
        .raw_os_error()
        .is_some_and(|code| code == nix::libc::ENODATA || code == nix::libc::ENOTSUP)
    {
        return Ok(false);
    }
    Err(error.into())
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
        // Linux clears PDEATHSIG when exec gains privilege through set-ID mode bits or file
        // capabilities. These commands already run as root, so privileged executable metadata is
        // unnecessary and would weaken kill-on-supervisor-death custody.
        if metadata.mode() & 0o6000 != 0 {
            return Err(ControllerError::State(format!(
                "system executable {} has set-user-ID or set-group-ID mode",
                path.display()
            )));
        }
        #[cfg(target_os = "linux")]
        if system_executable_has_file_capabilities(path)? {
            return Err(ControllerError::State(format!(
                "system executable {} has file capabilities",
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
fn parse_canonical_session_id(session_id: &str) -> Result<[u8; 16], ControllerError> {
    if session_id.len() != 32
        || !session_id
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ControllerError::InvalidPayload(
            "sessionId must contain exactly 32 lowercase hexadecimal characters".to_owned(),
        ));
    }
    let mut decoded = [0_u8; 16];
    hex::decode_to_slice(session_id, &mut decoded)
        .expect("canonical session id validation makes decoding infallible");
    Ok(decoded)
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
fn apply_terminal_network_lifecycle(state: &mut State, active: bool, repair_required: bool) {
    if !active && !repair_required {
        state.applied_network = None;
        state.network_service = None;
        if state.worker_identity.is_none() && state.network_worker_identity.is_none() {
            clear_session_binding(state);
        }
    }
}
fn current_state() -> Result<State, ControllerError> {
    let mut state = load_state()?;
    hydrate_runtime_fields(&mut state);
    scrub_stale_process(&mut state)?;
    validate_state_for_persistence(&state)?;
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
    validate_state_for_persistence(state)?;
    let parent = path.parent().ok_or_else(|| {
        ControllerError::State(format!("state path {} has no parent", path.display()))
    })?;
    prepare_private_state_root(parent)?;
    let bytes = encode_state_frame(state)?;
    write_file_atomic(path, &bytes, 0o600, true, "state file")
}
fn validate_state_invariants(state: &State) -> Result<(), ControllerError> {
    match &state.worker_identity {
        None if !state.active => {}
        Some(identity) if identity.pid > 1 && identity.role == WorkerRole::Tunnel => {}
        _ => {
            return Err(ControllerError::State(
                "active state must have a valid worker process identity".to_owned(),
            ));
        }
    }
    if state
        .network_worker_identity
        .as_ref()
        .is_some_and(|identity| identity.pid <= 1 || identity.role != WorkerRole::Network)
    {
        return Err(ControllerError::State(
            "network-worker state must carry a valid network-child identity".to_owned(),
        ));
    }
    let binding_is_complete = state.owner_uid.is_some_and(|uid| uid != 0)
        && state
            .session_id
            .as_deref()
            .is_some_and(|session_id| !session_id.is_empty())
        && state.relay_id.is_some()
        && state.network_policy_hash.is_some();
    let binding_is_pending_authentication = state.owner_uid.is_some_and(|uid| uid != 0)
        && state.worker_identity.is_some()
        && !state.active
        && !state.repair_required
        && state.applied_network.is_none()
        && state.session_id.is_none()
        && state.relay_endpoint.is_none()
        && state.relay_id.is_none()
        && state.network_policy_hash.is_none()
        && state.ticket_expires_at_ms.is_none();
    if state_has_session_binding(state)
        && !binding_is_complete
        && !binding_is_pending_authentication
    {
        return Err(ControllerError::State(
            "persisted VPN session ownership binding is incomplete".to_owned(),
        ));
    }
    if (state.active
        || state.repair_required
        || state.worker_identity.is_some()
        || state.network_worker_identity.is_some()
        || state.applied_network.is_some())
        && !binding_is_complete
        && !binding_is_pending_authentication
    {
        return Err(ControllerError::State(
            "privileged VPN state is missing its caller/session ownership binding".to_owned(),
        ));
    }
    Ok(())
}

fn active_runtime_state_complete_at(state: &State, now_ms: u64) -> bool {
    !state.repair_required
        && state
            .ticket_expires_at_ms
            .is_some_and(|expires_at_ms| expires_at_ms > now_ms)
        && state.network_worker_identity.is_some()
        && state
            .applied_network
            .as_ref()
            .is_some_and(|applied| applied.journal_phase == NetworkJournalPhase::Prepared)
}

fn demote_expired_active_state_at(state: &mut State, now_ms: u64) {
    if state.active
        && state
            .ticket_expires_at_ms
            .is_some_and(|expires_at_ms| expires_at_ms <= now_ms)
    {
        state.active = false;
        state.repair_required = true;
        state.message =
            "authenticated helper ticket expired; awaiting exact network-worker shutdown"
                .to_owned();
    }
}

fn validate_state_for_persistence(state: &State) -> Result<(), ControllerError> {
    validate_state_for_persistence_at(state, unix_now_ms()?)
}

fn validate_state_for_persistence_at(state: &State, now_ms: u64) -> Result<(), ControllerError> {
    validate_state_invariants(state)?;
    if state.active
        && !state
            .ticket_expires_at_ms
            .is_some_and(|expires_at_ms| expires_at_ms > now_ms)
    {
        return Err(ControllerError::State(
            "active state must retain an unexpired authenticated ticket deadline".to_owned(),
        ));
    }
    if state.active && state.network_worker_identity.is_none() {
        return Err(ControllerError::State(
            "active state must have a valid network-child identity".to_owned(),
        ));
    }
    if state.active && state.applied_network.is_none() {
        return Err(ControllerError::State(
            "active state must retain its privileged network repair journal".to_owned(),
        ));
    }
    if state.active
        && state
            .applied_network
            .as_ref()
            .is_some_and(|applied| applied.journal_phase != NetworkJournalPhase::Prepared)
    {
        return Err(ControllerError::State(
            "active state must retain a completely prepared network journal".to_owned(),
        ));
    }
    if state.active && state.repair_required {
        return Err(ControllerError::State(
            "active state cannot simultaneously require repair".to_owned(),
        ));
    }
    Ok(())
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
    let decode_limits = || {
        norito::DecodeLimits::new(
            MAX_STATE_SEQUENCE_ELEMENTS_V1,
            MAX_STATE_FIELD_BYTES_V1,
            MAX_STATE_TOTAL_ELEMENTS_V1,
            MAX_STATE_DECODE_ALLOCATION_BYTES_V1,
            MAX_STATE_DECODE_DEPTH_V1,
        )
    };
    if bytes.starts_with(STATE_FILE_FRAME_MAGIC) {
        return norito::codec::decode_exact_from_slice_with_limits::<State>(
            &bytes[STATE_FILE_FRAME_MAGIC.len()..],
            decode_limits(),
        )
        .map_err(|error| ControllerError::State(format!("failed to decode state: {error}")));
    }
    if bytes.starts_with(STATE_FILE_FRAME_MAGIC_V1) {
        return norito::codec::decode_exact_from_slice_with_limits::<StateV1>(
            &bytes[STATE_FILE_FRAME_MAGIC_V1.len()..],
            decode_limits(),
        )
        .map(State::from)
        .map_err(|error| {
            ControllerError::State(format!("failed to decode legacy state: {error}"))
        });
    }
    Err(ControllerError::State(
        "state file is not a supported Norito state frame".to_owned(),
    ))
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
    insert_u64_option(&mut map, "ticket_expires_at_ms", state.ticket_expires_at_ms);
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
    insert_string(
        &mut map,
        "journal_phase",
        match state.journal_phase {
            NetworkJournalPhase::Planned => "planned",
            NetworkJournalPhase::TunCreated => "tun-created",
            NetworkJournalPhase::LinkConfigured => "link-configured",
            NetworkJournalPhase::RoutesConfigured => "routes-configured",
            NetworkJournalPhase::ConfiguringExcludedRoutes => "configuring-excluded-routes",
            NetworkJournalPhase::ExcludedRoutesConfigured => "excluded-routes-configured",
            NetworkJournalPhase::DnsPlanned => "dns-planned",
            NetworkJournalPhase::Prepared => "prepared",
            NetworkJournalPhase::CleaningDns => "cleaning-dns",
            NetworkJournalPhase::CleaningRoutes => "cleaning-routes",
        },
    );
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
        DnsBackendState::ResolvedReverted { interface_name } => {
            insert_string(&mut map, "kind", "resolved-reverted");
            insert_string(&mut map, "interface_name", interface_name);
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
        "installed_route",
        snapshot.installed_route.as_deref(),
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
fn insert_u64_option(map: &mut JsonMap, key: &str, value: Option<u64>) {
    map.insert(
        key.to_owned(),
        value
            .map(|value| JsonValue::Number(JsonNumber::from(value)))
            .unwrap_or(JsonValue::Null),
    );
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
#[cfg(any(target_os = "linux", test))]
fn load_helper_ticket_issuer_public_key() -> Result<PublicKey, ControllerError> {
    if effective_uid() != 0 {
        return Err(ControllerError::State(
            "helper-ticket issuer trust anchor may only be loaded by the root-effective privileged controller"
                .to_owned(),
        ));
    }
    load_helper_ticket_issuer_public_key_at(Path::new(HELPER_TICKET_ISSUER_PUBLIC_KEY_PATH), 0)
}
#[cfg(any(target_os = "linux", test))]
fn load_helper_ticket_issuer_public_key_at(
    path: &Path,
    required_owner_uid: u32,
) -> Result<PublicKey, ControllerError> {
    if !path.is_absolute() {
        return Err(ControllerError::State(format!(
            "helper-ticket issuer public-key path {} must be absolute",
            path.display()
        )));
    }
    let parent = path.parent().ok_or_else(|| {
        ControllerError::State(format!(
            "helper-ticket issuer public-key path {} has no parent",
            path.display()
        ))
    })?;
    validate_directory_custody(parent)?;
    let metadata = fs::symlink_metadata(path)?;
    validate_regular_file_metadata(path, &metadata, "helper-ticket issuer public key", true)?;
    #[cfg(unix)]
    if metadata.uid() != required_owner_uid {
        return Err(ControllerError::State(format!(
            "helper-ticket issuer public key {} is not owned by uid {required_owner_uid}",
            path.display()
        )));
    }
    let encoded = read_private_stable_regular_file_bounded(
        path,
        HELPER_TICKET_ISSUER_PUBLIC_KEY_HEX_BYTES + 1,
        "helper-ticket issuer public key",
    )?;
    if encoded.len() != HELPER_TICKET_ISSUER_PUBLIC_KEY_HEX_BYTES
        || !encoded
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ControllerError::State(format!(
            "helper-ticket issuer public key {} must contain exactly {} lowercase hexadecimal bytes with no newline",
            path.display(),
            HELPER_TICKET_ISSUER_PUBLIC_KEY_HEX_BYTES
        )));
    }
    let mut key_bytes = [0_u8; 32];
    hex::decode_to_slice(&encoded, &mut key_bytes)
        .expect("canonical issuer public-key hexadecimal validation makes decoding infallible");
    PublicKey::from_bytes(Algorithm::Ed25519, &key_bytes).map_err(|error| {
        ControllerError::State(format!(
            "helper-ticket issuer public key {} is not a canonical Ed25519 public key: {error}",
            path.display()
        ))
    })
}
#[cfg(any(target_os = "linux", test))]
fn validate_privileged_caller_identity(
    real_uid: u32,
    effective_uid: u32,
    saved_uid: u32,
    real_gid: u32,
    effective_gid: u32,
    saved_gid: u32,
) -> Result<PrivilegedCaller, ControllerError> {
    if real_uid == 0 {
        return Err(ControllerError::State(
            "unsafe privileged invocation refused: connect, disconnect, repair, and run-tunnel require an authenticated non-root real UID; direct root and sudo invocation are unsupported"
                .to_owned(),
        ));
    }
    if effective_uid != 0 || saved_uid != 0 {
        return Err(ControllerError::State(
            "unsafe privileged invocation refused: install the root-owned controller with its set-user-ID bit, or use a future root daemon that authenticates SO_PEERCRED"
                .to_owned(),
        ));
    }
    if real_gid == 0 || effective_gid != real_gid || saved_gid != real_gid {
        return Err(ControllerError::State(
            "unsafe privileged invocation refused: the authenticated caller must have one non-root real/effective/saved GID"
                .to_owned(),
        ));
    }
    Ok(PrivilegedCaller {
        uid: real_uid,
        gid: real_gid,
    })
}
#[cfg(any(target_os = "linux", test))]
fn validate_privileged_executable_custody(
    owner_uid: u32,
    mode: u32,
) -> Result<(), ControllerError> {
    if owner_uid != 0 || mode & 0o4_000 == 0 || mode & 0o022 != 0 {
        return Err(ControllerError::State(
            "unsafe privileged invocation refused: the running controller inode must be root-owned, set-user-ID, and not group/other-writable; capability-only mode is unsupported"
                .to_owned(),
        ));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn current_privileged_caller() -> Result<PrivilegedCaller, ControllerError> {
    let mut real_uid = 0;
    let mut effective_uid = 0;
    let mut saved_uid = 0;
    let mut real_gid = 0;
    let mut effective_gid = 0;
    let mut saved_gid = 0;
    // SAFETY: `getresuid` receives three valid output pointers and retains none of them.
    let result = unsafe { nix::libc::getresuid(&mut real_uid, &mut effective_uid, &mut saved_uid) };
    if result != 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: `getresgid` receives three valid output pointers and retains none of them.
    let result = unsafe { nix::libc::getresgid(&mut real_gid, &mut effective_gid, &mut saved_gid) };
    if result != 0 {
        return Err(io::Error::last_os_error().into());
    }
    let caller = validate_privileged_caller_identity(
        real_uid,
        effective_uid,
        saved_uid,
        real_gid,
        effective_gid,
        saved_gid,
    )?;
    let executable = fs::metadata("/proc/self/exe").map_err(|error| {
        ControllerError::State(format!(
            "failed to inspect the running privileged controller inode: {error}"
        ))
    })?;
    if !executable.file_type().is_file() {
        return Err(ControllerError::State(
            "unsafe privileged invocation refused: /proc/self/exe is not a regular file".to_owned(),
        ));
    }
    validate_privileged_executable_custody(executable.uid(), executable.mode())?;
    Ok(caller)
}
#[cfg(not(target_os = "linux"))]
fn current_privileged_caller() -> Result<PrivilegedCaller, ControllerError> {
    Err(ControllerError::State(
        "privileged VPN controller caller authentication is only available on Linux".to_owned(),
    ))
}
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct DroppedPrivilegeSnapshot {
    real_uid: u32,
    effective_uid: u32,
    saved_uid: u32,
    real_gid: u32,
    effective_gid: u32,
    saved_gid: u32,
    supplementary_groups: Vec<u32>,
    no_new_privs: bool,
    capabilities_clear: bool,
}
#[cfg(target_os = "linux")]
trait PrivilegeDropOps {
    fn set_no_new_privs(&mut self) -> io::Result<()>;
    fn disable_keep_capabilities(&mut self) -> io::Result<()>;
    fn clear_ambient_capabilities(&mut self) -> io::Result<()>;
    fn clear_bounding_capabilities(&mut self) -> io::Result<()>;
    fn clear_supplementary_groups(&mut self) -> io::Result<()>;
    fn set_res_gid(&mut self, gid: u32) -> io::Result<()>;
    fn set_res_uid(&mut self, uid: u32) -> io::Result<()>;
    fn snapshot(&mut self) -> io::Result<DroppedPrivilegeSnapshot>;
}
#[cfg(target_os = "linux")]
fn permanent_privilege_drop_with<O: PrivilegeDropOps>(
    caller: PrivilegedCaller,
    operations: &mut O,
) -> Result<(), ControllerError> {
    operations.set_no_new_privs()?;
    operations.disable_keep_capabilities()?;
    operations.clear_ambient_capabilities()?;
    operations.clear_bounding_capabilities()?;
    operations.clear_supplementary_groups()?;
    operations.set_res_gid(caller.gid)?;
    operations.set_res_uid(caller.uid)?;
    let snapshot = operations.snapshot()?;
    if snapshot.real_uid != caller.uid
        || snapshot.effective_uid != caller.uid
        || snapshot.saved_uid != caller.uid
        || snapshot.real_gid != caller.gid
        || snapshot.effective_gid != caller.gid
        || snapshot.saved_gid != caller.gid
        || !snapshot.supplementary_groups.is_empty()
        || !snapshot.no_new_privs
        || !snapshot.capabilities_clear
    {
        return Err(ControllerError::State(format!(
            "network worker failed permanent privilege-drop verification: {snapshot:?}"
        )));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn permanent_privilege_drop(caller: PrivilegedCaller) -> Result<(), ControllerError> {
    permanent_privilege_drop_with(caller, &mut LinuxPrivilegeDropOps)
}
#[cfg(target_os = "linux")]
fn seccomp_statement(code: u16, value: u32) -> nix::libc::sock_filter {
    nix::libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k: value,
    }
}
#[cfg(target_os = "linux")]
fn seccomp_jump(syscall: nix::libc::c_long, skip_if_not_equal: u8) -> nix::libc::sock_filter {
    nix::libc::sock_filter {
        code: 0x15, // BPF_JMP | BPF_JEQ | BPF_K
        jt: 0,
        jf: skip_if_not_equal,
        k: u32::try_from(syscall).expect("Linux syscall number fits the seccomp instruction"),
    }
}
#[cfg(target_os = "linux")]
fn install_network_worker_parser_containment() -> Result<(), ControllerError> {
    linux_prctl(nix::libc::PR_SET_DUMPABLE, 0, 0)?;

    let deny = 0x0005_0000 | u32::try_from(nix::libc::EPERM).expect("errno is positive");
    let unavailable = 0x0005_0000 | u32::try_from(nix::libc::ENOSYS).expect("errno is positive");
    let allow = 0x7fff_0000;
    let kill_process = 0x8000_0000;
    let audit_arch = if cfg!(target_arch = "x86_64") {
        0xc000_003e
    } else if cfg!(target_arch = "aarch64") {
        0xc000_00b7
    } else {
        return Err(ControllerError::State(
            "network-worker seccomp policy does not support this Linux architecture".to_owned(),
        ));
    };
    let mut filter = Vec::<nix::libc::sock_filter>::with_capacity(48);
    filter.push(seccomp_statement(0x20, 4)); // seccomp_data.arch
    filter.push(nix::libc::sock_filter {
        code: 0x15,
        jt: 1,
        jf: 0,
        k: audit_arch,
    });
    filter.push(seccomp_statement(0x06, kill_process));
    filter.push(seccomp_statement(0x20, 0)); // BPF_LD | BPF_W | BPF_ABS: seccomp_data.nr
    #[cfg(target_arch = "x86_64")]
    {
        filter.push(seccomp_statement(0x54, 0x4000_0000)); // reject x32 syscall ABI
        filter.push(nix::libc::sock_filter {
            code: 0x15,
            jt: 1,
            jf: 0,
            k: 0,
        });
        filter.push(seccomp_statement(0x06, kill_process));
        filter.push(seccomp_statement(0x20, 0));
    }
    for syscall in [nix::libc::SYS_socket, nix::libc::SYS_socketpair] {
        filter.push(seccomp_jump(syscall, 3));
        filter.push(seccomp_statement(0x20, 16)); // seccomp_data.args[0], address family
        filter.push(nix::libc::sock_filter {
            code: 0x15,
            jt: 0,
            jf: 1,
            k: u32::try_from(nix::libc::AF_UNIX).expect("AF_UNIX fits u32"),
        });
        filter.push(seccomp_statement(0x06, deny));
        filter.push(seccomp_statement(0x20, 0)); // reload syscall number
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    for syscall in [nix::libc::SYS_fork, nix::libc::SYS_vfork] {
        filter.push(seccomp_jump(syscall, 1));
        filter.push(seccomp_statement(0x06, deny)); // BPF_RET | BPF_K
    }
    for syscall in [
        nix::libc::SYS_execve,
        nix::libc::SYS_execveat,
        nix::libc::SYS_unshare,
        nix::libc::SYS_setns,
        nix::libc::SYS_ptrace,
        nix::libc::SYS_process_vm_readv,
        nix::libc::SYS_process_vm_writev,
        nix::libc::SYS_pidfd_getfd,
        nix::libc::SYS_userfaultfd,
        nix::libc::SYS_io_uring_setup,
        nix::libc::SYS_bpf,
        nix::libc::SYS_perf_event_open,
        nix::libc::SYS_mount,
        nix::libc::SYS_open_by_handle_at,
        nix::libc::SYS_prctl,
        nix::libc::SYS_seccomp,
        nix::libc::SYS_personality,
    ] {
        filter.push(seccomp_jump(syscall, 1));
        filter.push(seccomp_statement(0x06, deny));
    }
    filter.push(seccomp_jump(nix::libc::SYS_clone3, 1));
    filter.push(seccomp_statement(0x06, unavailable));
    // A thread may be created only when CLONE_THREAD is present. Processes, which could retain
    // the TUN fd outside the exact pidfd-bound child lifetime, are denied.
    filter.push(seccomp_jump(nix::libc::SYS_clone, 4));
    filter.push(seccomp_statement(0x20, 16)); // seccomp_data.args[0], low 32 bits
    filter.push(seccomp_statement(
        0x54, // BPF_ALU | BPF_AND | BPF_K
        u32::try_from(nix::libc::CLONE_THREAD).expect("CLONE_THREAD fits u32"),
    ));
    filter.push(nix::libc::sock_filter {
        code: 0x15,
        jt: 0,
        jf: 1,
        k: 0,
    });
    filter.push(seccomp_statement(0x06, deny));
    filter.push(seccomp_statement(0x06, allow));
    let mut program = nix::libc::sock_fprog {
        len: u16::try_from(filter.len()).expect("small fixed seccomp program"),
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: `program` and its instruction vector remain live for the syscall, and
    // no-new-privileges was verified before installing the filter.
    let result = unsafe {
        nix::libc::prctl(
            nix::libc::PR_SET_SECCOMP,
            nix::libc::SECCOMP_MODE_FILTER,
            (&raw mut program) as nix::libc::c_ulong,
            0,
            0,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}
#[cfg(target_os = "linux")]
struct LinuxPrivilegeDropOps;
#[cfg(target_os = "linux")]
impl PrivilegeDropOps for LinuxPrivilegeDropOps {
    fn set_no_new_privs(&mut self) -> io::Result<()> {
        linux_prctl(nix::libc::PR_SET_NO_NEW_PRIVS, 1, 0)
    }

    fn disable_keep_capabilities(&mut self) -> io::Result<()> {
        linux_prctl(nix::libc::PR_SET_KEEPCAPS, 0, 0)
    }

    fn clear_ambient_capabilities(&mut self) -> io::Result<()> {
        linux_prctl(
            nix::libc::PR_CAP_AMBIENT,
            nix::libc::PR_CAP_AMBIENT_CLEAR_ALL as nix::libc::c_ulong,
            0,
        )
    }

    fn clear_bounding_capabilities(&mut self) -> io::Result<()> {
        for capability in 0_u32..=255 {
            // SAFETY: PR_CAPBSET_READ/DROP accept one numeric capability and retain no pointers.
            let present =
                unsafe { nix::libc::prctl(nix::libc::PR_CAPBSET_READ, capability, 0, 0, 0) };
            if present < 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(nix::libc::EINVAL) {
                    break;
                }
                return Err(error);
            }
            if present == 1
                && unsafe { nix::libc::prctl(nix::libc::PR_CAPBSET_DROP, capability, 0, 0, 0) } != 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn clear_supplementary_groups(&mut self) -> io::Result<()> {
        // SAFETY: a zero group count permits a null list and clears every supplementary group.
        if unsafe { nix::libc::setgroups(0, std::ptr::null()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn set_res_gid(&mut self, gid: u32) -> io::Result<()> {
        // SAFETY: all three IDs are concrete values authenticated from the setuid invocation.
        if unsafe { nix::libc::setresgid(gid, gid, gid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn set_res_uid(&mut self, uid: u32) -> io::Result<()> {
        // SAFETY: all three IDs are concrete values authenticated from the setuid invocation.
        if unsafe { nix::libc::setresuid(uid, uid, uid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn snapshot(&mut self) -> io::Result<DroppedPrivilegeSnapshot> {
        dropped_privilege_snapshot()
    }
}
#[cfg(target_os = "linux")]
fn linux_prctl(
    option: nix::libc::c_int,
    argument_2: nix::libc::c_ulong,
    argument_3: nix::libc::c_ulong,
) -> io::Result<()> {
    // SAFETY: these `prctl` operations use integer arguments only and retain no pointers.
    let result = unsafe { nix::libc::prctl(option, argument_2, argument_3, 0, 0) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(target_os = "linux")]
#[repr(C)]
struct LinuxCapabilityHeader {
    version: u32,
    pid: i32,
}
#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct LinuxCapabilityData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}
#[cfg(target_os = "linux")]
fn dropped_privilege_snapshot() -> io::Result<DroppedPrivilegeSnapshot> {
    let mut real_uid = 0;
    let mut effective_uid = 0;
    let mut saved_uid = 0;
    let mut real_gid = 0;
    let mut effective_gid = 0;
    let mut saved_gid = 0;
    // SAFETY: each call receives three valid output pointers and retains none of them.
    if unsafe { nix::libc::getresuid(&mut real_uid, &mut effective_uid, &mut saved_uid) } != 0
        || unsafe { nix::libc::getresgid(&mut real_gid, &mut effective_gid, &mut saved_gid) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: querying with a zero length and null pointer returns the required group count.
    let group_count = unsafe { nix::libc::getgroups(0, std::ptr::null_mut()) };
    if group_count < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut supplementary_groups = vec![0_u32; group_count as usize];
    if group_count > 0 {
        // SAFETY: the vector holds exactly `group_count` writable gid slots.
        let read =
            unsafe { nix::libc::getgroups(group_count, supplementary_groups.as_mut_ptr().cast()) };
        if read != group_count {
            return Err(io::Error::last_os_error());
        }
    }
    // Linux capability ABI v3 has two 32-bit data words and covers all capabilities through 63.
    let mut header = LinuxCapabilityHeader {
        version: 0x2008_0522,
        pid: 0,
    };
    let mut capability_data = [LinuxCapabilityData::default(); 2];
    // SAFETY: `capget` receives the documented v3 header and a two-element writable data array.
    let capget_result = unsafe {
        nix::libc::syscall(
            nix::libc::SYS_capget,
            &mut header as *mut LinuxCapabilityHeader,
            capability_data.as_mut_ptr(),
        )
    };
    if capget_result != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `PR_GET_NO_NEW_PRIVS` ignores the remaining integer arguments.
    let no_new_privs = unsafe { nix::libc::prctl(nix::libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) };
    if no_new_privs < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut bounding_clear = true;
    for capability in 0_u32..=255 {
        // SAFETY: PR_CAPBSET_READ reads one numeric capability from the calling process.
        let present = unsafe { nix::libc::prctl(nix::libc::PR_CAPBSET_READ, capability, 0, 0, 0) };
        if present < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(nix::libc::EINVAL) {
                break;
            }
            return Err(error);
        }
        bounding_clear &= present == 0;
    }
    Ok(DroppedPrivilegeSnapshot {
        real_uid,
        effective_uid,
        saved_uid,
        real_gid,
        effective_gid,
        saved_gid,
        supplementary_groups,
        no_new_privs: no_new_privs == 1,
        capabilities_clear: bounding_clear
            && capability_data
                .iter()
                .all(|data| data.effective == 0 && data.permitted == 0 && data.inheritable == 0),
    })
}
#[cfg(target_os = "linux")]
fn acquire_network_worker_ipc_fd() -> Result<OwnedFd, ControllerError> {
    // The fixed descriptor is the only non-stdio descriptor intentionally inherited across the
    // second exec. Validate it before assuming ownership, then restore close-on-exec for the rest
    // of the unprivileged worker lifetime.
    // SAFETY: `fcntl` only inspects the numeric descriptor.
    let descriptor_flags = unsafe { nix::libc::fcntl(NETWORK_WORKER_IPC_FD, nix::libc::F_GETFD) };
    if descriptor_flags < 0 {
        return Err(ControllerError::State(
            "network-worker inherited IPC descriptor is missing".to_owned(),
        ));
    }
    // SAFETY: the descriptor is live and `F_SETFD` changes only its descriptor flags.
    if unsafe {
        nix::libc::fcntl(
            NETWORK_WORKER_IPC_FD,
            nix::libc::F_SETFD,
            descriptor_flags | nix::libc::FD_CLOEXEC,
        )
    } < 0
    {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: `F_GETFL` only inspects the live descriptor's status flags.
    let status_flags = unsafe { nix::libc::fcntl(NETWORK_WORKER_IPC_FD, nix::libc::F_GETFL) };
    if status_flags < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if status_flags & nix::libc::O_NONBLOCK == 0 {
        return Err(ControllerError::State(
            "network-worker inherited IPC descriptor is not nonblocking".to_owned(),
        ));
    }
    // SAFETY: descriptor 3 has not yet been adopted and this function takes sole ownership.
    let fd = unsafe { OwnedFd::from_raw_fd(NETWORK_WORKER_IPC_FD) };
    let mut socket_type: nix::libc::c_int = 0;
    let mut socket_type_len = nix::libc::socklen_t::try_from(core::mem::size_of_val(&socket_type))
        .expect("socket type width fits socklen_t");
    // SAFETY: both output pointers name live storage of the exact declared widths.
    if unsafe {
        nix::libc::getsockopt(
            fd.as_raw_fd(),
            nix::libc::SOL_SOCKET,
            nix::libc::SO_TYPE,
            (&raw mut socket_type).cast(),
            &raw mut socket_type_len,
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    if socket_type != nix::libc::SOCK_SEQPACKET {
        return Err(ControllerError::State(
            "network-worker inherited IPC descriptor is not a Unix sequenced-packet socket"
                .to_owned(),
        ));
    }
    enable_network_ipc_credentials(fd.as_raw_fd())?;
    // Eliminate every unintended inherited descriptor before hostile payload parsing. Descriptor
    // 3 is the authenticated IPC channel and stdio is required for the withheld payload and
    // deterministic diagnostics; no higher descriptor may cross this boundary.
    const CLOSE_RANGE_UNSHARE: u32 = 1 << 1;
    // SAFETY: `close_range` receives only integer bounds and does not retain pointers.
    if unsafe {
        nix::libc::syscall(
            nix::libc::SYS_close_range,
            4_u32,
            u32::MAX,
            CLOSE_RANGE_UNSHARE,
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    Ok(fd)
}
#[cfg(target_os = "linux")]
fn authenticate_network_worker_supervisor(
    fd: &OwnedFd,
    caller: PrivilegedCaller,
) -> Result<u32, ControllerError> {
    // SAFETY: `getppid` has no preconditions.
    let parent_pid = unsafe { nix::libc::getppid() };
    let parent_pid = u32::try_from(parent_pid)
        .map_err(|_| ControllerError::State("network-worker parent PID is invalid".to_owned()))?;
    if parent_pid <= 1 {
        return Err(ControllerError::State(
            "network-worker parent is not a live supervisor process".to_owned(),
        ));
    }
    let mut peer = unsafe { core::mem::zeroed::<nix::libc::ucred>() };
    let mut peer_len = nix::libc::socklen_t::try_from(core::mem::size_of_val(&peer))
        .expect("peer credential width fits socklen_t");
    // SAFETY: output pointers name one writable Linux `ucred` and its exact length.
    if unsafe {
        nix::libc::getsockopt(
            fd.as_raw_fd(),
            nix::libc::SOL_SOCKET,
            nix::libc::SO_PEERCRED,
            (&raw mut peer).cast(),
            &raw mut peer_len,
        )
    } != 0
        || peer_len as usize != core::mem::size_of_val(&peer)
    {
        return Err(io::Error::last_os_error().into());
    }
    let peer_pid = u32::try_from(peer.pid).map_err(|_| {
        ControllerError::State("network-worker IPC supervisor PID is invalid".to_owned())
    })?;
    if peer_pid != parent_pid || peer.uid != 0 || peer.gid != caller.gid {
        return Err(ControllerError::State(format!(
            "network-worker IPC supervisor credentials pid={peer_pid} uid={} gid={} are not the exact root supervisor/caller binding",
            peer.uid, peer.gid
        )));
    }
    // `SO_PEERCRED` snapshots the peer's effective credentials. This is the launch-time root
    // authentication boundary; per-message `SCM_CREDENTIALS` below deliberately uses the
    // supervisor's real caller uid/gid, as required by Linux.
    Ok(parent_pid)
}
#[cfg(target_os = "linux")]
const fn expected_supervisor_message_credentials(
    supervisor_pid: u32,
    caller: PrivilegedCaller,
) -> NetworkPeerCredentials {
    // Linux fills automatically generated `SCM_CREDENTIALS` from the sender's real ids. The
    // set-user-ID supervisor therefore sends the authenticated caller's uid/gid here even though
    // its launch-time `SO_PEERCRED` effective uid is root.
    NetworkPeerCredentials {
        pid: supervisor_pid,
        uid: caller.uid,
        gid: caller.gid,
    }
}
#[cfg(target_os = "linux")]
fn install_network_worker_parent_death_signal(
    expected_parent_pid: u32,
) -> Result<(), ControllerError> {
    linux_prctl(
        nix::libc::PR_SET_PDEATHSIG,
        nix::libc::SIGKILL as nix::libc::c_ulong,
        0,
    )?;
    // This is deliberately repeated after both exec and the permanent setresgid/setresuid drop:
    // Linux clears PDEATHSIG for either credential transition. Checking the parent afterward
    // closes the death-before-prctl race on every installation.
    // SAFETY: `getppid` has no preconditions.
    let parent_pid = unsafe { nix::libc::getppid() };
    if u32::try_from(parent_pid).ok() != Some(expected_parent_pid) {
        return Err(ControllerError::State(
            "network-worker supervisor exited during launch".to_owned(),
        ));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn complete_network_worker_identity_isolation<I, D>(
    caller: PrivilegedCaller,
    supervisor_pid: u32,
    mut install_parent_death_signal: I,
    drop_privileges: D,
) -> Result<(), ControllerError>
where
    I: FnMut(u32) -> Result<(), ControllerError>,
    D: FnOnce(PrivilegedCaller) -> Result<(), ControllerError>,
{
    install_parent_death_signal(supervisor_pid)?;
    drop_privileges(caller)?;
    // setresgid/setresuid clears PDEATHSIG even after the post-exec installation.
    install_parent_death_signal(supervisor_pid)
}
#[cfg(target_os = "linux")]
fn isolate_network_worker_before_decode<T, I, D, C, R>(
    caller: PrivilegedCaller,
    supervisor_pid: u32,
    install_parent_death_signal: I,
    drop_privileges: D,
    install_parser_containment: C,
    read_launch: R,
) -> Result<T, ControllerError>
where
    I: FnMut(u32) -> Result<(), ControllerError>,
    D: FnOnce(PrivilegedCaller) -> Result<(), ControllerError>,
    C: FnOnce() -> Result<(), ControllerError>,
    R: FnOnce() -> Result<T, ControllerError>,
{
    complete_network_worker_identity_isolation(
        caller,
        supervisor_pid,
        install_parent_death_signal,
        drop_privileges,
    )?;
    install_parser_containment()?;
    read_launch()
}
#[cfg(target_os = "linux")]
fn read_fixed_hex_environment_32(name: &str, label: &str) -> Result<[u8; 32], ControllerError> {
    let value = env::var(name).map_err(|_| {
        ControllerError::State(format!("isolated network worker is missing {label}"))
    })?;
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ControllerError::State(format!(
            "isolated network worker {label} is not exact lowercase 32-byte hex"
        )));
    }
    let mut decoded = [0_u8; 32];
    hex::decode_to_slice(value, &mut decoded)?;
    if decoded == [0_u8; 32] {
        return Err(ControllerError::State(format!(
            "isolated network worker {label} must not be all zero"
        )));
    }
    Ok(decoded)
}
#[cfg(target_os = "linux")]
fn read_network_worker_bootstrap_after_drop()
-> Result<UnprivilegedNetworkWorkerInput, ControllerError> {
    let token = read_fixed_hex_environment_32(NETWORK_WORKER_TOKEN_ENV, "IPC token")?;
    let issuer_bytes =
        read_fixed_hex_environment_32(NETWORK_WORKER_ISSUER_ENV, "issuer public key")?;
    let issuer_public_key =
        PublicKey::from_bytes(Algorithm::Ed25519, &issuer_bytes).map_err(|error| {
            ControllerError::InvalidPayload(format!(
                "network-worker issuer public key is invalid: {error}"
            ))
        })?;
    Ok(UnprivilegedNetworkWorkerInput {
        token,
        issuer_public_key,
    })
}
#[cfg(target_os = "linux")]
fn read_authenticated_network_worker_payload(
    issuer_public_key: &PublicKey,
) -> Result<AuthenticatedConnectPayload, ControllerError> {
    let raw_payload = read_connect_payload_json_from_stdin_with_deadline()?;
    let parsed = std::str::from_utf8(&raw_payload)
        .map_err(|error| {
            ControllerError::InvalidPayload(format!("connect payload stdin is not UTF-8: {error}"))
        })
        .and_then(|raw| parse_connect_payload(Some(raw)));
    drop(raw_payload);
    authenticate_connect_payload(parsed?, issuer_public_key, unix_now_ms()?)
}
#[cfg(target_os = "linux")]
fn run_network_worker_entry() -> Result<(), ControllerError> {
    let caller = current_privileged_caller()?;
    let ipc_fd = acquire_network_worker_ipc_fd()?;
    let supervisor_pid = authenticate_network_worker_supervisor(&ipc_fd, caller)?;
    let input = isolate_network_worker_before_decode(
        caller,
        supervisor_pid,
        install_network_worker_parent_death_signal,
        permanent_privilege_drop,
        install_network_worker_parser_containment,
        read_network_worker_bootstrap_after_drop,
    )?;

    // Everything below this point, including all payload, ticket, DNS, QUIC, TLS, handshake,
    // record, voucher, and packet parsing, runs with the caller's permanent uid/gid and no
    // supplementary groups or capabilities.
    // Root authority was already established with `SO_PEERCRED`, the inherited socket, the exact
    // parent pid, and the unguessable launch token before this process dropped privilege.
    let expected_supervisor = expected_supervisor_message_credentials(supervisor_pid, caller);
    let token = input.token;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let ipc = Arc::new(NetworkIpcSocket::new(ipc_fd)?);
        let mut isolation_phase = WorkerIpcPhase::Connecting;
        worker_send_ipc(
            &ipc,
            token,
            &mut isolation_phase,
            NetworkIpcKind::Isolated,
            0,
            0,
        )
        .await?;
        let authenticated = read_authenticated_network_worker_payload(&input.issuer_public_key)?;
        run_network_worker_command(authenticated, ipc, token, expected_supervisor).await
    })
}
#[cfg(not(target_os = "linux"))]
fn run_network_worker_entry() -> Result<(), ControllerError> {
    Err(ControllerError::State(
        "the isolated VPN network worker is only supported on Linux".to_owned(),
    ))
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
fn pinned_controller_exec_path() -> Result<PathBuf, ControllerError> {
    #[cfg(target_os = "linux")]
    {
        // `/proc/self/exe` resolves the already-running inode at exec time. Never re-resolve the
        // installation pathname after validating a set-user-ID process: a writable ancestor could
        // otherwise be renamed between validation and either privileged child launch.
        Ok(PathBuf::from(PINNED_SELF_EXEC_PATH))
    }
    #[cfg(not(target_os = "linux"))]
    {
        env::current_exe().map_err(Into::into)
    }
}
fn pinned_controller_command() -> Result<ProcessCommand, ControllerError> {
    Ok(ProcessCommand::new(pinned_controller_exec_path()?))
}
fn current_controller_path() -> Option<String> {
    env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(ToOwned::to_owned))
}
fn scrub_stale_process_with<F>(
    state: &mut State,
    now_ms: u64,
    mut identity_alive: F,
) -> Result<(), ControllerError>
where
    F: FnMut(&WorkerProcessIdentity) -> Result<bool, ControllerError>,
{
    if let Some(identity) = state.worker_identity.as_ref()
        && !identity_alive(identity)?
    {
        state.worker_identity = None;
    }
    if let Some(identity) = state.network_worker_identity.as_ref()
        && !identity_alive(identity)?
    {
        state.network_worker_identity = None;
    }
    demote_expired_active_state_at(state, now_ms);
    if state.active && !active_runtime_state_complete_at(state, now_ms) {
        state.active = false;
        state.repair_required = true;
        state.message = "repair required".to_owned();
    }
    if state.worker_identity.is_none() {
        state.active = false;
        state.repair_required =
            state.applied_network.is_some() || state.network_worker_identity.is_some();
        if !state.repair_required {
            clear_session_binding(state);
        } else {
            state.message = "repair required".to_owned();
        }
    }
    Ok(())
}

fn scrub_stale_process(state: &mut State) -> Result<(), ControllerError> {
    scrub_stale_process_with(state, unix_now_ms()?, worker_identity_alive)
}
#[cfg(target_os = "linux")]
fn capture_worker_identity(
    pid: u32,
    role: WorkerRole,
) -> Result<WorkerProcessIdentity, ControllerError> {
    capture_worker_identity_with_pidfd(pid, role).map(|(identity, _pidfd)| identity)
}
#[cfg(target_os = "linux")]
fn capture_worker_identity_with_pidfd(
    pid: u32,
    role: WorkerRole,
) -> Result<(WorkerProcessIdentity, OwnedFd), ControllerError> {
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
    let current_metadata = fs::metadata(pinned_controller_exec_path()?)?;
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
    Ok((identity, pidfd))
}
#[cfg(target_os = "linux")]
fn observe_linux_process_start_time(pid: u32) -> Result<Option<u64>, ControllerError> {
    let stat = match read_small_proc_file(Path::new(&format!("/proc/{pid}/stat")), 16 * 1024) {
        Ok(stat) => stat,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let stat = std::str::from_utf8(&stat).map_err(|error| {
        ControllerError::State(format!("worker process {pid} stat is not UTF-8: {error}"))
    })?;
    let (process_state, start_time) = parse_linux_process_stat(stat).map_err(|error| {
        ControllerError::State(format!("worker process {pid} stat is malformed: {error}"))
    })?;
    Ok((process_state != 'Z').then_some(start_time))
}
#[cfg(target_os = "linux")]
fn unique_proc_status_value<'a>(status: &'a str, label: &str) -> Result<&'a str, ControllerError> {
    let prefix = format!("{label}:");
    let mut matches = status
        .lines()
        .filter_map(|line| line.strip_prefix(prefix.as_str()));
    let value = matches
        .next()
        .ok_or_else(|| ControllerError::State(format!("network-worker status omits {label}")))?;
    if matches.next().is_some() {
        return Err(ControllerError::State(format!(
            "network-worker status duplicates {label}"
        )));
    }
    Ok(value.trim())
}
#[cfg(target_os = "linux")]
fn verify_status_identity_quad(
    status: &str,
    label: &str,
    expected: u32,
) -> Result<(), ControllerError> {
    let values = unique_proc_status_value(status, label)?
        .split_ascii_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            ControllerError::State(format!("network-worker status has malformed {label}"))
        })?;
    if values.as_slice() != [expected, expected, expected, expected] {
        return Err(ControllerError::State(format!(
            "network-worker {label} identities are not permanently caller-bound: {values:?}"
        )));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn verify_network_worker_isolation_status(
    status: &str,
    caller: PrivilegedCaller,
) -> Result<(), ControllerError> {
    verify_status_identity_quad(status, "Uid", caller.uid)?;
    verify_status_identity_quad(status, "Gid", caller.gid)?;
    if !unique_proc_status_value(status, "Groups")?.is_empty() {
        return Err(ControllerError::State(
            "network-worker supplementary groups are not empty".to_owned(),
        ));
    }
    for label in ["CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"] {
        let value = unique_proc_status_value(status, label)?;
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().any(|byte| byte != b'0')
        {
            return Err(ControllerError::State(format!(
                "network-worker {label} is not zero"
            )));
        }
    }
    if unique_proc_status_value(status, "NoNewPrivs")? != "1"
        || unique_proc_status_value(status, "Seccomp")? != "2"
    {
        return Err(ControllerError::State(
            "network-worker has not enabled no-new-privileges and seccomp filtering".to_owned(),
        ));
    }
    if unique_proc_status_value(status, "TracerPid")? != "0" {
        return Err(ControllerError::State(
            "network-worker is unexpectedly traced".to_owned(),
        ));
    }
    if unique_proc_status_value(status, "Threads")? != "1" {
        return Err(ControllerError::State(
            "network-worker created threads before the isolation barrier".to_owned(),
        ));
    }
    if status.lines().any(|line| {
        line.strip_prefix("CoreDumping:")
            .is_some_and(|value| value.trim() != "0")
    }) {
        return Err(ControllerError::State(
            "network-worker is actively dumping core".to_owned(),
        ));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn verify_network_worker_isolation(
    process: &NetworkWorkerProcess,
    caller: PrivilegedCaller,
) -> Result<(), ControllerError> {
    if !pidfd_send_signal(&process.pidfd, 0)? {
        return Err(ControllerError::State(
            "network worker exited before root isolation verification".to_owned(),
        ));
    }
    if observe_linux_process_start_time(process.identity.pid)?
        != Some(process.identity.start_time_ticks)
    {
        return Err(ControllerError::State(
            "network worker changed identity before root isolation verification".to_owned(),
        ));
    }
    let proc_metadata = fs::metadata(format!("/proc/{}", process.identity.pid))?;
    if proc_metadata.uid() != 0 || proc_metadata.gid() != 0 {
        return Err(ControllerError::State(
            "network-worker proc directory is not root-custodied after disabling dumpability"
                .to_owned(),
        ));
    }
    let status_bytes = read_small_proc_file(
        Path::new(&format!("/proc/{}/status", process.identity.pid)),
        128 * 1024,
    )?;
    let status = std::str::from_utf8(&status_bytes).map_err(|error| {
        ControllerError::State(format!("network-worker status is not UTF-8: {error}"))
    })?;
    verify_network_worker_isolation_status(status, caller)?;
    if observe_linux_process_start_time(process.identity.pid)?
        != Some(process.identity.start_time_ticks)
        || !pidfd_send_signal(&process.pidfd, 0)?
    {
        return Err(ControllerError::State(
            "network worker changed identity during root isolation verification".to_owned(),
        ));
    }
    Ok(())
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
    if !worker_cmdline_has_exact_role(&cmdline, role) {
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
fn worker_cmdline_has_exact_role(cmdline: &[u8], role: WorkerRole) -> bool {
    let mut fields = cmdline.split(|byte| *byte == 0);
    let Some(program) = fields.next() else {
        return false;
    };
    let Some(command) = fields.next() else {
        return false;
    };
    !program.is_empty()
        && command == role.subcommand().as_bytes()
        && fields.next() == Some(&[][..])
        && fields.next().is_none()
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
enum BoundPersistedProcess {
    Gone,
    Live {
        pidfd: OwnedFd,
        identity_matches: bool,
    },
}
#[cfg(any(target_os = "linux", test))]
fn persisted_start_time_matches(expected: u64, observed: Option<u64>) -> bool {
    observed == Some(expected)
}
#[cfg(target_os = "linux")]
fn bind_persisted_process(
    identity: &WorkerProcessIdentity,
) -> Result<BoundPersistedProcess, ControllerError> {
    let Some(pidfd) = open_pidfd(identity.pid)? else {
        return Ok(BoundPersistedProcess::Gone);
    };
    if !pidfd_send_signal(&pidfd, 0)? {
        return Ok(BoundPersistedProcess::Gone);
    }
    if !persisted_start_time_matches(
        identity.start_time_ticks,
        observe_linux_process_start_time(identity.pid)?,
    ) {
        // A different start time proves PID reuse. Never signal the unrelated process.
        return Ok(BoundPersistedProcess::Gone);
    }
    let observed = observe_linux_worker_identity(identity.pid, identity.role)?;
    Ok(BoundPersistedProcess::Live {
        pidfd,
        identity_matches: observed.as_ref() == Some(identity),
    })
}
#[cfg(target_os = "linux")]
fn worker_identity_alive(identity: &WorkerProcessIdentity) -> Result<bool, ControllerError> {
    match bind_persisted_process(identity)? {
        BoundPersistedProcess::Gone => Ok(false),
        BoundPersistedProcess::Live {
            identity_matches: true,
            ..
        } => Ok(true),
        BoundPersistedProcess::Live {
            identity_matches: false,
            ..
        } => Err(ControllerError::State(format!(
            "worker process {} is live with the persisted start time but its executable or argv identity drifted",
            identity.pid
        ))),
    }
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
fn signal_worker_exact(
    identity: &WorkerProcessIdentity,
    signal: i32,
) -> Result<(), ControllerError> {
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
    let _ = pidfd_send_signal(&pidfd, signal)?;
    Ok(())
}
#[cfg(target_os = "linux")]
fn pidfd_wait_readable(pidfd: &OwnedFd, timeout_limit: Duration) -> Result<bool, ControllerError> {
    let deadline = Instant::now() + timeout_limit;
    let mut descriptor = nix::libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: nix::libc::POLLIN,
        revents: 0,
    };
    loop {
        let timeout_ms = deadline
            .checked_duration_since(Instant::now())
            .map_or(0, |remaining| {
                remaining.as_millis().max(1).min(i32::MAX as u128) as nix::libc::c_int
            });
        // SAFETY: `descriptor` is a live one-element poll array.
        let result = unsafe { nix::libc::poll(&raw mut descriptor, 1, timeout_ms) };
        if result > 0 {
            return Ok(descriptor.revents & (nix::libc::POLLIN | nix::libc::POLLHUP) != 0);
        }
        if result == 0 {
            return Ok(false);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}
#[cfg(target_os = "linux")]
fn terminate_and_wait_persisted_worker(
    identity: &WorkerProcessIdentity,
    grace: Duration,
) -> Result<(), ControllerError> {
    let BoundPersistedProcess::Live { pidfd, .. } = bind_persisted_process(identity)? else {
        return Ok(());
    };
    let _ = pidfd_send_signal(&pidfd, nix::libc::SIGTERM)?;
    if pidfd_wait_readable(&pidfd, grace)? {
        return Ok(());
    }
    let _ = pidfd_send_signal(&pidfd, nix::libc::SIGKILL)?;
    if !pidfd_wait_readable(&pidfd, Duration::from_secs(5))? {
        return Err(ControllerError::State(format!(
            "worker process {} did not exit after exact pidfd SIGKILL; retaining its identity and network journal without cleanup",
            identity.pid
        )));
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn terminate_and_wait_persisted_worker(
    _identity: &WorkerProcessIdentity,
    _grace: Duration,
) -> Result<(), ControllerError> {
    Err(ControllerError::State(
        "refusing to terminate a persisted VPN worker without Linux pidfd support".to_owned(),
    ))
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
fn parse_relay_mldsa65_public_key_hex(
    value: &str,
    label: &str,
) -> Result<[u8; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1], ControllerError> {
    let expected_hex_len = VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1
        .checked_mul(2)
        .expect("ML-DSA-65 public key hex length fits usize");
    if value.len() != expected_hex_len
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} must be exactly {expected_hex_len} lowercase hexadecimal characters"
        )));
    }
    let mut bytes = [0u8; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1];
    hex::decode_to_slice(value, &mut bytes)
        .expect("canonical hexadecimal validation makes decoding infallible");
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} must not be all zero"
        )));
    }
    Ok(bytes)
}
fn parse_canonical_secret_hex_32(value: &str, label: &str) -> Result<[u8; 32], ControllerError> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} must be exactly 64 lowercase hexadecimal characters"
        )));
    }
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(value, &mut bytes)
        .map_err(|error| ControllerError::InvalidPayload(format!("{label} is invalid: {error}")))?;
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
    validate_connect_payload_keys(object)?;
    let payload = ConnectPayload {
        session_id: require_json_string(object, &["sessionId"], "sessionId")?,
        relay_endpoint: require_json_string(object, &["relayEndpoint"], "relayEndpoint")?,
        helper_ticket_hex: require_json_string(object, &["helperTicketHex"], "helperTicketHex")?,
        relay_id_hex: require_json_string(object, &["relayIdHex"], "relayIdHex")?,
        relay_mldsa65_public_key_hex: require_json_string(
            object,
            &["relayMlDsa65PublicKeyHex"],
            "relayMlDsa65PublicKeyHex",
        )?,
        descriptor_commit_hex: require_json_string(
            object,
            &["descriptorCommitHex"],
            "descriptorCommitHex",
        )?,
        tls_server_name: require_json_string(object, &["tlsServerName"], "tlsServerName")?,
        relay_tls_spki_sha256_hex: require_json_string(
            object,
            &["relayTlsSpkiSha256Hex"],
            "relayTlsSpkiSha256Hex",
        )?,
        relay_certificate_sha256_hex: require_json_string(
            object,
            &["relayCertificateSha256Hex"],
            "relayCertificateSha256Hex",
        )?,
        directory_snapshot_digest_hex: require_json_string(
            object,
            &["directorySnapshotDigestHex"],
            "directorySnapshotDigestHex",
        )?,
        padding_budget_ms: require_json_u16(object, &["paddingBudgetMs"], "paddingBudgetMs")?,
        route_pushes: optional_json_string_array(object, &["routePushes"])?,
        excluded_routes: optional_json_string_array(object, &["excludedRoutes"])?,
        dns_servers: optional_json_string_array(object, &["dnsServers"])?,
        tunnel_addresses: optional_json_string_array(object, &["tunnelAddresses"])?,
        mtu_bytes: require_json_u64(object, &["mtuBytes"], "mtuBytes")?,
        metering_private_key_seed_hex: require_json_string(
            object,
            &["meteringPrivateKeySeedHex"],
            "meteringPrivateKeySeedHex",
        )?,
    };
    validate_connect_payload(payload)
}
fn validate_connect_payload_keys(object: &JsonMap) -> Result<(), ControllerError> {
    const ALLOWED_KEYS: &[&str] = &[
        "sessionId",
        "relayEndpoint",
        "helperTicketHex",
        "relayIdHex",
        "relayMlDsa65PublicKeyHex",
        "descriptorCommitHex",
        "tlsServerName",
        "relayTlsSpkiSha256Hex",
        "relayCertificateSha256Hex",
        "directorySnapshotDigestHex",
        "paddingBudgetMs",
        "routePushes",
        "excludedRoutes",
        "dnsServers",
        "tunnelAddresses",
        "mtuBytes",
        "meteringPrivateKeySeedHex",
    ];
    if let Some(key) = object
        .keys()
        .find(|key| !ALLOWED_KEYS.contains(&key.as_str()))
    {
        return Err(ControllerError::InvalidPayload(format!(
            "unknown connect payload field {key:?}"
        )));
    }
    Ok(())
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
        payload.helper_ticket_hex.as_str(),
        "helperTicketHex",
        MAX_HELPER_TICKET_HEX_BYTES_V1,
    )?;
    validate_text_field(
        payload.tls_server_name.as_str(),
        "tlsServerName",
        MAX_TLS_SERVER_NAME_BYTES_V1,
    )?;
    let _ = parse_canonical_secret_hex_32(
        payload.metering_private_key_seed_hex.as_str(),
        "meteringPrivateKeySeedHex",
    )?;
    let session_id = parse_canonical_session_id(payload.session_id.as_str())?;
    validate_network_policy_entries(&payload.route_pushes, "routePushes")?;
    validate_network_policy_entries(&payload.excluded_routes, "excludedRoutes")?;
    validate_network_policy_entries(&payload.dns_servers, "dnsServers")?;
    validate_network_policy_entries(&payload.tunnel_addresses, "tunnelAddresses")?;
    validate_v1_policy_cardinality(
        &payload.route_pushes,
        "routePushes",
        1,
        VPN_MAX_ROUTE_ENTRIES_V1,
        VPN_MAX_ROUTE_BYTES_V1,
    )?;
    validate_v1_policy_cardinality(
        &payload.excluded_routes,
        "excludedRoutes",
        0,
        VPN_MAX_ROUTE_ENTRIES_V1,
        VPN_MAX_ROUTE_BYTES_V1,
    )?;
    validate_v1_policy_cardinality(
        &payload.dns_servers,
        "dnsServers",
        1,
        VPN_MAX_DNS_ENTRIES_V1,
        VPN_MAX_DNS_BYTES_V1,
    )?;
    let route_pushes =
        validate_canonical_network_cidr_entries(&payload.route_pushes, "routePushes")?;
    let excluded_routes =
        validate_canonical_network_cidr_entries(&payload.excluded_routes, "excludedRoutes")?;
    if route_pushes
        .iter()
        .any(|route| excluded_routes.contains(route))
    {
        return Err(ControllerError::InvalidPayload(
            "routePushes and excludedRoutes must not contain the same canonical network prefix"
                .to_owned(),
        ));
    }
    validate_canonical_host_cidr_entries(&payload.tunnel_addresses, "tunnelAddresses")?;
    let address_plan = derive_vpn_session_address_plan_v1(session_id);
    if payload.tunnel_addresses != address_plan.client_tunnel_addresses {
        return Err(ControllerError::InvalidPayload(
            "tunnelAddresses must exactly match the V1 addresses derived from sessionId".to_owned(),
        ));
    }
    validate_dns_servers(&payload.dns_servers)?;
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
    let relay_mldsa65_public_key = parse_relay_mldsa65_public_key_hex(
        payload.relay_mldsa65_public_key_hex.as_str(),
        "relayMlDsa65PublicKeyHex",
    )?;
    PublicKey::from_bytes(Algorithm::MlDsa, &relay_mldsa65_public_key).map_err(|error| {
        ControllerError::InvalidPayload(format!(
            "relayMlDsa65PublicKeyHex is not a valid ML-DSA-65 key: {error}"
        ))
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
    if payload.padding_budget_ms == 0 {
        return Err(ControllerError::InvalidPayload(
            "paddingBudgetMs must be greater than zero".to_owned(),
        ));
    }
    if payload.mtu_bytes != u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES) {
        return Err(ControllerError::InvalidPayload(format!(
            "mtuBytes must be exactly {VPN_DEFAULT_TUNNEL_MTU_BYTES} in V1"
        )));
    }
    Ok(())
}
fn authenticate_connect_payload(
    payload: ConnectPayload,
    issuer_public_key: &PublicKey,
    now_ms: u64,
) -> Result<AuthenticatedConnectPayload, ControllerError> {
    let ticket = parse_authenticated_helper_ticket(
        payload.helper_ticket_hex.as_str(),
        issuer_public_key,
        now_ms,
    )?;
    let session_id = parse_canonical_session_id(payload.session_id.as_str())?;
    if ticket.session_id != session_id {
        return Err(ControllerError::InvalidPayload(
            "authenticated helper ticket session id does not match sessionId".to_owned(),
        ));
    }
    let address_plan = derive_vpn_session_address_plan_v1(ticket.session_id);
    if payload.tunnel_addresses != address_plan.client_tunnel_addresses
        || ticket.client_ipv4_address != address_plan.client_ipv4_address
        || ticket.client_ipv6_address != address_plan.client_ipv6_address
    {
        return Err(ControllerError::InvalidPayload(
            "authenticated helper ticket and connect payload do not carry the canonical client tunnel addresses"
                .to_owned(),
        ));
    }
    let relay_id = parse_canonical_nonzero_hex_32(payload.relay_id_hex.as_str(), "relayIdHex")?;
    if ticket.relay_id != relay_id {
        return Err(ControllerError::InvalidPayload(
            "authenticated helper ticket relay identity does not match relayIdHex".to_owned(),
        ));
    }
    if ticket.network_policy_hash != connect_payload_network_policy_hash(&payload)? {
        return Err(ControllerError::InvalidPayload(
            "authenticated helper ticket policy hash does not match the relay trust, padding, route, DNS, tunnel address, and MTU inputs"
                .to_owned(),
        ));
    }
    Ok(AuthenticatedConnectPayload { payload, ticket })
}
fn ensure_authenticated_ticket_unexpired_at(expires_at_ms: u64) -> Result<(), ControllerError> {
    let now_ms = unix_now_ms()?;
    if expires_at_ms <= now_ms {
        return Err(ControllerError::InvalidPayload(format!(
            "authenticated helper ticket expired at {} before privileged tunnel preparation (current time {now_ms})",
            expires_at_ms
        )));
    }
    Ok(())
}

fn ensure_authenticated_ticket_unexpired_for_connected_state_at(
    expires_at_ms: u64,
    now_ms: u64,
) -> Result<(), ControllerError> {
    if expires_at_ms <= now_ms {
        return Err(ControllerError::InvalidPayload(format!(
            "authenticated helper ticket expired at {expires_at_ms} before connected state publication (current time {now_ms})"
        )));
    }
    Ok(())
}

fn ensure_authenticated_ticket_unexpired_for_connected_state(
    expires_at_ms: u64,
) -> Result<(), ControllerError> {
    ensure_authenticated_ticket_unexpired_for_connected_state_at(expires_at_ms, unix_now_ms()?)
}

fn ensure_connected_publication_ready_at(
    expires_at_ms: u64,
    now_ms: u64,
    exact_child_alive: bool,
) -> Result<(), ControllerError> {
    ensure_authenticated_ticket_unexpired_for_connected_state_at(expires_at_ms, now_ms)?;
    if !exact_child_alive {
        return Err(ControllerError::State(
            "exact network worker is not alive at the connected-state publication barrier"
                .to_owned(),
        ));
    }
    Ok(())
}

fn authenticated_ticket_expiry_remaining_at(
    expires_at_ms: u64,
    now_ms: u64,
) -> Result<Duration, ControllerError> {
    let remaining_ms = expires_at_ms.checked_sub(now_ms).filter(|value| *value > 0).ok_or_else(
        || {
            ControllerError::InvalidPayload(format!(
                "authenticated helper ticket expired at {expires_at_ms} before the active packet loop (current time {now_ms})"
            ))
        },
    )?;
    Ok(Duration::from_millis(remaining_ms))
}

fn authenticated_ticket_expiry_deadline(
    expires_at_ms: u64,
) -> Result<tokio::time::Instant, ControllerError> {
    // Convert the signed wall-clock expiry to one monotonic deadline exactly once. Recreating a
    // relative timeout after STARTED or inside either packet loop would let scheduling, activity,
    // or a wall-clock rollback extend the issuer-authorized lifetime.
    let monotonic_anchor = tokio::time::Instant::now();
    let wall_now_ms = unix_now_ms()?;
    authenticated_ticket_expiry_deadline_at(expires_at_ms, wall_now_ms, monotonic_anchor)
}

fn authenticated_ticket_expiry_deadline_at(
    expires_at_ms: u64,
    wall_now_ms: u64,
    monotonic_anchor: tokio::time::Instant,
) -> Result<tokio::time::Instant, ControllerError> {
    let expiry_remaining = authenticated_ticket_expiry_remaining_at(expires_at_ms, wall_now_ms)?;
    monotonic_anchor
        .checked_add(expiry_remaining)
        .ok_or_else(|| {
            ControllerError::State(
                "authenticated helper ticket expiry exceeds the monotonic clock range".to_owned(),
            )
        })
}
fn parse_authenticated_helper_ticket(
    hex_ticket: &str,
    issuer_public_key: &PublicKey,
    now_ms: u64,
) -> Result<VpnHelperTicketV1, ControllerError> {
    VpnHelperTicketV1::parse_hex(hex_ticket, issuer_public_key, now_ms).map_err(|error| {
        ControllerError::InvalidPayload(format!(
            "helperTicketHex failed issuer authentication: {error}"
        ))
    })
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
fn validate_v1_policy_cardinality(
    entries: &[String],
    label: &str,
    minimum: usize,
    maximum: usize,
    maximum_entry_bytes: usize,
) -> Result<(), ControllerError> {
    if entries.len() < minimum || entries.len() > maximum {
        return Err(ControllerError::InvalidPayload(format!(
            "{label} must contain between {minimum} and {maximum} entries in V1"
        )));
    }
    if let Some((index, _)) = entries
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.len() > maximum_entry_bytes)
    {
        return Err(ControllerError::InvalidPayload(format!(
            "{label}[{index}] exceeds the V1 limit of {maximum_entry_bytes} bytes"
        )));
    }
    Ok(())
}
fn validate_canonical_host_cidr_entries(
    entries: &[String],
    label: &str,
) -> Result<Vec<ParsedCidr>, ControllerError> {
    let mut parsed_entries = Vec::with_capacity(entries.len());
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
        if parsed_entries.contains(&parsed) {
            return Err(ControllerError::InvalidPayload(format!(
                "{label} must not contain semantically duplicate entries"
            )));
        }
        parsed_entries.push(parsed);
    }
    Ok(parsed_entries)
}
fn network_prefix(parsed: ParsedCidr) -> IpAddr {
    match parsed.address {
        IpAddr::V4(address) => {
            let mask = if parsed.prefix == 0 {
                0
            } else {
                u32::MAX << (32 - parsed.prefix)
            };
            IpAddr::V4(Ipv4Addr::from(u32::from(address) & mask))
        }
        IpAddr::V6(address) => {
            let mask = if parsed.prefix == 0 {
                0
            } else {
                u128::MAX << (128 - parsed.prefix)
            };
            IpAddr::V6(Ipv6Addr::from(u128::from(address) & mask))
        }
    }
}
fn validate_canonical_network_cidr_entries(
    entries: &[String],
    label: &str,
) -> Result<Vec<ParsedCidr>, ControllerError> {
    let parsed_entries = validate_canonical_host_cidr_entries(entries, label)?;
    for (index, parsed) in parsed_entries.iter().copied().enumerate() {
        let network = network_prefix(parsed);
        if parsed.address != network {
            return Err(ControllerError::InvalidPayload(format!(
                "{label}[{index}] must clear all host bits; canonical network prefix is {network}/{}",
                parsed.prefix
            )));
        }
    }
    Ok(parsed_entries)
}
fn validate_dns_servers(entries: &[String]) -> Result<(), ControllerError> {
    let mut parsed_entries = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let address = entry.parse::<IpAddr>().map_err(|_| {
            ControllerError::InvalidPayload(format!(
                "dnsServers[{index}] must be a canonical IP address"
            ))
        })?;
        let canonical = address.to_string();
        let normalized = match address {
            IpAddr::V6(address) => address
                .to_ipv4_mapped()
                .map_or(IpAddr::V6(address), IpAddr::V4),
            IpAddr::V4(_) => address,
        };
        let limited_broadcast =
            matches!(normalized, IpAddr::V4(address) if address == Ipv4Addr::BROADCAST);
        if entry != &canonical
            || normalized.is_unspecified()
            || normalized.is_multicast()
            || limited_broadcast
        {
            return Err(ControllerError::InvalidPayload(format!(
                "dnsServers[{index}] must be a canonical unicast IP address"
            )));
        }
        if parsed_entries.contains(&normalized) {
            return Err(ControllerError::InvalidPayload(
                "dnsServers must not contain semantically duplicate entries".to_owned(),
            ));
        }
        parsed_entries.push(normalized);
    }
    Ok(())
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
#[cfg(target_os = "linux")]
fn wait_for_stdin_until(deadline: Instant, label: &str) -> Result<(), ControllerError> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| ControllerError::State(format!("timed out reading {label}")))?;
        let timeout_ms = remaining.as_millis().max(1).min(i32::MAX as u128) as nix::libc::c_int;
        let mut descriptor = nix::libc::pollfd {
            fd: nix::libc::STDIN_FILENO,
            events: nix::libc::POLLIN | nix::libc::POLLHUP,
            revents: 0,
        };
        // SAFETY: `descriptor` is a live single-element poll array for the duration of the call.
        let result = unsafe { nix::libc::poll(&raw mut descriptor, 1, timeout_ms) };
        if result > 0 {
            if descriptor.revents & (nix::libc::POLLERR | nix::libc::POLLNVAL) != 0 {
                return Err(ControllerError::State(format!(
                    "failed while waiting for {label}"
                )));
            }
            return Ok(());
        }
        if result == 0 {
            return Err(ControllerError::State(format!("timed out reading {label}")));
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}
#[cfg(target_os = "linux")]
fn read_sensitive_stdin_bounded_until(
    max_bytes: usize,
    label: &str,
    deadline: Instant,
) -> Result<WipeBytes, ControllerError> {
    let mut stdin = io::stdin().lock();
    let mut bytes = WipeBytes(Vec::new());
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        wait_for_stdin_until(deadline, label)?;
        let probe_only = bytes.len() == max_bytes;
        let read_len = if probe_only {
            1
        } else {
            (max_bytes - bytes.len()).min(chunk.len())
        };
        let count = loop {
            match stdin.read(&mut chunk[..read_len]) {
                Ok(count) => break count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        };
        if count == 0 {
            break;
        }
        if probe_only {
            return Err(ControllerError::InvalidPayload(format!(
                "{label} exceeds the v1 limit of {max_bytes} bytes"
            )));
        }
        bytes.0.try_reserve_exact(count).map_err(|error| {
            ControllerError::InvalidPayload(format!(
                "failed to reserve storage while reading {label}: {error}"
            ))
        })?;
        bytes.0.extend_from_slice(&chunk[..count]);
    }
    Ok(bytes)
}
fn read_connect_payload_json_from_stdin_with_deadline() -> Result<WipeBytes, ControllerError> {
    #[cfg(target_os = "linux")]
    let raw_payload = read_sensitive_stdin_bounded_until(
        MAX_CONNECT_PAYLOAD_FRAME_BYTES_V1,
        "connect payload stdin",
        Instant::now() + CONNECT_INPUT_TIMEOUT,
    )?;
    #[cfg(not(target_os = "linux"))]
    let raw_payload = read_connect_payload_json_from_stdin()?;
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
        verify_relay_tls_spki_pin(end_entity.as_ref(), &self.relay_tls_spki_sha256)?;
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
fn verify_relay_tls_spki_pin(
    certificate_der: &[u8],
    expected_spki_sha256: &[u8; 32],
) -> Result<(), rustls::Error> {
    let spki_digest = leaf_certificate_spki_sha256(certificate_der).map_err(|error| {
        rustls::Error::General(format!("invalid relay leaf certificate: {error}"))
    })?;
    if spki_digest != *expected_spki_sha256 {
        return Err(rustls::Error::General(
            "relay TLS SPKI pin mismatch".to_owned(),
        ));
    }
    Ok(())
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
        soranet::vpn::VpnTariffV1,
    };

    #[test]
    fn fixed_secret_owner_redacts_and_uses_the_drop_clear_path() {
        let mut guarded = WipeArray([0xA5; 16]);
        assert!(std::mem::needs_drop::<WipeArray<16>>());
        assert_eq!(
            format!("{guarded:?}"),
            "WipeArray(<redacted 16-byte buffer>)"
        );
        assert!(!format!("{guarded:?}").contains("165"));

        guarded.clear();
        assert!(guarded.iter().all(|byte| *byte == 0));

        let mut allocation = vec![0x5A; 64];
        allocation.truncate(17);
        let capacity = allocation.capacity();
        wipe_secret_vec(&mut allocation);
        assert!(allocation.is_empty());
        assert_eq!(allocation.capacity(), capacity);

        let mut string = String::with_capacity(64);
        string.push_str("sensitive credential");
        wipe_secret_string(&mut string);
        assert!(string.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_network_ipc_authenticates_frames_and_transfers_one_descriptor() {
        let (supervisor, worker) =
            create_network_ipc_socketpair().expect("create credentialed IPC socket pair");
        let token = [0xA5; 32];
        let expected_peer = NetworkPeerCredentials {
            pid: std::process::id(),
            // SAFETY: these process identity getters have no preconditions.
            uid: unsafe { nix::libc::getuid() },
            // SAFETY: these process identity getters have no preconditions.
            gid: unsafe { nix::libc::getgid() },
        };

        let ready = NetworkIpcFrame::new(NetworkIpcKind::WorkerReady, token, 0, 0);
        send_network_ipc_once(supervisor.as_raw_fd(), &ready.encode(), None)
            .expect("send credentialed frame");
        let received = receive_network_ipc_once(worker.as_raw_fd(), &token, expected_peer)
            .expect("receive credentialed frame");
        assert_eq!(received.frame, ready);
        assert!(received.descriptors.is_empty());

        let transferred = fs::File::open("/dev/null").expect("open descriptor fixture");
        let tun_ready = NetworkIpcFrame::new(NetworkIpcKind::TunReady, token, 1_280, 0);
        send_network_ipc_once(
            worker.as_raw_fd(),
            &tun_ready.encode(),
            Some(transferred.as_raw_fd()),
        )
        .expect("send descriptor frame");
        let mut received = receive_network_ipc_once(supervisor.as_raw_fd(), &token, expected_peer)
            .expect("receive descriptor frame");
        assert_eq!(received.frame, tun_ready);
        assert_eq!(received.descriptors.len(), 1);
        let descriptor = received.descriptors.pop().expect("one received descriptor");
        // SAFETY: `descriptor` is live and `F_GETFD` only reads its descriptor flags.
        let flags = unsafe { nix::libc::fcntl(descriptor.as_raw_fd(), nix::libc::F_GETFD) };
        assert!(flags >= 0, "received descriptor must be live");
        assert_ne!(flags & nix::libc::FD_CLOEXEC, 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn supervisor_message_credentials_use_real_caller_identity_after_root_launch_binding() {
        let caller = PrivilegedCaller {
            uid: 1_000,
            gid: 1_001,
        };
        assert_eq!(
            expected_supervisor_message_credentials(42, caller),
            NetworkPeerCredentials {
                pid: 42,
                uid: caller.uid,
                gid: caller.gid,
            }
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn malformed_multi_fd_ipc_closes_every_received_descriptor() {
        let (sender, receiver) =
            create_network_ipc_socketpair().expect("create credentialed IPC socket pair");
        let first = fs::File::open("/dev/null").expect("first descriptor fixture");
        let second = fs::File::open("/dev/null").expect("second descriptor fixture");
        let token = [0xC3; 32];
        let frame = NetworkIpcFrame::new(NetworkIpcKind::TunReady, token, 1_280, 0).encode();
        let descriptor_count_before = fs::read_dir("/proc/self/fd")
            .expect("inspect descriptor table")
            .count();

        let mut iov = nix::libc::iovec {
            iov_base: frame.as_ptr().cast_mut().cast(),
            iov_len: frame.len(),
        };
        let mut control = [0_usize; NETWORK_IPC_CONTROL_WORDS];
        let mut message = unsafe { core::mem::zeroed::<nix::libc::msghdr>() };
        message.msg_iov = &raw mut iov;
        message.msg_iovlen = 1;
        let rights_bytes = 2 * core::mem::size_of::<RawFd>();
        message.msg_control = control.as_mut_ptr().cast();
        // SAFETY: CMSG_SPACE is pure size arithmetic for the two-descriptor payload.
        message.msg_controllen = unsafe {
            nix::libc::CMSG_SPACE(u32::try_from(rights_bytes).expect("small rights payload"))
                as usize
        };
        // SAFETY: the aligned control buffer is large enough for the header and two RawFd values.
        unsafe {
            let header = nix::libc::CMSG_FIRSTHDR(&message);
            assert!(!header.is_null());
            (*header).cmsg_level = nix::libc::SOL_SOCKET;
            (*header).cmsg_type = nix::libc::SCM_RIGHTS;
            (*header).cmsg_len =
                nix::libc::CMSG_LEN(u32::try_from(rights_bytes).expect("small rights payload"))
                    as usize;
            let data = nix::libc::CMSG_DATA(header).cast::<RawFd>();
            core::ptr::write_unaligned(data, first.as_raw_fd());
            core::ptr::write_unaligned(data.add(1), second.as_raw_fd());
            assert_eq!(
                nix::libc::sendmsg(
                    sender.as_raw_fd(),
                    &raw const message,
                    nix::libc::MSG_NOSIGNAL,
                ),
                NETWORK_WORKER_IPC_FRAME_BYTES as isize
            );
        }
        let expected_peer = NetworkPeerCredentials {
            pid: std::process::id(),
            // SAFETY: process identity getters have no preconditions.
            uid: unsafe { nix::libc::getuid() },
            // SAFETY: process identity getters have no preconditions.
            gid: unsafe { nix::libc::getgid() },
        };
        let error = match receive_network_ipc_once(receiver.as_raw_fd(), &token, expected_peer) {
            Ok(_) => panic!("multiple SCM_RIGHTS descriptors are forbidden"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("more than one descriptor"));
        let descriptor_count_after = fs::read_dir("/proc/self/fd")
            .expect("inspect descriptor table after rejection")
            .count();
        assert_eq!(
            descriptor_count_after, descriptor_count_before,
            "malformed ancillary data must not leak installed descriptors"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn fixed_plan_rejects_and_closes_every_received_descriptor() {
        let (sender, receiver) =
            create_network_ipc_socketpair().expect("create credentialed IPC socket pair");
        let first = fs::File::open("/dev/null").expect("first descriptor fixture");
        let second = fs::File::open("/dev/null").expect("second descriptor fixture");
        let frame = [0_u8; NETWORK_WORKER_PLAN_FRAME_BYTES];
        let descriptor_count_before = fs::read_dir("/proc/self/fd")
            .expect("inspect descriptor table")
            .count();
        let mut iov = nix::libc::iovec {
            iov_base: frame.as_ptr().cast_mut().cast(),
            iov_len: frame.len(),
        };
        let mut control = [0_usize; NETWORK_IPC_CONTROL_WORDS];
        let mut message = unsafe { core::mem::zeroed::<nix::libc::msghdr>() };
        message.msg_iov = &raw mut iov;
        message.msg_iovlen = 1;
        let rights_bytes = 2 * core::mem::size_of::<RawFd>();
        message.msg_control = control.as_mut_ptr().cast();
        // SAFETY: CMSG_SPACE is pure size arithmetic for the two-descriptor payload.
        message.msg_controllen = unsafe {
            nix::libc::CMSG_SPACE(u32::try_from(rights_bytes).expect("small rights payload"))
                as usize
        };
        // SAFETY: the aligned buffer contains one complete SCM_RIGHTS header and two descriptors.
        unsafe {
            let header = nix::libc::CMSG_FIRSTHDR(&message);
            assert!(!header.is_null());
            (*header).cmsg_level = nix::libc::SOL_SOCKET;
            (*header).cmsg_type = nix::libc::SCM_RIGHTS;
            (*header).cmsg_len =
                nix::libc::CMSG_LEN(u32::try_from(rights_bytes).expect("small rights payload"))
                    as usize;
            let data = nix::libc::CMSG_DATA(header).cast::<RawFd>();
            core::ptr::write_unaligned(data, first.as_raw_fd());
            core::ptr::write_unaligned(data.add(1), second.as_raw_fd());
            assert_eq!(
                nix::libc::sendmsg(
                    sender.as_raw_fd(),
                    &raw const message,
                    nix::libc::MSG_NOSIGNAL,
                ),
                NETWORK_WORKER_PLAN_FRAME_BYTES as isize
            );
        }
        let expected_peer = NetworkPeerCredentials {
            pid: std::process::id(),
            // SAFETY: process identity getters have no preconditions.
            uid: unsafe { nix::libc::getuid() },
            // SAFETY: process identity getters have no preconditions.
            gid: unsafe { nix::libc::getgid() },
        };
        let error = receive_network_plan_once(receiver.as_raw_fd(), expected_peer)
            .expect_err("fixed privileged plans must reject all SCM_RIGHTS descriptors");
        assert!(error.to_string().contains("must not carry descriptors"));
        let descriptor_count_after = fs::read_dir("/proc/self/fd")
            .expect("inspect descriptor table after rejection")
            .count();
        assert_eq!(descriptor_count_after, descriptor_count_before);
    }

    #[test]
    fn network_ipc_fixed_frame_and_phase_machine_reject_malformed_transitions() {
        let token = [0x5A; 32];
        let ready = NetworkIpcFrame::new(NetworkIpcKind::WorkerReady, token, 0, 0);
        let encoded = ready.encode();
        assert_eq!(
            NetworkIpcFrame::decode(&encoded, &token).expect("canonical fixed frame"),
            ready
        );
        assert!(NetworkIpcFrame::decode(&encoded[..63], &token).is_err());

        for index in [0_usize, 8, 10, 15, 16] {
            let mut malformed = encoded;
            malformed[index] ^= 1;
            assert!(
                NetworkIpcFrame::decode(&malformed, &token).is_err(),
                "byte {index} is authenticated protocol structure"
            );
        }
        let mut unknown_kind = encoded;
        unknown_kind[9] = 0xFF;
        assert!(NetworkIpcFrame::decode(&unknown_kind, &token).is_err());

        let tun = NetworkIpcFrame::new(NetworkIpcKind::TunReady, token, 1_280, 0);
        assert!(validate_supervisor_sent_frame(SupervisorIpcPhase::Ready, tun, 0).is_err());
        assert!(validate_supervisor_sent_frame(SupervisorIpcPhase::AwaitingReady, tun, 1).is_err());
        assert_eq!(
            validate_supervisor_sent_frame(SupervisorIpcPhase::Ready, tun, 1)
                .expect("one TUN descriptor in the exact phase"),
            SupervisorIpcPhase::AwaitingTunAck
        );
        let started = NetworkIpcFrame::new(NetworkIpcKind::Started, token, 0, 0);
        assert!(
            validate_supervisor_received_frame(SupervisorIpcPhase::AwaitingTunAck, started, 0,)
                .is_err()
        );
        assert_eq!(
            validate_supervisor_received_frame(SupervisorIpcPhase::AwaitingStarted, started, 0,)
                .expect("STARTED only completes the explicit start barrier"),
            SupervisorIpcPhase::Running
        );
    }

    #[test]
    fn traffic_accounting_coalesces_each_interval_and_force_flushes_latest() {
        let now = tokio::time::Instant::now();
        let mut state = State::default();
        let mut accounting = WorkerTrafficAccounting::new(0, 0, now).expect("accounting window");
        accounting
            .observe_at(&mut state, 10, 20, now, 1_000)
            .expect("first cumulative frame");
        accounting
            .observe_at(&mut state, 30, 40, now + Duration::from_millis(500), 1_500)
            .expect("coalesced cumulative frame");

        let mut persisted = Vec::new();
        assert!(
            !accounting
                .flush_if_due_with(&state, now + Duration::from_millis(999), |_| panic!(
                    "sub-interval traffic must not be persisted"
                ),)
                .expect("not yet due")
        );
        assert!(
            accounting
                .flush_if_due_with(
                    &state,
                    now + TRAFFIC_ACCOUNTING_PERSIST_INTERVAL,
                    |snapshot| {
                        persisted.push((snapshot.bytes_out, snapshot.bytes_in));
                        Ok(())
                    },
                )
                .expect("one batched persistence")
        );
        assert_eq!(persisted, [(30, 40)]);
        assert!(
            !accounting
                .force_flush_with(&state, |_| panic!("clean batch must not be rewritten"))
                .expect("already flushed")
        );

        accounting
            .observe_at(
                &mut state,
                50,
                60,
                now + Duration::from_millis(1_100),
                2_100,
            )
            .expect("next interval frame");
        assert!(
            accounting
                .force_flush_with(&state, |snapshot| {
                    persisted.push((snapshot.bytes_out, snapshot.bytes_in));
                    Ok(())
                })
                .expect("orderly exit flushes the partial batch")
        );
        assert_eq!(persisted, [(30, 40), (50, 60)]);
    }

    #[test]
    fn traffic_accounting_rejects_counter_rollback_and_frame_floods() {
        let now = tokio::time::Instant::now();
        let mut state = State::default();
        let mut accounting = WorkerTrafficAccounting::new(0, 0, now).expect("accounting window");
        accounting
            .observe_at(&mut state, 5, 7, now, 1_000)
            .expect("initial counters");
        let rollback = accounting
            .observe_at(&mut state, 4, 8, now, 1_000)
            .expect_err("cumulative counters never move backwards");
        assert!(rollback.to_string().contains("moved backwards"));
        assert_eq!((state.bytes_out, state.bytes_in), (5, 7));

        let mut flood_state = State::default();
        let mut flood = WorkerTrafficAccounting::new(0, 0, now).expect("flood window");
        for counter in 1..=u64::from(MAX_TRAFFIC_FRAMES_PER_INTERVAL) {
            flood
                .observe_at(&mut flood_state, counter, counter, now, 1_000)
                .expect("frames through the conservative ceiling are accepted");
        }
        let error = flood
            .observe_at(
                &mut flood_state,
                u64::from(MAX_TRAFFIC_FRAMES_PER_INTERVAL) + 1,
                u64::from(MAX_TRAFFIC_FRAMES_PER_INTERVAL) + 1,
                now,
                1_000,
            )
            .expect_err("one interval cannot drive unbounded root work");
        assert!(error.to_string().contains("TRAFFIC frame ceiling"));
        flood
            .observe_at(
                &mut flood_state,
                u64::from(MAX_TRAFFIC_FRAMES_PER_INTERVAL) + 1,
                u64::from(MAX_TRAFFIC_FRAMES_PER_INTERVAL) + 1,
                now + TRAFFIC_ACCOUNTING_PERSIST_INTERVAL,
                2_000,
            )
            .expect("the next monotonic interval starts a new bounded budget");
    }

    #[test]
    fn traffic_accounting_flushes_latest_counters_on_error_exit() {
        let now = tokio::time::Instant::now();
        let mut state = State::default();
        let mut accounting = WorkerTrafficAccounting::new(0, 0, now).expect("accounting window");
        accounting
            .observe_at(&mut state, 90, 120, now, 1_000)
            .expect("dirty cumulative counters");

        let first_flush = accounting.force_flush_with(&state, |_| {
            Err(ControllerError::State(
                "injected persistence failure".to_owned(),
            ))
        });
        assert!(first_flush.is_err());
        let mut persisted = None;
        let flush = accounting.force_flush_with(&state, |snapshot| {
            persisted = Some((snapshot.bytes_out, snapshot.bytes_in));
            Ok(())
        });
        let outcome = finish_worker_traffic_accounting(
            Err(ControllerError::State("injected IPC failure".to_owned())),
            flush,
        )
        .expect_err("the IPC failure remains fatal after the forced flush");
        assert!(outcome.to_string().contains("injected IPC failure"));
        assert_eq!(persisted, Some((90, 120)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn permanent_privilege_drop_uses_fail_closed_order_and_verifies_every_identity() {
        #[derive(Default)]
        struct FakeDropOps {
            calls: Vec<&'static str>,
            snapshot: Option<DroppedPrivilegeSnapshot>,
        }
        impl PrivilegeDropOps for FakeDropOps {
            fn set_no_new_privs(&mut self) -> io::Result<()> {
                self.calls.push("no-new-privs");
                Ok(())
            }
            fn disable_keep_capabilities(&mut self) -> io::Result<()> {
                self.calls.push("disable-keepcaps");
                Ok(())
            }
            fn clear_ambient_capabilities(&mut self) -> io::Result<()> {
                self.calls.push("clear-ambient");
                Ok(())
            }
            fn clear_bounding_capabilities(&mut self) -> io::Result<()> {
                self.calls.push("clear-bounding");
                Ok(())
            }
            fn clear_supplementary_groups(&mut self) -> io::Result<()> {
                self.calls.push("clear-groups");
                Ok(())
            }
            fn set_res_gid(&mut self, _gid: u32) -> io::Result<()> {
                self.calls.push("setresgid");
                Ok(())
            }
            fn set_res_uid(&mut self, _uid: u32) -> io::Result<()> {
                self.calls.push("setresuid");
                Ok(())
            }
            fn snapshot(&mut self) -> io::Result<DroppedPrivilegeSnapshot> {
                self.calls.push("verify");
                Ok(self.snapshot.clone().expect("configured snapshot"))
            }
        }
        let caller = PrivilegedCaller {
            uid: 1_000,
            gid: 1_001,
        };
        let good_snapshot = DroppedPrivilegeSnapshot {
            real_uid: caller.uid,
            effective_uid: caller.uid,
            saved_uid: caller.uid,
            real_gid: caller.gid,
            effective_gid: caller.gid,
            saved_gid: caller.gid,
            supplementary_groups: Vec::new(),
            no_new_privs: true,
            capabilities_clear: true,
        };
        let mut operations = FakeDropOps {
            snapshot: Some(good_snapshot.clone()),
            ..FakeDropOps::default()
        };
        permanent_privilege_drop_with(caller, &mut operations).expect("complete irreversible drop");
        assert_eq!(
            operations.calls,
            [
                "no-new-privs",
                "disable-keepcaps",
                "clear-ambient",
                "clear-bounding",
                "clear-groups",
                "setresgid",
                "setresuid",
                "verify",
            ]
        );

        let mut unsafe_snapshot = good_snapshot;
        unsafe_snapshot.saved_uid = 0;
        let mut operations = FakeDropOps {
            snapshot: Some(unsafe_snapshot),
            ..FakeDropOps::default()
        };
        assert!(permanent_privilege_drop_with(caller, &mut operations).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parent_death_signal_is_reinstalled_after_the_credential_drop() {
        use std::{cell::RefCell, rc::Rc};

        let calls = Rc::new(RefCell::new(Vec::new()));
        let install_calls = Rc::clone(&calls);
        let drop_calls = Rc::clone(&calls);
        let caller = PrivilegedCaller {
            uid: 1_000,
            gid: 1_001,
        };
        complete_network_worker_identity_isolation(
            caller,
            42,
            move |parent_pid| {
                assert_eq!(parent_pid, 42);
                install_calls.borrow_mut().push("pdeathsig");
                Ok(())
            },
            move |dropped_caller| {
                assert_eq!(dropped_caller, caller);
                drop_calls.borrow_mut().push("setresuid/setresgid");
                Ok(())
            },
        )
        .expect("complete parent-bound identity isolation");
        assert_eq!(
            *calls.borrow(),
            ["pdeathsig", "setresuid/setresgid", "pdeathsig"]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn network_worker_reads_fixed_bootstrap_only_after_permanent_containment() {
        use std::{cell::RefCell, rc::Rc};

        let calls = Rc::new(RefCell::new(Vec::new()));
        let install_calls = Rc::clone(&calls);
        let drop_calls = Rc::clone(&calls);
        let containment_calls = Rc::clone(&calls);
        let read_calls = Rc::clone(&calls);
        let caller = PrivilegedCaller {
            uid: 1_000,
            gid: 1_001,
        };
        let decoded = isolate_network_worker_before_decode(
            caller,
            42,
            move |_| {
                install_calls.borrow_mut().push("pdeathsig");
                Ok(())
            },
            move |_| {
                drop_calls.borrow_mut().push("permanent-drop");
                Ok(())
            },
            move || {
                containment_calls.borrow_mut().push("parser-containment");
                Ok(())
            },
            move || {
                read_calls.borrow_mut().push("read-fixed-bootstrap");
                Ok(7_u8)
            },
        )
        .expect("isolate before decoding launch");
        assert_eq!(decoded, 7);
        assert_eq!(
            *calls.borrow(),
            [
                "pdeathsig",
                "permanent-drop",
                "pdeathsig",
                "parser-containment",
                "read-fixed-bootstrap",
            ]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scm_credentials_do_not_replace_irreversible_process_posture_proof() {
        let caller = PrivilegedCaller {
            uid: 1_000,
            gid: 1_001,
        };
        let isolated = "Uid:\t1000\t1000\t1000\t1000\n\
                        Gid:\t1001\t1001\t1001\t1001\n\
                        Groups:\t\n\
                        CapInh:\t0000000000000000\n\
                        CapPrm:\t0000000000000000\n\
                        CapEff:\t0000000000000000\n\
                        CapBnd:\t0000000000000000\n\
                        CapAmb:\t0000000000000000\n\
                        NoNewPrivs:\t1\n\
                        Seccomp:\t2\n\
                        TracerPid:\t0\n\
                        Threads:\t1\n\
                        CoreDumping:\t0\n";
        verify_network_worker_isolation_status(isolated, caller)
            .expect("complete irreversible posture is accepted");

        let still_effective_root =
            isolated.replacen("Uid:\t1000\t1000\t1000\t1000", "Uid:\t1000\t0\t0\t0", 1);
        assert!(
            verify_network_worker_isolation_status(&still_effective_root, caller).is_err(),
            "matching real-UID SCM_CREDENTIALS cannot hide effective/saved/fs root"
        );
        for (needle, replacement) in [
            ("Threads:\t1", "Threads:\t2"),
            ("TracerPid:\t0", "TracerPid:\t42"),
            ("CapBnd:\t0000000000000000", "CapBnd:\t0000000000000001"),
            ("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
            ("Seccomp:\t2", "Seccomp:\t0"),
        ] {
            assert!(
                verify_network_worker_isolation_status(
                    &isolated.replacen(needle, replacement, 1),
                    caller,
                )
                .is_err(),
                "unsafe posture mutation {needle} must fail closed"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn privileged_child_command_is_pinned_to_the_running_proc_inode() {
        let command = pinned_controller_command().expect("construct pinned child command");
        assert_eq!(command.get_program(), OsStr::new(PINNED_SELF_EXEC_PATH));

        let pinned = fs::metadata(PINNED_SELF_EXEC_PATH).expect("inspect pinned executable");
        let current = fs::metadata(env::current_exe().expect("resolve test executable"))
            .expect("inspect current executable pathname");
        assert_eq!((pinned.dev(), pinned.ino()), (current.dev(), current.ino()));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn direct_controller_child_is_dead_and_reaped_before_cleanup_proof() {
        let started = Instant::now();
        let mut child = ProcessCommand::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("spawn direct child fixture");
        let identity = WorkerProcessIdentity {
            pid: child.id(),
            start_time_ticks: 0,
            executable_device: 0,
            executable_inode: 0,
            role: WorkerRole::Tunnel,
        };
        let proof = terminate_and_reap_controller_child(&mut child, &identity, Duration::ZERO)
            .expect("terminate and reap exact direct child");
        assert!(proof.warning.is_none());
        assert!(
            child
                .try_wait()
                .expect("inspect reaped direct child")
                .is_some(),
            "cleanup proof is returned only after the direct child has been reaped"
        );
        assert!(
            started.elapsed() < PROCESS_KILL_REAP_TIMEOUT + Duration::from_secs(1),
            "exact-child cleanup proof is bounded even when graceful identity signaling fails"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn received_tun_fd_rejects_an_unrelated_character_device() {
        use std::os::fd::OwnedFd;

        let mut options = fs::OpenOptions::new();
        options
            .read(true)
            .write(true)
            .custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NONBLOCK);
        let unrelated = options
            .open("/dev/null")
            .expect("open character device fixture");
        let error = match LinuxTunDevice::from_received_fd(
            OwnedFd::from(unrelated),
            "srvpn0000000000",
            VPN_DEFAULT_TUNNEL_MTU_BYTES,
        ) {
            Ok(_) => panic!("only the exact /dev/net/tun device is accepted"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("/dev/net/tun"));
    }

    #[derive(Default)]
    struct FakeNetworkCleanupOps {
        persisted: Option<State>,
        persist_count: usize,
        fail_persist_once_at: Option<usize>,
        resolv_reverts: usize,
        restored_routes: Vec<String>,
        fail_route_once: Option<String>,
    }
    impl NetworkCleanupOps for FakeNetworkCleanupOps {
        fn persist(&mut self, state: &State) -> Result<(), ControllerError> {
            self.persist_count += 1;
            if self.fail_persist_once_at == Some(self.persist_count) {
                self.fail_persist_once_at = None;
                return Err(ControllerError::State(
                    "injected persist failure".to_owned(),
                ));
            }
            self.persisted = Some(state.clone());
            Ok(())
        }

        fn revert_resolved(&mut self, _interface_name: &str) -> Result<(), ControllerError> {
            self.resolv_reverts += 1;
            Ok(())
        }

        fn restore_excluded_route(
            &mut self,
            snapshot: &ExcludedRouteSnapshot,
        ) -> Result<(), ControllerError> {
            self.restored_routes.push(snapshot.cidr.clone());
            if self.fail_route_once.as_deref() == Some(snapshot.cidr.as_str()) {
                self.fail_route_once = None;
                return Err(ControllerError::State("injected route failure".to_owned()));
            }
            Ok(())
        }
    }
    fn cleanup_test_state(dns_backend: DnsBackendState) -> State {
        State {
            active: true,
            repair_required: false,
            interface_name: Some("srvpn0000000000".to_owned()),
            network_service: Some("resolvectl".to_owned()),
            owner_uid: Some(1_000),
            session_id: Some("session-1".to_owned()),
            relay_endpoint: Some("/ip4/93.184.216.34/udp/7777/quic".to_owned()),
            relay_id: Some([0x22; 32]),
            network_policy_hash: Some([0x11; 32]),
            applied_network: Some(AppliedNetworkState {
                interface_name: "srvpn0000000000".to_owned(),
                journal_phase: NetworkJournalPhase::Prepared,
                dns_backend: Some(dns_backend),
                excluded_route_snapshots: vec![
                    ExcludedRouteSnapshot {
                        cidr: "192.0.2.0/24".to_owned(),
                        family: IpFamily::V4,
                        installed_route: Some(
                            "192.0.2.0/24 via 192.0.2.1 dev eth0 proto 186".to_owned(),
                        ),
                    },
                    ExcludedRouteSnapshot {
                        cidr: "198.51.100.0/24".to_owned(),
                        family: IpFamily::V4,
                        installed_route: Some(
                            "198.51.100.0/24 via 192.0.2.1 dev eth0 proto 186".to_owned(),
                        ),
                    },
                ],
            }),
            ..State::default()
        }
    }
    #[test]
    fn cleanup_journal_retries_after_dns_success_and_later_route_failure() {
        let mut state = cleanup_test_state(DnsBackendState::Resolved {
            interface_name: "srvpn0000000000".to_owned(),
        });
        let mut operations = FakeNetworkCleanupOps {
            fail_route_once: Some("198.51.100.0/24".to_owned()),
            ..FakeNetworkCleanupOps::default()
        };
        assert!(cleanup_persisted_network_with(&mut state, &mut operations).is_err());
        assert_eq!(operations.resolv_reverts, 1);
        assert_eq!(
            state
                .applied_network
                .as_ref()
                .expect("failed route remains journaled")
                .dns_backend,
            None
        );

        cleanup_persisted_network_with(&mut state, &mut operations)
            .expect("idempotent retry completes remaining routes");
        assert_eq!(operations.resolv_reverts, 1, "DNS was not replayed");
        assert_eq!(
            operations.restored_routes,
            ["198.51.100.0/24", "198.51.100.0/24", "192.0.2.0/24",]
        );
        assert!(state.applied_network.is_none());
        assert!(!state.repair_required);
    }

    #[test]
    fn cleanup_journal_recovers_from_every_persist_boundary() {
        for fail_at in 1..=7 {
            let mut state = cleanup_test_state(DnsBackendState::Resolved {
                interface_name: "srvpn0000000000".to_owned(),
            });
            let mut operations = FakeNetworkCleanupOps {
                fail_persist_once_at: Some(fail_at),
                ..FakeNetworkCleanupOps::default()
            };
            let _ = cleanup_persisted_network_with(&mut state, &mut operations);
            cleanup_persisted_network_with(&mut state, &mut operations)
                .unwrap_or_else(|error| panic!("retry after persist boundary {fail_at}: {error}"));
            assert!(state.applied_network.is_none(), "boundary {fail_at}");
            assert!(!state.repair_required, "boundary {fail_at}");
        }
    }

    #[test]
    fn failed_connect_after_reap_cleans_a_pre_readiness_mutation_journal() {
        let proof = ReapedControllerChild::observed();
        let mut state = cleanup_test_state(DnsBackendState::Resolved {
            interface_name: "srvpn0000000000".to_owned(),
        });
        let mut operations = FakeNetworkCleanupOps::default();

        finalize_failed_connect_state_with(
            &proof,
            &mut state,
            &mut operations,
            "supervisor crashed before readiness",
        )
        .expect("reaped supervisor permits complete progressive cleanup");

        assert!(state.applied_network.is_none());
        assert!(state.worker_identity.is_none());
        assert!(!state.repair_required);
        assert!(state.owner_uid.is_none());
        assert!(state.session_id.is_none());
        assert_eq!(state.message, "ready");
        assert_eq!(operations.resolv_reverts, 1);
        assert_eq!(operations.restored_routes.len(), 2);
    }

    #[test]
    fn failed_connect_after_reap_persists_repair_state_until_cleanup_retry() {
        let proof = ReapedControllerChild::observed();
        let mut state = cleanup_test_state(DnsBackendState::Resolved {
            interface_name: "srvpn0000000000".to_owned(),
        });
        let mut operations = FakeNetworkCleanupOps {
            fail_route_once: Some("198.51.100.0/24".to_owned()),
            ..FakeNetworkCleanupOps::default()
        };

        assert!(
            finalize_failed_connect_state_with(
                &proof,
                &mut state,
                &mut operations,
                "supervisor timed out after mutation",
            )
            .is_err()
        );
        assert!(state.repair_required);
        assert!(state.applied_network.is_some());
        assert_eq!(state.owner_uid, Some(1_000));
        assert!(state.worker_identity.is_none());

        finalize_failed_connect_state_with(
            &proof,
            &mut state,
            &mut operations,
            "supervisor timed out after mutation",
        )
        .expect("same durable journal completes on retry");
        assert!(state.applied_network.is_none());
        assert!(!state.repair_required);
        assert!(state.owner_uid.is_none());
    }

    #[derive(Default)]
    struct FakeNetworkPrepareOps {
        events: Vec<&'static str>,
        fail_at: Option<usize>,
        authorization_fails_after_event: Option<&'static str>,
        persisted_excluded_routes: Vec<Vec<String>>,
        persisted_installed_routes: Vec<Vec<Option<String>>>,
    }
    impl FakeNetworkPrepareOps {
        fn step(&mut self, event: &'static str) -> Result<(), ControllerError> {
            self.events.push(event);
            if self.fail_at == Some(self.events.len()) {
                return Err(ControllerError::State(format!(
                    "injected preparation failure at {event}"
                )));
            }
            Ok(())
        }
    }
    impl NetworkPrepareOps for FakeNetworkPrepareOps {
        type Device = String;
        type ExcludedRouteMutation = String;

        fn check_preparation(&mut self) -> Result<(), ControllerError> {
            if self
                .authorization_fails_after_event
                .is_some_and(|event| self.events.last().copied() == Some(event))
            {
                return Err(ControllerError::State(
                    "injected preparation authorization expiry".to_owned(),
                ));
            }
            Ok(())
        }

        fn persist(&mut self, state: &State) -> Result<(), ControllerError> {
            let applied = state
                .applied_network
                .as_ref()
                .expect("preparation persist always carries its repair journal");
            self.persisted_excluded_routes.push(
                applied
                    .excluded_route_snapshots
                    .iter()
                    .map(|snapshot| snapshot.cidr.clone())
                    .collect(),
            );
            self.persisted_installed_routes.push(
                applied
                    .excluded_route_snapshots
                    .iter()
                    .map(|snapshot| snapshot.installed_route.clone())
                    .collect(),
            );
            let event = match applied.journal_phase {
                NetworkJournalPhase::Planned => "persist-planned",
                NetworkJournalPhase::TunCreated => "persist-tun-created",
                NetworkJournalPhase::LinkConfigured => "persist-link-configured",
                NetworkJournalPhase::RoutesConfigured => "persist-routes-configured",
                NetworkJournalPhase::ConfiguringExcludedRoutes => "persist-installed-exclusion",
                NetworkJournalPhase::ExcludedRoutesConfigured => "persist-exclusions-configured",
                NetworkJournalPhase::DnsPlanned => "persist-dns-intent",
                NetworkJournalPhase::Prepared => "persist-prepared",
                NetworkJournalPhase::CleaningDns | NetworkJournalPhase::CleaningRoutes => {
                    panic!("cleanup phases are not preparation states")
                }
            };
            self.step(event)
        }

        fn create_tun(&mut self, requested_name: &str) -> Result<Self::Device, ControllerError> {
            self.step("create-tun")?;
            Ok(requested_name.to_owned())
        }

        fn tun_name<'a>(&self, device: &'a Self::Device) -> &'a str {
            device
        }

        fn apply_link(
            &mut self,
            _interface_name: &str,
            _mtu: u16,
            _tunnel_addresses: &[ParsedCidr],
        ) -> Result<(), ControllerError> {
            self.step("mutate-link")
        }

        fn apply_routes(
            &mut self,
            _interface_name: &str,
            _routes: &[String],
        ) -> Result<(), ControllerError> {
            self.step("mutate-routes")
        }

        fn plan_excluded_route(
            &mut self,
            route: &str,
        ) -> Result<(ExcludedRouteSnapshot, Self::ExcludedRouteMutation), ControllerError> {
            self.step("plan-excluded-route")?;
            Ok((
                ExcludedRouteSnapshot {
                    cidr: route.to_owned(),
                    family: IpFamily::V4,
                    installed_route: Some(format!(
                        "{PLANNED_EXCLUDED_ROUTE_PREFIX_V1}{route} via 192.0.2.1 dev eth0 proto {EXCLUDED_ROUTE_PROTOCOL_V1}"
                    )),
                },
                route.to_owned(),
            ))
        }

        fn apply_excluded_route(
            &mut self,
            snapshot: &ExcludedRouteSnapshot,
            _mutation: Self::ExcludedRouteMutation,
        ) -> Result<String, ControllerError> {
            self.step("mutate-excluded-route")?;
            Ok(format!(
                "{} via 192.0.2.1 dev eth0 proto 186",
                snapshot.cidr
            ))
        }

        fn plan_dns(
            &mut self,
            interface_name: &str,
            _dns_servers: &[String],
        ) -> Result<Option<DnsBackendState>, ControllerError> {
            Ok(Some(DnsBackendState::Resolved {
                interface_name: interface_name.to_owned(),
            }))
        }

        fn apply_dns(
            &mut self,
            interface_name: &str,
            _dns_servers: &[String],
            _plan: DnsBackendState,
        ) -> Result<DnsBackendState, ControllerError> {
            self.step("mutate-dns")?;
            Ok(DnsBackendState::Resolved {
                interface_name: interface_name.to_owned(),
            })
        }
    }
    fn privileged_test_plan(payload: &ConnectPayload) -> AuthenticatedPrivilegedNetworkPlan {
        AuthenticatedPrivilegedNetworkPlan {
            session_id: payload.session_id.clone(),
            relay_endpoint: payload.relay_endpoint.clone(),
            relay_id: parse_canonical_nonzero_hex_32(&payload.relay_id_hex, "relayIdHex")
                .expect("fixture relay id"),
            network_policy_hash: connect_payload_network_policy_hash(payload)
                .expect("fixture policy hash"),
            ticket_expires_at_ms: unix_now_ms().expect("clock") + 60_000,
            route_pushes: payload.route_pushes.clone(),
            excluded_routes: payload.excluded_routes.clone(),
            dns_servers: payload.dns_servers.clone(),
            tunnel_addresses: payload.tunnel_addresses.clone(),
            mtu_bytes: payload.mtu_bytes,
        }
    }
    fn preparation_test_state(payload: &AuthenticatedPrivilegedNetworkPlan) -> State {
        State {
            owner_uid: Some(1_000),
            session_id: Some(payload.session_id.clone()),
            relay_endpoint: Some(payload.relay_endpoint.clone()),
            relay_id: Some([0x22; 32]),
            network_policy_hash: Some([0x11; 32]),
            ticket_expires_at_ms: Some(payload.ticket_expires_at_ms),
            message: "starting isolated network worker".to_owned(),
            ..State::default()
        }
    }
    #[test]
    fn preparation_journal_fails_closed_at_every_persist_and_mutation_boundary() {
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.excluded_routes = vec!["192.0.2.0/24".to_owned()];
        let payload = privileged_test_plan(&payload);
        let mut baseline_state = preparation_test_state(&payload);
        let mut baseline_operations = FakeNetworkPrepareOps::default();
        let prepared = prepare_tunnel_with(&payload, &mut baseline_state, &mut baseline_operations)
            .expect("complete fake preparation");
        drop(prepared);
        let expected_events = baseline_operations.events;
        assert_eq!(
            expected_events,
            [
                "plan-excluded-route",
                "persist-planned",
                "create-tun",
                "persist-tun-created",
                "mutate-link",
                "persist-link-configured",
                "mutate-routes",
                "persist-routes-configured",
                "mutate-excluded-route",
                "persist-installed-exclusion",
                "persist-exclusions-configured",
                "persist-dns-intent",
                "mutate-dns",
                "persist-prepared",
            ]
        );

        for fail_at in 1..=expected_events.len() {
            let mut state = preparation_test_state(&payload);
            let mut operations = FakeNetworkPrepareOps {
                fail_at: Some(fail_at),
                ..FakeNetworkPrepareOps::default()
            };
            let error = match prepare_tunnel_with(&payload, &mut state, &mut operations) {
                Ok(_) => panic!("boundary {fail_at} unexpectedly prepared a tunnel"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("injected preparation failure"));
            assert_eq!(
                operations.events,
                expected_events[..fail_at],
                "no privileged step may run after injected boundary {fail_at}"
            );
            if fail_at <= 2 {
                assert!(state.applied_network.is_none());
                assert!(
                    !state.repair_required,
                    "a failure before the planned journal is durable precedes every host-network mutation"
                );
                continue;
            }
            assert!(
                state.repair_required,
                "boundary {fail_at} retains repair intent"
            );
            if expected_events[fail_at - 1] == "persist-installed-exclusion" {
                assert!(
                    state
                        .applied_network
                        .as_ref()
                        .expect("last durable journal remains present")
                        .excluded_route_snapshots
                        .iter()
                        .all(
                            |snapshot| snapshot.installed_route.as_deref().is_some_and(|proof| {
                                proof.starts_with(PLANNED_EXCLUDED_ROUTE_PREFIX_V1)
                            })
                        ),
                    "a failed installed-readback persist must retain the durable precommitted ownership tuple"
                );
                continue;
            }
            let mut cleanup = FakeNetworkCleanupOps::default();
            cleanup_persisted_network_with(&mut state, &mut cleanup).unwrap_or_else(|error| {
                panic!("durable plan at preparation boundary {fail_at} must clean: {error}")
            });
            assert!(state.applied_network.is_none(), "boundary {fail_at}");
            assert!(!state.repair_required, "boundary {fail_at}");
        }
    }
    #[test]
    fn preparation_authorization_is_rechecked_between_network_steps() {
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.excluded_routes = vec!["192.0.2.0/24".to_owned()];
        let payload = privileged_test_plan(&payload);
        let mut state = preparation_test_state(&payload);
        let mut operations = FakeNetworkPrepareOps {
            authorization_fails_after_event: Some("mutate-link"),
            ..FakeNetworkPrepareOps::default()
        };

        let error = match prepare_tunnel_with(&payload, &mut state, &mut operations) {
            Ok(_) => panic!("expired preparation authorization unexpectedly continued"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("authorization expiry"));
        assert_eq!(operations.events.last(), Some(&"mutate-link"));
        assert!(
            !operations.events.contains(&"persist-link-configured"),
            "no later journal or host-network step may run after authorization expiry"
        );
    }
    #[test]
    fn excluded_routes_are_proven_absent_and_journaled_before_any_network_mutation() {
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.excluded_routes = vec!["192.0.2.0/24".to_owned(), "198.51.100.7/32".to_owned()];
        let payload = privileged_test_plan(&payload);
        let mut state = preparation_test_state(&payload);
        let mut operations = FakeNetworkPrepareOps::default();

        let prepared = prepare_tunnel_with(&payload, &mut state, &mut operations)
            .expect("complete fake preparation");
        drop(prepared);

        assert_eq!(
            &operations.events[..3],
            [
                "plan-excluded-route",
                "plan-excluded-route",
                "persist-planned",
            ],
            "every route snapshot must precede the first durable journal and TUN mutation"
        );
        assert_eq!(
            operations.persisted_excluded_routes.first(),
            Some(&payload.excluded_routes),
            "the first durable repair plan must already contain the complete pre-VPN rollback set"
        );
        assert_eq!(operations.events[3], "create-tun");
        assert!(
            operations
                .events
                .iter()
                .position(|event| *event == "mutate-routes")
                .is_some_and(|index| index > 3),
            "pushed routes must not exist while exclusions are being snapshotted"
        );
        assert!(
            operations
                .persisted_installed_routes
                .iter()
                .any(|routes| { routes.iter().all(Option::is_some) }),
            "the exact installed readback for every exclusion must become durable before preparation completes"
        );
    }

    const TEST_SESSION_ID: &str = "f69c894aa32726fe586fab520f88ae42";
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
    fn test_relay_mldsa65_public_key_from_seed(
        seed: u8,
    ) -> [u8; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1] {
        let keys = KeyPair::try_from_seed(vec![seed; 32], Algorithm::MlDsa)
            .expect("derive ML-DSA-65 relay fixture key");
        let (algorithm, bytes) = keys
            .public_key()
            .try_to_bytes()
            .expect("ML-DSA-65 relay fixture key");
        assert_eq!(algorithm, Algorithm::MlDsa);
        bytes
            .try_into()
            .expect("ML-DSA-65 relay identity has the fixed V1 width")
    }
    fn test_relay_mldsa65_public_key() -> [u8; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1] {
        test_relay_mldsa65_public_key_from_seed(0x45)
    }
    fn test_ticket_issuer(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive helper-ticket issuer fixture key")
    }
    fn test_helper_ticket(session_id: &str) -> VpnHelperTicketV1 {
        let metering_keys = KeyPair::try_from_seed(vec![0x66; 32], Algorithm::Ed25519)
            .expect("derive metering fixture key");
        let now_ms = unix_now_ms().expect("valid test clock");
        let session_id = parse_canonical_session_id(session_id).expect("canonical test session id");
        let address_plan = derive_vpn_session_address_plan_v1(session_id);
        VpnHelperTicketV1 {
            session_id,
            quote_id: [0x22; 32],
            lease_id: [0x23; 32],
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
            client_ipv4_address: address_plan.client_ipv4_address,
            client_ipv6_address: address_plan.client_ipv6_address,
            network_policy_hash: vpn_helper_network_policy_hash_v1(
                "/ip4/93.184.216.34/udp/7777/quic",
                &test_relay_id(),
                &test_relay_mldsa65_public_key(),
                &[0xCD; 32],
                "relay.example",
                &[0xAB; 32],
                &[0xEF; 32],
                &[0x42; 32],
                15,
                &["0.0.0.0/0".to_owned()],
                &[],
                &["1.1.1.1".to_owned()],
                &address_plan.client_tunnel_addresses,
                1_280,
            ),
            valid_after_ms: now_ms.saturating_sub(1_000),
            expires_at_ms: now_ms.saturating_add(60_000),
        }
    }
    fn test_connect_payload_json(
        session_id: &str,
        ticket: &VpnHelperTicketV1,
        metering_private_key_seed_hex: Option<&str>,
    ) -> String {
        let default_metering_seed = "66".repeat(32);
        let metering_seed = metering_private_key_seed_hex.unwrap_or(&default_metering_seed);
        let client_ipv4 = Ipv4Addr::from(ticket.client_ipv4_address);
        let client_ipv6 = Ipv6Addr::from(ticket.client_ipv6_address);
        format!(
            r#"{{"sessionId":"{session_id}","relayEndpoint":"/ip4/93.184.216.34/udp/7777/quic","helperTicketHex":"{}","relayIdHex":"{}","relayMlDsa65PublicKeyHex":"{}","descriptorCommitHex":"{}","tlsServerName":"relay.example","relayTlsSpkiSha256Hex":"{}","relayCertificateSha256Hex":"{}","directorySnapshotDigestHex":"{}","paddingBudgetMs":15,"routePushes":["0.0.0.0/0"],"excludedRoutes":[],"dnsServers":["1.1.1.1"],"tunnelAddresses":["{client_ipv4}/30","{client_ipv6}/126"],"mtuBytes":1280,"meteringPrivateKeySeedHex":"{metering_seed}"}}"#,
            ticket.to_hex(test_ticket_issuer(0xAA).private_key()),
            hex::encode(ticket.relay_id),
            hex::encode(test_relay_mldsa65_public_key()),
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
    fn connect_payload_rejects_caller_selected_runtime_knobs() {
        let ticket = test_helper_ticket(TEST_SESSION_ID);
        let canonical = test_connect_payload_json(TEST_SESSION_ID, &ticket, None);
        for retired_field in [
            r#","exitClass":"standard"}"#,
            r#","leaseSecs":600}"#,
            r#","usageVoucherIntervalMs":18446744073709551615}"#,
        ] {
            let mutated = format!("{}{retired_field}", canonical.trim_end_matches('}'));
            let error = parse_connect_payload(Some(&mutated))
                .expect_err("caller-selected runtime knobs must be rejected");
            assert!(
                error.to_string().contains("unknown connect payload field"),
                "unexpected error for {retired_field}: {error}"
            );
        }
    }
    fn authenticated_test_connect_payload(session_id: &str) -> AuthenticatedConnectPayload {
        let payload = test_connect_payload(session_id);
        authenticate_connect_payload(
            payload,
            test_ticket_issuer(0xAA).public_key(),
            unix_now_ms().expect("valid test clock"),
        )
        .expect("authenticate connect payload fixture")
    }
    fn signed_plan_payload(mut payload: ConnectPayload) -> AuthenticatedConnectPayload {
        let mut ticket = test_helper_ticket(payload.session_id.as_str());
        ticket.network_policy_hash =
            connect_payload_network_policy_hash(&payload).expect("fixture policy hash");
        payload.helper_ticket_hex = ticket.to_hex(test_ticket_issuer(0xAA).private_key());
        AuthenticatedConnectPayload { payload, ticket }
    }
    #[test]
    fn fixed_privileged_plan_roundtrips_and_authenticates_ticket_policy() {
        let authenticated = authenticated_test_connect_payload(TEST_SESSION_ID);
        let token = [0xA5; 32];
        let frame = encode_authenticated_network_plan(&authenticated, token).expect("encode plan");
        let decoded = decode_authenticated_network_plan(
            &frame,
            &token,
            test_ticket_issuer(0xAA).public_key(),
            unix_now_ms().expect("clock"),
        )
        .expect("decode authenticated plan");
        assert_eq!(decoded.session_id, TEST_SESSION_ID);
        assert_eq!(decoded.relay_endpoint, authenticated.payload.relay_endpoint);
        assert_eq!(decoded.relay_id, authenticated.ticket.relay_id);
        assert_eq!(
            decoded.network_policy_hash,
            authenticated.ticket.network_policy_hash
        );
        assert_eq!(decoded.route_pushes, authenticated.payload.route_pushes);
        assert_eq!(
            decoded.excluded_routes,
            authenticated.payload.excluded_routes
        );
        assert_eq!(decoded.dns_servers, authenticated.payload.dns_servers);
        assert_eq!(
            decoded.tunnel_addresses,
            authenticated.payload.tunnel_addresses
        );
        assert_eq!(decoded.mtu_bytes, u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES));
    }
    #[test]
    fn fixed_privileged_plan_rejects_bad_token_signature_and_signed_policy() {
        let authenticated = authenticated_test_connect_payload(TEST_SESSION_ID);
        let token = [0xA5; 32];
        let issuer = test_ticket_issuer(0xAA);
        let now_ms = unix_now_ms().expect("clock");
        let canonical =
            encode_authenticated_network_plan(&authenticated, token).expect("encode plan");

        assert!(
            decode_authenticated_network_plan(&canonical, &[0x5A; 32], issuer.public_key(), now_ms)
                .is_err(),
            "the inherited token authenticates the fixed worker plan"
        );
        let mut bad_signature = canonical.clone();
        bad_signature[PLAN_TICKET_RANGE.end - 1] ^= 1;
        assert!(
            decode_authenticated_network_plan(&bad_signature, &token, issuer.public_key(), now_ms,)
                .is_err(),
            "root independently verifies the exact signed helper ticket"
        );
        let mut bad_policy = canonical.clone();
        encode_plan_cidr(
            &mut bad_policy[PLAN_ROUTE_RANGE.start..PLAN_ROUTE_RANGE.start + PLAN_ROUTE_SLOT_BYTES],
            ParsedCidr {
                address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)),
                prefix: 8,
            },
        );
        assert!(
            decode_authenticated_network_plan(&bad_policy, &token, issuer.public_key(), now_ms,)
                .is_err(),
            "root recomputes the signed policy hash instead of trusting the parser"
        );
        let mut substituted_relay_mldsa65 = canonical.clone();
        substituted_relay_mldsa65[PLAN_RELAY_MLDSA65_RANGE.start] ^= 1;
        assert!(
            decode_authenticated_network_plan(
                &substituted_relay_mldsa65,
                &token,
                issuer.public_key(),
                now_ms,
            )
            .is_err(),
            "the signed policy must bind the exact live ML-DSA-65 relay identity"
        );
    }
    #[test]
    fn fixed_privileged_plan_rejects_every_reserved_or_unused_region() {
        let authenticated = authenticated_test_connect_payload(TEST_SESSION_ID);
        let token = [0x6C; 32];
        let issuer = test_ticket_issuer(0xAA);
        let now_ms = unix_now_ms().expect("clock");
        let canonical =
            encode_authenticated_network_plan(&authenticated, token).expect("encode plan");
        let unused_route = PLAN_ROUTE_RANGE.start + PLAN_ROUTE_SLOT_BYTES;
        let unused_dns = PLAN_DNS_RANGE.start + PLAN_DNS_SLOT_BYTES;
        for (label, index) in [
            ("header reserved", 9_usize),
            ("count reserved", 53),
            (
                "relay string padding",
                PLAN_RELAY_RANGE.start + authenticated.payload.relay_endpoint.len(),
            ),
            (
                "TLS-name padding",
                PLAN_TLS_NAME_RANGE.start + authenticated.payload.tls_server_name.len(),
            ),
            ("layout gap", PLAN_RELAY_MLDSA65_RANGE.end),
            ("unused route slot", unused_route),
            ("unused excluded-route slot", PLAN_EXCLUDED_RANGE.start),
            ("unused DNS slot", unused_dns),
            ("tail", NETWORK_WORKER_PLAN_FRAME_BYTES - 1),
        ] {
            let mut mutated = canonical.clone();
            mutated[index] = 1;
            assert!(
                decode_authenticated_network_plan(&mutated, &token, issuer.public_key(), now_ms,)
                    .is_err(),
                "noncanonical {label} byte {index} must be rejected"
            );
        }
    }
    #[test]
    fn fixed_privileged_plan_rejects_bounds_duplicates_and_noncanonical_prefixes() {
        let token = [0x37; 32];
        let issuer = test_ticket_issuer(0xAA);
        let now_ms = unix_now_ms().expect("clock");
        let canonical = encode_authenticated_network_plan(
            &authenticated_test_connect_payload(TEST_SESSION_ID),
            token,
        )
        .expect("encode plan");
        for (label, mutate) in [
            (
                "route count",
                (
                    PLAN_ROUTE_COUNT_OFFSET,
                    (VPN_MAX_ROUTE_ENTRIES_V1 + 1) as u8,
                ),
            ),
            ("relay length", (PLAN_RELAY_LENGTH_OFFSET, 0xFF)),
        ] {
            let mut frame = canonical.clone();
            frame[mutate.0] = mutate.1;
            assert!(
                decode_authenticated_network_plan(&frame, &token, issuer.public_key(), now_ms)
                    .is_err(),
                "invalid {label} must be rejected"
            );
        }

        let mut duplicate_routes = test_connect_payload(TEST_SESSION_ID);
        duplicate_routes.route_pushes.push("0.0.0.0/0".to_owned());
        let duplicate_routes = signed_plan_payload(duplicate_routes);
        let frame = encode_authenticated_network_plan(&duplicate_routes, token).expect("encode");
        assert!(
            decode_authenticated_network_plan(&frame, &token, issuer.public_key(), now_ms).is_err()
        );

        let mut duplicate_dns = test_connect_payload(TEST_SESSION_ID);
        duplicate_dns.dns_servers.push("1.1.1.1".to_owned());
        let duplicate_dns = signed_plan_payload(duplicate_dns);
        let frame = encode_authenticated_network_plan(&duplicate_dns, token).expect("encode");
        assert!(
            decode_authenticated_network_plan(&frame, &token, issuer.public_key(), now_ms).is_err()
        );

        let mut host_bits = test_connect_payload(TEST_SESSION_ID);
        host_bits.route_pushes = vec!["10.1.2.3/24".to_owned()];
        let host_bits = signed_plan_payload(host_bits);
        let frame = encode_authenticated_network_plan(&host_bits, token).expect("encode");
        assert!(
            decode_authenticated_network_plan(&frame, &token, issuer.public_key(), now_ms).is_err()
        );
    }
    #[test]
    fn parse_multiaddr_accepts_ipv4_quic() {
        let parsed = parse_multiaddr("/ip4/93.184.216.34/udp/7777/quic").expect("parse");
        assert_eq!(
            parsed,
            ParsedMultiaddr {
                host: ParsedMultiaddrHost::Ip(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))),
                port: 7777,
            }
        );
    }
    #[test]
    fn parse_multiaddr_accepts_ipv6_quic() {
        let parsed = parse_multiaddr("/ip6/2606:4700:4700::1111/udp/7777/quic").expect("parse");
        assert_eq!(
            parsed,
            ParsedMultiaddr {
                host: ParsedMultiaddrHost::Ip(IpAddr::V6(
                    "2606:4700:4700::1111".parse().expect("public IPv6"),
                )),
                port: 7777,
            }
        );
    }
    #[test]
    fn parse_multiaddr_rejects_special_ip_literals() {
        for endpoint in [
            "/ip4/0.0.0.0/udp/7777/quic",
            "/ip4/10.0.0.1/udp/7777/quic",
            "/ip4/127.0.0.1/udp/7777/quic",
            "/ip4/169.254.1.1/udp/7777/quic",
            "/ip4/192.168.1.1/udp/7777/quic",
            "/ip4/224.0.0.1/udp/7777/quic",
            "/ip6/::/udp/7777/quic",
            "/ip6/::1/udp/7777/quic",
            "/ip6/fd00::1/udp/7777/quic",
            "/ip6/fe80::1/udp/7777/quic",
            "/ip6/ff02::1/udp/7777/quic",
        ] {
            assert!(parse_multiaddr(endpoint).is_err(), "accepted {endpoint}");
        }
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
    fn relay_dns_selection_rejects_mixed_or_special_answers_and_pins_one_public_address() {
        let first: SocketAddr = "93.184.216.35:9443".parse().expect("first answer");
        let second: SocketAddr = "93.184.216.34:9443".parse().expect("second answer");
        let selected =
            select_resolved_relay_addr("relay.example", DnsAddressFamily::V4, 443, [first, second])
                .expect("public answer set");
        assert_eq!(
            selected,
            "93.184.216.34:443".parse().expect("pinned address")
        );

        for unsafe_answer in ["127.0.0.1:9443", "10.0.0.1:9443", "[::1]:9443"] {
            let unsafe_answer = unsafe_answer.parse().expect("unsafe answer fixture");
            let error = select_resolved_relay_addr(
                "relay.example",
                DnsAddressFamily::Any,
                443,
                [second, unsafe_answer],
            )
            .expect_err("one unsafe answer must reject the entire DNS response");
            assert!(error.to_string().contains("private, local, reserved"));
        }

        let wrong_family =
            select_resolved_relay_addr("relay.example", DnsAddressFamily::V6, 443, [second])
                .expect_err("signed DNS family must be enforced");
        assert!(wrong_family.to_string().contains("signed address family"));
    }
    #[test]
    fn relay_dns_selection_caps_answer_cardinality() {
        let answers = (0..=MAX_RELAY_DNS_ANSWERS_V1)
            .map(|offset| {
                let offset = u16::try_from(offset).expect("DNS fixture offset fits u16");
                SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
                    443_u16
                        .checked_add(offset)
                        .expect("DNS fixture port fits u16"),
                )
            })
            .collect::<Vec<_>>();
        let error =
            select_resolved_relay_addr("relay.example", DnsAddressFamily::Any, 443, answers)
                .expect_err("oversized DNS answer set must fail");
        assert!(error.to_string().contains("more than"));
    }
    #[test]
    fn parse_multiaddr_rejects_non_udp_transport() {
        let err = parse_multiaddr("/ip4/93.184.216.34/tcp/7777/quic").expect_err("must fail");
        assert!(err.to_string().contains("transport"));
    }
    #[test]
    fn connect_payload_deserializes_camel_case() {
        let payload = test_connect_payload(TEST_SESSION_ID);
        assert_eq!(TEST_SESSION_ID, payload.session_id);
        assert_eq!("/ip4/93.184.216.34/udp/7777/quic", payload.relay_endpoint);
        assert_eq!(1280, payload.mtu_bytes);
        assert_eq!(15, payload.padding_budget_ms);
    }
    #[test]
    fn connect_payload_requires_the_metering_private_key() {
        let ticket = test_helper_ticket(TEST_SESSION_ID);
        let canonical = test_connect_payload_json(TEST_SESSION_ID, &ticket, None);
        let seed_field = format!(r#","meteringPrivateKeySeedHex":"{}""#, "66".repeat(32));
        let without_seed = canonical.replace(&seed_field, "");
        let error = parse_connect_payload(Some(&without_seed))
            .expect_err("a tunnel without a voucher signing key must fail closed");
        assert!(error.to_string().contains("meteringPrivateKeySeedHex"));
    }
    #[test]
    fn state_frame_roundtrips_as_norito() {
        let state = State {
            owner_uid: Some(1_000),
            session_id: Some("session-1".to_owned()),
            relay_endpoint: Some("/ip4/93.184.216.34/udp/7777/quic".to_owned()),
            relay_id: Some([0x22; 32]),
            network_policy_hash: Some([0x33; 32]),
            ticket_expires_at_ms: Some(42_000),
            bytes_in: 7,
            bytes_out: 9,
            ..State::default()
        };
        let frame = encode_state_frame(&state).expect("encode state");
        assert!(frame.starts_with(STATE_FILE_FRAME_MAGIC));
        assert_eq!(decode_state_frame(&frame).expect("decode state"), state);
    }
    #[test]
    fn legacy_state_frame_preserves_recovery_but_cannot_report_active() {
        let state = active_runtime_test_state();
        let legacy = StateV1::from(&state);
        let mut frame = Vec::with_capacity(STATE_FILE_FRAME_MAGIC_V1.len() + legacy.encoded_len());
        frame.extend_from_slice(STATE_FILE_FRAME_MAGIC_V1);
        legacy.encode_to(&mut frame);

        let mut decoded = decode_state_frame(&frame).expect("decode legacy recovery state");
        assert!(decoded.active);
        assert!(decoded.ticket_expires_at_ms.is_none());
        scrub_stale_process_with(&mut decoded, 1_000, |_| Ok(true))
            .expect("normalize legacy active state without losing process custody");
        assert!(!decoded.active);
        assert!(decoded.repair_required);
        assert!(decoded.worker_identity.is_some());
        assert!(decoded.network_worker_identity.is_some());
        assert!(decoded.applied_network.is_some());
        validate_state_for_persistence_at(&decoded, 1_000)
            .expect("legacy state remains persistable as repair custody");
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
    fn command_cgroup_events_parser_requires_one_exact_populated_field() {
        assert!(!parse_system_command_cgroup_populated("populated 0\nfrozen 0\n").unwrap());
        assert!(parse_system_command_cgroup_populated("frozen 0\npopulated 1\n").unwrap());
        for malformed in [
            "",
            "frozen 0\n",
            "populated\n",
            "populated 2\n",
            "populated 0 extra\n",
            "populated 0\npopulated 1\n",
        ] {
            assert!(
                matches!(
                    parse_system_command_cgroup_populated(malformed),
                    Err(ControllerError::CommandCustody(_))
                ),
                "malformed cgroup.events must fail closed: {malformed:?}"
            );
        }
    }
    #[test]
    fn command_cgroup_control_custody_rejects_delegated_or_writable_controls() {
        assert!(system_command_cgroup_control_has_custody(
            0, 0, 0o100644, 0o400
        ));
        assert!(system_command_cgroup_control_has_custody(
            0, 0, 0o100200, 0o200
        ));
        for (uid, gid, mode) in [
            (1_000, 0, 0o100644),
            (0, 1_000, 0o100644),
            (0, 0, 0o100664),
            (0, 0, 0o100646),
            (0, 0, 0o100044),
        ] {
            assert!(
                !system_command_cgroup_control_has_custody(uid, gid, mode, 0o400),
                "delegated or insufficient cgroup control custody must fail closed"
            );
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn command_leader_observation_does_not_reap_or_release_its_process_group() {
        let executable = Path::new("/bin/true");
        if !executable.exists() {
            return;
        }
        let mut command = ProcessCommand::new(executable);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.process_group(0);
        let mut child = command.spawn().expect("spawn direct command child");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if system_command_leader_exited_unreaped(child.id()).expect("observe direct child") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "direct command child did not exit"
            );
            sleep_blocking(SYSTEM_COMMAND_POLL_INTERVAL);
        }
        assert!(
            system_command_leader_exited_unreaped(child.id()).expect("observe zombie again"),
            "WNOWAIT must leave the leader waitable and its PID/PGID reserved"
        );
        kill_command_process_group(child.id()).expect("kill pinned process group");
        assert!(
            child
                .try_wait()
                .expect("reap exact direct child")
                .expect("observed child remains waitable")
                .success()
        );
    }
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn privileged_system_commands_fail_closed_outside_linux() {
        let error = execute_system_command(
            "ip",
            Path::new("/sbin/ip"),
            &["route".to_owned()],
            SYSTEM_COMMAND_TIMEOUT,
        )
        .expect_err("non-Linux system command execution must be unreachable");
        assert!(error.to_string().contains("supported only on Linux"));
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires root and a writable cgroup-v2 mount"]
    fn command_cgroup_kills_a_descendant_that_detaches_with_setsid() {
        assert_eq!(
            effective_uid(),
            0,
            "this custody integration test requires root"
        );
        let shell = Path::new("/bin/sh");
        let setsid = [Path::new("/usr/bin/setsid"), Path::new("/bin/setsid")]
            .into_iter()
            .find(|candidate| candidate.exists())
            .expect("setsid is required for the detached-descendant regression");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("wall clock after Unix epoch")
            .as_nanos();
        let test_path = PathBuf::from(format!(
            "/sys/fs/cgroup/sora-vpn-controller-test-{}-{nonce}",
            std::process::id()
        ));
        let test_path = ensure_system_command_cgroup_at(&test_path)
            .expect("create isolated command-custody test cgroup");
        let script = format!("{} /bin/sleep 30 & /bin/sleep 0.1", setsid.display());
        let started = Instant::now();
        let result = execute_system_command_in_cgroup(
            "sh",
            shell,
            &["-c".to_owned(), script],
            Duration::from_secs(2),
            &test_path,
        );
        let cleanup = quiesce_system_command_cgroup_at_until(
            &test_path,
            Instant::now() + PROCESS_KILL_REAP_TIMEOUT,
        );
        let removal = fs::remove_dir(&test_path);

        cleanup.expect("detached command cgroup must be proven empty");
        removal.expect("remove isolated empty test cgroup");
        result.expect("successful leader with detached descendant must retain exact exit status");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "setsid moved the descendant out of its PGID, but it must remain in recursive cgroup custody"
        );
    }
    #[tokio::test]
    async fn tunnel_shutdown_handlers_install_before_network_setup() {
        let signals = TunnelShutdownSignals::install().expect("install Unix signal handlers");
        drop(signals);
    }
    #[test]
    fn connect_payload_rejects_network_policy_count_before_encoding() {
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.route_pushes = vec!["10.0.0.0/8".to_owned(); MAX_NETWORK_POLICY_ENTRIES_V1 + 1];
        let error = validate_connect_payload(payload)
            .expect_err("validator must enforce the route-count limit before any IPC encoding");
        let message = error.to_string();
        assert!(message.contains("routePushes"));
        assert!(message.contains("exceeds the v1 limit"));
        assert!(message.contains(&MAX_NETWORK_POLICY_ENTRIES_V1.to_string()));
    }
    #[test]
    fn connect_payload_enforces_exact_v1_policy_cardinalities() {
        let canonical = test_connect_payload(TEST_SESSION_ID);

        let mut empty_routes = canonical.clone();
        empty_routes.route_pushes.clear();
        assert!(
            validate_connect_payload(empty_routes)
                .expect_err("V1 needs at least one pushed route")
                .to_string()
                .contains("between 1 and 64")
        );
        let mut empty_dns = canonical.clone();
        empty_dns.dns_servers.clear();
        assert!(
            validate_connect_payload(empty_dns)
                .expect_err("V1 needs at least one resolver")
                .to_string()
                .contains("between 1 and 8")
        );

        let routes = (0..VPN_MAX_ROUTE_ENTRIES_V1)
            .map(|index| format!("10.{index}.0.0/16"))
            .collect::<Vec<_>>();
        let mut at_route_limit = canonical.clone();
        at_route_limit.route_pushes = routes.clone();
        validate_connect_payload_ref(&at_route_limit).expect("64 canonical routes are accepted");
        let mut over_route_limit = at_route_limit;
        over_route_limit
            .route_pushes
            .push("172.16.0.0/12".to_owned());
        assert!(
            validate_connect_payload(over_route_limit)
                .expect_err("65 routes exceed V1")
                .to_string()
                .contains("between 1 and 64")
        );

        let mut at_exclusion_limit = canonical.clone();
        at_exclusion_limit.excluded_routes = (0..VPN_MAX_ROUTE_ENTRIES_V1)
            .map(|index| format!("192.168.{index}.0/24"))
            .collect();
        validate_connect_payload_ref(&at_exclusion_limit)
            .expect("64 canonical exclusions are accepted");
        at_exclusion_limit
            .excluded_routes
            .push("198.18.0.0/15".to_owned());
        assert!(
            validate_connect_payload(at_exclusion_limit)
                .expect_err("65 exclusions exceed V1")
                .to_string()
                .contains("between 0 and 64")
        );

        let mut at_dns_limit = canonical;
        at_dns_limit.dns_servers = (1..=VPN_MAX_DNS_ENTRIES_V1)
            .map(|last| format!("1.1.1.{last}"))
            .collect();
        validate_connect_payload_ref(&at_dns_limit).expect("eight resolvers are accepted");
        at_dns_limit.dns_servers.push("8.8.8.8".to_owned());
        assert!(
            validate_connect_payload(at_dns_limit)
                .expect_err("nine resolvers exceed V1")
                .to_string()
                .contains("between 1 and 8")
        );
    }
    #[test]
    fn decode_hex_accepts_prefixed_values() {
        let decoded = decode_hex("0x0A0b").expect("hex");
        assert_eq!(decoded, vec![0x0A, 0x0B]);
    }
    #[test]
    fn helper_ticket_handshake_binding_is_nonzero_and_credential_bound() {
        let payload = test_connect_payload(TEST_SESSION_ID);
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
    fn tls_spki_and_post_tls_bundle_binding_are_independent() {
        let certificate_der =
            include_bytes!("../../../certs/google_attestation_root_ecdsa.der").as_slice();
        let spki_digest =
            leaf_certificate_spki_sha256(certificate_der).expect("fixture certificate SPKI");
        verify_relay_tls_spki_pin(certificate_der, &spki_digest)
            .expect("the live leaf certificate matches the signed SPKI pin");

        let mut wrong_spki = spki_digest;
        wrong_spki[0] ^= 1;
        let spki_error = verify_relay_tls_spki_pin(certificate_der, &wrong_spki)
            .expect_err("a different SPKI must fail live TLS authentication");
        assert!(spki_error.to_string().contains("SPKI pin"));

        // `relayCertificateSha256Hex` is the digest of the canonical signed
        // certificate bundle, not the DER digest of this live TLS leaf. TLS
        // therefore succeeds regardless of that unrelated bundle digest, while
        // the authenticated post-TLS handshake transcript remains bound to it.
        let payload = test_connect_payload(TEST_SESSION_ID);
        let binding = helper_ticket_handshake_binding(&payload, b"helper ticket")
            .expect("original helper handshake binding");
        let mut different_bundle = payload;
        different_bundle.relay_certificate_sha256_hex = "ee".repeat(32);
        assert_ne!(
            binding,
            helper_ticket_handshake_binding(&different_bundle, b"helper ticket")
                .expect("different bundle helper handshake binding")
        );
    }
    #[test]
    fn connect_payload_rejects_noncanonical_trust_hex() {
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.relay_tls_spki_sha256_hex.make_ascii_uppercase();
        let error = validate_connect_payload(payload).expect_err("uppercase pin must fail");
        assert!(
            error
                .to_string()
                .contains("exactly 64 lowercase hexadecimal characters")
        );

        let mut uppercase_mldsa = test_connect_payload(TEST_SESSION_ID);
        uppercase_mldsa
            .relay_mldsa65_public_key_hex
            .make_ascii_uppercase();
        assert!(
            validate_connect_payload(uppercase_mldsa)
                .expect_err("uppercase ML-DSA-65 key must fail")
                .to_string()
                .contains("lowercase hexadecimal characters")
        );
        let mut zero_mldsa = test_connect_payload(TEST_SESSION_ID);
        zero_mldsa.relay_mldsa65_public_key_hex =
            "00".repeat(VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1);
        assert!(
            validate_connect_payload(zero_mldsa)
                .expect_err("all-zero ML-DSA-65 key must fail")
                .to_string()
                .contains("must not be all zero")
        );
        let mut short_mldsa = test_connect_payload(TEST_SESSION_ID);
        short_mldsa.relay_mldsa65_public_key_hex.pop();
        assert!(
            validate_connect_payload(short_mldsa)
                .expect_err("short ML-DSA-65 key must fail")
                .to_string()
                .contains("lowercase hexadecimal characters")
        );
    }
    #[test]
    fn connect_payload_rejects_unknown_aliases_and_duplicate_fields() {
        let ticket = test_helper_ticket(TEST_SESSION_ID);
        let canonical = test_connect_payload_json(TEST_SESSION_ID, &ticket, None);
        let alias = canonical.replacen(
            r#""sessionId":"#,
            r#""session_id":"shadow","sessionId":"#,
            1,
        );
        let error = parse_connect_payload(Some(&alias)).expect_err("retired alias must fail");
        assert!(error.to_string().contains("unknown connect payload field"));

        let duplicate =
            canonical.replacen(r#""sessionId":"#, r#""sessionId":"shadow","sessionId":"#, 1);
        let error = parse_connect_payload(Some(&duplicate)).expect_err("duplicate key must fail");
        assert!(error.to_string().contains("duplicate field"));
    }
    #[test]
    fn connect_payload_rejects_dns_directive_injection() {
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.dns_servers = vec!["1.1.1.1\noptions trust-ad".to_owned()];
        let error = validate_connect_payload(payload).expect_err("DNS directives must fail");
        assert!(error.to_string().contains("canonical IP address"));
    }
    #[test]
    fn connect_payload_rejects_duplicate_and_non_unicast_dns_servers() {
        let mut duplicate = test_connect_payload(TEST_SESSION_ID);
        duplicate.dns_servers.push("1.1.1.1".to_owned());
        assert!(
            validate_connect_payload(duplicate)
                .expect_err("duplicate resolver")
                .to_string()
                .contains("duplicate")
        );

        for resolver in [
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "::",
            "ff02::1",
            "::ffff:0.0.0.0",
            "::ffff:255.255.255.255",
            "::ffff:224.0.0.1",
        ] {
            let mut payload = test_connect_payload(TEST_SESSION_ID);
            payload.dns_servers = vec![resolver.to_owned()];
            assert!(
                validate_connect_payload(payload).is_err(),
                "{resolver} is not a canonical unicast resolver"
            );
        }

        let mut mapped_duplicate = test_connect_payload(TEST_SESSION_ID);
        mapped_duplicate.dns_servers = vec!["1.1.1.1".to_owned(), "::ffff:1.1.1.1".to_owned()];
        assert!(
            validate_connect_payload(mapped_duplicate)
                .expect_err("mapped and native IPv4 resolvers are semantic duplicates")
                .to_string()
                .contains("duplicate")
        );
    }
    #[test]
    fn connect_payload_requires_canonical_network_policy() {
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.route_pushes = vec!["2001:0db8::/64".to_owned()];
        let error = validate_connect_payload(payload).expect_err("non-canonical CIDR must fail");
        assert!(error.to_string().contains("canonical CIDR syntax"));

        let mut host_bits = test_connect_payload(TEST_SESSION_ID);
        host_bits.route_pushes = vec!["10.1.2.3/24".to_owned()];
        let error = validate_connect_payload(host_bits)
            .expect_err("route network prefixes must clear host bits");
        assert!(error.to_string().contains("10.1.2.0/24"));

        let mut duplicate = test_connect_payload(TEST_SESSION_ID);
        duplicate.route_pushes = vec!["0.0.0.0/0".to_owned(); 2];
        assert!(
            validate_connect_payload(duplicate)
                .expect_err("semantic route duplicate")
                .to_string()
                .contains("duplicate")
        );

        let mut exact_conflict = test_connect_payload(TEST_SESSION_ID);
        exact_conflict.excluded_routes = vec!["0.0.0.0/0".to_owned()];
        assert!(
            validate_connect_payload(exact_conflict)
                .expect_err("exact include/exclude conflict")
                .to_string()
                .contains("same canonical network prefix")
        );

        let mut permitted_subnet = test_connect_payload(TEST_SESSION_ID);
        permitted_subnet.excluded_routes = vec!["192.0.2.0/24".to_owned()];
        validate_connect_payload_ref(&permitted_subnet)
            .expect("a more-specific exclusion below a pushed default is intentional");
    }
    #[test]
    fn connect_payload_requires_ticket_bound_privileged_policy() {
        let payload = test_connect_payload(TEST_SESSION_ID);
        let mut variants = Vec::new();
        let mut changed = payload.clone();
        changed.relay_endpoint = "/ip4/93.184.216.35/udp/7777/quic".to_owned();
        variants.push(("relay endpoint", changed));
        let mut changed = payload.clone();
        changed.descriptor_commit_hex = "ce".repeat(32);
        variants.push(("descriptor commitment", changed));
        let mut changed = payload.clone();
        changed.relay_mldsa65_public_key_hex =
            hex::encode(test_relay_mldsa65_public_key_from_seed(0x46));
        variants.push(("ML-DSA-65 relay identity", changed));
        let mut changed = payload.clone();
        changed.tls_server_name = "other.example".to_owned();
        variants.push(("TLS server name", changed));
        let mut changed = payload.clone();
        changed.relay_tls_spki_sha256_hex = "ac".repeat(32);
        variants.push(("TLS SPKI pin", changed));
        let mut changed = payload.clone();
        changed.relay_certificate_sha256_hex = "ee".repeat(32);
        variants.push(("relay certificate digest", changed));
        let mut changed = payload.clone();
        changed.directory_snapshot_digest_hex = "43".repeat(32);
        variants.push(("directory snapshot digest", changed));
        let mut changed = payload.clone();
        changed.padding_budget_ms = 16;
        variants.push(("padding budget", changed));
        let mut changed = payload.clone();
        changed.route_pushes.push("10.0.0.0/8".to_owned());
        variants.push(("route push", changed));
        let mut changed = payload.clone();
        changed.excluded_routes.push("192.0.2.0/24".to_owned());
        variants.push(("excluded route", changed));
        let mut changed = payload.clone();
        changed.dns_servers.push("8.8.8.8".to_owned());
        variants.push(("DNS server", changed));

        for (label, changed) in variants {
            let error = authenticate_connect_payload(
                changed,
                test_ticket_issuer(0xAA).public_key(),
                unix_now_ms().expect("valid test clock"),
            )
            .expect_err("ticket must authorize the exact privileged policy");
            assert!(
                error.to_string().contains("policy hash"),
                "unexpected error for {label}: {error}"
            );
        }

        let mut changed = payload.clone();
        changed.tunnel_addresses = vec!["10.208.0.3/32".to_owned()];
        let error = validate_connect_payload(changed)
            .expect_err("a non-derived tunnel-address plan must fail before authentication");
        assert!(error.to_string().contains("derived from sessionId"));

        let mut changed = payload;
        changed.mtu_bytes = 1_400;
        let error = validate_connect_payload(changed)
            .expect_err("a non-V1 MTU must fail before authentication");
        assert!(error.to_string().contains("exactly 1280"));
    }
    #[test]
    fn connect_payload_credentials_can_be_wiped_early() {
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.wipe_credentials();
        assert!(payload.helper_ticket_hex.is_empty());
        assert!(payload.metering_private_key_seed_hex.is_empty());
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
    fn vpn_quic_requires_exact_negotiated_alpn() {
        validate_soranet_quic_alpn(Some(SORANET_QUIC_ALPN))
            .expect("the canonical SoraNet ALPN must be accepted");
        for protocol in [None, Some(b"".as_slice()), Some(b"h3".as_slice())] {
            let error = validate_soranet_quic_alpn(protocol)
                .expect_err("missing or mismatched ALPN must fail closed");
            assert!(error.to_string().contains("exact SoraNet QUIC ALPN"));
        }
    }
    #[test]
    fn helper_ticket_requires_the_pinned_issuer() {
        let ticket = test_helper_ticket(TEST_SESSION_ID);
        let trusted_issuer = test_ticket_issuer(0xAA);
        let attacker_issuer = test_ticket_issuer(0xBB);
        let authenticated = parse_authenticated_helper_ticket(
            &ticket.to_hex(trusted_issuer.private_key()),
            trusted_issuer.public_key(),
            unix_now_ms().expect("valid test clock"),
        )
        .expect("trusted issuer ticket");
        assert_eq!(authenticated, ticket);

        let forged = ticket.to_hex(attacker_issuer.private_key());
        let error = parse_authenticated_helper_ticket(
            &forged,
            trusted_issuer.public_key(),
            unix_now_ms().expect("valid test clock"),
        )
        .expect_err("self-consistent attacker ticket must not authorize root networking");
        assert!(error.to_string().contains("issuer authentication"));
    }
    #[test]
    fn connect_authentication_rejects_a_fully_self_consistent_forged_ticket() {
        let attacker_issuer = test_ticket_issuer(0xBB);
        let mut forged_ticket = test_helper_ticket(TEST_SESSION_ID);
        let mut payload = test_connect_payload(TEST_SESSION_ID);
        payload.route_pushes = vec!["10.0.0.0/8".to_owned()];
        forged_ticket.network_policy_hash = connect_payload_network_policy_hash(&payload)
            .expect("attacker payload is structurally valid");
        payload.helper_ticket_hex = forged_ticket.to_hex(attacker_issuer.private_key());

        let error = authenticate_connect_payload(
            payload,
            test_ticket_issuer(0xAA).public_key(),
            unix_now_ms().expect("valid test clock"),
        )
        .expect_err("attacker-signed policy must fail before privileged mutation");
        assert!(error.to_string().contains("issuer authentication"));
    }
    #[test]
    fn usage_voucher_signer_builds_signed_cumulative_voucher() {
        let session_id = TEST_SESSION_ID;
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
        let mut signer =
            UsageVoucherSigner::from_payload(&payload, ticket.clone()).expect("signer");
        assert_eq!(signer.interval, USAGE_VOUCHER_INTERVAL);
        signer.started_at = Instant::now()
            .checked_sub(Duration::from_secs(30))
            .expect("test instant supports a short history");
        signer.begin_service();
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
        assert_eq!(
            envelope.voucher.body.ingress_bytes,
            10 + USAGE_VOUCHER_BYTE_CREDIT_WINDOW
        );
        assert_eq!(
            envelope.voucher.body.egress_bytes,
            20 + USAGE_VOUCHER_BYTE_CREDIT_WINDOW
        );
        assert!(envelope.voucher.body.active_ms >= USAGE_VOUCHER_ACTIVE_CREDIT_MS);
        assert!(
            envelope.voucher.body.active_ms
                < USAGE_VOUCHER_ACTIVE_CREDIT_MS + USAGE_VOUCHER_INTERVAL.as_millis() as u64
        );
        assert_eq!(
            envelope.fee_ceiling,
            ticket
                .tariff
                .fee_ceiling(&envelope.voucher.body)
                .expect("bounded fixture fee")
        );
    }
    #[tokio::test]
    async fn usage_voucher_control_cell_flushes_single_protected_record() {
        let session_id = TEST_SESSION_ID;
        let ticket = test_helper_ticket(session_id);
        let metering_seed = "66".repeat(32);
        let raw_payload =
            test_connect_payload_json(session_id, &ticket, Some(metering_seed.as_str()));
        let payload = parse_connect_payload(Some(&raw_payload)).expect("payload");
        let circuit_id = ticket.session_id;
        let mut signer = UsageVoucherSigner::from_payload(&payload, ticket).expect("signer");
        let counters = UsageVoucherCounters::default();
        let context =
            RecordStreamContext::new(RecordEndpoint::Client, RecordStreamKind::Bidirectional, 7);
        let client_record = RecordLayer::new(
            iroha_crypto::SessionKey::new(vec![0xA5; 32]),
            RecordEndpoint::Client,
        )
        .expect("client record layer")
        .stream(context)
        .expect("client record stream");
        let relay_record = RecordLayer::new(
            iroha_crypto::SessionKey::new(vec![0xA5; 32]),
            RecordEndpoint::Relay,
        )
        .expect("relay record layer")
        .stream(context)
        .expect("relay record stream");
        let (transport_writer, transport_reader) = tokio::io::duplex(4_096);
        let mut writer =
            soranet_record_io::RecordWriter::new(transport_writer, client_record.sealer);
        let mut reader =
            soranet_record_io::RecordReader::new(transport_reader, relay_record.opener);
        let mut sequence = 0;

        send_usage_voucher_control_cell(
            &mut writer,
            circuit_id,
            vpn_flow_label_from_session_id(circuit_id).expect("flow label"),
            payload.padding_budget_ms,
            &counters,
            &mut signer,
            &mut sequence,
        )
        .await
        .expect("send initial voucher");

        let mut frame = VpnPaddedCellV1::zeroed();
        timeout(Duration::from_secs(1), reader.read_exact(frame.as_mut()))
            .await
            .expect("the initial voucher must not wait for another write")
            .expect("read protected voucher cell");
        let cell = frame
            .parse_with_flow_label_bits(VpnFlowLabelV1::MAX_BITS)
            .expect("parse voucher cell");
        assert_eq!(cell.header.class, VpnCellClassV1::Control);
        assert_eq!(cell.header.sequence, 0);
        assert!(cell.payload.starts_with(VPN_USAGE_VOUCHER_CONTROL_MAGIC));
        assert_eq!(sequence, 1);
    }
    #[test]
    fn usage_voucher_counters_follow_relay_direction_semantics() {
        let counters = UsageVoucherCounters::default();
        counters.record_client_to_relay(11);
        counters.record_relay_to_client(29);
        assert_eq!(counters.snapshot(), (11, 29));
    }
    #[test]
    fn partial_tun_write_fails_before_relay_bytes_are_billed() {
        let counters = UsageVoucherCounters::default();
        let error = record_relay_packet_after_tun_write(&counters, 1_280, 640)
            .expect_err("partial packet write must fail closed");
        assert!(error.to_string().contains("wrote 640 of 1280 bytes"));
        assert_eq!(counters.snapshot(), (0, 0));

        assert_eq!(
            record_relay_packet_after_tun_write(&counters, 1_280, 1_280)
                .expect("complete packet write"),
            1_280
        );
        assert_eq!(counters.snapshot(), (0, 1_280));
    }
    #[test]
    fn usage_voucher_counters_saturate_instead_of_wrapping() {
        let counters = UsageVoucherCounters::default();
        counters.add_ingress(u64::MAX);
        counters.add_ingress(1);
        counters.add_egress(u64::MAX);
        counters.add_egress(1);
        assert_eq!(counters.snapshot(), (u64::MAX, u64::MAX));
    }
    #[test]
    fn usage_voucher_counters_request_refresh_before_credit_is_consumed() {
        let counters = UsageVoucherCounters::default();
        counters.set_authorization(
            USAGE_VOUCHER_BYTE_CREDIT_WINDOW,
            USAGE_VOUCHER_BYTE_CREDIT_WINDOW,
        );
        assert!(!counters.refresh_before_ingress(1_280));
        counters.add_ingress(USAGE_VOUCHER_BYTE_CREDIT_WINDOW / 2);
        assert!(counters.refresh_before_ingress(1_280));
        counters.record_relay_to_client(USAGE_VOUCHER_BYTE_CREDIT_WINDOW / 2);
        assert!(counters.remaining_egress_credit() <= USAGE_VOUCHER_BYTE_REFRESH_THRESHOLD);
    }
    #[test]
    fn usage_voucher_signer_rejects_wrong_metering_seed() {
        let session_id = TEST_SESSION_ID;
        let ticket = test_helper_ticket(session_id);
        let wrong_metering_seed = "77".repeat(32);
        let raw_payload =
            test_connect_payload_json(session_id, &ticket, Some(wrong_metering_seed.as_str()));
        let payload = parse_connect_payload(Some(&raw_payload)).expect("payload");
        let error = match UsageVoucherSigner::from_payload(&payload, ticket) {
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
    fn excluded_route_plan_marks_ownership_and_uses_numeric_exact_readbacks() {
        let mut outputs = ["\n", "default via 192.0.2.1 dev eth0 proto 4\n"].into_iter();
        let mut commands = Vec::new();
        let (snapshot, mutation) =
            plan_excluded_route_mutation_with("198.51.100.0/24", |_program, args| {
                commands.push(args);
                Ok(outputs.next().expect("one fake route readback").to_owned())
            })
            .expect("plan exact helper-owned exclusion");
        assert_eq!(
            snapshot.installed_route.as_deref(),
            Some("sora-vpn-planned-route-v1 198.51.100.0/24 via 192.0.2.1 dev eth0 proto 186"),
            "planning must precommit the complete route ownership tuple"
        );
        assert!(
            commands
                .iter()
                .all(|args| args.iter().any(|arg| arg == "-N"))
        );
        assert_eq!(mutation[1..3], ["route", "add"]);
        assert!(!mutation.iter().any(|argument| argument == "replace"));
        assert!(mutation.ends_with(&["proto".to_owned(), "186".to_owned()]));
        validate_installed_excluded_route(
            &snapshot,
            &mutation,
            "198.51.100.0/24 via 192.0.2.1 dev eth0 proto 186",
        )
        .expect("exact numeric readback proves installed ownership");

        for (cidr, installed) in [
            (
                "198.51.100.7/32",
                "198.51.100.7 via 192.0.2.1 dev eth0 proto 186",
            ),
            (
                "2001:db8::7/128",
                "2001:db8::7 via 2001:db8::1 dev eth0 proto 186",
            ),
        ] {
            let snapshot = ExcludedRouteSnapshot {
                cidr: cidr.to_owned(),
                family: parse_cidr(cidr).expect("host exclusion").family(),
                installed_route: Some(format!(
                    "{PLANNED_EXCLUDED_ROUTE_PREFIX_V1}{cidr} via 192.0.2.1 dev eth0 proto {EXCLUDED_ROUTE_PROTOCOL_V1}"
                )),
            };
            let mutation = installed
                .split_ascii_whitespace()
                .take(7)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            validate_installed_excluded_route(&snapshot, &mutation, installed)
                .expect("iproute2 host-route suffix elision remains semantically exact");
        }
    }
    #[test]
    fn excluded_route_plan_rejects_an_exact_ambient_route_without_mutation() {
        let mut commands = Vec::new();
        let error = plan_excluded_route_mutation_with("198.51.100.0/24", |_program, args| {
            commands.push(args);
            Ok("198.51.100.0/24 via 192.0.2.9 dev eth0 proto 4 metric 20\n".to_owned())
        })
        .expect_err("first-release exclusions never borrow or replace ambient exact routes");
        assert!(
            error
                .to_string()
                .contains("exact ambient route already exists")
        );
        assert_eq!(
            commands.len(),
            1,
            "no default lookup or mutation follows rejection"
        );
    }
    #[test]
    fn excluded_route_restore_requires_exact_helper_owned_readback() {
        let snapshot = ExcludedRouteSnapshot {
            cidr: "198.51.100.0/24".to_owned(),
            family: IpFamily::V4,
            installed_route: Some("198.51.100.0/24 via 192.0.2.1 dev eth0 proto 186".to_owned()),
        };
        assert_eq!(
            excluded_route_restore_action(
                &snapshot,
                Some("198.51.100.0/24 via 192.0.2.1 dev eth0 proto 186"),
            )
            .expect("exact installed readback is helper-owned"),
            ExcludedRouteRestoreAction::DeleteInstalled(
                "198.51.100.0/24 via 192.0.2.1 dev eth0 proto 186".to_owned()
            )
        );
        assert_eq!(
            excluded_route_restore_action(&snapshot, None).expect("absence is idempotent"),
            ExcludedRouteRestoreAction::AlreadyAbsent
        );
        let drift = excluded_route_restore_action(
            &snapshot,
            Some("198.51.100.0/24 via 203.0.113.1 dev eth1 proto 4"),
        )
        .expect_err("live external drift must never be overwritten");
        assert!(drift.to_string().contains("live route state drifted"));

        let unproven = ExcludedRouteSnapshot {
            installed_route: None,
            ..snapshot.clone()
        };
        assert_eq!(
            excluded_route_restore_action(&unproven, None).expect("no route needs no cleanup"),
            ExcludedRouteRestoreAction::AlreadyAbsent
        );
        assert!(
            excluded_route_restore_action(
                &unproven,
                Some("198.51.100.0/24 via 192.0.2.1 dev eth0 proto 186"),
            )
            .is_err(),
            "a crash before exact installed readback persistence must retain repair state"
        );

        let precommitted = ExcludedRouteSnapshot {
            installed_route: Some(format!(
                "{PLANNED_EXCLUDED_ROUTE_PREFIX_V1}198.51.100.0/24 via 192.0.2.1 dev eth0 proto {EXCLUDED_ROUTE_PROTOCOL_V1}"
            )),
            ..snapshot
        };
        let current = "198.51.100.0/24 via 192.0.2.1 dev eth0 proto 186 metric 20";
        assert_eq!(
            excluded_route_restore_action(&precommitted, Some(current))
                .expect("an exact precommitted ownership tuple is recoverable"),
            ExcludedRouteRestoreAction::DeleteInstalled(current.to_owned())
        );
        for drifted in [
            "198.51.100.0/24 via 203.0.113.1 dev eth0 proto 186",
            "198.51.100.0/24 via 192.0.2.1 dev eth1 proto 186",
            "198.51.100.0/24 via 192.0.2.1 dev eth0 proto 4",
            "203.0.113.0/24 via 192.0.2.1 dev eth0 proto 186",
        ] {
            assert!(
                excluded_route_restore_action(&precommitted, Some(drifted)).is_err(),
                "recovery must retain a route outside the precommitted ownership tuple: {drifted}"
            );
        }
    }
    #[test]
    fn exact_route_readback_rejects_ambiguous_results() {
        assert_eq!(
            exact_route_readback("\n", "198.51.100.0/24").expect("absent route"),
            None
        );
        assert_eq!(
            exact_route_readback(
                "198.51.100.7 via 192.0.2.1 dev eth0 proto 186\n",
                "198.51.100.7/32",
            )
            .expect("IPv4 host route is semantically exact")
            .as_deref(),
            Some("198.51.100.7 via 192.0.2.1 dev eth0 proto 186")
        );
        assert_eq!(
            exact_route_readback(
                "2001:db8::7 via 2001:db8::1 dev eth0 proto 186\n",
                "2001:db8::7/128",
            )
            .expect("IPv6 host route is semantically exact")
            .as_deref(),
            Some("2001:db8::7 via 2001:db8::1 dev eth0 proto 186")
        );
        assert!(
            exact_route_readback(
                "198.51.100.0/24 dev eth0\n198.51.100.0/24 dev eth1\n",
                "198.51.100.0/24",
            )
            .is_err()
        );
        assert!(
            exact_route_readback("198.51.100.8 dev eth0\n", "198.51.100.7/32").is_err(),
            "a single nonmatching route is not an exact-prefix readback"
        );
    }
    #[test]
    fn v1_dns_requires_resolvectl_without_resolv_conf_fallback() {
        let dns = vec!["1.1.1.1".to_owned()];
        let error = plan_dns_backend_for_availability("srvpn0000000000", &dns, false)
            .expect_err("direct resolv.conf mutation is outside the V1 contract");
        assert!(error.to_string().contains("requires a trusted resolvectl"));
        assert!(
            matches!(
                plan_dns_backend_for_availability("srvpn0000000000", &dns, true),
                Ok(Some(DnsBackendState::Resolved { .. }))
            ),
            "trusted resolvectl is the only V1 DNS backend"
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
        let runtime_flags = LINUX_IFF_TUN_BITS | LINUX_IFF_NO_PI_BITS;
        ensure_exact_tun_runtime_flags("srvpn0123456789", runtime_flags)
            .expect("exact packet framing flags");
        for unsafe_extra in [LINUX_IFF_TUN_EXCL_BITS, 0x0100, 0x4000] {
            assert!(
                ensure_exact_tun_runtime_flags("srvpn0123456789", runtime_flags | unsafe_extra,)
                    .is_err(),
                "unexpected runtime TUN flag {unsafe_extra:#06x} must fail closed"
            );
        }
        ensure_exact_tun_interface_name("srvpn0123456789", "srvpn0123456789")
            .expect("exact kernel name");
        let error = ensure_exact_tun_interface_name("srvpn0123456789", "srvpn9876543210")
            .expect_err("renamed or pre-existing interface must fail closed");
        assert!(error.to_string().contains("instead of requested"));
    }
    #[test]
    fn relay_session_id_matches_torii_derivation() {
        let derived = parse_canonical_session_id(TEST_SESSION_ID).expect("canonical session id");
        assert_eq!(hex::encode(derived), TEST_SESSION_ID);
        for invalid in [
            "f69c894aa32726fe586fab520f88ae4",
            "F69C894AA32726FE586FAB520F88AE42",
            "f69c894aa32726fe586fab520f88ae4z",
        ] {
            assert!(parse_canonical_session_id(invalid).is_err());
        }
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
    fn active_packet_loop_uses_the_exact_remaining_ticket_lifetime() {
        assert_eq!(
            authenticated_ticket_expiry_remaining_at(1_001, 1_000)
                .expect("one millisecond remains"),
            Duration::from_millis(1)
        );
        for now_ms in [1_001, 1_002] {
            let error = authenticated_ticket_expiry_remaining_at(1_001, now_ms)
                .expect_err("expired ticket must not enter the active packet loop");
            assert!(error.to_string().contains("active packet loop"));
        }
    }
    #[test]
    fn expired_ticket_cannot_cross_the_started_publication_barrier() {
        ensure_authenticated_ticket_unexpired_for_connected_state_at(1_001, 1_000)
            .expect("unexpired ticket may cross STARTED");
        for now_ms in [1_001, 1_002] {
            let error = ensure_authenticated_ticket_unexpired_for_connected_state_at(1_001, now_ms)
                .expect_err("expired ticket must not publish connected state");
            assert!(error.to_string().contains("connected state publication"));
        }
    }
    #[test]
    fn connected_publication_rechecks_expiry_and_exact_child_liveness() {
        ensure_connected_publication_ready_at(1_001, 1_000, true)
            .expect("live exact child with an unexpired ticket may publish");

        let expired_but_live = ensure_connected_publication_ready_at(1_001, 1_001, true)
            .expect_err("an expiry-triggered child may still be live while shutting down");
        assert!(
            expired_but_live
                .to_string()
                .contains("connected state publication")
        );

        let dead = ensure_connected_publication_ready_at(1_001, 1_000, false)
            .expect_err("dead exact child must not publish connected state");
        assert!(
            dead.to_string()
                .contains("exact network worker is not alive")
        );
    }
    #[test]
    fn monotonic_ticket_deadline_never_adds_wall_sample_delay() {
        let monotonic_before_wall = tokio::time::Instant::now();
        let simulated_wall_sample_delay = Duration::from_millis(250);
        let monotonic_after_wall = monotonic_before_wall + simulated_wall_sample_delay;
        let remaining = Duration::from_millis(1_000);

        let deadline = authenticated_ticket_expiry_deadline_at(2_000, 1_000, monotonic_before_wall)
            .expect("convert signed expiry from the conservative anchor");
        assert_eq!(deadline, monotonic_before_wall + remaining);
        assert!(
            deadline < monotonic_after_wall + remaining,
            "descheduling between monotonic and wall samples must shorten, never extend, lifetime"
        );
    }
    #[test]
    fn privileged_preparation_has_one_deadline_inside_the_worker_tun_wait() {
        assert!(
            PRIVILEGED_PREPARATION_TIMEOUT < NETWORK_WORKER_TUN_TIMEOUT,
            "root preparation must fail before the unprivileged worker abandons its TUN wait"
        );
        let child_bound = NETWORK_WORKER_READY_TIMEOUT * 4
            + NETWORK_WORKER_TUN_TIMEOUT
            + PRIVILEGED_PREPARATION_TIMEOUT;
        assert!(
            CONNECT_READY_TIMEOUT > child_bound,
            "the public parent must never kill a legitimate bounded child phase early"
        );
        let now = Instant::now();
        let preparation_deadline = now + PRIVILEGED_PREPARATION_TIMEOUT;
        let first = privileged_command_deadlines(preparation_deadline)
            .expect("reserve one fixed command-custody interval");
        let second = privileged_command_deadlines(preparation_deadline)
            .expect("route count must not create a new relative deadline");
        assert_eq!(first, second);
        assert_eq!(first.1, preparation_deadline);
        assert_eq!(first.0 + PROCESS_KILL_REAP_TIMEOUT, preparation_deadline);
        assert_eq!(
            privileged_preparation_deadline_at(now, Duration::from_secs(7))
                .expect("short authenticated lifetime fits monotonic clock"),
            now + Duration::from_secs(7),
            "the signed ticket lifetime must shorten root mutation authority"
        );
        assert_eq!(
            privileged_preparation_deadline_at(now, Duration::from_secs(90))
                .expect("long authenticated lifetime fits monotonic clock"),
            preparation_deadline,
            "the fixed preparation timeout remains the upper bound"
        );
        ensure_privileged_preparation_deadline_at(preparation_deadline, now)
            .expect("future absolute deadline");
        assert!(
            ensure_privileged_preparation_deadline_at(preparation_deadline, preparation_deadline,)
                .is_err(),
            "the absolute preparation deadline is strict"
        );
    }
    #[test]
    fn cli_requires_connect_payload_on_stdin() {
        let payload = r#"{"sessionId":"session-1","relayEndpoint":"/ip4/93.184.216.34/udp/7777/quic","exitClass":"standard","helperTicketHex":"aa","relayTlsSpkiSha256Hex":"abababababababababababababababababababababababababababababababab","paddingBudgetMs":15,"routePushes":[],"excludedRoutes":[],"dnsServers":[],"tunnelAddresses":["10.208.0.2/32"],"mtuBytes":1280}"#;
        parse_fixed_cli_arguments(vec!["connect".to_owned(), payload.to_owned()])
            .expect_err("connect secrets must not be accepted through argv");
        let cli = parse_fixed_cli_arguments(vec!["connect".to_owned()]).expect("parse");
        assert!(matches!(cli.command, Command::Connect));
    }
    #[test]
    fn install_check_derives_display_state_without_mutating_persisted_state() {
        let original = State {
            active: true,
            message: "persisted message".to_owned(),
            ..State::default()
        };
        let display = install_check_display_state();
        assert_eq!(display.message, "ready");
        assert!(!display.active);
        assert_eq!(display.bytes_in, 0);
        assert_eq!(display.bytes_out, 0);
        assert_eq!(original.message, "persisted message");

        assert_eq!(install_check_display_state().message, "ready");
    }
    #[test]
    fn privileged_commands_require_an_explicit_session_id() {
        for command in ["disconnect", "repair"] {
            parse_fixed_cli_arguments(vec![command.to_owned()])
                .expect_err("session-less privileged teardown must fail");
            let cli = parse_fixed_cli_arguments(vec![
                command.to_owned(),
                "--session-id".to_owned(),
                TEST_SESSION_ID.to_owned(),
            ])
            .expect("session-bound command");
            match cli.command {
                Command::Disconnect { session_id } | Command::Repair { session_id } => {
                    assert_eq!(session_id, TEST_SESSION_ID);
                }
                other => panic!("unexpected command: {other:?}"),
            }
        }
    }
    #[test]
    fn privileged_caller_identity_fails_closed() {
        assert_eq!(
            validate_privileged_caller_identity(1_000, 0, 0, 1_000, 1_000, 1_000)
                .expect("setuid identity"),
            PrivilegedCaller {
                uid: 1_000,
                gid: 1_000,
            }
        );
        for (label, ids) in [
            ("direct root or sudo", (0, 0, 0, 1_000, 1_000, 1_000)),
            (
                "unprivileged or capability-only executable",
                (1_000, 1_000, 1_000, 1_000, 1_000, 1_000),
            ),
            (
                "missing saved root UID",
                (1_000, 0, 1_000, 1_000, 1_000, 1_000),
            ),
            ("privileged effective GID", (1_000, 0, 0, 1_000, 0, 1_000)),
        ] {
            let error =
                validate_privileged_caller_identity(ids.0, ids.1, ids.2, ids.3, ids.4, ids.5)
                    .expect_err("unsafe credentials must fail");
            assert!(
                error.to_string().contains("unsafe privileged invocation"),
                "unexpected error for {label}: {error}"
            );
        }
        validate_privileged_executable_custody(0, 0o104_755).expect("root-owned setuid executable");
        for (label, owner_uid, mode) in [
            ("non-root owner", 1_000, 0o104_755),
            ("capability-only mode", 0, 0o100_755),
            ("group writable", 0, 0o104_775),
            ("other writable", 0, 0o104_757),
        ] {
            let error = validate_privileged_executable_custody(owner_uid, mode)
                .expect_err("unsafe executable custody must fail");
            assert!(
                error.to_string().contains("unsafe privileged invocation"),
                "unexpected error for {label}: {error}"
            );
        }
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
    #[cfg(target_os = "linux")]
    #[test]
    fn fresh_command_file_has_no_privilege_capability_xattr() {
        let root = private_test_state_root("command-capability-xattr");
        let path = root.join("command");
        let mut options = fs::OpenOptions::new();
        options
            .create_new(true)
            .write(true)
            .mode(0o700)
            .custom_flags(nix::libc::O_CLOEXEC | nix::libc::O_NOFOLLOW);
        drop(options.open(&path).expect("create command fixture"));
        assert!(
            !system_executable_has_file_capabilities(&path)
                .expect("query absent security.capability xattr")
        );
        fs::remove_file(path).expect("remove command fixture");
        fs::remove_dir(root).expect("remove command fixture root");
    }
    #[test]
    #[cfg(unix)]
    fn issuer_public_key_anchor_is_canonical_private_and_owner_pinned() {
        let root = private_test_state_root("issuer-anchor");
        let path = root.join("issuer.hex");
        let issuer = test_ticket_issuer(0xAA);
        let (algorithm, public_key_bytes) = issuer
            .public_key()
            .try_to_bytes()
            .expect("fixture issuer public key");
        assert_eq!(algorithm, Algorithm::Ed25519);
        let encoded = hex::encode(public_key_bytes);
        write_file_atomic(
            &path,
            encoded.as_bytes(),
            0o600,
            true,
            "helper-ticket issuer public key",
        )
        .expect("write issuer anchor");

        let loaded = load_helper_ticket_issuer_public_key_at(&path, effective_uid())
            .expect("load root-equivalent test anchor");
        assert_eq!(&loaded, issuer.public_key());
        let wrong_owner = effective_uid().wrapping_add(1);
        assert!(load_helper_ticket_issuer_public_key_at(&path, wrong_owner).is_err());

        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))
            .expect("make anchor group-readable");
        assert!(load_helper_ticket_issuer_public_key_at(&path, effective_uid()).is_err());
        fs::remove_file(path).expect("remove issuer anchor");
        fs::remove_dir(root).expect("remove issuer root");
    }
    #[test]
    #[cfg(unix)]
    fn issuer_public_key_anchor_rejects_links_and_noncanonical_content() {
        use std::os::unix::fs::symlink;

        let root = private_test_state_root("issuer-anchor-links");
        let target = root.join("issuer.hex");
        let link = root.join("issuer-link.hex");
        let hard_link = root.join("issuer-hard-link.hex");
        let issuer = test_ticket_issuer(0xAA);
        let (_, public_key_bytes) = issuer
            .public_key()
            .try_to_bytes()
            .expect("fixture issuer public key");
        let encoded = hex::encode(public_key_bytes);
        write_file_atomic(
            &target,
            encoded.as_bytes(),
            0o600,
            true,
            "helper-ticket issuer public key",
        )
        .expect("write issuer anchor");

        symlink(&target, &link).expect("create issuer symlink");
        assert!(load_helper_ticket_issuer_public_key_at(&link, effective_uid()).is_err());
        fs::remove_file(&link).expect("remove issuer symlink");

        fs::hard_link(&target, &hard_link).expect("create issuer hard link");
        assert!(load_helper_ticket_issuer_public_key_at(&target, effective_uid()).is_err());
        fs::remove_file(hard_link).expect("remove issuer hard link");

        write_file_atomic(
            &target,
            format!("{encoded}\n").as_bytes(),
            0o600,
            true,
            "helper-ticket issuer public key",
        )
        .expect("write noncanonical anchor");
        assert!(load_helper_ticket_issuer_public_key_at(&target, effective_uid()).is_err());

        write_file_atomic(
            &target,
            "00".repeat(32).as_bytes(),
            0o600,
            true,
            "helper-ticket issuer public key",
        )
        .expect("write inert anchor");
        assert!(load_helper_ticket_issuer_public_key_at(&target, effective_uid()).is_err());

        fs::remove_file(target).expect("remove issuer target");
        fs::remove_dir(root).expect("remove issuer root");
    }
    #[test]
    fn production_issuer_anchor_path_is_fixed_and_not_caller_selected() {
        assert_eq!(
            HELPER_TICKET_ISSUER_PUBLIC_KEY_PATH,
            "/etc/sora-vpn-controller/helper-ticket-issuer-public-key.hex"
        );
        assert!(load_helper_ticket_issuer_public_key_at(Path::new("relative.hex"), 0).is_err());
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
        assert!(
            error
                .to_string()
                .contains("not a supported Norito state frame")
        );
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
    fn status_never_discloses_another_local_users_session() {
        let state = cleanup_test_state(DnsBackendState::Resolved {
            interface_name: "srvpn0000000000".to_owned(),
        });
        assert!(
            authorize_status_access(
                &state,
                PrivilegedCaller {
                    uid: 2_000,
                    gid: 2_001,
                },
            )
            .is_err()
        );
        authorize_status_access(
            &state,
            PrivilegedCaller {
                uid: 1_000,
                gid: 9_999,
            },
        )
        .expect("the authenticated owning UID may inspect its session");

        let display = install_check_display_state();
        assert!(!display.active);
        assert!(display.owner_uid.is_none());
        assert!(display.session_id.is_none());
        assert!(display.relay_endpoint.is_none());
        assert!(display.applied_network.is_none());
        assert_eq!(display.message, "ready");
    }
    #[test]
    fn terminal_state_retains_network_snapshot_until_repair_succeeds() {
        let applied = AppliedNetworkState {
            interface_name: "srvpn0000000000".to_owned(),
            journal_phase: NetworkJournalPhase::Prepared,
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

        let mut exiting = State {
            worker_identity: Some(WorkerProcessIdentity {
                pid: 42,
                start_time_ticks: 10,
                executable_device: 11,
                executable_inode: 12,
                role: WorkerRole::Tunnel,
            }),
            owner_uid: Some(1_000),
            session_id: Some("session-1".to_owned()),
            relay_endpoint: Some("/ip4/93.184.216.34/udp/7777/quic".to_owned()),
            relay_id: Some([0x22; 32]),
            network_policy_hash: Some([0x11; 32]),
            ticket_expires_at_ms: Some(u64::MAX),
            interface_name: Some("srvpn0000000000".to_owned()),
            network_service: Some("resolvectl".to_owned()),
            bytes_in: 123,
            bytes_out: 456,
            message: "private relay handshake failure".to_owned(),
            ..State::default()
        };
        apply_terminal_network_lifecycle(&mut exiting, false, false);
        assert_eq!(exiting.owner_uid, Some(1_000));
        exiting.worker_identity = None;
        apply_terminal_network_lifecycle(&mut exiting, false, false);
        assert!(!state_has_session_binding(&exiting));
        assert_eq!(exiting.bytes_in, 0);
        assert_eq!(exiting.bytes_out, 0);
        assert!(exiting.interface_name.is_none());
        assert!(exiting.network_service.is_none());
        assert_eq!(exiting.message, "ready");
        authorize_status_access(
            &exiting,
            PrivilegedCaller {
                uid: 2_000,
                gid: 2_001,
            },
        )
        .expect("sanitized terminal state is safe for any local caller");
    }

    fn test_worker_identity(pid: u32, role: WorkerRole) -> WorkerProcessIdentity {
        WorkerProcessIdentity {
            pid,
            start_time_ticks: u64::from(pid) + 10,
            executable_device: 11,
            executable_inode: u64::from(pid) + 12,
            role,
        }
    }

    fn active_runtime_test_state() -> State {
        State {
            active: true,
            worker_identity: Some(test_worker_identity(42, WorkerRole::Tunnel)),
            network_worker_identity: Some(test_worker_identity(43, WorkerRole::Network)),
            owner_uid: Some(1_000),
            session_id: Some("session-1".to_owned()),
            relay_endpoint: Some("/ip4/93.184.216.34/udp/7777/quic".to_owned()),
            relay_id: Some([0x22; 32]),
            network_policy_hash: Some([0x11; 32]),
            ticket_expires_at_ms: Some(u64::MAX),
            interface_name: Some("srvpn0000000000".to_owned()),
            applied_network: Some(AppliedNetworkState {
                interface_name: "srvpn0000000000".to_owned(),
                journal_phase: NetworkJournalPhase::Prepared,
                dns_backend: None,
                excluded_route_snapshots: Vec::new(),
            }),
            message: "connected".to_owned(),
            ..State::default()
        }
    }

    #[test]
    fn connect_readiness_requires_a_live_exact_network_child() {
        let state = active_runtime_test_state();
        let supervisor = state
            .worker_identity
            .as_ref()
            .expect("active supervisor")
            .clone();
        let expected_network = state
            .network_worker_identity
            .as_ref()
            .expect("active network child")
            .clone();
        assert!(
            connect_state_ready_with(&state, &supervisor, true, 1_000, |identity| {
                assert_eq!(identity, &expected_network);
                Ok(true)
            })
            .expect("exact live identities are ready")
        );
        assert!(
            !connect_state_ready_with(&state, &supervisor, true, 1_000, |_| Ok(false))
                .expect("dead network child is not ready")
        );

        let mut expiring = state.clone();
        expiring.ticket_expires_at_ms = Some(1_001);
        validate_state_for_persistence_at(&expiring, 1_000)
            .expect("unexpired active state remains persistable");
        assert!(
            validate_state_for_persistence_at(&expiring, 1_001).is_err(),
            "an active frame cannot start persistence at the signed deadline"
        );
        assert!(
            !connect_state_ready_with(&expiring, &supervisor, true, 1_001, |_| {
                panic!("expired state must fail before a still-live child can imply readiness")
            })
            .expect("expiry-triggered child shutdown is not connected")
        );

        let mut repairing = state.clone();
        repairing.repair_required = true;
        assert!(
            !connect_state_ready_with(&repairing, &supervisor, true, 1_000, |_| Ok(true))
                .expect("repair state is not connected")
        );

        let mut missing = state.clone();
        missing.network_worker_identity = None;
        assert!(
            !connect_state_ready_with(&missing, &supervisor, true, 1_000, |_| {
                panic!("missing network child must not reach liveness inspection")
            })
            .expect("missing network child is not ready")
        );
    }

    #[test]
    fn current_state_scrub_never_reports_active_without_a_live_network_child() {
        let mut dead = active_runtime_test_state();
        scrub_stale_process_with(&mut dead, 1_000, |identity| {
            Ok(identity.role == WorkerRole::Tunnel)
        })
        .expect("scrub dead network child");
        assert!(!dead.active);
        assert!(dead.repair_required);
        assert!(dead.worker_identity.is_some());
        assert!(dead.network_worker_identity.is_none());
        assert!(dead.applied_network.is_some());
        validate_state_for_persistence(&dead).expect("dead child normalizes to repair state");

        let mut missing = active_runtime_test_state();
        missing.network_worker_identity = None;
        scrub_stale_process_with(&mut missing, 1_000, |_| Ok(true))
            .expect("scrub missing network child");
        assert!(!missing.active);
        assert!(missing.repair_required);
        assert!(missing.applied_network.is_some());
        validate_state_for_persistence(&missing).expect("missing child normalizes to repair state");

        let mut expired = active_runtime_test_state();
        expired.ticket_expires_at_ms = Some(1_001);
        scrub_stale_process_with(&mut expired, 1_001, |_| Ok(true))
            .expect("scrub expired state with both exact children still alive");
        assert!(!expired.active);
        assert!(expired.repair_required);
        assert!(expired.worker_identity.is_some());
        assert!(expired.network_worker_identity.is_some());
        assert!(expired.applied_network.is_some());
        assert!(expired.message.contains("ticket expired"));
        validate_state_for_persistence_at(&expired, 1_001)
            .expect("expired active state normalizes to owner-repairable custody");
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
        state.owner_uid = Some(1_000);
        state.session_id = Some("session-1".to_owned());
        state.relay_endpoint = Some("/ip4/93.184.216.34/udp/7777/quic".to_owned());
        state.relay_id = Some([0x22; 32]);
        state.network_policy_hash = Some([0x11; 32]);
        state.ticket_expires_at_ms = Some(u64::MAX);
        validate_state_invariants(&state)
            .expect("unsafe legacy active state remains readable for owner-authorized repair");
        assert!(
            validate_state_for_persistence(&state).is_err(),
            "active state cannot omit its network child"
        );
        state.network_worker_identity = Some(test_worker_identity(43, WorkerRole::Network));
        assert!(
            validate_state_for_persistence(&state).is_err(),
            "active state cannot omit its repair journal"
        );
        state.applied_network = active_runtime_test_state().applied_network;
        assert!(validate_state_for_persistence(&state).is_ok());
        state
            .applied_network
            .as_mut()
            .expect("active journal")
            .journal_phase = NetworkJournalPhase::CleaningRoutes;
        assert!(
            validate_state_for_persistence(&state).is_err(),
            "active state cannot publish an incomplete or cleaning journal"
        );
        state
            .applied_network
            .as_mut()
            .expect("active journal")
            .journal_phase = NetworkJournalPhase::Prepared;
        state.repair_required = true;
        assert!(
            validate_state_for_persistence(&state).is_err(),
            "repair state cannot also publish active"
        );
    }
    #[test]
    fn worker_start_requires_the_exact_pending_controller_identity_and_owner() {
        let identity = WorkerProcessIdentity {
            pid: 42,
            start_time_ticks: 10,
            executable_device: 11,
            executable_inode: 12,
            role: WorkerRole::Tunnel,
        };
        let mut state = State {
            message: "authenticating unprivileged connect payload".to_owned(),
            worker_identity: Some(identity.clone()),
            owner_uid: Some(1_000),
            ..State::default()
        };
        let caller = PrivilegedCaller {
            uid: 1_000,
            gid: 1_000,
        };
        authorize_unvalidated_worker_start(&state, &identity, caller).expect("exact start record");

        state.session_id = Some(TEST_SESSION_ID.to_owned());
        assert!(authorize_unvalidated_worker_start(&state, &identity, caller).is_err());
        state.session_id = None;
        state.worker_identity.as_mut().expect("identity").pid += 1;
        assert!(authorize_unvalidated_worker_start(&state, &identity, caller).is_err());
        assert!(authorize_unvalidated_worker_start(&State::default(), &identity, caller).is_err());
        state.worker_identity = Some(identity.clone());
        assert!(
            authorize_unvalidated_worker_start(
                &state,
                &identity,
                PrivilegedCaller {
                    uid: 1_001,
                    gid: 1_001,
                }
            )
            .is_err()
        );
    }
    #[test]
    fn privileged_session_control_requires_owner_and_exact_session() {
        let state = State {
            owner_uid: Some(1_000),
            session_id: Some("session-1".to_owned()),
            relay_endpoint: Some("/ip4/93.184.216.34/udp/7777/quic".to_owned()),
            relay_id: Some([0x22; 32]),
            network_policy_hash: Some([0x11; 32]),
            ..State::default()
        };
        authorize_session_control(
            &state,
            PrivilegedCaller {
                uid: 1_000,
                gid: 1_000,
            },
            "session-1",
        )
        .expect("owner controls exact session");
        assert!(
            authorize_session_control(
                &state,
                PrivilegedCaller {
                    uid: 1_001,
                    gid: 1_001,
                },
                "session-1"
            )
            .is_err()
        );
        assert!(
            authorize_session_control(
                &state,
                PrivilegedCaller {
                    uid: 1_000,
                    gid: 1_000,
                },
                "session-2"
            )
            .is_err()
        );
        assert!(
            authorize_connect_replacement(
                &state,
                PrivilegedCaller {
                    uid: 1_001,
                    gid: 1_001,
                }
            )
            .is_err()
        );
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
    fn persisted_process_start_time_not_argv_drift_is_the_exit_boundary() {
        assert!(persisted_start_time_matches(77, Some(77)));
        assert!(!persisted_start_time_matches(77, Some(78)));
        assert!(!persisted_start_time_matches(77, None));

        assert!(worker_cmdline_has_exact_role(
            b"/proc/self/exe\0run-tunnel\0",
            WorkerRole::Tunnel,
        ));
        assert!(
            !worker_cmdline_has_exact_role(
                b"/proc/self/exe\0run-tunnel\0extra\0",
                WorkerRole::Tunnel,
            ),
            "extra argv is identity drift, never an exact internal launch"
        );
        for ambiguous in [
            b"/proc/self/exe\0run-tunnel".as_slice(),
            b"/proc/self/exe\0run-tunnel\0\0".as_slice(),
            b"\0/proc/self/exe\0run-tunnel\0".as_slice(),
            b"/proc/self/exe\0\0run-tunnel\0".as_slice(),
        ] {
            assert!(
                !worker_cmdline_has_exact_role(ambiguous, WorkerRole::Tunnel),
                "empty fields and missing terminators are argv identity drift"
            );
        }
        assert!(
            !worker_cmdline_has_exact_role(b"/bin/other\0changed-role\0", WorkerRole::Tunnel,),
            "same start time with exec/argv drift stays a live process for pidfd termination but fails status authentication"
        );
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
