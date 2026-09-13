//! Independent polynomial and adversarial checks for the explicit masking transform.

use super::*;
use crate::field::GoldilocksFp4V1 as F;
use fastpq_isi::FASTPQ_FINAL_V1;

fn shape(n: usize, k: usize) -> MaskingShape {
    MaskingShape {
        trace_coefficients: n,
        trace_degree_bound: n,
        mask_coefficients: k,
        mask_degree_bound: k,
    }
}

fn limits() -> MaskingLimits {
    MaskingLimits {
        max_trace_coefficients: 65_536,
        max_mask_coefficients: 131_072,
        max_output_coefficients: 196_608,
        max_output_bytes: 8 * 1024 * 1024,
        max_work_units: 2_000_000,
    }
}

fn words(count: usize, seed: u64) -> Vec<F> {
    (0..count)
        .map(|i| {
            F::new([
                seed + i as u64,
                7 + 11 * i as u64,
                13 + 17 * i as u64,
                19 + 23 * i as u64,
            ])
            .unwrap()
        })
        .collect()
}

fn horner(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |value, &coefficient| {
            value.mul(point).add(coefficient)
        })
}

// General polynomial convolution, independent of the candidate's two sparse updates.
fn reference(trace: &[F], mask: &[F]) -> Vec<F> {
    let mut vanishing = vec![F::ZERO; trace.len() + 1];
    vanishing[0] = F::ZERO.sub(F::ONE);
    vanishing[trace.len()] = F::ONE;
    let mut output = vec![F::ZERO; trace.len() + mask.len()];
    for (left, &a) in vanishing.iter().enumerate() {
        for (right, &b) in mask.iter().enumerate() {
            output[left + right] = output[left + right].add(a.mul(b));
        }
    }
    for (index, &coefficient) in trace.iter().enumerate() {
        output[index] = output[index].add(coefficient);
    }
    output
}

#[test]
fn exact_base_coefficients_pin_both_signs_and_the_high_term() {
    let plan = MaskingPlan::<u64>::new(shape(4, 3), limits()).unwrap();
    let masked = plan.apply(&[11, 13, 17, 19], &[2, 3, 5]).unwrap();
    assert_eq!(masked, [9, 10, 12, 19, 2, 3, 5]);
    assert_eq!(plan.output_coefficients(), 7);
    assert_eq!(plan.masked_degree_bound(), 7);
}

#[test]
fn full_extension_transform_matches_general_polynomial_multiplication() {
    for n in [1, 2, 4, 8, 16] {
        for k in [1, 3, n, n + 1, n + 5] {
            let trace = words(n, 31);
            let mask = words(k, 71);
            let plan = MaskingPlan::<F>::new(shape(n, k), limits()).unwrap();
            let output = plan.apply(&trace, &mask).unwrap();
            assert_eq!(output, reference(&trace, &mask), "N={n}, mask extent={k}");
            for point in [
                F::ZERO,
                F::ONE,
                F::new([3, 0, 0, 0]).unwrap(),
                F::new([0, 1, 0, 0]).unwrap(),
                F::new([0, 0, 1, 0]).unwrap(),
                F::new([0, 0, 0, 1]).unwrap(),
                F::new([2, 3, 5, 7]).unwrap(),
            ] {
                assert_eq!(
                    horner(&output, point),
                    horner(&trace, point)
                        .add(point.power(n as u64).sub(F::ONE).mul(horner(&mask, point)))
                );
            }
        }
    }
}

#[test]
fn every_small_subgroup_value_is_preserved_with_overlapping_mask_extents() {
    for n in [1_usize, 2, 4, 8, 16] {
        let generator = super::super::field_pow(
            FASTPQ_FINAL_V1.trace_root,
            1_u64 << (FASTPQ_FINAL_V1.trace_log_size - n.ilog2()),
        );
        let trace = words(n, 31);
        let mask = words(n + 3, 71);
        let output = MaskingPlan::<F>::new(shape(n, n + 3), limits())
            .unwrap()
            .apply(&trace, &mask)
            .unwrap();
        let mut point = F::ONE;
        for _ in 0..n {
            assert_eq!(horner(&output, point), horner(&trace, point));
            point = point.mul_base(generator);
        }
        assert_eq!(point, F::ONE);
    }
}

#[test]
fn full_trace_order_keeps_subgroup_values_and_declared_high_zero_padding() {
    let n = 65_536;
    let mut declaration = shape(n, 6);
    declaration.mask_degree_bound = 4;
    let mut trace = vec![F::ZERO; n];
    trace[0] = F::from_base(41).unwrap();
    trace[n - 1] = F::new([2, 3, 5, 7]).unwrap();
    let mut mask = words(4, 71);
    mask.extend([F::ZERO; 2]);
    let plan = MaskingPlan::<F>::new(declaration, limits()).unwrap();
    let output = plan.apply(&trace, &mask).unwrap();
    assert_eq!(output.len(), n + 6);
    assert_eq!(plan.masked_degree_bound(), n + 4);
    assert_eq!(output[n + 3], mask[3]);
    assert_eq!(&output[n + 4..], &[F::ZERO; 2]);
    for row in [0, 1, 407, 65535] {
        let point = F::from_base(super::super::field_pow(FASTPQ_FINAL_V1.trace_root, row)).unwrap();
        let expected = trace[0].add(trace[n - 1].mul(point.power((n - 1) as u64)));
        assert_eq!(horner(&output, point), expected);
    }
    let point = F::new([2, 3, 5, 7]).unwrap();
    let expected = trace[0]
        .add(trace[n - 1].mul(point.power((n - 1) as u64)))
        .add(point.power(n as u64).sub(F::ONE).mul(horner(&mask, point)));
    assert_eq!(horner(&output, point), expected);
}

#[test]
fn supplied_masks_are_distinct_and_borrowed_inputs_remain_immutable() {
    let trace = words(8, 17);
    let first = words(5, 31);
    let mut second = first.clone();
    second[0] = second[0].add(F::ONE);
    let original = (trace.clone(), first.clone(), second.clone());
    let plan = MaskingPlan::<F>::new(shape(8, 5), limits()).unwrap();
    assert_ne!(
        plan.apply(&trace, &first).unwrap(),
        plan.apply(&trace, &second).unwrap()
    );
    assert_eq!((trace, first, second), original);
    let shared = words(4, 31);
    let before = shared.clone();
    let output = MaskingPlan::<F>::new(shape(4, 4), limits())
        .unwrap()
        .apply(&shared, &shared)
        .unwrap();
    assert_eq!(&output[..4], &[F::ZERO; 4]);
    assert_eq!(&output[4..], shared.as_slice());
    assert_eq!(shared, before);
}

#[test]
fn explicit_zero_masks_are_algebra_only_and_never_synthesized() {
    let plan = MaskingPlan::<F>::new(shape(4, 3), limits()).unwrap();
    let trace = words(4, 31);
    let output = plan.apply(&trace, &[F::ZERO; 3]).unwrap();
    assert_eq!(&output[..4], trace.as_slice());
    assert_eq!(&output[4..], &[F::ZERO; 3]);
    assert!(plan.apply(&trace, &[]).is_err());
    assert!(MaskingPlan::<F>::new(shape(4, 0), limits()).is_err());
}

#[test]
fn degree_padding_is_checked_and_leading_allowed_zero_is_not_resampled() {
    let declaration = MaskingShape {
        trace_degree_bound: 3,
        mask_degree_bound: 2,
        ..shape(8, 5)
    };
    let plan = MaskingPlan::<F>::new(declaration, limits()).unwrap();
    let mut trace = vec![F::ZERO; 8];
    trace[..3].copy_from_slice(&words(3, 11));
    let mut mask = vec![F::ZERO; 5];
    mask[0] = F::new([2, 3, 5, 7]).unwrap();
    let output = plan.apply(&trace, &mask).unwrap();
    assert_eq!(plan.masked_degree_bound(), 10);
    assert!(output[10..].iter().all(|&value| value == F::ZERO));
    assert_eq!(output[9], F::ZERO);
    for index in 3..8 {
        let mut bad = trace.clone();
        bad[index] = F::ONE;
        assert!(plan.apply(&bad, &mask).is_err());
    }
    for index in 2..5 {
        let mut bad = mask.clone();
        bad[index] = F::ONE;
        assert!(plan.apply(&trace, &bad).is_err());
    }
}

#[test]
fn each_noncanonical_coordinate_rejects_without_modifying_inputs() {
    let plan = MaskingPlan::<F>::new(shape(4, 3), limits()).unwrap();
    let zero_trace = [F::ZERO; 4];
    let zero_mask = [F::ZERO; 3];
    for lane in 0..4 {
        let mut words = [0; 4];
        words[lane] = GOLDILOCKS_MODULUS;
        let bad = F::from_coefficients_unchecked_for_test(words);
        for index in 0..4 {
            let mut trace = zero_trace;
            trace[index] = bad;
            let before = trace;
            assert!(
                matches!(plan.apply(&trace,&zero_mask),Err(Error::NonCanonicalGoldilocksElement { context:"masking_trace_coefficient",indices }) if indices==[index,lane])
            );
            assert_eq!(trace, before);
        }
        for index in 0..3 {
            let mut mask = zero_mask;
            mask[index] = bad;
            let before = mask;
            assert!(
                matches!(plan.apply(&zero_trace,&mask),Err(Error::NonCanonicalGoldilocksElement { context:"masking_mask_coefficient",indices }) if indices==[index,lane])
            );
            assert_eq!(mask, before);
        }
    }
    let base = MaskingPlan::<u64>::new(shape(1, 1), limits()).unwrap();
    for bad in [GOLDILOCKS_MODULUS, u64::MAX] {
        assert!(base.apply(&[bad], &[0]).is_err());
        assert!(base.apply(&[0], &[bad]).is_err());
    }
}

#[test]
fn exact_extents_reject_truncated_or_added_zero_coefficients() {
    let plan = MaskingPlan::<F>::new(shape(4, 3), limits()).unwrap();
    for n in [0, 3, 5] {
        assert!(plan.apply(&vec![F::ZERO; n], &[F::ZERO; 3]).is_err());
    }
    for k in [0, 2, 4] {
        assert!(plan.apply(&[F::ZERO; 4], &vec![F::ZERO; k]).is_err());
    }
}

#[test]
fn malformed_geometry_and_degree_declarations_reject_without_buffers() {
    for invalid_shape in [
        shape(0, 1),
        shape(3, 1),
        shape(4, 0),
        MaskingShape {
            trace_degree_bound: 0,
            ..shape(4, 1)
        },
        MaskingShape {
            trace_degree_bound: 5,
            ..shape(4, 1)
        },
        MaskingShape {
            mask_degree_bound: 0,
            ..shape(4, 3)
        },
        MaskingShape {
            mask_degree_bound: 4,
            ..shape(4, 3)
        },
    ] {
        assert!(matches!(
            MaskingPlan::<F>::new(invalid_shape, limits()),
            Err(Error::InvalidTraceShape { .. })
        ));
    }
    if let Some(n) = 1_usize.checked_shl(33) {
        assert!(MaskingPlan::<F>::new(shape(n, 1), limits()).is_err());
    }
}

#[test]
fn exact_resource_ceilings_pass_and_every_tighter_ceiling_rejects() {
    let declaration = shape(8, 5);
    let plan = MaskingPlan::<F>::new(declaration, limits()).unwrap();
    let exact = MaskingLimits {
        max_trace_coefficients: 8,
        max_mask_coefficients: 5,
        max_output_coefficients: 13,
        max_output_bytes: 13 * 32,
        max_work_units: plan.work_units(),
    };
    assert!(MaskingPlan::<F>::new(declaration, exact).is_ok());
    assert_eq!(plan.work_units(), 13 * 4 + 13 * 2 + 8 + 5 * 2);
    for selector in 0..5 {
        let mut smaller = exact;
        match selector {
            0 => smaller.max_trace_coefficients -= 1,
            1 => smaller.max_mask_coefficients -= 1,
            2 => smaller.max_output_coefficients -= 1,
            3 => smaller.max_output_bytes -= 1,
            _ => smaller.max_work_units -= 1,
        }
        assert!(matches!(
            MaskingPlan::<F>::new(declaration, smaller),
            Err(Error::VerifierLimitExceeded { .. })
        ));
    }
    let base_bytes = MaskingLimits {
        max_output_bytes: 13 * 8,
        ..limits()
    };
    assert!(MaskingPlan::<u64>::new(declaration, base_bytes).is_ok());
    assert!(MaskingPlan::<F>::new(declaration, base_bytes).is_err());
}

#[test]
fn all_dimension_byte_and_work_overflows_reject_before_allocation() {
    let unrestricted = MaskingLimits {
        max_trace_coefficients: usize::MAX,
        max_mask_coefficients: usize::MAX,
        max_output_coefficients: usize::MAX,
        max_output_bytes: usize::MAX,
        max_work_units: usize::MAX,
    };
    for k in [usize::MAX, usize::MAX / size_of::<F>(), usize::MAX / 8] {
        let declaration = MaskingShape {
            mask_degree_bound: 1,
            ..shape(1, k)
        };
        assert!(matches!(
            MaskingPlan::<F>::new(declaration, unrestricted),
            Err(Error::InvalidTraceShape { .. })
        ));
    }
    assert!(checked_add(usize::MAX, 1).is_err());
    assert!(checked_mul(usize::MAX, 2).is_err());
}

#[test]
fn discarded_high_terms_and_subgroup_aliases_fail_the_full_polynomial_identity() {
    let trace = words(4, 11);
    let mask = vec![F::ONE];
    let plan = MaskingPlan::<F>::new(shape(4, 1), limits()).unwrap();
    let actual = plan.apply(&trace, &mask).unwrap();
    let expected = reference(&trace, &mask);
    assert_eq!(actual, expected);
    assert_eq!(actual[4], F::ONE);
    let point = F::new([2, 3, 5, 7]).unwrap();
    let truncated = actual[..4].to_vec();
    assert_ne!(horner(&truncated, point), horner(&expected, point));
    // Reduction modulo X^N-1 preserves subgroup values but erases the mask's
    // actual high coefficients. It is therefore not the requested transform.
    let mut alias = actual[..4].to_vec();
    alias[0] = alias[0].add(actual[4]);
    assert_eq!(alias, trace);
    assert_eq!(horner(&alias, F::ONE), horner(&expected, F::ONE));
    assert_ne!(horner(&alias, point), horner(&expected, point));
    let mut wrong_shift = actual.clone();
    wrong_shift[4] = F::ZERO;
    wrong_shift[3] = wrong_shift[3].add(F::ONE);
    assert_ne!(wrong_shift, expected);
    assert_ne!(horner(&wrong_shift, point), horner(&expected, point));
}

#[test]
fn base_embeddings_preserve_wraparound_overlap_and_cancellation() {
    let p = GOLDILOCKS_MODULUS;
    let trace = [0, p - 1, 2, p - 2];
    let mask = [p - 1, 2, 0, p - 3, 1, p - 4, 0];
    let base = MaskingPlan::<u64>::new(shape(4, 7), limits())
        .unwrap()
        .apply(&trace, &mask)
        .unwrap();
    let extension_trace: Vec<_> = trace.iter().map(|&x| F::from_base(x).unwrap()).collect();
    let extension_mask: Vec<_> = mask.iter().map(|&x| F::from_base(x).unwrap()).collect();
    let extension = MaskingPlan::<F>::new(shape(4, 7), limits())
        .unwrap()
        .apply(&extension_trace, &extension_mask)
        .unwrap();
    assert_eq!(extension, reference(&extension_trace, &extension_mask));
    assert_eq!(
        extension,
        base.iter()
            .map(|&x| F::from_base(x).unwrap())
            .collect::<Vec<_>>()
    );
    assert!(base.iter().all(|&x| x < p));
    assert_eq!(base, [1, p - 3, 2, 1, p - 2, 6, 0, p - 3, 1, p - 4, 0]);
}

#[test]
fn dimensions_precede_coordinates_and_padding_is_canonical_before_degree_checks() {
    let plan = MaskingPlan::<F>::new(
        MaskingShape {
            trace_degree_bound: 1,
            mask_degree_bound: 1,
            ..shape(4, 3)
        },
        limits(),
    )
    .unwrap();
    let malformed = F::from_coefficients_unchecked_for_test([0, GOLDILOCKS_MODULUS, 0, 0]);
    assert!(matches!(
        plan.apply(&[malformed; 3], &[malformed; 3]),
        Err(Error::InvalidTraceShape { .. })
    ));
    assert!(matches!(
        plan.apply(&[malformed; 4], &[malformed; 2]),
        Err(Error::InvalidTraceShape { .. })
    ));
    let mut trace = [F::ZERO; 4];
    let mut mask = [F::ZERO; 3];
    trace[3] = malformed;
    assert!(matches!(
        plan.apply(&trace, &mask),
        Err(Error::NonCanonicalGoldilocksElement { context: "masking_trace_coefficient", indices }) if indices == [3, 1]
    ));
    trace[3] = F::ZERO;
    mask[2] = malformed;
    assert!(matches!(
        plan.apply(&trace, &mask),
        Err(Error::NonCanonicalGoldilocksElement { context: "masking_mask_coefficient", indices }) if indices == [2, 1]
    ));
}

#[test]
fn addressable_output_limit_rejects_without_reading_or_allocating_coefficients() {
    let unbounded = MaskingLimits {
        max_trace_coefficients: usize::MAX,
        max_mask_coefficients: usize::MAX,
        max_output_coefficients: usize::MAX,
        max_output_bytes: usize::MAX,
        max_work_units: usize::MAX,
    };
    let k = (isize::MAX as usize) / size_of::<u64>() + 1;
    let error = MaskingPlan::<u64>::new(shape(1, k), unbounded).unwrap_err();
    assert!(matches!(
        error,
        Error::VerifierLimitExceeded {
            limit: "max_masking_addressable_output_bytes",
            actual,
            max,
        } if actual == (k + 1) * size_of::<u64>() && max == isize::MAX as usize
    ));
}

#[test]
fn fixed_output_masking_shares_exact_algebra_and_preserves_failed_inputs() {
    let plan = MaskingPlan::<F>::new(shape(4, 7), limits()).unwrap();
    let trace = words(4, 11);
    let mask = words(7, 47);
    let mut output = vec![F::ONE; 11];
    plan.apply_into(&trace, &mask, &mut output).unwrap();
    assert_eq!(output, reference(&trace, &mask));
    assert_eq!(output, plan.apply(&trace, &mask).unwrap());
    let original_output = output.clone();
    assert!(plan.apply_into(&trace, &mask, &mut output[..10]).is_err());
    assert_eq!(output, original_output);
    let mut malformed = mask.clone();
    malformed[6] = F::from_coefficients_unchecked_for_test([0, 0, GOLDILOCKS_MODULUS, 0]);
    assert!(plan.apply_into(&trace, &malformed, &mut output).is_err());
    assert_eq!(output, original_output);
    assert_eq!(trace, words(4, 11));
    assert_eq!(mask, words(7, 47));
}
