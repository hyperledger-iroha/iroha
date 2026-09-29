//! Real confidential wallet proofs for generic verification records and events.
//!
//! These synthetic trees are proof fixtures, not ledger-authorized roots. The
//! scenarios exercise generic verification and storage without moving value.
use iroha_core::zk::{
    self,
    confidential::{ConfidentialProver, ConfidentialTree},
    confidential_v2::{
        CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID, ConfidentialUnshieldInputV2,
        compute_confidential_root_v2, confidential_unshield_v2_vk_record,
        default_confidential_diversifier_v2, derive_confidential_note_v2,
        derive_confidential_owner_tag_v2_with_diversifier,
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    isi::verifying_keys,
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    zk::OpenVerifyEnvelope,
};
use zeroize::Zeroizing;

pub(super) fn confidential_attachment(
    statement: &str,
    vk_name: &str,
) -> (ProofAttachment, verifying_keys::RegisterVerifyingKey) {
    let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(statement),
    ));
    let asset = AssetDefinitionId::from_uuid_bytes([
        1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
    ])
    .expect("canonical fixture asset");
    // Disposable test openings only. The public prover owns its private inputs
    // and obtains actual proof randomness from the operating system.
    let spend_key = Zeroizing::new([0x31; 32]);
    let input = ConfidentialUnshieldInputV2 {
        amount: 9,
        rho: [0x23; 32],
        diversifier: default_confidential_diversifier_v2(),
        leaf_index: 0,
    };
    let owner =
        derive_confidential_owner_tag_v2_with_diversifier(spend_key.as_ref(), input.diversifier)
            .expect("fixture note owner");
    let leaves = [
        derive_confidential_note_v2(&asset.to_string(), input.amount, input.rho, owner)
            .expect("fixture commitment"),
    ];
    let root = compute_confidential_root_v2(&leaves).expect("fixture root");
    let proof = ConfidentialProver::new(network, &asset, spend_key)
        .expect("wallet context")
        .prove_unshield(
            ConfidentialTree::Commitments {
                root,
                leaves: &leaves,
            },
            vec![input],
            9,
            None,
        )
        .expect("real one-note full-unshield proof");
    assert_eq!(proof.relation, zk::ProofRelation::ConfidentialFullUnshield);
    assert_eq!(proof.nullifiers.len(), 1);
    assert!(proof.output_commitments.is_empty());
    let record = confidential_unshield_v2_vk_record("integration", 1)
        .expect("canonical full-unshield registry key");
    let envelope: OpenVerifyEnvelope =
        norito::decode_canonical(&proof.proof.bytes).expect("canonical wallet envelope");
    let schema_hash: [u8; 32] = Hash::new(&envelope.public_inputs).into();
    assert_eq!(schema_hash, record.public_inputs_schema_hash);
    assert_eq!(envelope.vk_hash, record.commitment);
    assert_eq!(envelope.circuit_id, record.circuit_id);
    let id = VerifyingKeyId::new(zk::ZK_BACKEND_HALO2_IPA, vk_name);
    (
        ProofAttachment::new_ref(zk::ZK_BACKEND_HALO2_IPA.into(), proof.proof, id.clone()),
        verifying_keys::RegisterVerifyingKey { id, record },
    )
}

pub(super) fn rejected_confidential_attachment(statement: &str, vk_name: &str) -> ProofAttachment {
    let (mut attachment, _) = confidential_attachment(statement, vk_name);
    attachment.proof = corrupt_native_halo2_proof(&attachment.proof);
    attachment
}

pub(super) fn corrupt_native_halo2_proof(proof: &ProofBox) -> ProofBox {
    let mut envelope: OpenVerifyEnvelope =
        norito::decode_canonical(&proof.bytes).expect("canonical genuine Halo2 envelope");
    // Keep the carrier, public instances, schema, key and all TLV lengths intact.
    let carrier = &mut envelope.proof_bytes;
    assert!(carrier.starts_with(b"ZK1\0"));
    let mut position = 4_usize;
    let mut target = None;
    while position < carrier.len() {
        let header_end = position.checked_add(8).expect("TLV header extent");
        assert!(header_end <= carrier.len());
        let length = u32::from_le_bytes(
            carrier[position + 4..header_end]
                .try_into()
                .expect("TLV length"),
        ) as usize;
        let end = header_end.checked_add(length).expect("TLV payload extent");
        assert!(end <= carrier.len());
        if &carrier[position..position + 4] == b"PROF" {
            assert!(target.is_none() && length != 0);
            target = Some(header_end + length / 2);
        }
        position = end;
    }
    carrier[target.expect("native proof TLV")] ^= 1;
    ProofBox::new(
        proof.backend.clone(),
        norito::encode_canonical(&envelope).expect("canonical damaged proof"),
    )
}

#[test]
fn confidential_fixtures_verify_and_reject_only_native_proof_corruption() {
    let (valid, registration) = confidential_attachment("native-control", "native_control_vk");
    let mut invalid = valid.clone();
    invalid.proof = corrupt_native_halo2_proof(&valid.proof);
    let key = registration
        .record
        .key
        .as_ref()
        .expect("inline canonical key");
    assert!(zk::verify_backend(
        &valid.proof.backend,
        &valid.proof,
        Some(key)
    ));
    assert!(!zk::verify_backend(
        &invalid.proof.backend,
        &invalid.proof,
        Some(key)
    ));
    let valid_env: OpenVerifyEnvelope = norito::decode_canonical(&valid.proof.bytes).unwrap();
    let invalid_env: OpenVerifyEnvelope = norito::decode_canonical(&invalid.proof.bytes).unwrap();
    assert_eq!(valid_env.circuit_id, CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID);
    assert_eq!(valid_env.public_inputs, invalid_env.public_inputs);
    assert_eq!(valid_env.vk_hash, invalid_env.vk_hash);
    assert_eq!(valid_env.circuit_id, invalid_env.circuit_id);
    assert_eq!(valid_env.proof_bytes.len(), invalid_env.proof_bytes.len());
    let changed = valid_env
        .proof_bytes
        .iter()
        .zip(&invalid_env.proof_bytes)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .collect::<Vec<_>>();
    assert_eq!(changed.len(), 1, "exactly one native proof byte changes");
    let native_length =
        u32::from_le_bytes(valid_env.proof_bytes[8..12].try_into().unwrap()) as usize;
    assert_eq!(&valid_env.proof_bytes[4..8], b"PROF");
    assert!((12..12 + native_length).contains(&changed[0].0));
    assert_eq!(*changed[0].1.0 ^ *changed[0].1.1, 1);
    assert_eq!(
        &valid_env.proof_bytes[12 + native_length..],
        &invalid_env.proof_bytes[12 + native_length..],
        "public input TLV stays intact"
    );
    assert_eq!(valid.vk_ref, invalid.vk_ref);
    assert!(valid.proof.bytes.len() <= registration.record.max_proof_bytes as usize);
}
