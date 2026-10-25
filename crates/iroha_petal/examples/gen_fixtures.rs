//! Writes the cross-SDK golden vectors to `fixtures/petal/petal_stream_v2.json`.
//!
//! `cargo run -p iroha_petal --example gen_fixtures -- <output path>`
use std::fmt::Write as _;

use iroha_petal::crc::crc32c;
use iroha_petal::fountain::{mask_words, mix32};
use iroha_petal::lanes::{
    ATOM_LEN, ATOMS_PER_FRAME, D_PARITY, D_WORD, FIELD_ATOMS, FIELD_PARITY, FIELD_WORD, Lane,
};
use iroha_petal::layout::{
    BACKDROP_RADIUS, BAND_INNER, BAND_SLOTS, BAND_SPACING, CANVAS, DOT_RADIUS, FIELD_CELLS,
    FINDER_CENTERS, MASK, MASK_ORIGIN, MASK_PITCH, PETAL_HALF_LENGTH, PETAL_HALF_WIDTH,
    PETAL_NOTCH_DISTANCE, PETAL_NOTCH_RADIUS, PETAL_RADIUS, RING_RADII, RING_SLOTS, SECTORS,
    SlotRole, cell_light, data_slots, sector_cells, slot_roles,
};
use iroha_petal::prng::Xorshift32;
use iroha_petal::render::Palette;
use iroha_petal::rs::ReedSolomon;
use iroha_petal::stream::{FORMAT_VERSION, StreamEncoder, first_atom_id};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn list<T: ToString>(items: &[T]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Packs one flag per field cell into bytes, cell 0 in the most significant bit of byte 0.
fn bitmap(flags: impl Iterator<Item = bool>) -> Vec<u8> {
    let mut out = vec![0u8; FIELD_CELLS.div_ceil(8)];
    for (cell, flag) in flags.enumerate() {
        if flag {
            out[cell / 8] |= 1 << (7 - cell % 8);
        }
    }
    out
}

fn payload(len: usize, seed: u32) -> Vec<u8> {
    let mut rng = Xorshift32::new(seed);
    (0..len).map(|_| rng.next_byte()).collect()
}

fn main() {
    let path = std::env::args().nth(1).expect("output path");
    let mut out = String::new();
    out.push_str("{\n  \"fixture_version\": 2,\n  \"format\": \"petal-stream-v2\",\n");
    // constants
    let _ = writeln!(
        out,
        "  \"constants\": {{\"canvas\": {CANVAS}, \"atom_len\": {ATOM_LEN}, \"d_word\": {D_WORD}, \"d_parity\": {D_PARITY}, \"field_word\": {FIELD_WORD}, \"field_parity\": {FIELD_PARITY}, \"field_atoms\": {FIELD_ATOMS}, \"sectors\": {SECTORS}, \"atoms_per_frame\": {ATOMS_PER_FRAME}, \"field_cells\": {FIELD_CELLS}, \"beacon_interval\": 4, \"format_version\": {FORMAT_VERSION}}},"
    );
    // layout
    let roles = slot_roles();
    let gates: Vec<usize> = (0..roles.len())
        .filter(|&i| roles[i] == SlotRole::Gate)
        .collect();
    let guards: Vec<usize> = (0..roles.len())
        .filter(|&i| roles[i] == SlotRole::Guard)
        .collect();
    let sectors: Vec<String> = (0..SECTORS).map(|s| list(sector_cells(s))).collect();
    let _ = writeln!(
        out,
        "  \"layout\": {{\n    \"finder_centers\": {},\n    \"band_inner\": {BAND_INNER},\n    \"band_spacing\": {BAND_SPACING},\n    \"band_slots\": {},\n    \"petal_radius\": {PETAL_RADIUS},\n    \"petal_half_length\": {PETAL_HALF_LENGTH},\n    \"petal_half_width\": {PETAL_HALF_WIDTH},\n    \"petal_notch_radius\": {PETAL_NOTCH_RADIUS},\n    \"petal_notch_distance\": {PETAL_NOTCH_DISTANCE},\n    \"sector_cells\": [{}],\n    \"mask\": [{}],\n    \"mask_origin\": {MASK_ORIGIN},\n    \"mask_pitch\": {MASK_PITCH},\n    \"backdrop_radius\": {BACKDROP_RADIUS},\n    \"light_cells_hex\": \"{}\",\n    \"ring_radii\": {},\n    \"ring_slots\": {},\n    \"dot_radius\": {DOT_RADIUS},\n    \"data_slots\": {},\n    \"gate_slots\": {},\n    \"guard_slots\": {}\n  }},",
        list(
            &FINDER_CENTERS
                .iter()
                .flat_map(|&(x, y)| [x, y])
                .collect::<Vec<_>>()
        ),
        list(&BAND_SLOTS),
        sectors.join(",\n      "),
        MASK.iter()
            .map(|r| format!("\"{r}\""))
            .collect::<Vec<_>>()
            .join(", "),
        hex(&bitmap((0..FIELD_CELLS).map(cell_light))),
        list(&RING_RADII),
        list(&RING_SLOTS),
        list(&data_slots()),
        list(&gates),
        list(&guards)
    );
    let palette = Palette::default();
    let _ = writeln!(
        out,
        "  \"palette\": {{\"background\": {}, \"light\": {}, \"pink\": {}, \"backdrop\": {}}},",
        list(&palette.background),
        list(&palette.light),
        list(&palette.pink),
        list(&palette.backdrop)
    );
    // crc32c
    let crc_cases: Vec<&[u8]> = vec![
        b"",
        b"123456789",
        b"Petal Stream",
        &[0u8; 32],
        &[0xFFu8; 17],
    ];
    let crcs: Vec<String> = crc_cases
        .iter()
        .map(|c| {
            format!(
                "{{\"input_hex\": \"{}\", \"crc32c\": {}}}",
                hex(c),
                crc32c(c)
            )
        })
        .collect();
    let _ = writeln!(out, "  \"crc32c\": [{}],", crcs.join(", "));
    // prng + mixer
    let mut rng = Xorshift32::new(1);
    let xs: Vec<u32> = (0..6).map(|_| rng.next_u32()).collect();
    let _ = writeln!(
        out,
        "  \"prng\": {{\"xorshift32_seed1\": {}, \"mix32\": [{{\"in\": 0, \"out\": {}}}, {{\"in\": 1, \"out\": {}}}, {{\"in\": 305419896, \"out\": {}}}]}},",
        list(&xs),
        mix32(0),
        mix32(1),
        mix32(0x1234_5678)
    );
    // whitening
    let whitening: Vec<String> = Lane::ALL
        .iter()
        .map(|lane| {
            let name = match lane {
                Lane::D => "D".to_owned(),
                Lane::Field(sector) => format!("F{sector}"),
            };
            format!("\"{name}\": \"{}\"", hex(&lane.whitening()))
        })
        .collect();
    let _ = writeln!(out, "  \"whitening\": {{{}}},", whitening.join(", "));
    // reed-solomon
    let mut rs_cases = Vec::new();
    for (nsym, len, seed) in [
        (10usize, 16usize, 1u32),
        (22, 83, 2),
        (11, 19, 4),
        (2, 5, 5),
    ] {
        let data = payload(len, seed);
        let word = ReedSolomon::new(nsym).encode(&data);
        rs_cases.push(format!(
            "{{\"nsym\": {nsym}, \"data_hex\": \"{}\", \"codeword_hex\": \"{}\"}}",
            hex(&data),
            hex(&word)
        ));
    }
    let _ = writeln!(out, "  \"reed_solomon\": [{}],", rs_cases.join(",\n    "));
    // fountain masks
    let mut mask_cases = Vec::new();
    for (k, crc, id) in [
        (1usize, 7u32, 0u32),
        (1, 7, 5),
        (40, 0xDEAD_BEEF, 39),
        (40, 0xDEAD_BEEF, 40),
        (40, 0xDEAD_BEEF, 41),
        (100, 12345, 1000),
        (4096, 1, 70000),
    ] {
        let words = mask_words(k, crc, id);
        mask_cases.push(format!(
            "{{\"k\": {k}, \"crc\": {crc}, \"id\": {id}, \"mask\": {}}}",
            list(&words)
        ));
    }
    let _ = writeln!(
        out,
        "  \"fountain_masks\": [{}],",
        mask_cases.join(",\n    ")
    );
    // atom id schedule
    let ids: Vec<u32> = [0u16, 1, 2, 3, 4, 5, 8, 100, 65535]
        .iter()
        .map(|&f| first_atom_id(f))
        .collect();
    let _ = writeln!(
        out,
        "  \"first_atom_ids\": {{\"frames\": [0, 1, 2, 3, 4, 5, 8, 100, 65535], \"ids\": {}}},",
        list(&ids)
    );
    // streams
    let mut streams = Vec::new();
    for (name, len, seed, kind, frames) in [
        ("tiny", 13usize, 1u32, 1u8, vec![0u16, 1, 2, 3, 4]),
        (
            "one-pass",
            3000,
            2,
            2,
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        ),
        ("wrap", 3000, 3, 3, vec![65534, 65535, 0, 1]),
    ] {
        let data = payload(len, seed);
        let encoder = StreamEncoder::new(&data, kind).expect("stream");
        let meta = encoder.meta();
        let mut frame_json = Vec::new();
        for &frame in &frames {
            let lanes = encoder.lane_data(frame);
            let words = encoder.words(frame);
            let cells = encoder.cells(frame);
            let quoted = |items: &[Vec<u8>]| {
                format!(
                    "[{}]",
                    items
                        .iter()
                        .map(|i| format!("\"{}\"", hex(i)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            frame_json.push(format!(
                "      {{\"frame\": {frame}, \"d_data\": \"{}\", \"field_data\": {}, \"d_word\": \"{}\", \"field_words\": {}, \"lit_dots\": {}, \"lit_petals_hex\": \"{}\"}}",
                hex(&lanes.d),
                quoted(&lanes.fields),
                hex(&words.d),
                quoted(&words.fields),
                list(&(0..cells.dots.len()).filter(|&i| cells.dots[i]).collect::<Vec<_>>()),
                hex(&bitmap(cells.petals.iter().copied()))
            ));
        }
        streams.push(format!(
            "    {{\n      \"name\": \"{name}\", \"payload_hex\": \"{}\", \"kind\": {kind}, \"len\": {}, \"crc32c\": {}, \"tag\": {}, \"source_atoms\": {}, \"systematic_frames\": {},\n      \"frames\": [\n{}\n      ]\n    }}",
            hex(&data),
            meta.len,
            meta.crc,
            meta.tag(),
            meta.source_atoms(),
            encoder.systematic_frames(),
            frame_json.join(",\n")
        ));
    }
    let _ = writeln!(out, "  \"streams\": [\n{}\n  ]\n}}", streams.join(",\n"));
    std::fs::write(&path, out).expect("write fixture");
    println!("wrote {path}");
}
