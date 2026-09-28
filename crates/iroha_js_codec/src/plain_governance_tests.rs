//! Native public ballot framing, exact integers and closed-envelope admission.

use super::*;
use crate::{
    decode_instruction_archive, decode_instruction_frame, encode_instruction_archive,
    encode_instruction_frame,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::account::{AccountId, address::ChainDiscriminantGuard};

fn payload(name: &str, duration: u64) -> Value {
    let owner = AccountId::new(
        KeyPair::try_from_seed(vec![0x57; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    let mut fields = json::Map::new();
    fields.insert("referendum_id".into(), Value::String("ref-native".into()));
    fields.insert("owner".into(), json::to_value(&owner).unwrap());
    fields.insert(
        "amount".into(),
        Value::String("18446744073709551616.25".into()),
    );
    fields.insert("duration_blocks".into(), Value::Number(duration.into()));
    if name == CAST {
        fields.insert("direction".into(), Value::Number(2_u64.into()));
    }
    crate::instruction_envelope(name, Value::Object(fields))
}

fn reject(value: &Value) {
    let text = json::to_json(value).unwrap();
    assert!(encode_instruction_frame(&text, 369).is_err(), "{text}");
    assert!(encode_instruction_archive(&text, 369).is_err(), "{text}");
}

#[test]
fn direct_conviction_matches_native_golden_frame_and_archive() {
    let fixture: Value = json::from_json(include_str!(
        "../../../fixtures/governance/plain_v1/update_plain_conviction_instruction_v1.json"
    ))
    .unwrap();
    let instruction = crate::instruction_envelope(UPDATE, fixture["inputs"].clone());
    let text = json::to_json(&instruction).unwrap();
    let frame = encode_instruction_frame(&text, 753).unwrap();
    let archive = encode_instruction_archive(&text, 753).unwrap();
    assert_eq!(
        hex::encode(&frame),
        fixture["standalone_instruction_box_frame_hex"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        hex::encode(&archive),
        fixture["instruction_box_pair_hex"].as_str().unwrap()
    );
    assert_eq!(
        json::from_json::<Value>(&decode_instruction_frame(&frame, 753).unwrap()).unwrap(),
        instruction
    );
    assert_eq!(
        json::from_json::<Value>(&decode_instruction_archive(&archive, 753).unwrap()).unwrap(),
        instruction
    );
}

#[test]
fn both_plain_operations_preserve_all_u64_duration_boundaries_and_network_scope() {
    let _network = ChainDiscriminantGuard::enter(369);
    for name in [CAST, UPDATE] {
        for duration in [0, 1, (1_u64 << 53) - 1, 1_u64 << 53, u64::MAX] {
            let value = payload(name, duration);
            let text = json::to_json(&value).unwrap();
            type Encode = fn(&str, u16) -> CodecResult<Vec<u8>>;
            type Decode = fn(&[u8], u16) -> CodecResult<String>;
            let operations: [(Encode, Decode); 2] = [
                (encode_instruction_frame, decode_instruction_frame),
                (encode_instruction_archive, decode_instruction_archive),
            ];
            for (encode, decode) in operations {
                let bytes = encode(&text, 369).unwrap();
                let decoded: Value = json::from_json(&decode(&bytes, 369).unwrap()).unwrap();
                assert_eq!(decoded, value);
                assert_eq!(decoded[name]["duration_blocks"].as_u64(), Some(duration));
                let foreign: Value = json::from_json(&decode(&bytes, 753).unwrap()).unwrap();
                assert_ne!(foreign[name]["owner"], value[name]["owner"]);
                assert!(encode(&text, 753).is_err());
                assert_eq!(
                    iroha_data_model::account::address::chain_discriminant(),
                    369
                );
            }
        }
    }
}

#[test]
fn both_plain_operations_reject_missing_unknown_alias_and_wrong_payload_fields() {
    let _network = ChainDiscriminantGuard::enter(369);
    for name in [CAST, UPDATE] {
        let original = payload(name, 50);
        let Value::Object(envelope) = &original else {
            unreachable!()
        };
        let Value::Object(fields) = &envelope[name] else {
            unreachable!()
        };
        for key in fields.keys() {
            let mut missing = fields.clone();
            missing.remove(key);
            reject(&crate::instruction_envelope(name, Value::Object(missing)));
        }
        for key in [
            "durationBlocks",
            "referendumId",
            "choice",
            "action",
            "private_key",
        ] {
            let mut extra = fields.clone();
            extra.insert(key.into(), Value::Null);
            reject(&crate::instruction_envelope(name, Value::Object(extra)));
        }
        let mut extra = envelope.clone();
        extra.insert("unknown".into(), Value::Null);
        reject(&Value::Object(extra));
        for invalid in [Value::Null, Value::Bool(false), Value::Array(vec![])] {
            reject(&crate::instruction_envelope(name, invalid));
        }
    }
}

#[test]
fn both_plain_operations_reject_noncanonical_selectors_amounts_and_durations() {
    let _network = ChainDiscriminantGuard::enter(369);
    for name in [CAST, UPDATE] {
        let Value::Object(envelope) = payload(name, 50) else {
            unreachable!()
        };
        let Value::Object(fields) = &envelope[name] else {
            unreachable!()
        };
        for (field, invalid) in [
            ("referendum_id", Value::String(".hidden".into())),
            ("referendum_id", Value::String("a/b".into())),
            ("referendum_id", Value::String("a".repeat(129))),
            ("amount", Value::Number(1_u64.into())),
            ("amount", Value::String("1.00".into())),
            ("amount", Value::String(" 1".into())),
            ("duration_blocks", Value::String("50".into())),
            ("duration_blocks", Value::Number(json::Number::F64(50.5))),
            ("duration_blocks", Value::Bool(true)),
            ("owner", Value::String("not-an-account".into())),
        ] {
            let mut changed = fields.clone();
            changed.insert(field.into(), invalid);
            reject(&crate::instruction_envelope(name, Value::Object(changed)));
        }
        let mut both = fields.clone();
        both.insert("direction".into(), Value::Number(3_u64.into()));
        reject(&crate::instruction_envelope(name, Value::Object(both)));
    }
}

#[test]
fn decoding_invalid_typed_ballot_cannot_bypass_the_closed_contract() {
    let _network = ChainDiscriminantGuard::enter(369);
    let valid = from_json(&payload(CAST, 50)).unwrap().unwrap();
    let mut invalid = valid
        .as_any()
        .downcast_ref::<CastPlainBallot>()
        .unwrap()
        .clone();
    invalid.direction = 3;
    let boxed: InstructionBox = invalid.into();
    let bytes = norito::encode_canonical(&boxed).unwrap();
    assert!(decode_instruction_frame(&bytes, 369).is_err());
    assert!(to_json(&boxed).unwrap().is_err());
}

#[test]
fn plain_governance_rejects_ambiguous_json_integer_tokens_and_duplicate_fields() {
    let _network = ChainDiscriminantGuard::enter(369);
    for name in [CAST, UPDATE] {
        let original = json::to_json(&payload(name, 50)).unwrap();
        for token in ["50.0", "5e1", "-1", "18446744073709551616", "\"50\""] {
            let changed = original.replace(
                "\"duration_blocks\":50",
                &format!("\"duration_blocks\":{token}"),
            );
            assert_ne!(changed, original);
            assert!(
                encode_instruction_frame(&changed, 369).is_err(),
                "{changed}"
            );
            assert!(
                encode_instruction_archive(&changed, 369).is_err(),
                "{changed}"
            );
        }
        let duplicate = original.replace(
            "\"duration_blocks\":50",
            "\"duration_blocks\":49,\"duration_blocks\":50",
        );
        assert!(
            encode_instruction_frame(&duplicate, 369).is_err(),
            "{duplicate}"
        );
        assert!(
            encode_instruction_archive(&duplicate, 369).is_err(),
            "{duplicate}"
        );
    }
}
