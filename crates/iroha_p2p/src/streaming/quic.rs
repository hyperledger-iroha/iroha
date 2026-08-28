#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::missing_errors_doc,
    clippy::ignored_unit_patterns,
    clippy::unused_async
)]
use bytes::Bytes;
use norito::{
    decode_from_bytes,
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
use rustls::{client::danger::ServerCertVerifier, pki_types::PrivatePkcs8KeyDer};
use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
    time::Duration,
};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{sync::Notify, task::JoinHandle};
const CONTROL_STREAM_PREFACE: &[u8; 5] = b"NSC/1";
const CONTROL_TYPE_PUBLISHER_TO_VIEWER: u8 = 0x01;
const CONTROL_TYPE_VIEWER_TO_PUBLISHER: u8 = 0x02;
const DEFAULT_MAX_DATAGRAM_SIZE: usize = 1350;
const DEFAULT_DATAGRAM_BUFFER: usize = 1 << 20;
const MAX_CONTROL_FRAME_LEN: usize = 512 * 1024;
const MAX_DATAGRAM_INBOX_ENTRIES: usize = 256;
const DATAGRAM_NEGOTIATING: usize = usize::MAX;
const DATAGRAM_PROTOCOL_ERROR_CODE: u32 = 0x4e53_4301;
const SETUP_PENDING: u8 = 0;
const SETUP_COMPLETE: u8 = 1;
const SETUP_TIMED_OUT: u8 = 2;
const ALPN: &[u8] = b"nsc/1";
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Fingerprint that pins one streaming server certificate.
pub type CertificateFingerprint = [u8; iroha_crypto::Hash::LENGTH];
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
    /// Streaming transport setup exceeded its single absolute pre-authentication deadline.
    #[error("streaming setup exceeded its pre-authentication deadline")]
    SetupTimeout,
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
    certificate_fingerprint: CertificateFingerprint,
}
impl StreamingServer {
    /// Bind a QUIC listener on `addr` using the provided transport settings.
    pub async fn bind(addr: SocketAddr, settings: TransportConfigSettings) -> Result<Self> {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(["nsc.local".to_owned()])
                .map_err(|e| Error::TlsServer(e.to_string()))?;
        let cert_der = cert.der().clone().into_owned();
        let certificate_fingerprint = crate::transport::certificate_fingerprint(cert_der.as_ref());
        let priv_key = PrivatePkcs8KeyDer::from(signing_key.serialize_der());
        let mut rustls_config =
            rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_no_client_auth()
                .with_single_cert(vec![cert_der], priv_key.into())
                .map_err(|e| Error::TlsServer(e.to_string()))?;
        rustls_config.max_early_data_size = 0;
        rustls_config.alpn_protocols = vec![ALPN.to_vec()];
        let rustls_config = Arc::new(rustls_config);
        let crypto = QuinnRustlsServerConfig::try_from(rustls_config)
            .map_err(|e| Error::TlsServer(e.to_string()))?;
        let mut server_config = ServerConfig::with_crypto(Arc::new(crypto));
        let transport = build_transport_config(settings)?;
        server_config.transport_config(transport);
        let endpoint = Endpoint::server(server_config, addr)?;
        Ok(Self {
            endpoint,
            settings,
            certificate_fingerprint,
        })
    }
    /// Retrieve the socket address the server is listening on.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.endpoint.local_addr().map_err(Error::from)
    }
    /// Return the fingerprint clients must obtain through an authenticated channel.
    #[must_use]
    pub const fn certificate_fingerprint(&self) -> CertificateFingerprint {
        self.certificate_fingerprint
    }
    /// Accept the next inbound streaming connection.
    pub async fn accept(&self) -> Result<StreamingConnection> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| Error::ProtocolViolation("listener closed".into()))?;
        let deadline = setup_deadline(self.settings)?;
        let connecting = incoming
            .accept()
            .map_err(|e| std::io::Error::other(format!("listener accept failed: {e}")))?;
        let connection = deadline
            .run(None, connecting)
            .await
            .map_err(|_| Error::SetupTimeout)??;
        StreamingConnection::new_with_deadline(
            connection,
            EndpointRole::Publisher,
            self.settings,
            deadline,
        )
        .await
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
    ///
    /// `expected_certificate_fingerprint` must come from an authenticated manifest, directory, or
    /// operator configuration. Requiring it here keeps transport authentication fail-closed even
    /// before streaming `KeyUpdate` frames establish end-to-end content keys.
    pub async fn connect(
        multiaddr: &str,
        expected_certificate_fingerprint: CertificateFingerprint,
        settings: TransportConfigSettings,
    ) -> Result<Self> {
        let parsed = parse_multiaddr(multiaddr)?;
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())?;
        let verifier: Arc<dyn ServerCertVerifier> = Arc::new(
            crate::transport::CertificateKeyProofVerifier::pinned(expected_certificate_fingerprint),
        );
        let mut tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        tls_config.enable_early_data = false;
        tls_config.alpn_protocols = vec![ALPN.to_vec()];
        let tls_config = Arc::new(tls_config);
        let crypto = QuinnRustlsClientConfig::try_from(Arc::clone(&tls_config))
            .map_err(|e| Error::TlsClient(e.to_string()))?;
        let mut client_config = ClientConfig::new(Arc::new(crypto));
        let transport = build_transport_config(settings)?;
        client_config.transport_config(transport);
        endpoint.set_default_client_config(client_config);
        let server_addr = SocketAddr::new(parsed.host, parsed.port);
        let deadline = setup_deadline(settings)?;
        let connecting = endpoint.connect(server_addr, &parsed.server_name)?;
        let connection = deadline
            .run(None, connecting)
            .await
            .map_err(|_| Error::SetupTimeout)??;
        let connection = StreamingConnection::new_with_deadline(
            connection,
            EndpointRole::Viewer,
            settings,
            deadline,
        )
        .await?;
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

fn setup_deadline(settings: TransportConfigSettings) -> Result<crate::preauth::PreauthDeadline> {
    crate::preauth::PreauthDeadline::from_now(settings.idle_timeout).ok_or_else(|| {
        Error::TransportConfig("streaming setup timeout cannot be represented".into())
    })
}

#[derive(Clone, Debug)]
enum DatagramTerminal {
    Connection(ConnectionError),
    Protocol(String),
}

impl DatagramTerminal {
    fn into_error(self) -> Error {
        match self {
            Self::Connection(error) => Error::Connection(error),
            Self::Protocol(reason) => Error::ProtocolViolation(reason),
        }
    }
}

#[derive(Debug)]
struct DatagramInboxState {
    frames: VecDeque<Bytes>,
    bytes: usize,
    terminal: Option<DatagramTerminal>,
    policy: usize,
}

impl Default for DatagramInboxState {
    fn default() -> Self {
        Self {
            frames: VecDeque::new(),
            bytes: 0,
            terminal: None,
            policy: DATAGRAM_NEGOTIATING,
        }
    }
}

#[derive(Debug)]
struct DatagramInbox {
    state: Mutex<DatagramInboxState>,
    notify: Notify,
    max_bytes: usize,
}

impl DatagramInbox {
    fn new(max_bytes: usize) -> Self {
        Self {
            state: Mutex::new(DatagramInboxState::default()),
            notify: Notify::new(),
            max_bytes,
        }
    }

    fn admit(&self, frame: Bytes, configured_max: usize) -> core::result::Result<(), String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.terminal.is_some() {
            return Err("DATAGRAM session is already terminal".to_owned());
        }
        let max_datagram = if state.policy == DATAGRAM_NEGOTIATING {
            configured_max
        } else {
            state.policy
        };
        if max_datagram == 0 {
            return Err("peer sent a DATAGRAM while delivery was disabled".to_owned());
        }
        if frame.len() > max_datagram {
            return Err(format!(
                "peer sent datagram of {} bytes above negotiated maximum {max_datagram}",
                frame.len()
            ));
        }
        if state.frames.len() == MAX_DATAGRAM_INBOX_ENTRIES
            || frame.len() > self.max_bytes.saturating_sub(state.bytes)
        {
            // QUIC DATAGRAM delivery is unreliable. Dropping a newest frame
            // keeps both count and bytes bounded without backpressuring the
            // eager transport drain.
            return Ok(());
        }
        state.bytes += frame.len();
        state.frames.push_back(frame);
        drop(state);
        self.notify.notify_one();
        Ok(())
    }

    fn fail(&self, terminal: DatagramTerminal) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.terminal.is_none() {
            state.frames.clear();
            state.bytes = 0;
            state.terminal = Some(terminal);
        }
        drop(state);
        self.notify.notify_waiters();
    }

    fn apply_policy(&self, max_datagram: usize) -> Option<String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.policy = max_datagram;
        let violation = state.frames.iter().find_map(|frame| {
            if max_datagram == 0 {
                Some("peer sent a DATAGRAM while delivery was disabled".to_owned())
            } else if frame.len() > max_datagram {
                Some(format!(
                    "peer sent datagram of {} bytes above negotiated maximum {max_datagram}",
                    frame.len()
                ))
            } else {
                None
            }
        });
        if let Some(reason) = violation.as_ref() {
            state.frames.clear();
            state.bytes = 0;
            state.terminal = Some(DatagramTerminal::Protocol(reason.clone()));
        }
        drop(state);
        self.notify.notify_waiters();
        violation
    }

    fn has_protocol_terminal(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .terminal
            .as_ref()
            .is_some_and(|terminal| matches!(terminal, DatagramTerminal::Protocol(_)))
    }

    async fn recv(&self) -> Result<Bytes> {
        loop {
            let notified = self.notify.notified();
            let outcome = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match state.terminal.clone() {
                    Some(terminal) => Some(Err(terminal.into_error())),
                    None => {
                        let max_datagram = state.policy;
                        if max_datagram == DATAGRAM_NEGOTIATING {
                            None
                        } else if max_datagram == 0 {
                            Some(Err(Error::ProtocolViolation(
                                "DATAGRAM delivery is disabled for this session".into(),
                            )))
                        } else {
                            state.frames.pop_front().map(|frame| {
                            state.bytes = state.bytes.saturating_sub(frame.len());
                            if frame.len() > max_datagram {
                                let reason = format!(
                                    "peer sent datagram of {} bytes above negotiated maximum {max_datagram}",
                                    frame.len()
                                );
                                state.frames.clear();
                                state.bytes = 0;
                                state.terminal =
                                    Some(DatagramTerminal::Protocol(reason.clone()));
                                Err(Error::ProtocolViolation(reason))
                            } else {
                                Ok(frame)
                            }
                        })
                        }
                    }
                }
            };
            if let Some(outcome) = outcome {
                return outcome;
            }
            notified.await;
        }
    }
}

fn spawn_datagram_pump(
    connection: Connection,
    inbox: Arc<DatagramInbox>,
    configured_max: usize,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let frame = match connection.read_datagram().await {
                Ok(frame) => frame,
                Err(error) => {
                    inbox.fail(DatagramTerminal::Connection(error));
                    return;
                }
            };
            if frame.is_empty() {
                let reason = "peer sent a zero-length QUIC DATAGRAM".to_owned();
                inbox.fail(DatagramTerminal::Protocol(reason));
                connection.close(
                    VarInt::from_u32(DATAGRAM_PROTOCOL_ERROR_CODE),
                    b"zero-length DATAGRAM",
                );
                let mut drained = 0_u32;
                while connection.read_datagram().await.is_ok() {
                    drained = drained.wrapping_add(1);
                    if drained % 64 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
                return;
            }
            if let Err(reason) = inbox.admit(frame, configured_max) {
                inbox.fail(DatagramTerminal::Protocol(reason));
                connection.close(
                    VarInt::from_u32(DATAGRAM_PROTOCOL_ERROR_CODE),
                    b"invalid DATAGRAM",
                );
                while connection.read_datagram().await.is_ok() {
                    tokio::task::yield_now().await;
                }
                return;
            }
        }
    })
}

#[derive(Clone, Copy, Debug)]
struct PendingCapabilityAck {
    stream_id: streaming::Hash,
    protocol_version: u16,
    dplpmtud: bool,
    resolution: TransportCapabilityResolution,
}

/// Active streaming session over QUIC.
pub struct StreamingConnection {
    role: EndpointRole,
    connection: Connection,
    control_send: ControlStreamWriter,
    control_recv: ControlStreamReader,
    configured_max_datagram: usize,
    max_datagram: Arc<AtomicUsize>,
    datagram_inbox: Arc<DatagramInbox>,
    datagram_task: JoinHandle<()>,
    setup_deadline: Option<crate::preauth::PreauthDeadline>,
    setup_state: Arc<AtomicU8>,
    setup_watchdog: JoinHandle<()>,
    pending_publisher_ack: Option<PendingCapabilityAck>,
}
impl StreamingConnection {
    #[cfg(test)]
    async fn new(
        connection: Connection,
        role: EndpointRole,
        settings: TransportConfigSettings,
    ) -> Result<Self> {
        let deadline = setup_deadline(settings)?;
        Self::new_with_deadline(connection, role, settings, deadline).await
    }

    async fn new_with_deadline(
        connection: Connection,
        role: EndpointRole,
        settings: TransportConfigSettings,
        deadline: crate::preauth::PreauthDeadline,
    ) -> Result<Self> {
        let max_datagram = Arc::new(AtomicUsize::new(DATAGRAM_NEGOTIATING));
        let datagram_inbox = Arc::new(DatagramInbox::new(settings.datagram_receive_buffer));
        // Drain Quinn continuously before the control stream is authenticated.
        // The bounded application inbox counts entries as well as bytes and
        // closes the connection on the first empty DATAGRAM.
        // TODO: Patch or update Quinn so dependency-owned queued DATAGRAMs also
        // carry a fixed per-entry charge before this pump gets scheduled.
        let datagram_task = spawn_datagram_pump(
            connection.clone(),
            Arc::clone(&datagram_inbox),
            settings.max_datagram_size,
        );
        let setup = deadline
            .run(None, async {
                let send = connection
                    .open_uni()
                    .await
                    .map_err(|e| Error::Io(std::io::Error::from(e)))?;
                let control_send =
                    ControlStreamWriter::new(send, role.outgoing_direction()).await?;
                // QUIC streams are created implicitly when the first frame is sent. If we wait on
                // `accept_uni()` before writing anything to our outgoing stream, both endpoints can
                // deadlock waiting for the other side to "open" its control stream.
                let recv = connection.accept_uni().await?;
                let control_recv =
                    ControlStreamReader::new(recv, role.incoming_direction()).await?;
                Ok::<_, Error>((control_send, control_recv))
            })
            .await;
        let (control_send, control_recv) = match setup {
            Ok(Ok(streams)) => streams,
            Ok(Err(error)) => {
                datagram_task.abort();
                return Err(error);
            }
            Err(_) => {
                connection.close(VarInt::from_u32(0), b"setup timeout");
                datagram_task.abort();
                return Err(Error::SetupTimeout);
            }
        };
        let setup_state = Arc::new(AtomicU8::new(SETUP_PENDING));
        let watchdog_state = Arc::clone(&setup_state);
        let watchdog_connection = connection.clone();
        let setup_watchdog = tokio::spawn(async move {
            deadline.wait().await;
            if watchdog_state
                .compare_exchange(
                    SETUP_PENDING,
                    SETUP_TIMED_OUT,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                watchdog_connection.close(VarInt::from_u32(0), b"setup timeout");
            }
        });
        Ok(Self {
            role,
            connection,
            control_send,
            control_recv,
            configured_max_datagram: settings.max_datagram_size,
            max_datagram,
            datagram_inbox,
            datagram_task,
            setup_deadline: Some(deadline),
            setup_state,
            setup_watchdog,
            pending_publisher_ack: None,
        })
    }
    /// Return the local role.
    pub const fn role(&self) -> EndpointRole {
        self.role
    }
    /// Send a control frame to the peer.
    pub async fn send_control_frame(&mut self, frame: &ControlFrame) -> Result<()> {
        let completes_setup = if matches!(self.role, EndpointRole::Publisher) {
            if let ControlFrame::CapabilityAck(ack) = frame {
                let pending = self.pending_publisher_ack.ok_or_else(|| {
                    Error::ProtocolViolation(
                        "publisher sent a capability ack without a pending report".into(),
                    )
                })?;
                validate_capability_ack_binding(
                    ack,
                    pending.stream_id,
                    pending.protocol_version,
                    pending.dplpmtud,
                    pending.resolution,
                )?;
                true
            } else {
                false
            }
        } else {
            false
        };
        let result = if let Some(deadline) = self.setup_deadline {
            if let Ok(result) = deadline
                .run(None, self.control_send.send_frame(frame))
                .await
            {
                result
            } else {
                self.connection.close(VarInt::from_u32(0), b"setup timeout");
                return Err(Error::SetupTimeout);
            }
        } else {
            self.control_send.send_frame(frame).await
        };
        result?;
        if completes_setup {
            self.pending_publisher_ack = None;
            self.finish_setup()?;
        }
        Ok(())
    }
    /// Receive the next control frame from the peer.
    pub async fn next_control_frame(&mut self) -> Result<ControlFrame> {
        if let Some(deadline) = self.setup_deadline {
            if let Ok(result) = deadline.run(None, self.control_recv.next_frame()).await {
                result
            } else {
                self.connection.close(VarInt::from_u32(0), b"setup timeout");
                Err(Error::SetupTimeout)
            }
        } else {
            self.control_recv.next_frame().await
        }
    }
    /// Send a datagram payload.
    pub async fn send_datagram(&self, payload: &[u8]) -> Result<()> {
        if payload.is_empty() {
            return Err(Error::ProtocolViolation(
                "zero-length QUIC DATAGRAMs are forbidden".into(),
            ));
        }
        let max_datagram = self.max_datagram.load(Ordering::Acquire);
        if max_datagram == DATAGRAM_NEGOTIATING {
            return Err(Error::ProtocolViolation(
                "DATAGRAM capability negotiation is incomplete".into(),
            ));
        }
        if payload.len() > max_datagram {
            return Err(Error::DatagramTooLarge {
                len: payload.len(),
                max: max_datagram,
            });
        }
        self.connection
            .send_datagram(Bytes::copy_from_slice(payload))?;
        Ok(())
    }
    /// Receive the next datagram payload.
    pub async fn recv_datagram(&self) -> Result<Bytes> {
        let result = self.datagram_inbox.recv().await;
        if result.is_err() && self.datagram_inbox.has_protocol_terminal() {
            self.connection.close(
                VarInt::from_u32(DATAGRAM_PROTOCOL_ERROR_CODE),
                b"invalid DATAGRAM",
            );
        }
        result
    }
    /// Return the negotiated DATAGRAM payload limit for this session.
    pub fn max_datagram_size(&self) -> usize {
        match self.max_datagram.load(Ordering::Acquire) {
            DATAGRAM_NEGOTIATING => 0,
            limit => limit,
        }
    }
    /// Return `true` if DATAGRAM delivery is enabled for this session.
    pub fn datagram_enabled(&self) -> bool {
        let limit = self.max_datagram.load(Ordering::Acquire);
        limit != DATAGRAM_NEGOTIATING && limit > 0
    }
    /// Close the underlying QUIC connection.
    pub fn close(&self) {
        self.connection.close(VarInt::from_u32(0), &[]);
    }
    /// Wait for the underlying QUIC connection to close.
    pub async fn closed(&self) -> ConnectionError {
        self.connection.closed().await
    }
    fn finish_setup(&mut self) -> Result<()> {
        let Some(deadline) = self.setup_deadline else {
            return Err(Error::ProtocolViolation(
                "streaming setup was already completed".into(),
            ));
        };
        match self.setup_state.compare_exchange(
            SETUP_PENDING,
            SETUP_COMPLETE,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(SETUP_TIMED_OUT) => {
                self.connection.close(VarInt::from_u32(0), b"setup timeout");
                return Err(Error::SetupTimeout);
            }
            Err(_) => {
                return Err(Error::ProtocolViolation(
                    "streaming setup was already completed".into(),
                ));
            }
        }
        // The compare-exchange is the logical completion instant. A task that
        // was descheduled across the deadline must not win merely because the
        // watchdog has not yet been polled.
        if deadline.has_elapsed() {
            self.setup_state.store(SETUP_TIMED_OUT, Ordering::Release);
            self.connection.close(VarInt::from_u32(0), b"setup timeout");
            return Err(Error::SetupTimeout);
        }
        self.setup_deadline = None;
        self.setup_watchdog.abort();
        Ok(())
    }
    fn normalize_local_transport_capabilities(
        &self,
        mut capabilities: TransportCapabilities,
    ) -> TransportCapabilities {
        if capabilities.supports_datagram && self.configured_max_datagram > 0 {
            let configured = u16::try_from(self.configured_max_datagram).unwrap_or(u16::MAX);
            capabilities.max_segment_datagram_size =
                capabilities.max_segment_datagram_size.min(configured);
        } else {
            capabilities.supports_datagram = false;
            capabilities.max_segment_datagram_size = 0;
        }
        capabilities
    }
    fn apply_transport_resolution(
        &mut self,
        resolution: TransportCapabilityResolution,
    ) -> Result<()> {
        let negotiated = if resolution.use_datagram {
            usize::from(resolution.max_segment_datagram_size)
        } else {
            0
        };
        let max_datagram = negotiated;
        debug_assert!(max_datagram <= self.configured_max_datagram);
        if let Some(reason) = self.datagram_inbox.apply_policy(max_datagram) {
            self.connection.close(
                VarInt::from_u32(DATAGRAM_PROTOCOL_ERROR_CODE),
                b"invalid DATAGRAM",
            );
            return Err(Error::ProtocolViolation(reason));
        }
        self.max_datagram.store(max_datagram, Ordering::Release);
        Ok(())
    }
}

impl Drop for StreamingConnection {
    fn drop(&mut self) {
        self.datagram_task.abort();
        self.setup_watchdog.abort();
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
        validate_capability_report(&report)?;
        let mut record = Some(record);
        let local_frame = TransportCapabilitiesFrame {
            endpoint_role: streaming::CapabilityRole::Viewer,
            capabilities: conn.normalize_local_transport_capabilities(capabilities),
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
        let expected_stream_id = report.stream_id;
        let expected_version = report.protocol_version;
        let expected_dplpmtud = report.dplpmtud;
        conn.send_control_frame(&ControlFrame::CapabilityReport(report))
            .await?;
        loop {
            let response = conn.next_control_frame().await?;
            match response {
                ControlFrame::CapabilityAck(ack) => {
                    validate_capability_ack_binding(
                        &ack,
                        expected_stream_id,
                        expected_version,
                        expected_dplpmtud,
                        resolution,
                    )?;
                    conn.apply_transport_resolution(resolution)?;
                    conn.finish_setup()?;
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
            capabilities: conn.normalize_local_transport_capabilities(capabilities),
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
                    validate_capability_report(&report)?;
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
                    conn.apply_transport_resolution(resolution)?;
                    conn.pending_publisher_ack = Some(PendingCapabilityAck {
                        stream_id: report.stream_id,
                        protocol_version: report.protocol_version,
                        dplpmtud: report.dplpmtud,
                        resolution,
                    });
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
fn validate_capability_report(report: &CapabilityReport) -> Result<()> {
    if report.endpoint_role != streaming::CapabilityRole::Viewer {
        return Err(Error::ProtocolViolation(format!(
            "expected viewer capability report, received {role:?}",
            role = report.endpoint_role
        )));
    }
    if report.protocol_version == 0 {
        return Err(Error::ProtocolViolation(
            "capability report advertised zero protocol version".into(),
        ));
    }
    Ok(())
}
fn validate_capability_ack_binding(
    ack: &CapabilityAck,
    expected_stream_id: streaming::Hash,
    expected_version: u16,
    expected_dplpmtud: bool,
    resolution: TransportCapabilityResolution,
) -> Result<()> {
    if ack.stream_id != expected_stream_id {
        return Err(Error::ProtocolViolation(
            "capability ack stream_id does not match report".into(),
        ));
    }
    if ack.accepted_version != expected_version {
        return Err(Error::ProtocolViolation(format!(
            "capability ack accepted_version={} but report used {}",
            ack.accepted_version, expected_version
        )));
    }
    if ack.max_datagram_size != resolution.max_segment_datagram_size {
        return Err(Error::ProtocolViolation(format!(
            "capability ack reported max_datagram_size={} but negotiated {}",
            ack.max_datagram_size, resolution.max_segment_datagram_size
        )));
    }
    if ack.dplpmtud != expected_dplpmtud {
        return Err(Error::ProtocolViolation(format!(
            "capability ack dplpmtud={} but report used {}",
            ack.dplpmtud, expected_dplpmtud
        )));
    }
    Ok(())
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
        let frame = decode_control_frame(buf.as_slice())?;
        Ok(frame)
    }
}

fn decode_control_frame(bytes: &[u8]) -> Result<ControlFrame> {
    // `decode_from_bytes` derives a cumulative allocation budget from the
    // complete wire frame. This is intentionally narrower than the ambient
    // archive limit used by `deserialize_from`, while preserving supported
    // Norito compression and layout flags.
    Ok(decode_from_bytes(bytes)?)
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
                // Use a stable SNI value. Streaming authenticates the self-signed
                // certificate by fingerprint, so SNI is advisory but must still be
                // syntactically valid for the QUIC stack.
                server_name: "nsc.local".to_string(),
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
                server_name: "nsc.local".to_string(),
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
    // Quinn defaults to zero concurrent streams, which makes `open_uni()`/`accept_uni()` hang
    // indefinitely. Streaming sessions always use at least one uni stream in each direction for
    // control frames.
    transport.max_concurrent_uni_streams(VarInt::from_u32(16));
    transport.max_concurrent_bidi_streams(VarInt::from_u32(16));
    transport.datagram_receive_buffer_size(Some(settings.datagram_receive_buffer));
    transport.datagram_send_buffer_size(settings.datagram_send_buffer);
    transport.keep_alive_interval(Some(settings.idle_timeout / 2));
    let idle = IdleTimeout::try_from(settings.idle_timeout)
        .map_err(|e| Error::TransportConfig(e.to_string()))?;
    transport.max_idle_timeout(Some(idle));
    Ok(Arc::new(transport))
}
#[cfg(all(test, feature = "quic"))]
mod tests {
    use super::*;
    use iroha_crypto::streaming::StreamingSession;
    use norito::streaming::{
        AudioCapability, CapabilityAck, CapabilityFlags, CapabilityReport, CapabilityRole,
        ChunkDescriptor, EncryptionSuite, EntropyMode, FecScheme, FeedbackHintFrame, Hash,
        ManifestAnnounceFrame, ManifestV1, ProfileId, ReceiverReport, Resolution, StreamMetadata,
        TransportCapabilities,
    };
    use std::future::Future;
    use tokio::time::{Duration as TokioDuration, sleep, timeout};
    const TEST_TIMEOUT: TokioDuration = TokioDuration::from_secs(10);
    async fn within<T>(label: &'static str, fut: impl Future<Output = T>) -> T {
        timeout(TEST_TIMEOUT, fut)
            .await
            .unwrap_or_else(|_| panic!("{label} timed out"))
    }
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
    fn capability_report(protocol_version: u16) -> CapabilityReport {
        CapabilityReport {
            stream_id: hash(11),
            endpoint_role: CapabilityRole::Viewer,
            protocol_version,
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
            max_datagram_size: 1_024,
            dplpmtud: false,
        }
    }
    fn capability_ack(report: &CapabilityReport, max_datagram_size: u16) -> CapabilityAck {
        CapabilityAck {
            stream_id: report.stream_id,
            accepted_version: report.protocol_version,
            negotiated_features: report.feature_bits,
            max_datagram_size,
            dplpmtud: report.dplpmtud,
        }
    }
    fn capability_resolution() -> TransportCapabilityResolution {
        let capabilities = TransportCapabilities {
            max_segment_datagram_size: 1_024,
            ..TransportCapabilities::kyber768_default()
        };
        norito::streaming::resolve_transport_capabilities(&capabilities, &capabilities)
            .expect("default capabilities resolve")
    }
    #[test]
    fn control_frame_decode_uses_wire_derived_allocation_budget() {
        let frame = ControlFrame::CapabilityReport(capability_report(1));
        let encoded =
            norito::to_compressed_bytes(&frame, Some(norito::CompressionConfig::default()))
                .expect("compress control frame");
        assert!(matches!(
            decode_control_frame(&encoded).expect("valid compressed control frame"),
            ControlFrame::CapabilityReport(_)
        ));

        const DECLARED_EXPANSION: u64 = 63 * 1024 * 1024;
        const EXPECTED_WIRE_DERIVED_LIMIT: u64 = (MAX_CONTROL_FRAME_LEN as u64) * 64 + 64 * 1024;
        let mut forged = encoded;
        forged.resize(MAX_CONTROL_FRAME_LEN, 0);
        let length_offset = 4 + 1 + 1 + 16 + 1;
        forged[length_offset..length_offset + 8].copy_from_slice(&DECLARED_EXPANSION.to_le_bytes());
        let error = decode_control_frame(&forged)
            .expect_err("attacker-declared expansion must exceed the wire-derived budget");
        assert!(
            matches!(
                &error,
                Error::Norito(norito::Error::TotalAllocationExceeded { attempted, limit })
                    if *attempted == DECLARED_EXPANSION
                        && *limit == EXPECTED_WIRE_DERIVED_LIMIT
            ),
            "unexpected error: {error:?}"
        );
    }
    #[tokio::test]
    async fn datagram_inbox_bounds_count_and_bytes_and_revalidates_policy() {
        let inbox = DatagramInbox::new(MAX_DATAGRAM_INBOX_ENTRIES + 32);
        for _ in 0..(MAX_DATAGRAM_INBOX_ENTRIES + 32) {
            inbox
                .admit(Bytes::from_static(&[1]), 8)
                .expect("configured pre-negotiation DATAGRAM");
        }
        {
            let state = inbox.state.lock().expect("inbox state");
            assert_eq!(state.frames.len(), MAX_DATAGRAM_INBOX_ENTRIES);
            assert_eq!(state.bytes, MAX_DATAGRAM_INBOX_ENTRIES);
        }

        let inbox = DatagramInbox::new(8);
        inbox
            .admit(Bytes::from_static(&[1, 2, 3, 4]), 8)
            .expect("first DATAGRAM");
        inbox
            .admit(Bytes::from_static(&[5, 6, 7, 8]), 8)
            .expect("second DATAGRAM");
        inbox
            .admit(Bytes::from_static(&[9]), 8)
            .expect("unreliable inbox may drop at its byte bound");
        {
            let state = inbox.state.lock().expect("inbox state");
            assert_eq!(state.frames.len(), 2);
            assert_eq!(state.bytes, 8);
        }
        assert!(inbox.apply_policy(0).is_some());
        assert!(matches!(
            inbox.recv().await,
            Err(Error::ProtocolViolation(_))
        ));

        let disabled = DatagramInbox::new(8);
        assert!(disabled.apply_policy(0).is_none());
        assert!(
            disabled
                .admit(Bytes::from_static(&[1]), 8)
                .expect_err("policy update and admission share one lock")
                .contains("disabled")
        );
        let bounded = DatagramInbox::new(8);
        assert!(bounded.apply_policy(4).is_none());
        assert!(bounded.admit(Bytes::from_static(&[1, 2, 3, 4]), 8).is_ok());
        assert!(
            bounded
                .admit(Bytes::from_static(&[1, 2, 3, 4, 5]), 8)
                .expect_err("post-negotiation admission must use the negotiated bound")
                .contains("above negotiated maximum 4")
        );
    }
    #[test]
    fn streaming_certificate_pin_rejects_another_certificate() {
        let rcgen::CertifiedKey { cert, .. } =
            rcgen::generate_simple_self_signed(["nsc.local".to_owned()])
                .expect("generate certificate");
        let cert_der = cert.der().clone().into_owned();
        let fingerprint = crate::transport::certificate_fingerprint(cert_der.as_ref());
        let server_name =
            rustls::pki_types::ServerName::try_from("nsc.local").expect("valid server name");
        let now = rustls::pki_types::UnixTime::since_unix_epoch(Duration::ZERO);
        let verifier = crate::transport::CertificateKeyProofVerifier::pinned(fingerprint);
        verifier
            .verify_server_cert(&cert_der, &[], &server_name, &[], now)
            .expect("matching certificate fingerprint");
        let mut wrong_fingerprint = fingerprint;
        wrong_fingerprint[0] ^= 1;
        let verifier = crate::transport::CertificateKeyProofVerifier::pinned(wrong_fingerprint);
        let error = verifier
            .verify_server_cert(&cert_der, &[], &server_name, &[], now)
            .expect_err("different certificate fingerprint must fail closed");
        assert!(error.to_string().contains("fingerprint mismatch"));
    }
    #[tokio::test]
    async fn accept_rejects_client_that_never_opens_control_stream() {
        let settings = TransportConfigSettings {
            idle_timeout: TokioDuration::from_millis(100),
            ..TransportConfigSettings::default()
        };
        let server = match StreamingServer::bind(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            settings,
        )
        .await
        {
            Ok(server) => server,
            Err(Error::Io(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("quic test skipped: {err}");
                return;
            }
            Err(error) => panic!("server bind failed: {error:?}"),
        };
        let listen_addr = server.local_addr().expect("listen addr");
        let fingerprint = server.certificate_fingerprint();
        let raw_client = async move {
            let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap()).expect("endpoint");
            let verifier: Arc<dyn ServerCertVerifier> = Arc::new(
                crate::transport::CertificateKeyProofVerifier::pinned(fingerprint),
            );
            let mut tls_config = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(verifier)
                .with_no_client_auth();
            tls_config.alpn_protocols = vec![ALPN.to_vec()];
            let crypto = QuinnRustlsClientConfig::try_from(Arc::new(tls_config))
                .expect("QUIC client config");
            let mut client_config = ClientConfig::new(Arc::new(crypto));
            client_config.transport_config(build_transport_config(settings).expect("transport"));
            endpoint.set_default_client_config(client_config);
            let connection = endpoint
                .connect(listen_addr, "nsc.local")
                .expect("connect start")
                .await
                .expect("QUIC handshake");
            // Keep the transport alive beyond the setup deadline without ever
            // opening the required viewer-to-publisher control stream.
            sleep(TokioDuration::from_millis(250)).await;
            connection.close(VarInt::from_u32(0), &[]);
            endpoint.close(VarInt::from_u32(0), &[]);
        };
        let (accepted, ()) =
            tokio::join!(within("server setup deadline", server.accept()), raw_client);
        assert!(matches!(accepted, Err(Error::SetupTimeout)));
        within("server.shutdown", server.shutdown()).await;
    }
    #[tokio::test]
    async fn setup_watchdog_closes_an_active_connection_between_api_calls() {
        let settings = TransportConfigSettings {
            idle_timeout: TokioDuration::from_millis(150),
            ..TransportConfigSettings::default()
        };
        let server = match StreamingServer::bind(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            settings,
        )
        .await
        {
            Ok(server) => server,
            Err(Error::Io(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("quic test skipped: {err}");
                return;
            }
            Err(error) => panic!("server bind failed: {error:?}"),
        };
        let listen_addr = server.local_addr().expect("listen addr");
        let fingerprint = server.certificate_fingerprint();
        let multiaddr = format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port());
        let (accepted, connected) = tokio::join!(
            within("server.accept", server.accept()),
            within(
                "client.connect",
                StreamingClient::connect(&multiaddr, fingerprint, settings)
            )
        );
        let publisher = accepted.expect("publisher control streams");
        let mut client = connected.expect("viewer control streams");
        let raw_connection = client.connection().connection.clone();
        let transport_activity = tokio::spawn(async move {
            loop {
                if raw_connection
                    .send_datagram(Bytes::from_static(&[1]))
                    .is_err()
                {
                    break;
                }
                sleep(TokioDuration::from_millis(20)).await;
            }
        });

        within("setup watchdog close", publisher.closed()).await;
        transport_activity.abort();
        within("client.close", client.close()).await;
        within("server.shutdown", server.shutdown()).await;
    }
    #[test]
    fn capability_report_zero_protocol_version_rejected() {
        let report = capability_report(0);
        let err = validate_capability_report(&report).expect_err("zero protocol version rejected");
        match err {
            Error::ProtocolViolation(reason) => {
                assert!(reason.contains("zero protocol version"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn capability_report_non_viewer_role_rejected() {
        let mut report = capability_report(1);
        report.endpoint_role = CapabilityRole::Publisher;
        let err = validate_capability_report(&report).expect_err("non-viewer report role rejected");
        match err {
            Error::ProtocolViolation(reason) => {
                assert!(reason.contains("expected viewer capability report"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn capability_ack_binding_rejects_mismatched_report_echo() {
        let report = capability_report(3);
        let resolution = capability_resolution();
        let valid_ack = capability_ack(&report, resolution.max_segment_datagram_size);
        validate_capability_ack_binding(
            &valid_ack,
            report.stream_id,
            report.protocol_version,
            report.dplpmtud,
            resolution,
        )
        .expect("matching ack accepted");
        let mut wrong_stream = valid_ack.clone();
        wrong_stream.stream_id = hash(12);
        let err = validate_capability_ack_binding(
            &wrong_stream,
            report.stream_id,
            report.protocol_version,
            report.dplpmtud,
            resolution,
        )
        .expect_err("stream id mismatch rejected");
        match err {
            Error::ProtocolViolation(reason) => assert!(reason.contains("stream_id")),
            other => panic!("unexpected error: {other:?}"),
        }
        let mut wrong_version = valid_ack.clone();
        wrong_version.accepted_version = 2;
        let err = validate_capability_ack_binding(
            &wrong_version,
            report.stream_id,
            report.protocol_version,
            report.dplpmtud,
            resolution,
        )
        .expect_err("version mismatch rejected");
        match err {
            Error::ProtocolViolation(reason) => assert!(reason.contains("accepted_version")),
            other => panic!("unexpected error: {other:?}"),
        }
        let mut wrong_datagram = valid_ack;
        wrong_datagram.max_datagram_size = resolution.max_segment_datagram_size - 1;
        let err = validate_capability_ack_binding(
            &wrong_datagram,
            report.stream_id,
            report.protocol_version,
            report.dplpmtud,
            resolution,
        )
        .expect_err("datagram mismatch rejected");
        match err {
            Error::ProtocolViolation(reason) => assert!(reason.contains("max_datagram_size")),
            other => panic!("unexpected error: {other:?}"),
        }
        let mut wrong_dplpmtud = capability_ack(&report, resolution.max_segment_datagram_size);
        wrong_dplpmtud.dplpmtud = !report.dplpmtud;
        let err = validate_capability_ack_binding(
            &wrong_dplpmtud,
            report.stream_id,
            report.protocol_version,
            report.dplpmtud,
            resolution,
        )
        .expect_err("dplpmtud mismatch rejected");
        match err {
            Error::ProtocolViolation(reason) => assert!(reason.contains("dplpmtud")),
            other => panic!("unexpected error: {other:?}"),
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
        let server_certificate_fingerprint = server.certificate_fingerprint();
        let (datagram_read_tx, datagram_read_rx) = tokio::sync::oneshot::channel();
        let server_task = {
            let server = server.clone();
            async move {
                let incoming = within("endpoint.accept", server.endpoint.accept())
                    .await
                    .expect("incoming");
                let connecting = incoming.accept().expect("incoming.accept");
                let connection = within("server.handshake", connecting)
                    .await
                    .expect("handshake");
                let mut conn = within(
                    "server.streaming_conn",
                    StreamingConnection::new(connection, EndpointRole::Publisher, settings),
                )
                .await
                .expect("streaming conn");
                let publisher_caps = TransportCapabilities {
                    max_segment_datagram_size: settings.max_datagram_size as u16,
                    ..TransportCapabilities::kyber768_default()
                };
                let (report, transport_resolution) = within(
                    "publisher_handshake",
                    CapabilityNegotiation::publisher_handshake(
                        &mut conn,
                        publisher_caps.clone(),
                        |_| {},
                    ),
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
                within("send_ack", conn.send_control_frame(&ack_frame))
                    .await
                    .expect("ack");
                let announce = manifest();
                let transport_hash = transport_resolution.capabilities_hash();
                let mut manifest_with_hash = announce.clone();
                manifest_with_hash.manifest.transport_capabilities_hash = transport_hash;
                within(
                    "send_manifest_announce",
                    conn.send_control_frame(&ControlFrame::ManifestAnnounce(Box::new(
                        manifest_with_hash.clone(),
                    ))),
                )
                .await
                .expect("announce");
                let chunk = vec![0xDE, 0xAD, 0xBE, 0xEF];
                within("send_datagram", conn.send_datagram(&chunk))
                    .await
                    .expect("datagram");
                datagram_read_rx.await.expect("viewer read datagram");
                for _ in 0..1_024 {
                    conn.connection
                        .send_datagram(Bytes::new())
                        .expect("raw peer can queue an empty DATAGRAM burst");
                }
                let _ = within("wait_empty_datagram_close", conn.closed()).await;
            }
        };
        let viewer_task = async {
            let multiaddr = format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port());
            let parsed = parse_multiaddr(&multiaddr).expect("multiaddr");
            let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap()).expect("endpoint");
            let verifier: Arc<dyn ServerCertVerifier> =
                Arc::new(crate::transport::CertificateKeyProofVerifier::pinned(
                    server_certificate_fingerprint,
                ));
            let mut tls_config = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(verifier)
                .with_no_client_auth();
            tls_config.enable_early_data = false;
            tls_config.alpn_protocols = vec![ALPN.to_vec()];
            let tls_config = Arc::new(tls_config);
            let crypto =
                QuinnRustlsClientConfig::try_from(Arc::clone(&tls_config)).expect("tls config");
            let mut client_config = ClientConfig::new(Arc::new(crypto));
            let transport = build_transport_config(settings).expect("transport");
            client_config.transport_config(transport);
            endpoint.set_default_client_config(client_config);
            let server_addr = SocketAddr::new(parsed.host, parsed.port);
            let connecting = endpoint
                .connect(server_addr, &parsed.server_name)
                .expect("connect start");
            let connection = within("client.handshake", connecting)
                .await
                .expect("handshake");
            let connection = within(
                "client.streaming_conn",
                StreamingConnection::new(connection, EndpointRole::Viewer, settings),
            )
            .await
            .expect("streaming conn");
            let mut client = StreamingClient {
                endpoint,
                connection,
            };
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
            let (ack, transport_resolution) = within(
                "viewer_handshake",
                CapabilityNegotiation::viewer_handshake(
                    client.connection(),
                    viewer_caps,
                    report,
                    move |resolution| {
                        *recorded_clone.lock().expect("recorded hash lock") =
                            Some(resolution.capabilities_hash());
                    },
                ),
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
            let frame = within(
                "next_control_frame",
                client.connection().next_control_frame(),
            )
            .await
            .unwrap();
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
            let chunk = within("recv_datagram", client.connection().recv_datagram())
                .await
                .unwrap();
            assert_eq!(chunk.as_ref(), &[0xDE, 0xAD, 0xBE, 0xEF]);
            datagram_read_tx.send(()).expect("notify publisher");
            let error = within("reject_empty_datagram", client.connection().recv_datagram())
                .await
                .expect_err("empty DATAGRAM must close the session");
            match error {
                Error::ProtocolViolation(reason) => {
                    assert!(
                        reason.contains("zero-length"),
                        "unexpected reason: {reason}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
            within("client.close", client.close()).await;
        };
        tokio::join!(server_task, viewer_task);
        within("server.shutdown", server.shutdown()).await;
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
        let server_certificate_fingerprint = server.certificate_fingerprint();
        let server_task = {
            let server = server.clone();
            async move {
                let mut conn = within("server.accept", server.accept())
                    .await
                    .expect("accept");
                let mut session = StreamingSession::new(CapabilityRole::Publisher);
                let publisher_caps = TransportCapabilities::kyber768_default();
                let (report, resolution) = within(
                    "publisher_handshake",
                    CapabilityNegotiation::publisher_handshake(
                        &mut conn,
                        publisher_caps.clone(),
                        |_| {},
                    ),
                )
                .await
                .expect("handshake");
                session
                    .record_transport_capabilities(resolution.clone())
                    .expect("negotiated transport capabilities are valid");
                let ack = CapabilityAck {
                    stream_id: report.stream_id,
                    accepted_version: report.protocol_version,
                    negotiated_features: report.feature_bits,
                    max_datagram_size: resolution.max_segment_datagram_size,
                    dplpmtud: report.dplpmtud,
                };
                within(
                    "send_ack",
                    conn.send_control_frame(&ControlFrame::CapabilityAck(ack)),
                )
                .await
                .expect("ack");
                let mut received_hint = false;
                let mut received_report = false;
                while !(received_hint && received_report) {
                    match within("server.next_control_frame", conn.next_control_frame())
                        .await
                        .expect("frame")
                    {
                        ControlFrame::FeedbackHint(hint) => {
                            session
                                .process_feedback_hint(&hint)
                                .expect("feedback hint stream id matches session");
                            received_hint = true;
                        }
                        ControlFrame::ReceiverReport(report) => {
                            let parity = session
                                .process_receiver_report(&report)
                                .expect("receiver report stream id matches session");
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
            let mut client = within(
                "client.connect",
                StreamingClient::connect(
                    &format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port()),
                    server_certificate_fingerprint,
                    settings,
                ),
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
            let (_ack, _resolution) = within(
                "viewer_handshake",
                CapabilityNegotiation::viewer_handshake(
                    client.connection(),
                    viewer_caps,
                    report,
                    |_| {},
                ),
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
            within(
                "send_hint",
                client
                    .connection()
                    .send_control_frame(&ControlFrame::FeedbackHint(hint)),
            )
            .await
            .expect("send hint");
            within(
                "send_report",
                client
                    .connection()
                    .send_control_frame(&ControlFrame::ReceiverReport(recv_report)),
            )
            .await
            .expect("send report");
            // Wait for the publisher to process frames and close the connection before we tear down
            // our endpoint. Closing immediately can race with stream delivery and spuriously abort
            // the server-side receive loop.
            let _ = within("wait_server_close", client.connection().closed()).await;
            within("client.close", client.close()).await;
        };
        tokio::join!(server_task, viewer_task);
        within("server.shutdown", server.shutdown()).await;
    }
    #[tokio::test]
    async fn mtu_negotiation_clamps_asymmetric_local_limits_on_the_wire() {
        let server_settings = TransportConfigSettings {
            max_datagram_size: 1_200,
            ..TransportConfigSettings::default()
        };
        let viewer_settings = TransportConfigSettings {
            max_datagram_size: 800,
            ..TransportConfigSettings::default()
        };
        let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let server = match StreamingServer::bind(server_addr, server_settings).await {
            Ok(server) => server,
            Err(Error::Io(err)) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("quic test skipped: {err}");
                return;
            }
            Err(err) => panic!("server bind failed: {err:?}"),
        };
        let listen_addr = server.local_addr().expect("listen addr");
        let server_certificate_fingerprint = server.certificate_fingerprint();
        let (datagram_received_tx, datagram_received_rx) = tokio::sync::oneshot::channel();
        let server_task = {
            let server = server.clone();
            async move {
                let mut conn = within("server.accept", server.accept())
                    .await
                    .expect("accept");
                let mut publisher_caps = TransportCapabilities::kyber768_default();
                publisher_caps.max_segment_datagram_size = 1_350;
                let (report, resolution) = within(
                    "publisher_handshake",
                    CapabilityNegotiation::publisher_handshake(&mut conn, publisher_caps, |_| {}),
                )
                .await
                .expect("handshake");
                assert!(resolution.use_datagram);
                assert_eq!(resolution.max_segment_datagram_size, 800);
                assert_eq!(conn.max_datagram_size(), 800);
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
                within(
                    "send_ack",
                    conn.send_control_frame(&ControlFrame::CapabilityAck(ack)),
                )
                .await
                .expect("ack");
                let datagram = within("receive_negotiated_datagram", conn.recv_datagram())
                    .await
                    .expect("negotiated DATAGRAM delivery");
                assert_eq!(datagram.len(), 800);
                datagram_received_tx
                    .send(())
                    .expect("viewer still waits for DATAGRAM receipt");
            }
        };
        let viewer_task = async move {
            let mut client = within(
                "client.connect",
                StreamingClient::connect(
                    &format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port()),
                    server_certificate_fingerprint,
                    viewer_settings,
                ),
            )
            .await
            .expect("client");
            let mut viewer_caps = TransportCapabilities::kyber768_default();
            viewer_caps.max_segment_datagram_size = 1_350;
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
                max_datagram_size: 1_350,
                dplpmtud: true,
            };
            let (ack, resolution) = within(
                "viewer_handshake",
                CapabilityNegotiation::viewer_handshake(
                    client.connection(),
                    viewer_caps,
                    report,
                    |_| {},
                ),
            )
            .await
            .expect("handshake");
            assert!(resolution.use_datagram);
            assert_eq!(resolution.max_segment_datagram_size, 800);
            assert_eq!(ack.max_datagram_size, 800);
            assert_eq!(client.connection().max_datagram_size(), 800);
            assert!(client.connection().datagram_enabled());
            let payload = vec![0_u8; 801];
            let err = within(
                "send_oversized_datagram",
                client.connection().send_datagram(&payload),
            )
            .await
            .unwrap_err();
            match err {
                Error::DatagramTooLarge { max, .. } => assert_eq!(max, 800),
                other => panic!("unexpected error: {other:?}"),
            }
            within(
                "send_maximum_datagram",
                client.connection().send_datagram(&vec![0_u8; 800]),
            )
            .await
            .expect("maximum negotiated DATAGRAM size must be accepted");
            within("wait_datagram_receipt", datagram_received_rx)
                .await
                .expect("server received maximum-sized DATAGRAM");
            within("client.close", client.close()).await;
        };
        tokio::join!(server_task, viewer_task);
        within("server.shutdown", server.shutdown()).await;
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
        let server_certificate_fingerprint = server.certificate_fingerprint();
        let server_task = {
            let server = server.clone();
            async move {
                let mut conn = within("server.accept", server.accept())
                    .await
                    .expect("accept");
                let mut publisher_caps = TransportCapabilities::kyber768_default();
                publisher_caps.supports_datagram = false;
                let (report, resolution) = within(
                    "publisher_handshake",
                    CapabilityNegotiation::publisher_handshake(&mut conn, publisher_caps, |_| {}),
                )
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
                within(
                    "send_ack",
                    conn.send_control_frame(&ControlFrame::CapabilityAck(ack)),
                )
                .await
                .expect("ack");
                let _ = within("wait_client_close", conn.closed()).await;
            }
        };
        let viewer_task = async move {
            let mut client = within(
                "client.connect",
                StreamingClient::connect(
                    &format!("/ip4/127.0.0.1/udp/{}/quic", listen_addr.port()),
                    server_certificate_fingerprint,
                    settings,
                ),
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
            let (ack, resolution) = within(
                "viewer_handshake",
                CapabilityNegotiation::viewer_handshake(
                    client.connection(),
                    viewer_caps,
                    report,
                    |_| {},
                ),
            )
            .await
            .expect("handshake");
            assert!(!resolution.use_datagram);
            assert_eq!(resolution.max_segment_datagram_size, 0);
            assert_eq!(ack.max_datagram_size, 0);
            assert_eq!(client.connection().max_datagram_size(), 0);
            assert!(!client.connection().datagram_enabled());
            let payload = vec![0_u8; 1];
            let err = within("send_datagram", client.connection().send_datagram(&payload))
                .await
                .unwrap_err();
            match err {
                Error::DatagramTooLarge { max, .. } => assert_eq!(max, 0),
                other => panic!("unexpected error: {other:?}"),
            }
            client
                .connection()
                .connection
                .send_datagram(Bytes::from_static(&[1]))
                .expect("raw peer can attempt to violate negotiated policy");
            let _ = within("wait_server_close", client.connection().closed()).await;
            within("client.close", client.close()).await;
        };
        tokio::join!(server_task, viewer_task);
        within("server.shutdown", server.shutdown()).await;
    }
}
