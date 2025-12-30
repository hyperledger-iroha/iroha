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
    //! This module provides a minimal QUIC connector that establishes a
    //! client connection to a remote endpoint and opens a bidirectional
    //! stream. Full integration with the peer networking code will
    //! require IO abstraction; this module focuses on establishing the
    //! connection and stream.

    /// Establish a QUIC connection and open a bidirectional stream.
    ///
    /// Note: certificate verification policy must be defined by the
    /// caller. For P2P with application-level authentication, it is
    /// common to accept self-signed certs and authenticate the peer at
    /// the handshake layer.
    #[allow(dead_code)]
    pub async fn connect_and_open_bi(
        server_name: &str,
        addr: &str,
    ) -> std::io::Result<(quinn::Connection, quinn::SendStream, quinn::RecvStream)> {
        use std::io::Error;

        let endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| Error::other(e.to_string()))?;

        let conn = endpoint
            .connect(addr.parse().unwrap(), server_name)
            .map_err(|e| Error::other(e.to_string()))?;
        let connection = conn.await.map_err(|e| Error::other(e.to_string()))?;
        let (send, recv) = connection
            .open_bi()
            .await
            .map_err(|e| Error::other(e.to_string()))?;
        Ok((connection, send, recv))
    }
}

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

    /// Establish a TLS-over-TCP connection to `endpoint` (host:port).
    pub async fn connect_tls(endpoint: &str) -> tokio::io::Result<TlsStream<TcpStream>> {
        // Split host and port for SNI and TCP connect
        let (host, _port) = endpoint.rsplit_once(':').ok_or_else(|| {
            tokio::io::Error::new(tokio::io::ErrorKind::InvalidInput, "expected host:port")
        })?;

        // Build a permissive client config
        let verifier: Arc<dyn ServerCertVerifier> = Arc::new(NoCertificateVerification);
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        let config = Arc::new(config);
        let connector = TlsConnector::from(config);

        // Connect TCP then upgrade to TLS
        let tcp = TcpStream::connect(endpoint).await?;
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
    use futures::{SinkExt, StreamExt};
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

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

    impl<S: Unpin> AsyncRead for WsDuplex<S> {
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
                    self.read_buf = bytes::Bytes::from(b);
                    let n = std::cmp::min(self.read_buf.len(), buf.remaining());
                    buf.put_slice(&self.read_buf.split_to(n));
                    std::task::Poll::Ready(Ok(()))
                }
                Some(Ok(Message::Text(_)))
                | Some(Ok(Message::Ping(_)))
                | Some(Ok(Message::Pong(_))) => {
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

    impl<S: Unpin> AsyncWrite for WsDuplex<S> {
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
            match futures::ready!(
                sink.as_mut()
                    .start_send(Message::Binary(data))
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("ws send error: {e}"),
                        )
                    })
            ) {
                () => {}
            }
            match futures::ready!(sink.as_mut().poll_flush(cx).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("ws flush error: {e}"))
            })) {
                () => std::task::Poll::Ready(Ok(())),
            }
        }

        fn poll_shutdown(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            let mut sink = std::pin::Pin::new(&mut self.inner);
            match futures::ready!(sink.as_mut().start_send(Message::Close(None)).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("ws close error: {e}"))
            })) {
                () => {}
            }
            match futures::ready!(sink.as_mut().poll_flush(cx).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("ws close flush error: {e}"),
                )
            })) {
                () => std::task::Poll::Ready(Ok(())),
            }
        }
    }

    /// Connect a WSS endpoint `wss://host:port/p2p` and return a duplex stream.
    pub async fn connect_wss(
        endpoint: &str,
    ) -> std::io::Result<WsDuplex<tokio_tungstenite::ConnectorStream>> {
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
    ) -> std::io::Result<WsDuplex<tokio_tungstenite::ConnectorStream>> {
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

use std::{
    env,
    sync::{Mutex, OnceLock},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_primitives::addr::SocketAddr;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt, Result},
    net::TcpStream,
};

use crate::sampler::LogSampler;

#[derive(Debug, Clone)]
struct Proxy {
    host: String,
    port: u16,
    auth: Option<(String, String)>,
}

fn should_bypass_proxy(target_host: &str) -> bool {
    let no_proxy = env::var("NO_PROXY")
        .ok()
        .or_else(|| env::var("no_proxy").ok());
    if let Some(list) = no_proxy {
        for entry in list.split(',').map(str::trim) {
            if entry.is_empty() {
                continue;
            }
            if target_host.ends_with(entry) {
                return true;
            }
        }
    }
    false
}

fn parse_proxy_value(raw: &str) -> Option<Proxy> {
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
    let (host, port_str) = s.split_once(':')?;
    let port: u16 = port_str.parse().ok()?;
    Some(Proxy {
        host: host.to_string(),
        port,
        auth,
    })
}

fn parse_proxy_var(var: &str) -> Option<Proxy> {
    let raw = env::var(var).ok()?;
    parse_proxy_value(&raw)
}

fn pick_proxy_for_target(target: &SocketAddr) -> Option<Proxy> {
    // Resolve target host string for NO_PROXY checks
    let host = match target {
        SocketAddr::Host(h) => h.host.as_ref(),
        SocketAddr::Ipv4(v4) => {
            // Render IPv4 dotted quad
            return if should_bypass_proxy(&format!(
                "{}.{}.{}.{}",
                v4.ip[0], v4.ip[1], v4.ip[2], v4.ip[3]
            )) {
                None
            } else {
                // Prefer HTTPS_PROXY, then HTTP_PROXY
                parse_proxy_var("HTTPS_PROXY")
                    .or_else(|| parse_proxy_var("https_proxy"))
                    .or_else(|| parse_proxy_var("HTTP_PROXY"))
                    .or_else(|| parse_proxy_var("http_proxy"))
            };
        }
        SocketAddr::Ipv6(_v6) => {
            // For NO_PROXY matching on IPv6, skip and rely on explicit NO_PROXY entries.
            // Prefer HTTPS proxy if set.
            return parse_proxy_var("HTTPS_PROXY")
                .or_else(|| parse_proxy_var("https_proxy"))
                .or_else(|| parse_proxy_var("HTTP_PROXY"))
                .or_else(|| parse_proxy_var("http_proxy"));
        }
    };
    if should_bypass_proxy(host) {
        return None;
    }
    parse_proxy_var("HTTPS_PROXY")
        .or_else(|| parse_proxy_var("https_proxy"))
        .or_else(|| parse_proxy_var("HTTP_PROXY"))
        .or_else(|| parse_proxy_var("http_proxy"))
}

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
pub async fn connect(addr: &SocketAddr) -> Result<TcpStream> {
    // If a proxy is configured and the target is not in NO_PROXY, tunnel via HTTP CONNECT.
    if let Some(proxy) = pick_proxy_for_target(addr) {
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
            Ok(s) => Ok(s),
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
}
