//! Native AMX control-plane messages and deterministic vote-session cache.

use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::consensus::{NativeAmxAttestationBodyV1, NativeAmxAttestationQcV1, NativeAmxPhase},
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    peer::PeerId,
};
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Native AMX session key scoped to one source transaction and routing plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct NativeAmxSessionKey {
    /// Source transaction hash/id.
    pub source_id: [u8; iroha_crypto::Hash::LENGTH],
    /// Full routing-plan digest.
    pub plan_digest: Hash,
}

impl NativeAmxSessionKey {
    /// Construct a session key from an attestation body.
    #[must_use]
    pub fn from_body(body: &NativeAmxAttestationBodyV1) -> Self {
        Self {
            source_id: body.source_id,
            plan_digest: body.plan_digest,
        }
    }
}

/// Individual native AMX vote before participant committee aggregation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxVoteV1 {
    /// Body signed by the participant validator.
    pub body: NativeAmxAttestationBodyV1,
    /// Validator that produced the vote.
    pub signer: PeerId,
    /// BLS signature over [`NativeAmxAttestationBodyV1::signature_preimage`].
    pub bls_signature: Vec<u8>,
}

/// Native AMX control-plane request or vote.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum NativeAmxMessage {
    /// Coordinator asks a participant dataspace committee to prepare a leg.
    PrepareRequest(NativeAmxAttestationBodyV1),
    /// Participant validator prepare vote.
    PrepareVote(NativeAmxVoteV1),
    /// Coordinator asks a participant dataspace committee to commit a prepared leg.
    CommitRequest(NativeAmxAttestationBodyV1),
    /// Participant validator commit vote.
    CommitVote(NativeAmxVoteV1),
}

/// Failure while adding a native AMX vote to the session cache.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxSessionError {
    /// native AMX vote phase does not match the target cache bucket
    #[error("native AMX vote phase does not match the target cache bucket")]
    PhaseMismatch,
    /// native AMX vote signer already exists in this session
    #[error("native AMX vote signer already exists in this session")]
    DuplicateSigner,
}

/// Failure while building a native AMX attestation QC from participant votes.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxQcBuildError {
    /// no votes were supplied for the requested native AMX phase
    #[error("no votes were supplied for the requested native AMX phase")]
    EmptyVotes,
    /// a vote signed a different native AMX attestation body
    #[error("a vote signed a different native AMX attestation body")]
    BodyMismatch,
    /// a vote signer is not in the participant validator set
    #[error("a vote signer is not in the participant validator set")]
    SignerNotInValidatorSet,
    /// a vote signer appears more than once
    #[error("a vote signer appears more than once")]
    DuplicateSigner,
    /// the vote set does not satisfy the participant quorum
    #[error("native AMX vote quorum is not met")]
    QuorumNotMet,
    /// BLS signature aggregation failed
    #[error("failed to aggregate native AMX BLS signatures")]
    SignatureAggregate,
}

/// Build a native AMX attestation QC from sorted or unsorted participant votes.
///
/// The resulting bitmap and aggregate signature are deterministic because votes are projected into
/// the supplied validator-set order before aggregation.
///
/// # Errors
/// Returns an error when votes do not match `body`, include duplicate or unknown signers, fail to
/// meet `min_signers`, or cannot be aggregated as BLS-normal signatures.
pub fn aggregate_votes_to_qc(
    body: NativeAmxAttestationBodyV1,
    validator_set: Vec<PeerId>,
    votes: &[NativeAmxVoteV1],
    min_signers: usize,
) -> Result<NativeAmxAttestationQcV1, NativeAmxQcBuildError> {
    if votes.is_empty() {
        return Err(NativeAmxQcBuildError::EmptyVotes);
    }

    let mut indexed_signatures: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for vote in votes {
        if vote.body != body {
            return Err(NativeAmxQcBuildError::BodyMismatch);
        }
        let Some(index) = validator_set
            .iter()
            .position(|validator| validator == &vote.signer)
        else {
            return Err(NativeAmxQcBuildError::SignerNotInValidatorSet);
        };
        if indexed_signatures
            .insert(index, vote.bls_signature.clone())
            .is_some()
        {
            return Err(NativeAmxQcBuildError::DuplicateSigner);
        }
    }

    if indexed_signatures.len() < min_signers {
        return Err(NativeAmxQcBuildError::QuorumNotMet);
    }

    let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    let ordered_signatures = indexed_signatures
        .into_iter()
        .map(|(index, signature)| {
            signers_bitmap[index / 8] |= 1_u8 << (index % 8);
            signature
        })
        .collect::<Vec<_>>();
    let signature_refs = ordered_signatures
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let bls_aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .map_err(|_| NativeAmxQcBuildError::SignatureAggregate)?;

    Ok(NativeAmxAttestationQcV1 {
        body,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        signers_bitmap,
        bls_aggregate_signature,
    })
}

#[derive(Default)]
struct NativeAmxSession {
    votes: BTreeMap<NativeAmxVoteBucket, BTreeMap<PeerId, NativeAmxVoteV1>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NativeAmxVoteBucket {
    body: NativeAmxAttestationBodyV1,
}

impl NativeAmxVoteBucket {
    const fn from_body(body: &NativeAmxAttestationBodyV1) -> Self {
        Self { body: *body }
    }
}

impl NativeAmxSession {
    fn insert_vote(&mut self, vote: NativeAmxVoteV1) -> Result<(), NativeAmxSessionError> {
        let target = self
            .votes
            .entry(NativeAmxVoteBucket::from_body(&vote.body))
            .or_default();
        if target.contains_key(&vote.signer) {
            return Err(NativeAmxSessionError::DuplicateSigner);
        }
        target.insert(vote.signer.clone(), vote);
        Ok(())
    }

    fn votes_for_body(&self, body: &NativeAmxAttestationBodyV1) -> Vec<NativeAmxVoteV1> {
        self.votes
            .get(&NativeAmxVoteBucket::from_body(body))
            .map(|source| source.values().cloned().collect())
            .unwrap_or_default()
    }
}

/// Bounded cache of native AMX vote sessions keyed by source transaction and plan digest.
pub struct NativeAmxSessionCache {
    max_sessions: NonZeroUsize,
    order: VecDeque<NativeAmxSessionKey>,
    sessions: BTreeMap<NativeAmxSessionKey, NativeAmxSession>,
}

impl NativeAmxSessionCache {
    /// Create a bounded native AMX session cache.
    #[must_use]
    pub fn new(max_sessions: NonZeroUsize) -> Self {
        Self {
            max_sessions,
            order: VecDeque::new(),
            sessions: BTreeMap::new(),
        }
    }

    /// Insert a vote, rejecting duplicate signers for the same exact attestation body.
    ///
    /// Eviction is deterministic FIFO by session key insertion order.
    ///
    /// # Errors
    /// Returns [`NativeAmxSessionError::DuplicateSigner`] when a signer votes twice for one body.
    pub fn insert_vote(&mut self, vote: NativeAmxVoteV1) -> Result<(), NativeAmxSessionError> {
        let key = NativeAmxSessionKey::from_body(&vote.body);
        if !self.sessions.contains_key(&key) {
            while self.sessions.len() >= self.max_sessions.get() {
                let Some(oldest) = self.order.pop_front() else {
                    break;
                };
                self.sessions.remove(&oldest);
            }
            self.order.push_back(key);
        }
        self.sessions.entry(key).or_default().insert_vote(vote)
    }

    /// Return votes sorted deterministically by signer id for a session phase.
    #[must_use]
    pub fn sorted_votes(
        &self,
        key: NativeAmxSessionKey,
        phase: NativeAmxPhase,
    ) -> Vec<NativeAmxVoteV1> {
        self.sessions
            .get(&key)
            .map(|session| {
                session
                    .votes
                    .iter()
                    .filter(|(bucket, _)| bucket.body.phase == phase)
                    .flat_map(|(_, votes)| votes.values().cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return votes sorted deterministically by signer id for an exact participant body.
    #[must_use]
    pub fn sorted_votes_for_body(
        &self,
        key: NativeAmxSessionKey,
        body: &NativeAmxAttestationBodyV1,
    ) -> Vec<NativeAmxVoteV1> {
        self.sessions
            .get(&key)
            .map(|session| session.votes_for_body(body))
            .unwrap_or_default()
    }

    /// Return exact-body votes restricted to the validator set used for QC assembly.
    #[must_use]
    pub fn sorted_votes_for_body_from(
        &self,
        key: NativeAmxSessionKey,
        body: &NativeAmxAttestationBodyV1,
        validator_set: &[PeerId],
    ) -> Vec<NativeAmxVoteV1> {
        self.sorted_votes_for_body(key, body)
            .into_iter()
            .filter(|vote| {
                validator_set
                    .iter()
                    .any(|validator| validator == &vote.signer)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        block::consensus::NativeAmxAttestationBodyV1,
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::TransactionEntrypoint,
    };

    use super::*;

    fn checked_random_ed25519_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("generate checked native AMX fixture keypair")
    }

    fn checked_bls_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("generate checked native AMX BLS fixture keypair")
    }

    fn checked_bls_signature_payload(keypair: &KeyPair, message: &[u8]) -> Vec<u8> {
        let signature = Signature::try_new(keypair.private_key(), message)
            .expect("checked native AMX vote fixture signature");
        signature
            .verify(keypair.public_key(), message)
            .expect("checked native AMX vote fixture signature verifies");
        signature.payload().to_vec()
    }

    fn body(phase: NativeAmxPhase) -> NativeAmxAttestationBodyV1 {
        NativeAmxAttestationBodyV1 {
            source_id: [0xAB; iroha_crypto::Hash::LENGTH],
            tx_entrypoint_hash:
                iroha_crypto::HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                    Hash::prehashed([0xCD; iroha_crypto::Hash::LENGTH]),
                ),
            plan_digest: Hash::new(b"native-amx-plan"),
            phase,
            coordinator_lane_id: LaneId::new(1),
            coordinator_dataspace_id: DataSpaceId::new(7),
            participant_lane_id: LaneId::new(2),
            participant_dataspace_id: DataSpaceId::new(8),
            planned_coordinator_block_height: 42,
        }
    }

    fn vote(phase: NativeAmxPhase) -> NativeAmxVoteV1 {
        let keypair = checked_random_ed25519_keypair();
        NativeAmxVoteV1 {
            body: body(phase),
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: vec![0xA5; 96],
        }
    }

    #[test]
    fn session_cache_rejects_duplicate_signer() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        cache
            .insert_vote(vote.clone())
            .expect("first vote should insert");
        assert!(matches!(
            cache.insert_vote(vote),
            Err(NativeAmxSessionError::DuplicateSigner)
        ));
    }

    #[test]
    fn session_cache_allows_same_signer_for_retried_body() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&vote.body);
        let mut retried_vote = vote.clone();
        retried_vote.body.planned_coordinator_block_height = retried_vote
            .body
            .planned_coordinator_block_height
            .saturating_add(1);

        cache.insert_vote(vote.clone()).expect("first body vote");
        cache
            .insert_vote(retried_vote.clone())
            .expect("same signer may vote on a retried body");

        assert_eq!(cache.sorted_votes_for_body(key, &vote.body), vec![vote]);
        assert_eq!(
            cache.sorted_votes_for_body(key, &retried_vote.body),
            vec![retried_vote]
        );
        assert_eq!(cache.sorted_votes(key, NativeAmxPhase::Prepare).len(), 2);
    }

    #[test]
    fn session_cache_allows_same_signer_for_different_participant_legs() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&vote.body);
        let mut other_leg = vote.clone();
        other_leg.body.participant_lane_id = LaneId::new(9);
        other_leg.body.participant_dataspace_id = DataSpaceId::new(10);

        cache.insert_vote(vote.clone()).expect("first leg vote");
        cache
            .insert_vote(other_leg.clone())
            .expect("same signer may vote on another participant leg");

        assert_eq!(cache.sorted_votes_for_body(key, &vote.body), vec![vote]);
        assert_eq!(
            cache.sorted_votes_for_body(key, &other_leg.body),
            vec![other_leg]
        );
    }

    #[test]
    fn session_cache_filters_exact_body_votes_to_validator_set() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let allowed_keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked allowed native AMX BLS fixture keypair");
        let unknown_keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked unknown native AMX BLS fixture keypair");
        let allowed = PeerId::new(allowed_keypair.public_key().clone());
        let unknown = PeerId::new(unknown_keypair.public_key().clone());
        let body = body(NativeAmxPhase::Prepare);
        let allowed_vote = NativeAmxVoteV1 {
            body,
            signer: allowed.clone(),
            bls_signature: vec![1],
        };
        let unknown_vote = NativeAmxVoteV1 {
            body,
            signer: unknown,
            bls_signature: vec![2],
        };
        let key = NativeAmxSessionKey::from_body(&body);

        cache
            .insert_vote(allowed_vote.clone())
            .expect("allowed signer vote");
        cache
            .insert_vote(unknown_vote)
            .expect("unknown signer vote");

        assert_eq!(
            cache.sorted_votes_for_body_from(key, &body, &[allowed]),
            vec![allowed_vote]
        );
    }

    #[test]
    fn session_cache_eviction_is_fifo() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(1).expect("nonzero"));
        let first = vote(NativeAmxPhase::Prepare);
        let first_key = NativeAmxSessionKey::from_body(&first.body);
        cache.insert_vote(first).expect("first vote");

        let mut second = vote(NativeAmxPhase::Prepare);
        second.body.source_id = [0xAC; iroha_crypto::Hash::LENGTH];
        let second_key = NativeAmxSessionKey::from_body(&second.body);
        cache.insert_vote(second).expect("second vote");

        assert!(
            cache
                .sorted_votes(first_key, NativeAmxPhase::Prepare)
                .is_empty()
        );
        assert_eq!(
            cache
                .sorted_votes(second_key, NativeAmxPhase::Prepare)
                .len(),
            1
        );
    }

    fn signed_vote(body: &NativeAmxAttestationBodyV1, keypair: &KeyPair) -> NativeAmxVoteV1 {
        NativeAmxVoteV1 {
            body: body.clone(),
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: checked_bls_signature_payload(keypair, &body.signature_preimage()),
        }
    }

    #[test]
    fn aggregate_votes_to_qc_orders_votes_by_validator_set() {
        let keypairs = [
            checked_bls_keypair(0xA1),
            checked_bls_keypair(0xB2),
            checked_bls_keypair(0xC3),
        ];
        let validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        let body = body(NativeAmxPhase::Commit);
        let votes = vec![
            signed_vote(&body, &keypairs[2]),
            signed_vote(&body, &keypairs[0]),
        ];

        let qc = aggregate_votes_to_qc(body.clone(), validator_set.clone(), &votes, 2)
            .expect("valid quorum should aggregate");

        assert_eq!(qc.body, body);
        assert_eq!(qc.validator_set, validator_set);
        assert_eq!(qc.signers_bitmap, vec![0b0000_0101]);
        let individual_signatures = [
            signed_vote(&body, &keypairs[0]).bls_signature,
            signed_vote(&body, &keypairs[2]).bls_signature,
        ];
        let signature_refs = individual_signatures
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let expected_aggregate = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate reference signatures");
        assert_eq!(qc.bls_aggregate_signature, expected_aggregate);
    }

    #[test]
    fn aggregate_votes_to_qc_rejects_bad_vote_sets() {
        let keypairs = [checked_bls_keypair(0xD1), checked_bls_keypair(0xD2)];
        let validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        let body = body(NativeAmxPhase::Prepare);
        let vote = signed_vote(&body, &keypairs[0]);

        assert_eq!(
            aggregate_votes_to_qc(body.clone(), validator_set.clone(), &[], 1),
            Err(NativeAmxQcBuildError::EmptyVotes)
        );
        assert_eq!(
            aggregate_votes_to_qc(body.clone(), validator_set.clone(), &[vote.clone()], 2),
            Err(NativeAmxQcBuildError::QuorumNotMet)
        );
        assert_eq!(
            aggregate_votes_to_qc(
                body.clone(),
                validator_set.clone(),
                &[vote.clone(), vote.clone()],
                1
            ),
            Err(NativeAmxQcBuildError::DuplicateSigner)
        );

        let outsider = checked_bls_keypair(0xD3);
        assert_eq!(
            aggregate_votes_to_qc(
                body.clone(),
                validator_set.clone(),
                &[signed_vote(&body, &outsider)],
                1
            ),
            Err(NativeAmxQcBuildError::SignerNotInValidatorSet)
        );

        let mut wrong_body_vote = vote;
        wrong_body_vote.body.phase = NativeAmxPhase::Commit;
        assert_eq!(
            aggregate_votes_to_qc(body, validator_set, &[wrong_body_vote], 1),
            Err(NativeAmxQcBuildError::BodyMismatch)
        );
    }
}
