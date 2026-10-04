//! Optional native HTTPS transport; identity loading grants no route or provider authority.

use std::{io, sync::Arc, time::Duration};

use iroha_config::parameters::{actual::ToriiHttpsTransport, defaults::torii::transport::https};
use iroha_primitives::addr::SocketAddr;
use tokio::net::TcpListener;
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer},
    },
};

/// Validate programmatically constructed actual configurations as well as parsed ones.
pub(super) fn validate(config: &ToriiHttpsTransport) -> io::Result<()> {
    if !(1..=https::MAX_CERTIFICATES).contains(&config.certificate_chain.len())
        || config
            .certificate_chain
            .iter()
            .any(|path| path.as_os_str().is_empty())
        || config.private_key.as_os_str().is_empty()
        || config.handshake_timeout.is_zero()
        || config.handshake_timeout > Duration::from_millis(https::MAX_HANDSHAKE_TIMEOUT_MS)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Torii HTTPS identity or handshake bounds",
        ));
    }
    Ok(())
}

/// Successfully loaded identity; neither listener nor service worker has started.
pub(super) struct PreparedHttps {
    address: SocketAddr,
    acceptor: TlsAcceptor,
    handshake_timeout: Duration,
}
impl PreparedHttps {
    /// Read bounded native private files and check the actual leaf/key pair.
    pub(super) fn load(config: &ToriiHttpsTransport) -> io::Result<Self> {
        validate(config)?;
        let mut certificates = Vec::with_capacity(config.certificate_chain.len());
        for path in &config.certificate_chain {
            let bytes = iroha_fs::read_private(path, https::MAX_DER_BYTES)?;
            if bytes.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "empty Torii HTTPS certificate",
                ));
            }
            let (remaining, _) = x509_parser::parse_x509_certificate(&bytes).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid Torii HTTPS certificate DER",
                )
            })?;
            if !remaining.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trailing Torii HTTPS certificate bytes",
                ));
            }
            certificates.push(CertificateDer::from(bytes.to_vec()));
        }
        let bytes = iroha_fs::read_private(&config.private_key, https::MAX_DER_BYTES)?;
        let private_key = PrivateKeyDer::try_from(bytes.to_vec()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Torii HTTPS private key DER",
            )
        })?;
        // The ordinary Rustls owner validates DER and the leaf/private-key match.
        // Name, certificate lifetime and root validation remain with TLS clients.
        let mut server = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Torii HTTPS protocol configuration failed",
            )
        })?
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Torii HTTPS certificate/key identity",
            )
        })?;
        server.max_early_data_size = 0;
        server.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Self {
            address: config.address.value().clone(),
            acceptor: TlsAcceptor::from(Arc::new(server)),
            handshake_timeout: config.handshake_timeout,
        })
    }

    /// Bind only after identity admission, before Torii starts its service workers.
    pub(super) async fn bind(self) -> io::Result<BoundHttpsListener> {
        let listener = super::bind_torii_tcp_listener(self.address).await?;
        Ok(BoundHttpsListener {
            listener,
            identity: HttpsIdentity {
                acceptor: self.acceptor,
                handshake_timeout: self.handshake_timeout,
            },
        })
    }
}

#[derive(Clone)]
pub(super) struct HttpsIdentity {
    pub(super) acceptor: TlsAcceptor,
    pub(super) handshake_timeout: Duration,
}

pub(super) struct BoundHttpsListener {
    pub(super) listener: TcpListener,
    pub(super) identity: HttpsIdentity,
}

#[cfg(test)]
mod tests;
