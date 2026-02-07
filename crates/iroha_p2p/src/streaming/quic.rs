#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::missing_errors_doc,
    clippy::ignored_unit_patterns,
    clippy::unused_async
)]

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use norito::{
    deserialize_from,
    streaming::{
        self, CapabilityAck, CapabilityReport, ControlFrame, TransportCapabilities,
        TransportCapabilitiesFrame, TransportCapabilityError, TransportCapabilityResolution,
    },
    to_bytes,
};
use quinn::{
    self, ClientConfig, ConnectError, Connection, ConnectionError, Endpoint, IdleTimeout,
    ReadExactError, RecvStream, SendDatagramError, SendStream, ServerConfig, TransportConfig,
    VarInt,
    crypto::rustls::{
        QuicClientConfig as QuinnRustlsClientConfig, QuicServerConfig as QuinnRustlsServerConfig,
    },
};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CONTROL_STREAM_PREFACE: &[u8; 5] = b"NSC/1";
const CONTROL_TYPE_PUBLISHER_TO_VIEWER: u8 = 0x01;
const CONTROL_TYPE_VIEWER_TO_PUBLISHER: u8 = 0x02;
const DEFAULT_MAX_DATAGRAM_SIZE: usize = 1350;
const DEFAULT_DATAGRAM_BUFFER: usize = 1 << 20;
const MAX_CONTROL_FRAME_LEN: usize = 512 * 1024;
const ALPN: &[u8] = b"nsc/1";
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Result type used by the streaming QUIC helpers.
pub type Result<T> = core::result::Result<T, Error>;

/// Errors produced by the streaming QUIC transport helpers.
#[derive(Debug, Error)]
pub enum Error {
    /// Multiaddr string could not be parsed.
    #[error("invalid NSC multiaddr: {0}")]
    InvalidMultiaddr(String),
    /// Multiaddr did not advertise a UDP port.
    #[error("multiaddr is missing UDP segment")]
    MissingPort,
    /// Multiaddr contained an unsupported protocol component.
    #[error("unsupported multiaddr protocol '{0}'")]
    UnsupportedProtocol(String),
    /// I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// QUIC connection failure.
    #[error("QUIC connection error: {0}")]
    Connection(#[from] ConnectionError),
    /// QUIC connect-time failure.
    #[error("QUIC connect error: {0}")]
    Connect(#[from] ConnectError),
    /// Failed to send a datagram.
    #[error("failed to send datagram: {0}")]
    SendDatagram(#[from] SendDatagramError),
    /// Norito codec failure.
    #[error("Norito codec error: {0}")]
    Norito(#[from] norito::Error),
    /// Control-stream preface was missing or malformed.
    #[error("control stream preface mismatch")]
    BadPreface,
    /// Remote advertised an unexpected control-stream direction.
    #[error("unexpected control stream direction (expected {expected:?}, found {found:?})")]
    WrongDirection {
        /// Expected direction marker.
        expected: ControlStreamDirection,
        /// Marker advertised by the peer.
        found: ControlStreamDirection,
    },
    /// Control frame exceeded the configured maximum length.
    #[error("control frame length {len} exceeds maximum {max}")]
    FrameTooLarge {
        /// Reported frame length.
        len: usize,
        /// Maximum allowed frame length.
        max: usize,
    },
    /// Transport capability negotiation failed.
    #[error("transport capability negotiation failed: {0}")]
    TransportCapability(TransportCapabilityError),
    /// Control stream ended unexpectedly.
    #[error("control stream closed by peer")]
    ControlStreamClosed,
    /// TLS client configuration failure.
    #[error("TLS client configuration error: {0}")]
    TlsClient(String),
    /// TLS server configuration failure.
    #[error("TLS server configuration error: {0}")]
    TlsServer(String),
    /// Transport configuration failure.
    #[error("transport configuration error: {0}")]
    TransportConfig(String),
    /// Datagrams larger than the negotiated bound were rejected.
    #[error("datagram size {len} exceeds negotiated maximum {max}")]
    DatagramTooLarge {
        /// Attempted datagram size.
        len: usize,
        /// Negotiated limit.
        max: usize,
    },
    /// Peer delivered an unexpected control frame while a specific response was required.
    #[error("protocol violation: {0}")]
    ProtocolViolation(String),
}

/// Direction of the dedicated QUIC control stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlStreamDirection {
    /// Publisher → Viewer control flow.
    PublisherToViewer,
    /// Viewer → Publisher control flow.
    ViewerToPublisher,
}

impl ControlStreamDirection {
    const fn marker(self) -> u8 {
        match self {
            Self::PublisherToViewer => CONTROL_TYPE_PUBLISHER_TO_VIEWER,
            Self::ViewerToPublisher => CONTROL_TYPE_VIEWER_TO_PUBLISHER,
        }
    }

    fn from_marker(marker: u8) -> Option<Self> {
        match marker {
            CONTROL_TYPE_PUBLISHER_TO_VIEWER => Some(Self::PublisherToViewer),
            CONTROL_TYPE_VIEWER_TO_PUBLISHER => Some(Self::ViewerToPublisher),
            _ => None,
        }
    }

    const fn opposite(self) -> Self {
        match self {
            Self::PublisherToViewer => Self::ViewerToPublisher,
            Self::ViewerToPublisher => Self::PublisherToViewer,
        }
    }
}

/// Local endpoint role (publisher or viewer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointRole {
    /// Publisher side of the session.
    Publisher,
    /// Viewer side of the session.
    Viewer,
}

impl EndpointRole {
    const fn outgoing_direction(self) -> ControlStreamDirection {
        match self {
            Self::Publisher => ControlStreamDirection::PublisherToViewer,
            Self::Viewer => ControlStreamDirection::ViewerToPublisher,
        }
    }

    const fn incoming_direction(self) -> ControlStreamDirection {
        self.outgoing_direction().opposite()
    }
}

/// Transport tuning knobs shared by viewers and publishers.
#[derive(Clone, Copy, Debug)]
pub struct TransportConfigSettings {
    /// Maximum QUIC DATAGRAM payload size (after AEAD).
    pub max_datagram_size: usize,
    /// Total receive buffer reserved for datagrams.
    pub datagram_receive_buffer: usize,
    /// Total send buffer reserved for datagrams.
    pub datagram_send_buffer: usize,
    /// Idle timeout advertised at the transport layer.
    pub idle_timeout: Duration,
}

impl Default for TransportConfigSettings {
    fn default() -> Self {
        Self {
            max_datagram_size: DEFAULT_MAX_DATAGRAM_SIZE,
            datagram_receive_buffer: DEFAULT_DATAGRAM_BUFFER,
            datagram_send_buffer: DEFAULT_DATAGRAM_BUFFER,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }
}

/// Listener for incoming viewer connections.
#[derive(Clone)]
pub struct StreamingServer {
    endpoint: Endpoint,
    settings: TransportConfigSettings,
}

impl StreamingServer {
    /// Bind a QUIC listener on `addr` using the provided transport settings.
    pub async fn bind(addr: SocketAddr, settings: TransportConfigSettings) -> Result<Self> {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["nsc.local".into()])
                .map_err(|e| Error::TlsServer(e.to_string()))?;
        let cert_der = cert.der().clone();

        let cert_chain = vec![cert_der];
        let priv_key: PrivateKeyDer<'static> =
            PrivateKeyDer::from(PrivatePkcs8KeyDer::from(signing_key.serialize_der())).clone_key();

        let mut rustls_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, priv_key)
            .map_err(|e| Error::TlsServer(e.to_string()))?;
        rustls_config.max_early_data_size = u32::MAX;
        rustls_config.alpn_protocols = vec![ALPN.to_vec()];
        let rustls_config = Arc::new(rustls_config);

        let crypto = QuinnRustlsServerConfig::try_from(rustls_config)
            .map_err(|e| Error::TlsServer(e.to_string()))?;

        let mut server_config = ServerConfig::with_crypto(Arc::new(crypto));
        let transport = build_transport_config(settings)?;
        server_config.transport_config(transport);

        let endpoint = Endpoint::server(server_config, addr)?;

        Ok(Self { endpoint, settings })
    }

    /// Retrieve the socket address the server is listening on.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.endpoint.local_addr().map_err(Error::from)
    }

    /// Accept the next inbound streaming connection.
    pub async fn accept(&self) -> Result<StreamingConnection> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| Error::ProtocolViolation("listener closed".into()))?;
        let connection = incoming.await?;
        StreamingConnection::new(connection, EndpointRole::Publisher, self.settings).await
    }

    /// Close the listener and wait for active connections to drain.
    pub async fn shutdown(&self) {
        self.endpoint.close(VarInt::from_u32(0), &[]);
        self.endpoint.wait_idle().await;
    }
}

/// Viewer-side QUIC connector.
pub struct StreamingClient {
    endpoint: Endpoint,
    connection: StreamingConnection,
}

impl StreamingClient {
    /// Connect to the remote publisher described by `multiaddr`.
    pub async fn connect(multiaddr: &str, settings: TransportConfigSettings) -> Result<Self> {
        let parsed = parse_multiaddr(multiaddr)?;
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())?;

        let verifier: Arc<dyn ServerCertVerifier> = Arc::new(NoCertificateVerification);
        let mut tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        tls_config.enable_early_data = true;
        tls_config.alpn_protocols = vec![ALPN.to_vec()];
        let tls_config = Arc::new(tls_config);
        let crypto = QuinnRustlsClientConfig::try_from(Arc::clone(&tls_config))
            .map_err(|e| Error::TlsClient(e.to_string()))?;
        let mut client_config = ClientConfig::new(Arc::new(crypto));
        let transport = build_transport_config(settings)?;
        client_config.transport_config(transport);
        endpoint.set_default_client_config(client_config);

        let server_addr = SocketAddr::new(parsed.host, parsed.port);
        let connecting = endpoint.connect(server_addr, &parsed.server_name)?;
        let connection = connecting.await?;
        let connection =
            StreamingConnection::new(connection, EndpointRole::Viewer, settings).await?;

        Ok(Self {
            endpoint,
            connection,
        })
    }

    /// Access the underlying connection mutably.
    pub fn connection(&mut self) -> &mut StreamingConnection {
        &mut self.connection
    }

    /// Close the connection and wait for idle shutdown.
    pub async fn close(self) {
        self.connection.close();
        self.endpoint.close(VarInt::from_u32(0), &[]);
        self.endpoint.wait_idle().await;
    }
}

/// Active streaming session over QUIC.
pub struct StreamingConnection {
    role: EndpointRole,
    connection: Connection,
    control_send: ControlStreamWriter,
    control_recv: ControlStreamReader,
    max_datagram: usize,
}

impl StreamingConnection {
    async fn new(
        connection: Connection,
        role: EndpointRole,
        settings: TransportConfigSettings,
    ) -> Result<Self> {
        let send = connection
            .open_uni()
            .await
            .map_err(|e| Error::Io(std::io::Error::from(e)))?;
        let recv = connection.accept_uni().await?;
        let control_send = ControlStreamWriter::new(send, role.outgoing_direction()).await?;
        let control_recv = ControlStreamReader::new(recv, role.incoming_direction()).await?;

        Ok(Self {
            role,
            connection,
            control_send,
            control_recv,
            max_datagram: settings.max_datagram_size,
        })
    }

    /// Return the local role.
    pub const fn role(&self) -> EndpointRole {
        self.role
    }

    /// Send a control frame to the peer.
    pub async fn send_control_frame(&mut self, frame: &ControlFrame) -> Result<()> {
        self.control_send.send_frame(frame).await
    }

    /// Receive the next control frame from the peer.
    pub async fn next_control_frame(&mut self) -> Result<ControlFrame> {
        self.control_recv.next_frame().await
    }

    /// Send a datagram payload.
    pub async fn send_datagram(&self, payload: &[u8]) -> Result<()> {
        if payload.len() > self.max_datagram {
            return Err(Error::DatagramTooLarge {
                len: payload.len(),
                max: self.max_datagram,
            });
        }
        self.connection
            .send_datagram(Bytes::copy_from_slice(payload))?;
        Ok(())
    }

    /// Receive the next datagram payload.
    pub async fn recv_datagram(&self) -> Result<Bytes> {
        Ok(self.connection.read_datagram().await?)
    }

    /// Return the negotiated DATAGRAM payload limit for this session.
    pub const fn max_datagram_size(&self) -> usize {
        self.max_datagram
    }

    /// Return `true` if DATAGRAM delivery is enabled for this session.
    pub const fn datagram_enabled(&self) -> bool {
        self.max_datagram > 0
    }

    /// Close the underlying QUIC connection.
    pub fn close(&self) {
        self.connection.close(VarInt::from_u32(0), &[]);
    }

    /// Borrow the underlying QUIC connection.
    pub fn quic_connection(&self) -> &Connection {
        &self.connection
    }

    fn apply_transport_resolution(&mut self, resolution: TransportCapabilityResolution) {
        let negotiated = if resolution.use_datagram {
            usize::from(resolution.max_segment_datagram_size)
        } else {
            0
        };
        self.max_datagram = match negotiated {
            0 => 0,
            limit => {
                if self.max_datagram == 0 {
                    limit
                } else {
                    limit.min(self.max_datagram)
                }
            }
        };
    }
}

/// Capability negotiation helpers.
#[derive(Clone, Copy, Debug, Default)]
pub struct CapabilityNegotiation;

impl CapabilityNegotiation {
    /// Perform viewer-side negotiation: send `report` and await `CapabilityAck`.
    pub async fn viewer_handshake<F>(
        conn: &mut StreamingConnection,
        capabilities: TransportCapabilities,
        report: CapabilityReport,
        record: F,
    ) -> Result<(CapabilityAck, TransportCapabilityResolution)>
    where
        F: FnOnce(&TransportCapabilityResolution),
    {
        let mut record = Some(record);
        let local_frame = TransportCapabilitiesFrame {
            endpoint_role: streaming::CapabilityRole::Viewer,
            capabilities,
        };
        conn.send_control_frame(&ControlFrame::TransportCapabilities(local_frame.clone()))
            .await?;
        let remote_caps = loop {
            let response = conn.next_control_frame().await?;
            match response {
                ControlFrame::TransportCapabilities(frame) => {
                    if frame.endpoint_role != streaming::CapabilityRole::Publisher {
                        return Err(Error::ProtocolViolation(format!(
                            "expected publisher capabilities, received {role:?}",
                            role = frame.endpoint_role
                        )));
                    }
                    break frame;
                }
                ControlFrame::Error(err) => {
                    return Err(Error::ProtocolViolation(format!(
                        "peer reported error: {err:?}"
                    )));
                }
                other => {
                    return Err(Error::ProtocolViolation(format!(
                        "expected TransportCapabilities, received {other:?}"
                    )));
                }
            }
        };

        let mut resolution = streaming::resolve_transport_capabilities(
            &local_frame.capabilities,
            &remote_caps.capabilities,
        )
        .map_err(Error::TransportCapability)?;

        if resolution.use_datagram {
            if report.max_datagram_size == 0 {
                return Err(Error::ProtocolViolation(
                    "viewer capability report advertised zero datagram size".into(),
                ));
            }
            resolution.max_segment_datagram_size = resolution
                .max_segment_datagram_size
                .min(report.max_datagram_size);
        }

        conn.send_control_frame(&ControlFrame::CapabilityReport(report))
            .await?;
        loop {
            let response = conn.next_control_frame().await?;
            match response {
                ControlFrame::CapabilityAck(ack) => {
                    if ack.max_datagram_size != resolution.max_segment_datagram_size {
                        return Err(Error::ProtocolViolation(format!(
                            "capability ack reported max_datagram_size={} but negotiated {}",
                            ack.max_datagram_size, resolution.max_segment_datagram_size
                        )));
                    }
                    conn.apply_transport_resolution(resolution);
                    if let Some(callback) = record.take() {
                        callback(&resolution);
                    }
                    return Ok((ack, resolution));
                }
                ControlFrame::Error(err) => {
                    return Err(Error::ProtocolViolation(format!(
                        "peer reported error: {err:?}"
                    )));
                }
                other => {
                    return Err(Error::ProtocolViolation(format!(
                        "expected CapabilityAck, received {other:?}"
                    )));
                }
            }
        }
    }

    /// Await the viewer's capability report (publisher side).
    pub async fn publisher_handshake<F>(
        conn: &mut StreamingConnection,
        capabilities: TransportCapabilities,
        record: F,
    ) -> Result<(CapabilityReport, TransportCapabilityResolution)>
    where
        F: FnOnce(&TransportCapabilityResolution),
    {
        let mut record = Some(record);
        let viewer_caps = match conn.next_control_frame().await? {
            ControlFrame::TransportCapabilities(frame) => {
                if frame.endpoint_role != streaming::CapabilityRole::Viewer {
                    return Err(Error::ProtocolViolation(format!(
                        "expected viewer capabilities, received {role:?}",
                        role = frame.endpoint_role
                    )));
                }
                frame
            }
            ControlFrame::Error(err) => {
                return Err(Error::ProtocolViolation(format!(
                    "peer reported error: {err:?}"
                )));
            }
            other => {
                return Err(Error::ProtocolViolation(format!(
                    "expected TransportCapabilities, received {other:?}"
                )));
            }
        };

        let local_frame = TransportCapabilitiesFrame {
            endpoint_role: streaming::CapabilityRole::Publisher,
            capabilities,
        };
        conn.send_control_frame(&ControlFrame::TransportCapabilities(local_frame.clone()))
            .await?;

        let mut resolution = streaming::resolve_transport_capabilities(
            &local_frame.capabilities,
            &viewer_caps.capabilities,
        )
        .map_err(Error::TransportCapability)?;

        loop {
            let frame = conn.next_control_frame().await?;
            match frame {
                ControlFrame::CapabilityReport(report) => {
                    if resolution.use_datagram {
                        if report.max_datagram_size == 0 {
                            return Err(Error::ProtocolViolation(
                                "viewer capability report advertised zero datagram size".into(),
                            ));
                        }
                        resolution.max_segment_datagram_size = resolution
                            .max_segment_datagram_size
                            .min(report.max_datagram_size);
                    }
                    conn.apply_transport_resolution(resolution);
                    if let Some(callback) = record.take() {
                        callback(&resolution);
                    }
                    return Ok((report, resolution));
                }
                ControlFrame::Error(err) => {
                    return Err(Error::ProtocolViolation(format!(
                        "peer reported error: {err:?}"
                    )));
                }
                other => {
                    return Err(Error::ProtocolViolation(format!(
                        "expected CapabilityReport, received {other:?}"
                    )));
                }
            }
        }
    }
}

struct ControlStreamWriter {
    stream: SendStream,
}

impl ControlStreamWriter {
    async fn new(mut stream: SendStream, direction: ControlStreamDirection) -> Result<Self> {
        stream
            .write_all(CONTROL_STREAM_PREFACE)
            .await
            .map_err(|e| Error::Io(std::io::Error::from(e)))?;
        stream
            .write_u8(direction.marker())
            .await
            .map_err(|e| Error::Io(std::io::Error::from(e)))?;
        stream
            .flush()
            .await
            .map_err(|e| Error::Io(std::io::Error::from(e)))?;
        Ok(Self { stream })
    }

    async fn send_frame(&mut self, frame: &ControlFrame) -> Result<()> {
        let bytes = to_bytes(frame)?;
        if bytes.len() > MAX_CONTROL_FRAME_LEN {
            return Err(Error::FrameTooLarge {
                len: bytes.len(),
                max: MAX_CONTROL_FRAME_LEN,
            });
        }
        self.stream
            .write_u32_le(bytes.len() as u32)
            .await
            .map_err(|e| Error::Io(std::io::Error::from(e)))?;
        self.stream
            .write_all(&bytes)
            .await
            .map_err(|e| Error::Io(std::io::Error::from(e)))?;
        self.stream
            .flush()
            .await
            .map_err(|e| Error::Io(std::io::Error::from(e)))?;
        Ok(())
    }
}

struct ControlStreamReader {
    stream: RecvStream,
}

impl ControlStreamReader {
    async fn new(mut stream: RecvStream, expected: ControlStreamDirection) -> Result<Self> {
        let mut preface = [0u8; CONTROL_STREAM_PREFACE.len()];
        match stream.read_exact(&mut preface).await {
            Ok(()) => {}
            Err(ReadExactError::FinishedEarly(_)) => return Err(Error::ControlStreamClosed),
            Err(ReadExactError::ReadError(e)) => return Err(Error::Io(std::io::Error::from(e))),
        }
        if preface != *CONTROL_STREAM_PREFACE {
            return Err(Error::BadPreface);
        }
        let marker = match stream.read_u8().await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(Error::ControlStreamClosed);
            }
            Err(e) => return Err(Error::Io(e)),
        };
        let found = ControlStreamDirection::from_marker(marker).ok_or(Error::BadPreface)?;
        if found != expected {
            return Err(Error::WrongDirection { expected, found });
        }

        Ok(Self { stream })
    }

    async fn next_frame(&mut self) -> Result<ControlFrame> {
        let mut len_buf = [0u8; 4];
        match self.stream.read_exact(&mut len_buf).await {
            Ok(()) => {}
            Err(ReadExactError::FinishedEarly(_)) => return Err(Error::ControlStreamClosed),
            Err(ReadExactError::ReadError(e)) => return Err(Error::Io(std::io::Error::from(e))),
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > MAX_CONTROL_FRAME_LEN {
            return Err(Error::FrameTooLarge {
                len,
                max: MAX_CONTROL_FRAME_LEN,
            });
        }
        let mut buf = vec![0u8; len];
        match self.stream.read_exact(&mut buf).await {
            Ok(()) => {}
            Err(ReadExactError::FinishedEarly(_)) => return Err(Error::ControlStreamClosed),
            Err(ReadExactError::ReadError(e)) => return Err(Error::Io(std::io::Error::from(e))),
        }
        let frame = deserialize_from(buf.as_slice())?;
        Ok(frame)
    }
}

#[derive(Debug)]
struct ParsedMultiaddr {
    host: IpAddr,
    port: u16,
    server_name: String,
}

fn parse_multiaddr(addr: &str) -> Result<ParsedMultiaddr> {
    let trimmed = addr.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidMultiaddr(addr.into()));
    }
    let mut parts = trimmed.trim_matches('/').split('/');
    let proto = parts
        .next()
        .ok_or_else(|| Error::InvalidMultiaddr(addr.into()))?;
    match proto {
        "ip4" => {
            let host = parts
                .next()
                .ok_or_else(|| Error::InvalidMultiaddr(addr.into()))?;
            let ip: Ipv4Addr = host
                .parse()
                .map_err(|_| Error::InvalidMultiaddr(addr.into()))?;
            let port = parse_port(&mut parts, addr)?;
            Ok(ParsedMultiaddr {
                host: IpAddr::V4(ip),
                port,
                server_name: host.to_string(),
            })
        }
        "ip6" => {
            let host = parts
                .next()
                .ok_or_else(|| Error::InvalidMultiaddr(addr.into()))?;
            let ip: Ipv6Addr = host
                .parse()
                .map_err(|_| Error::InvalidMultiaddr(addr.into()))?;
            let port = parse_port(&mut parts, addr)?;
            Ok(ParsedMultiaddr {
                host: IpAddr::V6(ip),
                port,
                server_name: host.to_string(),
            })
        }
        other => Err(Error::UnsupportedProtocol(other.into())),
    }
}

fn parse_port<'a, I>(parts: &mut I, original: &str) -> Result<u16>
where
    I: Iterator<Item = &'a str>,
{
    let transport = parts.next().ok_or_else(|| Error::MissingPort)?;
    if transport != "udp" {
        return Err(Error::UnsupportedProtocol(transport.into()));
    }
    let port_str = parts.next().ok_or_else(|| Error::MissingPort)?;
    let port: u16 = port_str
        .parse()
        .map_err(|_| Error::InvalidMultiaddr(original.into()))?;
    if let Some(extra) = parts.next() {
        if extra != "quic" {
            return Err(Error::UnsupportedProtocol(extra.into()));
        }
        if let Some(rem) = parts.next() {
            return Err(Error::UnsupportedProtocol(rem.into()));
        }
    }
    Ok(port)
}

fn build_transport_config(settings: TransportConfigSettings) -> Result<Arc<TransportConfig>> {
    let mut transport = TransportConfig::default();
    transport.datagram_receive_buffer_size(Some(settings.datagram_receive_buffer));
    transport.datagram_send_buffer_size(settings.datagram_send_buffer);
    transport.keep_alive_interval(Some(settings.idle_timeout / 2));
    let idle = IdleTimeout::try_from(settings.idle_timeout)
        .map_err(|e| Error::TransportConfig(e.to_string()))?;
    transport.max_idle_timeout(Some(idle));
    Ok(Arc::new(transport))
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
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::RSA_PSS_SHA256,
        ]
    }
}

#[cfg(all(test, feature = "quic"))]
mod tests {
    use iroha_crypto::streaming::StreamingSession;
    use norito::streaming::{
        AudioCapability, CapabilityAck, CapabilityFlags, CapabilityReport, CapabilityRole,
        ChunkDescriptor, EncryptionSuite, EntropyMode, FecScheme, FeedbackHintFrame, Hash,
        ManifestAnnounceFrame, ManifestV1, ProfileId, ReceiverReport, Resolution, StreamMetadata,
        TransportCapabilities,
    };
    use tokio::time::{Duration as TokioDuration, sleep, timeout};

    use super::*;

    const TEST_TIMEOUT: TokioDuration = TokioDuration::from_secs(10);

    fn hash(byte: u8) -> Hash {
        [byte; 32]
    }

    fn manifest() -> ManifestAnnounceFrame {
        ManifestAnnounceFrame {
            manifest: ManifestV1 {
                stream_id: hash(1),
                protocol_version: 1,
                segment_number: 10,
                published_at: 1_701_234_567,
                profile: ProfileId::BASELINE,
                entropy_mode: EntropyMode::RansBundled,
                entropy_tables_checksum: None,
                da_endpoint: "/ip4/127.0.0.1/udp/9000/quic".into(),
                chunk_root: hash(2),
                content_key_id: 5,
                nonce_salt: hash(3),
                chunk_descriptors: vec![ChunkDescriptor {
                    chunk_id: 0,
                    offset: 0,
                    length: 3,
                    commitment: hash(4),
                    parity: false,
                }],
                transport_capabilities_hash: [0; 32],
                encryption_suite: EncryptionSuite::X25519ChaCha20Poly1305(hash(5)),
                fec_suite: FecScheme::Rs12_10,
                privacy_routes: Vec::new(),
                neural_bundle: None,
                audio_summary: None,
                public_metadata: StreamMetadata {
                    title: "NSC Test Stream".into(),
                    description: Some("integration harness".into()),
                    access_policy_id: Some(hash(6)),
                    tags: vec!["test".into(), "nsc".into()],
                },
                capabilities: CapabilityFlags::from_bits(0b101),
                signature: [0xAA; 64],
            },
        }
    }

    #[tokio::test]
    async fn capability_negotiation_and_datagram_roundtrip() {
        let settings = TransportConfigSettings::default();
        let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let server = match StreamingServer::bind(server_addr, settings).await {
            Ok(server) => server,
            Err(Error::Io(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("quic test skipped: {err}");
                return;
            }
            Err(err) => panic!("server bind failed: {err:?}"),
        };
        let listen_addr = server.local_addr().expect("listen addr");

        let server_task = {
            let server = server.clone();
            async move {
                let mut conn = server.accept().await.expect("accept");
                let publisher_caps = TransportCapabilities {
                    max_segment_datagram_size: settings.max_datagram_size as u16,
                    ..TransportCapabilities::kyber768_default()
                };
                let (report, transport_resolution) = CapabilityNegotiation::publisher_handshake(
                    &mut conn,
                    publisher_caps.clone(),
                    |_| {},
                )
                .await
                .expect("report");
                assert!(transport_resolution.use_datagram);
                assert_eq!(
                    transport_resolution.max_segment_datagram_size,
                    settings.max_datagram_size as u16
                );
                assert_eq!(report.endpoint_role, CapabilityRole::Viewer);
                let ack = CapabilityAck {
                    stream_id: report.stream_id,
                    accepted_version: report.protocol_version,
                    negotiated_features: CapabilityFlags::from_bits(
                        report.feature_bits.bits() | 0b1,
                    ),
                    max_datagram_size: transport_resolution.max_segment_datagram_size,
                    dplpmtud: report.dplpmtud,
                };
                let ack_frame = ControlFrame::CapabilityAck(ack.clone());
                conn.send_control_frame(&ack_frame).await.expect("ack");

                let announce = manifest();
                let transport_hash = transport_resolution.capabilities_hash();
                let mut manifest_with_hash = announce.clone();
                manifest_with_hash.manifest.transport_capabilities_hash = transport_hash;
                conn.send_control_frame(&ControlFrame::ManifestAnnounce(Box::new(
                    manifest_with_hash.clone(),
                )))
                .await
                .expect("announce");

                let chunk = vec![0xDE, 0xAD, 0xBE, 0xEF];
                conn.send_datagram(&chunk).await.expect("datagram");

                // allow the viewer to read before we close
                sleep(TokioDuration::from_millis(50)).await;
                conn.close();
            }
        };

        let viewer_task = async {
            let mut client = StreamingClient::connect(
                &format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port()),
                settings,
            )
            .await
            .expect("client");

            let recorded_hash = Arc::new(std::sync::Mutex::new(None));
            let max_size =
                u16::try_from(settings.max_datagram_size).expect("max_datagram_size fits u16");
            let report = CapabilityReport {
                stream_id: hash(9),
                endpoint_role: CapabilityRole::Viewer,
                protocol_version: 1,
                max_resolution: norito::streaming::Resolution::R1080p,
                hdr_supported: false,
                capture_hdr: false,
                neural_bundles: vec!["bundle-v1".into()],
                audio_caps: AudioCapability {
                    sample_rates: vec![48_000],
                    ambisonics: false,
                    max_channels: 2,
                },
                feature_bits: CapabilityFlags::from_bits(0b10),
                max_datagram_size: max_size,
                dplpmtud: false,
            };
            let viewer_caps = TransportCapabilities {
                max_segment_datagram_size: max_size,
                ..TransportCapabilities::kyber768_default()
            };
            let recorded_clone = Arc::clone(&recorded_hash);
            let (ack, transport_resolution) = CapabilityNegotiation::viewer_handshake(
                client.connection(),
                viewer_caps,
                report,
                move |resolution| {
                    *recorded_clone.lock().expect("recorded hash lock") =
                        Some(resolution.capabilities_hash());
                },
            )
            .await
            .unwrap();
            assert_eq!(ack.accepted_version, 1);
            assert!(transport_resolution.use_datagram);
            assert_eq!(transport_resolution.max_segment_datagram_size, max_size);
            assert_eq!(
                *recorded_hash.lock().expect("recorded hash checked"),
                Some(transport_resolution.capabilities_hash())
            );

            let frame = client.connection().next_control_frame().await.unwrap();
            match frame {
                ControlFrame::ManifestAnnounce(frame) => {
                    assert_eq!(frame.manifest.stream_id, hash(1));
                    assert_eq!(
                        frame.manifest.transport_capabilities_hash,
                        transport_resolution.capabilities_hash()
                    );
                }
                other => panic!("unexpected frame: {other:?}"),
            }

            let chunk = client.connection().recv_datagram().await.unwrap();
            assert_eq!(chunk.as_ref(), &[0xDE, 0xAD, 0xBE, 0xEF]);

            client.close().await;
        };

        let res = timeout(TEST_TIMEOUT, async { tokio::join!(server_task, viewer_task) }).await;
        server.shutdown().await;
        res.expect("capability_negotiation_and_datagram_roundtrip timed out");
    }

    #[tokio::test]
    async fn feedback_frames_roundtrip_over_quic() {
        let settings = TransportConfigSettings::default();
        let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let server = match StreamingServer::bind(server_addr, settings).await {
            Ok(server) => server,
            Err(Error::Io(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("quic test skipped: {err}");
                return;
            }
            Err(err) => panic!("server bind failed: {err:?}"),
        };
        let listen_addr = server.local_addr().expect("listen addr");

        let server_task = {
            let server = server.clone();
            async move {
                let mut conn = server.accept().await.expect("accept");
                let mut session = StreamingSession::new(CapabilityRole::Publisher);

                let publisher_caps = TransportCapabilities::kyber768_default();
                let (report, resolution) = CapabilityNegotiation::publisher_handshake(
                    &mut conn,
                    publisher_caps.clone(),
                    |_| {},
                )
                .await
                .expect("handshake");
                session.record_transport_capabilities(resolution.clone());

                let ack = CapabilityAck {
                    stream_id: report.stream_id,
                    accepted_version: report.protocol_version,
                    negotiated_features: report.feature_bits,
                    max_datagram_size: resolution.max_segment_datagram_size,
                    dplpmtud: report.dplpmtud,
                };
                conn.send_control_frame(&ControlFrame::CapabilityAck(ack))
                    .await
                    .expect("ack");

                let mut received_hint = false;
                let mut received_report = false;
                while !(received_hint && received_report) {
                    match conn.next_control_frame().await.expect("frame") {
                        ControlFrame::FeedbackHint(hint) => {
                            session.process_feedback_hint(&hint);
                            received_hint = true;
                        }
                        ControlFrame::ReceiverReport(report) => {
                            let parity = session.process_receiver_report(&report);
                            assert_eq!(parity, 2);
                            received_report = true;
                        }
                        ControlFrame::CapabilityReport(_) => {
                            // Viewer may resend capability report during handshake; ignore.
                        }
                        other => panic!("unexpected frame: {other:?}"),
                    }
                }

                assert_eq!(session.latest_feedback_parity(), Some(2));
                conn.close();
            }
        };

        let viewer_task = async move {
            let mut client = StreamingClient::connect(
                &format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port()),
                settings,
            )
            .await
            .expect("client");

            let report = CapabilityReport {
                stream_id: hash(11),
                endpoint_role: CapabilityRole::Viewer,
                protocol_version: 1,
                max_resolution: Resolution::R1080p,
                hdr_supported: false,
                capture_hdr: false,
                neural_bundles: vec!["bundle-v1".into()],
                audio_caps: AudioCapability {
                    sample_rates: vec![48_000],
                    ambisonics: false,
                    max_channels: 2,
                },
                feature_bits: CapabilityFlags::from_bits(0b10),
                max_datagram_size: settings.max_datagram_size as u16,
                dplpmtud: false,
            };
            let viewer_caps = TransportCapabilities {
                max_segment_datagram_size: settings.max_datagram_size as u16,
                ..TransportCapabilities::kyber768_default()
            };
            let (_ack, _resolution) = CapabilityNegotiation::viewer_handshake(
                client.connection(),
                viewer_caps,
                report,
                |_| {},
            )
            .await
            .expect("viewer handshake");

            let hint = FeedbackHintFrame {
                stream_id: hash(11),
                loss_ewma_q16: (0.08_f64 * 65536.0).round() as u32,
                latency_gradient_q16: 0,
                observed_rtt_ms: 30,
                report_interval_ms: 250,
                parity_chunks: 0,
            };
            let recv_report = ReceiverReport {
                stream_id: hint.stream_id,
                latest_segment: 12,
                layer_mask: 0,
                measured_throughput_kbps: 1_250,
                rtt_ms: 33,
                loss_percent_x100: 700,
                decoder_buffer_ms: 90,
                active_resolution: Resolution::R1080p,
                hdr_active: false,
                ecn_ce_count: 0,
                jitter_ms: 5,
                delivered_sequence: 900,
                parity_applied: 1,
                fec_budget: 1,
                sync_diagnostics: None,
            };

            client
                .connection()
                .send_control_frame(&ControlFrame::FeedbackHint(hint))
                .await
                .expect("send hint");
            client
                .connection()
                .send_control_frame(&ControlFrame::ReceiverReport(recv_report))
                .await
                .expect("send report");

            client.close().await;
        };

        let res = timeout(TEST_TIMEOUT, async { tokio::join!(server_task, viewer_task) }).await;
        server.shutdown().await;
        res.expect("feedback_frames_roundtrip_over_quic timed out");
    }

    #[tokio::test]
    async fn mtu_negotiation_clamps_to_smallest_limit() {
        let settings = TransportConfigSettings::default();
        let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let server = match StreamingServer::bind(server_addr, settings).await {
            Ok(server) => server,
            Err(Error::Io(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("quic test skipped: {err}");
                return;
            }
            Err(err) => panic!("server bind failed: {err:?}"),
        };
        let listen_addr = server.local_addr().expect("listen addr");

        let server_task = {
            let server = server.clone();
            async move {
                let mut conn = server.accept().await.expect("accept");

                let mut publisher_caps = TransportCapabilities::kyber768_default();
                publisher_caps.max_segment_datagram_size = 1100;
                let (report, resolution) =
                    CapabilityNegotiation::publisher_handshake(&mut conn, publisher_caps, |_| {})
                        .await
                        .expect("handshake");
                assert!(resolution.use_datagram);
                assert_eq!(resolution.max_segment_datagram_size, 900);
                assert_eq!(conn.max_datagram_size(), 900);
                assert!(conn.datagram_enabled());

                let ack = CapabilityAck {
                    stream_id: report.stream_id,
                    accepted_version: report.protocol_version,
                    negotiated_features: CapabilityFlags::from_bits(
                        report.feature_bits.bits() | 0b1,
                    ),
                    max_datagram_size: resolution.max_segment_datagram_size,
                    dplpmtud: report.dplpmtud,
                };
                conn.send_control_frame(&ControlFrame::CapabilityAck(ack))
                    .await
                    .expect("ack");

                conn.close();
            }
        };

        let viewer_task = async move {
            let mut client = StreamingClient::connect(
                &format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port()),
                settings,
            )
            .await
            .expect("client");

            let mut viewer_caps = TransportCapabilities::kyber768_default();
            viewer_caps.max_segment_datagram_size = 1024;

            let report = CapabilityReport {
                stream_id: hash(7),
                endpoint_role: CapabilityRole::Viewer,
                protocol_version: 1,
                max_resolution: norito::streaming::Resolution::R1080p,
                hdr_supported: true,
                capture_hdr: true,
                neural_bundles: vec!["bundle-v2".into()],
                audio_caps: AudioCapability {
                    sample_rates: vec![48_000, 96_000],
                    ambisonics: false,
                    max_channels: 2,
                },
                feature_bits: CapabilityFlags::from_bits(0b11),
                max_datagram_size: 900,
                dplpmtud: true,
            };

            let (ack, resolution) = CapabilityNegotiation::viewer_handshake(
                client.connection(),
                viewer_caps,
                report,
                |_| {},
            )
            .await
            .expect("handshake");
            assert!(resolution.use_datagram);
            assert_eq!(resolution.max_segment_datagram_size, 900);
            assert_eq!(ack.max_datagram_size, 900);
            assert_eq!(client.connection().max_datagram_size(), 900);
            assert!(client.connection().datagram_enabled());

            let payload = vec![0_u8; 901];
            let err = client
                .connection()
                .send_datagram(&payload)
                .await
                .unwrap_err();
            match err {
                Error::DatagramTooLarge { max, .. } => assert_eq!(max, 900),
                other => panic!("unexpected error: {other:?}"),
            }

            client.close().await;
        };

        let res =
            timeout(TEST_TIMEOUT, async { tokio::join!(server_task, viewer_task) }).await;
        server.shutdown().await;
        res.expect("mtu_negotiation_clamps_to_smallest_limit timed out");
    }

    #[tokio::test]
    async fn datagram_disabled_sets_zero_limit() {
        let settings = TransportConfigSettings::default();
        let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let server = match StreamingServer::bind(server_addr, settings).await {
            Ok(server) => server,
            Err(Error::Io(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("quic test skipped: {err}");
                return;
            }
            Err(err) => panic!("server bind failed: {err:?}"),
        };
        let listen_addr = server.local_addr().expect("listen addr");

        let server_task = {
            let server = server.clone();
            async move {
                let mut conn = server.accept().await.expect("accept");

                let mut publisher_caps = TransportCapabilities::kyber768_default();
                publisher_caps.supports_datagram = false;
                let (report, resolution) =
                    CapabilityNegotiation::publisher_handshake(&mut conn, publisher_caps, |_| {})
                        .await
                        .expect("handshake");
                assert!(!resolution.use_datagram);
                assert_eq!(resolution.max_segment_datagram_size, 0);
                assert_eq!(conn.max_datagram_size(), 0);
                assert!(!conn.datagram_enabled());

                let ack = CapabilityAck {
                    stream_id: report.stream_id,
                    accepted_version: report.protocol_version,
                    negotiated_features: CapabilityFlags::from_bits(report.feature_bits.bits()),
                    max_datagram_size: 0,
                    dplpmtud: report.dplpmtud,
                };
                conn.send_control_frame(&ControlFrame::CapabilityAck(ack))
                    .await
                    .expect("ack");

                conn.close();
            }
        };

        let viewer_task = async move {
            let mut client = StreamingClient::connect(
                &format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port()),
                settings,
            )
            .await
            .expect("client");

            let mut viewer_caps = TransportCapabilities::kyber768_default();
            viewer_caps.max_segment_datagram_size = 1024;

            let report = CapabilityReport {
                stream_id: hash(8),
                endpoint_role: CapabilityRole::Viewer,
                protocol_version: 1,
                max_resolution: norito::streaming::Resolution::R720p,
                hdr_supported: false,
                capture_hdr: false,
                neural_bundles: vec![],
                audio_caps: AudioCapability {
                    sample_rates: vec![48_000],
                    ambisonics: false,
                    max_channels: 2,
                },
                feature_bits: CapabilityFlags::from_bits(0b01),
                max_datagram_size: 1200,
                dplpmtud: false,
            };

            let (ack, resolution) = CapabilityNegotiation::viewer_handshake(
                client.connection(),
                viewer_caps,
                report,
                |_| {},
            )
            .await
            .expect("handshake");
            assert!(!resolution.use_datagram);
            assert_eq!(resolution.max_segment_datagram_size, 0);
            assert_eq!(ack.max_datagram_size, 0);
            assert_eq!(client.connection().max_datagram_size(), 0);
            assert!(!client.connection().datagram_enabled());

            let payload = vec![0_u8; 1];
            let err = client
                .connection()
                .send_datagram(&payload)
                .await
                .unwrap_err();
            match err {
                Error::DatagramTooLarge { max, .. } => assert_eq!(max, 0),
                other => panic!("unexpected error: {other:?}"),
            }

            client.close().await;
        };

        let res = timeout(TEST_TIMEOUT, async { tokio::join!(server_task, viewer_task) }).await;
        server.shutdown().await;
        res.expect("datagram_disabled_sets_zero_limit timed out");
    }
}
