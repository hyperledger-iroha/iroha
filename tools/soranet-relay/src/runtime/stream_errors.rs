#[derive(Debug, Error)]
enum ExitStreamError {
    #[error("failed to read route open frame: {0}")]
    Read(io::Error),
    #[error(transparent)]
    Decode(#[from] RouteOpenFrameError),
    #[error("{stream} exit routing disabled in configuration")]
    StreamDisabled { stream: &'static str },
    #[error(
        "{stream} channel {channel} is unavailable because first-release filesystem route publication is disabled"
    )]
    FilesystemPublicationDisabled {
        stream: &'static str,
        channel: String,
    },
    #[cfg(any())]
    #[error(transparent)]
    RouteLookup(#[from] RouteCatalogError),
    #[cfg(any())]
    #[error("{stream} channel {channel} requires authenticated viewers")]
    RouteRequiresAuthentication {
        stream: &'static str,
        channel: String,
    },
    #[cfg(any())]
    #[error("{stream} adapter connection timed out after {timeout:?}")]
    AdapterTimeout {
        stream: &'static str,
        timeout: Duration,
    },
    #[cfg(any())]
    #[error("{stream} adapter connection failed: {error}")]
    AdapterConnect {
        stream: &'static str,
        #[source]
        error: tungstenite::Error,
    },
    #[cfg(any())]
    #[error("failed to encode {stream} handshake: {error}")]
    HandshakeEncode {
        stream: &'static str,
        #[source]
        error: norito::Error,
    },
    #[cfg(any())]
    #[error("failed to send data to {stream} adapter: {error}")]
    AdapterSend {
        stream: &'static str,
        #[source]
        error: tungstenite::Error,
    },
    #[cfg(any())]
    #[error("failed to receive data from {stream} adapter: {error}")]
    AdapterReceive {
        stream: &'static str,
        #[source]
        error: tungstenite::Error,
    },
    #[cfg(any())]
    #[error("exit stream read error: {0}")]
    RecvRead(io::Error),
    #[cfg(any())]
    #[error("exit stream write error: {0}")]
    SendWrite(io::Error),
    #[cfg(any())]
    #[error("failed to finish exit stream: {0}")]
    SendFinish(io::Error),
}
impl ExitStreamError {
    fn compliance_context(&self) -> (Option<&'static str>, Option<&str>, &'static str) {
        match self {
            Self::StreamDisabled { stream } => (Some(*stream), None, "stream_disabled"),
            Self::FilesystemPublicationDisabled { stream, channel } => (
                Some(*stream),
                Some(channel),
                "filesystem_publication_disabled",
            ),
            #[cfg(any())]
            Self::RouteRequiresAuthentication { stream, channel } => (
                Some(*stream),
                Some(channel),
                "route_requires_authentication",
            ),
            #[cfg(any())]
            Self::AdapterTimeout { stream, .. } => (Some(*stream), None, "adapter_timeout"),
            #[cfg(any())]
            Self::AdapterConnect { stream, .. } => (Some(*stream), None, "adapter_connect"),
            #[cfg(any())]
            Self::HandshakeEncode { stream, .. } => (Some(*stream), None, "handshake_encode"),
            #[cfg(any())]
            Self::AdapterSend { stream, .. } => (Some(*stream), None, "adapter_send"),
            #[cfg(any())]
            Self::AdapterReceive { stream, .. } => (Some(*stream), None, "adapter_receive"),
            #[cfg(any())]
            Self::RouteLookup(_) => (None, None, "route_lookup"),
            Self::Read(_) => (None, None, "route_frame_read"),
            Self::Decode(_) => (None, None, "route_frame_decode"),
            #[cfg(any())]
            Self::RecvRead(_) => (None, None, "exit_stream_read"),
            #[cfg(any())]
            Self::SendWrite(_) => (None, None, "exit_stream_write"),
            #[cfg(any())]
            Self::SendFinish(_) => (None, None, "exit_stream_finish"),
        }
    }
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
    #[error("measurement frame allocation failed within its bounded corridor")]
    Allocation,
    #[error("failed to decode relay bandwidth proof: {0}")]
    Decode(#[from] norito::codec::Error),
    #[error("measurement frame contains {0} trailing bytes after decoding proof")]
    TrailingBytes(usize),
    #[error("measurement stream read error: {0}")]
    Read(io::Error),
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
    #[error("ticket replay store failed closed: {0}")]
    ReplayStore(String),
    #[error("admission verifier capacity is busy")]
    AdmissionWorkUnavailable,
    #[error("admission verifier worker failed: {0}")]
    AdmissionWorker(String),
    #[error("authenticated relay transport trust has expired")]
    TransportTrustExpired,
    #[error("missing admission challenge")]
    MissingChallenge,
    #[error("token decode failed: {0}")]
    TokenDecode(TokenDecodeError),
    #[error("token verification failed: {0}")]
    Token(#[from] TokenPolicyError),
    #[error("vpn helper ticket verification failed: {0}")]
    HelperTicket(#[from] VpnHelperTicketError),
}
