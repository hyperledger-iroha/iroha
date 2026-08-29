//! Poseidon hashing backends for FASTPQ.
//!
//! Scalar hashes and sponges use the canonical `fastpq_isi` implementation
//! directly. Wide, independent batches are accelerated by the dedicated GPU
//! paths in `trace` and `gpu`; routing a single sponge through a dynamic backend
//! only added allocation and dispatch overhead because that path was always
//! scalar.
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use crate::metal;
/// Goldilocks field modulus (2^64 - 2^32 + 1) and canonical scalar sponge.
pub use cpu::{FIELD_MODULUS, PoseidonSponge};
use fastpq_isi::poseidon as cpu;
#[cfg(feature = "fastpq-gpu")]
use fastpq_isi::poseidon::STATE_WIDTH;
#[cfg(feature = "fastpq-gpu")]
use {
    crate::backend::{self, GpuBackend},
    crate::fastpq_cuda,
    std::sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    tracing::warn,
};
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_GPU_DISABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "fastpq-gpu")]
static POSEIDON_GPU_SELF_TEST: OnceLock<bool> = OnceLock::new();
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
/// Hash field elements with the canonical scalar Poseidon implementation.
///
/// Independent batches use the explicit GPU helpers instead of paying dynamic
/// dispatch overhead for this small-message API.
#[must_use]
pub fn hash_field_elements(elements: &[u64]) -> u64 {
    cpu::hash_field_elements(elements)
}
/// Hash the provided field elements with the canonical scalar Poseidon backend.
#[must_use]
pub fn hash_field_elements_cpu(elements: &[u64]) -> u64 {
    cpu::hash_field_elements(elements)
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
        let _lane = backend::acquire_gpu_lane();
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
