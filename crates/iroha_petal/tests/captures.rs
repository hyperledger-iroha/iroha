//! Decodes the golden camera captures in `fixtures/petal/petal_captures_v1.json`.
//!
//! Every conforming decoder must read the lanes named in `must_decode`, must
//! never report wrong data for any lane, and must reject the negatives.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::io::Read as _;
use std::path::PathBuf;

use flate2::read::ZlibDecoder;
use iroha_petal::decode::{DecodeOptions, decode};
use iroha_petal::image::Luma;
use iroha_petal::lanes::{Lane, decode_lane};
use iroha_petal::stream::{AssemblerLimits, StreamAssembler};
use norito::json::{self, Value};

fn base64_decode(text: &str) -> Vec<u8> {
    let value = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("bad base64"),
        }
    };
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let mut n = 0u32;
        let mut real = 0;
        for &c in chunk {
            n <<= 6;
            if c != b'=' {
                n |= value(c);
                real += 1;
            }
        }
        n <<= 6 * (4 - chunk.len()) as u32;
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..real - 1]);
    }
    out
}

fn luma_of(entry: &Value) -> Luma {
    let compressed = base64_decode(
        entry
            .get("luma_zlib_base64")
            .and_then(Value::as_str)
            .expect("luma"),
    );
    let mut data = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut data)
        .expect("inflate");
    let width = entry.get("width").and_then(Value::as_u64).expect("width") as usize;
    let height = entry.get("height").and_then(Value::as_u64).expect("height") as usize;
    Luma::from_raw(width, height, data).expect("luma size")
}

fn load() -> Value {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../fixtures/petal/petal_captures_v1.json");
    json::from_str(&std::fs::read_to_string(path).expect("fixture file")).expect("fixture json")
}

fn hex_field(entry: &Value, key: &str) -> Vec<u8> {
    hex::decode(entry.get(key).and_then(Value::as_str).expect("hex field")).expect("hex")
}

#[test]
fn golden_captures_decode_as_recorded() {
    let doc = load();
    let mut assembler = StreamAssembler::new(AssemblerLimits::default());
    for capture in doc
        .get("captures")
        .and_then(Value::as_array)
        .expect("captures")
    {
        let name = capture.get("name").and_then(Value::as_str).unwrap();
        let image = luma_of(capture);
        let decoded =
            decode(&image, &DecodeOptions::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected_mirror = capture.get("mirrored").and_then(Value::as_bool).unwrap();
        assert_eq!(decoded.mirrored, expected_mirror, "{name}: mirror flag");
        let lanes = [
            ('P', &decoded.p, hex_field(capture, "p_data")),
            ('K', &decoded.k, hex_field(capture, "k_data")),
            ('D', &decoded.d, hex_field(capture, "d_data")),
        ];
        let must = capture.get("must_decode").and_then(Value::as_str).unwrap();
        for (letter, lane, expected) in lanes {
            match lane {
                Some(result) => assert_eq!(result.data, expected, "{name}: lane {letter} data"),
                None => assert!(
                    !must.contains(letter),
                    "{name}: required lane {letter} was not decoded"
                ),
            }
        }
        decoded.feed(&mut assembler);
    }
    // captures of different frames of the same stream accumulate in one assembler
    assert!(assembler.progress().atoms_received > 10);
}

#[test]
fn negative_captures_are_rejected() {
    let doc = load();
    for negative in doc
        .get("negatives")
        .and_then(Value::as_array)
        .expect("negatives")
    {
        assert!(decode(&luma_of(negative), &DecodeOptions::default()).is_err());
    }
}

#[test]
fn recorded_lane_data_is_a_valid_codeword_of_its_stream() {
    let doc = load();
    for capture in doc
        .get("captures")
        .and_then(Value::as_array)
        .expect("captures")
    {
        let data = hex_field(capture, "p_data");
        let word = iroha_petal::lanes::encode_lane(Lane::P, &data);
        assert_eq!(decode_lane(Lane::P, &word, &[]).unwrap(), data);
    }
}
