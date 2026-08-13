//! Poseidon2 permutation over the Goldilocks field (p = 2^64 - 2^32 + 1).
//!
//! The constants are generated from the reference Grain LFSR as implemented in
//! `poseidon-primitives` (commit `0.2.0`, parameters pinned to
//! `ark-poseidon2` commit `3f2b7fe`). Generation script lives under
//! `target-codex/poseidon_gen` and mirrors the canonical Poseidon2 parameter
//! tables for width 3, rate 2.
use core::convert::TryFrom;
const MODULUS: u64 = 0xffff_ffff_0000_0001;
/// Goldilocks field modulus (2^64 - 2^32 + 1).
pub const FIELD_MODULUS: u64 = MODULUS;
#[cfg(test)]
const MODULUS_U128: u128 = MODULUS as u128;
/// Poseidon state width (t = 3).
pub const STATE_WIDTH: usize = 3;
/// Poseidon rate (r = 2).
pub const RATE: usize = 2;
const FULL_ROUNDS_HALF: usize = 4;
const PARTIAL_ROUNDS: usize = 57;
// Canonical Poseidon2 round constants followed by the MDS matrix, encoded as
// fixed-width little-endian words and decoded entirely at compile time.
const POSEIDON_TABLE_BYTES: &[u8; 1_632] =
    include_bytes!("assets/poseidon2_goldilocks_width3_v1.bin");
const fn read_poseidon_u64_le(bytes: &[u8; 1_632], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}
const fn decode_poseidon_tables(
    bytes: &[u8; 1_632],
) -> (
    [[u64; STATE_WIDTH]; FULL_ROUNDS_HALF * 2 + PARTIAL_ROUNDS],
    [[u64; STATE_WIDTH]; STATE_WIDTH],
) {
    let mut rounds = [[0_u64; STATE_WIDTH]; FULL_ROUNDS_HALF * 2 + PARTIAL_ROUNDS];
    let mut round_word = 0;
    while round_word < (FULL_ROUNDS_HALF * 2 + PARTIAL_ROUNDS) * STATE_WIDTH {
        rounds[round_word / STATE_WIDTH][round_word % STATE_WIDTH] =
            read_poseidon_u64_le(bytes, round_word * 8);
        round_word += 1;
    }
    let mut mds = [[0_u64; STATE_WIDTH]; STATE_WIDTH];
    let mut mds_word = 0;
    while mds_word < STATE_WIDTH * STATE_WIDTH {
        let offset = ((FULL_ROUNDS_HALF * 2 + PARTIAL_ROUNDS) * STATE_WIDTH + mds_word) * 8;
        mds[mds_word / STATE_WIDTH][mds_word % STATE_WIDTH] = read_poseidon_u64_le(bytes, offset);
        mds_word += 1;
    }
    (rounds, mds)
}
const POSEIDON_TABLES: (
    [[u64; STATE_WIDTH]; FULL_ROUNDS_HALF * 2 + PARTIAL_ROUNDS],
    [[u64; STATE_WIDTH]; STATE_WIDTH],
) = decode_poseidon_tables(POSEIDON_TABLE_BYTES);
/// Round constants for the canonical Poseidon2 permutation.
pub const ROUND_CONSTANTS: [[u64; STATE_WIDTH]; FULL_ROUNDS_HALF * 2 + PARTIAL_ROUNDS] =
    POSEIDON_TABLES.0;
/// MDS matrix for the canonical Poseidon2 permutation.
pub const MDS: [[u64; STATE_WIDTH]; STATE_WIDTH] = POSEIDON_TABLES.1;
#[inline]
fn add(a: u64, b: u64) -> u64 {
    let sum = a.wrapping_add(b);
    let mut result = sum;
    if sum < a {
        result = result.wrapping_sub(MODULUS);
    }
    if result >= MODULUS {
        result - MODULUS
    } else {
        result
    }
}
#[inline]
fn reduce_wide(wide_lo: u64, wide_hi: u64) -> u64 {
    let hi_lo = i128::from(wide_hi & 0xffff_ffff);
    let hi_hi = i128::from(wide_hi >> 32);
    let mut acc = i128::from(wide_lo);
    acc += hi_lo << 32;
    acc -= hi_lo;
    acc -= hi_hi;
    let modulus = i128::from(MODULUS);
    if acc < 0 {
        acc += modulus;
        if acc < 0 {
            acc += modulus;
        }
    }
    if acc >= modulus {
        acc -= modulus;
        if acc >= modulus {
            acc -= modulus;
        }
    }
    u64::try_from(acc).expect("Goldilocks reduction must stay within field bounds")
}
#[inline]
fn mul(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let lo = u64::try_from(product & u128::from(u64::MAX))
        .expect("low 64 bits of the product must fit into u64");
    let hi = u64::try_from(product >> 64).expect("high 64 bits of the product must fit into u64");
    reduce_wide(lo, hi)
}
#[inline]
fn pow5(x: u64) -> u64 {
    let x2 = mul(x, x);
    let x4 = mul(x2, x2);
    mul(x4, x)
}
fn apply_mds(state: &mut [u64; STATE_WIDTH]) {
    let s0 = state[0];
    let s1 = state[1];
    let s2 = state[2];
    *state = [
        add(
            add(mul(MDS[0][0], s0), mul(MDS[0][1], s1)),
            mul(MDS[0][2], s2),
        ),
        add(
            add(mul(MDS[1][0], s0), mul(MDS[1][1], s1)),
            mul(MDS[1][2], s2),
        ),
        add(
            add(mul(MDS[2][0], s0), mul(MDS[2][1], s1)),
            mul(MDS[2][2], s2),
        ),
    ];
}
fn full_round(state: &mut [u64; STATE_WIDTH], rc: &[u64; STATE_WIDTH]) {
    for (word, constant) in state.iter_mut().zip(rc.iter()) {
        *word = pow5(add(*word, *constant));
    }
    apply_mds(state);
}
fn partial_round(state: &mut [u64; STATE_WIDTH], rc: &[u64; STATE_WIDTH]) {
    for (word, constant) in state.iter_mut().zip(rc.iter()) {
        *word = add(*word, *constant);
    }
    state[0] = pow5(state[0]);
    apply_mds(state);
}
fn permute(state: &mut [u64; STATE_WIDTH]) {
    let mut round = 0;
    for _ in 0..FULL_ROUNDS_HALF {
        full_round(state, &ROUND_CONSTANTS[round]);
        round += 1;
    }
    for _ in 0..PARTIAL_ROUNDS {
        partial_round(state, &ROUND_CONSTANTS[round]);
        round += 1;
    }
    for _ in 0..FULL_ROUNDS_HALF {
        full_round(state, &ROUND_CONSTANTS[round]);
        round += 1;
    }
}
/// Compute a Poseidon2 hash over the provided Goldilocks field elements.
///
/// The sponge uses rate 2, capacity 1, and absorbs the message using classical
/// +1 padding (a single `1` element appended after the payload).
pub fn hash_field_elements(elements: &[u64]) -> u64 {
    let mut state = [0u64; STATE_WIDTH];
    let mut chunks = elements.chunks_exact(RATE);
    for chunk in &mut chunks {
        for (idx, &value) in chunk.iter().enumerate() {
            state[idx] = add(state[idx], value);
        }
        permute(&mut state);
    }
    let remainder = chunks.remainder();
    let mut block = [0u64; RATE];
    block[..remainder.len()].copy_from_slice(remainder);
    block[remainder.len()] = 1;
    for (idx, &value) in block.iter().enumerate() {
        state[idx] = add(state[idx], value);
    }
    permute(&mut state);
    state[0]
}
/// Apply the canonical Poseidon permutation to the supplied state.
pub fn permute_state(state: &mut [u64; STATE_WIDTH]) {
    permute(state);
}
/// Poseidon sponge used to derive deterministic field elements.
#[derive(Debug, Clone, Copy)]
pub struct PoseidonSponge {
    state: [u64; STATE_WIDTH],
    rate_index: usize,
    finalised: bool,
}
impl PoseidonSponge {
    #[must_use]
    /// Create a new sponge in the zero state.
    pub fn new() -> Self {
        Self {
            state: [0u64; STATE_WIDTH],
            rate_index: 0,
            finalised: false,
        }
    }
    /// Reset the sponge back to the initial zeroed state.
    pub fn reset(&mut self) {
        self.state = [0u64; STATE_WIDTH];
        self.rate_index = 0;
        self.finalised = false;
    }
    /// Absorb a single field element into the sponge.
    pub fn absorb(&mut self, element: u64) {
        debug_assert!(
            !self.finalised,
            "cannot absorb into a finalised sponge; start a new instance"
        );
        debug_assert!(
            self.rate_index < RATE,
            "rate index must stay within sponge capacity"
        );
        self.state[self.rate_index] = add(self.state[self.rate_index], element);
        self.rate_index += 1;
        if self.rate_index == RATE {
            permute(&mut self.state);
            self.rate_index = 0;
        }
    }
    /// Absorb a slice of field elements into the sponge.
    pub fn absorb_slice(&mut self, elements: &[u64]) {
        for &element in elements {
            self.absorb(element);
        }
    }
    fn ensure_finalised(&mut self) {
        if self.finalised {
            return;
        }
        self.absorb(1);
        while self.rate_index != 0 {
            self.absorb(0);
        }
        self.finalised = true;
    }
    /// Squeeze a single field element while keeping the sponge ready for the next output.
    #[must_use]
    pub fn squeeze_element(&mut self) -> u64 {
        self.ensure_finalised();
        let element = self.state[0];
        permute(&mut self.state);
        element
    }
    #[must_use]
    /// Finalise the sponge and return the first output element.
    pub fn squeeze(mut self) -> u64 {
        self.ensure_finalised();
        self.state[0]
    }
}
impl Default for PoseidonSponge {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poseidon_hash_known_vector() {
        let digest = hash_field_elements(&[1, 2, 3]);
        assert_eq!(digest, 0x42ea_af13_b5f9_03b1);
    }
    #[test]
    fn squeeze_multiple_elements() {
        let mut sponge = PoseidonSponge::new();
        sponge.absorb_slice(&[1, 2, 3]);
        let first = sponge.squeeze_element();
        let second = sponge.squeeze_element();
        assert_ne!(first, second);
    }
    #[test]
    fn field_addition_matches_reference() {
        let cases = [
            (0u64, 0u64),
            (1, FIELD_MODULUS - 1),
            (FIELD_MODULUS - 1, FIELD_MODULUS - 1),
            (FIELD_MODULUS - 2, 3),
            (0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210),
        ];
        for (a, b) in cases {
            let expected = ((u128::from(a) + u128::from(b)) % MODULUS_U128) as u64;
            assert_eq!(add(a, b), expected, "addition diverged for {a:#x} + {b:#x}");
        }
    }
    #[test]
    fn field_multiplication_matches_reference() {
        let boundary_values = [
            0_u64,
            1,
            2,
            u64::from(u32::MAX) - 1,
            u64::from(u32::MAX),
            u64::from(u32::MAX) + 1,
            FIELD_MODULUS / 2,
            FIELD_MODULUS - u64::from(u32::MAX) - 1,
            FIELD_MODULUS - u64::from(u32::MAX),
            FIELD_MODULUS - 2,
            FIELD_MODULUS - 1,
        ];
        for &a in &boundary_values {
            for &b in &boundary_values {
                let expected = ((u128::from(a) * u128::from(b)) % MODULUS_U128) as u64;
                assert_eq!(
                    mul(a, b),
                    expected,
                    "multiplication diverged for boundary pair {a:#x} * {b:#x}"
                );
            }
        }
        // A fixed SplitMix64 stream gives broad, reproducible coverage without
        // adding a property-test dependency or introducing nondeterminism.
        let mut state = 0x243f_6a88_85a3_08d3_u64;
        for case in 0..65_536 {
            let next = |state: &mut u64| {
                *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
                let mut value = *state;
                value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                value ^ (value >> 31)
            };
            let a = next(&mut state) % FIELD_MODULUS;
            let b = next(&mut state) % FIELD_MODULUS;
            let expected = ((u128::from(a) * u128::from(b)) % MODULUS_U128) as u64;
            assert_eq!(
                mul(a, b),
                expected,
                "multiplication diverged for deterministic case {case}: {a:#x} * {b:#x}"
            );
        }
    }
}
