//! Generate the canonical ABI-21 Kagemusha peer-payment fixture used by Swift tests.

use std::{env, fs, path::Path};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    block::consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, ExecutionCommitment,
        GlobalPhase, HeightContextId, PROTOCOL_VERSION, PayloadEncoding, QuorumCertificate,
        ValidatorPower,
    },
    offline::{
        KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2, KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2, KagemushaConfidentialMerklePathV2,
        KagemushaNoteMembershipWitnessV2, KagemushaPastaCycleParityV1,
        KagemushaPastaCycleProofEnvelopeV4, KagemushaRecipientPaymentRequestV2,
        KagemushaRecursiveSpendArtifactBindingV4, KagemushaRecursiveSpendBranchClaimV2,
        KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendBundleV4,
        KagemushaRecursiveSpendOperationVectorV4, KagemushaRecursiveSpendPeerPaymentV4,
        KagemushaRecursiveSpendPeerSplitTransitionV4, KagemushaRecursiveSpendProofV4,
        KagemushaRecursiveSpendPublicStatementV4, KagemushaRecursiveSpendStateBoundaryV2,
        KagemushaRecursiveSpendTopUpAnchorV4, KagemushaRecursiveSpendTopUpFinalityEvidenceV4,
        KagemushaRecursiveSpendTopUpProvenanceV4, KagemushaRecursiveSpendTransitionV4,
        KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
        KagemushaTopUpAnchorMerkleProofV2, KagemushaTopUpFinalityCompactQcV2,
        KagemushaTopUpFinalityHeightContextV2, KagemushaTopUpFinalityProofV2,
        KagemushaTopUpFinalityRosterArtifactV2, KagemushaTopUpFinalityRosterWindowV2,
        kagemusha_recursive_spend_lineage_root_v2, kagemusha_recursive_spend_verifier_key_id_v4,
    },
    peer::PeerId,
    proof::{ProofBox, VerifyingKeyId},
};

fn execution_commitment(seed: u8) -> ExecutionCommitment {
    let ordinary_writes_root = Hash::new([seed, 3]);
    let topup_anchor_root = Hash::new([seed, 4]);
    ExecutionCommitment::new(
        Hash::new([seed, 1]),
        ExecutionCommitment::topup_post_state_root(1, ordinary_writes_root, topup_anchor_root),
        ordinary_writes_root,
        Some(topup_anchor_root),
        1,
        1,
        Hash::new([seed, 5]),
    )
    .expect("fixture execution commitment must be canonical")
}

fn finality_evidence(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    amount: KagemushaScaledAmountV2,
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
    seed: u8,
) -> KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
    let payer_key =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture payer key");
    let payer = AccountId::new(payer_key.public_key().clone());
    let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
        chain_id: chain_id.clone(),
        payer: payer.clone(),
        asset: AssetId::new(asset.clone(), payer),
        asset_scale: amount.scale,
        amount,
        initial_root: [seed.wrapping_add(1); 32],
        finalized_root: [seed.wrapping_add(2); 32],
        shield_leaf_index: u32::from(seed),
        current_note: KagemushaSpendableNoteDescriptorV2 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            note_commitment: [seed.wrapping_add(3); 32],
            spend_nullifier: [seed.wrapping_add(4); 32],
            amount,
        },
        topup_operation_id: [seed.wrapping_add(5); 32],
        shield_verifier_id: VerifyingKeyId::new("halo2/ipa", "topup-shield-v2"),
        shield_verifier_commitment: [seed.wrapping_add(6); 32],
        artifact_binding: binding.clone(),
        finalized_height: 42,
        finalized_tx_hash: [seed.wrapping_add(7); 32],
        anchor_digest: [0; 32],
    }
    .finalize_digest()
    .expect("fixture top-up anchor must be canonical");
    let context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new([seed, 8])));
    let round = ConsensusRound {
        context_id,
        height: anchor.finalized_height,
        view: 0,
    };
    let certificate = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject: BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 9])),
            payload_hash: Hash::new([seed, 10]),
        },
        execution_commitment: execution_commitment(seed),
        signers: vec![0],
        aggregate_signature: vec![seed; 96],
    };
    let proof = KagemushaTopUpFinalityProofV2 {
        version: KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
        anchor: anchor.compact_ref().expect("fixture compact anchor"),
        commit_qc: KagemushaTopUpFinalityCompactQcV2 {
            height_context: KagemushaTopUpFinalityHeightContextV2 {
                context_id,
                chain_id: chain_id.clone(),
                protocol_version: PROTOCOL_VERSION,
                height: anchor.finalized_height,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                nexus_amx_context_hash: Hash::new([seed, 11]),
                execution_policy_hash: Hash::new([seed, 12]),
                da_layout: DataAvailabilityLayout {
                    encoding: PayloadEncoding::Plain,
                    chunk_size_bytes: 1024,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 4,
                },
                leader_seed: [seed.wrapping_add(12); 32],
            },
            certificate,
        },
        anchor_path: KagemushaTopUpAnchorMerkleProofV2 {
            leaf_index: 0,
            leaf_count: 1,
            siblings: Vec::new(),
        },
    };
    KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
        topup_anchor: anchor,
        topup_finality_proof: proof,
    }
}

fn membership_path(
    leaf_index: u32,
    root: [u8; 32],
    sibling_seed: u8,
) -> KagemushaConfidentialMerklePathV2 {
    KagemushaConfidentialMerklePathV2 {
        siblings: (0..KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
            .map(|offset| [sibling_seed.wrapping_add(offset as u8); 32])
            .collect(),
        directions: (0..KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
            .map(|level| ((leaf_index >> level) & 1) as u8)
            .collect(),
        root,
    }
}

fn fixture(request: &KagemushaRecipientPaymentRequestV2) -> KagemushaRecursiveSpendPeerPaymentV4 {
    request
        .validate_public_binding()
        .expect("fixture recipient request must be canonical");
    let chain_id = request.chain_id().clone();
    let asset = request.asset().clone();
    let binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "swift-kagemusha-abi21-fixture".to_owned(),
        manifest_sha256: [0x51; 32],
    };
    let validator_key =
        KeyPair::try_from_seed(vec![0x61; 32], Algorithm::BlsNormal).expect("fixture validator");
    let roster = KagemushaTopUpFinalityRosterArtifactV2 {
        version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
        chain_id: chain_id.clone(),
        artifact_generation: binding.generation.clone(),
        windows: vec![KagemushaTopUpFinalityRosterWindowV2 {
            activates_at_height: 1,
            withdraws_at_height: 100,
            consensus_mode: ConsensusMode::Permissioned,
            validator_set: vec![ValidatorPower {
                validator: PeerId::new(validator_key.public_key().clone()),
                power: 1,
            }],
            validator_set_pops: vec![[0x62; 96]],
        }],
    };
    let evidence = finality_evidence(&chain_id, &asset, request.amount(), &binding, 0x31);
    let anchor_ref = evidence
        .topup_anchor
        .compact_ref()
        .expect("fixture compact anchor");
    let lineage_root = kagemusha_recursive_spend_lineage_root_v2(anchor_ref.anchor_digest)
        .expect("fixture lineage root");
    let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
        KagemushaPastaCycleParityV1::StepEq,
        binding.manifest_sha256,
    );
    let statement = KagemushaRecursiveSpendPublicStatementV4 {
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        asset_scale: request.amount().scale,
        final_root: [0x71; 32],
        next_zero_leaf_index: 7,
        topup_anchor_refs: vec![anchor_ref],
        proof_step_count: 2,
        peer_hop_count: 1,
        current_note: request.recipient_output().clone(),
        branch_claims: vec![
            KagemushaRecursiveSpendBranchClaimV2::root(lineage_root)
                .expect("fixture root branch claim"),
        ],
        transition: Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV4 {
                binding_digest: [0x74; 32],
                branch: KagemushaRecursiveSpendBranchV2::Recipient,
                recipient_request_digest: request
                    .digest()
                    .expect("fixture recipient request digest"),
                operation_id: [0x76; 32],
                parent_max_proof_step_count: 1,
                parent_max_peer_hop_count: 0,
            },
        )),
        artifact_binding: binding.clone(),
        verifier_key_id: verifier_key_id.clone(),
    };
    let public_statement_digest = statement.digest().expect("fixture statement digest");
    let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
    state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
    let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
        step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
        artifact_generation: binding.generation.clone(),
        manifest_sha256: binding.manifest_sha256,
        step_eq_parameter_generation: "swift-kagemusha-eq-params".to_owned(),
        step_ep_parameter_generation: "swift-kagemusha-ep-params".to_owned(),
        step_eq_circuit_params_sha256: [0x5b; 32],
        step_ep_circuit_params_sha256: [0x5c; 32],
        step_eq_verifier_key_sha256: [0x5d; 32],
        step_ep_verifier_key_sha256: [0x5e; 32],
        state_boundary: KagemushaRecursiveSpendStateBoundaryV2::new(state_limbs)
            .expect("fixture state boundary"),
        proof: ProofBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into(),
            vec![0x5f],
        ),
    };
    let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
    operation_limbs[0] = 1;
    let bundle = KagemushaRecursiveSpendBundleV4 {
        statement,
        operation: KagemushaRecursiveSpendOperationVectorV4 {
            limbs: operation_limbs,
        },
        recursive_proof: KagemushaRecursiveSpendProofV4 {
            verifier_key_id,
            public_statement_digest,
            proof_envelope,
        },
    };
    let witness = KagemushaNoteMembershipWitnessV2 {
        leaf_index: 5,
        input_path: membership_path(5, bundle.statement.final_root, 0x11),
        dummy_input_path: membership_path(
            bundle.statement.next_zero_leaf_index,
            bundle.statement.final_root,
            0x31,
        ),
    };
    let payment = KagemushaRecursiveSpendPeerPaymentV4 {
        recipient_bundle: bundle,
        recipient_membership_witness: witness,
        topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4 {
            topup_finality_roster_artifact: roster,
            topup_finality_evidence: vec![evidence],
        },
    };
    payment
        .validate_public_binding()
        .expect("fixture peer payment must be canonical");
    payment
}

fn read_recipient_request(path: &Path) -> (Vec<u8>, KagemushaRecipientPaymentRequestV2) {
    let encoded = fs::read_to_string(path).expect("read recipient-request hex fixture");
    let compact = encoded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let bytes = hex::decode(compact).expect("decode recipient-request hex fixture");
    let request = norito::decode_from_bytes::<KagemushaRecipientPaymentRequestV2>(&bytes)
        .expect("decode recipient-request fixture");
    let canonical = norito::to_bytes(&request).expect("re-encode recipient-request fixture");
    assert_eq!(
        canonical, bytes,
        "recipient-request fixture must already be canonical"
    );
    (canonical, request)
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let [_, flag, path] = args.as_slice() else {
        eprintln!("usage: swift_kagemusha_peer_payment_v4 --recipient-request-hex PATH");
        std::process::exit(2);
    };
    if flag != "--recipient-request-hex" {
        eprintln!("usage: swift_kagemusha_peer_payment_v4 --recipient-request-hex PATH");
        std::process::exit(2);
    }
    let (request_bytes, request) = read_recipient_request(Path::new(path));
    let request_digest = request.digest().expect("derive recipient-request digest");
    let payment = fixture(&request);
    let bytes = norito::to_bytes(&payment).expect("encode fixture peer payment");
    eprintln!("recipient_request_bytes={}", request_bytes.len());
    eprintln!("recipient_request_digest={}", hex::encode(request_digest));
    eprintln!("payment_archive_bytes={}", bytes.len());
    println!("{}", hex::encode(bytes));
}
