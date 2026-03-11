//! SoraNet relay payout accounting, dispute resolution, and dashboard tooling (SNNet-7a).
//!
//! This module builds on the reward engine exposed in `crate::incentives` and provides the
//! treasury-facing workflow required by SNNet-7a: it converts epoch metrics into XOR transfers,
//! records ledger state per relay/epoch, manages dispute lifecycles, and emits dashboard-friendly
//! aggregates for relay operators and oversight tooling.

use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use hex::encode as hex_encode;
use iroha_core::soranet_incentives::RelayPayoutLedger;
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{
        InstructionBox,
        transfer::{Transfer, TransferBox},
    },
    metadata::Metadata,
    name::Name,
    soranet::{
        RelayId,
        incentives::{
            RelayBondLedgerEntryV1, RelayEpochMetricsV1, RelayRewardDisputeStatusV1,
            RelayRewardDisputeV1, RelayRewardInstructionV1,
        },
    },
};
use iroha_primitives::{json::Json, numeric::Numeric};
use thiserror::Error;

use crate::incentives::RelayRewardEngine;

/// Identifier assigned to a dispute entry.
pub type DisputeId = u64;

/// End-to-end payout workflow used by the treasury daemon.
#[derive(Debug, Clone)]
pub struct RelayPayoutService {
    reward_engine: RelayRewardEngine,
    payout_ledger: RelayPayoutLedger,
    ledger: RewardLedger,
    disputes: RewardDisputeRegistry,
}

/// Batch input describing a relay payout to be processed.
#[derive(Debug, Clone)]
pub struct PayoutInput<'a> {
    /// Metrics captured for the relay and epoch.
    pub metrics: &'a RelayEpochMetricsV1,
    /// Bond ledger entry for the relay.
    pub bond_entry: &'a RelayBondLedgerEntryV1,
    /// Beneficiary account that receives the payout.
    pub beneficiary: AccountId,
    /// Additional metadata propagated to the reward instruction.
    pub metadata: Metadata,
}

impl RelayPayoutService {
    /// Construct a new payout service from a reward engine and treasury ledger helper.
    #[must_use]
    pub fn new(reward_engine: RelayRewardEngine, payout_ledger: RelayPayoutLedger) -> Self {
        Self {
            reward_engine,
            payout_ledger,
            ledger: RewardLedger::default(),
            disputes: RewardDisputeRegistry::default(),
        }
    }

    /// Access the underlying reward engine.
    #[must_use]
    pub fn reward_engine(&self) -> &RelayRewardEngine {
        &self.reward_engine
    }

    /// Access the ledger helper responsible for converting rewards into transfers.
    #[must_use]
    pub fn payout_ledger(&self) -> &RelayPayoutLedger {
        &self.payout_ledger
    }

    /// Returns the treasury account debited when materialising payouts.
    #[must_use]
    pub fn treasury_account(&self) -> &AccountId {
        self.payout_ledger.treasury_account()
    }

    /// Fetch a snapshot of the reward ledger for the specified relay.
    pub fn ledger_snapshot(
        &self,
        relay_id: RelayId,
    ) -> Result<RewardLedgerSnapshot, RewardLedgerError> {
        self.ledger.snapshot(relay_id)
    }

    /// Reconcile an exported XOR ledger snapshot against the recorded payout ledger.
    #[must_use]
    pub fn reconcile_ledger(&self, exports: &[LedgerTransferRecord]) -> LedgerReconciliationReport {
        let mut expected: BTreeMap<TransferKey, LedgerTransferRecord> = BTreeMap::new();
        let mut expected_amount_nanos = 0u128;
        let mut total_expected_transfers = 0usize;

        for (relay_id, entry) in self.ledger.iter() {
            for (&epoch, record) in &entry.payouts {
                if let Some(summary) = record.transfer_summary(epoch)
                    && summary.amount != Numeric::zero()
                {
                    let record = LedgerTransferRecord::from_summary(
                        *relay_id,
                        epoch,
                        TransferKind::Payout,
                        None,
                        &summary,
                    );
                    let key = record.key();
                    expected_amount_nanos = expected_amount_nanos
                        .saturating_add(numeric_to_nanos(&record.amount).unwrap_or(0));
                    total_expected_transfers += 1;
                    expected.insert(key, record);
                }

                for adjustment in &record.adjustments {
                    if adjustment.amount == Numeric::zero() {
                        continue;
                    }
                    let base_instruction = &record.instruction;
                    let treasury = self.payout_ledger.treasury_account();
                    let (kind, source_asset, destination) = match adjustment.kind {
                        AdjustmentKind::Credit => (
                            TransferKind::Credit,
                            AssetId::new(
                                base_instruction.payout_asset_id.clone(),
                                treasury.clone(),
                            ),
                            base_instruction.beneficiary.clone(),
                        ),
                        AdjustmentKind::Debit => (
                            TransferKind::Debit,
                            AssetId::new(
                                base_instruction.payout_asset_id.clone(),
                                base_instruction.beneficiary.clone(),
                            ),
                            treasury.clone(),
                        ),
                    };
                    let summary = TransferSummary {
                        epoch,
                        source_asset,
                        destination,
                        amount: adjustment.amount.clone(),
                    };
                    let dispute_id = Some(adjustment.dispute_id);
                    let record = LedgerTransferRecord::from_summary(
                        *relay_id, epoch, kind, dispute_id, &summary,
                    );
                    let key = record.key();
                    expected_amount_nanos = expected_amount_nanos
                        .saturating_add(numeric_to_nanos(&record.amount).unwrap_or(0));
                    total_expected_transfers += 1;
                    expected.insert(key, record);
                }
            }
        }

        let mut exported_amount_nanos = 0u128;
        let mut matched_transfers = 0usize;
        let mut mismatched_transfers = Vec::new();
        let mut unexpected_transfers = Vec::new();

        for export in exports {
            exported_amount_nanos =
                exported_amount_nanos.saturating_add(numeric_to_nanos(&export.amount).unwrap_or(0));
            let key = TransferKey::from_record(export);
            match expected.remove(&key) {
                Some(expected_record) => {
                    let reasons = mismatch_reasons(&expected_record, export);
                    if reasons.is_empty() {
                        matched_transfers += 1;
                    } else {
                        mismatched_transfers.push(LedgerTransferMismatch {
                            expected: expected_record,
                            actual: export.clone(),
                            reasons,
                        });
                    }
                }
                None => unexpected_transfers.push(export.clone()),
            }
        }

        let missing_transfers = expected
            .into_values()
            .map(|record| ExpectedLedgerTransfer { record })
            .collect();

        LedgerReconciliationReport {
            total_expected_transfers,
            matched_transfers,
            expected_amount_nanos,
            exported_amount_nanos,
            missing_transfers,
            unexpected_transfers,
            mismatched_transfers,
        }
    }

    /// Evaluate metrics/bonds and emit a reward instruction alongside a transfer.
    pub fn process_epoch(
        &mut self,
        metrics: &RelayEpochMetricsV1,
        bond_entry: &RelayBondLedgerEntryV1,
        beneficiary: AccountId,
        metadata: Metadata,
    ) -> Result<PayoutOutcome, PayoutServiceError> {
        self.process_entry(metrics, bond_entry, beneficiary, metadata)
    }

    /// Evaluate metrics/bonds for a batch of relays and emit payout instructions.
    pub fn process_batch<'a, I>(
        &mut self,
        inputs: I,
    ) -> Result<Vec<PayoutOutcome>, PayoutServiceError>
    where
        I: IntoIterator<Item = PayoutInput<'a>>,
    {
        let mut outcomes = Vec::new();
        for input in inputs {
            outcomes.push(self.process_entry(
                input.metrics,
                input.bond_entry,
                input.beneficiary,
                input.metadata,
            )?);
        }
        Ok(outcomes)
    }

    fn process_entry(
        &mut self,
        metrics: &RelayEpochMetricsV1,
        bond_entry: &RelayBondLedgerEntryV1,
        beneficiary: AccountId,
        metadata: Metadata,
    ) -> Result<PayoutOutcome, PayoutServiceError> {
        let instruction =
            self.reward_engine
                .compute_reward(metrics, bond_entry, beneficiary, metadata);
        let transfer = instruction.to_transfer_instruction(self.treasury_account());
        let snapshot = self
            .ledger
            .record_reward(instruction.clone(), transfer.clone())
            .map_err(PayoutServiceError::Ledger)?;

        Ok(PayoutOutcome {
            instruction,
            transfer,
            ledger_snapshot: snapshot,
        })
    }

    /// Record an externally supplied payout instruction.
    pub fn record_reward(
        &mut self,
        instruction: RelayRewardInstructionV1,
    ) -> Result<(), PayoutServiceError> {
        let transfer = instruction.to_transfer_instruction(self.treasury_account());
        self.ledger
            .record_reward(instruction, transfer)
            .map(|_| ())
            .map_err(PayoutServiceError::Ledger)
    }

    /// File a dispute against an epoch payout.
    #[allow(clippy::too_many_arguments)]
    pub fn file_dispute(
        &mut self,
        relay_id: RelayId,
        epoch: u32,
        submitted_by: AccountId,
        requested_amount: Numeric,
        reason: impl Into<String>,
        filed_at_unix: u64,
        requested_adjustment: Option<AdjustmentRequest>,
    ) -> Result<RewardDispute, PayoutServiceError> {
        let reason = reason.into();
        let payout_record = self.ledger.payout_record(relay_id, epoch).ok_or(
            PayoutServiceError::MissingPayoutRecord {
                relay: relay_id,
                epoch,
            },
        )?;

        let norito_record = self.payout_ledger.open_dispute(
            payout_record.instruction.clone(),
            requested_amount,
            submitted_by,
            filed_at_unix,
            reason.clone(),
        );

        let filed = self.disputes.file(
            relay_id,
            epoch,
            reason,
            filed_at_unix,
            requested_adjustment,
            norito_record,
        );
        record_dispute_metric("filed");

        Ok(filed)
    }

    /// Reject a dispute without adjusting the ledger.
    pub fn reject_dispute(
        &mut self,
        dispute_id: DisputeId,
        rejected_at_unix: u64,
        notes: impl Into<String>,
    ) -> Result<RewardDispute, PayoutServiceError> {
        let rejected = self
            .disputes
            .reject(dispute_id, rejected_at_unix, notes)
            .map_err(PayoutServiceError::Registry)?;
        record_dispute_metric("rejected");
        Ok(rejected)
    }

    /// Resolve a dispute, optionally applying credits or claw-backs.
    pub fn resolve_dispute(
        &mut self,
        dispute_id: DisputeId,
        resolution: DisputeResolution,
        resolved_at_unix: u64,
    ) -> Result<DisputeOutcome, PayoutServiceError> {
        let dispute = self
            .disputes
            .resolve(dispute_id, resolution, resolved_at_unix)
            .map_err(PayoutServiceError::Registry)?;

        let (snapshot, transfer, resolution_label, adjustment) = match &dispute.status {
            DisputeStatus::Open => unreachable!("resolve() never returns an open dispute"),
            DisputeStatus::Rejected { .. } => (
                self.ledger
                    .snapshot(dispute.relay_id)
                    .map_err(PayoutServiceError::Ledger)?,
                None,
                None,
                None,
            ),
            DisputeStatus::Resolved { outcome, .. } => match outcome.kind {
                ResolutionKind::NoChange => (
                    self.ledger
                        .snapshot(dispute.relay_id)
                        .map_err(PayoutServiceError::Ledger)?,
                    None,
                    Some("resolved_no_change"),
                    None,
                ),
                ResolutionKind::Credit => {
                    let amount = outcome
                        .amount
                        .clone()
                        .expect("credit resolution always carries an amount");
                    let (beneficiary, asset_def) = self
                        .ledger
                        .payout_owner(dispute.relay_id, dispute.epoch)
                        .ok_or(PayoutServiceError::MissingPayoutRecord {
                            relay: dispute.relay_id,
                            epoch: dispute.epoch,
                        })?;
                    let snapshot = self
                        .ledger
                        .apply_credit(
                            dispute.relay_id,
                            dispute.epoch,
                            dispute.id,
                            amount.clone(),
                            outcome.notes.clone(),
                            resolved_at_unix,
                        )
                        .map_err(PayoutServiceError::Ledger)?;

                    let asset = AssetId::new(asset_def.clone(), self.treasury_account().clone());
                    let transfer: InstructionBox =
                        Transfer::asset_numeric(asset, amount.clone(), beneficiary.clone()).into();

                    (
                        snapshot,
                        Some(transfer),
                        Some("resolved_credit"),
                        Some(("credit", amount)),
                    )
                }
                ResolutionKind::Debit => {
                    let amount = outcome
                        .amount
                        .clone()
                        .expect("debit resolution always carries an amount");
                    let (beneficiary, asset_def) = self
                        .ledger
                        .payout_owner(dispute.relay_id, dispute.epoch)
                        .ok_or(PayoutServiceError::MissingPayoutRecord {
                            relay: dispute.relay_id,
                            epoch: dispute.epoch,
                        })?;
                    let snapshot = self
                        .ledger
                        .apply_debit(
                            dispute.relay_id,
                            dispute.epoch,
                            dispute.id,
                            amount.clone(),
                            outcome.notes.clone(),
                            resolved_at_unix,
                        )
                        .map_err(PayoutServiceError::Ledger)?;

                    let asset = AssetId::new(asset_def, beneficiary.clone());
                    let transfer: InstructionBox = Transfer::asset_numeric(
                        asset,
                        amount.clone(),
                        self.treasury_account().clone(),
                    )
                    .into();

                    (
                        snapshot,
                        Some(transfer),
                        Some("resolved_debit"),
                        Some(("debit", amount)),
                    )
                }
            },
        };

        if let Some(label) = resolution_label {
            record_dispute_metric(label);
        }
        if let Some((kind, amount)) = adjustment {
            record_adjustment_metric(dispute.relay_id, &amount, kind);
        }

        Ok(DisputeOutcome {
            dispute,
            ledger_snapshot: snapshot,
            transfer,
        })
    }

    /// Access the internal reward ledger.
    #[must_use]
    pub fn ledger(&self) -> &RewardLedger {
        &self.ledger
    }

    /// Access the dispute registry.
    #[must_use]
    pub fn disputes(&self) -> &RewardDisputeRegistry {
        &self.disputes
    }

    /// Build an earnings dashboard across all tracked relays.
    pub fn earnings_dashboard(&self) -> Result<EarningsDashboard, RewardLedgerError> {
        let mut rows = Vec::new();
        let mut open = 0usize;
        for (relay_id, entry) in self.ledger.iter() {
            let snapshot = entry.snapshot()?;
            let open_disputes = self.disputes.open_count_for_relay(relay_id);
            open += open_disputes;
            let last_transfer = entry.latest_transfer_summary();
            rows.push(EarningsRow::from_snapshot(
                snapshot,
                open_disputes,
                last_transfer,
            ));
        }

        Ok(EarningsDashboard {
            total_relays: rows.len(),
            total_open_disputes: open,
            rows,
        })
    }
}

/// Outcome of a standard payout evaluation.
#[derive(Debug, Clone)]
pub struct PayoutOutcome {
    /// Reward instruction emitted by the incentive calculator.
    pub instruction: RelayRewardInstructionV1,
    /// Transfer instruction debiting the treasury account.
    pub transfer: InstructionBox,
    /// Ledger snapshot after recording the payout.
    pub ledger_snapshot: RewardLedgerSnapshot,
}

/// Result of resolving a dispute.
#[derive(Debug, Clone)]
pub struct DisputeOutcome {
    /// Updated dispute entry.
    pub dispute: RewardDispute,
    /// Ledger snapshot after applying adjustments.
    pub ledger_snapshot: RewardLedgerSnapshot,
    /// Optional transfer describing credits or claw-backs.
    pub transfer: Option<InstructionBox>,
}

/// Summary of a treasury transfer associated with a payout or dispute resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSummary {
    /// Epoch associated with the payout record.
    pub epoch: u32,
    /// Asset identifier debited for the transfer (includes source account).
    pub source_asset: AssetId,
    /// Account receiving the transfer.
    pub destination: AccountId,
    /// Amount of XOR moved as part of the transfer.
    pub amount: Numeric,
}

/// Direction of a treasury transfer recorded in the incentive ledger.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(tag = "kind", content = "detail")]
#[norito(decode_from_slice)]
pub enum TransferKind {
    /// Standard epoch payout from treasury to relay.
    Payout,
    /// Credit adjustment issued after a dispute (treasury → relay).
    Credit,
    /// Debit adjustment clawing back funds (relay → treasury).
    Debit,
}

/// Record produced from a Sora XOR ledger export.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(decode_from_slice)]
pub struct LedgerTransferRecord {
    /// Relay fingerprint associated with the payout or adjustment.
    #[norito(with = "crate::treasury::relay_id_json")]
    pub relay_id: RelayId,
    /// Epoch for which the transfer applies.
    pub epoch: u32,
    /// Direction of the transfer.
    pub kind: TransferKind,
    /// Optional dispute identifier (present when adjustments are reconciled).
    #[norito(default)]
    pub dispute_id: Option<DisputeId>,
    /// Amount of XOR transferred.
    pub amount: Numeric,
    /// Source account/asset debited by the transfer.
    pub source_asset: AssetId,
    /// Destination account that received the transfer.
    pub destination: AccountId,
}

impl LedgerTransferRecord {
    fn from_summary(
        relay_id: RelayId,
        epoch: u32,
        kind: TransferKind,
        dispute_id: Option<DisputeId>,
        summary: &TransferSummary,
    ) -> Self {
        Self {
            relay_id,
            epoch,
            kind,
            dispute_id,
            amount: summary.amount.clone(),
            source_asset: summary.source_asset.clone(),
            destination: summary.destination.clone(),
        }
    }

    fn key(&self) -> TransferKey {
        TransferKey {
            relay_id: self.relay_id,
            epoch: self.epoch,
            kind: self.kind,
            dispute_id: self.dispute_id,
        }
    }
}

/// Result of reconciling treasury exports with the payout ledger.
#[derive(Debug, Clone)]
pub struct LedgerReconciliationReport {
    /// Total transfers expected by the payout ledger (payouts + adjustments).
    pub total_expected_transfers: usize,
    /// Number of transfers that matched between export and ledger.
    pub matched_transfers: usize,
    /// Sum of expected transfers in XOR nanos.
    pub expected_amount_nanos: u128,
    /// Sum of exported transfers in XOR nanos.
    pub exported_amount_nanos: u128,
    /// Transfers expected by the ledger but missing from the export.
    pub missing_transfers: Vec<ExpectedLedgerTransfer>,
    /// Transfers present in the export but unknown to the ledger.
    pub unexpected_transfers: Vec<LedgerTransferRecord>,
    /// Transfers whose metadata differed between ledger and export.
    pub mismatched_transfers: Vec<LedgerTransferMismatch>,
}

impl LedgerReconciliationReport {
    /// Returns `true` when the export matches the ledger without discrepancies.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.missing_transfers.is_empty()
            && self.unexpected_transfers.is_empty()
            && self.mismatched_transfers.is_empty()
    }
}

/// Expected transfer that was absent from the external ledger export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedLedgerTransfer {
    /// Transfer metadata recorded in the payout ledger.
    pub record: LedgerTransferRecord,
}

/// Transfer present in the export but mismatching the payout ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerTransferMismatch {
    /// Expected transfer metadata.
    pub expected: LedgerTransferRecord,
    /// Exported transfer metadata.
    pub actual: LedgerTransferRecord,
    /// Reasons describing which fields diverged.
    pub reasons: Vec<MismatchReason>,
}

/// Dimension along which a mismatch occurred.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(tag = "reason", content = "detail")]
#[norito(decode_from_slice)]
pub enum MismatchReason {
    /// Amount differed between expected and exported transfers.
    Amount,
    /// Source asset or account differed.
    SourceAsset,
    /// Destination account differed.
    Destination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TransferKey {
    relay_id: RelayId,
    epoch: u32,
    kind: TransferKind,
    dispute_id: Option<DisputeId>,
}

impl TransferKey {
    fn new(
        relay_id: RelayId,
        epoch: u32,
        kind: TransferKind,
        dispute_id: Option<DisputeId>,
    ) -> Self {
        Self {
            relay_id,
            epoch,
            kind,
            dispute_id,
        }
    }

    fn from_record(record: &LedgerTransferRecord) -> Self {
        Self::new(
            record.relay_id,
            record.epoch,
            record.kind,
            record.dispute_id,
        )
    }
}

/// Aggregated dashboard surfaced to operators and oversight tooling.
#[derive(Debug, Clone)]
pub struct EarningsDashboard {
    /// Number of relays tracked in the ledger.
    pub total_relays: usize,
    /// Total count of open disputes.
    pub total_open_disputes: usize,
    /// Per-relay rows.
    pub rows: Vec<EarningsRow>,
}

/// Per-relay dashboard entry summarising payouts and adjustments.
#[derive(Debug, Clone)]
pub struct EarningsRow {
    /// Relay identifier.
    pub relay_id: RelayId,
    /// Total XOR released via standard payouts.
    pub total_paid: Numeric,
    /// Additional XOR awarded through dispute credits.
    pub total_rebated: Numeric,
    /// XOR withheld due to claw-backs.
    pub total_withheld: Numeric,
    /// Net XOR earned by the relay (`paid + rebated - withheld`).
    pub net_paid: Numeric,
    /// Number of epochs recorded for the relay.
    pub epochs_recorded: usize,
    /// Most recent epoch in the ledger.
    pub last_epoch: Option<u32>,
    /// Reward score observed for the most recent payout.
    pub last_reward_score: Option<u64>,
    /// Number of open disputes for the relay.
    pub open_disputes: usize,
    /// Latest treasury transfer recorded for the relay (if available).
    pub last_transfer: Option<TransferSummary>,
}

impl EarningsRow {
    fn from_snapshot(
        snapshot: RewardLedgerSnapshot,
        open_disputes: usize,
        last_transfer: Option<TransferSummary>,
    ) -> Self {
        Self {
            relay_id: snapshot.relay_id,
            total_paid: snapshot.total_paid,
            total_rebated: snapshot.total_rebated,
            total_withheld: snapshot.total_withheld,
            net_paid: snapshot.net_paid,
            epochs_recorded: snapshot.epochs_recorded,
            last_epoch: snapshot.last_epoch,
            last_reward_score: snapshot.last_reward_score,
            open_disputes,
            last_transfer,
        }
    }
}

/// Errors surfaced by the payout service.
#[derive(Debug, Error)]
pub enum PayoutServiceError {
    /// Dispute registry rejected the operation.
    #[error("dispute registry error: {0}")]
    Registry(#[from] DisputeRegistryError),
    /// Ledger operation failed (overflow, missing epoch, insufficient funds).
    #[error("reward ledger error: {0}")]
    Ledger(#[from] RewardLedgerError),
    /// Dispute references an epoch without a recorded payout.
    #[error("payout record missing for relay {relay:?} epoch {epoch}")]
    MissingPayoutRecord {
        /// Relay identifier.
        relay: RelayId,
        /// Epoch number.
        epoch: u32,
    },
}

/// Ledger aggregating payouts and adjustments per relay.
#[derive(Debug, Clone, Default)]
pub struct RewardLedger {
    entries: BTreeMap<RelayId, RewardLedgerEntry>,
}

impl RewardLedger {
    fn record_reward(
        &mut self,
        instruction: RelayRewardInstructionV1,
        transfer: InstructionBox,
    ) -> Result<RewardLedgerSnapshot, RewardLedgerError> {
        let relay = instruction.relay_id;
        let entry = self
            .entries
            .entry(relay)
            .or_insert_with(|| RewardLedgerEntry::new(relay));
        entry.record(PayoutRecord::new(instruction, transfer))?;
        entry.snapshot()
    }

    fn apply_credit(
        &mut self,
        relay: RelayId,
        epoch: u32,
        dispute_id: DisputeId,
        amount: Numeric,
        notes: String,
        applied_at_unix: u64,
    ) -> Result<RewardLedgerSnapshot, RewardLedgerError> {
        let entry = self
            .entries
            .get_mut(&relay)
            .ok_or(RewardLedgerError::UnknownRelay { relay })?;
        entry.apply_credit(epoch, dispute_id, amount, notes, applied_at_unix)?;
        entry.snapshot()
    }

    fn apply_debit(
        &mut self,
        relay: RelayId,
        epoch: u32,
        dispute_id: DisputeId,
        amount: Numeric,
        notes: String,
        applied_at_unix: u64,
    ) -> Result<RewardLedgerSnapshot, RewardLedgerError> {
        let entry = self
            .entries
            .get_mut(&relay)
            .ok_or(RewardLedgerError::UnknownRelay { relay })?;
        entry.apply_debit(epoch, dispute_id, amount, notes, applied_at_unix)?;
        entry.snapshot()
    }

    fn snapshot(&self, relay: RelayId) -> Result<RewardLedgerSnapshot, RewardLedgerError> {
        self.entries
            .get(&relay)
            .ok_or(RewardLedgerError::UnknownRelay { relay })?
            .snapshot()
    }

    fn payout_record(&self, relay: RelayId, epoch: u32) -> Option<&PayoutRecord> {
        self.entries.get(&relay)?.payouts.get(&epoch)
    }

    fn payout_owner(&self, relay: RelayId, epoch: u32) -> Option<(AccountId, AssetDefinitionId)> {
        self.payout_record(relay, epoch).map(|record| {
            let beneficiary = record.instruction.beneficiary.clone();
            let asset_def = record.instruction.payout_asset_id.clone();
            (beneficiary, asset_def)
        })
    }

    fn iter(&self) -> impl Iterator<Item = (&RelayId, &RewardLedgerEntry)> {
        self.entries.iter()
    }
}

/// Snapshot of a relay's ledger entry.
#[derive(Debug, Clone)]
pub struct RewardLedgerSnapshot {
    pub relay_id: RelayId,
    pub total_paid: Numeric,
    pub total_rebated: Numeric,
    pub total_withheld: Numeric,
    pub net_paid: Numeric,
    pub epochs_recorded: usize,
    pub last_epoch: Option<u32>,
    pub last_reward_score: Option<u64>,
}

/// Ledger-level failures.
#[derive(Debug, Error)]
pub enum RewardLedgerError {
    /// Duplicate payout recorded for the same epoch.
    #[error("duplicate payout entry for relay {relay:?} epoch {epoch}")]
    DuplicateEpoch { relay: RelayId, epoch: u32 },
    /// Numeric overflow while updating totals.
    #[error("numeric overflow while updating reward ledger")]
    NumericOverflow,
    /// Relay identifier unknown to the ledger.
    #[error("relay {relay:?} not present in reward ledger")]
    UnknownRelay { relay: RelayId },
    /// Epoch missing when applying an adjustment.
    #[error("relay {relay:?} has no payout for epoch {epoch}")]
    UnknownEpoch { relay: RelayId, epoch: u32 },
    /// Debit amount exceeds available net funds.
    #[error(
        "debit of {requested:?} exceeds net balance {available:?} for relay {relay:?} epoch {epoch}"
    )]
    InsufficientNet {
        relay: RelayId,
        epoch: u32,
        requested: Numeric,
        available: Numeric,
    },
    /// Net amount became negative after adjustments.
    #[error("negative net payout detected for relay {relay:?}")]
    NegativeNet { relay: RelayId },
}

#[derive(Debug, Clone)]
struct RewardLedgerEntry {
    relay_id: RelayId,
    payouts: BTreeMap<u32, PayoutRecord>,
    total_paid: Numeric,
    total_rebated: Numeric,
    total_withheld: Numeric,
    epochs_recorded: usize,
    last_epoch: Option<u32>,
    last_reward_score: Option<u64>,
}

impl RewardLedgerEntry {
    fn new(relay_id: RelayId) -> Self {
        Self {
            relay_id,
            payouts: BTreeMap::new(),
            total_paid: Numeric::zero(),
            total_rebated: Numeric::zero(),
            total_withheld: Numeric::zero(),
            epochs_recorded: 0,
            last_epoch: None,
            last_reward_score: None,
        }
    }

    fn record(&mut self, record: PayoutRecord) -> Result<(), RewardLedgerError> {
        let epoch = record.instruction.epoch;
        if self.payouts.contains_key(&epoch) {
            return Err(RewardLedgerError::DuplicateEpoch {
                relay: self.relay_id,
                epoch,
            });
        }

        let reward_score = record.instruction.reward_score;
        let payout_amount = record.instruction.payout_amount.clone();
        self.total_paid = self
            .total_paid
            .clone()
            .checked_add(payout_amount)
            .ok_or(RewardLedgerError::NumericOverflow)?;
        self.payouts.insert(epoch, record);
        self.epochs_recorded = self.epochs_recorded.saturating_add(1);
        self.last_epoch = Some(epoch);
        self.last_reward_score = Some(reward_score);
        Ok(())
    }

    fn apply_credit(
        &mut self,
        epoch: u32,
        dispute_id: DisputeId,
        amount: Numeric,
        notes: String,
        applied_at_unix: u64,
    ) -> Result<(), RewardLedgerError> {
        let amount_for_totals = amount.clone();
        let record = self
            .payouts
            .get_mut(&epoch)
            .ok_or(RewardLedgerError::UnknownEpoch {
                relay: self.relay_id,
                epoch,
            })?;

        record.add_adjustment(
            dispute_id,
            AdjustmentKind::Credit,
            amount,
            notes,
            applied_at_unix,
        );
        self.total_rebated = self
            .total_rebated
            .clone()
            .checked_add(amount_for_totals)
            .ok_or(RewardLedgerError::NumericOverflow)?;
        self.ensure_non_negative_net()
    }

    fn apply_debit(
        &mut self,
        epoch: u32,
        dispute_id: DisputeId,
        amount: Numeric,
        notes: String,
        applied_at_unix: u64,
    ) -> Result<(), RewardLedgerError> {
        let amount_for_totals = amount.clone();
        let record = self
            .payouts
            .get_mut(&epoch)
            .ok_or(RewardLedgerError::UnknownEpoch {
                relay: self.relay_id,
                epoch,
            })?;

        let available = record.net_amount()?;
        if amount > available {
            return Err(RewardLedgerError::InsufficientNet {
                relay: self.relay_id,
                epoch,
                requested: amount,
                available,
            });
        }

        record.add_adjustment(
            dispute_id,
            AdjustmentKind::Debit,
            amount,
            notes,
            applied_at_unix,
        );
        self.total_withheld = self
            .total_withheld
            .clone()
            .checked_add(amount_for_totals)
            .ok_or(RewardLedgerError::NumericOverflow)?;
        self.ensure_non_negative_net()
    }

    fn snapshot(&self) -> Result<RewardLedgerSnapshot, RewardLedgerError> {
        let net = self
            .total_paid
            .clone()
            .checked_add(self.total_rebated.clone())
            .ok_or(RewardLedgerError::NumericOverflow)?
            .checked_sub(self.total_withheld.clone())
            .ok_or(RewardLedgerError::NegativeNet {
                relay: self.relay_id,
            })?;

        Ok(RewardLedgerSnapshot {
            relay_id: self.relay_id,
            total_paid: self.total_paid.clone(),
            total_rebated: self.total_rebated.clone(),
            total_withheld: self.total_withheld.clone(),
            net_paid: net,
            epochs_recorded: self.epochs_recorded,
            last_epoch: self.last_epoch,
            last_reward_score: self.last_reward_score,
        })
    }

    fn latest_transfer_summary(&self) -> Option<TransferSummary> {
        let (&epoch, record) = self.payouts.iter().next_back()?;
        record.transfer_summary(epoch)
    }

    fn ensure_non_negative_net(&self) -> Result<(), RewardLedgerError> {
        let _ = self.snapshot()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PayoutRecord {
    instruction: RelayRewardInstructionV1,
    transfer: InstructionBox,
    adjustments: Vec<AdjustmentRecord>,
}

impl PayoutRecord {
    fn new(instruction: RelayRewardInstructionV1, transfer: InstructionBox) -> Self {
        Self {
            instruction,
            transfer,
            adjustments: Vec::new(),
        }
    }

    fn add_adjustment(
        &mut self,
        dispute_id: DisputeId,
        kind: AdjustmentKind,
        amount: Numeric,
        notes: String,
        applied_at_unix: u64,
    ) {
        self.adjustments.push(AdjustmentRecord::new(
            dispute_id,
            kind,
            amount,
            notes,
            applied_at_unix,
        ));
    }

    fn net_amount(&self) -> Result<Numeric, RewardLedgerError> {
        let mut net = self.instruction.payout_amount.clone();
        for adjustment in &self.adjustments {
            net = match adjustment.kind {
                AdjustmentKind::Credit => net
                    .checked_add(adjustment.amount.clone())
                    .ok_or(RewardLedgerError::NumericOverflow)?,
                AdjustmentKind::Debit => net
                    .checked_sub(adjustment.amount.clone())
                    .ok_or(RewardLedgerError::NumericOverflow)?,
            };
        }
        Ok(net)
    }

    fn transfer_summary(&self, epoch: u32) -> Option<TransferSummary> {
        let transfer_box = self.transfer.as_any().downcast_ref::<TransferBox>()?;
        let TransferBox::Asset(transfer) = transfer_box else {
            return None;
        };
        Some(TransferSummary {
            epoch,
            source_asset: transfer.source.clone(),
            destination: transfer.destination.clone(),
            amount: transfer.object.clone(),
        })
    }
}

/// Adjustment recorded against a payout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdjustmentRecord {
    pub dispute_id: DisputeId,
    pub kind: AdjustmentKind,
    pub amount: Numeric,
    pub notes: String,
    pub applied_at_unix: u64,
}

impl AdjustmentRecord {
    fn new(
        dispute_id: DisputeId,
        kind: AdjustmentKind,
        amount: Numeric,
        notes: String,
        applied_at_unix: u64,
    ) -> Self {
        Self {
            dispute_id,
            kind,
            amount,
            notes,
            applied_at_unix,
        }
    }
}

/// Direction of an adjustment applied to a payout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustmentKind {
    /// Treasury credited additional XOR.
    Credit,
    /// Treasury clawed back XOR.
    Debit,
}

/// Adjustment requested by the dispute filer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdjustmentRequest {
    pub kind: AdjustmentKind,
    pub amount: Numeric,
}

/// Record describing a lifecycle-managed dispute.
#[derive(Debug, Clone)]
pub struct RewardDispute {
    pub id: DisputeId,
    pub relay_id: RelayId,
    pub epoch: u32,
    pub filed_at_unix: u64,
    pub reason: String,
    pub requested_adjustment: Option<AdjustmentRequest>,
    pub status: DisputeStatus,
    pub norito_record: RelayRewardDisputeV1,
}

impl RewardDispute {
    /// Access the Norito dispute record for persistence.
    #[must_use]
    pub fn norito_record(&self) -> &RelayRewardDisputeV1 {
        &self.norito_record
    }
}

/// Status of a dispute in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisputeStatus {
    /// Dispute awaits review.
    Open,
    /// Dispute resolved with a given outcome.
    Resolved {
        resolved_at_unix: u64,
        outcome: ResolvedDispute,
    },
    /// Dispute rejected with an explanation.
    Rejected {
        rejected_at_unix: u64,
        notes: String,
    },
}

/// Outcome registered when closing a dispute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDispute {
    pub kind: ResolutionKind,
    pub amount: Option<Numeric>,
    pub notes: String,
}

/// Resolution direction used when closing a dispute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionKind {
    /// No ledger changes required.
    NoChange,
    /// Additional XOR credited to the relay.
    Credit,
    /// XOR clawed back from the relay.
    Debit,
}

/// Operator-supplied resolution payload.
#[derive(Debug, Clone)]
pub enum DisputeResolution {
    /// Close without modifications.
    NoChange { notes: String },
    /// Credit the relay with additional XOR.
    Credit { amount: Numeric, notes: String },
    /// Debit XOR from the relay.
    Debit { amount: Numeric, notes: String },
}

/// Registry tracking disputes keyed by identifier and relay.
#[derive(Debug, Default, Clone)]
pub struct RewardDisputeRegistry {
    disputes: BTreeMap<DisputeId, RewardDispute>,
    disputes_by_relay: BTreeMap<RelayId, BTreeSet<DisputeId>>,
    next_id: DisputeId,
}

impl RewardDisputeRegistry {
    fn file(
        &mut self,
        relay_id: RelayId,
        epoch: u32,
        reason: String,
        filed_at_unix: u64,
        requested_adjustment: Option<AdjustmentRequest>,
        norito_record: RelayRewardDisputeV1,
    ) -> RewardDispute {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);

        let dispute = RewardDispute {
            id,
            relay_id,
            epoch,
            filed_at_unix,
            reason,
            requested_adjustment,
            status: DisputeStatus::Open,
            norito_record,
        };

        self.disputes.insert(id, dispute.clone());
        self.disputes_by_relay
            .entry(relay_id)
            .or_default()
            .insert(id);

        dispute
    }

    fn resolve(
        &mut self,
        dispute_id: DisputeId,
        resolution: DisputeResolution,
        resolved_at_unix: u64,
    ) -> Result<RewardDispute, DisputeRegistryError> {
        let dispute = self
            .disputes
            .get_mut(&dispute_id)
            .ok_or(DisputeRegistryError::UnknownDispute { dispute_id })?;

        if !matches!(dispute.status, DisputeStatus::Open) {
            return Err(DisputeRegistryError::DisputeClosed { dispute_id });
        }

        let outcome = match resolution {
            DisputeResolution::NoChange { notes } => ResolvedDispute {
                kind: ResolutionKind::NoChange,
                amount: None,
                notes,
            },
            DisputeResolution::Credit { amount, notes } => ResolvedDispute {
                kind: ResolutionKind::Credit,
                amount: Some(amount),
                notes,
            },
            DisputeResolution::Debit { amount, notes } => ResolvedDispute {
                kind: ResolutionKind::Debit,
                amount: Some(amount),
                notes,
            },
        };

        dispute.status = DisputeStatus::Resolved {
            resolved_at_unix,
            outcome: outcome.clone(),
        };
        dispute.norito_record.status = RelayRewardDisputeStatusV1::Accepted;
        dispute.norito_record.resolution_metadata = resolution_metadata(&outcome, resolved_at_unix);

        if let Some(active) = self.disputes_by_relay.get_mut(&dispute.relay_id) {
            active.remove(&dispute_id);
        }

        Ok(dispute.clone())
    }

    fn reject(
        &mut self,
        dispute_id: DisputeId,
        rejected_at_unix: u64,
        notes: impl Into<String>,
    ) -> Result<RewardDispute, DisputeRegistryError> {
        let dispute = self
            .disputes
            .get_mut(&dispute_id)
            .ok_or(DisputeRegistryError::UnknownDispute { dispute_id })?;

        if !matches!(dispute.status, DisputeStatus::Open) {
            return Err(DisputeRegistryError::DisputeClosed { dispute_id });
        }

        let notes = notes.into();
        dispute.status = DisputeStatus::Rejected {
            rejected_at_unix,
            notes: notes.clone(),
        };
        dispute.norito_record.status = RelayRewardDisputeStatusV1::Rejected;
        dispute.norito_record.resolution_metadata = rejection_metadata(&notes, rejected_at_unix);

        if let Some(active) = self.disputes_by_relay.get_mut(&dispute.relay_id) {
            active.remove(&dispute_id);
        }

        Ok(dispute.clone())
    }

    fn open_count_for_relay(&self, relay_id: &RelayId) -> usize {
        self.disputes_by_relay
            .get(relay_id)
            .map(|set| set.len())
            .unwrap_or_default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&DisputeId, &RewardDispute)> {
        self.disputes.iter()
    }
}

/// Errors surfaced by the dispute registry.
#[derive(Debug, Error)]
pub enum DisputeRegistryError {
    /// Dispute identifier unknown to the registry.
    #[error("unknown dispute id {dispute_id}")]
    UnknownDispute { dispute_id: DisputeId },
    /// Attempted to mutate an already closed dispute.
    #[error("dispute {dispute_id} is already closed")]
    DisputeClosed { dispute_id: DisputeId },
}

fn metadata_insert(metadata: &mut Metadata, key: &str, value: impl Into<Json>) {
    if let Ok(name) = Name::from_str(key) {
        metadata.insert(name, value);
    }
}

fn resolution_metadata(outcome: &ResolvedDispute, timestamp: u64) -> Metadata {
    let mut metadata = Metadata::default();
    metadata_insert(
        &mut metadata,
        "resolution_kind",
        Json::new(match outcome.kind {
            ResolutionKind::NoChange => "no_change",
            ResolutionKind::Credit => "credit",
            ResolutionKind::Debit => "debit",
        }),
    );
    if let Some(amount) = &outcome.amount {
        metadata_insert(
            &mut metadata,
            "resolution_amount",
            Json::new(amount.to_string()),
        );
    }
    metadata_insert(
        &mut metadata,
        "resolution_notes",
        Json::new(outcome.notes.clone()),
    );
    metadata_insert(&mut metadata, "resolved_at_unix", Json::new(timestamp));
    metadata
}

fn rejection_metadata(notes: &str, timestamp: u64) -> Metadata {
    let mut metadata = Metadata::default();
    metadata_insert(&mut metadata, "resolution_kind", Json::new("rejected"));
    metadata_insert(
        &mut metadata,
        "resolution_notes",
        Json::new(notes.to_owned()),
    );
    metadata_insert(&mut metadata, "resolved_at_unix", Json::new(timestamp));
    metadata
}

fn mismatch_reasons(
    expected: &LedgerTransferRecord,
    actual: &LedgerTransferRecord,
) -> Vec<MismatchReason> {
    let mut reasons = Vec::new();
    if expected.amount != actual.amount {
        reasons.push(MismatchReason::Amount);
    }
    if expected.source_asset != actual.source_asset {
        reasons.push(MismatchReason::SourceAsset);
    }
    if expected.destination != actual.destination {
        reasons.push(MismatchReason::Destination);
    }
    reasons
}

fn record_dispute_metric(action: &str) {
    if let Some(metrics) = iroha_telemetry::metrics::global() {
        metrics.inc_soranet_dispute(action);
    }
}

fn record_adjustment_metric(relay_id: RelayId, amount: &Numeric, kind: &str) {
    if let Some(metrics) = iroha_telemetry::metrics::global()
        && let Some(nanos) = numeric_to_nanos(amount)
    {
        metrics.record_soranet_adjustment(&hex_encode(relay_id), nanos, kind);
    }
}

fn numeric_to_nanos(amount: &Numeric) -> Option<u128> {
    let scale = amount.scale();
    let mantissa = amount
        .try_mantissa_u128()
        .expect("positive amount mantissa should fit u128");
    if scale >= 9 {
        let divisor = 10u128.checked_pow(scale.saturating_sub(9))?;
        mantissa.checked_div(divisor)
    } else {
        let multiplier = 10u128.checked_pow(9 - scale)?;
        mantissa.checked_mul(multiplier)
    }
}

mod relay_id_json {
    use hex::{decode, encode};
    use norito::json::{JsonDeserialize, JsonSerialize, Parser};

    use super::*;

    pub fn serialize(relay_id: &RelayId, out: &mut String) {
        JsonSerialize::json_serialize(&encode(relay_id), out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<RelayId, norito::json::Error> {
        let value = String::json_deserialize(parser)?;
        let bytes = decode(&value).map_err(|err| norito::json::Error::Message(err.to_string()))?;
        if bytes.len() != 32 {
            return Err(norito::json::Error::Message(format!(
                "expected 64 hex characters for relay id, found {}",
                value.len()
            )));
        }
        let mut relay_id = [0u8; 32];
        relay_id.copy_from_slice(&bytes);
        Ok(relay_id)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        metadata::Metadata,
        name::Name,
        soranet::incentives::{RelayBondPolicyV1, RelayComplianceStatusV1},
    };

    use super::*;
    use crate::incentives::RewardConfig;

    fn numeric(value: u32) -> Numeric {
        Numeric::from(value)
    }

    fn asset_id() -> AssetDefinitionId {
        let domain = DomainId::from_str("sora").expect("domain id");
        let name = Name::from_str("xor").expect("asset name");
        AssetDefinitionId::new(domain, name)
    }

    fn budget_id() -> [u8; 32] {
        [0xCC; 32]
    }

    fn account(seed: u8) -> AccountId {
        let (public_key, _) = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519).into_parts();
        AccountId::new(public_key)
    }

    fn telemetry_handle() -> Arc<iroha_telemetry::metrics::Metrics> {
        if let Some(existing) = iroha_telemetry::metrics::global() {
            existing.clone()
        } else {
            let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
            let _ = iroha_telemetry::metrics::install_global(metrics.clone());
            metrics
        }
    }

    fn bond_entry(amount: u32) -> RelayBondLedgerEntryV1 {
        RelayBondLedgerEntryV1 {
            relay_id: [0xAB; 32],
            bonded_amount: numeric(amount),
            bond_asset_id: asset_id(),
            bonded_since_unix: 1,
            exit_capable: true,
        }
    }

    fn metrics(epoch: u32, bandwidth: u128) -> RelayEpochMetricsV1 {
        RelayEpochMetricsV1 {
            relay_id: [0xAB; 32],
            epoch,
            uptime_seconds: 3600,
            scheduled_uptime_seconds: 3600,
            verified_bandwidth_bytes: bandwidth,
            compliance: RelayComplianceStatusV1::Clean,
            reward_score: 50,
            confidence_floor_per_mille: 1_000,
            measurement_ids: Vec::new(),
            metadata: Metadata::default(),
        }
    }

    fn reward_engine() -> RelayRewardEngine {
        let policy = RelayBondPolicyV1 {
            minimum_exit_bond: numeric(500),
            bond_asset_id: asset_id(),
            uptime_floor_per_mille: 900,
            slash_penalty_basis_points: 100,
            activation_grace_epochs: 0,
        };
        let config = RewardConfig {
            policy,
            base_reward: numeric(100),
            uptime_weight_per_mille: 500,
            bandwidth_weight_per_mille: 500,
            compliance_penalty_basis_points: 0,
            bandwidth_target_bytes: 1_000,
            budget_approval_id: Some(budget_id()),
            metrics_log_path: None,
        };
        RelayRewardEngine::new(config).expect("valid reward config")
    }

    fn payout_service() -> (RelayPayoutService, AccountId) {
        let treasury = account(42);
        let service =
            RelayPayoutService::new(reward_engine(), RelayPayoutLedger::new(treasury.clone()));
        (service, treasury)
    }

    fn expected_exports(service: &RelayPayoutService) -> Vec<LedgerTransferRecord> {
        service
            .reconcile_ledger(&[])
            .missing_transfers
            .into_iter()
            .map(|entry| entry.record)
            .collect()
    }

    #[test]
    fn process_epoch_records_payout() {
        let (mut service, _) = payout_service();
        let metrics = metrics(1, 1_000);
        let bond = bond_entry(1_000);
        let outcome = service
            .process_epoch(&metrics, &bond, account(1), Metadata::default())
            .expect("payout recorded");

        assert_eq!(outcome.instruction.payout_amount, numeric(100));
        assert_eq!(outcome.ledger_snapshot.total_paid, numeric(100));
        assert_eq!(outcome.ledger_snapshot.net_paid, numeric(100));
    }

    #[test]
    fn process_batch_records_multiple_epochs() {
        let (mut service, _) = payout_service();
        let bond = bond_entry(1_000);
        let first = metrics(2, 900);
        let second = metrics(3, 1_200);

        let outcomes = service
            .process_batch([
                PayoutInput {
                    metrics: &first,
                    bond_entry: &bond,
                    beneficiary: account(2),
                    metadata: Metadata::default(),
                },
                PayoutInput {
                    metrics: &second,
                    bond_entry: &bond,
                    beneficiary: account(3),
                    metadata: Metadata::default(),
                },
            ])
            .expect("batch processed");

        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].instruction.epoch, first.epoch);
        assert_eq!(outcomes[1].instruction.epoch, second.epoch);

        let snapshot = service
            .ledger_snapshot(first.relay_id)
            .expect("ledger snapshot");
        assert_eq!(snapshot.epochs_recorded, 2);
        assert!(snapshot.net_paid > Numeric::zero());
    }

    #[test]
    fn credit_resolution_emits_transfer() {
        let (mut service, treasury) = payout_service();
        let metrics = metrics(2, 1_000);
        let bond = bond_entry(1_000);
        let beneficiary = account(2);
        service
            .process_epoch(&metrics, &bond, beneficiary.clone(), Metadata::default())
            .expect("payout recorded");

        let dispute = service
            .file_dispute(
                [0xAB; 32],
                2,
                account(9),
                numeric(125),
                "missing bytes",
                50,
                Some(AdjustmentRequest {
                    kind: AdjustmentKind::Credit,
                    amount: numeric(25),
                }),
            )
            .expect("dispute filed");
        assert_eq!(
            dispute.norito_record().status,
            RelayRewardDisputeStatusV1::Pending
        );

        let outcome = service
            .resolve_dispute(
                dispute.id,
                DisputeResolution::Credit {
                    amount: numeric(25),
                    notes: "approved".into(),
                },
                60,
            )
            .expect("resolution succeeds");

        assert!(outcome.transfer.is_some());
        assert_eq!(
            outcome.dispute.norito_record.status,
            RelayRewardDisputeStatusV1::Accepted
        );
        let transfer = outcome.transfer.unwrap();
        let transfer_box = transfer
            .as_any()
            .downcast_ref::<TransferBox>()
            .expect("transfer instruction");
        let TransferBox::Asset(transfer) = transfer_box else {
            panic!("expected asset transfer, found {transfer_box:?}");
        };
        assert_eq!(transfer.source, AssetId::new(asset_id(), treasury));
        assert_eq!(transfer.destination, beneficiary);
        assert_eq!(transfer.object, numeric(25));
    }

    #[test]
    fn debit_resolution_claws_back() {
        let (mut service, treasury) = payout_service();
        let metrics = metrics(3, 1_000);
        let bond = bond_entry(1_000);
        let beneficiary = account(3);
        service
            .process_epoch(&metrics, &bond, beneficiary.clone(), Metadata::default())
            .expect("payout recorded");

        let dispute = service
            .file_dispute(
                [0xAB; 32],
                3,
                account(10),
                numeric(60),
                "incorrect measurement",
                55,
                Some(AdjustmentRequest {
                    kind: AdjustmentKind::Debit,
                    amount: numeric(40),
                }),
            )
            .expect("dispute filed");

        let outcome = service
            .resolve_dispute(
                dispute.id,
                DisputeResolution::Debit {
                    amount: numeric(40),
                    notes: "clawed back".into(),
                },
                70,
            )
            .expect("resolution succeeds");
        let transfer = outcome.transfer.unwrap();
        let transfer_box = transfer
            .as_any()
            .downcast_ref::<TransferBox>()
            .expect("transfer instruction");
        let TransferBox::Asset(transfer) = transfer_box else {
            panic!("expected asset transfer, found {transfer_box:?}");
        };
        assert_eq!(transfer.source, AssetId::new(asset_id(), beneficiary));
        assert_eq!(transfer.destination, treasury);
        assert_eq!(transfer.object, numeric(40));
    }

    #[test]
    fn reject_dispute_updates_status() {
        let (mut service, _) = payout_service();
        let metrics = metrics(4, 1_000);
        let bond = bond_entry(1_000);
        service
            .process_epoch(&metrics, &bond, account(4), Metadata::default())
            .expect("payout recorded");

        let dispute = service
            .file_dispute(
                [0xAB; 32],
                4,
                account(11),
                numeric(100),
                "late measurement",
                80,
                None,
            )
            .expect("dispute filed");

        let rejected = service
            .reject_dispute(dispute.id, 90, "insufficient evidence")
            .expect("rejection succeeds");
        assert!(matches!(rejected.status, DisputeStatus::Rejected { .. }));
        assert_eq!(
            rejected.norito_record.status,
            RelayRewardDisputeStatusV1::Rejected
        );
    }

    #[test]
    fn dispute_metrics_increment_for_resolution() {
        let metrics_handle = telemetry_handle();
        let relay_hex = hex_encode([0xAB; 32]);
        let filed_before = metrics_handle
            .soranet_reward_disputes_total
            .with_label_values(&["filed"])
            .get() as u64;
        let resolved_before = metrics_handle
            .soranet_reward_disputes_total
            .with_label_values(&["resolved_credit"])
            .get() as u64;
        let credit_before = metrics_handle
            .soranet_reward_adjustment_nanos_total
            .with_label_values(&[relay_hex.as_str(), "credit"])
            .get() as u64;

        let (mut service, _) = payout_service();
        let bond = bond_entry(1_000);
        let metrics = metrics(18, 1_100);
        service
            .process_epoch(&metrics, &bond, account(18), Metadata::default())
            .expect("payout recorded");

        let dispute = service
            .file_dispute(
                metrics.relay_id,
                metrics.epoch,
                account(19),
                numeric(80),
                "measurement dispute",
                1_000,
                Some(AdjustmentRequest {
                    kind: AdjustmentKind::Credit,
                    amount: numeric(25),
                }),
            )
            .expect("dispute filed");

        service
            .resolve_dispute(
                dispute.id,
                DisputeResolution::Credit {
                    amount: numeric(25),
                    notes: "approved".into(),
                },
                1_020,
            )
            .expect("dispute resolved");

        let filed_after = metrics_handle
            .soranet_reward_disputes_total
            .with_label_values(&["filed"])
            .get() as u64;
        let resolved_after = metrics_handle
            .soranet_reward_disputes_total
            .with_label_values(&["resolved_credit"])
            .get() as u64;
        let credit_after = metrics_handle
            .soranet_reward_adjustment_nanos_total
            .with_label_values(&[relay_hex.as_str(), "credit"])
            .get() as u64;

        let expected_credit =
            u64::try_from(numeric_to_nanos(&numeric(25)).expect("convert numeric")).expect("u64");

        assert!(
            filed_after >= filed_before.saturating_add(1),
            "expected filed counter to increase (before={filed_before}, after={filed_after})"
        );
        assert!(
            resolved_after >= resolved_before.saturating_add(1),
            "expected resolved counter to increase (before={resolved_before}, after={resolved_after})"
        );
        assert!(
            credit_after >= credit_before.saturating_add(expected_credit),
            "expected credit counter to increase by at least {expected_credit} (before={credit_before}, after={credit_after})"
        );
    }

    #[test]
    fn reject_dispute_records_metric() {
        let metrics_handle = telemetry_handle();
        let rejected_before = metrics_handle
            .soranet_reward_disputes_total
            .with_label_values(&["rejected"])
            .get() as u64;

        let (mut service, _) = payout_service();
        let bond = bond_entry(1_000);
        let metrics = metrics(19, 1_000);
        service
            .process_epoch(&metrics, &bond, account(19), Metadata::default())
            .expect("payout recorded");

        let dispute = service
            .file_dispute(
                metrics.relay_id,
                metrics.epoch,
                account(20),
                numeric(40),
                "rejection test",
                1_000,
                None,
            )
            .expect("dispute filed");
        service
            .reject_dispute(dispute.id, 1_010, "no evidence")
            .expect("dispute rejected");

        let rejected_after = metrics_handle
            .soranet_reward_disputes_total
            .with_label_values(&["rejected"])
            .get() as u64;
        assert!(
            rejected_after >= rejected_before.saturating_add(1),
            "expected rejected counter to increase (before={rejected_before}, after={rejected_after})"
        );
    }

    #[test]
    fn dashboard_summarises_rows() {
        let (mut service, treasury) = payout_service();
        let bond = bond_entry(1_000);

        service
            .process_epoch(&metrics(5, 800), &bond, account(5), Metadata::default())
            .expect("payout recorded");
        service
            .process_epoch(&metrics(6, 600), &bond, account(5), Metadata::default())
            .expect("second payout recorded");

        let dispute = service
            .file_dispute(
                [0xAB; 32],
                6,
                account(12),
                numeric(100),
                "late measurement",
                100,
                None,
            )
            .expect("dispute filed");
        service
            .resolve_dispute(
                dispute.id,
                DisputeResolution::NoChange {
                    notes: "no action".into(),
                },
                120,
            )
            .expect("resolved");

        let dashboard = service.earnings_dashboard().expect("dashboard renders");
        assert_eq!(dashboard.total_relays, 1);
        assert_eq!(dashboard.rows[0].epochs_recorded, 2);
        assert!(dashboard.rows[0].net_paid > Numeric::zero());
        let last_transfer = dashboard.rows[0]
            .last_transfer
            .as_ref()
            .expect("last transfer summary present");
        assert_eq!(last_transfer.epoch, 6);
        assert_eq!(last_transfer.source_asset.account, treasury);
        assert_eq!(last_transfer.destination, account(5));
        assert_eq!(last_transfer.amount, numeric(80));
    }

    #[test]
    fn reconcile_matches_expected_transfers() {
        let (mut service, _) = payout_service();
        let bond = bond_entry(1_000);

        service
            .process_epoch(&metrics(10, 900), &bond, account(10), Metadata::default())
            .expect("epoch recorded");
        service
            .process_epoch(&metrics(11, 1_200), &bond, account(11), Metadata::default())
            .expect("epoch recorded");

        let exports = expected_exports(&service);
        let report = service.reconcile_ledger(&exports);

        assert!(report.is_clean());
        assert_eq!(report.matched_transfers, exports.len());
        assert_eq!(report.expected_amount_nanos, report.exported_amount_nanos);
        assert!(report.missing_transfers.is_empty());
        assert!(report.unexpected_transfers.is_empty());
        assert!(report.mismatched_transfers.is_empty());
    }

    #[test]
    fn reconcile_detects_missing_transfer() {
        let (mut service, _) = payout_service();
        let bond = bond_entry(1_000);

        service
            .process_epoch(&metrics(12, 1_000), &bond, account(12), Metadata::default())
            .expect("epoch recorded");
        service
            .process_epoch(&metrics(13, 1_050), &bond, account(13), Metadata::default())
            .expect("epoch recorded");

        let mut exports = expected_exports(&service);
        let missing = exports.pop().expect("at least one export");
        let report = service.reconcile_ledger(&exports);

        assert!(!report.is_clean());
        assert_eq!(report.missing_transfers.len(), 1);
        assert_eq!(
            report.missing_transfers[0].record.relay_id,
            missing.relay_id
        );
        assert_eq!(report.matched_transfers, exports.len());
    }

    #[test]
    fn reconcile_reports_unexpected_transfer() {
        let (mut service, _) = payout_service();
        let bond = bond_entry(1_000);

        service
            .process_epoch(&metrics(14, 1_000), &bond, account(14), Metadata::default())
            .expect("epoch recorded");

        let mut exports = expected_exports(&service);
        let mut unexpected = exports[0].clone();
        unexpected.epoch += 100;
        exports.push(unexpected.clone());

        let report = service.reconcile_ledger(&exports);
        assert!(!report.is_clean());
        assert_eq!(report.unexpected_transfers.len(), 1);
        assert_eq!(report.unexpected_transfers[0], unexpected);
    }

    #[test]
    fn reconcile_detects_mismatch_amount() {
        let (mut service, _) = payout_service();
        let bond = bond_entry(1_000);

        service
            .process_epoch(&metrics(15, 1_000), &bond, account(15), Metadata::default())
            .expect("epoch recorded");

        let mut exports = expected_exports(&service);
        exports[0].amount = numeric(999);

        let report = service.reconcile_ledger(&exports);
        assert!(!report.is_clean());
        assert_eq!(report.mismatched_transfers.len(), 1);
        let mismatch = &report.mismatched_transfers[0];
        assert!(
            mismatch
                .reasons
                .iter()
                .any(|reason| matches!(reason, MismatchReason::Amount))
        );
    }

    #[test]
    fn reconcile_includes_adjustments() {
        let (mut service, treasury) = payout_service();
        let bond = bond_entry(1_000);
        let relay_account = account(16);
        let relay_id = [0xCD; 32];
        let mut metric = metrics(20, 1_200);
        metric.relay_id = relay_id;

        service
            .process_epoch(&metric, &bond, relay_account.clone(), Metadata::default())
            .expect("epoch recorded");

        let credit_dispute = service
            .file_dispute(
                relay_id,
                20,
                account(90),
                numeric(125),
                "credit adjustment",
                1_000,
                Some(AdjustmentRequest {
                    kind: AdjustmentKind::Credit,
                    amount: numeric(25),
                }),
            )
            .expect("credit dispute filed");
        let credit_outcome = service
            .resolve_dispute(
                credit_dispute.id,
                DisputeResolution::Credit {
                    amount: numeric(25),
                    notes: "approved".into(),
                },
                1_010,
            )
            .expect("credit dispute resolved");
        assert!(credit_outcome.transfer.is_some());

        let debit_dispute = service
            .file_dispute(
                relay_id,
                20,
                account(91),
                numeric(10),
                "debit adjustment",
                1_020,
                Some(AdjustmentRequest {
                    kind: AdjustmentKind::Debit,
                    amount: numeric(10),
                }),
            )
            .expect("debit dispute filed");
        let debit_outcome = service
            .resolve_dispute(
                debit_dispute.id,
                DisputeResolution::Debit {
                    amount: numeric(10),
                    notes: "claw back".into(),
                },
                1_030,
            )
            .expect("debit dispute resolved");
        assert!(debit_outcome.transfer.is_some());

        let exports = expected_exports(&service);
        assert_eq!(exports.len(), 3);
        let report = service.reconcile_ledger(&exports);
        assert!(report.is_clean());

        let payout_count = exports
            .iter()
            .filter(|record| record.kind == TransferKind::Payout)
            .count();
        let credit_count = exports
            .iter()
            .filter(|record| record.kind == TransferKind::Credit)
            .count();
        let debit_count = exports
            .iter()
            .filter(|record| record.kind == TransferKind::Debit)
            .count();

        assert_eq!(payout_count, 1);
        assert_eq!(credit_count, 1);
        assert_eq!(debit_count, 1);

        // Ensure credit transfer points from treasury to relay and debit reverses.
        let credit = exports
            .iter()
            .find(|record| record.kind == TransferKind::Credit)
            .expect("credit record present");
        assert_eq!(credit.source_asset.account, treasury);
        assert_eq!(credit.destination, relay_account);

        let debit = exports
            .iter()
            .find(|record| record.kind == TransferKind::Debit)
            .expect("debit record present");
        assert_eq!(debit.source_asset.account, relay_account);
        assert_eq!(debit.destination, treasury);
    }

    #[test]
    fn replay_instruction_preserves_ledgers() {
        let (mut service, treasury) = payout_service();
        let bond = bond_entry(1_000);
        let metrics = metrics(30, 1_500);

        let outcome = service
            .process_epoch(&metrics, &bond, account(30), Metadata::default())
            .expect("epoch recorded");

        let instruction = outcome.instruction.clone();

        let mut replay_service =
            RelayPayoutService::new(reward_engine(), RelayPayoutLedger::new(treasury));
        replay_service
            .record_reward(instruction.clone())
            .expect("replay succeeds");

        let original_snapshot = service
            .ledger_snapshot(instruction.relay_id)
            .expect("original snapshot");
        let replay_snapshot = replay_service
            .ledger_snapshot(instruction.relay_id)
            .expect("replay snapshot");

        assert_eq!(original_snapshot.total_paid, replay_snapshot.total_paid);
        assert_eq!(original_snapshot.net_paid, replay_snapshot.net_paid);
        assert_eq!(
            original_snapshot.epochs_recorded,
            replay_snapshot.epochs_recorded
        );
    }
}
