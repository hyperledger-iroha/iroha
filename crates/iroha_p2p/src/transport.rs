//! Transport and handshake scaffolding traits.
//!
//! This module provides thin abstractions intended to support optional
//! transports (e.g., QUIC) and handshakes (e.g., Noise/TLS) behind
//! feature flags without affecting the default TCP path.

#[cfg(feature = "quic")]
pub mod quic {
    #![allow(clippy::missing_errors_doc)]
    //! QUIC transport integration (feature-gated, optional).
    //!
    //! This module provides a QUIC dialer that can be reused across many
    //! outbound dials. Certificate verification remains permissive because
    //! peer authentication is enforced by the signed handshake, which is
    //! bound to the presented server certificate fingerprint for the active
    //! session. ALPN is fixed.

    use std::{io, sync::Arc, time::Duration};

    use quinn::{
        ClientConfig, Connection, Endpoint, IdleTimeout, RecvStream, SendStream, TransportConfig,
        crypto::rustls::QuicClientConfig as QuinnRustlsClientConfig,
    };
    use rustls::{
        DigitallySignedStruct, Error as RustlsError, SignatureScheme,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, ServerName, UnixTime},
    };

    /// ALPN negotiated for Iroha P2P QUIC connections.
    pub const P2P_ALPN: &[u8] = b"iroha-p2p/1";

    #[derive(Debug)]
    struct NoCertificateVerification;

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, RustlsError> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::ED25519,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PKCS1_SHA256,
            ]
        }
    }

    /// QUIC transport tuning for outbound dials.
    #[derive(Clone, Copy, Debug)]
    pub struct DialerConfig {
        /// QUIC idle timeout (transport-level), if set.
        pub max_idle_timeout: Option<Duration>,
        /// QUIC keep-alive interval, if set.
        pub keep_alive_interval: Option<Duration>,
        /// Total receive buffer reserved for QUIC datagrams (bytes). `None` disables datagrams.
        pub datagram_receive_buffer: Option<usize>,
        /// Total send buffer reserved for QUIC datagrams (bytes). Set to 0 to disable.
        pub datagram_send_buffer: usize,
    }

    impl Default for DialerConfig {
        fn default() -> Self {
            Self {
                max_idle_timeout: None,
                // A small keep-alive keeps common NAT mappings fresh and reduces idle drops.
                keep_alive_interval: Some(Duration::from_secs(10)),
                datagram_receive_buffer: None,
                datagram_send_buffer: 0,
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

            let verifier: Arc<dyn ServerCertVerifier> = Arc::new(NoCertificateVerification);
            let mut tls = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(verifier)
                .with_no_client_auth();
            tls.enable_early_data = true;
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
        let mut transport = TransportConfig::default();
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
    //! Wraps a TCP stream with TLS 1.3 using rustls. Certificate verification is
    //! intentionally permissive for P2P, but peer authentication is enforced at
    //! the application layer by the signed handshake bound to the presented
    //! server certificate fingerprint.

    use std::sync::Arc;

    use rustls::{
        ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, ServerName, UnixTime},
    };
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio_rustls::{TlsConnector, client::TlsStream};

    #[derive(Debug)]
    struct NoCertificateVerification;

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, RustlsError> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::ED25519,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PKCS1_SHA256,
            ]
        }
    }

    /// Upgrade an already-connected TCP stream to TLS 1.3.
    pub async fn connect_tls<S>(host: &str, tcp: S) -> tokio::io::Result<TlsStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        // Build a permissive client config
        let verifier: Arc<dyn ServerCertVerifier> = Arc::new(NoCertificateVerification);
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

    #[derive(Clone, Debug)]
    struct PinnedCertificateVerification {
        expected_der: Arc<[u8]>,
    }

    impl ServerCertVerifier for PinnedCertificateVerification {
        fn verify_server_cert(
            &self,
            end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, RustlsError> {
            if end_entity.as_ref() == self.expected_der.as_ref() {
                Ok(ServerCertVerified::assertion())
            } else {
                Err(RustlsError::General(
                    "pinned proxy certificate mismatch".to_string(),
                ))
            }
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::ED25519,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PKCS1_SHA256,
            ]
        }
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
        let verifier: Arc<dyn ServerCertVerifier> = Arc::new(PinnedCertificateVerification {
            expected_der: expected_cert_der,
        });
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
        tungstenite::{Message, client::IntoClientRequest},
    };

    /// A duplex adaptor that implements `AsyncRead`/`AsyncWrite` over a WebSocket stream.
    /// Bytes written are sent as a single Binary frame on `poll_flush`. Bytes are read by
    /// consuming incoming Binary frames. This preserves application framing above.
    pub struct WsDuplex<S> {
        inner: tokio_tungstenite::WebSocketStream<S>,
        read_buf: bytes::Bytes, // remaining unread bytes from last Binary frame
        write_buf: Vec<u8>,
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
            }
        }
    }

    /// Perform a websocket client handshake over an already-established stream.
    ///
    /// This is useful for applying custom TCP dial logic (proxies, socket options) while
    /// still speaking WebSocket/WSS at the HTTP layer.
    pub async fn connect_with_stream<R, S>(
        request: R,
        stream: S,
    ) -> std::io::Result<WsDuplex<MaybeTlsStream<S>>>
    where
        R: IntoClientRequest + Unpin,
        S: 'static + AsyncRead + AsyncWrite + Send + Unpin,
        MaybeTlsStream<S>: Unpin,
    {
        let (ws_stream, _resp) = client_async_tls_with_config(request, stream, None, None)
            .await
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("ws connect: {e}"))
            })?;
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
            if !self.read_buf.is_empty() {
                let n = std::cmp::min(self.read_buf.len(), buf.remaining());
                buf.put_slice(&self.read_buf.split_to(n));
                return std::task::Poll::Ready(Ok(()));
            }
            // Pull next Binary frame
            match futures::ready!(std::pin::Pin::new(&mut self.inner).poll_next(cx)) {
                Some(Ok(Message::Binary(b))) => {
                    self.read_buf = b;
                    let n = std::cmp::min(self.read_buf.len(), buf.remaining());
                    buf.put_slice(&self.read_buf.split_to(n));
                    std::task::Poll::Ready(Ok(()))
                }
                Some(Ok(Message::Text(_)))
                | Some(Ok(Message::Ping(_)))
                | Some(Ok(Message::Pong(_)))
                | Some(Ok(Message::Frame(_))) => {
                    // Ignore control/text frames and read next
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
                Some(Ok(Message::Close(_))) | None => std::task::Poll::Ready(Ok(())),
                Some(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("ws read error: {e}"),
                ))),
            }
        }
    }

    impl<S> AsyncWrite for WsDuplex<S>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            data: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            self.write_buf.extend_from_slice(data);
            std::task::Poll::Ready(Ok(data.len()))
        }

        fn poll_flush(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            if self.write_buf.is_empty() {
                return std::task::Poll::Ready(Ok(()));
            }
            let data = std::mem::take(&mut self.write_buf);
            let mut sink = std::pin::Pin::new(&mut self.inner);
            futures::ready!(sink.as_mut().poll_ready(cx).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("ws poll_ready error: {e}"),
                )
            }))?;
            sink.as_mut()
                .start_send(Message::Binary(data.into()))
                .map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("ws send error: {e}"))
                })?;
            futures::ready!(sink.as_mut().poll_flush(cx).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("ws flush error: {e}"))
            }))?;
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            if !self.write_buf.is_empty() {
                // Flush any buffered payload first.
                futures::ready!(self.as_mut().poll_flush(cx))?;
            }
            let mut sink = std::pin::Pin::new(&mut self.inner);
            futures::ready!(sink.as_mut().poll_ready(cx).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("ws close poll_ready error: {e}"),
                )
            }))?;
            sink.as_mut()
                .start_send(Message::Close(None))
                .map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("ws close error: {e}"))
                })?;
            futures::ready!(sink.as_mut().poll_flush(cx).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("ws close flush error: {e}"),
                )
            }))?;
            std::task::Poll::Ready(Ok(()))
        }
    }

    /// Connect a WSS endpoint `wss://host:port/p2p` and return a duplex stream.
    pub async fn connect_wss(
        endpoint: &str,
    ) -> std::io::Result<WsDuplex<MaybeTlsStream<tokio::net::TcpStream>>> {
        let url = format!("wss://{endpoint}/p2p");
        let req = url.into_client_request().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad url: {e}"))
        })?;
        let (ws_stream, _resp) = tokio_tungstenite::connect_async(req).await.map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("wss connect: {e}"))
        })?;
        Ok(WsDuplex::new(ws_stream))
    }

    /// Connect a WS endpoint `ws://host:port/p2p` and return a duplex stream.
    pub async fn connect_ws(
        endpoint: &str,
    ) -> std::io::Result<WsDuplex<MaybeTlsStream<tokio::net::TcpStream>>> {
        let url = format!("ws://{endpoint}/p2p");
        let req = url.into_client_request().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad url: {e}"))
        })?;
        let (ws_stream, _resp) = tokio_tungstenite::connect_async(req).await.map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("ws connect: {e}"))
        })?;
        Ok(WsDuplex::new(ws_stream))
    }
}

#[cfg(feature = "p2p_turn")]
pub mod turn {
    //! Minimal TURN relay scaffold. This is a placeholder to exercise the
    //! feature flag and document env-based configuration; it does not implement
    //! the TURN protocol. When `P2P_TURN` is set to `host:port`, the function
    //! attempts to open a TCP connection as a smoke test.
    use tokio::net::TcpStream;

    /// Attempt to connect to a TURN relay specified via the `P2P_TURN` env var.
    pub async fn connect_via_env() -> std::io::Result<TcpStream> {
        let endpoint = std::env::var("P2P_TURN")
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "P2P_TURN not set"))?;
        TcpStream::connect(endpoint).await
    }

    #[cfg(test)]
    mod tests {
        #[tokio::test(flavor = "current_thread")]
        async fn turn_env_absent_errors() {
            std::env::remove_var("P2P_TURN");
            let err = super::connect_via_env().await.unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        }
    }
}

#[cfg(feature = "noise_handshake")]
pub mod noise {
    //! Noise-based handshake integration (feature-gated, optional).
    //!
    //! Provides helper to perform a Noise XX pattern handshake using the
    //! `snow` crate and derive a session key for symmetric encryption.
    //! The Iroha identity key can be embedded in the handshake payload
    //! to authenticate the peer at the application layer.

    use snow::{Builder, HandshakeState, params::NoiseParams};

    const MAX_HANDSHAKE_MESSAGE_LEN: usize = 65535;

    fn build_handshake_states() -> Result<(HandshakeState, HandshakeState), snow::Error> {
        let initiator_params: NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2b".parse().unwrap();
        let responder_params: NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2b".parse().unwrap();

        let initiator_builder = Builder::new(initiator_params);
        let initiator_keys = initiator_builder.generate_keypair()?;
        let initiator = initiator_builder
            .local_private_key(&initiator_keys.private)?
            .build_initiator()?;

        let responder_builder = Builder::new(responder_params);
        let responder_keys = responder_builder.generate_keypair()?;
        let responder = responder_builder
            .local_private_key(&responder_keys.private)?
            .build_responder()?;
        Ok((initiator, responder))
    }

    fn transmit_handshake_message(
        writer: &mut HandshakeState,
        reader: &mut HandshakeState,
        payload: &[u8],
        buffer: &mut [u8; MAX_HANDSHAKE_MESSAGE_LEN],
    ) -> Result<(), snow::Error> {
        let written = writer.write_message(payload, buffer)?;
        let mut sink = vec![0u8; MAX_HANDSHAKE_MESSAGE_LEN];
        reader.read_message(&buffer[..written], &mut sink)?;
        Ok(())
    }

    /// Perform a full Noise XX handshake exchanging three messages and return
    /// the derived handshake hash (identical for both peers).
    #[allow(dead_code)]
    pub fn xx_handshake(
        initiator_payload: &[u8],
        responder_payload: &[u8],
    ) -> Result<Vec<u8>, snow::Error> {
        let (mut initiator, mut responder) = build_handshake_states()?;
        let mut buffer = [0u8; MAX_HANDSHAKE_MESSAGE_LEN];
        transmit_handshake_message(
            &mut initiator,
            &mut responder,
            initiator_payload,
            &mut buffer,
        )?;
        transmit_handshake_message(
            &mut responder,
            &mut initiator,
            responder_payload,
            &mut buffer,
        )?;
        transmit_handshake_message(&mut initiator, &mut responder, &[], &mut buffer)?;

        let initiator_hash = initiator.get_handshake_hash().to_vec();
        let responder_hash = responder.get_handshake_hash().to_vec();
        if initiator_hash != responder_hash {
            return Err(snow::Error::Decrypt);
        }
        initiator.into_transport_mode()?;
        responder.into_transport_mode()?;
        Ok(initiator_hash)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn xx_handshake_roundtrip_hashes_match() {
            let hash = xx_handshake(b"init-payload", b"resp-payload").expect("handshake");
            assert!(!hash.is_empty(), "handshake hash must not be empty");
        }

        #[test]
        fn xx_handshake_detects_tampering() {
            let (mut initiator, mut responder) = build_handshake_states().expect("states");
            let mut buffer = [0u8; MAX_HANDSHAKE_MESSAGE_LEN];
            let mut sink = vec![0u8; MAX_HANDSHAKE_MESSAGE_LEN];
            let first_len = initiator.write_message(&[], &mut buffer).expect("write");
            responder
                .read_message(&buffer[..first_len], &mut sink)
                .expect("responder reads first message");
            let second_len = responder.write_message(&[], &mut buffer).expect("write2");
            assert!(second_len > 0, "second handshake message must have bytes");
            buffer[0] ^= 0xFF;
            let err = initiator
                .read_message(&buffer[..second_len], &mut sink)
                .expect_err("tampered message should fail");
            assert!(
                matches!(err, snow::Error::Decrypt | snow::Error::State(_)),
                "expected decrypt/state error, got {err:?}"
            );
        }

        #[test]
        fn xx_handshake_rejects_truncated_messages() {
            let (mut initiator, mut responder) = build_handshake_states().expect("states");
            let mut buffer = [0u8; MAX_HANDSHAKE_MESSAGE_LEN];
            let written = initiator.write_message(&[], &mut buffer).expect("write");
            assert!(written > 4);
            let mut sink = vec![0u8; MAX_HANDSHAKE_MESSAGE_LEN];
            let err = responder
                .read_message(&buffer[..written - 4], &mut sink)
                .expect_err("truncated message should fail");
            assert!(
                matches!(err, snow::Error::Input | snow::Error::State(_)),
                "expected input/state error, got {err:?}"
            );
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

use std::sync::{Mutex, OnceLock};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_primitives::addr::SocketAddr;
use socket2::{SockRef, TcpKeepalive};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt, Result},
    net::TcpStream,
};

use crate::sampler::LogSampler;

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

        // Insecure P2P TLS accepts the self-signed certificate.
        let tcp = TcpStream::connect(addr).await.expect("connect");
        let insecure = crate::transport::tls::connect_tls("proxy.local", tcp).await;
        assert!(
            insecure.is_ok(),
            "insecure TLS should accept self-signed cert"
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
