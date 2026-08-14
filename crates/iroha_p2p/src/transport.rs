//! Transport and handshake scaffolding traits.
//!
//! This module provides thin abstractions intended to support optional
//! transports (e.g., QUIC) and handshakes (e.g., Noise/TLS) behind
//! feature flags without affecting the default TCP path.
#[cfg(any(feature = "p2p_tls", feature = "quic"))]
use rustls::{
    DigitallySignedStruct, Error as RustlsError, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
#[cfg(any(feature = "p2p_tls", feature = "quic"))]
static SELF_SIGNED_SIGNATURE_ALGORITHMS: std::sync::LazyLock<
    rustls::crypto::WebPkiSupportedAlgorithms,
> = std::sync::LazyLock::new(|| {
    rustls::crypto::ring::default_provider().signature_verification_algorithms
});
/// Certificate verifier for self-signed transport certificates.
///
/// An unpinned verifier deliberately leaves naming and trust-root validation to the
/// application identity layer, but still verifies TLS `CertificateVerify`. That proof
/// of possession is required before a certificate fingerprint can serve as a channel
/// binding: accepting a signature produced by an unrelated key would let an attacker
/// replay another node's certificate bytes. A pinned verifier additionally authenticates
/// the exact leaf fingerprint at the transport layer.
#[cfg(any(feature = "p2p_tls", feature = "quic"))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct CertificateKeyProofVerifier {
    expected_fingerprint: Option<[u8; iroha_crypto::Hash::LENGTH]>,
}
#[cfg(any(feature = "p2p_tls", feature = "quic"))]
impl CertificateKeyProofVerifier {
    /// Verify certificate-key possession while deferring identity to the signed P2P handshake.
    pub(crate) const fn unpinned() -> Self {
        Self {
            expected_fingerprint: None,
        }
    }
    /// Verify certificate-key possession and require one exact certificate fingerprint.
    pub(crate) const fn pinned(expected_fingerprint: [u8; iroha_crypto::Hash::LENGTH]) -> Self {
        Self {
            expected_fingerprint: Some(expected_fingerprint),
        }
    }
}
#[cfg(any(feature = "p2p_tls", feature = "quic"))]
impl ServerCertVerifier for CertificateKeyProofVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, RustlsError> {
        if let Some(expected) = self.expected_fingerprint {
            let actual = certificate_fingerprint(end_entity.as_ref());
            if actual != expected {
                return Err(RustlsError::General(
                    "transport certificate fingerprint mismatch".to_owned(),
                ));
            }
        }
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &SELF_SIGNED_SIGNATURE_ALGORITHMS,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &SELF_SIGNED_SIGNATURE_ALGORITHMS,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        SELF_SIGNED_SIGNATURE_ALGORITHMS.supported_schemes()
    }
}
#[cfg(feature = "quic")]
pub mod quic {
    #![allow(clippy::missing_errors_doc)]
    //! QUIC transport integration (feature-gated, optional).
    //!
    //! This module provides a QUIC dialer that can be reused across many
    //! outbound dials. Self-signed certificates are accepted because peer
    //! identity is enforced by the signed application handshake, but TLS still
    //! verifies that the server owns the certificate key. The signed handshake
    //! binds the presented certificate fingerprint to the active session. ALPN
    //! is fixed.
    use quinn::{
        ClientConfig, Connection, Endpoint, IdleTimeout, RecvStream, SendStream, TransportConfig,
        VarInt, crypto::rustls::QuicClientConfig as QuinnRustlsClientConfig,
    };
    use rustls::client::danger::ServerCertVerifier;
    use std::{io, sync::Arc, time::Duration};
    /// ALPN negotiated for Iroha P2P QUIC connections.
    pub const P2P_ALPN: &[u8] = b"iroha-p2p/1";
    /// Number of bidirectional streams used by one Iroha P2P QUIC session.
    pub const P2P_BIDI_STREAMS_PER_CONNECTION: u32 = 2;
    /// Smallest per-direction flow-control allocation used by the budget split.
    pub const FLOW_CONTROL_GRANULE_BYTES: usize = 64 * 1024;
    const FLOW_CONTROL_DIRECTIONS_PER_CONNECTION: usize = 4;
    // Quinn's endpoint configuration expresses the maximum UDP payload as a
    // `u16`, and its receive path additionally caps one datagram at 64 KiB.
    // The first datagram retained by `Incoming` is explicitly excluded from
    // both configured incoming-buffer limits, so reserve a whole granule for it.
    const INCOMING_FIRST_PACKET_RESERVE_BYTES: usize = FLOW_CONTROL_GRANULE_BYTES;
    /// Inputs used to derive bounded QUIC flow-control credit.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct FlowControlConfig {
        /// Largest encrypted P2P frame body accepted on a stream.
        pub max_encrypted_frame_bytes: usize,
        /// Process-wide upper bound on simultaneously active P2P connections.
        pub max_total_connections: usize,
        /// Process-wide byte budget shared by QUIC send and receive flow control.
        pub process_budget_bytes: usize,
    }
    impl Default for FlowControlConfig {
        fn default() -> Self {
            Self {
                max_encrypted_frame_bytes: crate::MAX_ENCRYPTED_FRAME_BYTES,
                max_total_connections: 1,
                process_budget_bytes: FLOW_CONTROL_DIRECTIONS_PER_CONNECTION
                    * FLOW_CONTROL_GRANULE_BYTES,
            }
        }
    }
    /// Checked QUIC flow-control limits shared by client and server endpoints.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct FlowControlGeometry {
        /// Credit granted to each individual stream.
        pub stream_receive_window_bytes: u64,
        /// Aggregate receive credit granted to one connection.
        pub connection_receive_window_bytes: u64,
        /// Aggregate send credit retained for one connection.
        pub connection_send_window_bytes: u64,
    }
    /// Checked endpoint-side admission and datagram buffer geometry.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct EndpointBufferGeometry {
        /// Quinn `Incoming` records allowed before the application accepts them.
        pub max_incoming: usize,
        /// Bytes buffered after the first packet for one pending `Incoming`.
        pub incoming_buffer_size_bytes: u64,
        /// Aggregate post-first-packet bytes for all pending `Incoming` records.
        pub incoming_buffer_size_total_bytes: u64,
        /// Payload-byte reserve for the first packet retained by one `Incoming`.
        pub incoming_first_packet_reserve_bytes: usize,
        /// Aggregate first-packet reserve across all admitted `Incoming` records.
        pub incoming_first_packet_reserve_total_bytes: usize,
        /// Configured QUIC datagram buffers retained per active connection.
        pub datagram_buffer_bytes_per_connection: usize,
        /// Checked aggregate datagram buffer geometry for all connections.
        pub datagram_buffer_bytes_total: usize,
        /// Checked aggregate stream flow-control geometry for all connections.
        pub flow_control_buffer_bytes_total: usize,
        /// Combined flow-control, pending-Incoming payload, and datagram buffer bound.
        pub endpoint_buffer_bytes_total: usize,
    }
    /// Derive bounded Quinn endpoint admission and datagram buffer limits.
    ///
    /// Pending handshakes get one flow-control granule after their first packet,
    /// plus a separate granule for the first packet that Quinn excludes from
    /// both incoming-buffer limits. Since [`flow_control_geometry`] requires
    /// four such granules per active connection, each aggregate pending region
    /// fits within one quarter of the same minimum process geometry. Datagram
    /// buffers are separately configured, but their per-connection sum and
    /// aggregate multiplication are still checked explicitly. Fixed Quinn
    /// object and allocator metadata is count-bounded by `max_incoming`, but is
    /// not part of this payload/flow-credit byte geometry.
    pub fn endpoint_buffer_geometry(
        flow_control: FlowControlConfig,
        max_incoming: usize,
        datagram_receive_buffer: Option<usize>,
        datagram_send_buffer: usize,
    ) -> io::Result<EndpointBufferGeometry> {
        let flow_geometry = flow_control_geometry(flow_control)?;
        if max_incoming == 0 || max_incoming > flow_control.max_total_connections {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "QUIC max_incoming ({max_incoming}) must be between 1 and max_total_connections ({})",
                    flow_control.max_total_connections
                ),
            ));
        }
        let incoming_buffer_size_total = max_incoming
            .checked_mul(FLOW_CONTROL_GRANULE_BYTES)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC aggregate Incoming buffer geometry overflows usize",
                )
            })?;
        let incoming_buffer_size_bytes =
            u64::try_from(FLOW_CONTROL_GRANULE_BYTES).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC per-Incoming buffer does not fit u64",
                )
            })?;
        let incoming_buffer_size_total_bytes =
            u64::try_from(incoming_buffer_size_total).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC aggregate Incoming buffer does not fit u64",
                )
            })?;
        let incoming_first_packet_reserve_total_bytes = max_incoming
            .checked_mul(INCOMING_FIRST_PACKET_RESERVE_BYTES)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC aggregate Incoming first-packet reserve overflows usize",
                )
            })?;
        let datagram_buffer_bytes_per_connection = datagram_receive_buffer
            .unwrap_or(0)
            .checked_add(datagram_send_buffer)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC per-connection datagram buffer geometry overflows usize",
                )
            })?;
        let datagram_buffer_bytes_total = datagram_buffer_bytes_per_connection
            .checked_mul(flow_control.max_total_connections)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC aggregate datagram buffer geometry overflows usize",
                )
            })?;
        let flow_control_buffer_bytes_per_connection = flow_geometry
            .connection_receive_window_bytes
            .checked_add(flow_geometry.connection_send_window_bytes)
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC per-connection flow-control buffer geometry overflows usize",
                )
            })?;
        let flow_control_buffer_bytes_total = flow_control_buffer_bytes_per_connection
            .checked_mul(flow_control.max_total_connections)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC aggregate flow-control buffer geometry overflows usize",
                )
            })?;
        let endpoint_buffer_bytes_total = flow_control_buffer_bytes_total
            .checked_add(incoming_buffer_size_total)
            .and_then(|bytes| bytes.checked_add(incoming_first_packet_reserve_total_bytes))
            .and_then(|bytes| bytes.checked_add(datagram_buffer_bytes_total))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC combined endpoint buffer geometry overflows usize",
                )
            })?;
        Ok(EndpointBufferGeometry {
            max_incoming,
            incoming_buffer_size_bytes,
            incoming_buffer_size_total_bytes,
            incoming_first_packet_reserve_bytes: INCOMING_FIRST_PACKET_RESERVE_BYTES,
            incoming_first_packet_reserve_total_bytes,
            datagram_buffer_bytes_per_connection,
            datagram_buffer_bytes_total,
            flow_control_buffer_bytes_total,
            endpoint_buffer_bytes_total,
        })
    }
    /// Derive the per-connection QUIC flow-control geometry from a process budget.
    ///
    /// A connection has two receive streams and may retain the same aggregate
    /// amount for sending. Consequently, `4 * max_total_connections * W` is
    /// bounded by `process_budget_bytes`, where `W` is the stream window. Large
    /// frames do not require equally large static credit: QUIC replenishes the
    /// window as the application consumes stream bytes.
    pub fn flow_control_geometry(cfg: FlowControlConfig) -> io::Result<FlowControlGeometry> {
        if cfg.max_encrypted_frame_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "QUIC max encrypted frame bytes must be non-zero",
            ));
        }
        if cfg.max_total_connections == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "QUIC max total connections must be non-zero",
            ));
        }
        let minimum_budget = cfg
            .max_total_connections
            .checked_mul(FLOW_CONTROL_DIRECTIONS_PER_CONNECTION)
            .and_then(|value| value.checked_mul(FLOW_CONTROL_GRANULE_BYTES))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC minimum flow-control budget overflows usize",
                )
            })?;
        if cfg.process_budget_bytes < minimum_budget {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "QUIC process flow-control budget ({}) must be at least {} bytes for {} connections",
                    cfg.process_budget_bytes, minimum_budget, cfg.max_total_connections
                ),
            ));
        }
        let complete_frame_bytes = cfg
            .max_encrypted_frame_bytes
            .checked_add(crate::P2P_FRAME_LENGTH_PREFIX_BYTES)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC complete encrypted-frame length overflows usize",
                )
            })?;
        let denominator = cfg
            .max_total_connections
            .checked_mul(FLOW_CONTROL_DIRECTIONS_PER_CONNECTION)
            .expect("minimum-budget calculation already checked this product");
        let budget_share = cfg.process_budget_bytes / denominator;
        let rounded_budget_share =
            (budget_share / FLOW_CONTROL_GRANULE_BYTES) * FLOW_CONTROL_GRANULE_BYTES;
        let stream_window = complete_frame_bytes.min(rounded_budget_share);
        let connection_window = stream_window.checked_mul(2).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "QUIC per-connection flow-control window overflows usize",
            )
        })?;
        let stream_receive_window_bytes = u64::try_from(stream_window).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "QUIC stream flow-control window does not fit u64",
            )
        })?;
        let connection_window_bytes = u64::try_from(connection_window).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "QUIC connection flow-control window does not fit u64",
            )
        })?;
        VarInt::from_u64(stream_receive_window_bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
        VarInt::from_u64(connection_window_bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
        Ok(FlowControlGeometry {
            stream_receive_window_bytes,
            connection_receive_window_bytes: connection_window_bytes,
            connection_send_window_bytes: connection_window_bytes,
        })
    }
    /// Apply the protocol stream count and checked flow-control geometry.
    pub fn configure_flow_control(
        transport: &mut TransportConfig,
        cfg: FlowControlConfig,
    ) -> io::Result<FlowControlGeometry> {
        let geometry = flow_control_geometry(cfg)?;
        let stream_receive_window = VarInt::from_u64(geometry.stream_receive_window_bytes)
            .expect("flow-control geometry validated the stream window");
        let connection_receive_window = VarInt::from_u64(geometry.connection_receive_window_bytes)
            .expect("flow-control geometry validated the connection window");
        transport
            .max_concurrent_bidi_streams(VarInt::from_u32(P2P_BIDI_STREAMS_PER_CONNECTION))
            .max_concurrent_uni_streams(VarInt::from_u32(0))
            .stream_receive_window(stream_receive_window)
            .receive_window(connection_receive_window)
            .send_window(geometry.connection_send_window_bytes);
        Ok(geometry)
    }
    /// QUIC transport tuning for outbound dials.
    #[derive(Clone, Copy, Debug)]
    pub struct DialerConfig {
        /// QUIC idle timeout (transport-level), if set.
        pub max_idle_timeout: Option<Duration>,
        /// QUIC keep-alive interval, if set.
        pub keep_alive_interval: Option<Duration>,
        /// Receive buffer reserved for QUIC datagrams on each connection (bytes).
        /// `None` disables datagrams.
        pub datagram_receive_buffer: Option<usize>,
        /// Send buffer reserved for QUIC datagrams on each connection (bytes). Set to 0 to disable.
        pub datagram_send_buffer: usize,
        /// Checked process-wide flow-control geometry inputs.
        pub flow_control: FlowControlConfig,
    }
    impl Default for DialerConfig {
        fn default() -> Self {
            Self {
                max_idle_timeout: None,
                // A small keep-alive keeps common NAT mappings fresh and reduces idle drops.
                keep_alive_interval: Some(Duration::from_secs(10)),
                datagram_receive_buffer: None,
                datagram_send_buffer: 0,
                flow_control: FlowControlConfig::default(),
            }
        }
    }
    /// Reusable outbound QUIC dialer.
    #[derive(Clone, Debug)]
    pub struct Dialer {
        endpoint: Endpoint,
    }
    impl Dialer {
        /// Create a QUIC dialer bound to `bind_addr` (usually `0.0.0.0:0`).
        pub fn bind(bind_addr: std::net::SocketAddr, cfg: DialerConfig) -> io::Result<Self> {
            let mut endpoint = Endpoint::client(bind_addr)?;
            let verifier: Arc<dyn ServerCertVerifier> =
                Arc::new(super::CertificateKeyProofVerifier::unpinned());
            let mut tls = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(verifier)
                .with_no_client_auth();
            // Application authentication happens after the transport
            // handshake; replayable 0-RTT bytes must never enter that path.
            tls.enable_early_data = false;
            tls.alpn_protocols = vec![P2P_ALPN.to_vec()];
            let tls = Arc::new(tls);
            let crypto = QuinnRustlsClientConfig::try_from(tls)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            let mut client = ClientConfig::new(Arc::new(crypto));
            let transport = build_transport_config(cfg)?;
            client.transport_config(transport);
            endpoint.set_default_client_config(client);
            Ok(Self { endpoint })
        }
        /// Connect to `remote` and return an established connection.
        pub async fn connect(
            &self,
            remote: std::net::SocketAddr,
            server_name: &str,
        ) -> io::Result<Connection> {
            let connecting = self
                .endpoint
                .connect(remote, server_name)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            connecting
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
        }
        /// Connect and open a single bi-directional stream.
        pub async fn connect_and_open_bi(
            &self,
            remote: std::net::SocketAddr,
            server_name: &str,
        ) -> io::Result<(Connection, SendStream, RecvStream)> {
            let connection = self.connect(remote, server_name).await?;
            let (send, recv) = connection
                .open_bi()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            Ok((connection, send, recv))
        }
        /// Connect and open two bi-directional streams (recommended for separating priorities).
        pub async fn connect_and_open_two_bi(
            &self,
            remote: std::net::SocketAddr,
            server_name: &str,
        ) -> io::Result<(
            Connection,
            (SendStream, RecvStream),
            (SendStream, RecvStream),
        )> {
            let connection = self.connect(remote, server_name).await?;
            let hi = connection
                .open_bi()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            let lo = connection
                .open_bi()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            Ok((connection, hi, lo))
        }
    }
    fn build_transport_config(cfg: DialerConfig) -> io::Result<Arc<TransportConfig>> {
        endpoint_buffer_geometry(
            cfg.flow_control,
            cfg.flow_control.max_total_connections,
            cfg.datagram_receive_buffer,
            cfg.datagram_send_buffer,
        )?;
        let mut transport = TransportConfig::default();
        configure_flow_control(&mut transport, cfg.flow_control)?;
        if let Some(timeout) = cfg.max_idle_timeout {
            let idle = IdleTimeout::try_from(timeout)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
            transport.max_idle_timeout(Some(idle));
        }
        transport.keep_alive_interval(cfg.keep_alive_interval);
        transport.datagram_receive_buffer_size(cfg.datagram_receive_buffer);
        transport.datagram_send_buffer_size(cfg.datagram_send_buffer);
        Ok(Arc::new(transport))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        const MIB: usize = 1024 * 1024;
        #[test]
        fn core_profile_flow_control_stays_within_process_budget() {
            let cfg = FlowControlConfig {
                max_encrypted_frame_bytes: 16 * MIB,
                max_total_connections: 120,
                process_budget_bytes: 128 * MIB,
            };
            let geometry = flow_control_geometry(cfg).expect("valid core geometry");
            assert_eq!(geometry.stream_receive_window_bytes, 256 * 1024);
            assert_eq!(geometry.connection_receive_window_bytes, 512 * 1024);
            assert_eq!(geometry.connection_send_window_bytes, 512 * 1024);
            let total = usize::try_from(
                geometry
                    .connection_receive_window_bytes
                    .checked_add(geometry.connection_send_window_bytes)
                    .expect("small connection budget"),
            )
            .expect("connection budget fits usize")
            .checked_mul(cfg.max_total_connections)
            .expect("small process budget");
            assert!(total <= cfg.process_budget_bytes);
        }
        #[test]
        fn home_profile_uses_larger_window_under_same_process_budget() {
            let geometry = flow_control_geometry(FlowControlConfig {
                max_encrypted_frame_bytes: 16 * MIB,
                max_total_connections: 32,
                process_budget_bytes: 128 * MIB,
            })
            .expect("valid home geometry");
            assert_eq!(geometry.stream_receive_window_bytes, MIB as u64);
            assert_eq!(geometry.connection_receive_window_bytes, (2 * MIB) as u64);
            assert_eq!(geometry.connection_send_window_bytes, (2 * MIB) as u64);
        }
        #[test]
        fn budget_one_byte_below_minimum_is_rejected() {
            let connections = 7;
            let minimum =
                connections * FLOW_CONTROL_DIRECTIONS_PER_CONNECTION * FLOW_CONTROL_GRANULE_BYTES;
            let error = flow_control_geometry(FlowControlConfig {
                max_encrypted_frame_bytes: MIB,
                max_total_connections: connections,
                process_budget_bytes: minimum - 1,
            })
            .expect_err("undersized process budget must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains("must be at least"));
        }
        #[test]
        fn small_frame_window_includes_stream_length_prefix() {
            let geometry = flow_control_geometry(FlowControlConfig {
                max_encrypted_frame_bytes: 4096,
                max_total_connections: 1,
                process_budget_bytes: 4 * FLOW_CONTROL_GRANULE_BYTES,
            })
            .expect("valid small-frame geometry");
            assert_eq!(
                geometry.stream_receive_window_bytes,
                (4096 + crate::P2P_FRAME_LENGTH_PREFIX_BYTES) as u64
            );
        }
        #[test]
        fn zero_connections_and_arithmetic_overflow_are_rejected() {
            let zero_connections = flow_control_geometry(FlowControlConfig {
                max_encrypted_frame_bytes: 1,
                max_total_connections: 0,
                process_budget_bytes: usize::MAX,
            })
            .expect_err("zero connections must fail closed");
            assert_eq!(zero_connections.kind(), io::ErrorKind::InvalidInput);
            let overflow = flow_control_geometry(FlowControlConfig {
                max_encrypted_frame_bytes: 1,
                max_total_connections: usize::MAX,
                process_budget_bytes: usize::MAX,
            })
            .expect_err("minimum budget multiplication must be checked");
            assert_eq!(overflow.kind(), io::ErrorKind::InvalidInput);
        }
        #[test]
        fn endpoint_buffers_follow_connection_cap_with_checked_aggregates() {
            let cfg = FlowControlConfig {
                max_encrypted_frame_bytes: MIB,
                max_total_connections: 4,
                process_budget_bytes: 4 * 4 * FLOW_CONTROL_GRANULE_BYTES,
            };
            let geometry = endpoint_buffer_geometry(cfg, 2, Some(1024), 2048)
                .expect("small endpoint geometry must fit");
            assert_eq!(geometry.max_incoming, 2);
            assert_eq!(
                geometry.incoming_buffer_size_bytes,
                FLOW_CONTROL_GRANULE_BYTES as u64
            );
            assert_eq!(
                geometry.incoming_buffer_size_total_bytes,
                (2 * FLOW_CONTROL_GRANULE_BYTES) as u64
            );
            assert_eq!(
                geometry.incoming_first_packet_reserve_bytes,
                FLOW_CONTROL_GRANULE_BYTES
            );
            assert_eq!(
                geometry.incoming_first_packet_reserve_total_bytes,
                2 * FLOW_CONTROL_GRANULE_BYTES
            );
            assert_eq!(geometry.datagram_buffer_bytes_per_connection, 3072);
            assert_eq!(geometry.datagram_buffer_bytes_total, 12_288);
            assert_eq!(geometry.flow_control_buffer_bytes_total, 1_048_576);
            assert_eq!(geometry.endpoint_buffer_bytes_total, 1_323_008);
        }
        #[test]
        fn cap_one_and_datagram_overflow_fail_closed() {
            let cap_one = FlowControlConfig {
                max_encrypted_frame_bytes: MIB,
                max_total_connections: 1,
                process_budget_bytes: 4 * FLOW_CONTROL_GRANULE_BYTES,
            };
            let geometry = endpoint_buffer_geometry(cap_one, 1, None, 0)
                .expect("cap-one endpoint geometry must be valid");
            assert_eq!(geometry.max_incoming, 1);
            assert_eq!(
                geometry.incoming_buffer_size_total_bytes,
                geometry.incoming_buffer_size_bytes
            );
            assert_eq!(
                geometry.endpoint_buffer_bytes_total,
                6 * FLOW_CONTROL_GRANULE_BYTES
            );
            let excessive_incoming = endpoint_buffer_geometry(cap_one, 2, None, 0)
                .expect_err("Incoming cap may not exceed total connection geometry");
            assert_eq!(excessive_incoming.kind(), io::ErrorKind::InvalidInput);
            let overflow = endpoint_buffer_geometry(
                FlowControlConfig {
                    max_encrypted_frame_bytes: 1,
                    max_total_connections: 2,
                    process_budget_bytes: 8 * FLOW_CONTROL_GRANULE_BYTES,
                },
                2,
                Some(usize::MAX),
                1,
            )
            .expect_err("per-connection datagram addition must be checked");
            assert_eq!(overflow.kind(), io::ErrorKind::InvalidInput);
            let aggregate_overflow = endpoint_buffer_geometry(
                FlowControlConfig {
                    max_encrypted_frame_bytes: 1,
                    max_total_connections: 2,
                    process_budget_bytes: 8 * FLOW_CONTROL_GRANULE_BYTES,
                },
                2,
                Some(usize::MAX / 2 + 1),
                0,
            )
            .expect_err("aggregate datagram multiplication must be checked");
            assert_eq!(aggregate_overflow.kind(), io::ErrorKind::InvalidInput);
        }
        #[test]
        fn incoming_first_packet_reserve_is_checked_in_combined_endpoint_geometry() {
            let combined_overflow = endpoint_buffer_geometry(
                FlowControlConfig {
                    max_encrypted_frame_bytes: usize::MAX / 8,
                    max_total_connections: 2,
                    process_budget_bytes: usize::MAX,
                },
                2,
                None,
                2 * FLOW_CONTROL_GRANULE_BYTES,
            )
            .expect_err("first-packet reserve must participate in checked endpoint total");
            assert_eq!(combined_overflow.kind(), io::ErrorKind::InvalidInput);
            assert!(combined_overflow.to_string().contains("combined endpoint"));
        }
    }
}
/// QUIC dialer handle type.
#[cfg(feature = "quic")]
pub type QuicDialer = quic::Dialer;
/// Stub QUIC dialer type when QUIC support is not compiled in.
#[cfg(not(feature = "quic"))]
pub type QuicDialer = ();
/// QUIC connection handle type.
#[cfg(feature = "quic")]
pub type QuicConnection = quinn::Connection;
/// Stub QUIC connection handle type when QUIC support is not compiled in.
#[cfg(not(feature = "quic"))]
pub type QuicConnection = ();
/// Compute the stable certificate fingerprint used for transport channel binding.
#[must_use]
pub fn certificate_fingerprint(cert_der: &[u8]) -> crate::peer::TransportBinding {
    iroha_crypto::Hash::new(cert_der).into()
}
/// Extract the peer certificate fingerprint from an established TLS client session.
///
/// # Errors
///
/// Returns an error when the peer does not present a certificate or when the
/// certificate chain is empty.
#[cfg(feature = "p2p_tls")]
pub fn tls_peer_certificate_fingerprint<S>(
    tls: &tokio_rustls::client::TlsStream<S>,
) -> std::io::Result<crate::peer::TransportBinding> {
    let (_, session) = tls.get_ref();
    let certs = session.peer_certificates().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "tls peer did not present a certificate",
        )
    })?;
    let cert = certs.first().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "tls peer certificate chain is empty",
        )
    })?;
    Ok(certificate_fingerprint(cert.as_ref()))
}
/// Extract the peer certificate fingerprint from an established QUIC session.
///
/// # Errors
///
/// Returns an error when the peer does not present an identity, when the
/// identity is not encoded as a certificate chain, or when the chain is empty.
#[cfg(feature = "quic")]
pub fn quic_peer_certificate_fingerprint(
    connection: &quinn::Connection,
) -> std::io::Result<crate::peer::TransportBinding> {
    let identity = connection.peer_identity().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "quic peer did not present an identity",
        )
    })?;
    let certs = identity
        .downcast::<Vec<rustls::pki_types::CertificateDer<'static>>>()
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected quic peer identity type",
            )
        })?;
    let cert = certs.first().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "quic peer certificate chain is empty",
        )
    })?;
    Ok(certificate_fingerprint(cert.as_ref()))
}
#[cfg(feature = "p2p_tls")]
pub mod tls {
    //! TLS-over-TCP transport (feature-gated, optional).
    //!
    //! Wraps a TCP stream with TLS 1.3 using rustls. Self-signed certificates
    //! are accepted after TLS proves possession of their private key; peer
    //! identity is then enforced by the application handshake signature bound
    //! to the presented certificate fingerprint.
    use rustls::{ClientConfig, client::danger::ServerCertVerifier, pki_types::ServerName};
    use std::sync::Arc;
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio_rustls::{TlsConnector, client::TlsStream};
    /// Upgrade an already-connected TCP stream to TLS 1.3.
    pub async fn connect_tls<S>(host: &str, tcp: S) -> tokio::io::Result<TlsStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let verifier: Arc<dyn ServerCertVerifier> =
            Arc::new(super::CertificateKeyProofVerifier::unpinned());
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        let config = Arc::new(config);
        let connector = TlsConnector::from(config);
        let server_name = if let Ok(name) = ServerName::try_from(host) {
            name.to_owned()
        } else if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            ServerName::IpAddress(ip.into())
        } else {
            return Err(tokio::io::Error::new(
                tokio::io::ErrorKind::InvalidInput,
                "invalid SNI",
            ));
        };
        let tls = connector.connect(server_name, tcp).await?;
        Ok(tls)
    }
    /// Upgrade an already-connected TCP stream to TLS with end-entity certificate pinning.
    ///
    /// This is intended for `https://` proxy connections where operator-supplied pins can prevent
    /// MITM capture of proxy credentials.
    pub async fn connect_tls_pinned<S>(
        host: &str,
        tcp: S,
        expected_cert_der: Arc<[u8]>,
    ) -> tokio::io::Result<TlsStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let expected_fingerprint = super::certificate_fingerprint(expected_cert_der.as_ref());
        let verifier: Arc<dyn ServerCertVerifier> = Arc::new(
            super::CertificateKeyProofVerifier::pinned(expected_fingerprint),
        );
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = if let Ok(name) = ServerName::try_from(host) {
            name.to_owned()
        } else if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            ServerName::IpAddress(ip.into())
        } else {
            return Err(tokio::io::Error::new(
                tokio::io::ErrorKind::InvalidInput,
                "invalid SNI",
            ));
        };
        let tls = connector.connect(server_name, tcp).await?;
        Ok(tls)
    }
}
#[cfg(feature = "p2p_ws")]
pub mod ws {
    //! WebSocket fallback transport (client-side) over WSS to Torii `/p2p`.
    use futures::{Sink as _, Stream as _};
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio_tungstenite::{
        MaybeTlsStream, client_async_tls_with_config,
        tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
    };
    /// Maximum payload carried by one WebSocket transport message.
    ///
    /// P2P's encrypted stream framing remains continuous across these chunks;
    /// this bound prevents a maximal P2P frame from becoming one equally large
    /// WebSocket allocation before the inner frame cap can run.
    pub const WEBSOCKET_CHUNK_BYTES: usize = 64 * 1024;
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ReadState {
        Open,
        FlushingCloseReply,
        Eof,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ShutdownState {
        Open,
        FlushingForClose,
        Closing,
        Closed,
    }
    fn websocket_config() -> WebSocketConfig {
        WebSocketConfig::default()
            .read_buffer_size(WEBSOCKET_CHUNK_BYTES)
            .write_buffer_size(WEBSOCKET_CHUNK_BYTES)
            .max_write_buffer_size(WEBSOCKET_CHUNK_BYTES * 4)
            .max_message_size(Some(WEBSOCKET_CHUNK_BYTES))
            .max_frame_size(Some(WEBSOCKET_CHUNK_BYTES))
    }
    /// A duplex adaptor that implements `AsyncRead`/`AsyncWrite` over a WebSocket stream.
    /// Bytes written are segmented into bounded Binary messages. Reads concatenate
    /// those messages back into one byte stream, preserving application framing above.
    pub struct WsDuplex<S> {
        inner: tokio_tungstenite::WebSocketStream<S>,
        read_buf: bytes::Bytes, // remaining unread bytes from last Binary frame
        write_buf: Vec<u8>,
        read_state: ReadState,
        shutdown_state: ShutdownState,
    }
    impl<S> WsDuplex<S>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        fn new(inner: tokio_tungstenite::WebSocketStream<S>) -> Self {
            Self {
                inner,
                read_buf: bytes::Bytes::new(),
                write_buf: Vec::new(),
                read_state: ReadState::Open,
                shutdown_state: ShutdownState::Open,
            }
        }
        fn poll_send_buffered(
            &mut self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            if self.write_buf.is_empty() {
                return std::task::Poll::Ready(Ok(()));
            }
            let mut sink = std::pin::Pin::new(&mut self.inner);
            futures::ready!(
                sink.as_mut()
                    .poll_ready(cx)
                    .map_err(|e| std::io::Error::other(format!("ws poll_ready error: {e}")))
            )?;
            let data = std::mem::take(&mut self.write_buf);
            debug_assert!(data.len() <= WEBSOCKET_CHUNK_BYTES);
            sink.as_mut()
                .start_send(Message::Binary(data.into()))
                .map_err(|e| std::io::Error::other(format!("ws send error: {e}")))?;
            std::task::Poll::Ready(Ok(()))
        }
        fn poll_flush_buffered(
            &mut self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            futures::ready!(self.poll_send_buffered(cx))?;
            let mut sink = std::pin::Pin::new(&mut self.inner);
            futures::ready!(
                sink.as_mut()
                    .poll_flush(cx)
                    .map_err(|e| std::io::Error::other(format!("ws flush error: {e}")))
            )?;
            std::task::Poll::Ready(Ok(()))
        }
        fn mark_closed(&mut self) {
            self.read_buf = bytes::Bytes::new();
            self.write_buf.clear();
            self.read_state = ReadState::Eof;
            self.shutdown_state = ShutdownState::Closed;
        }
        fn begin_peer_close(&mut self) {
            // Tungstenite has queued the protocol-mandated close reply. No
            // buffered application payload may be emitted after that reply.
            self.write_buf.clear();
            self.read_state = ReadState::FlushingCloseReply;
            self.shutdown_state = ShutdownState::Closing;
        }
        fn poll_flush_close_reply(
            &mut self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            debug_assert_eq!(self.read_state, ReadState::FlushingCloseReply);
            let result = std::pin::Pin::new(&mut self.inner).poll_flush(cx);
            match result {
                std::task::Poll::Pending => std::task::Poll::Pending,
                std::task::Poll::Ready(
                    Ok(())
                    | Err(
                        tokio_tungstenite::tungstenite::Error::ConnectionClosed
                        | tokio_tungstenite::tungstenite::Error::AlreadyClosed,
                    ),
                ) => {
                    self.mark_closed();
                    std::task::Poll::Ready(Ok(()))
                }
                std::task::Poll::Ready(Err(error)) => {
                    self.mark_closed();
                    std::task::Poll::Ready(Err(std::io::Error::other(format!(
                        "ws close reply flush error: {error}"
                    ))))
                }
            }
        }
        fn reject_late_write() -> std::io::Error {
            std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "WebSocket transport is closing or closed",
            )
        }
    }
    /// Perform a websocket client handshake over an already-established stream.
    ///
    /// This is useful for applying custom TCP dial logic (proxies, socket options) while
    /// still speaking WebSocket/WSS at the HTTP layer.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the WebSocket client handshake over the supplied
    /// stream cannot be completed.
    pub async fn connect_with_stream<R, S>(
        request: R,
        stream: S,
    ) -> std::io::Result<WsDuplex<MaybeTlsStream<S>>>
    where
        R: IntoClientRequest + Unpin,
        S: 'static + AsyncRead + AsyncWrite + Send + Unpin,
        MaybeTlsStream<S>: Unpin,
    {
        let (ws_stream, _resp) =
            client_async_tls_with_config(request, stream, Some(websocket_config()), None)
                .await
                .map_err(|e| std::io::Error::other(format!("ws connect: {e}")))?;
        Ok(WsDuplex::new(ws_stream))
    }
    impl<S> AsyncRead for WsDuplex<S>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            // `AsyncRead` requires an empty destination to complete without
            // touching the transport. In particular, it must not consume a
            // complete WebSocket frame into the adaptor's private buffer.
            if buf.remaining() == 0 {
                return std::task::Poll::Ready(Ok(()));
            }
            match self.read_state {
                ReadState::Eof => return std::task::Poll::Ready(Ok(())),
                ReadState::FlushingCloseReply => {
                    return self.poll_flush_close_reply(cx);
                }
                ReadState::Open => {}
            }
            if !self.read_buf.is_empty() {
                let n = std::cmp::min(self.read_buf.len(), buf.remaining());
                buf.put_slice(&self.read_buf.split_to(n));
                return std::task::Poll::Ready(Ok(()));
            }
            // Pull next Binary frame
            match futures::ready!(std::pin::Pin::new(&mut self.inner).poll_next(cx)) {
                Some(Ok(Message::Binary(b))) if b.is_empty() => {
                    // An empty WebSocket data message carries no stream bytes;
                    // it is not the end of the byte stream. Yield after one
                    // ignored message so a hostile peer cannot monopolize a
                    // single poll with an unbounded run of empty messages.
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
                Some(Ok(Message::Binary(b))) => {
                    self.read_buf = b;
                    let n = std::cmp::min(self.read_buf.len(), buf.remaining());
                    buf.put_slice(&self.read_buf.split_to(n));
                    std::task::Poll::Ready(Ok(()))
                }
                Some(Ok(
                    Message::Text(_) | Message::Ping(_) | Message::Pong(_) | Message::Frame(_),
                )) => {
                    // Ignore control/text frames and read next
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
                Some(Ok(Message::Close(_))) => {
                    self.begin_peer_close();
                    self.poll_flush_close_reply(cx)
                }
                None
                | Some(Err(
                    tokio_tungstenite::tungstenite::Error::ConnectionClosed
                    | tokio_tungstenite::tungstenite::Error::AlreadyClosed,
                )) => {
                    self.mark_closed();
                    std::task::Poll::Ready(Ok(()))
                }
                Some(Err(error)) => {
                    self.mark_closed();
                    std::task::Poll::Ready(Err(std::io::Error::other(format!(
                        "ws read error: {error}"
                    ))))
                }
            }
        }
    }
    impl<S> AsyncWrite for WsDuplex<S>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            data: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            if self.shutdown_state != ShutdownState::Open {
                return std::task::Poll::Ready(Err(Self::reject_late_write()));
            }
            if data.is_empty() {
                return std::task::Poll::Ready(Ok(0));
            }
            if self.write_buf.len() == WEBSOCKET_CHUNK_BYTES {
                futures::ready!(self.poll_send_buffered(cx))?;
            }
            let accepted = data
                .len()
                .min(WEBSOCKET_CHUNK_BYTES.saturating_sub(self.write_buf.len()));
            self.write_buf.extend_from_slice(&data[..accepted]);
            std::task::Poll::Ready(Ok(accepted))
        }
        fn poll_flush(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            match self.shutdown_state {
                ShutdownState::Open | ShutdownState::FlushingForClose => {
                    self.poll_flush_buffered(cx)
                }
                ShutdownState::Closing => {
                    let result = std::pin::Pin::new(&mut self.inner).poll_flush(cx);
                    match result {
                        std::task::Poll::Pending => std::task::Poll::Pending,
                        std::task::Poll::Ready(
                            Ok(())
                            | Err(
                                tokio_tungstenite::tungstenite::Error::ConnectionClosed
                                | tokio_tungstenite::tungstenite::Error::AlreadyClosed,
                            ),
                        ) => std::task::Poll::Ready(Ok(())),
                        std::task::Poll::Ready(Err(error)) => std::task::Poll::Ready(Err(
                            std::io::Error::other(format!("ws close flush error: {error}")),
                        )),
                    }
                }
                ShutdownState::Closed => std::task::Poll::Ready(Ok(())),
            }
        }
        fn poll_shutdown(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            loop {
                match self.shutdown_state {
                    ShutdownState::Open => {
                        // Record shutdown before the first operation that can
                        // return `Pending`; dropping that future must never
                        // reopen the write side.
                        self.shutdown_state = ShutdownState::FlushingForClose;
                    }
                    ShutdownState::FlushingForClose => match self.poll_flush_buffered(cx) {
                        std::task::Poll::Pending => return std::task::Poll::Pending,
                        std::task::Poll::Ready(Ok(())) => {
                            self.shutdown_state = ShutdownState::Closing;
                        }
                        std::task::Poll::Ready(Err(error)) => {
                            self.mark_closed();
                            return std::task::Poll::Ready(Err(error));
                        }
                    },
                    ShutdownState::Closing => {
                        let result = std::pin::Pin::new(&mut self.inner).poll_close(cx);
                        return match result {
                            std::task::Poll::Pending => std::task::Poll::Pending,
                            std::task::Poll::Ready(Ok(())) => {
                                self.write_buf.clear();
                                self.shutdown_state = ShutdownState::Closed;
                                std::task::Poll::Ready(Ok(()))
                            }
                            std::task::Poll::Ready(Err(error)) => {
                                self.mark_closed();
                                std::task::Poll::Ready(Err(std::io::Error::other(format!(
                                    "ws close error: {error}"
                                ))))
                            }
                        };
                    }
                    ShutdownState::Closed => return std::task::Poll::Ready(Ok(())),
                }
            }
        }
    }
    /// Connect a WSS endpoint `wss://host:port/p2p` and return a duplex stream.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when `endpoint` cannot form a WebSocket
    /// request, or an I/O error when the WSS connection or handshake fails.
    pub async fn connect_wss(
        endpoint: &str,
    ) -> std::io::Result<WsDuplex<MaybeTlsStream<tokio::net::TcpStream>>> {
        let url = format!("wss://{endpoint}/p2p");
        let req = url.into_client_request().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad url: {e}"))
        })?;
        let (ws_stream, _resp) =
            tokio_tungstenite::connect_async_with_config(req, Some(websocket_config()), false)
                .await
                .map_err(|e| std::io::Error::other(format!("wss connect: {e}")))?;
        Ok(WsDuplex::new(ws_stream))
    }
    /// Connect a WS endpoint `ws://host:port/p2p` and return a duplex stream.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when `endpoint` cannot form a WebSocket
    /// request, or an I/O error when the WS connection or handshake fails.
    pub async fn connect_ws(
        endpoint: &str,
    ) -> std::io::Result<WsDuplex<MaybeTlsStream<tokio::net::TcpStream>>> {
        let url = format!("ws://{endpoint}/p2p");
        let req = url.into_client_request().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad url: {e}"))
        })?;
        let (ws_stream, _resp) =
            tokio_tungstenite::connect_async_with_config(req, Some(websocket_config()), false)
                .await
                .map_err(|e| std::io::Error::other(format!("ws connect: {e}")))?;
        Ok(WsDuplex::new(ws_stream))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use futures::{SinkExt as _, StreamExt as _};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        use tokio_tungstenite::{WebSocketStream, tungstenite::protocol::Role};
        async fn assert_chunked_stream_roundtrip(byte_len: usize) {
            let (client_io, server_io) = tokio::io::duplex(WEBSOCKET_CHUNK_BYTES * 2);
            let (client_ws, mut server_ws) = tokio::join!(
                WebSocketStream::from_raw_socket(client_io, Role::Client, Some(websocket_config()),),
                WebSocketStream::from_raw_socket(server_io, Role::Server, Some(websocket_config()),),
            );
            let mut client = WsDuplex::new(client_ws);
            let expected = (0..byte_len)
                .map(|index| u8::try_from(index % 251).expect("bounded fixture byte"))
                .collect::<Vec<_>>();
            let send = async {
                client
                    .write_all(&expected)
                    .await
                    .expect("write complete P2P byte stream");
                client.flush().await.expect("flush P2P byte stream");
            };
            let receive = async {
                let mut received = Vec::with_capacity(byte_len);
                let mut chunks = 0usize;
                while received.len() < byte_len {
                    match server_ws.next().await.expect("next WebSocket message") {
                        Ok(Message::Binary(chunk)) => {
                            assert!(!chunk.is_empty());
                            assert!(chunk.len() <= WEBSOCKET_CHUNK_BYTES);
                            received.extend_from_slice(&chunk);
                            chunks = chunks.checked_add(1).expect("small chunk count");
                        }
                        other => panic!("expected bounded binary chunk, got {other:?}"),
                    }
                }
                (received, chunks)
            };
            let ((), (received, chunks)) = tokio::join!(send, receive);
            assert_eq!(received, expected);
            assert_eq!(chunks, byte_len.div_ceil(WEBSOCKET_CHUNK_BYTES));
        }
        #[tokio::test(flavor = "current_thread")]
        async fn websocket_duplex_chunks_boundaries_and_default_maximum_p2p_frame() {
            for byte_len in [
                WEBSOCKET_CHUNK_BYTES - 1,
                WEBSOCKET_CHUNK_BYTES,
                WEBSOCKET_CHUNK_BYTES + 1,
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get()
                    + crate::P2P_FRAME_LENGTH_PREFIX_BYTES,
            ] {
                assert_chunked_stream_roundtrip(byte_len).await;
            }
        }
        #[tokio::test(flavor = "current_thread")]
        async fn websocket_config_rejects_one_oversized_transport_message() {
            let (client_io, server_io) = tokio::io::duplex(WEBSOCKET_CHUNK_BYTES * 2);
            let (mut client_ws, mut server_ws) = tokio::join!(
                WebSocketStream::from_raw_socket(client_io, Role::Client, Some(websocket_config()),),
                WebSocketStream::from_raw_socket(server_io, Role::Server, Some(websocket_config()),),
            );
            let send = async {
                futures::SinkExt::send(
                    &mut client_ws,
                    Message::Binary(vec![0xA5; WEBSOCKET_CHUNK_BYTES + 1].into()),
                )
                .await
            };
            let receive = async { server_ws.next().await.expect("oversized message result") };
            let (send_result, receive_result) = tokio::join!(send, receive);
            send_result.expect("peer can emit adversarial oversized transport message");
            assert!(
                receive_result.is_err(),
                "the receiver must reject a WebSocket message one byte above the chunk cap"
            );
        }
        struct ReadPollGuard<S> {
            inner: S,
            reject_reads: Arc<AtomicBool>,
        }
        impl<S> ReadPollGuard<S> {
            fn new(inner: S, reject_reads: Arc<AtomicBool>) -> Self {
                Self {
                    inner,
                    reject_reads,
                }
            }
        }
        impl<S: AsyncRead + Unpin> AsyncRead for ReadPollGuard<S> {
            fn poll_read(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                assert!(
                    !self.reject_reads.load(Ordering::SeqCst),
                    "WebSocket adaptor polled its transport after reads were forbidden"
                );
                std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
            }
        }
        impl<S: AsyncWrite + Unpin> AsyncWrite for ReadPollGuard<S> {
            fn poll_write(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                data: &[u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                std::pin::Pin::new(&mut self.inner).poll_write(cx, data)
            }
            fn poll_flush(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::pin::Pin::new(&mut self.inner).poll_flush(cx)
            }
            fn poll_shutdown(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
            }
        }
        #[tokio::test(flavor = "current_thread")]
        async fn websocket_duplex_zero_capacity_read_does_not_poll_or_consume_frame() {
            let (client_io, server_io) = tokio::io::duplex(WEBSOCKET_CHUNK_BYTES * 2);
            let reject_reads = Arc::new(AtomicBool::new(false));
            let client_io = ReadPollGuard::new(client_io, Arc::clone(&reject_reads));
            let (client_ws, mut server_ws) = tokio::join!(
                WebSocketStream::from_raw_socket(client_io, Role::Client, Some(websocket_config()),),
                WebSocketStream::from_raw_socket(server_io, Role::Server, Some(websocket_config()),),
            );
            let mut client = WsDuplex::new(client_ws);
            let expected = [0xC3, 0x7E, 0x41, 0x19];
            server_ws
                .send(Message::Binary(expected.to_vec().into()))
                .await
                .expect("send frame before zero-capacity read");
            reject_reads.store(true, Ordering::SeqCst);
            let mut empty = [];
            let mut empty_buf = tokio::io::ReadBuf::new(&mut empty);
            futures::future::poll_fn(|cx| {
                std::pin::Pin::new(&mut client).poll_read(cx, &mut empty_buf)
            })
            .await
            .expect("zero-capacity read succeeds immediately");
            assert!(empty_buf.filled().is_empty());
            reject_reads.store(false, Ordering::SeqCst);
            let mut received = [0_u8; 4];
            client
                .read_exact(&mut received)
                .await
                .expect("frame remains available after zero-capacity read");
            assert_eq!(received, expected);
        }
        #[tokio::test(flavor = "current_thread")]
        async fn websocket_duplex_ignores_empty_binary_without_reporting_stream_eof() {
            let (client_io, server_io) = tokio::io::duplex(WEBSOCKET_CHUNK_BYTES * 2);
            let (client_ws, mut server_ws) = tokio::join!(
                WebSocketStream::from_raw_socket(client_io, Role::Client, Some(websocket_config()),),
                WebSocketStream::from_raw_socket(server_io, Role::Server, Some(websocket_config()),),
            );
            let mut client = WsDuplex::new(client_ws);
            let expected = [0xA5, 0x5A, 0x11, 0x22];
            server_ws
                .send(Message::Binary(Vec::new().into()))
                .await
                .expect("send legal empty WebSocket data message");
            server_ws
                .send(Message::Binary(expected.to_vec().into()))
                .await
                .expect("send following non-empty WebSocket data message");
            let mut received = [0_u8; 4];
            client
                .read_exact(&mut received)
                .await
                .expect("empty Binary message must not terminate the byte stream");
            assert_eq!(received, expected);
        }
        #[tokio::test(flavor = "current_thread")]
        async fn websocket_duplex_flushes_close_reply_before_sticky_eof() {
            let (client_io, server_io) = tokio::io::duplex(WEBSOCKET_CHUNK_BYTES * 2);
            let reject_reads = Arc::new(AtomicBool::new(false));
            let client_io = ReadPollGuard::new(client_io, Arc::clone(&reject_reads));
            let (client_ws, mut server_ws) = tokio::join!(
                WebSocketStream::from_raw_socket(client_io, Role::Client, Some(websocket_config()),),
                WebSocketStream::from_raw_socket(server_io, Role::Server, Some(websocket_config()),),
            );
            let mut client = WsDuplex::new(client_ws);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let consume_close = async {
                    let mut byte = [0_u8; 1];
                    assert_eq!(
                        client.read(&mut byte).await.expect("read peer close"),
                        0,
                        "peer Close must become byte-stream EOF"
                    );
                    reject_reads.store(true, Ordering::SeqCst);
                    assert_eq!(
                        client.read(&mut byte).await.expect("read sticky EOF"),
                        0,
                        "EOF must remain stable without polling the transport"
                    );
                };
                let exchange_close = async {
                    server_ws
                        .send(Message::Close(None))
                        .await
                        .expect("send peer Close");
                    match server_ws
                        .next()
                        .await
                        .expect("client close acknowledgement")
                    {
                        Ok(Message::Close(_)) => {}
                        Ok(other) => {
                            panic!("expected WebSocket Close acknowledgement, got {other:?}")
                        }
                        Err(error) => {
                            panic!("failed to observe WebSocket Close acknowledgement: {error}")
                        }
                    }
                };
                tokio::join!(consume_close, exchange_close);
            })
            .await
            .expect("close acknowledgement and sticky EOF must not stall");
        }
        struct PendingFlushOnce<S> {
            inner: S,
            observed: Arc<AtomicBool>,
            pending: bool,
        }
        impl<S> PendingFlushOnce<S> {
            fn new(inner: S, observed: Arc<AtomicBool>) -> Self {
                Self {
                    inner,
                    observed,
                    pending: true,
                }
            }
        }
        impl<S: AsyncRead + Unpin> AsyncRead for PendingFlushOnce<S> {
            fn poll_read(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
            }
        }
        impl<S: AsyncWrite + Unpin> AsyncWrite for PendingFlushOnce<S> {
            fn poll_write(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                data: &[u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                std::pin::Pin::new(&mut self.inner).poll_write(cx, data)
            }
            fn poll_flush(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                if self.pending {
                    self.pending = false;
                    self.observed.store(true, Ordering::SeqCst);
                    cx.waker().wake_by_ref();
                    return std::task::Poll::Pending;
                }
                std::pin::Pin::new(&mut self.inner).poll_flush(cx)
            }
            fn poll_shutdown(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
            }
        }
        #[tokio::test(flavor = "current_thread")]
        async fn websocket_duplex_cancelled_shutdown_rejects_late_writes_and_resumes() {
            let (client_io, server_io) = tokio::io::duplex(WEBSOCKET_CHUNK_BYTES * 2);
            let pending_observed = Arc::new(AtomicBool::new(false));
            let client_io = PendingFlushOnce::new(client_io, Arc::clone(&pending_observed));
            let (client_ws, mut server_ws) = tokio::join!(
                WebSocketStream::from_raw_socket(client_io, Role::Client, Some(websocket_config()),),
                WebSocketStream::from_raw_socket(server_io, Role::Server, Some(websocket_config()),),
            );
            let mut client = WsDuplex::new(client_ws);
            let mut shutdown = Box::pin(client.shutdown());
            futures::future::poll_fn(
                |cx| match std::future::Future::poll(shutdown.as_mut(), cx) {
                    std::task::Poll::Pending => std::task::Poll::Ready(()),
                    std::task::Poll::Ready(result) => {
                        panic!("fixture must suspend the first shutdown poll, got {result:?}")
                    }
                },
            )
            .await;
            drop(shutdown);
            assert!(
                pending_observed.load(Ordering::SeqCst),
                "fixture must suspend shutdown while flushing before Close"
            );
            let error = client
                .write_all(b"must not escape after shutdown cancellation")
                .await
                .expect_err("a cancelled shutdown must leave the write side closed");
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let shutdown = client.shutdown();
                let observe_close = async {
                    loop {
                        match server_ws.next().await.expect("client close message") {
                            Ok(Message::Close(_)) => break,
                            Ok(other) => panic!("expected WebSocket Close, got {other:?}"),
                            Err(error) => panic!("failed to observe WebSocket Close: {error}"),
                        }
                    }
                };
                let (shutdown_result, ()) = tokio::join!(shutdown, observe_close);
                shutdown_result.expect("stateful close must survive a Pending flush");
            })
            .await
            .expect("WebSocket shutdown must not stall");
        }
    }
}
/// Transport connector abstraction (scaffolding).
///
/// The default implementation uses TCP; alternative transports should
/// match semantics (ordered, reliable) and integrate with the same
/// message framing.
#[allow(clippy::missing_errors_doc)]
pub trait TransportConnector {
    /// Underlying stream type used by the transport.
    type Stream;
    /// Dial a remote endpoint.
    fn dial(endpoint: &str) -> tokio::io::Result<Self::Stream>;
}
use crate::sampler::LogSampler;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_primitives::addr::SocketAddr;
use socket2::{SockRef, TcpKeepalive};
use std::sync::{Mutex, OnceLock};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt, Result},
    net::TcpStream,
};
/// Outbound proxy configuration for TCP-based dials (HTTP CONNECT / SOCKS5).
#[derive(Debug, Clone, Default)]
pub struct ProxyPolicy {
    proxy: Option<Proxy>,
    no_proxy: Vec<String>,
}
impl ProxyPolicy {
    /// Disable proxying entirely.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            proxy: None,
            no_proxy: Vec::new(),
        }
    }
    /// Build a proxy policy from config values.
    ///
    /// # Errors
    /// Returns an error if `proxy_url` is present but cannot be parsed.
    pub fn from_config(proxy_url: Option<String>, no_proxy: Vec<String>) -> io::Result<Self> {
        let proxy = proxy_url
            .map(|raw| parse_proxy_value(&raw))
            .transpose()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let no_proxy = normalize_no_proxy(no_proxy);
        Ok(Self { proxy, no_proxy })
    }
    fn should_bypass_proxy(&self, target_host: &str) -> bool {
        self.no_proxy.iter().any(|entry| {
            if entry.is_empty() {
                return false;
            }
            target_host.ends_with(entry)
        })
    }
    fn pick_proxy_for_target(&self, target: &SocketAddr) -> Option<&Proxy> {
        let proxy = self.proxy.as_ref()?;
        // Resolve target host string for NO_PROXY checks
        match target {
            SocketAddr::Host(h) => {
                let host = h.host.as_ref();
                if self.should_bypass_proxy(host) {
                    None
                } else {
                    Some(proxy)
                }
            }
            SocketAddr::Ipv4(v4) => {
                let host = format!("{}.{}.{}.{}", v4.ip[0], v4.ip[1], v4.ip[2], v4.ip[3]);
                if self.should_bypass_proxy(&host) {
                    None
                } else {
                    Some(proxy)
                }
            }
            SocketAddr::Ipv6(v6) => {
                // Represent as canonical without brackets.
                let host = v6.ip.to_string();
                if self.should_bypass_proxy(&host) {
                    None
                } else {
                    Some(proxy)
                }
            }
        }
    }
}
fn normalize_no_proxy(mut list: Vec<String>) -> Vec<String> {
    for entry in &mut list {
        // Keep ASCII; no unicode normalization needed.
        *entry = entry.trim().to_string();
    }
    list.retain(|s| !s.is_empty());
    list
}
/// TCP socket options applied to outbound dials.
#[derive(Debug, Clone)]
pub struct TcpConnectOptions {
    /// Proxy policy for this dial.
    pub proxy: ProxyPolicy,
    /// Whether to verify TLS certificates when connecting to an `https://` proxy.
    ///
    /// This does not affect P2P TLS-over-TCP (peer identity is authenticated at the application layer).
    pub proxy_tls_verify: bool,
    /// Optional DER-encoded (base64 decoded) end-entity certificate to pin when connecting to an `https://` proxy.
    ///
    /// Used only when `proxy_tls_verify=true` and the proxy URL uses the `https://` scheme.
    pub proxy_tls_pinned_cert_der: Option<std::sync::Arc<[u8]>>,
    /// Whether to enable `TCP_NODELAY` for reduced latency.
    pub tcp_nodelay: bool,
    /// Optional keepalive idle time. When `None`, keepalive is disabled.
    pub tcp_keepalive: Option<std::time::Duration>,
}
impl Default for TcpConnectOptions {
    fn default() -> Self {
        Self {
            proxy: ProxyPolicy::disabled(),
            proxy_tls_verify: true,
            proxy_tls_pinned_cert_der: None,
            tcp_nodelay: true,
            tcp_keepalive: None,
        }
    }
}
/// TCP-like outbound stream returned by [`connect`].
///
/// Most dials return a plain [`TcpStream`]. When tunnelling through an `https://`
/// proxy, the connection to the proxy is wrapped in TLS.
pub enum TcpConnectStream {
    /// Plain TCP stream (direct or proxied).
    Plain(TcpStream),
    /// TLS-wrapped stream to the proxy (`https://` proxies only).
    #[cfg(feature = "p2p_tls")]
    Tls(tokio_rustls::client::TlsStream<TcpStream>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProxyKind {
    HttpConnect,
    HttpConnectTls,
    Socks5,
}
#[derive(Debug, Clone)]
struct Proxy {
    kind: ProxyKind,
    host: String,
    port: u16,
    auth: Option<(String, String)>,
}
fn parse_proxy_value(raw: &str) -> std::result::Result<Proxy, String> {
    let mut s = raw;
    let mut kind = ProxyKind::HttpConnect;
    if let Some(rest) = s.strip_prefix("http://") {
        s = rest;
    } else if let Some(rest) = s.strip_prefix("https://") {
        s = rest;
        kind = ProxyKind::HttpConnectTls;
    } else if let Some(rest) = s.strip_prefix("socks5://") {
        s = rest;
        kind = ProxyKind::Socks5;
    } else if let Some(rest) = s.strip_prefix("socks5h://") {
        // `socks5h` indicates remote DNS resolution. When the target is a hostname,
        // we already forward it as a domain name, so this behaves the same as `socks5`.
        s = rest;
        kind = ProxyKind::Socks5;
    }
    // Strip credentials if present
    let mut auth: Option<(String, String)> = None;
    if let Some(at) = s.rfind('@') {
        let (creds, host_part) = s.split_at(at);
        s = host_part.get(1..).unwrap_or_default(); // skip '@'
        if !creds.is_empty() {
            let mut parts = creds.splitn(2, ':');
            let user = parts.next().unwrap_or("").to_string();
            let pass = parts.next().unwrap_or("").to_string();
            auth = Some((user, pass));
        }
    }
    let (host, port_str) = if let Some(rest) = s.strip_prefix('[') {
        let (host, rest) = rest
            .split_once(']')
            .ok_or_else(|| "proxy URL has unterminated IPv6 host".to_string())?;
        let port_str = rest
            .strip_prefix(':')
            .ok_or_else(|| "proxy URL missing port".to_string())?;
        (host, port_str)
    } else {
        // If the host contains multiple ':' characters, treat it as an IPv6 literal missing brackets.
        // Require bracketed form to avoid ambiguity with the port delimiter.
        if s.matches(':').count() > 1 {
            return Err("proxy URL has ambiguous IPv6 host; use [addr]:port".to_string());
        }
        s.rsplit_once(':')
            .ok_or_else(|| "proxy URL missing port".to_string())?
    };
    if host.is_empty() {
        return Err("proxy URL missing host".to_string());
    }
    let port: u16 = port_str
        .parse()
        .map_err(|_| "proxy URL has invalid port".to_string())?;
    Ok(Proxy {
        kind,
        host: host.to_string(),
        port,
        auth,
    })
}
// ---- TCP socket option helpers ----
fn build_connect_request(target: &str, proxy: &Proxy) -> String {
    let mut headers =
        format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nConnection: keep-alive\r\n");
    if let Some((user, pass)) = &proxy.auth {
        let creds = format!("{user}:{pass}");
        let auth = BASE64_STANDARD.encode(creds.as_bytes());
        headers.push_str("Proxy-Authorization: Basic ");
        headers.push_str(&auth);
        headers.push_str("\r\n");
    }
    headers.push_str("\r\n");
    headers
}
async fn socks5_negotiate_method<S>(stream: &mut S, proxy: &Proxy) -> Result<u8>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let mut methods: Vec<u8> = vec![0x00];
    if proxy.auth.is_some() {
        methods.push(0x02);
    }
    let methods_len = u8::try_from(methods.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "SOCKS5 method list too long"))?;
    let mut greeting = Vec::with_capacity(2 + methods.len());
    greeting.push(0x05);
    greeting.push(methods_len);
    greeting.extend_from_slice(&methods);
    stream.write_all(&greeting).await?;
    stream.flush().await?;
    let mut choice = [0u8; 2];
    stream.read_exact(&mut choice).await?;
    if choice[0] != 0x05 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SOCKS5 bad version",
        ));
    }
    match choice[1] {
        0x00 | 0x02 => Ok(choice[1]),
        0xFF => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SOCKS5 no acceptable auth methods",
        )),
        m => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("SOCKS5 unsupported auth method {m}"),
        )),
    }
}
async fn socks5_auth_user_pass<S>(stream: &mut S, user: &str, pass: &str) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // RFC 1929: username/password authentication.
    let user_len = u8::try_from(user.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "SOCKS5 username too long"))?;
    let pass_len = u8::try_from(pass.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "SOCKS5 password too long"))?;
    let mut auth_req = Vec::with_capacity(3 + user.len() + pass.len());
    auth_req.push(0x01);
    auth_req.push(user_len);
    auth_req.extend_from_slice(user.as_bytes());
    auth_req.push(pass_len);
    auth_req.extend_from_slice(pass.as_bytes());
    stream.write_all(&auth_req).await?;
    stream.flush().await?;
    let mut auth_resp = [0u8; 2];
    stream.read_exact(&mut auth_resp).await?;
    if auth_resp[0] != 0x01 || auth_resp[1] != 0x00 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SOCKS5 authentication failed",
        ));
    }
    Ok(())
}
fn socks5_build_connect_request(target: &SocketAddr) -> Result<Vec<u8>> {
    let mut req = Vec::with_capacity(32);
    req.push(0x05); // version
    req.push(0x01); // CMD=CONNECT
    req.push(0x00); // RSV
    match target {
        SocketAddr::Ipv4(v4) => {
            req.push(0x01); // ATYP=IPv4
            let ip: std::net::Ipv4Addr = v4.ip.into();
            req.extend_from_slice(&ip.octets());
            req.extend_from_slice(&v4.port.to_be_bytes());
        }
        SocketAddr::Ipv6(v6) => {
            req.push(0x04); // ATYP=IPv6
            let ip: std::net::Ipv6Addr = v6.ip.into();
            req.extend_from_slice(&ip.octets());
            req.extend_from_slice(&v6.port.to_be_bytes());
        }
        SocketAddr::Host(host) => {
            let name = host.host.as_ref();
            let name_len = u8::try_from(name.len()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "SOCKS5 target hostname too long",
                )
            })?;
            req.push(0x03); // ATYP=DOMAIN
            req.push(name_len);
            req.extend_from_slice(name.as_bytes());
            req.extend_from_slice(&host.port.to_be_bytes());
        }
    }
    Ok(req)
}
async fn socks5_read_connect_reply<S>(stream: &mut S) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // Reply: VER, REP, RSV, ATYP, BND.ADDR, BND.PORT.
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SOCKS5 bad reply version",
        ));
    }
    if head[1] != 0x00 {
        return Err(io::Error::other(format!(
            "SOCKS5 connect failed (rep={})",
            head[1]
        )));
    }
    match head[3] {
        0x01 => {
            let mut bnd = [0u8; 4];
            stream.read_exact(&mut bnd).await?;
        }
        0x04 => {
            let mut bnd = [0u8; 16];
            stream.read_exact(&mut bnd).await?;
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            let mut bnd = vec![0u8; len[0] as usize];
            stream.read_exact(&mut bnd).await?;
        }
        atyp => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("SOCKS5 bad reply ATYP {atyp}"),
            ));
        }
    }
    let mut port = [0u8; 2];
    stream.read_exact(&mut port).await?;
    Ok(())
}
async fn socks5_connect<S>(stream: &mut S, proxy: &Proxy, target: &SocketAddr) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // RFC 1928: SOCKS5 version/method negotiation.
    let method = socks5_negotiate_method(stream, proxy).await?;
    if method == 0x02 {
        let (user, pass) = proxy.auth.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "SOCKS5 proxy requires username/password",
            )
        })?;
        socks5_auth_user_pass(stream, user, pass).await?;
    }
    let req = socks5_build_connect_request(target)?;
    stream.write_all(&req).await?;
    stream.flush().await?;
    socks5_read_connect_reply(stream).await
}
async fn http_connect_tunnel<S>(
    stream: &mut S,
    proxy: &Proxy,
    target: &SocketAddr,
    proxy_endpoint: &str,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let target = target.to_string();
    let req = build_connect_request(&target, proxy);
    stream.write_all(req.as_bytes()).await?;
    // Read until end of headers (\r\n\r\n) or small cap
    let mut buf = vec![0u8; 1024];
    let mut acc = Vec::with_capacity(1024);
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        acc.extend_from_slice(&buf[..n]);
        if acc.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if acc.len() > 8192 {
            break;
        }
    }
    // Crude status check
    let text = String::from_utf8_lossy(&acc);
    if !(text.starts_with("HTTP/1.1 200") || text.starts_with("HTTP/1.0 200")) {
        static PROXY_CONNECT_SAMPLER: OnceLock<Mutex<LogSampler>> = OnceLock::new();
        let sampler = PROXY_CONNECT_SAMPLER.get_or_init(|| Mutex::new(LogSampler::new()));
        if let Ok(mut s) = sampler.lock() {
            if let Some(supp) = s.should_log(tokio::time::Duration::from_millis(500)) {
                iroha_logger::warn!(status=%text.lines().next().unwrap_or("?"), proxy=%proxy_endpoint, target=%target, suppressed=supp, "HTTP CONNECT to proxy failed");
            }
        }
        return Err(io::Error::other("proxy CONNECT failed"));
    }
    Ok(())
}
/// Connect to a peer using the default transport (TCP).
///
/// When the `quic` feature is enabled, this remains a placeholder and
/// will be upgraded to a proper QUIC transport in a future change.
///
/// # Errors
///
/// Returns an `io::Error` if TCP connect fails, proxy handshake fails, or I/O operations error.
pub async fn connect(addr: &SocketAddr, opts: &TcpConnectOptions) -> Result<TcpConnectStream> {
    // If a proxy is configured and the target is not in NO_PROXY, tunnel via HTTP CONNECT.
    if let Some(proxy) = opts.proxy.pick_proxy_for_target(addr) {
        let proxy_endpoint = if proxy.host.contains(':') {
            format!("[{}]:{}", proxy.host, proxy.port)
        } else {
            format!("{}:{}", proxy.host, proxy.port)
        };
        let mut stream = match TcpStream::connect(proxy_endpoint.as_str()).await {
            Ok(s) => s,
            Err(e) => {
                static PROXY_CONNECT_SAMPLER: OnceLock<Mutex<LogSampler>> = OnceLock::new();
                let sampler = PROXY_CONNECT_SAMPLER.get_or_init(|| Mutex::new(LogSampler::new()));
                if let Ok(mut s) = sampler.lock() {
                    if let Some(supp) = s.should_log(tokio::time::Duration::from_millis(500)) {
                        iroha_logger::warn!(%e, proxy=%proxy_endpoint, suppressed=supp, "Failed to connect to proxy");
                    }
                }
                return Err(e);
            }
        };
        apply_tcp_socket_options(&stream, opts.tcp_nodelay, opts.tcp_keepalive);
        match proxy.kind {
            ProxyKind::HttpConnect => {
                http_connect_tunnel(&mut stream, proxy, addr, &proxy_endpoint).await?;
            }
            ProxyKind::HttpConnectTls => {
                #[cfg(feature = "p2p_tls")]
                {
                    let mut tls = if opts.proxy_tls_verify {
                        let pinned = opts.proxy_tls_pinned_cert_der.clone().ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "https proxy verification enabled but no pin configured; set network.p2p_proxy_tls_pinned_cert_der_base64 or disable p2p_proxy_tls_verify",
                            )
                        })?;
                        crate::transport::tls::connect_tls_pinned(&proxy.host, stream, pinned)
                            .await?
                    } else {
                        crate::transport::tls::connect_tls(&proxy.host, stream).await?
                    };
                    http_connect_tunnel(&mut tls, proxy, addr, &proxy_endpoint).await?;
                    return Ok(TcpConnectStream::Tls(tls));
                }
                #[cfg(not(feature = "p2p_tls"))]
                {
                    let _ = proxy_endpoint;
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "https proxy requires a build with the `iroha_p2p/p2p_tls` feature",
                    ));
                }
            }
            ProxyKind::Socks5 => {
                socks5_connect(&mut stream, proxy, addr).await?;
            }
        }
        Ok(TcpConnectStream::Plain(stream))
    } else {
        match TcpStream::connect(addr.to_string()).await {
            Ok(stream) => {
                apply_tcp_socket_options(&stream, opts.tcp_nodelay, opts.tcp_keepalive);
                Ok(TcpConnectStream::Plain(stream))
            }
            Err(e) => {
                static DIRECT_CONNECT_SAMPLER: OnceLock<Mutex<LogSampler>> = OnceLock::new();
                let sampler = DIRECT_CONNECT_SAMPLER.get_or_init(|| Mutex::new(LogSampler::new()));
                if let Ok(mut s) = sampler.lock() {
                    if let Some(supp) = s.should_log(tokio::time::Duration::from_millis(500)) {
                        iroha_logger::warn!(%e, target=%addr.to_string(), suppressed=supp, "TCP connect failed");
                    }
                }
                Err(e)
            }
        }
    }
}
pub(crate) fn apply_tcp_socket_options(
    stream: &TcpStream,
    tcp_nodelay: bool,
    tcp_keepalive: Option<std::time::Duration>,
) {
    let sock_ref = SockRef::from(stream);
    apply_tcp_socket_options_sockref(&sock_ref, tcp_nodelay, tcp_keepalive);
}
fn apply_tcp_socket_options_sockref(
    sock_ref: &SockRef<'_>,
    tcp_nodelay: bool,
    tcp_keepalive: Option<std::time::Duration>,
) {
    let _ = sock_ref.set_nodelay(tcp_nodelay);
    if let Some(idle) = tcp_keepalive {
        // Best-effort: keepalive knobs vary across OSes. Socket2 provides a safe wrapper.
        let keepalive = TcpKeepalive::new().with_time(idle);
        let _ = sock_ref.set_tcp_keepalive(&keepalive);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(flavor = "current_thread")]
    async fn connect_failure_sampling_limits_logs() {
        let mut sampler = crate::sampler::LogSampler::new();
        let mut logged = 0u32;
        for _ in 0..1000 {
            if sampler
                .should_log(tokio::time::Duration::from_millis(500))
                .is_some()
            {
                logged += 1;
            }
        }
        assert!(logged <= 1, "sampler should limit logs within the window");
        // After the period elapses, the sampler should emit again
        tokio::time::pause();
        tokio::time::advance(tokio::time::Duration::from_millis(600)).await;
        assert!(
            sampler
                .should_log(tokio::time::Duration::from_millis(500))
                .is_some()
        );
    }
    #[test]
    fn parse_proxy_extracts_auth_and_host() {
        let proxy = parse_proxy_value("http://user:pass@example.com:8080").expect("proxy parsed");
        assert_eq!(proxy.kind, ProxyKind::HttpConnect);
        assert_eq!(proxy.host, "example.com");
        assert_eq!(proxy.port, 8080);
        assert_eq!(
            proxy.auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
            Some(("user", "pass"))
        );
    }
    #[test]
    fn parse_proxy_accepts_socks5_scheme() {
        let proxy = parse_proxy_value("socks5://proxy.example.com:1080").expect("proxy parsed");
        assert_eq!(proxy.kind, ProxyKind::Socks5);
        assert_eq!(proxy.host, "proxy.example.com");
        assert_eq!(proxy.port, 1080);
        assert!(proxy.auth.is_none());
    }
    #[test]
    fn parse_proxy_accepts_https_scheme() {
        let proxy = parse_proxy_value("https://proxy.example.com:8443").expect("proxy parsed");
        assert_eq!(proxy.kind, ProxyKind::HttpConnectTls);
        assert_eq!(proxy.host, "proxy.example.com");
        assert_eq!(proxy.port, 8443);
    }
    #[test]
    fn connect_request_includes_basic_auth_when_present() {
        let proxy = Proxy {
            kind: ProxyKind::HttpConnect,
            host: "example.com".into(),
            port: 8080,
            auth: Some(("user".into(), "pass".into())),
        };
        let req = build_connect_request("dest:443", &proxy);
        assert!(req.contains("Proxy-Authorization: Basic dXNlcjpwYXNz"));
        let proxy_no_auth = Proxy {
            kind: ProxyKind::HttpConnect,
            host: "example.com".into(),
            port: 8080,
            auth: None,
        };
        let req = build_connect_request("dest:443", &proxy_no_auth);
        assert!(!req.contains("Proxy-Authorization"));
    }
    #[test]
    fn apply_tcp_socket_options_enables_keepalive_when_configured() {
        use socket2::{Domain, Protocol, Socket, Type};
        // Binding/listening is prohibited in some sandbox environments. Keep this test local
        // to socket options and avoid requiring a live TCP connection.
        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).expect("socket");
        let sock_ref = SockRef::from(&socket);
        apply_tcp_socket_options_sockref(
            &sock_ref,
            true,
            Some(std::time::Duration::from_secs(123)),
        );
        let enabled = SockRef::from(&socket).keepalive().expect("read keepalive");
        assert!(enabled, "SO_KEEPALIVE was not enabled");
    }
    #[tokio::test(flavor = "current_thread")]
    async fn socks5_connect_no_auth_ipv4_target_roundtrips() {
        use iroha_primitives::addr::socket_addr;
        let (mut client, mut server) = tokio::io::duplex(1024);
        let proxy = Proxy {
            kind: ProxyKind::Socks5,
            host: "proxy.example.com".into(),
            port: 1080,
            auth: None,
        };
        let target = socket_addr!(1.2.3.4:1234);
        let client_fut = async { socks5_connect(&mut client, &proxy, &target).await };
        let server_fut = async move {
            // Greeting
            let mut head = [0u8; 2];
            server.read_exact(&mut head).await?;
            assert_eq!(head[0], 0x05, "VER");
            let n_methods = head[1] as usize;
            let mut methods = vec![0u8; n_methods];
            server.read_exact(&mut methods).await?;
            assert!(
                methods.contains(&0x00),
                "client must advertise NO AUTH method"
            );
            // Choose no-auth
            server.write_all(&[0x05, 0x00]).await?;
            // CONNECT request
            let mut req = [0u8; 4];
            server.read_exact(&mut req).await?;
            assert_eq!(req[0], 0x05, "VER");
            assert_eq!(req[1], 0x01, "CMD=CONNECT");
            assert_eq!(req[2], 0x00, "RSV");
            assert_eq!(req[3], 0x01, "ATYP=IPv4");
            let mut ip = [0u8; 4];
            server.read_exact(&mut ip).await?;
            let mut port = [0u8; 2];
            server.read_exact(&mut port).await?;
            assert_eq!(ip, [1, 2, 3, 4]);
            assert_eq!(u16::from_be_bytes(port), 1234);
            // Reply: success, bind 0.0.0.0:0
            server
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            Ok::<_, io::Error>(())
        };
        let (client_res, server_res) = tokio::join!(client_fut, server_fut);
        client_res.expect("client should succeed");
        server_res.expect("server should complete");
    }
    #[tokio::test(flavor = "current_thread")]
    async fn socks5_connect_username_password_auth_roundtrips() {
        use iroha_primitives::addr::socket_addr;
        let (mut client, mut server) = tokio::io::duplex(1024);
        let proxy = Proxy {
            kind: ProxyKind::Socks5,
            host: "proxy.example.com".into(),
            port: 1080,
            auth: Some(("user".into(), "pass".into())),
        };
        let target = socket_addr!(5.6.7.8:4321);
        let client_fut = async { socks5_connect(&mut client, &proxy, &target).await };
        let server_fut = async move {
            // Greeting
            let mut head = [0u8; 2];
            server.read_exact(&mut head).await?;
            assert_eq!(head[0], 0x05);
            let n_methods = head[1] as usize;
            let mut methods = vec![0u8; n_methods];
            server.read_exact(&mut methods).await?;
            assert!(methods.contains(&0x00));
            assert!(methods.contains(&0x02), "auth method must be advertised");
            // Choose username/password
            server.write_all(&[0x05, 0x02]).await?;
            // RFC 1929 auth request
            let mut ver = [0u8; 1];
            server.read_exact(&mut ver).await?;
            assert_eq!(ver[0], 0x01);
            let mut ulen = [0u8; 1];
            server.read_exact(&mut ulen).await?;
            let mut user = vec![0u8; ulen[0] as usize];
            server.read_exact(&mut user).await?;
            let mut plen = [0u8; 1];
            server.read_exact(&mut plen).await?;
            let mut pass = vec![0u8; plen[0] as usize];
            server.read_exact(&mut pass).await?;
            assert_eq!(user, b"user");
            assert_eq!(pass, b"pass");
            // Auth success
            server.write_all(&[0x01, 0x00]).await?;
            // CONNECT request
            let mut req = [0u8; 4];
            server.read_exact(&mut req).await?;
            assert_eq!(req[0], 0x05);
            assert_eq!(req[1], 0x01);
            assert_eq!(req[2], 0x00);
            assert_eq!(req[3], 0x01);
            let mut ip = [0u8; 4];
            server.read_exact(&mut ip).await?;
            let mut port = [0u8; 2];
            server.read_exact(&mut port).await?;
            assert_eq!(ip, [5, 6, 7, 8]);
            assert_eq!(u16::from_be_bytes(port), 4321);
            server
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            Ok::<_, io::Error>(())
        };
        let (client_res, server_res) = tokio::join!(client_fut, server_fut);
        client_res.expect("client should succeed");
        server_res.expect("server should complete");
    }
    #[tokio::test(flavor = "current_thread")]
    async fn socks5_connect_uses_domain_type_for_hostname_targets() {
        use iroha_primitives::addr::SocketAddrHost;
        let (mut client, mut server) = tokio::io::duplex(1024);
        let proxy = Proxy {
            kind: ProxyKind::Socks5,
            host: "proxy.example.com".into(),
            port: 1080,
            auth: None,
        };
        let target = SocketAddr::Host(SocketAddrHost {
            host: "example.com".into(),
            port: 9999,
        });
        let client_fut = async { socks5_connect(&mut client, &proxy, &target).await };
        let server_fut = async move {
            // Greeting, choose no-auth
            let mut head = [0u8; 2];
            server.read_exact(&mut head).await?;
            let mut methods = vec![0u8; head[1] as usize];
            server.read_exact(&mut methods).await?;
            server.write_all(&[0x05, 0x00]).await?;
            // CONNECT request: DOMAIN
            let mut req = [0u8; 4];
            server.read_exact(&mut req).await?;
            assert_eq!(req[0], 0x05);
            assert_eq!(req[1], 0x01);
            assert_eq!(req[2], 0x00);
            assert_eq!(req[3], 0x03, "ATYP=DOMAIN");
            let mut len = [0u8; 1];
            server.read_exact(&mut len).await?;
            let mut name = vec![0u8; len[0] as usize];
            server.read_exact(&mut name).await?;
            let mut port = [0u8; 2];
            server.read_exact(&mut port).await?;
            assert_eq!(name, b"example.com");
            assert_eq!(u16::from_be_bytes(port), 9999);
            server
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            Ok::<_, io::Error>(())
        };
        let (client_res, server_res) = tokio::join!(client_fut, server_fut);
        client_res.expect("client should succeed");
        server_res.expect("server should complete");
    }
    #[cfg(feature = "p2p_tls")]
    #[tokio::test(flavor = "current_thread")]
    async fn self_signed_tls_rejects_certificate_signed_by_another_key() {
        use rustls::{
            server::{ClientHello, ResolvesServerCert},
            sign::CertifiedKey,
        };
        use std::sync::Arc;
        use tokio::net::{TcpListener, TcpStream};
        use tokio_rustls::TlsAcceptor;
        #[derive(Debug)]
        struct FixedCertificate(Arc<CertifiedKey>);
        impl ResolvesServerCert for FixedCertificate {
            fn resolve(&self, _client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
                Some(Arc::clone(&self.0))
            }
        }
        let rcgen::CertifiedKey { cert, .. } =
            rcgen::generate_simple_self_signed(["iroha-tls".to_owned()])
                .expect("generate advertised certificate");
        let rcgen::CertifiedKey {
            signing_key: unrelated_key,
            ..
        } = rcgen::generate_simple_self_signed(["attacker.local".to_owned()])
            .expect("generate unrelated key");
        let unrelated_key = rustls::pki_types::PrivateKeyDer::from(
            rustls::pki_types::PrivatePkcs8KeyDer::from(unrelated_key.serialize_der()),
        );
        let unrelated_key = rustls::crypto::ring::sign::any_supported_type(&unrelated_key)
            .expect("parse unrelated signing key");
        let advertised_cert = rustls::pki_types::CertificateDer::from(cert.der().as_ref().to_vec());
        let certified_key = CertifiedKey::new(vec![advertised_cert], unrelated_key);
        let server_cfg = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(FixedCertificate(Arc::new(certified_key))));
        let acceptor = TlsAcceptor::from(Arc::new(server_cfg));
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(error) => panic!("bind: {error:?}"),
        };
        let addr = listener.local_addr().expect("local addr");
        let server = async move {
            let (tcp, _) = listener.accept().await.expect("accept");
            acceptor.accept(tcp).await
        };
        let client = async move {
            let tcp = TcpStream::connect(addr).await.expect("connect");
            crate::transport::tls::connect_tls("iroha-tls", tcp).await
        };
        let (client_result, _server_result) = tokio::join!(client, server);
        assert!(
            client_result.is_err(),
            "TLS CertificateVerify must reject a replayed certificate without its private key"
        );
    }
    #[cfg(feature = "p2p_tls")]
    #[tokio::test(flavor = "current_thread")]
    async fn https_proxy_tls_pinning_accepts_only_matching_cert() {
        use std::sync::Arc;
        use tokio::net::{TcpListener, TcpStream};
        use tokio_rustls::TlsAcceptor;
        // A self-signed TLS server stands in for an `https://` proxy.
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(["proxy.local".to_owned()]).expect("generate cert");
        let cert_der = cert.der().clone();
        let cert_chain = vec![rustls::pki_types::CertificateDer::from(cert_der).into_owned()];
        let priv_key = rustls::pki_types::PrivateKeyDer::from(
            rustls::pki_types::PrivatePkcs8KeyDer::from(signing_key.serialize_der()),
        )
        .clone_key();
        let server_cfg = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, priv_key)
            .expect("server config");
        let acceptor = TlsAcceptor::from(Arc::new(server_cfg));
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("bind: {e:?}"),
        };
        let addr = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                let (tcp, _) = listener.accept().await.expect("accept");
                let _ = acceptor.accept(tcp).await;
            }
        });
        // P2P TLS accepts the self-signed certificate after verifying key possession.
        let tcp = TcpStream::connect(addr).await.expect("connect");
        let self_signed = crate::transport::tls::connect_tls("proxy.local", tcp).await;
        assert!(
            self_signed.is_ok(),
            "P2P TLS should accept a self-signed cert with a valid CertificateVerify"
        );
        // Pinning should accept the exact end-entity certificate.
        let tcp = TcpStream::connect(addr).await.expect("connect");
        let pinned = Arc::<[u8]>::from(cert.der().as_ref().to_vec());
        let verified = crate::transport::tls::connect_tls_pinned("proxy.local", tcp, pinned).await;
        assert!(
            verified.is_ok(),
            "pinned TLS should accept the pinned certificate"
        );
        // A mismatched pin should be rejected.
        let tcp = TcpStream::connect(addr).await.expect("connect");
        let mut wrong = cert.der().as_ref().to_vec();
        wrong[0] = wrong[0].wrapping_add(1);
        let wrong = Arc::<[u8]>::from(wrong);
        let verified = crate::transport::tls::connect_tls_pinned("proxy.local", tcp, wrong).await;
        assert!(
            verified.is_err(),
            "pinned TLS should reject mismatched certificates"
        );
        let _ = server.await;
    }
}
