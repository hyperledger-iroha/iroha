//! Authoritative on-chain SoraFS moderation commit/reveal records.
//!
//! The local moderation runtime remains useful for orchestration, but these
//! records define the consensus-owned first-release source of truth for ballot
//! policy, lifecycle transitions, challenges, outcomes, and no-show penalties.

use std::collections::BTreeSet;

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    account::AccountId,
    sorafs::moderation::{SoraFsModerationBallotContextV1, SoraFsModerationVoteChoice},
};

/// First-release moderation-ledger policy version.
pub const MODERATION_LEDGER_POLICY_VERSION_V1: u16 = 1;
/// First-release moderation case specification version.
pub const MODERATION_LEDGER_CASE_VERSION_V1: u16 = 1;
/// Hard upper bound for a first-release moderation panel.
pub const MODERATION_LEDGER_MAX_PANEL_SIZE_V1: u16 = 128;
/// Hard upper bound for challenges retained by one ballot.
pub const MODERATION_LEDGER_MAX_CHALLENGES_V1: u16 = 128;
/// Hard upper bound for one ballot's complete lifetime.
pub const MODERATION_LEDGER_MAX_TOTAL_WINDOW_MS_V1: u64 = 90 * 24 * 60 * 60 * 1_000;
/// Maximum reveal nonce length accepted by the authoritative ledger.
pub const MODERATION_LEDGER_MAX_NONCE_BYTES_V1: usize = 64;
/// Maximum case, round, policy, finance-version, or challenge identifier length.
pub const MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1: usize = 256;
/// Maximum payload-free challenge reason length.
pub const MODERATION_LEDGER_MAX_REASON_BYTES_V1: usize = 512;
/// Maximum evidence URI length in an authoritative case context.
pub const MODERATION_LEDGER_MAX_EVIDENCE_URI_BYTES_V1: usize = 2_048;
/// Maximum configured penalty points for one no-show.
pub const MODERATION_LEDGER_MAX_PENALTY_POINTS_V1: u32 = 1_000_000;
/// Domain separator for moderation policy digests.
pub const MODERATION_LEDGER_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.ledger-policy.v1";
/// Roster hash domain shared with the local first-release ballot lifecycle.
pub const MODERATION_LEDGER_ROSTER_HASH_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.panel-roster-hash.v1";

/// Governance-controlled limits and no-show penalties for authoritative ballots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerPolicyV1 {
    /// Schema version; must equal [`MODERATION_LEDGER_POLICY_VERSION_V1`].
    pub version: u16,
    /// Monotonic revision, beginning at one.
    pub revision: u64,
    /// Digest of the immediately preceding policy, absent only for revision one.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub predecessor_policy_digest: Option<[u8; 32]>,
    /// Largest panel governance permits for one case.
    pub max_panel_size: u16,
    /// Largest complete interval from case opening through reveal closure.
    pub max_total_window_ms: u64,
    /// Largest number of distinct challenges retained by one case.
    pub max_challenges_per_case: u16,
    /// Penalty points recorded for a juror that never committed.
    pub missing_commit_penalty_points: u32,
    /// Penalty points recorded for a juror that committed but never revealed.
    pub unrevealed_commit_penalty_points: u32,
}

impl ModerationLedgerPolicyV1 {
    /// Validate all hard first-release policy bounds.
    ///
    /// # Errors
    ///
    /// Returns [`ModerationLedgerPolicyError`] for unsupported versions,
    /// malformed revision links, zero bounds, or values above hard ceilings.
    pub fn validate(&self) -> Result<(), ModerationLedgerPolicyError> {
        if self.version != MODERATION_LEDGER_POLICY_VERSION_V1 {
            return Err(ModerationLedgerPolicyError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.revision == 0 {
            return Err(ModerationLedgerPolicyError::ZeroRevision);
        }
        match (self.revision, self.predecessor_policy_digest) {
            (1, None) => {}
            (1, Some(_)) => return Err(ModerationLedgerPolicyError::UnexpectedPredecessor),
            (_, Some(digest)) if digest != [0; 32] => {}
            _ => return Err(ModerationLedgerPolicyError::MissingPredecessor),
        }
        if !(1..=MODERATION_LEDGER_MAX_PANEL_SIZE_V1).contains(&self.max_panel_size) {
            return Err(ModerationLedgerPolicyError::InvalidPanelSize {
                found: self.max_panel_size,
            });
        }
        if !(1..=MODERATION_LEDGER_MAX_TOTAL_WINDOW_MS_V1).contains(&self.max_total_window_ms) {
            return Err(ModerationLedgerPolicyError::InvalidTotalWindow {
                found: self.max_total_window_ms,
            });
        }
        if !(1..=MODERATION_LEDGER_MAX_CHALLENGES_V1).contains(&self.max_challenges_per_case) {
            return Err(ModerationLedgerPolicyError::InvalidChallengeLimit {
                found: self.max_challenges_per_case,
            });
        }
        if !(1..=MODERATION_LEDGER_MAX_PENALTY_POINTS_V1)
            .contains(&self.missing_commit_penalty_points)
            || !(1..=MODERATION_LEDGER_MAX_PENALTY_POINTS_V1)
                .contains(&self.unrevealed_commit_penalty_points)
        {
            return Err(ModerationLedgerPolicyError::InvalidPenaltyPoints {
                missing_commit: self.missing_commit_penalty_points,
                unrevealed_commit: self.unrevealed_commit_penalty_points,
            });
        }
        Ok(())
    }

    /// Compute the canonical domain-separated policy digest.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical serialization fails.
    pub fn digest(&self) -> Result<[u8; 32], norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(MODERATION_LEDGER_POLICY_DIGEST_DOMAIN_V1);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }
}

/// Validation errors for the authoritative moderation policy.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ModerationLedgerPolicyError {
    /// Unsupported schema version.
    #[error("unsupported moderation ledger policy version {found}")]
    UnsupportedVersion {
        /// Version carried by the policy.
        found: u16,
    },
    /// Revision zero is invalid.
    #[error("moderation ledger policy revision must be non-zero")]
    ZeroRevision,
    /// Revision one unexpectedly carries a predecessor.
    #[error("moderation ledger policy revision one must not carry a predecessor")]
    UnexpectedPredecessor,
    /// A later revision lacks a non-zero predecessor.
    #[error("moderation ledger policy revision after one requires a non-zero predecessor")]
    MissingPredecessor,
    /// The configured panel bound is outside hard limits.
    #[error("invalid moderation ledger panel-size limit {found}")]
    InvalidPanelSize {
        /// Configured bound.
        found: u16,
    },
    /// The configured total ballot window is outside hard limits.
    #[error("invalid moderation ledger total-window limit {found} ms")]
    InvalidTotalWindow {
        /// Configured bound.
        found: u64,
    },
    /// The configured challenge bound is outside hard limits.
    #[error("invalid moderation ledger challenge limit {found}")]
    InvalidChallengeLimit {
        /// Configured bound.
        found: u16,
    },
    /// One or both penalty values are zero or exceed the hard ceiling.
    #[error(
        "invalid moderation no-show penalties missing_commit={missing_commit} unrevealed_commit={unrevealed_commit}"
    )]
    InvalidPenaltyPoints {
        /// Missing-commit penalty.
        missing_commit: u32,
        /// Unrevealed-commit penalty.
        unrevealed_commit: u32,
    },
}

/// Activated moderation policy with consensus provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerPolicyRecord {
    /// Policy body.
    pub policy: ModerationLedgerPolicyV1,
    /// Canonical policy digest.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Block timestamp at activation.
    pub activated_at_unix_ms: u64,
    /// Governance authority that activated the policy.
    pub activated_by: AccountId,
}

/// Immutable input used to open one authoritative moderation ballot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationCaseSpecV1 {
    /// Schema version; must equal [`MODERATION_LEDGER_CASE_VERSION_V1`].
    pub version: u16,
    /// Immutable evidence, finance, roster, and policy scope.
    pub context: SoraFsModerationBallotContextV1,
    /// Ballot round identifier.
    pub round_id: String,
    /// Ordered canonical juror accounts.
    pub jurors: Vec<AccountId>,
    /// Minimum valid reveals required for a decision or contested outcome.
    pub quorum: u16,
    /// Last block timestamp at which commitments are accepted.
    pub commit_deadline_unix_ms: u64,
    /// Last block timestamp in the challenge buffer.
    pub challenge_deadline_unix_ms: u64,
    /// Last block timestamp at which reveals are accepted.
    pub reveal_deadline_unix_ms: u64,
    /// Active policy digest the opener expects.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
}

impl ModerationCaseSpecV1 {
    /// Validate context, identifiers, roster uniqueness, quorum, and deadline order.
    ///
    /// Policy-specific ceilings and the opening block time are enforced by the
    /// authoritative instruction handler.
    ///
    /// # Errors
    ///
    /// Returns [`ModerationCaseSpecError`] when structural material is invalid.
    pub fn validate(&self) -> Result<(), ModerationCaseSpecError> {
        if self.version != MODERATION_LEDGER_CASE_VERSION_V1 {
            return Err(ModerationCaseSpecError::UnsupportedVersion {
                found: self.version,
            });
        }
        self.context
            .validate()
            .map_err(|error| ModerationCaseSpecError::InvalidContext(error.to_string()))?;
        validate_identifier("case_id", &self.context.case_id)?;
        validate_identifier("round_id", &self.round_id)?;
        validate_identifier(
            "appeal_finance_config_version",
            &self.context.appeal_finance_config_version,
        )?;
        validate_identifier("policy_reference", &self.context.policy_reference)?;
        if self.context.evidence_uri.as_ref().is_some_and(|uri| {
            uri.is_empty() || uri.len() > MODERATION_LEDGER_MAX_EVIDENCE_URI_BYTES_V1
        }) {
            return Err(ModerationCaseSpecError::InvalidEvidenceUri);
        }
        if self.jurors.is_empty()
            || self.jurors.len() > usize::from(MODERATION_LEDGER_MAX_PANEL_SIZE_V1)
        {
            return Err(ModerationCaseSpecError::InvalidRosterSize {
                found: self.jurors.len(),
            });
        }
        let mut seen = BTreeSet::new();
        for juror in &self.jurors {
            let canonical = juror.to_string();
            if !seen.insert(canonical.clone()) {
                return Err(ModerationCaseSpecError::DuplicateJuror { juror: canonical });
            }
        }
        if self.quorum == 0 || usize::from(self.quorum) > self.jurors.len() {
            return Err(ModerationCaseSpecError::InvalidQuorum {
                quorum: self.quorum,
                roster_size: self.jurors.len(),
            });
        }
        if self.context.panel_roster_hash
            != sorafs_moderation_panel_roster_hash_v1(&self.jurors, self.quorum)
        {
            return Err(ModerationCaseSpecError::RosterHashMismatch);
        }
        if self.commit_deadline_unix_ms >= self.challenge_deadline_unix_ms
            || self.challenge_deadline_unix_ms >= self.reveal_deadline_unix_ms
        {
            return Err(ModerationCaseSpecError::InvalidDeadlines);
        }
        if self.policy_digest == [0; 32] {
            return Err(ModerationCaseSpecError::ZeroPolicyDigest);
        }
        Ok(())
    }
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), ModerationCaseSpecError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1
    {
        return Err(ModerationCaseSpecError::InvalidIdentifier {
            field,
            length: value.len(),
        });
    }
    Ok(())
}

/// Structural moderation-case errors.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModerationCaseSpecError {
    /// Unsupported case schema version.
    #[error("unsupported moderation case version {found}")]
    UnsupportedVersion {
        /// Version carried by the case.
        found: u16,
    },
    /// The embedded ballot context failed validation.
    #[error("invalid moderation case context: {0}")]
    InvalidContext(String),
    /// A bounded identifier is empty, padded, or too long.
    #[error("invalid moderation case {field} identifier length {length}")]
    InvalidIdentifier {
        /// Invalid field.
        field: &'static str,
        /// Byte length observed.
        length: usize,
    },
    /// Evidence URI is empty or too long.
    #[error("moderation case evidence URI is empty or exceeds the first-release bound")]
    InvalidEvidenceUri,
    /// Roster length is outside hard bounds.
    #[error("invalid moderation case roster size {found}")]
    InvalidRosterSize {
        /// Roster length observed.
        found: usize,
    },
    /// Roster contains a duplicate canonical account.
    #[error("duplicate moderation juror {juror}")]
    DuplicateJuror {
        /// Duplicated account.
        juror: String,
    },
    /// Quorum is zero or exceeds roster length.
    #[error("invalid moderation quorum {quorum} for roster size {roster_size}")]
    InvalidQuorum {
        /// Requested quorum.
        quorum: u16,
        /// Roster length.
        roster_size: usize,
    },
    /// Context roster hash does not commit to the ordered canonical roster and quorum.
    #[error("moderation case panel roster hash mismatch")]
    RosterHashMismatch,
    /// Deadlines are not strictly commit, challenge, reveal ordered.
    #[error("moderation case deadlines must satisfy commit < challenge < reveal")]
    InvalidDeadlines,
    /// Expected policy digest is zero.
    #[error("moderation case policy digest must be non-zero")]
    ZeroPolicyDigest,
}

/// Derive the roster digest shared by local and authoritative first-release ballots.
#[must_use]
pub fn sorafs_moderation_panel_roster_hash_v1(jurors: &[AccountId], quorum: u16) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_LEDGER_ROSTER_HASH_DOMAIN_V1);
    hasher.update(&quorum.to_le_bytes());
    let roster_size = u64::try_from(jurors.len()).expect("roster length fits u64");
    hasher.update(&roster_size.to_le_bytes());
    for juror in jurors {
        let canonical = juror.to_string();
        let canonical_len =
            u64::try_from(canonical.len()).expect("account literal length fits u64");
        hasher.update(&canonical_len.to_le_bytes());
        hasher.update(canonical.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

/// Consensus lifecycle status of an authoritative case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", rename_all = "snake_case")
)]
pub enum ModerationCaseStatusV1 {
    /// Commit, challenge, or reveal processing remains possible by block time.
    Open,
    /// An accepted challenge permanently blocks reveal/tally processing.
    Challenged,
    /// A terminal outcome and any no-show records have been persisted.
    Finalized,
}

/// Authoritative case header and constant-time lifecycle counters.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationCaseRecordV1 {
    /// Immutable case specification.
    pub spec: ModerationCaseSpecV1,
    /// Immutable policy snapshot fixing resource bounds and no-show penalties.
    pub policy: ModerationLedgerPolicyV1,
    /// Current consensus lifecycle status.
    pub status: ModerationCaseStatusV1,
    /// Block timestamp at opening.
    pub opened_at_unix_ms: u64,
    /// Governance authority that opened the case.
    pub opened_by: AccountId,
    /// Number of accepted commitments.
    pub commitment_count: u32,
    /// Number of accepted reveals.
    pub reveal_count: u32,
    /// Number of submitted challenges.
    pub challenge_count: u32,
    /// Number of unresolved challenges.
    pub pending_challenge_count: u32,
    /// Number of accepted challenges.
    pub accepted_challenge_count: u32,
}

/// Immutable accepted juror commitment and ledger provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationCommitRecordV1 {
    /// Case identifier.
    pub case_id: String,
    /// Ballot round identifier.
    pub round_id: String,
    /// Canonical juror authority.
    pub juror: AccountId,
    /// Exact canonical `SoraFsModerationBallotCommitV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_commit: Vec<u8>,
    /// Block timestamp assigned at admission.
    pub accepted_at_unix_ms: u64,
}

/// Immutable accepted juror reveal and ledger provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationRevealRecordV1 {
    /// Case identifier.
    pub case_id: String,
    /// Ballot round identifier.
    pub round_id: String,
    /// Canonical juror authority.
    pub juror: AccountId,
    /// Exact canonical `SoraFsModerationBallotRevealV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_reveal: Vec<u8>,
    /// Block timestamp assigned at admission.
    pub accepted_at_unix_ms: u64,
}

/// Payload-free authoritative challenge category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum ModerationChallengeKindV1 {
    /// Announced roster or roster digest is disputed.
    RosterMismatch,
    /// Duplicate commitment evidence is disputed.
    DuplicateCommit,
    /// Commit/reveal payload binding is disputed.
    PayloadMismatch,
    /// Juror eligibility is disputed.
    JurorEligibility,
    /// Evidence or policy binding is disputed.
    EvidenceMismatch,
    /// Bounded operator-reviewed category outside fixed labels.
    Other,
}

impl ModerationChallengeKindV1 {
    /// Return whether this category requires a target juror.
    #[must_use]
    pub fn requires_target_juror(self) -> bool {
        matches!(
            self,
            Self::DuplicateCommit | Self::PayloadMismatch | Self::JurorEligibility
        )
    }
}

/// Governance resolution for one challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "decision", content = "value", rename_all = "snake_case")
)]
pub enum ModerationChallengeDecisionV1 {
    /// Challenge was rejected and normal ballot processing may resume.
    Rejected,
    /// Challenge was accepted and the ballot must close as challenged.
    Accepted,
}

/// Durable payload-free challenge and optional resolution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationChallengeRecordV1 {
    /// Case identifier.
    pub case_id: String,
    /// Ballot round identifier.
    pub round_id: String,
    /// Challenge id unique within the case and round.
    pub challenge_id: String,
    /// Canonical challenger authority.
    pub challenger: AccountId,
    /// Fixed payload-free challenge kind.
    pub kind: ModerationChallengeKindV1,
    /// Optional juror target required by juror-scoped kinds.
    pub target_juror: Option<AccountId>,
    /// Digest of external challenge evidence; raw evidence is never stored.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub evidence_digest: [u8; 32],
    /// Bounded payload-free reason label.
    pub reason: String,
    /// Block timestamp at admission.
    pub raised_at_unix_ms: u64,
    /// Governance decision, absent while pending.
    pub decision: Option<ModerationChallengeDecisionV1>,
    /// Authority that resolved the challenge.
    pub resolved_by: Option<AccountId>,
    /// Block timestamp at resolution.
    pub resolved_at_unix_ms: Option<u64>,
}

/// Vote counts in a terminal authoritative outcome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationVoteCountsV1 {
    /// `uphold` reveals.
    pub uphold: u32,
    /// `overturn` reveals.
    pub overturn: u32,
    /// `modify` reveals.
    pub modify: u32,
    /// `escalate` reveals.
    pub escalate: u32,
}

impl ModerationVoteCountsV1 {
    /// Return the sum of all choice counters when it fits in `u32`.
    #[must_use]
    pub fn checked_total(self) -> Option<u32> {
        self.uphold
            .checked_add(self.overturn)?
            .checked_add(self.modify)?
            .checked_add(self.escalate)
    }

    /// Return the unique highest-vote choice, or `None` for empty/tied counts.
    #[must_use]
    pub fn winning_choice(self) -> Option<SoraFsModerationVoteChoice> {
        let choices = [
            (SoraFsModerationVoteChoice::Uphold, self.uphold),
            (SoraFsModerationVoteChoice::Overturn, self.overturn),
            (SoraFsModerationVoteChoice::Modify, self.modify),
            (SoraFsModerationVoteChoice::Escalate, self.escalate),
        ];
        let maximum = choices.iter().map(|(_, count)| *count).max().unwrap_or(0);
        if maximum == 0
            || choices
                .iter()
                .filter(|(_, count)| *count == maximum)
                .count()
                != 1
        {
            return None;
        }
        choices
            .into_iter()
            .find_map(|(choice, count)| (count == maximum).then_some(choice))
    }
}

/// Terminal classification for an authoritative moderation case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum ModerationOutcomeKindV1 {
    /// Quorum was met and one choice had a unique plurality.
    Decided(SoraFsModerationVoteChoice),
    /// Quorum was met but the highest vote count was tied.
    Contested,
    /// Reveal count did not satisfy quorum.
    QuorumNotMet,
    /// An accepted challenge blocked reveal and tally processing.
    Challenged,
}

/// Immutable terminal outcome for one case and round.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationOutcomeRecordV1 {
    /// Case identifier.
    pub case_id: String,
    /// Ballot round identifier.
    pub round_id: String,
    /// Terminal classification.
    pub kind: ModerationOutcomeKindV1,
    /// Deterministically recomputed vote counts.
    pub counts: ModerationVoteCountsV1,
    /// Number of valid reveals included.
    pub votes_total: u32,
    /// Required reveal quorum.
    pub quorum: u16,
    /// Number of no-show records emitted by finalization.
    pub no_show_count: u32,
    /// Block timestamp at finalization.
    pub finalized_at_unix_ms: u64,
    /// Governance authority that finalized the case.
    pub finalized_by: AccountId,
}

/// Classification of one ballot no-show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum ModerationNoShowKindV1 {
    /// Juror submitted no accepted commitment.
    MissingCommit,
    /// Juror committed but submitted no accepted reveal.
    UnrevealedCommit,
}

/// Durable no-show penalty record derived atomically during finalization.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationNoShowRecordV1 {
    /// Case identifier.
    pub case_id: String,
    /// Ballot round identifier.
    pub round_id: String,
    /// Canonical absent juror.
    pub juror: AccountId,
    /// Whether the juror never committed or committed without revealing.
    pub kind: ModerationNoShowKindV1,
    /// Policy-controlled points recorded for downstream reputation settlement.
    pub penalty_points: u32,
    /// Policy digest that fixed the penalty value.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Block timestamp assigned at finalization.
    pub recorded_at_unix_ms: u64,
}

/// Constant-time authoritative moderation-ledger counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerStatusV1 {
    /// Open, including challenged-but-not-finalized, cases.
    pub open_cases: u64,
    /// Finalized cases.
    pub finalized_cases: u64,
    /// Accepted commitments.
    pub commitments: u64,
    /// Accepted reveals.
    pub reveals: u64,
    /// Submitted challenges.
    pub challenges: u64,
    /// Persisted terminal outcomes.
    pub outcomes: u64,
    /// Persisted no-show penalty records.
    pub no_shows: u64,
    /// Block timestamp of the latest ledger mutation.
    pub updated_at_unix_ms: u64,
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::sorafs::moderation::{
        SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1, SoraFsModerationBallotContextV1,
    };

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
            .expect("nonzero deterministic Ed25519 seed");
        AccountId::new(keypair.public_key().clone())
    }

    fn policy() -> ModerationLedgerPolicyV1 {
        ModerationLedgerPolicyV1 {
            version: MODERATION_LEDGER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            max_panel_size: 5,
            max_total_window_ms: 60_000,
            max_challenges_per_case: 4,
            missing_commit_penalty_points: 10,
            unrevealed_commit_penalty_points: 20,
        }
    }

    fn case_spec() -> ModerationCaseSpecV1 {
        let jurors = vec![account(1), account(2), account(3)];
        ModerationCaseSpecV1 {
            version: MODERATION_LEDGER_CASE_VERSION_V1,
            context: SoraFsModerationBallotContextV1 {
                version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                case_id: "case-1".to_owned(),
                evidence_bundle_digest: [1; 32],
                appeal_finance_config_version: "finance-v1".to_owned(),
                panel_roster_hash: sorafs_moderation_panel_roster_hash_v1(&jurors, 2),
                policy_reference: "policy-v1".to_owned(),
                evidence_uri: Some("ipfs://evidence".to_owned()),
            },
            round_id: "round-1".to_owned(),
            jurors,
            quorum: 2,
            commit_deadline_unix_ms: 2_000,
            challenge_deadline_unix_ms: 3_000,
            reveal_deadline_unix_ms: 4_000,
            policy_digest: policy().digest().unwrap(),
        }
    }

    #[test]
    fn policy_digest_and_case_roundtrip_are_stable() {
        let policy = policy();
        policy.validate().unwrap();
        assert_eq!(policy.digest().unwrap(), policy.digest().unwrap());

        let case = case_spec();
        case.validate().unwrap();
        let encoded = norito::to_bytes(&case).unwrap();
        let decoded: ModerationCaseSpecV1 = norito::decode_from_bytes(&encoded).unwrap();
        assert_eq!(decoded, case);
    }

    #[test]
    fn policy_rejects_zero_and_overflowing_bounds() {
        let mut candidate = policy();
        candidate.max_panel_size = 0;
        assert!(matches!(
            candidate.validate(),
            Err(ModerationLedgerPolicyError::InvalidPanelSize { found: 0 })
        ));
        candidate = policy();
        candidate.max_total_window_ms = MODERATION_LEDGER_MAX_TOTAL_WINDOW_MS_V1 + 1;
        assert!(matches!(
            candidate.validate(),
            Err(ModerationLedgerPolicyError::InvalidTotalWindow { .. })
        ));
        candidate = policy();
        candidate.unrevealed_commit_penalty_points = u32::MAX;
        assert!(matches!(
            candidate.validate(),
            Err(ModerationLedgerPolicyError::InvalidPenaltyPoints { .. })
        ));
    }

    #[test]
    fn case_rejects_duplicate_roster_bad_hash_and_inverted_windows() {
        let mut candidate = case_spec();
        candidate.jurors[2] = candidate.jurors[0].clone();
        assert!(matches!(
            candidate.validate(),
            Err(ModerationCaseSpecError::DuplicateJuror { .. })
        ));

        candidate = case_spec();
        candidate.context.panel_roster_hash[0] ^= 1;
        assert_eq!(
            candidate.validate(),
            Err(ModerationCaseSpecError::RosterHashMismatch)
        );

        candidate = case_spec();
        candidate.challenge_deadline_unix_ms = candidate.commit_deadline_unix_ms;
        assert_eq!(
            candidate.validate(),
            Err(ModerationCaseSpecError::InvalidDeadlines)
        );
    }

    #[test]
    fn outcome_count_helpers_detect_ties_and_overflow() {
        let counts = ModerationVoteCountsV1 {
            uphold: 2,
            overturn: 1,
            modify: 0,
            escalate: 0,
        };
        assert_eq!(counts.checked_total(), Some(3));
        assert_eq!(
            counts.winning_choice(),
            Some(SoraFsModerationVoteChoice::Uphold)
        );
        assert_eq!(
            ModerationVoteCountsV1 {
                uphold: 2,
                overturn: 2,
                modify: 0,
                escalate: 0,
            }
            .winning_choice(),
            None
        );
        assert_eq!(
            ModerationVoteCountsV1 {
                uphold: u32::MAX,
                overturn: 1,
                modify: 0,
                escalate: 0,
            }
            .checked_total(),
            None
        );
    }
}
