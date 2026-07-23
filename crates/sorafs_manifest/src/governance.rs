//! Governance DAG node schemas used for audit publishing.

use std::collections::{BTreeMap, BTreeSet};

use blake3::Hasher;
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use iroha_crypto::{Algorithm, PublicKey};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use soranet_pq::MlDsaSuite;
use thiserror::Error;

use crate::{
    capacity::ReplicationOrderV1,
    deal::{DealSettlementV1, XorQuantity},
    orderbook::SettlementReceiptV1,
    pdp::{PdpGovernanceArchiveV1, PdpGovernanceArchiveValidationError},
    por::{AuditVerdictV1, PorChallengeV1, PorProofV1, PorReportIsoWeek},
    reconciliation::{SORAFS_RECONCILIATION_REPORT_VERSION_V1, SorafsReconciliationReportV1},
    repair::{
        GC_AUDIT_EVENT_VERSION_V1, GcAuditEventV1, REPAIR_AUDIT_EVENT_VERSION_V1,
        REPAIR_SLASH_PROPOSAL_VERSION_V1, RepairAuditEventV1, RepairSlashProposalV1,
    },
    reputation::signed::{SignedReputationSnapshotError, SignedReputationSnapshotV1},
    transparency::{
        MODERATION_LEDGER_PUBLICATION_VERSION_V1, ModerationLedgerCyclePublicationV1,
        PROOF_TOKEN_ISSUANCE_VERSION_V1, ProofTokenIssuanceV1,
    },
};

/// Current governance log schema version.
pub const GOVERNANCE_LOG_VERSION_V1: u8 = 1;

/// Current public Governance DAG block schema version.
pub const GOVERNANCE_DAG_BLOCK_VERSION_V1: u8 = 1;

/// Current public Governance DAG head manifest schema version.
pub const GOVERNANCE_DAG_HEAD_VERSION_V1: u8 = 1;

/// Exact byte length of every first-release Governance DAG CID.
pub const GOVERNANCE_DAG_CID_BYTES_V1: usize = blake3::OUT_LEN;

/// Maximum byte length of a first-release Governance DAG publisher peer ID.
pub const GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1: usize = 128;

/// Number of newest blocks committed by a checkpointed first-release head.
pub const GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1: usize = 64;

/// Current moderation ballot governance event schema version.
pub const SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1: u16 = 1;

/// Current SoraFS appeal finance report schema version.
pub const SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1: u16 = 1;

/// Current SoraFS appeal finance weekly rollup schema version.
pub const SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1: u16 = 1;

/// Current SoraFS appeal finance settlement receipt schema version.
pub const SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1: u16 = 1;

/// Current generic external Governance DAG payload schema version.
pub const SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_VERSION_V1: u16 = 1;

/// Maximum canonical bytes embedded in one first-release external payload.
pub const SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_MAX_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum public metadata rows on one first-release external payload.
pub const SORAFS_GOVERNANCE_EXTERNAL_METADATA_MAX_ENTRIES_V1: usize = 16;
/// Maximum UTF-8 byte length of an external metadata key.
pub const SORAFS_GOVERNANCE_EXTERNAL_METADATA_KEY_MAX_BYTES_V1: usize = 64;
/// Maximum UTF-8 byte length of an external metadata value.
pub const SORAFS_GOVERNANCE_EXTERNAL_METADATA_VALUE_MAX_BYTES_V1: usize = 2_048;
/// Maximum cumulative UTF-8 bytes across external metadata keys and values.
pub const SORAFS_GOVERNANCE_EXTERNAL_METADATA_TOTAL_MAX_BYTES_V1: usize = 16 * 1_024;

/// External payload kind for repair audit envelopes.
pub const GOVERNANCE_EXTERNAL_KIND_REPAIR_AUDIT_V1: &str = "repair_audit";
/// External payload kind for repair slash proposals.
pub const GOVERNANCE_EXTERNAL_KIND_REPAIR_SLASH_V1: &str = "repair_slash";
/// External payload kind for GC audit envelopes.
pub const GOVERNANCE_EXTERNAL_KIND_GC_AUDIT_V1: &str = "gc_audit";
/// External payload kind for reconciliation reports.
pub const GOVERNANCE_EXTERNAL_KIND_RECONCILIATION_V1: &str = "reconciliation";
/// External payload kind for transparency ledger publications.
pub const GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1: &str =
    "transparency_ledger_publication";
/// External payload kind for proof-token issuance records.
pub const GOVERNANCE_EXTERNAL_KIND_PROOF_TOKEN_ISSUANCE_V1: &str = "proof_token_issuance";

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
    /// Ballot challenge accepted during the post-commit dispute buffer.
    ChallengeSubmitted,
    /// Ballot challenge was resolved before reveal progress.
    ChallengeResolved,
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
            Self::ChallengeSubmitted => "challenge_submitted",
            Self::ChallengeResolved => "challenge_resolved",
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

/// Governance DAG moderation ballot challenge category.
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
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SoraFsModerationBallotGovernanceChallengeKindV1 {
    /// The announced panel roster or roster hash is disputed.
    RosterMismatch,
    /// A duplicate commitment attempt or duplicate-commit evidence is disputed.
    DuplicateCommit,
    /// A commitment or reveal is alleged to be bound to the wrong payload.
    PayloadMismatch,
    /// A juror eligibility or authority assertion is disputed.
    JurorEligibility,
    /// The evidence bundle or policy binding is disputed.
    EvidenceMismatch,
    /// Operator-reviewed challenge category outside the fixed labels.
    Other,
}

impl SoraFsModerationBallotGovernanceChallengeKindV1 {
    /// Stable label used in local indexes and JSON sidecars.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RosterMismatch => "roster_mismatch",
            Self::DuplicateCommit => "duplicate_commit",
            Self::PayloadMismatch => "payload_mismatch",
            Self::JurorEligibility => "juror_eligibility",
            Self::EvidenceMismatch => "evidence_mismatch",
            Self::Other => "other",
        }
    }

    const fn requires_target_juror(self) -> bool {
        matches!(
            self,
            Self::DuplicateCommit | Self::PayloadMismatch | Self::JurorEligibility
        )
    }
}

/// Governance DAG moderation ballot challenge decision.
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
#[norito(tag = "decision", content = "value", rename_all = "snake_case")]
pub enum SoraFsModerationBallotGovernanceChallengeDecisionV1 {
    /// The challenge was rejected and the ballot may continue.
    Rejected,
    /// The challenge was accepted and higher-level dispute handling must resolve the ballot.
    Accepted,
}

impl SoraFsModerationBallotGovernanceChallengeDecisionV1 {
    /// Stable label used in local indexes and JSON sidecars.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rejected => "rejected",
            Self::Accepted => "accepted",
        }
    }
}

/// Payload-free challenge record carried by moderation ballot Governance DAG events.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsModerationBallotGovernanceChallengeV1 {
    /// Challenge id unique within the ballot.
    pub challenge_id: String,
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Canonical account or service identifier raising the challenge.
    pub challenger_id: String,
    /// Challenge category.
    pub kind: SoraFsModerationBallotGovernanceChallengeKindV1,
    /// Juror targeted by the challenge, when any.
    #[norito(default)]
    pub target_juror_id: Option<String>,
    /// Digest of the payload-free challenge evidence packet.
    pub evidence_digest: [u8; 32],
    /// Payload-free operator-readable reason label.
    pub reason: String,
    /// UTC timestamp (milliseconds) when the challenge was raised.
    pub raised_at_unix_ms: u64,
    /// Resolution decision, when reviewed.
    #[norito(default)]
    pub decision: Option<SoraFsModerationBallotGovernanceChallengeDecisionV1>,
    /// Canonical account or service identifier that resolved the challenge.
    #[norito(default)]
    pub resolved_by: Option<String>,
    /// UTC timestamp (milliseconds) when the challenge was resolved.
    #[norito(default)]
    pub resolved_at_unix_ms: Option<u64>,
    /// Optional payload-free resolution note.
    #[norito(default)]
    pub resolution_note: Option<String>,
}

impl SoraFsModerationBallotGovernanceChallengeV1 {
    fn validate(
        &self,
        event_kind: SoraFsModerationBallotGovernanceEventKindV1,
        event_case_id: &str,
        event_round_id: &str,
    ) -> Result<(), SoraFsModerationBallotGovernanceEventValidationError> {
        validate_non_empty_governance_label(
            &self.challenge_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeId,
        )?;
        validate_non_empty_governance_label(
            &self.case_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingCaseId,
        )?;
        validate_non_empty_governance_label(
            &self.round_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingRoundId,
        )?;
        validate_non_empty_governance_label(
            &self.challenger_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingChallengerId,
        )?;
        if self.case_id != event_case_id {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::ChallengeCaseMismatch {
                    event: event_case_id.to_owned(),
                    challenge: self.case_id.clone(),
                },
            );
        }
        if self.round_id != event_round_id {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::ChallengeRoundMismatch {
                    event: event_round_id.to_owned(),
                    challenge: self.round_id.clone(),
                },
            );
        }
        if self.evidence_digest.iter().all(|byte| *byte == 0) {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::InvalidChallengeEvidence,
            );
        }
        validate_non_empty_governance_label(
            &self.reason,
            SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeReason,
        )?;
        if let Some(target) = self.target_juror_id.as_deref() {
            validate_non_empty_governance_label(
                target,
                SoraFsModerationBallotGovernanceEventValidationError::BlankChallengeTarget,
            )?;
        } else if self.kind.requires_target_juror() {
            return Err(
                SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeTarget,
            );
        }

        match event_kind {
            SoraFsModerationBallotGovernanceEventKindV1::ChallengeSubmitted => {
                if self.decision.is_some()
                    || self.resolved_by.is_some()
                    || self.resolved_at_unix_ms.is_some()
                    || self.resolution_note.is_some()
                {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::UnexpectedChallengeResolution,
                    );
                }
            }
            SoraFsModerationBallotGovernanceEventKindV1::ChallengeResolved => {
                if self.decision.is_none() {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeDecision,
                    );
                }
                let Some(resolved_by) = self.resolved_by.as_deref() else {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeResolver,
                    );
                };
                validate_non_empty_governance_label(
                    resolved_by,
                    SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeResolver,
                )?;
                let Some(resolved_at) = self.resolved_at_unix_ms else {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeResolvedAt,
                    );
                };
                if resolved_at < self.raised_at_unix_ms {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::InvalidChallengeResolutionTimestamp,
                    );
                }
                if self
                    .resolution_note
                    .as_deref()
                    .is_some_and(|note| note.trim().is_empty())
                {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::BlankChallengeResolutionNote,
                    );
                }
            }
            _ => {}
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
    /// Local challenge count after the event.
    pub challenge_count: u64,
    /// Final tally for `BallotTallied` events.
    #[norito(default)]
    pub tally: Option<SoraFsModerationBallotGovernanceTallyV1>,
    /// Challenge record for challenge submit/resolve events.
    #[norito(default)]
    pub challenge: Option<SoraFsModerationBallotGovernanceChallengeV1>,
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
                if self.challenge.is_some() {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::UnexpectedChallenge,
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
                if self.challenge.is_some() {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::UnexpectedChallenge,
                    );
                }
            }
            SoraFsModerationBallotGovernanceEventKindV1::ChallengeSubmitted
            | SoraFsModerationBallotGovernanceEventKindV1::ChallengeResolved => {
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
                if self.challenge_count == 0 {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::InvalidChallengeCount,
                    );
                }
                let Some(challenge) = self.challenge.as_ref() else {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::MissingChallenge,
                    );
                };
                challenge.validate(self.kind, &self.case_id, &self.round_id)?;
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
                if self.challenge.is_some() {
                    return Err(
                        SoraFsModerationBallotGovernanceEventValidationError::UnexpectedChallenge,
                    );
                }
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
    pub amount_xor: XorQuantity,
}

impl SoraFsAppealFinanceAccountFlowV1 {
    fn validate(&self, role: &'static str) -> Result<(), SoraFsAppealFinanceReportValidationError> {
        validate_non_empty_appeal_finance_label(
            &self.account_id,
            SoraFsAppealFinanceReportValidationError::MissingAccountId { role },
        )?;
        Ok(())
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
    pub stipend_xor: XorQuantity,
    /// Exact non-negative bonus XOR decimal amount.
    pub bonus_xor: XorQuantity,
    /// Exact non-negative total XOR decimal amount.
    pub total_xor: XorQuantity,
}

impl SoraFsAppealFinanceJurorPayoutV1 {
    fn validate(&self) -> Result<(), SoraFsAppealFinanceReportValidationError> {
        validate_non_empty_appeal_finance_label(
            &self.juror_id,
            SoraFsAppealFinanceReportValidationError::MissingJurorId,
        )?;
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
    pub deposit_xor: XorQuantity,
    /// Refund transfer line.
    pub refund: SoraFsAppealFinanceAccountFlowV1,
    /// Treasury transfer line, including slashed deposit and forfeited rewards.
    pub treasury: SoraFsAppealFinanceAccountFlowV1,
    /// Held-escrow line.
    pub held: SoraFsAppealFinanceAccountFlowV1,
    /// Declared panel size.
    pub panel_size: u32,
    /// Exact non-negative total panel reward budget.
    pub panel_reward_total_xor: XorQuantity,
    /// Exact non-negative paid panel reward total.
    pub rewards_paid_total_xor: XorQuantity,
    /// Exact non-negative rewards forfeited to treasury.
    pub rewards_forfeited_treasury_xor: XorQuantity,
    /// Juror payout lines for attending jurors.
    pub juror_payouts: Vec<SoraFsAppealFinanceJurorPayoutV1>,
    /// Canonical juror account ids that forfeited payout by no-show.
    #[norito(default)]
    pub no_show_juror_ids: Vec<String>,
}

/// Governance DAG receipt for a server-submitted appeal finance settlement step.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsAppealFinanceSettlementReceiptV1 {
    /// Schema version (`SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1`).
    pub version: u16,
    /// Stable receipt identifier derived from the submitted transaction context.
    pub receipt_id: [u8; 16],
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Optional moderation ballot round identifier.
    #[norito(default)]
    pub round_id: Option<String>,
    /// UTC timestamp (milliseconds) when the settlement transaction was queued.
    pub generated_at_unix_ms: u64,
    /// Appeal finance config version used to derive the plan.
    pub appeal_finance_config_version: String,
    /// Final appeal outcome used by settlement calculation.
    pub outcome: SoraFsAppealFinanceOutcomeV1,
    /// Canonical escrow id as lowercase hexadecimal.
    pub escrow_id_hex: String,
    /// Account that funded the deposit.
    pub payer_account: String,
    /// Account holding the locked asset.
    pub destination_account: String,
    /// Optional authority allowed to draw down non-refund funds.
    #[norito(default)]
    pub release_authority_account: Option<String>,
    /// Submitted settlement step label.
    pub submitted_step: String,
    /// Required transaction authority for the submitted step.
    pub required_authority: String,
    /// Exact XOR amount affected by the submitted step.
    pub amount_xor: XorQuantity,
    /// Queued transaction hash as lowercase hexadecimal.
    pub tx_hash_hex: String,
    /// Digest of the reconciliation snapshot that justified submission.
    pub reconciliation_digest_hex: String,
    /// Reconciliation status before this step was queued.
    pub reconciliation_status: String,
    /// Ledger lifecycle status observed before this step was queued.
    pub observed_lifecycle_status: String,
    /// Ledger remaining amount observed before this step was queued.
    pub observed_remaining_xor: XorQuantity,
    /// Exact deposited XOR amount.
    pub deposit_xor: XorQuantity,
    /// Exact refund XOR amount expected by the settlement plan.
    pub refund_xor: XorQuantity,
    /// Exact treasury XOR amount expected by the settlement plan.
    pub treasury_xor: XorQuantity,
    /// Exact held XOR amount expected by the settlement plan.
    pub held_xor: XorQuantity,
    /// Declared panel size.
    pub panel_size: u32,
    /// Number of configured submitter signers available on this node.
    pub configured_signer_count: u32,
}

impl SoraFsAppealFinanceSettlementReceiptV1 {
    /// Validate structural invariants for a settlement submission receipt.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsAppealFinanceSettlementReceiptValidationError`] when
    /// required identifiers are missing, digest fields are malformed, or
    /// decimal amounts are not canonical.
    pub fn validate(&self) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
        if self.version != SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1 {
            return Err(
                SoraFsAppealFinanceSettlementReceiptValidationError::UnsupportedVersion {
                    expected: SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
                    found: self.version,
                },
            );
        }
        if self.receipt_id == [0u8; 16] {
            return Err(SoraFsAppealFinanceSettlementReceiptValidationError::MissingReceiptId);
        }
        validate_non_empty_settlement_receipt_label(
            &self.case_id,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingCaseId,
        )?;
        if let Some(round_id) = self.round_id.as_deref() {
            validate_non_empty_settlement_receipt_label(
                round_id,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingRoundId,
            )?;
        }
        if self.generated_at_unix_ms == 0 {
            return Err(SoraFsAppealFinanceSettlementReceiptValidationError::MissingGeneratedAt);
        }
        validate_non_empty_settlement_receipt_label(
            &self.appeal_finance_config_version,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingFinanceConfigVersion,
        )?;
        validate_receipt_hex(
            &self.escrow_id_hex,
            "escrow_id_hex",
            32,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingEscrowId,
        )?;
        for (field, value, error) in [
            (
                "payer_account",
                &self.payer_account,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingPayerAccount,
            ),
            (
                "destination_account",
                &self.destination_account,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingDestinationAccount,
            ),
            (
                "submitted_step",
                &self.submitted_step,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingSubmittedStep,
            ),
            (
                "required_authority",
                &self.required_authority,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingRequiredAuthority,
            ),
            (
                "reconciliation_status",
                &self.reconciliation_status,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingReconciliationStatus,
            ),
            (
                "observed_lifecycle_status",
                &self.observed_lifecycle_status,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingObservedLifecycleStatus,
            ),
        ] {
            validate_non_empty_settlement_receipt_label(value, error)?;
            if value.trim() != value {
                return Err(
                    SoraFsAppealFinanceSettlementReceiptValidationError::InvalidLabel { field },
                );
            }
        }
        if let Some(account) = self.release_authority_account.as_deref() {
            validate_non_empty_settlement_receipt_label(
                account,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingReleaseAuthorityAccount,
            )?;
            if account.trim() != account {
                return Err(
                    SoraFsAppealFinanceSettlementReceiptValidationError::InvalidLabel {
                        field: "release_authority_account",
                    },
                );
            }
        }
        validate_receipt_hex(
            &self.tx_hash_hex,
            "tx_hash_hex",
            32,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingTxHash,
        )?;
        validate_receipt_hex(
            &self.reconciliation_digest_hex,
            "reconciliation_digest_hex",
            32,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingReconciliationDigest,
        )?;
        if self.panel_size == 0 {
            return Err(SoraFsAppealFinanceSettlementReceiptValidationError::InvalidPanelSize);
        }
        if self.configured_signer_count == 0 {
            return Err(
                SoraFsAppealFinanceSettlementReceiptValidationError::InvalidConfiguredSignerCount,
            );
        }
        Ok(())
    }
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
        self.refund.validate("refund")?;
        self.treasury.validate("treasury")?;
        self.held.validate("held")?;
        if self.panel_size == 0 {
            return Err(SoraFsAppealFinanceReportValidationError::InvalidPanelSize);
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
    pub total_deposit_xor: XorQuantity,
    /// Total refunded XOR across source reports.
    pub total_refund_xor: XorQuantity,
    /// Total treasury-bound XOR across source reports.
    pub total_treasury_xor: XorQuantity,
    /// Total held escrow XOR across source reports.
    pub total_held_xor: XorQuantity,
    /// Total panel reward budget across source reports.
    pub total_panel_reward_xor: XorQuantity,
    /// Total panel rewards paid across source reports.
    pub total_rewards_paid_xor: XorQuantity,
    /// Total forfeited rewards sent to treasury across source reports.
    pub total_rewards_forfeited_treasury_xor: XorQuantity,
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
    pub total_deposit_xor: XorQuantity,
    /// Total refunded XOR across source reports.
    pub total_refund_xor: XorQuantity,
    /// Total treasury-bound XOR across source reports.
    pub total_treasury_xor: XorQuantity,
    /// Total held escrow XOR across source reports.
    pub total_held_xor: XorQuantity,
    /// Total panel reward budget across source reports.
    pub total_panel_reward_xor: XorQuantity,
    /// Total panel rewards paid across source reports.
    pub total_rewards_paid_xor: XorQuantity,
    /// Total forfeited rewards sent to treasury across source reports.
    pub total_rewards_forfeited_treasury_xor: XorQuantity,
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
    /// identifiers are missing, outcome accumulation overflows, or top-level
    /// totals do not reconcile with the outcome rows.
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
    total_deposit_xor: XorQuantity,
    total_refund_xor: XorQuantity,
    total_treasury_xor: XorQuantity,
    total_held_xor: XorQuantity,
    total_panel_reward_xor: XorQuantity,
    total_rewards_paid_xor: XorQuantity,
    total_rewards_forfeited_treasury_xor: XorQuantity,
    report_count: u64,
    juror_payout_count: u64,
    no_show_juror_count: u64,
}

impl AppealFinanceRollupAccumulator {
    fn new() -> Self {
        Self {
            total_deposit_xor: XorQuantity::zero(),
            total_refund_xor: XorQuantity::zero(),
            total_treasury_xor: XorQuantity::zero(),
            total_held_xor: XorQuantity::zero(),
            total_panel_reward_xor: XorQuantity::zero(),
            total_rewards_paid_xor: XorQuantity::zero(),
            total_rewards_forfeited_treasury_xor: XorQuantity::zero(),
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
            if expected != actual {
                return Err(
                    SoraFsAppealFinanceWeeklyRollupValidationError::OutcomeAmountMismatch {
                        field,
                        expected: expected.to_string(),
                        actual: actual.to_string(),
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
    lhs: &XorQuantity,
    rhs: &XorQuantity,
) -> Result<XorQuantity, SoraFsAppealFinanceWeeklyRollupBuildError> {
    lhs.checked_add(rhs).map_err(
        |_| SoraFsAppealFinanceWeeklyRollupBuildError::AmountOverflow {
            report_id: report.report_id,
            field,
        },
    )
}

fn add_rollup_amount(
    field: &'static str,
    lhs: &XorQuantity,
    rhs: &XorQuantity,
) -> Result<XorQuantity, SoraFsAppealFinanceWeeklyRollupValidationError> {
    lhs.checked_add(rhs)
        .map_err(|_| SoraFsAppealFinanceWeeklyRollupValidationError::AmountOverflow { field })
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

/// Publication stage bound to an external repair slash proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceExternalRepairSlashStageV1 {
    /// Proposal is durable locally but has not been submitted to governance.
    Drafted,
    /// Proposal has been submitted to governance.
    Submitted,
}

impl GovernanceExternalRepairSlashStageV1 {
    /// Stable metadata label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Drafted => "drafted",
            Self::Submitted => "submitted",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "drafted" => Some(Self::Drafted),
            "submitted" => Some(Self::Submitted),
            _ => None,
        }
    }
}

/// Public metadata attached to an external Governance DAG payload.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GovernanceExternalPayloadMetadataV1 {
    /// Sorted metadata key.
    pub key: String,
    /// Public metadata value.
    pub value: String,
}

/// Canonical external payload bytes signed into the Governance DAG.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GovernanceExternalPayloadV1 {
    /// External payload wrapper schema version.
    pub version: u16,
    /// Stable public payload kind label.
    pub payload_kind: String,
    /// Schema version of the embedded canonical payload.
    pub payload_version: u16,
    /// BLAKE3 digest of `encoded_payload`.
    pub encoded_blake3: [u8; 32],
    /// Byte length of `encoded_payload`.
    pub encoded_len: u64,
    /// Canonical Norito payload bytes.
    pub encoded_payload: Vec<u8>,
    /// Sorted public metadata about the payload.
    pub metadata: Vec<GovernanceExternalPayloadMetadataV1>,
}

impl GovernanceExternalPayloadV1 {
    /// Build a canonical repair-audit external payload wrapper.
    pub fn from_repair_audit(
        event: &RepairAuditEventV1,
        encoded: &[u8],
    ) -> Result<Self, GovernanceExternalPayloadValidationError> {
        Self::build(
            GOVERNANCE_EXTERNAL_KIND_REPAIR_AUDIT_V1,
            u16::from(REPAIR_AUDIT_EVENT_VERSION_V1),
            encoded,
            repair_audit_external_metadata(event),
        )
    }

    /// Build a canonical repair-slash external payload wrapper.
    pub fn from_repair_slash(
        proposal: &RepairSlashProposalV1,
        stage: GovernanceExternalRepairSlashStageV1,
        encoded: &[u8],
    ) -> Result<Self, GovernanceExternalPayloadValidationError> {
        Self::build(
            GOVERNANCE_EXTERNAL_KIND_REPAIR_SLASH_V1,
            u16::from(REPAIR_SLASH_PROPOSAL_VERSION_V1),
            encoded,
            repair_slash_external_metadata(proposal, stage),
        )
    }

    /// Build a canonical GC-audit external payload wrapper.
    pub fn from_gc_audit(
        event: &GcAuditEventV1,
        encoded: &[u8],
    ) -> Result<Self, GovernanceExternalPayloadValidationError> {
        Self::build(
            GOVERNANCE_EXTERNAL_KIND_GC_AUDIT_V1,
            u16::from(GC_AUDIT_EVENT_VERSION_V1),
            encoded,
            gc_audit_external_metadata(event),
        )
    }

    /// Build a canonical reconciliation external payload wrapper.
    pub fn from_reconciliation(
        report: &SorafsReconciliationReportV1,
        encoded: &[u8],
    ) -> Result<Self, GovernanceExternalPayloadValidationError> {
        Self::build(
            GOVERNANCE_EXTERNAL_KIND_RECONCILIATION_V1,
            u16::from(SORAFS_RECONCILIATION_REPORT_VERSION_V1),
            encoded,
            reconciliation_external_metadata(report),
        )
    }

    /// Build a canonical transparency-publication external payload wrapper.
    pub fn from_transparency_ledger_publication(
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
    ) -> Result<Self, GovernanceExternalPayloadValidationError> {
        let metadata = transparency_publication_external_metadata(publication)?;
        Self::build(
            GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1,
            MODERATION_LEDGER_PUBLICATION_VERSION_V1,
            encoded,
            metadata,
        )
    }

    /// Build a canonical proof-token issuance external payload wrapper.
    pub fn from_proof_token_issuance(
        issuance: &ProofTokenIssuanceV1,
        encoded: &[u8],
    ) -> Result<Self, GovernanceExternalPayloadValidationError> {
        Self::build(
            GOVERNANCE_EXTERNAL_KIND_PROOF_TOKEN_ISSUANCE_V1,
            PROOF_TOKEN_ISSUANCE_VERSION_V1,
            encoded,
            proof_token_external_metadata(issuance),
        )
    }

    fn build(
        payload_kind: &str,
        payload_version: u16,
        encoded: &[u8],
        metadata: Vec<GovernanceExternalPayloadMetadataV1>,
    ) -> Result<Self, GovernanceExternalPayloadValidationError> {
        let payload = Self {
            version: SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_VERSION_V1,
            payload_kind: payload_kind.to_owned(),
            payload_version,
            encoded_blake3: *blake3::hash(encoded).as_bytes(),
            encoded_len: u64::try_from(encoded.len()).unwrap_or(u64::MAX),
            encoded_payload: encoded.to_vec(),
            metadata,
        };
        payload.validate()?;
        Ok(payload)
    }

    /// Validate the external payload wrapper and embedded byte commitment.
    ///
    /// # Errors
    ///
    /// Returns [`GovernanceExternalPayloadValidationError`] when the wrapper
    /// version, labels, length, digest, or metadata ordering are invalid.
    pub fn validate(&self) -> Result<(), GovernanceExternalPayloadValidationError> {
        if self.version != SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_VERSION_V1 {
            return Err(
                GovernanceExternalPayloadValidationError::UnsupportedVersion {
                    expected: SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_VERSION_V1,
                    found: self.version,
                },
            );
        }
        validate_external_payload_label(&self.payload_kind).map_err(|_| {
            GovernanceExternalPayloadValidationError::InvalidPayloadKind {
                payload_kind: self.payload_kind.clone(),
            }
        })?;
        if self.encoded_payload.is_empty() {
            return Err(GovernanceExternalPayloadValidationError::MissingEncodedPayload);
        }
        if self.encoded_payload.len() > SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_MAX_BYTES_V1 {
            return Err(
                GovernanceExternalPayloadValidationError::EncodedPayloadTooLarge {
                    length: self.encoded_payload.len(),
                    max: SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_MAX_BYTES_V1,
                },
            );
        }
        let actual_len = self.encoded_payload.len() as u64;
        if self.encoded_len != actual_len {
            return Err(
                GovernanceExternalPayloadValidationError::EncodedLengthMismatch {
                    declared: self.encoded_len,
                    actual: actual_len,
                },
            );
        }
        if self.encoded_blake3 != *blake3::hash(&self.encoded_payload).as_bytes() {
            return Err(GovernanceExternalPayloadValidationError::EncodedDigestMismatch);
        }
        if self.metadata.len() > SORAFS_GOVERNANCE_EXTERNAL_METADATA_MAX_ENTRIES_V1 {
            return Err(
                GovernanceExternalPayloadValidationError::MetadataCountTooLarge {
                    count: self.metadata.len(),
                    max: SORAFS_GOVERNANCE_EXTERNAL_METADATA_MAX_ENTRIES_V1,
                },
            );
        }
        let mut last_key: Option<&str> = None;
        let mut seen_keys = BTreeSet::new();
        let mut metadata_bytes = 0usize;
        for item in &self.metadata {
            validate_external_payload_label(&item.key).map_err(|_| {
                GovernanceExternalPayloadValidationError::InvalidMetadataKey {
                    key: item.key.clone(),
                }
            })?;
            if item.key.len() > SORAFS_GOVERNANCE_EXTERNAL_METADATA_KEY_MAX_BYTES_V1 {
                return Err(
                    GovernanceExternalPayloadValidationError::MetadataKeyTooLong {
                        key: item.key.clone(),
                        length: item.key.len(),
                        max: SORAFS_GOVERNANCE_EXTERNAL_METADATA_KEY_MAX_BYTES_V1,
                    },
                );
            }
            validate_external_payload_text(&item.value).map_err(|_| {
                GovernanceExternalPayloadValidationError::InvalidMetadataValue {
                    key: item.key.clone(),
                }
            })?;
            if item.value.len() > SORAFS_GOVERNANCE_EXTERNAL_METADATA_VALUE_MAX_BYTES_V1 {
                return Err(
                    GovernanceExternalPayloadValidationError::MetadataValueTooLong {
                        key: item.key.clone(),
                        length: item.value.len(),
                        max: SORAFS_GOVERNANCE_EXTERNAL_METADATA_VALUE_MAX_BYTES_V1,
                    },
                );
            }
            metadata_bytes = metadata_bytes
                .checked_add(item.key.len())
                .and_then(|value| value.checked_add(item.value.len()))
                .ok_or(
                    GovernanceExternalPayloadValidationError::MetadataBytesTooLarge {
                        bytes: usize::MAX,
                        max: SORAFS_GOVERNANCE_EXTERNAL_METADATA_TOTAL_MAX_BYTES_V1,
                    },
                )?;
            if metadata_bytes > SORAFS_GOVERNANCE_EXTERNAL_METADATA_TOTAL_MAX_BYTES_V1 {
                return Err(
                    GovernanceExternalPayloadValidationError::MetadataBytesTooLarge {
                        bytes: metadata_bytes,
                        max: SORAFS_GOVERNANCE_EXTERNAL_METADATA_TOTAL_MAX_BYTES_V1,
                    },
                );
            }
            if let Some(last) = last_key
                && last > item.key.as_str()
            {
                return Err(GovernanceExternalPayloadValidationError::MetadataKeysUnsorted);
            }
            if !seen_keys.insert(item.key.as_str()) {
                return Err(
                    GovernanceExternalPayloadValidationError::DuplicateMetadataKey {
                        key: item.key.clone(),
                    },
                );
            }
            last_key = Some(item.key.as_str());
        }

        let expected_metadata = match self.payload_kind.as_str() {
            GOVERNANCE_EXTERNAL_KIND_REPAIR_AUDIT_V1 => {
                self.require_payload_version(u16::from(REPAIR_AUDIT_EVENT_VERSION_V1))?;
                let event = decode_canonical_external_payload::<RepairAuditEventV1, _>(
                    &self.payload_kind,
                    &self.encoded_payload,
                    |event| event.validate().map_err(|err| err.to_string()),
                )?;
                repair_audit_external_metadata(&event)
            }
            GOVERNANCE_EXTERNAL_KIND_REPAIR_SLASH_V1 => {
                self.require_payload_version(u16::from(REPAIR_SLASH_PROPOSAL_VERSION_V1))?;
                let proposal = decode_canonical_external_payload::<RepairSlashProposalV1, _>(
                    &self.payload_kind,
                    &self.encoded_payload,
                    |proposal| proposal.validate().map_err(|err| err.to_string()),
                )?;
                if proposal.approval.is_some() {
                    return Err(
                        GovernanceExternalPayloadValidationError::RepairSlashApprovalForbidden,
                    );
                }
                let stage = external_metadata_value(&self.metadata, "stage")
                    .and_then(GovernanceExternalRepairSlashStageV1::parse)
                    .ok_or(GovernanceExternalPayloadValidationError::InvalidRepairSlashStage)?;
                repair_slash_external_metadata(&proposal, stage)
            }
            GOVERNANCE_EXTERNAL_KIND_GC_AUDIT_V1 => {
                self.require_payload_version(u16::from(GC_AUDIT_EVENT_VERSION_V1))?;
                let event = decode_canonical_external_payload::<GcAuditEventV1, _>(
                    &self.payload_kind,
                    &self.encoded_payload,
                    |event| event.validate().map_err(|err| err.to_string()),
                )?;
                gc_audit_external_metadata(&event)
            }
            GOVERNANCE_EXTERNAL_KIND_RECONCILIATION_V1 => {
                self.require_payload_version(u16::from(SORAFS_RECONCILIATION_REPORT_VERSION_V1))?;
                let report = decode_canonical_external_payload::<SorafsReconciliationReportV1, _>(
                    &self.payload_kind,
                    &self.encoded_payload,
                    |report| report.validate().map_err(|err| err.to_string()),
                )?;
                reconciliation_external_metadata(&report)
            }
            GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1 => {
                self.require_payload_version(MODERATION_LEDGER_PUBLICATION_VERSION_V1)?;
                let publication =
                    decode_canonical_external_payload::<ModerationLedgerCyclePublicationV1, _>(
                        &self.payload_kind,
                        &self.encoded_payload,
                        |publication| publication.validate().map_err(|err| err.to_string()),
                    )?;
                transparency_publication_external_metadata(&publication)?
            }
            GOVERNANCE_EXTERNAL_KIND_PROOF_TOKEN_ISSUANCE_V1 => {
                self.require_payload_version(PROOF_TOKEN_ISSUANCE_VERSION_V1)?;
                let issuance = decode_canonical_external_payload::<ProofTokenIssuanceV1, _>(
                    &self.payload_kind,
                    &self.encoded_payload,
                    |issuance| issuance.validate().map_err(|err| err.to_string()),
                )?;
                proof_token_external_metadata(&issuance)
            }
            _ => {
                return Err(
                    GovernanceExternalPayloadValidationError::UnsupportedPayloadKind {
                        payload_kind: self.payload_kind.clone(),
                    },
                );
            }
        };
        if self.metadata != expected_metadata {
            return Err(GovernanceExternalPayloadValidationError::MetadataMismatch {
                payload_kind: self.payload_kind.clone(),
            });
        }
        Ok(())
    }

    fn require_payload_version(
        &self,
        expected: u16,
    ) -> Result<(), GovernanceExternalPayloadValidationError> {
        if self.payload_version != expected {
            return Err(
                GovernanceExternalPayloadValidationError::UnsupportedPayloadVersion {
                    payload_kind: self.payload_kind.clone(),
                    expected,
                    found: self.payload_version,
                },
            );
        }
        Ok(())
    }
}

fn decode_canonical_external_payload<T, F>(
    payload_kind: &str,
    bytes: &[u8],
    validate: F,
) -> Result<T, GovernanceExternalPayloadValidationError>
where
    T: for<'decode> norito::NoritoDeserialize<'decode> + norito::NoritoSerialize,
    F: FnOnce(&T) -> Result<(), String>,
{
    let limits = norito::DecodeLimits::new(
        65_536,
        SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_MAX_BYTES_V1,
        1_000_000,
        SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_MAX_BYTES_V1.saturating_mul(4),
        128,
    );
    let decoded = norito::decode_from_bytes_with_limits::<T>(bytes, limits).map_err(|err| {
        GovernanceExternalPayloadValidationError::TypedPayloadDecode {
            payload_kind: payload_kind.to_owned(),
            reason: err.to_string(),
        }
    })?;
    let canonical = norito::to_bytes(&decoded).map_err(|err| {
        GovernanceExternalPayloadValidationError::TypedPayloadEncode {
            payload_kind: payload_kind.to_owned(),
            reason: err.to_string(),
        }
    })?;
    if canonical != bytes {
        return Err(
            GovernanceExternalPayloadValidationError::NonCanonicalEncodedPayload {
                payload_kind: payload_kind.to_owned(),
            },
        );
    }
    validate(&decoded).map_err(|reason| {
        GovernanceExternalPayloadValidationError::InvalidTypedPayload {
            payload_kind: payload_kind.to_owned(),
            reason,
        }
    })?;
    Ok(decoded)
}

fn external_metadata(
    values: impl IntoIterator<Item = (&'static str, String)>,
) -> Vec<GovernanceExternalPayloadMetadataV1> {
    let mut metadata = values
        .into_iter()
        .map(|(key, value)| GovernanceExternalPayloadMetadataV1 {
            key: key.to_owned(),
            value,
        })
        .collect::<Vec<_>>();
    metadata.sort_by(|left, right| left.key.cmp(&right.key));
    metadata
}

fn external_metadata_value<'a>(
    metadata: &'a [GovernanceExternalPayloadMetadataV1],
    key: &str,
) -> Option<&'a str> {
    metadata
        .iter()
        .find(|item| item.key == key)
        .map(|item| item.value.as_str())
}

fn repair_audit_external_metadata(
    event: &RepairAuditEventV1,
) -> Vec<GovernanceExternalPayloadMetadataV1> {
    external_metadata([
        (
            "manifest_digest_hex",
            hex::encode(event.payload.manifest_digest),
        ),
        (
            "occurred_at_unix",
            event.header.occurred_at_unix.to_string(),
        ),
        ("provider_id_hex", hex::encode(event.payload.provider_id)),
        ("sequence", event.header.sequence.to_string()),
        ("status", event.payload.status.to_string()),
        ("ticket_id", event.payload.ticket_id.0.clone()),
    ])
}

fn repair_slash_external_metadata(
    proposal: &RepairSlashProposalV1,
    stage: GovernanceExternalRepairSlashStageV1,
) -> Vec<GovernanceExternalPayloadMetadataV1> {
    external_metadata([
        ("manifest_digest_hex", hex::encode(proposal.manifest_digest)),
        ("provider_id_hex", hex::encode(proposal.provider_id)),
        ("stage", stage.as_str().to_owned()),
        ("submitted_at_unix", proposal.submitted_at_unix.to_string()),
        ("ticket_id", proposal.ticket_id.0.clone()),
    ])
}

fn gc_audit_external_metadata(event: &GcAuditEventV1) -> Vec<GovernanceExternalPayloadMetadataV1> {
    external_metadata([
        (
            "blocked_reason",
            event
                .payload
                .blocked_reason
                .clone()
                .unwrap_or_else(|| "none".to_owned()),
        ),
        ("evicted_at_unix", event.payload.evicted_at_unix.to_string()),
        (
            "manifest_digest_hex",
            hex::encode(event.payload.manifest_digest),
        ),
        ("provider_id_hex", hex::encode(event.payload.provider_id)),
        ("reason", event.payload.reason.clone()),
        ("sequence", event.header.sequence.to_string()),
    ])
}

fn reconciliation_external_metadata(
    report: &SorafsReconciliationReportV1,
) -> Vec<GovernanceExternalPayloadMetadataV1> {
    external_metadata([
        ("divergence_count", report.divergence_count.to_string()),
        ("gc_snapshot_hash_hex", hex::encode(report.gc_snapshot_hash)),
        ("generated_at_unix", report.generated_at_unix.to_string()),
        ("provider_id_hex", hex::encode(report.provider_id)),
        (
            "repair_snapshot_hash_hex",
            hex::encode(report.repair_snapshot_hash),
        ),
        (
            "retention_snapshot_hash_hex",
            hex::encode(report.retention_snapshot_hash),
        ),
    ])
}

fn transparency_publication_external_metadata(
    publication: &ModerationLedgerCyclePublicationV1,
) -> Result<Vec<GovernanceExternalPayloadMetadataV1>, GovernanceExternalPayloadValidationError> {
    publication.validate().map_err(|err| {
        GovernanceExternalPayloadValidationError::InvalidTypedPayload {
            payload_kind: GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1.to_owned(),
            reason: err.to_string(),
        }
    })?;
    let block_hash = publication.block.block_hash().map_err(|err| {
        GovernanceExternalPayloadValidationError::TypedPayloadEncode {
            payload_kind: GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1.to_owned(),
            reason: err.to_string(),
        }
    })?;
    let publication_hash = publication.publication_hash().map_err(|err| {
        GovernanceExternalPayloadValidationError::TypedPayloadEncode {
            payload_kind: GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1.to_owned(),
            reason: err.to_string(),
        }
    })?;
    Ok(external_metadata([
        ("block_hash_hex", hex::encode(block_hash)),
        ("cycle_id_hex", hex::encode(publication.block.cycle_id)),
        ("entry_count", publication.block.entry_count.to_string()),
        ("entry_root_hex", hex::encode(publication.block.entry_root)),
        ("publication_hash_hex", hex::encode(publication_hash)),
    ]))
}

fn proof_token_external_metadata(
    issuance: &ProofTokenIssuanceV1,
) -> Vec<GovernanceExternalPayloadMetadataV1> {
    let mut values = vec![
        ("blinded_digest_hex", hex::encode(issuance.blinded_digest)),
        ("entry_count", issuance.entry_ids.len().to_string()),
        ("issued_at_unix", issuance.issued_at_unix.to_string()),
        ("signer_key_hex", hex::encode(issuance.signer_key)),
        ("token_blake3_hex", hex::encode(issuance.token_blake3)),
        ("token_id_hex", hex::encode(issuance.token_id)),
    ];
    if let Some(expires_at) = issuance.expires_at_unix {
        values.push(("expires_at_unix", expires_at.to_string()));
    }
    if let Some(evidence_digest) = issuance.evidence_digest {
        values.push(("evidence_digest_hex", hex::encode(evidence_digest)));
    }
    if let Some(policy_digest) = issuance.policy_digest {
        values.push(("policy_digest_hex", hex::encode(policy_digest)));
    }
    external_metadata(values)
}

fn validate_external_payload_label(value: &str) -> Result<(), ()> {
    if value.is_empty() || value.trim() != value {
        return Err(());
    }
    if !value
        .bytes()
        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'-' | b'.'))
    {
        return Err(());
    }
    Ok(())
}

fn validate_external_payload_text(value: &str) -> Result<(), ()> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(());
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
    /// Admission-bound PDP terminal archive.
    PdpArchive(PdpGovernanceArchiveV1),
    /// Audit verdict for a challenge.
    AuditVerdict(AuditVerdictV1),
    /// Deal settlement snapshot.
    DealSettlement(Box<DealSettlementV1>),
    /// Externally authorized provider reputation snapshot and scoring evidence.
    SignedReputationSnapshot(SignedReputationSnapshotV1),
    /// SoraFS moderation ballot lifecycle event.
    ModerationBallotEvent(SoraFsModerationBallotGovernanceEventV1),
    /// SoraFS appeal finance report.
    AppealFinanceReport(SoraFsAppealFinanceReportV1),
    /// SoraFS weekly appeal finance transparency rollup.
    AppealFinanceWeeklyRollup(SoraFsAppealFinanceWeeklyRollupV1),
    /// SoraFS appeal finance settlement submission receipt.
    AppealFinanceSettlementReceipt(SoraFsAppealFinanceSettlementReceiptV1),
    /// SoraFS orderbook streaming-settlement receipt.
    OrderbookSettlementReceipt(SettlementReceiptV1),
    /// Canonical external SoraFS governance payload bytes.
    ExternalPayload(GovernanceExternalPayloadV1),
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
            GovernanceLogPayloadV1::PdpArchive(archive) => {
                archive
                    .validate()
                    .map_err(GovernanceLogValidationError::PdpArchive)?;
                if archive.decided_at_unix > timestamp {
                    return Err(GovernanceLogValidationError::PdpArchiveDecisionAfterNode {
                        decided_at: archive.decided_at_unix,
                        node_timestamp: timestamp,
                    });
                }
                Ok(())
            }
            GovernanceLogPayloadV1::AuditVerdict(verdict) => verdict
                .validate()
                .map_err(GovernanceLogValidationError::AuditVerdict),
            GovernanceLogPayloadV1::DealSettlement(settlement) => settlement
                .validate()
                .map_err(GovernanceLogValidationError::DealSettlement),
            GovernanceLogPayloadV1::SignedReputationSnapshot(envelope) => envelope
                .validate_structure()
                .map_err(GovernanceLogValidationError::SignedReputationSnapshot),
            GovernanceLogPayloadV1::ModerationBallotEvent(event) => event
                .validate()
                .map_err(GovernanceLogValidationError::ModerationBallotEvent),
            GovernanceLogPayloadV1::AppealFinanceReport(report) => report
                .validate()
                .map_err(GovernanceLogValidationError::AppealFinanceReport),
            GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(rollup) => rollup
                .validate()
                .map_err(GovernanceLogValidationError::AppealFinanceWeeklyRollup),
            GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(receipt) => receipt
                .validate()
                .map_err(GovernanceLogValidationError::AppealFinanceSettlementReceipt),
            GovernanceLogPayloadV1::OrderbookSettlementReceipt(receipt) => receipt
                .validate()
                .map_err(GovernanceLogValidationError::OrderbookSettlementReceipt),
            GovernanceLogPayloadV1::ExternalPayload(payload) => payload
                .validate()
                .map_err(GovernanceLogValidationError::ExternalPayload),
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

fn validate_non_empty_settlement_receipt_label(
    value: &str,
    error: SoraFsAppealFinanceSettlementReceiptValidationError,
) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
    if value.trim().is_empty() {
        return Err(error);
    }
    Ok(())
}

fn validate_receipt_hex(
    value: &str,
    field: &'static str,
    byte_len: usize,
    missing: SoraFsAppealFinanceSettlementReceiptValidationError,
) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
    if value.trim().is_empty() {
        return Err(missing);
    }
    if value.len() != byte_len.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(
            SoraFsAppealFinanceSettlementReceiptValidationError::InvalidHex {
                field,
                expected_bytes: byte_len,
            },
        );
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
    /// Exact deterministic BLAKE3-256 CID bytes derived from the canonical
    /// block payload excluding the block signature.
    pub block_cid: Vec<u8>,
    /// Optional exact 32-byte parent block CID.
    #[norito(default)]
    pub prev_block_cid: Option<Vec<u8>>,
    /// Monotonic sequence number in the public DAG chain.
    pub sequence: u64,
    /// Unix timestamp (seconds) when this block was assembled.
    pub timestamp: u64,
    /// Publisher peer identifier, bounded to 128 bytes.
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
        if self.block_cid.len() != GOVERNANCE_DAG_CID_BYTES_V1 {
            return Err(GovernanceDagBlockValidationError::InvalidBlockCidLength {
                length: self.block_cid.len(),
            });
        }
        if let Some(prev) = self.prev_block_cid.as_ref()
            && prev.len() != GOVERNANCE_DAG_CID_BYTES_V1
        {
            return Err(
                GovernanceDagBlockValidationError::InvalidPrevBlockCidLength { length: prev.len() },
            );
        }
        if self.sequence == 0 && self.prev_block_cid.is_some() {
            return Err(GovernanceDagBlockValidationError::RootHasParent);
        }
        if self.sequence > 0 && self.prev_block_cid.is_none() {
            return Err(GovernanceDagBlockValidationError::NonRootMissingParent);
        }
        if self.sequence == 0 && self.node.prev_cid.is_some() {
            return Err(GovernanceDagBlockValidationError::RootNodeHasParent);
        }
        if self.sequence > 0 && self.node.prev_cid.is_none() {
            return Err(GovernanceDagBlockValidationError::NonRootNodeMissingParent);
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceDagBlockValidationError::MissingPublisherPeerId);
        }
        if self.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 {
            return Err(GovernanceDagBlockValidationError::PublisherPeerIdTooLong {
                length: self.publisher_peer_id.len(),
                maximum: GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1,
            });
        }
        if self.block_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519 {
            return Err(GovernanceDagBlockValidationError::NonEd25519BlockSignature);
        }
        self.block_signature
            .validate()
            .map_err(|_| GovernanceDagBlockValidationError::InvalidSignature)?;
        self.node
            .validate()
            .map_err(GovernanceDagBlockValidationError::Node)?;
        if self.node.publisher_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519 {
            return Err(GovernanceDagBlockValidationError::NonEd25519NodeSignature);
        }
        if self.node.publisher_peer_id != self.publisher_peer_id {
            return Err(GovernanceDagBlockValidationError::NodePublisherPeerMismatch);
        }
        if self.node.publisher_signature.public_key != self.block_signature.public_key {
            return Err(GovernanceDagBlockValidationError::NodePublisherKeyMismatch);
        }
        if self.node.timestamp > self.timestamp {
            return Err(GovernanceDagBlockValidationError::NodeTimestampAfterBlock);
        }
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
    /// Exact 32-byte current head block CID.
    pub head_block_cid: Vec<u8>,
    /// Number of blocks in the chain this head advertises.
    pub block_count: u64,
    /// Unix timestamp (seconds) when this head manifest was generated.
    pub generated_at: u64,
    /// Publisher peer identifier, bounded to 128 bytes.
    pub publisher_peer_id: Vec<u8>,
    /// First block CID in the newest 64-block window.
    ///
    /// This is absent when `block_count <= 64` and present otherwise. It never
    /// identifies a previous head manifest.
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
        if self.head_block_cid.len() != GOVERNANCE_DAG_CID_BYTES_V1 {
            return Err(
                GovernanceDagHeadValidationError::InvalidHeadBlockCidLength {
                    length: self.head_block_cid.len(),
                },
            );
        }
        if self.block_count == 0 {
            return Err(GovernanceDagHeadValidationError::EmptyBlockCount);
        }
        if self.generated_at == 0 {
            return Err(GovernanceDagHeadValidationError::MissingGeneratedAt);
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceDagHeadValidationError::MissingPublisherPeerId);
        }
        if self.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 {
            return Err(GovernanceDagHeadValidationError::PublisherPeerIdTooLong {
                length: self.publisher_peer_id.len(),
                maximum: GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1,
            });
        }
        if let Some(checkpoint) = self.checkpoint_cid.as_ref()
            && checkpoint.len() != GOVERNANCE_DAG_CID_BYTES_V1
        {
            return Err(
                GovernanceDagHeadValidationError::InvalidCheckpointCidLength {
                    length: checkpoint.len(),
                },
            );
        }
        if self.head_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519 {
            return Err(GovernanceDagHeadValidationError::NonEd25519HeadSignature);
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
        if self.public_key.is_empty()
            || crate::inert_bytes(&self.public_key)
            || self.signature.is_empty()
            || crate::inert_bytes(&self.signature)
        {
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
    /// Exact deterministic BLAKE3-256 CID bytes for this canonical node.
    pub node_cid: Vec<u8>,
    /// Optional exact 32-byte previous node CID in the chain.
    #[norito(default)]
    pub prev_cid: Option<Vec<u8>>,
    /// Unix timestamp (seconds) when this node was published.
    pub timestamp: u64,
    /// Publisher peer identifier (e.g., libp2p peer ID), bounded to 128 bytes.
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
        if self.node_cid.len() != GOVERNANCE_DAG_CID_BYTES_V1 {
            return Err(GovernanceLogValidationError::InvalidNodeCidLength {
                length: self.node_cid.len(),
            });
        }
        if let Some(prev) = self.prev_cid.as_ref()
            && prev.len() != GOVERNANCE_DAG_CID_BYTES_V1
        {
            return Err(GovernanceLogValidationError::InvalidPrevCidLength { length: prev.len() });
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceLogValidationError::MissingPublisherPeerId);
        }
        if self.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 {
            return Err(GovernanceLogValidationError::PublisherPeerIdTooLong {
                length: self.publisher_peer_id.len(),
                maximum: GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1,
            });
        }
        self.publisher_signature.validate()?;
        self.payload.validate(self.timestamp)?;
        let expected_cid =
            self.recompute_node_cid()
                .map_err(|err| GovernanceLogValidationError::CidEncoding {
                    reason: err.to_string(),
                })?;
        if self.node_cid != expected_cid {
            return Err(GovernanceLogValidationError::InvalidNodeCid);
        }
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
    let verifying_key = crate::checked_ed25519_verifying_key_from_bytes(&public_key)
        .map_err(|err| GovernanceLogSignatureVerificationError::InvalidPublicKey { reason: err })?;

    let mut signature = [0u8; SIGNATURE_LENGTH];
    signature.copy_from_slice(&publisher_signature.signature);
    let signature = crate::checked_ed25519_signature_from_bytes(&signature)
        .map_err(|reason| GovernanceLogSignatureVerificationError::Verification { reason })?;

    verifying_key
        .verify_strict(payload_bytes, &signature)
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
    MlDsaSuite::MlDsa65
        .validate_public_key(&publisher_signature.public_key)
        .map_err(
            |err| GovernanceLogSignatureVerificationError::InvalidPublicKey {
                reason: err.to_string(),
            },
        )?;
    MlDsaSuite::MlDsa65
        .validate_signature(&publisher_signature.signature)
        .map_err(
            |err| GovernanceLogSignatureVerificationError::Verification {
                reason: format!("invalid signature material: {err}"),
            },
        )?;
    let public_key = PublicKey::from_bytes(Algorithm::MlDsa, &publisher_signature.public_key)
        .map_err(
            |err| GovernanceLogSignatureVerificationError::InvalidPublicKey {
                reason: err.to_string(),
            },
        )?;
    let signature =
        iroha_crypto::mldsa65_parse_signature(&publisher_signature.signature).map_err(|err| {
            GovernanceLogSignatureVerificationError::Verification {
                reason: format!("invalid signature material: {err}"),
            }
        })?;
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
    #[error("governance log node CID must be {GOVERNANCE_DAG_CID_BYTES_V1} bytes, got {length}")]
    InvalidNodeCidLength { length: usize },
    #[error(
        "previous governance log node CID must be {GOVERNANCE_DAG_CID_BYTES_V1} bytes, got {length}"
    )]
    InvalidPrevCidLength { length: usize },
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("publisher peer ID is {length} bytes, maximum is {maximum}")]
    PublisherPeerIdTooLong { length: usize, maximum: usize },
    #[error("publisher signature missing key or signature bytes")]
    InvalidSignature,
    #[error("failed to encode canonical governance log node CID payload: {reason}")]
    CidEncoding { reason: String },
    #[error("governance log node CID does not match the canonical node payload")]
    InvalidNodeCid,
    #[error("advert validation failed: {0}")]
    Advert(crate::provider_advert::AdvertValidationError),
    #[error("replication order validation failed: {0}")]
    ReplicationOrder(crate::capacity::ReplicationOrderValidationError),
    #[error("challenge validation failed: {0}")]
    PorChallenge(crate::por::PorChallengeValidationError),
    #[error("proof validation failed: {0}")]
    PorProof(crate::por::PorProofValidationError),
    #[error("PDP governance archive validation failed: {0}")]
    PdpArchive(PdpGovernanceArchiveValidationError),
    #[error(
        "PDP archive decision timestamp {decided_at} exceeds governance node timestamp {node_timestamp}"
    )]
    PdpArchiveDecisionAfterNode {
        decided_at: u64,
        node_timestamp: u64,
    },
    #[error("audit verdict validation failed: {0}")]
    AuditVerdict(crate::por::AuditVerdictValidationError),
    #[error("deal settlement validation failed: {0}")]
    DealSettlement(crate::deal::DealSettlementValidationError),
    #[error("signed reputation snapshot validation failed: {0}")]
    SignedReputationSnapshot(SignedReputationSnapshotError),
    #[error("moderation ballot event validation failed: {0}")]
    ModerationBallotEvent(SoraFsModerationBallotGovernanceEventValidationError),
    #[error("appeal finance report validation failed: {0}")]
    AppealFinanceReport(SoraFsAppealFinanceReportValidationError),
    #[error("appeal finance weekly rollup validation failed: {0}")]
    AppealFinanceWeeklyRollup(SoraFsAppealFinanceWeeklyRollupValidationError),
    #[error("appeal finance settlement receipt validation failed: {0}")]
    AppealFinanceSettlementReceipt(SoraFsAppealFinanceSettlementReceiptValidationError),
    #[error("orderbook settlement receipt validation failed: {0}")]
    OrderbookSettlementReceipt(crate::orderbook::OrderbookValidationError),
    #[error("external governance payload validation failed: {0}")]
    ExternalPayload(GovernanceExternalPayloadValidationError),
}

/// Validation errors for generic external Governance DAG payloads.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GovernanceExternalPayloadValidationError {
    /// External payload wrapper uses an unsupported schema version.
    #[error("unsupported external governance payload version `{found}` (expected {expected})")]
    UnsupportedVersion {
        /// Expected schema version.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Payload kind is empty, padded, or contains unsupported characters.
    #[error("external governance payload kind `{payload_kind}` is not a valid public label")]
    InvalidPayloadKind {
        /// Invalid payload kind.
        payload_kind: String,
    },
    /// Payload kind is syntactically valid but outside the closed V1 allowlist.
    #[error("unsupported external governance payload kind `{payload_kind}`")]
    UnsupportedPayloadKind {
        /// Unsupported kind label.
        payload_kind: String,
    },
    /// Embedded payload schema version is not the first-release version for its kind.
    #[error(
        "unsupported `{payload_kind}` external payload version `{found}` (expected {expected})"
    )]
    UnsupportedPayloadVersion {
        /// Closed payload kind.
        payload_kind: String,
        /// Required first-release version.
        expected: u16,
        /// Version observed in the wrapper.
        found: u16,
    },
    /// Embedded canonical payload bytes are missing.
    #[error("external governance payload bytes are required")]
    MissingEncodedPayload,
    /// Embedded payload exceeds the first-release byte bound.
    #[error("external governance payload has {length} bytes, exceeding limit {max}")]
    EncodedPayloadTooLarge {
        /// Observed byte length.
        length: usize,
        /// Maximum accepted byte length.
        max: usize,
    },
    /// Declared encoded length does not match the embedded bytes.
    #[error("external governance payload length mismatch: declared {declared}, actual {actual}")]
    EncodedLengthMismatch {
        /// Declared byte length.
        declared: u64,
        /// Actual byte length.
        actual: u64,
    },
    /// Embedded payload digest does not match the embedded bytes.
    #[error("external governance payload digest does not match encoded bytes")]
    EncodedDigestMismatch,
    /// Typed payload failed bounded Norito decoding.
    #[error("failed to decode `{payload_kind}` external payload: {reason}")]
    TypedPayloadDecode {
        /// Closed payload kind.
        payload_kind: String,
        /// Bounded decode error.
        reason: String,
    },
    /// Typed payload could not be canonically encoded after decoding.
    #[error("failed to encode `{payload_kind}` external payload canonically: {reason}")]
    TypedPayloadEncode {
        /// Closed payload kind.
        payload_kind: String,
        /// Canonical encoding error.
        reason: String,
    },
    /// Embedded bytes decode but are not the unique canonical Norito encoding.
    #[error("`{payload_kind}` external payload bytes are not canonical")]
    NonCanonicalEncodedPayload {
        /// Closed payload kind.
        payload_kind: String,
    },
    /// Typed payload violates its native schema invariants.
    #[error("invalid `{payload_kind}` external payload: {reason}")]
    InvalidTypedPayload {
        /// Closed payload kind.
        payload_kind: String,
        /// Native validation error.
        reason: String,
    },
    /// Metadata key is empty, padded, or contains unsupported characters.
    #[error("external governance payload metadata key `{key}` is not a valid public label")]
    InvalidMetadataKey {
        /// Invalid metadata key.
        key: String,
    },
    /// Metadata value is empty, padded, or contains a control character.
    #[error("external governance payload metadata value for `{key}` is not public text")]
    InvalidMetadataValue {
        /// Metadata key whose value is invalid.
        key: String,
    },
    /// External metadata row count exceeds the first-release bound.
    #[error("external governance payload has {count} metadata rows, exceeding limit {max}")]
    MetadataCountTooLarge {
        /// Observed row count.
        count: usize,
        /// Maximum accepted row count.
        max: usize,
    },
    /// External metadata key exceeds the first-release byte bound.
    #[error("external governance metadata key `{key}` has {length} bytes, exceeding limit {max}")]
    MetadataKeyTooLong {
        /// Oversized key.
        key: String,
        /// Observed UTF-8 byte length.
        length: usize,
        /// Maximum accepted UTF-8 byte length.
        max: usize,
    },
    /// External metadata value exceeds the first-release byte bound.
    #[error(
        "external governance metadata value for `{key}` has {length} bytes, exceeding limit {max}"
    )]
    MetadataValueTooLong {
        /// Metadata key.
        key: String,
        /// Observed UTF-8 byte length.
        length: usize,
        /// Maximum accepted UTF-8 byte length.
        max: usize,
    },
    /// Cumulative external metadata exceeds the first-release byte budget.
    #[error("external governance metadata uses {bytes} bytes, exceeding limit {max}")]
    MetadataBytesTooLarge {
        /// Observed cumulative bytes.
        bytes: usize,
        /// Maximum accepted cumulative bytes.
        max: usize,
    },
    /// Metadata keys are not sorted.
    #[error("external governance payload metadata keys must be sorted")]
    MetadataKeysUnsorted,
    /// Metadata key appears more than once.
    #[error("duplicate external governance payload metadata key `{key}`")]
    DuplicateMetadataKey {
        /// Duplicate metadata key.
        key: String,
    },
    /// Metadata is not the exact projection of the typed embedded payload.
    #[error("external governance metadata does not match `{payload_kind}` payload fields")]
    MetadataMismatch {
        /// Closed payload kind.
        payload_kind: String,
    },
    /// Repair slash metadata contains an unsupported stage.
    #[error("repair slash external payload stage must be `drafted` or `submitted`")]
    InvalidRepairSlashStage,
    /// Repair slash external payloads must not embed an approval summary.
    #[error("repair slash external payload must not embed a governance approval summary")]
    RepairSlashApprovalForbidden,
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
    /// A non-challenge event unexpectedly included a challenge record.
    #[error("SoraFS moderation ballot governance event must not include a challenge")]
    UnexpectedChallenge,
    /// A challenge event omitted its challenge payload.
    #[error("SoraFS moderation ballot governance challenge is required")]
    MissingChallenge,
    /// Challenge count is invalid for a challenge event.
    #[error("SoraFS moderation ballot governance challenge count must be nonzero")]
    InvalidChallengeCount,
    /// Missing moderation challenge identifier.
    #[error("SoraFS moderation ballot governance challenge id is required")]
    MissingChallengeId,
    /// Missing moderation challenge submitter.
    #[error("SoraFS moderation ballot governance challenger id is required")]
    MissingChallengerId,
    /// Challenge target juror id is required for this kind.
    #[error("SoraFS moderation ballot governance challenge target juror id is required")]
    MissingChallengeTarget,
    /// Challenge target juror id is blank.
    #[error("SoraFS moderation ballot governance challenge target juror id must not be blank")]
    BlankChallengeTarget,
    /// Challenge evidence digest is all zeroes.
    #[error("SoraFS moderation ballot governance challenge evidence digest must be nonzero")]
    InvalidChallengeEvidence,
    /// Challenge reason is missing.
    #[error("SoraFS moderation ballot governance challenge reason is required")]
    MissingChallengeReason,
    /// Challenge case id does not match the enclosing event case id.
    #[error(
        "SoraFS moderation challenge case id mismatch: event `{event}`, challenge `{challenge}`"
    )]
    ChallengeCaseMismatch {
        /// Case id from the event.
        event: String,
        /// Case id from the challenge.
        challenge: String,
    },
    /// Challenge round id does not match the enclosing event round id.
    #[error(
        "SoraFS moderation challenge round id mismatch: event `{event}`, challenge `{challenge}`"
    )]
    ChallengeRoundMismatch {
        /// Round id from the event.
        event: String,
        /// Round id from the challenge.
        challenge: String,
    },
    /// Submitted challenge event included resolution fields.
    #[error("SoraFS moderation challenge submission must not include resolution fields")]
    UnexpectedChallengeResolution,
    /// Resolved challenge event omitted its decision.
    #[error("SoraFS moderation challenge decision is required")]
    MissingChallengeDecision,
    /// Resolved challenge event omitted its resolver.
    #[error("SoraFS moderation challenge resolver is required")]
    MissingChallengeResolver,
    /// Resolved challenge event omitted its resolution timestamp.
    #[error("SoraFS moderation challenge resolved timestamp is required")]
    MissingChallengeResolvedAt,
    /// Challenge resolution timestamp predates the challenge.
    #[error("SoraFS moderation challenge resolution timestamp predates the challenge")]
    InvalidChallengeResolutionTimestamp,
    /// Challenge resolution note is blank.
    #[error("SoraFS moderation challenge resolution note must not be blank")]
    BlankChallengeResolutionNote,
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

/// Validation errors for SoraFS appeal finance settlement receipts.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoraFsAppealFinanceSettlementReceiptValidationError {
    /// Receipt uses an unsupported schema version.
    #[error(
        "unsupported SoraFS appeal finance settlement receipt version `{found}` (expected {expected})"
    )]
    UnsupportedVersion {
        /// Expected schema version.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Missing non-zero receipt id.
    #[error("SoraFS appeal finance settlement receipt id is required")]
    MissingReceiptId,
    /// Missing case id.
    #[error("SoraFS appeal finance settlement receipt case id is required")]
    MissingCaseId,
    /// Missing round id when the optional field is present.
    #[error("SoraFS appeal finance settlement receipt round id must not be empty")]
    MissingRoundId,
    /// Missing generated timestamp.
    #[error("SoraFS appeal finance settlement receipt generated timestamp is required")]
    MissingGeneratedAt,
    /// Missing finance config version.
    #[error("SoraFS appeal finance settlement receipt config version is required")]
    MissingFinanceConfigVersion,
    /// Missing escrow id.
    #[error("SoraFS appeal finance settlement receipt escrow id is required")]
    MissingEscrowId,
    /// Missing payer account.
    #[error("SoraFS appeal finance settlement receipt payer account is required")]
    MissingPayerAccount,
    /// Missing destination account.
    #[error("SoraFS appeal finance settlement receipt destination account is required")]
    MissingDestinationAccount,
    /// Missing optional release authority when present.
    #[error("SoraFS appeal finance settlement receipt release authority account is required")]
    MissingReleaseAuthorityAccount,
    /// Missing submitted step.
    #[error("SoraFS appeal finance settlement receipt submitted step is required")]
    MissingSubmittedStep,
    /// Missing required authority.
    #[error("SoraFS appeal finance settlement receipt required authority is required")]
    MissingRequiredAuthority,
    /// Missing transaction hash.
    #[error("SoraFS appeal finance settlement receipt transaction hash is required")]
    MissingTxHash,
    /// Missing reconciliation digest.
    #[error("SoraFS appeal finance settlement receipt reconciliation digest is required")]
    MissingReconciliationDigest,
    /// Missing reconciliation status.
    #[error("SoraFS appeal finance settlement receipt reconciliation status is required")]
    MissingReconciliationStatus,
    /// Missing observed lifecycle status.
    #[error("SoraFS appeal finance settlement receipt observed lifecycle status is required")]
    MissingObservedLifecycleStatus,
    /// Label contained leading or trailing whitespace.
    #[error("SoraFS appeal finance settlement receipt label `{field}` is not canonical")]
    InvalidLabel {
        /// Field containing the invalid label.
        field: &'static str,
    },
    /// Hex field had the wrong length or non-hex characters.
    #[error(
        "SoraFS appeal finance settlement receipt `{field}` must be {expected_bytes} bytes of hex"
    )]
    InvalidHex {
        /// Field containing invalid hex.
        field: &'static str,
        /// Expected decoded byte length.
        expected_bytes: usize,
    },
    /// Panel size must be non-zero.
    #[error("SoraFS appeal finance settlement receipt panel size must be greater than zero")]
    InvalidPanelSize,
    /// Submitter signer count must be non-zero for a queued receipt.
    #[error("SoraFS appeal finance settlement receipt configured signer count must be non-zero")]
    InvalidConfiguredSignerCount,
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
    /// Exact amount accumulation overflowed the bounded numeric domain.
    #[error(
        "SoraFS appeal finance weekly rollup amount `{field}` overflowed for report {report_id:?}"
    )]
    AmountOverflow {
        /// Source report id.
        report_id: [u8; 16],
        /// Amount field name.
        field: &'static str,
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
    /// Exact outcome accumulation overflowed the bounded numeric domain.
    #[error("SoraFS appeal finance weekly rollup amount `{field}` overflowed")]
    AmountOverflow {
        /// Field whose accumulation overflowed.
        field: &'static str,
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
    #[error("block CID must be {GOVERNANCE_DAG_CID_BYTES_V1} bytes, got {length}")]
    InvalidBlockCidLength { length: usize },
    #[error("previous block CID must be {GOVERNANCE_DAG_CID_BYTES_V1} bytes, got {length}")]
    InvalidPrevBlockCidLength { length: usize },
    #[error("root governance DAG block must not carry a previous block CID")]
    RootHasParent,
    #[error("non-root governance DAG block must carry a previous block CID")]
    NonRootMissingParent,
    #[error("root governance DAG block node must not carry a previous node CID")]
    RootNodeHasParent,
    #[error("non-root governance DAG block node must carry a previous node CID")]
    NonRootNodeMissingParent,
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("publisher peer ID is {length} bytes, maximum is {maximum}")]
    PublisherPeerIdTooLong { length: usize, maximum: usize },
    #[error("governance DAG block signature must use Ed25519")]
    NonEd25519BlockSignature,
    #[error("embedded governance node signature must use Ed25519")]
    NonEd25519NodeSignature,
    #[error("embedded governance node publisher peer ID differs from the block publisher")]
    NodePublisherPeerMismatch,
    #[error("embedded governance node publisher key differs from the block publisher key")]
    NodePublisherKeyMismatch,
    #[error("embedded governance node timestamp exceeds its containing block timestamp")]
    NodeTimestampAfterBlock,
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
    #[error("head block CID must be {GOVERNANCE_DAG_CID_BYTES_V1} bytes, got {length}")]
    InvalidHeadBlockCidLength { length: usize },
    #[error("head manifest block count must be greater than zero")]
    EmptyBlockCount,
    #[error("head manifest generated-at timestamp must be greater than zero")]
    MissingGeneratedAt,
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("publisher peer ID is {length} bytes, maximum is {maximum}")]
    PublisherPeerIdTooLong { length: usize, maximum: usize },
    #[error(
        "checkpoint CID must be {GOVERNANCE_DAG_CID_BYTES_V1} bytes when present, got {length}"
    )]
    InvalidCheckpointCidLength { length: usize },
    #[error("governance DAG head signature must use Ed25519")]
    NonEd25519HeadSignature,
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
    #[error("duplicate governance DAG node CID at block index {index}")]
    DuplicateNodeCid { index: usize },
    #[error("block at index {index} has sequence {sequence}, expected {expected}")]
    SequenceGap {
        index: usize,
        expected: u64,
        sequence: u64,
    },
    #[error("block sequence overflows before index {index}")]
    SequenceOverflow { index: usize },
    #[error("block at index {index} has timestamp earlier than its parent")]
    TimestampRegression { index: usize },
    #[error("governance node at block index {index} has timestamp earlier than its parent")]
    NodeTimestampRegression { index: usize },
    #[error("block at index {index} is not in canonical root-to-head order")]
    NonCanonicalOrder { index: usize },
    #[error("governance node at block index {index} does not reference its predecessor node")]
    NodeParentMismatch { index: usize },
    #[error("block at index {index} uses a different publisher peer ID")]
    PublisherPeerMismatch { index: usize },
    #[error("block at index {index} uses a different Ed25519 publisher key")]
    PublisherKeyMismatch { index: usize },
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
    #[error("governance DAG block slice length cannot be represented as u64")]
    BlockCountOverflow,
    #[error("checkpoint must be absent for a full history of at most 64 blocks")]
    UnexpectedCheckpoint,
    #[error("checkpoint is required for a history containing more than 64 blocks")]
    MissingCheckpoint,
    #[error("checkpoint does not identify the first block in the newest 64-block window")]
    CheckpointMismatch,
    #[error("checkpoint tail must contain exactly 64 blocks, got {count}")]
    CheckpointWindowLength { count: usize },
    #[error("checkpoint tail requires a total block count greater than 64, got {block_count}")]
    InvalidCheckpointBlockCount { block_count: u64 },
    #[error("checkpoint tail starts at sequence {sequence}, expected {expected}")]
    CheckpointStartSequence { expected: u64, sequence: u64 },
    #[error("head publisher peer ID differs from the block and node publisher peer ID")]
    PublisherPeerMismatch,
    #[error("head Ed25519 publisher key differs from the block and node publisher key")]
    PublisherKeyMismatch,
    #[error(
        "head generated-at timestamp {head_generated_at} precedes tip block timestamp {tip_timestamp}"
    )]
    HeadTimestampBeforeTip {
        head_generated_at: u64,
        tip_timestamp: u64,
    },
}

/// Validates a canonical contiguous Governance DAG history or checkpoint tail.
///
/// A root history begins at sequence zero. A checkpoint tail may begin at a
/// non-zero sequence and leave the first block and node parent references
/// outside the supplied slice. Every later block and node must link exactly to
/// its predecessor in the supplied root-to-head order.
pub fn validate_governance_dag_chain_v1(
    blocks: &[GovernanceDagBlockV1],
    expected_head_cid: Option<&[u8]>,
) -> Result<(), GovernanceDagChainValidationError> {
    if blocks.is_empty() {
        return Err(GovernanceDagChainValidationError::Empty);
    }

    let mut block_cids = BTreeSet::<Vec<u8>>::new();
    let mut node_cids = BTreeSet::<Vec<u8>>::new();
    let first = &blocks[0];
    let publisher_peer_id = &first.publisher_peer_id;
    let publisher_public_key = &first.block_signature.public_key;
    for (index, block) in blocks.iter().enumerate() {
        block
            .validate()
            .map_err(|source| GovernanceDagChainValidationError::InvalidBlock { index, source })?;
        if !block_cids.insert(block.block_cid.clone()) {
            return Err(GovernanceDagChainValidationError::DuplicateBlockCid { index });
        }
        if !node_cids.insert(block.node.node_cid.clone()) {
            return Err(GovernanceDagChainValidationError::DuplicateNodeCid { index });
        }
        if block.publisher_peer_id != *publisher_peer_id {
            return Err(GovernanceDagChainValidationError::PublisherPeerMismatch { index });
        }
        if block.block_signature.public_key != *publisher_public_key {
            return Err(GovernanceDagChainValidationError::PublisherKeyMismatch { index });
        }

        if index == 0 {
            continue;
        }

        let parent = &blocks[index - 1];
        if block.prev_block_cid.as_deref() != Some(parent.block_cid.as_slice()) {
            return Err(GovernanceDagChainValidationError::NonCanonicalOrder { index });
        }
        if block.node.prev_cid.as_deref() != Some(parent.node.node_cid.as_slice()) {
            return Err(GovernanceDagChainValidationError::NodeParentMismatch { index });
        }
        let expected = parent
            .sequence
            .checked_add(1)
            .ok_or(GovernanceDagChainValidationError::SequenceOverflow { index })?;
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
        if block.node.timestamp < parent.node.timestamp {
            return Err(GovernanceDagChainValidationError::NodeTimestampRegression { index });
        }
    }

    if let Some(expected_head_cid) = expected_head_cid
        && blocks.last().map(|block| block.block_cid.as_slice()) != Some(expected_head_cid)
    {
        return Err(GovernanceDagChainValidationError::ExpectedHeadMismatch);
    }
    Ok(())
}

/// Validates a signed head against a full history or its exact newest checkpoint window.
///
/// Full histories start at sequence zero and contain `head.block_count`
/// blocks. Histories of at most 64 blocks omit the checkpoint; longer
/// histories commit the first block in their newest 64-block window. A bounded
/// checkpoint replay supplies exactly those newest 64 blocks, beginning at
/// sequence `head.block_count - 64`.
pub fn validate_governance_dag_head_against_chain_v1(
    head: &GovernanceDagHeadV1,
    blocks: &[GovernanceDagBlockV1],
) -> Result<(), GovernanceDagHeadChainValidationError> {
    head.validate()
        .map_err(GovernanceDagHeadChainValidationError::Head)?;
    validate_governance_dag_chain_v1(blocks, Some(&head.head_block_cid))
        .map_err(GovernanceDagHeadChainValidationError::Chain)?;

    let first = blocks
        .first()
        .ok_or(GovernanceDagHeadChainValidationError::Chain(
            GovernanceDagChainValidationError::Empty,
        ))?;
    if head.publisher_peer_id != first.publisher_peer_id {
        return Err(GovernanceDagHeadChainValidationError::PublisherPeerMismatch);
    }
    if head.head_signature.public_key != first.block_signature.public_key {
        return Err(GovernanceDagHeadChainValidationError::PublisherKeyMismatch);
    }
    let tip = blocks
        .last()
        .ok_or(GovernanceDagHeadChainValidationError::Chain(
            GovernanceDagChainValidationError::Empty,
        ))?;
    if head.generated_at < tip.timestamp {
        return Err(
            GovernanceDagHeadChainValidationError::HeadTimestampBeforeTip {
                head_generated_at: head.generated_at,
                tip_timestamp: tip.timestamp,
            },
        );
    }

    let chain_count = u64::try_from(blocks.len())
        .map_err(|_| GovernanceDagHeadChainValidationError::BlockCountOverflow)?;
    let window_count = u64::try_from(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
        .map_err(|_| GovernanceDagHeadChainValidationError::BlockCountOverflow)?;
    if first.sequence == 0 {
        if head.block_count != chain_count {
            return Err(GovernanceDagHeadChainValidationError::BlockCountMismatch {
                head_count: head.block_count,
                chain_count,
            });
        }
        if blocks.len() <= GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 {
            if head.checkpoint_cid.is_some() {
                return Err(GovernanceDagHeadChainValidationError::UnexpectedCheckpoint);
            }
        } else {
            let checkpoint_index = blocks
                .len()
                .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
                .ok_or(GovernanceDagHeadChainValidationError::BlockCountOverflow)?;
            let checkpoint = head
                .checkpoint_cid
                .as_deref()
                .ok_or(GovernanceDagHeadChainValidationError::MissingCheckpoint)?;
            if checkpoint != blocks[checkpoint_index].block_cid {
                return Err(GovernanceDagHeadChainValidationError::CheckpointMismatch);
            }
        }
    } else {
        if blocks.len() != GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 {
            return Err(
                GovernanceDagHeadChainValidationError::CheckpointWindowLength {
                    count: blocks.len(),
                },
            );
        }
        if head.block_count <= window_count {
            return Err(
                GovernanceDagHeadChainValidationError::InvalidCheckpointBlockCount {
                    block_count: head.block_count,
                },
            );
        }
        let expected = head
            .block_count
            .checked_sub(window_count)
            .ok_or(GovernanceDagHeadChainValidationError::BlockCountOverflow)?;
        if first.sequence != expected {
            return Err(
                GovernanceDagHeadChainValidationError::CheckpointStartSequence {
                    expected,
                    sequence: first.sequence,
                },
            );
        }
        let checkpoint = head
            .checkpoint_cid
            .as_deref()
            .ok_or(GovernanceDagHeadChainValidationError::MissingCheckpoint)?;
        if checkpoint != first.block_cid {
            return Err(GovernanceDagHeadChainValidationError::CheckpointMismatch);
        }
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

    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

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
        let mut node = GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: Vec::new(),
            prev_cid: Some([0x31; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
            timestamp: 1_700_000_300,
            publisher_peer_id: b"12D3KooWGovernancePeer".to_vec(),
            payload: signed_por_proof_payload(),
            publisher_signature: GovernanceLogSignatureV1 {
                algorithm: GovernanceSignatureAlgorithm::Dilithium3,
                public_key: vec![0x99; 64],
                signature: vec![0xAA; 160],
            },
        };
        node.node_cid = node
            .recompute_node_cid()
            .expect("derive canonical governance log node CID");
        node
    }

    #[test]
    fn governance_log_node_cid_is_stable_and_input_sensitive() {
        let payload = signed_por_proof_payload();
        let prev_cid = [0x41; GOVERNANCE_DAG_CID_BYTES_V1];
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

    #[test]
    fn governance_signature_validate_rejects_all_zero_material() {
        let mut signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: vec![0x11; 32],
            signature: vec![0; 64],
        };

        assert!(matches!(
            signature.validate(),
            Err(GovernanceLogValidationError::InvalidSignature)
        ));

        signature.signature = vec![0x22; 64];
        signature.public_key = vec![0; 32];

        assert!(matches!(
            signature.validate(),
            Err(GovernanceLogValidationError::InvalidSignature)
        ));
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
        prev_node_cid: Option<Vec<u8>>,
        sequence: u64,
        timestamp: u64,
    ) -> GovernanceDagBlockV1 {
        let mut node = governance_node_for_signing();
        node.prev_cid = prev_node_cid;
        node.timestamp = timestamp;
        let publisher_peer_id = b"12D3KooWGovernanceDagPublisher".to_vec();
        node.publisher_peer_id.clone_from(&publisher_peer_id);
        node.node_cid = node
            .recompute_node_cid()
            .expect("derive governance DAG node CID");
        sign_governance_node(&mut node, &[0xC7; 32]);

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
        let checkpoint_cid =
            (blocks.len() > GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1).then(|| {
                let checkpoint_index = blocks
                    .len()
                    .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
                    .expect("long fixture history contains a checkpoint window");
                blocks[checkpoint_index].block_cid.clone()
            });
        let mut head = GovernanceDagHeadV1 {
            version: GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid,
            block_count: u64::try_from(blocks.len()).expect("fixture block count fits u64"),
            generated_at: 1_700_001_000,
            publisher_peer_id: b"12D3KooWGovernanceDagPublisher".to_vec(),
            checkpoint_cid,
            head_signature: empty_ed25519_signature(),
        };
        sign_governance_head(&mut head, &[0xC7; 32]);
        head
    }

    fn signed_governance_chain(start_sequence: u64, count: usize) -> Vec<GovernanceDagBlockV1> {
        assert!(count > 0);
        let mut blocks = Vec::with_capacity(count);
        let mut prev_block_cid =
            (start_sequence > 0).then(|| [0x81; GOVERNANCE_DAG_CID_BYTES_V1].to_vec());
        let mut prev_node_cid =
            (start_sequence > 0).then(|| [0x82; GOVERNANCE_DAG_CID_BYTES_V1].to_vec());
        for offset in 0..count {
            let offset = u64::try_from(offset).expect("fixture offset fits u64");
            let sequence = start_sequence
                .checked_add(offset)
                .expect("fixture sequence does not overflow");
            let timestamp = 1_700_000_400_u64
                .checked_add(offset)
                .expect("fixture timestamp does not overflow");
            let block = signed_governance_block(prev_block_cid, prev_node_cid, sequence, timestamp);
            prev_block_cid = Some(block.block_cid.clone());
            prev_node_cid = Some(block.node.node_cid.clone());
            blocks.push(block);
        }
        blocks
    }
    use crate::deal::{
        DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
        DealSettlementStatusV1, DealSettlementV1, XorQuantity,
    };
    use crate::reputation::{
        REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationWeightsV1, build_reputation_snapshot,
        signed::{
            REPUTATION_SCORING_EVIDENCE_VERSION_V1, ReputationScoringEvidenceV1,
            ReputationSnapshotSignatureV1, SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
            SignedReputationSnapshotV1,
        },
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
            .stake_amount(
                crate::deal::XorQuantity::try_from_micro(1_000_000)
                    .expect("fixture stake is representable"),
            )
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

        let mut node = GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: Vec::new(),
            prev_cid: Some([0x32; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
            timestamp: 1_700_000_100,
            publisher_peer_id: b"12D3KooWGovernancePeer".to_vec(),
            payload: GovernanceLogPayloadV1::ProviderAdvert(advert),
            publisher_signature: GovernanceLogSignatureV1 {
                algorithm: GovernanceSignatureAlgorithm::Dilithium3,
                public_key: vec![11; 64],
                signature: vec![12; 160],
            },
        };
        node.node_cid = node
            .recompute_node_cid()
            .expect("derive governance log node CID");

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
    fn verify_publisher_signature_rejects_all_zero_ed25519_signature_material() {
        let seed = [0xA5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node(&mut node, &seed);
        node.publisher_signature.signature.fill(0);

        let err = node
            .verify_publisher_signature()
            .expect_err("all-zero governance node signature must be rejected");
        assert!(matches!(
            err,
            GovernanceLogSignatureVerificationError::Verification { reason }
                if reason.contains("all zero")
        ));
    }

    #[test]
    fn verify_publisher_signature_rejects_malformed_ed25519_signature_r() {
        for (label, replacement_r, expected_reason) in [
            ("small-order", SMALL_ORDER_R, "small-order"),
            ("noncanonical", NONCANONICAL_R, "not a canonical"),
        ] {
            let seed = [0xA5; 32];
            let mut node = governance_node_for_signing();
            sign_governance_node(&mut node, &seed);
            node.publisher_signature.signature[..PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);

            let err = node
                .verify_publisher_signature()
                .expect_err("malformed governance publisher signature R must be rejected");
            assert!(
                matches!(
                    &err,
                    GovernanceLogSignatureVerificationError::Verification { reason }
                        if reason.contains(expected_reason)
                ),
                "{label} signature R produced unexpected error: {err}"
            );
        }
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
    fn verify_publisher_signature_rejects_all_zero_dilithium3_signature_material() {
        let seed = [0xB5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node_mldsa(&mut node, &seed);
        node.publisher_signature.signature.fill(0);

        assert!(matches!(
            node.verify_publisher_signature(),
            Err(GovernanceLogSignatureVerificationError::Verification { .. })
        ));
    }

    #[test]
    fn verify_publisher_signature_rejects_malformed_dilithium3_signature_lengths() {
        for label in ["short", "overlong"] {
            let seed = [0xB6; 32];
            let mut node = governance_node_for_signing();
            sign_governance_node_mldsa(&mut node, &seed);
            match label {
                "short" => {
                    node.publisher_signature
                        .signature
                        .pop()
                        .expect("signed ML-DSA fixture is non-empty");
                }
                "overlong" => node.publisher_signature.signature.push(0xA5),
                _ => unreachable!("covered labels"),
            }

            let err = node
                .verify_publisher_signature()
                .expect_err("malformed ML-DSA governance signature length must be rejected");
            assert!(
                matches!(
                    &err,
                    GovernanceLogSignatureVerificationError::Verification { reason }
                        if reason.contains("signature")
                ),
                "{label} ML-DSA governance signature produced unexpected error: {err}"
            );
        }
    }

    #[test]
    fn governance_dag_block_derives_cid_and_verifies_signature() {
        let block = signed_governance_block(None, None, 0, 1_700_000_400);

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
    fn governance_dag_block_rejects_all_zero_signature_material() {
        let mut block = signed_governance_block(None, None, 0, 1_700_000_400);
        block.block_signature.signature.fill(0);

        let err = block
            .verify_block_signature()
            .expect_err("all-zero governance block signature must be rejected");
        assert!(matches!(
            err,
            GovernanceLogSignatureVerificationError::Verification { reason }
                if reason.contains("all zero")
        ));
    }

    #[test]
    fn governance_dag_block_signature_payload_excludes_signature() {
        let block = signed_governance_block(None, None, 0, 1_700_000_400);
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
        let mut block = signed_governance_block(None, None, 0, 1_700_000_400);
        block.block_cid[0] ^= 0x01;

        assert!(matches!(
            block.validate(),
            Err(GovernanceDagBlockValidationError::InvalidBlockCid)
        ));
    }

    #[test]
    fn governance_log_node_requires_exact_cids_and_bounded_peer_id() {
        let mut node = governance_node_for_signing();
        node.node_cid.pop();
        assert!(matches!(
            node.validate(),
            Err(GovernanceLogValidationError::InvalidNodeCidLength { length: 31 })
        ));

        let mut node = governance_node_for_signing();
        node.prev_cid = Some(vec![0x41; GOVERNANCE_DAG_CID_BYTES_V1 + 1]);
        assert!(matches!(
            node.validate(),
            Err(GovernanceLogValidationError::InvalidPrevCidLength { length: 33 })
        ));

        let mut node = governance_node_for_signing();
        node.publisher_peer_id = vec![0x42; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1];
        assert!(matches!(
            node.validate(),
            Err(GovernanceLogValidationError::PublisherPeerIdTooLong { .. })
        ));
    }

    #[test]
    fn governance_log_node_rejects_noncanonical_cid() {
        let mut node = governance_node_for_signing();
        node.node_cid[0] ^= 0x01;

        assert!(matches!(
            node.validate(),
            Err(GovernanceLogValidationError::InvalidNodeCid)
        ));
    }

    #[test]
    fn governance_dag_block_requires_exact_cids_and_one_ed25519_identity() {
        let block = signed_governance_block(None, None, 0, 1_700_000_400);

        let mut invalid_cid = block.clone();
        invalid_cid.block_cid.push(0);
        assert!(matches!(
            invalid_cid.validate(),
            Err(GovernanceDagBlockValidationError::InvalidBlockCidLength { length: 33 })
        ));

        let mut invalid_prev = signed_governance_block(
            Some([0x61; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
            Some([0x62; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
            1,
            1_700_000_401,
        );
        invalid_prev.prev_block_cid = Some(vec![0x61; 31]);
        assert!(matches!(
            invalid_prev.validate(),
            Err(GovernanceDagBlockValidationError::InvalidPrevBlockCidLength { length: 31 })
        ));

        let mut oversized_peer = block.clone();
        oversized_peer.publisher_peer_id =
            vec![0x42; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1];
        assert!(matches!(
            oversized_peer.validate(),
            Err(GovernanceDagBlockValidationError::PublisherPeerIdTooLong { .. })
        ));

        let mut invalid_algorithm = block.clone();
        invalid_algorithm.block_signature.algorithm = GovernanceSignatureAlgorithm::Dilithium3;
        assert!(matches!(
            invalid_algorithm.validate(),
            Err(GovernanceDagBlockValidationError::NonEd25519BlockSignature)
        ));

        let mut invalid_peer = block.clone();
        invalid_peer.node.publisher_peer_id[0] ^= 0x01;
        invalid_peer.node.node_cid = invalid_peer
            .node
            .recompute_node_cid()
            .expect("recompute node CID");
        sign_governance_node(&mut invalid_peer.node, &[0xC7; 32]);
        assert!(matches!(
            invalid_peer.validate(),
            Err(GovernanceDagBlockValidationError::NodePublisherPeerMismatch)
        ));

        let mut invalid_key = block;
        sign_governance_node(&mut invalid_key.node, &[0xD7; 32]);
        invalid_key.block_cid = invalid_key
            .recompute_block_cid()
            .expect("recompute block CID");
        sign_governance_block(&mut invalid_key, &[0xC7; 32]);
        assert!(matches!(
            invalid_key.validate(),
            Err(GovernanceDagBlockValidationError::NodePublisherKeyMismatch)
        ));
    }

    #[test]
    fn governance_dag_head_requires_exact_cids_bounded_peer_and_ed25519() {
        let blocks = signed_governance_chain(0, 1);
        let head = signed_governance_head(&blocks);

        let mut invalid_cid = head.clone();
        invalid_cid.head_block_cid.pop();
        assert!(matches!(
            invalid_cid.validate(),
            Err(GovernanceDagHeadValidationError::InvalidHeadBlockCidLength { length: 31 })
        ));

        let mut invalid_checkpoint = head.clone();
        invalid_checkpoint.checkpoint_cid = Some(vec![0x55; 31]);
        assert!(matches!(
            invalid_checkpoint.validate(),
            Err(GovernanceDagHeadValidationError::InvalidCheckpointCidLength { length: 31 })
        ));

        let mut oversized_peer = head.clone();
        oversized_peer.publisher_peer_id =
            vec![0x44; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1];
        assert!(matches!(
            oversized_peer.validate(),
            Err(GovernanceDagHeadValidationError::PublisherPeerIdTooLong { .. })
        ));

        let mut invalid_algorithm = head;
        invalid_algorithm.head_signature.algorithm = GovernanceSignatureAlgorithm::Dilithium3;
        assert!(matches!(
            invalid_algorithm.validate(),
            Err(GovernanceDagHeadValidationError::NonEd25519HeadSignature)
        ));
    }

    #[test]
    fn governance_dag_chain_validates_parent_linkage_and_head() {
        let root = signed_governance_block(None, None, 0, 1_700_000_400);
        let child = signed_governance_block(
            Some(root.block_cid.clone()),
            Some(root.node.node_cid.clone()),
            1,
            1_700_000_500,
        );
        let blocks = vec![root, child];
        let expected_head = blocks[1].block_cid.clone();

        validate_governance_dag_chain_v1(&blocks, Some(&expected_head))
            .expect("valid governance DAG chain");
    }

    #[test]
    fn governance_dag_chain_accepts_external_tail_anchors() {
        let block =
            signed_governance_block(Some(vec![0xA5; 32]), Some(vec![0x5A; 32]), 1, 1_700_000_500);

        validate_governance_dag_chain_v1(&[block], None)
            .expect("the first checkpoint-tail block may reference external parents");
    }

    #[test]
    fn governance_dag_chain_rejects_noncanonical_order() {
        let root = signed_governance_block(None, None, 0, 1_700_000_400);
        let child = signed_governance_block(
            Some(root.block_cid.clone()),
            Some(root.node.node_cid.clone()),
            1,
            1_700_000_500,
        );
        let blocks = vec![child, root];

        assert!(matches!(
            validate_governance_dag_chain_v1(&blocks, None),
            Err(GovernanceDagChainValidationError::NonCanonicalOrder { index: 1 })
        ));
    }

    #[test]
    fn governance_dag_chain_rejects_duplicate_node_cid() {
        let first = signed_governance_block(None, None, 0, 1_700_000_400);
        let mut duplicate = first.clone();
        duplicate.timestamp = duplicate
            .timestamp
            .checked_add(1)
            .expect("fixture timestamp does not overflow");
        duplicate.block_cid = duplicate
            .recompute_block_cid()
            .expect("recompute duplicate-node block CID");
        sign_governance_block(&mut duplicate, &[0xC7; 32]);

        assert!(matches!(
            validate_governance_dag_chain_v1(&[first, duplicate], None),
            Err(GovernanceDagChainValidationError::DuplicateNodeCid { index: 1 })
        ));
    }

    #[test]
    fn governance_dag_chain_rejects_node_parent_discontinuity() {
        let root = signed_governance_block(None, None, 0, 1_700_000_400);
        let child = signed_governance_block(
            Some(root.block_cid.clone()),
            Some([0x52; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
            1,
            1_700_000_500,
        );

        assert!(matches!(
            validate_governance_dag_chain_v1(&[root, child], None),
            Err(GovernanceDagChainValidationError::NodeParentMismatch { index: 1 })
        ));
    }

    #[test]
    fn governance_dag_chain_rejects_publisher_peer_or_key_drift() {
        let blocks = signed_governance_chain(0, 2);

        let mut peer_drift = blocks.clone();
        let child = &mut peer_drift[1];
        child.publisher_peer_id = b"12D3KooWGovernanceDagPublisherOther".to_vec();
        child
            .node
            .publisher_peer_id
            .clone_from(&child.publisher_peer_id);
        child.node.node_cid = child
            .node
            .recompute_node_cid()
            .expect("recompute peer-drift node CID");
        sign_governance_node(&mut child.node, &[0xC7; 32]);
        child.block_cid = child
            .recompute_block_cid()
            .expect("recompute peer-drift block CID");
        sign_governance_block(child, &[0xC7; 32]);
        assert!(matches!(
            validate_governance_dag_chain_v1(&peer_drift, None),
            Err(GovernanceDagChainValidationError::PublisherPeerMismatch { index: 1 })
        ));

        let mut key_drift = blocks;
        let child = &mut key_drift[1];
        sign_governance_node(&mut child.node, &[0xD7; 32]);
        child.block_cid = child
            .recompute_block_cid()
            .expect("recompute key-drift block CID");
        sign_governance_block(child, &[0xD7; 32]);
        assert!(matches!(
            validate_governance_dag_chain_v1(&key_drift, None),
            Err(GovernanceDagChainValidationError::PublisherKeyMismatch { index: 1 })
        ));
    }

    #[test]
    fn governance_dag_chain_rejects_sequence_overflow() {
        let first = signed_governance_block(
            Some([0x91; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
            Some([0x92; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
            u64::MAX,
            1_700_000_400,
        );
        let second = signed_governance_block(
            Some(first.block_cid.clone()),
            Some(first.node.node_cid.clone()),
            u64::MAX,
            1_700_000_401,
        );

        assert!(matches!(
            validate_governance_dag_chain_v1(&[first, second], None),
            Err(GovernanceDagChainValidationError::SequenceOverflow { index: 1 })
        ));
    }

    #[test]
    fn governance_dag_head_manifest_signs_and_binds_chain() {
        let root = signed_governance_block(None, None, 0, 1_700_000_400);
        let child = signed_governance_block(
            Some(root.block_cid.clone()),
            Some(root.node.node_cid.clone()),
            1,
            1_700_000_500,
        );
        let blocks = vec![root, child];
        let head = signed_governance_head(&blocks);

        head.validate().expect("valid governance DAG head");
        head.verify_head_signature()
            .expect("head signature verifies");
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("head binds the governance DAG chain");
    }

    #[test]
    fn governance_dag_head_binds_full_history_checkpoint_window() {
        let blocks = signed_governance_chain(
            0,
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
                .checked_add(1)
                .expect("fixture count does not overflow"),
        );
        let head = signed_governance_head(&blocks);
        let checkpoint_index = blocks
            .len()
            .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
            .expect("full history contains checkpoint window");

        assert_eq!(
            head.checkpoint_cid.as_deref(),
            Some(blocks[checkpoint_index].block_cid.as_slice())
        );
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("full history binds its newest checkpoint window");
    }

    #[test]
    fn governance_dag_head_accepts_exact_checkpoint_tail() {
        let full = signed_governance_chain(
            0,
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
                .checked_add(1)
                .expect("fixture count does not overflow"),
        );
        let head = signed_governance_head(&full);
        let tail_start = full
            .len()
            .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
            .expect("full history contains checkpoint window");
        let tail = &full[tail_start..];

        validate_governance_dag_head_against_chain_v1(&head, tail)
            .expect("exact newest checkpoint tail binds the signed head");
    }

    #[test]
    fn governance_dag_head_rejects_short_or_misanchored_checkpoint_tail() {
        let full = signed_governance_chain(
            0,
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
                .checked_add(1)
                .expect("fixture count does not overflow"),
        );
        let mut head = signed_governance_head(&full);
        let tail_start = full
            .len()
            .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
            .expect("full history contains checkpoint window");
        let tail = &full[tail_start..];

        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, &tail[1..]),
            Err(GovernanceDagHeadChainValidationError::CheckpointWindowLength { count: 63 })
        ));

        head.checkpoint_cid = Some([0xE5; GOVERNANCE_DAG_CID_BYTES_V1].to_vec());
        sign_governance_head(&mut head, &[0xC7; 32]);
        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, tail),
            Err(GovernanceDagHeadChainValidationError::CheckpointMismatch)
        ));
    }

    #[test]
    fn governance_dag_head_rejects_checkpoint_tail_sequence_mismatch() {
        let full = signed_governance_chain(
            0,
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
                .checked_add(1)
                .expect("fixture count does not overflow"),
        );
        let mut head = signed_governance_head(&full);
        head.block_count = head
            .block_count
            .checked_add(1)
            .expect("fixture block count does not overflow");
        sign_governance_head(&mut head, &[0xC7; 32]);
        let tail_start = full
            .len()
            .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
            .expect("full history contains checkpoint window");

        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, &full[tail_start..]),
            Err(
                GovernanceDagHeadChainValidationError::CheckpointStartSequence {
                    expected: 2,
                    sequence: 1
                }
            )
        ));

        let mut missing_generated_at = head.clone();
        missing_generated_at.generated_at = 0;
        assert!(matches!(
            missing_generated_at.validate(),
            Err(GovernanceDagHeadValidationError::MissingGeneratedAt)
        ));
    }

    #[test]
    fn governance_dag_head_rejects_checkpoint_for_short_full_history() {
        let blocks = signed_governance_chain(0, 2);
        let mut head = signed_governance_head(&blocks);
        head.checkpoint_cid = Some(blocks[0].block_cid.clone());
        sign_governance_head(&mut head, &[0xC7; 32]);

        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, &blocks),
            Err(GovernanceDagHeadChainValidationError::UnexpectedCheckpoint)
        ));
    }

    #[test]
    fn governance_dag_head_rejects_signer_identity_drift() {
        let blocks = signed_governance_chain(0, 2);
        let mut head = signed_governance_head(&blocks);
        sign_governance_head(&mut head, &[0xD9; 32]);

        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, &blocks),
            Err(GovernanceDagHeadChainValidationError::PublisherKeyMismatch)
        ));

        let mut head = signed_governance_head(&blocks);
        head.publisher_peer_id = b"12D3KooWGovernanceDagPublisherOther".to_vec();
        sign_governance_head(&mut head, &[0xC7; 32]);
        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, &blocks),
            Err(GovernanceDagHeadChainValidationError::PublisherPeerMismatch)
        ));
    }

    #[test]
    fn governance_dag_head_rejects_generated_at_before_tip() {
        let blocks = signed_governance_chain(0, 2);
        let mut head = signed_governance_head(&blocks);
        let tip_timestamp = blocks.last().expect("fixture has tip").timestamp;
        head.generated_at = tip_timestamp
            .checked_sub(1)
            .expect("fixture tip timestamp is positive");
        sign_governance_head(&mut head, &[0xC7; 32]);

        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, &blocks),
            Err(
                GovernanceDagHeadChainValidationError::HeadTimestampBeforeTip {
                    head_generated_at,
                    tip_timestamp: observed_tip
                }
            ) if head_generated_at.checked_add(1) == Some(observed_tip)
        ));
    }

    #[test]
    fn governance_dag_head_rejects_all_zero_signature_material() {
        let root = signed_governance_block(None, None, 0, 1_700_000_400);
        let blocks = vec![root];
        let mut head = signed_governance_head(&blocks);
        head.head_signature.signature.fill(0);

        let err = head
            .verify_head_signature()
            .expect_err("all-zero governance head signature must be rejected");
        assert!(matches!(
            err,
            GovernanceLogSignatureVerificationError::Verification { reason }
                if reason.contains("all zero")
        ));
    }

    #[test]
    fn governance_dag_head_rejects_block_count_mismatch() {
        let root = signed_governance_block(None, None, 0, 1_700_000_400);
        let blocks = vec![root];
        let mut head = signed_governance_head(&blocks);
        head.block_count += 1;
        sign_governance_head(&mut head, &[0xC7; 32]);

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
        let xor_nanos = |value: u128| -> XorQuantity {
            let whole = value / 1_000_000_000;
            let fractional = value % 1_000_000_000;
            format!("{whole}.{fractional:09}")
                .parse()
                .expect("nano-XOR fixture is canonical")
        };
        let mut ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: 1,
            previous_snapshot_id: None,
            deal_id: [0xAA; 32],
            terms_digest: [0x44; 32],
            provider_id: [0xBB; 32],
            client_id: [0xCC; 32],
            deal_start_epoch: 1_700_199_900,
            deal_end_epoch: 1_700_199_999,
            settlement_window_epochs: 100,
            window_start_epoch: 1_700_199_900,
            window_end_epoch: 1_700_200_000,
            provider_accrual: xor_nanos(100),
            client_liability: xor_nanos(100),
            micropayment_credit_generated: XorQuantity::zero(),
            micropayment_credit_applied: XorQuantity::zero(),
            micropayment_credit_carry: XorQuantity::zero(),
            client_debit: xor_nanos(100),
            outstanding_liability: XorQuantity::zero(),
            bond_total: xor_nanos(50),
            bond_locked: XorQuantity::zero(),
            bond_slashed: XorQuantity::zero(),
            bond_released: xor_nanos(50),
            window_expected_charge: xor_nanos(100),
            window_micropayment_generated: XorQuantity::zero(),
            window_micropayment_applied: XorQuantity::zero(),
            window_client_debit: xor_nanos(100),
            window_bond_slashed: XorQuantity::zero(),
            window_bond_released: xor_nanos(50),
            captured_at: 1_700_200_000,
        };
        ledger.snapshot_id = ledger.derive_snapshot_id().expect("ledger id");
        let mut settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            settlement_id: [0; 32],
            deal_id: [0xAA; 32],
            ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at: 1_700_200_000,
            audit_notes: None,
        };
        settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
        let payload = GovernanceLogPayloadV1::DealSettlement(Box::new(settlement));
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
        let inputs = vec![input];
        let snapshot = build_reputation_snapshot(
            [0x42; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &inputs,
            None,
        )
        .expect("reputation snapshot");
        let scoring_evidence = ReputationScoringEvidenceV1 {
            version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
            provider_inputs: inputs,
            trust_edges: Vec::new(),
        };
        let mut envelope = SignedReputationSnapshotV1 {
            version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
            policy_digest: [0xA5; 32],
            snapshot,
            scoring_evidence_digest: scoring_evidence
                .canonical_digest()
                .expect("scoring evidence digest"),
            scoring_evidence,
            signatures: Vec::new(),
        };
        let signing_key = SigningKey::from_bytes(&[0x5A; 32]);
        envelope.signatures.push(ReputationSnapshotSignatureV1 {
            signer_id: "council-1".to_owned(),
            signature: signing_key
                .sign(&envelope.signing_digest().expect("signing digest"))
                .to_bytes(),
        });
        let payload = GovernanceLogPayloadV1::SignedReputationSnapshot(envelope);

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
            challenge_count: 0,
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
            challenge: None,
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
            deposit_xor: "420".parse().expect("canonical XOR quantity"),
            refund: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "refund-account".to_string(),
                amount_xor: "420".parse().expect("canonical XOR quantity"),
            },
            treasury: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "treasury-account".to_string(),
                amount_xor: "50".parse().expect("canonical XOR quantity"),
            },
            held: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "escrow-account".to_string(),
                amount_xor: "0".parse().expect("canonical XOR quantity"),
            },
            panel_size: 3,
            panel_reward_total_xor: "85".parse().expect("canonical XOR quantity"),
            rewards_paid_total_xor: "60".parse().expect("canonical XOR quantity"),
            rewards_forfeited_treasury_xor: "25".parse().expect("canonical XOR quantity"),
            juror_payouts: vec![
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-a".to_string(),
                    stipend_xor: "25".parse().expect("canonical XOR quantity"),
                    bonus_xor: "5".parse().expect("canonical XOR quantity"),
                    total_xor: "30".parse().expect("canonical XOR quantity"),
                },
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-b".to_string(),
                    stipend_xor: "25".parse().expect("canonical XOR quantity"),
                    bonus_xor: "5".parse().expect("canonical XOR quantity"),
                    total_xor: "30".parse().expect("canonical XOR quantity"),
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

    fn sample_external_payload() -> GovernanceExternalPayloadV1 {
        let cycle_id = *b"cycle-2026-wk-01";
        let entry = crate::transparency::ModerationLedgerEntryV1 {
            version: crate::transparency::MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id,
            entry_id: [0x11; 16],
            sequence: 1,
            occurred_at_unix: 1_800_000_001,
            kind: crate::transparency::ModerationLedgerEntryKindV1::GarEnforcementReceipt,
            subject: "gar-receipt-1".to_owned(),
            subject_digest: [0x21; 32],
            payload_digest: [0x22; 32],
            summary_digest: [0x23; 32],
            policy_digest: Some([0x24; 32]),
            evidence_uris: vec!["sora://transparency/gar-receipt-1".to_owned()],
            metadata: Vec::new(),
        };
        let publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id,
            1_800_000_000,
            1_800_000_010,
            1_800_000_011,
            None,
            &[entry],
        )
        .expect("build transparency publication");
        let encoded_payload = norito::to_bytes(&publication).expect("encode publication");
        GovernanceExternalPayloadV1::from_transparency_ledger_publication(
            &publication,
            &encoded_payload,
        )
        .expect("wrap transparency publication")
    }

    #[test]
    fn governance_payload_accepts_external_payload() {
        let payload = GovernanceLogPayloadV1::ExternalPayload(sample_external_payload());

        payload
            .validate(1_800_000_031)
            .expect("valid external governance payload");
    }

    #[test]
    fn external_payload_rejects_digest_mismatch() {
        let mut payload = sample_external_payload();
        payload.encoded_blake3[0] ^= 0xFF;

        let err = payload.validate().expect_err("digest mismatch rejected");
        assert_eq!(
            err,
            GovernanceExternalPayloadValidationError::EncodedDigestMismatch
        );
        let err = GovernanceLogPayloadV1::ExternalPayload(payload)
            .validate(1_800_000_031)
            .expect_err("governance payload rejects digest mismatch");
        assert!(matches!(
            err,
            GovernanceLogValidationError::ExternalPayload(
                GovernanceExternalPayloadValidationError::EncodedDigestMismatch
            )
        ));
    }

    #[test]
    fn external_payload_rejects_unsorted_metadata() {
        let mut payload = sample_external_payload();
        payload.metadata.swap(0, 1);

        let err = payload
            .validate()
            .expect_err("metadata ordering is canonical");
        assert_eq!(
            err,
            GovernanceExternalPayloadValidationError::MetadataKeysUnsorted
        );
    }

    #[test]
    fn external_payload_rejects_unknown_kind_and_version() {
        let mut payload = sample_external_payload();
        payload.payload_kind = "arbitrary_payload".to_owned();
        assert!(matches!(
            payload.validate(),
            Err(GovernanceExternalPayloadValidationError::UnsupportedPayloadKind {
                payload_kind
            }) if payload_kind == "arbitrary_payload"
        ));

        let mut payload = sample_external_payload();
        payload.payload_version = MODERATION_LEDGER_PUBLICATION_VERSION_V1 + 1;
        assert!(matches!(
            payload.validate(),
            Err(GovernanceExternalPayloadValidationError::UnsupportedPayloadVersion {
                payload_kind,
                expected: MODERATION_LEDGER_PUBLICATION_VERSION_V1,
                found
            }) if payload_kind == GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1
                && found == MODERATION_LEDGER_PUBLICATION_VERSION_V1 + 1
        ));
    }

    #[test]
    fn external_payload_rejects_oversized_and_trailing_payload_bytes() {
        let mut oversized = sample_external_payload();
        oversized.encoded_payload = vec![0xA5; SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_MAX_BYTES_V1 + 1];
        oversized.encoded_len = oversized.encoded_payload.len() as u64;
        oversized.encoded_blake3 = *blake3::hash(&oversized.encoded_payload).as_bytes();
        assert!(matches!(
            oversized.validate(),
            Err(GovernanceExternalPayloadValidationError::EncodedPayloadTooLarge { .. })
        ));

        let mut trailing = sample_external_payload();
        trailing.encoded_payload.push(0);
        trailing.encoded_len = trailing.encoded_payload.len() as u64;
        trailing.encoded_blake3 = *blake3::hash(&trailing.encoded_payload).as_bytes();
        assert!(matches!(
            trailing.validate(),
            Err(GovernanceExternalPayloadValidationError::TypedPayloadDecode { .. })
        ));
    }

    #[test]
    fn external_payload_rejects_noncanonical_compressed_encoding() {
        let mut payload = sample_external_payload();
        let publication: ModerationLedgerCyclePublicationV1 =
            norito::decode_from_bytes(&payload.encoded_payload).expect("decode publication");
        let compressed =
            norito::to_compressed_bytes(&publication, Some(norito::CompressionConfig::default()))
                .expect("compress publication");
        assert_ne!(compressed, payload.encoded_payload);
        payload.encoded_len = compressed.len() as u64;
        payload.encoded_blake3 = *blake3::hash(&compressed).as_bytes();
        payload.encoded_payload = compressed;
        assert!(matches!(
            payload.validate(),
            Err(GovernanceExternalPayloadValidationError::NonCanonicalEncodedPayload { .. })
        ));
    }

    #[test]
    fn external_payload_rejects_invalid_typed_payload() {
        let mut payload = sample_external_payload();
        let mut publication: ModerationLedgerCyclePublicationV1 =
            norito::decode_from_bytes(&payload.encoded_payload).expect("decode publication");
        publication.version = MODERATION_LEDGER_PUBLICATION_VERSION_V1 + 1;
        payload.encoded_payload =
            norito::to_bytes(&publication).expect("encode invalid publication");
        payload.encoded_len = payload.encoded_payload.len() as u64;
        payload.encoded_blake3 = *blake3::hash(&payload.encoded_payload).as_bytes();
        assert!(matches!(
            payload.validate(),
            Err(GovernanceExternalPayloadValidationError::InvalidTypedPayload { .. })
        ));
    }

    #[test]
    fn external_payload_rejects_metadata_count_key_value_duplicate_and_mismatch() {
        let mut too_many = sample_external_payload();
        too_many.metadata = (0..=SORAFS_GOVERNANCE_EXTERNAL_METADATA_MAX_ENTRIES_V1)
            .map(|index| GovernanceExternalPayloadMetadataV1 {
                key: format!("k{index:02}"),
                value: "v".to_owned(),
            })
            .collect();
        assert!(matches!(
            too_many.validate(),
            Err(GovernanceExternalPayloadValidationError::MetadataCountTooLarge { .. })
        ));

        let mut long_key = sample_external_payload();
        long_key.metadata.last_mut().expect("metadata").key =
            "z".repeat(SORAFS_GOVERNANCE_EXTERNAL_METADATA_KEY_MAX_BYTES_V1 + 1);
        assert!(matches!(
            long_key.validate(),
            Err(GovernanceExternalPayloadValidationError::MetadataKeyTooLong { .. })
        ));

        let mut long_value = sample_external_payload();
        long_value.metadata[0].value =
            "v".repeat(SORAFS_GOVERNANCE_EXTERNAL_METADATA_VALUE_MAX_BYTES_V1 + 1);
        assert!(matches!(
            long_value.validate(),
            Err(GovernanceExternalPayloadValidationError::MetadataValueTooLong { .. })
        ));

        let mut total_too_large = sample_external_payload();
        total_too_large.metadata = (0..SORAFS_GOVERNANCE_EXTERNAL_METADATA_MAX_ENTRIES_V1)
            .map(|index| GovernanceExternalPayloadMetadataV1 {
                key: format!("key-{index:02}"),
                value: "v".repeat(SORAFS_GOVERNANCE_EXTERNAL_METADATA_VALUE_MAX_BYTES_V1),
            })
            .collect();
        assert!(matches!(
            total_too_large.validate(),
            Err(GovernanceExternalPayloadValidationError::MetadataBytesTooLarge { .. })
        ));

        let mut duplicate = sample_external_payload();
        duplicate.metadata.insert(1, duplicate.metadata[0].clone());
        assert!(matches!(
            duplicate.validate(),
            Err(GovernanceExternalPayloadValidationError::DuplicateMetadataKey { .. })
        ));

        let mut mismatch = sample_external_payload();
        mismatch.metadata[0].value = "00".repeat(32);
        assert!(matches!(
            mismatch.validate(),
            Err(GovernanceExternalPayloadValidationError::MetadataMismatch { .. })
        ));
    }

    #[test]
    fn external_repair_slash_rejects_embedded_approval() {
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: crate::repair::RepairTicketId("REP-351".to_owned()),
            provider_id: [0x31; 32],
            manifest_digest: [0x32; 32],
            auditor_account: "auditor@sorafs".to_owned(),
            proposed_penalty: XorQuantity::try_from_micro(1)
                .expect("legacy nano-XOR penalty is representable"),
            submitted_at_unix: 1_800_000_001,
            rationale: "repeated proof failures".to_owned(),
            approval: Some(crate::repair::RepairEscalationApprovalV1 {
                version: crate::repair::REPAIR_ESCALATION_APPROVAL_VERSION_V1,
                approve_votes: 2,
                reject_votes: 1,
                abstain_votes: 0,
                approved_at_unix: 1_800_000_002,
                finalized_at_unix: 1_800_000_003,
            }),
        };
        let encoded = norito::to_bytes(&proposal).expect("encode slash proposal");
        assert_eq!(
            GovernanceExternalPayloadV1::from_repair_slash(
                &proposal,
                GovernanceExternalRepairSlashStageV1::Submitted,
                &encoded,
            ),
            Err(GovernanceExternalPayloadValidationError::RepairSlashApprovalForbidden)
        );
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

    fn sample_appeal_finance_settlement_receipt() -> SoraFsAppealFinanceSettlementReceiptV1 {
        SoraFsAppealFinanceSettlementReceiptV1 {
            version: SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: [0x52; 16],
            case_id: "case-42".to_string(),
            round_id: Some("round-1".to_string()),
            generated_at_unix_ms: 1_800_000_032_000,
            appeal_finance_config_version: "baseline-v1".to_string(),
            outcome: SoraFsAppealFinanceOutcomeV1::Frivolous,
            escrow_id_hex: "11".repeat(32),
            payer_account: "payer-account".to_string(),
            destination_account: "escrow-account".to_string(),
            release_authority_account: Some("release-authority".to_string()),
            submitted_step: "drawdown_non_refund".to_string(),
            required_authority: "release-authority".to_string(),
            amount_xor: "420".parse().expect("canonical XOR quantity"),
            tx_hash_hex: "22".repeat(32),
            reconciliation_digest_hex: "33".repeat(32),
            reconciliation_status: "pending_client_submission".to_string(),
            observed_lifecycle_status: "locked".to_string(),
            observed_remaining_xor: "420".parse().expect("canonical XOR quantity"),
            deposit_xor: "420".parse().expect("canonical XOR quantity"),
            refund_xor: "0".parse().expect("canonical XOR quantity"),
            treasury_xor: "210".parse().expect("canonical XOR quantity"),
            held_xor: "210".parse().expect("canonical XOR quantity"),
            panel_size: 7,
            configured_signer_count: 1,
        }
    }

    #[test]
    fn governance_payload_accepts_appeal_finance_settlement_receipt() {
        let receipt = sample_appeal_finance_settlement_receipt();
        let payload = GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(receipt);

        payload
            .validate(1_800_000_032)
            .expect("settlement receipt payload validates");
    }

    fn sample_orderbook_settlement_receipt() -> SettlementReceiptV1 {
        SettlementReceiptV1 {
            version: crate::SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: [0x72; 32],
            channel_id: [0x73; 32],
            trade_id: [0x74; 32],
            range: crate::ByteRangeV1 {
                start: 0,
                end: crate::BYTES_PER_GIB,
            },
            chunk_hash: [0x75; 32],
            bytes_delivered: crate::BYTES_PER_GIB,
            xor_debited: crate::XorQuantity::try_from_micro(500)
                .expect("legacy micro-XOR value is representable"),
            provider_credit: crate::XorQuantity::try_from_micro(450)
                .expect("legacy micro-XOR value is representable"),
            fee_amount: crate::XorQuantity::try_from_micro(50)
                .expect("legacy micro-XOR value is representable"),
            issued_at_unix: 1_800_000_033,
            settlement_signature: crate::OrderbookSignatureV1 {
                algorithm: crate::provider_advert::SignatureAlgorithm::Ed25519,
                public_key: vec![0x76; 32],
                signature: vec![0x77; 64],
            },
        }
    }

    #[test]
    fn governance_payload_accepts_orderbook_settlement_receipt() {
        let receipt = sample_orderbook_settlement_receipt();
        let payload = GovernanceLogPayloadV1::OrderbookSettlementReceipt(receipt);

        payload
            .validate(1_800_000_033)
            .expect("orderbook settlement receipt payload validates");
    }

    #[test]
    fn appeal_finance_settlement_receipt_rejects_invalid_reconciliation_digest() {
        let mut receipt = sample_appeal_finance_settlement_receipt();
        receipt.reconciliation_digest_hex = "AA".repeat(32);

        let err = receipt
            .validate()
            .expect_err("uppercase digest rejected as non-canonical");
        assert_eq!(
            err,
            SoraFsAppealFinanceSettlementReceiptValidationError::InvalidHex {
                field: "reconciliation_digest_hex",
                expected_bytes: 32,
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
            deposit_xor: "80.25".parse().expect("canonical XOR quantity"),
            refund: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "refund-account".to_string(),
                amount_xor: "0.25".parse().expect("canonical XOR quantity"),
            },
            treasury: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "treasury-account".to_string(),
                amount_xor: "80".parse().expect("canonical XOR quantity"),
            },
            held: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "escrow-account".to_string(),
                amount_xor: "0.00".parse().expect("canonical XOR quantity"),
            },
            panel_size: 1,
            panel_reward_total_xor: "30".parse().expect("canonical XOR quantity"),
            rewards_paid_total_xor: "30".parse().expect("canonical XOR quantity"),
            rewards_forfeited_treasury_xor: "0".parse().expect("canonical XOR quantity"),
            juror_payouts: vec![SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-d".to_string(),
                stipend_xor: "25".parse().expect("canonical XOR quantity"),
                bonus_xor: "5".parse().expect("canonical XOR quantity"),
                total_xor: "30".parse().expect("canonical XOR quantity"),
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
        assert_eq!(rollup.total_deposit_xor.to_string(), "500.25");
        assert_eq!(rollup.total_refund_xor.to_string(), "420.25");
        assert_eq!(rollup.total_treasury_xor.to_string(), "130");
        assert_eq!(rollup.total_held_xor.to_string(), "0");
        assert_eq!(rollup.total_panel_reward_xor.to_string(), "115");
        assert_eq!(rollup.total_rewards_paid_xor.to_string(), "90");
        assert_eq!(
            rollup.total_rewards_forfeited_treasury_xor.to_string(),
            "25"
        );
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
    fn appeal_finance_weekly_rollup_build_rejects_quantity_overflow() {
        let mut first = sample_appeal_finance_report();
        first.deposit_xor =
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
                .parse()
                .expect("maximum positive quantity");
        let mut second = second_appeal_finance_report();
        second.deposit_xor = "1".parse().expect("canonical quantity");

        let err = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
            PorReportIsoWeek {
                year: 2026,
                week: 26,
            },
            1_800_000_100_000,
            &[first, second.clone()],
        )
        .expect_err("overflowing report totals must fail closed");

        assert!(matches!(
            err,
            SoraFsAppealFinanceWeeklyRollupBuildError::AmountOverflow {
                report_id,
                field: "deposit_xor",
                ..
            } if report_id == second.report_id
        ));
    }

    #[test]
    fn appeal_finance_weekly_rollup_validation_rejects_outcome_overflow() {
        let first = sample_appeal_finance_report();
        let second = second_appeal_finance_report();
        let mut rollup = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
            PorReportIsoWeek {
                year: 2026,
                week: 26,
            },
            1_800_000_100_000,
            &[first, second],
        )
        .expect("baseline rollup");
        rollup.outcomes[0].total_deposit_xor =
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
                .parse()
                .expect("maximum positive quantity");
        rollup.outcomes[1].total_deposit_xor = "1".parse().expect("canonical quantity");

        assert!(matches!(
            rollup.validate(),
            Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::AmountOverflow {
                    field: "outcomes.total_deposit_xor",
                    ..
                }
            )
        ));
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
            challenge_count: 0,
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
            challenge: None,
        };

        assert!(matches!(
            event.validate(),
            Err(SoraFsModerationBallotGovernanceEventValidationError::TallyRoundMismatch { .. })
        ));
    }

    #[test]
    fn moderation_ballot_event_accepts_challenge_lifecycle_events() {
        let submitted_challenge = SoraFsModerationBallotGovernanceChallengeV1 {
            challenge_id: "challenge-1".to_owned(),
            case_id: "case-42".to_owned(),
            round_id: "round-1".to_owned(),
            challenger_id: "moderation-provider".to_owned(),
            kind: SoraFsModerationBallotGovernanceChallengeKindV1::EvidenceMismatch,
            target_juror_id: None,
            evidence_digest: [0x42; 32],
            reason: "payload-free-evidence-digest".to_owned(),
            raised_at_unix_ms: 1_800_000_011_000,
            decision: None,
            resolved_by: None,
            resolved_at_unix_ms: None,
            resolution_note: None,
        };
        let submitted = SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: 3,
            kind: SoraFsModerationBallotGovernanceEventKindV1::ChallengeSubmitted,
            generated_at_unix_ms: 1_800_000_011_000,
            case_id: "case-42".to_owned(),
            round_id: "round-1".to_owned(),
            juror_id: None,
            committed_count: 2,
            revealed_count: 0,
            challenge_count: 1,
            tally: None,
            challenge: Some(submitted_challenge.clone()),
        };
        submitted
            .validate()
            .expect("submitted challenge event validates");

        let resolved_challenge = SoraFsModerationBallotGovernanceChallengeV1 {
            decision: Some(SoraFsModerationBallotGovernanceChallengeDecisionV1::Rejected),
            resolved_by: Some("moderation-operator".to_owned()),
            resolved_at_unix_ms: Some(1_800_000_012_000),
            resolution_note: Some("packet matches ballot".to_owned()),
            ..submitted_challenge
        };
        let resolved = SoraFsModerationBallotGovernanceEventV1 {
            kind: SoraFsModerationBallotGovernanceEventKindV1::ChallengeResolved,
            generated_at_unix_ms: 1_800_000_012_000,
            challenge: Some(resolved_challenge),
            ..submitted
        };
        resolved
            .validate()
            .expect("resolved challenge event validates");
    }

    #[test]
    fn moderation_ballot_event_rejects_invalid_challenge_payloads() {
        let base_challenge = SoraFsModerationBallotGovernanceChallengeV1 {
            challenge_id: "challenge-1".to_owned(),
            case_id: "case-42".to_owned(),
            round_id: "round-1".to_owned(),
            challenger_id: "moderation-provider".to_owned(),
            kind: SoraFsModerationBallotGovernanceChallengeKindV1::DuplicateCommit,
            target_juror_id: None,
            evidence_digest: [0x42; 32],
            reason: "payload-free-evidence-digest".to_owned(),
            raised_at_unix_ms: 1_800_000_011_000,
            decision: None,
            resolved_by: None,
            resolved_at_unix_ms: None,
            resolution_note: None,
        };
        let event = SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: 3,
            kind: SoraFsModerationBallotGovernanceEventKindV1::ChallengeSubmitted,
            generated_at_unix_ms: 1_800_000_011_000,
            case_id: "case-42".to_owned(),
            round_id: "round-1".to_owned(),
            juror_id: None,
            committed_count: 2,
            revealed_count: 0,
            challenge_count: 1,
            tally: None,
            challenge: Some(base_challenge),
        };
        assert!(matches!(
            event.validate(),
            Err(SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeTarget)
        ));

        let mut resolved = event;
        let challenge = resolved.challenge.as_mut().expect("challenge");
        challenge.kind = SoraFsModerationBallotGovernanceChallengeKindV1::EvidenceMismatch;
        challenge.decision = Some(SoraFsModerationBallotGovernanceChallengeDecisionV1::Accepted);
        resolved.kind = SoraFsModerationBallotGovernanceEventKindV1::ChallengeResolved;
        assert!(matches!(
            resolved.validate(),
            Err(SoraFsModerationBallotGovernanceEventValidationError::MissingChallengeResolver)
        ));
    }
}
