//! Lossless JavaScript projections of the native Kaigi unsigned 64-bit fields.
//!
//! Values through JavaScript's maximum safe integer use JSON numbers; larger
//! values use exact decimal strings. The native model and Norito wire remain u64.

use iroha_data_model::kaigi::{KaigiRelayManifest, NewKaigi};
use norito::json::{self, Value};

use crate::{CodecError, CodecResult, codec_error};

pub(super) use crate::json_u64::{parse_u64, u64_json};

fn native_field(value: &mut Value, key: &str, optional: bool, label: &str) -> CodecResult<()> {
    if let Some(field) = value.as_object_mut().and_then(|fields| fields.get_mut(key)) {
        if optional && *field == Value::Null {
            return Ok(());
        }
        *field = Value::Number(parse_u64(field.clone(), label)?.into());
    }
    // Required fields and object shape are checked by the native model decoder.
    Ok(())
}

pub(super) fn parse_relay_manifest(mut value: Value) -> CodecResult<KaigiRelayManifest> {
    native_field(
        &mut value,
        "expiry_ms",
        false,
        "KaigiRelayManifest.expiry_ms",
    )?;
    json::from_value(value).map_err(codec_error)
}

pub(super) fn relay_manifest_json(manifest: &KaigiRelayManifest) -> CodecResult<Value> {
    let mut value = json::to_value(manifest).map_err(codec_error)?;
    let fields = value
        .as_object_mut()
        .ok_or_else(|| CodecError::failure("native Kaigi relay manifest must be an object"))?;
    fields.insert("expiry_ms".to_owned(), u64_json(manifest.expiry_ms));
    Ok(value)
}

pub(super) fn parse_call(mut value: Value) -> CodecResult<NewKaigi> {
    native_field(
        &mut value,
        "gas_rate_per_minute",
        false,
        "CreateKaigi.call.gas_rate_per_minute",
    )?;
    native_field(
        &mut value,
        "scheduled_start_ms",
        true,
        "CreateKaigi.call.scheduled_start_ms",
    )?;
    if let Some(manifest) = value
        .as_object_mut()
        .and_then(|fields| fields.get_mut("relay_manifest"))
    {
        native_field(
            manifest,
            "expiry_ms",
            false,
            "CreateKaigi.call.relay_manifest.expiry_ms",
        )?;
    }
    json::from_value(value).map_err(codec_error)
}

pub(super) fn call_json(call: &NewKaigi) -> CodecResult<Value> {
    let mut value = json::to_value(call).map_err(codec_error)?;
    let fields = value
        .as_object_mut()
        .ok_or_else(|| CodecError::failure("native Kaigi call must be an object"))?;
    fields.insert(
        "gas_rate_per_minute".to_owned(),
        u64_json(call.gas_rate_per_minute),
    );
    fields.insert(
        "scheduled_start_ms".to_owned(),
        call.scheduled_start_ms.map_or(Value::Null, u64_json),
    );
    fields.insert(
        "relay_manifest".to_owned(),
        call.relay_manifest
            .as_ref()
            .map(relay_manifest_json)
            .transpose()?
            .unwrap_or(Value::Null),
    );
    Ok(value)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        isi::{
            CreateKaigi, EndKaigi, Instruction, InstructionBox, RecordKaigiUsage,
            ReportKaigiRelayHealth, SetKaigiRelayManifest,
        },
        kaigi::{KAIGI_MAX_PARTICIPANTS_V1, KaigiId, KaigiRelayHealthStatus, KaigiRelayHop},
    };

    const FIXTURE_NETWORK_PREFIX: u16 = 753;

    use super::*;
    use crate::{CodecErrorKind, json_u64::MAX_SAFE_INTEGER};
    use crate::{
        decode_instruction_archive, decode_instruction_frame, encode_instruction_archive,
        encode_instruction_frame, instruction_to_json_value,
    };

    fn fixture_call(value: u64) -> NewKaigi {
        let key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).expect("fixture key");
        let account = AccountId::new(key.public_key().clone());
        let mut call = NewKaigi::with_defaults(
            KaigiId::new(
                iroha_model_base::domain::DomainId::parse_fully_qualified("wonderland.sora")
                    .expect("domain"),
                "full-width".parse().expect("call name"),
            ),
            account,
        );
        call.max_participants =
            Some(u32::try_from(KAIGI_MAX_PARTICIPANTS_V1).expect("participant cap"));
        call.gas_rate_per_minute = value;
        call.scheduled_start_ms = Some(value);
        call.relay_manifest = Some(KaigiRelayManifest {
            hops: (1_u8..=3)
                .map(|seed| {
                    let relay = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                        .expect("relay key");
                    KaigiRelayHop {
                        relay_id: AccountId::new(relay.public_key().clone()),
                        hpke_public_key: vec![seed; 32],
                        weight: 1,
                    }
                })
                .collect(),
            expiry_ms: value,
        });
        call
    }

    fn instructions(value: u64) -> Vec<(InstructionBox, &'static str, Vec<Vec<&'static str>>)> {
        let call = fixture_call(value);
        let call_id = call.id.clone();
        vec![
            (
                Box::new(CreateKaigi {
                    call: call.clone(),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                })
                .into_instruction_box(),
                "CreateKaigi",
                vec![
                    vec!["call", "gas_rate_per_minute"],
                    vec!["call", "scheduled_start_ms"],
                    vec!["call", "relay_manifest", "expiry_ms"],
                ],
            ),
            (
                Box::new(EndKaigi {
                    call_id: call_id.clone(),
                    ended_at_ms: Some(value),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                })
                .into_instruction_box(),
                "EndKaigi",
                vec![vec!["ended_at_ms"]],
            ),
            (
                Box::new(RecordKaigiUsage {
                    call_id: call_id.clone(),
                    duration_ms: value,
                    billed_gas: value,
                    usage_commitment: None,
                    proof: None,
                })
                .into_instruction_box(),
                "RecordKaigiUsage",
                vec![vec!["duration_ms"], vec!["billed_gas"]],
            ),
            (
                Box::new(ReportKaigiRelayHealth {
                    call_id: call_id.clone(),
                    relay_id: call.host.clone(),
                    status: KaigiRelayHealthStatus::Healthy,
                    reported_at_ms: value,
                    notes: None,
                })
                .into_instruction_box(),
                "ReportKaigiRelayHealth",
                vec![vec!["reported_at_ms"]],
            ),
            (
                Box::new(SetKaigiRelayManifest {
                    call_id,
                    relay_manifest: call.relay_manifest,
                })
                .into_instruction_box(),
                "SetKaigiRelayManifest",
                vec![vec!["relay_manifest", "expiry_ms"]],
            ),
        ]
    }

    fn field<'a>(value: &'a mut Value, variant: &str, path: &[&str]) -> &'a mut Value {
        let mut value = value
            .as_object_mut()
            .expect("instruction")
            .get_mut("Kaigi")
            .expect("Kaigi")
            .as_object_mut()
            .expect("Kaigi object")
            .get_mut(variant)
            .expect("variant");
        for key in path {
            value = value
                .as_object_mut()
                .expect("object")
                .get_mut(*key)
                .expect("field");
        }
        value
    }

    #[test]
    fn full_u64_projections_preserve_native_frame_and_archive_bytes() {
        for number in [
            0,
            1,
            MAX_SAFE_INTEGER - 1,
            MAX_SAFE_INTEGER,
            MAX_SAFE_INTEGER + 1,
            u64::MAX,
        ] {
            for (instruction, variant, paths) in instructions(number) {
                let mut expected = instruction_to_json_value(&instruction).expect("SDK projection");
                for path in paths {
                    let scalar = field(&mut expected, variant, &path);
                    if number <= MAX_SAFE_INTEGER {
                        assert!(matches!(scalar, Value::Number(_)), "{variant} {path:?}");
                        assert_eq!(scalar.as_u64(), Some(number));
                    } else {
                        assert_eq!(
                            *scalar,
                            Value::String(number.to_string()),
                            "{variant} {path:?}"
                        );
                    }
                }
                let source = json::to_json(&expected).expect("SDK JSON");
                let frame = norito::encode_canonical(&instruction).expect("native typed frame");
                assert_eq!(
                    encode_instruction_frame(&source, FIXTURE_NETWORK_PREFIX).expect("SDK frame"),
                    frame,
                    "{variant}"
                );
                let mut archive = Vec::new();
                norito::codec::encode_adaptive_into(&instruction, &mut archive)
                    .expect("native typed archive");
                assert_eq!(
                    encode_instruction_archive(&source, FIXTURE_NETWORK_PREFIX)
                        .expect("SDK archive"),
                    archive,
                    "{variant}"
                );
                for decoded in [
                    decode_instruction_frame(&frame, FIXTURE_NETWORK_PREFIX),
                    decode_instruction_archive(&archive, FIXTURE_NETWORK_PREFIX),
                ] {
                    let decoded: Value = json::from_json(&decoded.expect("native decode"))
                        .expect("SDK decoded JSON");
                    assert_eq!(decoded, expected, "{variant} {number}");
                }
            }
        }
    }

    #[test]
    fn every_u64_field_rejects_noncanonical_strings_and_unsafe_numbers() {
        let invalid: Vec<Value> = [
            "",
            "0",
            "1",
            "9007199254740991",
            "09007199254740992",
            "+9007199254740992",
            " 9007199254740992",
            "9007199254740992 ",
            "9007199254740992.0",
            "9.007199254740992e15",
            "-1",
            "18446744073709551616",
            "999999999999999999999",
            "９００７１９９２５４７４０９９２",
        ]
        .into_iter()
        .map(|text| Value::String(text.to_owned()))
        .chain([
            Value::Number((MAX_SAFE_INTEGER + 1).into()),
            Value::Number(u64::MAX.into()),
            Value::Number((-1_i64).into()),
            Value::Number(json::Number::F64(1.5)),
            Value::Bool(true),
        ])
        .collect();
        for (instruction, variant, paths) in instructions(u64::MAX) {
            let source = instruction_to_json_value(&instruction).expect("SDK projection");
            for path in paths {
                for rejected in &invalid {
                    let mut mutated = source.clone();
                    *field(&mut mutated, variant, &path) = rejected.clone();
                    let text = json::to_json(&mutated).expect("mutation JSON");
                    for encode in [encode_instruction_frame, encode_instruction_archive] {
                        let error = encode(&text, FIXTURE_NETWORK_PREFIX)
                            .expect_err("noncanonical u64 projection");
                        assert_eq!(
                            error.kind(),
                            CodecErrorKind::InvalidArgument,
                            "{variant} {path:?}: {error}"
                        );
                        assert!(
                            error.to_string().contains("JSON safe unsigned integer"),
                            "{variant} {path:?}: {error}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn call_projection_preserves_nulls_and_does_not_rewrite_metadata() {
        let mut call = fixture_call(u64::MAX);
        call.scheduled_start_ms = None;
        call.relay_manifest = None;
        let mut value = call_json(&call).expect("call projection");
        assert_eq!(value.get("scheduled_start_ms"), Some(&Value::Null));
        assert_eq!(value.get("relay_manifest"), Some(&Value::Null));
        assert_eq!(parse_call(value.clone()).expect("null option fields"), call);
        value.as_object_mut().expect("object").insert(
            "metadata".to_owned(),
            norito::json!({"expiry_ms": "18446744073709551615", "gas_rate_per_minute": 1}),
        );
        let projected =
            call_json(&parse_call(value.clone()).expect("typed metadata")).expect("projection");
        assert_eq!(projected.get("metadata"), value.get("metadata"));
        let mut null_required = value.clone();
        *null_required
            .as_object_mut()
            .expect("object")
            .get_mut("gas_rate_per_minute")
            .expect("gas") = Value::Null;
        assert!(parse_call(null_required).is_err());
        let mut missing = value;
        missing
            .as_object_mut()
            .expect("object")
            .remove("gas_rate_per_minute");
        assert!(parse_call(missing).is_err());
        assert!(parse_call(Value::Array(Vec::new())).is_err());
        assert!(parse_relay_manifest(norito::json!({"hops": []})).is_err());
        assert!(parse_relay_manifest(norito::json!({"hops": [], "expiry_ms": null})).is_err());
    }
}
