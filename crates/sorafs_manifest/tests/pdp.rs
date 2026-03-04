use sorafs_manifest::{
    ChunkingProfileV1, ProfileId,
    pdp::{
        HashAlgorithmV1, PDP_CHALLENGE_VERSION_V1, PDP_COMMITMENT_VERSION_V1, PDP_PROOF_VERSION_V1,
        PdpChallengeV1, PdpChallengeValidationError, PdpCommitmentV1, PdpCommitmentValidationError,
        PdpHotLeafProofV1, PdpProofLeafV1, PdpProofV1, PdpProofValidationError, PdpSampleV1,
    },
};

fn sample_profile() -> ChunkingProfileV1 {
    ChunkingProfileV1 {
        profile_id: ProfileId(7),
        namespace: "sorafs".into(),
        name: "sf1".into(),
        semver: "1.0.0".into(),
        min_size: 4 * 1024,
        target_size: 256 * 1024,
        max_size: 256 * 1024,
        break_mask: 0,
        multihash_code: sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
        aliases: vec!["sorafs.sf1@1.0.0".into()],
    }
}

fn sample_commitment() -> PdpCommitmentV1 {
    PdpCommitmentV1 {
        version: PDP_COMMITMENT_VERSION_V1,
        manifest_digest: [0x11; 32],
        chunk_profile: sample_profile(),
        commitment_root_hot: [0x22; 32],
        commitment_root_segment: [0x33; 32],
        hash_algorithm: HashAlgorithmV1::Blake3_256,
        hot_tree_height: 8,
        segment_tree_height: 6,
        sample_window: 64,
        sealed_at: 1_700_000_000,
    }
}

fn sample_challenge() -> PdpChallengeV1 {
    PdpChallengeV1 {
        version: PDP_CHALLENGE_VERSION_V1,
        challenge_id: [0xAA; 32],
        manifest_digest: [0xBB; 32],
        provider_id: [0xCC; 32],
        chunk_profile: sample_profile(),
        seed: [0xDD; 32],
        epoch_id: 9,
        drand_round: 42,
        response_deadline_unix: 1_700_000_360_000,
        samples: vec![PdpSampleV1 {
            segment_index: 5,
            hot_leaf_indices: vec![0, 3, 7],
            segment_leaf_hash: [0x44; 32],
        }],
    }
}

fn sample_proof() -> PdpProofV1 {
    PdpProofV1 {
        version: PDP_PROOF_VERSION_V1,
        challenge_id: [0xAA; 32],
        manifest_digest: [0xBB; 32],
        provider_id: [0xCC; 32],
        epoch_id: 9,
        proof_leaves: vec![PdpProofLeafV1 {
            segment_index: 5,
            segment_hash: [0x44; 32],
            segment_merkle_path: vec![[0x55; 32]],
            hot_leaves: vec![PdpHotLeafProofV1 {
                leaf_index: 0,
                leaf_hash: [0x66; 32],
                leaf_merkle_path: vec![[0x77; 32]],
            }],
        }],
        signature: vec![0x99; 64],
        issued_at_unix: 1_700_000_050_000,
    }
}

#[test]
fn commitment_validation_succeeds() {
    sample_commitment().validate().expect("commitment valid");
}

#[test]
fn commitment_invalid_manifest_digest() {
    let mut commitment = sample_commitment();
    commitment.manifest_digest = [0u8; 32];
    let err = commitment.validate().expect_err("must fail");
    assert_eq!(err, PdpCommitmentValidationError::InvalidManifestDigest);
}

#[test]
fn challenge_validation_succeeds() {
    sample_challenge().validate().expect("challenge valid");
}

#[test]
fn challenge_detects_duplicate_hot_leaves() {
    let mut challenge = sample_challenge();
    challenge.samples[0].hot_leaf_indices = vec![1, 1];
    let err = challenge.validate().expect_err("must fail");
    assert!(matches!(
        err,
        PdpChallengeValidationError::DuplicateHotLeafIndex { segment_index: 5 }
    ));
}

#[test]
fn proof_validation_succeeds() {
    sample_proof().validate().expect("proof valid");
}

#[test]
fn proof_missing_signature_fails() {
    let mut proof = sample_proof();
    proof.signature.clear();
    let err = proof.validate().expect_err("must fail");
    assert_eq!(err, PdpProofValidationError::MissingSignature);
}
