//! Alias management service primitives.
//!
//! This module wires the data-model types into a runtime friendly storage
//! container. The full VOPRF/STARK pipeline is tracked in roadmap notes to keep
//! incremental progress manageable.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, RwLock},
};

use iroha_crypto::{
    HashOf, KeyPair, Signature,
    blake2::{Blake2b512, Digest as _},
};
use iroha_data_model::{
    account::{AccountId, rekey::AccountAlias},
    alias::{
        AliasAttestation, AliasEvent, AliasIndex, AliasRecord, AliasRecordedEvent, AliasTarget,
    },
    domain::DomainId,
    name::Name,
    nexus::DataSpaceId,
    permission::Permission,
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias,
};
use iroha_telemetry::metrics::Metrics;
use mv::storage::StorageReadOnly;
use thiserror::Error;
use tracing::{Level, event, instrument};

use crate::state::WorldReadOnly;

const MOCK_VOPRF_DOMAIN: &[u8] = b"iroha.alias.voprf.mock.v1";
const ALIAS_ATTESTATION_SIGNATURE_DOMAIN: &[u8] = b"iroha:alias:attestation:v1";
const MAX_VOPRF_INPUT_BYTES: usize = 4096;

fn alias_attestation_signature_preimage(record: &AliasRecord, attester: &AccountId) -> Vec<u8> {
    let attester_bytes = norito::to_bytes(attester).expect("AccountId must encode");
    let record_hash = HashOf::<AliasRecord>::new(record);
    let attester_len = u64::try_from(attester_bytes.len()).expect("attester encoding length fits");
    let mut preimage = Vec::with_capacity(
        ALIAS_ATTESTATION_SIGNATURE_DOMAIN.len()
            + std::mem::size_of::<u64>()
            + attester_bytes.len()
            + record_hash.as_ref().len(),
    );
    preimage.extend_from_slice(ALIAS_ATTESTATION_SIGNATURE_DOMAIN);
    preimage.extend_from_slice(&attester_len.to_be_bytes());
    preimage.extend_from_slice(&attester_bytes);
    preimage.extend_from_slice(record_hash.as_ref());
    preimage
}

/// Signer used to attest alias storage updates.
#[derive(Clone, Debug)]
pub struct AliasAttester {
    account_id: AccountId,
    key_pair: KeyPair,
}

impl AliasAttester {
    /// Build an alias attester from a checked key pair.
    #[must_use]
    pub fn new(key_pair: KeyPair) -> Self {
        let account_id = AccountId::new(key_pair.public_key().clone());
        Self {
            account_id,
            key_pair,
        }
    }

    /// Account identity that will be recorded as the attester.
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    fn sign_record(&self, record: &AliasRecord) -> Result<AliasAttestation, AliasError> {
        let preimage = alias_attestation_signature_preimage(record, &self.account_id);
        let signature = Signature::try_new(self.key_pair.private_key(), &preimage)
            .map_err(|err| AliasError::Signing(err.to_string()))?;
        Ok(AliasAttestation::new(
            record.alias.clone(),
            self.account_id.clone(),
            signature,
            ALIAS_ATTESTATION_SIGNATURE_DOMAIN.to_vec(),
        ))
    }
}

/// Verify that `attestation` signs the canonical alias-record preimage.
///
/// # Errors
/// Returns [`AliasError::InvalidAttestation`] for mismatched fields, unsupported
/// attester identities, or signature verification failures.
pub fn verify_alias_attestation(
    record: &AliasRecord,
    attestation: &AliasAttestation,
) -> Result<(), AliasError> {
    if attestation.alias != record.alias {
        return Err(AliasError::InvalidAttestation("alias mismatch"));
    }
    if attestation.context != ALIAS_ATTESTATION_SIGNATURE_DOMAIN {
        return Err(AliasError::InvalidAttestation("unsupported context"));
    }
    let Some(public_key) = attestation.attester.try_signatory() else {
        return Err(AliasError::InvalidAttestation(
            "attester must use a single signatory",
        ));
    };
    let preimage = alias_attestation_signature_preimage(record, &attestation.attester);
    attestation
        .signature
        .verify(public_key, &preimage)
        .map_err(|_| AliasError::InvalidAttestation("signature verification failed"))
}

fn authority_has_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    target: &Permission,
) -> bool {
    match world.account_permissions_iter(authority) {
        Ok(permissions) => {
            let direct_match = permissions
                .into_iter()
                .any(|permission| permission == target);
            if direct_match {
                return true;
            }
        }
        Err(error) => {
            iroha_logger::warn!(
                authority = %authority,
                target_name = %target.name(),
                target_payload = %target.payload().get(),
                ?error,
                "alias permission lookup could not load authority permissions"
            );
        }
    }

    if world.account_roles_iter(authority).any(|role_id| {
        world
            .roles()
            .get(role_id)
            .is_some_and(|role| role.permissions.contains(target))
    }) {
        return true;
    }

    let direct_permissions: Vec<String> = world
        .account_permissions()
        .get(authority)
        .map(|permissions| {
            permissions
                .iter()
                .map(|permission| format!("{}:{}", permission.name(), permission.payload().get()))
                .collect()
        })
        .unwrap_or_default();
    let roles: Vec<String> = world
        .account_roles_iter(authority)
        .map(|role_id| role_id.to_string())
        .collect();
    let account_present = world.accounts().get(authority).is_some();
    iroha_logger::warn!(
        authority = %authority,
        account_present,
        target_name = %target.name(),
        target_payload = %target.payload().get(),
        direct_permissions = ?direct_permissions,
        roles = ?roles,
        "alias permission lookup denied authority"
    );
    false
}

fn account_alias_is_open_retail_namespace(
    world: &impl WorldReadOnly,
    alias: &AccountAlias,
) -> bool {
    let Some(dataspace) = world.dataspace_catalog().by_id(alias.dataspace) else {
        return false;
    };
    let dataspace_alias = dataspace.alias.as_str();
    let domain_alias = alias.domain.as_ref().map(|domain| domain.name().as_ref());
    matches!(
        (dataspace_alias, domain_alias),
        ("paynet", None) | ("paynet", Some("hbl" | "ubl")) | ("cbuae", None)
    )
}

fn authority_controls_open_retail_account_alias(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    alias: &AccountAlias,
) -> bool {
    if !account_alias_is_open_retail_namespace(world, alias) {
        return false;
    }
    if world
        .account_aliases()
        .get(alias)
        .is_some_and(|account_id| account_id == authority)
    {
        return true;
    }
    world
        .account_rekey_records()
        .get(alias)
        .is_some_and(|record| &record.active_account_id == authority)
}

/// Return `true` when the authority holds the exact permissions required to resolve `alias`.
pub fn authority_can_resolve_account_alias(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    alias: &AccountAlias,
) -> bool {
    let dataspace_permission: Permission = CanResolveAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(alias.dataspace),
    }
    .into();
    if !authority_has_permission(world, authority, &dataspace_permission) {
        return false;
    }

    match alias.domain_id(world.dataspace_catalog()) {
        Ok(Some(domain_id)) => {
            let domain_permission: Permission = CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(domain_id),
            }
            .into();
            authority_has_permission(world, authority, &domain_permission)
        }
        Ok(None) => true,
        Err(_) => false,
    }
}

/// Return `true` when the authority holds the exact permissions required to mutate `alias`.
pub fn authority_can_manage_account_alias(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    alias: &AccountAlias,
) -> bool {
    if authority_controls_open_retail_account_alias(world, authority, alias) {
        return true;
    }

    match alias.domain_id(world.dataspace_catalog()) {
        Ok(domain_id) => authority_can_manage_account_alias_scope(
            world,
            authority,
            alias.dataspace,
            domain_id.as_ref(),
        ),
        Err(_) => false,
    }
}

/// Return `true` when `authority` holds account-alias management permission for an explicit
/// dataspace/domain scope.
///
/// This variant remains usable while a dynamic dataspace alias is inactive, allowing a stale
/// binding to be cleared without trusting caller-supplied namespace metadata.
pub fn authority_can_manage_account_alias_scope(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    dataspace: DataSpaceId,
    domain: Option<&DomainId>,
) -> bool {
    let dataspace_permission: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(dataspace),
    }
    .into();
    if !authority_has_permission(world, authority, &dataspace_permission) {
        return false;
    }

    let Some(domain_id) = domain else {
        return true;
    };
    let domain_permission: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Domain(domain_id.clone()),
    }
    .into();
    authority_has_permission(world, authority, &domain_permission)
}

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
#[derive(Clone)]
pub struct AliasStorage {
    inner: Arc<RwLock<BTreeMap<Name, AliasRecord>>>,
    index: Arc<RwLock<BTreeMap<AliasIndex, Name>>>,
    attester: AliasAttester,
    metrics: Option<Arc<Metrics>>,
}

impl fmt::Debug for AliasStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let alias_count = self.inner.read().map(|map| map.len()).unwrap_or(0);
        let index_count = self.index.read().map(|map| map.len()).unwrap_or(0);
        f.debug_struct("AliasStorage")
            .field("alias_count", &alias_count)
            .field("index_count", &index_count)
            .field("attester", &self.attester.account_id)
            .field("metrics_attached", &self.metrics.is_some())
            .finish()
    }
}

impl AliasStorage {
    /// Create an empty storage instance backed by `attester`.
    #[must_use]
    pub fn new(attester: AliasAttester) -> Self {
        Self {
            inner: Arc::default(),
            index: Arc::default(),
            attester,
            metrics: None,
        }
    }

    /// Create storage wired to the shared telemetry metrics registry.
    #[must_use]
    pub fn with_metrics(attester: AliasAttester, metrics: Arc<Metrics>) -> Self {
        let mut storage = Self::new(attester);
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
        let attestation = self.attester.sign_record(&record)?;
        let index = record.index;
        let alias = record.alias.clone();
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
            attestation,
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
    /// Alias attestation signing failed.
    #[error("alias attestation signing failed: {0}")]
    Signing(String),
    /// Alias attestation failed verification.
    #[error("alias attestation invalid: {0}")]
    InvalidAttestation(&'static str),
}

/// Helper builder for CLI/SDK wiring. Keeps operations explicit.
#[derive(Debug)]
pub struct AliasService {
    storage: AliasStorage,
}

impl AliasService {
    /// Construct service with empty storage backed by `attester`.
    #[must_use]
    pub fn new(attester: AliasAttester) -> Self {
        Self {
            storage: AliasStorage::new(attester),
        }
    }

    /// Construct service with metrics instrumentation attached.
    #[must_use]
    pub fn with_metrics(attester: AliasAttester, metrics: Arc<Metrics>) -> Self {
        Self {
            storage: AliasStorage::with_metrics(attester, metrics),
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

    use iroha_crypto::Algorithm;
    use iroha_data_model::{account::AccountId, alias::AliasIndex, name::Name};

    use super::*;

    fn owner() -> AccountId {
        const SIGNATORY: &str =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
        AccountId::new(SIGNATORY.parse().expect("public key"))
    }

    fn alias_attester(seed: u8) -> AliasAttester {
        AliasAttester::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive checked alias attester fixture key"),
        )
    }

    fn alias_service() -> AliasService {
        AliasService::new(alias_attester(0xA1))
    }

    fn alias_storage() -> AliasStorage {
        AliasStorage::new(alias_attester(0xA2))
    }

    #[test]
    fn storage_roundtrip() {
        let service = alias_service();
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
                let signature = payload.attestation.signature.payload();
                assert!(!signature.is_empty());
                assert!(!signature.iter().all(|byte| *byte == 0));
                verify_alias_attestation(&payload.record, &payload.attestation)
                    .expect("storage event attestation verifies");
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
        let service = alias_service();
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
        let storage = alias_storage();
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
        let storage = alias_storage();
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
        let service = alias_service();
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
        let storage = alias_storage();
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
        let storage = AliasStorage::with_metrics(alias_attester(0xA3), Arc::clone(&metrics));
        let alias = Name::from_str("usage").expect("valid");

        storage.emit_metrics(&alias, "global", AliasMetricKind::Resolve);

        let counter = metrics
            .alias_usage_total
            .with_label_values(&["global", AliasMetricKind::Resolve.as_label()])
            .get();
        assert_eq!(counter, 1);
    }

    #[test]
    fn verify_alias_attestation_rejects_tampered_record() {
        let storage = alias_storage();
        let alias = Name::from_str("signedalias").expect("valid");
        let record = AliasRecord::new(
            alias,
            owner(),
            AliasTarget::Custom(vec![1, 2, 3]),
            AliasIndex(9),
        );
        let AliasEvent::Recorded(event) = storage.put(record).expect("put signs") else {
            panic!("unexpected event");
        };
        verify_alias_attestation(&event.record, &event.attestation).expect("valid attestation");

        let mut tampered = event.record.clone();
        tampered.index = AliasIndex(10);
        let err = verify_alias_attestation(&tampered, &event.attestation)
            .expect_err("tampered record must not verify");
        assert!(matches!(
            err,
            AliasError::InvalidAttestation("signature verification failed")
        ));
    }
}
