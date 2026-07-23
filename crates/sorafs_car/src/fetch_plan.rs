//! Helpers for serialising and parsing chunk fetch specifications and expected
//! payload metadata from JSON reports emitted by SoraFS tooling.

use norito::json::{Map, Value, to_string_pretty};

use crate::{CarBuildPlan, CarPlanError, ChunkFetchSpec, TaikaiSegmentHint};

/// Errors that can occur while parsing chunk fetch specifications from JSON.
#[derive(Debug, thiserror::Error)]
pub enum FetchPlanError {
    #[error("expected chunk fetch plan JSON array or object containing chunk_fetch_specs")]
    MissingChunkFetchSpecs,
    #[error("chunk fetch spec entry {index} is not an object")]
    InvalidEntry { index: usize },
    #[error("chunk fetch spec entry {index} missing or invalid {field}")]
    MissingField { index: usize, field: &'static str },
    #[error("chunk fetch spec entry {index} digest {digest} is not valid hex")]
    InvalidDigest { digest: String, index: usize },
    #[error("invalid digest hex: {digest}")]
    InvalidDigestString { digest: String },
    #[error("expected payload field `{field}` {reason}")]
    InvalidExpectedPayloadField {
        field: &'static str,
        reason: &'static str,
    },
    #[error("chunk fetch spec entry {index} invalid {field}: {reason}")]
    InvalidField {
        index: usize,
        field: &'static str,
        reason: String,
    },
}

/// Parses chunk fetch specs from either an array of objects or an object that
/// contains a `chunk_fetch_specs` array (as emitted by
/// `sorafs_manifest_stub --json-out`).
pub fn chunk_fetch_specs_from_json(value: &Value) -> Result<Vec<ChunkFetchSpec>, FetchPlanError> {
    if let Some(array) = value.as_array() {
        return parse_chunk_fetch_specs(array);
    }
    if let Some(obj) = value.as_object()
        && let Some(specs) = obj.get("chunk_fetch_specs")
        && let Some(array) = specs.as_array()
    {
        return parse_chunk_fetch_specs(array);
    }
    Err(FetchPlanError::MissingChunkFetchSpecs)
}

/// Extracts an expected payload digest from a manifest JSON report, if present.
///
/// # Errors
///
/// Returns an error when a digest field is present but is not a canonical
/// 32-byte lowercase hexadecimal string.
pub fn expected_payload_digest_from_json(
    value: &Value,
) -> Result<Option<[u8; 32]>, FetchPlanError> {
    let (field, raw) = if let Some(raw) = value.get("payload_digest_hex") {
        ("payload_digest_hex", raw)
    } else if let Some(raw) = manifest_field(value, "car_digest_hex")? {
        ("manifest.car_digest_hex", raw)
    } else {
        return Ok(None);
    };

    let hex = raw
        .as_str()
        .ok_or(FetchPlanError::InvalidExpectedPayloadField {
            field,
            reason: "must be a 64-character lowercase hexadecimal string",
        })?;
    decode_digest_hex(hex)
        .map(Some)
        .map_err(|()| FetchPlanError::InvalidExpectedPayloadField {
            field,
            reason: "must be a 64-character lowercase hexadecimal string",
        })
}

/// Extracts an expected payload length from a manifest JSON report, if present.
///
/// # Errors
///
/// Returns an error when a length field is present but is not an unsigned integer.
pub fn expected_payload_len_from_json(value: &Value) -> Result<Option<u64>, FetchPlanError> {
    let (field, raw) = if let Some(raw) = value.get("payload_len") {
        ("payload_len", raw)
    } else if let Some(raw) = manifest_field(value, "content_length")? {
        ("manifest.content_length", raw)
    } else {
        return Ok(None);
    };

    raw.as_u64()
        .map(Some)
        .ok_or(FetchPlanError::InvalidExpectedPayloadField {
            field,
            reason: "must be an unsigned integer",
        })
}

fn manifest_field<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<Option<&'a Value>, FetchPlanError> {
    let Some(manifest) = value.get("manifest") else {
        return Ok(None);
    };
    let object = manifest
        .as_object()
        .ok_or(FetchPlanError::InvalidExpectedPayloadField {
            field: "manifest",
            reason: "must be an object when present",
        })?;
    Ok(object.get(field))
}

/// Parses a 64-character hex digest string into a BLAKE3 digest array.
pub fn parse_digest_hex(hex: &str) -> Result<[u8; 32], FetchPlanError> {
    decode_digest_hex(hex).map_err(|_| FetchPlanError::InvalidDigestString {
        digest: hex.to_string(),
    })
}

/// Serialises the chunk fetch specifications contained in the provided plan
/// into a JSON array matching the format emitted by SoraFS manifests.
pub fn chunk_fetch_specs_to_json(plan: &CarBuildPlan) -> Value {
    let specs = plan.chunk_fetch_specs();
    Value::Array(chunk_fetch_specs_to_array(&specs))
}

/// Fallible counterpart to [`chunk_fetch_specs_to_json`] for production paths handling
/// untrusted plans.
pub fn try_chunk_fetch_specs_to_json(plan: &CarBuildPlan) -> Result<Value, CarPlanError> {
    let specs = plan.try_chunk_fetch_specs()?;
    Ok(Value::Array(chunk_fetch_specs_to_array(&specs)))
}

/// Serialises chunk fetch specifications into a pretty-printed JSON string,
/// appending a trailing newline for CLI friendliness.
pub fn chunk_fetch_specs_to_string(
    specs: &[ChunkFetchSpec],
) -> Result<String, norito::json::Error> {
    let json = Value::Array(chunk_fetch_specs_to_array(specs));
    let mut rendered = to_string_pretty(&json)?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn chunk_fetch_specs_to_array(specs: &[ChunkFetchSpec]) -> Vec<Value> {
    specs
        .iter()
        .map(|spec| {
            let mut obj = Map::new();
            obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            obj.insert("offset".into(), Value::from(spec.offset));
            obj.insert("length".into(), Value::from(spec.length as u64));
            obj.insert(
                "digest_blake3".into(),
                Value::from(digest_to_hex(&spec.digest)),
            );
            if let Some(hint) = &spec.taikai_segment_hint {
                let mut hint_obj = Map::new();
                hint_obj.insert("event".into(), Value::from(hint.event.clone()));
                hint_obj.insert("stream".into(), Value::from(hint.stream.clone()));
                hint_obj.insert("rendition".into(), Value::from(hint.rendition.clone()));
                hint_obj.insert("sequence".into(), Value::from(hint.sequence));
                if let Some(len) = hint.payload_len {
                    hint_obj.insert("payload_len".into(), Value::from(len));
                }
                if let Some(digest) = hint.payload_digest {
                    hint_obj.insert(
                        "payload_blake3_hex".into(),
                        Value::from(digest_to_hex(&digest)),
                    );
                }
                obj.insert("taikai_segment_hint".into(), Value::Object(hint_obj));
            }
            Value::Object(obj)
        })
        .collect()
}

fn parse_chunk_fetch_specs(array: &[Value]) -> Result<Vec<ChunkFetchSpec>, FetchPlanError> {
    let mut specs = Vec::with_capacity(array.len());
    for (index, entry) in array.iter().enumerate() {
        let obj = entry
            .as_object()
            .ok_or(FetchPlanError::InvalidEntry { index })?;
        let chunk_index_raw =
            obj.get("chunk_index")
                .and_then(Value::as_u64)
                .ok_or(FetchPlanError::MissingField {
                    index,
                    field: "chunk_index",
                })?;
        let chunk_index =
            usize::try_from(chunk_index_raw).map_err(|_| FetchPlanError::InvalidField {
                index,
                field: "chunk_index",
                reason: format!("value {chunk_index_raw} exceeds platform usize"),
            })?;
        let offset =
            obj.get("offset")
                .and_then(Value::as_u64)
                .ok_or(FetchPlanError::MissingField {
                    index,
                    field: "offset",
                })?;
        let length =
            obj.get("length")
                .and_then(Value::as_u64)
                .ok_or(FetchPlanError::MissingField {
                    index,
                    field: "length",
                })?;
        let length = u32::try_from(length).map_err(|_| FetchPlanError::InvalidField {
            index,
            field: "length",
            reason: "value exceeds u32::MAX".to_string(),
        })?;
        if length == 0 {
            return Err(FetchPlanError::InvalidField {
                index,
                field: "length",
                reason: "must be greater than zero".to_string(),
            });
        }
        offset
            .checked_add(u64::from(length))
            .ok_or_else(|| FetchPlanError::InvalidField {
                index,
                field: "offset",
                reason: "offset + length overflows u64".to_string(),
            })?;
        let digest_hex = obj.get("digest_blake3").and_then(Value::as_str).ok_or(
            FetchPlanError::MissingField {
                index,
                field: "digest_blake3",
            },
        )?;
        let digest = decode_digest_hex(digest_hex).map_err(|_| FetchPlanError::InvalidDigest {
            digest: digest_hex.to_string(),
            index,
        })?;
        let taikai_segment_hint = match obj.get("taikai_segment_hint") {
            Some(Value::Object(hint_obj)) => Some((|| -> Result<_, FetchPlanError> {
                let event = hint_obj
                    .get("event")
                    .and_then(Value::as_str)
                    .ok_or(FetchPlanError::MissingField {
                        index,
                        field: "taikai_segment_hint.event",
                    })?
                    .to_owned();
                let stream = hint_obj
                    .get("stream")
                    .and_then(Value::as_str)
                    .ok_or(FetchPlanError::MissingField {
                        index,
                        field: "taikai_segment_hint.stream",
                    })?
                    .to_owned();
                let rendition = hint_obj
                    .get("rendition")
                    .and_then(Value::as_str)
                    .ok_or(FetchPlanError::MissingField {
                        index,
                        field: "taikai_segment_hint.rendition",
                    })?
                    .to_owned();
                let sequence = hint_obj.get("sequence").and_then(Value::as_u64).ok_or(
                    FetchPlanError::MissingField {
                        index,
                        field: "taikai_segment_hint.sequence",
                    },
                )?;
                let payload_len = hint_obj
                    .get("payload_len")
                    .map(|value| {
                        value.as_u64().ok_or(FetchPlanError::InvalidField {
                            index,
                            field: "taikai_segment_hint.payload_len",
                            reason: "expected unsigned integer".to_string(),
                        })
                    })
                    .transpose()?;
                let payload_digest =
                    match hint_obj.get("payload_blake3_hex") {
                        Some(Value::String(hex)) => Some(decode_digest_hex(hex).map_err(|_| {
                            FetchPlanError::InvalidDigest {
                                digest: hex.to_string(),
                                index,
                            }
                        })?),
                        Some(other) => {
                            return Err(FetchPlanError::InvalidField {
                                index,
                                field: "taikai_segment_hint.payload_blake3_hex",
                                reason: format!("expected hex string, found {other:?}"),
                            });
                        }
                        None => None,
                    };
                Ok(TaikaiSegmentHint {
                    event,
                    stream,
                    rendition,
                    sequence,
                    payload_len,
                    payload_digest,
                })
            })()?),
            Some(other) => {
                return Err(FetchPlanError::InvalidField {
                    index,
                    field: "taikai_segment_hint",
                    reason: format!("expected object, found {other:?}"),
                });
            }
            None => None,
        };
        specs.push(ChunkFetchSpec {
            chunk_index,
            offset,
            length,
            digest,
            taikai_segment_hint,
        });
    }
    Ok(specs)
}

fn decode_digest_hex(hex: &str) -> Result<[u8; 32], ()> {
    if hex.len() != 64 {
        return Err(());
    }
    let mut bytes = [0u8; 32];
    for (idx, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        let hi = decode_hex_nibble(chunk[0])?;
        let lo = decode_hex_nibble(chunk[1])?;
        bytes[idx] = (hi << 4) | lo;
    }
    Ok(bytes)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(()),
    }
}

fn digest_to_hex(digest: &[u8; 32]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(digest.len() * 2);
    for &byte in digest {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_specs_from_array() {
        let value = norito::json!([
            {
                "chunk_index": 0,
                "offset": 0,
                "length": 512,
                "digest_blake3": "0000000000000000000000000000000000000000000000000000000000000000"
            },
            {
                "chunk_index": 1,
                "offset": 512,
                "length": 256,
                "digest_blake3": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            }
        ]);
        let specs = chunk_fetch_specs_from_json(&value).expect("parse specs");
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].chunk_index, 0);
        assert_eq!(specs[1].offset, 512);
    }

    #[test]
    fn parse_specs_from_manifest_object() {
        let value = norito::json!({
            "chunk_fetch_specs": [
                {
                    "chunk_index": 0,
                    "offset": 0,
                    "length": 128,
                    "digest_blake3": "0000000000000000000000000000000000000000000000000000000000000000"
                }
            ],
            "payload_digest_hex": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "payload_len": 128
        });
        let specs = chunk_fetch_specs_from_json(&value).expect("parse specs");
        assert_eq!(specs.len(), 1);
        let digest = expected_payload_digest_from_json(&value)
            .expect("parse digest")
            .expect("digest");
        let encoded = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            encoded,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        let len = expected_payload_len_from_json(&value)
            .expect("parse length")
            .expect("length");
        assert_eq!(len, 128);
    }

    #[test]
    fn expected_payload_metadata_absent_returns_none() {
        let value = norito::json!({ "chunk_fetch_specs": [] });

        assert_eq!(
            expected_payload_digest_from_json(&value).expect("parse absent digest"),
            None
        );
        assert_eq!(
            expected_payload_len_from_json(&value).expect("parse absent length"),
            None
        );
    }

    #[test]
    fn expected_payload_metadata_uses_manifest_fallback() {
        let value = norito::json!({
            "manifest": {
                "car_digest_hex": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "content_length": 128
            }
        });

        let digest = expected_payload_digest_from_json(&value)
            .expect("parse fallback digest")
            .expect("fallback digest");
        assert_eq!(
            digest_to_hex(&digest),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            expected_payload_len_from_json(&value).expect("parse fallback length"),
            Some(128)
        );
    }

    #[test]
    fn expected_payload_digest_rejects_malformed_present_field() {
        for value in [
            norito::json!({ "payload_digest_hex": "not-hex" }),
            norito::json!({ "payload_digest_hex": 7 }),
            norito::json!({ "payload_digest_hex": null }),
        ] {
            let err = expected_payload_digest_from_json(&value).unwrap_err();
            assert!(matches!(
                err,
                FetchPlanError::InvalidExpectedPayloadField {
                    field: "payload_digest_hex",
                    ..
                }
            ));
        }
    }

    #[test]
    fn expected_payload_len_rejects_malformed_present_field() {
        let negative_len = -1_i64;
        for value in [
            norito::json!({ "payload_len": "128" }),
            norito::json!({ "payload_len": negative_len }),
            norito::json!({ "payload_len": null }),
        ] {
            let err = expected_payload_len_from_json(&value).unwrap_err();
            assert!(matches!(
                err,
                FetchPlanError::InvalidExpectedPayloadField {
                    field: "payload_len",
                    ..
                }
            ));
        }
    }

    #[test]
    fn expected_payload_metadata_rejects_malformed_manifest_fallback() {
        let digest = norito::json!({ "manifest": { "car_digest_hex": false } });
        let digest_err = expected_payload_digest_from_json(&digest).unwrap_err();
        assert!(matches!(
            digest_err,
            FetchPlanError::InvalidExpectedPayloadField {
                field: "manifest.car_digest_hex",
                ..
            }
        ));

        let length = norito::json!({ "manifest": { "content_length": "128" } });
        let length_err = expected_payload_len_from_json(&length).unwrap_err();
        assert!(matches!(
            length_err,
            FetchPlanError::InvalidExpectedPayloadField {
                field: "manifest.content_length",
                ..
            }
        ));
    }

    #[test]
    fn expected_payload_metadata_rejects_non_object_manifest() {
        for value in [
            norito::json!({ "manifest": null }),
            norito::json!({ "manifest": "invalid" }),
            norito::json!({ "manifest": [] }),
        ] {
            let digest_err = expected_payload_digest_from_json(&value).unwrap_err();
            assert!(matches!(
                digest_err,
                FetchPlanError::InvalidExpectedPayloadField {
                    field: "manifest",
                    ..
                }
            ));

            let length_err = expected_payload_len_from_json(&value).unwrap_err();
            assert!(matches!(
                length_err,
                FetchPlanError::InvalidExpectedPayloadField {
                    field: "manifest",
                    ..
                }
            ));
        }
    }

    #[test]
    fn parse_missing_specs_returns_error() {
        let value = norito::json!({ "payload_digest_hex": "deadbeef" });
        let err = chunk_fetch_specs_from_json(&value).unwrap_err();
        assert!(matches!(err, FetchPlanError::MissingChunkFetchSpecs));
    }

    #[test]
    fn parse_rejects_noncanonical_uppercase_digest() {
        let value = norito::json!([
            {
                "chunk_index": 0,
                "offset": 0,
                "length": 512,
                "digest_blake3": "ABCDEF0000000000000000000000000000000000000000000000000000000000"
            }
        ]);
        let err = chunk_fetch_specs_from_json(&value).unwrap_err();
        assert!(matches!(
            err,
            FetchPlanError::InvalidDigest { index: 0, .. }
        ));
        assert!(
            parse_digest_hex("ABCDEF0000000000000000000000000000000000000000000000000000000000")
                .is_err()
        );
    }

    #[test]
    fn parse_rejects_zero_and_oversized_lengths() {
        let zero = norito::json!([
            {
                "chunk_index": 0,
                "offset": 0,
                "length": 0,
                "digest_blake3": "0000000000000000000000000000000000000000000000000000000000000000"
            }
        ]);
        let err = chunk_fetch_specs_from_json(&zero).unwrap_err();
        assert!(matches!(
            err,
            FetchPlanError::InvalidField {
                index: 0,
                field: "length",
                ..
            }
        ));

        let oversized_length = u64::from(u32::MAX) + 1;
        let oversized = norito::json!([
            {
                "chunk_index": 0,
                "offset": 0,
                "length": oversized_length,
                "digest_blake3": "0000000000000000000000000000000000000000000000000000000000000000"
            }
        ]);
        let err = chunk_fetch_specs_from_json(&oversized).unwrap_err();
        assert!(matches!(
            err,
            FetchPlanError::InvalidField {
                index: 0,
                field: "length",
                ..
            }
        ));
    }

    #[test]
    fn parse_rejects_offset_length_overflow() {
        let max_offset = u64::MAX;
        let value = norito::json!([
            {
                "chunk_index": 0,
                "offset": max_offset,
                "length": 1,
                "digest_blake3": "0000000000000000000000000000000000000000000000000000000000000000"
            }
        ]);
        let err = chunk_fetch_specs_from_json(&value).unwrap_err();
        assert!(matches!(
            err,
            FetchPlanError::InvalidField {
                index: 0,
                field: "offset",
                ..
            }
        ));
    }

    #[test]
    fn parse_rejects_non_object_taikai_segment_hint() {
        let value = norito::json!([
            {
                "chunk_index": 0,
                "offset": 0,
                "length": 512,
                "digest_blake3": "0000000000000000000000000000000000000000000000000000000000000000",
                "taikai_segment_hint": "ignored-before-hardening"
            }
        ]);
        let err = chunk_fetch_specs_from_json(&value).unwrap_err();
        assert!(matches!(
            err,
            FetchPlanError::InvalidField {
                index: 0,
                field: "taikai_segment_hint",
                ..
            }
        ));
    }

    #[test]
    fn taikai_segment_hint_round_trips_extended_fields() {
        let specs = vec![ChunkFetchSpec {
            chunk_index: 0,
            offset: 0,
            length: 256,
            digest: [0xAA; 32],
            taikai_segment_hint: Some(TaikaiSegmentHint {
                event: "demo".into(),
                stream: "stream".into(),
                rendition: "rendition".into(),
                sequence: 7,
                payload_len: Some(4096),
                payload_digest: Some([0xBB; 32]),
            }),
        }];
        let json = Value::Array(chunk_fetch_specs_to_array(&specs));
        let parsed = chunk_fetch_specs_from_json(&json).expect("parse specs");
        assert_eq!(parsed.len(), 1);
        let hint = parsed[0]
            .taikai_segment_hint
            .as_ref()
            .expect("hint present");
        assert_eq!(hint.sequence, 7);
        assert_eq!(hint.payload_len, Some(4096));
        assert_eq!(hint.payload_digest, Some([0xBB; 32]));
    }
}
