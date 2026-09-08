//! Catalog-bound assembly of native HTTPS leaves for the existing authenticated source broker.
//!
//! This constructor supplies no governance, credentials, readiness, or publisher seed. The
//! deployment's backend registry must explicitly provide independently governed grant resolvers.
use super::{VerifiedProviderIngestPayloadV1, https_source::*};
use crate::runtime_provider_registry::{
    IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderSlotV1,
};
use sorafs_node::provider_ingest_runtime::{
    ProviderIngestAuthenticatedSourcePoolV1, ProviderIngestAuthenticatedSourceRegistrationV1,
    ProviderIngestRuntimeProviderQualificationV1,
};
use std::{collections::BTreeSet, fmt, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// One explicitly configured HTTPS source and its independently administered authority.
#[derive(Clone)]
pub struct ProviderIngestHttpsSourceRegistrationV1 {
    /// Public network, identity pins, bounds and deadlines; no endpoint or credential.
    pub config: ProviderIngestHttpsSourceConfigV1,
    /// Mandatory live governance, admitted-advert and authenticated grant service.
    pub resolver: Arc<dyn ProviderIngestGovernedHttpsGrantResolverV1>,
}
impl fmt::Debug for ProviderIngestHttpsSourceRegistrationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestHttpsSourceRegistrationV1")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Payload-free failure to compose the native source backend for a public broker catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderIngestHttpsCompositionErrorV1 {
    /// The catalog lacks exactly one completely qualified authenticated-source role.
    CatalogMismatch,
    /// Public source configuration violates the catalog, identity uniqueness or native bounds.
    InvalidSources,
    /// An injected resolver refused or changed its independently pinned source qualification.
    SourceUnavailable,
    /// A source backend is already installed and cannot be silently replaced.
    AlreadyInstalled,
}
impl fmt::Display for ProviderIngestHttpsCompositionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CatalogMismatch => "provider-ingest HTTPS source catalog mismatch",
            Self::InvalidSources => "provider-ingest HTTPS source policy mismatch",
            Self::SourceUnavailable => "provider-ingest HTTPS source qualification unavailable",
            Self::AlreadyInstalled => "provider-ingest authenticated source already installed",
        })
    }
}
impl std::error::Error for ProviderIngestHttpsCompositionErrorV1 {}

/// Compose real HTTPS leaves into the native authenticated source pool for this exact catalog.
///
/// The deployment registry owns the source policy inventory and its catalog digest. Construction
/// validates every public policy before consulting any resolver, then checks live qualifications.
/// It never resolves a stream grant, checks readiness, opens a socket or alters storage.
///
/// All sources must specify the same `max_in_flight`: this is one shared pool budget, retained
/// across DNS work, network acquisition and payload-reader lifetime. The source payload ceiling
/// may be lower than the catalog's storage-capacity ceiling; requests above it fail closed.
///
/// # Errors
///
/// Rejects absent/invalid catalog roles, mismatched networks, duplicate identities/handles,
/// invalid/excessive bounds, or any refused source qualification. Broker startup still performs
/// its existing independent authenticated readiness checks after successful construction.
pub fn compose_provider_ingest_https_pool_v1(
    catalog: &IrohaRuntimeProviderBindingsV1,
    sources: Vec<ProviderIngestHttpsSourceRegistrationV1>,
) -> Result<
    ProviderIngestAuthenticatedSourcePoolV1<VerifiedProviderIngestPayloadV1>,
    ProviderIngestHttpsCompositionErrorV1,
> {
    use ProviderIngestHttpsCompositionErrorV1::{
        CatalogMismatch, InvalidSources, SourceUnavailable,
    };
    let mut matches = catalog.iter().filter(|binding| {
        binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource
    });
    let binding = matches.next().ok_or(CatalogMismatch)?;
    if matches.next().is_some() {
        return Err(CatalogMismatch);
    }
    let limits = binding
        .provider_ingest_source_limits()
        .ok_or(CatalogMismatch)?;
    let qualification = ProviderIngestRuntimeProviderQualificationV1::new(
        binding.revision().ok_or(CatalogMismatch)?,
        binding.policy_digest().ok_or(CatalogMismatch)?,
    );
    if !qualification.is_valid()
        || sources.len() < 2
        || sources.len() > limits.max_source_providers as usize
    {
        return Err(InvalidSources);
    }
    let concurrency = sources[0].config.max_in_flight;
    let mut provider_ids = BTreeSet::new();
    let mut handles = BTreeSet::new();
    for source in &sources {
        let config = &source.config;
        if config.validate().is_err()
            || config.network_id != *catalog.network_id()
            || config.binding.runtime_handle == binding.handle()
            || config.operation_timeout > Duration::from_millis(limits.operation_timeout_ms)
            || config.limits.max_payload_bytes > limits.max_content_bytes
            || config.max_in_flight != concurrency
            || concurrency > limits.max_concurrent_streams as usize
            || !provider_ids.insert(config.binding.provider_id)
            || !handles.insert(config.binding.runtime_handle.as_str())
        {
            return Err(InvalidSources);
        }
    }
    let admissions = Arc::new(Semaphore::new(concurrency));
    let mut registrations = Vec::with_capacity(sources.len());
    for source in sources {
        let binding = source.config.binding.clone();
        let leaf = ProviderIngestHttpsSourceV1::new_with_admissions(
            source.config,
            source.resolver,
            Arc::clone(&admissions),
        )
        .map_err(|_| SourceUnavailable)?;
        registrations.push(ProviderIngestAuthenticatedSourceRegistrationV1::new(
            binding,
            Arc::new(leaf),
        ));
    }
    ProviderIngestAuthenticatedSourcePoolV1::new(
        binding.handle(),
        qualification,
        limits.max_source_providers as usize,
        registrations,
    )
    .map_err(|_| SourceUnavailable)
}

#[cfg(test)]
mod tests;
