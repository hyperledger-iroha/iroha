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
    let shared_frame =
        iroha_js_codec::encode_instruction_frame(&source, 369).expect("shared frame");
    let native_frame = norito_encode_instruction(source.clone(), 369.0).expect("native frame");
    assert_eq!(native_frame.as_ref(), shared_frame.as_slice());
    assert_eq!(
        norito_decode_instruction(Uint8Array::from(shared_frame.clone()), 369.0)
            .expect("native decode"),
        iroha_js_codec::decode_instruction_frame(&shared_frame, 369).expect("shared decode")
    );
    let shared_archive =
        iroha_js_codec::encode_instruction_archive(&source, 369).expect("shared archive");
    let native_archive =
        norito_encode_instruction_box_archive(source, 369.0).expect("native archive");
    assert_eq!(native_archive.as_ref(), shared_archive.as_slice());
    assert_eq!(
        norito_decode_instruction_box_archive(Uint8Array::from(shared_archive.clone()), 369.0)
            .expect("native archive decode"),
        iroha_js_codec::decode_instruction_archive(&shared_archive, 369)
            .expect("shared archive decode")
    );
    for malformed in ["{", "{\"Register\":null}"] {
        let shared_error = iroha_js_codec::encode_instruction_frame(malformed, 369).unwrap_err();
        let native_error = norito_encode_instruction(malformed.to_owned(), 369.0)
            .err()
            .expect("malformed instruction must fail native encoding");
        let expected_status = match shared_error.kind() {
            iroha_js_codec::CodecErrorKind::InvalidArgument => napi::Status::InvalidArg,
            iroha_js_codec::CodecErrorKind::Failure => napi::Status::GenericFailure,
        };
        assert_eq!(native_error.status, expected_status);
        assert_eq!(native_error.reason, shared_error.reason());
    }
}

fn malformed_context_operations(prefix: f64) -> [napi::Error; 13] {
    [
        norito_encode_instruction("{}".to_owned(), prefix)
            .err()
            .unwrap(),
        norito_decode_instruction(Uint8Array::from(vec![0_u8]), prefix)
            .err()
            .unwrap(),
        norito_encode_instruction_box_archive("{}".to_owned(), prefix)
            .err()
            .unwrap(),
        norito_decode_instruction_box_archive(Uint8Array::from(vec![0_u8]), prefix)
            .err()
            .unwrap(),
        inspect_subscription_trigger_action("bad-action".to_owned(), prefix)
            .err()
            .unwrap(),
        decode_signed_transaction_json(Uint8Array::from(vec![0_u8]), prefix)
            .err()
            .unwrap(),
        hash_instruction_batch(vec!["{}".to_owned()], prefix)
            .err()
            .unwrap(),
        validation_fee_verify_current_policy_proof_v1(
            Uint8Array::from(vec![0_u8]),
            Uint8Array::from(vec![0_u8]),
            Uint8Array::from(vec![0_u8]),
            JsU64(0),
            Uint8Array::from(vec![0_u8]),
            prefix,
        )
        .err()
        .unwrap(),
        validation_fee_verify_hijiri_quote_response_v1(
            Uint8Array::from(vec![0_u8]),
            Uint8Array::from(vec![0_u8]),
            prefix,
        )
        .err()
        .unwrap(),
        decode_lane_relay_envelope(Uint8Array::from(vec![0_u8]), prefix)
            .err()
            .unwrap(),
        verify_lane_relay_envelope_json("{}".to_owned(), prefix)
            .err()
            .unwrap(),
        lane_settlement_hash("{}".to_owned(), prefix).err().unwrap(),
        encode_contract_argument_record_json("{}".to_owned(), "{}".to_owned(), prefix)
            .err()
            .unwrap(),
    ]
}

#[test]
fn shared_codec_adapter_rejects_invalid_numeric_context_before_decoding() {
    for prefix in [
        -1.0,
        65536.0,
        0.5,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        for error in malformed_context_operations(prefix) {
            assert_eq!(error.status, napi::Status::InvalidArg);
            assert_eq!(
                error.reason,
                "network prefix must be an integer between 0 and 65535"
            );
        }
    }
}

#[test]
fn shared_codec_adapter_restores_context_after_malformed_inputs() {
    let _outer = ChainDiscriminantGuard::enter(753);
    for prefix in [369.0, 42.0] {
        let errors = malformed_context_operations(prefix);
        assert_eq!(errors.len(), 13);
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
    }
}

#[test]
fn native_lane_context_operations_preserve_the_canonical_settlement() {
    let _outer = ChainDiscriminantGuard::enter(753);
    let sample = lane_relay_envelope_sample().unwrap();
    let envelope: LaneRelayEnvelope = decode_from_bytes(sample.valid.as_ref()).unwrap();
    let expected_hash = hex::encode_upper(envelope.settlement_hash.as_ref());
    for prefix in [369.0, 42.0] {
        let decoded =
            decode_lane_relay_envelope(Uint8Array::from(sample.valid.to_vec()), prefix).unwrap();
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
        verify_lane_relay_envelope_json(decoded.clone(), prefix).unwrap();
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
        let value: json::Value = json::from_json(&decoded).unwrap();
        let settlement = json::to_json(&value["settlement_commitment"]).unwrap();
        assert_eq!(
            lane_settlement_hash(settlement, prefix).unwrap(),
            expected_hash
        );
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
    }
}

#[test]
fn contract_argument_record_requires_the_selected_account_prefix() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointArgumentFieldV1, EntrypointArgumentSchemaV1, EntrypointValueKindV1,
        EntrypointValueTypeNodeV1, EntrypointValueTypeV1,
    };
    let _outer = ChainDiscriminantGuard::enter(753);
    let key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).unwrap();
    let account = AccountId::new(key.public_key().clone());
    let schema = EntrypointArgumentSchemaV1 {
        fields: vec![EntrypointArgumentFieldV1 {
            name: "owner".to_owned(),
            ty: EntrypointValueTypeV1 {
                nodes: vec![EntrypointValueTypeNodeV1::Leaf(
                    EntrypointValueKindV1::AccountId,
                )],
            },
        }],
    };
    let schema_json = json::to_json(&schema).unwrap();
    for prefix in [369_u16, 42] {
        let value = norito_json!({ "owner": account.to_i105_for_discriminant(prefix).unwrap() });
        let payload_json = json::to_json(&value).unwrap();
        let expected = {
            let _selected = ChainDiscriminantGuard::enter(prefix);
            let payload: Json = json::from_value(value).unwrap();
            iroha_core::encode_argument_record_from_json(&schema, &payload).unwrap()
        };
        let record = encode_contract_argument_record_json(
            schema_json.clone(),
            payload_json.clone(),
            f64::from(prefix),
        )
        .unwrap();
        assert_eq!(record.as_ref(), expected.as_slice());
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
        assert!(
            encode_contract_argument_record_json(schema_json.clone(), payload_json, 753.0,)
                .is_err()
        );
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
    }
}

#[test]
fn quoted_payload_context_is_owned_only_by_its_required_authority() {
    let _outer = ChainDiscriminantGuard::enter(753);
    let key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).unwrap();
    let account = AccountId::new(key.public_key().clone());
    for prefix in [369_u16, 42] {
        let literal = account.to_i105_for_discriminant(prefix).unwrap();
        let payload_json = json::to_json(&norito_json!({ "authority": literal.clone() })).unwrap();
        {
            let _selected =
                scoped_chain_discriminant_for_transaction_payload(&payload_json).unwrap();
            assert_eq!(
                iroha_data_model::account::address::chain_discriminant(),
                prefix
            );
            assert_eq!(parse_account_id(&literal, "authority").unwrap(), account);
        }
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
    }
    for payload in [
        "{",
        "{}",
        "{\"authority\":null}",
        "{\"authority\":\"bad-account\"}",
    ] {
        assert!(scoped_chain_discriminant_for_transaction_payload(payload).is_err());
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
    }
}

#[test]
fn shared_codec_adapter_uses_explicit_network_for_instruction_and_batch_context() {
    let _outer = ChainDiscriminantGuard::enter(753);
    let key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).unwrap();
    let account = AccountId::new(key.public_key().clone());
    let literal = account.to_i105_for_discriminant(369).unwrap();
    let source = json::to_json(&norito_json!({
        "Unregister": norito_json!({ "Account": literal.clone() })
    }))
    .unwrap();
    let frame = norito_encode_instruction(source.clone(), 369.0).unwrap();
    assert_eq!(
        norito_decode_instruction(Uint8Array::from(frame.to_vec()), 369.0).unwrap(),
        source
    );
    let archive = norito_encode_instruction_box_archive(source.clone(), 369.0).unwrap();
    assert_eq!(
        norito_decode_instruction_box_archive(Uint8Array::from(archive.to_vec()), 369.0).unwrap(),
        source
    );
    let batch = hash_instruction_batch(vec![source.clone()], 369.0).unwrap();
    let expected = {
        let _selected = ChainDiscriminantGuard::enter(369);
        HashOf::new(&vec![instruction_from_json(&source).unwrap()])
    };
    assert_eq!(batch.as_ref(), expected.as_ref());
    assert!(norito_encode_instruction(source.clone(), 753.0).is_err());
    assert!(norito_encode_instruction_box_archive(source.clone(), 753.0).is_err());
    assert!(hash_instruction_batch(vec![source], 753.0).is_err());
    assert_eq!(
        iroha_data_model::account::address::chain_discriminant(),
        753
    );
}

#[test]
fn shared_codec_adapter_scopes_existing_literal_owned_native_helpers() {
    let _outer = ChainDiscriminantGuard::enter(753);
    let key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).unwrap();
    let account = AccountId::new(key.public_key().clone());
    for prefix in [369, 42] {
        let literal = account.to_i105_for_discriminant(prefix).unwrap();
        let asset =
            encode_asset_id("62Fk4FPcMuLvW5QjDGNF2a4jAmjM".to_owned(), literal.clone()).unwrap();
        assert!(asset.contains(&literal));
        {
            let _selected = scoped_chain_discriminant_for_literal(&literal).unwrap();
            assert_eq!(parse_account_id(&literal, "account").unwrap(), account);
            assert!(
                parse_account_id(
                    &account.to_i105_for_discriminant(753).unwrap(),
                    "other account"
                )
                .is_err()
            );
        }
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
    }
    assert!(scoped_chain_discriminant_for_literal("bad-account").is_err());
    assert_eq!(
        iroha_data_model::account::address::chain_discriminant(),
        753
    );
}
