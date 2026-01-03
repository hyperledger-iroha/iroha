//! Cross-language fixture parity with Python and Java bindings.

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let cleaned: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    cleaned
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            let hi = chunk[0];
            let lo = chunk[1];
            let hi = (hi as char).to_digit(16).expect("hex") as u8;
            let lo = (lo as char).to_digit(16).expect("hex") as u8;
            (hi << 4) | lo
        })
        .collect()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn python_sequence_fixture_with_wrong_minor_version_is_rejected() {
    let hex = "4e5254300027055a2713834c8240055a2713834c82400015000000000000002fc2516e36ed1be83f040404040401000000020000000300000004000000";
    let bytes = hex_to_bytes(hex);
    assert_eq!(&bytes[0..4], b"NRT0");
    assert_eq!(bytes[4], norito::core::VERSION_MAJOR);
    assert_ne!(
        bytes[5],
        norito::core::VERSION_MINOR,
        "fixture should exercise non-v1 minor version",
    );
    let err = norito::decode_from_bytes::<Vec<u8>>(&bytes)
        .expect_err("fixture must be rejected in v1");
    assert!(matches!(err, norito::Error::UnsupportedMinorVersion { .. }));
}

#[test]
fn decode_python_adaptive_rows_fixture() {
    let hex = "0103000000530000000100000000000000020200000000000005000000080000000f000000616c696365626f62636861726c696505";
    let bytes = hex_to_bytes(hex);
    let rows = [
        (1u64, "alice", true),
        (2, "bob", false),
        (3, "charlie", true),
    ];
    let generated = norito::columnar::encode_rows_u64_str_bool_adaptive(&rows);
    assert_eq!(
        bytes,
        generated,
        "update python fixture hex to {}",
        bytes_to_hex(&generated)
    );
    let decoded = norito::columnar::decode_rows_u64_str_bool_adaptive(&bytes)
        .expect("decode python adaptive rows");
    let expected = vec![
        (1, String::from("alice"), true),
        (2, String::from("bob"), false),
        (3, String::from("charlie"), true),
    ];
    assert_eq!(decoded, expected);
}

#[test]
fn decode_java_adaptive_rows_fixture() {
    let hex =
        "0102000000530000000a000000000000001400000000000000050000000900000064656c74616563686f02";
    let bytes = hex_to_bytes(hex);
    let rows = [(10u64, "delta", false), (20, "echo", true)];
    let generated = norito::columnar::encode_rows_u64_str_bool_adaptive(&rows);
    assert_eq!(
        bytes,
        generated,
        "update java fixture hex to {}",
        bytes_to_hex(&generated)
    );
    let decoded = norito::columnar::decode_rows_u64_str_bool_adaptive(&bytes)
        .expect("decode java adaptive rows");
    let expected = vec![
        (10, String::from("delta"), false),
        (20, String::from("echo"), true),
    ];
    assert_eq!(decoded, expected);
}

#[cfg(feature = "schema-structural")]
#[test]
fn structural_schema_hash_matches_bindings() {
    let json = r#"
    {
        "Sample": {
            "Struct": [
                {"name": "id", "type": "u64"},
                {"name": "name", "type": "String"},
                {"name": "flag", "type": "bool"}
            ]
        },
        "String": "String",
        "bool": "bool",
        "u64": {"Int": "FixedWidth"}
    }
    "#;
    let expected = hex_to_bytes("0107fec5fc24ac0d0107fec5fc24ac0d");
    let actual = norito::core::schema_hash_structural_from_json_str(json)
        .expect("parse structural descriptor");
    assert_eq!(actual.as_slice(), expected.as_slice());
}
