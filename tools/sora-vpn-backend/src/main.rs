#![allow(unexpected_cfgs)]
//! Runs the privileged local backend used by the Sora VPN client.
use clap::Parser;
use norito::{
    DecodeLimits,
    codec::{Decode, Encode},
};
#[cfg(unix)]
use std::os::fd::AsRawFd as _;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt as _;
#[cfg(unix)]
use std::os::unix::fs::{
    DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt,
};
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::{
    collections::{HashSet, VecDeque},
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, Read as _, Seek as _, SeekFrom, Write as _},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
    process::{Child, Command as ProcessCommand, ExitCode, ExitStatus, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[cfg(target_os = "linux")]
use std::{ffi::CStr, os::fd::FromRawFd};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, unix::AsyncFd},
    net::{UnixListener, UnixStream},
    signal::unix::{SignalKind, signal},
    sync::{OwnedSemaphorePermit, Semaphore, watch},
    task::JoinSet,
    time::timeout,
};
const DEFAULT_BACKEND_ENDPOINT: &str = "unix:/run/sora-vpn-backend.sock";
const DEFAULT_BOOTSTRAP_REPLAY_DIRECTORY: &str = "/run/sora-vpn-backend-replay";
const DEFAULT_INTERFACE_PREFIX: &str = "svpn";
const DEFAULT_ROUTE_CMD: &str = "ip";
const PACKET_LEN_PREFIX_BYTES: usize = 2;
const VPN_BACKEND_BOOTSTRAP_MAGIC: &[u8; 8] = b"SVPNBE1\0";
const VPN_BACKEND_STATUS_READY: u8 = 1;
const VPN_BACKEND_BOOTSTRAP_MAX_SKEW_MS: u64 = 60_000;
// A frame may arrive at either edge of the accepted wall-clock window. Keep
// its nonce through both edges so a future-dated frame cannot be replayed
// after only one skew interval from its first receipt.
const VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION: Duration =
    Duration::from_millis(VPN_BACKEND_BOOTSTRAP_MAX_SKEW_MS * 2);
const VPN_BACKEND_BOOTSTRAP_NONCE_CACHE_CAPACITY_V1: usize = 65_536;
const VPN_BACKEND_REPLAY_LOCK_FILE: &str = ".lock";
const VPN_BACKEND_REPLAY_HIGH_WATER_FILE: &str = ".time-high-water";
const VPN_BACKEND_REPLAY_ENTRY_SUFFIX: &str = ".nonce";
const VPN_BACKEND_BOOTSTRAP_SECRET_FILE_MAX_BYTES_V1: usize = 66;
const VPN_BACKEND_MAX_CONCURRENT_SESSIONS_V1: usize = 128;
const VPN_BACKEND_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(5);
const VPN_BACKEND_SOCKET_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const VPN_BACKEND_SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const VPN_BACKEND_BOOTSTRAP_MAX_TUNNEL_ADDRESSES_V1: usize = 8;
const VPN_BACKEND_BOOTSTRAP_MAX_SESSION_ROUTES_V1: usize = 8;
const VPN_BACKEND_BOOTSTRAP_MAX_CIDR_BYTES_V1: usize = 64;
const VPN_BACKEND_BOOTSTRAP_SESSION_ID_HEX_BYTES_V1: usize = 32;
const VPN_BACKEND_BOOTSTRAP_DECODE_LIMITS_V1: DecodeLimits =
    DecodeLimits::new(16, 2 * 1024, 64, 32 * 1024, 8);
const TRUSTED_COMMAND_TIMEOUT_V1: Duration = Duration::from_secs(10);
const TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1: usize = 64 * 1024;
const TRUSTED_COMMAND_MAX_ARGUMENTS_V1: usize = 32;
const TRUSTED_COMMAND_MAX_ARGUMENT_BYTES_V1: usize = 256;
// The first-release backend is a public-Internet exit, not a route into the
// host's control plane or adjacent networks. These ranges are denied even
// when they are reachable through the same interface as the default route.
const IPV4_PROTECTED_DESTINATION_CIDRS_V1: &[&str] = &[
    "0.0.0.0/8",
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.0.0.0/24",
    "192.0.2.0/24",
    "192.88.99.0/24",
    "192.168.0.0/16",
    "198.18.0.0/15",
    "198.51.100.0/24",
    "203.0.113.0/24",
    "224.0.0.0/4",
    "240.0.0.0/4",
];
const IPV6_PROTECTED_DESTINATION_CIDRS_V1: &[&str] = &[
    "::/96",
    "::ffff:0:0/96",
    "64:ff9b::/96",
    "64:ff9b:1::/48",
    "100::/64",
    "2001:2::/48",
    "2001:10::/28",
    "2001:20::/28",
    "2001:db8::/32",
    "2002::/16",
    "3fff::/20",
    "5f00::/16",
    "fc00::/7",
    "fec0::/10",
    "fe80::/10",
    "ff00::/8",
];
#[cfg(target_os = "linux")]
const LINUX_IFF_TUN: nix::libc::c_short = 0x0001;
#[cfg(target_os = "linux")]
const LINUX_IFF_NO_PI: nix::libc::c_short = 0x1000;
#[cfg(target_os = "linux")]
const LINUX_IFF_TUN_EXCL: nix::libc::c_short = 0x8000_u16 as nix::libc::c_short;
#[cfg(target_os = "linux")]
const LINUX_TUNSETIFF: nix::libc::c_ulong = 0x4004_54ca;
#[derive(Debug, Parser)]
#[command(name = "sora-vpn-backend")]
struct Cli {
    #[arg(long, env = "SORANET_VPN_BACKEND_ENDPOINT", default_value = DEFAULT_BACKEND_ENDPOINT)]
    endpoint: String,
    #[arg(long, env = "SORANET_VPN_BACKEND_BOOTSTRAP_SECRET_PATH")]
    bootstrap_secret_path: Option<PathBuf>,
    #[arg(
        long,
        env = "SORANET_VPN_BACKEND_REPLAY_DIRECTORY",
        default_value = DEFAULT_BOOTSTRAP_REPLAY_DIRECTORY
    )]
    replay_directory: PathBuf,
    #[arg(long, env = "SORANET_VPN_BACKEND_ALLOWED_UID")]
    allowed_uid: Option<u32>,
    #[arg(long, env = "SORANET_VPN_BACKEND_ALLOWED_GID")]
    allowed_gid: Option<u32>,
    #[arg(long = "interface-prefix", env = "SORANET_VPN_BACKEND_INTERFACE", default_value = DEFAULT_INTERFACE_PREFIX)]
    interface_prefix: String,
    #[arg(long, env = "SORANET_VPN_BACKEND_MTU", default_value_t = 1280)]
    mtu: u64,
    #[arg(long, env = "SORANET_VPN_BACKEND_EGRESS_INTERFACE")]
    egress_interface: Option<String>,
    #[arg(long, env = "SORANET_VPN_BACKEND_IPV4_FORWARD", default_value_t = true)]
    ipv4_forward: bool,
    #[arg(long, env = "SORANET_VPN_BACKEND_IPV6_FORWARD", default_value_t = true)]
    ipv6_forward: bool,
    #[arg(
        long,
        env = "SORANET_VPN_BACKEND_ENABLE_IPV4_NAT",
        default_value_t = true
    )]
    enable_ipv4_nat: bool,
    #[arg(
        long,
        env = "SORANET_VPN_BACKEND_ENABLE_IPV6_NAT",
        default_value_t = false
    )]
    enable_ipv6_nat: bool,
}
struct BackendConfig {
    endpoint: BackendEndpoint,
    bootstrap_secret: [u8; 32],
    allowed_uid: Option<u32>,
    allowed_gid: Option<u32>,
    bootstrap_replay: Mutex<DurableBootstrapReplay>,
    interface_prefix: String,
    default_mtu: u16,
    ipv4_forward: bool,
    ipv6_forward: bool,
    enable_ipv4_nat: bool,
    enable_ipv6_nat: bool,
    egress_v4_interface: Option<String>,
    egress_v6_interface: Option<String>,
}
#[derive(Debug, Default)]
struct SeenBootstrapNonces {
    nonces: HashSet<[u8; 16]>,
    receipts: VecDeque<(Instant, [u8; 16], u64)>,
}
struct DurableBootstrapReplay {
    seen: SeenBootstrapNonces,
    directory: PathBuf,
    _lock: File,
    high_water_file: File,
    high_water_ms: u64,
    durability_failed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct BackendEndpoint(PathBuf);
impl BackendConfig {
    fn from_cli(cli: Cli) -> Result<Self, BackendError> {
        let endpoint = parse_backend_endpoint(&cli.endpoint)?;
        let bootstrap_secret_path = cli.bootstrap_secret_path.as_deref().ok_or_else(|| {
            BackendError::InvalidConfig(
                "bootstrap_secret_path is required for every backend endpoint".to_owned(),
            )
        })?;
        let bootstrap_secret = read_bootstrap_secret(bootstrap_secret_path)?;
        let bootstrap_replay =
            DurableBootstrapReplay::open(&cli.replay_directory, unix_time_ms()?, Instant::now())?;
        let interface_prefix =
            validate_linux_interface_name(&cli.interface_prefix, "interface_prefix")?;
        let default_mtu = normalize_mtu(cli.mtu)?;
        let egress_v4_interface = if cli.ipv4_forward || cli.enable_ipv4_nat {
            Some(
                resolve_egress_interface(cli.egress_interface.as_deref(), IpFamily::V4)?
                    .ok_or_else(|| {
                        BackendError::InvalidConfig(
                            "could not resolve IPv4 default egress interface while forwarding or NAT is enabled"
                                .to_owned(),
                        )
                    })?,
            )
        } else {
            None
        };
        let egress_v6_interface = if cli.ipv6_forward || cli.enable_ipv6_nat {
            Some(
                resolve_egress_interface(cli.egress_interface.as_deref(), IpFamily::V6)?
                    .ok_or_else(|| {
                        BackendError::InvalidConfig(
                            "could not resolve IPv6 default egress interface while forwarding or NAT is enabled"
                                .to_owned(),
                        )
                    })?,
            )
        } else {
            None
        };
        Ok(Self {
            endpoint,
            bootstrap_secret,
            allowed_uid: cli.allowed_uid.or_else(default_allowed_uid),
            allowed_gid: cli.allowed_gid.or_else(default_allowed_gid),
            bootstrap_replay: Mutex::new(bootstrap_replay),
            interface_prefix,
            default_mtu,
            ipv4_forward: cli.ipv4_forward,
            ipv6_forward: cli.ipv6_forward,
            enable_ipv4_nat: cli.enable_ipv4_nat,
            enable_ipv6_nat: cli.enable_ipv6_nat,
            egress_v4_interface,
            egress_v6_interface,
        })
    }
}
impl std::fmt::Debug for BackendConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BackendConfig")
            .field("endpoint", &self.endpoint)
            .field("bootstrap_secret", &"<redacted>")
            .field("allowed_uid", &self.allowed_uid)
            .field("allowed_gid", &self.allowed_gid)
            .field("interface_prefix", &self.interface_prefix)
            .field("default_mtu", &self.default_mtu)
            .field("ipv4_forward", &self.ipv4_forward)
            .field("ipv6_forward", &self.ipv6_forward)
            .field("enable_ipv4_nat", &self.enable_ipv4_nat)
            .field("enable_ipv6_nat", &self.enable_ipv6_nat)
            .field("egress_v4_interface", &self.egress_v4_interface)
            .field("egress_v6_interface", &self.egress_v6_interface)
            .finish_non_exhaustive()
    }
}
impl Drop for BackendConfig {
    fn drop(&mut self) {
        zeroize_secret(&mut self.bootstrap_secret);
    }
}
fn zeroize_secret(secret: &mut [u8; 32]) {
    secret.fill(0);
    std::hint::black_box(secret);
}
impl BackendEndpoint {
    fn label(&self) -> String {
        format!("unix:{}", self.0.display())
    }
}
fn parse_backend_endpoint(endpoint: &str) -> Result<BackendEndpoint, BackendError> {
    let trimmed = endpoint.trim();
    if let Some(path) = trimmed.strip_prefix("unix:") {
        let path = path.trim();
        if path.is_empty() || !path.starts_with('/') {
            return Err(BackendError::InvalidConfig(
                "backend endpoint unix form must be unix:/absolute/path".to_owned(),
            ));
        }
        return Ok(BackendEndpoint(PathBuf::from(path)));
    }
    Err(BackendError::InvalidConfig(
        "backend endpoint must use unix:/absolute/path; TCP is not an authenticated local peer boundary"
            .to_owned(),
    ))
}
fn read_bootstrap_secret(path: &std::path::Path) -> Result<[u8; 32], BackendError> {
    let mut raw = read_bounded_private_file(
        path,
        VPN_BACKEND_BOOTSTRAP_SECRET_FILE_MAX_BYTES_V1,
        "VPN backend bootstrap secret",
    )?;
    parse_bootstrap_secret(&mut raw)
}
fn parse_bootstrap_secret(raw: &mut [u8]) -> Result<[u8; 32], BackendError> {
    let mut decoded = [0u8; 32];
    let parsed = (|| {
        if raw.len() != 64
            || !raw
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(BackendError::InvalidConfig(
                "bootstrap secret file must contain exactly 64 lowercase hexadecimal characters with no whitespace"
                    .to_owned(),
            ));
        }
        hex::decode_to_slice(&*raw, &mut decoded).map_err(|error| {
            BackendError::InvalidConfig(format!("invalid bootstrap secret file hex: {error}"))
        })?;
        if decoded.iter().all(|byte| *byte == 0) {
            return Err(BackendError::InvalidConfig(
                "bootstrap secret must not be the all-zero value".to_owned(),
            ));
        }
        Ok(())
    })();
    raw.fill(0);
    std::hint::black_box(&mut *raw);
    match parsed {
        Ok(()) => {
            let secret = decoded;
            decoded.fill(0);
            std::hint::black_box(&mut decoded);
            Ok(secret)
        }
        Err(error) => {
            decoded.fill(0);
            std::hint::black_box(&mut decoded);
            Err(error)
        }
    }
}
fn read_bounded_private_file(
    path: &std::path::Path,
    maximum: usize,
    artifact: &str,
) -> Result<Vec<u8>, BackendError> {
    let path = trusted_private_file_path(path, artifact)?;
    let before = fs::symlink_metadata(&path)?;
    validate_private_file_metadata(&before, artifact)?;
    if before.len() > u64::try_from(maximum).unwrap_or(u64::MAX) {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} exceeds the first-release {maximum}-byte limit"
        )));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&path)?;
    let opened = file.metadata()?;
    validate_private_file_metadata(&opened, artifact)?;
    if before.dev() != opened.dev()
        || before.ino() != opened.ino()
        || before.len() != opened.len()
        || before.mode() != opened.mode()
    {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} changed between inspection and open"
        )));
    }
    let expected_len = usize::try_from(opened.len()).map_err(|_| {
        BackendError::InvalidConfig(format!("{artifact} length is not representable"))
    })?;
    let mut bytes = SensitiveReadBuffer(vec![0u8; expected_len]);
    file.read_exact(&mut bytes.0)?;
    let mut growth = SensitiveReadProbe([0u8; 1]);
    if file.read(&mut growth.0)? != 0 {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} grew while being read"
        )));
    }
    let after = file.metadata()?;
    let after_path = fs::symlink_metadata(&path)?;
    validate_private_file_metadata(&after, artifact)?;
    validate_private_file_metadata(&after_path, artifact)?;
    if opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || opened.len() != after.len()
        || opened.mode() != after.mode()
        || opened.dev() != after_path.dev()
        || opened.ino() != after_path.ino()
        || opened.len() != after_path.len()
        || opened.mode() != after_path.mode()
    {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} changed while being read"
        )));
    }
    Ok(bytes.into_vec())
}
struct SensitiveReadBuffer(Vec<u8>);
impl SensitiveReadBuffer {
    fn clear(&mut self) {
        self.0.fill(0);
        std::hint::black_box(self.0.as_mut_slice());
    }
    fn into_vec(mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}
impl Drop for SensitiveReadBuffer {
    fn drop(&mut self) {
        self.clear();
    }
}
struct SensitiveReadProbe([u8; 1]);
impl SensitiveReadProbe {
    fn clear(&mut self) {
        self.0.fill(0);
        std::hint::black_box(&mut self.0);
    }
}
impl Drop for SensitiveReadProbe {
    fn drop(&mut self) {
        self.clear();
    }
}
fn trusted_private_file_path(
    path: &std::path::Path,
    artifact: &str,
) -> Result<PathBuf, BackendError> {
    if !path.is_absolute() {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} path must be absolute"
        )));
    }
    let name = path
        .file_name()
        .ok_or_else(|| BackendError::InvalidConfig(format!("{artifact} path must name a file")))?;
    let parent = path.parent().ok_or_else(|| {
        BackendError::InvalidConfig(format!("{artifact} path must have a parent directory"))
    })?;
    let canonical_parent = fs::canonicalize(parent)?;
    if canonical_parent != parent {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} parent {} must not contain symbolic links or traversal",
            parent.display()
        )));
    }
    validate_trusted_directory_chain(&canonical_parent, artifact)?;
    Ok(canonical_parent.join(name))
}
fn validate_trusted_directory_chain(
    directory: &std::path::Path,
    artifact: &str,
) -> Result<(), BackendError> {
    let effective_uid = unsafe { nix::libc::geteuid() };
    for ancestor in directory.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(BackendError::InvalidConfig(format!(
                "{artifact} parent {} must be a direct directory",
                ancestor.display()
            )));
        }
        if metadata.uid() != effective_uid && metadata.uid() != 0 {
            return Err(BackendError::InvalidConfig(format!(
                "{artifact} parent {} must be owned by the effective user or root",
                ancestor.display()
            )));
        }
        let root_owned_sticky_boundary = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
        if metadata.mode() & 0o022 != 0 && !root_owned_sticky_boundary {
            return Err(BackendError::InvalidConfig(format!(
                "{artifact} parent {} must not be group- or other-writable",
                ancestor.display()
            )));
        }
    }
    Ok(())
}
fn validate_private_file_metadata(
    metadata: &fs::Metadata,
    artifact: &str,
) -> Result<(), BackendError> {
    let effective_uid = unsafe { nix::libc::geteuid() };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} must be a direct regular file"
        )));
    }
    if metadata.uid() != effective_uid && metadata.uid() != 0 {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} must be owned by the effective user or root"
        )));
    }
    if metadata.nlink() != 1 || metadata.mode() & 0o077 != 0 {
        return Err(BackendError::InvalidConfig(format!(
            "{artifact} must have one link and no group or other permissions"
        )));
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn default_allowed_uid() -> Option<u32> {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    Some(unsafe { nix::libc::geteuid() })
}
#[cfg(not(target_os = "linux"))]
fn default_allowed_uid() -> Option<u32> {
    None
}
#[cfg(target_os = "linux")]
fn default_allowed_gid() -> Option<u32> {
    // SAFETY: getegid has no preconditions and does not dereference pointers.
    Some(unsafe { nix::libc::getegid() })
}
#[cfg(not(target_os = "linux"))]
fn default_allowed_gid() -> Option<u32> {
    None
}
#[cfg(target_os = "linux")]
fn verify_unix_peer_credentials(
    stream: &UnixStream,
    allowed_uid: Option<u32>,
    allowed_gid: Option<u32>,
) -> Result<(), BackendError> {
    let credentials = stream.peer_cred()?;
    if let Some(allowed_uid) = allowed_uid
        && credentials.uid() != allowed_uid
    {
        return Err(BackendError::InvalidConfig(format!(
            "unix backend peer uid {} is not allowed",
            credentials.uid()
        )));
    }
    if let Some(allowed_gid) = allowed_gid
        && credentials.gid() != allowed_gid
    {
        return Err(BackendError::InvalidConfig(format!(
            "unix backend peer gid {} is not allowed",
            credentials.gid()
        )));
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn verify_unix_peer_credentials(
    _stream: &UnixStream,
    allowed_uid: Option<u32>,
    allowed_gid: Option<u32>,
) -> Result<(), BackendError> {
    if allowed_uid.is_some() || allowed_gid.is_some() {
        return Err(BackendError::InvalidConfig(
            "unix peer credential checks are not supported on this platform".to_owned(),
        ));
    }
    Ok(())
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
#[norito(decode_from_slice)]
struct VpnBackendBootstrapEnvelope {
    bootstrap: VpnBackendBootstrap,
    timestamp_ms: u64,
    nonce: [u8; 16],
    mac: [u8; 32],
}
#[derive(Debug, Clone)]
struct SessionRuntimeConfig {
    interface_name: String,
    mtu: u16,
    tunnel_addresses: Vec<ParsedCidr>,
    session_routes: Vec<ParsedCidr>,
    nat_cidrs: Vec<ParsedCidr>,
    client_addresses: ClientTunnelAddresses,
}
impl SessionRuntimeConfig {
    fn from_bootstrap(
        config: &BackendConfig,
        bootstrap: VpnBackendBootstrap,
    ) -> Result<Self, BackendError> {
        validate_bootstrap_semantics(&bootstrap)?;
        let session_id_hex = bootstrap.session_id_hex.clone();
        let interface_name = derive_interface_name(&config.interface_prefix, &session_id_hex)?;
        let tunnel_addresses = parse_cidr_list(&bootstrap.server_tunnel_addresses)?;
        let session_routes = parse_cidr_list(&bootstrap.session_routes)?;
        let client_addresses = ClientTunnelAddresses {
            ipv4: Ipv4Addr::from(bootstrap.client_ipv4_address),
            ipv6: Ipv6Addr::from(bootstrap.client_ipv6_address),
        };
        let nat_cidrs = vec![
            ParsedCidr {
                address: IpAddr::V4(client_addresses.ipv4),
                prefix: 32,
            },
            ParsedCidr {
                address: IpAddr::V6(client_addresses.ipv6),
                prefix: 128,
            },
        ];
        let mtu = if bootstrap.mtu_bytes == 0 {
            config.default_mtu
        } else {
            bootstrap.mtu_bytes
        };
        Ok(Self {
            interface_name,
            mtu,
            tunnel_addresses,
            session_routes,
            nat_cidrs,
            client_addresses,
        })
    }
}
fn validate_bootstrap_semantics(bootstrap: &VpnBackendBootstrap) -> Result<(), BackendError> {
    if bootstrap.session_id_hex.len() != VPN_BACKEND_BOOTSTRAP_SESSION_ID_HEX_BYTES_V1
        || !bootstrap
            .session_id_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || bootstrap.session_id_hex.bytes().all(|byte| byte == b'0')
    {
        return Err(BackendError::InvalidConfig(format!(
            "bootstrap session_id_hex must be exactly {VPN_BACKEND_BOOTSTRAP_SESSION_ID_HEX_BYTES_V1} canonical, non-zero lowercase hexadecimal bytes"
        )));
    }
    validate_bootstrap_cidr_list(
        "server_tunnel_addresses",
        &bootstrap.server_tunnel_addresses,
        VPN_BACKEND_BOOTSTRAP_MAX_TUNNEL_ADDRESSES_V1,
    )?;
    validate_bootstrap_cidr_list(
        "session_routes",
        &bootstrap.session_routes,
        VPN_BACKEND_BOOTSTRAP_MAX_SESSION_ROUTES_V1,
    )?;
    let client_addresses = ClientTunnelAddresses {
        ipv4: Ipv4Addr::from(bootstrap.client_ipv4_address),
        ipv6: Ipv6Addr::from(bootstrap.client_ipv6_address),
    };
    if client_addresses.ipv4.is_unspecified()
        || client_addresses.ipv4.is_multicast()
        || client_addresses.ipv6.is_unspecified()
        || client_addresses.ipv6.is_multicast()
    {
        return Err(BackendError::InvalidConfig(
            "bootstrap client tunnel addresses must be exact unicast IPv4 and IPv6 addresses"
                .to_owned(),
        ));
    }
    let routes = parse_cidr_list(&bootstrap.session_routes)?;
    if !routes
        .iter()
        .any(|route| cidr_contains_ip(route, IpAddr::V4(client_addresses.ipv4)))
        || !routes
            .iter()
            .any(|route| cidr_contains_ip(route, IpAddr::V6(client_addresses.ipv6)))
    {
        return Err(BackendError::InvalidConfig(
            "bootstrap client tunnel addresses must belong to their authenticated session routes"
                .to_owned(),
        ));
    }
    Ok(())
}
fn validate_bootstrap_cidr_list(
    label: &str,
    values: &[String],
    maximum: usize,
) -> Result<(), BackendError> {
    if values.is_empty() || values.len() > maximum {
        return Err(BackendError::InvalidConfig(format!(
            "bootstrap {label} must contain 1..={maximum} entries"
        )));
    }
    let mut canonical = HashSet::with_capacity(values.len());
    for value in values {
        if value.len() > VPN_BACKEND_BOOTSTRAP_MAX_CIDR_BYTES_V1 {
            return Err(BackendError::InvalidConfig(format!(
                "bootstrap {label} entry exceeds the {VPN_BACKEND_BOOTSTRAP_MAX_CIDR_BYTES_V1}-byte limit"
            )));
        }
        let parsed = parse_cidr(value)?;
        let rendered = parsed.render();
        if rendered != *value {
            return Err(BackendError::InvalidConfig(format!(
                "bootstrap {label} entry `{value}` is not a canonical CIDR"
            )));
        }
        if !canonical.insert(rendered) {
            return Err(BackendError::InvalidConfig(format!(
                "bootstrap {label} contains a duplicate CIDR"
            )));
        }
    }
    Ok(())
}
#[derive(Debug, Clone)]
struct PreparedTunnel {
    device: Arc<LinuxTunDevice>,
    applied_network: AppliedNetworkState,
    packet_read_mtu: usize,
    client_addresses: ClientTunnelAddresses,
}
#[derive(Debug, Clone)]
struct AppliedNetworkState {
    interface_name: String,
    forwarding_leases: Vec<IpFamily>,
    nat_rules: Vec<NatRule>,
    firewall_rules: Vec<FirewallRule>,
}
#[derive(Debug, Clone)]
struct ForwardingReservation {
    previous_value: String,
    ref_count: usize,
}
#[derive(Debug, Default)]
struct SharedNetworkState {
    ipv4_forwarding: Option<ForwardingReservation>,
    ipv6_forwarding: Option<ForwardingReservation>,
}
#[derive(Debug, Clone)]
struct NatRule {
    family: IpFamily,
    source_cidr: String,
    egress_interface: String,
}
#[derive(Debug, Clone)]
struct FirewallRule {
    family: IpFamily,
    chain: &'static str,
    arguments: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClientTunnelAddresses {
    ipv4: Ipv4Addr,
    ipv6: Ipv6Addr,
}
#[derive(Debug)]
struct LinuxTunDevice {
    file: AsyncFd<fs::File>,
    name: String,
}
#[derive(Debug)]
struct PacketStreamDecoder {
    buffer: Vec<u8>,
    expected_len: Option<usize>,
    maximum_packet_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    const fn forwarding_key(self) -> &'static str {
        match self {
            Self::V4 => "net.ipv4.ip_forward",
            Self::V6 => "net.ipv6.conf.all.forwarding",
        }
    }
    const fn nat_program(self) -> &'static str {
        match self {
            Self::V4 => "iptables",
            Self::V6 => "ip6tables",
        }
    }
    const fn protected_destination_cidrs(self) -> &'static [&'static str] {
        match self {
            Self::V4 => IPV4_PROTECTED_DESTINATION_CIDRS_V1,
            Self::V6 => IPV6_PROTECTED_DESTINATION_CIDRS_V1,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCidr {
    address: IpAddr,
    prefix: u8,
}
impl ParsedCidr {
    const fn family(&self) -> IpFamily {
        match self.address {
            IpAddr::V4(_) => IpFamily::V4,
            IpAddr::V6(_) => IpFamily::V6,
        }
    }
    fn render(&self) -> String {
        format!("{}/{}", self.address, self.prefix)
    }
}
#[derive(Debug, Error)]
enum BackendError {
    #[error("invalid backend config: {0}")]
    InvalidConfig(String),
    #[error("invalid cidr: {0}")]
    InvalidCidr(String),
    #[error("invalid VPN packet: {0}")]
    InvalidPacket(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("backend state error: {0}")]
    State(String),
    #[error("{program} {arguments} failed ({status}): {detail}")]
    CommandFailed {
        program: String,
        arguments: String,
        status: String,
        exit_code: Option<i32>,
        detail: String,
    },
}
#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
async fn run(cli: Cli) -> Result<(), BackendError> {
    let config = Arc::new(BackendConfig::from_cli(cli)?);
    let shared_network = Arc::new(Mutex::new(SharedNetworkState::default()));
    let session_permits = Arc::new(Semaphore::new(VPN_BACKEND_MAX_CONCURRENT_SESSIONS_V1));
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut sessions = JoinSet::new();
    eprintln!(
        "sora-vpn-backend listening on {} with interface prefix {}",
        config.endpoint.label(),
        config.interface_prefix
    );
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let path = &config.endpoint.0;
    let accept_result = {
        ensure_unix_socket_path_available(path)?;
        let listener = UnixListener::bind(path)?;
        let mut socket_guard = UnixSocketGuard::capture(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o660))?;
        let result = loop {
            tokio::select! {
                _ = sigterm.recv() => break Ok(()),
                _ = sigint.recv() => break Ok(()),
                Some(result) = sessions.join_next(), if !sessions.is_empty() => {
                    report_session_task_result(result);
                }
                accept = listener.accept() => {
                    let (stream, _addr) = match accept {
                        Ok(accepted) => accepted,
                        Err(error) => break Err(error.into()),
                    };
                    if let Err(error) = verify_unix_peer_credentials(&stream, config.allowed_uid, config.allowed_gid) {
                        eprintln!("vpn backend rejected unix peer: {error}");
                        continue;
                    }
                    let Some(permit) = try_session_permit(&session_permits) else {
                        eprintln!("vpn backend rejected unix peer: session capacity reached");
                        continue;
                    };
                    let session_config = Arc::clone(&config);
                    let session_shared = Arc::clone(&shared_network);
                    let session_shutdown = shutdown_rx.clone();
                    let session_shutdown_requested = Arc::clone(&shutdown_requested);
                    sessions.spawn(async move {
                        let _permit = permit;
                        if let Err(error) = serve_connection(
                            stream,
                            session_config,
                            &session_shared,
                            session_shutdown,
                            session_shutdown_requested,
                        ).await {
                            eprintln!("vpn backend session from unix-peer failed: {error}");
                        }
                    });
                }
            }
        };
        drop(listener);
        match (result, socket_guard.cleanup()) {
            (result, Ok(())) => result,
            (Ok(()), Err(error)) => Err(error.into()),
            (Err(run_error), Err(cleanup_error)) => Err(BackendError::State(format!(
                "{run_error}; Unix socket cleanup failed: {cleanup_error}"
            ))),
        }
    };
    shutdown_requested.store(true, Ordering::Release);
    let _ = shutdown_tx.send(true);
    await_session_tasks(&mut sessions).await;
    accept_result
}
async fn await_session_tasks(sessions: &mut JoinSet<()>) {
    while let Some(result) = sessions.join_next().await {
        report_session_task_result(result);
    }
}
fn report_session_task_result(result: Result<(), tokio::task::JoinError>) {
    if let Err(error) = result {
        eprintln!("vpn backend session task failed: {error}");
    }
}
fn try_session_permit(semaphore: &Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
    Arc::clone(semaphore).try_acquire_owned().ok()
}
fn ensure_backend_running(shutdown_requested: &AtomicBool) -> Result<(), BackendError> {
    if shutdown_requested.load(Ordering::Acquire) {
        return Err(BackendError::State(
            "vpn backend shutdown was requested".to_owned(),
        ));
    }
    Ok(())
}
fn ensure_unix_socket_path_available(path: &std::path::Path) -> Result<(), BackendError> {
    validate_unix_socket_path(path)?;
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(BackendError::InvalidConfig(format!(
                "backend endpoint {} already exists; refusing to unlink it (remove a verified stale endpoint explicitly)",
                path.display()
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
#[derive(Debug)]
struct UnixSocketGuard {
    path: PathBuf,
    device: u64,
    inode: u64,
    armed: bool,
}
impl UnixSocketGuard {
    fn capture(path: &std::path::Path) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "created backend endpoint is not a Unix socket",
            ));
        }
        Ok(Self {
            path: path.to_path_buf(),
            device: metadata.dev(),
            inode: metadata.ino(),
            armed: true,
        })
    }
    fn cleanup(&mut self) -> io::Result<()> {
        if !self.armed {
            return Ok(());
        }
        self.armed = false;
        match fs::symlink_metadata(&self.path) {
            Ok(metadata)
                if metadata.file_type().is_socket()
                    && metadata.dev() == self.device
                    && metadata.ino() == self.inode =>
            {
                fs::remove_file(&self.path)
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}
impl Drop for UnixSocketGuard {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!(
                "failed to remove owned Unix backend endpoint {}: {error}",
                self.path.display()
            );
        }
    }
}
fn validate_unix_socket_path(path: &std::path::Path) -> Result<(), BackendError> {
    if !path.is_absolute() {
        return Err(BackendError::InvalidConfig(
            "backend Unix socket path must be absolute".to_owned(),
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        BackendError::InvalidConfig("backend Unix socket path must have a parent".to_owned())
    })?;
    let canonical_parent = fs::canonicalize(parent)?;
    if canonical_parent != parent {
        return Err(BackendError::InvalidConfig(format!(
            "backend Unix socket parent {} must not contain symlinks or traversal",
            parent.display()
        )));
    }
    validate_trusted_directory_chain(&canonical_parent, "backend Unix socket")
}
async fn serve_connection<S>(
    mut stream: S,
    config: Arc<BackendConfig>,
    shared_network: &Arc<Mutex<SharedNetworkState>>,
    mut shutdown: watch::Receiver<bool>,
    shutdown_requested: Arc<AtomicBool>,
) -> Result<(), BackendError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    ensure_backend_running(&shutdown_requested)?;
    let bootstrap = read_vpn_backend_bootstrap_until_shutdown(
        &mut stream,
        &config,
        VPN_BACKEND_BOOTSTRAP_TIMEOUT,
        &mut shutdown,
    )
    .await?;
    let session_config = match SessionRuntimeConfig::from_bootstrap(&config, bootstrap) {
        Ok(session_config) => session_config,
        Err(error) => {
            let _ = write_vpn_backend_status(&mut stream, false, &error.to_string()).await;
            return Err(error);
        }
    };
    let prepare_config = Arc::clone(&config);
    let prepare_shared_network = Arc::clone(shared_network);
    let prepare_shutdown = Arc::clone(&shutdown_requested);
    let prepared = match tokio::task::spawn_blocking(move || {
        prepare_tunnel(
            &prepare_config,
            &session_config,
            &prepare_shared_network,
            &prepare_shutdown,
        )
    })
    .await
    {
        Ok(Ok(prepared)) => prepared,
        Ok(Err(error)) => {
            let _ = write_vpn_backend_status(&mut stream, false, &error.to_string()).await;
            return Err(error);
        }
        Err(error) => {
            let error = BackendError::State(format!("VPN tunnel setup worker failed: {error}"));
            let _ = write_vpn_backend_status(&mut stream, false, &error.to_string()).await;
            return Err(error);
        }
    };
    if let Err(shutdown_error) = ensure_backend_running(&shutdown_requested) {
        let shutdown_cleanup_shared_network = Arc::clone(shared_network);
        let cleanup_result = tokio::task::spawn_blocking(move || {
            cleanup_tunnel(prepared, &shutdown_cleanup_shared_network)
        })
        .await
        .map_err(|error| BackendError::State(format!("VPN cleanup worker failed: {error}")))?;
        return match cleanup_result {
            Ok(()) => Err(shutdown_error),
            Err(cleanup_error) => Err(BackendError::State(format!(
                "{shutdown_error}; cleanup failed: {cleanup_error}"
            ))),
        };
    }
    let mut prepared = Some(prepared);
    let ready_cleanup_shared_network = Arc::clone(shared_network);
    write_ready_status_or_cleanup(&mut stream, &mut prepared, move |prepared| {
        cleanup_tunnel(prepared, &ready_cleanup_shared_network)
    })
    .await?;
    let prepared = prepared.expect("successful ready write preserves the prepared tunnel");
    eprintln!(
        "vpn backend accepted authenticated session on interface {}",
        prepared.applied_network.interface_name
    );
    let session_result = backend_packet_loop(
        Arc::clone(&prepared.device),
        stream,
        prepared.packet_read_mtu,
        prepared.client_addresses,
        shutdown,
    )
    .await;
    let cleanup_shared_network = Arc::clone(shared_network);
    let cleanup_result =
        tokio::task::spawn_blocking(move || cleanup_tunnel(prepared, &cleanup_shared_network))
            .await
            .map_err(|error| BackendError::State(format!("VPN cleanup worker failed: {error}")))?;
    match (session_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(BackendError::State(format!(
            "{error}; cleanup failed: {cleanup_error}"
        ))),
    }
}
fn prepare_tunnel(
    config: &BackendConfig,
    session_config: &SessionRuntimeConfig,
    shared_network: &Arc<Mutex<SharedNetworkState>>,
    shutdown_requested: &AtomicBool,
) -> Result<PreparedTunnel, BackendError> {
    ensure_backend_running(shutdown_requested)?;
    let ipv4_forward_egress = forwarding_egress_interface(config, IpFamily::V4)?;
    let ipv6_forward_egress = forwarding_egress_interface(config, IpFamily::V6)?;
    let device = Arc::new(LinuxTunDevice::create(&session_config.interface_name)?);
    let mut applied_network = AppliedNetworkState {
        interface_name: device.name().to_owned(),
        forwarding_leases: Vec::new(),
        nat_rules: Vec::new(),
        firewall_rules: Vec::new(),
    };
    if let Err(error) = apply_tunnel_link_config(
        &applied_network.interface_name,
        session_config.mtu,
        &session_config.tunnel_addresses,
        shutdown_requested,
    ) {
        return Err(network_setup_error(error, &applied_network, shared_network));
    }
    if let Err(error) = apply_client_routes(
        &applied_network.interface_name,
        &session_config.session_routes,
        shutdown_requested,
    ) {
        return Err(network_setup_error(error, &applied_network, shared_network));
    }
    for rule in session_firewall_rules(
        &applied_network.interface_name,
        session_config.client_addresses,
        ipv4_forward_egress,
        ipv6_forward_egress,
    ) {
        ensure_backend_running(shutdown_requested)
            .map_err(|error| network_setup_error(error, &applied_network, shared_network))?;
        match apply_firewall_rule(&rule) {
            Ok(true) => applied_network.firewall_rules.push(rule),
            Ok(false) => {}
            Err(error) => {
                return Err(network_setup_error(error, &applied_network, shared_network));
            }
        }
    }
    if config.ipv4_forward && has_family(&session_config.session_routes, IpFamily::V4) {
        match acquire_forwarding(shared_network, IpFamily::V4, shutdown_requested) {
            Ok(()) => applied_network.forwarding_leases.push(IpFamily::V4),
            Err(error) => {
                return Err(network_setup_error(error, &applied_network, shared_network));
            }
        }
    }
    if config.ipv6_forward && has_family(&session_config.session_routes, IpFamily::V6) {
        match acquire_forwarding(shared_network, IpFamily::V6, shutdown_requested) {
            Ok(()) => applied_network.forwarding_leases.push(IpFamily::V6),
            Err(error) => {
                return Err(network_setup_error(error, &applied_network, shared_network));
            }
        }
    }
    if config.enable_ipv4_nat {
        let Some(egress_interface) = config.egress_v4_interface.as_ref() else {
            return Err(network_setup_error(
                BackendError::InvalidConfig(
                    "IPv4 NAT enabled without a resolved egress interface".to_owned(),
                ),
                &applied_network,
                shared_network,
            ));
        };
        for cidr in session_config
            .nat_cidrs
            .iter()
            .filter(|cidr| cidr.family() == IpFamily::V4)
        {
            match apply_nat_rule(
                IpFamily::V4,
                &cidr.render(),
                egress_interface,
                shutdown_requested,
            ) {
                Ok(Some(rule)) => applied_network.nat_rules.push(rule),
                Ok(None) => {}
                Err(error) => {
                    return Err(network_setup_error(error, &applied_network, shared_network));
                }
            }
        }
    }
    if config.enable_ipv6_nat {
        let Some(egress_interface) = config.egress_v6_interface.as_ref() else {
            return Err(network_setup_error(
                BackendError::InvalidConfig(
                    "IPv6 NAT enabled without a resolved egress interface".to_owned(),
                ),
                &applied_network,
                shared_network,
            ));
        };
        for cidr in session_config
            .nat_cidrs
            .iter()
            .filter(|cidr| cidr.family() == IpFamily::V6)
        {
            match apply_nat_rule(
                IpFamily::V6,
                &cidr.render(),
                egress_interface,
                shutdown_requested,
            ) {
                Ok(Some(rule)) => applied_network.nat_rules.push(rule),
                Ok(None) => {}
                Err(error) => {
                    return Err(network_setup_error(error, &applied_network, shared_network));
                }
            }
        }
    }
    if let Err(error) = ensure_backend_running(shutdown_requested) {
        return Err(network_setup_error(error, &applied_network, shared_network));
    }
    Ok(PreparedTunnel {
        device,
        applied_network,
        packet_read_mtu: usize::from(session_config.mtu),
        client_addresses: session_config.client_addresses,
    })
}
fn network_setup_error(
    setup_error: BackendError,
    applied: &AppliedNetworkState,
    shared_network: &Arc<Mutex<SharedNetworkState>>,
) -> BackendError {
    match cleanup_network(applied, shared_network) {
        Ok(()) => setup_error,
        Err(cleanup_error) => BackendError::State(format!(
            "{setup_error}; cleanup also failed: {cleanup_error}"
        )),
    }
}
fn cleanup_tunnel(
    prepared: PreparedTunnel,
    shared_network: &Arc<Mutex<SharedNetworkState>>,
) -> Result<(), BackendError> {
    cleanup_network(&prepared.applied_network, shared_network)?;
    drop(prepared);
    Ok(())
}
fn cleanup_network(
    applied: &AppliedNetworkState,
    shared_network: &Arc<Mutex<SharedNetworkState>>,
) -> Result<(), BackendError> {
    cleanup_network_with(
        applied,
        remove_nat_rule,
        remove_firewall_rule,
        |family| match release_forwarding(shared_network, family) {
            Ok(()) => Ok(()),
            Err(first_error) => release_forwarding(shared_network, family).map_err(|retry_error| {
                BackendError::State(format!(
                    "initial forwarding restore failed: {first_error}; retry failed: {retry_error}"
                ))
            }),
        },
        |interface_name| {
            let result = run_command(
                DEFAULT_ROUTE_CMD,
                vec![
                    "link".to_owned(),
                    "set".to_owned(),
                    "dev".to_owned(),
                    interface_name.to_owned(),
                    "down".to_owned(),
                ],
            );
            match result {
                Ok(_) => Ok(()),
                Err(BackendError::CommandFailed { detail, .. })
                    if detail.contains("Cannot find device")
                        || detail.contains("does not exist") =>
                {
                    Ok(())
                }
                Err(error) => Err(error),
            }
        },
    )
}
fn cleanup_network_with<FN, FW, FF, FD>(
    applied: &AppliedNetworkState,
    mut remove_nat: FN,
    mut remove_firewall: FW,
    mut release_forwarding_lease: FF,
    mut bring_link_down: FD,
) -> Result<(), BackendError>
where
    FN: FnMut(&NatRule) -> Result<(), BackendError>,
    FW: FnMut(&FirewallRule) -> Result<(), BackendError>,
    FF: FnMut(IpFamily) -> Result<(), BackendError>,
    FD: FnMut(&str) -> Result<(), BackendError>,
{
    let mut failures = Vec::new();
    for rule in applied.nat_rules.iter().rev() {
        if let Err(error) = remove_nat(rule) {
            failures.push(format!("NAT rollback failed: {error}"));
        }
    }
    for family in applied.forwarding_leases.iter().rev() {
        if let Err(error) = release_forwarding_lease(*family) {
            failures.push(format!("forwarding rollback failed: {error}"));
        }
    }
    for rule in applied.firewall_rules.iter().rev() {
        if let Err(error) = remove_firewall(rule) {
            failures.push(format!("firewall rollback failed: {error}"));
        }
    }
    if let Err(error) = bring_link_down(&applied.interface_name) {
        failures.push(format!("link rollback failed: {error}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(BackendError::State(format!(
            "VPN network cleanup encountered {} failure(s): {}",
            failures.len(),
            failures.join("; ")
        )))
    }
}
async fn backend_packet_loop<S>(
    device: Arc<LinuxTunDevice>,
    stream: S,
    packet_read_mtu: usize,
    client_addresses: ClientTunnelAddresses,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), BackendError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let upstream = tun_to_socket_loop(
        Arc::clone(&device),
        &mut writer,
        packet_read_mtu,
        client_addresses,
    );
    let downstream = socket_to_tun_loop(device, &mut reader, packet_read_mtu, client_addresses);
    tokio::pin!(upstream);
    tokio::pin!(downstream);
    tokio::select! {
        () = wait_for_backend_shutdown(&mut shutdown) => Ok(()),
        result = &mut upstream => result,
        result = &mut downstream => result,
    }
}
fn unix_time_ms() -> Result<u64, BackendError> {
    unix_time_ms_at(SystemTime::now())
}
fn unix_time_ms_at(now: SystemTime) -> Result<u64, BackendError> {
    let duration = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| BackendError::State("system clock is before the Unix epoch".to_owned()))?;
    u64::try_from(duration.as_millis())
        .map_err(|_| BackendError::State("system clock exceeds u64 milliseconds".to_owned()))
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
fn bootstrap_mac_matches(expected: &[u8; 32], candidate: &[u8; 32]) -> bool {
    blake3::Hash::from_bytes(*expected) == blake3::Hash::from_bytes(*candidate)
}
impl DurableBootstrapReplay {
    fn open(directory: &std::path::Path, now_ms: u64, now: Instant) -> Result<Self, BackendError> {
        let directory = prepare_private_replay_directory(directory)?;
        let lock_path = directory.join(VPN_BACKEND_REPLAY_LOCK_FILE);
        let lock = open_private_replay_file(&lock_path, true, "bootstrap replay lock")?;
        let lock_result =
            unsafe { nix::libc::flock(lock.as_raw_fd(), nix::libc::LOCK_EX | nix::libc::LOCK_NB) };
        if lock_result != 0 {
            return Err(BackendError::State(format!(
                "failed to lock bootstrap replay directory {}: {}",
                directory.display(),
                io::Error::last_os_error()
            )));
        }
        let high_water_path = directory.join(VPN_BACKEND_REPLAY_HIGH_WATER_FILE);
        let mut high_water_file = open_private_replay_file(
            &high_water_path,
            true,
            "bootstrap replay time high-water file",
        )?;
        let high_water_ms = load_or_initialize_replay_high_water(&mut high_water_file, now_ms)?;
        if now_ms < high_water_ms {
            return Err(BackendError::State(format!(
                "system clock {now_ms}ms is behind the durable bootstrap replay high-water mark {high_water_ms}ms"
            )));
        }
        if now_ms > high_water_ms {
            write_replay_high_water(&mut high_water_file, now_ms)?;
        }

        let mut active = Vec::new();
        let mut expired = Vec::new();
        let mut scanned = 0usize;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(BackendError::State(
                    "bootstrap replay directory contains a non-UTF-8 entry".to_owned(),
                ));
            };
            if matches!(
                name,
                VPN_BACKEND_REPLAY_LOCK_FILE | VPN_BACKEND_REPLAY_HIGH_WATER_FILE
            ) {
                continue;
            }
            scanned = scanned.checked_add(1).ok_or_else(|| {
                BackendError::State("bootstrap replay entry count overflow".to_owned())
            })?;
            if scanned > VPN_BACKEND_BOOTSTRAP_NONCE_CACHE_CAPACITY_V1 {
                return Err(BackendError::State(format!(
                    "bootstrap replay directory exceeds its first-release capacity of {VPN_BACKEND_BOOTSTRAP_NONCE_CACHE_CAPACITY_V1} entries"
                )));
            }
            let (nonce, expires_at_ms) = parse_replay_entry_name(name)?;
            let metadata = fs::symlink_metadata(entry.path())?;
            validate_replay_entry_metadata(&metadata, "bootstrap replay entry")?;
            if metadata.len() != 0 {
                return Err(BackendError::State(format!(
                    "bootstrap replay entry {} must be empty",
                    entry.path().display()
                )));
            }
            // The timestamp corridor is inclusive at both skew edges. A frame
            // first accepted at the future edge remains valid for replay at
            // exactly `2 * MAX_SKEW`, so retain its nonce through the exact
            // expiry millisecond and remove it only after that boundary.
            if expires_at_ms < now_ms {
                expired.push(entry.path());
            } else {
                let maximum_expiry = now_ms
                    .checked_add(
                        u64::try_from(VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION.as_millis()).map_err(
                            |_| {
                                BackendError::State(
                                    "bootstrap replay retention exceeds u64 milliseconds"
                                        .to_owned(),
                                )
                            },
                        )?,
                    )
                    .ok_or_else(|| {
                        BackendError::State("bootstrap replay expiry overflow".to_owned())
                    })?;
                if expires_at_ms > maximum_expiry {
                    return Err(BackendError::State(format!(
                        "bootstrap replay entry {name:?} exceeds the retention window"
                    )));
                }
                active.push((expires_at_ms, nonce));
            }
        }
        for path in &expired {
            fs::remove_file(path)?;
        }
        if !expired.is_empty() {
            sync_replay_directory(&directory)?;
        }
        active.sort_unstable_by_key(|(expires_at_ms, nonce)| (*expires_at_ms, *nonce));
        let mut seen = SeenBootstrapNonces::default();
        seen.nonces.try_reserve(active.len()).map_err(|error| {
            BackendError::State(format!("failed to reserve bootstrap replay cache: {error}"))
        })?;
        seen.receipts.try_reserve(active.len()).map_err(|error| {
            BackendError::State(format!("failed to reserve bootstrap replay queue: {error}"))
        })?;
        for (expires_at_ms, nonce) in active {
            if !seen.nonces.insert(nonce) {
                return Err(BackendError::State(format!(
                    "bootstrap replay directory contains duplicate nonce {}",
                    hex::encode(nonce)
                )));
            }
            let remaining = Duration::from_millis(expires_at_ms.saturating_sub(now_ms))
                .min(VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION);
            let age = VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION.saturating_sub(remaining);
            let received_at = now.checked_sub(age).unwrap_or(now);
            seen.receipts.push_back((received_at, nonce, expires_at_ms));
        }
        Ok(Self {
            seen,
            directory,
            _lock: lock,
            high_water_file,
            high_water_ms: now_ms,
            durability_failed: false,
        })
    }

    fn admit(
        &mut self,
        nonce: [u8; 16],
        received_at: Instant,
        now_ms: u64,
    ) -> Result<(), BackendError> {
        if self.durability_failed {
            return Err(BackendError::State(
                "bootstrap replay persistence previously failed; refusing further sessions"
                    .to_owned(),
            ));
        }
        let result = self.admit_inner(nonce, received_at, now_ms);
        if result.is_err() {
            // Authentication errors do not poison storage, but every I/O or time-custody failure
            // must fail all later admissions rather than risk a replay after restart.
            if matches!(&result, Err(BackendError::Io(_) | BackendError::State(_))) {
                self.durability_failed = true;
            }
        }
        result
    }

    fn admit_inner(
        &mut self,
        nonce: [u8; 16],
        received_at: Instant,
        now_ms: u64,
    ) -> Result<(), BackendError> {
        if now_ms < self.high_water_ms {
            return Err(BackendError::State(format!(
                "system clock {now_ms}ms moved behind the bootstrap replay high-water mark {}ms",
                self.high_water_ms
            )));
        }
        if now_ms > self.high_water_ms {
            write_replay_high_water(&mut self.high_water_file, now_ms)?;
            self.high_water_ms = now_ms;
        }
        let retention = VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION;
        let mut removed = false;
        while self
            .seen
            .receipts
            .front()
            .is_some_and(|(prior_receipt, _, _)| {
                received_at.saturating_duration_since(*prior_receipt) > retention
            })
        {
            let (_, expired_nonce, expires_at_ms) = self
                .seen
                .receipts
                .front()
                .copied()
                .expect("front entry checked above");
            let path = replay_entry_path(&self.directory, expired_nonce, expires_at_ms);
            validate_and_remove_replay_entry(&path)?;
            self.seen.receipts.pop_front();
            self.seen.nonces.remove(&expired_nonce);
            removed = true;
        }
        if removed {
            sync_replay_directory(&self.directory)?;
        }
        if self.seen.nonces.contains(&nonce) {
            return Err(BackendError::InvalidConfig(
                "vpn backend bootstrap nonce was replayed".to_owned(),
            ));
        }
        if self.seen.nonces.len() >= VPN_BACKEND_BOOTSTRAP_NONCE_CACHE_CAPACITY_V1 {
            return Err(BackendError::State(format!(
                "vpn backend bootstrap nonce cache reached its first-release capacity of {VPN_BACKEND_BOOTSTRAP_NONCE_CACHE_CAPACITY_V1}"
            )));
        }
        self.seen.nonces.try_reserve(1).map_err(|error| {
            BackendError::State(format!(
                "failed to reserve bounded bootstrap nonce cache storage: {error}"
            ))
        })?;
        self.seen.receipts.try_reserve(1).map_err(|error| {
            BackendError::State(format!(
                "failed to reserve bounded bootstrap nonce expiry storage: {error}"
            ))
        })?;
        let retention_ms = u64::try_from(retention.as_millis()).map_err(|_| {
            BackendError::State(
                "bootstrap replay retention does not fit u64 milliseconds".to_owned(),
            )
        })?;
        let expires_at_ms = now_ms
            .checked_add(retention_ms)
            .ok_or_else(|| BackendError::State("bootstrap replay expiry overflow".to_owned()))?;
        let path = replay_entry_path(&self.directory, nonce, expires_at_ms);
        let entry = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(&path)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    BackendError::InvalidConfig(
                        "vpn backend bootstrap nonce was already durably recorded".to_owned(),
                    )
                } else {
                    error.into()
                }
            })?;
        validate_replay_entry_metadata(&entry.metadata()?, "bootstrap replay entry")?;
        entry.sync_all()?;
        sync_replay_directory(&self.directory)?;
        self.seen.nonces.insert(nonce);
        self.seen
            .receipts
            .push_back((received_at, nonce, expires_at_ms));
        Ok(())
    }
}

fn prepare_private_replay_directory(path: &std::path::Path) -> Result<PathBuf, BackendError> {
    if !path.is_absolute() {
        return Err(BackendError::InvalidConfig(
            "bootstrap replay directory must be absolute".to_owned(),
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        BackendError::InvalidConfig("bootstrap replay directory has no parent".to_owned())
    })?;
    let canonical_parent = fs::canonicalize(parent)?;
    if canonical_parent != parent {
        return Err(BackendError::InvalidConfig(
            "bootstrap replay directory parent must not contain symlinks or traversal".to_owned(),
        ));
    }
    validate_trusted_directory_chain(&canonical_parent, "bootstrap replay directory")?;
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(path)?;
            sync_replay_directory(&canonical_parent)?;
        }
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    let effective_uid = unsafe { nix::libc::geteuid() };
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != effective_uid
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(BackendError::InvalidConfig(format!(
            "bootstrap replay directory {} must be a direct, effective-user-owned directory with mode 0700",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err(BackendError::InvalidConfig(
            "bootstrap replay directory must not be reached through a symlink".to_owned(),
        ));
    }
    Ok(canonical)
}

fn open_private_replay_file(
    path: &std::path::Path,
    create: bool,
    label: &str,
) -> Result<File, BackendError> {
    let before = fs::symlink_metadata(path).ok();
    if let Some(metadata) = &before {
        validate_replay_entry_metadata(metadata, label)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)?;
    let opened = file.metadata()?;
    validate_replay_entry_metadata(&opened, label)?;
    if let Some(before) = before
        && (before.dev() != opened.dev() || before.ino() != opened.ino())
    {
        return Err(BackendError::State(format!(
            "{label} changed identity while opening"
        )));
    }
    Ok(file)
}

fn validate_replay_entry_metadata(
    metadata: &fs::Metadata,
    label: &str,
) -> Result<(), BackendError> {
    let effective_uid = unsafe { nix::libc::geteuid() };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != effective_uid
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err(BackendError::InvalidConfig(format!(
            "{label} must be a direct, single-link, effective-user-owned regular file with mode 0600"
        )));
    }
    Ok(())
}

fn load_or_initialize_replay_high_water(file: &mut File, now_ms: u64) -> Result<u64, BackendError> {
    let length = file.metadata()?.len();
    if length == 0 {
        write_replay_high_water(file, now_ms)?;
        return Ok(now_ms);
    }
    if length != 8 {
        return Err(BackendError::State(
            "bootstrap replay time high-water file has an invalid length".to_owned(),
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = [0_u8; 8];
    file.read_exact(&mut bytes)?;
    Ok(u64::from_be_bytes(bytes))
}

fn write_replay_high_water(file: &mut File, now_ms: u64) -> Result<(), BackendError> {
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&now_ms.to_be_bytes())?;
    file.set_len(8)?;
    file.sync_all()?;
    Ok(())
}

fn replay_entry_path(directory: &std::path::Path, nonce: [u8; 16], expiry_ms: u64) -> PathBuf {
    directory.join(format!(
        "{}.{expiry_ms}{VPN_BACKEND_REPLAY_ENTRY_SUFFIX}",
        hex::encode(nonce)
    ))
}

fn parse_replay_entry_name(name: &str) -> Result<([u8; 16], u64), BackendError> {
    let body = name
        .strip_suffix(VPN_BACKEND_REPLAY_ENTRY_SUFFIX)
        .ok_or_else(|| BackendError::State(format!("unknown bootstrap replay entry {name:?}")))?;
    let (nonce_hex, expiry) = body
        .rsplit_once('.')
        .ok_or_else(|| BackendError::State(format!("malformed bootstrap replay entry {name:?}")))?;
    if nonce_hex.len() != 32
        || !nonce_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(BackendError::State(format!(
            "malformed bootstrap replay nonce in {name:?}"
        )));
    }
    let nonce: [u8; 16] = hex::decode(nonce_hex)
        .map_err(|error| BackendError::State(format!("invalid replay nonce: {error}")))?
        .try_into()
        .map_err(|_| BackendError::State("invalid bootstrap replay nonce length".to_owned()))?;
    let expires_at_ms = expiry.parse::<u64>().map_err(|_| {
        BackendError::State(format!("malformed bootstrap replay expiry in {name:?}"))
    })?;
    if expiry != expires_at_ms.to_string() {
        return Err(BackendError::State(format!(
            "non-canonical bootstrap replay expiry in {name:?}"
        )));
    }
    Ok((nonce, expires_at_ms))
}

fn validate_and_remove_replay_entry(path: &std::path::Path) -> Result<(), BackendError> {
    let metadata = fs::symlink_metadata(path)?;
    validate_replay_entry_metadata(&metadata, "bootstrap replay entry")?;
    if metadata.len() != 0 {
        return Err(BackendError::State(format!(
            "bootstrap replay entry {} must be empty",
            path.display()
        )));
    }
    fs::remove_file(path)?;
    Ok(())
}

fn sync_replay_directory(directory: &std::path::Path) -> Result<(), BackendError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_DIRECTORY | nix::libc::O_CLOEXEC)
        .open(directory)?;
    file.sync_all()?;
    Ok(())
}
async fn read_vpn_backend_bootstrap<R: AsyncRead + Unpin>(
    reader: &mut R,
    config: &BackendConfig,
) -> Result<VpnBackendBootstrap, BackendError> {
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic).await?;
    if &magic != VPN_BACKEND_BOOTSTRAP_MAGIC {
        return Err(BackendError::InvalidConfig(
            "vpn backend bootstrap magic prefix is invalid".to_owned(),
        ));
    }
    let mut len = [0u8; 2];
    reader.read_exact(&mut len).await?;
    let len = usize::from(u16::from_be_bytes(len));
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    let envelope: VpnBackendBootstrapEnvelope = norito::codec::decode_exact_from_slice_with_limits(
        &payload,
        VPN_BACKEND_BOOTSTRAP_DECODE_LIMITS_V1,
    )
    .map_err(|error| BackendError::InvalidConfig(format!("invalid backend bootstrap: {error}")))?;
    validate_bootstrap_semantics(&envelope.bootstrap)?;
    let now_ms = unix_time_ms()?;
    let age = now_ms.abs_diff(envelope.timestamp_ms);
    if age > VPN_BACKEND_BOOTSTRAP_MAX_SKEW_MS {
        return Err(BackendError::InvalidConfig(
            "vpn backend bootstrap timestamp is stale".to_owned(),
        ));
    }
    let expected = vpn_backend_bootstrap_mac(
        &config.bootstrap_secret,
        &envelope.bootstrap,
        envelope.timestamp_ms,
        &envelope.nonce,
    );
    if !bootstrap_mac_matches(&expected, &envelope.mac) {
        return Err(BackendError::InvalidConfig(
            "vpn backend bootstrap MAC is invalid".to_owned(),
        ));
    }
    let mut replay = config
        .bootstrap_replay
        .lock()
        .map_err(|_| BackendError::State("bootstrap nonce cache poisoned".to_owned()))?;
    replay.admit(envelope.nonce, Instant::now(), now_ms)?;
    Ok(envelope.bootstrap)
}
async fn read_vpn_backend_bootstrap_with_deadline<R: AsyncRead + Unpin>(
    reader: &mut R,
    config: &BackendConfig,
    deadline: Duration,
) -> Result<VpnBackendBootstrap, BackendError> {
    timeout(deadline, read_vpn_backend_bootstrap(reader, config))
        .await
        .map_err(|_| BackendError::State("vpn backend bootstrap timed out".to_owned()))?
}
async fn read_vpn_backend_bootstrap_until_shutdown<R: AsyncRead + Unpin>(
    reader: &mut R,
    config: &BackendConfig,
    deadline: Duration,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<VpnBackendBootstrap, BackendError> {
    tokio::select! {
        result = read_vpn_backend_bootstrap_with_deadline(reader, config, deadline) => result,
        () = wait_for_backend_shutdown(shutdown) => Err(BackendError::State(
            "vpn backend shutdown was requested during bootstrap".to_owned(),
        )),
    }
}
async fn wait_for_backend_shutdown(shutdown: &mut watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow_and_update() {
            return;
        }
        if shutdown.changed().await.is_err() {
            return;
        }
    }
}
async fn write_vpn_backend_status<W: AsyncWrite + Unpin>(
    writer: &mut W,
    ready: bool,
    message: &str,
) -> Result<(), BackendError> {
    let payload = message.as_bytes();
    let len = u16::try_from(payload.len()).map_err(|_| {
        BackendError::State(format!(
            "vpn backend status payload {} exceeds u16 length prefix",
            payload.len()
        ))
    })?;
    timeout(VPN_BACKEND_SOCKET_WRITE_TIMEOUT, async {
        writer
            .write_all(&[if ready { VPN_BACKEND_STATUS_READY } else { 0u8 }])
            .await?;
        writer.write_all(&len.to_be_bytes()).await?;
        writer.write_all(payload).await
    })
    .await
    .map_err(|_| BackendError::State("vpn backend status write timed out".to_owned()))??;
    Ok(())
}
async fn write_ready_status_or_cleanup<W, T, F>(
    writer: &mut W,
    prepared: &mut Option<T>,
    cleanup: F,
) -> Result<(), BackendError>
where
    W: AsyncWrite + Unpin,
    T: Send + 'static,
    F: FnOnce(T) -> Result<(), BackendError> + Send + 'static,
{
    let Err(write_error) = write_vpn_backend_status(writer, true, "ready").await else {
        return Ok(());
    };
    let Some(prepared) = prepared.take() else {
        return Err(BackendError::State(format!(
            "{write_error}; prepared tunnel was unavailable for cleanup"
        )));
    };
    match tokio::task::spawn_blocking(move || cleanup(prepared)).await {
        Ok(Ok(())) => Err(write_error),
        Ok(Err(cleanup_error)) => Err(BackendError::State(format!(
            "{write_error}; cleanup failed: {cleanup_error}"
        ))),
        Err(cleanup_error) => Err(BackendError::State(format!(
            "{write_error}; cleanup worker failed: {cleanup_error}"
        ))),
    }
}
async fn tun_to_socket_loop<W: AsyncWriteExt + Unpin>(
    device: Arc<LinuxTunDevice>,
    writer: &mut W,
    packet_read_mtu: usize,
    client_addresses: ClientTunnelAddresses,
) -> Result<(), BackendError> {
    let mut packet_buf = vec![0u8; packet_read_mtu.max(512)];
    loop {
        let packet_len = device.recv(&mut packet_buf).await?;
        if packet_len == 0 {
            continue;
        }
        validate_vpn_packet(
            &packet_buf[..packet_len],
            client_addresses,
            PacketDirection::BackendToClient,
        )?;
        let encoded = encode_packet_stream_frame(&packet_buf[..packet_len])?;
        timeout(VPN_BACKEND_SOCKET_WRITE_TIMEOUT, writer.write_all(&encoded))
            .await
            .map_err(|_| BackendError::State("vpn backend socket write timed out".to_owned()))??;
    }
}
async fn socket_to_tun_loop<R: AsyncReadExt + Unpin>(
    device: Arc<LinuxTunDevice>,
    reader: &mut R,
    maximum_packet_bytes: usize,
    client_addresses: ClientTunnelAddresses,
) -> Result<(), BackendError> {
    let mut decoder = PacketStreamDecoder::new(maximum_packet_bytes);
    let mut buf = vec![0u8; 4096];
    loop {
        let read = timeout(VPN_BACKEND_SOCKET_IDLE_TIMEOUT, reader.read(&mut buf))
            .await
            .map_err(|_| BackendError::State("vpn backend socket idle timeout".to_owned()))??;
        if read == 0 {
            return Ok(());
        }
        for packet in decoder.ingest(&buf[..read])? {
            validate_vpn_packet(&packet, client_addresses, PacketDirection::ClientToBackend)?;
            let written = device.send(&packet).await?;
            if written != packet.len() {
                return Err(BackendError::State(format!(
                    "vpn backend TUN write accepted {written} of {} packet bytes",
                    packet.len()
                )));
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketDirection {
    ClientToBackend,
    BackendToClient,
}
fn validate_vpn_packet(
    packet: &[u8],
    client_addresses: ClientTunnelAddresses,
    direction: PacketDirection,
) -> Result<(), BackendError> {
    let Some(version) = packet.first().map(|byte| byte >> 4) else {
        return Err(BackendError::InvalidPacket("packet is empty".to_owned()));
    };
    match version {
        4 => validate_ipv4_packet(packet, client_addresses.ipv4, direction),
        6 => validate_ipv6_packet(packet, client_addresses.ipv6, direction),
        _ => Err(BackendError::InvalidPacket(format!(
            "IP version nibble {version} is unsupported"
        ))),
    }
}
fn validate_ipv4_packet(
    packet: &[u8],
    client_address: Ipv4Addr,
    direction: PacketDirection,
) -> Result<(), BackendError> {
    const IPV4_HEADER_BYTES: usize = 20;
    if packet.len() < IPV4_HEADER_BYTES {
        return Err(BackendError::InvalidPacket(
            "IPv4 packet is shorter than its minimum header".to_owned(),
        ));
    }
    let header_bytes = usize::from(packet[0] & 0x0f) * 4;
    if header_bytes != IPV4_HEADER_BYTES {
        return Err(BackendError::InvalidPacket(format!(
            "IPv4 IHL must be exactly {IPV4_HEADER_BYTES} bytes in the V1 corridor (got {header_bytes})"
        )));
    }
    let total_length = usize::from(u16::from_be_bytes([packet[2], packet[3]]));
    if total_length != packet.len() {
        return Err(BackendError::InvalidPacket(format!(
            "IPv4 total length {total_length} does not match packet frame length {}",
            packet.len()
        )));
    }
    let fragment = u16::from_be_bytes([packet[6], packet[7]]);
    if fragment & 0x8000 != 0 {
        return Err(BackendError::InvalidPacket(
            "IPv4 reserved fragment flag is set".to_owned(),
        ));
    }
    if fragment & 0x3fff != 0 {
        return Err(BackendError::InvalidPacket(
            "fragmented IPv4 packets are outside the V1 anti-spoof corridor".to_owned(),
        ));
    }
    if !ipv4_header_checksum_valid(&packet[..header_bytes]) {
        return Err(BackendError::InvalidPacket(
            "IPv4 header checksum is invalid".to_owned(),
        ));
    }
    let source = Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]);
    let destination = Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]);
    validate_packet_endpoint(
        IpAddr::V4(source),
        IpAddr::V4(destination),
        IpAddr::V4(client_address),
        direction,
    )
}
fn ipv4_header_checksum_valid(header: &[u8]) -> bool {
    let mut sum = 0u32;
    for word in header.chunks_exact(2) {
        sum = sum.saturating_add(u32::from(u16::from_be_bytes([word[0], word[1]])));
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    sum == 0xffff
}
fn validate_ipv6_packet(
    packet: &[u8],
    client_address: Ipv6Addr,
    direction: PacketDirection,
) -> Result<(), BackendError> {
    const IPV6_HEADER_BYTES: usize = 40;
    if packet.len() < IPV6_HEADER_BYTES {
        return Err(BackendError::InvalidPacket(
            "IPv6 packet is shorter than its fixed header".to_owned(),
        ));
    }
    let payload_length = usize::from(u16::from_be_bytes([packet[4], packet[5]]));
    let total_length = IPV6_HEADER_BYTES
        .checked_add(payload_length)
        .ok_or_else(|| {
            BackendError::InvalidPacket("IPv6 payload length overflowed packet size".to_owned())
        })?;
    if total_length != packet.len() {
        return Err(BackendError::InvalidPacket(format!(
            "IPv6 payload length {payload_length} does not match packet frame length {}",
            packet.len()
        )));
    }
    validate_ipv6_extension_chain(packet, packet[6])?;
    let mut source = [0u8; 16];
    source.copy_from_slice(&packet[8..24]);
    let mut destination = [0u8; 16];
    destination.copy_from_slice(&packet[24..40]);
    validate_packet_endpoint(
        IpAddr::V6(Ipv6Addr::from(source)),
        IpAddr::V6(Ipv6Addr::from(destination)),
        IpAddr::V6(client_address),
        direction,
    )
}
fn validate_ipv6_extension_chain(packet: &[u8], mut next_header: u8) -> Result<(), BackendError> {
    let mut offset = 40usize;
    for _ in 0..8 {
        match next_header {
            44 => {
                return Err(BackendError::InvalidPacket(
                    "IPv6 fragment headers are outside the V1 anti-spoof corridor".to_owned(),
                ));
            }
            43 => {
                return Err(BackendError::InvalidPacket(
                    "IPv6 routing headers are outside the V1 packet corridor".to_owned(),
                ));
            }
            0 | 60 | 135 | 139 | 140 => {
                if packet.len().saturating_sub(offset) < 2 {
                    return Err(BackendError::InvalidPacket(
                        "IPv6 extension header is truncated".to_owned(),
                    ));
                }
                let extension_length = (usize::from(packet[offset + 1]) + 1)
                    .checked_mul(8)
                    .ok_or_else(|| {
                        BackendError::InvalidPacket(
                            "IPv6 extension header length overflowed".to_owned(),
                        )
                    })?;
                next_header = packet[offset];
                offset = offset.checked_add(extension_length).ok_or_else(|| {
                    BackendError::InvalidPacket("IPv6 extension chain overflowed".to_owned())
                })?;
                if offset > packet.len() {
                    return Err(BackendError::InvalidPacket(
                        "IPv6 extension header exceeds packet length".to_owned(),
                    ));
                }
            }
            51 => {
                if packet.len().saturating_sub(offset) < 2 {
                    return Err(BackendError::InvalidPacket(
                        "IPv6 authentication header is truncated".to_owned(),
                    ));
                }
                let extension_length = (usize::from(packet[offset + 1]) + 2)
                    .checked_mul(4)
                    .ok_or_else(|| {
                        BackendError::InvalidPacket(
                            "IPv6 authentication header length overflowed".to_owned(),
                        )
                    })?;
                next_header = packet[offset];
                offset = offset.checked_add(extension_length).ok_or_else(|| {
                    BackendError::InvalidPacket("IPv6 extension chain overflowed".to_owned())
                })?;
                if offset > packet.len() {
                    return Err(BackendError::InvalidPacket(
                        "IPv6 authentication header exceeds packet length".to_owned(),
                    ));
                }
            }
            59 if offset != packet.len() => {
                return Err(BackendError::InvalidPacket(
                    "IPv6 no-next-header packet carries trailing payload".to_owned(),
                ));
            }
            _ => return Ok(()),
        }
    }
    Err(BackendError::InvalidPacket(
        "IPv6 extension chain exceeds the V1 depth limit".to_owned(),
    ))
}
fn validate_packet_endpoint(
    source: IpAddr,
    destination: IpAddr,
    client_address: IpAddr,
    direction: PacketDirection,
) -> Result<(), BackendError> {
    let (observed, label) = match direction {
        PacketDirection::ClientToBackend => (source, "source"),
        PacketDirection::BackendToClient => (destination, "destination"),
    };
    if observed != client_address {
        return Err(BackendError::InvalidPacket(format!(
            "packet {label} {observed} does not match assigned client address {client_address}"
        )));
    }
    if direction == PacketDirection::ClientToBackend
        && is_protected_public_exit_destination(destination)
    {
        return Err(BackendError::InvalidPacket(format!(
            "public-exit packet destination {destination} belongs to a protected local, private, special-use, or non-unicast range"
        )));
    }
    Ok(())
}

fn is_protected_public_exit_destination(address: IpAddr) -> bool {
    let family = match address {
        IpAddr::V4(_) => IpFamily::V4,
        IpAddr::V6(_) => IpFamily::V6,
    };
    protected_destination_cidrs(family)
        .iter()
        .any(|cidr| cidr_contains_ip(cidr, address))
}

fn protected_destination_cidrs(family: IpFamily) -> &'static [ParsedCidr] {
    static IPV4: OnceLock<Vec<ParsedCidr>> = OnceLock::new();
    static IPV6: OnceLock<Vec<ParsedCidr>> = OnceLock::new();
    let (slot, values) = match family {
        IpFamily::V4 => (&IPV4, IPV4_PROTECTED_DESTINATION_CIDRS_V1),
        IpFamily::V6 => (&IPV6, IPV6_PROTECTED_DESTINATION_CIDRS_V1),
    };
    slot.get_or_init(|| {
        values
            .iter()
            .map(|value| {
                parse_cidr(value).expect("static protected public-exit CIDRs must be canonical")
            })
            .collect()
    })
}
impl PacketStreamDecoder {
    fn new(maximum_packet_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            expected_len: None,
            maximum_packet_bytes,
        }
    }
    fn ingest(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, BackendError> {
        if self.expected_len.is_none() {
            if self.buffer.is_empty() && bytes.len() >= PACKET_LEN_PREFIX_BYTES {
                self.validate_packet_length(usize::from(u16::from_be_bytes([bytes[0], bytes[1]])))?;
            } else if self.buffer.len() == 1 && !bytes.is_empty() {
                self.validate_packet_length(usize::from(u16::from_be_bytes([
                    self.buffer[0],
                    bytes[0],
                ])))?;
            }
        }
        self.buffer.extend_from_slice(bytes);
        let mut packets = Vec::new();
        loop {
            if self.expected_len.is_none() {
                if self.buffer.len() < PACKET_LEN_PREFIX_BYTES {
                    break;
                }
                let len = usize::from(u16::from_be_bytes([self.buffer[0], self.buffer[1]]));
                self.validate_packet_length(len)?;
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
    fn validate_packet_length(&self, length: usize) -> Result<(), BackendError> {
        if length == 0 || length > self.maximum_packet_bytes {
            return Err(BackendError::InvalidConfig(format!(
                "VPN packet frame length {length} is outside the negotiated 1..={} byte corridor",
                self.maximum_packet_bytes
            )));
        }
        Ok(())
    }
}
impl LinuxTunDevice {
    #[cfg(target_os = "linux")]
    fn create(requested_name: &str) -> Result<Self, BackendError> {
        let name_bytes = requested_name.as_bytes();
        if name_bytes.is_empty() || name_bytes.len() >= nix::libc::IFNAMSIZ {
            return Err(BackendError::InvalidConfig(format!(
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
            return Err(BackendError::Io(io::Error::last_os_error()));
        }
        let mut req = unsafe { std::mem::zeroed::<nix::libc::ifreq>() };
        unsafe {
            std::ptr::copy_nonoverlapping(
                name_bytes.as_ptr() as *const nix::libc::c_char,
                req.ifr_name.as_mut_ptr(),
                name_bytes.len(),
            );
            req.ifr_ifru.ifru_flags = linux_tun_create_flags() as _;
        }
        let ioctl_result = unsafe { nix::libc::ioctl(fd, LINUX_TUNSETIFF as _, &req) };
        if ioctl_result < 0 {
            let error = io::Error::last_os_error();
            unsafe {
                nix::libc::close(fd);
            }
            return Err(BackendError::Io(error));
        }
        let name = unsafe { CStr::from_ptr(req.ifr_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        if let Err(error) = verify_requested_tun_name(requested_name, &name) {
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
    fn create(_requested_name: &str) -> Result<Self, BackendError> {
        Err(BackendError::State(
            "sora-vpn-backend only supports Linux TUN hosts".to_owned(),
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
const fn linux_tun_create_flags() -> nix::libc::c_short {
    LINUX_IFF_TUN | LINUX_IFF_NO_PI | LINUX_IFF_TUN_EXCL
}
#[cfg(target_os = "linux")]
fn verify_requested_tun_name(
    requested_name: &str,
    returned_name: &str,
) -> Result<(), BackendError> {
    if returned_name != requested_name {
        return Err(BackendError::State(format!(
            "kernel returned TUN interface `{returned_name}` instead of requested `{requested_name}`"
        )));
    }
    Ok(())
}
fn encode_packet_stream_frame(packet: &[u8]) -> Result<Vec<u8>, BackendError> {
    let packet_len = u16::try_from(packet.len()).map_err(|_| {
        BackendError::State(format!(
            "packet length {} exceeds u16 packet-stream limit",
            packet.len()
        ))
    })?;
    let mut encoded = Vec::with_capacity(PACKET_LEN_PREFIX_BYTES + packet.len());
    encoded.extend_from_slice(&packet_len.to_be_bytes());
    encoded.extend_from_slice(packet);
    Ok(encoded)
}
fn apply_tunnel_link_config(
    interface_name: &str,
    mtu: u16,
    tunnel_addresses: &[ParsedCidr],
    shutdown_requested: &AtomicBool,
) -> Result<(), BackendError> {
    ensure_backend_running(shutdown_requested)?;
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
        ensure_backend_running(shutdown_requested)?;
        run_command(DEFAULT_ROUTE_CMD, address_add_args(interface_name, address))?;
    }
    Ok(())
}
fn apply_client_routes(
    interface_name: &str,
    routes: &[ParsedCidr],
    shutdown_requested: &AtomicBool,
) -> Result<(), BackendError> {
    for route in routes {
        ensure_backend_running(shutdown_requested)?;
        run_command(DEFAULT_ROUTE_CMD, route_add_args(interface_name, route))?;
    }
    Ok(())
}
fn address_add_args(interface_name: &str, address: &ParsedCidr) -> Vec<String> {
    vec![
        address.family().flag().to_owned(),
        "address".to_owned(),
        "add".to_owned(),
        address.render(),
        "dev".to_owned(),
        interface_name.to_owned(),
    ]
}
fn route_add_args(interface_name: &str, route: &ParsedCidr) -> Vec<String> {
    vec![
        route.family().flag().to_owned(),
        "route".to_owned(),
        "add".to_owned(),
        route.render(),
        "dev".to_owned(),
        interface_name.to_owned(),
    ]
}
fn acquire_forwarding(
    shared_network: &Arc<Mutex<SharedNetworkState>>,
    family: IpFamily,
    shutdown_requested: &AtomicBool,
) -> Result<(), BackendError> {
    ensure_backend_running(shutdown_requested)?;
    let mut guard = shared_network
        .lock()
        .map_err(|_| BackendError::State("shared network state mutex poisoned".to_owned()))?;
    let slot = forwarding_slot_mut(&mut guard, family);
    match slot {
        Some(reservation) => {
            reservation.ref_count = reservation.ref_count.saturating_add(1);
            Ok(())
        }
        None => {
            let key = family.forwarding_key();
            let previous_value = run_command("sysctl", vec!["-n".to_owned(), key.to_owned()])?
                .trim()
                .to_owned();
            if previous_value != "1" {
                run_command("sysctl", vec!["-w".to_owned(), format!("{key}=1")])?;
            }
            *slot = Some(ForwardingReservation {
                previous_value,
                ref_count: 1,
            });
            Ok(())
        }
    }
}
fn release_forwarding(
    shared_network: &Arc<Mutex<SharedNetworkState>>,
    family: IpFamily,
) -> Result<(), BackendError> {
    release_forwarding_with(shared_network, family, |key, previous_value| {
        run_command(
            "sysctl",
            vec!["-w".to_owned(), format!("{key}={previous_value}")],
        )
        .map(|_| ())
    })
}
fn release_forwarding_with<F>(
    shared_network: &Arc<Mutex<SharedNetworkState>>,
    family: IpFamily,
    mut restore: F,
) -> Result<(), BackendError>
where
    F: FnMut(&str, &str) -> Result<(), BackendError>,
{
    let mut guard = shared_network
        .lock()
        .map_err(|_| BackendError::State("shared network state mutex poisoned".to_owned()))?;
    let slot = forwarding_slot_mut(&mut guard, family);
    let Some(reservation) = slot.as_mut() else {
        return Ok(());
    };
    if reservation.ref_count > 1 {
        reservation.ref_count -= 1;
        return Ok(());
    }
    if !reservation.previous_value.is_empty() {
        restore(family.forwarding_key(), &reservation.previous_value)?;
    }
    *slot = None;
    Ok(())
}
fn forwarding_slot_mut(
    shared_network: &mut SharedNetworkState,
    family: IpFamily,
) -> &mut Option<ForwardingReservation> {
    match family {
        IpFamily::V4 => &mut shared_network.ipv4_forwarding,
        IpFamily::V6 => &mut shared_network.ipv6_forwarding,
    }
}
fn forwarding_egress_interface(
    config: &BackendConfig,
    family: IpFamily,
) -> Result<Option<&str>, BackendError> {
    let (forwarding_enabled, egress_interface) = match family {
        IpFamily::V4 => (config.ipv4_forward, config.egress_v4_interface.as_deref()),
        IpFamily::V6 => (config.ipv6_forward, config.egress_v6_interface.as_deref()),
    };
    if !forwarding_enabled {
        return Ok(None);
    }
    egress_interface.map(Some).ok_or_else(|| {
        BackendError::InvalidConfig(format!(
            "{} forwarding requires a resolved egress interface",
            match family {
                IpFamily::V4 => "IPv4",
                IpFamily::V6 => "IPv6",
            }
        ))
    })
}
fn session_firewall_rules(
    interface_name: &str,
    client_addresses: ClientTunnelAddresses,
    ipv4_forward_egress: Option<&str>,
    ipv6_forward_egress: Option<&str>,
) -> Vec<FirewallRule> {
    let mut rules = Vec::with_capacity(
        12 + IPV4_PROTECTED_DESTINATION_CIDRS_V1.len() + IPV6_PROTECTED_DESTINATION_CIDRS_V1.len(),
    );
    for (family, address, forward_egress) in [
        (
            IpFamily::V4,
            format!("{}/32", client_addresses.ipv4),
            ipv4_forward_egress,
        ),
        (
            IpFamily::V6,
            format!("{}/128", client_addresses.ipv6),
            ipv6_forward_egress,
        ),
    ] {
        rules.push(FirewallRule {
            family,
            chain: "INPUT",
            arguments: vec![
                "-i".to_owned(),
                interface_name.to_owned(),
                "-j".to_owned(),
                "DROP".to_owned(),
            ],
        });
        rules.push(FirewallRule {
            family,
            chain: "OUTPUT",
            arguments: vec![
                "-o".to_owned(),
                interface_name.to_owned(),
                "!".to_owned(),
                "-d".to_owned(),
                address.clone(),
                "-j".to_owned(),
                "DROP".to_owned(),
            ],
        });

        // Rules are installed with `iptables -I ... 1`, so append each
        // direction's catch-all before its allow rule. The resulting chain
        // permits only the pinned egress path and keeps every other host
        // interface fail-closed.
        rules.push(FirewallRule {
            family,
            chain: "FORWARD",
            arguments: vec![
                "-o".to_owned(),
                interface_name.to_owned(),
                "-j".to_owned(),
                "DROP".to_owned(),
            ],
        });
        if let Some(egress_interface) = forward_egress {
            rules.push(FirewallRule {
                family,
                chain: "FORWARD",
                arguments: vec![
                    "-i".to_owned(),
                    egress_interface.to_owned(),
                    "-o".to_owned(),
                    interface_name.to_owned(),
                    "-d".to_owned(),
                    address.clone(),
                    "-m".to_owned(),
                    "conntrack".to_owned(),
                    "--ctstate".to_owned(),
                    "ESTABLISHED,RELATED".to_owned(),
                    "-j".to_owned(),
                    "ACCEPT".to_owned(),
                ],
            });
        }
        rules.push(FirewallRule {
            family,
            chain: "FORWARD",
            arguments: vec![
                "-i".to_owned(),
                interface_name.to_owned(),
                "-j".to_owned(),
                "DROP".to_owned(),
            ],
        });
        if let Some(egress_interface) = forward_egress {
            rules.push(FirewallRule {
                family,
                chain: "FORWARD",
                arguments: vec![
                    "-i".to_owned(),
                    interface_name.to_owned(),
                    "-o".to_owned(),
                    egress_interface.to_owned(),
                    "-s".to_owned(),
                    address.clone(),
                    "-m".to_owned(),
                    "conntrack".to_owned(),
                    "--ctstate".to_owned(),
                    "NEW,ESTABLISHED,RELATED".to_owned(),
                    "-j".to_owned(),
                    "ACCEPT".to_owned(),
                ],
            });
            // Each rule is inserted at chain position one. Append protected
            // destinations after the allow so the kernel evaluates every
            // same-interface SSRF/lateral-movement deny before that allow.
            for destination in family.protected_destination_cidrs() {
                rules.push(FirewallRule {
                    family,
                    chain: "FORWARD",
                    arguments: vec![
                        "-i".to_owned(),
                        interface_name.to_owned(),
                        "-o".to_owned(),
                        egress_interface.to_owned(),
                        "-s".to_owned(),
                        address.clone(),
                        "-d".to_owned(),
                        (*destination).to_owned(),
                        "-j".to_owned(),
                        "DROP".to_owned(),
                    ],
                });
            }
        }
    }
    rules
}
fn apply_firewall_rule(rule: &FirewallRule) -> Result<bool, BackendError> {
    let program = rule.family.nat_program();
    if !command_exists(program) {
        return Err(BackendError::State(format!(
            "{program} is required for VPN anti-spoofing"
        )));
    }
    match classify_rule_check(run_command(program, firewall_rule_args("-C", rule)))? {
        RuleCheck::Exists => Ok(false),
        RuleCheck::Missing => {
            run_command(program, firewall_rule_args("-I", rule))?;
            Ok(true)
        }
    }
}
fn remove_firewall_rule(rule: &FirewallRule) -> Result<(), BackendError> {
    let result = run_command(rule.family.nat_program(), firewall_rule_args("-D", rule));
    match result {
        Ok(_) => Ok(()),
        Err(BackendError::CommandFailed { detail, .. })
            if detail.contains("Bad rule")
                || detail.contains("No chain/target/match")
                || detail.contains("does a matching rule exist") =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
fn firewall_rule_args(action: &str, rule: &FirewallRule) -> Vec<String> {
    let mut args = vec![
        "-w".to_owned(),
        "-t".to_owned(),
        "filter".to_owned(),
        action.to_owned(),
        rule.chain.to_owned(),
    ];
    if action == "-I" {
        args.push("1".to_owned());
    }
    args.extend(rule.arguments.iter().cloned());
    args
}
fn apply_nat_rule(
    family: IpFamily,
    source_cidr: &str,
    egress_interface: &str,
    shutdown_requested: &AtomicBool,
) -> Result<Option<NatRule>, BackendError> {
    ensure_backend_running(shutdown_requested)?;
    let program = family.nat_program();
    if !command_exists(program) {
        return Err(BackendError::State(format!(
            "{program} is required for {} NAT",
            match family {
                IpFamily::V4 => "IPv4",
                IpFamily::V6 => "IPv6",
            }
        )));
    }
    let args = nat_rule_args("-C", source_cidr, egress_interface);
    match classify_rule_check(run_command(program, args))? {
        RuleCheck::Exists => return Ok(None),
        RuleCheck::Missing => {
            run_command(program, nat_rule_args("-A", source_cidr, egress_interface))?;
        }
    }
    Ok(Some(NatRule {
        family,
        source_cidr: source_cidr.to_owned(),
        egress_interface: egress_interface.to_owned(),
    }))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleCheck {
    Exists,
    Missing,
}
fn classify_rule_check(result: Result<String, BackendError>) -> Result<RuleCheck, BackendError> {
    match result {
        Ok(_) => Ok(RuleCheck::Exists),
        Err(BackendError::CommandFailed {
            exit_code: Some(1), ..
        }) => Ok(RuleCheck::Missing),
        Err(error) => Err(error),
    }
}
fn remove_nat_rule(rule: &NatRule) -> Result<(), BackendError> {
    let result = run_command(
        rule.family.nat_program(),
        nat_rule_args("-D", &rule.source_cidr, &rule.egress_interface),
    );
    match result {
        Ok(_) => Ok(()),
        Err(BackendError::CommandFailed { detail, .. })
            if detail.contains("Bad rule")
                || detail.contains("No chain/target/match")
                || detail.contains("does a matching rule exist") =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
fn nat_rule_args(action: &str, source_cidr: &str, egress_interface: &str) -> Vec<String> {
    vec![
        "-w".to_owned(),
        "-t".to_owned(),
        "nat".to_owned(),
        action.to_owned(),
        "POSTROUTING".to_owned(),
        "-s".to_owned(),
        source_cidr.to_owned(),
        "-o".to_owned(),
        egress_interface.to_owned(),
        "-j".to_owned(),
        "MASQUERADE".to_owned(),
    ]
}
fn resolve_egress_interface(
    override_interface: Option<&str>,
    family: IpFamily,
) -> Result<Option<String>, BackendError> {
    if let Some(value) = override_interface {
        return validate_linux_interface_name(value, "egress interface").map(Some);
    }
    let output = run_command(
        DEFAULT_ROUTE_CMD,
        vec![
            family.flag().to_owned(),
            "-o".to_owned(),
            "route".to_owned(),
            "show".to_owned(),
            "default".to_owned(),
        ],
    )?;
    for line in output.lines() {
        if let Some(device) = parse_route_device(line)? {
            return Ok(Some(device));
        }
    }
    Ok(None)
}
fn parse_route_device(line: &str) -> Result<Option<String>, BackendError> {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    let mut idx = 0usize;
    while idx < tokens.len() {
        if tokens[idx] == "dev" && idx + 1 < tokens.len() {
            return validate_linux_interface_name(tokens[idx + 1], "discovered egress interface")
                .map(Some);
        }
        idx += 1;
    }
    Ok(None)
}
fn normalize_mtu(value: u64) -> Result<u16, BackendError> {
    if value == 0 || value > u64::from(u16::MAX) {
        return Err(BackendError::InvalidConfig(format!(
            "mtu must be within 1..={}",
            u16::MAX
        )));
    }
    u16::try_from(value)
        .map_err(|_| BackendError::InvalidConfig(format!("mtu {value} does not fit into u16")))
}
fn derive_interface_name(prefix: &str, session_id_hex: &str) -> Result<String, BackendError> {
    let normalized_prefix = validate_linux_interface_name(prefix, "interface prefix")?;
    let suffix_source = session_id_hex
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();
    let remaining = (nix::libc::IFNAMSIZ - 1).saturating_sub(normalized_prefix.len());
    if remaining == 0 {
        return Err(BackendError::InvalidConfig(format!(
            "interface prefix `{normalized_prefix}` leaves no room for a session suffix"
        )));
    }
    let suffix = if suffix_source.is_empty() {
        "0".repeat(remaining.min(1))
    } else {
        suffix_source.chars().take(remaining).collect::<String>()
    };
    Ok(format!("{normalized_prefix}{suffix}"))
}
fn validate_linux_interface_name(value: &str, label: &str) -> Result<String, BackendError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() >= nix::libc::IFNAMSIZ
        || bytes[0] == b'-'
        || bytes
            .iter()
            .any(|byte| !byte.is_ascii_graphic() || matches!(byte, b'/' | b':'))
    {
        return Err(BackendError::InvalidConfig(format!(
            "{label} must be a nonempty canonical Linux interface name shorter than IFNAMSIZ, using ASCII graphic bytes without a leading '-', '/', ':', whitespace, control bytes, or NUL"
        )));
    }
    Ok(value.to_owned())
}
fn parse_cidr_list(values: &[String]) -> Result<Vec<ParsedCidr>, BackendError> {
    values.iter().map(|value| parse_cidr(value)).collect()
}
fn parse_cidr(value: &str) -> Result<ParsedCidr, BackendError> {
    let trimmed = value.trim();
    let Some((address, prefix)) = trimmed.split_once('/') else {
        return Err(BackendError::InvalidCidr(trimmed.to_owned()));
    };
    let address = address
        .parse::<IpAddr>()
        .map_err(|_| BackendError::InvalidCidr(trimmed.to_owned()))?;
    let prefix = prefix
        .parse::<u8>()
        .map_err(|_| BackendError::InvalidCidr(trimmed.to_owned()))?;
    let family = match address {
        IpAddr::V4(_) => IpFamily::V4,
        IpAddr::V6(_) => IpFamily::V6,
    };
    if prefix > family.max_prefix() {
        return Err(BackendError::InvalidCidr(trimmed.to_owned()));
    }
    Ok(ParsedCidr { address, prefix })
}
fn cidr_contains_ip(cidr: &ParsedCidr, address: IpAddr) -> bool {
    match (cidr.address, address) {
        (IpAddr::V4(network), IpAddr::V4(address)) => {
            let mask = if cidr.prefix == 0 {
                0
            } else {
                u32::MAX << (32 - u32::from(cidr.prefix))
            };
            u32::from(network) & mask == u32::from(address) & mask
        }
        (IpAddr::V6(network), IpAddr::V6(address)) => {
            let mask = if cidr.prefix == 0 {
                0
            } else {
                u128::MAX << (128 - u32::from(cidr.prefix))
            };
            u128::from(network) & mask == u128::from(address) & mask
        }
        _ => false,
    }
}
fn has_family(values: &[ParsedCidr], family: IpFamily) -> bool {
    values.iter().any(|value| value.family() == family)
}
#[derive(Debug)]
struct BoundedCommandOutput {
    bytes: Vec<u8>,
    exceeded_limit: bool,
}
fn read_bounded_command_output_until<R: io::Read>(
    mut reader: R,
    cancelled: &AtomicBool,
) -> io::Result<BoundedCommandOutput> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1)
        .map_err(|error| io::Error::other(format!("failed to reserve command output: {error}")))?;
    let mut exceeded_limit = false;
    let mut buffer = [0_u8; 4096];
    // Cancellation must not discard bytes a short-lived command already
    // placed in its pipe: callers rely on outputs such as `sysctl -n` to
    // restore privileged state safely. At the same time, an escaped
    // descendant may keep writing forever, so post-cancellation draining has
    // a strict byte budget and also stops as soon as the nonblocking pipe is
    // empty.
    let mut cancelled_drain_remaining =
        TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1.saturating_add(buffer.len());
    loop {
        let cancellation_requested = cancelled.load(Ordering::Acquire);
        if cancellation_requested && cancelled_drain_remaining == 0 {
            break;
        }
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted && !cancellation_requested => {
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => break,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if cancellation_requested || cancelled.load(Ordering::Acquire) {
                    break;
                }
                thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(error) => return Err(error),
        };
        if read == 0 {
            break;
        }
        if cancellation_requested {
            cancelled_drain_remaining = cancelled_drain_remaining.saturating_sub(read);
        }
        let remaining = TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1.saturating_sub(bytes.len());
        let retained = read.min(remaining);
        bytes.extend_from_slice(&buffer[..retained]);
        exceeded_limit |= retained != read;
    }
    Ok(BoundedCommandOutput {
        bytes,
        exceeded_limit,
    })
}
fn set_nonblocking(descriptor: i32) -> io::Result<()> {
    // SAFETY: `descriptor` is an open child-pipe descriptor, and both fcntl
    // operations only query/update its status flags.
    let flags = unsafe { nix::libc::fcntl(descriptor, nix::libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the descriptor remains owned by the corresponding `ChildStdout`
    // or `ChildStderr`; setting O_NONBLOCK does not transfer ownership.
    if unsafe {
        nix::libc::fcntl(
            descriptor,
            nix::libc::F_SETFL,
            flags | nix::libc::O_NONBLOCK,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn command_exists(program: &str) -> bool {
    resolve_trusted_command(program).is_some()
}
fn resolve_trusted_command(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        return None;
    }
    ["/usr/sbin", "/sbin", "/usr/bin", "/bin"]
        .into_iter()
        .map(|dir| PathBuf::from(dir).join(program))
        .find_map(|candidate| validate_trusted_command_path(&candidate))
}
fn validate_trusted_command_path(candidate: &std::path::Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(candidate).ok()?;
    let metadata = fs::symlink_metadata(&canonical).ok()?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o111 == 0
    {
        return None;
    }
    let parent = canonical.parent()?;
    for ancestor in parent.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).ok()?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
        {
            return None;
        }
    }
    Some(canonical)
}
fn run_command<I, S>(program: &str, args: I) -> Result<String, BackendError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let collected = args
        .into_iter()
        .map(|item| item.as_ref().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if collected.len() > TRUSTED_COMMAND_MAX_ARGUMENTS_V1
        || collected
            .iter()
            .any(|argument| argument.len() > TRUSTED_COMMAND_MAX_ARGUMENT_BYTES_V1)
    {
        return Err(BackendError::State(
            "trusted command arguments exceed the first-release corridor".to_owned(),
        ));
    }
    let program_path = resolve_trusted_command(program).ok_or_else(|| {
        BackendError::State(format!("{program} was not found in trusted system paths"))
    })?;
    execute_trusted_command(
        program,
        &program_path,
        &collected,
        TRUSTED_COMMAND_TIMEOUT_V1,
    )
}
fn execute_trusted_command(
    program: &str,
    program_path: &std::path::Path,
    arguments: &[String],
    deadline: Duration,
) -> Result<String, BackendError> {
    let mut command = ProcessCommand::new(program_path);
    command
        .env_clear()
        .env("PATH", "/usr/sbin:/sbin:/usr/bin:/bin")
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.process_group(0);
    let mut child = command.spawn()?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = kill_trusted_command_process_group(child.id());
            let _ = child.wait();
            return Err(BackendError::State(
                "trusted command stdout pipe is missing".to_owned(),
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let _ = kill_trusted_command_process_group(child.id());
            let _ = child.wait();
            return Err(BackendError::State(
                "trusted command stderr pipe is missing".to_owned(),
            ));
        }
    };
    if let Err(error) =
        set_nonblocking(stdout.as_raw_fd()).and_then(|()| set_nonblocking(stderr.as_raw_fd()))
    {
        let _ = terminate_trusted_command(&mut child);
        return Err(error.into());
    }
    let reader_cancelled = Arc::new(AtomicBool::new(false));
    let stdout_cancelled = Arc::clone(&reader_cancelled);
    let stdout_reader = match thread::Builder::new()
        .name("sora-vpn-command-stdout".to_owned())
        .spawn(move || read_bounded_command_output_until(stdout, &stdout_cancelled))
    {
        Ok(reader) => reader,
        Err(error) => {
            let _ = terminate_trusted_command(&mut child);
            return Err(error.into());
        }
    };
    let stderr_cancelled = Arc::clone(&reader_cancelled);
    let stderr_reader = match thread::Builder::new()
        .name("sora-vpn-command-stderr".to_owned())
        .spawn(move || read_bounded_command_output_until(stderr, &stderr_cancelled))
    {
        Ok(reader) => reader,
        Err(error) => {
            let _ = terminate_trusted_command(&mut child);
            reader_cancelled.store(true, Ordering::Release);
            let _ = stdout_reader.join();
            return Err(error.into());
        }
    };
    let started = Instant::now();
    let (status, timed_out) = loop {
        match trusted_command_leader_exited(child.id()) {
            Ok(true) => break (terminate_trusted_command(&mut child), false),
            Ok(false) => {}
            Err(error) => {
                let status = terminate_trusted_command(&mut child).and(Err(error));
                break (status, false);
            }
        }
        if started.elapsed() >= deadline {
            break (terminate_trusted_command(&mut child), true);
        }
        thread::sleep(Duration::from_millis(10));
    };
    // A descendant may have deliberately changed process groups while
    // retaining an inherited pipe. Nonblocking readers honor this flag, so
    // their joins cannot hold the privileged daemon indefinitely.
    reader_cancelled.store(true, Ordering::Release);
    let stdout = stdout_reader
        .join()
        .map_err(|_| BackendError::State("trusted command stdout reader panicked".to_owned()))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| BackendError::State("trusted command stderr reader panicked".to_owned()))??;
    let status = status?;
    if timed_out {
        return Err(BackendError::State(format!(
            "{program} timed out after {} seconds",
            deadline.as_secs_f64()
        )));
    }
    if stdout.exceeded_limit || stderr.exceeded_limit {
        return Err(BackendError::State(format!(
            "{program} output exceeded the first-release {TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1}-byte per-stream limit"
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
    Err(BackendError::CommandFailed {
        program: program.to_owned(),
        arguments: arguments.join(" "),
        status: status.to_string(),
        exit_code: status.code(),
        detail,
    })
}
fn trusted_command_leader_exited(child_pid: u32) -> io::Result<bool> {
    let mut information = std::mem::MaybeUninit::<nix::libc::siginfo_t>::zeroed();
    // SAFETY: `information` points to writable storage for one siginfo_t.
    // WNOWAIT observes only this direct child and deliberately leaves it
    // unreaped, pinning the numeric PID/process-group ID until killpg runs.
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
    // SAFETY: successful waitid initializes siginfo_t; with WNOHANG libc
    // reports si_pid == 0 when the child has not exited.
    let information = unsafe { information.assume_init() };
    // SAFETY: waitid(WEXITED) populates the SIGCHLD variant of siginfo_t.
    let observed_pid = unsafe { information.si_pid() };
    Ok(observed_pid > 0 && u32::try_from(observed_pid).ok() == Some(child_pid))
}
fn terminate_trusted_command(child: &mut Child) -> io::Result<ExitStatus> {
    let group_result = kill_trusted_command_process_group(child.id());
    if group_result.is_err() {
        // If group signalling failed, still prevent a live leader from making
        // `wait` unbounded. The original group error is returned after reap.
        let _ = child.kill();
    }
    let wait_result = child.wait();
    match (group_result, wait_result) {
        (Err(error), _) => Err(error),
        (Ok(()), result) => result,
    }
}
fn kill_trusted_command_process_group(child_pid: u32) -> io::Result<()> {
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::current_dir()
                .expect("current directory")
                .join(format!(
                    ".sora-vpn-backend-{label}-{}-{sequence}",
                    std::process::id()
                ));
            fs::create_dir(&path).expect("create test directory");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("protect test directory");
            Self(path)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn write_test_secret(byte: u8) -> (TestDirectory, PathBuf) {
        let directory = TestDirectory::new("secret");
        let path = directory.path().join("bootstrap.hex");
        fs::write(&path, hex::encode([byte; 32])).expect("write test secret");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("protect test secret");
        (directory, path)
    }
    fn command_failure(exit_code: i32) -> BackendError {
        BackendError::CommandFailed {
            program: "test-command".to_owned(),
            arguments: "test-arguments".to_owned(),
            status: format!("exit status {exit_code}"),
            exit_code: Some(exit_code),
            detail: "test failure".to_owned(),
        }
    }
    fn test_client_addresses() -> ClientTunnelAddresses {
        ClientTunnelAddresses {
            ipv4: Ipv4Addr::new(10, 208, 0, 2),
            ipv6: "fd53:7261:6574::2".parse().expect("canonical client IPv6"),
        }
    }
    fn test_ipv4_packet(source: Ipv4Addr, destination: Ipv4Addr) -> Vec<u8> {
        let mut packet = vec![0u8; 24];
        packet[0] = 0x45;
        let packet_len = u16::try_from(packet.len()).expect("test IPv4 packet length");
        packet[2..4].copy_from_slice(&packet_len.to_be_bytes());
        packet[6..8].copy_from_slice(&0x4000u16.to_be_bytes());
        packet[8] = 64;
        packet[9] = 17;
        packet[12..16].copy_from_slice(&source.octets());
        packet[16..20].copy_from_slice(&destination.octets());
        packet[20..].copy_from_slice(b"test");
        set_test_ipv4_checksum(&mut packet);
        packet
    }
    fn set_test_ipv4_checksum(packet: &mut [u8]) {
        packet[10..12].fill(0);
        let mut sum = 0u32;
        for word in packet[..20].chunks_exact(2) {
            sum += u32::from(u16::from_be_bytes([word[0], word[1]]));
        }
        while sum >> 16 != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        packet[10..12].copy_from_slice(&(!(sum as u16)).to_be_bytes());
    }
    fn test_ipv6_packet(source: Ipv6Addr, destination: Ipv6Addr) -> Vec<u8> {
        let mut packet = vec![0u8; 44];
        packet[0] = 0x60;
        packet[4..6].copy_from_slice(&4u16.to_be_bytes());
        packet[6] = 17;
        packet[7] = 64;
        packet[8..24].copy_from_slice(&source.octets());
        packet[24..40].copy_from_slice(&destination.octets());
        packet[40..].copy_from_slice(b"test");
        packet
    }
    #[test]
    fn backend_config_requires_bootstrap_secret_for_unix_endpoints() {
        let error = BackendConfig::from_cli(test_cli())
            .expect_err("Unix peer credentials do not replace bootstrap authentication");
        assert!(error.to_string().contains("every backend endpoint"));
    }
    #[test]
    fn backend_config_debug_redacts_and_secret_zeroizer_clears_bytes() {
        let (_secret_directory, config) = test_backend_config();
        let rendered = format!("{config:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("165, 165"));

        let mut secret = [0xA5; 32];
        zeroize_secret(&mut secret);
        assert_eq!(secret, [0; 32]);

        let mut buffer = SensitiveReadBuffer(vec![0xA5; 32]);
        buffer.clear();
        assert!(buffer.0.iter().all(|byte| *byte == 0));

        let mut probe = SensitiveReadProbe([0xA5]);
        probe.clear();
        assert_eq!(probe.0, [0]);
    }
    #[test]
    fn backend_runtime_shares_config_without_copying_secret() {
        let (_secret_directory, config) = test_backend_config();
        let config = Arc::new(config);
        let session_config = Arc::clone(&config);
        assert!(Arc::ptr_eq(&config, &session_config));
        assert_eq!(Arc::strong_count(&config), 2);

        let source = include_str!("main.rs");
        let clone_derive = ["#[derive(", "Clone)]\nstruct BackendConfig"].concat();
        assert!(
            !source.contains(&clone_derive),
            "BackendConfig must remain non-Clone so its bootstrap secret has one owner"
        );
    }
    #[test]
    fn unix_clock_conversion_fails_closed_before_epoch() {
        let error = unix_time_ms_at(UNIX_EPOCH - Duration::from_millis(1))
            .expect_err("pre-epoch clocks must not panic or authenticate frames");
        assert!(error.to_string().contains("before the Unix epoch"));
        assert_eq!(
            unix_time_ms_at(UNIX_EPOCH + Duration::from_millis(42)).expect("valid clock"),
            42
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn tun_creation_requests_exclusive_interface_ownership() {
        let flags = linux_tun_create_flags();
        assert_ne!(flags & LINUX_IFF_TUN, 0);
        assert_ne!(flags & LINUX_IFF_NO_PI, 0);
        assert_ne!(flags & LINUX_IFF_TUN_EXCL, 0);
        verify_requested_tun_name("svpn0", "svpn0").expect("exact kernel name");
        assert!(verify_requested_tun_name("svpn0", "svpn1").is_err());
    }
    #[test]
    fn route_and_address_commands_fail_on_preexisting_host_state() {
        let cidr = parse_cidr("10.208.0.1/32").expect("cidr");
        assert_eq!(address_add_args("svpn0", &cidr)[2], "add");
        assert_eq!(route_add_args("svpn0", &cidr)[2], "add");
        assert!(!address_add_args("svpn0", &cidr).contains(&"replace".to_owned()));
        assert!(!route_add_args("svpn0", &cidr).contains(&"replace".to_owned()));
    }
    #[test]
    fn packet_boundary_accepts_only_the_assigned_endpoint_in_each_direction() {
        let assigned = test_client_addresses();
        let remote_v4 = Ipv4Addr::new(1, 1, 1, 1);
        let remote_v6 = "2606:4700:4700::1111"
            .parse::<Ipv6Addr>()
            .expect("remote IPv6");
        let client_v4 = test_ipv4_packet(assigned.ipv4, remote_v4);
        validate_vpn_packet(&client_v4, assigned, PacketDirection::ClientToBackend)
            .expect("assigned IPv4 source");
        let server_v4 = test_ipv4_packet(remote_v4, assigned.ipv4);
        validate_vpn_packet(&server_v4, assigned, PacketDirection::BackendToClient)
            .expect("assigned IPv4 destination");
        let client_v6 = test_ipv6_packet(assigned.ipv6, remote_v6);
        validate_vpn_packet(&client_v6, assigned, PacketDirection::ClientToBackend)
            .expect("assigned IPv6 source");
        let server_v6 = test_ipv6_packet(remote_v6, assigned.ipv6);
        validate_vpn_packet(&server_v6, assigned, PacketDirection::BackendToClient)
            .expect("assigned IPv6 destination");

        let spoofed_v4 = test_ipv4_packet(Ipv4Addr::new(10, 208, 0, 3), remote_v4);
        assert!(
            validate_vpn_packet(&spoofed_v4, assigned, PacketDirection::ClientToBackend).is_err()
        );
        let leaked_v4 = test_ipv4_packet(remote_v4, Ipv4Addr::new(10, 208, 0, 3));
        assert!(
            validate_vpn_packet(&leaked_v4, assigned, PacketDirection::BackendToClient).is_err()
        );
        let spoofed_v6 = test_ipv6_packet(
            "fd53:7261:6574::3".parse().expect("spoofed IPv6"),
            remote_v6,
        );
        assert!(
            validate_vpn_packet(&spoofed_v6, assigned, PacketDirection::ClientToBackend).is_err()
        );
        let leaked_v6 =
            test_ipv6_packet(remote_v6, "fd53:7261:6574::3".parse().expect("leaked IPv6"));
        assert!(
            validate_vpn_packet(&leaked_v6, assigned, PacketDirection::BackendToClient).is_err()
        );
    }
    #[test]
    fn public_exit_rejects_same_egress_private_special_and_mapped_destinations() {
        let assigned = test_client_addresses();
        for destination in [
            Ipv4Addr::UNSPECIFIED,
            Ipv4Addr::new(10, 1, 2, 3),
            Ipv4Addr::new(100, 64, 0, 1),
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::new(169, 254, 169, 254),
            Ipv4Addr::new(172, 31, 0, 1),
            Ipv4Addr::new(192, 0, 2, 1),
            Ipv4Addr::new(192, 168, 1, 1),
            Ipv4Addr::new(198, 18, 0, 1),
            Ipv4Addr::new(198, 51, 100, 1),
            Ipv4Addr::new(203, 0, 113, 1),
            Ipv4Addr::new(224, 0, 0, 1),
            Ipv4Addr::BROADCAST,
        ] {
            let packet = test_ipv4_packet(assigned.ipv4, destination);
            let error = validate_vpn_packet(&packet, assigned, PacketDirection::ClientToBackend)
                .expect_err("protected IPv4 destination must not leave the public exit");
            assert!(error.to_string().contains("protected"));
        }

        for destination in [
            Ipv6Addr::UNSPECIFIED,
            Ipv6Addr::LOCALHOST,
            "::ffff:10.1.2.3".parse().expect("mapped private IPv4"),
            "64:ff9b::a01:203".parse().expect("well-known NAT64"),
            "64:ff9b:1::1".parse().expect("local-use NAT64"),
            "100::1".parse().expect("discard-only IPv6"),
            "2001:2::1".parse().expect("benchmark IPv6"),
            "2001:db8::1".parse().expect("documentation IPv6"),
            "3fff::1".parse().expect("documentation IPv6"),
            "fc00::1".parse().expect("unique-local IPv6"),
            "fec0::1".parse().expect("deprecated site-local IPv6"),
            "fe80::1".parse().expect("link-local IPv6"),
            "ff02::1".parse().expect("multicast IPv6"),
        ] {
            let packet = test_ipv6_packet(assigned.ipv6, destination);
            let error = validate_vpn_packet(&packet, assigned, PacketDirection::ClientToBackend)
                .expect_err("protected IPv6 destination must not leave the public exit");
            assert!(error.to_string().contains("protected"));
        }

        validate_vpn_packet(
            &test_ipv4_packet(assigned.ipv4, Ipv4Addr::new(1, 1, 1, 1)),
            assigned,
            PacketDirection::ClientToBackend,
        )
        .expect("globally routed IPv4 destination");
        validate_vpn_packet(
            &test_ipv6_packet(
                assigned.ipv6,
                "2606:4700:4700::1111"
                    .parse()
                    .expect("globally routed IPv6"),
            ),
            assigned,
            PacketDirection::ClientToBackend,
        )
        .expect("globally routed IPv6 destination");
    }
    #[test]
    fn ipv4_packet_boundary_rejects_malformed_lengths_checksum_and_fragments() {
        let assigned = test_client_addresses();
        let mut bad_ihl = test_ipv4_packet(assigned.ipv4, Ipv4Addr::new(1, 1, 1, 1));
        bad_ihl[0] = 0x44;
        assert!(validate_vpn_packet(&bad_ihl, assigned, PacketDirection::ClientToBackend).is_err());

        let mut bad_length = test_ipv4_packet(assigned.ipv4, Ipv4Addr::new(1, 1, 1, 1));
        bad_length[2..4].copy_from_slice(&20u16.to_be_bytes());
        assert!(
            validate_vpn_packet(&bad_length, assigned, PacketDirection::ClientToBackend).is_err()
        );

        let mut bad_checksum = test_ipv4_packet(assigned.ipv4, Ipv4Addr::new(1, 1, 1, 1));
        bad_checksum[8] ^= 1;
        assert!(
            validate_vpn_packet(&bad_checksum, assigned, PacketDirection::ClientToBackend).is_err()
        );

        let mut fragment = test_ipv4_packet(assigned.ipv4, Ipv4Addr::new(1, 1, 1, 1));
        fragment[6..8].copy_from_slice(&0x2000u16.to_be_bytes());
        set_test_ipv4_checksum(&mut fragment);
        assert!(
            validate_vpn_packet(&fragment, assigned, PacketDirection::ClientToBackend).is_err()
        );
    }
    #[test]
    fn ipv6_packet_boundary_rejects_malformed_payload_lengths_and_fragments() {
        let assigned = test_client_addresses();
        let remote = "2606:4700:4700::1111"
            .parse::<Ipv6Addr>()
            .expect("remote IPv6");
        let mut bad_length = test_ipv6_packet(assigned.ipv6, remote);
        bad_length[4..6].copy_from_slice(&3u16.to_be_bytes());
        assert!(
            validate_vpn_packet(&bad_length, assigned, PacketDirection::ClientToBackend).is_err()
        );

        let mut fragment = vec![0u8; 48];
        fragment[0] = 0x60;
        fragment[4..6].copy_from_slice(&8u16.to_be_bytes());
        fragment[6] = 44;
        fragment[7] = 64;
        fragment[8..24].copy_from_slice(&assigned.ipv6.octets());
        fragment[24..40].copy_from_slice(&remote.octets());
        fragment[40] = 17;
        assert!(
            validate_vpn_packet(&fragment, assigned, PacketDirection::ClientToBackend).is_err()
        );
    }
    #[test]
    fn per_session_firewall_rules_are_interface_and_address_scoped() {
        let rules =
            session_firewall_rules("svpn0", test_client_addresses(), Some("wan0"), Some("wan6"));
        assert_eq!(
            rules.len(),
            12 + IPV4_PROTECTED_DESTINATION_CIDRS_V1.len()
                + IPV6_PROTECTED_DESTINATION_CIDRS_V1.len()
        );
        assert!(rules.iter().any(|rule| {
            rule.family == IpFamily::V4
                && rule.chain == "INPUT"
                && rule
                    .arguments
                    .iter()
                    .map(String::as_str)
                    .eq(["-i", "svpn0", "-j", "DROP"])
        }));
        assert!(rules.iter().any(|rule| {
            rule.family == IpFamily::V6
                && rule.chain == "OUTPUT"
                && rule
                    .arguments
                    .iter()
                    .any(|argument| argument == "fd53:7261:6574::2/128")
        }));
        let outbound_allow = rules
            .iter()
            .find(|rule| {
                rule.family == IpFamily::V4
                    && rule.chain == "FORWARD"
                    && rule.arguments.last().map(String::as_str) == Some("ACCEPT")
                    && rule.arguments.first().map(String::as_str) == Some("-i")
                    && rule.arguments.get(1).map(String::as_str) == Some("svpn0")
            })
            .expect("IPv4 outbound allow rule");
        assert!(outbound_allow.arguments.iter().map(String::as_str).eq([
            "-i",
            "svpn0",
            "-o",
            "wan0",
            "-s",
            "10.208.0.2/32",
            "-m",
            "conntrack",
            "--ctstate",
            "NEW,ESTABLISHED,RELATED",
            "-j",
            "ACCEPT",
        ]));
        assert!(rules.iter().any(|rule| {
            rule.family == IpFamily::V4
                && rule.chain == "FORWARD"
                && rule.arguments.iter().map(String::as_str).eq([
                    "-i",
                    "wan0",
                    "-o",
                    "svpn0",
                    "-d",
                    "10.208.0.2/32",
                    "-m",
                    "conntrack",
                    "--ctstate",
                    "ESTABLISHED,RELATED",
                    "-j",
                    "ACCEPT",
                ])
        }));
        for destination in IPV4_PROTECTED_DESTINATION_CIDRS_V1 {
            assert!(rules.iter().any(|rule| {
                rule.family == IpFamily::V4
                    && rule.chain == "FORWARD"
                    && rule.arguments.iter().map(String::as_str).eq([
                        "-i",
                        "svpn0",
                        "-o",
                        "wan0",
                        "-s",
                        "10.208.0.2/32",
                        "-d",
                        *destination,
                        "-j",
                        "DROP",
                    ])
            }));
        }
        for destination in IPV6_PROTECTED_DESTINATION_CIDRS_V1 {
            assert!(rules.iter().any(|rule| {
                rule.family == IpFamily::V6
                    && rule.chain == "FORWARD"
                    && rule.arguments.iter().map(String::as_str).eq([
                        "-i",
                        "svpn0",
                        "-o",
                        "wan6",
                        "-s",
                        "fd53:7261:6574::2/128",
                        "-d",
                        *destination,
                        "-j",
                        "DROP",
                    ])
            }));
        }
        let insert = firewall_rule_args("-I", outbound_allow);
        assert!(
            insert[..6]
                .iter()
                .map(String::as_str)
                .eq(["-w", "-t", "filter", "-I", "FORWARD", "1"])
        );
    }
    #[test]
    fn forwarding_rules_drop_wrong_interfaces_and_disabled_families() {
        let enabled = session_firewall_rules("svpn0", test_client_addresses(), Some("wan0"), None);
        let ipv4_effective = enabled
            .iter()
            .rev()
            .filter(|rule| rule.family == IpFamily::V4 && rule.chain == "FORWARD")
            .collect::<Vec<_>>();
        let protected_count = IPV4_PROTECTED_DESTINATION_CIDRS_V1.len();
        assert_eq!(ipv4_effective.len(), protected_count + 4);
        assert!(ipv4_effective[..protected_count].iter().all(|rule| {
            rule.arguments.last().map(String::as_str) == Some("DROP")
                && rule
                    .arguments
                    .windows(2)
                    .any(|window| window[0] == "-o" && window[1] == "wan0")
                && rule
                    .arguments
                    .windows(2)
                    .any(|window| window[0] == "-s" && window[1] == "10.208.0.2/32")
                && rule.arguments.iter().any(|argument| argument == "-d")
        }));
        assert_eq!(
            ipv4_effective[protected_count]
                .arguments
                .last()
                .map(String::as_str),
            Some("ACCEPT")
        );
        assert!(
            ipv4_effective[protected_count]
                .arguments
                .windows(2)
                .any(|window| { window[0] == "-o" && window[1] == "wan0" })
        );
        assert!(
            ipv4_effective[protected_count + 1]
                .arguments
                .iter()
                .map(String::as_str)
                .eq(["-i", "svpn0", "-j", "DROP"])
        );
        assert_eq!(
            ipv4_effective[protected_count + 2]
                .arguments
                .last()
                .map(String::as_str),
            Some("ACCEPT")
        );
        assert!(
            ipv4_effective[protected_count + 2]
                .arguments
                .windows(2)
                .any(|window| { window[0] == "-i" && window[1] == "wan0" })
        );
        assert!(
            ipv4_effective[protected_count + 3]
                .arguments
                .iter()
                .map(String::as_str)
                .eq(["-o", "svpn0", "-j", "DROP"])
        );

        let ipv6_effective = enabled
            .iter()
            .rev()
            .filter(|rule| rule.family == IpFamily::V6 && rule.chain == "FORWARD")
            .collect::<Vec<_>>();
        assert_eq!(ipv6_effective.len(), 2);
        assert!(ipv6_effective.iter().all(|rule| {
            rule.arguments.last().map(String::as_str) == Some("DROP")
                && !rule.arguments.iter().any(|argument| argument == "wan0")
        }));
        assert!(
            ipv6_effective[0]
                .arguments
                .iter()
                .map(String::as_str)
                .eq(["-i", "svpn0", "-j", "DROP"])
        );
        assert!(
            ipv6_effective[1]
                .arguments
                .iter()
                .map(String::as_str)
                .eq(["-o", "svpn0", "-j", "DROP"])
        );
    }
    #[test]
    fn forwarding_requires_a_pinned_family_egress_interface() {
        let (_secret_directory, mut config) = test_backend_config();
        assert_eq!(
            forwarding_egress_interface(&config, IpFamily::V4)
                .expect("configured IPv4 forwarding egress"),
            Some("eth0")
        );

        config.ipv4_forward = false;
        assert_eq!(
            forwarding_egress_interface(&config, IpFamily::V4).expect("disabled IPv4 forwarding"),
            None
        );

        config.ipv6_forward = true;
        assert!(forwarding_egress_interface(&config, IpFamily::V6).is_err());
    }
    #[test]
    fn firewall_and_nat_checks_track_only_rules_created_by_this_session() {
        assert_eq!(
            classify_rule_check(Ok(String::new())).expect("preexisting rule"),
            RuleCheck::Exists
        );
        assert_eq!(
            classify_rule_check(Err(command_failure(1))).expect("missing rule"),
            RuleCheck::Missing
        );
        assert!(classify_rule_check(Err(command_failure(2))).is_err());
        assert!(classify_rule_check(Err(BackendError::State("timeout".to_owned()))).is_err());
    }
    #[test]
    fn forwarding_restore_failure_preserves_state_for_retry() {
        let shared = Arc::new(Mutex::new(SharedNetworkState {
            ipv4_forwarding: Some(ForwardingReservation {
                previous_value: "0".to_owned(),
                ref_count: 1,
            }),
            ipv6_forwarding: None,
        }));
        let first = release_forwarding_with(&shared, IpFamily::V4, |_, _| {
            Err(BackendError::State("transient restore failure".to_owned()))
        });
        assert!(first.is_err());
        assert!(shared.lock().expect("state").ipv4_forwarding.is_some());
        release_forwarding_with(&shared, IpFamily::V4, |_, previous| {
            assert_eq!(previous, "0");
            Ok(())
        })
        .expect("retry restores original state");
        assert!(shared.lock().expect("state").ipv4_forwarding.is_none());
    }
    #[test]
    fn cleanup_continues_after_early_rollback_failures() {
        let applied = AppliedNetworkState {
            interface_name: "svpn0".to_owned(),
            forwarding_leases: vec![IpFamily::V4],
            nat_rules: vec![
                NatRule {
                    family: IpFamily::V4,
                    source_cidr: "10.0.0.0/24".to_owned(),
                    egress_interface: "eth0".to_owned(),
                },
                NatRule {
                    family: IpFamily::V4,
                    source_cidr: "10.1.0.0/24".to_owned(),
                    egress_interface: "eth0".to_owned(),
                },
            ],
            firewall_rules: vec![FirewallRule {
                family: IpFamily::V4,
                chain: "INPUT",
                arguments: vec![
                    "-i".to_owned(),
                    "svpn0".to_owned(),
                    "-j".to_owned(),
                    "DROP".to_owned(),
                ],
            }],
        };
        let mut nat_attempts = 0;
        let mut firewall_attempts = 0;
        let mut forwarding_attempts = 0;
        let mut link_attempts = 0;
        let error = cleanup_network_with(
            &applied,
            |_| {
                nat_attempts += 1;
                Err(BackendError::State("NAT cleanup fixture".to_owned()))
            },
            |_| {
                firewall_attempts += 1;
                Err(BackendError::State("firewall cleanup fixture".to_owned()))
            },
            |_| {
                forwarding_attempts += 1;
                Ok(())
            },
            |_| {
                link_attempts += 1;
                Ok(())
            },
        )
        .expect_err("aggregate cleanup failure");
        assert_eq!(nat_attempts, 2);
        assert_eq!(firewall_attempts, 1);
        assert_eq!(forwarding_attempts, 1);
        assert_eq!(link_attempts, 1);
        assert!(error.to_string().contains("3 failure(s)"));
    }
    #[test]
    fn parse_cidr_accepts_ipv4() {
        let parsed = parse_cidr("10.208.0.1/32").expect("cidr");
        assert_eq!(
            parsed,
            ParsedCidr {
                address: IpAddr::V4(Ipv4Addr::new(10, 208, 0, 1)),
                prefix: 32,
            }
        );
    }
    #[test]
    fn parse_cidr_accepts_ipv6() {
        let parsed = parse_cidr("fd53:7261:6574::1/128").expect("cidr");
        assert_eq!(
            parsed,
            ParsedCidr {
                address: IpAddr::V6(
                    "fd53:7261:6574::1"
                        .parse::<Ipv6Addr>()
                        .expect("ipv6 address")
                ),
                prefix: 128,
            }
        );
    }
    #[test]
    fn parse_route_device_extracts_dev_name() {
        let device = parse_route_device("default via 192.168.1.1 dev eth0 proto dhcp metric 100")
            .expect("valid route output");
        assert_eq!(device.as_deref(), Some("eth0"));
    }
    #[test]
    fn linux_interface_names_reject_command_and_kernel_metacharacters() {
        for invalid in [
            "",
            "-eth0",
            "eth 0",
            "eth/0",
            "eth:0",
            "eth\0",
            "éth0",
            "abcdefghijklmnop",
        ] {
            assert!(
                validate_linux_interface_name(invalid, "fixture interface").is_err(),
                "hostile interface name {invalid:?} must fail closed"
            );
        }
        for valid in ["eth0", "veth-1", "wg.test"] {
            assert_eq!(
                validate_linux_interface_name(valid, "fixture interface")
                    .expect("canonical interface"),
                valid
            );
        }
        assert!(parse_route_device("default dev -injected").is_err());
    }
    #[test]
    fn backend_endpoint_is_unix_only() {
        assert_eq!(
            parse_backend_endpoint("unix:/run/sora-vpn-backend.sock")
                .expect("absolute Unix endpoint"),
            BackendEndpoint(PathBuf::from("/run/sora-vpn-backend.sock"))
        );
        assert!(parse_backend_endpoint("unix:relative.sock").is_err());
        assert!(parse_backend_endpoint("tcp://127.0.0.1:19090").is_err());
        assert!(parse_backend_endpoint("tcp://192.0.2.1:19090").is_err());
    }
    #[test]
    fn command_resolution_ignores_untrusted_paths() {
        assert!(resolve_trusted_command("../ip").is_none());
        assert!(resolve_trusted_command("/tmp/ip").is_none());
        assert!(resolve_trusted_command("soranet-command-that-does-not-exist").is_none());
    }
    #[test]
    fn trusted_command_path_rejects_writable_executable() {
        let directory = TestDirectory::new("command");
        let command = directory.path().join("command");
        fs::write(&command, b"#!/bin/sh\nexit 0\n").expect("write command fixture");
        fs::set_permissions(&command, fs::Permissions::from_mode(0o777))
            .expect("make command writable");
        assert!(validate_trusted_command_path(&command).is_none());
    }
    #[test]
    fn command_output_reader_caps_retained_bytes() {
        let cancelled = AtomicBool::new(false);
        let exact = read_bounded_command_output_until(
            io::Cursor::new(vec![0xAA; TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1]),
            &cancelled,
        )
        .expect("read exact command output");
        assert_eq!(exact.bytes.len(), TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1);
        assert!(!exact.exceeded_limit);
        let oversized = read_bounded_command_output_until(
            io::Cursor::new(vec![0xAA; TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1 + 1]),
            &cancelled,
        )
        .expect("drain oversized command output");
        assert_eq!(oversized.bytes.len(), TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1);
        assert!(oversized.exceeded_limit);
    }
    #[test]
    fn cancelled_nonblocking_command_reader_does_not_wait_for_inherited_writer() {
        let (reader, _writer) = std::os::unix::net::UnixStream::pair().expect("pipe fixture");
        set_nonblocking(reader.as_raw_fd()).expect("make reader nonblocking");
        let cancelled = Arc::new(AtomicBool::new(false));
        let reader_cancelled = Arc::clone(&cancelled);
        let handle = thread::spawn(move || {
            read_bounded_command_output_until(reader, &reader_cancelled)
                .expect("cancelled read must finish")
        });
        thread::sleep(Duration::from_millis(10));
        cancelled.store(true, Ordering::Release);
        let started = Instant::now();
        let output = handle.join().expect("reader thread");
        assert!(output.bytes.is_empty());
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn cancelled_nonblocking_command_reader_drains_already_buffered_output() {
        let (reader, mut writer) = std::os::unix::net::UnixStream::pair().expect("pipe fixture");
        writer
            .write_all(b"required-state\n")
            .expect("buffer command output");
        set_nonblocking(reader.as_raw_fd()).expect("make reader nonblocking");
        let cancelled = AtomicBool::new(true);
        let output = read_bounded_command_output_until(reader, &cancelled)
            .expect("cancelled reader must drain buffered output");
        assert_eq!(output.bytes, b"required-state\n");
        assert!(!output.exceeded_limit);
    }
    #[test]
    fn cancelled_command_reader_has_bounded_drain_when_writer_never_blocks() {
        struct EndlessReader;

        impl io::Read for EndlessReader {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                buffer.fill(0xA5);
                Ok(buffer.len())
            }
        }

        let cancelled = AtomicBool::new(true);
        let output = read_bounded_command_output_until(EndlessReader, &cancelled)
            .expect("pre-cancelled endless reader must finish");
        assert_eq!(output.bytes.len(), TRUSTED_COMMAND_MAX_OUTPUT_BYTES_V1);
        assert!(output.exceeded_limit);
    }
    #[test]
    fn waitid_observes_child_exit_without_reaping_it() {
        let Some(program) = resolve_trusted_command("true") else {
            return;
        };
        let mut child = ProcessCommand::new(program).spawn().expect("spawn true");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if trusted_command_leader_exited(child.id()).expect("observe child") {
                break;
            }
            assert!(Instant::now() < deadline, "child did not exit in time");
            thread::sleep(Duration::from_millis(2));
        }
        assert!(
            child
                .wait()
                .expect("wait after non-reaping observation")
                .success(),
            "waitid(WNOWAIT) must leave the child available for the owning Child handle"
        );
    }
    #[test]
    fn trusted_command_deadline_terminates_stalled_process() {
        let Some(sleep) = resolve_trusted_command("sleep") else {
            return;
        };
        let error = execute_trusted_command(
            "sleep",
            &sleep,
            &["1".to_owned()],
            Duration::from_millis(10),
        )
        .expect_err("stalled command must be terminated");
        assert!(error.to_string().contains("timed out"));
    }
    #[test]
    fn trusted_command_deadline_terminates_descendants_holding_pipes() {
        let shell = std::path::Path::new("/bin/sh");
        if !shell.exists() {
            return;
        }
        let started = Instant::now();
        let error = execute_trusted_command(
            "sh",
            shell,
            &["-c".to_owned(), "sleep 30 & wait".to_owned()],
            Duration::from_millis(25),
        )
        .expect_err("the complete stalled command group must be terminated");
        assert!(error.to_string().contains("timed out"));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "a descendant retained the command output pipes"
        );
    }
    #[test]
    fn trusted_command_normal_exit_sweeps_descendants_before_reaping_leader() {
        let shell = std::path::Path::new("/bin/sh");
        if !shell.exists() {
            return;
        }
        let started = Instant::now();
        execute_trusted_command(
            "sh",
            shell,
            &["-c".to_owned(), "sleep 30 & exit 0".to_owned()],
            Duration::from_secs(2),
        )
        .expect("successful leader exit should still sweep descendants");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "a descendant retained the command output pipes"
        );
    }
    #[test]
    fn trusted_command_arguments_are_bounded() {
        let error = run_command("sh", vec!["x"; TRUSTED_COMMAND_MAX_ARGUMENTS_V1 + 1])
            .expect_err("too many arguments must fail before execution");
        assert!(error.to_string().contains("arguments exceed"));
    }
    #[test]
    fn unix_socket_startup_refuses_existing_regular_files() {
        let directory = TestDirectory::new("regular-socket");
        let path = directory.path().join("backend.sock");
        fs::write(&path, b"operator data").expect("write regular-file fixture");
        let error = ensure_unix_socket_path_available(&path)
            .expect_err("backend startup must not delete a non-socket path");
        assert!(error.to_string().contains("refusing to unlink"));
        assert_eq!(fs::read(&path).expect("fixture remains"), b"operator data");
    }
    #[test]
    fn unix_socket_startup_preserves_existing_live_socket() {
        let directory = TestDirectory::new("live-socket");
        let path = directory.path().join("s");
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind live socket");
        let error = ensure_unix_socket_path_available(&path)
            .expect_err("a second backend must not unlink a live listener");
        assert!(error.to_string().contains("refusing to unlink"));
        assert!(
            fs::symlink_metadata(&path)
                .expect("live socket remains")
                .file_type()
                .is_socket()
        );
        drop(listener);
    }
    #[test]
    fn unix_socket_guard_removes_only_the_endpoint_it_captured() {
        let directory = TestDirectory::new("socket-guard");
        let path = directory.path().join("g");
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind owned socket");
        let mut guard = UnixSocketGuard::capture(&path).expect("capture owned socket identity");
        drop(listener);
        guard.cleanup().expect("remove exact owned socket");
        assert!(!path.exists());

        let old_listener = std::os::unix::net::UnixListener::bind(&path).expect("bind old socket");
        let mut guard = UnixSocketGuard::capture(&path).expect("capture old socket identity");
        fs::remove_file(&path).expect("unlink old socket pathname");
        let replacement = std::os::unix::net::UnixListener::bind(&path)
            .expect("bind replacement socket at same path");
        let replacement_metadata = fs::symlink_metadata(&path).expect("replacement metadata");
        guard.cleanup().expect("replacement must be preserved");
        let after = fs::symlink_metadata(&path).expect("replacement remains");
        assert_eq!(after.dev(), replacement_metadata.dev());
        assert_eq!(after.ino(), replacement_metadata.ino());
        drop(replacement);
        drop(old_listener);
        fs::remove_file(&path).expect("remove replacement fixture");
    }
    #[test]
    fn unix_socket_path_rejects_attacker_writable_parent() {
        let directory = TestDirectory::new("writable-socket-parent");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o777))
            .expect("make socket parent writable");
        let path = directory.path().join("sora-vpn-backend.sock");
        let error = validate_unix_socket_path(&path)
            .expect_err("world-writable socket parent must fail closed");
        assert!(error.to_string().contains("writable"));
    }
    #[test]
    fn packet_stream_round_trips_fragmented_payload() {
        let packet = vec![0xAB; 1500];
        let encoded = encode_packet_stream_frame(&packet).expect("encode");
        let mut decoder = PacketStreamDecoder::new(packet.len());
        let first = decoder.ingest(&encoded[..700]).expect("first fragment");
        assert!(first.is_empty());
        let second = decoder.ingest(&encoded[700..]).expect("second fragment");
        assert_eq!(second, vec![packet]);
    }
    #[test]
    fn packet_stream_rejects_zero_and_over_mtu_lengths_before_payload_dispatch() {
        let mut decoder = PacketStreamDecoder::new(1280);
        assert!(decoder.ingest(&0u16.to_be_bytes()).is_err());

        let mut decoder = PacketStreamDecoder::new(1280);
        let exact = encode_packet_stream_frame(&vec![0xAB; 1280]).expect("encode exact MTU frame");
        assert_eq!(
            decoder.ingest(&exact).expect("exact MTU frame"),
            vec![vec![0xAB; 1280]]
        );

        let mut decoder = PacketStreamDecoder::new(1280);
        let oversized = 1281u16.to_be_bytes();
        let error = decoder
            .ingest(&oversized)
            .expect_err("MTU + 1 frame must fail from its prefix alone");
        assert!(error.to_string().contains("negotiated"));
        assert!(decoder.buffer.is_empty());
        assert!(decoder.expected_len.is_none());
    }
    #[test]
    fn derive_interface_name_appends_session_suffix() {
        let name = derive_interface_name("svpn", "deadbeefcafebabe").expect("name");
        assert!(name.starts_with("svpn"));
        assert!(name.len() < nix::libc::IFNAMSIZ);
        assert_ne!(name, "svpn");
    }
    fn test_cli() -> Cli {
        Cli {
            endpoint: DEFAULT_BACKEND_ENDPOINT.to_owned(),
            bootstrap_secret_path: None,
            replay_directory: PathBuf::from("/test-only-replay-directory-must-be-overridden"),
            allowed_uid: None,
            allowed_gid: None,
            interface_prefix: DEFAULT_INTERFACE_PREFIX.to_owned(),
            mtu: 1280,
            egress_interface: Some("eth0".to_owned()),
            ipv4_forward: true,
            ipv6_forward: false,
            enable_ipv4_nat: true,
            enable_ipv6_nat: false,
        }
    }
    fn test_backend_config() -> (TestDirectory, BackendConfig) {
        let (secret_directory, secret_path) = write_test_secret(0xA5);
        let replay_directory = secret_directory.path().join("replay");
        let config = BackendConfig::from_cli(Cli {
            bootstrap_secret_path: Some(secret_path),
            replay_directory,
            ..test_cli()
        })
        .expect("test backend config");
        (secret_directory, config)
    }
    #[tokio::test]
    async fn bootstrap_round_trips_from_framed_norito() {
        let bootstrap = VpnBackendBootstrap {
            session_id_hex: "aabbccddaabbccddaabbccddaabbccdd".to_owned(),
            server_tunnel_addresses: vec![
                "10.10.0.1/30".to_owned(),
                "fd53:7261:6574::1/126".to_owned(),
            ],
            client_ipv4_address: [10, 10, 0, 2],
            client_ipv6_address: "fd53:7261:6574::2"
                .parse::<Ipv6Addr>()
                .expect("client IPv6")
                .octets(),
            session_routes: vec!["10.10.0.0/30".to_owned(), "fd53:7261:6574::/126".to_owned()],
            mtu_bytes: 1280,
        };
        let envelope = VpnBackendBootstrapEnvelope {
            bootstrap: bootstrap.clone(),
            timestamp_ms: unix_time_ms().expect("test clock"),
            nonce: [0xA1; 16],
            mac: [0u8; 32],
        };
        let mut envelope = envelope;
        envelope.mac = vpn_backend_bootstrap_mac(
            &[0xA5; 32],
            &envelope.bootstrap,
            envelope.timestamp_ms,
            &envelope.nonce,
        );
        let payload = envelope.encode();
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        writer
            .write_all(VPN_BACKEND_BOOTSTRAP_MAGIC)
            .await
            .expect("magic");
        writer
            .write_all(&(payload.len() as u16).to_be_bytes())
            .await
            .expect("len");
        writer.write_all(&payload).await.expect("payload");
        let (_secret_directory, config) = test_backend_config();
        let decoded = read_vpn_backend_bootstrap(&mut reader, &config)
            .await
            .expect("decoded");
        assert_eq!(decoded, bootstrap);
    }
    #[test]
    fn bootstrap_semantics_bound_privileged_network_operations() {
        let bootstrap = VpnBackendBootstrap {
            session_id_hex: "aabbccddaabbccddaabbccddaabbccdd".to_owned(),
            server_tunnel_addresses: vec![
                "10.10.0.1/30".to_owned(),
                "fd53:7261:6574::1/126".to_owned(),
            ],
            client_ipv4_address: [10, 10, 0, 2],
            client_ipv6_address: "fd53:7261:6574::2"
                .parse::<Ipv6Addr>()
                .expect("client IPv6")
                .octets(),
            session_routes: vec!["10.10.0.0/30".to_owned(), "fd53:7261:6574::/126".to_owned()],
            mtu_bytes: 1280,
        };
        validate_bootstrap_semantics(&bootstrap).expect("bounded canonical bootstrap");

        let mut oversized_routes = bootstrap.clone();
        oversized_routes.session_routes =
            vec!["10.10.0.0/30".to_owned(); VPN_BACKEND_BOOTSTRAP_MAX_SESSION_ROUTES_V1 + 1];
        assert!(validate_bootstrap_semantics(&oversized_routes).is_err());

        let mut oversized_address = bootstrap.clone();
        oversized_address.server_tunnel_addresses = vec![format!(
            "10.10.0.1/30{}",
            "0".repeat(VPN_BACKEND_BOOTSTRAP_MAX_CIDR_BYTES_V1)
        )];
        assert!(validate_bootstrap_semantics(&oversized_address).is_err());

        let mut uppercase_session = bootstrap.clone();
        uppercase_session.session_id_hex = "AABBCCDDAABBCCDDAABBCCDDAABBCCDD".to_owned();
        assert!(validate_bootstrap_semantics(&uppercase_session).is_err());

        let mut outside_authenticated_routes = bootstrap;
        outside_authenticated_routes.client_ipv4_address = [10, 10, 1, 2];
        assert!(validate_bootstrap_semantics(&outside_authenticated_routes).is_err());
    }
    #[tokio::test]
    async fn bootstrap_rejects_bad_mac_and_replay() {
        let secret = [0xA5; 32];
        let (secret_directory, secret_path) = write_test_secret(0xA5);
        let bootstrap = VpnBackendBootstrap {
            session_id_hex: "aabbccddaabbccddaabbccddaabbccdd".to_owned(),
            server_tunnel_addresses: vec![
                "10.10.0.1/30".to_owned(),
                "fd53:7261:6574::1/126".to_owned(),
            ],
            client_ipv4_address: [10, 10, 0, 2],
            client_ipv6_address: "fd53:7261:6574::2"
                .parse::<Ipv6Addr>()
                .expect("client IPv6")
                .octets(),
            session_routes: vec!["10.10.0.0/30".to_owned(), "fd53:7261:6574::/126".to_owned()],
            mtu_bytes: 1280,
        };
        let timestamp_ms = unix_time_ms().expect("test clock");
        let nonce = [0x55; 16];
        let mut envelope = VpnBackendBootstrapEnvelope {
            bootstrap: bootstrap.clone(),
            timestamp_ms,
            nonce,
            mac: [0u8; 32],
        };
        envelope.mac = vpn_backend_bootstrap_mac(&secret, &bootstrap, timestamp_ms, &nonce);
        let config = BackendConfig::from_cli(Cli {
            bootstrap_secret_path: Some(secret_path),
            replay_directory: secret_directory.path().join("replay"),
            ..test_cli()
        })
        .expect("config");
        let mut bad = envelope.clone();
        bad.mac[0] ^= 0xFF;
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        write_bootstrap_test_frame(&mut writer, &bad).await;
        let error = read_vpn_backend_bootstrap(&mut reader, &config)
            .await
            .expect_err("bad mac must fail");
        assert!(error.to_string().contains("MAC"));
        let stale_timestamp = unix_time_ms()
            .expect("test clock")
            .saturating_sub(VPN_BACKEND_BOOTSTRAP_MAX_SKEW_MS + 1);
        let stale_nonce = [0x66; 16];
        let mut stale = VpnBackendBootstrapEnvelope {
            bootstrap: bootstrap.clone(),
            timestamp_ms: stale_timestamp,
            nonce: stale_nonce,
            mac: [0u8; 32],
        };
        stale.mac = vpn_backend_bootstrap_mac(&secret, &bootstrap, stale_timestamp, &stale_nonce);
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        write_bootstrap_test_frame(&mut writer, &stale).await;
        let stale_error = read_vpn_backend_bootstrap(&mut reader, &config)
            .await
            .expect_err("stale timestamp must fail");
        assert!(stale_error.to_string().contains("stale"));
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        write_bootstrap_test_frame(&mut writer, &envelope).await;
        let decoded = read_vpn_backend_bootstrap(&mut reader, &config)
            .await
            .expect("valid bootstrap");
        assert_eq!(decoded, bootstrap);
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        write_bootstrap_test_frame(&mut writer, &envelope).await;
        let replay = read_vpn_backend_bootstrap(&mut reader, &config)
            .await
            .expect_err("replay must fail");
        assert!(replay.to_string().contains("replayed"));
    }
    #[test]
    fn bootstrap_mac_comparison_rejects_every_changed_digest() {
        let expected = [0xA5; 32];
        assert!(bootstrap_mac_matches(&expected, &expected));
        for index in 0..expected.len() {
            let mut changed = expected;
            changed[index] ^= 1;
            assert!(!bootstrap_mac_matches(&expected, &changed));
        }
    }
    #[test]
    fn private_bootstrap_secret_rejects_permissions_and_symlinks() {
        use std::os::unix::fs::symlink;
        let (directory, path) = write_test_secret(0xA5);
        assert_eq!(
            read_bootstrap_secret(&path).expect("private secret"),
            [0xA5; 32]
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))
            .expect("make secret group-readable");
        assert!(read_bootstrap_secret(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("protect secret again");
        let link = directory.path().join("bootstrap-link.hex");
        symlink(&path, &link).expect("create secret symlink");
        assert!(read_bootstrap_secret(&link).is_err());
    }
    #[test]
    fn bootstrap_secret_parser_requires_canonical_nonzero_hex_and_wipes_input() {
        let mut valid = "a5".repeat(32).into_bytes();
        assert_eq!(
            parse_bootstrap_secret(&mut valid).expect("canonical secret"),
            [0xA5; 32]
        );
        assert!(valid.iter().all(|byte| *byte == 0));

        for (contents, expected) in [
            ("A5".repeat(32).into_bytes(), "lowercase"),
            (format!("{}\n", "a5".repeat(32)).into_bytes(), "whitespace"),
            (format!(" {}", "a5".repeat(32)).into_bytes(), "whitespace"),
            ("00".repeat(32).into_bytes(), "all-zero"),
        ] {
            let mut contents = contents;
            let error = parse_bootstrap_secret(&mut contents)
                .expect_err("noncanonical bootstrap secret must fail closed");
            assert!(
                error.to_string().contains(expected),
                "unexpected {expected} error: {error}"
            );
            assert!(contents.iter().all(|byte| *byte == 0));
        }
    }
    #[tokio::test]
    async fn bootstrap_deadline_rejects_stalled_peer() {
        let (_secret_directory, config) = test_backend_config();
        let (_writer, mut reader) = tokio::io::duplex(64);
        let error = read_vpn_backend_bootstrap_with_deadline(
            &mut reader,
            &config,
            Duration::from_millis(10),
        )
        .await
        .expect_err("stalled bootstrap must time out");
        assert!(error.to_string().contains("timed out"));
    }
    #[tokio::test]
    async fn shutdown_cancels_stalled_authenticated_bootstrap() {
        let (_secret_directory, config) = test_backend_config();
        let (_writer, mut reader) = tokio::io::duplex(64);
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        shutdown_tx.send(true).expect("request shutdown");
        let error = read_vpn_backend_bootstrap_until_shutdown(
            &mut reader,
            &config,
            Duration::from_secs(1),
            &mut shutdown_rx,
        )
        .await
        .expect_err("shutdown must cancel a pre-authentication slow peer");
        assert!(error.to_string().contains("shutdown was requested"));
    }
    #[tokio::test]
    async fn shutdown_awaits_all_tracked_session_tasks() {
        let completed = Arc::new(AtomicBool::new(false));
        let completed_task = Arc::clone(&completed);
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let mut sessions = JoinSet::new();
        sessions.spawn(async move {
            wait_for_backend_shutdown(&mut shutdown_rx).await;
            completed_task.store(true, Ordering::Release);
        });
        shutdown_tx.send(true).expect("request shutdown");
        await_session_tasks(&mut sessions).await;
        assert!(completed.load(Ordering::Acquire));
        assert!(sessions.is_empty());
    }
    #[test]
    fn privileged_prepare_steps_fail_after_shutdown_request() {
        let shutdown = AtomicBool::new(false);
        ensure_backend_running(&shutdown).expect("backend initially running");
        shutdown.store(true, Ordering::Release);
        let error = ensure_backend_running(&shutdown)
            .expect_err("each privileged setup step must observe cancellation");
        assert!(error.to_string().contains("shutdown was requested"));
    }
    #[test]
    fn session_permits_fail_closed_at_capacity() {
        let permits = Arc::new(Semaphore::new(1));
        let held = try_session_permit(&permits).expect("first session permit");
        assert!(try_session_permit(&permits).is_none());
        drop(held);
        assert!(try_session_permit(&permits).is_some());
    }
    #[test]
    fn durable_bootstrap_nonce_cache_fails_closed_at_capacity() {
        let parent = TestDirectory::new("durable-replay-capacity");
        let replay_directory = parent.path().join("replay");
        let now = Instant::now();
        let now_ms = 1_500_000;
        let expires_at_ms = now_ms
            + u64::try_from(VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION.as_millis())
                .expect("retention milliseconds");
        let mut replay = DurableBootstrapReplay::open(&replay_directory, now_ms, now)
            .expect("open replay state");
        replay
            .seen
            .nonces
            .try_reserve(VPN_BACKEND_BOOTSTRAP_NONCE_CACHE_CAPACITY_V1)
            .expect("reserve test nonce set");
        replay
            .seen
            .receipts
            .try_reserve(VPN_BACKEND_BOOTSTRAP_NONCE_CACHE_CAPACITY_V1)
            .expect("reserve test receipt queue");
        for index in 0..VPN_BACKEND_BOOTSTRAP_NONCE_CACHE_CAPACITY_V1 {
            let mut nonce = [0u8; 16];
            nonce[..8].copy_from_slice(&(index as u64).to_be_bytes());
            replay.seen.nonces.insert(nonce);
            replay.seen.receipts.push_back((now, nonce, expires_at_ms));
        }
        let full = replay
            .admit([0xFF; 16], now, now_ms)
            .expect_err("full nonce cache must fail closed");
        assert!(full.to_string().contains("capacity"));
    }
    #[test]
    fn bootstrap_nonce_replay_survives_restart_and_clock_rollback_fails_closed() {
        let parent = TestDirectory::new("durable-replay");
        let replay_directory = parent.path().join("replay");
        let receipt = Instant::now();
        let now_ms = 1_000_000;
        let nonce = [0x71; 16];
        let mut replay = DurableBootstrapReplay::open(&replay_directory, now_ms, receipt)
            .expect("open replay state");
        replay
            .admit(nonce, receipt, now_ms)
            .expect("durably admit nonce");
        drop(replay);

        let mut reopened = DurableBootstrapReplay::open(
            &replay_directory,
            now_ms + 1,
            receipt + Duration::from_millis(1),
        )
        .expect("reopen replay state");
        let error = reopened
            .admit(nonce, receipt + Duration::from_millis(1), now_ms + 1)
            .expect_err("restart must not erase nonce replay state");
        assert!(error.to_string().contains("replayed"));
        drop(reopened);

        let error = match DurableBootstrapReplay::open(&replay_directory, now_ms, receipt) {
            Ok(_) => panic!("wall-clock rollback behind durable high-water must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("high-water"));
    }
    #[test]
    fn expired_durable_nonce_is_removed_and_can_be_reused() {
        let parent = TestDirectory::new("durable-replay-expiry");
        let replay_directory = parent.path().join("replay");
        let receipt = Instant::now();
        let now_ms = 2_000_000;
        let retention_ms = u64::try_from(VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION.as_millis())
            .expect("retention milliseconds");
        let nonce = [0x72; 16];
        let mut replay = DurableBootstrapReplay::open(&replay_directory, now_ms, receipt)
            .expect("open replay state");
        replay
            .admit(nonce, receipt, now_ms)
            .expect("durably admit nonce");
        drop(replay);

        let at_expiry_ms = now_ms + retention_ms;
        let at_expiry = receipt + VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION;
        let mut at_boundary =
            DurableBootstrapReplay::open(&replay_directory, at_expiry_ms, at_expiry)
                .expect("exact inclusive replay boundary must retain the nonce");
        let replay_error = at_boundary
            .admit(nonce, at_expiry, at_expiry_ms)
            .expect_err("nonce must remain blocked at the inclusive replay boundary");
        assert!(replay_error.to_string().contains("replayed"));
        drop(at_boundary);

        let after_expiry_ms = now_ms + retention_ms + 1;
        let after_expiry =
            receipt + VPN_BACKEND_BOOTSTRAP_NONCE_RETENTION + Duration::from_millis(1);
        let mut reopened =
            DurableBootstrapReplay::open(&replay_directory, after_expiry_ms, after_expiry)
                .expect("expired replay entry should be pruned");
        reopened
            .admit(nonce, after_expiry, after_expiry_ms)
            .expect("expired nonce may be used again only after both skew edges");
    }
    #[cfg(unix)]
    #[test]
    fn bootstrap_replay_directory_rejects_symlinks_and_permissive_custody() {
        use std::os::unix::fs::symlink;

        let parent = TestDirectory::new("durable-replay-custody");
        let target = parent.path().join("target");
        fs::create_dir(&target).expect("create target");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).expect("protect target");
        let link = parent.path().join("link");
        symlink(&target, &link).expect("create replay directory symlink");
        assert!(prepare_private_replay_directory(&link).is_err());

        let permissive = parent.path().join("permissive");
        fs::create_dir(&permissive).expect("create permissive directory");
        fs::set_permissions(&permissive, fs::Permissions::from_mode(0o750))
            .expect("set permissive mode");
        assert!(prepare_private_replay_directory(&permissive).is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn unix_peer_credentials_reject_unauthorized_peer() {
        let (stream, _peer) = UnixStream::pair().expect("unix stream pair");
        let error = verify_unix_peer_credentials(&stream, Some(u32::MAX), None)
            .expect_err("unexpected uid must fail");
        assert!(
            error.to_string().contains("not allowed")
                || error.to_string().contains("not supported")
        );
    }
    async fn write_bootstrap_test_frame<W: AsyncWrite + Unpin>(
        writer: &mut W,
        envelope: &VpnBackendBootstrapEnvelope,
    ) {
        let payload = envelope.encode();
        writer
            .write_all(VPN_BACKEND_BOOTSTRAP_MAGIC)
            .await
            .expect("magic");
        writer
            .write_all(&(payload.len() as u16).to_be_bytes())
            .await
            .expect("len");
        writer.write_all(&payload).await.expect("payload");
    }
    #[tokio::test]
    async fn status_frame_round_trips() {
        let (mut writer, mut reader) = tokio::io::duplex(256);
        write_vpn_backend_status(&mut writer, true, "ready")
            .await
            .expect("status write");
        let mut status = [0u8; 1];
        reader.read_exact(&mut status).await.expect("status");
        assert_eq!(status[0], VPN_BACKEND_STATUS_READY);
        let mut len = [0u8; 2];
        reader.read_exact(&mut len).await.expect("len");
        let len = usize::from(u16::from_be_bytes(len));
        let mut payload = vec![0u8; len];
        reader.read_exact(&mut payload).await.expect("payload");
        assert_eq!(payload, b"ready");
    }
    #[tokio::test]
    async fn ready_status_write_failure_releases_prepared_resource() {
        let (mut writer, reader) = tokio::io::duplex(64);
        drop(reader);
        let cleaned = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cleanup_observer = Arc::clone(&cleaned);
        let mut prepared = Some(());
        let _error = write_ready_status_or_cleanup(&mut writer, &mut prepared, move |()| {
            cleanup_observer.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await
        .expect_err("disconnected peer must fail the ready status write");
        assert!(prepared.is_none());
        assert!(cleaned.load(std::sync::atomic::Ordering::SeqCst));
    }
    #[test]
    fn session_runtime_config_uses_bootstrap_values() {
        let (_secret_directory, config) = test_backend_config();
        let session = SessionRuntimeConfig::from_bootstrap(
            &config,
            VpnBackendBootstrap {
                session_id_hex: "aabbccddeeff00112233445566778899".to_owned(),
                server_tunnel_addresses: vec![
                    "10.10.0.1/30".to_owned(),
                    "fd53:7261:6574::1/126".to_owned(),
                ],
                client_ipv4_address: [10, 10, 0, 2],
                client_ipv6_address: "fd53:7261:6574::2"
                    .parse::<Ipv6Addr>()
                    .expect("client IPv6")
                    .octets(),
                session_routes: vec!["10.10.0.0/30".to_owned(), "fd53:7261:6574::/126".to_owned()],
                mtu_bytes: 1400,
            },
        )
        .expect("session config");
        assert_eq!(session.mtu, 1400);
        assert_eq!(session.tunnel_addresses.len(), 2);
        assert_eq!(
            session.nat_cidrs,
            vec![
                parse_cidr("10.10.0.2/32").expect("exact client IPv4 CIDR"),
                parse_cidr("fd53:7261:6574::2/128").expect("exact client IPv6 CIDR"),
            ]
        );
        assert!(session.interface_name.starts_with(DEFAULT_INTERFACE_PREFIX));
    }
}
