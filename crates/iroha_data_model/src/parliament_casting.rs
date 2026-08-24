//! Finality-bound commitments for SORA Parliament timed-OVN casting contexts.
//!
//! A context is included only when Core has joined and rechecked the authoritative
//! active governance attempt, hidden-binding body, active ballot, timed-OVN
//! lifecycle phase, and exact height window. The compact leaf binds the public
//! archive material needed to reconstruct the cryptographic session without
//! carrying the response-sized TLE transcript or registration corpus.

use core::num::NonZeroU64;
use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree, MerkleTreeCommitment};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::parliament_types::{
    BallotAttemptId, BodyInstanceId, GovernanceAttemptId, ProposalContentId, TleKeySessionId,
};

/// Current compact casting-context commitment version.
pub const PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1: u16 = 1;
/// Protocol maximum of simultaneously cast-capable timed-OVN contexts.
pub const MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1: u32 = 1_000;
/// Exact sparse-tree depth of the execution-witness proof.
pub const PARLIAMENT_TIMED_OVN_CASTING_WITNESS_SIBLINGS_V1: usize = 256;
/// Fixed synthetic execution-witness key committed by every non-replay block.
pub const PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1: &[u8] =
    b"\xd5iroha:parliament:timed-ovn:casting-contexts:v1";

/// Return whether one more cast-capable context fits the protocol maximum.
///
/// Callers must pass the exact number of currently authorized contexts before
/// admitting a new lifecycle. Values at or above the maximum fail closed.
#[must_use]
pub const fn parliament_timed_ovn_casting_capacity_allows_new_v1(current_count: u32) -> bool {
    current_count < MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1
}

const REGISTRATION_CORPUS_DOMAIN_V1: &[u8] = b"iroha:parliament:timed-ovn:registration-corpus:v1\0";
const EMPTY_CONTEXT_ROOT_DOMAIN_V1: &[u8] =
    b"iroha:parliament:timed-ovn:casting-contexts:empty:v1\0";

/// Cast-capable lifecycle phases admitted to the authenticated context set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "phase", content = "value", rename_all = "SCREAMING_SNAKE_CASE")
)]
pub enum ParliamentTimedOvnCastingPhaseV1 {
    /// Participant registration is open.
    Registered,
    /// Registration is immutable and authenticated dropouts may accumulate.
    RegistrationClosed,
    /// The exact survivor subsequence and future release identity are frozen.
    SurvivorsFrozen,
}

/// Cached commitment to the exact ordered canonical registration-record bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ParliamentTimedOvnRegistrationCorpusCommitmentV1 {
    /// Commitment format version.
    pub version: u16,
    /// Exact number of canonical records covered by `digest`.
    pub record_count: u32,
    /// Domain-separated digest of count, record lengths, and exact record bytes.
    pub digest: Hash,
}

impl ParliamentTimedOvnRegistrationCorpusCommitmentV1 {
    /// Commit to an exact ordered registration corpus without materializing a second corpus.
    #[must_use]
    pub fn from_records(records: &[Vec<u8>]) -> Option<Self> {
        let record_count = u32::try_from(records.len()).ok()?;
        let digest = Hash::new_from_writer(|writer| {
            writer.write_all(REGISTRATION_CORPUS_DOMAIN_V1)?;
            writer.write_all(&PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1.to_be_bytes())?;
            writer.write_all(&record_count.to_be_bytes())?;
            for record in records {
                let record_len = u32::try_from(record.len()).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "timed-OVN registration record length exceeds u32",
                    )
                })?;
                writer.write_all(&record_len.to_be_bytes())?;
                writer.write_all(record)?;
            }
            Ok(())
        })
        .ok()?;
        Some(Self {
            version: PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1,
            record_count,
            digest,
        })
    }

    /// Return whether this is the exact commitment to `records`.
    #[must_use]
    pub fn matches_records(&self, records: &[Vec<u8>]) -> bool {
        Self::from_records(records).as_ref() == Some(self)
    }
}

/// Compact exact future-release identity carried after survivor freeze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ParliamentTimedOvnReleaseBindingV1 {
    /// Long-lived TLE threshold key session.
    pub tle_key_session_id: TleKeySessionId,
    /// Governance lifecycle attempt.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Governed Parliament body instance.
    pub body_instance_id: BodyInstanceId,
    /// Retryable hidden ballot attempt.
    pub ballot_attempt_id: BallotAttemptId,
    /// Replay-derived root of the exact frozen survivor corpus.
    pub survivor_corpus_root: [u8; 32],
    /// Replay-derived sentinel proving that no post-freeze recovery corpus exists.
    pub no_recovery_root: [u8; 32],
    /// First finalized height permitting threshold release.
    pub target_finalized_height: u64,
    /// Exact timed-OVN parameter-profile commitment.
    pub parameter_hash: [u8; 32],
}

/// Compact archive-derived commitment for one authorized timed-OVN casting context.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ParliamentTimedOvnCastingContextBindingV1 {
    /// Binding format version.
    pub version: u16,
    /// Finalized block height at which Core rechecked authorization.
    pub evaluated_height: u64,
    /// Exact cast-capable lifecycle phase.
    pub phase: ParliamentTimedOvnCastingPhaseV1,
    /// Canonical network/genesis binding.
    pub network_id: [u8; 32],
    /// Immutable proposal content.
    pub proposal_content_id: ProposalContentId,
    /// Active governance lifecycle attempt.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Active hidden-binding body instance.
    pub body_instance_id: BodyInstanceId,
    /// Active hidden ballot attempt and commitment-set key.
    pub ballot_attempt_id: BallotAttemptId,
    /// Exact timed-OVN parameter-profile commitment.
    pub parameter_hash: [u8; 32],
    /// Long-lived TLE threshold key session.
    pub tle_key_session_id: TleKeySessionId,
    /// Commitment to the complete replay-validated adaptive TLE transcript.
    pub tle_key_transcript_hash: [u8; 32],
    /// Canonical compressed TLE threshold master public key.
    pub tle_master_public_key: [u8; 96],
    /// Finalized height at which registration opened.
    pub registration_opened_at_finalized_height: u64,
    /// First height at which registration is closed.
    pub registration_close_height: u64,
    /// First height at which the survivor set is frozen.
    pub survivor_freeze_height: u64,
    /// First height at which timed commitments are closed.
    pub commitment_close_height: u64,
    /// First height at which threshold release is permitted.
    pub target_finalized_height: u64,
    /// Cached exact ordered registration-corpus commitment.
    pub registration_corpus: ParliamentTimedOvnRegistrationCorpusCommitmentV1,
    /// Exact survivor count, present only after survivor freeze.
    pub survivor_count: Option<u32>,
    /// Root of one authenticated keep/drop decision per registration after freeze.
    pub dropout_root: Option<[u8; 32]>,
    /// Exact replay-derived release identity, present only after survivor freeze.
    pub release_identity: Option<ParliamentTimedOvnReleaseBindingV1>,
}

impl ParliamentTimedOvnCastingContextBindingV1 {
    /// Return whether all compact bindings and the exact phase window are coherent.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        if self.version != PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1
            || self.registration_corpus.version
                != PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1
            || self.registration_corpus.record_count
                > crate::parliament_types::MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1
            || self.registration_opened_at_finalized_height == 0
            || !(self.registration_opened_at_finalized_height < self.registration_close_height
                && self.registration_close_height < self.survivor_freeze_height
                && self.survivor_freeze_height < self.commitment_close_height
                && self.commitment_close_height < self.target_finalized_height)
        {
            return false;
        }
        let in_phase_window = match self.phase {
            ParliamentTimedOvnCastingPhaseV1::Registered => {
                self.evaluated_height >= self.registration_opened_at_finalized_height
                    && self.evaluated_height < self.registration_close_height
            }
            ParliamentTimedOvnCastingPhaseV1::RegistrationClosed => {
                self.evaluated_height >= self.registration_close_height
                    && self.evaluated_height < self.survivor_freeze_height
            }
            ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen => {
                self.evaluated_height >= self.survivor_freeze_height
                    && self.evaluated_height < self.commitment_close_height
            }
        };
        if !in_phase_window {
            return false;
        }
        match self.phase {
            ParliamentTimedOvnCastingPhaseV1::Registered => {
                self.survivor_count.is_none()
                    && self.dropout_root.is_none()
                    && self.release_identity.is_none()
            }
            ParliamentTimedOvnCastingPhaseV1::RegistrationClosed => {
                self.registration_corpus.record_count > 0
                    && self.survivor_count.is_none()
                    && self.dropout_root.is_none()
                    && self.release_identity.is_none()
            }
            ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen => {
                let Some(survivor_count) = self.survivor_count else {
                    return false;
                };
                let Some(dropout_root) = self.dropout_root else {
                    return false;
                };
                let Some(release) = self.release_identity else {
                    return false;
                };
                survivor_count > 0
                    && survivor_count <= self.registration_corpus.record_count
                    && dropout_root.iter().any(|byte| *byte != 0)
                    && release.tle_key_session_id == self.tle_key_session_id
                    && release.governance_attempt_id == self.governance_attempt_id
                    && release.body_instance_id == self.body_instance_id
                    && release.ballot_attempt_id == self.ballot_attempt_id
                    && release.target_finalized_height == self.target_finalized_height
                    && release.parameter_hash == self.parameter_hash
                    && release.survivor_corpus_root.iter().any(|byte| *byte != 0)
                    && release.no_recovery_root.iter().any(|byte| *byte != 0)
            }
        }
    }
}

/// Root-and-count commitment written into every non-replay block witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ParliamentTimedOvnCastingSnapshotCommitmentV1 {
    /// Snapshot format version.
    pub version: u16,
    /// Block height whose post-execution state was evaluated.
    pub evaluated_height: u64,
    /// Canonical application-Merkle root, or the fixed empty-set root when `count` is zero.
    pub root: Hash,
    /// Exact number of authorized contexts committed by `root`.
    pub count: u32,
}

impl ParliamentTimedOvnCastingSnapshotCommitmentV1 {
    /// Derive the fixed empty-set commitment for one block height.
    #[must_use]
    pub fn empty(evaluated_height: u64) -> Self {
        Self {
            version: PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1,
            evaluated_height,
            root: parliament_timed_ovn_empty_casting_root_v1(),
            count: 0,
        }
    }

    /// Commit a strictly ballot-id-ordered set of independently valid bindings.
    pub fn from_ordered_bindings(
        evaluated_height: u64,
        bindings: &[ParliamentTimedOvnCastingContextBindingV1],
    ) -> Result<Self, &'static str> {
        if evaluated_height == 0 {
            return Err("casting snapshot evaluated height must be nonzero");
        }
        let count = u32::try_from(bindings.len()).map_err(|_| "casting context count overflow")?;
        if count > MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1 {
            return Err("casting context count exceeds the protocol maximum");
        }
        if bindings
            .iter()
            .any(|binding| binding.evaluated_height != evaluated_height || !binding.is_valid())
            || bindings
                .windows(2)
                .any(|pair| pair[0].ballot_attempt_id >= pair[1].ballot_attempt_id)
        {
            return Err("casting context bindings are invalid or not canonically ordered");
        }
        if bindings.is_empty() {
            return Ok(Self::empty(evaluated_height));
        }
        let root = MerkleTree::<ParliamentTimedOvnCastingContextBindingV1>::root_from_typed_leaves(
            bindings.iter().map(HashOf::new),
        )
        .ok_or("non-empty casting context tree has no root")?;
        Ok(Self {
            version: PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1,
            evaluated_height,
            root: root.into(),
            count,
        })
    }

    /// Return whether the root, count, and protocol version are coherent.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.version == PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1
            && self.evaluated_height != 0
            && self.count <= MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1
            && if self.count == 0 {
                self.root == parliament_timed_ovn_empty_casting_root_v1()
            } else {
                self.root != parliament_timed_ovn_empty_casting_root_v1()
            }
    }
}

/// Merkle membership proof for one compact casting-context binding.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ParliamentTimedOvnCastingContextMembershipProofV1 {
    proof: MerkleProof<ParliamentTimedOvnCastingContextBindingV1>,
}

impl ParliamentTimedOvnCastingContextMembershipProofV1 {
    /// Wrap the canonical application-Merkle proof for one context leaf.
    #[must_use]
    pub const fn new(proof: MerkleProof<ParliamentTimedOvnCastingContextBindingV1>) -> Self {
        Self { proof }
    }

    /// Borrow the canonical application-Merkle proof.
    #[must_use]
    pub const fn proof(&self) -> &MerkleProof<ParliamentTimedOvnCastingContextBindingV1> {
        &self.proof
    }

    /// Verify membership using only the archive-derived compact binding and snapshot.
    #[must_use]
    pub fn verify(
        &self,
        binding: &ParliamentTimedOvnCastingContextBindingV1,
        snapshot: &ParliamentTimedOvnCastingSnapshotCommitmentV1,
    ) -> bool {
        if !binding.is_valid()
            || !snapshot.is_valid()
            || snapshot.count == 0
            || binding.evaluated_height != snapshot.evaluated_height
        {
            return false;
        }
        let Some(leaf_count) = NonZeroU64::new(u64::from(snapshot.count)) else {
            return false;
        };
        let root =
            HashOf::<MerkleTree<ParliamentTimedOvnCastingContextBindingV1>>::from_untyped_unchecked(
                snapshot.root,
            );
        self.proof.verify(
            &HashOf::new(binding),
            &MerkleTreeCommitment::new(root, leaf_count),
        )
    }
}

/// Sparse-SMT proof that the casting-context snapshot is an ordinary write.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ParliamentTimedOvnCastingWitnessProofV1 {
    /// Fixed raw execution-witness key.
    pub key: Vec<u8>,
    /// Exact canonical encoded snapshot commitment.
    pub value: Vec<u8>,
    /// Exactly 256 siblings from leaf level to the ordinary-write root.
    pub siblings: Vec<Hash>,
}

/// Finalized proof material for one requested authorized casting context.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ParliamentTimedOvnFinalizedCastingProofV1 {
    /// Fixed-write proof tying the snapshot to the block's ordinary-write root.
    pub snapshot_witness: ParliamentTimedOvnCastingWitnessProofV1,
    /// Exact archive-derived compact context leaf.
    pub binding: ParliamentTimedOvnCastingContextBindingV1,
    /// Membership of `binding` in the authenticated snapshot root and count.
    pub membership_proof: ParliamentTimedOvnCastingContextMembershipProofV1,
}

impl ParliamentTimedOvnFinalizedCastingProofV1 {
    /// Verify the fixed write and context membership against a finality-authenticated root.
    #[must_use]
    pub fn verify(&self, expected_ordinary_writes_root: Hash) -> bool {
        let Ok(snapshot) = self.snapshot_witness.commitment() else {
            return false;
        };
        self.snapshot_witness.verify(expected_ordinary_writes_root)
            && self.membership_proof.verify(&self.binding, &snapshot)
    }
}

impl ParliamentTimedOvnCastingWitnessProofV1 {
    /// Verify the fixed synthetic write against an ordinary-write SMT root.
    #[must_use]
    pub fn verify(&self, expected_ordinary_writes_root: Hash) -> bool {
        if self.key != PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1
            || self.siblings.len() != PARLIAMENT_TIMED_OVN_CASTING_WITNESS_SIBLINGS_V1
        {
            return false;
        }
        let Ok(commitment) =
            norito::decode_canonical::<ParliamentTimedOvnCastingSnapshotCommitmentV1>(&self.value)
        else {
            return false;
        };
        if !commitment.is_valid() {
            return false;
        }
        let path = Hash::new(&self.key);
        let value_hash = Hash::new(&self.value);
        let mut leaf_preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
        leaf_preimage.push(0);
        leaf_preimage.extend_from_slice(path.as_ref());
        leaf_preimage.extend_from_slice(value_hash.as_ref());
        let mut current = Hash::new(leaf_preimage);
        for (level, sibling) in self.siblings.iter().copied().enumerate() {
            let path_bit = 255_usize.saturating_sub(level);
            let byte = path.as_ref()[path_bit / 8];
            let right = byte & (1_u8 << (path_bit % 8)) != 0;
            current = if right {
                casting_ordinary_smt_node_hash(sibling, current)
            } else {
                casting_ordinary_smt_node_hash(current, sibling)
            };
        }
        current == expected_ordinary_writes_root
    }

    /// Decode and return the exact canonical snapshot commitment.
    pub fn commitment(&self) -> Result<ParliamentTimedOvnCastingSnapshotCommitmentV1, String> {
        let commitment: ParliamentTimedOvnCastingSnapshotCommitmentV1 =
            norito::decode_canonical(&self.value).map_err(|error| {
                if matches!(&error, norito::Error::NonCanonicalEncoding) {
                    "timed-OVN casting snapshot commitment is non-canonical".to_owned()
                } else {
                    format!("timed-OVN casting snapshot commitment is invalid: {error}")
                }
            })?;
        if !commitment.is_valid() {
            return Err("timed-OVN casting snapshot commitment is incoherent".to_owned());
        }
        Ok(commitment)
    }
}

/// Return the fixed domain-separated root for an empty authorized context set.
#[must_use]
pub fn parliament_timed_ovn_empty_casting_root_v1() -> Hash {
    Hash::new(EMPTY_CONTEXT_ROOT_DOMAIN_V1)
}

fn casting_ordinary_smt_node_hash(left: Hash, right: Hash) -> Hash {
    let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
    preimage.push(1);
    preimage.extend_from_slice(left.as_ref());
    preimage.extend_from_slice(right.as_ref());
    Hash::new(preimage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(height: u64, ballot: u8) -> ParliamentTimedOvnCastingContextBindingV1 {
        ParliamentTimedOvnCastingContextBindingV1 {
            version: PARLIAMENT_TIMED_OVN_CASTING_COMMITMENT_VERSION_V1,
            evaluated_height: height,
            phase: ParliamentTimedOvnCastingPhaseV1::Registered,
            network_id: [1; 32],
            proposal_content_id: ProposalContentId::new([2; 32]),
            governance_attempt_id: GovernanceAttemptId::new([3; 32]),
            body_instance_id: BodyInstanceId::new([4; 32]),
            ballot_attempt_id: BallotAttemptId::new([ballot; 32]),
            parameter_hash: [5; 32],
            tle_key_session_id: TleKeySessionId::new([6; 32]),
            tle_key_transcript_hash: [7; 32],
            tle_master_public_key: [8; 96],
            registration_opened_at_finalized_height: 10,
            registration_close_height: 20,
            survivor_freeze_height: 30,
            commitment_close_height: 40,
            target_finalized_height: 50,
            registration_corpus:
                ParliamentTimedOvnRegistrationCorpusCommitmentV1::from_records(&[])
                    .expect("empty corpus commitment"),
            survivor_count: None,
            dropout_root: None,
            release_identity: None,
        }
    }

    #[test]
    fn empty_snapshot_uses_fixed_root() {
        let snapshot =
            ParliamentTimedOvnCastingSnapshotCommitmentV1::from_ordered_bindings(12, &[])
                .expect("empty commitment");
        assert_eq!(
            snapshot,
            ParliamentTimedOvnCastingSnapshotCommitmentV1::empty(12)
        );
        assert!(snapshot.is_valid());
        assert!(!ParliamentTimedOvnCastingSnapshotCommitmentV1::empty(0).is_valid());
        assert!(
            ParliamentTimedOvnCastingSnapshotCommitmentV1::from_ordered_bindings(0, &[]).is_err()
        );
    }

    #[test]
    fn membership_is_deterministic_and_rejects_tampering() {
        let bindings = vec![binding(12, 1), binding(12, 2)];
        let snapshot =
            ParliamentTimedOvnCastingSnapshotCommitmentV1::from_ordered_bindings(12, &bindings)
                .expect("snapshot");
        let tree = MerkleTree::from_iter(bindings.iter().map(HashOf::new));
        let proof = ParliamentTimedOvnCastingContextMembershipProofV1::new(
            tree.get_proof(1).expect("second proof"),
        );
        assert!(proof.verify(&bindings[1], &snapshot));
        let mut tampered = bindings[1].clone();
        tampered.tle_key_transcript_hash[0] ^= 1;
        assert!(!proof.verify(&tampered, &snapshot));
        let repeated =
            ParliamentTimedOvnCastingSnapshotCommitmentV1::from_ordered_bindings(12, &bindings)
                .expect("repeated snapshot");
        assert_eq!(snapshot, repeated);
    }

    #[test]
    fn registration_commitment_binds_order_lengths_and_bytes() {
        let records = vec![vec![1, 2], vec![3]];
        let commitment = ParliamentTimedOvnRegistrationCorpusCommitmentV1::from_records(&records)
            .expect("corpus commitment");
        assert!(commitment.matches_records(&records));
        assert!(!commitment.matches_records(&[vec![1], vec![2, 3]]));
        assert!(!commitment.matches_records(&[vec![3], vec![1, 2]]));
    }

    #[test]
    fn casting_capacity_rejects_admission_at_the_protocol_maximum() {
        assert!(parliament_timed_ovn_casting_capacity_allows_new_v1(
            MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1 - 1
        ));
        assert!(!parliament_timed_ovn_casting_capacity_allows_new_v1(
            MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1
        ));
        assert!(!parliament_timed_ovn_casting_capacity_allows_new_v1(
            MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1 + 1
        ));
    }

    #[test]
    fn casting_binding_snapshot_and_membership_roundtrip_canonically() {
        let bindings = vec![binding(12, 1), binding(12, 2)];
        let snapshot =
            ParliamentTimedOvnCastingSnapshotCommitmentV1::from_ordered_bindings(12, &bindings)
                .expect("casting snapshot");
        let tree = MerkleTree::from_iter(bindings.iter().map(HashOf::new));
        let membership = ParliamentTimedOvnCastingContextMembershipProofV1::new(
            tree.get_proof(0).expect("first membership proof"),
        );

        let binding_bytes = norito::to_bytes(&bindings[0]).expect("encode casting binding");
        let decoded_binding: ParliamentTimedOvnCastingContextBindingV1 =
            norito::decode_canonical(&binding_bytes).expect("decode canonical casting binding");
        assert_eq!(decoded_binding, bindings[0]);
        let snapshot_bytes = norito::to_bytes(&snapshot).expect("encode casting snapshot");
        let decoded_snapshot: ParliamentTimedOvnCastingSnapshotCommitmentV1 =
            norito::decode_canonical(&snapshot_bytes).expect("decode canonical casting snapshot");
        assert_eq!(decoded_snapshot, snapshot);
        let membership_bytes = norito::to_bytes(&membership).expect("encode membership proof");
        let decoded_membership: ParliamentTimedOvnCastingContextMembershipProofV1 =
            norito::decode_canonical(&membership_bytes).expect("decode canonical membership proof");
        assert_eq!(decoded_membership, membership);
        assert!(decoded_membership.verify(&decoded_binding, &decoded_snapshot));
    }
}
