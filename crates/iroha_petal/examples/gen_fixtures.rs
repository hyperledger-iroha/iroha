//! Writes the cross-SDK golden vectors to `fixtures/petal/petal_stream_v5.json`.
//!
//! `cargo run -p iroha_petal --example gen_fixtures -- <output path>`
use std::fmt::Write as _;

use iroha_petal::crc::crc32c;
use iroha_petal::fountain::{mask_words, mix32};
use iroha_petal::glyphs::{GLYPH_CHARS, GLYPH_GRID, STROKE_WIDTH, STROKES};
use iroha_petal::kana::{
    KANA_BLURS, KANA_DATA_TILES, KANA_GLYPH_ATOMS, KANA_GLYPH_PARITY, KANA_GLYPH_WORD, KANA_GRID,
    KANA_ID_BASE, KANA_POLARITY_ATOMS, KANA_POLARITY_PARITY, KANA_POLARITY_WORD, KanaLane,
    kana_first_id, template_probe,
};
use iroha_petal::lanes::{
    ATOM_LEN, ATOMS_PER_FRAME, D_PARITY, D_WORD, FIELD_ATOMS, FIELD_PARITY, FIELD_WORD, Lane,
};
use iroha_petal::layout::{
    BACKDROP_RADIUS, BAND_INNER, BAND_SLOTS, BAND_SPACING, CANVAS, CARD_HEIGHT, DOT_RADIUS,
    FIELD_CELLS, FINDER_CENTERS, KANA_BANDS, KANA_COLUMNS, KANA_CORNER, KANA_FINDER_CLEARANCE,
    KANA_GAP, KANA_GLYPH_BOX, KANA_MARGIN, KANA_PILOT_SYMBOLS, KANA_PILOTS, KANA_PITCH, KANA_ROWS,
    KANA_TILE, MASK, MASK_ORIGIN, MASK_PITCH, PETAL_HALF_LENGTH, PETAL_HALF_WIDTH,
    PETAL_NOTCH_DISTANCE, PETAL_NOTCH_RADIUS, PETAL_RADIUS, RING_RADII, RING_SLOTS, SECTORS,
    SQUARE_TOP, SlotRole, cell_light, data_slots, kana_tiles, sector_cells, slot_roles,
};
use iroha_petal::prng::Xorshift32;
use iroha_petal::render::Palette;
use iroha_petal::rs::ReedSolomon;
use iroha_petal::stream::{FORMAT_VERSION, StreamEncoder, first_atom_id};
use iroha_petal::turns::{
    TURN_ATOMS, TURN_BLURS, TURN_CARRIERS, TURN_ID_BASE, TURN_PARITY, TURN_POSITIONS, TURN_WORD,
    carrier_order, turn_first_id, turn_template, turn_whitening,
};

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

/// Packs one flag per katakana tile into bytes, tile 0 in the most significant bit of byte 0.
fn tile_bitmap(flags: impl Iterator<Item = bool>) -> Vec<u8> {
    let mut out = vec![0u8; kana_tiles().len().div_ceil(8)];
    for (tile, flag) in flags.enumerate() {
        if flag {
            out[tile / 8] |= 1 << (7 - tile % 8);
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
    out.push_str("{\n  \"fixture_version\": 5,\n  \"format\": \"petal-stream-v5\",\n");
    // constants
    let _ = writeln!(
        out,
        "  \"constants\": {{\"canvas\": {CANVAS}, \"atom_len\": {ATOM_LEN}, \"d_word\": {D_WORD}, \"d_parity\": {D_PARITY}, \"field_word\": {FIELD_WORD}, \"field_parity\": {FIELD_PARITY}, \"field_atoms\": {FIELD_ATOMS}, \"sectors\": {SECTORS}, \"atoms_per_frame\": {ATOMS_PER_FRAME}, \"field_cells\": {FIELD_CELLS}, \"beacon_interval\": 4, \"format_version\": {FORMAT_VERSION}, \"turn_positions\": {TURN_POSITIONS}, \"turn_carriers\": {TURN_CARRIERS}, \"turn_word\": {TURN_WORD}, \"turn_parity\": {TURN_PARITY}, \"turn_atoms\": {TURN_ATOMS}, \"turn_id_base\": {TURN_ID_BASE}, \"card_height\": {CARD_HEIGHT}, \"square_top\": {SQUARE_TOP}, \"kana_tile\": {KANA_TILE}, \"kana_pitch\": {KANA_PITCH}, \"kana_glyph_box\": {KANA_GLYPH_BOX}, \"kana_corner\": {KANA_CORNER}, \"kana_rows\": {KANA_ROWS}, \"kana_columns\": {KANA_COLUMNS}, \"kana_gap\": {KANA_GAP}, \"kana_margin\": {KANA_MARGIN}, \"kana_finder_clearance\": {KANA_FINDER_CLEARANCE}, \"kana_bands\": {KANA_BANDS}, \"kana_pilots\": {KANA_PILOTS}, \"kana_data_tiles\": {KANA_DATA_TILES}, \"kana_glyph_word\": {KANA_GLYPH_WORD}, \"kana_glyph_parity\": {KANA_GLYPH_PARITY}, \"kana_glyph_atoms\": {KANA_GLYPH_ATOMS}, \"kana_polarity_word\": {KANA_POLARITY_WORD}, \"kana_polarity_parity\": {KANA_POLARITY_PARITY}, \"kana_polarity_atoms\": {KANA_POLARITY_ATOMS}, \"kana_id_base\": {KANA_ID_BASE}}},"
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
    let orders: Vec<String> = (0..SECTORS).map(|s| list(carrier_order(s))).collect();
    let _ = writeln!(
        out,
        "  \"layout\": {{\n    \"finder_centers\": {},\n    \"band_inner\": {BAND_INNER},\n    \"band_spacing\": {BAND_SPACING},\n    \"band_slots\": {},\n    \"petal_radius\": {PETAL_RADIUS},\n    \"petal_half_length\": {PETAL_HALF_LENGTH},\n    \"petal_half_width\": {PETAL_HALF_WIDTH},\n    \"petal_notch_radius\": {PETAL_NOTCH_RADIUS},\n    \"petal_notch_distance\": {PETAL_NOTCH_DISTANCE},\n    \"sector_cells\": [{}],\n    \"carrier_orders\": [{}],\n    \"mask\": [{}],\n    \"mask_origin\": {MASK_ORIGIN},\n    \"mask_pitch\": {MASK_PITCH},\n    \"backdrop_radius\": {BACKDROP_RADIUS},\n    \"light_cells_hex\": \"{}\",\n    \"ring_radii\": {},\n    \"ring_slots\": {},\n    \"dot_radius\": {DOT_RADIUS},\n    \"data_slots\": {},\n    \"gate_slots\": {},\n    \"guard_slots\": {},\n    \"kana_tiles\": [{}],\n    \"kana_pilot_symbols\": {}\n  }},",
        list(
            &FINDER_CENTERS
                .iter()
                .flat_map(|&(x, y)| [x, y])
                .collect::<Vec<_>>()
        ),
        list(&BAND_SLOTS),
        sectors.join(",\n      "),
        orders.join(",\n      "),
        MASK.iter()
            .map(|r| format!("\"{r}\""))
            .collect::<Vec<_>>()
            .join(", "),
        hex(&bitmap((0..FIELD_CELLS).map(cell_light))),
        list(&RING_RADII),
        list(&RING_SLOTS),
        list(&data_slots()),
        list(&gates),
        list(&guards),
        kana_tiles()
            .iter()
            .map(|t| format!(
                "[{}, {}, {}, {:?}, {:?}, {}]",
                t.band,
                t.row,
                t.column,
                t.x,
                t.y,
                t.pilot.map_or(-1, i32::from)
            ))
            .collect::<Vec<_>>()
            .join(", "),
        list(
            &KANA_PILOT_SYMBOLS
                .iter()
                .map(|&(glyph, light)| format!("[{glyph}, {}]", u8::from(light)))
                .collect::<Vec<_>>()
        )
    );
    let palette = Palette::default();
    let _ = writeln!(
        out,
        "  \"palette\": {{\"background\": {}, \"light\": {}, \"pink\": {}, \"backdrop\": {}, \"ink\": {}}},",
        list(&palette.background),
        list(&palette.light),
        list(&palette.pink),
        list(&palette.backdrop),
        list(&palette.ink)
    );
    // glyphs
    let strokes: Vec<String> = STROKES
        .iter()
        .map(|glyph| {
            format!(
                "[{}]",
                glyph
                    .iter()
                    .map(|stroke| format!(
                        "[{}]",
                        stroke
                            .iter()
                            .map(|&(x, y)| format!("[{x:?}, {y:?}]"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect();
    let _ = writeln!(
        out,
        "  \"glyphs\": {{\"grid\": {GLYPH_GRID:?}, \"stroke_width\": {STROKE_WIDTH:?}, \"chars\": \"{}\", \"strokes\": [\n    {}\n  ]}},",
        GLYPH_CHARS.iter().collect::<String>(),
        strokes.join(",\n    ")
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
    let mut whitening: Vec<String> = Lane::ALL
        .iter()
        .map(|lane| {
            let name = match lane {
                Lane::D => "D".to_owned(),
                Lane::Field(sector) => format!("F{sector}"),
            };
            format!("\"{name}\": \"{}\"", hex(&lane.whitening()))
        })
        .collect();
    whitening.extend((0..SECTORS).map(|s| {
        let sector = u8::try_from(s).expect("sector fits u8");
        format!("\"T{sector}\": \"{}\"", hex(&turn_whitening(sector)))
    }));
    whitening.extend(
        KanaLane::ALL
            .iter()
            .map(|lane| format!("\"{}\": \"{}\"", lane.name(), hex(&lane.whitening()))),
    );
    let _ = writeln!(out, "  \"whitening\": {{{}}},", whitening.join(", "));
    // reed-solomon
    let mut rs_cases = Vec::new();
    for (nsym, len, seed) in [
        (10usize, 16usize, 1u32),
        (22, 83, 2),
        (11, 19, 4),
        (2, 5, 5),
        (22, 68, 6),
        (33, 116, 7),
        (17, 20, 8),
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
    // turn lane ids
    let turn_cases: [(u16, u8); 5] = [(0, 0), (0, 3), (1, 2), (517, 1), (65535, 3)];
    let _ = writeln!(
        out,
        "  \"turn_first_ids\": {{\"frames\": {}, \"sectors\": {}, \"ids\": {}}},",
        list(&turn_cases.map(|c| c.0)),
        list(&turn_cases.map(|c| c.1)),
        list(&turn_cases.map(|(frame, sector)| turn_first_id(frame, sector)))
    );
    // kana lane ids
    let kana_frames = [0u16, 1, 517, 65535];
    let kana_ids: Vec<String> = kana_frames
        .iter()
        .map(|&frame| list(&KanaLane::ALL.map(|lane| kana_first_id(frame, lane))))
        .collect();
    let _ = writeln!(
        out,
        "  \"kana_first_ids\": {{\"frames\": {}, \"lanes\": [\"P0\", \"K0\", \"P1\", \"K1\"], \"ids\": [{}]}},",
        list(&kana_frames),
        kana_ids.join(", ")
    );
    // the kana reader's templates at a few grid points of every blur
    let mut kana_probes = Vec::new();
    for (blur, sigma) in KANA_BLURS.iter().enumerate() {
        for (glyph, light, i, j) in [
            (0u8, true, 0usize, 0usize),
            (1, true, 5, 6),
            (9, false, 3, 9),
            (14, false, 11, 2),
            (15, true, 7, 7),
            (6, false, 6, 1),
        ] {
            kana_probes.push(format!(
                "{{\"blur\": {sigma:?}, \"glyph\": {glyph}, \"light\": {}, \"i\": {i}, \"j\": {j}, \"value\": {:?}}}",
                u8::from(light),
                template_probe(blur, glyph, light, i, j)
            ));
        }
    }
    let _ = writeln!(
        out,
        "  \"kana_reader\": {{\"blurs\": {}, \"grid\": {KANA_GRID}, \"template_probes\": [\n    {}\n  ]}},",
        list(&KANA_BLURS.map(|b| format!("{b:?}"))),
        kana_probes.join(",\n    ")
    );
    // the turn reader's templates at a few points of every blur
    let mut probes = Vec::new();
    for (blur, sigma) in TURN_BLURS.iter().enumerate() {
        for (along, across) in [
            (0.0, 0.0),
            (5.5, 0.0),
            (6.1, 0.9),
            (-3.3, 2.2),
            (0.37, -4.11),
            (8.0, 8.0),
        ] {
            probes.push(format!(
                "{{\"blur\": {sigma:?}, \"along\": {along:?}, \"across\": {across:?}, \"value\": {:?}}}",
                turn_template(blur, along, across)
            ));
        }
    }
    let _ = writeln!(
        out,
        "  \"turn_reader\": {{\"blurs\": {}, \"template_probes\": [\n    {}\n  ]}},",
        list(&TURN_BLURS.map(|b| format!("{b:?}"))),
        probes.join(",\n    ")
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
            let turns: String = cells.turns.iter().map(|t| char::from(b'0' + t)).collect();
            let kana_data: Vec<Vec<u8>> = lanes.kana.iter().flatten().cloned().collect();
            let kana_words: Vec<Vec<u8>> = words.kana.iter().flatten().cloned().collect();
            let glyphs: String = cells
                .kana
                .iter()
                .map(|t| char::from_digit(u32::from(t.glyph), 16).expect("nibble"))
                .collect();
            frame_json.push(format!(
                "      {{\"frame\": {frame}, \"d_data\": \"{}\", \"field_data\": {}, \"turn_data\": {}, \"kana_data\": {}, \"d_word\": \"{}\", \"field_words\": {}, \"turn_words\": {}, \"kana_words\": {}, \"lit_dots\": {}, \"lit_petals_hex\": \"{}\", \"turns\": \"{turns}\", \"kana_glyphs\": \"{glyphs}\", \"kana_light_hex\": \"{}\"}}",
                hex(&lanes.d),
                quoted(&lanes.fields),
                quoted(&lanes.turns),
                quoted(&kana_data),
                hex(&words.d),
                quoted(&words.fields),
                quoted(&words.turns),
                quoted(&kana_words),
                list(&(0..cells.dots.len()).filter(|&i| cells.dots[i]).collect::<Vec<_>>()),
                hex(&bitmap(cells.petals.iter().copied())),
                hex(&tile_bitmap(cells.kana.iter().map(|t| t.light)))
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
