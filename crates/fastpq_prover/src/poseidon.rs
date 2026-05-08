//! Poseidon hashing backends for FASTPQ.
//!
//! The prover routes all Poseidon usage (commitments, lookup witnesses,
//! transcripts) through this module so we can transparently switch between the
//! canonical scalar implementation and accelerator-backed permutations.  CUDA
//! kernels mirror the exact permutation used by `fastpq_isi`, and hosts without
//! GPU support automatically fall back to the CPU path while keeping the API
//! stable.

use std::sync::OnceLock;

/// Goldilocks field modulus (2^64 - 2^32 + 1).
pub use cpu::FIELD_MODULUS;
#[cfg(feature = "fastpq-gpu")]
use fastpq_isi::poseidon::STATE_WIDTH;
use fastpq_isi::poseidon::{self as cpu, PoseidonSponge as CpuPoseidonSponge};
#[cfg(feature = "fastpq-gpu")]
use {
    crate::backend::{self, GpuBackend},
    crate::fastpq_cuda,
    std::sync::atomic::{AtomicBool, Ordering},
    tracing::warn,
};

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use crate::metal;

#[cfg(feature = "fastpq-gpu")]
static POSEIDON_GPU_DISABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_GPU_SELF_TEST: OnceLock<bool> = OnceLock::new();

/// Trait describing a Poseidon backend.
pub trait PoseidonBackend: Send + Sync {
    /// Hash the provided field elements with the Poseidon permutation.
    fn hash_field_elements(&self, elements: &[u64]) -> u64;

    /// Create a new sponge instance backed by this implementation.
    fn new_sponge(&self) -> Box<dyn PoseidonSpongeCore>;
}

/// Trait describing a Poseidon sponge instance.
pub trait PoseidonSpongeCore: Send {
    /// Absorb a single field element into the sponge.
    fn absorb(&mut self, element: u64);
    /// Absorb a slice of field elements into the sponge.
    fn absorb_slice(&mut self, elements: &[u64]);
    #[allow(dead_code)]
    /// Squeeze a single field element from the sponge.
    fn squeeze_element(&mut self) -> u64;
    /// Finalise the sponge and return the first output element.
    fn squeeze(self: Box<Self>) -> u64;
}

#[derive(Clone, Default)]
struct CpuPoseidonBackend;

impl PoseidonBackend for CpuPoseidonBackend {
    fn hash_field_elements(&self, elements: &[u64]) -> u64 {
        cpu::hash_field_elements(elements)
    }

    fn new_sponge(&self) -> Box<dyn PoseidonSpongeCore> {
        Box::new(CpuSponge(CpuPoseidonSponge::new()))
    }
}

struct CpuSponge(CpuPoseidonSponge);

impl PoseidonSpongeCore for CpuSponge {
    fn absorb(&mut self, element: u64) {
        self.0.absorb(element);
    }

    fn absorb_slice(&mut self, elements: &[u64]) {
        self.0.absorb_slice(elements);
    }

    fn squeeze_element(&mut self) -> u64 {
        self.0.squeeze_element()
    }

    fn squeeze(self: Box<Self>) -> u64 {
        self.0.squeeze()
    }
}

#[cfg(feature = "fastpq-gpu")]
#[derive(Clone)]
struct GpuPoseidonBackend {
    fallback: CpuPoseidonBackend,
}

#[cfg(feature = "fastpq-gpu")]
impl GpuPoseidonBackend {
    fn new(_accelerator: GpuBackend) -> Self {
        Self {
            fallback: CpuPoseidonBackend,
        }
    }
}

#[cfg(feature = "fastpq-gpu")]
impl PoseidonBackend for GpuPoseidonBackend {
    fn hash_field_elements(&self, elements: &[u64]) -> u64 {
        self.fallback.hash_field_elements(elements)
    }

    fn new_sponge(&self) -> Box<dyn PoseidonSpongeCore> {
        self.fallback.new_sponge()
    }
}

fn backend() -> &'static dyn PoseidonBackend {
    static BACKEND: OnceLock<Box<dyn PoseidonBackend>> = OnceLock::new();
    BACKEND
        .get_or_init(|| {
            #[cfg(feature = "fastpq-gpu")]
            if !POSEIDON_GPU_DISABLED.load(Ordering::Acquire)
                && let Some(accelerator) = backend::current_gpu_backend()
            {
                return Box::new(GpuPoseidonBackend::new(accelerator));
            }
            Box::new(CpuPoseidonBackend)
        })
        .as_ref()
}

/// Preflight the configured Poseidon GPU backend used by the prover path.
///
/// The preflight performs backend discovery and a tiny deterministic
/// `poseidon_permute` parity check against the scalar implementation. A failed
/// self-test disables the accelerated Poseidon path for this process so later
/// prover work keeps the existing CPU fallback behavior.
#[cfg(feature = "fastpq-gpu")]
#[must_use]
pub fn preflight_gpu_backend() -> bool {
    if POSEIDON_GPU_DISABLED.load(Ordering::Acquire) {
        return false;
    }
    let Some(backend) = backend::current_gpu_backend() else {
        return false;
    };
    if *POSEIDON_GPU_SELF_TEST.get_or_init(|| run_poseidon_gpu_self_test(backend)) {
        true
    } else {
        POSEIDON_GPU_DISABLED.store(true, Ordering::Release);
        false
    }
}

#[cfg(feature = "fastpq-gpu")]
fn run_poseidon_gpu_self_test(backend: GpuBackend) -> bool {
    let inputs = [
        [0u64, 1, 2],
        [
            0x0123_4567_89ab_cdef,
            0xfedc_ba98_7654_3210,
            0x0f0f_f0f0_aaaa_5555,
        ],
    ];
    let mut expected = inputs;
    for state in &mut expected {
        cpu::permute_state(state);
    }
    let expected = expected.iter().flatten().copied().collect::<Vec<_>>();
    let mut actual = inputs.into_iter().flatten().collect::<Vec<_>>();

    let result = {
        let _guard = backend::acquire_gpu_lane();
        match backend {
            GpuBackend::Cuda => fastpq_cuda::fastpq_poseidon_permute(actual.as_mut_slice())
                .map_err(|err| err.to_string()),
            #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
            GpuBackend::Metal => {
                metal::poseidon_permute(actual.as_mut_slice()).map_err(|err| err.to_string())
            }
            other => Err(format!(
                "{other:?} backend unsupported for Poseidon preflight"
            )),
        }
    };

    match result {
        Ok(()) if actual == expected => true,
        Ok(()) => {
            let (mismatch_state, mismatch_lane, expected_value, actual_value) =
                first_poseidon_state_mismatch(&expected, &actual).unwrap_or((0, 0, 0, None));
            warn!(
                target: "fastpq::poseidon",
                backend = backend.as_str(),
                mismatch_state,
                mismatch_lane,
                expected = expected_value,
                actual = ?actual_value,
                "FASTPQ Poseidon GPU preflight produced a CPU parity mismatch; falling back to CPU"
            );
            false
        }
        Err(error) => {
            warn!(
                target: "fastpq::poseidon",
                %error,
                backend = backend.as_str(),
                "FASTPQ Poseidon GPU preflight failed; falling back to CPU"
            );
            false
        }
    }
}

#[cfg(feature = "fastpq-gpu")]
fn first_poseidon_state_mismatch(
    expected: &[u64],
    actual: &[u64],
) -> Option<(usize, usize, u64, Option<u64>)> {
    let mismatch = expected
        .iter()
        .zip(actual.iter().map(Some).chain(core::iter::repeat(None)))
        .enumerate()
        .find_map(|(idx, (expected, actual))| {
            (actual != Some(expected)).then_some((
                idx / STATE_WIDTH,
                idx % STATE_WIDTH,
                *expected,
                actual.copied(),
            ))
        });
    mismatch.or_else(|| {
        actual.get(expected.len()).map(|actual| {
            (
                expected.len() / STATE_WIDTH,
                expected.len() % STATE_WIDTH,
                0,
                Some(*actual),
            )
        })
    })
}

/// Hash the provided field elements with the active Poseidon backend.
#[must_use]
pub fn hash_field_elements(elements: &[u64]) -> u64 {
    backend().hash_field_elements(elements)
}

/// Hash the provided field elements with the canonical scalar Poseidon backend.
#[must_use]
pub(crate) fn hash_field_elements_cpu(elements: &[u64]) -> u64 {
    cpu::hash_field_elements(elements)
}

/// Create a new Poseidon sponge backed by the active backend.
pub struct PoseidonSponge {
    inner: Box<dyn PoseidonSpongeCore>,
}

impl PoseidonSponge {
    /// Construct a sponge using the active backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: backend().new_sponge(),
        }
    }

    /// Absorb a single field element into the sponge.
    pub fn absorb(&mut self, element: u64) {
        self.inner.absorb(element);
    }

    /// Absorb a slice of field elements into the sponge.
    pub fn absorb_slice(&mut self, elements: &[u64]) {
        self.inner.absorb_slice(elements);
    }

    /// Squeeze a field element while keeping the sponge ready for the next output.
    #[allow(dead_code)]
    #[must_use]
    pub fn squeeze_element(&mut self) -> u64 {
        self.inner.squeeze_element()
    }

    /// Finalise the sponge and return the first output element.
    #[must_use]
    pub fn squeeze(self) -> u64 {
        self.inner.squeeze()
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
    fn hash_matches_cpu_reference() {
        let inputs = [1u64, 2, 3, 4];
        let cpu_digest = cpu::hash_field_elements(&inputs);
        assert_eq!(hash_field_elements(&inputs), cpu_digest);
    }

    #[test]
    fn sponge_roundtrip_matches_cpu_reference() {
        let mut cpu_sponge = cpu::PoseidonSponge::new();
        cpu_sponge.absorb_slice(&[10, 20, 30]);
        let mut backend_sponge = PoseidonSponge::new();
        backend_sponge.absorb_slice(&[10, 20, 30]);

        let cpu_first = cpu_sponge.squeeze_element();
        let backend_first = backend_sponge.squeeze_element();
        assert_eq!(cpu_first, backend_first);

        let cpu_second = cpu_sponge.squeeze_element();
        let backend_second = backend_sponge.squeeze_element();
        assert_eq!(cpu_second, backend_second);
    }

    #[cfg(all(test, feature = "fastpq-gpu"))]
    #[test]
    fn preflight_gpu_backend_returns_safely() {
        if crate::backend::current_gpu_backend().is_none() {
            return;
        }
        assert!(
            super::preflight_gpu_backend(),
            "Poseidon GPU preflight must pass when a GPU backend is available"
        );
    }

    #[cfg(all(test, feature = "fastpq-gpu"))]
    #[test]
    fn gpu_poseidon_preflight_passes_after_bn254_digest_preflight() {
        if crate::backend::current_gpu_backend().is_none() {
            return;
        }
        assert!(
            crate::preflight_bn254_poseidon_word_batches(),
            "BN254 digest preflight must pass before checking prover Poseidon independence"
        );
        assert!(
            super::preflight_gpu_backend(),
            "Poseidon prover preflight must remain independent after BN254 digest preflight"
        );
    }

    #[cfg(all(test, feature = "fastpq-gpu"))]
    fn run_gpu_poseidon_permute(states: &mut [u64]) -> Result<(), String> {
        use crate::backend;
        match backend::current_gpu_backend() {
            Some(backend::GpuBackend::Cuda) => {
                fastpq_cuda::fastpq_poseidon_permute(states).map_err(|err| err.to_string())
            }
            #[cfg(target_os = "macos")]
            Some(backend::GpuBackend::Metal) => match crate::metal::poseidon_permute(states) {
                Ok(()) => Ok(()),
                Err(crate::gpu::GpuError::Unsupported(_)) => Err("GPU backend unavailable".into()),
                Err(err) => Err(err.to_string()),
            },
            _ => Err("GPU backend unavailable".into()),
        }
    }

    #[cfg(all(test, feature = "fastpq-gpu"))]
    #[test]
    fn gpu_poseidon_matches_cpu_for_single_state() {
        let mut cpu_state = [1u64, 2, 3];
        cpu::permute_state(&mut cpu_state);

        let mut gpu_state = [1u64, 2, 3];
        match run_gpu_poseidon_permute(gpu_state.as_mut_slice()) {
            Ok(()) => assert_eq!(
                gpu_state, cpu_state,
                "Poseidon GPU permutation diverged from CPU reference"
            ),
            Err(message) => {
                if message == "GPU backend unavailable" {
                    eprintln!("GPU backend unavailable; skipping Poseidon parity check");
                } else {
                    panic!("Poseidon GPU permutation failed: {message}");
                }
            }
        }
    }

    #[cfg(all(test, feature = "fastpq-gpu"))]
    #[test]
    fn gpu_poseidon_matches_cpu_for_batched_states() {
        let inputs = [
            [0u64, 1, 2],
            [3, 4, 5],
            [u64::MAX, u64::MAX, u64::MAX],
            [
                0x0123_4567_89ab_cdef,
                0xfedc_ba98_7654_3210,
                0x0f0f_f0f0_aaaa_5555,
            ],
        ];
        let mut cpu_outputs = inputs;
        for state in &mut cpu_outputs {
            cpu::permute_state(state);
        }
        let expected: Vec<u64> = cpu_outputs.iter().flatten().copied().collect();
        let mut gpu_inputs: Vec<u64> = inputs.into_iter().flatten().collect();

        match run_gpu_poseidon_permute(gpu_inputs.as_mut_slice()) {
            Ok(()) => assert_eq!(
                gpu_inputs, expected,
                "Batched Poseidon GPU permutation diverged from CPU reference"
            ),
            Err(message) => {
                if message == "GPU backend unavailable" {
                    eprintln!("GPU backend unavailable; skipping batched Poseidon parity check");
                } else {
                    panic!("Poseidon GPU batched permutation failed: {message}");
                }
            }
        }
    }
}
