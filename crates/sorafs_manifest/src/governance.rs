//! Governance DAG node schemas used for audit publishing.

use std::collections::{BTreeMap, BTreeSet};

use blake3::Hasher;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signature as DalekSignature, Verifier, VerifyingKey,
};
use iroha_crypto::{Algorithm, PublicKey, Signature as IrohaSignature};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::{
    capacity::ReplicationOrderV1,
    deal::DealSettlementV1,
    por::{AuditVerdictV1, PorChallengeV1, PorProofV1, PorReportIsoWeek},
    reputation::ReputationSnapshotV1,
};

/// Current governance log schema version.
pub const GOVERNANCE_LOG_VERSION_V1: u8 = 1;

/// Current public Governance DAG block schema version.
pub const GOVERNANCE_DAG_BLOCK_VERSION_V1: u8 = 1;

/// Current public Governance DAG head manifest schema version.
pub const GOVERNANCE_DAG_HEAD_VERSION_V1: u8 = 1;

/// Current moderation ballot governance event schema version.
pub const SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1: u16 = 1;

/// Current SoraFS appeal finance report schema version.
pub const SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1: u16 = 1;

/// Current SoraFS appeal finance weekly rollup schema version.
pub const SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1: u16 = 1;

const GOVERNANCE_DAG_BLOCK_CID_DOMAIN_V1: &[u8] = b"sorafs.governance_dag.block.cid.v1";
const GOVERNANCE_LOG_NODE_CID_DOMAIN_V1: &[u8] = b"sorafs.governance_log.node.cid.v1";

/// Governance DAG event kind for a SoraFS moderation ballot lifecycle transition.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SoraFsModerationBallotGovernanceEventKindV1 {
    /// Ballot announcement accepted by a node.
    BallotAnnounced,
    /// Juror commitment accepted by a node.
    CommitAccepted,
    /// Juror reveal accepted by a node.
    RevealAccepted,
    /// Ballot tally finalized by a node.
    BallotTallied,
}

impl SoraFsModerationBallotGovernanceEventKindV1 {
    /// Stable label used in local indexes and JSON sidecars.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BallotAnnounced => "ballot_announced",
            Self::CommitAccepted => "commit_accepted",
            Self::RevealAccepted => "reveal_accepted",
            Self::BallotTallied => "ballot_tallied",
        }
    }
}

/// Governance DAG vote choice for SoraFS moderation ballots.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
#[norito(tag = "choice", content = "value", rename_all = "snake_case")]
pub enum SoraFsModerationVoteChoiceV1 {
    /// Keep the original moderation action.
    Uphold,
    /// Reverse the original moderation action.
    Overturn,
    /// Change the moderation action without fully reversing it.
    Modify,
    /// Escalate the case for another review path.
    Escalate,
}

impl SoraFsModerationVoteChoiceV1 {
    /// Stable label used in local indexes and JSON sidecars.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uphold => "uphold",
            Self::Overturn => "overturn",
            Self::Modify => "modify",
            Self::Escalate => "escalate",
        }
    }
}

/// Vote totals by moderation choice for a SoraFS ballot tally.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct SoraFsModerationVoteCountsV1 {
    /// Number of `uphold` reveals.
    pub uphold: u32,
    /// Number of `overturn` reveals.
    pub overturn: u32,
    /// Number of `modify` reveals.
    pub modify: u32,
    /// Number of `escalate` reveals.
    pub escalate: u32,
}

impl SoraFsModerationVoteCountsV1 {
    /// Total votes represented by these counts.
    pub fn total_votes(self) -> u64 {
        u64::from(self.uphold)
            .saturating_add(u64::from(self.overturn))
            .saturating_add(u64::from(self.modify))
            .saturating_add(u64::from(self.escalate))
    }

    fn winning_choice(self) -> Option<SoraFsModerationVoteChoiceV1> {
        let choices = [
            (SoraFsModerationVoteChoiceV1::Uphold, self.uphold),
            (SoraFsModerationVoteChoiceV1::Overturn, self.overturn),
            (SoraFsModerationVoteChoiceV1::Modify, self.modify),
            (SoraFsModerationVoteChoiceV1::Escalate, self.escalate),
        ];
        let max_votes = choices.iter().map(|(_, count)| *count).max().unwrap_or(0);
        if max_votes == 0
            || choices
                .iter()
                .filter(|(_, count)| *count == max_votes)
                .count()
                != 1
        {
            return None;
        }
        choices
            .into_iter()
            .find_map(|(choice, count)| (count == max_votes).then_some(choice))
    }
}

/// Final tally carried by a SoraFS moderation ballot governance event.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsModerationBallotGovernanceTallyV1 {
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Vote counts by moderation choice.
    pub counts: SoraFsModerationVoteCountsV1,
    /// Number of valid reveals included in the tally.
    pub votes_total: u32,
    /// Required reveal quorum.
    pub quorum: u16,
    /// Winning choice when the tally has exactly one highest vote count.
    #[norito(default)]
    pub winning_choice: Option<SoraFsModerationVoteChoiceV1>,
    /// True when quorum was reached but no unique winner exists.
    pub contested: bool,
    /// UTC timestamp (milliseconds) when the tally was finalized locally.
    pub tallied_at_unix_ms: u64,
}

impl SoraFsModerationBallotGovernanceTallyV1 {
    fn validate(
        &self,
        event_case_id: &str,
        event_round_id: &str,
    ) -> Result<(), SoraFsModerationBallotGovernanceEventValidationError> {
        validate_non_empty_governance_label(
            &self.case_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingCaseId,
        )?;
        validate_non_empty_governance_label(
            &self.round_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingRoundId,
        )?;
        if self.case_id != event_case_id {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::TallyCaseMismatch {
                    event: event_case_id.to_owned(),
                    tally: self.case_id.clone(),
                },
            );
        }
        if self.round_id != event_round_id {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::TallyRoundMismatch {
                    event: event_round_id.to_owned(),
                    tally: self.round_id.clone(),
                },
            );
        }
        if self.quorum == 0 {
            return Err(SoraFsModerationBallotGovernanceEventValidationError::InvalidQuorum);
        }
        let counted = self.counts.total_votes();
        if counted != u64::from(self.votes_total) {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::VoteCountMismatch {
                    counted,
                    votes_total: self.votes_total,
                },
            );
        }
        if self.votes_total < u32::from(self.quorum) {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::QuorumNotMet {
                    quorum: self.quorum,
                    votes_total: self.votes_total,
                },
            );
        }
        let expected_winner = self.counts.winning_choice();
        if self.winning_choice != expected_winner {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::WinningChoiceMismatch,
            );
        }
        if self.contested != self.winning_choice.is_none() {
            return Err(SoraFsModerationBallotGovernanceEventValidationError::ContestedMismatch);
        }
        Ok(())
    }
}

/// Governance DAG payload for one local SoraFS moderation ballot lifecycle event.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsModerationBallotGovernanceEventV1 {
    /// Schema version (`SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1`).
    pub version: u16,
    /// Monotonic local event sequence.
    pub sequence: u64,
    /// Event kind.
    pub kind: SoraFsModerationBallotGovernanceEventKindV1,
    /// UTC timestamp (milliseconds) when the event was generated.
    pub generated_at_unix_ms: u64,
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Juror associated with commit/reveal events.
    #[norito(default)]
    pub juror_id: Option<String>,
    /// Accepted commitment count after the event.
    pub committed_count: u64,
    /// Accepted reveal count after the event.
    pub revealed_count: u64,
    /// Final tally for `BallotTallied` events.
    #[norito(default)]
    pub tally: Option<SoraFsModerationBallotGovernanceTallyV1>,
}

impl SoraFsModerationBallotGovernanceEventV1 {
    /// Validate structural invariants for a moderation ballot governance event.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsModerationBallotGovernanceEventValidationError`] when the
    /// schema version is unsupported, required identifiers are missing, or the
    /// lifecycle kind does not match the juror/tally fields.
    pub fn validate(&self) -> Result<(), SoraFsModerationBallotGovernanceEventValidationError> {
        if self.version != SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1 {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::UnsupportedVersion {
                    expected: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
                    found: self.version,
                },
            );
        }
        validate_non_empty_governance_label(
            &self.case_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingCaseId,
        )?;
        validate_non_empty_governance_label(
            &self.round_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingRoundId,
        )?;

        match self.kind {
            SoraFsModerationBallotGovernanceEventKindV1::BallotAnnounced => {
                if self.juror_id.is_some() {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::UnexpectedJurorId,
                    );
                }
                if self.tally.is_some() {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::UnexpectedTally,
                    );
                }
            }
            SoraFsModerationBallotGovernanceEventKindV1::CommitAccepted
            | SoraFsModerationBallotGovernanceEventKindV1::RevealAccepted => {
                let Some(juror_id) = self.juror_id.as_deref() else {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::MissingJurorId,
                    );
                };
                validate_non_empty_governance_label(
                    juror_id,
                    SoraFsModerationBallotGovernanceEventValidationError::MissingJurorId,
                )?;
                if self.tally.is_some() {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::UnexpectedTally,
                    );
                }
            }
            SoraFsModerationBallotGovernanceEventKindV1::BallotTallied => {
                if self.juror_id.is_some() {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::UnexpectedJurorId,
                    );
                }
                let Some(tally) = self.tally.as_ref() else {
                    return Err(SoraFsModerationBallotGovernanceEventValidationError::MissingTally);
                };
                tally.validate(&self.case_id, &self.round_id)?;
            }
        }
        Ok(())
    }
}

/// Final SoraFS appeal outcome used for finance reporting.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
#[norito(tag = "outcome", content = "value", rename_all = "snake_case")]
pub enum SoraFsAppealFinanceOutcomeV1 {
    /// Original moderation action was kept.
    Uphold,
    /// Original moderation action was reversed.
    Overturn,
    /// Original moderation action was changed without a full reversal.
    Modify,
    /// Appeal was withdrawn before jurors were seated.
    WithdrawnBeforePanel,
    /// Appeal was withdrawn after jurors were seated.
    WithdrawnAfterPanel,
    /// Appeal was marked frivolous.
    Frivolous,
    /// Appeal remains escalated and funds are held for follow-up.
    Escalated,
}

impl SoraFsAppealFinanceOutcomeV1 {
    /// Stable label used in local indexes and JSON sidecars.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uphold => "uphold",
            Self::Overturn => "overturn",
            Self::Modify => "modify",
            Self::WithdrawnBeforePanel => "withdrawn_before_panel",
            Self::WithdrawnAfterPanel => "withdrawn_after_panel",
            Self::Frivolous => "frivolous",
            Self::Escalated => "escalated",
        }
    }
}

/// Account-level appeal finance flow.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsAppealFinanceAccountFlowV1 {
    /// Canonical account id receiving or holding the amount.
    pub account_id: String,
    /// Exact non-negative XOR decimal amount.
    pub amount_xor: String,
}

impl SoraFsAppealFinanceAccountFlowV1 {
    fn validate(&self, role: &'static str) -> Result<(), SoraFsAppealFinanceReportValidationError> {
        validate_non_empty_appeal_finance_label(
            &self.account_id,
            SoraFsAppealFinanceReportValidationError::MissingAccountId { role },
        )?;
        validate_appeal_finance_decimal(&self.amount_xor).map_err(|reason| {
            SoraFsAppealFinanceReportValidationError::InvalidAmount {
                field: role,
                reason,
            }
        })
    }
}

/// Per-juror appeal finance payout.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsAppealFinanceJurorPayoutV1 {
    /// Canonical juror account id.
    pub juror_id: String,
    /// Exact non-negative stipend XOR decimal amount.
    pub stipend_xor: String,
    /// Exact non-negative bonus XOR decimal amount.
    pub bonus_xor: String,
    /// Exact non-negative total XOR decimal amount.
    pub total_xor: String,
}

impl SoraFsAppealFinanceJurorPayoutV1 {
    fn validate(&self) -> Result<(), SoraFsAppealFinanceReportValidationError> {
        validate_non_empty_appeal_finance_label(
            &self.juror_id,
            SoraFsAppealFinanceReportValidationError::MissingJurorId,
        )?;
        for (field, value) in [
            ("stipend_xor", &self.stipend_xor),
            ("bonus_xor", &self.bonus_xor),
            ("total_xor", &self.total_xor),
        ] {
            validate_appeal_finance_decimal(value).map_err(|reason| {
                SoraFsAppealFinanceReportValidationError::InvalidAmount { field, reason }
            })?;
        }
        Ok(())
    }
}

/// Governance DAG appeal finance report for settlement/disbursement audits.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsAppealFinanceReportV1 {
    /// Schema version (`SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1`).
    pub version: u16,
    /// Stable report identifier.
    pub report_id: [u8; 16],
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Optional moderation ballot round identifier.
    #[norito(default)]
    pub round_id: Option<String>,
    /// UTC timestamp (milliseconds) when the report was generated.
    pub generated_at_unix_ms: u64,
    /// Appeal finance config version used to derive the plan.
    pub appeal_finance_config_version: String,
    /// Optional evidence bundle digest reviewed by the panel.
    #[norito(default)]
    pub evidence_bundle_digest: Option<[u8; 32]>,
    /// Final appeal outcome.
    pub outcome: SoraFsAppealFinanceOutcomeV1,
    /// Exact non-negative deposited XOR decimal amount.
    pub deposit_xor: String,
    /// Refund transfer line.
    pub refund: SoraFsAppealFinanceAccountFlowV1,
    /// Treasury transfer line, including slashed deposit and forfeited rewards.
    pub treasury: SoraFsAppealFinanceAccountFlowV1,
    /// Held-escrow line.
    pub held: SoraFsAppealFinanceAccountFlowV1,
    /// Declared panel size.
    pub panel_size: u32,
    /// Exact non-negative total panel reward budget.
    pub panel_reward_total_xor: String,
    /// Exact non-negative paid panel reward total.
    pub rewards_paid_total_xor: String,
    /// Exact non-negative rewards forfeited to treasury.
    pub rewards_forfeited_treasury_xor: String,
    /// Juror payout lines for attending jurors.
    pub juror_payouts: Vec<SoraFsAppealFinanceJurorPayoutV1>,
    /// Canonical juror account ids that forfeited payout by no-show.
    #[norito(default)]
    pub no_show_juror_ids: Vec<String>,
}

impl SoraFsAppealFinanceReportV1 {
    /// Validate structural invariants for an appeal finance report.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsAppealFinanceReportValidationError`] when required
    /// identifiers are missing, decimal amounts are malformed, or the
    /// attendance lines do not reconcile to the declared panel size.
    pub fn validate(&self) -> Result<(), SoraFsAppealFinanceReportValidationError> {
        if self.version != SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1 {
            return Err(
                SoraFsAppealFinanceReportValidationError::UnsupportedVersion {
                    expected: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
                    found: self.version,
                },
            );
        }
        if self.report_id == [0u8; 16] {
            return Err(SoraFsAppealFinanceReportValidationError::MissingReportId);
        }
        validate_non_empty_appeal_finance_label(
            &self.case_id,
            SoraFsAppealFinanceReportValidationError::MissingCaseId,
        )?;
        if let Some(round_id) = self.round_id.as_deref() {
            validate_non_empty_appeal_finance_label(
                round_id,
                SoraFsAppealFinanceReportValidationError::MissingRoundId,
            )?;
        }
        if self.generated_at_unix_ms == 0 {
            return Err(SoraFsAppealFinanceReportValidationError::MissingGeneratedAt);
        }
        validate_non_empty_appeal_finance_label(
            &self.appeal_finance_config_version,
            SoraFsAppealFinanceReportValidationError::MissingFinanceConfigVersion,
        )?;
        if self
            .evidence_bundle_digest
            .as_ref()
            .is_some_and(|digest| *digest == [0u8; 32])
        {
            return Err(SoraFsAppealFinanceReportValidationError::InvalidEvidenceBundleDigest);
        }
        validate_appeal_finance_decimal(&self.deposit_xor).map_err(|reason| {
            SoraFsAppealFinanceReportValidationError::InvalidAmount {
                field: "deposit_xor",
                reason,
            }
        })?;
        self.refund.validate("refund")?;
        self.treasury.validate("treasury")?;
        self.held.validate("held")?;
        if self.panel_size == 0 {
            return Err(SoraFsAppealFinanceReportValidationError::InvalidPanelSize);
        }
        for (field, value) in [
            ("panel_reward_total_xor", &self.panel_reward_total_xor),
            ("rewards_paid_total_xor", &self.rewards_paid_total_xor),
            (
                "rewards_forfeited_treasury_xor",
                &self.rewards_forfeited_treasury_xor,
            ),
        ] {
            validate_appeal_finance_decimal(value).map_err(|reason| {
                SoraFsAppealFinanceReportValidationError::InvalidAmount { field, reason }
            })?;
        }

        let mut payout_jurors = BTreeSet::new();
        for payout in &self.juror_payouts {
            payout.validate()?;
            if !payout_jurors.insert(payout.juror_id.clone()) {
                return Err(SoraFsAppealFinanceReportValidationError::DuplicateJurorId {
                    juror_id: payout.juror_id.clone(),
                });
            }
        }
        let mut no_show_jurors = BTreeSet::new();
        for juror_id in &self.no_show_juror_ids {
            validate_non_empty_appeal_finance_label(
                juror_id,
                SoraFsAppealFinanceReportValidationError::MissingNoShowJurorId,
            )?;
            if !no_show_jurors.insert(juror_id.clone()) {
                return Err(
                    SoraFsAppealFinanceReportValidationError::DuplicateNoShowJurorId {
                        juror_id: juror_id.clone(),
                    },
                );
            }
            if payout_jurors.contains(juror_id) {
                return Err(SoraFsAppealFinanceReportValidationError::NoShowJurorPaid {
                    juror_id: juror_id.clone(),
                });
            }
        }
        let accounted = self
            .juror_payouts
            .len()
            .saturating_add(self.no_show_juror_ids.len());
        let panel_size = usize::try_from(self.panel_size).map_err(|_| {
            SoraFsAppealFinanceReportValidationError::PanelSizeOverflow {
                panel_size: self.panel_size,
            }
        })?;
        if accounted != panel_size {
            return Err(
                SoraFsAppealFinanceReportValidationError::PanelReconciliation {
                    panel_size: self.panel_size,
                    accounted,
                },
            );
        }
        Ok(())
    }
}

/// Outcome-level summary for weekly appeal finance rollups.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsAppealFinanceOutcomeRollupV1 {
    /// Final appeal outcome represented by this row.
    pub outcome: SoraFsAppealFinanceOutcomeV1,
    /// Number of source reports for this outcome.
    pub report_count: u64,
    /// Number of distinct case ids for this outcome.
    pub case_count: u64,
    /// Total deposited XOR across source reports.
    pub total_deposit_xor: String,
    /// Total refunded XOR across source reports.
    pub total_refund_xor: String,
    /// Total treasury-bound XOR across source reports.
    pub total_treasury_xor: String,
    /// Total held escrow XOR across source reports.
    pub total_held_xor: String,
    /// Total panel reward budget across source reports.
    pub total_panel_reward_xor: String,
    /// Total panel rewards paid across source reports.
    pub total_rewards_paid_xor: String,
    /// Total forfeited rewards sent to treasury across source reports.
    pub total_rewards_forfeited_treasury_xor: String,
    /// Number of juror payout lines represented by this row.
    pub juror_payout_count: u64,
    /// Number of no-show juror ids represented by this row.
    pub no_show_juror_count: u64,
}

impl SoraFsAppealFinanceOutcomeRollupV1 {
    fn validate(&self) -> Result<(), SoraFsAppealFinanceWeeklyRollupValidationError> {
        if self.report_count == 0 {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::EmptyOutcome {
                    outcome: self.outcome,
                },
            );
        }
        if self.case_count == 0 || self.case_count > self.report_count {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::InvalidOutcomeCaseCount {
                    outcome: self.outcome,
                    case_count: self.case_count,
                    report_count: self.report_count,
                },
            );
        }
        for (field, value) in [
            ("total_deposit_xor", &self.total_deposit_xor),
            ("total_refund_xor", &self.total_refund_xor),
            ("total_treasury_xor", &self.total_treasury_xor),
            ("total_held_xor", &self.total_held_xor),
            ("total_panel_reward_xor", &self.total_panel_reward_xor),
            ("total_rewards_paid_xor", &self.total_rewards_paid_xor),
            (
                "total_rewards_forfeited_treasury_xor",
                &self.total_rewards_forfeited_treasury_xor,
            ),
        ] {
            validate_appeal_finance_decimal(value).map_err(|reason| {
                SoraFsAppealFinanceWeeklyRollupValidationError::InvalidAmount { field, reason }
            })?;
        }
        Ok(())
    }
}

/// Weekly appeal finance transparency rollup for dashboards and treasury review.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsAppealFinanceWeeklyRollupV1 {
    /// Schema version (`SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1`).
    pub version: u16,
    /// ISO-8601 reporting cycle.
    pub cycle: PorReportIsoWeek,
    /// UTC timestamp (milliseconds) when the rollup was generated.
    pub generated_at_unix_ms: u64,
    /// Number of source reports included.
    pub report_count: u64,
    /// Number of distinct case ids included.
    pub case_count: u64,
    /// Sorted appeal finance config versions observed in source reports.
    pub appeal_finance_config_versions: Vec<String>,
    /// Total deposited XOR across source reports.
    pub total_deposit_xor: String,
    /// Total refunded XOR across source reports.
    pub total_refund_xor: String,
    /// Total treasury-bound XOR across source reports.
    pub total_treasury_xor: String,
    /// Total held escrow XOR across source reports.
    pub total_held_xor: String,
    /// Total panel reward budget across source reports.
    pub total_panel_reward_xor: String,
    /// Total panel rewards paid across source reports.
    pub total_rewards_paid_xor: String,
    /// Total forfeited rewards sent to treasury across source reports.
    pub total_rewards_forfeited_treasury_xor: String,
    /// Number of juror payout lines represented by this rollup.
    pub juror_payout_count: u64,
    /// Number of no-show juror ids represented by this rollup.
    pub no_show_juror_count: u64,
    /// Outcome-level dashboard rows.
    pub outcomes: Vec<SoraFsAppealFinanceOutcomeRollupV1>,
    /// Sorted source report ids included in the rollup.
    pub source_report_ids: Vec<[u8; 16]>,
}

impl SoraFsAppealFinanceWeeklyRollupV1 {
    /// Build a deterministic weekly rollup from validated appeal finance reports.
    ///
    /// Source report order does not affect the resulting rollup. Report ids and
    /// config versions are sorted, and outcome rows are emitted in stable enum
    /// order.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsAppealFinanceWeeklyRollupBuildError`] when the cycle,
    /// generated timestamp, source report set, or computed rollup is invalid.
    pub fn from_reports(
        cycle: PorReportIsoWeek,
        generated_at_unix_ms: u64,
        reports: &[SoraFsAppealFinanceReportV1],
    ) -> Result<Self, SoraFsAppealFinanceWeeklyRollupBuildError> {
        cycle
            .validate()
            .map_err(SoraFsAppealFinanceWeeklyRollupBuildError::InvalidCycle)?;
        if generated_at_unix_ms == 0 {
            return Err(SoraFsAppealFinanceWeeklyRollupBuildError::MissingGeneratedAt);
        }
        if reports.is_empty() {
            return Err(SoraFsAppealFinanceWeeklyRollupBuildError::NoReports);
        }

        let mut report_ids = BTreeSet::new();
        let mut case_ids = BTreeSet::new();
        let mut config_versions = BTreeSet::new();
        let mut totals = AppealFinanceRollupAccumulator::new();
        let mut outcome_totals = BTreeMap::new();

        for (index, report) in reports.iter().enumerate() {
            report.validate().map_err(|source| {
                SoraFsAppealFinanceWeeklyRollupBuildError::InvalidReport { index, source }
            })?;
            if !report_ids.insert(report.report_id) {
                return Err(
                    SoraFsAppealFinanceWeeklyRollupBuildError::DuplicateReportId {
                        report_id: report.report_id,
                    },
                );
            }
            case_ids.insert(report.case_id.clone());
            config_versions.insert(report.appeal_finance_config_version.clone());
            totals.add_report(report)?;
            outcome_totals
                .entry(report.outcome)
                .or_insert_with(AppealFinanceOutcomeAccumulator::new)
                .add_report(report)?;
        }

        let outcomes = outcome_totals
            .into_iter()
            .map(|(outcome, accumulator)| accumulator.finish(outcome))
            .collect();
        let rollup = Self {
            version: SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1,
            cycle,
            generated_at_unix_ms,
            report_count: reports.len() as u64,
            case_count: case_ids.len() as u64,
            appeal_finance_config_versions: config_versions.into_iter().collect(),
            total_deposit_xor: totals.total_deposit_xor,
            total_refund_xor: totals.total_refund_xor,
            total_treasury_xor: totals.total_treasury_xor,
            total_held_xor: totals.total_held_xor,
            total_panel_reward_xor: totals.total_panel_reward_xor,
            total_rewards_paid_xor: totals.total_rewards_paid_xor,
            total_rewards_forfeited_treasury_xor: totals.total_rewards_forfeited_treasury_xor,
            juror_payout_count: totals.juror_payout_count,
            no_show_juror_count: totals.no_show_juror_count,
            outcomes,
            source_report_ids: report_ids.into_iter().collect(),
        };
        rollup
            .validate()
            .map_err(SoraFsAppealFinanceWeeklyRollupBuildError::InvalidRollup)?;
        Ok(rollup)
    }

    /// Validate structural and aggregate invariants for the weekly rollup.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsAppealFinanceWeeklyRollupValidationError`] when required
    /// identifiers are missing, totals are malformed, or top-level totals do not
    /// reconcile with the outcome rows.
    pub fn validate(&self) -> Result<(), SoraFsAppealFinanceWeeklyRollupValidationError> {
        if self.version != SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1 {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::UnsupportedVersion {
                    expected: SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1,
                    found: self.version,
                },
            );
        }
        self.cycle
            .validate()
            .map_err(SoraFsAppealFinanceWeeklyRollupValidationError::InvalidCycle)?;
        if self.generated_at_unix_ms == 0 {
            return Err(SoraFsAppealFinanceWeeklyRollupValidationError::MissingGeneratedAt);
        }
        if self.report_count == 0 {
            return Err(SoraFsAppealFinanceWeeklyRollupValidationError::NoReports);
        }
        if self.case_count == 0 || self.case_count > self.report_count {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::InvalidCaseCount {
                    case_count: self.case_count,
                    report_count: self.report_count,
                },
            );
        }
        validate_sorted_non_empty_labels(
            "appeal_finance_config_versions",
            &self.appeal_finance_config_versions,
        )?;
        if self.outcomes.is_empty() {
            return Err(SoraFsAppealFinanceWeeklyRollupValidationError::NoOutcomes);
        }

        for (field, value) in [
            ("total_deposit_xor", &self.total_deposit_xor),
            ("total_refund_xor", &self.total_refund_xor),
            ("total_treasury_xor", &self.total_treasury_xor),
            ("total_held_xor", &self.total_held_xor),
            ("total_panel_reward_xor", &self.total_panel_reward_xor),
            ("total_rewards_paid_xor", &self.total_rewards_paid_xor),
            (
                "total_rewards_forfeited_treasury_xor",
                &self.total_rewards_forfeited_treasury_xor,
            ),
        ] {
            validate_appeal_finance_decimal(value).map_err(|reason| {
                SoraFsAppealFinanceWeeklyRollupValidationError::InvalidAmount { field, reason }
            })?;
        }

        let mut source_report_ids = BTreeSet::new();
        for report_id in &self.source_report_ids {
            if *report_id == [0u8; 16] {
                return Err(SoraFsAppealFinanceWeeklyRollupValidationError::MissingSourceReportId);
            }
            if !source_report_ids.insert(*report_id) {
                return Err(
                    SoraFsAppealFinanceWeeklyRollupValidationError::DuplicateSourceReportId {
                        report_id: *report_id,
                    },
                );
            }
        }
        if self.source_report_ids.len() as u64 != self.report_count {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::SourceReportCountMismatch {
                    report_count: self.report_count,
                    source_report_count: self.source_report_ids.len() as u64,
                },
            );
        }

        let mut seen_outcomes = BTreeSet::new();
        let mut reconciled = AppealFinanceRollupAccumulator::new();
        for row in &self.outcomes {
            row.validate()?;
            if !seen_outcomes.insert(row.outcome) {
                return Err(
                    SoraFsAppealFinanceWeeklyRollupValidationError::DuplicateOutcome {
                        outcome: row.outcome,
                    },
                );
            }
            reconciled.add_outcome(row)?;
        }
        reconciled.compare(self)
    }
}

#[derive(Debug)]
struct AppealFinanceRollupAccumulator {
    total_deposit_xor: String,
    total_refund_xor: String,
    total_treasury_xor: String,
    total_held_xor: String,
    total_panel_reward_xor: String,
    total_rewards_paid_xor: String,
    total_rewards_forfeited_treasury_xor: String,
    report_count: u64,
    juror_payout_count: u64,
    no_show_juror_count: u64,
}

impl AppealFinanceRollupAccumulator {
    fn new() -> Self {
        Self {
            total_deposit_xor: "0".to_string(),
            total_refund_xor: "0".to_string(),
            total_treasury_xor: "0".to_string(),
            total_held_xor: "0".to_string(),
            total_panel_reward_xor: "0".to_string(),
            total_rewards_paid_xor: "0".to_string(),
            total_rewards_forfeited_treasury_xor: "0".to_string(),
            report_count: 0,
            juror_payout_count: 0,
            no_show_juror_count: 0,
        }
    }

    fn add_report(
        &mut self,
        report: &SoraFsAppealFinanceReportV1,
    ) -> Result<(), SoraFsAppealFinanceWeeklyRollupBuildError> {
        self.total_deposit_xor = add_report_amount(
            report,
            "deposit_xor",
            &self.total_deposit_xor,
            &report.deposit_xor,
        )?;
        self.total_refund_xor = add_report_amount(
            report,
            "refund.amount_xor",
            &self.total_refund_xor,
            &report.refund.amount_xor,
        )?;
        self.total_treasury_xor = add_report_amount(
            report,
            "treasury.amount_xor",
            &self.total_treasury_xor,
            &report.treasury.amount_xor,
        )?;
        self.total_held_xor = add_report_amount(
            report,
            "held.amount_xor",
            &self.total_held_xor,
            &report.held.amount_xor,
        )?;
        self.total_panel_reward_xor = add_report_amount(
            report,
            "panel_reward_total_xor",
            &self.total_panel_reward_xor,
            &report.panel_reward_total_xor,
        )?;
        self.total_rewards_paid_xor = add_report_amount(
            report,
            "rewards_paid_total_xor",
            &self.total_rewards_paid_xor,
            &report.rewards_paid_total_xor,
        )?;
        self.total_rewards_forfeited_treasury_xor = add_report_amount(
            report,
            "rewards_forfeited_treasury_xor",
            &self.total_rewards_forfeited_treasury_xor,
            &report.rewards_forfeited_treasury_xor,
        )?;
        self.report_count = self.report_count.saturating_add(1);
        self.juror_payout_count = self
            .juror_payout_count
            .saturating_add(report.juror_payouts.len() as u64);
        self.no_show_juror_count = self
            .no_show_juror_count
            .saturating_add(report.no_show_juror_ids.len() as u64);
        Ok(())
    }

    fn add_outcome(
        &mut self,
        row: &SoraFsAppealFinanceOutcomeRollupV1,
    ) -> Result<(), SoraFsAppealFinanceWeeklyRollupValidationError> {
        self.total_deposit_xor = add_rollup_amount(
            "outcomes.total_deposit_xor",
            &self.total_deposit_xor,
            &row.total_deposit_xor,
        )?;
        self.total_refund_xor = add_rollup_amount(
            "outcomes.total_refund_xor",
            &self.total_refund_xor,
            &row.total_refund_xor,
        )?;
        self.total_treasury_xor = add_rollup_amount(
            "outcomes.total_treasury_xor",
            &self.total_treasury_xor,
            &row.total_treasury_xor,
        )?;
        self.total_held_xor = add_rollup_amount(
            "outcomes.total_held_xor",
            &self.total_held_xor,
            &row.total_held_xor,
        )?;
        self.total_panel_reward_xor = add_rollup_amount(
            "outcomes.total_panel_reward_xor",
            &self.total_panel_reward_xor,
            &row.total_panel_reward_xor,
        )?;
        self.total_rewards_paid_xor = add_rollup_amount(
            "outcomes.total_rewards_paid_xor",
            &self.total_rewards_paid_xor,
            &row.total_rewards_paid_xor,
        )?;
        self.total_rewards_forfeited_treasury_xor = add_rollup_amount(
            "outcomes.total_rewards_forfeited_treasury_xor",
            &self.total_rewards_forfeited_treasury_xor,
            &row.total_rewards_forfeited_treasury_xor,
        )?;
        self.report_count = self.report_count.saturating_add(row.report_count);
        self.juror_payout_count = self
            .juror_payout_count
            .saturating_add(row.juror_payout_count);
        self.no_show_juror_count = self
            .no_show_juror_count
            .saturating_add(row.no_show_juror_count);
        Ok(())
    }

    fn compare(
        self,
        rollup: &SoraFsAppealFinanceWeeklyRollupV1,
    ) -> Result<(), SoraFsAppealFinanceWeeklyRollupValidationError> {
        for (field, expected, actual) in [
            (
                "total_deposit_xor",
                &rollup.total_deposit_xor,
                &self.total_deposit_xor,
            ),
            (
                "total_refund_xor",
                &rollup.total_refund_xor,
                &self.total_refund_xor,
            ),
            (
                "total_treasury_xor",
                &rollup.total_treasury_xor,
                &self.total_treasury_xor,
            ),
            (
                "total_held_xor",
                &rollup.total_held_xor,
                &self.total_held_xor,
            ),
            (
                "total_panel_reward_xor",
                &rollup.total_panel_reward_xor,
                &self.total_panel_reward_xor,
            ),
            (
                "total_rewards_paid_xor",
                &rollup.total_rewards_paid_xor,
                &self.total_rewards_paid_xor,
            ),
            (
                "total_rewards_forfeited_treasury_xor",
                &rollup.total_rewards_forfeited_treasury_xor,
                &self.total_rewards_forfeited_treasury_xor,
            ),
        ] {
            let expected = canonicalize_appeal_finance_decimal(expected).map_err(|reason| {
                SoraFsAppealFinanceWeeklyRollupValidationError::InvalidAmount { field, reason }
            })?;
            let actual = canonicalize_appeal_finance_decimal(actual).map_err(|reason| {
                SoraFsAppealFinanceWeeklyRollupValidationError::InvalidAmount { field, reason }
            })?;
            if expected != actual {
                return Err(
                    SoraFsAppealFinanceWeeklyRollupValidationError::OutcomeAmountMismatch {
                        field,
                        expected,
                        actual,
                    },
                );
            }
        }
        if self.report_count != rollup.report_count {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::OutcomeReportCountMismatch {
                    report_count: rollup.report_count,
                    outcome_report_count: self.report_count,
                },
            );
        }
        if self.juror_payout_count != rollup.juror_payout_count {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::OutcomeJurorPayoutCountMismatch {
                    juror_payout_count: rollup.juror_payout_count,
                    outcome_juror_payout_count: self.juror_payout_count,
                },
            );
        }
        if self.no_show_juror_count != rollup.no_show_juror_count {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::OutcomeNoShowCountMismatch {
                    no_show_juror_count: rollup.no_show_juror_count,
                    outcome_no_show_juror_count: self.no_show_juror_count,
                },
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
struct AppealFinanceOutcomeAccumulator {
    case_ids: BTreeSet<String>,
    totals: AppealFinanceRollupAccumulator,
}

impl AppealFinanceOutcomeAccumulator {
    fn new() -> Self {
        Self {
            case_ids: BTreeSet::new(),
            totals: AppealFinanceRollupAccumulator::new(),
        }
    }

    fn add_report(
        &mut self,
        report: &SoraFsAppealFinanceReportV1,
    ) -> Result<(), SoraFsAppealFinanceWeeklyRollupBuildError> {
        self.case_ids.insert(report.case_id.clone());
        self.totals.add_report(report)
    }

    fn finish(self, outcome: SoraFsAppealFinanceOutcomeV1) -> SoraFsAppealFinanceOutcomeRollupV1 {
        SoraFsAppealFinanceOutcomeRollupV1 {
            outcome,
            report_count: self.totals.report_count,
            case_count: self.case_ids.len() as u64,
            total_deposit_xor: self.totals.total_deposit_xor,
            total_refund_xor: self.totals.total_refund_xor,
            total_treasury_xor: self.totals.total_treasury_xor,
            total_held_xor: self.totals.total_held_xor,
            total_panel_reward_xor: self.totals.total_panel_reward_xor,
            total_rewards_paid_xor: self.totals.total_rewards_paid_xor,
            total_rewards_forfeited_treasury_xor: self.totals.total_rewards_forfeited_treasury_xor,
            juror_payout_count: self.totals.juror_payout_count,
            no_show_juror_count: self.totals.no_show_juror_count,
        }
    }
}

fn add_report_amount(
    report: &SoraFsAppealFinanceReportV1,
    field: &'static str,
    lhs: &str,
    rhs: &str,
) -> Result<String, SoraFsAppealFinanceWeeklyRollupBuildError> {
    add_appeal_finance_decimal_strings(lhs, rhs).map_err(|reason| {
        SoraFsAppealFinanceWeeklyRollupBuildError::InvalidAmount {
            report_id: report.report_id,
            field,
            reason,
        }
    })
}

fn add_rollup_amount(
    field: &'static str,
    lhs: &str,
    rhs: &str,
) -> Result<String, SoraFsAppealFinanceWeeklyRollupValidationError> {
    add_appeal_finance_decimal_strings(lhs, rhs).map_err(|reason| {
        SoraFsAppealFinanceWeeklyRollupValidationError::InvalidAmount { field, reason }
    })
}

fn add_appeal_finance_decimal_strings(lhs: &str, rhs: &str) -> Result<String, String> {
    let lhs = split_appeal_finance_decimal(lhs)?;
    let rhs = split_appeal_finance_decimal(rhs)?;
    let scale = lhs.1.len().max(rhs.1.len());
    let lhs_digits = decimal_digits_for_scale(lhs, scale);
    let rhs_digits = decimal_digits_for_scale(rhs, scale);
    let max_len = lhs_digits.len().max(rhs_digits.len());
    let mut result = Vec::with_capacity(max_len + 1);
    let mut carry = 0u8;
    for index in 0..max_len {
        let lhs_digit = lhs_digits.get(index).copied().unwrap_or(0);
        let rhs_digit = rhs_digits.get(index).copied().unwrap_or(0);
        let sum = lhs_digit + rhs_digit + carry;
        result.push(sum % 10);
        carry = sum / 10;
    }
    if carry != 0 {
        result.push(carry);
    }
    while result.len() > scale + 1 && result.last() == Some(&0) {
        result.pop();
    }
    let mut chars: Vec<char> = result
        .into_iter()
        .rev()
        .map(|digit| char::from(b'0' + digit))
        .collect();
    if scale > 0 {
        while chars.len() <= scale {
            chars.insert(0, '0');
        }
        let split = chars.len() - scale;
        chars.insert(split, '.');
    }
    canonicalize_appeal_finance_decimal(&chars.into_iter().collect::<String>())
}

fn canonicalize_appeal_finance_decimal(value: &str) -> Result<String, String> {
    let (integral, fractional) = split_appeal_finance_decimal(value)?;
    let integral = integral.trim_start_matches('0');
    let integral = if integral.is_empty() { "0" } else { integral };
    let fractional = fractional.trim_end_matches('0');
    if fractional.is_empty() {
        Ok(integral.to_string())
    } else {
        Ok(format!("{integral}.{fractional}"))
    }
}

fn split_appeal_finance_decimal(value: &str) -> Result<(&str, &str), String> {
    validate_appeal_finance_decimal(value)?;
    match value.split_once('.') {
        Some((integral, fractional)) => Ok((integral, fractional)),
        None => Ok((value, "")),
    }
}

fn decimal_digits_for_scale(parts: (&str, &str), scale: usize) -> Vec<u8> {
    let mut digits = String::with_capacity(parts.0.len() + scale);
    digits.push_str(parts.0);
    digits.push_str(parts.1);
    for _ in parts.1.len()..scale {
        digits.push('0');
    }
    digits.bytes().rev().map(|byte| byte - b'0').collect()
}

fn validate_sorted_non_empty_labels(
    field: &'static str,
    labels: &[String],
) -> Result<(), SoraFsAppealFinanceWeeklyRollupValidationError> {
    if labels.is_empty() {
        return Err(SoraFsAppealFinanceWeeklyRollupValidationError::MissingConfigVersions);
    }
    let mut seen = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for label in labels {
        let label = label.as_str();
        if label.trim().is_empty() {
            return Err(SoraFsAppealFinanceWeeklyRollupValidationError::InvalidLabel { field });
        }
        if let Some(prev) = previous
            && prev > label
        {
            return Err(SoraFsAppealFinanceWeeklyRollupValidationError::UnsortedLabels { field });
        }
        if !seen.insert(label) {
            return Err(SoraFsAppealFinanceWeeklyRollupValidationError::DuplicateLabel { field });
        }
        previous = Some(label);
    }
    Ok(())
}

/// Governance log node payload enumeration.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub enum GovernanceLogPayloadV1 {
    /// Provider advertisement snapshot.
    ProviderAdvert(crate::provider_advert::ProviderAdvertV1),
    /// Replication order snapshot.
    ReplicationOrder(ReplicationOrderV1),
    /// Proof-of-Retrievability challenge.
    PorChallenge(PorChallengeV1),
    /// Proof-of-Retrievability response.
    PorProof(PorProofV1),
    /// Audit verdict for a challenge.
    AuditVerdict(AuditVerdictV1),
    /// Deal settlement snapshot.
    DealSettlement(DealSettlementV1),
    /// Provider reputation snapshot.
    ReputationSnapshot(ReputationSnapshotV1),
    /// SoraFS moderation ballot lifecycle event.
    ModerationBallotEvent(SoraFsModerationBallotGovernanceEventV1),
    /// SoraFS appeal finance report.
    AppealFinanceReport(SoraFsAppealFinanceReportV1),
    /// SoraFS weekly appeal finance transparency rollup.
    AppealFinanceWeeklyRollup(SoraFsAppealFinanceWeeklyRollupV1),
}

impl GovernanceLogPayloadV1 {
    fn validate(&self, timestamp: u64) -> Result<(), GovernanceLogValidationError> {
        match self {
            GovernanceLogPayloadV1::ProviderAdvert(advert) => {
                advert
                    .validate_with_body(timestamp)
                    .map_err(GovernanceLogValidationError::Advert)?;
                Ok(())
            }
            GovernanceLogPayloadV1::ReplicationOrder(order) => order
                .validate()
                .map_err(GovernanceLogValidationError::ReplicationOrder),
            GovernanceLogPayloadV1::PorChallenge(challenge) => challenge
                .validate()
                .map_err(GovernanceLogValidationError::PorChallenge),
            GovernanceLogPayloadV1::PorProof(proof) => proof
                .validate()
                .map_err(GovernanceLogValidationError::PorProof),
            GovernanceLogPayloadV1::AuditVerdict(verdict) => verdict
                .validate()
                .map_err(GovernanceLogValidationError::AuditVerdict),
            GovernanceLogPayloadV1::DealSettlement(settlement) => settlement
                .validate()
                .map_err(GovernanceLogValidationError::DealSettlement),
            GovernanceLogPayloadV1::ReputationSnapshot(snapshot) => snapshot
                .validate()
                .map_err(GovernanceLogValidationError::ReputationSnapshot),
            GovernanceLogPayloadV1::ModerationBallotEvent(event) => event
                .validate()
                .map_err(GovernanceLogValidationError::ModerationBallotEvent),
            GovernanceLogPayloadV1::AppealFinanceReport(report) => report
                .validate()
                .map_err(GovernanceLogValidationError::AppealFinanceReport),
            GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(rollup) => rollup
                .validate()
                .map_err(GovernanceLogValidationError::AppealFinanceWeeklyRollup),
        }
    }
}

fn validate_non_empty_governance_label(
    value: &str,
    error: SoraFsModerationBallotGovernanceEventValidationError,
) -> Result<(), SoraFsModerationBallotGovernanceEventValidationError> {
    if value.trim().is_empty() {
        return Err(error);
    }
    Ok(())
}

fn validate_non_empty_appeal_finance_label(
    value: &str,
    error: SoraFsAppealFinanceReportValidationError,
) -> Result<(), SoraFsAppealFinanceReportValidationError> {
    if value.trim().is_empty() {
        return Err(error);
    }
    Ok(())
}

fn validate_appeal_finance_decimal(value: &str) -> Result<(), String> {
    if value.trim() != value || value.is_empty() {
        return Err("amount must be a non-empty canonical decimal string".to_string());
    }
    let mut saw_digit = false;
    let mut saw_dot = false;
    let mut prev_dot = false;
    for byte in value.bytes() {
        match byte {
            b'0'..=b'9' => {
                saw_digit = true;
                prev_dot = false;
            }
            b'.' if !saw_dot => {
                saw_dot = true;
                prev_dot = true;
            }
            _ => {
                return Err(
                    "amount must contain only ASCII digits and at most one decimal point"
                        .to_string(),
                );
            }
        }
    }
    if !saw_digit || prev_dot {
        return Err("amount must include digits and must not end with a decimal point".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceLogNodeCidPayloadV1 {
    version: u8,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    payload: GovernanceLogPayloadV1,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceDagBlockCidPayloadV1 {
    version: u8,
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    node: GovernanceLogNodeV1,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceDagBlockSignaturePayloadV1 {
    version: u8,
    block_cid: Vec<u8>,
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    node: GovernanceLogNodeV1,
}

impl From<&GovernanceDagBlockV1> for GovernanceDagBlockSignaturePayloadV1 {
    fn from(block: &GovernanceDagBlockV1) -> Self {
        Self {
            version: block.version,
            block_cid: block.block_cid.clone(),
            prev_block_cid: block.prev_block_cid.clone(),
            sequence: block.sequence,
            timestamp: block.timestamp,
            publisher_peer_id: block.publisher_peer_id.clone(),
            node: block.node.clone(),
        }
    }
}

/// Public Governance DAG block wrapping one validated governance log node.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceDagBlockV1 {
    /// Schema version (`GOVERNANCE_DAG_BLOCK_VERSION_V1`).
    pub version: u8,
    /// Deterministic BLAKE3-256 CID bytes derived from the canonical block
    /// payload excluding signatures.
    pub block_cid: Vec<u8>,
    /// Optional parent block CID.
    #[norito(default)]
    pub prev_block_cid: Option<Vec<u8>>,
    /// Monotonic sequence number in the public DAG chain.
    pub sequence: u64,
    /// Unix timestamp (seconds) when this block was assembled.
    pub timestamp: u64,
    /// Publisher peer identifier for the DAG builder/publisher.
    pub publisher_peer_id: Vec<u8>,
    /// Governance log node carried by this block.
    pub node: GovernanceLogNodeV1,
    /// Publisher signature over the canonical block signing payload.
    pub block_signature: GovernanceLogSignatureV1,
}

impl GovernanceDagBlockV1 {
    /// Returns canonical Norito bytes signed by the block publisher.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(&GovernanceDagBlockSignaturePayloadV1::from(self))
    }

    /// Recomputes this block's deterministic CID bytes.
    pub fn recompute_block_cid(&self) -> Result<Vec<u8>, norito::core::Error> {
        governance_dag_block_cid_v1(
            self.prev_block_cid.as_deref(),
            self.sequence,
            self.timestamp,
            &self.publisher_peer_id,
            &self.node,
        )
    }

    /// Validates the block structure, embedded node, CID, and block signature.
    pub fn validate(&self) -> Result<(), GovernanceDagBlockValidationError> {
        if self.version != GOVERNANCE_DAG_BLOCK_VERSION_V1 {
            return Err(GovernanceDagBlockValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.block_cid.is_empty() {
            return Err(GovernanceDagBlockValidationError::MissingBlockCid);
        }
        if self
            .prev_block_cid
            .as_ref()
            .is_some_and(|prev| prev.is_empty())
        {
            return Err(GovernanceDagBlockValidationError::InvalidPrevBlockCid);
        }
        if self.sequence == 0 && self.prev_block_cid.is_some() {
            return Err(GovernanceDagBlockValidationError::RootHasParent);
        }
        if self.sequence > 0 && self.prev_block_cid.is_none() {
            return Err(GovernanceDagBlockValidationError::NonRootMissingParent);
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceDagBlockValidationError::MissingPublisherPeerId);
        }
        self.block_signature
            .validate()
            .map_err(|_| GovernanceDagBlockValidationError::InvalidSignature)?;
        self.node
            .validate()
            .map_err(GovernanceDagBlockValidationError::Node)?;
        self.node
            .verify_publisher_signature()
            .map_err(GovernanceDagBlockValidationError::NodeSignature)?;

        let expected_cid = self.recompute_block_cid().map_err(|err| {
            GovernanceDagBlockValidationError::CidEncoding {
                reason: err.to_string(),
            }
        })?;
        if self.block_cid != expected_cid {
            return Err(GovernanceDagBlockValidationError::InvalidBlockCid);
        }

        self.verify_block_signature()
            .map_err(GovernanceDagBlockValidationError::BlockSignature)
    }

    /// Verifies the block publisher signature.
    pub fn verify_block_signature(&self) -> Result<(), GovernanceLogSignatureVerificationError> {
        let payload_bytes = self.signature_payload_bytes().map_err(|err| {
            GovernanceLogSignatureVerificationError::PayloadEncoding {
                reason: err.to_string(),
            }
        })?;
        verify_governance_signature_bytes(&self.block_signature, &payload_bytes)
    }
}

/// Derives deterministic Governance DAG block CID bytes.
pub fn governance_dag_block_cid_v1(
    prev_block_cid: Option<&[u8]>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: &[u8],
    node: &GovernanceLogNodeV1,
) -> Result<Vec<u8>, norito::core::Error> {
    let payload = GovernanceDagBlockCidPayloadV1 {
        version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
        prev_block_cid: prev_block_cid.map(<[u8]>::to_vec),
        sequence,
        timestamp,
        publisher_peer_id: publisher_peer_id.to_vec(),
        node: node.clone(),
    };
    let payload_bytes = norito::to_bytes(&payload)?;
    let mut hasher = Hasher::new();
    hasher.update(GOVERNANCE_DAG_BLOCK_CID_DOMAIN_V1);
    hasher.update(&payload_bytes);
    Ok(hasher.finalize().as_bytes().to_vec())
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceDagHeadSignaturePayloadV1 {
    version: u8,
    head_block_cid: Vec<u8>,
    block_count: u64,
    generated_at: u64,
    publisher_peer_id: Vec<u8>,
    checkpoint_cid: Option<Vec<u8>>,
}

impl From<&GovernanceDagHeadV1> for GovernanceDagHeadSignaturePayloadV1 {
    fn from(head: &GovernanceDagHeadV1) -> Self {
        Self {
            version: head.version,
            head_block_cid: head.head_block_cid.clone(),
            block_count: head.block_count,
            generated_at: head.generated_at,
            publisher_peer_id: head.publisher_peer_id.clone(),
            checkpoint_cid: head.checkpoint_cid.clone(),
        }
    }
}

/// Signed public Governance DAG head manifest.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceDagHeadV1 {
    /// Schema version (`GOVERNANCE_DAG_HEAD_VERSION_V1`).
    pub version: u8,
    /// Current head block CID.
    pub head_block_cid: Vec<u8>,
    /// Number of blocks in the chain this head advertises.
    pub block_count: u64,
    /// Unix timestamp (seconds) when this head manifest was generated.
    pub generated_at: u64,
    /// Publisher peer identifier for the head signer.
    pub publisher_peer_id: Vec<u8>,
    /// Optional trusted checkpoint or previous public head CID.
    #[norito(default)]
    pub checkpoint_cid: Option<Vec<u8>>,
    /// Publisher signature over the canonical head manifest payload.
    pub head_signature: GovernanceLogSignatureV1,
}

impl GovernanceDagHeadV1 {
    /// Returns canonical Norito bytes signed by the head publisher.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(&GovernanceDagHeadSignaturePayloadV1::from(self))
    }

    /// Validates the head manifest structure and signature.
    pub fn validate(&self) -> Result<(), GovernanceDagHeadValidationError> {
        if self.version != GOVERNANCE_DAG_HEAD_VERSION_V1 {
            return Err(GovernanceDagHeadValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.head_block_cid.is_empty() {
            return Err(GovernanceDagHeadValidationError::MissingHeadBlockCid);
        }
        if self.block_count == 0 {
            return Err(GovernanceDagHeadValidationError::EmptyBlockCount);
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceDagHeadValidationError::MissingPublisherPeerId);
        }
        if self
            .checkpoint_cid
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.is_empty())
        {
            return Err(GovernanceDagHeadValidationError::InvalidCheckpointCid);
        }
        self.head_signature
            .validate()
            .map_err(|_| GovernanceDagHeadValidationError::InvalidSignature)?;
        self.verify_head_signature()
            .map_err(GovernanceDagHeadValidationError::HeadSignature)
    }

    /// Verifies the head publisher signature.
    pub fn verify_head_signature(&self) -> Result<(), GovernanceLogSignatureVerificationError> {
        let payload_bytes = self.signature_payload_bytes().map_err(|err| {
            GovernanceLogSignatureVerificationError::PayloadEncoding {
                reason: err.to_string(),
            }
        })?;
        verify_governance_signature_bytes(&self.head_signature, &payload_bytes)
    }
}

/// Signature covering a governance log node.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceLogSignatureV1 {
    /// Signature algorithm.
    pub algorithm: GovernanceSignatureAlgorithm,
    /// Publisher public key.
    pub public_key: Vec<u8>,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}

impl GovernanceLogSignatureV1 {
    fn validate(&self) -> Result<(), GovernanceLogValidationError> {
        if self.public_key.is_empty() || self.signature.is_empty() {
            return Err(GovernanceLogValidationError::InvalidSignature);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceLogSignaturePayloadV1 {
    version: u8,
    node_cid: Vec<u8>,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    payload: GovernanceLogPayloadV1,
}

impl From<&GovernanceLogNodeV1> for GovernanceLogSignaturePayloadV1 {
    fn from(node: &GovernanceLogNodeV1) -> Self {
        Self {
            version: node.version,
            node_cid: node.node_cid.clone(),
            prev_cid: node.prev_cid.clone(),
            timestamp: node.timestamp,
            publisher_peer_id: node.publisher_peer_id.clone(),
            payload: node.payload.clone(),
        }
    }
}

/// Algorithms supported for governance signatures.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum GovernanceSignatureAlgorithm {
    /// Ed25519 signature.
    Ed25519 = 1,
    /// Dilithium3 (post-quantum) signature.
    Dilithium3 = 2,
}

/// Governance log node entry appended to the DAG.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceLogNodeV1 {
    /// Schema version (`GOVERNANCE_LOG_VERSION_V1`).
    pub version: u8,
    /// CID of this node (multihash bytes).
    pub node_cid: Vec<u8>,
    /// Optional previous CID in the chain.
    #[norito(default)]
    pub prev_cid: Option<Vec<u8>>,
    /// Unix timestamp (seconds) when this node was published.
    pub timestamp: u64,
    /// Publisher peer identifier (e.g., libp2p peer ID).
    pub publisher_peer_id: Vec<u8>,
    /// Payload carried by this node.
    pub payload: GovernanceLogPayloadV1,
    /// Publisher signature covering the canonical node signing payload.
    pub publisher_signature: GovernanceLogSignatureV1,
}

impl GovernanceLogNodeV1 {
    /// Validates the log node payload.
    pub fn validate(&self) -> Result<(), GovernanceLogValidationError> {
        if self.version != GOVERNANCE_LOG_VERSION_V1 {
            return Err(GovernanceLogValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.node_cid.is_empty() {
            return Err(GovernanceLogValidationError::MissingNodeCid);
        }
        if self.prev_cid.as_ref().is_some_and(|prev| prev.is_empty()) {
            return Err(GovernanceLogValidationError::InvalidPrevCid);
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceLogValidationError::MissingPublisherPeerId);
        }
        self.publisher_signature.validate()?;
        self.payload.validate(self.timestamp)?;
        Ok(())
    }

    /// Returns canonical Norito bytes signed by the publisher.
    ///
    /// The payload deliberately excludes `publisher_signature` so signers and
    /// verifiers use stable bytes before and after the signature is attached.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(&GovernanceLogSignaturePayloadV1::from(self))
    }

    /// Recomputes this node's deterministic CID bytes.
    pub fn recompute_node_cid(&self) -> Result<Vec<u8>, norito::core::Error> {
        governance_log_node_cid_v1(
            self.prev_cid.as_deref(),
            self.timestamp,
            &self.publisher_peer_id,
            &self.payload,
        )
    }

    /// Verifies a publisher signature over the canonical node payload.
    pub fn verify_publisher_signature(
        &self,
    ) -> Result<(), GovernanceLogSignatureVerificationError> {
        let payload_bytes = self.signature_payload_bytes().map_err(|err| {
            GovernanceLogSignatureVerificationError::PayloadEncoding {
                reason: err.to_string(),
            }
        })?;

        verify_governance_signature_bytes(&self.publisher_signature, &payload_bytes)
    }
}

/// Derives deterministic Governance log node CID bytes.
pub fn governance_log_node_cid_v1(
    prev_cid: Option<&[u8]>,
    timestamp: u64,
    publisher_peer_id: &[u8],
    payload: &GovernanceLogPayloadV1,
) -> Result<Vec<u8>, norito::core::Error> {
    let payload = GovernanceLogNodeCidPayloadV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        prev_cid: prev_cid.map(<[u8]>::to_vec),
        timestamp,
        publisher_peer_id: publisher_peer_id.to_vec(),
        payload: payload.clone(),
    };
    let payload_bytes = norito::to_bytes(&payload)?;
    let mut hasher = Hasher::new();
    hasher.update(GOVERNANCE_LOG_NODE_CID_DOMAIN_V1);
    hasher.update(&payload_bytes);
    Ok(hasher.finalize().as_bytes().to_vec())
}

fn verify_governance_signature_bytes(
    publisher_signature: &GovernanceLogSignatureV1,
    payload_bytes: &[u8],
) -> Result<(), GovernanceLogSignatureVerificationError> {
    match publisher_signature.algorithm {
        GovernanceSignatureAlgorithm::Ed25519 => {
            verify_ed25519_governance_signature(publisher_signature, payload_bytes)
        }
        GovernanceSignatureAlgorithm::Dilithium3 => {
            verify_mldsa_governance_signature(publisher_signature, payload_bytes)
        }
    }
}

fn verify_ed25519_governance_signature(
    publisher_signature: &GovernanceLogSignatureV1,
    payload_bytes: &[u8],
) -> Result<(), GovernanceLogSignatureVerificationError> {
    if publisher_signature.public_key.len() != PUBLIC_KEY_LENGTH {
        return Err(
            GovernanceLogSignatureVerificationError::InvalidPublicKeyLength {
                length: publisher_signature.public_key.len(),
            },
        );
    }
    if publisher_signature.signature.len() != SIGNATURE_LENGTH {
        return Err(
            GovernanceLogSignatureVerificationError::InvalidSignatureLength {
                length: publisher_signature.signature.len(),
            },
        );
    }

    let mut public_key = [0u8; PUBLIC_KEY_LENGTH];
    public_key.copy_from_slice(&publisher_signature.public_key);
    let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|err| {
        GovernanceLogSignatureVerificationError::InvalidPublicKey {
            reason: err.to_string(),
        }
    })?;

    let mut signature = [0u8; SIGNATURE_LENGTH];
    signature.copy_from_slice(&publisher_signature.signature);
    let signature = DalekSignature::from_bytes(&signature);

    verifying_key
        .verify(payload_bytes, &signature)
        .map_err(
            |err| GovernanceLogSignatureVerificationError::Verification {
                reason: err.to_string(),
            },
        )
}

fn verify_mldsa_governance_signature(
    publisher_signature: &GovernanceLogSignatureV1,
    payload_bytes: &[u8],
) -> Result<(), GovernanceLogSignatureVerificationError> {
    let public_key = PublicKey::from_bytes(Algorithm::MlDsa, &publisher_signature.public_key)
        .map_err(
            |err| GovernanceLogSignatureVerificationError::InvalidPublicKey {
                reason: err.to_string(),
            },
        )?;
    let signature = IrohaSignature::from_bytes(&publisher_signature.signature);
    signature.verify(&public_key, payload_bytes).map_err(|err| {
        GovernanceLogSignatureVerificationError::Verification {
            reason: err.to_string(),
        }
    })
}

/// Validation errors for governance log nodes.
#[derive(Debug, Error)]
pub enum GovernanceLogValidationError {
    #[error("unsupported governance log version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("node CID must not be empty")]
    MissingNodeCid,
    #[error("previous CID must be None or non-empty")]
    InvalidPrevCid,
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("publisher signature missing key or signature bytes")]
    InvalidSignature,
    #[error("advert validation failed: {0}")]
    Advert(crate::provider_advert::AdvertValidationError),
    #[error("replication order validation failed: {0}")]
    ReplicationOrder(crate::capacity::ReplicationOrderValidationError),
    #[error("challenge validation failed: {0}")]
    PorChallenge(crate::por::PorChallengeValidationError),
    #[error("proof validation failed: {0}")]
    PorProof(crate::por::PorProofValidationError),
    #[error("audit verdict validation failed: {0}")]
    AuditVerdict(crate::por::AuditVerdictValidationError),
    #[error("deal settlement validation failed: {0}")]
    DealSettlement(crate::deal::DealSettlementValidationError),
    #[error("reputation snapshot validation failed: {0}")]
    ReputationSnapshot(crate::reputation::ReputationValidationError),
    #[error("moderation ballot event validation failed: {0}")]
    ModerationBallotEvent(SoraFsModerationBallotGovernanceEventValidationError),
    #[error("appeal finance report validation failed: {0}")]
    AppealFinanceReport(SoraFsAppealFinanceReportValidationError),
    #[error("appeal finance weekly rollup validation failed: {0}")]
    AppealFinanceWeeklyRollup(SoraFsAppealFinanceWeeklyRollupValidationError),
}

/// Validation errors for SoraFS moderation ballot governance events.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoraFsModerationBallotGovernanceEventValidationError {
    /// Event uses an unsupported schema version.
    #[error(
        "unsupported SoraFS moderation ballot governance event version `{found}` (expected {expected})"
    )]
    UnsupportedVersion {
        /// Expected schema version.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Missing moderation case identifier.
    #[error("SoraFS moderation ballot governance event case id is required")]
    MissingCaseId,
    /// Missing moderation round identifier.
    #[error("SoraFS moderation ballot governance event round id is required")]
    MissingRoundId,
    /// Missing juror identifier for a juror-scoped event.
    #[error("SoraFS moderation ballot governance event juror id is required")]
    MissingJurorId,
    /// A non-juror event unexpectedly included a juror id.
    #[error("SoraFS moderation ballot governance event must not include a juror id")]
    UnexpectedJurorId,
    /// A non-tally event unexpectedly included a tally.
    #[error("SoraFS moderation ballot governance event must not include a tally")]
    UnexpectedTally,
    /// A tally event omitted its tally payload.
    #[error("SoraFS moderation ballot governance tally is required")]
    MissingTally,
    /// Tally case id does not match the enclosing event case id.
    #[error("SoraFS moderation tally case id mismatch: event `{event}`, tally `{tally}`")]
    TallyCaseMismatch {
        /// Case id from the event.
        event: String,
        /// Case id from the tally.
        tally: String,
    },
    /// Tally round id does not match the enclosing event round id.
    #[error("SoraFS moderation tally round id mismatch: event `{event}`, tally `{tally}`")]
    TallyRoundMismatch {
        /// Round id from the event.
        event: String,
        /// Round id from the tally.
        tally: String,
    },
    /// Tally quorum must be non-zero.
    #[error("SoraFS moderation tally quorum must be nonzero")]
    InvalidQuorum,
    /// Tally counts do not add up to the advertised total.
    #[error(
        "SoraFS moderation tally vote count mismatch: counted `{counted}`, votes_total `{votes_total}`"
    )]
    VoteCountMismatch {
        /// Sum of the choice counts.
        counted: u64,
        /// Advertised vote total.
        votes_total: u32,
    },
    /// Tally did not meet quorum.
    #[error("SoraFS moderation tally quorum `{quorum}` not met by `{votes_total}` votes")]
    QuorumNotMet {
        /// Required quorum.
        quorum: u16,
        /// Advertised vote total.
        votes_total: u32,
    },
    /// Winning choice does not match the counts.
    #[error("SoraFS moderation tally winning choice does not match vote counts")]
    WinningChoiceMismatch,
    /// Contested flag does not match the winner state.
    #[error("SoraFS moderation tally contested flag does not match winner state")]
    ContestedMismatch,
}

/// Validation errors for SoraFS appeal finance reports.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoraFsAppealFinanceReportValidationError {
    /// Report uses an unsupported schema version.
    #[error("unsupported SoraFS appeal finance report version `{found}` (expected {expected})")]
    UnsupportedVersion {
        /// Expected schema version.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Missing non-zero report id.
    #[error("SoraFS appeal finance report id is required")]
    MissingReportId,
    /// Missing case id.
    #[error("SoraFS appeal finance report case id is required")]
    MissingCaseId,
    /// Missing round id when the optional field is present.
    #[error("SoraFS appeal finance report round id must not be empty")]
    MissingRoundId,
    /// Missing generated timestamp.
    #[error("SoraFS appeal finance report generated timestamp is required")]
    MissingGeneratedAt,
    /// Missing finance config version.
    #[error("SoraFS appeal finance config version is required")]
    MissingFinanceConfigVersion,
    /// Evidence bundle digest was all zeroes.
    #[error("SoraFS appeal finance evidence bundle digest must not be all zeroes")]
    InvalidEvidenceBundleDigest,
    /// Decimal amount was malformed.
    #[error("SoraFS appeal finance amount `{field}` is invalid: {reason}")]
    InvalidAmount {
        /// Field containing the invalid amount.
        field: &'static str,
        /// Human-readable validation reason.
        reason: String,
    },
    /// Missing account id for a flow.
    #[error("SoraFS appeal finance `{role}` account id is required")]
    MissingAccountId {
        /// Flow role.
        role: &'static str,
    },
    /// Panel size must be non-zero.
    #[error("SoraFS appeal finance panel size must be greater than zero")]
    InvalidPanelSize,
    /// Panel size could not be represented for reconciliation.
    #[error("SoraFS appeal finance panel size `{panel_size}` is too large")]
    PanelSizeOverflow {
        /// Declared panel size.
        panel_size: u32,
    },
    /// Missing juror id in a payout line.
    #[error("SoraFS appeal finance juror payout id is required")]
    MissingJurorId,
    /// Duplicate payout juror id.
    #[error("SoraFS appeal finance duplicate paid juror `{juror_id}`")]
    DuplicateJurorId {
        /// Duplicate juror id.
        juror_id: String,
    },
    /// Missing no-show juror id.
    #[error("SoraFS appeal finance no-show juror id is required")]
    MissingNoShowJurorId,
    /// Duplicate no-show juror id.
    #[error("SoraFS appeal finance duplicate no-show juror `{juror_id}`")]
    DuplicateNoShowJurorId {
        /// Duplicate no-show juror id.
        juror_id: String,
    },
    /// A no-show juror also received a payout.
    #[error("SoraFS appeal finance no-show juror `{juror_id}` also has a payout")]
    NoShowJurorPaid {
        /// Conflicting juror id.
        juror_id: String,
    },
    /// Paid and no-show juror lines do not reconcile to panel size.
    #[error(
        "SoraFS appeal finance panel reconciliation mismatch: panel size `{panel_size}`, accounted `{accounted}`"
    )]
    PanelReconciliation {
        /// Declared panel size.
        panel_size: u32,
        /// Number of paid plus no-show juror lines.
        accounted: usize,
    },
}

/// Errors raised while building a weekly appeal finance rollup.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoraFsAppealFinanceWeeklyRollupBuildError {
    /// Reporting cycle is invalid.
    #[error("invalid SoraFS appeal finance weekly rollup cycle: {0}")]
    InvalidCycle(#[from] crate::por::PorReportIsoWeekValidationError),
    /// Generated timestamp was missing.
    #[error("SoraFS appeal finance weekly rollup generated timestamp is required")]
    MissingGeneratedAt,
    /// At least one source report is required.
    #[error("SoraFS appeal finance weekly rollup requires at least one source report")]
    NoReports,
    /// A source report failed validation.
    #[error("SoraFS appeal finance weekly rollup source report #{index} is invalid: {source}")]
    InvalidReport {
        /// Source report index.
        index: usize,
        /// Report validation error.
        source: SoraFsAppealFinanceReportValidationError,
    },
    /// Duplicate source report id.
    #[error("SoraFS appeal finance weekly rollup duplicate report id {report_id:?}")]
    DuplicateReportId {
        /// Duplicate report id.
        report_id: [u8; 16],
    },
    /// Decimal amount could not be accumulated.
    #[error(
        "SoraFS appeal finance weekly rollup amount `{field}` is invalid for report {report_id:?}: {reason}"
    )]
    InvalidAmount {
        /// Source report id.
        report_id: [u8; 16],
        /// Amount field name.
        field: &'static str,
        /// Human-readable validation reason.
        reason: String,
    },
    /// The computed rollup failed its own validator.
    #[error("computed SoraFS appeal finance weekly rollup is invalid: {0}")]
    InvalidRollup(#[from] SoraFsAppealFinanceWeeklyRollupValidationError),
}

/// Validation errors for weekly SoraFS appeal finance rollups.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoraFsAppealFinanceWeeklyRollupValidationError {
    /// Rollup uses an unsupported schema version.
    #[error(
        "unsupported SoraFS appeal finance weekly rollup version `{found}` (expected {expected})"
    )]
    UnsupportedVersion {
        /// Expected schema version.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Reporting cycle is invalid.
    #[error("invalid SoraFS appeal finance weekly rollup cycle: {0}")]
    InvalidCycle(#[from] crate::por::PorReportIsoWeekValidationError),
    /// Generated timestamp was missing.
    #[error("SoraFS appeal finance weekly rollup generated timestamp is required")]
    MissingGeneratedAt,
    /// At least one source report is required.
    #[error("SoraFS appeal finance weekly rollup requires at least one source report")]
    NoReports,
    /// Distinct case count is inconsistent.
    #[error(
        "SoraFS appeal finance weekly rollup case count `{case_count}` is invalid for `{report_count}` reports"
    )]
    InvalidCaseCount {
        /// Distinct case count.
        case_count: u64,
        /// Source report count.
        report_count: u64,
    },
    /// Config versions list is empty.
    #[error("SoraFS appeal finance weekly rollup requires config versions")]
    MissingConfigVersions,
    /// Label was empty or whitespace.
    #[error("SoraFS appeal finance weekly rollup `{field}` contains an empty label")]
    InvalidLabel {
        /// Label field name.
        field: &'static str,
    },
    /// Labels are not sorted.
    #[error("SoraFS appeal finance weekly rollup `{field}` must be sorted")]
    UnsortedLabels {
        /// Label field name.
        field: &'static str,
    },
    /// Labels contain duplicates.
    #[error("SoraFS appeal finance weekly rollup `{field}` contains duplicates")]
    DuplicateLabel {
        /// Label field name.
        field: &'static str,
    },
    /// Decimal amount was malformed.
    #[error("SoraFS appeal finance weekly rollup amount `{field}` is invalid: {reason}")]
    InvalidAmount {
        /// Field containing the invalid amount.
        field: &'static str,
        /// Human-readable validation reason.
        reason: String,
    },
    /// Source report id is all zeroes.
    #[error("SoraFS appeal finance weekly rollup source report id is required")]
    MissingSourceReportId,
    /// Duplicate source report id.
    #[error("SoraFS appeal finance weekly rollup duplicate source report id {report_id:?}")]
    DuplicateSourceReportId {
        /// Duplicate source report id.
        report_id: [u8; 16],
    },
    /// Source report id list does not match declared report count.
    #[error(
        "SoraFS appeal finance weekly rollup source report count `{source_report_count}` does not match report count `{report_count}`"
    )]
    SourceReportCountMismatch {
        /// Declared report count.
        report_count: u64,
        /// Number of source report ids.
        source_report_count: u64,
    },
    /// Outcome rows are required.
    #[error("SoraFS appeal finance weekly rollup requires outcome rows")]
    NoOutcomes,
    /// Outcome row is empty.
    #[error("SoraFS appeal finance weekly rollup outcome `{outcome:?}` has no reports")]
    EmptyOutcome {
        /// Outcome row.
        outcome: SoraFsAppealFinanceOutcomeV1,
    },
    /// Outcome case count is inconsistent.
    #[error(
        "SoraFS appeal finance weekly rollup outcome `{outcome:?}` case count `{case_count}` is invalid for `{report_count}` reports"
    )]
    InvalidOutcomeCaseCount {
        /// Outcome row.
        outcome: SoraFsAppealFinanceOutcomeV1,
        /// Distinct case count.
        case_count: u64,
        /// Source report count.
        report_count: u64,
    },
    /// Duplicate outcome row.
    #[error("SoraFS appeal finance weekly rollup duplicate outcome `{outcome:?}`")]
    DuplicateOutcome {
        /// Duplicate outcome.
        outcome: SoraFsAppealFinanceOutcomeV1,
    },
    /// Outcome row report counts do not reconcile.
    #[error(
        "SoraFS appeal finance weekly rollup outcome report count `{outcome_report_count}` does not match report count `{report_count}`"
    )]
    OutcomeReportCountMismatch {
        /// Declared report count.
        report_count: u64,
        /// Sum of outcome report counts.
        outcome_report_count: u64,
    },
    /// Outcome row juror payout counts do not reconcile.
    #[error(
        "SoraFS appeal finance weekly rollup outcome juror payout count `{outcome_juror_payout_count}` does not match `{juror_payout_count}`"
    )]
    OutcomeJurorPayoutCountMismatch {
        /// Declared juror payout count.
        juror_payout_count: u64,
        /// Sum of outcome juror payout counts.
        outcome_juror_payout_count: u64,
    },
    /// Outcome row no-show counts do not reconcile.
    #[error(
        "SoraFS appeal finance weekly rollup outcome no-show count `{outcome_no_show_juror_count}` does not match `{no_show_juror_count}`"
    )]
    OutcomeNoShowCountMismatch {
        /// Declared no-show count.
        no_show_juror_count: u64,
        /// Sum of outcome no-show counts.
        outcome_no_show_juror_count: u64,
    },
    /// Outcome row amounts do not reconcile.
    #[error(
        "SoraFS appeal finance weekly rollup `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    OutcomeAmountMismatch {
        /// Amount field name.
        field: &'static str,
        /// Top-level value.
        expected: String,
        /// Reconciled outcome value.
        actual: String,
    },
}

/// Validation errors for public Governance DAG blocks.
#[derive(Debug, Error)]
pub enum GovernanceDagBlockValidationError {
    #[error("unsupported governance DAG block version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("block CID must not be empty")]
    MissingBlockCid,
    #[error("previous block CID must be None or non-empty")]
    InvalidPrevBlockCid,
    #[error("root governance DAG block must not carry a previous block CID")]
    RootHasParent,
    #[error("non-root governance DAG block must carry a previous block CID")]
    NonRootMissingParent,
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("block signature missing key or signature bytes")]
    InvalidSignature,
    #[error("embedded governance node validation failed: {0}")]
    Node(GovernanceLogValidationError),
    #[error("embedded governance node signature validation failed: {0}")]
    NodeSignature(GovernanceLogSignatureVerificationError),
    #[error("failed to encode governance DAG block CID payload: {reason}")]
    CidEncoding { reason: String },
    #[error("governance DAG block CID does not match the canonical block payload")]
    InvalidBlockCid,
    #[error("governance DAG block signature validation failed: {0}")]
    BlockSignature(GovernanceLogSignatureVerificationError),
}

/// Validation errors for public Governance DAG head manifests.
#[derive(Debug, Error)]
pub enum GovernanceDagHeadValidationError {
    #[error("unsupported governance DAG head version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("head block CID must not be empty")]
    MissingHeadBlockCid,
    #[error("head manifest block count must be greater than zero")]
    EmptyBlockCount,
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("checkpoint CID must be None or non-empty")]
    InvalidCheckpointCid,
    #[error("head signature missing key or signature bytes")]
    InvalidSignature,
    #[error("governance DAG head signature validation failed: {0}")]
    HeadSignature(GovernanceLogSignatureVerificationError),
}

/// Validation errors for Governance DAG block chains.
#[derive(Debug, Error)]
pub enum GovernanceDagChainValidationError {
    #[error("governance DAG chain must contain at least one block")]
    Empty,
    #[error("block at index {index} failed validation: {source}")]
    InvalidBlock {
        index: usize,
        source: GovernanceDagBlockValidationError,
    },
    #[error("duplicate governance DAG block CID at index {index}")]
    DuplicateBlockCid { index: usize },
    #[error("block at index {index} references a missing parent")]
    MissingParent { index: usize },
    #[error("block at index {index} has sequence {sequence}, expected {expected}")]
    SequenceGap {
        index: usize,
        expected: u64,
        sequence: u64,
    },
    #[error("block at index {index} has timestamp earlier than its parent")]
    TimestampRegression { index: usize },
    #[error("expected exactly one governance DAG head, found {count}")]
    HeadCount { count: usize },
    #[error("governance DAG head does not match expected CID")]
    ExpectedHeadMismatch,
}

/// Validation errors for binding a signed head manifest to a block chain.
#[derive(Debug, Error)]
pub enum GovernanceDagHeadChainValidationError {
    #[error("head manifest validation failed: {0}")]
    Head(GovernanceDagHeadValidationError),
    #[error("block chain validation failed: {0}")]
    Chain(GovernanceDagChainValidationError),
    #[error("head block count {head_count} does not match chain block count {chain_count}")]
    BlockCountMismatch { head_count: u64, chain_count: u64 },
}

/// Validates a public Governance DAG chain and optional expected head CID.
pub fn validate_governance_dag_chain_v1(
    blocks: &[GovernanceDagBlockV1],
    expected_head_cid: Option<&[u8]>,
) -> Result<(), GovernanceDagChainValidationError> {
    if blocks.is_empty() {
        return Err(GovernanceDagChainValidationError::Empty);
    }

    let mut block_by_cid = BTreeMap::<Vec<u8>, usize>::new();
    let mut referenced_parents = BTreeSet::<Vec<u8>>::new();
    for (index, block) in blocks.iter().enumerate() {
        block
            .validate()
            .map_err(|source| GovernanceDagChainValidationError::InvalidBlock { index, source })?;
        if block_by_cid
            .insert(block.block_cid.clone(), index)
            .is_some()
        {
            return Err(GovernanceDagChainValidationError::DuplicateBlockCid { index });
        }
        if let Some(prev) = &block.prev_block_cid {
            referenced_parents.insert(prev.clone());
        }
    }

    for (index, block) in blocks.iter().enumerate() {
        let Some(prev) = &block.prev_block_cid else {
            continue;
        };
        let Some(parent_index) = block_by_cid.get(prev).copied() else {
            return Err(GovernanceDagChainValidationError::MissingParent { index });
        };
        let parent = &blocks[parent_index];
        let expected = parent.sequence.saturating_add(1);
        if block.sequence != expected {
            return Err(GovernanceDagChainValidationError::SequenceGap {
                index,
                expected,
                sequence: block.sequence,
            });
        }
        if block.timestamp < parent.timestamp {
            return Err(GovernanceDagChainValidationError::TimestampRegression { index });
        }
    }

    let mut heads = Vec::<&[u8]>::new();
    for block in blocks {
        if !referenced_parents.contains(&block.block_cid) {
            heads.push(block.block_cid.as_slice());
        }
    }
    if heads.len() != 1 {
        return Err(GovernanceDagChainValidationError::HeadCount { count: heads.len() });
    }
    if let Some(expected_head_cid) = expected_head_cid
        && heads[0] != expected_head_cid
    {
        return Err(GovernanceDagChainValidationError::ExpectedHeadMismatch);
    }
    Ok(())
}

/// Validates a signed head manifest against its advertised block chain.
pub fn validate_governance_dag_head_against_chain_v1(
    head: &GovernanceDagHeadV1,
    blocks: &[GovernanceDagBlockV1],
) -> Result<(), GovernanceDagHeadChainValidationError> {
    head.validate()
        .map_err(GovernanceDagHeadChainValidationError::Head)?;
    validate_governance_dag_chain_v1(blocks, Some(&head.head_block_cid))
        .map_err(GovernanceDagHeadChainValidationError::Chain)?;
    let chain_count = u64::try_from(blocks.len()).unwrap_or(u64::MAX);
    if head.block_count != chain_count {
        return Err(GovernanceDagHeadChainValidationError::BlockCountMismatch {
            head_count: head.block_count,
            chain_count,
        });
    }
    Ok(())
}

/// Errors raised while verifying a governance log publisher signature.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GovernanceLogSignatureVerificationError {
    /// Signature algorithm is not supported by this validator.
    #[error("unsupported governance log signature algorithm: {0:?}")]
    UnsupportedAlgorithm(GovernanceSignatureAlgorithm),
    /// Ed25519 public key length is invalid.
    #[error("ed25519 governance public key must be 32 bytes, got {length}")]
    InvalidPublicKeyLength {
        /// Observed public key byte length.
        length: usize,
    },
    /// Ed25519 signature length is invalid.
    #[error("ed25519 governance signature must be 64 bytes, got {length}")]
    InvalidSignatureLength {
        /// Observed signature byte length.
        length: usize,
    },
    /// Public key bytes could not be parsed.
    #[error("invalid governance public key: {reason}")]
    InvalidPublicKey {
        /// Underlying parser diagnostic.
        reason: String,
    },
    /// Canonical signature payload could not be encoded.
    #[error("failed to encode governance log signature payload: {reason}")]
    PayloadEncoding {
        /// Underlying Norito diagnostic.
        reason: String,
    },
    /// Signature verification failed.
    #[error("governance log publisher signature verification failed: {reason}")]
    Verification {
        /// Underlying signature verification diagnostic.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};

    fn signed_por_proof_payload() -> GovernanceLogPayloadV1 {
        GovernanceLogPayloadV1::PorProof(crate::por::PorProofV1 {
            version: crate::POR_PROOF_VERSION_V1,
            challenge_id: [0x11; 32],
            manifest_digest: [0x22; 32],
            provider_id: [0x33; 32],
            samples: vec![crate::por::PorProofSampleV1 {
                sample_index: 7,
                chunk_offset: 4096,
                chunk_size: 1024,
                chunk_digest: [0x44; 32],
                leaf_digest: [0x55; 32],
            }],
            auth_path: vec![[0x66; 32]],
            signature: crate::provider_advert::AdvertSignature {
                algorithm: crate::provider_advert::SignatureAlgorithm::Ed25519,
                public_key: vec![0x77; 32],
                signature: vec![0x88; 64],
            },
            submitted_at: 1_700_000_200,
        })
    }

    fn governance_node_for_signing() -> GovernanceLogNodeV1 {
        GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: b"bafygovernancelognode".to_vec(),
            prev_cid: Some(b"bafypreviouscid".to_vec()),
            timestamp: 1_700_000_300,
            publisher_peer_id: b"12D3KooWGovernancePeer".to_vec(),
            payload: signed_por_proof_payload(),
            publisher_signature: GovernanceLogSignatureV1 {
                algorithm: GovernanceSignatureAlgorithm::Dilithium3,
                public_key: vec![0x99; 64],
                signature: vec![0xAA; 160],
            },
        }
    }

    #[test]
    fn governance_log_node_cid_is_stable_and_input_sensitive() {
        let payload = signed_por_proof_payload();
        let prev_cid = b"bafypreviouscid";
        let publisher_peer_id = b"12D3KooWGovernancePeer";
        let first = governance_log_node_cid_v1(
            Some(prev_cid.as_slice()),
            1_700_000_300,
            publisher_peer_id,
            &payload,
        )
        .expect("derive governance log node CID");
        let second = governance_log_node_cid_v1(
            Some(prev_cid.as_slice()),
            1_700_000_300,
            publisher_peer_id,
            &payload,
        )
        .expect("derive governance log node CID again");
        let changed = governance_log_node_cid_v1(
            Some(prev_cid.as_slice()),
            1_700_000_301,
            publisher_peer_id,
            &payload,
        )
        .expect("derive changed governance log node CID");

        assert_eq!(first, second);
        assert_ne!(first, changed);
        assert_eq!(first.len(), blake3::OUT_LEN);
    }

    fn sign_governance_node(node: &mut GovernanceLogNodeV1, seed: &[u8; 32]) {
        let signing_key = SigningKey::from_bytes(seed);
        let payload_bytes = node
            .signature_payload_bytes()
            .expect("encode governance signing payload");
        let signature = signing_key.sign(&payload_bytes);
        node.publisher_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
    }

    fn sign_governance_node_mldsa(node: &mut GovernanceLogNodeV1, seed: &[u8]) {
        let key_pair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::MlDsa)
            .expect("generate ML-DSA governance keypair");
        let payload_bytes = node
            .signature_payload_bytes()
            .expect("encode governance signing payload");
        let signature = IrohaSignature::try_new(key_pair.private_key(), &payload_bytes)
            .expect("sign governance payload with ML-DSA key");
        let (algorithm, public_key) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("encode ML-DSA public key");
        assert_eq!(algorithm, Algorithm::MlDsa);
        node.publisher_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Dilithium3,
            public_key: public_key.to_vec(),
            signature: signature.payload().to_vec(),
        };
    }

    fn empty_ed25519_signature() -> GovernanceLogSignatureV1 {
        GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn sign_governance_block(block: &mut GovernanceDagBlockV1, seed: &[u8; 32]) {
        let signing_key = SigningKey::from_bytes(seed);
        let payload_bytes = block
            .signature_payload_bytes()
            .expect("encode governance DAG block signing payload");
        let signature = signing_key.sign(&payload_bytes);
        block.block_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
    }

    fn sign_governance_head(head: &mut GovernanceDagHeadV1, seed: &[u8; 32]) {
        let signing_key = SigningKey::from_bytes(seed);
        let payload_bytes = head
            .signature_payload_bytes()
            .expect("encode governance DAG head signing payload");
        let signature = signing_key.sign(&payload_bytes);
        head.head_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
    }

    fn signed_governance_block(
        prev_block_cid: Option<Vec<u8>>,
        sequence: u64,
        timestamp: u64,
    ) -> GovernanceDagBlockV1 {
        let mut node = governance_node_for_signing();
        node.node_cid = format!("bafygovernancelognode{sequence}").into_bytes();
        node.prev_cid = sequence
            .checked_sub(1)
            .map(|prev| format!("bafygovernancelognode{prev}").into_bytes());
        node.timestamp = timestamp;
        sign_governance_node(&mut node, &[0xA5; 32]);

        let publisher_peer_id = b"12D3KooWGovernanceDagPublisher".to_vec();
        let block_cid = governance_dag_block_cid_v1(
            prev_block_cid.as_deref(),
            sequence,
            timestamp + 10,
            &publisher_peer_id,
            &node,
        )
        .expect("derive governance DAG block CID");
        let mut block = GovernanceDagBlockV1 {
            version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
            block_cid,
            prev_block_cid,
            sequence,
            timestamp: timestamp + 10,
            publisher_peer_id,
            node,
            block_signature: empty_ed25519_signature(),
        };
        sign_governance_block(&mut block, &[0xC7; 32]);
        block
    }

    fn signed_governance_head(blocks: &[GovernanceDagBlockV1]) -> GovernanceDagHeadV1 {
        let head_block_cid = blocks.last().expect("at least one block").block_cid.clone();
        let mut head = GovernanceDagHeadV1 {
            version: GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid,
            block_count: blocks.len() as u64,
            generated_at: 1_700_001_000,
            publisher_peer_id: b"12D3KooWGovernanceDagPublisher".to_vec(),
            checkpoint_cid: None,
            head_signature: empty_ed25519_signature(),
        };
        sign_governance_head(&mut head, &[0xD9; 32]);
        head
    }
    use crate::deal::{
        DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
        DealSettlementStatusV1, DealSettlementV1, XorAmount,
    };
    use crate::reputation::{
        REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationWeightsV1, build_reputation_snapshot,
    };

    #[test]
    fn governance_node_validation_succeeds() {
        let mut builder = crate::provider_advert::ProviderAdvertV1::builder();
        let range_capability = crate::provider_advert::ProviderCapabilityRangeV1 {
            max_chunk_span: 1_048_576,
            min_granularity: 4_096,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        };
        let _ = builder
            .profile_id("sorafs.sf1@1.0.0")
            .profile_aliases(vec![
                "sorafs.sf1@1.0.0".to_string(),
                "sorafs-sf1".to_string(),
            ])
            .provider_id([5; 32])
            .stake_pool_id([6; 32])
            .stake_amount(1_000_000)
            .availability(crate::provider_advert::AvailabilityTier::Hot)
            .max_retrieval_latency_ms(250)
            .max_concurrent_streams(32)
            .add_capability(crate::provider_advert::CapabilityTlv {
                cap_type: crate::provider_advert::CapabilityType::ToriiGateway,
                payload: Vec::new(),
            })
            .add_range_capability(range_capability)
            .expect("range capability")
            .add_endpoint(crate::provider_advert::AdvertEndpoint {
                kind: crate::provider_advert::EndpointKind::Torii,
                host_pattern: "gateway.sora".to_string(),
                metadata: Vec::new(),
            })
            .add_topic(crate::provider_advert::RendezvousTopic {
                topic: "sorafs.sf1.primary".to_string(),
                region: "global".to_string(),
            })
            .path_policy_min_guard_weight(5)
            .path_policy_max_same_asn_per_path(2)
            .path_policy_max_same_pool_per_path(1)
            .stream_budget(crate::provider_advert::StreamBudgetV1 {
                max_in_flight: 4,
                max_bytes_per_sec: 512_000,
                burst_bytes: Some(64_000),
            })
            .add_transport_hint(crate::provider_advert::TransportHintV1 {
                protocol: crate::provider_advert::TransportProtocol::ToriiHttpRange,
                priority: 0,
            })
            .issued_at(1_700_000_000)
            .ttl_secs(3_600);
        let _ = builder.signature(
            crate::provider_advert::SignatureAlgorithm::Ed25519,
            vec![9; 32],
            vec![10; 64],
        );
        let advert = builder.build().expect("valid advert");

        let node = GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: b"bafygovernancenodecid".to_vec(),
            prev_cid: Some(b"bafypreviouscid".to_vec()),
            timestamp: 1_700_000_100,
            publisher_peer_id: b"12D3KooWGovernancePeer".to_vec(),
            payload: GovernanceLogPayloadV1::ProviderAdvert(advert),
            publisher_signature: GovernanceLogSignatureV1 {
                algorithm: GovernanceSignatureAlgorithm::Dilithium3,
                public_key: vec![11; 64],
                signature: vec![12; 160],
            },
        };

        assert!(node.validate().is_ok());
    }

    #[test]
    fn governance_signature_payload_excludes_publisher_signature() {
        let node = governance_node_for_signing();
        let mut different_signature = node.clone();
        different_signature.publisher_signature.signature = vec![0xBB; 96];

        assert_eq!(
            node.signature_payload_bytes()
                .expect("encode governance signature payload"),
            different_signature
                .signature_payload_bytes()
                .expect("encode governance signature payload")
        );

        let mut different_payload = node.clone();
        different_payload.timestamp += 1;
        assert_ne!(
            node.signature_payload_bytes()
                .expect("encode governance signature payload"),
            different_payload
                .signature_payload_bytes()
                .expect("encode governance signature payload")
        );
    }

    #[test]
    fn verify_publisher_signature_accepts_ed25519_signed_node() {
        let seed = [0xA5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node(&mut node, &seed);

        node.verify_publisher_signature()
            .expect("governance node signature verifies");
    }

    #[test]
    fn verify_publisher_signature_rejects_tampered_payload() {
        let seed = [0xA5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node(&mut node, &seed);
        node.timestamp += 1;

        assert!(matches!(
            node.verify_publisher_signature(),
            Err(GovernanceLogSignatureVerificationError::Verification { .. })
        ));
    }

    #[test]
    fn verify_publisher_signature_accepts_dilithium3_signed_node() {
        let seed = [0xB5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node_mldsa(&mut node, &seed);

        node.verify_publisher_signature()
            .expect("ML-DSA governance node signature verifies");
    }

    #[test]
    fn verify_publisher_signature_rejects_tampered_dilithium3_payload() {
        let seed = [0xB5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node_mldsa(&mut node, &seed);
        node.publisher_peer_id.extend_from_slice(b"-tampered");

        assert!(matches!(
            node.verify_publisher_signature(),
            Err(GovernanceLogSignatureVerificationError::Verification { .. })
        ));
    }

    #[test]
    fn governance_dag_block_derives_cid_and_verifies_signature() {
        let block = signed_governance_block(None, 0, 1_700_000_400);

        block.validate().expect("valid governance DAG block");
        assert_eq!(
            block
                .recompute_block_cid()
                .expect("recompute governance DAG block CID"),
            block.block_cid
        );
        block
            .verify_block_signature()
            .expect("block signature verifies");
    }

    #[test]
    fn governance_dag_block_signature_payload_excludes_signature() {
        let block = signed_governance_block(None, 0, 1_700_000_400);
        let mut different_signature = block.clone();
        different_signature.block_signature.signature = vec![0xEE; 64];

        assert_eq!(
            block
                .signature_payload_bytes()
                .expect("encode block signature payload"),
            different_signature
                .signature_payload_bytes()
                .expect("encode block signature payload")
        );

        let mut different_payload = block.clone();
        different_payload.sequence = 1;
        assert_ne!(
            block
                .signature_payload_bytes()
                .expect("encode block signature payload"),
            different_payload
                .signature_payload_bytes()
                .expect("encode block signature payload")
        );
    }

    #[test]
    fn governance_dag_block_rejects_tampered_cid() {
        let mut block = signed_governance_block(None, 0, 1_700_000_400);
        block.block_cid[0] ^= 0x01;

        assert!(matches!(
            block.validate(),
            Err(GovernanceDagBlockValidationError::InvalidBlockCid)
        ));
    }

    #[test]
    fn governance_dag_chain_validates_parent_linkage_and_head() {
        let root = signed_governance_block(None, 0, 1_700_000_400);
        let child = signed_governance_block(Some(root.block_cid.clone()), 1, 1_700_000_500);
        let blocks = vec![root, child];
        let expected_head = blocks[1].block_cid.clone();

        validate_governance_dag_chain_v1(&blocks, Some(&expected_head))
            .expect("valid governance DAG chain");
    }

    #[test]
    fn governance_dag_chain_rejects_missing_parent() {
        let block = signed_governance_block(Some(vec![0xA5; 32]), 1, 1_700_000_500);

        assert!(matches!(
            validate_governance_dag_chain_v1(&[block], None),
            Err(GovernanceDagChainValidationError::MissingParent { index: 0 })
        ));
    }

    #[test]
    fn governance_dag_head_manifest_signs_and_binds_chain() {
        let root = signed_governance_block(None, 0, 1_700_000_400);
        let child = signed_governance_block(Some(root.block_cid.clone()), 1, 1_700_000_500);
        let blocks = vec![root, child];
        let head = signed_governance_head(&blocks);

        head.validate().expect("valid governance DAG head");
        head.verify_head_signature()
            .expect("head signature verifies");
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("head binds the governance DAG chain");
    }

    #[test]
    fn governance_dag_head_rejects_block_count_mismatch() {
        let root = signed_governance_block(None, 0, 1_700_000_400);
        let blocks = vec![root];
        let mut head = signed_governance_head(&blocks);
        head.block_count += 1;
        sign_governance_head(&mut head, &[0xD9; 32]);

        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, &blocks),
            Err(GovernanceDagHeadChainValidationError::BlockCountMismatch {
                head_count: 2,
                chain_count: 1
            })
        ));
    }

    #[test]
    fn governance_payload_accepts_deal_settlement() {
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: [0xAA; 32],
            provider_id: [0xBB; 32],
            client_id: [0xCC; 32],
            provider_accrual: XorAmount::from_micro(100),
            client_liability: XorAmount::from_micro(100),
            bond_locked: XorAmount::from_micro(50),
            bond_slashed: XorAmount::zero(),
            captured_at: 1_700_200_000,
        };
        let settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            deal_id: [0xAA; 32],
            ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at: 1_700_200_100,
            audit_notes: None,
        };
        let payload = GovernanceLogPayloadV1::DealSettlement(settlement);
        payload.validate(1_700_200_200).expect("valid settlement");
    }

    #[test]
    fn governance_payload_accepts_reputation_snapshot() {
        let input = ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: "provider-a".to_string(),
            metrics: ReputationProviderMetricsV1 {
                version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
                por_success_bps: 9_600,
                pdp_success_bps: 9_700,
                potr_success_bps: 9_500,
                latency_health_bps: 9_100,
                dispute_rate_bps: 0,
                token_violation_rate_bps: 0,
                repair_breach_rate_bps: 0,
            },
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        };
        let snapshot = build_reputation_snapshot(
            [0x42; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input],
            None,
        )
        .expect("reputation snapshot");
        let payload = GovernanceLogPayloadV1::ReputationSnapshot(snapshot);

        payload
            .validate(1_800_000_100)
            .expect("valid reputation snapshot");
    }

    #[test]
    fn governance_payload_accepts_moderation_ballot_event() {
        let event = SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: 6,
            kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
            generated_at_unix_ms: 1_800_000_030_000,
            case_id: "case-42".to_string(),
            round_id: "round-1".to_string(),
            juror_id: None,
            committed_count: 2,
            revealed_count: 2,
            tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
                case_id: "case-42".to_string(),
                round_id: "round-1".to_string(),
                counts: SoraFsModerationVoteCountsV1 {
                    uphold: 2,
                    overturn: 0,
                    modify: 0,
                    escalate: 0,
                },
                votes_total: 2,
                quorum: 2,
                winning_choice: Some(SoraFsModerationVoteChoiceV1::Uphold),
                contested: false,
                tallied_at_unix_ms: 1_800_000_030_000,
            }),
        };
        let payload = GovernanceLogPayloadV1::ModerationBallotEvent(event);

        payload
            .validate(1_800_000_030)
            .expect("valid moderation ballot event");
    }

    fn sample_appeal_finance_report() -> SoraFsAppealFinanceReportV1 {
        SoraFsAppealFinanceReportV1 {
            version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
            report_id: [0x42; 16],
            case_id: "case-42".to_string(),
            round_id: Some("round-1".to_string()),
            generated_at_unix_ms: 1_800_000_031_000,
            appeal_finance_config_version: "baseline-v1".to_string(),
            evidence_bundle_digest: Some([0xA7; 32]),
            outcome: SoraFsAppealFinanceOutcomeV1::Overturn,
            deposit_xor: "420".to_string(),
            refund: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "refund-account".to_string(),
                amount_xor: "420".to_string(),
            },
            treasury: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "treasury-account".to_string(),
                amount_xor: "50".to_string(),
            },
            held: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "escrow-account".to_string(),
                amount_xor: "0".to_string(),
            },
            panel_size: 3,
            panel_reward_total_xor: "85".to_string(),
            rewards_paid_total_xor: "60".to_string(),
            rewards_forfeited_treasury_xor: "25".to_string(),
            juror_payouts: vec![
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-a".to_string(),
                    stipend_xor: "25".to_string(),
                    bonus_xor: "5".to_string(),
                    total_xor: "30".to_string(),
                },
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-b".to_string(),
                    stipend_xor: "25".to_string(),
                    bonus_xor: "5".to_string(),
                    total_xor: "30".to_string(),
                },
            ],
            no_show_juror_ids: vec!["juror-c".to_string()],
        }
    }

    #[test]
    fn governance_payload_accepts_appeal_finance_report() {
        let payload = GovernanceLogPayloadV1::AppealFinanceReport(sample_appeal_finance_report());

        payload
            .validate(1_800_000_031)
            .expect("valid appeal finance report");
    }

    #[test]
    fn appeal_finance_report_rejects_panel_reconciliation_mismatch() {
        let mut report = sample_appeal_finance_report();
        report.no_show_juror_ids.clear();

        let err = report.validate().expect_err("panel mismatch rejected");
        assert_eq!(
            err,
            SoraFsAppealFinanceReportValidationError::PanelReconciliation {
                panel_size: 3,
                accounted: 2,
            }
        );
    }

    fn second_appeal_finance_report() -> SoraFsAppealFinanceReportV1 {
        SoraFsAppealFinanceReportV1 {
            version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
            report_id: [0x43; 16],
            case_id: "case-43".to_string(),
            round_id: Some("round-1".to_string()),
            generated_at_unix_ms: 1_800_000_032_000,
            appeal_finance_config_version: "baseline-v1".to_string(),
            evidence_bundle_digest: Some([0xB8; 32]),
            outcome: SoraFsAppealFinanceOutcomeV1::Uphold,
            deposit_xor: "80.25".to_string(),
            refund: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "refund-account".to_string(),
                amount_xor: "0.25".to_string(),
            },
            treasury: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "treasury-account".to_string(),
                amount_xor: "80".to_string(),
            },
            held: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "escrow-account".to_string(),
                amount_xor: "0.00".to_string(),
            },
            panel_size: 1,
            panel_reward_total_xor: "30".to_string(),
            rewards_paid_total_xor: "30".to_string(),
            rewards_forfeited_treasury_xor: "0".to_string(),
            juror_payouts: vec![SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-d".to_string(),
                stipend_xor: "25".to_string(),
                bonus_xor: "5".to_string(),
                total_xor: "30".to_string(),
            }],
            no_show_juror_ids: Vec::new(),
        }
    }

    #[test]
    fn governance_payload_accepts_appeal_finance_weekly_rollup() {
        let first = sample_appeal_finance_report();
        let second = second_appeal_finance_report();
        let cycle = PorReportIsoWeek {
            year: 2026,
            week: 26,
        };

        let rollup = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
            cycle,
            1_800_000_100_000,
            &[second.clone(), first.clone()],
        )
        .expect("weekly rollup");

        assert_eq!(rollup.report_count, 2);
        assert_eq!(rollup.case_count, 2);
        assert_eq!(
            rollup.appeal_finance_config_versions,
            vec!["baseline-v1".to_string()]
        );
        assert_eq!(rollup.total_deposit_xor, "500.25");
        assert_eq!(rollup.total_refund_xor, "420.25");
        assert_eq!(rollup.total_treasury_xor, "130");
        assert_eq!(rollup.total_held_xor, "0");
        assert_eq!(rollup.total_panel_reward_xor, "115");
        assert_eq!(rollup.total_rewards_paid_xor, "90");
        assert_eq!(rollup.total_rewards_forfeited_treasury_xor, "25");
        assert_eq!(rollup.juror_payout_count, 3);
        assert_eq!(rollup.no_show_juror_count, 1);
        assert_eq!(
            rollup.source_report_ids,
            vec![first.report_id, second.report_id]
        );
        assert_eq!(rollup.outcomes.len(), 2);
        assert_eq!(
            rollup.outcomes[0].outcome,
            SoraFsAppealFinanceOutcomeV1::Uphold
        );
        assert_eq!(
            rollup.outcomes[1].outcome,
            SoraFsAppealFinanceOutcomeV1::Overturn
        );

        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(rollup)
            .validate(1_800_000_100)
            .expect("weekly rollup payload validates");
    }

    #[test]
    fn appeal_finance_weekly_rollup_rejects_duplicate_report_ids() {
        let report = sample_appeal_finance_report();
        let cycle = PorReportIsoWeek {
            year: 2026,
            week: 26,
        };

        let err = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
            cycle,
            1_800_000_100_000,
            &[report.clone(), report.clone()],
        )
        .expect_err("duplicate report rejected");

        assert_eq!(
            err,
            SoraFsAppealFinanceWeeklyRollupBuildError::DuplicateReportId {
                report_id: report.report_id,
            }
        );
    }

    #[test]
    fn moderation_ballot_event_rejects_inconsistent_tally() {
        let event = SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: 6,
            kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
            generated_at_unix_ms: 1_800_000_030_000,
            case_id: "case-42".to_string(),
            round_id: "round-1".to_string(),
            juror_id: None,
            committed_count: 2,
            revealed_count: 2,
            tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
                case_id: "case-42".to_string(),
                round_id: "round-2".to_string(),
                counts: SoraFsModerationVoteCountsV1 {
                    uphold: 1,
                    overturn: 1,
                    modify: 0,
                    escalate: 0,
                },
                votes_total: 2,
                quorum: 2,
                winning_choice: Some(SoraFsModerationVoteChoiceV1::Uphold),
                contested: false,
                tallied_at_unix_ms: 1_800_000_030_000,
            }),
        };

        assert!(matches!(
            event.validate(),
            Err(SoraFsModerationBallotGovernanceEventValidationError::TallyRoundMismatch { .. })
        ));
    }
}
