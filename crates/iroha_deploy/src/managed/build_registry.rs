//! Explicit retained parent build registry for private developer environments.
//!
//! This owner keeps a separate advancing finality journal, independent of the live attachment
//! worker. It never derives a parent from the child's signer, network, or listener credential.

use super::{Error, InstalledRuntime, ManagedStore, Result};
use crate::bootstrap::{AuthenticatedBootstrap, BootstrapError, ParentFinalityStore};
use iroha::config::Config;
use iroha_data_model::sorafs::{
    capacity::ProviderId,
    provider_admission::discovery::account_read::VerifiedAccountReadProviderV1,
};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

/// Independently bound parent signer and exclusive lazy provider-discovery checkpoint owner.
#[derive(Clone)]
pub struct ManagedBuildRegistry {
    config: Config,
    bootstrap: Arc<AuthenticatedBootstrap>,
    finality: Arc<Mutex<ParentFinalityStore>>,
}
impl std::fmt::Debug for ManagedBuildRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ManagedBuildRegistry { retained parent context }")
    }
}
impl ManagedBuildRegistry {
    /// Exact parent-only registry client configuration; never the selected child configuration.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Reobserve the parent's quorum and authenticate provider policy/signer at that exact cut.
    /// # Errors
    /// Refuses a poisoned owner, expired deadline/release, unavailable quorum, or invalid evidence.
    pub fn discover(
        &self,
        provider: ProviderId,
        deadline: Instant,
    ) -> std::result::Result<VerifiedAccountReadProviderV1, BootstrapError> {
        if Instant::now() >= deadline {
            return Err(BootstrapError::Invalid("build registry deadline expired"));
        }
        self.finality
            .try_lock()
            .map_err(|error| match error {
                std::sync::TryLockError::WouldBlock => BootstrapError::Busy,
                std::sync::TryLockError::Poisoned(_) => {
                    BootstrapError::Invalid("build registry checkpoint owner is poisoned")
                }
            })?
            .discover_build_provider(&self.bootstrap, &self.config, provider, deadline)
    }
}
impl ManagedStore {
    /// Resolve an explicit signed parent registry for the retained developer environment.
    ///
    /// Absence means no retained remote binding or an explicitly absent signed registry policy.
    /// The separate registry checkpoint prevents contention with the live attachment worker.
    /// # Errors
    /// Invalid retained ownership, changed signed release, missing wallet custody, expired
    /// deadline, or unavailable exclusive registry checkpoint ownership.
    pub fn build_registry(
        &self,
        runtime: &InstalledRuntime,
        name: &str,
        deadline: Instant,
    ) -> Result<Option<ManagedBuildRegistry>> {
        let Some(context) = super::remote::build_registry_context(self, runtime, name, deadline)?
        else {
            return Ok(None);
        };
        let finality = ParentFinalityStore::open(&context.finality_path, &context.bootstrap)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(Some(ManagedBuildRegistry {
            config: context.parent,
            bootstrap: Arc::new(context.bootstrap),
            finality: Arc::new(Mutex::new(finality)),
        }))
    }
}
