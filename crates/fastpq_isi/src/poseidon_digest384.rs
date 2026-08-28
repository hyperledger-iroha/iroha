//! Six-lane Poseidon-x7 digest over the Goldilocks field.
//!
//! This is the canonical native-STARK commitment and transcript digest for the
//! first release.  It runs six independent width-three Poseidon permutations
//! over the same canonical frame.  Each lane has independently generated round
//! constants and an independently generated initial state, so the result is
//! not six squeezes from one capacity-one sponge.

use std::sync::OnceLock;

use crate::poseidon::{FIELD_MODULUS, MDS, RATE, STATE_WIDTH};

/// Number of independent Goldilocks lanes in the canonical digest.
pub const GOLDILOCKS_DIGEST384_LANES_V1: usize = 6;
/// Canonical encoded size of a six-lane digest.
pub const GOLDILOCKS_DIGEST384_BYTES_V1: usize = GOLDILOCKS_DIGEST384_LANES_V1 * 8;
/// Width-three Poseidon full rounds.
const FULL_ROUNDS_HALF_V1: usize = 4;
/// Width-three Poseidon partial rounds.
const PARTIAL_ROUNDS_V1: usize = 57;
/// Total width-three Poseidon rounds.
const TOTAL_ROUNDS_V1: usize = FULL_ROUNDS_HALF_V1 * 2 + PARTIAL_ROUNDS_V1;
/// Maximum byte length accepted for one framed field.
const MAX_FRAMED_FIELD_BYTES_V1: usize = u32::MAX as usize;
/// Parameter-generation algorithm identifier.
pub const GOLDILOCKS_DIGEST384_PARAMETER_GENERATOR_V1: &[u8] =
    b"shake256-rejection-sampling-u64le-below-goldilocks-v1";
/// Public seed for independently generating every lane's constants and IV.
pub const GOLDILOCKS_DIGEST384_PARAMETER_SEED_V1: &[u8] =
    b"iroha:first-release:native-stark:goldilocks-digest384:2026-08-28:v1";
const PARAMETER_GENERATOR_DOMAIN_V1: &[u8] =
    b"iroha:goldilocks-digest384:poseidon-x7:parameter-generator:v1";
#[cfg(test)]
const PARAMETER_ASSET_DOMAIN_V1: &[u8] = b"iroha:goldilocks-digest384:parameter-asset:v1";
const MESSAGE_FRAME_DOMAIN_V1: &[u8] = b"iroha:goldilocks-digest384:message-frame:v1";

/// SHA3-256 of the generated IVs, round constants, and shared MDS matrix.
///
/// The digest is checked by tests against an independently reproduced
/// generator.  It is metadata identity, not a native-STARK commitment hash.
pub const GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1: [u8; 32] = [
    0x84, 0xc5, 0x05, 0x5b, 0x47, 0xcc, 0x72, 0x89, 0x83, 0x5e, 0x0a, 0x5f, 0x31, 0xd4, 0x56, 0x38,
    0x49, 0x24, 0x4f, 0xfd, 0xdb, 0xf5, 0x1f, 0x5d, 0x67, 0xb1, 0xdb, 0x95, 0x22, 0x2c, 0xe3, 0xe6,
];

/// Complete typed domain for a native-STARK digest invocation.
///
/// Empty byte strings are valid values, but their field positions remain
/// explicit in the canonical frame.  Tree level and index are always present,
/// including for transcript roles where both are zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldilocksDigestDomainV1<'a> {
    /// Exact final catalog identity.
    pub catalog: &'a [u8],
    /// Exact protocol identity.
    pub protocol: &'a [u8],
    /// Exact proof profile identity.
    pub profile: &'a [u8],
    /// Tree, oracle, or transcript role.
    pub role: &'a [u8],
    /// Protocol phase within that role.
    pub phase: &'a [u8],
    /// Merkle or FRI level; zero for non-tree roles.
    pub level: u64,
    /// Leaf, node, query, or challenge index; zero when not applicable.
    pub index: u64,
    /// Monotonic transcript/challenge counter.
    pub counter: u64,
}

/// Canonical six-field-element Goldilocks digest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoldilocksDigest384V1([u64; GOLDILOCKS_DIGEST384_LANES_V1]);

impl GoldilocksDigest384V1 {
    /// Construct a digest only when every word is a canonical field element.
    #[must_use]
    pub fn new(words: [u64; GOLDILOCKS_DIGEST384_LANES_V1]) -> Option<Self> {
        words
            .iter()
            .all(|word| *word < FIELD_MODULUS)
            .then_some(Self(words))
    }

    /// Decode six canonical little-endian Goldilocks elements.
    #[must_use]
    pub fn from_le_bytes(bytes: [u8; GOLDILOCKS_DIGEST384_BYTES_V1]) -> Option<Self> {
        let mut words = [0_u64; GOLDILOCKS_DIGEST384_LANES_V1];
        for (word, chunk) in words.iter_mut().zip(bytes.chunks_exact(8)) {
            *word = u64::from_le_bytes(
                chunk
                    .try_into()
                    .expect("a 48-byte digest has six exact eight-byte chunks"),
            );
        }
        Self::new(words)
    }

    /// Return all six canonical field elements in lane order.
    #[must_use]
    pub const fn words(self) -> [u64; GOLDILOCKS_DIGEST384_LANES_V1] {
        self.0
    }

    /// Encode the six field elements as canonical little-endian words.
    #[must_use]
    pub fn to_le_bytes(self) -> [u8; GOLDILOCKS_DIGEST384_BYTES_V1] {
        let mut bytes = [0_u8; GOLDILOCKS_DIGEST384_BYTES_V1];
        for (index, word) in self.0.iter().enumerate() {
            bytes[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
}

#[derive(Clone)]
struct LaneParametersV1 {
    initial_state: [u64; STATE_WIDTH],
    round_constants: [[u64; STATE_WIDTH]; TOTAL_ROUNDS_V1],
}

static LANE_PARAMETERS_V1: OnceLock<[LaneParametersV1; GOLDILOCKS_DIGEST384_LANES_V1]> =
    OnceLock::new();

const KECCAK_RATE_256_V1: usize = 136;
const KECCAK_ROUND_CONSTANTS_V1: [u64; 24] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808a,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808b,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008a,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000a,
    0x0000_0000_8000_808b,
    0x8000_0000_0000_008b,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800a,
    0x8000_0000_8000_000a,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];
const KECCAK_RHO_V1: [u32; 25] = [
    0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14,
];

#[derive(Clone)]
struct Keccak256SpongeV1 {
    state: [u64; 25],
    position: usize,
}

impl Keccak256SpongeV1 {
    fn new() -> Self {
        Self {
            state: [0; 25],
            position: 0,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            let lane = self.position / 8;
            let shift = (self.position % 8) * 8;
            self.state[lane] ^= u64::from(*byte) << shift;
            self.position += 1;
            if self.position == KECCAK_RATE_256_V1 {
                keccak_f1600_v1(&mut self.state);
                self.position = 0;
            }
        }
    }

    fn finalize(mut self, suffix: u8) -> Keccak256ReaderV1 {
        let lane = self.position / 8;
        let shift = (self.position % 8) * 8;
        self.state[lane] ^= u64::from(suffix) << shift;
        let terminal_position = KECCAK_RATE_256_V1 - 1;
        self.state[terminal_position / 8] ^= 0x80_u64 << ((terminal_position % 8) * 8);
        keccak_f1600_v1(&mut self.state);
        Keccak256ReaderV1 {
            state: self.state,
            position: 0,
        }
    }
}

struct Keccak256ReaderV1 {
    state: [u64; 25],
    position: usize,
}

impl Keccak256ReaderV1 {
    fn read(&mut self, output: &mut [u8]) {
        for byte in output {
            if self.position == KECCAK_RATE_256_V1 {
                keccak_f1600_v1(&mut self.state);
                self.position = 0;
            }
            let lane = self.position / 8;
            let shift = (self.position % 8) * 8;
            *byte = u8::try_from((self.state[lane] >> shift) & 0xff)
                .expect("masked Keccak byte fits u8");
            self.position += 1;
        }
    }
}

fn keccak_f1600_v1(state: &mut [u64; 25]) {
    for round_constant in KECCAK_ROUND_CONSTANTS_V1 {
        let mut parity = [0_u64; 5];
        for x in 0..5 {
            parity[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        for x in 0..5 {
            let adjustment = parity[(x + 4) % 5] ^ parity[(x + 1) % 5].rotate_left(1);
            for y in 0..5 {
                state[x + 5 * y] ^= adjustment;
            }
        }
        let mut permuted = [0_u64; 25];
        for x in 0..5 {
            for y in 0..5 {
                let new_x = y;
                let new_y = (2 * x + 3 * y) % 5;
                permuted[new_x + 5 * new_y] =
                    state[x + 5 * y].rotate_left(KECCAK_RHO_V1[x + 5 * y]);
            }
        }
        for x in 0..5 {
            for y in 0..5 {
                state[x + 5 * y] = permuted[x + 5 * y]
                    ^ ((!permuted[(x + 1) % 5 + 5 * y]) & permuted[(x + 2) % 5 + 5 * y]);
            }
        }
        state[0] ^= round_constant;
    }
}

fn shake256_reader_v1(fields: &[&[u8]]) -> Keccak256ReaderV1 {
    let mut sponge = Keccak256SpongeV1::new();
    for field in fields {
        sponge.update(field);
    }
    sponge.finalize(0x1f)
}

#[cfg(test)]
fn sha3_256_v1(fields: &[&[u8]]) -> [u8; 32] {
    let mut sponge = Keccak256SpongeV1::new();
    for field in fields {
        sponge.update(field);
    }
    let mut reader = sponge.finalize(0x06);
    let mut output = [0_u8; 32];
    reader.read(&mut output);
    output
}

fn lane_parameters_v1() -> &'static [LaneParametersV1; GOLDILOCKS_DIGEST384_LANES_V1] {
    LANE_PARAMETERS_V1.get_or_init(generate_lane_parameters_v1)
}

fn generate_lane_parameters_v1() -> [LaneParametersV1; GOLDILOCKS_DIGEST384_LANES_V1] {
    core::array::from_fn(|lane| {
        let generator_length = u64::try_from(GOLDILOCKS_DIGEST384_PARAMETER_GENERATOR_V1.len())
            .expect("fixed generator identifier length fits u64")
            .to_le_bytes();
        let seed_length = u64::try_from(GOLDILOCKS_DIGEST384_PARAMETER_SEED_V1.len())
            .expect("fixed parameter seed length fits u64")
            .to_le_bytes();
        let lane = u64::try_from(lane)
            .expect("six-lane index fits u64")
            .to_le_bytes();
        let mut reader = shake256_reader_v1(&[
            PARAMETER_GENERATOR_DOMAIN_V1,
            &generator_length,
            GOLDILOCKS_DIGEST384_PARAMETER_GENERATOR_V1,
            &seed_length,
            GOLDILOCKS_DIGEST384_PARAMETER_SEED_V1,
            &lane,
        ]);
        let mut next_field = || loop {
            let mut bytes = [0_u8; 8];
            reader.read(&mut bytes);
            let candidate = u64::from_le_bytes(bytes);
            if candidate < FIELD_MODULUS {
                break candidate;
            }
        };
        LaneParametersV1 {
            initial_state: core::array::from_fn(|_| next_field()),
            round_constants: core::array::from_fn(|_| core::array::from_fn(|_| next_field())),
        }
    })
}

#[inline]
fn add_v1(left: u64, right: u64) -> u64 {
    let sum = left.wrapping_add(right);
    let mut reduced = sum;
    if sum < left {
        reduced = reduced.wrapping_sub(FIELD_MODULUS);
    }
    if reduced >= FIELD_MODULUS {
        reduced - FIELD_MODULUS
    } else {
        reduced
    }
}

#[inline]
fn reduce_wide_v1(value: u128) -> u64 {
    let low =
        u64::try_from(value & u128::from(u64::MAX)).expect("masked low product word fits u64");
    let high = u64::try_from(value >> 64).expect("high product word fits u64");
    let high_low = i128::from(high & 0xffff_ffff);
    let high_high = i128::from(high >> 32);
    let mut accumulated = i128::from(low) + (high_low << 32) - high_low - high_high;
    let modulus = i128::from(FIELD_MODULUS);
    while accumulated < 0 {
        accumulated += modulus;
    }
    while accumulated >= modulus {
        accumulated -= modulus;
    }
    u64::try_from(accumulated).expect("Goldilocks reduction is canonical")
}

#[inline]
fn multiply_v1(left: u64, right: u64) -> u64 {
    reduce_wide_v1(u128::from(left) * u128::from(right))
}

#[inline]
fn pow7_v1(value: u64) -> u64 {
    let square = multiply_v1(value, value);
    let fourth = multiply_v1(square, square);
    multiply_v1(multiply_v1(fourth, square), value)
}

fn apply_mds_v1(state: &mut [u64; STATE_WIDTH]) {
    let prior = *state;
    for row in 0..STATE_WIDTH {
        state[row] = prior
            .iter()
            .enumerate()
            .fold(0_u64, |sum, (column, value)| {
                add_v1(sum, multiply_v1(MDS[row][column], *value))
            });
    }
}

fn permute_v1(state: &mut [u64; STATE_WIDTH], parameters: &LaneParametersV1) {
    for round in 0..TOTAL_ROUNDS_V1 {
        for (word, constant) in state
            .iter_mut()
            .zip(parameters.round_constants[round].iter())
        {
            *word = add_v1(*word, *constant);
        }
        let is_full =
            !(FULL_ROUNDS_HALF_V1..FULL_ROUNDS_HALF_V1 + PARTIAL_ROUNDS_V1).contains(&round);
        if is_full {
            for word in state.iter_mut() {
                *word = pow7_v1(*word);
            }
        } else {
            state[0] = pow7_v1(state[0]);
        }
        apply_mds_v1(state);
    }
}

struct LaneSpongeV1<'a> {
    state: [u64; STATE_WIDTH],
    pending: [u64; RATE],
    pending_len: usize,
    parameters: &'a LaneParametersV1,
}

impl<'a> LaneSpongeV1<'a> {
    fn new(parameters: &'a LaneParametersV1) -> Self {
        Self {
            state: parameters.initial_state,
            pending: [0; RATE],
            pending_len: 0,
            parameters,
        }
    }

    fn absorb(&mut self, value: u64) {
        debug_assert!(value < FIELD_MODULUS);
        self.pending[self.pending_len] = value;
        self.pending_len += 1;
        if self.pending_len == RATE {
            self.flush();
        }
    }

    fn flush(&mut self) {
        for (state, value) in self.state.iter_mut().zip(self.pending) {
            *state = add_v1(*state, value);
        }
        permute_v1(&mut self.state, self.parameters);
        self.pending = [0; RATE];
        self.pending_len = 0;
    }

    fn finish(mut self) -> u64 {
        // A final element equal to one makes the field-element stream
        // prefix-free, including when the pending rate block is empty.
        self.absorb(1);
        if self.pending_len != 0 {
            self.flush();
        }
        self.state[0]
    }
}

fn absorb_byte_field_v1(sponge: &mut LaneSpongeV1<'_>, tag: u64, bytes: &[u8]) -> Option<()> {
    if bytes.len() > MAX_FRAMED_FIELD_BYTES_V1 {
        return None;
    }
    sponge.absorb(tag);
    sponge.absorb(u64::try_from(bytes.len()).ok()?);
    let mut chunks = bytes.chunks_exact(7);
    for chunk in &mut chunks {
        let mut word = [0_u8; 8];
        word[..7].copy_from_slice(chunk);
        sponge.absorb(u64::from_le_bytes(word));
    }
    let remainder = chunks.remainder();
    let mut terminal = [0_u8; 8];
    terminal[..remainder.len()].copy_from_slice(remainder);
    terminal[remainder.len()] = 1;
    sponge.absorb(u64::from_le_bytes(terminal));
    Some(())
}

fn absorb_domain_v1(
    sponge: &mut LaneSpongeV1<'_>,
    domain: GoldilocksDigestDomainV1<'_>,
    lane: u64,
) -> Option<()> {
    absorb_byte_field_v1(sponge, 1, MESSAGE_FRAME_DOMAIN_V1)?;
    absorb_byte_field_v1(sponge, 2, domain.catalog)?;
    absorb_byte_field_v1(sponge, 3, domain.protocol)?;
    absorb_byte_field_v1(sponge, 4, domain.profile)?;
    absorb_byte_field_v1(sponge, 5, domain.role)?;
    absorb_byte_field_v1(sponge, 6, domain.phase)?;
    absorb_byte_field_v1(sponge, 7, &domain.level.to_le_bytes())?;
    absorb_byte_field_v1(sponge, 8, &domain.index.to_le_bytes())?;
    absorb_byte_field_v1(sponge, 9, &domain.counter.to_le_bytes())?;
    absorb_byte_field_v1(sponge, 10, &lane.to_le_bytes())?;
    Some(())
}

/// Hash a typed domain and ordered byte fields into six independent lanes.
///
/// Returns `None` only if the part count or one field length exceeds the
/// canonical 32-bit framing ceiling.
#[must_use]
pub fn hash_bytes_384_v1(
    domain: GoldilocksDigestDomainV1<'_>,
    fields: &[&[u8]],
) -> Option<GoldilocksDigest384V1> {
    let domain_fields = [
        domain.catalog,
        domain.protocol,
        domain.profile,
        domain.role,
        domain.phase,
    ];
    if fields.len() > MAX_FRAMED_FIELD_BYTES_V1
        || fields
            .iter()
            .chain(domain_fields.iter())
            .any(|field| field.len() > MAX_FRAMED_FIELD_BYTES_V1)
    {
        return None;
    }
    let words = core::array::from_fn(|lane| {
        let mut sponge = LaneSpongeV1::new(&lane_parameters_v1()[lane]);
        absorb_domain_v1(
            &mut sponge,
            domain,
            u64::try_from(lane).expect("six-lane index fits u64"),
        )
        .expect("domain fields passed the shared size check");
        sponge.absorb(11);
        sponge.absorb(u64::try_from(fields.len()).expect("field count passed the framing ceiling"));
        for (index, field) in fields.iter().enumerate() {
            absorb_byte_field_v1(
                &mut sponge,
                12 + u64::try_from(index).expect("field count fits u64"),
                field,
            )
            .expect("field length was checked for every lane");
        }
        sponge.finish()
    });
    GoldilocksDigest384V1::new(words)
}

#[cfg(test)]
fn parameter_asset_sha3_256_v1() -> [u8; 32] {
    let mut bytes = Vec::with_capacity(
        PARAMETER_ASSET_DOMAIN_V1.len()
            + GOLDILOCKS_DIGEST384_PARAMETER_GENERATOR_V1.len()
            + GOLDILOCKS_DIGEST384_PARAMETER_SEED_V1.len()
            + GOLDILOCKS_DIGEST384_LANES_V1 * (1 + STATE_WIDTH + TOTAL_ROUNDS_V1 * STATE_WIDTH) * 8
            + STATE_WIDTH * STATE_WIDTH * 8,
    );
    bytes.extend_from_slice(PARAMETER_ASSET_DOMAIN_V1);
    bytes.extend_from_slice(GOLDILOCKS_DIGEST384_PARAMETER_GENERATOR_V1);
    bytes.extend_from_slice(GOLDILOCKS_DIGEST384_PARAMETER_SEED_V1);
    for (lane, parameters) in lane_parameters_v1().iter().enumerate() {
        bytes.extend_from_slice(
            &u64::try_from(lane)
                .expect("six-lane index fits u64")
                .to_le_bytes(),
        );
        for value in parameters
            .initial_state
            .iter()
            .chain(parameters.round_constants.iter().flatten())
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for value in MDS.iter().flatten() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    sha3_256_v1(&[&bytes])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain() -> GoldilocksDigestDomainV1<'static> {
        GoldilocksDigestDomainV1 {
            catalog: b"iroha-privacy-exact12-v1",
            protocol: b"test-protocol-v1",
            profile: b"stark-fri-poseidon-x7-goldilocks-6x64-v1",
            role: b"trace-merkle",
            phase: b"leaf",
            level: 0,
            index: 7,
            counter: 0,
        }
    }

    #[test]
    fn internal_keccak_matches_empty_string_kats() {
        assert_eq!(
            sha3_256_v1(&[b""]),
            [
                0xa7, 0xff, 0xc6, 0xf8, 0xbf, 0x1e, 0xd7, 0x66, 0x51, 0xc1, 0x47, 0x56, 0xa0, 0x61,
                0xd6, 0x62, 0xf5, 0x80, 0xff, 0x4d, 0xe4, 0x3b, 0x49, 0xfa, 0x82, 0xd8, 0x0a, 0x4b,
                0x80, 0xf8, 0x43, 0x4a,
            ]
        );
        let mut shake = shake256_reader_v1(&[b""]);
        let mut output = [0_u8; 32];
        shake.read(&mut output);
        assert_eq!(
            output,
            [
                0x46, 0xb9, 0xdd, 0x2b, 0x0b, 0xa8, 0x8d, 0x13, 0x23, 0x3b, 0x3f, 0xeb, 0x74, 0x3e,
                0xeb, 0x24, 0x3f, 0xcd, 0x52, 0xea, 0x62, 0xb8, 0x1b, 0x82, 0xb5, 0x0c, 0x27, 0x64,
                0x6e, 0xd5, 0x76, 0x2f,
            ]
        );
    }

    #[test]
    fn digest_wire_rejects_noncanonical_words() {
        assert!(GoldilocksDigest384V1::new([0; 6]).is_some());
        let mut invalid = [0_u64; 6];
        invalid[4] = FIELD_MODULUS;
        assert!(GoldilocksDigest384V1::new(invalid).is_none());
        let mut bytes = [0_u8; 48];
        bytes[16..24].copy_from_slice(&FIELD_MODULUS.to_le_bytes());
        assert!(GoldilocksDigest384V1::from_le_bytes(bytes).is_none());
    }

    #[test]
    fn generated_parameters_are_canonical_and_independent() {
        let parameters = lane_parameters_v1();
        for lane in parameters {
            assert!(lane.initial_state.iter().all(|word| *word < FIELD_MODULUS));
            assert!(
                lane.round_constants
                    .iter()
                    .flatten()
                    .all(|word| *word < FIELD_MODULUS)
            );
        }
        for left in 0..parameters.len() {
            for right in left + 1..parameters.len() {
                assert_ne!(
                    parameters[left].initial_state,
                    parameters[right].initial_state
                );
                assert_ne!(
                    parameters[left].round_constants,
                    parameters[right].round_constants
                );
            }
        }
    }

    #[test]
    fn parameter_asset_digest_is_pinned() {
        assert_eq!(
            parameter_asset_sha3_256_v1(),
            GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1
        );
    }

    #[test]
    fn byte_field_boundaries_are_unambiguous() {
        let split = hash_bytes_384_v1(domain(), &[b"abc", b"def"]).unwrap();
        let joined = hash_bytes_384_v1(domain(), &[b"abcdef"]).unwrap();
        let trailing_zero = hash_bytes_384_v1(domain(), &[b"abcdef\0"]).unwrap();
        assert_ne!(split, joined);
        assert_ne!(joined, trailing_zero);
    }

    #[test]
    fn every_typed_domain_coordinate_is_binding() {
        let original = hash_bytes_384_v1(domain(), &[b"payload"]).unwrap();
        let mut changed = domain();
        changed.protocol = b"other-protocol-v1";
        assert_ne!(original, hash_bytes_384_v1(changed, &[b"payload"]).unwrap());
        changed = domain();
        changed.level = 1;
        assert_ne!(original, hash_bytes_384_v1(changed, &[b"payload"]).unwrap());
        changed = domain();
        changed.index = 8;
        assert_ne!(original, hash_bytes_384_v1(changed, &[b"payload"]).unwrap());
        changed = domain();
        changed.counter = 1;
        assert_ne!(original, hash_bytes_384_v1(changed, &[b"payload"]).unwrap());
    }

    #[test]
    fn digest_has_six_independent_canonical_outputs() {
        let digest = hash_bytes_384_v1(domain(), &[b"payload"]).unwrap();
        let words = digest.words();
        assert_eq!(
            words,
            [
                0x0a08_4d27_65a9_990b,
                0xd59f_602c_37b6_9e1b,
                0xde9b_b335_7209_fa18,
                0x3faf_16ba_65a6_7ba3,
                0xe68c_cc7d_9933_b79d,
                0xcad6_6b94_7931_4d52,
            ]
        );
        assert!(words.iter().all(|word| *word < FIELD_MODULUS));
        for left in 0..words.len() {
            for right in left + 1..words.len() {
                assert_ne!(words[left], words[right]);
            }
        }
        assert_eq!(
            GoldilocksDigest384V1::from_le_bytes(digest.to_le_bytes()),
            Some(digest)
        );
    }
}
