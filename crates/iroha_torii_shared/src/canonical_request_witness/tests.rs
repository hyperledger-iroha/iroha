//! Captured complete-frame and borrowed-payload contracts for witness signing.

use iroha_crypto::{PublicKey, Signature};
use iroha_data_model::soracloud::{
    CANONICAL_REQUEST_WITNESS_VERSION_V1, CanonicalRequestSignatureWitnessV1,
};

use super::*;

const SDK_FRAME: &[u8] =
    include_bytes!("../../tests/fixtures/canonical_request_witness_sdk_v1.bin");

fn captured_witness() -> CanonicalRequestWitnessV1 {
    let public_key: PublicKey =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("captured public test key");
    CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: AccountId::new(public_key),
        timestamp_ms: 42,
        nonce: "borrowed-wire-parity".to_owned(),
        canonical_request_hash: Hash::new(b"borrowed witness payload parity"),
        signatures: Vec::new(),
    }
}

#[derive(norito::derive::Encode, norito::derive::Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_torii_shared::canonical_request_witness::tests::OwnedPayloadV1",
    frame = "iroha::client::canonical_request_witness_message::CanonicalRequestWitnessPayloadV1"
)]
struct OwnedCanonicalRequestWitnessPayloadV1 {
    schema_version: u16,
    subject_account: AccountId,
    timestamp_ms: u64,
    nonce: String,
    canonical_request_hash: Hash,
}

#[test]
fn shared_signing_message_preserves_captured_sdk_frame_and_fields() {
    let witness = captured_witness();
    let bytes = encode_signing_message(&witness, SDK_FRAME.len()).expect("exact captured bound");
    assert_eq!(bytes, SDK_FRAME);
    let view = norito::core::from_bytes_view(&bytes).expect("validated archive");
    assert_eq!(view.flags(), 0x02);
    assert_eq!(
        hex::encode(view.schema()),
        "ef8788d6a3ac039b7406e5ae7b6c673e"
    );
    assert_eq!(
        view.schema(),
        norito::schema::identity::frame_hash::<CanonicalRequestWitnessPayloadV1<'_>>()
    );
    let decoded = norito::decode_from_bytes::<OwnedCanonicalRequestWitnessPayloadV1>(&bytes)
        .expect("independent owned decoder");
    assert_eq!(decoded.schema_version, witness.schema_version);
    assert_eq!(decoded.subject_account, witness.subject_account);
    assert_eq!(decoded.timestamp_ms, witness.timestamp_ms);
    assert_eq!(decoded.nonce, witness.nonce);
    assert_eq!(
        decoded.canonical_request_hash,
        witness.canonical_request_hash
    );
}

#[test]
fn borrowed_witness_signature_payload_preserves_owned_wire_bytes() {
    fn encode_bare<T: SerializePayload>(value: &T) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = norito::core::Encoder::for_buffer(&mut bytes);
        norito::core::SerializePayload::serialize(value, &mut encoder)
            .expect("serialize bare witness payload");
        bytes
    }

    let witness = captured_witness();
    let borrowed = CanonicalRequestWitnessPayloadV1 {
        schema_version: witness.schema_version,
        subject_account: BorrowedCanonicalRequestAccountId(&witness.subject_account),
        timestamp_ms: witness.timestamp_ms,
        nonce: Cow::Borrowed(&witness.nonce),
        canonical_request_hash: witness.canonical_request_hash,
    };
    let owned = OwnedCanonicalRequestWitnessPayloadV1 {
        schema_version: witness.schema_version,
        subject_account: witness.subject_account.clone(),
        timestamp_ms: witness.timestamp_ms,
        nonce: witness.nonce.clone(),
        canonical_request_hash: witness.canonical_request_hash,
    };
    let _canonical = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    assert_eq!(encode_bare(&borrowed), encode_bare(&owned));
    let flags = norito::core::header_flags::PACKED_STRUCT
        | norito::core::header_flags::FIELD_BITSET
        | norito::core::header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(flags);
    assert_eq!(encode_bare(&borrowed), encode_bare(&owned));
}

#[test]
fn signing_message_bounds_and_canonical_flags_restore_the_caller_context() {
    let witness = captured_witness();
    for flags in [0, 2, 4, 5, 6, 7, 0x20, 0x22, 0x26, 0x27] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let current_flags = norito::core::effective_decode_flags();
        assert_eq!(
            encode_signing_message(&witness, SDK_FRAME.len()).expect("exact byte limit"),
            SDK_FRAME
        );
        assert_eq!(norito::core::effective_decode_flags(), current_flags);
        for limit in [0, SDK_FRAME.len() - 1] {
            assert!(matches!(
                encode_signing_message(&witness, limit),
                Err(BoundedEncodeError::FrameTooLarge { encoded_bytes, max_bytes })
                    if encoded_bytes == SDK_FRAME.len() && max_bytes == limit
            ));
            assert_eq!(norito::core::effective_decode_flags(), current_flags);
        }
    }
}

#[test]
fn signing_message_excludes_every_signature_entry() {
    let mut witness = captured_witness();
    let signer = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
        .parse()
        .expect("captured public test key");
    let entry = CanonicalRequestSignatureWitnessV1 {
        signer,
        signature: Signature::from_bytes(&[0x11; 64]),
    };
    for count in [0, 1, 64] {
        witness.signatures = vec![entry.clone(); count];
        assert_eq!(
            encode_signing_message(&witness, SDK_FRAME.len()).expect("signatures are excluded"),
            SDK_FRAME
        );
    }
}
