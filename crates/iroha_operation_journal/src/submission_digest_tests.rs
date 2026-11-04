//! Exact digest text, submission marker bytes and original operation serialization.

use super::*;
use std::{cell::Cell, fmt::Write as _};

fn standard_hex(bytes: &[u8]) -> String {
    let mut text = String::new();
    for byte in bytes {
        write!(&mut text, "{byte:02x}").unwrap();
    }
    text
}

#[test]
fn submission_digest_preserves_leading_zeroes_and_byte_order() {
    assert_eq!(submission_digest_hex(&[0; 32]), [b'0'; 64]);
    assert_eq!(submission_digest_hex(&[255; 32]), [b'f'; 64]);
    let digest = std::array::from_fn(|index| u8::try_from(index).unwrap());
    assert_eq!(
        &submission_digest_hex(&digest),
        b"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
    );
}

#[test]
fn submission_digest_matches_standard_hex_for_every_byte_and_position() {
    for position in 0..32 {
        for byte in u8::MIN..=u8::MAX {
            let mut digest = [0xa5; 32];
            digest[position] = byte;
            assert_eq!(
                submission_digest_hex(&digest),
                standard_hex(&digest).as_bytes()
            );
        }
    }
}

#[test]
fn submission_marker_preserves_exact_schema_key_order_and_digest_goldens() {
    for (operation, expected_digest) in [
        (
            json::Value::Null,
            "74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b",
        ),
        (
            norito::json!({}),
            "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
        ),
        (
            norito::json!([]),
            "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        ),
        (
            json::Value::from(""),
            "12ae32cb1ec02d01eda3581b127c1fee3b0dc53572ed6baf239721a03d82e126",
        ),
    ] {
        let expected = format!(
            "{{\"operation_sha256\":\"{expected_digest}\",\"schema\":\"iroha.wallet.submission-intent.v1\"}}"
        );
        assert_eq!(
            Journal::submission_bytes(&operation).unwrap(),
            expected.as_bytes()
        );
    }
}

#[test]
fn submission_marker_matches_original_value_encoding_for_nested_operations() {
    for index in 0..256_u16 {
        let operation = norito::json!({
            "z": [true, null, index, "μ東京\n\"\\\0"],
            "a": {"sequence": index, "empty": ""},
        });
        let digest = Sha256::digest(json::to_vec(&operation).unwrap());
        let expected = json::to_vec(&norito::json!({
            "schema": "iroha.wallet.submission-intent.v1",
            "operation_sha256": (standard_hex(&digest)),
        }))
        .unwrap();
        assert_eq!(Journal::submission_bytes(&operation).unwrap(), expected);
    }
}

#[test]
fn submission_marker_hashes_the_original_serialization_exactly_once() {
    struct Operation(Cell<usize>);
    impl JsonSerialize for Operation {
        fn json_serialize(&self, out: &mut String) {
            self.0.set(self.0.get() + 1);
            // Preserve the caller's exact serialization, including its whitespace.
            out.push_str("{ \"z\": 1, \"a\": 2 }");
        }
    }
    let operation = Operation(Cell::new(0));
    let digest = standard_hex(&Sha256::digest(b"{ \"z\": 1, \"a\": 2 }"));
    let expected = format!(
        "{{\"operation_sha256\":\"{digest}\",\"schema\":\"iroha.wallet.submission-intent.v1\"}}"
    );
    assert_eq!(
        Journal::submission_bytes(&operation).unwrap(),
        expected.as_bytes()
    );
    assert_eq!(operation.0.get(), 1);
}
