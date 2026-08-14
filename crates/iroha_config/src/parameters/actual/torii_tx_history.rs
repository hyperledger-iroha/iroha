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
#[derive(Debug, Clone)]
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
