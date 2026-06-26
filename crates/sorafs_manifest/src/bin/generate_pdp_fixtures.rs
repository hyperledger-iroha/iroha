//! Generates deterministic PDP commitment, challenge, and proof fixtures.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use hex::encode;
use norito::{
    core::NoritoSerialize,
    json::{Map, Value, to_string_pretty},
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, ProfileId,
    pdp::{
        HashAlgorithmV1, PDP_CHALLENGE_VERSION_V1, PDP_COMMITMENT_VERSION_V1, PDP_PROOF_VERSION_V1,
        PdpChallengeV1, PdpCommitmentV1, PdpHotLeafProofV1, PdpProofLeafV1, PdpProofV1,
        PdpSampleV1,
    },
};

fn main() -> Result<(), Box<dyn Error>> {
    let fixture_dir = PathBuf::from("fixtures/sorafs_manifest/pdp");
    let negative_dir = fixture_dir.join("negative");
    fs::create_dir_all(&fixture_dir)?;
    fs::create_dir_all(&negative_dir)?;

    let manifest_digest = [0x42; 32];
    let provider_id = [0x10; 32];
    let chunk_profile = chunk_profile();
    let segment_one_hash = digest("sorafs.pdp.segment.1");
    let segment_two_hash = digest("sorafs.pdp.segment.2048");

    let commitment = PdpCommitmentV1 {
        version: PDP_COMMITMENT_VERSION_V1,
        manifest_digest,
        chunk_profile: chunk_profile.clone(),
        commitment_root_hot: digest("sorafs.pdp.commitment.hot.root"),
        commitment_root_segment: digest("sorafs.pdp.commitment.segment.root"),
        hash_algorithm: HashAlgorithmV1::Blake3_256,
        hot_tree_height: 8,
        segment_tree_height: 6,
        sample_window: 4,
        sealed_at: 1_700_000_000,
    };
    commitment.validate()?;

    let challenge = PdpChallengeV1 {
        version: PDP_CHALLENGE_VERSION_V1,
        challenge_id: digest("sorafs.pdp.challenge.v1"),
        manifest_digest,
        provider_id,
        chunk_profile,
        seed: digest("sorafs.pdp.challenge.seed"),
        epoch_id: 1_700_000,
        drand_round: 5_432_101,
        response_deadline_unix: 1_700_000_360_000,
        samples: vec![
            PdpSampleV1 {
                segment_index: 1,
                hot_leaf_indices: vec![0, 3, 7],
                segment_leaf_hash: segment_one_hash,
            },
            PdpSampleV1 {
                segment_index: 2_048,
                hot_leaf_indices: vec![1, 2],
                segment_leaf_hash: segment_two_hash,
            },
        ],
    };
    challenge.validate()?;

    let proof = PdpProofV1 {
        version: PDP_PROOF_VERSION_V1,
        challenge_id: challenge.challenge_id,
        manifest_digest: challenge.manifest_digest,
        provider_id: challenge.provider_id,
        epoch_id: challenge.epoch_id,
        proof_leaves: vec![
            PdpProofLeafV1 {
                segment_index: 1,
                segment_hash: segment_one_hash,
                segment_merkle_path: vec![
                    digest("sorafs.pdp.segment.1.path.0"),
                    digest("sorafs.pdp.segment.1.path.1"),
                ],
                hot_leaves: vec![
                    hot_leaf(0, "sorafs.pdp.segment.1.leaf.0"),
                    hot_leaf(3, "sorafs.pdp.segment.1.leaf.3"),
                    hot_leaf(7, "sorafs.pdp.segment.1.leaf.7"),
                ],
            },
            PdpProofLeafV1 {
                segment_index: 2_048,
                segment_hash: segment_two_hash,
                segment_merkle_path: vec![
                    digest("sorafs.pdp.segment.2048.path.0"),
                    digest("sorafs.pdp.segment.2048.path.1"),
                ],
                hot_leaves: vec![
                    hot_leaf(1, "sorafs.pdp.segment.2048.leaf.1"),
                    hot_leaf(2, "sorafs.pdp.segment.2048.leaf.2"),
                ],
            },
        ],
        signature: vec![0x99; 64],
        issued_at_unix: 1_700_000_050_000,
    };
    proof.validate()?;

    write_norito_pair(
        &fixture_dir.join("commitment_v1"),
        &commitment,
        commitment_json(&commitment),
    )?;
    write_norito_pair(
        &fixture_dir.join("challenge_v1"),
        &challenge,
        challenge_json(&challenge),
    )?;
    write_norito_pair(&fixture_dir.join("proof_v1"), &proof, proof_json(&proof))?;

    let mut duplicate_hot_leaf_challenge = challenge.clone();
    duplicate_hot_leaf_challenge.samples[0].hot_leaf_indices = vec![0, 0];
    assert!(duplicate_hot_leaf_challenge.validate().is_err());
    write_norito_pair(
        &negative_dir.join("duplicate_hot_leaf_challenge_v1"),
        &duplicate_hot_leaf_challenge,
        challenge_json(&duplicate_hot_leaf_challenge),
    )?;

    let mut missing_signature_proof = proof.clone();
    missing_signature_proof.signature.clear();
    assert!(missing_signature_proof.validate().is_err());
    write_norito_pair(
        &negative_dir.join("missing_signature_proof_v1"),
        &missing_signature_proof,
        proof_json(&missing_signature_proof),
    )?;

    let mut missing_segment_path_proof = proof.clone();
    missing_segment_path_proof.proof_leaves[0]
        .segment_merkle_path
        .clear();
    assert!(missing_segment_path_proof.validate().is_err());
    write_norito_pair(
        &negative_dir.join("missing_segment_path_proof_v1"),
        &missing_segment_path_proof,
        proof_json(&missing_segment_path_proof),
    )?;

    let mut missing_hot_leaf_path_proof = proof.clone();
    missing_hot_leaf_path_proof.proof_leaves[0].hot_leaves[0]
        .leaf_merkle_path
        .clear();
    assert!(missing_hot_leaf_path_proof.validate().is_err());
    write_norito_pair(
        &negative_dir.join("missing_hot_leaf_path_proof_v1"),
        &missing_hot_leaf_path_proof,
        proof_json(&missing_hot_leaf_path_proof),
    )?;

    let mut late_proof = proof.clone();
    late_proof.issued_at_unix = challenge.response_deadline_unix + 1;
    late_proof.validate()?;
    write_norito_pair(
        &negative_dir.join("late_proof_v1"),
        &late_proof,
        proof_json(&late_proof),
    )?;

    let mut wrong_provider_proof = proof.clone();
    wrong_provider_proof.provider_id = [0x88; 32];
    wrong_provider_proof.validate()?;
    write_norito_pair(
        &negative_dir.join("wrong_provider_proof_v1"),
        &wrong_provider_proof,
        proof_json(&wrong_provider_proof),
    )?;

    let mut wrong_manifest_proof = proof.clone();
    wrong_manifest_proof.manifest_digest = [0x77; 32];
    wrong_manifest_proof.validate()?;
    write_norito_pair(
        &negative_dir.join("wrong_manifest_proof_v1"),
        &wrong_manifest_proof,
        proof_json(&wrong_manifest_proof),
    )?;

    let mut wrong_path_proof = proof;
    wrong_path_proof.proof_leaves[0].segment_hash = digest("sorafs.pdp.segment.1.wrong");
    wrong_path_proof.validate()?;
    write_norito_pair(
        &negative_dir.join("wrong_path_proof_v1"),
        &wrong_path_proof,
        proof_json(&wrong_path_proof),
    )?;

    Ok(())
}

fn chunk_profile() -> ChunkingProfileV1 {
    ChunkingProfileV1 {
        profile_id: ProfileId(7),
        namespace: "sorafs".to_owned(),
        name: "sf1".to_owned(),
        semver: "1.0.0".to_owned(),
        min_size: 4 * 1024,
        target_size: 256 * 1024,
        max_size: 256 * 1024,
        break_mask: 0,
        multihash_code: BLAKE3_256_MULTIHASH_CODE,
        aliases: vec!["sorafs.sf1@1.0.0".to_owned()],
    }
}

fn digest(label: &str) -> [u8; 32] {
    *blake3::hash(label.as_bytes()).as_bytes()
}

fn hot_leaf(leaf_index: u32, label: &str) -> PdpHotLeafProofV1 {
    PdpHotLeafProofV1 {
        leaf_index,
        leaf_hash: digest(label),
        leaf_merkle_path: vec![
            digest(&format!("{label}.path.0")),
            digest(&format!("{label}.path.1")),
        ],
    }
}

fn write_norito_pair<T>(
    base_path: &Path,
    value: &T,
    mut json_value: Value,
) -> Result<(), Box<dyn Error>>
where
    T: NoritoSerialize,
{
    let bytes = norito::to_bytes(value)?;
    fs::write(base_path.with_extension("to"), &bytes)?;
    if let Value::Object(map) = &mut json_value {
        map.insert("norito_bytes_hex".to_owned(), Value::from(encode(&bytes)));
    }
    let json = to_string_pretty(&json_value)?;
    fs::write(base_path.with_extension("json"), json)?;
    Ok(())
}

fn commitment_json(commitment: &PdpCommitmentV1) -> Value {
    let mut map = Map::new();
    map.insert("version".to_owned(), Value::from(commitment.version));
    map.insert(
        "manifest_digest_hex".to_owned(),
        Value::from(encode(commitment.manifest_digest)),
    );
    map.insert(
        "chunk_profile".to_owned(),
        chunk_profile_json(&commitment.chunk_profile),
    );
    map.insert(
        "commitment_root_hot_hex".to_owned(),
        Value::from(encode(commitment.commitment_root_hot)),
    );
    map.insert(
        "commitment_root_segment_hex".to_owned(),
        Value::from(encode(commitment.commitment_root_segment)),
    );
    map.insert(
        "hash_algorithm".to_owned(),
        Value::from(commitment.hash_algorithm.as_str()),
    );
    map.insert(
        "hot_tree_height".to_owned(),
        Value::from(commitment.hot_tree_height),
    );
    map.insert(
        "segment_tree_height".to_owned(),
        Value::from(commitment.segment_tree_height),
    );
    map.insert(
        "sample_window".to_owned(),
        Value::from(commitment.sample_window),
    );
    map.insert("sealed_at".to_owned(), Value::from(commitment.sealed_at));
    Value::Object(map)
}

fn challenge_json(challenge: &PdpChallengeV1) -> Value {
    let mut map = Map::new();
    map.insert("version".to_owned(), Value::from(challenge.version));
    map.insert(
        "challenge_id_hex".to_owned(),
        Value::from(encode(challenge.challenge_id)),
    );
    map.insert(
        "manifest_digest_hex".to_owned(),
        Value::from(encode(challenge.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".to_owned(),
        Value::from(encode(challenge.provider_id)),
    );
    map.insert(
        "chunk_profile".to_owned(),
        chunk_profile_json(&challenge.chunk_profile),
    );
    map.insert("seed_hex".to_owned(), Value::from(encode(challenge.seed)));
    map.insert("epoch_id".to_owned(), Value::from(challenge.epoch_id));
    map.insert("drand_round".to_owned(), Value::from(challenge.drand_round));
    map.insert(
        "response_deadline_unix".to_owned(),
        Value::from(challenge.response_deadline_unix),
    );
    map.insert(
        "samples".to_owned(),
        Value::Array(
            challenge
                .samples
                .iter()
                .map(sample_json)
                .collect::<Vec<_>>(),
        ),
    );
    Value::Object(map)
}

fn proof_json(proof: &PdpProofV1) -> Value {
    let mut map = Map::new();
    map.insert("version".to_owned(), Value::from(proof.version));
    map.insert(
        "challenge_id_hex".to_owned(),
        Value::from(encode(proof.challenge_id)),
    );
    map.insert(
        "manifest_digest_hex".to_owned(),
        Value::from(encode(proof.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".to_owned(),
        Value::from(encode(proof.provider_id)),
    );
    map.insert("epoch_id".to_owned(), Value::from(proof.epoch_id));
    map.insert(
        "proof_leaves".to_owned(),
        Value::Array(proof.proof_leaves.iter().map(proof_leaf_json).collect()),
    );
    map.insert(
        "signature_hex".to_owned(),
        Value::from(encode(&proof.signature)),
    );
    map.insert(
        "issued_at_unix".to_owned(),
        Value::from(proof.issued_at_unix),
    );
    Value::Object(map)
}

fn chunk_profile_json(profile: &ChunkingProfileV1) -> Value {
    let mut map = Map::new();
    map.insert("profile_id".to_owned(), Value::from(profile.profile_id.0));
    map.insert(
        "namespace".to_owned(),
        Value::from(profile.namespace.clone()),
    );
    map.insert("name".to_owned(), Value::from(profile.name.clone()));
    map.insert("semver".to_owned(), Value::from(profile.semver.clone()));
    map.insert("min_size".to_owned(), Value::from(profile.min_size));
    map.insert("target_size".to_owned(), Value::from(profile.target_size));
    map.insert("max_size".to_owned(), Value::from(profile.max_size));
    map.insert("break_mask".to_owned(), Value::from(profile.break_mask));
    map.insert(
        "multihash_code".to_owned(),
        Value::from(profile.multihash_code),
    );
    map.insert(
        "aliases".to_owned(),
        Value::Array(
            profile
                .aliases
                .iter()
                .map(|alias| Value::from(alias.clone()))
                .collect(),
        ),
    );
    Value::Object(map)
}

fn sample_json(sample: &PdpSampleV1) -> Value {
    let mut map = Map::new();
    map.insert(
        "segment_index".to_owned(),
        Value::from(sample.segment_index),
    );
    map.insert(
        "hot_leaf_indices".to_owned(),
        Value::Array(
            sample
                .hot_leaf_indices
                .iter()
                .map(|index| Value::from(*index))
                .collect(),
        ),
    );
    map.insert(
        "segment_leaf_hash_hex".to_owned(),
        Value::from(encode(sample.segment_leaf_hash)),
    );
    Value::Object(map)
}

fn proof_leaf_json(leaf: &PdpProofLeafV1) -> Value {
    let mut map = Map::new();
    map.insert("segment_index".to_owned(), Value::from(leaf.segment_index));
    map.insert(
        "segment_hash_hex".to_owned(),
        Value::from(encode(leaf.segment_hash)),
    );
    map.insert(
        "segment_merkle_path_hex".to_owned(),
        Value::Array(
            leaf.segment_merkle_path
                .iter()
                .map(|node| Value::from(encode(node)))
                .collect(),
        ),
    );
    map.insert(
        "hot_leaves".to_owned(),
        Value::Array(leaf.hot_leaves.iter().map(hot_leaf_json).collect()),
    );
    Value::Object(map)
}

fn hot_leaf_json(leaf: &PdpHotLeafProofV1) -> Value {
    let mut map = Map::new();
    map.insert("leaf_index".to_owned(), Value::from(leaf.leaf_index));
    map.insert(
        "leaf_hash_hex".to_owned(),
        Value::from(encode(leaf.leaf_hash)),
    );
    map.insert(
        "leaf_merkle_path_hex".to_owned(),
        Value::Array(
            leaf.leaf_merkle_path
                .iter()
                .map(|node| Value::from(encode(node)))
                .collect(),
        ),
    );
    Value::Object(map)
}
