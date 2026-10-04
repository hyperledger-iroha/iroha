/// Optional native HTTPS listener on the same router and socket pool as HTTP.
#[derive(Debug, Clone)]
pub struct ToriiHttpsTransport {
    /// Address for the additional HTTPS listener.
    pub address: WithOrigin<SocketAddr>,
    /// Bounded leaf-first DER certificate chain, resolved from configuration origins.
    pub certificate_chain: Vec<PathBuf>,
    /// DER private key file read through native private-file custody at startup.
    pub private_key: PathBuf,
    /// Absolute deadline from socket admission to completion of the TLS handshake.
    pub handshake_timeout: Duration,
}
