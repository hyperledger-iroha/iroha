//! Verifies `fixtures/petal/petal_stream_v2.json` against the reference codec.
//!
//! The same file is the conformance suite of every SDK port; this test makes
//! sure the file and the reference implementation cannot drift apart.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::path::PathBuf;

use iroha_petal::crc::crc32c;
use iroha_petal::fountain::{mask_words, mix32};
use iroha_petal::lanes::{
    ATOM_LEN, ATOMS_PER_FRAME, D_PARITY, FIELD_ATOMS, FIELD_PARITY, FIELD_WORD, FrameCells, Lane,
    decode_lane, encode_lane,
};
use iroha_petal::layout::{
    BAND_SLOTS, FIELD_CELLS, MASK, RING_RADII, RING_SLOTS, SECTORS, SlotRole, cell_light,
    data_slots, sector_cells, slot_roles,
};
use iroha_petal::prng::Xorshift32;
use iroha_petal::rs::ReedSolomon;
use iroha_petal::stream::{
    AssemblerLimits, FORMAT_VERSION, StreamAssembler, StreamEncoder, first_atom_id,
    parse_atom_lane, parse_d_lane,
};
use norito::json::{self, Value};

fn fixture() -> Value {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../fixtures/petal/petal_stream_v2.json");
    let text = std::fs::read_to_string(&path).expect("fixture file");
    json::from_str(&text).expect("fixture json")
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("missing field {key}"))
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    field(value, key)
        .as_str()
        .unwrap_or_else(|| panic!("{key} is not a string"))
}

fn number(value: &Value, key: &str) -> u64 {
    field(value, key)
        .as_u64()
        .unwrap_or_else(|| panic!("{key} is not an unsigned integer"))
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    field(value, key)
        .as_array()
        .unwrap_or_else(|| panic!("{key} is not an array"))
}

fn bytes(value: &Value, key: &str) -> Vec<u8> {
    hex::decode(text(value, key)).unwrap_or_else(|_| panic!("{key} is not hex"))
}

fn hex_list(value: &Value, key: &str) -> Vec<Vec<u8>> {
    array(value, key)
        .iter()
        .map(|v| hex::decode(v.as_str().expect("hex string")).expect("hex"))
        .collect()
}

fn numbers(values: &[Value]) -> Vec<u64> {
    values
        .iter()
        .map(|v| v.as_u64().expect("unsigned integer"))
        .collect()
}

/// One flag per field cell, cell 0 in the most significant bit of byte 0.
fn flags(bitmap: &[u8]) -> Vec<bool> {
    (0..FIELD_CELLS)
        .map(|cell| bitmap[cell / 8] >> (7 - cell % 8) & 1 == 1)
        .collect()
}

#[test]
fn constants_and_layout_match() {
    let doc = fixture();
    assert_eq!(number(&doc, "fixture_version"), 2);
    let constants = field(&doc, "constants");
    assert_eq!(number(constants, "atom_len") as usize, ATOM_LEN);
    assert_eq!(number(constants, "d_parity") as usize, D_PARITY);
    assert_eq!(number(constants, "field_word") as usize, FIELD_WORD);
    assert_eq!(number(constants, "field_parity") as usize, FIELD_PARITY);
    assert_eq!(number(constants, "field_atoms") as usize, FIELD_ATOMS);
    assert_eq!(
        number(constants, "atoms_per_frame") as usize,
        ATOMS_PER_FRAME
    );
    assert_eq!(number(constants, "field_cells") as usize, FIELD_CELLS);
    assert_eq!(
        number(constants, "format_version"),
        u64::from(FORMAT_VERSION)
    );
    let layout = field(&doc, "layout");
    let mask: Vec<&str> = array(layout, "mask")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(mask, MASK.to_vec());
    assert_eq!(
        numbers(array(layout, "band_slots")),
        BAND_SLOTS.iter().map(|&n| u64::from(n)).collect::<Vec<_>>()
    );
    let sectors = array(layout, "sector_cells");
    assert_eq!(sectors.len(), SECTORS);
    for (sector, cells) in sectors.iter().enumerate() {
        let expected: Vec<u64> = sector_cells(sector).iter().map(|&c| c as u64).collect();
        assert_eq!(
            numbers(cells.as_array().unwrap()),
            expected,
            "sector {sector}"
        );
    }
    let light = flags(&bytes(layout, "light_cells_hex"));
    assert_eq!(light, (0..FIELD_CELLS).map(cell_light).collect::<Vec<_>>());
    assert_eq!(
        numbers(array(layout, "ring_slots")),
        RING_SLOTS.iter().map(|&n| n as u64).collect::<Vec<_>>()
    );
    assert_eq!(array(layout, "ring_radii").len(), RING_RADII.len());
    let data: Vec<u64> = data_slots().into_iter().map(|s| s as u64).collect();
    assert_eq!(numbers(array(layout, "data_slots")), data);
    let roles = slot_roles();
    let of = |role: SlotRole| -> Vec<u64> {
        (0..roles.len())
            .filter(|&i| roles[i] == role)
            .map(|i| i as u64)
            .collect()
    };
    assert_eq!(numbers(array(layout, "gate_slots")), of(SlotRole::Gate));
    assert_eq!(numbers(array(layout, "guard_slots")), of(SlotRole::Guard));
}

#[test]
fn checksums_prng_and_whitening_match() {
    let doc = fixture();
    for case in array(&doc, "crc32c") {
        assert_eq!(
            u64::from(crc32c(&bytes(case, "input_hex"))),
            number(case, "crc32c")
        );
    }
    let prng = field(&doc, "prng");
    let mut rng = Xorshift32::new(1);
    let expected: Vec<u64> = (0..6).map(|_| u64::from(rng.next_u32())).collect();
    assert_eq!(numbers(array(prng, "xorshift32_seed1")), expected);
    for case in array(prng, "mix32") {
        assert_eq!(
            u64::from(mix32(number(case, "in") as u32)),
            number(case, "out")
        );
    }
    let whitening = field(&doc, "whitening");
    assert_eq!(bytes(whitening, "D"), Lane::D.whitening());
    for sector in 0..SECTORS as u8 {
        assert_eq!(
            bytes(whitening, &format!("F{sector}")),
            Lane::Field(sector).whitening()
        );
    }
}

#[test]
fn reed_solomon_vectors_encode_and_correct() {
    let doc = fixture();
    for case in array(&doc, "reed_solomon") {
        let nsym = number(case, "nsym") as usize;
        let data = bytes(case, "data_hex");
        let word = bytes(case, "codeword_hex");
        let rs = ReedSolomon::new(nsym);
        assert_eq!(rs.encode(&data), word);
        // damage up to the correction capacity and recover
        let mut damaged = word.clone();
        for position in (0..nsym / 2).map(|i| i * 3 % word.len()) {
            damaged[position] ^= 0x5A;
        }
        let mut fixed = damaged.clone();
        rs.decode(&mut fixed, &[]).expect("correctable");
        assert_eq!(fixed, word);
    }
}

#[test]
fn fountain_masks_and_atom_ids_match() {
    let doc = fixture();
    for case in array(&doc, "fountain_masks") {
        let mask = mask_words(
            number(case, "k") as usize,
            number(case, "crc") as u32,
            number(case, "id") as u32,
        );
        let expected: Vec<u32> = numbers(array(case, "mask"))
            .into_iter()
            .map(|v| v as u32)
            .collect();
        assert_eq!(mask, expected);
    }
    let ids = field(&doc, "first_atom_ids");
    for (frame, id) in numbers(array(ids, "frames"))
        .into_iter()
        .zip(numbers(array(ids, "ids")))
    {
        assert_eq!(u64::from(first_atom_id(frame as u16)), id);
    }
}

#[test]
fn streams_encode_identically_and_reassemble() {
    let doc = fixture();
    for stream in array(&doc, "streams") {
        let name = text(stream, "name");
        let payload = bytes(stream, "payload_hex");
        let encoder = StreamEncoder::new(&payload, number(stream, "kind") as u8).expect("encoder");
        let meta = encoder.meta();
        assert_eq!(u64::from(meta.len), number(stream, "len"), "{name}");
        assert_eq!(u64::from(meta.crc), number(stream, "crc32c"), "{name}");
        assert_eq!(u64::from(meta.tag()), number(stream, "tag"), "{name}");
        assert_eq!(
            meta.source_atoms() as u64,
            number(stream, "source_atoms"),
            "{name}"
        );
        assert_eq!(
            encoder.systematic_frames() as u64,
            number(stream, "systematic_frames"),
            "{name}"
        );
        for frame in array(stream, "frames") {
            let frame_no = number(frame, "frame") as u16;
            let lanes = encoder.lane_data(frame_no);
            assert_eq!(lanes.d, bytes(frame, "d_data"), "{name} frame {frame_no}");
            assert_eq!(
                lanes.fields.to_vec(),
                hex_list(frame, "field_data"),
                "{name} frame {frame_no}"
            );
            let d_word = bytes(frame, "d_word");
            let field_words = hex_list(frame, "field_words");
            assert_eq!(encode_lane(Lane::D, &lanes.d), d_word);
            for (sector, word) in field_words.iter().enumerate() {
                assert_eq!(
                    &encode_lane(Lane::Field(sector as u8), &lanes.fields[sector]),
                    word,
                    "{name} frame {frame_no} sector {sector}"
                );
            }
            let cells = FrameCells::from_words(
                &d_word,
                [
                    &field_words[0],
                    &field_words[1],
                    &field_words[2],
                    &field_words[3],
                ],
            );
            let lit: Vec<u64> = (0..cells.dots.len())
                .filter(|&i| cells.dots[i])
                .map(|i| i as u64)
                .collect();
            assert_eq!(lit, numbers(array(frame, "lit_dots")));
            assert_eq!(
                cells.petals.to_vec(),
                flags(&bytes(frame, "lit_petals_hex")),
                "{name} frame {frame_no}"
            );
            assert_eq!(cells, encoder.cells(frame_no));
        }
        // push every fixture frame through decode_lane + assembler
        if name == "one-pass" {
            let mut assembler = StreamAssembler::new(AssemblerLimits::default());
            for frame in array(stream, "frames") {
                let d = decode_lane(Lane::D, &bytes(frame, "d_word"), &[]).unwrap();
                assembler.push_d_lane(&parse_d_lane(&d).unwrap());
                for (sector, word) in hex_list(frame, "field_words").iter().enumerate() {
                    let lane = Lane::Field(sector as u8);
                    let data = decode_lane(lane, word, &[]).unwrap();
                    assembler.push_atoms(&parse_atom_lane(lane, &data).unwrap());
                }
            }
            assert_eq!(
                assembler.take_completed().expect("complete").payload,
                payload
            );
        }
    }
}
