//! Native binding conversion checks against the platform-independent codec owner.

use super::*;

#[test]
fn shared_codec_adapter_preserves_account_result_and_error() {
    let key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).expect("fixture key");
    let account = AccountId::new(key.public_key().clone());
    let literal = account.to_i105_for_discriminant(369).expect("Taira I105");
    let shared =
        iroha_js_codec::account_address_parse_encoded(&literal, None).expect("shared parse");
    let native = account_address_parse_encoded(literal.clone(), None).expect("native parse");
    assert_eq!(
        native.canonical_bytes.as_ref(),
        shared.canonical_bytes.as_slice()
    );
    assert_eq!(native.network_prefix, Some(shared.network_prefix));
    let native_render =
        account_address_render(Uint8Array::from(shared.canonical_bytes.clone()), 369)
            .expect("native render");
    let shared_render = iroha_js_codec::account_address_render(&shared.canonical_bytes, 369)
        .expect("shared render");
    assert_eq!(native_render.canonical_hex, shared_render.canonical_hex);
    assert_eq!(native_render.i105, shared_render.i105);
    let error = account_address_parse_encoded(literal.clone(), Some(42))
        .err()
        .expect("wrong network");
    let shared_error =
        iroha_js_codec::account_address_parse_encoded(&literal, Some(42)).unwrap_err();
    assert_eq!(error.status, napi::Status::InvalidArg);
    assert_eq!(error.reason, shared_error.reason());
}

#[test]
fn shared_codec_adapter_preserves_instruction_frames_archives_and_errors() {
    let source = json::to_json(&norito_json!({
        "CancelAssetLock": json::to_value(&CancelAssetLock::new(
            EscrowId::new(Hash::new(b"shared-native-adapter")), Quantity::from(15_u32)
        )).expect("cancel JSON")
    }))
    .expect("instruction JSON");
    let shared_frame = iroha_js_codec::encode_instruction_frame(&source).expect("shared frame");
    let native_frame = norito_encode_instruction(source.clone()).expect("native frame");
    assert_eq!(native_frame.as_ref(), shared_frame.as_slice());
    assert_eq!(
        norito_decode_instruction(Uint8Array::from(shared_frame.clone())).expect("native decode"),
        iroha_js_codec::decode_instruction_frame(&shared_frame).expect("shared decode")
    );
    let shared_archive =
        iroha_js_codec::encode_instruction_archive(&source).expect("shared archive");
    let native_archive = norito_encode_instruction_box_archive(source).expect("native archive");
    assert_eq!(native_archive.as_ref(), shared_archive.as_slice());
    assert_eq!(
        norito_decode_instruction_box_archive(Uint8Array::from(shared_archive.clone()))
            .expect("native archive decode"),
        iroha_js_codec::decode_instruction_archive(&shared_archive).expect("shared archive decode")
    );
    for malformed in ["{", "{\"Register\":null}"] {
        let shared_error = iroha_js_codec::encode_instruction_frame(malformed).unwrap_err();
        let native_error = norito_encode_instruction(malformed.to_owned()).unwrap_err();
        let expected_status = match shared_error.kind() {
            iroha_js_codec::CodecErrorKind::InvalidArgument => napi::Status::InvalidArg,
            iroha_js_codec::CodecErrorKind::Failure => napi::Status::GenericFailure,
        };
        assert_eq!(native_error.status, expected_status);
        assert_eq!(native_error.reason, shared_error.reason());
    }
}
