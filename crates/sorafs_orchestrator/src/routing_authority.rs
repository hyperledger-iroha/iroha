//! Finalized-ledger routing authority projection and bounded cache.
//!
//! SFM-1 routing authority is a deterministic join of approved pin manifests
//! and completed replication orders. This module deliberately accepts only an
//! immutable finalized-state source: provider adverts can add current
//! connectivity details later, but cannot grant content authority.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
    },
};

use iroha_data_model::sorafs::pin_registry::{
    ManifestDigest, ManifestRootCid, PinManifestRecord, PinStatus, ReplicationOrderId,
    ReplicationOrderRecord, ReplicationOrderStatus,
};
use norito::{core::DecodeLimits, derive::NoritoSerialize};
use sorafs_manifest::capacity::{MAX_CAPACITY_METADATA_VALUE_BYTES, ReplicationOrderV1};
use thiserror::Error;

/// Canonical projection envelope version for the first-release SFM-1 join.
pub const ROUTING_AUTHORITY_PROJECTION_VERSION_V1: u8 = 1;

const MAX_AUTHORITY_MANIFESTS: usize = 65_536;
const MAX_AUTHORITY_ORDERS: usize = 65_536;
const MAX_AUTHORITY_PROVIDER_REFS: usize = 262_144;
const MAX_REPLICATION_ORDER_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_AUTHORITY_ORDER_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_AUTHORITY_PROJECTION_BYTES: usize = 16 * 1024 * 1024;

const REPLICATION_ORDER_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    MAX_CAPACITY_METADATA_VALUE_BYTES,
    MAX_REPLICATION_ORDER_PAYLOAD_BYTES,
    65_536,
    MAX_REPLICATION_ORDER_PAYLOAD_BYTES * 4,
    32,
);

#[derive(Debug, Clone, Copy)]
struct AuthorityJoinLimits {
    manifests: usize,
    orders: usize,
    provider_refs: usize,
    order_payload_bytes: usize,
    projection_bytes: usize,
}

impl AuthorityJoinLimits {
    const PRODUCTION: Self = Self {
        manifests: MAX_AUTHORITY_MANIFESTS,
        orders: MAX_AUTHORITY_ORDERS,
        provider_refs: MAX_AUTHORITY_PROVIDER_REFS,
        order_payload_bytes: MAX_AUTHORITY_ORDER_PAYLOAD_BYTES,
        projection_bytes: MAX_AUTHORITY_PROJECTION_BYTES,
    };
}

/// Exact identity of an immutable finalized ledger view.
///
/// The bootstrap identity is `(height = 0, block_hash = None)`. Every
/// committed height must carry the canonical 32-byte block hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize)]
pub struct FinalizedStateIdentityV1 {
    height: u64,
    block_hash: Option<[u8; 32]>,
}

impl FinalizedStateIdentityV1 {
    /// Construct and validate a finalized-state identity.
    ///
    /// # Errors
    ///
    /// Returns [`RoutingAuthorityError::InvalidFinalizedIdentity`] when a
    /// bootstrap view has a hash, a committed view has no hash, or a committed
    /// hash does not satisfy the canonical Iroha hash marker.
    pub fn new(height: u64, block_hash: Option<[u8; 32]>) -> Result<Self, RoutingAuthorityError> {
        let valid = match (height, block_hash) {
            (0, None) => true,
            (0, Some(_)) | (_, None) => false,
            (_, Some(hash)) => hash[31] & 1 == 1,
        };
        if !valid {
            return Err(RoutingAuthorityError::InvalidFinalizedIdentity);
        }
        Ok(Self { height, block_hash })
    }

    /// Finalized block height.
    #[must_use]
    pub const fn height(self) -> u64 {
        self.height
    }

    /// Finalized block hash, absent only before genesis is committed.
    #[must_use]
    pub const fn block_hash(self) -> Option<[u8; 32]> {
        self.block_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize)]
struct RoutingAuthorityRouteV1 {
    manifest_root_cid: ManifestRootCid,
    provider_ids: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize)]
struct RoutingAuthorityProjectionEnvelopeV1 {
    version: u8,
    finalized_state: FinalizedStateIdentityV1,
    routes: Vec<RoutingAuthorityRouteV1>,
}

/// Deterministic SFM-1 authority projection derived from finalized ledger state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingAuthorityProjection {
    identity: FinalizedStateIdentityV1,
    by_content: BTreeMap<ManifestRootCid, BTreeSet<[u8; 32]>>,
    all_providers: BTreeSet<[u8; 32]>,
    canonical_bytes: Vec<u8>,
}

impl RoutingAuthorityProjection {
    /// Finalized ledger identity from which this projection was rebuilt.
    #[must_use]
    pub const fn identity(&self) -> FinalizedStateIdentityV1 {
        self.identity
    }

    /// Providers authorized for one canonical manifest root CID.
    #[must_use]
    pub fn providers_for_content(&self, content: &ManifestRootCid) -> Option<&BTreeSet<[u8; 32]>> {
        self.by_content.get(content)
    }

    /// Every provider referenced by an active authoritative route.
    #[must_use]
    pub fn all_providers(&self) -> &BTreeSet<[u8; 32]> {
        &self.all_providers
    }

    /// Canonical Norito bytes used for cross-replica parity checks.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Finalized-state source used by [`RoutingAuthorityCache`].
///
/// Implementations must derive both methods from the same immutable state
/// view. The cache opens the source only after acquiring its single-flight
/// lock, so concurrent callers cannot race independent rebuilds.
pub trait RoutingAuthoritySource {
    /// Return the exact finalized identity of this immutable view.
    ///
    /// # Errors
    ///
    /// Returns a routing-authority error when the view has no coherent
    /// finalized identity.
    fn finalized_identity(&self) -> Result<FinalizedStateIdentityV1, RoutingAuthorityError>;

    /// Rebuild the authority projection for `identity` from this same view.
    ///
    /// # Errors
    ///
    /// Returns a routing-authority error when the bounded join fails closed.
    fn build_projection(
        &self,
        identity: FinalizedStateIdentityV1,
    ) -> Result<RoutingAuthorityProjection, RoutingAuthorityError>;
}

/// Errors returned by the finalized routing-authority join and cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RoutingAuthorityError {
    /// Finalized state did not provide a structurally valid height/hash pair.
    #[error("routing authority source has an invalid finalized identity")]
    InvalidFinalizedIdentity,
    /// A caller attempted to replace the cache with an older finalized view.
    #[error("routing authority source is older than the cached finalized identity")]
    StaleFinalizedIdentity,
    /// Distinct block hashes were observed for the same finalized height.
    #[error("routing authority source conflicts with the cached finalized block hash")]
    FinalizedFork,
    /// The source or resulting projection exceeded a first-release safety cap.
    #[error("routing authority snapshot exceeds first-release safety limits")]
    CapacityExceeded,
    /// Ledger records or canonical replication payloads were inconsistent.
    #[error("routing authority snapshot failed canonical validation")]
    Corrupt,
}

#[derive(Debug, Clone)]
struct CachedAuthorityProjection {
    identity: FinalizedStateIdentityV1,
    result: Result<Arc<RoutingAuthorityProjection>, RoutingAuthorityError>,
}

/// Bounded, single-flight cache for the SFM-1 finalized authority join.
///
/// One projection (or deterministic failure) is retained. Older identities
/// and same-height conflicting hashes fail closed without evicting the cached
/// entry. A newer identity is rebuilt while holding the single-flight lock and
/// atomically replaces the previous result; no local fallback can become
/// authoritative.
#[derive(Debug, Default)]
pub struct RoutingAuthorityCache {
    cached: tokio::sync::Mutex<Option<CachedAuthorityProjection>>,
    hits: AtomicU64,
    rebuilds: AtomicU64,
    rebuild_failures: AtomicU64,
    stale_rejections: AtomicU64,
    fork_rejections: AtomicU64,
    evictions: AtomicU64,
}

/// Payload-free counters for bounded routing-authority cache outcomes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoutingAuthorityCacheMetrics {
    /// Exact finalized-identity cache hits.
    pub hits: u64,
    /// Single-flight rebuild attempts.
    pub rebuilds: u64,
    /// Deterministic rebuild failures cached for a finalized identity.
    pub rebuild_failures: u64,
    /// Older finalized identities rejected without mutation.
    pub stale_rejections: u64,
    /// Same-height conflicting hashes rejected without mutation.
    pub fork_rejections: u64,
    /// Previous entries atomically replaced by a newer finalized identity.
    pub evictions: u64,
}

/// Bounded telemetry outcome for one routing-authority cache request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingAuthorityCacheOutcome {
    /// The exact finalized identity was already cached.
    Hit,
    /// A new finalized identity was rebuilt successfully.
    Rebuild,
    /// A new finalized identity rebuilt to a deterministic failure.
    RebuildFailure,
    /// An older finalized identity was rejected.
    StaleRejected,
    /// A same-height conflicting finalized hash was rejected.
    ForkRejected,
}

impl RoutingAuthorityCacheOutcome {
    /// Stable bounded Prometheus label for this outcome.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Rebuild => "rebuild",
            Self::RebuildFailure => "rebuild_failure",
            Self::StaleRejected => "stale_rejected",
            Self::ForkRejected => "fork_rejected",
        }
    }
}

impl RoutingAuthorityCache {
    /// Resolve or rebuild the projection from one immutable finalized source.
    ///
    /// `open_source` is invoked after the single-flight lock is acquired. This
    /// lets a service open one point-in-time ledger view without carrying that
    /// view across an async suspension.
    pub async fn get_or_rebuild<F, S>(
        &self,
        open_source: F,
    ) -> (
        Result<Arc<RoutingAuthorityProjection>, RoutingAuthorityError>,
        RoutingAuthorityCacheOutcome,
    )
    where
        F: FnOnce() -> S,
        S: RoutingAuthoritySource,
    {
        let mut cached = self.cached.lock().await;
        let source = open_source();
        let identity = match source.finalized_identity() {
            Ok(identity) => identity,
            Err(error) => {
                increment(&self.rebuild_failures);
                return (Err(error), RoutingAuthorityCacheOutcome::RebuildFailure);
            }
        };

        if let Some(entry) = cached.as_ref() {
            match identity.height.cmp(&entry.identity.height) {
                Ordering::Less => {
                    increment(&self.stale_rejections);
                    return (
                        Err(RoutingAuthorityError::StaleFinalizedIdentity),
                        RoutingAuthorityCacheOutcome::StaleRejected,
                    );
                }
                Ordering::Equal if identity.block_hash != entry.identity.block_hash => {
                    increment(&self.fork_rejections);
                    return (
                        Err(RoutingAuthorityError::FinalizedFork),
                        RoutingAuthorityCacheOutcome::ForkRejected,
                    );
                }
                Ordering::Equal => {
                    increment(&self.hits);
                    return (entry.result.clone(), RoutingAuthorityCacheOutcome::Hit);
                }
                Ordering::Greater => {}
            }
        }

        increment(&self.rebuilds);
        let result = source.build_projection(identity).and_then(|projection| {
            if projection.identity != identity {
                return Err(RoutingAuthorityError::Corrupt);
            }
            Ok(Arc::new(projection))
        });
        let outcome = if result.is_err() {
            increment(&self.rebuild_failures);
            RoutingAuthorityCacheOutcome::RebuildFailure
        } else {
            RoutingAuthorityCacheOutcome::Rebuild
        };
        if cached.is_some() {
            increment(&self.evictions);
        }
        *cached = Some(CachedAuthorityProjection {
            identity,
            result: result.clone(),
        });
        (result, outcome)
    }

    /// Return payload-free cache counters.
    #[must_use]
    pub fn metrics(&self) -> RoutingAuthorityCacheMetrics {
        RoutingAuthorityCacheMetrics {
            hits: self.hits.load(AtomicOrdering::Relaxed),
            rebuilds: self.rebuilds.load(AtomicOrdering::Relaxed),
            rebuild_failures: self.rebuild_failures.load(AtomicOrdering::Relaxed),
            stale_rejections: self.stale_rejections.load(AtomicOrdering::Relaxed),
            fork_rejections: self.fork_rejections.load(AtomicOrdering::Relaxed),
            evictions: self.evictions.load(AtomicOrdering::Relaxed),
        }
    }
}

fn increment(counter: &AtomicU64) {
    let _ = counter.fetch_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}

/// Build the canonical SFM-1 projection from finalized ledger records.
///
/// Input iteration order does not affect the projection or its canonical
/// bytes. Only approved manifests and valid completed replication orders at
/// `identity.height()` grant authority.
///
/// # Errors
///
/// Returns [`RoutingAuthorityError::CapacityExceeded`] for bounded-resource
/// violations and [`RoutingAuthorityError::Corrupt`] for non-canonical or
/// inconsistent ledger state.
pub fn build_routing_authority_projection<'a, M, O>(
    identity: FinalizedStateIdentityV1,
    manifests: M,
    orders: O,
) -> Result<RoutingAuthorityProjection, RoutingAuthorityError>
where
    M: IntoIterator<Item = (&'a ManifestDigest, &'a PinManifestRecord)>,
    O: IntoIterator<Item = (&'a ReplicationOrderId, &'a ReplicationOrderRecord)>,
{
    build_routing_authority_projection_with_limits(
        identity,
        manifests,
        orders,
        AuthorityJoinLimits::PRODUCTION,
    )
}

fn build_routing_authority_projection_with_limits<'a, M, O>(
    identity: FinalizedStateIdentityV1,
    manifests: M,
    orders: O,
    limits: AuthorityJoinLimits,
) -> Result<RoutingAuthorityProjection, RoutingAuthorityError>
where
    M: IntoIterator<Item = (&'a ManifestDigest, &'a PinManifestRecord)>,
    O: IntoIterator<Item = (&'a ReplicationOrderId, &'a ReplicationOrderRecord)>,
{
    let current_epoch = identity.height();
    let mut all_manifests = BTreeMap::new();
    let mut manifest_count = 0usize;
    for (digest, record) in manifests {
        manifest_count = manifest_count.saturating_add(1);
        if manifest_count > limits.manifests {
            return Err(RoutingAuthorityError::CapacityExceeded);
        }
        if digest != &record.digest || all_manifests.insert(*digest, record).is_some() {
            return Err(RoutingAuthorityError::Corrupt);
        }
    }

    let mut by_content: BTreeMap<ManifestRootCid, BTreeSet<[u8; 32]>> = BTreeMap::new();
    let mut all_providers = BTreeSet::new();
    let mut order_count = 0usize;
    let mut order_payload_bytes = 0usize;
    let mut provider_refs = 0usize;
    let mut seen_orders = BTreeSet::new();
    for (order_id, record) in orders {
        order_count = order_count.saturating_add(1);
        if order_count > limits.orders {
            return Err(RoutingAuthorityError::CapacityExceeded);
        }
        if order_id != &record.order_id || !seen_orders.insert(*order_id) {
            return Err(RoutingAuthorityError::Corrupt);
        }
        order_payload_bytes = order_payload_bytes
            .checked_add(record.canonical_order.len())
            .ok_or(RoutingAuthorityError::CapacityExceeded)?;
        if order_payload_bytes > limits.order_payload_bytes {
            return Err(RoutingAuthorityError::CapacityExceeded);
        }
        let ReplicationOrderStatus::Completed(completed_epoch) = record.status else {
            continue;
        };
        if completed_epoch < record.issued_epoch
            || completed_epoch > record.deadline_epoch
            || completed_epoch > current_epoch
            || record.issued_epoch > current_epoch
        {
            return Err(RoutingAuthorityError::Corrupt);
        }
        let Some(manifest) = all_manifests.get(&record.manifest_digest) else {
            return Err(RoutingAuthorityError::Corrupt);
        };
        if manifest.root_cid != record.manifest_root_cid {
            return Err(RoutingAuthorityError::Corrupt);
        }
        let payload = decode_canonical_order(record)?;
        if payload.chunking_profile != manifest.chunker.to_handle() {
            return Err(RoutingAuthorityError::Corrupt);
        }
        if !matches!(manifest.status, PinStatus::Approved(epoch) if epoch <= current_epoch) {
            // Historical orders for retired or not-yet-approved manifests are
            // auditable, but never grant a current route.
            continue;
        }
        provider_refs = provider_refs
            .checked_add(payload.assignments.len())
            .ok_or(RoutingAuthorityError::CapacityExceeded)?;
        if provider_refs > limits.provider_refs {
            return Err(RoutingAuthorityError::CapacityExceeded);
        }
        let providers = by_content.entry(record.manifest_root_cid).or_default();
        for assignment in payload.assignments {
            providers.insert(assignment.provider_id);
            all_providers.insert(assignment.provider_id);
        }
    }

    let routes = by_content
        .iter()
        .map(
            |(manifest_root_cid, provider_ids)| RoutingAuthorityRouteV1 {
                manifest_root_cid: *manifest_root_cid,
                provider_ids: provider_ids.iter().copied().collect(),
            },
        )
        .collect();
    let envelope = RoutingAuthorityProjectionEnvelopeV1 {
        version: ROUTING_AUTHORITY_PROJECTION_VERSION_V1,
        finalized_state: identity,
        routes,
    };
    let canonical_bytes =
        norito::to_bytes(&envelope).map_err(|_| RoutingAuthorityError::Corrupt)?;
    if canonical_bytes.len() > limits.projection_bytes {
        return Err(RoutingAuthorityError::CapacityExceeded);
    }

    Ok(RoutingAuthorityProjection {
        identity,
        by_content,
        all_providers,
        canonical_bytes,
    })
}

fn decode_canonical_order(
    record: &ReplicationOrderRecord,
) -> Result<ReplicationOrderV1, RoutingAuthorityError> {
    if record.canonical_order.is_empty()
        || record.canonical_order.len() > MAX_REPLICATION_ORDER_PAYLOAD_BYTES
    {
        return Err(RoutingAuthorityError::Corrupt);
    }
    let payload: ReplicationOrderV1 = norito::decode_from_bytes_with_limits(
        &record.canonical_order,
        REPLICATION_ORDER_DECODE_LIMITS,
    )
    .map_err(|_| RoutingAuthorityError::Corrupt)?;
    payload
        .validate()
        .map_err(|_| RoutingAuthorityError::Corrupt)?;
    let canonical = norito::to_bytes(&payload).map_err(|_| RoutingAuthorityError::Corrupt)?;
    if canonical != record.canonical_order
        || payload.order_id != *record.order_id.as_bytes()
        || payload.manifest_digest != *record.manifest_digest.as_bytes()
        || payload.manifest_cid.as_slice() != record.manifest_root_cid.as_bytes()
    {
        return Err(RoutingAuthorityError::Corrupt);
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering as AtomicUsizeOrdering},
        },
        time::Duration,
    };

    use iroha_data_model::{
        account::AccountId,
        metadata::Metadata,
        sorafs::{
            capacity::ProviderId,
            pin_registry::{
                ChunkerProfileHandle, PinPolicy, ProviderIngestCompletionAuthorityV1,
                ProviderIngestCompletionSignerPolicyV1, ProviderIngestFinalizedAnchorV1,
                ReplicationOrderCompletionRecord, StorageClass,
            },
        },
    };
    use sorafs_manifest::capacity::{
        REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
    };

    use super::*;

    const NOW: u64 = 1_700_000_100;

    fn finalized_identity(height: u64, seed: u8) -> FinalizedStateIdentityV1 {
        let mut hash = [seed.max(1); 32];
        hash[31] |= 1;
        FinalizedStateIdentityV1::new(height, Some(hash)).expect("valid finalized identity")
    }

    fn fixture_account() -> AccountId {
        AccountId::new(
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("fixture public key"),
        )
    }

    fn sample_cid(seed: u8) -> ManifestRootCid {
        ManifestRootCid::from_blake3_digest([seed.max(1); 32]).expect("canonical root CID")
    }

    fn sample_digest(seed: u8) -> ManifestDigest {
        ManifestDigest::new([seed.max(1); 32])
    }

    fn sample_chunker() -> ChunkerProfileHandle {
        ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        }
    }

    fn sample_manifest(seed: u8, status: PinStatus) -> (ManifestDigest, PinManifestRecord) {
        let digest = sample_digest(seed);
        let mut record = PinManifestRecord::new(
            digest,
            sample_cid(seed.wrapping_add(1)),
            sample_chunker(),
            [seed.wrapping_add(2).max(1); 32],
            [seed.wrapping_add(3).max(1); 32],
            1,
            PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: 1_000,
            },
            fixture_account(),
            1,
            None,
            None,
            Metadata::default(),
        );
        record.status = status;
        (digest, record)
    }

    fn sample_order(
        seed: u8,
        manifest: &(ManifestDigest, PinManifestRecord),
        providers: &[[u8; 32]],
        status: ReplicationOrderStatus,
    ) -> (ReplicationOrderId, ReplicationOrderRecord) {
        let order_id = ReplicationOrderId::new([seed.max(1); 32]);
        let mut canonical_providers = providers.to_vec();
        canonical_providers.sort_unstable();
        canonical_providers.dedup();
        let assignments = canonical_providers
            .iter()
            .copied()
            .map(|provider_id| ReplicationAssignmentV1 {
                provider_id,
                slice_gib: 1,
                lane: None,
            })
            .collect::<Vec<_>>();
        let payload = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: *order_id.as_bytes(),
            manifest_cid: manifest.1.root_cid.as_bytes().to_vec(),
            manifest_digest: *manifest.0.as_bytes(),
            chunking_profile: manifest.1.chunker.to_handle(),
            target_replicas: u16::try_from(canonical_providers.len())
                .expect("bounded provider count"),
            assignments,
            issued_at: NOW.saturating_sub(100),
            deadline_at: NOW.saturating_add(100),
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 100,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: Vec::new(),
        };
        payload.validate().expect("valid test replication order");
        let issued_by = fixture_account();
        let provider_completions = match status {
            ReplicationOrderStatus::Completed(completion_epoch) => canonical_providers
                .iter()
                .copied()
                .map(|provider_id| ReplicationOrderCompletionRecord {
                    provider_id: ProviderId::new(provider_id),
                    completed_by: issued_by.clone(),
                    completion_epoch,
                    assignment_revision: 1,
                    completion_authority: ProviderIngestCompletionAuthorityV1::new(
                        issued_by.clone(),
                        ProviderIngestCompletionSignerPolicyV1 {
                            policy_id: [0xA1; 32],
                            revision: 1,
                            predecessor_digest: None,
                            policy_digest: [0xA2; 32],
                        },
                    ),
                    finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                        height: completion_epoch,
                        block_hash: [0xA3; 32],
                    },
                })
                .collect(),
            ReplicationOrderStatus::Pending | ReplicationOrderStatus::Expired(_) => Vec::new(),
        };
        let record = ReplicationOrderRecord {
            order_id,
            manifest_digest: manifest.0,
            manifest_root_cid: manifest.1.root_cid,
            issued_by,
            issued_epoch: 5,
            deadline_epoch: 20,
            canonical_order: norito::to_bytes(&payload).expect("encode replication order"),
            assignment_revision: 1,
            provider_completions,
            status,
        };
        (order_id, record)
    }

    fn build_test_projection(
        identity: FinalizedStateIdentityV1,
        manifests: &[(ManifestDigest, PinManifestRecord)],
        orders: &[(ReplicationOrderId, ReplicationOrderRecord)],
    ) -> Result<RoutingAuthorityProjection, RoutingAuthorityError> {
        build_routing_authority_projection(
            identity,
            manifests.iter().map(|(id, record)| (id, record)),
            orders.iter().map(|(id, record)| (id, record)),
        )
    }

    #[derive(Clone)]
    struct StaticSource {
        identity: FinalizedStateIdentityV1,
        projection: RoutingAuthorityProjection,
        builds: Arc<AtomicUsize>,
        delay: Duration,
    }

    impl RoutingAuthoritySource for StaticSource {
        fn finalized_identity(&self) -> Result<FinalizedStateIdentityV1, RoutingAuthorityError> {
            Ok(self.identity)
        }

        fn build_projection(
            &self,
            _identity: FinalizedStateIdentityV1,
        ) -> Result<RoutingAuthorityProjection, RoutingAuthorityError> {
            self.builds.fetch_add(1, AtomicUsizeOrdering::SeqCst);
            std::thread::sleep(self.delay);
            Ok(self.projection.clone())
        }
    }

    struct FailingSource {
        identity: FinalizedStateIdentityV1,
        builds: Arc<AtomicUsize>,
    }

    impl RoutingAuthoritySource for FailingSource {
        fn finalized_identity(&self) -> Result<FinalizedStateIdentityV1, RoutingAuthorityError> {
            Ok(self.identity)
        }

        fn build_projection(
            &self,
            _identity: FinalizedStateIdentityV1,
        ) -> Result<RoutingAuthorityProjection, RoutingAuthorityError> {
            self.builds.fetch_add(1, AtomicUsizeOrdering::SeqCst);
            Err(RoutingAuthorityError::Corrupt)
        }
    }

    fn empty_source(identity: FinalizedStateIdentityV1, builds: Arc<AtomicUsize>) -> StaticSource {
        StaticSource {
            identity,
            projection: build_routing_authority_projection(
                identity,
                std::iter::empty(),
                std::iter::empty(),
            )
            .expect("empty projection"),
            builds,
            delay: Duration::ZERO,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_callers_share_one_rebuild() {
        let cache = Arc::new(RoutingAuthorityCache::default());
        let builds = Arc::new(AtomicUsize::new(0));
        let source = StaticSource {
            delay: Duration::from_millis(20),
            ..empty_source(finalized_identity(10, 1), Arc::clone(&builds))
        };
        let barrier = Arc::new(tokio::sync::Barrier::new(16));
        let mut tasks = Vec::new();
        for _ in 0..16 {
            let cache = Arc::clone(&cache);
            let source = source.clone();
            let barrier = Arc::clone(&barrier);
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                cache.get_or_rebuild(|| source).await
            }));
        }
        let mut first = None;
        let mut rebuilds = 0;
        for task in tasks {
            let (projection, outcome) = task.await.expect("cache task");
            let projection = projection.expect("cached projection");
            if let Some(first) = first.as_ref() {
                assert!(Arc::ptr_eq(first, &projection));
            } else {
                first = Some(projection);
            }
            rebuilds += usize::from(outcome == RoutingAuthorityCacheOutcome::Rebuild);
        }
        assert_eq!(builds.load(AtomicUsizeOrdering::SeqCst), 1);
        assert_eq!(rebuilds, 1);
        assert_eq!(
            cache.metrics(),
            RoutingAuthorityCacheMetrics {
                hits: 15,
                rebuilds: 1,
                ..RoutingAuthorityCacheMetrics::default()
            }
        );
    }

    #[tokio::test]
    async fn stale_and_fork_identities_fail_without_evicting() {
        let cache = RoutingAuthorityCache::default();
        let builds = Arc::new(AtomicUsize::new(0));
        let current = finalized_identity(10, 1);
        let (original, outcome) = cache
            .get_or_rebuild(|| empty_source(current, Arc::clone(&builds)))
            .await;
        let original = original.expect("initial projection");
        assert_eq!(outcome, RoutingAuthorityCacheOutcome::Rebuild);

        let (stale, outcome) = cache
            .get_or_rebuild(|| empty_source(finalized_identity(9, 2), Arc::clone(&builds)))
            .await;
        assert_eq!(stale, Err(RoutingAuthorityError::StaleFinalizedIdentity));
        assert_eq!(outcome, RoutingAuthorityCacheOutcome::StaleRejected);

        let (fork, outcome) = cache
            .get_or_rebuild(|| empty_source(finalized_identity(10, 3), Arc::clone(&builds)))
            .await;
        assert_eq!(fork, Err(RoutingAuthorityError::FinalizedFork));
        assert_eq!(outcome, RoutingAuthorityCacheOutcome::ForkRejected);

        let (cached, outcome) = cache
            .get_or_rebuild(|| empty_source(current, Arc::clone(&builds)))
            .await;
        assert_eq!(outcome, RoutingAuthorityCacheOutcome::Hit);
        assert!(Arc::ptr_eq(
            &original,
            &cached.expect("original remains cached")
        ));
        assert_eq!(builds.load(AtomicUsizeOrdering::SeqCst), 1);
        assert_eq!(
            cache.metrics(),
            RoutingAuthorityCacheMetrics {
                hits: 1,
                rebuilds: 1,
                stale_rejections: 1,
                fork_rejections: 1,
                ..RoutingAuthorityCacheMetrics::default()
            }
        );
    }

    #[tokio::test]
    async fn newer_identity_rebuilds_and_evicts_atomically() {
        let cache = RoutingAuthorityCache::default();
        let builds = Arc::new(AtomicUsize::new(0));
        let first = finalized_identity(10, 1);
        let second = finalized_identity(11, 2);
        cache
            .get_or_rebuild(|| empty_source(first, Arc::clone(&builds)))
            .await
            .0
            .expect("first projection");
        let (projection, outcome) = cache
            .get_or_rebuild(|| empty_source(second, Arc::clone(&builds)))
            .await;
        assert_eq!(outcome, RoutingAuthorityCacheOutcome::Rebuild);
        assert_eq!(projection.expect("new projection").identity(), second);
        assert_eq!(builds.load(AtomicUsizeOrdering::SeqCst), 2);
        assert_eq!(
            cache.metrics(),
            RoutingAuthorityCacheMetrics {
                rebuilds: 2,
                evictions: 1,
                ..RoutingAuthorityCacheMetrics::default()
            }
        );
    }

    #[tokio::test]
    async fn newer_failure_replaces_success_without_local_fallback() {
        let cache = RoutingAuthorityCache::default();
        let builds = Arc::new(AtomicUsize::new(0));
        let first = finalized_identity(10, 1);
        let failed = finalized_identity(11, 2);
        cache
            .get_or_rebuild(|| empty_source(first, Arc::clone(&builds)))
            .await
            .0
            .expect("first projection");

        let (result, outcome) = cache
            .get_or_rebuild(|| FailingSource {
                identity: failed,
                builds: Arc::clone(&builds),
            })
            .await;
        assert_eq!(result, Err(RoutingAuthorityError::Corrupt));
        assert_eq!(outcome, RoutingAuthorityCacheOutcome::RebuildFailure);

        let (stale, outcome) = cache
            .get_or_rebuild(|| empty_source(first, Arc::clone(&builds)))
            .await;
        assert_eq!(stale, Err(RoutingAuthorityError::StaleFinalizedIdentity));
        assert_eq!(outcome, RoutingAuthorityCacheOutcome::StaleRejected);

        let (cached_failure, outcome) = cache
            .get_or_rebuild(|| FailingSource {
                identity: failed,
                builds: Arc::clone(&builds),
            })
            .await;
        assert_eq!(cached_failure, Err(RoutingAuthorityError::Corrupt));
        assert_eq!(outcome, RoutingAuthorityCacheOutcome::Hit);
        assert_eq!(builds.load(AtomicUsizeOrdering::SeqCst), 2);
    }

    #[test]
    fn replica_input_order_produces_byte_identical_projection() {
        let identity = finalized_identity(10, 7);
        let first_manifest = sample_manifest(1, PinStatus::Approved(3));
        let second_manifest = sample_manifest(2, PinStatus::Approved(3));
        let first_order = sample_order(
            11,
            &first_manifest,
            &[[0x42; 32], [0x41; 32]],
            ReplicationOrderStatus::Completed(8),
        );
        let second_order = sample_order(
            12,
            &second_manifest,
            &[[0x43; 32]],
            ReplicationOrderStatus::Completed(8),
        );
        let manifests_a = vec![first_manifest.clone(), second_manifest.clone()];
        let manifests_b = vec![second_manifest, first_manifest];
        let orders_a = vec![first_order.clone(), second_order.clone()];
        let orders_b = vec![second_order, first_order];

        let first = build_test_projection(identity, &manifests_a, &orders_a)
            .expect("first replica projection");
        let second = build_test_projection(identity, &manifests_b, &orders_b)
            .expect("second replica projection");
        assert_eq!(first, second);
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    }

    #[test]
    fn only_approved_manifests_with_completed_orders_grant_authority() {
        let identity = finalized_identity(10, 8);
        let provider = [0x44; 32];
        let approved = sample_manifest(1, PinStatus::Approved(3));
        let completed = sample_order(
            7,
            &approved,
            &[provider],
            ReplicationOrderStatus::Completed(8),
        );
        let projection =
            build_test_projection(identity, std::slice::from_ref(&approved), &[completed])
                .expect("projection");
        assert_eq!(
            projection.providers_for_content(&approved.1.root_cid),
            Some(&BTreeSet::from([provider]))
        );

        for status in [
            ReplicationOrderStatus::Pending,
            ReplicationOrderStatus::Expired(9),
        ] {
            let order = sample_order(8, &approved, &[provider], status);
            assert!(
                build_test_projection(identity, std::slice::from_ref(&approved), &[order])
                    .expect("inactive order projection")
                    .all_providers()
                    .is_empty()
            );
        }
        for status in [
            PinStatus::Pending,
            PinStatus::Retired(9),
            PinStatus::Approved(11),
        ] {
            let manifest = sample_manifest(2, status);
            let order = sample_order(
                9,
                &manifest,
                &[provider],
                ReplicationOrderStatus::Completed(8),
            );
            assert!(
                build_test_projection(identity, &[manifest], &[order])
                    .expect("inactive manifest projection")
                    .all_providers()
                    .is_empty()
            );
        }
    }

    #[test]
    fn join_enforces_manifest_provider_and_projection_bounds() {
        let identity = finalized_identity(10, 9);
        let first_manifest = sample_manifest(1, PinStatus::Approved(3));
        let second_manifest = sample_manifest(2, PinStatus::Approved(3));
        let order = sample_order(
            11,
            &first_manifest,
            &[[0x42; 32], [0x43; 32]],
            ReplicationOrderStatus::Completed(8),
        );
        let manifests = [first_manifest.clone(), second_manifest.clone()];
        let orders = [order.clone()];
        let base_limits = AuthorityJoinLimits {
            manifests: 2,
            orders: 1,
            provider_refs: 2,
            order_payload_bytes: MAX_AUTHORITY_ORDER_PAYLOAD_BYTES,
            projection_bytes: MAX_AUTHORITY_PROJECTION_BYTES,
        };

        assert_eq!(
            build_routing_authority_projection_with_limits(
                identity,
                manifests.iter().map(|(id, record)| (id, record)),
                orders.iter().map(|(id, record)| (id, record)),
                AuthorityJoinLimits {
                    manifests: 1,
                    ..base_limits
                },
            ),
            Err(RoutingAuthorityError::CapacityExceeded)
        );
        assert_eq!(
            build_routing_authority_projection_with_limits(
                identity,
                manifests[..1].iter().map(|(id, record)| (id, record)),
                orders.iter().map(|(id, record)| (id, record)),
                AuthorityJoinLimits {
                    orders: 0,
                    ..base_limits
                },
            ),
            Err(RoutingAuthorityError::CapacityExceeded)
        );
        assert_eq!(
            build_routing_authority_projection_with_limits(
                identity,
                manifests[..1].iter().map(|(id, record)| (id, record)),
                orders.iter().map(|(id, record)| (id, record)),
                AuthorityJoinLimits {
                    provider_refs: 1,
                    ..base_limits
                },
            ),
            Err(RoutingAuthorityError::CapacityExceeded)
        );
        assert_eq!(
            build_routing_authority_projection_with_limits(
                identity,
                manifests[..1].iter().map(|(id, record)| (id, record)),
                orders.iter().map(|(id, record)| (id, record)),
                AuthorityJoinLimits {
                    order_payload_bytes: order.1.canonical_order.len().saturating_sub(1),
                    ..base_limits
                },
            ),
            Err(RoutingAuthorityError::CapacityExceeded)
        );
        assert_eq!(
            build_routing_authority_projection_with_limits(
                identity,
                manifests[..1].iter().map(|(id, record)| (id, record)),
                orders.iter().map(|(id, record)| (id, record)),
                AuthorityJoinLimits {
                    projection_bytes: 1,
                    ..base_limits
                },
            ),
            Err(RoutingAuthorityError::CapacityExceeded)
        );
    }

    #[test]
    fn invalid_identity_shapes_are_rejected() {
        assert_eq!(
            FinalizedStateIdentityV1::new(0, Some([1; 32])),
            Err(RoutingAuthorityError::InvalidFinalizedIdentity)
        );
        assert_eq!(
            FinalizedStateIdentityV1::new(1, None),
            Err(RoutingAuthorityError::InvalidFinalizedIdentity)
        );
        assert_eq!(
            FinalizedStateIdentityV1::new(1, Some([2; 32])),
            Err(RoutingAuthorityError::InvalidFinalizedIdentity)
        );
        assert!(FinalizedStateIdentityV1::new(0, None).is_ok());
    }

    #[test]
    fn authority_rejects_future_completion_and_payload_equivocation() {
        let identity = finalized_identity(10, 11);
        let manifest = sample_manifest(3, PinStatus::Approved(1));
        let provider = [0x55; 32];
        let future = sample_order(
            10,
            &manifest,
            &[provider],
            ReplicationOrderStatus::Completed(11),
        );
        assert_eq!(
            build_test_projection(identity, std::slice::from_ref(&manifest), &[future]),
            Err(RoutingAuthorityError::Corrupt)
        );

        let mut corrupt = sample_order(
            11,
            &manifest,
            &[provider],
            ReplicationOrderStatus::Completed(8),
        );
        corrupt.1.canonical_order.push(0);
        assert_eq!(
            build_test_projection(identity, std::slice::from_ref(&manifest), &[corrupt]),
            Err(RoutingAuthorityError::Corrupt)
        );

        let valid = sample_order(
            12,
            &manifest,
            &[provider],
            ReplicationOrderStatus::Completed(8),
        );
        assert_eq!(
            build_test_projection(identity, &[manifest], &[valid.clone(), valid]),
            Err(RoutingAuthorityError::Corrupt)
        );
    }

    #[test]
    fn authority_rejects_oversized_order_before_decode() {
        let identity = finalized_identity(10, 13);
        let manifest = sample_manifest(4, PinStatus::Approved(1));
        let mut order = sample_order(
            13,
            &manifest,
            &[[0x66; 32]],
            ReplicationOrderStatus::Completed(8),
        );
        order.1.canonical_order = vec![0; MAX_REPLICATION_ORDER_PAYLOAD_BYTES + 1];
        assert_eq!(
            build_test_projection(identity, &[manifest], &[order]),
            Err(RoutingAuthorityError::Corrupt)
        );
    }
}
