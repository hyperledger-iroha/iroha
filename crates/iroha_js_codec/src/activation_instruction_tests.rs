//! Native lifecycle instruction identity, exact CAS and frame/archive regressions.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    isi::instruction_wire_id,
    smart_contract::{ContractAddress, ContractLifecycleOwnerV1},
};
use norito::codec::Encode;

use super::*;
use crate::{
    decode_instruction_archive, decode_instruction_frame, encode_instruction_archive,
    encode_instruction_frame, instruction_from_json, instruction_to_json_value,
};

const ADDRESS: &str = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn text(value: &Value) -> String {
    json::to_json(value).expect("fixture JSON")
}

fn map(value: &mut Value) -> &mut json::Map {
    value.as_object_mut().expect("fixture object")
}

fn account() -> AccountId {
    let pair = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).expect("fixture key");
    AccountId::new(pair.public_key().clone())
}

// Construct the expected native instructions independently of the codec under
// test. Each JSON fixture is paired with its actual ledger type and exact fields.
fn cases(expected_revision: u64) -> Vec<(&'static str, InstructionBox, Value)> {
    let address: ContractAddress = ADDRESS.parse().expect("native contract address");
    let code_hash = Hash::new(b"activation-codec-artifact");
    let common = || {
        [
            ("contract_address", Value::String(ADDRESS.to_owned())),
            (
                "expected_revision",
                Value::String(expected_revision.to_string()),
            ),
        ]
    };
    let make = |name, extra: Vec<(&str, Value)>| {
        let fields = common()
            .into_iter()
            .chain(extra)
            .map(|(key, value)| (key.to_owned(), value))
            .collect();
        object([(name, Value::Object(fields))])
    };
    let mut cases = vec![
        (
            "ActivateContractInstance",
            ActivateContractInstance {
                contract_address: address.clone(),
                expected_revision,
                code_hash,
            }
            .into(),
            make(
                "ActivateContractInstance",
                vec![("code_hash", json::to_value(&code_hash).unwrap())],
            ),
        ),
        (
            "DeactivateContractInstance",
            DeactivateContractInstance {
                contract_address: address.clone(),
                expected_revision,
                reason: None,
            }
            .into(),
            make("DeactivateContractInstance", vec![("reason", Value::Null)]),
        ),
        (
            "SetContractParliamentDelegation",
            SetContractParliamentDelegation {
                contract_address: address.clone(),
                expected_revision,
                delegated: true,
            }
            .into(),
            make(
                "SetContractParliamentDelegation",
                vec![("delegated", Value::Bool(true))],
            ),
        ),
        (
            "AcceptContractOwnership",
            AcceptContractOwnership {
                contract_address: address.clone(),
                expected_revision,
            }
            .into(),
            make("AcceptContractOwnership", vec![]),
        ),
        (
            "CancelContractOwnershipOffer",
            CancelContractOwnershipOffer {
                contract_address: address.clone(),
                expected_revision,
            }
            .into(),
            make("CancelContractOwnershipOffer", vec![]),
        ),
    ];
    for (new_owner, owner_json) in [
        (
            ContractLifecycleOwnerV1::Account(account()),
            object([
                ("owner", Value::String("Account".into())),
                ("value", json::to_value(&account()).unwrap()),
            ]),
        ),
        (
            ContractLifecycleOwnerV1::Parliament,
            object([
                ("owner", Value::String("Parliament".into())),
                ("value", Value::Null),
            ]),
        ),
    ] {
        cases.push((
            "OfferContractOwnership",
            OfferContractOwnership {
                contract_address: address.clone(),
                expected_revision,
                new_owner,
            }
            .into(),
            make("OfferContractOwnership", vec![("new_owner", owner_json)]),
        ));
    }
    cases
}

fn assert_roundtrip(name: &str, expected: &InstructionBox, value: &Value) {
    let source = text(value);
    assert!(is_activation_instruction(expected));
    assert_eq!(
        instruction_wire_id(expected),
        Some(format!("iroha.instruction.v1::smart_contract_code::{name}").as_str())
    );
    assert_eq!(instruction_from_json(&source).unwrap(), *expected);
    assert_eq!(instruction_to_json_value(expected).unwrap(), *value);
    let frame = norito::encode_canonical(expected).expect("native public frame");
    let archive = expected.encode();
    assert_eq!(encode_instruction_frame(&source).unwrap(), frame);
    assert_eq!(encode_instruction_archive(&source).unwrap(), archive);
    for decoded in [
        decode_instruction_frame(&frame),
        decode_instruction_archive(&archive),
    ] {
        let decoded: Value = json::from_json(&decoded.unwrap()).unwrap();
        assert_eq!(decoded, *value);
        assert_eq!(instruction_from_json(&text(&decoded)).unwrap(), *expected);
    }
}

fn rejects(value: &Value) {
    for encode in [encode_instruction_frame, encode_instruction_archive] {
        let error = encode(&text(value)).expect_err("closed JSON must reject mutation");
        assert_eq!(error.kind(), CodecErrorKind::InvalidArgument, "{error}");
    }
}

#[test]
fn all_activation_variants_match_native_types_wire_ids_and_both_encodings() {
    for (name, native, value) in cases(7) {
        assert_roundtrip(name, &native, &value);
    }
}

#[test]
fn revisions_preserve_every_u64_bit_in_native_frames_and_archives() {
    // Zero remains representable as native u64. Whether a revision names a
    // retained lifecycle is an executor CAS decision, never codec admission.
    for revision in [
        0,
        1,
        crate::json_u64::MAX_SAFE_INTEGER,
        crate::json_u64::MAX_SAFE_INTEGER + 1,
        u64::MAX,
    ] {
        for (name, native, value) in cases(revision) {
            assert_roundtrip(name, &native, &value);
        }
    }
}

#[test]
fn every_lifecycle_field_and_named_envelope_is_mandatory_and_closed() {
    for (name, _, value) in cases(9) {
        for key in value.get(name).unwrap().as_object().unwrap().keys() {
            let mut missing = value.clone();
            map(map(&mut missing).get_mut(name).unwrap()).remove(key);
            rejects(&missing);
        }
        let mut extra = value.clone();
        map(map(&mut extra).get_mut(name).unwrap()).insert("authority".into(), Value::Null);
        rejects(&extra);
        let mut multiple = value.clone();
        map(&mut multiple).insert("other".into(), Value::Null);
        rejects(&multiple);
        for malformed in [Value::Null, Value::Bool(false), Value::Array(vec![])] {
            rejects(&object([(name, malformed)]));
        }
        for namespace in ["smart_contract_code", "SmartContract", "Contract"] {
            rejects(&object([(namespace, value.clone())]));
        }
    }
}

#[test]
fn every_revision_rejects_numeric_aliases_null_overflow_and_noncanonical_text() {
    for (name, _, value) in cases(1) {
        for invalid in [
            Value::Null,
            Value::Bool(true),
            Value::from(1_u64),
            Value::from(u64::MAX),
            Value::Number(json::Number::F64(1.0)),
            Value::Number(json::Number::F64(1.5)),
            Value::Array(vec![]),
            Value::String("".into()),
            Value::String("01".into()),
            Value::String("+1".into()),
            Value::String("-1".into()),
            Value::String("-0".into()),
            Value::String(" 1".into()),
            Value::String("1 ".into()),
            Value::String("1.0".into()),
            Value::String("1e0".into()),
            Value::String("18446744073709551616".into()),
        ] {
            let mut malformed = value.clone();
            map(map(&mut malformed).get_mut(name).unwrap())
                .insert("expected_revision".into(), invalid);
            rejects(&malformed);
        }
        let mut missing = value.clone();
        map(map(&mut missing).get_mut(name).unwrap()).remove("expected_revision");
        for encode in [encode_instruction_frame, encode_instruction_archive] {
            assert!(
                encode(&text(&missing))
                    .unwrap_err()
                    .reason()
                    .contains("expected_revision")
            );
        }
    }
}

#[test]
fn owner_union_and_native_hash_address_spellings_are_exact() {
    for (name, _, value) in cases(2) {
        for address in [
            format!(" {ADDRESS}"),
            format!("{ADDRESS} "),
            "demo::universal".into(),
            "alice@wonderland".into(),
        ] {
            let mut malformed = value.clone();
            map(map(&mut malformed).get_mut(name).unwrap())
                .insert("contract_address".into(), Value::String(address));
            rejects(&malformed);
        }
        if name == "ActivateContractInstance" {
            let hash = value
                .get(name)
                .unwrap()
                .get("code_hash")
                .unwrap()
                .as_str()
                .unwrap();
            let mut bad_checksum = hash.to_owned();
            let previous = bad_checksum.pop().unwrap();
            bad_checksum.push(if previous == '0' { '1' } else { '0' });
            assert_ne!(
                bad_checksum, hash,
                "checksum mutation must change the hash literal"
            );
            for invalid in [
                hash.to_lowercase(),
                "ab".repeat(32),
                format!(" {hash}"),
                bad_checksum,
            ] {
                let mut malformed = value.clone();
                map(map(&mut malformed).get_mut(name).unwrap())
                    .insert("code_hash".into(), Value::String(invalid));
                rejects(&malformed);
            }
        }
        if name == "OfferContractOwnership" {
            for owner in [
                Value::String("Parliament".into()),
                object([("Parliament", Value::Null)]),
                object([("owner", Value::String("Parliament".into()))]),
                object([
                    ("owner", Value::String("Parliament".into())),
                    ("value", Value::Bool(false)),
                ]),
                object([
                    ("owner", Value::String("account".into())),
                    ("value", json::to_value(&account()).unwrap()),
                ]),
                object([
                    ("owner", Value::String("Account".into())),
                    ("value", Value::Null),
                ]),
                object([
                    ("owner", Value::String("Account".into())),
                    ("value", Value::String(format!(" {}", account()))),
                ]),
                object([
                    ("owner", Value::String("Parliament".into())),
                    ("value", Value::Null),
                    ("extra", Value::Null),
                ]),
            ] {
                let mut malformed = value.clone();
                map(map(&mut malformed).get_mut(name).unwrap()).insert("new_owner".into(), owner);
                rejects(&malformed);
            }
        }
    }
}

#[test]
fn nullable_reason_and_boolean_delegation_retain_native_semantics() {
    let address: ContractAddress = ADDRESS.parse().unwrap();
    for reason in [
        None,
        Some(String::new()),
        Some("incident containment".into()),
        Some("改善\u{feff}".into()),
    ] {
        let native: InstructionBox = DeactivateContractInstance {
            contract_address: address.clone(),
            expected_revision: 8,
            reason: reason.clone(),
        }
        .into();
        let value = object([(
            "DeactivateContractInstance",
            object([
                ("contract_address", Value::String(ADDRESS.into())),
                ("expected_revision", Value::String("8".into())),
                ("reason", json::to_value(&reason).unwrap()),
            ]),
        )]);
        assert_roundtrip("DeactivateContractInstance", &native, &value);
        let mut malformed = value.clone();
        map(map(&mut malformed)
            .get_mut("DeactivateContractInstance")
            .unwrap())
        .insert("reason".into(), Value::Bool(false));
        rejects(&malformed);
    }
    for delegated in [true, false] {
        let native: InstructionBox = SetContractParliamentDelegation {
            contract_address: address.clone(),
            expected_revision: 9,
            delegated,
        }
        .into();
        let value = object([(
            "SetContractParliamentDelegation",
            object([
                ("contract_address", Value::String(ADDRESS.into())),
                ("expected_revision", Value::String("9".into())),
                ("delegated", Value::Bool(delegated)),
            ]),
        )]);
        assert_roundtrip("SetContractParliamentDelegation", &native, &value);
        for invalid in [
            Value::from(1_u64),
            Value::String("true".into()),
            Value::Null,
        ] {
            let mut malformed = value.clone();
            map(map(&mut malformed)
                .get_mut("SetContractParliamentDelegation")
                .unwrap())
            .insert("delegated".into(), invalid);
            rejects(&malformed);
        }
    }
}

#[test]
fn generic_native_envelopes_cannot_bypass_closed_lifecycle_contracts() {
    for (name, native, value) in cases(3) {
        let generic = json::to_value(&native).unwrap();
        assert_ne!(generic, value);
        rejects(&generic);
        rejects(&Value::String(
            STANDARD.encode(norito::encode_canonical(&native).unwrap()),
        ));
        // The native generic object grammar is name/params. It must not gain
        // another route to a lifecycle instruction if that registry expands.
        rejects(&object([
            ("name", Value::String(name.to_owned())),
            ("params", value.get(name).unwrap().clone()),
        ]));
    }
    assert!(from_json(&Value::Null).is_none());
    assert!(from_json(&object([("UnknownInstruction", Value::Null)])).is_none());
    let other: InstructionBox =
        iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload {
            code_hash: Hash::new(b"other"),
        }
        .into();
    assert!(!is_activation_instruction(&other));
    assert!(to_json(&other).is_none());
}

#[test]
fn frame_archive_confusion_truncation_and_trailing_data_fail_for_every_variant() {
    for (_, native, _) in cases(4) {
        let frame = norito::encode_canonical(&native).unwrap();
        let archive = native.encode();
        assert!(decode_instruction_frame(&archive).is_err());
        assert!(decode_instruction_archive(&frame).is_err());
        let encodings: [(Vec<u8>, fn(&[u8]) -> CodecResult<String>); 2] = [
            (frame, decode_instruction_frame),
            (archive, decode_instruction_archive),
        ];
        for (bytes, decode) in encodings {
            assert!(decode(&bytes[..bytes.len() - 1]).is_err());
            let mut trailing = bytes;
            trailing.push(0);
            assert!(decode(&trailing).is_err());
        }
    }
}

#[test]
fn duplicate_revision_tokens_fail_before_typed_reconstruction() {
    for (_, _, value) in cases(7) {
        let source = text(&value);
        let mutation = source.replace(
            "\"expected_revision\":\"7\"",
            "\"expected_revision\":\"7\",\"expected_revision\":\"8\"",
        );
        assert_ne!(
            mutation, source,
            "the duplicate mutation must change the fixture"
        );
        for encode in [encode_instruction_frame, encode_instruction_archive] {
            assert!(encode(&mutation).is_err());
        }
    }
}
