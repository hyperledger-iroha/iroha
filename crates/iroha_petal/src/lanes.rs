//! Lane codecs: bytes ⇄ transmitted cell states.
//!
//! A frame carries three lanes. Each lane is exactly one Reed–Solomon
//! codeword, XOR-whitened with a fixed pseudo-random sequence so the picture
//! is statistically balanced whatever the payload is:
//!
//! | lane | cells | codeword | data | parity |
//! |------|-------|----------|------|--------|
//! | `P` polarity | 256 tiles × 1 bit | 32 B | 19 B | 13 B |
//! | `K` katakana | 256 tiles × 4 bits | 128 B | 83 B | 45 B |
//! | `D` dots | 240 ring slots × 1 bit | 30 B | 19 B | 11 B |
//!
//! Bit order is most-significant-bit first; lane `K` packs the first tile of a
//! pair into the high nibble.

use crate::layout::{D_BITS, SlotRole, TILE_COUNT, TOTAL_SLOTS, data_slots, slot_roles};
use crate::prng::Xorshift32;
use crate::rs::{ReedSolomon, RsError};

/// Length of an atom in bytes.
pub const ATOM_LEN: usize = 16;
/// Bytes of the per-lane header (`tag`, `frame` high, `frame` low).
pub const LANE_HEADER_LEN: usize = 3;
/// Atoms carried by lane `P`.
pub const P_ATOMS: usize = 1;
/// Atoms carried by lane `D` on frames that do not carry a beacon.
pub const D_ATOMS: usize = 1;
/// Atoms carried by lane `K`.
pub const K_ATOMS: usize = 5;
/// Most atoms one frame can carry (lanes `P`, `D` and `K`).
pub const ATOMS_PER_FRAME: usize = P_ATOMS + D_ATOMS + K_ATOMS;

/// Codeword length of lane `P` in bytes.
pub const P_WORD: usize = TILE_COUNT / 8;
/// Codeword length of lane `K` in bytes.
pub const K_WORD: usize = TILE_COUNT / 2;
/// Codeword length of lane `D` in bytes.
pub const D_WORD: usize = D_BITS / 8;
/// Parity bytes of lane `P`.
pub const P_PARITY: usize = 13;
/// Parity bytes of lane `K`.
pub const K_PARITY: usize = 45;
/// Parity bytes of lane `D`.
pub const D_PARITY: usize = 11;
/// Data bytes of lane `P`.
pub const P_DATA: usize = P_WORD - P_PARITY;
/// Data bytes of lane `K`.
pub const K_DATA: usize = K_WORD - K_PARITY;
/// Data bytes of lane `D`.
pub const D_DATA: usize = D_WORD - D_PARITY;

const _: () = {
    assert!(P_DATA == LANE_HEADER_LEN + P_ATOMS * ATOM_LEN);
    assert!(K_DATA == LANE_HEADER_LEN + K_ATOMS * ATOM_LEN);
    assert!(D_DATA == LANE_HEADER_LEN + D_ATOMS * ATOM_LEN);
};

/// One of the three data lanes of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    /// Light/dark polarity of the tiles.
    P,
    /// Katakana glyph of each tile.
    K,
    /// Dots on the three rings.
    D,
}

impl Lane {
    /// All lanes in decode order.
    pub const ALL: [Self; 3] = [Self::P, Self::D, Self::K];

    /// Codeword length in bytes.
    #[must_use]
    pub const fn word_len(self) -> usize {
        match self {
            Self::P => P_WORD,
            Self::K => K_WORD,
            Self::D => D_WORD,
        }
    }

    /// Parity bytes.
    #[must_use]
    pub const fn parity_len(self) -> usize {
        match self {
            Self::P => P_PARITY,
            Self::K => K_PARITY,
            Self::D => D_PARITY,
        }
    }

    /// Data bytes.
    #[must_use]
    pub const fn data_len(self) -> usize {
        self.word_len() - self.parity_len()
    }

    const fn whitening_seed(self) -> u32 {
        match self {
            Self::P => 0x5045_5441, // "PETA"
            Self::K => 0x4B41_4E41, // "KANA"
            Self::D => 0x444F_5453, // "DOTS"
        }
    }

    /// The fixed whitening sequence of the lane.
    #[must_use]
    pub fn whitening(self) -> Vec<u8> {
        let mut rng = Xorshift32::new(self.whitening_seed());
        (0..self.word_len()).map(|_| rng.next_byte()).collect()
    }
}

/// Encodes lane data into the transmitted (whitened) codeword.
///
/// # Panics
/// Panics when `data` is not exactly [`Lane::data_len`] bytes.
#[must_use]
pub fn encode_lane(lane: Lane, data: &[u8]) -> Vec<u8> {
    assert_eq!(data.len(), lane.data_len(), "lane data length mismatch");
    let mut word = ReedSolomon::new(lane.parity_len()).encode(data);
    for (byte, mask) in word.iter_mut().zip(lane.whitening()) {
        *byte ^= mask;
    }
    word
}

/// Decodes a transmitted codeword, returning the lane data bytes.
///
/// `erasures` lists byte positions the caller distrusts.
///
/// # Errors
/// Returns [`RsError`] when the word is uncorrectable.
pub fn decode_lane(lane: Lane, transmitted: &[u8], erasures: &[usize]) -> Result<Vec<u8>, RsError> {
    decode_lane_counted(lane, transmitted, erasures).map(|(data, _)| data)
}

/// Like [`decode_lane`], also returning how many byte positions the
/// Reed–Solomon decoder rewrote (erased bytes plus unflagged errors).
///
/// # Errors
/// Returns [`RsError`] when the word is uncorrectable.
pub fn decode_lane_counted(
    lane: Lane,
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

/// Every cell of one frame: what a renderer draws and a decoder samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameCells {
    /// Polarity of each tile; `true` is a light tile.
    pub light: [bool; TILE_COUNT],
    /// Glyph symbol (`0..16`) of each tile.
    pub glyph: [u8; TILE_COUNT],
    /// Lit state of every ring slot, gate dots included.
    pub dots: [bool; TOTAL_SLOTS],
}

impl FrameCells {
    /// Builds the cells from the three transmitted codewords.
    ///
    /// # Panics
    /// Panics when a codeword has the wrong length.
    #[must_use]
    pub fn from_words(p: &[u8], k: &[u8], d: &[u8]) -> Self {
        assert_eq!(p.len(), P_WORD);
        assert_eq!(k.len(), K_WORD);
        assert_eq!(d.len(), D_WORD);
        let mut light = [false; TILE_COUNT];
        let mut glyph = [0u8; TILE_COUNT];
        for tile in 0..TILE_COUNT {
            light[tile] = p[tile / 8] >> (7 - tile % 8) & 1 == 1;
            let byte = k[tile / 2];
            glyph[tile] = if tile % 2 == 0 {
                byte >> 4
            } else {
                byte & 0x0F
            };
        }
        let mut dots = [false; TOTAL_SLOTS];
        for (slot, role) in slot_roles().into_iter().enumerate() {
            dots[slot] = match role {
                SlotRole::Gate => true,
                SlotRole::Data(bit) => {
                    d[usize::from(bit) / 8] >> (7 - usize::from(bit) % 8) & 1 == 1
                }
                SlotRole::Guard | SlotRole::Spare => false,
            };
        }
        Self { light, glyph, dots }
    }

    /// Packs the polarity cells into a lane `P` codeword.
    #[must_use]
    pub fn p_word(&self) -> [u8; P_WORD] {
        let mut word = [0u8; P_WORD];
        for (tile, &light) in self.light.iter().enumerate() {
            if light {
                word[tile / 8] |= 1 << (7 - tile % 8);
            }
        }
        word
    }

    /// Packs the glyph cells into a lane `K` codeword.
    #[must_use]
    pub fn k_word(&self) -> [u8; K_WORD] {
        let mut word = [0u8; K_WORD];
        for (tile, &glyph) in self.glyph.iter().enumerate() {
            let nibble = glyph & 0x0F;
            word[tile / 2] |= if tile % 2 == 0 { nibble << 4 } else { nibble };
        }
        word
    }

    /// Packs the data dots into a lane `D` codeword.
    #[must_use]
    pub fn d_word(&self) -> [u8; D_WORD] {
        let mut word = [0u8; D_WORD];
        for (bit, slot) in data_slots().into_iter().enumerate() {
            if self.dots[slot] {
                word[bit / 8] |= 1 << (7 - bit % 8);
            }
        }
        word
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_sizes_are_consistent() {
        assert_eq!(P_WORD, 32);
        assert_eq!(K_WORD, 128);
        assert_eq!(D_WORD, 30);
        assert_eq!((P_DATA, K_DATA, D_DATA), (19, 83, 19));
    }

    #[test]
    fn whitening_is_deterministic_and_balanced() {
        for lane in Lane::ALL {
            let a = lane.whitening();
            assert_eq!(a, lane.whitening());
            let ones: u32 = a.iter().map(|b| b.count_ones()).sum();
            let bits = (a.len() * 8) as u32;
            assert!(
                ones > bits * 38 / 100 && ones < bits * 62 / 100,
                "{lane:?} ones {ones}/{bits}"
            );
        }
    }

    #[test]
    fn lanes_roundtrip_through_cells() {
        let p_data: Vec<u8> = (0..P_DATA as u8).collect();
        let k_data: Vec<u8> = (0..K_DATA as u8).map(|b| b.wrapping_mul(37)).collect();
        let d_data: Vec<u8> = (0..D_DATA as u8).map(|b| b ^ 0xA5).collect();
        let (p, k, d) = (
            encode_lane(Lane::P, &p_data),
            encode_lane(Lane::K, &k_data),
            encode_lane(Lane::D, &d_data),
        );
        let cells = FrameCells::from_words(&p, &k, &d);
        assert_eq!(cells.p_word().as_slice(), p.as_slice());
        assert_eq!(cells.k_word().as_slice(), k.as_slice());
        assert_eq!(cells.d_word().as_slice(), d.as_slice());
        assert_eq!(decode_lane(Lane::P, &cells.p_word(), &[]).unwrap(), p_data);
        assert_eq!(decode_lane(Lane::K, &cells.k_word(), &[]).unwrap(), k_data);
        assert_eq!(decode_lane(Lane::D, &cells.d_word(), &[]).unwrap(), d_data);
    }

    #[test]
    fn all_zero_data_still_lights_roughly_half_the_cells() {
        let cells = FrameCells::from_words(
            &encode_lane(Lane::P, &[0; P_DATA]),
            &encode_lane(Lane::K, &[0; K_DATA]),
            &encode_lane(Lane::D, &[0; D_DATA]),
        );
        let lit = cells.light.iter().filter(|l| **l).count();
        assert!((90..=166).contains(&lit), "{lit} light tiles");
    }

    #[test]
    fn gate_dots_are_always_lit_and_guards_dark() {
        let cells = FrameCells::from_words(&[0xFF; P_WORD], &[0xFF; K_WORD], &[0xFF; D_WORD]);
        for (slot, role) in slot_roles().into_iter().enumerate() {
            match role {
                SlotRole::Gate | SlotRole::Data(_) => assert!(cells.dots[slot]),
                SlotRole::Guard | SlotRole::Spare => assert!(!cells.dots[slot]),
            }
        }
    }

    #[test]
    fn counted_decoding_reports_the_rewritten_positions() {
        let data: Vec<u8> = (0..P_DATA as u8).collect();
        let clean = encode_lane(Lane::P, &data);
        assert_eq!(
            decode_lane_counted(Lane::P, &clean, &[]).unwrap(),
            (data.clone(), 0)
        );
        let mut damaged = clean;
        for position in [0, 7, 19, 31] {
            damaged[position] ^= 0xC3;
        }
        assert_eq!(
            decode_lane_counted(Lane::P, &damaged, &[]).unwrap(),
            (data, 4)
        );
    }

    #[test]
    fn decode_survives_burst_damage_to_a_lane() {
        let k_data: Vec<u8> = (0..K_DATA as u8).collect();
        let mut word = encode_lane(Lane::K, &k_data);
        for byte in word.iter_mut().take(K_PARITY / 2) {
            *byte ^= 0x5A;
        }
        assert_eq!(decode_lane(Lane::K, &word, &[]).unwrap(), k_data);
    }
}
