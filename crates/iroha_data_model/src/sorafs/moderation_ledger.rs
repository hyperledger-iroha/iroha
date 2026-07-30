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
    events::data::sorafs::{SorafsModerationLedgerEvent, SorafsRepairLedgerEvent},
    sorafs::moderation::{SoraFsModerationBallotContextV1, SoraFsModerationVoteChoice},
};

/// First-release moderation-ledger policy version.
pub const MODERATION_LEDGER_POLICY_VERSION_V1: u16 = 1;
/// First-release moderation case specification version.
pub const MODERATION_LEDGER_CASE_VERSION_V1: u16 = 1;
/// First-release moderation appeal-intake version.
pub const MODERATION_APPEAL_INTAKE_VERSION_V1: u16 = 1;
/// Hard upper bound for a first-release moderation panel.
pub const MODERATION_LEDGER_MAX_PANEL_SIZE_V1: u16 = 128;
/// Hard upper bound for one appeal's PoP-eligible candidate pool.
pub const MODERATION_LEDGER_MAX_CANDIDATE_POOL_SIZE_V1: u16 = 1_024;
/// Hard upper bound for one appeal's deterministic failover waitlist.
pub const MODERATION_LEDGER_MAX_WAITLIST_SIZE_V1: u16 = 128;
/// Hard upper bound for conflict-of-interest exclusions on one appeal.
pub const MODERATION_LEDGER_MAX_EXCLUSIONS_V1: u16 = 256;
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
/// First-release finalized moderation snapshot schema version.
pub const MODERATION_FINALIZED_SNAPSHOT_VERSION_V1: u16 = 1;
/// Hard upper bound for appeals and activated cases in one finalized snapshot.
pub const MODERATION_QUERY_MAX_CASES_V1: u32 = 1_024;
/// Hard upper bound for committed events returned by one moderation query.
pub const MODERATION_QUERY_MAX_EVENTS_V1: u32 = 4_096;
/// Hard upper bound for one encoded finalized moderation snapshot.
pub const MODERATION_QUERY_MAX_SNAPSHOT_BYTES_V1: usize = 32 * 1024 * 1024;
/// Hard upper bound for one encoded committed moderation-event page.
pub const MODERATION_QUERY_MAX_EVENT_PAGE_BYTES_V1: usize = 8 * 1024 * 1024;
/// Domain separator for moderation policy digests.
pub const MODERATION_LEDGER_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.ledger-policy.v1";
/// Roster hash domain shared with the local first-release ballot lifecycle.
pub const MODERATION_LEDGER_ROSTER_HASH_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.panel-roster-hash.v1";
/// Domain separator for immutable appeal-intake digests.
pub const MODERATION_APPEAL_INTAKE_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.appeal-intake.v1";
/// Domain separator for pinned `PoP` registry snapshots.
pub const MODERATION_POP_SNAPSHOT_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.pop-snapshot.v1";
/// Domain separator for the shared, per-appeal `PoP` proof challenge.
pub const MODERATION_POP_CHALLENGE_DOMAIN_V1: &[u8] = b"sorafs.moderation.pop-challenge.v1";
/// Domain separator for deterministic panel-selection seed derivation.
pub const MODERATION_SORTITION_SEED_DOMAIN_V1: &[u8] = b"sorafs.moderation.sortition-seed.v1";
/// Domain separator for deterministic candidate scores.
pub const MODERATION_SORTITION_SCORE_DOMAIN_V1: &[u8] = b"sorafs.moderation.sortition-score.v1";
/// Domain separator for selected roster and waitlist commitments.
pub const MODERATION_SORTITION_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.sortition-record.v1";

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
    /// Largest PoP-verified candidate pool retained for one appeal.
    pub max_candidate_pool_size: u16,
    /// Largest deterministic failover waitlist retained for one appeal.
    pub max_waitlist_size: u16,
    /// Largest conflict-of-interest exclusion list retained for one appeal.
    pub max_exclusions_per_case: u16,
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
        if self.max_candidate_pool_size < self.max_panel_size
            || self.max_candidate_pool_size > MODERATION_LEDGER_MAX_CANDIDATE_POOL_SIZE_V1
        {
            return Err(ModerationLedgerPolicyError::InvalidCandidatePoolSize {
                found: self.max_candidate_pool_size,
            });
        }
        if self.max_waitlist_size > MODERATION_LEDGER_MAX_WAITLIST_SIZE_V1 {
            return Err(ModerationLedgerPolicyError::InvalidWaitlistSize {
                found: self.max_waitlist_size,
            });
        }
        if !(1..=MODERATION_LEDGER_MAX_EXCLUSIONS_V1).contains(&self.max_exclusions_per_case) {
            return Err(ModerationLedgerPolicyError::InvalidExclusionLimit {
                found: self.max_exclusions_per_case,
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
        let encoded = norito::encode_canonical(self)?;
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
    /// The candidate-pool bound is smaller than the panel bound or above the hard ceiling.
    #[error("invalid moderation ledger candidate-pool limit {found}")]
    InvalidCandidatePoolSize {
        /// Configured bound.
        found: u16,
    },
    /// The waitlist bound exceeds the hard ceiling.
    #[error("invalid moderation ledger waitlist limit {found}")]
    InvalidWaitlistSize {
        /// Configured bound.
        found: u16,
    },
    /// The conflict-exclusion bound is zero or above the hard ceiling.
    #[error("invalid moderation ledger exclusion limit {found}")]
    InvalidExclusionLimit {
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

/// Immutable active `PoP` registry anchors captured when an appeal is admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationPoPRegistrySnapshotV1 {
    /// Issuer policy digest used to admit both active publications.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub issuer_policy_digest: [u8; 32],
    /// Active private-credential commitment root.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub commitment_root: [u8; 32],
    /// Active commitment-tree version.
    pub commitment_tree_version: u64,
    /// Active sparse revocation root.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub revocation_root: [u8; 32],
    /// Active revocation-list version.
    pub revocation_list_version: u64,
    /// Monotonic registry audit sequence fixing the snapshot.
    pub registry_audit_sequence: u64,
    /// Registry audit-chain head fixing the snapshot.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub registry_audit_head: [u8; 32],
    /// Consensus block timestamp at which the snapshot was captured.
    pub captured_at_unix_ms: u64,
}

impl ModerationPoPRegistrySnapshotV1 {
    /// Validate that every authoritative anchor is present and non-inert.
    ///
    /// # Errors
    ///
    /// Returns an error when a required digest, publication version, audit
    /// sequence, or capture time is zero.
    pub fn validate(&self) -> Result<(), ModerationPoPRegistrySnapshotError> {
        for (field, digest) in [
            ("issuer_policy_digest", self.issuer_policy_digest),
            ("commitment_root", self.commitment_root),
            ("revocation_root", self.revocation_root),
            ("registry_audit_head", self.registry_audit_head),
        ] {
            if digest == [0; 32] {
                return Err(ModerationPoPRegistrySnapshotError::ZeroDigest { field });
            }
        }
        if self.commitment_tree_version == 0
            || self.revocation_list_version == 0
            || self.registry_audit_sequence == 0
        {
            return Err(ModerationPoPRegistrySnapshotError::ZeroVersion);
        }
        if self.captured_at_unix_ms == 0 {
            return Err(ModerationPoPRegistrySnapshotError::ZeroCaptureTime);
        }
        Ok(())
    }

    /// Compute the canonical, domain-separated snapshot digest.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito cannot encode the snapshot.
    pub fn digest(&self) -> Result<[u8; 32], norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(MODERATION_POP_SNAPSHOT_DIGEST_DOMAIN_V1);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }
}

/// Invalid authoritative `PoP` snapshot.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ModerationPoPRegistrySnapshotError {
    /// A required digest is all zeroes.
    #[error("moderation PoP snapshot {field} must be non-zero")]
    ZeroDigest {
        /// Invalid field.
        field: &'static str,
    },
    /// One or more monotonic publication counters are zero.
    #[error("moderation PoP snapshot versions and audit sequence must be non-zero")]
    ZeroVersion,
    /// Snapshot capture time is zero.
    #[error("moderation PoP snapshot capture time must be non-zero")]
    ZeroCaptureTime,
}

/// Authoritative, pre-sortition appeal intake submitted by the appellant.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationAppealIntakeV1 {
    /// Schema version; must equal [`MODERATION_APPEAL_INTAKE_VERSION_V1`].
    pub version: u16,
    /// Unique appeal case identifier.
    pub case_id: String,
    /// Unique ballot round identifier.
    pub round_id: String,
    /// Universal account submitting and owning the appeal.
    pub appellant: AccountId,
    /// Digest of the moderation decision being appealed.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub appealed_decision_digest: [u8; 32],
    /// Single-use digest of proof tokens authorising the appeal without placing them on-chain.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub proof_token_digest: [u8; 32],
    /// Digest of the complete evidence bundle.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub evidence_bundle_digest: [u8; 32],
    /// Single-use digest of the confirmed appeal-deposit lock.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub appeal_deposit_lock_digest: [u8; 32],
    /// Appeal-finance configuration version fixing quote and settlement policy.
    pub appeal_finance_config_version: String,
    /// Moderation policy reference applied to the appeal.
    pub policy_reference: String,
    /// Optional payload-free evidence location.
    pub evidence_uri: Option<String>,
    /// Number of primary jurors to draw.
    pub panel_size: u16,
    /// Maximum number of deterministic failover jurors to retain.
    pub waitlist_size: u16,
    /// Number of valid reveals required for a decision.
    pub quorum: u16,
    /// Canonically ordered conflict-of-interest exclusions; must include the appellant.
    pub exclusions: Vec<AccountId>,
    /// Last timestamp at which private `PoP` proofs may be registered.
    pub registration_deadline_unix_ms: u64,
    /// Last timestamp at which primary jurors may accept their assignment.
    pub acceptance_deadline_unix_ms: u64,
    /// Last timestamp at which commitments are accepted.
    pub commit_deadline_unix_ms: u64,
    /// Last timestamp in the challenge buffer.
    pub challenge_deadline_unix_ms: u64,
    /// Last timestamp at which reveals are accepted.
    pub reveal_deadline_unix_ms: u64,
    /// Active moderation policy digest expected by the appellant.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
}

impl ModerationAppealIntakeV1 {
    /// Validate the bounded appeal body independently of live policy and block time.
    ///
    /// # Errors
    ///
    /// Returns an error when the version, identifiers, digests, panel bounds,
    /// exclusions, evidence URI, or deadline ordering is invalid.
    pub fn validate(&self) -> Result<(), ModerationAppealIntakeError> {
        if self.version != MODERATION_APPEAL_INTAKE_VERSION_V1 {
            return Err(ModerationAppealIntakeError::UnsupportedVersion {
                found: self.version,
            });
        }
        validate_appeal_identifier("case_id", &self.case_id)?;
        validate_appeal_identifier("round_id", &self.round_id)?;
        validate_appeal_identifier(
            "appeal_finance_config_version",
            &self.appeal_finance_config_version,
        )?;
        validate_appeal_identifier("policy_reference", &self.policy_reference)?;
        for (field, digest) in [
            ("appealed_decision_digest", self.appealed_decision_digest),
            ("proof_token_digest", self.proof_token_digest),
            ("evidence_bundle_digest", self.evidence_bundle_digest),
            (
                "appeal_deposit_lock_digest",
                self.appeal_deposit_lock_digest,
            ),
            ("policy_digest", self.policy_digest),
        ] {
            if digest == [0; 32] {
                return Err(ModerationAppealIntakeError::ZeroDigest { field });
            }
        }
        if self.evidence_uri.as_ref().is_some_and(|uri| {
            uri.is_empty()
                || uri.len() > MODERATION_LEDGER_MAX_EVIDENCE_URI_BYTES_V1
                || !uri.is_ascii()
                || uri
                    .bytes()
                    .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        }) {
            return Err(ModerationAppealIntakeError::InvalidEvidenceUri);
        }
        if !(1..=MODERATION_LEDGER_MAX_PANEL_SIZE_V1).contains(&self.panel_size) {
            return Err(ModerationAppealIntakeError::InvalidPanelSize {
                found: self.panel_size,
            });
        }
        if self.waitlist_size > MODERATION_LEDGER_MAX_WAITLIST_SIZE_V1 {
            return Err(ModerationAppealIntakeError::InvalidWaitlistSize {
                found: self.waitlist_size,
            });
        }
        if self.quorum == 0 || self.quorum > self.panel_size {
            return Err(ModerationAppealIntakeError::InvalidQuorum {
                quorum: self.quorum,
                panel_size: self.panel_size,
            });
        }
        if self.exclusions.is_empty()
            || self.exclusions.len() > usize::from(MODERATION_LEDGER_MAX_EXCLUSIONS_V1)
        {
            return Err(ModerationAppealIntakeError::InvalidExclusionCount {
                found: self.exclusions.len(),
            });
        }
        validate_canonical_account_list(&self.exclusions)
            .map_err(ModerationAppealIntakeError::InvalidExclusions)?;
        if self
            .exclusions
            .binary_search_by(|candidate| candidate.to_string().cmp(&self.appellant.to_string()))
            .is_err()
        {
            return Err(ModerationAppealIntakeError::AppellantNotExcluded);
        }
        if !(self.registration_deadline_unix_ms < self.acceptance_deadline_unix_ms
            && self.acceptance_deadline_unix_ms < self.commit_deadline_unix_ms
            && self.commit_deadline_unix_ms < self.challenge_deadline_unix_ms
            && self.challenge_deadline_unix_ms < self.reveal_deadline_unix_ms)
        {
            return Err(ModerationAppealIntakeError::InvalidDeadlines);
        }
        Ok(())
    }

    /// Compute the immutable canonical appeal-intake digest.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito cannot encode the intake.
    pub fn digest(&self) -> Result<[u8; 32], norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(MODERATION_APPEAL_INTAKE_DIGEST_DOMAIN_V1);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }
}

fn validate_appeal_identifier(
    field: &'static str,
    value: &str,
) -> Result<(), ModerationAppealIntakeError> {
    if !is_canonical_moderation_identifier_v1(value) {
        return Err(ModerationAppealIntakeError::InvalidIdentifier {
            field,
            length: value.len(),
        });
    }
    Ok(())
}

/// Return whether a moderation identifier is bounded canonical ASCII.
///
/// This grammar is shared by appeal, case, round, and challenge identifiers so
/// control characters, Unicode confusables, and whitespace cannot create
/// ambiguous state keys or operator displays.
#[must_use]
pub fn is_canonical_moderation_identifier_v1(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/' | b'@')
        })
}

fn validate_canonical_account_list(accounts: &[AccountId]) -> Result<(), String> {
    let mut previous: Option<String> = None;
    for account in accounts {
        let current = account.to_string();
        if previous.as_ref().is_some_and(|value| value >= &current) {
            return Err(format!(
                "accounts must be strictly ordered; duplicate or non-canonical entry `{current}`"
            ));
        }
        previous = Some(current);
    }
    Ok(())
}

/// Structural appeal-intake validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModerationAppealIntakeError {
    /// Unsupported intake version.
    #[error("unsupported moderation appeal intake version {found}")]
    UnsupportedVersion {
        /// Supplied version.
        found: u16,
    },
    /// A bounded identifier is empty or not canonical ASCII.
    #[error(
        "invalid moderation appeal {field} identifier (length {length}; expected bounded canonical ASCII)"
    )]
    InvalidIdentifier {
        /// Invalid field.
        field: &'static str,
        /// Observed byte length.
        length: usize,
    },
    /// Required payload-free digest is inert.
    #[error("moderation appeal {field} must be non-zero")]
    ZeroDigest {
        /// Invalid field.
        field: &'static str,
    },
    /// Optional evidence URI is not bounded, whitespace-free canonical ASCII.
    #[error("moderation appeal evidence URI is not bounded canonical ASCII")]
    InvalidEvidenceUri,
    /// Requested primary panel is outside hard bounds.
    #[error("invalid moderation appeal panel size {found}")]
    InvalidPanelSize {
        /// Supplied size.
        found: u16,
    },
    /// Requested waitlist is outside hard bounds.
    #[error("invalid moderation appeal waitlist size {found}")]
    InvalidWaitlistSize {
        /// Supplied size.
        found: u16,
    },
    /// Quorum is zero or exceeds panel size.
    #[error("invalid moderation appeal quorum {quorum} for panel size {panel_size}")]
    InvalidQuorum {
        /// Supplied quorum.
        quorum: u16,
        /// Supplied panel size.
        panel_size: u16,
    },
    /// Exclusion count is zero or above the hard ceiling.
    #[error("invalid moderation appeal exclusion count {found}")]
    InvalidExclusionCount {
        /// Supplied count.
        found: usize,
    },
    /// Exclusions are duplicated or not canonically ordered.
    #[error("invalid moderation appeal exclusions: {0}")]
    InvalidExclusions(String),
    /// Appellant could otherwise enter their own panel.
    #[error("moderation appeal exclusions must contain the appellant")]
    AppellantNotExcluded,
    /// Lifecycle deadlines are not strictly ordered.
    #[error(
        "moderation appeal deadlines must satisfy registration < acceptance < commit < challenge < reveal"
    )]
    InvalidDeadlines,
}

/// Public eligibility class retained without credential or attribute disclosure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "class", content = "value", rename_all = "snake_case")
)]
pub enum ModerationJurorEligibilityClassV1 {
    /// General juror pool.
    General,
    /// Region-scoped juror pool.
    Regional,
    /// Domain-expert juror pool.
    Expert,
    /// Emergency juror pool.
    Emergency,
    /// Observer-only credentials; never eligible for a voting panel.
    Observer,
}

/// Payload-free result of one verified private `PoP` membership proof.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationJurorEligibilityRecordV1 {
    /// Appeal case identifier.
    pub case_id: String,
    /// Ballot round identifier.
    pub round_id: String,
    /// Universal account that presented and owns the proof.
    pub juror: AccountId,
    /// Eligibility class proven by the hidden credential.
    pub eligibility_class: ModerationJurorEligibilityClassV1,
    /// Digest of exact canonical proof bytes; proof bytes are not persisted.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub proof_digest: [u8; 32],
    /// Per-credential, per-appeal nullifier preventing duplicate-person entries.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub nullifier: [u8; 32],
    /// `PoP` registry snapshot digest against which the proof verified.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub pop_snapshot_digest: [u8; 32],
    /// Hidden credential expiry disclosed by the membership statement.
    pub credential_expires_at_epoch: u64,
    /// Consensus block timestamp at admission.
    pub registered_at_unix_ms: u64,
}

/// Deterministically selected primary panel and failover queue.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationPanelSelectionV1 {
    /// Exact already-committed parent block fixed only after registration closes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub randomness_anchor: [u8; 32],
    /// Deterministic seed digest fixed by appeal, `PoP` snapshot, and parent block.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub seed_digest: [u8; 32],
    /// Primary jurors in canonical score order.
    pub jurors: Vec<AccountId>,
    /// Failover jurors in canonical score order.
    pub waitlist: Vec<AccountId>,
    /// Digest committing to snapshot, seed, primary roster, waitlist, and quorum.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub sortition_digest: [u8; 32],
    /// Consensus block timestamp at selection.
    pub selected_at_unix_ms: u64,
    /// Authorised moderation operator that closed registration.
    pub selected_by: AccountId,
}

/// One deterministic primary-juror no-show replacement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationJurorReplacementV1 {
    /// Primary juror who did not accept by the deadline.
    pub absent_juror: AccountId,
    /// Next unused juror in the immutable waitlist.
    pub replacement_juror: AccountId,
}

/// Consensus-owned appeal/sortition lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", rename_all = "snake_case")
)]
pub enum ModerationAppealStatusV1 {
    /// Appeal admitted; private `PoP` eligibility proofs are being collected.
    RegisteringJurors,
    /// Primary panel and waitlist selected; primary acceptance window is open.
    AwaitingAcceptance,
    /// Commit/reveal case activated with accepted primaries and replacements.
    BallotOpen,
    /// Eligible pool could not fill the requested primary panel.
    InsufficientEligiblePool,
    /// Primary no-shows exceeded the immutable failover waitlist.
    FailoverExhausted,
    /// Underlying authoritative ballot reached a terminal outcome.
    Finalized,
}

/// Authoritative appeal intake, `PoP` snapshot, sortition, and activation record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationAppealRecordV1 {
    /// Immutable appellant intake.
    pub intake: ModerationAppealIntakeV1,
    /// Canonical intake digest.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub intake_digest: [u8; 32],
    /// Immutable active moderation-policy snapshot.
    pub policy: ModerationLedgerPolicyV1,
    /// Immutable active `PoP` registry snapshot.
    pub pop_snapshot: ModerationPoPRegistrySnapshotV1,
    /// Canonical `PoP` snapshot digest.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub pop_snapshot_digest: [u8; 32],
    /// Current appeal lifecycle phase.
    pub status: ModerationAppealStatusV1,
    /// Appellant authority that submitted the intake.
    pub submitted_by: AccountId,
    /// Consensus block timestamp at intake.
    pub submitted_at_unix_ms: u64,
    /// Canonically account-ordered `PoP`-eligible candidates.
    pub eligible_jurors: Vec<AccountId>,
    /// Selected primary panel and waitlist after registration closes.
    pub selection: Option<ModerationPanelSelectionV1>,
    /// Canonically account-ordered primary assignment acceptances.
    pub accepted_jurors: Vec<AccountId>,
    /// Slot-ordered deterministic primary no-show replacements.
    pub replacements: Vec<ModerationJurorReplacementV1>,
    /// Underlying ballot activation time, if activated.
    pub activated_at_unix_ms: Option<u64>,
    /// Underlying ballot finalization time, if finalized.
    pub finalized_at_unix_ms: Option<u64>,
}

/// Return the shared per-appeal `PoP` membership-proof challenge.
#[must_use]
pub fn sorafs_moderation_pop_challenge_v1(
    intake_digest: [u8; 32],
    pop_snapshot_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_POP_CHALLENGE_DOMAIN_V1);
    hasher.update(&intake_digest);
    hasher.update(&pop_snapshot_digest);
    *hasher.finalize().as_bytes()
}

/// Return the bounded verifier context used by every candidate for one appeal.
#[must_use]
pub fn sorafs_moderation_pop_verifier_context_v1(intake_digest: [u8; 32]) -> String {
    format!(
        "sorafs-moderation-sortition-v1:{}",
        hex::encode(intake_digest)
    )
}

/// Derive the immutable selection seed from appeal and non-applicant anchors.
#[must_use]
pub fn sorafs_moderation_sortition_seed_v1(
    intake_digest: [u8; 32],
    pop_snapshot_digest: [u8; 32],
    randomness_anchor: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_SORTITION_SEED_DOMAIN_V1);
    hasher.update(&intake_digest);
    hasher.update(&pop_snapshot_digest);
    hasher.update(&randomness_anchor);
    *hasher.finalize().as_bytes()
}

fn sortition_score(
    seed_digest: [u8; 32],
    candidate: &ModerationJurorEligibilityRecordV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_SORTITION_SCORE_DOMAIN_V1);
    hasher.update(&seed_digest);
    hasher.update(&candidate.nullifier);
    *hasher.finalize().as_bytes()
}

/// Commit to one deterministic panel/waitlist result.
#[must_use]
pub fn sorafs_moderation_sortition_digest_v1(
    pop_snapshot_digest: [u8; 32],
    seed_digest: [u8; 32],
    jurors: &[AccountId],
    waitlist: &[AccountId],
    quorum: u16,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_SORTITION_DIGEST_DOMAIN_V1);
    hasher.update(&pop_snapshot_digest);
    hasher.update(&seed_digest);
    hasher.update(&quorum.to_le_bytes());
    for accounts in [jurors, waitlist] {
        hasher.update(&(accounts.len() as u64).to_le_bytes());
        for account in accounts {
            let canonical = account.to_string();
            hasher.update(&(canonical.len() as u64).to_le_bytes());
            hasher.update(canonical.as_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

/// Successful output of deterministic moderation panel selection.
///
/// The tuple contains the primary jurors, waitlist, seed digest, and sortition
/// digest, in that order.
pub type ModerationPanelSelectionResultV1 = (Vec<AccountId>, Vec<AccountId>, [u8; 32], [u8; 32]);

/// Select a panel and bounded failover queue independently of candidate input order.
///
/// # Errors
///
/// Returns an error for invalid bounds or candidates, duplicate accounts or
/// person nullifiers, or a candidate pool too small to fill the primary panel.
pub fn sorafs_moderation_select_panel_v1(
    intake_digest: [u8; 32],
    pop_snapshot_digest: [u8; 32],
    randomness_anchor: [u8; 32],
    candidates: &[ModerationJurorEligibilityRecordV1],
    panel_size: u16,
    waitlist_size: u16,
    quorum: u16,
) -> Result<ModerationPanelSelectionResultV1, ModerationSortitionError> {
    if !(1..=MODERATION_LEDGER_MAX_PANEL_SIZE_V1).contains(&panel_size)
        || quorum == 0
        || quorum > panel_size
        || waitlist_size > MODERATION_LEDGER_MAX_WAITLIST_SIZE_V1
        || candidates.len() > usize::from(MODERATION_LEDGER_MAX_CANDIDATE_POOL_SIZE_V1)
    {
        return Err(ModerationSortitionError::InvalidBounds);
    }
    let seed_digest =
        sorafs_moderation_sortition_seed_v1(intake_digest, pop_snapshot_digest, randomness_anchor);
    let mut accounts = BTreeSet::new();
    let mut nullifiers = BTreeSet::new();
    let mut scored = Vec::with_capacity(candidates.len());
    let scope = candidates
        .first()
        .map(|candidate| (&candidate.case_id, &candidate.round_id));
    for candidate in candidates {
        if candidate.proof_digest == [0; 32]
            || candidate.nullifier == [0; 32]
            || candidate.pop_snapshot_digest != pop_snapshot_digest
            || !is_canonical_moderation_identifier_v1(&candidate.case_id)
            || !is_canonical_moderation_identifier_v1(&candidate.round_id)
            || scope.is_some_and(|(case_id, round_id)| {
                &candidate.case_id != case_id || &candidate.round_id != round_id
            })
            || candidate.registered_at_unix_ms == 0
            || candidate.credential_expires_at_epoch == 0
            || candidate.eligibility_class == ModerationJurorEligibilityClassV1::Observer
        {
            return Err(ModerationSortitionError::InvalidCandidate {
                juror: candidate.juror.to_string(),
            });
        }
        let account = candidate.juror.to_string();
        if !accounts.insert(account.clone()) {
            return Err(ModerationSortitionError::DuplicateJuror { juror: account });
        }
        if !nullifiers.insert(candidate.nullifier) {
            return Err(ModerationSortitionError::DuplicatePersonNullifier);
        }
        scored.push((
            sortition_score(seed_digest, candidate),
            account,
            candidate.juror.clone(),
        ));
    }
    scored.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    if scored.len() < usize::from(panel_size) {
        return Err(ModerationSortitionError::InsufficientEligiblePool {
            required: panel_size,
            found: scored.len(),
        });
    }
    let jurors = scored
        .iter()
        .take(usize::from(panel_size))
        .map(|(_, _, account)| account.clone())
        .collect::<Vec<_>>();
    let waitlist = scored
        .iter()
        .skip(usize::from(panel_size))
        .take(usize::from(waitlist_size))
        .map(|(_, _, account)| account.clone())
        .collect::<Vec<_>>();
    let digest = sorafs_moderation_sortition_digest_v1(
        pop_snapshot_digest,
        seed_digest,
        &jurors,
        &waitlist,
        quorum,
    );
    Ok((jurors, waitlist, seed_digest, digest))
}

/// Deterministic sortition validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModerationSortitionError {
    /// Requested sizes exceed hard bounds.
    #[error("invalid moderation sortition bounds")]
    InvalidBounds,
    /// Candidate is structurally invalid or observer-only.
    #[error("invalid moderation sortition candidate {juror}")]
    InvalidCandidate {
        /// Invalid juror.
        juror: String,
    },
    /// One account appears more than once.
    #[error("duplicate moderation sortition juror {juror}")]
    DuplicateJuror {
        /// Duplicated juror.
        juror: String,
    },
    /// One hidden credential was presented through multiple accounts.
    #[error("duplicate moderation sortition person nullifier")]
    DuplicatePersonNullifier,
    /// Verified candidates cannot fill the primary panel.
    #[error("insufficient moderation eligible pool: required {required}, found {found}")]
    InsufficientEligiblePool {
        /// Required primary count.
        required: u16,
        /// Verified candidate count.
        found: usize,
    },
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
            uri.is_empty()
                || uri.len() > MODERATION_LEDGER_MAX_EVIDENCE_URI_BYTES_V1
                || !uri.is_ascii()
                || uri
                    .bytes()
                    .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
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
    if !is_canonical_moderation_identifier_v1(value) {
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
    /// A bounded identifier is empty or not canonical ASCII.
    #[error(
        "invalid moderation case {field} identifier (length {length}; expected bounded canonical ASCII)"
    )]
    InvalidIdentifier {
        /// Invalid field.
        field: &'static str,
        /// Byte length observed.
        length: usize,
    },
    /// Evidence URI is not bounded, whitespace-free canonical ASCII.
    #[error("moderation case evidence URI is not bounded canonical ASCII")]
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
    /// Canonically ordered challenge identifiers used for bounded terminal expiry.
    pub challenge_ids: Vec<String>,
    /// Number of unresolved challenges.
    pub pending_challenge_count: u32,
    /// Number of accepted challenges.
    pub accepted_challenge_count: u32,
    /// Number of challenges that expired unresolved and forced fail-safe closure.
    pub expired_challenge_count: u32,
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
    /// Challenge was not resolved before the reveal window closed.
    ///
    /// Finalization derives this state and closes fail-safe without penalizing
    /// jurors who were prevented from revealing.
    Expired,
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
    /// Authoritative appeal intakes admitted.
    pub appeal_intakes: u64,
    /// Private `PoP` eligibility proofs admitted without retaining proof bodies.
    pub eligibility_proofs: u64,
    /// Deterministic panel selections persisted.
    pub panel_selections: u64,
    /// Primary assignment acceptances persisted.
    pub assignment_acceptances: u64,
    /// Primary no-shows replaced from immutable waitlists.
    pub failover_replacements: u64,
    /// Appeals terminally unable to form a panel.
    pub failed_panel_formations: u64,
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

/// Finalized block anchor for one coherent moderation query result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationFinalizedCursorV1 {
    /// Finalized block height observed by the immutable state view.
    pub height: u64,
    /// Finalized block hash resolved from that same state view.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
}

/// Exclusive cursor for one committed moderation event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationFinalizedEventCursorV1 {
    /// Monotonic moderation-event sequence beginning at one.
    pub sequence: u64,
    /// Finalized block height containing the event.
    pub block_height: u64,
    /// Finalized block hash resolved only after the block commits.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Moderation-event index within the committing block.
    pub event_index: u32,
}

/// Typed moderation event with an unambiguous finalized-chain cursor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationFinalizedEventV1 {
    /// Monotonic moderation-event sequence beginning at one.
    pub sequence: u64,
    /// Committing block height.
    pub block_height: u64,
    /// Committing block hash resolved from finalized state.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Moderation-event index within the committing block.
    pub event_index: u32,
    /// Existing typed, payload-free native moderation event.
    pub event: SorafsModerationLedgerEvent,
}

impl ModerationFinalizedEventV1 {
    /// Return the exclusive cursor identifying this event.
    #[must_use]
    pub const fn cursor(&self) -> ModerationFinalizedEventCursorV1 {
        ModerationFinalizedEventCursorV1 {
            sequence: self.sequence,
            block_height: self.block_height,
            block_hash: self.block_hash,
            event_index: self.event_index,
        }
    }
}

/// Cursor-bounded page of typed committed moderation events.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationFinalizedEventPageV1 {
    /// Finalized state anchor shared by every event in the page.
    pub finalized_cursor: ModerationFinalizedCursorV1,
    /// Events in strictly increasing sequence and block/index order.
    pub events: Vec<ModerationFinalizedEventV1>,
    /// Whether at least one later committed event exists at this anchor.
    pub has_more: bool,
    /// Exclusive continuation cursor, present only when `has_more` is true.
    pub next_after: Option<ModerationFinalizedEventCursorV1>,
}

/// One appeal and every payload-free eligibility record from one finalized view.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationFinalizedAppealViewV1 {
    /// Authoritative appeal, sortition, and activation record.
    pub appeal: ModerationAppealRecordV1,
    /// Eligibility records sorted by canonical juror identity.
    pub eligibility: Vec<ModerationJurorEligibilityRecordV1>,
}

/// One case and every authoritative ballot subrecord from one finalized view.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationFinalizedCaseViewV1 {
    /// Authoritative case header.
    pub case: ModerationCaseRecordV1,
    /// Commitments sorted by canonical juror identity.
    pub commits: Vec<ModerationCommitRecordV1>,
    /// Reveals sorted by canonical juror identity.
    pub reveals: Vec<ModerationRevealRecordV1>,
    /// Challenges sorted by challenge identifier.
    pub challenges: Vec<ModerationChallengeRecordV1>,
    /// Terminal outcome, when the case is finalized.
    pub outcome: Option<ModerationOutcomeRecordV1>,
    /// No-show records sorted by canonical juror identity.
    pub no_shows: Vec<ModerationNoShowRecordV1>,
}

/// Complete bounded moderation projection read from one finalized state view.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationFinalizedLedgerSnapshotV1 {
    /// Schema version; must equal [`MODERATION_FINALIZED_SNAPSHOT_VERSION_V1`].
    pub version: u16,
    /// Finalized block height.
    pub finalized_height: u64,
    /// Finalized block hash resolved from the same immutable state view.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub finalized_block_hash: [u8; 32],
    /// Signed creation timestamp of that exact finalized block, in Unix milliseconds.
    pub finalized_at_unix_ms: u64,
    /// Active moderation policy, absent only before ledger initialization.
    pub policy: Option<ModerationLedgerPolicyRecord>,
    /// Constant-time ledger counters, absent only before initialization.
    pub status: Option<ModerationLedgerStatusV1>,
    /// Every retained appeal sorted by `(case_id, round_id)`.
    pub appeals: Vec<ModerationFinalizedAppealViewV1>,
    /// Every retained activated case sorted by `(case_id, round_id)`.
    pub cases: Vec<ModerationFinalizedCaseViewV1>,
    /// Latest bounded committed-event suffix in increasing cursor order.
    pub events: Vec<ModerationFinalizedEventV1>,
}

impl ModerationFinalizedLedgerSnapshotV1 {
    /// Return the finalized anchor shared by the complete snapshot.
    #[must_use]
    pub const fn anchor(&self) -> ModerationFinalizedCursorV1 {
        ModerationFinalizedCursorV1 {
            height: self.finalized_height,
            block_hash: self.finalized_block_hash,
        }
    }

    /// Return one appeal by exact case and round identifier.
    #[must_use]
    pub fn appeal(
        &self,
        case_id: &str,
        round_id: &str,
    ) -> Option<&ModerationFinalizedAppealViewV1> {
        self.appeals
            .binary_search_by(|entry| {
                (
                    entry.appeal.intake.case_id.as_str(),
                    entry.appeal.intake.round_id.as_str(),
                )
                    .cmp(&(case_id, round_id))
            })
            .ok()
            .map(|index| &self.appeals[index])
    }

    /// Return one activated case by exact case and round identifier.
    #[must_use]
    pub fn case(&self, case_id: &str, round_id: &str) -> Option<&ModerationFinalizedCaseViewV1> {
        self.cases
            .binary_search_by(|entry| {
                (
                    entry.case.spec.context.case_id.as_str(),
                    entry.case.spec.round_id.as_str(),
                )
                    .cmp(&(case_id, round_id))
            })
            .ok()
            .map(|index| &self.cases[index])
    }
}

/// First-release chain-authoritative repair-task record version.
pub const REPAIR_LEDGER_TASK_VERSION_V1: u16 = 1;
/// Shortest lease accepted by the chain-authoritative repair ledger.
pub const REPAIR_LEDGER_MIN_LEASE_MS_V1: u64 = 1_000;
/// Longest lease accepted by the chain-authoritative repair ledger.
pub const REPAIR_LEDGER_MAX_LEASE_MS_V1: u64 = 24 * 60 * 60 * 1_000;
/// Maximum idempotency receipts retained by one repair task.
pub const REPAIR_LEDGER_MAX_RECEIPTS_V1: usize = 64;
/// Maximum UTF-8 byte length of one repair idempotency key.
pub const REPAIR_LEDGER_MAX_IDEMPOTENCY_KEY_BYTES_V1: usize = 256;
/// Maximum payload-free repair appeal reason length.
pub const REPAIR_LEDGER_MAX_APPEAL_REASON_BYTES_V1: usize = 512;
/// Hard encoded-byte ceiling for a canonical repair report or slash proposal.
pub const REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1: usize = 64 * 1024;
/// Hard upper bound for tasks or committed events returned by one repair query.
pub const REPAIR_QUERY_MAX_ITEMS_V1: u32 = 500;
/// Hard encoded-byte ceiling for one finalized repair-task page.
pub const REPAIR_QUERY_MAX_TASK_PAGE_BYTES_V1: usize = 8 * 1024 * 1024;
/// Hard encoded-byte ceiling for one committed repair-event page.
pub const REPAIR_QUERY_MAX_EVENT_PAGE_BYTES_V1: usize = 512 * 1024;
/// Domain separator for immutable repair-task identities.
pub const REPAIR_LEDGER_TASK_ID_DOMAIN_V1: &[u8] = b"sorafs.repair.ledger-task-id.v1";
/// Domain separator for repair action idempotency keys.
pub const REPAIR_LEDGER_IDEMPOTENCY_DOMAIN_V1: &[u8] = b"sorafs.repair.ledger-idempotency-key.v1";
/// Domain separator for authority-bound canonical repair actions.
pub const REPAIR_LEDGER_ACTION_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.repair.action-digest.v1";
/// Domain separator for repair appeal identities.
pub const REPAIR_LEDGER_APPEAL_ID_DOMAIN_V1: &[u8] = b"sorafs.repair.ledger-appeal-id.v1";

/// Derive the immutable task identity from a subsystem's exactly-once source identity.
#[must_use]
pub fn sorafs_repair_task_id_v1(source_identity: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(REPAIR_LEDGER_TASK_ID_DOMAIN_V1);
    hasher.update(&source_identity);
    *hasher.finalize().as_bytes()
}

/// Derive a task-scoped digest for a bounded idempotency key.
#[must_use]
pub fn sorafs_repair_idempotency_digest_v1(ticket_id: &str, key: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(REPAIR_LEDGER_IDEMPOTENCY_DOMAIN_V1);
    for value in [ticket_id, key] {
        hasher.update(
            &u64::try_from(value.len())
                .expect("string length fits u64")
                .to_le_bytes(),
        );
        hasher.update(value.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

/// Derive the consensus action digest for one authority-bound repair instruction.
///
/// Clients and durable transaction forwarders use this exact helper to
/// reconcile an idempotency receipt against finalized state. Keeping the
/// preimage construction in the data model prevents replicas from
/// reimplementing a consensus-private hash contract.
///
/// # Errors
///
/// Returns an error when Norito cannot encode `action`.
pub fn sorafs_repair_action_digest_v1<T: norito::core::NoritoSerialize>(
    authority: &AccountId,
    action: &T,
) -> Result<[u8; 32], norito::Error> {
    let bytes = norito::encode_canonical(action)?;
    let authority = authority.to_string();
    let mut hasher = blake3::Hasher::new();
    hasher.update(REPAIR_LEDGER_ACTION_DIGEST_DOMAIN_V1);
    hasher.update(
        &u64::try_from(authority.len())
            .expect("string length fits u64")
            .to_le_bytes(),
    );
    hasher.update(authority.as_bytes());
    hasher.update(
        &u64::try_from(bytes.len())
            .expect("slice length fits u64")
            .to_le_bytes(),
    );
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}

/// Consensus-owned repair worker lease.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerLeaseV1 {
    /// Canonical account that exclusively owns the lease.
    pub owner: AccountId,
    /// Monotonic lease generation, incremented on every successful claim.
    pub generation: u64,
    /// Block timestamp at which the lease was acquired.
    pub acquired_at_unix_ms: u64,
    /// Block timestamp of the latest accepted claim or heartbeat.
    pub renewed_at_unix_ms: u64,
    /// Exclusive lease expiry; actions at or after this timestamp are rejected.
    pub expires_at_unix_ms: u64,
}

/// Successful repair outcome payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerCompletedV1 {
    /// Digest of the completion evidence.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub evidence_digest: [u8; 32],
}

/// Unsuccessful repair outcome payload without slashing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerFailedV1 {
    /// Digest of the payload-free failure reason or external evidence.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub failure_digest: [u8; 32],
}

/// Escalated repair outcome payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerEscalatedV1 {
    /// Digest of the exact canonical slash proposal.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub slash_proposal_digest: [u8; 32],
}

/// One immutable terminal repair outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum RepairLedgerTerminalKindV1 {
    /// Repair completed and the external verification evidence is committed by digest.
    Completed(RepairLedgerCompletedV1),
    /// Repair failed without a slash proposal.
    Failed(RepairLedgerFailedV1),
    /// Repair failed and atomically committed a slash proposal.
    Escalated(RepairLedgerEscalatedV1),
}

/// Provenance for the single terminal outcome of a repair task.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerTerminalOutcomeV1 {
    /// Terminal result.
    pub kind: RepairLedgerTerminalKindV1,
    /// Lease generation whose owner committed the result.
    pub lease_generation: u64,
    /// Worker account that committed the result.
    pub finalized_by: AccountId,
    /// Finalizing block timestamp.
    pub finalized_at_unix_ms: u64,
}

/// Idempotent repair action accepted by consensus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerActionReceiptV1 {
    /// Task-scoped idempotency key digest.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub idempotency_digest: [u8; 32],
    /// Digest of the exact canonical action body.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub action_digest: [u8; 32],
    /// Task revision produced by the action.
    pub resulting_revision: u64,
}

/// Slash proposal committed atomically with an escalated terminal outcome.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerSlashRecordV1 {
    /// Exact canonical `sorafs_manifest::repair::RepairSlashProposalV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_proposal: Vec<u8>,
    /// Digest of `canonical_proposal`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub proposal_digest: [u8; 32],
    /// Worker that committed the proposal.
    pub submitted_by: AccountId,
    /// Committing block timestamp.
    pub submitted_at_unix_ms: u64,
}

/// Provider appeal committed against one escalated repair slash proposal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerAppealRecordV1 {
    /// Deterministic appeal identity.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub appeal_id: [u8; 32],
    /// Digest of the slash proposal being appealed.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub slash_proposal_digest: [u8; 32],
    /// Provider-owner account that submitted the appeal.
    pub appellant: AccountId,
    /// Digest of external appeal evidence.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub evidence_digest: [u8; 32],
    /// Bounded payload-free reason.
    pub reason: String,
    /// Committing block timestamp.
    pub submitted_at_unix_ms: u64,
}

/// Derive the immutable identity of a repair slash appeal.
#[must_use]
pub fn sorafs_repair_appeal_id_v1(
    task_id: [u8; 32],
    slash_proposal_digest: [u8; 32],
    appellant: &AccountId,
    evidence_digest: [u8; 32],
    reason: &str,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(REPAIR_LEDGER_APPEAL_ID_DOMAIN_V1);
    hasher.update(&task_id);
    hasher.update(&slash_proposal_digest);
    let appellant = appellant.to_string();
    for value in [appellant.as_str(), reason] {
        hasher.update(
            &u64::try_from(value.len())
                .expect("string length fits u64")
                .to_le_bytes(),
        );
        hasher.update(value.as_bytes());
    }
    hasher.update(&evidence_digest);
    *hasher.finalize().as_bytes()
}

/// Chain-authoritative repair task, lease, terminal result, slash, and appeal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerTaskV1 {
    /// Schema version; must equal [`REPAIR_LEDGER_TASK_VERSION_V1`].
    pub version: u16,
    /// Immutable task identity derived from the exact canonical report.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub task_id: [u8; 32],
    /// Non-zero exactly-once identity supplied by the originating proof subsystem.
    ///
    /// `PoTR` uses the signed receipt digest; other subsystems use an equally
    /// stable canonical failure or handoff digest.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub source_identity: [u8; 32],
    /// Canonical repair ticket identifier.
    pub ticket_id: String,
    /// Exact canonical `sorafs_manifest::repair::RepairReportV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_report: Vec<u8>,
    /// Manifest under repair.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub manifest_digest: [u8; 32],
    /// Provider responsible for the affected replica.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub provider_id: [u8; 32],
    /// Auditor account that admitted the task.
    pub submitted_by: AccountId,
    /// Admission block timestamp.
    pub submitted_at_unix_ms: u64,
    /// Monotonic task revision.
    pub revision: u64,
    /// Current exclusive worker lease, including an expired lease until reclaimed.
    pub lease: Option<RepairLedgerLeaseV1>,
    /// Immutable terminal outcome, when finalized.
    pub terminal_outcome: Option<RepairLedgerTerminalOutcomeV1>,
    /// Slash proposal committed with an escalated terminal outcome.
    pub slash: Option<RepairLedgerSlashRecordV1>,
    /// Single provider appeal against the committed slash proposal.
    pub appeal: Option<RepairLedgerAppealRecordV1>,
    /// Bounded idempotency receipts ordered by acceptance.
    pub action_receipts: Vec<RepairLedgerActionReceiptV1>,
    /// Block timestamp of the latest accepted mutation.
    pub updated_at_unix_ms: u64,
}

/// Constant-time counters for the authoritative repair ledger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerStatusV1 {
    /// Admitted repair tasks.
    pub tasks: u64,
    /// Tasks that currently retain a lease record.
    pub leased_tasks: u64,
    /// Persisted terminal outcomes.
    pub terminal_outcomes: u64,
    /// Completed terminal outcomes.
    pub completed: u64,
    /// Failed terminal outcomes without a slash proposal.
    pub failed: u64,
    /// Escalated terminal outcomes with a slash proposal.
    pub escalated: u64,
    /// Committed slash proposals.
    pub slash_proposals: u64,
    /// Committed provider appeals.
    pub appeals: u64,
    /// Block timestamp of the latest mutation.
    pub updated_at_unix_ms: u64,
}

/// Finalized block anchor for one coherent repair-ledger query result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairFinalizedCursorV1 {
    /// Finalized block height observed by the immutable state view.
    pub height: u64,
    /// Finalized block hash resolved from that same immutable state view.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
}

/// One authoritative repair task anchored to finalized chain state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairFinalizedTaskV1 {
    /// Finalized state anchor at which the task was read.
    pub finalized_cursor: RepairFinalizedCursorV1,
    /// Chain-authoritative repair task.
    pub task: RepairLedgerTaskV1,
}

/// Authoritative repair counters anchored to finalized chain state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairFinalizedStatusV1 {
    /// Finalized state anchor at which the counters were read.
    pub finalized_cursor: RepairFinalizedCursorV1,
    /// Chain-authoritative repair-ledger counters.
    pub status: RepairLedgerStatusV1,
}

/// Cursor-bounded authoritative repair-task page.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairLedgerTaskPageV1 {
    /// Finalized state anchor shared by every task in the page.
    pub finalized_cursor: RepairFinalizedCursorV1,
    /// Canonical records in ascending immutable task-id order.
    pub tasks: Vec<RepairLedgerTaskV1>,
    /// Whether at least one later task exists at this anchor.
    pub has_more: bool,
    /// Exclusive task-id cursor for the next page, present only when `has_more` is true.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub next_after_task_id: Option<[u8; 32]>,
}

/// Exclusive cursor for one committed repair-ledger event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairFinalizedEventCursorV1 {
    /// Monotonic repair-event sequence beginning at one.
    pub sequence: u64,
    /// Finalized block height containing the event.
    pub block_height: u64,
    /// Finalized block hash resolved only after the block commits.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Repair-event index within the committing block.
    pub event_index: u32,
}

/// Typed repair-ledger event with an unambiguous finalized-chain cursor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairFinalizedEventV1 {
    /// Monotonic repair-event sequence beginning at one.
    pub sequence: u64,
    /// Committing block height.
    pub block_height: u64,
    /// Committing block hash resolved from finalized state.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Repair-event index within the committing block.
    pub event_index: u32,
    /// Existing typed, payload-free native repair-ledger event.
    pub event: SorafsRepairLedgerEvent,
}

impl RepairFinalizedEventV1 {
    /// Return the exclusive cursor identifying this event.
    #[must_use]
    pub const fn cursor(&self) -> RepairFinalizedEventCursorV1 {
        RepairFinalizedEventCursorV1 {
            sequence: self.sequence,
            block_height: self.block_height,
            block_hash: self.block_hash,
            event_index: self.event_index,
        }
    }
}

/// Cursor-bounded page of typed committed repair-ledger events.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RepairFinalizedEventPageV1 {
    /// Finalized state anchor shared by every event in the page.
    pub finalized_cursor: RepairFinalizedCursorV1,
    /// Events in strictly increasing sequence and block/index order.
    pub events: Vec<RepairFinalizedEventV1>,
    /// Whether at least one later committed event exists at this anchor.
    pub has_more: bool,
    /// Exclusive continuation cursor, present only when `has_more` is true.
    pub next_after: Option<RepairFinalizedEventCursorV1>,
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

    fn encode_with_alternate_norito_layout<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(value).expect("encode alternate-layout repair-ledger value")
    }

    fn assert_canonical_norito_round_trip<T>(value: &T)
    where
        T: core::fmt::Debug + PartialEq + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let encoded =
            norito::encode_canonical(value).expect("encode canonical repair-ledger value");
        let decoded: T =
            norito::decode_canonical(&encoded).expect("decode canonical repair-ledger value");
        assert_eq!(&decoded, value);
        assert_eq!(
            norito::encode_canonical(&decoded).expect("re-encode canonical repair-ledger value"),
            encoded
        );
        let alternate = encode_with_alternate_norito_layout(value);
        assert_ne!(alternate, encoded);
        let alternate_decoded: T = norito::decode_from_bytes(&alternate)
            .expect("alternate-layout repair-ledger value remains structurally decodable");
        assert_eq!(&alternate_decoded, value);
        assert!(matches!(
            norito::decode_canonical::<T>(&alternate),
            Err(norito::Error::NonCanonicalEncoding)
        ));
    }

    fn policy() -> ModerationLedgerPolicyV1 {
        ModerationLedgerPolicyV1 {
            version: MODERATION_LEDGER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            max_panel_size: 5,
            max_candidate_pool_size: 32,
            max_waitlist_size: 5,
            max_exclusions_per_case: 16,
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

    fn appeal_intake() -> ModerationAppealIntakeV1 {
        let appellant = account(9);
        ModerationAppealIntakeV1 {
            version: MODERATION_APPEAL_INTAKE_VERSION_V1,
            case_id: "appeal-1".to_owned(),
            round_id: "round-1".to_owned(),
            appellant: appellant.clone(),
            appealed_decision_digest: [0x11; 32],
            proof_token_digest: [0x12; 32],
            evidence_bundle_digest: [0x13; 32],
            appeal_deposit_lock_digest: [0x14; 32],
            appeal_finance_config_version: "finance-v1".to_owned(),
            policy_reference: "policy-v1".to_owned(),
            evidence_uri: Some("ipfs://appeal-evidence".to_owned()),
            panel_size: 3,
            waitlist_size: 2,
            quorum: 2,
            exclusions: vec![appellant],
            registration_deadline_unix_ms: 2_000,
            acceptance_deadline_unix_ms: 3_000,
            commit_deadline_unix_ms: 4_000,
            challenge_deadline_unix_ms: 5_000,
            reveal_deadline_unix_ms: 6_000,
            policy_digest: policy().digest().unwrap(),
        }
    }

    fn eligibility(seed: u8, snapshot_digest: [u8; 32]) -> ModerationJurorEligibilityRecordV1 {
        ModerationJurorEligibilityRecordV1 {
            case_id: "appeal-1".to_owned(),
            round_id: "round-1".to_owned(),
            juror: account(seed),
            eligibility_class: ModerationJurorEligibilityClassV1::General,
            proof_digest: [seed.wrapping_add(0x20); 32],
            nullifier: [seed.wrapping_add(0x40); 32],
            pop_snapshot_digest: snapshot_digest,
            credential_expires_at_epoch: 10_000,
            registered_at_unix_ms: 1_500,
        }
    }

    #[test]
    fn policy_digest_and_case_roundtrip_are_stable() {
        let policy = policy();
        policy.validate().unwrap();
        assert_eq!(policy.digest().unwrap(), policy.digest().unwrap());

        let case = case_spec();
        case.validate().unwrap();
        assert_canonical_norito_round_trip(&case);
    }

    #[test]
    fn ledger_identity_digests_ignore_ambient_norito_layout() {
        let policy = policy();
        let snapshot = ModerationPoPRegistrySnapshotV1 {
            issuer_policy_digest: [0x11; 32],
            commitment_root: [0x22; 32],
            commitment_tree_version: 7,
            revocation_root: [0x33; 32],
            revocation_list_version: 8,
            registry_audit_sequence: 9,
            registry_audit_head: [0x44; 32],
            captured_at_unix_ms: 10,
        };
        let intake = appeal_intake();
        let authority = account(12);
        let action = "canonical-repair-action".to_owned();
        let expected = (
            policy.digest().expect("policy digest"),
            snapshot.digest().expect("snapshot digest"),
            intake.digest().expect("appeal-intake digest"),
            sorafs_repair_action_digest_v1(&authority, &action).expect("repair action digest"),
        );
        let canonical = norito::encode_canonical(&action).expect("canonical repair action");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_ne!(
            norito::to_bytes(&action).expect("ambient-layout repair action"),
            canonical
        );
        assert_eq!(
            (
                policy.digest().expect("policy digest"),
                snapshot.digest().expect("snapshot digest"),
                intake.digest().expect("appeal-intake digest"),
                sorafs_repair_action_digest_v1(&authority, &action).expect("repair action digest"),
            ),
            expected
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn finalized_event_page_json_roundtrip_preserves_typed_event() {
        let event = ModerationFinalizedEventV1 {
            sequence: 7,
            block_height: 11,
            block_hash: [0xAB; 32],
            event_index: 2,
            event: SorafsModerationLedgerEvent {
                kind: crate::events::data::sorafs::SorafsModerationLedgerEventKind::CaseFinalized,
                case_id: Some("case-1".to_owned()),
                round_id: Some("round-1".to_owned()),
                authority: account(7),
                occurred_at_unix_ms: 4_321,
            },
        };
        let page = ModerationFinalizedEventPageV1 {
            finalized_cursor: ModerationFinalizedCursorV1 {
                height: event.block_height,
                block_hash: event.block_hash,
            },
            events: vec![event.clone()],
            has_more: true,
            next_after: Some(event.cursor()),
        };

        let json = norito::json::to_json(&page).expect("serialize finalized moderation event page");
        let decoded: ModerationFinalizedEventPageV1 =
            norito::json::from_json(&json).expect("deserialize finalized moderation event page");
        let value = norito::json::to_value(&page).expect("serialize event page as JSON value");
        let norito::json::Value::Object(root) = value else {
            panic!("finalized moderation event page must be a JSON object");
        };
        let Some(norito::json::Value::Array(events)) = root.get("events") else {
            panic!("finalized moderation event page must contain an events array");
        };
        let Some(norito::json::Value::Object(committed_event)) = events.first() else {
            panic!("finalized moderation event must be a JSON object");
        };
        let Some(norito::json::Value::Object(ledger_event)) = committed_event.get("event") else {
            panic!("committed event must contain a typed ledger event");
        };

        assert_eq!(decoded, page);
        assert_eq!(
            ledger_event.get("kind"),
            Some(&norito::json!({
                "kind": "case_finalized",
                "detail": null,
            }))
        );
    }

    #[test]
    fn moderation_identifiers_reject_controls_unicode_whitespace_and_overflow() {
        for valid in ["case-1", "round_2", "policy.v1", "ipfs:bag/id@v1"] {
            assert!(is_canonical_moderation_identifier_v1(valid), "{valid}");
        }
        for invalid in [
            "",
            " leading",
            "trailing ",
            "embedded space",
            "line\nbreak",
            "nul\0byte",
            "confusable-é",
        ] {
            assert!(
                !is_canonical_moderation_identifier_v1(invalid),
                "{invalid:?}"
            );
        }
        assert!(!is_canonical_moderation_identifier_v1(
            &"a".repeat(MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1 + 1)
        ));
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
        candidate = policy();
        candidate.max_candidate_pool_size = candidate.max_panel_size - 1;
        assert!(matches!(
            candidate.validate(),
            Err(ModerationLedgerPolicyError::InvalidCandidatePoolSize { .. })
        ));
        candidate = policy();
        candidate.max_exclusions_per_case = 0;
        assert!(matches!(
            candidate.validate(),
            Err(ModerationLedgerPolicyError::InvalidExclusionLimit { found: 0 })
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

        candidate = case_spec();
        candidate.round_id = "round-1\nforged".to_owned();
        assert!(matches!(
            candidate.validate(),
            Err(ModerationCaseSpecError::InvalidIdentifier {
                field: "round_id",
                ..
            })
        ));

        candidate = case_spec();
        candidate.context.evidence_uri = Some("ipfs://evidence\tforged".to_owned());
        assert_eq!(
            candidate.validate(),
            Err(ModerationCaseSpecError::InvalidEvidenceUri)
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

    #[test]
    fn appeal_intake_rejects_inert_material_exclusion_bias_and_bad_windows() {
        let intake = appeal_intake();
        intake.validate().unwrap();
        assert_eq!(intake.digest().unwrap(), intake.digest().unwrap());

        let mut invalid = intake.clone();
        invalid.proof_token_digest = [0; 32];
        assert!(matches!(
            invalid.validate(),
            Err(ModerationAppealIntakeError::ZeroDigest {
                field: "proof_token_digest"
            })
        ));

        invalid = intake.clone();
        invalid.case_id = "appeal-1\nforged".to_owned();
        assert!(matches!(
            invalid.validate(),
            Err(ModerationAppealIntakeError::InvalidIdentifier {
                field: "case_id",
                ..
            })
        ));

        invalid = intake.clone();
        invalid.policy_reference = "policy-１".to_owned();
        assert!(matches!(
            invalid.validate(),
            Err(ModerationAppealIntakeError::InvalidIdentifier {
                field: "policy_reference",
                ..
            })
        ));

        invalid = intake.clone();
        invalid.evidence_uri = Some("ipfs://evidence\tforged".to_owned());
        assert_eq!(
            invalid.validate(),
            Err(ModerationAppealIntakeError::InvalidEvidenceUri)
        );

        invalid = intake.clone();
        invalid.exclusions.push(invalid.appellant.clone());
        assert!(matches!(
            invalid.validate(),
            Err(ModerationAppealIntakeError::InvalidExclusions(_))
        ));

        invalid = intake.clone();
        invalid.exclusions.clear();
        invalid.exclusions.push(account(8));
        assert_eq!(
            invalid.validate(),
            Err(ModerationAppealIntakeError::AppellantNotExcluded)
        );

        invalid = intake;
        invalid.acceptance_deadline_unix_ms = invalid.registration_deadline_unix_ms;
        assert_eq!(
            invalid.validate(),
            Err(ModerationAppealIntakeError::InvalidDeadlines)
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one fail-closed sortition scenario checks ordering, uniqueness, and every rejection class"
    )]
    fn sortition_is_order_independent_unique_and_fail_closed() {
        let intake = appeal_intake();
        let snapshot_digest = [0xA5; 32];
        let candidates = (1..=6)
            .map(|seed| eligibility(seed, snapshot_digest))
            .collect::<Vec<_>>();
        let seed_digest = sorafs_moderation_sortition_seed_v1(
            intake.digest().unwrap(),
            snapshot_digest,
            [0xB1; 32],
        );
        let mut grind_attempt = candidates[0].clone();
        grind_attempt.juror = account(42);
        grind_attempt.proof_digest = [0xEF; 32];
        assert_eq!(
            sortition_score(seed_digest, &candidates[0]),
            sortition_score(seed_digest, &grind_attempt),
            "account choice and randomized proof bytes must not change rank"
        );
        let first = sorafs_moderation_select_panel_v1(
            intake.digest().unwrap(),
            snapshot_digest,
            [0xB1; 32],
            &candidates,
            intake.panel_size,
            intake.waitlist_size,
            intake.quorum,
        )
        .unwrap();
        let mut reversed = candidates.clone();
        reversed.reverse();
        let second = sorafs_moderation_select_panel_v1(
            intake.digest().unwrap(),
            snapshot_digest,
            [0xB1; 32],
            &reversed,
            intake.panel_size,
            intake.waitlist_size,
            intake.quorum,
        )
        .unwrap();
        assert_eq!(first, second);
        let later_anchor = sorafs_moderation_select_panel_v1(
            intake.digest().unwrap(),
            snapshot_digest,
            [0xB2; 32],
            &candidates,
            intake.panel_size,
            intake.waitlist_size,
            intake.quorum,
        )
        .unwrap();
        assert_ne!(
            first.2, later_anchor.2,
            "post-registration parent anchors must produce distinct draw seeds"
        );
        assert_ne!(
            first.3, later_anchor.3,
            "sortition commitments must bind the frozen parent anchor through the seed"
        );
        assert_eq!(first.0.len(), 3);
        assert_eq!(first.1.len(), 2);
        let unique = first
            .0
            .iter()
            .chain(first.1.iter())
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), 5);

        let mut duplicate_account = candidates.clone();
        duplicate_account[1].juror = duplicate_account[0].juror.clone();
        assert!(matches!(
            sorafs_moderation_select_panel_v1(
                intake.digest().unwrap(),
                snapshot_digest,
                [0xB1; 32],
                &duplicate_account,
                3,
                2,
                2,
            ),
            Err(ModerationSortitionError::DuplicateJuror { .. })
        ));

        let mut duplicate_person = candidates.clone();
        duplicate_person[1].nullifier = duplicate_person[0].nullifier;
        assert_eq!(
            sorafs_moderation_select_panel_v1(
                intake.digest().unwrap(),
                snapshot_digest,
                [0xB1; 32],
                &duplicate_person,
                3,
                2,
                2,
            ),
            Err(ModerationSortitionError::DuplicatePersonNullifier)
        );

        let mut observer = candidates.clone();
        observer[0].eligibility_class = ModerationJurorEligibilityClassV1::Observer;
        assert!(matches!(
            sorafs_moderation_select_panel_v1(
                intake.digest().unwrap(),
                snapshot_digest,
                [0xB1; 32],
                &observer,
                3,
                2,
                2,
            ),
            Err(ModerationSortitionError::InvalidCandidate { .. })
        ));

        let mut wrong_snapshot = candidates.clone();
        wrong_snapshot[0].pop_snapshot_digest = [0xCC; 32];
        assert!(matches!(
            sorafs_moderation_select_panel_v1(
                intake.digest().unwrap(),
                snapshot_digest,
                [0xB1; 32],
                &wrong_snapshot,
                3,
                2,
                2,
            ),
            Err(ModerationSortitionError::InvalidCandidate { .. })
        ));

        let mut mixed_scope = candidates.clone();
        mixed_scope[0].round_id = "round-2".to_owned();
        assert!(matches!(
            sorafs_moderation_select_panel_v1(
                intake.digest().unwrap(),
                snapshot_digest,
                [0xB1; 32],
                &mixed_scope,
                3,
                2,
                2,
            ),
            Err(ModerationSortitionError::InvalidCandidate { .. })
        ));

        assert_eq!(
            sorafs_moderation_select_panel_v1(
                intake.digest().unwrap(),
                snapshot_digest,
                [0xB1; 32],
                &candidates,
                0,
                2,
                0,
            ),
            Err(ModerationSortitionError::InvalidBounds)
        );

        assert!(matches!(
            sorafs_moderation_select_panel_v1(
                intake.digest().unwrap(),
                snapshot_digest,
                [0xB1; 32],
                &candidates[..2],
                3,
                2,
                2,
            ),
            Err(ModerationSortitionError::InsufficientEligiblePool {
                required: 3,
                found: 2
            })
        ));
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one repair-ledger vector binds deterministic identity, canonical roundtrip, cursor, and mutation invariants"
    )]
    fn repair_identity_and_ledger_roundtrip_are_stable() {
        let source_identity = [0xA5; 32];
        let task_id = sorafs_repair_task_id_v1(source_identity);
        assert_eq!(task_id, sorafs_repair_task_id_v1(source_identity));
        assert_ne!(task_id, sorafs_repair_task_id_v1([0xA6; 32]));
        assert_ne!(
            sorafs_repair_idempotency_digest_v1("REP-1", "claim-1"),
            sorafs_repair_idempotency_digest_v1("REP-1", "claim-2")
        );

        let owner = account(21);
        let action_digest =
            sorafs_repair_action_digest_v1(&owner, &"claim-1").expect("encode repair action");
        assert_eq!(
            action_digest,
            sorafs_repair_action_digest_v1(&owner, &"claim-1")
                .expect("repeat repair action digest")
        );
        assert_ne!(
            action_digest,
            sorafs_repair_action_digest_v1(&owner, &"claim-2")
                .expect("changed repair action digest")
        );
        assert_ne!(
            action_digest,
            sorafs_repair_action_digest_v1(&account(22), &"claim-1")
                .expect("changed repair authority digest")
        );
        let task = RepairLedgerTaskV1 {
            version: REPAIR_LEDGER_TASK_VERSION_V1,
            task_id,
            source_identity,
            ticket_id: "REP-1".to_owned(),
            canonical_report: norito::encode_canonical(&"canonical-report").unwrap(),
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            submitted_by: owner.clone(),
            submitted_at_unix_ms: 1_000,
            revision: 2,
            lease: Some(RepairLedgerLeaseV1 {
                owner,
                generation: 1,
                acquired_at_unix_ms: 1_000,
                renewed_at_unix_ms: 1_000,
                expires_at_unix_ms: 2_000,
            }),
            terminal_outcome: None,
            slash: None,
            appeal: None,
            action_receipts: vec![RepairLedgerActionReceiptV1 {
                idempotency_digest: sorafs_repair_idempotency_digest_v1("REP-1", "claim-1"),
                action_digest: [0x33; 32],
                resulting_revision: 2,
            }],
            updated_at_unix_ms: 1_000,
        };
        let encoded = norito::encode_canonical(&task).unwrap();
        let decoded: RepairLedgerTaskV1 = norito::decode_canonical(&encoded).unwrap();
        assert_eq!(decoded, task);

        let finalized_cursor = RepairFinalizedCursorV1 {
            height: 7,
            block_hash: [0x71; 32],
        };
        let event = RepairFinalizedEventV1 {
            sequence: 2,
            block_height: 7,
            block_hash: finalized_cursor.block_hash,
            event_index: 1,
            event: SorafsRepairLedgerEvent {
                kind: crate::events::data::sorafs::SorafsRepairLedgerEventKind::LeaseClaimed,
                ticket_id: task.ticket_id.clone(),
                task_id,
                provider_id: crate::sorafs::capacity::ProviderId::new(task.provider_id),
                manifest_digest: crate::sorafs::pin_registry::ManifestDigest::new(
                    task.manifest_digest,
                ),
                revision: task.revision,
                authority: task.submitted_by.clone(),
                occurred_at_unix_ms: task.updated_at_unix_ms,
            },
        };
        let event_cursor = event.cursor();
        let event_page = RepairFinalizedEventPageV1 {
            finalized_cursor,
            events: vec![event],
            has_more: true,
            next_after: Some(event_cursor),
        };
        let task_page = RepairLedgerTaskPageV1 {
            finalized_cursor,
            tasks: vec![task.clone()],
            has_more: false,
            next_after_task_id: None,
        };
        let finalized_task = RepairFinalizedTaskV1 {
            finalized_cursor,
            task: task.clone(),
        };
        let finalized_status = RepairFinalizedStatusV1 {
            finalized_cursor,
            status: RepairLedgerStatusV1 {
                tasks: 1,
                leased_tasks: 1,
                terminal_outcomes: 0,
                completed: 0,
                failed: 0,
                escalated: 0,
                slash_proposals: 0,
                appeals: 0,
                updated_at_unix_ms: task.updated_at_unix_ms,
            },
        };
        assert_canonical_norito_round_trip(&finalized_cursor);
        assert_canonical_norito_round_trip(&finalized_task);
        assert_canonical_norito_round_trip(&finalized_status);
        assert_canonical_norito_round_trip(&task_page);
        assert_canonical_norito_round_trip(&event_cursor);
        assert_canonical_norito_round_trip(&event_page);

        #[cfg(feature = "json")]
        {
            let encoded =
                norito::json::to_vec(&event_page).expect("encode finalized repair event page JSON");
            let decoded: RepairFinalizedEventPageV1 = norito::json::from_slice(&encoded)
                .expect("decode finalized repair event page JSON");
            assert_eq!(decoded, event_page);

            let encoded =
                norito::json::to_vec(&task_page).expect("encode finalized repair task page JSON");
            let decoded: RepairLedgerTaskPageV1 =
                norito::json::from_slice(&encoded).expect("decode finalized repair task page JSON");
            assert_eq!(decoded, task_page);
        }

        let appeal_a =
            sorafs_repair_appeal_id_v1(task_id, [0x44; 32], &task.submitted_by, [0x55; 32], "why");
        let appeal_b =
            sorafs_repair_appeal_id_v1(task_id, [0x44; 32], &task.submitted_by, [0x55; 32], "why");
        assert_eq!(appeal_a, appeal_b);
    }
}
