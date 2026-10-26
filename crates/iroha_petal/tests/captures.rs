//! Decodes the golden camera captures in `fixtures/petal/petal_captures_v5.json`.
//!
//! Every conforming decoder must read the lanes named in `must_decode`, the
//! turn lanes named in `must_turn` and the kana lanes named in `must_kana`, must
//! never report wrong data for any lane,
//! must report the recorded inferred corner, must follow each tracking pair from
//! its first frame into its second,
//! must recover the `must_merge` field lanes of every merge set from the merge of
//! its captures, must never let a session offer a lane of a mixed set, and must
//! reject the negatives. The reference decoder reproduces every recorded result.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::io::Read as _;
use std::path::PathBuf;

use flate2::read::ZlibDecoder;
use iroha_petal::decode::{
    DecodeOptions, DecodedFrame, LaneResult, decode, field_evidence, merge_fields, track,
};
use iroha_petal::image::Luma;
use iroha_petal::kana::{KanaLane, kana_packet, read_kana};
use iroha_petal::lanes::{Lane, decode_lane, encode_lane};
use iroha_petal::layout::SECTORS;
use iroha_petal::session::{ScanLimits, ScanSession, ScanStats};
use iroha_petal::stream::{AssemblerLimits, LaneHeader, StreamAssembler};
use iroha_petal::turns::{read_turns, turn_packet};
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
    path.push("../../fixtures/petal/petal_captures_v5.json");
    json::from_str(&std::fs::read_to_string(path).expect("fixture file")).expect("fixture json")
}

fn hex_field(entry: &Value, key: &str) -> Vec<u8> {
    hex::decode(entry.get(key).and_then(Value::as_str).expect("hex field")).expect("hex")
}

/// The recorded data of every lane: lane `D`, then the four field lanes.
fn truth(entry: &Value) -> Vec<(Lane, Vec<u8>)> {
    let fields = entry
        .get("field_data")
        .and_then(Value::as_array)
        .expect("field data");
    let mut lanes = vec![(Lane::D, hex_field(entry, "d_data"))];
    for (sector, data) in fields.iter().enumerate() {
        lanes.push((
            Lane::Field(sector as u8),
            hex::decode(data.as_str().expect("hex")).expect("hex"),
        ));
    }
    lanes
}

/// Checks every decoded lane against the recorded data and returns the lane letters.
fn read_lanes(name: &str, frame: &DecodedFrame, truth: &[(Lane, Vec<u8>)]) -> String {
    for (lane, expected) in truth {
        if let Some(result) = frame.lane(*lane) {
            assert_eq!(
                &result.data,
                expected,
                "{name}: lane {} data",
                lane.letter()
            );
        }
    }
    frame.letters()
}

fn text<'a>(entry: &'a Value, key: &str) -> &'a str {
    entry.get(key).and_then(Value::as_str).expect(key)
}

/// Reads the turn lanes of a decoded capture, checks them against the recorded data and the
/// requirements, and returns their sector digits; their atoms go to `assembler` when given.
fn read_turn_lanes(
    name: &str,
    image: &Luma,
    frame: &DecodedFrame,
    entry: &Value,
    assembler: Option<&mut StreamAssembler>,
) -> String {
    let expected: Vec<Vec<u8>> = entry
        .get("turn_data")
        .and_then(Value::as_array)
        .expect("turn data")
        .iter()
        .map(|data| hex::decode(data.as_str().expect("hex")).expect("hex"))
        .collect();
    let mut letters = String::new();
    let mut packets = Vec::new();
    for (sector, result) in read_turns(image, frame, [false; SECTORS])
        .iter()
        .enumerate()
    {
        let Some(result) = result else {
            continue;
        };
        assert_eq!(
            result.data, expected[sector],
            "{name}: turn lane {sector} data"
        );
        let field = &frame.fields[sector]
            .as_ref()
            .expect("a turn lane rides on its field lane");
        let header = LaneHeader {
            tag: field.data[0],
            frame: u16::from_be_bytes([field.data[1], field.data[2]]),
        };
        packets.push(turn_packet(header, sector as u8, &result.data).expect("turn check"));
        letters.push(Lane::Field(sector as u8).letter());
    }
    if let Some(assembler) = assembler {
        for packet in &packets {
            assembler.push_atoms(packet);
        }
    }
    let must = text(entry, "must_turn");
    assert!(
        must.chars().all(|letter| letters.contains(letter)),
        "{name}: turned {letters:?}, must turn {must:?}"
    );
    assert_eq!(
        letters,
        text(entry, "reference_turned"),
        "{name}: turn lanes read"
    );
    letters
}

/// Reads the kana lanes of a decoded capture under the header its lanes name, checks them
/// against the recorded data and the requirements, and returns their names; their atoms go to
/// `assembler` when given.
fn read_kana_lanes(
    name: &str,
    image: &Luma,
    frame: &DecodedFrame,
    entry: &Value,
    assembler: Option<&mut StreamAssembler>,
) -> String {
    let expected: Vec<Vec<u8>> = entry
        .get("kana_data")
        .and_then(Value::as_array)
        .expect("kana data")
        .iter()
        .map(|data| hex::decode(data.as_str().expect("hex")).expect("hex"))
        .collect();
    let header = frame
        .atom_packets()
        .first()
        .map(|p| p.header)
        .or_else(|| frame.beacon().map(|b| b.header))
        .expect("a decoded capture names its frame");
    let mut names = String::new();
    let mut packets = Vec::new();
    for ((k, lane), result) in KanaLane::ALL
        .into_iter()
        .enumerate()
        .zip(read_kana(image, frame, header, [false; 2]).iter().flatten())
    {
        let Some(result) = result else {
            continue;
        };
        assert_eq!(
            result.result.data,
            expected[k],
            "{name}: kana lane {}",
            lane.name()
        );
        packets.push(kana_packet(result.header, lane, &result.result.data).expect("kana check"));
        names.push_str(&lane.name());
    }
    if let Some(assembler) = assembler {
        for packet in &packets {
            assembler.push_atoms(packet);
        }
    }
    let must = entry.get("must_kana").and_then(Value::as_str).unwrap_or("");
    for required in must.as_bytes().chunks(2) {
        assert!(
            names.as_bytes().chunks(2).any(|lane| lane == required),
            "{name}: kana {names:?}, must read {must:?}"
        );
    }
    assert_eq!(
        names,
        text(entry, "reference_kana"),
        "{name}: kana lanes read"
    );
    names
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
        let name = text(capture, "name");
        let image = luma_of(capture);
        let decoded =
            decode(&image, &DecodeOptions::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected_mirror = capture.get("mirrored").and_then(Value::as_bool).unwrap();
        assert_eq!(decoded.mirrored, expected_mirror, "{name}: mirror flag");
        let read = read_lanes(name, &decoded, &truth(capture));
        let must = text(capture, "must_decode");
        assert!(
            must.chars().all(|letter| read.contains(letter)),
            "{name}: read {read:?}, must read {must:?}"
        );
        assert_eq!(
            read,
            text(capture, "reference_decoded"),
            "{name}: lanes read"
        );
        let inferred = capture.get("inferred_corner").and_then(Value::as_u64);
        assert_eq!(
            decoded.inferred_corner.map(u64::from),
            inferred,
            "{name}: inferred corner"
        );
        decoded.feed(&mut assembler);
        read_turn_lanes(name, &image, &decoded, capture, Some(&mut assembler));
        read_kana_lanes(name, &image, &decoded, capture, Some(&mut assembler));
    }
    // captures of different frames of the same stream accumulate in one assembler
    assert!(assembler.progress().atoms_received > 100);
}

#[test]
fn golden_tracks_follow_the_pose_into_the_next_frame() {
    let doc = load();
    let options = DecodeOptions::default();
    for pair in doc.get("tracks").and_then(Value::as_array).expect("tracks") {
        let name = text(pair, "name");
        let previous = decode(&luma_at(pair, "from_luma_zlib_base64"), &options)
            .unwrap_or_else(|e| panic!("{name}: first frame: {e}"));
        let followed = track(&luma_at(pair, "to_luma_zlib_base64"), &previous, &options)
            .unwrap_or_else(|| panic!("{name}: tracking lost the code"));
        let read = read_lanes(name, &followed, &truth(pair));
        let must = text(pair, "must_track");
        assert!(
            must.chars().all(|letter| read.contains(letter)),
            "{name}: tracked {read:?}, must track {must:?}"
        );
        assert_eq!(
            read,
            text(pair, "reference_tracked"),
            "{name}: lanes tracked"
        );
        let inferred = pair.get("inferred_corner").and_then(Value::as_u64);
        assert_eq!(
            followed.inferred_corner.map(u64::from),
            inferred,
            "{name}: inferred corner"
        );
        let to = luma_at(pair, "to_luma_zlib_base64");
        read_turn_lanes(name, &to, &followed, pair, None);
        read_kana_lanes(name, &to, &followed, pair, None);
    }
}

/// Decodes every capture of a merge set as recorded and pushes it through a fresh session
/// (30 fps); returns the merge of their field evidence, the lanes the session merged and its
/// counters.
fn merge_set(set: &Value) -> ([Option<LaneResult>; SECTORS], String, ScanStats) {
    let name = text(set, "name");
    let options = DecodeOptions::default();
    let truth = truth(set);
    let mut evidence = Vec::new();
    let mut session = ScanSession::new(ScanLimits::default());
    let mut merged = String::new();
    let captures = set
        .get("captures")
        .and_then(Value::as_array)
        .expect("captures");
    for (i, capture) in captures.iter().enumerate() {
        let mut entry = capture.clone();
        for key in ["width", "height"] {
            if let (Some(map), Some(value)) = (entry.as_object_mut(), set.get(key)) {
                map.insert(key.to_string(), value.clone());
            }
        }
        let image = luma_of(&entry);
        let decoded =
            decode(&image, &options).unwrap_or_else(|e| panic!("{name} capture {i}: {e}"));
        let read = read_lanes(&format!("{name} capture {i}"), &decoded, &truth);
        let must = text(capture, "must_decode");
        assert!(
            must.chars().all(|letter| read.contains(letter)),
            "{name} capture {i}: read {read:?}, must read {must:?}"
        );
        assert_eq!(
            read,
            text(capture, "reference_decoded"),
            "{name} capture {i}: lanes read alone"
        );
        evidence.push(field_evidence(&image, &decoded, &options).expect("field evidence"));
        merged.push_str(&session.push(&image, 33 * i as u64).merged);
    }
    (
        merge_fields(&evidence, [false; SECTORS]),
        merged,
        session.stats(),
    )
}

/// Checks merged field lanes against the recorded data and returns their sector digits.
fn merged_letters(name: &str, results: &[Option<LaneResult>; SECTORS], set: &Value) -> String {
    let truth = truth(set);
    let mut letters = String::new();
    for (sector, result) in results.iter().enumerate() {
        if let Some(result) = result {
            let lane = Lane::Field(sector as u8);
            let expected = &truth.iter().find(|(l, _)| *l == lane).unwrap().1;
            assert_eq!(&result.data, expected, "{name}: merged lane {sector} data");
            letters.push(lane.letter());
        }
    }
    letters
}

#[test]
fn golden_merge_sets_recover_lanes_no_capture_reads_alone() {
    let doc = load();
    for set in doc.get("merges").and_then(Value::as_array).expect("merges") {
        let name = text(set, "name");
        let (results, session_merged, stats) = merge_set(set);
        let merged = merged_letters(name, &results, set);
        assert_eq!(
            merged,
            text(set, "reference_merged"),
            "{name}: merged lanes"
        );
        for letter in text(set, "must_merge").chars() {
            assert!(
                session_merged.contains(letter),
                "{name}: the session did not recover lane {letter} (merged {session_merged:?})"
            );
        }
        assert!(stats.merges > 0, "{name}: no merge");
    }
}

#[test]
fn golden_mixed_sets_never_offer_the_other_frames_lanes() {
    let doc = load();
    for set in doc
        .get("mixed_merges")
        .and_then(Value::as_array)
        .expect("mixed merges")
    {
        let name = text(set, "name");
        let (results, session_merged, stats) = merge_set(set);
        // the merge itself reads the petal field, which shows another frame than the rings
        let merged = merged_letters(name, &results, set);
        let rings_frame = hex_field(set, "d_data")[1..3].to_vec();
        for result in results.iter().flatten() {
            assert_ne!(
                result.data[1..3],
                rings_frame[..],
                "{name}: a merged lane names the rings' frame"
            );
        }
        assert_eq!(
            merged,
            text(set, "reference_merged"),
            "{name}: merged lanes"
        );
        assert!(stats.merges > 0, "{name}: no merge was tried");
        assert_eq!(
            session_merged, "",
            "{name}: the session offered another frame's lanes"
        );
        assert_eq!(stats.merged_lanes, 0, "{name}");
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
        for (lane, data) in truth(capture) {
            let word = encode_lane(lane, &data);
            assert_eq!(decode_lane(lane, &word, &[]).unwrap(), data);
        }
    }
}
