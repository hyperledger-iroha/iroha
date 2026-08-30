// Runtime transaction-history visibility and authentication configuration.
/// Transaction-history visibility/auth configuration for Torii app API.
#[derive(Debug, Clone)]
pub struct ToriiTxHistory {
    /// Optional dataspace-keyed mandatory-alias policy file.
    pub mandatory_aliases_path: Option<PathBuf>,
    /// Maximum bytes accepted from the mandatory-alias policy file.
    pub mandatory_aliases_max_file_bytes: usize,
    /// Optional asset-definition restriction applied to visible-history endpoints.
    ///
    /// This may be either a canonical Base58 asset definition identifier or an
    /// on-chain asset alias that must be resolved against world state.
    pub allowed_asset_definition_id: Option<String>,
    /// Optional JWT bearer verification configuration for wallet history reads.
    pub jwt: Option<ToriiTxHistoryJwt>,
}
/// JWT bearer verification inputs for transaction-history endpoints.
#[derive(Clone)]
pub struct ToriiTxHistoryJwt {
    /// Expected JWT algorithm label (for example `RS256` or `HS256`).
    pub algorithm: String,
    /// Shared-secret material used for HMAC JWT algorithms.
    pub secret: Option<String>,
    /// PEM-encoded public key used for asymmetric JWT algorithms.
    pub public_key_pem: Option<String>,
    /// Optional issuer constraint.
    pub issuer: Option<String>,
    /// Optional audience constraint.
    pub audience: Option<String>,
}
impl std::fmt::Debug for ToriiTxHistoryJwt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToriiTxHistoryJwt")
            .field("algorithm", &self.algorithm)
            .field(
                "secret",
                &self
                    .secret
                    .as_ref()
                    .map(|_| "[REDACTED transaction-history JWT secret]"),
            )
            .field(
                "public_key_pem",
                &self
                    .public_key_pem
                    .as_ref()
                    .map(|_| "[REDACTED transaction-history JWT key]"),
            )
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .finish()
    }
}

#[cfg(test)]
mod torii_tx_history_actual_tests {
    use super::*;

    #[test]
    fn jwt_debug_redacts_key_material() {
        let jwt = ToriiTxHistoryJwt {
            algorithm: "HS256".to_owned(),
            secret: Some("do-not-log-this-secret".to_owned()),
            public_key_pem: Some("do-not-log-this-key".to_owned()),
            issuer: Some("issuer".to_owned()),
            audience: Some("audience".to_owned()),
        };
        let debug = format!("{jwt:?}");
        assert!(debug.contains("REDACTED"), "{debug}");
        assert!(!debug.contains("do-not-log-this-secret"), "{debug}");
        assert!(!debug.contains("do-not-log-this-key"), "{debug}");
    }
}
