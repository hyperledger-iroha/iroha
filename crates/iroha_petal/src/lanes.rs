//! Lane codecs: bytes ⇄ transmitted cell states.
//!
//! A frame carries five lanes. Each lane is exactly one Reed–Solomon
//! codeword, XOR-whitened with a fixed pseudo-random sequence so the picture
//! is statistically balanced whatever the payload is:
//!
//! | lane | cells | codeword | data | parity |
//! |------|-------|----------|------|--------|
//! | `D` dots | 240 ring slots × 1 bit | 30 B | 19 B | 11 B |
//! | `F0`–`F3` petals | 840 field cells of one sector × 1 bit | 105 B | 83 B | 22 B |
//!
//! Bit order is most-significant-bit first.

use crate::layout::{
    D_BITS, FIELD_CELLS, SECTORS, SlotRole, TOTAL_SLOTS, data_slots, sector_cells, slot_roles,
};
use crate::prng::Xorshift32;
use crate::rs::{ReedSolomon, RsError};

/// Length of an atom in bytes.
pub const ATOM_LEN: usize = 16;
/// Bytes of the per-lane header (`tag`, `frame` high, `frame` low).
pub const LANE_HEADER_LEN: usize = 3;
/// Atoms carried by lane `D` on frames that do not carry a beacon.
pub const D_ATOMS: usize = 1;
/// Atoms carried by each field lane.
pub const FIELD_ATOMS: usize = 5;
/// Most atoms one frame can carry (the four field lanes and lane `D`).
pub const ATOMS_PER_FRAME: usize = SECTORS * FIELD_ATOMS + D_ATOMS;

/// Codeword length of lane `D` in bytes.
pub const D_WORD: usize = D_BITS / 8;
/// Parity bytes of lane `D`.
pub const D_PARITY: usize = 11;
/// Data bytes of lane `D`.
pub const D_DATA: usize = D_WORD - D_PARITY;
/// Codeword length of a field lane in bytes.
pub const FIELD_WORD: usize = 105;
/// Parity bytes of a field lane.
pub const FIELD_PARITY: usize = 22;
/// Data bytes of a field lane.
pub const FIELD_DATA: usize = FIELD_WORD - FIELD_PARITY;
/// Field cells of a sector that carry no bit (they stay dark).
pub const SPARE_SECTOR_CELLS: usize = FIELD_CELLS / SECTORS - FIELD_WORD * 8;

const _: () = {
    assert!(D_DATA == LANE_HEADER_LEN + D_ATOMS * ATOM_LEN);
    assert!(FIELD_DATA == LANE_HEADER_LEN + FIELD_ATOMS * ATOM_LEN);
    assert!(FIELD_WORD * 8 <= FIELD_CELLS / SECTORS);
};

/// One of the five data lanes of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    /// Dots on the three rings.
    D,
    /// The petals of field sector `0..4`.
    Field(u8),
}

impl Lane {
    /// All lanes in decode order.
    pub const ALL: [Self; 5] = [
        Self::D,
        Self::Field(0),
        Self::Field(1),
        Self::Field(2),
        Self::Field(3),
    ];

    /// The four field lanes.
    pub const FIELDS: [Self; SECTORS] = [
        Self::Field(0),
        Self::Field(1),
        Self::Field(2),
        Self::Field(3),
    ];

    /// Codeword length in bytes.
    #[must_use]
    pub const fn word_len(self) -> usize {
        match self {
            Self::D => D_WORD,
            Self::Field(_) => FIELD_WORD,
        }
    }

    /// Parity bytes.
    #[must_use]
    pub const fn parity_len(self) -> usize {
        match self {
            Self::D => D_PARITY,
            Self::Field(_) => FIELD_PARITY,
        }
    }

    /// Data bytes.
    #[must_use]
    pub const fn data_len(self) -> usize {
        self.word_len() - self.parity_len()
    }

    /// Atoms carried (lane `D`: on frames without a beacon).
    #[must_use]
    pub const fn atoms(self) -> usize {
        match self {
            Self::D => D_ATOMS,
            Self::Field(_) => FIELD_ATOMS,
        }
    }

    /// The letter that names the lane in reports: `D`, or the sector digit `0`–`3`.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::D => 'D',
            Self::Field(sector) => (b'0' + sector) as char,
        }
    }

    const fn whitening_seed(self) -> u32 {
        match self {
            Self::D => 0x444F_5453,                             // "DOTS"
            Self::Field(sector) => 0x5045_5430 + sector as u32, // "PET0" … "PET3"
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
    /// Lit state of every ring slot, gate dots included.
    pub dots: [bool; TOTAL_SLOTS],
    /// Lit state of every field cell (spare cells are dark).
    pub petals: [bool; FIELD_CELLS],
}

impl FrameCells {
    /// Builds the cells from the transmitted codewords of lane `D` and the four field lanes.
    ///
    /// # Panics
    /// Panics when a codeword has the wrong length.
    #[must_use]
    pub fn from_words(d: &[u8], fields: [&[u8]; SECTORS]) -> Self {
        assert_eq!(d.len(), D_WORD);
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
        let mut petals = [false; FIELD_CELLS];
        for (sector, word) in fields.into_iter().enumerate() {
            assert_eq!(word.len(), FIELD_WORD);
            for (bit, &cell) in sector_cells(sector).iter().take(FIELD_WORD * 8).enumerate() {
                petals[cell] = word[bit / 8] >> (7 - bit % 8) & 1 == 1;
            }
        }
        Self { dots, petals }
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

    /// Packs the petals of sector `sector` into its field codeword.
    #[must_use]
    pub fn field_word(&self, sector: usize) -> [u8; FIELD_WORD] {
        let mut word = [0u8; FIELD_WORD];
        for (bit, &cell) in sector_cells(sector).iter().take(FIELD_WORD * 8).enumerate() {
            if self.petals[cell] {
                word[bit / 8] |= 1 << (7 - bit % 8);
            }
        }
        word
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_data(seed: u8) -> Vec<u8> {
        (0..FIELD_DATA as u8)
            .map(|b| b.wrapping_mul(37) ^ seed)
            .collect()
    }

    #[test]
    fn lane_sizes_are_consistent() {
        assert_eq!(D_WORD, 30);
        assert_eq!((D_DATA, FIELD_DATA), (19, 83));
        assert_eq!(SPARE_SECTOR_CELLS, 4);
        assert_eq!(ATOMS_PER_FRAME, 21);
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
        assert_ne!(Lane::Field(0).whitening(), Lane::Field(1).whitening());
    }

    #[test]
    fn lanes_roundtrip_through_cells() {
        let d_data: Vec<u8> = (0..D_DATA as u8).map(|b| b ^ 0xA5).collect();
        let d = encode_lane(Lane::D, &d_data);
        let fields: Vec<Vec<u8>> = (0..SECTORS as u8)
            .map(|s| encode_lane(Lane::Field(s), &field_data(s)))
            .collect();
        let cells = FrameCells::from_words(&d, [&fields[0], &fields[1], &fields[2], &fields[3]]);
        assert_eq!(cells.d_word().as_slice(), d.as_slice());
        assert_eq!(decode_lane(Lane::D, &cells.d_word(), &[]).unwrap(), d_data);
        for (s, field) in fields.iter().enumerate() {
            assert_eq!(cells.field_word(s).as_slice(), field.as_slice());
            assert_eq!(
                decode_lane(Lane::Field(s as u8), &cells.field_word(s), &[]).unwrap(),
                field_data(s as u8)
            );
        }
        // the spare cells at the end of every sector stay dark
        for s in 0..SECTORS {
            for &cell in &sector_cells(s)[FIELD_WORD * 8..] {
                assert!(!cells.petals[cell]);
            }
        }
    }

    #[test]
    fn all_zero_data_still_lights_about_half_the_petals() {
        let zero = vec![0u8; FIELD_DATA];
        let fields: Vec<Vec<u8>> = (0..SECTORS as u8)
            .map(|s| encode_lane(Lane::Field(s), &zero))
            .collect();
        let cells = FrameCells::from_words(
            &encode_lane(Lane::D, &[0; D_DATA]),
            [&fields[0], &fields[1], &fields[2], &fields[3]],
        );
        let lit = cells.petals.iter().filter(|l| **l).count();
        assert!((1300..=2050).contains(&lit), "{lit} lit petals");
    }

    #[test]
    fn gate_dots_are_always_lit_and_guards_dark() {
        let field = [0xFFu8; FIELD_WORD];
        let cells = FrameCells::from_words(&[0xFF; D_WORD], [&field, &field, &field, &field]);
        for (slot, role) in slot_roles().into_iter().enumerate() {
            match role {
                SlotRole::Gate | SlotRole::Data(_) => assert!(cells.dots[slot]),
                SlotRole::Guard | SlotRole::Spare => assert!(!cells.dots[slot]),
            }
        }
    }

    #[test]
    fn counted_decoding_reports_the_rewritten_positions() {
        let data = field_data(9);
        let clean = encode_lane(Lane::Field(2), &data);
        assert_eq!(
            decode_lane_counted(Lane::Field(2), &clean, &[]).unwrap(),
            (data.clone(), 0)
        );
        let mut damaged = clean;
        for position in [0, 7, 19, 31, 60, 104] {
            damaged[position] ^= 0xC3;
        }
        assert_eq!(
            decode_lane_counted(Lane::Field(2), &damaged, &[]).unwrap(),
            (data, 6)
        );
    }

    #[test]
    fn a_word_decoded_under_another_sector_is_refused() {
        let word = encode_lane(Lane::Field(0), &field_data(1));
        for lane in [Lane::Field(1), Lane::Field(2), Lane::Field(3)] {
            assert!(decode_lane(lane, &word, &[]).is_err());
        }
    }
}
