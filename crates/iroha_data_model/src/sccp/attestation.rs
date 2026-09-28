//! SCCP v1 block commitments, history, attestation subjects, statements and signatures
//! (`specs/sccp.md` §3.5, §3.6.1, §4.5, §4.6, §4.8).
//!
//! Every SCCP-bearing block, epoch boundary and rotation height gets an attestation subject;
//! the statement bridge keys sign is the subject plus the block hash. The EIP-712 digest of a
//! statement is contract-visible and is computed by `iroha_sccp::v1` from these fields.

use super::params::SCCP_MESSAGES_MAX_PER_BLOCK_V1;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Byte length of one `r ‖ s ‖ v` bridge-key signature (§3.8).
pub const SCCP_SIGNATURE_BYTES_V1: usize = 65;
/// Largest history size a verifier accepts (`history_size ≤ 2^32`, §3.5).
pub const SCCP_HISTORY_MAX_SIZE_V1: u64 = 1 << 32;

/// Signed attestation statement: the ten fields of §3.6.1.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::attestation::SccpAttestationStatementV1")]
pub struct SccpAttestationStatementV1 {
    /// Taira block height `h`.
    pub height: u64,
    /// `NPoS` epoch of `h`.
    pub epoch: u64,
    /// `creation_time_ms` of block `h`'s header.
    pub timestamp_ms: u64,
    /// `HashOf<BlockHeader>` of block `h`.
    pub block_hash: [u8; 32],
    /// Root of block `h`'s commitment tree, or zero if `message_count = 0`.
    pub sccp_root: [u8; 32],
    /// SCCP messages (transfer and control leaves) committed by `h`, `0..=512`.
    pub message_count: u32,
    /// `history_root(history_size)`.
    pub history_root: [u8; 32],
    /// SCCP-bearing blocks with height `≤ h`.
    pub history_size: u64,
    /// §3.7 digest of the generation that signs `h`.
    pub roster_digest: [u8; 32],
    /// Digest of the successor generation if `h` is a rotation boundary, else zero.
    pub next_roster_digest: [u8; 32],
}

/// A statement breaks a §3.6.1 invariant every verifier checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SccpStatementInvariantError {
    /// `message_count = 0` and `sccp_root ≠ 0`, or the reverse.
    #[error("message_count is zero exactly when sccp_root is zero")]
    RootCountMismatch,
    /// `history_size = 0` and `history_root ≠ 0`, or the reverse.
    #[error("history_size is zero exactly when history_root is zero")]
    HistoryRootSizeMismatch,
    /// `message_count > 512`.
    #[error("message_count {count} exceeds 512")]
    TooManyMessages {
        /// Stated message count.
        count: u32,
    },
}

impl SccpAttestationStatementV1 {
    /// Check the §3.6.1 verifier invariants.
    ///
    /// # Errors
    ///
    /// Returns the first violated invariant.
    pub fn check_invariants(&self) -> Result<(), SccpStatementInvariantError> {
        if (self.message_count == 0) != (self.sccp_root == [0; 32]) {
            return Err(SccpStatementInvariantError::RootCountMismatch);
        }
        if (self.history_size == 0) != (self.history_root == [0; 32]) {
            return Err(SccpStatementInvariantError::HistoryRootSizeMismatch);
        }
        if self.message_count > SCCP_MESSAGES_MAX_PER_BLOCK_V1 {
            return Err(SccpStatementInvariantError::TooManyMessages {
                count: self.message_count,
            });
        }
        Ok(())
    }
}

/// Stored attestation subject (`sccp_attestation_subjects[height]`, §4.6).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::attestation::SccpAttestationSubjectV1")]
pub struct SccpAttestationSubjectV1 {
    /// Taira block height.
    pub height: u64,
    /// `NPoS` epoch of the height.
    pub epoch: u64,
    /// Block `creation_time_ms`.
    pub timestamp_ms: u64,
    /// Commitment-tree root, or zero.
    pub sccp_root: [u8; 32],
    /// Committed SCCP messages.
    pub message_count: u32,
    /// History root after this block.
    pub history_root: [u8; 32],
    /// History size after this block.
    pub history_size: u64,
    /// Generation that signs the height.
    pub generation: u64,
    /// Digest of that generation.
    pub roster_digest: [u8; 32],
    /// Digest of the successor generation at a rotation height, else zero.
    pub next_roster_digest: [u8; 32],
}

impl SccpAttestationSubjectV1 {
    /// Return the statement of this subject for the committed `block_hash`.
    #[must_use]
    pub const fn statement(&self, block_hash: [u8; 32]) -> SccpAttestationStatementV1 {
        SccpAttestationStatementV1 {
            height: self.height,
            epoch: self.epoch,
            timestamp_ms: self.timestamp_ms,
            block_hash,
            sccp_root: self.sccp_root,
            message_count: self.message_count,
            history_root: self.history_root,
            history_size: self.history_size,
            roster_digest: self.roster_digest,
            next_roster_digest: self.next_roster_digest,
        }
    }

    /// Return whether the subject is a rotation (it hands off to a successor generation).
    #[must_use]
    pub fn is_rotation(&self) -> bool {
        self.next_roster_digest != [0; 32]
    }
}

/// Signature progress of one subject (`sccp_attestation_status[height]`, §4.6).
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::attestation::SccpAttestationStatusV1")]
pub struct SccpAttestationStatusV1 {
    /// Bit `i` is set once member `i`'s signature is stored.
    pub signer_bitmap: u32,
    /// Height at which the bitmap first reached the threshold.
    #[norito(required)]
    pub attested_at_height: Option<u64>,
}

impl SccpAttestationStatusV1 {
    /// Return the number of stored signatures.
    #[must_use]
    pub const fn signer_count(&self) -> u32 {
        self.signer_bitmap.count_ones()
    }

    /// Return whether member `index`'s signature is stored (always false for `index ≥ 32`).
    #[must_use]
    pub const fn has_signer(&self, index: u8) -> bool {
        index < 32 && self.signer_bitmap & (1 << index) != 0
    }

    /// Set member `index`'s bit; returns whether it was newly set (false for `index ≥ 32`).
    pub fn record_signer(&mut self, index: u8) -> bool {
        if index >= 32 || self.has_signer(index) {
            return false;
        }
        self.signer_bitmap |= 1 << index;
        true
    }

    /// Return whether the subject reached its threshold.
    #[must_use]
    pub const fn is_attested(&self) -> bool {
        self.attested_at_height.is_some()
    }
}

/// One entry of `SubmitSccpAttestationsV1` (§4.8).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::attestation::SccpAttestationSignatureV1")]
pub struct SccpAttestationSignatureV1 {
    /// Attested height.
    pub height: u64,
    /// Roster slot of the signer.
    pub signer_index: u8,
    /// `r ‖ s ‖ v` over the statement digest (§3.8).
    pub signature: [u8; 65],
}

impl SccpAttestationSignatureV1 {
    /// Return the `(height, signer_index)` key that orders and deduplicates entries.
    #[must_use]
    pub const fn key(&self) -> (u64, u8) {
        (self.height, self.signer_index)
    }
}

/// Commitment of one SCCP-bearing block (`sccp_block_commitments[height]`, §4.5).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::attestation::SccpBlockCommitmentV1")]
pub struct SccpBlockCommitmentV1 {
    /// §3.4 promote-odd root over the block's leaves.
    pub root: [u8; 32],
    /// Number of leaves, `1..=512`.
    pub message_count: u32,
    /// 0-based index of the block's history leaf (§3.5).
    pub history_index: u64,
}

/// History accumulator state: size and perfect-subtree peaks, largest first (§3.5).
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::attestation::SccpHistoryStateV1")]
pub struct SccpHistoryStateV1 {
    /// Number of history leaves.
    pub size: u64,
    /// Roots of the perfect subtrees of the binary decomposition of `size`, largest first.
    pub peaks: Vec<[u8; 32]>,
}

impl SccpHistoryStateV1 {
    /// Return whether the peak count matches `size` and `size ≤ 2^32`.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.size <= SCCP_HISTORY_MAX_SIZE_V1
            && u32::try_from(self.peaks.len()).is_ok_and(|len| len == self.size.count_ones())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sccp::test_support::{assert_rejects_unknown_field, roundtrip};

    fn subject() -> SccpAttestationSubjectV1 {
        SccpAttestationSubjectV1 {
            height: 100,
            epoch: 2,
            timestamp_ms: 1_758_000_000_000,
            sccp_root: [1; 32],
            message_count: 3,
            history_root: [2; 32],
            history_size: 7,
            generation: 4,
            roster_digest: [3; 32],
            next_roster_digest: [0; 32],
        }
    }

    #[test]
    fn constants_match_the_spec() {
        assert_eq!(SCCP_SIGNATURE_BYTES_V1, 65);
        assert_eq!(SCCP_HISTORY_MAX_SIZE_V1, 4_294_967_296);
    }

    #[test]
    fn binary_and_json_roundtrip() {
        let subject = subject();
        roundtrip(&subject);
        roundtrip(&subject.statement([9; 32]));
        roundtrip(&SccpAttestationStatusV1::default());
        roundtrip(&SccpAttestationStatusV1 {
            signer_bitmap: u32::MAX,
            attested_at_height: Some(u64::MAX),
        });
        roundtrip(&SccpAttestationSignatureV1 {
            height: 5,
            signer_index: 30,
            signature: [0x1b; 65],
        });
        roundtrip(&SccpBlockCommitmentV1 {
            root: [4; 32],
            message_count: 512,
            history_index: 9,
        });
        roundtrip(&SccpHistoryStateV1::default());
        roundtrip(&SccpHistoryStateV1 {
            size: 3,
            peaks: vec![[5; 32], [6; 32]],
        });
        assert_rejects_unknown_field(&subject, &[]);
        assert_rejects_unknown_field(&subject.statement([9; 32]), &[]);
        assert_rejects_unknown_field(&SccpAttestationStatusV1::default(), &[]);
    }

    #[test]
    fn statement_copies_the_subject_and_binds_the_block_hash() {
        let subject = subject();
        let statement = subject.statement([0xee; 32]);
        assert_eq!(statement.height, subject.height);
        assert_eq!(statement.epoch, subject.epoch);
        assert_eq!(statement.timestamp_ms, subject.timestamp_ms);
        assert_eq!(statement.block_hash, [0xee; 32]);
        assert_eq!(statement.sccp_root, subject.sccp_root);
        assert_eq!(statement.message_count, subject.message_count);
        assert_eq!(statement.history_root, subject.history_root);
        assert_eq!(statement.history_size, subject.history_size);
        assert_eq!(statement.roster_digest, subject.roster_digest);
        assert_eq!(statement.next_roster_digest, subject.next_roster_digest);
        assert!(!subject.is_rotation());
        let rotation = SccpAttestationSubjectV1 {
            next_roster_digest: [8; 32],
            ..subject
        };
        assert!(rotation.is_rotation());
    }

    #[test]
    fn statement_invariants() {
        let valid = subject().statement([1; 32]);
        assert_eq!(valid.check_invariants(), Ok(()));
        let empty = SccpAttestationStatementV1 {
            sccp_root: [0; 32],
            message_count: 0,
            history_root: [0; 32],
            history_size: 0,
            ..valid
        };
        assert_eq!(empty.check_invariants(), Ok(()));
        for broken in [
            SccpAttestationStatementV1 {
                sccp_root: [0; 32],
                ..valid
            },
            SccpAttestationStatementV1 {
                message_count: 0,
                ..valid
            },
        ] {
            assert_eq!(
                broken.check_invariants(),
                Err(SccpStatementInvariantError::RootCountMismatch)
            );
        }
        for broken in [
            SccpAttestationStatementV1 {
                history_root: [0; 32],
                ..valid
            },
            SccpAttestationStatementV1 {
                history_size: 0,
                ..valid
            },
        ] {
            assert_eq!(
                broken.check_invariants(),
                Err(SccpStatementInvariantError::HistoryRootSizeMismatch)
            );
        }
        let at_limit = SccpAttestationStatementV1 {
            message_count: 512,
            ..valid
        };
        assert_eq!(at_limit.check_invariants(), Ok(()));
        assert_eq!(
            SccpAttestationStatementV1 {
                message_count: 513,
                ..valid
            }
            .check_invariants(),
            Err(SccpStatementInvariantError::TooManyMessages { count: 513 })
        );
    }

    #[test]
    fn status_bitmap_records_each_signer_once() {
        let mut status = SccpAttestationStatusV1::default();
        assert_eq!(status.signer_count(), 0);
        assert!(!status.is_attested());
        assert!(status.record_signer(0));
        assert!(status.record_signer(30));
        assert!(status.record_signer(31));
        assert!(!status.record_signer(30), "second signature is skipped");
        assert!(!status.record_signer(32), "bits beyond 31 do not exist");
        assert!(!status.has_signer(32));
        assert!(status.has_signer(0));
        assert!(!status.has_signer(1));
        assert_eq!(status.signer_count(), 3);
        assert_eq!(status.signer_bitmap, 0xc000_0001);
        status.attested_at_height = Some(9);
        assert!(status.is_attested());
    }

    #[test]
    fn signature_key_orders_by_height_then_index() {
        let entry = |height, signer_index| SccpAttestationSignatureV1 {
            height,
            signer_index,
            signature: [0; 65],
        };
        assert_eq!(entry(3, 7).key(), (3, 7));
        assert!(entry(3, 7).key() < entry(3, 8).key());
        assert!(entry(3, 30).key() < entry(4, 0).key());
    }

    #[test]
    fn history_peaks_follow_the_binary_decomposition() {
        assert!(SccpHistoryStateV1::default().is_well_formed());
        let state = |size, peaks: usize| SccpHistoryStateV1 {
            size,
            peaks: vec![[1; 32]; peaks],
        };
        assert!(state(1, 1).is_well_formed());
        assert!(state(6, 2).is_well_formed());
        assert!(state(7, 3).is_well_formed());
        assert!(!state(7, 2).is_well_formed());
        assert!(!state(0, 1).is_well_formed());
        assert!(state(SCCP_HISTORY_MAX_SIZE_V1, 1).is_well_formed());
        assert!(!state(SCCP_HISTORY_MAX_SIZE_V1 + 1, 2).is_well_formed());
    }
}
