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
    //! outbound dials. Peer authentication remains at the application
    //! layer (signed handshake), so certificate verification is intentionally
    //! permissive and ALPN is fixed.

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
    }

    impl Default for DialerConfig {
        fn default() -> Self {
            Self {
                max_idle_timeout: None,
                // A small keep-alive keeps common NAT mappings fresh and reduces idle drops.
                keep_alive_interval: Some(Duration::from_secs(10)),
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
        Ok(Arc::new(transport))
    }
}

/// QUIC dialer handle type.
#[cfg(feature = "quic")]
pub type QuicDialer = quic::Dialer;
/// Stub QUIC dialer type when QUIC support is not compiled in.
#[cfg(not(feature = "quic"))]
pub type QuicDialer = ();

#[cfg(feature = "p2p_tls")]
pub mod tls {
    //! TLS-over-TCP transport (feature-gated, optional).
    //!
    //! Wraps a TCP stream with TLS 1.3 using rustls. Certificate verification is
    //! intentionally permissive for P2P; peer authentication is enforced at the
    //! application layer by the signed handshake.

    use std::sync::Arc;

    use rustls::{
        ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, ServerName, UnixTime},
    };
    use tokio::net::TcpStream;
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
    pub async fn connect_tls(
        host: &str,
        tcp: TcpStream,
    ) -> tokio::io::Result<TlsStream<TcpStream>> {
        // Build a permissive client config
        let verifier: Arc<dyn ServerCertVerifier> = Arc::new(NoCertificateVerification);
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        let config = Arc::new(config);
        let connector = TlsConnector::from(config);

        let server_name = ServerName::try_from(host)
            .map_err(|_| tokio::io::Error::new(tokio::io::ErrorKind::InvalidInput, "invalid SNI"))?
            .to_owned();
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
        MaybeTlsStream,
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

/// HTTP CONNECT proxy configuration for outbound TCP dials.
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
            .as_deref()
            .map(parse_proxy_value)
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
    /// Whether to enable TCP_NODELAY for reduced latency.
    pub tcp_nodelay: bool,
    /// Optional keepalive idle time. When `None`, keepalive is disabled.
    pub tcp_keepalive: Option<std::time::Duration>,
}

impl Default for TcpConnectOptions {
    fn default() -> Self {
        Self {
            proxy: ProxyPolicy::disabled(),
            tcp_nodelay: true,
            tcp_keepalive: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Proxy {
    host: String,
    port: u16,
    auth: Option<(String, String)>,
}

fn parse_proxy_value(raw: &str) -> std::result::Result<Proxy, String> {
    let mut s = raw;
    if let Some(rest) = s.strip_prefix("http://") {
        s = rest;
    } else if let Some(rest) = s.strip_prefix("https://") {
        s = rest; // we still do a plain HTTP CONNECT; TLS to proxy not supported here
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
    let (host, port_str) = s
        .split_once(':')
        .ok_or_else(|| "proxy URL missing port".to_string())?;
    if host.is_empty() {
        return Err("proxy URL missing host".to_string());
    }
    let port: u16 = port_str
        .parse()
        .map_err(|_| "proxy URL has invalid port".to_string())?;
    Ok(Proxy {
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

/// Connect to a peer using the default transport (TCP).
///
/// When the `quic` feature is enabled, this remains a placeholder and
/// will be upgraded to a proper QUIC transport in a future change.
///
/// # Errors
///
/// Returns an `io::Error` if TCP connect fails, proxy handshake fails, or I/O operations error.
pub async fn connect(addr: &SocketAddr, opts: &TcpConnectOptions) -> Result<TcpStream> {
    // If a proxy is configured and the target is not in NO_PROXY, tunnel via HTTP CONNECT.
    if let Some(proxy) = opts.proxy.pick_proxy_for_target(addr) {
        let mut stream = match TcpStream::connect(format!("{}:{}", proxy.host, proxy.port)).await {
            Ok(s) => s,
            Err(e) => {
                static PROXY_CONNECT_SAMPLER: OnceLock<Mutex<LogSampler>> = OnceLock::new();
                let sampler = PROXY_CONNECT_SAMPLER.get_or_init(|| Mutex::new(LogSampler::new()));
                if let Ok(mut s) = sampler.lock() {
                    if let Some(supp) = s.should_log(tokio::time::Duration::from_millis(500)) {
                        iroha_logger::warn!(%e, proxy=%format!("{}:{}", proxy.host, proxy.port), suppressed=supp, "Failed to connect to HTTP proxy");
                    }
                }
                return Err(e);
            }
        };
        apply_tcp_socket_options(&stream, opts.tcp_nodelay, opts.tcp_keepalive);
        let target = addr.to_string();
        let req = build_connect_request(&target, &proxy);
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
                    iroha_logger::warn!(status=%text.lines().next().unwrap_or("?"), target=%target, suppressed=supp, "HTTP CONNECT to proxy failed");
                }
            }
            return Err(io::Error::other("proxy CONNECT failed"));
        }
        Ok(stream)
    } else {
        match TcpStream::connect(addr.to_string()).await {
            Ok(stream) => {
                apply_tcp_socket_options(&stream, opts.tcp_nodelay, opts.tcp_keepalive);
                Ok(stream)
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
    let _ = stream.set_nodelay(tcp_nodelay);
    if let Some(idle) = tcp_keepalive {
        // Best-effort: keepalive knobs vary across OSes. Socket2 provides a safe wrapper.
        let sock_ref = SockRef::from(stream);
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
        assert_eq!(proxy.host, "example.com");
        assert_eq!(proxy.port, 8080);
        assert_eq!(
            proxy.auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
            Some(("user", "pass"))
        );
    }

    #[test]
    fn connect_request_includes_basic_auth_when_present() {
        let proxy = Proxy {
            host: "example.com".into(),
            port: 8080,
            auth: Some(("user".into(), "pass".into())),
        };
        let req = build_connect_request("dest:443", &proxy);
        assert!(req.contains("Proxy-Authorization: Basic dXNlcjpwYXNz"));

        let proxy_no_auth = Proxy {
            host: "example.com".into(),
            port: 8080,
            auth: None,
        };
        let req = build_connect_request("dest:443", &proxy_no_auth);
        assert!(!req.contains("Proxy-Authorization"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn apply_tcp_socket_options_enables_keepalive_when_configured() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let accept_task = tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
        apply_tcp_socket_options(&stream, true, Some(std::time::Duration::from_secs(123)));

        let enabled = SockRef::from(&stream).keepalive().expect("read keepalive");
        assert!(enabled, "SO_KEEPALIVE was not enabled");

        let _ = accept_task.await;
    }
}
