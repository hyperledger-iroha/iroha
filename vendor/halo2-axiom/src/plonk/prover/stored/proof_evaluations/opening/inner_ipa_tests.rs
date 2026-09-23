//! Guarded inner-IPA arithmetic, allocation reuse, checked admission and erasure tests.

use super::*;
use crate::{
    arithmetic::best_multiexp_with_extra,
    plonk::prover::stored::proof_evaluations::take_clear_observations,
    poly::{
        commitment::ParamsProver,
        ipa::commitment::{ParamsIPA, collapse_round_vectors},
    },
};
use group::Curve;
use halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn round_parity<F: StoredAssignmentFieldV1>() {
    for k in [1, 4, 8] {
        let n = 1_usize << k;
        for offset in 0..3 {
            let mut p = (0..n)
                .map(|i| F::from((i * i + 7) as u64))
                .collect::<Vec<_>>();
            let mut b = (0..n)
                .map(|i| F::from((i * 13 + 3) as u64))
                .collect::<Vec<_>>();
            let (p_pointer, b_pointer) = (p.as_ptr(), b.as_ptr());
            let (p_capacity, b_capacity) = (p.capacity(), b.capacity());
            let mut original_p = p.clone();
            let mut original_b = b.clone();
            let mut sequential_p = p.clone();
            let mut sequential_b = b.clone();
            let mut live = n;
            for round in 0..k {
                let half = live / 2;
                let u = [F::ONE, -F::ONE, F::from(17)][(round + offset) % 3];
                let inverse = Option::<F>::from(u.invert()).unwrap();
                collapse_round_vectors(&mut original_p, &mut original_b, half, u, inverse);
                original_p.truncate(half);
                original_b.truncate(half);
                for index in 0..half {
                    sequential_p[index] =
                        sequential_p[index] + sequential_p[index + half] * inverse;
                    sequential_b[index] = sequential_b[index] + sequential_b[index + half] * u;
                }
                sequential_p.truncate(half);
                sequential_b.truncate(half);
                collapse(&mut p, &mut b, &mut live, u).unwrap();
                assert_eq!(live, half);
                assert_eq!(&p[..live], original_p.as_slice());
                assert_eq!(&b[..live], original_b.as_slice());
                assert_eq!(&p[..live], sequential_p.as_slice());
                assert_eq!(&b[..live], sequential_b.as_slice());
                assert!(p[live..].iter().chain(&b[live..]).all(|v| *v == F::ZERO));
                assert_eq!((p.len(), b.len()), (n, n));
                assert_eq!((p.capacity(), b.capacity()), (p_capacity, b_capacity));
                assert_eq!((p.as_ptr(), b.as_ptr()), (p_pointer, b_pointer));
            }
            assert_eq!(live, 1);
        }
    }
}

#[test]
fn both_pasta_inner_collapse_matches_original_and_independent_rounds_and_erases_tails() {
    round_parity::<Fp>();
    round_parity::<Fq>();
}

fn invalid_rounds<F: StoredAssignmentFieldV1>() {
    for (p_len, b_len, initial_live, u) in [
        (0, 0, 0, F::ONE),
        (1, 1, 1, F::ONE),
        (4, 4, 0, F::ONE),
        (4, 4, 3, F::ONE),
        (4, 4, 6, F::ONE),
        (4, 4, usize::MAX, F::ONE),
        (4, 4, 4, F::ZERO),
        (2, 4, 4, F::ONE),
        (4, 2, 4, F::ONE),
    ] {
        let original_p = vec![F::from(11); p_len];
        let original_b = vec![F::from(19); b_len];
        let mut p = original_p.clone();
        let mut b = original_b.clone();
        let mut live = initial_live;
        assert!(collapse(&mut p, &mut b, &mut live, u).is_err());
        assert_eq!(p, original_p);
        assert_eq!(b, original_b);
        assert_eq!(live, initial_live);
    }
}

#[test]
fn both_pasta_inner_collapse_zero_challenge_and_invalid_bounds_preserve_inputs() {
    invalid_rounds::<Fp>();
    invalid_rounds::<Fq>();
}

fn payload_bounds<C: CurveAffine>()
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let base = payload::<C>(1, 1, 2, 1, 2).unwrap();
    for slot in 0..5 {
        let mut values = [1, 1, 2, 1, 2];
        values[slot] = usize::MAX;
        assert!(payload::<C>(values[0], values[1], values[2], values[3], values[4]).is_err());
        values = [1, 1, 2, 1, 2];
        values[slot] += 1;
        let actual = payload::<C>(values[0], values[1], values[2], values[3], values[4]).unwrap();
        let width = if slot < 3 {
            std::mem::size_of::<C::Scalar>()
        } else {
            std::mem::size_of::<C>()
        };
        assert_eq!(actual - base, width, "every owned capacity must be charged");
    }
    assert!(base > 4 * std::mem::size_of::<C::Scalar>() + 3 * std::mem::size_of::<C>());
    assert!(MsmScratch::<C>::new(usize::MAX).is_err());
    assert!(MsmScratch::<C>::new(usize::MAX - 1).is_err());
}

#[test]
fn both_pasta_inner_payload_counts_all_capacities_and_refuses_overflow() {
    payload_bounds::<EqAffine>();
    payload_bounds::<EpAffine>();
}

fn msm_parity<C: CurveAffine>()
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let params = ParamsIPA::<C>::new(8);
    // k=0 has no IPA rounds, but its admitted empty-half scratch still owns two extras.
    let mut empty = MsmScratch::<C>::new(0).unwrap();
    assert_eq!(empty.scalars.0.len(), 2);
    assert_eq!(empty.bases.len(), 2);
    let extra_value = C::Scalar::from(11);
    let extra_random = C::Scalar::from(13);
    let extra_z = C::Scalar::from(17);
    let expected = best_multiexp_with_extra::<C>(
        &[],
        &[],
        &[(extra_value * extra_z, params.u), (extra_random, params.w)],
    );
    let actual = empty
        .msm(
            &[],
            &[],
            extra_value,
            extra_random,
            extra_z,
            params.u,
            params.w,
        )
        .unwrap();
    assert_eq!(actual.to_affine(), expected.to_affine());
    assert!(empty.scalars.0.iter().all(|v| *v == C::Scalar::ZERO));
    drop(empty);
    let mut scratch = MsmScratch::<C>::new(128).unwrap();
    let scalar_pointer = scratch.scalars.0.as_ptr();
    let base_pointer = scratch.bases.as_ptr();
    let scalar_capacity = scratch.scalars.0.capacity();
    let base_capacity = scratch.bases.capacity();
    let initialized = scratch.scalars.0.len();
    assert_eq!(initialized, 130);
    for length in [128, 1, 32, 64, 2] {
        let coefficients = (0..length)
            .map(|index| C::Scalar::from((index * index + 5) as u64))
            .collect::<Vec<_>>();
        let bases = &params.get_g()[..length];
        for z in [
            C::Scalar::ZERO,
            C::Scalar::ONE,
            -C::Scalar::ONE,
            C::Scalar::from(23),
        ] {
            let value = C::Scalar::from(31);
            let random = C::Scalar::from(43);
            let expected = best_multiexp_with_extra(
                &coefficients,
                bases,
                &[(value * z, params.u), (random, params.w)],
            );
            // Stale values outside the current shorter join must not survive success.
            scratch.scalars.0.fill(C::Scalar::from(97));
            let actual = scratch
                .msm(&coefficients, bases, value, random, z, params.u, params.w)
                .unwrap();
            assert_eq!(actual.to_affine(), expected.to_affine());
            assert!(scratch.scalars.0.iter().all(|v| *v == C::Scalar::ZERO));
            assert_eq!(scratch.scalars.0.len(), initialized);
            assert_eq!(scratch.scalars.0.as_ptr(), scalar_pointer);
            assert_eq!(scratch.bases.as_ptr(), base_pointer);
            assert_eq!(scratch.scalars.0.capacity(), scalar_capacity);
            assert_eq!(scratch.bases.capacity(), base_capacity);
        }
    }
    let too_many = vec![C::Scalar::ONE; 129];
    assert!(
        scratch
            .msm(
                &too_many,
                &params.get_g()[..129],
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                params.u,
                params.w
            )
            .is_err()
    );
    assert!(
        scratch
            .msm(
                &[C::Scalar::ONE; 2],
                &params.get_g()[..1],
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                params.u,
                params.w
            )
            .is_err()
    );
    scratch.scalars.0.fill(C::Scalar::from(89));
    take_clear_observations();
    drop(scratch);
    let (count, zero) = take_clear_observations();
    assert!(count >= initialized && zero);

    take_clear_observations();
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let mut scratch = MsmScratch::<C>::new(8).unwrap();
        scratch.scalars.0.fill(C::Scalar::from(101));
        panic!("injected unwind while guarded joined MSM scalars are live");
    }));
    assert!(unwound.is_err());
    let (count, zero) = take_clear_observations();
    assert!(count >= 10 && zero);
}

#[test]
fn both_pasta_inner_joined_msm_matches_original_reuses_storage_and_clears_success_and_unwind() {
    msm_parity::<EqAffine>();
    msm_parity::<EpAffine>();
}
