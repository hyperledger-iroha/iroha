//! Allocation-free ordinary BN254 batch operations and shared operand validation.

use super::{FieldElem, MODULUS};

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
    u32::try_from(elements).is_ok()
        && elements
            .checked_mul(3 * std::mem::size_of::<[u64; 4]>())
            .is_some_and(crate::vector::gpu_launch_eligible)
}

type NativeBatch = fn(&[[u64; 4]], &[[u64; 4]], &mut [[u64; 4]]) -> bool;

fn batch_into(
    left: &[[u64; 4]],
    right: &[[u64; 4]],
    destination: &mut [[u64; 4]],
    native: NativeBatch,
    fallback: fn(FieldElem, FieldElem) -> FieldElem,
) -> bool {
    if !valid_batch(left, right, destination.len()) {
        return false;
    }
    if destination.is_empty() || (gpu_eligible(left.len()) && native(left, right, destination)) {
        return true;
    }
    // Always recompute every lane from the original operands after refusal.
    // The caller owns and funds the initialized destination for its full lifetime.
    for ((out, left), right) in destination.iter_mut().zip(left).zip(right) {
        *out = fallback(FieldElem(*left), FieldElem(*right)).0;
    }
    true
}

/// Add canonical BN254 elements into initialized caller-owned storage.
///
/// Selects a qualified CUDA path automatically, falling back to CPU/SIMD on local
/// refusal. Returns false for unequal lengths or noncanonical operands without
/// changing the destination. Empty batches succeed without accessing hardware.
pub fn add_batch_into(left: &[[u64; 4]], right: &[[u64; 4]], destination: &mut [[u64; 4]]) -> bool {
    batch_into(
        left,
        right,
        destination,
        crate::cuda::bn254_add_batch_cuda_into,
        super::add,
    )
}

/// Subtract canonical BN254 elements with the same validation and ownership as
/// [`add_batch_into`].
pub fn sub_batch_into(left: &[[u64; 4]], right: &[[u64; 4]], destination: &mut [[u64; 4]]) -> bool {
    batch_into(
        left,
        right,
        destination,
        crate::cuda::bn254_sub_batch_cuda_into,
        super::sub,
    )
}

/// Multiply canonical BN254 elements with the same validation and ownership as
/// [`add_batch_into`].
pub fn mul_batch_into(left: &[[u64; 4]], right: &[[u64; 4]], destination: &mut [[u64; 4]]) -> bool {
    batch_into(
        left,
        right,
        destination,
        crate::cuda::bn254_mul_batch_cuda_into,
        super::mul,
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
        let never: NativeBatch = |_, _, _| panic!("malformed inputs must not dispatch");
        let valid = [[0; 4]; 2];
        for (left, right) in [(&valid[..1], &valid[..]), (&valid[..], &valid[..1])] {
            assert!(!batch_into(
                left,
                right,
                &mut output,
                never,
                super::super::add
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
                super::super::mul
            ));
            assert_eq!(output, untouched);
        }
        assert!(!batch_into(
            &valid,
            &valid,
            &mut output[..1],
            never,
            super::super::add
        ));
        assert_eq!(output, untouched);
        assert!(batch_into(&[], &[], &mut [], never, super::super::add));
    }

    #[test]
    fn tiny_and_overwide_geometry_stays_on_cpu() {
        assert!(!gpu_eligible(0));
        assert!(!gpu_eligible(1));
        assert!(!gpu_eligible(usize::MAX));
        if let Some(overwide) = (u32::MAX as usize).checked_add(1) {
            assert!(!gpu_eligible(overwide));
        }
        let never: NativeBatch = |_, _, _| panic!("tiny batches stay on CPU");
        let left = [FieldElem::from_u64(5).0];
        let right = [FieldElem::from_u64(7).0];
        let mut output = [[0; 4]];
        assert!(batch_into(
            &left,
            &right,
            &mut output,
            never,
            super::super::add
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
        let refusing: NativeBatch = |_, _, out| {
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
            super::super::mul
        ));
        assert_eq!(output, [FieldElem::from_u64(35).0; 64]);
        assert_eq!((left, right), before);
    }
}
