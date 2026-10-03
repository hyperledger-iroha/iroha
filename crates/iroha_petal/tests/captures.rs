//! Decodes the golden camera captures in `fixtures/petal/petal_captures_v1.json`.
//!
//! Every conforming decoder must read the lanes named in `must_decode`, must
//! never report wrong data for any lane, must report the recorded inferred
//! corner, must follow each tracking pair from its first frame into its second,
//! and must reject the negatives.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::io::Read as _;
use std::path::PathBuf;

use flate2::read::ZlibDecoder;
use iroha_petal::decode::{DecodeOptions, decode, track};
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
    luma_at(entry, "luma_zlib_base64")
}

fn luma_at(entry: &Value, key: &str) -> Luma {
    let compressed = base64_decode(entry.get(key).and_then(Value::as_str).expect("luma"));
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
        let inferred = capture.get("inferred_corner").and_then(Value::as_u64);
        assert_eq!(
            decoded.inferred_corner.map(u64::from),
            inferred,
            "{name}: inferred corner"
        );
        decoded.feed(&mut assembler);
    }
    // captures of different frames of the same stream accumulate in one assembler
    assert!(assembler.progress().atoms_received > 10);
}

#[test]
fn golden_tracks_follow_the_pose_into_the_next_frame() {
    let doc = load();
    let options = DecodeOptions::default();
    for pair in doc.get("tracks").and_then(Value::as_array).expect("tracks") {
        let name = pair.get("name").and_then(Value::as_str).unwrap();
        let previous = decode(&luma_at(pair, "from_luma_zlib_base64"), &options)
            .unwrap_or_else(|e| panic!("{name}: first frame: {e}"));
        let followed = track(&luma_at(pair, "to_luma_zlib_base64"), &previous, &options)
            .unwrap_or_else(|| panic!("{name}: tracking lost the code"));
        let must = pair.get("must_track").and_then(Value::as_str).unwrap();
        for (letter, lane, expected) in [
            ('P', &followed.p, hex_field(pair, "p_data")),
            ('K', &followed.k, hex_field(pair, "k_data")),
            ('D', &followed.d, hex_field(pair, "d_data")),
        ] {
            match lane {
                Some(result) => assert_eq!(result.data, expected, "{name}: lane {letter} data"),
                None => assert!(!must.contains(letter), "{name}: lane {letter} lost"),
            }
        }
        let inferred = pair.get("inferred_corner").and_then(Value::as_u64);
        assert_eq!(
            followed.inferred_corner.map(u64::from),
            inferred,
            "{name}: inferred corner"
        );
    }
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
