//! Lazy registry discovery from an explicitly selected parent wallet and fresh native state.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use iroha::{client::Client, config::Config};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    sorafs::{
        capacity::ProviderId,
        provider_admission::discovery::account_read::VerifiedAccountReadProviderV1,
    },
    sumeragi_finality::VerifiedSumeragiBlock,
};

use super::{AuthenticatedBootstrap, BootstrapError, ParentFinalityStore, Result};
use crate::verify::finality::FinalitySource;

impl ParentFinalityStore {
    /// Authenticate a cold build download's provider policy and token signer at fresh parent state.
    ///
    /// The caller supplies a separately resolved public parent wallet. A private child context
    /// cannot choose its registry, forward its listener token or supply a checkpoint. Call this
    /// lazily from the shared archive fetcher's typed discovery callback; warm cache hits need
    /// no parent I/O. The exclusive checkpoint owner must remain held for the complete call.
    ///
    /// # Errors
    /// Missing signed registry policy, stale release, changed parent identity, credentials,
    /// unapproved endpoint, missing fresh quorum, expired deadline or invalid provider evidence.
    pub fn discover_build_provider(
        &mut self,
        bootstrap: &AuthenticatedBootstrap,
        config: &Config,
        provider: ProviderId,
        deadline: Instant,
    ) -> Result<VerifiedAccountReadProviderV1> {
        let deadline = self.registry_deadline(bootstrap, config, provider, deadline, unix_ms()?)?;
        let source = bootstrap
            .parent_http_source(config, deadline)
            .map_err(|_| {
                BootstrapError::Invalid("cannot select public registry parent endpoints")
            })?;
        let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        let client = Client::builder(config.clone())
            .build()
            .map_err(|_| BootstrapError::Invalid("cannot construct public registry context"))?
            .with_request_deadline(deadline);
        self.discover_build_provider_with(bootstrap, config, provider, deadline, &source, |block| {
            client.get_account_read_provider_discovery(
                provider, bootstrap.release().native_world_schema, block, unix_ms()?,
            ).map_err(|_| BootstrapError::Invalid("cannot authenticate current build provider; retry from fresh parent quorum"))
        })
    }

    fn discover_build_provider_with<S: FinalitySource + ?Sized>(
        &mut self,
        bootstrap: &AuthenticatedBootstrap,
        config: &Config,
        provider: ProviderId,
        deadline: Instant,
        source: &S,
        read: impl FnOnce(&VerifiedSumeragiBlock) -> Result<VerifiedAccountReadProviderV1>,
    ) -> Result<VerifiedAccountReadProviderV1> {
        self.registry_deadline(bootstrap, config, provider, deadline, unix_ms()?)?;
        self.observe(source, &rand::random())?;
        let block = self.verifier().verified_tip()?;
        let verified = read(&block)?;
        self.registry_deadline(bootstrap, config, provider, deadline, unix_ms()?)?;
        if verified.chain_id() != bootstrap.release().chain_id
            || verified.discovery().network_id() != config.network_id
            || verified.discovery().height() != block.height()
            || verified.discovery().context_id() != block.context_id()
            || verified.discovery().admission().provider_id() != provider.as_bytes()
        {
            return Err(BootstrapError::Invalid(
                "build provider differs from selected parent decision",
            ));
        }
        Ok(verified)
    }

    fn registry_deadline(
        &self,
        bootstrap: &AuthenticatedBootstrap,
        config: &Config,
        provider: ProviderId,
        deadline: Instant,
        now_ms: u64,
    ) -> Result<Instant> {
        let release = bootstrap.release();
        let registry = release
            .build_registry
            .as_ref()
            .ok_or(BootstrapError::Invalid(
                "signed network release has no build registry",
            ))?;
        if self.network_name() != release.network_name
            || self.generation() != release.generation
            || self.verifier().checkpoint().network_id() != release.network_id
            || self.verifier().checkpoint().chain_id() != release.chain_id
            || config.network_id != release.network_id
            || config.chain.to_string() != release.chain_id
            || config.account_chain_discriminant != release.account_chain_discriminant
            || config.api_token.is_some()
            || config.basic_auth.is_some()
            || !registry
                .torii_roots
                .iter()
                .any(|root| root == config.torii_api_url.as_str())
            || provider.as_bytes() == &[0; 32]
        {
            return Err(BootstrapError::Invalid(
                "build registry requires its exact public parent context",
            ));
        }
        if now_ms < release.issued_at_ms
            || now_ms >= release.expires_at_ms
            || Instant::now() >= deadline
        {
            return Err(BootstrapError::Invalid(
                "build registry release or request deadline expired",
            ));
        }
        let expires = Instant::now()
            .checked_add(Duration::from_millis(release.expires_at_ms - now_ms))
            .ok_or(BootstrapError::Invalid("build registry deadline overflow"))?;
        Ok(deadline.min(expires))
    }
}

fn unix_ms() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| value.as_millis().try_into().ok())
        .ok_or(BootstrapError::Invalid(
            "build registry clock is unavailable",
        ))
}

#[cfg(test)]
mod tests;
