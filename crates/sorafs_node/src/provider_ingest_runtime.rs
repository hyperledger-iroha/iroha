//! Supervised finalized-ledger runtime for provider replication ingest.
//!
//! The runtime deliberately owns no authoritative order, completion, or
//! provider-registration state. Every scan reads one immutable finalized view,
//! drives the durable [`ProviderIngestOutbox`], and reconciles semantic ledger
//! completion before considering transaction-level delivery state.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha_config::parameters::is_production_runtime_handle;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            PinManifestFinalizedRecordV1, PinStatus, ProviderIngestCompletionAuthorityV1,
            ProviderIngestCompletionSignerPolicyV1, ReplicationOrderRecord, ReplicationOrderStatus,
        },
    },
    transaction::{SignedTransaction, TransactionPayload},
};
use norito::{core::DecodeLimits, decode_from_bytes_with_limits};
use sorafs_manifest::capacity::{
    MAX_CAPACITY_METADATA_VALUE_BYTES, MAX_REPLICATION_ORDER_ASSIGNMENTS, ReplicationOrderV1,
};
use thiserror::Error;
use tokio::sync::watch;

use crate::provider_ingest_outbox::{
    FinalizedProviderIngestAuthorizationV1, PROVIDER_INGEST_STATUS_PAGE_MAX_V1,
    ProviderIngestCancellationReasonV1, ProviderIngestClaimOwnerV1,
    ProviderIngestCompletionSigningContextV1, ProviderIngestCompletionStateV1,
    ProviderIngestDeadLetterReasonV1, ProviderIngestDeliveryStateV1,
    ProviderIngestExposedCompletionExpiryV1, ProviderIngestFailureClassV1,
    ProviderIngestFinalizedCancellationV1, ProviderIngestFinalizedCompletionV1,
    ProviderIngestFinalizedCursorV1, ProviderIngestOutbox, ProviderIngestOutboxError,
    ProviderIngestSignerPolicyObservationV1, ProviderIngestSourceClaimV1,
};

const REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1: usize = 256 * 1024;
const PROVIDER_INGEST_SOURCE_QUALIFICATION_VERSION_V1: u8 = 1;
const PROVIDER_INGEST_COMPLETION_SIGNER_QUALIFICATION_VERSION_V1: u8 = 1;
const REPLICATION_ORDER_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    MAX_CAPACITY_METADATA_VALUE_BYTES,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1,
    131_072,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1 * 4,
    32,
);

/// Boxed asynchronous operation used by provider-ingest integration traits.
pub type ProviderIngestFutureV1<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Resource and timeout policy for one provider-ingest runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestRuntimePolicyV1 {
    /// Maximum finalized assignment rows requested in one page.
    pub max_page_rows: usize,
    /// Maximum finalized pages reconciled in one tick.
    pub max_pages_per_tick: usize,
    /// Maximum source jobs performed in one tick.
    pub max_source_jobs_per_tick: usize,
    /// Maximum governed source provider identities passed to one fetch.
    pub max_source_providers: usize,
    /// Delay between supervised scans.
    pub scan_interval_ms: u64,
    /// Timeout for source verification and fetch, and soft deadline for a
    /// mutating storage operation that must finish under its durable lease.
    pub source_operation_timeout_ms: u64,
    /// Interval used to durably renew a source lease during slow I/O.
    pub source_lease_renew_interval_ms: u64,
    /// Timeout for payload construction, signer resolution, and signing.
    pub signer_timeout_ms: u64,
    /// Timeout for queue preflight, exposure, and transaction observation.
    pub ingress_timeout_ms: u64,
}

impl Default for ProviderIngestRuntimePolicyV1 {
    fn default() -> Self {
        Self {
            max_page_rows: 256,
            max_pages_per_tick: 16,
            max_source_jobs_per_tick: 16,
            max_source_providers: MAX_REPLICATION_ORDER_ASSIGNMENTS,
            scan_interval_ms: 1_000,
            source_operation_timeout_ms: 5 * 60_000,
            source_lease_renew_interval_ms: 15_000,
            signer_timeout_ms: 30_000,
            ingress_timeout_ms: 30_000,
        }
    }
}

impl ProviderIngestRuntimePolicyV1 {
    fn validate(self, outbox: &ProviderIngestOutbox) -> Result<(), ProviderIngestRuntimeErrorV1> {
        if self.max_page_rows == 0
            || self.max_page_rows > PROVIDER_INGEST_STATUS_PAGE_MAX_V1
            || self.max_pages_per_tick == 0
            || self.max_source_jobs_per_tick == 0
            || self.max_source_providers == 0
            || self.max_source_providers > MAX_REPLICATION_ORDER_ASSIGNMENTS
            || self.scan_interval_ms == 0
            || self.source_operation_timeout_ms == 0
            || self.source_lease_renew_interval_ms == 0
            || self.source_lease_renew_interval_ms >= outbox.policy().source_lease_ttl_ms
            || self.signer_timeout_ms == 0
            || self.ingress_timeout_ms == 0
            || self
                .max_page_rows
                .checked_mul(self.max_pages_per_tick)
                .is_none()
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidPolicy);
        }
        Ok(())
    }
}

/// One assignment row read from a single immutable finalized state view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestFinalizedAssignmentV1 {
    /// Chain-authoritative pin manifest and its finalized cursor.
    pub pin: PinManifestFinalizedRecordV1,
    /// Chain-authoritative replication order.
    pub order: ReplicationOrderRecord,
    /// Current registered owner of this runtime's provider identity.
    pub provider_owner: Option<AccountId>,
    /// Exact current chain-authoritative completion owner and signer policy.
    pub completion_authority: Option<ProviderIngestCompletionAuthorityV1>,
    /// Current authoritative epoch to use for a new completion transaction.
    pub completion_epoch: Option<u64>,
    /// Exact committed transaction hash, when the finalized reader exposes it.
    pub committed_transaction_hash: Option<[u8; 32]>,
}

/// Bounded stable page of provider assignments from one finalized state view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestFinalizedAssignmentPageV1 {
    /// Immutable finalized cursor shared by every row.
    pub finalized_cursor: ProviderIngestFinalizedCursorV1,
    /// Finalized block creation time used for transaction-TTL proofs.
    pub finalized_block_time_ms: u64,
    /// Rows in strictly increasing replication-order identity.
    pub rows: Vec<ProviderIngestFinalizedAssignmentV1>,
    /// Last returned order identity when another page exists.
    pub next_after_order_id: Option<[u8; 32]>,
}

/// Finalized-ledger paging failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestFinalizedLedgerErrorV1 {
    /// The finalized query service is temporarily unavailable.
    Unavailable,
    /// The finalized query service rejected the bounded request.
    Rejected,
}

/// Reader for chain-authoritative assignments and provider completions.
pub trait ProviderIngestFinalizedLedgerV1: Send + Sync + 'static {
    /// Read one stable page after `after_order_id`.
    ///
    /// `at_finalized_cursor` is `None` only for the first page of a scan. Every
    /// continuation supplies the exact immutable cursor returned by that first
    /// page, including continuations resumed in a later runtime tick.
    fn read_assignment_page<'a>(
        &'a self,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: usize,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<ProviderIngestFinalizedAssignmentPageV1, ProviderIngestFinalizedLedgerErrorV1>,
    >;
}

/// Exact fetch request containing no source credentials or lease material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestSourceRequestV1 {
    /// Immutable finalized provider/order/manifest authorization.
    pub authorization: FinalizedProviderIngestAuthorizationV1,
    /// Canonically ordered governed source provider identities.
    pub source_provider_ids: Vec<[u8; 32]>,
}

/// Authenticated source-fetch outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestSourceFetchErrorV1 {
    /// No currently admitted authenticated source was reachable.
    Unavailable,
    /// A source returned malformed, noncanonical, or authorization-mismatched material.
    ContentRejected,
    /// A source identity, public policy, or qualification binding was rejected.
    Rejected,
}

/// Public, non-secret qualification for a top-level provider-ingest adapter.
///
/// The source-pool and governed signer-resolver roles each expose an
/// independently configured value. Credentials, endpoint material, grants,
/// private keys, and payload data are never represented by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestRuntimeProviderQualificationV1 {
    /// Non-zero adapter and public-policy revision.
    pub revision: u64,
    /// Non-zero digest of the exact public adapter policy.
    pub policy_digest: [u8; 32],
}

impl ProviderIngestRuntimeProviderQualificationV1 {
    /// Construct one first-release public adapter qualification.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }

    /// Return whether both required public qualification pins are non-zero.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.revision != 0 && self.policy_digest != [0; 32]
    }
}

/// Payload-free public qualification of one authenticated provider source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestSourceQualificationV1 {
    /// Qualification schema version.
    pub version: u8,
    /// Non-zero adapter and public-policy revision.
    pub revision: u64,
    /// Non-zero digest of the exact public source policy.
    pub policy_digest: [u8; 32],
}

impl ProviderIngestSourceQualificationV1 {
    /// Construct a first-release source qualification.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            version: PROVIDER_INGEST_SOURCE_QUALIFICATION_VERSION_V1,
            revision,
            policy_digest,
        }
    }

    /// Validate the qualification schema and non-zero binding.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported schema or zero revision/digest.
    pub fn validate(self) -> Result<(), ProviderIngestAuthenticatedSourcePoolErrorV1> {
        if self.version != PROVIDER_INGEST_SOURCE_QUALIFICATION_VERSION_V1
            || self.revision == 0
            || self.policy_digest == [0; 32]
        {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceQualification);
        }
        Ok(())
    }
}

/// Independently configured public binding for one authenticated provider source.
///
/// The binding contains no endpoint credentials, grants, tokens, private keys,
/// or payload material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestAuthenticatedSourceBindingV1 {
    /// Exact governed provider identity served by the source.
    pub provider_id: [u8; 32],
    /// Stable opaque identity of the provider-specific transport.
    pub runtime_handle: String,
    /// Exact non-zero adapter and public-policy revision.
    pub revision: u64,
    /// Exact non-zero digest of the public source policy.
    pub policy_digest: [u8; 32],
}

impl ProviderIngestAuthenticatedSourceBindingV1 {
    /// Return the exact qualification required from the injected source.
    #[must_use]
    pub const fn qualification(&self) -> ProviderIngestSourceQualificationV1 {
        ProviderIngestSourceQualificationV1::new(self.revision, self.policy_digest)
    }

    fn validate(
        &self,
        pool_runtime_handle: &str,
    ) -> Result<(), ProviderIngestAuthenticatedSourcePoolErrorV1> {
        if self.provider_id == [0; 32] {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidProviderId);
        }
        if !is_production_runtime_handle(&self.runtime_handle)
            || self.runtime_handle == pool_runtime_handle
        {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceHandle);
        }
        self.qualification().validate()
    }
}

/// Authenticated source fetch boundary.
///
/// Production implementations must resolve only current governance-admitted
/// signed adverts, authenticate a bounded stream grant, require HTTPS with
/// pinned trust and DNS-rebinding defenses, reject redirects and implicit
/// decompression, and enforce the exact manifest, chunk-plan, payload-length,
/// chunk digest, and PoR-root binding in the request. Implementations try only
/// a bounded canonical source list and never persist tokens, URLs, or payload
/// bytes in the outbox.
pub trait ProviderIngestAuthenticatedSourceFetchV1: Send + Sync + 'static {
    /// Verified material passed directly to local storage.
    type Fetched: Send + 'static;

    /// Fetch and verify exact material from an authenticated governed source.
    fn fetch<'a>(
        &'a self,
        request: ProviderIngestSourceRequestV1,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>>;
}

/// One independently authenticated provider source used by the production pool.
///
/// Implementations own their runtime-only endpoint, grant, credential, and
/// pinned-trust material. The stable handle and provider identity are public
/// policy identifiers only. A fetch must use a hard transport deadline, reject
/// redirects and implicit decompression, and verify the exact finalized
/// authorization before returning.
pub trait ProviderIngestAuthenticatedProviderSourceV1: Send + Sync + 'static {
    /// Verified material returned by this source.
    type Fetched: Send + 'static;

    /// Exact governed provider identity served by this source.
    fn provider_id(&self) -> [u8; 32];

    /// Stable public identity of this provider-specific transport.
    fn runtime_handle(&self) -> &str;

    /// Current payload-free adapter and public-policy qualification.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free source failure when the qualification
    /// cannot be authenticated.
    fn qualification(
        &self,
    ) -> Result<ProviderIngestSourceQualificationV1, ProviderIngestSourceFetchErrorV1>;

    /// Non-mutating authenticated readiness check.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free source failure when the source is not
    /// currently ready or its public policy is rejected.
    fn check_readiness(&self) -> Result<(), ProviderIngestSourceFetchErrorV1>;

    /// Fetch exact material for one finalized authorization.
    fn fetch_provider<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>>;
}

/// Invalid construction of an authenticated multi-provider source pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProviderIngestAuthenticatedSourcePoolErrorV1 {
    /// The pool's public runtime identity is not production-safe.
    #[error("provider-ingest source-pool handle is invalid")]
    InvalidPoolHandle,
    /// The pool's top-level public qualification is zero or otherwise invalid.
    #[error("provider-ingest source-pool qualification is invalid")]
    InvalidPoolQualification,
    /// The per-request source bound is outside the V1 protocol range.
    #[error("provider-ingest source-pool request bound is invalid")]
    InvalidSourceLimit,
    /// A production pool must contain at least two and at most the V1 maximum sources.
    #[error("provider-ingest source-pool size is invalid")]
    InvalidSourceCount,
    /// A source exposed a zero provider identity.
    #[error("provider-ingest source-pool provider identity is invalid")]
    InvalidProviderId,
    /// A source's public runtime identity is invalid or aliases the pool.
    #[error("provider-ingest provider-source handle is invalid")]
    InvalidSourceHandle,
    /// A configured source qualification is unsupported or contains a zero pin.
    #[error("provider-ingest provider-source qualification is invalid")]
    InvalidSourceQualification,
    /// An injected source does not match its independent public binding.
    #[error("provider-ingest provider-source identity or qualification does not match")]
    SourceBindingMismatch,
    /// Two sources claimed the same governed provider.
    #[error("provider-ingest source-pool contains a duplicate provider")]
    DuplicateProvider,
    /// Two independently administered sources reused one runtime identity.
    #[error("provider-ingest source-pool contains a duplicate source handle")]
    DuplicateSourceHandle,
}

/// One independently bound authenticated provider source.
pub struct ProviderIngestAuthenticatedSourceRegistrationV1<Fetched: Send + 'static> {
    binding: ProviderIngestAuthenticatedSourceBindingV1,
    source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Fetched>>,
}

impl<Fetched: Send + 'static> fmt::Debug
    for ProviderIngestAuthenticatedSourceRegistrationV1<Fetched>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestAuthenticatedSourceRegistrationV1")
            .field("binding", &self.binding)
            .finish_non_exhaustive()
    }
}

impl<Fetched: Send + 'static> ProviderIngestAuthenticatedSourceRegistrationV1<Fetched> {
    /// Pair an independently configured public binding with one injected source.
    #[must_use]
    pub fn new(
        binding: ProviderIngestAuthenticatedSourceBindingV1,
        source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Fetched>>,
    ) -> Self {
        Self { binding, source }
    }
}

struct PinnedProviderIngestSourceV1<Fetched: Send + 'static> {
    binding: ProviderIngestAuthenticatedSourceBindingV1,
    source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Fetched>>,
}

/// Bounded, identity-pinned authenticated multi-provider source coordinator.
///
/// The pool freezes a canonical public provider inventory at construction,
/// retains an independent top-level revision/policy-digest qualification,
/// rejects missing or substituted sources before contacting any transport, and
/// tries only the exact canonical source list carried by finalized assignment
/// state. Each selected source is rechecked before and after its fetch. Source
/// locations, grants, credentials, and payload bytes remain inside the child
/// adapters and are never copied into pool metadata or durable state.
pub struct ProviderIngestAuthenticatedSourcePoolV1<Fetched: Send + 'static> {
    runtime_handle: String,
    qualification: ProviderIngestRuntimeProviderQualificationV1,
    max_sources_per_fetch: usize,
    provider_ids: Vec<[u8; 32]>,
    sources: BTreeMap<[u8; 32], PinnedProviderIngestSourceV1<Fetched>>,
}

impl<Fetched: Send + 'static> fmt::Debug for ProviderIngestAuthenticatedSourcePoolV1<Fetched> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestAuthenticatedSourcePoolV1")
            .field("runtime_handle", &self.runtime_handle)
            .field("qualification", &self.qualification)
            .field("max_sources_per_fetch", &self.max_sources_per_fetch)
            .field("provider_ids", &self.provider_ids)
            .finish_non_exhaustive()
    }
}

impl<Fetched: Send + 'static> ProviderIngestAuthenticatedSourcePoolV1<Fetched> {
    /// Construct a production pool from independently configured source bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when the pool bounds or any independently configured
    /// source identity, handle, qualification, or injected adapter is invalid.
    pub fn new(
        runtime_handle: impl Into<String>,
        qualification: ProviderIngestRuntimeProviderQualificationV1,
        max_sources_per_fetch: usize,
        sources: Vec<ProviderIngestAuthenticatedSourceRegistrationV1<Fetched>>,
    ) -> Result<Self, ProviderIngestAuthenticatedSourcePoolErrorV1> {
        let runtime_handle = runtime_handle.into();
        if !is_production_runtime_handle(&runtime_handle) {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidPoolHandle);
        }
        if !qualification.is_valid() {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidPoolQualification);
        }
        if max_sources_per_fetch == 0 || max_sources_per_fetch > MAX_REPLICATION_ORDER_ASSIGNMENTS {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceLimit);
        }
        if sources.len() < 2 || sources.len() > MAX_REPLICATION_ORDER_ASSIGNMENTS {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceCount);
        }

        let mut pinned_handles = BTreeSet::new();
        let mut pinned_sources = BTreeMap::new();
        for registration in sources {
            let binding = registration.binding;
            binding.validate(&runtime_handle)?;
            if !pinned_handles.insert(binding.runtime_handle.clone()) {
                return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateSourceHandle);
            }
            let source = registration.source;
            let qualification = source
                .qualification()
                .map_err(|_| ProviderIngestAuthenticatedSourcePoolErrorV1::SourceBindingMismatch)?;
            if source.provider_id() != binding.provider_id
                || source.runtime_handle() != binding.runtime_handle.as_str()
                || qualification != binding.qualification()
            {
                return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::SourceBindingMismatch);
            }
            let provider_id = binding.provider_id;
            let pinned = PinnedProviderIngestSourceV1 { binding, source };
            if pinned_sources.insert(provider_id, pinned).is_some() {
                return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateProvider);
            }
        }
        let provider_ids = pinned_sources.keys().copied().collect();
        Ok(Self {
            runtime_handle,
            qualification,
            max_sources_per_fetch,
            provider_ids,
            sources: pinned_sources,
        })
    }

    /// Stable public identity of the complete source pool.
    #[must_use]
    pub fn runtime_handle(&self) -> &str {
        &self.runtime_handle
    }

    /// Return the pool's exact top-level adapter qualification.
    #[must_use]
    pub const fn qualification(&self) -> ProviderIngestRuntimeProviderQualificationV1 {
        self.qualification
    }

    /// Canonical identity-pinned provider inventory.
    #[must_use]
    pub fn source_provider_ids(&self) -> &[[u8; 32]] {
        &self.provider_ids
    }

    /// Maximum sources admitted in one finalized fetch request.
    #[must_use]
    pub const fn max_sources_per_fetch(&self) -> usize {
        self.max_sources_per_fetch
    }

    /// Revalidate every pinned source without exposing request material.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderIngestSourceFetchErrorV1::Unavailable`] when no
    /// independently qualified source is currently ready, or
    /// [`ProviderIngestSourceFetchErrorV1::Rejected`] when any pinned source is
    /// substituted, stale, or policy-rejected.
    pub fn check_readiness(&self) -> Result<(), ProviderIngestSourceFetchErrorV1> {
        if !is_production_runtime_handle(&self.runtime_handle)
            || !self.qualification.is_valid()
            || self.provider_ids.len() < 2
            || self.provider_ids.len() != self.sources.len()
            || self.max_sources_per_fetch == 0
            || self.max_sources_per_fetch > MAX_REPLICATION_ORDER_ASSIGNMENTS
        {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        let mut ready = false;
        for provider_id in &self.provider_ids {
            let source = self
                .sources
                .get(provider_id)
                .ok_or(ProviderIngestSourceFetchErrorV1::Rejected)?;
            if self.source_is_ready(source)? {
                ready = true;
            }
        }
        if !ready {
            return Err(ProviderIngestSourceFetchErrorV1::Unavailable);
        }
        Ok(())
    }

    fn source_is_ready(
        &self,
        source: &PinnedProviderIngestSourceV1<Fetched>,
    ) -> Result<bool, ProviderIngestSourceFetchErrorV1> {
        let before_ready = match self.validate_source(source) {
            Ok(()) => true,
            Err(ProviderIngestSourceFetchErrorV1::Unavailable) => false,
            Err(
                ProviderIngestSourceFetchErrorV1::ContentRejected
                | ProviderIngestSourceFetchErrorV1::Rejected,
            ) => return Err(ProviderIngestSourceFetchErrorV1::Rejected),
        };
        let readiness = source.source.check_readiness();
        let after = self.validate_source(source);
        if matches!(
            readiness,
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected
                | ProviderIngestSourceFetchErrorV1::Rejected)
        ) || matches!(
            after,
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected
                | ProviderIngestSourceFetchErrorV1::Rejected)
        ) {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        Ok(before_ready && readiness.is_ok() && after.is_ok())
    }

    fn validate_source(
        &self,
        source: &PinnedProviderIngestSourceV1<Fetched>,
    ) -> Result<(), ProviderIngestSourceFetchErrorV1> {
        let actual_handle = source.source.runtime_handle();
        if source.binding.validate(&self.runtime_handle).is_err()
            || source.source.provider_id() != source.binding.provider_id
            || !is_production_runtime_handle(actual_handle)
            || actual_handle != source.binding.runtime_handle.as_str()
        {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        let actual_qualification = match source.source.qualification() {
            Ok(qualification) => qualification,
            Err(ProviderIngestSourceFetchErrorV1::Unavailable) => {
                return Err(ProviderIngestSourceFetchErrorV1::Unavailable);
            }
            Err(
                ProviderIngestSourceFetchErrorV1::ContentRejected
                | ProviderIngestSourceFetchErrorV1::Rejected,
            ) => return Err(ProviderIngestSourceFetchErrorV1::Rejected),
        };
        if actual_qualification != source.binding.qualification() {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        Ok(())
    }

    fn validate_request(
        &self,
        request: &ProviderIngestSourceRequestV1,
    ) -> Result<(), ProviderIngestSourceFetchErrorV1> {
        if request.source_provider_ids.is_empty()
            || request.source_provider_ids.len() > self.max_sources_per_fetch
            || request.source_provider_ids.iter().any(|provider_id| {
                *provider_id == [0; 32]
                    || *provider_id == request.authorization.provider_id()
                    || !self.sources.contains_key(provider_id)
            })
            || request
                .source_provider_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        Ok(())
    }
}

impl<Fetched: Send + 'static> ProviderIngestAuthenticatedSourceFetchV1
    for ProviderIngestAuthenticatedSourcePoolV1<Fetched>
{
    type Fetched = Fetched;

    fn fetch<'a>(
        &'a self,
        request: ProviderIngestSourceRequestV1,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>> {
        Box::pin(async move {
            self.validate_request(&request)?;
            let mut content_rejected = false;
            for provider_id in &request.source_provider_ids {
                let source = self
                    .sources
                    .get(provider_id)
                    .ok_or(ProviderIngestSourceFetchErrorV1::Rejected)?;
                if !self.source_is_ready(source)? {
                    continue;
                }
                let result = source
                    .source
                    .fetch_provider(request.authorization.clone())
                    .await;
                let post_fetch_ready = self.source_is_ready(source)?;
                match result {
                    Ok(fetched) if post_fetch_ready => return Ok(fetched),
                    Ok(_) => {}
                    Err(ProviderIngestSourceFetchErrorV1::Unavailable) => {}
                    Err(ProviderIngestSourceFetchErrorV1::ContentRejected) => {
                        content_rejected = true;
                    }
                    Err(ProviderIngestSourceFetchErrorV1::Rejected) => {
                        return Err(ProviderIngestSourceFetchErrorV1::Rejected);
                    }
                }
            }
            Err(if content_rejected {
                ProviderIngestSourceFetchErrorV1::ContentRejected
            } else {
                ProviderIngestSourceFetchErrorV1::Unavailable
            })
        })
    }
}

/// Local storage verification/persistence failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestLocalStorageErrorV1 {
    /// Local storage failed in a retryable manner.
    Retryable,
    /// Local storage permanently rejected exact verified material.
    Permanent,
}

/// Exact local storage boundary.
pub trait ProviderIngestLocalStorageV1<Fetched>: Send + Sync + 'static {
    /// Verify whether exact authorized material is already durable locally.
    fn verify_existing<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
    ) -> ProviderIngestFutureV1<'a, Result<Option<String>, ProviderIngestLocalStorageErrorV1>>;

    /// Atomically store verified material and return its canonical manifest ID.
    fn store<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        fetched: Fetched,
    ) -> ProviderIngestFutureV1<'a, Result<String, ProviderIngestLocalStorageErrorV1>>;
}

/// Request for one exact fee-quoted provider completion payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletionPayloadRequestV1 {
    /// Immutable provider/order/manifest authorization.
    pub authorization: FinalizedProviderIngestAuthorizationV1,
    /// Current finalized provider owner.
    pub provider_owner: AccountId,
    /// Exact finalized completion authority to compare again at commit.
    pub expected_authority: ProviderIngestCompletionAuthorityV1,
    /// Exact order-scoped assignment revision to compare again at commit.
    pub expected_assignment_revision: u64,
    /// Exact configured production chain identity.
    pub chain_id: ChainId,
    /// Authoritative completion epoch.
    pub completion_epoch: u64,
    /// Finalized baseline preceding signing.
    pub finalized_cursor: ProviderIngestFinalizedCursorV1,
}

/// Completion payload construction failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestCompletionPayloadErrorV1 {
    /// Fee quoting or payload construction is temporarily unavailable.
    Unavailable,
    /// Current policy rejects completion payload construction.
    Rejected,
}

/// Builds the exact bounded, fee-quoted transaction payload to sign.
pub trait ProviderIngestCompletionPayloadBuilderV1: Send + Sync + 'static {
    /// Build one exact completion payload.
    fn build_payload<'a>(
        &'a self,
        request: ProviderIngestCompletionPayloadRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1>,
    >;
}

/// Isolated signer failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestCompletionSignerErrorV1 {
    /// HSM/KMS signing is temporarily unavailable.
    Unavailable,
    /// The signer rejected an otherwise exact prepared operation.
    Rejected,
}

/// Payload-free public qualification of one provider-ingest completion signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletionSignerQualificationV1 {
    /// Qualification schema version.
    pub version: u8,
    /// Non-zero signer adapter and public-policy revision.
    pub adapter_revision: u64,
    /// Exact chain-authoritative signer policy identity and digest lineage.
    pub signer_policy: ProviderIngestCompletionSignerPolicyV1,
    /// Exact admitted signature algorithm.
    pub algorithm: Algorithm,
    /// Exact public key controlled by the external signer.
    pub public_key: PublicKey,
}

impl ProviderIngestCompletionSignerQualificationV1 {
    /// Construct a first-release completion-signer qualification.
    #[must_use]
    pub fn new(
        adapter_revision: u64,
        signer_policy: ProviderIngestCompletionSignerPolicyV1,
        algorithm: Algorithm,
        public_key: PublicKey,
    ) -> Self {
        Self {
            version: PROVIDER_INGEST_COMPLETION_SIGNER_QUALIFICATION_VERSION_V1,
            adapter_revision,
            signer_policy,
            algorithm,
            public_key,
        }
    }

    /// Validate the schema, revision, policy lineage, algorithm, and public key.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported schema, zero revision, invalid
    /// signer policy, unapproved algorithm, or algorithm/key mismatch.
    pub fn validate(&self) -> Result<(), ProviderIngestCompletionSignerBindingErrorV1> {
        let public_key = self.public_key.try_to_bytes().ok();
        if self.version != PROVIDER_INGEST_COMPLETION_SIGNER_QUALIFICATION_VERSION_V1
            || self.adapter_revision == 0
            || !self.signer_policy.is_valid()
            || !matches!(self.algorithm, Algorithm::Ed25519 | Algorithm::MlDsa)
            || public_key.is_none_or(|(algorithm, bytes)| {
                algorithm != self.algorithm
                    || bytes.is_empty()
                    || bytes.iter().all(|byte| *byte == 0)
            })
        {
            return Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerQualification);
        }
        Ok(())
    }

    /// Return whether the qualified public key is the exact single-key authority.
    #[must_use]
    pub fn matches_authority(&self, authority: &AccountId) -> bool {
        authority.try_signatory() == Some(&self.public_key)
    }
}

/// Independently configured public binding for one completion signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletionSignerBindingV1 {
    /// Stable opaque HSM/KMS signer or key handle.
    pub runtime_handle: String,
    /// Exact payload-free signer qualification.
    pub qualification: ProviderIngestCompletionSignerQualificationV1,
}

impl ProviderIngestCompletionSignerBindingV1 {
    /// Pair a stable public handle with the required signer qualification.
    #[must_use]
    pub fn new(
        runtime_handle: impl Into<String>,
        qualification: ProviderIngestCompletionSignerQualificationV1,
    ) -> Self {
        Self {
            runtime_handle: runtime_handle.into(),
            qualification,
        }
    }

    /// Validate the complete configured signer binding.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-production handle or invalid qualification.
    pub fn validate(&self) -> Result<(), ProviderIngestCompletionSignerBindingErrorV1> {
        if !is_production_runtime_handle(&self.runtime_handle) {
            return Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerHandle);
        }
        self.qualification.validate()
    }
}

/// Invalid public binding for a provider-ingest completion signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProviderIngestCompletionSignerBindingErrorV1 {
    /// The signer handle is missing, test-marked, or otherwise not production-safe.
    #[error("provider-ingest completion-signer handle is invalid")]
    InvalidSignerHandle,
    /// The signer qualification is unsupported, zero, malformed, or inconsistent.
    #[error("provider-ingest completion-signer qualification is invalid")]
    InvalidSignerQualification,
}

/// Isolated runtime signer that has no queue or outbox access.
pub trait ProviderIngestCompletionSignerV1: Send + Sync + 'static {
    /// Stable public HSM/KMS signer or key handle.
    fn runtime_handle(&self) -> &str;

    /// Account controlled by this signer.
    fn authority(&self) -> &AccountId;

    /// Current payload-free signer qualification.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free signer failure when the signer
    /// qualification cannot be authenticated.
    fn qualification(
        &self,
    ) -> Result<ProviderIngestCompletionSignerQualificationV1, ProviderIngestCompletionSignerErrorV1>;

    /// Exact governed policy identity under which this signer is currently
    /// eligible. Implementations must change this value on key rotation and
    /// reject signing atomically when the policy is revoked or superseded.
    fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1;

    /// Revalidate the live owner/key/policy authority represented by this
    /// signer and return its exact current policy identity.
    ///
    /// This method runs on the async worker thread and therefore must be a
    /// bounded, non-blocking read of a locally maintained eligibility snapshot;
    /// it must never perform HSM/KMS, network, filesystem, or other blocking
    /// I/O. Implementations must update that snapshot on revocation/rotation
    /// and fail closed when it is stale or unavailable. The timed [`Self::sign`]
    /// operation remains responsible for the HSM/KMS-side atomic check.
    fn current_eligibility(
        &self,
    ) -> Result<ProviderIngestCompletionSignerPolicyV1, ProviderIngestCompletionSignerErrorV1>;

    /// Sign exactly the supplied payload without rewriting any field.
    fn sign<'a>(
        &'a self,
        payload: TransactionPayload,
    ) -> ProviderIngestFutureV1<'a, Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentSignerPolicyErrorV1 {
    Unavailable,
    Ineligible,
    ProtocolViolation,
}

fn exact_current_signer_policy<Signer: ProviderIngestCompletionSignerV1>(
    signer: &Signer,
    expected_owner: &AccountId,
) -> Result<ProviderIngestCompletionSignerPolicyV1, CurrentSignerPolicyErrorV1> {
    if signer.authority() != expected_owner
        || !is_production_runtime_handle(signer.runtime_handle())
    {
        return Err(CurrentSignerPolicyErrorV1::ProtocolViolation);
    }
    let qualification = signer.qualification().map_err(|error| match error {
        ProviderIngestCompletionSignerErrorV1::Unavailable => {
            CurrentSignerPolicyErrorV1::Unavailable
        }
        ProviderIngestCompletionSignerErrorV1::Rejected => CurrentSignerPolicyErrorV1::Ineligible,
    })?;
    let policy = signer.current_eligibility().map_err(|error| match error {
        ProviderIngestCompletionSignerErrorV1::Unavailable => {
            CurrentSignerPolicyErrorV1::Unavailable
        }
        ProviderIngestCompletionSignerErrorV1::Rejected => CurrentSignerPolicyErrorV1::Ineligible,
    })?;
    if qualification.validate().is_err()
        || !qualification.matches_authority(expected_owner)
        || !policy.is_valid()
    {
        return Err(CurrentSignerPolicyErrorV1::ProtocolViolation);
    }
    if signer.signer_policy() != policy || qualification.signer_policy != policy {
        return Err(CurrentSignerPolicyErrorV1::Ineligible);
    }
    Ok(policy)
}

/// Signer resolution failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestCompletionSignerResolverErrorV1 {
    /// Signer discovery is temporarily unavailable.
    Unavailable,
    /// The requested finalized owner is revoked or disallowed.
    Rejected,
}

/// Resolves the signer for the exact current finalized provider owner.
pub trait ProviderIngestCompletionSignerResolverV1: Send + Sync + 'static {
    /// Isolated signer implementation.
    type Signer: ProviderIngestCompletionSignerV1;

    /// Resolve an eligible signer for `provider_owner` at `finalized_cursor`.
    fn resolve<'a>(
        &'a self,
        provider_owner: AccountId,
        finalized_cursor: ProviderIngestFinalizedCursorV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<Option<Self::Signer>, ProviderIngestCompletionSignerResolverErrorV1>,
    >;
}

/// Queue preflight failure that occurs before transaction exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestIngressPrepareErrorV1 {
    /// Queue preflight is temporarily unavailable.
    Unavailable,
    /// The exact transaction was terminally rejected before exposure.
    Rejected,
}

/// Outcome after an exact transaction may have been exposed to ingress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestIngressDispositionV1 {
    /// The exact transaction is pending or applied.
    Submitted,
    /// The adapter proves exposure did not reach the queue.
    DefinitelyNotSubmitted,
    /// The exact transaction was terminally rejected.
    Rejected,
    /// Exposure may have happened and requires reconciliation.
    Ambiguous,
}

/// Observation of one exact retained transaction hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestTransactionObservationV1 {
    /// The exact transaction committed and execution succeeded.
    ///
    /// This proves only the transaction-level outcome. The finalized
    /// replication-order projection remains the sole semantic completion
    /// authority.
    CommittedSuccess,
    /// The exact transaction committed but execution was rejected.
    CommittedRejected,
    /// The exact transaction remains pending or applied but unfinalized.
    Pending,
    /// The exact transaction is absent from the observed finalized/pipeline view.
    Unknown,
    /// The observation service is temporarily unavailable.
    Unavailable,
}

/// Transaction ingress split into preflight and post-durable exposure phases.
pub trait ProviderIngestTransactionIngressV1: Send + Sync + 'static {
    /// Opaque prepared queue operation that has not exposed transaction bytes.
    type Prepared: Send + 'static;

    /// Validate and prepare queue admission without exposing transaction bytes.
    fn prepare<'a>(
        &'a self,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Prepared, ProviderIngestIngressPrepareErrorV1>>;

    /// Expose the exact transaction only after the durable ambiguous transition.
    fn expose<'a>(
        &'a self,
        prepared: Self::Prepared,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, ProviderIngestIngressDispositionV1>;

    /// Observe one exact retained transaction hash without mutating ingress.
    ///
    /// A committed observation must include the execution result; block/hash
    /// membership alone is not a successful semantic completion signal.
    fn observe<'a>(
        &'a self,
        transaction_hash: [u8; 32],
    ) -> ProviderIngestFutureV1<'a, ProviderIngestTransactionObservationV1>;
}

/// Runtime clock used only for leases, backoff, and timeouts.
pub trait ProviderIngestClockV1: Send + Sync + 'static {
    /// Current runtime time in milliseconds.
    fn now_ms(&self) -> u64;
}

/// Wall-clock implementation for production runtime use.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProviderIngestSystemClockV1;

impl ProviderIngestClockV1 for ProviderIngestSystemClockV1 {
    fn now_ms(&self) -> u64 {
        u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(u64::MAX)
    }
}

/// Payload-free counters for one bounded runtime tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderIngestTickOutcomeV1 {
    /// Finalized rows validated.
    pub rows_scanned: usize,
    /// New durable jobs admitted.
    pub jobs_inserted: usize,
    /// Jobs reconciled to semantic finalized completion.
    pub jobs_finalized: usize,
    /// Jobs cancelled from finalized state.
    pub jobs_cancelled: usize,
    /// Source jobs claimed in this tick.
    pub source_jobs_claimed: usize,
    /// Exact manifests confirmed or stored locally.
    pub manifests_stored: usize,
    /// Completion transactions durably signed.
    pub completions_signed: usize,
    /// Completion transaction exposure attempts.
    pub completion_submissions: usize,
}

/// Supervised provider-ingest runtime.
pub struct ProviderIngestRuntimeV1<Ledger, Fetch, Storage, Builder, Resolver, Ingress, Clock>
where
    Ledger: ProviderIngestFinalizedLedgerV1,
    Fetch: ProviderIngestAuthenticatedSourceFetchV1,
    Storage: ProviderIngestLocalStorageV1<Fetch::Fetched>,
    Builder: ProviderIngestCompletionPayloadBuilderV1,
    Resolver: ProviderIngestCompletionSignerResolverV1,
    Ingress: ProviderIngestTransactionIngressV1,
    Clock: ProviderIngestClockV1,
{
    provider_id: [u8; 32],
    chain_id: ChainId,
    claim_owner: ProviderIngestClaimOwnerV1,
    policy: ProviderIngestRuntimePolicyV1,
    outbox: ProviderIngestOutbox,
    ledger: Arc<Ledger>,
    fetch: Arc<Fetch>,
    storage: Arc<Storage>,
    payload_builder: Arc<Builder>,
    signer_resolver: Arc<Resolver>,
    ingress: Arc<Ingress>,
    clock: Arc<Clock>,
    last_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
    scan_cursor: Option<ProviderIngestFinalizedCursorV1>,
    scan_after_order_id: Option<[u8; 32]>,
}

impl<Ledger, Fetch, Storage, Builder, Resolver, Ingress, Clock>
    ProviderIngestRuntimeV1<Ledger, Fetch, Storage, Builder, Resolver, Ingress, Clock>
where
    Ledger: ProviderIngestFinalizedLedgerV1,
    Fetch: ProviderIngestAuthenticatedSourceFetchV1,
    Storage: ProviderIngestLocalStorageV1<Fetch::Fetched>,
    Builder: ProviderIngestCompletionPayloadBuilderV1,
    Resolver: ProviderIngestCompletionSignerResolverV1,
    Ingress: ProviderIngestTransactionIngressV1,
    Clock: ProviderIngestClockV1,
{
    /// Construct a bounded runtime from explicit production dependencies.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider_id: [u8; 32],
        chain_id: ChainId,
        claim_owner: ProviderIngestClaimOwnerV1,
        policy: ProviderIngestRuntimePolicyV1,
        outbox: ProviderIngestOutbox,
        ledger: Arc<Ledger>,
        fetch: Arc<Fetch>,
        storage: Arc<Storage>,
        payload_builder: Arc<Builder>,
        signer_resolver: Arc<Resolver>,
        ingress: Arc<Ingress>,
        clock: Arc<Clock>,
    ) -> Result<Self, ProviderIngestRuntimeErrorV1> {
        if provider_id == [0; 32] {
            return Err(ProviderIngestRuntimeErrorV1::InvalidProviderId);
        }
        policy.validate(&outbox)?;
        let last_finalized_cursor = outbox.finalized_cursor_high_water()?;
        Ok(Self {
            provider_id,
            chain_id,
            claim_owner,
            policy,
            outbox,
            ledger,
            fetch,
            storage,
            payload_builder,
            signer_resolver,
            ingress,
            clock,
            last_finalized_cursor,
            scan_cursor: None,
            scan_after_order_id: None,
        })
    }

    /// Run until shutdown or a fatal supervised-runtime error.
    pub async fn run(
        mut self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        if *shutdown.borrow() {
            return Ok(());
        }
        let mut interval =
            tokio::time::interval(Duration::from_millis(self.policy.scan_interval_ms));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(());
                    }
                }
                _ = interval.tick() => {
                    let shutdown_requested = std::sync::atomic::AtomicBool::new(false);
                    let tick = self.tick_with_shutdown(&shutdown_requested);
                    tokio::pin!(tick);
                    let mut stop_after_tick = false;
                    loop {
                        tokio::select! {
                            result = &mut tick => {
                                result?;
                                break;
                            }
                            changed = shutdown.changed(), if !stop_after_tick => {
                                if changed.is_err() || *shutdown.borrow() {
                                    shutdown_requested.store(
                                        true,
                                        std::sync::atomic::Ordering::Release,
                                    );
                                    stop_after_tick = true;
                                }
                            }
                        }
                    }
                    if stop_after_tick {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Execute one bounded finalized scan and delivery pass.
    pub async fn tick(
        &mut self,
    ) -> Result<ProviderIngestTickOutcomeV1, ProviderIngestRuntimeErrorV1> {
        self.tick_inner(None).await
    }

    /// Execute one bounded scan while honoring a cooperative shutdown request.
    ///
    /// The current row always runs to a durable boundary. A request observed
    /// between rows or pages prevents additional work from starting, so callers
    /// may keep polling this same future after selecting their shutdown signal
    /// without detaching a source claim or an in-flight storage mutation.
    pub async fn tick_with_shutdown(
        &mut self,
        shutdown_requested: &std::sync::atomic::AtomicBool,
    ) -> Result<ProviderIngestTickOutcomeV1, ProviderIngestRuntimeErrorV1> {
        self.tick_inner(Some(shutdown_requested)).await
    }

    async fn tick_inner(
        &mut self,
        shutdown_requested: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<ProviderIngestTickOutcomeV1, ProviderIngestRuntimeErrorV1> {
        let mut outcome = ProviderIngestTickOutcomeV1::default();
        let mut source_budget = self.policy.max_source_jobs_per_tick;
        let mut after = self.scan_after_order_id;
        let mut expected_cursor = self.scan_cursor;
        let mut recovered_interrupted_signing = false;

        for _ in 0..self.policy.max_pages_per_tick {
            if shutdown_requested
                .is_some_and(|requested| requested.load(std::sync::atomic::Ordering::Acquire))
            {
                return Ok(outcome);
            }
            let page = self
                .ledger
                .read_assignment_page(expected_cursor, after, self.policy.max_page_rows)
                .await
                .map_err(|_| ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable)?;
            if after.is_some()
                && expected_cursor.is_some_and(|cursor| cursor != page.finalized_cursor)
            {
                return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
            }
            let cursor = expected_cursor.unwrap_or(page.finalized_cursor);
            validate_page(&page, after, cursor, self.policy.max_page_rows)?;
            validate_monotonic_finalized_cursor(self.last_finalized_cursor, cursor)?;
            self.outbox
                .observe_finalized_snapshot(cursor, page.finalized_block_time_ms)?;
            self.last_finalized_cursor = Some(cursor);
            expected_cursor = Some(cursor);
            if !recovered_interrupted_signing {
                self.outbox
                    .recover_expired_completion_signing(self.clock.now_ms(), cursor)?;
                recovered_interrupted_signing = true;
            }

            let finalized_block_time_ms = page.finalized_block_time_ms;
            for row in page.rows {
                if shutdown_requested
                    .is_some_and(|requested| requested.load(std::sync::atomic::Ordering::Acquire))
                {
                    return Ok(outcome);
                }
                outcome.rows_scanned = outcome.rows_scanned.saturating_add(1);
                self.process_row(
                    row,
                    cursor,
                    finalized_block_time_ms,
                    &mut source_budget,
                    &mut outcome,
                )
                .await?;
            }

            after = page.next_after_order_id;
            if after.is_none() {
                self.scan_after_order_id = None;
                self.scan_cursor = None;
                return Ok(outcome);
            }
        }

        self.scan_after_order_id = after;
        self.scan_cursor = expected_cursor;
        Ok(outcome)
    }

    #[allow(clippy::too_many_lines)]
    async fn process_row(
        &self,
        row: ProviderIngestFinalizedAssignmentV1,
        cursor: ProviderIngestFinalizedCursorV1,
        finalized_block_time_ms: u64,
        source_budget: &mut usize,
        outcome: &mut ProviderIngestTickOutcomeV1,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let ValidatedAssignmentV1 {
            authorization,
            source_provider_ids,
        } = validate_assignment(&row, cursor, self.provider_id, self.policy)?;
        let job_id = authorization.job_id();
        let provider_id = ProviderId::new(self.provider_id);

        if let Some(completion) = row.order.provider_completion(provider_id) {
            self.outbox.reconcile_finalized_completion(
                authorization,
                ProviderIngestFinalizedCompletionV1 {
                    finalized_cursor: cursor,
                    provider_id: self.provider_id,
                    order_id: *row.order.order_id.as_bytes(),
                    manifest_digest: *row.order.manifest_digest.as_bytes(),
                    completion_epoch: completion.completion_epoch,
                    completed_by: completion.completed_by.clone(),
                    committed_transaction_hash: row.committed_transaction_hash,
                },
            )?;
            outcome.jobs_finalized = outcome.jobs_finalized.saturating_add(1);
            return Ok(());
        }

        let cancellation_reason = match (&row.pin.manifest.status, &row.order.status) {
            (PinStatus::Retired(_), _) => Some(ProviderIngestCancellationReasonV1::ManifestRetired),
            (_, ReplicationOrderStatus::Expired(_)) => {
                Some(ProviderIngestCancellationReasonV1::OrderExpired)
            }
            (_, ReplicationOrderStatus::Completed(_)) => {
                Some(ProviderIngestCancellationReasonV1::OrderCompletedByOther)
            }
            _ => None,
        };
        if let Some(reason) = cancellation_reason {
            self.outbox.reconcile_finalized_cancellation(
                authorization,
                ProviderIngestFinalizedCancellationV1 {
                    finalized_cursor: cursor,
                    provider_id: self.provider_id,
                    order_id: *row.order.order_id.as_bytes(),
                    manifest_digest: *row.order.manifest_digest.as_bytes(),
                    reason,
                },
            )?;
            outcome.jobs_cancelled = outcome.jobs_cancelled.saturating_add(1);
            return Ok(());
        }
        if !matches!(row.pin.manifest.status, PinStatus::Approved(_))
            || !matches!(row.order.status, ReplicationOrderStatus::Pending)
        {
            return Ok(());
        }

        let enqueue = self.outbox.enqueue(authorization.clone())?;
        if matches!(
            enqueue,
            crate::provider_ingest_outbox::ProviderIngestEnqueueResultV1::Inserted { .. }
        ) {
            outcome.jobs_inserted = outcome.jobs_inserted.saturating_add(1);
        }
        let status = self.outbox.status(job_id)?;
        match status.state {
            ProviderIngestDeliveryStateV1::PendingSource { .. }
            | ProviderIngestDeliveryStateV1::RetryScheduled { .. }
            | ProviderIngestDeliveryStateV1::SourceClaimed { .. }
                if *source_budget != 0 =>
            {
                if self
                    .process_source(authorization, source_provider_ids, cursor, outcome)
                    .await?
                {
                    *source_budget -= 1;
                    if let Ok(status) = self.outbox.status(job_id)
                        && let ProviderIngestDeliveryStateV1::LocalStored { completion, .. } =
                            status.state
                    {
                        self.process_completion(
                            &row,
                            status.job_id,
                            completion,
                            cursor,
                            finalized_block_time_ms,
                            outcome,
                        )
                        .await?;
                    }
                }
            }
            ProviderIngestDeliveryStateV1::LocalStored { completion, .. } => {
                self.process_completion(
                    &row,
                    status.job_id,
                    completion,
                    cursor,
                    finalized_block_time_ms,
                    outcome,
                )
                .await?;
            }
            ProviderIngestDeliveryStateV1::PendingSource { .. }
            | ProviderIngestDeliveryStateV1::RetryScheduled { .. }
            | ProviderIngestDeliveryStateV1::SourceClaimed { .. }
            | ProviderIngestDeliveryStateV1::FinalizedCompleted { .. }
            | ProviderIngestDeliveryStateV1::Cancelled { .. }
            | ProviderIngestDeliveryStateV1::DeadLetter { .. } => {}
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn process_source(
        &self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        source_provider_ids: Vec<[u8; 32]>,
        cursor: ProviderIngestFinalizedCursorV1,
        outcome: &mut ProviderIngestTickOutcomeV1,
    ) -> Result<bool, ProviderIngestRuntimeErrorV1> {
        let claim = match self.outbox.claim_source(
            authorization.job_id(),
            self.claim_owner,
            self.clock.now_ms(),
            cursor,
        ) {
            Ok(claim) => claim,
            Err(
                ProviderIngestOutboxError::RetryNotDue
                | ProviderIngestOutboxError::LeaseAlreadyHeld
                | ProviderIngestOutboxError::InvalidTransition
                | ProviderIngestOutboxError::RetryExhausted,
            ) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        outcome.source_jobs_claimed = outcome.source_jobs_claimed.saturating_add(1);

        let verify = self.storage.verify_existing(authorization.clone());
        let (claim, existing) = self.await_with_lease(claim, cursor, verify).await?;
        match existing {
            LeaseOperationOutcomeV1::Completed(Ok(Some(manifest_id))) => {
                if let Err(error) =
                    self.outbox
                        .mark_local_stored(&claim, self.clock.now_ms(), manifest_id)
                {
                    if error == ProviderIngestOutboxError::InvalidManifestId {
                        self.outbox.dead_letter_source(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                            ProviderIngestDeadLetterReasonV1::StorageRejected,
                            ProviderIngestFailureClassV1::StorageRejected,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::StorageProtocolViolation);
                    }
                    return Err(error.into());
                }
                outcome.manifests_stored = outcome.manifests_stored.saturating_add(1);
                return Ok(true);
            }
            LeaseOperationOutcomeV1::Completed(Ok(None)) => {}
            LeaseOperationOutcomeV1::Completed(Err(error)) => {
                self.handle_storage_failure(claim, cursor, error)?;
                return Ok(true);
            }
            LeaseOperationOutcomeV1::TimedOut => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::StorageRejected,
                )?;
                return Ok(true);
            }
        }

        let request = ProviderIngestSourceRequestV1 {
            authorization: authorization.clone(),
            source_provider_ids,
        };
        let fetch = self.fetch.fetch(request);
        let (claim, fetched) = self.await_with_lease(claim, cursor, fetch).await?;
        let fetched = match fetched {
            LeaseOperationOutcomeV1::Completed(Ok(fetched)) => fetched,
            LeaseOperationOutcomeV1::Completed(Err(
                ProviderIngestSourceFetchErrorV1::Unavailable,
            ))
            | LeaseOperationOutcomeV1::TimedOut => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::SourceUnavailable,
                )?;
                return Ok(true);
            }
            LeaseOperationOutcomeV1::Completed(Err(
                ProviderIngestSourceFetchErrorV1::ContentRejected,
            )) => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::SourceRejected,
                )?;
                return Ok(true);
            }
            LeaseOperationOutcomeV1::Completed(Err(ProviderIngestSourceFetchErrorV1::Rejected)) => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::SourceRejected,
                )?;
                return Err(ProviderIngestRuntimeErrorV1::SourceProtocolViolation);
            }
        };

        let store = self.storage.store(authorization, fetched);
        let (claim, stored) = self
            .await_mutating_storage_with_lease(claim, cursor, store)
            .await?;
        let stored = match stored {
            MutatingStorageOutcomeV1::Completed(output)
            | MutatingStorageOutcomeV1::CompletedAfterSoftTimeout(output) => output,
        };
        match stored {
            Ok(manifest_id) => {
                if let Err(error) =
                    self.outbox
                        .mark_local_stored(&claim, self.clock.now_ms(), manifest_id)
                {
                    if error == ProviderIngestOutboxError::InvalidManifestId {
                        self.outbox.dead_letter_source(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                            ProviderIngestDeadLetterReasonV1::StorageRejected,
                            ProviderIngestFailureClassV1::StorageRejected,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::StorageProtocolViolation);
                    }
                    return Err(error.into());
                }
                outcome.manifests_stored = outcome.manifests_stored.saturating_add(1);
            }
            Err(error) => {
                self.handle_storage_failure(claim, cursor, error)?;
            }
        }
        Ok(true)
    }

    fn handle_storage_failure(
        &self,
        claim: ProviderIngestSourceClaimV1,
        cursor: ProviderIngestFinalizedCursorV1,
        error: ProviderIngestLocalStorageErrorV1,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        match error {
            ProviderIngestLocalStorageErrorV1::Retryable => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::StorageRejected,
                )?;
            }
            ProviderIngestLocalStorageErrorV1::Permanent => {
                self.outbox.dead_letter_source(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestDeadLetterReasonV1::StorageRejected,
                    ProviderIngestFailureClassV1::StorageRejected,
                )?;
            }
        }
        Ok(())
    }

    async fn await_with_lease<T, Fut>(
        &self,
        mut claim: ProviderIngestSourceClaimV1,
        cursor: ProviderIngestFinalizedCursorV1,
        future: Fut,
    ) -> Result<
        (ProviderIngestSourceClaimV1, LeaseOperationOutcomeV1<T>),
        ProviderIngestRuntimeErrorV1,
    >
    where
        Fut: Future<Output = T> + Send,
    {
        let future = future;
        tokio::pin!(future);
        let timeout = tokio::time::sleep(Duration::from_millis(
            self.policy.source_operation_timeout_ms,
        ));
        tokio::pin!(timeout);
        let renew_period = Duration::from_millis(self.policy.source_lease_renew_interval_ms);
        let mut renewal =
            tokio::time::interval_at(tokio::time::Instant::now() + renew_period, renew_period);
        renewal.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                biased;
                _ = renewal.tick() => {
                    claim = self.outbox.renew_source_claim(
                        &claim,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                }
                output = &mut future => {
                    return Ok((claim, LeaseOperationOutcomeV1::Completed(output)));
                }
                _ = &mut timeout => {
                    return Ok((claim, LeaseOperationOutcomeV1::TimedOut));
                }
            }
        }
    }

    /// Await an in-flight atomic storage mutation without ever detaching it.
    ///
    /// The configured operation timeout is a soft diagnostic boundary for
    /// mutating storage. Once storage may be writing, the runtime keeps the
    /// durable claim renewed and waits for the exact operation to finish before
    /// it can persist success or schedule a retry. This prevents a timed-out
    /// blocking writer from racing a replacement attempt.
    async fn await_mutating_storage_with_lease<T, Fut>(
        &self,
        mut claim: ProviderIngestSourceClaimV1,
        cursor: ProviderIngestFinalizedCursorV1,
        future: Fut,
    ) -> Result<
        (ProviderIngestSourceClaimV1, MutatingStorageOutcomeV1<T>),
        ProviderIngestRuntimeErrorV1,
    >
    where
        Fut: Future<Output = T> + Send,
    {
        let future = future;
        tokio::pin!(future);
        let soft_timeout = tokio::time::sleep(Duration::from_millis(
            self.policy.source_operation_timeout_ms,
        ));
        tokio::pin!(soft_timeout);
        let renew_period = Duration::from_millis(self.policy.source_lease_renew_interval_ms);
        let mut renewal =
            tokio::time::interval_at(tokio::time::Instant::now() + renew_period, renew_period);
        renewal.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut exceeded_soft_timeout = false;
        loop {
            tokio::select! {
                biased;
                _ = renewal.tick() => {
                    claim = self.outbox.renew_source_claim(
                        &claim,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                }
                output = &mut future => {
                    let outcome = if exceeded_soft_timeout {
                        MutatingStorageOutcomeV1::CompletedAfterSoftTimeout(output)
                    } else {
                        MutatingStorageOutcomeV1::Completed(output)
                    };
                    return Ok((claim, outcome));
                }
                _ = &mut soft_timeout, if !exceeded_soft_timeout => {
                    exceeded_soft_timeout = true;
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn process_completion(
        &self,
        row: &ProviderIngestFinalizedAssignmentV1,
        job_id: [u8; 32],
        completion: ProviderIngestCompletionStateV1,
        cursor: ProviderIngestFinalizedCursorV1,
        finalized_block_time_ms: u64,
        outcome: &mut ProviderIngestTickOutcomeV1,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let mut completion = completion;
        let mut exposed_absent_transaction = None;
        let completion_authority = row.completion_authority.as_ref().filter(|binding| {
            binding.is_valid() && row.provider_owner.as_ref() == Some(&binding.provider_owner)
        });

        // Bytes that may already have crossed the queue boundary are always
        // reconciled by exact hash before any signer/HSM dependency is queried.
        match &completion {
            ProviderIngestCompletionStateV1::Ambiguous {
                baseline_finalized_cursor,
                transaction_hash,
                ..
            } => {
                self.reconcile_transaction(
                    job_id,
                    *transaction_hash,
                    *baseline_finalized_cursor,
                    cursor,
                    true,
                )
                .await?;
                return Ok(());
            }
            ProviderIngestCompletionStateV1::Submitted {
                baseline_finalized_cursor,
                transaction_hash,
                ..
            } => {
                self.reconcile_transaction(
                    job_id,
                    *transaction_hash,
                    *baseline_finalized_cursor,
                    cursor,
                    false,
                )
                .await?;
                return Ok(());
            }
            ProviderIngestCompletionStateV1::Ready { .. }
            | ProviderIngestCompletionStateV1::Signing { .. }
            | ProviderIngestCompletionStateV1::Signed { .. } => {}
        }

        if let ProviderIngestCompletionStateV1::Signed {
            baseline_finalized_cursor,
            transaction_hash,
            ever_exposed: true,
            ..
        } = &completion
        {
            let observation = tokio::time::timeout(
                Duration::from_millis(self.policy.ingress_timeout_ms),
                self.ingress.observe(*transaction_hash),
            )
            .await
            .unwrap_or(ProviderIngestTransactionObservationV1::Unavailable);
            match observation {
                ProviderIngestTransactionObservationV1::CommittedSuccess
                | ProviderIngestTransactionObservationV1::Pending => {
                    self.outbox
                        .mark_exposed_completion_observed(job_id, *transaction_hash)?;
                    return Ok(());
                }
                ProviderIngestTransactionObservationV1::CommittedRejected => {
                    self.outbox.mark_completion_transaction_rejected(
                        job_id,
                        *transaction_hash,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                    return Ok(());
                }
                ProviderIngestTransactionObservationV1::Unknown => {
                    if cursor.height > baseline_finalized_cursor.height {
                        exposed_absent_transaction = Some(*transaction_hash);
                    }
                }
                ProviderIngestTransactionObservationV1::Unavailable => return Ok(()),
            }
        }

        let mut submission_authority = None;
        let mut checked_signer_policy = None;
        if matches!(
            &completion,
            ProviderIngestCompletionStateV1::Signing { .. }
                | ProviderIngestCompletionStateV1::Signed { .. }
        ) {
            if completion_authority.is_none() {
                self.outbox.invalidate_stale_completion_authority(
                    job_id,
                    row.provider_owner.as_ref(),
                    ProviderIngestSignerPolicyObservationV1::Missing,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
            self.outbox.invalidate_stale_completion_authority(
                job_id,
                row.provider_owner.as_ref(),
                ProviderIngestSignerPolicyObservationV1::NotChecked,
                self.clock.now_ms(),
                cursor,
            )?;
            completion = match self.outbox.status(job_id)?.state {
                ProviderIngestDeliveryStateV1::LocalStored { completion, .. } => completion,
                ProviderIngestDeliveryStateV1::PendingSource { .. }
                | ProviderIngestDeliveryStateV1::SourceClaimed { .. }
                | ProviderIngestDeliveryStateV1::RetryScheduled { .. }
                | ProviderIngestDeliveryStateV1::FinalizedCompleted { .. }
                | ProviderIngestDeliveryStateV1::Cancelled { .. }
                | ProviderIngestDeliveryStateV1::DeadLetter { .. } => return Ok(()),
            };
        }
        if matches!(
            &completion,
            ProviderIngestCompletionStateV1::Signing { .. }
                | ProviderIngestCompletionStateV1::Signed { .. }
        ) {
            if let Some(provider_owner) = row.provider_owner.clone() {
                let Some(expected_policy) =
                    completion_authority.map(|binding| binding.signer_policy)
                else {
                    return Ok(());
                };
                let signer_policy_observation = match tokio::time::timeout(
                    Duration::from_millis(self.policy.signer_timeout_ms),
                    self.signer_resolver.resolve(provider_owner.clone(), cursor),
                )
                .await
                {
                    Ok(Ok(Some(signer))) => {
                        match exact_current_signer_policy(&signer, &provider_owner) {
                            Ok(signer_policy) => {
                                if signer_policy == expected_policy {
                                    submission_authority =
                                        Some((provider_owner.clone(), signer_policy));
                                }
                                ProviderIngestSignerPolicyObservationV1::Active(signer_policy)
                            }
                            Err(CurrentSignerPolicyErrorV1::Ineligible) => {
                                ProviderIngestSignerPolicyObservationV1::Missing
                            }
                            Err(CurrentSignerPolicyErrorV1::Unavailable) => return Ok(()),
                            Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                                return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                            }
                        }
                    }
                    Ok(Ok(None))
                    | Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)) => {
                        ProviderIngestSignerPolicyObservationV1::Missing
                    }
                    Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable))
                    | Err(_) => {
                        return Ok(());
                    }
                };
                checked_signer_policy = Some(signer_policy_observation);
                self.outbox.invalidate_stale_completion_authority(
                    job_id,
                    Some(&provider_owner),
                    signer_policy_observation,
                    self.clock.now_ms(),
                    cursor,
                )?;
                completion = match self.outbox.status(job_id)?.state {
                    ProviderIngestDeliveryStateV1::LocalStored { completion, .. } => completion,
                    ProviderIngestDeliveryStateV1::PendingSource { .. }
                    | ProviderIngestDeliveryStateV1::SourceClaimed { .. }
                    | ProviderIngestDeliveryStateV1::RetryScheduled { .. }
                    | ProviderIngestDeliveryStateV1::FinalizedCompleted { .. }
                    | ProviderIngestDeliveryStateV1::Cancelled { .. }
                    | ProviderIngestDeliveryStateV1::DeadLetter { .. } => return Ok(()),
                };
            } else {
                checked_signer_policy = Some(ProviderIngestSignerPolicyObservationV1::NotChecked);
            }
        }
        if let Some(transaction_hash) = exposed_absent_transaction
            && self
                .outbox
                .expire_absent_exposed_completion(ProviderIngestExposedCompletionExpiryV1 {
                    job_id,
                    expected_transaction_hash: transaction_hash,
                    current_provider_owner: row.provider_owner.as_ref(),
                    current_signer_policy: checked_signer_policy
                        .unwrap_or(ProviderIngestSignerPolicyObservationV1::NotChecked),
                    runtime_now_ms: self.clock.now_ms(),
                    finalized_block_time_ms,
                    observed_finalized_cursor: cursor,
                })?
                .is_some()
        {
            return Ok(());
        }
        match completion {
            ProviderIngestCompletionStateV1::Ready {
                next_attempt_at_ms, ..
            } => {
                let (Some(completion_authority), Some(completion_epoch)) =
                    (completion_authority.cloned(), row.completion_epoch)
                else {
                    return Ok(());
                };
                let provider_owner = completion_authority.provider_owner.clone();
                if completion_epoch < row.order.issued_epoch
                    || completion_epoch > row.order.deadline_epoch
                {
                    return Ok(());
                }
                let owner_changed = self.outbox.reconcile_ready_completion_owner(
                    job_id,
                    &provider_owner,
                    cursor,
                )?;
                if !owner_changed && self.clock.now_ms() < next_attempt_at_ms {
                    return Ok(());
                }
                let signer = match tokio::time::timeout(
                    Duration::from_millis(self.policy.signer_timeout_ms),
                    self.signer_resolver.resolve(provider_owner.clone(), cursor),
                )
                .await
                {
                    Ok(Ok(Some(signer))) => signer,
                    Ok(Ok(None))
                    | Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)) => {
                        self.outbox.record_completion_signer_policy_missing(
                            job_id,
                            &provider_owner,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                    Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable))
                    | Err(_) => {
                        self.outbox.record_completion_signer_resolution_failure(
                            job_id,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                };
                let signer_policy = match exact_current_signer_policy(&signer, &provider_owner) {
                    Ok(policy) => policy,
                    Err(CurrentSignerPolicyErrorV1::Unavailable) => {
                        self.outbox.record_completion_signer_resolution_failure(
                            job_id,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                    Err(CurrentSignerPolicyErrorV1::Ineligible) => {
                        self.outbox.record_completion_signer_policy_missing(
                            job_id,
                            &provider_owner,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                    Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                        self.outbox.record_completion_signer_resolution_failure(
                            job_id,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                };
                if signer_policy != completion_authority.signer_policy {
                    self.outbox.record_completion_signer_policy_missing(
                        job_id,
                        &provider_owner,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                    return Ok(());
                }
                if let Err(error) = self.outbox.validate_ready_completion_signer_policy(
                    job_id,
                    &provider_owner,
                    signer_policy,
                    cursor,
                ) {
                    self.outbox.record_completion_signer_resolution_failure(
                        job_id,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                    return Err(error.into());
                }
                let status = self.outbox.status(job_id)?;
                let request = ProviderIngestCompletionPayloadRequestV1 {
                    authorization: authorization_from_status_and_row(&status, row, cursor)?,
                    provider_owner: provider_owner.clone(),
                    expected_authority: completion_authority,
                    expected_assignment_revision: row.order.assignment_revision,
                    chain_id: self.chain_id.clone(),
                    completion_epoch,
                    finalized_cursor: cursor,
                };
                let payload = match tokio::time::timeout(
                    Duration::from_millis(self.policy.signer_timeout_ms),
                    self.payload_builder.build_payload(request),
                )
                .await
                {
                    Ok(Ok(payload)) => payload,
                    Ok(Err(
                        ProviderIngestCompletionPayloadErrorV1::Unavailable
                        | ProviderIngestCompletionPayloadErrorV1::Rejected,
                    ))
                    | Err(_) => {
                        self.outbox.record_completion_preparation_failure(
                            job_id,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                };
                let context = ProviderIngestCompletionSigningContextV1 {
                    baseline_finalized_cursor: cursor,
                    chain_id: self.chain_id.clone(),
                    provider_owner: provider_owner.clone(),
                    signer_policy,
                    assignment_revision: row.order.assignment_revision,
                    completion_epoch,
                    expected_payload: payload,
                };
                let claim =
                    match self
                        .outbox
                        .claim_completion_signing(job_id, context, self.clock.now_ms())
                    {
                        Ok(claim) => claim,
                        Err(ProviderIngestOutboxError::RetryNotDue) => return Ok(()),
                        Err(error) => return Err(error.into()),
                    };
                match exact_current_signer_policy(&signer, &provider_owner) {
                    Ok(policy) if policy == claim.context().signer_policy => {}
                    Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                    Ok(_)
                    | Err(
                        CurrentSignerPolicyErrorV1::Unavailable
                        | CurrentSignerPolicyErrorV1::Ineligible,
                    ) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                }
                let transaction = match tokio::time::timeout(
                    Duration::from_millis(self.policy.signer_timeout_ms),
                    signer.sign(claim.context().expected_payload.clone()),
                )
                .await
                {
                    Ok(Ok(transaction)) => transaction,
                    Ok(Err(ProviderIngestCompletionSignerErrorV1::Unavailable)) | Err(_) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                    Ok(Err(ProviderIngestCompletionSignerErrorV1::Rejected)) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                };
                match exact_current_signer_policy(&signer, &provider_owner) {
                    Ok(policy) if policy == claim.context().signer_policy => {}
                    Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                    Ok(_)
                    | Err(
                        CurrentSignerPolicyErrorV1::Unavailable
                        | CurrentSignerPolicyErrorV1::Ineligible,
                    ) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                }
                match self
                    .outbox
                    .store_completion_transaction(&claim, transaction)
                {
                    Ok(_) => {}
                    Err(
                        ProviderIngestOutboxError::InvalidSignedTransaction
                        | ProviderIngestOutboxError::InvalidSigningClaim,
                    ) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                    Err(error) => return Err(error.into()),
                }
                outcome.completions_signed = outcome.completions_signed.saturating_add(1);
                let status = self.outbox.status(job_id)?;
                if let ProviderIngestDeliveryStateV1::LocalStored {
                    completion: ProviderIngestCompletionStateV1::Signed { .. },
                    ..
                } = status.state
                {
                    self.submit_signed(job_id, &provider_owner, signer_policy, cursor, outcome)
                        .await?;
                }
            }
            ProviderIngestCompletionStateV1::Signing { .. } => {}
            ProviderIngestCompletionStateV1::Signed {
                next_attempt_at_ms, ..
            } => {
                if self.clock.now_ms() >= next_attempt_at_ms {
                    let Some((provider_owner, signer_policy)) = submission_authority else {
                        return Ok(());
                    };
                    self.submit_signed(job_id, &provider_owner, signer_policy, cursor, outcome)
                        .await?;
                }
            }
            ProviderIngestCompletionStateV1::Ambiguous {
                baseline_finalized_cursor,
                transaction_hash,
                ..
            } => {
                self.reconcile_transaction(
                    job_id,
                    transaction_hash,
                    baseline_finalized_cursor,
                    cursor,
                    true,
                )
                .await?;
            }
            ProviderIngestCompletionStateV1::Submitted {
                baseline_finalized_cursor,
                transaction_hash,
                ..
            } => {
                self.reconcile_transaction(
                    job_id,
                    transaction_hash,
                    baseline_finalized_cursor,
                    cursor,
                    false,
                )
                .await?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn submit_signed(
        &self,
        job_id: [u8; 32],
        provider_owner: &AccountId,
        signer_policy: ProviderIngestCompletionSignerPolicyV1,
        cursor: ProviderIngestFinalizedCursorV1,
        outcome: &mut ProviderIngestTickOutcomeV1,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let exact = match self.outbox.completion_transaction_for_authorized_preflight(
            job_id,
            provider_owner,
            signer_policy,
            cursor,
            self.clock.now_ms(),
        ) {
            Ok(exact) => exact,
            Err(
                ProviderIngestOutboxError::RetryNotDue
                | ProviderIngestOutboxError::InvalidTransition
                | ProviderIngestOutboxError::InvalidSigningContext
                | ProviderIngestOutboxError::SignerPolicyRollback
                | ProviderIngestOutboxError::StaleFinalizedCursor,
            ) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let prepared = match tokio::time::timeout(
            Duration::from_millis(self.policy.ingress_timeout_ms),
            self.ingress.prepare(exact.signed_transaction.clone()),
        )
        .await
        {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(ProviderIngestIngressPrepareErrorV1::Unavailable)) | Err(_) => {
                self.outbox.mark_completion_preflight_unavailable(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
            Ok(Err(ProviderIngestIngressPrepareErrorV1::Rejected)) => {
                self.outbox.mark_completion_preflight_rejected(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
        };
        let signer = match tokio::time::timeout(
            Duration::from_millis(self.policy.signer_timeout_ms),
            self.signer_resolver.resolve(provider_owner.clone(), cursor),
        )
        .await
        {
            Ok(Ok(Some(signer))) => signer,
            Ok(Ok(None)) | Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)) => {
                self.outbox.invalidate_stale_completion_authority(
                    job_id,
                    Some(provider_owner),
                    ProviderIngestSignerPolicyObservationV1::Missing,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
            Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable)) | Err(_) => {
                return Ok(());
            }
        };
        let signer_policy = match exact_current_signer_policy(&signer, provider_owner) {
            Ok(policy) => policy,
            Err(CurrentSignerPolicyErrorV1::Ineligible) => {
                self.outbox.invalidate_stale_completion_authority(
                    job_id,
                    Some(provider_owner),
                    ProviderIngestSignerPolicyObservationV1::Missing,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
            Err(CurrentSignerPolicyErrorV1::Unavailable) => return Ok(()),
            Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
            }
        };
        self.outbox.invalidate_stale_completion_authority(
            job_id,
            Some(provider_owner),
            ProviderIngestSignerPolicyObservationV1::Active(signer_policy),
            self.clock.now_ms(),
            cursor,
        )?;
        match exact_current_signer_policy(&signer, provider_owner) {
            Ok(current_policy) if current_policy == signer_policy => {}
            Ok(_)
            | Err(
                CurrentSignerPolicyErrorV1::Ineligible | CurrentSignerPolicyErrorV1::Unavailable,
            ) => return Ok(()),
            Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
            }
        }
        let submission = match self.outbox.authorize_and_begin_completion_submission(
            job_id,
            exact.transaction_hash,
            provider_owner,
            signer_policy,
            cursor,
            self.clock.now_ms(),
        ) {
            Ok(submission) => submission,
            Err(
                ProviderIngestOutboxError::RetryNotDue
                | ProviderIngestOutboxError::InvalidTransition
                | ProviderIngestOutboxError::InvalidSigningContext
                | ProviderIngestOutboxError::SignerPolicyRollback
                | ProviderIngestOutboxError::StaleFinalizedCursor,
            ) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if submission.signed_transaction != exact.signed_transaction {
            return Err(ProviderIngestRuntimeErrorV1::IngressProtocolViolation);
        }
        match exact_current_signer_policy(&signer, provider_owner) {
            Ok(current_policy) if current_policy == signer_policy => {}
            result => {
                self.outbox.mark_completion_not_submitted(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
                if result == Err(CurrentSignerPolicyErrorV1::ProtocolViolation) {
                    return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                }
                return Ok(());
            }
        }
        outcome.completion_submissions = outcome.completion_submissions.saturating_add(1);
        let disposition = match tokio::time::timeout(
            Duration::from_millis(self.policy.ingress_timeout_ms),
            self.ingress.expose(prepared, submission.signed_transaction),
        )
        .await
        {
            Ok(disposition) => disposition,
            Err(_) => ProviderIngestIngressDispositionV1::Ambiguous,
        };
        match disposition {
            ProviderIngestIngressDispositionV1::Submitted => self
                .outbox
                .mark_completion_submitted(job_id, exact.transaction_hash)?,
            ProviderIngestIngressDispositionV1::DefinitelyNotSubmitted => {
                self.outbox.mark_completion_not_submitted(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
            }
            ProviderIngestIngressDispositionV1::Rejected => {
                self.outbox.mark_completion_transaction_rejected(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
            }
            ProviderIngestIngressDispositionV1::Ambiguous => {}
        }
        Ok(())
    }

    async fn reconcile_transaction(
        &self,
        job_id: [u8; 32],
        transaction_hash: [u8; 32],
        baseline: ProviderIngestFinalizedCursorV1,
        cursor: ProviderIngestFinalizedCursorV1,
        ambiguous: bool,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let observation = tokio::time::timeout(
            Duration::from_millis(self.policy.ingress_timeout_ms),
            self.ingress.observe(transaction_hash),
        )
        .await
        .unwrap_or(ProviderIngestTransactionObservationV1::Unavailable);
        match observation {
            ProviderIngestTransactionObservationV1::CommittedSuccess
            | ProviderIngestTransactionObservationV1::Pending => {
                if ambiguous {
                    self.outbox
                        .mark_completion_submitted(job_id, transaction_hash)?;
                }
            }
            ProviderIngestTransactionObservationV1::CommittedRejected => {
                self.outbox.mark_completion_transaction_rejected(
                    job_id,
                    transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
            }
            ProviderIngestTransactionObservationV1::Unknown if cursor.height > baseline.height => {
                self.outbox.mark_completion_finalized_absent(
                    job_id,
                    transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
            }
            ProviderIngestTransactionObservationV1::Unknown
            | ProviderIngestTransactionObservationV1::Unavailable => {}
        }
        Ok(())
    }
}

struct ValidatedAssignmentV1 {
    authorization: FinalizedProviderIngestAuthorizationV1,
    source_provider_ids: Vec<[u8; 32]>,
}

fn validate_monotonic_finalized_cursor(
    previous: Option<ProviderIngestFinalizedCursorV1>,
    candidate: ProviderIngestFinalizedCursorV1,
) -> Result<(), ProviderIngestRuntimeErrorV1> {
    if previous.is_some_and(|previous| {
        candidate.height < previous.height
            || (candidate.height == previous.height && candidate.block_hash != previous.block_hash)
    }) {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    Ok(())
}

fn validate_page(
    page: &ProviderIngestFinalizedAssignmentPageV1,
    after_order_id: Option<[u8; 32]>,
    expected_cursor: ProviderIngestFinalizedCursorV1,
    limit: usize,
) -> Result<(), ProviderIngestRuntimeErrorV1> {
    if page.finalized_cursor != expected_cursor
        || page.finalized_cursor.height == 0
        || page.finalized_cursor.block_hash == [0; 32]
        || page.finalized_block_time_ms == 0
        || page.rows.len() > limit
        || page.next_after_order_id.is_some() && page.rows.is_empty()
        || page.next_after_order_id.is_some() && page.rows.len() != limit
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    let mut previous = after_order_id;
    for row in &page.rows {
        let order_id = *row.order.order_id.as_bytes();
        if previous.is_some_and(|previous| previous >= order_id) {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
        }
        previous = Some(order_id);
    }
    if let Some(next) = page.next_after_order_id
        && Some(next) != previous
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    Ok(())
}

fn validate_assignment(
    row: &ProviderIngestFinalizedAssignmentV1,
    cursor: ProviderIngestFinalizedCursorV1,
    provider_id: [u8; 32],
    policy: ProviderIngestRuntimePolicyV1,
) -> Result<ValidatedAssignmentV1, ProviderIngestRuntimeErrorV1> {
    if row.pin.finalized_cursor.height != cursor.height
        || row.pin.finalized_cursor.block_hash != cursor.block_hash
        || row.order.deadline_epoch <= row.order.issued_epoch
        || row
            .committed_transaction_hash
            .is_some_and(|hash| hash == [0; 32])
        || row.order.assignment_revision == 0
        || row.order.canonical_order.is_empty()
        || row.order.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
        || row.completion_authority.as_ref().is_some_and(|authority| {
            !authority.is_valid() || row.provider_owner.as_ref() != Some(&authority.provider_owner)
        })
        || row.provider_owner.is_none() && row.completion_authority.is_some()
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    let order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
        &row.order.canonical_order,
        REPLICATION_ORDER_DECODE_LIMITS_V1,
    )
    .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    order
        .validate()
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    let canonical = norito::to_bytes(&order)
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    if canonical != row.order.canonical_order
        || order.order_id != *row.order.order_id.as_bytes()
        || order.manifest_digest != *row.order.manifest_digest.as_bytes()
        || order.manifest_cid.as_slice() != row.order.manifest_root_cid.as_bytes()
        || row.pin.manifest.digest != row.order.manifest_digest
        || row.pin.manifest.root_cid != row.order.manifest_root_cid
        || row.pin.manifest.chunker.to_handle() != order.chunking_profile
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    if !order
        .assignments
        .iter()
        .any(|assignment| assignment.provider_id == provider_id)
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    let source_provider_ids = order
        .assignments
        .iter()
        .filter_map(|assignment| {
            (assignment.provider_id != provider_id).then_some(assignment.provider_id)
        })
        .collect::<Vec<_>>();
    if source_provider_ids.len() > policy.max_source_providers {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    let target_replicas = usize::from(order.target_replicas);
    if row.order.provider_completions.len() > target_replicas {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    let mut completions = BTreeSet::new();
    for completion in &row.order.provider_completions {
        if !order
            .assignments
            .iter()
            .any(|assignment| assignment.provider_id == *completion.provider_id.as_bytes())
            || completion.completion_epoch < row.order.issued_epoch
            || completion.completion_epoch > row.order.deadline_epoch
            || completion.assignment_revision != row.order.assignment_revision
            || !completion.completion_authority.is_valid()
            || completion.completion_authority.provider_owner != completion.completed_by
            || !completion.finalized_anchor.is_valid()
            || completion.finalized_anchor.height > cursor.height
            || completion.finalized_anchor.height == cursor.height
                && completion.finalized_anchor.block_hash != cursor.block_hash
            || !completions.insert(*completion.provider_id.as_bytes())
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
    }
    match row.order.status {
        ReplicationOrderStatus::Pending | ReplicationOrderStatus::Expired(_)
            if row.order.provider_completions.len() < target_replicas => {}
        ReplicationOrderStatus::Completed(epoch)
            if row.order.provider_completions.len() == target_replicas
                && row
                    .order
                    .provider_completions
                    .last()
                    .is_some_and(|completion| completion.completion_epoch == epoch) => {}
        ReplicationOrderStatus::Pending
        | ReplicationOrderStatus::Completed(_)
        | ReplicationOrderStatus::Expired(_) => {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
    }
    let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        cursor.height,
        cursor.block_hash,
        provider_id,
        *row.order.order_id.as_bytes(),
        *row.pin.manifest.digest.as_bytes(),
        order.manifest_cid,
        order.chunking_profile,
        row.pin.manifest.chunk_digest_sha3_256,
        row.pin.manifest.por_root,
        row.pin.manifest.content_length,
    )?;
    Ok(ValidatedAssignmentV1 {
        authorization,
        source_provider_ids,
    })
}

fn authorization_from_status_and_row(
    status: &crate::provider_ingest_outbox::ProviderIngestStatusV1,
    row: &ProviderIngestFinalizedAssignmentV1,
    cursor: ProviderIngestFinalizedCursorV1,
) -> Result<FinalizedProviderIngestAuthorizationV1, ProviderIngestRuntimeErrorV1> {
    let order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
        &row.order.canonical_order,
        REPLICATION_ORDER_DECODE_LIMITS_V1,
    )
    .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        cursor.height,
        cursor.block_hash,
        status.provider_id,
        status.order_id,
        status.manifest_digest,
        order.manifest_cid,
        order.chunking_profile,
        row.pin.manifest.chunk_digest_sha3_256,
        row.pin.manifest.por_root,
        row.pin.manifest.content_length,
    )?;
    if authorization.job_id() != status.job_id {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    Ok(authorization)
}

enum LeaseOperationOutcomeV1<T> {
    Completed(T),
    TimedOut,
}

enum MutatingStorageOutcomeV1<T> {
    Completed(T),
    CompletedAfterSoftTimeout(T),
}

/// Fatal supervised-runtime failure.
#[allow(clippy::large_enum_variant, variant_size_differences)]
#[derive(Debug, Error)]
pub enum ProviderIngestRuntimeErrorV1 {
    /// Runtime bounds or timeout policy is invalid.
    #[error("provider-ingest runtime policy is invalid")]
    InvalidPolicy,
    /// Configured provider identity is zero.
    #[error("provider-ingest runtime provider identity is invalid")]
    InvalidProviderId,
    /// Finalized ledger paging is unavailable.
    #[error("provider-ingest finalized ledger is unavailable")]
    FinalizedLedgerUnavailable,
    /// Finalized page cursor, bounds, or ordering is invalid.
    #[error("provider-ingest finalized page is invalid")]
    InvalidFinalizedPage,
    /// Finalized pin/order/provider material is noncanonical or inconsistent.
    #[error("provider-ingest finalized binding is invalid")]
    InvalidFinalizedBinding,
    /// Local storage returned a manifest identity that violates its exact contract.
    #[error("provider-ingest local storage violated the exact binding")]
    StorageProtocolViolation,
    /// Authenticated source identity, policy, or qualification drifted.
    #[error("provider-ingest authenticated source violated its exact binding")]
    SourceProtocolViolation,
    /// Resolved signer or signed transaction violated the prepared context.
    #[error("provider-ingest signer violated the prepared operation")]
    SignerProtocolViolation,
    /// Queue preflight/exposure violated the exact transaction contract.
    #[error("provider-ingest ingress violated the prepared operation")]
    IngressProtocolViolation,
    /// Durable outbox transition failed.
    #[error(transparent)]
    Outbox(#[from] ProviderIngestOutboxError),
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
mod tests {
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        },
        time::Instant,
    };

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        isi::{InstructionBox, sorafs::CompleteReplicationOrder},
        metadata::Metadata,
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestDigest, ManifestRootCid, PinManifestFinalizedCursorV1,
            PinManifestRecord, PinPolicy, ProviderIngestFinalizedAnchorV1,
            ReplicationOrderCompletionRecord, ReplicationOrderId,
        },
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_manifest::capacity::{
        REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
    };

    use super::*;
    use crate::provider_ingest_outbox::{
        ProviderIngestCompletionStateV1, ProviderIngestDeliveryStateV1,
        ProviderIngestOutboxPolicyV1,
    };

    const LOCAL_PROVIDER: [u8; 32] = [0x11; 32];
    const SOURCE_PROVIDER: [u8; 32] = [0x22; 32];

    fn account(seed: u8) -> AccountId {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key");
        AccountId::new(key.public_key().clone())
    }

    fn cursor(height: u64) -> ProviderIngestFinalizedCursorV1 {
        ProviderIngestFinalizedCursorV1 {
            height,
            block_hash: [u8::try_from(height).unwrap_or(0xFE); 32],
        }
    }

    fn completion_signer_policy(revision: u64) -> ProviderIngestCompletionSignerPolicyV1 {
        let digest_byte = u8::try_from(revision).unwrap_or(0xFE);
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0xA1; 32],
            revision,
            predecessor_digest: (revision > 1).then(|| [digest_byte.saturating_sub(1); 32]),
            policy_digest: [digest_byte; 32],
        }
    }

    #[test]
    fn completion_signer_binding_rejects_test_handles_stale_revisions_and_key_mismatch() {
        let key = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519)
            .expect("completion signer key");
        let authority = AccountId::new(key.public_key().clone());
        let qualification = ProviderIngestCompletionSignerQualificationV1::new(
            1,
            completion_signer_policy(1),
            Algorithm::Ed25519,
            key.public_key().clone(),
        );
        assert!(qualification.matches_authority(&authority));
        assert_eq!(qualification.validate(), Ok(()));
        assert_eq!(
            ProviderIngestCompletionSignerBindingV1::new(
                "pkcs11:sorafs-provider-ingest-primary",
                qualification.clone(),
            )
            .validate(),
            Ok(())
        );
        assert_eq!(
            ProviderIngestCompletionSignerBindingV1::new(
                "pkcs11:sorafs-provider-ingest-test",
                qualification.clone(),
            )
            .validate(),
            Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerHandle)
        );

        let mut stale = qualification.clone();
        stale.adapter_revision = 0;
        assert_eq!(
            stale.validate(),
            Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerQualification)
        );
        let mut mismatched_algorithm = qualification;
        mismatched_algorithm.algorithm = Algorithm::MlDsa;
        assert_eq!(
            mismatched_algorithm.validate(),
            Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerQualification)
        );
    }

    fn completion_record(
        provider_id: ProviderId,
        completed_by: AccountId,
        completion_epoch: u64,
    ) -> ReplicationOrderCompletionRecord {
        ReplicationOrderCompletionRecord {
            provider_id,
            completed_by: completed_by.clone(),
            completion_epoch,
            assignment_revision: 1,
            completion_authority: ProviderIngestCompletionAuthorityV1::new(
                completed_by,
                completion_signer_policy(1),
            ),
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: completion_epoch,
                block_hash: cursor(completion_epoch).block_hash,
            },
        }
    }

    fn fixture_row(order_seed: u8) -> ProviderIngestFinalizedAssignmentV1 {
        let digest = ManifestDigest::new([order_seed.wrapping_add(0x40); 32]);
        let root =
            ManifestRootCid::from_blake3_digest([order_seed.wrapping_add(0x50); 32]).unwrap();
        let chunker = ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        };
        let mut manifest = PinManifestRecord::new(
            digest,
            root.clone(),
            chunker,
            [order_seed.wrapping_add(0x60); 32],
            [order_seed.wrapping_add(0x70); 32],
            4_096,
            PinPolicy::default(),
            account(1),
            7,
            None,
            None,
            Metadata::default(),
        );
        manifest.status = PinStatus::Approved(7);
        let order_id = [order_seed; 32];
        let order_body = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id,
            manifest_cid: root.as_bytes().to_vec(),
            manifest_digest: *digest.as_bytes(),
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            target_replicas: 2,
            assignments: vec![
                ReplicationAssignmentV1 {
                    provider_id: LOCAL_PROVIDER,
                    slice_gib: 1,
                    lane: None,
                },
                ReplicationAssignmentV1 {
                    provider_id: SOURCE_PROVIDER,
                    slice_gib: 1,
                    lane: None,
                },
            ],
            issued_at: 100,
            deadline_at: 200,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 10,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 99_000,
            },
            metadata: Vec::new(),
        };
        order_body.validate().expect("valid order");
        ProviderIngestFinalizedAssignmentV1 {
            pin: PinManifestFinalizedRecordV1 {
                finalized_cursor: PinManifestFinalizedCursorV1 {
                    height: 8,
                    block_hash: cursor(8).block_hash,
                },
                manifest,
            },
            order: ReplicationOrderRecord {
                order_id: ReplicationOrderId::new(order_id),
                manifest_digest: digest,
                manifest_root_cid: root,
                issued_by: account(1),
                issued_epoch: 7,
                deadline_epoch: 20,
                canonical_order: norito::to_bytes(&order_body).expect("order bytes"),
                assignment_revision: 1,
                provider_completions: Vec::new(),
                status: ReplicationOrderStatus::Pending,
            },
            provider_owner: Some(account(8)),
            completion_authority: Some(ProviderIngestCompletionAuthorityV1::new(
                account(8),
                completion_signer_policy(1),
            )),
            completion_epoch: Some(8),
            committed_transaction_hash: None,
        }
    }

    fn fixture_page(
        row: ProviderIngestFinalizedAssignmentV1,
    ) -> ProviderIngestFinalizedAssignmentPageV1 {
        let finalized_cursor = ProviderIngestFinalizedCursorV1 {
            height: row.pin.finalized_cursor.height,
            block_hash: row.pin.finalized_cursor.block_hash,
        };
        ProviderIngestFinalizedAssignmentPageV1 {
            finalized_cursor,
            finalized_block_time_ms: finalized_cursor.height.saturating_mul(1_000),
            rows: vec![row],
            next_after_order_id: None,
        }
    }

    fn outbox_policy() -> ProviderIngestOutboxPolicyV1 {
        ProviderIngestOutboxPolicyV1 {
            max_active_entries: 32,
            max_terminal_entries: 32,
            max_attempts: 4,
            checkpoint_max_bytes: 8 * 1024 * 1024,
            checkpoint_operation_timeout_ms: 250,
            source_lease_ttl_ms: 20,
            retry_base_delay_ms: 10_000,
            retry_max_delay_ms: 100_000,
            terminal_retention_blocks: 100,
            max_signed_transaction_bytes: 128 * 1024,
            max_status_page_size: 32,
        }
    }

    fn runtime_policy() -> ProviderIngestRuntimePolicyV1 {
        ProviderIngestRuntimePolicyV1 {
            max_page_rows: 16,
            max_pages_per_tick: 2,
            max_source_jobs_per_tick: 4,
            max_source_providers: 4,
            scan_interval_ms: 10,
            source_operation_timeout_ms: 250,
            source_lease_renew_interval_ms: 5,
            signer_timeout_ms: 100,
            ingress_timeout_ms: 100,
        }
    }

    struct TestLedger {
        page: Mutex<ProviderIngestFinalizedAssignmentPageV1>,
    }

    impl ProviderIngestFinalizedLedgerV1 for TestLedger {
        fn read_assignment_page<'a>(
            &'a self,
            at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
            after_order_id: Option<[u8; 32]>,
            _limit: usize,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<ProviderIngestFinalizedAssignmentPageV1, ProviderIngestFinalizedLedgerErrorV1>,
        > {
            let page = self.page.lock().unwrap().clone();
            Box::pin(async move {
                if at_finalized_cursor.is_some_and(|cursor| cursor != page.finalized_cursor) {
                    return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
                }
                if after_order_id.is_some() {
                    Ok(ProviderIngestFinalizedAssignmentPageV1 {
                        finalized_cursor: page.finalized_cursor,
                        finalized_block_time_ms: page.finalized_block_time_ms,
                        rows: Vec::new(),
                        next_after_order_id: None,
                    })
                } else {
                    Ok(page)
                }
            })
        }
    }

    struct TestFetch {
        result: Mutex<Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>>,
        delay_ms: u64,
        calls: AtomicUsize,
    }

    impl ProviderIngestAuthenticatedSourceFetchV1 for TestFetch {
        type Fetched = Vec<u8>;

        fn fetch<'a>(
            &'a self,
            request: ProviderIngestSourceRequestV1,
        ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>>
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.source_provider_ids, vec![SOURCE_PROVIDER]);
            let result = self.result.lock().unwrap().clone();
            let delay_ms = self.delay_ms;
            Box::pin(async move {
                if delay_ms != 0 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
                result
            })
        }
    }

    struct TestProviderSource {
        provider_id: [u8; 32],
        runtime_handle: &'static str,
        drifted_runtime_handle: &'static str,
        drift_after_fetch: bool,
        drifted: AtomicBool,
        qualification: Mutex<ProviderIngestSourceQualificationV1>,
        qualification_after_fetch: Mutex<Option<ProviderIngestSourceQualificationV1>>,
        readiness: Mutex<Result<(), ProviderIngestSourceFetchErrorV1>>,
        result: Mutex<Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>>,
        calls: Arc<Mutex<Vec<[u8; 32]>>>,
    }

    impl ProviderIngestAuthenticatedProviderSourceV1 for TestProviderSource {
        type Fetched = Vec<u8>;

        fn provider_id(&self) -> [u8; 32] {
            self.provider_id
        }

        fn runtime_handle(&self) -> &str {
            if self.drifted.load(Ordering::SeqCst) {
                self.drifted_runtime_handle
            } else {
                self.runtime_handle
            }
        }

        fn qualification(
            &self,
        ) -> Result<ProviderIngestSourceQualificationV1, ProviderIngestSourceFetchErrorV1> {
            Ok(*self.qualification.lock().unwrap())
        }

        fn check_readiness(&self) -> Result<(), ProviderIngestSourceFetchErrorV1> {
            *self.readiness.lock().unwrap()
        }

        fn fetch_provider<'a>(
            &'a self,
            authorization: FinalizedProviderIngestAuthorizationV1,
        ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>>
        {
            assert_eq!(authorization.provider_id(), LOCAL_PROVIDER);
            self.calls.lock().unwrap().push(self.provider_id);
            let result = self.result.lock().unwrap().clone();
            let qualification_after_fetch = self.qualification_after_fetch.lock().unwrap().take();
            if let Some(qualification) = qualification_after_fetch {
                *self.qualification.lock().unwrap() = qualification;
            }
            if self.drift_after_fetch {
                self.drifted.store(true, Ordering::SeqCst);
            }
            Box::pin(async move { result })
        }
    }

    fn test_provider_source(
        provider_id: [u8; 32],
        runtime_handle: &'static str,
        readiness: Result<(), ProviderIngestSourceFetchErrorV1>,
        result: Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>,
        drift_after_fetch: bool,
        calls: Arc<Mutex<Vec<[u8; 32]>>>,
    ) -> Arc<TestProviderSource> {
        Arc::new(TestProviderSource {
            provider_id,
            runtime_handle,
            drifted_runtime_handle: "https-pinned:provider-substituted",
            drift_after_fetch,
            drifted: AtomicBool::new(false),
            qualification: Mutex::new(ProviderIngestSourceQualificationV1::new(1, provider_id)),
            qualification_after_fetch: Mutex::new(None),
            readiness: Mutex::new(readiness),
            result: Mutex::new(result),
            calls,
        })
    }

    fn test_source_binding(
        provider_id: [u8; 32],
        runtime_handle: impl Into<String>,
    ) -> ProviderIngestAuthenticatedSourceBindingV1 {
        ProviderIngestAuthenticatedSourceBindingV1 {
            provider_id,
            runtime_handle: runtime_handle.into(),
            revision: 1,
            policy_digest: provider_id,
        }
    }

    fn test_source_registration(
        source: Arc<TestProviderSource>,
        binding: ProviderIngestAuthenticatedSourceBindingV1,
    ) -> ProviderIngestAuthenticatedSourceRegistrationV1<Vec<u8>> {
        let source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Vec<u8>>> =
            source;
        ProviderIngestAuthenticatedSourceRegistrationV1::new(binding, source)
    }

    fn test_source_pool(
        sources: Vec<Arc<TestProviderSource>>,
    ) -> Result<
        ProviderIngestAuthenticatedSourcePoolV1<Vec<u8>>,
        ProviderIngestAuthenticatedSourcePoolErrorV1,
    > {
        let sources = sources
            .into_iter()
            .map(|source| {
                let binding = test_source_binding(source.provider_id, source.runtime_handle);
                test_source_registration(source, binding)
            })
            .collect();
        ProviderIngestAuthenticatedSourcePoolV1::new(
            "https-pinned-source-pool:region-a",
            ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
            4,
            sources,
        )
    }

    fn test_source_request(source_provider_ids: Vec<[u8; 32]>) -> ProviderIngestSourceRequestV1 {
        let row = fixture_row(0x31);
        let validated =
            validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).unwrap();
        ProviderIngestSourceRequestV1 {
            authorization: validated.authorization,
            source_provider_ids,
        }
    }

    #[test]
    fn authenticated_source_qualification_rejects_unsupported_or_zero_pins() {
        let valid = ProviderIngestSourceQualificationV1::new(1, [0x22; 32]);
        assert_eq!(valid.validate(), Ok(()));

        let mut unsupported = valid;
        unsupported.version = 2;
        for invalid in [
            unsupported,
            ProviderIngestSourceQualificationV1::new(0, [0x22; 32]),
            ProviderIngestSourceQualificationV1::new(1, [0; 32]),
        ] {
            assert_eq!(
                invalid.validate(),
                Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceQualification)
            );
        }
    }

    #[test]
    fn runtime_provider_qualification_requires_both_public_pins() {
        assert!(ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]).is_valid());
        assert!(!ProviderIngestRuntimeProviderQualificationV1::new(0, [0xA9; 32]).is_valid());
        assert!(!ProviderIngestRuntimeProviderQualificationV1::new(9, [0; 32]).is_valid());
    }

    #[test]
    fn authenticated_source_pool_rejects_incomplete_duplicate_and_test_marked_inventory() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        assert_eq!(
            test_source_pool(vec![Arc::clone(&source_a)]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceCount
        );

        let duplicate_provider = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        assert_eq!(
            test_source_pool(vec![Arc::clone(&source_a), duplicate_provider]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateProvider
        );

        let duplicate_handle = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        assert_eq!(
            test_source_pool(vec![Arc::clone(&source_a), duplicate_handle]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateSourceHandle
        );

        let credential_handle = test_provider_source(
            [0x33; 32],
            "https://operator:secret@provider.example",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        assert_eq!(
            test_source_pool(vec![Arc::clone(&source_a), credential_handle]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceHandle
        );

        let test_marked = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-test",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            calls,
        );
        assert_eq!(
            test_source_pool(vec![source_a, test_marked]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceHandle
        );
    }

    #[test]
    fn authenticated_source_pool_requires_independent_valid_qualification_pins() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            calls,
        );

        for invalid_pool_qualification in [
            ProviderIngestRuntimeProviderQualificationV1::new(0, [0xA9; 32]),
            ProviderIngestRuntimeProviderQualificationV1::new(9, [0; 32]),
        ] {
            assert_eq!(
                ProviderIngestAuthenticatedSourcePoolV1::new(
                    "https-pinned-source-pool:region-a",
                    invalid_pool_qualification,
                    4,
                    vec![
                        test_source_registration(
                            Arc::clone(&source_a),
                            test_source_binding(source_a.provider_id, source_a.runtime_handle),
                        ),
                        test_source_registration(
                            Arc::clone(&source_b),
                            test_source_binding(source_b.provider_id, source_b.runtime_handle),
                        ),
                    ],
                )
                .unwrap_err(),
                ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidPoolQualification
            );
        }

        let mut invalid_binding =
            test_source_binding(source_b.provider_id, source_b.runtime_handle);
        invalid_binding.revision = 0;
        assert_eq!(
            ProviderIngestAuthenticatedSourcePoolV1::new(
                "https-pinned-source-pool:region-a",
                ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
                4,
                vec![
                    test_source_registration(
                        Arc::clone(&source_a),
                        test_source_binding(source_a.provider_id, source_a.runtime_handle),
                    ),
                    test_source_registration(Arc::clone(&source_b), invalid_binding),
                ],
            )
            .unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceQualification
        );

        let mut substituted_binding =
            test_source_binding(source_b.provider_id, source_b.runtime_handle);
        substituted_binding.revision = 2;
        assert_eq!(
            ProviderIngestAuthenticatedSourcePoolV1::new(
                "https-pinned-source-pool:region-a",
                ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
                4,
                vec![
                    test_source_registration(
                        Arc::clone(&source_a),
                        test_source_binding(source_a.provider_id, source_a.runtime_handle),
                    ),
                    test_source_registration(source_b, substituted_binding),
                ],
            )
            .unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::SourceBindingMismatch
        );
    }

    #[test]
    fn authenticated_source_pool_rejects_qualification_drift_at_readiness() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![Arc::clone(&source_a), source_b]).unwrap();
        *source_a.qualification.lock().unwrap() =
            ProviderIngestSourceQualificationV1::new(2, [0x22; 32]);

        assert_eq!(
            pool.check_readiness(),
            Err(ProviderIngestSourceFetchErrorV1::Rejected)
        );
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn authenticated_source_pool_is_ready_when_one_qualified_source_is_available() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            calls,
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(pool.check_readiness(), Ok(()));
    }

    #[tokio::test]
    async fn authenticated_source_pool_fails_over_in_canonical_provider_order() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Ok(vec![0xA5]),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(pool.runtime_handle(), "https-pinned-source-pool:region-a");
        assert_eq!(pool.source_provider_ids(), &[[0x22; 32], [0x33; 32]]);
        assert_eq!(pool.max_sources_per_fetch(), 4);
        assert!(pool.check_readiness().is_ok());
        assert_eq!(
            pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
                .await,
            Ok(vec![0xA5])
        );
        assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32], [0x33; 32]]);
    }

    #[tokio::test]
    async fn authenticated_source_pool_fails_over_after_content_rejection() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Ok(vec![0xA5]),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(
            pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
                .await,
            Ok(vec![0xA5])
        );
        assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32], [0x33; 32]]);
    }

    #[tokio::test]
    async fn authenticated_source_pool_rejects_noncanonical_or_unpinned_requests_before_io() {
        for source_provider_ids in [
            vec![[0x33; 32], [0x22; 32]],
            vec![[0x22; 32], [0x44; 32]],
            vec![LOCAL_PROVIDER, [0x22; 32]],
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let source_a = test_provider_source(
                [0x22; 32],
                "https-pinned:provider-a",
                Ok(()),
                Err(ProviderIngestSourceFetchErrorV1::Unavailable),
                false,
                Arc::clone(&calls),
            );
            let source_b = test_provider_source(
                [0x33; 32],
                "https-pinned:provider-b",
                Ok(()),
                Err(ProviderIngestSourceFetchErrorV1::Unavailable),
                false,
                Arc::clone(&calls),
            );
            let pool = test_source_pool(vec![source_a, source_b]).unwrap();

            assert_eq!(
                pool.fetch(test_source_request(source_provider_ids)).await,
                Err(ProviderIngestSourceFetchErrorV1::Rejected)
            );
            assert!(calls.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn authenticated_source_pool_does_not_mask_identity_drift_with_later_success() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Ok(vec![0xA5]),
            true,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Ok(vec![0xB6]),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(
            pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
                .await,
            Err(ProviderIngestSourceFetchErrorV1::Rejected)
        );
        assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32]]);
    }

    #[tokio::test]
    async fn authenticated_source_pool_does_not_mask_qualification_drift_with_later_success() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Ok(vec![0xA5]),
            false,
            Arc::clone(&calls),
        );
        *source_a.qualification_after_fetch.lock().unwrap() =
            Some(ProviderIngestSourceQualificationV1::new(2, [0x22; 32]));
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Ok(vec![0xB6]),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(
            pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
                .await,
            Err(ProviderIngestSourceFetchErrorV1::Rejected)
        );
        assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32]]);
    }

    struct TestStorage {
        existing: AtomicBool,
    }

    impl ProviderIngestLocalStorageV1<Vec<u8>> for TestStorage {
        fn verify_existing<'a>(
            &'a self,
            authorization: FinalizedProviderIngestAuthorizationV1,
        ) -> ProviderIngestFutureV1<'a, Result<Option<String>, ProviderIngestLocalStorageErrorV1>>
        {
            let existing = self.existing.load(Ordering::SeqCst);
            Box::pin(
                async move { Ok(existing.then(|| hex::encode(authorization.manifest_digest()))) },
            )
        }

        fn store<'a>(
            &'a self,
            authorization: FinalizedProviderIngestAuthorizationV1,
            fetched: Vec<u8>,
        ) -> ProviderIngestFutureV1<'a, Result<String, ProviderIngestLocalStorageErrorV1>> {
            Box::pin(async move {
                if fetched != vec![0xA5] {
                    return Err(ProviderIngestLocalStorageErrorV1::Retryable);
                }
                if authorization.order_id() == [0x3E; 32] {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                Ok(hex::encode(authorization.manifest_digest()))
            })
        }
    }

    struct TestPayloadBuilder;

    impl ProviderIngestCompletionPayloadBuilderV1 for TestPayloadBuilder {
        fn build_payload<'a>(
            &'a self,
            request: ProviderIngestCompletionPayloadRequestV1,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1>,
        > {
            Box::pin(async move {
                if request.authorization.order_id() == [0x3B; 32] {
                    return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
                }
                let mut builder = TransactionBuilder::new(
                    request.chain_id,
                    request.provider_owner,
                    FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([InstructionBox::from(
                    CompleteReplicationOrder {
                        order_id: ReplicationOrderId::new(request.authorization.order_id()),
                        provider_id: ProviderId::new(request.authorization.provider_id()),
                        completion_epoch: request.completion_epoch,
                        expected_authority: request.expected_authority,
                        expected_assignment_revision: request.expected_assignment_revision,
                        finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                            height: request.finalized_cursor.height,
                            block_hash: request.finalized_cursor.block_hash,
                        },
                    },
                )]);
                builder.set_creation_time(Duration::from_millis(1_000));
                builder.set_ttl(Duration::from_secs(30));
                builder
                    .into_payload()
                    .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)
            })
        }
    }

    struct TestSigner {
        key: KeyPair,
        authority: AccountId,
        signer_policy_revision: Arc<AtomicU64>,
        eligibility_flip_on_call: usize,
        eligibility_flip_to_revision: u64,
        eligibility_calls: AtomicUsize,
    }

    impl ProviderIngestCompletionSignerV1 for TestSigner {
        fn runtime_handle(&self) -> &str {
            "pkcs11:sorafs-provider-ingest-unit"
        }

        fn authority(&self) -> &AccountId {
            &self.authority
        }

        fn qualification(
            &self,
        ) -> Result<
            ProviderIngestCompletionSignerQualificationV1,
            ProviderIngestCompletionSignerErrorV1,
        > {
            Ok(ProviderIngestCompletionSignerQualificationV1::new(
                1,
                self.signer_policy(),
                self.key.public_key().algorithm(),
                self.key.public_key().clone(),
            ))
        }

        fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
            completion_signer_policy(self.signer_policy_revision.load(Ordering::SeqCst))
        }

        fn current_eligibility(
            &self,
        ) -> Result<ProviderIngestCompletionSignerPolicyV1, ProviderIngestCompletionSignerErrorV1>
        {
            let call = self
                .eligibility_calls
                .fetch_add(1, Ordering::SeqCst)
                .saturating_add(1);
            if self.eligibility_flip_on_call != 0 && call == self.eligibility_flip_on_call {
                self.signer_policy_revision
                    .store(self.eligibility_flip_to_revision, Ordering::SeqCst);
            }
            let signer_policy = self.signer_policy();
            if signer_policy.is_valid() {
                Ok(signer_policy)
            } else {
                Err(ProviderIngestCompletionSignerErrorV1::Rejected)
            }
        }

        fn sign<'a>(
            &'a self,
            payload: TransactionPayload,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>,
        > {
            Box::pin(async move {
                TransactionBuilder::from_payload(payload)
                    .and_then(|builder| builder.try_sign(self.key.private_key()))
                    .map_err(|_| ProviderIngestCompletionSignerErrorV1::Rejected)
            })
        }
    }

    struct TestResolver {
        wrong_authority: AtomicBool,
        signer_policy_revision: Arc<AtomicU64>,
        eligibility_flip_on_call: AtomicUsize,
        eligibility_flip_to_revision: AtomicU64,
    }

    impl ProviderIngestCompletionSignerResolverV1 for TestResolver {
        type Signer = TestSigner;

        fn resolve<'a>(
            &'a self,
            _provider_owner: AccountId,
            _finalized_cursor: ProviderIngestFinalizedCursorV1,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<Option<Self::Signer>, ProviderIngestCompletionSignerResolverErrorV1>,
        > {
            let seed = if self.wrong_authority.load(Ordering::SeqCst) {
                9
            } else {
                8
            };
            let signer_policy_revision = Arc::clone(&self.signer_policy_revision);
            let eligibility_flip_on_call = self.eligibility_flip_on_call.load(Ordering::SeqCst);
            let eligibility_flip_to_revision =
                self.eligibility_flip_to_revision.load(Ordering::SeqCst);
            Box::pin(async move {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key");
                let authority = AccountId::new(key.public_key().clone());
                Ok(Some(TestSigner {
                    key,
                    authority,
                    signer_policy_revision,
                    eligibility_flip_on_call,
                    eligibility_flip_to_revision,
                    eligibility_calls: AtomicUsize::new(0),
                }))
            })
        }
    }

    struct TestIngress {
        outbox: ProviderIngestOutbox,
        job_id: [u8; 32],
        prepare_error: Mutex<Option<ProviderIngestIngressPrepareErrorV1>>,
        disposition: Mutex<ProviderIngestIngressDispositionV1>,
        observation: Mutex<ProviderIngestTransactionObservationV1>,
        observe_calls: AtomicUsize,
        events: Mutex<Vec<&'static str>>,
    }

    impl ProviderIngestTransactionIngressV1 for TestIngress {
        type Prepared = SignedTransaction;

        fn prepare<'a>(
            &'a self,
            transaction: SignedTransaction,
        ) -> ProviderIngestFutureV1<'a, Result<Self::Prepared, ProviderIngestIngressPrepareErrorV1>>
        {
            let state = self.outbox.status(self.job_id).unwrap().state;
            assert!(matches!(
                state,
                ProviderIngestDeliveryStateV1::LocalStored {
                    completion: ProviderIngestCompletionStateV1::Signed { .. },
                    ..
                }
            ));
            self.events.lock().unwrap().push("prepare_signed");
            let error = *self.prepare_error.lock().unwrap();
            Box::pin(async move {
                if let Some(error) = error {
                    Err(error)
                } else {
                    Ok(transaction)
                }
            })
        }

        fn expose<'a>(
            &'a self,
            prepared: Self::Prepared,
            transaction: SignedTransaction,
        ) -> ProviderIngestFutureV1<'a, ProviderIngestIngressDispositionV1> {
            assert_eq!(prepared, transaction);
            let state = self.outbox.status(self.job_id).unwrap().state;
            assert!(matches!(
                state,
                ProviderIngestDeliveryStateV1::LocalStored {
                    completion: ProviderIngestCompletionStateV1::Ambiguous { .. },
                    ..
                }
            ));
            self.events.lock().unwrap().push("expose_ambiguous");
            let disposition = *self.disposition.lock().unwrap();
            Box::pin(async move { disposition })
        }

        fn observe<'a>(
            &'a self,
            _transaction_hash: [u8; 32],
        ) -> ProviderIngestFutureV1<'a, ProviderIngestTransactionObservationV1> {
            self.observe_calls.fetch_add(1, Ordering::SeqCst);
            let observation = *self.observation.lock().unwrap();
            Box::pin(async move { observation })
        }
    }

    struct TestClock {
        start: Instant,
        base_ms: AtomicU64,
    }

    impl ProviderIngestClockV1 for TestClock {
        fn now_ms(&self) -> u64 {
            self.base_ms
                .load(Ordering::SeqCst)
                .saturating_add(u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX))
        }
    }

    type TestRuntime = ProviderIngestRuntimeV1<
        TestLedger,
        TestFetch,
        TestStorage,
        TestPayloadBuilder,
        TestResolver,
        TestIngress,
        TestClock,
    >;

    fn test_runtime(
        row: ProviderIngestFinalizedAssignmentV1,
        existing: bool,
        fetch_result: Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>,
        fetch_delay_ms: u64,
        disposition: ProviderIngestIngressDispositionV1,
        wrong_signer: bool,
    ) -> (
        TestRuntime,
        Arc<TestLedger>,
        Arc<TestFetch>,
        Arc<TestIngress>,
    ) {
        let page = fixture_page(row.clone());
        let finalized_cursor = page.finalized_cursor;
        let ledger = Arc::new(TestLedger {
            page: Mutex::new(page),
        });
        let fetch = Arc::new(TestFetch {
            result: Mutex::new(fetch_result),
            delay_ms: fetch_delay_ms,
            calls: AtomicUsize::new(0),
        });
        let storage = Arc::new(TestStorage {
            existing: AtomicBool::new(existing),
        });
        let outbox = ProviderIngestOutbox::in_memory(outbox_policy()).expect("outbox");
        let validated =
            validate_assignment(&row, finalized_cursor, LOCAL_PROVIDER, runtime_policy()).unwrap();
        let ingress = Arc::new(TestIngress {
            outbox: outbox.clone(),
            job_id: validated.authorization.job_id(),
            prepare_error: Mutex::new(None),
            disposition: Mutex::new(disposition),
            observation: Mutex::new(ProviderIngestTransactionObservationV1::Unavailable),
            observe_calls: AtomicUsize::new(0),
            events: Mutex::new(Vec::new()),
        });
        let runtime = ProviderIngestRuntimeV1::new(
            LOCAL_PROVIDER,
            ChainId::from("provider-ingest-runtime-test"),
            ProviderIngestClaimOwnerV1::new([0xCC; 32]).unwrap(),
            runtime_policy(),
            outbox,
            ledger.clone(),
            fetch.clone(),
            storage,
            Arc::new(TestPayloadBuilder),
            Arc::new(TestResolver {
                wrong_authority: AtomicBool::new(wrong_signer),
                signer_policy_revision: Arc::new(AtomicU64::new(1)),
                eligibility_flip_on_call: AtomicUsize::new(0),
                eligibility_flip_to_revision: AtomicU64::new(0),
            }),
            ingress.clone(),
            Arc::new(TestClock {
                start: Instant::now(),
                base_ms: AtomicU64::new(1_000),
            }),
        )
        .expect("runtime");
        (runtime, ledger, fetch, ingress)
    }

    #[test]
    fn finalized_page_rejects_cursor_order_and_pagination_substitution() {
        let row = fixture_row(0x31);
        let page = fixture_page(row.clone());
        validate_page(&page, None, cursor(8), 16).expect("valid page");

        let mut wrong_cursor = page.clone();
        wrong_cursor.finalized_cursor = cursor(9);
        assert!(matches!(
            validate_page(&wrong_cursor, None, cursor(8), 16),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));

        let mut duplicate = page.clone();
        duplicate.rows.push(row);
        assert!(matches!(
            validate_page(&duplicate, None, cursor(8), 16),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));

        let mut forged_next = page;
        forged_next.next_after_order_id = Some([0xFF; 32]);
        assert!(matches!(
            validate_page(&forged_next, None, cursor(8), 16),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));
    }

    #[test]
    fn finalized_cursor_and_order_lifecycle_fail_closed_on_substitution() {
        assert!(validate_monotonic_finalized_cursor(None, cursor(8)).is_ok());
        assert!(validate_monotonic_finalized_cursor(Some(cursor(8)), cursor(8)).is_ok());
        assert!(matches!(
            validate_monotonic_finalized_cursor(Some(cursor(8)), cursor(7)),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));
        let fork = ProviderIngestFinalizedCursorV1 {
            height: 8,
            block_hash: [0xFE; 32],
        };
        assert!(matches!(
            validate_monotonic_finalized_cursor(Some(cursor(8)), fork),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));

        let mut unassigned_completion = fixture_row(0x30);
        unassigned_completion
            .order
            .provider_completions
            .push(completion_record(
                ProviderId::new([0x99; 32]),
                account(9),
                8,
            ));
        assert!(matches!(
            validate_assignment(
                &unassigned_completion,
                cursor(8),
                LOCAL_PROVIDER,
                runtime_policy(),
            ),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));

        let mut inconsistent_status = fixture_row(0x31);
        inconsistent_status.order.status = ReplicationOrderStatus::Completed(8);
        assert!(matches!(
            validate_assignment(
                &inconsistent_status,
                cursor(8),
                LOCAL_PROVIDER,
                runtime_policy(),
            ),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));
    }

    #[tokio::test]
    async fn runtime_recovers_durable_finalized_high_water_before_scanning() {
        let row = fixture_row(0x2F);
        let (runtime, _, _, _) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime
            .outbox
            .observe_finalized_snapshot(cursor(9), 9_000)
            .expect("persist later cursor");
        let mut restarted = ProviderIngestRuntimeV1::new(
            runtime.provider_id,
            runtime.chain_id.clone(),
            runtime.claim_owner,
            runtime.policy,
            runtime.outbox.clone(),
            runtime.ledger.clone(),
            runtime.fetch.clone(),
            runtime.storage.clone(),
            runtime.payload_builder.clone(),
            runtime.signer_resolver.clone(),
            runtime.ingress.clone(),
            runtime.clock.clone(),
        )
        .expect("restart runtime");
        assert_eq!(restarted.last_finalized_cursor, Some(cursor(9)));
        assert!(matches!(
            restarted.tick().await,
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));
        assert_eq!(
            restarted.outbox.finalized_cursor_high_water().unwrap(),
            Some(cursor(9))
        );
    }

    #[tokio::test]
    async fn finalized_block_time_equivocation_is_rejected_after_restart() {
        let row = fixture_row(0x44);
        let (runtime, ledger, _, _) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime
            .outbox
            .observe_finalized_snapshot(cursor(8), 8_000)
            .expect("persist finalized snapshot");
        ledger.page.lock().unwrap().finalized_block_time_ms = 8_001;
        let mut restarted = ProviderIngestRuntimeV1::new(
            runtime.provider_id,
            runtime.chain_id.clone(),
            runtime.claim_owner,
            runtime.policy,
            runtime.outbox.clone(),
            runtime.ledger.clone(),
            runtime.fetch.clone(),
            runtime.storage.clone(),
            runtime.payload_builder.clone(),
            runtime.signer_resolver.clone(),
            runtime.ingress.clone(),
            runtime.clock.clone(),
        )
        .expect("restart runtime");
        assert!(matches!(
            restarted.tick().await,
            Err(ProviderIngestRuntimeErrorV1::Outbox(
                ProviderIngestOutboxError::FinalizedSnapshotConflict
            ))
        ));
        assert_eq!(
            restarted.outbox.finalized_snapshot_high_water().unwrap(),
            Some((cursor(8), 8_000))
        );
    }

    #[tokio::test]
    async fn local_existing_path_skips_network_and_preflights_before_ambiguity() {
        let row = fixture_row(0x32);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        let outcome = runtime.tick().await.expect("tick");
        assert_eq!(outcome.manifests_stored, 1);
        assert_eq!(outcome.completions_signed, 1);
        assert_eq!(outcome.completion_submissions, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            *ingress.events.lock().unwrap(),
            vec!["prepare_signed", "expose_ambiguous"]
        );
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn corrupt_authenticated_source_is_retryable_not_a_permanent_dead_letter() {
        let row = fixture_row(0x33);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("tick");
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::RetryScheduled {
                failure_class: ProviderIngestFailureClassV1::SourceRejected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn authenticated_source_binding_rejection_is_terminal_for_the_tick() {
        let row = fixture_row(0x34);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Err(ProviderIngestSourceFetchErrorV1::Rejected),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );

        assert!(matches!(
            runtime.tick().await,
            Err(ProviderIngestRuntimeErrorV1::SourceProtocolViolation)
        ));
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::RetryScheduled {
                failure_class: ProviderIngestFailureClassV1::SourceRejected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn ineligible_early_source_does_not_consume_fair_work_budget() {
        let first = fixture_row(0x10);
        let second = fixture_row(0x20);
        let (mut runtime, ledger, fetch, _) = test_runtime(
            first.clone(),
            false,
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        ledger.page.lock().unwrap().rows = vec![first.clone(), second.clone()];
        let first_authorization =
            validate_assignment(&first, cursor(8), LOCAL_PROVIDER, runtime_policy())
                .unwrap()
                .authorization;
        runtime.outbox.enqueue(first_authorization.clone()).unwrap();
        let claim = runtime
            .outbox
            .claim_source(
                first_authorization.job_id(),
                ProviderIngestClaimOwnerV1::new([0xDD; 32]).unwrap(),
                10_000,
                cursor(8),
            )
            .unwrap();
        runtime
            .outbox
            .schedule_source_retry(
                &claim,
                10_001,
                cursor(8),
                ProviderIngestFailureClassV1::SourceUnavailable,
            )
            .unwrap();
        runtime.policy.max_source_jobs_per_tick = 1;

        let outcome = runtime.tick().await.expect("fair source tick");
        assert_eq!(outcome.source_jobs_claimed, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        let second_authorization =
            validate_assignment(&second, cursor(8), LOCAL_PROVIDER, runtime_policy())
                .unwrap()
                .authorization;
        assert!(matches!(
            runtime
                .outbox
                .status(second_authorization.job_id())
                .unwrap()
                .state,
            ProviderIngestDeliveryStateV1::RetryScheduled {
                failure_class: ProviderIngestFailureClassV1::SourceRejected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn slow_fetch_renews_the_source_lease_until_atomic_storage_finishes() {
        let row = fixture_row(0x34);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Ok(vec![0xA5]),
            45,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("renewed tick");
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored { .. }
        ));
    }

    #[tokio::test]
    async fn semantic_completion_from_another_replica_wins_over_ambiguous_local_hash() {
        let row = fixture_row(0x35);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Ambiguous,
            false,
        );
        runtime.tick().await.expect("first tick");
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ambiguous { .. },
                ..
            }
        ));

        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0]
                .order
                .provider_completions
                .push(completion_record(
                    ProviderId::new(SOURCE_PROVIDER),
                    account(7),
                    8,
                ));
            page.rows[0]
                .order
                .provider_completions
                .push(completion_record(
                    ProviderId::new(LOCAL_PROVIDER),
                    account(9),
                    9,
                ));
            page.rows[0].order.status = ReplicationOrderStatus::Completed(9);
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("semantic reconciliation");
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::FinalizedCompleted {
                completion_epoch: 9,
                completed_by,
                committed_transaction_hash: None,
                ..
            } if completed_by == account(9)
        ));
    }

    #[tokio::test]
    async fn finalized_completion_first_row_bypasses_full_active_capacity() {
        let mut completed = fixture_row(0x2E);
        completed.order.provider_completions = vec![
            completion_record(ProviderId::new(SOURCE_PROVIDER), account(7), 8),
            completion_record(ProviderId::new(LOCAL_PROVIDER), account(9), 9),
        ];
        completed.order.status = ReplicationOrderStatus::Completed(9);
        completed.pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        completed.completion_epoch = Some(9);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            completed,
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        for seed in 0x40_u8..=0x5F {
            let pending = fixture_row(seed);
            let authorization =
                validate_assignment(&pending, cursor(8), LOCAL_PROVIDER, runtime_policy())
                    .unwrap()
                    .authorization;
            runtime.outbox.enqueue(authorization).expect("fill active");
        }
        assert_eq!(
            runtime
                .outbox
                .aggregate_counts()
                .expect("full active inventory")
                .active,
            runtime.outbox.policy().max_active_entries
        );

        let outcome = runtime.tick().await.expect("finalized reconciliation");
        assert_eq!(outcome.rows_scanned, 1);
        assert_eq!(outcome.jobs_inserted, 0);
        assert_eq!(outcome.jobs_finalized, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::FinalizedCompleted {
                manifest_id: None,
                completion_epoch: 9,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn preflight_rejection_resigns_without_entering_ambiguous_state() {
        let row = fixture_row(0x39);
        let (mut runtime, _, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Rejected);
        runtime.tick().await.expect("preflight rejection");
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::TransactionRejected),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn payload_and_preflight_failures_are_durably_backed_off() {
        let payload_row = fixture_row(0x3B);
        let (mut payload_runtime, _, _, payload_ingress) = test_runtime(
            payload_row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        payload_runtime.tick().await.expect("payload failure tick");
        assert!(matches!(
            payload_runtime
                .outbox
                .status(payload_ingress.job_id)
                .unwrap()
                .state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    attempts: 1,
                    last_failure_class: Some(
                        ProviderIngestFailureClassV1::PayloadPreparationFailed
                    ),
                    ..
                },
                ..
            }
        ));
        payload_runtime.tick().await.expect("payload backoff tick");
        assert!(matches!(
            payload_runtime
                .outbox
                .status(payload_ingress.job_id)
                .unwrap()
                .state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready { attempts: 1, .. },
                ..
            }
        ));

        let ingress_row = fixture_row(0x3C);
        let (mut ingress_runtime, _, _, ingress) = test_runtime(
            ingress_row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
        ingress_runtime.tick().await.expect("ingress unavailable");
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
        assert!(matches!(
            ingress_runtime
                .outbox
                .status(ingress.job_id)
                .unwrap()
                .state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed {
                    attempts: 2,
                    next_attempt_at_ms,
                    ..
                },
                ..
            } if next_attempt_at_ms > ingress_runtime.clock.now_ms()
        ));
        ingress_runtime.tick().await.expect("signed retry not due");
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
    }

    #[tokio::test]
    async fn ambiguous_unknown_retries_only_after_a_later_finalized_cursor() {
        let row = fixture_row(0x3A);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Ambiguous,
            false,
        );
        runtime.tick().await.expect("ambiguous submit");
        *ingress.observation.lock().unwrap() = ProviderIngestTransactionObservationV1::Unknown;

        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("finalized absence");
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed {
                    baseline_finalized_cursor,
                    ..
                },
                ..
            } if baseline_finalized_cursor == cursor(9)
        ));
    }

    #[tokio::test]
    async fn committed_hash_outcome_never_substitutes_for_semantic_completion() {
        let row = fixture_row(0x3D);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("submitted transaction");

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedSuccess;
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].completion_epoch = Some(9);
        }

        let outcome = runtime.tick().await.expect("committed-success observation");
        assert_eq!(outcome.jobs_finalized, 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedRejected;
        runtime
            .tick()
            .await
            .expect("committed rejection is retryable");
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::TransactionRejected),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn owner_rotation_reconciles_exposed_transaction_before_authority_change() {
        let row = fixture_row(0x3F);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("submitted transaction");

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedSuccess;
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].provider_owner = Some(account(9));
            page.rows[0].completion_authority = Some(ProviderIngestCompletionAuthorityV1::new(
                account(9),
                completion_signer_policy(1),
            ));
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("owner rotation");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn signer_policy_rotation_reconciles_exposed_transaction_before_authority_change() {
        let row = fixture_row(0x41);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("submitted transaction");

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedSuccess;
        runtime
            .signer_resolver
            .signer_policy_revision
            .store(2, Ordering::SeqCst);
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("signer policy rotation");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn owner_removal_reconciles_exposed_transaction_before_authority_change() {
        let row = fixture_row(0x40);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("submitted transaction");

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedSuccess;
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].provider_owner = None;
            page.rows[0].completion_authority = None;
            page.rows[0].completion_epoch = None;
        }

        runtime.tick().await.expect("owner removal");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn owner_rotation_invalidates_never_exposed_signed_bytes_without_preflight() {
        let row = fixture_row(0x42);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
        let first = runtime
            .tick()
            .await
            .expect("sign before unavailable preflight");
        assert_eq!(first.completions_signed, 1);
        assert_eq!(first.completion_submissions, 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed {
                    ever_exposed: false,
                    ..
                },
                ..
            }
        ));

        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].provider_owner = Some(account(9));
            page.rows[0].completion_authority = Some(ProviderIngestCompletionAuthorityV1::new(
                account(9),
                completion_signer_policy(1),
            ));
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("invalidate stale owner");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 0);
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::ProviderOwnerChanged),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn signer_policy_rotation_invalidates_never_exposed_signed_bytes_without_preflight() {
        let row = fixture_row(0x43);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
        runtime
            .tick()
            .await
            .expect("sign before unavailable preflight");
        runtime
            .signer_resolver
            .signer_policy_revision
            .store(2, Ordering::SeqCst);
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].completion_epoch = Some(9);
        }

        runtime
            .tick()
            .await
            .expect("invalidate stale signer policy");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 0);
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::SignerPolicyChanged),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn policy_rotation_after_durable_begin_never_reaches_ingress_exposure() {
        let row = fixture_row(0x44);
        let (mut runtime, _, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
        let first = runtime
            .tick()
            .await
            .expect("retain signed bytes after unavailable preflight");
        assert_eq!(first.completions_signed, 1);
        assert_eq!(first.completion_submissions, 0);
        let next_attempt_at_ms = match runtime.outbox.status(ingress.job_id).unwrap().state {
            ProviderIngestDeliveryStateV1::LocalStored {
                completion:
                    ProviderIngestCompletionStateV1::Signed {
                        next_attempt_at_ms,
                        ever_exposed: false,
                        ..
                    },
                ..
            } => next_attempt_at_ms,
            other => panic!("expected a signed retry, got {other:?}"),
        };

        *ingress.prepare_error.lock().unwrap() = None;
        runtime
            .signer_resolver
            .eligibility_flip_on_call
            .store(3, Ordering::SeqCst);
        runtime
            .signer_resolver
            .eligibility_flip_to_revision
            .store(2, Ordering::SeqCst);
        runtime
            .clock
            .base_ms
            .store(next_attempt_at_ms, Ordering::SeqCst);

        let second = runtime
            .tick()
            .await
            .expect("policy loss after durable begin is retryable");
        assert_eq!(second.completions_signed, 0);
        assert_eq!(second.completion_submissions, 0);
        assert_eq!(
            *ingress.events.lock().unwrap(),
            vec!["prepare_signed", "prepare_signed"]
        );
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed {
                    ever_exposed: true,
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn mutating_storage_soft_timeout_awaits_late_success_without_retry() {
        let row = fixture_row(0x3E);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );

        let outcome = runtime.tick().await.expect("late atomic store");
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert_eq!(outcome.manifests_stored, 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored { .. }
        ));
    }

    #[tokio::test]
    async fn wrong_owner_signer_is_released_and_fails_the_supervised_runtime() {
        let row = fixture_row(0x36);
        let (mut runtime, _, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            true,
        );
        assert!(matches!(
            runtime.tick().await,
            Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation)
        ));
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::SignerUnavailable),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn finalized_expiry_cancels_retained_work_without_fetching() {
        let mut row = fixture_row(0x37);
        let authorization = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
            .unwrap()
            .authorization;
        row.order.status = ReplicationOrderStatus::Expired(8);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        assert_eq!(authorization.job_id(), ingress.job_id);
        runtime.tick().await.expect("expiry tick");
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::Cancelled {
                reason: ProviderIngestCancellationReasonV1::OrderExpired,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn cooperative_shutdown_drains_active_store_before_skipping_next_row() {
        let first = fixture_row(0x3E);
        let second = fixture_row(0x40);
        let second_authorization =
            validate_assignment(&second, cursor(8), LOCAL_PROVIDER, runtime_policy())
                .expect("second assignment")
                .authorization;
        let (mut runtime, ledger, fetch, _) = test_runtime(
            first.clone(),
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        ledger.page.lock().unwrap().rows = vec![first, second];
        let shutdown_requested = AtomicBool::new(false);

        let request_shutdown = async {
            tokio::time::sleep(Duration::from_millis(270)).await;
            shutdown_requested.store(true, Ordering::Release);
        };
        let (result, ()) = tokio::join!(
            runtime.tick_with_shutdown(&shutdown_requested),
            request_shutdown
        );
        let outcome = result.expect("drained cooperative shutdown");

        assert_eq!(outcome.rows_scanned, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(second_authorization.job_id()),
            Err(ProviderIngestOutboxError::UnknownJob)
        ));
    }

    #[tokio::test]
    async fn pre_signalled_shutdown_returns_without_detaching_work() {
        let row = fixture_row(0x38);
        let (runtime, _, _, _) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        let (sender, receiver) = watch::channel(true);
        runtime.run(receiver).await.expect("clean shutdown");
        drop(sender);
    }
}
