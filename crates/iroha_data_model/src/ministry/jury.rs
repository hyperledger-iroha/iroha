//! Policy jury sortition and ballot data structures.
//!
//! This module implements the data-level deliverables for roadmap item
//! **MINFO-5 — Policy jury voting toolkit**.  It provides deterministic
//! sortition manifests plus sealed commit / reveal ballots so governance
//! clients can prove juror selection and ballot integrity before admitting
//! policy votes to the ledger.

use std::collections::BTreeSet;

use blake2::digest::Digest;
use iroha_crypto::Blake2b256;
use iroha_schema::IntoSchema;
use thiserror::Error;

use crate::{Decode, Encode};
/// Schema version tag for [`PolicyJuryBallotCommitV1`].
pub const POLICY_JURY_BALLOT_COMMIT_VERSION_V1: u16 = 1;
/// Schema version tag for [`PolicyJuryBallotRevealV1`].
pub const POLICY_JURY_BALLOT_REVEAL_VERSION_V1: u16 = 1;
/// Schema version tag for [`PolicyJurySortitionV1`].
pub const POLICY_JURY_SORTITION_VERSION_V1: u16 = 1;

/// Vote options available to policy juries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "choice", content = "value", rename_all = "kebab-case")
)]
pub enum PolicyJuryVoteChoice {
    /// Vote in favour of the requested policy action.
    Approve,
    /// Vote against the requested policy action.
    Reject,
    /// Abstain or return the proposal for more work.
    Abstain,
}

impl PolicyJuryVoteChoice {
    fn discriminant(self) -> u8 {
        match self {
            Self::Approve => 1,
            Self::Reject => 2,
            Self::Abstain => 3,
        }
    }
}

/// Ballot channel used for a juror's vote.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(not(feature = "zk-ballot"), derive(Copy))]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "mode", content = "value", rename_all = "kebab-case")]
pub enum PolicyJuryBallotMode {
    /// Standard plaintext commit → reveal workflow.
    Plaintext,
    /// Commitments reference a ZK proof artifact.
    #[cfg(feature = "zk-ballot")]
    ZkEnvelope {
        /// URI referencing the proof bundle backing the reveal.
        proof_uri: String,
        /// Optional attached artefacts (e.g., Sigma transcripts).
        #[norito(default)]
        attachments: Vec<String>,
    },
}

/// Juror ballot commitment produced during the sealed phase.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PolicyJuryBallotCommitV1 {
    /// Schema version; must equal [`POLICY_JURY_BALLOT_COMMIT_VERSION_V1`].
    pub version: u16,
    /// Proposal identifier (`AC-YYYY-###`) for the vote.
    pub proposal_id: String,
    /// Policy jury round identifier (`PJ-YYYY-##`).
    pub round_id: String,
    /// Stable juror identifier.
    pub juror_id: String,
    /// Blake2b commitment for the juror + choice + nonce tuple.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub commitment_blake2b_256: [u8; 32],
    /// UTC timestamp (milliseconds) when the commitment was recorded.
    pub committed_at_unix_ms: u64,
    /// Delivery mode (plaintext vs zk-envelope).
    pub mode: PolicyJuryBallotMode,
}

impl PolicyJuryBallotCommitV1 {
    /// Validate structural invariants and ensure the reveal matches this commitment.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyJuryBallotError`] if the supplied reveal has mismatched
    /// identifiers, invalid nonces, or fails the commitment check.
    pub fn verify_reveal(
        &self,
        reveal: &PolicyJuryBallotRevealV1,
    ) -> Result<(), PolicyJuryBallotError> {
        self.validate()?;
        reveal.validate()?;
        if self.proposal_id != reveal.proposal_id {
            return Err(PolicyJuryBallotError::ProposalMismatch {
                commit: self.proposal_id.clone(),
                reveal: reveal.proposal_id.clone(),
            });
        }
        if self.round_id != reveal.round_id {
            return Err(PolicyJuryBallotError::RoundMismatch {
                commit: self.round_id.clone(),
                reveal: reveal.round_id.clone(),
            });
        }
        if self.juror_id != reveal.juror_id {
            return Err(PolicyJuryBallotError::JurorMismatch {
                commit: self.juror_id.clone(),
                reveal: reveal.juror_id.clone(),
            });
        }
        #[cfg(feature = "zk-ballot")]
        {
            let expects_zk = matches!(self.mode, PolicyJuryBallotMode::ZkEnvelope { .. });
            if expects_zk && reveal.zk_proof_uris.is_empty() {
                return Err(PolicyJuryBallotError::MissingZkEvidence);
            }
            if !expects_zk && !reveal.zk_proof_uris.is_empty() {
                return Err(PolicyJuryBallotError::UnexpectedZkEvidence);
            }
        }
        let expected = reveal.compute_commitment();
        if expected != self.commitment_blake2b_256 {
            return Err(PolicyJuryBallotError::CommitmentMismatch);
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), PolicyJuryBallotError> {
        if self.version != POLICY_JURY_BALLOT_COMMIT_VERSION_V1 {
            return Err(PolicyJuryBallotError::UnsupportedCommitVersion {
                expected: POLICY_JURY_BALLOT_COMMIT_VERSION_V1,
                found: self.version,
            });
        }
        if self.juror_id.trim().is_empty() {
            return Err(PolicyJuryBallotError::MissingJurorId);
        }
        if self.proposal_id.trim().is_empty() {
            return Err(PolicyJuryBallotError::MissingProposalId);
        }
        if self.round_id.trim().is_empty() {
            return Err(PolicyJuryBallotError::MissingRoundId);
        }
        Ok(())
    }
}

/// Public reveal payload proving the juror's choice and salt.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PolicyJuryBallotRevealV1 {
    /// Schema version; must equal [`POLICY_JURY_BALLOT_REVEAL_VERSION_V1`].
    pub version: u16,
    /// Proposal identifier (`AC-YYYY-###`) for the vote.
    pub proposal_id: String,
    /// Policy jury round identifier (`PJ-YYYY-##`).
    pub round_id: String,
    /// Stable juror identifier.
    pub juror_id: String,
    /// Vote selected by the juror.
    pub choice: PolicyJuryVoteChoice,
    /// Random nonce used when generating the commitment.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub nonce: Vec<u8>,
    /// UTC timestamp (milliseconds) when the reveal was recorded.
    pub revealed_at_unix_ms: u64,
    /// Optional references to ZK proofs that justify the reveal.
    #[cfg(feature = "zk-ballot")]
    #[norito(default)]
    pub zk_proof_uris: Vec<String>,
}

impl PolicyJuryBallotRevealV1 {
    /// Compute the canonical commitment digest tied to this reveal.
    #[must_use]
    pub fn compute_commitment(&self) -> [u8; 32] {
        canonical_commitment(
            &self.round_id,
            &self.proposal_id,
            &self.juror_id,
            self.choice,
            &self.nonce,
        )
    }

    fn validate(&self) -> Result<(), PolicyJuryBallotError> {
        if self.version != POLICY_JURY_BALLOT_REVEAL_VERSION_V1 {
            return Err(PolicyJuryBallotError::UnsupportedRevealVersion {
                expected: POLICY_JURY_BALLOT_REVEAL_VERSION_V1,
                found: self.version,
            });
        }
        if self.nonce.len() < 16 {
            return Err(PolicyJuryBallotError::NonceTooShort {
                length: self.nonce.len(),
            });
        }
        if self.juror_id.trim().is_empty() {
            return Err(PolicyJuryBallotError::MissingJurorId);
        }
        if self.proposal_id.trim().is_empty() {
            return Err(PolicyJuryBallotError::MissingProposalId);
        }
        if self.round_id.trim().is_empty() {
            return Err(PolicyJuryBallotError::MissingRoundId);
        }
        Ok(())
    }
}

/// Errors surfaced while validating policy jury ballots.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyJuryBallotError {
    /// Commitment version mismatch.
    #[error("unsupported ballot commit version `{found}` (expected {expected})")]
    UnsupportedCommitVersion {
        /// Version required by the protocol.
        expected: u16,
        /// Version embedded in the reveal.
        found: u16,
    },
    /// Reveal version mismatch.
    #[error("unsupported ballot reveal version `{found}` (expected {expected})")]
    UnsupportedRevealVersion {
        /// Version required by the reveal format.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Reveal references a different proposal identifier.
    #[error("proposal mismatch: commit `{commit}`, reveal `{reveal}`")]
    ProposalMismatch {
        /// Proposal identifier stored in the commit.
        commit: String,
        /// Proposal identifier supplied by the reveal.
        reveal: String,
    },
    /// Reveal references a different round identifier.
    #[error("round mismatch: commit `{commit}`, reveal `{reveal}`")]
    RoundMismatch {
        /// Round identifier stored in the commit.
        commit: String,
        /// Round identifier supplied by the reveal.
        reveal: String,
    },
    /// Reveal references a different juror identifier.
    #[error("juror mismatch: commit `{commit}`, reveal `{reveal}`")]
    JurorMismatch {
        /// Juror id stored in the commit.
        commit: String,
        /// Juror id supplied by the reveal.
        reveal: String,
    },
    /// Reveal nonce too short for a secure commitment.
    #[error("ballot reveal nonce must be >=16 bytes (found {length})")]
    NonceTooShort {
        /// Nonce length observed in the reveal.
        length: usize,
    },
    /// Juror identifier missing or blank.
    #[error("juror identifier is required")]
    MissingJurorId,
    /// Proposal identifier missing or blank.
    #[error("proposal identifier is required")]
    MissingProposalId,
    /// Round identifier missing or blank.
    #[error("round identifier is required")]
    MissingRoundId,
    /// Revealed payload does not match the stored commitment.
    #[error("commitment mismatch for juror reveal")]
    CommitmentMismatch,
    /// ZK proof references missing even though the commit expects them.
    #[cfg(feature = "zk-ballot")]
    #[error("zk ballot requires proof references in the reveal")]
    MissingZkEvidence,
    /// Reveal unexpectedly contained ZK proof references.
    #[cfg(feature = "zk-ballot")]
    #[error("plain ballots must not reference zk proofs")]
    UnexpectedZkEvidence,
}

/// Deterministic sortition manifest for a policy jury round.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PolicyJurySortitionV1 {
    /// Schema version; must equal [`POLICY_JURY_SORTITION_VERSION_V1`].
    pub version: u16,
    /// Proposal identifier (`AC-YYYY-###`).
    pub proposal_id: String,
    /// Policy jury round identifier (`PJ-YYYY-##`).
    pub round_id: String,
    /// UTC timestamp (milliseconds) when the draw was executed.
    pub drawn_at_unix_ms: u64,
    /// Blake2b digest of the proof-of-personhood snapshot used for sortition.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub pop_snapshot_digest_blake2b_256: [u8; 32],
    /// Randomness beacon used for the draw.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub randomness_beacon: [u8; 32],
    /// Target committee size.
    pub committee_size: u32,
    /// Selected jurors (primary slots).
    #[norito(default)]
    pub assignments: Vec<PolicyJuryAssignment>,
    /// Waitlisted jurors used for automatic failover.
    #[norito(default)]
    pub waitlist: Vec<PolicyJuryWaitlistEntry>,
}

impl PolicyJurySortitionV1 {
    /// Validate structural invariants for the sortition manifest.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyJurySortitionError`] when the manifest violates any of
    /// the structural constraints (version mismatch, duplicate jurors, and so
    /// on).
    pub fn validate(&self) -> Result<(), PolicyJurySortitionError> {
        if self.version != POLICY_JURY_SORTITION_VERSION_V1 {
            return Err(PolicyJurySortitionError::UnsupportedVersion {
                expected: POLICY_JURY_SORTITION_VERSION_V1,
                found: self.version,
            });
        }
        let expected_size = usize::try_from(self.committee_size).unwrap_or(usize::MAX);
        if expected_size != self.assignments.len() {
            let found = u32::try_from(self.assignments.len()).unwrap_or(u32::MAX);
            return Err(PolicyJurySortitionError::CommitteeSizeMismatch {
                expected: self.committee_size,
                found,
            });
        }
        let mut slots = BTreeSet::new();
        let mut jurors = BTreeSet::new();
        for assignment in &self.assignments {
            if !slots.insert(assignment.slot) {
                return Err(PolicyJurySortitionError::DuplicateSlot {
                    slot: assignment.slot,
                });
            }
            if !jurors.insert(assignment.juror_id.clone()) {
                return Err(PolicyJurySortitionError::DuplicateJuror {
                    juror_id: assignment.juror_id.clone(),
                });
            }
            if assignment.juror_id.trim().is_empty() {
                return Err(PolicyJurySortitionError::MissingJurorId {
                    slot: assignment.slot,
                });
            }
            if let Some(plan) = assignment.failover.as_ref()
                && plan.waitlist_rank == 0
            {
                return Err(PolicyJurySortitionError::InvalidFailoverRank {
                    slot: assignment.slot,
                    rank: plan.waitlist_rank,
                });
            }
        }
        let mut expected_rank = 1u32;
        for entry in &self.waitlist {
            if entry.rank != expected_rank {
                return Err(PolicyJurySortitionError::WaitlistOutOfOrder {
                    expected: expected_rank,
                    found: entry.rank,
                });
            }
            if !jurors.insert(entry.juror_id.clone()) {
                return Err(PolicyJurySortitionError::DuplicateJuror {
                    juror_id: entry.juror_id.clone(),
                });
            }
            expected_rank += 1;
        }
        if let Some(plan) = self
            .assignments
            .iter()
            .filter_map(|assignment| assignment.failover.as_ref())
            .find(|plan| {
                usize::try_from(plan.waitlist_rank).map_or(true, |rank| rank > self.waitlist.len())
            })
        {
            return Err(PolicyJurySortitionError::InvalidFailoverRank {
                slot: self
                    .assignments
                    .iter()
                    .find(|assignment| {
                        assignment
                            .failover
                            .as_ref()
                            .is_some_and(|candidate| candidate.waitlist_rank == plan.waitlist_rank)
                    })
                    .map(|assignment| assignment.slot)
                    .unwrap_or_default(),
                rank: plan.waitlist_rank,
            });
        }
        Ok(())
    }
}

/// Sortition validation errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyJurySortitionError {
    /// Sortition schema version mismatch.
    #[error("unsupported sortition version `{found}` (expected {expected})")]
    UnsupportedVersion {
        /// Version expected by the verifier.
        expected: u16,
        /// Version observed in the manifest.
        found: u16,
    },
    /// Committee size and assignment count differ.
    #[error("committee size mismatch: expected {expected}, found {found}")]
    CommitteeSizeMismatch {
        /// Committee size declared in the header.
        expected: u32,
        /// Actual number of juror assignments.
        found: u32,
    },
    /// Duplicate slot identifier encountered.
    #[error("slot {slot} assigned to multiple jurors")]
    DuplicateSlot {
        /// Slot identifier present multiple times.
        slot: u32,
    },
    /// Duplicate juror identifier encountered.
    #[error("juror `{juror_id}` appears multiple times in the sortition manifest")]
    DuplicateJuror {
        /// Juror identifier present multiple times.
        juror_id: String,
    },
    /// Juror identifier missing.
    #[error("juror id missing for slot {slot}")]
    MissingJurorId {
        /// Slot lacking a juror id.
        slot: u32,
    },
    /// Waitlist entries not ordered by ascending rank.
    #[error("waitlist rank mismatch: expected {expected}, found {found}")]
    WaitlistOutOfOrder {
        /// Rank that should appear next.
        expected: u32,
        /// Rank encountered instead.
        found: u32,
    },
    /// Failover plan references an invalid waitlist rank.
    #[error("slot {slot} references invalid failover rank {rank}")]
    InvalidFailoverRank {
        /// Slot referencing an invalid rank.
        slot: u32,
        /// Waitlist rank referenced in the plan.
        rank: u32,
    },
}

/// Primary juror assignment emitted by the sortition workflow.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PolicyJuryAssignment {
    /// Slot number (`0..committee_size`).
    pub slot: u32,
    /// Juror identifier selected for this slot.
    pub juror_id: String,
    /// Proof-of-personhood record identifier or fingerprint.
    pub pop_identity: String,
    /// Grace period (seconds) granted before failover is triggered.
    pub grace_period_secs: u32,
    /// Optional automatic failover plan referencing the waitlist.
    #[norito(default)]
    pub failover: Option<PolicyJuryFailoverPlan>,
}

/// Automatic failover plan mapping a slot to the waitlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PolicyJuryFailoverPlan {
    /// Waitlist rank that should replace this slot if the juror no-shows.
    pub waitlist_rank: u32,
    /// Additional grace period before escalation is triggered.
    pub escalate_after_secs: u32,
}

/// Waitlisted juror entry.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PolicyJuryWaitlistEntry {
    /// Rank position (1-indexed, ascending).
    pub rank: u32,
    /// Juror identifier.
    pub juror_id: String,
    /// Proof-of-personhood fingerprint for the juror.
    pub pop_identity: String,
    /// UTC timestamp (milliseconds) when the waitlist entry expires.
    pub expires_at_unix_ms: u64,
}

fn canonical_commitment(
    round_id: &str,
    proposal_id: &str,
    juror_id: &str,
    choice: PolicyJuryVoteChoice,
    nonce: &[u8],
) -> [u8; 32] {
    let mut hasher = Blake2b256::new();
    hasher.update(round_id.as_bytes());
    hasher.update([0u8]);
    hasher.update(proposal_id.as_bytes());
    hasher.update([0u8]);
    hasher.update(juror_id.as_bytes());
    hasher.update([0u8]);
    hasher.update([choice.discriminant()]);
    hasher.update([0u8]);
    hasher.update(nonce);
    let digest = hasher.finalize();
    digest.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_reveal(choice: PolicyJuryVoteChoice) -> PolicyJuryBallotRevealV1 {
        PolicyJuryBallotRevealV1 {
            version: POLICY_JURY_BALLOT_REVEAL_VERSION_V1,
            proposal_id: "AC-2026-042".into(),
            round_id: "PJ-2026-02".into(),
            juror_id: "juror#1".into(),
            choice,
            nonce: vec![0x42; 32],
            revealed_at_unix_ms: 1_738_000_000_000,
            #[cfg(feature = "zk-ballot")]
            zk_proof_uris: Vec::new(),
        }
    }

    fn sample_commit(mode: PolicyJuryBallotMode) -> PolicyJuryBallotCommitV1 {
        let reveal = sample_reveal(PolicyJuryVoteChoice::Approve);
        PolicyJuryBallotCommitV1 {
            version: POLICY_JURY_BALLOT_COMMIT_VERSION_V1,
            proposal_id: reveal.proposal_id.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
            commitment_blake2b_256: reveal.compute_commitment(),
            committed_at_unix_ms: 1_738_000_000_000,
            mode,
        }
    }

    #[test]
    fn ballot_commit_reveal_roundtrip() {
        let commit = sample_commit(PolicyJuryBallotMode::Plaintext);
        let reveal = sample_reveal(PolicyJuryVoteChoice::Approve);
        commit.verify_reveal(&reveal).expect("roundtrip succeeds");
    }

    #[test]
    fn ballot_commit_rejects_mismatched_choice() {
        let commit = sample_commit(PolicyJuryBallotMode::Plaintext);
        let mut reveal = sample_reveal(PolicyJuryVoteChoice::Reject);
        reveal.nonce = vec![0x24; 32];
        let err = commit.verify_reveal(&reveal).expect_err("mismatch");
        assert!(matches!(err, PolicyJuryBallotError::CommitmentMismatch));
    }

    #[test]
    fn sortition_validation_enforces_uniqueness() {
        let manifest = PolicyJurySortitionV1 {
            version: POLICY_JURY_SORTITION_VERSION_V1,
            proposal_id: "AC-2026-042".into(),
            round_id: "PJ-2026-02".into(),
            drawn_at_unix_ms: 1_738_000_000_000,
            pop_snapshot_digest_blake2b_256: [1; 32],
            randomness_beacon: [2; 32],
            committee_size: 2,
            assignments: vec![
                PolicyJuryAssignment {
                    slot: 0,
                    juror_id: "juror#1".into(),
                    pop_identity: "pop#1".into(),
                    grace_period_secs: 900,
                    failover: Some(PolicyJuryFailoverPlan {
                        waitlist_rank: 1,
                        escalate_after_secs: 600,
                    }),
                },
                PolicyJuryAssignment {
                    slot: 1,
                    juror_id: "juror#2".into(),
                    pop_identity: "pop#2".into(),
                    grace_period_secs: 900,
                    failover: None,
                },
            ],
            waitlist: vec![PolicyJuryWaitlistEntry {
                rank: 1,
                juror_id: "juror#3".into(),
                pop_identity: "pop#3".into(),
                expires_at_unix_ms: 1_738_086_400_000,
            }],
        };
        manifest.validate().expect("valid manifest");
    }

    #[test]
    fn sortition_validation_rejects_duplicate_jurors() {
        let manifest = PolicyJurySortitionV1 {
            version: POLICY_JURY_SORTITION_VERSION_V1,
            proposal_id: "AC-2026-042".into(),
            round_id: "PJ-2026-02".into(),
            drawn_at_unix_ms: 1_738_000_000_000,
            pop_snapshot_digest_blake2b_256: [1; 32],
            randomness_beacon: [2; 32],
            committee_size: 1,
            assignments: vec![PolicyJuryAssignment {
                slot: 0,
                juror_id: "juror#1".into(),
                pop_identity: "pop#1".into(),
                grace_period_secs: 900,
                failover: None,
            }],
            waitlist: vec![PolicyJuryWaitlistEntry {
                rank: 1,
                juror_id: "juror#1".into(),
                pop_identity: "pop#1".into(),
                expires_at_unix_ms: 1_738_086_400_000,
            }],
        };
        let err = manifest.validate().expect_err("duplicate juror");
        assert!(matches!(
            err,
            PolicyJurySortitionError::DuplicateJuror { .. }
        ));
    }

    #[test]
    fn ballot_reveal_requires_minimum_nonce_length() {
        let commit = sample_commit(PolicyJuryBallotMode::Plaintext);
        let mut reveal = sample_reveal(PolicyJuryVoteChoice::Approve);
        reveal.nonce = vec![0u8; 4];
        let err = commit.verify_reveal(&reveal).expect_err("nonce too short");
        assert!(matches!(
            err,
            PolicyJuryBallotError::NonceTooShort { length: 4 }
        ));
    }

    #[test]
    fn ballot_commit_rejects_round_mismatch() {
        let commit = sample_commit(PolicyJuryBallotMode::Plaintext);
        let mut reveal = sample_reveal(PolicyJuryVoteChoice::Approve);
        reveal.round_id = "PJ-2026-03".into();
        let err = commit.verify_reveal(&reveal).expect_err("round mismatch");
        assert!(matches!(
            err,
            PolicyJuryBallotError::RoundMismatch { ref reveal, .. } if reveal == "PJ-2026-03"
        ));
    }

    #[test]
    fn sortition_validation_rejects_zero_rank_failover() {
        let manifest = PolicyJurySortitionV1 {
            version: POLICY_JURY_SORTITION_VERSION_V1,
            proposal_id: "AC-2026-042".into(),
            round_id: "PJ-2026-02".into(),
            drawn_at_unix_ms: 1_738_000_000_000,
            pop_snapshot_digest_blake2b_256: [1; 32],
            randomness_beacon: [2; 32],
            committee_size: 1,
            assignments: vec![PolicyJuryAssignment {
                slot: 0,
                juror_id: "juror#1".into(),
                pop_identity: "pop#1".into(),
                grace_period_secs: 900,
                failover: Some(PolicyJuryFailoverPlan {
                    waitlist_rank: 0,
                    escalate_after_secs: 600,
                }),
            }],
            waitlist: vec![],
        };
        let err = manifest
            .validate()
            .expect_err("rank zero failover rejected");
        assert!(matches!(
            err,
            PolicyJurySortitionError::InvalidFailoverRank { rank: 0, .. }
        ));
    }

    #[test]
    fn sortition_validation_rejects_waitlist_rank_gaps() {
        let manifest = PolicyJurySortitionV1 {
            version: POLICY_JURY_SORTITION_VERSION_V1,
            proposal_id: "AC-2026-042".into(),
            round_id: "PJ-2026-02".into(),
            drawn_at_unix_ms: 1_738_000_000_000,
            pop_snapshot_digest_blake2b_256: [1; 32],
            randomness_beacon: [2; 32],
            committee_size: 1,
            assignments: vec![PolicyJuryAssignment {
                slot: 0,
                juror_id: "juror#1".into(),
                pop_identity: "pop#1".into(),
                grace_period_secs: 900,
                failover: None,
            }],
            waitlist: vec![
                PolicyJuryWaitlistEntry {
                    rank: 1,
                    juror_id: "juror#2".into(),
                    pop_identity: "pop#2".into(),
                    expires_at_unix_ms: 1_738_086_400_000,
                },
                PolicyJuryWaitlistEntry {
                    rank: 3,
                    juror_id: "juror#3".into(),
                    pop_identity: "pop#3".into(),
                    expires_at_unix_ms: 1_738_086_400_000,
                },
            ],
        };
        let err = manifest
            .validate()
            .expect_err("gap in waitlist ranks rejected");
        assert!(matches!(
            err,
            PolicyJurySortitionError::WaitlistOutOfOrder {
                expected: 2,
                found: 3
            }
        ));
    }

    #[cfg(feature = "zk-ballot")]
    #[test]
    fn zk_ballot_commit_requires_proof_references() {
        let commit = sample_commit(PolicyJuryBallotMode::ZkEnvelope {
            proof_uri: "sorafs://proofs/pj-2026-02/juror-1".into(),
            attachments: Vec::new(),
        });
        let reveal = sample_reveal(PolicyJuryVoteChoice::Approve);
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("missing ZK evidence");
        assert!(matches!(err, PolicyJuryBallotError::MissingZkEvidence));
    }

    #[cfg(feature = "zk-ballot")]
    #[test]
    fn plain_ballot_rejects_unexpected_proof_references() {
        let commit = sample_commit(PolicyJuryBallotMode::Plaintext);
        let mut reveal = sample_reveal(PolicyJuryVoteChoice::Approve);
        reveal.zk_proof_uris = vec!["sorafs://proofs/pj-2026-02/juror-1".into()];
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("unexpected ZK evidence");
        assert!(matches!(err, PolicyJuryBallotError::UnexpectedZkEvidence));
    }
}
