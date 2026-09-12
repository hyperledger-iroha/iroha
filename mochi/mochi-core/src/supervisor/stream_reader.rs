//! Account authority for reading a validated local generation's ledger streams.

use super::*;

impl Supervisor {
    /// Construct the ledger reader belonging to this selected genesis generation.
    ///
    /// Kagami grants this exact genesis account `CanReadAllLedgerData`. The
    /// immutable SDK context retains its endpoint, network and authority across
    /// reconnects. A replaced generation or revoked permission cannot select a
    /// different account implicitly.
    ///
    /// # Errors
    /// Returns an error for an unknown peer, stale or invalid generation, or
    /// invalid SDK context. Validation occurs under the generation selection lease.
    pub fn stream_reader(&self, alias: &str) -> Result<iroha::client::AccountClient> {
        let peer = self
            .peers
            .iter()
            .find(|peer| peer.alias() == alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        let (_selected, _selection_lease) = self.selected_generation_with_lease()?;
        let configuration = iroha::config::Config {
            chain: self.chain_id.parse().map_err(|error| {
                SupervisorError::Config(format!("invalid chain label: {error}"))
            })?,
            network_id: self.network_id()?,
            account: AccountId::new(self.genesis.public_key().clone()),
            account_chain_discriminant: self.genesis.chain_discriminant,
            key_pair: self.genesis.key_pair.clone(),
            basic_auth: None,
            torii_api_url: peer.spec.torii_base_http().parse().map_err(|error| {
                SupervisorError::Config(format!("invalid managed Torii endpoint: {error}"))
            })?,
            torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
            transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
            transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
            sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
            sorafs_anonymity_policy: Default::default(),
            sorafs_rollout_phase: Default::default(),
        };
        let client = iroha::client::Client::builder(configuration)
            .build()
            .map_err(|error| SupervisorError::Config(format!("invalid stream reader: {error}")))?;
        client
            .account_client()
            .map_err(|error| SupervisorError::Config(format!("invalid stream authority: {error}")))
    }
}
