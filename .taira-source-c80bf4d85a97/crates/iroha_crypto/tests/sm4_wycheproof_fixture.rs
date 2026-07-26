//! Helpers for loading invalid Wycheproof SM4 test vectors used by regression tests.

use norito::json::{self, Value};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// SM4 AEAD mode exposed by the Wycheproof fixtures.
pub enum Sm4WycheproofMode {
    /// SM4-GCM vectors.
    Gcm,
    /// SM4-CCM vectors.
    Ccm,
}

#[derive(Clone, Debug)]
/// A single invalid Wycheproof SM4 test case.
pub struct Sm4WycheproofCase {
    /// AEAD mode covered by the case.
    pub mode: Sm4WycheproofMode,
    /// Wycheproof-assigned test case identifier.
    pub tc_id: u32,
    /// Free-form description of the scenario.
    pub comment: String,
    /// SM4 key bytes.
    pub key: [u8; 16],
    /// Nonce or IV supplied by the case.
    pub nonce: Vec<u8>,
    /// Additional authenticated data.
    pub aad: Vec<u8>,
    /// Ciphertext payload.
    pub ciphertext: Vec<u8>,
    /// Authentication tag bytes.
    pub tag: Vec<u8>,
}

/// Load all invalid Wycheproof SM4 cases from the embedded JSON fixture.
pub fn load_sm4_wycheproof_cases() -> Vec<Sm4WycheproofCase> {
    let raw = include_str!("fixtures/wycheproof_sm4.json");
    let root: Value = json::from_str(raw).expect("parse wycheproof_sm4.json");

    let groups = root
        .as_object()
        .and_then(|object| object.get("testGroups"))
        .and_then(Value::as_array)
        .expect("Wycheproof SM4 fixture missing testGroups array");

    let mut cases = Vec::new();
    for group in groups {
        let group_obj = group
            .as_object()
            .expect("Wycheproof SM4 group must be an object");

        let mode = group_obj
            .get("mode")
            .and_then(Value::as_str)
            .expect("Wycheproof SM4 group missing mode");
        let mode = match mode.to_ascii_lowercase().as_str() {
            "gcm" => Sm4WycheproofMode::Gcm,
            "ccm" => Sm4WycheproofMode::Ccm,
            other => panic!("unknown Wycheproof SM4 mode {other}"),
        };

        let tests = group_obj
            .get("tests")
            .and_then(Value::as_array)
            .expect("Wycheproof SM4 group missing tests array");

        for test_value in tests {
            let test = test_value
                .as_object()
                .expect("Wycheproof SM4 test must be an object");

            let result = test
                .get("result")
                .and_then(Value::as_str)
                .expect("Wycheproof SM4 test missing result");
            if result != "invalid" {
                continue;
            }

            let tc_id = u32::try_from(
                test.get("tcId")
                    .and_then(Value::as_u64)
                    .expect("Wycheproof SM4 test missing tcId"),
            )
            .expect("Wycheproof SM4 tcId out of range for u32");
            let comment = test
                .get("comment")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let key_hex = test
                .get("key")
                .and_then(Value::as_str)
                .expect("Wycheproof SM4 test missing key");
            let nonce_hex = test
                .get("nonce")
                .or_else(|| test.get("iv"))
                .and_then(Value::as_str)
                .expect("Wycheproof SM4 test missing nonce/iv");
            let aad_hex = test.get("aad").and_then(Value::as_str).unwrap_or_default();
            let ciphertext_hex = test
                .get("ciphertext")
                .or_else(|| test.get("ct"))
                .and_then(Value::as_str)
                .expect("Wycheproof SM4 test missing ciphertext");
            let tag_hex = test
                .get("tag")
                .and_then(Value::as_str)
                .expect("Wycheproof SM4 test missing tag");

            cases.push(Sm4WycheproofCase {
                mode,
                tc_id,
                comment,
                key: hex_to_array::<16>(key_hex),
                nonce: hex_to_vec(nonce_hex),
                aad: hex_to_vec(aad_hex),
                ciphertext: hex_to_vec(ciphertext_hex),
                tag: hex_to_vec(tag_hex),
            });
        }
    }

    assert!(
        !cases.is_empty(),
        "Wycheproof SM4 fixture must contain at least one invalid case"
    );
    cases
}

/// Decode a hexadecimal string into a byte vector.
fn hex_to_vec(input: &str) -> Vec<u8> {
    if input.is_empty() {
        Vec::new()
    } else {
        hex::decode(input).unwrap_or_else(|err| panic!("invalid hex {input}: {err}"))
    }
}

/// Decode a hexadecimal string into a fixed-size byte array.
fn hex_to_array<const N: usize>(input: &str) -> [u8; N] {
    let bytes = hex_to_vec(input);
    bytes
        .as_slice()
        .try_into()
        .unwrap_or_else(|_| panic!("expected {N} bytes"))
}
