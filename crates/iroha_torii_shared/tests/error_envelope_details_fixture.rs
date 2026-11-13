//! Cross-SDK `ErrorEnvelope` frames whose `details` carry Torii's admission,
//! throttling and observation metadata, pinned to the authoritative codec.
//!
//! The JavaScript and Python SDK decoders read
//! `fixtures/torii/error_envelope_details_v1.json`: they must decode every
//! `decoded` row exactly and leave every `undecoded` row to the caller. Each row
//! here must be a value of the real [`ErrorEnvelope`] whose canonical Norito
//! frame and Norito JSON rendering are byte-identical to the fixture.
use iroha_torii_shared::ErrorEnvelope;
use norito::json::{self, Value};

const FIXTURE: &str = include_str!("../../../fixtures/torii/error_envelope_details_v1.json");

fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("fixture row field `{key}` must be a string"))
}

#[test]
fn error_envelope_details_fixture_matches_authoritative_codec() {
    let fixture: Value = json::from_str(FIXTURE).expect("error envelope details fixture parses");
    let header = fixture
        .get("norito_envelope")
        .expect("fixture names the Norito envelope header");
    let type_name = text(header, "schema_type_name");
    assert_eq!(type_name, "iroha_torii_shared::ErrorEnvelope");
    assert_eq!(
        text(header, "schema_hash_hex"),
        hex::encode(norito::core::schema_hash_for_name(type_name)),
    );
    assert_eq!(
        norito::schema::identity::frame_hash::<ErrorEnvelope>(),
        norito::core::schema_hash_for_name(type_name),
    );
    assert_eq!(text(header, "flags_hex"), "02");

    let mut checked = 0_usize;
    let mut stale = Vec::new();
    for (group, expected_rows) in [("decoded", 8_usize), ("undecoded", 2)] {
        let rows = fixture
            .get(group)
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("fixture group `{group}` must be an array"));
        assert_eq!(rows.len(), expected_rows, "fixture group `{group}` size");
        for row in rows {
            let name = text(row, "name");
            let json_text = text(row, "json_utf8");
            let envelope: ErrorEnvelope = json::from_str(json_text)
                .unwrap_or_else(|error| panic!("{name}: JSON is not an ErrorEnvelope: {error}"));
            let canonical_json = json::to_string(&envelope).expect("render Norito JSON");
            let canonical_hex =
                hex::encode(norito::to_bytes(&envelope).expect("encode Norito frame"));
            if canonical_json != json_text || canonical_hex != text(row, "norito_hex") {
                stale.push(format!(
                    "{name}:\n  norito_hex {canonical_hex}\n  json_utf8 {canonical_json}"
                ));
                continue;
            }
            let frame = hex::decode(text(row, "norito_hex")).expect("fixture frame hex");
            let decoded: ErrorEnvelope =
                norito::decode_from_bytes(&frame).expect("fixture frame decodes");
            assert_eq!(
                json::to_string(&decoded).expect("render decoded frame"),
                canonical_json,
                "{name}: the frame and the JSON must be one value"
            );
            assert!(
                norito::decode_from_bytes::<ErrorEnvelope>(&frame[..frame.len() - 1]).is_err(),
                "{name}: a truncated frame must not decode"
            );
            assert_eq!(text(row, "code"), envelope.code(), "{name}: code");
            assert_eq!(text(row, "message"), envelope.message(), "{name}: message");
            assert_eq!(
                row.get("details").expect("fixture row details"),
                &json::to_value(&envelope.details).expect("render details"),
                "{name}: details",
            );
            assert!(
                row.get("status").and_then(Value::as_u64).is_some(),
                "{name}: status"
            );
            checked += 1;
        }
    }
    assert!(
        stale.is_empty(),
        "stale error envelope details fixture rows; replace only with these authoritative codec outputs:\n{}",
        stale.join("\n"),
    );
    assert_eq!(checked, 10);
}
