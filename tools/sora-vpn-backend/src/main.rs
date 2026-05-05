use std::{
    env,
    ffi::OsStr,
    fs, io,
    net::{IpAddr, SocketAddr},
    process::{Command as ProcessCommand, ExitCode},
    sync::{Arc, Mutex},
};
#[cfg(target_os = "linux")]
use std::{ffi::CStr, os::fd::FromRawFd};

use clap::Parser;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, unix::AsyncFd},
    net::{TcpListener, TcpStream},
    signal::unix::{SignalKind, signal},
};

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:19090";
const DEFAULT_INTERFACE_PREFIX: &str = "svpn";
const DEFAULT_ROUTE_CMD: &str = "ip";
const PACKET_LEN_PREFIX_BYTES: usize = 2;
const VPN_BACKEND_BOOTSTRAP_MAGIC: &[u8; 8] = b"SVPNBE1\0";
const VPN_BACKEND_STATUS_READY: u8 = 1;
#[cfg(target_os = "linux")]
const LINUX_IFF_TUN: nix::libc::c_short = 0x0001;
#[cfg(target_os = "linux")]
const LINUX_IFF_NO_PI: nix::libc::c_short = 0x1000;
#[cfg(target_os = "linux")]
const LINUX_TUNSETIFF: nix::libc::c_ulong = 0x4004_54ca;

#[derive(Debug, Parser)]
#[command(name = "sora-vpn-backend")]
struct Cli {
    #[arg(long, env = "SORANET_VPN_BACKEND_LISTEN_ADDR", default_value = DEFAULT_LISTEN_ADDR)]
    listen: SocketAddr,
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

#[derive(Debug, Clone)]
struct BackendConfig {
    listen: SocketAddr,
    interface_prefix: String,
    default_mtu: u16,
    ipv4_forward: bool,
    ipv6_forward: bool,
    enable_ipv4_nat: bool,
    enable_ipv6_nat: bool,
    egress_v4_interface: Option<String>,
    egress_v6_interface: Option<String>,
}

impl BackendConfig {
    fn from_cli(cli: Cli) -> Result<Self, BackendError> {
        let interface_prefix = trim_to_option(&cli.interface_prefix).ok_or_else(|| {
            BackendError::InvalidConfig("interface_prefix must not be empty".to_owned())
        })?;
        if interface_prefix.len() >= nix::libc::IFNAMSIZ {
            return Err(BackendError::InvalidConfig(format!(
                "interface_prefix `{interface_prefix}` exceeds Linux IFNAMSIZ"
            )));
        }

        let default_mtu = normalize_mtu(cli.mtu)?;
        let egress_interface = cli
            .egress_interface
            .as_deref()
            .and_then(trim_to_option)
            .map(|value| value.to_owned());
        let egress_v4_interface = if cli.enable_ipv4_nat {
            Some(
                resolve_egress_interface(egress_interface.as_deref(), IpFamily::V4)?.ok_or_else(
                    || {
                        BackendError::InvalidConfig(
                            "could not resolve IPv4 default egress interface".to_owned(),
                        )
                    },
                )?,
            )
        } else {
            None
        };
        let egress_v6_interface = if cli.enable_ipv6_nat {
            Some(
                resolve_egress_interface(egress_interface.as_deref(), IpFamily::V6)?.ok_or_else(
                    || {
                        BackendError::InvalidConfig(
                            "could not resolve IPv6 default egress interface".to_owned(),
                        )
                    },
                )?,
            )
        } else {
            None
        };

        Ok(Self {
            listen: cli.listen,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct VpnBackendBootstrap {
    session_id_hex: String,
    server_tunnel_addresses: Vec<String>,
    session_routes: Vec<String>,
    mtu_bytes: u16,
}

#[derive(Debug, Clone)]
struct SessionRuntimeConfig {
    interface_name: String,
    mtu: u16,
    tunnel_addresses: Vec<ParsedCidr>,
    session_routes: Vec<ParsedCidr>,
    nat_cidrs: Vec<ParsedCidr>,
}

impl SessionRuntimeConfig {
    fn from_bootstrap(
        config: &BackendConfig,
        bootstrap: VpnBackendBootstrap,
    ) -> Result<Self, BackendError> {
        let session_id_hex = trim_to_option(&bootstrap.session_id_hex).ok_or_else(|| {
            BackendError::InvalidConfig("bootstrap session_id_hex must not be empty".to_owned())
        })?;
        let interface_name = derive_interface_name(&config.interface_prefix, &session_id_hex)?;
        let tunnel_addresses = parse_cidr_list(&bootstrap.server_tunnel_addresses)?;
        let session_routes = parse_cidr_list(&bootstrap.session_routes)?;
        let nat_cidrs = session_routes.clone();
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
        })
    }
}

#[derive(Debug, Clone)]
struct PreparedTunnel {
    device: Arc<LinuxTunDevice>,
    applied_network: AppliedNetworkState,
    packet_read_mtu: usize,
}

#[derive(Debug, Clone)]
struct AppliedNetworkState {
    interface_name: String,
    forwarding_leases: Vec<IpFamily>,
    nat_rules: Vec<NatRule>,
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

#[derive(Debug)]
struct LinuxTunDevice {
    file: AsyncFd<fs::File>,
    name: String,
}

#[derive(Debug, Default)]
struct PacketStreamDecoder {
    buffer: Vec<u8>,
    expected_len: Option<usize>,
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
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("backend state error: {0}")]
    State(String),
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
    let config = BackendConfig::from_cli(cli)?;
    let listener = TcpListener::bind(config.listen).await?;
    let shared_network = Arc::new(Mutex::new(SharedNetworkState::default()));
    eprintln!(
        "sora-vpn-backend listening on {} with interface prefix {}",
        config.listen, config.interface_prefix
    );

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    loop {
        tokio::select! {
            _ = sigterm.recv() => return Ok(()),
            _ = sigint.recv() => return Ok(()),
            accept = listener.accept() => {
                let (stream, remote) = accept?;
                let session_config = config.clone();
                let session_shared = Arc::clone(&shared_network);
                tokio::spawn(async move {
                    if let Err(error) = serve_connection(stream, remote, &session_config, &session_shared).await {
                        eprintln!("vpn backend session from {remote} failed: {error}");
                    }
                });
            }
        }
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    remote: SocketAddr,
    config: &BackendConfig,
    shared_network: &Arc<Mutex<SharedNetworkState>>,
) -> Result<(), BackendError> {
    let bootstrap = read_vpn_backend_bootstrap(&mut stream).await?;
    let session_config = match SessionRuntimeConfig::from_bootstrap(config, bootstrap) {
        Ok(session_config) => session_config,
        Err(error) => {
            let _ = write_vpn_backend_status(&mut stream, false, &error.to_string()).await;
            return Err(error);
        }
    };
    let prepared = match prepare_tunnel(config, &session_config, shared_network) {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = write_vpn_backend_status(&mut stream, false, &error.to_string()).await;
            return Err(error);
        }
    };
    write_vpn_backend_status(&mut stream, true, "ready").await?;
    eprintln!(
        "vpn backend accepted {remote} on interface {}",
        prepared.applied_network.interface_name
    );

    let session_result = backend_packet_loop(
        Arc::clone(&prepared.device),
        stream,
        prepared.packet_read_mtu,
    )
    .await;
    let cleanup_result = cleanup_tunnel(prepared, shared_network);

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
) -> Result<PreparedTunnel, BackendError> {
    let device = Arc::new(LinuxTunDevice::create(&session_config.interface_name)?);
    let mut applied_network = AppliedNetworkState {
        interface_name: device.name().to_owned(),
        forwarding_leases: Vec::new(),
        nat_rules: Vec::new(),
    };

    if let Err(error) = apply_tunnel_link_config(
        &applied_network.interface_name,
        session_config.mtu,
        &session_config.tunnel_addresses,
    ) {
        let _ = cleanup_network(&applied_network, shared_network);
        return Err(error);
    }
    if let Err(error) = apply_client_routes(
        &applied_network.interface_name,
        &session_config.session_routes,
    ) {
        let _ = cleanup_network(&applied_network, shared_network);
        return Err(error);
    }

    if config.ipv4_forward && has_family(&session_config.session_routes, IpFamily::V4) {
        match acquire_forwarding(shared_network, IpFamily::V4) {
            Ok(()) => applied_network.forwarding_leases.push(IpFamily::V4),
            Err(error) => {
                let _ = cleanup_network(&applied_network, shared_network);
                return Err(error);
            }
        }
    }
    if config.ipv6_forward && has_family(&session_config.session_routes, IpFamily::V6) {
        match acquire_forwarding(shared_network, IpFamily::V6) {
            Ok(()) => applied_network.forwarding_leases.push(IpFamily::V6),
            Err(error) => {
                let _ = cleanup_network(&applied_network, shared_network);
                return Err(error);
            }
        }
    }

    if config.enable_ipv4_nat {
        let Some(egress_interface) = config.egress_v4_interface.as_ref() else {
            let _ = cleanup_network(&applied_network, shared_network);
            return Err(BackendError::InvalidConfig(
                "IPv4 NAT enabled without a resolved egress interface".to_owned(),
            ));
        };
        for cidr in session_config
            .nat_cidrs
            .iter()
            .filter(|cidr| cidr.family() == IpFamily::V4)
        {
            match apply_nat_rule(IpFamily::V4, &cidr.render(), egress_interface) {
                Ok(Some(rule)) => applied_network.nat_rules.push(rule),
                Ok(None) => {}
                Err(error) => {
                    let _ = cleanup_network(&applied_network, shared_network);
                    return Err(error);
                }
            }
        }
    }
    if config.enable_ipv6_nat {
        let Some(egress_interface) = config.egress_v6_interface.as_ref() else {
            let _ = cleanup_network(&applied_network, shared_network);
            return Err(BackendError::InvalidConfig(
                "IPv6 NAT enabled without a resolved egress interface".to_owned(),
            ));
        };
        for cidr in session_config
            .nat_cidrs
            .iter()
            .filter(|cidr| cidr.family() == IpFamily::V6)
        {
            match apply_nat_rule(IpFamily::V6, &cidr.render(), egress_interface) {
                Ok(Some(rule)) => applied_network.nat_rules.push(rule),
                Ok(None) => {}
                Err(error) => {
                    let _ = cleanup_network(&applied_network, shared_network);
                    return Err(error);
                }
            }
        }
    }

    Ok(PreparedTunnel {
        device,
        applied_network,
        packet_read_mtu: usize::from(session_config.mtu),
    })
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
    for rule in applied.nat_rules.iter().rev() {
        remove_nat_rule(rule)?;
    }
    for family in applied.forwarding_leases.iter().rev() {
        release_forwarding(shared_network, *family)?;
    }
    let down_result = run_command(
        DEFAULT_ROUTE_CMD,
        vec![
            "link".to_owned(),
            "set".to_owned(),
            "dev".to_owned(),
            applied.interface_name.clone(),
            "down".to_owned(),
        ],
    );
    match down_result {
        Ok(_) => Ok(()),
        Err(BackendError::State(message))
            if message.contains("Cannot find device") || message.contains("does not exist") =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

async fn backend_packet_loop(
    device: Arc<LinuxTunDevice>,
    stream: TcpStream,
    packet_read_mtu: usize,
) -> Result<(), BackendError> {
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let (mut reader, mut writer) = stream.into_split();

    let upstream = tun_to_socket_loop(Arc::clone(&device), &mut writer, packet_read_mtu);
    let downstream = socket_to_tun_loop(device, &mut reader);
    tokio::pin!(upstream);
    tokio::pin!(downstream);

    tokio::select! {
        _ = sigterm.recv() => Ok(()),
        _ = sigint.recv() => Ok(()),
        result = &mut upstream => result,
        result = &mut downstream => result,
    }
}

async fn read_vpn_backend_bootstrap<R: AsyncRead + Unpin>(
    reader: &mut R,
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
    serde_json::from_slice(&payload)
        .map_err(|error| BackendError::InvalidConfig(format!("invalid backend bootstrap: {error}")))
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
    writer
        .write_all(&[if ready { VPN_BACKEND_STATUS_READY } else { 0u8 }])
        .await?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(payload).await?;
    Ok(())
}

async fn tun_to_socket_loop<W: AsyncWriteExt + Unpin>(
    device: Arc<LinuxTunDevice>,
    writer: &mut W,
    packet_read_mtu: usize,
) -> Result<(), BackendError> {
    let mut packet_buf = vec![0u8; packet_read_mtu.max(512)];
    loop {
        let packet_len = device.recv(&mut packet_buf).await?;
        if packet_len == 0 {
            continue;
        }
        let encoded = encode_packet_stream_frame(&packet_buf[..packet_len])?;
        writer.write_all(&encoded).await?;
    }
}

async fn socket_to_tun_loop<R: AsyncReadExt + Unpin>(
    device: Arc<LinuxTunDevice>,
    reader: &mut R,
) -> Result<(), BackendError> {
    let mut decoder = PacketStreamDecoder::default();
    let mut buf = vec![0u8; 4096];
    loop {
        let read = reader.read(&mut buf).await?;
        if read == 0 {
            return Ok(());
        }
        for packet in decoder.ingest(&buf[..read])? {
            device.send(&packet).await?;
        }
    }
}

impl PacketStreamDecoder {
    fn ingest(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, BackendError> {
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
            req.ifr_ifru.ifru_flags = (LINUX_IFF_TUN | LINUX_IFF_NO_PI) as _;
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
) -> Result<(), BackendError> {
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
                address.render(),
                "dev".to_owned(),
                interface_name.to_owned(),
            ],
        )?;
    }
    Ok(())
}

fn apply_client_routes(interface_name: &str, routes: &[ParsedCidr]) -> Result<(), BackendError> {
    for route in routes {
        run_command(
            DEFAULT_ROUTE_CMD,
            vec![
                route.family().flag().to_owned(),
                "route".to_owned(),
                "replace".to_owned(),
                route.render(),
                "dev".to_owned(),
                interface_name.to_owned(),
            ],
        )?;
    }
    Ok(())
}

fn acquire_forwarding(
    shared_network: &Arc<Mutex<SharedNetworkState>>,
    family: IpFamily,
) -> Result<(), BackendError> {
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
    let previous_value = reservation.previous_value.clone();
    *slot = None;
    drop(guard);
    if previous_value.is_empty() {
        return Ok(());
    }
    run_command(
        "sysctl",
        vec![
            "-w".to_owned(),
            format!("{}={}", family.forwarding_key(), previous_value),
        ],
    )?;
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

fn apply_nat_rule(
    family: IpFamily,
    source_cidr: &str,
    egress_interface: &str,
) -> Result<Option<NatRule>, BackendError> {
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
    if run_command(program, args).is_err() {
        run_command(program, nat_rule_args("-A", source_cidr, egress_interface))?;
    }

    Ok(Some(NatRule {
        family,
        source_cidr: source_cidr.to_owned(),
        egress_interface: egress_interface.to_owned(),
    }))
}

fn remove_nat_rule(rule: &NatRule) -> Result<(), BackendError> {
    let result = run_command(
        rule.family.nat_program(),
        nat_rule_args("-D", &rule.source_cidr, &rule.egress_interface),
    );
    match result {
        Ok(_) => Ok(()),
        Err(BackendError::State(message))
            if message.contains("Bad rule")
                || message.contains("No chain/target/match")
                || message.contains("does a matching rule exist") =>
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
    if let Some(value) = override_interface.and_then(trim_to_option) {
        return Ok(Some(value));
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
    Ok(output.lines().find_map(parse_route_device))
}

fn parse_route_device(line: &str) -> Option<String> {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    let mut idx = 0usize;
    while idx < tokens.len() {
        if tokens[idx] == "dev" && idx + 1 < tokens.len() {
            return Some(tokens[idx + 1].to_owned());
        }
        idx += 1;
    }
    None
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
    let normalized_prefix = trim_to_option(prefix).ok_or_else(|| {
        BackendError::InvalidConfig("interface prefix must not be empty".to_owned())
    })?;
    if normalized_prefix.len() >= nix::libc::IFNAMSIZ {
        return Err(BackendError::InvalidConfig(format!(
            "interface prefix `{normalized_prefix}` exceeds Linux IFNAMSIZ"
        )));
    }
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

fn has_family(values: &[ParsedCidr], family: IpFamily) -> bool {
    values.iter().any(|value| value.family() == family)
}

fn trim_to_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn command_exists(program: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|dir| dir.join(program).exists())
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
    let output = ProcessCommand::new(program).args(&collected).output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let detail = if stderr.is_empty() {
        format!("exit status {}", output.status)
    } else {
        stderr
    };
    Err(BackendError::State(format!(
        "{program} {} failed: {detail}",
        collected.join(" ")
    )))
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

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
        let device = parse_route_device("default via 192.168.1.1 dev eth0 proto dhcp metric 100");
        assert_eq!(device.as_deref(), Some("eth0"));
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
    fn derive_interface_name_appends_session_suffix() {
        let name = derive_interface_name("svpn", "deadbeefcafebabe").expect("name");
        assert!(name.starts_with("svpn"));
        assert!(name.len() < nix::libc::IFNAMSIZ);
        assert_ne!(name, "svpn");
    }

    #[tokio::test]
    async fn bootstrap_round_trips_from_framed_json() {
        let bootstrap = VpnBackendBootstrap {
            session_id_hex: "aabbccdd".to_owned(),
            server_tunnel_addresses: vec!["10.10.0.1/30".to_owned()],
            session_routes: vec!["10.10.0.0/30".to_owned()],
            mtu_bytes: 1280,
        };
        let payload = serde_json::to_vec(&bootstrap).expect("json");
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

        let decoded = read_vpn_backend_bootstrap(&mut reader)
            .await
            .expect("decoded");
        assert_eq!(decoded, bootstrap);
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

    #[test]
    fn session_runtime_config_uses_bootstrap_values() {
        let config = BackendConfig::from_cli(Cli {
            listen: DEFAULT_LISTEN_ADDR.parse().expect("listen addr"),
            interface_prefix: DEFAULT_INTERFACE_PREFIX.to_owned(),
            mtu: 1280,
            egress_interface: Some("eth0".to_owned()),
            ipv4_forward: true,
            ipv6_forward: false,
            enable_ipv4_nat: true,
            enable_ipv6_nat: false,
        })
        .expect("config");

        let session = SessionRuntimeConfig::from_bootstrap(
            &config,
            VpnBackendBootstrap {
                session_id_hex: "aabbccddeeff".to_owned(),
                server_tunnel_addresses: vec![
                    "10.10.0.1/30".to_owned(),
                    "fd53:7261:6574::1/126".to_owned(),
                ],
                session_routes: vec!["10.10.0.0/30".to_owned(), "fd53:7261:6574::/126".to_owned()],
                mtu_bytes: 1400,
            },
        )
        .expect("session config");

        assert_eq!(session.mtu, 1400);
        assert_eq!(session.tunnel_addresses.len(), 2);
        assert_eq!(session.nat_cidrs, session.session_routes);
        assert!(session.interface_name.starts_with(DEFAULT_INTERFACE_PREFIX));
    }
}
