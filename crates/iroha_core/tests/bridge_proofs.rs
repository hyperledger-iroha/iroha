//! Bridge proof submission and retention tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    executor::Executor,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, WorldReadOnly},
    telemetry::StateTelemetry,
};
use iroha_data_model::{
    prelude::*,
    proof::{ProofBox, ProofId, ProofStatus},
};
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn bridge_proof_id(proof: &BridgeProof) -> ProofId {
    let encoded = norito::to_bytes(proof).expect("encode bridge proof");
    let backend = proof.backend_label();
    let proof = ProofBox::new(backend.clone(), encoded);
    ProofId {
        backend,
        proof_hash: iroha_core::zk::hash_proof(&proof),
    }
}

fn configured_sol_source_verifier_material() -> iroha_sccp::SccpSourceVerifierMaterialV1 {
    let mut material =
        iroha_sccp::sccp_source_verifier_material_for_domain(iroha_sccp::SCCP_DOMAIN_SOL)
            .expect("SOL source verifier material");
    material.placeholder_material = false;
    material.source_trust_anchor_id = "sccp:sol:source-trust-anchor:mainnet:v1".to_owned();
    material.source_trust_anchor_hash = [0x11; 32];
    material.consensus_verifier_id = "sccp:sol:consensus-verifier:mainnet:v1".to_owned();
    material.consensus_verifier_hash = [0x22; 32];
    material.message_inclusion_verifier_id =
        "sccp:sol:message-inclusion-verifier:mainnet:v1".to_owned();
    material.message_inclusion_verifier_hash = [0x33; 32];
    material.finality_policy_id = "sccp:sol:finality-policy:mainnet:v1".to_owned();
    material.finality_policy_hash = [0x44; 32];
    assert!(
        !iroha_sccp::sccp_source_verifier_material_is_production_ready(&material),
        "SOL material must remain fail-closed until the real mainnet verifier is wired"
    );
    material
}

fn actual_source_verifier_material(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
) -> iroha_config::parameters::actual::SccpSourceVerifierMaterial {
    iroha_config::parameters::actual::SccpSourceVerifierMaterial {
        version: material.version,
        source_domain: material.source_domain,
        source_chain: material.source_chain.clone(),
        source_proof_plan: material.source_proof_plan.as_str().to_owned(),
        finality_model: material.finality_model.as_str().to_owned(),
        adapter_circuit_id: material.adapter_circuit_id.clone(),
        source_trust_anchor_id: material.source_trust_anchor_id.clone(),
        source_trust_anchor_hash: hex::encode(material.source_trust_anchor_hash),
        consensus_verifier_id: material.consensus_verifier_id.clone(),
        consensus_verifier_hash: hex::encode(material.consensus_verifier_hash),
        message_inclusion_verifier_id: material.message_inclusion_verifier_id.clone(),
        message_inclusion_verifier_hash: hex::encode(material.message_inclusion_verifier_hash),
        finality_policy_id: material.finality_policy_id.clone(),
        finality_policy_hash: hex::encode(material.finality_policy_hash),
        placeholder_material: material.placeholder_material,
    }
}

fn make_ics_proof(leaf_fill: u8, range: (u64, u64), pinned: bool) -> BridgeProof {
    let leaves = vec![[leaf_fill; 32], [leaf_fill.wrapping_add(1); 32]];
    let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
    let root_bytes: [u8; 32] = *tree.root().expect("root").as_ref();
    let proof = tree.get_proof(0).expect("proof");

    BridgeProof {
        range: BridgeProofRange {
            start_height: range.0,
            end_height: range.1,
        },
        manifest_hash: [0xAA; 32],
        payload: BridgeProofPayload::Ics(BridgeIcsProof {
            state_root: root_bytes,
            leaf_hash: leaves[0],
            proof,
            hash_function: BridgeHashFunction::Sha256,
        }),
        pinned,
    }
}

fn make_sccp_sol_to_sora_message_bridge_proof(nonce: u64) -> BridgeProof {
    make_sccp_sol_to_sora_message_bridge_proof_with_material(nonce, None)
}

fn make_sccp_sol_to_sora_message_bridge_proof_with_material(
    nonce: u64,
    source_material: Option<&iroha_sccp::SccpSourceVerifierMaterialV1>,
) -> BridgeProof {
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SOL,
        dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        nonce,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SOL,
        asset_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        asset_id: b"wsol#sol".to_vec(),
        amount: 7,
        sender_codec: iroha_sccp::SCCP_CODEC_SOLANA_BASE58,
        sender: b"11111111111111111111111111111111".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        recipient: b"alice@universal".to_vec(),
        route_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        route_id: b"sol:sora:wsol".to_vec(),
    });
    let payload_hash =
        iroha_sccp::payload_hash(&iroha_sccp::canonical_sccp_payload_bytes(&payload));
    let commitment = iroha_sccp::SccpHubCommitmentV1 {
        version: 1,
        kind: iroha_sccp::sccp_message_kind(&payload),
        target_domain: iroha_sccp::sccp_message_target_domain(&payload),
        message_id: iroha_sccp::sccp_message_id(&payload),
        payload_hash,
    };
    let merkle_proof = iroha_sccp::SccpMerkleProofV1 { steps: Vec::new() };
    let commitment_root = iroha_sccp::merkle_root_from_commitment(&commitment, &merkle_proof);

    let source_domain = iroha_sccp::SCCP_DOMAIN_SOL;
    let target_domain = iroha_sccp::SCCP_DOMAIN_SORA;
    let source_chain = iroha_sccp::sccp_chain_key_for_domain(source_domain)
        .expect("SOL chain key")
        .to_owned();
    let source_proof_plan = iroha_sccp::sccp_source_proof_plan_for_domain(source_domain)
        .expect("SOL source proof plan");
    let finality_model = iroha_sccp::sccp_proof_finality_model_for_domain(source_domain)
        .expect("SOL finality model");
    let finality_height = 3;
    let finality_block_hash = [0x55; 32];
    let source_event_digest = iroha_sccp::sccp_source_event_digest(
        source_domain,
        target_domain,
        commitment.message_id,
        commitment.payload_hash,
    );
    let source_event_leaf_hash = iroha_sccp::sccp_source_event_leaf_hash(source_event_digest);
    let inclusion_branch = vec![[0x44; 32].to_vec()];
    let receipt_or_message_root = iroha_sccp::sccp_source_message_root_from_branch(
        source_event_leaf_hash,
        0,
        &inclusion_branch,
    )
    .expect("source receipt root");
    let finalized_header_hash = iroha_sccp::sccp_source_finalized_header_hash(
        source_domain,
        finality_model,
        finality_height,
        finality_block_hash,
        receipt_or_message_root,
    );
    let adapter_proof = iroha_sccp::SccpSourceAdapterProofV1::SolanaFinalizedTransaction(
        iroha_sccp::SccpSolanaFinalizedSourceProofV1 {
            version: 1,
            source_domain,
            finalized_slot: finality_height,
            blockhash: finality_block_hash,
            bank_hash: [0x66; 32],
            transaction_status_root: receipt_or_message_root,
            message_proof_hash: [0x77; 32],
        },
    );
    let adapter_transcript_hash = iroha_sccp::sccp_source_adapter_transcript_hash(
        source_domain,
        target_domain,
        source_proof_plan,
        finality_model,
        finality_height,
        finality_block_hash,
        receipt_or_message_root,
        source_event_digest,
        &adapter_proof,
    );
    let envelope_context = iroha_sccp::SccpSourceChainProofEnvelopeV1 {
        version: 1,
        source_domain,
        target_domain,
        source_chain: source_chain.clone(),
        source_proof_plan,
        finality_model,
        message_id: commitment.message_id,
        payload_hash: commitment.payload_hash,
        source_event_digest,
        commitment_root,
        finality_height,
        finality_block_hash,
        finalized_header_hash,
        receipt_or_message_root,
        consensus_proof: Vec::new(),
        message_inclusion_proof: Vec::new(),
        inclusion_branch: inclusion_branch.clone(),
    };
    let adapter_verification_proof = if let Some(material) = source_material {
        iroha_sccp::build_sccp_source_adapter_verification_proof_with_material(
            &envelope_context,
            &adapter_proof,
            adapter_transcript_hash,
            material,
        )
    } else {
        iroha_sccp::build_sccp_source_adapter_verification_proof(
            &envelope_context,
            &adapter_proof,
            adapter_transcript_hash,
        )
    }
    .expect("build source adapter verification proof");
    let verifier_evidence = if let Some(material) = source_material {
        iroha_sccp::build_sccp_source_verifier_evidence_with_material(
            &envelope_context,
            &adapter_proof,
            adapter_transcript_hash,
            material,
        )
    } else {
        iroha_sccp::build_sccp_source_verifier_evidence(
            &envelope_context,
            &adapter_proof,
            adapter_transcript_hash,
        )
    }
    .expect("build source verifier evidence");
    let consensus_proof = norito::to_bytes(&iroha_sccp::SccpSourceConsensusProofV1 {
        version: 1,
        source_domain,
        source_chain: source_chain.clone(),
        source_proof_plan,
        finality_model,
        finality_height,
        finality_block_hash,
        receipt_or_message_root,
        finalized_header_hash,
        adapter_proof,
        adapter_transcript_hash,
        verifier_evidence,
        adapter_verification_proof,
    })
    .expect("encode source consensus proof");
    let message_inclusion_proof =
        norito::to_bytes(&iroha_sccp::SccpSourceMessageInclusionProofV1 {
            version: 1,
            source_domain,
            target_domain,
            message_id: commitment.message_id,
            payload_hash: commitment.payload_hash,
            source_event_digest,
            source_event_leaf_hash,
            receipt_or_message_root,
            leaf_index: 0,
        })
        .expect("encode source inclusion proof");
    let finality_proof = norito::to_bytes(&iroha_sccp::SccpSourceChainProofEnvelopeV1 {
        version: 1,
        source_domain,
        target_domain,
        source_chain,
        source_proof_plan,
        finality_model,
        message_id: commitment.message_id,
        payload_hash: commitment.payload_hash,
        source_event_digest,
        commitment_root,
        finality_height,
        finality_block_hash,
        finalized_header_hash,
        receipt_or_message_root,
        consensus_proof,
        message_inclusion_proof,
        inclusion_branch,
    })
    .expect("encode source-chain proof envelope");
    let bundle = iroha_sccp::NexusSccpMessageProofV1 {
        version: 1,
        commitment_root,
        commitment,
        merkle_proof,
        payload,
        finality_proof,
    };
    assert!(iroha_sccp::verified_sccp_message_nexus_finality_proof(&bundle).is_none());
    if let Some(material) = source_material {
        assert!(!iroha_sccp::verify_message_bundle_structure(&bundle));
        assert!(
            iroha_sccp::verify_message_bundle_structure_with_source_verifier_material(
                &bundle, material
            )
        );
        let source_proof =
            iroha_sccp::decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)
                .expect("decode source proof");
        assert!(
            iroha_sccp::verify_sccp_source_chain_proof_envelope_structure_with_material(
                &source_proof,
                material,
            )
        );
        assert!(
            !iroha_sccp::sccp_source_verifier_material_is_production_ready(material),
            "SOL material must remain fail-closed until the real mainnet verifier is wired"
        );
        assert!(
            !iroha_sccp::verify_sccp_source_chain_proof_envelope_production_with_material(
                &source_proof,
                material,
            )
        );
        assert_eq!(source_proof.source_domain, source_domain);
        assert_eq!(source_proof.target_domain, target_domain);
        assert_eq!(source_proof.message_id, bundle.commitment.message_id);
        assert_eq!(source_proof.payload_hash, bundle.commitment.payload_hash);
        assert_eq!(source_proof.commitment_root, bundle.commitment_root);
        assert!(
            iroha_sccp::verified_sccp_message_source_chain_proof_envelope_for_production_with_material(
                &bundle,
                material
            )
            .is_none()
        );
    } else {
        assert!(iroha_sccp::verify_message_bundle_structure(&bundle));
        assert!(iroha_sccp::verified_sccp_message_source_chain_proof_envelope(&bundle).is_some());
    }

    let artifact = if let Some(material) = source_material {
        iroha_sccp::build_nexus_sccp_message_transparent_proof_with_source_verifier_material_allow_unready(
            &bundle,
            material,
            true,
        )
    } else {
        iroha_sccp::build_nexus_sccp_message_transparent_proof_allow_unready(&bundle, true)
    }
    .expect("build SOL SCCP transparent proof");
    let manifest_hash = iroha_sccp::sccp_bridge_manifest_hash_for_seed(&artifact.manifest_seed);
    let backend = artifact.message_backend.clone();
    let proof_bytes = norito::to_bytes(&artifact).expect("encode SCCP transparent artifact");
    BridgeProof {
        range: BridgeProofRange {
            start_height: finality_height,
            end_height: finality_height,
        },
        manifest_hash,
        payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
            proof: ProofBox::new(backend, proof_bytes),
            recursion_depth: None,
        }),
        pinned: false,
    }
}

#[test]
fn submit_bridge_proof_records_metadata() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let proof = make_ics_proof(0x11, (1, 1), false);
    let expected_id = bridge_proof_id(&proof);
    let encoded_len = u32::try_from(norito::to_bytes(&proof).expect("encode proof").len())
        .expect("bridge proof length fits in u32");

    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect("bridge proof accepted");

    let rec = stx
        .world
        .proofs()
        .get(&expected_id)
        .expect("proof recorded");
    assert_eq!(rec.status, ProofStatus::Verified);
    let bridge = rec.bridge.as_ref().expect("bridge metadata stored");
    assert_eq!(bridge.commitment, expected_id.proof_hash);
    assert_eq!(bridge.size_bytes, encoded_len);
    assert_eq!(bridge.proof.range.start_height, 1);
    assert_eq!(bridge.proof.range.end_height, 1);
}

#[test]
fn bridge_retention_prunes_oldest_unpinned() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    state.zk.proof_history_cap = 1;
    state.zk.proof_retention_grace_blocks = 0;
    state.zk.proof_prune_batch = 10;

    let exec = Executor::default();

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let proof1 = make_ics_proof(0x21, (1, 1), false);
    let id1 = bridge_proof_id(&proof1);
    let submit1: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof1).into();
    exec.execute_instruction(&mut stx1, &ALICE_ID.clone(), submit1)
        .expect("first proof accepted");
    stx1.apply();
    block1
        .commit()
        .expect("commit first bridge-proof block snapshot");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let proof2 = make_ics_proof(0x33, (2, 2), false);
    let id2 = bridge_proof_id(&proof2);
    let submit2: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof2).into();
    exec.execute_instruction(&mut stx2, &ALICE_ID.clone(), submit2)
        .expect("second proof accepted");

    assert!(stx2.world.proofs().get(&id2).is_some());
    assert!(
        stx2.world.proofs().get(&id1).is_none(),
        "older unpinned proof should be pruned when cap is hit"
    );
}

#[test]
fn bridge_range_length_cap_enforced() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    state.zk.bridge_proof_max_range_len = 2;

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_ics_proof(0x44, (5, 10), false);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("range cap should reject long bridge proofs");
    assert!(
        format!("{err:?}").contains("range too large"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn bridge_height_window_respected() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();

    state.zk.bridge_proof_max_future_drift_blocks = 1;
    let header_future =
        iroha_data_model::block::BlockHeader::new(nonzero!(5_u64), None, None, None, 0, 0);
    let mut block_future = state.block(header_future);
    let mut stx_future = block_future.transaction();
    let future_proof = make_ics_proof(0x55, (7, 7), false);
    let submit_future: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(future_proof).into();
    let err = exec
        .execute_instruction(&mut stx_future, &ALICE_ID.clone(), submit_future)
        .expect_err("future drift guard should reject proof ahead of window");
    assert!(
        format!("{err:?}").contains("future window"),
        "unexpected error for future drift: {err:?}"
    );
    drop(stx_future);
    drop(block_future);

    state.zk.bridge_proof_max_future_drift_blocks = 10;
    state.zk.bridge_proof_max_past_age_blocks = 2;
    let header_past =
        iroha_data_model::block::BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
    let mut block_past = state.block(header_past);
    let mut stx_past = block_past.transaction();
    let stale_proof = make_ics_proof(0x66, (1, 7), false);
    let submit_past: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(stale_proof).into();
    let err = exec
        .execute_instruction(&mut stx_past, &ALICE_ID.clone(), submit_past)
        .expect_err("past window should reject stale proof");
    assert!(
        format!("{err:?}").contains("past window"),
        "unexpected error for stale proof: {err:?}"
    );
}

#[test]
fn generic_ics_proof_rejects_reserved_sccp_manifest_hash() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let mut proof = make_ics_proof(0x67, (1, 1), false);
    proof.manifest_hash = iroha_sccp::sccp_burn_bridge_manifest_hash_v1();
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("generic ICS SCCP manifest bypass must be rejected");
    assert!(
        format!("{err:?}").contains("typed SCCP bridge proof backends"),
        "unexpected error for reserved manifest bypass: {err:?}"
    );
}

#[test]
fn bridge_overlapping_ranges_are_rejected() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let proof1 = make_ics_proof(0x71, (10, 12), false);
    let submit1: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof1).into();
    exec.execute_instruction(&mut stx1, &ALICE_ID.clone(), submit1)
        .expect("first proof accepted");
    stx1.apply();
    block1
        .commit()
        .expect("commit first bridge-proof block snapshot");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let proof2 = make_ics_proof(0x72, (11, 13), false);
    let submit2: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof2).into();
    let err = exec
        .execute_instruction(&mut stx2, &ALICE_ID.clone(), submit2)
        .expect_err("overlapping bridge proof must be rejected");
    assert!(
        format!("{err:?}").contains("overlaps existing proof"),
        "unexpected error for overlap: {err:?}"
    );
}

#[test]
fn re_submitting_identical_bridge_proof_is_idempotent() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();
    let proof = make_ics_proof(0x73, (21, 21), false);
    let proof_id = bridge_proof_id(&proof);

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let submit1: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    exec.execute_instruction(&mut stx1, &ALICE_ID.clone(), submit1)
        .expect("first proof accepted");
    stx1.apply();
    block1
        .commit()
        .expect("commit first bridge-proof block snapshot");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let submit2: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    exec.execute_instruction(&mut stx2, &ALICE_ID.clone(), submit2)
        .expect("identical proof should be a no-op");

    let rec = stx2
        .world
        .proofs()
        .get(&proof_id)
        .expect("original proof remains recorded");
    assert_eq!(rec.status, ProofStatus::Verified);
    let bridge = rec.bridge.as_ref().expect("bridge metadata stored");
    assert_eq!(bridge.commitment, proof_id.proof_hash);
    assert_eq!(bridge.proof.range.start_height, 21);
    assert_eq!(bridge.proof.range.end_height, 21);
}

#[test]
fn malformed_sccp_transparent_bridge_proof_is_rejected() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = BridgeProof {
        range: BridgeProofRange {
            start_height: 1,
            end_height: 1,
        },
        manifest_hash: [0x44; 32],
        payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
            proof: ProofBox::new("sccp/stark-fri-v1/eth".into(), vec![0xAA]),
            recursion_depth: None,
        }),
        pinned: false,
    };
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("malformed SCCP artifact must be rejected");
    assert!(
        format!("{err:?}").contains("typed message artifacts"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_unready_lane_even_if_config_allows() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.sccp_allow_unready_transparent_proofs = true;
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof(99);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("unready SCCP lanes must not be accepted on-chain");
    assert!(
        format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_configured_source_verifier_material_for_unready_lane() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(100, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err(
            "configured source verifier material must not open an unready SCCP source lane",
        );
    assert!(
        format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );

    let proof_id = bridge_proof_id(&proof);
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_duplicate_source_verifier_material() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let actual = actual_source_verifier_material(&material);
    state.zk.sccp_source_verifier_materials.push(actual.clone());
    state.zk.sccp_source_verifier_materials.push(actual);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(101, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("duplicate source verifier material must fail closed");
    assert!(
        format!("{err:?}").contains("duplicated"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_placeholder_source_verifier_material() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let mut placeholder = actual_source_verifier_material(&material);
    placeholder.placeholder_material = true;
    state.zk.sccp_source_verifier_materials.push(placeholder);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(102, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("placeholder material must remain disabled");
    assert!(
        format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_malformed_source_verifier_material_hash() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let mut actual = actual_source_verifier_material(&material);
    actual.source_trust_anchor_hash = "not-hex".to_owned();
    state.zk.sccp_source_verifier_materials.push(actual);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(103, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("malformed material hash must fail closed");
    assert!(
        format!("{err:?}").contains("source_trust_anchor_hash"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_replayed_source_verifier_material_while_lane_closed() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let mut replayed = material.clone();
    replayed.consensus_verifier_hash = [0xEE; 32];
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&replayed));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(104, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("replayed verifier material must not open a closed source lane");
    assert!(
        format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
}
