//! The kana layer: data in the katakana tiles of the bands above and below the square.
//!
//! The card ([`crate::layout::CARD_HEIGHT`]) is the square code with a band of katakana tiles
//! above and below it, in the part of an upright phone's camera frame that the square leaves
//! unused. Each band holds 306 tiles ([`kana_tiles`]); eight are pilots with fixed symbols
//! ([`KANA_PILOT_SYMBOLS`]) and the other 298 carry two lanes:
//!
//! | lane | cells | codeword | data | parity |
//! |------|-------|----------|------|--------|
//! | `P0`, `P1` polarity | 296 data tiles × 1 bit (light or dark) | 37 B | 20 B | 17 B |
//! | `K0`, `K1` glyph | 298 data tiles × 4 bits (one of sixteen katakana) | 149 B | 116 B | 33 B |
//!
//! A light tile is filled light with its glyph in ink; a dark tile is black with its glyph in
//! pink, as format 1's tiles were. Data tile `i` of a band (its tiles that are not pilots, in
//! reading order) shows bit `i` of the band's polarity codeword (most significant bit first; the
//! last two data tiles, beyond the codeword, are light) and nibble `i` of its glyph codeword (the
//! high nibble of byte `i / 2` for even `i`).
//!
//! Every kana lane is one Reed–Solomon codeword, XOR-whitened: fountain atoms from their own
//! repair range ([`KANA_ID_BASE`], see [`kana_first_id`]) and a CRC-32C check that binds them to
//! the stream tag, the frame and the lane ([`kana_check`]). The lanes have no header: a reader
//! takes the frame from the square's lanes and also tries the neighbouring frames, because a
//! rolling shutter or a stale frame can show a band from the frame before or after the square.
//!
//! [`read_kana`] reads the bands of a decoded capture. The square's pose, extended beyond the
//! square, places every tile; each band's pilots are found by a small search around their places,
//! and an affine correction fitted to their offsets moves every tile of the band. The pilots also
//! pick the blur of the glyph templates and the levels that tell light tiles from dark ones. Each
//! data tile then takes the polarity its mean level shows and the glyph whose template correlates
//! best with its samples.

use std::sync::OnceLock;

use crate::crc::crc32c;
use crate::decode::{DecodedFrame, LaneResult};
use crate::fountain::Atom;
use crate::glyphs::GLYPH_COUNT;
use crate::image::Luma;
use crate::lanes::ATOM_LEN;
use crate::layout::{
    CANVAS, KANA_BANDS, KANA_PILOT_SYMBOLS, KANA_PILOTS, KANA_TILE, kana_data_tiles, kana_inked,
    kana_inside, kana_pilot_tiles, kana_tiles,
};
use crate::prng::Xorshift32;
use crate::rs::{ReedSolomon, RsError};
use crate::stream::{AtomPacket, LaneHeader};
use crate::turns::TURN_ID_BASE;

/// Data tiles of each band.
pub const KANA_DATA_TILES: usize = 298;
/// Codeword length of a glyph lane in bytes (two data tiles per byte).
pub const KANA_GLYPH_WORD: usize = KANA_DATA_TILES / 2;
/// Parity bytes of a glyph lane.
pub const KANA_GLYPH_PARITY: usize = 33;
/// Atoms carried by each glyph lane.
pub const KANA_GLYPH_ATOMS: usize = 7;
/// Codeword length of a polarity lane in bytes (eight data tiles per byte).
pub const KANA_POLARITY_WORD: usize = KANA_DATA_TILES / 8;
/// Parity bytes of a polarity lane.
pub const KANA_POLARITY_PARITY: usize = 17;
/// Atoms carried by each polarity lane.
pub const KANA_POLARITY_ATOMS: usize = 1;
/// Bytes of the check that closes a kana lane's data.
pub const KANA_CHECK_LEN: usize = 4;
/// Fountain id of the first kana atom of frame 0, just above the turn lanes' range
/// (`2³¹ + 16 · 65536`).
pub const KANA_ID_BASE: u32 = TURN_ID_BASE + (1 << 20);
/// Kana atom ids reserved per frame (eight per band).
const KANA_IDS_PER_FRAME: u32 = 16;
/// Tiles of both bands.
pub const KANA_TILE_COUNT: usize = 612;

const _: () = {
    assert!(KANA_GLYPH_WORD - KANA_GLYPH_PARITY == KANA_GLYPH_ATOMS * ATOM_LEN + KANA_CHECK_LEN);
    assert!(
        KANA_POLARITY_WORD - KANA_POLARITY_PARITY
            == KANA_POLARITY_ATOMS * ATOM_LEN + KANA_CHECK_LEN
    );
    assert!(KANA_GLYPH_ATOMS + KANA_POLARITY_ATOMS <= KANA_IDS_PER_FRAME as usize / KANA_BANDS);
    assert!(KANA_TILE_COUNT == KANA_BANDS * (KANA_DATA_TILES + KANA_PILOTS));
};

/// What a kana lane carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KanaKind {
    /// Light or dark tiles.
    Polarity,
    /// The tiles' katakana.
    Glyph,
}

/// One of the four kana lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KanaLane {
    /// Band: 0 above the square, 1 below it.
    pub band: u8,
    /// What the lane carries.
    pub kind: KanaKind,
}

impl KanaLane {
    /// The four lanes: `P0`, `K0`, `P1`, `K1`.
    pub const ALL: [Self; 4] = [
        Self {
            band: 0,
            kind: KanaKind::Polarity,
        },
        Self {
            band: 0,
            kind: KanaKind::Glyph,
        },
        Self {
            band: 1,
            kind: KanaKind::Polarity,
        },
        Self {
            band: 1,
            kind: KanaKind::Glyph,
        },
    ];

    /// Codeword length in bytes.
    #[must_use]
    pub const fn word_len(self) -> usize {
        match self.kind {
            KanaKind::Polarity => KANA_POLARITY_WORD,
            KanaKind::Glyph => KANA_GLYPH_WORD,
        }
    }

    /// Parity bytes.
    #[must_use]
    pub const fn parity_len(self) -> usize {
        match self.kind {
            KanaKind::Polarity => KANA_POLARITY_PARITY,
            KanaKind::Glyph => KANA_GLYPH_PARITY,
        }
    }

    /// Data bytes: the atoms, then their check.
    #[must_use]
    pub const fn data_len(self) -> usize {
        self.word_len() - self.parity_len()
    }

    /// Atoms carried.
    #[must_use]
    pub const fn atoms(self) -> usize {
        match self.kind {
            KanaKind::Polarity => KANA_POLARITY_ATOMS,
            KanaKind::Glyph => KANA_GLYPH_ATOMS,
        }
    }

    /// The byte that names the lane in its check: `0x10 | band << 1 | kind` (polarity 0, glyph 1).
    #[must_use]
    pub const fn code(self) -> u8 {
        0x10 | self.band << 1
            | match self.kind {
                KanaKind::Polarity => 0,
                KanaKind::Glyph => 1,
            }
    }

    /// The lane's name in reports: `P0`, `K0`, `P1` or `K1`.
    #[must_use]
    pub fn name(self) -> String {
        let kind = match self.kind {
            KanaKind::Polarity => 'P',
            KanaKind::Glyph => 'K',
        };
        format!("{kind}{}", self.band)
    }

    /// Seed of the whitening sequence: "POL0", "POL1" for polarity, "KAN0", "KAN1" for glyphs.
    const fn whitening_seed(self) -> u32 {
        match self.kind {
            KanaKind::Polarity => 0x504F_4C30 + self.band as u32,
            KanaKind::Glyph => 0x4B41_4E30 + self.band as u32,
        }
    }

    /// The whitening sequence: one byte per codeword byte from a [`Xorshift32`].
    #[must_use]
    pub fn whitening(self) -> Vec<u8> {
        let mut rng = Xorshift32::new(self.whitening_seed());
        (0..self.word_len()).map(|_| rng.next_byte()).collect()
    }
}

/// Fountain id of the first atom `lane` carries in `frame`: `KANA_ID_BASE + 16 · frame +
/// 8 · band`, plus 7 for the polarity lane (the glyph lane's seven atoms come first).
#[must_use]
pub fn kana_first_id(frame: u16, lane: KanaLane) -> u32 {
    let slot = match lane.kind {
        KanaKind::Glyph => 0,
        KanaKind::Polarity => KANA_GLYPH_ATOMS as u32,
    };
    KANA_ID_BASE + u32::from(frame) * KANA_IDS_PER_FRAME + u32::from(lane.band) * 8 + slot
}

/// The check of a kana lane: CRC-32C over the stream tag, the frame (big-endian), the lane's
/// [`KanaLane::code`] and the atoms.
#[must_use]
pub fn kana_check(header: LaneHeader, lane: KanaLane, atoms: &[u8]) -> u32 {
    let mut bytes = Vec::with_capacity(4 + atoms.len());
    bytes.push(header.tag);
    bytes.extend_from_slice(&header.frame.to_be_bytes());
    bytes.push(lane.code());
    bytes.extend_from_slice(atoms);
    crc32c(&bytes)
}

/// The data of a kana lane: the atoms followed by their [`kana_check`] (big-endian).
///
/// # Panics
/// Panics when `atoms` is not the lane's atom count long.
#[must_use]
pub fn kana_data(header: LaneHeader, lane: KanaLane, atoms: &[u8]) -> Vec<u8> {
    assert_eq!(atoms.len(), lane.atoms() * ATOM_LEN, "kana atoms length");
    let mut data = atoms.to_vec();
    data.extend_from_slice(&kana_check(header, lane, atoms).to_be_bytes());
    data
}

/// The atoms of kana lane data whose check matches `header` and `lane`.
#[must_use]
pub fn kana_packet(header: LaneHeader, lane: KanaLane, data: &[u8]) -> Option<AtomPacket> {
    if data.len() != lane.data_len() {
        return None;
    }
    let (atoms, check) = data.split_at(lane.atoms() * ATOM_LEN);
    if kana_check(header, lane, atoms).to_be_bytes() != check {
        return None;
    }
    Some(AtomPacket {
        header,
        first_id: kana_first_id(header.frame, lane),
        atoms: atoms
            .chunks_exact(ATOM_LEN)
            .map(|chunk| {
                let mut atom: Atom = [0; ATOM_LEN];
                atom.copy_from_slice(chunk);
                atom
            })
            .collect(),
    })
}

/// Encodes kana lane data into the transmitted (whitened) codeword.
///
/// # Panics
/// Panics when `data` is not exactly [`KanaLane::data_len`] bytes.
#[must_use]
pub fn encode_kana_lane(lane: KanaLane, data: &[u8]) -> Vec<u8> {
    assert_eq!(data.len(), lane.data_len(), "kana data length mismatch");
    let mut word = ReedSolomon::new(lane.parity_len()).encode(data);
    for (byte, mask) in word.iter_mut().zip(lane.whitening()) {
        *byte ^= mask;
    }
    word
}

/// Decodes a transmitted kana codeword, returning its data and how many byte positions the
/// Reed–Solomon decoder rewrote. The check is not verified here (see [`kana_packet`]).
///
/// # Errors
/// Returns [`RsError`] when the word is uncorrectable.
pub fn decode_kana_lane_counted(
    lane: KanaLane,
    transmitted: &[u8],
    erasures: &[usize],
) -> Result<(Vec<u8>, usize), RsError> {
    if transmitted.len() != lane.word_len() {
        return Err(RsError::InvalidShape);
    }
    let mut word: Vec<u8> = transmitted
        .iter()
        .zip(lane.whitening())
        .map(|(byte, mask)| byte ^ mask)
        .collect();
    let corrected = ReedSolomon::new(lane.parity_len()).decode(&mut word, erasures)?;
    word.truncate(lane.data_len());
    Ok((word, corrected))
}

/// What one tile shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KanaSymbol {
    /// Glyph `0..16` ([`crate::glyphs::GLYPH_CHARS`]).
    pub glyph: u8,
    /// Light (filled) or dark.
    pub light: bool,
}

/// The symbols of every tile (pilots fixed, data tiles light with glyph 0).
#[must_use]
pub fn pilot_symbols() -> [KanaSymbol; KANA_TILE_COUNT] {
    let mut symbols = [KanaSymbol {
        glyph: 0,
        light: true,
    }; KANA_TILE_COUNT];
    for (index, tile) in kana_tiles().iter().enumerate() {
        if let Some(pilot) = tile.pilot {
            let (glyph, light) = KANA_PILOT_SYMBOLS[usize::from(pilot)];
            symbols[index] = KanaSymbol { glyph, light };
        }
    }
    symbols
}

/// Shows the transmitted codewords of `band` on its data tiles: `polarity` (37 bytes) and
/// `glyphs` (149 bytes).
///
/// # Panics
/// Panics when a codeword has the wrong length or `band` is not below [`KANA_BANDS`].
pub fn set_kana_symbols(
    symbols: &mut [KanaSymbol; KANA_TILE_COUNT],
    band: u8,
    polarity: &[u8],
    glyphs: &[u8],
) {
    assert_eq!(polarity.len(), KANA_POLARITY_WORD);
    assert_eq!(glyphs.len(), KANA_GLYPH_WORD);
    for (i, &tile) in kana_data_tiles(band).iter().enumerate() {
        let light = if i < KANA_POLARITY_WORD * 8 {
            polarity[i / 8] >> (7 - i % 8) & 1 == 1
        } else {
            true
        };
        let byte = glyphs[i / 2];
        let glyph = if i % 2 == 0 { byte >> 4 } else { byte & 0x0F };
        symbols[tile] = KanaSymbol { glyph, light };
    }
}

/// Packs the polarity codeword `band`'s data tiles show.
#[must_use]
pub fn polarity_word(
    symbols: &[KanaSymbol; KANA_TILE_COUNT],
    band: u8,
) -> [u8; KANA_POLARITY_WORD] {
    let mut word = [0u8; KANA_POLARITY_WORD];
    for (i, &tile) in kana_data_tiles(band)
        .iter()
        .enumerate()
        .take(KANA_POLARITY_WORD * 8)
    {
        if symbols[tile].light {
            word[i / 8] |= 1 << (7 - i % 8);
        }
    }
    word
}

/// Packs the glyph codeword `band`'s data tiles show.
#[must_use]
pub fn glyph_word(symbols: &[KanaSymbol; KANA_TILE_COUNT], band: u8) -> [u8; KANA_GLYPH_WORD] {
    let mut word = [0u8; KANA_GLYPH_WORD];
    for (i, &tile) in kana_data_tiles(band).iter().enumerate() {
        let nibble = symbols[tile].glyph & 0x0F;
        word[i / 2] |= if i % 2 == 0 { nibble << 4 } else { nibble };
    }
    word
}

// ---- reading ------------------------------------------------------------------------------------

/// Gaussian blurs (σ in canvas units) of the glyph templates; the pilots pick one per band.
pub const KANA_BLURS: [f64; 5] = [1.0, 1.6, 2.4, 3.4, 4.8];
/// The blur of the template the pilots are first searched with (3.4 units): coarse enough to
/// find a pilot on a 2-unit grid under any blur.
const COARSE_BLUR: usize = 3;
/// Step of the coarse pilot search in canvas units.
const COARSE_STEP: f64 = 2.0;
/// Samples per tile side: points `(i + ½) · 25 / 12` units from the tile's top-left corner.
pub const KANA_GRID: usize = 12;
/// Template raster step in canvas units.
const RASTER_STEP: f64 = 0.5;
/// The raster starts this far outside the tile square, so blur reaches in from the gutter.
const RASTER_MARGIN: f64 = 4.0;
/// Raster side in steps.
const RASTER_SIDE: usize = 67;
/// A pilot whose coarse correlation is below this is not followed further.
const PILOT_COARSE_MIN: f64 = 0.35;
/// A pilot whose final correlation is below this is not used.
const PILOT_MIN: f64 = 0.5;

const POINTS: usize = KANA_GRID * KANA_GRID;

/// Templates of one blur: for every glyph, the light tile (fill coverage) and the dark tile (ink
/// coverage) at the [`KANA_GRID`] points, mean-centred and scaled to unit length.
struct Templates {
    light: Vec<[f64; POINTS]>,
    dark: Vec<[f64; POINTS]>,
}

/// Coverage of `inside && !inked` (light) or `inked` (dark) on the template raster: raster point
/// `a` is at `−4 + a / 2` units; each value is the share of its 4 × 4 sub-points
/// `(m − 1.5) / 8` units around it that are covered.
fn raster(glyph: u8, light: bool) -> Vec<f64> {
    let mut table = vec![0.0; RASTER_SIDE * RASTER_SIDE];
    for b in 0..RASTER_SIDE {
        for a in 0..RASTER_SIDE {
            let (u0, v0) = (
                -RASTER_MARGIN + a as f64 * RASTER_STEP,
                -RASTER_MARGIN + b as f64 * RASTER_STEP,
            );
            let mut covered = 0u32;
            for n in 0..4 {
                for m in 0..4 {
                    let (u, v) = (
                        u0 + (f64::from(m) - 1.5) / 8.0,
                        v0 + (f64::from(n) - 1.5) / 8.0,
                    );
                    let inked = kana_inked(glyph, u, v);
                    let hit = if light {
                        kana_inside(u, v) && !inked
                    } else {
                        inked
                    };
                    covered += u32::from(hit);
                }
            }
            table[b * RASTER_SIDE + a] = f64::from(covered) / 16.0;
        }
    }
    table
}

/// Separable Gaussian blur of a raster: taps every [`RASTER_STEP`] out to `⌈3σ / step⌉`, weights
/// `exp(−d² / (2σ²))` normalised by their sum, points outside counting as zero; rows first.
fn blur_raster(table: &[f64], sigma: f64) -> Vec<f64> {
    let reach = (3.0 * sigma / RASTER_STEP).ceil() as i32;
    let weights: Vec<f64> = (-reach..=reach)
        .map(|k| {
            let d = f64::from(k) * RASTER_STEP;
            (-(d * d) / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f64 = weights.iter().sum();
    let weights: Vec<f64> = weights.iter().map(|w| w / sum).collect();
    let pass = |source: &[f64], along_rows: bool| -> Vec<f64> {
        let mut out = vec![0.0; RASTER_SIDE * RASTER_SIDE];
        for b in 0..RASTER_SIDE {
            for a in 0..RASTER_SIDE {
                let mut acc = 0.0;
                for (k, &w) in (-reach..=reach).zip(&weights) {
                    let (aa, bb) = if along_rows {
                        (a as i32 + k, b as i32)
                    } else {
                        (a as i32, b as i32 + k)
                    };
                    if aa < 0 || bb < 0 || aa >= RASTER_SIDE as i32 || bb >= RASTER_SIDE as i32 {
                        continue;
                    }
                    acc += w * source[bb as usize * RASTER_SIDE + aa as usize];
                }
                out[b * RASTER_SIDE + a] = acc;
            }
        }
        out
    };
    pass(&pass(table, true), false)
}

/// Tile-local coordinate of grid index `i` on either axis.
fn grid_point(i: usize) -> f64 {
    (i as f64 + 0.5) * f64::from(KANA_TILE) / KANA_GRID as f64
}

/// A blurred raster at the [`KANA_GRID`] points (bilinear), mean-centred and scaled to unit
/// length.
fn sample_raster(table: &[f64]) -> [f64; POINTS] {
    let mut out = [0.0; POINTS];
    for j in 0..KANA_GRID {
        for i in 0..KANA_GRID {
            let fa = (grid_point(i) + RASTER_MARGIN) / RASTER_STEP;
            let fb = (grid_point(j) + RASTER_MARGIN) / RASTER_STEP;
            let (a0, b0) = (fa.floor() as usize, fb.floor() as usize);
            let (ta, tb) = (fa - a0 as f64, fb - b0 as f64);
            let at = |a: usize, b: usize| table[b * RASTER_SIDE + a];
            let top = at(a0, b0) * (1.0 - ta) + at(a0 + 1, b0) * ta;
            let bottom = at(a0, b0 + 1) * (1.0 - ta) + at(a0 + 1, b0 + 1) * ta;
            out[j * KANA_GRID + i] = top * (1.0 - tb) + bottom * tb;
        }
    }
    let mean = out.iter().sum::<f64>() / POINTS as f64;
    let mut norm = 0.0;
    for value in &mut out {
        *value -= mean;
        norm += *value * *value;
    }
    let norm = norm.sqrt().max(1e-12);
    for value in &mut out {
        *value /= norm;
    }
    out
}

fn templates() -> &'static [Templates] {
    static TEMPLATES: OnceLock<Vec<Templates>> = OnceLock::new();
    TEMPLATES.get_or_init(|| {
        let rasters: Vec<(Vec<f64>, Vec<f64>)> = (0..GLYPH_COUNT as u8)
            .map(|g| (raster(g, true), raster(g, false)))
            .collect();
        KANA_BLURS
            .iter()
            .map(|&sigma| Templates {
                light: rasters
                    .iter()
                    .map(|(light, _)| sample_raster(&blur_raster(light, sigma)))
                    .collect(),
                dark: rasters
                    .iter()
                    .map(|(_, dark)| sample_raster(&blur_raster(dark, sigma)))
                    .collect(),
            })
            .collect()
    })
}

/// The template value of `glyph` (light or dark tile) under blur `blur` (an index into
/// [`KANA_BLURS`]) at grid point `(i, j)`, mean-centred and scaled to unit length: conformance
/// probes for ports.
#[must_use]
pub fn template_probe(blur: usize, glyph: u8, light: bool, i: usize, j: usize) -> f64 {
    let t = &templates()[blur];
    let table = if light { &t.light } else { &t.dark };
    table[usize::from(glyph)][j * KANA_GRID + i]
}

/// Luma at the grid points of the tile whose top-left corner is `(x, y)` (square coordinates),
/// moved by `(dx, dy)`, and their mean.
fn sample_tile(
    image: &Luma,
    frame: &DecodedFrame,
    x: f64,
    y: f64,
    shift: (f64, f64),
) -> ([f64; POINTS], f64) {
    let mut out = [0.0; POINTS];
    let mut sum = 0.0;
    for j in 0..KANA_GRID {
        for i in 0..KANA_GRID {
            let (px, py) = frame
                .pose
                .apply(x + grid_point(i) + shift.0, y + grid_point(j) + shift.1);
            let value = image.sample(px, py);
            out[j * KANA_GRID + i] = value;
            sum += value;
        }
    }
    (out, sum / POINTS as f64)
}

/// Correlation of samples (mean `mean`) with a unit, mean-centred template.
fn correlate(samples: &[f64; POINTS], mean: f64, template: &[f64; POINTS]) -> f64 {
    let (mut dot, mut energy) = (0.0, 0.0);
    for (x, t) in samples.iter().zip(template) {
        let centred = x - mean;
        dot += centred * t;
        energy += centred * centred;
    }
    if energy <= 1e-9 {
        0.0
    } else {
        dot / energy.sqrt()
    }
}

/// Whether every grid point of the tile at `(x, y)` maps inside the image.
fn tile_in_image(image: &Luma, frame: &DecodedFrame, x: f64, y: f64) -> bool {
    let side = f64::from(KANA_TILE);
    [(0.0, 0.0), (side, 0.0), (0.0, side), (side, side)]
        .iter()
        .all(|&(u, v)| {
            let (px, py) = frame.pose.apply(x + u, y + v);
            px >= 0.0 && py >= 0.0 && px < image.width as f64 && py < image.height as f64
        })
}

/// A pilot as found: its tile centre, the shift that fits it best, that correlation, its level
/// and its polarity.
#[derive(Debug, Clone, Copy)]
struct FoundPilot {
    centre: (f64, f64),
    shift: (f64, f64),
    level: f64,
    light: bool,
}

/// How far (canvas units) the pose can miss a pilot centred `y` units down the square (negative
/// above it): 4 units at the square's edge, growing by 10 over a band's 290-unit height, because a
/// lens's distortion grows away from the square the pose was fitted to.
fn pilot_reach(y: f64) -> f64 {
    let beyond = if y < 0.0 { -y } else { y - f64::from(CANVAS) };
    4.0 + 10.0 * (beyond.max(0.0) / 290.0).min(1.2)
}

/// The template a pilot shows under blur `blur`.
fn pilot_template(blur: usize, pilot: u8) -> &'static [f64; POINTS] {
    let (glyph, light) = KANA_PILOT_SYMBOLS[usize::from(pilot)];
    let t = &templates()[blur];
    if light {
        &t.light[usize::from(glyph)]
    } else {
        &t.dark[usize::from(glyph)]
    }
}

/// The correlation of a pilot tile's samples, moved by `shift`, with its template under `blur`.
fn pilot_score(
    image: &Luma,
    frame: &DecodedFrame,
    tile: usize,
    blur: usize,
    shift: (f64, f64),
) -> f64 {
    let t = kana_tiles()[tile];
    let Some(pilot) = t.pilot else {
        return f64::MIN;
    };
    let (samples, mean) = sample_tile(image, frame, t.x, t.y, shift);
    correlate(&samples, mean, pilot_template(blur, pilot))
}

/// Coarse search of a pilot: shifts on a [`COARSE_STEP`] grid within its [`pilot_reach`], under
/// the [`COARSE_BLUR`] template; returns the best shift and its correlation.
fn coarse_pilot(image: &Luma, frame: &DecodedFrame, tile: usize) -> ((f64, f64), f64) {
    let reach = pilot_reach(kana_tiles()[tile].centre().1);
    let steps = (reach / COARSE_STEP).floor() as i32;
    let mut best = ((0.0, 0.0), f64::MIN);
    for dy in -steps..=steps {
        for dx in -steps..=steps {
            let shift = (COARSE_STEP * f64::from(dx), COARSE_STEP * f64::from(dy));
            let score = pilot_score(image, frame, tile, COARSE_BLUR, shift);
            if score > best.1 {
                best = (shift, score);
            }
        }
    }
    best
}

/// Refines a pilot's shift under `blur`: the best of the shifts one step around it, for steps of
/// `steps` in turn (ties keep the earlier).
fn refine_pilot(
    image: &Luma,
    frame: &DecodedFrame,
    tile: usize,
    blur: usize,
    start: (f64, f64),
    steps: &[f64],
) -> ((f64, f64), f64) {
    let mut best = (start, pilot_score(image, frame, tile, blur, start));
    for &step in steps {
        let centre = best.0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let shift = (
                    centre.0 + step * f64::from(dx),
                    centre.1 + step * f64::from(dy),
                );
                let score = pilot_score(image, frame, tile, blur, shift);
                if score > best.1 {
                    best = (shift, score);
                }
            }
        }
    }
    best
}

/// Solves the 3 × 3 system `m · x = r` by Gauss–Jordan elimination with partial pivoting;
/// `None` when a pivot is below `1e-9`.
fn solve3(mut m: [[f64; 3]; 3], mut r: [f64; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let pivot = (col..3).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivot][col].abs() < 1e-9 {
            return None;
        }
        m.swap(col, pivot);
        r.swap(col, pivot);
        for row in 0..3 {
            if row == col {
                continue;
            }
            let factor = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (value, pivot_value) in m[row].iter_mut().zip(pivot_row).skip(col) {
                *value -= factor * pivot_value;
            }
            r[row] -= factor * r[col];
        }
    }
    Some([r[0] / m[0][0], r[1] / m[1][1], r[2] / m[2][2]])
}

/// The shift of every point of a band: an affine function of the tile centre fitted to the found
/// pilots' shifts by least squares (`x` scaled by 512 around the square's middle, `y` by 145
/// around the pilots' mean), or their mean shift when fewer than three pilots fit one.
struct ShiftField {
    x: [f64; 3],
    y: [f64; 3],
    y_centre: f64,
}

impl ShiftField {
    fn fit(pilots: &[FoundPilot]) -> Self {
        let n = pilots.len() as f64;
        let y_centre = pilots.iter().map(|p| p.centre.1).sum::<f64>() / n;
        let mean = |f: &dyn Fn(&FoundPilot) -> f64| pilots.iter().map(f).sum::<f64>() / n;
        let flat = Self {
            x: [mean(&|p| p.shift.0), 0.0, 0.0],
            y: [mean(&|p| p.shift.1), 0.0, 0.0],
            y_centre,
        };
        if pilots.len() < 3 {
            return flat;
        }
        let basis = |p: &FoundPilot| {
            [
                1.0,
                (p.centre.0 - f64::from(CANVAS) / 2.0) / 512.0,
                (p.centre.1 - y_centre) / 145.0,
            ]
        };
        let mut m = [[0.0; 3]; 3];
        let (mut rx, mut ry) = ([0.0; 3], [0.0; 3]);
        for p in pilots {
            let b = basis(p);
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] += b[i] * b[j];
                }
                rx[i] += b[i] * p.shift.0;
                ry[i] += b[i] * p.shift.1;
            }
        }
        match (solve3(m, rx), solve3(m, ry)) {
            (Some(x), Some(y)) => Self { x, y, y_centre },
            _ => flat,
        }
    }

    fn at(&self, centre: (f64, f64)) -> (f64, f64) {
        let b = [
            1.0,
            (centre.0 - f64::from(CANVAS) / 2.0) / 512.0,
            (centre.1 - self.y_centre) / 145.0,
        ];
        (
            self.x[0] * b[0] + self.x[1] * b[1] + self.x[2] * b[2],
            self.y[0] * b[0] + self.y[1] * b[1] + self.y[2] * b[2],
        )
    }
}

/// A level that varies linearly across the band, fitted by least squares to pilot levels against
/// `(x − 512) / 512`; a constant when the pilots do not spread.
fn level_line(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx < 1e-6 {
        return Some((my, 0.0));
    }
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let slope = sxy / sxx;
    Some((my - slope * mx, slope))
}

/// What a band showed one data tile as.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileReading {
    /// Index into [`kana_tiles`].
    pub tile: usize,
    /// Decided polarity.
    pub light: bool,
    /// How far the tile's mean level lay from the threshold, in half-contrasts.
    pub light_margin: f64,
    /// Decided glyph.
    pub glyph: u8,
    /// How far the glyph's correlation led the runner-up's.
    pub glyph_lead: f64,
}

/// What a reader saw of one band.
#[derive(Debug, Clone, PartialEq)]
pub struct BandReading {
    /// The blur (σ in canvas units) the pilots chose.
    pub blur: f64,
    /// Pilots found.
    pub pilots: usize,
    /// Every data tile of the band in data order, or `None` for tiles outside the image.
    pub tiles: Vec<Option<TileReading>>,
}

/// Reads one band's tiles; `None` when fewer than two pilots are found or the pilots show no
/// contrast between light and dark tiles.
#[must_use]
pub fn observe_band(image: &Luma, frame: &DecodedFrame, band: u8) -> Option<BandReading> {
    let tiles = kana_tiles();
    // coarse search of every pilot in view
    let coarse: Vec<(usize, (f64, f64))> = kana_pilot_tiles(band)
        .into_iter()
        .filter(|&tile| tile_in_image(image, frame, tiles[tile].x, tiles[tile].y))
        .filter_map(|tile| {
            let (shift, score) = coarse_pilot(image, frame, tile);
            (score >= PILOT_COARSE_MIN).then_some((tile, shift))
        })
        .collect();
    if coarse.len() < 2 {
        return None;
    }
    // the blur under which the pilots, each at its best shift within a unit, correlate best
    let mut blur = 0;
    let mut best = f64::MIN;
    for index in 0..KANA_BLURS.len() {
        let sum: f64 = coarse
            .iter()
            .map(|&(tile, shift)| refine_pilot(image, frame, tile, index, shift, &[1.0]).1)
            .sum();
        if sum > best {
            best = sum;
            blur = index;
        }
    }
    let mut found = Vec::new();
    for &(tile, shift) in &coarse {
        let (shift, score) = refine_pilot(image, frame, tile, blur, shift, &[1.0, 0.5, 0.25]);
        if score < PILOT_MIN {
            continue;
        }
        let t = tiles[tile];
        let (_, level) = sample_tile(image, frame, t.x, t.y, shift);
        let (_, light) = KANA_PILOT_SYMBOLS[usize::from(t.pilot?)];
        found.push((
            tile,
            FoundPilot {
                centre: t.centre(),
                shift,
                level,
                light,
            },
        ));
    }
    if found.len() < 2 {
        return None;
    }
    let pilots: Vec<FoundPilot> = found.iter().map(|(_, p)| *p).collect();
    let field = ShiftField::fit(&pilots);
    let scaled = |p: &FoundPilot| (p.centre.0 - f64::from(CANVAS) / 2.0) / 512.0;
    let light_line = level_line(
        &pilots
            .iter()
            .filter(|p| p.light)
            .map(|p| (scaled(p), p.level))
            .collect::<Vec<_>>(),
    );
    let dark_line = level_line(
        &pilots
            .iter()
            .filter(|p| !p.light)
            .map(|p| (scaled(p), p.level))
            .collect::<Vec<_>>(),
    );
    let (Some(light_line), Some(dark_line)) = (light_line, dark_line) else {
        return None;
    };
    let templates = &templates()[blur];
    let readings = kana_data_tiles(band)
        .iter()
        .map(|&tile| {
            let t = tiles[tile];
            if !tile_in_image(image, frame, t.x, t.y) {
                return None;
            }
            let centre = t.centre();
            let shift = field.at(centre);
            let (samples, mean) = sample_tile(image, frame, t.x, t.y, shift);
            let x = (centre.0 - f64::from(CANVAS) / 2.0) / 512.0;
            let high = light_line.0 + light_line.1 * x;
            let low = dark_line.0 + dark_line.1 * x;
            let half = (high - low) / 2.0;
            if half <= 1.0 {
                return None;
            }
            let threshold = high.midpoint(low);
            let mut scores = [0.0f64; GLYPH_COUNT];
            for (g, score) in scores.iter_mut().enumerate() {
                *score = correlate(&samples, mean, &templates.light[g]).max(correlate(
                    &samples,
                    mean,
                    &templates.dark[g],
                ));
            }
            let mut first = 0;
            for g in 1..GLYPH_COUNT {
                if scores[g] > scores[first] {
                    first = g;
                }
            }
            let second = (0..GLYPH_COUNT)
                .filter(|&g| g != first)
                .map(|g| scores[g])
                .fold(f64::MIN, f64::max);
            Some(TileReading {
                tile,
                light: mean > threshold,
                light_margin: (mean - threshold).abs() / half,
                glyph: first as u8,
                glyph_lead: scores[first] - second,
            })
        })
        .collect();
    Some(BandReading {
        blur: KANA_BLURS[blur],
        pilots: pilots.len(),
        tiles: readings,
    })
}

/// A kana lane read from a capture, with the header (stream tag and frame) its check matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanaResult {
    /// The lane data (atoms and check).
    pub result: LaneResult,
    /// The header the check matched: the square's frame or a neighbouring one.
    pub header: LaneHeader,
}

/// Tries Reed–Solomon with 0, n/8, n/4, n/3 and n/2 of the least confident bytes erased (n the
/// parity bytes; ties keep byte order) and accepts the first result whose check matches the
/// square's header or the frame before or after it.
fn decode_kana_word(
    lane: KanaLane,
    header: LaneHeader,
    word: &[u8],
    confidence: &[f64],
) -> Option<KanaResult> {
    let mut order: Vec<usize> = (0..word.len()).collect();
    order.sort_by(|&a, &b| confidence[a].total_cmp(&confidence[b]));
    let nsym = lane.parity_len();
    let mut schedule = vec![0, nsym / 8, nsym / 4, nsym / 3, nsym / 2];
    schedule.dedup();
    let frames = [
        header.frame,
        header.frame.wrapping_add(1),
        header.frame.wrapping_sub(1),
    ];
    for erasures in schedule {
        let positions: Vec<usize> = order.iter().copied().take(erasures).collect();
        let mut trial = word.to_vec();
        for &p in &positions {
            trial[p] = 0;
        }
        let Ok((data, corrected)) = decode_kana_lane_counted(lane, &trial, &positions) else {
            continue;
        };
        for frame in frames {
            let candidate = LaneHeader {
                tag: header.tag,
                frame,
            };
            if kana_packet(candidate, lane, &data).is_some() {
                return Some(KanaResult {
                    result: LaneResult {
                        data,
                        corrected,
                        erasures: positions.len(),
                    },
                    header: candidate,
                });
            }
        }
    }
    None
}

/// Reads the kana lanes of the bands not in `skip`, for a capture whose square `frame` decoded
/// under `header` (the stream tag and frame its lanes name). Returns `[[P0, K0], [P1, K1]]`.
#[must_use]
pub fn read_kana(
    image: &Luma,
    frame: &DecodedFrame,
    header: LaneHeader,
    skip: [bool; KANA_BANDS],
) -> [[Option<KanaResult>; 2]; KANA_BANDS] {
    let mut results: [[Option<KanaResult>; 2]; KANA_BANDS] = Default::default();
    for band in 0..KANA_BANDS as u8 {
        if skip[usize::from(band)] {
            continue;
        }
        let Some(reading) = observe_band(image, frame, band) else {
            continue;
        };
        let mut polarity = vec![0u8; KANA_POLARITY_WORD];
        let mut polarity_confidence = vec![f64::MAX; KANA_POLARITY_WORD];
        let mut glyphs = vec![0u8; KANA_GLYPH_WORD];
        let mut glyph_confidence = vec![f64::MAX; KANA_GLYPH_WORD];
        for (i, tile) in reading.tiles.iter().enumerate() {
            let (light, light_margin, glyph, lead) = tile.map_or((false, 0.0, 0, 0.0), |t| {
                (t.light, t.light_margin, t.glyph, t.glyph_lead)
            });
            if i < KANA_POLARITY_WORD * 8 {
                if light {
                    polarity[i / 8] |= 1 << (7 - i % 8);
                }
                let c = &mut polarity_confidence[i / 8];
                *c = c.min(light_margin);
            }
            glyphs[i / 2] |= if i % 2 == 0 { glyph << 4 } else { glyph };
            let c = &mut glyph_confidence[i / 2];
            *c = c.min(lead);
        }
        let lanes = [
            KanaLane {
                band,
                kind: KanaKind::Polarity,
            },
            KanaLane {
                band,
                kind: KanaKind::Glyph,
            },
        ];
        results[usize::from(band)] = [
            decode_kana_word(lanes[0], header, &polarity, &polarity_confidence),
            decode_kana_word(lanes[1], header, &glyphs, &glyph_confidence),
        ];
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_sizes_and_ids_are_consistent() {
        assert_eq!(KANA_GLYPH_WORD, 149);
        assert_eq!(KANA_POLARITY_WORD, 37);
        assert_eq!(kana_tiles().len(), KANA_TILE_COUNT);
        assert_eq!(kana_data_tiles(0).len(), KANA_DATA_TILES);
        let mut ids = std::collections::HashSet::new();
        for frame in [0u16, 1, 65_535] {
            for lane in KanaLane::ALL {
                for k in 0..lane.atoms() as u32 {
                    assert!(ids.insert(kana_first_id(frame, lane) + k));
                }
            }
        }
        // above every turn atom id and below the u32 limit
        assert!(kana_first_id(0, KanaLane::ALL[1]) >= crate::turns::turn_first_id(65_535, 3) + 4);
        assert!(kana_first_id(65_535, KanaLane::ALL[2]) < u32::MAX - 16);
    }

    #[test]
    fn whitening_is_balanced_and_distinct_per_lane() {
        let all: Vec<Vec<u8>> = KanaLane::ALL.iter().map(|l| l.whitening()).collect();
        for (lane, w) in KanaLane::ALL.iter().zip(&all) {
            let ones: u32 = w.iter().map(|b| b.count_ones()).sum();
            let bits = (w.len() * 8) as u32;
            assert!(ones > bits * 35 / 100 && ones < bits * 65 / 100, "{lane:?}");
        }
        assert_ne!(all[1][..37], all[0][..]);
        assert_ne!(all[1], all[3]);
    }

    #[test]
    fn symbols_roundtrip_through_codewords() {
        let header = LaneHeader {
            tag: 0x5A,
            frame: 77,
        };
        let mut symbols = pilot_symbols();
        let mut words = Vec::new();
        for band in 0..KANA_BANDS as u8 {
            let polarity_lane = KanaLane {
                band,
                kind: KanaKind::Polarity,
            };
            let glyph_lane = KanaLane {
                band,
                kind: KanaKind::Glyph,
            };
            let p_atoms: Vec<u8> = (0..16).map(|i| i as u8 ^ band).collect();
            let g_atoms: Vec<u8> = (0..112).map(|i| (i as u8).wrapping_mul(7) ^ band).collect();
            let p = encode_kana_lane(polarity_lane, &kana_data(header, polarity_lane, &p_atoms));
            let g = encode_kana_lane(glyph_lane, &kana_data(header, glyph_lane, &g_atoms));
            set_kana_symbols(&mut symbols, band, &p, &g);
            words.push((p, g, p_atoms, g_atoms));
        }
        for (band, (p, g, p_atoms, g_atoms)) in words.iter().enumerate() {
            let band = band as u8;
            assert_eq!(polarity_word(&symbols, band).as_slice(), p.as_slice());
            assert_eq!(glyph_word(&symbols, band).as_slice(), g.as_slice());
            let lane = KanaLane {
                band,
                kind: KanaKind::Glyph,
            };
            let (data, _) = decode_kana_lane_counted(lane, g, &[]).unwrap();
            let packet = kana_packet(header, lane, &data).expect("check matches");
            assert_eq!(packet.first_id, kana_first_id(77, lane));
            assert_eq!(packet.atoms.concat(), *g_atoms);
            // the check refuses another frame, band or kind
            let other = LaneHeader {
                tag: 0x5A,
                frame: 78,
            };
            assert!(kana_packet(other, lane, &data).is_none());
            let polarity = KanaLane {
                band,
                kind: KanaKind::Polarity,
            };
            let (data, _) = decode_kana_lane_counted(polarity, p, &[]).unwrap();
            assert_eq!(
                kana_packet(header, polarity, &data).unwrap().atoms.concat(),
                *p_atoms
            );
        }
        // pilots keep their symbols
        for (index, tile) in kana_tiles().iter().enumerate() {
            if let Some(p) = tile.pilot {
                let (glyph, light) = KANA_PILOT_SYMBOLS[usize::from(p)];
                assert_eq!(symbols[index], KanaSymbol { glyph, light });
            }
        }
    }

    #[test]
    fn templates_are_unit_and_tell_glyphs_apart() {
        for blur in 0..KANA_BLURS.len() {
            for g in 0..GLYPH_COUNT as u8 {
                for light in [true, false] {
                    let norm: f64 = (0..POINTS)
                        .map(|p| {
                            template_probe(blur, g, light, p % KANA_GRID, p / KANA_GRID).powi(2)
                        })
                        .sum();
                    assert!((norm - 1.0).abs() < 1e-9);
                }
            }
        }
        // at the sharpest blur no two light templates correlate above 0.8
        let t = &templates()[0];
        for a in 0..GLYPH_COUNT {
            for b in (a + 1)..GLYPH_COUNT {
                let dot: f64 = t.light[a].iter().zip(&t.light[b]).map(|(x, y)| x * y).sum();
                assert!(dot < 0.8, "glyphs {a} and {b}: {dot}");
            }
        }
    }
}
