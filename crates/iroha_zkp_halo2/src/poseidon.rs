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
use std::io::{self, Write};

const FULL_ROUNDS: usize = 8;
const FULL_ROUNDS_HALF: usize = FULL_ROUNDS / 2;
const PARTIAL_ROUNDS: usize = 56;
const ROUND_COUNT: usize = FULL_ROUNDS + PARTIAL_ROUNDS;
type PoseidonConstants<const W: usize> = ([[Fr; W]; ROUND_COUNT], [[Fr; W]; W]);

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
        FULL_ROUNDS
    }

    fn partial_rounds() -> usize {
        PARTIAL_ROUNDS
    }

    fn sbox(val: Fr) -> Fr {
        crate::poseidon::sbox(val)
    }

    fn secure_mds() -> usize {
        0
    }
}

impl Spec<Fr, 6, 5> for FrSpec {
    fn full_rounds() -> usize {
        FULL_ROUNDS
    }

    fn partial_rounds() -> usize {
        PARTIAL_ROUNDS
    }

    fn sbox(val: Fr) -> Fr {
        crate::poseidon::sbox(val)
    }

    fn secure_mds() -> usize {
        0
    }
}

#[inline(always)]
fn sbox(x: Fr) -> Fr {
    let x2 = x.square();
    let x4 = x2.square();
    x4 * x
}

#[inline(always)]
fn apply_mds3(state: &mut [Fr; 3], mds: &[[Fr; 3]; 3]) {
    let s0 = state[0];
    let s1 = state[1];
    let s2 = state[2];
    let [m00, m01, m02] = mds[0];
    let [m10, m11, m12] = mds[1];
    let [m20, m21, m22] = mds[2];
    *state = [
        m00 * s0 + m01 * s1 + m02 * s2,
        m10 * s0 + m11 * s1 + m12 * s2,
        m20 * s0 + m21 * s1 + m22 * s2,
    ];
}

#[inline(always)]
fn full_round3(state: &mut [Fr; 3], rc: &[Fr; 3], mds: &[[Fr; 3]; 3]) {
    state[0] = sbox(state[0] + rc[0]);
    state[1] = sbox(state[1] + rc[1]);
    state[2] = sbox(state[2] + rc[2]);
    apply_mds3(state, mds);
}

#[inline(always)]
fn partial_round3(state: &mut [Fr; 3], rc: &[Fr; 3], mds: &[[Fr; 3]; 3]) {
    state[0] += rc[0];
    state[1] += rc[1];
    state[2] += rc[2];
    state[0] = sbox(state[0]);
    apply_mds3(state, mds);
}

#[inline(always)]
fn fr_from_le_bytes(bytes: &[u8]) -> Fr {
    debug_assert!(bytes.len() >= 8);
    Fr::from(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

#[inline(always)]
fn partial_u64_from_le_bytes(bytes: &[u8; 8], len: usize) -> u64 {
    debug_assert!(len <= bytes.len());
    let word = u64::from_le_bytes(*bytes);
    match len {
        0 => 0,
        8 => word,
        _ => word & ((1u64 << (len * 8)) - 1),
    }
}

#[inline(always)]
fn apply_mds6(state: &mut [Fr; 6], mds: &[[Fr; 6]; 6]) {
    let s0 = state[0];
    let s1 = state[1];
    let s2 = state[2];
    let s3 = state[3];
    let s4 = state[4];
    let s5 = state[5];
    *state = [
        mds[0][0] * s0
            + mds[0][1] * s1
            + mds[0][2] * s2
            + mds[0][3] * s3
            + mds[0][4] * s4
            + mds[0][5] * s5,
        mds[1][0] * s0
            + mds[1][1] * s1
            + mds[1][2] * s2
            + mds[1][3] * s3
            + mds[1][4] * s4
            + mds[1][5] * s5,
        mds[2][0] * s0
            + mds[2][1] * s1
            + mds[2][2] * s2
            + mds[2][3] * s3
            + mds[2][4] * s4
            + mds[2][5] * s5,
        mds[3][0] * s0
            + mds[3][1] * s1
            + mds[3][2] * s2
            + mds[3][3] * s3
            + mds[3][4] * s4
            + mds[3][5] * s5,
        mds[4][0] * s0
            + mds[4][1] * s1
            + mds[4][2] * s2
            + mds[4][3] * s3
            + mds[4][4] * s4
            + mds[4][5] * s5,
        mds[5][0] * s0
            + mds[5][1] * s1
            + mds[5][2] * s2
            + mds[5][3] * s3
            + mds[5][4] * s4
            + mds[5][5] * s5,
    ];
}

fn poseidon3_params() -> &'static PoseidonConstants<3> {
    static CONSTS: OnceCell<PoseidonConstants<3>> = OnceCell::new();
    CONSTS.get_or_init(|| {
        let (rc, m, _) = <FrSpec as Spec<Fr, 3, 2>>::constants();
        (round_constants_array(rc), m)
    })
}

fn poseidon6_params() -> &'static PoseidonConstants<6> {
    static CONSTS: OnceCell<PoseidonConstants<6>> = OnceCell::new();
    CONSTS.get_or_init(|| {
        let (rc, m, _) = <FrSpec as Spec<Fr, 6, 5>>::constants();
        (round_constants_array(rc), m)
    })
}

fn round_constants_array<const W: usize>(rc: Vec<[Fr; W]>) -> [[Fr; W]; ROUND_COUNT] {
    rc.try_into()
        .unwrap_or_else(|rc: Vec<[Fr; W]>| panic!("unexpected Poseidon round count: {}", rc.len()))
}

#[inline(always)]
fn poseidon3_permute(state: &mut [Fr; 3]) {
    let (round_constants, mds) = poseidon3_params();

    for rc in &round_constants[..FULL_ROUNDS_HALF] {
        full_round3(state, rc, mds);
    }

    let partial_end = FULL_ROUNDS_HALF + PARTIAL_ROUNDS;
    for rc in &round_constants[FULL_ROUNDS_HALF..partial_end] {
        partial_round3(state, rc, mds);
    }

    for rc in &round_constants[partial_end..partial_end + FULL_ROUNDS_HALF] {
        full_round3(state, rc, mds);
    }
}

#[inline(always)]
fn poseidon2_field(a: u64, b: u64) -> Fr {
    let mut state = [Fr::from(a), Fr::from(b), Fr::ZERO];
    poseidon3_permute(&mut state);
    state[0]
}

#[inline(always)]
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
        apply_mds6(&mut state, mds);
    }

    for rc in round_constants.iter().skip(rf_half).take(rp) {
        for (i, s) in state.iter_mut().enumerate() {
            *s += rc[i];
        }
        state[0] = sbox(state[0]);
        apply_mds6(&mut state, mds);
    }

    let tail_start = rf_half + rp;
    for rc in round_constants.iter().skip(tail_start).take(rf_half) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = sbox(*s + rc[i]);
        }
        apply_mds6(&mut state, mds);
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

#[cfg(test)]
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
    let mut state = [Fr::ZERO; 3];
    let mut index = 0;
    while index + 1 < words.len() {
        state[0] += words[index];
        state[1] += words[index + 1];
        poseidon3_permute(&mut state);
        index += 2;
    }
    if index == words.len() {
        state[0] += Fr::ONE;
    } else {
        state[0] += words[index];
        state[1] += Fr::ONE;
    }
    poseidon3_permute(&mut state);
    state[0]
}

fn hash_u64_words_internal(words: &[u64]) -> Fr {
    let mut state = [Fr::ZERO; 3];
    let mut index = 0;
    while index + 1 < words.len() {
        state[0] += Fr::from(words[index]);
        state[1] += Fr::from(words[index + 1]);
        poseidon3_permute(&mut state);
        index += 2;
    }
    if index == words.len() {
        state[0] += Fr::ONE;
    } else {
        state[0] += Fr::from(words[index]);
        state[1] += Fr::ONE;
    }
    poseidon3_permute(&mut state);
    state[0]
}

/// Streaming byte hasher matching [`hash_bytes`] without materializing field words.
#[derive(Debug, Clone)]
pub struct PoseidonByteHasher {
    state: [Fr; 3],
    rate_len: usize,
    pending_bytes: [u8; 8],
    pending_len: usize,
}

impl Default for PoseidonByteHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl PoseidonByteHasher {
    /// Create an empty streaming byte hasher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: [Fr::ZERO; 3],
            rate_len: 0,
            pending_bytes: [0; 8],
            pending_len: 0,
        }
    }

    #[inline(always)]
    fn absorb_word(&mut self, word: Fr) {
        self.state[self.rate_len] += word;
        self.rate_len += 1;
        if self.rate_len == 2 {
            poseidon3_permute(&mut self.state);
            self.rate_len = 0;
        }
    }

    #[inline(always)]
    fn absorb_word_pair(&mut self, first: Fr, second: Fr) {
        debug_assert_eq!(self.rate_len, 0);
        self.state[0] += first;
        self.state[1] += second;
        poseidon3_permute(&mut self.state);
    }

    /// Add one already-packed little-endian `u64` byte word to the Poseidon sponge.
    #[inline]
    pub fn update_u64_le_word(&mut self, word: u64) {
        if self.pending_len == 0 {
            self.absorb_word(Fr::from(word));
        } else {
            self.update(&word.to_le_bytes());
        }
    }

    /// Add bytes to the Poseidon sponge.
    #[inline]
    pub fn update(&mut self, mut bytes: &[u8]) {
        if self.pending_len > 0 {
            let needed = self.pending_bytes.len() - self.pending_len;
            let take = needed.min(bytes.len());
            self.pending_bytes[self.pending_len..self.pending_len + take]
                .copy_from_slice(&bytes[..take]);
            self.pending_len += take;
            bytes = &bytes[take..];
            if self.pending_len == 8 {
                self.absorb_word(Fr::from(u64::from_le_bytes(self.pending_bytes)));
                self.pending_len = 0;
            }
        }

        if self.rate_len == 1 && bytes.len() >= 8 {
            self.absorb_word(fr_from_le_bytes(&bytes[..8]));
            bytes = &bytes[8..];
        }

        while bytes.len() >= 16 {
            self.absorb_word_pair(
                fr_from_le_bytes(&bytes[..8]),
                fr_from_le_bytes(&bytes[8..16]),
            );
            bytes = &bytes[16..];
        }

        if bytes.len() >= 8 {
            self.absorb_word(fr_from_le_bytes(&bytes[..8]));
            bytes = &bytes[8..];
        }

        if !bytes.is_empty() {
            self.pending_bytes[..bytes.len()].copy_from_slice(bytes);
            self.pending_len = bytes.len();
        }
    }

    /// Finish hashing and return canonical BN254 field bytes.
    #[must_use]
    #[inline]
    pub fn finalize(mut self) -> [u8; 32] {
        if self.pending_len > 0 {
            self.absorb_word(Fr::from(partial_u64_from_le_bytes(
                &self.pending_bytes,
                self.pending_len,
            )));
        }
        match self.rate_len {
            0 => {
                self.state[0] += Fr::ONE;
                poseidon3_permute(&mut self.state);
            }
            1 => {
                self.state[1] += Fr::ONE;
                poseidon3_permute(&mut self.state);
            }
            _ => unreachable!("Poseidon byte hasher rate length cannot exceed the rate"),
        }
        field_to_bytes(self.state[0])
    }
}

impl Write for PoseidonByteHasher {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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

/// Hash already packed little-endian `u64` byte words using the Poseidon2 sponge.
#[must_use]
pub fn hash_u64_words_bytes(words: &[u64]) -> [u8; 32] {
    field_to_bytes(hash_u64_words_internal(words))
}

/// Hash an arbitrary byte slice using the Poseidon2 sponge.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut state = [Fr::ZERO; 3];
    let mut bytes = bytes;

    while bytes.len() >= 16 {
        state[0] += fr_from_le_bytes(&bytes[..8]);
        state[1] += fr_from_le_bytes(&bytes[8..16]);
        poseidon3_permute(&mut state);
        bytes = &bytes[16..];
    }

    let mut rate_len = if bytes.len() >= 8 {
        state[0] += fr_from_le_bytes(&bytes[..8]);
        bytes = &bytes[8..];
        1
    } else {
        0
    };

    if !bytes.is_empty() {
        debug_assert!(bytes.len() < 8);
        let mut pending = [0u8; 8];
        pending[..bytes.len()].copy_from_slice(bytes);
        state[rate_len] += Fr::from(u64::from_le_bytes(pending));
        rate_len += 1;
        if rate_len == 2 {
            poseidon3_permute(&mut state);
            rate_len = 0;
        }
    }

    match rate_len {
        0 => state[0] += Fr::ONE,
        1 => state[1] += Fr::ONE,
        _ => unreachable!("Poseidon byte hash rate length cannot exceed the rate"),
    }

    poseidon3_permute(&mut state);
    field_to_bytes(state[0])
}

fn field_to_bytes(f: Fr) -> [u8; 32] {
    f.to_repr().into()
}

fn field_to_u64(f: Fr) -> u64 {
    let repr = f.to_repr();
    let bytes = repr.as_ref();
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
    fn hash_bytes_streaming_matches_word_path() {
        for len in [
            0usize, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33, 96, 127, 128, 129, 255, 256, 257, 511,
        ] {
            let input = (0..len)
                .map(|idx| idx.wrapping_mul(31) as u8)
                .collect::<Vec<_>>();
            let words = pack_bytes_to_fr(&input);
            assert_eq!(hash_bytes(&input), hash_words_bytes(&words), "len {len}");
            let u64_words = input
                .chunks(8)
                .map(|chunk| {
                    let mut word = [0u8; 8];
                    word[..chunk.len()].copy_from_slice(chunk);
                    u64::from_le_bytes(word)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                hash_bytes(&input),
                hash_u64_words_bytes(&u64_words),
                "len {len}"
            );
        }
    }

    #[test]
    fn poseidon_byte_hasher_split_updates_match_one_shot() {
        let input = (0..97).map(|idx| (idx * 17 + 3) as u8).collect::<Vec<_>>();
        let one_shot = hash_bytes(&input);

        for split in [0usize, 1, 2, 7, 8, 9, 16, 33, input.len()] {
            let mut hasher = PoseidonByteHasher::new();
            hasher.update(&input[..split]);
            hasher.update(&input[split..]);
            assert_eq!(hasher.finalize(), one_shot, "split {split}");
        }

        let mut bytewise = PoseidonByteHasher::new();
        for byte in &input {
            bytewise.update(core::slice::from_ref(byte));
        }
        assert_eq!(bytewise.finalize(), one_shot, "bytewise updates");
    }

    #[test]
    fn poseidon_byte_hasher_packed_word_after_partial_update_matches_one_shot() {
        let prefix = [0x10, 0x20, 0x30];
        let word = 0x8877_6655_4433_2211_u64;
        let suffix = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
        let mut input = Vec::new();
        input.extend_from_slice(&prefix);
        input.extend_from_slice(&word.to_le_bytes());
        input.extend_from_slice(&suffix);

        let mut hasher = PoseidonByteHasher::new();
        hasher.update(&prefix);
        hasher.update_u64_le_word(word);
        hasher.update(&suffix);

        assert_eq!(hasher.finalize(), hash_bytes(&input));
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
