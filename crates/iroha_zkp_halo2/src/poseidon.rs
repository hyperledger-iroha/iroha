//! Deterministic Poseidon2 helpers shared across IVM and host components.
//!
//! These functions mirror the internal helpers historically shipped in the
//! `ivm` crate but live here so that other crates can depend on the canonical
//! permutation without duplicating the arithmetic. The implementation sticks
//! to the BN254 field parameters used by the fastpq Halo2 gadgets to keep all
//! call-sites in sync with the proving backend.

use halo2curves::{
    bn256::Fr,
    ff::{Field, PrimeField},
};
use once_cell::sync::OnceCell;
use poseidon_primitives::poseidon::primitives::Spec;

type PoseidonConstants<const W: usize> = (Vec<[Fr; W]>, [[Fr; W]; W]);

/// Poseidon2 parameters (round constants + MDS) encoded as byte arrays.
#[derive(Debug, Clone)]
pub struct Poseidon2Params<const W: usize> {
    /// Round constants for the width.
    pub round_constants: Vec<[[u8; 32]; W]>,
    /// MDS matrix entries for the width.
    pub mds: [[[u8; 32]; W]; W],
}

#[derive(Debug)]
struct FrSpec;

impl Spec<Fr, 3, 2> for FrSpec {
    fn full_rounds() -> usize {
        8
    }

    fn partial_rounds() -> usize {
        56
    }

    fn sbox(val: Fr) -> Fr {
        val.pow_vartime([5])
    }

    fn secure_mds() -> usize {
        0
    }
}

impl Spec<Fr, 6, 5> for FrSpec {
    fn full_rounds() -> usize {
        8
    }

    fn partial_rounds() -> usize {
        56
    }

    fn sbox(val: Fr) -> Fr {
        val.pow_vartime([5])
    }

    fn secure_mds() -> usize {
        0
    }
}

fn sbox(x: Fr) -> Fr {
    let x2 = x.square();
    let x4 = x2.square();
    x4 * x
}

fn apply_mds<const W: usize>(state: &mut [Fr; W], mds: &[[Fr; W]; W]) {
    let mut new_state = [Fr::ZERO; W];
    for (row_idx, row) in mds.iter().enumerate() {
        let mut acc = Fr::ZERO;
        for (col_idx, coeff) in row.iter().enumerate() {
            acc += *coeff * state[col_idx];
        }
        new_state[row_idx] = acc;
    }
    *state = new_state;
}

fn poseidon3_params() -> &'static PoseidonConstants<3> {
    static CONSTS: OnceCell<PoseidonConstants<3>> = OnceCell::new();
    CONSTS.get_or_init(|| {
        let (rc, m, _) = <FrSpec as Spec<Fr, 3, 2>>::constants();
        (rc, m)
    })
}

fn poseidon6_params() -> &'static PoseidonConstants<6> {
    static CONSTS: OnceCell<PoseidonConstants<6>> = OnceCell::new();
    CONSTS.get_or_init(|| {
        let (rc, m, _) = <FrSpec as Spec<Fr, 6, 5>>::constants();
        (rc, m)
    })
}

fn poseidon3_permute(state: &mut [Fr; 3]) {
    let (round_constants, mds) = poseidon3_params();
    let rf_half = <FrSpec as Spec<Fr, 3, 2>>::full_rounds() / 2;
    let rp = <FrSpec as Spec<Fr, 3, 2>>::partial_rounds();

    for rc in round_constants.iter().take(rf_half) {
        for (idx, word) in state.iter_mut().enumerate() {
            *word = sbox(*word + rc[idx]);
        }
        apply_mds(state, mds);
    }

    for rc in round_constants.iter().skip(rf_half).take(rp) {
        for (idx, word) in state.iter_mut().enumerate() {
            *word += rc[idx];
        }
        state[0] = sbox(state[0]);
        apply_mds(state, mds);
    }

    let tail_start = rf_half + rp;
    for rc in round_constants.iter().skip(tail_start).take(rf_half) {
        for (idx, word) in state.iter_mut().enumerate() {
            *word = sbox(*word + rc[idx]);
        }
        apply_mds(state, mds);
    }
}

fn poseidon2_field(a: u64, b: u64) -> Fr {
    let mut state = [Fr::from(a), Fr::from(b), Fr::ZERO];
    poseidon3_permute(&mut state);
    state[0]
}

fn poseidon6_field(inputs: [u64; 6]) -> Fr {
    let (round_constants, mds) = poseidon6_params();

    let mut state = [
        Fr::from(inputs[0]),
        Fr::from(inputs[1]),
        Fr::from(inputs[2]),
        Fr::from(inputs[3]),
        Fr::from(inputs[4]),
        Fr::from(inputs[5]),
    ];

    let rf_half = <FrSpec as Spec<Fr, 6, 5>>::full_rounds() / 2;
    let rp = <FrSpec as Spec<Fr, 6, 5>>::partial_rounds();

    for rc in round_constants.iter().take(rf_half) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = sbox(*s + rc[i]);
        }
        apply_mds(&mut state, mds);
    }

    for rc in round_constants.iter().skip(rf_half).take(rp) {
        for (i, s) in state.iter_mut().enumerate() {
            *s += rc[i];
        }
        state[0] = sbox(state[0]);
        apply_mds(&mut state, mds);
    }

    let tail_start = rf_half + rp;
    for rc in round_constants.iter().skip(tail_start).take(rf_half) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = sbox(*s + rc[i]);
        }
        apply_mds(&mut state, mds);
    }

    state[0]
}

fn params_to_bytes<const W: usize>(params: &PoseidonConstants<W>) -> Poseidon2Params<W> {
    let (round_constants, mds) = params;
    let round_constants = round_constants
        .iter()
        .map(|round| round.map(field_to_bytes))
        .collect();
    let mds = mds.map(|row| row.map(field_to_bytes));
    Poseidon2Params {
        round_constants,
        mds,
    }
}

/// Export Poseidon2 parameters for width 3 as byte arrays.
#[must_use]
pub fn poseidon2_params_width3() -> Poseidon2Params<3> {
    params_to_bytes(poseidon3_params())
}

/// Export Poseidon2 parameters for width 6 as byte arrays.
#[must_use]
pub fn poseidon2_params_width6() -> Poseidon2Params<6> {
    params_to_bytes(poseidon6_params())
}

fn pack_bytes_to_fr(bytes: &[u8]) -> Vec<Fr> {
    if bytes.is_empty() {
        return Vec::new();
    }
    bytes
        .chunks(8)
        .map(|chunk| {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            Fr::from(u64::from_le_bytes(buf))
        })
        .collect()
}

fn hash_words_internal(words: &[Fr]) -> Fr {
    const RATE: usize = 2;
    let mut padded = Vec::with_capacity(words.len() + RATE);
    padded.extend_from_slice(words);
    padded.push(Fr::ONE);
    while padded.len() % RATE != 0 {
        padded.push(Fr::ZERO);
    }

    let mut state = [Fr::ZERO; 3];
    for chunk in padded.chunks(RATE) {
        for (idx, word) in chunk.iter().enumerate() {
            state[idx] += *word;
        }
        poseidon3_permute(&mut state);
    }
    state[0]
}

/// Hash an arbitrary list of BN254 field elements using Poseidon2 (rate 2).
#[must_use]
pub fn hash_words(words: &[Fr]) -> Fr {
    hash_words_internal(words)
}

/// Hash an arbitrary list of BN254 field elements and return canonical bytes.
#[must_use]
pub fn hash_words_bytes(words: &[Fr]) -> [u8; 32] {
    field_to_bytes(hash_words_internal(words))
}

/// Hash an arbitrary byte slice using the Poseidon2 sponge.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let words = pack_bytes_to_fr(bytes);
    hash_words_bytes(&words)
}

fn field_to_bytes(f: Fr) -> [u8; 32] {
    let repr = f.to_repr();
    let mut out = [0u8; 32];
    out.copy_from_slice(repr.as_ref());
    out
}

fn field_to_u64(f: Fr) -> u64 {
    let bytes = field_to_bytes(f);
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// Hash two 64-bit limbs with Poseidon2 and return the resulting field element as bytes.
#[must_use]
pub fn hash2_bytes(a: u64, b: u64) -> [u8; 32] {
    field_to_bytes(poseidon2_field(a, b))
}

/// Hash six 64-bit limbs with Poseidon2 (width 6) and return the resulting bytes.
#[must_use]
pub fn hash6_bytes(inputs: [u64; 6]) -> [u8; 32] {
    field_to_bytes(poseidon6_field(inputs))
}

/// Hash two 64-bit limbs with Poseidon2 and return the low 64 bits.
#[must_use]
pub fn hash2_u64(a: u64, b: u64) -> u64 {
    field_to_u64(poseidon2_field(a, b))
}

/// Hash six 64-bit limbs with Poseidon2 (width 6) and return the low 64 bits.
#[must_use]
pub fn hash6_u64(inputs: [u64; 6]) -> u64 {
    field_to_u64(poseidon6_field(inputs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poseidon2_samples_are_consistent() {
        let cases = [
            (0u64, 0u64),
            (1u64, 2u64),
            (u64::MAX, 123_456_789),
            (0xDEAD_BEEF_DEAD_BEEF, 0x0123_4567_89AB_CDEF),
        ];

        for (a, b) in cases {
            let bytes_first = hash2_bytes(a, b);
            let bytes_second = hash2_bytes(a, b);
            assert_eq!(bytes_first, bytes_second, "Poseidon2 must be deterministic");

            let low = hash2_u64(a, b);
            assert_eq!(bytes_first[..8], low.to_le_bytes());
        }
    }

    #[test]
    fn poseidon6_samples_are_consistent() {
        let inputs = [1u64, 2, 3, 4, 5, 6];
        let bytes_first = hash6_bytes(inputs);
        let bytes_second = hash6_bytes(inputs);
        assert_eq!(bytes_first, bytes_second, "Poseidon6 must be deterministic");
        assert_eq!(bytes_first[..8], hash6_u64(inputs).to_le_bytes());
    }

    #[test]
    fn hash_bytes_known_vectors() {
        let cases: &[(&[u8], [u8; 32])] = &[
            (
                b"",
                [
                    8, 67, 194, 8, 22, 178, 229, 240, 102, 97, 83, 149, 171, 96, 134, 190, 43, 147,
                    105, 171, 134, 224, 225, 65, 17, 39, 233, 125, 191, 232, 56, 48,
                ],
            ),
            (
                b"poseidon",
                [
                    0, 85, 138, 198, 129, 161, 139, 250, 58, 61, 11, 20, 118, 192, 98, 213, 242,
                    123, 213, 245, 248, 208, 48, 83, 132, 50, 13, 171, 162, 58, 138, 4,
                ],
            ),
            (
                b"\x00\x01\x02\x03\x04\x05\x06\x07\x08",
                [
                    74, 93, 102, 31, 250, 207, 83, 16, 39, 42, 230, 225, 6, 130, 26, 222, 47, 33,
                    160, 220, 38, 95, 46, 37, 137, 14, 152, 198, 101, 168, 47, 8,
                ],
            ),
        ];

        for (input, expected) in cases {
            let digest = hash_bytes(input);
            assert_eq!(digest, *expected, "unexpected digest for input {input:?}");
        }
    }

    #[test]
    fn poseidon_params_exports_match_widths() {
        let params3 = poseidon2_params_width3();
        assert_eq!(
            params3.round_constants.len(),
            <FrSpec as Spec<Fr, 3, 2>>::full_rounds()
                + <FrSpec as Spec<Fr, 3, 2>>::partial_rounds(),
        );
        assert_eq!(params3.mds.len(), 3);
        assert_eq!(params3.mds[0].len(), 3);

        let params6 = poseidon2_params_width6();
        assert_eq!(
            params6.round_constants.len(),
            <FrSpec as Spec<Fr, 6, 5>>::full_rounds()
                + <FrSpec as Spec<Fr, 6, 5>>::partial_rounds(),
        );
        assert_eq!(params6.mds.len(), 6);
        assert_eq!(params6.mds[0].len(), 6);
    }
}
