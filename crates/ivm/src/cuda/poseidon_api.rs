//! Poseidon CUDA publication from one complete charged state owner.

use super::policy::{Kernel, public_workload_task_id};
use iroha_accel::{PtxArtifact, cuda::CudaFailure};
use std::ffi::CStr;
#[path = "poseidon_launch.rs"]
mod launch;

static ARTIFACT: PtxArtifact = PtxArtifact::new(
    match CStr::from_bytes_with_nul(
        concat!(
            include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx")),
            "\0"
        )
        .as_bytes(),
    ) {
        Ok(bytes) => bytes,
        Err(_) => panic!("embedded Poseidon artifact must have exactly one terminal NUL"),
    },
);

#[derive(Clone, Copy)]
enum Input<'a> {
    Two(&'a [(u64, u64)]),
    Six(&'a [[u64; 6]]),
}
impl Input<'_> {
    fn width(self) -> usize {
        match self {
            Self::Two(_) => 3,
            Self::Six(_) => 6,
        }
    }
    fn len(self) -> usize {
        match self {
            Self::Two(values) => values.len(),
            Self::Six(values) => values.len(),
        }
    }
    fn kernel(self) -> Kernel {
        match self {
            Self::Two(_) => Kernel::Poseidon2,
            Self::Six(_) => Kernel::Poseidon6,
        }
    }
    fn word(self, index: usize) -> u64 {
        if index % 4 != 0 {
            return 0;
        }
        let lane = index / 4;
        match self {
            Self::Two(values) => match lane % 3 {
                0 => values[lane / 3].0,
                1 => values[lane / 3].1,
                _ => 0,
            },
            Self::Six(values) => values[lane / 6][lane % 6],
        }
    }
}

fn stage(input: Input<'_>, geometry: launch::Geometry) -> Result<launch::Output, CudaFailure> {
    let result = crate::cuda_dispatch::with_selected(input.kernel(), ARTIFACT, |device| {
        match input {
            Input::Two(_) => {
                let (constants, matrix) = crate::poseidon::poseidon2_params();
                // SAFETY: exact fixed-width representation, complete initialized
                // state, and the canonical CPU parameters for the admitted artifact.
                unsafe {
                    launch::output(
                        device,
                        ARTIFACT,
                        geometry,
                        |i| input.word(i),
                        |i| constants[i / 12][(i / 4) % 3].0[i % 4],
                        |i| matrix[i / 12][(i / 4) % 3].0[i % 4],
                    )
                }
            }
            Input::Six(_) => {
                let (constants, matrix) = crate::poseidon::poseidon6_params();
                // SAFETY: same contract as the width-three branch with width six.
                unsafe {
                    launch::output(
                        device,
                        ARTIFACT,
                        geometry,
                        |i| input.word(i),
                        |i| constants[i / 24][(i / 4) % 6].0[i % 4],
                        |i| matrix[i / 24][(i / 4) % 6].0[i % 4],
                    )
                }
            }
        }
    });
    match result {
        Ok(output) => {
            super::imp::record_completed_cuda_dispatch();
            Ok(output)
        }
        Err(error) => {
            if !matches!(
                error,
                CudaFailure::Capacity | CudaFailure::Busy | CudaFailure::Unavailable
            ) {
                crate::cuda_dispatch::quarantine_current_kernel();
            }
            Err(error)
        }
    }
}

fn valid_output(output: &launch::Output, input: Input<'_>) -> bool {
    let Some(state) = output.state.as_ref() else {
        return false;
    };
    output.status == [0, 0]
        && state.len() == input.len() * input.width() * 4
        && state.chunks_exact(4).all(|field| {
            field
                .iter()
                .rev()
                .cmp(crate::bn254_vec::MODULUS.iter().rev())
                .is_lt()
        })
}

pub(super) fn admit(kernel: Kernel) -> bool {
    crate::cuda_dispatch::admit_kernel(kernel, ARTIFACT, || {
        let Some(_guard) = super::imp::SelftestRunningGuard::enter() else {
            return Err(CudaFailure::Busy);
        };
        let two = [(0, 0), (7, 11), (u64::MAX, 1), (u64::MAX, u64::MAX)];
        let six = [
            [0; 6],
            [1, 2, 3, 4, 5, 6],
            [u64::MAX, 0, 1, 2, 3, 4],
            [u64::MAX; 6],
        ];
        let input = match kernel {
            Kernel::Poseidon2 => Input::Two(&two),
            Kernel::Poseidon6 => Input::Six(&six),
            _ => return Ok(false),
        };
        let output = stage(
            input,
            launch::Geometry::canonical(input.width(), input.len()),
        )?;
        if !valid_output(&output, input) {
            return Ok(false);
        }
        let state = output.state.as_ref().expect("valid output contains state");
        Ok(state
            .chunks_exact(input.width() * 4)
            .enumerate()
            .all(|(index, words)| {
                words[0]
                    == match input {
                        Input::Two(values) => {
                            crate::poseidon::poseidon2_simd(values[index].0, values[index].1)
                        }
                        Input::Six(values) => crate::poseidon::poseidon6_simd(values[index]),
                    }
            }))
    })
}

fn into(input: Input<'_>, destination: &mut [u64]) -> bool {
    if input.len() != destination.len()
        || u32::try_from(input.len()).is_err()
        || input.len().checked_mul(input.width() * 4).is_none()
    {
        return false;
    }
    if input.len() == 0 {
        return true;
    }
    let task = public_workload_task_id(
        0x0f0f_0f0f_0000_0030,
        &[input.kernel() as u64, input.len() as u64],
    );
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(input.kernel()) {
            return false;
        }
        let Ok(output) = stage(
            input,
            launch::Geometry::canonical(input.width(), input.len()),
        ) else {
            return false;
        };
        if !valid_output(&output, input) {
            crate::cuda_dispatch::quarantine_current_kernel();
            return false;
        }
        let state = output.state.as_ref().expect("validated complete state");
        for (result, words) in destination
            .iter_mut()
            .zip(state.chunks_exact(input.width() * 4))
        {
            *result = words[0];
        }
        true
    })
}

/// Attempt a Poseidon2 batch into caller storage; failure leaves it unchanged.
pub fn poseidon2_cuda_many_into(inputs: &[(u64, u64)], destination: &mut [u64]) -> bool {
    into(Input::Two(inputs), destination)
}
/// Attempt a Poseidon6 batch into caller storage; failure leaves it unchanged.
pub fn poseidon6_cuda_many_into(inputs: &[[u64; 6]], destination: &mut [u64]) -> bool {
    into(Input::Six(inputs), destination)
}
/// Attempt one Poseidon2 hash using stack-owned output.
pub fn poseidon2_cuda(a: u64, b: u64) -> Option<u64> {
    let mut output = [0];
    poseidon2_cuda_many_into(&[(a, b)], &mut output).then_some(output[0])
}
/// Attempt one Poseidon6 hash using stack-owned output.
pub fn poseidon6_cuda(inputs: [u64; 6]) -> Option<u64> {
    let mut output = [0];
    poseidon6_cuda_many_into(&[inputs], &mut output).then_some(output[0])
}

#[cfg(test)]
pub(super) fn fault_probe(short_stride: bool) -> Option<[u32; 2]> {
    crate::cuda_dispatch::with_task_scope(0, || {
        if !super::imp::ensure_cuda_kernel(Kernel::Poseidon2) {
            return None;
        }
        let input = Input::Two(&[(0, 1)]);
        let mut geometry = launch::Geometry::canonical(3, 1);
        if short_stride {
            geometry.stride = 1;
        } else {
            geometry.full_rounds = 0;
            geometry.partial_rounds = 0;
        }
        // These exact invalid parameters return before state/constant accesses.
        stage(input, geometry).ok().map(|output| output.status)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_state_words_preserve_every_input_and_zero_upper_limbs() {
        let pairs = [(u64::MAX, 7), (11, 13)];
        let input = Input::Two(&pairs);
        let words: [u64; 24] = std::array::from_fn(|i| input.word(i));
        assert_eq!(
            [
                words[0], words[4], words[8], words[12], words[16], words[20]
            ],
            [u64::MAX, 7, 0, 11, 13, 0]
        );
        assert!(
            words
                .iter()
                .enumerate()
                .all(|(i, word)| i % 4 == 0 || *word == 0)
        );
        let lanes = [[1, 2, 3, 4, 5, u64::MAX]];
        let input = Input::Six(&lanes);
        assert_eq!(
            std::array::from_fn::<_, 6, _>(|i| input.word(i * 4)),
            lanes[0]
        );
    }
}
