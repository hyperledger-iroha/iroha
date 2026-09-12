//! Account admission, strict instruction contracts and canonical archive regressions.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::{AccountId, NewAccount, address::AccountAddress},
    escrow::EscrowId,
    isi::{InstructionBox, escrow::CancelAssetLock},
    smart_contract::manifest::ContractManifest,
};
use iroha_primitives::numeric::Quantity;
use norito::{
    codec::Encode,
    json::{self, Value},
};

use super::*;

const FIXTURE_NETWORK_PREFIX: u16 = 753;

fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

fn account() -> AccountId {
    let key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).expect("fixture key");
    AccountId::new(key.public_key().clone())
}

fn text(value: &Value) -> String {
    json::to_json(value).expect("fixture JSON")
}

fn cancel_payload() -> Value {
    json::to_value(&CancelAssetLock::new(
        EscrowId::new(Hash::new(b"shared-codec-cancel")),
        Quantity::from(15_u32),
    ))
    .expect("cancel fixture")
}

fn assert_strict_rejection(value: &Value) {
    for encode in [encode_instruction_frame, encode_instruction_archive] {
        let error = encode(&text(value), FIXTURE_NETWORK_PREFIX)
            .expect_err("strict admission must reject input");
        assert_eq!(error.kind(), CodecErrorKind::InvalidArgument, "{error}");
    }
}

#[test]
fn account_admission_preserves_discriminants_and_rejects_malformed_controllers() {
    let native = AccountAddress::from_account_id(&account()).expect("native account");
    let canonical = hex::decode(
        native
            .canonical_hex()
            .expect("canonical hex")
            .trim_start_matches("0x"),
    )
    .expect("controller bytes");
    for prefix in [369, 42] {
        let rendered = account_address_render(&canonical, prefix).expect("render account");
        assert_eq!(
            rendered.i105,
            native
                .to_i105_for_discriminant(prefix)
                .expect("native I105")
        );
        assert_eq!(
            rendered.canonical_hex,
            native.canonical_hex().expect("native hex")
        );
        let parsed = account_address_parse_encoded(&rendered.i105, None).expect("infer prefix");
        assert_eq!(parsed.canonical_bytes, canonical);
        assert_eq!(parsed.network_prefix, prefix);
        let error = account_address_parse_encoded(&rendered.i105, Some(prefix + 1)).unwrap_err();
        let native_error =
            AccountAddress::parse_encoded(&rendered.i105, Some(prefix + 1)).unwrap_err();
        assert_eq!(error.kind(), CodecErrorKind::InvalidArgument);
        assert_eq!(
            error.reason(),
            format!("{}: {native_error}", native_error.code_str())
        );
        assert!(account_address_parse_encoded(&format!(" {}", rendered.i105), None).is_err());
    }
    for first_byte in [0, 1] {
        let mut malformed = vec![0x02, 0, 1, 32];
        malformed.resize(36, 0);
        malformed[4] = first_byte;
        assert_eq!(
            account_address_render(&malformed, 369).unwrap_err().kind(),
            CodecErrorKind::InvalidArgument
        );
    }
    let mut trailing = canonical;
    trailing.push(0);
    assert!(account_address_render(&trailing, 369).is_err());
}

#[test]
fn instruction_frames_and_archives_preserve_existing_golden_bytes() {
    let _network = ChainDiscriminantGuard::enter(FIXTURE_NETWORK_PREFIX);
    for source in [
        include_str!("../../../fixtures/norito_instructions/mint_asset_quantity.json"),
        include_str!("../../../fixtures/norito_instructions/burn_asset_fractional.json"),
        include_str!("../../../fixtures/norito_instructions/burn_trigger_repetitions.json"),
    ] {
        let fixture: Value = json::from_json(source).expect("golden fixture");
        let frame = STANDARD
            .decode(
                fixture
                    .get("instruction")
                    .and_then(Value::as_str)
                    .expect("frame base64"),
            )
            .expect("frame bytes");
        let archive = hex::decode(
            fixture
                .get("encoded_hex")
                .and_then(Value::as_str)
                .expect("archive hex"),
        )
        .expect("archive bytes");
        let decoded =
            decode_instruction_frame(&frame, FIXTURE_NETWORK_PREFIX).expect("decode golden frame");
        assert_eq!(
            encode_instruction_frame(&decoded, FIXTURE_NETWORK_PREFIX).expect("encode frame"),
            frame
        );
        assert_eq!(
            encode_instruction_archive(&decoded, FIXTURE_NETWORK_PREFIX).expect("encode archive"),
            archive
        );
        assert_eq!(
            decode_instruction_archive(&archive, FIXTURE_NETWORK_PREFIX).expect("decode archive"),
            decoded
        );
        let native = instruction_from_json(&decoded).expect("native instruction");
        assert_eq!(archive, native.encode());
        assert_eq!(
            frame,
            norito::encode_canonical(&native).expect("native frame")
        );
    }
}

#[test]
fn canonical_codec_restores_ambient_layout_on_success_and_error() {
    let input = text(&object([("CancelAssetLock", cancel_payload())]));
    let frame = encode_instruction_frame(&input, FIXTURE_NETWORK_PREFIX).expect("canonical frame");
    let archive =
        encode_instruction_archive(&input, FIXTURE_NETWORK_PREFIX).expect("canonical archive");
    let outer = norito::core::DecodeFlagsGuard::enter(0);
    assert_eq!(
        encode_instruction_frame(&input, FIXTURE_NETWORK_PREFIX)
            .expect("frame under ambient flags"),
        frame
    );
    assert_eq!(norito::core::get_decode_flags(), 0);
    assert_eq!(
        encode_instruction_archive(&input, FIXTURE_NETWORK_PREFIX)
            .expect("archive under ambient flags"),
        archive
    );
    assert_eq!(norito::core::get_decode_flags(), 0);
    decode_instruction_archive(&archive, FIXTURE_NETWORK_PREFIX)
        .expect("decode under ambient flags");
    assert_eq!(norito::core::get_decode_flags(), 0);
    decode_instruction_frame(&frame, FIXTURE_NETWORK_PREFIX)
        .expect("decode frame under ambient flags");
    assert_eq!(norito::core::get_decode_flags(), 0);
    assert!(decode_instruction_frame(&[1], FIXTURE_NETWORK_PREFIX).is_err());
    assert_eq!(norito::core::get_decode_flags(), 0);
    assert!(decode_instruction_archive(&[1], FIXTURE_NETWORK_PREFIX).is_err());
    assert_eq!(norito::core::get_decode_flags(), 0);
    assert!(encode_instruction_frame("{", FIXTURE_NETWORK_PREFIX).is_err());
    assert_eq!(norito::core::get_decode_flags(), 0);
    drop(outer);
}

#[test]
fn archive_admission_rejects_frames_trailing_bytes_and_noncanonical_inner_layout() {
    let input = text(&object([("CancelAssetLock", cancel_payload())]));
    let archive = encode_instruction_archive(&input, FIXTURE_NETWORK_PREFIX).expect("archive");
    assert!(
        decode_instruction_archive(
            &encode_instruction_frame(&input, FIXTURE_NETWORK_PREFIX).expect("frame"),
            FIXTURE_NETWORK_PREFIX
        )
        .is_err()
    );
    assert!(
        decode_instruction_archive(&archive[..archive.len() - 1], FIXTURE_NETWORK_PREFIX).is_err()
    );
    let mut trailing = archive.clone();
    trailing.push(0);
    assert!(decode_instruction_archive(&trailing, FIXTURE_NETWORK_PREFIX).is_err());
    assert!(archive[0] < 0x80, "fixture first length fits one byte");
    let mut overlong_length = vec![archive[0] | 0x80, 0];
    overlong_length.extend_from_slice(&archive[1..]);
    assert!(decode_instruction_archive(&overlong_length, FIXTURE_NETWORK_PREFIX).is_err());
    let mut wrong_schema = archive;
    let frame_offset = wrong_schema
        .windows(4)
        .position(|window| window == b"NRT0")
        .expect("inner frame");
    // Schema identity is in the header, outside the payload CRC. A checksum-only
    // decoder would miss this mutation.
    wrong_schema[frame_offset + 6] ^= 1;
    assert!(decode_instruction_archive(&wrong_schema, FIXTURE_NETWORK_PREFIX).is_err());
}

#[test]
fn cancel_lock_strict_fields_and_quantities_survive_extraction() {
    let canonical = cancel_payload();
    for missing in ["escrow_id", "expected_remaining_amount"] {
        let mut payload = canonical.clone();
        payload.as_object_mut().expect("payload").remove(missing);
        assert_strict_rejection(&object([("CancelAssetLock", payload)]));
    }
    for quantity in [
        Value::String("0".to_owned()),
        Value::String("-1".to_owned()),
        Value::String("01".to_owned()),
        Value::String("1.0".to_owned()),
        Value::from(15_u64),
    ] {
        let mut payload = canonical.clone();
        payload
            .as_object_mut()
            .expect("payload")
            .insert("expected_remaining_amount".to_owned(), quantity);
        assert_strict_rejection(&object([("CancelAssetLock", payload)]));
    }
    let mut unknown = canonical.clone();
    unknown
        .as_object_mut()
        .expect("payload")
        .insert("unexpected".to_owned(), Value::Bool(true));
    assert_strict_rejection(&object([("CancelAssetLock", unknown)]));
    let mut noncanonical_hash = canonical.clone();
    let lowercase_hash = noncanonical_hash
        .get("escrow_id")
        .and_then(Value::as_str)
        .expect("canonical hash")
        .to_ascii_lowercase();
    noncanonical_hash
        .as_object_mut()
        .expect("payload")
        .insert("escrow_id".to_owned(), Value::String(lowercase_hash));
    assert_strict_rejection(&object([("CancelAssetLock", noncanonical_hash)]));
    assert_strict_rejection(&object([
        ("CancelAssetLock", canonical),
        ("unexpected", Value::Null),
    ]));
}

#[test]
fn register_account_requires_metadata_and_rejects_unknown_envelopes() {
    let _network = ChainDiscriminantGuard::enter(FIXTURE_NETWORK_PREFIX);
    let account_json = json::to_value(&NewAccount::new(account())).expect("new account JSON");
    let registration = object([("Register", object([("Account", account_json.clone())]))]);
    let canonical = text(&registration);
    encode_instruction_frame(&canonical, FIXTURE_NETWORK_PREFIX)
        .expect("complete canonical account registration");
    encode_instruction_archive(&canonical, FIXTURE_NETWORK_PREFIX)
        .expect("complete canonical account archive");
    let mut missing_metadata = account_json.clone();
    missing_metadata
        .as_object_mut()
        .expect("account object")
        .remove("metadata");
    for encode in [encode_instruction_frame, encode_instruction_archive] {
        let error = encode(
            &text(&object([(
                "Register",
                object([("Account", missing_metadata.clone())]),
            )])),
            FIXTURE_NETWORK_PREFIX,
        )
        .expect_err("native account requires metadata");
        assert_eq!(error.kind(), CodecErrorKind::Failure);
        assert!(error.reason().contains("metadata"));
    }
    let mut unknown = account_json.clone();
    unknown
        .as_object_mut()
        .expect("account object")
        .insert("unexpected".to_owned(), Value::Bool(true));
    for encode in [encode_instruction_frame, encode_instruction_archive] {
        let error = encode(
            &text(&object([(
                "Register",
                object([("Account", unknown.clone())]),
            )])),
            FIXTURE_NETWORK_PREFIX,
        )
        .expect_err("native account rejects unknown fields");
        assert_eq!(error.kind(), CodecErrorKind::Failure);
    }
    assert_strict_rejection(&object([(
        "Register",
        object([("Account", account_json.clone()), ("Domain", Value::Null)]),
    )]));
    assert_strict_rejection(&object([
        ("Register", object([("Account", account_json)])),
        ("SetParameter", Value::Null),
    ]));
    assert_strict_rejection(&object([("Register", Value::Null)]));
}

#[test]
fn nested_custom_multisig_payload_is_preserved_by_both_encodings() {
    let _network = ChainDiscriminantGuard::enter(FIXTURE_NETWORK_PREFIX);
    let literal = account().canonical_i105().expect("account I105");
    let payload = object([(
        "Propose",
        object([
            ("account", Value::String(literal)),
            (
                "instructions",
                Value::Array(vec![object([(
                    "Log",
                    Value::String("nested instruction".to_owned()),
                )])]),
            ),
            ("transaction_ttl_ms", Value::from(30_000_u64)),
        ]),
    )]);
    let input = object([("Custom", object([("payload", payload)]))]);
    let source = text(&input);
    for decoded in [
        decode_instruction_frame(
            &encode_instruction_frame(&source, FIXTURE_NETWORK_PREFIX).expect("frame"),
            FIXTURE_NETWORK_PREFIX,
        )
        .expect("frame JSON"),
        decode_instruction_archive(
            &encode_instruction_archive(&source, FIXTURE_NETWORK_PREFIX).expect("archive"),
            FIXTURE_NETWORK_PREFIX,
        )
        .expect("archive JSON"),
    ] {
        assert_eq!(
            json::from_json::<Value>(&decoded).expect("decoded JSON"),
            input
        );
    }
    let native: InstructionBox = instruction_from_json(&source).expect("native custom instruction");
    assert!(
        native
            .as_any()
            .downcast_ref::<iroha_data_model::isi::CustomInstruction>()
            .is_some()
    );
}

fn assert_typed_instruction_roundtrip(instruction: InstructionBox, value: Value) {
    let input = text(&value);
    let native_frame = norito::encode_canonical(&instruction).expect("native instruction frame");
    let native_archive = instruction.encode();
    assert_eq!(
        encode_instruction_frame(&input, FIXTURE_NETWORK_PREFIX).expect("encode typed frame"),
        native_frame
    );
    assert_eq!(
        encode_instruction_archive(&input, FIXTURE_NETWORK_PREFIX).expect("encode typed archive"),
        native_archive
    );
    for decoded in [
        decode_instruction_frame(&native_frame, FIXTURE_NETWORK_PREFIX)
            .expect("decode typed frame"),
        decode_instruction_archive(&native_archive, FIXTURE_NETWORK_PREFIX)
            .expect("decode typed archive"),
    ] {
        assert_eq!(
            json::from_json::<Value>(&decoded).expect("decoded JSON"),
            value
        );
        assert_eq!(
            encode_instruction_frame(&decoded, FIXTURE_NETWORK_PREFIX)
                .expect("reencode typed frame"),
            native_frame
        );
        assert_eq!(
            encode_instruction_archive(&decoded, FIXTURE_NETWORK_PREFIX)
                .expect("reencode typed archive"),
            native_archive
        );
    }
    let Value::Object(fields) = &value else {
        panic!("instruction envelope");
    };
    let mut extra_envelope = fields.clone();
    extra_envelope.insert("unrecognized_instruction".to_owned(), Value::Null);
    assert_strict_rejection(&Value::Object(extra_envelope));
    let (name, payload) = fields.first_key_value().expect("one instruction");
    let mut extra_payload = payload.as_object().expect("instruction payload").clone();
    extra_payload.insert("unrecognized_field".to_owned(), Value::Null);
    assert_strict_rejection(&object([(name.as_str(), Value::Object(extra_payload))]));
}

#[test]
fn all_browser_contract_deployment_instructions_roundtrip_exact_native_bytes() {
    let code_hash = Hash::new(b"browser-deployment-code");
    let address = "irohac1qyqqqqqqqqqqqq8y2pcrtkxvkrn5nt74kjjkjcst6kc56qcqa2dqp"
        .parse()
        .expect("canonical contract address");
    let manifest = ContractManifest {
        seiyaku_name: None,
        code_hash: Some(code_hash),
        abi_hash: Some(Hash::new(b"browser-deployment-abi")),
        compiler_fingerprint: Some("codec-fixture".to_owned()),
        features_bitmap: Some(0),
        access_set_hints: None,
        entrypoints: None,
        states: None,
        kotoba: None,
        error_types: None,
        provenance: None,
    };
    let instructions: Vec<InstructionBox> = vec![
        Box::new(UploadSmartContractCodeChunk {
            code_hash,
            total_size: 4,
            chunk_index: 0,
            chunk_count: 1,
            chunk: vec![1, 2, 3, 4],
        })
        .into_instruction_box(),
        Box::new(FinalizeSmartContractCodeUpload {
            code_hash,
            total_size: 4,
            chunk_count: 1,
        })
        .into_instruction_box(),
        Box::new(CancelSmartContractCodeUpload { code_hash }).into_instruction_box(),
        Box::new(RegisterSmartContractCode { manifest }).into_instruction_box(),
        Box::new(CommitContractDeployment {
            expected_deploy_nonce: u64::MAX,
            contract_address: address,
            code_hash,
            contract_alias: "demo::universal".parse().expect("contract alias"),
            lease_expiry_ms: Some(u64::MAX),
            expected_previous_contract_address: None,
        })
        .into_instruction_box(),
    ];
    for instruction in instructions {
        let value = instruction_to_json_value(&instruction).expect("deployment JSON");
        assert_typed_instruction_roundtrip(instruction, value);
    }
    let cancel = object([(
        "CancelSmartContractCodeUpload",
        object([("code_hash", json::to_value(&code_hash).expect("hash JSON"))]),
    )]);
    let proposed = custom_json_value(object([(
        "Propose",
        object([("instructions", Value::Array(vec![cancel.clone()]))]),
    )]));
    let decoded = decode_instruction_archive(
        &encode_instruction_archive(&text(&proposed), FIXTURE_NETWORK_PREFIX).unwrap(),
        FIXTURE_NETWORK_PREFIX,
    )
    .unwrap();
    assert_eq!(json::from_json::<Value>(&decoded).unwrap(), proposed);
    assert_strict_rejection(&object([("CancelSmartContractCodeUpload", object([]))]));
    assert_strict_rejection(&object([(
        "CancelSmartContractCodeUpload",
        object([("code_hash", Value::String("not-a-hash".to_owned()))]),
    )]));
}

fn fixture_value(fixture: &Value, name: &str) -> Value {
    fixture["vectors"]
        .as_array()
        .expect("fixture vectors")
        .iter()
        .find(|row| row["name"].as_str() == Some(name))
        .expect(name)["value"]
        .clone()
}

#[test]
fn all_browser_game_instruction_families_roundtrip_native_json() {
    let _network = ChainDiscriminantGuard::enter(FIXTURE_NETWORK_PREFIX);
    let fixture: Value = json::from_json(include_str!(
        "../../../javascript/iroha_js/test/fixtures/game-v1-codec.json"
    ))
    .expect("native game fixture");
    let checkpoint = fixture_value(&fixture, "SignedGameCheckpointV1");
    let session = checkpoint["checkpoint"]["session_id"].clone();
    let signature = checkpoint["signatures"][0]["signature"].clone();
    let commitment = fixture_value(&fixture, "GameCommitmentSetBodyV1");
    let proof = fixture_value(&fixture, "ExecutionProofEnvelopeV1");
    let mut frontier = commitment.as_object().unwrap().clone();
    frontier.insert("signatures".to_owned(), checkpoint["signatures"].clone());
    let values = [
        (
            "OpenGameSessionV1",
            fixture_value(&fixture, "OpenGameSessionV1"),
        ),
        (
            "JoinGameSessionV1",
            fixture_value(&fixture, "JoinGameSessionV1"),
        ),
        (
            "StartGameSessionV1",
            object([("session_id", session.clone())]),
        ),
        (
            "CommitGameCheckpointV1",
            object([
                ("session_id", session.clone()),
                ("checkpoint", checkpoint),
                ("frontier", Value::Object(frontier)),
            ]),
        ),
        (
            "ChallengeGameSessionV1",
            object([
                ("session_id", session.clone()),
                ("epoch", Value::from(2_u64)),
                ("slot", Value::from(0_u64)),
                ("signature", signature.clone()),
            ]),
        ),
        (
            "CommitGameInputsV1",
            object([(
                "input",
                object([
                    ("session_id", session.clone()),
                    ("epoch", Value::from(2_u64)),
                    ("start_tick", Value::from(0_u64)),
                    ("slot", Value::from(0_u64)),
                    ("commitment", commitment["commitments"][0].clone()),
                    ("signature", signature),
                ]),
            )]),
        ),
        (
            "RevealGameInputsV1",
            object([("reveal", fixture_value(&fixture, "GameInputRevealV1"))]),
        ),
        (
            "AdvanceGameDeadlineV1",
            object([("session_id", session.clone())]),
        ),
        (
            "SettleGameSessionV1",
            object([
                ("session_id", session.clone()),
                ("proof", proof.clone()),
                ("outcome", fixture_value(&fixture, "GameOutcomeV1")),
            ]),
        ),
        ("ExpireGameSessionV1", object([("session_id", session)])),
        (
            "ClaimGamePayoutV1",
            fixture_value(&fixture, "ClaimGamePayoutV1"),
        ),
        (
            "StakeGameItemV1",
            fixture_value(&fixture, "StakeGameItemV1"),
        ),
        (
            "RegisterExecutionProofProfileV1",
            object([("profile_id", proof["profile_id"].clone())]),
        ),
        ("VerifyExecutionProofV1", object([("proof", proof)])),
    ];
    for (name, payload) in values {
        macro_rules! native_from_catalog {
            ($($variant:ident => $ty:ty,)*) => {
                match name {
                    $(stringify!($variant) => {
                        let typed: $ty = json::from_value(payload.clone()).expect(name);
                        Box::new(typed).into_instruction_box()
                    },)*
                    _ => panic!("fixture outside explicit catalog: {name}"),
                }
            };
        }
        let instruction = typed_browser_instruction_catalog!(native_from_catalog);
        assert_typed_instruction_roundtrip(instruction, object([(name, payload)]));
    }
}

#[test]
fn all_browser_nft_market_instruction_families_preserve_native_fixture_frames() {
    let _network = ChainDiscriminantGuard::enter(FIXTURE_NETWORK_PREFIX);
    let fixture: Value = json::from_json(include_str!(
        "../../../javascript/iroha_js/test/fixtures/nft-market-v1-codec.json"
    ))
    .expect("native NFT fixture");
    for name in ["OfferNftV1", "BuyNftV1", "CancelNftOfferV1"] {
        let payload = fixture_value(&fixture, name);
        let row = fixture["vectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"].as_str() == Some(name))
            .unwrap();
        let frame = hex::decode(row["framed_hex"].as_str().unwrap()).expect("native NFT frame");
        let wire_id = format!("iroha.instruction.v1::nft_market::{name}");
        let instruction = iroha_data_model::isi::decode_instruction_from_pair(&wire_id, &frame)
            .expect("native NFT instruction");
        assert_typed_instruction_roundtrip(instruction, object([(name, payload)]));
    }
}

#[test]
fn browser_kagemusha_top_up_roundtrips_existing_native_identity_fixture() {
    let _network = ChainDiscriminantGuard::enter(FIXTURE_NETWORK_PREFIX);
    let fixtures: Value = json::from_json(include_str!(
        "../../../crates/iroha_data_model/tests/fixtures/instruction_record_generated_identity_frames.json"
    )).expect("native instruction identity fixtures");
    let row = fixtures
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["nominal"].as_str() == Some("iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1")
        })
        .unwrap();
    let frame =
        hex::decode(row["cases"][0]["frame"].as_str().unwrap()).expect("native top-up frame");
    let instruction = iroha_data_model::isi::decode_instruction_from_pair(
        iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1::WIRE_ID,
        &frame,
    )
    .expect("native top-up instruction");
    let value = instruction_to_json_value(&instruction).expect("native top-up JSON");
    assert_typed_instruction_roundtrip(instruction, value);
}

#[test]
fn set_parameter_explicit_json_roundtrips_through_both_native_encodings() {
    let parameter = CustomParameter::new(
        "codec_fixture".parse().expect("parameter name"),
        Json::new(object([("enabled", Value::Bool(true))])),
    );
    let instruction = InstructionBox::from(SetParameter::new(Parameter::Custom(parameter)));
    let value = instruction_to_json_value(&instruction).expect("parameter JSON");
    let input = text(&value);
    let frame = encode_instruction_frame(&input, FIXTURE_NETWORK_PREFIX).expect("parameter frame");
    let archive =
        encode_instruction_archive(&input, FIXTURE_NETWORK_PREFIX).expect("parameter archive");
    assert_eq!(
        decode_instruction_frame(&frame, FIXTURE_NETWORK_PREFIX).unwrap(),
        input
    );
    assert_eq!(
        decode_instruction_archive(&archive, FIXTURE_NETWORK_PREFIX).unwrap(),
        input
    );
    assert_eq!(archive, instruction.encode());
}

#[test]
fn structured_asset_holding_limit_roundtrips_the_existing_model_json_contract() {
    let _network = ChainDiscriminantGuard::enter(FIXTURE_NETWORK_PREFIX);
    let instruction = InstructionBox::from(
        iroha_data_model::isi::asset_transfer_control::SetAssetHoldingLimit::new(
            account(),
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
                .parse()
                .expect("native fixture asset"),
            Some(Quantity::from(5_u32)),
        ),
    );
    let value = instruction_to_json_value(&instruction).expect("holding limit JSON");
    let input = text(&value);
    let archive =
        encode_instruction_archive(&input, FIXTURE_NETWORK_PREFIX).expect("holding limit archive");
    assert_eq!(archive, instruction.encode());
    assert_eq!(
        decode_instruction_archive(&archive, FIXTURE_NETWORK_PREFIX).unwrap(),
        input
    );
    let frame = encode_instruction_frame(&input, FIXTURE_NETWORK_PREFIX).unwrap();
    assert_eq!(
        decode_instruction_frame(&frame, FIXTURE_NETWORK_PREFIX).unwrap(),
        input
    );
}

#[test]
fn deployment_json_rejects_noncanonical_integer_and_incomplete_payloads() {
    let valid = instruction_to_json_value(
        &Box::new(FinalizeSmartContractCodeUpload {
            code_hash: Hash::new(b"strict-upload"),
            total_size: 4,
            chunk_count: 1,
        })
        .into_instruction_box(),
    )
    .unwrap();
    for replacement in ["01", "+4", " 4", "18446744073709551616"] {
        let mut payload = valid["FinalizeSmartContractCodeUpload"]
            .as_object()
            .unwrap()
            .clone();
        payload.insert(
            "total_size".to_owned(),
            Value::String(replacement.to_owned()),
        );
        assert_strict_rejection(&object([(
            "FinalizeSmartContractCodeUpload",
            Value::Object(payload),
        )]));
    }
    let payload = valid["FinalizeSmartContractCodeUpload"]
        .as_object()
        .unwrap();
    for key in payload.keys() {
        let mut missing = payload.clone();
        missing.remove(key);
        assert_strict_rejection(&object([(
            "FinalizeSmartContractCodeUpload",
            Value::Object(missing),
        )]));
    }
}

#[test]
fn checked_network_prefix_rejects_numeric_coercion() {
    for prefix in [
        -1.0,
        65536.0,
        0.5,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert_eq!(
            checked_network_prefix(prefix).unwrap_err().kind(),
            CodecErrorKind::InvalidArgument
        );
    }
    for prefix in [0_u16, 42, 369, 753, u16::MAX] {
        assert_eq!(checked_network_prefix(f64::from(prefix)).unwrap(), prefix);
    }
}

#[test]
fn instruction_account_helper_requires_exact_literals_in_the_selected_context() {
    let _outer = ChainDiscriminantGuard::enter(1234);
    let account = account();
    for prefix in [0_u16, 42, 369, 753, u16::MAX] {
        {
            let _selected = ChainDiscriminantGuard::enter(prefix);
            let literal = account.to_i105_for_discriminant(prefix).unwrap();
            assert_eq!(parse_account_id(&literal, "operand").unwrap(), account);
            for invalid in [
                account
                    .to_i105_for_discriminant(prefix.wrapping_add(1))
                    .unwrap(),
                format!(" {literal}"),
                format!("{literal}\n"),
                account.to_canonical_hex().unwrap(),
                "account@universal".to_owned(),
            ] {
                let error = parse_account_id(&invalid, "operand").unwrap_err();
                assert_eq!(error.kind(), CodecErrorKind::InvalidArgument);
                assert!(error.reason().starts_with("invalid operand: "));
                assert_eq!(chain_discriminant(), prefix);
            }
        }
        assert_eq!(chain_discriminant(), 1234);
    }
}

#[test]
fn instruction_network_context_is_explicit_and_restored_on_success_and_failure() {
    let _outer = ChainDiscriminantGuard::enter(1234);
    let account = account();
    let mut identity_bytes = None;
    for prefix in [0_u16, 42, 369, 753, u16::MAX] {
        let literal = account.to_i105_for_discriminant(prefix).unwrap();
        let source = text(&object([(
            "Unregister",
            object([("Account", Value::String(literal))]),
        )]));
        let frame = encode_instruction_frame(&source, prefix).unwrap();
        assert_eq!(chain_discriminant(), 1234);
        let archive = encode_instruction_archive(&source, prefix).unwrap();
        assert_eq!(chain_discriminant(), 1234);
        assert_eq!(decode_instruction_frame(&frame, prefix).unwrap(), source);
        assert_eq!(chain_discriminant(), 1234);
        assert_eq!(
            decode_instruction_archive(&archive, prefix).unwrap(),
            source
        );
        assert_eq!(chain_discriminant(), 1234);
        if let Some((expected_frame, expected_archive)) = &identity_bytes {
            assert_eq!(
                &frame, expected_frame,
                "display prefix cannot alter domainless identity bytes"
            );
            assert_eq!(&archive, expected_archive);
        } else {
            identity_bytes = Some((frame.clone(), archive.clone()));
        }
        let other = prefix.wrapping_add(1);
        let other_source = text(&object([(
            "Unregister",
            object([(
                "Account",
                Value::String(account.to_i105_for_discriminant(other).unwrap()),
            )]),
        )]));
        assert_eq!(
            decode_instruction_frame(&frame, other).unwrap(),
            other_source
        );
        assert_eq!(chain_discriminant(), 1234);
        assert_eq!(
            decode_instruction_archive(&archive, other).unwrap(),
            other_source
        );
        assert_eq!(chain_discriminant(), 1234);
        for encode in [encode_instruction_frame, encode_instruction_archive] {
            assert!(
                encode(&source, other).is_err(),
                "foreign network operand must fail"
            );
            assert_eq!(chain_discriminant(), 1234);
            assert!(encode("{", prefix).is_err());
            assert_eq!(chain_discriminant(), 1234);
        }
        for decode in [decode_instruction_frame, decode_instruction_archive] {
            assert!(decode(&[0], prefix).is_err());
            assert_eq!(chain_discriminant(), 1234);
        }
    }
}

#[test]
fn typed_registration_respects_selected_context_and_native_error_categories() {
    let _outer = ChainDiscriminantGuard::enter(42);
    let source = {
        let _selected = ChainDiscriminantGuard::enter(369);
        text(&object([(
            "Register",
            object([(
                "Account",
                json::to_value(&NewAccount::new(account())).unwrap(),
            )]),
        )]))
    };
    for (encode, decode) in [
        (
            encode_instruction_frame as fn(&str, u16) -> CodecResult<Vec<u8>>,
            decode_instruction_frame as fn(&[u8], u16) -> CodecResult<String>,
        ),
        (encode_instruction_archive, decode_instruction_archive),
    ] {
        let bytes = encode(&source, 369).unwrap();
        assert_eq!(chain_discriminant(), 42);
        assert_eq!(decode(&bytes, 369).unwrap(), source);
        assert_eq!(chain_discriminant(), 42);
        assert_eq!(
            encode(&source, 42).unwrap_err().kind(),
            CodecErrorKind::Failure
        );
        assert_eq!(chain_discriminant(), 42);
    }
}

#[test]
fn instruction_network_context_is_thread_local() {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = [369_u16, 42]
        .into_iter()
        .map(|prefix| {
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                let _outer = ChainDiscriminantGuard::enter(prefix.wrapping_add(10));
                let source = text(&object([(
                    "Unregister",
                    object([(
                        "Account",
                        Value::String(account().to_i105_for_discriminant(prefix).unwrap()),
                    )]),
                )]));
                barrier.wait();
                let bytes = encode_instruction_archive(&source, prefix).unwrap();
                barrier.wait();
                assert_eq!(decode_instruction_archive(&bytes, prefix).unwrap(), source);
                assert_eq!(chain_discriminant(), prefix.wrapping_add(10));
                bytes
            })
        })
        .collect();
    let bytes: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(bytes[0], bytes[1]);
}
