//! BN254 Poseidon batch helpers for FASTPQ host-side transcript work.

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

#[cfg(any(test, all(feature = "fastpq-gpu", target_os = "macos")))]
use halo2curves::bn256::Fr as Bn254Fr;
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use tracing::warn;

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use crate::backend::{self, GpuBackend};

/// Offset metadata for one BN254 Poseidon word-hash input inside a flattened word buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bn254PoseidonBatchSlice {
    offset: usize,
    len: usize,
}

impl Bn254PoseidonBatchSlice {
    /// Create a slice from a flattened word offset and word length.
    #[must_use]
    pub const fn new(offset: usize, len: usize) -> Self {
        Self { offset, len }
    }

    /// Return the first word index in the flattened input buffer.
    #[must_use]
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Return the number of words belonging to this input.
    #[must_use]
    pub const fn len(self) -> usize {
        self.len
    }

    /// Return true when this slice has no words.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

static BN254_POSEIDON_GPU_DISABLED: AtomicBool = AtomicBool::new(false);
static BN254_POSEIDON_SELF_TEST: OnceLock<bool> = OnceLock::new();

/// Try to hash flattened BN254 Poseidon word batches with the configured GPU backend.
///
/// The input words are interpreted exactly like `iroha_zkp_halo2::poseidon::hash_words_bytes`:
/// each `u64` becomes a BN254 field element, then Poseidon rate padding is applied by the
/// backend. Returns `None` when the accelerator is unavailable, disabled, or fails validation.
#[must_use]
pub fn try_hash_bn254_poseidon_word_batches(
    words: &[u64],
    slices: &[Bn254PoseidonBatchSlice],
) -> Option<Vec<[u8; 32]>> {
    if slices.is_empty() {
        return Some(Vec::new());
    }
    if BN254_POSEIDON_GPU_DISABLED.load(Ordering::Acquire) {
        return None;
    }
    if !bn254_poseidon_self_test_passed() {
        return None;
    }

    try_hash_bn254_poseidon_word_batches_impl(words, slices).or_else(|| {
        BN254_POSEIDON_GPU_DISABLED.store(true, Ordering::Release);
        None
    })
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn try_hash_bn254_poseidon_word_batches_impl(
    words: &[u64],
    slices: &[Bn254PoseidonBatchSlice],
) -> Option<Vec<[u8; 32]>> {
    if backend::current_gpu_backend() != Some(GpuBackend::Metal) {
        return None;
    }
    match crate::metal::bn254_poseidon_hash_words(words, slices) {
        Ok(result) => Some(result),
        Err(error) => {
            warn!(
                target: "fastpq::bn254_poseidon",
                %error,
                "BN254 Poseidon Metal batch failed; falling back to scalar hashing"
            );
            None
        }
    }
}

#[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
fn try_hash_bn254_poseidon_word_batches_impl(
    _words: &[u64],
    _slices: &[Bn254PoseidonBatchSlice],
) -> Option<Vec<[u8; 32]>> {
    None
}

fn bn254_poseidon_self_test_passed() -> bool {
    *BN254_POSEIDON_SELF_TEST.get_or_init(run_bn254_poseidon_self_test)
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn run_bn254_poseidon_self_test() -> bool {
    if backend::current_gpu_backend() != Some(GpuBackend::Metal) {
        return false;
    }

    let cases: [&[u64]; 6] = [
        &[],
        &[0],
        &[1],
        &[u64::MAX],
        &[1, 2],
        &[3, 5, 8, 13, 21, u64::MAX - 7],
    ];
    let mut words = Vec::new();
    let mut slices = Vec::with_capacity(cases.len());
    for case in cases {
        let offset = words.len();
        words.extend_from_slice(case);
        slices.push(Bn254PoseidonBatchSlice::new(offset, case.len()));
    }

    match crate::metal::bn254_poseidon_hash_words(&words, &slices) {
        Ok(actual) => {
            let expected = slices
                .iter()
                .map(|slice| scalar_hash_words_u64(&words[slice.offset()..][..slice.len()]))
                .collect::<Vec<_>>();
            let passed = actual == expected;
            if !passed {
                warn!(
                    target: "fastpq::bn254_poseidon",
                    "BN254 Poseidon Metal self-test mismatch; falling back to scalar hashing"
                );
            }
            passed
        }
        Err(error) => {
            warn!(
                target: "fastpq::bn254_poseidon",
                %error,
                "BN254 Poseidon Metal self-test failed; falling back to scalar hashing"
            );
            false
        }
    }
}

#[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
fn run_bn254_poseidon_self_test() -> bool {
    false
}

#[cfg(any(test, all(feature = "fastpq-gpu", target_os = "macos")))]
fn scalar_hash_words_u64(words: &[u64]) -> [u8; 32] {
    let words = words.iter().copied().map(Bn254Fr::from).collect::<Vec<_>>();
    iroha_zkp_halo2::poseidon::hash_words_bytes(&words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_slice_reports_offsets_and_lengths() {
        let slice = Bn254PoseidonBatchSlice::new(7, 3);
        assert_eq!(slice.offset(), 7);
        assert_eq!(slice.len(), 3);
        assert!(!slice.is_empty());
        assert!(Bn254PoseidonBatchSlice::new(1, 0).is_empty());
    }

    #[test]
    fn scalar_word_hash_matches_poseidon_word_path() {
        let words = [1, 2, 3, u64::MAX];
        let fr_words = words.iter().copied().map(Bn254Fr::from).collect::<Vec<_>>();
        assert_eq!(
            scalar_hash_words_u64(&words),
            iroha_zkp_halo2::poseidon::hash_words_bytes(&fr_words)
        );
    }
}
