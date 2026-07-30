//! Supervised runtime wiring for the committed SoraFS reputation projector.
//!
//! The parent module deliberately owns only deterministic projection. This
//! module supplies the production-side boundaries around it:
//!
//! - exact-anchor native finalized-query polling;
//! - durable PoR and provider-attributable stream-token journal production;
//! - external threshold-signer and Governance DAG acknowledgement
//!   reconciliation;
//! - a bounded durable read projection populated only after authoritative
//!   Governance DAG readback and projector acknowledgement.
//!
//! Every external dependency is identity pinned. No implementation in this
//! module accepts a private key, mutates a local reputation authority, or
//! treats submission success as finality.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use iroha_config::parameters::validate_production_runtime_handle;
use iroha_data_model::{
    ChainId,
    account::AccountId,
    isi::sorafs::{
        AppendSorafsPorReputationJournalEntry, AppendSorafsStreamTokenReputationJournalEntry,
    },
    query::sorafs::prelude::FindSorafsReputationJournalAuthorityPolicy,
    sorafs::{
        capacity::ProviderId,
        moderation_ledger::{
            REPAIR_QUERY_MAX_ITEMS_V1, RepairFinalizedEventCursorV1, RepairFinalizedEventPageV1,
        },
        orderbook::{
            ORDERBOOK_QUERY_MAX_ITEMS_V1, OrderbookFinalizedEventCursorV1,
            OrderbookFinalizedEventPageV1,
        },
        proof_ledger::{
            PROOF_OUTCOME_QUERY_MAX_ITEMS_V1, ProofOutcomeFinalizedEventCursorV1,
            ProofOutcomeFinalizedEventPageV1,
        },
        reputation::{
            PorTerminalOutcomeV1, REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1,
            ReputationJournalAuthorityPolicyRecordV1, ReputationJournalAuthorityPolicyV1,
            ReputationJournalEntryV1, ReputationJournalEventIdV1,
            ReputationJournalFinalizedCursorV1, ReputationJournalFinalizedEventCursorV1,
            ReputationJournalFinalizedEventPageV1, ReputationJournalPayloadV1,
            ReputationJournalSourceIdV1, ReputationJournalSourceKindV1,
            StreamTokenValidationBindingV1, StreamTokenValidationOutcomeV1,
        },
        reserve::{
            RESERVE_QUERY_MAX_ITEMS_V1, ReserveFinalizedEventCursorV1, ReserveFinalizedEventPageV1,
            ReserveProviderAccountPageV1,
        },
    },
};
use norito::{
    DecodeLimits, decode_from_bytes_with_limits,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use sorafs_manifest::{
    GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_LOG_VERSION_V1, GovernanceDagBlockV1,
    GovernanceDagHeadV1, GovernanceLogNodeV1, GovernanceLogPayloadV1, GovernanceLogSignatureV1,
    ReputationSnapshotEventV1, ReputationSnapshotV1,
    reputation::signed::{ReputationSnapshotTrustPolicyV1, SignedReputationSnapshotV1},
    validate_governance_dag_chain_v1,
};
use thiserror::Error;

use super::{
    REPUTATION_INGEST_MAX_PAGES_PER_BATCH_V1, ReputationCommittedEventIdentityV1,
    ReputationCommittedFeedCursorV1, ReputationCommittedFeedV1, ReputationFinalizedBatchV1,
    ReputationFinalizedIdentityV1, ReputationIngestError, ReputationIngestOutcomeV1,
    ReputationIngestPolicyV1, ReputationIngestService, ReputationUnsignedMaterialDeliveryStateV1,
    ReputationUnsignedMaterialDeliveryV1, ReputationUnsignedSigningMaterialV1,
};

use crate::durable_transaction_forwarder::{AtomicCheckpointStore, CheckpointStoreError};
#[cfg(test)]
use iroha_data_model::sorafs::reputation::{
    StreamTokenValidationRequestContextV1, StreamTokenValidationStatusV1,
};

/// Runtime finalized-query policy version.
pub const REPUTATION_FINALIZED_QUERY_POLICY_VERSION_V1: u8 = 1;
/// Journal-producer checkpoint version.
pub const REPUTATION_JOURNAL_PRODUCER_CHECKPOINT_VERSION_V1: u8 = 1;
/// Journal transaction-delivery worker policy version.
pub const REPUTATION_JOURNAL_DELIVERY_POLICY_VERSION_V1: u8 = 1;
/// Publication-reconciler checkpoint version.
pub const REPUTATION_PUBLICATION_CHECKPOINT_VERSION_V1: u8 = 1;
/// Governance DAG acknowledgement version.
pub const REPUTATION_GOVERNANCE_DAG_ACKNOWLEDGEMENT_VERSION_V1: u8 = 1;
/// Governance DAG signed-head inclusion readback version.
pub const REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1: u8 = 1;
/// Maximum signed Governance DAG blocks accepted in one inclusion readback.
pub const REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1: usize =
    sorafs_manifest::GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1;
/// Committed reputation read-projection version.
pub const REPUTATION_COMMITTED_READ_PROJECTION_VERSION_V1: u8 = 1;
/// Maximum authoritative snapshots and matching events retained by the read projection.
pub const REPUTATION_COMMITTED_READ_MAX_EVENTS_V1: usize = 1_024;
/// Maximum independently sequenced gateways retained by the token outbox.
pub const REPUTATION_STREAM_TOKEN_GATEWAY_HEADS_MAX_V1: usize = 1_024;
/// Maximum canonical counted-token admissions retained for bounded replay.
///
/// The dedicated 4,096-row cap is large enough to pin one authenticated head
/// for every allowed gateway while bounding the complete canonical entries
/// retained behind those heads. It is intentionally independent of the much
/// larger generic completed-tombstone limit.
pub const REPUTATION_STREAM_TOKEN_GATEWAY_ADMISSIONS_MAX_V1: usize = 4_096;
/// Canonical durable journal-producer checkpoint file.
pub const REPUTATION_JOURNAL_PRODUCER_CHECKPOINT_FILE_NAME_V1: &str =
    "reputation-journal-producer-v1.to";
/// Canonical durable journal-producer lock file.
pub const REPUTATION_JOURNAL_PRODUCER_LOCK_FILE_NAME_V1: &str =
    "reputation-journal-producer-v1.lock";
/// Canonical durable publication-reconciler checkpoint file.
pub const REPUTATION_PUBLICATION_CHECKPOINT_FILE_NAME_V1: &str =
    "reputation-publication-reconciler-v1.to";
/// Canonical durable publication-reconciler lock file.
pub const REPUTATION_PUBLICATION_LOCK_FILE_NAME_V1: &str =
    "reputation-publication-reconciler-v1.lock";

/// Exact public provider-contract revision accepted by the V1 runtime.
pub const REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1: u64 = 1;
/// Maximum bytes in a pinned Governance DAG peer identity.
pub const REPUTATION_RUNTIME_MAX_GOVERNANCE_PEER_ID_BYTES_V1: usize = 128;
/// Default hard pending-operation ceiling for the journal producer.
pub const REPUTATION_JOURNAL_PRODUCER_MAX_PENDING_V1: u32 = 65_536;
/// Default hard completed-tombstone ceiling for the journal producer.
pub const REPUTATION_JOURNAL_PRODUCER_MAX_COMPLETED_V1: u32 = 262_144;
/// Default hard dead-letter ceiling for the journal producer.
pub const REPUTATION_JOURNAL_PRODUCER_MAX_DEAD_LETTERS_V1: u32 = 65_536;
/// Default retry ceiling for one journal append.
pub const REPUTATION_JOURNAL_PRODUCER_MAX_ATTEMPTS_V1: u32 = 16;
/// Default hard journal pages consumed by one delivery tick.
pub const REPUTATION_JOURNAL_DELIVERY_MAX_PAGES_PER_TICK_V1: u32 = 64;
/// Default hard submissions begun by one delivery tick.
pub const REPUTATION_JOURNAL_DELIVERY_MAX_SUBMISSIONS_PER_TICK_V1: u32 = 256;
/// Maximum retained recorder-policy revisions in one producer checkpoint.
pub const REPUTATION_JOURNAL_PRODUCER_MAX_POLICY_REVISIONS_V1: usize = 1_024;
/// Maximum canonical journal-producer checkpoint bytes.
pub const REPUTATION_JOURNAL_PRODUCER_MAX_CHECKPOINT_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Maximum canonical publication-reconciler checkpoint bytes.
pub const REPUTATION_PUBLICATION_MAX_CHECKPOINT_BYTES_V1: u64 = 32 * 1024 * 1024;
/// Minimum accepted checkpoint ceiling for either runtime store.
pub const REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1: u64 = 64 * 1024;

const RUNTIME_CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT: usize = 16;
const RUNTIME_CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT: usize = 20;
const RUNTIME_CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 2 * 1024 * 1024;
const RUNTIME_CHECKPOINT_MAX_NESTING_DEPTH: usize = 96;

/// Payload-free failure returned by an injected runtime dependency.
///
/// The receipt must be stable for an exact failed attempt. It is retained only
/// as an idempotency digest and is safe to expose in status or logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("external reputation dependency failed")]
pub struct ReputationExternalFailureV1 {
    receipt: [u8; 32],
}

impl ReputationExternalFailureV1 {
    /// Construct a non-inert payload-free failure receipt.
    ///
    /// # Errors
    ///
    /// Returns [`ReputationRuntimeError::InvalidExternalReceipt`] for zero.
    pub fn try_new(receipt: [u8; 32]) -> Result<Self, ReputationRuntimeError> {
        if receipt == [0; 32] {
            return Err(ReputationRuntimeError::InvalidExternalReceipt);
        }
        Ok(Self { receipt })
    }

    /// Return the stable payload-free failure receipt.
    #[must_use]
    pub const fn receipt(self) -> [u8; 32] {
        self.receipt
    }
}

/// Public, non-secret qualification for one reputation runtime provider.
///
/// The revision identifies the V1 provider contract. The digest is supplied
/// by an independently constructed runtime policy rather than learned from the
/// provider at first use. Runtime components require both values to match
/// before and after every external operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationRuntimeProviderQualificationV1 {
    revision: u64,
    policy_digest: [u8; 32],
}

impl ReputationRuntimeProviderQualificationV1 {
    /// Construct one provider qualification observation.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }

    /// Return the exact public provider-contract revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Return the digest of the independently governed public policy.
    #[must_use]
    pub const fn policy_digest(self) -> [u8; 32] {
        self.policy_digest
    }

    fn is_valid(self) -> bool {
        self.revision != 0 && self.policy_digest != [0; 32]
    }
}

/// Stable identity and qualification shared by every reputation provider.
///
/// Implementations retain credentials, key identifiers, and vendor
/// diagnostics inside their protected boundary. Qualification must fail for
/// unavailable, revoked, stale, or otherwise ineligible providers.
pub trait ReputationRuntimeProviderV1: Send + Sync + fmt::Debug {
    /// Opaque deployment handle, stable for the provider lifetime.
    fn handle(&self) -> &str;

    /// Observe the active public provider revision and policy digest.
    ///
    /// # Errors
    ///
    /// Returns a payload-free receipt when the provider is unavailable,
    /// revoked, stale, or otherwise cannot prove the active qualification.
    fn qualification(
        &self,
    ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1>;
}

/// One immutable finalized chain view selected for a runtime poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationFinalizedAnchorV1 {
    /// Exact active chain.
    pub chain_id: ChainId,
    /// Finalized height and hash.
    pub identity: ReputationFinalizedIdentityV1,
    /// Timestamp of that exact finalized block in Unix milliseconds.
    pub finalized_at_unix_ms: u64,
}

impl ReputationFinalizedAnchorV1 {
    fn validate(&self) -> Result<(), ReputationRuntimeError> {
        self.identity
            .validate()
            .map_err(ReputationRuntimeError::Projector)?;
        if self.chain_id.as_str().is_empty()
            || self.finalized_at_unix_ms == 0
            || self.finalized_at_unix_ms == u64::MAX
        {
            return Err(ReputationRuntimeError::InvalidFinalizedAnchor);
        }
        Ok(())
    }
}

/// Policy and journal page read from one immutable finalized state view.
///
/// The query adapter must execute
/// [`FindSorafsReputationJournalAuthorityPolicy`] and the journal event query
/// before releasing the same state-view guard. Echoing the finalized cursor
/// makes an accidental current-head/historical-page mix detectable by the
/// worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationJournalDeliveryFinalizedViewV1 {
    /// Exact chain, block identity, and block timestamp of the immutable view.
    pub anchor: ReputationFinalizedAnchorV1,
    /// Complete bounded recorder-policy predecessor chain from revision one
    /// through `authority_policy`, read under the same immutable view guard.
    pub authority_policy_history: Vec<ReputationJournalAuthorityPolicyRecordV1>,
    /// Result of the typed active-policy query at `anchor`.
    pub authority_policy: ReputationJournalAuthorityPolicyRecordV1,
    /// Result of the typed committed-event query at `anchor`.
    pub journal_page: ReputationJournalFinalizedEventPageV1,
}

impl ReputationJournalDeliveryFinalizedViewV1 {
    /// Validate this response against the exact immutable query request.
    ///
    /// This checks the chain and upper-height selection, active authority
    /// policy, finalized block time, journal continuation, requested row
    /// bound, and the journal page's exact finalized cursor as one unit.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed runtime error when any response field is
    /// malformed or does not match the exact request.
    pub fn validate_for_request(
        &self,
        chain_id: &ChainId,
        requested_after: Option<ReputationJournalFinalizedEventCursorV1>,
        requested_limit: u32,
        maximum_height: u64,
    ) -> Result<(), ReputationRuntimeError> {
        let requested_limit = usize::try_from(requested_limit)
            .map_err(|_| ReputationRuntimeError::QueryResourceExhausted)?;
        if requested_limit == 0 || requested_limit > REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1 {
            return Err(ReputationRuntimeError::QueryResourceExhausted);
        }
        self.anchor.validate()?;
        if &self.anchor.chain_id != chain_id {
            return Err(ReputationRuntimeError::ChainIdMismatch);
        }
        if self.anchor.identity.height > maximum_height {
            return Err(ReputationRuntimeError::FinalizedAnchorPastTarget);
        }
        self.authority_policy
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidAuthorityPolicy)?;
        if self.authority_policy.activated_at_unix_ms > self.anchor.finalized_at_unix_ms {
            return Err(ReputationRuntimeError::InvalidAuthorityPolicy);
        }
        validate_authority_policy_history(
            &self.authority_policy_history,
            &self.authority_policy,
            self.anchor.finalized_at_unix_ms,
        )?;
        self.journal_page
            .validate_after(requested_after)
            .map_err(|_| ReputationRuntimeError::InvalidQueryPage)?;
        if self.journal_page.events.len() > requested_limit
            || self.journal_page.finalized_cursor.height != self.anchor.identity.height
            || self.journal_page.finalized_cursor.block_hash != self.anchor.identity.block_hash
            || self.journal_page.finalized_cursor.finalized_at_unix_ms
                != self.anchor.finalized_at_unix_ms
        {
            return Err(ReputationRuntimeError::InvalidQueryPage);
        }
        Ok(())
    }
}

fn validate_authority_policy_history(
    history: &[ReputationJournalAuthorityPolicyRecordV1],
    active: &ReputationJournalAuthorityPolicyRecordV1,
    finalized_at_unix_ms: u64,
) -> Result<(), ReputationRuntimeError> {
    if history.is_empty()
        || history.len() > REPUTATION_JOURNAL_PRODUCER_MAX_POLICY_REVISIONS_V1
        || history.last() != Some(active)
    {
        return Err(ReputationRuntimeError::AuthorityPolicyLineage);
    }
    let mut previous: Option<&ReputationJournalAuthorityPolicyRecordV1> = None;
    for record in history {
        record
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidAuthorityPolicy)?;
        if record.activated_at_unix_ms > finalized_at_unix_ms {
            return Err(ReputationRuntimeError::InvalidAuthorityPolicy);
        }
        match previous {
            None => {
                if record.policy.revision != 1 || record.policy.predecessor_policy_digest.is_some()
                {
                    return Err(ReputationRuntimeError::AuthorityPolicyLineage);
                }
            }
            Some(predecessor)
                if predecessor.policy.revision.checked_add(1) == Some(record.policy.revision)
                    && record.policy.predecessor_policy_digest
                        == Some(predecessor.policy_digest)
                    && predecessor.activated_at_unix_ms <= record.activated_at_unix_ms => {}
            Some(_) => return Err(ReputationRuntimeError::AuthorityPolicyLineage),
        }
        previous = Some(record);
    }
    Ok(())
}

/// Identity-pinned source for all native finalized reputation queries.
///
/// Implementations must execute every page method against the exact `anchor`
/// supplied by the caller. A node that cannot serve the requested immutable
/// view must return a failure rather than silently selecting a newer view.
pub trait ReputationFinalizedQueryV1: ReputationRuntimeProviderV1 {
    /// Return the highest finalized anchor at or below `maximum_height`.
    ///
    /// Once the chain has finalized `maximum_height`, the implementation must
    /// return that exact historical anchor rather than the current head.
    fn finalized_at_or_before(
        &self,
        chain_id: &ChainId,
        maximum_height: u64,
    ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1>;

    /// Execute the typed active-policy, complete bounded policy-history, and
    /// journal-page queries in one view.
    ///
    /// Implementations must hold one immutable state-view/archive-generation
    /// guard while resolving all three projections. They must not assemble
    /// this response from separate current-head reads, even when those reads
    /// report the same height.
    fn reputation_journal_delivery_view(
        &self,
        chain_id: &ChainId,
        maximum_height: u64,
        _policy_query: FindSorafsReputationJournalAuthorityPolicy,
        after: Option<ReputationJournalFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<ReputationJournalDeliveryFinalizedViewV1, ReputationExternalFailureV1>;

    /// Fetch one proof-outcome page after the exclusive cursor.
    fn proof_outcome_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ProofOutcomeFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<ProofOutcomeFinalizedEventPageV1, ReputationExternalFailureV1>;

    /// Fetch one unified PoR/dispute/token journal page.
    fn reputation_journal_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReputationJournalFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<ReputationJournalFinalizedEventPageV1, ReputationExternalFailureV1>;

    /// Fetch one native repair-event page.
    fn repair_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<RepairFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<RepairFinalizedEventPageV1, ReputationExternalFailureV1>;

    /// Fetch one native orderbook-event page.
    fn orderbook_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<OrderbookFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<OrderbookFinalizedEventPageV1, ReputationExternalFailureV1>;

    /// Fetch one native reserve-event page.
    fn reserve_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReserveFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<ReserveFinalizedEventPageV1, ReputationExternalFailureV1>;

    /// Fetch one authoritative reserve-provider projection page.
    fn reserve_provider_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after_provider_id: Option<ProviderId>,
        limit: u32,
    ) -> Result<ReserveProviderAccountPageV1, ReputationExternalFailureV1>;
}

/// Strict construction policy for exact-anchor finalized polling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationFinalizedQueryPolicyV1 {
    version: u8,
    chain_id: ChainId,
    window_end_height: u64,
    ingest_policy_digest: [u8; 32],
    query_handle: String,
    query_qualification: ReputationRuntimeProviderQualificationV1,
    page_items: u32,
    max_pages_per_batch: u32,
}

impl ReputationFinalizedQueryPolicyV1 {
    /// Construct the strict V1 polling policy from the exact ingest policy.
    ///
    /// # Errors
    ///
    /// Rejects an invalid ingest policy, handle, page size, or page budget.
    pub fn try_new(
        ingest_policy: &ReputationIngestPolicyV1,
        query_handle: impl Into<String>,
        page_items: u32,
        max_pages_per_batch: u32,
    ) -> Result<Self, ReputationRuntimeError> {
        ingest_policy.validate()?;
        let query_handle = validate_runtime_handle(query_handle.into())?;
        let native_page_ceiling = [
            u32::try_from(PROOF_OUTCOME_QUERY_MAX_ITEMS_V1)
                .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?,
            u32::try_from(REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
                .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?,
            REPAIR_QUERY_MAX_ITEMS_V1,
            ORDERBOOK_QUERY_MAX_ITEMS_V1,
            RESERVE_QUERY_MAX_ITEMS_V1,
        ]
        .into_iter()
        .min()
        .ok_or(ReputationRuntimeError::InvalidRuntimePolicy)?;
        let reserve_projection_pages = ingest_policy
            .max_providers
            .checked_add(RESERVE_QUERY_MAX_ITEMS_V1.saturating_sub(1))
            .and_then(|count| count.checked_div(RESERVE_QUERY_MAX_ITEMS_V1))
            .ok_or(ReputationRuntimeError::InvalidRuntimePolicy)?;
        let minimum_complete_batch_pages = reserve_projection_pages
            .checked_add(5)
            .ok_or(ReputationRuntimeError::InvalidRuntimePolicy)?;
        if page_items == 0
            || page_items > native_page_ceiling
            || max_pages_per_batch < minimum_complete_batch_pages
            || max_pages_per_batch > ingest_policy.max_pages_per_batch
            || max_pages_per_batch > REPUTATION_INGEST_MAX_PAGES_PER_BATCH_V1
        {
            return Err(ReputationRuntimeError::InvalidRuntimePolicy);
        }
        let ingest_policy_digest = ingest_policy.canonical_digest()?;
        Ok(Self {
            version: REPUTATION_FINALIZED_QUERY_POLICY_VERSION_V1,
            chain_id: ingest_policy.chain_id.clone(),
            window_end_height: ingest_policy.window_end_height,
            ingest_policy_digest,
            query_handle,
            query_qualification: ReputationRuntimeProviderQualificationV1::new(
                REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
                ingest_policy_digest,
            ),
            page_items,
            max_pages_per_batch,
        })
    }

    /// Exact active chain selected by the policy.
    #[must_use]
    pub const fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    /// Inclusive finalized target for this immutable release window.
    #[must_use]
    pub const fn window_end_height(&self) -> u64 {
        self.window_end_height
    }

    /// Return the independently derived finalized-query qualification.
    #[must_use]
    pub const fn query_qualification(&self) -> ReputationRuntimeProviderQualificationV1 {
        self.query_qualification
    }

    /// Qualify or revalidate the exact finalized-query provider.
    ///
    /// # Errors
    ///
    /// Rejects malformed, test-marked, substituted, stale, or policy-mismatched
    /// providers without exposing provider diagnostics.
    pub fn revalidate_provider(
        &self,
        query: &dyn ReputationFinalizedQueryV1,
    ) -> Result<(), ReputationRuntimeError> {
        assert_runtime_provider_qualification(&self.query_handle, self.query_qualification, query)
    }
}

/// Result of one exact-anchor projector poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationFinalizedPollOutcomeV1 {
    /// The projector already completed the governed release target.
    Complete,
    /// A coherent batch was durably applied.
    Applied {
        /// Number of new committed events.
        events: u32,
    },
    /// The exact coherent batch was already durable.
    ExactReplay,
}

/// Payload-free status of the supervised finalized-query runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationFinalizedRuntimeStatusV1 {
    /// Whether the runtime has completed at least one successful poll.
    pub live: bool,
    /// Whether every required source is complete at the release target.
    pub ready: bool,
    /// Consecutive failed polls since the latest success.
    pub consecutive_failures: u64,
    /// Latest successfully selected finalized identity.
    pub latest_anchor: Option<ReputationFinalizedIdentityV1>,
    /// Payload-free receipt from the latest external query failure.
    pub latest_failure_receipt: Option<[u8; 32]>,
}

#[derive(Debug, Default)]
struct FinalizedSupervisorState {
    live: bool,
    ready: bool,
    consecutive_failures: u64,
    latest_anchor: Option<ReputationFinalizedIdentityV1>,
    latest_failure_receipt: Option<[u8; 32]>,
}

/// Supervised exact-anchor query runner around [`ReputationIngestService`].
#[derive(Debug)]
pub struct ReputationCommittedProjectorRuntimeV1 {
    projector: Arc<ReputationIngestService>,
    policy: ReputationFinalizedQueryPolicyV1,
    query: Arc<dyn ReputationFinalizedQueryV1>,
    state: Mutex<FinalizedSupervisorState>,
    reconcile_lock: Mutex<()>,
}

impl ReputationCommittedProjectorRuntimeV1 {
    /// Bind the exact ingest policy, projector, and identity-pinned query source.
    ///
    /// # Errors
    ///
    /// Construction fails before polling if any policy digest or runtime handle
    /// differs from the supplied production contract.
    pub fn new(
        projector: Arc<ReputationIngestService>,
        ingest_policy: &ReputationIngestPolicyV1,
        policy: ReputationFinalizedQueryPolicyV1,
        query: Arc<dyn ReputationFinalizedQueryV1>,
    ) -> Result<Self, ReputationRuntimeError> {
        ingest_policy.validate()?;
        if policy.version != REPUTATION_FINALIZED_QUERY_POLICY_VERSION_V1
            || policy.chain_id != ingest_policy.chain_id
            || policy.window_end_height != ingest_policy.window_end_height
            || policy.ingest_policy_digest != ingest_policy.canonical_digest()?
            || projector.status()?.policy_digest != policy.ingest_policy_digest
        {
            return Err(ReputationRuntimeError::RuntimeBindingMismatch);
        }
        qualify_runtime_provider(
            &policy.query_handle,
            policy.query_qualification,
            query.as_ref(),
        )?;
        Ok(Self {
            projector,
            policy,
            query,
            state: Mutex::new(FinalizedSupervisorState::default()),
            reconcile_lock: Mutex::new(()),
        })
    }

    /// Poll every physical committed feed through one immutable finalized view.
    ///
    /// This is the unit scheduled by the deployment supervisor. It performs no
    /// sleeping and holds no runtime-state lock while invoking the query source.
    ///
    /// # Errors
    ///
    /// Fails closed for handle substitution, anchor rollback/fork, missed
    /// release target, malformed continuation, resource exhaustion, external
    /// query failure, or projector rejection.
    pub fn reconcile_once(
        &self,
    ) -> Result<ReputationFinalizedPollOutcomeV1, ReputationRuntimeError> {
        let _guard = self
            .reconcile_lock
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let result = self.reconcile_once_inner();
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        match &result {
            Ok(outcome) => {
                state.live = true;
                state.consecutive_failures = 0;
                state.latest_failure_receipt = None;
                state.ready = matches!(outcome, ReputationFinalizedPollOutcomeV1::Complete)
                    || projector_ready_at(&self.projector, self.policy.window_end_height)?;
            }
            Err(error) => {
                state.ready = false;
                state.consecutive_failures = state.consecutive_failures.saturating_add(1);
                state.latest_failure_receipt = error.external_receipt();
            }
        }
        result
    }

    /// Return payload-free liveness/readiness state.
    ///
    /// # Errors
    ///
    /// Returns a runtime-lock error if the supervisor state was poisoned.
    pub fn status(&self) -> Result<ReputationFinalizedRuntimeStatusV1, ReputationRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        Ok(ReputationFinalizedRuntimeStatusV1 {
            live: state.live,
            ready: state.ready,
            consecutive_failures: state.consecutive_failures,
            latest_anchor: state.latest_anchor,
            latest_failure_receipt: state.latest_failure_receipt,
        })
    }

    fn reconcile_once_inner(
        &self,
    ) -> Result<ReputationFinalizedPollOutcomeV1, ReputationRuntimeError> {
        self.ensure_query_binding()?;
        if projector_ready_at(&self.projector, self.policy.window_end_height)? {
            return Ok(ReputationFinalizedPollOutcomeV1::Complete);
        }

        self.ensure_query_binding()?;
        let anchor_result = self
            .query
            .finalized_at_or_before(&self.policy.chain_id, self.policy.window_end_height);
        self.ensure_query_binding()?;
        let anchor = anchor_result?;
        anchor.validate()?;
        if anchor.chain_id != self.policy.chain_id {
            return Err(ReputationRuntimeError::ChainIdMismatch);
        }
        if anchor.identity.height > self.policy.window_end_height {
            return Err(ReputationRuntimeError::FinalizedAnchorPastTarget);
        }
        self.validate_anchor_progress(&anchor)?;

        let cursors = committed_cursor_inventory(&self.projector)?;
        let mut page_budget = PageBudget::new(self.policy.max_pages_per_batch);
        let proof_pages = self.collect_proof_pages(
            &anchor,
            cursor_after(&cursors, ReputationCommittedFeedV1::Proof)?,
            &mut page_budget,
        )?;
        let journal_pages = self.collect_journal_pages(
            &anchor,
            cursor_after(&cursors, ReputationCommittedFeedV1::Journal)?,
            &mut page_budget,
        )?;
        let repair_pages = self.collect_repair_pages(
            &anchor,
            cursor_after(&cursors, ReputationCommittedFeedV1::Repair)?,
            &mut page_budget,
        )?;
        let orderbook_pages = self.collect_orderbook_pages(
            &anchor,
            cursor_after(&cursors, ReputationCommittedFeedV1::Orderbook)?,
            &mut page_budget,
        )?;
        let reserve_pages = self.collect_reserve_pages(
            &anchor,
            cursor_after(&cursors, ReputationCommittedFeedV1::Reserve)?,
            &mut page_budget,
        )?;
        let reserve_provider_pages = if reserve_pages.last().is_some_and(|page| !page.has_more) {
            self.collect_reserve_provider_pages(&anchor, &mut page_budget)?
        } else {
            Vec::new()
        };

        let outcome = self
            .projector
            .ingest_finalized_batch(ReputationFinalizedBatchV1 {
                chain_id: self.policy.chain_id.clone(),
                finalized_at_unix_ms: anchor.finalized_at_unix_ms,
                proof_pages,
                journal_pages,
                repair_pages,
                orderbook_pages,
                reserve_pages,
                reserve_provider_pages,
            })?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        state.latest_anchor = Some(anchor.identity);
        drop(state);
        Ok(match outcome {
            ReputationIngestOutcomeV1::Applied { events } => {
                ReputationFinalizedPollOutcomeV1::Applied { events }
            }
            ReputationIngestOutcomeV1::ExactReplay => ReputationFinalizedPollOutcomeV1::ExactReplay,
        })
    }

    fn ensure_query_binding(&self) -> Result<(), ReputationRuntimeError> {
        self.policy.revalidate_provider(self.query.as_ref())
    }

    fn validate_anchor_progress(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
    ) -> Result<(), ReputationRuntimeError> {
        let status = self.projector.status()?;
        if status.policy_digest != self.policy.ingest_policy_digest {
            return Err(ReputationRuntimeError::RuntimeBindingChanged);
        }
        if let Some(previous) = status.latest_finalized {
            if anchor.identity.height < previous.height
                || anchor.finalized_at_unix_ms < status.latest_finalized_at_unix_ms
            {
                return Err(ReputationRuntimeError::FinalizedRollback);
            }
            if anchor.identity.height == previous.height
                && (anchor.identity.block_hash != previous.block_hash
                    || anchor.finalized_at_unix_ms != status.latest_finalized_at_unix_ms)
            {
                return Err(ReputationRuntimeError::FinalizedFork);
            }
        }
        Ok(())
    }

    fn collect_proof_pages(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReputationCommittedEventIdentityV1>,
        budget: &mut PageBudget,
    ) -> Result<Vec<ProofOutcomeFinalizedEventPageV1>, ReputationRuntimeError> {
        let after = after.map(proof_cursor);
        budget.take()?;
        self.ensure_query_binding()?;
        let page_result = self
            .query
            .proof_outcome_page(anchor, after, self.policy.page_items);
        self.ensure_query_binding()?;
        let page = page_result?;
        validate_event_page(
            anchor.identity,
            after.map(committed_from_proof_cursor),
            page.events.len(),
            page.has_more,
            page.next_after.map(committed_from_proof_cursor),
            page.events
                .last()
                .map(|event| committed_from_proof_cursor(event.cursor())),
            page.finalized_cursor.height,
            page.finalized_cursor.block_hash,
            self.policy.page_items,
        )?;
        Ok(vec![page])
    }

    fn collect_journal_pages(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReputationCommittedEventIdentityV1>,
        budget: &mut PageBudget,
    ) -> Result<Vec<ReputationJournalFinalizedEventPageV1>, ReputationRuntimeError> {
        let after = after.map(journal_cursor);
        budget.take()?;
        self.ensure_query_binding()?;
        let page_result = self
            .query
            .reputation_journal_page(anchor, after, self.policy.page_items);
        self.ensure_query_binding()?;
        let page = page_result?;
        page.validate_after(after)
            .map_err(|_| ReputationRuntimeError::InvalidQueryPage)?;
        if page.finalized_cursor
            != (ReputationJournalFinalizedCursorV1 {
                height: anchor.identity.height,
                block_hash: anchor.identity.block_hash,
                finalized_at_unix_ms: anchor.finalized_at_unix_ms,
            })
            || page.events.len()
                > usize::try_from(self.policy.page_items)
                    .map_err(|_| ReputationRuntimeError::QueryResourceExhausted)?
        {
            return Err(ReputationRuntimeError::InvalidQueryPage);
        }
        Ok(vec![page])
    }

    fn collect_repair_pages(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReputationCommittedEventIdentityV1>,
        budget: &mut PageBudget,
    ) -> Result<Vec<RepairFinalizedEventPageV1>, ReputationRuntimeError> {
        let after = after.map(repair_cursor);
        budget.take()?;
        self.ensure_query_binding()?;
        let page_result = self
            .query
            .repair_page(anchor, after, self.policy.page_items);
        self.ensure_query_binding()?;
        let page = page_result?;
        validate_event_page(
            anchor.identity,
            after.map(committed_from_repair_cursor),
            page.events.len(),
            page.has_more,
            page.next_after.map(committed_from_repair_cursor),
            page.events
                .last()
                .map(|event| committed_from_repair_cursor(event.cursor())),
            page.finalized_cursor.height,
            page.finalized_cursor.block_hash,
            self.policy.page_items,
        )?;
        Ok(vec![page])
    }

    fn collect_orderbook_pages(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReputationCommittedEventIdentityV1>,
        budget: &mut PageBudget,
    ) -> Result<Vec<OrderbookFinalizedEventPageV1>, ReputationRuntimeError> {
        let after = after.map(orderbook_cursor);
        budget.take()?;
        self.ensure_query_binding()?;
        let page_result = self
            .query
            .orderbook_page(anchor, after, self.policy.page_items);
        self.ensure_query_binding()?;
        let page = page_result?;
        validate_event_page(
            anchor.identity,
            after.map(committed_from_orderbook_cursor),
            page.events.len(),
            page.has_more,
            page.next_after.map(committed_from_orderbook_cursor),
            page.events
                .last()
                .map(|event| committed_from_orderbook_cursor(event.cursor())),
            page.finalized_cursor.height,
            page.finalized_cursor.block_hash,
            self.policy.page_items,
        )?;
        Ok(vec![page])
    }

    fn collect_reserve_pages(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReputationCommittedEventIdentityV1>,
        budget: &mut PageBudget,
    ) -> Result<Vec<ReserveFinalizedEventPageV1>, ReputationRuntimeError> {
        let after = after.map(reserve_cursor);
        budget.take()?;
        self.ensure_query_binding()?;
        let page_result = self
            .query
            .reserve_page(anchor, after, self.policy.page_items);
        self.ensure_query_binding()?;
        let page = page_result?;
        validate_event_page(
            anchor.identity,
            after.map(committed_from_reserve_cursor),
            page.events.len(),
            page.has_more,
            page.next_after.map(committed_from_reserve_cursor),
            page.events
                .last()
                .map(|event| committed_from_reserve_cursor(event.cursor())),
            page.finalized_cursor.height,
            page.finalized_cursor.block_hash,
            self.policy.page_items,
        )?;
        Ok(vec![page])
    }

    fn collect_reserve_provider_pages(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        budget: &mut PageBudget,
    ) -> Result<Vec<ReserveProviderAccountPageV1>, ReputationRuntimeError> {
        let mut after = None;
        let mut pages = Vec::new();
        loop {
            budget.take()?;
            self.ensure_query_binding()?;
            let page_result =
                self.query
                    .reserve_provider_page(anchor, after, RESERVE_QUERY_MAX_ITEMS_V1);
            self.ensure_query_binding()?;
            let page = page_result?;
            if page.finalized_cursor.height != anchor.identity.height
                || page.finalized_cursor.block_hash != anchor.identity.block_hash
                || page.accounts.len()
                    > usize::try_from(RESERVE_QUERY_MAX_ITEMS_V1)
                        .map_err(|_| ReputationRuntimeError::QueryResourceExhausted)?
            {
                return Err(ReputationRuntimeError::InvalidQueryPage);
            }
            let last = page
                .accounts
                .last()
                .map(|account| account.terms.provider_id);
            match (page.has_more, page.next_after, last) {
                (true, Some(next), Some(last)) if Some(next) != after && next == last => {}
                (false, None, _) => {}
                _ => return Err(ReputationRuntimeError::InvalidQueryContinuation),
            }
            let has_more = page.has_more;
            after = page.next_after;
            pages.push(page);
            if !has_more {
                return Ok(pages);
            }
        }
    }
}

#[derive(Debug)]
struct PageBudget {
    remaining: u32,
}

impl PageBudget {
    const fn new(remaining: u32) -> Self {
        Self { remaining }
    }

    fn take(&mut self) -> Result<(), ReputationRuntimeError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(ReputationRuntimeError::QueryResourceExhausted)?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_event_page(
    target: ReputationFinalizedIdentityV1,
    requested_after: Option<ReputationCommittedEventIdentityV1>,
    event_count: usize,
    has_more: bool,
    next_after: Option<ReputationCommittedEventIdentityV1>,
    last_event: Option<ReputationCommittedEventIdentityV1>,
    page_height: u64,
    page_hash: [u8; 32],
    page_items: u32,
) -> Result<(), ReputationRuntimeError> {
    if page_height != target.height
        || page_hash != target.block_hash
        || event_count
            > usize::try_from(page_items)
                .map_err(|_| ReputationRuntimeError::QueryResourceExhausted)?
    {
        return Err(ReputationRuntimeError::InvalidQueryPage);
    }
    match (has_more, next_after, last_event) {
        (true, Some(next), Some(last))
            if next == last && requested_after.is_none_or(|after| next > after) => {}
        (false, None, _) => {}
        _ => return Err(ReputationRuntimeError::InvalidQueryContinuation),
    }
    Ok(())
}

fn committed_cursor_inventory(
    projector: &ReputationIngestService,
) -> Result<Vec<ReputationCommittedFeedCursorV1>, ReputationRuntimeError> {
    let cursors = projector.committed_feed_cursors()?;
    if cursors.len() != 5 {
        return Err(ReputationRuntimeError::InvalidCursorInventory);
    }
    let feeds = cursors
        .iter()
        .map(|cursor| cursor.feed)
        .collect::<BTreeSet<_>>();
    if feeds.len() != 5 {
        return Err(ReputationRuntimeError::InvalidCursorInventory);
    }
    Ok(cursors)
}

fn cursor_after(
    cursors: &[ReputationCommittedFeedCursorV1],
    feed: ReputationCommittedFeedV1,
) -> Result<Option<ReputationCommittedEventIdentityV1>, ReputationRuntimeError> {
    cursors
        .iter()
        .find(|cursor| cursor.feed == feed)
        .map(|cursor| cursor.after)
        .ok_or(ReputationRuntimeError::InvalidCursorInventory)
}

fn projector_ready_at(
    projector: &ReputationIngestService,
    target_height: u64,
) -> Result<bool, ReputationRuntimeError> {
    let status = projector.status()?;
    Ok(status
        .latest_finalized
        .is_some_and(|identity| identity.height == target_height)
        && status.missing_sources.is_empty())
}

const fn proof_cursor(
    cursor: ReputationCommittedEventIdentityV1,
) -> ProofOutcomeFinalizedEventCursorV1 {
    ProofOutcomeFinalizedEventCursorV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn journal_cursor(
    cursor: ReputationCommittedEventIdentityV1,
) -> ReputationJournalFinalizedEventCursorV1 {
    ReputationJournalFinalizedEventCursorV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn repair_cursor(cursor: ReputationCommittedEventIdentityV1) -> RepairFinalizedEventCursorV1 {
    RepairFinalizedEventCursorV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn orderbook_cursor(
    cursor: ReputationCommittedEventIdentityV1,
) -> OrderbookFinalizedEventCursorV1 {
    OrderbookFinalizedEventCursorV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn reserve_cursor(
    cursor: ReputationCommittedEventIdentityV1,
) -> ReserveFinalizedEventCursorV1 {
    ReserveFinalizedEventCursorV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn committed_from_proof_cursor(
    cursor: ProofOutcomeFinalizedEventCursorV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn committed_from_journal_cursor(
    cursor: ReputationJournalFinalizedEventCursorV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn committed_from_repair_cursor(
    cursor: RepairFinalizedEventCursorV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn committed_from_orderbook_cursor(
    cursor: OrderbookFinalizedEventCursorV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

const fn committed_from_reserve_cursor(
    cursor: ReserveFinalizedEventCursorV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: cursor.sequence,
        block_height: cursor.block_height,
        block_hash: cursor.block_hash,
        event_index: cursor.event_index,
    }
}

/// Strict durable-outbox policy for native reputation-journal producers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationJournalProducerPolicyV1 {
    chain_id: ChainId,
    authority_policy: ReputationJournalAuthorityPolicyV1,
    authority_policy_digest: [u8; 32],
    max_pending: u32,
    max_completed: u32,
    max_dead_letters: u32,
    max_attempts: u32,
    checkpoint_max_bytes: u64,
}

impl ReputationJournalProducerPolicyV1 {
    /// Construct the bounded first-release producer policy.
    ///
    /// # Errors
    ///
    /// Rejects an inert chain, invalid recorder policy, or unsafe bound.
    pub fn strict_v1(
        chain_id: ChainId,
        authority_policy: ReputationJournalAuthorityPolicyV1,
    ) -> Result<Self, ReputationRuntimeError> {
        let authority_policy_digest = authority_policy
            .canonical_digest()
            .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
        let policy = Self {
            chain_id,
            authority_policy,
            authority_policy_digest,
            max_pending: REPUTATION_JOURNAL_PRODUCER_MAX_PENDING_V1,
            max_completed: REPUTATION_JOURNAL_PRODUCER_MAX_COMPLETED_V1,
            max_dead_letters: REPUTATION_JOURNAL_PRODUCER_MAX_DEAD_LETTERS_V1,
            max_attempts: REPUTATION_JOURNAL_PRODUCER_MAX_ATTEMPTS_V1,
            checkpoint_max_bytes: REPUTATION_JOURNAL_PRODUCER_MAX_CHECKPOINT_BYTES_V1,
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), ReputationRuntimeError> {
        self.authority_policy
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
        if self.chain_id.as_str().is_empty()
            || self.chain_id.as_str().len() > super::REPUTATION_INGEST_MAX_CHAIN_ID_BYTES_V1
            || self.authority_policy_digest == [0; 32]
            || self.authority_policy.canonical_digest().ok() != Some(self.authority_policy_digest)
            || self.max_pending == 0
            || self.max_pending > REPUTATION_JOURNAL_PRODUCER_MAX_PENDING_V1
            || self.max_completed == 0
            || self.max_completed > REPUTATION_JOURNAL_PRODUCER_MAX_COMPLETED_V1
            || self.max_dead_letters == 0
            || self.max_dead_letters > REPUTATION_JOURNAL_PRODUCER_MAX_DEAD_LETTERS_V1
            || self.max_attempts == 0
            || self.max_attempts > REPUTATION_JOURNAL_PRODUCER_MAX_ATTEMPTS_V1
            || self.checkpoint_max_bytes < REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1
            || self.checkpoint_max_bytes > REPUTATION_JOURNAL_PRODUCER_MAX_CHECKPOINT_BYTES_V1
        {
            return Err(ReputationRuntimeError::InvalidRuntimePolicy);
        }
        Ok(())
    }

    fn digest(&self) -> Result<[u8; 32], ReputationRuntimeError> {
        self.validate()?;
        hash_canonical(
            b"sorafs-reputation-journal-producer-policy-v1",
            &ReputationJournalProducerPolicyDigestMaterialV1 {
                chain_id: self.chain_id.clone(),
                max_pending: self.max_pending,
                max_completed: self.max_completed,
                max_dead_letters: self.max_dead_letters,
                max_attempts: self.max_attempts,
                checkpoint_max_bytes: self.checkpoint_max_bytes,
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReputationJournalProducerPolicyDigestMaterialV1 {
    chain_id: ChainId,
    max_pending: u32,
    max_completed: u32,
    max_dead_letters: u32,
    max_attempts: u32,
    checkpoint_max_bytes: u64,
}

/// Durable crash state of one native journal append.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum ReputationJournalDeliveryStateV1 {
    /// No submission may have happened.
    Ready,
    /// Exact semantic material may have reached a submitter; query before retry.
    Ambiguous,
    /// A submitter accepted the exact append; finality remains authoritative.
    Submitted,
}

/// Exact native append exposed by the durable producer outbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReputationJournalAppendInstructionV1 {
    /// Native PoR terminal append.
    Por(AppendSorafsPorReputationJournalEntry),
    /// Native provider-attributable stream-token append.
    StreamToken(AppendSorafsStreamTokenReputationJournalEntry),
}

/// Durable journal submission projection returned to an external transaction worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationJournalSubmissionV1 {
    /// Monotonic local outbox sequence.
    pub sequence: u64,
    /// Exact active chain.
    pub chain_id: ChainId,
    /// Exact governed transaction authority.
    pub authority: AccountId,
    /// Content-derived ledger event identity.
    pub event_id: ReputationJournalEventIdV1,
    /// Native source identity used for conflict rejection.
    pub source_id: ReputationJournalSourceIdV1,
    /// Current crash-safe delivery state.
    pub state: ReputationJournalDeliveryStateV1,
    /// Bounded attempts consumed.
    pub attempts: u32,
    /// Native typed instruction; an external signer builds the transaction.
    pub instruction: ReputationJournalAppendInstructionV1,
}

/// Payload-free journal outbox row safe to inspect before submission begins.
///
/// The native instruction is deliberately absent. Call
/// [`ReputationJournalProducerOutboxV1::begin_submission`] to durably enter
/// [`ReputationJournalDeliveryStateV1::Ambiguous`] before exact transaction
/// material is exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationJournalPendingV1 {
    /// Monotonic local outbox sequence.
    pub sequence: u64,
    /// Content-derived ledger event identity.
    pub event_id: ReputationJournalEventIdV1,
    /// Native source identity used for conflict rejection.
    pub source_id: ReputationJournalSourceIdV1,
    /// Current crash-safe delivery state.
    pub state: ReputationJournalDeliveryStateV1,
    /// Bounded attempts consumed.
    pub attempts: u32,
    /// Exact finalized identity captured before any submission material was exposed.
    pub baseline_finalized: Option<ReputationFinalizedIdentityV1>,
}

/// Payload-free terminal journal outbox row retained for operator recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationJournalDeadLetterV1 {
    /// Monotonic local outbox sequence.
    pub sequence: u64,
    /// Content-derived ledger event identity.
    pub event_id: ReputationJournalEventIdV1,
    /// Native source identity used for conflict rejection.
    pub source_id: ReputationJournalSourceIdV1,
    /// Governed attempt ceiling consumed by this row.
    pub attempts: u32,
    /// Number of distinct payload-free failure receipts retained.
    pub failure_receipts: u32,
}

/// Payload-free durable journal producer status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationJournalProducerStatusV1 {
    /// Rows that can begin a bounded submission attempt.
    pub ready: u32,
    /// Rows requiring committed-state reconciliation before retry.
    pub ambiguous: u32,
    /// Rows accepted by a submitter but not yet observed finalized.
    pub submitted: u32,
    /// Exact committed tombstones retained in the bounded replay suffix.
    pub completed: u32,
    /// Terminal rows awaiting governed operator handling.
    pub dead_letters: u32,
    /// Next monotonic local outbox sequence.
    pub next_sequence: u64,
}

/// Payload-free durable position of the committed-journal scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationJournalScanStatusV1 {
    /// Exclusive committed-event cursor already reconciled.
    pub after: Option<ReputationJournalFinalizedEventCursorV1>,
    /// Latest exact finalized view durably scanned.
    pub finalized: Option<ReputationJournalFinalizedCursorV1>,
    /// Whether the scanner consumed the terminal page at `finalized`.
    pub caught_up: bool,
    /// Canonical digest of the policy currently used for new Ready rows.
    pub active_authority_policy_digest: [u8; 32],
    /// Active recorder-policy revision.
    pub active_authority_policy_revision: u64,
}

/// Result of binding the outbox to an exact finalized authority policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationJournalPolicySyncOutcomeV1 {
    /// The exact active record was already durable.
    ExactReplay,
    /// The configured bootstrap policy was bound to its first ledger record.
    Initialized,
    /// A direct successor was activated and unsigned Ready rows were rebound.
    Rotated {
        /// Number of never-exposed rows rebound to the successor.
        rebound_ready: u32,
    },
}

/// Result of durably enqueuing one canonical native journal append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationJournalEnqueueOutcomeV1 {
    /// A new append was durably inserted.
    Inserted {
        /// Content-derived event identity.
        event_id: ReputationJournalEventIdV1,
    },
    /// The exact source event was already retained.
    ExactReplay {
        /// Content-derived event identity.
        event_id: ReputationJournalEventIdV1,
    },
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CountedStreamTokenProducerOutcomeV1 {
    Enqueued(ReputationJournalEnqueueOutcomeV1),
    NotCounted,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StreamTokenCountedValidationV1 {
    token_body_digest: [u8; 32],
    token_key_version: u32,
    validated_at_unix_ms: u64,
    status: StreamTokenValidationStatusV1,
}

/// Result of a durable journal-delivery transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationJournalDeliveryOutcomeV1 {
    /// The entry remains available for a bounded retry.
    RetryReady {
        /// Attempts already consumed.
        attempts: u32,
    },
    /// The entry reached its bounded terminal dead-letter state.
    DeadLettered {
        /// Attempts consumed.
        attempts: u32,
    },
    /// The exact transition receipt was already retained.
    ExactReplay,
    /// The exact committed event was acknowledged and tombstoned.
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationJournalDeliveryV1 {
    sequence: u64,
    entry_digest: [u8; 32],
    source_material_digest: [u8; 32],
    entry: ReputationJournalEntryV1,
    state: ReputationJournalDeliveryStateV1,
    attempts: u32,
    baseline_finalized: Option<ReputationFinalizedIdentityV1>,
    failure_receipts: Vec<[u8; 32]>,
}

impl StoredReputationJournalDeliveryV1 {
    const fn pending_status(&self) -> ReputationJournalPendingV1 {
        ReputationJournalPendingV1 {
            sequence: self.sequence,
            event_id: self.entry.event_id,
            source_id: self.entry.source_id,
            state: self.state,
            attempts: self.attempts,
            baseline_finalized: self.baseline_finalized,
        }
    }

    fn submission(
        &self,
        chain_id: &ChainId,
    ) -> Result<ReputationJournalSubmissionV1, ReputationRuntimeError> {
        Ok(ReputationJournalSubmissionV1 {
            sequence: self.sequence,
            chain_id: chain_id.clone(),
            authority: self.entry.recorded_by.clone(),
            event_id: self.entry.event_id,
            source_id: self.entry.source_id,
            state: self.state,
            attempts: self.attempts,
            instruction: instruction_for_entry(&self.entry)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationJournalCompletionV1 {
    sequence: u64,
    event_id: ReputationJournalEventIdV1,
    source_id: ReputationJournalSourceIdV1,
    entry_digest: [u8; 32],
    source_material_digest: [u8; 32],
    committed: ReputationCommittedEventIdentityV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationJournalObservationV1 {
    event_id: ReputationJournalEventIdV1,
    source_id: ReputationJournalSourceIdV1,
    entry_digest: [u8; 32],
    source_material_digest: [u8; 32],
    committed: ReputationCommittedEventIdentityV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationJournalDeadLetterV1 {
    sequence: u64,
    event_id: ReputationJournalEventIdV1,
    source_id: ReputationJournalSourceIdV1,
    entry_digest: [u8; 32],
    source_material_digest: [u8; 32],
    attempts: u32,
    failure_receipts: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredStreamTokenGatewayHeadV1 {
    binding: StreamTokenValidationBindingV1,
    admission_digest: [u8; 32],
    event_id: ReputationJournalEventIdV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredStreamTokenGatewayAdmissionV1 {
    binding: StreamTokenValidationBindingV1,
    admission_digest: [u8; 32],
    event_id: ReputationJournalEventIdV1,
    entry: ReputationJournalEntryV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReputationJournalProducerCheckpointV1 {
    version: u8,
    policy_digest: [u8; 32],
    authority_policies: Vec<ReputationJournalAuthorityPolicyV1>,
    authority_policy_records: Vec<ReputationJournalAuthorityPolicyRecordV1>,
    active_authority_policy_record: Option<ReputationJournalAuthorityPolicyRecordV1>,
    observed_journal_after: Option<ReputationJournalFinalizedEventCursorV1>,
    observed_finalized: Option<ReputationJournalFinalizedCursorV1>,
    journal_scan_caught_up: bool,
    last_assigned_sequence: u64,
    next_sequence: u64,
    pending: Vec<StoredReputationJournalDeliveryV1>,
    completed: Vec<StoredReputationJournalCompletionV1>,
    observed: Vec<StoredReputationJournalObservationV1>,
    dead_letters: Vec<StoredReputationJournalDeadLetterV1>,
    stream_token_gateway_heads: Vec<StoredStreamTokenGatewayHeadV1>,
    stream_token_gateway_admissions: Vec<StoredStreamTokenGatewayAdmissionV1>,
}

impl ReputationJournalProducerCheckpointV1 {
    fn empty(
        policy_digest: [u8; 32],
        authority_policy: ReputationJournalAuthorityPolicyV1,
    ) -> Self {
        Self {
            version: REPUTATION_JOURNAL_PRODUCER_CHECKPOINT_VERSION_V1,
            policy_digest,
            authority_policies: vec![authority_policy],
            authority_policy_records: Vec::new(),
            active_authority_policy_record: None,
            observed_journal_after: None,
            observed_finalized: None,
            journal_scan_caught_up: false,
            last_assigned_sequence: 0,
            next_sequence: 1,
            pending: Vec::new(),
            completed: Vec::new(),
            observed: Vec::new(),
            dead_letters: Vec::new(),
            stream_token_gateway_heads: Vec::new(),
            stream_token_gateway_admissions: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct JournalProducerRuntimeState {
    checkpoint: ReputationJournalProducerCheckpointV1,
    fingerprint: Option<[u8; 32]>,
}

/// Durable native PoR journal outbox with a non-deployed token replay foundation.
///
/// No production counted-token callback is exposed. The token admission state
/// remains test-only scaffolding until its qualified gateway, sealed CAS,
/// transactional quota, and efficient ordered-outbox dependencies exist.
#[derive(Debug)]
pub struct ReputationJournalProducerOutboxV1 {
    policy: ReputationJournalProducerPolicyV1,
    policy_digest: [u8; 32],
    store: AtomicCheckpointStore,
    state: Mutex<JournalProducerRuntimeState>,
    durability_poisoned: AtomicBool,
}

impl ReputationJournalProducerOutboxV1 {
    /// Open or initialize the hardened single-writer producer checkpoint.
    ///
    /// # Errors
    ///
    /// Rejects invalid policy, unsafe paths, noncanonical state, policy
    /// substitution, and inconsistent crash states.
    pub fn open(
        root: &Path,
        policy: ReputationJournalProducerPolicyV1,
    ) -> Result<Self, ReputationRuntimeError> {
        policy.validate()?;
        let policy_digest = policy.digest()?;
        let store = AtomicCheckpointStore::new(
            root,
            REPUTATION_JOURNAL_PRODUCER_CHECKPOINT_FILE_NAME_V1,
            REPUTATION_JOURNAL_PRODUCER_LOCK_FILE_NAME_V1,
            policy.checkpoint_max_bytes,
        )?;
        let (bytes, fingerprint) = store.load_bytes()?;
        let checkpoint = match bytes {
            Some(bytes) => decode_journal_checkpoint(&bytes, &policy, policy_digest)?,
            None => ReputationJournalProducerCheckpointV1::empty(
                policy_digest,
                policy.authority_policy.clone(),
            ),
        };
        validate_journal_checkpoint(&checkpoint, &policy, policy_digest)?;
        Ok(Self {
            policy,
            policy_digest,
            store,
            state: Mutex::new(JournalProducerRuntimeState {
                checkpoint,
                fingerprint,
            }),
            durability_poisoned: AtomicBool::new(false),
        })
    }

    /// Open a retained checkpoint and atomically apply an immutable finalized
    /// authority-policy history before returning the outbox.
    ///
    /// Unlike [`Self::open`], this recovery entry point may decode a checkpoint
    /// whose retained active revision predates the supplied producer policy.
    /// It does not trust that difference: the bounded history must begin at
    /// revision one, contain the retained checkpoint lineage byte-for-byte,
    /// advance by direct revision and predecessor-digest links, and end at the
    /// exact policy configured for this process. Every missing successor is
    /// applied in one durable checkpoint commit before the caller can expose an
    /// active runtime.
    ///
    /// # Errors
    ///
    /// Rejects malformed or substituted history, a checkpoint that is not an
    /// exact prefix of that history, unsafe paths, or persistence failure.
    pub fn open_with_authority_policy_history(
        root: &Path,
        policy: ReputationJournalProducerPolicyV1,
        authority_policy_history: &[ReputationJournalAuthorityPolicyRecordV1],
        finalized: ReputationJournalFinalizedCursorV1,
    ) -> Result<Self, ReputationRuntimeError> {
        policy.validate()?;
        finalized
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidFinalizedAnchor)?;
        let active = authority_policy_history
            .last()
            .ok_or(ReputationRuntimeError::AuthorityPolicyLineage)?;
        validate_authority_policy_history(
            authority_policy_history,
            active,
            finalized.finalized_at_unix_ms,
        )?;
        if active.policy != policy.authority_policy {
            return Err(ReputationRuntimeError::AuthorityPolicyLineage);
        }
        let policy_digest = policy.digest()?;
        let store = AtomicCheckpointStore::new(
            root,
            REPUTATION_JOURNAL_PRODUCER_CHECKPOINT_FILE_NAME_V1,
            REPUTATION_JOURNAL_PRODUCER_LOCK_FILE_NAME_V1,
            policy.checkpoint_max_bytes,
        )?;
        let (bytes, fingerprint) = store.load_bytes()?;
        let checkpoint = match bytes {
            Some(bytes) => {
                decode_journal_checkpoint_for_policy_recovery(&bytes, &policy, policy_digest)?
            }
            None => ReputationJournalProducerCheckpointV1::empty(
                policy_digest,
                authority_policy_history
                    .first()
                    .ok_or(ReputationRuntimeError::AuthorityPolicyLineage)?
                    .policy
                    .clone(),
            ),
        };
        let outbox = Self {
            policy,
            policy_digest,
            store,
            state: Mutex::new(JournalProducerRuntimeState {
                checkpoint,
                fingerprint,
            }),
            durability_poisoned: AtomicBool::new(false),
        };
        outbox.synchronize_authority_policy_history(authority_policy_history, finalized)?;
        {
            let state = outbox
                .state
                .lock()
                .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
            validate_journal_checkpoint(&state.checkpoint, &outbox.policy, outbox.policy_digest)?;
        }
        Ok(outbox)
    }

    /// Return payload-free pending statuses in stable local sequence order.
    ///
    /// # Errors
    ///
    /// Rejects a zero/oversized request or poisoned durable state.
    pub fn pending(
        &self,
        limit: u32,
    ) -> Result<Vec<ReputationJournalPendingV1>, ReputationRuntimeError> {
        self.pending_matching(limit, |_| true)
    }

    fn pending_matching(
        &self,
        limit: u32,
        include: impl Fn(ReputationJournalDeliveryStateV1) -> bool,
    ) -> Result<Vec<ReputationJournalPendingV1>, ReputationRuntimeError> {
        if limit == 0 || limit > self.policy.max_pending {
            return Err(ReputationRuntimeError::InvalidScanLimit);
        }
        self.ensure_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        Ok(state
            .checkpoint
            .pending
            .iter()
            .filter(|entry| include(entry.state))
            .take(
                usize::try_from(limit)
                    .map_err(|_| ReputationRuntimeError::JournalResourceExhausted)?,
            )
            .map(StoredReputationJournalDeliveryV1::pending_status)
            .collect())
    }

    /// Return payload-free durable producer status.
    ///
    /// # Errors
    ///
    /// Returns a resource, durability, or runtime-lock error.
    pub fn status(&self) -> Result<ReputationJournalProducerStatusV1, ReputationRuntimeError> {
        self.ensure_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let mut ready = 0_u32;
        let mut ambiguous = 0_u32;
        let mut submitted = 0_u32;
        for entry in &state.checkpoint.pending {
            let counter = match entry.state {
                ReputationJournalDeliveryStateV1::Ready => &mut ready,
                ReputationJournalDeliveryStateV1::Ambiguous => &mut ambiguous,
                ReputationJournalDeliveryStateV1::Submitted => &mut submitted,
            };
            *counter = counter
                .checked_add(1)
                .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
        }
        Ok(ReputationJournalProducerStatusV1 {
            ready,
            ambiguous,
            submitted,
            completed: u32::try_from(
                state
                    .checkpoint
                    .completed
                    .len()
                    .checked_add(state.checkpoint.observed.len())
                    .ok_or(ReputationRuntimeError::JournalResourceExhausted)?,
            )
            .map_err(|_| ReputationRuntimeError::JournalResourceExhausted)?,
            dead_letters: u32::try_from(state.checkpoint.dead_letters.len())
                .map_err(|_| ReputationRuntimeError::JournalResourceExhausted)?,
            next_sequence: state.checkpoint.next_sequence,
        })
    }

    /// Return the durable exact-view journal scan and active-policy position.
    ///
    /// # Errors
    ///
    /// Returns a durability, checkpoint, or runtime-lock error.
    pub fn scan_status(&self) -> Result<ReputationJournalScanStatusV1, ReputationRuntimeError> {
        self.ensure_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let active = state
            .checkpoint
            .authority_policies
            .last()
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        Ok(ReputationJournalScanStatusV1 {
            after: state.checkpoint.observed_journal_after,
            finalized: state.checkpoint.observed_finalized,
            caught_up: state.checkpoint.journal_scan_caught_up,
            active_authority_policy_digest: active
                .canonical_digest()
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?,
            active_authority_policy_revision: active.revision,
        })
    }

    /// Bind new Ready material to the exact chain-authoritative recorder policy.
    ///
    /// Only a byte-identical record or the direct predecessor-bound successor
    /// is accepted. Ambiguous and submitted rows are immutable because exact
    /// signed bytes may already have escaped; they continue to reconcile under
    /// their captured authority and policy digest. Rotation fails closed if
    /// such bytes fall in the successor's source-time interval. Only
    /// never-exposed Ready rows are reconstructed under the successor.
    ///
    /// # Errors
    ///
    /// Rejects invalid records, skipped/rolled-back lineage, activation after
    /// the supplied finality point, collisions, or persistence failure.
    pub fn synchronize_authority_policy(
        &self,
        record: ReputationJournalAuthorityPolicyRecordV1,
        finalized: ReputationJournalFinalizedCursorV1,
    ) -> Result<ReputationJournalPolicySyncOutcomeV1, ReputationRuntimeError> {
        finalized
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidFinalizedAnchor)?;
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let mut candidate = state.checkpoint.clone();
        let outcome =
            apply_authority_policy_record(&mut candidate, record, finalized.finalized_at_unix_ms)?;
        if outcome != ReputationJournalPolicySyncOutcomeV1::ExactReplay {
            self.commit_journal_candidate(&mut state, candidate)?;
        }
        Ok(outcome)
    }

    /// Atomically synchronize every missed recorder-policy revision from one
    /// immutable finalized view.
    ///
    /// The complete bounded history must start at revision one and end at the
    /// finalized active record. The retained checkpoint lineage must be an
    /// exact contiguous subrange of that history. Missing successors are
    /// applied to one candidate and persisted once, so startup never exposes a
    /// partially recovered policy head.
    ///
    /// Returns the number of direct rotations applied. Initial binding of an
    /// existing policy record is not counted as a rotation.
    ///
    /// # Errors
    ///
    /// Rejects missing, duplicated, skipped, substituted, rolled-back, or
    /// time-invalid history and every persistence failure.
    pub fn synchronize_authority_policy_history(
        &self,
        history: &[ReputationJournalAuthorityPolicyRecordV1],
        finalized: ReputationJournalFinalizedCursorV1,
    ) -> Result<u32, ReputationRuntimeError> {
        finalized
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidFinalizedAnchor)?;
        let terminal = history
            .last()
            .ok_or(ReputationRuntimeError::AuthorityPolicyLineage)?;
        validate_authority_policy_history(history, terminal, finalized.finalized_at_unix_ms)?;
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let retained_policies = &state.checkpoint.authority_policies;
        let retained_records = &state.checkpoint.authority_policy_records;
        let first_retained = retained_policies
            .first()
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        let retained_start = history
            .iter()
            .position(|record| &record.policy == first_retained)
            .ok_or(ReputationRuntimeError::AuthorityPolicyLineage)?;
        if (retained_records.is_empty() && retained_start != 0)
            || retained_start
                .checked_add(retained_policies.len())
                .is_none_or(|end| end > history.len())
            || retained_policies
                .iter()
                .zip(&history[retained_start..])
                .any(|(policy, record)| policy != &record.policy)
            || (!retained_records.is_empty()
                && retained_records
                    .iter()
                    .zip(&history[retained_start..])
                    .any(|(retained, authenticated)| retained != authenticated))
        {
            return Err(ReputationRuntimeError::AuthorityPolicyLineage);
        }
        let mut candidate = state.checkpoint.clone();
        let mut next_history_index = retained_start
            .checked_add(candidate.authority_policies.len())
            .ok_or(ReputationRuntimeError::AuthorityPolicyLineage)?;
        if candidate.active_authority_policy_record.is_none() {
            next_history_index = next_history_index
                .checked_sub(1)
                .ok_or(ReputationRuntimeError::AuthorityPolicyLineage)?;
        }
        let mut rotations = 0_u32;
        let mut changed = false;
        for record in &history[next_history_index..] {
            match apply_authority_policy_record(
                &mut candidate,
                record.clone(),
                finalized.finalized_at_unix_ms,
            )? {
                ReputationJournalPolicySyncOutcomeV1::ExactReplay => {}
                ReputationJournalPolicySyncOutcomeV1::Initialized => changed = true,
                ReputationJournalPolicySyncOutcomeV1::Rotated { .. } => {
                    changed = true;
                    rotations = rotations
                        .checked_add(1)
                        .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
                }
            }
        }
        if candidate.active_authority_policy_record.as_ref() != Some(terminal)
            || candidate.authority_policies.last() != Some(&terminal.policy)
        {
            return Err(ReputationRuntimeError::AuthorityPolicyLineage);
        }
        if changed {
            self.commit_journal_candidate(&mut state, candidate)?;
        }
        Ok(rotations)
    }

    /// Atomically reconcile one typed committed-event page and advance its cursor.
    ///
    /// Exact entries are tombstoned even when still Ready: finalized chain
    /// state is stronger evidence than a local delivery phase. A matching
    /// source with different canonical material fails closed.
    ///
    /// # Errors
    ///
    /// Rejects malformed/replayed pages, rollback/fork, source equivocation,
    /// acknowledgement conflict, or persistence failure.
    pub fn reconcile_finalized_journal_page(
        &self,
        page: ReputationJournalFinalizedEventPageV1,
    ) -> Result<u32, ReputationRuntimeError> {
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        page.validate_after(state.checkpoint.observed_journal_after)
            .map_err(|_| ReputationRuntimeError::InvalidQueryPage)?;
        validate_journal_scan_progress(&state.checkpoint, page.finalized_cursor)?;

        let mut candidate = state.checkpoint.clone();
        let mut acknowledged = 0_u32;
        for event in &page.events {
            let committed = committed_from_journal_cursor(event.cursor());
            let digest = journal_entry_digest(&event.entry)?;
            let source_material_digest = journal_source_material_digest(
                event.entry.provider_id,
                event.entry.source_time_unix_ms,
                event.entry.predecessor_event_id,
                &event.entry.payload,
            )?;
            reconcile_finalized_stream_token_gateway_admission(&mut candidate, &event.entry)?;
            if let Some(existing) = candidate.completed.iter().find(|entry| {
                entry.event_id == event.entry.event_id || entry.source_id == event.entry.source_id
            }) {
                if existing.event_id != event.entry.event_id
                    || existing.source_id != event.entry.source_id
                    || existing.entry_digest != digest
                    || existing.source_material_digest != source_material_digest
                    || existing.committed != committed
                {
                    return Err(ReputationRuntimeError::JournalAcknowledgementConflict);
                }
                continue;
            }
            if let Some(existing) = candidate.observed.iter().find(|entry| {
                entry.event_id == event.entry.event_id || entry.source_id == event.entry.source_id
            }) {
                if existing.event_id != event.entry.event_id
                    || existing.source_id != event.entry.source_id
                    || existing.entry_digest != digest
                    || existing.source_material_digest != source_material_digest
                    || existing.committed != committed
                {
                    return Err(ReputationRuntimeError::JournalAcknowledgementConflict);
                }
                continue;
            }
            if let Some(position) = candidate.dead_letters.iter().position(|delivery| {
                delivery.event_id == event.entry.event_id
                    || delivery.source_id == event.entry.source_id
            }) {
                let delivery = &candidate.dead_letters[position];
                if delivery.event_id != event.entry.event_id
                    || delivery.source_id != event.entry.source_id
                    || delivery.entry_digest != digest
                    || delivery.source_material_digest != source_material_digest
                {
                    return Err(ReputationRuntimeError::JournalSourceConflict);
                }
                let delivery = candidate.dead_letters.remove(position);
                candidate
                    .completed
                    .push(StoredReputationJournalCompletionV1 {
                        sequence: delivery.sequence,
                        event_id: delivery.event_id,
                        source_id: delivery.source_id,
                        entry_digest: delivery.entry_digest,
                        source_material_digest: delivery.source_material_digest,
                        committed,
                    });
                acknowledged = acknowledged
                    .checked_add(1)
                    .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
                continue;
            }
            if let Some(position) = candidate.pending.iter().position(|delivery| {
                delivery.entry.event_id == event.entry.event_id
                    || delivery.entry.source_id == event.entry.source_id
            }) {
                let delivery = &candidate.pending[position];
                if delivery.entry.event_id != event.entry.event_id
                    || delivery.entry.source_id != event.entry.source_id
                    || delivery.entry_digest != digest
                    || delivery.source_material_digest != source_material_digest
                {
                    return Err(ReputationRuntimeError::JournalSourceConflict);
                }
                let delivery = candidate.pending.remove(position);
                candidate
                    .completed
                    .push(StoredReputationJournalCompletionV1 {
                        sequence: delivery.sequence,
                        event_id: delivery.entry.event_id,
                        source_id: delivery.entry.source_id,
                        entry_digest: delivery.entry_digest,
                        source_material_digest: delivery.source_material_digest,
                        committed,
                    });
                acknowledged = acknowledged
                    .checked_add(1)
                    .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
            } else {
                candidate
                    .observed
                    .push(StoredReputationJournalObservationV1 {
                        event_id: event.entry.event_id,
                        source_id: event.entry.source_id,
                        entry_digest: digest,
                        source_material_digest,
                        committed,
                    });
            }
        }
        candidate.completed.sort_by_key(|entry| entry.sequence);
        candidate
            .observed
            .sort_by_key(|entry| entry.committed.sequence);
        compact_journal_tombstones(&mut candidate, self.policy.max_completed)?;
        if let Some(last) = page.events.last() {
            candidate.observed_journal_after = Some(last.cursor());
        }
        candidate.observed_finalized = Some(page.finalized_cursor);
        candidate.journal_scan_caught_up = !page.has_more;
        self.commit_journal_candidate(&mut state, candidate)?;
        Ok(acknowledged)
    }

    /// Return payload-free dead letters in stable local sequence order.
    ///
    /// # Errors
    ///
    /// Rejects a zero/oversized request or poisoned durable state.
    pub fn dead_letters(
        &self,
        limit: u32,
    ) -> Result<Vec<ReputationJournalDeadLetterV1>, ReputationRuntimeError> {
        if limit == 0 || limit > self.policy.max_dead_letters {
            return Err(ReputationRuntimeError::InvalidScanLimit);
        }
        self.ensure_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        state
            .checkpoint
            .dead_letters
            .iter()
            .take(
                usize::try_from(limit)
                    .map_err(|_| ReputationRuntimeError::JournalResourceExhausted)?,
            )
            .map(|entry| {
                Ok(ReputationJournalDeadLetterV1 {
                    sequence: entry.sequence,
                    event_id: entry.event_id,
                    source_id: entry.source_id,
                    attempts: entry.attempts,
                    failure_receipts: u32::try_from(entry.failure_receipts.len())
                        .map_err(|_| ReputationRuntimeError::JournalResourceExhausted)?,
                })
            })
            .collect()
    }

    /// Enter ambiguity before exact append material reaches a transaction worker.
    ///
    /// # Errors
    ///
    /// Rejects an invalid baseline, unknown event, unsafe state transition, or
    /// exhausted retry budget.
    #[cfg(test)]
    pub fn begin_submission(
        &self,
        event_id: ReputationJournalEventIdV1,
        baseline_finalized: ReputationFinalizedIdentityV1,
    ) -> Result<ReputationJournalSubmissionV1, ReputationRuntimeError> {
        baseline_finalized
            .validate()
            .map_err(ReputationRuntimeError::Projector)?;
        self.mutate_pending(event_id, |entry, policy| {
            if entry.state != ReputationJournalDeliveryStateV1::Ready
                || entry.baseline_finalized.is_some()
                || entry.attempts >= policy.max_attempts
            {
                return Err(ReputationRuntimeError::InvalidJournalTransition);
            }
            entry.attempts = entry
                .attempts
                .checked_add(1)
                .ok_or(ReputationRuntimeError::JournalRetryExhausted)?;
            entry.baseline_finalized = Some(baseline_finalized);
            entry.state = ReputationJournalDeliveryStateV1::Ambiguous;
            Ok(())
        })?;
        self.ensure_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let entry = state
            .checkpoint
            .pending
            .iter()
            .find(|entry| entry.entry.event_id == event_id)
            .ok_or(ReputationRuntimeError::UnknownJournalEvent)?;
        entry.submission(&self.policy.chain_id)
    }

    /// Bind one never-exposed Ready row to its source-time-valid policy and
    /// enter ambiguity.
    ///
    /// This is the rotation-safe worker entrypoint. The caller supplies the
    /// digest read from the same exact finalized view used as the absence
    /// baseline, including that view's authoritative block time. A source
    /// observation not strictly earlier than the finalized view is not exposed
    /// for signing; this ensures a later block with the same timestamp cannot
    /// rotate policy and retroactively invalidate Ambiguous bytes. Rows that
    /// have ever entered Ambiguous or Submitted are never rewritten. A row
    /// sourced before the active policy's activation retains its historical
    /// policy bytes; only source material in the active interval is rebound.
    ///
    /// # Errors
    ///
    /// Rejects a stale policy digest, invalid or source-incomplete baseline,
    /// non-Ready row, collision, exhausted retry budget, or persistence
    /// failure.
    pub fn begin_submission_against_active_policy(
        &self,
        event_id: ReputationJournalEventIdV1,
        active_policy_digest: [u8; 32],
        baseline_finalized: ReputationFinalizedIdentityV1,
        baseline_finalized_at_unix_ms: u64,
    ) -> Result<ReputationJournalSubmissionV1, ReputationRuntimeError> {
        baseline_finalized
            .validate()
            .map_err(ReputationRuntimeError::Projector)?;
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let active = state
            .checkpoint
            .authority_policies
            .last()
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        let active_record = state
            .checkpoint
            .active_authority_policy_record
            .as_ref()
            .ok_or(ReputationRuntimeError::AuthorityPolicyLineage)?;
        if baseline_finalized_at_unix_ms == 0
            || active
                .canonical_digest()
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?
                != active_policy_digest
            || active_record.policy_digest != active_policy_digest
            || active_record.activated_at_unix_ms > baseline_finalized_at_unix_ms
        {
            return Err(ReputationRuntimeError::AuthorityPolicyLineage);
        }
        let mut candidate = state.checkpoint.clone();
        let position = candidate
            .pending
            .iter()
            .position(|delivery| delivery.entry.event_id == event_id)
            .ok_or(ReputationRuntimeError::UnknownJournalEvent)?;
        let rebound = {
            let delivery = &mut candidate.pending[position];
            if delivery.state != ReputationJournalDeliveryStateV1::Ready
                || delivery.baseline_finalized.is_some()
                || delivery.attempts >= self.policy.max_attempts
            {
                return Err(ReputationRuntimeError::InvalidJournalTransition);
            }
            if delivery.entry.source_time_unix_ms >= baseline_finalized_at_unix_ms {
                return Err(ReputationRuntimeError::JournalSourceNotFinalized);
            }
            if delivery.entry.authority_policy_digest == active_policy_digest
                || delivery.entry.source_time_unix_ms < active_record.activated_at_unix_ms
            {
                false
            } else {
                let rebound = ReputationJournalEntryV1::try_new(
                    delivery.entry.provider_id,
                    active_policy_digest,
                    active
                        .recorder_authority(delivery.entry.source_kind())
                        .clone(),
                    delivery.entry.source_time_unix_ms,
                    delivery.entry.predecessor_event_id,
                    delivery.entry.payload.clone(),
                )
                .map_err(|_| ReputationRuntimeError::InvalidJournalEntry)?;
                delivery.entry_digest = journal_entry_digest(&rebound)?;
                delivery.entry = rebound;
                true
            }
        };
        if rebound {
            ensure_retained_journal_identities_unique(&candidate)?;
        }
        let delivery = &mut candidate.pending[position];
        delivery.attempts = delivery
            .attempts
            .checked_add(1)
            .ok_or(ReputationRuntimeError::JournalRetryExhausted)?;
        delivery.baseline_finalized = Some(baseline_finalized);
        delivery.state = ReputationJournalDeliveryStateV1::Ambiguous;
        let submission = delivery.submission(&self.policy.chain_id)?;
        self.commit_journal_candidate(&mut state, candidate)?;
        Ok(submission)
    }

    /// Record submitter acceptance while retaining finality as authoritative.
    ///
    /// # Errors
    ///
    /// Rejects an unknown event or non-ambiguous transition.
    pub fn mark_submitted(
        &self,
        event_id: ReputationJournalEventIdV1,
    ) -> Result<(), ReputationRuntimeError> {
        self.mutate_pending(event_id, |entry, _| {
            if entry.state != ReputationJournalDeliveryStateV1::Ambiguous
                || entry.baseline_finalized.is_none()
            {
                return Err(ReputationRuntimeError::InvalidJournalTransition);
            }
            entry.state = ReputationJournalDeliveryStateV1::Submitted;
            Ok(())
        })
    }

    /// Record a payload-free failure proven to precede queue submission.
    ///
    /// Replaying the same receipt is idempotent and never consumes another
    /// attempt.
    ///
    /// # Errors
    ///
    /// Rejects an inert receipt, unknown event, unsafe transition, or durable
    /// persistence failure.
    pub fn record_not_submitted(
        &self,
        event_id: ReputationJournalEventIdV1,
        failure_receipt: [u8; 32],
    ) -> Result<ReputationJournalDeliveryOutcomeV1, ReputationRuntimeError> {
        if failure_receipt == [0; 32] {
            return Err(ReputationRuntimeError::InvalidExternalReceipt);
        }
        self.transition_failed_attempt(event_id, failure_receipt, None)
    }

    /// Retry only after a later finalized view proves the event absent.
    ///
    /// # Errors
    ///
    /// Rejects a stale/forked observation, ready entry, exhausted retry budget,
    /// or durable persistence failure.
    pub fn mark_finalized_absent(
        &self,
        event_id: ReputationJournalEventIdV1,
        observed_finalized: ReputationFinalizedIdentityV1,
        absence_receipt: [u8; 32],
    ) -> Result<ReputationJournalDeliveryOutcomeV1, ReputationRuntimeError> {
        observed_finalized
            .validate()
            .map_err(ReputationRuntimeError::Projector)?;
        if absence_receipt == [0; 32] {
            return Err(ReputationRuntimeError::InvalidExternalReceipt);
        }
        self.transition_failed_attempt(event_id, absence_receipt, Some(observed_finalized))
    }

    /// Durably tombstone an exact finalized journal event.
    ///
    /// # Errors
    ///
    /// Rejects an invalid committed cursor, unknown/ready append, conflicting
    /// acknowledgement, completed-capacity exhaustion, or persistence failure.
    pub fn acknowledge_committed(
        &self,
        event_id: ReputationJournalEventIdV1,
        committed: ReputationCommittedEventIdentityV1,
    ) -> Result<ReputationJournalDeliveryOutcomeV1, ReputationRuntimeError> {
        committed
            .validate()
            .map_err(ReputationRuntimeError::Projector)?;
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        if let Some(existing) = state
            .checkpoint
            .completed
            .iter()
            .find(|entry| entry.event_id == event_id)
        {
            return if existing.committed == committed {
                Ok(ReputationJournalDeliveryOutcomeV1::ExactReplay)
            } else {
                Err(ReputationRuntimeError::JournalAcknowledgementConflict)
            };
        }
        if let Some(existing) = state
            .checkpoint
            .observed
            .iter()
            .find(|entry| entry.event_id == event_id)
        {
            return if existing.committed == committed {
                Ok(ReputationJournalDeliveryOutcomeV1::ExactReplay)
            } else {
                Err(ReputationRuntimeError::JournalAcknowledgementConflict)
            };
        }
        let position = state
            .checkpoint
            .pending
            .iter()
            .position(|entry| entry.entry.event_id == event_id)
            .ok_or(ReputationRuntimeError::UnknownJournalEvent)?;
        let entry = &state.checkpoint.pending[position];
        if !matches!(
            entry.state,
            ReputationJournalDeliveryStateV1::Ambiguous
                | ReputationJournalDeliveryStateV1::Submitted
        ) || entry.baseline_finalized.is_none()
        {
            return Err(ReputationRuntimeError::InvalidJournalTransition);
        }
        let mut candidate = state.checkpoint.clone();
        let entry = candidate.pending.remove(position);
        candidate
            .completed
            .push(StoredReputationJournalCompletionV1 {
                sequence: entry.sequence,
                event_id: entry.entry.event_id,
                source_id: entry.entry.source_id,
                entry_digest: entry.entry_digest,
                source_material_digest: entry.source_material_digest,
                committed,
            });
        candidate.completed.sort_by_key(|entry| entry.sequence);
        compact_journal_tombstones(&mut candidate, self.policy.max_completed)?;
        self.commit_journal_candidate(&mut state, candidate)?;
        Ok(ReputationJournalDeliveryOutcomeV1::Committed)
    }

    fn enqueue_payload(
        &self,
        provider_id: ProviderId,
        source_time_unix_ms: u64,
        payload: ReputationJournalPayloadV1,
    ) -> Result<ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError> {
        if !matches!(
            payload.source_kind(),
            ReputationJournalSourceKindV1::Por | ReputationJournalSourceKindV1::StreamToken
        ) {
            return Err(ReputationRuntimeError::InvalidJournalEntry);
        }
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        self.enqueue_payload_locked(&mut state, provider_id, source_time_unix_ms, payload)
    }

    fn enqueue_payload_locked(
        &self,
        state: &mut JournalProducerRuntimeState,
        provider_id: ProviderId,
        source_time_unix_ms: u64,
        payload: ReputationJournalPayloadV1,
    ) -> Result<ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError> {
        let source_id = payload.source_id();
        let source_material_digest =
            journal_source_material_digest(provider_id, source_time_unix_ms, None, &payload)?;
        if let Some(retained) = retained_journal_identities(&state.checkpoint)
            .find(|retained| retained.source_id == source_id)
        {
            return if retained.source_material_digest == source_material_digest {
                Ok(ReputationJournalEnqueueOutcomeV1::ExactReplay {
                    event_id: retained.event_id,
                })
            } else {
                Err(ReputationRuntimeError::JournalSourceConflict)
            };
        }
        let source_policy = journal_authority_policy_at(&state.checkpoint, source_time_unix_ms)?;
        let source_policy_digest = source_policy
            .canonical_digest()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        let entry = ReputationJournalEntryV1::try_new(
            provider_id,
            source_policy_digest,
            source_policy
                .recorder_authority(payload.source_kind())
                .clone(),
            source_time_unix_ms,
            None,
            payload,
        )
        .map_err(|_| ReputationRuntimeError::InvalidJournalEntry)?;
        let entry_digest = journal_entry_digest(&entry)?;
        if let Some(event_id) = inspect_stream_token_gateway_admission(&state.checkpoint, &entry)? {
            return Ok(ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id });
        }
        for retained in retained_journal_identities(&state.checkpoint) {
            if retained.event_id == entry.event_id {
                return Err(ReputationRuntimeError::JournalSourceConflict);
            }
        }
        if state.checkpoint.pending.len()
            >= usize::try_from(self.policy.max_pending)
                .map_err(|_| ReputationRuntimeError::JournalResourceExhausted)?
        {
            return Err(ReputationRuntimeError::JournalResourceExhausted);
        }
        let sequence = state.checkpoint.next_sequence;
        let mut candidate = state.checkpoint.clone();
        candidate.last_assigned_sequence = sequence;
        candidate.next_sequence = sequence
            .checked_add(1)
            .ok_or(ReputationRuntimeError::JournalSequenceExhausted)?;
        let event_id = entry.event_id;
        retain_stream_token_gateway_admission(&mut candidate, &entry)?;
        candidate.pending.push(StoredReputationJournalDeliveryV1 {
            sequence,
            entry_digest,
            source_material_digest,
            entry,
            state: ReputationJournalDeliveryStateV1::Ready,
            attempts: 0,
            baseline_finalized: None,
            failure_receipts: Vec::new(),
        });
        candidate.pending.sort_by_key(|entry| entry.sequence);
        self.commit_journal_candidate(state, candidate)?;
        Ok(ReputationJournalEnqueueOutcomeV1::Inserted { event_id })
    }

    #[cfg(test)]
    fn enqueue_counted_stream_token_validation(
        &self,
        gateway_id: [u8; 32],
        context: &StreamTokenValidationRequestContextV1,
        validation: StreamTokenCountedValidationV1,
    ) -> Result<ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError> {
        // TODO: this is a non-deployed local sequencing foundation. Before any
        // Torii capture is wired, bind a qualified policy-pinned gateway
        // adapter, fence the checkpoint with deployment-owned sealed CAS,
        // replace full-checkpoint synchronous rewrites with an efficient
        // ordered outbox, and make quota admission transactional and durable
        // with the journal row.
        // TODO: define chain-authoritative cross-gateway/global request-context
        // deduplication before any dual-gateway deployment capture is enabled.
        if !validation.status.counts_for_provider() {
            return Err(ReputationRuntimeError::InvalidJournalEntry);
        }
        let request_context_digest = context
            .digest()
            .map_err(|_| ReputationRuntimeError::InvalidJournalEntry)?;
        let provider_id = context.provider_id();
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        if let Some(retained) = state
            .checkpoint
            .stream_token_gateway_admissions
            .iter()
            .find(|retained| {
                retained.binding.gateway_id == gateway_id
                    && retained.binding.request_context_digest == request_context_digest
            })
        {
            let replay = StreamTokenValidationOutcomeV1 {
                binding: retained.binding,
                token_body_digest: Some(validation.token_body_digest),
                token_key_version: Some(validation.token_key_version),
                validated_at_unix_ms: validation.validated_at_unix_ms,
                status: validation.status,
            };
            return if retained.admission_digest
                == stream_token_admission_digest(provider_id, &replay)?
            {
                Ok(ReputationJournalEnqueueOutcomeV1::ExactReplay {
                    event_id: retained.event_id,
                })
            } else {
                Err(ReputationRuntimeError::JournalSourceConflict)
            };
        }
        // TODO: the replay suffix is deliberately bounded by the dedicated
        // counted-token admission cap.
        // Deployment-owned archival replay lookup is required before claiming
        // indefinite idempotency after this local suffix compacts.
        let gateway_sequence = match state
            .checkpoint
            .stream_token_gateway_heads
            .iter()
            .find(|head| head.binding.gateway_id == gateway_id)
        {
            Some(head) => head
                .binding
                .gateway_sequence
                .checked_add(1)
                .ok_or(ReputationRuntimeError::JournalSequenceExhausted)?,
            None => 1,
        };
        let binding = StreamTokenValidationBindingV1::try_new(
            gateway_id,
            gateway_sequence,
            request_context_digest,
        )
        .map_err(|_| ReputationRuntimeError::InvalidJournalEntry)?;
        let outcome = StreamTokenValidationOutcomeV1 {
            binding,
            token_body_digest: Some(validation.token_body_digest),
            token_key_version: Some(validation.token_key_version),
            validated_at_unix_ms: validation.validated_at_unix_ms,
            status: validation.status,
        };
        self.enqueue_payload_locked(
            &mut state,
            provider_id,
            validation.validated_at_unix_ms,
            ReputationJournalPayloadV1::StreamTokenValidation(outcome),
        )
    }

    fn mutate_pending(
        &self,
        event_id: ReputationJournalEventIdV1,
        mutate: impl FnOnce(
            &mut StoredReputationJournalDeliveryV1,
            &ReputationJournalProducerPolicyV1,
        ) -> Result<(), ReputationRuntimeError>,
    ) -> Result<(), ReputationRuntimeError> {
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let mut candidate = state.checkpoint.clone();
        let entry = candidate
            .pending
            .iter_mut()
            .find(|entry| entry.entry.event_id == event_id)
            .ok_or(ReputationRuntimeError::UnknownJournalEvent)?;
        mutate(entry, &self.policy)?;
        self.commit_journal_candidate(&mut state, candidate)
    }

    fn transition_failed_attempt(
        &self,
        event_id: ReputationJournalEventIdV1,
        failure_receipt: [u8; 32],
        observed_finalized: Option<ReputationFinalizedIdentityV1>,
    ) -> Result<ReputationJournalDeliveryOutcomeV1, ReputationRuntimeError> {
        self.ensure_durable()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        if let Some(observed) = observed_finalized {
            let scanned = state
                .checkpoint
                .observed_finalized
                .ok_or(ReputationRuntimeError::FinalizedAbsenceNotProven)?;
            if !state.checkpoint.journal_scan_caught_up
                || scanned.height != observed.height
                || scanned.block_hash != observed.block_hash
            {
                return Err(ReputationRuntimeError::FinalizedAbsenceNotProven);
            }
        }
        if let Some(dead_letter) = state
            .checkpoint
            .dead_letters
            .iter()
            .find(|entry| entry.event_id == event_id)
        {
            return if dead_letter.failure_receipts.contains(&failure_receipt) {
                Ok(ReputationJournalDeliveryOutcomeV1::ExactReplay)
            } else {
                Err(ReputationRuntimeError::JournalRetryExhausted)
            };
        }
        let position = state
            .checkpoint
            .pending
            .iter()
            .position(|entry| entry.entry.event_id == event_id)
            .ok_or(ReputationRuntimeError::UnknownJournalEvent)?;
        let entry = &state.checkpoint.pending[position];
        if entry.failure_receipts.contains(&failure_receipt) {
            return Ok(ReputationJournalDeliveryOutcomeV1::ExactReplay);
        }
        if entry.state != ReputationJournalDeliveryStateV1::Ambiguous {
            if observed_finalized.is_none()
                || entry.state != ReputationJournalDeliveryStateV1::Submitted
            {
                return Err(ReputationRuntimeError::InvalidJournalTransition);
            }
        }
        let baseline = entry
            .baseline_finalized
            .ok_or(ReputationRuntimeError::InvalidJournalTransition)?;
        if let Some(observed) = observed_finalized {
            if observed.height <= baseline.height {
                return Err(ReputationRuntimeError::FinalizedAbsenceNotProven);
            }
        }

        let mut candidate = state.checkpoint.clone();
        let entry = &mut candidate.pending[position];
        entry.failure_receipts.push(failure_receipt);
        entry.baseline_finalized = None;
        if entry.attempts >= self.policy.max_attempts {
            if candidate.dead_letters.len()
                >= usize::try_from(self.policy.max_dead_letters)
                    .map_err(|_| ReputationRuntimeError::JournalResourceExhausted)?
            {
                return Err(ReputationRuntimeError::JournalResourceExhausted);
            }
            let entry = candidate.pending.remove(position);
            let attempts = entry.attempts;
            candidate
                .dead_letters
                .push(StoredReputationJournalDeadLetterV1 {
                    sequence: entry.sequence,
                    event_id: entry.entry.event_id,
                    source_id: entry.entry.source_id,
                    entry_digest: entry.entry_digest,
                    source_material_digest: entry.source_material_digest,
                    attempts,
                    failure_receipts: entry.failure_receipts,
                });
            candidate.dead_letters.sort_by_key(|entry| entry.sequence);
            self.commit_journal_candidate(&mut state, candidate)?;
            Ok(ReputationJournalDeliveryOutcomeV1::DeadLettered { attempts })
        } else {
            entry.state = ReputationJournalDeliveryStateV1::Ready;
            let attempts = entry.attempts;
            self.commit_journal_candidate(&mut state, candidate)?;
            Ok(ReputationJournalDeliveryOutcomeV1::RetryReady { attempts })
        }
    }

    fn commit_journal_candidate(
        &self,
        state: &mut JournalProducerRuntimeState,
        candidate: ReputationJournalProducerCheckpointV1,
    ) -> Result<(), ReputationRuntimeError> {
        let (candidate, encoded) = encode_bounded_journal_checkpoint(
            candidate,
            &self.policy,
            self.policy_digest,
            self.policy.checkpoint_max_bytes,
        )?;
        match self.store.commit_bytes(&encoded, state.fingerprint) {
            Ok(fingerprint) => {
                state.checkpoint = candidate;
                state.fingerprint = Some(fingerprint);
                Ok(())
            }
            Err(CheckpointStoreError::DurabilityUncertain) => {
                self.durability_poisoned.store(true, Ordering::Release);
                Err(ReputationRuntimeError::CheckpointDurabilityUncertain)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn ensure_durable(&self) -> Result<(), ReputationRuntimeError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationRuntimeError::CheckpointDurabilityUncertain);
        }
        Ok(())
    }
}

/// PoR terminal adapter that can only create the governed native append.
#[derive(Debug, Clone)]
pub struct PorReputationJournalProducerV1 {
    outbox: Arc<ReputationJournalProducerOutboxV1>,
}

impl PorReputationJournalProducerV1 {
    /// Bind the adapter to the durable journal outbox.
    #[must_use]
    pub fn new(outbox: Arc<ReputationJournalProducerOutboxV1>) -> Self {
        Self { outbox }
    }

    /// Validate and durably enqueue one terminal PoR projection.
    ///
    /// The recorder authority, policy digest, and decision timestamp come only
    /// from the bound production policy and typed terminal.
    ///
    /// # Errors
    ///
    /// Rejects invalid terminal material, source conflicts, or persistence
    /// failures.
    pub fn enqueue_terminal(
        &self,
        provider_id: ProviderId,
        outcome: PorTerminalOutcomeV1,
    ) -> Result<ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError> {
        let source_time_unix_ms = outcome.decided_at_unix_ms;
        self.outbox.enqueue_payload(
            provider_id,
            source_time_unix_ms,
            ReputationJournalPayloadV1::PorTerminal(outcome),
        )
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct CountedStreamTokenReputationJournalProducerV1 {
    outbox: Arc<ReputationJournalProducerOutboxV1>,
}

#[cfg(test)]
impl CountedStreamTokenReputationJournalProducerV1 {
    fn new(outbox: Arc<ReputationJournalProducerOutboxV1>) -> Self {
        Self { outbox }
    }

    fn enqueue_counted(
        &self,
        provider_id: ProviderId,
        outcome: StreamTokenValidationOutcomeV1,
    ) -> Result<CountedStreamTokenProducerOutcomeV1, ReputationRuntimeError> {
        let counts_for_provider = outcome.status.counts_for_provider();
        if !counts_for_provider {
            return Ok(CountedStreamTokenProducerOutcomeV1::NotCounted);
        }
        let source_time_unix_ms = outcome.validated_at_unix_ms;
        self.outbox
            .enqueue_payload(
                provider_id,
                source_time_unix_ms,
                ReputationJournalPayloadV1::StreamTokenValidation(outcome),
            )
            .map(CountedStreamTokenProducerOutcomeV1::Enqueued)
    }

    fn enqueue_validation(
        &self,
        gateway_id: [u8; 32],
        context: &StreamTokenValidationRequestContextV1,
        validation: StreamTokenCountedValidationV1,
    ) -> Result<ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError> {
        self.outbox
            .enqueue_counted_stream_token_validation(gateway_id, context, validation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RetainedJournalIdentity {
    event_id: ReputationJournalEventIdV1,
    source_id: ReputationJournalSourceIdV1,
    entry_digest: [u8; 32],
    source_material_digest: [u8; 32],
}

fn retained_journal_identities(
    checkpoint: &ReputationJournalProducerCheckpointV1,
) -> impl Iterator<Item = RetainedJournalIdentity> + '_ {
    checkpoint
        .pending
        .iter()
        .map(|entry| RetainedJournalIdentity {
            event_id: entry.entry.event_id,
            source_id: entry.entry.source_id,
            entry_digest: entry.entry_digest,
            source_material_digest: entry.source_material_digest,
        })
        .chain(
            checkpoint
                .completed
                .iter()
                .map(|entry| RetainedJournalIdentity {
                    event_id: entry.event_id,
                    source_id: entry.source_id,
                    entry_digest: entry.entry_digest,
                    source_material_digest: entry.source_material_digest,
                }),
        )
        .chain(
            checkpoint
                .observed
                .iter()
                .map(|entry| RetainedJournalIdentity {
                    event_id: entry.event_id,
                    source_id: entry.source_id,
                    entry_digest: entry.entry_digest,
                    source_material_digest: entry.source_material_digest,
                }),
        )
        .chain(
            checkpoint
                .dead_letters
                .iter()
                .map(|entry| RetainedJournalIdentity {
                    event_id: entry.event_id,
                    source_id: entry.source_id,
                    entry_digest: entry.entry_digest,
                    source_material_digest: entry.source_material_digest,
                }),
        )
}

fn ensure_retained_journal_identities_unique(
    checkpoint: &ReputationJournalProducerCheckpointV1,
) -> Result<(), ReputationRuntimeError> {
    let mut event_ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    for retained in retained_journal_identities(checkpoint) {
        if retained.event_id == ReputationJournalEventIdV1::ZERO
            || retained.source_id == ReputationJournalSourceIdV1::ZERO
            || retained.entry_digest == [0; 32]
            || retained.source_material_digest == [0; 32]
            || !event_ids.insert(retained.event_id)
            || !source_ids.insert(retained.source_id)
        {
            return Err(ReputationRuntimeError::JournalSourceConflict);
        }
    }
    Ok(())
}

fn compact_journal_tombstones(
    checkpoint: &mut ReputationJournalProducerCheckpointV1,
    maximum: u32,
) -> Result<(), ReputationRuntimeError> {
    let tombstone_limit =
        usize::try_from(maximum).map_err(|_| ReputationRuntimeError::JournalResourceExhausted)?;
    while checkpoint
        .completed
        .len()
        .checked_add(checkpoint.observed.len())
        .ok_or(ReputationRuntimeError::JournalResourceExhausted)?
        > tombstone_limit
    {
        if !evict_oldest_journal_tombstone(checkpoint) {
            return Err(ReputationRuntimeError::JournalResourceExhausted);
        }
    }
    Ok(())
}

fn evict_oldest_journal_tombstone(checkpoint: &mut ReputationJournalProducerCheckpointV1) -> bool {
    let completed_oldest = checkpoint
        .completed
        .iter()
        .enumerate()
        .min_by_key(|(_, entry)| entry.committed.sequence)
        .map(|(position, entry)| (position, entry.committed.sequence));
    let observed_oldest = checkpoint
        .observed
        .first()
        .map(|entry| entry.committed.sequence);
    match (completed_oldest, observed_oldest) {
        (Some((position, completed)), Some(observed)) if completed <= observed => {
            checkpoint.completed.remove(position);
        }
        (Some(_), Some(_)) | (None, Some(_)) => {
            checkpoint.observed.remove(0);
        }
        (Some((position, _)), None) => {
            checkpoint.completed.remove(position);
        }
        (None, None) => return false,
    }
    true
}

fn stream_token_admission_eviction_plan(
    checkpoint: &ReputationJournalProducerCheckpointV1,
) -> Vec<ReputationJournalEventIdV1> {
    let pinned_bindings = checkpoint
        .stream_token_gateway_heads
        .iter()
        .map(|head| head.binding)
        .collect::<BTreeSet<_>>();
    let mut candidates = checkpoint
        .stream_token_gateway_admissions
        .iter()
        .filter(|admission| !pinned_bindings.contains(&admission.binding))
        .map(|admission| {
            (
                (
                    admission.binding.gateway_sequence,
                    admission.binding.gateway_id,
                    admission.event_id,
                ),
                admission.event_id,
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(age, _)| *age);
    candidates
        .into_iter()
        .map(|(_, event_id)| event_id)
        .collect()
}

struct JournalCheckpointEvictionProbe {
    checkpoint: ReputationJournalProducerCheckpointV1,
    removed: Vec<StoredStreamTokenGatewayAdmissionV1>,
    prefix: usize,
}

impl JournalCheckpointEvictionProbe {
    fn new(checkpoint: &ReputationJournalProducerCheckpointV1, plan_len: usize) -> Self {
        Self {
            checkpoint: checkpoint.clone(),
            removed: Vec::with_capacity(plan_len),
            prefix: 0,
        }
    }

    fn move_to_prefix(
        &mut self,
        plan: &[ReputationJournalEventIdV1],
        target: usize,
    ) -> Result<(), ReputationRuntimeError> {
        if target > plan.len() {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        while self.prefix < target {
            let event_id = plan[self.prefix];
            let position = self
                .checkpoint
                .stream_token_gateway_admissions
                .iter()
                .position(|admission| admission.event_id == event_id)
                .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
            self.removed.push(
                self.checkpoint
                    .stream_token_gateway_admissions
                    .swap_remove(position),
            );
            self.prefix = self
                .prefix
                .checked_add(1)
                .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        }
        while self.prefix > target {
            let admission = self
                .removed
                .pop()
                .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
            self.checkpoint
                .stream_token_gateway_admissions
                .push(admission);
            self.prefix -= 1;
        }
        self.checkpoint
            .stream_token_gateway_admissions
            .sort_by_key(|admission| {
                (
                    admission.binding.gateway_id,
                    admission.binding.gateway_sequence,
                )
            });
        Ok(())
    }

    fn encoded_frame_len(&self) -> Result<usize, ReputationRuntimeError> {
        // This avoids materializing the final output Vec for a probe. Norito's
        // field serializers may still use their normal staging allocations.
        norito::core::encoded_frame_len(&self.checkpoint)
            .map_err(|_| ReputationRuntimeError::CanonicalEncoding)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JournalCheckpointEvictionSearch {
    prefix: usize,
    #[cfg(test)]
    probes: usize,
}

fn checkpoint_frame_fits(encoded_len: usize, checkpoint_max_bytes: u64) -> bool {
    u64::try_from(encoded_len).unwrap_or(u64::MAX) <= checkpoint_max_bytes
}

fn smallest_stream_token_admission_eviction_prefix(
    checkpoint: &ReputationJournalProducerCheckpointV1,
    plan: &[ReputationJournalEventIdV1],
    checkpoint_max_bytes: u64,
    original_encoded_len: usize,
) -> Result<JournalCheckpointEvictionSearch, ReputationRuntimeError> {
    if checkpoint_frame_fits(original_encoded_len, checkpoint_max_bytes) {
        return Ok(JournalCheckpointEvictionSearch {
            prefix: 0,
            #[cfg(test)]
            probes: 0,
        });
    }
    if plan.is_empty() {
        return Err(ReputationRuntimeError::CheckpointTooLarge);
    }

    let mut probe = JournalCheckpointEvictionProbe::new(checkpoint, plan.len());
    probe.move_to_prefix(plan, plan.len())?;
    #[cfg(test)]
    let mut probes = 1;
    if !checkpoint_frame_fits(probe.encoded_frame_len()?, checkpoint_max_bytes) {
        return Err(ReputationRuntimeError::CheckpointTooLarge);
    }

    // Prefix zero is already known to be too large and the full plan is known
    // to fit. The encoded size is monotonic because the plan only removes
    // complete admission rows, so a lower-bound search finds the unique
    // smallest fitting prefix.
    let mut lower = 1;
    let mut upper = plan.len();
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        probe.move_to_prefix(plan, middle)?;
        #[cfg(test)]
        probes += 1;
        if checkpoint_frame_fits(probe.encoded_frame_len()?, checkpoint_max_bytes) {
            upper = middle;
        } else {
            lower = middle
                .checked_add(1)
                .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        }
    }
    Ok(JournalCheckpointEvictionSearch {
        prefix: lower,
        #[cfg(test)]
        probes,
    })
}

fn apply_stream_token_admission_eviction_prefix(
    checkpoint: &mut ReputationJournalProducerCheckpointV1,
    plan: &[ReputationJournalEventIdV1],
    prefix: usize,
) -> Result<(), ReputationRuntimeError> {
    let selected = plan
        .get(..prefix)
        .ok_or(ReputationRuntimeError::InvalidCheckpoint)?
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if selected.len() != prefix {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    let original_len = checkpoint.stream_token_gateway_admissions.len();
    checkpoint
        .stream_token_gateway_admissions
        .retain(|admission| !selected.contains(&admission.event_id));
    if original_len.checked_sub(checkpoint.stream_token_gateway_admissions.len()) != Some(prefix) {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    Ok(())
}

fn encode_bounded_journal_checkpoint(
    mut candidate: ReputationJournalProducerCheckpointV1,
    policy: &ReputationJournalProducerPolicyV1,
    policy_digest: [u8; 32],
    checkpoint_max_bytes: u64,
) -> Result<(ReputationJournalProducerCheckpointV1, Vec<u8>), ReputationRuntimeError> {
    validate_journal_checkpoint_structure(&candidate, policy, policy_digest)?;
    let original_encoded_len = norito::core::encoded_frame_len(&candidate)
        .map_err(|_| ReputationRuntimeError::CanonicalEncoding)?;
    let eviction_plan = stream_token_admission_eviction_plan(&candidate);
    let search = smallest_stream_token_admission_eviction_prefix(
        &candidate,
        &eviction_plan,
        checkpoint_max_bytes,
        original_encoded_len,
    )?;
    if search.prefix > 0 {
        apply_stream_token_admission_eviction_prefix(
            &mut candidate,
            &eviction_plan,
            search.prefix,
        )?;
        validate_journal_checkpoint_structure(&candidate, policy, policy_digest)?;
    }

    let encoded_len = if search.prefix == 0 {
        original_encoded_len
    } else {
        norito::core::encoded_frame_len(&candidate)
            .map_err(|_| ReputationRuntimeError::CanonicalEncoding)?
    };
    if !checkpoint_frame_fits(encoded_len, checkpoint_max_bytes) {
        return Err(ReputationRuntimeError::CheckpointTooLarge);
    }
    let encoded =
        norito::to_bytes(&candidate).map_err(|_| ReputationRuntimeError::CanonicalEncoding)?;
    if encoded.len() != encoded_len || !checkpoint_frame_fits(encoded.len(), checkpoint_max_bytes) {
        return Err(ReputationRuntimeError::CanonicalEncoding);
    }
    Ok((candidate, encoded))
}

fn validate_journal_scan_progress(
    checkpoint: &ReputationJournalProducerCheckpointV1,
    observed: ReputationJournalFinalizedCursorV1,
) -> Result<(), ReputationRuntimeError> {
    observed
        .validate()
        .map_err(|_| ReputationRuntimeError::InvalidQueryPage)?;
    if let Some(previous) = checkpoint.observed_finalized {
        if observed.height < previous.height
            || observed.finalized_at_unix_ms < previous.finalized_at_unix_ms
        {
            return Err(ReputationRuntimeError::FinalizedRollback);
        }
        if observed.height == previous.height
            && (observed.block_hash != previous.block_hash
                || observed.finalized_at_unix_ms != previous.finalized_at_unix_ms)
        {
            return Err(ReputationRuntimeError::FinalizedFork);
        }
    }
    Ok(())
}

fn instruction_for_entry(
    entry: &ReputationJournalEntryV1,
) -> Result<ReputationJournalAppendInstructionV1, ReputationRuntimeError> {
    match entry.source_kind() {
        ReputationJournalSourceKindV1::Por => Ok(ReputationJournalAppendInstructionV1::Por(
            AppendSorafsPorReputationJournalEntry::new(entry.clone()),
        )),
        ReputationJournalSourceKindV1::StreamToken => {
            Ok(ReputationJournalAppendInstructionV1::StreamToken(
                AppendSorafsStreamTokenReputationJournalEntry::new(entry.clone()),
            ))
        }
        ReputationJournalSourceKindV1::ProviderDispute => {
            Err(ReputationRuntimeError::InvalidCheckpoint)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize)]
struct ReputationJournalSourceMaterialV1 {
    provider_id: ProviderId,
    source_time_unix_ms: u64,
    predecessor_event_id: Option<ReputationJournalEventIdV1>,
    payload: ReputationJournalPayloadV1,
}

fn journal_source_material_digest(
    provider_id: ProviderId,
    source_time_unix_ms: u64,
    predecessor_event_id: Option<ReputationJournalEventIdV1>,
    payload: &ReputationJournalPayloadV1,
) -> Result<[u8; 32], ReputationRuntimeError> {
    hash_canonical(
        b"sorafs-reputation-journal-source-material-v1",
        &ReputationJournalSourceMaterialV1 {
            provider_id,
            source_time_unix_ms,
            predecessor_event_id,
            payload: payload.clone(),
        },
    )
}

fn journal_entry_digest(
    entry: &ReputationJournalEntryV1,
) -> Result<[u8; 32], ReputationRuntimeError> {
    hash_canonical(b"sorafs-reputation-journal-producer-entry-v1", entry)
}

fn stream_token_admission_digest(
    provider_id: ProviderId,
    outcome: &StreamTokenValidationOutcomeV1,
) -> Result<[u8; 32], ReputationRuntimeError> {
    // `validated_at_unix_ms` is intentionally excluded from allocator replay
    // identity. A retry of the same nonce-bound request may be observed later,
    // but must return the first durable event (whose original timestamp remains
    // authoritative) rather than manufacture another gateway sequence.
    let status_bytes =
        norito::to_bytes(&outcome.status).map_err(|_| ReputationRuntimeError::CanonicalEncoding)?;
    let status_len =
        u64::try_from(status_bytes.len()).map_err(|_| ReputationRuntimeError::CanonicalEncoding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.reputation.stream-token.admission.v1");
    hasher.update(provider_id.as_bytes());
    hasher.update(&outcome.binding.gateway_id);
    hasher.update(&outcome.binding.gateway_sequence.to_le_bytes());
    hasher.update(&outcome.binding.request_context_digest);
    match outcome.token_body_digest {
        Some(digest) => {
            hasher.update(&[1]);
            hasher.update(&digest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    match outcome.token_key_version {
        Some(version) => {
            hasher.update(&[1]);
            hasher.update(&version.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.update(&status_len.to_le_bytes());
    hasher.update(&status_bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn inspect_stream_token_gateway_admission(
    checkpoint: &ReputationJournalProducerCheckpointV1,
    entry: &ReputationJournalEntryV1,
) -> Result<Option<ReputationJournalEventIdV1>, ReputationRuntimeError> {
    let ReputationJournalPayloadV1::StreamTokenValidation(outcome) = &entry.payload else {
        return Ok(None);
    };
    if !outcome.status.counts_for_provider() {
        return Err(ReputationRuntimeError::InvalidJournalEntry);
    }
    if let Some(retained) = checkpoint
        .stream_token_gateway_admissions
        .iter()
        .find(|retained| {
            retained.binding.gateway_id == outcome.binding.gateway_id
                && retained.binding.gateway_sequence == outcome.binding.gateway_sequence
        })
    {
        let admission_digest = stream_token_admission_digest(entry.provider_id, outcome)?;
        return if retained.binding == outcome.binding
            && retained.admission_digest == admission_digest
            && retained.event_id == entry.event_id
            && retained.entry == *entry
        {
            Ok(Some(retained.event_id))
        } else {
            Err(ReputationRuntimeError::JournalSourceConflict)
        };
    }
    let Some(head) = checkpoint
        .stream_token_gateway_heads
        .iter()
        .find(|head| head.binding.gateway_id == outcome.binding.gateway_id)
    else {
        return Ok(None);
    };
    if outcome.binding.gateway_sequence < head.binding.gateway_sequence {
        return Err(ReputationRuntimeError::JournalSourceConflict);
    }
    if outcome.binding.gateway_sequence > head.binding.gateway_sequence {
        return Ok(None);
    }
    let admission_digest = stream_token_admission_digest(entry.provider_id, outcome)?;
    if head.binding != outcome.binding || head.admission_digest != admission_digest {
        return Err(ReputationRuntimeError::JournalSourceConflict);
    }
    if head.event_id != entry.event_id {
        return Err(ReputationRuntimeError::JournalSourceConflict);
    }
    Ok(Some(head.event_id))
}

fn retain_stream_token_gateway_admission(
    checkpoint: &mut ReputationJournalProducerCheckpointV1,
    entry: &ReputationJournalEntryV1,
) -> Result<(), ReputationRuntimeError> {
    let ReputationJournalPayloadV1::StreamTokenValidation(outcome) = &entry.payload else {
        return Ok(());
    };
    if !outcome.status.counts_for_provider() {
        return Err(ReputationRuntimeError::InvalidJournalEntry);
    }
    let admission_digest = stream_token_admission_digest(entry.provider_id, outcome)?;
    let retained = StoredStreamTokenGatewayHeadV1 {
        binding: outcome.binding,
        admission_digest,
        event_id: entry.event_id,
    };
    match checkpoint
        .stream_token_gateway_heads
        .iter_mut()
        .find(|head| head.binding.gateway_id == outcome.binding.gateway_id)
    {
        Some(head) => {
            if outcome.binding.gateway_sequence <= head.binding.gateway_sequence {
                return Err(ReputationRuntimeError::JournalSourceConflict);
            }
            *head = retained;
        }
        None => {
            if checkpoint.stream_token_gateway_heads.len()
                >= REPUTATION_STREAM_TOKEN_GATEWAY_HEADS_MAX_V1
            {
                return Err(ReputationRuntimeError::JournalResourceExhausted);
            }
            checkpoint.stream_token_gateway_heads.push(retained);
        }
    }
    checkpoint
        .stream_token_gateway_heads
        .sort_by_key(|head| head.binding.gateway_id);
    retain_stream_token_gateway_admission_suffix(
        checkpoint,
        StoredStreamTokenGatewayAdmissionV1 {
            binding: outcome.binding,
            admission_digest,
            event_id: entry.event_id,
            entry: entry.clone(),
        },
        REPUTATION_STREAM_TOKEN_GATEWAY_ADMISSIONS_MAX_V1,
    )
}

fn reconcile_finalized_stream_token_gateway_admission(
    checkpoint: &mut ReputationJournalProducerCheckpointV1,
    entry: &ReputationJournalEntryV1,
) -> Result<(), ReputationRuntimeError> {
    let ReputationJournalPayloadV1::StreamTokenValidation(outcome) = &entry.payload else {
        return Ok(());
    };
    if !outcome.status.counts_for_provider() {
        // Chain-authoritative excluded outcomes remain in the generic
        // committed/observed reconciliation state, but they can never create
        // local replay admissions or advance a gateway allocator head.
        return Ok(());
    }
    let admission_digest = stream_token_admission_digest(entry.provider_id, outcome)?;
    let retained = StoredStreamTokenGatewayHeadV1 {
        binding: outcome.binding,
        admission_digest,
        event_id: entry.event_id,
    };
    match checkpoint
        .stream_token_gateway_heads
        .iter_mut()
        .find(|head| head.binding.gateway_id == outcome.binding.gateway_id)
    {
        Some(head) if outcome.binding.gateway_sequence < head.binding.gateway_sequence => {
            // A finalized scan may legitimately observe an older locally
            // pending sequence after a newer sequence was admitted. The typed
            // finalized page plus the event/source checks below authenticate
            // the row, while the durable local high-water mark never moves backwards.
        }
        Some(head) if outcome.binding.gateway_sequence == head.binding.gateway_sequence => {
            if *head != retained {
                return Err(ReputationRuntimeError::JournalSourceConflict);
            }
        }
        Some(head) => *head = retained,
        None => {
            if checkpoint.stream_token_gateway_heads.len()
                >= REPUTATION_STREAM_TOKEN_GATEWAY_HEADS_MAX_V1
            {
                return Err(ReputationRuntimeError::JournalResourceExhausted);
            }
            checkpoint.stream_token_gateway_heads.push(retained);
            checkpoint
                .stream_token_gateway_heads
                .sort_by_key(|head| head.binding.gateway_id);
        }
    }
    retain_stream_token_gateway_admission_suffix(
        checkpoint,
        StoredStreamTokenGatewayAdmissionV1 {
            binding: outcome.binding,
            admission_digest,
            event_id: entry.event_id,
            entry: entry.clone(),
        },
        REPUTATION_STREAM_TOKEN_GATEWAY_ADMISSIONS_MAX_V1,
    )
}

fn retain_stream_token_gateway_admission_suffix(
    checkpoint: &mut ReputationJournalProducerCheckpointV1,
    retained: StoredStreamTokenGatewayAdmissionV1,
    max_retained: usize,
) -> Result<(), ReputationRuntimeError> {
    if let Some(existing) = checkpoint
        .stream_token_gateway_admissions
        .iter()
        .find(|existing| {
            existing.binding.gateway_id == retained.binding.gateway_id
                && existing.binding.gateway_sequence == retained.binding.gateway_sequence
        })
    {
        return if *existing == retained {
            Ok(())
        } else {
            Err(ReputationRuntimeError::JournalSourceConflict)
        };
    }
    checkpoint.stream_token_gateway_admissions.push(retained);
    while checkpoint.stream_token_gateway_admissions.len() > max_retained {
        if !evict_oldest_non_head_stream_token_admission(checkpoint) {
            return Err(ReputationRuntimeError::JournalResourceExhausted);
        }
    }
    checkpoint
        .stream_token_gateway_admissions
        .sort_by_key(|admission| {
            (
                admission.binding.gateway_id,
                admission.binding.gateway_sequence,
            )
        });
    Ok(())
}

fn evict_oldest_non_head_stream_token_admission(
    checkpoint: &mut ReputationJournalProducerCheckpointV1,
) -> bool {
    let Some(position) = checkpoint
        .stream_token_gateway_admissions
        .iter()
        .enumerate()
        .filter_map(|(position, admission)| {
            let head = checkpoint
                .stream_token_gateway_heads
                .iter()
                .find(|head| head.binding.gateway_id == admission.binding.gateway_id)?;
            (head.binding != admission.binding).then_some((
                position,
                (
                    admission.binding.gateway_sequence,
                    admission.binding.gateway_id,
                    admission.event_id,
                ),
            ))
        })
        .min_by_key(|(_, age)| *age)
        .map(|(position, _)| position)
    else {
        return false;
    };
    checkpoint.stream_token_gateway_admissions.remove(position);
    true
}

fn apply_authority_policy_record(
    checkpoint: &mut ReputationJournalProducerCheckpointV1,
    record: ReputationJournalAuthorityPolicyRecordV1,
    finalized_at_unix_ms: u64,
) -> Result<ReputationJournalPolicySyncOutcomeV1, ReputationRuntimeError> {
    record
        .validate()
        .map_err(|_| ReputationRuntimeError::InvalidAuthorityPolicy)?;
    if record.activated_at_unix_ms > finalized_at_unix_ms {
        return Err(ReputationRuntimeError::InvalidAuthorityPolicy);
    }
    let active = checkpoint
        .authority_policies
        .last()
        .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
    let active_digest = active
        .canonical_digest()
        .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
    if record.policy_digest == active_digest {
        if record.policy != *active {
            return Err(ReputationRuntimeError::InvalidAuthorityPolicy);
        }
        return match &checkpoint.active_authority_policy_record {
            Some(existing) if existing == &record => {
                Ok(ReputationJournalPolicySyncOutcomeV1::ExactReplay)
            }
            Some(_) => Err(ReputationRuntimeError::AuthorityPolicyRecordConflict),
            None => {
                if checkpoint.pending.iter().any(|delivery| {
                    delivery.entry.source_time_unix_ms < record.activated_at_unix_ms
                }) {
                    return Err(ReputationRuntimeError::InvalidAuthorityPolicy);
                }
                checkpoint.authority_policy_records.push(record.clone());
                checkpoint.active_authority_policy_record = Some(record);
                Ok(ReputationJournalPolicySyncOutcomeV1::Initialized)
            }
        };
    }
    let expected_revision = active
        .revision
        .checked_add(1)
        .ok_or(ReputationRuntimeError::AuthorityPolicyLineage)?;
    if record.policy.revision != expected_revision
        || record.policy.predecessor_policy_digest != Some(active_digest)
        || checkpoint.authority_policies.len()
            >= REPUTATION_JOURNAL_PRODUCER_MAX_POLICY_REVISIONS_V1
        || checkpoint
            .active_authority_policy_record
            .as_ref()
            .is_none_or(|active_record| {
                active_record.policy_digest != active_digest
                    || active_record.activated_at_unix_ms > record.activated_at_unix_ms
            })
    {
        return Err(ReputationRuntimeError::AuthorityPolicyLineage);
    }

    checkpoint.authority_policies.push(record.policy.clone());
    checkpoint.authority_policy_records.push(record.clone());
    checkpoint.active_authority_policy_record = Some(record.clone());
    let mut rebound_ready = 0_u32;
    let (pending, stream_token_gateway_heads, stream_token_gateway_admissions) = (
        &mut checkpoint.pending,
        &mut checkpoint.stream_token_gateway_heads,
        &mut checkpoint.stream_token_gateway_admissions,
    );
    for delivery in pending {
        if delivery.state != ReputationJournalDeliveryStateV1::Ready
            || delivery.entry.source_time_unix_ms < record.activated_at_unix_ms
        {
            continue;
        }
        let rebound = ReputationJournalEntryV1::try_new(
            delivery.entry.provider_id,
            record.policy_digest,
            record
                .policy
                .recorder_authority(delivery.entry.source_kind())
                .clone(),
            delivery.entry.source_time_unix_ms,
            delivery.entry.predecessor_event_id,
            delivery.entry.payload.clone(),
        )
        .map_err(|_| ReputationRuntimeError::InvalidJournalEntry)?;
        delivery.entry_digest = journal_entry_digest(&rebound)?;
        if let ReputationJournalPayloadV1::StreamTokenValidation(outcome) = &rebound.payload {
            let admission_digest = stream_token_admission_digest(rebound.provider_id, outcome)?;
            let head = stream_token_gateway_heads
                .iter_mut()
                .find(|head| head.binding.gateway_id == outcome.binding.gateway_id)
                .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
            if outcome.binding.gateway_sequence > head.binding.gateway_sequence {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
            if outcome.binding.gateway_sequence == head.binding.gateway_sequence {
                if head.binding != outcome.binding || head.admission_digest != admission_digest {
                    return Err(ReputationRuntimeError::InvalidCheckpoint);
                }
                head.event_id = rebound.event_id;
            }
            if let Some(admission) = stream_token_gateway_admissions
                .iter_mut()
                .find(|admission| admission.binding == outcome.binding)
            {
                if admission.admission_digest != admission_digest
                    || admission.event_id != delivery.entry.event_id
                    || admission.entry != delivery.entry
                {
                    return Err(ReputationRuntimeError::InvalidCheckpoint);
                }
                admission.event_id = rebound.event_id;
                admission.entry = rebound.clone();
            }
        }
        delivery.entry = rebound;
        rebound_ready = rebound_ready
            .checked_add(1)
            .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
    }
    ensure_retained_journal_identities_unique(checkpoint)?;
    Ok(ReputationJournalPolicySyncOutcomeV1::Rotated { rebound_ready })
}

fn decode_journal_checkpoint(
    bytes: &[u8],
    policy: &ReputationJournalProducerPolicyV1,
    policy_digest: [u8; 32],
) -> Result<ReputationJournalProducerCheckpointV1, ReputationRuntimeError> {
    let checkpoint: ReputationJournalProducerCheckpointV1 =
        decode_runtime_checkpoint(bytes, policy.checkpoint_max_bytes)?;
    validate_journal_checkpoint(&checkpoint, policy, policy_digest)?;
    Ok(checkpoint)
}

fn decode_journal_checkpoint_for_policy_recovery(
    bytes: &[u8],
    policy: &ReputationJournalProducerPolicyV1,
    policy_digest: [u8; 32],
) -> Result<ReputationJournalProducerCheckpointV1, ReputationRuntimeError> {
    let checkpoint: ReputationJournalProducerCheckpointV1 =
        decode_runtime_checkpoint(bytes, policy.checkpoint_max_bytes)?;
    validate_journal_checkpoint_structure(&checkpoint, policy, policy_digest)?;
    Ok(checkpoint)
}

fn journal_authority_policy_at(
    checkpoint: &ReputationJournalProducerCheckpointV1,
    source_time_unix_ms: u64,
) -> Result<&ReputationJournalAuthorityPolicyV1, ReputationRuntimeError> {
    if checkpoint.authority_policy_records.is_empty() {
        return Err(ReputationRuntimeError::AuthorityPolicyLineage);
    }
    if checkpoint.authority_policy_records.len() != checkpoint.authority_policies.len() {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    checkpoint
        .authority_policy_records
        .iter()
        .zip(&checkpoint.authority_policies)
        .rev()
        .find_map(|(record, policy)| {
            (source_time_unix_ms >= record.activated_at_unix_ms).then_some((record, policy))
        })
        .ok_or(ReputationRuntimeError::InvalidAuthorityPolicy)
        .and_then(|(record, policy)| {
            if record.policy == *policy {
                Ok(policy)
            } else {
                Err(ReputationRuntimeError::InvalidCheckpoint)
            }
        })
}

fn validate_journal_checkpoint(
    checkpoint: &ReputationJournalProducerCheckpointV1,
    policy: &ReputationJournalProducerPolicyV1,
    policy_digest: [u8; 32],
) -> Result<(), ReputationRuntimeError> {
    validate_journal_checkpoint_structure(checkpoint, policy, policy_digest)?;
    if checkpoint.authority_policies.last() != Some(&policy.authority_policy) {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    Ok(())
}

fn validate_journal_checkpoint_structure(
    checkpoint: &ReputationJournalProducerCheckpointV1,
    policy: &ReputationJournalProducerPolicyV1,
    policy_digest: [u8; 32],
) -> Result<(), ReputationRuntimeError> {
    policy.validate()?;
    if checkpoint.version != REPUTATION_JOURNAL_PRODUCER_CHECKPOINT_VERSION_V1
        || checkpoint.policy_digest != policy_digest
        || checkpoint.authority_policies.is_empty()
        || checkpoint.authority_policies.len() > REPUTATION_JOURNAL_PRODUCER_MAX_POLICY_REVISIONS_V1
        || checkpoint.authority_policy_records.len()
            > REPUTATION_JOURNAL_PRODUCER_MAX_POLICY_REVISIONS_V1
        || checkpoint.next_sequence == 0
        || checkpoint.last_assigned_sequence.checked_add(1) != Some(checkpoint.next_sequence)
        || checkpoint.pending.len()
            > usize::try_from(policy.max_pending)
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?
        || checkpoint
            .completed
            .len()
            .checked_add(checkpoint.observed.len())
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?
            > usize::try_from(policy.max_completed)
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?
        || checkpoint.dead_letters.len()
            > usize::try_from(policy.max_dead_letters)
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?
        || checkpoint.stream_token_gateway_heads.len()
            > REPUTATION_STREAM_TOKEN_GATEWAY_HEADS_MAX_V1
        || checkpoint.stream_token_gateway_admissions.len()
            > REPUTATION_STREAM_TOKEN_GATEWAY_ADMISSIONS_MAX_V1
    {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    // Count ceilings remain independent logical resource bounds. Byte
    // compaction may prune only non-head stream-token admission cache rows.
    // Generic completed/observed identities remain exact-replay authorities
    // unless count compaction applies; irreducible byte pressure fails closed.
    let mut policy_positions_by_digest = BTreeMap::new();
    let mut policy_positions_by_revision = BTreeMap::new();
    let mut previous_policy: Option<(&ReputationJournalAuthorityPolicyV1, [u8; 32])> = None;
    for (position, authority_policy) in checkpoint.authority_policies.iter().enumerate() {
        authority_policy
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        let digest = authority_policy
            .canonical_digest()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if policy_positions_by_digest
            .insert(digest, position)
            .is_some()
            || policy_positions_by_revision
                .insert(authority_policy.revision, position)
                .is_some()
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        match previous_policy {
            None if authority_policy.revision == 1
                && authority_policy.predecessor_policy_digest.is_none() => {}
            Some((previous, previous_digest))
                if previous.revision.checked_add(1) == Some(authority_policy.revision)
                    && authority_policy.predecessor_policy_digest == Some(previous_digest) => {}
            _ => return Err(ReputationRuntimeError::InvalidCheckpoint),
        }
        previous_policy = Some((authority_policy, digest));
    }
    let active_policy = checkpoint
        .authority_policies
        .last()
        .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
    if checkpoint.authority_policy_records.is_empty() {
        if checkpoint.active_authority_policy_record.is_some()
            || checkpoint.authority_policies.len() != 1
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
    } else {
        if checkpoint.authority_policy_records.len() != checkpoint.authority_policies.len()
            || checkpoint.active_authority_policy_record
                != checkpoint.authority_policy_records.last().cloned()
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        let mut previous_activation = 0_u64;
        for (position, (record, authority_policy)) in checkpoint
            .authority_policy_records
            .iter()
            .zip(&checkpoint.authority_policies)
            .enumerate()
        {
            record
                .validate()
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
            if record.policy != *authority_policy
                || record.activated_at_unix_ms < previous_activation
                || policy_positions_by_revision.get(&record.policy.revision) != Some(&position)
                || policy_positions_by_digest.get(&record.policy_digest) != Some(&position)
            {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
            previous_activation = record.activated_at_unix_ms;
        }
        if checkpoint
            .active_authority_policy_record
            .as_ref()
            .is_none_or(|record| record.policy != *active_policy)
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
    }
    if checkpoint
        .observed_journal_after
        .is_some_and(|cursor| cursor.validate().is_err())
        || checkpoint
            .observed_finalized
            .is_some_and(|cursor| cursor.validate().is_err())
        || checkpoint.journal_scan_caught_up && checkpoint.observed_finalized.is_none()
        || matches!(
            (
                checkpoint.observed_journal_after,
                checkpoint.observed_finalized
            ),
            (Some(after), Some(finalized)) if after.block_height > finalized.height
        )
    {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    let mut retained_by_event = BTreeMap::new();
    let mut retained_by_source = BTreeMap::new();
    for retained in retained_journal_identities(checkpoint) {
        if retained.event_id == ReputationJournalEventIdV1::ZERO
            || retained.source_id == ReputationJournalSourceIdV1::ZERO
            || retained.entry_digest == [0; 32]
            || retained.source_material_digest == [0; 32]
            || retained_by_event
                .insert(retained.event_id, retained)
                .is_some()
            || retained_by_source
                .insert(retained.source_id, retained)
                .is_some()
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
    }
    let mut pending_by_event = BTreeMap::new();
    for delivery in &checkpoint.pending {
        if pending_by_event
            .insert(delivery.entry.event_id, delivery)
            .is_some()
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
    }
    let mut sequences = BTreeSet::new();
    let mut committed_sequences = BTreeSet::new();
    let mut max_sequence = 0_u64;
    let mut previous_gateway_id = None;
    let mut gateway_heads_by_id = BTreeMap::new();
    for head in &checkpoint.stream_token_gateway_heads {
        if head.binding.gateway_id == [0; 32]
            || head.binding.gateway_sequence == 0
            || head.binding.request_context_digest == [0; 32]
            || head.binding.validation_id() == [0; 32]
            || head.admission_digest == [0; 32]
            || head.event_id == ReputationJournalEventIdV1::ZERO
            || previous_gateway_id.is_some_and(|previous| previous >= head.binding.gateway_id)
            || gateway_heads_by_id
                .insert(head.binding.gateway_id, head)
                .is_some()
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        previous_gateway_id = Some(head.binding.gateway_id);
    }
    let mut admissions_by_sequence = BTreeMap::new();
    let mut admissions_by_binding = BTreeMap::new();
    let mut admissions_by_event = BTreeMap::new();
    let mut previous_admission_binding = None;
    for admission in &checkpoint.stream_token_gateway_admissions {
        let ReputationJournalPayloadV1::StreamTokenValidation(outcome) = &admission.entry.payload
        else {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        };
        let head = gateway_heads_by_id
            .get(&admission.binding.gateway_id)
            .copied()
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        let entry_policy_position = policy_positions_by_digest
            .get(&admission.entry.authority_policy_digest)
            .copied()
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        admission
            .entry
            .validate_against_policy(&checkpoint.authority_policies[entry_policy_position])
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if let Some(record) = checkpoint
            .authority_policy_records
            .get(entry_policy_position)
            && (admission.entry.source_time_unix_ms < record.activated_at_unix_ms
                || checkpoint
                    .authority_policy_records
                    .get(entry_policy_position.saturating_add(1))
                    .is_some_and(|successor| {
                        admission.entry.source_time_unix_ms >= successor.activated_at_unix_ms
                    }))
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        let entry_digest = journal_entry_digest(&admission.entry)?;
        let source_material_digest = journal_source_material_digest(
            admission.entry.provider_id,
            admission.entry.source_time_unix_ms,
            admission.entry.predecessor_event_id,
            &admission.entry.payload,
        )?;
        if admission.binding.gateway_id == [0; 32]
            || admission.binding.gateway_sequence == 0
            || admission.binding.request_context_digest == [0; 32]
            || admission.admission_digest == [0; 32]
            || admission.event_id == ReputationJournalEventIdV1::ZERO
            || admission.entry.event_id != admission.event_id
            || admission.entry.source_kind() != ReputationJournalSourceKindV1::StreamToken
            || admission.binding != outcome.binding
            || !outcome.status.counts_for_provider()
            || stream_token_admission_digest(admission.entry.provider_id, outcome)?
                != admission.admission_digest
            || admission.binding.gateway_sequence > head.binding.gateway_sequence
            || previous_admission_binding.is_some_and(|previous| {
                previous
                    >= (
                        admission.binding.gateway_id,
                        admission.binding.gateway_sequence,
                    )
            })
            || admissions_by_sequence
                .insert(
                    (
                        admission.binding.gateway_id,
                        admission.binding.gateway_sequence,
                    ),
                    admission,
                )
                .is_some()
            || admissions_by_binding
                .insert(admission.binding, admission)
                .is_some()
            || admissions_by_event
                .insert(admission.event_id, admission)
                .is_some()
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        let retained_event = retained_by_event.get(&admission.event_id).copied();
        let retained_source = retained_by_source.get(&admission.entry.source_id).copied();
        match (retained_event, retained_source) {
            (None, None) => {}
            (Some(by_event), Some(by_source))
                if by_event == by_source
                    && by_event.entry_digest == entry_digest
                    && by_event.source_material_digest == source_material_digest =>
            {
                if pending_by_event
                    .get(&admission.event_id)
                    .is_some_and(|delivery| delivery.entry != admission.entry)
                {
                    return Err(ReputationRuntimeError::InvalidCheckpoint);
                }
            }
            _ => return Err(ReputationRuntimeError::InvalidCheckpoint),
        }
        previous_admission_binding = Some((
            admission.binding.gateway_id,
            admission.binding.gateway_sequence,
        ));
    }
    for head in &checkpoint.stream_token_gateway_heads {
        let admission = admissions_by_binding
            .get(&head.binding)
            .copied()
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        if admission.admission_digest != head.admission_digest
            || admission.event_id != head.event_id
            || admissions_by_event.get(&head.event_id).copied() != Some(admission)
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
    }
    let mut previous_pending = 0_u64;
    for entry in &checkpoint.pending {
        let entry_policy_position = policy_positions_by_digest
            .get(&entry.entry.authority_policy_digest)
            .copied()
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        let entry_policy = &checkpoint.authority_policies[entry_policy_position];
        entry
            .entry
            .validate_against_policy(entry_policy)
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if let Some(record) = checkpoint
            .authority_policy_records
            .get(entry_policy_position)
        {
            if entry.entry.source_time_unix_ms < record.activated_at_unix_ms
                || checkpoint
                    .authority_policy_records
                    .get(entry_policy_position.saturating_add(1))
                    .is_some_and(|successor| {
                        entry.entry.source_time_unix_ms >= successor.activated_at_unix_ms
                    })
            {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
        }
        if let ReputationJournalPayloadV1::StreamTokenValidation(outcome) = &entry.entry.payload {
            let head = gateway_heads_by_id
                .get(&outcome.binding.gateway_id)
                .copied()
                .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
            let admission_digest = stream_token_admission_digest(entry.entry.provider_id, outcome)?;
            if outcome.binding.gateway_sequence > head.binding.gateway_sequence {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
            if outcome.binding.gateway_sequence == head.binding.gateway_sequence
                && (head.binding != outcome.binding
                    || head.admission_digest != admission_digest
                    || head.event_id != entry.entry.event_id)
            {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
            if let Some(admission) = admissions_by_binding.get(&outcome.binding).copied()
                && (admission.admission_digest != admission_digest
                    || admission.event_id != entry.entry.event_id
                    || admission.entry != entry.entry)
            {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
        }
        if !matches!(
            entry.entry.source_kind(),
            ReputationJournalSourceKindV1::Por | ReputationJournalSourceKindV1::StreamToken
        ) || entry.sequence == 0
            || entry.sequence <= previous_pending
            || entry.entry_digest == [0; 32]
            || journal_entry_digest(&entry.entry)? != entry.entry_digest
            || entry.source_material_digest == [0; 32]
            || journal_source_material_digest(
                entry.entry.provider_id,
                entry.entry.source_time_unix_ms,
                entry.entry.predecessor_event_id,
                &entry.entry.payload,
            )? != entry.source_material_digest
            || entry.attempts > policy.max_attempts
            || entry.failure_receipts.len()
                > usize::try_from(entry.attempts)
                    .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?
            || !valid_failure_receipts(&entry.failure_receipts)
            || !match entry.state {
                ReputationJournalDeliveryStateV1::Ready => entry.baseline_finalized.is_none(),
                ReputationJournalDeliveryStateV1::Ambiguous
                | ReputationJournalDeliveryStateV1::Submitted => entry
                    .baseline_finalized
                    .is_some_and(|cursor| cursor.validate().is_ok()),
            }
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        previous_pending = entry.sequence;
        max_sequence = max_sequence.max(entry.sequence);
        if !sequences.insert(entry.sequence) {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
    }
    let mut previous_completed = 0_u64;
    for entry in &checkpoint.completed {
        if entry.sequence == 0
            || entry.sequence <= previous_completed
            || entry.event_id == ReputationJournalEventIdV1::ZERO
            || entry.source_id == ReputationJournalSourceIdV1::ZERO
            || entry.entry_digest == [0; 32]
            || entry.source_material_digest == [0; 32]
            || entry.committed.validate().is_err()
            || !committed_sequences.insert(entry.committed.sequence)
            || !sequences.insert(entry.sequence)
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        previous_completed = entry.sequence;
        max_sequence = max_sequence.max(entry.sequence);
    }
    let mut previous_observed_sequence = 0_u64;
    for entry in &checkpoint.observed {
        if entry.event_id == ReputationJournalEventIdV1::ZERO
            || entry.source_id == ReputationJournalSourceIdV1::ZERO
            || entry.entry_digest == [0; 32]
            || entry.source_material_digest == [0; 32]
            || entry.committed.validate().is_err()
            || entry.committed.sequence <= previous_observed_sequence
            || !committed_sequences.insert(entry.committed.sequence)
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        previous_observed_sequence = entry.committed.sequence;
    }
    let mut previous_dead = 0_u64;
    for entry in &checkpoint.dead_letters {
        if entry.sequence == 0
            || entry.sequence <= previous_dead
            || entry.event_id == ReputationJournalEventIdV1::ZERO
            || entry.source_id == ReputationJournalSourceIdV1::ZERO
            || entry.entry_digest == [0; 32]
            || entry.source_material_digest == [0; 32]
            || entry.attempts != policy.max_attempts
            || entry.failure_receipts.len()
                > usize::try_from(entry.attempts)
                    .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?
            || !valid_failure_receipts(&entry.failure_receipts)
            || !sequences.insert(entry.sequence)
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        previous_dead = entry.sequence;
        max_sequence = max_sequence.max(entry.sequence);
    }
    if max_sequence > checkpoint.last_assigned_sequence {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    Ok(())
}

fn valid_failure_receipts(receipts: &[[u8; 32]]) -> bool {
    let mut unique = BTreeSet::new();
    receipts
        .iter()
        .all(|receipt| *receipt != [0; 32] && unique.insert(*receipt))
}

/// Strict bounded policy for native reputation-journal transaction delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationJournalDeliveryPolicyV1 {
    version: u8,
    chain_id: ChainId,
    finalized_query_handle: String,
    finalized_query_qualification: ReputationRuntimeProviderQualificationV1,
    transaction_submitter_handle: String,
    transaction_submitter_qualification: ReputationRuntimeProviderQualificationV1,
    page_items: u32,
    max_pages_per_tick: u32,
    max_submissions_per_tick: u32,
}

impl ReputationJournalDeliveryPolicyV1 {
    /// Construct the bounded first-release delivery policy.
    ///
    /// # Errors
    ///
    /// Rejects an inert chain or dependency handle.
    pub fn strict_v1(
        chain_id: ChainId,
        finalized_query_handle: impl Into<String>,
        finalized_query_qualification: ReputationRuntimeProviderQualificationV1,
        transaction_submitter_handle: impl Into<String>,
    ) -> Result<Self, ReputationRuntimeError> {
        let page_items = u32::try_from(REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
            .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
        let finalized_query_handle = validate_runtime_handle(finalized_query_handle.into())?;
        let transaction_submitter_handle =
            validate_runtime_handle(transaction_submitter_handle.into())?;
        let transaction_submitter_qualification = ReputationRuntimeProviderQualificationV1::new(
            REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
            reputation_journal_submitter_policy_digest_v1(
                &chain_id,
                &transaction_submitter_handle,
            )?,
        );
        let policy = Self {
            version: REPUTATION_JOURNAL_DELIVERY_POLICY_VERSION_V1,
            chain_id,
            finalized_query_handle,
            finalized_query_qualification,
            transaction_submitter_handle,
            transaction_submitter_qualification,
            page_items,
            max_pages_per_tick: REPUTATION_JOURNAL_DELIVERY_MAX_PAGES_PER_TICK_V1,
            max_submissions_per_tick: REPUTATION_JOURNAL_DELIVERY_MAX_SUBMISSIONS_PER_TICK_V1,
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), ReputationRuntimeError> {
        if self.version != REPUTATION_JOURNAL_DELIVERY_POLICY_VERSION_V1
            || self.chain_id.as_str().is_empty()
            || self.chain_id.as_str().len() > super::REPUTATION_INGEST_MAX_CHAIN_ID_BYTES_V1
            || self.page_items == 0
            || usize::try_from(self.page_items)
                .map_or(true, |limit| limit > REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
            || self.max_pages_per_tick == 0
            || self.max_pages_per_tick > REPUTATION_JOURNAL_DELIVERY_MAX_PAGES_PER_TICK_V1
            || self.max_submissions_per_tick == 0
            || self.max_submissions_per_tick
                > REPUTATION_JOURNAL_DELIVERY_MAX_SUBMISSIONS_PER_TICK_V1
            || self.finalized_query_qualification.revision()
                != REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1
            || !self.finalized_query_qualification.is_valid()
            || self.transaction_submitter_qualification.revision()
                != REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1
            || !self.transaction_submitter_qualification.is_valid()
            || self.transaction_submitter_qualification.policy_digest()
                != reputation_journal_submitter_policy_digest_v1(
                    &self.chain_id,
                    &self.transaction_submitter_handle,
                )?
        {
            return Err(ReputationRuntimeError::InvalidRuntimePolicy);
        }
        validate_runtime_handle(self.finalized_query_handle.clone())?;
        validate_runtime_handle(self.transaction_submitter_handle.clone())?;
        Ok(())
    }

    /// Qualify or revalidate the exact finalized-query provider.
    ///
    /// # Errors
    ///
    /// Rejects a provider whose handle, revision, or public policy does not
    /// match the independently constructed delivery policy.
    pub fn revalidate_query_provider(
        &self,
        query: &dyn ReputationFinalizedQueryV1,
    ) -> Result<(), ReputationRuntimeError> {
        assert_runtime_provider_qualification(
            &self.finalized_query_handle,
            self.finalized_query_qualification,
            query,
        )
    }

    /// Qualify or revalidate the exact native transaction submitter.
    ///
    /// # Errors
    ///
    /// Rejects a provider whose handle, revision, or public policy does not
    /// match the independently constructed delivery policy.
    pub fn revalidate_submitter_provider(
        &self,
        submitter: &dyn ReputationJournalTransactionSubmitterV1,
    ) -> Result<(), ReputationRuntimeError> {
        assert_runtime_provider_qualification(
            &self.transaction_submitter_handle,
            self.transaction_submitter_qualification,
            submitter,
        )
    }
}

/// Derive the public V1 policy digest for a chain-bound journal submitter.
///
/// The handle remains an independently configured identity; including its
/// digest here prevents the same qualification from authorizing a substituted
/// submitter role.
///
/// # Errors
///
/// Rejects an invalid chain or malformed/test-marked runtime handle, or a
/// canonical digest failure.
pub fn reputation_journal_submitter_policy_digest_v1(
    chain_id: &ChainId,
    handle: &str,
) -> Result<[u8; 32], ReputationRuntimeError> {
    if chain_id.as_str().is_empty()
        || chain_id.as_str().len() > super::REPUTATION_INGEST_MAX_CHAIN_ID_BYTES_V1
    {
        return Err(ReputationRuntimeError::InvalidRuntimePolicy);
    }
    let handle = validate_runtime_handle(handle.to_owned())?;
    hash_canonical(
        b"sorafs-reputation-journal-submitter-policy-v1",
        &ReputationJournalSubmitterPolicyDigestMaterialV1 {
            chain_id: chain_id.clone(),
            handle_digest: domain_digest(
                b"sorafs-reputation-runtime-handle-v1",
                handle.as_bytes(),
            )?,
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize)]
struct ReputationJournalSubmitterPolicyDigestMaterialV1 {
    chain_id: ChainId,
    handle_digest: [u8; 32],
}

/// Exact canonical append request sent to an injected transaction submitter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationJournalTransactionRequestV1 {
    /// Monotonic local producer sequence.
    pub sequence: u64,
    /// Exact active chain.
    pub chain_id: ChainId,
    /// Governed transaction authority captured by the typed append.
    pub authority: AccountId,
    /// Content-derived native journal event identity.
    pub event_id: ReputationJournalEventIdV1,
    /// Native source identity.
    pub source_id: ReputationJournalSourceIdV1,
    /// One-based bounded delivery attempt.
    pub attempt: u32,
    /// Domain-separated retry-safe operation key.
    pub idempotency_key: [u8; 32],
    /// Exact typed native append instruction.
    pub instruction: ReputationJournalAppendInstructionV1,
}

impl ReputationJournalTransactionRequestV1 {
    /// Validate the exact typed instruction and attempt-bound operation key.
    ///
    /// # Errors
    ///
    /// Rejects inert bounds, malformed native material, field substitution,
    /// or an idempotency key that was not derived from the exact request.
    pub fn validate(&self) -> Result<(), ReputationRuntimeError> {
        if self.sequence == 0
            || self.chain_id.as_str().is_empty()
            || self.chain_id.as_str().len() > super::REPUTATION_INGEST_MAX_CHAIN_ID_BYTES_V1
            || self.attempt == 0
            || self.attempt > REPUTATION_JOURNAL_PRODUCER_MAX_ATTEMPTS_V1
            || self.idempotency_key == [0; 32]
        {
            return Err(ReputationRuntimeError::InvalidJournalTransition);
        }
        let entry = match &self.instruction {
            ReputationJournalAppendInstructionV1::Por(instruction) => {
                if instruction.entry().source_kind() != ReputationJournalSourceKindV1::Por {
                    return Err(ReputationRuntimeError::InvalidJournalEntry);
                }
                instruction.entry()
            }
            ReputationJournalAppendInstructionV1::StreamToken(instruction) => {
                if instruction.entry().source_kind() != ReputationJournalSourceKindV1::StreamToken {
                    return Err(ReputationRuntimeError::InvalidJournalEntry);
                }
                instruction.entry()
            }
        };
        entry
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidJournalEntry)?;
        if entry.event_id != self.event_id
            || entry.source_id != self.source_id
            || entry.recorded_by != self.authority
        {
            return Err(ReputationRuntimeError::InvalidJournalEntry);
        }
        let instruction_digest = journal_entry_digest(entry)?;
        let expected = hash_canonical(
            b"sorafs-reputation-journal-transaction-operation-v1",
            &ReputationJournalTransactionIdempotencyMaterialV1 {
                sequence: self.sequence,
                chain_id: self.chain_id.clone(),
                authority: self.authority.clone(),
                event_id: self.event_id,
                source_id: self.source_id,
                attempt: self.attempt,
                instruction_digest,
            },
        )?;
        if self.idempotency_key != expected {
            return Err(ReputationRuntimeError::InvalidJournalTransition);
        }
        Ok(())
    }
}

/// Synchronous queue handoff result; none of these variants imply finality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationJournalTransactionSubmitOutcomeV1 {
    /// The normal transaction queue accepted the exact signed transaction.
    Queued {
        /// Stable payload-free queue receipt.
        receipt: [u8; 32],
    },
    /// The submitter proves the transaction never entered the queue.
    NotQueued {
        /// Stable payload-free failed-attempt receipt.
        receipt: [u8; 32],
    },
    /// Queue ownership is uncertain; committed state must be queried.
    Ambiguous {
        /// Stable payload-free ambiguity receipt.
        receipt: [u8; 32],
    },
}

impl ReputationJournalTransactionSubmitOutcomeV1 {
    const fn receipt(self) -> [u8; 32] {
        match self {
            Self::Queued { receipt }
            | Self::NotQueued { receipt }
            | Self::Ambiguous { receipt } => receipt,
        }
    }
}

/// Identity-pinned runtime-only signer and normal-queue transaction submitter.
///
/// Implementations own or route to runtime-only signing identities. They must
/// reject any request whose `authority` is not the exact account derived from
/// the selected signing key. A queued receipt is only a handoff
/// acknowledgement; this interface has no method that can report finality.
pub trait ReputationJournalTransactionSubmitterV1: ReputationRuntimeProviderV1 {
    /// Return whether one exact governed authority has a runtime-only signer.
    fn supports_authority(&self, authority: &AccountId) -> bool;

    /// Sign and hand one exact typed append to the normal transaction queue.
    fn submit(
        &self,
        request: &ReputationJournalTransactionRequestV1,
    ) -> ReputationJournalTransactionSubmitOutcomeV1;
}

/// Payload-free result of one bounded journal-delivery tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationJournalDeliveryTickOutcomeV1 {
    /// Typed committed journal pages durably scanned.
    pub pages: u32,
    /// Exact local pending rows acknowledged from committed events.
    pub committed: u32,
    /// Rows proven absent at later finality and returned to Ready.
    pub retries_ready: u32,
    /// New queue handoffs accepted, still awaiting finality.
    pub queued: u32,
    /// New attempts whose queue ownership remains ambiguous.
    pub ambiguous: u32,
    /// Rows that reached the governed terminal retry ceiling in this tick.
    pub dead_lettered: u32,
    /// Whether the scanner consumed the terminal page of the selected view.
    pub caught_up: bool,
}

/// Payload-free supervised status of journal transaction delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationJournalDeliveryRuntimeStatusV1 {
    /// Whether at least one complete worker tick succeeded.
    pub live: bool,
    /// Whether dependencies, scan, outbox, and dead-letter state are ready.
    pub ready: bool,
    /// Consecutive failed ticks since the latest success.
    pub consecutive_failures: u64,
    /// Latest exact finalized identity scanned to its terminal page.
    pub latest_finalized: Option<ReputationFinalizedIdentityV1>,
    /// Active recorder-policy digest bound from that view.
    pub active_authority_policy_digest: [u8; 32],
    /// Active recorder-policy revision.
    pub active_authority_policy_revision: u64,
    /// Payload-free durable producer counts.
    pub producer: ReputationJournalProducerStatusV1,
    /// Latest external failure receipt, if the latest tick failed externally.
    pub latest_failure_receipt: Option<[u8; 32]>,
}

/// Monotonic payload-free worker counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReputationJournalDeliveryMetricsV1 {
    /// Successful bounded ticks.
    pub successful_ticks: u64,
    /// Failed bounded ticks.
    pub failed_ticks: u64,
    /// Committed pages scanned.
    pub pages: u64,
    /// Exact pending rows acknowledged as committed.
    pub committed: u64,
    /// Queue handoffs accepted but not treated as final.
    pub queued: u64,
    /// Ambiguous queue handoffs.
    pub ambiguous: u64,
    /// Attempts returned to Ready.
    pub retries_ready: u64,
    /// Rows moved to terminal dead letter.
    pub dead_lettered: u64,
    /// Direct recorder-policy rotations accepted.
    pub policy_rotations: u64,
}

#[derive(Debug, Default)]
struct ReputationJournalDeliverySupervisorState {
    live: bool,
    consecutive_failures: u64,
    latest_failure_receipt: Option<[u8; 32]>,
    metrics: ReputationJournalDeliveryMetricsV1,
}

/// Supervised bounded worker for durable native reputation-journal appends.
#[derive(Debug)]
pub struct ReputationJournalDeliveryWorkerV1 {
    outbox: Arc<ReputationJournalProducerOutboxV1>,
    query: Arc<dyn ReputationFinalizedQueryV1>,
    submitter: Arc<dyn ReputationJournalTransactionSubmitterV1>,
    policy: ReputationJournalDeliveryPolicyV1,
    state: Mutex<ReputationJournalDeliverySupervisorState>,
    reconcile_lock: Mutex<()>,
}

impl ReputationJournalDeliveryWorkerV1 {
    /// Bind one outbox, exact finalized query, and runtime-only submitter.
    ///
    /// # Errors
    ///
    /// Rejects chain or dependency identity substitution.
    pub fn new(
        outbox: Arc<ReputationJournalProducerOutboxV1>,
        policy: ReputationJournalDeliveryPolicyV1,
        query: Arc<dyn ReputationFinalizedQueryV1>,
        submitter: Arc<dyn ReputationJournalTransactionSubmitterV1>,
    ) -> Result<Self, ReputationRuntimeError> {
        policy.validate()?;
        if outbox.policy.chain_id != policy.chain_id {
            return Err(ReputationRuntimeError::RuntimeBindingMismatch);
        }
        qualify_runtime_provider(
            &policy.finalized_query_handle,
            policy.finalized_query_qualification,
            query.as_ref(),
        )?;
        qualify_runtime_provider(
            &policy.transaction_submitter_handle,
            policy.transaction_submitter_qualification,
            submitter.as_ref(),
        )?;
        Ok(Self {
            outbox,
            query,
            submitter,
            policy,
            state: Mutex::new(ReputationJournalDeliverySupervisorState::default()),
            reconcile_lock: Mutex::new(()),
        })
    }

    /// Return a concrete production callback for native PoR terminal owners.
    #[must_use]
    pub fn por_producer(&self) -> PorReputationJournalProducerV1 {
        PorReputationJournalProducerV1::new(Arc::clone(&self.outbox))
    }

    /// Execute one bounded scan, retry, and queue-submission tick.
    ///
    /// # Errors
    ///
    /// Fails closed for dependency substitution, malformed exact views,
    /// rollback/fork, policy-lineage gaps, unknown signer authority, durable
    /// state failure, or resource exhaustion.
    pub fn reconcile_once(
        &self,
    ) -> Result<ReputationJournalDeliveryTickOutcomeV1, ReputationRuntimeError> {
        let _guard = self
            .reconcile_lock
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let result = self.reconcile_once_inner();
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        match &result {
            Ok(outcome) => {
                state.live = true;
                state.consecutive_failures = 0;
                state.latest_failure_receipt = None;
                state.metrics.successful_ticks = state.metrics.successful_ticks.saturating_add(1);
                state.metrics.pages = state.metrics.pages.saturating_add(u64::from(outcome.pages));
                state.metrics.committed = state
                    .metrics
                    .committed
                    .saturating_add(u64::from(outcome.committed));
                state.metrics.queued = state
                    .metrics
                    .queued
                    .saturating_add(u64::from(outcome.queued));
                state.metrics.ambiguous = state
                    .metrics
                    .ambiguous
                    .saturating_add(u64::from(outcome.ambiguous));
                state.metrics.retries_ready = state
                    .metrics
                    .retries_ready
                    .saturating_add(u64::from(outcome.retries_ready));
                state.metrics.dead_lettered = state
                    .metrics
                    .dead_lettered
                    .saturating_add(u64::from(outcome.dead_lettered));
            }
            Err(error) => {
                state.consecutive_failures = state.consecutive_failures.saturating_add(1);
                state.latest_failure_receipt = error.external_receipt();
                state.metrics.failed_ticks = state.metrics.failed_ticks.saturating_add(1);
            }
        }
        result
    }

    /// Return payload-free worker health and durable queue state.
    ///
    /// # Errors
    ///
    /// Returns a checkpoint or runtime-lock error.
    pub fn status(
        &self,
    ) -> Result<ReputationJournalDeliveryRuntimeStatusV1, ReputationRuntimeError> {
        let producer = self.outbox.status()?;
        let scan = self.outbox.scan_status()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let pending = producer
            .ready
            .saturating_add(producer.ambiguous)
            .saturating_add(producer.submitted);
        let ready = state.live
            && state.consecutive_failures == 0
            && scan.caught_up
            && pending == 0
            && producer.dead_letters == 0;
        Ok(ReputationJournalDeliveryRuntimeStatusV1 {
            live: state.live,
            ready,
            consecutive_failures: state.consecutive_failures,
            latest_finalized: scan.finalized.map(|cursor| ReputationFinalizedIdentityV1 {
                height: cursor.height,
                block_hash: cursor.block_hash,
            }),
            active_authority_policy_digest: scan.active_authority_policy_digest,
            active_authority_policy_revision: scan.active_authority_policy_revision,
            producer,
            latest_failure_receipt: state.latest_failure_receipt,
        })
    }

    /// Return monotonic payload-free worker counters.
    ///
    /// # Errors
    ///
    /// Returns a runtime-lock error.
    pub fn metrics(&self) -> Result<ReputationJournalDeliveryMetricsV1, ReputationRuntimeError> {
        self.state
            .lock()
            .map(|state| state.metrics)
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)
    }

    fn reconcile_once_inner(
        &self,
    ) -> Result<ReputationJournalDeliveryTickOutcomeV1, ReputationRuntimeError> {
        self.ensure_bindings()?;

        let mut pages = 0_u32;
        let mut committed = 0_u32;
        let terminal_view = loop {
            if pages >= self.policy.max_pages_per_tick {
                return Err(ReputationRuntimeError::QueryResourceExhausted);
            }
            let scan = self.outbox.scan_status()?;
            let requested_after = scan.after;
            self.ensure_query_binding()?;
            let view_result = self.query.reputation_journal_delivery_view(
                &self.policy.chain_id,
                u64::MAX,
                FindSorafsReputationJournalAuthorityPolicy,
                requested_after,
                self.policy.page_items,
            );
            self.ensure_query_binding()?;
            let view = view_result?;
            view.validate_for_request(
                &self.policy.chain_id,
                requested_after,
                self.policy.page_items,
                u64::MAX,
            )?;
            let rotations = self.outbox.synchronize_authority_policy_history(
                &view.authority_policy_history,
                view.journal_page.finalized_cursor,
            )?;
            if rotations != 0 {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
                state.metrics.policy_rotations = state
                    .metrics
                    .policy_rotations
                    .saturating_add(u64::from(rotations));
            }
            let has_more = view.journal_page.has_more;
            committed = committed
                .checked_add(
                    self.outbox
                        .reconcile_finalized_journal_page(view.journal_page.clone())?,
                )
                .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
            pages = pages
                .checked_add(1)
                .ok_or(ReputationRuntimeError::QueryResourceExhausted)?;
            if !has_more {
                break view;
            }
        };

        let current_identity = terminal_view.anchor.identity;
        let mut retries_ready = 0_u32;
        let mut dead_lettered = 0_u32;
        for pending in self
            .outbox
            .pending_matching(self.policy.max_submissions_per_tick, |state| {
                state != ReputationJournalDeliveryStateV1::Ready
            })?
        {
            if !matches!(
                pending.state,
                ReputationJournalDeliveryStateV1::Ambiguous
                    | ReputationJournalDeliveryStateV1::Submitted
            ) {
                continue;
            }
            let baseline = pending
                .baseline_finalized
                .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
            if current_identity.height <= baseline.height {
                continue;
            }
            let receipt = journal_absence_receipt(
                pending.event_id,
                pending.attempts,
                baseline,
                current_identity,
            )?;
            match self
                .outbox
                .mark_finalized_absent(pending.event_id, current_identity, receipt)?
            {
                ReputationJournalDeliveryOutcomeV1::RetryReady { .. } => {
                    retries_ready = retries_ready
                        .checked_add(1)
                        .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
                }
                ReputationJournalDeliveryOutcomeV1::DeadLettered { .. } => {
                    dead_lettered = dead_lettered
                        .checked_add(1)
                        .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
                }
                ReputationJournalDeliveryOutcomeV1::ExactReplay => {}
                ReputationJournalDeliveryOutcomeV1::Committed => {
                    return Err(ReputationRuntimeError::InvalidJournalTransition);
                }
            }
        }

        let mut queued = 0_u32;
        let mut ambiguous = 0_u32;
        let mut submissions = 0_u32;
        for pending in self
            .outbox
            .pending_matching(self.policy.max_submissions_per_tick, |state| {
                state == ReputationJournalDeliveryStateV1::Ready
            })?
        {
            if submissions >= self.policy.max_submissions_per_tick {
                break;
            }
            let submission = match self.outbox.begin_submission_against_active_policy(
                pending.event_id,
                terminal_view.authority_policy.policy_digest,
                current_identity,
                terminal_view.anchor.finalized_at_unix_ms,
            ) {
                Ok(submission) => submission,
                Err(ReputationRuntimeError::JournalSourceNotFinalized) => continue,
                Err(error) => return Err(error),
            };
            self.ensure_submitter_binding()?;
            let supports_authority = self.submitter.supports_authority(&submission.authority);
            self.ensure_submitter_binding()?;
            if !supports_authority {
                return Err(ReputationRuntimeError::JournalSubmitterAuthorityMismatch);
            }
            let request = journal_transaction_request(submission)?;
            self.ensure_submitter_binding()?;
            let outcome = self.submitter.submit(&request);
            self.ensure_submitter_binding()?;
            if outcome.receipt() == [0; 32] {
                return Err(ReputationRuntimeError::InvalidExternalReceipt);
            }
            match outcome {
                ReputationJournalTransactionSubmitOutcomeV1::Queued { .. } => {
                    self.outbox.mark_submitted(request.event_id)?;
                    queued = queued
                        .checked_add(1)
                        .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
                }
                ReputationJournalTransactionSubmitOutcomeV1::NotQueued { receipt } => {
                    if matches!(
                        self.outbox
                            .record_not_submitted(request.event_id, receipt)?,
                        ReputationJournalDeliveryOutcomeV1::DeadLettered { .. }
                    ) {
                        dead_lettered = dead_lettered
                            .checked_add(1)
                            .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
                    }
                }
                ReputationJournalTransactionSubmitOutcomeV1::Ambiguous { .. } => {
                    ambiguous = ambiguous
                        .checked_add(1)
                        .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
                }
            }
            submissions = submissions
                .checked_add(1)
                .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
        }
        Ok(ReputationJournalDeliveryTickOutcomeV1 {
            pages,
            committed,
            retries_ready,
            queued,
            ambiguous,
            dead_lettered,
            caught_up: true,
        })
    }

    fn ensure_bindings(&self) -> Result<(), ReputationRuntimeError> {
        self.ensure_query_binding()?;
        self.ensure_submitter_binding()
    }

    fn ensure_query_binding(&self) -> Result<(), ReputationRuntimeError> {
        self.policy.revalidate_query_provider(self.query.as_ref())
    }

    fn ensure_submitter_binding(&self) -> Result<(), ReputationRuntimeError> {
        self.policy
            .revalidate_submitter_provider(self.submitter.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize)]
struct ReputationJournalTransactionIdempotencyMaterialV1 {
    sequence: u64,
    chain_id: ChainId,
    authority: AccountId,
    event_id: ReputationJournalEventIdV1,
    source_id: ReputationJournalSourceIdV1,
    attempt: u32,
    instruction_digest: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize)]
struct ReputationJournalAbsenceReceiptMaterialV1 {
    event_id: ReputationJournalEventIdV1,
    attempt: u32,
    baseline: ReputationFinalizedIdentityV1,
    observed: ReputationFinalizedIdentityV1,
}

fn journal_transaction_request(
    submission: ReputationJournalSubmissionV1,
) -> Result<ReputationJournalTransactionRequestV1, ReputationRuntimeError> {
    let instruction_digest = match &submission.instruction {
        ReputationJournalAppendInstructionV1::Por(instruction) => {
            journal_entry_digest(instruction.entry())?
        }
        ReputationJournalAppendInstructionV1::StreamToken(instruction) => {
            journal_entry_digest(instruction.entry())?
        }
    };
    let material = ReputationJournalTransactionIdempotencyMaterialV1 {
        sequence: submission.sequence,
        chain_id: submission.chain_id.clone(),
        authority: submission.authority.clone(),
        event_id: submission.event_id,
        source_id: submission.source_id,
        attempt: submission.attempts,
        instruction_digest,
    };
    let idempotency_key = hash_canonical(
        b"sorafs-reputation-journal-transaction-operation-v1",
        &material,
    )?;
    let request = ReputationJournalTransactionRequestV1 {
        sequence: submission.sequence,
        chain_id: submission.chain_id,
        authority: submission.authority,
        event_id: submission.event_id,
        source_id: submission.source_id,
        attempt: submission.attempts,
        idempotency_key,
        instruction: submission.instruction,
    };
    request.validate()?;
    Ok(request)
}

fn journal_absence_receipt(
    event_id: ReputationJournalEventIdV1,
    attempt: u32,
    baseline: ReputationFinalizedIdentityV1,
    observed: ReputationFinalizedIdentityV1,
) -> Result<[u8; 32], ReputationRuntimeError> {
    hash_canonical(
        b"sorafs-reputation-journal-finalized-absence-v1",
        &ReputationJournalAbsenceReceiptMaterialV1 {
            event_id,
            attempt,
            baseline,
            observed,
        },
    )
}

/// Exact idempotent request sent to the external threshold-signing service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationThresholdSigningRequestV1 {
    /// Stable release-window delivery sequence.
    pub sequence: u64,
    /// Exact projector material digest.
    pub material_digest: [u8; 32],
    /// Domain-separated idempotency key for the external service.
    pub idempotency_key: [u8; 32],
    /// Canonical immutable unsigned projector material.
    pub material: ReputationUnsignedSigningMaterialV1,
}

/// Identity-pinned external threshold-signing client.
///
/// The client must reconcile by `idempotency_key`: returning `None` means the
/// operation remains pending, and a later call must never sign different
/// material under the same key.
pub trait ReputationThresholdSignerClientV1: ReputationRuntimeProviderV1 {
    /// Submit or reconcile one exact unsigned-material operation.
    fn reconcile_signature(
        &self,
        request: &ReputationThresholdSigningRequestV1,
    ) -> Result<Option<SignedReputationSnapshotV1>, ReputationExternalFailureV1>;
}

/// Exact idempotent Governance DAG publication request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationGovernanceDagPublicationRequestV1 {
    /// Stable release-window delivery sequence.
    pub sequence: u64,
    /// Exact projector material digest.
    pub material_digest: [u8; 32],
    /// Digest of the canonical signed result.
    pub signed_result_digest: [u8; 32],
    /// Domain-separated publication idempotency key.
    pub idempotency_key: [u8; 32],
    /// Verified threshold-signed snapshot.
    pub signed_result: SignedReputationSnapshotV1,
    /// Exact canonical bytes of `signed_result`.
    pub canonical_signed_result: Vec<u8>,
}

/// Authenticated bounded proof that one publication is included below a signed
/// Governance DAG head.
///
/// `inclusion_path` is in ascending block order, contains the exact requested
/// snapshot exactly once, and ends at `head.head_block_cid`. After the first
/// acknowledgement, its first block must be the immediate successor of the
/// previously authenticated head.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ReputationGovernanceDagReadbackV1 {
    /// Schema version.
    pub version: u8,
    /// Pinned-publisher signed head manifest.
    pub head: GovernanceDagHeadV1,
    /// Bounded contiguous block suffix containing the requested snapshot.
    pub inclusion_path: Vec<GovernanceDagBlockV1>,
}

/// Identity-pinned Governance DAG publication and readback client.
///
/// A submitter-side success without a verified signed head and bounded
/// inclusion path remains pending.
pub trait ReputationGovernanceDagClientV1: ReputationRuntimeProviderV1 {
    /// Publish or reconcile the exact snapshot, returning an authenticated
    /// signed-head inclusion receipt only after readback.
    fn reconcile_publication(
        &self,
        request: &ReputationGovernanceDagPublicationRequestV1,
    ) -> Result<Option<ReputationGovernanceDagReadbackV1>, ReputationExternalFailureV1>;
}

/// Derive the payload-free public policy digest for one Governance DAG client.
///
/// The digest binds the configured publisher peer identity and exact Ed25519
/// public key under the reputation Governance DAG role. Runtime credentials,
/// private keys, endpoints, and publication payloads are deliberately absent.
///
/// # Errors
///
/// Rejects an empty or oversized peer identity and an inert or malformed
/// Ed25519 public key.
pub fn reputation_governance_dag_policy_digest_v1(
    publisher_peer_id: &[u8],
    publisher_public_key: [u8; 32],
) -> Result<[u8; 32], ReputationRuntimeError> {
    if publisher_peer_id.is_empty()
        || publisher_peer_id.len() > REPUTATION_RUNTIME_MAX_GOVERNANCE_PEER_ID_BYTES_V1
        || publisher_public_key == [0; 32]
        || !valid_ed25519_verifying_key(publisher_public_key)
    {
        return Err(ReputationRuntimeError::InvalidRuntimePolicy);
    }
    let peer_id_len = u64::try_from(publisher_peer_id.len())
        .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs-reputation-governance-dag-provider-ed25519-v1");
    hasher.update(&peer_id_len.to_le_bytes());
    hasher.update(publisher_peer_id);
    hasher.update(&publisher_public_key);
    Ok(*hasher.finalize().as_bytes())
}

/// Payload-free proof that the exact threshold result is in the Governance DAG.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct ReputationGovernanceDagAcknowledgementV1 {
    /// Schema version.
    pub version: u8,
    /// Stable projector delivery sequence.
    pub sequence: u64,
    /// Exact unsigned-material digest.
    pub material_digest: [u8; 32],
    /// Digest of the exact signed result.
    pub signed_result_digest: [u8; 32],
    /// Governance DAG block sequence.
    pub dag_block_sequence: u64,
    /// Exact 32-byte signed block CID.
    pub dag_block_cid: [u8; 32],
    /// Exact 32-byte signed node CID.
    pub dag_node_cid: [u8; 32],
    /// Governance block publication time in Unix seconds.
    pub published_at_unix: u64,
    /// Digest of the pinned Governance DAG Ed25519 public key.
    pub publisher_key_digest: [u8; 32],
}

impl ReputationGovernanceDagAcknowledgementV1 {
    fn validate(&self) -> Result<(), ReputationRuntimeError> {
        if self.version != REPUTATION_GOVERNANCE_DAG_ACKNOWLEDGEMENT_VERSION_V1
            || self.sequence == 0
            || self.material_digest == [0; 32]
            || self.signed_result_digest == [0; 32]
            || self.dag_block_cid == [0; 32]
            || self.dag_node_cid == [0; 32]
            || self.published_at_unix == 0
            || self.publisher_key_digest == [0; 32]
        {
            return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
        }
        Ok(())
    }
}

/// Compact durable form of the exact target block in a Governance DAG path.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationGovernanceDagTargetBlockV1 {
    block_cid: Vec<u8>,
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    node_cid: Vec<u8>,
    prev_node_cid: Option<Vec<u8>>,
    node_timestamp: u64,
    node_publisher_signature: GovernanceLogSignatureV1,
    block_signature: GovernanceLogSignatureV1,
}

impl StoredReputationGovernanceDagTargetBlockV1 {
    fn from_block(block: &GovernanceDagBlockV1) -> Self {
        Self {
            block_cid: block.block_cid.clone(),
            prev_block_cid: block.prev_block_cid.clone(),
            sequence: block.sequence,
            timestamp: block.timestamp,
            publisher_peer_id: block.publisher_peer_id.clone(),
            node_cid: block.node.node_cid.clone(),
            prev_node_cid: block.node.prev_cid.clone(),
            node_timestamp: block.node.timestamp,
            node_publisher_signature: block.node.publisher_signature.clone(),
            block_signature: block.block_signature.clone(),
        }
    }

    fn reconstruct_block(
        &self,
        signed_result: &SignedReputationSnapshotV1,
    ) -> GovernanceDagBlockV1 {
        GovernanceDagBlockV1 {
            version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
            block_cid: self.block_cid.clone(),
            prev_block_cid: self.prev_block_cid.clone(),
            sequence: self.sequence,
            timestamp: self.timestamp,
            publisher_peer_id: self.publisher_peer_id.clone(),
            node: GovernanceLogNodeV1 {
                version: GOVERNANCE_LOG_VERSION_V1,
                node_cid: self.node_cid.clone(),
                prev_cid: self.prev_node_cid.clone(),
                timestamp: self.node_timestamp,
                publisher_peer_id: self.publisher_peer_id.clone(),
                payload: GovernanceLogPayloadV1::SignedReputationSnapshot(signed_result.clone()),
                publisher_signature: self.node_publisher_signature.clone(),
            },
            block_signature: self.block_signature.clone(),
        }
    }
}

/// Durable signed-head readback with the exact target payload stored once.
///
/// Non-target path blocks and the signed head are retained verbatim. The
/// target block is reconstructed from the checkpoint's separately retained
/// signed reputation snapshot before every validation, including restart.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationGovernanceDagReadbackV1 {
    version: u8,
    head: GovernanceDagHeadV1,
    path_before_target: Vec<GovernanceDagBlockV1>,
    target: StoredReputationGovernanceDagTargetBlockV1,
    path_after_target: Vec<GovernanceDagBlockV1>,
}

impl StoredReputationGovernanceDagReadbackV1 {
    fn from_readback(
        readback: &ReputationGovernanceDagReadbackV1,
        target_index: usize,
    ) -> Result<Self, ReputationRuntimeError> {
        let target = readback
            .inclusion_path
            .get(target_index)
            .ok_or(ReputationRuntimeError::InvalidGovernanceAcknowledgement)?;
        Ok(Self {
            version: readback.version,
            head: readback.head.clone(),
            path_before_target: readback.inclusion_path[..target_index].to_vec(),
            target: StoredReputationGovernanceDagTargetBlockV1::from_block(target),
            path_after_target: readback.inclusion_path[target_index + 1..].to_vec(),
        })
    }

    fn reconstruct_readback(
        &self,
        signed_result: &SignedReputationSnapshotV1,
    ) -> Result<ReputationGovernanceDagReadbackV1, ReputationRuntimeError> {
        let path_len = self
            .path_before_target
            .len()
            .checked_add(1)
            .and_then(|len| len.checked_add(self.path_after_target.len()))
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        if path_len > REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1 {
            return Err(ReputationRuntimeError::GovernanceReadbackPathTooLong {
                found: path_len,
                maximum: REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1,
            });
        }
        let mut inclusion_path = Vec::with_capacity(path_len);
        inclusion_path.extend(self.path_before_target.iter().cloned());
        inclusion_path.push(self.target.reconstruct_block(signed_result));
        inclusion_path.extend(self.path_after_target.iter().cloned());
        Ok(ReputationGovernanceDagReadbackV1 {
            version: self.version,
            head: self.head.clone(),
            inclusion_path,
        })
    }
}

/// Strict external publication contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationPublicationPolicyV1 {
    trust_policy_digest: [u8; 32],
    threshold_signer_handle: String,
    threshold_signer_qualification: ReputationRuntimeProviderQualificationV1,
    governance_dag_handle: String,
    governance_dag_qualification: ReputationRuntimeProviderQualificationV1,
    governance_publisher_peer_id: Vec<u8>,
    governance_publisher_public_key: [u8; 32],
    governance_publisher_key_digest: [u8; 32],
    checkpoint_max_bytes: u64,
}

impl ReputationPublicationPolicyV1 {
    /// Construct a policy that pins public signer policy, external dependency
    /// identities, and the Governance DAG publisher.
    ///
    /// # Errors
    ///
    /// Rejects invalid trust material, handles, peer identity, key, or storage
    /// bound.
    pub fn try_new(
        trust_policy: &ReputationSnapshotTrustPolicyV1,
        threshold_signer_handle: impl Into<String>,
        governance_dag_handle: impl Into<String>,
        governance_publisher_peer_id: Vec<u8>,
        governance_publisher_public_key: [u8; 32],
        checkpoint_max_bytes: u64,
    ) -> Result<Self, ReputationRuntimeError> {
        trust_policy
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
        let trust_policy_digest = trust_policy
            .canonical_digest()
            .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
        let threshold_signer_handle = validate_runtime_handle(threshold_signer_handle.into())?;
        let governance_dag_handle = validate_runtime_handle(governance_dag_handle.into())?;
        if governance_publisher_peer_id.is_empty()
            || governance_publisher_peer_id.len()
                > REPUTATION_RUNTIME_MAX_GOVERNANCE_PEER_ID_BYTES_V1
            || governance_publisher_public_key == [0; 32]
            || !valid_ed25519_verifying_key(governance_publisher_public_key)
            || !(REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1
                ..=REPUTATION_PUBLICATION_MAX_CHECKPOINT_BYTES_V1)
                .contains(&checkpoint_max_bytes)
        {
            return Err(ReputationRuntimeError::InvalidRuntimePolicy);
        }
        let governance_publisher_key_digest = domain_digest(
            b"sorafs-reputation-governance-publisher-key-v1",
            &governance_publisher_public_key,
        )?;
        let governance_dag_policy_digest = reputation_governance_dag_policy_digest_v1(
            &governance_publisher_peer_id,
            governance_publisher_public_key,
        )?;
        Ok(Self {
            trust_policy_digest,
            threshold_signer_handle,
            threshold_signer_qualification: ReputationRuntimeProviderQualificationV1::new(
                REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
                trust_policy_digest,
            ),
            governance_dag_handle,
            governance_dag_qualification: ReputationRuntimeProviderQualificationV1::new(
                REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
                governance_dag_policy_digest,
            ),
            governance_publisher_peer_id,
            governance_publisher_public_key,
            governance_publisher_key_digest,
            checkpoint_max_bytes,
        })
    }

    /// Return the independently governed threshold-signer qualification.
    #[must_use]
    pub const fn threshold_signer_qualification(&self) -> ReputationRuntimeProviderQualificationV1 {
        self.threshold_signer_qualification
    }

    /// Return the independently governed Governance DAG qualification.
    #[must_use]
    pub const fn governance_dag_qualification(&self) -> ReputationRuntimeProviderQualificationV1 {
        self.governance_dag_qualification
    }

    /// Qualify or revalidate the external threshold signer.
    ///
    /// # Errors
    ///
    /// Rejects a signer whose handle, revision, or trust-policy digest differs
    /// from the publication policy.
    pub fn revalidate_threshold_signer(
        &self,
        signer: &dyn ReputationThresholdSignerClientV1,
    ) -> Result<(), ReputationRuntimeError> {
        assert_runtime_provider_qualification(
            &self.threshold_signer_handle,
            self.threshold_signer_qualification,
            signer,
        )
    }

    /// Qualify or revalidate authenticated Governance DAG publication/readback.
    ///
    /// # Errors
    ///
    /// Rejects a client whose handle, revision, or governed publisher
    /// peer-and-key digest differs from the publication policy.
    pub fn revalidate_governance_dag(
        &self,
        governance_dag: &dyn ReputationGovernanceDagClientV1,
    ) -> Result<(), ReputationRuntimeError> {
        assert_runtime_provider_qualification(
            &self.governance_dag_handle,
            self.governance_dag_qualification,
            governance_dag,
        )
    }

    fn digest(&self) -> Result<[u8; 32], ReputationRuntimeError> {
        hash_canonical(
            b"sorafs-reputation-publication-policy-v1",
            &ReputationPublicationPolicyDigestMaterialV1 {
                trust_policy_digest: self.trust_policy_digest,
                threshold_signer_handle_digest: domain_digest(
                    b"sorafs-reputation-runtime-handle-v1",
                    self.threshold_signer_handle.as_bytes(),
                )?,
                governance_dag_handle_digest: domain_digest(
                    b"sorafs-reputation-runtime-handle-v1",
                    self.governance_dag_handle.as_bytes(),
                )?,
                governance_publisher_peer_id: self.governance_publisher_peer_id.clone(),
                governance_publisher_public_key: self.governance_publisher_public_key,
                checkpoint_max_bytes: self.checkpoint_max_bytes,
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReputationPublicationPolicyDigestMaterialV1 {
    trust_policy_digest: [u8; 32],
    threshold_signer_handle_digest: [u8; 32],
    governance_dag_handle_digest: [u8; 32],
    governance_publisher_peer_id: Vec<u8>,
    governance_publisher_public_key: [u8; 32],
    checkpoint_max_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationPublicationV1 {
    sequence: u64,
    material_digest: [u8; 32],
    signed_result_digest: [u8; 32],
    signed_result: SignedReputationSnapshotV1,
    governance_acknowledgement: Option<ReputationGovernanceDagAcknowledgementV1>,
    governance_readback: Option<StoredReputationGovernanceDagReadbackV1>,
}

/// Exact threshold-signed snapshot proven present in the authoritative
/// Governance DAG.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ReputationCommittedSnapshotV1 {
    /// Stable projector delivery sequence.
    pub sequence: u64,
    /// Digest of the exact unsigned projector material.
    pub material_digest: [u8; 32],
    /// Digest of the exact canonical signed snapshot.
    pub signed_result_digest: [u8; 32],
    /// Exact threshold-signed snapshot and deterministic scoring evidence.
    pub signed_result: SignedReputationSnapshotV1,
    /// Authenticated Governance DAG readback acknowledgement.
    pub governance_acknowledgement: ReputationGovernanceDagAcknowledgementV1,
}

impl ReputationCommittedSnapshotV1 {
    fn from_pending(
        pending: &StoredReputationPublicationV1,
        acknowledgement: ReputationGovernanceDagAcknowledgementV1,
    ) -> Result<Self, ReputationRuntimeError> {
        if pending.governance_acknowledgement != Some(acknowledgement) {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        Ok(Self {
            sequence: pending.sequence,
            material_digest: pending.material_digest,
            signed_result_digest: pending.signed_result_digest,
            signed_result: pending.signed_result.clone(),
            governance_acknowledgement: acknowledgement,
        })
    }

    fn validate(
        &self,
        policy: &ReputationPublicationPolicyV1,
        trust_policy: &ReputationSnapshotTrustPolicyV1,
        governance_readback: &StoredReputationGovernanceDagReadbackV1,
        previous_readback: Option<&ReputationGovernanceDagReadbackV1>,
    ) -> Result<ReputationGovernanceDagReadbackV1, ReputationRuntimeError> {
        verify_persisted_signed_result(&self.signed_result, trust_policy)?;
        if self.sequence == 0
            || self.material_digest == [0; 32]
            || self.signed_result_digest == [0; 32]
            || self.signed_result.policy_digest != policy.trust_policy_digest
            || signed_result_digest(&self.signed_result)? != self.signed_result_digest
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        let readback = governance_readback
            .reconstruct_readback(&self.signed_result)
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        let (expected_acknowledgement, _) = governance_acknowledgement_from_readback(
            policy,
            self.sequence,
            self.material_digest,
            self.signed_result_digest,
            &self.signed_result,
            &readback,
            previous_readback,
        )
        .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if expected_acknowledgement != self.governance_acknowledgement {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        Ok(readback)
    }
}

/// Bounded durable public projection of authoritative reputation publication.
///
/// `latest` retains the exact signed snapshot and authenticated Governance DAG
/// acknowledgement. `events` is a consecutive retained suffix derived from
/// committed snapshots only; it never records signer submission or
/// submitter-side publication success.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ReputationCommittedReadProjectionV1 {
    /// Schema version.
    pub version: u8,
    /// Digest of the publication policy that owns this projection.
    pub publication_policy_digest: [u8; 32],
    /// Latest authoritative signed snapshot, when publication completed.
    pub latest: Option<ReputationCommittedSnapshotV1>,
    /// Bounded consecutive committed-event suffix in ascending sequence order.
    pub events: Vec<ReputationSnapshotEventV1>,
}

impl ReputationCommittedReadProjectionV1 {
    const fn empty(publication_policy_digest: [u8; 32]) -> Self {
        Self {
            version: REPUTATION_COMMITTED_READ_PROJECTION_VERSION_V1,
            publication_policy_digest,
            latest: None,
            events: Vec::new(),
        }
    }

    fn append(
        &mut self,
        committed: ReputationCommittedSnapshotV1,
        policy: &ReputationPublicationPolicyV1,
        trust_policy: &ReputationSnapshotTrustPolicyV1,
        governance_readback: &StoredReputationGovernanceDagReadbackV1,
        previous_readback: Option<&ReputationGovernanceDagReadbackV1>,
    ) -> Result<bool, ReputationRuntimeError> {
        if let Some(existing) = &self.latest {
            if existing == &committed {
                return Ok(false);
            }
        }
        committed.validate(policy, trust_policy, governance_readback, previous_readback)?;
        if self
            .events
            .iter()
            .any(|event| event.snapshot_id == committed.signed_result.snapshot.snapshot_id)
        {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        let sequence = match self.events.last() {
            Some(event) => event
                .sequence
                .checked_add(1)
                .ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?,
            None => 1,
        };
        let event =
            ReputationSnapshotEventV1::from_snapshot(sequence, &committed.signed_result.snapshot)
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if let Some(previous) = self.events.last()
            && (event.previous_snapshot_id != Some(previous.snapshot_id)
                || event.generated_at_unix <= previous.generated_at_unix)
        {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        self.events.push(event);
        self.latest = Some(committed);
        Ok(true)
    }

    fn validate(&self, policy_digest: [u8; 32]) -> Result<(), ReputationRuntimeError> {
        if self.version != REPUTATION_COMMITTED_READ_PROJECTION_VERSION_V1
            || self.publication_policy_digest != policy_digest
            || self.events.len() > REPUTATION_COMMITTED_READ_MAX_EVENTS_V1
            || self.latest.is_none() != self.events.is_empty()
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        let mut previous: Option<&ReputationSnapshotEventV1> = None;
        for event in &self.events {
            event
                .validate()
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
            if let Some(previous) = previous
                && (previous.sequence.checked_add(1) != Some(event.sequence)
                    || event.previous_snapshot_id != Some(previous.snapshot_id)
                    || event.generated_at_unix <= previous.generated_at_unix)
            {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
            previous = Some(event);
        }
        if let Some(latest) = &self.latest {
            let latest_event = self
                .events
                .last()
                .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
            let expected_event = ReputationSnapshotEventV1::from_snapshot(
                latest_event.sequence,
                &latest.signed_result.snapshot,
            )
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
            if latest_event != &expected_event {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
        }
        Ok(())
    }
}

/// Object-safe read boundary for the authoritative reputation projection.
///
/// Implementations must return only durable snapshots that completed
/// authenticated Governance DAG readback. The boundary intentionally exposes
/// no mutation or signing operation.
pub trait ReputationCommittedReadApiV1: Send + Sync + fmt::Debug {
    /// Return an exact clone of the durable committed projection.
    ///
    /// # Errors
    ///
    /// Returns a durable-state or runtime-lock error.
    fn committed_read_projection(
        &self,
    ) -> Result<ReputationCommittedReadProjectionV1, ReputationRuntimeError>;

    /// Return the exact retained authoritative snapshot identified by
    /// `snapshot_id`.
    ///
    /// Unknown and evicted identifiers return `None`; implementations must not
    /// substitute the latest snapshot.
    fn committed_snapshot_by_id(
        &self,
        snapshot_id: [u8; 16],
    ) -> Result<Option<ReputationSnapshotV1>, ReputationRuntimeError> {
        Ok(self
            .committed_read_projection()?
            .latest
            .filter(|committed| committed.signed_result.snapshot.snapshot_id == snapshot_id)
            .map(|committed| committed.signed_result.snapshot))
    }

    /// Return the retained committed events strictly after `sequence`.
    ///
    /// Implementations should override this method when they can clone the
    /// small event suffix without cloning the full provider projection.
    fn committed_events_after(
        &self,
        sequence: u64,
    ) -> Result<Vec<ReputationSnapshotEventV1>, ReputationRuntimeError> {
        Ok(self
            .committed_read_projection()?
            .events
            .into_iter()
            .filter(|event| event.sequence > sequence)
            .collect())
    }
}

/// Activation state of the durable native-outcome admission runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationNativeOutcomeAdmissionStateV1 {
    /// The configured runtime is waiting for its deployment-owned dependencies.
    Deferred,
    /// The runtime is active and every admission must succeed or fail closed.
    Active,
}

/// Object-safe durable admission boundary for native reputation outcomes.
///
/// Implementations must commit the native journal producer checkpoint before
/// returning success. Repeating the exact provider and typed source must return
/// [`ReputationJournalEnqueueOutcomeV1::ExactReplay`]; substituting payload
/// material for the same native source must fail closed.
pub trait ReputationNativeOutcomeAdmissionApiV1: Send + Sync + fmt::Debug {
    /// Return whether the configured runtime has completed activation.
    ///
    /// A supervised caller may idle while this reports
    /// [`ReputationNativeOutcomeAdmissionStateV1::Deferred`].
    /// Once active, dependency substitution, staleness, or unavailability must
    /// be returned as an error rather than downgraded to deferred.
    ///
    /// # Errors
    ///
    /// Returns a runtime-state error when activation state cannot be read.
    fn activation_state(
        &self,
    ) -> Result<ReputationNativeOutcomeAdmissionStateV1, ReputationRuntimeError>;

    /// Durably admit one retained PoR terminal.
    ///
    /// # Errors
    ///
    /// Returns a validation, source-conflict, runtime-binding, or durable
    /// checkpoint error. No success may be reported for best-effort admission.
    fn record_por_terminal(
        &self,
        provider_id: ProviderId,
        outcome: PorTerminalOutcomeV1,
    ) -> Result<ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReputationPublicationCheckpointV1 {
    version: u8,
    policy_digest: [u8; 32],
    pending: Option<StoredReputationPublicationV1>,
    committed_read: ReputationCommittedReadProjectionV1,
    committed_snapshots: Vec<ReputationCommittedSnapshotV1>,
    committed_governance_readbacks: Vec<StoredReputationGovernanceDagReadbackV1>,
}

impl ReputationPublicationCheckpointV1 {
    const fn empty(policy_digest: [u8; 32]) -> Self {
        Self {
            version: REPUTATION_PUBLICATION_CHECKPOINT_VERSION_V1,
            policy_digest,
            pending: None,
            committed_read: ReputationCommittedReadProjectionV1::empty(policy_digest),
            committed_snapshots: Vec::new(),
            committed_governance_readbacks: Vec::new(),
        }
    }

    fn commit_authoritative(
        &mut self,
        committed: ReputationCommittedSnapshotV1,
        policy: &ReputationPublicationPolicyV1,
        trust_policy: &ReputationSnapshotTrustPolicyV1,
        governance_readback: StoredReputationGovernanceDagReadbackV1,
    ) -> Result<(), ReputationRuntimeError> {
        self.commit_authoritative_with_retention_limit(
            committed,
            policy,
            trust_policy,
            governance_readback,
            REPUTATION_COMMITTED_READ_MAX_EVENTS_V1,
        )
    }

    fn commit_authoritative_with_retention_limit(
        &mut self,
        committed: ReputationCommittedSnapshotV1,
        policy: &ReputationPublicationPolicyV1,
        trust_policy: &ReputationSnapshotTrustPolicyV1,
        governance_readback: StoredReputationGovernanceDagReadbackV1,
        retention_limit: usize,
    ) -> Result<(), ReputationRuntimeError> {
        if retention_limit == 0 || retention_limit > REPUTATION_COMMITTED_READ_MAX_EVENTS_V1 {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        let previous_readback = self
            .committed_snapshots
            .last()
            .zip(self.committed_governance_readbacks.last())
            .map(|(previous, readback)| readback.reconstruct_readback(&previous.signed_result))
            .transpose()?;
        let appended = self.committed_read.append(
            committed.clone(),
            policy,
            trust_policy,
            &governance_readback,
            previous_readback.as_ref(),
        )?;
        if !appended {
            return if self.committed_snapshots.last() == Some(&committed)
                && self.committed_governance_readbacks.last() == Some(&governance_readback)
            {
                Ok(())
            } else {
                Err(ReputationRuntimeError::PublicationCheckpointConflict)
            };
        }
        self.committed_snapshots.push(committed);
        self.committed_governance_readbacks
            .push(governance_readback);
        let overflow = self
            .committed_snapshots
            .len()
            .saturating_sub(retention_limit);
        if overflow != 0 {
            self.committed_snapshots.drain(..overflow);
            self.committed_governance_readbacks.drain(..overflow);
            self.committed_read.events.drain(..overflow);
        }
        Ok(())
    }

    fn evict_oldest_committed(&mut self) -> bool {
        if self.committed_snapshots.len() <= 1 {
            return false;
        }
        self.committed_snapshots.drain(..1);
        self.committed_governance_readbacks.drain(..1);
        self.committed_read.events.drain(..1);
        true
    }
}

#[derive(Debug)]
struct PublicationRuntimeState {
    checkpoint: ReputationPublicationCheckpointV1,
    fingerprint: Option<[u8; 32]>,
}

/// Result of one external signer/Governance DAG reconciliation tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationPublicationOutcomeV1 {
    /// The projector has not enqueued release material.
    Idle,
    /// The exact request is pending at the external threshold signer.
    AwaitingThresholdSignature,
    /// A verified signed result is durable and its DAG acknowledgement is pending.
    AwaitingGovernanceDag,
    /// Governance readback was durable and projector acknowledgement completed.
    Acknowledged,
    /// The exact completed acknowledgement was replayed.
    ExactReplay,
    /// The projector exhausted its external-delivery retry budget.
    DeadLetter,
}

/// Payload-free publication-reconciler status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationPublicationStatusV1 {
    /// Durable projector delivery state, when signing material is outstanding.
    pub delivery_state: Option<ReputationUnsignedMaterialDeliveryStateV1>,
    /// Distinct payload-free external failures retained by the projector.
    pub failed_attempts: u32,
    /// Whether a verified signed result is durably staged.
    pub signed_result_staged: bool,
    /// Whether authenticated Governance DAG readback is durable.
    pub governance_acknowledged: bool,
    /// Whether the projector and publication checkpoint both acknowledge the result.
    pub complete: bool,
    /// Signed result digest, when staged or complete.
    pub signed_result_digest: Option<[u8; 32]>,
}

/// Durable coordinator for external threshold signing and Governance DAG readback.
#[derive(Debug)]
pub struct ReputationPublicationReconcilerV1 {
    projector: Arc<ReputationIngestService>,
    trust_policy: ReputationSnapshotTrustPolicyV1,
    policy: ReputationPublicationPolicyV1,
    policy_digest: [u8; 32],
    threshold_signer: Arc<dyn ReputationThresholdSignerClientV1>,
    governance_dag: Arc<dyn ReputationGovernanceDagClientV1>,
    store: AtomicCheckpointStore,
    state: Mutex<PublicationRuntimeState>,
    reconcile_lock: Mutex<()>,
    durability_poisoned: AtomicBool,
}

impl ReputationPublicationReconcilerV1 {
    /// Open the publication checkpoint and bind every external identity.
    ///
    /// # Errors
    ///
    /// Fails before any external call for trust-policy substitution, handle
    /// mismatch, unsafe storage, or inconsistent durable state.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        root: &Path,
        projector: Arc<ReputationIngestService>,
        trust_policy: ReputationSnapshotTrustPolicyV1,
        policy: ReputationPublicationPolicyV1,
        threshold_signer: Arc<dyn ReputationThresholdSignerClientV1>,
        governance_dag: Arc<dyn ReputationGovernanceDagClientV1>,
    ) -> Result<Self, ReputationRuntimeError> {
        trust_policy
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
        if trust_policy.canonical_digest().ok() != Some(policy.trust_policy_digest)
            || projector.policy.snapshot_trust_policy_digest != policy.trust_policy_digest
        {
            return Err(ReputationRuntimeError::RuntimeBindingMismatch);
        }
        qualify_runtime_provider(
            &policy.threshold_signer_handle,
            policy.threshold_signer_qualification,
            threshold_signer.as_ref(),
        )?;
        qualify_runtime_provider(
            &policy.governance_dag_handle,
            policy.governance_dag_qualification,
            governance_dag.as_ref(),
        )?;
        let policy_digest = policy.digest()?;
        let store = AtomicCheckpointStore::new(
            root,
            REPUTATION_PUBLICATION_CHECKPOINT_FILE_NAME_V1,
            REPUTATION_PUBLICATION_LOCK_FILE_NAME_V1,
            policy.checkpoint_max_bytes,
        )?;
        let (bytes, fingerprint) = store.load_bytes()?;
        let checkpoint = match bytes {
            Some(bytes) => {
                decode_publication_checkpoint(&bytes, &policy, policy_digest, &trust_policy)?
            }
            None => ReputationPublicationCheckpointV1::empty(policy_digest),
        };
        Ok(Self {
            projector,
            trust_policy,
            policy,
            policy_digest,
            threshold_signer,
            governance_dag,
            store,
            state: Mutex::new(PublicationRuntimeState {
                checkpoint,
                fingerprint,
            }),
            reconcile_lock: Mutex::new(()),
            durability_poisoned: AtomicBool::new(false),
        })
    }

    /// Reconcile one material item through signing, signed DAG readback, and
    /// projector acknowledgement.
    ///
    /// The signed result is committed before publication. The signed DAG
    /// acknowledgement is committed before the projector outbox is removed.
    /// This ordering makes every crash point safely replayable.
    ///
    /// # Errors
    ///
    /// Rejects dependency identity changes, signer substitution, mismatched
    /// material, forged/wrong-publisher DAG blocks, durable conflicts, and
    /// persistence uncertainty.
    pub fn reconcile_once(&self) -> Result<ReputationPublicationOutcomeV1, ReputationRuntimeError> {
        let _guard = self
            .reconcile_lock
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        self.ensure_publication_durable()?;
        self.check_readiness()?;

        if let Some(projector_ack) = self.projector.unsigned_material_acknowledgement()? {
            return self.reconcile_completed_projector_ack(projector_ack);
        }
        let Some(delivery) = self.projector.unsigned_material_delivery()? else {
            return Ok(ReputationPublicationOutcomeV1::Idle);
        };
        if delivery.state == ReputationUnsignedMaterialDeliveryStateV1::DeadLetter {
            return Ok(ReputationPublicationOutcomeV1::DeadLetter);
        }
        self.validate_delivery_policy(&delivery)?;

        if self.pending_publication()?.is_none() {
            let request = threshold_signing_request(&delivery)?;
            self.ensure_threshold_signer_binding()?;
            let signed_result = self.threshold_signer.reconcile_signature(&request);
            self.ensure_threshold_signer_binding()?;
            let signed = match signed_result {
                Ok(Some(signed)) => signed,
                Ok(None) => return Ok(ReputationPublicationOutcomeV1::AwaitingThresholdSignature),
                Err(failure) => {
                    return self.record_external_failure(
                        &delivery,
                        request.idempotency_key,
                        failure,
                    );
                }
            };
            self.validate_signed_result(&delivery, &signed)?;
            self.store_signed_result(&delivery, signed)?;
        }

        let pending = self
            .pending_publication()?
            .ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?;
        if pending.sequence != delivery.sequence
            || pending.material_digest != delivery.material_digest
        {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        self.validate_signed_result(&delivery, &pending.signed_result)?;
        if pending.governance_acknowledgement.is_none() {
            let request = governance_publication_request(&pending)?;
            let readback_result = self.reconcile_governance_publication(&request)?;
            let readback = match readback_result {
                Ok(Some(readback)) => readback,
                Ok(None) => return Ok(ReputationPublicationOutcomeV1::AwaitingGovernanceDag),
                Err(failure) => {
                    return self.record_external_failure(
                        &delivery,
                        request.idempotency_key,
                        failure,
                    );
                }
            };
            let (acknowledgement, stored_readback) =
                self.validate_governance_readback(&pending, &readback)?;
            self.store_governance_readback(acknowledgement, stored_readback)?;
        }

        let pending = self
            .pending_publication()?
            .ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?;
        let acknowledgement = pending
            .governance_acknowledgement
            .ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?;
        self.projector.acknowledge_unsigned_material(
            pending.sequence,
            pending.material_digest,
            &pending.signed_result,
            &self.trust_policy,
        )?;
        self.complete_publication(acknowledgement)?;
        Ok(ReputationPublicationOutcomeV1::Acknowledged)
    }

    /// Return payload-free durable reconciliation status.
    ///
    /// # Errors
    ///
    /// Returns a durable-state or runtime-lock error.
    pub fn status(&self) -> Result<ReputationPublicationStatusV1, ReputationRuntimeError> {
        self.ensure_publication_durable()?;
        let delivery = self.projector.unsigned_material_delivery()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let pending = state.checkpoint.pending.as_ref();
        let completed = state.checkpoint.committed_read.latest.as_ref();
        Ok(ReputationPublicationStatusV1 {
            delivery_state: delivery.as_ref().map(|entry| entry.state),
            failed_attempts: delivery.map_or(0, |entry| entry.failed_attempts),
            signed_result_staged: pending.is_some(),
            governance_acknowledged: pending.map_or(completed.is_some(), |entry| {
                entry.governance_acknowledgement.is_some()
            }),
            complete: pending.is_none() && completed.is_some(),
            signed_result_digest: pending
                .map(|entry| entry.signed_result_digest)
                .or_else(|| completed.map(|entry| entry.signed_result_digest)),
        })
    }

    /// Return an exact clone of the durable authoritative read projection.
    ///
    /// The projection becomes visible only after authenticated Governance DAG
    /// readback and projector acknowledgement have both completed.
    ///
    /// # Errors
    ///
    /// Returns a durable-state or runtime-lock error.
    pub fn committed_read_projection(
        &self,
    ) -> Result<ReputationCommittedReadProjectionV1, ReputationRuntimeError> {
        self.ensure_publication_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        Ok(state.checkpoint.committed_read.clone())
    }

    /// Return one exact retained authoritative snapshot by its identifier.
    ///
    /// Unknown and evicted identifiers return `None`; the latest snapshot is
    /// never substituted for a miss.
    pub fn committed_snapshot_by_id(
        &self,
        snapshot_id: [u8; 16],
    ) -> Result<Option<ReputationSnapshotV1>, ReputationRuntimeError> {
        self.ensure_publication_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        Ok(state
            .checkpoint
            .committed_snapshots
            .iter()
            .find(|committed| committed.signed_result.snapshot.snapshot_id == snapshot_id)
            .map(|committed| committed.signed_result.snapshot.clone()))
    }

    /// Return only the bounded committed-event suffix after `sequence`.
    ///
    /// This avoids cloning the signed provider projection for every live-stream
    /// poll.
    pub fn committed_events_after(
        &self,
        sequence: u64,
    ) -> Result<Vec<ReputationSnapshotEventV1>, ReputationRuntimeError> {
        self.ensure_publication_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        Ok(state
            .checkpoint
            .committed_read
            .events
            .iter()
            .filter(|event| event.sequence > sequence)
            .cloned()
            .collect())
    }

    fn validate_delivery_policy(
        &self,
        delivery: &ReputationUnsignedMaterialDeliveryV1,
    ) -> Result<(), ReputationRuntimeError> {
        if delivery.sequence == 0
            || delivery.material_digest == [0; 32]
            || delivery.material.snapshot_trust_policy_digest != self.policy.trust_policy_digest
        {
            return Err(ReputationRuntimeError::PublicationMaterialMismatch);
        }
        Ok(())
    }

    fn validate_signed_result(
        &self,
        delivery: &ReputationUnsignedMaterialDeliveryV1,
        signed: &SignedReputationSnapshotV1,
    ) -> Result<(), ReputationRuntimeError> {
        let admitted_at_unix = delivery
            .material
            .target_finalized_at_unix_ms
            .checked_div(1_000)
            .filter(|value| *value != 0)
            .ok_or(ReputationRuntimeError::PublicationMaterialMismatch)?;
        signed
            .verify(&self.trust_policy, admitted_at_unix)
            .map_err(|_| ReputationRuntimeError::InvalidThresholdResult)?;
        if signed.policy_digest != delivery.material.snapshot_trust_policy_digest
            || signed.snapshot != delivery.material.snapshot
            || signed.scoring_evidence != delivery.material.scoring_evidence
            || signed.scoring_evidence_digest != delivery.material.scoring_evidence_digest
            || signed.signing_digest().ok() != Some(delivery.material.snapshot_signing_digest)
        {
            return Err(ReputationRuntimeError::PublicationMaterialMismatch);
        }
        Ok(())
    }

    fn reconcile_governance_publication(
        &self,
        request: &ReputationGovernanceDagPublicationRequestV1,
    ) -> Result<
        Result<Option<ReputationGovernanceDagReadbackV1>, ReputationExternalFailureV1>,
        ReputationRuntimeError,
    > {
        self.ensure_governance_dag_binding()?;
        let result = self.governance_dag.reconcile_publication(request);
        self.ensure_governance_dag_binding()?;
        Ok(result)
    }

    fn validate_governance_readback(
        &self,
        pending: &StoredReputationPublicationV1,
        readback: &ReputationGovernanceDagReadbackV1,
    ) -> Result<
        (
            ReputationGovernanceDagAcknowledgementV1,
            StoredReputationGovernanceDagReadbackV1,
        ),
        ReputationRuntimeError,
    > {
        let previous_readback = {
            let state = self
                .state
                .lock()
                .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
            state
                .checkpoint
                .committed_snapshots
                .last()
                .zip(state.checkpoint.committed_governance_readbacks.last())
                .map(|(committed, stored)| stored.reconstruct_readback(&committed.signed_result))
                .transpose()?
        };
        let (acknowledgement, target_index) = governance_acknowledgement_from_readback(
            &self.policy,
            pending.sequence,
            pending.material_digest,
            pending.signed_result_digest,
            &pending.signed_result,
            readback,
            previous_readback.as_ref(),
        )?;
        Ok((
            acknowledgement,
            StoredReputationGovernanceDagReadbackV1::from_readback(readback, target_index)?,
        ))
    }

    fn store_signed_result(
        &self,
        delivery: &ReputationUnsignedMaterialDeliveryV1,
        signed_result: SignedReputationSnapshotV1,
    ) -> Result<(), ReputationRuntimeError> {
        let signed_result_digest = signed_result_digest(&signed_result)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        if let Some(existing) = &state.checkpoint.pending {
            if existing.sequence == delivery.sequence
                && existing.material_digest == delivery.material_digest
                && existing.signed_result_digest == signed_result_digest
                && existing.signed_result == signed_result
            {
                return Ok(());
            }
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        if let Some(latest) = state.checkpoint.committed_read.latest.as_ref() {
            if signed_result.snapshot.previous_snapshot_id
                != Some(latest.signed_result.snapshot.snapshot_id)
                || signed_result.snapshot.generated_at_unix
                    <= latest.signed_result.snapshot.generated_at_unix
                || state
                    .checkpoint
                    .committed_snapshots
                    .iter()
                    .any(|committed| {
                        committed.signed_result.snapshot.snapshot_id
                            == signed_result.snapshot.snapshot_id
                    })
            {
                return Err(ReputationRuntimeError::PublicationCheckpointConflict);
            }
        } else if signed_result.snapshot.previous_snapshot_id.is_some() {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        let mut candidate = state.checkpoint.clone();
        candidate.pending = Some(StoredReputationPublicationV1 {
            sequence: delivery.sequence,
            material_digest: delivery.material_digest,
            signed_result_digest,
            signed_result,
            governance_acknowledgement: None,
            governance_readback: None,
        });
        self.commit_publication_candidate(&mut state, candidate)
    }

    /// Verify both authenticated external publication dependencies.
    ///
    /// The identity check is repeated after the probes so an adapter cannot
    /// swap its governed binding during readiness validation.
    pub fn check_readiness(&self) -> Result<(), ReputationRuntimeError> {
        self.ensure_external_bindings()
    }

    fn store_governance_readback(
        &self,
        acknowledgement: ReputationGovernanceDagAcknowledgementV1,
        governance_readback: StoredReputationGovernanceDagReadbackV1,
    ) -> Result<(), ReputationRuntimeError> {
        acknowledgement.validate()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let pending = state
            .checkpoint
            .pending
            .as_ref()
            .ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?;
        if pending.sequence != acknowledgement.sequence
            || pending.material_digest != acknowledgement.material_digest
            || pending.signed_result_digest != acknowledgement.signed_result_digest
        {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        if let (Some(existing_acknowledgement), Some(existing_readback)) = (
            pending.governance_acknowledgement,
            pending.governance_readback.as_ref(),
        ) {
            return if existing_acknowledgement == acknowledgement
                && existing_readback == &governance_readback
            {
                Ok(())
            } else {
                Err(ReputationRuntimeError::PublicationCheckpointConflict)
            };
        }
        if pending.governance_acknowledgement.is_some() || pending.governance_readback.is_some() {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        let mut candidate = state.checkpoint.clone();
        let candidate_pending = candidate
            .pending
            .as_mut()
            .ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?;
        candidate_pending.governance_acknowledgement = Some(acknowledgement);
        candidate_pending.governance_readback = Some(governance_readback);
        self.commit_publication_candidate(&mut state, candidate)
    }

    fn complete_publication(
        &self,
        acknowledgement: ReputationGovernanceDagAcknowledgementV1,
    ) -> Result<(), ReputationRuntimeError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        if state.checkpoint.pending.is_none() {
            return if state
                .checkpoint
                .committed_snapshots
                .iter()
                .any(|existing| existing.governance_acknowledgement == acknowledgement)
            {
                Ok(())
            } else {
                Err(ReputationRuntimeError::PublicationCheckpointConflict)
            };
        }
        let pending = state
            .checkpoint
            .pending
            .as_ref()
            .ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?;
        if pending.governance_acknowledgement != Some(acknowledgement) {
            return Err(ReputationRuntimeError::PublicationCheckpointConflict);
        }
        let governance_readback = pending
            .governance_readback
            .clone()
            .ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?;
        let committed = ReputationCommittedSnapshotV1::from_pending(pending, acknowledgement)?;
        let mut candidate = state.checkpoint.clone();
        candidate.pending = None;
        candidate.commit_authoritative(
            committed,
            &self.policy,
            &self.trust_policy,
            governance_readback,
        )?;
        self.commit_publication_candidate(&mut state, candidate)
    }

    fn reconcile_completed_projector_ack(
        &self,
        projector_ack: super::ReputationUnsignedMaterialAcknowledgementV1,
    ) -> Result<ReputationPublicationOutcomeV1, ReputationRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        if state
            .checkpoint
            .committed_snapshots
            .iter()
            .any(|completed| {
                completed.sequence == projector_ack.sequence
                    && completed.material_digest == projector_ack.material_digest
                    && completed.signed_result_digest == projector_ack.signed_result_digest
            })
        {
            return Ok(ReputationPublicationOutcomeV1::ExactReplay);
        }
        let recover = state
            .checkpoint
            .pending
            .as_ref()
            .and_then(|pending| pending.governance_acknowledgement)
            .filter(|ack| {
                ack.sequence == projector_ack.sequence
                    && ack.material_digest == projector_ack.material_digest
                    && ack.signed_result_digest == projector_ack.signed_result_digest
            });
        drop(state);
        let acknowledgement =
            recover.ok_or(ReputationRuntimeError::PublicationCheckpointConflict)?;
        self.complete_publication(acknowledgement)?;
        Ok(ReputationPublicationOutcomeV1::ExactReplay)
    }

    fn record_external_failure(
        &self,
        delivery: &ReputationUnsignedMaterialDeliveryV1,
        operation_key: [u8; 32],
        failure: ReputationExternalFailureV1,
    ) -> Result<ReputationPublicationOutcomeV1, ReputationRuntimeError> {
        let failure_receipt = external_failure_receipt(operation_key, failure.receipt())?;
        let outcome = self.projector.record_unsigned_material_delivery_failure(
            delivery.sequence,
            delivery.material_digest,
            failure_receipt,
        )?;
        if matches!(
            outcome,
            super::ReputationMaterialFailureOutcomeV1::DeadLettered { .. }
        ) {
            Ok(ReputationPublicationOutcomeV1::DeadLetter)
        } else {
            Err(ReputationRuntimeError::External(failure))
        }
    }

    fn pending_publication(
        &self,
    ) -> Result<Option<StoredReputationPublicationV1>, ReputationRuntimeError> {
        self.ensure_publication_durable()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        Ok(state.checkpoint.pending.clone())
    }

    fn ensure_external_bindings(&self) -> Result<(), ReputationRuntimeError> {
        self.ensure_threshold_signer_binding()?;
        self.ensure_governance_dag_binding()
    }

    fn ensure_threshold_signer_binding(&self) -> Result<(), ReputationRuntimeError> {
        self.policy
            .revalidate_threshold_signer(self.threshold_signer.as_ref())
    }

    fn ensure_governance_dag_binding(&self) -> Result<(), ReputationRuntimeError> {
        self.policy
            .revalidate_governance_dag(self.governance_dag.as_ref())
    }

    fn commit_publication_candidate(
        &self,
        state: &mut PublicationRuntimeState,
        candidate: ReputationPublicationCheckpointV1,
    ) -> Result<(), ReputationRuntimeError> {
        let (candidate, encoded) = encode_bounded_publication_checkpoint(
            candidate,
            &self.policy,
            self.policy_digest,
            &self.trust_policy,
            self.policy.checkpoint_max_bytes,
        )?;
        match self.store.commit_bytes(&encoded, state.fingerprint) {
            Ok(fingerprint) => {
                state.checkpoint = candidate;
                state.fingerprint = Some(fingerprint);
                Ok(())
            }
            Err(CheckpointStoreError::DurabilityUncertain) => {
                self.durability_poisoned.store(true, Ordering::Release);
                Err(ReputationRuntimeError::CheckpointDurabilityUncertain)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn ensure_publication_durable(&self) -> Result<(), ReputationRuntimeError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationRuntimeError::CheckpointDurabilityUncertain);
        }
        Ok(())
    }
}

fn encode_bounded_publication_checkpoint(
    mut candidate: ReputationPublicationCheckpointV1,
    policy: &ReputationPublicationPolicyV1,
    policy_digest: [u8; 32],
    trust_policy: &ReputationSnapshotTrustPolicyV1,
    checkpoint_max_bytes: u64,
) -> Result<(ReputationPublicationCheckpointV1, Vec<u8>), ReputationRuntimeError> {
    loop {
        validate_publication_checkpoint(&candidate, policy, policy_digest, trust_policy)?;
        let encoded =
            norito::to_bytes(&candidate).map_err(|_| ReputationRuntimeError::CanonicalEncoding)?;
        if u64::try_from(encoded.len()).unwrap_or(u64::MAX) <= checkpoint_max_bytes {
            return Ok((candidate, encoded));
        }
        if !candidate.evict_oldest_committed() {
            return Err(ReputationRuntimeError::CheckpointTooLarge);
        }
    }
}

fn external_failure_receipt(
    operation_key: [u8; 32],
    dependency_receipt: [u8; 32],
) -> Result<[u8; 32], ReputationRuntimeError> {
    if operation_key == [0; 32] || dependency_receipt == [0; 32] {
        return Err(ReputationRuntimeError::InvalidExternalReceipt);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs-reputation-external-failure-receipt-v1");
    hasher.update(&operation_key);
    hasher.update(&dependency_receipt);
    Ok(*hasher.finalize().as_bytes())
}

fn threshold_signing_request(
    delivery: &ReputationUnsignedMaterialDeliveryV1,
) -> Result<ReputationThresholdSigningRequestV1, ReputationRuntimeError> {
    let idempotency_key = publication_idempotency_key(
        b"sorafs-reputation-threshold-signing-operation-v1",
        delivery.sequence,
        delivery.material_digest,
        None,
    )?;
    Ok(ReputationThresholdSigningRequestV1 {
        sequence: delivery.sequence,
        material_digest: delivery.material_digest,
        idempotency_key,
        material: delivery.material.clone(),
    })
}

fn governance_publication_request(
    pending: &StoredReputationPublicationV1,
) -> Result<ReputationGovernanceDagPublicationRequestV1, ReputationRuntimeError> {
    let canonical_signed_result = pending
        .signed_result
        .canonical_bytes()
        .map_err(|_| ReputationRuntimeError::InvalidThresholdResult)?;
    let idempotency_key = publication_idempotency_key(
        b"sorafs-reputation-governance-publication-operation-v1",
        pending.sequence,
        pending.material_digest,
        Some(pending.signed_result_digest),
    )?;
    Ok(ReputationGovernanceDagPublicationRequestV1 {
        sequence: pending.sequence,
        material_digest: pending.material_digest,
        signed_result_digest: pending.signed_result_digest,
        idempotency_key,
        signed_result: pending.signed_result.clone(),
        canonical_signed_result,
    })
}

fn publication_idempotency_key(
    domain: &'static [u8],
    sequence: u64,
    material_digest: [u8; 32],
    signed_result_digest: Option<[u8; 32]>,
) -> Result<[u8; 32], ReputationRuntimeError> {
    if sequence == 0 || material_digest == [0; 32] {
        return Err(ReputationRuntimeError::PublicationMaterialMismatch);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&sequence.to_le_bytes());
    hasher.update(&material_digest);
    if let Some(digest) = signed_result_digest {
        if digest == [0; 32] {
            return Err(ReputationRuntimeError::PublicationMaterialMismatch);
        }
        hasher.update(&digest);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn signed_result_digest(
    signed_result: &SignedReputationSnapshotV1,
) -> Result<[u8; 32], ReputationRuntimeError> {
    let bytes = signed_result
        .canonical_bytes()
        .map_err(|_| ReputationRuntimeError::InvalidThresholdResult)?;
    domain_digest(b"sorafs-reputation-signed-material-result-v1", &bytes)
}

fn verify_persisted_signed_result(
    signed_result: &SignedReputationSnapshotV1,
    trust_policy: &ReputationSnapshotTrustPolicyV1,
) -> Result<(), ReputationRuntimeError> {
    let admitted_at_unix = signed_result.snapshot.generated_at_unix;
    signed_result
        .verify(trust_policy, admitted_at_unix)
        .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)
}

fn validate_governance_readback_envelope<'a>(
    policy: &ReputationPublicationPolicyV1,
    readback: &'a ReputationGovernanceDagReadbackV1,
) -> Result<(&'a GovernanceDagBlockV1, &'a GovernanceDagBlockV1), ReputationRuntimeError> {
    let path_len = readback.inclusion_path.len();
    if path_len > REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1 {
        return Err(ReputationRuntimeError::GovernanceReadbackPathTooLong {
            found: path_len,
            maximum: REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1,
        });
    }
    if path_len == 0 {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }
    if readback.version != REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1 {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }

    readback
        .head
        .validate()
        .map_err(|_| ReputationRuntimeError::InvalidGovernanceAcknowledgement)?;
    if readback.head.publisher_peer_id != policy.governance_publisher_peer_id
        || readback.head.head_signature.public_key != policy.governance_publisher_public_key
    {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }
    let maximum = u64::try_from(REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1)
        .map_err(|_| ReputationRuntimeError::InvalidGovernanceAcknowledgement)?;
    if (readback.head.block_count <= maximum && readback.head.checkpoint_cid.is_some())
        || (readback.head.block_count > maximum && readback.head.checkpoint_cid.is_none())
    {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }

    validate_governance_dag_chain_v1(
        &readback.inclusion_path,
        Some(readback.head.head_block_cid.as_slice()),
    )
    .map_err(|_| ReputationRuntimeError::InvalidGovernanceAcknowledgement)?;
    let first = readback
        .inclusion_path
        .first()
        .ok_or(ReputationRuntimeError::InvalidGovernanceAcknowledgement)?;
    let tip = readback
        .inclusion_path
        .last()
        .ok_or(ReputationRuntimeError::InvalidGovernanceAcknowledgement)?;
    if tip.sequence.checked_add(1) != Some(readback.head.block_count)
        || readback.head.generated_at < tip.timestamp
        || readback.inclusion_path.iter().any(|block| {
            block.publisher_peer_id != policy.governance_publisher_peer_id
                || block.node.publisher_peer_id != policy.governance_publisher_peer_id
                || block.block_signature.public_key != policy.governance_publisher_public_key
                || block.node.publisher_signature.public_key
                    != policy.governance_publisher_public_key
        })
    {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }
    if readback.head.block_count > maximum
        && first.sequence == readback.head.block_count - maximum
        && readback.head.checkpoint_cid.as_deref() != Some(first.block_cid.as_slice())
    {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }
    Ok((first, tip))
}

fn governance_acknowledgement_from_readback(
    policy: &ReputationPublicationPolicyV1,
    sequence: u64,
    material_digest: [u8; 32],
    expected_signed_result_digest: [u8; 32],
    signed_result: &SignedReputationSnapshotV1,
    readback: &ReputationGovernanceDagReadbackV1,
    previous_readback: Option<&ReputationGovernanceDagReadbackV1>,
) -> Result<(ReputationGovernanceDagAcknowledgementV1, usize), ReputationRuntimeError> {
    if sequence == 0
        || material_digest == [0; 32]
        || expected_signed_result_digest == [0; 32]
        || signed_result_digest(signed_result).ok() != Some(expected_signed_result_digest)
    {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }
    let (first, _) = validate_governance_readback_envelope(policy, readback)?;
    if let Some(previous_readback) = previous_readback {
        let (_, previous_tip) = validate_governance_readback_envelope(policy, previous_readback)?;
        if first.prev_block_cid.as_deref() != Some(previous_tip.block_cid.as_slice())
            || first.node.prev_cid.as_deref() != Some(previous_tip.node.node_cid.as_slice())
            || previous_tip.sequence.checked_add(1) != Some(first.sequence)
            || first.timestamp < previous_tip.timestamp
            || first.node.timestamp < previous_tip.node.timestamp
            || first.timestamp < previous_readback.head.generated_at
            || first.node.timestamp < previous_readback.head.generated_at
            || readback.head.block_count <= previous_readback.head.block_count
            || readback.head.generated_at < previous_readback.head.generated_at
        {
            return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
        }
    }

    let target_payload = GovernanceLogPayloadV1::SignedReputationSnapshot(signed_result.clone());
    let mut target_index = None;
    for (index, block) in readback.inclusion_path.iter().enumerate() {
        if block.node.payload == target_payload {
            if target_index.replace(index).is_some() {
                return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
            }
        }
    }
    let target_index =
        target_index.ok_or(ReputationRuntimeError::InvalidGovernanceAcknowledgement)?;
    let block = &readback.inclusion_path[target_index];
    if block.node.timestamp < signed_result.snapshot.generated_at_unix
        || block.timestamp < signed_result.snapshot.generated_at_unix
    {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }
    let acknowledgement = ReputationGovernanceDagAcknowledgementV1 {
        version: REPUTATION_GOVERNANCE_DAG_ACKNOWLEDGEMENT_VERSION_V1,
        sequence,
        material_digest,
        signed_result_digest: expected_signed_result_digest,
        dag_block_sequence: block.sequence,
        dag_block_cid: exact_32(&block.block_cid)
            .ok_or(ReputationRuntimeError::InvalidGovernanceAcknowledgement)?,
        dag_node_cid: exact_32(&block.node.node_cid)
            .ok_or(ReputationRuntimeError::InvalidGovernanceAcknowledgement)?,
        published_at_unix: block.timestamp,
        publisher_key_digest: policy.governance_publisher_key_digest,
    };
    acknowledgement.validate()?;
    Ok((acknowledgement, target_index))
}

fn decode_publication_checkpoint(
    bytes: &[u8],
    policy: &ReputationPublicationPolicyV1,
    policy_digest: [u8; 32],
    trust_policy: &ReputationSnapshotTrustPolicyV1,
) -> Result<ReputationPublicationCheckpointV1, ReputationRuntimeError> {
    let checkpoint: ReputationPublicationCheckpointV1 =
        decode_runtime_checkpoint(bytes, policy.checkpoint_max_bytes)?;
    validate_publication_checkpoint(&checkpoint, policy, policy_digest, trust_policy)?;
    Ok(checkpoint)
}

fn validate_publication_checkpoint(
    checkpoint: &ReputationPublicationCheckpointV1,
    policy: &ReputationPublicationPolicyV1,
    policy_digest: [u8; 32],
    trust_policy: &ReputationSnapshotTrustPolicyV1,
) -> Result<(), ReputationRuntimeError> {
    if trust_policy.canonical_digest().ok() != Some(policy.trust_policy_digest)
        || checkpoint.version != REPUTATION_PUBLICATION_CHECKPOINT_VERSION_V1
        || checkpoint.policy_digest != policy_digest
        || checkpoint.committed_snapshots.len() != checkpoint.committed_governance_readbacks.len()
        || checkpoint.committed_snapshots.len() != checkpoint.committed_read.events.len()
        || checkpoint.committed_snapshots.len() > REPUTATION_COMMITTED_READ_MAX_EVENTS_V1
        || checkpoint.committed_read.latest.as_ref() != checkpoint.committed_snapshots.last()
    {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    checkpoint.committed_read.validate(policy_digest)?;
    let mut retained_snapshot_ids = BTreeSet::new();
    let mut previous_readback = None;
    for ((committed, governance_readback), event) in checkpoint
        .committed_snapshots
        .iter()
        .zip(&checkpoint.committed_governance_readbacks)
        .zip(&checkpoint.committed_read.events)
    {
        let readback = committed.validate(
            policy,
            trust_policy,
            governance_readback,
            previous_readback.as_ref(),
        )?;
        if !retained_snapshot_ids.insert(committed.signed_result.snapshot.snapshot_id) {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        let expected_event = ReputationSnapshotEventV1::from_snapshot(
            event.sequence,
            &committed.signed_result.snapshot,
        )
        .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if event != &expected_event {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        previous_readback = Some(readback);
    }
    if let Some(pending) = &checkpoint.pending {
        verify_persisted_signed_result(&pending.signed_result, trust_policy)?;
        if pending.sequence == 0
            || pending.material_digest == [0; 32]
            || pending.signed_result_digest == [0; 32]
            || pending.signed_result.policy_digest != policy.trust_policy_digest
            || signed_result_digest(&pending.signed_result)? != pending.signed_result_digest
            || retained_snapshot_ids.contains(&pending.signed_result.snapshot.snapshot_id)
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        match checkpoint.committed_read.latest.as_ref() {
            Some(latest)
                if pending.signed_result.snapshot.previous_snapshot_id
                    == Some(latest.signed_result.snapshot.snapshot_id)
                    && pending.signed_result.snapshot.generated_at_unix
                        > latest.signed_result.snapshot.generated_at_unix => {}
            None if pending
                .signed_result
                .snapshot
                .previous_snapshot_id
                .is_none() => {}
            _ => return Err(ReputationRuntimeError::InvalidCheckpoint),
        }
        match (
            pending.governance_acknowledgement,
            pending.governance_readback.as_ref(),
        ) {
            (None, None) => {}
            (Some(acknowledgement), Some(readback)) => {
                let readback = readback
                    .reconstruct_readback(&pending.signed_result)
                    .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
                let (expected_acknowledgement, _) = governance_acknowledgement_from_readback(
                    policy,
                    pending.sequence,
                    pending.material_digest,
                    pending.signed_result_digest,
                    &pending.signed_result,
                    &readback,
                    previous_readback.as_ref(),
                )
                .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
                if acknowledgement != expected_acknowledgement {
                    return Err(ReputationRuntimeError::InvalidCheckpoint);
                }
            }
            _ => {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
        }
    }
    Ok(())
}

/// One supervised production tick across committed ingest and external publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationRuntimeTickOutcomeV1 {
    /// Exact-anchor finalized ingest outcome.
    pub finalized: ReputationFinalizedPollOutcomeV1,
    /// External signing/publication outcome, when the target became complete.
    pub publication: Option<ReputationPublicationOutcomeV1>,
    /// Native journal transaction-delivery outcome.
    pub journal_delivery: Option<ReputationJournalDeliveryTickOutcomeV1>,
    /// True only after both projector and Governance DAG acknowledgements are durable.
    pub ready: bool,
}

/// Payload-free health and readiness projection for the combined runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationRuntimeStatusV1 {
    /// Finalized-query supervisor health.
    pub finalized: ReputationFinalizedRuntimeStatusV1,
    /// External signer and Governance DAG reconciliation status.
    pub publication: ReputationPublicationStatusV1,
    /// Whether the projector durably acknowledged the exact signed result.
    pub material_acknowledged: bool,
    /// Native journal delivery status; absence is fail-closed.
    pub journal_delivery: Option<ReputationJournalDeliveryRuntimeStatusV1>,
    /// True only after all committed sources and both publication acknowledgements are durable.
    pub ready: bool,
}

/// Collision-minimizing production supervisor assembled without Torii/config wiring.
#[derive(Debug)]
pub struct ReputationRuntimeSupervisorV1 {
    projector: Arc<ReputationIngestService>,
    finalized: ReputationCommittedProjectorRuntimeV1,
    publication: ReputationPublicationReconcilerV1,
    journal_delivery: Option<ReputationJournalDeliveryWorkerV1>,
    target_height: u64,
}

impl ReputationRuntimeSupervisorV1 {
    /// Assemble the exact projector/query/publication runtime.
    ///
    /// # Errors
    ///
    /// Rejects components that do not share the identical projector instance.
    pub fn new(
        projector: Arc<ReputationIngestService>,
        finalized: ReputationCommittedProjectorRuntimeV1,
        publication: ReputationPublicationReconcilerV1,
    ) -> Result<Self, ReputationRuntimeError> {
        if !Arc::ptr_eq(&projector, &finalized.projector)
            || !Arc::ptr_eq(&projector, &publication.projector)
        {
            return Err(ReputationRuntimeError::RuntimeBindingMismatch);
        }
        let target_height = finalized.policy.window_end_height;
        Ok(Self {
            projector,
            finalized,
            publication,
            journal_delivery: None,
            target_height,
        })
    }

    /// Attach the required native journal transaction-delivery worker.
    ///
    /// # Errors
    ///
    /// Rejects a worker that uses another finalized-query instance.
    pub fn with_journal_delivery(
        mut self,
        journal_delivery: ReputationJournalDeliveryWorkerV1,
    ) -> Result<Self, ReputationRuntimeError> {
        if !Arc::ptr_eq(&self.finalized.query, &journal_delivery.query) {
            return Err(ReputationRuntimeError::RuntimeBindingMismatch);
        }
        self.journal_delivery = Some(journal_delivery);
        Ok(self)
    }

    /// Return the concrete callback injected into the PoR terminal owner.
    #[must_use]
    pub fn por_journal_producer(&self) -> Option<PorReputationJournalProducerV1> {
        self.journal_delivery
            .as_ref()
            .map(ReputationJournalDeliveryWorkerV1::por_producer)
    }

    /// Revalidate every active deployment-owned dependency without advancing
    /// finalized, delivery, or publication state.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed binding error for missing, stale, test-marked, or
    /// substituted active dependencies.
    pub fn check_external_bindings(&self) -> Result<(), ReputationRuntimeError> {
        self.finalized.ensure_query_binding()?;
        self.publication.check_readiness()?;
        self.journal_delivery
            .as_ref()
            .ok_or(ReputationRuntimeError::RuntimeBindingMismatch)?
            .ensure_bindings()
    }

    /// Execute one supervisor-owned deterministic reconciliation tick.
    ///
    /// Deployment wiring schedules this method with bounded backoff and
    /// shutdown handling. The method itself performs no sleeps.
    ///
    /// # Errors
    ///
    /// Propagates finalized query, projector, external delivery, Governance
    /// readback, or persistence failures.
    pub fn reconcile_once(&self) -> Result<ReputationRuntimeTickOutcomeV1, ReputationRuntimeError> {
        // Probe both independent halves before propagating either failure. This
        // keeps finalized ingest making progress during signer/DAG outages
        // while ensuring every tick still observes all external dependencies.
        let finalized = self.finalized.reconcile_once();
        let publication_ready = self.publication.check_readiness();
        let journal_delivery = self
            .journal_delivery
            .as_ref()
            .map(ReputationJournalDeliveryWorkerV1::reconcile_once)
            .transpose();
        let finalized = finalized?;
        publication_ready?;
        let journal_delivery = journal_delivery?;
        let publication = if projector_ready_at(&self.projector, self.target_height)? {
            self.projector.enqueue_unsigned_signing_material()?;
            Some(self.publication.reconcile_once()?)
        } else {
            None
        };
        let ready = self.status()?.ready;
        Ok(ReputationRuntimeTickOutcomeV1 {
            finalized,
            publication,
            journal_delivery,
            ready,
        })
    }

    /// Return payload-free runtime health and readiness without performing work.
    ///
    /// # Errors
    ///
    /// Returns a projector, durable-state, or runtime-lock error.
    pub fn status(&self) -> Result<ReputationRuntimeStatusV1, ReputationRuntimeError> {
        let finalized = self.finalized.status()?;
        let publication = self.publication.status()?;
        let projector = self.projector.status()?;
        let journal_delivery = self
            .journal_delivery
            .as_ref()
            .map(ReputationJournalDeliveryWorkerV1::status)
            .transpose()?;
        let material_acknowledged = projector.material_acknowledged;
        let ready = projector
            .latest_finalized
            .is_some_and(|identity| identity.height == self.target_height)
            && projector.missing_sources.is_empty()
            && material_acknowledged
            && finalized.ready
            && publication.complete
            && journal_delivery.is_some_and(|status| status.ready);
        Ok(ReputationRuntimeStatusV1 {
            finalized,
            publication,
            material_acknowledged,
            journal_delivery,
            ready,
        })
    }

    /// Return the payload-free committed projector status used by deployment
    /// readiness and metrics adapters.
    ///
    /// # Errors
    ///
    /// Returns a durable-state or runtime-lock error.
    pub fn ingest_status(&self) -> Result<super::ReputationIngestStatusV1, ReputationRuntimeError> {
        self.projector.status().map_err(Into::into)
    }

    /// Return the committed projector's payload-free counters.
    #[must_use]
    pub fn ingest_metrics(&self) -> super::ReputationIngestMetricsSnapshot {
        self.projector.metrics()
    }

    /// Return monotonic payload-free native journal-delivery counters.
    ///
    /// # Errors
    ///
    /// Returns a runtime-lock error. Absence means the required worker was not
    /// attached and therefore the supervisor cannot report ready.
    pub fn journal_delivery_metrics(
        &self,
    ) -> Result<Option<ReputationJournalDeliveryMetricsV1>, ReputationRuntimeError> {
        self.journal_delivery
            .as_ref()
            .map(ReputationJournalDeliveryWorkerV1::metrics)
            .transpose()
    }

    /// Return an exact clone of the durable authoritative read projection.
    ///
    /// # Errors
    ///
    /// Returns a durable-state or runtime-lock error.
    pub fn committed_read_projection(
        &self,
    ) -> Result<ReputationCommittedReadProjectionV1, ReputationRuntimeError> {
        self.publication.committed_read_projection()
    }

    /// Return one exact retained authoritative snapshot by its identifier.
    pub fn committed_snapshot_by_id(
        &self,
        snapshot_id: [u8; 16],
    ) -> Result<Option<ReputationSnapshotV1>, ReputationRuntimeError> {
        self.publication.committed_snapshot_by_id(snapshot_id)
    }

    /// Return only the retained committed-event suffix after `sequence`.
    pub fn committed_events_after(
        &self,
        sequence: u64,
    ) -> Result<Vec<ReputationSnapshotEventV1>, ReputationRuntimeError> {
        self.publication.committed_events_after(sequence)
    }
}

impl ReputationCommittedReadApiV1 for ReputationRuntimeSupervisorV1 {
    fn committed_read_projection(
        &self,
    ) -> Result<ReputationCommittedReadProjectionV1, ReputationRuntimeError> {
        self.publication.committed_read_projection()
    }

    fn committed_snapshot_by_id(
        &self,
        snapshot_id: [u8; 16],
    ) -> Result<Option<ReputationSnapshotV1>, ReputationRuntimeError> {
        self.publication.committed_snapshot_by_id(snapshot_id)
    }

    fn committed_events_after(
        &self,
        sequence: u64,
    ) -> Result<Vec<ReputationSnapshotEventV1>, ReputationRuntimeError> {
        self.publication.committed_events_after(sequence)
    }
}

/// Fail-closed runtime, producer, or reconciliation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ReputationRuntimeError {
    /// The runtime policy or one of its hard resource bounds is invalid.
    #[error("reputation runtime policy is invalid")]
    InvalidRuntimePolicy,
    /// A constructed component does not match the pinned production contract.
    #[error("reputation runtime binding does not match policy")]
    RuntimeBindingMismatch,
    /// An injected dependency changed identity after construction.
    #[error("reputation runtime dependency identity changed")]
    RuntimeBindingChanged,
    /// A provider returned a zero or otherwise malformed public qualification.
    #[error("reputation runtime provider qualification is invalid")]
    InvalidProviderQualification,
    /// A finalized anchor is inert or carries an invalid timestamp.
    #[error("reputation finalized anchor is invalid")]
    InvalidFinalizedAnchor,
    /// A query returned another chain.
    #[error("reputation finalized query returned another chain")]
    ChainIdMismatch,
    /// A query violated the at-or-before governed release target.
    #[error("reputation finalized query returned an anchor past the release target")]
    FinalizedAnchorPastTarget,
    /// Finalized height or timestamp moved backward.
    #[error("reputation finalized query rolled back")]
    FinalizedRollback,
    /// A finalized height was returned with a conflicting hash or timestamp.
    #[error("reputation finalized query forked")]
    FinalizedFork,
    /// A typed native query page is malformed or anchored incorrectly.
    #[error("reputation finalized query page is invalid")]
    InvalidQueryPage,
    /// A query continuation is empty, repeated, skipped, or inconsistent.
    #[error("reputation finalized query continuation is invalid")]
    InvalidQueryContinuation,
    /// The configured page budget was exhausted before every source completed.
    #[error("reputation finalized query resource bound was exhausted")]
    QueryResourceExhausted,
    /// The projector did not return one row for every physical feed.
    #[error("reputation committed cursor inventory is invalid")]
    InvalidCursorInventory,
    /// An external payload-free failure receipt is inert.
    #[error("reputation external failure receipt is invalid")]
    InvalidExternalReceipt,
    /// A journal producer scan bound is invalid.
    #[error("reputation journal scan limit is invalid")]
    InvalidScanLimit,
    /// A native PoR or stream-token journal entry is invalid.
    #[error("reputation journal producer entry is invalid")]
    InvalidJournalEntry,
    /// The exact finalized recorder-policy record is malformed or time-invalid.
    #[error("reputation journal authority policy is invalid")]
    InvalidAuthorityPolicy,
    /// Recorder-policy activation history skipped, rolled back, or equivocated.
    #[error("reputation journal authority policy lineage is invalid")]
    AuthorityPolicyLineage,
    /// One policy digest was returned with conflicting activation metadata.
    #[error("reputation journal authority policy record conflicts")]
    AuthorityPolicyRecordConflict,
    /// One native source id was presented with conflicting canonical material.
    #[error("reputation journal producer source conflicts with retained material")]
    JournalSourceConflict,
    /// A journal event is not retained in the producer outbox.
    #[error("reputation journal producer event is unknown")]
    UnknownJournalEvent,
    /// A source observation is newer than the exact finalized policy view.
    #[error("reputation journal source observation is not finalized")]
    JournalSourceNotFinalized,
    /// A journal delivery transition is unsafe from the current crash state.
    #[error("reputation journal producer transition is invalid")]
    InvalidJournalTransition,
    /// A later finalized absence was not proven.
    #[error("reputation journal finalized absence is not proven")]
    FinalizedAbsenceNotProven,
    /// A journal append exhausted its governed retry budget.
    #[error("reputation journal producer retry budget is exhausted")]
    JournalRetryExhausted,
    /// The local journal sequence cannot advance.
    #[error("reputation journal producer sequence is exhausted")]
    JournalSequenceExhausted,
    /// Pending, completed, dead-letter, or checkpoint capacity is exhausted.
    #[error("reputation journal producer resource bound is exhausted")]
    JournalResourceExhausted,
    /// A finalized journal acknowledgement conflicts with its tombstone.
    #[error("reputation journal committed acknowledgement conflicts")]
    JournalAcknowledgementConflict,
    /// The runtime submitter has no identity matching the governed recorder.
    #[error("reputation journal submitter authority does not match policy")]
    JournalSubmitterAuthorityMismatch,
    /// Threshold result differs from the exact projector material.
    #[error("reputation threshold result does not bind projector material")]
    PublicationMaterialMismatch,
    /// The external threshold result is structurally or cryptographically invalid.
    #[error("reputation threshold result is invalid")]
    InvalidThresholdResult,
    /// Governance readback is invalid, forged, or from the wrong publisher.
    #[error("reputation Governance DAG acknowledgement is invalid")]
    InvalidGovernanceAcknowledgement,
    /// A Governance DAG inclusion readback exceeds the hard V1 path bound.
    #[error("reputation Governance DAG inclusion path has {found} blocks; maximum is {maximum}")]
    GovernanceReadbackPathTooLong {
        /// Observed inclusion-path block count.
        found: usize,
        /// Hard maximum accepted by this runtime.
        maximum: usize,
    },
    /// Publication durable state conflicts with projector/external state.
    #[error("reputation publication checkpoint conflicts with reconciliation state")]
    PublicationCheckpointConflict,
    /// Canonical Norito encoding failed.
    #[error("reputation runtime canonical encoding failed")]
    CanonicalEncoding,
    /// A durable checkpoint is malformed, noncanonical, or inconsistent.
    #[error("reputation runtime checkpoint is invalid")]
    InvalidCheckpoint,
    /// Irreducible durable state exceeds its hard byte ceiling after replay compaction.
    #[error("reputation runtime checkpoint exceeds its byte ceiling")]
    CheckpointTooLarge,
    /// Durable state is unsafe or inaccessible.
    #[error("reputation runtime checkpoint I/O failed")]
    CheckpointIo,
    /// Another writer owns the durable checkpoint.
    #[error("reputation runtime checkpoint writer is busy")]
    CheckpointBusy,
    /// Durable state changed concurrently.
    #[error("reputation runtime checkpoint changed concurrently")]
    CheckpointStale,
    /// Rename may be visible while parent-directory durability is uncertain.
    #[error("reputation runtime checkpoint durability is uncertain")]
    CheckpointDurabilityUncertain,
    /// An in-process runtime mutex or writer registry was poisoned.
    #[error("reputation runtime is poisoned")]
    RuntimePoisoned,
    /// The deterministic projector rejected the operation.
    #[error(transparent)]
    Projector(#[from] ReputationIngestError),
    /// An injected dependency returned a payload-free failure.
    #[error(transparent)]
    External(#[from] ReputationExternalFailureV1),
}

impl ReputationRuntimeError {
    fn external_receipt(&self) -> Option<[u8; 32]> {
        match self {
            Self::External(failure) => Some(failure.receipt()),
            _ => None,
        }
    }
}

impl From<CheckpointStoreError> for ReputationRuntimeError {
    fn from(error: CheckpointStoreError) -> Self {
        match error {
            CheckpointStoreError::Io => Self::CheckpointIo,
            CheckpointStoreError::TooLarge => Self::CheckpointTooLarge,
            CheckpointStoreError::Busy => Self::CheckpointBusy,
            CheckpointStoreError::Stale => Self::CheckpointStale,
            CheckpointStoreError::DurabilityUncertain => Self::CheckpointDurabilityUncertain,
            CheckpointStoreError::RuntimePoisoned => Self::RuntimePoisoned,
        }
    }
}

fn validate_runtime_handle(handle: String) -> Result<String, ReputationRuntimeError> {
    validate_production_runtime_handle(&handle)
        .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
    Ok(handle)
}

fn qualify_runtime_provider<P: ReputationRuntimeProviderV1 + ?Sized>(
    expected_handle: &str,
    expected_qualification: ReputationRuntimeProviderQualificationV1,
    provider: &P,
) -> Result<(), ReputationRuntimeError> {
    validate_runtime_handle(expected_handle.to_owned())?;
    if !expected_qualification.is_valid()
        || expected_qualification.revision()
            != REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1
    {
        return Err(ReputationRuntimeError::InvalidRuntimePolicy);
    }
    if validate_runtime_handle(provider.handle().to_owned()).is_err()
        || provider.handle() != expected_handle
    {
        return Err(ReputationRuntimeError::RuntimeBindingMismatch);
    }
    let qualification = provider.qualification()?;
    if !qualification.is_valid() {
        return Err(ReputationRuntimeError::InvalidProviderQualification);
    }
    if qualification != expected_qualification || provider.handle() != expected_handle {
        return Err(ReputationRuntimeError::RuntimeBindingMismatch);
    }
    Ok(())
}

fn assert_runtime_provider_qualification<P: ReputationRuntimeProviderV1 + ?Sized>(
    expected_handle: &str,
    expected_qualification: ReputationRuntimeProviderQualificationV1,
    provider: &P,
) -> Result<(), ReputationRuntimeError> {
    if provider.handle() != expected_handle {
        return Err(ReputationRuntimeError::RuntimeBindingChanged);
    }
    let qualification = provider.qualification()?;
    if !qualification.is_valid()
        || qualification != expected_qualification
        || provider.handle() != expected_handle
    {
        return Err(ReputationRuntimeError::RuntimeBindingChanged);
    }
    Ok(())
}

fn decode_runtime_checkpoint<T>(bytes: &[u8], max_bytes: u64) -> Result<T, ReputationRuntimeError>
where
    T: for<'decode> norito::NoritoDeserialize<'decode> + norito::NoritoSerialize,
{
    if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(ReputationRuntimeError::CheckpointTooLarge);
    }
    norito::core::from_bytes_view(bytes).map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
    let limits = runtime_decode_limits(bytes.len())?;
    let value: T = decode_from_bytes_with_limits(bytes, limits)
        .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
    if norito::to_bytes(&value).map_err(|_| ReputationRuntimeError::CanonicalEncoding)? != bytes {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    Ok(value)
}

fn runtime_decode_limits(encoded_bytes: usize) -> Result<DecodeLimits, ReputationRuntimeError> {
    if encoded_bytes == 0 {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    let elements = encoded_bytes
        .checked_mul(RUNTIME_CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT)
        .ok_or(ReputationRuntimeError::CheckpointTooLarge)?;
    let allocations = encoded_bytes
        .checked_mul(RUNTIME_CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT)
        .and_then(|value| value.checked_add(RUNTIME_CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES))
        .ok_or(ReputationRuntimeError::CheckpointTooLarge)?;
    Ok(DecodeLimits::new(
        encoded_bytes,
        encoded_bytes,
        elements,
        allocations,
        RUNTIME_CHECKPOINT_MAX_NESTING_DEPTH,
    ))
}

fn hash_canonical<T: norito::NoritoSerialize>(
    domain: &'static [u8],
    value: &T,
) -> Result<[u8; 32], ReputationRuntimeError> {
    let bytes = norito::to_bytes(value).map_err(|_| ReputationRuntimeError::CanonicalEncoding)?;
    domain_digest(domain, &bytes)
}

fn domain_digest(domain: &'static [u8], bytes: &[u8]) -> Result<[u8; 32], ReputationRuntimeError> {
    let len = u64::try_from(bytes.len()).map_err(|_| ReputationRuntimeError::CanonicalEncoding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&len.to_le_bytes());
    hasher.update(bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn exact_32(bytes: &[u8]) -> Option<[u8; 32]> {
    <[u8; 32]>::try_from(bytes).ok()
}

fn valid_ed25519_verifying_key(bytes: [u8; 32]) -> bool {
    iroha_crypto::ed25519_parse_public_key(&bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, fs, path::Path};

    use ed25519_dalek::{Signer, SigningKey};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::sorafs::reputation::{
        PorTerminalStatusV1, REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
        ReputationJournalFinalizedEventV1, StreamTokenExcludedKindV1,
        StreamTokenValidationBindingV1, StreamTokenValidationStatusV1,
    };
    use sorafs_manifest::reputation::{
        REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationWeightsV1, build_reputation_snapshot,
        signed::{
            REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1, REPUTATION_TRUSTED_SIGNER_VERSION_V1,
            ReputationScoringEvidenceV1, ReputationSnapshotSignatureV1, ReputationTrustedSignerV1,
            SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
        },
    };
    use sorafs_manifest::{GOVERNANCE_DAG_HEAD_VERSION_V1, GovernanceSignatureAlgorithm};
    use tempfile::TempDir;

    use super::*;

    const FINALIZED_AT_MS: u64 = 1_800_000_010_000;

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
            .expect("derive deterministic account");
        AccountId::new(keypair.public_key().clone())
    }

    const fn provider(seed: u8) -> ProviderId {
        ProviderId::new([seed; 32])
    }

    fn journal_authority_policy() -> ReputationJournalAuthorityPolicyV1 {
        ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: account(1),
            dispute_recorder_authority: account(2),
            token_recorder_authority: account(3),
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        }
    }

    fn producer_policy() -> ReputationJournalProducerPolicyV1 {
        ReputationJournalProducerPolicyV1::strict_v1(
            ChainId::from("reputation-runtime-test"),
            journal_authority_policy(),
        )
        .expect("valid producer policy")
    }

    fn authority_record(
        policy: ReputationJournalAuthorityPolicyV1,
        activated_at_unix_ms: u64,
    ) -> ReputationJournalAuthorityPolicyRecordV1 {
        ReputationJournalAuthorityPolicyRecordV1::try_new(
            policy,
            account(0x44),
            activated_at_unix_ms,
        )
        .expect("authority policy record")
    }

    fn successor_authority_policy(
        predecessor: &ReputationJournalAuthorityPolicyV1,
        recorder_marker: u8,
    ) -> ReputationJournalAuthorityPolicyV1 {
        let mut successor = predecessor.clone();
        successor.revision = predecessor.revision.checked_add(1).expect("test revision");
        successor.predecessor_policy_digest = Some(
            predecessor
                .canonical_digest()
                .expect("predecessor policy digest"),
        );
        successor.por_recorder_authority = account(recorder_marker);
        successor
    }

    fn open_initialized_producer_outbox(
        root: &Path,
        policy: ReputationJournalProducerPolicyV1,
    ) -> ReputationJournalProducerOutboxV1 {
        let record = authority_record(
            policy.authority_policy.clone(),
            FINALIZED_AT_MS.saturating_sub(1_000),
        );
        let outbox =
            ReputationJournalProducerOutboxV1::open(root, policy).expect("producer outbox");
        assert!(matches!(
            outbox
                .synchronize_authority_policy(
                    record,
                    ReputationJournalFinalizedCursorV1 {
                        height: 1,
                        block_hash: [0x70; 32],
                        finalized_at_unix_ms: FINALIZED_AT_MS,
                    },
                )
                .expect("initialize producer authority policy"),
            ReputationJournalPolicySyncOutcomeV1::Initialized
                | ReputationJournalPolicySyncOutcomeV1::ExactReplay
        ));
        outbox
    }

    fn delivery_view(
        height: u64,
        block_hash: [u8; 32],
        finalized_at_unix_ms: u64,
        authority_policy: ReputationJournalAuthorityPolicyRecordV1,
        events: Vec<ReputationJournalFinalizedEventV1>,
    ) -> ReputationJournalDeliveryFinalizedViewV1 {
        ReputationJournalDeliveryFinalizedViewV1 {
            anchor: ReputationFinalizedAnchorV1 {
                chain_id: ChainId::from("reputation-runtime-test"),
                identity: ReputationFinalizedIdentityV1 { height, block_hash },
                finalized_at_unix_ms,
            },
            authority_policy_history: vec![authority_policy.clone()],
            authority_policy,
            journal_page: ReputationJournalFinalizedEventPageV1 {
                finalized_cursor: ReputationJournalFinalizedCursorV1 {
                    height,
                    block_hash,
                    finalized_at_unix_ms,
                },
                events,
                has_more: false,
                next_after: None,
            },
        }
    }

    fn reconcile_empty_journal(
        outbox: &ReputationJournalProducerOutboxV1,
        height: u64,
        block_hash: [u8; 32],
        finalized_at_unix_ms: u64,
    ) {
        outbox
            .reconcile_finalized_journal_page(ReputationJournalFinalizedEventPageV1 {
                finalized_cursor: ReputationJournalFinalizedCursorV1 {
                    height,
                    block_hash,
                    finalized_at_unix_ms,
                },
                events: Vec::new(),
                has_more: false,
                next_after: None,
            })
            .expect("reconcile terminal empty journal page");
    }

    fn verified_por(challenge_marker: u8) -> PorTerminalOutcomeV1 {
        PorTerminalOutcomeV1 {
            challenge_id: [challenge_marker; 32],
            manifest_digest: [0x41; 32],
            epoch_id: 7,
            drand_round: 11,
            forced: false,
            sample_count: 4,
            failed_samples: 0,
            issued_at_unix_ms: FINALIZED_AT_MS - 2_000,
            deadline_at_unix_ms: FINALIZED_AT_MS - 500,
            responded_at_unix_ms: Some(FINALIZED_AT_MS - 750),
            decided_at_unix_ms: FINALIZED_AT_MS,
            proof_digest: Some([0x42; 32]),
            repair_task_id: None,
            verifier_latency_ms: Some(17),
            status: PorTerminalStatusV1::Verified,
        }
    }

    fn counted_token(validation_marker: u8, request_marker: u8) -> StreamTokenValidationOutcomeV1 {
        StreamTokenValidationOutcomeV1 {
            binding: StreamTokenValidationBindingV1 {
                gateway_id: [validation_marker; 32],
                gateway_sequence: 1,
                request_context_digest: [request_marker; 32],
            },
            token_body_digest: Some([0x52; 32]),
            token_key_version: Some(1),
            validated_at_unix_ms: FINALIZED_AT_MS,
            status: StreamTokenValidationStatusV1::Accepted,
        }
    }

    fn excluded_token(validation_marker: u8) -> StreamTokenValidationOutcomeV1 {
        StreamTokenValidationOutcomeV1 {
            binding: StreamTokenValidationBindingV1 {
                gateway_id: [validation_marker; 32],
                gateway_sequence: 1,
                request_context_digest: [0x61; 32],
            },
            token_body_digest: None,
            token_key_version: None,
            validated_at_unix_ms: FINALIZED_AT_MS,
            status: StreamTokenValidationStatusV1::Excluded(
                StreamTokenExcludedKindV1::MissingToken,
            ),
        }
    }

    fn stream_token_entry(outcome: StreamTokenValidationOutcomeV1) -> ReputationJournalEntryV1 {
        let policy = journal_authority_policy();
        ReputationJournalEntryV1::try_new(
            provider(9),
            policy.canonical_digest().expect("authority policy digest"),
            policy.token_recorder_authority,
            outcome.validated_at_unix_ms,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(outcome),
        )
        .expect("canonical stream-token journal entry")
    }

    fn finalized_journal_event(
        sequence: u64,
        block_height: u64,
        block_hash: [u8; 32],
        event_index: u32,
        entry: ReputationJournalEntryV1,
    ) -> ReputationJournalFinalizedEventV1 {
        ReputationJournalFinalizedEventV1 {
            sequence,
            block_height,
            block_hash,
            event_index,
            recorded_at_unix_ms: entry.source_time_unix_ms.saturating_add(10),
            entry,
        }
    }

    fn counted_validation(
        validated_at_unix_ms: u64,
        status: StreamTokenValidationStatusV1,
    ) -> StreamTokenCountedValidationV1 {
        StreamTokenCountedValidationV1 {
            token_body_digest: [0x52; 32],
            token_key_version: 1,
            validated_at_unix_ms,
            status,
        }
    }

    fn counted_request_context(nonce: &str) -> StreamTokenValidationRequestContextV1 {
        StreamTokenValidationRequestContextV1::try_new(
            provider(9),
            [0x33; 32],
            sorafs_manifest::canonical_manifest_root_cid([0x44; 32]),
            "sorafs.sf1@1.0.0".to_owned(),
            nonce,
            Some(b"Q2Fub25pY2FsVG9rZW4="),
            iroha_data_model::sorafs::reputation::StreamTokenRequestRouteV1::car_range(64, 1_023)
                .expect("canonical CAR range"),
        )
        .expect("canonical payload-free request context")
    }

    fn trust_policy() -> ReputationSnapshotTrustPolicyV1 {
        let signing_key = SigningKey::from_bytes(&[0x71; 32]);
        ReputationSnapshotTrustPolicyV1 {
            version: REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1,
            policy_id: [0x72; 32],
            valid_from_unix: FINALIZED_AT_MS / 1_000 - 1_000,
            valid_until_unix: FINALIZED_AT_MS / 1_000 + 1_000,
            max_snapshot_age_secs: 600,
            max_future_skew_secs: 30,
            min_signatures: 1,
            signers: vec![ReputationTrustedSignerV1 {
                version: REPUTATION_TRUSTED_SIGNER_VERSION_V1,
                signer_id: "threshold-a".to_owned(),
                public_key: signing_key.verifying_key().to_bytes(),
            }],
            revoked_signer_ids: Vec::new(),
        }
    }

    fn ingest_policy(trust_policy: &ReputationSnapshotTrustPolicyV1) -> ReputationIngestPolicyV1 {
        ReputationIngestPolicyV1::strict_v1(
            ChainId::from("reputation-runtime-test"),
            1,
            10,
            trust_policy
                .canonical_digest()
                .expect("canonical trust policy"),
            ReputationWeightsV1::default(),
        )
    }

    fn publication_policy(
        trust_policy: &ReputationSnapshotTrustPolicyV1,
    ) -> ReputationPublicationPolicyV1 {
        ReputationPublicationPolicyV1::try_new(
            trust_policy,
            "signer-a",
            "dag-a",
            b"peer-a".to_vec(),
            SigningKey::from_bytes(&[0xB1; 32])
                .verifying_key()
                .to_bytes(),
            REPUTATION_PUBLICATION_MAX_CHECKPOINT_BYTES_V1,
        )
        .expect("publication policy")
    }

    fn reputation_input(provider_id: &str) -> ReputationProviderInputV1 {
        ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: provider_id.to_owned(),
            metrics: ReputationProviderMetricsV1 {
                version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
                por_success_bps: 9_800,
                pdp_success_bps: 9_700,
                potr_success_bps: 9_600,
                latency_health_bps: 9_500,
                dispute_rate_bps: 0,
                token_violation_rate_bps: 0,
                repair_breach_rate_bps: 0,
            },
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        }
    }

    fn signed_snapshot(
        trust_policy: &ReputationSnapshotTrustPolicyV1,
        snapshot_id: [u8; 16],
        previous_snapshot_id: Option<[u8; 16]>,
        generated_at_unix: u64,
    ) -> SignedReputationSnapshotV1 {
        let scoring_evidence = ReputationScoringEvidenceV1 {
            version: 1,
            provider_inputs: vec![reputation_input("provider-a")],
            trust_edges: Vec::new(),
        };
        let snapshot = build_reputation_snapshot(
            snapshot_id,
            generated_at_unix,
            ReputationWeightsV1::default(),
            &scoring_evidence.provider_inputs,
            previous_snapshot_id,
        )
        .expect("reputation snapshot");
        let scoring_evidence_digest = scoring_evidence
            .canonical_digest()
            .expect("scoring evidence digest");
        let policy_digest = trust_policy
            .canonical_digest()
            .expect("trust policy digest");
        let signing_digest = sorafs_manifest::reputation::signed::snapshot_signing_digest(
            &snapshot,
            policy_digest,
            scoring_evidence_digest,
        )
        .expect("snapshot signing digest");
        SignedReputationSnapshotV1 {
            version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
            policy_digest,
            snapshot,
            scoring_evidence_digest,
            scoring_evidence,
            signatures: vec![ReputationSnapshotSignatureV1 {
                signer_id: "threshold-a".to_owned(),
                signature: SigningKey::from_bytes(&[0x71; 32])
                    .sign(&signing_digest)
                    .to_bytes(),
            }],
        }
    }

    fn empty_governance_signature() -> GovernanceLogSignatureV1 {
        GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn sign_governance_node(node: &mut GovernanceLogNodeV1) {
        let signing_key = SigningKey::from_bytes(&[0xB1; 32]);
        let payload = node
            .signature_payload_bytes()
            .expect("encode governance node signature payload");
        node.publisher_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signing_key.sign(&payload).to_bytes().to_vec(),
        };
    }

    fn governance_block_after(
        signed_result: &SignedReputationSnapshotV1,
        previous: Option<&GovernanceDagBlockV1>,
    ) -> GovernanceDagBlockV1 {
        let signing_key = SigningKey::from_bytes(&[0xB1; 32]);
        let publisher_peer_id = b"peer-a".to_vec();
        let publication_timestamp =
            previous.map_or(signed_result.snapshot.generated_at_unix, |block| {
                block
                    .timestamp
                    .max(signed_result.snapshot.generated_at_unix)
            });
        let mut node = GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: Vec::new(),
            prev_cid: previous.map(|block| block.node.node_cid.clone()),
            timestamp: publication_timestamp,
            publisher_peer_id: publisher_peer_id.clone(),
            payload: GovernanceLogPayloadV1::SignedReputationSnapshot(signed_result.clone()),
            publisher_signature: empty_governance_signature(),
        };
        node.node_cid = node
            .recompute_node_cid()
            .expect("derive governance node CID");
        sign_governance_node(&mut node);

        let mut block = GovernanceDagBlockV1 {
            version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
            block_cid: Vec::new(),
            prev_block_cid: previous.map(|block| block.block_cid.clone()),
            sequence: previous.map_or(0, |block| {
                block.sequence.checked_add(1).expect("fixture sequence")
            }),
            timestamp: publication_timestamp,
            publisher_peer_id,
            node,
            block_signature: empty_governance_signature(),
        };
        block.block_cid = block
            .recompute_block_cid()
            .expect("derive governance block CID");
        let payload = block
            .signature_payload_bytes()
            .expect("encode governance block signature payload");
        block.block_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signing_key.sign(&payload).to_bytes().to_vec(),
        };
        block.validate().expect("validate governance block fixture");
        block
    }

    fn governance_head(path: &[GovernanceDagBlockV1]) -> GovernanceDagHeadV1 {
        let signing_key = SigningKey::from_bytes(&[0xB1; 32]);
        let tip = path.last().expect("non-empty Governance DAG path");
        let block_count = tip.sequence.checked_add(1).expect("fixture block count");
        let mut head = GovernanceDagHeadV1 {
            version: GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid: tip.block_cid.clone(),
            block_count,
            generated_at: tip.timestamp,
            publisher_peer_id: b"peer-a".to_vec(),
            checkpoint_cid: (block_count
                > u64::try_from(REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1)
                    .expect("fixture bound"))
            .then(|| path.first().expect("non-empty path").block_cid.clone()),
            head_signature: empty_governance_signature(),
        };
        let payload = head
            .signature_payload_bytes()
            .expect("encode Governance DAG head signature payload");
        head.head_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signing_key.sign(&payload).to_bytes().to_vec(),
        };
        head.validate()
            .expect("validate Governance DAG head fixture");
        head
    }

    fn governance_readback_after(
        policy: &ReputationPublicationPolicyV1,
        sequence: u64,
        material_digest: [u8; 32],
        signed_result_digest: [u8; 32],
        signed_result: &SignedReputationSnapshotV1,
        previous_readback: Option<&ReputationGovernanceDagReadbackV1>,
    ) -> (
        ReputationGovernanceDagAcknowledgementV1,
        StoredReputationGovernanceDagReadbackV1,
    ) {
        let previous_tip = previous_readback.and_then(|readback| readback.inclusion_path.last());
        let block = governance_block_after(signed_result, previous_tip);
        let readback = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(std::slice::from_ref(&block)),
            inclusion_path: vec![block],
        };
        let (acknowledgement, target_index) = governance_acknowledgement_from_readback(
            policy,
            sequence,
            material_digest,
            signed_result_digest,
            signed_result,
            &readback,
            previous_readback,
        )
        .expect("derive governance acknowledgement");
        (
            acknowledgement,
            StoredReputationGovernanceDagReadbackV1::from_readback(&readback, target_index)
                .expect("store Governance DAG readback"),
        )
    }

    fn governance_readback(
        policy: &ReputationPublicationPolicyV1,
        sequence: u64,
        material_digest: [u8; 32],
        signed_result_digest: [u8; 32],
        signed_result: &SignedReputationSnapshotV1,
    ) -> (
        ReputationGovernanceDagAcknowledgementV1,
        StoredReputationGovernanceDagReadbackV1,
    ) {
        governance_readback_after(
            policy,
            sequence,
            material_digest,
            signed_result_digest,
            signed_result,
            None,
        )
    }

    fn signing_delivery(
        signed_result: &SignedReputationSnapshotV1,
    ) -> ReputationUnsignedMaterialDeliveryV1 {
        let material = ReputationUnsignedSigningMaterialV1 {
            version: crate::reputation::REPUTATION_UNSIGNED_MATERIAL_VERSION_V1,
            chain_id: ChainId::from("reputation-runtime-test"),
            ingest_policy_digest: [0xD1; 32],
            snapshot_trust_policy_digest: signed_result.policy_digest,
            window_start_height: 1,
            window_end_height: 10,
            target_finalized: ReputationFinalizedIdentityV1 {
                height: 10,
                block_hash: [0xD2; 32],
            },
            target_finalized_at_unix_ms: FINALIZED_AT_MS,
            source_finality: Vec::new(),
            scoring_evidence: signed_result.scoring_evidence.clone(),
            scoring_evidence_digest: signed_result.scoring_evidence_digest,
            snapshot: signed_result.snapshot.clone(),
            snapshot_signing_digest: signed_result
                .signing_digest()
                .expect("snapshot signing digest"),
        };
        let material_digest = hash_canonical(
            b"sorafs-reputation-unsigned-material-delivery-v1",
            &material,
        )
        .expect("unsigned material digest");
        ReputationUnsignedMaterialDeliveryV1 {
            version: crate::reputation::REPUTATION_UNSIGNED_MATERIAL_DELIVERY_VERSION_V1,
            sequence: 1,
            material_digest,
            material,
            failed_attempts: 0,
            state: ReputationUnsignedMaterialDeliveryStateV1::Pending,
        }
    }

    #[derive(Debug)]
    struct NullQuery {
        handle: String,
        qualification: Mutex<ReputationRuntimeProviderQualificationV1>,
        drift_after_anchor: bool,
    }

    impl ReputationRuntimeProviderV1 for NullQuery {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1> {
            Ok(*self.qualification.lock().expect("qualification lock"))
        }
    }

    impl ReputationFinalizedQueryV1 for NullQuery {
        fn finalized_at_or_before(
            &self,
            chain_id: &ChainId,
            maximum_height: u64,
        ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1> {
            if self.drift_after_anchor {
                self.qualification
                    .lock()
                    .expect("qualification lock")
                    .revision = REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1 + 1;
                return Ok(ReputationFinalizedAnchorV1 {
                    chain_id: chain_id.clone(),
                    identity: ReputationFinalizedIdentityV1 {
                        height: maximum_height,
                        block_hash: [0x91; 32],
                    },
                    finalized_at_unix_ms: FINALIZED_AT_MS,
                });
            }
            Err(ReputationExternalFailureV1::try_new([1; 32]).expect("failure"))
        }

        fn reputation_journal_delivery_view(
            &self,
            _chain_id: &ChainId,
            _maximum_height: u64,
            _policy_query: FindSorafsReputationJournalAuthorityPolicy,
            _after: Option<ReputationJournalFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReputationJournalDeliveryFinalizedViewV1, ReputationExternalFailureV1> {
            Err(ReputationExternalFailureV1::try_new([8; 32]).expect("failure"))
        }

        fn proof_outcome_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ProofOutcomeFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ProofOutcomeFinalizedEventPageV1, ReputationExternalFailureV1> {
            Err(ReputationExternalFailureV1::try_new([2; 32]).expect("failure"))
        }

        fn reputation_journal_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ReputationJournalFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReputationJournalFinalizedEventPageV1, ReputationExternalFailureV1> {
            Err(ReputationExternalFailureV1::try_new([3; 32]).expect("failure"))
        }

        fn repair_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<RepairFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<RepairFinalizedEventPageV1, ReputationExternalFailureV1> {
            Err(ReputationExternalFailureV1::try_new([4; 32]).expect("failure"))
        }

        fn orderbook_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<OrderbookFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<OrderbookFinalizedEventPageV1, ReputationExternalFailureV1> {
            Err(ReputationExternalFailureV1::try_new([5; 32]).expect("failure"))
        }

        fn reserve_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ReserveFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReserveFinalizedEventPageV1, ReputationExternalFailureV1> {
            Err(ReputationExternalFailureV1::try_new([6; 32]).expect("failure"))
        }

        fn reserve_provider_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after_provider_id: Option<ProviderId>,
            _limit: u32,
        ) -> Result<ReserveProviderAccountPageV1, ReputationExternalFailureV1> {
            Err(ReputationExternalFailureV1::try_new([7; 32]).expect("failure"))
        }
    }

    #[derive(Debug)]
    struct ScriptedDeliveryQuery {
        handle: String,
        qualification: ReputationRuntimeProviderQualificationV1,
        views: Mutex<VecDeque<ReputationJournalDeliveryFinalizedViewV1>>,
    }

    impl ReputationRuntimeProviderV1 for ScriptedDeliveryQuery {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1> {
            Ok(self.qualification)
        }
    }

    impl ReputationFinalizedQueryV1 for ScriptedDeliveryQuery {
        fn finalized_at_or_before(
            &self,
            _chain_id: &ChainId,
            _maximum_height: u64,
        ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1> {
            self.views
                .lock()
                .expect("script lock")
                .front()
                .map(|view| view.anchor.clone())
                .ok_or_else(|| ReputationExternalFailureV1::try_new([0xE1; 32]).expect("failure"))
        }

        fn reputation_journal_delivery_view(
            &self,
            _chain_id: &ChainId,
            _maximum_height: u64,
            _policy_query: FindSorafsReputationJournalAuthorityPolicy,
            _after: Option<ReputationJournalFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReputationJournalDeliveryFinalizedViewV1, ReputationExternalFailureV1> {
            self.views
                .lock()
                .expect("script lock")
                .pop_front()
                .ok_or_else(|| ReputationExternalFailureV1::try_new([0xE2; 32]).expect("failure"))
        }

        fn proof_outcome_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ProofOutcomeFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ProofOutcomeFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("delivery test does not query proof outcomes")
        }

        fn reputation_journal_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ReputationJournalFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReputationJournalFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("delivery test uses the coherent view method")
        }

        fn repair_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<RepairFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<RepairFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("delivery test does not query repairs")
        }

        fn orderbook_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<OrderbookFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<OrderbookFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("delivery test does not query orderbook")
        }

        fn reserve_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ReserveFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReserveFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("delivery test does not query reserve events")
        }

        fn reserve_provider_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after_provider_id: Option<ProviderId>,
            _limit: u32,
        ) -> Result<ReserveProviderAccountPageV1, ReputationExternalFailureV1> {
            unreachable!("delivery test does not query reserve providers")
        }
    }

    #[derive(Debug)]
    struct RecordingJournalSubmitter {
        handle: String,
        qualification: ReputationRuntimeProviderQualificationV1,
        requests: Mutex<Vec<ReputationJournalTransactionRequestV1>>,
    }

    impl ReputationRuntimeProviderV1 for RecordingJournalSubmitter {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1> {
            Ok(self.qualification)
        }
    }

    impl ReputationJournalTransactionSubmitterV1 for RecordingJournalSubmitter {
        fn supports_authority(&self, _authority: &AccountId) -> bool {
            true
        }

        fn submit(
            &self,
            request: &ReputationJournalTransactionRequestV1,
        ) -> ReputationJournalTransactionSubmitOutcomeV1 {
            self.requests
                .lock()
                .expect("request lock")
                .push(request.clone());
            ReputationJournalTransactionSubmitOutcomeV1::Queued {
                receipt: request.idempotency_key,
            }
        }
    }

    #[derive(Debug)]
    struct NullThresholdSigner {
        handle: String,
        qualification: ReputationRuntimeProviderQualificationV1,
    }

    impl ReputationRuntimeProviderV1 for NullThresholdSigner {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1> {
            Ok(self.qualification)
        }
    }

    impl ReputationThresholdSignerClientV1 for NullThresholdSigner {
        fn reconcile_signature(
            &self,
            _request: &ReputationThresholdSigningRequestV1,
        ) -> Result<Option<SignedReputationSnapshotV1>, ReputationExternalFailureV1> {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct NullGovernanceDag {
        handle: String,
        qualification: ReputationRuntimeProviderQualificationV1,
    }

    impl ReputationRuntimeProviderV1 for NullGovernanceDag {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1> {
            Ok(self.qualification)
        }
    }

    impl ReputationGovernanceDagClientV1 for NullGovernanceDag {
        fn reconcile_publication(
            &self,
            _request: &ReputationGovernanceDagPublicationRequestV1,
        ) -> Result<Option<ReputationGovernanceDagReadbackV1>, ReputationExternalFailureV1>
        {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct DriftingGovernanceDag {
        handle: String,
        qualification: Mutex<ReputationRuntimeProviderQualificationV1>,
        readback: ReputationGovernanceDagReadbackV1,
    }

    impl ReputationRuntimeProviderV1 for DriftingGovernanceDag {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1> {
            Ok(*self.qualification.lock().expect("qualification lock"))
        }
    }

    impl ReputationGovernanceDagClientV1 for DriftingGovernanceDag {
        fn reconcile_publication(
            &self,
            _request: &ReputationGovernanceDagPublicationRequestV1,
        ) -> Result<Option<ReputationGovernanceDagReadbackV1>, ReputationExternalFailureV1>
        {
            self.qualification
                .lock()
                .expect("qualification lock")
                .revision = REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1 + 1;
            Ok(Some(self.readback.clone()))
        }
    }

    fn open_publication_reconciler(
        root: &Path,
        projector: Arc<ReputationIngestService>,
        trust_policy: ReputationSnapshotTrustPolicyV1,
        policy: ReputationPublicationPolicyV1,
    ) -> ReputationPublicationReconcilerV1 {
        let signer_qualification = policy.threshold_signer_qualification();
        let dag_qualification = policy.governance_dag_qualification();
        ReputationPublicationReconcilerV1::open(
            root,
            projector,
            trust_policy,
            policy,
            Arc::new(NullThresholdSigner {
                handle: "signer-a".to_owned(),
                qualification: signer_qualification,
            }),
            Arc::new(NullGovernanceDag {
                handle: "dag-a".to_owned(),
                qualification: dag_qualification,
            }),
        )
        .expect("publication reconciler")
    }

    #[test]
    fn strict_construction_rejects_query_handle_substitution() {
        let temp = TempDir::new().expect("tempdir");
        let trust = trust_policy();
        let ingest = ingest_policy(&trust);
        let projector = Arc::new(
            ReputationIngestService::open(temp.path(), ingest.clone()).expect("projector"),
        );
        let policy = ReputationFinalizedQueryPolicyV1::try_new(&ingest, "query-a", 32, 1_024)
            .expect("query policy");
        let query_qualification = policy.query_qualification();
        let result = ReputationCommittedProjectorRuntimeV1::new(
            projector,
            &ingest,
            policy,
            Arc::new(NullQuery {
                handle: "query-b".to_owned(),
                qualification: Mutex::new(query_qualification),
                drift_after_anchor: false,
            }),
        );
        assert!(matches!(
            result,
            Err(ReputationRuntimeError::RuntimeBindingMismatch)
        ));
    }

    #[test]
    fn runtime_handles_use_canonical_production_grammar() {
        for handle in [
            "hsm://sorafs/reputation/threshold-primary",
            "https-pinned-source-pool:reputation-finalized-primary",
        ] {
            assert_eq!(
                validate_runtime_handle(handle.to_owned()),
                Ok(handle.to_owned())
            );
        }
        for handle in [
            "hsm://sorafs/reputation/operator@threshold",
            "hsm://sorafs/reputation/threshold?token",
            "hsm://sorafs/reputation/threshold#fragment",
            "hsm://sorafs/reputation/%74hreshold",
            "hsm://sorafs/reputation/threshold\\primary",
        ] {
            assert_eq!(
                validate_runtime_handle(handle.to_owned()),
                Err(ReputationRuntimeError::InvalidRuntimePolicy)
            );
        }
    }

    #[test]
    fn finalized_query_result_is_discarded_when_qualification_drifts() {
        let temp = TempDir::new().expect("tempdir");
        let trust = trust_policy();
        let ingest = ingest_policy(&trust);
        let projector = Arc::new(
            ReputationIngestService::open(temp.path(), ingest.clone()).expect("projector"),
        );
        let policy = ReputationFinalizedQueryPolicyV1::try_new(&ingest, "query-a", 32, 1_024)
            .expect("query policy");
        let query = Arc::new(NullQuery {
            handle: "query-a".to_owned(),
            qualification: Mutex::new(policy.query_qualification()),
            drift_after_anchor: true,
        });
        let runtime = ReputationCommittedProjectorRuntimeV1::new(
            Arc::clone(&projector),
            &ingest,
            policy,
            query,
        )
        .expect("qualified runtime");

        assert!(matches!(
            runtime.reconcile_once(),
            Err(ReputationRuntimeError::RuntimeBindingChanged)
        ));
        assert!(
            projector
                .status()
                .expect("projector status")
                .latest_finalized
                .is_none(),
            "the anchor returned during qualification drift must not reach durable ingest"
        );
    }

    #[test]
    fn query_policy_reserves_capacity_for_authoritative_provider_projection() {
        let trust = trust_policy();
        let ingest = ingest_policy(&trust);
        assert!(matches!(
            ReputationFinalizedQueryPolicyV1::try_new(&ingest, "query-a", 32, 516),
            Err(ReputationRuntimeError::InvalidRuntimePolicy)
        ));
        ReputationFinalizedQueryPolicyV1::try_new(&ingest, "query-a", 32, 517)
            .expect("exact worst-case provider-page budget");
    }

    #[test]
    fn exact_journal_view_validation_rejects_malformed_bootstrap_projection() {
        let chain_id = ChainId::from("reputation-runtime-test");
        let authority_policy =
            authority_record(journal_authority_policy(), FINALIZED_AT_MS - 1_000);
        let view = delivery_view(
            10,
            [0x81; 32],
            FINALIZED_AT_MS,
            authority_policy.clone(),
            Vec::new(),
        );
        view.validate_for_request(&chain_id, None, 1, u64::MAX)
            .expect("valid exact bootstrap view");
        assert!(matches!(
            view.validate_for_request(&chain_id, None, 0, u64::MAX),
            Err(ReputationRuntimeError::QueryResourceExhausted)
        ));
        assert!(matches!(
            view.validate_for_request(&chain_id, None, 1, 9),
            Err(ReputationRuntimeError::FinalizedAnchorPastTarget)
        ));

        let mut malformed_continuation = view.clone();
        malformed_continuation.journal_page.has_more = true;
        assert!(matches!(
            malformed_continuation.validate_for_request(&chain_id, None, 1, u64::MAX),
            Err(ReputationRuntimeError::InvalidQueryPage)
        ));

        let mut missing_policy_history = view.clone();
        missing_policy_history.authority_policy_history.clear();
        assert!(matches!(
            missing_policy_history.validate_for_request(&chain_id, None, 1, u64::MAX),
            Err(ReputationRuntimeError::AuthorityPolicyLineage)
        ));

        let mut future_policy = view.clone();
        future_policy.authority_policy =
            authority_record(journal_authority_policy(), FINALIZED_AT_MS + 1);
        assert!(matches!(
            future_policy.validate_for_request(&chain_id, None, 1, u64::MAX),
            Err(ReputationRuntimeError::InvalidAuthorityPolicy)
        ));

        let mut mismatched_cursor = view;
        mismatched_cursor
            .journal_page
            .finalized_cursor
            .finalized_at_unix_ms += 1;
        assert!(matches!(
            mismatched_cursor.validate_for_request(&chain_id, None, 1, u64::MAX),
            Err(ReputationRuntimeError::InvalidQueryPage)
        ));
    }

    #[test]
    fn continuation_validation_rejects_replay_and_anchor_substitution() {
        let target = ReputationFinalizedIdentityV1 {
            height: 10,
            block_hash: [0x81; 32],
        };
        let previous = ReputationCommittedEventIdentityV1 {
            sequence: 4,
            block_height: 9,
            block_hash: [0x82; 32],
            event_index: 0,
        };
        assert!(matches!(
            validate_event_page(
                target,
                Some(previous),
                1,
                true,
                Some(previous),
                Some(previous),
                target.height,
                target.block_hash,
                32,
            ),
            Err(ReputationRuntimeError::InvalidQueryContinuation)
        ));
        let next = ReputationCommittedEventIdentityV1 {
            sequence: 5,
            ..previous
        };
        assert!(matches!(
            validate_event_page(
                target,
                Some(previous),
                1,
                false,
                None,
                Some(next),
                target.height,
                [0x99; 32],
                32,
            ),
            Err(ReputationRuntimeError::InvalidQueryPage)
        ));
    }

    #[test]
    fn producer_rejects_first_seen_material_before_policy_record_initialization() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), producer_policy())
                .expect("producer outbox"),
        );
        assert!(matches!(
            PorReputationJournalProducerV1::new(outbox)
                .enqueue_terminal(provider(9), verified_por(0x10)),
            Err(ReputationRuntimeError::AuthorityPolicyLineage)
        ));
    }

    #[test]
    fn counted_token_adapter_filters_unattributable_attempts() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            producer_policy(),
        ));
        let producer = CountedStreamTokenReputationJournalProducerV1::new(outbox.clone());
        assert_eq!(
            producer
                .enqueue_counted(provider(9), excluded_token(1))
                .expect("valid excluded token"),
            CountedStreamTokenProducerOutcomeV1::NotCounted
        );
        assert!(outbox.pending(8).expect("pending").is_empty());
        assert_eq!(
            producer
                .enqueue_counted(provider(8), excluded_token(1))
                .expect("exact excluded replay ignores unattributable provider input"),
            CountedStreamTokenProducerOutcomeV1::NotCounted
        );
        let mut substituted_excluded = excluded_token(1);
        substituted_excluded.binding.request_context_digest[0] ^= 1;
        assert_eq!(
            producer
                .enqueue_counted(provider(9), substituted_excluded)
                .expect("excluded material never enters durable sequence state"),
            CountedStreamTokenProducerOutcomeV1::NotCounted
        );
        {
            let state = outbox.state.lock().expect("producer state");
            assert!(state.checkpoint.stream_token_gateway_heads.is_empty());
            assert!(state.checkpoint.stream_token_gateway_admissions.is_empty());
        }

        let token_source_time_unix_ms = FINALIZED_AT_MS - 125;
        let mut token = counted_token(2, 3);
        token.validated_at_unix_ms = token_source_time_unix_ms;
        let CountedStreamTokenProducerOutcomeV1::Enqueued(
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id },
        ) = producer
            .enqueue_counted(provider(9), token)
            .expect("counted token")
        else {
            panic!("counted token must enter the native outbox exactly once");
        };
        assert_eq!(outbox.pending(8).expect("pending").len(), 1);
        let submission = outbox
            .begin_submission(
                event_id,
                ReputationFinalizedIdentityV1 {
                    height: 10,
                    block_hash: [0x79; 32],
                },
            )
            .expect("begin counted-token submission");
        let ReputationJournalAppendInstructionV1::StreamToken(instruction) = submission.instruction
        else {
            panic!("counted-token producer must emit only the native token append");
        };
        assert_eq!(
            instruction.entry().source_time_unix_ms,
            token_source_time_unix_ms
        );
        let ReputationJournalPayloadV1::StreamTokenValidation(outcome) =
            &instruction.entry().payload
        else {
            panic!("token append must retain the typed observation");
        };
        assert_eq!(outcome.validated_at_unix_ms, token_source_time_unix_ms);
    }

    #[test]
    fn finalized_excluded_tokens_never_create_or_advance_gateway_admissions() {
        let temp = TempDir::new().expect("tempdir");
        let policy = producer_policy();
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            policy.clone(),
        ));
        let gateway_id = [0x91; 32];
        let mut excluded_before = excluded_token(0x91);
        excluded_before.binding.gateway_id = gateway_id;
        let mut counted = counted_token(0x91, 0x62);
        counted.binding.gateway_sequence = 2;
        let counted_event_id = stream_token_entry(counted).event_id;
        let mut excluded_after = excluded_token(0x91);
        excluded_after.binding.gateway_id = gateway_id;
        excluded_after.binding.gateway_sequence = 3;
        excluded_after.binding.request_context_digest = [0x63; 32];
        let block_hash = [0xA8; 32];
        outbox
            .reconcile_finalized_journal_page(ReputationJournalFinalizedEventPageV1 {
                finalized_cursor: ReputationJournalFinalizedCursorV1 {
                    height: 10,
                    block_hash,
                    finalized_at_unix_ms: FINALIZED_AT_MS.saturating_add(100),
                },
                events: vec![
                    finalized_journal_event(
                        1,
                        10,
                        block_hash,
                        0,
                        stream_token_entry(excluded_before),
                    ),
                    finalized_journal_event(2, 10, block_hash, 1, stream_token_entry(counted)),
                    finalized_journal_event(
                        3,
                        10,
                        block_hash,
                        2,
                        stream_token_entry(excluded_after),
                    ),
                ],
                has_more: false,
                next_after: None,
            })
            .expect("reconcile counted and excluded finalized rows");
        {
            let state = outbox.state.lock().expect("producer state");
            assert_eq!(state.checkpoint.observed.len(), 3);
            assert_eq!(state.checkpoint.stream_token_gateway_heads.len(), 1);
            assert_eq!(state.checkpoint.stream_token_gateway_admissions.len(), 1);
            let head = state
                .checkpoint
                .stream_token_gateway_heads
                .first()
                .expect("counted gateway head");
            let _: ReputationJournalEventIdV1 = head.event_id;
            assert_eq!(head.binding.gateway_sequence, 2);
            assert_eq!(head.event_id, counted_event_id);
            let retained = state
                .checkpoint
                .stream_token_gateway_admissions
                .first()
                .expect("counted replay admission");
            assert_eq!(retained.binding, counted.binding);
            assert_eq!(retained.entry.event_id, counted_event_id);
        }
        drop(outbox);
        let restored = ReputationJournalProducerOutboxV1::open(temp.path(), policy)
            .expect("restore excluded-safe checkpoint");
        let state = restored.state.lock().expect("restored producer state");
        assert_eq!(
            state
                .checkpoint
                .stream_token_gateway_heads
                .first()
                .expect("restored counted head")
                .binding
                .gateway_sequence,
            2
        );
        assert_eq!(state.checkpoint.stream_token_gateway_admissions.len(), 1);
    }

    #[test]
    fn late_finalization_retains_repeated_context_by_consensus_gateway_sequence() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            producer_policy(),
        ));
        let gateway_id = [0x92; 32];
        let repeated_context = [0x64; 32];
        let mut later_gateway_sequence = counted_token(0x92, 0x64);
        later_gateway_sequence.binding.gateway_id = gateway_id;
        later_gateway_sequence.binding.gateway_sequence = 2;
        later_gateway_sequence.binding.request_context_digest = repeated_context;
        let later_gateway_entry = stream_token_entry(later_gateway_sequence);
        let later_gateway_event_id = later_gateway_entry.event_id;
        outbox
            .reconcile_finalized_journal_page(ReputationJournalFinalizedEventPageV1 {
                finalized_cursor: ReputationJournalFinalizedCursorV1 {
                    height: 10,
                    block_hash: [0xB8; 32],
                    finalized_at_unix_ms: FINALIZED_AT_MS.saturating_add(100),
                },
                events: vec![finalized_journal_event(
                    1,
                    10,
                    [0xB8; 32],
                    0,
                    later_gateway_entry,
                )],
                has_more: false,
                next_after: None,
            })
            .expect("reconcile sequence two before sequence one");

        let mut earlier_gateway_sequence = later_gateway_sequence;
        earlier_gateway_sequence.binding.gateway_sequence = 1;
        let earlier_gateway_entry = stream_token_entry(earlier_gateway_sequence);
        let earlier_gateway_event_id = earlier_gateway_entry.event_id;
        outbox
            .reconcile_finalized_journal_page(ReputationJournalFinalizedEventPageV1 {
                finalized_cursor: ReputationJournalFinalizedCursorV1 {
                    height: 11,
                    block_hash: [0xB9; 32],
                    finalized_at_unix_ms: FINALIZED_AT_MS.saturating_add(200),
                },
                events: vec![finalized_journal_event(
                    2,
                    11,
                    [0xB9; 32],
                    0,
                    earlier_gateway_entry,
                )],
                has_more: false,
                next_after: None,
            })
            .expect("late older gateway sequence with repeated context");
        {
            let state = outbox.state.lock().expect("producer state");
            assert_eq!(
                state
                    .checkpoint
                    .stream_token_gateway_admissions
                    .iter()
                    .map(|admission| (
                        admission.binding.gateway_sequence,
                        admission.binding.request_context_digest,
                    ))
                    .collect::<Vec<_>>(),
                vec![(1, repeated_context), (2, repeated_context)]
            );
            assert_eq!(
                state
                    .checkpoint
                    .stream_token_gateway_admissions
                    .iter()
                    .map(|admission| admission.event_id)
                    .collect::<Vec<_>>(),
                vec![earlier_gateway_event_id, later_gateway_event_id]
            );
            let head = state
                .checkpoint
                .stream_token_gateway_heads
                .first()
                .expect("gateway head");
            assert_eq!(head.binding.gateway_sequence, 2);
            assert_eq!(head.event_id, later_gateway_event_id);
        }
    }

    #[test]
    fn counted_validation_allocates_atomic_gateway_sequences_and_recovers() {
        let temp = TempDir::new().expect("tempdir");
        let policy = producer_policy();
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            policy.clone(),
        ));
        let producer = CountedStreamTokenReputationJournalProducerV1::new(Arc::clone(&outbox));
        let gateway_id = [0x91; 32];

        let mut workers = Vec::new();
        for index in 0_u8..16 {
            let producer = producer.clone();
            workers.push(std::thread::spawn(move || {
                let context = counted_request_context(&format!("concurrent-nonce-{index:02}"));
                producer.enqueue_validation(
                    gateway_id,
                    &context,
                    counted_validation(
                        FINALIZED_AT_MS - 500 + u64::from(index),
                        StreamTokenValidationStatusV1::Accepted,
                    ),
                )
            }));
        }
        for worker in workers {
            assert!(matches!(
                worker.join().expect("join validation worker"),
                Ok(ReputationJournalEnqueueOutcomeV1::Inserted { .. })
            ));
        }

        let mut gateway_sequences = {
            let state = outbox.state.lock().expect("producer state");
            state
                .checkpoint
                .pending
                .iter()
                .filter_map(|delivery| {
                    let ReputationJournalPayloadV1::StreamTokenValidation(outcome) =
                        &delivery.entry.payload
                    else {
                        return None;
                    };
                    Some(outcome.binding.gateway_sequence)
                })
                .collect::<Vec<_>>()
        };
        gateway_sequences.sort_unstable();
        assert_eq!(gateway_sequences, (1_u64..=16).collect::<Vec<_>>());

        let (latest_context, earlier_context) = {
            let state = outbox.state.lock().expect("producer state");
            let latest = state
                .checkpoint
                .stream_token_gateway_heads
                .iter()
                .find(|head| head.binding.gateway_id == gateway_id)
                .expect("gateway head");
            let latest_context = state
                .checkpoint
                .pending
                .iter()
                .find_map(|delivery| {
                    let ReputationJournalPayloadV1::StreamTokenValidation(outcome) =
                        &delivery.entry.payload
                    else {
                        return None;
                    };
                    (outcome.binding == latest.binding).then(|| {
                        (
                            latest.binding.request_context_digest,
                            delivery.entry.event_id,
                        )
                    })
                })
                .expect("latest pending gateway event");
            let earlier = state
                .checkpoint
                .pending
                .iter()
                .find_map(|delivery| {
                    let ReputationJournalPayloadV1::StreamTokenValidation(outcome) =
                        &delivery.entry.payload
                    else {
                        return None;
                    };
                    (outcome.binding.gateway_id == gateway_id && outcome.binding != latest.binding)
                        .then(|| {
                            (
                                outcome.binding.request_context_digest,
                                delivery.entry.event_id,
                                delivery.entry.clone(),
                            )
                        })
                })
                .expect("earlier pending gateway event");
            (latest_context, earlier)
        };
        let earlier_replay_context = (0_u8..16)
            .map(|index| counted_request_context(&format!("concurrent-nonce-{index:02}")))
            .find(|context| context.digest().ok() == Some(earlier_context.0))
            .expect("recover earlier exact request context");
        assert_eq!(
            producer
                .enqueue_validation(
                    gateway_id,
                    &earlier_replay_context,
                    counted_validation(
                        FINALIZED_AT_MS + 200,
                        StreamTokenValidationStatusV1::Accepted,
                    ),
                )
                .expect("non-head retry is retained"),
            ReputationJournalEnqueueOutcomeV1::ExactReplay {
                event_id: earlier_context.1
            }
        );
        outbox
            .reconcile_finalized_journal_page(ReputationJournalFinalizedEventPageV1 {
                finalized_cursor: ReputationJournalFinalizedCursorV1 {
                    height: 10,
                    block_hash: [0xA4; 32],
                    finalized_at_unix_ms: FINALIZED_AT_MS + 220,
                },
                events: vec![ReputationJournalFinalizedEventV1 {
                    sequence: 1,
                    block_height: 10,
                    block_hash: [0xA4; 32],
                    event_index: 0,
                    recorded_at_unix_ms: FINALIZED_AT_MS + 210,
                    entry: earlier_context.2,
                }],
                has_more: false,
                next_after: None,
            })
            .expect("finalize earlier retained validation");
        assert_eq!(
            producer
                .enqueue_validation(
                    gateway_id,
                    &earlier_replay_context,
                    counted_validation(
                        FINALIZED_AT_MS + 230,
                        StreamTokenValidationStatusV1::Accepted,
                    ),
                )
                .expect("completed non-head retry is retained"),
            ReputationJournalEnqueueOutcomeV1::ExactReplay {
                event_id: earlier_context.1
            }
        );
        let replay_context = (0_u8..16)
            .map(|index| counted_request_context(&format!("concurrent-nonce-{index:02}")))
            .find(|context| context.digest().ok() == Some(latest_context.0))
            .expect("recover latest exact request context");
        assert_eq!(
            producer
                .enqueue_validation(
                    gateway_id,
                    &replay_context,
                    counted_validation(
                        FINALIZED_AT_MS + 250,
                        StreamTokenValidationStatusV1::Accepted,
                    ),
                )
                .expect("latest retry is an exact replay"),
            ReputationJournalEnqueueOutcomeV1::ExactReplay {
                event_id: latest_context.1
            }
        );
        assert_eq!(outbox.status().expect("status").ready, 15);
        assert_eq!(outbox.status().expect("status").completed, 1);

        assert!(matches!(
            producer.enqueue_validation(
                gateway_id,
                &replay_context,
                counted_validation(
                    FINALIZED_AT_MS + 251,
                    StreamTokenValidationStatusV1::ProviderViolation(
                        iroha_data_model::sorafs::reputation::StreamTokenViolationKindV1::RequestQuotaExceeded,
                    ),
                ),
            ),
            Err(ReputationRuntimeError::JournalSourceConflict)
        ));

        drop(producer);
        drop(outbox);
        let restored = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), policy)
                .expect("restore producer checkpoint"),
        );
        CountedStreamTokenReputationJournalProducerV1::new(Arc::clone(&restored))
            .enqueue_validation(
                gateway_id,
                &counted_request_context("restart-nonce-17"),
                counted_validation(
                    FINALIZED_AT_MS + 300,
                    StreamTokenValidationStatusV1::Accepted,
                ),
            )
            .expect("post-restart validation");
        let state = restored.state.lock().expect("restored producer state");
        assert_eq!(
            state
                .checkpoint
                .stream_token_gateway_heads
                .iter()
                .find(|head| head.binding.gateway_id == gateway_id)
                .expect("restored gateway head")
                .binding
                .gateway_sequence,
            17
        );
    }

    #[test]
    fn counted_validation_saturation_does_not_advance_gateway_head() {
        let temp = TempDir::new().expect("tempdir");
        let mut policy = producer_policy();
        policy.max_pending = 1;
        let outbox = Arc::new(open_initialized_producer_outbox(temp.path(), policy));
        let producer = CountedStreamTokenReputationJournalProducerV1::new(Arc::clone(&outbox));
        let gateway_id = [0x92; 32];
        producer
            .enqueue_validation(
                gateway_id,
                &counted_request_context("capacity-nonce-01"),
                counted_validation(
                    FINALIZED_AT_MS - 100,
                    StreamTokenValidationStatusV1::Accepted,
                ),
            )
            .expect("first validation");
        assert!(matches!(
            producer.enqueue_validation(
                gateway_id,
                &counted_request_context("capacity-nonce-02"),
                counted_validation(
                    FINALIZED_AT_MS - 99,
                    StreamTokenValidationStatusV1::Accepted,
                ),
            ),
            Err(ReputationRuntimeError::JournalResourceExhausted)
        ));
        let state = outbox.state.lock().expect("producer state");
        let head = state
            .checkpoint
            .stream_token_gateway_heads
            .iter()
            .find(|head| head.binding.gateway_id == gateway_id)
            .expect("gateway head");
        assert_eq!(head.binding.gateway_sequence, 1);
        assert_eq!(state.checkpoint.pending.len(), 1);
    }

    #[test]
    fn bounded_gateway_replay_suffix_keeps_newest_rows_and_every_head() {
        let temp = TempDir::new().expect("tempdir");
        let mut policy = producer_policy();
        policy.max_completed = 2;
        let outbox = Arc::new(open_initialized_producer_outbox(temp.path(), policy));
        let producer = CountedStreamTokenReputationJournalProducerV1::new(Arc::clone(&outbox));
        let first = counted_token(0x93, 1);
        let first_event_id = match producer
            .enqueue_counted(provider(9), first)
            .expect("first gateway row")
        {
            CountedStreamTokenProducerOutcomeV1::Enqueued(
                ReputationJournalEnqueueOutcomeV1::Inserted { event_id },
            ) => event_id,
            other => panic!("unexpected first admission: {other:?}"),
        };
        for sequence in 2_u64..=3 {
            let mut outcome = counted_token(0x93, u8::try_from(sequence).expect("small sequence"));
            outcome.binding.gateway_sequence = sequence;
            producer
                .enqueue_counted(provider(9), outcome)
                .expect("newer gateway row");
        }
        let state = outbox.state.lock().expect("producer state");
        assert_eq!(
            state
                .checkpoint
                .stream_token_gateway_admissions
                .iter()
                .map(|admission| admission.binding.gateway_sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3],
            "the token replay cap must be independent of max_completed"
        );
        let mut candidate = state.checkpoint.clone();
        let admissions = std::mem::take(&mut candidate.stream_token_gateway_admissions);
        for admission in admissions {
            retain_stream_token_gateway_admission_suffix(&mut candidate, admission, 2)
                .expect("bounded replay retention");
        }
        assert_eq!(
            candidate
                .stream_token_gateway_admissions
                .iter()
                .map(|admission| admission.binding.gateway_sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        let first_admission = state
            .checkpoint
            .stream_token_gateway_admissions
            .iter()
            .find(|admission| admission.event_id == first_event_id)
            .expect("first admission")
            .clone();
        retain_stream_token_gateway_admission_suffix(&mut candidate, first_admission, 2)
            .expect("an evicted old row cannot displace newer history");
        assert_eq!(
            candidate
                .stream_token_gateway_admissions
                .iter()
                .map(|admission| admission.binding.gateway_sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        let head = candidate
            .stream_token_gateway_heads
            .iter()
            .find(|head| head.binding.gateway_id == first.binding.gateway_id)
            .expect("gateway head");
        assert_eq!(head.binding.gateway_sequence, 3);
        assert!(
            candidate
                .stream_token_gateway_admissions
                .iter()
                .any(|admission| admission.binding == head.binding
                    && admission.event_id == head.event_id)
        );
        drop(state);
    }

    #[test]
    fn bounded_gateway_replay_suffix_pins_every_head_across_gateways() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            producer_policy(),
        ));
        let producer = CountedStreamTokenReputationJournalProducerV1::new(Arc::clone(&outbox));
        for gateway_marker in [0x95, 0x96] {
            let first = counted_token(gateway_marker, 1);
            producer
                .enqueue_counted(provider(9), first)
                .expect("first gateway row");
            let mut second = counted_token(gateway_marker, 2);
            second.binding.gateway_sequence = 2;
            producer
                .enqueue_counted(provider(9), second)
                .expect("second gateway row");
        }
        let state = outbox.state.lock().expect("producer state");
        let mut candidate = state.checkpoint.clone();
        let admissions = std::mem::take(&mut candidate.stream_token_gateway_admissions);
        for admission in admissions {
            retain_stream_token_gateway_admission_suffix(&mut candidate, admission, 3)
                .expect("bounded multi-gateway replay retention");
        }
        assert_eq!(candidate.stream_token_gateway_admissions.len(), 3);
        for head in &candidate.stream_token_gateway_heads {
            assert!(
                candidate
                    .stream_token_gateway_admissions
                    .iter()
                    .any(|admission| admission.binding == head.binding
                        && admission.event_id == head.event_id),
                "each gateway head must remain pinned"
            );
        }
        assert_eq!(
            candidate
                .stream_token_gateway_admissions
                .iter()
                .map(|admission| (
                    admission.binding.gateway_id,
                    admission.binding.gateway_sequence,
                ))
                .collect::<Vec<_>>(),
            vec![([0x95; 32], 2), ([0x96; 32], 1), ([0x96; 32], 2)]
        );
    }

    #[test]
    fn gateway_admission_checkpoint_rejects_canonical_and_head_cross_link_tampering() {
        let temp = TempDir::new().expect("tempdir");
        let policy = producer_policy();
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            policy.clone(),
        ));
        let producer = CountedStreamTokenReputationJournalProducerV1::new(Arc::clone(&outbox));
        let first = counted_token(0x94, 1);
        let first_event_id = match producer
            .enqueue_counted(provider(9), first)
            .expect("first admission")
        {
            CountedStreamTokenProducerOutcomeV1::Enqueued(
                ReputationJournalEnqueueOutcomeV1::Inserted { event_id },
            ) => event_id,
            other => panic!("unexpected first admission: {other:?}"),
        };
        let mut second = counted_token(0x94, 2);
        second.binding.gateway_sequence = 2;
        producer
            .enqueue_counted(provider(9), second)
            .expect("second admission");
        let checkpoint = outbox
            .state
            .lock()
            .expect("producer state")
            .checkpoint
            .clone();

        let authority_policy = journal_authority_policy();
        let substituted_entry = ReputationJournalEntryV1::try_new(
            provider(8),
            authority_policy
                .canonical_digest()
                .expect("authority policy digest"),
            authority_policy.token_recorder_authority,
            first.validated_at_unix_ms,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(first),
        )
        .expect("canonical substituted admission entry");
        let mut canonical_tamper = checkpoint.clone();
        let substituted_admission = &mut canonical_tamper.stream_token_gateway_admissions[0];
        substituted_admission.admission_digest =
            stream_token_admission_digest(provider(8), &first).expect("substituted digest");
        substituted_admission.event_id = substituted_entry.event_id;
        substituted_admission.entry = substituted_entry;
        assert!(matches!(
            validate_journal_checkpoint_structure(&canonical_tamper, &policy, outbox.policy_digest,),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));

        let mut cross_link_tamper = checkpoint;
        cross_link_tamper.stream_token_gateway_heads[0].event_id = first_event_id;
        assert!(matches!(
            validate_journal_checkpoint_structure(
                &cross_link_tamper,
                &policy,
                outbox.policy_digest,
            ),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn producer_rejects_same_source_with_substituted_material() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            producer_policy(),
        ));
        let producer = CountedStreamTokenReputationJournalProducerV1::new(outbox);
        producer
            .enqueue_counted(provider(9), counted_token(4, 5))
            .expect("first token");
        assert!(matches!(
            producer.enqueue_counted(provider(9), counted_token(4, 6)),
            Err(ReputationRuntimeError::JournalSourceConflict)
        ));
    }

    #[test]
    fn stream_token_gateway_head_survives_restart_and_replays_only_exact_history() {
        let temp = TempDir::new().expect("tempdir");
        let policy = producer_policy();
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            policy.clone(),
        ));
        let producer = CountedStreamTokenReputationJournalProducerV1::new(outbox.clone());
        let first = counted_token(4, 5);
        let first_event_id = match producer
            .enqueue_counted(provider(9), first)
            .expect("first gateway sequence")
        {
            CountedStreamTokenProducerOutcomeV1::Enqueued(
                ReputationJournalEnqueueOutcomeV1::Inserted { event_id },
            ) => event_id,
            other => panic!("unexpected first admission: {other:?}"),
        };
        let mut second = counted_token(4, 6);
        second.binding.gateway_sequence = 2;
        let second_event_id = match producer
            .enqueue_counted(provider(9), second)
            .expect("strictly newer gateway sequence")
        {
            CountedStreamTokenProducerOutcomeV1::Enqueued(
                ReputationJournalEnqueueOutcomeV1::Inserted { event_id },
            ) => event_id,
            other => panic!("unexpected second admission: {other:?}"),
        };
        let first_entry = outbox
            .state
            .lock()
            .expect("journal state")
            .checkpoint
            .pending
            .iter()
            .find(|delivery| delivery.entry.event_id == first_event_id)
            .expect("first pending entry")
            .entry
            .clone();
        assert_eq!(
            outbox
                .reconcile_finalized_journal_page(ReputationJournalFinalizedEventPageV1 {
                    finalized_cursor: ReputationJournalFinalizedCursorV1 {
                        height: 10,
                        block_hash: [0xA5; 32],
                        finalized_at_unix_ms: FINALIZED_AT_MS.saturating_add(100),
                    },
                    events: vec![ReputationJournalFinalizedEventV1 {
                        sequence: 1,
                        block_height: 10,
                        block_hash: [0xA5; 32],
                        event_index: 0,
                        recorded_at_unix_ms: FINALIZED_AT_MS.saturating_add(50),
                        entry: first_entry,
                    }],
                    has_more: false,
                    next_after: None,
                })
                .expect("older pending sequence may finalize after a newer admission"),
            1
        );
        assert_eq!(outbox.status().expect("status").ready, 1);
        assert_eq!(outbox.status().expect("status").completed, 1);
        drop(producer);
        drop(outbox);

        let restored = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), policy.clone())
                .expect("restore producer outbox"),
        );
        let producer = CountedStreamTokenReputationJournalProducerV1::new(restored.clone());
        assert_eq!(
            producer
                .enqueue_counted(provider(9), second)
                .expect("latest exact replay"),
            CountedStreamTokenProducerOutcomeV1::Enqueued(
                ReputationJournalEnqueueOutcomeV1::ExactReplay {
                    event_id: second_event_id
                }
            )
        );

        let mut substituted_context = second;
        substituted_context.binding.request_context_digest[0] ^= 1;
        assert!(matches!(
            producer.enqueue_counted(provider(9), substituted_context),
            Err(ReputationRuntimeError::JournalSourceConflict)
        ));
        let mut substituted_outcome = second;
        substituted_outcome.status = StreamTokenValidationStatusV1::ProviderViolation(
            iroha_data_model::sorafs::reputation::StreamTokenViolationKindV1::RequestQuotaExceeded,
        );
        assert!(matches!(
            producer.enqueue_counted(provider(9), substituted_outcome),
            Err(ReputationRuntimeError::JournalSourceConflict)
        ));
        assert!(matches!(
            producer.enqueue_counted(provider(8), second),
            Err(ReputationRuntimeError::JournalSourceConflict)
        ));
        assert_eq!(
            producer
                .enqueue_counted(provider(9), first)
                .expect("retained older exact replay"),
            CountedStreamTokenProducerOutcomeV1::Enqueued(
                ReputationJournalEnqueueOutcomeV1::ExactReplay {
                    event_id: first_event_id
                }
            )
        );
        let mut stale_substituted = first;
        stale_substituted.status = StreamTokenValidationStatusV1::ProviderViolation(
            iroha_data_model::sorafs::reputation::StreamTokenViolationKindV1::RequestQuotaExceeded,
        );
        assert!(matches!(
            producer.enqueue_counted(provider(9), stale_substituted),
            Err(ReputationRuntimeError::JournalSourceConflict)
        ));

        let mut third = second;
        third.binding.gateway_sequence = 3;
        third.binding.request_context_digest = [0x77; 32];
        third.validated_at_unix_ms = FINALIZED_AT_MS.saturating_add(260);
        let third_pre_rotation_event_id = match producer
            .enqueue_counted(provider(9), third)
            .expect("strictly increasing gateway sequence")
        {
            CountedStreamTokenProducerOutcomeV1::Enqueued(
                ReputationJournalEnqueueOutcomeV1::Inserted { event_id },
            ) => event_id,
            other => panic!("unexpected third admission: {other:?}"),
        };

        let current_policy = policy.authority_policy.clone();
        restored
            .synchronize_authority_policy(
                authority_record(current_policy.clone(), FINALIZED_AT_MS - 1_000),
                ReputationJournalFinalizedCursorV1 {
                    height: 11,
                    block_hash: [0xA6; 32],
                    finalized_at_unix_ms: FINALIZED_AT_MS.saturating_add(200),
                },
            )
            .expect("initialize active policy record");
        let mut successor = current_policy.clone();
        successor.revision = successor.revision.saturating_add(1);
        successor.predecessor_policy_digest =
            Some(current_policy.canonical_digest().expect("current digest"));
        successor.token_recorder_authority = account(0x71);
        let successor_authority = successor.token_recorder_authority.clone();
        assert_eq!(
            restored
                .synchronize_authority_policy(
                    authority_record(successor, FINALIZED_AT_MS.saturating_add(250)),
                    ReputationJournalFinalizedCursorV1 {
                        height: 12,
                        block_hash: [0xA7; 32],
                        finalized_at_unix_ms: FINALIZED_AT_MS.saturating_add(300),
                    },
                )
                .expect("rotate multiple ready sequences behind one gateway head"),
            ReputationJournalPolicySyncOutcomeV1::Rotated { rebound_ready: 1 }
        );
        let state = restored.state.lock().expect("rotated producer state");
        let rebound = state
            .checkpoint
            .pending
            .iter()
            .find(|delivery| {
                matches!(
                    &delivery.entry.payload,
                    ReputationJournalPayloadV1::StreamTokenValidation(outcome)
                        if outcome.binding == third.binding
                )
            })
            .expect("rebound stream-token row");
        assert_ne!(rebound.entry.event_id, third_pre_rotation_event_id);
        assert_eq!(rebound.entry.recorded_by, successor_authority);
        let head = state
            .checkpoint
            .stream_token_gateway_heads
            .iter()
            .find(|head| head.binding == third.binding)
            .expect("rebound gateway head");
        assert_eq!(head.event_id, rebound.entry.event_id);
        let admission = state
            .checkpoint
            .stream_token_gateway_admissions
            .iter()
            .find(|admission| admission.binding == third.binding)
            .expect("rebound gateway admission");
        assert_eq!(admission.event_id, rebound.entry.event_id);
        assert_eq!(admission.entry, rebound.entry);
    }

    #[test]
    fn ambiguous_journal_append_survives_restart_and_requires_later_finality() {
        let temp = TempDir::new().expect("tempdir");
        let policy = producer_policy();
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            policy.clone(),
        ));
        let producer = PorReputationJournalProducerV1::new(outbox.clone());
        let mut terminal = verified_por(8);
        terminal.decided_at_unix_ms = FINALIZED_AT_MS - 250;
        let source_time_unix_ms = terminal.decided_at_unix_ms;
        let event_id = match producer
            .enqueue_terminal(provider(7), terminal)
            .expect("enqueue PoR")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let submitted_instruction = outbox
            .begin_submission(
                event_id,
                ReputationFinalizedIdentityV1 {
                    height: 10,
                    block_hash: [0x91; 32],
                },
            )
            .expect("begin submission");
        let ReputationJournalAppendInstructionV1::Por(instruction) =
            &submitted_instruction.instruction
        else {
            panic!("PoR producer must emit only the native PoR append");
        };
        assert_eq!(instruction.entry().source_time_unix_ms, source_time_unix_ms);
        let ReputationJournalPayloadV1::PorTerminal(outcome) = &instruction.entry().payload else {
            panic!("PoR append must retain the typed terminal");
        };
        assert_eq!(outcome.decided_at_unix_ms, source_time_unix_ms);
        outbox.mark_submitted(event_id).expect("submitted");
        drop(producer);
        drop(outbox);

        let restored =
            ReputationJournalProducerOutboxV1::open(temp.path(), policy).expect("restore outbox");
        let restored_submission = {
            let state = restored.state.lock().expect("restored journal state");
            state
                .checkpoint
                .pending
                .iter()
                .find(|delivery| delivery.entry.event_id == event_id)
                .expect("restored submitted delivery")
                .submission(&restored.policy.chain_id)
                .expect("restore exact submitted instruction")
        };
        assert_eq!(
            restored_submission.instruction,
            submitted_instruction.instruction
        );
        assert_eq!(
            restored.pending(1).expect("pending")[0].state,
            ReputationJournalDeliveryStateV1::Submitted
        );
        assert!(matches!(
            restored.mark_finalized_absent(
                event_id,
                ReputationFinalizedIdentityV1 {
                    height: 10,
                    block_hash: [0x91; 32],
                },
                [0x92; 32],
            ),
            Err(ReputationRuntimeError::FinalizedAbsenceNotProven)
        ));
        assert!(matches!(
            restored.mark_finalized_absent(
                event_id,
                ReputationFinalizedIdentityV1 {
                    height: 11,
                    block_hash: [0x93; 32],
                },
                [0x94; 32],
            ),
            Err(ReputationRuntimeError::FinalizedAbsenceNotProven)
        ));
        reconcile_empty_journal(
            &restored,
            11,
            [0x93; 32],
            FINALIZED_AT_MS.saturating_add(1_000),
        );
        assert!(matches!(
            restored
                .mark_finalized_absent(
                    event_id,
                    ReputationFinalizedIdentityV1 {
                        height: 11,
                        block_hash: [0x93; 32],
                    },
                    [0x94; 32],
                )
                .expect("later absence"),
            ReputationJournalDeliveryOutcomeV1::RetryReady { attempts: 1 }
        ));
    }

    #[test]
    fn committed_journal_ack_is_exactly_once_and_fork_safe() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            producer_policy(),
        ));
        let producer = PorReputationJournalProducerV1::new(outbox.clone());
        let event_id = match producer
            .enqueue_terminal(provider(7), verified_por(9))
            .expect("enqueue PoR")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        outbox
            .begin_submission(
                event_id,
                ReputationFinalizedIdentityV1 {
                    height: 10,
                    block_hash: [0xA1; 32],
                },
            )
            .expect("begin");
        let committed = ReputationCommittedEventIdentityV1 {
            sequence: 5,
            block_height: 11,
            block_hash: [0xA2; 32],
            event_index: 2,
        };
        assert_eq!(
            outbox
                .acknowledge_committed(event_id, committed)
                .expect("commit"),
            ReputationJournalDeliveryOutcomeV1::Committed
        );
        assert_eq!(
            outbox
                .acknowledge_committed(event_id, committed)
                .expect("replay"),
            ReputationJournalDeliveryOutcomeV1::ExactReplay
        );
        let forked = ReputationCommittedEventIdentityV1 {
            block_hash: [0xA3; 32],
            ..committed
        };
        assert!(matches!(
            outbox.acknowledge_committed(event_id, forked),
            Err(ReputationRuntimeError::JournalAcknowledgementConflict)
        ));
    }

    #[test]
    fn scanner_tombstones_peer_commit_before_local_callback_and_restart() {
        let temp = TempDir::new().expect("tempdir");
        let policy = producer_policy();
        let outbox = open_initialized_producer_outbox(temp.path(), policy.clone());
        let terminal = verified_por(0x2A);
        let entry = ReputationJournalEntryV1::try_new(
            provider(7),
            policy.authority_policy_digest,
            policy.authority_policy.por_recorder_authority.clone(),
            terminal.decided_at_unix_ms,
            None,
            ReputationJournalPayloadV1::PorTerminal(terminal.clone()),
        )
        .expect("peer entry");
        let event_id = entry.event_id;
        assert_eq!(
            outbox
                .reconcile_finalized_journal_page(ReputationJournalFinalizedEventPageV1 {
                    finalized_cursor: ReputationJournalFinalizedCursorV1 {
                        height: 10,
                        block_hash: [0xAA; 32],
                        finalized_at_unix_ms: FINALIZED_AT_MS.saturating_add(100),
                    },
                    events: vec![ReputationJournalFinalizedEventV1 {
                        sequence: 1,
                        block_height: 10,
                        block_hash: [0xAA; 32],
                        event_index: 0,
                        recorded_at_unix_ms: FINALIZED_AT_MS.saturating_add(50),
                        entry,
                    }],
                    has_more: false,
                    next_after: None,
                })
                .expect("scan peer commit"),
            0
        );
        assert_eq!(outbox.status().expect("status").completed, 1);
        drop(outbox);

        let restored = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), policy).expect("restore"),
        );
        assert_eq!(
            PorReputationJournalProducerV1::new(Arc::clone(&restored))
                .enqueue_terminal(provider(7), terminal)
                .expect("late callback"),
            ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id }
        );
        assert_eq!(restored.status().expect("status").ready, 0);
    }

    #[test]
    fn delivery_worker_treats_queue_receipt_as_non_final_until_exact_event_is_scanned() {
        let temp = TempDir::new().expect("tempdir");
        let policy = journal_authority_policy();
        let record = authority_record(policy.clone(), FINALIZED_AT_MS - 1_000);
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                policy,
            )
            .expect("producer policy"),
        ));
        let event_id = match PorReputationJournalProducerV1::new(Arc::clone(&outbox))
            .enqueue_terminal(provider(7), verified_por(0x31))
            .expect("enqueue terminal")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let query_qualification = ReputationRuntimeProviderQualificationV1::new(
            REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
            [0xC1; 32],
        );
        let query = Arc::new(ScriptedDeliveryQuery {
            handle: "ledger.finalized.primary".to_owned(),
            qualification: query_qualification,
            views: Mutex::new(VecDeque::from([delivery_view(
                10,
                [0xB1; 32],
                FINALIZED_AT_MS + 100,
                record.clone(),
                Vec::new(),
            )])),
        });
        let submitter = Arc::new(RecordingJournalSubmitter {
            handle: "queue.reputation.journal".to_owned(),
            qualification: ReputationRuntimeProviderQualificationV1::new(
                REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
                reputation_journal_submitter_policy_digest_v1(
                    &ChainId::from("reputation-runtime-test"),
                    "queue.reputation.journal",
                )
                .expect("submitter policy digest"),
            ),
            requests: Mutex::new(Vec::new()),
        });
        let worker = ReputationJournalDeliveryWorkerV1::new(
            Arc::clone(&outbox),
            ReputationJournalDeliveryPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                query.handle.clone(),
                query_qualification,
                submitter.handle.clone(),
            )
            .expect("delivery policy"),
            query.clone(),
            submitter.clone(),
        )
        .expect("worker");
        let first = worker.reconcile_once().expect("queue tick");
        assert_eq!(first.queued, 1);
        assert_eq!(first.committed, 0);
        assert_eq!(outbox.status().expect("status").submitted, 1);
        assert!(
            !worker.status().expect("worker status").ready,
            "a queue receipt must not fabricate finality"
        );

        let request = submitter.requests.lock().expect("request lock")[0].clone();
        assert_eq!(request.event_id, event_id);
        request.validate().expect("exact request validates");
        let mut tampered_request = request.clone();
        tampered_request.attempt = tampered_request.attempt.saturating_add(1);
        assert!(matches!(
            tampered_request.validate(),
            Err(ReputationRuntimeError::InvalidJournalTransition)
        ));
        let entry = match request.instruction {
            ReputationJournalAppendInstructionV1::Por(instruction) => instruction.entry().clone(),
            ReputationJournalAppendInstructionV1::StreamToken(_) => {
                panic!("PoR callback must create the PoR append")
            }
        };
        query
            .views
            .lock()
            .expect("script lock")
            .push_back(delivery_view(
                11,
                [0xB2; 32],
                FINALIZED_AT_MS + 200,
                record,
                vec![ReputationJournalFinalizedEventV1 {
                    sequence: 1,
                    block_height: 11,
                    block_hash: [0xB2; 32],
                    event_index: 0,
                    recorded_at_unix_ms: FINALIZED_AT_MS + 150,
                    entry,
                }],
            ));
        let second = worker.reconcile_once().expect("commit tick");
        assert_eq!(second.committed, 1);
        assert_eq!(outbox.status().expect("status").submitted, 0);
        assert!(worker.status().expect("worker status").ready);
    }

    #[test]
    fn policy_history_recovery_applies_one_and_multiple_missed_rotations_before_open() {
        let first_policy = journal_authority_policy();
        let second_policy = successor_authority_policy(&first_policy, 0x51);
        let third_policy = successor_authority_policy(&second_policy, 0x52);
        let first_record = authority_record(first_policy.clone(), FINALIZED_AT_MS - 1_000);
        let second_record = authority_record(second_policy.clone(), FINALIZED_AT_MS + 100);
        let third_record = authority_record(third_policy.clone(), FINALIZED_AT_MS + 200);
        let finalized = ReputationJournalFinalizedCursorV1 {
            height: 12,
            block_hash: [0xC8; 32],
            finalized_at_unix_ms: FINALIZED_AT_MS + 300,
        };

        let strict_missed = TempDir::new().expect("strict-missed tempdir");
        assert!(matches!(
            ReputationJournalProducerOutboxV1::open(
                strict_missed.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    second_policy.clone(),
                )
                .expect("second policy"),
            ),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));

        let one_missed = TempDir::new().expect("one-missed tempdir");
        let initial = ReputationJournalProducerOutboxV1::open(
            one_missed.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                first_policy.clone(),
            )
            .expect("initial policy"),
        )
        .expect("initial outbox");
        initial
            .synchronize_authority_policy(first_record.clone(), finalized)
            .expect("initialize first policy");
        drop(initial);

        let recovered = ReputationJournalProducerOutboxV1::open_with_authority_policy_history(
            one_missed.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                second_policy.clone(),
            )
            .expect("second policy"),
            &[first_record.clone(), second_record.clone()],
            finalized,
        )
        .expect("recover one missed rotation");
        assert_eq!(
            recovered
                .scan_status()
                .expect("one-missed recovery status")
                .active_authority_policy_revision,
            2
        );
        drop(recovered);
        ReputationJournalProducerOutboxV1::open(
            one_missed.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                second_policy,
            )
            .expect("strict second policy"),
        )
        .expect("ordinary strict open accepts the recovered terminal policy");

        let multiple_missed = TempDir::new().expect("multiple-missed tempdir");
        let initial = ReputationJournalProducerOutboxV1::open(
            multiple_missed.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                first_policy,
            )
            .expect("initial policy"),
        )
        .expect("initial outbox");
        initial
            .synchronize_authority_policy(first_record.clone(), finalized)
            .expect("initialize first policy");
        drop(initial);

        let recovered = ReputationJournalProducerOutboxV1::open_with_authority_policy_history(
            multiple_missed.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                third_policy.clone(),
            )
            .expect("third policy"),
            &[
                first_record.clone(),
                second_record.clone(),
                third_record.clone(),
            ],
            finalized,
        )
        .expect("recover multiple missed rotations");
        let state = recovered.state.lock().expect("recovered state");
        assert_eq!(state.checkpoint.authority_policy_records.len(), 3);
        assert_eq!(
            state.checkpoint.active_authority_policy_record,
            Some(third_record)
        );
        drop(state);
        drop(recovered);
        ReputationJournalProducerOutboxV1::open(
            multiple_missed.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                third_policy,
            )
            .expect("strict third policy"),
        )
        .expect("recovered multi-rotation checkpoint survives strict restart");
    }

    #[test]
    fn policy_history_recovery_rejects_skips_duplicates_and_substitution() {
        let first_policy = journal_authority_policy();
        let second_policy = successor_authority_policy(&first_policy, 0x61);
        let third_policy = successor_authority_policy(&second_policy, 0x62);
        let first_record = authority_record(first_policy.clone(), FINALIZED_AT_MS - 1_000);
        let second_record = authority_record(second_policy.clone(), FINALIZED_AT_MS + 100);
        let third_record = authority_record(third_policy.clone(), FINALIZED_AT_MS + 200);
        let finalized = ReputationJournalFinalizedCursorV1 {
            height: 12,
            block_hash: [0xC9; 32],
            finalized_at_unix_ms: FINALIZED_AT_MS + 300,
        };
        let temp = TempDir::new().expect("tempdir");
        let initial = ReputationJournalProducerOutboxV1::open(
            temp.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                first_policy,
            )
            .expect("first policy"),
        )
        .expect("initial outbox");
        initial
            .synchronize_authority_policy(first_record.clone(), finalized)
            .expect("initialize first record");
        drop(initial);

        assert!(matches!(
            ReputationJournalProducerOutboxV1::open_with_authority_policy_history(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    third_policy,
                )
                .expect("third policy"),
                &[first_record.clone(), third_record],
                finalized,
            ),
            Err(ReputationRuntimeError::AuthorityPolicyLineage)
        ));
        assert!(matches!(
            ReputationJournalProducerOutboxV1::open_with_authority_policy_history(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    second_policy.clone(),
                )
                .expect("second policy"),
                &[
                    first_record.clone(),
                    second_record.clone(),
                    second_record.clone(),
                ],
                finalized,
            ),
            Err(ReputationRuntimeError::AuthorityPolicyLineage)
        ));

        let recovered = ReputationJournalProducerOutboxV1::open_with_authority_policy_history(
            temp.path(),
            ReputationJournalProducerPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                second_policy.clone(),
            )
            .expect("second policy"),
            &[first_record.clone(), second_record.clone()],
            finalized,
        )
        .expect("establish retained second record");
        drop(recovered);
        let substituted_record = ReputationJournalAuthorityPolicyRecordV1::try_new(
            second_policy.clone(),
            account(0x7F),
            second_record.activated_at_unix_ms,
        )
        .expect("substituted activation metadata remains structurally valid");
        assert!(matches!(
            ReputationJournalProducerOutboxV1::open_with_authority_policy_history(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    second_policy,
                )
                .expect("second policy"),
                &[first_record, substituted_record],
                finalized,
            ),
            Err(ReputationRuntimeError::AuthorityPolicyLineage)
        ));
    }

    #[test]
    fn policy_rotation_rebinds_only_ready_rows_and_preserves_ambiguous_bytes() {
        let temp = TempDir::new().expect("tempdir");
        let first_policy = journal_authority_policy();
        let first_activation = FINALIZED_AT_MS - 1_000;
        let outbox = Arc::new(
            ReputationJournalProducerOutboxV1::open(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    first_policy.clone(),
                )
                .expect("producer policy"),
            )
            .expect("outbox"),
        );
        let first_cursor = ReputationJournalFinalizedCursorV1 {
            height: 10,
            block_hash: [0xC1; 32],
            finalized_at_unix_ms: FINALIZED_AT_MS + 100,
        };
        outbox
            .synchronize_authority_policy(
                authority_record(first_policy.clone(), first_activation),
                first_cursor,
            )
            .expect("initialize policy");
        let producer = PorReputationJournalProducerV1::new(Arc::clone(&outbox));
        let first_id = match producer
            .enqueue_terminal(provider(7), verified_por(0x41))
            .expect("first")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let second_id = match producer
            .enqueue_terminal(provider(8), verified_por(0x42))
            .expect("second")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let mut post_activation = verified_por(0x43);
        post_activation.issued_at_unix_ms = post_activation.issued_at_unix_ms.saturating_add(200);
        post_activation.deadline_at_unix_ms =
            post_activation.deadline_at_unix_ms.saturating_add(200);
        post_activation.responded_at_unix_ms = post_activation
            .responded_at_unix_ms
            .map(|responded_at| responded_at.saturating_add(200));
        post_activation.decided_at_unix_ms = post_activation.decided_at_unix_ms.saturating_add(200);
        let post_activation_id = match producer
            .enqueue_terminal(provider(9), post_activation)
            .expect("post-activation source queued before observing rotation")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let ambiguous = outbox
            .begin_submission(
                first_id,
                ReputationFinalizedIdentityV1 {
                    height: 10,
                    block_hash: [0xC1; 32],
                },
            )
            .expect("capture first bytes");

        let mut successor = first_policy.clone();
        successor.revision = 2;
        successor.predecessor_policy_digest =
            Some(first_policy.canonical_digest().expect("first digest"));
        successor.por_recorder_authority = account(0x51);
        let successor_activation = FINALIZED_AT_MS + 150;
        let successor_record = authority_record(successor.clone(), successor_activation);
        assert_eq!(
            outbox
                .synchronize_authority_policy(
                    successor_record.clone(),
                    ReputationJournalFinalizedCursorV1 {
                        height: 11,
                        block_hash: [0xC2; 32],
                        finalized_at_unix_ms: FINALIZED_AT_MS + 200,
                    },
                )
                .expect("rotate"),
            ReputationJournalPolicySyncOutcomeV1::Rotated { rebound_ready: 1 }
        );

        let late_historical_id = match producer
            .enqueue_terminal(provider(11), verified_por(0x45))
            .expect("first-seen historical source after rotation")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let mut boundary_source = verified_por(0x46);
        boundary_source.issued_at_unix_ms = boundary_source
            .issued_at_unix_ms
            .saturating_add(successor_activation - FINALIZED_AT_MS);
        boundary_source.deadline_at_unix_ms = boundary_source
            .deadline_at_unix_ms
            .saturating_add(successor_activation - FINALIZED_AT_MS);
        boundary_source.responded_at_unix_ms = boundary_source
            .responded_at_unix_ms
            .map(|timestamp| timestamp.saturating_add(successor_activation - FINALIZED_AT_MS));
        boundary_source.decided_at_unix_ms = successor_activation;
        let boundary_id = match producer
            .enqueue_terminal(provider(12), boundary_source)
            .expect("successor activation boundary")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let (late_policy_digest, late_authority, boundary_policy_digest, boundary_authority) = {
            let state = outbox.state.lock().expect("outbox state");
            let late = state
                .checkpoint
                .pending
                .iter()
                .find(|delivery| delivery.entry.event_id == late_historical_id)
                .expect("late historical row");
            let boundary = state
                .checkpoint
                .pending
                .iter()
                .find(|delivery| delivery.entry.event_id == boundary_id)
                .expect("boundary row");
            (
                late.entry.authority_policy_digest,
                late.entry.recorded_by.clone(),
                boundary.entry.authority_policy_digest,
                boundary.entry.recorded_by.clone(),
            )
        };
        assert_eq!(
            late_policy_digest,
            first_policy.canonical_digest().expect("first digest")
        );
        assert_eq!(late_authority, first_policy.por_recorder_authority);
        assert_eq!(boundary_policy_digest, successor_record.policy_digest);
        assert_eq!(boundary_authority, successor.por_recorder_authority);

        let mut predating_source = verified_por(0x47);
        let predating_delta = FINALIZED_AT_MS - first_activation + 1;
        predating_source.issued_at_unix_ms = predating_source
            .issued_at_unix_ms
            .saturating_sub(predating_delta);
        predating_source.deadline_at_unix_ms = predating_source
            .deadline_at_unix_ms
            .saturating_sub(predating_delta);
        predating_source.responded_at_unix_ms = predating_source
            .responded_at_unix_ms
            .map(|timestamp| timestamp.saturating_sub(predating_delta));
        predating_source.decided_at_unix_ms = predating_source
            .decided_at_unix_ms
            .saturating_sub(predating_delta);
        assert!(matches!(
            producer.enqueue_terminal(provider(13), predating_source),
            Err(ReputationRuntimeError::InvalidAuthorityPolicy)
        ));

        let pending = outbox.pending(8).expect("pending");
        assert!(
            pending.iter().any(|row| {
                row.event_id == ambiguous.event_id
                    && row.state == ReputationJournalDeliveryStateV1::Ambiguous
            }),
            "ambiguous exact bytes must remain immutable across rotation"
        );
        assert!(
            pending.iter().any(|row| row.event_id == second_id),
            "source material from before activation must retain its historical policy"
        );
        assert!(
            pending.iter().all(|row| row.event_id != post_activation_id),
            "a never-exposed Ready row sourced after activation must be rebound"
        );
        assert_eq!(
            producer
                .enqueue_terminal(provider(7), verified_por(0x41))
                .expect("retained source replay resolves before current-policy construction"),
            ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id: first_id }
        );
        let mut substituted_source = verified_por(0x41);
        substituted_source.decided_at_unix_ms =
            substituted_source.decided_at_unix_ms.saturating_add(1);
        assert!(matches!(
            producer.enqueue_terminal(provider(7), substituted_source),
            Err(ReputationRuntimeError::JournalSourceConflict)
        ));

        reconcile_empty_journal(&outbox, 12, [0xC3; 32], FINALIZED_AT_MS.saturating_add(300));
        outbox
            .mark_finalized_absent(
                first_id,
                ReputationFinalizedIdentityV1 {
                    height: 12,
                    block_hash: [0xC3; 32],
                },
                [0xC4; 32],
            )
            .expect("prove old append absent");
        let rebound = outbox
            .begin_submission_against_active_policy(
                first_id,
                successor_record.policy_digest,
                ReputationFinalizedIdentityV1 {
                    height: 12,
                    block_hash: [0xC3; 32],
                },
                FINALIZED_AT_MS.saturating_add(300),
            )
            .expect("retry source-time-valid historical bytes");
        assert_eq!(rebound.event_id, first_id);
        assert_eq!(rebound.authority, first_policy.por_recorder_authority);

        let mut not_yet_finalized_source = verified_por(0x44);
        not_yet_finalized_source.issued_at_unix_ms = not_yet_finalized_source
            .issued_at_unix_ms
            .saturating_add(400);
        not_yet_finalized_source.deadline_at_unix_ms = not_yet_finalized_source
            .deadline_at_unix_ms
            .saturating_add(400);
        not_yet_finalized_source.responded_at_unix_ms = not_yet_finalized_source
            .responded_at_unix_ms
            .map(|responded_at| responded_at.saturating_add(400));
        not_yet_finalized_source.decided_at_unix_ms = not_yet_finalized_source
            .decided_at_unix_ms
            .saturating_add(400);
        let future_event_id = match producer
            .enqueue_terminal(provider(10), not_yet_finalized_source)
            .expect("retain source awaiting a sufficiently new finalized view")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        assert!(matches!(
            outbox.begin_submission_against_active_policy(
                future_event_id,
                successor_record.policy_digest,
                ReputationFinalizedIdentityV1 {
                    height: 12,
                    block_hash: [0xC3; 32],
                },
                FINALIZED_AT_MS.saturating_add(300),
            ),
            Err(ReputationRuntimeError::JournalSourceNotFinalized)
        ));

        drop(producer);
        drop(outbox);
        assert!(matches!(
            ReputationJournalProducerOutboxV1::open(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    first_policy.clone(),
                )
                .expect("stale producer policy"),
            ),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));
        let mut substituted_successor = successor.clone();
        substituted_successor.por_recorder_authority = account(0x52);
        assert!(matches!(
            ReputationJournalProducerOutboxV1::open(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    substituted_successor,
                )
                .expect("substituted producer policy"),
            ),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));
        let mut skipped_successor = successor.clone();
        skipped_successor.revision = skipped_successor.revision.saturating_add(1);
        skipped_successor.predecessor_policy_digest =
            Some(successor.canonical_digest().expect("successor digest"));
        assert!(matches!(
            ReputationJournalProducerOutboxV1::open(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    skipped_successor,
                )
                .expect("skipped producer policy"),
            ),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));
        let restored = Arc::new(
            ReputationJournalProducerOutboxV1::open(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    successor,
                )
                .expect("producer policy"),
            )
            .expect("restore rotated producer checkpoint"),
        );
        let restored_producer = PorReputationJournalProducerV1::new(restored);
        assert_eq!(
            restored_producer
                .enqueue_terminal(provider(7), verified_por(0x41))
                .expect("exact retained replay after restart"),
            ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id: first_id }
        );
    }

    #[test]
    fn durable_journal_scan_rejects_same_height_fork() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = ReputationJournalProducerOutboxV1::open(temp.path(), producer_policy())
            .expect("outbox");
        let first = ReputationJournalFinalizedEventPageV1 {
            finalized_cursor: ReputationJournalFinalizedCursorV1 {
                height: 10,
                block_hash: [0xD1; 32],
                finalized_at_unix_ms: FINALIZED_AT_MS,
            },
            events: Vec::new(),
            has_more: false,
            next_after: None,
        };
        outbox
            .reconcile_finalized_journal_page(first)
            .expect("first scan");
        let fork = ReputationJournalFinalizedEventPageV1 {
            finalized_cursor: ReputationJournalFinalizedCursorV1 {
                height: 10,
                block_hash: [0xD2; 32],
                finalized_at_unix_ms: FINALIZED_AT_MS,
            },
            events: Vec::new(),
            has_more: false,
            next_after: None,
        };
        assert!(matches!(
            outbox.reconcile_finalized_journal_page(fork),
            Err(ReputationRuntimeError::FinalizedFork)
        ));
    }

    #[test]
    fn completed_suffix_compacts_without_reordering_checkpoint_sequence() {
        let temp = TempDir::new().expect("tempdir");
        let mut policy = producer_policy();
        policy.max_completed = 1;
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            policy.clone(),
        ));
        let producer = PorReputationJournalProducerV1::new(outbox.clone());
        let first = match producer
            .enqueue_terminal(provider(7), verified_por(0x21))
            .expect("first PoR")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let second = match producer
            .enqueue_terminal(provider(7), verified_por(0x22))
            .expect("second PoR")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let baseline = ReputationFinalizedIdentityV1 {
            height: 10,
            block_hash: [0xA4; 32],
        };
        outbox
            .begin_submission(first, baseline)
            .expect("begin first");
        outbox
            .begin_submission(second, baseline)
            .expect("begin second");
        outbox
            .acknowledge_committed(
                second,
                ReputationCommittedEventIdentityV1 {
                    sequence: 2,
                    block_height: 11,
                    block_hash: [0xA5; 32],
                    event_index: 1,
                },
            )
            .expect("commit second");
        outbox
            .acknowledge_committed(
                first,
                ReputationCommittedEventIdentityV1 {
                    sequence: 1,
                    block_height: 11,
                    block_hash: [0xA5; 32],
                    event_index: 0,
                },
            )
            .expect("commit first");
        assert_eq!(outbox.status().expect("status").completed, 1);
        drop(producer);
        drop(outbox);
        assert_eq!(
            ReputationJournalProducerOutboxV1::open(temp.path(), policy)
                .expect("restore compacted outbox")
                .status()
                .expect("restored status")
                .next_sequence,
            3
        );
    }

    #[test]
    fn journal_checkpoint_byte_ceiling_binary_search_preserves_exact_replay_tombstones() {
        let temp = TempDir::new().expect("tempdir");
        let mut policy = producer_policy();
        policy.max_attempts = 1;
        policy.checkpoint_max_bytes = REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1;
        let policy_digest = policy.digest().expect("producer policy digest");
        let outbox = Arc::new(open_initialized_producer_outbox(
            temp.path(),
            policy.clone(),
        ));

        let authority_policy = policy.authority_policy.clone();
        let observed_outcome = verified_por(0x31);
        let observed_entry = ReputationJournalEntryV1::try_new(
            provider(6),
            authority_policy
                .canonical_digest()
                .expect("authority policy digest"),
            authority_policy.por_recorder_authority,
            observed_outcome.decided_at_unix_ms,
            None,
            ReputationJournalPayloadV1::PorTerminal(observed_outcome),
        )
        .expect("observed journal entry");
        outbox
            .reconcile_finalized_journal_page(ReputationJournalFinalizedEventPageV1 {
                finalized_cursor: ReputationJournalFinalizedCursorV1 {
                    height: 10,
                    block_hash: [0xD3; 32],
                    finalized_at_unix_ms: FINALIZED_AT_MS.saturating_add(100),
                },
                events: vec![finalized_journal_event(
                    1,
                    10,
                    [0xD3; 32],
                    0,
                    observed_entry,
                )],
                has_more: false,
                next_after: None,
            })
            .expect("retain observed tombstone");

        let token_producer =
            CountedStreamTokenReputationJournalProducerV1::new(Arc::clone(&outbox));
        let mut head_event_id = ReputationJournalEventIdV1::ZERO;
        for sequence in 1_u8..=17 {
            let mut token = counted_token(0x97, sequence);
            token.binding.gateway_sequence = u64::from(sequence);
            head_event_id = match token_producer
                .enqueue_counted(provider(9), token)
                .expect("token admission")
            {
                CountedStreamTokenProducerOutcomeV1::Enqueued(
                    ReputationJournalEnqueueOutcomeV1::Inserted { event_id },
                ) => event_id,
                other => panic!("unexpected token admission: {other:?}"),
            };
        }

        let por_producer = PorReputationJournalProducerV1::new(Arc::clone(&outbox));
        let completed_event_id = match por_producer
            .enqueue_terminal(provider(7), verified_por(0x32))
            .expect("local PoR row")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        outbox
            .begin_submission(
                completed_event_id,
                ReputationFinalizedIdentityV1 {
                    height: 10,
                    block_hash: [0xD3; 32],
                },
            )
            .expect("begin local PoR submission");
        outbox
            .acknowledge_committed(
                completed_event_id,
                ReputationCommittedEventIdentityV1 {
                    sequence: 2,
                    block_height: 11,
                    block_hash: [0xD4; 32],
                    event_index: 0,
                },
            )
            .expect("retain completed tombstone");

        let dead_letter_event_id = match por_producer
            .enqueue_terminal(provider(8), verified_por(0x33))
            .expect("dead-letter PoR row")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        outbox
            .begin_submission(
                dead_letter_event_id,
                ReputationFinalizedIdentityV1 {
                    height: 11,
                    block_hash: [0xD4; 32],
                },
            )
            .expect("begin terminal PoR submission");
        assert!(matches!(
            outbox
                .record_not_submitted(dead_letter_event_id, [0xE3; 32])
                .expect("dead-letter failed PoR"),
            ReputationJournalDeliveryOutcomeV1::DeadLettered { attempts: 1 }
        ));

        let original = outbox
            .state
            .lock()
            .expect("producer state")
            .checkpoint
            .clone();
        assert_eq!(original.observed.len(), 1);
        assert_eq!(original.completed.len(), 1);
        assert_eq!(original.dead_letters.len(), 1);
        assert_eq!(original.stream_token_gateway_admissions.len(), 17);
        let original_pending = original.pending.clone();
        let original_completed = original.completed.clone();
        let original_observed = original.observed.clone();
        let original_dead_letters = original.dead_letters.clone();
        let original_heads = original.stream_token_gateway_heads.clone();

        // Derive the minimal fitting prefix independently from the production
        // search, plan, and eviction helpers. The fixture fixes sequences
        // 1..=16 as evictable oldest-to-newest and sequence 17 as the head.
        const EXPECTED_PREFIX: usize = 9;
        let expected_eviction_order = (1_u64..17)
            .map(|sequence| {
                original
                    .stream_token_gateway_admissions
                    .iter()
                    .find(|admission| admission.binding.gateway_sequence == sequence)
                    .expect("hard-coded non-head admission")
                    .event_id
            })
            .collect::<Vec<_>>();
        let mut iterative = original.clone();
        let mut iterative_lengths = vec![
            norito::core::encoded_frame_len(&iterative).expect("measure original checkpoint frame"),
        ];
        let mut expected = None;
        for (index, event_id) in expected_eviction_order.iter().copied().enumerate() {
            let position = iterative
                .stream_token_gateway_admissions
                .iter()
                .position(|admission| admission.event_id == event_id)
                .expect("hard-coded admission remains");
            iterative.stream_token_gateway_admissions.remove(position);
            iterative_lengths.push(
                norito::core::encoded_frame_len(&iterative)
                    .expect("measure iterative checkpoint frame"),
            );
            if index + 1 == EXPECTED_PREFIX {
                expected = Some(iterative.clone());
            }
        }
        assert_eq!(iterative.stream_token_gateway_admissions.len(), 1);
        assert!(
            iterative_lengths
                .windows(2)
                .all(|adjacent| adjacent[0] > adjacent[1]),
            "each complete admission removal must strictly reduce the frame"
        );
        let expected = expected.expect("capture independently compacted checkpoint");
        let ceiling =
            u64::try_from(iterative_lengths[EXPECTED_PREFIX]).expect("fixture length fits u64");
        assert!(
            u64::try_from(iterative_lengths[EXPECTED_PREFIX - 1]).expect("fixture length fits u64")
                > ceiling,
            "the preceding prefix must remain over the selected ceiling"
        );

        let eviction_plan = stream_token_admission_eviction_plan(&original);
        assert_eq!(eviction_plan, expected_eviction_order);
        let search = smallest_stream_token_admission_eviction_prefix(
            &original,
            &eviction_plan,
            ceiling,
            iterative_lengths[0],
        )
        .expect("find smallest fitting admission prefix");
        assert_eq!(search.prefix, EXPECTED_PREFIX);
        let mut ceiling_log2 = 0;
        let mut covered = 1;
        while covered < eviction_plan.len() {
            covered *= 2;
            ceiling_log2 += 1;
        }
        assert!(
            search.probes <= ceiling_log2 + 1,
            "full-plan qualification plus binary search must be logarithmic"
        );

        let (bounded, bounded_bytes) =
            encode_bounded_journal_checkpoint(original.clone(), &policy, policy_digest, ceiling)
                .expect("evict the independently minimal admission prefix");
        assert_eq!(bounded, expected);
        assert_eq!(
            bounded_bytes.len(),
            norito::core::encoded_frame_len(&bounded).expect("measure exact bounded frame")
        );
        assert_eq!(
            bounded_bytes,
            norito::to_bytes(&expected).expect("encode exact expected checkpoint")
        );
        assert_eq!(bounded.pending, original_pending);
        assert_eq!(bounded.completed, original_completed);
        assert_eq!(bounded.observed, original_observed);
        assert_eq!(bounded.dead_letters, original_dead_letters);
        assert_eq!(bounded.stream_token_gateway_heads, original_heads);
        let head = bounded
            .stream_token_gateway_heads
            .first()
            .expect("gateway head");
        assert_eq!(head.event_id, head_event_id);
        assert!(
            bounded
                .stream_token_gateway_admissions
                .iter()
                .any(|admission| admission.binding == head.binding
                    && admission.event_id == head.event_id),
            "the canonical head admission must remain pinned"
        );
        assert_eq!(
            decode_journal_checkpoint(&bounded_bytes, &policy, policy_digest)
                .expect("decode bounded checkpoint"),
            bounded
        );

        let irreducible = iterative;
        let mut irreducible_probe = irreducible.clone();
        assert!(!evict_oldest_non_head_stream_token_admission(
            &mut irreducible_probe
        ));
        assert_eq!(irreducible.pending, original_pending);
        assert_eq!(irreducible.completed, original_completed);
        assert_eq!(irreducible.observed, original_observed);
        assert_eq!(irreducible.dead_letters, original_dead_letters);
        assert_eq!(irreducible.stream_token_gateway_heads, original_heads);
        let irreducible_ceiling = u64::try_from(
            norito::core::encoded_frame_len(&irreducible).expect("measure irreducible checkpoint"),
        )
        .expect("fixture length fits u64")
        .saturating_sub(1);
        assert!(matches!(
            encode_bounded_journal_checkpoint(
                original.clone(),
                &policy,
                policy_digest,
                irreducible_ceiling,
            ),
            Err(ReputationRuntimeError::CheckpointTooLarge)
        ));
        assert_eq!(original.completed, original_completed);
        assert_eq!(original.observed, original_observed);
    }

    #[test]
    fn publication_construction_rejects_signer_handle_substitution() {
        let projector_root = TempDir::new().expect("projector root");
        let publication_root = TempDir::new().expect("publication root");
        let trust = trust_policy();
        let ingest = ingest_policy(&trust);
        let projector = Arc::new(
            ReputationIngestService::open(projector_root.path(), ingest).expect("projector"),
        );
        let policy = ReputationPublicationPolicyV1::try_new(
            &trust,
            "signer-a",
            "dag-a",
            b"peer-a".to_vec(),
            SigningKey::from_bytes(&[0xB1; 32])
                .verifying_key()
                .to_bytes(),
            REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1,
        )
        .expect("publication policy");
        let signer_qualification = policy.threshold_signer_qualification();
        let dag_qualification = policy.governance_dag_qualification();
        let result = ReputationPublicationReconcilerV1::open(
            publication_root.path(),
            projector,
            trust,
            policy,
            Arc::new(NullThresholdSigner {
                handle: "signer-b".to_owned(),
                qualification: signer_qualification,
            }),
            Arc::new(NullGovernanceDag {
                handle: "dag-a".to_owned(),
                qualification: dag_qualification,
            }),
        );
        assert!(matches!(
            result,
            Err(ReputationRuntimeError::RuntimeBindingMismatch)
        ));
    }

    #[test]
    fn governance_dag_qualification_binds_peer_identity_and_ed25519_key() {
        let first_key = SigningKey::from_bytes(&[0xB1; 32])
            .verifying_key()
            .to_bytes();
        let second_key = SigningKey::from_bytes(&[0xB2; 32])
            .verifying_key()
            .to_bytes();
        let expected = reputation_governance_dag_policy_digest_v1(b"peer-a", first_key)
            .expect("policy digest");

        assert_eq!(
            expected,
            reputation_governance_dag_policy_digest_v1(b"peer-a", first_key)
                .expect("stable policy digest")
        );
        assert_ne!(
            expected,
            reputation_governance_dag_policy_digest_v1(b"peer-b", first_key)
                .expect("peer-bound policy digest")
        );
        assert_ne!(
            expected,
            reputation_governance_dag_policy_digest_v1(b"peer-a", second_key)
                .expect("key-bound policy digest")
        );
        assert!(matches!(
            reputation_governance_dag_policy_digest_v1(b"", first_key),
            Err(ReputationRuntimeError::InvalidRuntimePolicy)
        ));
        assert!(matches!(
            reputation_governance_dag_policy_digest_v1(b"peer-a", [0; 32]),
            Err(ReputationRuntimeError::InvalidRuntimePolicy)
        ));
    }

    #[test]
    fn publication_construction_rejects_same_key_different_peer_qualification() {
        let projector_root = TempDir::new().expect("projector root");
        let publication_root = TempDir::new().expect("publication root");
        let trust = trust_policy();
        let projector = Arc::new(
            ReputationIngestService::open(projector_root.path(), ingest_policy(&trust))
                .expect("projector"),
        );
        let publisher_key = SigningKey::from_bytes(&[0xB1; 32])
            .verifying_key()
            .to_bytes();
        let policy = ReputationPublicationPolicyV1::try_new(
            &trust,
            "signer-a",
            "dag-a",
            b"peer-a".to_vec(),
            publisher_key,
            REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1,
        )
        .expect("publication policy");
        let substituted_peer_qualification = ReputationRuntimeProviderQualificationV1::new(
            REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
            reputation_governance_dag_policy_digest_v1(b"peer-b", publisher_key)
                .expect("substituted peer policy digest"),
        );
        let result = ReputationPublicationReconcilerV1::open(
            publication_root.path(),
            projector,
            trust,
            policy.clone(),
            Arc::new(NullThresholdSigner {
                handle: "signer-a".to_owned(),
                qualification: policy.threshold_signer_qualification(),
            }),
            Arc::new(NullGovernanceDag {
                handle: "dag-a".to_owned(),
                qualification: substituted_peer_qualification,
            }),
        );

        assert!(matches!(
            result,
            Err(ReputationRuntimeError::RuntimeBindingMismatch)
        ));
        assert!(
            !publication_root
                .path()
                .join(REPUTATION_PUBLICATION_CHECKPOINT_FILE_NAME_V1)
                .exists(),
            "peer substitution must fail before publication state opens"
        );
    }

    #[test]
    fn governance_readback_is_discarded_when_provider_qualification_drifts() {
        let projector_root = TempDir::new().expect("projector root");
        let publication_root = TempDir::new().expect("publication root");
        let trust = trust_policy();
        let projector = Arc::new(
            ReputationIngestService::open(projector_root.path(), ingest_policy(&trust))
                .expect("projector"),
        );
        let policy = publication_policy(&trust);
        let signed = signed_snapshot(&trust, [0xD8; 16], None, FINALIZED_AT_MS / 1_000);
        let block = governance_block_after(&signed, None);
        let readback = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(std::slice::from_ref(&block)),
            inclusion_path: vec![block],
        };
        let pending = StoredReputationPublicationV1 {
            sequence: 1,
            material_digest: [0xD9; 32],
            signed_result_digest: signed_result_digest(&signed).expect("signed result digest"),
            signed_result: signed,
            governance_acknowledgement: None,
            governance_readback: None,
        };
        let request = governance_publication_request(&pending).expect("publication request");
        let reconciler = ReputationPublicationReconcilerV1::open(
            publication_root.path(),
            projector,
            trust,
            policy.clone(),
            Arc::new(NullThresholdSigner {
                handle: "signer-a".to_owned(),
                qualification: policy.threshold_signer_qualification(),
            }),
            Arc::new(DriftingGovernanceDag {
                handle: "dag-a".to_owned(),
                qualification: Mutex::new(policy.governance_dag_qualification()),
                readback,
            }),
        )
        .expect("open reconciler");

        assert!(matches!(
            reconciler.reconcile_governance_publication(&request),
            Err(ReputationRuntimeError::RuntimeBindingChanged)
        ));
        assert!(
            reconciler
                .pending_publication()
                .expect("read durable publication state")
                .is_none(),
            "a receipt returned during qualification drift must not become durable"
        );
    }

    #[test]
    fn committed_projection_is_gated_idempotent_restart_safe_and_corruption_strict() {
        let projector_root = TempDir::new().expect("projector root");
        let publication_root = TempDir::new().expect("publication root");
        let trust = trust_policy();
        let projector = Arc::new(
            ReputationIngestService::open(projector_root.path(), ingest_policy(&trust))
                .expect("projector"),
        );
        let policy = publication_policy(&trust);
        let reconciler = open_publication_reconciler(
            publication_root.path(),
            Arc::clone(&projector),
            trust.clone(),
            policy.clone(),
        );
        assert_eq!(
            reconciler
                .committed_read_projection()
                .expect("empty projection"),
            ReputationCommittedReadProjectionV1::empty(
                policy.digest().expect("publication policy digest")
            )
        );

        let signed = signed_snapshot(&trust, [0xE1; 16], None, FINALIZED_AT_MS / 1_000);
        signed
            .verify(&trust, FINALIZED_AT_MS / 1_000)
            .expect("threshold-signed fixture");
        let delivery = signing_delivery(&signed);
        reconciler
            .validate_signed_result(&delivery, &signed)
            .expect("signed result binds exact delivery");
        reconciler
            .store_signed_result(&delivery, signed.clone())
            .expect("stage signed result");
        assert!(
            reconciler
                .committed_read_projection()
                .expect("staged projection")
                .latest
                .is_none(),
            "threshold signing alone must not enter the committed projection"
        );

        let digest = signed_result_digest(&signed).expect("signed result digest");
        let (acknowledgement, governance_readback) = governance_readback(
            &policy,
            delivery.sequence,
            delivery.material_digest,
            digest,
            &signed,
        );
        assert!(matches!(
            reconciler.complete_publication(acknowledgement),
            Err(ReputationRuntimeError::PublicationCheckpointConflict)
        ));
        assert!(
            reconciler
                .committed_read_projection()
                .expect("pre-readback projection")
                .latest
                .is_none(),
            "a structurally valid but unobserved acknowledgement must not publish"
        );
        reconciler
            .store_governance_readback(acknowledgement, governance_readback)
            .expect("store authoritative DAG readback");
        assert!(
            reconciler
                .committed_read_projection()
                .expect("DAG-staged projection")
                .latest
                .is_none(),
            "DAG readback must remain hidden until projector acknowledgement"
        );

        reconciler
            .complete_publication(acknowledgement)
            .expect("commit authoritative projection");
        let committed = reconciler
            .committed_read_projection()
            .expect("committed projection");
        assert_eq!(committed.events.len(), 1);
        assert_eq!(
            reconciler
                .committed_snapshot_by_id(signed.snapshot.snapshot_id)
                .expect("committed snapshot lookup"),
            Some(signed.snapshot.clone())
        );
        assert_eq!(
            committed
                .latest
                .as_ref()
                .expect("latest committed snapshot")
                .signed_result,
            signed
        );
        assert_eq!(committed.events[0].snapshot_id, signed.snapshot.snapshot_id);

        reconciler
            .complete_publication(acknowledgement)
            .expect("exact completion replay");
        assert_eq!(
            reconciler
                .committed_read_projection()
                .expect("idempotent projection"),
            committed,
            "an exact replay must not duplicate committed history"
        );
        drop(reconciler);

        let restored = open_publication_reconciler(
            publication_root.path(),
            Arc::clone(&projector),
            trust.clone(),
            policy.clone(),
        );
        assert_eq!(
            restored
                .committed_read_projection()
                .expect("restored projection"),
            committed
        );
        drop(restored);

        fs::write(
            publication_root
                .path()
                .join(REPUTATION_PUBLICATION_CHECKPOINT_FILE_NAME_V1),
            [0xFF, 0x00, 0x01],
        )
        .expect("write corrupt publication checkpoint");
        let signer_qualification = policy.threshold_signer_qualification();
        let dag_qualification = policy.governance_dag_qualification();
        assert!(matches!(
            ReputationPublicationReconcilerV1::open(
                publication_root.path(),
                projector,
                trust,
                policy,
                Arc::new(NullThresholdSigner {
                    handle: "signer-a".to_owned(),
                    qualification: signer_qualification,
                }),
                Arc::new(NullGovernanceDag {
                    handle: "dag-a".to_owned(),
                    qualification: dag_qualification,
                }),
            ),
            Err(ReputationRuntimeError::InvalidCheckpoint)
                | Err(ReputationRuntimeError::CheckpointIo)
        ));
    }

    #[test]
    fn publication_reconciler_retains_exact_successor_snapshots_across_restart() {
        let projector_root = TempDir::new().expect("projector root");
        let publication_root = TempDir::new().expect("publication root");
        let trust = trust_policy();
        let projector = Arc::new(
            ReputationIngestService::open(projector_root.path(), ingest_policy(&trust))
                .expect("projector"),
        );
        let policy = publication_policy(&trust);
        let reconciler = open_publication_reconciler(
            publication_root.path(),
            Arc::clone(&projector),
            trust.clone(),
            policy.clone(),
        );

        let first = signed_snapshot(&trust, [0xB8; 16], None, FINALIZED_AT_MS / 1_000);
        let first_delivery = signing_delivery(&first);
        reconciler
            .store_signed_result(&first_delivery, first.clone())
            .expect("stage first signed result");
        let first_digest = signed_result_digest(&first).expect("first signed result digest");
        let (first_acknowledgement, first_readback) = governance_readback(
            &policy,
            first_delivery.sequence,
            first_delivery.material_digest,
            first_digest,
            &first,
        );
        let first_public_readback = first_readback
            .reconstruct_readback(&first)
            .expect("reconstruct first Governance DAG readback");
        reconciler
            .store_governance_readback(first_acknowledgement, first_readback)
            .expect("store first Governance DAG readback");
        reconciler
            .complete_publication(first_acknowledgement)
            .expect("complete first publication");

        let unlinked = signed_snapshot(&trust, [0xBA; 16], None, FINALIZED_AT_MS / 1_000 + 1);
        let unlinked_delivery = signing_delivery(&unlinked);
        assert!(matches!(
            reconciler.store_signed_result(&unlinked_delivery, unlinked),
            Err(ReputationRuntimeError::PublicationCheckpointConflict)
        ));

        let second = signed_snapshot(
            &trust,
            [0xB9; 16],
            Some(first.snapshot.snapshot_id),
            FINALIZED_AT_MS / 1_000 + 1,
        );
        let second_delivery = signing_delivery(&second);
        reconciler
            .store_signed_result(&second_delivery, second.clone())
            .expect("stage successor signed result");
        let staged_status = reconciler.status().expect("staged successor status");
        assert!(staged_status.signed_result_staged);
        assert!(!staged_status.governance_acknowledged);
        assert!(!staged_status.complete);
        assert_eq!(
            reconciler
                .committed_snapshot_by_id(first.snapshot.snapshot_id)
                .expect("lookup first while successor is staged"),
            Some(first.snapshot.clone()),
            "staging a successor must not mutate committed history"
        );
        assert_eq!(
            reconciler
                .committed_read_projection()
                .expect("projection while successor is staged")
                .latest
                .as_ref()
                .map(|committed| committed.signed_result.snapshot.snapshot_id),
            Some(first.snapshot.snapshot_id)
        );

        let second_digest = signed_result_digest(&second).expect("second signed result digest");
        let (second_acknowledgement, second_readback) = governance_readback_after(
            &policy,
            second_delivery.sequence,
            second_delivery.material_digest,
            second_digest,
            &second,
            Some(&first_public_readback),
        );
        reconciler
            .store_governance_readback(second_acknowledgement, second_readback)
            .expect("store successor Governance DAG readback");
        let acknowledged_status = reconciler.status().expect("acknowledged successor status");
        assert!(acknowledged_status.signed_result_staged);
        assert!(acknowledged_status.governance_acknowledged);
        assert!(!acknowledged_status.complete);
        reconciler
            .complete_publication(second_acknowledgement)
            .expect("complete successor publication");
        let completed_status = reconciler.status().expect("completed successor status");
        assert!(!completed_status.signed_result_staged);
        assert!(completed_status.governance_acknowledged);
        assert!(completed_status.complete);

        let projection = reconciler
            .committed_read_projection()
            .expect("successor projection");
        assert_eq!(projection.events.len(), 2);
        assert_eq!(
            projection
                .latest
                .as_ref()
                .map(|committed| committed.signed_result.snapshot.snapshot_id),
            Some(second.snapshot.snapshot_id)
        );
        assert_eq!(
            reconciler
                .committed_snapshot_by_id(first.snapshot.snapshot_id)
                .expect("lookup retained predecessor"),
            Some(first.snapshot.clone())
        );
        assert_eq!(
            reconciler
                .committed_snapshot_by_id(second.snapshot.snapshot_id)
                .expect("lookup retained successor"),
            Some(second.snapshot.clone())
        );
        drop(reconciler);

        let restored =
            open_publication_reconciler(publication_root.path(), projector, trust, policy);
        assert_eq!(
            restored
                .committed_snapshot_by_id(first.snapshot.snapshot_id)
                .expect("lookup restored predecessor"),
            Some(first.snapshot)
        );
        assert_eq!(
            restored
                .committed_snapshot_by_id(second.snapshot.snapshot_id)
                .expect("lookup restored successor"),
            Some(second.snapshot)
        );
    }

    #[test]
    fn committed_projection_canonical_validation_rejects_reordering_and_overflow() {
        let trust = trust_policy();
        let policy = publication_policy(&trust);
        let policy_digest = policy.digest().expect("publication policy digest");
        let signed = signed_snapshot(&trust, [0xE2; 16], None, FINALIZED_AT_MS / 1_000);
        let material_digest = [0xE3; 32];
        let signed_result_digest = signed_result_digest(&signed).expect("signed digest");
        let (acknowledgement, governance_readback) =
            governance_readback(&policy, 1, material_digest, signed_result_digest, &signed);
        let committed = ReputationCommittedSnapshotV1 {
            sequence: 1,
            material_digest,
            signed_result_digest,
            signed_result: signed.clone(),
            governance_acknowledgement: acknowledgement,
        };
        let mut checkpoint = ReputationPublicationCheckpointV1::empty(policy_digest);
        checkpoint
            .commit_authoritative(
                committed.clone(),
                &policy,
                &trust,
                governance_readback.clone(),
            )
            .expect("commit projection");
        checkpoint
            .committed_read
            .validate(policy_digest)
            .expect("validate projection");
        checkpoint
            .commit_authoritative(committed, &policy, &trust, governance_readback.clone())
            .expect("exact replay");
        assert_eq!(checkpoint.committed_read.events.len(), 1);
        assert_eq!(checkpoint.committed_snapshots.len(), 1);
        let projection = checkpoint.committed_read.clone();

        let canonical = norito::to_bytes(&checkpoint).expect("canonical checkpoint");
        assert_eq!(
            decode_publication_checkpoint(&canonical, &policy, policy_digest, &trust)
                .expect("decode canonical checkpoint"),
            checkpoint
        );
        let mut noncanonical = canonical;
        noncanonical.push(0);
        assert!(matches!(
            decode_publication_checkpoint(&noncanonical, &policy, policy_digest, &trust),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));

        let mut reordered = projection.clone();
        reordered.events.push(reordered.events[0].clone());
        assert!(matches!(
            reordered.validate(policy_digest),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));

        let mut oversized = projection;
        oversized.events =
            vec![oversized.events[0].clone(); REPUTATION_COMMITTED_READ_MAX_EVENTS_V1 + 1];
        assert!(matches!(
            oversized.validate(policy_digest),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn committed_snapshot_history_is_bounded_ordered_and_restart_safe() {
        let trust = trust_policy();
        let policy = publication_policy(&trust);
        let policy_digest = policy.digest().expect("publication policy digest");
        let mut checkpoint = ReputationPublicationCheckpointV1::empty(policy_digest);
        let mut previous_snapshot_id = None;
        let mut previous_governance_readback = None;

        for offset in 0_u8..3 {
            let snapshot_id = [0xC0 + offset; 16];
            let signed = signed_snapshot(
                &trust,
                snapshot_id,
                previous_snapshot_id,
                FINALIZED_AT_MS / 1_000 + u64::from(offset),
            );
            let material_digest = [0xD0 + offset; 32];
            let signed_result_digest = signed_result_digest(&signed).expect("signed result digest");
            let sequence = u64::from(offset) + 1;
            let (acknowledgement, governance_readback) = governance_readback_after(
                &policy,
                sequence,
                material_digest,
                signed_result_digest,
                &signed,
                previous_governance_readback.as_ref(),
            );
            let current_governance_readback = governance_readback
                .reconstruct_readback(&signed)
                .expect("reconstruct Governance DAG readback");
            let committed = ReputationCommittedSnapshotV1 {
                sequence,
                material_digest,
                signed_result_digest,
                signed_result: signed,
                governance_acknowledgement: acknowledgement,
            };
            checkpoint
                .commit_authoritative_with_retention_limit(
                    committed,
                    &policy,
                    &trust,
                    governance_readback,
                    2,
                )
                .expect("commit bounded authoritative snapshot");
            previous_snapshot_id = Some(snapshot_id);
            previous_governance_readback = Some(current_governance_readback);
        }

        assert_eq!(checkpoint.committed_snapshots.len(), 2);
        assert_eq!(checkpoint.committed_governance_readbacks.len(), 2);
        assert_eq!(
            checkpoint
                .committed_snapshots
                .iter()
                .map(|committed| committed.signed_result.snapshot.snapshot_id)
                .collect::<Vec<_>>(),
            vec![[0xC1; 16], [0xC2; 16]]
        );
        assert_eq!(
            checkpoint
                .committed_read
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(
            checkpoint
                .committed_read
                .latest
                .as_ref()
                .map(|committed| committed.signed_result.snapshot.snapshot_id),
            Some([0xC2; 16])
        );
        assert!(
            checkpoint
                .committed_snapshots
                .iter()
                .all(|committed| committed.signed_result.snapshot.snapshot_id != [0xC0; 16]),
            "the oldest identifier must be unavailable after bounded eviction"
        );
        validate_publication_checkpoint(&checkpoint, &policy, policy_digest, &trust)
            .expect("validate bounded history");

        let mut expected_byte_bounded = checkpoint.clone();
        assert!(expected_byte_bounded.evict_oldest_committed());
        let expected_byte_bounded_bytes =
            norito::to_bytes(&expected_byte_bounded).expect("encode one retained snapshot");
        let byte_ceiling =
            u64::try_from(expected_byte_bounded_bytes.len()).expect("fixture length fits u64");
        let (byte_bounded, byte_bounded_bytes) = encode_bounded_publication_checkpoint(
            checkpoint.clone(),
            &policy,
            policy_digest,
            &trust,
            byte_ceiling,
        )
        .expect("evict the oldest snapshot to meet the byte ceiling");
        assert_eq!(byte_bounded, expected_byte_bounded);
        assert_eq!(byte_bounded_bytes, expected_byte_bounded_bytes);
        assert_eq!(
            decode_publication_checkpoint(&byte_bounded_bytes, &policy, policy_digest, &trust,)
                .expect("restore byte-bounded history"),
            expected_byte_bounded
        );
        assert!(matches!(
            encode_bounded_publication_checkpoint(
                checkpoint.clone(),
                &policy,
                policy_digest,
                &trust,
                byte_ceiling - 1,
            ),
            Err(ReputationRuntimeError::CheckpointTooLarge)
        ));

        let canonical = norito::to_bytes(&checkpoint).expect("encode bounded history");
        assert_eq!(
            decode_publication_checkpoint(&canonical, &policy, policy_digest, &trust)
                .expect("restore bounded history"),
            checkpoint
        );
    }

    #[test]
    fn canonical_checkpoint_rejects_forged_snapshot_signature() {
        let trust = trust_policy();
        let policy = publication_policy(&trust);
        let policy_digest = policy.digest().expect("publication policy digest");
        let signed = signed_snapshot(&trust, [0xE4; 16], None, FINALIZED_AT_MS / 1_000);
        let mut checkpoint = ReputationPublicationCheckpointV1 {
            version: REPUTATION_PUBLICATION_CHECKPOINT_VERSION_V1,
            policy_digest,
            pending: Some(StoredReputationPublicationV1 {
                sequence: 1,
                material_digest: [0xE5; 32],
                signed_result_digest: signed_result_digest(&signed).expect("signed digest"),
                signed_result: signed,
                governance_acknowledgement: None,
                governance_readback: None,
            }),
            committed_read: ReputationCommittedReadProjectionV1::empty(policy_digest),
            committed_snapshots: Vec::new(),
            committed_governance_readbacks: Vec::new(),
        };
        let canonical = norito::to_bytes(&checkpoint).expect("canonical valid checkpoint");
        decode_publication_checkpoint(&canonical, &policy, policy_digest, &trust)
            .expect("valid pending checkpoint");

        let mut untrusted_signer = checkpoint.clone();
        let untrusted_pending = untrusted_signer
            .pending
            .as_mut()
            .expect("pending publication");
        untrusted_pending.signed_result.signatures[0].signer_id = "threshold-z".to_owned();
        untrusted_pending.signed_result_digest =
            signed_result_digest(&untrusted_pending.signed_result)
                .expect("untrusted structural digest");
        let untrusted_signer =
            norito::to_bytes(&untrusted_signer).expect("canonical untrusted-signer checkpoint");
        assert!(matches!(
            decode_publication_checkpoint(&untrusted_signer, &policy, policy_digest, &trust),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));

        let pending = checkpoint.pending.as_mut().expect("pending publication");
        let signing_digest = pending
            .signed_result
            .signing_digest()
            .expect("snapshot signing digest");
        pending.signed_result.signatures[0].signature = SigningKey::from_bytes(&[0x72; 32])
            .sign(&signing_digest)
            .to_bytes();
        pending.signed_result_digest =
            signed_result_digest(&pending.signed_result).expect("forged structural digest");
        let forged = norito::to_bytes(&checkpoint).expect("canonical forged checkpoint");
        assert!(matches!(
            decode_publication_checkpoint(&forged, &policy, policy_digest, &trust),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn signed_head_inclusion_path_is_persisted_and_reverified_on_restart() {
        let trust = trust_policy();
        let policy = publication_policy(&trust);
        let policy_digest = policy.digest().expect("publication policy digest");
        let before = signed_snapshot(&trust, [0x31; 16], None, FINALIZED_AT_MS / 1_000);
        let target = signed_snapshot(&trust, [0x32; 16], None, FINALIZED_AT_MS / 1_000 + 1);
        let after = signed_snapshot(&trust, [0x33; 16], None, FINALIZED_AT_MS / 1_000 + 2);
        let first = governance_block_after(&before, None);
        let target_block = governance_block_after(&target, Some(&first));
        let tip = governance_block_after(&after, Some(&target_block));
        let readback = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(&[first.clone(), target_block.clone(), tip.clone()]),
            inclusion_path: vec![first, target_block, tip],
        };
        let material_digest = [0x34; 32];
        let target_digest = signed_result_digest(&target).expect("signed result digest");
        let (acknowledgement, target_index) = governance_acknowledgement_from_readback(
            &policy,
            1,
            material_digest,
            target_digest,
            &target,
            &readback,
            None,
        )
        .expect("validate signed-head inclusion");
        assert_eq!(target_index, 1);
        let mut substituted_head = readback.clone();
        substituted_head.head.publisher_peer_id = b"peer-b".to_vec();
        substituted_head.head.head_signature = empty_governance_signature();
        let substituted_payload = substituted_head
            .head
            .signature_payload_bytes()
            .expect("encode substituted head payload");
        let governance_signing_key = SigningKey::from_bytes(&[0xB1; 32]);
        substituted_head.head.head_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: governance_signing_key.verifying_key().to_bytes().to_vec(),
            signature: governance_signing_key
                .sign(&substituted_payload)
                .to_bytes()
                .to_vec(),
        };
        substituted_head
            .head
            .validate()
            .expect("substituted head remains cryptographically valid");
        assert!(matches!(
            governance_acknowledgement_from_readback(
                &policy,
                1,
                material_digest,
                target_digest,
                &target,
                &substituted_head,
                None,
            ),
            Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement)
        ));
        let stored =
            StoredReputationGovernanceDagReadbackV1::from_readback(&readback, target_index)
                .expect("compact readback");
        let committed = ReputationCommittedSnapshotV1 {
            sequence: 1,
            material_digest,
            signed_result_digest: target_digest,
            signed_result: target,
            governance_acknowledgement: acknowledgement,
        };
        let mut checkpoint = ReputationPublicationCheckpointV1::empty(policy_digest);
        checkpoint
            .commit_authoritative(committed, &policy, &trust, stored)
            .expect("commit signed-head inclusion");

        let canonical = norito::to_bytes(&checkpoint).expect("encode checkpoint");
        assert_eq!(
            decode_publication_checkpoint(&canonical, &policy, policy_digest, &trust)
                .expect("restart must reverify the full retained path"),
            checkpoint
        );

        let mut forged_path = checkpoint.clone();
        forged_path.committed_governance_readbacks[0].path_after_target[0]
            .block_signature
            .signature[0] ^= 0x01;
        let forged_path = norito::to_bytes(&forged_path).expect("encode forged path checkpoint");
        assert!(matches!(
            decode_publication_checkpoint(&forged_path, &policy, policy_digest, &trust),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));

        let mut forged_head = checkpoint;
        forged_head.committed_governance_readbacks[0]
            .head
            .head_signature
            .signature[0] ^= 0x01;
        let forged_head = norito::to_bytes(&forged_head).expect("encode forged head checkpoint");
        assert!(matches!(
            decode_publication_checkpoint(&forged_head, &policy, policy_digest, &trust),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn governance_readback_rejects_oversize_path_without_truncation() {
        let trust = trust_policy();
        let policy = publication_policy(&trust);
        let signed = signed_snapshot(&trust, [0x41; 16], None, FINALIZED_AT_MS / 1_000);
        let mut path = Vec::new();
        for _ in 0..=REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1 {
            let block = governance_block_after(&signed, path.last());
            path.push(block);
        }
        let readback = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(&path),
            inclusion_path: path,
        };
        let error = governance_acknowledgement_from_readback(
            &policy,
            1,
            [0x42; 32],
            signed_result_digest(&signed).expect("signed result digest"),
            &signed,
            &readback,
            None,
        )
        .expect_err("oversize inclusion path must fail closed");
        assert_eq!(
            error,
            ReputationRuntimeError::GovernanceReadbackPathTooLong {
                found: REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1 + 1,
                maximum: REPUTATION_GOVERNANCE_DAG_MAX_INCLUSION_BLOCKS_V1,
            }
        );
    }

    #[test]
    fn governance_readback_requires_the_exact_target_once() {
        let trust = trust_policy();
        let policy = publication_policy(&trust);
        let target = signed_snapshot(&trust, [0x45; 16], None, FINALIZED_AT_MS / 1_000);
        let other = signed_snapshot(&trust, [0x46; 16], None, FINALIZED_AT_MS / 1_000);
        let other_block = governance_block_after(&other, None);
        let missing = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(std::slice::from_ref(&other_block)),
            inclusion_path: vec![other_block],
        };
        assert!(matches!(
            governance_acknowledgement_from_readback(
                &policy,
                1,
                [0x47; 32],
                signed_result_digest(&target).expect("target digest"),
                &target,
                &missing,
                None,
            ),
            Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement)
        ));

        let first_target = governance_block_after(&target, None);
        let wrong_version = ReputationGovernanceDagReadbackV1 {
            version: 0,
            head: governance_head(std::slice::from_ref(&first_target)),
            inclusion_path: vec![first_target.clone()],
        };
        assert!(matches!(
            governance_acknowledgement_from_readback(
                &policy,
                1,
                [0x48; 32],
                signed_result_digest(&target).expect("target digest"),
                &target,
                &wrong_version,
                None,
            ),
            Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement)
        ));

        let second_target = governance_block_after(&target, Some(&first_target));
        let duplicate = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(&[first_target.clone(), second_target.clone()]),
            inclusion_path: vec![first_target, second_target],
        };
        assert!(matches!(
            governance_acknowledgement_from_readback(
                &policy,
                1,
                [0x49; 32],
                signed_result_digest(&target).expect("target digest"),
                &target,
                &duplicate,
                None,
            ),
            Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement)
        ));
    }

    #[test]
    fn governance_readback_rejects_fork_and_rollback_from_previous_head() {
        let trust = trust_policy();
        let policy = publication_policy(&trust);
        let previous = signed_snapshot(&trust, [0x51; 16], None, FINALIZED_AT_MS / 1_000);
        let previous_block = governance_block_after(&previous, None);
        let previous_readback = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(std::slice::from_ref(&previous_block)),
            inclusion_path: vec![previous_block],
        };
        governance_acknowledgement_from_readback(
            &policy,
            1,
            [0x52; 32],
            signed_result_digest(&previous).expect("previous digest"),
            &previous,
            &previous_readback,
            None,
        )
        .expect("validate previous authenticated head");

        let target = signed_snapshot(
            &trust,
            [0x53; 16],
            Some(previous.snapshot.snapshot_id),
            FINALIZED_AT_MS / 1_000 + 1,
        );
        let fork_root = governance_block_after(
            &signed_snapshot(&trust, [0x54; 16], None, FINALIZED_AT_MS / 1_000),
            None,
        );
        let fork_target = governance_block_after(&target, Some(&fork_root));
        let fork_readback = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(std::slice::from_ref(&fork_target)),
            inclusion_path: vec![fork_target],
        };
        assert!(matches!(
            governance_acknowledgement_from_readback(
                &policy,
                2,
                [0x55; 32],
                signed_result_digest(&target).expect("target digest"),
                &target,
                &fork_readback,
                Some(&previous_readback),
            ),
            Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement)
        ));

        let rollback_block = governance_block_after(&target, None);
        let rollback_readback = ReputationGovernanceDagReadbackV1 {
            version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
            head: governance_head(std::slice::from_ref(&rollback_block)),
            inclusion_path: vec![rollback_block],
        };
        assert!(matches!(
            governance_acknowledgement_from_readback(
                &policy,
                2,
                [0x56; 32],
                signed_result_digest(&target).expect("target digest"),
                &target,
                &rollback_readback,
                Some(&previous_readback),
            ),
            Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement)
        ));
    }

    #[test]
    fn canonical_checkpoint_rejects_forged_governance_readback_signature() {
        let trust = trust_policy();
        let policy = publication_policy(&trust);
        let policy_digest = policy.digest().expect("publication policy digest");
        let signed = signed_snapshot(&trust, [0xE6; 16], None, FINALIZED_AT_MS / 1_000);
        let material_digest = [0xE7; 32];
        let signed_result_digest = signed_result_digest(&signed).expect("signed digest");
        let (acknowledgement, governance_readback) =
            governance_readback(&policy, 1, material_digest, signed_result_digest, &signed);
        let committed = ReputationCommittedSnapshotV1 {
            sequence: 1,
            material_digest,
            signed_result_digest,
            signed_result: signed,
            governance_acknowledgement: acknowledgement,
        };
        let mut checkpoint = ReputationPublicationCheckpointV1::empty(policy_digest);
        checkpoint
            .commit_authoritative(committed, &policy, &trust, governance_readback)
            .expect("commit valid projection");
        let canonical = norito::to_bytes(&checkpoint).expect("canonical valid checkpoint");
        decode_publication_checkpoint(&canonical, &policy, policy_digest, &trust)
            .expect("valid committed checkpoint");

        let mut forged_acknowledgement = checkpoint.clone();
        forged_acknowledgement
            .committed_read
            .latest
            .as_mut()
            .expect("committed snapshot")
            .governance_acknowledgement
            .dag_block_cid[0] ^= 0x01;
        let forged_acknowledgement =
            norito::to_bytes(&forged_acknowledgement).expect("canonical forged acknowledgement");
        assert!(matches!(
            decode_publication_checkpoint(&forged_acknowledgement, &policy, policy_digest, &trust),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));

        checkpoint
            .committed_governance_readbacks
            .last_mut()
            .expect("committed Governance DAG readback")
            .target
            .block_signature
            .signature[0] ^= 0x01;
        let forged = norito::to_bytes(&checkpoint).expect("canonical forged DAG checkpoint");
        assert!(matches!(
            decode_publication_checkpoint(&forged, &policy, policy_digest, &trust),
            Err(ReputationRuntimeError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn malformed_ack_and_noncanonical_checkpoint_fail_closed() {
        let mut acknowledgement = ReputationGovernanceDagAcknowledgementV1 {
            version: REPUTATION_GOVERNANCE_DAG_ACKNOWLEDGEMENT_VERSION_V1,
            sequence: 1,
            material_digest: [1; 32],
            signed_result_digest: [2; 32],
            dag_block_sequence: 0,
            dag_block_cid: [3; 32],
            dag_node_cid: [4; 32],
            published_at_unix: 1,
            publisher_key_digest: [5; 32],
        };
        acknowledgement.dag_block_cid = [0; 32];
        assert!(matches!(
            acknowledgement.validate(),
            Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement)
        ));

        let temp = TempDir::new().expect("tempdir");
        fs::write(
            temp.path()
                .join(REPUTATION_JOURNAL_PRODUCER_CHECKPOINT_FILE_NAME_V1),
            [0xFF, 0x00, 0x01],
        )
        .expect("write corrupt checkpoint");
        assert!(matches!(
            ReputationJournalProducerOutboxV1::open(temp.path(), producer_policy()),
            Err(ReputationRuntimeError::InvalidCheckpoint)
                | Err(ReputationRuntimeError::CheckpointIo)
        ));
    }

    #[test]
    fn operation_keys_bind_phase_material_and_signed_result() {
        let signing = publication_idempotency_key(
            b"sorafs-reputation-threshold-signing-operation-v1",
            1,
            [0xC1; 32],
            None,
        )
        .expect("signing key");
        let governance = publication_idempotency_key(
            b"sorafs-reputation-governance-publication-operation-v1",
            1,
            [0xC1; 32],
            Some([0xC2; 32]),
        )
        .expect("governance key");
        let substituted = publication_idempotency_key(
            b"sorafs-reputation-governance-publication-operation-v1",
            1,
            [0xC1; 32],
            Some([0xC3; 32]),
        )
        .expect("substituted key");
        assert_ne!(signing, governance);
        assert_ne!(governance, substituted);
    }

    #[test]
    fn governance_publisher_key_rejects_weak_and_noncanonical_points() {
        let mut weak_identity = [0_u8; 32];
        weak_identity[0] = 1;
        assert!(!valid_ed25519_verifying_key(weak_identity));
        assert!(!valid_ed25519_verifying_key([0xFF; 32]));
        assert!(valid_ed25519_verifying_key(
            SigningKey::from_bytes(&[0xD1; 32])
                .verifying_key()
                .to_bytes()
        ));
    }
}
