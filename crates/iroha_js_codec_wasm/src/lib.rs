//! Browser bindings for the shared canonical account and strict Norito codecs.
//!
//! This adapter has no signing, transport, filesystem or private-key API. Crypto
//! admission and instruction reconstruction belong to `iroha_js_codec`.

use iroha_js_codec::{CodecError, CodecResult, ParsedAccountAddress, RenderedAccountAddress};
use norito::json::{self, Value};
use wasm_bindgen::prelude::*;

const MAX_ERROR_BYTES: usize = 4096;

fn diagnostic(reason: &str) -> &str {
    &reason[..reason.floor_char_boundary(MAX_ERROR_BYTES.min(reason.len()))]
}

fn js_error(error: CodecError) -> JsError {
    JsError::new(diagnostic(error.reason()))
}

fn checked_prefix(prefix: f64) -> CodecResult<u16> {
    iroha_js_codec::checked_network_prefix(prefix)
}

fn parsed_json(parsed: ParsedAccountAddress) -> CodecResult<String> {
    let value = Value::Object(
        [
            (
                "canonicalBytes".to_owned(),
                Value::Array(
                    parsed
                        .canonical_bytes
                        .into_iter()
                        .map(Value::from)
                        .collect(),
                ),
            ),
            (
                "networkPrefix".to_owned(),
                Value::from(parsed.network_prefix),
            ),
        ]
        .into_iter()
        .collect(),
    );
    json::to_json(&value).map_err(|error| CodecError::failure(error.to_string()))
}

fn rendered_json(rendered: RenderedAccountAddress) -> CodecResult<String> {
    let value = Value::Object(
        [
            (
                "canonicalHex".to_owned(),
                Value::from(rendered.canonical_hex),
            ),
            ("i105".to_owned(), Value::from(rendered.i105)),
        ]
        .into_iter()
        .collect(),
    );
    json::to_json(&value).map_err(|error| CodecError::failure(error.to_string()))
}

/// Parse an encoded account and return its canonical bytes and network prefix as JSON.
#[wasm_bindgen(js_name = accountAddressParseEncoded)]
pub fn account_address_parse_encoded(
    input: &str,
    expected_prefix: Option<f64>,
) -> Result<String, JsError> {
    let expected_prefix = expected_prefix
        .map(checked_prefix)
        .transpose()
        .map_err(js_error)?;
    iroha_js_codec::account_address_parse_encoded(input, expected_prefix)
        .and_then(parsed_json)
        .map_err(js_error)
}

/// Render canonical account bytes and return canonical hex and I105 as JSON.
#[wasm_bindgen(js_name = accountAddressRender)]
pub fn account_address_render(bytes: &[u8], network_prefix: f64) -> Result<String, JsError> {
    let prefix = checked_prefix(network_prefix).map_err(js_error)?;
    iroha_js_codec::account_address_render(bytes, prefix)
        .and_then(rendered_json)
        .map_err(js_error)
}

/// Encode strict instruction JSON as its canonical public Norito frame.
/// The network prefix is required and must be a finite integer fitting `u16`.
#[wasm_bindgen(js_name = noritoEncodeInstruction)]
pub fn encode_instruction_frame(input: &str, network_prefix: f64) -> Result<Vec<u8>, JsError> {
    let prefix = checked_prefix(network_prefix).map_err(js_error)?;
    iroha_js_codec::encode_instruction_frame(input, prefix).map_err(js_error)
}

/// Decode one canonical public Norito instruction frame into strict JSON.
/// The network prefix is required and selects account rendering.
#[wasm_bindgen(js_name = noritoDecodeInstruction)]
pub fn decode_instruction_frame(bytes: &[u8], network_prefix: f64) -> Result<String, JsError> {
    let prefix = checked_prefix(network_prefix).map_err(js_error)?;
    iroha_js_codec::decode_instruction_frame(bytes, prefix).map_err(js_error)
}

/// Encode strict instruction JSON as the canonical transaction instruction archive.
/// The network prefix is required and must be a finite integer fitting `u16`.
#[wasm_bindgen(js_name = noritoEncodeInstructionBoxArchive)]
pub fn encode_instruction_archive(input: &str, network_prefix: f64) -> Result<Vec<u8>, JsError> {
    let prefix = checked_prefix(network_prefix).map_err(js_error)?;
    iroha_js_codec::encode_instruction_archive(input, prefix).map_err(js_error)
}

/// Decode exactly one canonical transaction instruction archive into strict JSON.
/// The network prefix is required and selects account rendering.
#[wasm_bindgen(js_name = noritoDecodeInstructionBoxArchive)]
pub fn decode_instruction_archive(bytes: &[u8], network_prefix: f64) -> Result<String, JsError> {
    let prefix = checked_prefix(network_prefix).map_err(js_error)?;
    iroha_js_codec::decode_instruction_archive(bytes, prefix).map_err(js_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_js_codec::CodecErrorKind;

    #[test]
    fn prefix_admission_rejects_javascript_numeric_coercion() {
        for invalid in [
            -1.0,
            65536.0,
            0.5,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(
                checked_prefix(invalid).unwrap_err().kind(),
                CodecErrorKind::InvalidArgument
            );
        }
        for valid in [0_u16, 42, 369, u16::MAX] {
            assert_eq!(checked_prefix(f64::from(valid)).unwrap(), valid);
        }
    }

    #[test]
    fn account_json_matches_the_exact_browser_object_contract() {
        let parsed = parsed_json(ParsedAccountAddress {
            canonical_bytes: vec![0, 127, 255],
            network_prefix: 369,
        })
        .unwrap();
        assert_eq!(
            parsed,
            r#"{"canonicalBytes":[0,127,255],"networkPrefix":369}"#
        );
        let rendered = rendered_json(RenderedAccountAddress {
            canonical_hex: "0x00ff".to_owned(),
            i105: "quoted\"value".to_owned(),
        })
        .unwrap();
        assert_eq!(
            rendered,
            r#"{"canonicalHex":"0x00ff","i105":"quoted\"value"}"#
        );
    }

    #[test]
    fn instruction_bindings_pass_the_required_context_to_the_shared_owner() {
        // Existing canonical account fixture; render the same domainless controller
        // for each explicitly requested network before invoking the four exports.
        let fixture: Value = json::from_json(include_str!(
            "../../../javascript/iroha_js/test/fixtures/game-v1-codec.json"
        ))
        .unwrap();
        let account = fixture["vectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"].as_str() == Some("JoinGameSessionV1"))
            .unwrap()["value"]["player"]
            .as_str()
            .unwrap();
        let parsed = iroha_js_codec::account_address_parse_encoded(account, None).unwrap();
        for prefix in [369_u16, 42] {
            let account =
                iroha_js_codec::account_address_render(&parsed.canonical_bytes, prefix).unwrap();
            let account_value = Value::Object(
                [("Account".to_owned(), Value::String(account.i105))]
                    .into_iter()
                    .collect(),
            );
            let source = json::to_json(&Value::Object(
                [("Unregister".to_owned(), account_value)]
                    .into_iter()
                    .collect(),
            ))
            .unwrap();
            let frame = encode_instruction_frame(&source, f64::from(prefix)).unwrap();
            assert_eq!(
                decode_instruction_frame(&frame, f64::from(prefix)).unwrap(),
                source
            );
            let archive = encode_instruction_archive(&source, f64::from(prefix)).unwrap();
            assert_eq!(
                decode_instruction_archive(&archive, f64::from(prefix)).unwrap(),
                source
            );
        }
    }

    #[test]
    fn public_diagnostics_remain_bounded_valid_utf8() {
        let reason = format!("{}{}", "a".repeat(MAX_ERROR_BYTES - 1), "界".repeat(4));
        assert_eq!(diagnostic(&reason).len(), MAX_ERROR_BYTES - 1);
        assert_eq!(diagnostic("native reason"), "native reason");
        assert!(diagnostic(&reason).is_char_boundary(diagnostic(&reason).len()));
    }
}
