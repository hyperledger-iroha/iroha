//! Durable finalized-ledger billing, statement delivery, and hedge-intent service.
//!
//! This module is deliberately a projector and delivery coordinator. Billing inputs must arrive as
//! typed, contiguous pages from a finalized native-ledger query. The checkpoint retains rebuildable
//! accrual material, statement delivery state, acknowledgements, and hedge intents; it never
//! becomes an independent authority for orderbook, reserve/rent, metering, or penalty state.
//! Statement signatures are produced by a runtime-only HSM/KMS provider, and automatic hedge
//! execution is not exposed by this V1 service.
//!
//! Checkpoint compaction and signer/feed-policy rotation use one consensus-authenticated,
//! governance-signed epoch transition plus a runtime-only sealed monotonic witness archive. The
//! service hard-stops before dropping unsettled state and rejects silent history truncation, policy
//! substitution, rollback, and skipped predecessors.
use crate::durable_transaction_forwarder::{AtomicCheckpointStore, CheckpointStoreError};
use ed25519_dalek::{Signature, VerifyingKey};
use iroha_config::parameters::{ProductionRuntimeHandleError, validate_production_runtime_handle};
use iroha_data_model::{NetworkId, account::AccountId};
use norito::{
    DecodeLimits, NoritoSerialize, decode_from_bytes_with_limits,
    derive::{
        JsonDeserialize as DeriveJsonDeserialize, JsonSerialize as DeriveJsonSerialize,
        NoritoDeserialize as DeriveNoritoDeserialize, NoritoSerialize as DeriveNoritoSerialize,
    },
};
use sorafs_manifest::{
    deal::XorQuantity,
    hedging::{
        BillingLineDirectionV1, BillingLineItemKindV1, MAX_BILLING_ACCOUNT_ID_BYTES,
        MAX_BILLING_LINES, MAX_HEDGING_IDENTIFIER_BYTES, build_billing_line_item_v1,
        build_billing_statement_v1,
        signed::{
            GovernedBillingStatementV1, GovernedHedgingReferencePriceDecisionV1,
            HedgingFeedTrustPolicyV1, MAX_GOVERNED_BILLING_STATEMENT_BYTES, SignedHedgingError,
            bind_governed_billing_statement_v1,
        },
        validate_billing_statement_transition,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Path,
    sync::{Arc, Mutex},
};
use thiserror::Error;
/// Durable checkpoint schema version.
pub const HEDGING_BILLING_CHECKPOINT_VERSION_V1: u8 = 1;
/// Finalized billing page schema version.
pub const HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1: u8 = 1;
/// Finalized billing event schema version.
pub const HEDGING_BILLING_FINALIZED_EVENT_VERSION_V1: u8 = 1;
/// Billing policy schema version.
pub const HEDGING_BILLING_POLICY_VERSION_V1: u8 = 1;
/// Statement signer policy schema version.
pub const BILLING_STATEMENT_SIGNER_POLICY_VERSION_V1: u8 = 1;
/// Signed governed statement schema version.
pub const SIGNED_GOVERNED_BILLING_STATEMENT_VERSION_V1: u8 = 1;
/// Publication receipt schema version.
pub const BILLING_STATEMENT_PUBLICATION_RECEIPT_VERSION_V1: u8 = 1;
/// Acknowledgement schema version.
pub const BILLING_STATEMENT_ACKNOWLEDGEMENT_VERSION_V1: u8 = 1;
/// Hedge-intent schema version.
pub const HEDGE_INTENT_VERSION_V1: u8 = 1;
/// Consensus journal commitment schema version.
pub const HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1: u8 = 1;
/// Consensus-authenticated period-close schema version.
pub const HEDGING_BILLING_PERIOD_CLOSE_VERSION_V1: u8 = 1;
/// Statement publisher signer policy schema version.
pub const BILLING_STATEMENT_PUBLISHER_POLICY_VERSION_V1: u8 = 1;
/// Governed hedge-execution policy schema version.
pub const HEDGE_EXECUTION_POLICY_VERSION_V1: u8 = 1;
/// Explicit operator authorization schema version.
pub const HEDGE_EXECUTION_AUTHORIZATION_VERSION_V1: u8 = 1;
/// Venue submission receipt schema version.
pub const HEDGE_EXECUTION_RECEIPT_VERSION_V1: u8 = 1;
/// Billing epoch-transition authority schema version.
pub const HEDGING_BILLING_TRANSITION_AUTHORITY_VERSION_V1: u8 = 1;
/// Authenticated compaction/policy-transition schema version.
pub const HEDGING_BILLING_EPOCH_TRANSITION_VERSION_V1: u8 = 1;
/// Runtime sealed epoch-witness record schema version.
pub const HEDGING_BILLING_EPOCH_WITNESS_RECORD_VERSION_V1: u8 = 1;
/// Canonical durable checkpoint file name.
pub const HEDGING_BILLING_CHECKPOINT_FILE_NAME_V1: &str =
    "hedging-billing-committed-projector-v1.to";
/// Canonical single-writer lock file name.
pub const HEDGING_BILLING_LOCK_FILE_NAME_V1: &str = "hedging-billing-committed-projector-v1.lock";
/// Maximum finalized events accepted in one page.
pub const HEDGING_BILLING_MAX_EVENTS_PER_PAGE_V1: u32 = 65_536;
/// Maximum finalized query pages consumed in one reconciliation scan.
pub const HEDGING_BILLING_MAX_PAGES_PER_SCAN_V1: u32 = 4_096;
/// Maximum retained consensus-authenticated source pages.
pub const HEDGING_BILLING_MAX_RETAINED_SOURCE_PAGES_V1: u32 = 262_144;
/// Maximum retained consensus-authenticated period closes.
pub const HEDGING_BILLING_MAX_RETAINED_PERIOD_CLOSES_V1: u32 = 262_144;
/// Maximum accounts retained by one service instance.
pub const HEDGING_BILLING_MAX_ACCOUNTS_V1: u32 = 65_536;
/// Maximum unstatemented accruals retained.
pub const HEDGING_BILLING_MAX_OPEN_ACCRUALS_V1: u32 = 262_144;
/// Maximum semantic replay receipts retained.
pub const HEDGING_BILLING_MAX_REPLAY_RECEIPTS_V1: u32 = 1_048_576;
/// Maximum statements retained by one release checkpoint.
pub const HEDGING_BILLING_MAX_STATEMENTS_V1: u32 = 262_144;
/// Maximum acknowledgements retained by one release checkpoint.
pub const HEDGING_BILLING_MAX_ACKNOWLEDGEMENTS_V1: u32 = 262_144;
/// Maximum hedge intents retained by one release checkpoint.
pub const HEDGING_BILLING_MAX_HEDGE_INTENTS_V1: u32 = 65_536;
/// Maximum failed signing attempts for one statement.
pub const HEDGING_BILLING_MAX_SIGNING_ATTEMPTS_V1: u32 = 64;
/// Maximum canonical checkpoint length.
pub const HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1: u64 = 128 * 1024 * 1024;
/// Minimum canonical checkpoint ceiling accepted by a production policy.
pub const HEDGING_BILLING_MIN_CHECKPOINT_BYTES_V1: u64 = 64 * 1024;
/// Maximum canonical signed governed-statement length.
pub const SIGNED_GOVERNED_BILLING_STATEMENT_MAX_BYTES_V1: usize = 20 * 1024 * 1024;
/// Maximum publication route identifier length.
pub const BILLING_PUBLICATION_ROUTE_MAX_BYTES_V1: usize = 256;
/// Maximum signer identifier length.
pub const BILLING_SIGNER_ID_MAX_BYTES_V1: usize = 128;
/// Maximum source identifier length.
pub const BILLING_SOURCE_ID_MAX_BYTES_V1: usize = 512;
/// Maximum consensus proof bytes carried by one page or period close.
pub const HEDGING_BILLING_CONSENSUS_PROOF_MAX_BYTES_V1: usize = 1024 * 1024;
/// Maximum acknowledgement authentication proof bytes.
pub const BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum items returned by one owner or finance projection page.
pub const HEDGING_BILLING_RUNTIME_API_MAX_PAGE_ITEMS_V1: u16 = 100;
/// Maximum statement-delivery work projections returned to one daemon tick.
pub const HEDGING_BILLING_MAX_DELIVERY_WORK_ITEMS_V1: u32 = 4_096;
/// Maximum runtime epoch-witness store handle length.
pub const HEDGING_BILLING_WITNESS_HANDLE_MAX_BYTES_V1: usize = 256;
/// Maximum canonical bytes accepted for one governed service-policy artifact.
pub const HEDGING_BILLING_SERVICE_POLICY_MAX_BYTES_V1: usize = 64 * 1024;
/// Fixed canonical wrapper allowance around one embedded checkpoint witness.
pub const HEDGING_BILLING_EPOCH_WITNESS_WRAPPER_MAX_BYTES_V1: u64 = 16 * 1024;
const CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT: usize = 24;
const CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT: usize = 24;
const CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 4 * 1024 * 1024;
const CHECKPOINT_MAX_NESTING_DEPTH: usize = 96;
const POLICY_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.policy.v1";
const PAGE_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.finalized-page.v1";
const SOURCE_RECEIPT_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.source-receipt.v1";
const EVENT_REPLAY_RECEIPT_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.event-replay-receipt.v1";
const STATEMENT_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.billing.statement-signature.v1";
const PUBLICATION_RECEIPT_DOMAIN_V1: &[u8] = b"sorafs.billing.publication-receipt.v1";
const ACKNOWLEDGEMENT_DOMAIN_V1: &[u8] = b"sorafs.billing.acknowledgement.v1";
const ACKNOWLEDGEMENT_REQUEST_DOMAIN_V1: &[u8] = b"sorafs.billing.acknowledgement-request.v1";
const EXPOSURE_CURSOR_DOMAIN_V1: &[u8] = b"sorafs.billing.exposure-cursor.v1";
const HEDGE_INTENT_DOMAIN_V1: &[u8] = b"sorafs.hedging.intent.v1";
const PERIOD_CLOSE_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.period-close.v1";
const PUBLISHER_RECEIPT_SIGNATURE_DOMAIN_V1: &[u8] =
    b"sorafs.billing.publisher-receipt-signature.v1";
const HEDGE_EXECUTION_POLICY_DOMAIN_V1: &[u8] = b"sorafs.hedging.execution-policy.v1";
const HEDGE_EXECUTION_AUTHORIZATION_DOMAIN_V1: &[u8] = b"sorafs.hedging.execution-authorization.v1";
const HEDGE_EXECUTION_AUTHORIZATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"sorafs.hedging.execution-authorization-signature.v1";
const HEDGE_EXECUTION_RECEIPT_DOMAIN_V1: &[u8] = b"sorafs.hedging.execution-receipt.v1";
const HEDGE_EXECUTION_RECEIPT_SIGNATURE_DOMAIN_V1: &[u8] =
    b"sorafs.hedging.execution-receipt-signature.v1";
const EPOCH_TRANSITION_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.epoch-transition.v1";
const EPOCH_TRANSITION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"sorafs.hedging-billing.epoch-transition-signature.v1";
const EPOCH_WITNESS_RECORD_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.epoch-witness-record.v1";
const COMPACTED_SOURCE_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.compacted-source.v1";
const COMPACTED_ECONOMIC_STATE_DOMAIN_V1: &[u8] =
    b"sorafs.hedging-billing.compacted-economic-state.v1";
const RETAINED_EPOCH_STATE_DOMAIN_V1: &[u8] = b"sorafs.hedging-billing.retained-epoch-state.v1";
/// One finalized block identity.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
pub struct HedgingBillingFinalizedCursorV1 {
    /// Finalized block height.
    pub height: u64,
    /// Exact finalized block hash.
    pub block_hash: [u8; 32],
    /// Finalized block timestamp in Unix seconds.
    pub finalized_at_unix: u64,
}
impl HedgingBillingFinalizedCursorV1 {
    fn validate(self) -> Result<(), HedgingBillingServiceError> {
        if self.height == 0 || self.block_hash == [0; 32] || self.finalized_at_unix == 0 {
            return Err(HedgingBillingServiceError::InvalidFinalizedCursor);
        }
        Ok(())
    }
}
/// Exact consensus-committed billing-journal identity at one finalized cursor.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
)]
pub struct HedgingBillingJournalCommitmentV1 {
    /// Schema version.
    pub version: u8,
    /// Exact genesis-derived network whose journal is committed.
    pub network_id: NetworkId,
    /// Finalized block through which this journal prefix is authoritative.
    pub finalized_cursor: HedgingBillingFinalizedCursorV1,
    /// First journal sequence not committed into `journal_root`.
    pub journal_next_sequence: u64,
    /// Consensus-committed cumulative root for sequences below the tail.
    pub journal_root: [u8; 32],
}
impl HedgingBillingJournalCommitmentV1 {
    fn validate(self, network_id: NetworkId) -> Result<(), HedgingBillingServiceError> {
        self.finalized_cursor.validate()?;
        if self.version != HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1
            || self.network_id != network_id
            || self.journal_next_sequence == 0
            || self.journal_root == [0; 32]
        {
            return Err(HedgingBillingServiceError::InvalidJournalCommitment);
        }
        Ok(())
    }
}
/// Native committed source from which a billing line is projected.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
)]
pub enum BillingAccrualSourceV1 {
    /// Storage-capacity settlement.
    Storage,
    /// Orderbook trade or settlement fee.
    OrderbookSettlement,
    /// Reserve/rent lifecycle charge.
    ReserveRent,
    /// Metered egress charge.
    Egress,
    /// Orchestrator fee.
    OrchestratorFee,
    /// Provider or buyer incentive.
    Incentive,
    /// Governance or SLA penalty.
    Penalty,
    /// Explicit governance-approved adjustment.
    GovernanceAdjustment,
}
/// One typed billing event returned by a finalized native-ledger query.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgingBillingFinalizedEventV1 {
    /// Schema version.
    pub version: u8,
    /// Global contiguous billing-journal sequence, beginning at one.
    pub sequence: u64,
    /// Finalized block height containing the source event.
    pub block_height: u64,
    /// Exact finalized hash for `block_height`.
    pub block_hash: [u8; 32],
    /// Source-event index in the committing block.
    pub event_index: u32,
    /// Source domain.
    pub source: BillingAccrualSourceV1,
    /// Canonical account bytes receiving the statement.
    pub account_id: Vec<u8>,
    /// Canonical source event, trade, settlement, or adjustment identifier.
    pub source_id: String,
    /// Debit or credit projection.
    pub direction: BillingLineDirectionV1,
    /// Canonical statement line category.
    pub kind: BillingLineItemKindV1,
    /// Exact XOR amount.
    pub xor_amount: XorQuantity,
    /// Source-specific metered quantity.
    pub quantity_units: u128,
    /// Committed event timestamp in Unix seconds.
    pub occurred_at_unix: u64,
}
impl HedgingBillingFinalizedEventV1 {
    fn validate(
        &self,
        epoch_unix: u64,
        page_cursor: HedgingBillingFinalizedCursorV1,
    ) -> Result<(), HedgingBillingServiceError> {
        if self.version != HEDGING_BILLING_FINALIZED_EVENT_VERSION_V1
            || self.sequence == 0
            || self.block_height == 0
            || self.block_hash == [0; 32]
            || self.block_height > page_cursor.height
            || (self.block_height == page_cursor.height
                && self.block_hash != page_cursor.block_hash)
            || self.xor_amount.is_zero()
            || self.occurred_at_unix < epoch_unix
            || self.occurred_at_unix > page_cursor.finalized_at_unix
        {
            return Err(HedgingBillingServiceError::InvalidFinalizedEvent);
        }
        validate_canonical_account_id_bytes(&self.account_id)
            .map_err(|_| HedgingBillingServiceError::InvalidFinalizedEvent)?;
        validate_identifier(
            &self.source_id,
            BILLING_SOURCE_ID_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidFinalizedEvent,
        )?;
        canonical_billing_line_source_id(self)?;
        let valid_mapping = match self.source {
            BillingAccrualSourceV1::Storage => {
                self.kind == BillingLineItemKindV1::Storage
                    && self.direction == BillingLineDirectionV1::Debit
                    && self.quantity_units != 0
            }
            BillingAccrualSourceV1::OrderbookSettlement => {
                self.kind == BillingLineItemKindV1::SettlementFee
                    && self.direction == BillingLineDirectionV1::Debit
            }
            BillingAccrualSourceV1::ReserveRent => {
                self.kind == BillingLineItemKindV1::ReserveRent
                    && self.direction == BillingLineDirectionV1::Debit
            }
            BillingAccrualSourceV1::Egress => {
                self.kind == BillingLineItemKindV1::Egress
                    && self.direction == BillingLineDirectionV1::Debit
                    && self.quantity_units != 0
            }
            BillingAccrualSourceV1::OrchestratorFee => {
                self.kind == BillingLineItemKindV1::SettlementFee
                    && self.direction == BillingLineDirectionV1::Debit
            }
            BillingAccrualSourceV1::Incentive => {
                self.kind == BillingLineItemKindV1::IncentiveCredit
                    && self.direction == BillingLineDirectionV1::Credit
            }
            BillingAccrualSourceV1::Penalty => {
                self.kind == BillingLineItemKindV1::Penalty
                    && self.direction == BillingLineDirectionV1::Debit
            }
            BillingAccrualSourceV1::GovernanceAdjustment => {
                self.kind == BillingLineItemKindV1::Adjustment
            }
        };
        if !valid_mapping {
            return Err(HedgingBillingServiceError::InvalidFinalizedEvent);
        }
        Ok(())
    }
}
/// One bounded contiguous page from the authoritative finalized billing journal.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgingBillingFinalizedEventPageV1 {
    /// Schema version.
    pub version: u8,
    /// Exact active chain.
    pub network_id: NetworkId,
    /// First event sequence in this page.
    pub start_sequence: u64,
    /// First event sequence after this page.
    pub next_sequence: u64,
    /// Exact consensus-committed journal tail and cumulative root.
    pub journal_commitment: HedgingBillingJournalCommitmentV1,
    /// Consensus append proof from the previously observed journal commitment.
    pub append_proof: Vec<u8>,
    /// Consensus inclusion proof for the exact event range in this page.
    pub inclusion_proof: Vec<u8>,
    /// Events in strictly contiguous sequence order.
    pub events: Vec<HedgingBillingFinalizedEventV1>,
}
impl HedgingBillingFinalizedEventPageV1 {
    fn validate(
        &self,
        policy: &HedgingBillingServicePolicyV1,
    ) -> Result<[u8; 32], HedgingBillingServiceError> {
        if self.version != HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1
            || self.network_id != policy.network_id
            || self.start_sequence == 0
            || self.events.len() > usize::try_from(policy.max_events_per_page).unwrap_or(usize::MAX)
        {
            return Err(HedgingBillingServiceError::InvalidFinalizedPage);
        }
        self.journal_commitment.validate(policy.network_id)?;
        if self.next_sequence > self.journal_commitment.journal_next_sequence
            || self.append_proof.is_empty()
            || self.append_proof.len() > HEDGING_BILLING_CONSENSUS_PROOF_MAX_BYTES_V1
            || self.inclusion_proof.is_empty()
            || self.inclusion_proof.len() > HEDGING_BILLING_CONSENSUS_PROOF_MAX_BYTES_V1
        {
            return Err(HedgingBillingServiceError::InvalidFinalizedPage);
        }
        let expected_next = self
            .start_sequence
            .checked_add(
                u64::try_from(self.events.len())
                    .map_err(|_| HedgingBillingServiceError::InvalidFinalizedPage)?,
            )
            .ok_or(HedgingBillingServiceError::InvalidFinalizedPage)?;
        if self.next_sequence != expected_next {
            return Err(HedgingBillingServiceError::InvalidFinalizedPage);
        }
        let mut previous_position: Option<(u64, u32)> = None;
        let mut block_hashes = BTreeMap::new();
        for (offset, event) in self.events.iter().enumerate() {
            let expected = self
                .start_sequence
                .checked_add(
                    u64::try_from(offset)
                        .map_err(|_| HedgingBillingServiceError::InvalidFinalizedPage)?,
                )
                .ok_or(HedgingBillingServiceError::InvalidFinalizedPage)?;
            if event.sequence != expected {
                return Err(HedgingBillingServiceError::FinalizedSequenceGap);
            }
            event.validate(
                policy.billing_epoch_unix,
                self.journal_commitment.finalized_cursor,
            )?;
            let position = (event.block_height, event.event_index);
            if previous_position.is_some_and(|previous| previous >= position)
                || block_hashes
                    .insert(event.block_height, event.block_hash)
                    .is_some_and(|previous| previous != event.block_hash)
            {
                return Err(HedgingBillingServiceError::InvalidFinalizedPage);
            }
            previous_position = Some(position);
        }
        hash_canonical(PAGE_DIGEST_DOMAIN_V1, self)
    }
}
/// Durable position supplied to the authoritative finalized-journal query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedgingBillingQueryPositionV1 {
    /// First event sequence not yet projected.
    pub next_sequence: u64,
    /// Latest authenticated consensus journal commitment already observed.
    pub journal_commitment: Option<HedgingBillingJournalCommitmentV1>,
}
/// Public, payload-free qualification for one hedging/billing runtime provider.
///
/// `revision` identifies the deployment-owned adapter and public policy
/// revision. `policy_digest` binds that exact public policy. Production
/// launchers pin both values before opening durable state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedgingBillingRuntimeProviderQualificationV1 {
    revision: u64,
    policy_digest: [u8; 32],
}
impl HedgingBillingRuntimeProviderQualificationV1 {
    /// Construct one provider qualification observation.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }
    /// Return the non-zero deployment adapter/policy revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Return the non-zero digest of the public provider policy.
    #[must_use]
    pub const fn policy_digest(self) -> [u8; 32] {
        self.policy_digest
    }
    fn is_valid(self) -> bool {
        self.revision != 0 && self.policy_digest != [0; 32]
    }
}
/// Stable, payload-free hedging/billing provider-qualification failures.
///
/// Provider implementations retain credentials, key identifiers, customer
/// material, and vendor diagnostics behind the runtime boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HedgingBillingRuntimeProviderQualificationErrorV1 {
    /// The configured opaque provider handle is malformed.
    #[error("configured hedging/billing runtime provider handle is invalid")]
    InvalidConfiguredHandle,
    /// The configured handle is explicitly marked for test or development use.
    #[error("configured hedging/billing runtime provider handle is test-marked")]
    TestMarkedConfiguredHandle,
    /// The injected provider's opaque handle is malformed.
    #[error("injected hedging/billing runtime provider handle is invalid")]
    InvalidProviderHandle,
    /// The injected provider advertises a test- or development-marked handle.
    #[error("injected hedging/billing runtime provider handle is test-marked")]
    TestMarkedProviderHandle,
    /// The configured provider revision or public policy digest is zero.
    #[error("configured hedging/billing runtime provider qualification is invalid")]
    InvalidConfiguredQualification,
    /// The injected provider does not match the configured stable handle.
    #[error("hedging/billing runtime provider handle does not match configuration")]
    SubstitutedProvider,
    /// Qualification could not prove that the provider is current and usable.
    #[error("hedging/billing runtime provider is unavailable, stale, or unqualified")]
    UnavailableOrStale,
    /// The provider returned a zero revision or all-zero public policy digest.
    #[error("hedging/billing runtime provider returned an invalid qualification")]
    InvalidQualification,
    /// The provider does not match the independently governed qualification.
    #[error("hedging/billing runtime provider qualification does not match configuration")]
    QualificationMismatch,
    /// The provider identity or public policy changed after it was pinned.
    #[error("hedging/billing runtime provider identity or policy changed after qualification")]
    IdentityOrPolicyChanged,
}
/// Fixed readiness failures returned by a hedging/billing runtime provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HedgingBillingRuntimeProviderReadinessErrorV1 {
    /// The provider or a required credential is temporarily unavailable.
    #[error("hedging/billing runtime provider unavailable")]
    Unavailable,
    /// The provider is revoked, stale, unauthorized, or otherwise ineligible.
    #[error("hedging/billing runtime provider rejected qualification")]
    Rejected,
}
/// Stable identity and readiness exposed by an external production provider.
///
/// Implementations own credentials, signing keys, authentication material, and provider-specific
/// diagnostics. `qualification` must fail when the provider is unavailable, revoked, stale,
/// test-marked, or otherwise not production-ready.
pub trait HedgingBillingRuntimeProviderV1: Send + Sync + fmt::Debug {
    /// Return the stable opaque deployment handle for this provider.
    fn handle(&self) -> &str;
    /// Qualify the active adapter and its public policy revision.
    fn qualification(
        &self,
    ) -> Result<
        HedgingBillingRuntimeProviderQualificationV1,
        HedgingBillingRuntimeProviderReadinessErrorV1,
    >;
}
/// Qualify one provider against an independently configured exact binding.
///
/// # Errors
///
/// Fails for malformed or test-marked handles, unavailable providers, invalid
/// observations, substitutions, and revision or policy-digest mismatches.
pub fn qualify_hedging_billing_runtime_provider_v1<P: HedgingBillingRuntimeProviderV1 + ?Sized>(
    expected_handle: &str,
    expected_qualification: HedgingBillingRuntimeProviderQualificationV1,
    provider: &P,
) -> Result<(), HedgingBillingRuntimeProviderQualificationErrorV1> {
    validate_hedging_billing_runtime_provider_handle(expected_handle, true)?;
    if !expected_qualification.is_valid() {
        return Err(
            HedgingBillingRuntimeProviderQualificationErrorV1::InvalidConfiguredQualification,
        );
    }
    validate_hedging_billing_runtime_provider_handle(provider.handle(), false)?;
    if provider.handle() != expected_handle {
        return Err(HedgingBillingRuntimeProviderQualificationErrorV1::SubstitutedProvider);
    }
    let qualification = provider
        .qualification()
        .map_err(|_| HedgingBillingRuntimeProviderQualificationErrorV1::UnavailableOrStale)?;
    if !qualification.is_valid() {
        return Err(HedgingBillingRuntimeProviderQualificationErrorV1::InvalidQualification);
    }
    if qualification != expected_qualification {
        return Err(HedgingBillingRuntimeProviderQualificationErrorV1::QualificationMismatch);
    }
    if provider.handle() != expected_handle {
        return Err(HedgingBillingRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    Ok(())
}
/// Revalidate an already pinned provider immediately around external work.
///
/// # Errors
///
/// Fails when readiness, identity, revision, or public policy differs from the
/// exact binding qualified at startup.
pub fn revalidate_hedging_billing_runtime_provider_v1<
    P: HedgingBillingRuntimeProviderV1 + ?Sized,
>(
    expected_handle: &str,
    expected_qualification: HedgingBillingRuntimeProviderQualificationV1,
    provider: &P,
) -> Result<(), HedgingBillingRuntimeProviderQualificationErrorV1> {
    if provider.handle() != expected_handle {
        return Err(HedgingBillingRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    let qualification = provider
        .qualification()
        .map_err(|_| HedgingBillingRuntimeProviderQualificationErrorV1::UnavailableOrStale)?;
    if !qualification.is_valid() {
        return Err(HedgingBillingRuntimeProviderQualificationErrorV1::InvalidQualification);
    }
    if provider.handle() != expected_handle || qualification != expected_qualification {
        return Err(HedgingBillingRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    Ok(())
}
fn validate_hedging_billing_runtime_provider_handle(
    handle: &str,
    configured: bool,
) -> Result<(), HedgingBillingRuntimeProviderQualificationErrorV1> {
    validate_production_runtime_handle(handle).map_err(|error| match (configured, error) {
        (true, ProductionRuntimeHandleError::InvalidSyntax) => {
            HedgingBillingRuntimeProviderQualificationErrorV1::InvalidConfiguredHandle
        }
        (false, ProductionRuntimeHandleError::InvalidSyntax) => {
            HedgingBillingRuntimeProviderQualificationErrorV1::InvalidProviderHandle
        }
        (true, ProductionRuntimeHandleError::TestMarked) => {
            HedgingBillingRuntimeProviderQualificationErrorV1::TestMarkedConfiguredHandle
        }
        (false, ProductionRuntimeHandleError::TestMarked) => {
            HedgingBillingRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle
        }
    })
}
/// Public runtime identity of one non-secret production adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HedgingBillingRuntimeAdapterIdentityV1 {
    /// Stable opaque provider handle resolved by the deployment launcher.
    pub handle: String,
}
/// Runtime verifier for consensus-committed journal pages and period closes.
///
/// Implementations must authenticate finality against the configured chain, verify page inclusion,
/// and prove that a new journal commitment is an append-only successor (or exact replay) of
/// `previous`. When `previous` is a compacted epoch frontier, verification must also authenticate
/// semantic source-identity uniqueness against the native ledger or the sealed compaction archive;
/// the bounded local receipt set no longer contains those older identities. A locally generated
/// hash or an unauthenticated Torii response is not a valid implementation.
pub trait HedgingBillingJournalVerifier: HedgingBillingRuntimeProviderV1 {
    /// Return the current verifier identity.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free dependency failure.
    fn identity(
        &self,
    ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError>;
    /// Authenticate provider readiness without accepting ledger material.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free dependency failure.
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError>;
    /// Authenticate one exact page and its append/inclusion proofs.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when finality, append consistency,
    /// inclusion, or chain authentication cannot be established.
    fn verify_page(
        &self,
        network_id: &NetworkId,
        previous: Option<HedgingBillingJournalCommitmentV1>,
        page: &HedgingBillingFinalizedEventPageV1,
    ) -> Result<(), HedgingBillingExternalError>;
    /// Authenticate one exact consensus-committed period close.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when the close record is not committed
    /// under its declared journal root and finalized cursor.
    fn verify_period_close(
        &self,
        network_id: &NetworkId,
        close: &HedgingBillingFinalizedPeriodCloseV1,
    ) -> Result<(), HedgingBillingExternalError>;
    /// Authenticate one governance-signed compaction frontier against consensus.
    ///
    /// Verification must bind the exact predecessor root/tail and close frontier, archived
    /// source/economic digests and counts, retained account bases, next sequence, and both policy
    /// envelopes. Merely validating the governance signature or proof shape is insufficient.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when any bound archive field or the
    /// consensus authentication proof is invalid.
    fn verify_epoch_transition(
        &self,
        network_id: &NetworkId,
        transition: &HedgingBillingEpochTransitionV1,
    ) -> Result<(), HedgingBillingExternalError>;
}
/// Runtime adapter for the native finalized billing-journal query.
pub trait HedgingBillingFinalizedQuery: HedgingBillingRuntimeProviderV1 {
    /// Return the current exact-view query identity.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free dependency failure.
    fn identity(
        &self,
    ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError>;
    /// Authenticate provider readiness, including typed period-close support.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free dependency failure.
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError>;
    /// Whether this adapter supplies typed consensus-authenticated period-close
    /// records as well as finalized journal pages.
    fn supplies_period_closes(&self) -> bool;
    /// Return the exact current finalized chain head authenticated by this query provider.
    ///
    /// Implementors must not report an unfinalized or locally inferred head. They must durably
    /// preserve monotonic finalized-head identity across provider and daemon restarts. The runtime
    /// independently rejects in-process regression/equivocation and uses this cursor to bound every
    /// query scan as well as projector lag.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free dependency failure.
    fn finalized_head(
        &self,
    ) -> Result<HedgingBillingFinalizedCursorV1, HedgingBillingExternalError>;
    /// Return the next bounded page after the durable projector position.
    ///
    /// `None` means the adapter has no newer complete finalized view. Local process events and
    /// unfinalized subscriptions must never implement this interface.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class without query payloads or credentials.
    fn query_finalized_page(
        &self,
        position: HedgingBillingQueryPositionV1,
        max_events: u32,
    ) -> Result<Option<HedgingBillingFinalizedEventPageV1>, HedgingBillingExternalError>;
    /// Return the exact next finalized period-close record when committed.
    ///
    /// `None` means the requested boundary has not yet finalized. Implementors
    /// must never synthesize a close from local feed or clock state.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free dependency failure.
    fn query_finalized_period_close(
        &self,
        period_end_unix: u64,
        position: HedgingBillingQueryPositionV1,
    ) -> Result<Option<HedgingBillingFinalizedPeriodCloseV1>, HedgingBillingExternalError>;
}
/// Expected runtime identity for the statement-signing HSM/KMS provider.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct BillingStatementSignerPolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Stable governed signer identifier.
    pub signer_id: String,
    /// Exact strong Ed25519 public key.
    pub public_key: [u8; 32],
    /// First finalized height at which this signer may sign.
    pub valid_from_block_height: u64,
    /// First finalized height at which this signer is revoked.
    pub revoked_at_block_height: Option<u64>,
}
impl BillingStatementSignerPolicyV1 {
    fn validate(&self) -> Result<(), HedgingBillingServiceError> {
        if self.version != BILLING_STATEMENT_SIGNER_POLICY_VERSION_V1
            || self.valid_from_block_height == 0
            || self
                .revoked_at_block_height
                .is_some_and(|height| height <= self.valid_from_block_height)
        {
            return Err(HedgingBillingServiceError::InvalidPolicy);
        }
        validate_identifier(
            &self.signer_id,
            BILLING_SIGNER_ID_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidPolicy,
        )?;
        checked_verifying_key(self.public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidPolicy)?;
        Ok(())
    }
    fn active_at(&self, height: u64) -> bool {
        height >= self.valid_from_block_height
            && self
                .revoked_at_block_height
                .is_none_or(|revoked_at| height < revoked_at)
    }
}
/// Governed identity for the authoritative statement publication service.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct BillingStatementPublisherPolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Stable authenticated publisher identifier.
    pub publisher_id: String,
    /// Stable publication route identity.
    pub route_id: String,
    /// Strong Ed25519 key used to sign immutable publication receipts.
    pub public_key: [u8; 32],
}
impl BillingStatementPublisherPolicyV1 {
    fn validate(&self) -> Result<(), HedgingBillingServiceError> {
        if self.version != BILLING_STATEMENT_PUBLISHER_POLICY_VERSION_V1 {
            return Err(HedgingBillingServiceError::InvalidPolicy);
        }
        validate_identifier(
            &self.publisher_id,
            BILLING_SIGNER_ID_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidPolicy,
        )?;
        validate_identifier(
            &self.route_id,
            BILLING_PUBLICATION_ROUTE_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidPolicy,
        )?;
        checked_verifying_key(self.public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidPolicy)?;
        Ok(())
    }
}
/// Governance key allowed to authorize the next billing epoch and policy.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgingBillingTransitionAuthorityV1 {
    /// Schema version.
    pub version: u8,
    /// Stable governed authority identity.
    pub authority_id: String,
    /// Exact Ed25519 verification key.
    pub public_key: [u8; 32],
}
impl HedgingBillingTransitionAuthorityV1 {
    fn validate(&self) -> Result<(), HedgingBillingServiceError> {
        if self.version != HEDGING_BILLING_TRANSITION_AUTHORITY_VERSION_V1 {
            return Err(HedgingBillingServiceError::InvalidPolicy);
        }
        validate_identifier(
            &self.authority_id,
            BILLING_SIGNER_ID_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidPolicy,
        )?;
        checked_verifying_key(self.public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidPolicy)?;
        Ok(())
    }
}
/// Deterministic resource, billing-cycle, signer, and exposure policy.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgingBillingServicePolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Monotonic governed policy revision, beginning at one.
    pub revision: u64,
    /// Exact predecessor policy digest for revisions after one.
    pub predecessor_policy_digest: Option<[u8; 32]>,
    /// Exact chain whose finalized journal may be projected.
    pub network_id: NetworkId,
    /// Reviewed billing policy digest.
    pub billing_policy_digest: [u8; 32],
    /// Exact external feed-trust policy digest.
    pub feed_trust_policy_digest: [u8; 32],
    /// Unix second from which billing periods are measured.
    pub billing_epoch_unix: u64,
    /// Fixed billing-period length.
    pub billing_period_secs: u64,
    /// Delay from period end to payment due time.
    pub payment_due_after_secs: u64,
    /// Maximum feed age used for governed reference decisions.
    pub max_feed_age_secs: u64,
    /// Maximum feed divergence in basis points.
    pub max_divergence_bps: u16,
    /// Exact runtime statement signer policy.
    pub statement_signer: BillingStatementSignerPolicyV1,
    /// Exact authoritative publisher identity and receipt key.
    pub statement_publisher: BillingStatementPublisherPolicyV1,
    /// Authority signing the next epoch/policy transition.
    pub transition_authority: HedgingBillingTransitionAuthorityV1,
    /// Exact runtime-only sealed epoch-witness store handle.
    pub epoch_witness_store_handle: String,
    /// Exposure threshold at which a hedge intent is generated.
    pub hedge_intent_threshold_xor: XorQuantity,
    /// Per-intent exposure ceiling.
    pub max_hedge_intent_xor: XorQuantity,
    /// Hedge-intent expiry after the statement period ends.
    pub hedge_intent_ttl_secs: u64,
    /// Governed maximum slippage for a later explicit execution.
    pub hedge_max_slippage_bps: u16,
    /// Maximum events in one finalized page.
    pub max_events_per_page: u32,
    /// Maximum retained authenticated source pages.
    pub max_retained_source_pages: u32,
    /// Maximum retained authenticated period-close records.
    pub max_retained_period_closes: u32,
    /// Maximum retained accounts.
    pub max_accounts: u32,
    /// Maximum open accruals.
    pub max_open_accruals: u32,
    /// Maximum replay receipts.
    pub max_replay_receipts: u32,
    /// Maximum retained statements.
    pub max_statements: u32,
    /// Maximum retained acknowledgements.
    pub max_acknowledgements: u32,
    /// Maximum retained hedge intents.
    pub max_hedge_intents: u32,
    /// Maximum signing attempts per statement.
    pub max_signing_attempts: u32,
    /// Maximum canonical checkpoint bytes.
    pub checkpoint_max_bytes: u64,
}
impl HedgingBillingServicePolicyV1 {
    /// Validate all deterministic first-release policy bounds.
    ///
    /// # Errors
    ///
    /// Returns [`HedgingBillingServiceError::InvalidPolicy`] when any identity,
    /// time, signer, amount, or resource bound is invalid.
    pub fn validate(&self) -> Result<(), HedgingBillingServiceError> {
        if self.version != HEDGING_BILLING_POLICY_VERSION_V1
            || self.revision == 0
            || match (self.revision, self.predecessor_policy_digest) {
                (1, None) => false,
                (1, Some(_)) => true,
                (_, Some(digest)) => digest == [0; 32],
                (_, None) => true,
            }
            || self.billing_policy_digest == [0; 32]
            || self.feed_trust_policy_digest == [0; 32]
            || self.billing_epoch_unix == 0
            || self.billing_period_secs == 0
            || self.payment_due_after_secs == 0
            || self.max_feed_age_secs == 0
            || self.max_divergence_bps > 10_000
            || self.hedge_intent_threshold_xor.is_zero()
            || self.max_hedge_intent_xor < self.hedge_intent_threshold_xor
            || self.hedge_intent_ttl_secs == 0
            || self.hedge_max_slippage_bps > 10_000
            || self.max_events_per_page == 0
            || self.max_events_per_page > HEDGING_BILLING_MAX_EVENTS_PER_PAGE_V1
            || self.max_retained_source_pages == 0
            || self.max_retained_source_pages > HEDGING_BILLING_MAX_RETAINED_SOURCE_PAGES_V1
            || self.max_retained_period_closes == 0
            || self.max_retained_period_closes > HEDGING_BILLING_MAX_RETAINED_PERIOD_CLOSES_V1
            || self.max_accounts == 0
            || self.max_accounts > HEDGING_BILLING_MAX_ACCOUNTS_V1
            || self.max_open_accruals == 0
            || self.max_open_accruals > HEDGING_BILLING_MAX_OPEN_ACCRUALS_V1
            || self.max_replay_receipts == 0
            || self.max_replay_receipts > HEDGING_BILLING_MAX_REPLAY_RECEIPTS_V1
            || self.max_statements == 0
            || self.max_statements > HEDGING_BILLING_MAX_STATEMENTS_V1
            || self.max_acknowledgements == 0
            || self.max_acknowledgements > HEDGING_BILLING_MAX_ACKNOWLEDGEMENTS_V1
            || self.max_acknowledgements < self.max_statements
            || self.max_hedge_intents == 0
            || self.max_hedge_intents > HEDGING_BILLING_MAX_HEDGE_INTENTS_V1
            || self.max_signing_attempts == 0
            || self.max_signing_attempts > HEDGING_BILLING_MAX_SIGNING_ATTEMPTS_V1
            || self.checkpoint_max_bytes < HEDGING_BILLING_MIN_CHECKPOINT_BYTES_V1
            || self.checkpoint_max_bytes > HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1
        {
            return Err(HedgingBillingServiceError::InvalidPolicy);
        }
        self.statement_signer.validate()?;
        self.statement_publisher.validate()?;
        self.transition_authority.validate()?;
        validate_identifier(
            &self.epoch_witness_store_handle,
            HEDGING_BILLING_WITNESS_HANDLE_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidPolicy,
        )
    }
    /// Return the canonical digest used by policy succession and signed records.
    ///
    /// # Errors
    ///
    /// Rejects invalid policy material or canonical encoding failure.
    pub fn canonical_digest(&self) -> Result<[u8; 32], HedgingBillingServiceError> {
        self.validate()?;
        hash_canonical(POLICY_DIGEST_DOMAIN_V1, self)
    }
    /// Encode one exact canonical public service-policy artifact.
    ///
    /// # Errors
    ///
    /// Rejects invalid policy material or an encoding above the V1 policy artifact ceiling.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, HedgingBillingServiceError> {
        self.validate()?;
        let bytes =
            norito::to_bytes(self).map_err(|_| HedgingBillingServiceError::EncodingFailed)?;
        if bytes.is_empty() || bytes.len() > HEDGING_BILLING_SERVICE_POLICY_MAX_BYTES_V1 {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        Ok(bytes)
    }
    /// Decode one exact canonical public service-policy artifact.
    ///
    /// # Errors
    ///
    /// Rejects malformed, oversized, noncanonical, or invalid policy bytes.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, HedgingBillingServiceError> {
        if bytes.is_empty() || bytes.len() > HEDGING_BILLING_SERVICE_POLICY_MAX_BYTES_V1 {
            return Err(HedgingBillingServiceError::InvalidPolicy);
        }
        let max_bytes = HEDGING_BILLING_SERVICE_POLICY_MAX_BYTES_V1;
        let policy = decode_from_bytes_with_limits::<Self>(
            bytes,
            DecodeLimits::new(
                max_bytes,
                max_bytes,
                max_bytes.saturating_mul(CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT),
                max_bytes
                    .saturating_mul(CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT)
                    .saturating_add(CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES),
                CHECKPOINT_MAX_NESTING_DEPTH,
            ),
        )
        .map_err(|_| HedgingBillingServiceError::InvalidPolicy)?;
        if policy.canonical_bytes()?.as_slice() != bytes {
            return Err(HedgingBillingServiceError::InvalidPolicy);
        }
        Ok(policy)
    }
    fn digest(&self) -> Result<[u8; 32], HedgingBillingServiceError> {
        self.canonical_digest()
    }
}
/// Counts of exact records archived by one authenticated epoch transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgingBillingCompactionCountsV1 {
    /// Removed finalized source pages.
    pub source_pages: u64,
    /// Removed finalized source events.
    pub source_events: u64,
    /// Removed period-close records.
    pub period_closes: u64,
    /// Removed fully acknowledged statement delivery records.
    pub statements: u64,
    /// Removed authoritative acknowledgements.
    pub acknowledgements: u64,
    /// Archived hedge projections.
    pub hedge_intents: u64,
}
/// Signed audit witness for compaction and an optional signer/feed-policy rotation.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgingBillingEpochTransitionV1 {
    /// Schema version.
    pub version: u8,
    /// New monotonic checkpoint epoch.
    pub epoch_sequence: u64,
    /// Exact prior checkpoint epoch.
    pub predecessor_epoch_sequence: u64,
    /// Digest of the preceding epoch witness, absent only for epoch one.
    pub predecessor_transition_digest: Option<[u8; 32]>,
    /// Exact committed journal frontier being compacted.
    pub predecessor_journal_commitment: HedgingBillingJournalCommitmentV1,
    /// Last closed billing period archived by this transition.
    pub compacted_through_period_end_unix: u64,
    /// First source sequence retained in the new epoch.
    pub next_source_sequence: u64,
    /// Exact archived record counts.
    pub compacted_counts: HedgingBillingCompactionCountsV1,
    /// Digest of removed pages, events, and replay material.
    pub compacted_source_digest: [u8; 32],
    /// Digest of removed signed statements, receipts, acknowledgements, and intents.
    pub compacted_economic_state_digest: [u8; 32],
    /// Digest of the bounded latest acknowledged statement retained per account.
    pub compacted_account_bases_digest: [u8; 32],
    /// Digest of the immutable epoch-base state installed by this witness.
    pub retained_epoch_state_digest: [u8; 32],
    /// Exact previously active service policy.
    pub previous_service_policy: HedgingBillingServicePolicyV1,
    /// Exact next service policy.
    pub next_service_policy: HedgingBillingServicePolicyV1,
    /// Exact previously active feed policy.
    pub previous_feed_policy: HedgingFeedTrustPolicyV1,
    /// Exact next feed policy.
    pub next_feed_policy: HedgingFeedTrustPolicyV1,
    /// Deterministic transition identity.
    pub transition_id: [u8; 32],
    /// Exact previous-policy transition authority.
    pub authority_id: String,
    /// Strong Ed25519 authorization.
    pub signature: [u8; 64],
    /// Bounded consensus inclusion/finality proof for the compacted frontier.
    pub consensus_authentication_proof: Vec<u8>,
}
impl HedgingBillingEpochTransitionV1 {
    /// Derive the exact transition identity/signing digest.
    ///
    /// # Errors
    ///
    /// Fails when canonical encoding fails.
    pub fn transition_digest(&self) -> Result<[u8; 32], HedgingBillingServiceError> {
        let mut canonical = self.clone();
        canonical.transition_id = [0; 32];
        canonical.signature = [0; 64];
        hash_canonical(EPOCH_TRANSITION_DOMAIN_V1, &canonical)
    }
    /// Verify the policy chain, compaction bindings, and Ed25519 authorization.
    ///
    /// The opaque consensus proof is only shape-checked here. Callers must also
    /// authenticate it through [`HedgingBillingJournalVerifier::verify_epoch_transition`].
    ///
    /// # Errors
    ///
    /// Rejects skipped or substituted policy/epoch predecessors, malformed
    /// archive commitments, invalid authority material, and forged signatures.
    pub fn verify(&self) -> Result<(), HedgingBillingServiceError> {
        self.previous_service_policy.validate()?;
        self.next_service_policy.validate()?;
        self.previous_feed_policy.validate()?;
        self.next_feed_policy.validate()?;
        let previous_policy_digest = self.previous_service_policy.digest()?;
        let next_policy_digest = self.next_service_policy.digest()?;
        if self.version != HEDGING_BILLING_EPOCH_TRANSITION_VERSION_V1
            || self.epoch_sequence == 0
            || self.predecessor_epoch_sequence.checked_add(1) != Some(self.epoch_sequence)
            || (self.predecessor_epoch_sequence == 0)
                != self.predecessor_transition_digest.is_none()
            || self
                .predecessor_transition_digest
                .is_some_and(|digest| digest == [0; 32])
            || self.predecessor_epoch_sequence.checked_add(1)
                != Some(self.previous_service_policy.revision)
            || self.epoch_sequence.checked_add(1) != Some(self.next_service_policy.revision)
            || self.next_service_policy.revision
                != self
                    .previous_service_policy
                    .revision
                    .checked_add(1)
                    .ok_or(HedgingBillingServiceError::InvalidEpochTransition)?
            || self.next_service_policy.predecessor_policy_digest != Some(previous_policy_digest)
            || self.previous_service_policy.network_id != self.next_service_policy.network_id
            || self.previous_service_policy.billing_epoch_unix
                != self.next_service_policy.billing_epoch_unix
            || self.previous_service_policy.billing_period_secs
                != self.next_service_policy.billing_period_secs
            || self.previous_service_policy.epoch_witness_store_handle
                != self.next_service_policy.epoch_witness_store_handle
            || self.previous_feed_policy.canonical_digest()?
                != self.previous_service_policy.feed_trust_policy_digest
            || self.next_feed_policy.canonical_digest()?
                != self.next_service_policy.feed_trust_policy_digest
            || self.predecessor_journal_commitment.journal_next_sequence
                != self.next_source_sequence
            || self.compacted_through_period_end_unix
                <= self.previous_service_policy.billing_epoch_unix
            || !(self.compacted_through_period_end_unix
                - self.previous_service_policy.billing_epoch_unix)
                .is_multiple_of(self.previous_service_policy.billing_period_secs)
            || self.compacted_counts.source_pages == 0
            || self.compacted_counts.period_closes == 0
            || self.compacted_counts.statements != self.compacted_counts.acknowledgements
            || self.compacted_source_digest == [0; 32]
            || self.compacted_economic_state_digest == [0; 32]
            || self.compacted_account_bases_digest == [0; 32]
            || self.retained_epoch_state_digest == [0; 32]
            || self.consensus_authentication_proof.is_empty()
            || self.consensus_authentication_proof.len()
                > HEDGING_BILLING_CONSENSUS_PROOF_MAX_BYTES_V1
            || self.authority_id
                != self
                    .previous_service_policy
                    .transition_authority
                    .authority_id
            || self.transition_id != self.transition_digest()?
            || next_policy_digest == previous_policy_digest
        {
            return Err(HedgingBillingServiceError::InvalidEpochTransition);
        }
        self.predecessor_journal_commitment
            .validate(self.previous_service_policy.network_id)?;
        let key =
            checked_verifying_key(self.previous_service_policy.transition_authority.public_key)
                .map_err(|_| HedgingBillingServiceError::InvalidEpochTransition)?;
        key.verify_strict(
            &epoch_transition_signature_digest(self.transition_id),
            &Signature::from_bytes(&self.signature),
        )
        .map_err(|_| HedgingBillingServiceError::InvalidEpochTransition)
    }
}
/// Sealed immutable recovery record for one authenticated billing epoch.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgingBillingEpochWitnessRecordV1 {
    /// Schema version.
    pub version: u8,
    /// Exact genesis-derived network owning this recovery record.
    pub network_id: NetworkId,
    /// Exact monotonic epoch sequence.
    pub epoch_sequence: u64,
    /// Signed transition identity.
    pub transition_id: [u8; 32],
    /// Digest of exact canonical checkpoint bytes.
    pub checkpoint_digest: [u8; 32],
    /// Exact canonical post-transition checkpoint bytes.
    pub checkpoint_bytes: Vec<u8>,
    /// Deterministic CAS revision.
    pub revision: [u8; 32],
}
impl HedgingBillingEpochWitnessRecordV1 {
    fn new(
        network_id: NetworkId,
        epoch_sequence: u64,
        transition_id: [u8; 32],
        checkpoint_bytes: Vec<u8>,
    ) -> Self {
        let checkpoint_digest = *blake3::hash(&checkpoint_bytes).as_bytes();
        let mut record = Self {
            version: HEDGING_BILLING_EPOCH_WITNESS_RECORD_VERSION_V1,
            network_id,
            epoch_sequence,
            transition_id,
            checkpoint_digest,
            checkpoint_bytes,
            revision: [0; 32],
        };
        record.revision = epoch_witness_record_revision(&record);
        record
    }
    /// Validate schema, checkpoint bounds, digest binding, and CAS revision.
    ///
    /// # Errors
    ///
    /// Rejects malformed, oversized, tampered, or revision-substituted records.
    pub fn validate(&self, max_bytes: u64) -> Result<(), HedgingBillingServiceError> {
        if self.version != HEDGING_BILLING_EPOCH_WITNESS_RECORD_VERSION_V1
            || max_bytes == 0
            || max_bytes > HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1
            || self.epoch_sequence == 0
            || self.transition_id == [0; 32]
            || self.checkpoint_bytes.is_empty()
            || u64::try_from(self.checkpoint_bytes.len()).unwrap_or(u64::MAX) > max_bytes
            || self.checkpoint_digest != *blake3::hash(&self.checkpoint_bytes).as_bytes()
            || self.revision != epoch_witness_record_revision(self)
        {
            return Err(HedgingBillingServiceError::InvalidEpochWitness);
        }
        Ok(())
    }
    /// Encode the one canonical Norito record accepted by sealed stores.
    ///
    /// # Errors
    ///
    /// Rejects invalid records and bounded wrapper overflow.
    pub fn to_canonical_bytes(
        &self,
        checkpoint_max_bytes: u64,
    ) -> Result<Vec<u8>, HedgingBillingServiceError> {
        self.validate(checkpoint_max_bytes)?;
        let max_record_bytes = epoch_witness_record_max_bytes(checkpoint_max_bytes)?;
        let bytes =
            norito::to_bytes(self).map_err(|_| HedgingBillingServiceError::InvalidEpochWitness)?;
        if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_record_bytes {
            return Err(HedgingBillingServiceError::InvalidEpochWitness);
        }
        Ok(bytes)
    }
    /// Decode one exact canonical Norito record returned by a sealed store.
    ///
    /// # Errors
    ///
    /// Rejects malformed, oversized, noncanonical, tampered, or
    /// revision-substituted records before recovery uses their checkpoint.
    pub fn from_canonical_bytes(
        bytes: &[u8],
        checkpoint_max_bytes: u64,
    ) -> Result<Self, HedgingBillingServiceError> {
        let max_record_bytes = epoch_witness_record_max_bytes(checkpoint_max_bytes)?;
        if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_record_bytes {
            return Err(HedgingBillingServiceError::InvalidEpochWitness);
        }
        let max_record_bytes = usize::try_from(max_record_bytes)
            .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?;
        let record = decode_from_bytes_with_limits::<Self>(
            bytes,
            DecodeLimits::new(
                max_record_bytes,
                max_record_bytes,
                max_record_bytes.saturating_mul(CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT),
                max_record_bytes
                    .saturating_mul(CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT)
                    .saturating_add(CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES),
                CHECKPOINT_MAX_NESTING_DEPTH,
            ),
        )
        .map_err(|_| HedgingBillingServiceError::InvalidEpochWitness)?;
        if record.to_canonical_bytes(checkpoint_max_bytes)?.as_slice() != bytes {
            return Err(HedgingBillingServiceError::InvalidEpochWitness);
        }
        Ok(record)
    }
}
/// Runtime-only sealed, monotonic, immutable billing epoch witness archive.
///
/// Implementations must authenticate records at rest, preserve every epoch for audit export,
/// persist records only through [`HedgingBillingEpochWitnessRecordV1::to_canonical_bytes`] and
/// [`HedgingBillingEpochWitnessRecordV1::from_canonical_bytes`], and make `compare_and_swap_latest`
/// linearizable. Revisions and epochs may never roll back or fork.
pub trait HedgingBillingEpochWitnessStore: HedgingBillingRuntimeProviderV1 {
    /// Authenticate sealed-store readiness without reading witness bytes.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free provider failure.
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError>;
    /// Load the latest authenticated epoch record.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free failure.
    fn load_latest(
        &self,
    ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError>;
    /// Load one immutable historical audit witness.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free failure.
    fn load_epoch(
        &self,
        epoch_sequence: u64,
    ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError>;
    /// Atomically append `next` if the latest exact revision is unchanged.
    ///
    /// A successful return must preserve the record immutably for both lookup
    /// methods. Uncertain writes must return `Ambiguous`.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free failure.
    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &HedgingBillingEpochWitnessRecordV1,
    ) -> Result<(), HedgingBillingExternalError>;
}
/// Exact consensus-authenticated close of one billing period.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgingBillingFinalizedPeriodCloseV1 {
    /// Schema version.
    pub version: u8,
    /// Exact active chain.
    pub network_id: NetworkId,
    /// Exclusive period boundary being closed.
    pub period_end_unix: u64,
    /// Consensus journal prefix that is complete for this close.
    pub journal_commitment: HedgingBillingJournalCommitmentV1,
    /// Reviewed billing policy digest.
    pub billing_policy_digest: [u8; 32],
    /// Digest of the complete deterministic service policy.
    pub service_policy_digest: [u8; 32],
    /// Exact feed-policy digest used by the governed reference decision.
    pub feed_trust_policy_digest: [u8; 32],
    /// Consensus-authenticated feed admission time.
    pub feed_admitted_at_unix: u64,
    /// Exact governed reference-price decision committed by the close.
    pub governed_reference_price: GovernedHedgingReferencePriceDecisionV1,
    /// Bounded consensus proof authenticating the close record.
    pub authentication_proof: Vec<u8>,
}
impl HedgingBillingFinalizedPeriodCloseV1 {
    fn validate(
        &self,
        policy: &HedgingBillingServicePolicyV1,
        feed_policy: &HedgingFeedTrustPolicyV1,
    ) -> Result<(), HedgingBillingServiceError> {
        self.journal_commitment.validate(policy.network_id)?;
        if self.version != HEDGING_BILLING_PERIOD_CLOSE_VERSION_V1
            || self.network_id != policy.network_id
            || self.period_end_unix <= policy.billing_epoch_unix
            || !(self.period_end_unix - policy.billing_epoch_unix)
                .is_multiple_of(policy.billing_period_secs)
            || self.journal_commitment.finalized_cursor.finalized_at_unix < self.period_end_unix
            || self.billing_policy_digest != policy.billing_policy_digest
            || self.service_policy_digest != policy.digest()?
            || self.feed_trust_policy_digest != policy.feed_trust_policy_digest
            || self.feed_admitted_at_unix
                != self.journal_commitment.finalized_cursor.finalized_at_unix
            || self.governed_reference_price.decision.effective_at_unix != self.period_end_unix
            || self.governed_reference_price.decision.max_feed_age_secs != policy.max_feed_age_secs
            || self.governed_reference_price.decision.max_divergence_bps
                != policy.max_divergence_bps
            || self.authentication_proof.is_empty()
            || self.authentication_proof.len() > HEDGING_BILLING_CONSENSUS_PROOF_MAX_BYTES_V1
        {
            return Err(HedgingBillingServiceError::InvalidPeriodClose);
        }
        if feed_policy.canonical_digest()? != self.feed_trust_policy_digest {
            return Err(HedgingBillingServiceError::FeedPolicyMismatch);
        }
        self.governed_reference_price
            .verify(feed_policy, self.feed_admitted_at_unix)?;
        Ok(())
    }
    /// Derive the exact close-record digest, excluding transport proof bytes.
    ///
    /// # Errors
    ///
    /// Fails when the close is invalid or cannot be encoded canonically.
    pub fn close_digest(
        &self,
        policy: &HedgingBillingServicePolicyV1,
        feed_policy: &HedgingFeedTrustPolicyV1,
    ) -> Result<[u8; 32], HedgingBillingServiceError> {
        self.validate(policy, feed_policy)?;
        hash_canonical(
            PERIOD_CLOSE_DOMAIN_V1,
            &HedgingBillingPeriodClosePreimageV1 {
                version: self.version,
                network_id: self.network_id,
                period_end_unix: self.period_end_unix,
                journal_commitment: self.journal_commitment,
                billing_policy_digest: self.billing_policy_digest,
                service_policy_digest: self.service_policy_digest,
                feed_trust_policy_digest: self.feed_trust_policy_digest,
                feed_admitted_at_unix: self.feed_admitted_at_unix,
                governed_reference_price: self.governed_reference_price.clone(),
            },
        )
    }
}
#[derive(Debug, Clone, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
struct HedgingBillingPeriodClosePreimageV1 {
    version: u8,
    network_id: NetworkId,
    period_end_unix: u64,
    journal_commitment: HedgingBillingJournalCommitmentV1,
    billing_policy_digest: [u8; 32],
    service_policy_digest: [u8; 32],
    feed_trust_policy_digest: [u8; 32],
    feed_admitted_at_unix: u64,
    governed_reference_price: GovernedHedgingReferencePriceDecisionV1,
}
/// Runtime signer identity rechecked around every HSM operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingStatementSignerIdentityV1 {
    /// Stable opaque HSM/KMS provider handle.
    pub provider_handle: String,
    /// Stable signer identifier.
    pub signer_id: String,
    /// Exact strong Ed25519 public key.
    pub public_key: [u8; 32],
}
/// Fixed, payload-free failure classes returned by external dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HedgingBillingExternalError {
    /// External provider is temporarily unavailable.
    ///
    /// This class is valid only when the adapter can prove no external write
    /// occurred. Every uncertain result must be reported as [`Self::Ambiguous`].
    #[error("external provider unavailable")]
    Unavailable,
    /// External provider rejected the request.
    #[error("external provider rejected request")]
    Rejected,
    /// External result may have committed and requires reconciliation.
    #[error("external result is ambiguous")]
    Ambiguous,
}
/// Runtime-only HSM/KMS statement signer.
pub trait BillingStatementRuntimeSigner: HedgingBillingRuntimeProviderV1 {
    /// Return the current public signer identity.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class without provider diagnostics.
    fn identity(&self) -> Result<BillingStatementSignerIdentityV1, HedgingBillingExternalError>;
    /// Authenticate HSM/KMS readiness without signing payload material.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free provider failure.
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError>;
    /// Sign one exact domain-separated digest.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class without key material or provider diagnostics.
    fn sign_digest(&self, digest: [u8; 32]) -> Result<[u8; 64], HedgingBillingExternalError>;
}
/// Signed, governed billing statement returned by a runtime HSM/KMS provider.
#[derive(
    Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct SignedGovernedBillingStatementV1 {
    /// Schema version.
    pub version: u8,
    /// Exact active chain.
    pub network_id: NetworkId,
    /// Reviewed billing policy digest.
    pub billing_policy_digest: [u8; 32],
    /// Digest of the complete deterministic service policy.
    pub service_policy_digest: [u8; 32],
    /// Governed signer identifier.
    pub signer_id: String,
    /// Unix second bound into the signature.
    pub signed_at_unix: u64,
    /// Finalized block identity at signing.
    pub finalized_cursor: HedgingBillingFinalizedCursorV1,
    /// Exact consensus-authenticated period-close digest.
    pub period_close_digest: [u8; 32],
    /// Consensus-authenticated feed admission time retained for replay.
    pub feed_admitted_at_unix: u64,
    /// Exact governed statement.
    pub governed_statement: GovernedBillingStatementV1,
    /// Strong Ed25519 signature over [`Self::signing_digest`].
    pub signature: [u8; 64],
}
impl SignedGovernedBillingStatementV1 {
    /// Derive the exact statement-signing digest.
    ///
    /// # Errors
    ///
    /// Fails when the envelope is structurally invalid or cannot be encoded.
    pub fn signing_digest(&self) -> Result<[u8; 32], HedgingBillingServiceError> {
        let preimage = BillingStatementSignaturePreimageV1 {
            version: self.version,
            network_id: self.network_id,
            billing_policy_digest: self.billing_policy_digest,
            service_policy_digest: self.service_policy_digest,
            signer_id: self.signer_id.clone(),
            signed_at_unix: self.signed_at_unix,
            finalized_cursor: self.finalized_cursor,
            period_close_digest: self.period_close_digest,
            feed_admitted_at_unix: self.feed_admitted_at_unix,
            governed_statement: self.governed_statement.clone(),
        };
        hash_canonical(STATEMENT_SIGNATURE_DOMAIN_V1, &preimage)
    }
    /// Verify policy binding, statement structure, signer activation, and signature.
    ///
    /// # Errors
    ///
    /// Returns a fixed service error for malformed or untrusted material.
    pub fn verify(
        &self,
        policy: &HedgingBillingServicePolicyV1,
        feed_policy: &HedgingFeedTrustPolicyV1,
        period_close: &HedgingBillingFinalizedPeriodCloseV1,
    ) -> Result<(), HedgingBillingServiceError> {
        policy.validate()?;
        period_close.validate(policy, feed_policy)?;
        if self.version != SIGNED_GOVERNED_BILLING_STATEMENT_VERSION_V1
            || self.network_id != policy.network_id
            || self.billing_policy_digest != policy.billing_policy_digest
            || self.service_policy_digest != policy.digest()?
            || feed_policy.canonical_digest()? != policy.feed_trust_policy_digest
            || self.signer_id != policy.statement_signer.signer_id
            || self.signed_at_unix
                != period_close
                    .journal_commitment
                    .finalized_cursor
                    .finalized_at_unix
            || self.finalized_cursor != period_close.journal_commitment.finalized_cursor
            || self.period_close_digest != period_close.close_digest(policy, feed_policy)?
            || self.feed_admitted_at_unix != period_close.feed_admitted_at_unix
            || self.governed_statement.governed_reference_price
                != period_close.governed_reference_price
            || !policy
                .statement_signer
                .active_at(self.finalized_cursor.height)
        {
            return Err(HedgingBillingServiceError::InvalidSignedStatement);
        }
        self.finalized_cursor.validate()?;
        self.governed_statement
            .verify(feed_policy, self.feed_admitted_at_unix)?;
        self.governed_statement
            .canonical_bytes()
            .map_err(HedgingBillingServiceError::from)?;
        let key = checked_verifying_key(policy.statement_signer.public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidSignedStatement)?;
        let signature = Signature::from_bytes(&self.signature);
        key.verify_strict(&self.signing_digest()?, &signature)
            .map_err(|_| HedgingBillingServiceError::InvalidSignedStatement)?;
        let bytes = norito::to_bytes(self)
            .map_err(|_| HedgingBillingServiceError::InvalidSignedStatement)?;
        if bytes.len() > SIGNED_GOVERNED_BILLING_STATEMENT_MAX_BYTES_V1 {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        Ok(())
    }
    /// Return bounded canonical bytes.
    ///
    /// # Errors
    ///
    /// Fails when verification or encoding fails.
    pub fn canonical_bytes(
        &self,
        policy: &HedgingBillingServicePolicyV1,
        feed_policy: &HedgingFeedTrustPolicyV1,
        period_close: &HedgingBillingFinalizedPeriodCloseV1,
    ) -> Result<Vec<u8>, HedgingBillingServiceError> {
        self.verify(policy, feed_policy, period_close)?;
        let bytes =
            norito::to_bytes(self).map_err(|_| HedgingBillingServiceError::EncodingFailed)?;
        if bytes.len() > SIGNED_GOVERNED_BILLING_STATEMENT_MAX_BYTES_V1 {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        Ok(bytes)
    }
}
#[derive(Debug, Clone, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
struct BillingStatementSignaturePreimageV1 {
    version: u8,
    network_id: NetworkId,
    billing_policy_digest: [u8; 32],
    service_policy_digest: [u8; 32],
    signer_id: String,
    signed_at_unix: u64,
    finalized_cursor: HedgingBillingFinalizedCursorV1,
    period_close_digest: [u8; 32],
    feed_admitted_at_unix: u64,
    governed_statement: GovernedBillingStatementV1,
}
/// Durable receipt returned by an authenticated statement publication sink.
#[derive(
    Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct BillingStatementPublicationReceiptV1 {
    /// Schema version.
    pub version: u8,
    /// Published statement identifier.
    pub statement_id: [u8; 32],
    /// Digest of the exact signed statement bytes.
    pub signed_statement_digest: [u8; 32],
    /// Stable publication route identifier.
    pub route_id: String,
    /// Stable authenticated publisher identifier.
    pub publisher_id: String,
    /// Publication timestamp in Unix seconds.
    pub published_at_unix: u64,
    /// Sink-owned immutable receipt digest.
    pub receipt_digest: [u8; 32],
    /// Strong Ed25519 signature over `receipt_digest`.
    pub signature: [u8; 64],
}
impl BillingStatementPublicationReceiptV1 {
    fn validate(
        &self,
        signed: &SignedGovernedBillingStatementV1,
        policy: &BillingStatementPublisherPolicyV1,
    ) -> Result<(), HedgingBillingServiceError> {
        policy.validate()?;
        validate_identifier(
            &self.route_id,
            BILLING_PUBLICATION_ROUTE_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidPublicationReceipt,
        )?;
        if self.version != BILLING_STATEMENT_PUBLICATION_RECEIPT_VERSION_V1
            || self.statement_id != signed.governed_statement.statement.statement_id
            || self.publisher_id != policy.publisher_id
            || self.route_id != policy.route_id
            || self.published_at_unix < signed.signed_at_unix
            || self.published_at_unix == u64::MAX
            || self.receipt_digest == [0; 32]
        {
            return Err(HedgingBillingServiceError::InvalidPublicationReceipt);
        }
        let signed_bytes =
            norito::to_bytes(signed).map_err(|_| HedgingBillingServiceError::EncodingFailed)?;
        if self.signed_statement_digest != *blake3::hash(&signed_bytes).as_bytes() {
            return Err(HedgingBillingServiceError::InvalidPublicationReceipt);
        }
        let expected = publication_receipt_digest(self)?;
        if expected != self.receipt_digest {
            return Err(HedgingBillingServiceError::InvalidPublicationReceipt);
        }
        let key = checked_verifying_key(policy.public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidPublicationReceipt)?;
        let signature = Signature::from_bytes(&self.signature);
        let mut message = Vec::with_capacity(
            PUBLISHER_RECEIPT_SIGNATURE_DOMAIN_V1.len() + self.receipt_digest.len(),
        );
        message.extend_from_slice(PUBLISHER_RECEIPT_SIGNATURE_DOMAIN_V1);
        message.extend_from_slice(&self.receipt_digest);
        key.verify_strict(&message, &signature)
            .map_err(|_| HedgingBillingServiceError::InvalidPublicationReceipt)?;
        Ok(())
    }
}
/// Runtime identity for the authoritative publication service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingStatementPublisherIdentityV1 {
    /// Stable opaque immutable-publisher provider handle.
    pub provider_handle: String,
    /// Stable publisher identifier.
    pub publisher_id: String,
    /// Stable publication route.
    pub route_id: String,
    /// Exact receipt-verification key.
    pub public_key: [u8; 32],
}
/// Exact immutable statement and signed receipt returned by publisher lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingStatementAuthoritativePublicationV1 {
    /// Exact signed statement bytes retained by the publication service.
    pub signed_statement: SignedGovernedBillingStatementV1,
    /// Publisher-signed receipt for those exact bytes.
    pub receipt: BillingStatementPublicationReceiptV1,
}
/// Authenticated immutable statement publication and lookup adapter.
pub trait BillingStatementPublisher: HedgingBillingRuntimeProviderV1 {
    /// Return the currently active publisher identity.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class without credentials or provider payloads.
    fn identity(&self) -> Result<BillingStatementPublisherIdentityV1, HedgingBillingExternalError>;
    /// Authenticate immutable-publisher readiness without writing.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free provider failure.
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError>;
    /// Publish exact signed statement bytes.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class. `Unavailable` guarantees that no sink write occurred; every
    /// uncertain result must be `Ambiguous` and requires lookup before retry. A successful return
    /// guarantees that immediate and future `lookup(idempotency_key)` calls return the immutable
    /// exact signed statement and receipt.
    fn publish(
        &self,
        idempotency_key: [u8; 32],
        signed_statement_digest: [u8; 32],
        statement: &SignedGovernedBillingStatementV1,
    ) -> Result<BillingStatementPublicationReceiptV1, HedgingBillingExternalError>;
    /// Look up an immutable publication by statement identity.
    ///
    /// The returned record must include the exact signed statement, allowing restart reconciliation
    /// even when a rolled-back local checkpoint omitted its signed envelope. A publisher must
    /// reject statement-ID collisions and never replace a previously returned record.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class without sink diagnostics.
    fn lookup(
        &self,
        statement_id: [u8; 32],
    ) -> Result<Option<BillingStatementAuthoritativePublicationV1>, HedgingBillingExternalError>;
}
/// Durable authenticated account acknowledgement.
#[derive(Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct BillingStatementAcknowledgementV1 {
    /// Schema version.
    pub version: u8,
    /// Exact genesis-derived network owning the acknowledgement.
    pub network_id: NetworkId,
    /// Acknowledged statement identifier.
    pub statement_id: [u8; 32],
    /// Digest of the exact account bytes.
    pub account_digest: [u8; 32],
    /// Canonical request authentication/idempotency binding.
    pub request_binding_digest: [u8; 32],
    /// Acknowledgement timestamp in Unix seconds.
    pub acknowledged_at_unix: u64,
    /// Bounded authentication proof issued by the account authority.
    pub authentication_proof: Vec<u8>,
    /// Deterministic acknowledgement identity.
    pub acknowledgement_id: [u8; 32],
}
impl fmt::Debug for BillingStatementAcknowledgementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BillingStatementAcknowledgementV1")
            .field("version", &self.version)
            .field("network_id", &self.network_id)
            .field("statement_id", &self.statement_id)
            .field("account_digest", &self.account_digest)
            .field("request_binding_digest", &self.request_binding_digest)
            .field("acknowledged_at_unix", &self.acknowledged_at_unix)
            .field("authentication_proof", &"[REDACTED]")
            .field("acknowledgement_id", &self.acknowledgement_id)
            .finish()
    }
}
/// Runtime identity of the authoritative acknowledgement service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingStatementAcknowledgementAuthorityIdentityV1 {
    /// Stable opaque provider handle.
    pub provider_handle: String,
}
/// Authoritative verifier and lookup service for account acknowledgements.
pub trait BillingStatementAcknowledgementAuthority: HedgingBillingRuntimeProviderV1 {
    /// Return the current acknowledgement-authority identity.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free provider failure.
    fn identity(
        &self,
    ) -> Result<BillingStatementAcknowledgementAuthorityIdentityV1, HedgingBillingExternalError>;
    /// Authenticate acknowledgement-authority readiness without reading account material.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free provider failure.
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError>;
    /// Authenticate an acknowledgement against the exact published statement.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class without account credentials or proof bytes.
    fn verify(
        &self,
        statement: &SignedGovernedBillingStatementV1,
        acknowledgement: &BillingStatementAcknowledgementV1,
    ) -> Result<(), HedgingBillingExternalError>;
    /// Idempotently record one authenticated acknowledgement.
    ///
    /// The authority must key the operation by `acknowledgement_id`, reject a
    /// conflicting statement acknowledgement, and return exact immutable bytes.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class; uncertain writes must return
    /// [`HedgingBillingExternalError::Ambiguous`].
    fn record(
        &self,
        statement: &SignedGovernedBillingStatementV1,
        acknowledgement: &BillingStatementAcknowledgementV1,
    ) -> Result<BillingStatementAcknowledgementV1, HedgingBillingExternalError>;
    /// Look up the authoritative acknowledgement for one statement.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class without authority diagnostics.
    fn lookup(
        &self,
        statement_id: [u8; 32],
    ) -> Result<Option<BillingStatementAcknowledgementV1>, HedgingBillingExternalError>;
}
/// Provider wrapper that pins one production identity and public policy.
///
/// The wrapper revalidates the exact handle, revision, and policy digest immediately before and
/// after every external operation. Read-only results observed across drift are discarded as
/// unavailable. Results from operations that may have committed externally are classified as
/// ambiguous so callers reconcile through their existing immutable lookup protocol.
pub struct QualifiedHedgingBillingRuntimeProviderV1<P: HedgingBillingRuntimeProviderV1 + ?Sized> {
    handle: String,
    qualification: HedgingBillingRuntimeProviderQualificationV1,
    provider: Arc<P>,
}
impl<P: HedgingBillingRuntimeProviderV1 + ?Sized> QualifiedHedgingBillingRuntimeProviderV1<P> {
    /// Qualify and pin one deployment-owned provider before durable state opens.
    ///
    /// # Errors
    ///
    /// Fails for an invalid, unavailable, stale, substituted, or mismatched provider binding.
    pub fn try_new(
        expected_handle: &str,
        expected_qualification: HedgingBillingRuntimeProviderQualificationV1,
        provider: Arc<P>,
    ) -> Result<Self, HedgingBillingRuntimeProviderQualificationErrorV1> {
        qualify_hedging_billing_runtime_provider_v1(
            expected_handle,
            expected_qualification,
            provider.as_ref(),
        )?;
        Ok(Self {
            handle: expected_handle.to_owned(),
            qualification: expected_qualification,
            provider,
        })
    }
    fn revalidate(&self) -> Result<(), HedgingBillingRuntimeProviderQualificationErrorV1> {
        revalidate_hedging_billing_runtime_provider_v1(
            &self.handle,
            self.qualification,
            self.provider.as_ref(),
        )
    }
    fn read<T>(
        &self,
        operation: impl FnOnce(&P) -> Result<T, HedgingBillingExternalError>,
    ) -> Result<T, HedgingBillingExternalError> {
        self.revalidate()
            .map_err(|_| HedgingBillingExternalError::Unavailable)?;
        let result = operation(self.provider.as_ref());
        self.revalidate()
            .map_err(|_| HedgingBillingExternalError::Unavailable)?;
        result
    }
    fn write<T>(
        &self,
        operation: impl FnOnce(&P) -> Result<T, HedgingBillingExternalError>,
    ) -> Result<T, HedgingBillingExternalError> {
        self.revalidate()
            .map_err(|_| HedgingBillingExternalError::Unavailable)?;
        let result = operation(self.provider.as_ref());
        self.revalidate()
            .map_err(|_| HedgingBillingExternalError::Ambiguous)?;
        result
    }
}
impl<P: HedgingBillingRuntimeProviderV1 + ?Sized> fmt::Debug
    for QualifiedHedgingBillingRuntimeProviderV1<P>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedHedgingBillingRuntimeProviderV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("provider", &"[RUNTIME-ONLY]")
            .finish()
    }
}
impl<P: HedgingBillingRuntimeProviderV1 + ?Sized> HedgingBillingRuntimeProviderV1
    for QualifiedHedgingBillingRuntimeProviderV1<P>
{
    fn handle(&self) -> &str {
        &self.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        HedgingBillingRuntimeProviderQualificationV1,
        HedgingBillingRuntimeProviderReadinessErrorV1,
    > {
        self.revalidate().map_err(|error| match error {
            HedgingBillingRuntimeProviderQualificationErrorV1::UnavailableOrStale => {
                HedgingBillingRuntimeProviderReadinessErrorV1::Unavailable
            }
            _ => HedgingBillingRuntimeProviderReadinessErrorV1::Rejected,
        })?;
        Ok(self.qualification)
    }
}
impl<P: HedgingBillingFinalizedQuery + ?Sized> HedgingBillingFinalizedQuery
    for QualifiedHedgingBillingRuntimeProviderV1<P>
{
    fn identity(
        &self,
    ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError> {
        self.read(HedgingBillingFinalizedQuery::identity)
    }
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
        self.read(HedgingBillingFinalizedQuery::check_readiness)
    }
    fn supplies_period_closes(&self) -> bool {
        if self.revalidate().is_err() {
            return false;
        }
        let supplies_period_closes =
            HedgingBillingFinalizedQuery::supplies_period_closes(self.provider.as_ref());
        if self.revalidate().is_err() {
            return false;
        }
        supplies_period_closes
    }
    fn finalized_head(
        &self,
    ) -> Result<HedgingBillingFinalizedCursorV1, HedgingBillingExternalError> {
        self.read(HedgingBillingFinalizedQuery::finalized_head)
    }
    fn query_finalized_page(
        &self,
        position: HedgingBillingQueryPositionV1,
        max_events: u32,
    ) -> Result<Option<HedgingBillingFinalizedEventPageV1>, HedgingBillingExternalError> {
        self.read(|provider| provider.query_finalized_page(position, max_events))
    }
    fn query_finalized_period_close(
        &self,
        period_end_unix: u64,
        position: HedgingBillingQueryPositionV1,
    ) -> Result<Option<HedgingBillingFinalizedPeriodCloseV1>, HedgingBillingExternalError> {
        self.read(|provider| provider.query_finalized_period_close(period_end_unix, position))
    }
}
impl<P: HedgingBillingJournalVerifier + ?Sized> HedgingBillingJournalVerifier
    for QualifiedHedgingBillingRuntimeProviderV1<P>
{
    fn identity(
        &self,
    ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError> {
        self.read(HedgingBillingJournalVerifier::identity)
    }
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
        self.read(HedgingBillingJournalVerifier::check_readiness)
    }
    fn verify_page(
        &self,
        network_id: &NetworkId,
        previous: Option<HedgingBillingJournalCommitmentV1>,
        page: &HedgingBillingFinalizedEventPageV1,
    ) -> Result<(), HedgingBillingExternalError> {
        self.read(|provider| provider.verify_page(network_id, previous, page))
    }
    fn verify_period_close(
        &self,
        network_id: &NetworkId,
        close: &HedgingBillingFinalizedPeriodCloseV1,
    ) -> Result<(), HedgingBillingExternalError> {
        self.read(|provider| provider.verify_period_close(network_id, close))
    }
    fn verify_epoch_transition(
        &self,
        network_id: &NetworkId,
        transition: &HedgingBillingEpochTransitionV1,
    ) -> Result<(), HedgingBillingExternalError> {
        self.read(|provider| provider.verify_epoch_transition(network_id, transition))
    }
}
impl<P: BillingStatementRuntimeSigner + ?Sized> BillingStatementRuntimeSigner
    for QualifiedHedgingBillingRuntimeProviderV1<P>
{
    fn identity(&self) -> Result<BillingStatementSignerIdentityV1, HedgingBillingExternalError> {
        self.read(BillingStatementRuntimeSigner::identity)
    }
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
        self.read(BillingStatementRuntimeSigner::check_readiness)
    }
    fn sign_digest(&self, digest: [u8; 32]) -> Result<[u8; 64], HedgingBillingExternalError> {
        self.read(|provider| provider.sign_digest(digest))
    }
}
impl<P: BillingStatementPublisher + ?Sized> BillingStatementPublisher
    for QualifiedHedgingBillingRuntimeProviderV1<P>
{
    fn identity(&self) -> Result<BillingStatementPublisherIdentityV1, HedgingBillingExternalError> {
        self.read(BillingStatementPublisher::identity)
    }
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
        self.read(BillingStatementPublisher::check_readiness)
    }
    fn publish(
        &self,
        idempotency_key: [u8; 32],
        signed_statement_digest: [u8; 32],
        statement: &SignedGovernedBillingStatementV1,
    ) -> Result<BillingStatementPublicationReceiptV1, HedgingBillingExternalError> {
        self.write(|provider| provider.publish(idempotency_key, signed_statement_digest, statement))
    }
    fn lookup(
        &self,
        statement_id: [u8; 32],
    ) -> Result<Option<BillingStatementAuthoritativePublicationV1>, HedgingBillingExternalError>
    {
        self.read(|provider| provider.lookup(statement_id))
    }
}
impl<P: BillingStatementAcknowledgementAuthority + ?Sized> BillingStatementAcknowledgementAuthority
    for QualifiedHedgingBillingRuntimeProviderV1<P>
{
    fn identity(
        &self,
    ) -> Result<BillingStatementAcknowledgementAuthorityIdentityV1, HedgingBillingExternalError>
    {
        self.read(BillingStatementAcknowledgementAuthority::identity)
    }
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
        self.read(BillingStatementAcknowledgementAuthority::check_readiness)
    }
    fn verify(
        &self,
        statement: &SignedGovernedBillingStatementV1,
        acknowledgement: &BillingStatementAcknowledgementV1,
    ) -> Result<(), HedgingBillingExternalError> {
        self.read(|provider| provider.verify(statement, acknowledgement))
    }
    fn record(
        &self,
        statement: &SignedGovernedBillingStatementV1,
        acknowledgement: &BillingStatementAcknowledgementV1,
    ) -> Result<BillingStatementAcknowledgementV1, HedgingBillingExternalError> {
        self.write(|provider| provider.record(statement, acknowledgement))
    }
    fn lookup(
        &self,
        statement_id: [u8; 32],
    ) -> Result<Option<BillingStatementAcknowledgementV1>, HedgingBillingExternalError> {
        self.read(|provider| provider.lookup(statement_id))
    }
}
impl<P: HedgingBillingEpochWitnessStore + ?Sized> HedgingBillingEpochWitnessStore
    for QualifiedHedgingBillingRuntimeProviderV1<P>
{
    fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
        self.read(HedgingBillingEpochWitnessStore::check_readiness)
    }
    fn load_latest(
        &self,
    ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError> {
        self.read(HedgingBillingEpochWitnessStore::load_latest)
    }
    fn load_epoch(
        &self,
        epoch_sequence: u64,
    ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError> {
        self.read(|provider| provider.load_epoch(epoch_sequence))
    }
    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &HedgingBillingEpochWitnessRecordV1,
    ) -> Result<(), HedgingBillingExternalError> {
        self.write(|provider| provider.compare_and_swap_latest(expected_revision, next))
    }
}
/// Direction of a generated hedge intent.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
#[norito(tag = "direction", content = "value", rename_all = "snake_case")]
pub enum HedgeIntentDirectionV1 {
    /// Sell XOR exposure against the governed reference quote.
    SellXor,
}
/// Whether a hedge projection may be submitted to an execution venue.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
#[norito(tag = "disposition", content = "value", rename_all = "snake_case")]
pub enum HedgeIntentDispositionV1 {
    /// Exposure is within the governed per-intent venue ceiling.
    Executable,
    /// Exposure exceeds the ceiling and requires a new governed operator plan.
    GovernedOverflow,
}
/// Deterministic intent for a later, separately governed execution adapter.
#[derive(
    Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct HedgeIntentV1 {
    /// Schema version.
    pub version: u8,
    /// Deterministic intent identifier.
    pub intent_id: [u8; 32],
    /// Exact active chain.
    pub network_id: NetworkId,
    /// Digest of the deterministic service policy.
    pub service_policy_digest: [u8; 32],
    /// Exact consensus-authenticated period-close digest.
    pub period_close_digest: [u8; 32],
    /// Statement period end.
    pub period_end_unix: u64,
    /// Finalized cursor covering every contributing accrual.
    pub finalized_cursor: HedgingBillingFinalizedCursorV1,
    /// Governed reference-price decision identifier.
    pub reference_price_decision_id: [u8; 32],
    /// Statement bundle digest.
    pub statement_bundle_digest: [u8; 32],
    /// Hedge direction.
    pub direction: HedgeIntentDirectionV1,
    /// Explicit execution eligibility.
    pub disposition: HedgeIntentDispositionV1,
    /// Exact XOR exposure.
    pub xor_amount: XorQuantity,
    /// Governed maximum slippage.
    pub max_slippage_bps: u16,
    /// Intent expiry in Unix seconds.
    pub expires_at_unix: u64,
    /// Automatic execution is always false for V1.
    pub automatic_execution: bool,
}
impl HedgeIntentV1 {
    /// Verify exact policy, amount, close, expiry, disposition, and identity binding.
    ///
    /// # Errors
    ///
    /// Rejects malformed, substituted, automatically executable, or
    /// incorrectly classified hedge projections.
    pub fn validate(
        &self,
        policy: &HedgingBillingServicePolicyV1,
    ) -> Result<(), HedgingBillingServiceError> {
        if self.version != HEDGE_INTENT_VERSION_V1
            || self.intent_id == [0; 32]
            || self.network_id != policy.network_id
            || self.service_policy_digest != policy.digest()?
            || self.period_close_digest == [0; 32]
            || self.period_end_unix == 0
            || self.reference_price_decision_id == [0; 32]
            || self.statement_bundle_digest == [0; 32]
            || self.xor_amount < policy.hedge_intent_threshold_xor
            || match self.disposition {
                HedgeIntentDispositionV1::Executable => {
                    self.xor_amount > policy.max_hedge_intent_xor
                }
                HedgeIntentDispositionV1::GovernedOverflow => {
                    self.xor_amount <= policy.max_hedge_intent_xor
                }
            }
            || self.max_slippage_bps != policy.hedge_max_slippage_bps
            || self.expires_at_unix
                != self
                    .period_end_unix
                    .checked_add(policy.hedge_intent_ttl_secs)
                    .ok_or(HedgingBillingServiceError::AmountOverflow)?
            || self.automatic_execution
        {
            return Err(HedgingBillingServiceError::InvalidHedgeIntent);
        }
        self.finalized_cursor.validate()?;
        let mut canonical = self.clone();
        canonical.intent_id = [0; 32];
        if self.intent_id != hash_canonical(HEDGE_INTENT_DOMAIN_V1, &canonical)? {
            return Err(HedgingBillingServiceError::InvalidHedgeIntent);
        }
        Ok(())
    }
}
/// Governed identities and limits for one explicitly authorized hedge venue.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct GovernedHedgeExecutionPolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Stable reviewed policy identity.
    pub policy_id: [u8; 32],
    /// Stable non-secret venue identifier.
    pub venue_id: String,
    /// Venue key authenticating immutable submission receipts.
    pub venue_public_key: [u8; 32],
    /// Stable operator authorization signer.
    pub operator_signer_id: String,
    /// Operator key authenticating explicit execution authorizations.
    pub operator_public_key: [u8; 32],
    /// First Unix second at which the policy is valid.
    pub valid_from_unix: u64,
    /// Exclusive Unix second at which the policy expires.
    pub valid_until_unix: u64,
    /// Maximum XOR amount admitted to one venue submission.
    pub max_submission_xor: XorQuantity,
    /// Maximum admitted slippage in basis points.
    pub max_slippage_bps: u16,
}
impl GovernedHedgeExecutionPolicyV1 {
    /// Validate identities, keys, time bounds, and venue limits.
    ///
    /// # Errors
    ///
    /// Rejects malformed or unsafe execution policy material.
    pub fn validate(&self) -> Result<(), HedgingBillingServiceError> {
        if self.version != HEDGE_EXECUTION_POLICY_VERSION_V1
            || self.policy_id == [0; 32]
            || self.valid_from_unix == 0
            || self.valid_until_unix <= self.valid_from_unix
            || self.valid_until_unix == u64::MAX
            || self.max_submission_xor.is_zero()
            || self.max_slippage_bps > 10_000
        {
            return Err(HedgingBillingServiceError::InvalidExecutionAdapter);
        }
        validate_identifier(
            &self.venue_id,
            BILLING_PUBLICATION_ROUTE_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidExecutionAdapter,
        )?;
        validate_identifier(
            &self.operator_signer_id,
            BILLING_SIGNER_ID_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidExecutionAdapter,
        )?;
        checked_verifying_key(self.venue_public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidExecutionAdapter)?;
        checked_verifying_key(self.operator_public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidExecutionAdapter)?;
        Ok(())
    }
    /// Return the canonical governed policy digest.
    ///
    /// # Errors
    ///
    /// Rejects invalid policy material or canonical encoding failure.
    pub fn canonical_digest(&self) -> Result<[u8; 32], HedgingBillingServiceError> {
        self.validate()?;
        hash_canonical(HEDGE_EXECUTION_POLICY_DOMAIN_V1, self)
    }
}
/// Explicit operator authorization binding one executable intent to one venue.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgeExecutionAuthorizationV1 {
    /// Schema version.
    pub version: u8,
    /// Deterministic authorization identity.
    pub authorization_id: [u8; 32],
    /// Exact governed execution-policy digest.
    pub execution_policy_digest: [u8; 32],
    /// Exact hedge intent.
    pub intent_id: [u8; 32],
    /// Exact billing-service policy digest.
    pub service_policy_digest: [u8; 32],
    /// Exact period-close digest.
    pub period_close_digest: [u8; 32],
    /// Exact governed venue.
    pub venue_id: String,
    /// Exact operator authorization signer.
    pub operator_signer_id: String,
    /// Exact authorized XOR amount.
    pub xor_amount: XorQuantity,
    /// Exact authorized slippage ceiling.
    pub max_slippage_bps: u16,
    /// Consensus or operator-controlled authorization time.
    pub authorized_at_unix: u64,
    /// Exclusive authorization expiry.
    pub expires_at_unix: u64,
    /// Monotonic operator-controlled release sequence.
    pub authorization_sequence: u64,
    /// Strong Ed25519 signature over the authorization identity.
    pub signature: [u8; 64],
}
impl HedgeExecutionAuthorizationV1 {
    /// Derive the exact authorization identity/signing digest.
    ///
    /// The stored identity and signature fields are zeroed for this digest.
    ///
    /// # Errors
    ///
    /// Fails when canonical encoding fails.
    pub fn authorization_digest(&self) -> Result<[u8; 32], HedgingBillingServiceError> {
        execution_authorization_digest(self)
    }
    /// Verify an explicit operator authorization against exact governed limits.
    ///
    /// # Errors
    ///
    /// Rejects overflow projections, substituted policy/venue/intent fields,
    /// invalid limits or time bounds, and forged operator signatures.
    pub fn verify(
        &self,
        billing_policy: &HedgingBillingServicePolicyV1,
        execution_policy: &GovernedHedgeExecutionPolicyV1,
        intent: &HedgeIntentV1,
    ) -> Result<(), HedgingBillingServiceError> {
        billing_policy.validate()?;
        execution_policy.validate()?;
        intent.validate(billing_policy)?;
        if self.version != HEDGE_EXECUTION_AUTHORIZATION_VERSION_V1
            || intent.disposition != HedgeIntentDispositionV1::Executable
            || self.execution_policy_digest != execution_policy.canonical_digest()?
            || self.intent_id != intent.intent_id
            || self.service_policy_digest != intent.service_policy_digest
            || self.period_close_digest != intent.period_close_digest
            || self.venue_id != execution_policy.venue_id
            || self.operator_signer_id != execution_policy.operator_signer_id
            || self.xor_amount != intent.xor_amount
            || self.xor_amount > execution_policy.max_submission_xor
            || self.max_slippage_bps != intent.max_slippage_bps
            || self.max_slippage_bps > execution_policy.max_slippage_bps
            || self.authorized_at_unix < execution_policy.valid_from_unix
            || self.authorized_at_unix >= execution_policy.valid_until_unix
            || self.authorized_at_unix == u64::MAX
            || self.expires_at_unix != intent.expires_at_unix
            || self.expires_at_unix <= self.authorized_at_unix
            || self.expires_at_unix > execution_policy.valid_until_unix
            || self.expires_at_unix == u64::MAX
            || self.authorization_sequence == 0
            || self.authorization_id != execution_authorization_digest(self)?
        {
            return Err(HedgingBillingServiceError::InvalidHedgeExecutionAuthorization);
        }
        let key = checked_verifying_key(execution_policy.operator_public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidHedgeExecutionAuthorization)?;
        let mut message = Vec::with_capacity(
            HEDGE_EXECUTION_AUTHORIZATION_SIGNATURE_DOMAIN_V1.len() + self.authorization_id.len(),
        );
        message.extend_from_slice(HEDGE_EXECUTION_AUTHORIZATION_SIGNATURE_DOMAIN_V1);
        message.extend_from_slice(&self.authorization_id);
        key.verify_strict(&message, &Signature::from_bytes(&self.signature))
            .map_err(|_| HedgingBillingServiceError::InvalidHedgeExecutionAuthorization)
    }
}
/// Runtime identity of one governed hedge venue adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedHedgeExecutionVenueIdentityV1 {
    /// Stable venue identity.
    pub venue_id: String,
    /// Exact receipt verification key.
    pub public_key: [u8; 32],
}
/// Immutable venue receipt for one explicitly authorized hedge submission.
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
pub struct HedgeExecutionSubmissionReceiptV1 {
    /// Schema version.
    pub version: u8,
    /// Exact operator authorization.
    pub authorization_id: [u8; 32],
    /// Exact hedge intent.
    pub intent_id: [u8; 32],
    /// Exact governed venue.
    pub venue_id: String,
    /// Stable venue-owned order identity.
    pub venue_order_id: String,
    /// Venue submission time.
    pub submitted_at_unix: u64,
    /// Deterministic receipt identity.
    pub receipt_digest: [u8; 32],
    /// Venue Ed25519 signature over the receipt identity.
    pub signature: [u8; 64],
}
impl HedgeExecutionSubmissionReceiptV1 {
    /// Derive the venue receipt identity/signing digest.
    ///
    /// The stored receipt identity and signature fields are zeroed.
    ///
    /// # Errors
    ///
    /// Fails when canonical encoding fails.
    pub fn receipt_digest(&self) -> Result<[u8; 32], HedgingBillingServiceError> {
        execution_receipt_digest(self)
    }
    fn validate(
        &self,
        policy: &GovernedHedgeExecutionPolicyV1,
        authorization: &HedgeExecutionAuthorizationV1,
    ) -> Result<(), HedgingBillingServiceError> {
        policy.validate()?;
        if self.version != HEDGE_EXECUTION_RECEIPT_VERSION_V1
            || self.authorization_id != authorization.authorization_id
            || self.intent_id != authorization.intent_id
            || self.venue_id != policy.venue_id
            || self.submitted_at_unix < authorization.authorized_at_unix
            || self.submitted_at_unix >= authorization.expires_at_unix
            || self.submitted_at_unix == u64::MAX
            || self.receipt_digest != execution_receipt_digest(self)?
        {
            return Err(HedgingBillingServiceError::InvalidHedgeExecutionReceipt);
        }
        validate_identifier(
            &self.venue_order_id,
            BILLING_PUBLICATION_ROUTE_MAX_BYTES_V1,
            HedgingBillingServiceError::InvalidHedgeExecutionReceipt,
        )?;
        let key = checked_verifying_key(policy.venue_public_key)
            .map_err(|_| HedgingBillingServiceError::InvalidHedgeExecutionReceipt)?;
        let mut message = Vec::with_capacity(
            HEDGE_EXECUTION_RECEIPT_SIGNATURE_DOMAIN_V1.len() + self.receipt_digest.len(),
        );
        message.extend_from_slice(HEDGE_EXECUTION_RECEIPT_SIGNATURE_DOMAIN_V1);
        message.extend_from_slice(&self.receipt_digest);
        key.verify_strict(&message, &Signature::from_bytes(&self.signature))
            .map_err(|_| HedgingBillingServiceError::InvalidHedgeExecutionReceipt)
    }
}
/// Adapter boundary for an explicitly operator-authorized venue integration.
///
/// No service loop invokes this trait. The only submission helper requires a valid authorization,
/// performs authoritative lookup first, and uses the authorization identity as its idempotency key.
pub trait GovernedHedgeExecutionAdapter: Send + Sync + fmt::Debug {
    /// Return the current venue identity and receipt key.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free failure class.
    fn identity(
        &self,
    ) -> Result<GovernedHedgeExecutionVenueIdentityV1, HedgingBillingExternalError>;
    /// Whether the adapter would execute automatically.
    ///
    /// Production V1 supervisors must reject adapters returning `true`.
    fn automatic_execution_enabled(&self) -> bool;
    /// Submit one exact explicitly authorized intent idempotently.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class. Uncertain writes must be `Ambiguous`; a
    /// successful return guarantees immutable lookup by `idempotency_key`.
    fn submit_authorized(
        &self,
        idempotency_key: [u8; 32],
        intent: &HedgeIntentV1,
        authorization: &HedgeExecutionAuthorizationV1,
    ) -> Result<HedgeExecutionSubmissionReceiptV1, HedgingBillingExternalError>;
    /// Look up an immutable submission by authorization identity.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free failure class.
    fn lookup_authorization(
        &self,
        authorization_id: [u8; 32],
    ) -> Result<Option<HedgeExecutionSubmissionReceiptV1>, HedgingBillingExternalError>;
}
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
struct StoredAccrualV1 {
    event: HedgingBillingFinalizedEventV1,
    source_receipt: [u8; 32],
}
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
)]
struct StoredEventReplayReceiptV1 {
    sequence: u64,
    event_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
struct StoredAccountStatementHeadV1 {
    account_id: Vec<u8>,
    statement_id: [u8; 32],
    period_end_unix: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
enum StoredStatementDeliveryStateV1 {
    ReadyForSigning,
    Signing,
    ReadyForPublication,
    PublicationAmbiguous,
    Published,
    Acknowledged,
    DeadLetter,
}
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
struct StoredStatementV1 {
    governed_statement: GovernedBillingStatementV1,
    state: StoredStatementDeliveryStateV1,
    signing_attempts: u32,
    signing_claim_cursor: Option<HedgingBillingFinalizedCursorV1>,
    signed_statement: Option<SignedGovernedBillingStatementV1>,
    publication_receipt: Option<BillingStatementPublicationReceiptV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize)]
struct HedgingBillingCheckpointV1 {
    version: u8,
    network_id: NetworkId,
    policy_digest: [u8; 32],
    epoch_sequence: u64,
    compacted_journal_commitment: Option<HedgingBillingJournalCommitmentV1>,
    compacted_through_period_end_unix: u64,
    compacted_account_bases: Vec<GovernedBillingStatementV1>,
    latest_epoch_transition: Option<HedgingBillingEpochTransitionV1>,
    next_event_sequence: u64,
    finalized_cursor: Option<HedgingBillingFinalizedCursorV1>,
    journal_commitment: Option<HedgingBillingJournalCommitmentV1>,
    last_page_digest: Option<[u8; 32]>,
    last_period_end_unix: u64,
    source_pages: Vec<HedgingBillingFinalizedEventPageV1>,
    period_closes: Vec<HedgingBillingFinalizedPeriodCloseV1>,
    open_accruals: Vec<StoredAccrualV1>,
    replay_receipts: Vec<[u8; 32]>,
    event_replay_receipts: Vec<StoredEventReplayReceiptV1>,
    account_heads: Vec<StoredAccountStatementHeadV1>,
    statements: Vec<StoredStatementV1>,
    acknowledgements: Vec<BillingStatementAcknowledgementV1>,
    hedge_intents: Vec<HedgeIntentV1>,
}
impl HedgingBillingCheckpointV1 {
    fn empty(policy: &HedgingBillingServicePolicyV1) -> Result<Self, HedgingBillingServiceError> {
        Ok(Self {
            version: HEDGING_BILLING_CHECKPOINT_VERSION_V1,
            network_id: policy.network_id,
            policy_digest: policy.digest()?,
            epoch_sequence: 0,
            compacted_journal_commitment: None,
            compacted_through_period_end_unix: policy.billing_epoch_unix,
            compacted_account_bases: Vec::new(),
            latest_epoch_transition: None,
            next_event_sequence: 1,
            finalized_cursor: None,
            journal_commitment: None,
            last_page_digest: None,
            last_period_end_unix: policy.billing_epoch_unix,
            source_pages: Vec::new(),
            period_closes: Vec::new(),
            open_accruals: Vec::new(),
            replay_receipts: Vec::new(),
            event_replay_receipts: Vec::new(),
            account_heads: Vec::new(),
            statements: Vec::new(),
            acknowledgements: Vec::new(),
            hedge_intents: Vec::new(),
        })
    }
    fn validate(
        &self,
        policy: &HedgingBillingServicePolicyV1,
        feed_policy: &HedgingFeedTrustPolicyV1,
    ) -> Result<(), HedgingBillingServiceError> {
        if self.version != HEDGING_BILLING_CHECKPOINT_VERSION_V1
            || self.network_id != policy.network_id
            || self.policy_digest != policy.digest()?
            || self.epoch_sequence.checked_add(1) != Some(policy.revision)
            || self.next_event_sequence == 0
            || self.compacted_account_bases.len()
                > usize::try_from(policy.max_accounts).unwrap_or(usize::MAX)
            || self.compacted_through_period_end_unix < policy.billing_epoch_unix
            || self.compacted_through_period_end_unix > self.last_period_end_unix
            || !(self.compacted_through_period_end_unix - policy.billing_epoch_unix)
                .is_multiple_of(policy.billing_period_secs)
            || self.last_period_end_unix < policy.billing_epoch_unix
            || !(self.last_period_end_unix - policy.billing_epoch_unix)
                .is_multiple_of(policy.billing_period_secs)
            || self.open_accruals.len()
                > usize::try_from(policy.max_open_accruals).unwrap_or(usize::MAX)
            || self.replay_receipts.len()
                > usize::try_from(policy.max_replay_receipts).unwrap_or(usize::MAX)
            || self.event_replay_receipts.len()
                > usize::try_from(policy.max_replay_receipts).unwrap_or(usize::MAX)
            || self.account_heads.len() > usize::try_from(policy.max_accounts).unwrap_or(usize::MAX)
            || self.statements.len() > usize::try_from(policy.max_statements).unwrap_or(usize::MAX)
            || self.acknowledgements.len()
                > usize::try_from(policy.max_acknowledgements).unwrap_or(usize::MAX)
            || self.hedge_intents.len()
                > usize::try_from(policy.max_hedge_intents).unwrap_or(usize::MAX)
            || self.source_pages.len()
                > usize::try_from(policy.max_retained_source_pages).unwrap_or(usize::MAX)
            || self.period_closes.len()
                > usize::try_from(policy.max_retained_period_closes).unwrap_or(usize::MAX)
        {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        match (
            self.epoch_sequence,
            self.compacted_journal_commitment,
            &self.latest_epoch_transition,
        ) {
            (0, None, None)
                if policy.revision == 1
                    && self.compacted_through_period_end_unix == policy.billing_epoch_unix
                    && self.compacted_account_bases.is_empty() => {}
            (epoch, Some(commitment), Some(transition)) if epoch != 0 => {
                transition.verify()?;
                if transition.epoch_sequence != epoch
                    || &transition.next_service_policy != policy
                    || &transition.next_feed_policy != feed_policy
                    || transition.predecessor_journal_commitment != commitment
                    || transition.compacted_through_period_end_unix
                        != self.compacted_through_period_end_unix
                    || transition.next_source_sequence != commitment.journal_next_sequence
                    || transition.compacted_account_bases_digest
                        != hash_canonical(
                            b"sorafs.hedging-billing.compacted-account-bases.v1",
                            &self.compacted_account_bases,
                        )?
                    || transition.retained_epoch_state_digest != retained_epoch_state_digest(self)?
                {
                    return Err(HedgingBillingServiceError::InvalidCheckpoint);
                }
            }
            _ => return Err(HedgingBillingServiceError::InvalidCheckpoint),
        }
        if let Some(cursor) = self.finalized_cursor {
            cursor.validate()?;
            if self
                .journal_commitment
                .is_none_or(|commitment| commitment.finalized_cursor != cursor)
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        } else if self.next_event_sequence != 1
            || self.journal_commitment.is_some()
            || self.last_page_digest.is_some()
            || !self.open_accruals.is_empty()
            || !self.replay_receipts.is_empty()
            || !self.event_replay_receipts.is_empty()
            || !self.source_pages.is_empty()
            || !self.period_closes.is_empty()
        {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        if let Some(commitment) = self.journal_commitment {
            commitment.validate(policy.network_id)?;
            if commitment.journal_next_sequence < self.next_event_sequence
                || self.last_period_end_unix > commitment.finalized_cursor.finalized_at_unix
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        }
        if let Some(base) = self.compacted_journal_commitment {
            base.validate(policy.network_id)?;
            if self.next_event_sequence < base.journal_next_sequence
                || self.journal_commitment.is_none_or(|current| {
                    current.journal_next_sequence < base.journal_next_sequence
                        || current.finalized_cursor.height < base.finalized_cursor.height
                })
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        }
        let mut source_next_sequence = self
            .compacted_journal_commitment
            .map_or(1, |commitment| commitment.journal_next_sequence);
        let mut previous_source_commitment = self.compacted_journal_commitment;
        let mut previous_source_position = None;
        let mut source_block_hashes = BTreeMap::new();
        let mut observed_commitments = BTreeSet::new();
        if let Some(commitment) = self.compacted_journal_commitment {
            observed_commitments.insert(commitment);
        }
        let mut source_event_digests = Vec::new();
        let mut source_receipts = BTreeSet::new();
        let mut source_events = Vec::new();
        let mut last_source_page_digest = None;
        for page in &self.source_pages {
            last_source_page_digest = Some(page.validate(policy)?);
            if page.start_sequence != source_next_sequence {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
            if let Some(previous) = previous_source_commitment {
                if page.journal_commitment.finalized_cursor.height
                    < previous.finalized_cursor.height
                    || page.journal_commitment.finalized_cursor.finalized_at_unix
                        < previous.finalized_cursor.finalized_at_unix
                    || (page.journal_commitment.finalized_cursor.height
                        == previous.finalized_cursor.height
                        && page.journal_commitment.finalized_cursor.block_hash
                            != previous.finalized_cursor.block_hash)
                    || page.journal_commitment.journal_next_sequence
                        < previous.journal_next_sequence
                    || (page.journal_commitment.journal_next_sequence
                        == previous.journal_next_sequence
                        && page.journal_commitment.journal_root != previous.journal_root)
                    || (page.journal_commitment.journal_next_sequence
                        > previous.journal_next_sequence
                        && page.journal_commitment.journal_root == previous.journal_root)
                {
                    return Err(HedgingBillingServiceError::InvalidCheckpoint);
                }
            }
            for event in &page.events {
                let position = (event.block_height, event.event_index);
                if previous_source_position.is_some_and(|previous| previous >= position)
                    || source_block_hashes
                        .insert(event.block_height, event.block_hash)
                        .is_some_and(|previous| previous != event.block_hash)
                    || !source_receipts.insert(source_receipt(policy.network_id, event)?)
                {
                    return Err(HedgingBillingServiceError::InvalidCheckpoint);
                }
                source_event_digests.push(StoredEventReplayReceiptV1 {
                    sequence: event.sequence,
                    event_digest: event_replay_digest(policy.network_id, event)?,
                });
                source_events.push(event.clone());
                previous_source_position = Some(position);
            }
            source_next_sequence = page.next_sequence;
            previous_source_commitment = Some(page.journal_commitment);
            observed_commitments.insert(page.journal_commitment);
        }
        if source_next_sequence != self.next_event_sequence
            || previous_source_commitment != self.journal_commitment
            || last_source_page_digest != self.last_page_digest
            || source_event_digests != self.event_replay_receipts
            || source_receipts.iter().copied().collect::<Vec<_>>() != self.replay_receipts
        {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        let mut expected_period_end = self.compacted_through_period_end_unix;
        for close in &self.period_closes {
            close.validate(policy, feed_policy)?;
            expected_period_end = expected_period_end
                .checked_add(policy.billing_period_secs)
                .ok_or(HedgingBillingServiceError::AmountOverflow)?;
            if close.period_end_unix != expected_period_end
                || close.journal_commitment.journal_next_sequence > self.next_event_sequence
                || !observed_commitments.contains(&close.journal_commitment)
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        }
        if expected_period_end != self.last_period_end_unix {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        let derived = derive_billing_domain_state(
            policy,
            feed_policy,
            &source_events,
            &self.period_closes,
            &self.compacted_account_bases,
        )?;
        let stored_governed_statements: Vec<GovernedBillingStatementV1> = self
            .statements
            .iter()
            .map(|record| record.governed_statement.clone())
            .collect();
        if self.open_accruals != derived.open_accruals
            || self.account_heads != derived.account_heads
            || stored_governed_statements != derived.governed_statements
            || self.hedge_intents != derived.hedge_intents
        {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        let mut previous_event_sequence = None;
        let mut open_receipts = BTreeSet::new();
        for accrual in &self.open_accruals {
            let cursor = self
                .finalized_cursor
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            accrual.event.validate(policy.billing_epoch_unix, cursor)?;
            if previous_event_sequence.is_some_and(|previous| previous >= accrual.event.sequence)
                || accrual.source_receipt != source_receipt(policy.network_id, &accrual.event)?
                || !open_receipts.insert(accrual.source_receipt)
                || accrual.event.occurred_at_unix < self.last_period_end_unix
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
            previous_event_sequence = Some(accrual.event.sequence);
        }
        if !is_strictly_sorted_unique(&self.replay_receipts) {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        if !is_strictly_sorted_unique(&self.event_replay_receipts)
            || self
                .event_replay_receipts
                .last()
                .is_some_and(|receipt| receipt.sequence >= self.next_event_sequence)
        {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        if open_receipts
            .iter()
            .any(|receipt| self.replay_receipts.binary_search(receipt).is_err())
        {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        for accrual in &self.open_accruals {
            let replay = self
                .event_replay_receipts
                .binary_search_by_key(&accrual.event.sequence, |receipt| receipt.sequence)
                .ok()
                .and_then(|index| self.event_replay_receipts.get(index))
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            if replay.event_digest != event_replay_digest(policy.network_id, &accrual.event)? {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        }
        let mut previous_account: Option<&[u8]> = None;
        for head in &self.account_heads {
            if validate_canonical_account_id_bytes(&head.account_id).is_err()
                || head.statement_id == [0; 32]
                || head.period_end_unix <= policy.billing_epoch_unix
                || previous_account.is_some_and(|previous| previous >= head.account_id.as_slice())
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
            previous_account = Some(&head.account_id);
        }
        let mut previous_statement_id = None;
        let mut known_statement_ids = BTreeSet::new();
        let mut statements_by_account: BTreeMap<Vec<u8>, Vec<&GovernedBillingStatementV1>> =
            BTreeMap::new();
        let mut compacted_base_ids = BTreeSet::new();
        let mut previous_base_account: Option<&[u8]> = None;
        for governed in &self.compacted_account_bases {
            governed.validate_structure()?;
            let statement = &governed.statement;
            if statement.period_end_unix > self.compacted_through_period_end_unix
                || validate_canonical_account_id_bytes(&statement.account_id).is_err()
                || previous_base_account
                    .is_some_and(|previous| previous >= statement.account_id.as_slice())
                || !known_statement_ids.insert(statement.statement_id)
                || !compacted_base_ids.insert(statement.statement_id)
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
            previous_base_account = Some(&statement.account_id);
            statements_by_account
                .entry(statement.account_id.clone())
                .or_default()
                .push(governed);
        }
        let mut statement_ids_by_period: BTreeMap<u64, Vec<[u8; 32]>> = BTreeMap::new();
        let mut exposure_by_period: BTreeMap<u64, XorQuantity> = BTreeMap::new();
        for record in &self.statements {
            let statement = &record.governed_statement.statement;
            record.governed_statement.validate_structure()?;
            if previous_statement_id.is_some_and(|previous| previous >= statement.statement_id)
                || !known_statement_ids.insert(statement.statement_id)
                || validate_canonical_account_id_bytes(&statement.account_id).is_err()
                || record.signing_attempts > policy.max_signing_attempts
                || statement.period_end_unix > self.last_period_end_unix
                || record
                    .governed_statement
                    .governed_reference_price
                    .decision
                    .max_feed_age_secs
                    != policy.max_feed_age_secs
                || record
                    .governed_statement
                    .governed_reference_price
                    .decision
                    .max_divergence_bps
                    != policy.max_divergence_bps
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
            record
                .governed_statement
                .governed_reference_price
                .verify(feed_policy, statement.period_end_unix)?;
            previous_statement_id = Some(statement.statement_id);
            statements_by_account
                .entry(statement.account_id.clone())
                .or_default()
                .push(&record.governed_statement);
            statement_ids_by_period
                .entry(statement.period_end_unix)
                .or_default()
                .push(statement.statement_id);
            let exposure = exposure_by_period
                .entry(statement.period_end_unix)
                .or_insert_with(XorQuantity::zero);
            *exposure = exposure.checked_add(&statement.net_due_xor)?;
            match record.state {
                StoredStatementDeliveryStateV1::ReadyForSigning => {
                    if record.signing_claim_cursor.is_some()
                        || record.signed_statement.is_some()
                        || record.publication_receipt.is_some()
                    {
                        return Err(HedgingBillingServiceError::InvalidCheckpoint);
                    }
                }
                StoredStatementDeliveryStateV1::Signing => {
                    if record.signing_claim_cursor.is_none()
                        || record.signed_statement.is_some()
                        || record.publication_receipt.is_some()
                    {
                        return Err(HedgingBillingServiceError::InvalidCheckpoint);
                    }
                }
                StoredStatementDeliveryStateV1::ReadyForPublication
                | StoredStatementDeliveryStateV1::PublicationAmbiguous => {
                    if record.signing_claim_cursor.is_some()
                        || record.signed_statement.is_none()
                        || record.publication_receipt.is_some()
                    {
                        return Err(HedgingBillingServiceError::InvalidCheckpoint);
                    }
                }
                StoredStatementDeliveryStateV1::Published
                | StoredStatementDeliveryStateV1::Acknowledged => {
                    if record.signing_claim_cursor.is_some()
                        || record.signed_statement.is_none()
                        || record.publication_receipt.is_none()
                    {
                        return Err(HedgingBillingServiceError::InvalidCheckpoint);
                    }
                }
                StoredStatementDeliveryStateV1::DeadLetter => {
                    if record.signing_claim_cursor.is_some()
                        || record.signed_statement.is_some()
                        || record.publication_receipt.is_some()
                        || record.signing_attempts != policy.max_signing_attempts
                    {
                        return Err(HedgingBillingServiceError::InvalidCheckpoint);
                    }
                }
            }
            if let Some(signed) = &record.signed_statement {
                let close = self
                    .period_closes
                    .iter()
                    .find(|close| close.period_end_unix == statement.period_end_unix)
                    .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
                signed.verify(policy, feed_policy, close)?;
                if signed.governed_statement != record.governed_statement {
                    return Err(HedgingBillingServiceError::InvalidCheckpoint);
                }
            }
            if let Some(receipt) = &record.publication_receipt {
                receipt.validate(
                    record
                        .signed_statement
                        .as_ref()
                        .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?,
                    &policy.statement_publisher,
                )?;
            }
        }
        for statements in statements_by_account.values_mut() {
            statements.sort_by_key(|governed| governed.statement.period_end_unix);
            let mut previous = None;
            for governed in statements.iter() {
                if previous.is_none()
                    && compacted_base_ids.contains(&governed.statement.statement_id)
                {
                    previous = Some(&governed.statement);
                    continue;
                }
                validate_billing_statement_transition(previous, &governed.statement)?;
                if previous.is_none()
                    && governed.statement.period_start_unix != policy.billing_epoch_unix
                {
                    return Err(HedgingBillingServiceError::InvalidCheckpoint);
                }
                previous = Some(&governed.statement);
            }
        }
        let mut previous_ack = None;
        let mut acknowledged_statements = BTreeSet::new();
        for acknowledgement in &self.acknowledgements {
            let statement_record = self
                .statements
                .binary_search_by_key(&acknowledgement.statement_id, |record| {
                    record.governed_statement.statement.statement_id
                })
                .ok()
                .and_then(|index| self.statements.get(index))
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            let publication = statement_record
                .publication_receipt
                .as_ref()
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            if previous_ack.is_some_and(|previous| previous >= acknowledgement.acknowledgement_id)
                || acknowledgement.version != BILLING_STATEMENT_ACKNOWLEDGEMENT_VERSION_V1
                || acknowledgement.network_id != policy.network_id
                || acknowledgement.statement_id == [0; 32]
                || acknowledgement.account_digest == [0; 32]
                || acknowledgement.request_binding_digest == [0; 32]
                || acknowledgement.acknowledged_at_unix == 0
                || acknowledgement.acknowledged_at_unix == u64::MAX
                || acknowledgement.authentication_proof.is_empty()
                || acknowledgement.authentication_proof.len()
                    > BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1
                || acknowledgement.acknowledgement_id != acknowledgement_digest(acknowledgement)?
                || !known_statement_ids.contains(&acknowledgement.statement_id)
                || !acknowledged_statements.insert(acknowledgement.statement_id)
                || acknowledgement.account_digest
                    != *blake3::hash(&statement_record.governed_statement.statement.account_id)
                        .as_bytes()
                || acknowledgement.acknowledged_at_unix < publication.published_at_unix
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
            previous_ack = Some(acknowledgement.acknowledgement_id);
        }
        for record in &self.statements {
            let statement_id = record.governed_statement.statement.statement_id;
            if (record.state == StoredStatementDeliveryStateV1::Acknowledged)
                != acknowledged_statements.contains(&statement_id)
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        }
        let mut previous_intent = None;
        let mut intent_periods = BTreeSet::new();
        for intent in &self.hedge_intents {
            intent.validate(policy)?;
            let mut period_statement_ids = statement_ids_by_period
                .get(&intent.period_end_unix)
                .cloned()
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            period_statement_ids.sort();
            if previous_intent.is_some_and(|previous| previous >= intent.intent_id)
                || !intent_periods.insert(intent.period_end_unix)
                || intent.statement_bundle_digest
                    != hash_canonical(b"sorafs.billing.statement-bundle.v1", &period_statement_ids)?
                || exposure_by_period.get(&intent.period_end_unix) != Some(&intent.xor_amount)
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
            previous_intent = Some(intent.intent_id);
        }
        for (period_end_unix, exposure) in &exposure_by_period {
            let intent_required = exposure >= &policy.hedge_intent_threshold_xor;
            if intent_required != intent_periods.contains(period_end_unix) {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        }
        for head in &self.account_heads {
            let latest = statements_by_account
                .get(&head.account_id)
                .and_then(|statements| statements.last())
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            if latest.statement.statement_id != head.statement_id
                || latest.statement.period_end_unix != head.period_end_unix
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        }
        if self.account_heads.len() != statements_by_account.len() {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
        Ok(())
    }
    fn verify_authenticated_sources(
        &self,
        policy: &HedgingBillingServicePolicyV1,
        verifier: &dyn HedgingBillingJournalVerifier,
    ) -> Result<(), HedgingBillingServiceError> {
        let mut previous = self.compacted_journal_commitment;
        for page in &self.source_pages {
            verifier.verify_page(&policy.network_id, previous, page)?;
            previous = Some(page.journal_commitment);
        }
        for close in &self.period_closes {
            verifier.verify_period_close(&policy.network_id, close)?;
        }
        Ok(())
    }
}
#[derive(Debug)]
struct DerivedBillingDomainState {
    open_accruals: Vec<StoredAccrualV1>,
    account_heads: Vec<StoredAccountStatementHeadV1>,
    governed_statements: Vec<GovernedBillingStatementV1>,
    hedge_intents: Vec<HedgeIntentV1>,
}
fn derive_billing_domain_state(
    policy: &HedgingBillingServicePolicyV1,
    feed_policy: &HedgingFeedTrustPolicyV1,
    events: &[HedgingBillingFinalizedEventV1],
    closes: &[HedgingBillingFinalizedPeriodCloseV1],
    compacted_account_bases: &[GovernedBillingStatementV1],
) -> Result<DerivedBillingDomainState, HedgingBillingServiceError> {
    let mut closes_by_period = BTreeMap::new();
    for close in closes {
        close.validate(policy, feed_policy)?;
        if closes_by_period
            .insert(close.period_end_unix, close)
            .is_some()
        {
            return Err(HedgingBillingServiceError::InvalidPeriodClose);
        }
    }
    let last_closed_period = closes.last().map(|close| close.period_end_unix);
    let mut events_by_period: BTreeMap<
        u64,
        BTreeMap<Vec<u8>, Vec<&HedgingBillingFinalizedEventV1>>,
    > = BTreeMap::new();
    for event in events {
        let elapsed = event
            .occurred_at_unix
            .checked_sub(policy.billing_epoch_unix)
            .ok_or(HedgingBillingServiceError::InvalidFinalizedEvent)?;
        let period_number = elapsed
            .checked_div(policy.billing_period_secs)
            .and_then(|number| number.checked_add(1))
            .ok_or(HedgingBillingServiceError::AmountOverflow)?;
        let period_end_unix = policy
            .billing_period_secs
            .checked_mul(period_number)
            .and_then(|offset| policy.billing_epoch_unix.checked_add(offset))
            .ok_or(HedgingBillingServiceError::AmountOverflow)?;
        if let Some(close) = closes_by_period.get(&period_end_unix) {
            if event.sequence >= close.journal_commitment.journal_next_sequence {
                return Err(HedgingBillingServiceError::LateFinalizedEvent);
            }
            events_by_period
                .entry(period_end_unix)
                .or_default()
                .entry(event.account_id.clone())
                .or_default()
                .push(event);
        } else if last_closed_period.is_some_and(|last| period_end_unix <= last) {
            return Err(HedgingBillingServiceError::InvalidPeriodClose);
        }
    }
    let mut consumed_sequences = BTreeSet::new();
    let mut heads: BTreeMap<Vec<u8>, StoredAccountStatementHeadV1> = BTreeMap::new();
    let mut governed_by_id: BTreeMap<[u8; 32], GovernedBillingStatementV1> = BTreeMap::new();
    let mut active_governed_by_id: BTreeMap<[u8; 32], GovernedBillingStatementV1> = BTreeMap::new();
    let mut hedge_intents = Vec::new();
    for governed in compacted_account_bases {
        governed.validate_structure()?;
        let statement = &governed.statement;
        if governed_by_id
            .insert(statement.statement_id, governed.clone())
            .is_some()
            || heads
                .insert(
                    statement.account_id.clone(),
                    StoredAccountStatementHeadV1 {
                        account_id: statement.account_id.clone(),
                        statement_id: statement.statement_id,
                        period_end_unix: statement.period_end_unix,
                    },
                )
                .is_some()
        {
            return Err(HedgingBillingServiceError::InvalidCheckpoint);
        }
    }
    for close in closes {
        let by_account = events_by_period
            .remove(&close.period_end_unix)
            .unwrap_or_default();
        let due_at_unix = close
            .period_end_unix
            .checked_add(policy.payment_due_after_secs)
            .ok_or(HedgingBillingServiceError::AmountOverflow)?;
        let mut period_statements = Vec::new();
        let mut exposure = XorQuantity::zero();
        for (account_id, account_events) in by_account {
            if account_events.len() > MAX_BILLING_LINES {
                return Err(HedgingBillingServiceError::ResourceExhausted);
            }
            let previous_head = heads.get(&account_id);
            let previous = previous_head
                .and_then(|head| governed_by_id.get(&head.statement_id))
                .map(|governed| &governed.statement);
            let period_start_unix =
                previous_head.map_or(policy.billing_epoch_unix, |head| head.period_end_unix);
            let mut lines = Vec::new();
            lines
                .try_reserve_exact(account_events.len())
                .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?;
            for event in account_events {
                lines.push(build_billing_line_item_v1(
                    event.kind,
                    event.direction,
                    canonical_billing_line_source_id(event)?,
                    event.xor_amount.clone(),
                    &close.governed_reference_price.decision.xor_usd_price,
                    event.quantity_units,
                    None,
                )?);
                consumed_sequences.insert(event.sequence);
            }
            let statement = build_billing_statement_v1(
                account_id.clone(),
                period_start_unix,
                close.period_end_unix,
                due_at_unix,
                close.governed_reference_price.decision.clone(),
                lines,
                previous_head.map(|head| head.statement_id),
            )?;
            validate_billing_statement_transition(previous, &statement)?;
            exposure = exposure.checked_add(&statement.net_due_xor)?;
            let statement_id = statement.statement_id;
            let governed = bind_governed_billing_statement_v1(
                statement,
                close.governed_reference_price.clone(),
            )?;
            let governed_bytes = governed.canonical_bytes()?;
            if governed_bytes.len() > MAX_GOVERNED_BILLING_STATEMENT_BYTES {
                return Err(HedgingBillingServiceError::ResourceExhausted);
            }
            heads.insert(
                account_id.clone(),
                StoredAccountStatementHeadV1 {
                    account_id,
                    statement_id,
                    period_end_unix: close.period_end_unix,
                },
            );
            if governed_by_id
                .insert(statement_id, governed.clone())
                .is_some()
            {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
            active_governed_by_id.insert(statement_id, governed.clone());
            period_statements.push(governed);
        }
        period_statements.sort_by_key(|governed| governed.statement.statement_id);
        let statement_ids: Vec<[u8; 32]> = period_statements
            .iter()
            .map(|governed| governed.statement.statement_id)
            .collect();
        let statement_bundle_digest =
            hash_canonical(b"sorafs.billing.statement-bundle.v1", &statement_ids)?;
        if exposure >= policy.hedge_intent_threshold_xor {
            let mut intent = HedgeIntentV1 {
                version: HEDGE_INTENT_VERSION_V1,
                intent_id: [0; 32],
                network_id: policy.network_id,
                service_policy_digest: policy.digest()?,
                period_close_digest: close.close_digest(policy, feed_policy)?,
                period_end_unix: close.period_end_unix,
                finalized_cursor: close.journal_commitment.finalized_cursor,
                reference_price_decision_id: close.governed_reference_price.decision.decision_id,
                statement_bundle_digest,
                direction: HedgeIntentDirectionV1::SellXor,
                disposition: if exposure > policy.max_hedge_intent_xor {
                    HedgeIntentDispositionV1::GovernedOverflow
                } else {
                    HedgeIntentDispositionV1::Executable
                },
                xor_amount: exposure,
                max_slippage_bps: policy.hedge_max_slippage_bps,
                expires_at_unix: close
                    .period_end_unix
                    .checked_add(policy.hedge_intent_ttl_secs)
                    .ok_or(HedgingBillingServiceError::AmountOverflow)?,
                automatic_execution: false,
            };
            intent.intent_id = hash_canonical(HEDGE_INTENT_DOMAIN_V1, &intent)?;
            intent.validate(policy)?;
            hedge_intents.push(intent);
        }
    }
    if !events_by_period.is_empty() {
        return Err(HedgingBillingServiceError::InvalidPeriodClose);
    }
    let mut open_accruals = events
        .iter()
        .filter(|event| !consumed_sequences.contains(&event.sequence))
        .map(|event| {
            Ok(StoredAccrualV1 {
                event: event.clone(),
                source_receipt: source_receipt(policy.network_id, event)?,
            })
        })
        .collect::<Result<Vec<_>, HedgingBillingServiceError>>()?;
    open_accruals.sort_by_key(|accrual| accrual.event.sequence);
    let mut governed_statements: Vec<GovernedBillingStatementV1> =
        active_governed_by_id.into_values().collect();
    governed_statements.sort_by_key(|governed| governed.statement.statement_id);
    hedge_intents.sort_by_key(|intent| intent.intent_id);
    Ok(DerivedBillingDomainState {
        open_accruals,
        account_heads: heads.into_values().collect(),
        governed_statements,
        hedge_intents,
    })
}
fn canonical_billing_line_source_id(
    event: &HedgingBillingFinalizedEventV1,
) -> Result<String, HedgingBillingServiceError> {
    let source = match event.source {
        BillingAccrualSourceV1::Storage => "storage",
        BillingAccrualSourceV1::OrderbookSettlement => "orderbook-settlement",
        BillingAccrualSourceV1::ReserveRent => "reserve-rent",
        BillingAccrualSourceV1::Egress => "egress",
        BillingAccrualSourceV1::OrchestratorFee => "orchestrator-fee",
        BillingAccrualSourceV1::Incentive => "incentive",
        BillingAccrualSourceV1::Penalty => "penalty",
        BillingAccrualSourceV1::GovernanceAdjustment => "governance-adjustment",
    };
    let identifier = format!("{source}:{}", event.source_id);
    validate_identifier(
        &identifier,
        MAX_HEDGING_IDENTIFIER_BYTES,
        HedgingBillingServiceError::InvalidFinalizedEvent,
    )?;
    Ok(identifier)
}
#[derive(Debug)]
struct RuntimeState {
    checkpoint: HedgingBillingCheckpointV1,
    fingerprint: Option<[u8; 32]>,
}
/// Result of ingesting one finalized page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedgingBillingIngestOutcomeV1 {
    /// A new page advanced the durable cursor.
    Applied {
        /// Number of newly projected events.
        event_count: usize,
        /// First sequence expected by the next page.
        next_sequence: u64,
    },
    /// The exact most-recent page was replayed.
    Replay {
        /// First sequence expected by the next page.
        next_sequence: u64,
    },
}
/// Result of finalizing one billing period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HedgingBillingPeriodOutcomeV1 {
    /// Finalized period end.
    pub period_end_unix: u64,
    /// Generated statement identifiers in canonical order.
    pub statement_ids: Vec<[u8; 32]>,
    /// Digest of the canonical statement-id inventory.
    pub statement_bundle_digest: [u8; 32],
    /// Optional generated hedge intent.
    pub hedge_intent: Option<HedgeIntentV1>,
}
/// Bounded reconciliation scan outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedgingBillingReconcileOutcomeV1 {
    /// Newly applied pages.
    pub pages_applied: u32,
    /// Newly projected events.
    pub events_applied: u64,
    /// First event sequence expected by the next scan.
    pub next_sequence: u64,
    /// Latest finalized view observed by the projector.
    pub finalized_cursor: Option<HedgingBillingFinalizedCursorV1>,
}
/// Result of sealing and atomically installing one new billing epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HedgingBillingEpochTransitionOutcomeV1 {
    /// New monotonic epoch sequence.
    pub epoch_sequence: u64,
    /// Exact signed audit witness.
    pub transition: HedgingBillingEpochTransitionV1,
    /// Sealed witness-store revision.
    pub witness_revision: [u8; 32],
}
/// Payload-free statement delivery status.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum BillingStatementDeliveryStatusV1 {
    /// Waiting for HSM/KMS signing.
    ReadyForSigning,
    /// An HSM/KMS signing claim is durable.
    Signing,
    /// Signed bytes are durable and ready for publication.
    ReadyForPublication,
    /// Publication may have committed and requires lookup.
    PublicationAmbiguous,
    /// Publication is durably reconciled.
    Published,
    /// The account durably acknowledged the publication.
    Acknowledged,
    /// Signing retry budget was exhausted.
    DeadLetter,
}
impl From<StoredStatementDeliveryStateV1> for BillingStatementDeliveryStatusV1 {
    fn from(value: StoredStatementDeliveryStateV1) -> Self {
        match value {
            StoredStatementDeliveryStateV1::ReadyForSigning => Self::ReadyForSigning,
            StoredStatementDeliveryStateV1::Signing => Self::Signing,
            StoredStatementDeliveryStateV1::ReadyForPublication => Self::ReadyForPublication,
            StoredStatementDeliveryStateV1::PublicationAmbiguous => Self::PublicationAmbiguous,
            StoredStatementDeliveryStateV1::Published => Self::Published,
            StoredStatementDeliveryStateV1::Acknowledged => Self::Acknowledged,
            StoredStatementDeliveryStateV1::DeadLetter => Self::DeadLetter,
        }
    }
}
/// Payload-free statement delivery projection.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
pub struct BillingStatementDeliveryProjectionV1 {
    /// Statement identifier.
    pub statement_id: [u8; 32],
    /// Account digest.
    pub account_digest: [u8; 32],
    /// Period end.
    pub period_end_unix: u64,
    /// Delivery status.
    pub status: BillingStatementDeliveryStatusV1,
    /// Signing attempts consumed.
    pub signing_attempts: u32,
}
/// Payload-free durable service projection for health and telemetry.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
pub struct HedgingBillingServiceStatusV1 {
    /// First finalized journal sequence not yet projected.
    pub next_event_sequence: u64,
    /// Latest finalized height represented locally, or zero before first page.
    pub finalized_height: u64,
    /// Exact next fixed billing boundary.
    pub next_period_end_unix: u64,
    /// Statements waiting for an HSM/KMS signature.
    pub ready_for_signing: u32,
    /// Statements with an in-progress durable signing claim.
    pub signing: u32,
    /// Signed statements waiting for immutable publication.
    pub ready_for_publication: u32,
    /// Publications requiring authoritative lookup.
    pub publication_ambiguous: u32,
    /// Published statements without a durable acknowledgement.
    pub published: u32,
    /// Durably acknowledged statements.
    pub acknowledged: u32,
    /// Statements whose signing retry budget is exhausted.
    pub dead_letter: u32,
    /// Retained generated hedge intents. V1 never executes them automatically.
    pub hedge_intents: u32,
}
/// Retention contract for V1 runtime projections.
///
/// The local checkpoint exposes only the active billing epoch. Older epochs remain available
/// through sealed audit witnesses, not through these bounded runtime pages.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
#[norito(tag = "scope", content = "value", rename_all = "snake_case")]
pub enum HedgingBillingRetentionScopeV1 {
    /// Only records retained in the currently active checkpoint epoch.
    ActiveEpochOnly,
}
/// Exact immutable anchor for one runtime projection read.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
pub struct HedgingBillingProjectionAnchorV1 {
    /// BLAKE3 fingerprint of the exact canonical checkpoint bytes.
    pub checkpoint_fingerprint: [u8; 32],
    /// Latest finalized block represented by the checkpoint.
    pub finalized_cursor: Option<HedgingBillingFinalizedCursorV1>,
    /// First finalized journal sequence not represented by the checkpoint.
    pub next_event_sequence: u64,
    /// Monotonic active checkpoint epoch.
    pub active_epoch_sequence: u64,
    /// Governed service-policy revision for the active epoch.
    pub active_policy_revision: u64,
    /// Digest of the exact governed service policy for the active epoch.
    pub service_policy_digest: [u8; 32],
    /// Records through this period end were compacted out of the active epoch.
    pub compacted_through_period_end_unix: u64,
    /// Explicit V1 retention contract.
    pub retention_scope: HedgingBillingRetentionScopeV1,
}
/// Owner-scoped bounded statement-list request.
#[derive(
    Clone,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
pub struct BillingStatementListRequestV1 {
    /// Exact canonical UTF-8 I105 account bytes.
    pub owner_account_id: Vec<u8>,
    /// Exclusive statement-id cursor.
    pub after_statement_id: Option<[u8; 32]>,
    /// Page size in `1..=100`.
    pub limit: u16,
    /// Exact checkpoint fingerprint observed by the caller.
    pub expected_checkpoint_fingerprint: [u8; 32],
}
impl fmt::Debug for BillingStatementListRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BillingStatementListRequestV1")
            .field(
                "owner_account_digest",
                &blake3::hash(&self.owner_account_id),
            )
            .field("after_statement_id", &self.after_statement_id)
            .field("limit", &self.limit)
            .field(
                "expected_checkpoint_fingerprint",
                &self.expected_checkpoint_fingerprint,
            )
            .finish()
    }
}
/// Public terminal delivery state visible to a statement owner.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum BillingStatementOwnerStatusV1 {
    /// The signed statement and publisher receipt are durably reconciled.
    Published,
    /// The owner acknowledgement is durably reconciled.
    Acknowledged,
}
/// Compact owner-visible statement list item.
#[derive(
    Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct BillingStatementListItemV1 {
    /// Statement identifier and exclusive-page cursor.
    pub statement_id: [u8; 32],
    /// Inclusive billing-period start.
    pub period_start_unix: u64,
    /// Exclusive billing-period end.
    pub period_end_unix: u64,
    /// Payment due timestamp.
    pub due_at_unix: u64,
    /// Exact net XOR amount due.
    pub net_due_xor: XorQuantity,
    /// Digest of the exact signed statement envelope.
    pub signed_statement_digest: [u8; 32],
    /// Immutable publisher receipt digest.
    pub publication_receipt_digest: [u8; 32],
    /// Owner-visible terminal delivery state.
    pub status: BillingStatementOwnerStatusV1,
    /// Durable acknowledgement identity, when acknowledged.
    pub acknowledgement_id: Option<[u8; 32]>,
}
/// One bounded owner-scoped statement page.
#[derive(
    Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct BillingStatementPageV1 {
    /// Exact checkpoint anchor shared by every item.
    pub anchor: HedgingBillingProjectionAnchorV1,
    /// Digest of the canonical owner bytes; raw owner bytes are not echoed.
    pub owner_account_digest: [u8; 32],
    /// Compact terminal statement projections.
    pub items: Vec<BillingStatementListItemV1>,
    /// Last returned statement id when another page exists.
    pub next_cursor: Option<[u8; 32]>,
}
impl fmt::Debug for BillingStatementPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BillingStatementPageV1")
            .field("anchor", &self.anchor)
            .field("owner_account_digest", &self.owner_account_digest)
            .field("item_count", &self.items.len())
            .field("next_cursor", &self.next_cursor)
            .finish()
    }
}
/// Owner-scoped request for one published statement.
#[derive(
    Clone,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
pub struct BillingPublishedStatementRequestV1 {
    /// Exact canonical UTF-8 I105 account bytes.
    pub owner_account_id: Vec<u8>,
    /// Requested statement identifier.
    pub statement_id: [u8; 32],
    /// Exact checkpoint fingerprint observed by the caller.
    pub expected_checkpoint_fingerprint: [u8; 32],
}
impl fmt::Debug for BillingPublishedStatementRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BillingPublishedStatementRequestV1")
            .field(
                "owner_account_digest",
                &blake3::hash(&self.owner_account_id),
            )
            .field("statement_id", &self.statement_id)
            .field(
                "expected_checkpoint_fingerprint",
                &self.expected_checkpoint_fingerprint,
            )
            .finish()
    }
}
/// Payload-free projection of one durable account acknowledgement.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
pub struct BillingStatementAcknowledgementProjectionV1 {
    /// Deterministic acknowledgement identity.
    pub acknowledgement_id: [u8; 32],
    /// Acknowledged statement identifier.
    pub statement_id: [u8; 32],
    /// Digest of the exact canonical owner bytes.
    pub account_digest: [u8; 32],
    /// Domain-separated request binding.
    pub request_binding_digest: [u8; 32],
    /// Server-controlled acknowledgement timestamp.
    pub acknowledged_at_unix: u64,
}
/// Exact published statement plus immutable publication and acknowledgement projections.
#[derive(
    Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct BillingPublishedStatementV1 {
    /// Exact checkpoint anchor for this read.
    pub anchor: HedgingBillingProjectionAnchorV1,
    /// Full signed governed statement requested by its owner.
    pub signed_statement: SignedGovernedBillingStatementV1,
    /// Exact publisher-signed immutable receipt.
    pub publisher_receipt: BillingStatementPublicationReceiptV1,
    /// Payload-free durable acknowledgement, when present.
    pub acknowledgement: Option<BillingStatementAcknowledgementProjectionV1>,
}
impl fmt::Debug for BillingPublishedStatementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BillingPublishedStatementV1")
            .field("anchor", &self.anchor)
            .field(
                "statement_id",
                &self
                    .signed_statement
                    .governed_statement
                    .statement
                    .statement_id,
            )
            .field(
                "publisher_receipt_digest",
                &self.publisher_receipt.receipt_digest,
            )
            .field("acknowledgement", &self.acknowledgement)
            .finish()
    }
}
/// Authenticated owner acknowledgement request.
#[derive(
    Clone,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
pub struct BillingStatementAcknowledgementRequestV1 {
    /// Exact checkpoint fingerprint observed by the caller.
    pub expected_checkpoint_fingerprint: [u8; 32],
    /// Statement being acknowledged.
    pub statement_id: [u8; 32],
    /// Exact canonical UTF-8 I105 account bytes.
    pub owner_account_id: Vec<u8>,
    /// Non-zero caller idempotency nonce.
    pub request_nonce: [u8; 32],
    /// Bounded proof consumed only by the acknowledgement authority.
    pub authentication_proof: Vec<u8>,
}
impl fmt::Debug for BillingStatementAcknowledgementRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BillingStatementAcknowledgementRequestV1")
            .field(
                "expected_checkpoint_fingerprint",
                &self.expected_checkpoint_fingerprint,
            )
            .field("statement_id", &self.statement_id)
            .field(
                "owner_account_digest",
                &blake3::hash(&self.owner_account_id),
            )
            .field("request_nonce", &self.request_nonce)
            .field("authentication_proof", &"[REDACTED]")
            .finish()
    }
}
/// Successful acknowledgement response without authentication proof bytes.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
pub struct BillingStatementAcknowledgementResponseV1 {
    /// Checkpoint anchor after the acknowledgement is durably reconciled.
    pub anchor: HedgingBillingProjectionAnchorV1,
    /// Payload-free acknowledgement projection.
    pub acknowledgement: BillingStatementAcknowledgementProjectionV1,
}
/// Common bounded projection page request for finance-owned aggregate reads.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
pub struct HedgingBillingProjectionPageRequestV1 {
    /// Exact checkpoint fingerprint observed by the caller.
    pub expected_checkpoint_fingerprint: [u8; 32],
    /// Exclusive 32-byte item cursor.
    pub after: Option<[u8; 32]>,
    /// Page size in `1..=100`.
    pub limit: u16,
}
/// Compact finalized exposure for one retained active-epoch period.
#[derive(
    Debug, Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct HedgingBillingExposureItemV1 {
    /// Opaque exclusive-page cursor.
    pub cursor: [u8; 32],
    /// Exact consensus-authenticated period-close digest.
    pub period_close_digest: [u8; 32],
    /// Exact finalized period end.
    pub period_end_unix: u64,
    /// Finalized cursor authenticating the close.
    pub finalized_cursor: HedgingBillingFinalizedCursorV1,
    /// Number of statements contributing to exposure.
    pub statement_count: u32,
    /// Exact aggregate net-due XOR, including zero/below-threshold periods.
    pub xor_exposure: XorQuantity,
    /// Whether the governed generation threshold was reached.
    pub hedge_threshold_reached: bool,
    /// Generated hedge intent, when the threshold was reached.
    pub hedge_intent_id: Option<[u8; 32]>,
    /// Automatic execution remains false for every V1 projection.
    pub automatic_execution: bool,
}
/// One bounded active-epoch exposure page.
#[derive(
    Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct HedgingBillingExposurePageV1 {
    /// Exact checkpoint anchor shared by every item.
    pub anchor: HedgingBillingProjectionAnchorV1,
    /// Compact finalized exposure items.
    pub items: Vec<HedgingBillingExposureItemV1>,
    /// Last returned opaque exposure cursor when another page exists.
    pub next_cursor: Option<[u8; 32]>,
}
impl fmt::Debug for HedgingBillingExposurePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HedgingBillingExposurePageV1")
            .field("anchor", &self.anchor)
            .field("item_count", &self.items.len())
            .field("next_cursor", &self.next_cursor)
            .finish()
    }
}
/// One bounded active-epoch hedge-intent page.
#[derive(
    Clone, PartialEq, Eq, DeriveNoritoSerialize, DeriveNoritoDeserialize, DeriveJsonSerialize,
)]
pub struct HedgeIntentPageV1 {
    /// Exact checkpoint anchor shared by every item.
    pub anchor: HedgingBillingProjectionAnchorV1,
    /// Compact generated intents; these contain no feed or statement envelopes.
    pub items: Vec<HedgeIntentV1>,
    /// Last returned intent id when another page exists.
    pub next_cursor: Option<[u8; 32]>,
    /// Unconditionally false for V1.
    pub automatic_execution_enabled: bool,
}
impl fmt::Debug for HedgeIntentPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HedgeIntentPageV1")
            .field("anchor", &self.anchor)
            .field("item_count", &self.items.len())
            .field("next_cursor", &self.next_cursor)
            .field(
                "automatic_execution_enabled",
                &self.automatic_execution_enabled,
            )
            .finish()
    }
}
/// Payload-free daemon health projection owned by the node API boundary.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
pub struct HedgingBillingDaemonStatusV1 {
    /// Exact projection anchor clients must bind subsequent reads to.
    pub anchor: HedgingBillingProjectionAnchorV1,
    /// Durable deterministic projector and delivery state.
    pub service: HedgingBillingServiceStatusV1,
    /// Whether at least one complete supervised reconciliation tick succeeded.
    pub live: bool,
    /// Whether all identity-pinned runtime dependencies passed their latest probe.
    pub external_dependencies_healthy: bool,
    /// Whether the latest complete bounded reconciliation tick succeeded.
    pub last_tick_healthy: bool,
    /// Whether the latest successful tick is within the fixed V1 freshness
    /// window derived from the configured poll interval.
    pub last_tick_fresh: bool,
    /// Whether at least one non-zero finalized block is represented.
    pub finalized_projection_ready: bool,
    /// Latest authenticated finalized head observed from the configured query.
    pub finalized_head_height: u64,
    /// Exact block distance between the observed head and projector cursor.
    pub finalized_lag_blocks: u64,
    /// Automatic hedge execution is unconditionally absent in V1.
    pub automatic_hedge_execution_enabled: bool,
    /// Overall supervised readiness.
    pub ready: bool,
}
/// Payload-free supervised-worker counters owned by the node API boundary.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
pub struct HedgingBillingDaemonMetricsV1 {
    /// Reconciliation ticks that completed without error.
    pub successful_ticks: u64,
    /// Reconciliation ticks that returned a typed error.
    pub failed_ticks: u64,
    /// Spawn-blocking tasks that panicked or were cancelled.
    pub panicked_ticks: u64,
    /// Finalized journal pages applied.
    pub finalized_pages_applied: u64,
    /// Finalized billing events applied.
    pub finalized_events_applied: u64,
    /// Finalized period closes applied.
    pub period_closes_applied: u64,
    /// Statements signed through the configured HSM/KMS.
    pub statements_signed: u64,
    /// Statements durably published.
    pub statements_published: u64,
    /// Ambiguous publications reconciled.
    pub publications_reconciled: u64,
    /// Account acknowledgements reconciled.
    pub acknowledgements_reconciled: u64,
}
/// Payload-free exact reconciliation view for supervision and Torii reads.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
    DeriveJsonSerialize,
)]
pub struct HedgingBillingReconciliationStatusV1 {
    /// Exact checkpoint observed when this projection was built.
    pub anchor: HedgingBillingProjectionAnchorV1,
    /// Whether the latest complete bounded tick succeeded.
    pub last_tick_healthy: bool,
    /// Successful reconciliation ticks.
    pub successful_ticks: u64,
    /// Failed reconciliation ticks.
    pub failed_ticks: u64,
    /// Current bounded count of non-terminal delivery records.
    pub pending_delivery_operations: u32,
}
/// Oracle-safe failures returned by the runtime API boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HedgingBillingRuntimeApiErrorV1 {
    /// Request shape, canonical account encoding, cursor, or bound is invalid.
    #[error("hedging/billing runtime request is invalid")]
    InvalidRequest,
    /// The exact checkpoint expected by the caller is no longer current.
    #[error("hedging/billing runtime projection changed")]
    ProjectionChanged,
    /// The statement is absent, not owned by the caller, or not terminally published.
    #[error("billing statement is unavailable to this owner")]
    StatementUnavailableToOwner,
    /// An immutable acknowledgement already exists with a different request binding.
    #[error("billing acknowledgement conflicts with retained state")]
    AcknowledgementConflict,
    /// A deterministic runtime resource bound was exhausted.
    #[error("hedging/billing runtime resource bound was exhausted")]
    ResourceExhausted,
    /// The supervised runtime cannot safely answer the request.
    #[error("hedging/billing runtime is unavailable")]
    Unavailable,
}
/// Object-safe production API implemented by the supervised `irohad` runtime.
///
/// Torii depends only on this node-owned boundary and never receives the raw billing service,
/// HSM/KMS adapters, publisher, or hedge-execution adapter. Projection methods, including
/// reconciliation status, must fail closed unless a live qualified finalized head proves the
/// retained projection fresh and remains stable through response construction. Payload-free daemon
/// health and metrics remain observable while the projection is unavailable.
pub trait HedgingBillingRuntimeApiV1: Send + Sync + fmt::Debug {
    /// Return the current exact checkpoint/finality anchor.
    fn projection_anchor(
        &self,
    ) -> Result<HedgingBillingProjectionAnchorV1, HedgingBillingRuntimeApiErrorV1>;
    /// Return one bounded owner-scoped terminal statement page.
    fn list_statements(
        &self,
        request: &BillingStatementListRequestV1,
    ) -> Result<BillingStatementPageV1, HedgingBillingRuntimeApiErrorV1>;
    /// Return one exact terminally published statement to its owner.
    fn published_statement(
        &self,
        request: &BillingPublishedStatementRequestV1,
    ) -> Result<BillingPublishedStatementV1, HedgingBillingRuntimeApiErrorV1>;
    /// Authenticate and durably acknowledge one owner statement under a fresh
    /// finalized-head fence that is rechecked immediately before local commit.
    fn acknowledge_statement(
        &self,
        request: &BillingStatementAcknowledgementRequestV1,
        server_time_unix: u64,
    ) -> Result<BillingStatementAcknowledgementResponseV1, HedgingBillingRuntimeApiErrorV1>;
    /// Return finalized active-epoch exposure, including below-threshold periods.
    fn exposure_page(
        &self,
        request: &HedgingBillingProjectionPageRequestV1,
    ) -> Result<HedgingBillingExposurePageV1, HedgingBillingRuntimeApiErrorV1>;
    /// Return generated active-epoch hedge intents without an execution surface.
    fn hedge_intent_page(
        &self,
        request: &HedgingBillingProjectionPageRequestV1,
    ) -> Result<HedgeIntentPageV1, HedgingBillingRuntimeApiErrorV1>;
    /// Return payload-free daemon health.
    fn daemon_status(
        &self,
    ) -> Result<HedgingBillingDaemonStatusV1, HedgingBillingRuntimeApiErrorV1>;
    /// Return payload-free monotonic daemon counters.
    fn daemon_metrics(&self) -> HedgingBillingDaemonMetricsV1;
    /// Return exact payload-free reconciliation status.
    fn reconciliation_status(
        &self,
    ) -> Result<HedgingBillingReconciliationStatusV1, HedgingBillingRuntimeApiErrorV1>;
}
/// Durable finalized-ledger billing and statement-delivery service.
pub struct HedgingBillingService {
    policy: HedgingBillingServicePolicyV1,
    feed_policy: HedgingFeedTrustPolicyV1,
    journal_verifier: Arc<dyn HedgingBillingJournalVerifier>,
    publisher: Arc<dyn BillingStatementPublisher>,
    acknowledgement_authority: Arc<dyn BillingStatementAcknowledgementAuthority>,
    epoch_witness_store: Arc<dyn HedgingBillingEpochWitnessStore>,
    store: AtomicCheckpointStore,
    state: Mutex<RuntimeState>,
}
impl fmt::Debug for HedgingBillingService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HedgingBillingService")
            .field("network_id", &self.policy.network_id)
            .field("policy_revision", &self.policy.revision)
            .field("runtime_state", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}
impl HedgingBillingService {
    /// Open or initialize a durable service checkpoint.
    ///
    /// Interrupted signer-only claims are reset to ready because the signer cannot publish.
    /// Interrupted publication remains ambiguous and requires sink lookup before retry.
    ///
    /// # Errors
    ///
    /// Fails closed on invalid policy, substituted feed policy, unsafe storage,
    /// noncanonical checkpoint bytes, policy mismatch, or invalid restart state.
    pub fn new(
        state_root: &Path,
        policy: HedgingBillingServicePolicyV1,
        feed_policy: HedgingFeedTrustPolicyV1,
        journal_verifier: Arc<dyn HedgingBillingJournalVerifier>,
        publisher: Arc<dyn BillingStatementPublisher>,
        acknowledgement_authority: Arc<dyn BillingStatementAcknowledgementAuthority>,
        epoch_witness_store: Arc<dyn HedgingBillingEpochWitnessStore>,
    ) -> Result<Self, HedgingBillingServiceError> {
        policy.validate()?;
        feed_policy.validate()?;
        if feed_policy.canonical_digest()? != policy.feed_trust_policy_digest {
            return Err(HedgingBillingServiceError::FeedPolicyMismatch);
        }
        if epoch_witness_store.handle() != policy.epoch_witness_store_handle {
            return Err(HedgingBillingServiceError::EpochWitnessIdentityMismatch);
        }
        let store = AtomicCheckpointStore::new(
            state_root,
            HEDGING_BILLING_CHECKPOINT_FILE_NAME_V1,
            HEDGING_BILLING_LOCK_FILE_NAME_V1,
            policy.checkpoint_max_bytes,
        )?;
        let (bytes, fingerprint) = store.load_bytes()?;
        let had_checkpoint = bytes.is_some();
        let witness = epoch_witness_store.load_latest()?;
        if let Some(record) = &witness {
            record.validate(policy.checkpoint_max_bytes)?;
        }
        let local_checkpoint = bytes
            .as_deref()
            .map(|bytes| decode_checkpoint(bytes, &policy, &feed_policy))
            .transpose();
        let mut epoch_recovered = false;
        let mut checkpoint = match (local_checkpoint, witness.as_ref()) {
            (Ok(Some(checkpoint)), Some(record)) => {
                if checkpoint.epoch_sequence > record.epoch_sequence {
                    return Err(HedgingBillingServiceError::EpochWitnessRollback);
                }
                if checkpoint.epoch_sequence == record.epoch_sequence {
                    validate_checkpoint_epoch_witness(
                        &checkpoint,
                        record,
                        &policy,
                        &feed_policy,
                        journal_verifier.as_ref(),
                        epoch_witness_store.as_ref(),
                    )?;
                    checkpoint
                } else {
                    epoch_recovered = true;
                    decode_and_validate_epoch_witness(
                        record,
                        &policy,
                        &feed_policy,
                        journal_verifier.as_ref(),
                        epoch_witness_store.as_ref(),
                    )?
                }
            }
            (Ok(Some(checkpoint)), None) => {
                if checkpoint.epoch_sequence != 0 {
                    return Err(HedgingBillingServiceError::EpochWitnessRollback);
                }
                checkpoint
            }
            (Ok(None), Some(record)) | (Err(_), Some(record)) => {
                epoch_recovered = true;
                decode_and_validate_epoch_witness(
                    record,
                    &policy,
                    &feed_policy,
                    journal_verifier.as_ref(),
                    epoch_witness_store.as_ref(),
                )?
            }
            (Ok(None), None) => HedgingBillingCheckpointV1::empty(&policy)?,
            (Err(error), None) => return Err(error),
        };
        let mut fingerprint = fingerprint;
        if epoch_recovered {
            let recovered_bytes = encode_checkpoint(&checkpoint, &policy, &feed_policy)?;
            fingerprint = Some(store.commit_bytes(&recovered_bytes, fingerprint)?);
        }
        checkpoint.verify_authenticated_sources(&policy, journal_verifier.as_ref())?;
        verify_publisher_identity(&policy.statement_publisher, publisher.as_ref())?;
        let recovered = checkpoint
            .statements
            .iter_mut()
            .fold(false, |changed, record| {
                if record.state == StoredStatementDeliveryStateV1::Signing {
                    record.state = if record.signing_attempts >= policy.max_signing_attempts {
                        StoredStatementDeliveryStateV1::DeadLetter
                    } else {
                        StoredStatementDeliveryStateV1::ReadyForSigning
                    };
                    record.signing_claim_cursor = None;
                    true
                } else {
                    changed
                }
            });
        checkpoint.validate(&policy, &feed_policy)?;
        let reconciled = reconcile_authoritative_delivery_state(
            &mut checkpoint,
            &policy,
            &feed_policy,
            publisher.as_ref(),
            acknowledgement_authority.as_ref(),
        )?;
        checkpoint.validate(&policy, &feed_policy)?;
        let fingerprint = if (!had_checkpoint && !epoch_recovered) || recovered || reconciled {
            let encoded = encode_checkpoint(&checkpoint, &policy, &feed_policy)?;
            Some(store.commit_bytes(&encoded, fingerprint)?)
        } else {
            fingerprint
        };
        Ok(Self {
            policy,
            feed_policy,
            journal_verifier,
            publisher,
            acknowledgement_authority,
            epoch_witness_store,
            store,
            state: Mutex::new(RuntimeState {
                checkpoint,
                fingerprint,
            }),
        })
    }
    /// Ingest one typed, contiguous, finalized billing-journal page.
    ///
    /// # Errors
    ///
    /// Rejects gaps, forks, timestamp rollback, late events for an already closed period, semantic
    /// replay, resource exhaustion, and persistence uncertainty without changing in-memory state.
    pub fn ingest_finalized_page(
        &self,
        page: &HedgingBillingFinalizedEventPageV1,
    ) -> Result<HedgingBillingIngestOutcomeV1, HedgingBillingServiceError> {
        let page_digest = page.validate(&self.policy)?;
        let (observed_commitment, verification_predecessor) = {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
            let current = guard.checkpoint.journal_commitment;
            let predecessor = if page.start_sequence < guard.checkpoint.next_event_sequence {
                let index = guard
                    .checkpoint
                    .source_pages
                    .iter()
                    .position(|retained| {
                        retained.start_sequence == page.start_sequence
                            && retained.next_sequence == page.next_sequence
                    })
                    .ok_or(HedgingBillingServiceError::FinalizedSequenceRollback)?;
                index
                    .checked_sub(1)
                    .and_then(|previous| guard.checkpoint.source_pages.get(previous))
                    .map(|previous| previous.journal_commitment)
            } else {
                current
            };
            (current, predecessor)
        };
        self.journal_verifier.verify_page(
            &self.policy.network_id,
            verification_predecessor,
            page,
        )?;
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        if guard.checkpoint.journal_commitment != observed_commitment {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        if page.events.is_empty()
            && page.start_sequence == guard.checkpoint.next_event_sequence
            && guard.checkpoint.journal_commitment == Some(page.journal_commitment)
        {
            return Ok(HedgingBillingIngestOutcomeV1::Replay {
                next_sequence: guard.checkpoint.next_event_sequence,
            });
        }
        if page.start_sequence < guard.checkpoint.next_event_sequence {
            if guard.checkpoint.last_page_digest == Some(page_digest) {
                return Ok(HedgingBillingIngestOutcomeV1::Replay {
                    next_sequence: guard.checkpoint.next_event_sequence,
                });
            }
            if page.next_sequence > guard.checkpoint.next_event_sequence
                || page.events.is_empty()
                || guard.checkpoint.finalized_cursor.is_some_and(|current| {
                    page.journal_commitment.finalized_cursor.height > current.height
                        || (page.journal_commitment.finalized_cursor.height == current.height
                            && page.journal_commitment.finalized_cursor.block_hash
                                != current.block_hash)
                })
            {
                return Err(HedgingBillingServiceError::FinalizedSequenceRollback);
            }
            for event in &page.events {
                let retained = guard
                    .checkpoint
                    .event_replay_receipts
                    .binary_search_by_key(&event.sequence, |receipt| receipt.sequence)
                    .ok()
                    .and_then(|index| guard.checkpoint.event_replay_receipts.get(index))
                    .ok_or(HedgingBillingServiceError::FinalizedSequenceRollback)?;
                if retained.event_digest != event_replay_digest(self.policy.network_id, event)? {
                    return Err(HedgingBillingServiceError::FinalizedEventEquivocation);
                }
            }
            return Ok(HedgingBillingIngestOutcomeV1::Replay {
                next_sequence: guard.checkpoint.next_event_sequence,
            });
        }
        if page.start_sequence != guard.checkpoint.next_event_sequence {
            return Err(HedgingBillingServiceError::FinalizedSequenceGap);
        }
        if let Some(previous) = guard.checkpoint.finalized_cursor {
            if page.journal_commitment.finalized_cursor.height < previous.height
                || page.journal_commitment.finalized_cursor.finalized_at_unix
                    < previous.finalized_at_unix
                || (page.journal_commitment.finalized_cursor.height == previous.height
                    && page.journal_commitment.finalized_cursor.block_hash != previous.block_hash)
            {
                return Err(HedgingBillingServiceError::FinalizedForkOrRollback);
            }
        }
        if guard.checkpoint.journal_commitment.is_some_and(|previous| {
            page.journal_commitment.journal_next_sequence < previous.journal_next_sequence
                || (page.journal_commitment.journal_next_sequence == previous.journal_next_sequence
                    && page.journal_commitment.journal_root != previous.journal_root)
                || (page.journal_commitment.journal_next_sequence > previous.journal_next_sequence
                    && page.journal_commitment.journal_root == previous.journal_root)
        }) {
            return Err(HedgingBillingServiceError::FinalizedForkOrRollback);
        }
        let resulting_accruals = guard
            .checkpoint
            .open_accruals
            .len()
            .checked_add(page.events.len())
            .ok_or(HedgingBillingServiceError::ResourceExhausted)?;
        let resulting_receipts = guard
            .checkpoint
            .replay_receipts
            .len()
            .checked_add(page.events.len())
            .ok_or(HedgingBillingServiceError::ResourceExhausted)?;
        let resulting_event_receipts = guard
            .checkpoint
            .event_replay_receipts
            .len()
            .checked_add(page.events.len())
            .ok_or(HedgingBillingServiceError::ResourceExhausted)?;
        if resulting_accruals > usize::try_from(self.policy.max_open_accruals).unwrap_or(usize::MAX)
            || resulting_receipts
                > usize::try_from(self.policy.max_replay_receipts).unwrap_or(usize::MAX)
            || resulting_event_receipts
                > usize::try_from(self.policy.max_replay_receipts).unwrap_or(usize::MAX)
            || guard.checkpoint.source_pages.len()
                >= usize::try_from(self.policy.max_retained_source_pages).unwrap_or(usize::MAX)
        {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        let mut next = guard.checkpoint.clone();
        let mut receipts: BTreeSet<[u8; 32]> = next.replay_receipts.iter().copied().collect();
        for event in &page.events {
            if event.occurred_at_unix < next.last_period_end_unix {
                return Err(HedgingBillingServiceError::LateFinalizedEvent);
            }
            let receipt = source_receipt(self.policy.network_id, event)?;
            if !receipts.insert(receipt) {
                return Err(HedgingBillingServiceError::DuplicateBillingSource);
            }
            next.open_accruals.push(StoredAccrualV1 {
                event: event.clone(),
                source_receipt: receipt,
            });
            next.event_replay_receipts.push(StoredEventReplayReceiptV1 {
                sequence: event.sequence,
                event_digest: event_replay_digest(self.policy.network_id, event)?,
            });
        }
        next.open_accruals
            .sort_by_key(|accrual| accrual.event.sequence);
        next.replay_receipts = receipts.into_iter().collect();
        next.event_replay_receipts.sort();
        next.next_event_sequence = page.next_sequence;
        next.finalized_cursor = Some(page.journal_commitment.finalized_cursor);
        next.journal_commitment = Some(page.journal_commitment);
        next.last_page_digest = Some(page_digest);
        next.source_pages.push(page.clone());
        let next_sequence = next.next_event_sequence;
        self.commit_locked(guard, next)?;
        Ok(HedgingBillingIngestOutcomeV1::Applied {
            event_count: page.events.len(),
            next_sequence,
        })
    }
    /// Return the exact durable position for a finalized native query.
    ///
    /// # Errors
    ///
    /// Fails when the runtime state lock is poisoned.
    pub fn query_position(
        &self,
    ) -> Result<HedgingBillingQueryPositionV1, HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        Ok(HedgingBillingQueryPositionV1 {
            next_sequence: guard.checkpoint.next_event_sequence,
            journal_commitment: guard.checkpoint.journal_commitment,
        })
    }
    /// Consume a bounded sequence of authoritative finalized query pages.
    ///
    /// A scan stops on `None`, an exact replay, or a page that advances only finality without
    /// advancing the event sequence. This prevents a faulty adapter from spinning on an empty page.
    ///
    /// # Errors
    ///
    /// Rejects an invalid page limit, propagates fixed query failures, and
    /// fail-closes on every ingest or persistence invariant.
    pub fn reconcile_finalized_query(
        &self,
        query: &dyn HedgingBillingFinalizedQuery,
        max_pages: u32,
        finalized_head: HedgingBillingFinalizedCursorV1,
    ) -> Result<HedgingBillingReconcileOutcomeV1, HedgingBillingServiceError> {
        if max_pages == 0 || max_pages > HEDGING_BILLING_MAX_PAGES_PER_SCAN_V1 {
            return Err(HedgingBillingServiceError::InvalidQueryBound);
        }
        finalized_head.validate()?;
        let mut pages_applied = 0_u32;
        let mut events_applied = 0_u64;
        for _ in 0..max_pages {
            let before = self.query_position()?;
            let Some(page) = query.query_finalized_page(before, self.policy.max_events_per_page)?
            else {
                break;
            };
            if !finalized_cursor_at_or_before(
                page.journal_commitment.finalized_cursor,
                finalized_head,
            ) {
                return Err(HedgingBillingServiceError::FinalizedForkOrRollback);
            }
            let outcome = self.ingest_finalized_page(&page)?;
            match outcome {
                HedgingBillingIngestOutcomeV1::Replay { .. } => break,
                HedgingBillingIngestOutcomeV1::Applied { event_count, .. } => {
                    pages_applied = pages_applied
                        .checked_add(1)
                        .ok_or(HedgingBillingServiceError::AmountOverflow)?;
                    events_applied = events_applied
                        .checked_add(
                            u64::try_from(event_count)
                                .map_err(|_| HedgingBillingServiceError::AmountOverflow)?,
                        )
                        .ok_or(HedgingBillingServiceError::AmountOverflow)?;
                    let after = self.query_position()?;
                    if after.next_sequence == before.next_sequence {
                        break;
                    }
                }
            }
        }
        let position = self.query_position()?;
        Ok(HedgingBillingReconcileOutcomeV1 {
            pages_applied,
            events_applied,
            next_sequence: position.next_sequence,
            finalized_cursor: position
                .journal_commitment
                .map(|commitment| commitment.finalized_cursor),
        })
    }
    /// Finalize the next fixed billing period from committed accruals.
    ///
    /// The governed reference price must be effective exactly at the next
    /// period boundary and retain externally authenticated feed envelopes.
    ///
    /// # Errors
    ///
    /// Rejects skipped periods, incomplete finality, unauthenticated pricing,
    /// invalid statement transitions, credit underflow, resource exhaustion,
    /// and persistence uncertainty. Exposure above the venue ceiling produces
    /// a non-executable governed overflow projection instead of blocking close.
    pub fn finalize_next_period(
        &self,
        close: &HedgingBillingFinalizedPeriodCloseV1,
    ) -> Result<HedgingBillingPeriodOutcomeV1, HedgingBillingServiceError> {
        close.validate(&self.policy, &self.feed_policy)?;
        self.journal_verifier
            .verify_period_close(&self.policy.network_id, close)?;
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        if let Some(existing_close) = guard
            .checkpoint
            .period_closes
            .iter()
            .find(|existing| existing.period_end_unix == close.period_end_unix)
        {
            if existing_close != close {
                return Err(HedgingBillingServiceError::InvalidPeriodClose);
            }
            return period_outcome_from_checkpoint(&guard.checkpoint, close.period_end_unix);
        }
        let expected_period_end = guard
            .checkpoint
            .last_period_end_unix
            .checked_add(self.policy.billing_period_secs)
            .ok_or(HedgingBillingServiceError::AmountOverflow)?;
        if close.period_end_unix != expected_period_end
            || guard.checkpoint.journal_commitment != Some(close.journal_commitment)
            || guard.checkpoint.next_event_sequence
                != close.journal_commitment.journal_next_sequence
        {
            return Err(HedgingBillingServiceError::InvalidPeriodClose);
        }
        if guard.checkpoint.period_closes.len()
            >= usize::try_from(self.policy.max_retained_period_closes).unwrap_or(usize::MAX)
        {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        let source_events: Vec<HedgingBillingFinalizedEventV1> = guard
            .checkpoint
            .source_pages
            .iter()
            .flat_map(|page| page.events.iter().cloned())
            .collect();
        let mut closes = guard.checkpoint.period_closes.clone();
        closes.push(close.clone());
        let derived = derive_billing_domain_state(
            &self.policy,
            &self.feed_policy,
            &source_events,
            &closes,
            &guard.checkpoint.compacted_account_bases,
        )?;
        if derived.account_heads.len()
            > usize::try_from(self.policy.max_accounts).unwrap_or(usize::MAX)
            || derived.governed_statements.len()
                > usize::try_from(self.policy.max_statements).unwrap_or(usize::MAX)
            || derived.hedge_intents.len()
                > usize::try_from(self.policy.max_hedge_intents).unwrap_or(usize::MAX)
        {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        let existing: BTreeMap<[u8; 32], StoredStatementV1> = guard
            .checkpoint
            .statements
            .iter()
            .cloned()
            .map(|record| (record.governed_statement.statement.statement_id, record))
            .collect();
        let mut statements = Vec::new();
        statements
            .try_reserve_exact(derived.governed_statements.len())
            .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?;
        for governed_statement in &derived.governed_statements {
            let statement_id = governed_statement.statement.statement_id;
            if let Some(record) = existing.get(&statement_id) {
                if record.governed_statement != *governed_statement {
                    return Err(HedgingBillingServiceError::InvalidCheckpoint);
                }
                statements.push(record.clone());
            } else {
                statements.push(StoredStatementV1 {
                    governed_statement: governed_statement.clone(),
                    state: StoredStatementDeliveryStateV1::ReadyForSigning,
                    signing_attempts: 0,
                    signing_claim_cursor: None,
                    signed_statement: None,
                    publication_receipt: None,
                });
            }
        }
        statements.sort_by_key(|record| record.governed_statement.statement.statement_id);
        let statement_ids: Vec<[u8; 32]> = derived
            .governed_statements
            .iter()
            .filter(|governed| governed.statement.period_end_unix == close.period_end_unix)
            .map(|governed| governed.statement.statement_id)
            .collect();
        let statement_bundle_digest =
            hash_canonical(b"sorafs.billing.statement-bundle.v1", &statement_ids)?;
        let hedge_intent = derived
            .hedge_intents
            .iter()
            .find(|intent| intent.period_end_unix == close.period_end_unix)
            .cloned();
        let mut next = guard.checkpoint.clone();
        next.period_closes = closes;
        next.last_period_end_unix = close.period_end_unix;
        next.open_accruals = derived.open_accruals;
        next.account_heads = derived.account_heads;
        next.statements = statements;
        next.hedge_intents = derived.hedge_intents;
        self.commit_locked(guard, next)?;
        Ok(HedgingBillingPeriodOutcomeV1 {
            period_end_unix: close.period_end_unix,
            statement_ids,
            statement_bundle_digest,
            hedge_intent,
        })
    }
    /// Compact a fully settled epoch and install an explicitly governed policy successor.
    ///
    /// This consumes the service instance. Every statement in the compacted epoch must already have
    /// an authoritative acknowledgement, the journal must be exactly at the last authenticated
    /// close, and no open accrual may cross the frontier. The sealed witness is appended before the
    /// local checkpoint replacement, allowing restart to recover either side of a crash without
    /// accepting rollback.
    ///
    /// # Errors
    ///
    /// Rejects unsettled economic state, incomplete or skipped frontiers,
    /// policy forks/rollback, signer substitution, invalid consensus proof,
    /// witness-store fork/rollback, capacity failure, and uncertain durability.
    pub fn transition_epoch(
        self,
        next_policy: HedgingBillingServicePolicyV1,
        next_feed_policy: HedgingFeedTrustPolicyV1,
        consensus_authentication_proof: Vec<u8>,
        signer: &dyn BillingStatementRuntimeSigner,
    ) -> Result<HedgingBillingEpochTransitionOutcomeV1, HedgingBillingServiceError> {
        next_policy.validate()?;
        next_feed_policy.validate()?;
        if next_feed_policy.canonical_digest()? != next_policy.feed_trust_policy_digest {
            return Err(HedgingBillingServiceError::FeedPolicyMismatch);
        }
        if consensus_authentication_proof.is_empty()
            || consensus_authentication_proof.len() > HEDGING_BILLING_CONSENSUS_PROOF_MAX_BYTES_V1
        {
            return Err(HedgingBillingServiceError::InvalidEpochTransition);
        }
        let identity = signer.identity()?;
        if identity.signer_id != self.policy.transition_authority.authority_id
            || identity.public_key != self.policy.transition_authority.public_key
            || checked_verifying_key(identity.public_key).is_err()
        {
            return Err(HedgingBillingServiceError::SignerIdentityMismatch);
        }
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        let checkpoint = &guard.checkpoint;
        let close = checkpoint
            .period_closes
            .last()
            .ok_or(HedgingBillingServiceError::InvalidEpochTransition)?;
        if checkpoint.last_period_end_unix <= checkpoint.compacted_through_period_end_unix
            || checkpoint.journal_commitment != Some(close.journal_commitment)
            || checkpoint.next_event_sequence != close.journal_commitment.journal_next_sequence
            || !checkpoint.open_accruals.is_empty()
            || checkpoint.source_pages.is_empty()
            || checkpoint.source_pages.last().is_none_or(|page| {
                page.next_sequence != close.journal_commitment.journal_next_sequence
                    || page.journal_commitment != close.journal_commitment
            })
            || checkpoint
                .statements
                .iter()
                .any(|record| record.state != StoredStatementDeliveryStateV1::Acknowledged)
            || checkpoint.statements.len() != checkpoint.acknowledgements.len()
        {
            return Err(HedgingBillingServiceError::UnsettledEpochTransition);
        }
        let mut account_bases: BTreeMap<Vec<u8>, GovernedBillingStatementV1> = checkpoint
            .compacted_account_bases
            .iter()
            .cloned()
            .map(|governed| (governed.statement.account_id.clone(), governed))
            .collect();
        for record in &checkpoint.statements {
            let account_id = record.governed_statement.statement.account_id.clone();
            let period_end_unix = record.governed_statement.statement.period_end_unix;
            let replace = account_bases
                .get(&account_id)
                .is_none_or(|existing| existing.statement.period_end_unix < period_end_unix);
            if replace {
                account_bases.insert(account_id, record.governed_statement.clone());
            } else if account_bases.get(&account_id).is_some_and(|existing| {
                existing.statement.period_end_unix == period_end_unix
                    && existing != &record.governed_statement
            }) {
                return Err(HedgingBillingServiceError::InvalidEpochTransition);
            }
        }
        let compacted_account_bases: Vec<GovernedBillingStatementV1> =
            account_bases.into_values().collect();
        if compacted_account_bases.len()
            > usize::try_from(next_policy.max_accounts).unwrap_or(usize::MAX)
        {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        let compacted_source_digest = hash_canonical(
            COMPACTED_SOURCE_DOMAIN_V1,
            &CompactedSourceArchiveV1 {
                network_id: checkpoint.network_id,
                predecessor_compacted_journal_commitment: checkpoint.compacted_journal_commitment,
                predecessor_compacted_through_period_end_unix: checkpoint
                    .compacted_through_period_end_unix,
                source_pages: checkpoint.source_pages.clone(),
                period_closes: checkpoint.period_closes.clone(),
                replay_receipts: checkpoint.replay_receipts.clone(),
                event_replay_receipts: checkpoint.event_replay_receipts.clone(),
            },
        )?;
        let compacted_economic_state_digest = hash_canonical(
            COMPACTED_ECONOMIC_STATE_DOMAIN_V1,
            &CompactedEconomicArchiveV1 {
                network_id: checkpoint.network_id,
                predecessor_account_bases: checkpoint.compacted_account_bases.clone(),
                statements: checkpoint.statements.clone(),
                acknowledgements: checkpoint.acknowledgements.clone(),
                hedge_intents: checkpoint.hedge_intents.clone(),
            },
        )?;
        let compacted_account_bases_digest = hash_canonical(
            b"sorafs.hedging-billing.compacted-account-bases.v1",
            &compacted_account_bases,
        )?;
        let compacted_counts = HedgingBillingCompactionCountsV1 {
            source_pages: u64::try_from(checkpoint.source_pages.len())
                .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?,
            source_events: checkpoint
                .source_pages
                .iter()
                .try_fold(0_u64, |count, page| {
                    count
                        .checked_add(
                            u64::try_from(page.events.len())
                                .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?,
                        )
                        .ok_or(HedgingBillingServiceError::ResourceExhausted)
                })?,
            period_closes: u64::try_from(checkpoint.period_closes.len())
                .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?,
            statements: u64::try_from(checkpoint.statements.len())
                .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?,
            acknowledgements: u64::try_from(checkpoint.acknowledgements.len())
                .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?,
            hedge_intents: u64::try_from(checkpoint.hedge_intents.len())
                .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?,
        };
        let epoch_sequence = checkpoint
            .epoch_sequence
            .checked_add(1)
            .ok_or(HedgingBillingServiceError::InvalidEpochTransition)?;
        let mut next_checkpoint = checkpoint.clone();
        next_checkpoint.policy_digest = next_policy.digest()?;
        next_checkpoint.epoch_sequence = epoch_sequence;
        next_checkpoint.compacted_journal_commitment = Some(close.journal_commitment);
        next_checkpoint.compacted_through_period_end_unix = close.period_end_unix;
        next_checkpoint.compacted_account_bases = compacted_account_bases;
        next_checkpoint.latest_epoch_transition = None;
        next_checkpoint.last_page_digest = None;
        next_checkpoint.source_pages.clear();
        next_checkpoint.period_closes.clear();
        next_checkpoint.open_accruals.clear();
        next_checkpoint.replay_receipts.clear();
        next_checkpoint.event_replay_receipts.clear();
        next_checkpoint.statements.clear();
        next_checkpoint.acknowledgements.clear();
        next_checkpoint.hedge_intents.clear();
        next_checkpoint.account_heads = next_checkpoint
            .compacted_account_bases
            .iter()
            .map(|governed| StoredAccountStatementHeadV1 {
                account_id: governed.statement.account_id.clone(),
                statement_id: governed.statement.statement_id,
                period_end_unix: governed.statement.period_end_unix,
            })
            .collect();
        let retained_epoch_state_digest = retained_epoch_state_digest(&next_checkpoint)?;
        let mut transition = HedgingBillingEpochTransitionV1 {
            version: HEDGING_BILLING_EPOCH_TRANSITION_VERSION_V1,
            epoch_sequence,
            predecessor_epoch_sequence: checkpoint.epoch_sequence,
            predecessor_transition_digest: checkpoint
                .latest_epoch_transition
                .as_ref()
                .map(|transition| transition.transition_id),
            predecessor_journal_commitment: close.journal_commitment,
            compacted_through_period_end_unix: close.period_end_unix,
            next_source_sequence: close.journal_commitment.journal_next_sequence,
            compacted_counts,
            compacted_source_digest,
            compacted_economic_state_digest,
            compacted_account_bases_digest,
            retained_epoch_state_digest,
            previous_service_policy: self.policy.clone(),
            next_service_policy: next_policy.clone(),
            previous_feed_policy: self.feed_policy.clone(),
            next_feed_policy: next_feed_policy.clone(),
            transition_id: [0; 32],
            authority_id: self.policy.transition_authority.authority_id.clone(),
            signature: [0; 64],
            consensus_authentication_proof,
        };
        transition.transition_id = transition.transition_digest()?;
        let signature =
            signer.sign_digest(epoch_transition_signature_digest(transition.transition_id))?;
        let identity_after = signer.identity()?;
        if identity_after != identity {
            return Err(HedgingBillingServiceError::SignerIdentityMismatch);
        }
        transition.signature = signature;
        transition.verify()?;
        self.journal_verifier
            .verify_epoch_transition(&self.policy.network_id, &transition)?;
        next_checkpoint.latest_epoch_transition = Some(transition.clone());
        next_checkpoint.validate(&next_policy, &next_feed_policy)?;
        let checkpoint_bytes =
            encode_checkpoint(&next_checkpoint, &next_policy, &next_feed_policy)?;
        let next_witness = HedgingBillingEpochWitnessRecordV1::new(
            self.policy.network_id,
            epoch_sequence,
            transition.transition_id,
            checkpoint_bytes.clone(),
        );
        next_witness.validate(next_policy.checkpoint_max_bytes)?;
        let previous_witness = self.epoch_witness_store.load_latest()?;
        let expected_revision = match (&previous_witness, checkpoint.epoch_sequence) {
            (None, 0) => None,
            (Some(previous), epoch)
                if previous.epoch_sequence == epoch
                    && checkpoint
                        .latest_epoch_transition
                        .as_ref()
                        .map(|transition| transition.transition_id)
                        == Some(previous.transition_id) =>
            {
                previous.validate(self.policy.checkpoint_max_bytes)?;
                validate_checkpoint_epoch_witness(
                    checkpoint,
                    previous,
                    &self.policy,
                    &self.feed_policy,
                    self.journal_verifier.as_ref(),
                    self.epoch_witness_store.as_ref(),
                )?;
                Some(previous.revision)
            }
            _ => return Err(HedgingBillingServiceError::EpochWitnessFork),
        };
        match self
            .epoch_witness_store
            .compare_and_swap_latest(expected_revision, &next_witness)
        {
            Ok(()) => {}
            Err(HedgingBillingExternalError::Ambiguous) => {
                if self.epoch_witness_store.load_latest()?.as_ref() != Some(&next_witness) {
                    return Err(HedgingBillingServiceError::External(
                        HedgingBillingExternalError::Ambiguous,
                    ));
                }
            }
            Err(error) => return Err(HedgingBillingServiceError::External(error)),
        }
        if self.epoch_witness_store.load_latest()?.as_ref() != Some(&next_witness)
            || self
                .epoch_witness_store
                .load_epoch(epoch_sequence)?
                .as_ref()
                != Some(&next_witness)
        {
            return Err(HedgingBillingServiceError::InvalidEpochWitness);
        }
        self.store
            .commit_bytes(&checkpoint_bytes, guard.fingerprint)?;
        Ok(HedgingBillingEpochTransitionOutcomeV1 {
            epoch_sequence,
            transition,
            witness_revision: next_witness.revision,
        })
    }
    /// Sign the first ready statement with a runtime-only HSM/KMS provider.
    ///
    /// Identity is checked before the durable claim, immediately before the signing call, and after
    /// the call. The produced signature is verified locally before it is persisted.
    ///
    /// # Errors
    ///
    /// Fails closed on signer drift, inactive/revoked policy, invalid output,
    /// retry exhaustion, concurrent mutation, or persistence uncertainty.
    pub fn sign_next_statement(
        &self,
        signer: &dyn BillingStatementRuntimeSigner,
    ) -> Result<Option<SignedGovernedBillingStatementV1>, HedgingBillingServiceError> {
        self.sign_statement_selected(None, signer)
    }
    /// Sign one exact ready statement selected by the supervised fair scanner.
    ///
    /// # Errors
    ///
    /// Fails when the statement is absent or no longer ready, as well as for every signer, policy,
    /// persistence, and concurrency failure documented by [`Self::sign_next_statement`].
    pub fn sign_statement(
        &self,
        statement_id: [u8; 32],
        signer: &dyn BillingStatementRuntimeSigner,
    ) -> Result<SignedGovernedBillingStatementV1, HedgingBillingServiceError> {
        self.sign_statement_selected(Some(statement_id), signer)?
            .ok_or(HedgingBillingServiceError::InvalidDeliveryState)
    }
    fn sign_statement_selected(
        &self,
        requested_statement_id: Option<[u8; 32]>,
        signer: &dyn BillingStatementRuntimeSigner,
    ) -> Result<Option<SignedGovernedBillingStatementV1>, HedgingBillingServiceError> {
        self.verify_runtime_signer_identity(signer)?;
        let (statement_id, mut candidate, period_close) = {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
            let index = if let Some(statement_id) = requested_statement_id {
                guard
                    .checkpoint
                    .statements
                    .iter()
                    .position(|record| {
                        record.governed_statement.statement.statement_id == statement_id
                            && record.state == StoredStatementDeliveryStateV1::ReadyForSigning
                    })
                    .ok_or(HedgingBillingServiceError::InvalidDeliveryState)?
            } else {
                let Some(index) = guard.checkpoint.statements.iter().position(|record| {
                    record.state == StoredStatementDeliveryStateV1::ReadyForSigning
                }) else {
                    return Ok(None);
                };
                index
            };
            let mut next = guard.checkpoint.clone();
            let record = &mut next.statements[index];
            let period_close = guard
                .checkpoint
                .period_closes
                .iter()
                .find(|close| {
                    close.period_end_unix == record.governed_statement.statement.period_end_unix
                })
                .cloned()
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            let cursor = period_close.journal_commitment.finalized_cursor;
            if !self.policy.statement_signer.active_at(cursor.height) {
                return Err(HedgingBillingServiceError::SignerInactive);
            }
            if record.signing_attempts >= self.policy.max_signing_attempts {
                return Err(HedgingBillingServiceError::SigningRetryExhausted);
            }
            record.signing_attempts = record
                .signing_attempts
                .checked_add(1)
                .ok_or(HedgingBillingServiceError::SigningRetryExhausted)?;
            record.state = StoredStatementDeliveryStateV1::Signing;
            record.signing_claim_cursor = Some(cursor);
            let candidate = SignedGovernedBillingStatementV1 {
                version: SIGNED_GOVERNED_BILLING_STATEMENT_VERSION_V1,
                network_id: self.policy.network_id,
                billing_policy_digest: self.policy.billing_policy_digest,
                service_policy_digest: self.policy.digest()?,
                signer_id: self.policy.statement_signer.signer_id.clone(),
                signed_at_unix: cursor.finalized_at_unix,
                finalized_cursor: cursor,
                period_close_digest: period_close.close_digest(&self.policy, &self.feed_policy)?,
                feed_admitted_at_unix: period_close.feed_admitted_at_unix,
                governed_statement: record.governed_statement.clone(),
                signature: [0; 64],
            };
            let candidate_bytes = norito::to_bytes(&candidate)
                .map_err(|_| HedgingBillingServiceError::EncodingFailed)?;
            if candidate_bytes.len() > SIGNED_GOVERNED_BILLING_STATEMENT_MAX_BYTES_V1 {
                return Err(HedgingBillingServiceError::ResourceExhausted);
            }
            let statement_id = record.governed_statement.statement.statement_id;
            self.commit_locked(guard, next)?;
            (statement_id, candidate, period_close)
        };
        self.verify_runtime_signer_identity(signer)?;
        let signature = match signer.sign_digest(candidate.signing_digest()?) {
            Ok(signature) => signature,
            Err(error) => {
                self.release_failed_signing_claim(statement_id)?;
                return Err(HedgingBillingServiceError::External(error));
            }
        };
        if let Err(error) = self.verify_runtime_signer_identity(signer) {
            self.release_failed_signing_claim(statement_id)?;
            return Err(error);
        }
        candidate.signature = signature;
        if let Err(error) = candidate.verify(&self.policy, &self.feed_policy, &period_close) {
            self.release_failed_signing_claim(statement_id)?;
            return Err(error);
        }
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        let mut next = guard.checkpoint.clone();
        let record = find_statement_mut(&mut next, statement_id)?;
        if record.state != StoredStatementDeliveryStateV1::Signing
            || record.signing_claim_cursor != Some(candidate.finalized_cursor)
            || record.governed_statement != candidate.governed_statement
        {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        record.state = StoredStatementDeliveryStateV1::ReadyForPublication;
        record.signing_claim_cursor = None;
        record.signed_statement = Some(candidate.clone());
        self.commit_locked(guard, next)?;
        Ok(Some(candidate))
    }
    /// Publish the first signed statement through an authenticated immutable sink.
    ///
    /// The checkpoint enters the ambiguous state before bytes leave the process. Ambiguous sink
    /// failures therefore require lookup, never blind resubmission.
    ///
    /// # Errors
    ///
    /// Returns fixed external failures or rejects malformed receipts and persistence uncertainty.
    pub fn publish_next_statement(
        &self,
    ) -> Result<Option<BillingStatementPublicationReceiptV1>, HedgingBillingServiceError> {
        self.publish_statement_selected(None)
    }
    /// Publish one exact signed statement selected by the supervised fair scanner.
    ///
    /// # Errors
    ///
    /// Fails when the statement is absent or no longer ready, as well as for
    /// every publisher, receipt, persistence, and reconciliation failure
    /// documented by [`Self::publish_next_statement`].
    pub fn publish_statement(
        &self,
        statement_id: [u8; 32],
    ) -> Result<BillingStatementPublicationReceiptV1, HedgingBillingServiceError> {
        self.publish_statement_selected(Some(statement_id))?
            .ok_or(HedgingBillingServiceError::InvalidDeliveryState)
    }
    fn publish_statement_selected(
        &self,
        requested_statement_id: Option<[u8; 32]>,
    ) -> Result<Option<BillingStatementPublicationReceiptV1>, HedgingBillingServiceError> {
        verify_publisher_identity(&self.policy.statement_publisher, self.publisher.as_ref())?;
        let (statement_id, signed) = {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
            let index = if let Some(statement_id) = requested_statement_id {
                guard
                    .checkpoint
                    .statements
                    .iter()
                    .position(|record| {
                        record.governed_statement.statement.statement_id == statement_id
                            && record.state == StoredStatementDeliveryStateV1::ReadyForPublication
                    })
                    .ok_or(HedgingBillingServiceError::InvalidDeliveryState)?
            } else {
                let Some(index) = guard.checkpoint.statements.iter().position(|record| {
                    record.state == StoredStatementDeliveryStateV1::ReadyForPublication
                }) else {
                    return Ok(None);
                };
                index
            };
            let record = &guard.checkpoint.statements[index];
            let signed = record
                .signed_statement
                .clone()
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            let close = guard
                .checkpoint
                .period_closes
                .iter()
                .find(|close| {
                    close.period_end_unix == record.governed_statement.statement.period_end_unix
                })
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            signed.verify(&self.policy, &self.feed_policy, close)?;
            let statement_id = record.governed_statement.statement.statement_id;
            (statement_id, signed)
        };
        let signed_statement_digest = signed_statement_digest(&signed)?;
        if let Some(publication) = self.publisher.lookup(statement_id)? {
            if publication.signed_statement != signed {
                return Err(HedgingBillingServiceError::AuthoritativeReconciliationMismatch);
            }
            publication
                .receipt
                .validate(&signed, &self.policy.statement_publisher)?;
            self.store_publication_receipt(statement_id, publication.receipt.clone())?;
            return Ok(Some(publication.receipt));
        }
        self.mark_publication_ambiguous(statement_id, &signed)?;
        let receipt = match self
            .publisher
            .publish(statement_id, signed_statement_digest, &signed)
        {
            Ok(receipt) => receipt,
            Err(HedgingBillingExternalError::Unavailable) => {
                self.mark_publication_absent(statement_id)?;
                return Err(HedgingBillingServiceError::External(
                    HedgingBillingExternalError::Unavailable,
                ));
            }
            Err(error) => return Err(HedgingBillingServiceError::External(error)),
        };
        receipt.validate(&signed, &self.policy.statement_publisher)?;
        self.store_publication_receipt(statement_id, receipt.clone())?;
        Ok(Some(receipt))
    }
    /// Reconcile one ambiguous publication through authoritative sink lookup.
    ///
    /// # Errors
    ///
    /// Returns a fixed external failure or rejects a conflicting receipt.
    pub fn reconcile_ambiguous_publication(
        &self,
        statement_id: [u8; 32],
    ) -> Result<Option<BillingStatementPublicationReceiptV1>, HedgingBillingServiceError> {
        let signed = {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
            let record = find_statement(&guard.checkpoint, statement_id)?;
            if record.state != StoredStatementDeliveryStateV1::PublicationAmbiguous {
                return Err(HedgingBillingServiceError::InvalidDeliveryState);
            }
            record
                .signed_statement
                .clone()
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?
        };
        match self.publisher.lookup(statement_id)? {
            Some(publication) => {
                if publication.signed_statement != signed {
                    return Err(HedgingBillingServiceError::AuthoritativeReconciliationMismatch);
                }
                publication
                    .receipt
                    .validate(&signed, &self.policy.statement_publisher)?;
                self.store_publication_receipt(statement_id, publication.receipt.clone())?;
                Ok(Some(publication.receipt))
            }
            None => {
                self.mark_publication_absent(statement_id)?;
                Ok(None)
            }
        }
    }
    /// Durably acknowledge a published statement under a canonical authenticated request binding.
    ///
    /// # Errors
    ///
    /// Rejects unknown/unpublished statements, account substitution, replay
    /// conflict, timestamp rollback, and persistence uncertainty.
    pub fn acknowledge_statement(
        &self,
        statement_id: [u8; 32],
        account_id: &[u8],
        request_binding_digest: [u8; 32],
        acknowledged_at_unix: u64,
        authentication_proof: Vec<u8>,
    ) -> Result<BillingStatementAcknowledgementV1, HedgingBillingServiceError> {
        self.acknowledge_statement_at_fingerprint(
            statement_id,
            account_id,
            request_binding_digest,
            acknowledged_at_unix,
            authentication_proof,
            None,
        )
    }
    fn acknowledge_statement_at_fingerprint(
        &self,
        statement_id: [u8; 32],
        account_id: &[u8],
        request_binding_digest: [u8; 32],
        acknowledged_at_unix: u64,
        authentication_proof: Vec<u8>,
        expected_checkpoint_fingerprint: Option<[u8; 32]>,
    ) -> Result<BillingStatementAcknowledgementV1, HedgingBillingServiceError> {
        let mut allow_commit = || Ok(());
        self.acknowledge_statement_at_fingerprint_with_precommit_fence(
            statement_id,
            account_id,
            request_binding_digest,
            acknowledged_at_unix,
            authentication_proof,
            expected_checkpoint_fingerprint,
            &mut allow_commit,
        )
    }
    fn acknowledge_statement_at_fingerprint_with_precommit_fence(
        &self,
        statement_id: [u8; 32],
        account_id: &[u8],
        request_binding_digest: [u8; 32],
        acknowledged_at_unix: u64,
        authentication_proof: Vec<u8>,
        expected_checkpoint_fingerprint: Option<[u8; 32]>,
        pre_commit_fence: &mut dyn FnMut() -> Result<(), HedgingBillingServiceError>,
    ) -> Result<BillingStatementAcknowledgementV1, HedgingBillingServiceError> {
        if validate_canonical_account_id_bytes(account_id).is_err()
            || statement_id == [0; 32]
            || request_binding_digest == [0; 32]
            || acknowledged_at_unix == 0
            || acknowledged_at_unix == u64::MAX
            || authentication_proof.is_empty()
            || authentication_proof.len() > BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1
        {
            return Err(HedgingBillingServiceError::InvalidAcknowledgement);
        }
        let signed = {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
            if expected_checkpoint_fingerprint
                .is_some_and(|expected| guard.fingerprint != Some(expected))
            {
                return Err(HedgingBillingServiceError::ProjectionAnchorConflict);
            }
            let record = find_statement(&guard.checkpoint, statement_id)?;
            if !matches!(
                record.state,
                StoredStatementDeliveryStateV1::Published
                    | StoredStatementDeliveryStateV1::Acknowledged
            ) || record.governed_statement.statement.account_id != account_id
            {
                return Err(HedgingBillingServiceError::InvalidAcknowledgement);
            }
            let publication = record
                .publication_receipt
                .as_ref()
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            if acknowledged_at_unix < publication.published_at_unix {
                return Err(HedgingBillingServiceError::InvalidAcknowledgement);
            }
            if let Some(existing) = guard
                .checkpoint
                .acknowledgements
                .iter()
                .find(|existing| existing.statement_id == statement_id)
            {
                if existing.account_digest == *blake3::hash(account_id).as_bytes()
                    && existing.request_binding_digest == request_binding_digest
                {
                    return Ok(existing.clone());
                }
                return Err(HedgingBillingServiceError::AcknowledgementConflict);
            }
            if guard.checkpoint.acknowledgements.len()
                >= usize::try_from(self.policy.max_acknowledgements).unwrap_or(usize::MAX)
            {
                return Err(HedgingBillingServiceError::ResourceExhausted);
            }
            (
                record
                    .signed_statement
                    .clone()
                    .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?,
                guard.fingerprint,
            )
        };
        let (signed, preflight_fingerprint) = signed;
        let mut acknowledgement = BillingStatementAcknowledgementV1 {
            version: BILLING_STATEMENT_ACKNOWLEDGEMENT_VERSION_V1,
            network_id: self.policy.network_id,
            statement_id,
            account_digest: *blake3::hash(account_id).as_bytes(),
            request_binding_digest,
            acknowledged_at_unix,
            authentication_proof,
            acknowledgement_id: [0; 32],
        };
        acknowledgement.acknowledgement_id = acknowledgement_digest(&acknowledgement)?;
        self.acknowledgement_authority
            .verify(&signed, &acknowledgement)?;
        pre_commit_fence()?;
        let authoritative = self
            .acknowledgement_authority
            .record(&signed, &acknowledgement)?;
        if authoritative != acknowledgement {
            return Err(HedgingBillingServiceError::AcknowledgementConflict);
        }
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        if guard.fingerprint != preflight_fingerprint {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        if let Some(existing) = guard
            .checkpoint
            .acknowledgements
            .iter()
            .find(|existing| existing.statement_id == statement_id)
        {
            if existing == &acknowledgement {
                return Ok(existing.clone());
            }
            return Err(HedgingBillingServiceError::AcknowledgementConflict);
        }
        let record = find_statement(&guard.checkpoint, statement_id)?;
        if !matches!(
            record.state,
            StoredStatementDeliveryStateV1::Published
                | StoredStatementDeliveryStateV1::Acknowledged
        ) || record.signed_statement.as_ref() != Some(&signed)
        {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        if guard.checkpoint.acknowledgements.len()
            >= usize::try_from(self.policy.max_acknowledgements).unwrap_or(usize::MAX)
        {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        let mut next = guard.checkpoint.clone();
        next.acknowledgements.push(acknowledgement.clone());
        next.acknowledgements
            .sort_by_key(|entry| entry.acknowledgement_id);
        find_statement_mut(&mut next, statement_id)?.state =
            StoredStatementDeliveryStateV1::Acknowledged;
        self.commit_locked_with_precommit_fence(guard, next, pre_commit_fence)?;
        Ok(acknowledgement)
    }
    /// Reconcile one statement acknowledgement from its authoritative service.
    ///
    /// # Errors
    ///
    /// Fails closed on authority unavailability, invalid authentication,
    /// conflicting local state, or persistence uncertainty.
    pub fn reconcile_acknowledgement(
        &self,
        statement_id: [u8; 32],
    ) -> Result<Option<BillingStatementAcknowledgementV1>, HedgingBillingServiceError> {
        self.reconcile_acknowledgement_at_fingerprint(statement_id, None)
    }
    fn reconcile_acknowledgement_at_fingerprint(
        &self,
        statement_id: [u8; 32],
        expected_checkpoint_fingerprint: Option<[u8; 32]>,
    ) -> Result<Option<BillingStatementAcknowledgementV1>, HedgingBillingServiceError> {
        let mut allow_commit = || Ok(());
        self.reconcile_acknowledgement_at_fingerprint_with_precommit_fence(
            statement_id,
            expected_checkpoint_fingerprint,
            &mut allow_commit,
        )
    }
    fn reconcile_acknowledgement_at_fingerprint_with_precommit_fence(
        &self,
        statement_id: [u8; 32],
        expected_checkpoint_fingerprint: Option<[u8; 32]>,
        pre_commit_fence: &mut dyn FnMut() -> Result<(), HedgingBillingServiceError>,
    ) -> Result<Option<BillingStatementAcknowledgementV1>, HedgingBillingServiceError> {
        let Some(acknowledgement) = self.acknowledgement_authority.lookup(statement_id)? else {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
            if expected_checkpoint_fingerprint
                .is_some_and(|expected| guard.fingerprint != Some(expected))
            {
                return Err(HedgingBillingServiceError::ProjectionAnchorConflict);
            }
            let record = find_statement(&guard.checkpoint, statement_id)?;
            if record.state == StoredStatementDeliveryStateV1::Acknowledged {
                return Err(HedgingBillingServiceError::AuthoritativeReconciliationMismatch);
            }
            return Ok(None);
        };
        let (signed, preflight_fingerprint) = {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
            if expected_checkpoint_fingerprint
                .is_some_and(|expected| guard.fingerprint != Some(expected))
            {
                return Err(HedgingBillingServiceError::ProjectionAnchorConflict);
            }
            let record = find_statement(&guard.checkpoint, statement_id)?;
            validate_acknowledgement_record(record, &acknowledgement, self.policy.network_id)?;
            (
                record
                    .signed_statement
                    .clone()
                    .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?,
                guard.fingerprint,
            )
        };
        self.acknowledgement_authority
            .verify(&signed, &acknowledgement)?;
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        if guard.fingerprint != preflight_fingerprint {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        if expected_checkpoint_fingerprint
            .is_some_and(|expected| guard.fingerprint != Some(expected))
        {
            return Err(HedgingBillingServiceError::ProjectionAnchorConflict);
        }
        let record = find_statement(&guard.checkpoint, statement_id)?;
        validate_acknowledgement_record(record, &acknowledgement, self.policy.network_id)?;
        if record.signed_statement.as_ref() != Some(&signed) {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        if let Some(existing) = guard
            .checkpoint
            .acknowledgements
            .iter()
            .find(|existing| existing.statement_id == statement_id)
        {
            if existing == &acknowledgement {
                return Ok(Some(existing.clone()));
            }
            return Err(HedgingBillingServiceError::AcknowledgementConflict);
        }
        if guard.checkpoint.acknowledgements.len()
            >= usize::try_from(self.policy.max_acknowledgements).unwrap_or(usize::MAX)
        {
            return Err(HedgingBillingServiceError::ResourceExhausted);
        }
        let mut next = guard.checkpoint.clone();
        next.acknowledgements.push(acknowledgement.clone());
        next.acknowledgements
            .sort_by_key(|entry| entry.acknowledgement_id);
        find_statement_mut(&mut next, statement_id)?.state =
            StoredStatementDeliveryStateV1::Acknowledged;
        self.commit_locked_with_precommit_fence(guard, next, pre_commit_fence)?;
        Ok(Some(acknowledgement))
    }
    /// Return the exact active-epoch projection anchor.
    ///
    /// # Errors
    ///
    /// Returns a fixed unavailable class if durable state cannot be inspected.
    pub fn api_projection_anchor(
        &self,
    ) -> Result<HedgingBillingProjectionAnchorV1, HedgingBillingRuntimeApiErrorV1> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        projection_anchor(&guard, &self.policy)
    }
    /// Return one exact anchor and its payload-free service counters under the
    /// same state-lock snapshot.
    pub fn api_anchored_service_status(
        &self,
    ) -> Result<
        (
            HedgingBillingProjectionAnchorV1,
            HedgingBillingServiceStatusV1,
        ),
        HedgingBillingRuntimeApiErrorV1,
    > {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        let anchor = projection_anchor(&guard, &self.policy)?;
        let status =
            service_status(&guard.checkpoint, &self.policy).map_err(runtime_api_service_error)?;
        Ok((anchor, status))
    }
    /// Return a bounded owner-scoped terminal statement page.
    ///
    /// Records in signing, publication-ambiguous, retry, or dead-letter states
    /// are intentionally indistinguishable from absent records to this API.
    pub fn api_list_statements(
        &self,
        request: &BillingStatementListRequestV1,
    ) -> Result<BillingStatementPageV1, HedgingBillingRuntimeApiErrorV1> {
        validate_runtime_page_bound(request.limit)?;
        validate_canonical_account_id_bytes(&request.owner_account_id)
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::InvalidRequest)?;
        if request.expected_checkpoint_fingerprint == [0; 32]
            || request.after_statement_id == Some([0; 32])
        {
            return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
        }
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        let anchor = projection_anchor(&guard, &self.policy)?;
        require_projection_fingerprint(&anchor, request.expected_checkpoint_fingerprint)?;
        let start = if let Some(after) = request.after_statement_id {
            let index = guard
                .checkpoint
                .statements
                .binary_search_by_key(&after, |record| {
                    record.governed_statement.statement.statement_id
                })
                .map_err(|_| HedgingBillingRuntimeApiErrorV1::InvalidRequest)?;
            let cursor_record = guard
                .checkpoint
                .statements
                .get(index)
                .ok_or(HedgingBillingRuntimeApiErrorV1::InvalidRequest)?;
            if cursor_record.governed_statement.statement.account_id != request.owner_account_id
                || !matches!(
                    cursor_record.state,
                    StoredStatementDeliveryStateV1::Published
                        | StoredStatementDeliveryStateV1::Acknowledged
                )
            {
                return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
            }
            index
                .checked_add(1)
                .ok_or(HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?
        } else {
            0
        };
        let wanted = usize::from(request.limit);
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(wanted.saturating_add(1))
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?;
        for record in guard.checkpoint.statements.iter().skip(start) {
            if record.governed_statement.statement.account_id != request.owner_account_id
                || !matches!(
                    record.state,
                    StoredStatementDeliveryStateV1::Published
                        | StoredStatementDeliveryStateV1::Acknowledged
                )
            {
                continue;
            }
            let signed = record
                .signed_statement
                .as_ref()
                .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?;
            let receipt = record
                .publication_receipt
                .as_ref()
                .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?;
            let statement = &record.governed_statement.statement;
            selected.push(BillingStatementListItemV1 {
                statement_id: statement.statement_id,
                period_start_unix: statement.period_start_unix,
                period_end_unix: statement.period_end_unix,
                due_at_unix: statement.due_at_unix,
                net_due_xor: statement.net_due_xor.clone(),
                signed_statement_digest: receipt.signed_statement_digest,
                publication_receipt_digest: receipt.receipt_digest,
                status: match record.state {
                    StoredStatementDeliveryStateV1::Published => {
                        BillingStatementOwnerStatusV1::Published
                    }
                    StoredStatementDeliveryStateV1::Acknowledged => {
                        BillingStatementOwnerStatusV1::Acknowledged
                    }
                    _ => return Err(HedgingBillingRuntimeApiErrorV1::Unavailable),
                },
                acknowledgement_id: None,
            });
            if signed.governed_statement.statement.statement_id != statement.statement_id {
                return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
            }
            if selected.len() > wanted {
                break;
            }
        }
        let has_more = selected.len() > wanted;
        if has_more {
            selected.pop();
        }
        if selected
            .iter()
            .any(|item| item.status == BillingStatementOwnerStatusV1::Acknowledged)
        {
            let selected_index: BTreeMap<[u8; 32], usize> = selected
                .iter()
                .enumerate()
                .map(|(index, item)| (item.statement_id, index))
                .collect();
            for acknowledgement in &guard.checkpoint.acknowledgements {
                if let Some(index) = selected_index.get(&acknowledgement.statement_id) {
                    selected[*index].acknowledgement_id = Some(acknowledgement.acknowledgement_id);
                }
            }
            if selected.iter().any(|item| {
                item.status == BillingStatementOwnerStatusV1::Acknowledged
                    && item.acknowledgement_id.is_none()
            }) {
                return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
            }
        }
        let next_cursor = if has_more {
            selected.last().map(|item| item.statement_id)
        } else {
            None
        };
        Ok(BillingStatementPageV1 {
            anchor,
            owner_account_digest: *blake3::hash(&request.owner_account_id).as_bytes(),
            items: selected,
            next_cursor,
        })
    }
    /// Return one exact published statement and payload-free delivery evidence
    /// to its canonical owner.
    pub fn api_published_statement(
        &self,
        request: &BillingPublishedStatementRequestV1,
    ) -> Result<BillingPublishedStatementV1, HedgingBillingRuntimeApiErrorV1> {
        validate_canonical_account_id_bytes(&request.owner_account_id)
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::InvalidRequest)?;
        if request.statement_id == [0; 32] || request.expected_checkpoint_fingerprint == [0; 32] {
            return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
        }
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        let anchor = projection_anchor(&guard, &self.policy)?;
        require_projection_fingerprint(&anchor, request.expected_checkpoint_fingerprint)?;
        let record = terminal_owned_statement(
            &guard.checkpoint,
            request.statement_id,
            &request.owner_account_id,
        )?;
        let signed_statement = record
            .signed_statement
            .clone()
            .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        let publisher_receipt = record
            .publication_receipt
            .clone()
            .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        let acknowledgement = guard
            .checkpoint
            .acknowledgements
            .iter()
            .find(|acknowledgement| acknowledgement.statement_id == request.statement_id)
            .map(acknowledgement_projection);
        if (record.state == StoredStatementDeliveryStateV1::Acknowledged)
            != acknowledgement.is_some()
        {
            return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
        }
        Ok(BillingPublishedStatementV1 {
            anchor,
            signed_statement,
            publisher_receipt,
            acknowledgement,
        })
    }
    /// Authenticate and durably acknowledge one owner statement at an exact checkpoint anchor.
    ///
    /// The statement, canonical account, and idempotency nonce form the
    /// domain-separated request binding. The bounded proof authenticates that
    /// binding and is never copied into the response projection.
    pub fn api_acknowledge_statement(
        &self,
        request: &BillingStatementAcknowledgementRequestV1,
        server_time_unix: u64,
    ) -> Result<BillingStatementAcknowledgementResponseV1, HedgingBillingRuntimeApiErrorV1> {
        let mut allow_commit = || Ok(());
        self.api_acknowledge_statement_with_precommit_fence(
            request,
            server_time_unix,
            &mut allow_commit,
        )
    }
    /// Authenticate and durably acknowledge one owner statement while invoking an external
    /// authority fence immediately before every durable local checkpoint store write.
    ///
    /// # Errors
    ///
    /// Returns the same bounded runtime API errors as [`Self::api_acknowledge_statement`], and maps
    /// a failed fence to the caller-supplied service error without changing the local checkpoint.
    pub fn api_acknowledge_statement_with_precommit_fence(
        &self,
        request: &BillingStatementAcknowledgementRequestV1,
        server_time_unix: u64,
        pre_commit_fence: &mut dyn FnMut() -> Result<(), HedgingBillingServiceError>,
    ) -> Result<BillingStatementAcknowledgementResponseV1, HedgingBillingRuntimeApiErrorV1> {
        validate_canonical_account_id_bytes(&request.owner_account_id)
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::InvalidRequest)?;
        if request.expected_checkpoint_fingerprint == [0; 32]
            || request.statement_id == [0; 32]
            || request.request_nonce == [0; 32]
            || server_time_unix == 0
            || server_time_unix == u64::MAX
            || request.authentication_proof.is_empty()
            || request.authentication_proof.len() > BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1
        {
            return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
        }
        {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
            let anchor = projection_anchor(&guard, &self.policy)?;
            require_projection_fingerprint(&anchor, request.expected_checkpoint_fingerprint)?;
            let record = terminal_owned_statement(
                &guard.checkpoint,
                request.statement_id,
                &request.owner_account_id,
            )?;
            let published_at_unix = record
                .publication_receipt
                .as_ref()
                .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?
                .published_at_unix;
            if server_time_unix < published_at_unix {
                return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
            }
        }
        let request_binding_digest = billing_statement_acknowledgement_request_digest_v1(
            request.statement_id,
            &request.owner_account_id,
            request.request_nonce,
        )?;
        if let Some(authoritative) = self
            .reconcile_acknowledgement_at_fingerprint_with_precommit_fence(
                request.statement_id,
                Some(request.expected_checkpoint_fingerprint),
                pre_commit_fence,
            )
            .map_err(runtime_api_acknowledgement_error)?
        {
            if authoritative.account_digest != *blake3::hash(&request.owner_account_id).as_bytes()
                || authoritative.request_binding_digest != request_binding_digest
            {
                return Err(HedgingBillingRuntimeApiErrorV1::AcknowledgementConflict);
            }
            let signed = {
                let guard = self
                    .state
                    .lock()
                    .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
                terminal_owned_statement(
                    &guard.checkpoint,
                    request.statement_id,
                    &request.owner_account_id,
                )?
                .signed_statement
                .clone()
                .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?
            };
            let mut retry_authentication = authoritative.clone();
            retry_authentication.authentication_proof = request.authentication_proof.clone();
            retry_authentication.acknowledgement_id =
                acknowledgement_digest(&retry_authentication).map_err(runtime_api_service_error)?;
            self.acknowledgement_authority
                .verify(&signed, &retry_authentication)
                .map_err(|error| {
                    runtime_api_acknowledgement_error(HedgingBillingServiceError::External(error))
                })?;
            return Ok(BillingStatementAcknowledgementResponseV1 {
                anchor: self.api_projection_anchor()?,
                acknowledgement: acknowledgement_projection(&authoritative),
            });
        }
        let acknowledgement = self
            .acknowledge_statement_at_fingerprint_with_precommit_fence(
                request.statement_id,
                &request.owner_account_id,
                request_binding_digest,
                server_time_unix,
                request.authentication_proof.clone(),
                Some(request.expected_checkpoint_fingerprint),
                pre_commit_fence,
            )
            .map_err(runtime_api_acknowledgement_error)?;
        let anchor = self.api_projection_anchor()?;
        Ok(BillingStatementAcknowledgementResponseV1 {
            anchor,
            acknowledgement: acknowledgement_projection(&acknowledgement),
        })
    }
    /// Return a bounded active-epoch exposure page, including finalized periods
    /// whose exposure is zero or below the hedge threshold.
    pub fn api_exposure_page(
        &self,
        request: &HedgingBillingProjectionPageRequestV1,
    ) -> Result<HedgingBillingExposurePageV1, HedgingBillingRuntimeApiErrorV1> {
        validate_projection_page_request(request)?;
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        let anchor = projection_anchor(&guard, &self.policy)?;
        require_projection_fingerprint(&anchor, request.expected_checkpoint_fingerprint)?;
        let start = projection_close_start(
            &guard.checkpoint.period_closes,
            request.after,
            &self.policy,
            &self.feed_policy,
        )?;
        let wanted = usize::from(request.limit);
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(wanted.saturating_add(1))
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?;
        for close in guard
            .checkpoint
            .period_closes
            .iter()
            .skip(start)
            .take(wanted.saturating_add(1))
        {
            selected.push((
                close,
                close
                    .close_digest(&self.policy, &self.feed_policy)
                    .map_err(runtime_api_service_error)?,
            ));
        }
        let has_more = selected.len() > wanted;
        if has_more {
            selected.pop();
        }
        let mut aggregate: BTreeMap<u64, (u32, XorQuantity)> = selected
            .iter()
            .map(|(close, _)| (close.period_end_unix, (0, XorQuantity::zero())))
            .collect();
        for record in &guard.checkpoint.statements {
            let statement = &record.governed_statement.statement;
            if let Some((count, exposure)) = aggregate.get_mut(&statement.period_end_unix) {
                *count = count
                    .checked_add(1)
                    .ok_or(HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?;
                *exposure = exposure
                    .checked_add(&statement.net_due_xor)
                    .map_err(|_| HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?;
            }
        }
        let mut items = Vec::new();
        items
            .try_reserve_exact(selected.len())
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?;
        for (close, period_close_digest) in selected {
            let (statement_count, xor_exposure) = aggregate
                .remove(&close.period_end_unix)
                .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?;
            let hedge_intent_id = guard
                .checkpoint
                .hedge_intents
                .iter()
                .find(|intent| intent.period_end_unix == close.period_end_unix)
                .map(|intent| intent.intent_id);
            let hedge_threshold_reached = xor_exposure >= self.policy.hedge_intent_threshold_xor;
            if hedge_threshold_reached != hedge_intent_id.is_some() {
                return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
            }
            items.push(HedgingBillingExposureItemV1 {
                cursor: exposure_cursor(close.period_end_unix, period_close_digest),
                period_close_digest,
                period_end_unix: close.period_end_unix,
                finalized_cursor: close.journal_commitment.finalized_cursor,
                statement_count,
                xor_exposure,
                hedge_threshold_reached,
                hedge_intent_id,
                automatic_execution: false,
            });
        }
        let next_cursor = if has_more {
            items.last().map(|item| item.cursor)
        } else {
            None
        };
        Ok(HedgingBillingExposurePageV1 {
            anchor,
            items,
            next_cursor,
        })
    }
    /// Return a bounded active-epoch hedge-intent page.
    pub fn api_hedge_intent_page(
        &self,
        request: &HedgingBillingProjectionPageRequestV1,
    ) -> Result<HedgeIntentPageV1, HedgingBillingRuntimeApiErrorV1> {
        validate_projection_page_request(request)?;
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::Unavailable)?;
        let anchor = projection_anchor(&guard, &self.policy)?;
        require_projection_fingerprint(&anchor, request.expected_checkpoint_fingerprint)?;
        let start = if let Some(after) = request.after {
            guard
                .checkpoint
                .hedge_intents
                .binary_search_by_key(&after, |intent| intent.intent_id)
                .map_err(|_| HedgingBillingRuntimeApiErrorV1::InvalidRequest)?
                .checked_add(1)
                .ok_or(HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?
        } else {
            0
        };
        let wanted = usize::from(request.limit);
        let mut items: Vec<_> = guard
            .checkpoint
            .hedge_intents
            .iter()
            .skip(start)
            .take(wanted.saturating_add(1))
            .cloned()
            .collect();
        let has_more = items.len() > wanted;
        if has_more {
            items.pop();
        }
        if items.iter().any(|intent| intent.automatic_execution) {
            return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
        }
        let next_cursor = if has_more {
            items.last().map(|intent| intent.intent_id)
        } else {
            None
        };
        Ok(HedgeIntentPageV1 {
            anchor,
            items,
            next_cursor,
            automatic_execution_enabled: false,
        })
    }
    /// Return at most `limit` actionable statement-delivery projections without
    /// cloning the retained statement inventory.
    ///
    /// # Errors
    ///
    /// Rejects zero or excessive limits and fails on a poisoned state lock.
    pub fn pending_statement_delivery_projections(
        &self,
        limit: u32,
    ) -> Result<Vec<BillingStatementDeliveryProjectionV1>, HedgingBillingServiceError> {
        self.pending_statement_delivery_projections_rotated(limit, 0)
    }
    /// Return at most `limit` actionable statement-delivery projections using
    /// a bounded fair scan rotated by `scan_sequence`.
    ///
    /// The fixed stage order puts ambiguous-write reconciliation before new publication, but the
    /// starting stage and record rotate on every worker attempt. Each non-empty stage contributes
    /// at most one record per round, preventing a persistent published or signing backlog from
    /// starving the other stages.
    ///
    /// # Errors
    ///
    /// Rejects zero or excessive limits and fails on a poisoned state lock.
    pub fn pending_statement_delivery_projections_rotated(
        &self,
        limit: u32,
        scan_sequence: u64,
    ) -> Result<Vec<BillingStatementDeliveryProjectionV1>, HedgingBillingServiceError> {
        if limit == 0 || limit > HEDGING_BILLING_MAX_DELIVERY_WORK_ITEMS_V1 {
            return Err(HedgingBillingServiceError::InvalidQueryBound);
        }
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        let maximum = usize::try_from(limit).unwrap_or(usize::MAX);
        let mut projections = Vec::new();
        projections
            .try_reserve_exact(maximum)
            .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?;
        if guard.checkpoint.statements.is_empty() {
            return Ok(projections);
        }
        let states = [
            StoredStatementDeliveryStateV1::PublicationAmbiguous,
            StoredStatementDeliveryStateV1::ReadyForPublication,
            StoredStatementDeliveryStateV1::ReadyForSigning,
            StoredStatementDeliveryStateV1::Published,
        ];
        let record_count = guard.checkpoint.statements.len();
        let record_start =
            usize::try_from(scan_sequence % u64::try_from(record_count).unwrap_or(u64::MAX))
                .unwrap_or(0);
        let mut stage_records = Vec::new();
        stage_records
            .try_reserve_exact(states.len())
            .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?;
        for state in states {
            let mut records = Vec::new();
            records
                .try_reserve_exact(record_count)
                .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?;
            for offset in 0..record_count {
                let index = record_start
                    .checked_add(offset)
                    .map(|value| value % record_count)
                    .ok_or(HedgingBillingServiceError::ResourceExhausted)?;
                let record = &guard.checkpoint.statements[index];
                if record.state == state {
                    records.push(record);
                }
            }
            if !records.is_empty() {
                stage_records.push(records);
            }
        }
        if stage_records.is_empty() {
            return Ok(projections);
        }
        let stage_count = stage_records.len();
        let stage_start =
            usize::try_from(scan_sequence % u64::try_from(stage_count).unwrap_or(u64::MAX))
                .unwrap_or(0);
        let mut stage_offsets = vec![0_usize; stage_count];
        while projections.len() < maximum {
            let mut advanced = false;
            for offset in 0..stage_count {
                let stage = stage_start
                    .checked_add(offset)
                    .map(|value| value % stage_count)
                    .ok_or(HedgingBillingServiceError::ResourceExhausted)?;
                let position = stage_offsets[stage];
                if let Some(record) = stage_records[stage].get(position) {
                    projections.push(statement_delivery_projection(record));
                    stage_offsets[stage] = position
                        .checked_add(1)
                        .ok_or(HedgingBillingServiceError::ResourceExhausted)?;
                    advanced = true;
                    if projections.len() == maximum {
                        return Ok(projections);
                    }
                }
            }
            if !advanced {
                break;
            }
        }
        Ok(projections)
    }
    /// Return payload-free statement delivery projections.
    ///
    /// # Errors
    ///
    /// Fails when the runtime state lock is poisoned.
    pub fn statement_delivery_projections(
        &self,
    ) -> Result<Vec<BillingStatementDeliveryProjectionV1>, HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        Ok(guard
            .checkpoint
            .statements
            .iter()
            .map(statement_delivery_projection)
            .collect())
    }
    /// Return bounded payload-free durable state for supervision and metrics.
    ///
    /// # Errors
    ///
    /// Fails on a poisoned state lock, timestamp overflow, or a retained count
    /// outside the configured V1 bounds.
    pub fn status(&self) -> Result<HedgingBillingServiceStatusV1, HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        service_status(&guard.checkpoint, &self.policy)
    }
    /// Return one exact published signed statement.
    ///
    /// # Errors
    ///
    /// Rejects unknown or unpublished statement identities.
    pub fn published_statement(
        &self,
        statement_id: [u8; 32],
    ) -> Result<SignedGovernedBillingStatementV1, HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        let record = find_statement(&guard.checkpoint, statement_id)?;
        if !matches!(
            record.state,
            StoredStatementDeliveryStateV1::Published
                | StoredStatementDeliveryStateV1::Acknowledged
        ) {
            return Err(HedgingBillingServiceError::InvalidDeliveryState);
        }
        record
            .signed_statement
            .clone()
            .ok_or(HedgingBillingServiceError::InvalidCheckpoint)
    }
    /// Return generated, never-automatically-executed hedge intents.
    ///
    /// # Errors
    ///
    /// Fails when the runtime state lock is poisoned.
    pub fn hedge_intents(&self) -> Result<Vec<HedgeIntentV1>, HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        Ok(guard.checkpoint.hedge_intents.clone())
    }
    /// Validate that a supervised execution adapter cannot auto-execute.
    ///
    /// # Errors
    ///
    /// Rejects an invalid venue identity or any adapter enabling automatic execution in V1.
    pub fn validate_execution_adapter(
        policy: &GovernedHedgeExecutionPolicyV1,
        adapter: &dyn GovernedHedgeExecutionAdapter,
    ) -> Result<(), HedgingBillingServiceError> {
        policy.validate()?;
        let identity = adapter.identity()?;
        if identity.venue_id != policy.venue_id
            || identity.public_key != policy.venue_public_key
            || checked_verifying_key(identity.public_key).is_err()
        {
            return Err(HedgingBillingServiceError::InvalidExecutionAdapter);
        }
        if adapter.automatic_execution_enabled() {
            return Err(HedgingBillingServiceError::AutomaticHedgeExecutionForbidden);
        }
        Ok(())
    }
    /// Explicitly submit one retained executable intent under operator authority.
    ///
    /// This method is never called by a worker or timer. It authenticates the
    /// adapter and authorization, performs authoritative lookup before write,
    /// and uses the authorization identity as the exact idempotency key.
    ///
    /// # Errors
    ///
    /// Rejects unknown/overflow intents, substituted policies or venues, forged/expired
    /// authorizations and receipts, automatic adapters, and fixed external failures.
    pub fn submit_authorized_hedge_intent(
        &self,
        execution_policy: &GovernedHedgeExecutionPolicyV1,
        authorization: &HedgeExecutionAuthorizationV1,
        adapter: &dyn GovernedHedgeExecutionAdapter,
    ) -> Result<HedgeExecutionSubmissionReceiptV1, HedgingBillingServiceError> {
        Self::validate_execution_adapter(execution_policy, adapter)?;
        let intent = {
            let guard = self
                .state
                .lock()
                .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
            guard
                .checkpoint
                .hedge_intents
                .iter()
                .find(|intent| intent.intent_id == authorization.intent_id)
                .cloned()
                .ok_or(HedgingBillingServiceError::HedgeIntentNotFound)?
        };
        authorization.verify(&self.policy, execution_policy, &intent)?;
        if let Some(receipt) = adapter.lookup_authorization(authorization.authorization_id)? {
            receipt.validate(execution_policy, authorization)?;
            return Ok(receipt);
        }
        let receipt =
            adapter.submit_authorized(authorization.authorization_id, &intent, authorization)?;
        receipt.validate(execution_policy, authorization)?;
        Ok(receipt)
    }
    fn verify_runtime_signer_identity(
        &self,
        signer: &dyn BillingStatementRuntimeSigner,
    ) -> Result<(), HedgingBillingServiceError> {
        let identity = signer.identity()?;
        if identity.signer_id != self.policy.statement_signer.signer_id
            || identity.public_key != self.policy.statement_signer.public_key
            || checked_verifying_key(identity.public_key).is_err()
        {
            return Err(HedgingBillingServiceError::SignerIdentityMismatch);
        }
        Ok(())
    }
    fn release_failed_signing_claim(
        &self,
        statement_id: [u8; 32],
    ) -> Result<(), HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        let mut next = guard.checkpoint.clone();
        let record = find_statement_mut(&mut next, statement_id)?;
        if record.state != StoredStatementDeliveryStateV1::Signing
            || record.signed_statement.is_some()
        {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        record.signing_claim_cursor = None;
        record.state = if record.signing_attempts >= self.policy.max_signing_attempts {
            StoredStatementDeliveryStateV1::DeadLetter
        } else {
            StoredStatementDeliveryStateV1::ReadyForSigning
        };
        self.commit_locked(guard, next)
    }
    fn mark_publication_ambiguous(
        &self,
        statement_id: [u8; 32],
        signed: &SignedGovernedBillingStatementV1,
    ) -> Result<(), HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        let mut next = guard.checkpoint.clone();
        let record = find_statement_mut(&mut next, statement_id)?;
        if record.state != StoredStatementDeliveryStateV1::ReadyForPublication
            || record.signed_statement.as_ref() != Some(signed)
            || record.publication_receipt.is_some()
        {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        record.state = StoredStatementDeliveryStateV1::PublicationAmbiguous;
        self.commit_locked(guard, next)
    }
    fn mark_publication_absent(
        &self,
        statement_id: [u8; 32],
    ) -> Result<(), HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        let mut next = guard.checkpoint.clone();
        let record = find_statement_mut(&mut next, statement_id)?;
        if record.state != StoredStatementDeliveryStateV1::PublicationAmbiguous
            || record.publication_receipt.is_some()
        {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        record.state = StoredStatementDeliveryStateV1::ReadyForPublication;
        self.commit_locked(guard, next)
    }
    fn store_publication_receipt(
        &self,
        statement_id: [u8; 32],
        receipt: BillingStatementPublicationReceiptV1,
    ) -> Result<(), HedgingBillingServiceError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| HedgingBillingServiceError::StateLockPoisoned)?;
        let mut next = guard.checkpoint.clone();
        let record = find_statement_mut(&mut next, statement_id)?;
        if !matches!(
            record.state,
            StoredStatementDeliveryStateV1::ReadyForPublication
                | StoredStatementDeliveryStateV1::PublicationAmbiguous
        ) {
            return Err(HedgingBillingServiceError::ConcurrentMutation);
        }
        let signed = record
            .signed_statement
            .as_ref()
            .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
        receipt.validate(signed, &self.policy.statement_publisher)?;
        record.publication_receipt = Some(receipt);
        record.state = StoredStatementDeliveryStateV1::Published;
        self.commit_locked(guard, next)
    }
    fn commit_locked(
        &self,
        guard: std::sync::MutexGuard<'_, RuntimeState>,
        next: HedgingBillingCheckpointV1,
    ) -> Result<(), HedgingBillingServiceError> {
        let mut allow_commit = || Ok(());
        self.commit_locked_with_precommit_fence(guard, next, &mut allow_commit)
    }
    fn commit_locked_with_precommit_fence(
        &self,
        mut guard: std::sync::MutexGuard<'_, RuntimeState>,
        next: HedgingBillingCheckpointV1,
        pre_commit_fence: &mut dyn FnMut() -> Result<(), HedgingBillingServiceError>,
    ) -> Result<(), HedgingBillingServiceError> {
        next.validate(&self.policy, &self.feed_policy)?;
        let bytes = encode_checkpoint(&next, &self.policy, &self.feed_policy)?;
        pre_commit_fence()?;
        let fingerprint = self.store.commit_bytes(&bytes, guard.fingerprint)?;
        guard.checkpoint = next;
        guard.fingerprint = Some(fingerprint);
        Ok(())
    }
}
fn reconcile_authoritative_delivery_state(
    checkpoint: &mut HedgingBillingCheckpointV1,
    policy: &HedgingBillingServicePolicyV1,
    feed_policy: &HedgingFeedTrustPolicyV1,
    publisher: &dyn BillingStatementPublisher,
    acknowledgement_authority: &dyn BillingStatementAcknowledgementAuthority,
) -> Result<bool, HedgingBillingServiceError> {
    let before = checkpoint.clone();
    let local_acknowledgements: BTreeMap<[u8; 32], BillingStatementAcknowledgementV1> = checkpoint
        .acknowledgements
        .iter()
        .cloned()
        .map(|acknowledgement| (acknowledgement.statement_id, acknowledgement))
        .collect();
    let mut reconciled_acknowledgements = Vec::new();
    let period_closes = &checkpoint.period_closes;
    for record in &mut checkpoint.statements {
        let close = period_closes
            .iter()
            .find(|close| {
                close.period_end_unix == record.governed_statement.statement.period_end_unix
            })
            .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
        if let Some(local_signed) = &record.signed_statement {
            local_signed.verify(policy, feed_policy, close)?;
            if local_signed.governed_statement != record.governed_statement {
                return Err(HedgingBillingServiceError::InvalidCheckpoint);
            }
        }
        match publisher.lookup(record.governed_statement.statement.statement_id)? {
            Some(publication) => {
                publication
                    .signed_statement
                    .verify(policy, feed_policy, close)?;
                if publication.signed_statement.governed_statement != record.governed_statement
                    || record
                        .signed_statement
                        .as_ref()
                        .is_some_and(|local| local != &publication.signed_statement)
                {
                    return Err(HedgingBillingServiceError::AuthoritativeReconciliationMismatch);
                }
                publication
                    .receipt
                    .validate(&publication.signed_statement, &policy.statement_publisher)?;
                if record
                    .publication_receipt
                    .as_ref()
                    .is_some_and(|local| local != &publication.receipt)
                {
                    return Err(HedgingBillingServiceError::AuthoritativeReconciliationMismatch);
                }
                record.signing_claim_cursor = None;
                record.signed_statement = Some(publication.signed_statement);
                record.publication_receipt = Some(publication.receipt);
                record.state = StoredStatementDeliveryStateV1::Published;
            }
            None => {
                if record.publication_receipt.is_some()
                    || matches!(
                        record.state,
                        StoredStatementDeliveryStateV1::Published
                            | StoredStatementDeliveryStateV1::Acknowledged
                    )
                {
                    return Err(HedgingBillingServiceError::AuthoritativeReconciliationMismatch);
                }
                if record.state == StoredStatementDeliveryStateV1::PublicationAmbiguous {
                    record.state = StoredStatementDeliveryStateV1::ReadyForPublication;
                }
            }
        }
        if record.publication_receipt.is_some() {
            let signed = record
                .signed_statement
                .as_ref()
                .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
            match acknowledgement_authority
                .lookup(record.governed_statement.statement.statement_id)?
            {
                Some(acknowledgement) => {
                    validate_acknowledgement_record(record, &acknowledgement, policy.network_id)?;
                    acknowledgement_authority.verify(signed, &acknowledgement)?;
                    if local_acknowledgements
                        .get(&acknowledgement.statement_id)
                        .is_some_and(|local| local != &acknowledgement)
                    {
                        return Err(
                            HedgingBillingServiceError::AuthoritativeReconciliationMismatch,
                        );
                    }
                    if reconciled_acknowledgements.len()
                        >= usize::try_from(policy.max_acknowledgements).unwrap_or(usize::MAX)
                    {
                        return Err(HedgingBillingServiceError::ResourceExhausted);
                    }
                    record.state = StoredStatementDeliveryStateV1::Acknowledged;
                    reconciled_acknowledgements.push(acknowledgement);
                }
                None => {
                    if local_acknowledgements
                        .contains_key(&record.governed_statement.statement.statement_id)
                    {
                        return Err(
                            HedgingBillingServiceError::AuthoritativeReconciliationMismatch,
                        );
                    }
                }
            }
        }
    }
    reconciled_acknowledgements.sort_by_key(|entry| entry.acknowledgement_id);
    checkpoint.acknowledgements = reconciled_acknowledgements;
    Ok(*checkpoint != before)
}
fn validate_acknowledgement_record(
    record: &StoredStatementV1,
    acknowledgement: &BillingStatementAcknowledgementV1,
    network_id: NetworkId,
) -> Result<(), HedgingBillingServiceError> {
    let publication = record
        .publication_receipt
        .as_ref()
        .ok_or(HedgingBillingServiceError::InvalidCheckpoint)?;
    if acknowledgement.version != BILLING_STATEMENT_ACKNOWLEDGEMENT_VERSION_V1
        || acknowledgement.network_id != network_id
        || acknowledgement.statement_id != record.governed_statement.statement.statement_id
        || acknowledgement.account_digest
            != *blake3::hash(&record.governed_statement.statement.account_id).as_bytes()
        || acknowledgement.request_binding_digest == [0; 32]
        || acknowledgement.acknowledged_at_unix < publication.published_at_unix
        || acknowledgement.acknowledged_at_unix == u64::MAX
        || acknowledgement.authentication_proof.is_empty()
        || acknowledgement.authentication_proof.len() > BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1
        || acknowledgement.acknowledgement_id != acknowledgement_digest(acknowledgement)?
    {
        return Err(HedgingBillingServiceError::InvalidAcknowledgement);
    }
    Ok(())
}
fn decode_and_validate_epoch_witness(
    record: &HedgingBillingEpochWitnessRecordV1,
    policy: &HedgingBillingServicePolicyV1,
    feed_policy: &HedgingFeedTrustPolicyV1,
    journal_verifier: &dyn HedgingBillingJournalVerifier,
    witness_store: &dyn HedgingBillingEpochWitnessStore,
) -> Result<HedgingBillingCheckpointV1, HedgingBillingServiceError> {
    let checkpoint = decode_checkpoint(&record.checkpoint_bytes, policy, feed_policy)?;
    validate_checkpoint_epoch_witness(
        &checkpoint,
        record,
        policy,
        feed_policy,
        journal_verifier,
        witness_store,
    )?;
    Ok(checkpoint)
}
fn validate_checkpoint_epoch_witness(
    checkpoint: &HedgingBillingCheckpointV1,
    record: &HedgingBillingEpochWitnessRecordV1,
    policy: &HedgingBillingServicePolicyV1,
    feed_policy: &HedgingFeedTrustPolicyV1,
    journal_verifier: &dyn HedgingBillingJournalVerifier,
    witness_store: &dyn HedgingBillingEpochWitnessStore,
) -> Result<(), HedgingBillingServiceError> {
    record.validate(policy.checkpoint_max_bytes)?;
    let archived = witness_store
        .load_epoch(record.epoch_sequence)?
        .ok_or(HedgingBillingServiceError::InvalidEpochWitness)?;
    if archived != *record {
        return Err(HedgingBillingServiceError::EpochWitnessFork);
    }
    let base_checkpoint = decode_checkpoint(&record.checkpoint_bytes, policy, feed_policy)?;
    let transition = base_checkpoint
        .latest_epoch_transition
        .as_ref()
        .ok_or(HedgingBillingServiceError::InvalidEpochWitness)?;
    let compacted_commitment = base_checkpoint
        .compacted_journal_commitment
        .ok_or(HedgingBillingServiceError::InvalidEpochWitness)?;
    if record.network_id != policy.network_id
        || checkpoint.epoch_sequence != record.epoch_sequence
        || base_checkpoint.epoch_sequence != record.epoch_sequence
        || checkpoint
            .latest_epoch_transition
            .as_ref()
            .map(|transition| transition.transition_id)
            != Some(record.transition_id)
        || transition.transition_id != record.transition_id
        || base_checkpoint.journal_commitment != Some(compacted_commitment)
        || base_checkpoint.finalized_cursor != Some(compacted_commitment.finalized_cursor)
        || base_checkpoint.next_event_sequence != compacted_commitment.journal_next_sequence
        || base_checkpoint.last_period_end_unix != base_checkpoint.compacted_through_period_end_unix
        || base_checkpoint.last_page_digest.is_some()
        || !base_checkpoint.source_pages.is_empty()
        || !base_checkpoint.period_closes.is_empty()
        || !base_checkpoint.open_accruals.is_empty()
        || !base_checkpoint.replay_receipts.is_empty()
        || !base_checkpoint.event_replay_receipts.is_empty()
        || !base_checkpoint.statements.is_empty()
        || !base_checkpoint.acknowledgements.is_empty()
        || !base_checkpoint.hedge_intents.is_empty()
    {
        return Err(HedgingBillingServiceError::InvalidEpochWitness);
    }
    journal_verifier.verify_epoch_transition(&policy.network_id, transition)?;
    if record.epoch_sequence > 1 {
        let previous_record = witness_store
            .load_epoch(record.epoch_sequence - 1)?
            .ok_or(HedgingBillingServiceError::EpochWitnessRollback)?;
        previous_record.validate(transition.previous_service_policy.checkpoint_max_bytes)?;
        if transition.predecessor_transition_digest != Some(previous_record.transition_id) {
            return Err(HedgingBillingServiceError::EpochWitnessFork);
        }
        let previous_checkpoint = decode_checkpoint(
            &previous_record.checkpoint_bytes,
            &transition.previous_service_policy,
            &transition.previous_feed_policy,
        )?;
        if previous_checkpoint
            .latest_epoch_transition
            .as_ref()
            .map(|previous| previous.transition_id)
            != Some(previous_record.transition_id)
        {
            return Err(HedgingBillingServiceError::InvalidEpochWitness);
        }
    } else if transition.predecessor_transition_digest.is_some() {
        return Err(HedgingBillingServiceError::EpochWitnessFork);
    }
    Ok(())
}
fn decode_checkpoint(
    bytes: &[u8],
    policy: &HedgingBillingServicePolicyV1,
    feed_policy: &HedgingFeedTrustPolicyV1,
) -> Result<HedgingBillingCheckpointV1, HedgingBillingServiceError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > policy.checkpoint_max_bytes {
        return Err(HedgingBillingServiceError::ResourceExhausted);
    }
    let max_bytes = usize::try_from(policy.checkpoint_max_bytes).unwrap_or(usize::MAX);
    let max_elements = max_bytes
        .saturating_mul(CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT)
        .max(1);
    let max_allocation = max_bytes
        .saturating_mul(CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT)
        .saturating_add(CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES);
    let checkpoint: HedgingBillingCheckpointV1 = decode_from_bytes_with_limits(
        bytes,
        DecodeLimits::new(
            max_elements,
            max_bytes,
            max_elements,
            max_allocation,
            CHECKPOINT_MAX_NESTING_DEPTH,
        ),
    )
    .map_err(|_| HedgingBillingServiceError::InvalidCheckpoint)?;
    let canonical =
        norito::to_bytes(&checkpoint).map_err(|_| HedgingBillingServiceError::EncodingFailed)?;
    if canonical != bytes {
        return Err(HedgingBillingServiceError::NonCanonicalCheckpoint);
    }
    checkpoint.validate(policy, feed_policy)?;
    Ok(checkpoint)
}
fn encode_checkpoint(
    checkpoint: &HedgingBillingCheckpointV1,
    policy: &HedgingBillingServicePolicyV1,
    feed_policy: &HedgingFeedTrustPolicyV1,
) -> Result<Vec<u8>, HedgingBillingServiceError> {
    checkpoint.validate(policy, feed_policy)?;
    if let Some(length) = checkpoint.encoded_len_exact()
        && u64::try_from(length).unwrap_or(u64::MAX) > policy.checkpoint_max_bytes
    {
        return Err(HedgingBillingServiceError::ResourceExhausted);
    }
    let bytes =
        norito::to_bytes(checkpoint).map_err(|_| HedgingBillingServiceError::EncodingFailed)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > policy.checkpoint_max_bytes {
        return Err(HedgingBillingServiceError::ResourceExhausted);
    }
    Ok(bytes)
}
fn period_outcome_from_checkpoint(
    checkpoint: &HedgingBillingCheckpointV1,
    period_end_unix: u64,
) -> Result<HedgingBillingPeriodOutcomeV1, HedgingBillingServiceError> {
    if !checkpoint
        .period_closes
        .iter()
        .any(|close| close.period_end_unix == period_end_unix)
    {
        return Err(HedgingBillingServiceError::InvalidPeriodClose);
    }
    let statement_ids: Vec<[u8; 32]> = checkpoint
        .statements
        .iter()
        .filter(|record| record.governed_statement.statement.period_end_unix == period_end_unix)
        .map(|record| record.governed_statement.statement.statement_id)
        .collect();
    let statement_bundle_digest =
        hash_canonical(b"sorafs.billing.statement-bundle.v1", &statement_ids)?;
    Ok(HedgingBillingPeriodOutcomeV1 {
        period_end_unix,
        statement_ids,
        statement_bundle_digest,
        hedge_intent: checkpoint
            .hedge_intents
            .iter()
            .find(|intent| intent.period_end_unix == period_end_unix)
            .cloned(),
    })
}
fn find_statement(
    checkpoint: &HedgingBillingCheckpointV1,
    statement_id: [u8; 32],
) -> Result<&StoredStatementV1, HedgingBillingServiceError> {
    checkpoint
        .statements
        .binary_search_by_key(&statement_id, |record| {
            record.governed_statement.statement.statement_id
        })
        .ok()
        .and_then(|index| checkpoint.statements.get(index))
        .ok_or(HedgingBillingServiceError::StatementNotFound)
}
fn find_statement_mut(
    checkpoint: &mut HedgingBillingCheckpointV1,
    statement_id: [u8; 32],
) -> Result<&mut StoredStatementV1, HedgingBillingServiceError> {
    let index = checkpoint
        .statements
        .binary_search_by_key(&statement_id, |record| {
            record.governed_statement.statement.statement_id
        })
        .map_err(|_| HedgingBillingServiceError::StatementNotFound)?;
    checkpoint
        .statements
        .get_mut(index)
        .ok_or(HedgingBillingServiceError::StatementNotFound)
}
fn validate_canonical_account_id_bytes(
    account_id: &[u8],
) -> Result<(), HedgingBillingServiceError> {
    if account_id.is_empty() || account_id.len() > MAX_BILLING_ACCOUNT_ID_BYTES {
        return Err(HedgingBillingServiceError::InvalidFinalizedEvent);
    }
    let literal = std::str::from_utf8(account_id)
        .map_err(|_| HedgingBillingServiceError::InvalidFinalizedEvent)?;
    let (_, canonical, _) = AccountId::parse_encoded(literal)
        .map_err(|_| HedgingBillingServiceError::InvalidFinalizedEvent)?
        .into_parts();
    if literal != canonical || account_id != canonical.as_bytes() {
        return Err(HedgingBillingServiceError::InvalidFinalizedEvent);
    }
    Ok(())
}
fn projection_anchor(
    state: &RuntimeState,
    policy: &HedgingBillingServicePolicyV1,
) -> Result<HedgingBillingProjectionAnchorV1, HedgingBillingRuntimeApiErrorV1> {
    let checkpoint_fingerprint = state
        .fingerprint
        .ok_or(HedgingBillingRuntimeApiErrorV1::Unavailable)?;
    if checkpoint_fingerprint == [0; 32] || state.checkpoint.next_event_sequence == 0 {
        return Err(HedgingBillingRuntimeApiErrorV1::Unavailable);
    }
    Ok(HedgingBillingProjectionAnchorV1 {
        checkpoint_fingerprint,
        finalized_cursor: state.checkpoint.finalized_cursor,
        next_event_sequence: state.checkpoint.next_event_sequence,
        active_epoch_sequence: state.checkpoint.epoch_sequence,
        active_policy_revision: policy.revision,
        service_policy_digest: policy.digest().map_err(runtime_api_service_error)?,
        compacted_through_period_end_unix: state.checkpoint.compacted_through_period_end_unix,
        retention_scope: HedgingBillingRetentionScopeV1::ActiveEpochOnly,
    })
}
fn service_status(
    checkpoint: &HedgingBillingCheckpointV1,
    policy: &HedgingBillingServicePolicyV1,
) -> Result<HedgingBillingServiceStatusV1, HedgingBillingServiceError> {
    let mut status = HedgingBillingServiceStatusV1 {
        next_event_sequence: checkpoint.next_event_sequence,
        finalized_height: checkpoint
            .finalized_cursor
            .map_or(0, |cursor| cursor.height),
        next_period_end_unix: checkpoint
            .last_period_end_unix
            .checked_add(policy.billing_period_secs)
            .ok_or(HedgingBillingServiceError::AmountOverflow)?,
        ready_for_signing: 0,
        signing: 0,
        ready_for_publication: 0,
        publication_ambiguous: 0,
        published: 0,
        acknowledged: 0,
        dead_letter: 0,
        hedge_intents: u32::try_from(checkpoint.hedge_intents.len())
            .map_err(|_| HedgingBillingServiceError::ResourceExhausted)?,
    };
    for statement in &checkpoint.statements {
        let counter = match statement.state {
            StoredStatementDeliveryStateV1::ReadyForSigning => &mut status.ready_for_signing,
            StoredStatementDeliveryStateV1::Signing => &mut status.signing,
            StoredStatementDeliveryStateV1::ReadyForPublication => {
                &mut status.ready_for_publication
            }
            StoredStatementDeliveryStateV1::PublicationAmbiguous => {
                &mut status.publication_ambiguous
            }
            StoredStatementDeliveryStateV1::Published => &mut status.published,
            StoredStatementDeliveryStateV1::Acknowledged => &mut status.acknowledged,
            StoredStatementDeliveryStateV1::DeadLetter => &mut status.dead_letter,
        };
        *counter = (*counter)
            .checked_add(1)
            .ok_or(HedgingBillingServiceError::ResourceExhausted)?;
    }
    Ok(status)
}
fn require_projection_fingerprint(
    anchor: &HedgingBillingProjectionAnchorV1,
    expected: [u8; 32],
) -> Result<(), HedgingBillingRuntimeApiErrorV1> {
    if anchor.checkpoint_fingerprint != expected {
        return Err(HedgingBillingRuntimeApiErrorV1::ProjectionChanged);
    }
    Ok(())
}
fn validate_runtime_page_bound(limit: u16) -> Result<(), HedgingBillingRuntimeApiErrorV1> {
    if limit == 0 || limit > HEDGING_BILLING_RUNTIME_API_MAX_PAGE_ITEMS_V1 {
        return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
    }
    Ok(())
}
fn validate_projection_page_request(
    request: &HedgingBillingProjectionPageRequestV1,
) -> Result<(), HedgingBillingRuntimeApiErrorV1> {
    validate_runtime_page_bound(request.limit)?;
    if request.expected_checkpoint_fingerprint == [0; 32] || request.after == Some([0; 32]) {
        return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
    }
    Ok(())
}
fn terminal_owned_statement<'checkpoint>(
    checkpoint: &'checkpoint HedgingBillingCheckpointV1,
    statement_id: [u8; 32],
    owner_account_id: &[u8],
) -> Result<&'checkpoint StoredStatementV1, HedgingBillingRuntimeApiErrorV1> {
    let record = find_statement(checkpoint, statement_id)
        .map_err(|_| HedgingBillingRuntimeApiErrorV1::StatementUnavailableToOwner)?;
    if record.governed_statement.statement.account_id != owner_account_id
        || !matches!(
            record.state,
            StoredStatementDeliveryStateV1::Published
                | StoredStatementDeliveryStateV1::Acknowledged
        )
    {
        return Err(HedgingBillingRuntimeApiErrorV1::StatementUnavailableToOwner);
    }
    Ok(record)
}
fn acknowledgement_projection(
    acknowledgement: &BillingStatementAcknowledgementV1,
) -> BillingStatementAcknowledgementProjectionV1 {
    BillingStatementAcknowledgementProjectionV1 {
        acknowledgement_id: acknowledgement.acknowledgement_id,
        statement_id: acknowledgement.statement_id,
        account_digest: acknowledgement.account_digest,
        request_binding_digest: acknowledgement.request_binding_digest,
        acknowledged_at_unix: acknowledgement.acknowledged_at_unix,
    }
}
/// Derive the canonical proof challenge and idempotency binding for one owner
/// acknowledgement request.
///
/// The bounded authentication proof is deliberately excluded: it authenticates
/// this digest and therefore cannot be part of its own preimage.
///
/// # Errors
///
/// Rejects zero statement/nonces and account bytes that are not the exact
/// canonical UTF-8 I105 representation.
pub fn billing_statement_acknowledgement_request_digest_v1(
    statement_id: [u8; 32],
    owner_account_id: &[u8],
    request_nonce: [u8; 32],
) -> Result<[u8; 32], HedgingBillingRuntimeApiErrorV1> {
    validate_canonical_account_id_bytes(owner_account_id)
        .map_err(|_| HedgingBillingRuntimeApiErrorV1::InvalidRequest)?;
    if statement_id == [0; 32] || request_nonce == [0; 32] {
        return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
    }
    let account_length = u64::try_from(owner_account_id.len())
        .map_err(|_| HedgingBillingRuntimeApiErrorV1::ResourceExhausted)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(ACKNOWLEDGEMENT_REQUEST_DOMAIN_V1);
    hasher.update(&statement_id);
    hasher.update(&account_length.to_le_bytes());
    hasher.update(owner_account_id);
    hasher.update(&request_nonce);
    Ok(*hasher.finalize().as_bytes())
}
fn projection_close_start(
    closes: &[HedgingBillingFinalizedPeriodCloseV1],
    after: Option<[u8; 32]>,
    policy: &HedgingBillingServicePolicyV1,
    feed_policy: &HedgingFeedTrustPolicyV1,
) -> Result<usize, HedgingBillingRuntimeApiErrorV1> {
    let Some(after) = after else {
        return Ok(0);
    };
    let period_end_unix = u64::from_le_bytes(
        after[..8]
            .try_into()
            .map_err(|_| HedgingBillingRuntimeApiErrorV1::InvalidRequest)?,
    );
    let index = closes
        .binary_search_by_key(&period_end_unix, |close| close.period_end_unix)
        .map_err(|_| HedgingBillingRuntimeApiErrorV1::InvalidRequest)?;
    let close = closes
        .get(index)
        .ok_or(HedgingBillingRuntimeApiErrorV1::InvalidRequest)?;
    let digest = close
        .close_digest(policy, feed_policy)
        .map_err(runtime_api_service_error)?;
    if exposure_cursor(period_end_unix, digest) != after {
        return Err(HedgingBillingRuntimeApiErrorV1::InvalidRequest);
    }
    index
        .checked_add(1)
        .ok_or(HedgingBillingRuntimeApiErrorV1::ResourceExhausted)
}
fn exposure_cursor(period_end_unix: u64, period_close_digest: [u8; 32]) -> [u8; 32] {
    let mut cursor = [0_u8; 32];
    cursor[..8].copy_from_slice(&period_end_unix.to_le_bytes());
    let mut hasher = blake3::Hasher::new();
    hasher.update(EXPOSURE_CURSOR_DOMAIN_V1);
    hasher.update(&period_end_unix.to_le_bytes());
    hasher.update(&period_close_digest);
    cursor[8..].copy_from_slice(&hasher.finalize().as_bytes()[..24]);
    cursor
}
fn statement_delivery_projection(
    record: &StoredStatementV1,
) -> BillingStatementDeliveryProjectionV1 {
    BillingStatementDeliveryProjectionV1 {
        statement_id: record.governed_statement.statement.statement_id,
        account_digest: *blake3::hash(&record.governed_statement.statement.account_id).as_bytes(),
        period_end_unix: record.governed_statement.statement.period_end_unix,
        status: record.state.into(),
        signing_attempts: record.signing_attempts,
    }
}
fn runtime_api_acknowledgement_error(
    error: HedgingBillingServiceError,
) -> HedgingBillingRuntimeApiErrorV1 {
    match error {
        HedgingBillingServiceError::ProjectionAnchorConflict
        | HedgingBillingServiceError::ConcurrentMutation
        | HedgingBillingServiceError::CheckpointStale => {
            HedgingBillingRuntimeApiErrorV1::ProjectionChanged
        }
        HedgingBillingServiceError::AcknowledgementConflict => {
            HedgingBillingRuntimeApiErrorV1::AcknowledgementConflict
        }
        HedgingBillingServiceError::InvalidAcknowledgement
        | HedgingBillingServiceError::External(HedgingBillingExternalError::Rejected) => {
            HedgingBillingRuntimeApiErrorV1::InvalidRequest
        }
        HedgingBillingServiceError::StatementNotFound
        | HedgingBillingServiceError::InvalidDeliveryState => {
            HedgingBillingRuntimeApiErrorV1::StatementUnavailableToOwner
        }
        other => runtime_api_service_error(other),
    }
}
fn runtime_api_service_error(error: HedgingBillingServiceError) -> HedgingBillingRuntimeApiErrorV1 {
    match error {
        HedgingBillingServiceError::ResourceExhausted
        | HedgingBillingServiceError::CheckpointTooLarge => {
            HedgingBillingRuntimeApiErrorV1::ResourceExhausted
        }
        HedgingBillingServiceError::ProjectionAnchorConflict
        | HedgingBillingServiceError::ConcurrentMutation
        | HedgingBillingServiceError::CheckpointStale => {
            HedgingBillingRuntimeApiErrorV1::ProjectionChanged
        }
        HedgingBillingServiceError::InvalidQueryBound
        | HedgingBillingServiceError::InvalidAcknowledgement => {
            HedgingBillingRuntimeApiErrorV1::InvalidRequest
        }
        HedgingBillingServiceError::AcknowledgementConflict => {
            HedgingBillingRuntimeApiErrorV1::AcknowledgementConflict
        }
        HedgingBillingServiceError::StatementNotFound
        | HedgingBillingServiceError::InvalidDeliveryState => {
            HedgingBillingRuntimeApiErrorV1::StatementUnavailableToOwner
        }
        _ => HedgingBillingRuntimeApiErrorV1::Unavailable,
    }
}
#[derive(DeriveNoritoSerialize)]
struct BillingSourceReceiptPreimageV1 {
    network_id: NetworkId,
    source: BillingAccrualSourceV1,
    source_id: String,
}
#[derive(DeriveNoritoSerialize)]
struct BillingEventReplayPreimageV1 {
    network_id: NetworkId,
    event: HedgingBillingFinalizedEventV1,
}
fn source_receipt(
    network_id: NetworkId,
    event: &HedgingBillingFinalizedEventV1,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    let preimage = BillingSourceReceiptPreimageV1 {
        network_id,
        source: event.source,
        source_id: event.source_id.clone(),
    };
    hash_canonical(SOURCE_RECEIPT_DOMAIN_V1, &preimage)
}
fn event_replay_digest(
    network_id: NetworkId,
    event: &HedgingBillingFinalizedEventV1,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    hash_canonical(
        EVENT_REPLAY_RECEIPT_DOMAIN_V1,
        &BillingEventReplayPreimageV1 {
            network_id,
            event: event.clone(),
        },
    )
}
fn signed_statement_digest(
    signed: &SignedGovernedBillingStatementV1,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    let bytes = norito::to_bytes(signed).map_err(|_| HedgingBillingServiceError::EncodingFailed)?;
    Ok(*blake3::hash(&bytes).as_bytes())
}
fn verify_publisher_identity(
    policy: &BillingStatementPublisherPolicyV1,
    publisher: &dyn BillingStatementPublisher,
) -> Result<(), HedgingBillingServiceError> {
    policy.validate()?;
    let identity = publisher.identity()?;
    if identity.publisher_id != policy.publisher_id
        || identity.route_id != policy.route_id
        || identity.public_key != policy.public_key
        || checked_verifying_key(identity.public_key).is_err()
    {
        return Err(HedgingBillingServiceError::PublisherIdentityMismatch);
    }
    Ok(())
}
fn publication_receipt_digest(
    receipt: &BillingStatementPublicationReceiptV1,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    let mut canonical = receipt.clone();
    canonical.receipt_digest = [0; 32];
    canonical.signature = [0; 64];
    hash_canonical(PUBLICATION_RECEIPT_DOMAIN_V1, &canonical)
}
fn acknowledgement_digest(
    acknowledgement: &BillingStatementAcknowledgementV1,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    let mut canonical = acknowledgement.clone();
    canonical.acknowledgement_id = [0; 32];
    hash_canonical(ACKNOWLEDGEMENT_DOMAIN_V1, &canonical)
}
fn execution_authorization_digest(
    authorization: &HedgeExecutionAuthorizationV1,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    let mut canonical = authorization.clone();
    canonical.authorization_id = [0; 32];
    canonical.signature = [0; 64];
    hash_canonical(HEDGE_EXECUTION_AUTHORIZATION_DOMAIN_V1, &canonical)
}
fn execution_receipt_digest(
    receipt: &HedgeExecutionSubmissionReceiptV1,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    let mut canonical = receipt.clone();
    canonical.receipt_digest = [0; 32];
    canonical.signature = [0; 64];
    hash_canonical(HEDGE_EXECUTION_RECEIPT_DOMAIN_V1, &canonical)
}
fn retained_epoch_state_digest(
    checkpoint: &HedgingBillingCheckpointV1,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    hash_canonical(
        RETAINED_EPOCH_STATE_DOMAIN_V1,
        &RetainedEpochBaseStateV1 {
            checkpoint_version: checkpoint.version,
            network_id: checkpoint.network_id,
            policy_digest: checkpoint.policy_digest,
            epoch_sequence: checkpoint.epoch_sequence,
            compacted_journal_commitment: checkpoint.compacted_journal_commitment,
            compacted_through_period_end_unix: checkpoint.compacted_through_period_end_unix,
            compacted_account_bases: checkpoint.compacted_account_bases.clone(),
        },
    )
}
fn epoch_witness_record_revision(record: &HedgingBillingEpochWitnessRecordV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(EPOCH_WITNESS_RECORD_DOMAIN_V1);
    hasher.update(&[record.version]);
    hasher.update(record.network_id.as_bytes());
    hasher.update(&record.epoch_sequence.to_le_bytes());
    hasher.update(&record.transition_id);
    hasher.update(&record.checkpoint_digest);
    hasher.update(
        &u64::try_from(record.checkpoint_bytes.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(&record.checkpoint_bytes);
    *hasher.finalize().as_bytes()
}
fn epoch_witness_record_max_bytes(
    checkpoint_max_bytes: u64,
) -> Result<u64, HedgingBillingServiceError> {
    if checkpoint_max_bytes == 0 || checkpoint_max_bytes > HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1 {
        return Err(HedgingBillingServiceError::ResourceExhausted);
    }
    checkpoint_max_bytes
        .checked_add(HEDGING_BILLING_EPOCH_WITNESS_WRAPPER_MAX_BYTES_V1)
        .ok_or(HedgingBillingServiceError::ResourceExhausted)
}
fn epoch_transition_signature_digest(transition_id: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(EPOCH_TRANSITION_SIGNATURE_DOMAIN_V1);
    hasher.update(&transition_id);
    *hasher.finalize().as_bytes()
}
#[derive(DeriveNoritoSerialize)]
struct RetainedEpochBaseStateV1 {
    checkpoint_version: u8,
    network_id: NetworkId,
    policy_digest: [u8; 32],
    epoch_sequence: u64,
    compacted_journal_commitment: Option<HedgingBillingJournalCommitmentV1>,
    compacted_through_period_end_unix: u64,
    compacted_account_bases: Vec<GovernedBillingStatementV1>,
}
#[derive(DeriveNoritoSerialize)]
struct CompactedSourceArchiveV1 {
    network_id: NetworkId,
    predecessor_compacted_journal_commitment: Option<HedgingBillingJournalCommitmentV1>,
    predecessor_compacted_through_period_end_unix: u64,
    source_pages: Vec<HedgingBillingFinalizedEventPageV1>,
    period_closes: Vec<HedgingBillingFinalizedPeriodCloseV1>,
    replay_receipts: Vec<[u8; 32]>,
    event_replay_receipts: Vec<StoredEventReplayReceiptV1>,
}
#[derive(DeriveNoritoSerialize)]
struct CompactedEconomicArchiveV1 {
    network_id: NetworkId,
    predecessor_account_bases: Vec<GovernedBillingStatementV1>,
    statements: Vec<StoredStatementV1>,
    acknowledgements: Vec<BillingStatementAcknowledgementV1>,
    hedge_intents: Vec<HedgeIntentV1>,
}
fn checked_verifying_key(bytes: [u8; 32]) -> Result<VerifyingKey, ()> {
    let key = VerifyingKey::from_bytes(&bytes).map_err(|_| ())?;
    if key.to_bytes() != bytes || key.is_weak() {
        return Err(());
    }
    Ok(key)
}
fn validate_identifier(
    value: &str,
    max_bytes: usize,
    error: HedgingBillingServiceError,
) -> Result<(), HedgingBillingServiceError> {
    if value.is_empty()
        || value.len() > max_bytes
        || value != value.trim()
        || value.chars().any(char::is_control)
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'/' | b'@')
        })
    {
        return Err(error);
    }
    Ok(())
}
fn is_strictly_sorted_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn finalized_cursor_at_or_before(
    cursor: HedgingBillingFinalizedCursorV1,
    head: HedgingBillingFinalizedCursorV1,
) -> bool {
    cursor.finalized_at_unix <= head.finalized_at_unix
        && (cursor.height < head.height || cursor == head)
}
fn hash_canonical<T: NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], HedgingBillingServiceError> {
    let bytes = norito::to_bytes(value).map_err(|_| HedgingBillingServiceError::EncodingFailed)?;
    let length =
        u64::try_from(bytes.len()).map_err(|_| HedgingBillingServiceError::AmountOverflow)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&length.to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}
/// Durable finalized billing and delivery failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HedgingBillingServiceError {
    /// The configured service policy is invalid.
    #[error("hedging/billing policy is invalid")]
    InvalidPolicy,
    /// Runtime feed policy does not match the reviewed service-policy anchor.
    #[error("hedging feed trust policy does not match service policy")]
    FeedPolicyMismatch,
    /// Finalized block identity is invalid.
    #[error("finalized billing cursor is invalid")]
    InvalidFinalizedCursor,
    /// Consensus journal commitment is structurally invalid.
    #[error("consensus billing journal commitment is invalid")]
    InvalidJournalCommitment,
    /// Finalized event structure or source/category mapping is invalid.
    #[error("finalized billing event is invalid")]
    InvalidFinalizedEvent,
    /// Finalized page shape or chain binding is invalid.
    #[error("finalized billing page is invalid")]
    InvalidFinalizedPage,
    /// Finalized-query page budget is zero or exceeds the V1 ceiling.
    #[error("finalized billing query bound is invalid")]
    InvalidQueryBound,
    /// A required event sequence is missing.
    #[error("finalized billing sequence has a gap")]
    FinalizedSequenceGap,
    /// An older non-identical page was replayed.
    #[error("finalized billing sequence rolled back")]
    FinalizedSequenceRollback,
    /// A retained event sequence was replayed with different exact bytes.
    #[error("finalized billing event equivocation was rejected")]
    FinalizedEventEquivocation,
    /// Finalized height, hash, or timestamp forked or rolled back.
    #[error("finalized billing cursor forked or rolled back")]
    FinalizedForkOrRollback,
    /// An event arrived after its billing period was already closed.
    #[error("finalized billing event arrived after period closure")]
    LateFinalizedEvent,
    /// A semantic source was projected more than once.
    #[error("billing source was already projected")]
    DuplicateBillingSource,
    /// No finalized view is available.
    #[error("finalized billing view is unavailable")]
    FinalizedViewUnavailable,
    /// The requested billing boundary or governed reference is invalid.
    #[error("billing period or governed reference price is invalid")]
    InvalidBillingPeriod,
    /// Consensus period-close record is invalid or does not match the local tail.
    #[error("consensus billing period close is invalid")]
    InvalidPeriodClose,
    /// Signed epoch compaction or policy succession is invalid.
    #[error("hedging/billing epoch transition is invalid")]
    InvalidEpochTransition,
    /// Economic or source state is not terminal at the requested epoch frontier.
    #[error("hedging/billing epoch contains unsettled state")]
    UnsettledEpochTransition,
    /// Runtime epoch witness store does not match governed configuration.
    #[error("hedging/billing epoch witness store identity mismatch")]
    EpochWitnessIdentityMismatch,
    /// Sealed epoch witness record is malformed or inconsistent.
    #[error("hedging/billing epoch witness is invalid")]
    InvalidEpochWitness,
    /// Sealed epoch witness history rolled back.
    #[error("hedging/billing epoch witness rolled back")]
    EpochWitnessRollback,
    /// Sealed epoch witness history forked or skipped a predecessor.
    #[error("hedging/billing epoch witness forked")]
    EpochWitnessFork,
    /// The runtime signer is not active at the finalized cursor.
    #[error("billing statement signer is inactive or revoked")]
    SignerInactive,
    /// Runtime signer identity does not match governed configuration.
    #[error("billing statement signer identity mismatch")]
    SignerIdentityMismatch,
    /// Runtime publisher identity does not match governed configuration.
    #[error("billing statement publisher identity mismatch")]
    PublisherIdentityMismatch,
    /// The returned statement signature or envelope is invalid.
    #[error("signed governed billing statement is invalid")]
    InvalidSignedStatement,
    /// Statement signing retry budget is exhausted.
    #[error("billing statement signing retry budget exhausted")]
    SigningRetryExhausted,
    /// Publication receipt is invalid or conflicts with exact signed bytes.
    #[error("billing statement publication receipt is invalid")]
    InvalidPublicationReceipt,
    /// Acknowledgement structure, account, or timestamp is invalid.
    #[error("billing statement acknowledgement is invalid")]
    InvalidAcknowledgement,
    /// A statement already has a conflicting acknowledgement.
    #[error("billing statement acknowledgement conflicts with retained state")]
    AcknowledgementConflict,
    /// Local delivery state conflicts with an authoritative external lookup.
    #[error("billing delivery state conflicts with authoritative reconciliation")]
    AuthoritativeReconciliationMismatch,
    /// Hedge intent structure or digest is invalid.
    #[error("hedge intent is invalid")]
    InvalidHedgeIntent,
    /// Execution adapter identity is invalid.
    #[error("governed hedge execution adapter is invalid")]
    InvalidExecutionAdapter,
    /// Explicit operator hedge authorization is invalid.
    #[error("governed hedge execution authorization is invalid")]
    InvalidHedgeExecutionAuthorization,
    /// Venue hedge-submission receipt is invalid.
    #[error("governed hedge execution receipt is invalid")]
    InvalidHedgeExecutionReceipt,
    /// An execution authorization named no retained hedge intent.
    #[error("authorized hedge intent was not found")]
    HedgeIntentNotFound,
    /// Automatic hedge execution is forbidden for V1.
    #[error("automatic hedge execution is forbidden for SoraFS V1")]
    AutomaticHedgeExecutionForbidden,
    /// A statement identity is unknown.
    #[error("billing statement not found")]
    StatementNotFound,
    /// The requested statement delivery transition is invalid.
    #[error("billing statement delivery state is invalid")]
    InvalidDeliveryState,
    /// Durable state changed while an external operation was in flight.
    #[error("hedging/billing state changed concurrently")]
    ConcurrentMutation,
    /// An API mutation expected a different exact durable checkpoint.
    #[error("hedging/billing projection anchor changed")]
    ProjectionAnchorConflict,
    /// Durable checkpoint structure or replay validation failed.
    #[error("hedging/billing checkpoint is invalid")]
    InvalidCheckpoint,
    /// Checkpoint bytes were validly decodable but noncanonical.
    #[error("hedging/billing checkpoint is not canonically encoded")]
    NonCanonicalCheckpoint,
    /// A deterministic amount or timestamp overflowed.
    #[error("hedging/billing deterministic arithmetic overflow")]
    AmountOverflow,
    /// A deterministic resource bound was exceeded.
    #[error("hedging/billing resource bound exceeded")]
    ResourceExhausted,
    /// Canonical encoding failed.
    #[error("hedging/billing canonical encoding failed")]
    EncodingFailed,
    /// Runtime state lock was poisoned.
    #[error("hedging/billing runtime state lock poisoned")]
    StateLockPoisoned,
    /// Governed feed or billing payload validation failed.
    #[error("governed hedging or billing payload is invalid")]
    InvalidGovernedHedging,
    /// Intrinsic billing construction or transition validation failed.
    #[error("intrinsic billing payload is invalid")]
    InvalidBillingPayload,
    /// Exact XOR arithmetic failed.
    #[error("invalid exact XOR arithmetic")]
    XorAmount,
    /// Durable checkpoint path is unsafe or inaccessible.
    #[error("hedging/billing checkpoint I/O failed")]
    CheckpointIo,
    /// Durable checkpoint exceeds its policy ceiling.
    #[error("hedging/billing checkpoint exceeds its byte limit")]
    CheckpointTooLarge,
    /// Another runtime owns the checkpoint writer.
    #[error("hedging/billing checkpoint writer is busy")]
    CheckpointBusy,
    /// Another runtime changed the checkpoint.
    #[error("hedging/billing checkpoint changed concurrently")]
    CheckpointStale,
    /// Rename became visible but parent durability is uncertain.
    #[error("hedging/billing checkpoint durability is uncertain")]
    CheckpointDurabilityUncertain,
    /// In-process checkpoint coordination was poisoned.
    #[error("hedging/billing checkpoint runtime is poisoned")]
    CheckpointRuntimePoisoned,
    /// External dependency returned a payload-free failure class.
    #[error(transparent)]
    External(#[from] HedgingBillingExternalError),
}
impl From<sorafs_manifest::hedging::HedgingValidationError> for HedgingBillingServiceError {
    fn from(_: sorafs_manifest::hedging::HedgingValidationError) -> Self {
        Self::InvalidBillingPayload
    }
}
impl From<sorafs_manifest::deal::DealAmountError> for HedgingBillingServiceError {
    fn from(_: sorafs_manifest::deal::DealAmountError) -> Self {
        Self::XorAmount
    }
}
impl From<SignedHedgingError> for HedgingBillingServiceError {
    fn from(_: SignedHedgingError) -> Self {
        Self::InvalidGovernedHedging
    }
}
impl From<CheckpointStoreError> for HedgingBillingServiceError {
    fn from(error: CheckpointStoreError) -> Self {
        match error {
            CheckpointStoreError::Io => Self::CheckpointIo,
            CheckpointStoreError::TooLarge => Self::CheckpointTooLarge,
            CheckpointStoreError::Busy => Self::CheckpointBusy,
            CheckpointStoreError::Stale => Self::CheckpointStale,
            CheckpointStoreError::DurabilityUncertain => Self::CheckpointDurabilityUncertain,
            CheckpointStoreError::RuntimePoisoned => Self::CheckpointRuntimePoisoned,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, numeric::Quantity};
    use iroha_data_model::block::BlockHeader;
    use sorafs_manifest::hedging::{
        HEDGING_PRICE_FEED_VERSION_V1, HedgingFeedStatusV1, HedgingPriceFeedV1,
        signed::{
            HEDGING_FEED_BINDING_VERSION_V1, HEDGING_FEED_TRUST_POLICY_VERSION_V1,
            HEDGING_TRUSTED_SIGNER_VERSION_V1, HedgingFeedBindingV1, HedgingTrustedSignerV1,
            SIGNED_HEDGING_PRICE_FEED_VERSION_V1, SignedHedgingPriceFeedV1,
            derive_governed_reference_price_decision_v1,
        },
    };
    use std::collections::VecDeque;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    };
    const EPOCH: u64 = 1_900_000_000;
    const PERIOD_SECS: u64 = 3_600;
    const PERIOD_END: u64 = EPOCH + PERIOD_SECS;
    const FINALIZED_HASH: [u8; 32] = [0xA1; 32];
    const TEST_PROVIDER_QUALIFICATION: HedgingBillingRuntimeProviderQualificationV1 =
        HedgingBillingRuntimeProviderQualificationV1::new(1, [0xA2; 32]);
    fn test_network_id(genesis: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            genesis,
        )))
    }
    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }
    fn usd(value: &str) -> Quantity {
        value.parse().expect("canonical USD quantity")
    }
    fn account_bytes(seed: u8) -> Vec<u8> {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic account key")
                .public_key()
                .clone(),
        )
        .canonical_i105()
        .expect("canonical I105 account")
        .into_bytes()
    }
    fn primary_account_bytes() -> Vec<u8> {
        account_bytes(0x91)
    }
    fn feed_key() -> SigningKey {
        SigningKey::from_bytes(&[0x11; 32])
    }
    fn statement_key() -> SigningKey {
        SigningKey::from_bytes(&[0x22; 32])
    }
    fn publisher_key() -> SigningKey {
        SigningKey::from_bytes(&[0x23; 32])
    }
    fn transition_key() -> SigningKey {
        SigningKey::from_bytes(&[0x27; 32])
    }
    fn rotated_statement_key() -> SigningKey {
        SigningKey::from_bytes(&[0x28; 32])
    }
    fn rotated_transition_key() -> SigningKey {
        SigningKey::from_bytes(&[0x29; 32])
    }
    fn rotated_policies() -> (HedgingBillingServicePolicyV1, HedgingFeedTrustPolicyV1) {
        let previous = service_policy();
        let mut next_feed = feed_policy();
        next_feed.policy_id = [0x32; 32];
        let mut next = previous.clone();
        next.revision = previous.revision + 1;
        next.predecessor_policy_digest =
            Some(previous.digest().expect("previous service policy digest"));
        next.feed_trust_policy_digest = next_feed
            .canonical_digest()
            .expect("next feed policy digest");
        next.statement_signer.signer_id = "billing-hsm-2".to_owned();
        next.statement_signer.public_key = rotated_statement_key().verifying_key().to_bytes();
        next.transition_authority.authority_id = "billing-transition-hsm-2".to_owned();
        next.transition_authority.public_key = rotated_transition_key().verifying_key().to_bytes();
        (next, next_feed)
    }
    fn operator_key() -> SigningKey {
        SigningKey::from_bytes(&[0x25; 32])
    }
    fn venue_key() -> SigningKey {
        SigningKey::from_bytes(&[0x26; 32])
    }
    fn execution_policy() -> GovernedHedgeExecutionPolicyV1 {
        GovernedHedgeExecutionPolicyV1 {
            version: HEDGE_EXECUTION_POLICY_VERSION_V1,
            policy_id: [0x81; 32],
            venue_id: "venue-1".to_owned(),
            venue_public_key: venue_key().verifying_key().to_bytes(),
            operator_signer_id: "hedge-operator-1".to_owned(),
            operator_public_key: operator_key().verifying_key().to_bytes(),
            valid_from_unix: EPOCH,
            valid_until_unix: PERIOD_END + 1_000,
            max_submission_xor: xor("100"),
            max_slippage_bps: 100,
        }
    }
    fn execution_authorization(intent: &HedgeIntentV1) -> HedgeExecutionAuthorizationV1 {
        let policy = execution_policy();
        let mut authorization = HedgeExecutionAuthorizationV1 {
            version: HEDGE_EXECUTION_AUTHORIZATION_VERSION_V1,
            authorization_id: [0; 32],
            execution_policy_digest: policy.canonical_digest().expect("execution policy digest"),
            intent_id: intent.intent_id,
            service_policy_digest: intent.service_policy_digest,
            period_close_digest: intent.period_close_digest,
            venue_id: policy.venue_id,
            operator_signer_id: policy.operator_signer_id,
            xor_amount: intent.xor_amount.clone(),
            max_slippage_bps: intent.max_slippage_bps,
            authorized_at_unix: PERIOD_END + 2,
            expires_at_unix: intent.expires_at_unix,
            authorization_sequence: 1,
            signature: [0; 64],
        };
        authorization.authorization_id =
            execution_authorization_digest(&authorization).expect("authorization digest");
        let mut message = Vec::new();
        message.extend_from_slice(HEDGE_EXECUTION_AUTHORIZATION_SIGNATURE_DOMAIN_V1);
        message.extend_from_slice(&authorization.authorization_id);
        authorization.signature = operator_key().sign(&message).to_bytes();
        authorization
    }
    fn feed_policy() -> HedgingFeedTrustPolicyV1 {
        HedgingFeedTrustPolicyV1 {
            version: HEDGING_FEED_TRUST_POLICY_VERSION_V1,
            policy_id: [0x31; 32],
            valid_from_unix: EPOCH - 100,
            valid_until_unix: PERIOD_END + 10_000,
            max_sample_age_secs: 300,
            max_future_skew_secs: 30,
            signers: vec![HedgingTrustedSignerV1 {
                version: HEDGING_TRUSTED_SIGNER_VERSION_V1,
                signer_id: "collector-1".to_owned(),
                public_key: feed_key().verifying_key().to_bytes(),
                authorized_feeds: vec![HedgingFeedBindingV1 {
                    version: HEDGING_FEED_BINDING_VERSION_V1,
                    feed_id: "primary".to_owned(),
                    source: "primary-source".to_owned(),
                }],
            }],
            revoked_signer_ids: Vec::new(),
        }
    }
    fn governed_reference(
        policy: &HedgingFeedTrustPolicyV1,
    ) -> GovernedHedgingReferencePriceDecisionV1 {
        governed_reference_at(policy, PERIOD_END)
    }
    fn governed_reference_at(
        policy: &HedgingFeedTrustPolicyV1,
        period_end_unix: u64,
    ) -> GovernedHedgingReferencePriceDecisionV1 {
        let mut signed = SignedHedgingPriceFeedV1 {
            version: SIGNED_HEDGING_PRICE_FEED_VERSION_V1,
            policy_digest: policy.canonical_digest().expect("feed policy digest"),
            feed: HedgingPriceFeedV1 {
                version: HEDGING_PRICE_FEED_VERSION_V1,
                feed_id: "primary".to_owned(),
                source: "primary-source".to_owned(),
                observed_at_unix: period_end_unix - 10,
                xor_usd_price: usd("2"),
                weight_bps: 10_000,
                evidence_digest: [0x41; 32],
                status: HedgingFeedStatusV1::Ok,
            },
            signer_id: "collector-1".to_owned(),
            signature: [0; 64],
        };
        signed.signature = feed_key()
            .sign(&signed.signing_digest().expect("feed signing digest"))
            .to_bytes();
        derive_governed_reference_price_decision_v1(
            policy,
            vec![signed],
            period_end_unix,
            period_end_unix,
            300,
            500,
        )
        .expect("governed reference decision")
    }
    fn service_policy() -> HedgingBillingServicePolicyV1 {
        HedgingBillingServicePolicyV1 {
            version: HEDGING_BILLING_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            network_id: test_network_id(b"hedging-billing-test-genesis"),
            billing_policy_digest: [0x51; 32],
            feed_trust_policy_digest: feed_policy()
                .canonical_digest()
                .expect("feed policy digest"),
            billing_epoch_unix: EPOCH,
            billing_period_secs: PERIOD_SECS,
            payment_due_after_secs: 600,
            max_feed_age_secs: 300,
            max_divergence_bps: 500,
            statement_signer: BillingStatementSignerPolicyV1 {
                version: BILLING_STATEMENT_SIGNER_POLICY_VERSION_V1,
                signer_id: "billing-hsm-1".to_owned(),
                public_key: statement_key().verifying_key().to_bytes(),
                valid_from_block_height: 1,
                revoked_at_block_height: None,
            },
            statement_publisher: BillingStatementPublisherPolicyV1 {
                version: BILLING_STATEMENT_PUBLISHER_POLICY_VERSION_V1,
                publisher_id: "billing-publisher-1".to_owned(),
                route_id: "regional-gateway-1".to_owned(),
                public_key: publisher_key().verifying_key().to_bytes(),
            },
            transition_authority: HedgingBillingTransitionAuthorityV1 {
                version: HEDGING_BILLING_TRANSITION_AUTHORITY_VERSION_V1,
                authority_id: "billing-transition-hsm-1".to_owned(),
                public_key: transition_key().verifying_key().to_bytes(),
            },
            epoch_witness_store_handle: "billing-epoch-witness-test".to_owned(),
            hedge_intent_threshold_xor: xor("5"),
            max_hedge_intent_xor: xor("100"),
            hedge_intent_ttl_secs: 900,
            hedge_max_slippage_bps: 100,
            max_events_per_page: 64,
            max_retained_source_pages: 1_024,
            max_retained_period_closes: 256,
            max_accounts: 64,
            max_open_accruals: 256,
            max_replay_receipts: 1_024,
            max_statements: 256,
            max_acknowledgements: 256,
            max_hedge_intents: 64,
            max_signing_attempts: 3,
            checkpoint_max_bytes: 16 * 1024 * 1024,
        }
    }
    fn cursor(
        height: u64,
        hash: [u8; 32],
        finalized_at_unix: u64,
    ) -> HedgingBillingFinalizedCursorV1 {
        HedgingBillingFinalizedCursorV1 {
            height,
            block_hash: hash,
            finalized_at_unix,
        }
    }
    fn event(sequence: u64, source_id: &str, amount: &str) -> HedgingBillingFinalizedEventV1 {
        HedgingBillingFinalizedEventV1 {
            version: HEDGING_BILLING_FINALIZED_EVENT_VERSION_V1,
            sequence,
            block_height: 10,
            block_hash: FINALIZED_HASH,
            event_index: u32::try_from(sequence).expect("small sequence"),
            source: BillingAccrualSourceV1::Storage,
            account_id: primary_account_bytes(),
            source_id: source_id.to_owned(),
            direction: BillingLineDirectionV1::Debit,
            kind: BillingLineItemKindV1::Storage,
            xor_amount: xor(amount),
            quantity_units: 1,
            occurred_at_unix: EPOCH + 100,
        }
    }
    fn page(events: Vec<HedgingBillingFinalizedEventV1>) -> HedgingBillingFinalizedEventPageV1 {
        let start_sequence = events.first().map_or(1, |event| event.sequence);
        let next_sequence = start_sequence + u64::try_from(events.len()).expect("small page");
        let cursor_height = 9_u64.saturating_add(next_sequence);
        let cursor_hash = if cursor_height == 10 {
            FINALIZED_HASH
        } else {
            [u8::try_from(cursor_height).expect("small cursor height"); 32]
        };
        HedgingBillingFinalizedEventPageV1 {
            version: HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1,
            network_id: test_network_id(b"hedging-billing-test-genesis"),
            start_sequence,
            next_sequence,
            journal_commitment: HedgingBillingJournalCommitmentV1 {
                version: HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1,
                network_id: test_network_id(b"hedging-billing-test-genesis"),
                finalized_cursor: cursor(
                    cursor_height,
                    cursor_hash,
                    PERIOD_END.saturating_add(cursor_height.saturating_sub(10)),
                ),
                journal_next_sequence: next_sequence,
                journal_root: [u8::try_from(next_sequence).expect("small journal tail"); 32],
            },
            append_proof: vec![0xA5],
            inclusion_proof: vec![0xB6],
            events,
        }
    }
    fn period_close(
        reference: &GovernedHedgingReferencePriceDecisionV1,
        commitment: HedgingBillingJournalCommitmentV1,
    ) -> HedgingBillingFinalizedPeriodCloseV1 {
        let policy = service_policy();
        period_close_at(&policy, reference, commitment, PERIOD_END)
    }
    fn period_close_at(
        policy: &HedgingBillingServicePolicyV1,
        reference: &GovernedHedgingReferencePriceDecisionV1,
        commitment: HedgingBillingJournalCommitmentV1,
        period_end_unix: u64,
    ) -> HedgingBillingFinalizedPeriodCloseV1 {
        HedgingBillingFinalizedPeriodCloseV1 {
            version: HEDGING_BILLING_PERIOD_CLOSE_VERSION_V1,
            network_id: policy.network_id,
            period_end_unix,
            journal_commitment: commitment,
            billing_policy_digest: policy.billing_policy_digest,
            service_policy_digest: policy.digest().expect("service policy digest"),
            feed_trust_policy_digest: policy.feed_trust_policy_digest,
            feed_admitted_at_unix: commitment.finalized_cursor.finalized_at_unix,
            governed_reference_price: reference.clone(),
            authentication_proof: vec![0xC7],
        }
    }
    #[derive(Debug, Default)]
    struct TestJournalVerifier {
        page_calls: AtomicUsize,
        close_calls: AtomicUsize,
        transition_calls: AtomicUsize,
        reject_pages: AtomicBool,
        witness_records: Mutex<BTreeMap<u64, HedgingBillingEpochWitnessRecordV1>>,
        witness_ambiguous_after_write: AtomicBool,
        witness_fork_on_epoch_lookup: AtomicBool,
    }
    impl HedgingBillingRuntimeProviderV1 for TestJournalVerifier {
        fn handle(&self) -> &str {
            "billing-epoch-witness-test"
        }
        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(TEST_PROVIDER_QUALIFICATION)
        }
    }
    impl HedgingBillingJournalVerifier for TestJournalVerifier {
        fn identity(
            &self,
        ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError> {
            Ok(HedgingBillingRuntimeAdapterIdentityV1 {
                handle: "billing-journal-verifier-test".to_owned(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn verify_page(
            &self,
            network_id: &NetworkId,
            _previous: Option<HedgingBillingJournalCommitmentV1>,
            page: &HedgingBillingFinalizedEventPageV1,
        ) -> Result<(), HedgingBillingExternalError> {
            self.page_calls.fetch_add(1, Ordering::Relaxed);
            if self.reject_pages.load(Ordering::Relaxed)
                || network_id != &test_network_id(b"hedging-billing-test-genesis")
                || page.append_proof != [0xA5]
                || page.inclusion_proof != [0xB6]
            {
                return Err(HedgingBillingExternalError::Rejected);
            }
            Ok(())
        }
        fn verify_period_close(
            &self,
            network_id: &NetworkId,
            close: &HedgingBillingFinalizedPeriodCloseV1,
        ) -> Result<(), HedgingBillingExternalError> {
            self.close_calls.fetch_add(1, Ordering::Relaxed);
            if network_id != &test_network_id(b"hedging-billing-test-genesis")
                || close.authentication_proof != [0xC7]
            {
                return Err(HedgingBillingExternalError::Rejected);
            }
            Ok(())
        }
        fn verify_epoch_transition(
            &self,
            network_id: &NetworkId,
            transition: &HedgingBillingEpochTransitionV1,
        ) -> Result<(), HedgingBillingExternalError> {
            self.transition_calls.fetch_add(1, Ordering::Relaxed);
            if network_id != &test_network_id(b"hedging-billing-test-genesis")
                || transition.consensus_authentication_proof != [0xD8]
            {
                return Err(HedgingBillingExternalError::Rejected);
            }
            Ok(())
        }
    }
    impl HedgingBillingEpochWitnessStore for TestJournalVerifier {
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn load_latest(
            &self,
        ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError>
        {
            Ok(self
                .witness_records
                .lock()
                .expect("epoch witness state")
                .last_key_value()
                .map(|(_, record)| record.clone()))
        }
        fn load_epoch(
            &self,
            epoch_sequence: u64,
        ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError>
        {
            let mut record = self
                .witness_records
                .lock()
                .expect("epoch witness state")
                .get(&epoch_sequence)
                .cloned();
            if self.witness_fork_on_epoch_lookup.load(Ordering::Relaxed)
                && let Some(record) = &mut record
            {
                record.revision[0] ^= 0x80;
            }
            Ok(record)
        }
        fn compare_and_swap_latest(
            &self,
            expected_revision: Option<[u8; 32]>,
            next: &HedgingBillingEpochWitnessRecordV1,
        ) -> Result<(), HedgingBillingExternalError> {
            let mut records = self.witness_records.lock().expect("epoch witness state");
            let current = records.last_key_value().map(|(_, record)| record);
            if current.map(|record| record.revision) != expected_revision
                || current.map_or(1, |record| record.epoch_sequence.saturating_add(1))
                    != next.epoch_sequence
            {
                return Err(HedgingBillingExternalError::Rejected);
            }
            if let Some(existing) = records.get(&next.epoch_sequence)
                && existing != next
            {
                return Err(HedgingBillingExternalError::Rejected);
            }
            records.insert(next.epoch_sequence, next.clone());
            if self
                .witness_ambiguous_after_write
                .swap(false, Ordering::Relaxed)
            {
                return Err(HedgingBillingExternalError::Ambiguous);
            }
            Ok(())
        }
    }
    #[derive(Debug)]
    struct TestSigner {
        key: SigningKey,
        signer_id: String,
        corrupt: AtomicBool,
        sign_calls: AtomicUsize,
    }
    impl TestSigner {
        fn valid() -> Self {
            Self {
                key: statement_key(),
                signer_id: "billing-hsm-1".to_owned(),
                corrupt: AtomicBool::new(false),
                sign_calls: AtomicUsize::new(0),
            }
        }
        fn transition() -> Self {
            Self {
                key: transition_key(),
                signer_id: "billing-transition-hsm-1".to_owned(),
                corrupt: AtomicBool::new(false),
                sign_calls: AtomicUsize::new(0),
            }
        }
    }
    impl HedgingBillingRuntimeProviderV1 for TestSigner {
        fn handle(&self) -> &str {
            "billing-statement-hsm-test"
        }
        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(TEST_PROVIDER_QUALIFICATION)
        }
    }
    impl BillingStatementRuntimeSigner for TestSigner {
        fn identity(
            &self,
        ) -> Result<BillingStatementSignerIdentityV1, HedgingBillingExternalError> {
            Ok(BillingStatementSignerIdentityV1 {
                provider_handle: "billing-statement-hsm-test".to_owned(),
                signer_id: self.signer_id.clone(),
                public_key: self.key.verifying_key().to_bytes(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn sign_digest(&self, digest: [u8; 32]) -> Result<[u8; 64], HedgingBillingExternalError> {
            self.sign_calls.fetch_add(1, Ordering::Relaxed);
            let mut signature = self.key.sign(&digest).to_bytes();
            if self.corrupt.load(Ordering::Relaxed) {
                signature[0] ^= 0x80;
            }
            Ok(signature)
        }
    }
    #[derive(Debug)]
    struct TestPublisher {
        key: SigningKey,
        publications: Mutex<BTreeMap<[u8; 32], BillingStatementAuthoritativePublicationV1>>,
        ambiguous_once: AtomicBool,
        publish_calls: AtomicUsize,
    }
    impl TestPublisher {
        fn new(ambiguous_once: bool) -> Self {
            Self {
                key: publisher_key(),
                publications: Mutex::new(BTreeMap::new()),
                ambiguous_once: AtomicBool::new(ambiguous_once),
                publish_calls: AtomicUsize::new(0),
            }
        }
        fn receipt(
            &self,
            statement: &SignedGovernedBillingStatementV1,
        ) -> BillingStatementPublicationReceiptV1 {
            let bytes = norito::to_bytes(statement).expect("signed statement bytes");
            let mut receipt = BillingStatementPublicationReceiptV1 {
                version: BILLING_STATEMENT_PUBLICATION_RECEIPT_VERSION_V1,
                statement_id: statement.governed_statement.statement.statement_id,
                signed_statement_digest: *blake3::hash(&bytes).as_bytes(),
                route_id: "regional-gateway-1".to_owned(),
                publisher_id: "billing-publisher-1".to_owned(),
                published_at_unix: statement.signed_at_unix + 1,
                receipt_digest: [0; 32],
                signature: [0; 64],
            };
            receipt.receipt_digest =
                publication_receipt_digest(&receipt).expect("publication receipt digest");
            let mut message = Vec::new();
            message.extend_from_slice(PUBLISHER_RECEIPT_SIGNATURE_DOMAIN_V1);
            message.extend_from_slice(&receipt.receipt_digest);
            receipt.signature = self.key.sign(&message).to_bytes();
            receipt
        }
    }
    impl HedgingBillingRuntimeProviderV1 for TestPublisher {
        fn handle(&self) -> &str {
            "billing-publisher-test"
        }
        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(TEST_PROVIDER_QUALIFICATION)
        }
    }
    impl BillingStatementPublisher for TestPublisher {
        fn identity(
            &self,
        ) -> Result<BillingStatementPublisherIdentityV1, HedgingBillingExternalError> {
            Ok(BillingStatementPublisherIdentityV1 {
                provider_handle: "billing-publisher-test".to_owned(),
                publisher_id: "billing-publisher-1".to_owned(),
                route_id: "regional-gateway-1".to_owned(),
                public_key: self.key.verifying_key().to_bytes(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn publish(
            &self,
            idempotency_key: [u8; 32],
            signed_statement_digest: [u8; 32],
            statement: &SignedGovernedBillingStatementV1,
        ) -> Result<BillingStatementPublicationReceiptV1, HedgingBillingExternalError> {
            self.publish_calls.fetch_add(1, Ordering::Relaxed);
            let receipt = self.receipt(statement);
            if idempotency_key != receipt.statement_id
                || signed_statement_digest != receipt.signed_statement_digest
            {
                return Err(HedgingBillingExternalError::Rejected);
            }
            let publication = BillingStatementAuthoritativePublicationV1 {
                signed_statement: statement.clone(),
                receipt: receipt.clone(),
            };
            let mut publications = self.publications.lock().expect("publisher state");
            if let Some(existing) = publications.get(&receipt.statement_id) {
                if existing != &publication {
                    return Err(HedgingBillingExternalError::Rejected);
                }
            } else {
                publications.insert(receipt.statement_id, publication);
            }
            if self.ambiguous_once.swap(false, Ordering::Relaxed) {
                return Err(HedgingBillingExternalError::Ambiguous);
            }
            Ok(receipt)
        }
        fn lookup(
            &self,
            statement_id: [u8; 32],
        ) -> Result<Option<BillingStatementAuthoritativePublicationV1>, HedgingBillingExternalError>
        {
            Ok(self
                .publications
                .lock()
                .expect("publisher state")
                .get(&statement_id)
                .cloned())
        }
    }
    #[derive(Debug, Default)]
    struct TestAcknowledgementAuthority {
        records: Mutex<BTreeMap<[u8; 32], BillingStatementAcknowledgementV1>>,
    }
    impl HedgingBillingRuntimeProviderV1 for TestAcknowledgementAuthority {
        fn handle(&self) -> &str {
            "billing-acknowledgement-authority-test"
        }
        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(TEST_PROVIDER_QUALIFICATION)
        }
    }
    impl BillingStatementAcknowledgementAuthority for TestAcknowledgementAuthority {
        fn identity(
            &self,
        ) -> Result<BillingStatementAcknowledgementAuthorityIdentityV1, HedgingBillingExternalError>
        {
            Ok(BillingStatementAcknowledgementAuthorityIdentityV1 {
                provider_handle: "billing-acknowledgement-authority-test".to_owned(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn verify(
            &self,
            statement: &SignedGovernedBillingStatementV1,
            acknowledgement: &BillingStatementAcknowledgementV1,
        ) -> Result<(), HedgingBillingExternalError> {
            if acknowledgement.statement_id != statement.governed_statement.statement.statement_id
                || acknowledgement.authentication_proof != [0xAC]
            {
                return Err(HedgingBillingExternalError::Rejected);
            }
            Ok(())
        }
        fn record(
            &self,
            statement: &SignedGovernedBillingStatementV1,
            acknowledgement: &BillingStatementAcknowledgementV1,
        ) -> Result<BillingStatementAcknowledgementV1, HedgingBillingExternalError> {
            self.verify(statement, acknowledgement)?;
            let mut records = self.records.lock().expect("ack authority state");
            if let Some(existing) = records.get(&acknowledgement.statement_id) {
                if existing != acknowledgement {
                    return Err(HedgingBillingExternalError::Rejected);
                }
                return Ok(existing.clone());
            }
            records.insert(acknowledgement.statement_id, acknowledgement.clone());
            Ok(acknowledgement.clone())
        }
        fn lookup(
            &self,
            statement_id: [u8; 32],
        ) -> Result<Option<BillingStatementAcknowledgementV1>, HedgingBillingExternalError>
        {
            Ok(self
                .records
                .lock()
                .expect("ack authority state")
                .get(&statement_id)
                .cloned())
        }
    }
    #[derive(Debug)]
    struct TestFinalizedQuery {
        pages: Mutex<VecDeque<Option<HedgingBillingFinalizedEventPageV1>>>,
        head: HedgingBillingFinalizedCursorV1,
        calls: AtomicUsize,
        requested_max_events: Mutex<Vec<u32>>,
        positions: Mutex<Vec<HedgingBillingQueryPositionV1>>,
    }
    impl TestFinalizedQuery {
        fn new(pages: Vec<Option<HedgingBillingFinalizedEventPageV1>>) -> Self {
            let head = pages
                .iter()
                .flatten()
                .map(|page| page.journal_commitment.finalized_cursor)
                .max_by_key(|cursor| cursor.height)
                .unwrap_or_else(|| cursor(10, FINALIZED_HASH, EPOCH + 60));
            Self {
                pages: Mutex::new(pages.into()),
                head,
                calls: AtomicUsize::new(0),
                requested_max_events: Mutex::new(Vec::new()),
                positions: Mutex::new(Vec::new()),
            }
        }
    }
    impl HedgingBillingRuntimeProviderV1 for TestFinalizedQuery {
        fn handle(&self) -> &str {
            "billing-finalized-query-test"
        }
        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(TEST_PROVIDER_QUALIFICATION)
        }
    }
    impl HedgingBillingFinalizedQuery for TestFinalizedQuery {
        fn identity(
            &self,
        ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError> {
            Ok(HedgingBillingRuntimeAdapterIdentityV1 {
                handle: "billing-finalized-query-test".to_owned(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn supplies_period_closes(&self) -> bool {
            true
        }
        fn finalized_head(
            &self,
        ) -> Result<HedgingBillingFinalizedCursorV1, HedgingBillingExternalError> {
            Ok(self.head)
        }
        fn query_finalized_page(
            &self,
            position: HedgingBillingQueryPositionV1,
            max_events: u32,
        ) -> Result<Option<HedgingBillingFinalizedEventPageV1>, HedgingBillingExternalError>
        {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.requested_max_events
                .lock()
                .expect("requested max-events state")
                .push(max_events);
            self.positions
                .lock()
                .expect("query-position state")
                .push(position);
            Ok(self
                .pages
                .lock()
                .expect("query page state")
                .pop_front()
                .unwrap_or(None))
        }
        fn query_finalized_period_close(
            &self,
            _period_end_unix: u64,
            _position: HedgingBillingQueryPositionV1,
        ) -> Result<Option<HedgingBillingFinalizedPeriodCloseV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    enum DriftingProviderOperation {
        None = 0,
        FinalizedQuery = 1,
        JournalVerification = 2,
        StatementSigning = 3,
        StatementPublication = 4,
        AcknowledgementRecord = 5,
        EpochWitnessCas = 6,
    }
    #[derive(Debug)]
    struct DriftingRuntimeProvider {
        handle: String,
        qualification: Mutex<HedgingBillingRuntimeProviderQualificationV1>,
        drift_on: AtomicU8,
        calls: AtomicUsize,
    }
    impl DriftingRuntimeProvider {
        fn new(handle: &str, drift_on: DriftingProviderOperation) -> Self {
            Self {
                handle: handle.to_owned(),
                qualification: Mutex::new(TEST_PROVIDER_QUALIFICATION),
                drift_on: AtomicU8::new(drift_on as u8),
                calls: AtomicUsize::new(0),
            }
        }
        fn set_qualification(&self, qualification: HedgingBillingRuntimeProviderQualificationV1) {
            *self.qualification.lock().expect("qualification state") = qualification;
        }
        fn drift_after(&self, operation: DriftingProviderOperation) {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self
                .drift_on
                .compare_exchange(
                    operation as u8,
                    DriftingProviderOperation::None as u8,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                self.set_qualification(HedgingBillingRuntimeProviderQualificationV1::new(
                    2, [0xB2; 32],
                ));
            }
        }
    }
    impl HedgingBillingRuntimeProviderV1 for DriftingRuntimeProvider {
        fn handle(&self) -> &str {
            &self.handle
        }
        fn qualification(
            &self,
        ) -> Result<
            HedgingBillingRuntimeProviderQualificationV1,
            HedgingBillingRuntimeProviderReadinessErrorV1,
        > {
            Ok(*self.qualification.lock().expect("qualification state"))
        }
    }
    impl HedgingBillingFinalizedQuery for DriftingRuntimeProvider {
        fn identity(
            &self,
        ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError> {
            Ok(HedgingBillingRuntimeAdapterIdentityV1 {
                handle: self.handle.clone(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn supplies_period_closes(&self) -> bool {
            true
        }
        fn finalized_head(
            &self,
        ) -> Result<HedgingBillingFinalizedCursorV1, HedgingBillingExternalError> {
            self.drift_after(DriftingProviderOperation::FinalizedQuery);
            Ok(cursor(10, FINALIZED_HASH, PERIOD_END))
        }
        fn query_finalized_page(
            &self,
            _position: HedgingBillingQueryPositionV1,
            _max_events: u32,
        ) -> Result<Option<HedgingBillingFinalizedEventPageV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
        fn query_finalized_period_close(
            &self,
            _period_end_unix: u64,
            _position: HedgingBillingQueryPositionV1,
        ) -> Result<Option<HedgingBillingFinalizedPeriodCloseV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
    }
    impl HedgingBillingJournalVerifier for DriftingRuntimeProvider {
        fn identity(
            &self,
        ) -> Result<HedgingBillingRuntimeAdapterIdentityV1, HedgingBillingExternalError> {
            Ok(HedgingBillingRuntimeAdapterIdentityV1 {
                handle: self.handle.clone(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn verify_page(
            &self,
            _network_id: &NetworkId,
            _previous: Option<HedgingBillingJournalCommitmentV1>,
            _page: &HedgingBillingFinalizedEventPageV1,
        ) -> Result<(), HedgingBillingExternalError> {
            self.drift_after(DriftingProviderOperation::JournalVerification);
            Ok(())
        }
        fn verify_period_close(
            &self,
            _network_id: &NetworkId,
            _close: &HedgingBillingFinalizedPeriodCloseV1,
        ) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn verify_epoch_transition(
            &self,
            _network_id: &NetworkId,
            _transition: &HedgingBillingEpochTransitionV1,
        ) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
    }
    impl BillingStatementRuntimeSigner for DriftingRuntimeProvider {
        fn identity(
            &self,
        ) -> Result<BillingStatementSignerIdentityV1, HedgingBillingExternalError> {
            Ok(BillingStatementSignerIdentityV1 {
                provider_handle: self.handle.clone(),
                signer_id: "billing-hsm-1".to_owned(),
                public_key: statement_key().verifying_key().to_bytes(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn sign_digest(&self, digest: [u8; 32]) -> Result<[u8; 64], HedgingBillingExternalError> {
            let signature = statement_key().sign(&digest).to_bytes();
            self.drift_after(DriftingProviderOperation::StatementSigning);
            Ok(signature)
        }
    }
    impl BillingStatementPublisher for DriftingRuntimeProvider {
        fn identity(
            &self,
        ) -> Result<BillingStatementPublisherIdentityV1, HedgingBillingExternalError> {
            Ok(BillingStatementPublisherIdentityV1 {
                provider_handle: self.handle.clone(),
                publisher_id: "billing-publisher-1".to_owned(),
                route_id: "regional-gateway-1".to_owned(),
                public_key: publisher_key().verifying_key().to_bytes(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn publish(
            &self,
            _idempotency_key: [u8; 32],
            _signed_statement_digest: [u8; 32],
            _statement: &SignedGovernedBillingStatementV1,
        ) -> Result<BillingStatementPublicationReceiptV1, HedgingBillingExternalError> {
            self.drift_after(DriftingProviderOperation::StatementPublication);
            Err(HedgingBillingExternalError::Rejected)
        }
        fn lookup(
            &self,
            _statement_id: [u8; 32],
        ) -> Result<Option<BillingStatementAuthoritativePublicationV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
    }
    impl BillingStatementAcknowledgementAuthority for DriftingRuntimeProvider {
        fn identity(
            &self,
        ) -> Result<BillingStatementAcknowledgementAuthorityIdentityV1, HedgingBillingExternalError>
        {
            Ok(BillingStatementAcknowledgementAuthorityIdentityV1 {
                provider_handle: self.handle.clone(),
            })
        }
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn verify(
            &self,
            _statement: &SignedGovernedBillingStatementV1,
            _acknowledgement: &BillingStatementAcknowledgementV1,
        ) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn record(
            &self,
            _statement: &SignedGovernedBillingStatementV1,
            acknowledgement: &BillingStatementAcknowledgementV1,
        ) -> Result<BillingStatementAcknowledgementV1, HedgingBillingExternalError> {
            self.drift_after(DriftingProviderOperation::AcknowledgementRecord);
            Ok(acknowledgement.clone())
        }
        fn lookup(
            &self,
            _statement_id: [u8; 32],
        ) -> Result<Option<BillingStatementAcknowledgementV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
    }
    impl HedgingBillingEpochWitnessStore for DriftingRuntimeProvider {
        fn check_readiness(&self) -> Result<(), HedgingBillingExternalError> {
            Ok(())
        }
        fn load_latest(
            &self,
        ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
        fn load_epoch(
            &self,
            _epoch_sequence: u64,
        ) -> Result<Option<HedgingBillingEpochWitnessRecordV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
        fn compare_and_swap_latest(
            &self,
            _expected_revision: Option<[u8; 32]>,
            _next: &HedgingBillingEpochWitnessRecordV1,
        ) -> Result<(), HedgingBillingExternalError> {
            self.drift_after(DriftingProviderOperation::EpochWitnessCas);
            Ok(())
        }
    }
    fn qualified_drifting_provider(
        operation: DriftingProviderOperation,
    ) -> (
        Arc<DriftingRuntimeProvider>,
        QualifiedHedgingBillingRuntimeProviderV1<DriftingRuntimeProvider>,
    ) {
        let provider = Arc::new(DriftingRuntimeProvider::new(
            "billing.external.primary",
            operation,
        ));
        let qualified = QualifiedHedgingBillingRuntimeProviderV1::try_new(
            "billing.external.primary",
            TEST_PROVIDER_QUALIFICATION,
            Arc::clone(&provider),
        )
        .expect("initial provider qualification");
        (provider, qualified)
    }
    fn ready_service(
        root: &Path,
    ) -> (
        HedgingBillingService,
        HedgingFeedTrustPolicyV1,
        GovernedHedgingReferencePriceDecisionV1,
        Arc<TestJournalVerifier>,
        Arc<TestPublisher>,
        Arc<TestAcknowledgementAuthority>,
    ) {
        let feed_policy = feed_policy();
        let reference = governed_reference(&feed_policy);
        let journal_verifier = Arc::new(TestJournalVerifier::default());
        let publisher = Arc::new(TestPublisher::new(false));
        let acknowledgement_authority = Arc::new(TestAcknowledgementAuthority::default());
        let service = HedgingBillingService::new(
            root,
            service_policy(),
            feed_policy.clone(),
            journal_verifier.clone(),
            publisher.clone(),
            acknowledgement_authority.clone(),
            journal_verifier.clone(),
        )
        .expect("initialize service");
        (
            service,
            feed_policy,
            reference,
            journal_verifier,
            publisher,
            acknowledgement_authority,
        )
    }
    fn settle_first_period(
        service: &HedgingBillingService,
        reference: &GovernedHedgingReferencePriceDecisionV1,
    ) -> HedgingBillingPeriodOutcomeV1 {
        let first = page(vec![event(1, "storage:event:1", "10")]);
        let close = period_close(reference, first.journal_commitment);
        settle_period(service, first, close)
    }
    fn settle_period(
        service: &HedgingBillingService,
        page: HedgingBillingFinalizedEventPageV1,
        close: HedgingBillingFinalizedPeriodCloseV1,
    ) -> HedgingBillingPeriodOutcomeV1 {
        service
            .ingest_finalized_page(&page)
            .expect("committed source page");
        let outcome = service
            .finalize_next_period(&close)
            .expect("committed close");
        service
            .sign_next_statement(&TestSigner::valid())
            .expect("sign")
            .expect("signed statement");
        let receipt = service
            .publish_next_statement()
            .expect("publish")
            .expect("publication receipt");
        service
            .acknowledge_statement(
                receipt.statement_id,
                &primary_account_bytes(),
                [0x74; 32],
                receipt.published_at_unix + 1,
                vec![0xAC],
            )
            .expect("authoritative acknowledgement");
        outcome
    }
    fn signed_statement_fixture() -> SignedGovernedBillingStatementV1 {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:qualification:1", "10")]);
        let close = period_close(&reference, first.journal_commitment);
        service
            .ingest_finalized_page(&first)
            .expect("committed source page");
        service
            .finalize_next_period(&close)
            .expect("committed close");
        service
            .sign_next_statement(&TestSigner::valid())
            .expect("sign statement")
            .expect("signed statement")
    }
    #[test]
    fn runtime_provider_handles_use_canonical_production_grammar() {
        for handle in [
            "hsm://sorafs/billing/statement-primary",
            "sealed://sorafs/billing/epoch-primary",
        ] {
            assert_eq!(
                validate_hedging_billing_runtime_provider_handle(handle, true),
                Ok(())
            );
        }
        for handle in [
            "hsm://sorafs/billing/operator@statement",
            "hsm://sorafs/billing/statement?token",
            "hsm://sorafs/billing/statement#fragment",
            "hsm://sorafs/billing/%73tatement",
            "hsm://sorafs/billing/statement\\primary",
        ] {
            assert_eq!(
                validate_hedging_billing_runtime_provider_handle(handle, true),
                Err(HedgingBillingRuntimeProviderQualificationErrorV1::InvalidConfiguredHandle)
            );
            assert_eq!(
                validate_hedging_billing_runtime_provider_handle(handle, false),
                Err(HedgingBillingRuntimeProviderQualificationErrorV1::InvalidProviderHandle)
            );
        }
        assert_eq!(
            validate_hedging_billing_runtime_provider_handle("hsm://sorafs/billing/dummy", true,),
            Err(HedgingBillingRuntimeProviderQualificationErrorV1::TestMarkedConfiguredHandle)
        );
        assert_eq!(
            validate_hedging_billing_runtime_provider_handle("hsm://sorafs/billing/dummy", false,),
            Err(HedgingBillingRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle)
        );
    }
    #[test]
    fn provider_qualification_rejects_unsafe_substituted_and_stale_bindings() {
        let unsafe_provider =
            DriftingRuntimeProvider::new("billing.test.primary", DriftingProviderOperation::None);
        assert_eq!(
            qualify_hedging_billing_runtime_provider_v1(
                "billing.external.primary",
                TEST_PROVIDER_QUALIFICATION,
                &unsafe_provider,
            ),
            Err(HedgingBillingRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle)
        );
        let provider = DriftingRuntimeProvider::new(
            "billing.external.primary",
            DriftingProviderOperation::None,
        );
        assert_eq!(
            qualify_hedging_billing_runtime_provider_v1(
                "billing.external.secondary",
                TEST_PROVIDER_QUALIFICATION,
                &provider,
            ),
            Err(HedgingBillingRuntimeProviderQualificationErrorV1::SubstitutedProvider)
        );
        provider.set_qualification(HedgingBillingRuntimeProviderQualificationV1::new(
            2, [0xC2; 32],
        ));
        assert_eq!(
            qualify_hedging_billing_runtime_provider_v1(
                "billing.external.primary",
                TEST_PROVIDER_QUALIFICATION,
                &provider,
            ),
            Err(HedgingBillingRuntimeProviderQualificationErrorV1::QualificationMismatch)
        );
    }
    #[test]
    fn provider_drift_before_an_operation_suppresses_the_call() {
        let (provider, qualified) = qualified_drifting_provider(DriftingProviderOperation::None);
        provider.set_qualification(HedgingBillingRuntimeProviderQualificationV1::new(
            2, [0xD2; 32],
        ));
        assert_eq!(
            HedgingBillingFinalizedQuery::finalized_head(&qualified),
            Err(HedgingBillingExternalError::Unavailable)
        );
        assert_eq!(provider.calls.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn read_and_signing_drift_discards_provider_results() {
        let (query, qualified_query) =
            qualified_drifting_provider(DriftingProviderOperation::FinalizedQuery);
        assert_eq!(
            HedgingBillingFinalizedQuery::finalized_head(&qualified_query),
            Err(HedgingBillingExternalError::Unavailable)
        );
        assert_eq!(query.calls.load(Ordering::Relaxed), 1);
        let (verifier, qualified_verifier) =
            qualified_drifting_provider(DriftingProviderOperation::JournalVerification);
        assert_eq!(
            HedgingBillingJournalVerifier::verify_page(
                &qualified_verifier,
                &test_network_id(b"hedging-billing-test-genesis"),
                None,
                &page(Vec::new()),
            ),
            Err(HedgingBillingExternalError::Unavailable)
        );
        assert_eq!(verifier.calls.load(Ordering::Relaxed), 1);
        let (signer, qualified_signer) =
            qualified_drifting_provider(DriftingProviderOperation::StatementSigning);
        assert_eq!(
            BillingStatementRuntimeSigner::sign_digest(&qualified_signer, [0x7A; 32]),
            Err(HedgingBillingExternalError::Unavailable)
        );
        assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn committing_provider_drift_requires_immutable_reconciliation() {
        let signed = signed_statement_fixture();
        let (publisher, qualified_publisher) =
            qualified_drifting_provider(DriftingProviderOperation::StatementPublication);
        assert_eq!(
            BillingStatementPublisher::publish(
                &qualified_publisher,
                signed.governed_statement.statement.statement_id,
                signed_statement_digest(&signed).expect("signed statement digest"),
                &signed,
            ),
            Err(HedgingBillingExternalError::Ambiguous)
        );
        assert_eq!(publisher.calls.load(Ordering::Relaxed), 1);
        let acknowledgement = BillingStatementAcknowledgementV1 {
            version: BILLING_STATEMENT_ACKNOWLEDGEMENT_VERSION_V1,
            network_id: test_network_id(b"hedging-billing-test-genesis"),
            statement_id: signed.governed_statement.statement.statement_id,
            account_digest: [0x81; 32],
            request_binding_digest: [0x82; 32],
            acknowledged_at_unix: signed.signed_at_unix.saturating_add(1),
            authentication_proof: vec![0x83],
            acknowledgement_id: [0x84; 32],
        };
        let (authority, qualified_authority) =
            qualified_drifting_provider(DriftingProviderOperation::AcknowledgementRecord);
        assert_eq!(
            BillingStatementAcknowledgementAuthority::record(
                &qualified_authority,
                &signed,
                &acknowledgement,
            ),
            Err(HedgingBillingExternalError::Ambiguous)
        );
        assert_eq!(authority.calls.load(Ordering::Relaxed), 1);
        let witness = HedgingBillingEpochWitnessRecordV1 {
            version: HEDGING_BILLING_EPOCH_WITNESS_RECORD_VERSION_V1,
            network_id: test_network_id(b"hedging-billing-test-genesis"),
            epoch_sequence: 1,
            transition_id: [0x85; 32],
            checkpoint_digest: [0x86; 32],
            checkpoint_bytes: vec![0x87],
            revision: [0x88; 32],
        };
        let (store, qualified_store) =
            qualified_drifting_provider(DriftingProviderOperation::EpochWitnessCas);
        assert_eq!(
            HedgingBillingEpochWitnessStore::compare_and_swap_latest(
                &qualified_store,
                None,
                &witness,
            ),
            Err(HedgingBillingExternalError::Ambiguous)
        );
        assert_eq!(store.calls.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn finalized_query_reconciliation_is_bounded_and_stops_on_empty_progress() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, _reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first_query = TestFinalizedQuery::new(vec![
            Some(page(vec![event(1, "storage:event:1", "10")])),
            Some(page(vec![event(2, "storage:event:2", "2")])),
        ]);
        let first_head = first_query.finalized_head().expect("first query head");
        assert!(matches!(
            service.reconcile_finalized_query(&first_query, 0, first_head),
            Err(HedgingBillingServiceError::InvalidQueryBound)
        ));
        assert!(matches!(
            service.reconcile_finalized_query(
                &first_query,
                HEDGING_BILLING_MAX_PAGES_PER_SCAN_V1 + 1,
                first_head,
            ),
            Err(HedgingBillingServiceError::InvalidQueryBound)
        ));
        assert_eq!(first_query.calls.load(Ordering::Relaxed), 0);
        let first = service
            .reconcile_finalized_query(&first_query, 1, first_head)
            .expect("bounded first scan");
        assert_eq!(
            first,
            HedgingBillingReconcileOutcomeV1 {
                pages_applied: 1,
                events_applied: 1,
                next_sequence: 2,
                finalized_cursor: Some(
                    page(vec![event(1, "storage:event:1", "10")])
                        .journal_commitment
                        .finalized_cursor,
                ),
            }
        );
        assert_eq!(first_query.calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            first_query
                .requested_max_events
                .lock()
                .expect("requested max-events state")
                .as_slice(),
            &[service_policy().max_events_per_page]
        );
        let finality_only_cursor = cursor(12, [0xB2; 32], PERIOD_END + 2);
        let first_commitment = page(vec![event(1, "storage:event:1", "10")]).journal_commitment;
        let finality_only_page = HedgingBillingFinalizedEventPageV1 {
            version: HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1,
            network_id: test_network_id(b"hedging-billing-test-genesis"),
            start_sequence: 2,
            next_sequence: 2,
            journal_commitment: HedgingBillingJournalCommitmentV1 {
                version: HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1,
                network_id: test_network_id(b"hedging-billing-test-genesis"),
                finalized_cursor: finality_only_cursor,
                journal_next_sequence: 2,
                journal_root: first_commitment.journal_root,
            },
            append_proof: vec![0xA5],
            inclusion_proof: vec![0xB6],
            events: Vec::new(),
        };
        let empty_query = TestFinalizedQuery::new(vec![
            Some(finality_only_page.clone()),
            Some(finality_only_page),
        ]);
        let empty_head = empty_query.finalized_head().expect("empty query head");
        let empty = service
            .reconcile_finalized_query(&empty_query, 10, empty_head)
            .expect("finality-only scan");
        assert_eq!(
            empty,
            HedgingBillingReconcileOutcomeV1 {
                pages_applied: 1,
                events_applied: 0,
                next_sequence: 2,
                finalized_cursor: Some(finality_only_cursor),
            }
        );
        assert_eq!(
            empty_query.calls.load(Ordering::Relaxed),
            1,
            "a finality-only page must terminate the scan"
        );
        assert_eq!(
            empty_query
                .positions
                .lock()
                .expect("query-position state")
                .as_slice(),
            &[HedgingBillingQueryPositionV1 {
                next_sequence: 2,
                journal_commitment: Some(first_commitment),
            }]
        );
        assert_eq!(
            service
                .reconcile_finalized_query(&empty_query, 10, empty_head)
                .expect("exact finality-only replay"),
            HedgingBillingReconcileOutcomeV1 {
                pages_applied: 0,
                events_applied: 0,
                next_sequence: 2,
                finalized_cursor: Some(finality_only_cursor),
            }
        );
        assert_eq!(empty_query.calls.load(Ordering::Relaxed), 2);
        let bounded_root = tempfile::tempdir().expect("bounded state root");
        let (bounded_service, ..) = ready_service(bounded_root.path());
        let beyond_head_query =
            TestFinalizedQuery::new(vec![Some(page(vec![event(1, "storage:beyond-head", "1")]))]);
        let query_head = beyond_head_query
            .finalized_head()
            .expect("beyond-head query cursor");
        let earlier_head = HedgingBillingFinalizedCursorV1 {
            height: query_head.height - 1,
            block_hash: [0x31; 32],
            finalized_at_unix: query_head.finalized_at_unix - 1,
        };
        assert_eq!(
            bounded_service
                .reconcile_finalized_query(&beyond_head_query, 1, earlier_head)
                .expect_err("a page beyond the authenticated scan head must fail before ingest"),
            HedgingBillingServiceError::FinalizedForkOrRollback
        );
        assert_eq!(
            bounded_service
                .query_position()
                .expect("unchanged bounded query position")
                .next_sequence,
            1
        );
    }
    #[test]
    fn committed_accrual_to_acknowledgement_survives_restart() {
        let root = tempfile::tempdir().expect("state root");
        let (service, feed_policy, reference, verifier, publisher, ack_authority) =
            ready_service(root.path());
        let first_page = page(vec![event(1, "storage:event:1", "10")]);
        assert_eq!(
            service
                .ingest_finalized_page(&first_page)
                .expect("ingest page"),
            HedgingBillingIngestOutcomeV1::Applied {
                event_count: 1,
                next_sequence: 2,
            }
        );
        assert_eq!(
            service
                .ingest_finalized_page(&first_page)
                .expect("exact replay"),
            HedgingBillingIngestOutcomeV1::Replay { next_sequence: 2 }
        );
        let close = period_close(&reference, first_page.journal_commitment);
        let period = service
            .finalize_next_period(&close)
            .expect("finalize period");
        assert_eq!(period.statement_ids.len(), 1);
        assert_eq!(
            service
                .finalize_next_period(&close)
                .expect("idempotent close replay"),
            period
        );
        assert!(
            !period
                .hedge_intent
                .as_ref()
                .expect("hedge intent")
                .automatic_execution
        );
        assert_eq!(
            period
                .hedge_intent
                .as_ref()
                .expect("hedge intent")
                .disposition,
            HedgeIntentDispositionV1::Executable
        );
        let signer = TestSigner::valid();
        let signed = service
            .sign_next_statement(&signer)
            .expect("sign statement")
            .expect("ready statement");
        assert_eq!(signer.sign_calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            signed.signed_at_unix,
            close.journal_commitment.finalized_cursor.finalized_at_unix
        );
        signed
            .verify(&service_policy(), &feed_policy, &close)
            .expect("signed statement verifies");
        let receipt = service
            .publish_next_statement()
            .expect("publish statement")
            .expect("publication receipt");
        assert_eq!(receipt.statement_id, period.statement_ids[0]);
        let mut forged_receipt = receipt.clone();
        forged_receipt.signature[0] ^= 0x80;
        assert!(matches!(
            forged_receipt.validate(&signed, &service_policy().statement_publisher),
            Err(HedgingBillingServiceError::InvalidPublicationReceipt)
        ));
        let mut terminal_timestamp_receipt = receipt.clone();
        terminal_timestamp_receipt.published_at_unix = u64::MAX;
        assert!(matches!(
            terminal_timestamp_receipt.validate(&signed, &service_policy().statement_publisher),
            Err(HedgingBillingServiceError::InvalidPublicationReceipt)
        ));
        assert!(matches!(
            service.acknowledge_statement(
                receipt.statement_id,
                &primary_account_bytes(),
                [0x6F; 32],
                u64::MAX,
                vec![0xAC],
            ),
            Err(HedgingBillingServiceError::InvalidAcknowledgement)
        ));
        assert!(matches!(
            service.acknowledge_statement(
                receipt.statement_id,
                &primary_account_bytes(),
                [0x70; 32],
                receipt.published_at_unix + 1,
                vec![0xAD],
            ),
            Err(HedgingBillingServiceError::External(
                HedgingBillingExternalError::Rejected
            ))
        ));
        assert_eq!(
            service
                .statement_delivery_projections()
                .expect("delivery projection after rejected acknowledgement")[0]
                .status,
            BillingStatementDeliveryStatusV1::Published
        );
        let acknowledgement = service
            .acknowledge_statement(
                receipt.statement_id,
                &primary_account_bytes(),
                [0x71; 32],
                receipt.published_at_unix + 1,
                vec![0xAC],
            )
            .expect("acknowledge");
        assert_eq!(acknowledgement.statement_id, receipt.statement_id);
        assert_eq!(
            service
                .acknowledge_statement(
                    receipt.statement_id,
                    &primary_account_bytes(),
                    [0x71; 32],
                    receipt.published_at_unix + 30,
                    vec![0xAC],
                )
                .expect("idempotent acknowledgement retry"),
            acknowledgement
        );
        drop(service);
        let restored = HedgingBillingService::new(
            root.path(),
            service_policy(),
            feed_policy,
            verifier.clone(),
            publisher,
            ack_authority,
            verifier,
        )
        .expect("restore service");
        assert_eq!(
            restored
                .statement_delivery_projections()
                .expect("delivery projections")[0]
                .status,
            BillingStatementDeliveryStatusV1::Acknowledged
        );
        assert_eq!(
            restored
                .published_statement(receipt.statement_id)
                .expect("published statement"),
            signed
        );
    }
    #[test]
    fn checkpoint_rejects_omitted_intent_and_acknowledgement_substitution() {
        let root = tempfile::tempdir().expect("state root");
        let (service, feed_policy, reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first_page = page(vec![event(1, "storage:event:1", "10")]);
        service.ingest_finalized_page(&first_page).expect("ingest");
        let close = period_close(&reference, first_page.journal_commitment);
        let period = service.finalize_next_period(&close).expect("finalize");
        service
            .sign_next_statement(&TestSigner::valid())
            .expect("sign");
        let receipt = service
            .publish_next_statement()
            .expect("publish")
            .expect("publication receipt");
        service
            .acknowledge_statement(
                receipt.statement_id,
                &primary_account_bytes(),
                [0x72; 32],
                receipt.published_at_unix + 1,
                vec![0xAC],
            )
            .expect("acknowledge");
        let checkpoint = service
            .state
            .lock()
            .expect("service state")
            .checkpoint
            .clone();
        assert_eq!(checkpoint.hedge_intents.len(), 1);
        assert_eq!(
            period.hedge_intent,
            checkpoint.hedge_intents.first().cloned()
        );
        let mut omitted_intent = checkpoint.clone();
        omitted_intent.hedge_intents.clear();
        assert!(matches!(
            omitted_intent.validate(&service_policy(), &feed_policy),
            Err(HedgingBillingServiceError::InvalidCheckpoint)
        ));
        let mut substituted_acknowledgement = checkpoint;
        let acknowledgement = substituted_acknowledgement
            .acknowledgements
            .first_mut()
            .expect("retained acknowledgement");
        acknowledgement.account_digest = [0x99; 32];
        acknowledgement.acknowledgement_id =
            acknowledgement_digest(acknowledgement).expect("acknowledgement digest");
        assert!(matches!(
            substituted_acknowledgement.validate(&service_policy(), &feed_policy),
            Err(HedgingBillingServiceError::InvalidCheckpoint)
        ));
    }
    #[test]
    fn finalized_forks_gaps_semantic_replay_and_late_events_fail_closed() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first_page = page(vec![event(1, "storage:event:1", "10")]);
        service
            .ingest_finalized_page(&first_page)
            .expect("ingest first");
        let mut gap = page(vec![event(3, "storage:event:3", "1")]);
        gap.start_sequence = 3;
        gap.next_sequence = 4;
        assert!(matches!(
            service.ingest_finalized_page(&gap),
            Err(HedgingBillingServiceError::FinalizedSequenceGap)
        ));
        let mut fork = HedgingBillingFinalizedEventPageV1 {
            version: HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1,
            network_id: service_policy().network_id,
            start_sequence: 2,
            next_sequence: 2,
            journal_commitment: first_page.journal_commitment,
            append_proof: vec![0xA5],
            inclusion_proof: vec![0xB6],
            events: Vec::new(),
        };
        fork.journal_commitment.finalized_cursor.block_hash = [0xB2; 32];
        assert!(matches!(
            service.ingest_finalized_page(&fork),
            Err(HedgingBillingServiceError::FinalizedForkOrRollback)
        ));
        let mut duplicate_event = event(2, "storage:event:1", "10");
        duplicate_event.account_id = account_bytes(0x92);
        let duplicate = page(vec![duplicate_event]);
        assert!(matches!(
            service.ingest_finalized_page(&duplicate),
            Err(HedgingBillingServiceError::DuplicateBillingSource)
        ));
        service
            .finalize_next_period(&period_close(&reference, first_page.journal_commitment))
            .expect("finalize");
        let mut late = event(2, "storage:event:2", "1");
        late.occurred_at_unix = PERIOD_END - 1;
        assert!(matches!(
            service.ingest_finalized_page(&page(vec![late])),
            Err(HedgingBillingServiceError::LateFinalizedEvent)
        ));
    }
    #[test]
    fn incomplete_consensus_tail_cannot_close_or_sign() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let mut incomplete = page(vec![event(1, "storage:event:1", "10")]);
        incomplete.journal_commitment.journal_next_sequence = 3;
        incomplete.journal_commitment.journal_root = [3; 32];
        service
            .ingest_finalized_page(&incomplete)
            .expect("authenticated prefix page");
        let close = period_close(&reference, incomplete.journal_commitment);
        assert!(matches!(
            service.finalize_next_period(&close),
            Err(HedgingBillingServiceError::InvalidPeriodClose)
        ));
        assert!(
            service
                .statement_delivery_projections()
                .expect("no statements")
                .is_empty()
        );
        assert!(
            service
                .sign_next_statement(&TestSigner::valid())
                .expect("no signable work")
                .is_none()
        );
    }
    #[test]
    fn governed_overflow_projection_never_blocks_billing_close() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:event:overflow", "101")]);
        service
            .ingest_finalized_page(&first)
            .expect("committed source page");
        let outcome = service
            .finalize_next_period(&period_close(&reference, first.journal_commitment))
            .expect("overflow must not block billing closure");
        assert_eq!(outcome.statement_ids.len(), 1);
        let intent = outcome
            .hedge_intent
            .expect("explicit governed overflow projection");
        assert_eq!(
            intent.disposition,
            HedgeIntentDispositionV1::GovernedOverflow
        );
        assert_eq!(intent.xor_amount, xor("101"));
        assert!(!intent.automatic_execution);
        let adapter = TestExecutionAdapter::new();
        assert!(matches!(
            service.submit_authorized_hedge_intent(
                &execution_policy(),
                &execution_authorization(&intent),
                &adapter,
            ),
            Err(HedgingBillingServiceError::InvalidHedgeExecutionAuthorization)
        ));
        assert_eq!(adapter.submit_calls.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn policy_reserves_acknowledgement_capacity_for_every_statement() {
        let mut policy = service_policy();
        policy.max_acknowledgements = policy.max_statements - 1;
        assert!(matches!(
            policy.validate(),
            Err(HedgingBillingServiceError::InvalidPolicy)
        ));
    }
    #[test]
    fn retained_history_bound_requires_an_authenticated_epoch_transition() {
        let root = tempfile::tempdir().expect("state root");
        let feed_policy = feed_policy();
        let mut policy = service_policy();
        policy.max_retained_source_pages = 1;
        let verifier = Arc::new(TestJournalVerifier::default());
        let service = HedgingBillingService::new(
            root.path(),
            policy,
            feed_policy,
            verifier.clone(),
            Arc::new(TestPublisher::new(false)),
            Arc::new(TestAcknowledgementAuthority::default()),
            verifier,
        )
        .expect("initialize bounded service");
        service
            .ingest_finalized_page(&page(vec![event(1, "storage:event:1", "10")]))
            .expect("first retained page");
        assert!(matches!(
            service.ingest_finalized_page(&page(vec![event(2, "storage:event:2", "2")])),
            Err(HedgingBillingServiceError::ResourceExhausted)
        ));
    }
    #[test]
    fn policy_rotation_is_fail_closed_without_a_governed_checkpoint_transition() {
        let root = tempfile::tempdir().expect("state root");
        let (service, feed_policy, _reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        service
            .ingest_finalized_page(&page(vec![event(1, "storage:event:1", "10")]))
            .expect("committed source page");
        let checkpoint = service
            .state
            .lock()
            .expect("service state")
            .checkpoint
            .clone();
        let mut rotated = service_policy();
        let rotated_key = SigningKey::from_bytes(&[0x24; 32]);
        rotated.statement_signer.signer_id = "billing-hsm-2".to_owned();
        rotated.statement_signer.public_key = rotated_key.verifying_key().to_bytes();
        assert!(matches!(
            checkpoint.validate(&rotated, &feed_policy),
            Err(HedgingBillingServiceError::InvalidCheckpoint)
        ));
        let mut rotated_feed_policy = feed_policy;
        rotated_feed_policy.policy_id = [0x32; 32];
        let mut feed_rotated_service_policy = service_policy();
        feed_rotated_service_policy.feed_trust_policy_digest = rotated_feed_policy
            .canonical_digest()
            .expect("rotated feed policy digest");
        assert!(matches!(
            checkpoint.validate(&feed_rotated_service_policy, &rotated_feed_policy),
            Err(HedgingBillingServiceError::InvalidCheckpoint)
        ));
    }
    #[test]
    fn non_genesis_policy_cannot_bootstrap_without_a_sealed_predecessor() {
        let root = tempfile::tempdir().expect("state root");
        let (next_policy, next_feed_policy) = rotated_policies();
        let verifier = Arc::new(TestJournalVerifier::default());
        assert!(matches!(
            HedgingBillingService::new(
                root.path(),
                next_policy,
                next_feed_policy,
                verifier.clone(),
                Arc::new(TestPublisher::new(false)),
                Arc::new(TestAcknowledgementAuthority::default()),
                verifier,
            ),
            Err(HedgingBillingServiceError::InvalidCheckpoint)
        ));
    }
    #[test]
    fn authenticated_epoch_transition_compacts_rotates_and_reopens_after_local_rename() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, verifier, publisher, acknowledgement_authority) =
            ready_service(root.path());
        settle_first_period(&service, &reference);
        let (next_policy, next_feed_policy) = rotated_policies();
        let outcome = service
            .transition_epoch(
                next_policy.clone(),
                next_feed_policy.clone(),
                vec![0xD8],
                &TestSigner::transition(),
            )
            .expect("authenticated epoch transition");
        assert_eq!(outcome.epoch_sequence, 1);
        assert_eq!(outcome.transition.compacted_counts.statements, 1);
        assert_eq!(outcome.transition.compacted_counts.acknowledgements, 1);
        assert_eq!(
            verifier
                .load_epoch(1)
                .expect("witness lookup")
                .expect("epoch witness")
                .transition_id,
            outcome.transition.transition_id
        );
        let restored = HedgingBillingService::new(
            root.path(),
            next_policy,
            next_feed_policy,
            verifier.clone(),
            publisher,
            acknowledgement_authority,
            verifier,
        )
        .expect("open rotated epoch");
        assert!(
            restored
                .statement_delivery_projections()
                .expect("active delivery state")
                .is_empty()
        );
        assert!(restored.hedge_intents().expect("active intents").is_empty());
        let checkpoint = restored
            .state
            .lock()
            .expect("service state")
            .checkpoint
            .clone();
        assert_eq!(checkpoint.epoch_sequence, 1);
        assert_eq!(checkpoint.compacted_account_bases.len(), 1);
        assert!(checkpoint.source_pages.is_empty());
        assert!(checkpoint.period_closes.is_empty());
        let mut next_event = event(2, "storage:event:2", "2");
        next_event.occurred_at_unix = PERIOD_END + 1;
        next_event.block_height = 12;
        next_event.block_hash = [12; 32];
        restored
            .ingest_finalized_page(&page(vec![next_event]))
            .expect("new epoch reuses released source-page capacity");
    }
    #[test]
    fn epoch_transition_retains_latest_chronological_account_base() {
        let root = tempfile::tempdir().expect("state root");
        let (service, feed_policy, first_reference, verifier, publisher, acknowledgement_authority) =
            ready_service(root.path());
        settle_first_period(&service, &first_reference);
        let second_period_end = PERIOD_END + PERIOD_SECS;
        let mut second_event = event(2, "storage:event:2", "2");
        second_event.occurred_at_unix = PERIOD_END + 100;
        second_event.block_height = 12;
        second_event.block_hash = [12; 32];
        let mut second_page = page(vec![second_event]);
        second_page
            .journal_commitment
            .finalized_cursor
            .finalized_at_unix = second_period_end;
        let second_reference = governed_reference_at(&feed_policy, second_period_end);
        let second_close = period_close_at(
            &service_policy(),
            &second_reference,
            second_page.journal_commitment,
            second_period_end,
        );
        let second_outcome = settle_period(&service, second_page, second_close);
        let (next_policy, next_feed_policy) = rotated_policies();
        service
            .transition_epoch(
                next_policy.clone(),
                next_feed_policy.clone(),
                vec![0xD8],
                &TestSigner::transition(),
            )
            .expect("compact two settled periods");
        let restored = HedgingBillingService::new(
            root.path(),
            next_policy,
            next_feed_policy,
            verifier.clone(),
            publisher,
            acknowledgement_authority,
            verifier,
        )
        .expect("open compacted epoch");
        let checkpoint = restored
            .state
            .lock()
            .expect("service state")
            .checkpoint
            .clone();
        assert_eq!(checkpoint.compacted_account_bases.len(), 1);
        assert_eq!(
            checkpoint.compacted_account_bases[0]
                .statement
                .period_end_unix,
            second_period_end
        );
        assert_eq!(
            checkpoint.compacted_account_bases[0].statement.statement_id,
            second_outcome.statement_ids[0]
        );
    }
    #[test]
    fn epoch_transition_releases_capacity_at_the_configured_hard_limit() {
        let root = tempfile::tempdir().expect("state root");
        let feed_policy = feed_policy();
        let reference = governed_reference(&feed_policy);
        let mut policy = service_policy();
        policy.max_retained_source_pages = 1;
        let verifier = Arc::new(TestJournalVerifier::default());
        let publisher = Arc::new(TestPublisher::new(false));
        let acknowledgement_authority = Arc::new(TestAcknowledgementAuthority::default());
        let service = HedgingBillingService::new(
            root.path(),
            policy.clone(),
            feed_policy,
            verifier.clone(),
            publisher.clone(),
            acknowledgement_authority.clone(),
            verifier.clone(),
        )
        .expect("initialize bounded service");
        let first = page(vec![event(1, "storage:event:1", "10")]);
        let first_close =
            period_close_at(&policy, &reference, first.journal_commitment, PERIOD_END);
        settle_period(&service, first, first_close);
        let (mut next_policy, next_feed_policy) = rotated_policies();
        next_policy.max_retained_source_pages = 1;
        next_policy.predecessor_policy_digest =
            Some(policy.canonical_digest().expect("bounded policy digest"));
        service
            .transition_epoch(
                next_policy.clone(),
                next_feed_policy.clone(),
                vec![0xD8],
                &TestSigner::transition(),
            )
            .expect("authenticated compaction at capacity");
        let restored = HedgingBillingService::new(
            root.path(),
            next_policy,
            next_feed_policy,
            verifier.clone(),
            publisher,
            acknowledgement_authority,
            verifier,
        )
        .expect("open bounded successor");
        let mut next_event = event(2, "storage:event:2", "2");
        next_event.occurred_at_unix = PERIOD_END + 1;
        next_event.block_height = 12;
        next_event.block_hash = [12; 32];
        restored
            .ingest_finalized_page(&page(vec![next_event]))
            .expect("released capacity accepts the next authenticated page");
    }
    #[test]
    fn epoch_transition_rejects_unsettled_state_before_witness_write() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:event:1", "10")]);
        service
            .ingest_finalized_page(&first)
            .expect("committed source");
        service
            .finalize_next_period(&period_close(&reference, first.journal_commitment))
            .expect("close");
        let (next_policy, next_feed_policy) = rotated_policies();
        assert!(matches!(
            service.transition_epoch(
                next_policy,
                next_feed_policy,
                vec![0xD8],
                &TestSigner::transition(),
            ),
            Err(HedgingBillingServiceError::UnsettledEpochTransition)
        ));
        assert!(verifier.load_latest().expect("witness lookup").is_none());
    }
    #[test]
    fn epoch_transition_rejects_oversized_proof_before_signing_or_witness_write() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        settle_first_period(&service, &reference);
        let (next_policy, next_feed_policy) = rotated_policies();
        let signer = TestSigner::transition();
        assert!(matches!(
            service.transition_epoch(
                next_policy,
                next_feed_policy,
                vec![0; HEDGING_BILLING_CONSENSUS_PROOF_MAX_BYTES_V1 + 1],
                &signer,
            ),
            Err(HedgingBillingServiceError::InvalidEpochTransition)
        ));
        assert_eq!(signer.sign_calls.load(Ordering::Relaxed), 0);
        assert_eq!(verifier.transition_calls.load(Ordering::Relaxed), 0);
        assert!(verifier.load_latest().expect("witness lookup").is_none());
    }
    #[test]
    fn sealed_epoch_witness_recovers_crash_before_local_checkpoint_rename() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, verifier, publisher, acknowledgement_authority) =
            ready_service(root.path());
        settle_first_period(&service, &reference);
        let old_checkpoint_bytes = service
            .store
            .load_bytes()
            .expect("load old checkpoint")
            .0
            .expect("old checkpoint bytes");
        let (next_policy, next_feed_policy) = rotated_policies();
        service
            .transition_epoch(
                next_policy.clone(),
                next_feed_policy.clone(),
                vec![0xD8],
                &TestSigner::transition(),
            )
            .expect("seal and install transition");
        let raw_store = AtomicCheckpointStore::new(
            root.path(),
            HEDGING_BILLING_CHECKPOINT_FILE_NAME_V1,
            HEDGING_BILLING_LOCK_FILE_NAME_V1,
            next_policy.checkpoint_max_bytes,
        )
        .expect("open raw checkpoint store");
        let current_fingerprint = raw_store
            .load_bytes()
            .expect("load transitioned checkpoint")
            .1;
        raw_store
            .commit_bytes(&old_checkpoint_bytes, current_fingerprint)
            .expect("simulate crash before local rename");
        drop(raw_store);
        let restored = HedgingBillingService::new(
            root.path(),
            next_policy,
            next_feed_policy,
            verifier.clone(),
            publisher,
            acknowledgement_authority,
            verifier,
        )
        .expect("recover exact sealed transition checkpoint");
        assert_eq!(
            restored
                .state
                .lock()
                .expect("service state")
                .checkpoint
                .epoch_sequence,
            1
        );
    }
    #[test]
    fn epoch_witness_tamper_and_rollback_fail_closed() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, verifier, publisher, acknowledgement_authority) =
            ready_service(root.path());
        settle_first_period(&service, &reference);
        let (next_policy, next_feed_policy) = rotated_policies();
        service
            .transition_epoch(
                next_policy.clone(),
                next_feed_policy.clone(),
                vec![0xD8],
                &TestSigner::transition(),
            )
            .expect("transition");
        {
            let mut records = verifier
                .witness_records
                .lock()
                .expect("epoch witness state");
            records.get_mut(&1).expect("epoch witness").checkpoint_bytes[0] ^= 0x80;
        }
        assert!(matches!(
            HedgingBillingService::new(
                root.path(),
                next_policy.clone(),
                next_feed_policy.clone(),
                verifier.clone(),
                publisher.clone(),
                acknowledgement_authority.clone(),
                verifier.clone(),
            ),
            Err(HedgingBillingServiceError::InvalidEpochWitness)
        ));
        verifier
            .witness_records
            .lock()
            .expect("epoch witness state")
            .clear();
        assert!(matches!(
            HedgingBillingService::new(
                root.path(),
                next_policy,
                next_feed_policy,
                verifier.clone(),
                publisher,
                acknowledgement_authority,
                verifier,
            ),
            Err(HedgingBillingServiceError::EpochWitnessRollback)
        ));
    }
    #[test]
    fn epoch_witness_has_one_bounded_canonical_persistence_format() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, verifier, _publisher, _acknowledgement_authority) =
            ready_service(root.path());
        settle_first_period(&service, &reference);
        let (next_policy, next_feed_policy) = rotated_policies();
        service
            .transition_epoch(
                next_policy.clone(),
                next_feed_policy,
                vec![0xD8],
                &TestSigner::transition(),
            )
            .expect("transition");
        let record = verifier
            .witness_records
            .lock()
            .expect("epoch witness state")
            .get(&1)
            .expect("epoch witness")
            .clone();
        let bytes = record
            .to_canonical_bytes(next_policy.checkpoint_max_bytes)
            .expect("canonical witness bytes");
        assert_eq!(
            HedgingBillingEpochWitnessRecordV1::from_canonical_bytes(
                &bytes,
                next_policy.checkpoint_max_bytes,
            )
            .expect("decode canonical witness"),
            record
        );
        let mut substituted = record;
        substituted.revision[0] ^= 0x80;
        let substituted_bytes = norito::to_bytes(&substituted).expect("substituted bytes");
        assert!(matches!(
            HedgingBillingEpochWitnessRecordV1::from_canonical_bytes(
                &substituted_bytes,
                next_policy.checkpoint_max_bytes,
            ),
            Err(HedgingBillingServiceError::InvalidEpochWitness)
        ));
    }
    #[test]
    fn epoch_witness_rejects_valid_same_epoch_non_base_checkpoint_substitution() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, verifier, publisher, acknowledgement_authority) =
            ready_service(root.path());
        settle_first_period(&service, &reference);
        let (next_policy, next_feed_policy) = rotated_policies();
        let outcome = service
            .transition_epoch(
                next_policy.clone(),
                next_feed_policy.clone(),
                vec![0xD8],
                &TestSigner::transition(),
            )
            .expect("transition");
        let restored = HedgingBillingService::new(
            root.path(),
            next_policy.clone(),
            next_feed_policy.clone(),
            verifier.clone(),
            publisher.clone(),
            acknowledgement_authority.clone(),
            verifier.clone(),
        )
        .expect("open successor");
        let mut next_event = event(2, "storage:event:2", "2");
        next_event.occurred_at_unix = PERIOD_END + 1;
        next_event.block_height = 12;
        next_event.block_hash = [12; 32];
        restored
            .ingest_finalized_page(&page(vec![next_event]))
            .expect("authenticated same-epoch progress");
        let progressed_bytes = {
            let guard = restored.state.lock().expect("service state");
            encode_checkpoint(&guard.checkpoint, &next_policy, &next_feed_policy)
                .expect("progressed checkpoint bytes")
        };
        verifier
            .witness_records
            .lock()
            .expect("epoch witness state")
            .insert(
                1,
                HedgingBillingEpochWitnessRecordV1::new(
                    test_network_id(b"hedging-billing-test-genesis"),
                    1,
                    outcome.transition.transition_id,
                    progressed_bytes,
                ),
            );
        drop(restored);
        assert!(matches!(
            HedgingBillingService::new(
                root.path(),
                next_policy,
                next_feed_policy,
                verifier.clone(),
                publisher,
                acknowledgement_authority,
                verifier,
            ),
            Err(HedgingBillingServiceError::InvalidEpochWitness)
        ));
    }
    #[test]
    fn skipped_epoch_and_immutable_archive_fork_fail_closed() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, verifier, publisher, acknowledgement_authority) =
            ready_service(root.path());
        settle_first_period(&service, &reference);
        let (next_policy, next_feed_policy) = rotated_policies();
        service
            .transition_epoch(
                next_policy.clone(),
                next_feed_policy.clone(),
                vec![0xD8],
                &TestSigner::transition(),
            )
            .expect("transition");
        {
            let mut records = verifier
                .witness_records
                .lock()
                .expect("epoch witness state");
            let mut skipped = records.get(&1).expect("epoch one witness").clone();
            skipped.epoch_sequence = 2;
            skipped.revision = epoch_witness_record_revision(&skipped);
            records.insert(2, skipped);
        }
        assert!(matches!(
            HedgingBillingService::new(
                root.path(),
                next_policy.clone(),
                next_feed_policy.clone(),
                verifier.clone(),
                publisher.clone(),
                acknowledgement_authority.clone(),
                verifier.clone(),
            ),
            Err(HedgingBillingServiceError::InvalidEpochWitness)
        ));
        verifier
            .witness_records
            .lock()
            .expect("epoch witness state")
            .remove(&2);
        verifier
            .witness_fork_on_epoch_lookup
            .store(true, Ordering::Relaxed);
        assert!(matches!(
            HedgingBillingService::new(
                root.path(),
                next_policy,
                next_feed_policy,
                verifier.clone(),
                publisher,
                acknowledgement_authority,
                verifier,
            ),
            Err(HedgingBillingServiceError::EpochWitnessFork)
        ));
    }
    #[test]
    fn rejected_journal_proof_does_not_advance_durable_state() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, _reference, verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        verifier.reject_pages.store(true, Ordering::Relaxed);
        assert!(matches!(
            service.ingest_finalized_page(&page(vec![event(1, "storage:event:1", "10")])),
            Err(HedgingBillingServiceError::External(
                HedgingBillingExternalError::Rejected
            ))
        ));
        assert_eq!(
            service.query_position().expect("durable position"),
            HedgingBillingQueryPositionV1 {
                next_sequence: 1,
                journal_commitment: None,
            }
        );
    }
    #[test]
    fn cross_page_position_rollback_is_rejected_atomically() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, _reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:event:1", "10")]);
        service
            .ingest_finalized_page(&first)
            .expect("first committed page");
        let mut reordered_event = event(2, "storage:event:2", "2");
        reordered_event.block_height = 9;
        reordered_event.block_hash = [9; 32];
        let reordered = page(vec![reordered_event]);
        assert!(matches!(
            service.ingest_finalized_page(&reordered),
            Err(HedgingBillingServiceError::InvalidCheckpoint)
        ));
        assert_eq!(
            service.query_position().expect("unchanged position"),
            HedgingBillingQueryPositionV1 {
                next_sequence: 2,
                journal_commitment: Some(first.journal_commitment),
            }
        );
    }
    #[test]
    fn checkpoint_cannot_inject_a_ready_for_signing_statement() {
        let root = tempfile::tempdir().expect("state root");
        let (service, feed_policy, reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:event:1", "10")]);
        service
            .ingest_finalized_page(&first)
            .expect("committed source page");
        service
            .finalize_next_period(&period_close(&reference, first.journal_commitment))
            .expect("committed period close");
        let injected = service
            .state
            .lock()
            .expect("service state")
            .checkpoint
            .statements[0]
            .clone();
        assert_eq!(
            injected.state,
            StoredStatementDeliveryStateV1::ReadyForSigning
        );
        let mut poisoned =
            HedgingBillingCheckpointV1::empty(&service_policy()).expect("empty checkpoint");
        poisoned.statements.push(injected);
        assert!(matches!(
            poisoned.validate(&service_policy(), &feed_policy),
            Err(HedgingBillingServiceError::InvalidCheckpoint)
        ));
    }
    include!("hedging_billing_service/replay_digest_tests.rs");
    #[test]
    fn invalid_hsm_output_is_not_persisted_or_published() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first_page = page(vec![event(1, "storage:event:1", "10")]);
        service.ingest_finalized_page(&first_page).expect("ingest");
        service
            .finalize_next_period(&period_close(&reference, first_page.journal_commitment))
            .expect("finalize");
        let signer = TestSigner::valid();
        signer.corrupt.store(true, Ordering::Relaxed);
        assert!(matches!(
            service.sign_next_statement(&signer),
            Err(HedgingBillingServiceError::InvalidSignedStatement)
        ));
        assert_eq!(
            service
                .statement_delivery_projections()
                .expect("projection")[0]
                .status,
            BillingStatementDeliveryStatusV1::ReadyForSigning
        );
        assert!(
            service
                .publish_next_statement()
                .expect("no publishable work")
                .is_none()
        );
    }
    #[test]
    fn ambiguous_publication_requires_authoritative_lookup() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, publisher, _ack_authority) =
            ready_service(root.path());
        let first_page = page(vec![event(1, "storage:event:1", "10")]);
        service.ingest_finalized_page(&first_page).expect("ingest");
        let outcome = service
            .finalize_next_period(&period_close(&reference, first_page.journal_commitment))
            .expect("finalize");
        service
            .sign_next_statement(&TestSigner::valid())
            .expect("sign");
        publisher.ambiguous_once.store(true, Ordering::Relaxed);
        assert!(matches!(
            service.publish_next_statement(),
            Err(HedgingBillingServiceError::External(
                HedgingBillingExternalError::Ambiguous
            ))
        ));
        assert_eq!(
            service
                .statement_delivery_projections()
                .expect("projection")[0]
                .status,
            BillingStatementDeliveryStatusV1::PublicationAmbiguous
        );
        let receipt = service
            .reconcile_ambiguous_publication(outcome.statement_ids[0])
            .expect("reconcile")
            .expect("sink receipt");
        assert_eq!(receipt.statement_id, outcome.statement_ids[0]);
    }
    #[test]
    fn publisher_lookup_prevents_duplicate_write_after_local_rollback() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, publisher, _ack_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:event:1", "10")]);
        service
            .ingest_finalized_page(&first)
            .expect("committed source page");
        let outcome = service
            .finalize_next_period(&period_close(&reference, first.journal_commitment))
            .expect("committed close");
        service
            .sign_next_statement(&TestSigner::valid())
            .expect("sign")
            .expect("signed statement");
        let receipt = service
            .publish_next_statement()
            .expect("publish")
            .expect("publication receipt");
        assert_eq!(publisher.publish_calls.load(Ordering::Relaxed), 1);
        {
            let mut guard = service.state.lock().expect("service state");
            let record = find_statement_mut(&mut guard.checkpoint, outcome.statement_ids[0])
                .expect("retained statement");
            record.state = StoredStatementDeliveryStateV1::ReadyForPublication;
            record.publication_receipt = None;
        }
        assert_eq!(
            service
                .publish_next_statement()
                .expect("authoritative preflight lookup")
                .expect("reconciled receipt"),
            receipt
        );
        assert_eq!(
            publisher.publish_calls.load(Ordering::Relaxed),
            1,
            "authoritative lookup must prevent a second external write"
        );
    }
    #[test]
    fn startup_reconciliation_repairs_publisher_and_acknowledgement_rollback() {
        let root = tempfile::tempdir().expect("state root");
        let (service, feed_policy, reference, verifier, publisher, acknowledgement_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:event:1", "10")]);
        service
            .ingest_finalized_page(&first)
            .expect("committed source page");
        let outcome = service
            .finalize_next_period(&period_close(&reference, first.journal_commitment))
            .expect("committed close");
        let signed = service
            .sign_next_statement(&TestSigner::valid())
            .expect("sign")
            .expect("signed statement");
        let receipt = service
            .publish_next_statement()
            .expect("publish")
            .expect("publication receipt");
        service
            .acknowledge_statement(
                receipt.statement_id,
                &primary_account_bytes(),
                [0x73; 32],
                receipt.published_at_unix + 1,
                vec![0xAC],
            )
            .expect("authenticated acknowledgement");
        let (rolled_back_bytes, fingerprint) = {
            let guard = service.state.lock().expect("service state");
            let mut rolled_back = guard.checkpoint.clone();
            let record = find_statement_mut(&mut rolled_back, outcome.statement_ids[0])
                .expect("retained statement");
            record.state = StoredStatementDeliveryStateV1::ReadyForSigning;
            record.signing_attempts = 0;
            record.signing_claim_cursor = None;
            record.signed_statement = None;
            record.publication_receipt = None;
            rolled_back.acknowledgements.clear();
            (
                encode_checkpoint(&rolled_back, &service_policy(), &feed_policy)
                    .expect("structurally valid rolled-back checkpoint"),
                guard.fingerprint,
            )
        };
        service
            .store
            .commit_bytes(&rolled_back_bytes, fingerprint)
            .expect("replace checkpoint with stale but structurally valid delivery state");
        drop(service);
        let restored = HedgingBillingService::new(
            root.path(),
            service_policy(),
            feed_policy,
            verifier.clone(),
            publisher,
            acknowledgement_authority,
            verifier,
        )
        .expect("authoritative startup reconciliation");
        assert_eq!(
            restored
                .statement_delivery_projections()
                .expect("delivery projection")[0]
                .status,
            BillingStatementDeliveryStatusV1::Acknowledged
        );
        assert_eq!(
            restored
                .published_statement(outcome.statement_ids[0])
                .expect("recovered published statement"),
            signed
        );
    }
    #[derive(Debug)]
    struct TestExecutionAdapter {
        key: SigningKey,
        receipts: Mutex<BTreeMap<[u8; 32], HedgeExecutionSubmissionReceiptV1>>,
        submit_calls: AtomicUsize,
    }
    impl TestExecutionAdapter {
        fn new() -> Self {
            Self {
                key: venue_key(),
                receipts: Mutex::new(BTreeMap::new()),
                submit_calls: AtomicUsize::new(0),
            }
        }
        fn receipt(
            &self,
            intent: &HedgeIntentV1,
            authorization: &HedgeExecutionAuthorizationV1,
        ) -> HedgeExecutionSubmissionReceiptV1 {
            let mut receipt = HedgeExecutionSubmissionReceiptV1 {
                version: HEDGE_EXECUTION_RECEIPT_VERSION_V1,
                authorization_id: authorization.authorization_id,
                intent_id: intent.intent_id,
                venue_id: "venue-1".to_owned(),
                venue_order_id: "venue-order-1".to_owned(),
                submitted_at_unix: authorization.authorized_at_unix + 1,
                receipt_digest: [0; 32],
                signature: [0; 64],
            };
            receipt.receipt_digest =
                execution_receipt_digest(&receipt).expect("execution receipt digest");
            let mut message = Vec::new();
            message.extend_from_slice(HEDGE_EXECUTION_RECEIPT_SIGNATURE_DOMAIN_V1);
            message.extend_from_slice(&receipt.receipt_digest);
            receipt.signature = self.key.sign(&message).to_bytes();
            receipt
        }
    }
    impl GovernedHedgeExecutionAdapter for TestExecutionAdapter {
        fn identity(
            &self,
        ) -> Result<GovernedHedgeExecutionVenueIdentityV1, HedgingBillingExternalError> {
            Ok(GovernedHedgeExecutionVenueIdentityV1 {
                venue_id: "venue-1".to_owned(),
                public_key: self.key.verifying_key().to_bytes(),
            })
        }
        fn automatic_execution_enabled(&self) -> bool {
            false
        }
        fn submit_authorized(
            &self,
            idempotency_key: [u8; 32],
            intent: &HedgeIntentV1,
            authorization: &HedgeExecutionAuthorizationV1,
        ) -> Result<HedgeExecutionSubmissionReceiptV1, HedgingBillingExternalError> {
            self.submit_calls.fetch_add(1, Ordering::Relaxed);
            if idempotency_key != authorization.authorization_id
                || authorization.intent_id != intent.intent_id
            {
                return Err(HedgingBillingExternalError::Rejected);
            }
            let receipt = self.receipt(intent, authorization);
            let mut receipts = self.receipts.lock().expect("execution adapter state");
            if let Some(existing) = receipts.get(&idempotency_key) {
                if existing != &receipt {
                    return Err(HedgingBillingExternalError::Rejected);
                }
                return Ok(existing.clone());
            }
            receipts.insert(idempotency_key, receipt.clone());
            Ok(receipt)
        }
        fn lookup_authorization(
            &self,
            authorization_id: [u8; 32],
        ) -> Result<Option<HedgeExecutionSubmissionReceiptV1>, HedgingBillingExternalError>
        {
            Ok(self
                .receipts
                .lock()
                .expect("execution adapter state")
                .get(&authorization_id)
                .cloned())
        }
    }
    #[derive(Debug)]
    struct AutoExecutionAdapter;
    impl GovernedHedgeExecutionAdapter for AutoExecutionAdapter {
        fn identity(
            &self,
        ) -> Result<GovernedHedgeExecutionVenueIdentityV1, HedgingBillingExternalError> {
            Ok(GovernedHedgeExecutionVenueIdentityV1 {
                venue_id: "venue-1".to_owned(),
                public_key: venue_key().verifying_key().to_bytes(),
            })
        }
        fn automatic_execution_enabled(&self) -> bool {
            true
        }
        fn submit_authorized(
            &self,
            _idempotency_key: [u8; 32],
            _intent: &HedgeIntentV1,
            _authorization: &HedgeExecutionAuthorizationV1,
        ) -> Result<HedgeExecutionSubmissionReceiptV1, HedgingBillingExternalError> {
            Err(HedgingBillingExternalError::Rejected)
        }
        fn lookup_authorization(
            &self,
            _authorization_id: [u8; 32],
        ) -> Result<Option<HedgeExecutionSubmissionReceiptV1>, HedgingBillingExternalError>
        {
            Ok(None)
        }
    }
    #[test]
    fn operator_authorized_execution_is_policy_bound_and_idempotent() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, _publisher, _ack_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:event:1", "10")]);
        service
            .ingest_finalized_page(&first)
            .expect("committed source page");
        let intent = service
            .finalize_next_period(&period_close(&reference, first.journal_commitment))
            .expect("close")
            .hedge_intent
            .expect("executable intent");
        let authorization = execution_authorization(&intent);
        let adapter = TestExecutionAdapter::new();
        let receipt = service
            .submit_authorized_hedge_intent(&execution_policy(), &authorization, &adapter)
            .expect("explicit authorized submission");
        assert_eq!(receipt.intent_id, intent.intent_id);
        assert_eq!(adapter.submit_calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            service
                .submit_authorized_hedge_intent(&execution_policy(), &authorization, &adapter)
                .expect("idempotent authoritative lookup"),
            receipt
        );
        assert_eq!(adapter.submit_calls.load(Ordering::Relaxed), 1);
    }
    include!("hedging_billing_service/finalized_account_validation_tests.rs");
    #[test]
    fn owner_api_is_anchor_bound_terminal_only_and_oracle_safe() {
        let root = tempfile::tempdir().expect("state root");
        let (service, _feed_policy, reference, _verifier, _publisher, ack_authority) =
            ready_service(root.path());
        let first = page(vec![event(1, "storage:event:owner-api", "10")]);
        service
            .ingest_finalized_page(&first)
            .expect("committed source page");
        let period = service
            .finalize_next_period(&period_close(&reference, first.journal_commitment))
            .expect("committed period");
        let statement_id = period.statement_ids[0];
        let owner = primary_account_bytes();
        let other_owner = account_bytes(0x92);
        let intermediate_anchor = service.api_projection_anchor().expect("projection anchor");
        assert_eq!(
            intermediate_anchor.retention_scope,
            HedgingBillingRetentionScopeV1::ActiveEpochOnly
        );
        let intermediate_page = service
            .api_list_statements(&BillingStatementListRequestV1 {
                owner_account_id: owner.clone(),
                after_statement_id: None,
                limit: 100,
                expected_checkpoint_fingerprint: intermediate_anchor.checkpoint_fingerprint,
            })
            .expect("owner terminal projection");
        assert!(
            intermediate_page.items.is_empty(),
            "signing/publication intermediate state must not be exposed"
        );
        let intermediate_get = service
            .api_published_statement(&BillingPublishedStatementRequestV1 {
                owner_account_id: owner.clone(),
                statement_id,
                expected_checkpoint_fingerprint: intermediate_anchor.checkpoint_fingerprint,
            })
            .expect_err("unpublished statement remains unavailable");
        assert_eq!(
            intermediate_get,
            HedgingBillingRuntimeApiErrorV1::StatementUnavailableToOwner
        );
        service
            .sign_next_statement(&TestSigner::valid())
            .expect("sign")
            .expect("signed statement");
        let receipt = service
            .publish_next_statement()
            .expect("publish")
            .expect("publication receipt");
        let published_anchor = service.api_projection_anchor().expect("published anchor");
        let intent_page = service
            .api_hedge_intent_page(&HedgingBillingProjectionPageRequestV1 {
                expected_checkpoint_fingerprint: published_anchor.checkpoint_fingerprint,
                after: None,
                limit: 100,
            })
            .expect("generated hedge-intent page");
        assert_eq!(intent_page.items.len(), 1);
        assert!(!intent_page.items[0].automatic_execution);
        assert!(!intent_page.automatic_execution_enabled);
        let after_intent = service
            .api_hedge_intent_page(&HedgingBillingProjectionPageRequestV1 {
                expected_checkpoint_fingerprint: published_anchor.checkpoint_fingerprint,
                after: Some(intent_page.items[0].intent_id),
                limit: 100,
            })
            .expect("exclusive hedge-intent cursor");
        assert!(after_intent.items.is_empty());
        let list_request = BillingStatementListRequestV1 {
            owner_account_id: owner.clone(),
            after_statement_id: None,
            limit: 1,
            expected_checkpoint_fingerprint: published_anchor.checkpoint_fingerprint,
        };
        let page = service
            .api_list_statements(&list_request)
            .expect("published owner page");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].statement_id, statement_id);
        assert_eq!(
            page.items[0].status,
            BillingStatementOwnerStatusV1::Published
        );
        assert_eq!(
            page.items[0].signed_statement_digest,
            receipt.signed_statement_digest
        );
        assert!(page.next_cursor.is_none());
        let after_page = service
            .api_list_statements(&BillingStatementListRequestV1 {
                after_statement_id: Some(statement_id),
                ..list_request.clone()
            })
            .expect("exclusive statement cursor");
        assert!(after_page.items.is_empty());
        assert_eq!(
            service
                .api_list_statements(&BillingStatementListRequestV1 {
                    limit: 0,
                    ..list_request.clone()
                })
                .expect_err("zero page bound"),
            HedgingBillingRuntimeApiErrorV1::InvalidRequest
        );
        assert_eq!(
            service
                .api_list_statements(&BillingStatementListRequestV1 {
                    owner_account_id: b"account-1".to_vec(),
                    ..list_request.clone()
                })
                .expect_err("non-I105 owner bytes"),
            HedgingBillingRuntimeApiErrorV1::InvalidRequest
        );
        assert_eq!(
            service
                .api_list_statements(&BillingStatementListRequestV1 {
                    limit: HEDGING_BILLING_RUNTIME_API_MAX_PAGE_ITEMS_V1 + 1,
                    ..list_request.clone()
                })
                .expect_err("oversized page bound"),
            HedgingBillingRuntimeApiErrorV1::InvalidRequest
        );
        assert_eq!(
            service
                .api_list_statements(&BillingStatementListRequestV1 {
                    expected_checkpoint_fingerprint: intermediate_anchor.checkpoint_fingerprint,
                    ..list_request.clone()
                })
                .expect_err("stale checkpoint anchor"),
            HedgingBillingRuntimeApiErrorV1::ProjectionChanged
        );
        let wrong_owner_error = service
            .api_published_statement(&BillingPublishedStatementRequestV1 {
                owner_account_id: other_owner,
                statement_id,
                expected_checkpoint_fingerprint: published_anchor.checkpoint_fingerprint,
            })
            .expect_err("wrong owner");
        let unknown_error = service
            .api_published_statement(&BillingPublishedStatementRequestV1 {
                owner_account_id: owner.clone(),
                statement_id: [0xEE; 32],
                expected_checkpoint_fingerprint: published_anchor.checkpoint_fingerprint,
            })
            .expect_err("unknown statement");
        assert_eq!(wrong_owner_error, unknown_error);
        assert_eq!(
            unknown_error,
            HedgingBillingRuntimeApiErrorV1::StatementUnavailableToOwner
        );
        let mut acknowledgement_request = BillingStatementAcknowledgementRequestV1 {
            expected_checkpoint_fingerprint: published_anchor.checkpoint_fingerprint,
            statement_id,
            owner_account_id: owner,
            request_nonce: [0x91; 32],
            authentication_proof: vec![0xAC],
        };
        let mut oversized_proof = acknowledgement_request.clone();
        oversized_proof.authentication_proof =
            vec![0xAC; BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1 + 1];
        assert_eq!(
            service
                .api_acknowledge_statement(
                    &oversized_proof,
                    receipt.published_at_unix.saturating_add(1),
                )
                .expect_err("oversized proof"),
            HedgingBillingRuntimeApiErrorV1::InvalidRequest
        );
        let response = service
            .api_acknowledge_statement(&acknowledgement_request, receipt.published_at_unix + 1)
            .expect("durable owner acknowledgement");
        assert_eq!(response.acknowledgement.statement_id, statement_id);
        assert_eq!(
            response.acknowledgement.request_binding_digest,
            billing_statement_acknowledgement_request_digest_v1(
                acknowledgement_request.statement_id,
                &acknowledgement_request.owner_account_id,
                acknowledgement_request.request_nonce,
            )
            .expect("domain-separated request binding")
        );
        assert!(
            !format!("{response:?}").contains("authentication_proof"),
            "acknowledgement responses must never return raw authentication proof"
        );
        let durable_acknowledgement = ack_authority
            .lookup(statement_id)
            .expect("authoritative acknowledgement lookup")
            .expect("durable acknowledgement");
        let durable_debug = format!("{durable_acknowledgement:?}");
        assert!(durable_debug.contains("[REDACTED]"));
        assert!(
            !durable_debug.contains("[172]"),
            "durable acknowledgement Debug must not expose proof bytes"
        );
        assert_eq!(
            service
                .api_acknowledge_statement(&acknowledgement_request, receipt.published_at_unix + 2,)
                .expect_err("stale acknowledgement replay anchor"),
            HedgingBillingRuntimeApiErrorV1::ProjectionChanged
        );
        acknowledgement_request.expected_checkpoint_fingerprint =
            response.anchor.checkpoint_fingerprint;
        assert_eq!(
            service
                .api_acknowledge_statement(&acknowledgement_request, receipt.published_at_unix + 2,)
                .expect("idempotent acknowledgement replay")
                .acknowledgement,
            response.acknowledgement
        );
        let current_anchor = service.api_projection_anchor().expect("current anchor");
        let mut conflict = acknowledgement_request.clone();
        conflict.expected_checkpoint_fingerprint = current_anchor.checkpoint_fingerprint;
        conflict.request_nonce = [0x92; 32];
        assert_eq!(
            service
                .api_acknowledge_statement(&conflict, receipt.published_at_unix + 3)
                .expect_err("conflicting acknowledgement replay"),
            HedgingBillingRuntimeApiErrorV1::AcknowledgementConflict
        );
        let mut proof_conflict = acknowledgement_request;
        proof_conflict.expected_checkpoint_fingerprint = current_anchor.checkpoint_fingerprint;
        proof_conflict.authentication_proof = vec![0xAD];
        assert_eq!(
            billing_statement_acknowledgement_request_digest_v1(
                proof_conflict.statement_id,
                &proof_conflict.owner_account_id,
                proof_conflict.request_nonce,
            )
            .expect("proof-independent authentication challenge"),
            response.acknowledgement.request_binding_digest
        );
        assert_eq!(
            service
                .api_acknowledge_statement(&proof_conflict, receipt.published_at_unix + 3)
                .expect_err("invalid retry authentication proof"),
            HedgingBillingRuntimeApiErrorV1::InvalidRequest
        );
    }
    include!("hedging_billing_service/freshness_api_tests.rs");
    #[test]
    fn automatic_hedge_execution_is_unconditionally_rejected() {
        assert!(matches!(
            HedgingBillingService::validate_execution_adapter(
                &execution_policy(),
                &AutoExecutionAdapter,
            ),
            Err(HedgingBillingServiceError::AutomaticHedgeExecutionForbidden)
        ));
    }
}
