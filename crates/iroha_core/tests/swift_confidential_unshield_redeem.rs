//! Cross-language acceptance for Swift-built confidential-unshield redeem attachments.
#[cfg(feature = "zk-tests")]
#[path = "kaigi_privacy.rs"]
mod kaigi_privacy;

use iroha_core::zk::{
    ZK_BACKEND_HALO2_IPA,
    confidential_v2::{
        CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID, CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1,
        confidential_unshield_v3_vk_record, ensure_confidential_unshield_v3_canonical_vk_box,
    },
    verify_backend,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    proof::{ProofAttachment, VerifyingKeyId},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use std::{env, fs};
#[test]
fn swift_confidential_unshield_redeem_attachment_is_canonical_and_verifies() {
    let Ok(path) = env::var("IROHA_SWIFT_UNSHIELD_ATTACHMENT_PATH") else {
        eprintln!("skipping Swift attachment acceptance without external artifact path");
        return;
    };
    let bytes = fs::read(&path).expect("read Swift confidential-unshield attachment");
    let attachment: ProofAttachment =
        norito::decode_from_bytes(&bytes).expect("decode Swift ProofAttachment");
    let record =
        confidential_unshield_v3_vk_record(iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE, 1)
            .expect("construct canonical unshield-v3 verifier record");
    let vk = record
        .key
        .as_ref()
        .expect("canonical unshield-v3 verifier record carries inline key");
    assert_eq!(attachment.backend.as_str(), ZK_BACKEND_HALO2_IPA);
    assert_eq!(
        attachment.vk_ref,
        VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "vk_unshield")
    );
    assert_eq!(attachment.vk_commitment, Some(record.commitment));
    assert_eq!(
        attachment.envelope_hash,
        Some(Hash::new(&attachment.proof.bytes).into())
    );
    assert!(attachment.lane_privacy.is_none());
    ensure_confidential_unshield_v3_canonical_vk_box(vk)
        .expect("Swift attachment record is bound to the canonical verifier key");
    let envelope: OpenVerifyEnvelope = norito::decode_from_bytes(&attachment.proof.bytes)
        .expect("Swift attachment carries canonical OpenVerifyEnvelope");
    assert_eq!(envelope.backend, BackendTag::Halo2IpaPasta);
    assert_eq!(envelope.circuit_id, CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID);
    assert_eq!(
        envelope.public_inputs,
        CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1
    );
    assert_eq!(envelope.vk_hash, record.commitment);
    assert!(envelope.aux.is_empty());
    assert!(verify_backend(
        ZK_BACKEND_HALO2_IPA,
        &attachment.proof,
        Some(vk)
    ));
}

#[path = "kagemusha_artifact_v4_streaming.rs"]
mod kagemusha_artifact_v4_streaming;
