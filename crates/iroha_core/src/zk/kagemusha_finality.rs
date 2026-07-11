//! Native verifier for compact Kagemusha V2 top-up finality proofs.
//!
//! A spendable recursive note must not trust a Core API response as its source
//! of finality. This verifier binds the exact top-up `(operation_id,
//! anchor_digest)` write to a Commit QC signed by a content-addressed,
//! pre-fetched validator roster. The block-local Merkle tree is deliberately
//! bounded to keep the complete peer payload small and independent of unrelated
//! writes in the finalized block.

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    block::{BlockHeader, consensus::CertPhase},
    offline::{
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2, KagemushaTopUpFinalityProofV2,
        KagemushaTopUpFinalityRosterArtifactV2, KagemushaTopUpFinalityRosterWindowV2,
    },
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::sumeragi::{
    consensus::{PERMISSIONED_TAG, QcVote, vote_preimage},
    network_topology::commit_quorum_from_len,
    smt::{
        KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG, KagemushaTopUpMerkleProof, KvPair,
        verify_kagemusha_topup_write_inclusion,
    },
};

/// Failure returned by the native top-up finality verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KagemushaTopUpFinalityVerifyError {
    /// The proof, roster, bitmap, or activation window is non-canonical.
    #[error("invalid Kagemusha top-up finality structure")]
    InvalidStructure,
    /// The roster artifact is not the exact digest selected by the authenticated manifest.
    #[error("Kagemusha top-up finality roster digest mismatch")]
    ArtifactDigestMismatch,
    /// Only count-quorum permissioned consensus is supported by this artifact schema.
    #[error("unsupported Kagemusha top-up finality consensus mode")]
    UnsupportedConsensusMode,
    /// A raw Iroha hash does not carry the canonical marker bit.
    #[error("non-canonical Kagemusha top-up finality hash")]
    NonCanonicalHash,
    /// The signer bitmap does not contain the consensus commit quorum.
    #[error("insufficient Kagemusha top-up finality quorum")]
    InsufficientQuorum,
    /// The BLS aggregate does not authenticate the canonical Commit vote.
    #[error("invalid Kagemusha top-up finality aggregate signature")]
    InvalidAggregateSignature,
    /// The exact top-up anchor is not included in the QC-authenticated post root.
    #[error("invalid Kagemusha top-up anchor inclusion proof")]
    InvalidAnchorInclusion,
}

/// Verify one compact Kagemusha top-up finality proof against a trusted roster.
///
/// `expected_roster_sha256` comes from the already authenticated product
/// manifest. A human-readable generation label is not a trust anchor; the
/// exact canonical roster bytes must match this digest before any QC is
/// evaluated.
///
/// # Errors
///
/// Fails closed for malformed inputs, unsupported consensus modes, roster or
/// generation mismatch, insufficient quorum, invalid BLS signatures, and any
/// Merkle-path mutation.
pub fn verify_kagemusha_topup_finality_v2(
    proof: &KagemushaTopUpFinalityProofV2,
    roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
    expected_roster_sha256: [u8; 32],
) -> Result<(), KagemushaTopUpFinalityVerifyError> {
    proof
        .validate_structure()
        .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
    // Authenticate the cheap, bounded archive commitment before validating any
    // BLS proofs of possession. Otherwise an unauthenticated roster can force
    // thousands of pairings even though its manifest digest is already wrong.
    if expected_roster_sha256 == [0; 32] {
        return Err(KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch);
    }
    let roster_bytes = norito::to_bytes(roster_artifact)
        .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
    if u64::try_from(roster_bytes.len()).map_or(true, |length| {
        length > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2
    }) || <[u8; 32]>::from(Sha256::digest(&roster_bytes)) != expected_roster_sha256
    {
        return Err(KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch);
    }
    roster_artifact
        .validate()
        .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;

    let qc = &proof.commit_qc;
    if qc.mode_tag != PERMISSIONED_TAG {
        // NPoS needs a stake-snapshot commitment and weighted-quorum proof. The
        // V2 roster artifact intentionally does not pretend count quorum is
        // equivalent, so that mode remains unavailable until its typed wire is
        // added.
        return Err(KagemushaTopUpFinalityVerifyError::UnsupportedConsensusMode);
    }
    let mut matching = roster_artifact.windows.iter().filter(|window| {
        qc.height >= window.activates_at_height && qc.height < window.withdraws_at_height
    });
    let window = matching
        .next()
        .ok_or(KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
    if matching.next().is_some() {
        return Err(KagemushaTopUpFinalityVerifyError::InvalidStructure);
    }
    let signer_indices = signer_indices(&qc.signers_bitmap, window.validator_set.len())?;
    let required = commit_quorum_from_len(window.validator_set.len());
    if signer_indices.len() < required {
        return Err(KagemushaTopUpFinalityVerifyError::InsufficientQuorum);
    }
    qc.validate_for_roster_window(window)
        .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;

    verify_commit_aggregate(proof, roster_artifact, window, &signer_indices)?;
    verify_anchor_inclusion(proof)
}

fn verify_commit_aggregate(
    proof: &KagemushaTopUpFinalityProofV2,
    roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
    window: &KagemushaTopUpFinalityRosterWindowV2,
    signer_indices: &[usize],
) -> Result<(), KagemushaTopUpFinalityVerifyError> {
    let qc = &proof.commit_qc;

    let vote = QcVote {
        phase: CertPhase::Commit,
        block_hash: canonical_block_hash(qc.subject_block_hash)?,
        parent_state_root: canonical_hash(qc.parent_state_root)?,
        post_state_root: canonical_hash(qc.post_state_root)?,
        height: qc.height,
        view: qc.view,
        epoch: qc.epoch,
        chain_order_hash: canonical_hash(qc.chain_order_hash)?,
        rechain_seq: qc.rechain_seq,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage = vote_preimage(&roster_artifact.chain_id, &qc.mode_tag, &vote);
    let public_keys = signer_indices
        .iter()
        .map(|index| window.validator_set[*index].public_key())
        .collect::<Vec<&PublicKey>>();
    let pops = signer_indices
        .iter()
        .map(|index| window.validator_set_pops[*index].as_slice())
        .collect::<Vec<&[u8]>>();
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &qc.bls_aggregate_signature,
        &public_keys,
        &pops,
    )
    .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidAggregateSignature)
}

fn verify_anchor_inclusion(
    proof: &KagemushaTopUpFinalityProofV2,
) -> Result<(), KagemushaTopUpFinalityVerifyError> {
    let mut key = Vec::with_capacity(33);
    key.push(KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG);
    key.extend_from_slice(&proof.anchor.topup_operation_id);
    let leaf = KvPair::new(key, proof.anchor.anchor_digest);
    let path = KagemushaTopUpMerkleProof {
        leaf_index: proof.anchor_path.leaf_index,
        leaf_count: proof.anchor_path.leaf_count,
        siblings: proof
            .anchor_path
            .siblings
            .iter()
            .copied()
            .map(canonical_hash)
            .collect::<Result<Vec<_>, _>>()?,
    };
    let ordinary_writes_root = canonical_hash(proof.ordinary_writes_root)?;
    let post_state_root = canonical_hash(proof.commit_qc.post_state_root)?;
    if !verify_kagemusha_topup_write_inclusion(&leaf, &path, ordinary_writes_root, post_state_root)
    {
        return Err(KagemushaTopUpFinalityVerifyError::InvalidAnchorInclusion);
    }
    Ok(())
}

fn signer_indices(
    bitmap: &[u8],
    roster_len: usize,
) -> Result<Vec<usize>, KagemushaTopUpFinalityVerifyError> {
    if roster_len == 0 || bitmap.len() != roster_len.div_ceil(8) {
        return Err(KagemushaTopUpFinalityVerifyError::InvalidStructure);
    }
    let mut indices = Vec::new();
    for (byte_index, byte) in bitmap.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (1 << bit) == 0 {
                continue;
            }
            let index = byte_index * 8 + bit;
            if index >= roster_len {
                return Err(KagemushaTopUpFinalityVerifyError::InvalidStructure);
            }
            indices.push(index);
        }
    }
    Ok(indices)
}

fn canonical_hash(bytes: [u8; Hash::LENGTH]) -> Result<Hash, KagemushaTopUpFinalityVerifyError> {
    // `Hash::prehashed` sets this marker bit. Reject first so attacker input is
    // never normalized into a different signed or Merkle-authenticated value.
    if bytes[Hash::LENGTH - 1] & 1 == 0 {
        return Err(KagemushaTopUpFinalityVerifyError::NonCanonicalHash);
    }
    Ok(Hash::prehashed(bytes))
}

fn canonical_block_hash(
    bytes: [u8; Hash::LENGTH],
) -> Result<HashOf<BlockHeader>, KagemushaTopUpFinalityVerifyError> {
    Ok(HashOf::from_untyped_unchecked(canonical_hash(bytes)?))
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        ChainId, PeerId,
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        offline::{
            KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaTopUpAnchorMerkleProofV2,
            KagemushaTopUpFinalityCompactQcV2,
        },
    };

    use super::*;
    use crate::sumeragi::smt::build_kagemusha_topup_block_commitment;

    struct Fixture {
        proof: KagemushaTopUpFinalityProofV2,
        artifact: KagemushaTopUpFinalityRosterArtifactV2,
    }

    fn roster_digest(artifact: &KagemushaTopUpFinalityRosterArtifactV2) -> [u8; 32] {
        Sha256::digest(norito::to_bytes(artifact).expect("encode roster artifact")).into()
    }

    fn fixture(roster_len: usize, signer_count: usize) -> Fixture {
        let keys = (0..roster_len)
            .map(|_| {
                KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                    .expect("BLS fixture keypair")
            })
            .collect::<Vec<_>>();
        let validator_set = keys
            .iter()
            .map(|key| PeerId::from(key.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_set_pops = keys
            .iter()
            .map(|key| {
                let pop =
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("BLS fixture PoP");
                <[u8; 96]>::try_from(pop).expect("BLS proof of possession has canonical width")
            })
            .collect::<Vec<_>>();
        let validator_set_hash = HashOf::new(&validator_set);
        let operation_id = [0xA5; 32];
        let anchor_digest = [0x5B; 32];
        let mut witness_key = vec![KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG];
        witness_key.extend_from_slice(&operation_id);
        let commitment =
            build_kagemusha_topup_block_commitment(&[KvPair::new(witness_key, anchor_digest)])
                .expect("bounded commitment")
                .expect("top-up commitment");
        let block_hash = Hash::new(b"kagemusha-finality-block");
        let chain_order_hash = Hash::new(b"kagemusha-finality-roster-order");
        let parent_state_root = Hash::new(b"kagemusha-finality-parent");
        let chain_id = ChainId::from("kagemusha-finality-test-chain");
        let height = 42;
        let view = 3;
        let epoch = 0;
        let mut bitmap = vec![0_u8; roster_len.div_ceil(8)];
        for index in 0..signer_count {
            bitmap[index / 8] |= 1 << (index % 8);
        }
        let vote = QcVote {
            phase: CertPhase::Commit,
            block_hash: HashOf::from_untyped_unchecked(block_hash),
            parent_state_root,
            post_state_root: commitment.post_state_root,
            height,
            view,
            epoch,
            chain_order_hash,
            rechain_seq: 9,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let preimage = vote_preimage(&chain_id, PERMISSIONED_TAG, &vote);
        let signatures = keys
            .iter()
            .take(signer_count)
            .map(|key| {
                Signature::try_new(key.private_key(), &preimage)
                    .expect("BLS fixture vote signature")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let aggregate = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("BLS fixture aggregate");
        let path = &commitment.proofs[0];
        let proof = KagemushaTopUpFinalityProofV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
            anchor: KagemushaRecursiveSpendTopUpAnchorRefV2 {
                topup_operation_id: operation_id,
                anchor_digest,
            },
            commit_qc: KagemushaTopUpFinalityCompactQcV2 {
                subject_block_hash: *block_hash.as_ref(),
                parent_state_root: *parent_state_root.as_ref(),
                post_state_root: *commitment.post_state_root.as_ref(),
                height,
                view,
                epoch,
                chain_order_hash: *chain_order_hash.as_ref(),
                rechain_seq: 9,
                mode_tag: PERMISSIONED_TAG.to_owned(),
                validator_set_hash: *validator_set_hash.as_ref(),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                signers_bitmap: bitmap,
                bls_aggregate_signature: aggregate,
            },
            ordinary_writes_root: *commitment.ordinary_writes_root.as_ref(),
            anchor_path: KagemushaTopUpAnchorMerkleProofV2 {
                leaf_index: path.leaf_index,
                leaf_count: path.leaf_count,
                siblings: path.siblings.iter().map(|hash| *hash.as_ref()).collect(),
            },
        };
        let artifact = KagemushaTopUpFinalityRosterArtifactV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            chain_id,
            artifact_generation: "finality-roster-v1".to_owned(),
            windows: vec![KagemushaTopUpFinalityRosterWindowV2 {
                activates_at_height: 1,
                withdraws_at_height: 100,
                validator_set_hash: *validator_set_hash.as_ref(),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set,
                validator_set_pops,
            }],
        };
        Fixture { proof, artifact }
    }

    #[test]
    fn verifies_real_commit_qc_and_exact_anchor_path() {
        let fixture = fixture(5, 4);
        verify_kagemusha_topup_finality_v2(
            &fixture.proof,
            &fixture.artifact,
            roster_digest(&fixture.artifact),
        )
        .expect("valid finality proof");
    }

    #[test]
    fn rejects_under_quorum_for_five_and_six_validator_rosters() {
        for roster_len in [5, 6] {
            let fixture = fixture(roster_len, 3);
            assert_eq!(
                verify_kagemusha_topup_finality_v2(
                    &fixture.proof,
                    &fixture.artifact,
                    roster_digest(&fixture.artifact),
                ),
                Err(KagemushaTopUpFinalityVerifyError::InsufficientQuorum)
            );
        }
    }

    #[test]
    fn rejects_untrusted_roster_digest_before_bls_pop_validation() {
        let fixture = fixture(4, 3);
        let trusted_digest = roster_digest(&fixture.artifact);
        let mut attacker_roster = fixture.artifact.clone();
        attacker_roster.windows[0].validator_set_pops[0][0] ^= 0x80;

        assert_eq!(
            verify_kagemusha_topup_finality_v2(&fixture.proof, &attacker_roster, trusted_digest,),
            Err(KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch)
        );
        assert_eq!(
            verify_kagemusha_topup_finality_v2(&fixture.proof, &attacker_roster, [0; 32]),
            Err(KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch)
        );
    }

    #[test]
    fn rejects_signature_path_anchor_chain_and_roster_digest_mutations() {
        let fixture = fixture(4, 3);

        let mut bad_signature = fixture.proof.clone();
        bad_signature.commit_qc.bls_aggregate_signature[0] ^= 1;
        assert_eq!(
            verify_kagemusha_topup_finality_v2(
                &bad_signature,
                &fixture.artifact,
                roster_digest(&fixture.artifact),
            ),
            Err(KagemushaTopUpFinalityVerifyError::InvalidAggregateSignature)
        );

        let mut bad_anchor = fixture.proof.clone();
        bad_anchor.anchor.anchor_digest[0] ^= 1;
        assert_eq!(
            verify_kagemusha_topup_finality_v2(
                &bad_anchor,
                &fixture.artifact,
                roster_digest(&fixture.artifact),
            ),
            Err(KagemushaTopUpFinalityVerifyError::InvalidAnchorInclusion)
        );

        let mut wrong_chain = fixture.artifact.clone();
        wrong_chain.chain_id = ChainId::from("attacker-chain");
        assert_eq!(
            verify_kagemusha_topup_finality_v2(
                &fixture.proof,
                &wrong_chain,
                roster_digest(&fixture.artifact),
            ),
            Err(KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch)
        );

        assert_eq!(
            verify_kagemusha_topup_finality_v2(&fixture.proof, &fixture.artifact, [0xA5; 32],),
            Err(KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch)
        );
    }

    #[test]
    fn rejects_noncanonical_hash_without_normalizing_it() {
        let fixture = fixture(4, 3);
        let mut proof = fixture.proof.clone();
        proof.commit_qc.subject_block_hash[31] &= !1;
        assert_eq!(
            verify_kagemusha_topup_finality_v2(
                &proof,
                &fixture.artifact,
                roster_digest(&fixture.artifact),
            ),
            Err(KagemushaTopUpFinalityVerifyError::NonCanonicalHash)
        );
    }

    #[test]
    fn rejects_npos_until_weighted_snapshot_is_bound_by_the_artifact() {
        let fixture = fixture(4, 3);
        let mut proof = fixture.proof.clone();
        proof.commit_qc.mode_tag = crate::sumeragi::consensus::NPOS_TAG.to_owned();
        assert_eq!(
            verify_kagemusha_topup_finality_v2(
                &proof,
                &fixture.artifact,
                roster_digest(&fixture.artifact),
            ),
            Err(KagemushaTopUpFinalityVerifyError::UnsupportedConsensusMode)
        );
    }
}
