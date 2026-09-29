//! Genuine admitted proof fixtures for generic proof records and events.
//!
//! These statements prove commitment binding only. They do not authorize ledger
//! execution; the network scenarios exercise generic verification and storage.
use iroha_core::zk::{self, test_utils::halo2_ivm_replay_binding_envelope};
use iroha_crypto::Hash;
use iroha_data_model::{
    isi::verifying_keys,
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    zk::OpenVerifyEnvelope,
};

pub(super) fn replay_binding_attachment(
    statement: &str,
    vk_name: &str,
) -> (ProofAttachment, verifying_keys::RegisterVerifyingKey) {
    let fixture = halo2_ivm_replay_binding_envelope(
        Hash::new(format!("{statement}:code")),
        Hash::new(format!("{statement}:overlay")),
        Hash::new(format!("{statement}:events")),
        Hash::new(format!("{statement}:gas")),
    );
    let record = zk::halo2_ipa_ivm_replay_binding_vk_record("integration", 1)
        .expect("canonical replay-binding registry key");
    assert_eq!(fixture.schema_hash, record.public_inputs_schema_hash);
    assert_eq!(fixture.vk_box(zk::ZK_BACKEND_HALO2_IPA), record.key);
    let id = VerifyingKeyId::new(zk::ZK_BACKEND_HALO2_IPA, vk_name);
    (
        ProofAttachment::new_ref(
            zk::ZK_BACKEND_HALO2_IPA.into(),
            fixture.proof_box(zk::ZK_BACKEND_HALO2_IPA),
            id.clone(),
        ),
        verifying_keys::RegisterVerifyingKey { id, record },
    )
}

pub(super) fn rejected_replay_binding_attachment(
    statement: &str,
    vk_name: &str,
) -> ProofAttachment {
    let (mut attachment, _) = replay_binding_attachment(statement, vk_name);
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
fn replay_binding_fixtures_verify_and_reject_only_native_proof_corruption() {
    let (valid, registration) = replay_binding_attachment("native-control", "native_control_vk");
    let invalid = rejected_replay_binding_attachment("native-control", "native_control_vk");
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
    assert_eq!(
        valid_env.circuit_id,
        zk::IVM_REPLAY_BINDING_V1_CANONICAL_CIRCUIT_ID
    );
    assert_eq!(valid_env.public_inputs, invalid_env.public_inputs);
    assert_eq!(valid_env.vk_hash, invalid_env.vk_hash);
    assert_eq!(valid_env.circuit_id, invalid_env.circuit_id);
    assert_eq!(valid_env.proof_bytes.len(), invalid_env.proof_bytes.len());
    assert_ne!(valid_env.proof_bytes, invalid_env.proof_bytes);
    assert_eq!(valid.vk_ref, invalid.vk_ref);
    assert!(valid.proof.bytes.len() <= registration.record.max_proof_bytes as usize);
}
