//! FASTPQ packing roundtrip tests.

use std::path::{Path, PathBuf};

use fastpq_prover::{pack_bytes, unpack_bytes};

#[test]
fn roundtrip_reference_vectors() {
    for data in load_vectors() {
        let packed = pack_bytes(&data);
        assert_eq!(packed.length, data.len());
        assert_eq!(unpack_bytes(&packed), data);
    }
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn load_vectors() -> Vec<Vec<u8>> {
    let path = fixtures_dir().join("packing_roundtrip.json");
    let bytes = std::fs::read(&path).expect("read packing_roundtrip.json");
    let value: norito::json::Value =
        norito::json::from_slice(&bytes).expect("parse packing_roundtrip.json");
    let array = value
        .as_array()
        .expect("packing_roundtrip.json must contain an array");

    array
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let hex = entry
                .as_str()
                .unwrap_or_else(|| panic!("packing_roundtrip[{index}] must be a string"));
            decode_hex(hex).unwrap_or_else(|error| {
                panic!("invalid hex string at packing_roundtrip[{index}]: {error}")
            })
        })
        .collect()
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if !trimmed.len().is_multiple_of(2) {
        return Err(format!(
            "expected even number of characters, got {}",
            trimmed.len()
        ));
    }

    let mut bytes = Vec::with_capacity(trimmed.len() / 2);
    for chunk in trimmed.as_bytes().chunks_exact(2) {
        let hi = decode_nibble(chunk[0])?;
        let lo = decode_nibble(chunk[1])?;
        bytes.push((hi << 4) | lo);
    }
    Ok(bytes)
}

fn decode_nibble(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        other => Err(format!("invalid hex digit {other:?}")),
    }
}
