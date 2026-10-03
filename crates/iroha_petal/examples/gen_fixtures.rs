//! Writes the cross-SDK golden vectors to `fixtures/petal/petal_stream_v1.json`.
//!
//! `cargo run -p iroha_petal --example gen_fixtures -- <output path>`
use std::fmt::Write as _;

use iroha_petal::crc::crc32c;
use iroha_petal::fountain::{mask_words, mix32};
use iroha_petal::glyphs::{GLYPH_CHARS, STROKE_WIDTH, STROKES, TEMPLATES};
use iroha_petal::lanes::{Lane, encode_lane};
use iroha_petal::layout::{
    CANVAS, DOT_RADIUS, FINDER_CENTERS, MASK, RING_RADII, RING_SLOTS, SlotRole, TILE_ORIGIN,
    TILE_PITCH, TILE_SIZE, TILES, data_slots, slot_roles,
};
use iroha_petal::prng::Xorshift32;
use iroha_petal::rs::ReedSolomon;
use iroha_petal::stream::{StreamEncoder, first_atom_id};

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

fn payload(len: usize, seed: u32) -> Vec<u8> {
    let mut rng = Xorshift32::new(seed);
    (0..len).map(|_| rng.next_byte()).collect()
}

fn main() {
    let path = std::env::args().nth(1).expect("output path");
    let mut out = String::new();
    out.push_str("{\n  \"fixture_version\": 1,\n  \"format\": \"petal-stream-v1\",\n");
    // constants
    let _ = writeln!(
        out,
        "  \"constants\": {{\"canvas\": {CANVAS}, \"tile_origin\": {TILE_ORIGIN}, \"tile_pitch\": {TILE_PITCH}, \"tile_size\": {TILE_SIZE}, \"dot_radius\": {DOT_RADIUS}, \"atom_len\": {}, \"p_word\": 32, \"k_word\": 128, \"d_word\": 30, \"p_parity\": {}, \"k_parity\": {}, \"d_parity\": {}, \"beacon_interval\": 4, \"format_version\": 16}},",
        iroha_petal::lanes::ATOM_LEN,
        iroha_petal::lanes::P_PARITY,
        iroha_petal::lanes::K_PARITY,
        iroha_petal::lanes::D_PARITY
    );
    // layout
    let roles = slot_roles();
    let gates: Vec<usize> = (0..roles.len())
        .filter(|&i| roles[i] == SlotRole::Gate)
        .collect();
    let guards: Vec<usize> = (0..roles.len())
        .filter(|&i| roles[i] == SlotRole::Guard)
        .collect();
    let _ = writeln!(
        out,
        "  \"layout\": {{\n    \"mask\": [{}],\n    \"tiles_col_row\": {},\n    \"ring_radii\": {},\n    \"ring_slots\": {},\n    \"finder_centers\": {},\n    \"data_slots\": {},\n    \"gate_slots\": {},\n    \"guard_slots\": {}\n  }},",
        MASK.iter()
            .map(|r| format!("\"{r}\""))
            .collect::<Vec<_>>()
            .join(", "),
        list(
            &TILES
                .iter()
                .flat_map(|&(c, r)| [u32::from(c), u32::from(r)])
                .collect::<Vec<_>>()
        ),
        list(&RING_RADII),
        list(&RING_SLOTS),
        list(
            &FINDER_CENTERS
                .iter()
                .flat_map(|&(x, y)| [x, y])
                .collect::<Vec<_>>()
        ),
        list(&data_slots()),
        list(&gates),
        list(&guards)
    );
    // glyphs
    let strokes: Vec<String> = STROKES
        .iter()
        .map(|glyph| {
            let polylines: Vec<String> = glyph
                .iter()
                .map(|stroke| list(&stroke.iter().flat_map(|&(x, y)| [x, y]).collect::<Vec<_>>()))
                .collect();
            format!("[{}]", polylines.join(", "))
        })
        .collect();
    let templates: Vec<String> = TEMPLATES.iter().map(|t| list(t)).collect();
    let _ = writeln!(
        out,
        "  \"glyphs\": {{\n    \"chars\": \"{}\",\n    \"stroke_width\": {STROKE_WIDTH},\n    \"strokes\": [{}],\n    \"templates\": [{}]\n  }},",
        GLYPH_CHARS.iter().collect::<String>(),
        strokes.join(", "),
        templates.join(",\n      ")
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
    let _ = writeln!(
        out,
        "  \"whitening\": {{\"P\": \"{}\", \"K\": \"{}\", \"D\": \"{}\"}},",
        hex(&Lane::P.whitening()),
        hex(&Lane::K.whitening()),
        hex(&Lane::D.whitening())
    );
    // reed-solomon
    let mut rs_cases = Vec::new();
    for (nsym, len, seed) in [
        (10usize, 16usize, 1u32),
        (13, 19, 2),
        (45, 83, 3),
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
            700,
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
            let (p, k, d) = encoder.lane_data(frame);
            let cells = encoder.cells(frame);
            frame_json.push(format!(
                "      {{\"frame\": {frame}, \"p_data\": \"{}\", \"k_data\": \"{}\", \"d_data\": \"{}\", \"p_word\": \"{}\", \"k_word\": \"{}\", \"d_word\": \"{}\", \"glyphs\": \"{}\", \"lit_dots\": {}}}",
                hex(&p),
                hex(&k),
                hex(&d),
                hex(&encode_lane(Lane::P, &p)),
                hex(&encode_lane(Lane::K, &k)),
                hex(&encode_lane(Lane::D, &d)),
                cells.glyph.iter().fold(String::new(), |mut acc, g| {
                    let _ = write!(acc, "{g:x}");
                    acc
                }),
                list(&(0..cells.dots.len()).filter(|&i| cells.dots[i]).collect::<Vec<_>>())
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
