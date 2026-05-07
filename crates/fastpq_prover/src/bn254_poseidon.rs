//! BN254 Poseidon batch helpers for FASTPQ host-side transcript work.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

#[cfg(feature = "fastpq-gpu")]
use tracing::warn;

#[cfg(feature = "fastpq-gpu")]
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
    try_submit_bn254_poseidon_word_batches(words, slices)?.wait()
}

/// Pending BN254 Poseidon word-batch GPU work.
///
/// This is an opaque helper for host code that can submit transcript hashing
/// before it needs the digest bytes. Failed waits disable the accelerated path
/// and return `None` so callers can compute the same batch on the scalar path.
pub struct PendingBn254PoseidonWordBatch {
    inner: PendingBn254PoseidonWordBatchInner,
}

enum PendingBn254PoseidonWordBatchInner {
    Ready(Vec<[u8; 32]>),
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    Metal(crate::metal::PendingBn254PoseidonWords),
}

impl PendingBn254PoseidonWordBatch {
    /// Wait for completion and return digest bytes, or `None` when the GPU path failed.
    #[must_use]
    #[cfg_attr(
        not(all(feature = "fastpq-gpu", target_os = "macos")),
        expect(
            clippy::unnecessary_wraps,
            reason = "the public wait API stays fallible because Metal submissions can fail after being accepted"
        )
    )]
    pub fn wait(self) -> Option<Vec<[u8; 32]>> {
        match self.inner {
            PendingBn254PoseidonWordBatchInner::Ready(result) => Some(result),
            #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
            PendingBn254PoseidonWordBatchInner::Metal(pending) => match pending.wait() {
                Ok(result) => Some(result),
                Err(error) => {
                    warn!(
                        target: "fastpq::bn254_poseidon",
                        %error,
                        "BN254 Poseidon Metal batch failed while waiting; falling back to scalar hashing"
                    );
                    disable_bn254_poseidon_gpu();
                    None
                }
            },
        }
    }
}

/// Try to submit flattened BN254 Poseidon word batches on the configured GPU backend.
///
/// Returns `None` when the accelerator is unavailable, disabled, or fails validation.
#[must_use]
pub fn try_submit_bn254_poseidon_word_batches(
    words: &[u64],
    slices: &[Bn254PoseidonBatchSlice],
) -> Option<PendingBn254PoseidonWordBatch> {
    if slices.is_empty() {
        return Some(PendingBn254PoseidonWordBatch {
            inner: PendingBn254PoseidonWordBatchInner::Ready(Vec::new()),
        });
    }
    if BN254_POSEIDON_GPU_DISABLED.load(Ordering::Acquire) {
        return None;
    }
    if !bn254_poseidon_backend_available() {
        return None;
    }
    if !bn254_poseidon_self_test_passed() {
        return None;
    }

    try_submit_bn254_poseidon_word_batches_impl(words, slices).or_else(|| {
        disable_bn254_poseidon_gpu();
        None
    })
}

/// Preflight the configured BN254 Poseidon word-batch accelerator.
///
/// This performs the same backend discovery and parity self-test that the first
/// real batch would otherwise do. It returns `false` when the GPU path is not
/// available and preserves the normal scalar fallback behavior.
#[must_use]
pub fn preflight_bn254_poseidon_word_batches() -> bool {
    if BN254_POSEIDON_GPU_DISABLED.load(Ordering::Acquire) {
        return false;
    }
    if !bn254_poseidon_backend_available() {
        return false;
    }
    if bn254_poseidon_self_test_passed() {
        true
    } else {
        disable_bn254_poseidon_gpu();
        false
    }
}

#[cfg(feature = "fastpq-gpu")]
fn try_submit_bn254_poseidon_word_batches_impl(
    words: &[u64],
    slices: &[Bn254PoseidonBatchSlice],
) -> Option<PendingBn254PoseidonWordBatch> {
    match backend::current_gpu_backend() {
        Some(GpuBackend::Cuda) => {
            match crate::fastpq_cuda::fastpq_bn254_poseidon_hash_words(words, slices) {
                Ok(result) => Some(PendingBn254PoseidonWordBatch {
                    inner: PendingBn254PoseidonWordBatchInner::Ready(result),
                }),
                Err(error) => {
                    warn!(
                        target: "fastpq::bn254_poseidon",
                        %error,
                        "BN254 Poseidon CUDA batch failed; falling back to scalar hashing"
                    );
                    None
                }
            }
        }
        #[cfg(target_os = "macos")]
        Some(GpuBackend::Metal) => {
            match crate::metal::bn254_poseidon_hash_words_async(words, slices) {
                Ok(pending) => Some(PendingBn254PoseidonWordBatch {
                    inner: PendingBn254PoseidonWordBatchInner::Metal(pending),
                }),
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
        _ => None,
    }
}

#[cfg(not(feature = "fastpq-gpu"))]
fn try_submit_bn254_poseidon_word_batches_impl(
    _words: &[u64],
    _slices: &[Bn254PoseidonBatchSlice],
) -> Option<PendingBn254PoseidonWordBatch> {
    None
}

fn disable_bn254_poseidon_gpu() {
    BN254_POSEIDON_GPU_DISABLED.store(true, Ordering::Release);
}

#[cfg(feature = "fastpq-gpu")]
fn bn254_poseidon_backend_available() -> bool {
    match backend::current_gpu_backend() {
        Some(GpuBackend::Cuda) => true,
        #[cfg(target_os = "macos")]
        Some(GpuBackend::Metal) => true,
        _ => false,
    }
}

#[cfg(not(feature = "fastpq-gpu"))]
fn bn254_poseidon_backend_available() -> bool {
    false
}

fn bn254_poseidon_self_test_passed() -> bool {
    *BN254_POSEIDON_SELF_TEST.get_or_init(run_bn254_poseidon_self_test)
}

#[cfg(feature = "fastpq-gpu")]
fn run_bn254_poseidon_self_test() -> bool {
    let Some(backend) = backend::current_gpu_backend() else {
        return false;
    };

    let (words, slices) = bn254_poseidon_self_test_batch();

    let actual = match backend {
        GpuBackend::Cuda => crate::fastpq_cuda::fastpq_bn254_poseidon_hash_words(&words, &slices)
            .map_err(|error| error.to_string()),
        #[cfg(target_os = "macos")]
        GpuBackend::Metal => crate::metal::bn254_poseidon_hash_words(&words, &slices)
            .map_err(|error| error.to_string()),
        other => {
            warn!(
                target: "fastpq::bn254_poseidon",
                backend = other.as_str(),
                "BN254 Poseidon GPU self-test skipped for unsupported backend"
            );
            return false;
        }
    };
    match actual {
        Ok(actual) => {
            let expected = expected_bn254_poseidon_word_hashes(&words, &slices);
            let passed = actual == expected;
            if !passed {
                let mismatch = first_bn254_poseidon_mismatch(&expected, &actual);
                warn!(
                    target: "fastpq::bn254_poseidon",
                    backend = backend.as_str(),
                    ?mismatch,
                    "BN254 Poseidon GPU self-test mismatch; falling back to scalar hashing"
                );
            }
            passed
        }
        Err(error) => {
            warn!(
                target: "fastpq::bn254_poseidon",
                %error,
                backend = backend.as_str(),
                "BN254 Poseidon GPU self-test failed; falling back to scalar hashing"
            );
            false
        }
    }
}

#[cfg(any(test, feature = "fastpq-gpu"))]
fn bn254_poseidon_self_test_batch() -> (Vec<u64>, Vec<Bn254PoseidonBatchSlice>) {
    let cases: [&[u64]; 10] = [
        &[],
        &[0],
        &[1],
        &[u64::MAX],
        &[1, 2],
        &[1, 2, 3],
        &[3, 5, 8, 13, 21, u64::MAX - 7],
        &[u64::MAX, u64::MAX - 1, u64::MAX - 2, u64::MAX - 3],
        &[0, u64::MAX, 1, u64::MAX - 1, 2],
        &[42; 17],
    ];
    let mut words = Vec::new();
    let mut slices = Vec::with_capacity(cases.len());
    for case in cases {
        let offset = words.len();
        words.extend_from_slice(case);
        slices.push(Bn254PoseidonBatchSlice::new(offset, case.len()));
    }
    (words, slices)
}

#[cfg(any(test, feature = "fastpq-gpu"))]
fn expected_bn254_poseidon_word_hashes(
    words: &[u64],
    slices: &[Bn254PoseidonBatchSlice],
) -> Vec<[u8; 32]> {
    slices
        .iter()
        .map(|slice| scalar_hash_words_u64(&words[slice.offset()..][..slice.len()]))
        .collect()
}

#[cfg(feature = "fastpq-gpu")]
fn first_bn254_poseidon_mismatch(
    expected: &[[u8; 32]],
    actual: &[[u8; 32]],
) -> Option<(usize, [u8; 32], Option<[u8; 32]>)> {
    let mismatch = expected
        .iter()
        .zip(actual.iter().map(Some).chain(core::iter::repeat(None)))
        .enumerate()
        .find_map(|(idx, (expected, actual))| {
            (actual != Some(expected)).then_some((idx, *expected, actual.copied()))
        });
    mismatch.or_else(|| {
        actual
            .get(expected.len())
            .map(|actual| (expected.len(), [0; 32], Some(*actual)))
    })
}

#[cfg(not(feature = "fastpq-gpu"))]
fn run_bn254_poseidon_self_test() -> bool {
    false
}

#[cfg(any(test, feature = "fastpq-gpu"))]
fn scalar_hash_words_u64(words: &[u64]) -> [u8; 32] {
    iroha_zkp_halo2::poseidon::hash_u64_words_bytes(words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2curves::bn256::Fr as Bn254Fr;

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn metal_backend_selected() -> bool {
        crate::backend::current_gpu_backend() == Some(crate::backend::GpuBackend::Metal)
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn generated_word_batch(batch_count: usize) -> (Vec<u64>, Vec<Bn254PoseidonBatchSlice>) {
        let mut words = Vec::new();
        let mut slices = Vec::with_capacity(batch_count);
        for idx in 0..batch_count {
            let len = match idx % 11 {
                0 => 0,
                1 => 1,
                2 => 2,
                3 => 3,
                4 => 5,
                5 => 17,
                6 => 18,
                7 => 23,
                8 => 31,
                9 => 64,
                _ => 7,
            };
            let offset = words.len();
            for word in 0..len {
                let seed = ((idx as u64) << 32) ^ word as u64;
                words.push(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            }
            if len > 0 && idx % 13 == 0 {
                words[offset] = u64::MAX;
            }
            slices.push(Bn254PoseidonBatchSlice::new(offset, len));
        }
        (words, slices)
    }

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

    #[test]
    fn bn254_poseidon_self_test_batch_covers_edge_shapes() {
        let (words, slices) = bn254_poseidon_self_test_batch();
        assert_eq!(slices.len(), 10);
        assert!(slices.iter().any(|slice| slice.is_empty()));
        assert!(slices.iter().any(|slice| slice.len() > 16));
        assert!(words.contains(&u64::MAX));
        assert_eq!(
            expected_bn254_poseidon_word_hashes(&words, &slices).len(),
            slices.len()
        );
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    #[test]
    fn metal_bn254_poseidon_word_batch_matches_cpu_self_test_cases() {
        if !metal_backend_selected() {
            return;
        }
        let (words, slices) = bn254_poseidon_self_test_batch();
        let actual = crate::metal::bn254_poseidon_hash_words(&words, &slices)
            .expect("Metal BN254 Poseidon word batch should run");
        let expected = expected_bn254_poseidon_word_hashes(&words, &slices);
        assert_eq!(actual, expected);
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    #[test]
    fn metal_bn254_poseidon_word_batches_match_cpu_large_shapes() {
        if !metal_backend_selected() {
            return;
        }
        for batch_count in [64, 128, 512, 1_024] {
            let (words, slices) = generated_word_batch(batch_count);
            let actual = crate::metal::bn254_poseidon_hash_words(&words, &slices)
                .expect("Metal BN254 Poseidon word batch should run");
            let expected = expected_bn254_poseidon_word_hashes(&words, &slices);
            assert_eq!(
                actual, expected,
                "Metal BN254 Poseidon mismatch for batch_count={batch_count}"
            );
        }
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    #[test]
    fn metal_bn254_poseidon_repeated_word_batches_are_stable() {
        if !metal_backend_selected() {
            return;
        }
        let (words, slices) = generated_word_batch(512);
        let expected = expected_bn254_poseidon_word_hashes(&words, &slices);
        for iteration in 0..8 {
            let actual = crate::metal::bn254_poseidon_hash_words(&words, &slices)
                .expect("repeated Metal BN254 Poseidon word batch should run");
            assert_eq!(
                actual, expected,
                "Metal BN254 Poseidon output drifted on iteration {iteration}"
            );
        }
    }

    #[cfg(not(feature = "fastpq-gpu"))]
    #[test]
    fn preflight_reports_unavailable_without_gpu_feature() {
        assert!(!preflight_bn254_poseidon_word_batches());
    }
}
