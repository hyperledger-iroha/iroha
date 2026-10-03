//! Rateless fountain code over GF(2).
//!
//! A payload is cut into `k` source atoms of [`ATOM_LEN`] bytes (the last one
//! zero-padded). Encoded atom `id` is source atom `id` for `id < k`
//! (systematic), and otherwise the XOR of a pseudo-random half of the source
//! atoms chosen by [`mask_words`]. A receiver that holds any `k + 2` or so
//! independent atoms, in any order, recovers the payload by Gaussian
//! elimination; lost frames cost nothing but time.

use crate::lanes::ATOM_LEN;

/// One fountain atom.
pub type Atom = [u8; ATOM_LEN];

/// Splits a payload into zero-padded source atoms.
#[must_use]
pub fn split_payload(payload: &[u8]) -> Vec<Atom> {
    payload
        .chunks(ATOM_LEN)
        .map(|chunk| {
            let mut atom = [0u8; ATOM_LEN];
            atom[..chunk.len()].copy_from_slice(chunk);
            atom
        })
        .collect()
}

/// Number of 32-bit words needed for a mask over `k` source atoms.
#[must_use]
pub fn mask_len(k: usize) -> usize {
    k.div_ceil(32)
}

/// The 32-bit finalizer of `MurmurHash3` (`fmix32`).
///
/// Masks must not come from a GF(2)-linear generator such as xorshift: every
/// mask would then lie in a subspace of dimension at most 32 and repair atoms
/// could never raise the decoder rank past 32. The multiplications make this
/// mixer nonlinear over GF(2).
#[must_use]
pub fn mix32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x
}

/// The combination mask of encoded atom `id`, as little-endian bit words.
///
/// `crc` is the payload CRC-32C and only diversifies masks between streams.
/// `k` must be at least one. Atoms with `id < k` are systematic (a unit
/// vector); every other atom combines a pseudo-random half of the sources:
///
/// ```text
/// seed    = mix32((id * 0x9E3779B1) ^ crc ^ 0xA5A5A5A5)
/// word[w] = mix32(seed + (w + 1) * 0x9E3779B9)      (all arithmetic mod 2^32)
/// ```
///
/// Bits at or above `k` are cleared, and an all-zero mask is replaced by the
/// single bit `id mod k`.
#[must_use]
pub fn mask_words(k: usize, crc: u32, id: u32) -> Vec<u32> {
    let mut mask = vec![0u32; mask_len(k)];
    if (id as usize) < k {
        mask[id as usize / 32] = 1 << (id % 32);
        return mask;
    }
    let seed = mix32(id.wrapping_mul(0x9E37_79B1) ^ crc ^ 0xA5A5_A5A5);
    for (w, word) in mask.iter_mut().enumerate() {
        *word = mix32(seed.wrapping_add((w as u32 + 1).wrapping_mul(0x9E37_79B9)));
    }
    let tail = k % 32;
    if tail != 0 {
        let last = mask.len() - 1;
        mask[last] &= (1u32 << tail) - 1;
    }
    if mask.iter().all(|&w| w == 0) {
        let bit = id as usize % k;
        mask[bit / 32] |= 1 << (bit % 32);
    }
    mask
}

/// Encodes atom `id` from the source atoms.
#[must_use]
pub fn encode_atom(source: &[Atom], crc: u32, id: u32) -> Atom {
    let mask = mask_words(source.len(), crc, id);
    let mut out = [0u8; ATOM_LEN];
    for (index, atom) in source.iter().enumerate() {
        if mask[index / 32] >> (index % 32) & 1 == 1 {
            for (o, a) in out.iter_mut().zip(atom) {
                *o ^= a;
            }
        }
    }
    out
}

struct Row {
    mask: Vec<u32>,
    data: Atom,
}

/// Incremental Gaussian-elimination decoder.
pub struct FountainDecoder {
    k: usize,
    pivot: Vec<Option<usize>>,
    rows: Vec<Row>,
}

impl FountainDecoder {
    /// Creates a decoder for `k` source atoms.
    ///
    /// # Panics
    /// Panics when `k` is zero.
    #[must_use]
    pub fn new(k: usize) -> Self {
        assert!(k > 0, "a stream has at least one source atom");
        Self {
            k,
            pivot: vec![None; k],
            rows: Vec::new(),
        }
    }

    /// Number of source atoms.
    #[must_use]
    pub fn source_atoms(&self) -> usize {
        self.k
    }

    /// Number of linearly independent atoms received so far.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.rows.len()
    }

    /// Whether enough independent atoms arrived to recover the payload.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.rows.len() == self.k
    }

    /// Adds encoded atom `id`; returns whether it increased the rank.
    pub fn add_encoded(&mut self, crc: u32, id: u32, data: Atom) -> bool {
        self.add(mask_words(self.k, crc, id), data)
    }

    /// Adds a received combination; returns whether it increased the rank.
    pub fn add(&mut self, mut mask: Vec<u32>, mut data: Atom) -> bool {
        if mask.len() != mask_len(self.k) {
            return false;
        }
        let mut word = 0usize;
        loop {
            while word < mask.len() && mask[word] == 0 {
                word += 1;
            }
            if word == mask.len() {
                return false;
            }
            let column = word * 32 + mask[word].trailing_zeros() as usize;
            if column >= self.k {
                return false;
            }
            if let Some(row) = self.pivot[column] {
                let pivot = &self.rows[row];
                for (m, p) in mask[word..].iter_mut().zip(&pivot.mask[word..]) {
                    *m ^= p;
                }
                for (d, p) in data.iter_mut().zip(&pivot.data) {
                    *d ^= p;
                }
            } else {
                self.pivot[column] = Some(self.rows.len());
                self.rows.push(Row { mask, data });
                return true;
            }
        }
    }

    /// Returns the source atoms once the decoder is complete.
    #[must_use]
    pub fn solve(&self) -> Option<Vec<Atom>> {
        if !self.is_complete() {
            return None;
        }
        let mut solution: Vec<Atom> = vec![[0u8; ATOM_LEN]; self.k];
        for column in (0..self.k).rev() {
            let row = &self.rows[self.pivot[column]?];
            let mut value = row.data;
            let first_word = column / 32;
            for word in first_word..row.mask.len() {
                let mut bits = row.mask[word];
                if word == first_word {
                    // keep only columns strictly above the pivot
                    let shift = column % 32 + 1;
                    bits = if shift >= 32 {
                        0
                    } else {
                        bits >> shift << shift
                    };
                }
                while bits != 0 {
                    let bit = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let other = word * 32 + bit;
                    for (v, s) in value.iter_mut().zip(&solution[other]) {
                        *v ^= s;
                    }
                }
            }
            solution[column] = value;
        }
        Some(solution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(len: usize, seed: u32) -> Vec<u8> {
        let mut rng = crate::prng::Xorshift32::new(seed);
        (0..len).map(|_| rng.next_byte()).collect()
    }

    fn reassemble(atoms: &[Atom], len: usize) -> Vec<u8> {
        let mut bytes: Vec<u8> = atoms.iter().flatten().copied().collect();
        bytes.truncate(len);
        bytes
    }

    #[test]
    fn systematic_atoms_alone_recover_the_payload() {
        let data = payload(100, 5);
        let source = split_payload(&data);
        let mut decoder = FountainDecoder::new(source.len());
        for (id, atom) in source.iter().enumerate() {
            assert!(decoder.add_encoded(7, id as u32, *atom));
        }
        assert_eq!(reassemble(&decoder.solve().unwrap(), 100), data);
    }

    #[test]
    fn repair_atoms_cover_for_lost_systematic_atoms() {
        let data = payload(1000, 9);
        let source = split_payload(&data);
        let k = source.len();
        let crc = 0x1234_5678;
        let mut decoder = FountainDecoder::new(k);
        // lose every third systematic atom, then take repair atoms
        for id in (0..k as u32).filter(|id| id % 3 != 0) {
            decoder.add_encoded(crc, id, encode_atom(&source, crc, id));
        }
        let mut id = k as u32;
        let mut used = 0;
        while !decoder.is_complete() {
            decoder.add_encoded(crc, id, encode_atom(&source, crc, id));
            id += 1;
            used += 1;
            assert!(used < k, "decoder must converge");
        }
        let missing = (0..k).filter(|i| i % 3 == 0).count();
        assert!(
            used <= missing + 8,
            "needed {used} repairs for {missing} missing"
        );
        assert_eq!(reassemble(&decoder.solve().unwrap(), 1000), data);
    }

    #[test]
    fn pure_repair_streams_decode_with_small_overhead() {
        let data = payload(5000, 11);
        let source = split_payload(&data);
        let k = source.len();
        let crc = 0xCAFE_F00D;
        let mut total_overhead = 0usize;
        for trial in 0..20u32 {
            let mut decoder = FountainDecoder::new(k);
            let mut id = k as u32 + trial * 1000;
            let mut received = 0usize;
            while !decoder.is_complete() {
                decoder.add_encoded(crc, id, encode_atom(&source, crc, id));
                id += 1;
                received += 1;
            }
            total_overhead += received - k;
            assert_eq!(reassemble(&decoder.solve().unwrap(), 5000), data);
        }
        assert!(
            total_overhead <= 20 * 4,
            "average overhead {}",
            total_overhead as f64 / 20.0
        );
    }

    #[test]
    fn duplicate_and_dependent_atoms_do_not_raise_rank() {
        let source = split_payload(&payload(60, 3));
        let mut decoder = FountainDecoder::new(source.len());
        assert!(decoder.add_encoded(1, 0, source[0]));
        assert!(!decoder.add_encoded(1, 0, source[0]));
        assert_eq!(decoder.rank(), 1);
        assert!(decoder.solve().is_none());
    }

    #[test]
    fn repair_masks_span_far_more_than_thirty_two_dimensions() {
        // Regression: an xorshift-derived mask is GF(2)-linear in a 32-bit seed
        // and can never exceed rank 32.
        let k = 200;
        let mut decoder = FountainDecoder::new(k);
        for id in k as u32..(k as u32 + 400) {
            decoder.add(mask_words(k, 5, id), [0u8; ATOM_LEN]);
        }
        assert_eq!(decoder.rank(), k, "repair masks must reach full rank");
    }

    #[test]
    fn masks_are_nonzero_and_padded_bits_are_clear() {
        for k in [1usize, 2, 31, 32, 33, 100] {
            for id in 0..200u32 {
                let mask = mask_words(k, 99, id);
                assert!(mask.iter().any(|&w| w != 0));
                if k % 32 != 0 {
                    assert_eq!(mask[mask.len() - 1] >> (k % 32), 0);
                }
            }
        }
    }
}
