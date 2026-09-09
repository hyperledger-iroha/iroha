//! Governance DAG node schemas used for audit publishing.

mod canonical_payload_codec;

use crate::{
    capacity::ReplicationOrderV1,
    deal::{DealSettlementV1, XorQuantity},
    orderbook::SettlementReceiptV1,
    pdp::{PdpGovernanceArchiveV1, PdpGovernanceArchiveValidationError},
    por::{
        AuditVerdictV1, PorChallengePublicationV1, PorProofV1, PorReportIsoWeek, PorWeeklyReportV1,
    },
    reconciliation::{SORAFS_RECONCILIATION_REPORT_VERSION_V1, SorafsReconciliationReportV1},
    repair::{
        GC_AUDIT_EVENT_VERSION_V1, GcAuditEventV1, REPAIR_AUDIT_EVENT_VERSION_V1,
        REPAIR_SLASH_PROPOSAL_VERSION_V1, RepairAuditEventV1, RepairSlashProposalV1,
    },
    reputation::signed::{
        MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES, SignedReputationSnapshotError,
        SignedReputationSnapshotV1,
    },
    transparency::{
        MODERATION_LEDGER_PUBLICATION_VERSION_V1, ModerationLedgerCyclePublicationV1,
        PROOF_TOKEN_ISSUANCE_VERSION_V1, ProofTokenIssuanceV1,
    },
};
use blake3::Hasher;
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use iroha_crypto::{Algorithm, PublicKey};
use norito::core::SerializePayload as _;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use soranet_pq::MlDsaSuite;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
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
/// Maximum number of scalar labels attached to one filesystem publication.
pub const GOVERNANCE_PUBLICATION_LABEL_MAX_ENTRIES_V1: usize = 64;
/// Maximum UTF-8 byte length of one filesystem publication label key.
pub const GOVERNANCE_PUBLICATION_LABEL_KEY_MAX_BYTES_V1: usize = 128;
/// Maximum UTF-8 byte length of one string-valued filesystem publication label.
pub const GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1: usize = 4 * 1024;
/// Maximum compact-JSON bytes occupied by one filesystem publication label map.
pub const GOVERNANCE_PUBLICATION_LABEL_TOTAL_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum byte length of one retained filesystem CAR-segment manifest.
pub const GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1: usize = 128 * 1024;
/// Exact byte length of the authenticated account digest committed by a
/// first-release Governance DAG node.
pub const GOVERNANCE_DAG_SUBMISSION_ACCOUNT_DIGEST_BYTES_V1: usize = blake3::OUT_LEN;
/// Authenticated ingress that admitted a caller-supplied Governance DAG payload.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceDagSubmissionOriginV1")]
#[derive(
    Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, PartialOrd, Ord,
)]
#[repr(u8)]
pub enum GovernanceDagSubmissionOriginV1 {
    /// Canonical Torii proof-token issuance ingress.
    TransparencyTokenIssuance = 0,
    /// Canonical Torii privacy-aggregate source-event ingress.
    PrivacyAggregateSourceEvent = 1,
    /// Canonical Torii due-cycle publication ingress.
    PrivacyAggregatePublishDue = 2,
    /// Canonical Torii appeal-finance report ingress.
    AppealFinanceReport = 3,
    /// Canonical Torii appeal-finance weekly-rollup ingress.
    AppealFinanceWeeklyRollup = 4,
}
impl GovernanceDagSubmissionOriginV1 {
    /// Return the stable public label for this authenticated ingress.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TransparencyTokenIssuance => "transparency_token_issuance",
            Self::PrivacyAggregateSourceEvent => "privacy_aggregate_source_event",
            Self::PrivacyAggregatePublishDue => "privacy_aggregate_publish_due",
            Self::AppealFinanceReport => "appeal_finance_report",
            Self::AppealFinanceWeeklyRollup => "appeal_finance_weekly_rollup",
        }
    }
}
const GOVERNANCE_DAG_SUBMISSION_ACCOUNT_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.governance_dag.submission_account.digest.v1";
const GOVERNANCE_PUBLICATION_SOURCE_PAIR_ID_DOMAIN_V1: &[u8] =
    b"sorafs.governance.publication_source_pair.id.v1";
/// Derive the authenticated-account commitment stored in signed DAG provenance.
///
/// `canonical_account_bytes` must be the canonical Norito encoding of the authenticated
/// `AccountId`. Keeping that dependency at the producer boundary lets this wire-format crate commit
/// the identity without embedding an unbounded or presentation-dependent account string.
#[must_use]
pub fn governance_dag_submission_account_digest_v1(
    canonical_account_bytes: &[u8],
) -> [u8; GOVERNANCE_DAG_SUBMISSION_ACCOUNT_DIGEST_BYTES_V1] {
    let mut hasher = Hasher::new();
    hasher.update(GOVERNANCE_DAG_SUBMISSION_ACCOUNT_DIGEST_DOMAIN_V1);
    hasher.update(
        &u64::try_from(canonical_account_bytes.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(canonical_account_bytes);
    *hasher.finalize().as_bytes()
}
/// Derive the content identity for one filesystem publication source pair.
///
/// The identity binds the internal payload kind plus the exact encoded and JSON byte lengths and
/// BLAKE3 digests. Filesystem publishers use it as the only variable path component, so
/// presentation labels and model identifiers never become path authority.
#[must_use]
pub fn governance_publication_source_pair_id_v1(
    payload_kind: &str,
    encoded_len: u64,
    encoded_blake3: [u8; blake3::OUT_LEN],
    json_len: u64,
    json_blake3: [u8; blake3::OUT_LEN],
) -> [u8; blake3::OUT_LEN] {
    let mut hasher = Hasher::new();
    hasher.update(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ID_DOMAIN_V1);
    hasher.update(
        &u64::try_from(payload_kind.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(payload_kind.as_bytes());
    hasher.update(&encoded_len.to_le_bytes());
    hasher.update(&encoded_blake3);
    hasher.update(&json_len.to_le_bytes());
    hasher.update(&json_blake3);
    *hasher.finalize().as_bytes()
}
/// Server-derived caller identity commitment in a signed Governance DAG node.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceDagSubmissionProvenanceV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceDagSubmissionProvenanceV1 {
    /// Domain-separated digest of the authenticated account's canonical Norito bytes.
    pub publisher_account_digest: [u8; GOVERNANCE_DAG_SUBMISSION_ACCOUNT_DIGEST_BYTES_V1],
    /// Exact authenticated ingress that admitted the payload.
    pub origin: GovernanceDagSubmissionOriginV1,
}
impl GovernanceDagSubmissionProvenanceV1 {
    fn validate(&self) -> Result<(), GovernanceLogValidationError> {
        Ok(())
    }
}
/// Maximum canonical bytes for one source payload admitted by the V1
/// filesystem Governance DAG producer.
///
/// This matches the largest independently bounded producer envelope, the
/// signed reputation snapshot. Payload variants without their own tighter
/// semantic ceiling are still subject to this producer hard cut.
pub const GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1: usize =
    MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES;
/// Maximum canonical bytes hashed or signed for one first-release Governance
/// DAG node, block, or head payload.
///
/// The largest independently bounded embedded envelope is a signed reputation snapshot. Reserving
/// an equal amount for the enclosing Governance enum, node/block fields, CIDs, peer identity, and
/// signature metadata keeps the wire globally bounded while admitting every canonical V1 snapshot.
/// Signing and CID helpers encode borrowed canonical views, so these bounds do not require cloning
/// the embedded payload or node.
pub const GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1: usize =
    GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1 * 2;
/// Fixed allowance for the canonical block signature and outer Norito envelope.
pub const GOVERNANCE_DAG_BLOCK_ENVELOPE_MAX_BYTES_V1: usize = 64 * 1024;
const fn governance_dag_block_max_canonical_bytes_v1() -> usize {
    match GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1
        .checked_add(GOVERNANCE_DAG_BLOCK_ENVELOPE_MAX_BYTES_V1)
    {
        Some(limit) => limit,
        None => panic!("Governance DAG block byte ceiling overflow"),
    }
}
/// Maximum canonical header-bearing Norito bytes for one V1 Governance DAG block.
///
/// The block signing payload is capped independently above. This ceiling adds one checked, fixed
/// allowance for the block signature and outer schema envelope so persistence and transport readers
/// do not reject a block whose canonical signing payload was admitted.
pub const GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1: usize =
    governance_dag_block_max_canonical_bytes_v1();
/// Number of newest blocks committed by a checkpointed first-release head.
pub const GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1: usize = 64;
/// Current moderation ballot governance event schema version.
pub const SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1: u16 = 1;
/// Maximum exact canonical size of one moderation ballot governance event.
pub const SORAFS_MODERATION_BALLOT_EVENT_MAX_CANONICAL_BYTES_V1: usize = 64 * 1024;
/// Maximum UTF-8 byte length of a moderation case, round, or challenge id.
pub const SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1: usize = 256;
/// Maximum UTF-8 byte length of a moderation account or service id.
pub const SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1: usize = 512;
/// Maximum UTF-8 byte length of a moderation reason or resolution note.
pub const SORAFS_MODERATION_PUBLIC_TEXT_MAX_BYTES_V1: usize = 4 * 1024;
/// Current SoraFS appeal finance report schema version.
pub const SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1: u16 = 1;
/// Maximum exact canonical size of one appeal finance report.
pub const SORAFS_APPEAL_FINANCE_REPORT_MAX_CANONICAL_BYTES_V1: usize = 4 * 1024 * 1024;
/// Maximum UTF-8 byte length of an appeal case or round identifier.
pub const SORAFS_APPEAL_FINANCE_IDENTIFIER_MAX_BYTES_V1: usize = 256;
/// Maximum UTF-8 byte length of an appeal finance config version.
pub const SORAFS_APPEAL_FINANCE_CONFIG_VERSION_MAX_BYTES_V1: usize = 128;
/// Maximum UTF-8 byte length of an appeal finance account or juror id.
pub const SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1: usize = 512;
/// Maximum combined paid/no-show juror rows in one appeal finance report.
pub const SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1: usize = 65_536;
/// Current SoraFS appeal finance weekly rollup schema version.
pub const SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1: u16 = 1;
/// Maximum exact canonical size of one appeal finance weekly rollup.
pub const SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_MAX_CANONICAL_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum distinct config versions represented in one weekly rollup.
pub const SORAFS_APPEAL_FINANCE_WEEKLY_CONFIG_VERSIONS_MAX_V1: usize = 64;
/// Maximum source report ids represented in one weekly rollup.
pub const SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1: usize = 65_536;
/// Maximum outcome rows; V1 has exactly seven possible outcomes.
pub const SORAFS_APPEAL_FINANCE_WEEKLY_OUTCOMES_MAX_V1: usize = 7;
/// Current SoraFS appeal finance settlement receipt schema version.
pub const SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1: u16 = 1;
/// Exact Ed25519 public-key length admitted on Governance DAG signatures.
pub const GOVERNANCE_ED25519_PUBLIC_KEY_BYTES_V1: usize = PUBLIC_KEY_LENGTH;
/// Exact Ed25519 signature length admitted on Governance DAG signatures.
pub const GOVERNANCE_ED25519_SIGNATURE_BYTES_V1: usize = SIGNATURE_LENGTH;
/// Exact ML-DSA-65 public-key length admitted on Governance DAG signatures.
pub const GOVERNANCE_ML_DSA_65_PUBLIC_KEY_BYTES_V1: usize = 1_952;
/// Exact ML-DSA-65 detached-signature length admitted on Governance DAG signatures.
pub const GOVERNANCE_ML_DSA_65_SIGNATURE_BYTES_V1: usize = 3_309;
const APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1: usize = 256;
const APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1: usize = 512;
const APPEAL_FINANCE_SETTLEMENT_RECEIPT_CONFIG_VERSION_MAX_BYTES_V1: usize = 128;
const APPEAL_FINANCE_SETTLEMENT_RECEIPT_SUBMITTED_STEPS_V1: [&str; 2] =
    ["drawdown_non_refund", "cancel_refund"];
const APPEAL_FINANCE_SETTLEMENT_RECEIPT_RECONCILIATION_STATUSES_V1: [&str; 2] =
    ["awaiting_refund_cancel", "settled"];
const APPEAL_FINANCE_SETTLEMENT_RECEIPT_LIFECYCLE_STATUSES_V1: [&str; 3] =
    ["locked", "drawn_down", "cancelled"];
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsModerationBallotGovernanceEventKindV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsModerationVoteChoiceV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsModerationVoteCountsV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsModerationBallotGovernanceTallyV1")]
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
        validate_moderation_text(
            &self.case_id,
            "tally.case_id",
            SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1,
        )?;
        validate_non_empty_governance_label(
            &self.round_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingRoundId,
        )?;
        validate_moderation_text(
            &self.round_id,
            "tally.round_id",
            SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1,
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
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::governance::SoraFsModerationBallotGovernanceChallengeKindV1"
)]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::governance::SoraFsModerationBallotGovernanceChallengeDecisionV1"
)]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsModerationBallotGovernanceChallengeV1")]
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
        validate_moderation_text(
            &self.challenge_id,
            "challenge.challenge_id",
            SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1,
        )?;
        validate_non_empty_governance_label(
            &self.case_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingCaseId,
        )?;
        validate_moderation_text(
            &self.case_id,
            "challenge.case_id",
            SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1,
        )?;
        validate_non_empty_governance_label(
            &self.round_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingRoundId,
        )?;
        validate_moderation_text(
            &self.round_id,
            "challenge.round_id",
            SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1,
        )?;
        validate_non_empty_governance_label(
            &self.challenger_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingChallengerId,
        )?;
        validate_moderation_text(
            &self.challenger_id,
            "challenge.challenger_id",
            SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1,
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
        validate_moderation_text(
            &self.reason,
            "challenge.reason",
            SORAFS_MODERATION_PUBLIC_TEXT_MAX_BYTES_V1,
        )?;
        if let Some(target) = self.target_juror_id.as_deref() {
            validate_non_empty_governance_label(
                target,
                SoraFsModerationBallotGovernanceEventValidationError::BlankChallengeTarget,
            )?;
            validate_moderation_text(
                target,
                "challenge.target_juror_id",
                SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1,
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
                validate_moderation_text(
                    resolved_by,
                    "challenge.resolved_by",
                    SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1,
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
                if let Some(note) = self.resolution_note.as_deref() {
                    validate_moderation_text(
                        note,
                        "challenge.resolution_note",
                        SORAFS_MODERATION_PUBLIC_TEXT_MAX_BYTES_V1,
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
/// Governance DAG payload for one local SoraFS moderation ballot lifecycle event.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsModerationBallotGovernanceEventV1")]
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
        preflight_moderation_event_len(
            self,
            SORAFS_MODERATION_BALLOT_EVENT_MAX_CANONICAL_BYTES_V1,
        )?;
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
        validate_moderation_text(
            &self.case_id,
            "case_id",
            SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1,
        )?;
        validate_non_empty_governance_label(
            &self.round_id,
            SoraFsModerationBallotGovernanceEventValidationError::MissingRoundId,
        )?;
        validate_moderation_text(
            &self.round_id,
            "round_id",
            SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1,
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
                validate_moderation_text(
                    juror_id,
                    "juror_id",
                    SORAFS_MODERATION_ACCOUNT_MAX_BYTES_V1,
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsAppealFinanceOutcomeV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsAppealFinanceAccountFlowV1")]
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
            role,
            SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1,
            SoraFsAppealFinanceReportValidationError::MissingAccountId { role },
        )?;
        Ok(())
    }
}
/// Per-juror appeal finance payout.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsAppealFinanceJurorPayoutV1")]
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
            "juror_payouts.juror_id",
            SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1,
            SoraFsAppealFinanceReportValidationError::MissingJurorId,
        )?;
        Ok(())
    }
}
/// Governance DAG appeal finance report for settlement/disbursement audits.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsAppealFinanceReportV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsAppealFinanceSettlementReceiptV1")]
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SoraFsAppealFinanceSettlementReceiptV1 {
    /// Schema version (`SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1`).
    pub version: u16,
    /// Stable receipt identifier derived from the submitted transaction context.
    pub receipt_id: [u8; 16],
    /// Bounded canonical ASCII moderation or appeal case identifier.
    pub case_id: String,
    /// Optional bounded canonical ASCII moderation ballot round identifier.
    #[norito(default)]
    pub round_id: Option<String>,
    /// UTC timestamp (milliseconds) when the settlement transaction was queued.
    pub generated_at_unix_ms: u64,
    /// Height of the finalized block whose committed state contains the settlement.
    pub finalized_block_height: u64,
    /// Exact non-zero hash of the finalized block at `finalized_block_height`.
    pub finalized_block_hash: [u8; 32],
    /// Lowercase kebab config name with a positive canonical `-vN` suffix.
    pub appeal_finance_config_version: String,
    /// Canonical digest of the governed appeal finance policy used to derive the plan.
    pub appeal_finance_policy_digest: [u8; 32],
    /// Final appeal outcome used by settlement calculation.
    pub outcome: SoraFsAppealFinanceOutcomeV1,
    /// Canonical escrow id as lowercase hexadecimal.
    pub escrow_id_hex: String,
    /// Bounded canonical ASCII account that funded the deposit.
    pub payer_account: String,
    /// Bounded canonical ASCII account holding the locked asset.
    pub destination_account: String,
    /// Optional bounded canonical ASCII authority allowed to draw down funds.
    #[norito(default)]
    pub release_authority_account: Option<String>,
    /// Finalized step: `drawdown_non_refund` or `cancel_refund`.
    pub submitted_step: String,
    /// Bounded canonical ASCII transaction authority for the submitted step.
    pub required_authority: String,
    /// Exact XOR amount affected by the submitted step.
    pub amount_xor: XorQuantity,
    /// Exact committed transaction hash as lowercase hexadecimal.
    pub tx_hash_hex: String,
    /// Digest of the reconciliation snapshot that justified submission.
    pub reconciliation_digest_hex: String,
    /// Applied reconciliation status: `awaiting_refund_cancel` or `settled`.
    pub reconciliation_status: String,
    /// Applied lock lifecycle status: `locked`, `drawn_down`, or `cancelled`.
    pub observed_lifecycle_status: String,
    /// Ledger remaining amount observed after the step was applied.
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
    /// Returns [`SoraFsAppealFinanceSettlementReceiptValidationError`] when required identifiers
    /// are missing or noncanonical, finalized-state labels are unsupported or inconsistent, digest
    /// fields are malformed, or required timestamps, finalized cursors, or counts are zero.
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
        validate_bounded_visible_settlement_receipt_label(
            &self.case_id,
            "case_id",
            APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingCaseId,
        )?;
        if let Some(round_id) = self.round_id.as_deref() {
            validate_bounded_visible_settlement_receipt_label(
                round_id,
                "round_id",
                APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingRoundId,
            )?;
        }
        if self.generated_at_unix_ms == 0 {
            return Err(SoraFsAppealFinanceSettlementReceiptValidationError::MissingGeneratedAt);
        }
        if self.finalized_block_height == 0 {
            return Err(
                SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinalizedBlockHeight,
            );
        }
        if self.finalized_block_hash == [0u8; 32] {
            return Err(
                SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinalizedBlockHash,
            );
        }
        validate_settlement_receipt_config_version(&self.appeal_finance_config_version)?;
        if self.appeal_finance_policy_digest == [0u8; 32] {
            return Err(
                SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinancePolicyDigest,
            );
        }
        validate_receipt_hex(
            &self.escrow_id_hex,
            "escrow_id_hex",
            32,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingEscrowId,
        )?;
        validate_bounded_visible_settlement_receipt_label(
            &self.payer_account,
            "payer_account",
            APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingPayerAccount,
        )?;
        validate_bounded_visible_settlement_receipt_label(
            &self.destination_account,
            "destination_account",
            APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingDestinationAccount,
        )?;
        if let Some(account) = self.release_authority_account.as_deref() {
            validate_bounded_visible_settlement_receipt_label(
                account,
                "release_authority_account",
                APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1,
                SoraFsAppealFinanceSettlementReceiptValidationError::MissingReleaseAuthorityAccount,
            )?;
        }
        validate_bounded_visible_settlement_receipt_label(
            &self.required_authority,
            "required_authority",
            APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1,
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingRequiredAuthority,
        )?;
        validate_settlement_receipt_submitted_step(&self.submitted_step)?;
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
        validate_settlement_receipt_reconciliation_status(&self.reconciliation_status)?;
        validate_settlement_receipt_lifecycle_status(&self.observed_lifecycle_status)?;
        validate_settlement_receipt_finalized_state(
            &self.submitted_step,
            &self.reconciliation_status,
            &self.observed_lifecycle_status,
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
        preflight_appeal_finance_report_len(
            self,
            SORAFS_APPEAL_FINANCE_REPORT_MAX_CANONICAL_BYTES_V1,
        )?;
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
            "case_id",
            SORAFS_APPEAL_FINANCE_IDENTIFIER_MAX_BYTES_V1,
            SoraFsAppealFinanceReportValidationError::MissingCaseId,
        )?;
        if let Some(round_id) = self.round_id.as_deref() {
            validate_non_empty_appeal_finance_label(
                round_id,
                "round_id",
                SORAFS_APPEAL_FINANCE_IDENTIFIER_MAX_BYTES_V1,
                SoraFsAppealFinanceReportValidationError::MissingRoundId,
            )?;
        }
        if self.generated_at_unix_ms == 0 {
            return Err(SoraFsAppealFinanceReportValidationError::MissingGeneratedAt);
        }
        validate_non_empty_appeal_finance_label(
            &self.appeal_finance_config_version,
            "appeal_finance_config_version",
            SORAFS_APPEAL_FINANCE_CONFIG_VERSION_MAX_BYTES_V1,
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
        let accounted = validate_appeal_finance_panel_bounds(
            self.panel_size,
            self.juror_payouts.len(),
            self.no_show_juror_ids.len(),
        )?;
        let mut payout_jurors = BTreeSet::new();
        for payout in &self.juror_payouts {
            payout.validate()?;
            if !payout_jurors.insert(payout.juror_id.as_str()) {
                return Err(SoraFsAppealFinanceReportValidationError::DuplicateJurorId {
                    juror_id: payout.juror_id.clone(),
                });
            }
        }
        let mut no_show_jurors = BTreeSet::new();
        for juror_id in &self.no_show_juror_ids {
            validate_non_empty_appeal_finance_label(
                juror_id,
                "no_show_juror_ids",
                SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1,
                SoraFsAppealFinanceReportValidationError::MissingNoShowJurorId,
            )?;
            if !no_show_jurors.insert(juror_id.as_str()) {
                return Err(
                    SoraFsAppealFinanceReportValidationError::DuplicateNoShowJurorId {
                        juror_id: juror_id.clone(),
                    },
                );
            }
            if payout_jurors.contains(juror_id.as_str()) {
                return Err(SoraFsAppealFinanceReportValidationError::NoShowJurorPaid {
                    juror_id: juror_id.clone(),
                });
            }
        }
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsAppealFinanceOutcomeRollupV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::SoraFsAppealFinanceWeeklyRollupV1")]
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
    /// Source report order does not affect the resulting rollup. Report ids and config versions are
    /// sorted, and outcome rows are emitted in stable enum order.
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
        if reports.len() > SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1 {
            return Err(SoraFsAppealFinanceWeeklyRollupBuildError::TooManyReports {
                found: reports.len(),
                maximum: SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1,
            });
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
        preflight_appeal_finance_weekly_rollup_len(
            self,
            SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_MAX_CANONICAL_BYTES_V1,
        )?;
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
        if self.report_count
            > u64::try_from(SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1)
                .expect("weekly source-report ceiling fits u64")
        {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::TooManySourceReports {
                    found: self.report_count,
                    maximum: SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1,
                },
            );
        }
        if self.case_count == 0 || self.case_count > self.report_count {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::InvalidCaseCount {
                    case_count: self.case_count,
                    report_count: self.report_count,
                },
            );
        }
        if self.appeal_finance_config_versions.len()
            > SORAFS_APPEAL_FINANCE_WEEKLY_CONFIG_VERSIONS_MAX_V1
        {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::TooManyConfigVersions {
                    found: self.appeal_finance_config_versions.len(),
                    maximum: SORAFS_APPEAL_FINANCE_WEEKLY_CONFIG_VERSIONS_MAX_V1,
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
        if self.outcomes.len() > SORAFS_APPEAL_FINANCE_WEEKLY_OUTCOMES_MAX_V1 {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::TooManyOutcomes {
                    found: self.outcomes.len(),
                    maximum: SORAFS_APPEAL_FINANCE_WEEKLY_OUTCOMES_MAX_V1,
                },
            );
        }
        if self.source_report_ids.len() > SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1 {
            return Err(
                SoraFsAppealFinanceWeeklyRollupValidationError::TooManySourceReportIds {
                    found: self.source_report_ids.len(),
                    maximum: SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1,
                },
            );
        }
        let mut source_report_ids = BTreeSet::new();
        let mut previous_report_id: Option<[u8; 16]> = None;
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
            if previous_report_id.is_some_and(|previous| previous > *report_id) {
                return Err(
                    SoraFsAppealFinanceWeeklyRollupValidationError::UnsortedSourceReportIds,
                );
            }
            previous_report_id = Some(*report_id);
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
        let mut previous_outcome: Option<SoraFsAppealFinanceOutcomeV1> = None;
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
            if previous_outcome.is_some_and(|previous| previous > row.outcome) {
                return Err(SoraFsAppealFinanceWeeklyRollupValidationError::UnsortedOutcomes);
            }
            reconciled.add_outcome(row)?;
            previous_outcome = Some(row.outcome);
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
        if label.trim().is_empty()
            || label.trim() != label
            || label.len() > SORAFS_APPEAL_FINANCE_CONFIG_VERSION_MAX_BYTES_V1
            || label.chars().any(char::is_control)
        {
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
fn preflight_appeal_finance_weekly_rollup_len(
    rollup: &SoraFsAppealFinanceWeeklyRollupV1,
    maximum: usize,
) -> Result<usize, SoraFsAppealFinanceWeeklyRollupValidationError> {
    let found = rollup
        .encoded_len_exact()
        .ok_or(SoraFsAppealFinanceWeeklyRollupValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(
            SoraFsAppealFinanceWeeklyRollupValidationError::PayloadTooLarge { found, maximum },
        );
    }
    Ok(found)
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceExternalPayloadMetadataV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceExternalPayloadV1")]
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
    let decoded = norito::decode_canonical_with_limits::<T>(bytes, limits).map_err(|err| {
        if matches!(err, norito::Error::NonCanonicalEncoding) {
            GovernanceExternalPayloadValidationError::NonCanonicalEncodedPayload {
                payload_kind: payload_kind.to_owned(),
            }
        } else {
            GovernanceExternalPayloadValidationError::TypedPayloadDecode {
                payload_kind: payload_kind.to_owned(),
                reason: err.to_string(),
            }
        }
    })?;
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceLogPayloadV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub enum GovernanceLogPayloadV1 {
    /// Provider advertisement snapshot.
    ProviderAdvert(crate::provider_advert::ProviderAdvertV1),
    /// Replication order snapshot.
    ReplicationOrder(ReplicationOrderV1),
    /// Canonical PoR challenge publication with duplicate-sample metadata.
    PorChallengePublication(PorChallengePublicationV1),
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
    /// Validated PoR weekly health report.
    PorWeeklyReport(PorWeeklyReportV1),
}
impl GovernanceLogPayloadV1 {
    fn required_submission_origin(&self) -> Option<GovernanceDagSubmissionOriginV1> {
        match self {
            Self::AppealFinanceReport(_) => {
                Some(GovernanceDagSubmissionOriginV1::AppealFinanceReport)
            }
            Self::AppealFinanceWeeklyRollup(_) => {
                Some(GovernanceDagSubmissionOriginV1::AppealFinanceWeeklyRollup)
            }
            _ => None,
        }
    }
    fn optional_submission_origin(&self) -> Option<GovernanceDagSubmissionOriginV1> {
        match self {
            Self::ExternalPayload(payload)
                if payload.payload_kind == GOVERNANCE_EXTERNAL_KIND_PROOF_TOKEN_ISSUANCE_V1 =>
            {
                Some(GovernanceDagSubmissionOriginV1::TransparencyTokenIssuance)
            }
            Self::ExternalPayload(payload)
                if payload.payload_kind
                    == GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1 =>
            {
                Some(GovernanceDagSubmissionOriginV1::PrivacyAggregatePublishDue)
            }
            _ => None,
        }
    }
    /// Validate the server-derived provenance required by this payload kind.
    ///
    /// Caller-supplied finance payloads must retain their authenticated ingress identity.
    /// Proof-token and transparency-ledger payloads may be produced by a trusted in-process
    /// producer; when they instead enter through an authenticated route, the signed node must
    /// retain the matching identity. All other internally produced payloads reject provenance so a
    /// node cannot misleadingly label them as caller submissions.
    pub fn validate_submission_provenance(
        &self,
        provenance: Option<&GovernanceDagSubmissionProvenanceV1>,
    ) -> Result<(), GovernanceLogValidationError> {
        if let Some(expected) = self.required_submission_origin() {
            let provenance = provenance
                .ok_or(GovernanceLogValidationError::MissingSubmissionProvenance { expected })?;
            if provenance.origin != expected {
                return Err(GovernanceLogValidationError::SubmissionOriginMismatch {
                    expected,
                    found: provenance.origin,
                });
            }
            return provenance.validate();
        }
        let Some(provenance) = provenance else {
            return Ok(());
        };
        let Some(expected) = self.optional_submission_origin() else {
            return Err(
                GovernanceLogValidationError::UnexpectedSubmissionProvenance {
                    found: provenance.origin,
                },
            );
        };
        if provenance.origin != expected {
            return Err(GovernanceLogValidationError::SubmissionOriginMismatch {
                expected,
                found: provenance.origin,
            });
        }
        provenance.validate()
    }
    fn validate(&self, timestamp: u64) -> Result<(), GovernanceLogValidationError> {
        preflight_governance_source_payload_len(
            self,
            GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1,
        )?;
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
            GovernanceLogPayloadV1::PorChallengePublication(publication) => publication
                .validate()
                .map_err(GovernanceLogValidationError::PorChallengePublication),
            GovernanceLogPayloadV1::PorWeeklyReport(report) => report
                .validate()
                .map_err(GovernanceLogValidationError::PorWeeklyReport),
        }
    }
}
fn preflight_governance_source_payload_len(
    payload: &GovernanceLogPayloadV1,
    maximum: usize,
) -> Result<usize, GovernanceLogValidationError> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let found = payload
        .encoded_len_exact()
        .ok_or(GovernanceLogValidationError::CanonicalPayloadLengthUnavailable)?;
    if found > maximum {
        return Err(GovernanceLogValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
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
fn validate_moderation_text(
    value: &str,
    field: &'static str,
    maximum: usize,
) -> Result<(), SoraFsModerationBallotGovernanceEventValidationError> {
    if value.len() > maximum || value.trim() != value || value.chars().any(char::is_control) {
        return Err(
            SoraFsModerationBallotGovernanceEventValidationError::InvalidBoundedText {
                field,
                found: value.len(),
                maximum,
            },
        );
    }
    Ok(())
}
fn preflight_moderation_event_len(
    event: &SoraFsModerationBallotGovernanceEventV1,
    maximum: usize,
) -> Result<usize, SoraFsModerationBallotGovernanceEventValidationError> {
    let found = event
        .encoded_len_exact()
        .ok_or(SoraFsModerationBallotGovernanceEventValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(
            SoraFsModerationBallotGovernanceEventValidationError::PayloadTooLarge {
                found,
                maximum,
            },
        );
    }
    Ok(found)
}
fn validate_non_empty_appeal_finance_label(
    value: &str,
    field: &'static str,
    maximum: usize,
    error: SoraFsAppealFinanceReportValidationError,
) -> Result<(), SoraFsAppealFinanceReportValidationError> {
    if value.trim().is_empty() {
        return Err(error);
    }
    if value.len() > maximum || value.trim() != value || value.chars().any(char::is_control) {
        return Err(
            SoraFsAppealFinanceReportValidationError::InvalidBoundedText {
                field,
                found: value.len(),
                maximum,
            },
        );
    }
    Ok(())
}
fn preflight_appeal_finance_report_len(
    report: &SoraFsAppealFinanceReportV1,
    maximum: usize,
) -> Result<usize, SoraFsAppealFinanceReportValidationError> {
    let found = report
        .encoded_len_exact()
        .ok_or(SoraFsAppealFinanceReportValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(SoraFsAppealFinanceReportValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
}
fn validate_appeal_finance_panel_bounds(
    panel_size: u32,
    payout_rows: usize,
    no_show_rows: usize,
) -> Result<usize, SoraFsAppealFinanceReportValidationError> {
    let accounted = payout_rows
        .checked_add(no_show_rows)
        .ok_or(SoraFsAppealFinanceReportValidationError::PanelRowCountOverflow)?;
    if accounted > SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1 {
        return Err(SoraFsAppealFinanceReportValidationError::TooManyPanelRows {
            found: accounted,
            maximum: SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1,
        });
    }
    if usize::try_from(panel_size).unwrap_or(usize::MAX) > SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1 {
        return Err(
            SoraFsAppealFinanceReportValidationError::PanelSizeExceedsMaximum {
                panel_size,
                maximum: SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1,
            },
        );
    }
    Ok(accounted)
}
fn validate_bounded_visible_settlement_receipt_label(
    value: &str,
    field: &'static str,
    max_bytes: usize,
    error: SoraFsAppealFinanceSettlementReceiptValidationError,
) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
    if value.trim().is_empty() {
        return Err(error);
    }
    if value.len() > max_bytes
        || !value.is_ascii()
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/' | b'@')
        })
    {
        return Err(
            SoraFsAppealFinanceSettlementReceiptValidationError::InvalidLabel { field, max_bytes },
        );
    }
    Ok(())
}
fn validate_settlement_receipt_config_version(
    value: &str,
) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
    if value.trim().is_empty() {
        return Err(
            SoraFsAppealFinanceSettlementReceiptValidationError::MissingFinanceConfigVersion,
        );
    }
    let valid = value.len() <= APPEAL_FINANCE_SETTLEMENT_RECEIPT_CONFIG_VERSION_MAX_BYTES_V1
        && value.rsplit_once("-v").is_some_and(|(name, version)| {
            !name.is_empty()
                && name.split('-').all(|segment| {
                    !segment.is_empty()
                        && segment
                            .bytes()
                            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                })
                && matches!(version.as_bytes(), [b'1'..=b'9', rest @ ..]
                    if rest.iter().all(|byte| byte.is_ascii_digit()))
        });
    if !valid {
        return Err(
            SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinanceConfigVersion {
                max_bytes: APPEAL_FINANCE_SETTLEMENT_RECEIPT_CONFIG_VERSION_MAX_BYTES_V1,
            },
        );
    }
    Ok(())
}
fn validate_settlement_receipt_submitted_step(
    value: &str,
) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
    validate_bounded_visible_settlement_receipt_label(
        value,
        "submitted_step",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1,
        SoraFsAppealFinanceSettlementReceiptValidationError::MissingSubmittedStep,
    )?;
    if !APPEAL_FINANCE_SETTLEMENT_RECEIPT_SUBMITTED_STEPS_V1.contains(&value) {
        return Err(SoraFsAppealFinanceSettlementReceiptValidationError::UnsupportedSubmittedStep);
    }
    Ok(())
}
fn validate_settlement_receipt_reconciliation_status(
    value: &str,
) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
    validate_bounded_visible_settlement_receipt_label(
        value,
        "reconciliation_status",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1,
        SoraFsAppealFinanceSettlementReceiptValidationError::MissingReconciliationStatus,
    )?;
    if !APPEAL_FINANCE_SETTLEMENT_RECEIPT_RECONCILIATION_STATUSES_V1.contains(&value) {
        return Err(
            SoraFsAppealFinanceSettlementReceiptValidationError::UnsupportedReconciliationStatus,
        );
    }
    Ok(())
}
fn validate_settlement_receipt_lifecycle_status(
    value: &str,
) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
    validate_bounded_visible_settlement_receipt_label(
        value,
        "observed_lifecycle_status",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1,
        SoraFsAppealFinanceSettlementReceiptValidationError::MissingObservedLifecycleStatus,
    )?;
    if !APPEAL_FINANCE_SETTLEMENT_RECEIPT_LIFECYCLE_STATUSES_V1.contains(&value) {
        return Err(
            SoraFsAppealFinanceSettlementReceiptValidationError::UnsupportedLifecycleStatus,
        );
    }
    Ok(())
}
fn validate_settlement_receipt_finalized_state(
    submitted_step: &str,
    reconciliation_status: &str,
    lifecycle_status: &str,
) -> Result<(), SoraFsAppealFinanceSettlementReceiptValidationError> {
    if !matches!(
        (submitted_step, reconciliation_status, lifecycle_status),
        ("drawdown_non_refund", "awaiting_refund_cancel", "locked")
            | ("drawdown_non_refund", "settled", "drawn_down")
            | ("cancel_refund", "settled", "cancelled")
    ) {
        return Err(
            SoraFsAppealFinanceSettlementReceiptValidationError::InconsistentFinalizedState,
        );
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
#[path = "governance/borrowed_norito.rs"]
mod borrowed_norito;
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceLogNodeCidPayloadV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceLogNodeCidPayloadV1 {
    version: u8,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    submission_provenance: Option<GovernanceDagSubmissionProvenanceV1>,
    payload: GovernanceLogPayloadV1,
}
#[derive(norito::derive::SerializePayload)]
struct GovernanceLogNodeCidPayloadViewWireV1<'a> {
    version: u8,
    prev_cid: borrowed_norito::Option<'a>,
    timestamp: u64,
    publisher_peer_id: borrowed_norito::Vec<'a>,
    // Retain Option's self-delimiting packed-field layout without copying the value.
    submission_provenance: Option<borrowed_norito::Value<'a, GovernanceDagSubmissionProvenanceV1>>,
    payload: borrowed_norito::Value<'a, GovernanceLogPayloadV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::governance::GovernanceLogNodeCidPayloadViewV1",
    frame = "sorafs_manifest::governance::GovernanceLogNodeCidPayloadV1"
)]
struct GovernanceLogNodeCidPayloadViewV1<'a>(GovernanceLogNodeCidPayloadViewWireV1<'a>);
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceDagBlockCidPayloadV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceDagBlockCidPayloadV1 {
    version: u8,
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    node: GovernanceLogNodeV1,
}
#[derive(norito::derive::SerializePayload)]
struct GovernanceDagBlockCidPayloadViewWireV1<'a> {
    version: u8,
    prev_block_cid: borrowed_norito::Option<'a>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: borrowed_norito::Vec<'a>,
    node: borrowed_norito::Value<'a, GovernanceLogNodeV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::governance::GovernanceDagBlockCidPayloadViewV1",
    frame = "sorafs_manifest::governance::GovernanceDagBlockCidPayloadV1"
)]
struct GovernanceDagBlockCidPayloadViewV1<'a>(GovernanceDagBlockCidPayloadViewWireV1<'a>);
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceDagBlockSignaturePayloadV1")]
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
#[derive(norito::derive::SerializePayload)]
struct GovernanceDagBlockSignaturePayloadViewWireV1<'a> {
    version: u8,
    block_cid: borrowed_norito::Vec<'a>,
    prev_block_cid: borrowed_norito::Option<'a>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: borrowed_norito::Vec<'a>,
    node: borrowed_norito::Value<'a, GovernanceLogNodeV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::governance::GovernanceDagBlockSignaturePayloadViewV1",
    frame = "sorafs_manifest::governance::GovernanceDagBlockSignaturePayloadV1"
)]
struct GovernanceDagBlockSignaturePayloadViewV1<'a>(
    GovernanceDagBlockSignaturePayloadViewWireV1<'a>,
);
fn validate_governance_dag_signing_payload_len(length: usize) -> Result<(), norito::core::Error> {
    if length > GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1 {
        return Err(norito::core::Error::Message(format!(
            "Governance DAG canonical signing payload has {length} bytes, exceeding V1 limit {GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1}"
        )));
    }
    Ok(())
}
fn encode_governance_dag_signing_payload<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<Vec<u8>, norito::core::Error> {
    // Size checks and signatures/CIDs use one V1 layout even inside a foreign decode guard.
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let length = value.encoded_len_exact().ok_or_else(|| {
        norito::core::Error::Message(
            "Governance DAG canonical signing payload has no allocation-free exact size".to_owned(),
        )
    })?;
    validate_governance_dag_signing_payload_len(length)?;
    let bytes = encode_governance_dag_frame(value, length)?;
    validate_governance_dag_signing_payload_len(bytes.len())?;
    Ok(bytes)
}
fn encode_governance_dag_frame<T: norito::NoritoSerialize>(
    value: &T,
    payload_length: usize,
) -> Result<Vec<u8>, norito::core::Error> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let capacity = payload_length
        .checked_add(norito::core::Header::SIZE)
        .and_then(|length| length.checked_add(64))
        .ok_or(norito::core::Error::LengthMismatch)?;
    let mut writer = std::io::Cursor::new(Vec::with_capacity(capacity));
    norito::core::to_writer_seek(&mut writer, value)?;
    Ok(writer.into_inner())
}
fn validate_governance_dag_block_canonical_len(length: usize) -> Result<(), norito::core::Error> {
    if length > GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 {
        return Err(norito::core::Error::Message(format!(
            "Governance DAG canonical block has {length} bytes, exceeding V1 limit {GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1}"
        )));
    }
    Ok(())
}
/// Public Governance DAG block wrapping one validated governance log node.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceDagBlockV1")]
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
    /// Returns the bounded canonical header-bearing Norito block bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        let _canonical_flags =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let length =
            <Self as norito::SerializePayload>::encoded_len_exact(self).ok_or_else(|| {
                norito::core::Error::Message(
                    "Governance DAG canonical block has no allocation-free exact size".to_owned(),
                )
            })?;
        validate_governance_dag_block_canonical_len(length)?;
        let bytes = encode_governance_dag_frame(self, length)?;
        validate_governance_dag_block_canonical_len(bytes.len())?;
        Ok(bytes)
    }
    /// Returns canonical Norito bytes signed by the block publisher.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        encode_governance_dag_signing_payload(&GovernanceDagBlockSignaturePayloadViewV1::from(self))
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
        preflight_governance_dag_block_len(self, GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1)?;
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
fn preflight_governance_dag_block_len(
    block: &GovernanceDagBlockV1,
    maximum: usize,
) -> Result<usize, GovernanceDagBlockValidationError> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let found = block
        .encoded_len_exact()
        .ok_or(GovernanceDagBlockValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(GovernanceDagBlockValidationError::BlockTooLarge { found, maximum });
    }
    Ok(found)
}
/// Derives deterministic Governance DAG block CID bytes.
pub fn governance_dag_block_cid_v1(
    prev_block_cid: Option<&[u8]>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: &[u8],
    node: &GovernanceLogNodeV1,
) -> Result<Vec<u8>, norito::core::Error> {
    let payload = GovernanceDagBlockCidPayloadViewV1(GovernanceDagBlockCidPayloadViewWireV1 {
        version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
        prev_block_cid: borrowed_norito::Option(prev_block_cid),
        sequence,
        timestamp,
        publisher_peer_id: borrowed_norito::Vec(publisher_peer_id),
        node: borrowed_norito::Value(node),
    });
    let payload_bytes = encode_governance_dag_signing_payload(&payload)?;
    let mut hasher = Hasher::new();
    hasher.update(GOVERNANCE_DAG_BLOCK_CID_DOMAIN_V1);
    hasher.update(&payload_bytes);
    Ok(hasher.finalize().as_bytes().to_vec())
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceDagHeadSignaturePayloadV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceDagHeadSignaturePayloadV1 {
    version: u8,
    head_block_cid: Vec<u8>,
    block_count: u64,
    generated_at: u64,
    publisher_peer_id: Vec<u8>,
    checkpoint_cid: Option<Vec<u8>>,
}
#[derive(norito::derive::SerializePayload)]
struct GovernanceDagHeadSignaturePayloadViewWireV1<'a> {
    version: u8,
    head_block_cid: borrowed_norito::Vec<'a>,
    block_count: u64,
    generated_at: u64,
    publisher_peer_id: borrowed_norito::Vec<'a>,
    checkpoint_cid: borrowed_norito::Option<'a>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::governance::GovernanceDagHeadSignaturePayloadViewV1",
    frame = "sorafs_manifest::governance::GovernanceDagHeadSignaturePayloadV1"
)]
struct GovernanceDagHeadSignaturePayloadViewV1<'a>(GovernanceDagHeadSignaturePayloadViewWireV1<'a>);
/// Signed public Governance DAG head manifest.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceDagHeadV1")]
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
        encode_governance_dag_signing_payload(&GovernanceDagHeadSignaturePayloadViewV1::from(self))
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceLogSignatureV1")]
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
        let (expected_public_key, expected_signature) = match self.algorithm {
            GovernanceSignatureAlgorithm::Ed25519 => (
                GOVERNANCE_ED25519_PUBLIC_KEY_BYTES_V1,
                GOVERNANCE_ED25519_SIGNATURE_BYTES_V1,
            ),
            GovernanceSignatureAlgorithm::Dilithium3 => (
                GOVERNANCE_ML_DSA_65_PUBLIC_KEY_BYTES_V1,
                GOVERNANCE_ML_DSA_65_SIGNATURE_BYTES_V1,
            ),
        };
        if self.public_key.len() != expected_public_key {
            return Err(
                GovernanceLogValidationError::InvalidSignaturePublicKeyLength {
                    algorithm: self.algorithm,
                    found: self.public_key.len(),
                    expected: expected_public_key,
                },
            );
        }
        if self.signature.len() != expected_signature {
            return Err(GovernanceLogValidationError::InvalidSignatureLength {
                algorithm: self.algorithm,
                found: self.signature.len(),
                expected: expected_signature,
            });
        }
        if crate::inert_bytes(&self.public_key) || crate::inert_bytes(&self.signature) {
            return Err(GovernanceLogValidationError::InvalidSignature);
        }
        Ok(())
    }
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceLogSignaturePayloadV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceLogSignaturePayloadV1 {
    version: u8,
    node_cid: Vec<u8>,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    submission_provenance: Option<GovernanceDagSubmissionProvenanceV1>,
    payload: GovernanceLogPayloadV1,
}
#[derive(norito::derive::SerializePayload)]
struct GovernanceLogSignaturePayloadViewWireV1<'a> {
    version: u8,
    node_cid: borrowed_norito::Vec<'a>,
    prev_cid: borrowed_norito::Option<'a>,
    timestamp: u64,
    publisher_peer_id: borrowed_norito::Vec<'a>,
    submission_provenance: Option<borrowed_norito::Value<'a, GovernanceDagSubmissionProvenanceV1>>,
    payload: borrowed_norito::Value<'a, GovernanceLogPayloadV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::governance::GovernanceLogSignaturePayloadViewV1",
    frame = "sorafs_manifest::governance::GovernanceLogSignaturePayloadV1"
)]
struct GovernanceLogSignaturePayloadViewV1<'a>(GovernanceLogSignaturePayloadViewWireV1<'a>);
/// Algorithms supported for governance signatures.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceSignatureAlgorithm")]
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum GovernanceSignatureAlgorithm {
    /// Ed25519 signature.
    Ed25519 = 1,
    /// Dilithium3 (post-quantum) signature.
    Dilithium3 = 2,
}
/// Governance log node entry appended to the DAG.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::governance::GovernanceLogNodeV1")]
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
    /// Authenticated caller identity for payloads admitted through Torii.
    pub submission_provenance: Option<GovernanceDagSubmissionProvenanceV1>,
    /// Payload carried by this node.
    pub payload: GovernanceLogPayloadV1,
    /// Publisher signature covering the canonical node signing payload.
    pub publisher_signature: GovernanceLogSignatureV1,
}
impl GovernanceLogNodeV1 {
    /// Validates the log node payload.
    pub fn validate(&self) -> Result<(), GovernanceLogValidationError> {
        preflight_governance_log_node_len(self, GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1)?;
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
        self.payload
            .validate_submission_provenance(self.submission_provenance.as_ref())?;
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
        encode_governance_dag_signing_payload(&GovernanceLogSignaturePayloadViewV1::from(self))
    }
    /// Recomputes this node's deterministic CID bytes.
    pub fn recompute_node_cid(&self) -> Result<Vec<u8>, norito::core::Error> {
        governance_log_node_cid_v1(
            self.prev_cid.as_deref(),
            self.timestamp,
            &self.publisher_peer_id,
            self.submission_provenance.as_ref(),
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
fn preflight_governance_log_node_len(
    node: &GovernanceLogNodeV1,
    maximum: usize,
) -> Result<usize, GovernanceLogValidationError> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let found = node
        .encoded_len_exact()
        .ok_or(GovernanceLogValidationError::CanonicalNodeLengthUnavailable)?;
    if found > maximum {
        return Err(GovernanceLogValidationError::NodeTooLarge { found, maximum });
    }
    Ok(found)
}
/// Derives deterministic Governance log node CID bytes.
pub fn governance_log_node_cid_v1(
    prev_cid: Option<&[u8]>,
    timestamp: u64,
    publisher_peer_id: &[u8],
    submission_provenance: Option<&GovernanceDagSubmissionProvenanceV1>,
    payload: &GovernanceLogPayloadV1,
) -> Result<Vec<u8>, norito::core::Error> {
    let payload = GovernanceLogNodeCidPayloadViewV1(GovernanceLogNodeCidPayloadViewWireV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        prev_cid: borrowed_norito::Option(prev_cid),
        timestamp,
        publisher_peer_id: borrowed_norito::Vec(publisher_peer_id),
        submission_provenance: submission_provenance.map(borrowed_norito::Value),
        payload: borrowed_norito::Value(payload),
    });
    let payload_bytes = encode_governance_dag_signing_payload(&payload)?;
    let mut hasher = Hasher::new();
    hasher.update(GOVERNANCE_LOG_NODE_CID_DOMAIN_V1);
    hasher.update(&payload_bytes);
    Ok(hasher.finalize().as_bytes().to_vec())
}
include!("governance_signing_payload_validation.rs");
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
    #[error("governance log payload has no exact canonical encoded length")]
    CanonicalPayloadLengthUnavailable,
    #[error("governance log payload has {found} canonical bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    #[error("governance log node has no exact canonical encoded length")]
    CanonicalNodeLengthUnavailable,
    #[error("governance log node has {found} canonical bytes; maximum is {maximum}")]
    NodeTooLarge { found: usize, maximum: usize },
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
    #[error("governance payload requires authenticated submission provenance from {expected:?}")]
    MissingSubmissionProvenance {
        expected: GovernanceDagSubmissionOriginV1,
    },
    #[error(
        "internally produced governance payload must not carry {found:?} submission provenance"
    )]
    UnexpectedSubmissionProvenance {
        found: GovernanceDagSubmissionOriginV1,
    },
    #[error(
        "governance submission provenance origin mismatch: expected {expected:?}, found {found:?}"
    )]
    SubmissionOriginMismatch {
        expected: GovernanceDagSubmissionOriginV1,
        found: GovernanceDagSubmissionOriginV1,
    },
    #[error("publisher signature missing key or signature bytes")]
    InvalidSignature,
    #[error("governance {algorithm:?} public key has {found} bytes; expected {expected}")]
    InvalidSignaturePublicKeyLength {
        algorithm: GovernanceSignatureAlgorithm,
        found: usize,
        expected: usize,
    },
    #[error("governance {algorithm:?} signature has {found} bytes; expected {expected}")]
    InvalidSignatureLength {
        algorithm: GovernanceSignatureAlgorithm,
        found: usize,
        expected: usize,
    },
    #[error("failed to encode canonical governance log node CID payload: {reason}")]
    CidEncoding { reason: String },
    #[error("governance log node CID does not match the canonical node payload")]
    InvalidNodeCid,
    #[error("advert validation failed: {0}")]
    Advert(crate::provider_advert::AdvertValidationError),
    #[error("replication order validation failed: {0}")]
    ReplicationOrder(crate::capacity::ReplicationOrderValidationError),
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
    #[error("PoR challenge publication validation failed: {0}")]
    PorChallengePublication(crate::por::PorChallengePublicationValidationError),
    #[error("PoR weekly report validation failed: {0}")]
    PorWeeklyReport(crate::por::PorWeeklyReportValidationError),
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
    /// Canonical encoder cannot provide an allocation-free exact length.
    #[error("SoraFS moderation ballot event has no exact canonical encoded length")]
    CanonicalLengthUnavailable,
    /// Canonical event bytes exceed the first-release ceiling.
    #[error("SoraFS moderation ballot event has {found} bytes; maximum is {maximum}")]
    PayloadTooLarge {
        /// Exact canonical event length.
        found: usize,
        /// Maximum accepted canonical length.
        maximum: usize,
    },
    /// Public event text is padded, contains controls, or exceeds its field ceiling.
    #[error(
        "SoraFS moderation ballot field `{field}` has {found} bytes or noncanonical text; maximum is {maximum}"
    )]
    InvalidBoundedText {
        /// Invalid field name.
        field: &'static str,
        /// Observed UTF-8 byte length.
        found: usize,
        /// Maximum field byte length.
        maximum: usize,
    },
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
    /// Canonical encoder cannot provide an allocation-free exact length.
    #[error("SoraFS appeal finance report has no exact canonical encoded length")]
    CanonicalLengthUnavailable,
    /// Canonical report bytes exceed the first-release ceiling.
    #[error("SoraFS appeal finance report has {found} bytes; maximum is {maximum}")]
    PayloadTooLarge {
        /// Exact canonical report bytes.
        found: usize,
        /// Maximum accepted canonical bytes.
        maximum: usize,
    },
    /// Public report text is padded, contains controls, or exceeds its field ceiling.
    #[error(
        "SoraFS appeal finance field `{field}` has {found} bytes or noncanonical text; maximum is {maximum}"
    )]
    InvalidBoundedText {
        /// Invalid field name.
        field: &'static str,
        /// Observed UTF-8 byte length.
        found: usize,
        /// Maximum field byte length.
        maximum: usize,
    },
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
    /// Paid/no-show row count arithmetic overflowed.
    #[error("SoraFS appeal finance panel row count overflow")]
    PanelRowCountOverflow,
    /// Paid/no-show rows exceed the deterministic first-release ceiling.
    #[error("SoraFS appeal finance report has {found} panel rows; maximum is {maximum}")]
    TooManyPanelRows {
        /// Combined payout/no-show rows.
        found: usize,
        /// Maximum accepted rows.
        maximum: usize,
    },
    /// Declared panel size exceeds the deterministic first-release ceiling.
    #[error("SoraFS appeal finance panel size `{panel_size}` exceeds maximum {maximum}")]
    PanelSizeExceedsMaximum {
        /// Declared panel size.
        panel_size: u32,
        /// Maximum accepted panel size.
        maximum: usize,
    },
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
    /// Finalized block height is zero.
    #[error("SoraFS appeal finance settlement receipt finalized block height must be non-zero")]
    InvalidFinalizedBlockHeight,
    /// Finalized block hash is all zero.
    #[error("SoraFS appeal finance settlement receipt finalized block hash must be non-zero")]
    InvalidFinalizedBlockHash,
    /// Missing finance config version.
    #[error("SoraFS appeal finance settlement receipt config version is required")]
    MissingFinanceConfigVersion,
    /// Finance config version was not bounded canonical lowercase kebab syntax.
    #[error(
        "SoraFS appeal finance settlement receipt config version must be at most {max_bytes} bytes and use lowercase kebab syntax ending in a positive canonical `-vN`"
    )]
    InvalidFinanceConfigVersion {
        /// Maximum permitted UTF-8 byte length.
        max_bytes: usize,
    },
    /// Governed appeal finance policy digest is all zero.
    #[error("SoraFS appeal finance settlement receipt policy digest must not be zero")]
    InvalidFinancePolicyDigest,
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
    /// Submitted step is outside the finalized settlement-step inventory.
    #[error("SoraFS appeal finance settlement receipt submitted step is unsupported")]
    UnsupportedSubmittedStep,
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
    /// Reconciliation status is not valid for a finalized applied receipt.
    #[error("SoraFS appeal finance settlement receipt reconciliation status is unsupported")]
    UnsupportedReconciliationStatus,
    /// Missing observed lifecycle status.
    #[error("SoraFS appeal finance settlement receipt observed lifecycle status is required")]
    MissingObservedLifecycleStatus,
    /// Lifecycle status is not valid for a finalized applied receipt.
    #[error("SoraFS appeal finance settlement receipt lifecycle status is unsupported")]
    UnsupportedLifecycleStatus,
    /// Step, reconciliation, and lifecycle labels do not describe one applied state.
    #[error(
        "SoraFS appeal finance settlement receipt step, reconciliation, and lifecycle state are inconsistent"
    )]
    InconsistentFinalizedState,
    /// Label was oversized or outside canonical visible ASCII syntax.
    #[error(
        "SoraFS appeal finance settlement receipt label `{field}` must be at most {max_bytes} bytes of canonical visible ASCII"
    )]
    InvalidLabel {
        /// Field containing the invalid label.
        field: &'static str,
        /// Maximum permitted byte length.
        max_bytes: usize,
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
    /// Source report slice exceeds the deterministic V1 ceiling.
    #[error("SoraFS appeal finance weekly rollup has {found} reports; maximum is {maximum}")]
    TooManyReports {
        /// Supplied report rows.
        found: usize,
        /// Maximum accepted report rows.
        maximum: usize,
    },
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
    /// Canonical encoder cannot provide an allocation-free exact length.
    #[error("SoraFS appeal finance weekly rollup has no exact canonical encoded length")]
    CanonicalLengthUnavailable,
    /// Canonical rollup bytes exceed the first-release ceiling.
    #[error("SoraFS appeal finance weekly rollup has {found} bytes; maximum is {maximum}")]
    PayloadTooLarge {
        /// Exact canonical rollup bytes.
        found: usize,
        /// Maximum accepted canonical bytes.
        maximum: usize,
    },
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
    /// Declared source report count exceeds the deterministic V1 ceiling.
    #[error("SoraFS appeal finance weekly rollup has {found} source reports; maximum is {maximum}")]
    TooManySourceReports {
        /// Declared report count.
        found: u64,
        /// Maximum accepted report count.
        maximum: usize,
    },
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
    /// Config-version inventory exceeds the deterministic V1 ceiling.
    #[error(
        "SoraFS appeal finance weekly rollup has {found} config versions; maximum is {maximum}"
    )]
    TooManyConfigVersions {
        /// Supplied config-version rows.
        found: usize,
        /// Maximum accepted config-version rows.
        maximum: usize,
    },
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
    /// Source report ids are not in strictly increasing canonical order.
    #[error("SoraFS appeal finance weekly rollup source report ids must be sorted")]
    UnsortedSourceReportIds,
    /// Source report id inventory exceeds the deterministic V1 ceiling.
    #[error(
        "SoraFS appeal finance weekly rollup has {found} source report ids; maximum is {maximum}"
    )]
    TooManySourceReportIds {
        /// Supplied source report ids.
        found: usize,
        /// Maximum accepted source report ids.
        maximum: usize,
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
    /// Outcome rows exceed the seven-value V1 enum inventory.
    #[error("SoraFS appeal finance weekly rollup has {found} outcomes; maximum is {maximum}")]
    TooManyOutcomes {
        /// Supplied outcome rows.
        found: usize,
        /// Maximum accepted outcome rows.
        maximum: usize,
    },
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
    /// Outcome rows are not in strict enum order.
    #[error("SoraFS appeal finance weekly rollup outcomes must be in canonical enum order")]
    UnsortedOutcomes,
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
    #[error("governance DAG block has no exact canonical encoded length")]
    CanonicalLengthUnavailable,
    #[error("governance DAG block has {found} canonical bytes; maximum is {maximum}")]
    BlockTooLarge { found: usize, maximum: usize },
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
        #[source]
        source: Box<GovernanceDagBlockValidationError>,
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
/// A root history begins at sequence zero. A checkpoint tail may begin at a non-zero sequence and
/// leave the first block and node parent references outside the supplied slice. Every later block
/// and node must link exactly to its predecessor in the supplied root-to-head order. This
/// standalone validator requires one stable publisher identity and key across the supplied slice.
/// Rotation-aware consumers must separately authenticate their authority transitions and then bind
/// the signed head with [`validate_governance_dag_head_against_rotatable_chain_v1`].
pub fn validate_governance_dag_chain_v1(
    blocks: &[GovernanceDagBlockV1],
    expected_head_cid: Option<&[u8]>,
) -> Result<(), GovernanceDagChainValidationError> {
    validate_governance_dag_chain_with_authority_policy_v1(blocks, expected_head_cid, true)
}
fn validate_governance_dag_chain_with_authority_policy_v1(
    blocks: &[GovernanceDagBlockV1],
    expected_head_cid: Option<&[u8]>,
    require_stable_authority: bool,
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
            .map_err(|source| GovernanceDagChainValidationError::InvalidBlock {
                index,
                source: Box::new(source),
            })?;
        if !block_cids.insert(block.block_cid.clone()) {
            return Err(GovernanceDagChainValidationError::DuplicateBlockCid { index });
        }
        if !node_cids.insert(block.node.node_cid.clone()) {
            return Err(GovernanceDagChainValidationError::DuplicateNodeCid { index });
        }
        if require_stable_authority && block.publisher_peer_id != *publisher_peer_id {
            return Err(GovernanceDagChainValidationError::PublisherPeerMismatch { index });
        }
        if require_stable_authority && block.block_signature.public_key != *publisher_public_key {
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
/// Validates a stable-authority signed head against a full history or its exact newest
/// checkpoint window.
///
/// Full histories start at sequence zero and contain `head.block_count` blocks. Histories of at
/// most 64 blocks omit the checkpoint; longer histories commit the first block in their newest
/// 64-block window. A bounded checkpoint replay supplies exactly those newest 64 blocks, beginning
/// at sequence `head.block_count - 64`. One publisher identity and key must sign every supplied
/// block, and that authority must also sign the head.
pub fn validate_governance_dag_head_against_chain_v1(
    head: &GovernanceDagHeadV1,
    blocks: &[GovernanceDagBlockV1],
) -> Result<(), GovernanceDagHeadChainValidationError> {
    validate_governance_dag_head_against_chain_with_authority_policy_v1(head, blocks, true)
}
/// Validates a signed head against a chain whose authority rotations were authenticated separately.
///
/// This performs every structural, signature, checkpoint, count, and timestamp check made by
/// [`validate_governance_dag_head_against_chain_v1`], but permits predecessor blocks to have
/// different publisher identities and keys. The head must still be signed by the newest supplied
/// block's authority. Callers must authenticate every authority transition before trusting this
/// result; this helper deliberately does not define or verify a rotation policy.
pub fn validate_governance_dag_head_against_rotatable_chain_v1(
    head: &GovernanceDagHeadV1,
    blocks: &[GovernanceDagBlockV1],
) -> Result<(), GovernanceDagHeadChainValidationError> {
    validate_governance_dag_head_against_chain_with_authority_policy_v1(head, blocks, false)
}
fn validate_governance_dag_head_against_chain_with_authority_policy_v1(
    head: &GovernanceDagHeadV1,
    blocks: &[GovernanceDagBlockV1],
    require_stable_authority: bool,
) -> Result<(), GovernanceDagHeadChainValidationError> {
    head.validate()
        .map_err(GovernanceDagHeadChainValidationError::Head)?;
    validate_governance_dag_chain_with_authority_policy_v1(
        blocks,
        Some(&head.head_block_cid),
        require_stable_authority,
    )
    .map_err(GovernanceDagHeadChainValidationError::Chain)?;
    let first = blocks
        .first()
        .ok_or(GovernanceDagHeadChainValidationError::Chain(
            GovernanceDagChainValidationError::Empty,
        ))?;
    let tip = blocks
        .last()
        .ok_or(GovernanceDagHeadChainValidationError::Chain(
            GovernanceDagChainValidationError::Empty,
        ))?;
    if head.publisher_peer_id != tip.publisher_peer_id {
        return Err(GovernanceDagHeadChainValidationError::PublisherPeerMismatch);
    }
    if head.head_signature.public_key != tip.block_signature.public_key {
        return Err(GovernanceDagHeadChainValidationError::PublisherKeyMismatch);
    }
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
#[path = "governance/tests.rs"]
mod tests;

#[cfg(test)]
include!("governance/captured_owner_identity_tests.rs");

#[cfg(test)]
#[path = "governance/borrowed_payload_tests.rs"]
mod borrowed_payload_tests;

#[cfg(test)]
#[path = "governance/signing_identity_tests.rs"]
pub(crate) mod signing_identity_tests;
