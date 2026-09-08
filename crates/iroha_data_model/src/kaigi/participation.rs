//! Bounded participation sequences for the final Kaigi authorization relation.
//!
//! Each retained subject stores the complete original canonical account identity.
//! A live subject owns one commitment; leaving consumes its participation sequence.
//! Core authenticates rekey continuity and derives the six-lane proof identity;
//! the original account remains unchanged across successors and rejoining.
//! The call's separate append-only nullifier log continues to reject replay.
//!
//! The enclosing `KaigiRecord` carries this owner on every canonical wire.
//! Core validates it against the roster before authorizing state transitions.

use super::{KAIGI_MAX_PARTICIPANTS_V1, scalar::KaigiAuthorizationScalarV1};
use crate::account::AccountId;
use iroha_schema::IntoSchema;
use norito::{
    Decode, Encode,
    derive::{JsonDeserialize, JsonSerialize},
};
use std::{collections::BTreeSet, fmt};

/// One original account's current participation sequence in a permanent call.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(reuse_archived)]
pub struct KaigiPrivateParticipationV1 {
    original_account: AccountId,
    sequence: u64,
    active_commitment: Option<KaigiAuthorizationScalarV1>,
}

impl KaigiPrivateParticipationV1 {
    /// Original canonical account, retained unchanged after leave or account rekey.
    #[must_use]
    pub const fn original_account(&self) -> &AccountId {
        &self.original_account
    }

    /// Sequence used by the active commitment or the next permitted join.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Exact live commitment, absent after a successful leave.
    #[must_use]
    pub const fn active_commitment(&self) -> Option<KaigiAuthorizationScalarV1> {
        self.active_commitment
    }
}

/// Sorted, bounded subject ownership retained for one permanent Kaigi call.
///
/// Decoded state must pass [`Self::validate_against_roster`] before use. This
/// ledger supplies participation sequencing, not proof or signer authority.
/// Core must retain dependencies for every original account while the call is
/// active, including subjects whose current commitment is absent.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(reuse_archived)]
pub struct KaigiPrivateParticipationLedgerV1 {
    entries: Vec<KaigiPrivateParticipationV1>,
}

/// Failure to inspect or advance a private participation sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KaigiPrivateParticipationErrorV1 {
    /// Stored subjects, sequences or active commitments are malformed.
    InvalidState,
    /// Another live commitment already belongs to this subject.
    AlreadyActive,
    /// The subject has no live participation.
    NotActive,
    /// The supplied sequence differs from the exact ledger-owned sequence.
    SequenceMismatch,
    /// A supplied commitment differs from the subject's live commitment.
    CommitmentMismatch,
    /// The commitment is already owned by a different live subject.
    DuplicateCommitment,
    /// The retained subject bound has been reached.
    Capacity,
    /// Advancing the subject's participation sequence would overflow.
    SequenceExhausted,
}

impl fmt::Display for KaigiPrivateParticipationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidState => "invalid Kaigi private participation state",
            Self::AlreadyActive => "Kaigi private subject is already active",
            Self::NotActive => "Kaigi private subject is not active",
            Self::SequenceMismatch => {
                "Kaigi private participation sequence differs from ledger state"
            }
            Self::CommitmentMismatch => "Kaigi private commitment differs from ledger state",
            Self::DuplicateCommitment => "Kaigi private commitment is already active",
            Self::Capacity => "Kaigi retained private subject limit reached",
            Self::SequenceExhausted => "Kaigi private participation sequence exhausted",
        })
    }
}

impl std::error::Error for KaigiPrivateParticipationErrorV1 {}

impl KaigiPrivateParticipationLedgerV1 {
    /// Borrow the canonical subject order without exposing mutable state.
    #[must_use]
    pub fn entries(&self) -> &[KaigiPrivateParticipationV1] {
        &self.entries
    }

    fn validate(&self) -> Result<(), KaigiPrivateParticipationErrorV1> {
        if self.entries.len() > KAIGI_MAX_PARTICIPANTS_V1
            || self.entries.iter().any(|entry| {
                entry.sequence == 0
                    || (entry.sequence == u64::MAX && entry.active_commitment.is_some())
            })
            || self
                .entries
                .windows(2)
                .any(|pair| pair[0].original_account >= pair[1].original_account)
        {
            return Err(KaigiPrivateParticipationErrorV1::InvalidState);
        }
        let mut commitments = BTreeSet::new();
        if self
            .entries
            .iter()
            .filter_map(|entry| entry.active_commitment)
            .any(|commitment| !commitments.insert(commitment))
        {
            return Err(KaigiPrivateParticipationErrorV1::InvalidState);
        }
        Ok(())
    }

    /// Check that every live subject owns exactly one current roster entry.
    ///
    /// # Errors
    /// Rejects malformed state, duplicate roster entries, and either direction
    /// of an ownership/roster mismatch. The caller separately validates the
    /// canonical root of the ordered roster and the authenticated call owner.
    pub fn validate_against_roster(
        &self,
        roster: &[KaigiAuthorizationScalarV1],
    ) -> Result<(), KaigiPrivateParticipationErrorV1> {
        self.validate()?;
        let active: BTreeSet<_> = self
            .entries
            .iter()
            .filter_map(|entry| entry.active_commitment)
            .collect();
        let observed: BTreeSet<_> = roster.iter().copied().collect();
        if roster.len() != observed.len() || active != observed {
            return Err(KaigiPrivateParticipationErrorV1::InvalidState);
        }
        Ok(())
    }

    /// Inspect the only sequence that a new join may prove.
    ///
    /// # Errors
    /// Rejects malformed state, an already active subject, or a new subject
    /// beyond the retained bound. A never-seen participant starts at one;
    /// sequence zero is reserved for host authorization.
    pub fn prepare_join(
        &self,
        original_account: &AccountId,
    ) -> Result<u64, KaigiPrivateParticipationErrorV1> {
        self.validate()?;
        match self
            .entries
            .binary_search_by(|entry| entry.original_account.cmp(original_account))
        {
            Ok(index) => {
                let entry = &self.entries[index];
                if entry.active_commitment.is_some() {
                    return Err(KaigiPrivateParticipationErrorV1::AlreadyActive);
                }
                // A join must always reserve the ability to leave.
                if entry.sequence == u64::MAX {
                    return Err(KaigiPrivateParticipationErrorV1::SequenceExhausted);
                }
                Ok(entry.sequence)
            }
            Err(_) if self.entries.len() == KAIGI_MAX_PARTICIPANTS_V1 => {
                Err(KaigiPrivateParticipationErrorV1::Capacity)
            }
            Err(_) => Ok(1),
        }
    }

    /// Record one successfully authorized join without permitting replacement.
    ///
    /// # Errors
    /// Rejects an unexpected sequence, a duplicate commitment, or any
    /// [`Self::prepare_join`] failure. Every failure leaves the ledger unchanged.
    /// Proof, signer, call/root and nullifier checks belong to the caller.
    /// Core must supply the retained original account after authenticating its
    /// unique active rekey successor; this owner never resolves aliases itself.
    pub fn commit_join(
        &mut self,
        original_account: &AccountId,
        expected_sequence: u64,
        commitment: KaigiAuthorizationScalarV1,
    ) -> Result<(), KaigiPrivateParticipationErrorV1> {
        if self.prepare_join(original_account)? != expected_sequence {
            return Err(KaigiPrivateParticipationErrorV1::SequenceMismatch);
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.active_commitment == Some(commitment))
        {
            return Err(KaigiPrivateParticipationErrorV1::DuplicateCommitment);
        }
        match self
            .entries
            .binary_search_by(|entry| entry.original_account.cmp(original_account))
        {
            Ok(index) => self.entries[index].active_commitment = Some(commitment),
            Err(index) => self.entries.insert(
                index,
                KaigiPrivateParticipationV1 {
                    original_account: original_account.clone(),
                    sequence: expected_sequence,
                    active_commitment: Some(commitment),
                },
            ),
        }
        Ok(())
    }

    /// Inspect the sequence and exact commitment a leave must authorize.
    ///
    /// # Errors
    /// Rejects malformed state, an inactive subject or another commitment.
    pub fn prepare_leave(
        &self,
        original_account: &AccountId,
        commitment: KaigiAuthorizationScalarV1,
    ) -> Result<u64, KaigiPrivateParticipationErrorV1> {
        self.validate()?;
        let index = self
            .entries
            .binary_search_by(|entry| entry.original_account.cmp(original_account))
            .map_err(|_| KaigiPrivateParticipationErrorV1::NotActive)?;
        let entry = &self.entries[index];
        let active = entry
            .active_commitment
            .ok_or(KaigiPrivateParticipationErrorV1::NotActive)?;
        if active != commitment {
            return Err(KaigiPrivateParticipationErrorV1::CommitmentMismatch);
        }
        Ok(entry.sequence)
    }

    /// Retire the exact live commitment and advance its subject's sequence.
    ///
    /// # Errors
    /// Rejects a stale sequence, exhaustion or any [`Self::prepare_leave`]
    /// failure. Every failure leaves the ledger unchanged. Old nullifiers must
    /// remain in the call's independently bounded append-only log.
    pub fn commit_leave(
        &mut self,
        original_account: &AccountId,
        expected_sequence: u64,
        commitment: KaigiAuthorizationScalarV1,
    ) -> Result<(), KaigiPrivateParticipationErrorV1> {
        let sequence = self.prepare_leave(original_account, commitment)?;
        if sequence != expected_sequence {
            return Err(KaigiPrivateParticipationErrorV1::SequenceMismatch);
        }
        let successor = sequence
            .checked_add(1)
            .ok_or(KaigiPrivateParticipationErrorV1::SequenceExhausted)?;
        let index = self
            .entries
            .binary_search_by(|entry| entry.original_account.cmp(original_account))
            .map_err(|_| KaigiPrivateParticipationErrorV1::NotActive)?;
        self.entries[index].sequence = successor;
        self.entries[index].active_commitment = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(value: u64) -> AccountId {
        let pair = iroha_crypto::KeyPair::from_seed(
            value.to_le_bytes().to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        );
        AccountId::new(pair.public_key().clone())
    }
    fn commitment(value: u8) -> KaigiAuthorizationScalarV1 {
        KaigiAuthorizationScalarV1::from_le_bytes([value; 32]).expect("canonical scalar")
    }
    fn roster(values: &[u8]) -> Vec<KaigiAuthorizationScalarV1> {
        values.iter().map(|value| commitment(*value)).collect()
    }

    #[test]
    fn join_leave_rejoin_preserves_ownership_and_consumes_sequences() {
        let mut ledger = KaigiPrivateParticipationLedgerV1::default();
        assert_eq!(ledger.prepare_join(&subject(2)), Ok(1));
        ledger.commit_join(&subject(2), 1, commitment(3)).unwrap();
        ledger.commit_join(&subject(1), 1, commitment(5)).unwrap();
        let participant = ledger
            .entries()
            .iter()
            .find(|entry| entry.original_account() == &subject(1))
            .unwrap();
        assert_eq!(participant.original_account(), &subject(1));
        assert_eq!(participant.sequence(), 1);
        assert_eq!(participant.active_commitment(), Some(commitment(5)));
        ledger.validate_against_roster(&roster(&[3, 5])).unwrap();
        let before = ledger.clone();
        assert_eq!(
            ledger.commit_join(&subject(2), 1, commitment(7)),
            Err(KaigiPrivateParticipationErrorV1::AlreadyActive)
        );
        assert_eq!(ledger, before);
        assert_eq!(ledger.prepare_leave(&subject(2), commitment(3)), Ok(1));
        ledger.commit_leave(&subject(2), 1, commitment(3)).unwrap();
        ledger.validate_against_roster(&roster(&[5])).unwrap();
        assert_eq!(ledger.prepare_join(&subject(2)), Ok(2));
        let before = ledger.clone();
        assert_eq!(
            ledger.commit_join(&subject(2), 1, commitment(7)),
            Err(KaigiPrivateParticipationErrorV1::SequenceMismatch)
        );
        assert_eq!(
            ledger.commit_leave(&subject(2), 1, commitment(3)),
            Err(KaigiPrivateParticipationErrorV1::NotActive)
        );
        assert_eq!(ledger, before);
        ledger.commit_join(&subject(2), 2, commitment(7)).unwrap();
        assert_eq!(
            ledger.prepare_leave(&subject(2), commitment(3)),
            Err(KaigiPrivateParticipationErrorV1::CommitmentMismatch)
        );
        ledger.validate_against_roster(&roster(&[5, 7])).unwrap();
    }

    #[test]
    fn failed_cross_subject_or_stale_transitions_never_mutate_state() {
        let mut ledger = KaigiPrivateParticipationLedgerV1::default();
        ledger.commit_join(&subject(1), 1, commitment(3)).unwrap();
        let before = ledger.clone();
        assert_eq!(
            ledger.commit_join(&subject(2), 1, commitment(3)),
            Err(KaigiPrivateParticipationErrorV1::DuplicateCommitment)
        );
        assert_eq!(
            ledger.commit_leave(&subject(1), 2, commitment(3)),
            Err(KaigiPrivateParticipationErrorV1::SequenceMismatch)
        );
        assert_eq!(
            ledger.commit_leave(&subject(2), 1, commitment(3)),
            Err(KaigiPrivateParticipationErrorV1::NotActive)
        );
        assert_eq!(
            ledger.commit_leave(&subject(1), 1, commitment(5)),
            Err(KaigiPrivateParticipationErrorV1::CommitmentMismatch)
        );
        assert_eq!(ledger, before);
        assert!(
            !KaigiPrivateParticipationErrorV1::NotActive
                .to_string()
                .is_empty()
        );
    }

    #[test]
    fn restored_state_requires_exact_unique_roster_and_sorted_subjects() {
        let mut ledger = KaigiPrivateParticipationLedgerV1::default();
        ledger.commit_join(&subject(1), 1, commitment(3)).unwrap();
        ledger.commit_join(&subject(2), 1, commitment(5)).unwrap();
        let wire = norito::to_bytes(&ledger).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<KaigiPrivateParticipationLedgerV1>(&wire).unwrap(),
            ledger
        );
        let json = norito::json::to_json(&ledger).unwrap();
        assert_eq!(
            norito::json::from_str::<KaigiPrivateParticipationLedgerV1>(&json).unwrap(),
            ledger
        );
        for values in [&[][..], &[3][..], &[3, 3][..], &[3, 5, 7][..]] {
            assert_eq!(
                ledger.validate_against_roster(&roster(values)),
                Err(KaigiPrivateParticipationErrorV1::InvalidState)
            );
        }
        let mut reordered = ledger.clone();
        reordered.entries.swap(0, 1);
        assert_eq!(
            reordered.prepare_join(&subject(3)),
            Err(KaigiPrivateParticipationErrorV1::InvalidState)
        );
        let mut duplicate = ledger.clone();
        duplicate.entries[1].original_account = duplicate.entries[0].original_account.clone();
        assert!(duplicate.validate().is_err());
        duplicate = ledger.clone();
        duplicate.entries[1].active_commitment = duplicate.entries[0].active_commitment;
        assert!(duplicate.validate().is_err());
        ledger.entries[0].sequence = 0;
        assert_eq!(
            ledger.validate_against_roster(&roster(&[3, 5])),
            Err(KaigiPrivateParticipationErrorV1::InvalidState)
        );
    }

    #[test]
    fn retained_account_preserves_complete_multisig_policy_across_roundtrips_and_leave() {
        use crate::account::{MultisigMember, MultisigPolicy};

        let members: Vec<_> = (1..=24)
            .map(|seed| {
                MultisigMember::new(subject(seed).expect_single_signatory().clone(), 1).unwrap()
            })
            .collect();
        let policy = MultisigPolicy::new(2, members).unwrap();
        let original = AccountId::new_multisig(policy.clone());
        let mut changed_members = policy.members().to_vec();
        let last = changed_members.last_mut().unwrap();
        *last = MultisigMember::new(last.public_key().clone(), 2).unwrap();
        let changed = AccountId::new_multisig(MultisigPolicy::new(2, changed_members).unwrap());
        assert_ne!(original, changed);
        let original_frame = norito::encode_canonical(&original).unwrap();
        assert!(original_frame.len() > 256);

        let mut ledger = KaigiPrivateParticipationLedgerV1::default();
        ledger.commit_join(&original, 1, commitment(3)).unwrap();
        ledger.commit_join(&changed, 1, commitment(5)).unwrap();
        ledger.commit_leave(&original, 1, commitment(3)).unwrap();
        let frame = norito::to_bytes(&ledger).unwrap();
        let decoded =
            norito::decode_from_bytes::<KaigiPrivateParticipationLedgerV1>(&frame).unwrap();
        assert_eq!(decoded, ledger);
        decoded.validate_against_roster(&roster(&[5])).unwrap();
        let retained = decoded
            .entries()
            .iter()
            .find(|entry| entry.original_account() == &original)
            .unwrap();
        assert_eq!(
            norito::encode_canonical(retained.original_account()).unwrap(),
            original_frame
        );
        assert_eq!(retained.sequence(), 2);
        assert_eq!(retained.active_commitment(), None);
        assert_eq!(decoded.prepare_join(&original), Ok(2));
        assert_eq!(
            decoded.prepare_join(&changed),
            Err(KaigiPrivateParticipationErrorV1::AlreadyActive)
        );

        {
            let json = norito::json::to_json(&ledger).unwrap();
            assert_eq!(
                norito::json::from_str::<KaigiPrivateParticipationLedgerV1>(&json).unwrap(),
                ledger
            );
            assert!(json.contains("original_account"));
            assert!(!json.contains("subject_id"));
        }
    }

    #[test]
    fn subject_bound_and_sequence_exhaustion_preserve_a_leave_reservation() {
        let mut ledger = KaigiPrivateParticipationLedgerV1 {
            entries: (1..=KAIGI_MAX_PARTICIPANTS_V1)
                .map(|index| KaigiPrivateParticipationV1 {
                    original_account: subject(u64::try_from(index).unwrap()),
                    sequence: 2,
                    active_commitment: None,
                })
                .collect(),
        };
        // Entries use canonical AccountId order, independently of fixture seed order.
        ledger
            .entries
            .sort_by(|left, right| left.original_account.cmp(&right.original_account));
        ledger.validate_against_roster(&[]).unwrap();
        assert_eq!(ledger.prepare_join(&subject(1)), Ok(2));
        assert_eq!(
            ledger.prepare_join(&subject(5000)),
            Err(KaigiPrivateParticipationErrorV1::Capacity)
        );
        ledger.entries[0].sequence = u64::MAX;
        let id = ledger.entries[0].original_account.clone();
        assert_eq!(
            ledger.prepare_join(&id),
            Err(KaigiPrivateParticipationErrorV1::SequenceExhausted)
        );
        ledger.entries[0].active_commitment = Some(commitment(3));
        let before = ledger.clone();
        assert_eq!(
            ledger.commit_leave(&id, u64::MAX, commitment(3)),
            Err(KaigiPrivateParticipationErrorV1::InvalidState)
        );
        assert_eq!(ledger, before);
        ledger.entries[0].sequence = u64::MAX - 1;
        ledger
            .commit_leave(&id, u64::MAX - 1, commitment(3))
            .unwrap();
        assert_eq!(ledger.entries[0].sequence(), u64::MAX);
        assert_eq!(
            ledger.prepare_join(&id),
            Err(KaigiPrivateParticipationErrorV1::SequenceExhausted)
        );
        ledger.entries.push(KaigiPrivateParticipationV1 {
            original_account: subject(5001),
            sequence: 1,
            active_commitment: None,
        });
        assert!(ledger.validate().is_err());
    }
}
