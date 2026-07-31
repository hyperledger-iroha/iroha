//! Deterministic canonical ABI-21 peer archives used only by release fixture generation.

use iroha_crypto::HashOf;
use norito::to_bytes;
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

use super::*;
use crate::{
    block::consensus_v2::{BlockSubject, ConsensusRound, ExecutionCommitment},
    domain::DomainId,
    peer::PeerId,
};

const PROOF_BYTES: usize = 4_096;

/// One canonical, serialization-derived peer archive measurement.
pub struct KagemushaPeerTransportFixtureRecordV1 {
    /// Stable release-measurement label.
    pub label: String,
    /// Peer payload discriminator.
    pub kind: &'static str,
    /// Conflict-claim branch depth represented by this sample.
    pub branch_depth: u8,
    /// Peer-hop count represented by this sample.
    pub peer_hops: u32,
    /// Canonical Norito archive bytes.
    pub archive: Vec<u8>,
}

/// Construct and validate the complete ABI-21 peer measurement inventory.
pub fn kagemusha_peer_transport_fixture_records_v1()
-> Result<Vec<KagemushaPeerTransportFixtureRecordV1>, KagemushaValidationError> {
    let signing_key =
        SigningKey::from_bytes((&[0x41; 32]).into()).expect("fixed non-zero P-256 fixture scalar");
    let receiver_public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(
        signing_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )?;
    let chain_id = ChainId::from("swift-kagemusha-transport");
    let asset = AssetDefinitionId::new(
        DomainId::try_new("transport", "universal").expect("portable fixture domain"),
        "cash".parse().expect("portable fixture asset name"),
    );
    let recipient_keypair = KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519)
        .expect("fixed Ed25519 fixture seed");
    let recipient = AccountId::new(recipient_keypair.public_key().clone());
    let amount = KagemushaScaledAmountV2::new(125, 2)?;
    let recipient_key_reference = kagemusha_receiver_key_reference_v2(&receiver_public_key)?;
    let receiver_device_id = "ios-transport-fixture";
    let recipient_commitment = [0x42; 32];
    let final_root = [0x44; 32];
    let note = KagemushaSpendableNoteDescriptorV2 {
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        note_commitment: recipient_commitment,
        spend_nullifier: [0x43; 32],
        amount,
    };
    let request_payload = KagemushaRecipientPaymentRequestSigningPayloadV2 {
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        amount,
        recipient,
        recipient_key_reference,
        receiver_device_id: receiver_device_id.to_owned(),
        receiver_public_key,
        request_id: [0x45; 32],
        issued_at_ms: 1_800_000_000_000,
        expires_at_ms: 1_800_000_060_000,
        recipient_output: note.clone(),
        sender_output_prover_material: vec![0x41, 0x42, 0x43],
    };
    let request_signature = sign(&signing_key, &request_payload.signing_bytes()?)?;
    let request = KagemushaRecipientPaymentRequestV2::from_signed_payload(
        request_payload,
        request_signature,
    )?;
    let mut records = vec![KagemushaPeerTransportFixtureRecordV1 {
        label: "request".to_owned(),
        kind: "request",
        branch_depth: 0,
        peer_hops: 0,
        archive: to_bytes(&request)?,
    }];

    let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "peer-transport-fixture-v4".to_owned(),
        manifest_sha256: [0xA7; 32],
    };
    let (anchor_ref, provenance) = topup_provenance(&chain_id, &asset, &note, &artifact_binding)?;
    let mut acknowledgement_bundle = None;
    for (depth, peer_hops) in [(1_u8, 1_u32), (8, 8), (16, 8), (32, 8), (64, 8)] {
        let bundle = payment_bundle(
            &request,
            &note,
            final_root,
            anchor_ref,
            artifact_binding.clone(),
            depth,
            peer_hops,
        )?;
        let payment = KagemushaRecursiveSpendPeerPaymentV4 {
            recipient_bundle: bundle.clone(),
            recipient_membership_witness: membership_witness(final_root, u32::from(depth) + 1),
            topup_provenance: provenance.clone(),
        };
        payment.validate_public_binding()?;
        records.push(KagemushaPeerTransportFixtureRecordV1 {
            label: format!("payment-depth-{depth}-hop-{peer_hops}"),
            kind: "payment",
            branch_depth: depth,
            peer_hops,
            archive: to_bytes(&payment)?,
        });
        if depth == 64 {
            acknowledgement_bundle = Some(bundle);
        }
    }

    let bundle = acknowledgement_bundle.expect("depth-64 fixture profile is present");
    let transition = match bundle.statement.transition.as_ref() {
        Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) => transition,
        _ => unreachable!("fixture bundle always carries a peer split"),
    };
    let ack_payload = KagemushaReceiverAcknowledgementPayloadV2 {
        operation_id: transition.operation_id,
        recipient_request_digest: request.digest()?,
        payment_bundle_digest: bundle.digest()?,
        recipient_commitment,
        accepted_at_ms: 1_800_000_001_000,
        receiver_device_id: receiver_device_id.to_owned(),
        receiver_key_reference: recipient_key_reference,
        receiver_public_key,
    };
    let ack = KagemushaReceiverAcknowledgementV2 {
        signature: sign(&signing_key, &ack_payload.signing_bytes()?)?,
        payload: ack_payload,
    };
    records.insert(
        1,
        KagemushaPeerTransportFixtureRecordV1 {
            label: "acknowledgement".to_owned(),
            kind: "acknowledgement",
            branch_depth: 0,
            peer_hops: 0,
            archive: ack.canonical_archive_for_payment_v4(&request, &bundle)?,
        },
    );
    Ok(records)
}

fn payment_bundle(
    request: &KagemushaRecipientPaymentRequestV2,
    note: &KagemushaSpendableNoteDescriptorV2,
    final_root: [u8; 32],
    anchor_ref: KagemushaRecursiveSpendTopUpAnchorRefV2,
    artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
    depth: u8,
    peer_hops: u32,
) -> Result<KagemushaRecursiveSpendBundleV4, KagemushaValidationError> {
    let lineage_root = kagemusha_recursive_spend_lineage_root_v2(anchor_ref.anchor_digest)?;
    let mut claim = KagemushaRecursiveSpendBranchClaimV2::root(lineage_root)?;
    for edge in 0..depth {
        let mut binding = [0x60; 32];
        binding[0] = edge.saturating_add(1);
        claim = claim.child(KagemushaRecursiveSpendBranchV2::Recipient, binding)?;
    }
    let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
        KagemushaPastaCycleParityV1::StepEq,
        artifact_binding.manifest_sha256,
    );
    let mut operation_id = [0x51; 32];
    operation_id[0] = depth;
    let statement = KagemushaRecursiveSpendPublicStatementV4 {
        chain_id: note.chain_id.clone(),
        asset: note.asset.clone(),
        asset_scale: note.amount.scale,
        final_root,
        next_zero_leaf_index: u32::from(depth) + 1,
        topup_anchor_refs: vec![anchor_ref],
        proof_step_count: u32::from(depth) + 1,
        peer_hop_count: peer_hops,
        current_note: note.clone(),
        branch_claims: vec![claim],
        transition: Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV4 {
                binding_digest: [0x50; 32],
                branch: KagemushaRecursiveSpendBranchV2::Recipient,
                recipient_request_digest: request.digest()?,
                operation_id,
                parent_max_proof_step_count: u32::from(depth),
                parent_max_peer_hop_count: peer_hops - 1,
            },
        )),
        artifact_binding: artifact_binding.clone(),
        verifier_key_id: verifier_key_id.clone(),
    };
    let public_statement_digest = statement.digest()?;
    let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
    state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
    let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
        step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
        artifact_generation: artifact_binding.generation,
        manifest_sha256: artifact_binding.manifest_sha256,
        step_eq_parameter_generation: "peer-fixture-eq-params-v5".to_owned(),
        step_ep_parameter_generation: "peer-fixture-ep-params-v5".to_owned(),
        step_eq_circuit_params_sha256: [0xB1; 32],
        step_ep_circuit_params_sha256: [0xB2; 32],
        step_eq_verifier_key_sha256: [0xB3; 32],
        step_ep_verifier_key_sha256: [0xB4; 32],
        state_boundary: KagemushaRecursiveSpendStateBoundaryV2::new(state_limbs)?,
        proof: ProofBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into(),
            vec![0x71; PROOF_BYTES],
        ),
    };
    let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
    operation_limbs[0] = u32::from(depth);
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
    bundle.validate_public_binding()?;
    Ok(bundle)
}

fn topup_provenance(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    note: &KagemushaSpendableNoteDescriptorV2,
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<
    (
        KagemushaRecursiveSpendTopUpAnchorRefV2,
        KagemushaRecursiveSpendTopUpProvenanceV4,
    ),
    KagemushaValidationError,
> {
    let seed = 0x21;
    let payer_key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("deterministic payer key");
    let payer = AccountId::new(payer_key.public_key().clone());
    let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
        chain_id: chain_id.clone(),
        payer: payer.clone(),
        asset: AssetId::new(asset.clone(), payer),
        asset_scale: note.amount.scale,
        amount: note.amount,
        initial_root: [0x22; 32],
        finalized_root: [0x23; 32],
        shield_leaf_index: 5,
        current_note: note.clone(),
        topup_operation_id: [0x24; 32],
        shield_verifier_id: VerifyingKeyId::new("halo2/ipa", "topup-shield-v2"),
        shield_verifier_commitment: [0x25; 32],
        artifact_binding: binding.clone(),
        finalized_height: 42,
        finalized_tx_hash: [0x26; 32],
        anchor_digest: [0; 32],
    }
    .finalize_digest()?;
    let anchor_ref = anchor.compact_ref()?;

    let ordinary_writes_root = Hash::new([seed, 3]);
    let topup_anchor_root = Hash::new([seed, 4]);
    let execution_commitment = ExecutionCommitment::new(
        Hash::new([seed, 1]),
        ExecutionCommitment::topup_post_state_root(1, ordinary_writes_root, topup_anchor_root),
        ordinary_writes_root,
        Some(topup_anchor_root),
        1,
        Hash::new([seed, 5]),
    )
    .expect("deterministic execution commitment");
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
        execution_commitment,
        signers: vec![0],
        aggregate_signature: vec![seed; 96],
    };
    let proof = KagemushaTopUpFinalityProofV2 {
        version: KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
        anchor: anchor_ref,
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
                da_layout: DataAvailabilityLayout {
                    encoding: crate::block::consensus_v2::PayloadEncoding::Plain,
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
    let validator_key = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::BlsNormal)
        .expect("deterministic validator key");
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
    Ok((
        anchor_ref,
        KagemushaRecursiveSpendTopUpProvenanceV4 {
            topup_finality_roster_artifact: roster,
            topup_finality_evidence: vec![KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
                topup_anchor: anchor,
                topup_finality_proof: proof,
            }],
        },
    ))
}

fn membership_witness(
    root: [u8; 32],
    next_zero_leaf_index: u32,
) -> KagemushaNoteMembershipWitnessV2 {
    let leaf_index = 5_u32;
    let input_directions = (0..KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
        .map(|level| ((leaf_index >> level) & 1) as u8)
        .collect();
    let dummy_directions = (0..KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
        .map(|level| ((next_zero_leaf_index >> level) & 1) as u8)
        .collect();
    KagemushaNoteMembershipWitnessV2 {
        leaf_index,
        input_path: KagemushaConfidentialMerklePathV2 {
            siblings: (1..=KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
                .map(|seed| [seed as u8; 32])
                .collect(),
            directions: input_directions,
            root,
        },
        dummy_input_path: KagemushaConfidentialMerklePathV2 {
            siblings: (1..=KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
                .map(|seed| [(seed + 32) as u8; 32])
                .collect(),
            directions: dummy_directions,
            root,
        },
    }
}

fn sign(
    key: &SigningKey,
    message: &[u8],
) -> Result<KagemushaDeviceSignatureV2, KagemushaValidationError> {
    let signature: P256Signature = key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_slice())
}
