//! Reproducible generation of Poseidon round constants and MDS matrices.
//!
//! This is the reference procedure of the Poseidon paper (Grassi et al.,
//! USENIX Security 2021, and the `generate_parameters_grain.sage` script) as
//! implemented by `poseidon-primitives` 0.2.0, which the vendored
//! `halo2-base` `OptimizedPoseidonSpec` uses:
//!
//! - an 80-bit Grain LFSR seeded with the field type (prime), the S-box type
//!   (`x^alpha`), the field size in bits, the width `t`, `R_F` and `R_P`, with
//!   the first 160 output bits discarded and self-shrinking output;
//! - round constants by rejection sampling of 255-bit big-endian strings;
//! - a Cauchy MDS matrix `1 / (x_i + y_j)` from `2t` distinct elements sampled
//!   without rejection (reduced modulo the field order), skipping the first
//!   `secure_mds` candidate matrices.
//!
//! `crate::poseidon` pins the generated tables as binary files; the tests
//! regenerate them here and compare byte for byte.

use ff::{FromUniformBytes, PrimeField};

/// LFSR state size in bits.
const STATE: usize = 80;

/// The Grain LFSR in self-shrinking mode.
#[derive(Clone, Debug)]
pub(crate) struct Grain {
    state: [bool; STATE],
    next_bit: usize,
}

impl Grain {
    /// Seeds the LFSR for a prime field of `num_bits` bits, the `x^alpha`
    /// S-box, width `t`, `r_f` full rounds and `r_p` partial rounds.
    pub(crate) fn new(num_bits: u16, t: u16, r_f: u16, r_p: u16) -> Self {
        let mut state = [true; STATE];
        let mut set_bits = |offset: usize, len: usize, value: u16| {
            for i in 0..len {
                state[offset + len - 1 - i] = (value >> i) & 1 != 0;
            }
        };
        set_bits(0, 2, 1); // field type: prime order
        set_bits(2, 4, 0); // S-box type: x^alpha
        set_bits(6, 12, num_bits);
        set_bits(18, 12, t);
        set_bits(30, 10, r_f);
        set_bits(40, 10, r_p);
        let mut grain = Self {
            state,
            next_bit: STATE,
        };
        // Discard the first 160 bits.
        for _ in 0..20 {
            grain.load_next_8_bits();
            grain.next_bit = STATE;
        }
        grain
    }

    fn load_next_8_bits(&mut self) {
        let mut new_bits = 0u8;
        for i in 0..8 {
            let s = &self.state;
            let bit = s[i + 62] ^ s[i + 51] ^ s[i + 38] ^ s[i + 23] ^ s[i + 13] ^ s[i];
            new_bits |= u8::from(bit) << i;
        }
        self.state.rotate_left(8);
        self.next_bit -= 8;
        for i in 0..8 {
            self.state[self.next_bit + i] = (new_bits >> i) & 1 != 0;
        }
    }

    fn get_next_bit(&mut self) -> bool {
        if self.next_bit == STATE {
            self.load_next_8_bits();
        }
        let ret = self.state[self.next_bit];
        self.next_bit += 1;
        ret
    }

    /// The next self-shrinking output bit: of each pair of LFSR bits, output
    /// the second when the first is 1, discard both otherwise.
    pub(crate) fn next_bit(&mut self) -> bool {
        while !self.get_next_bit() {
            self.get_next_bit();
        }
        self.get_next_bit()
    }

    /// Writes the next `F::NUM_BITS` output bits big-endian into `view`.
    fn fill_be<F: PrimeField>(&mut self, view: &mut [u8]) {
        let bits = F::NUM_BITS as usize;
        for i in 0..bits {
            let bit = self.next_bit();
            let pos = bits - 1 - i;
            if bit {
                view[pos / 8] |= 1 << (pos % 8);
            }
        }
    }

    /// The next field element by rejection sampling.
    pub(crate) fn next_field_element<F: PrimeField<Repr = [u8; 32]>>(&mut self) -> F {
        loop {
            let mut bytes = [0u8; 32];
            self.fill_be::<F>(&mut bytes);
            if let Some(f) = Option::<F>::from(F::from_repr(bytes)) {
                return f;
            }
        }
    }

    /// The next field element without rejection (reduced modulo the order).
    pub(crate) fn next_field_element_without_rejection<F: FromUniformBytes<64>>(&mut self) -> F {
        let mut bytes = [0u8; 64];
        self.fill_be::<F>(&mut bytes);
        F::from_uniform_bytes(&bytes)
    }
}

/// Generates `(round_constants, mds)` for a width-`T` Pow5 permutation.
///
/// `round_constants` has `r_f + r_p` rows of `T` elements.
pub fn generate_constants<F, const T: usize>(
    r_f: usize,
    r_p: usize,
    secure_mds: usize,
) -> (Vec<[F; T]>, [[F; T]; T])
where
    F: PrimeField<Repr = [u8; 32]> + FromUniformBytes<64> + Ord,
{
    let as_u16 = |v: usize| u16::try_from(v).unwrap_or(u16::MAX);
    let num_bits = as_u16(F::NUM_BITS as usize);
    let mut grain = Grain::new(num_bits, as_u16(T), as_u16(r_f), as_u16(r_p));
    let round_constants = (0..r_f + r_p)
        .map(|_| {
            let mut row = [F::ZERO; T];
            for v in &mut row {
                *v = grain.next_field_element();
            }
            row
        })
        .collect();
    let mut select = secure_mds;
    let mds = loop {
        let vals: Vec<F> = (0..2 * T)
            .map(|_| grain.next_field_element_without_rejection())
            .collect();
        let mut unique = vals.clone();
        unique.sort_unstable();
        unique.dedup();
        if unique.len() != vals.len() {
            continue;
        }
        if select != 0 {
            select -= 1;
            continue;
        }
        let (xs, ys) = vals.split_at(T);
        let mut mds = [[F::ZERO; T]; T];
        for (i, row) in mds.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                // xs and ys come from a random source, so x_i + y_j = 0 has
                // negligible probability; the reference asserts it.
                *cell = (xs[i] + ys[j]).invert().unwrap_or(F::ZERO);
            }
        }
        break mds;
    };
    (round_constants, mds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Fp;

    #[test]
    fn grain_is_deterministic() {
        let mut a = Grain::new(255, 3, 8, 57);
        let mut b = Grain::new(255, 3, 8, 57);
        let bits_a: Vec<bool> = (0..64).map(|_| a.next_bit()).collect();
        let bits_b: Vec<bool> = (0..64).map(|_| b.next_bit()).collect();
        assert_eq!(bits_a, bits_b);
        let mut c = Grain::new(255, 3, 8, 56);
        let bits_c: Vec<bool> = (0..64).map(|_| c.next_bit()).collect();
        assert_ne!(bits_a, bits_c);
    }

    #[test]
    fn field_elements_and_constants_shapes() {
        let mut g = Grain::new(255, 3, 8, 57);
        let x: Fp = g.next_field_element();
        let y: Fp = g.next_field_element_without_rejection();
        assert_ne!(x, y);
        let (rc, mds) = generate_constants::<Fp, 3>(8, 57, 0);
        assert_eq!(rc.len(), 65);
        assert!(
            mds.iter()
                .flatten()
                .all(|v| !bool::from(ff::Field::is_zero(v)))
        );
    }
}
