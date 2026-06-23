//! Generates PoR, PoTR, repair, and governance log fixtures.

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
    CapacityMetadataEntry, POTR_RECEIPT_VERSION_V1, PotrReceiptV1, PotrStatus, ProofStreamTier,
    REPAIR_TASK_VERSION_V1, RepairTaskRecordV1, RepairTaskStateV1, RepairTicketId,
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
    repair::QueuedRepairStateV1,
};
use soranet_pq::{HedgedRngSeed, MlDsaSuite, deterministic_chacha20_rng, sign_mldsa};

fn main() -> Result<(), Box<dyn Error>> {
    let por_dir = PathBuf::from("fixtures/sorafs_manifest/por");
    let potr_dir = PathBuf::from("fixtures/sorafs_manifest/potr");
    let repair_dir = PathBuf::from("fixtures/sorafs_manifest/repair");
    let gov_dir = PathBuf::from("fixtures/sorafs_manifest/governance");
    fs::create_dir_all(&por_dir)?;
    fs::create_dir_all(&potr_dir)?;
    fs::create_dir_all(&repair_dir)?;
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

    let potr_receipt = PotrReceiptV1 {
        version: POTR_RECEIPT_VERSION_V1,
        manifest_digest,
        provider_id,
        tier: ProofStreamTier::Hot,
        deadline_ms: 90_000,
        latency_ms: 42_000,
        status: PotrStatus::Success,
        requested_at_ms: 1_700_000_000_000,
        responded_at_ms: 1_700_000_042_000,
        recorded_at_ms: 1_700_000_042_100,
        range_start: 0,
        range_end: 1_048_575,
        request_id: Some([0x44; 16]),
        trace_id: Some([0x33; 16]),
        note: Some("fixture retrieval completed".to_string()),
        gateway_signature: None,
        provider_signature: None,
    };
    potr_receipt.validate()?;
    write_norito_pair(
        &potr_dir.join("receipt_v1"),
        &potr_receipt,
        potr_receipt_json(&potr_receipt),
    )?;

    let repair_task = RepairTaskRecordV1 {
        version: REPAIR_TASK_VERSION_V1,
        ticket_id: RepairTicketId("REP-900".to_owned()),
        manifest_digest,
        provider_id,
        auditor_account: "auditor@sora".to_owned(),
        state: RepairTaskStateV1::Queued(QueuedRepairStateV1 {
            queued_at_unix: 1_700_000_060,
            sla_deadline_unix: Some(1_700_086_400),
        }),
        por_history_id: Some(drand_round),
        sla_deadline_unix: Some(1_700_086_400),
        scheduler_notes: Some("waiting for worker claim".to_owned()),
        slash_proposal_digest: None,
    };
    repair_task.validate()?;
    write_norito_pair(
        &repair_dir.join("task_v1"),
        &repair_task,
        repair_task_json(&repair_task),
    )?;

    // Governance node sample (wrap proof).
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: b"bafygovernancelognode".to_vec(),
        prev_cid: Some(b"bafygovernancelognodeprev".to_vec()),
        timestamp: 1_700_000_700,
        publisher_peer_id: b"12D3KooWGovernancePublisher".to_vec(),
        payload: GovernanceLogPayloadV1::PorProof(proof.clone()),
        publisher_signature: GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Dilithium3,
            public_key: Vec::new(),
            signature: Vec::new(),
        },
    };
    sign_governance_log_node_mldsa(&mut node, b"sorafs-fixture-governance-mldsa-v1")?;
    node.validate()?;
    node.verify_publisher_signature()?;

    write_norito_pair(
        &gov_dir.join("node_v1"),
        &node,
        governance_node_json(&node, proof_digest),
    )?;

    Ok(())
}

fn sign_governance_log_node_mldsa(
    node: &mut GovernanceLogNodeV1,
    seed: &[u8],
) -> Result<(), Box<dyn Error>> {
    let seed = blake3::hash(seed);
    let key_pair = soranet_pq::generate_mldsa_keypair_from_seed(
        MlDsaSuite::MlDsa65,
        HedgedRngSeed::from_entropy(*seed.as_bytes()),
        b"sorafs-fixture-governance-mldsa-keypair-v1",
    )?;
    let payload_bytes = node.signature_payload_bytes()?;
    let mut signing_rng = deterministic_chacha20_rng(
        HedgedRngSeed::from_entropy(
            *blake3::hash(b"sorafs-fixture-governance-mldsa-sign-v1").as_bytes(),
        ),
        b"sorafs-fixture-governance-mldsa-sign-v1",
    );
    let signature = sign_mldsa(
        MlDsaSuite::MlDsa65,
        key_pair.secret_key(),
        &[],
        &payload_bytes,
        &mut signing_rng,
    )?;
    node.publisher_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Dilithium3,
        public_key: key_pair.public_key().to_vec(),
        signature: signature.as_bytes().to_vec(),
    };
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

fn potr_receipt_json(receipt: &PotrReceiptV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(receipt.version));
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(receipt.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(receipt.provider_id)),
    );
    map.insert("tier".into(), Value::from(proof_stream_tier(receipt.tier)));
    map.insert("deadline_ms".into(), Value::from(receipt.deadline_ms));
    map.insert("latency_ms".into(), Value::from(receipt.latency_ms));
    map.insert("status".into(), Value::from(potr_status(receipt.status)));
    map.insert(
        "requested_at_ms".into(),
        Value::from(receipt.requested_at_ms),
    );
    map.insert(
        "responded_at_ms".into(),
        Value::from(receipt.responded_at_ms),
    );
    map.insert("recorded_at_ms".into(), Value::from(receipt.recorded_at_ms));
    map.insert("range_start".into(), Value::from(receipt.range_start));
    map.insert("range_end".into(), Value::from(receipt.range_end));
    map.insert(
        "request_id_hex".into(),
        receipt
            .request_id
            .map(|request_id| Value::from(encode(request_id)))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "trace_id_hex".into(),
        receipt
            .trace_id
            .map(|trace_id| Value::from(encode(trace_id)))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "note".into(),
        receipt
            .note
            .as_ref()
            .map(|note| Value::from(note.clone()))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "gateway_signature".into(),
        if receipt.gateway_signature.is_some() {
            Value::from("present")
        } else {
            Value::Null
        },
    );
    map.insert(
        "provider_signature".into(),
        if receipt.provider_signature.is_some() {
            Value::from("present")
        } else {
            Value::Null
        },
    );
    Value::Object(map)
}

fn repair_task_json(task: &RepairTaskRecordV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(task.version));
    map.insert("ticket_id".into(), Value::from(task.ticket_id.to_string()));
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(task.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(task.provider_id)),
    );
    map.insert(
        "auditor_account".into(),
        Value::from(task.auditor_account.clone()),
    );
    map.insert("state".into(), repair_state_json(&task.state));
    map.insert(
        "por_history_id".into(),
        task.por_history_id.map(Value::from).unwrap_or(Value::Null),
    );
    map.insert(
        "sla_deadline_unix".into(),
        task.sla_deadline_unix
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "scheduler_notes".into(),
        task.scheduler_notes
            .as_ref()
            .map(|notes| Value::from(notes.clone()))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "slash_proposal_digest_hex".into(),
        task.slash_proposal_digest
            .map(|digest| Value::from(encode(digest)))
            .unwrap_or(Value::Null),
    );
    Value::Object(map)
}

fn repair_state_json(state: &RepairTaskStateV1) -> Value {
    let mut map = Map::new();
    match state {
        RepairTaskStateV1::Queued(queued) => {
            map.insert("state".into(), Value::from("queued"));
            map.insert("queued_at_unix".into(), Value::from(queued.queued_at_unix));
            map.insert(
                "sla_deadline_unix".into(),
                queued
                    .sla_deadline_unix
                    .map(Value::from)
                    .unwrap_or(Value::Null),
            );
        }
        RepairTaskStateV1::InProgress(in_progress) => {
            map.insert("state".into(), Value::from("in_progress"));
            map.insert(
                "queued_at_unix".into(),
                Value::from(in_progress.queued_at_unix),
            );
            map.insert(
                "started_at_unix".into(),
                Value::from(in_progress.started_at_unix),
            );
            map.insert(
                "repair_agent".into(),
                in_progress
                    .repair_agent
                    .as_ref()
                    .map(|agent| Value::from(agent.clone()))
                    .unwrap_or(Value::Null),
            );
        }
        RepairTaskStateV1::Completed(completed) => {
            map.insert("state".into(), Value::from("completed"));
            map.insert(
                "queued_at_unix".into(),
                Value::from(completed.queued_at_unix),
            );
            map.insert(
                "started_at_unix".into(),
                Value::from(completed.started_at_unix),
            );
            map.insert(
                "completed_at_unix".into(),
                Value::from(completed.completed_at_unix),
            );
            map.insert(
                "resolution_notes".into(),
                completed
                    .resolution_notes
                    .as_ref()
                    .map(|notes| Value::from(notes.clone()))
                    .unwrap_or(Value::Null),
            );
        }
        RepairTaskStateV1::Failed(failed) => {
            map.insert("state".into(), Value::from("failed"));
            map.insert("queued_at_unix".into(), Value::from(failed.queued_at_unix));
            map.insert("failed_at_unix".into(), Value::from(failed.failed_at_unix));
            map.insert("reason".into(), Value::from(failed.reason.clone()));
        }
        RepairTaskStateV1::Escalated(escalated) => {
            map.insert("state".into(), Value::from("escalated"));
            map.insert(
                "queued_at_unix".into(),
                Value::from(escalated.queued_at_unix),
            );
            map.insert(
                "escalated_at_unix".into(),
                Value::from(escalated.escalated_at_unix),
            );
            map.insert("reason".into(), Value::from(escalated.reason.clone()));
        }
    }
    Value::Object(map)
}

fn proof_stream_tier(tier: ProofStreamTier) -> &'static str {
    match tier {
        ProofStreamTier::Hot => "hot",
        ProofStreamTier::Warm => "warm",
        ProofStreamTier::Archive => "archive",
    }
}

fn potr_status(status: PotrStatus) -> &'static str {
    match status {
        PotrStatus::Success => "success",
        PotrStatus::MissedDeadline => "missed_deadline",
        PotrStatus::ProviderError => "provider_error",
        PotrStatus::GatewayError => "gateway_error",
        PotrStatus::ClientCancelled => "client_cancelled",
    }
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
