//! FIPS 202 SHA3-256 and SHAKE256 with clearing state and permutation scratch.
//!
//! These byte primitives do not frame protocol inputs or qualify FASTPQ. Callers
//! must absorb the complete canonical domain/context/body; a cached prefix is an
//! exact sponge-state clone, never a substitute context digest. The implementation
//! has constant resident storage and no internal queued byte allocations.
//! TODO: Qualify deterministic SIMD/device execution before selecting accelerated
//! FASTPQ hashing; existing Poseidon device kernels cannot execute these states.

use core::{
    fmt,
    ops::{Deref, DerefMut},
};
use zeroize::Zeroize;

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

/// Opaque 32-byte SHA3-256 output; every bit string is a valid representation.
///
/// No Goldilocks reduction, Iroha hash marker, truncation, or field serialization
/// is applied. Protocol-specific framing and wire codecs belong to their callers.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha3Digest256V1([u8; 32]);

impl Sha3Digest256V1 {
    /// Fixed number of raw commitment bytes.
    pub const BYTES: usize = 32;

    /// Preserve all 256 input bits without interpreting field elements.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; Self::BYTES]) -> Self {
        Self(bytes)
    }

    /// Borrow the opaque digest bytes in their SHA3 output order.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Self::BYTES] {
        &self.0
    }

    /// Consume this public commitment without changing any bit.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; Self::BYTES] {
        self.0
    }
}

// Every resident state/scratch word has this drop owner. No Debug implementation
// exposes the words, and no method transfers a raw private array out of its guard.
#[derive(Clone)]
struct ClearingWords<const N: usize>([u64; N]);
impl<const N: usize> ClearingWords<N> {
    fn zeroed() -> Self {
        Self([0; N])
    }
}
impl<const N: usize> Deref for ClearingWords<N> {
    type Target = [u64; N];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<const N: usize> DerefMut for ClearingWords<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<const N: usize> Drop for ClearingWords<N> {
    fn drop(&mut self) {
        self.0.zeroize();
        #[cfg(test)]
        observe_cleared(&self.0);
    }
}

#[derive(Clone)]
struct Sponge {
    state: ClearingWords<25>,
    position: usize,
}
impl Sponge {
    fn new() -> Self {
        Self {
            state: ClearingWords::zeroed(),
            position: 0,
        }
    }
    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state[self.position / 8] ^= u64::from(byte) << ((self.position % 8) * 8);
            self.position += 1;
            if self.position == KECCAK_RATE_256_V1 {
                permute(&mut self.state);
                self.position = 0;
            }
        }
    }
    fn finalize(mut self, suffix: u8) -> Reader {
        // Both supported delimited suffixes are below 0x80. If the suffix is at
        // the last rate byte, its terminal padding bit shares that same byte.
        self.state[self.position / 8] ^= u64::from(suffix) << ((self.position % 8) * 8);
        self.state[(KECCAK_RATE_256_V1 - 1) / 8] ^= 0x80_u64 << 56;
        permute(&mut self.state);
        Reader {
            state: self.state,
            position: 0,
        }
    }
}

struct Reader {
    state: ClearingWords<25>,
    position: usize,
}
impl Reader {
    fn read(&mut self, output: &mut [u8]) {
        for byte in output {
            if self.position == KECCAK_RATE_256_V1 {
                permute(&mut self.state);
                self.position = 0;
            }
            *byte = ((self.state[self.position / 8] >> ((self.position % 8) * 8)) & 0xff) as u8;
            self.position += 1;
        }
    }
}

fn permute(state: &mut ClearingWords<25>) {
    // Allocate once per permutation, with unconditional clearing on every exit.
    let mut parity = ClearingWords::<5>::zeroed();
    let mut adjustment = ClearingWords::<5>::zeroed();
    let mut permuted = ClearingWords::<25>::zeroed();
    for round_constant in KECCAK_ROUND_CONSTANTS_V1 {
        for x in 0..5 {
            parity[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        for x in 0..5 {
            adjustment[x] = parity[(x + 4) % 5] ^ parity[(x + 1) % 5].rotate_left(1);
            for y in 0..5 {
                state[x + 5 * y] ^= adjustment[x];
            }
        }
        for x in 0..5 {
            for y in 0..5 {
                permuted[y + 5 * ((2 * x + 3 * y) % 5)] =
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

/// Streaming SHA3-256 state with clearing ownership and redacted diagnostics.
///
/// Cloning preserves the entire absorbed prefix and its partial rate block.
#[derive(Clone)]
pub struct Sha3_256V1(Sponge);
impl Default for Sha3_256V1 {
    fn default() -> Self {
        Self::new()
    }
}
impl Sha3_256V1 {
    /// Exact resident owner size, excluding borrowed caller input/output.
    pub const RETAINED_BYTES: usize = core::mem::size_of::<Self>();
    /// Begin the standard SHA3-256 function.
    #[must_use]
    pub fn new() -> Self {
        Self(Sponge::new())
    }
    /// Absorb exact bytes without allocating or retaining a separate queue.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    /// Consume the state with the SHA3 delimited suffix 0x06.
    #[must_use]
    pub fn finalize(self) -> Sha3Digest256V1 {
        let mut output = [0; 32];
        self.0.finalize(0x06).read(&mut output);
        Sha3Digest256V1::from_bytes(output)
    }
}
impl fmt::Debug for Sha3_256V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sha3_256V1").finish_non_exhaustive()
    }
}

/// Streaming SHAKE256 prefix with clearing ownership and redacted diagnostics.
#[derive(Clone)]
pub struct Shake256V1(Sponge);
impl Default for Shake256V1 {
    fn default() -> Self {
        Self::new()
    }
}
impl Shake256V1 {
    /// Exact resident owner size, excluding borrowed caller input/output.
    pub const RETAINED_BYTES: usize = core::mem::size_of::<Self>();
    /// Begin the standard SHAKE256 function.
    #[must_use]
    pub fn new() -> Self {
        Self(Sponge::new())
    }
    /// Absorb exact bytes without allocating or retaining a separate queue.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    /// Consume the state with the SHAKE delimited suffix 0x1f.
    #[must_use]
    pub fn finalize(self) -> Shake256ReaderV1 {
        Shake256ReaderV1(self.0.finalize(0x1f))
    }
}
impl fmt::Debug for Shake256V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shake256V1").finish_non_exhaustive()
    }
}

/// SHAKE256 output reader; callers own and clear output storage as appropriate.
///
/// This low-level reader also serves public parameter derivation. Protocol
/// transcripts must use one pre-sized whole tape, never adaptive extra squeezes.
pub struct Shake256ReaderV1(Reader);
impl Shake256ReaderV1 {
    /// Exact resident reader size, excluding borrowed caller output.
    pub const RETAINED_BYTES: usize = core::mem::size_of::<Self>();
    /// Fill the exact requested output slice in standard SHAKE byte order.
    pub fn read(&mut self, output: &mut [u8]) {
        self.0.read(output);
    }
}
impl fmt::Debug for Shake256ReaderV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shake256ReaderV1").finish_non_exhaustive()
    }
}

#[cfg(test)]
std::thread_local! {
    static ERASURE: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
fn observe_cleared(words: &[u64]) {
    ERASURE.with(|counts| {
        if let Some((clean, dirty)) = counts.get() {
            let zero = words.iter().filter(|&&word| word == 0).count();
            counts.set(Some((clean + zero, dirty + words.len() - zero)));
        }
    });
}
#[cfg(test)]
#[path = "keccak256/tests.rs"]
mod tests;
