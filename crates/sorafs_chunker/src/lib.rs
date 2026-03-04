//! Deterministic content-defined chunking utilities for the SoraFS stack.
//!
//! The implementation follows the profile described in the SoraFS Architecture
//! RFC (SF-1): a FastCDC-inspired rolling hash with a 256 KiB target chunk
//! size, 64 KiB minimum, 512 KiB maximum, and a 16-bit break mask. The chunker
//! emits consistent splits across platforms by avoiding architecture-specific
//! optimisations and by deriving the gear table from a fixed SHA3-256 seed.
//! Empty inputs still emit a single zero-length chunk so that manifests preserve
//! deterministic ordering for empty files.
//!
//! # Examples
//!
//! ```rust
//! use sorafs_chunker::{ChunkProfile, chunk_bytes};
//!
//! let data = vec![42u8; 600 * 1024];
//! let chunks = chunk_bytes(&data);
//! assert_eq!(chunks.iter().map(|c| c.length).sum::<usize>(), data.len());
//! assert!(
//!     chunks
//!         .iter()
//!         .all(|c| c.length >= ChunkProfile::DEFAULT.min_size)
//! );
//! ```

use std::sync::OnceLock;

use sha3::{Digest, Sha3_256};

/// Rolling hash mask controlling the probability of a boundary for the default profile.
const BREAK_MASK: u64 = 0x0000_FFFF;
/// Rolling hash mask used by the high-density profile (smaller target chunks).
const BREAK_MASK_SF2: u64 = 0x0000_7FFF;

/// Seed tag used when expanding the SHA3-256-based gear table.
const GEAR_TABLE_SEED: &[u8] = b"sorafs-v1-gear";

/// Default chunking profile matching the SF-1 spec.
pub const DEFAULT_PROFILE: ChunkProfile = ChunkProfile {
    min_size: 64 * 1024,
    target_size: 256 * 1024,
    max_size: 512 * 1024,
    break_mask: BREAK_MASK,
};

/// Higher-density profile used by SF-2 (smaller target size, tighter mask).
pub const HIGH_DENSITY_PROFILE: ChunkProfile = ChunkProfile {
    min_size: 32 * 1024,
    target_size: 128 * 1024,
    max_size: 384 * 1024,
    break_mask: BREAK_MASK_SF2,
};

/// Content-defined chunk produced by the chunker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    /// Absolute offset of the chunk relative to the start of the input.
    pub offset: usize,
    /// Length of the chunk in bytes.
    pub length: usize,
}

impl Chunk {
    /// Returns the end offset (`offset + length`) of the chunk.
    #[must_use]
    pub fn end(&self) -> usize {
        self.offset + self.length
    }
}

/// Chunk metadata enriched with a BLAKE3 digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkDigest {
    /// Absolute offset of the chunk relative to the start of the input.
    pub offset: usize,
    /// Length of the chunk in bytes.
    pub length: usize,
    /// 32-byte BLAKE3 digest of the chunk contents.
    pub digest: [u8; 32],
}

/// Chunking configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkProfile {
    /// Minimum chunk size (bytes). Chunks smaller than this only appear at EOF.
    pub min_size: usize,
    /// Target chunk size (bytes). Breaks cluster around this value.
    pub target_size: usize,
    /// Maximum chunk size (bytes). Chunks never exceed this length.
    pub max_size: usize,
    /// Mask applied to the rolling hash; a zero result triggers a boundary.
    pub break_mask: u64,
}

impl ChunkProfile {
    /// Default profile used by SoraFS.
    pub const DEFAULT: Self = DEFAULT_PROFILE;
    /// Higher-density SF-2 profile (smaller chunk sizes for latency-sensitive workloads).
    pub const SF2: Self = HIGH_DENSITY_PROFILE;

    /// Validates that the profile respects ordering and mask requirements.
    fn validate(self) {
        assert!(self.min_size > 0, "min_size must be > 0");
        assert!(
            self.min_size <= self.target_size && self.target_size <= self.max_size,
            "expected min <= target <= max"
        );
        assert!(self.break_mask != 0, "break_mask may not be zero");
    }
}

/// Chunks the provided bytes with the default SoraFS profile.
#[must_use]
pub fn chunk_bytes(input: &[u8]) -> Vec<Chunk> {
    chunk_bytes_with_profile(ChunkProfile::DEFAULT, input)
}

/// Chunks the provided bytes using a custom profile.
#[must_use]
pub fn chunk_bytes_with_profile(profile: ChunkProfile, input: &[u8]) -> Vec<Chunk> {
    profile.validate();
    if input.is_empty() {
        return vec![Chunk {
            offset: 0,
            length: 0,
        }];
    }

    let table = gear_table();
    let mut offset = 0usize;
    let len = input.len();
    let mut chunks = Vec::new();

    while offset < len {
        let max_end = len.min(offset + profile.max_size);
        let mut idx = offset;
        let mut hash = 0u64;

        let must_end = len.min(offset + profile.min_size);

        while idx < must_end {
            hash = roll(hash, input[idx], table);
            idx += 1;
        }

        let mut chunk_end = idx;

        if idx < max_end {
            while idx < max_end {
                hash = roll(hash, input[idx], table);
                idx += 1;
                if (hash & profile.break_mask) == 0 {
                    chunk_end = idx;
                    break;
                }
            }

            if idx == max_end {
                chunk_end = idx;
            }
        } else {
            chunk_end = idx;
        }

        if chunk_end <= offset {
            // Safety net: never emit zero-length chunks.
            chunk_end = (offset + profile.max_size).min(len);
        }

        chunks.push(Chunk {
            offset,
            length: chunk_end - offset,
        });
        offset = chunk_end;
    }

    chunks
}

/// Chunks the input and returns chunk metadata with BLAKE3 digests.
#[must_use]
pub fn chunk_bytes_with_digests(input: &[u8]) -> Vec<ChunkDigest> {
    chunk_bytes_with_digests_profile(ChunkProfile::DEFAULT, input)
}

/// Chunks the input with a custom profile and computes BLAKE3 digests per chunk.
#[must_use]
pub fn chunk_bytes_with_digests_profile(profile: ChunkProfile, input: &[u8]) -> Vec<ChunkDigest> {
    let chunks = chunk_bytes_with_profile(profile, input);
    chunks
        .iter()
        .map(|chunk| {
            let start = chunk.offset;
            let end = chunk.end().min(input.len());
            let digest = blake3::hash(&input[start..end]).into();
            ChunkDigest {
                offset: chunk.offset,
                length: chunk.length,
                digest,
            }
        })
        .collect()
}

#[inline]
fn roll(mut hash: u64, byte: u8, table: &[u64; 256]) -> u64 {
    hash = hash.rotate_left(1);
    hash.wrapping_add(table[byte as usize])
}

fn gear_table() -> &'static [u64; 256] {
    static TABLE: OnceLock<[u64; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u64; 256];
        for (idx, slot) in table.iter_mut().enumerate() {
            let mut hasher = Sha3_256::new();
            hasher.update(GEAR_TABLE_SEED);
            hasher.update((idx as u32).to_le_bytes());
            let digest = hasher.finalize();
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&digest[..8]);
            *slot = u64::from_le_bytes(buf);
        }
        table
    })
}

/// Incremental chunker that streams input and emits deterministic chunk
/// boundaries as data is fed.
#[derive(Debug, Clone, Copy)]
pub struct Chunker {
    profile: ChunkProfile,
    table: &'static [u64; 256],
    offset: usize,
    current_len: usize,
    current_hash: u64,
    chunk_start: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker {
    /// Creates a new chunker configured with the default SoraFS profile.
    #[must_use]
    pub fn new() -> Self {
        Self::with_profile(ChunkProfile::DEFAULT)
    }

    /// Creates a chunker configured with a custom profile.
    #[must_use]
    pub fn with_profile(profile: ChunkProfile) -> Self {
        profile.validate();
        Self {
            profile,
            table: gear_table(),
            offset: 0,
            current_len: 0,
            current_hash: 0,
            chunk_start: 0,
        }
    }

    /// Feeds data into the chunker, invoking `emit` for each completed chunk.
    pub fn feed(&mut self, data: &[u8], mut emit: impl FnMut(Chunk)) {
        for &byte in data {
            self.offset += 1;
            self.current_len += 1;
            self.current_hash = roll(self.current_hash, byte, self.table);

            let min_reached = self.current_len >= self.profile.min_size;
            let max_reached = self.current_len >= self.profile.max_size;
            let matches_mask = (self.current_hash & self.profile.break_mask) == 0;

            if max_reached || (min_reached && matches_mask) {
                let chunk = Chunk {
                    offset: self.chunk_start,
                    length: self.current_len,
                };
                emit(chunk);
                self.chunk_start = self.offset;
                self.current_len = 0;
                self.current_hash = 0;
            }
        }
    }

    /// Flushes any remaining buffered data as the final chunk.
    pub fn finish(&mut self, mut emit: impl FnMut(Chunk)) {
        if self.current_len > 0 {
            let chunk = Chunk {
                offset: self.chunk_start,
                length: self.current_len,
            };
            emit(chunk);
            self.chunk_start = self.offset;
            self.current_len = 0;
            self.current_hash = 0;
        } else if self.offset == 0 {
            emit(Chunk {
                offset: 0,
                length: 0,
            });
        }
    }
}

/// Canonical fixtures for the SoraFS chunker.
pub mod fixtures {
    use sha3::{Digest, Sha3_256};

    use super::{ChunkDigest, ChunkProfile, chunk_bytes_with_digests_profile};

    /// Parameters for the deterministic pseudo-random generator.
    #[derive(Debug, Clone, Copy)]
    pub struct PrngSpec {
        /// Multiplicative factor applied on each step.
        pub multiplier: u64,
        /// Additive offset applied on each step.
        pub increment: u64,
    }

    /// Fixture definition describing how to derive canonical chunking vectors.
    #[derive(Debug, Clone, Copy)]
    pub struct FixtureProfile {
        /// Human-readable profile identifier.
        pub profile_id: &'static str,
        /// Hex-encoded initial state for the PRNG.
        pub input_seed_hex: &'static str,
        /// Number of bytes generated for the fixture input.
        pub input_length: usize,
        /// Deterministic PRNG parameters.
        pub prng: PrngSpec,
        /// Chunking configuration used to derive the vectors.
        pub chunk_profile: ChunkProfile,
    }

    impl FixtureProfile {
        /// Canonical SF1 profile used by SoraFS.
        pub const SF1_V1: Self = Self {
            profile_id: "sorafs.sf1@1.0.0",
            input_seed_hex: "0x0000000000DEC0DED",
            input_length: 1 << 20,
            prng: PrngSpec {
                multiplier: 2_862_933_555_777_941_757,
                increment: 3_037_000_493,
            },
            chunk_profile: ChunkProfile::DEFAULT,
        };

        /// Generates the deterministic input bytes described by the fixture.
        #[must_use]
        pub fn generate_input(self) -> Vec<u8> {
            let mut state = parse_seed(self.input_seed_hex);
            let mut out = Vec::with_capacity(self.input_length);
            for _ in 0..self.input_length {
                state = state
                    .wrapping_mul(self.prng.multiplier)
                    .wrapping_add(self.prng.increment);
                out.push((state >> 32) as u8);
            }
            out
        }

        /// Produces the canonical chunking vectors for this fixture.
        #[must_use]
        pub fn generate_vectors(self) -> FixtureVectors {
            let input = self.generate_input();
            let chunks = chunk_bytes_with_digests_profile(self.chunk_profile, &input);
            let chunk_lengths = chunks.iter().map(|chunk| chunk.length).collect();
            let chunk_offsets = chunks.iter().map(|chunk| chunk.offset).collect();
            let chunk_digests_blake3 = chunks.iter().map(|chunk| chunk.digest).collect();

            let chunk_digest_sha3_256 = sha3_digest(&chunks);

            FixtureVectors {
                profile_id: self.profile_id,
                input_seed_hex: self.input_seed_hex,
                input_length: self.input_length,
                prng: self.prng,
                chunk_profile: self.chunk_profile,
                input,
                chunk_lengths,
                chunk_offsets,
                chunk_digests_blake3,
                chunk_digest_sha3_256,
            }
        }
    }

    /// Fully materialised fixture vectors.
    #[derive(Debug, Clone)]
    pub struct FixtureVectors {
        /// Profile identifier.
        pub profile_id: &'static str,
        /// Hex-encoded initial PRNG state.
        pub input_seed_hex: &'static str,
        /// Number of bytes in the generated input.
        pub input_length: usize,
        /// Deterministic PRNG parameters.
        pub prng: PrngSpec,
        /// Chunking parameters used to derive the fixture.
        pub chunk_profile: ChunkProfile,
        /// Raw input bytes produced by the generator.
        pub input: Vec<u8>,
        /// Chunk lengths in order of appearance.
        pub chunk_lengths: Vec<usize>,
        /// Chunk offsets matching the lengths.
        pub chunk_offsets: Vec<usize>,
        /// BLAKE3 digest per chunk.
        pub chunk_digests_blake3: Vec<[u8; 32]>,
        /// SHA3-256 digest of the `(offset, length)` pairs.
        pub chunk_digest_sha3_256: [u8; 32],
    }

    impl FixtureVectors {
        /// Total number of chunks present in the fixture.
        #[must_use]
        pub fn chunk_count(&self) -> usize {
            self.chunk_lengths.len()
        }

        /// Renders the SHA3-256 digest as a lowercase hexadecimal string.
        #[must_use]
        pub fn sha3_digest_hex(&self) -> String {
            to_hex(&self.chunk_digest_sha3_256)
        }

        /// Renders each chunk BLAKE3 digest as lowercase hexadecimal strings.
        #[must_use]
        pub fn blake3_digest_hexes(&self) -> Vec<String> {
            self.chunk_digests_blake3
                .iter()
                .map(|digest| to_hex(digest))
                .collect()
        }
    }

    fn parse_seed(hex: &str) -> u64 {
        let trimmed = hex.strip_prefix("0x").unwrap_or(hex);
        u64::from_str_radix(trimmed, 16).expect("fixture seed must be valid hex")
    }

    fn sha3_digest(chunks: &[ChunkDigest]) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        for chunk in chunks {
            hasher.update((chunk.offset as u64).to_le_bytes());
            hasher.update((chunk.length as u64).to_le_bytes());
        }
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    /// Converts bytes to a lowercase hexadecimal string.
    #[must_use]
    pub fn to_hex(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for &byte in bytes {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_bytes(len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        let mut state: u64 = 0xBAD5EED;
        for _ in 0..len {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            out.push((state >> 32) as u8);
        }
        out
    }

    #[test]
    fn empty_input_produces_zero_length_chunk() {
        let chunks = chunk_bytes(&[]);
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0],
            Chunk {
                offset: 0,
                length: 0
            }
        );

        let digests = chunk_bytes_with_digests(&[]);
        assert_eq!(digests.len(), 1);
        assert_eq!(digests[0].offset, 0);
        assert_eq!(digests[0].length, 0);
        assert_eq!(digests[0].digest, *blake3::hash(&[]).as_bytes());
    }

    #[test]
    fn small_input_single_chunk() {
        let buf = vec![0u8; 32];
        let chunks = chunk_bytes(&buf);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].length, buf.len());
    }

    #[test]
    fn deterministic_boundaries() {
        let buf = random_bytes(2 * 1024 * 1024);
        let a = chunk_bytes(&buf);
        let b = chunk_bytes(&buf);
        assert_eq!(a, b);
    }

    #[test]
    fn respects_profile_bounds() {
        let buf = random_bytes(3 * 1024 * 1024);
        let chunks = chunk_bytes(&buf);

        assert_eq!(
            chunks.iter().map(|c| c.length).sum::<usize>(),
            buf.len(),
            "chunk lengths should sum to total input length"
        );

        for (idx, chunk) in chunks.iter().enumerate() {
            let is_last = idx == chunks.len() - 1;
            if !is_last {
                assert!(
                    chunk.length >= DEFAULT_PROFILE.min_size,
                    "chunk {} below min: {}",
                    idx,
                    chunk.length
                );
            }

            assert!(
                chunk.length <= DEFAULT_PROFILE.max_size,
                "chunk {} above max: {}",
                idx,
                chunk.length
            );
        }
    }

    #[test]
    fn streaming_matches_batch() {
        let input = random_bytes(3 * 1024 * 1024 + 17);
        let expected = chunk_bytes(&input);

        let mut chunker = Chunker::new();
        let mut actual = Vec::new();

        let mut idx = 0;
        while idx < input.len() {
            let end = (idx + 37_000).min(input.len());
            chunker.feed(&input[idx..end], |chunk| actual.push(chunk));
            idx = end;
        }
        chunker.finish(|chunk| actual.push(chunk));

        assert_eq!(expected, actual);
    }

    #[test]
    fn streaming_emits_zero_length_chunk_when_no_data() {
        let mut chunker = Chunker::new();
        let mut chunks = Vec::new();
        chunker.finish(|chunk| chunks.push(chunk));
        assert_eq!(
            chunks,
            vec![Chunk {
                offset: 0,
                length: 0
            }]
        );
    }

    #[test]
    fn streaming_backpressure_fuzz_matches_batch() {
        let mut state: u64 = 0xC001_FEED;
        for iteration in 0..64 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let len = (state >> 8) as usize % (2 * 1024 * 1024 + 1);
            let input = random_bytes(len);
            let expected = chunk_bytes(&input);

            let mut chunker = Chunker::new();
            let mut actual = Vec::new();
            let mut idx = 0usize;

            while idx < input.len() {
                state = state
                    .wrapping_mul(2862933555777941757)
                    .wrapping_add(3_037_000_493);
                let step = ((state >> 16) as usize % 131_072).max(1);
                let end = (idx + step).min(input.len());
                chunker.feed(&input[idx..end], |chunk| actual.push(chunk));
                idx = end;
            }

            chunker.finish(|chunk| actual.push(chunk));

            assert_eq!(expected, actual, "iteration {iteration}");
        }
    }

    #[test]
    fn digest_matches_manual_hash() {
        let buf = random_bytes(512 * 1024 + 123);
        let digests = chunk_bytes_with_digests(&buf);
        for chunk in chunk_bytes(&buf).iter().zip(digests.iter()) {
            let (plain, digest) = chunk;
            assert_eq!(plain.offset, digest.offset);
            assert_eq!(plain.length, digest.length);
            let expected = blake3::hash(&buf[plain.offset..plain.end()]);
            assert_eq!(expected.as_bytes(), &digest.digest);
        }
    }
}
