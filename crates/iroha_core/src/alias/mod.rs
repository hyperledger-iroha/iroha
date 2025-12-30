//! Alias management service primitives.
//!
//! This module wires the data-model types into a runtime friendly storage
//! container. The full VOPRF/STARK pipeline is tracked in roadmap notes to keep
//! incremental progress manageable.

use std::{
    collections::BTreeMap,
    fmt,
    str::FromStr,
    sync::{Arc, RwLock},
};

use iroha_crypto::{
    HashOf, Signature,
    blake2::{Blake2b512, Digest as _},
};
use iroha_data_model::{
    account,
    alias::{
        AliasAttestation, AliasEvent, AliasIndex, AliasRecord, AliasRecordedEvent, AliasTarget,
    },
    name::Name,
};
use iroha_telemetry::metrics::Metrics;
use thiserror::Error;
use tracing::{Level, event, instrument};

const MOCK_VOPRF_DOMAIN: &[u8] = b"iroha.alias.voprf.mock.v1";
const MAX_VOPRF_INPUT_BYTES: usize = 4096;

/// Supported alias VOPRF backend (placeholder).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoprfBackend {
    /// Blake2b-based deterministic mock evaluator.
    Blake2b512Mock,
}

impl VoprfBackend {
    /// Stable backend identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blake2b512Mock => "blake2b512-mock",
        }
    }
}

/// Result of an alias VOPRF evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoprfEvaluation {
    /// Backend that produced the result.
    pub backend: VoprfBackend,
    /// Evaluated (unblinded) element bytes.
    pub evaluated_element: Vec<u8>,
}

/// Evaluate the mock alias VOPRF used by the current pipeline.
///
/// This is a deterministic placeholder that domain-separates inputs with
/// `iroha.alias.voprf.mock.v1` and hashes them with BLAKE2b-512.
///
/// # Errors
///
/// Returns [`AliasError::Voprf`] when the blinded element is empty or exceeds the maximum length.
pub fn evaluate_alias_voprf(blinded: &[u8]) -> Result<VoprfEvaluation, AliasError> {
    if blinded.is_empty() {
        return Err(AliasError::Voprf("blinded element must not be empty"));
    }
    if blinded.len() > MAX_VOPRF_INPUT_BYTES {
        return Err(AliasError::Voprf("blinded element exceeds maximum length"));
    }

    let mut hasher = Blake2b512::new();
    hasher.update(MOCK_VOPRF_DOMAIN);
    hasher.update(blinded);
    let evaluated_element = hasher.finalize().to_vec();
    Ok(VoprfEvaluation {
        backend: VoprfBackend::Blake2b512Mock,
        evaluated_element,
    })
}

/// Metric categories emitted by the alias service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasMetricKind {
    /// Tracks alias resolution operations for telemetry emission.
    Resolve,
}

impl AliasMetricKind {
    const fn as_label(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
        }
    }
}

/// Alias storage backed by a Merkle-friendly map.
#[derive(Clone, Default)]
pub struct AliasStorage {
    inner: Arc<RwLock<BTreeMap<Name, AliasRecord>>>,
    index: Arc<RwLock<BTreeMap<AliasIndex, Name>>>,
    metrics: Option<Arc<Metrics>>,
}

impl fmt::Debug for AliasStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let alias_count = self.inner.read().map(|map| map.len()).unwrap_or(0);
        let index_count = self.index.read().map(|map| map.len()).unwrap_or(0);
        f.debug_struct("AliasStorage")
            .field("alias_count", &alias_count)
            .field("index_count", &index_count)
            .field("metrics_attached", &self.metrics.is_some())
            .finish()
    }
}

impl AliasStorage {
    /// Create an empty storage instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create storage wired to the shared telemetry metrics registry.
    #[must_use]
    pub fn with_metrics(metrics: Arc<Metrics>) -> Self {
        let mut storage = Self::new();
        storage.metrics = Some(metrics);
        storage
    }

    /// Attach telemetry metrics to an existing storage instance.
    pub fn set_metrics(&mut self, metrics: Arc<Metrics>) {
        self.metrics = Some(metrics);
    }

    /// Insert or update an alias record.
    ///
    /// # Errors
    /// Returns [`AliasError::Poison`] when the alias or index map lock is poisoned.
    #[instrument(skip(self))]
    pub fn put(&self, record: AliasRecord) -> Result<AliasEvent, AliasError> {
        let index = record.index;
        let alias = record.alias.clone();
        let owner = record.owner.clone();
        {
            let mut by_alias = self
                .inner
                .write()
                .map_err(|_| AliasError::Poison("alias"))?;
            by_alias.insert(alias.clone(), record.clone());
        }
        {
            let mut by_index = self
                .index
                .write()
                .map_err(|_| AliasError::Poison("index"))?;
            by_index.insert(index, alias.clone());
        }
        Ok(AliasEvent::Recorded(AliasRecordedEvent {
            record,
            attestation: AliasAttestation::new(
                alias,
                // Placeholder attester id; callers should override via `push_attestation`.
                owner,
                Signature::from_bytes(&[]),
                Vec::new(),
            ),
        }))
    }

    /// Resolve alias by name.
    ///
    /// # Errors
    /// Returns [`AliasError::Poison`] if the alias map lock is poisoned.
    pub fn resolve(&self, alias: &Name) -> Result<Option<AliasRecord>, AliasError> {
        let guard = self.inner.read().map_err(|_| AliasError::Poison("alias"))?;
        Ok(guard.get(alias).cloned())
    }

    /// Resolve alias by Merkle index.
    ///
    /// # Errors
    /// Returns [`AliasError::Poison`] if the alias or index map lock is poisoned.
    pub fn resolve_index(&self, index: AliasIndex) -> Result<Option<AliasRecord>, AliasError> {
        let alias = self
            .index
            .read()
            .map_err(|_| AliasError::Poison("index"))?
            .get(&index)
            .cloned();

        alias.map_or_else(|| Ok(None), |name| self.resolve(&name))
    }

    /// Apply the mock VOPRF hash used by the current alias pipeline.
    ///
    /// # Errors
    ///
    /// Propagates [`AliasError::Voprf`] for invalid blinded inputs.
    pub fn voprf_evaluate(&self, blinded_element: &[u8]) -> Result<VoprfEvaluation, AliasError> {
        evaluate_alias_voprf(blinded_element)
    }

    /// Record a Merkle attestation hash for an alias if present.
    ///
    /// # Errors
    /// Returns [`AliasError::Poison`] if the alias map lock is poisoned or
    /// [`AliasError::NotFound`] if the alias is unknown.
    pub fn push_attestation(
        &self,
        alias: &Name,
        hash: HashOf<AliasAttestation>,
    ) -> Result<(), AliasError> {
        let mut guard = self
            .inner
            .write()
            .map_err(|_| AliasError::Poison("alias"))?;
        let record = guard
            .get_mut(alias)
            .ok_or_else(|| AliasError::NotFound(alias.clone()))?;
        record.push_attestation(hash);
        Ok(())
    }

    /// Emit telemetry for alias usage (lookup, attestation, etc.).
    pub fn emit_metrics(&self, alias: &Name, lane: &'static str, kind: AliasMetricKind) {
        if let Some(metrics) = &self.metrics {
            metrics
                .alias_usage_total
                .with_label_values(&[lane, kind.as_label()])
                .inc();
        }
        event!(
            Level::INFO,
            alias = %alias.as_ref(),
            lane,
            event = kind.as_label(),
            data_source = "ds_placeholder",
            "alias_usage"
        );
    }

    /// Emit an audit log entry capturing attester signature material.
    pub fn audit_attestation(&self, alias: &Name, attestation: &AliasAttestation) {
        event!(
            Level::INFO,
            alias = %alias.as_ref(),
            attester = %attestation.attester,
            signature_len = attestation.signature.payload().len(),
            "alias_attestation_recorded"
        );
    }
}

/// Errors returned by alias operations.
#[derive(Debug, Error)]
pub enum AliasError {
    /// Provided alias was not found.
    #[error("alias not found: {0}")]
    NotFound(Name),
    /// Storage lock poisoned.
    #[error("alias storage poisoned: {0}")]
    Poison(&'static str),
    /// Alias VOPRF input failed validation.
    #[error("alias voprf error: {0}")]
    Voprf(&'static str),
}

/// Helper builder for CLI/SDK wiring. Keeps operations explicit.
#[derive(Debug, Default)]
pub struct AliasService {
    storage: AliasStorage,
}

impl AliasService {
    /// Construct service with empty storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: AliasStorage::new(),
        }
    }

    /// Construct service with metrics instrumentation attached.
    #[must_use]
    pub fn with_metrics(metrics: Arc<Metrics>) -> Self {
        Self {
            storage: AliasStorage::with_metrics(metrics),
        }
    }

    /// Access storage for read/write operations.
    pub fn storage(&self) -> &AliasStorage {
        &self.storage
    }

    /// Attach metrics instrumentation to the service storage.
    pub fn set_metrics(&mut self, metrics: Arc<Metrics>) {
        self.storage.set_metrics(metrics);
    }

    /// Install the global account alias resolver consulted when parsing account strings.
    pub fn install_account_alias_resolver(service: &Arc<Self>) {
        let resolver = Arc::clone(service);
        account::clear_account_alias_resolver();
        account::set_account_alias_resolver(Arc::new(move |label, domain| {
            let alias_name = match Name::from_str(label) {
                Ok(name) => name,
                Err(_) => return None,
            };
            let record = match resolver.storage.resolve(&alias_name) {
                Ok(Some(record)) => record,
                Ok(None) => return None,
                Err(err) => {
                    tracing::warn!(
                        alias = %alias_name.as_ref(),
                        ?err,
                        "failed to resolve alias while parsing account identifier"
                    );
                    return None;
                }
            };
            if let AliasTarget::Account(account_id) = &record.target {
                if account_id.domain() != domain {
                    tracing::warn!(
                        alias = %alias_name.as_ref(),
                        expected_domain = %domain,
                        resolved_domain = %account_id.domain(),
                        "alias resolved to a different domain"
                    );
                }
                Some(account_id.clone())
            } else {
                None
            }
        }));
    }

    /// Resolve alias to target, returning attestation hashes for auditing.
    ///
    /// # Errors
    /// Propagates [`AliasError::Poison`] from the storage backend and returns
    /// [`AliasError::NotFound`] when the alias is absent.
    pub fn resolve(
        &self,
        alias: &Name,
    ) -> Result<(AliasTarget, Vec<HashOf<AliasAttestation>>), AliasError> {
        let record = self
            .storage
            .resolve(alias)?
            .ok_or_else(|| AliasError::NotFound(alias.clone()))?;
        Ok((record.target, record.attestation_hashes))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        str::FromStr,
        sync::Arc,
    };

    use iroha_data_model::{account::AccountId, alias::AliasIndex, name::Name};

    use super::*;

    fn owner() -> AccountId {
        const SIGNATORY: &str =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
        AccountId::from_str(&format!("{SIGNATORY}@wonderland"))
            .expect("account identifier should parse")
    }

    #[test]
    fn storage_roundtrip() {
        let service = AliasService::new();
        let alias = Name::from_str("alias").expect("valid");
        let record = AliasRecord::new(
            alias.clone(),
            owner(),
            AliasTarget::Custom(vec![1, 2, 3]),
            AliasIndex(1),
        );
        let event = service
            .storage
            .put(record)
            .expect("put should succeed without poisoning");
        match event {
            AliasEvent::Recorded(payload) => {
                assert_eq!(payload.record.index, AliasIndex(1));
            }
            _ => panic!("unexpected event"),
        }
        let resolved = service
            .storage
            .resolve(&alias)
            .expect("lock not poisoned")
            .expect("alias present");
        assert_eq!(resolved.index, AliasIndex(1));
        let resolved_by_index = service
            .storage
            .resolve_index(AliasIndex(1))
            .expect("lock not poisoned")
            .expect("alias present");
        assert_eq!(resolved_by_index.alias, alias);
    }

    #[test]
    fn service_resolve_success() {
        let service = AliasService::new();
        let alias = Name::from_str("alice").expect("valid");
        let target = AliasTarget::Custom(vec![4, 5, 6]);
        let record = AliasRecord::new(alias.clone(), owner(), target.clone(), AliasIndex(2));
        let expected_attestations = record.attestation_hashes.clone();
        service.storage.put(record).expect("put should succeed");

        let (resolved_target, attestations) = service.resolve(&alias).expect("should resolve");
        assert_eq!(resolved_target, target);
        assert_eq!(attestations, expected_attestations);
    }

    #[test]
    fn put_returns_error_when_alias_lock_poisoned() {
        let storage = AliasStorage::new();
        let alias = Name::from_str("alias").expect("valid");
        let record = AliasRecord::new(
            alias.clone(),
            owner(),
            AliasTarget::Custom(vec![1, 2, 3]),
            AliasIndex(1),
        );

        let storage_clone = storage.clone();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = storage_clone
                .inner
                .write()
                .expect("poison setup should acquire alias lock");
            panic!("poison alias lock");
        }));

        let err = storage
            .put(record)
            .expect_err("alias lock poisoning should error");
        assert!(matches!(err, AliasError::Poison("alias")));
    }

    #[test]
    fn put_returns_error_when_index_lock_poisoned() {
        let storage = AliasStorage::new();
        let alias = Name::from_str("alias").expect("valid");
        let record = AliasRecord::new(
            alias.clone(),
            owner(),
            AliasTarget::Custom(vec![1, 2, 3]),
            AliasIndex(1),
        );

        // Pre-populate alias map so alias lock isn't poisoned.
        storage
            .inner
            .write()
            .expect("setup should not be poisoned")
            .insert(alias.clone(), record.clone());

        let storage_clone = storage.clone();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = storage_clone
                .index
                .write()
                .expect("poison setup should acquire index lock");
            panic!("poison index lock");
        }));

        let err = storage
            .put(record)
            .expect_err("index lock poisoning should error");
        assert!(matches!(err, AliasError::Poison("index")));
    }

    #[test]
    fn service_resolve_poisoned_lock() {
        let service = AliasService::new();
        let alias = Name::from_str("bob").expect("valid");

        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = service
                .storage
                .inner
                .write()
                .expect("lock should be available");
            panic!("poisoning alias storage");
        }));

        let err = service.resolve(&alias).expect_err("lock is poisoned");
        assert!(matches!(err, AliasError::Poison("alias")));
    }

    #[test]
    fn voprf_evaluate_matches_helper() {
        let storage = AliasStorage::new();
        let blinded = b"deadbeef";
        let expected = evaluate_alias_voprf(blinded).expect("evaluates");
        assert_eq!(
            storage.voprf_evaluate(blinded).expect("evaluates"),
            expected
        );
    }
    #[test]
    fn emit_metrics_records_usage_counter() {
        let metrics = Arc::new(Metrics::default());
        let storage = AliasStorage::with_metrics(Arc::clone(&metrics));
        let alias = Name::from_str("usage").expect("valid");

        storage.emit_metrics(&alias, "global", AliasMetricKind::Resolve);

        let counter = metrics
            .alias_usage_total
            .with_label_values(&["global", AliasMetricKind::Resolve.as_label()])
            .get();
        assert_eq!(counter, 1);
    }
}
