/// HTTP/1 limits enforced before request middleware.
#[derive(Debug, Clone, Copy)]
pub struct ToriiHttpTransport {
    /// Maximum accepted TCP connections retained by Torii.
    pub max_connections: NonZeroUsize,
    /// Maximum accepted TCP connections retained for one source IP.
    pub max_connections_per_ip: NonZeroUsize,
    /// Absolute deadline for reading one HTTP/1 request head.
    pub header_read_timeout: Duration,
    /// Maximum duration without socket write progress.
    pub write_timeout: Duration,
    /// Maximum number of HTTP/1 headers accepted in one request.
    pub max_headers: NonZeroUsize,
    /// Maximum HTTP/1 parser buffer, including the request head.
    pub max_header_bytes: Bytes<u64>,
}
impl Default for ToriiHttpTransport {
    fn default() -> Self {
        Self {
            max_connections: defaults::torii::transport::http::MAX_CONNECTIONS,
            max_connections_per_ip: defaults::torii::transport::http::MAX_CONNECTIONS_PER_IP,
            header_read_timeout: Duration::from_millis(
                defaults::torii::transport::http::HEADER_READ_TIMEOUT_MS,
            ),
            write_timeout: Duration::from_millis(
                defaults::torii::transport::http::WRITE_TIMEOUT_MS,
            ),
            max_headers: defaults::torii::transport::http::MAX_HEADERS,
            max_header_bytes: defaults::torii::transport::http::MAX_HEADER_BYTES,
        }
    }
}
