//! Generates PoR challenge/proof and governance log fixtures.

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
    CapacityMetadataEntry,
    governance::{
        GOVERNANCE_LOG_VERSION_V1, GovernanceLogNodeV1, GovernanceLogPayloadV1,
        GovernanceLogSignatureV1, GovernanceSignatureAlgorithm,
    },
    por::{
        AUDIT_VERDICT_VERSION_V1, AuditOutcomeV1, AuditVerdictV1, POR_CHALLENGE_VERSION_V1,
        POR_PROOF_VERSION_V1, PorChallengeV1, PorProofSampleV1, PorProofV1, derive_challenge_id,
        derive_challenge_seed,
    },
    provider_advert::{AdvertSignature, SignatureAlgorithm},
};

fn main() -> Result<(), Box<dyn Error>> {
    let por_dir = PathBuf::from("fixtures/sorafs_manifest/por");
    let gov_dir = PathBuf::from("fixtures/sorafs_manifest/governance");
    fs::create_dir_all(&por_dir)?;
    fs::create_dir_all(&gov_dir)?;

    let manifest_digest = [0x42; 32];
    let provider_id = [0x10; 32];
    let epoch_id = 1_700_000;
    let drand_round = 5_432_101;
    let drand_randomness = [0x21; 32];
    let vrf_output = [0x23; 32];
    let seed = derive_challenge_seed(
        &drand_randomness,
        Some(&vrf_output),
        &manifest_digest,
        epoch_id,
    );
    let challenge_id =
        derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);

    let challenge = PorChallengeV1 {
        version: POR_CHALLENGE_VERSION_V1,
        challenge_id,
        manifest_digest,
        provider_id,
        epoch_id,
        drand_round,
        drand_randomness,
        drand_signature: vec![0x24; 96],
        vrf_output: Some(vrf_output),
        vrf_proof: Some(vec![0x25; 80]),
        forced: false,
        chunking_profile: "sorafs.sf1@1.0.0".to_string(),
        seed,
        sample_tier: 2,
        sample_count: 3,
        sample_indices: vec![1, 2_048, 65_535],
        issued_at: 1_700_000_000,
        deadline_at: 1_700_000_900,
    };
    challenge.validate()?;

    let proof_samples = vec![
        PorProofSampleV1 {
            sample_index: 1,
            chunk_offset: 0,
            chunk_size: 65_536,
            chunk_digest: [0xAA; 32],
            leaf_digest: [0xBB; 32],
        },
        PorProofSampleV1 {
            sample_index: 2_048,
            chunk_offset: 134_217_728,
            chunk_size: 65_536,
            chunk_digest: [0xAC; 32],
            leaf_digest: [0xBC; 32],
        },
        PorProofSampleV1 {
            sample_index: 65_535,
            chunk_offset: 4_294_836_224,
            chunk_size: 65_536,
            chunk_digest: [0xAD; 32],
            leaf_digest: [0xBD; 32],
        },
    ];

    let proof = PorProofV1 {
        version: POR_PROOF_VERSION_V1,
        challenge_id: challenge.challenge_id,
        manifest_digest: challenge.manifest_digest,
        provider_id: challenge.provider_id,
        samples: proof_samples,
        auth_path: vec![[0x11; 32], [0x22; 32], [0x33; 32]],
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![0x01; 32],
            signature: vec![0x02; 64],
        },
        submitted_at: 1_700_000_540,
    };
    proof.validate()?;
    let proof_digest = proof.proof_digest();

    let verdict = AuditVerdictV1 {
        version: AUDIT_VERDICT_VERSION_V1,
        manifest_digest: challenge.manifest_digest,
        provider_id: challenge.provider_id,
        challenge_id: challenge.challenge_id,
        proof_digest: Some(proof_digest),
        outcome: AuditOutcomeV1::Success,
        failure_reason: None,
        decided_at: 1_700_000_600,
        auditor_signatures: vec![AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![0x03; 32],
            signature: vec![0x04; 64],
        }],
        metadata: vec![CapacityMetadataEntry {
            key: "auditor.note".to_string(),
            value: "PoR verified successfully".to_string(),
        }],
    };
    verdict.validate()?;

    write_norito_pair(
        &por_dir.join("challenge_v1"),
        &challenge,
        challenge_json(&challenge),
    )?;
    write_norito_pair(
        &por_dir.join("proof_v1"),
        &proof,
        proof_json(&proof, proof_digest),
    )?;
    write_norito_pair(
        &por_dir.join("verdict_v1"),
        &verdict,
        verdict_json(&verdict),
    )?;

    // Governance node sample (wrap proof).
    let node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: b"bafygovernancelognode".to_vec(),
        prev_cid: Some(b"bafygovernancelognodeprev".to_vec()),
        timestamp: 1_700_000_700,
        publisher_peer_id: b"12D3KooWGovernancePublisher".to_vec(),
        payload: GovernanceLogPayloadV1::PorProof(proof.clone()),
        publisher_signature: GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Dilithium3,
            public_key: vec![0x05; 64],
            signature: vec![0x06; 160],
        },
    };
    node.validate()?;

    write_norito_pair(
        &gov_dir.join("node_v1"),
        &node,
        governance_node_json(&node, proof_digest),
    )?;

    Ok(())
}

fn write_norito_pair<T>(
    base_path: &Path,
    value: &T,
    json_value: Value,
) -> Result<(), Box<dyn Error>>
where
    T: NoritoSerialize,
{
    let bytes = norito::to_bytes(value)?;
    fs::write(base_path.with_extension("to"), &bytes)?;
    let json = to_string_pretty(&json_value)?;
    fs::write(base_path.with_extension("json"), json)?;
    Ok(())
}

fn challenge_json(challenge: &PorChallengeV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(challenge.version));
    map.insert(
        "challenge_id_hex".into(),
        Value::from(encode(challenge.challenge_id)),
    );
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(challenge.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(challenge.provider_id)),
    );
    map.insert("epoch_id".into(), Value::from(challenge.epoch_id));
    map.insert("drand_round".into(), Value::from(challenge.drand_round));
    map.insert(
        "drand_randomness_hex".into(),
        Value::from(encode(challenge.drand_randomness)),
    );
    map.insert(
        "drand_signature_hex".into(),
        Value::from(encode(&challenge.drand_signature)),
    );
    map.insert(
        "vrf_output_hex".into(),
        match challenge.vrf_output {
            Some(output) => Value::from(encode(output)),
            None => Value::Null,
        },
    );
    map.insert(
        "vrf_proof_hex".into(),
        match &challenge.vrf_proof {
            Some(proof) => Value::from(encode(proof)),
            None => Value::Null,
        },
    );
    map.insert("forced".into(), Value::from(challenge.forced));
    map.insert(
        "chunking_profile".into(),
        Value::from(challenge.chunking_profile.clone()),
    );
    map.insert("seed_hex".into(), Value::from(encode(challenge.seed)));
    map.insert("sample_tier".into(), Value::from(challenge.sample_tier));
    map.insert("sample_count".into(), Value::from(challenge.sample_count));
    map.insert(
        "sample_indices".into(),
        Value::Array(
            challenge
                .sample_indices
                .iter()
                .map(|idx| Value::from(*idx))
                .collect(),
        ),
    );
    map.insert("issued_at".into(), Value::from(challenge.issued_at));
    map.insert("deadline_at".into(), Value::from(challenge.deadline_at));
    Value::Object(map)
}

fn proof_json(proof: &PorProofV1, digest: [u8; 32]) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(proof.version));
    map.insert(
        "challenge_id_hex".into(),
        Value::from(encode(proof.challenge_id)),
    );
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(proof.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(proof.provider_id)),
    );
    map.insert("submitted_at".into(), Value::from(proof.submitted_at));
    let samples = proof
        .samples
        .iter()
        .map(|sample| {
            let mut sample_map = Map::new();
            sample_map.insert("sample_index".into(), Value::from(sample.sample_index));
            sample_map.insert("chunk_offset".into(), Value::from(sample.chunk_offset));
            sample_map.insert("chunk_size".into(), Value::from(sample.chunk_size));
            sample_map.insert(
                "chunk_digest_hex".into(),
                Value::from(encode(sample.chunk_digest)),
            );
            sample_map.insert(
                "leaf_digest_hex".into(),
                Value::from(encode(sample.leaf_digest)),
            );
            Value::Object(sample_map)
        })
        .collect();
    map.insert("samples".into(), Value::Array(samples));
    map.insert(
        "auth_path_hex".into(),
        Value::Array(
            proof
                .auth_path
                .iter()
                .map(|node| Value::from(encode(node)))
                .collect(),
        ),
    );
    let mut sig = Map::new();
    let algorithm = match proof.signature.algorithm {
        SignatureAlgorithm::Ed25519 => "ed25519",
        SignatureAlgorithm::MultiSig => "multisig",
    };
    sig.insert("algorithm".into(), Value::from(algorithm));
    sig.insert(
        "public_key_hex".into(),
        Value::from(encode(&proof.signature.public_key)),
    );
    sig.insert(
        "signature_hex".into(),
        Value::from(encode(&proof.signature.signature)),
    );
    map.insert("signature".into(), Value::Object(sig));
    map.insert("proof_digest_hex".into(), Value::from(encode(digest)));
    Value::Object(map)
}

fn verdict_json(verdict: &AuditVerdictV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(verdict.version));
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(verdict.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(verdict.provider_id)),
    );
    map.insert(
        "challenge_id_hex".into(),
        Value::from(encode(verdict.challenge_id)),
    );
    map.insert(
        "proof_digest_hex".into(),
        verdict
            .proof_digest
            .as_ref()
            .map(|digest| Value::from(encode(digest)))
            .unwrap_or(Value::Null),
    );
    let outcome = match verdict.outcome {
        AuditOutcomeV1::Success => "success",
        AuditOutcomeV1::Failed => "failed",
        AuditOutcomeV1::Repaired => "repaired",
    };
    map.insert("outcome".into(), Value::from(outcome));
    map.insert(
        "failure_reason".into(),
        verdict
            .failure_reason
            .as_ref()
            .map(|s| Value::from(s.clone()))
            .unwrap_or(Value::Null),
    );
    map.insert("decided_at".into(), Value::from(verdict.decided_at));
    let signatures = verdict
        .auditor_signatures
        .iter()
        .map(|sig| {
            let mut sig_map = Map::new();
            let algorithm = match sig.algorithm {
                SignatureAlgorithm::Ed25519 => "ed25519",
                SignatureAlgorithm::MultiSig => "multisig",
            };
            sig_map.insert("algorithm".into(), Value::from(algorithm));
            sig_map.insert(
                "public_key_hex".into(),
                Value::from(encode(&sig.public_key)),
            );
            sig_map.insert("signature_hex".into(), Value::from(encode(&sig.signature)));
            Value::Object(sig_map)
        })
        .collect();
    map.insert("auditor_signatures".into(), Value::Array(signatures));
    let metadata = verdict
        .metadata
        .iter()
        .map(|entry| {
            let mut meta_map = Map::new();
            meta_map.insert("key".into(), Value::from(entry.key.clone()));
            meta_map.insert("value".into(), Value::from(entry.value.clone()));
            Value::Object(meta_map)
        })
        .collect();
    map.insert("metadata".into(), Value::Array(metadata));
    Value::Object(map)
}

fn governance_node_json(node: &GovernanceLogNodeV1, proof_digest: [u8; 32]) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(node.version));
    map.insert(
        "node_cid".into(),
        Value::from(String::from_utf8_lossy(&node.node_cid).into_owned()),
    );
    map.insert(
        "prev_cid".into(),
        node.prev_cid
            .as_ref()
            .map(|cid| Value::from(String::from_utf8_lossy(cid).into_owned()))
            .unwrap_or(Value::Null),
    );
    map.insert("timestamp".into(), Value::from(node.timestamp));
    map.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&node.publisher_peer_id).into_owned()),
    );
    map.insert("payload_kind".into(), Value::from("por_proof"));
    let mut sig = Map::new();
    let algorithm = match node.publisher_signature.algorithm {
        GovernanceSignatureAlgorithm::Ed25519 => "ed25519",
        GovernanceSignatureAlgorithm::Dilithium3 => "dilithium3",
    };
    sig.insert("algorithm".into(), Value::from(algorithm));
    sig.insert(
        "public_key_hex".into(),
        Value::from(encode(&node.publisher_signature.public_key)),
    );
    sig.insert(
        "signature_hex".into(),
        Value::from(encode(&node.publisher_signature.signature)),
    );
    map.insert("publisher_signature".into(), Value::Object(sig));
    map.insert(
        "embedded_proof_digest_hex".into(),
        Value::from(encode(proof_digest)),
    );
    Value::Object(map)
}
