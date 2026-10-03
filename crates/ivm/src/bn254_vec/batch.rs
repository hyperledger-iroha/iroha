//! Allocation-free ordinary BN254 batch operations and shared operand validation.

use super::{FieldElem, MODULUS};
use crate::field_dispatch::{FieldArithmetic, field_impl};

// One measured public span shared by ordinary routing and CUDA calibration.
pub(crate) const BATCH_SIZES: [usize; 4] = [64, 256, 1_024, 4_096];

#[derive(Clone, Copy, Debug)]
pub(crate) enum BatchOperation {
    Add,
    Sub,
    Mul,
}

pub(crate) fn cpu_batch_into(
    operation: BatchOperation,
    cpu: &'static dyn FieldArithmetic,
    left: &[[u64; 4]],
    right: &[[u64; 4]],
    destination: &mut [[u64; 4]],
) {
    for ((out, left), right) in destination.iter_mut().zip(left).zip(right) {
        let (left, right) = (FieldElem(*left), FieldElem(*right));
        *out = match operation {
            BatchOperation::Add => cpu.add(left, right),
            BatchOperation::Sub => cpu.sub(left, right),
            BatchOperation::Mul => cpu.mul(left, right),
        }
        .0;
    }
}

pub(crate) fn canonical(words: &[u64; 4]) -> bool {
    words.iter().rev().cmp(MODULUS.iter().rev()).is_lt()
}

pub(crate) fn valid_batch(left: &[[u64; 4]], right: &[[u64; 4]], destination_len: usize) -> bool {
    left.len() == right.len()
        && left.len() == destination_len
        && left.iter().all(canonical)
        && right.iter().all(canonical)
}

fn gpu_eligible(elements: usize) -> bool {
    (BATCH_SIZES[0]..=BATCH_SIZES[BATCH_SIZES.len() - 1]).contains(&elements)
}

type NativeBatch = fn(
    BatchOperation,
    &[[u64; 4]],
    &[[u64; 4]],
    &mut [[u64; 4]],
    &'static dyn FieldArithmetic,
) -> bool;

fn batch_into(
    left: &[[u64; 4]],
    right: &[[u64; 4]],
    destination: &mut [[u64; 4]],
    native: NativeBatch,
    operation: BatchOperation,
) -> bool {
    if !valid_batch(left, right, destination.len()) {
        return false;
    }
    if destination.is_empty()
        || (gpu_eligible(left.len()) && native(operation, left, right, destination, field_impl()))
    {
        return true;
    }
    // Always recompute every lane from the original operands after refusal.
    // Resolve the current CPU policy again after calibration or native work.
    cpu_batch_into(operation, field_impl(), left, right, destination);
    true
}

/// Add canonical BN254 elements into initialized caller-owned storage.
///
/// Selects CUDA only within a qualified device's measured public cost span,
/// falling back to the current CPU/SIMD backend on local
/// refusal. Returns false for unequal lengths or noncanonical operands without
/// changing the destination. Empty batches succeed without accessing hardware.
pub fn add_batch_into(left: &[[u64; 4]], right: &[[u64; 4]], destination: &mut [[u64; 4]]) -> bool {
    batch_into(
        left,
        right,
        destination,
        crate::cuda::bn254_batch_auto_into,
        BatchOperation::Add,
    )
}

/// Subtract canonical BN254 elements with the same validation and ownership as
/// [`add_batch_into`].
pub fn sub_batch_into(left: &[[u64; 4]], right: &[[u64; 4]], destination: &mut [[u64; 4]]) -> bool {
    batch_into(
        left,
        right,
        destination,
        crate::cuda::bn254_batch_auto_into,
        BatchOperation::Sub,
    )
}

/// Multiply canonical BN254 elements with the same validation and ownership as
/// [`add_batch_into`].
pub fn mul_batch_into(left: &[[u64; 4]], right: &[[u64; 4]], destination: &mut [[u64; 4]]) -> bool {
    batch_into(
        left,
        right,
        destination,
        crate::cuda::bn254_batch_auto_into,
        BatchOperation::Mul,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_bounds_cover_every_limb() {
        assert!(canonical(&[0; 4]));
        assert!(FieldElem([0; 4]).is_canonical());
        assert!(!FieldElem(MODULUS).is_canonical());
        let mut below = MODULUS;
        below[0] -= 1;
        assert!(canonical(&below));
        assert!(!canonical(&MODULUS));
        assert!(!canonical(&[u64::MAX; 4]));
        for index in 0..4 {
            let mut above = MODULUS;
            above[index] += 1;
            assert!(!canonical(&above));
        }
    }

    #[test]
    fn malformed_inputs_decline_before_dispatch_or_publication() {
        let untouched = [[7; 4]; 2];
        let mut output = untouched;
        let never: NativeBatch = |_, _, _, _, _| panic!("malformed inputs must not dispatch");
        let valid = [[0; 4]; 2];
        for (left, right) in [(&valid[..1], &valid[..]), (&valid[..], &valid[..1])] {
            assert!(!batch_into(
                left,
                right,
                &mut output,
                never,
                BatchOperation::Add
            ));
            assert_eq!(output, untouched);
        }
        let invalid = [[0; 4], MODULUS];
        for (left, right) in [(&invalid, &valid), (&valid, &invalid)] {
            assert!(!batch_into(
                left,
                right,
                &mut output,
                never,
                BatchOperation::Mul
            ));
            assert_eq!(output, untouched);
        }
        assert!(!batch_into(
            &valid,
            &valid,
            &mut output[..1],
            never,
            BatchOperation::Add
        ));
        assert_eq!(output, untouched);
        assert!(batch_into(&[], &[], &mut [], never, BatchOperation::Add));
    }

    #[test]
    fn tiny_and_overwide_geometry_stays_on_cpu() {
        assert!(!gpu_eligible(0));
        assert!(!gpu_eligible(1));
        assert!(!gpu_eligible(usize::MAX));
        if let Some(overwide) = (u32::MAX as usize).checked_add(1) {
            assert!(!gpu_eligible(overwide));
        }
        let never: NativeBatch = |_, _, _, _, _| panic!("tiny batches stay on CPU");
        let left = [FieldElem::from_u64(5).0];
        let right = [FieldElem::from_u64(7).0];
        let mut output = [[0; 4]];
        assert!(batch_into(
            &left,
            &right,
            &mut output,
            never,
            BatchOperation::Add
        ));
        assert_eq!(output, [FieldElem::from_u64(12).0]);
    }

    #[test]
    fn refusal_recomputes_all_lanes_from_original_operands() {
        let left = [FieldElem::from_u64(5).0; 64];
        let right = [FieldElem::from_u64(7).0; 64];
        let before = (left, right);
        let mut output = [[u64::MAX; 4]; 64];
        // A deliberately broken adapter must still not taint ordinary fallback.
        let refusing: NativeBatch = |_, _, _, out, _| {
            out[0] = [0; 4];
            out[31] = [1; 4];
            false
        };
        assert!(gpu_eligible(left.len()));
        assert!(batch_into(
            &left,
            &right,
            &mut output,
            refusing,
            BatchOperation::Mul
        ));
        assert_eq!(output, [FieldElem::from_u64(35).0; 64]);
        assert_eq!((left, right), before);
    }
}
