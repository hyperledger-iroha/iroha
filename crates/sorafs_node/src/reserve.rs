//! Local reserve lifecycle runtime for SoraFS SFM-6.

use std::collections::BTreeMap;

use iroha_data_model::sorafs::reserve::{
    ReserveLedgerProjection, ReserveLifecycleProjection, ReserveLifecycleStage, ReserveQuote,
};
use sorafs_manifest::deal::XorAmount;
use thiserror::Error;

const RESERVE_LIFECYCLE_DAY_SECS: u64 = 86_400;

/// Reserve lifecycle service error.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReserveLifecycleRuntimeError {
    /// The deterministic lifecycle projection failed.
    #[error("reserve lifecycle projection failed: {0}")]
    ProjectionFailed(String),
    /// Runtime event sequencing overflowed.
    #[error("reserve lifecycle event sequence overflow")]
    EventSequenceOverflow,
    /// The local reserve lifecycle runtime lock was poisoned.
    #[error("reserve lifecycle state lock poisoned")]
    StateLockPoisoned,
}

/// Reserve movement service error.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReserveMovementRuntimeError {
    /// Reserve movement amounts must be positive.
    #[error("reserve movement amount must be greater than zero")]
    ZeroAmount,
    /// The supplied movement id collides with a different movement.
    #[error("reserve movement id collision for {movement_id_hex}")]
    MovementIdCollision {
        /// Hex-encoded colliding movement identifier.
        movement_id_hex: String,
    },
    /// The provider does not have enough locally recorded reserve balance.
    #[error(
        "reserve balance underflow for provider {provider_id_hex}: requested {requested_micro_xor} micro-XOR, available {available_micro_xor} micro-XOR"
    )]
    InsufficientBalance {
        /// Hex-encoded provider identifier.
        provider_id_hex: String,
        /// Requested movement amount in micro-XOR.
        requested_micro_xor: u128,
        /// Available local reserve balance in micro-XOR.
        available_micro_xor: u128,
    },
    /// The referenced reserve movement was not found.
    #[error("reserve movement {movement_id_hex} was not found")]
    MovementNotFound {
        /// Hex-encoded movement identifier.
        movement_id_hex: String,
    },
    /// The custody status update is not allowed from the current state.
    #[error("invalid reserve custody transition for movement {movement_id_hex}: {from} -> {to}")]
    InvalidCustodyTransition {
        /// Hex-encoded movement identifier.
        movement_id_hex: String,
        /// Current custody status.
        from: &'static str,
        /// Requested custody status.
        to: &'static str,
    },
    /// The custody update tried to replace existing transaction evidence.
    #[error(
        "reserve custody transaction hash mismatch for movement {movement_id_hex}: existing {existing_tx_hash_hex}, requested {requested_tx_hash_hex}"
    )]
    CustodyTransactionHashMismatch {
        /// Hex-encoded movement identifier.
        movement_id_hex: String,
        /// Previously recorded custody transaction hash.
        existing_tx_hash_hex: String,
        /// Requested custody transaction hash.
        requested_tx_hash_hex: String,
    },
    /// Runtime event sequencing overflowed.
    #[error("reserve movement event sequence overflow")]
    EventSequenceOverflow,
    /// Arithmetic overflowed while applying the movement.
    #[error("reserve movement arithmetic failed: {0}")]
    ArithmeticFailed(String),
    /// The local reserve movement runtime lock was poisoned.
    #[error("reserve movement state lock poisoned")]
    StateLockPoisoned,
}

/// Reserve appeal and lifecycle policy service error.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReserveAppealRuntimeError {
    /// Appeal reasons, decision rationales, and policy reasons must be non-empty.
    #[error("reserve appeal field `{field}` must not be empty")]
    EmptyText {
        /// Empty field name.
        field: &'static str,
    },
    /// The supplied appeal id collides with a different appeal.
    #[error("reserve appeal id collision for {appeal_id_hex}")]
    AppealIdCollision {
        /// Hex-encoded colliding appeal identifier.
        appeal_id_hex: String,
    },
    /// The referenced reserve appeal was not found.
    #[error("reserve appeal {appeal_id_hex} was not found")]
    AppealNotFound {
        /// Hex-encoded appeal identifier.
        appeal_id_hex: String,
    },
    /// The appeal decision requested an unsupported status.
    #[error("reserve appeal decision status must be accepted or rejected")]
    InvalidDecisionStatus,
    /// The appeal decision is not allowed from the current state.
    #[error("invalid reserve appeal transition for appeal {appeal_id_hex}: {from} -> {to}")]
    InvalidAppealTransition {
        /// Hex-encoded appeal identifier.
        appeal_id_hex: String,
        /// Current appeal status.
        from: &'static str,
        /// Requested appeal status.
        to: &'static str,
    },
    /// The supplied policy id collides with a different policy update.
    #[error("reserve lifecycle policy id collision for {policy_id_hex}")]
    PolicyIdCollision {
        /// Hex-encoded colliding policy identifier.
        policy_id_hex: String,
    },
    /// Lifecycle policy windows must preserve the projection invariant.
    #[error(
        "reserve lifecycle policy grace period ({grace_period_days}) must be before default threshold ({default_after_days})"
    )]
    InvalidLifecyclePolicyWindow {
        /// Grace window before delinquency.
        grace_period_days: u16,
        /// Default threshold after the due date.
        default_after_days: u16,
    },
    /// The deterministic lifecycle projection failed while applying a policy.
    #[error("reserve lifecycle policy projection failed: {0}")]
    LifecyclePolicyProjectionFailed(String),
    /// Runtime event sequencing overflowed.
    #[error("reserve appeal event sequence overflow")]
    EventSequenceOverflow,
    /// The local reserve appeal runtime lock was poisoned.
    #[error("reserve appeal state lock poisoned")]
    StateLockPoisoned,
}

/// Input used to update one provider's local reserve lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveLifecycleUpdate {
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Canonical provider account bytes.
    pub provider_account: Vec<u8>,
    /// Deterministic reserve quote used for the projection.
    pub quote: ReserveQuote,
    /// Days since the current rent obligation became due.
    pub days_past_due: u16,
    /// Grace window before delinquency.
    pub grace_period_days: u16,
    /// Default threshold after the due date.
    pub default_after_days: u16,
    /// Unix timestamp when the service observed this state.
    pub observed_at_unix: u64,
}

/// Provider reserve lifecycle summary stored by the local runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveProviderLifecycleSummary {
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Canonical provider account bytes.
    pub provider_account: Vec<u8>,
    /// Latest deterministic reserve quote.
    pub quote: ReserveQuote,
    /// Ledger projection derived from the latest quote.
    pub ledger: ReserveLedgerProjection,
    /// Lifecycle projection derived from the latest quote and aging windows.
    pub lifecycle: ReserveLifecycleProjection,
    /// Grace window that was used for the lifecycle projection.
    pub grace_period_days: u16,
    /// Default threshold that was used for the lifecycle projection.
    pub default_after_days: u16,
    /// Applied lifecycle-policy record id, when the projection used a local policy update.
    pub applied_policy_id: Option<[u8; 32]>,
    /// Applied accepted appeal id, when the lifecycle stage was overridden by governance.
    pub applied_appeal_id: Option<[u8; 32]>,
    /// Unix timestamp when this summary was last updated.
    pub updated_at_unix: u64,
}

/// Provider credit-line state derived from the latest accepted lifecycle update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveProviderCreditLineState {
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Canonical provider account bytes.
    pub provider_account: Vec<u8>,
    /// Lifecycle event sequence that produced this credit-line state.
    pub lifecycle_event_sequence: u64,
    /// Lifecycle stage associated with this credit-line state.
    pub stage: ReserveLifecycleStage,
    /// Effective rent due for the current period.
    pub rent_due: XorAmount,
    /// Automatic credit drawn for the current period.
    pub credit_draw: XorAmount,
    /// Remaining automatic credit capacity after the draw, when the tier has an automatic line.
    pub credit_available_after_draw: Option<XorAmount>,
    /// Uncovered rent after applying automatic credit.
    pub credit_shortfall: XorAmount,
    /// Pro-rated interest accrued against the drawn credit.
    pub accrued_interest: XorAmount,
    /// Rent still payable after automatic credit plus accrued interest.
    pub total_due_after_credit: XorAmount,
    /// Whether manual credit approval is required for this tier/update.
    pub requires_manual_credit_approval: bool,
    /// Whether governance notification is required for this state.
    pub requires_governance_notification: bool,
    /// Applied accepted appeal id, when the lifecycle stage was overridden by governance.
    pub applied_appeal_id: Option<[u8; 32]>,
    /// Unix timestamp when this state was last updated.
    pub updated_at_unix: u64,
}

/// Point-in-time local reserve credit-line snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReserveCreditLineSnapshot {
    /// Unix timestamp used when the snapshot was produced.
    pub generated_at_unix: u64,
    /// Credit-line states sorted by provider id.
    pub credit_lines: Vec<ReserveProviderCreditLineState>,
}

/// Local reserve movement kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveMovementKind {
    /// Provider adds funds to the reserve account.
    TopUp,
    /// Provider withdraws funds from the reserve account.
    Withdrawal,
}

/// Local chain-custody status for a reserve movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveMovementCustodyStatus {
    /// The movement intent is recorded locally but no chain transaction evidence is attached.
    IntentRecorded,
    /// A chain transaction hash has been submitted for the movement.
    Submitted,
    /// The submitted chain transaction has been confirmed.
    Confirmed,
    /// The submitted chain transaction was rejected or permanently failed.
    Rejected,
}

impl ReserveMovementCustodyStatus {
    /// Return the stable JSON label for this custody status.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::IntentRecorded => "intent_recorded",
            Self::Submitted => "submitted",
            Self::Confirmed => "confirmed",
            Self::Rejected => "rejected",
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Confirmed | Self::Rejected)
    }
}

/// Input used to record one local reserve movement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveMovementRequest {
    /// Deterministic movement identifier derived from the signed request.
    pub movement_id: [u8; 32],
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Canonical provider account bytes.
    pub provider_account: Vec<u8>,
    /// Canonical reserve custody account bytes.
    pub reserve_account: Vec<u8>,
    /// Canonical asset definition identifier bytes.
    pub asset_definition_id: Vec<u8>,
    /// Movement kind.
    pub kind: ReserveMovementKind,
    /// Movement amount.
    pub amount: XorAmount,
    /// Idempotency key supplied by the caller.
    pub idempotency_key: String,
    /// Unix timestamp when the service accepted this movement.
    pub observed_at_unix: u64,
}

/// Input used to attach chain custody evidence to a recorded reserve movement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveMovementCustodyUpdate {
    /// Deterministic movement identifier being reconciled.
    pub movement_id: [u8; 32],
    /// Requested chain custody status.
    pub status: ReserveMovementCustodyStatus,
    /// Hex-encoded transaction hash supplied by the chain submitter/reconciler.
    pub tx_hash_hex: String,
    /// Unix timestamp when the custody status was observed.
    pub observed_at_unix: u64,
}

/// Provider reserve balance tracked by the local movement ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveProviderBalance {
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Canonical provider account bytes.
    pub provider_account: Vec<u8>,
    /// Canonical reserve custody account bytes.
    pub reserve_account: Vec<u8>,
    /// Canonical asset definition identifier bytes.
    pub asset_definition_id: Vec<u8>,
    /// Locally recorded reserve balance.
    pub balance: XorAmount,
    /// Chain-confirmed reserve balance from movements with confirmed custody.
    pub confirmed_balance: XorAmount,
    /// Unix timestamp when this balance last changed.
    pub updated_at_unix: u64,
}

/// Sequenced reserve movement record retained by the local ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveMovementRecord {
    /// Monotonic local movement sequence.
    pub sequence: u64,
    /// Deterministic movement identifier derived from the signed request.
    pub movement_id: [u8; 32],
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Canonical provider account bytes.
    pub provider_account: Vec<u8>,
    /// Canonical reserve custody account bytes.
    pub reserve_account: Vec<u8>,
    /// Canonical asset definition identifier bytes.
    pub asset_definition_id: Vec<u8>,
    /// Movement kind.
    pub kind: ReserveMovementKind,
    /// Movement amount.
    pub amount: XorAmount,
    /// Reserve balance after applying the movement.
    pub balance_after: XorAmount,
    /// Chain-confirmed reserve balance after applying confirmed custody movements.
    pub confirmed_balance_after: XorAmount,
    /// Idempotency key supplied by the caller.
    pub idempotency_key: String,
    /// Unix timestamp supplied with the accepted movement.
    pub observed_at_unix: u64,
    /// Latest local chain custody status for this movement.
    pub custody_status: ReserveMovementCustodyStatus,
    /// Hex-encoded chain transaction hash bound to the custody status, when known.
    pub custody_tx_hash_hex: Option<String>,
    /// Unix timestamp when the custody status was last updated.
    pub custody_updated_at_unix: Option<u64>,
}

/// Result returned after recording a reserve movement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveMovementOutcome {
    /// Accepted or previously recorded movement.
    pub record: ReserveMovementRecord,
    /// Whether this response came from a prior idempotent movement.
    pub duplicate: bool,
}

type RecomputedReserveMovementBalances = (
    BTreeMap<[u8; 32], ReserveProviderBalance>,
    Vec<ReserveMovementRecord>,
);

/// Local reserve appeal status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveAppealStatus {
    /// The provider appeal is open and awaiting a decision.
    Open,
    /// The appeal was accepted by the signed decision authority.
    Accepted,
    /// The appeal was rejected by the signed decision authority.
    Rejected,
}

impl ReserveAppealStatus {
    /// Return the stable JSON label for this appeal status.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Accepted | Self::Rejected)
    }
}

/// Input used to submit a provider reserve appeal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveAppealRequest {
    /// Deterministic appeal identifier derived from the signed request.
    pub appeal_id: [u8; 32],
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Canonical provider account bytes.
    pub provider_account: Vec<u8>,
    /// Requested lifecycle stage override, when the appeal asks for one.
    pub requested_stage: Option<ReserveLifecycleStage>,
    /// Provider-supplied appeal reason.
    pub reason: String,
    /// Optional payload-free evidence digest associated with the appeal packet.
    pub evidence_digest_hex: Option<String>,
    /// Idempotency key supplied by the caller.
    pub idempotency_key: String,
    /// Unix timestamp when the service accepted this appeal.
    pub observed_at_unix: u64,
}

/// Input used to decide a provider reserve appeal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveAppealDecision {
    /// Deterministic appeal identifier being decided.
    pub appeal_id: [u8; 32],
    /// Requested final appeal status.
    pub status: ReserveAppealStatus,
    /// Canonical account bytes for the signed decision authority.
    pub decision_account: Vec<u8>,
    /// Decision rationale recorded for audit handoff.
    pub rationale: String,
    /// Unix timestamp when the decision was observed.
    pub decided_at_unix: u64,
}

/// Sequenced local reserve appeal record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveAppealRecord {
    /// Monotonic local appeal sequence.
    pub sequence: u64,
    /// Deterministic appeal identifier derived from the signed request.
    pub appeal_id: [u8; 32],
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Canonical provider account bytes.
    pub provider_account: Vec<u8>,
    /// Requested lifecycle stage override, when present.
    pub requested_stage: Option<ReserveLifecycleStage>,
    /// Provider-supplied appeal reason.
    pub reason: String,
    /// Optional payload-free evidence digest associated with the appeal packet.
    pub evidence_digest_hex: Option<String>,
    /// Idempotency key supplied by the caller.
    pub idempotency_key: String,
    /// Current local appeal status.
    pub status: ReserveAppealStatus,
    /// Unix timestamp when the appeal was opened.
    pub opened_at_unix: u64,
    /// Canonical account bytes for the signed decision authority, when decided.
    pub decision_account: Option<Vec<u8>>,
    /// Decision rationale recorded for audit handoff, when decided.
    pub decision_rationale: Option<String>,
    /// Unix timestamp when the decision was observed, when decided.
    pub decided_at_unix: Option<u64>,
}

/// Result returned after recording a reserve appeal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveAppealOutcome {
    /// Accepted or previously recorded appeal.
    pub record: ReserveAppealRecord,
    /// Whether this response came from a prior idempotent appeal.
    pub duplicate: bool,
}

/// Result returned after recording a reserve appeal decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveAppealDecisionOutcome {
    /// Accepted or previously recorded appeal decision.
    pub record: ReserveAppealRecord,
    /// Lifecycle override event emitted when an accepted decision applies a requested stage.
    pub lifecycle_event: Option<ReserveLifecycleEvent>,
    /// Whether this response came from a prior idempotent decision.
    pub duplicate: bool,
}

/// Point-in-time reserve appeal snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReserveAppealSnapshot {
    /// Next sequence that will be assigned to a reserve appeal.
    pub next_sequence: u64,
    /// Unix timestamp used when the snapshot was produced.
    pub generated_at_unix: u64,
    /// Appeal records sorted by local sequence.
    pub appeals: Vec<ReserveAppealRecord>,
}

/// Input used to update local reserve lifecycle policy windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveLifecyclePolicyUpdate {
    /// Deterministic policy identifier derived from the signed request.
    pub policy_id: [u8; 32],
    /// Canonical account bytes for the signed policy authority.
    pub authority_account: Vec<u8>,
    /// Grace window before delinquency.
    pub grace_period_days: u16,
    /// Default threshold after the due date.
    pub default_after_days: u16,
    /// Unix timestamp when this policy becomes effective.
    pub effective_at_unix: u64,
    /// Human-readable reason for the policy update.
    pub reason: String,
    /// Idempotency key supplied by the caller.
    pub idempotency_key: String,
    /// Unix timestamp when the service accepted this policy update.
    pub observed_at_unix: u64,
}

/// Sequenced local reserve lifecycle policy record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveLifecyclePolicyRecord {
    /// Monotonic local policy sequence.
    pub sequence: u64,
    /// Deterministic policy identifier derived from the signed request.
    pub policy_id: [u8; 32],
    /// Canonical account bytes for the signed policy authority.
    pub authority_account: Vec<u8>,
    /// Grace window before delinquency.
    pub grace_period_days: u16,
    /// Default threshold after the due date.
    pub default_after_days: u16,
    /// Unix timestamp when this policy becomes effective.
    pub effective_at_unix: u64,
    /// Human-readable reason for the policy update.
    pub reason: String,
    /// Idempotency key supplied by the caller.
    pub idempotency_key: String,
    /// Unix timestamp when the service accepted this policy update.
    pub observed_at_unix: u64,
}

/// Result returned after recording a reserve lifecycle policy update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveLifecyclePolicyOutcome {
    /// Accepted or previously recorded policy update.
    pub record: ReserveLifecyclePolicyRecord,
    /// Lifecycle events emitted while reprojecting current providers under the policy.
    pub reprojected_events: Vec<ReserveLifecycleEvent>,
    /// Whether this response came from a prior idempotent policy update.
    pub duplicate: bool,
}

/// Point-in-time reserve lifecycle policy snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReserveLifecyclePolicySnapshot {
    /// Next sequence that will be assigned to a reserve lifecycle policy record.
    pub next_sequence: u64,
    /// Unix timestamp used when the snapshot was produced.
    pub generated_at_unix: u64,
    /// Latest policy record by local sequence, when any update has been accepted.
    pub latest: Option<ReserveLifecyclePolicyRecord>,
    /// Policy update records sorted by local sequence.
    pub policies: Vec<ReserveLifecyclePolicyRecord>,
}

/// Point-in-time reserve movement ledger snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReserveMovementSnapshot {
    /// Next sequence that will be assigned to a reserve movement.
    pub next_sequence: u64,
    /// Unix timestamp used when the snapshot was produced.
    pub generated_at_unix: u64,
    /// Provider balances sorted by provider id.
    pub provider_balances: Vec<ReserveProviderBalance>,
    /// Movement replay history sorted by sequence.
    pub movements: Vec<ReserveMovementRecord>,
}

/// Sequenced reserve lifecycle event retained for replay and future Torii streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveLifecycleEvent {
    /// Monotonic local event sequence.
    pub sequence: u64,
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Previous lifecycle stage for this provider, when known.
    pub previous_stage: Option<ReserveLifecycleStage>,
    /// Current lifecycle stage after the update.
    pub current_stage: ReserveLifecycleStage,
    /// Unix timestamp supplied with the accepted update.
    pub observed_at_unix: u64,
    /// Latest ledger projection for the provider.
    pub ledger: ReserveLedgerProjection,
    /// Latest lifecycle projection for the provider.
    pub lifecycle: ReserveLifecycleProjection,
    /// Grace window that was used for the lifecycle projection.
    pub grace_period_days: u16,
    /// Default threshold that was used for the lifecycle projection.
    pub default_after_days: u16,
    /// Applied lifecycle-policy record id, when the projection used a local policy update.
    pub applied_policy_id: Option<[u8; 32]>,
    /// Applied accepted appeal id, when the lifecycle stage was overridden by governance.
    pub applied_appeal_id: Option<[u8; 32]>,
}

/// Point-in-time reserve lifecycle runtime snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReserveLifecycleSnapshot {
    /// Next sequence that will be assigned to a reserve lifecycle event.
    pub next_sequence: u64,
    /// Unix timestamp used when the snapshot was produced.
    pub generated_at_unix: u64,
    /// Provider summaries sorted by provider id.
    pub providers: Vec<ReserveProviderLifecycleSummary>,
    /// Event replay history sorted by sequence.
    pub events: Vec<ReserveLifecycleEvent>,
}

/// In-memory reserve lifecycle runtime.
#[derive(Debug, Default)]
pub(crate) struct ReserveLifecycleRuntime {
    next_sequence: u64,
    next_movement_sequence: u64,
    next_appeal_sequence: u64,
    next_policy_sequence: u64,
    providers: BTreeMap<[u8; 32], ReserveProviderLifecycleSummary>,
    events: Vec<ReserveLifecycleEvent>,
    provider_balances: BTreeMap<[u8; 32], ReserveProviderBalance>,
    provider_credit_lines: BTreeMap<[u8; 32], ReserveProviderCreditLineState>,
    movements_by_id: BTreeMap<[u8; 32], ReserveMovementRecord>,
    movements: Vec<ReserveMovementRecord>,
    appeals_by_id: BTreeMap<[u8; 32], ReserveAppealRecord>,
    appeals: Vec<ReserveAppealRecord>,
    lifecycle_policies_by_id: BTreeMap<[u8; 32], ReserveLifecyclePolicyRecord>,
    lifecycle_policies: Vec<ReserveLifecyclePolicyRecord>,
}

impl ReserveLifecycleRuntime {
    /// Record a provider lifecycle update and append a replay event.
    pub(crate) fn record_update(
        &mut self,
        update: ReserveLifecycleUpdate,
    ) -> Result<ReserveLifecycleEvent, ReserveLifecycleRuntimeError> {
        let applied_policy = self.effective_lifecycle_policy(update.observed_at_unix);
        let grace_period_days = applied_policy
            .as_ref()
            .map_or(update.grace_period_days, |policy| policy.grace_period_days);
        let default_after_days = applied_policy
            .as_ref()
            .map_or(update.default_after_days, |policy| {
                policy.default_after_days
            });
        let applied_policy_id = applied_policy.as_ref().map(|policy| policy.policy_id);
        let ledger = update.quote.ledger_projection();
        let lifecycle = update
            .quote
            .lifecycle_projection(update.days_past_due, grace_period_days, default_after_days)
            .map_err(|err| ReserveLifecycleRuntimeError::ProjectionFailed(err.to_string()))?;
        let applied_appeal =
            self.effective_appeal_override(update.provider_id, update.observed_at_unix);
        let lifecycle = applied_appeal
            .as_ref()
            .and_then(|appeal| appeal.requested_stage)
            .map_or(lifecycle, |stage| {
                lifecycle_with_stage_override(lifecycle, stage)
            });
        let applied_appeal_id = applied_appeal.as_ref().map(|appeal| appeal.appeal_id);
        let previous_stage = self
            .providers
            .get(&update.provider_id)
            .map(|summary| summary.lifecycle.stage);
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(ReserveLifecycleRuntimeError::EventSequenceOverflow)?;

        let summary = ReserveProviderLifecycleSummary {
            provider_id: update.provider_id,
            provider_account: update.provider_account,
            quote: update.quote,
            ledger,
            lifecycle,
            grace_period_days,
            default_after_days,
            applied_policy_id,
            applied_appeal_id,
            updated_at_unix: update.observed_at_unix,
        };
        self.providers.insert(update.provider_id, summary.clone());
        let credit_line = reserve_credit_line_state_from_lifecycle(
            update.provider_id,
            &summary.provider_account,
            sequence,
            lifecycle,
            applied_appeal_id,
            update.observed_at_unix,
        );
        self.provider_credit_lines
            .insert(update.provider_id, credit_line);

        let event = ReserveLifecycleEvent {
            sequence,
            provider_id: update.provider_id,
            previous_stage,
            current_stage: lifecycle.stage,
            observed_at_unix: update.observed_at_unix,
            ledger,
            lifecycle,
            grace_period_days,
            default_after_days,
            applied_policy_id,
            applied_appeal_id,
        };
        self.events.push(event.clone());
        Ok(event)
    }

    /// Advance retained provider lifecycle projections to an explicit service timestamp.
    pub(crate) fn advance_lifecycle_to(
        &mut self,
        observed_at_unix: u64,
    ) -> Result<Vec<ReserveLifecycleEvent>, ReserveLifecycleRuntimeError> {
        let provider_ids = self.providers.keys().copied().collect::<Vec<_>>();
        let mut advanced_events = Vec::new();
        for provider_id in provider_ids {
            let Some(summary) = self.providers.get(&provider_id).cloned() else {
                continue;
            };
            let elapsed_secs = observed_at_unix.saturating_sub(summary.updated_at_unix);
            let elapsed_days = elapsed_secs / RESERVE_LIFECYCLE_DAY_SECS;
            if elapsed_days == 0 {
                continue;
            }
            let days_past_due = summary
                .lifecycle
                .days_past_due
                .saturating_add(u16::try_from(elapsed_days).unwrap_or(u16::MAX));
            if days_past_due == summary.lifecycle.days_past_due {
                continue;
            }

            let applied_policy = self.effective_lifecycle_policy(observed_at_unix);
            let grace_period_days = applied_policy
                .as_ref()
                .map_or(summary.grace_period_days, |policy| policy.grace_period_days);
            let default_after_days = applied_policy
                .as_ref()
                .map_or(summary.default_after_days, |policy| {
                    policy.default_after_days
                });
            let applied_policy_id = applied_policy.as_ref().map(|policy| policy.policy_id);
            let lifecycle = summary
                .quote
                .lifecycle_projection(days_past_due, grace_period_days, default_after_days)
                .map_err(|err| ReserveLifecycleRuntimeError::ProjectionFailed(err.to_string()))?;
            let applied_appeal = self.effective_appeal_override(provider_id, observed_at_unix);
            let lifecycle = applied_appeal
                .as_ref()
                .and_then(|appeal| appeal.requested_stage)
                .map_or(lifecycle, |stage| {
                    lifecycle_with_stage_override(lifecycle, stage)
                });
            let applied_appeal_id = applied_appeal.as_ref().map(|appeal| appeal.appeal_id);

            let sequence = self.next_sequence;
            self.next_sequence = self
                .next_sequence
                .checked_add(1)
                .ok_or(ReserveLifecycleRuntimeError::EventSequenceOverflow)?;
            let updated_summary = ReserveProviderLifecycleSummary {
                lifecycle,
                grace_period_days,
                default_after_days,
                applied_policy_id,
                applied_appeal_id,
                updated_at_unix: observed_at_unix,
                ..summary.clone()
            };
            let credit_line = reserve_credit_line_state_from_lifecycle(
                provider_id,
                &updated_summary.provider_account,
                sequence,
                lifecycle,
                applied_appeal_id,
                observed_at_unix,
            );
            let event = ReserveLifecycleEvent {
                sequence,
                provider_id,
                previous_stage: Some(summary.lifecycle.stage),
                current_stage: lifecycle.stage,
                observed_at_unix,
                ledger: updated_summary.ledger,
                lifecycle,
                grace_period_days,
                default_after_days,
                applied_policy_id,
                applied_appeal_id,
            };
            self.providers.insert(provider_id, updated_summary);
            self.provider_credit_lines.insert(provider_id, credit_line);
            self.events.push(event.clone());
            advanced_events.push(event);
        }
        Ok(advanced_events)
    }

    fn effective_lifecycle_policy(
        &self,
        observed_at_unix: u64,
    ) -> Option<ReserveLifecyclePolicyRecord> {
        self.lifecycle_policies
            .iter()
            .filter(|policy| policy.effective_at_unix <= observed_at_unix)
            .max_by_key(|policy| (policy.effective_at_unix, policy.sequence))
            .cloned()
    }

    fn effective_appeal_override(
        &self,
        provider_id: [u8; 32],
        observed_at_unix: u64,
    ) -> Option<ReserveAppealRecord> {
        self.appeals
            .iter()
            .filter(|appeal| {
                appeal.provider_id == provider_id
                    && appeal.status == ReserveAppealStatus::Accepted
                    && appeal.requested_stage.is_some()
                    && appeal
                        .decided_at_unix
                        .is_some_and(|decided_at| decided_at <= observed_at_unix)
            })
            .max_by_key(|appeal| (appeal.decided_at_unix.unwrap_or(0), appeal.sequence))
            .cloned()
    }

    /// Return a provider summary by provider id.
    pub(crate) fn provider_summary(
        &self,
        provider_id: [u8; 32],
    ) -> Option<ReserveProviderLifecycleSummary> {
        self.providers.get(&provider_id).cloned()
    }

    /// Return one provider's local credit-line state.
    pub(crate) fn provider_credit_line(
        &self,
        provider_id: [u8; 32],
    ) -> Option<ReserveProviderCreditLineState> {
        self.provider_credit_lines.get(&provider_id).cloned()
    }

    /// Return retained events after the optional sequence, bounded by `limit`.
    pub(crate) fn events_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<ReserveLifecycleEvent> {
        if limit == 0 {
            return Vec::new();
        }
        self.events
            .iter()
            .filter(|event| match since_sequence {
                Some(sequence) => event.sequence > sequence,
                None => true,
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Return the latest retained event sequence.
    pub(crate) fn latest_event_sequence(&self) -> Option<u64> {
        self.events.last().map(|event| event.sequence)
    }

    /// Return a point-in-time snapshot of the runtime.
    pub(crate) fn snapshot(&self, generated_at_unix: u64) -> ReserveLifecycleSnapshot {
        ReserveLifecycleSnapshot {
            next_sequence: self.next_sequence,
            generated_at_unix,
            providers: self.providers.values().cloned().collect(),
            events: self.events.clone(),
        }
    }

    /// Return a point-in-time snapshot of local reserve credit-line state.
    pub(crate) fn credit_line_snapshot(&self, generated_at_unix: u64) -> ReserveCreditLineSnapshot {
        ReserveCreditLineSnapshot {
            generated_at_unix,
            credit_lines: self.provider_credit_lines.values().cloned().collect(),
        }
    }

    /// Record a local reserve movement and update the provider reserve balance.
    pub(crate) fn record_movement(
        &mut self,
        request: ReserveMovementRequest,
    ) -> Result<ReserveMovementOutcome, ReserveMovementRuntimeError> {
        if request.amount.is_zero() {
            return Err(ReserveMovementRuntimeError::ZeroAmount);
        }
        if let Some(existing) = self.movements_by_id.get(&request.movement_id) {
            if movement_record_matches_request(existing, &request) {
                return Ok(ReserveMovementOutcome {
                    record: existing.clone(),
                    duplicate: true,
                });
            }
            return Err(ReserveMovementRuntimeError::MovementIdCollision {
                movement_id_hex: hex::encode(request.movement_id),
            });
        }

        let current_balance = self
            .provider_balances
            .get(&request.provider_id)
            .map_or_else(XorAmount::zero, |balance| balance.balance);
        let current_confirmed_balance = self
            .provider_balances
            .get(&request.provider_id)
            .map_or_else(XorAmount::zero, |balance| balance.confirmed_balance);
        let balance_after = apply_reserve_movement_balance(
            request.kind,
            request.provider_id,
            current_balance,
            request.amount,
        )?;
        let sequence = self.next_movement_sequence;
        self.next_movement_sequence = self
            .next_movement_sequence
            .checked_add(1)
            .ok_or(ReserveMovementRuntimeError::EventSequenceOverflow)?;

        let record = ReserveMovementRecord {
            sequence,
            movement_id: request.movement_id,
            provider_id: request.provider_id,
            provider_account: request.provider_account,
            reserve_account: request.reserve_account,
            asset_definition_id: request.asset_definition_id,
            kind: request.kind,
            amount: request.amount,
            balance_after,
            confirmed_balance_after: current_confirmed_balance,
            idempotency_key: request.idempotency_key,
            observed_at_unix: request.observed_at_unix,
            custody_status: ReserveMovementCustodyStatus::IntentRecorded,
            custody_tx_hash_hex: None,
            custody_updated_at_unix: None,
        };
        let balance = ReserveProviderBalance {
            provider_id: record.provider_id,
            provider_account: record.provider_account.clone(),
            reserve_account: record.reserve_account.clone(),
            asset_definition_id: record.asset_definition_id.clone(),
            balance: record.balance_after,
            confirmed_balance: record.confirmed_balance_after,
            updated_at_unix: record.observed_at_unix,
        };
        self.provider_balances.insert(record.provider_id, balance);
        self.movements_by_id
            .insert(record.movement_id, record.clone());
        self.movements.push(record.clone());
        Ok(ReserveMovementOutcome {
            record,
            duplicate: false,
        })
    }

    /// Return one recorded reserve movement by movement id.
    pub(crate) fn movement(&self, movement_id: [u8; 32]) -> Option<ReserveMovementRecord> {
        self.movements_by_id.get(&movement_id).cloned()
    }

    /// Attach chain custody evidence to a recorded reserve movement.
    pub(crate) fn record_movement_custody_update(
        &mut self,
        update: ReserveMovementCustodyUpdate,
    ) -> Result<ReserveMovementRecord, ReserveMovementRuntimeError> {
        let Some(existing) = self.movements_by_id.get(&update.movement_id).cloned() else {
            return Err(ReserveMovementRuntimeError::MovementNotFound {
                movement_id_hex: hex::encode(update.movement_id),
            });
        };
        if existing.custody_status == update.status
            && existing
                .custody_tx_hash_hex
                .as_deref()
                .is_some_and(|tx_hash| tx_hash == update.tx_hash_hex)
        {
            return Ok(existing);
        }
        validate_custody_transition(&existing, &update)?;

        let mut movements = self.movements.clone();
        let Some(position) = movements
            .iter()
            .position(|movement| movement.movement_id == update.movement_id)
        else {
            return Err(ReserveMovementRuntimeError::MovementNotFound {
                movement_id_hex: hex::encode(update.movement_id),
            });
        };
        movements[position].custody_status = update.status;
        movements[position].custody_tx_hash_hex = Some(update.tx_hash_hex);
        movements[position].custody_updated_at_unix = Some(update.observed_at_unix);

        let (provider_balances, movements) = recompute_reserve_movement_balances(&movements)?;
        let record = movements[position].clone();
        self.provider_balances = provider_balances;
        self.movements_by_id = movements
            .iter()
            .map(|movement| (movement.movement_id, movement.clone()))
            .collect();
        self.movements = movements;
        Ok(record)
    }

    /// Return one provider's local reserve balance.
    pub(crate) fn provider_balance(&self, provider_id: [u8; 32]) -> Option<ReserveProviderBalance> {
        self.provider_balances.get(&provider_id).cloned()
    }

    /// Return retained movement records after the optional sequence, bounded by `limit`.
    pub(crate) fn movements_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<ReserveMovementRecord> {
        if limit == 0 {
            return Vec::new();
        }
        self.movements
            .iter()
            .filter(|movement| match since_sequence {
                Some(sequence) => movement.sequence > sequence,
                None => true,
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Return the latest retained reserve movement sequence.
    pub(crate) fn latest_movement_sequence(&self) -> Option<u64> {
        self.movements.last().map(|movement| movement.sequence)
    }

    /// Return a point-in-time snapshot of the reserve movement ledger.
    pub(crate) fn movement_snapshot(&self, generated_at_unix: u64) -> ReserveMovementSnapshot {
        ReserveMovementSnapshot {
            next_sequence: self.next_movement_sequence,
            generated_at_unix,
            provider_balances: self.provider_balances.values().cloned().collect(),
            movements: self.movements.clone(),
        }
    }

    /// Record a local reserve appeal.
    pub(crate) fn record_appeal(
        &mut self,
        request: ReserveAppealRequest,
    ) -> Result<ReserveAppealOutcome, ReserveAppealRuntimeError> {
        validate_required_text("reason", &request.reason)?;
        validate_required_text("idempotency_key", &request.idempotency_key)?;
        if let Some(existing) = self.appeals_by_id.get(&request.appeal_id) {
            if appeal_record_matches_request(existing, &request) {
                return Ok(ReserveAppealOutcome {
                    record: existing.clone(),
                    duplicate: true,
                });
            }
            return Err(ReserveAppealRuntimeError::AppealIdCollision {
                appeal_id_hex: hex::encode(request.appeal_id),
            });
        }

        let sequence = self.next_appeal_sequence;
        self.next_appeal_sequence = self
            .next_appeal_sequence
            .checked_add(1)
            .ok_or(ReserveAppealRuntimeError::EventSequenceOverflow)?;
        let record = ReserveAppealRecord {
            sequence,
            appeal_id: request.appeal_id,
            provider_id: request.provider_id,
            provider_account: request.provider_account,
            requested_stage: request.requested_stage,
            reason: request.reason,
            evidence_digest_hex: request.evidence_digest_hex,
            idempotency_key: request.idempotency_key,
            status: ReserveAppealStatus::Open,
            opened_at_unix: request.observed_at_unix,
            decision_account: None,
            decision_rationale: None,
            decided_at_unix: None,
        };
        self.appeals_by_id.insert(record.appeal_id, record.clone());
        self.appeals.push(record.clone());
        Ok(ReserveAppealOutcome {
            record,
            duplicate: false,
        })
    }

    /// Return one local reserve appeal by appeal id.
    pub(crate) fn appeal(&self, appeal_id: [u8; 32]) -> Option<ReserveAppealRecord> {
        self.appeals_by_id.get(&appeal_id).cloned()
    }

    /// Record a local decision for an open reserve appeal.
    pub(crate) fn record_appeal_decision(
        &mut self,
        decision: ReserveAppealDecision,
    ) -> Result<ReserveAppealDecisionOutcome, ReserveAppealRuntimeError> {
        validate_required_text("rationale", &decision.rationale)?;
        if !decision.status.is_terminal() {
            return Err(ReserveAppealRuntimeError::InvalidDecisionStatus);
        }
        let Some(existing) = self.appeals_by_id.get(&decision.appeal_id).cloned() else {
            return Err(ReserveAppealRuntimeError::AppealNotFound {
                appeal_id_hex: hex::encode(decision.appeal_id),
            });
        };
        if existing.status == decision.status
            && existing
                .decision_account
                .as_ref()
                .is_some_and(|account| *account == decision.decision_account)
            && existing
                .decision_rationale
                .as_deref()
                .is_some_and(|rationale| rationale == decision.rationale)
            && existing.decided_at_unix == Some(decision.decided_at_unix)
        {
            return Ok(ReserveAppealDecisionOutcome {
                record: existing,
                lifecycle_event: None,
                duplicate: true,
            });
        }
        if existing.status.is_terminal() {
            return Err(ReserveAppealRuntimeError::InvalidAppealTransition {
                appeal_id_hex: hex::encode(existing.appeal_id),
                from: existing.status.label(),
                to: decision.status.label(),
            });
        }

        let mut record = existing;
        record.status = decision.status;
        record.decision_account = Some(decision.decision_account);
        record.decision_rationale = Some(decision.rationale);
        record.decided_at_unix = Some(decision.decided_at_unix);
        let lifecycle_event = if record.status == ReserveAppealStatus::Accepted {
            self.apply_accepted_appeal_override(&record)?
        } else {
            None
        };
        if let Some(position) = self
            .appeals
            .iter()
            .position(|appeal| appeal.appeal_id == decision.appeal_id)
        {
            self.appeals[position] = record.clone();
        }
        self.appeals_by_id.insert(record.appeal_id, record.clone());
        Ok(ReserveAppealDecisionOutcome {
            record,
            lifecycle_event,
            duplicate: false,
        })
    }

    fn apply_accepted_appeal_override(
        &mut self,
        appeal: &ReserveAppealRecord,
    ) -> Result<Option<ReserveLifecycleEvent>, ReserveAppealRuntimeError> {
        let Some(requested_stage) = appeal.requested_stage else {
            return Ok(None);
        };
        let Some(decided_at_unix) = appeal.decided_at_unix else {
            return Ok(None);
        };
        let Some(summary) = self.providers.get(&appeal.provider_id).cloned() else {
            return Ok(None);
        };
        if summary.lifecycle.stage == requested_stage
            && summary.applied_appeal_id == Some(appeal.appeal_id)
        {
            return Ok(None);
        }

        let sequence = self.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(ReserveAppealRuntimeError::EventSequenceOverflow)?;
        let lifecycle = lifecycle_with_stage_override(summary.lifecycle, requested_stage);
        let updated_summary = ReserveProviderLifecycleSummary {
            lifecycle,
            applied_appeal_id: Some(appeal.appeal_id),
            updated_at_unix: decided_at_unix,
            ..summary.clone()
        };
        let credit_line = reserve_credit_line_state_from_lifecycle(
            appeal.provider_id,
            &updated_summary.provider_account,
            sequence,
            lifecycle,
            Some(appeal.appeal_id),
            decided_at_unix,
        );
        let event = ReserveLifecycleEvent {
            sequence,
            provider_id: appeal.provider_id,
            previous_stage: Some(summary.lifecycle.stage),
            current_stage: lifecycle.stage,
            observed_at_unix: decided_at_unix,
            ledger: updated_summary.ledger,
            lifecycle,
            grace_period_days: updated_summary.grace_period_days,
            default_after_days: updated_summary.default_after_days,
            applied_policy_id: updated_summary.applied_policy_id,
            applied_appeal_id: Some(appeal.appeal_id),
        };

        self.next_sequence = next_sequence;
        self.providers.insert(appeal.provider_id, updated_summary);
        self.provider_credit_lines
            .insert(appeal.provider_id, credit_line);
        self.events.push(event.clone());
        Ok(Some(event))
    }

    /// Return a point-in-time snapshot of local reserve appeals.
    pub(crate) fn appeal_snapshot(&self, generated_at_unix: u64) -> ReserveAppealSnapshot {
        ReserveAppealSnapshot {
            next_sequence: self.next_appeal_sequence,
            generated_at_unix,
            appeals: self.appeals.clone(),
        }
    }

    /// Record a local reserve lifecycle policy update.
    pub(crate) fn record_lifecycle_policy_update(
        &mut self,
        update: ReserveLifecyclePolicyUpdate,
    ) -> Result<ReserveLifecyclePolicyOutcome, ReserveAppealRuntimeError> {
        validate_required_text("reason", &update.reason)?;
        validate_required_text("idempotency_key", &update.idempotency_key)?;
        if update.grace_period_days >= update.default_after_days {
            return Err(ReserveAppealRuntimeError::InvalidLifecyclePolicyWindow {
                grace_period_days: update.grace_period_days,
                default_after_days: update.default_after_days,
            });
        }
        if let Some(existing) = self.lifecycle_policies_by_id.get(&update.policy_id) {
            if lifecycle_policy_record_matches_update(existing, &update) {
                return Ok(ReserveLifecyclePolicyOutcome {
                    record: existing.clone(),
                    reprojected_events: Vec::new(),
                    duplicate: true,
                });
            }
            return Err(ReserveAppealRuntimeError::PolicyIdCollision {
                policy_id_hex: hex::encode(update.policy_id),
            });
        }

        let sequence = self.next_policy_sequence;
        self.next_policy_sequence = self
            .next_policy_sequence
            .checked_add(1)
            .ok_or(ReserveAppealRuntimeError::EventSequenceOverflow)?;
        let record = ReserveLifecyclePolicyRecord {
            sequence,
            policy_id: update.policy_id,
            authority_account: update.authority_account,
            grace_period_days: update.grace_period_days,
            default_after_days: update.default_after_days,
            effective_at_unix: update.effective_at_unix,
            reason: update.reason,
            idempotency_key: update.idempotency_key,
            observed_at_unix: update.observed_at_unix,
        };
        self.lifecycle_policies_by_id
            .insert(record.policy_id, record.clone());
        self.lifecycle_policies.push(record.clone());
        let reprojected_events = self.reproject_providers_for_lifecycle_policy(&record)?;
        Ok(ReserveLifecyclePolicyOutcome {
            record,
            reprojected_events,
            duplicate: false,
        })
    }

    fn reproject_providers_for_lifecycle_policy(
        &mut self,
        policy: &ReserveLifecyclePolicyRecord,
    ) -> Result<Vec<ReserveLifecycleEvent>, ReserveAppealRuntimeError> {
        if policy.effective_at_unix > policy.observed_at_unix {
            return Ok(Vec::new());
        }
        let provider_ids = self.providers.keys().copied().collect::<Vec<_>>();
        let mut reprojected_events = Vec::new();
        for provider_id in provider_ids {
            let Some(summary) = self.providers.get(&provider_id).cloned() else {
                continue;
            };
            if summary.updated_at_unix < policy.effective_at_unix {
                continue;
            }
            let effective_policy = self.effective_lifecycle_policy(summary.updated_at_unix);
            if effective_policy.as_ref().map(|record| record.policy_id) != Some(policy.policy_id) {
                continue;
            }
            if summary.applied_policy_id == Some(policy.policy_id)
                && summary.grace_period_days == policy.grace_period_days
                && summary.default_after_days == policy.default_after_days
            {
                continue;
            }

            let base_lifecycle = summary
                .quote
                .lifecycle_projection(
                    summary.lifecycle.days_past_due,
                    policy.grace_period_days,
                    policy.default_after_days,
                )
                .map_err(|err| {
                    ReserveAppealRuntimeError::LifecyclePolicyProjectionFailed(err.to_string())
                })?;
            let applied_appeal =
                self.effective_appeal_override(provider_id, policy.observed_at_unix);
            let lifecycle = applied_appeal
                .as_ref()
                .and_then(|appeal| appeal.requested_stage)
                .map_or(base_lifecycle, |stage| {
                    lifecycle_with_stage_override(base_lifecycle, stage)
                });
            let applied_appeal_id = applied_appeal.as_ref().map(|appeal| appeal.appeal_id);
            let sequence = self.next_sequence;
            self.next_sequence = self
                .next_sequence
                .checked_add(1)
                .ok_or(ReserveAppealRuntimeError::EventSequenceOverflow)?;

            let updated_summary = ReserveProviderLifecycleSummary {
                lifecycle,
                grace_period_days: policy.grace_period_days,
                default_after_days: policy.default_after_days,
                applied_policy_id: Some(policy.policy_id),
                applied_appeal_id,
                updated_at_unix: policy.observed_at_unix,
                ..summary.clone()
            };
            let credit_line = reserve_credit_line_state_from_lifecycle(
                provider_id,
                &updated_summary.provider_account,
                sequence,
                lifecycle,
                applied_appeal_id,
                policy.observed_at_unix,
            );
            let event = ReserveLifecycleEvent {
                sequence,
                provider_id,
                previous_stage: Some(summary.lifecycle.stage),
                current_stage: lifecycle.stage,
                observed_at_unix: policy.observed_at_unix,
                ledger: updated_summary.ledger,
                lifecycle,
                grace_period_days: policy.grace_period_days,
                default_after_days: policy.default_after_days,
                applied_policy_id: Some(policy.policy_id),
                applied_appeal_id,
            };
            self.providers.insert(provider_id, updated_summary);
            self.provider_credit_lines.insert(provider_id, credit_line);
            self.events.push(event.clone());
            reprojected_events.push(event);
        }
        Ok(reprojected_events)
    }

    /// Return the latest local reserve lifecycle policy update by sequence.
    pub(crate) fn latest_lifecycle_policy(&self) -> Option<ReserveLifecyclePolicyRecord> {
        self.lifecycle_policies.last().cloned()
    }

    /// Return a point-in-time snapshot of local reserve lifecycle policy records.
    pub(crate) fn lifecycle_policy_snapshot(
        &self,
        generated_at_unix: u64,
    ) -> ReserveLifecyclePolicySnapshot {
        ReserveLifecyclePolicySnapshot {
            next_sequence: self.next_policy_sequence,
            generated_at_unix,
            latest: self.latest_lifecycle_policy(),
            policies: self.lifecycle_policies.clone(),
        }
    }
}

fn movement_record_matches_request(
    record: &ReserveMovementRecord,
    request: &ReserveMovementRequest,
) -> bool {
    record.provider_id == request.provider_id
        && record.provider_account == request.provider_account
        && record.reserve_account == request.reserve_account
        && record.asset_definition_id == request.asset_definition_id
        && record.kind == request.kind
        && record.amount == request.amount
        && record.idempotency_key == request.idempotency_key
}

fn recompute_reserve_movement_balances(
    movements: &[ReserveMovementRecord],
) -> Result<RecomputedReserveMovementBalances, ReserveMovementRuntimeError> {
    let mut provider_balances = BTreeMap::<[u8; 32], ReserveProviderBalance>::new();
    let mut reconciled_movements = Vec::with_capacity(movements.len());
    for movement in movements {
        let current_balance = provider_balances
            .get(&movement.provider_id)
            .map_or_else(XorAmount::zero, |balance| balance.balance);
        let current_confirmed_balance = provider_balances
            .get(&movement.provider_id)
            .map_or_else(XorAmount::zero, |balance| balance.confirmed_balance);
        let mut movement = movement.clone();
        if movement.custody_status == ReserveMovementCustodyStatus::Rejected {
            movement.balance_after = current_balance;
            movement.confirmed_balance_after = current_confirmed_balance;
            reconciled_movements.push(movement);
            continue;
        }

        let balance_after = apply_reserve_movement_balance(
            movement.kind,
            movement.provider_id,
            current_balance,
            movement.amount,
        )?;
        let confirmed_balance_after =
            if movement.custody_status == ReserveMovementCustodyStatus::Confirmed {
                apply_reserve_movement_balance(
                    movement.kind,
                    movement.provider_id,
                    current_confirmed_balance,
                    movement.amount,
                )?
            } else {
                current_confirmed_balance
            };
        movement.balance_after = balance_after;
        movement.confirmed_balance_after = confirmed_balance_after;
        provider_balances.insert(
            movement.provider_id,
            ReserveProviderBalance {
                provider_id: movement.provider_id,
                provider_account: movement.provider_account.clone(),
                reserve_account: movement.reserve_account.clone(),
                asset_definition_id: movement.asset_definition_id.clone(),
                balance: movement.balance_after,
                confirmed_balance: movement.confirmed_balance_after,
                updated_at_unix: movement.observed_at_unix,
            },
        );
        reconciled_movements.push(movement);
    }
    Ok((provider_balances, reconciled_movements))
}

fn apply_reserve_movement_balance(
    kind: ReserveMovementKind,
    provider_id: [u8; 32],
    current_balance: XorAmount,
    amount: XorAmount,
) -> Result<XorAmount, ReserveMovementRuntimeError> {
    match kind {
        ReserveMovementKind::TopUp => current_balance
            .checked_add(amount)
            .map_err(|err| ReserveMovementRuntimeError::ArithmeticFailed(err.to_string())),
        ReserveMovementKind::Withdrawal => current_balance.checked_sub(amount).map_err(|_| {
            ReserveMovementRuntimeError::InsufficientBalance {
                provider_id_hex: hex::encode(provider_id),
                requested_micro_xor: amount.as_micro(),
                available_micro_xor: current_balance.as_micro(),
            }
        }),
    }
}

fn validate_custody_transition(
    record: &ReserveMovementRecord,
    update: &ReserveMovementCustodyUpdate,
) -> Result<(), ReserveMovementRuntimeError> {
    if update.status == ReserveMovementCustodyStatus::IntentRecorded
        || record.custody_status.is_terminal()
    {
        return Err(ReserveMovementRuntimeError::InvalidCustodyTransition {
            movement_id_hex: hex::encode(record.movement_id),
            from: record.custody_status.label(),
            to: update.status.label(),
        });
    }
    if let Some(existing_tx_hash_hex) = record.custody_tx_hash_hex.as_deref()
        && existing_tx_hash_hex != update.tx_hash_hex
    {
        return Err(
            ReserveMovementRuntimeError::CustodyTransactionHashMismatch {
                movement_id_hex: hex::encode(record.movement_id),
                existing_tx_hash_hex: existing_tx_hash_hex.to_owned(),
                requested_tx_hash_hex: update.tx_hash_hex.clone(),
            },
        );
    }
    Ok(())
}

fn validate_required_text(
    field: &'static str,
    value: &str,
) -> Result<(), ReserveAppealRuntimeError> {
    if value.trim().is_empty() {
        Err(ReserveAppealRuntimeError::EmptyText { field })
    } else {
        Ok(())
    }
}

fn appeal_record_matches_request(
    record: &ReserveAppealRecord,
    request: &ReserveAppealRequest,
) -> bool {
    record.provider_id == request.provider_id
        && record.provider_account == request.provider_account
        && record.requested_stage == request.requested_stage
        && record.reason == request.reason
        && record.evidence_digest_hex == request.evidence_digest_hex
        && record.idempotency_key == request.idempotency_key
        && record.opened_at_unix == request.observed_at_unix
}

fn lifecycle_policy_record_matches_update(
    record: &ReserveLifecyclePolicyRecord,
    update: &ReserveLifecyclePolicyUpdate,
) -> bool {
    record.authority_account == update.authority_account
        && record.grace_period_days == update.grace_period_days
        && record.default_after_days == update.default_after_days
        && record.effective_at_unix == update.effective_at_unix
        && record.reason == update.reason
        && record.idempotency_key == update.idempotency_key
        && record.observed_at_unix == update.observed_at_unix
}

fn lifecycle_with_stage_override(
    mut lifecycle: ReserveLifecycleProjection,
    stage: ReserveLifecycleStage,
) -> ReserveLifecycleProjection {
    lifecycle.stage = stage;
    lifecycle.restrict_new_manifests = !matches!(stage, ReserveLifecycleStage::Active);
    lifecycle.disable_adverts = matches!(stage, ReserveLifecycleStage::Default);
    lifecycle.requires_governance_notification = matches!(
        stage,
        ReserveLifecycleStage::Delinquent | ReserveLifecycleStage::Default
    );
    lifecycle
}

fn reserve_credit_line_state_from_lifecycle(
    provider_id: [u8; 32],
    provider_account: &[u8],
    lifecycle_event_sequence: u64,
    lifecycle: ReserveLifecycleProjection,
    applied_appeal_id: Option<[u8; 32]>,
    updated_at_unix: u64,
) -> ReserveProviderCreditLineState {
    ReserveProviderCreditLineState {
        provider_id,
        provider_account: provider_account.to_vec(),
        lifecycle_event_sequence,
        stage: lifecycle.stage,
        rent_due: lifecycle.rent_due,
        credit_draw: lifecycle.credit_draw,
        credit_available_after_draw: lifecycle.credit_available_after_draw,
        credit_shortfall: lifecycle.credit_shortfall,
        accrued_interest: lifecycle.accrued_interest,
        total_due_after_credit: lifecycle.total_due_after_credit,
        requires_manual_credit_approval: lifecycle.requires_manual_credit_approval,
        requires_governance_notification: lifecycle.requires_governance_notification,
        applied_appeal_id,
        updated_at_unix,
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::sorafs::{
        pin_registry::StorageClass,
        reserve::{ReserveDuration, ReservePolicyV1, ReserveTier},
    };

    use super::*;

    fn update(
        provider_byte: u8,
        days_past_due: u16,
        reserve_balance: XorAmount,
    ) -> ReserveLifecycleUpdate {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                reserve_balance,
            )
            .expect("quote");
        ReserveLifecycleUpdate {
            provider_id: [provider_byte; 32],
            provider_account: vec![provider_byte],
            quote,
            days_past_due,
            grace_period_days: 7,
            default_after_days: 30,
            observed_at_unix: u64::from(days_past_due) + 100,
        }
    }

    #[test]
    fn runtime_records_provider_summaries_and_stage_transitions() {
        let mut runtime = ReserveLifecycleRuntime::default();

        let first = runtime
            .record_update(update(0x11, 0, XorAmount::zero()))
            .expect("record warning");
        assert_eq!(first.sequence, 0);
        assert_eq!(first.previous_stage, None);
        assert_eq!(first.current_stage, ReserveLifecycleStage::Warning);

        let second = runtime
            .record_update(update(0x11, 3, XorAmount::zero()))
            .expect("record grace");
        assert_eq!(second.sequence, 1);
        assert_eq!(second.previous_stage, Some(ReserveLifecycleStage::Warning));
        assert_eq!(second.current_stage, ReserveLifecycleStage::Grace);

        let summary = runtime
            .provider_summary([0x11; 32])
            .expect("provider summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Grace);
        assert_eq!(summary.ledger.rent_due.as_micro(), 120_000_000);
        assert_eq!(summary.grace_period_days, 7);
        assert_eq!(summary.default_after_days, 30);
        assert_eq!(summary.applied_policy_id, None);
        assert_eq!(summary.applied_appeal_id, None);
        assert_eq!(runtime.events_since(Some(0), 10), vec![second]);
    }

    #[test]
    fn snapshot_is_sorted_by_provider_id_and_bounds_event_replay() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_update(update(0x22, 0, XorAmount::zero()))
            .expect("record provider b");
        runtime
            .record_update(update(0x11, 0, XorAmount::zero()))
            .expect("record provider a");

        let snapshot = runtime.snapshot(555);
        assert_eq!(snapshot.generated_at_unix, 555);
        assert_eq!(snapshot.next_sequence, 2);
        assert_eq!(snapshot.providers[0].provider_id, [0x11; 32]);
        assert_eq!(snapshot.providers[1].provider_id, [0x22; 32]);
        assert_eq!(runtime.events_since(None, 1).len(), 1);
        assert!(runtime.events_since(None, 0).is_empty());
    }

    #[test]
    fn runtime_records_credit_line_state_from_lifecycle_updates() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let event = runtime
            .record_update(update(0x23, 3, XorAmount::zero()))
            .expect("record grace update");

        let credit_line = runtime
            .provider_credit_line([0x23; 32])
            .expect("credit-line state");
        assert_eq!(credit_line.lifecycle_event_sequence, event.sequence);
        assert_eq!(credit_line.stage, ReserveLifecycleStage::Grace);
        assert_eq!(credit_line.rent_due.as_micro(), 120_000_000);
        assert_eq!(credit_line.credit_draw.as_micro(), 120_000_000);
        assert_eq!(
            credit_line
                .credit_available_after_draw
                .expect("automatic credit capacity")
                .as_micro(),
            120_000_000
        );
        assert!(credit_line.credit_shortfall.is_zero());
        assert!(credit_line.accrued_interest.is_zero());

        runtime
            .record_update(update(0x23, 10, XorAmount::zero()))
            .expect("record delinquent update");
        let credit_line = runtime
            .provider_credit_line([0x23; 32])
            .expect("updated credit-line state");
        assert_eq!(credit_line.stage, ReserveLifecycleStage::Delinquent);
        assert_eq!(credit_line.accrued_interest.as_micro(), 29_589);
        assert_eq!(credit_line.total_due_after_credit.as_micro(), 29_589);
        let snapshot = runtime.credit_line_snapshot(999);
        assert_eq!(snapshot.generated_at_unix, 999);
        assert_eq!(snapshot.credit_lines, vec![credit_line]);
    }

    #[test]
    fn runtime_applies_effective_lifecycle_policy_to_later_updates() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_lifecycle_policy_update(ReserveLifecyclePolicyUpdate {
                grace_period_days: 2,
                default_after_days: 5,
                effective_at_unix: 150,
                ..lifecycle_policy_update(0x24)
            })
            .expect("record policy");

        let mut update = update(0x25, 6, XorAmount::zero());
        update.observed_at_unix = 200;
        let event = runtime
            .record_update(update)
            .expect("record policy-driven update");
        assert_eq!(event.current_stage, ReserveLifecycleStage::Default);
        assert_eq!(event.grace_period_days, 2);
        assert_eq!(event.default_after_days, 5);
        assert_eq!(event.applied_policy_id, Some([0x24; 32]));
        assert_eq!(event.applied_appeal_id, None);

        let summary = runtime
            .provider_summary([0x25; 32])
            .expect("provider summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Default);
        assert_eq!(summary.grace_period_days, 2);
        assert_eq!(summary.default_after_days, 5);
        assert_eq!(summary.applied_policy_id, Some([0x24; 32]));
        assert_eq!(summary.applied_appeal_id, None);
    }

    #[test]
    fn runtime_ignores_lifecycle_policy_until_effective_time() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_lifecycle_policy_update(ReserveLifecyclePolicyUpdate {
                grace_period_days: 2,
                default_after_days: 5,
                effective_at_unix: 1_000,
                ..lifecycle_policy_update(0x26)
            })
            .expect("record future policy");

        let mut update = update(0x27, 6, XorAmount::zero());
        update.observed_at_unix = 200;
        let event = runtime
            .record_update(update)
            .expect("record pre-policy update");
        assert_eq!(event.current_stage, ReserveLifecycleStage::Grace);
        assert_eq!(event.grace_period_days, 7);
        assert_eq!(event.default_after_days, 30);
        assert_eq!(event.applied_policy_id, None);
        assert_eq!(event.applied_appeal_id, None);
    }

    #[test]
    fn runtime_reprojects_current_providers_when_policy_is_already_effective() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let event = runtime
            .record_update(update(0x28, 6, XorAmount::zero()))
            .expect("record initial lifecycle");
        assert_eq!(event.current_stage, ReserveLifecycleStage::Grace);

        let outcome = runtime
            .record_lifecycle_policy_update(ReserveLifecyclePolicyUpdate {
                grace_period_days: 2,
                default_after_days: 5,
                effective_at_unix: 100,
                observed_at_unix: 150,
                ..lifecycle_policy_update(0x29)
            })
            .expect("record effective policy");

        assert!(!outcome.duplicate);
        assert_eq!(outcome.reprojected_events.len(), 1);
        let reprojected = &outcome.reprojected_events[0];
        assert_eq!(reprojected.sequence, 1);
        assert_eq!(reprojected.provider_id, [0x28; 32]);
        assert_eq!(
            reprojected.previous_stage,
            Some(ReserveLifecycleStage::Grace)
        );
        assert_eq!(reprojected.current_stage, ReserveLifecycleStage::Default);
        assert_eq!(reprojected.observed_at_unix, 150);
        assert_eq!(reprojected.grace_period_days, 2);
        assert_eq!(reprojected.default_after_days, 5);
        assert_eq!(reprojected.applied_policy_id, Some([0x29; 32]));

        let summary = runtime
            .provider_summary([0x28; 32])
            .expect("provider summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Default);
        assert_eq!(summary.lifecycle.days_past_due, 6);
        assert_eq!(summary.grace_period_days, 2);
        assert_eq!(summary.default_after_days, 5);
        assert_eq!(summary.applied_policy_id, Some([0x29; 32]));
        assert_eq!(summary.updated_at_unix, 150);
        let credit_line = runtime
            .provider_credit_line([0x28; 32])
            .expect("credit line");
        assert_eq!(credit_line.stage, ReserveLifecycleStage::Default);
        assert_eq!(credit_line.lifecycle_event_sequence, 1);
        assert_eq!(credit_line.updated_at_unix, 150);
    }

    #[test]
    fn runtime_does_not_reproject_current_providers_for_future_policy() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_update(update(0x2A, 6, XorAmount::zero()))
            .expect("record initial lifecycle");

        let outcome = runtime
            .record_lifecycle_policy_update(ReserveLifecyclePolicyUpdate {
                grace_period_days: 2,
                default_after_days: 5,
                effective_at_unix: 200,
                observed_at_unix: 150,
                ..lifecycle_policy_update(0x2B)
            })
            .expect("record future policy");

        assert!(outcome.reprojected_events.is_empty());
        let summary = runtime
            .provider_summary([0x2A; 32])
            .expect("provider summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Grace);
        assert_eq!(summary.grace_period_days, 7);
        assert_eq!(summary.default_after_days, 30);
        assert_eq!(summary.applied_policy_id, None);
        assert_eq!(runtime.events_since(None, 10).len(), 1);
    }

    #[test]
    fn runtime_advances_lifecycle_by_whole_days() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let initial = runtime
            .record_update(update(0x2C, 29, XorAmount::zero()))
            .expect("record initial lifecycle");
        assert_eq!(initial.current_stage, ReserveLifecycleStage::Delinquent);

        let advanced = runtime
            .advance_lifecycle_to(initial.observed_at_unix + 2 * RESERVE_LIFECYCLE_DAY_SECS)
            .expect("advance lifecycle");

        assert_eq!(advanced.len(), 1);
        let event = &advanced[0];
        assert_eq!(event.sequence, 1);
        assert_eq!(event.provider_id, [0x2C; 32]);
        assert_eq!(
            event.previous_stage,
            Some(ReserveLifecycleStage::Delinquent)
        );
        assert_eq!(event.current_stage, ReserveLifecycleStage::Default);
        assert_eq!(event.lifecycle.days_past_due, 31);
        assert!(event.lifecycle.disable_adverts);
        let summary = runtime
            .provider_summary([0x2C; 32])
            .expect("provider summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Default);
        assert_eq!(summary.lifecycle.days_past_due, 31);
        let credit_line = runtime
            .provider_credit_line([0x2C; 32])
            .expect("credit line");
        assert_eq!(credit_line.stage, ReserveLifecycleStage::Default);
        assert_eq!(credit_line.lifecycle_event_sequence, 1);
    }

    #[test]
    fn runtime_advance_ignores_subday_elapsed_time() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let initial = runtime
            .record_update(update(0x2D, 29, XorAmount::zero()))
            .expect("record initial lifecycle");

        let advanced = runtime
            .advance_lifecycle_to(initial.observed_at_unix + RESERVE_LIFECYCLE_DAY_SECS - 1)
            .expect("advance lifecycle");

        assert!(advanced.is_empty());
        let summary = runtime
            .provider_summary([0x2D; 32])
            .expect("provider summary");
        assert_eq!(summary.lifecycle.days_past_due, 29);
        assert_eq!(runtime.events_since(None, 10).len(), 1);
    }

    fn appeal_request(appeal_byte: u8, provider_byte: u8) -> ReserveAppealRequest {
        ReserveAppealRequest {
            appeal_id: [appeal_byte; 32],
            provider_id: [provider_byte; 32],
            provider_account: format!("provider-{provider_byte}").into_bytes(),
            requested_stage: Some(ReserveLifecycleStage::Grace),
            reason: format!("appeal reason {appeal_byte}"),
            evidence_digest_hex: Some(hex::encode([0xE0 | appeal_byte; 32])),
            idempotency_key: format!("appeal-{appeal_byte}"),
            observed_at_unix: 2_000 + u64::from(appeal_byte),
        }
    }

    fn appeal_decision(appeal_byte: u8, status: ReserveAppealStatus) -> ReserveAppealDecision {
        ReserveAppealDecision {
            appeal_id: [appeal_byte; 32],
            status,
            decision_account: b"reserve-policy-authority".to_vec(),
            rationale: format!("decision for {appeal_byte}"),
            decided_at_unix: 3_000 + u64::from(appeal_byte),
        }
    }

    fn lifecycle_policy_update(policy_byte: u8) -> ReserveLifecyclePolicyUpdate {
        ReserveLifecyclePolicyUpdate {
            policy_id: [policy_byte; 32],
            authority_account: b"reserve-policy-authority".to_vec(),
            grace_period_days: 7,
            default_after_days: 30,
            effective_at_unix: 4_000 + u64::from(policy_byte),
            reason: format!("policy update {policy_byte}"),
            idempotency_key: format!("policy-{policy_byte}"),
            observed_at_unix: 5_000 + u64::from(policy_byte),
        }
    }

    #[test]
    fn runtime_records_appeals_and_decisions() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let request = appeal_request(0x10, 0x44);

        let first = runtime
            .record_appeal(request.clone())
            .expect("record appeal");
        assert!(!first.duplicate);
        assert_eq!(first.record.sequence, 0);
        assert_eq!(first.record.status, ReserveAppealStatus::Open);
        assert_eq!(
            first.record.requested_stage,
            Some(ReserveLifecycleStage::Grace)
        );

        let duplicate = runtime
            .record_appeal(request)
            .expect("replay appeal idempotently");
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.record, first.record);

        let decided = runtime
            .record_appeal_decision(appeal_decision(0x10, ReserveAppealStatus::Accepted))
            .expect("decide appeal");
        assert!(!decided.duplicate);
        assert_eq!(decided.lifecycle_event, None);
        assert_eq!(decided.record.status, ReserveAppealStatus::Accepted);
        assert_eq!(
            decided.record.decision_account,
            Some(b"reserve-policy-authority".to_vec())
        );
        assert_eq!(runtime.appeal([0x10; 32]), Some(decided.record.clone()));

        let snapshot = runtime.appeal_snapshot(9_000);
        assert_eq!(snapshot.generated_at_unix, 9_000);
        assert_eq!(snapshot.next_sequence, 1);
        assert_eq!(snapshot.appeals, vec![decided.record]);
    }

    #[test]
    fn runtime_applies_accepted_appeal_decision_to_current_lifecycle_state() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_update(update(0x47, 31, XorAmount::zero()))
            .expect("record defaulted provider");
        runtime
            .record_appeal(appeal_request(0x15, 0x47))
            .expect("record appeal");

        let decided = runtime
            .record_appeal_decision(appeal_decision(0x15, ReserveAppealStatus::Accepted))
            .expect("accept appeal");

        assert!(!decided.duplicate);
        let event = decided.lifecycle_event.expect("lifecycle override event");
        assert_eq!(event.sequence, 1);
        assert_eq!(event.previous_stage, Some(ReserveLifecycleStage::Default));
        assert_eq!(event.current_stage, ReserveLifecycleStage::Grace);
        assert_eq!(event.applied_appeal_id, Some([0x15; 32]));
        assert!(!event.lifecycle.disable_adverts);
        assert!(event.lifecycle.restrict_new_manifests);
        assert!(!event.lifecycle.requires_governance_notification);

        let summary = runtime
            .provider_summary([0x47; 32])
            .expect("provider summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Grace);
        assert_eq!(summary.applied_appeal_id, Some([0x15; 32]));
        assert!(!summary.lifecycle.disable_adverts);

        let credit_line = runtime
            .provider_credit_line([0x47; 32])
            .expect("credit-line state");
        assert_eq!(credit_line.stage, ReserveLifecycleStage::Grace);
        assert_eq!(credit_line.applied_appeal_id, Some([0x15; 32]));
        assert_eq!(credit_line.lifecycle_event_sequence, 1);

        let replay = runtime
            .record_appeal_decision(appeal_decision(0x15, ReserveAppealStatus::Accepted))
            .expect("replay appeal decision");
        assert!(replay.duplicate);
        assert_eq!(replay.lifecycle_event, None);
        assert_eq!(runtime.events_since(None, 10).len(), 2);
    }

    #[test]
    fn runtime_applies_effective_accepted_appeal_to_later_updates() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_appeal(appeal_request(0x16, 0x48))
            .expect("record appeal");
        runtime
            .record_appeal_decision(appeal_decision(0x16, ReserveAppealStatus::Accepted))
            .expect("accept appeal without current summary");

        let mut update = update(0x48, 31, XorAmount::zero());
        update.observed_at_unix = 4_000;
        let event = runtime
            .record_update(update)
            .expect("record appeal-driven lifecycle update");

        assert_eq!(event.current_stage, ReserveLifecycleStage::Grace);
        assert_eq!(event.applied_appeal_id, Some([0x16; 32]));
        assert!(!event.lifecycle.disable_adverts);
        assert!(!event.lifecycle.requires_governance_notification);
    }

    #[test]
    fn runtime_ignores_rejected_appeal_decisions_for_lifecycle_override() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_update(update(0x49, 31, XorAmount::zero()))
            .expect("record defaulted provider");
        runtime
            .record_appeal(appeal_request(0x17, 0x49))
            .expect("record appeal");

        let decided = runtime
            .record_appeal_decision(appeal_decision(0x17, ReserveAppealStatus::Rejected))
            .expect("reject appeal");
        assert_eq!(decided.lifecycle_event, None);
        assert_eq!(
            runtime
                .provider_summary([0x49; 32])
                .expect("provider summary")
                .lifecycle
                .stage,
            ReserveLifecycleStage::Default
        );
    }

    #[test]
    fn runtime_rejects_invalid_appeal_transitions() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let missing = runtime
            .record_appeal_decision(appeal_decision(0x12, ReserveAppealStatus::Rejected))
            .expect_err("missing appeal decision should fail");
        assert!(matches!(
            missing,
            ReserveAppealRuntimeError::AppealNotFound { .. }
        ));

        runtime
            .record_appeal(appeal_request(0x13, 0x45))
            .expect("record appeal");
        runtime
            .record_appeal_decision(appeal_decision(0x13, ReserveAppealStatus::Rejected))
            .expect("reject appeal");
        let second_decision = runtime
            .record_appeal_decision(appeal_decision(0x13, ReserveAppealStatus::Accepted))
            .expect_err("terminal decision cannot be replaced");
        assert!(matches!(
            second_decision,
            ReserveAppealRuntimeError::InvalidAppealTransition { .. }
        ));

        let empty = ReserveAppealRequest {
            reason: " ".to_owned(),
            ..appeal_request(0x14, 0x46)
        };
        let err = runtime
            .record_appeal(empty)
            .expect_err("empty appeal reason should fail");
        assert_eq!(
            err,
            ReserveAppealRuntimeError::EmptyText { field: "reason" }
        );
    }

    #[test]
    fn runtime_records_lifecycle_policy_updates() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let update = lifecycle_policy_update(0x20);

        let first = runtime
            .record_lifecycle_policy_update(update.clone())
            .expect("record lifecycle policy");
        assert!(!first.duplicate);
        assert_eq!(first.record.sequence, 0);
        assert_eq!(first.record.grace_period_days, 7);
        assert_eq!(first.record.default_after_days, 30);

        let duplicate = runtime
            .record_lifecycle_policy_update(update)
            .expect("replay policy idempotently");
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.record, first.record);

        let second = runtime
            .record_lifecycle_policy_update(ReserveLifecyclePolicyUpdate {
                grace_period_days: 10,
                default_after_days: 45,
                ..lifecycle_policy_update(0x21)
            })
            .expect("record second policy");
        assert_eq!(
            runtime.latest_lifecycle_policy(),
            Some(second.record.clone())
        );

        let snapshot = runtime.lifecycle_policy_snapshot(10_000);
        assert_eq!(snapshot.generated_at_unix, 10_000);
        assert_eq!(snapshot.next_sequence, 2);
        assert_eq!(snapshot.latest, Some(second.record));
        assert_eq!(snapshot.policies.len(), 2);
    }

    #[test]
    fn runtime_rejects_invalid_lifecycle_policy_windows() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let err = runtime
            .record_lifecycle_policy_update(ReserveLifecyclePolicyUpdate {
                grace_period_days: 30,
                default_after_days: 30,
                ..lifecycle_policy_update(0x22)
            })
            .expect_err("equal grace/default windows should fail");
        assert_eq!(
            err,
            ReserveAppealRuntimeError::InvalidLifecyclePolicyWindow {
                grace_period_days: 30,
                default_after_days: 30
            }
        );
    }

    fn movement_request(
        movement_byte: u8,
        provider_byte: u8,
        kind: ReserveMovementKind,
        amount: XorAmount,
    ) -> ReserveMovementRequest {
        ReserveMovementRequest {
            movement_id: [movement_byte; 32],
            provider_id: [provider_byte; 32],
            provider_account: format!("provider-{provider_byte}").into_bytes(),
            reserve_account: b"reserve-account".to_vec(),
            asset_definition_id: b"xor#sora".to_vec(),
            kind,
            amount,
            idempotency_key: format!("movement-{movement_byte}"),
            observed_at_unix: u64::from(movement_byte),
        }
    }

    fn custody_update(
        movement_byte: u8,
        status: ReserveMovementCustodyStatus,
        tx_byte: u8,
    ) -> ReserveMovementCustodyUpdate {
        ReserveMovementCustodyUpdate {
            movement_id: [movement_byte; 32],
            status,
            tx_hash_hex: hex::encode([tx_byte; 32]),
            observed_at_unix: 1_000 + u64::from(tx_byte),
        }
    }

    #[test]
    fn runtime_records_top_up_withdrawal_and_balances() {
        let mut runtime = ReserveLifecycleRuntime::default();

        let top_up = runtime
            .record_movement(movement_request(
                0x01,
                0x31,
                ReserveMovementKind::TopUp,
                XorAmount::from_micro(100),
            ))
            .expect("record top-up");
        assert!(!top_up.duplicate);
        assert_eq!(top_up.record.sequence, 0);
        assert_eq!(top_up.record.balance_after.as_micro(), 100);
        assert!(top_up.record.confirmed_balance_after.is_zero());

        let withdrawal = runtime
            .record_movement(movement_request(
                0x02,
                0x31,
                ReserveMovementKind::Withdrawal,
                XorAmount::from_micro(40),
            ))
            .expect("record withdrawal");
        assert_eq!(withdrawal.record.sequence, 1);
        assert_eq!(withdrawal.record.balance_after.as_micro(), 60);
        assert!(withdrawal.record.confirmed_balance_after.is_zero());

        let balance = runtime
            .provider_balance([0x31; 32])
            .expect("provider balance");
        assert_eq!(balance.balance.as_micro(), 60);
        assert!(balance.confirmed_balance.is_zero());
        assert_eq!(runtime.latest_movement_sequence(), Some(1));
        assert_eq!(
            runtime.movements_since(Some(0), 10),
            vec![withdrawal.record]
        );

        let snapshot = runtime.movement_snapshot(777);
        assert_eq!(snapshot.generated_at_unix, 777);
        assert_eq!(snapshot.next_sequence, 2);
        assert_eq!(snapshot.provider_balances.len(), 1);
        assert_eq!(snapshot.movements.len(), 2);
    }

    #[test]
    fn runtime_replays_duplicate_movement_without_mutating_balance() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let request = movement_request(
            0x03,
            0x32,
            ReserveMovementKind::TopUp,
            XorAmount::from_micro(55),
        );

        let first = runtime
            .record_movement(request.clone())
            .expect("record first movement");
        let duplicate = runtime
            .record_movement(request)
            .expect("record duplicate movement");

        assert!(!first.duplicate);
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.record, first.record);
        assert_eq!(runtime.movement_snapshot(1).movements.len(), 1);
        assert_eq!(
            runtime
                .provider_balance([0x32; 32])
                .expect("provider balance")
                .balance
                .as_micro(),
            55
        );
    }

    #[test]
    fn runtime_records_custody_submission_and_confirmation() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_movement(movement_request(
                0x06,
                0x34,
                ReserveMovementKind::TopUp,
                XorAmount::from_micro(75),
            ))
            .expect("record movement");

        let submitted = runtime
            .record_movement_custody_update(custody_update(
                0x06,
                ReserveMovementCustodyStatus::Submitted,
                0xAA,
            ))
            .expect("record submitted status");
        assert_eq!(
            submitted.custody_status,
            ReserveMovementCustodyStatus::Submitted
        );
        assert!(submitted.confirmed_balance_after.is_zero());
        assert_eq!(submitted.custody_tx_hash_hex, Some(hex::encode([0xAA; 32])));
        assert_eq!(submitted.custody_updated_at_unix, Some(1_170));
        assert!(
            runtime
                .provider_balance([0x34; 32])
                .expect("provider balance")
                .confirmed_balance
                .is_zero()
        );

        let confirmed = runtime
            .record_movement_custody_update(custody_update(
                0x06,
                ReserveMovementCustodyStatus::Confirmed,
                0xAA,
            ))
            .expect("record confirmed status");
        assert_eq!(
            confirmed.custody_status,
            ReserveMovementCustodyStatus::Confirmed
        );
        assert_eq!(confirmed.confirmed_balance_after.as_micro(), 75);
        assert_eq!(
            runtime
                .provider_balance([0x34; 32])
                .expect("provider balance")
                .confirmed_balance
                .as_micro(),
            75
        );
        assert_eq!(runtime.movement([0x06; 32]), Some(confirmed.clone()));
        assert_eq!(runtime.movement_snapshot(2).movements[0], confirmed);
    }

    #[test]
    fn runtime_rejects_confirmed_withdrawal_without_confirmed_reserve_funds() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_movement(movement_request(
                0x08,
                0x35,
                ReserveMovementKind::TopUp,
                XorAmount::from_micro(100),
            ))
            .expect("record unconfirmed top-up");
        runtime
            .record_movement(movement_request(
                0x09,
                0x35,
                ReserveMovementKind::Withdrawal,
                XorAmount::from_micro(40),
            ))
            .expect("record local withdrawal intent");

        let err = runtime
            .record_movement_custody_update(custody_update(
                0x09,
                ReserveMovementCustodyStatus::Confirmed,
                0xAC,
            ))
            .expect_err("confirmed withdrawal must require confirmed reserve funds");
        assert!(matches!(
            err,
            ReserveMovementRuntimeError::InsufficientBalance { .. }
        ));
        let withdrawal = runtime.movement([0x09; 32]).expect("withdrawal movement");
        assert_eq!(
            withdrawal.custody_status,
            ReserveMovementCustodyStatus::IntentRecorded
        );
        assert!(withdrawal.confirmed_balance_after.is_zero());

        let confirmed_top_up = runtime
            .record_movement_custody_update(custody_update(
                0x08,
                ReserveMovementCustodyStatus::Confirmed,
                0xAB,
            ))
            .expect("confirm top-up");
        assert_eq!(confirmed_top_up.confirmed_balance_after.as_micro(), 100);
        let confirmed_withdrawal = runtime
            .record_movement_custody_update(custody_update(
                0x09,
                ReserveMovementCustodyStatus::Confirmed,
                0xAC,
            ))
            .expect("confirm withdrawal after top-up finality");
        assert_eq!(confirmed_withdrawal.confirmed_balance_after.as_micro(), 60);
        let balance = runtime
            .provider_balance([0x35; 32])
            .expect("provider balance");
        assert_eq!(balance.balance.as_micro(), 60);
        assert_eq!(balance.confirmed_balance.as_micro(), 60);
    }

    #[test]
    fn runtime_reconciles_rejected_movement_balances() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_movement(movement_request(
                0x60,
                0x36,
                ReserveMovementKind::TopUp,
                XorAmount::from_micro(100),
            ))
            .expect("record top-up");

        let rejected_top_up = runtime
            .record_movement_custody_update(custody_update(
                0x60,
                ReserveMovementCustodyStatus::Rejected,
                0xE0,
            ))
            .expect("reject top-up");
        assert_eq!(
            rejected_top_up.custody_status,
            ReserveMovementCustodyStatus::Rejected
        );
        assert_eq!(rejected_top_up.balance_after.as_micro(), 0);
        assert!(rejected_top_up.confirmed_balance_after.is_zero());
        assert!(
            runtime.provider_balance([0x36; 32]).is_none(),
            "rejected top-up must not remain spendable"
        );

        runtime
            .record_movement(movement_request(
                0x61,
                0x36,
                ReserveMovementKind::TopUp,
                XorAmount::from_micro(100),
            ))
            .expect("record replacement top-up");
        runtime
            .record_movement(movement_request(
                0x62,
                0x36,
                ReserveMovementKind::Withdrawal,
                XorAmount::from_micro(40),
            ))
            .expect("record withdrawal");

        let rejected_withdrawal = runtime
            .record_movement_custody_update(custody_update(
                0x62,
                ReserveMovementCustodyStatus::Rejected,
                0xE1,
            ))
            .expect("reject withdrawal");
        assert_eq!(
            rejected_withdrawal.custody_status,
            ReserveMovementCustodyStatus::Rejected
        );
        assert_eq!(rejected_withdrawal.balance_after.as_micro(), 100);
        assert!(rejected_withdrawal.confirmed_balance_after.is_zero());
        assert_eq!(
            runtime
                .provider_balance([0x36; 32])
                .expect("provider balance")
                .balance
                .as_micro(),
            100
        );
    }

    #[test]
    fn runtime_rejects_reconciliation_that_would_underflow_later_movements() {
        let mut runtime = ReserveLifecycleRuntime::default();
        runtime
            .record_movement(movement_request(
                0x63,
                0x37,
                ReserveMovementKind::TopUp,
                XorAmount::from_micro(100),
            ))
            .expect("record top-up");
        runtime
            .record_movement(movement_request(
                0x64,
                0x37,
                ReserveMovementKind::Withdrawal,
                XorAmount::from_micro(40),
            ))
            .expect("record withdrawal");

        let err = runtime
            .record_movement_custody_update(custody_update(
                0x63,
                ReserveMovementCustodyStatus::Rejected,
                0xE2,
            ))
            .expect_err("top-up rejection would invalidate later withdrawal");
        assert!(matches!(
            err,
            ReserveMovementRuntimeError::InsufficientBalance { .. }
        ));
        assert_eq!(
            runtime
                .provider_balance([0x37; 32])
                .expect("provider balance")
                .balance
                .as_micro(),
            60
        );
        assert_eq!(
            runtime
                .movement([0x63; 32])
                .expect("top-up movement")
                .custody_status,
            ReserveMovementCustodyStatus::IntentRecorded
        );
    }

    #[test]
    fn runtime_rejects_invalid_custody_updates() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let missing = runtime
            .record_movement_custody_update(custody_update(
                0x07,
                ReserveMovementCustodyStatus::Submitted,
                0xBB,
            ))
            .expect_err("missing movement should fail");
        assert!(matches!(
            missing,
            ReserveMovementRuntimeError::MovementNotFound { .. }
        ));

        runtime
            .record_movement(movement_request(
                0x08,
                0x35,
                ReserveMovementKind::TopUp,
                XorAmount::from_micro(90),
            ))
            .expect("record movement");
        runtime
            .record_movement_custody_update(custody_update(
                0x08,
                ReserveMovementCustodyStatus::Submitted,
                0xCC,
            ))
            .expect("record submitted status");
        let mismatched_hash = runtime
            .record_movement_custody_update(custody_update(
                0x08,
                ReserveMovementCustodyStatus::Confirmed,
                0xCD,
            ))
            .expect_err("hash replacement should fail");
        assert!(matches!(
            mismatched_hash,
            ReserveMovementRuntimeError::CustodyTransactionHashMismatch { .. }
        ));

        let rejected = runtime
            .record_movement_custody_update(custody_update(
                0x08,
                ReserveMovementCustodyStatus::Rejected,
                0xCC,
            ))
            .expect("record terminal rejection");
        let idempotent = runtime
            .record_movement_custody_update(custody_update(
                0x08,
                ReserveMovementCustodyStatus::Rejected,
                0xCC,
            ))
            .expect("same terminal status is idempotent");
        assert_eq!(idempotent, rejected);
        let terminal_change = runtime
            .record_movement_custody_update(custody_update(
                0x08,
                ReserveMovementCustodyStatus::Confirmed,
                0xCC,
            ))
            .expect_err("terminal status should not change");
        assert!(matches!(
            terminal_change,
            ReserveMovementRuntimeError::InvalidCustodyTransition { .. }
        ));
    }

    #[test]
    fn runtime_rejects_invalid_movements() {
        let mut runtime = ReserveLifecycleRuntime::default();
        let zero = runtime
            .record_movement(movement_request(
                0x04,
                0x33,
                ReserveMovementKind::TopUp,
                XorAmount::zero(),
            ))
            .expect_err("zero movement should fail");
        assert_eq!(zero, ReserveMovementRuntimeError::ZeroAmount);

        let underflow = runtime
            .record_movement(movement_request(
                0x05,
                0x33,
                ReserveMovementKind::Withdrawal,
                XorAmount::from_micro(1),
            ))
            .expect_err("withdrawal without balance should fail");
        assert!(matches!(
            underflow,
            ReserveMovementRuntimeError::InsufficientBalance { .. }
        ));
    }
}
