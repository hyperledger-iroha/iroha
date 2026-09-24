/// Asset restriction for the signed transaction-history feed.
#[derive(Debug, Clone)]
pub struct ToriiTxHistory {
    /// A canonical Base58 asset definition id or an on-chain asset alias.
    pub allowed_asset_definition_id: Option<String>,
}
