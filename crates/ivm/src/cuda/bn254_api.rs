//! Caller-owned BN254 destinations with charged native staging and qualification.

use super::policy::{Kernel, public_workload_task_id};
use iroha_accel::{HostOutput, PtxArtifact, cuda::CudaFailure};
use std::ffi::CStr;

#[path = "bn254_launch.rs"]
mod launch;

static ARTIFACT: PtxArtifact = PtxArtifact::new(
    match CStr::from_bytes_with_nul(
        concat!(include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx")), "\0").as_bytes(),
    ) {
        Ok(bytes) => bytes,
        Err(_) => panic!("embedded BN254 PTX must have exactly one terminal NUL"),
    },
);

fn failure_quarantines(error: CudaFailure) -> bool {
    match error {
        CudaFailure::Capacity | CudaFailure::Busy | CudaFailure::Unavailable => false,
        CudaFailure::Quarantined
        | CudaFailure::InvalidRequest
        | CudaFailure::Driver(_)
        | CudaFailure::Timeout => true,
    }
}

fn stage(
    kernel: Kernel,
    left: &[[u64; 4]],
    right: &[[u64; 4]],
) -> Result<HostOutput<[u64; 4]>, CudaFailure> {
    let result = crate::cuda_dispatch::with_selected(kernel, ARTIFACT, |device| {
        // SAFETY: this adapter fixes the embedded qualified artifact and the
        // launch module fixes its exact typed BN254 symbols and geometry.
        unsafe { launch::output(device, ARTIFACT, kernel, left, right) }
    });
    match result {
        Ok(output) => {
            super::imp::record_completed_cuda_dispatch();
            Ok(output)
        }
        Err(error) => {
            if failure_quarantines(error) {
                crate::cuda_dispatch::quarantine_current_kernel();
            }
            Err(error)
        }
    }
}

// Full-width public operands exercise wraparound, borrow and reduction of high
// product limbs. The expected results use the separate halo2curves field relation.
fn golden_operands() -> ([[u64; 4]; 8], [[u64; 4]; 8]) {
    let mut minus_one = crate::bn254_vec::MODULUS;
    minus_one[0] -= 1;
    (
        [
            [0; 4],
            [1, 0, 0, 0],
            minus_one,
            minus_one,
            [u64::MAX, u64::MAX, 0, 0],
            [0, 0, 0, 1],
            [u64::MAX, 0, u64::MAX, 0],
            [u32::MAX as u64 + 17, 0, 0, 0],
        ],
        [
            minus_one,
            [2, 0, 0, 0],
            [2, 0, 0, 0],
            minus_one,
            [1, 0, 0, 0],
            [0, 0, 0, 1],
            [u64::MAX, u64::MAX, 0, 0],
            [11, 0, 0, 0],
        ],
    )
}

fn golden_output(
    kernel: Kernel,
    left: &[[u64; 4]; 8],
    right: &[[u64; 4]; 8],
) -> Option<[[u64; 4]; 8]> {
    use crate::bn254_vec::FieldElem;
    let operation: fn(halo2curves::bn256::Fr, halo2curves::bn256::Fr) -> halo2curves::bn256::Fr =
        match kernel {
            Kernel::BnAdd => |a, b| a + b,
            Kernel::BnSub => |a, b| a - b,
            Kernel::BnMul => |a, b| a * b,
            _ => return None,
        };
    Some(std::array::from_fn(|index| {
        FieldElem::from_fr(operation(
            FieldElem(left[index]).to_fr(),
            FieldElem(right[index]).to_fr(),
        ))
        .0
    }))
}

/// Qualify the exact device/kernel/artifact with actual native arithmetic only.
pub(super) fn admit(kernel: Kernel) -> bool {
    crate::cuda_dispatch::admit_kernel(kernel, ARTIFACT, || {
        let Some(_guard) = super::imp::SelftestRunningGuard::enter() else {
            return Err(CudaFailure::Busy);
        };
        let (left, right) = golden_operands();
        let Some(expected) = golden_output(kernel, &left, &right) else {
            return Ok(false);
        };
        stage(kernel, &left, &right).map(|output| output.as_slice() == expected)
    })
}

fn publish(output: &[[u64; 4]], destination: &mut [[u64; 4]]) -> bool {
    if output.len() != destination.len() || !output.iter().all(crate::bn254_vec::canonical) {
        return false;
    }
    destination.copy_from_slice(output);
    true
}

fn into(
    kernel: Kernel,
    left: &[[u64; 4]],
    right: &[[u64; 4]],
    destination: &mut [[u64; 4]],
) -> bool {
    if !crate::bn254_vec::valid_batch(left, right, destination.len()) {
        return false;
    }
    if destination.is_empty() {
        return true;
    }
    if launch::request(left.len()).is_none() {
        return false;
    }
    let task = public_workload_task_id(0x0f0f_0f0f_0000_0060, &[kernel as u64, left.len() as u64]);
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(kernel) {
            return false;
        }
        let Ok(output) = stage(kernel, left, right) else {
            return false;
        };
        // HostOutput retains the original reservation until publication finishes.
        if !publish(output.as_slice(), destination) {
            crate::cuda_dispatch::quarantine_current_kernel();
            return false;
        }
        true
    })
}

/// Attempt BN254 addition into caller-owned storage. False leaves it untouched.
/// Inputs must be canonical field elements with lengths equal to the destination.
pub fn bn254_add_batch_cuda_into(
    left: &[[u64; 4]],
    right: &[[u64; 4]],
    destination: &mut [[u64; 4]],
) -> bool {
    into(Kernel::BnAdd, left, right, destination)
}

/// Attempt BN254 subtraction with the same contract as [`bn254_add_batch_cuda_into`].
pub fn bn254_sub_batch_cuda_into(
    left: &[[u64; 4]],
    right: &[[u64; 4]],
    destination: &mut [[u64; 4]],
) -> bool {
    into(Kernel::BnSub, left, right, destination)
}

/// Attempt BN254 multiplication with the same contract as [`bn254_add_batch_cuda_into`].
pub fn bn254_mul_batch_cuda_into(
    left: &[[u64; 4]],
    right: &[[u64; 4]],
    destination: &mut [[u64; 4]],
) -> bool {
    into(Kernel::BnMul, left, right, destination)
}

/// Attempt one canonical BN254 addition using stack-owned output storage.
pub fn bn254_add_cuda(left: [u64; 4], right: [u64; 4]) -> Option<[u64; 4]> {
    let mut output = [[0; 4]];
    bn254_add_batch_cuda_into(&[left], &[right], &mut output).then_some(output[0])
}

/// Attempt one canonical BN254 subtraction using stack-owned output storage.
pub fn bn254_sub_cuda(left: [u64; 4], right: [u64; 4]) -> Option<[u64; 4]> {
    let mut output = [[0; 4]];
    bn254_sub_batch_cuda_into(&[left], &[right], &mut output).then_some(output[0])
}

/// Attempt one canonical BN254 multiplication using stack-owned output storage.
pub fn bn254_mul_cuda(left: [u64; 4], right: [u64; 4]) -> Option<[u64; 4]> {
    let mut output = [[0; 4]];
    bn254_mul_batch_cuda_into(&[left], &[right], &mut output).then_some(output[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_failure_quarantines_but_local_pressure_only_refuses() {
        for error in [
            CudaFailure::Driver(2),
            CudaFailure::Timeout,
            CudaFailure::Quarantined,
            CudaFailure::InvalidRequest,
        ] {
            assert!(failure_quarantines(error));
        }
        for error in [
            CudaFailure::Capacity,
            CudaFailure::Busy,
            CudaFailure::Unavailable,
        ] {
            assert!(!failure_quarantines(error));
        }
    }

    #[test]
    fn publication_rejects_malformed_native_output_without_partial_changes() {
        let untouched = [[7; 4]; 2];
        let mut output = untouched;
        assert!(!publish(&[[0; 4]], &mut output));
        assert_eq!(output, untouched);
        assert!(!publish(&[[0; 4], crate::bn254_vec::MODULUS], &mut output));
        assert_eq!(output, untouched);
        assert!(publish(&[[0; 4], [1, 0, 0, 0]], &mut output));
        assert_eq!(output, [[0; 4], [1, 0, 0, 0]]);
    }

    #[test]
    fn golden_full_width_controls_match_scalar_arithmetic() {
        let (left, right) = golden_operands();
        assert!(crate::bn254_vec::valid_batch(&left, &right, 8));
        for (kernel, operation) in [
            (Kernel::BnAdd, crate::bn254_vec::add_scalar as fn(_, _) -> _),
            (Kernel::BnSub, crate::bn254_vec::sub_scalar),
            (Kernel::BnMul, crate::bn254_vec::mul_scalar),
        ] {
            let expected = golden_output(kernel, &left, &right).unwrap();
            for i in 0..left.len() {
                assert_eq!(
                    expected[i],
                    operation(
                        crate::bn254_vec::FieldElem(left[i]),
                        crate::bn254_vec::FieldElem(right[i])
                    )
                    .0
                );
            }
        }
        assert!(golden_output(Kernel::Add32, &left, &right).is_none());
    }
}
