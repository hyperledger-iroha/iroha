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
    collections::BTreeSet,
    fmt,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

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
            StreamTokenValidationOutcomeV1,
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
    GovernanceLogNodeV1, GovernanceLogPayloadV1, GovernanceLogSignatureV1,
    ReputationSnapshotEventV1, ReputationSnapshotV1,
    reputation::signed::{ReputationSnapshotTrustPolicyV1, SignedReputationSnapshotV1},
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
/// Committed reputation read-projection version.
pub const REPUTATION_COMMITTED_READ_PROJECTION_VERSION_V1: u8 = 1;
/// Maximum authoritative snapshots and matching events retained by the read projection.
pub const REPUTATION_COMMITTED_READ_MAX_EVENTS_V1: usize = 1_024;
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

/// Maximum bytes in a pinned external runtime handle.
pub const REPUTATION_RUNTIME_MAX_HANDLE_BYTES_V1: usize = 256;
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
    /// Result of the typed active-policy query at `anchor`.
    pub authority_policy: ReputationJournalAuthorityPolicyRecordV1,
    /// Result of the typed committed-event query at `anchor`.
    pub journal_page: ReputationJournalFinalizedEventPageV1,
}

impl ReputationJournalDeliveryFinalizedViewV1 {
    fn validate(
        &self,
        chain_id: &ChainId,
        requested_after: Option<ReputationJournalFinalizedEventCursorV1>,
        requested_limit: u32,
        maximum_height: u64,
    ) -> Result<(), ReputationRuntimeError> {
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
        self.journal_page
            .validate_after(requested_after)
            .map_err(|_| ReputationRuntimeError::InvalidQueryPage)?;
        if self.journal_page.events.len()
            > usize::try_from(requested_limit)
                .map_err(|_| ReputationRuntimeError::QueryResourceExhausted)?
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

/// Identity-pinned source for all native finalized reputation queries.
///
/// Implementations must execute every page method against the exact `anchor`
/// supplied by the caller. A node that cannot serve the requested immutable
/// view must return a failure rather than silently selecting a newer view.
pub trait ReputationFinalizedQueryV1: Send + Sync + fmt::Debug {
    /// Opaque deployment handle, stable for the lifetime of the provider.
    fn handle(&self) -> &str;

    /// Verify that the exact-anchor query service is authenticated and ready.
    ///
    /// Implementations must not return success for a null, disconnected, or
    /// identity-unverified backend.
    fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1>;

    /// Return the highest finalized anchor at or below `maximum_height`.
    ///
    /// Once the chain has finalized `maximum_height`, the implementation must
    /// return that exact historical anchor rather than the current head.
    fn finalized_at_or_before(
        &self,
        chain_id: &ChainId,
        maximum_height: u64,
    ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1>;

    /// Execute the typed active-policy and journal-page queries in one view.
    ///
    /// Implementations must hold one immutable state-view guard while
    /// executing both queries. They must not assemble this response from two
    /// current-head reads, even when both reads report the same height.
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
        Ok(Self {
            version: REPUTATION_FINALIZED_QUERY_POLICY_VERSION_V1,
            chain_id: ingest_policy.chain_id.clone(),
            window_end_height: ingest_policy.window_end_height,
            ingest_policy_digest: ingest_policy.canonical_digest()?,
            query_handle,
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
            || query.handle() != policy.query_handle
        {
            return Err(ReputationRuntimeError::RuntimeBindingMismatch);
        }
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
        self.query.check_readiness()?;
        self.ensure_query_binding()?;
        if projector_ready_at(&self.projector, self.policy.window_end_height)? {
            return Ok(ReputationFinalizedPollOutcomeV1::Complete);
        }

        let anchor = self
            .query
            .finalized_at_or_before(&self.policy.chain_id, self.policy.window_end_height)?;
        self.ensure_query_binding()?;
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
        if self.query.handle() != self.policy.query_handle {
            return Err(ReputationRuntimeError::RuntimeBindingChanged);
        }
        Ok(())
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
        let page = self
            .query
            .proof_outcome_page(anchor, after, self.policy.page_items)?;
        self.ensure_query_binding()?;
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
        let page = self
            .query
            .reputation_journal_page(anchor, after, self.policy.page_items)?;
        self.ensure_query_binding()?;
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
        let page = self
            .query
            .repair_page(anchor, after, self.policy.page_items)?;
        self.ensure_query_binding()?;
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
        let page = self
            .query
            .orderbook_page(anchor, after, self.policy.page_items)?;
        self.ensure_query_binding()?;
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
        let page = self
            .query
            .reserve_page(anchor, after, self.policy.page_items)?;
        self.ensure_query_binding()?;
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
            let page =
                self.query
                    .reserve_provider_page(anchor, after, RESERVE_QUERY_MAX_ITEMS_V1)?;
            self.ensure_query_binding()?;
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

/// Counted stream-token adapter result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountedStreamTokenProducerOutcomeV1 {
    /// A provider-attributable result entered the durable native outbox.
    Enqueued(ReputationJournalEnqueueOutcomeV1),
    /// An unauthenticated or undecodable token result was valid but not
    /// provider-attributable and therefore did not enter the journal.
    NotCounted,
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
    committed: ReputationCommittedEventIdentityV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationJournalObservationV1 {
    event_id: ReputationJournalEventIdV1,
    source_id: ReputationJournalSourceIdV1,
    entry_digest: [u8; 32],
    committed: ReputationCommittedEventIdentityV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationJournalDeadLetterV1 {
    sequence: u64,
    event_id: ReputationJournalEventIdV1,
    source_id: ReputationJournalSourceIdV1,
    entry_digest: [u8; 32],
    attempts: u32,
    failure_receipts: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReputationJournalProducerCheckpointV1 {
    version: u8,
    policy_digest: [u8; 32],
    authority_policies: Vec<ReputationJournalAuthorityPolicyV1>,
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
        }
    }
}

#[derive(Debug)]
struct JournalProducerRuntimeState {
    checkpoint: ReputationJournalProducerCheckpointV1,
    fingerprint: Option<[u8; 32]>,
}

/// Durable native PoR and counted stream-token journal outbox.
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
    /// their captured authority and policy digest. Only never-exposed Ready
    /// rows are reconstructed under the successor.
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
        record
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidAuthorityPolicy)?;
        finalized
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidFinalizedAnchor)?;
        if record.activated_at_unix_ms > finalized.finalized_at_unix_ms {
            return Err(ReputationRuntimeError::InvalidAuthorityPolicy);
        }
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
        let active_digest = active
            .canonical_digest()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if record.policy_digest == active_digest {
            if record.policy != *active {
                return Err(ReputationRuntimeError::InvalidAuthorityPolicy);
            }
            return match &state.checkpoint.active_authority_policy_record {
                Some(existing) if existing == &record => {
                    Ok(ReputationJournalPolicySyncOutcomeV1::ExactReplay)
                }
                Some(_) => Err(ReputationRuntimeError::AuthorityPolicyRecordConflict),
                None => {
                    let mut candidate = state.checkpoint.clone();
                    candidate.active_authority_policy_record = Some(record);
                    self.commit_journal_candidate(&mut state, candidate)?;
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
            || state.checkpoint.authority_policies.len()
                >= REPUTATION_JOURNAL_PRODUCER_MAX_POLICY_REVISIONS_V1
        {
            return Err(ReputationRuntimeError::AuthorityPolicyLineage);
        }

        let mut candidate = state.checkpoint.clone();
        candidate.authority_policies.push(record.policy.clone());
        candidate.active_authority_policy_record = Some(record.clone());
        let mut rebound_ready = 0_u32;
        for delivery in &mut candidate.pending {
            if delivery.state != ReputationJournalDeliveryStateV1::Ready {
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
            delivery.entry = rebound;
            rebound_ready = rebound_ready
                .checked_add(1)
                .ok_or(ReputationRuntimeError::JournalResourceExhausted)?;
        }
        ensure_retained_journal_identities_unique(&candidate)?;
        self.commit_journal_candidate(&mut state, candidate)?;
        Ok(ReputationJournalPolicySyncOutcomeV1::Rotated { rebound_ready })
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
            if let Some(existing) = candidate.completed.iter().find(|entry| {
                entry.event_id == event.entry.event_id || entry.source_id == event.entry.source_id
            }) {
                if existing.event_id != event.entry.event_id
                    || existing.source_id != event.entry.source_id
                    || existing.entry_digest != digest
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
        self.pending_by_id(event_id)
    }

    /// Rebind one never-exposed Ready row to the active policy and enter ambiguity.
    ///
    /// This is the rotation-safe worker entrypoint. The caller supplies the
    /// digest read from the same exact finalized view used as the absence
    /// baseline. Rows that have ever entered Ambiguous or Submitted are never
    /// rewritten.
    ///
    /// # Errors
    ///
    /// Rejects a stale policy digest, invalid baseline, non-Ready row,
    /// collision, exhausted retry budget, or persistence failure.
    pub fn begin_submission_against_active_policy(
        &self,
        event_id: ReputationJournalEventIdV1,
        active_policy_digest: [u8; 32],
        baseline_finalized: ReputationFinalizedIdentityV1,
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
        if active
            .canonical_digest()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?
            != active_policy_digest
            || state
                .checkpoint
                .active_authority_policy_record
                .as_ref()
                .map(|record| record.policy_digest)
                != Some(active_policy_digest)
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
            if delivery.entry.authority_policy_digest == active_policy_digest {
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
        let active_policy = state
            .checkpoint
            .authority_policies
            .last()
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        let active_policy_digest = active_policy
            .canonical_digest()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        let entry = ReputationJournalEntryV1::try_new(
            provider_id,
            active_policy_digest,
            active_policy
                .recorder_authority(payload.source_kind())
                .clone(),
            source_time_unix_ms,
            None,
            payload,
        )
        .map_err(|_| ReputationRuntimeError::InvalidJournalEntry)?;
        let entry_digest = journal_entry_digest(&entry)?;
        for retained in retained_journal_identities(&state.checkpoint) {
            if retained.source_id == entry.source_id {
                if retained.event_id == entry.event_id && retained.entry_digest == entry_digest {
                    return Ok(ReputationJournalEnqueueOutcomeV1::ExactReplay {
                        event_id: entry.event_id,
                    });
                }
                return Err(ReputationRuntimeError::JournalSourceConflict);
            }
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
        candidate.pending.push(StoredReputationJournalDeliveryV1 {
            sequence,
            entry_digest,
            entry,
            state: ReputationJournalDeliveryStateV1::Ready,
            attempts: 0,
            baseline_finalized: None,
            failure_receipts: Vec::new(),
        });
        candidate.pending.sort_by_key(|entry| entry.sequence);
        self.commit_journal_candidate(&mut state, candidate)?;
        Ok(ReputationJournalEnqueueOutcomeV1::Inserted { event_id })
    }

    fn pending_by_id(
        &self,
        event_id: ReputationJournalEventIdV1,
    ) -> Result<ReputationJournalSubmissionV1, ReputationRuntimeError> {
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
        validate_journal_checkpoint(&candidate, &self.policy, self.policy_digest)?;
        let encoded =
            norito::to_bytes(&candidate).map_err(|_| ReputationRuntimeError::CanonicalEncoding)?;
        if u64::try_from(encoded.len()).unwrap_or(u64::MAX) > self.policy.checkpoint_max_bytes {
            return Err(ReputationRuntimeError::CheckpointTooLarge);
        }
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

/// Stream-token adapter that journals only safely provider-attributable results.
#[derive(Debug, Clone)]
pub struct CountedStreamTokenReputationJournalProducerV1 {
    outbox: Arc<ReputationJournalProducerOutboxV1>,
}

impl CountedStreamTokenReputationJournalProducerV1 {
    /// Bind the adapter to the durable journal outbox.
    #[must_use]
    pub fn new(outbox: Arc<ReputationJournalProducerOutboxV1>) -> Self {
        Self { outbox }
    }

    /// Validate one terminal token result and durably enqueue it only when it
    /// is safely attributable to the provider.
    ///
    /// # Errors
    ///
    /// Rejects malformed typed material, source conflicts, or persistence
    /// failures.
    pub fn enqueue_counted(
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
}

#[derive(Debug, Clone, Copy)]
struct RetainedJournalIdentity {
    event_id: ReputationJournalEventIdV1,
    source_id: ReputationJournalSourceIdV1,
    entry_digest: [u8; 32],
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
        })
        .chain(
            checkpoint
                .completed
                .iter()
                .map(|entry| RetainedJournalIdentity {
                    event_id: entry.event_id,
                    source_id: entry.source_id,
                    entry_digest: entry.entry_digest,
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
            (None, None) => break,
        }
    }
    Ok(())
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

fn journal_entry_digest(
    entry: &ReputationJournalEntryV1,
) -> Result<[u8; 32], ReputationRuntimeError> {
    hash_canonical(b"sorafs-reputation-journal-producer-entry-v1", entry)
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

fn validate_journal_checkpoint(
    checkpoint: &ReputationJournalProducerCheckpointV1,
    policy: &ReputationJournalProducerPolicyV1,
    policy_digest: [u8; 32],
) -> Result<(), ReputationRuntimeError> {
    policy.validate()?;
    if checkpoint.version != REPUTATION_JOURNAL_PRODUCER_CHECKPOINT_VERSION_V1
        || checkpoint.policy_digest != policy_digest
        || checkpoint.authority_policies.is_empty()
        || checkpoint.authority_policies.len() > REPUTATION_JOURNAL_PRODUCER_MAX_POLICY_REVISIONS_V1
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
    {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
    }
    let mut previous_policy: Option<(&ReputationJournalAuthorityPolicyV1, [u8; 32])> = None;
    for authority_policy in &checkpoint.authority_policies {
        authority_policy
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        let digest = authority_policy
            .canonical_digest()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if let Some((previous, previous_digest)) = previous_policy {
            if authority_policy.revision != previous.revision.saturating_add(1)
                || authority_policy.predecessor_policy_digest != Some(previous_digest)
            {
                return Err(ReputationRuntimeError::InvalidCheckpoint);
            }
        }
        previous_policy = Some((authority_policy, digest));
    }
    let active_policy = checkpoint
        .authority_policies
        .last()
        .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
    if let Some(record) = &checkpoint.active_authority_policy_record {
        record
            .validate()
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if record.policy != *active_policy {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
    } else if checkpoint.authority_policies.len() != 1
        || checkpoint.authority_policies.first() != Some(&policy.authority_policy)
    {
        return Err(ReputationRuntimeError::InvalidCheckpoint);
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
    let mut sequences = BTreeSet::new();
    let mut event_ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    let mut committed_sequences = BTreeSet::new();
    let mut max_sequence = 0_u64;
    let mut previous_pending = 0_u64;
    for entry in &checkpoint.pending {
        let entry_policy = checkpoint
            .authority_policies
            .iter()
            .find(|candidate| {
                candidate.canonical_digest().ok() == Some(entry.entry.authority_policy_digest)
            })
            .ok_or(ReputationRuntimeError::InvalidCheckpoint)?;
        entry
            .entry
            .validate_against_policy(entry_policy)
            .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if !matches!(
            entry.entry.source_kind(),
            ReputationJournalSourceKindV1::Por | ReputationJournalSourceKindV1::StreamToken
        ) || entry.sequence == 0
            || entry.sequence <= previous_pending
            || entry.entry_digest == [0; 32]
            || journal_entry_digest(&entry.entry)? != entry.entry_digest
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
        if !sequences.insert(entry.sequence)
            || !event_ids.insert(entry.entry.event_id)
            || !source_ids.insert(entry.entry.source_id)
        {
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
            || entry.committed.validate().is_err()
            || !committed_sequences.insert(entry.committed.sequence)
            || !sequences.insert(entry.sequence)
            || !event_ids.insert(entry.event_id)
            || !source_ids.insert(entry.source_id)
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
            || entry.committed.validate().is_err()
            || entry.committed.sequence <= previous_observed_sequence
            || !committed_sequences.insert(entry.committed.sequence)
            || !event_ids.insert(entry.event_id)
            || !source_ids.insert(entry.source_id)
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
            || entry.attempts != policy.max_attempts
            || entry.failure_receipts.len()
                > usize::try_from(entry.attempts)
                    .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?
            || !valid_failure_receipts(&entry.failure_receipts)
            || !sequences.insert(entry.sequence)
            || !event_ids.insert(entry.event_id)
            || !source_ids.insert(entry.source_id)
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
    transaction_submitter_handle: String,
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
        transaction_submitter_handle: impl Into<String>,
    ) -> Result<Self, ReputationRuntimeError> {
        let page_items = u32::try_from(REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
            .map_err(|_| ReputationRuntimeError::InvalidRuntimePolicy)?;
        let policy = Self {
            version: REPUTATION_JOURNAL_DELIVERY_POLICY_VERSION_V1,
            chain_id,
            finalized_query_handle: validate_runtime_handle(finalized_query_handle.into())?,
            transaction_submitter_handle: validate_runtime_handle(
                transaction_submitter_handle.into(),
            )?,
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
        {
            return Err(ReputationRuntimeError::InvalidRuntimePolicy);
        }
        validate_runtime_handle(self.finalized_query_handle.clone())?;
        validate_runtime_handle(self.transaction_submitter_handle.clone())?;
        Ok(())
    }
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
pub trait ReputationJournalTransactionSubmitterV1: Send + Sync + fmt::Debug {
    /// Opaque deployment handle, stable for the submitter lifetime.
    fn handle(&self) -> &str;

    /// Verify authenticated signer and transaction-queue readiness.
    fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1>;

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
        if outbox.policy.chain_id != policy.chain_id
            || query.handle() != policy.finalized_query_handle
            || submitter.handle() != policy.transaction_submitter_handle
        {
            return Err(ReputationRuntimeError::RuntimeBindingMismatch);
        }
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

    /// Return a concrete production callback for counted stream-token owners.
    #[must_use]
    pub fn counted_stream_token_producer(&self) -> CountedStreamTokenReputationJournalProducerV1 {
        CountedStreamTokenReputationJournalProducerV1::new(Arc::clone(&self.outbox))
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
        self.query.check_readiness()?;
        self.submitter.check_readiness()?;
        self.ensure_bindings()?;

        let mut pages = 0_u32;
        let mut committed = 0_u32;
        let terminal_view = loop {
            if pages >= self.policy.max_pages_per_tick {
                return Err(ReputationRuntimeError::QueryResourceExhausted);
            }
            let scan = self.outbox.scan_status()?;
            let requested_after = scan.after;
            let view = self.query.reputation_journal_delivery_view(
                &self.policy.chain_id,
                u64::MAX,
                FindSorafsReputationJournalAuthorityPolicy,
                requested_after,
                self.policy.page_items,
            )?;
            self.ensure_bindings()?;
            view.validate(
                &self.policy.chain_id,
                requested_after,
                self.policy.page_items,
                u64::MAX,
            )?;
            if matches!(
                self.outbox.synchronize_authority_policy(
                    view.authority_policy.clone(),
                    view.journal_page.finalized_cursor,
                )?,
                ReputationJournalPolicySyncOutcomeV1::Rotated { .. }
            ) {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
                state.metrics.policy_rotations = state.metrics.policy_rotations.saturating_add(1);
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
            let submission = self.outbox.begin_submission_against_active_policy(
                pending.event_id,
                terminal_view.authority_policy.policy_digest,
                current_identity,
            )?;
            if !self.submitter.supports_authority(&submission.authority) {
                return Err(ReputationRuntimeError::JournalSubmitterAuthorityMismatch);
            }
            let request = journal_transaction_request(submission)?;
            let outcome = self.submitter.submit(&request);
            self.ensure_bindings()?;
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
        if self.query.handle() != self.policy.finalized_query_handle
            || self.submitter.handle() != self.policy.transaction_submitter_handle
        {
            return Err(ReputationRuntimeError::RuntimeBindingChanged);
        }
        Ok(())
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
pub trait ReputationThresholdSignerClientV1: Send + Sync + fmt::Debug {
    /// Opaque deployment handle, stable for the provider lifetime.
    fn handle(&self) -> &str;

    /// Verify authenticated connectivity and governed signer readiness.
    fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1>;

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

/// Identity-pinned Governance DAG publication and readback client.
///
/// A successful acknowledgement is the exact signed DAG block read back from
/// the governed publication service. A submitter-side success without this
/// block remains pending.
pub trait ReputationGovernanceDagClientV1: Send + Sync + fmt::Debug {
    /// Opaque deployment handle, stable for the provider lifetime.
    fn handle(&self) -> &str;

    /// Verify authenticated publication/readback readiness.
    fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1>;

    /// Publish or reconcile the exact snapshot, returning its signed DAG block
    /// only after authenticated readback.
    fn reconcile_publication(
        &self,
        request: &ReputationGovernanceDagPublicationRequestV1,
    ) -> Result<Option<GovernanceDagBlockV1>, ReputationExternalFailureV1>;
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

/// Compact durable form of the authenticated Governance DAG readback.
///
/// The signed reputation payload is already retained by the publication
/// checkpoint, so persisting the full block would duplicate its potentially
/// large provider inventory. These fields are sufficient to reconstruct the
/// exact block and re-run both node and block signature verification.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredReputationGovernanceDagReadbackV1 {
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

impl StoredReputationGovernanceDagReadbackV1 {
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

/// Strict external publication contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationPublicationPolicyV1 {
    trust_policy_digest: [u8; 32],
    threshold_signer_handle: String,
    governance_dag_handle: String,
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
            || checkpoint_max_bytes < REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1
            || checkpoint_max_bytes > REPUTATION_PUBLICATION_MAX_CHECKPOINT_BYTES_V1
        {
            return Err(ReputationRuntimeError::InvalidRuntimePolicy);
        }
        let governance_publisher_key_digest = domain_digest(
            b"sorafs-reputation-governance-publisher-key-v1",
            &governance_publisher_public_key,
        )?;
        Ok(Self {
            trust_policy_digest,
            threshold_signer_handle,
            governance_dag_handle,
            governance_publisher_peer_id,
            governance_publisher_public_key,
            governance_publisher_key_digest,
            checkpoint_max_bytes,
        })
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
    ) -> Result<(), ReputationRuntimeError> {
        verify_persisted_signed_result(&self.signed_result, trust_policy)?;
        if self.sequence == 0
            || self.material_digest == [0; 32]
            || self.signed_result_digest == [0; 32]
            || self.signed_result.policy_digest != policy.trust_policy_digest
            || signed_result_digest(&self.signed_result)? != self.signed_result_digest
        {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        let block = governance_readback.reconstruct_block(&self.signed_result);
        let expected_acknowledgement = governance_acknowledgement_from_block(
            policy,
            self.sequence,
            self.material_digest,
            self.signed_result_digest,
            &self.signed_result,
            &block,
        )
        .map_err(|_| ReputationRuntimeError::InvalidCheckpoint)?;
        if expected_acknowledgement != self.governance_acknowledgement {
            return Err(ReputationRuntimeError::InvalidCheckpoint);
        }
        Ok(())
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
    ) -> Result<bool, ReputationRuntimeError> {
        committed.validate(policy, trust_policy, governance_readback)?;
        if let Some(existing) = &self.latest {
            if existing == &committed {
                return Ok(false);
            }
        }
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
            || self.latest.is_some() != !self.events.is_empty()
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
        let appended = self.committed_read.append(
            committed.clone(),
            policy,
            trust_policy,
            &governance_readback,
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
            || threshold_signer.handle() != policy.threshold_signer_handle
            || governance_dag.handle() != policy.governance_dag_handle
        {
            return Err(ReputationRuntimeError::RuntimeBindingMismatch);
        }
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
            let signed = match self.threshold_signer.reconcile_signature(&request) {
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
            self.ensure_external_bindings()?;
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
            let block = match self.governance_dag.reconcile_publication(&request) {
                Ok(Some(block)) => block,
                Ok(None) => return Ok(ReputationPublicationOutcomeV1::AwaitingGovernanceDag),
                Err(failure) => {
                    return self.record_external_failure(
                        &delivery,
                        request.idempotency_key,
                        failure,
                    );
                }
            };
            self.ensure_external_bindings()?;
            let (acknowledgement, readback) = self.validate_governance_block(&pending, &block)?;
            self.store_governance_readback(acknowledgement, readback)?;
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

    fn validate_governance_block(
        &self,
        pending: &StoredReputationPublicationV1,
        block: &GovernanceDagBlockV1,
    ) -> Result<
        (
            ReputationGovernanceDagAcknowledgementV1,
            StoredReputationGovernanceDagReadbackV1,
        ),
        ReputationRuntimeError,
    > {
        let acknowledgement = governance_acknowledgement_from_block(
            &self.policy,
            pending.sequence,
            pending.material_digest,
            pending.signed_result_digest,
            &pending.signed_result,
            block,
        )?;
        Ok((
            acknowledgement,
            StoredReputationGovernanceDagReadbackV1::from_block(block),
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
        self.ensure_external_bindings()?;
        self.threshold_signer.check_readiness()?;
        self.governance_dag.check_readiness()?;
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
        if self.threshold_signer.handle() != self.policy.threshold_signer_handle
            || self.governance_dag.handle() != self.policy.governance_dag_handle
        {
            return Err(ReputationRuntimeError::RuntimeBindingChanged);
        }
        Ok(())
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

fn governance_acknowledgement_from_block(
    policy: &ReputationPublicationPolicyV1,
    sequence: u64,
    material_digest: [u8; 32],
    signed_result_digest: [u8; 32],
    signed_result: &SignedReputationSnapshotV1,
    block: &GovernanceDagBlockV1,
) -> Result<ReputationGovernanceDagAcknowledgementV1, ReputationRuntimeError> {
    block
        .validate()
        .map_err(|_| ReputationRuntimeError::InvalidGovernanceAcknowledgement)?;
    if sequence == 0
        || material_digest == [0; 32]
        || signed_result_digest == [0; 32]
        || block.publisher_peer_id != policy.governance_publisher_peer_id
        || block.node.publisher_peer_id != policy.governance_publisher_peer_id
        || block.block_signature.public_key != policy.governance_publisher_public_key
        || block.node.publisher_signature.public_key != policy.governance_publisher_public_key
        || block.node.payload
            != GovernanceLogPayloadV1::SignedReputationSnapshot(signed_result.clone())
        || block.node.timestamp < signed_result.snapshot.generated_at_unix
        || block.timestamp < signed_result.snapshot.generated_at_unix
    {
        return Err(ReputationRuntimeError::InvalidGovernanceAcknowledgement);
    }
    let acknowledgement = ReputationGovernanceDagAcknowledgementV1 {
        version: REPUTATION_GOVERNANCE_DAG_ACKNOWLEDGEMENT_VERSION_V1,
        sequence,
        material_digest,
        signed_result_digest,
        dag_block_sequence: block.sequence,
        dag_block_cid: exact_32(&block.block_cid)
            .ok_or(ReputationRuntimeError::InvalidGovernanceAcknowledgement)?,
        dag_node_cid: exact_32(&block.node.node_cid)
            .ok_or(ReputationRuntimeError::InvalidGovernanceAcknowledgement)?,
        published_at_unix: block.timestamp,
        publisher_key_digest: policy.governance_publisher_key_digest,
    };
    acknowledgement.validate()?;
    Ok(acknowledgement)
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
    for ((committed, governance_readback), event) in checkpoint
        .committed_snapshots
        .iter()
        .zip(&checkpoint.committed_governance_readbacks)
        .zip(&checkpoint.committed_read.events)
    {
        committed.validate(policy, trust_policy, governance_readback)?;
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
                let block = readback.reconstruct_block(&pending.signed_result);
                let expected_acknowledgement = governance_acknowledgement_from_block(
                    policy,
                    pending.sequence,
                    pending.material_digest,
                    pending.signed_result_digest,
                    &pending.signed_result,
                    &block,
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

    /// Return the concrete callback injected into the stream-token owner.
    #[must_use]
    pub fn counted_stream_token_journal_producer(
        &self,
    ) -> Option<CountedStreamTokenReputationJournalProducerV1> {
        self.journal_delivery
            .as_ref()
            .map(ReputationJournalDeliveryWorkerV1::counted_stream_token_producer)
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
    /// Publication durable state conflicts with projector/external state.
    #[error("reputation publication checkpoint conflicts with reconciliation state")]
    PublicationCheckpointConflict,
    /// Canonical Norito encoding failed.
    #[error("reputation runtime canonical encoding failed")]
    CanonicalEncoding,
    /// A durable checkpoint is malformed, noncanonical, or inconsistent.
    #[error("reputation runtime checkpoint is invalid")]
    InvalidCheckpoint,
    /// A durable checkpoint exceeds its hard byte ceiling.
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
    if handle.is_empty()
        || handle.len() > REPUTATION_RUNTIME_MAX_HANDLE_BYTES_V1
        || handle.trim() != handle
        || handle.chars().any(char::is_control)
    {
        return Err(ReputationRuntimeError::InvalidRuntimePolicy);
    }
    Ok(handle)
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
    use std::{collections::VecDeque, fs};

    use ed25519_dalek::{Signer, SigningKey};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::sorafs::reputation::{
        PorTerminalStatusV1, REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
        ReputationJournalFinalizedEventV1, StreamTokenExcludedKindV1,
        StreamTokenValidationStatusV1,
    };
    use sorafs_manifest::GovernanceSignatureAlgorithm;
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
            validation_id: [validation_marker; 32],
            request_digest: [request_marker; 32],
            token_body_digest: Some([0x52; 32]),
            token_key_version: Some(1),
            validated_at_unix_ms: FINALIZED_AT_MS,
            status: StreamTokenValidationStatusV1::Accepted,
        }
    }

    fn excluded_token(validation_marker: u8) -> StreamTokenValidationOutcomeV1 {
        StreamTokenValidationOutcomeV1 {
            validation_id: [validation_marker; 32],
            request_digest: [0x61; 32],
            token_body_digest: None,
            token_key_version: None,
            validated_at_unix_ms: FINALIZED_AT_MS,
            status: StreamTokenValidationStatusV1::Excluded(
                StreamTokenExcludedKindV1::MissingToken,
            ),
        }
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

    fn governance_block(signed_result: &SignedReputationSnapshotV1) -> GovernanceDagBlockV1 {
        let signing_key = SigningKey::from_bytes(&[0xB1; 32]);
        let publisher_peer_id = b"peer-a".to_vec();
        let publication_timestamp = signed_result.snapshot.generated_at_unix;
        let mut node = GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: Vec::new(),
            prev_cid: None,
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
            prev_block_cid: None,
            sequence: 0,
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
        let block = governance_block(signed_result);
        let acknowledgement = governance_acknowledgement_from_block(
            policy,
            sequence,
            material_digest,
            signed_result_digest,
            signed_result,
            &block,
        )
        .expect("derive governance acknowledgement");
        (
            acknowledgement,
            StoredReputationGovernanceDagReadbackV1::from_block(&block),
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
    }

    impl ReputationFinalizedQueryV1 for NullQuery {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            Ok(())
        }

        fn finalized_at_or_before(
            &self,
            _chain_id: &ChainId,
            _maximum_height: u64,
        ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1> {
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
        views: Mutex<VecDeque<ReputationJournalDeliveryFinalizedViewV1>>,
    }

    impl ReputationFinalizedQueryV1 for ScriptedDeliveryQuery {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            Ok(())
        }

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
        requests: Mutex<Vec<ReputationJournalTransactionRequestV1>>,
    }

    impl ReputationJournalTransactionSubmitterV1 for RecordingJournalSubmitter {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            Ok(())
        }

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
    }

    impl ReputationThresholdSignerClientV1 for NullThresholdSigner {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            Ok(())
        }

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
    }

    impl ReputationGovernanceDagClientV1 for NullGovernanceDag {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            Ok(())
        }

        fn reconcile_publication(
            &self,
            _request: &ReputationGovernanceDagPublicationRequestV1,
        ) -> Result<Option<GovernanceDagBlockV1>, ReputationExternalFailureV1> {
            Ok(None)
        }
    }

    fn open_publication_reconciler(
        root: &Path,
        projector: Arc<ReputationIngestService>,
        trust_policy: ReputationSnapshotTrustPolicyV1,
        policy: ReputationPublicationPolicyV1,
    ) -> ReputationPublicationReconcilerV1 {
        ReputationPublicationReconcilerV1::open(
            root,
            projector,
            trust_policy,
            policy,
            Arc::new(NullThresholdSigner {
                handle: "signer-a".to_owned(),
            }),
            Arc::new(NullGovernanceDag {
                handle: "dag-a".to_owned(),
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
        let result = ReputationCommittedProjectorRuntimeV1::new(
            projector,
            &ingest,
            policy,
            Arc::new(NullQuery {
                handle: "query-b".to_owned(),
            }),
        );
        assert!(matches!(
            result,
            Err(ReputationRuntimeError::RuntimeBindingMismatch)
        ));
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
    fn counted_token_adapter_filters_unattributable_attempts() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), producer_policy())
                .expect("producer outbox"),
        );
        let producer = CountedStreamTokenReputationJournalProducerV1::new(outbox.clone());
        assert_eq!(
            producer
                .enqueue_counted(provider(9), excluded_token(1))
                .expect("valid excluded token"),
            CountedStreamTokenProducerOutcomeV1::NotCounted
        );
        assert!(outbox.pending(8).expect("pending").is_empty());

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
    fn producer_rejects_same_source_with_substituted_material() {
        let temp = TempDir::new().expect("tempdir");
        let outbox = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), producer_policy())
                .expect("producer outbox"),
        );
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
    fn ambiguous_journal_append_survives_restart_and_requires_later_finality() {
        let temp = TempDir::new().expect("tempdir");
        let policy = producer_policy();
        let outbox = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), policy.clone()).expect("outbox"),
        );
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
        let restored_submission = restored
            .pending_by_id(event_id)
            .expect("restore exact submitted instruction");
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
        let outbox = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), producer_policy())
                .expect("producer outbox"),
        );
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
        let outbox =
            ReputationJournalProducerOutboxV1::open(temp.path(), policy.clone()).expect("outbox");
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
        let outbox = Arc::new(
            ReputationJournalProducerOutboxV1::open(
                temp.path(),
                ReputationJournalProducerPolicyV1::strict_v1(
                    ChainId::from("reputation-runtime-test"),
                    policy,
                )
                .expect("producer policy"),
            )
            .expect("outbox"),
        );
        let event_id = match PorReputationJournalProducerV1::new(Arc::clone(&outbox))
            .enqueue_terminal(provider(7), verified_por(0x31))
            .expect("enqueue terminal")
        {
            ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            | ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id } => event_id,
        };
        let query = Arc::new(ScriptedDeliveryQuery {
            handle: "ledger.finalized.primary".to_owned(),
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
            requests: Mutex::new(Vec::new()),
        });
        let worker = ReputationJournalDeliveryWorkerV1::new(
            Arc::clone(&outbox),
            ReputationJournalDeliveryPolicyV1::strict_v1(
                ChainId::from("reputation-runtime-test"),
                query.handle.clone(),
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
    fn policy_rotation_rebinds_only_ready_rows_and_preserves_ambiguous_bytes() {
        let temp = TempDir::new().expect("tempdir");
        let first_policy = journal_authority_policy();
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
                authority_record(first_policy.clone(), FINALIZED_AT_MS - 1_000),
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
        let successor_record = authority_record(successor.clone(), FINALIZED_AT_MS + 150);
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
        let pending = outbox.pending(8).expect("pending");
        assert!(
            pending.iter().any(|row| {
                row.event_id == ambiguous.event_id
                    && row.state == ReputationJournalDeliveryStateV1::Ambiguous
            }),
            "ambiguous exact bytes must remain immutable across rotation"
        );
        assert!(
            pending.iter().all(|row| row.event_id != second_id),
            "the never-exposed Ready row must be rebound to the successor"
        );

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
            )
            .expect("rebind retry");
        assert_ne!(rebound.event_id, first_id);
        assert_eq!(rebound.authority, successor.por_recorder_authority);
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
        let outbox = Arc::new(
            ReputationJournalProducerOutboxV1::open(temp.path(), policy.clone()).expect("outbox"),
        );
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
        let result = ReputationPublicationReconcilerV1::open(
            publication_root.path(),
            projector,
            trust,
            policy,
            Arc::new(NullThresholdSigner {
                handle: "signer-b".to_owned(),
            }),
            Arc::new(NullGovernanceDag {
                handle: "dag-a".to_owned(),
            }),
        );
        assert!(matches!(
            result,
            Err(ReputationRuntimeError::RuntimeBindingMismatch)
        ));
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
        assert!(matches!(
            ReputationPublicationReconcilerV1::open(
                publication_root.path(),
                projector,
                trust,
                policy,
                Arc::new(NullThresholdSigner {
                    handle: "signer-a".to_owned(),
                }),
                Arc::new(NullGovernanceDag {
                    handle: "dag-a".to_owned(),
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
        let (second_acknowledgement, second_readback) = governance_readback(
            &policy,
            second_delivery.sequence,
            second_delivery.material_digest,
            second_digest,
            &second,
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
            let (acknowledgement, governance_readback) = governance_readback(
                &policy,
                sequence,
                material_digest,
                signed_result_digest,
                &signed,
            );
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
        pending.signed_result.signatures[0].signature[0] ^= 0x01;
        pending.signed_result_digest =
            signed_result_digest(&pending.signed_result).expect("forged structural digest");
        let forged = norito::to_bytes(&checkpoint).expect("canonical forged checkpoint");
        assert!(matches!(
            decode_publication_checkpoint(&forged, &policy, policy_digest, &trust),
            Err(ReputationRuntimeError::InvalidCheckpoint)
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
