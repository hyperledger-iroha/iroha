//! Account admission, strict instruction contracts and canonical archive regressions.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::{AccountId, NewAccount, address::AccountAddress},
    escrow::EscrowId,
    isi::{InstructionBox, escrow::CancelAssetLock},
};
use iroha_primitives::numeric::Quantity;
use norito::{
    codec::Encode,
    json::{self, Value},
};

use super::*;

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
        let error = encode(&text(value)).expect_err("strict admission must reject input");
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
        let decoded = decode_instruction_frame(&frame).expect("decode golden frame");
        assert_eq!(
            encode_instruction_frame(&decoded).expect("encode frame"),
            frame
        );
        assert_eq!(
            encode_instruction_archive(&decoded).expect("encode archive"),
            archive
        );
        assert_eq!(
            decode_instruction_archive(&archive).expect("decode archive"),
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
    let frame = encode_instruction_frame(&input).expect("canonical frame");
    let archive = encode_instruction_archive(&input).expect("canonical archive");
    let outer = norito::core::DecodeFlagsGuard::enter(0);
    assert_eq!(
        encode_instruction_frame(&input).expect("frame under ambient flags"),
        frame
    );
    assert_eq!(norito::core::get_decode_flags(), 0);
    assert_eq!(
        encode_instruction_archive(&input).expect("archive under ambient flags"),
        archive
    );
    assert_eq!(norito::core::get_decode_flags(), 0);
    decode_instruction_archive(&archive).expect("decode under ambient flags");
    assert_eq!(norito::core::get_decode_flags(), 0);
    decode_instruction_frame(&frame).expect("decode frame under ambient flags");
    assert_eq!(norito::core::get_decode_flags(), 0);
    assert!(decode_instruction_frame(&[1]).is_err());
    assert_eq!(norito::core::get_decode_flags(), 0);
    assert!(decode_instruction_archive(&[1]).is_err());
    assert_eq!(norito::core::get_decode_flags(), 0);
    assert!(encode_instruction_frame("{").is_err());
    assert_eq!(norito::core::get_decode_flags(), 0);
    drop(outer);
}

#[test]
fn archive_admission_rejects_frames_trailing_bytes_and_noncanonical_inner_layout() {
    let input = text(&object([("CancelAssetLock", cancel_payload())]));
    let archive = encode_instruction_archive(&input).expect("archive");
    assert!(decode_instruction_archive(&encode_instruction_frame(&input).expect("frame")).is_err());
    assert!(decode_instruction_archive(&archive[..archive.len() - 1]).is_err());
    let mut trailing = archive.clone();
    trailing.push(0);
    assert!(decode_instruction_archive(&trailing).is_err());
    assert!(archive[0] < 0x80, "fixture first length fits one byte");
    let mut overlong_length = vec![archive[0] | 0x80, 0];
    overlong_length.extend_from_slice(&archive[1..]);
    assert!(decode_instruction_archive(&overlong_length).is_err());
    let mut wrong_schema = archive;
    let frame_offset = wrong_schema
        .windows(4)
        .position(|window| window == b"NRT0")
        .expect("inner frame");
    // Schema identity is in the header, outside the payload CRC. A checksum-only
    // decoder would miss this mutation.
    wrong_schema[frame_offset + 6] ^= 1;
    assert!(decode_instruction_archive(&wrong_schema).is_err());
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
fn register_account_defaults_remain_valid_but_unknown_envelopes_do_not() {
    let account_json = json::to_value(&NewAccount::new(account())).expect("new account JSON");
    let defaults = object([(
        "Register",
        object([(
            "Account",
            object([("id", account_json.get("id").expect("account id").clone())]),
        )]),
    )]);
    encode_instruction_frame(&text(&defaults)).expect("optional account defaults");
    let mut unknown = account_json.clone();
    unknown
        .as_object_mut()
        .expect("account object")
        .insert("unexpected".to_owned(), Value::Bool(true));
    assert_strict_rejection(&object([("Register", object([("Account", unknown)]))]));
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
        decode_instruction_frame(&encode_instruction_frame(&source).expect("frame"))
            .expect("frame JSON"),
        decode_instruction_archive(&encode_instruction_archive(&source).expect("archive"))
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
