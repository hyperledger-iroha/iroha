/// Optional native HTTPS transport. An absent address disables the listener.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiHttpsTransport {
    /// Additional listener address; it does not replace Torii's HTTP address.
    pub address: Option<WithOrigin<SocketAddr>>,
    /// One to four leaf-first DER certificate paths; relative to their config origin.
    #[config(default)]
    pub certificate_chain: WithOrigin<Vec<PathBuf>>,
    /// DER private key path; relative to its config origin.
    pub private_key: Option<WithOrigin<PathBuf>>,
    /// Absolute handshake deadline, including peers that send no TLS bytes.
    #[config(
        default = "DurationMs(Duration::from_millis(defaults::torii::transport::https::HANDSHAKE_TIMEOUT_MS))"
    )]
    pub handshake_timeout_ms: DurationMs,
}
impl Default for ToriiHttpsTransport {
    fn default() -> Self {
        Self {
            address: None,
            certificate_chain: WithOrigin::inline(Vec::new()),
            private_key: None,
            handshake_timeout_ms: DurationMs(Duration::from_millis(
                defaults::torii::transport::https::HANDSHAKE_TIMEOUT_MS,
            )),
        }
    }
}
impl ToriiHttpsTransport {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::ToriiHttpsTransport> {
        use defaults::torii::transport::https::{MAX_CERTIFICATES, MAX_HANDSHAKE_TIMEOUT_MS};
        let timeout = self.handshake_timeout_ms.get();
        if timeout.is_zero() || timeout > Duration::from_millis(MAX_HANDSHAKE_TIMEOUT_MS) {
            emit_torii_config_error(
                emitter,
                "torii.transport.https.handshake_timeout_ms must be between 1 and 120000",
            );
        }
        let Some(address) = self.address else {
            if !self.certificate_chain.value().is_empty() || self.private_key.is_some() {
                emit_torii_config_error(
                    emitter,
                    "torii.transport.https.address is required when an identity is configured",
                );
            }
            return None;
        };
        if !(1..=MAX_CERTIFICATES).contains(&self.certificate_chain.value().len())
            || self
                .certificate_chain
                .value()
                .iter()
                .any(|path| path.as_os_str().is_empty())
        {
            emit_torii_config_error(
                emitter,
                "torii.transport.https.certificate_chain requires one to four nonempty DER paths",
            );
        }
        let Some(private_key) = self.private_key else {
            emit_torii_config_error(emitter, "torii.transport.https.private_key is required");
            return None;
        };
        if private_key.value().as_os_str().is_empty() {
            emit_torii_config_error(
                emitter,
                "torii.transport.https.private_key must be nonempty",
            );
        }
        Some(actual::ToriiHttpsTransport {
            address,
            certificate_chain: self
                .certificate_chain
                .value()
                .iter()
                .map(|path| {
                    WithOrigin::new(path, self.certificate_chain.origin().clone())
                        .resolve_relative_path()
                })
                .collect(),
            private_key: private_key.resolve_relative_path(),
            handshake_timeout: timeout,
        })
    }
}
