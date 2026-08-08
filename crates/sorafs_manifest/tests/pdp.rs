//! PDP manifest structure and reference binding validation tests.

use ed25519_dalek::SigningKey;
use sorafs_manifest::{
    ChunkingProfileV1, ProfileId,
    pdp::{
        HashAlgorithmV1, PDP_MAX_MERKLE_PATH_DEPTH_V1, PDP_PROOF_VERSION_V1, PdpChallengeV1,
        PdpChallengeValidationError, PdpCommitmentV1, PdpCommitmentValidationError,
        PdpEd25519SignatureV1, PdpMerkleTreeV1, PdpProofV1, PdpProofValidationError, PdpSampleV1,
        PdpSignatureVerificationError, sign_pdp_proof_ed25519_v1,
    },
    validate_pdp_challenge_proof_bytes,
};

const MANIFEST_DIGEST: [u8; 32] = [0x11; 32];
const PROVIDER_ID: [u8; 32] = [0x22; 32];

struct Fixture {
    commitment: PdpCommitmentV1,
    challenge: PdpChallengeV1,
    proof: PdpProofV1,
    signing_key: SigningKey,
}

fn sample_profile() -> ChunkingProfileV1 {
    let descriptor = sorafs_manifest::chunker_registry::lookup(ProfileId(1))
        .expect("canonical SF1 profile exists");
    ChunkingProfileV1::from_descriptor(descriptor)
}

fn deterministic_payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((index.wrapping_mul(131).wrapping_add(17)) % 251) as u8)
        .collect()
}

fn fixture() -> Fixture {
    let payload = deterministic_payload(
        sorafs_manifest::pdp::PDP_SEGMENT_SIZE_V1 as usize
            + sorafs_manifest::pdp::PDP_HOT_LEAF_SIZE_V1 as usize
            + 37,
    );
    let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("build canonical PDP tree");
    let commitment = PdpCommitmentV1::from_tree(&tree, MANIFEST_DIGEST, sample_profile(), 4, 100)
        .expect("build commitment");
    let challenge = PdpChallengeV1::new(
        commitment.commitment_digest().expect("commitment digest"),
        MANIFEST_DIGEST,
        PROVIDER_ID,
        sample_profile(),
        [0x33; 32],
        9,
        42,
        200,
        300,
        vec![
            PdpSampleV1 {
                segment_index: 0,
                hot_leaf_indices: vec![0, 63],
            },
            PdpSampleV1 {
                segment_index: 1,
                hot_leaf_indices: vec![0, 1],
            },
        ],
    )
    .expect("build challenge");
    let signing_key = SigningKey::from_bytes(&[0x44; 32]);
    let proof = sign_pdp_proof_ed25519_v1(
        PdpProofV1 {
            version: PDP_PROOF_VERSION_V1,
            commitment_digest: challenge.commitment_digest,
            challenge_id: challenge.challenge_id,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            epoch_id: challenge.epoch_id,
            proof_leaves: tree
                .prove_samples(&challenge.samples, &payload)
                .expect("construct witnesses"),
            issued_at_unix: 250,
            signature: PdpEd25519SignatureV1 {
                public_key: [0; 32],
                signature: [0; 64],
            },
        },
        &signing_key,
    )
    .expect("sign proof");
    Fixture {
        commitment,
        challenge,
        proof,
        signing_key,
    }
}

fn resign(proof: &mut PdpProofV1, signing_key: &SigningKey) {
    *proof = sign_pdp_proof_ed25519_v1(proof.clone(), signing_key).expect("re-sign proof");
}

#[test]
fn commitment_validation_succeeds() {
    fixture().commitment.validate().expect("commitment valid");
}

#[test]
fn fresh_commitment_roundtrip_preserves_explicit_hash_algorithm_tag() {
    let commitment = fixture().commitment;
    let bytes = norito::to_bytes(&commitment).expect("encode fresh commitment");
    let decoded: PdpCommitmentV1 =
        norito::decode_from_bytes(&bytes).expect("decode fresh commitment");
    assert_eq!(decoded, commitment);

    let mut algorithm_payload = Vec::new();
    norito::core::serialize_to_buffer(&HashAlgorithmV1::Blake3_256, &mut algorithm_payload)
        .expect("encode bare hash algorithm");
    assert_eq!(algorithm_payload, 1_u32.to_le_bytes());
    assert_eq!(
        norito::core::decode_field_canonical::<HashAlgorithmV1>(&algorithm_payload)
            .expect("decode canonical hash algorithm")
            .0,
        HashAlgorithmV1::Blake3_256
    );
    assert!(
        norito::core::decode_field_canonical::<HashAlgorithmV1>(&0_u32.to_le_bytes()).is_err(),
        "legacy enum tag 0 must remain rejected"
    );
}

#[test]
fn commitment_invalid_manifest_digest() {
    let mut commitment = fixture().commitment;
    commitment.manifest_digest = [0; 32];
    assert_eq!(
        commitment.validate(),
        Err(PdpCommitmentValidationError::InvalidManifestDigest)
    );
}

#[test]
fn challenge_validation_succeeds() {
    fixture().challenge.validate().expect("challenge valid");
}

#[test]
fn challenge_detects_duplicate_hot_leaves() {
    let mut challenge = fixture().challenge;
    challenge.samples[0].hot_leaf_indices = vec![1, 1];
    assert_eq!(
        challenge.validate(),
        Err(PdpChallengeValidationError::NonCanonicalHotLeafOrder { segment_index: 0 })
    );
}

#[test]
fn proof_validation_succeeds() {
    fixture().proof.validate().expect("proof valid");
}

#[test]
fn proof_inert_signature_fails() {
    let mut proof = fixture().proof;
    proof.signature.public_key = [0; 32];
    assert!(matches!(
        proof.validate(),
        Err(PdpProofValidationError::InvalidSignature(
            PdpSignatureVerificationError::InvalidPublicKey { .. }
        ))
    ));
}

#[test]
fn proof_overdeep_segment_path_fails() {
    let mut proof = fixture().proof;
    proof.proof_leaves[0].segment_merkle_path = vec![[0x55; 32]; PDP_MAX_MERKLE_PATH_DEPTH_V1 + 1];
    assert!(matches!(
        proof.validate(),
        Err(PdpProofValidationError::MerklePathTooDeep {
            kind: "segment",
            segment_index: 0,
            leaf_index: None,
            ..
        })
    ));
}

#[test]
fn proof_leaf_byte_length_mismatch_fails() {
    let mut proof = fixture().proof;
    proof.proof_leaves[0].hot_leaves[0].leaf_bytes.pop();
    assert!(matches!(
        proof.validate(),
        Err(PdpProofValidationError::LeafByteLengthMismatch {
            segment_index: 0,
            leaf_index: 0,
            ..
        })
    ));
}

#[test]
fn challenge_proof_pair_rejects_late_proof() {
    let fixture = fixture();
    let mut proof = fixture.proof;
    proof.issued_at_unix = fixture.challenge.response_deadline_unix + 1;
    resign(&mut proof, &fixture.signing_key);

    let outcome = validate_pair(&fixture.challenge, &proof);
    assert_eq!(outcome.code, "SFS-POL-002");
    assert!(!outcome.is_ok(), "{outcome:?}");
}

#[test]
fn challenge_proof_pair_rejects_wrong_provider() {
    let fixture = fixture();
    let mut proof = fixture.proof;
    proof.provider_id = [0x88; 32];
    resign(&mut proof, &fixture.signing_key);

    let outcome = validate_pair(&fixture.challenge, &proof);
    assert_eq!(outcome.code, "SFS-PDP-003");
    assert!(!outcome.is_ok(), "{outcome:?}");
}

#[test]
fn challenge_proof_pair_rejects_wrong_manifest() {
    let fixture = fixture();
    let mut proof = fixture.proof;
    proof.manifest_digest = [0x77; 32];
    resign(&mut proof, &fixture.signing_key);

    let outcome = validate_pair(&fixture.challenge, &proof);
    assert_eq!(outcome.code, "SFS-PDP-003");
    assert!(!outcome.is_ok(), "{outcome:?}");
}

#[test]
fn challenge_proof_pair_rejects_witness_coverage_omission() {
    let fixture = fixture();
    let mut proof = fixture.proof;
    proof.proof_leaves[0].hot_leaves.pop();
    resign(&mut proof, &fixture.signing_key);

    let outcome = validate_pair(&fixture.challenge, &proof);
    assert_eq!(outcome.code, "SFS-PDP-001");
    assert!(!outcome.is_ok(), "{outcome:?}");
}

fn validate_pair(
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
) -> sorafs_manifest::reference::ValidationOutcomeV1 {
    validate_pdp_challenge_proof_bytes(
        &norito::to_bytes(challenge).expect("challenge encodes"),
        &norito::to_bytes(proof).expect("proof encodes"),
        "challenge.to",
        "proof.to",
        123,
    )
}
