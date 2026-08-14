//! Helpers for serialising and parsing chunk fetch specifications and expected
//! payload metadata from JSON reports emitted by SoraFS tooling.
use crate::{CarBuildPlan, CarPlanError, ChunkFetchSpec, TaikaiSegmentHint};
use norito::json::{Map, Value, to_string_pretty};
/// Canonical schema identifier for standalone SoraFS chunk fetch plans.
pub const CHUNK_FETCH_PLAN_SCHEMA_V1: &str = "sorafs.chunk_fetch_plan.v1";
/// Schema identifier for manifest-builder reports that embed chunk specs.
pub const MANIFEST_BUILDER_REPORT_SCHEMA_V1: &str = "sorafs.manifest_builder_report.v1";
/// Schema identifier for `iroha app sorafs toolkit pack` reports.
pub const TOOLKIT_PACK_REPORT_SCHEMA_V1: &str = "sorafs.toolkit_pack_report.v1";
/// Schema identifier for chunk-store reports that embed chunk specs.
pub const CHUNK_STORE_REPORT_SCHEMA_V1: &str = "sorafs.chunk_store_report.v1";
/// Canonical standalone chunk fetch plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkFetchPlanV1 {
    /// BLAKE3 digest of the complete unchunked payload.
    pub payload_digest: [u8; 32],
    /// Ordered chunk locations and digests used to fetch the payload.
    pub chunk_fetch_specs: Vec<ChunkFetchSpec>,
}
/// Errors that can occur while parsing chunk fetch specifications from JSON.
#[derive(Debug, thiserror::Error)]
pub enum FetchPlanError {
    #[error("embedded chunk fetch specifications must be a JSON array")]
    InvalidEmbeddedChunkFetchSpecs,
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
    #[error("standalone chunk fetch plan must be a JSON object")]
    InvalidPlanRoot,
    #[error("standalone chunk fetch plan missing required `{field}` field")]
    MissingPlanField { field: &'static str },
    #[error(
        "standalone chunk fetch plan schema must be `{CHUNK_FETCH_PLAN_SCHEMA_V1}`, found `{found}`"
    )]
    InvalidPlanSchema { found: String },
    #[error("standalone chunk fetch plan contains unsupported field `{field}`")]
    UnsupportedPlanField { field: String },
    #[error(
        "standalone chunk fetch plan `payload_digest_blake3_hex` must be canonical lowercase 32-byte hex"
    )]
    InvalidPlanPayloadDigest,
    #[error("standalone chunk fetch plan payload digest must not be all zeroes")]
    ZeroPlanPayloadDigest,
}
/// Errors that can occur while rendering a standalone chunk fetch plan.
#[derive(Debug, thiserror::Error)]
pub enum FetchPlanRenderError {
    #[error("failed to derive chunk fetch specifications: {0}")]
    Plan(#[from] CarPlanError),
    #[error("failed to render chunk fetch plan JSON: {0}")]
    Json(#[from] norito::json::Error),
    #[error("standalone chunk fetch plan payload digest must not be all zeroes")]
    ZeroPayloadDigest,
}
/// Parses a chunk-spec array already selected from a canonical versioned
/// envelope.
///
/// This helper deliberately does not accept an object or a standalone
/// bare-array plan. Callers must first validate the containing envelope and
/// select its typed `chunk_fetch_specs` field. Standalone interchange uses
/// [`chunk_fetch_plan_from_json`] exclusively.
pub fn chunk_fetch_specs_from_embedded_array(
    value: &Value,
) -> Result<Vec<ChunkFetchSpec>, FetchPlanError> {
    value
        .as_array()
        .ok_or(FetchPlanError::InvalidEmbeddedChunkFetchSpecs)
        .and_then(|values| parse_chunk_fetch_specs(values))
}
/// Parses the canonical V1 standalone chunk fetch plan envelope.
///
/// The parser intentionally rejects the retired bare-array representation and
/// requires the exact whole-payload BLAKE3 digest committed by the producer.
pub fn chunk_fetch_plan_from_json(value: &Value) -> Result<ChunkFetchPlanV1, FetchPlanError> {
    let obj = value.as_object().ok_or(FetchPlanError::InvalidPlanRoot)?;
    for field in obj.keys() {
        if !matches!(
            field.as_str(),
            "schema" | "payload_digest_blake3_hex" | "chunk_fetch_specs"
        ) {
            return Err(FetchPlanError::UnsupportedPlanField {
                field: field.clone(),
            });
        }
    }
    let schema = obj
        .get("schema")
        .and_then(Value::as_str)
        .ok_or(FetchPlanError::MissingPlanField { field: "schema" })?;
    if schema != CHUNK_FETCH_PLAN_SCHEMA_V1 {
        return Err(FetchPlanError::InvalidPlanSchema {
            found: schema.to_owned(),
        });
    }
    let payload_digest_hex = obj
        .get("payload_digest_blake3_hex")
        .and_then(Value::as_str)
        .ok_or(FetchPlanError::MissingPlanField {
            field: "payload_digest_blake3_hex",
        })?;
    let payload_digest = decode_digest_hex(payload_digest_hex)
        .map_err(|()| FetchPlanError::InvalidPlanPayloadDigest)?;
    if payload_digest == [0; 32] {
        return Err(FetchPlanError::ZeroPlanPayloadDigest);
    }
    let chunk_fetch_specs = obj
        .get("chunk_fetch_specs")
        .and_then(Value::as_array)
        .ok_or(FetchPlanError::MissingPlanField {
            field: "chunk_fetch_specs",
        })
        .and_then(|specs| parse_chunk_fetch_specs(specs))?;
    Ok(ChunkFetchPlanV1 {
        payload_digest,
        chunk_fetch_specs,
    })
}
/// Extracts an explicit expected payload digest from a JSON report, if present.
///
/// V1 reports must carry the canonical whole-payload digest in the top-level
/// `payload_digest_hex` field. `manifest.car_digest_hex` commits the complete
/// CARv2 archive and is deliberately never accepted as a payload-digest
/// fallback.
///
/// # Errors
///
/// Returns an error when a digest field is present but is not a canonical
/// 32-byte lowercase hexadecimal string.
pub fn expected_payload_digest_from_json(
    value: &Value,
) -> Result<Option<[u8; 32]>, FetchPlanError> {
    let Some(raw) = value.get("payload_digest_hex") else {
        return Ok(None);
    };
    let field = "payload_digest_hex";
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
/// Serialises chunk fetch specifications for a typed field in another
/// canonical versioned envelope.
///
/// This value is never a standalone plan. Use
/// [`try_chunk_fetch_plan_to_json`] for interchange.
pub fn try_chunk_fetch_specs_to_json(plan: &CarBuildPlan) -> Result<Value, CarPlanError> {
    let specs = plan.try_chunk_fetch_specs()?;
    Ok(Value::Array(chunk_fetch_specs_to_array(&specs)))
}
/// Serialises a plan into the canonical V1 standalone chunk fetch plan envelope.
pub fn try_chunk_fetch_plan_to_json(plan: &CarBuildPlan) -> Result<Value, FetchPlanRenderError> {
    if plan.payload_digest.as_bytes() == &[0; 32] {
        return Err(FetchPlanRenderError::ZeroPayloadDigest);
    }
    let specs = plan.try_chunk_fetch_specs()?;
    let mut obj = Map::new();
    obj.insert("schema".into(), Value::from(CHUNK_FETCH_PLAN_SCHEMA_V1));
    obj.insert(
        "payload_digest_blake3_hex".into(),
        Value::from(digest_to_hex(plan.payload_digest.as_bytes())),
    );
    obj.insert(
        "chunk_fetch_specs".into(),
        Value::Array(chunk_fetch_specs_to_array(&specs)),
    );
    Ok(Value::Object(obj))
}
/// Serialises a plan into a pretty-printed canonical V1 standalone envelope,
/// appending a trailing newline for CLI friendliness.
pub fn chunk_fetch_plan_to_string(plan: &CarBuildPlan) -> Result<String, FetchPlanRenderError> {
    let json = try_chunk_fetch_plan_to_json(plan)?;
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
    fn sample_plan() -> CarBuildPlan {
        CarBuildPlan::single_file(b"canonical fetch plan").expect("sample plan")
    }
    #[test]
    fn standalone_plan_round_trips_payload_digest_and_specs() {
        let plan = sample_plan();
        let json = try_chunk_fetch_plan_to_json(&plan).expect("render plan");
        let parsed = chunk_fetch_plan_from_json(&json).expect("parse plan");
        assert_eq!(parsed.payload_digest, *plan.payload_digest.as_bytes());
        assert_eq!(
            parsed.chunk_fetch_specs,
            plan.try_chunk_fetch_specs().expect("valid sample plan")
        );
    }
    #[test]
    fn standalone_plan_rejects_retired_array_and_missing_payload_digest() {
        let plan = sample_plan();
        let retired = try_chunk_fetch_specs_to_json(&plan).expect("valid sample plan");
        assert!(matches!(
            chunk_fetch_plan_from_json(&retired),
            Err(FetchPlanError::InvalidPlanRoot)
        ));
        let mut missing_digest = Map::new();
        missing_digest.insert("schema".into(), Value::from(CHUNK_FETCH_PLAN_SCHEMA_V1));
        missing_digest.insert(
            "chunk_fetch_specs".into(),
            try_chunk_fetch_specs_to_json(&plan).expect("valid sample plan"),
        );
        let missing_digest = Value::Object(missing_digest);
        assert!(matches!(
            chunk_fetch_plan_from_json(&missing_digest),
            Err(FetchPlanError::MissingPlanField {
                field: "payload_digest_blake3_hex"
            })
        ));
    }
    #[test]
    fn standalone_plan_rejects_zero_or_noncanonical_payload_digest() {
        let plan = sample_plan();
        let specs = try_chunk_fetch_specs_to_json(&plan).expect("valid sample plan");
        for payload_digest in ["0".repeat(64), "A1".repeat(32), "deadbeef".to_owned()] {
            let mut value = Map::new();
            value.insert("schema".into(), Value::from(CHUNK_FETCH_PLAN_SCHEMA_V1));
            value.insert(
                "payload_digest_blake3_hex".into(),
                Value::from(payload_digest.clone()),
            );
            value.insert("chunk_fetch_specs".into(), specs.clone());
            let value = Value::Object(value);
            assert!(
                chunk_fetch_plan_from_json(&value).is_err(),
                "payload digest must be rejected: {payload_digest}"
            );
        }
        let mut zero_plan = plan;
        zero_plan.payload_digest = blake3::Hash::from_bytes([0; 32]);
        assert!(matches!(
            try_chunk_fetch_plan_to_json(&zero_plan),
            Err(FetchPlanRenderError::ZeroPayloadDigest)
        ));
    }
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
        let specs = chunk_fetch_specs_from_embedded_array(&value).expect("parse embedded specs");
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
        let specs = chunk_fetch_specs_from_embedded_array(
            value
                .get("chunk_fetch_specs")
                .expect("versioned report chunk_fetch_specs"),
        )
        .expect("parse embedded specs");
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
    fn expected_payload_digest_never_uses_manifest_car_digest_fallback() {
        let value = norito::json!({
            "manifest": {
                "car_digest_hex": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "content_length": 128
            }
        });
        assert_eq!(
            expected_payload_digest_from_json(&value).expect("parse explicit payload digest"),
            None
        );
        assert_eq!(
            expected_payload_len_from_json(&value).expect("parse fallback length"),
            Some(128)
        );
    }
    #[test]
    fn expected_payload_digest_uses_only_the_explicit_top_level_field() {
        let value = norito::json!({
            "payload_digest_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "manifest": {
                "car_digest_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            }
        });
        assert_eq!(
            expected_payload_digest_from_json(&value)
                .expect("parse explicit payload digest")
                .expect("explicit payload digest"),
            [0xaa; 32]
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
    fn expected_payload_len_rejects_malformed_manifest_fallback() {
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
    fn payload_digest_ignores_nonobject_manifest_but_length_rejects_it() {
        for value in [
            norito::json!({ "manifest": null }),
            norito::json!({ "manifest": "invalid" }),
            norito::json!({ "manifest": [] }),
        ] {
            assert_eq!(
                expected_payload_digest_from_json(&value)
                    .expect("manifest shape is irrelevant to explicit payload digest"),
                None
            );
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
    fn embedded_parser_rejects_containing_object() {
        let value = norito::json!({ "payload_digest_hex": "deadbeef" });
        let err = chunk_fetch_specs_from_embedded_array(&value).unwrap_err();
        assert!(matches!(
            err,
            FetchPlanError::InvalidEmbeddedChunkFetchSpecs
        ));
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
        let err = chunk_fetch_specs_from_embedded_array(&value).unwrap_err();
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
        let err = chunk_fetch_specs_from_embedded_array(&zero).unwrap_err();
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
        let err = chunk_fetch_specs_from_embedded_array(&oversized).unwrap_err();
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
        let err = chunk_fetch_specs_from_embedded_array(&value).unwrap_err();
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
        let err = chunk_fetch_specs_from_embedded_array(&value).unwrap_err();
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
        let parsed =
            chunk_fetch_specs_from_embedded_array(&json).expect("parse embedded chunk specs");
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
