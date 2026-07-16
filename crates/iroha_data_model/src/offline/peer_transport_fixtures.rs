//! Deterministic canonical peer archives used only by release fixture generation.

use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

use super::*;
use crate::domain::DomainId;

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

/// Construct and validate the complete first-release peer measurement inventory.
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

    let mut acknowledgement_bundle = None;
    for (depth, peer_hops) in [(1_u8, 1_u32), (8, 8), (16, 8), (32, 8), (64, 8)] {
        let bundle = payment_bundle(
            &chain_id,
            &asset,
            &note,
            request.digest()?,
            depth,
            peer_hops,
        )?;
        let payment = KagemushaRecursiveSpendPeerPaymentV2 {
            recipient_bundle: bundle.clone(),
            recipient_membership_witness: membership_witness(final_root),
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
    let ack_payload = KagemushaReceiverAcknowledgementPayloadV2 {
        operation_id: [0x51; 32],
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
    ack.validate_for_payment(&request, &bundle)?;
    records.insert(
        1,
        KagemushaPeerTransportFixtureRecordV1 {
            label: "acknowledgement".to_owned(),
            kind: "acknowledgement",
            branch_depth: 0,
            peer_hops: 0,
            archive: ack.canonical_archive_for_payment(&request, &bundle)?,
        },
    );
    Ok(records)
}

fn payment_bundle(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    note: &KagemushaSpendableNoteDescriptorV2,
    request_digest: [u8; 32],
    depth: u8,
    peer_hops: u32,
) -> Result<KagemushaRecursiveSpendBundleV2, KagemushaValidationError> {
    let anchor_digest = [0x54; 32];
    let anchor_ref = KagemushaRecursiveSpendTopUpAnchorRefV2 {
        topup_operation_id: [0x52; 32],
        anchor_digest,
    };
    let mut claim = KagemushaRecursiveSpendBranchClaimV2::root(
        kagemusha_recursive_spend_lineage_root_v2(anchor_digest)?,
    )?;
    for edge in 0..depth {
        let mut binding = [0x60; 32];
        binding[0] = edge.saturating_add(1);
        claim = claim.child(KagemushaRecursiveSpendBranchV2::Recipient, binding)?;
    }
    let verifier_key_id = VerifyingKeyId::new(
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
    );
    let statement = KagemushaRecursiveSpendPublicStatementV2 {
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        asset_scale: 2,
        final_root: [0x44; 32],
        topup_anchor_refs: vec![anchor_ref],
        proof_step_count: u32::from(depth) + 1,
        peer_hop_count: peer_hops,
        current_note: note.clone(),
        branch_claims: vec![claim],
        transition: Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV2 {
                binding_digest: [0x50; 32],
                branch: KagemushaRecursiveSpendBranchV2::Recipient,
                recipient_request_digest: request_digest,
                operation_id: [0x51; 32],
                parent_max_proof_step_count: u32::from(depth),
                parent_max_peer_hop_count: peer_hops - 1,
            },
        )),
        artifact_binding: KagemushaRecursiveSpendArtifactBindingV3 {
            generation: "transport-fixture-v3".to_owned(),
            manifest_sha256: [0xA7; 32],
        },
        verifier_key_id: verifier_key_id.clone(),
    };
    let public_statement_digest = statement.digest()?;
    let bundle = KagemushaRecursiveSpendBundleV2 {
        statement,
        recursive_proof: KagemushaRecursiveSpendProofV2 {
            verifier_key_id,
            public_statement_digest,
            proof: ProofBox::new(
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3
                    .parse()
                    .expect("portable fixture backend"),
                vec![0x71; PROOF_BYTES],
            ),
        },
    };
    bundle.validate_public_binding()?;
    Ok(bundle)
}

fn membership_witness(root: [u8; 32]) -> KagemushaNoteMembershipWitnessV2 {
    let leaf_index = 5_u32;
    let input_directions = (0..KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
        .map(|level| ((leaf_index >> level) & 1) as u8)
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
            directions: vec![0; KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2],
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
