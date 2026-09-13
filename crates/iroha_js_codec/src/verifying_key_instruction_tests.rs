//! Verifying-key namespace, native record preservation and strict shape tests.

use super::*;
use crate::{
    decode_instruction_archive, decode_instruction_frame, encode_instruction_archive,
    encode_instruction_frame, value_to_instruction,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_data_model::{confidential::ConfidentialStatus, proof::VerifyingKeyBox, zk::BackendTag};
use norito::codec::Encode;

fn text(value: &Value) -> String {
    json::to_json(value).expect("fixture JSON")
}

fn record() -> VerifyingKeyRecord {
    let mut record = VerifyingKeyRecord::new(
        7,
        "ivm-execution-v1",
        BackendTag::Stark,
        "goldilocks",
        [0x11; 32],
        [0x22; 32],
    );
    record.key = Some(VerifyingKeyBox::new(
        "stark/fri-v1".to_owned(),
        vec![1, 2, 3],
    ));
    record.vk_len = 3;
    record.max_proof_bytes = 1_000_000;
    record.activation_height = Some(u64::MAX - 1);
    record.withdraw_height = Some(u64::MAX);
    record.gas_schedule_id = Some("execution-v1".to_owned());
    record.metadata_uri_cid = Some("fixture-metadata".to_owned());
    record.vk_bytes_cid = Some("fixture-key".to_owned());
    record.status = ConfidentialStatus::Active;
    record
}

fn id() -> VerifyingKeyId {
    VerifyingKeyId::new("stark/fri-v1", "execution-v1")
}

fn payload() -> Value {
    emit("RegisterVerifyingKey", &id(), &record())
        .expect("SDK record projection")
        .get("verifying_keys")
        .expect("namespace")
        .get("RegisterVerifyingKey")
        .expect("variant")
        .clone()
}

fn envelope(name: &str, payload: Value) -> Value {
    object([("verifying_keys", object([(name, payload)]))])
}

fn rejects(value: &Value) {
    for encode in [encode_instruction_frame, encode_instruction_archive] {
        let error = encode(&text(value)).expect_err("closed VK admission must reject");
        assert_eq!(error.kind(), CodecErrorKind::InvalidArgument, "{error}");
    }
}

#[test]
fn verifying_key_records_roundtrip_both_native_encodings_without_height_loss() {
    for name in ["RegisterVerifyingKey", "UpdateVerifyingKey"] {
        for inline in [true, false] {
            let mut record = record();
            if !inline {
                record.key = None;
            }
            let value = emit(name, &id(), &record).expect("canonical native record JSON");
            let source = text(&value);
            let frame = encode_instruction_frame(&source).expect("VK frame");
            let archive = encode_instruction_archive(&source).expect("VK archive");
            for decoded in [
                decode_instruction_frame(&frame),
                decode_instruction_archive(&archive),
            ] {
                let decoded = decoded.expect("native VK decode");
                assert_eq!(json::from_json::<Value>(&decoded).unwrap(), value);
                assert_eq!(encode_instruction_frame(&decoded).unwrap(), frame);
                assert_eq!(encode_instruction_archive(&decoded).unwrap(), archive);
            }
            let native: InstructionBox = match name {
                "RegisterVerifyingKey" => RegisterVerifyingKey { id: id(), record }.into(),
                _ => UpdateVerifyingKey { id: id(), record }.into(),
            };
            assert_eq!(frame, norito::encode_canonical(&native).unwrap());
            assert_eq!(archive, native.encode());
            let mut trailing = archive;
            trailing.push(0);
            assert!(decode_instruction_archive(&trailing).is_err());
        }
    }
}

#[test]
fn verifying_key_instructions_reject_namespace_aliases_and_incomplete_nested_records() {
    let payload = payload();
    for name in ["RegisterVerifyingKey", "UpdateVerifyingKey"] {
        rejects(&object([(name, payload.clone())]));
        let alias = object([("VerifyingKeys", object([(name, payload.clone())]))]);
        assert!(encode_instruction_frame(&text(&alias)).is_err());
        assert!(encode_instruction_archive(&text(&alias)).is_err());
        rejects(&object([
            ("verifying_keys", object([(name, payload.clone())])),
            ("extra", Value::Null),
        ]));
        rejects(&object([(
            "verifying_keys",
            object([(name, payload.clone()), ("extra", Value::Null)]),
        )]));
        for field in ["id", "record"] {
            let mut missing = payload.clone();
            missing.as_object_mut().unwrap().remove(field);
            rejects(&envelope(name, missing));
        }
        let keys = payload
            .get("record")
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for field in keys {
            let mut missing = payload.clone();
            missing
                .as_object_mut()
                .unwrap()
                .get_mut("record")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&field);
            rejects(&envelope(name, missing));
        }
        for field in ["id", "record"] {
            let mut extra = payload.clone();
            extra
                .as_object_mut()
                .unwrap()
                .get_mut(field)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("extra".to_owned(), Value::Null);
            rejects(&envelope(name, extra));
        }
        let mut extra = payload.clone();
        extra
            .as_object_mut()
            .unwrap()
            .get_mut("record")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .get_mut("key")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".to_owned(), Value::Null);
        rejects(&envelope(name, extra));
    }
    rejects(&object([("verifying_keys", Value::Null)]));
    rejects(&envelope("Unknown", payload));
}

#[test]
fn verifying_key_record_bytes_and_registry_ids_keep_native_owner_bounds() {
    let mut malformed = record();
    malformed
        .key
        .as_mut()
        .unwrap()
        .bytes
        .resize(VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1 + 1, 0);
    assert!(record_bounds(&id(), &malformed).is_err());
    let invalid_id = VerifyingKeyId::new("stark/fri-v1", "bad name");
    assert!(record_bounds(&invalid_id, &record()).is_err());
    let native: InstructionBox = RegisterVerifyingKey {
        id: invalid_id,
        record: record(),
    }
    .into();
    assert_eq!(
        decode_instruction_frame(&norito::encode_canonical(&native).unwrap())
            .unwrap_err()
            .kind(),
        CodecErrorKind::InvalidArgument
    );
    let valid: InstructionBox = RegisterVerifyingKey {
        id: id(),
        record: record(),
    }
    .into();
    let alternative = Value::String(STANDARD.encode(norito::encode_canonical(&valid).unwrap()));
    assert!(value_to_instruction(alternative).is_err());
}

#[test]
fn verifying_key_height_projection_has_one_lossless_sdk_representation() {
    const SAFE: u64 = (1_u64 << 53) - 1;
    for name in ["RegisterVerifyingKey", "UpdateVerifyingKey"] {
        for number in [0, 1, SAFE - 1, SAFE, SAFE + 1, u64::MAX] {
            let mut record = record();
            record.activation_height = Some(number);
            record.withdraw_height = Some(number);
            let value = emit(name, &id(), &record).unwrap();
            let expected = if number <= SAFE {
                Value::from(number)
            } else {
                Value::String(number.to_string())
            };
            let fields = value
                .get("verifying_keys")
                .unwrap()
                .get(name)
                .unwrap()
                .get("record")
                .unwrap();
            assert_eq!(fields.get("activation_height"), Some(&expected));
            assert_eq!(fields.get("withdraw_height"), Some(&expected));
            let native: InstructionBox = match name {
                "RegisterVerifyingKey" => RegisterVerifyingKey { id: id(), record }.into(),
                _ => UpdateVerifyingKey { id: id(), record }.into(),
            };
            let source = text(&value);
            let frame = norito::encode_canonical(&native).unwrap();
            let archive = native.encode();
            assert_eq!(encode_instruction_frame(&source).unwrap(), frame);
            assert_eq!(encode_instruction_archive(&source).unwrap(), archive);
            for decoded in [
                decode_instruction_frame(&frame),
                decode_instruction_archive(&archive),
            ] {
                assert_eq!(json::from_json::<Value>(&decoded.unwrap()).unwrap(), value);
            }
        }
        for field in ["activation_height", "withdraw_height"] {
            for rejected in [
                Value::String("0".to_owned()),
                Value::String("9007199254740991".to_owned()),
                Value::String("09007199254740992".to_owned()),
                Value::String("+9007199254740992".to_owned()),
                Value::String("9007199254740992 ".to_owned()),
                Value::String("18446744073709551616".to_owned()),
                Value::from(SAFE + 1),
                Value::from(u64::MAX),
            ] {
                let mut payload = payload();
                payload
                    .as_object_mut()
                    .unwrap()
                    .get_mut("record")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(field.to_owned(), rejected);
                rejects(&envelope(name, payload));
            }
        }
    }
}
