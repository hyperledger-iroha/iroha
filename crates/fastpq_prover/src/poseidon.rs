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
use fastpq_isi::poseidon::{self as cpu, PoseidonSponge as CpuPoseidonSponge};
#[cfg(feature = "fastpq-gpu")]
use fastpq_isi::poseidon::{RATE, STATE_WIDTH};
#[cfg(feature = "fastpq-gpu")]
use {
    crate::backend::{self, GpuBackend},
    crate::fastpq_cuda,
    std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    tracing::warn,
};

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use crate::metal;

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
    accelerator: GpuBackend,
    fallback: CpuPoseidonBackend,
    gpu_enabled: Arc<AtomicBool>,
}

#[cfg(feature = "fastpq-gpu")]
impl GpuPoseidonBackend {
    fn new(accelerator: GpuBackend) -> Self {
        Self {
            accelerator,
            fallback: CpuPoseidonBackend,
            gpu_enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    fn is_gpu_enabled(&self) -> bool {
        self.gpu_enabled.load(Ordering::Acquire)
    }

    fn disable_gpu_with_warning(&self, message: &str, context: &str) {
        if self
            .gpu_enabled
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            warn!(
                target: "fastpq::poseidon",
                backend = ?self.accelerator,
                error = message,
                context,
                "GPU Poseidon backend failed; falling back to CPU implementation"
            );
        }
    }

    fn permute_state(&self, state: &mut [u64; STATE_WIDTH]) {
        if !self.is_gpu_enabled() {
            cpu::permute_state(state);
            return;
        }

        let mut buffer = *state;
        let result: Result<(), String> = {
            let _guard = backend::acquire_gpu_lane();
            match self.accelerator {
                GpuBackend::Cuda => fastpq_cuda::fastpq_poseidon_permute(buffer.as_mut_slice())
                    .map_err(|err| err.to_string()),
                #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
                GpuBackend::Metal => {
                    metal::poseidon_permute(buffer.as_mut_slice()).map_err(|err| err.to_string())
                }
                other => Err(format!("{other:?} backend unsupported")),
            }
        };

        match result {
            Ok(()) => {
                *state = buffer;
            }
            Err(error) => {
                self.disable_gpu_with_warning(&error, "poseidon_permute");
                cpu::permute_state(state);
            }
        }
    }
}

#[cfg(feature = "fastpq-gpu")]
impl PoseidonBackend for GpuPoseidonBackend {
    fn hash_field_elements(&self, elements: &[u64]) -> u64 {
        if !self.is_gpu_enabled() {
            return self.fallback.hash_field_elements(elements);
        }
        hash_with_permute(elements, |state| self.permute_state(state))
    }

    fn new_sponge(&self) -> Box<dyn PoseidonSpongeCore> {
        Box::new(GpuPoseidonSponge::new(self.clone()))
    }
}

#[cfg(feature = "fastpq-gpu")]
struct GpuPoseidonSponge {
    backend: GpuPoseidonBackend,
    state: [u64; STATE_WIDTH],
    rate_index: usize,
    finalised: bool,
}

#[cfg(feature = "fastpq-gpu")]
impl GpuPoseidonSponge {
    fn new(backend: GpuPoseidonBackend) -> Self {
        Self {
            backend,
            state: [0u64; STATE_WIDTH],
            rate_index: 0,
            finalised: false,
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
}

#[cfg(feature = "fastpq-gpu")]
impl PoseidonSpongeCore for GpuPoseidonSponge {
    fn absorb(&mut self, element: u64) {
        debug_assert!(
            !self.finalised,
            "cannot absorb into a finalised sponge; start a new instance"
        );
        self.state[self.rate_index] = add_mod(self.state[self.rate_index], element);
        self.rate_index += 1;
        if self.rate_index == RATE {
            self.backend.permute_state(&mut self.state);
            self.rate_index = 0;
        }
    }

    fn absorb_slice(&mut self, elements: &[u64]) {
        for &element in elements {
            self.absorb(element);
        }
    }

    fn squeeze_element(&mut self) -> u64 {
        self.ensure_finalised();
        let element = self.state[0];
        self.backend.permute_state(&mut self.state);
        element
    }

    fn squeeze(mut self: Box<Self>) -> u64 {
        self.ensure_finalised();
        self.state[0]
    }
}

fn backend() -> &'static dyn PoseidonBackend {
    static BACKEND: OnceLock<Box<dyn PoseidonBackend>> = OnceLock::new();
    BACKEND
        .get_or_init(|| {
            #[cfg(feature = "fastpq-gpu")]
            if let Some(accelerator) = backend::current_gpu_backend() {
                return Box::new(GpuPoseidonBackend::new(accelerator));
            }
            Box::new(CpuPoseidonBackend)
        })
        .as_ref()
}

/// Hash the provided field elements with the active Poseidon backend.
#[must_use]
pub fn hash_field_elements(elements: &[u64]) -> u64 {
    backend().hash_field_elements(elements)
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

#[cfg(feature = "fastpq-gpu")]
fn hash_with_permute(elements: &[u64], mut permute: impl FnMut(&mut [u64; STATE_WIDTH])) -> u64 {
    if elements.is_empty() {
        return cpu::hash_field_elements(elements);
    }
    let mut padded = Vec::with_capacity(elements.len() + RATE);
    padded.extend_from_slice(elements);
    padded.push(1);
    while padded.len() % RATE != 0 {
        padded.push(0);
    }

    let mut state = [0u64; STATE_WIDTH];
    for chunk in padded.chunks(RATE) {
        state[0] = add_mod(state[0], chunk[0]);
        if RATE > 1 {
            state[1] = add_mod(state[1], chunk[1]);
        }
        permute(&mut state);
    }
    state[0]
}

#[cfg(feature = "fastpq-gpu")]
#[inline]
fn add_mod(a: u64, b: u64) -> u64 {
    let sum = u128::from(a) + u128::from(b);
    let modulus = u128::from(FIELD_MODULUS);
    let reduced = if sum >= modulus { sum - modulus } else { sum };
    u64::try_from(reduced).expect("Goldilocks reduction fits in u64")
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
