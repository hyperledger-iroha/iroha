//! Dependency-free replay of the proof-scoped RNG pinned by the Figure 9 profile.
//!
//! The released profile used `rand` 0.8.5 `StdRng`, which is
//! `rand_chacha` 0.3.1 `ChaCha12Rng` backed by `rand_core` 0.6.4 `BlockRng`.
//! This module ports only that fixed seed-to-byte-stream mapping.  Provenance:
//!
//! - `rand-0.8.5/src/rngs/std.rs` SHA-256
//!   `3cee48bf1fea18b84f585680a947f3aeea949b756cc37d99217291f9759be7c9`;
//! - `rand_chacha-0.3.1/src/chacha.rs` SHA-256
//!   `dfd79ed4762e8267148d1776381c71b898808014a4069cfafbc78177247d5fe9`;
//! - `rand_core-0.6.4/src/block.rs` SHA-256
//!   `c0b606dc404a1f4b25eebf388e9c0da583ee571214cdcb0bac1b592450d6b4fa`;
//! - `rand_core-0.6.4/src/impls.rs` SHA-256
//!   `b861532f8a3500de6bd0e926b3677a15261df4b12d253e4a8fd6acc5e64f1d36`.
//!
//! No ambient entropy path exists.  One fallible external 32-byte seed is
//! health-checked before this owner is constructed, and every later draw comes
//! from its private ChaCha12 stream.

use parking_lot::Mutex;

use super::super::{
    VegaT256ScalarV1 as Scalar,
    engine::{VegaRandomSourceErrorV1, VegaRandomSourceV1},
    sponge::keccak256,
};

const CHACHA_WORDS: usize = 16;
const BUFFER_BLOCKS: usize = 4;
const BUFFER_BYTES: usize = CHACHA_WORDS * BUFFER_BLOCKS * core::mem::size_of::<u32>();
const SIGMA: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

static LAST_PROVER_SEED_DIGEST: Mutex<Option<[u8; 32]>> = Mutex::new(None);

/// Stack-owned secret bytes erased on every exit, including unwind.
struct Figure9SecretBytes<const N: usize>([u8; N]);

impl<const N: usize> Figure9SecretBytes<N> {
    fn as_ref(&self) -> &[u8; N] {
        &self.0
    }

    fn as_mut(&mut self) -> &mut [u8; N] {
        &mut self.0
    }
}

impl<const N: usize> Drop for Figure9SecretBytes<N> {
    fn drop(&mut self) {
        #[cfg(test)]
        let had_secret = self.0.iter().any(|byte| *byte != 0);
        clear_bytes(&mut self.0);
        #[cfg(test)]
        if had_secret && self.0.iter().all(|byte| *byte == 0) {
            let _ = FIGURE9_SECRET_BYTE_ZEROIZED_DROPS.try_with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
    }
}

/// Stack-owned ChaCha words erased on every exit, including unwind.
struct Figure9SecretWords<const N: usize>([u32; N]);

impl<const N: usize> Drop for Figure9SecretWords<N> {
    fn drop(&mut self) {
        #[cfg(test)]
        let had_secret = self.0.iter().any(|word| *word != 0);
        clear_words(&mut self.0);
        #[cfg(test)]
        if had_secret && self.0.iter().all(|word| *word == 0) {
            let _ = FIGURE9_SECRET_WORD_ZEROIZED_DROPS.try_with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
    }
}

/// Failure while establishing the governed Figure 9 proof-scoped RNG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9RandomError {
    /// The injected source could not provide the complete external seed.
    Source(VegaRandomSourceErrorV1),
    /// The seed was constant-byte or immediately reused by another proof.
    DegenerateOrReused,
}

/// Exact `rand` 0.8.5 `StdRng::from_seed` byte stream.
///
/// This owner is intentionally move-only.  Cloning a partially consumed proof
/// stream would make accidental blinding reuse easy.
pub(super) struct Figure9StdRng {
    key: [u32; 8],
    counter: u64,
    buffer: [u8; BUFFER_BYTES],
    cursor: usize,
}

impl Figure9StdRng {
    /// Consume and validate the sole external seed for one proof.
    pub(super) fn from_external<R: VegaRandomSourceV1>(
        random: &mut R,
    ) -> Result<Self, Figure9RandomError> {
        let seed = take_seed_with_guard(random, &LAST_PROVER_SEED_DIGEST)?;
        Ok(Self::from_seed_ref(seed.as_ref()))
    }

    #[cfg(test)]
    pub(super) fn from_seed(seed: [u8; 32]) -> Self {
        let seed = Figure9SecretBytes(seed);
        Self::from_seed_ref(seed.as_ref())
    }

    fn from_seed_ref(seed: &[u8; 32]) -> Self {
        let key = core::array::from_fn(|index| {
            u32::from_le_bytes(
                seed[index * 4..(index + 1) * 4]
                    .try_into()
                    .expect("fixed ChaCha key word"),
            )
        });
        Self {
            key,
            counter: 0,
            buffer: [0; BUFFER_BYTES],
            // `BlockRng::new` starts with its result buffer exhausted.
            cursor: BUFFER_BYTES,
        }
    }

    /// Fill bytes with the exact buffered `ChaCha12Rng` stream.
    pub(super) fn fill_bytes(&mut self, destination: &mut [u8]) {
        let mut written = 0;
        while written < destination.len() {
            if self.cursor == self.buffer.len() {
                self.refill();
            }
            let available = self.buffer.len() - self.cursor;
            let requested = destination.len() - written;
            let take = available.min(requested);
            destination[written..written + take]
                .copy_from_slice(&self.buffer[self.cursor..self.cursor + take]);
            self.cursor += take;
            written += take;
        }
    }

    /// Draw one uniformly reduced T256 scalar exactly as the pinned PCS did.
    pub(super) fn scalar(&mut self) -> Scalar {
        let mut wide = Figure9SecretBytes([0_u8; 64]);
        self.fill_bytes(wide.as_mut());
        Scalar::from_uniform_le_bytes_ref(wide.as_ref())
    }

    fn refill(&mut self) {
        for block in 0..BUFFER_BLOCKS {
            let counter = self.counter.wrapping_add(block as u64);
            let output = chacha12_block(&self.key, counter);
            let start = block * CHACHA_WORDS * core::mem::size_of::<u32>();
            self.buffer[start..start + output.as_ref().len()].copy_from_slice(output.as_ref());
        }
        self.counter = self.counter.wrapping_add(BUFFER_BLOCKS as u64);
        self.cursor = 0;
    }
}

impl Drop for Figure9StdRng {
    fn drop(&mut self) {
        #[cfg(test)]
        let had_state = self.key.iter().any(|word| *word != 0)
            || self.buffer.iter().any(|byte| *byte != 0)
            || self.counter != 0
            || self.cursor != 0;
        clear_words(&mut self.key);
        self.counter = 0;
        clear_bytes(&mut self.buffer);
        self.cursor = 0;
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.counter);
        let _ = core::hint::black_box(&mut self.cursor);
        #[cfg(test)]
        if had_state
            && self.key.iter().all(|word| *word == 0)
            && self.buffer.iter().all(|byte| *byte == 0)
            && self.counter == 0
            && self.cursor == 0
        {
            let _ = FIGURE9_RNG_ZEROIZED_DROPS.try_with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
    }
}

fn take_seed_with_guard<R: VegaRandomSourceV1>(
    random: &mut R,
    previous_digest: &Mutex<Option<[u8; 32]>>,
) -> Result<Figure9SecretBytes<32>, Figure9RandomError> {
    let mut seed = Figure9SecretBytes([0_u8; 32]);
    random
        .fill_bytes(seed.as_mut())
        .map_err(Figure9RandomError::Source)?;
    if seed.as_ref().iter().all(|byte| *byte == seed.as_ref()[0]) {
        return Err(Figure9RandomError::DegenerateOrReused);
    }
    let digest = keccak256(seed.as_ref());
    let mut previous = previous_digest.lock();
    if previous.as_ref() == Some(&digest) {
        return Err(Figure9RandomError::DegenerateOrReused);
    }
    *previous = Some(digest);
    Ok(seed)
}

fn chacha12_block(
    key: &[u32; 8],
    counter: u64,
) -> Figure9SecretBytes<{ CHACHA_WORDS * core::mem::size_of::<u32>() }> {
    let state = Figure9SecretWords([
        SIGMA[0],
        SIGMA[1],
        SIGMA[2],
        SIGMA[3],
        key[0],
        key[1],
        key[2],
        key[3],
        key[4],
        key[5],
        key[6],
        key[7],
        counter as u32,
        (counter >> 32) as u32,
        0,
        0,
    ]);
    let mut working = Figure9SecretWords(state.0);
    // Six double rounds are exactly twelve ChaCha rounds.
    for _ in 0..6 {
        quarter_round(&mut working.0, 0, 4, 8, 12);
        quarter_round(&mut working.0, 1, 5, 9, 13);
        quarter_round(&mut working.0, 2, 6, 10, 14);
        quarter_round(&mut working.0, 3, 7, 11, 15);
        quarter_round(&mut working.0, 0, 5, 10, 15);
        quarter_round(&mut working.0, 1, 6, 11, 12);
        quarter_round(&mut working.0, 2, 7, 8, 13);
        quarter_round(&mut working.0, 3, 4, 9, 14);
    }
    let mut output = Figure9SecretBytes([0_u8; CHACHA_WORDS * core::mem::size_of::<u32>()]);
    for index in 0..CHACHA_WORDS {
        output.0[index * 4..(index + 1) * 4]
            .copy_from_slice(&state.0[index].wrapping_add(working.0[index]).to_le_bytes());
    }
    output
}

fn clear_bytes(bytes: &mut [u8]) {
    let bytes = core::hint::black_box(bytes);
    bytes.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *bytes);
}

fn clear_words(words: &mut [u32]) {
    let words = core::hint::black_box(words);
    words.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *words);
}

#[cfg(test)]
std::thread_local! {
    static FIGURE9_RNG_ZEROIZED_DROPS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
    static FIGURE9_SECRET_BYTE_ZEROIZED_DROPS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
    static FIGURE9_SECRET_WORD_ZEROIZED_DROPS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}

#[cfg(test)]
fn rng_zeroized_drop_count() -> usize {
    FIGURE9_RNG_ZEROIZED_DROPS
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}

#[cfg(test)]
fn secret_byte_zeroized_drop_count() -> usize {
    FIGURE9_SECRET_BYTE_ZEROIZED_DROPS
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}

#[cfg(test)]
fn secret_word_zeroized_drop_count() -> usize {
    FIGURE9_SECRET_WORD_ZEROIZED_DROPS
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}

fn quarter_round(state: &mut [u32; CHACHA_WORDS], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(7);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SeedSource {
        seed: [u8; 32],
        calls: usize,
        fail: bool,
    }

    impl VegaRandomSourceV1 for SeedSource {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1> {
            self.calls += 1;
            if self.fail || destination.len() != self.seed.len() {
                return Err(VegaRandomSourceErrorV1::Unavailable);
            }
            destination.copy_from_slice(&self.seed);
            Ok(())
        }
    }

    fn next_u64(rng: &mut Figure9StdRng) -> u64 {
        let mut bytes = [0_u8; 8];
        rng.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    #[test]
    fn replay_matches_rand_085_stdrng_reference_kat() {
        let seed = [
            1, 0, 0, 0, 23, 0, 0, 0, 200, 1, 0, 0, 210, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0,
        ];
        let mut parent = Figure9StdRng::from_seed(seed);
        let first = next_u64(&mut parent);
        let mut child_seed = [0_u8; 32];
        parent.fill_bytes(&mut child_seed);
        let mut child = Figure9StdRng::from_seed(child_seed);
        let second = next_u64(&mut child);
        assert_eq!(
            [first, second],
            [10_719_222_850_664_546_238, 14_064_965_282_130_556_830]
        );
    }

    #[test]
    fn buffered_chunking_is_stream_identical() {
        let seed = core::array::from_fn(|index| (index as u8).wrapping_mul(7).wrapping_add(3));
        let mut contiguous = Figure9StdRng::from_seed(seed);
        let mut expected = [0_u8; 777];
        contiguous.fill_bytes(&mut expected);

        let mut chunked = Figure9StdRng::from_seed(seed);
        let mut actual = [0_u8; 777];
        let mut offset = 0;
        for length in [1, 31, 224, 3, 256, 17, 245] {
            chunked.fill_bytes(&mut actual[offset..offset + length]);
            offset += length;
        }
        assert_eq!(offset, actual.len());
        assert_eq!(actual, expected);
    }

    #[test]
    fn external_seed_is_single_draw_health_checked_and_immediate_reuse_rejected() {
        let guard = Mutex::new(None);
        let seed = core::array::from_fn(|index| index as u8 + 1);
        let mut first = SeedSource {
            seed,
            calls: 0,
            fail: false,
        };
        let accepted = take_seed_with_guard(&mut first, &guard).expect("fresh seed");
        assert_eq!(accepted.as_ref(), &seed);
        assert_eq!(first.calls, 1);

        let mut repeated = SeedSource {
            seed,
            calls: 0,
            fail: false,
        };
        assert!(matches!(
            take_seed_with_guard(&mut repeated, &guard),
            Err(Figure9RandomError::DegenerateOrReused)
        ));
        assert_eq!(repeated.calls, 1);

        let mut constant = SeedSource {
            seed: [0x42; 32],
            calls: 0,
            fail: false,
        };
        assert!(matches!(
            take_seed_with_guard(&mut constant, &guard),
            Err(Figure9RandomError::DegenerateOrReused)
        ));
        assert_eq!(constant.calls, 1);

        let mut failed = SeedSource {
            seed,
            calls: 0,
            fail: true,
        };
        assert!(matches!(
            take_seed_with_guard(&mut failed, &guard),
            Err(Figure9RandomError::Source(
                VegaRandomSourceErrorV1::Unavailable
            ))
        ));
        assert_eq!(failed.calls, 1);
    }

    #[test]
    fn stack_secret_owners_zeroize_on_success_error_and_unwind() {
        fn error_path() -> Result<(), ()> {
            let _bytes = Figure9SecretBytes([0x51; 8]);
            let _words = Figure9SecretWords([0x5252_5252; 4]);
            Err(())
        }

        let bytes_before_success = secret_byte_zeroized_drop_count();
        let words_before_success = secret_word_zeroized_drop_count();
        drop(Figure9SecretBytes([0x31; 8]));
        drop(Figure9SecretWords([0x3232_3232; 4]));
        assert_eq!(secret_byte_zeroized_drop_count(), bytes_before_success + 1);
        assert_eq!(secret_word_zeroized_drop_count(), words_before_success + 1);

        let bytes_before_error = secret_byte_zeroized_drop_count();
        let words_before_error = secret_word_zeroized_drop_count();
        assert_eq!(error_path(), Err(()));
        assert_eq!(secret_byte_zeroized_drop_count(), bytes_before_error + 1);
        assert_eq!(secret_word_zeroized_drop_count(), words_before_error + 1);

        let bytes_before_unwind = secret_byte_zeroized_drop_count();
        let words_before_unwind = secret_word_zeroized_drop_count();
        let unwind = std::panic::catch_unwind(|| {
            let _bytes = Figure9SecretBytes([0x71; 8]);
            let _words = Figure9SecretWords([0x7272_7272; 4]);
            panic!("injected Figure 9 stack-secret unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(secret_byte_zeroized_drop_count(), bytes_before_unwind + 1);
        assert_eq!(secret_word_zeroized_drop_count(), words_before_unwind + 1);
    }

    #[test]
    fn rng_owner_zeroizes_on_success_error_and_unwind() {
        fn error_path() -> Result<(), ()> {
            let mut rng = Figure9StdRng::from_seed([0x31; 32]);
            let mut byte = [0_u8; 1];
            rng.fill_bytes(&mut byte);
            Err(())
        }

        let before_success = rng_zeroized_drop_count();
        let mut success = Figure9StdRng::from_seed([0x21; 32]);
        let mut byte = [0_u8; 1];
        success.fill_bytes(&mut byte);
        drop(success);
        assert_eq!(rng_zeroized_drop_count(), before_success + 1);

        let before_error = rng_zeroized_drop_count();
        assert_eq!(error_path(), Err(()));
        assert_eq!(rng_zeroized_drop_count(), before_error + 1);

        let before_unwind = rng_zeroized_drop_count();
        let unwind = std::panic::catch_unwind(|| {
            let mut rng = Figure9StdRng::from_seed([0x41; 32]);
            let mut byte = [0_u8; 1];
            rng.fill_bytes(&mut byte);
            panic!("injected Figure 9 RNG unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(rng_zeroized_drop_count(), before_unwind + 1);
    }

    #[test]
    fn source_contract_has_no_rng_dependency_or_clone_boundary() {
        let source = include_str!("rng.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production RNG source");
        assert!(production.contains("rand` 0.8.5 `StdRng"));
        assert!(production.contains("rand_chacha` 0.3.1 `ChaCha12Rng"));
        assert!(production.contains("Six double rounds are exactly twelve ChaCha rounds"));
        assert!(!production.contains("use rand::"));
        assert!(!production.contains("rand_chacha::"));
        assert!(!production.contains("#[derive(Clone)]\npub(super) struct Figure9StdRng"));
        assert!(production.contains("impl Drop for Figure9StdRng"));
        assert!(production.contains("impl<const N: usize> Drop for Figure9SecretBytes<N>"));
        assert!(production.contains("impl<const N: usize> Drop for Figure9SecretWords<N>"));
    }
}
