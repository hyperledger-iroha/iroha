//! Independent complete arithmetic AIR parity, field refusal and extension fallback.

use super::*;
use crate::privacy_engines::transparent_stark::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1 as E};

fn words<const N: usize>(seed: u64) -> [F; N] {
    let mut state = seed.wrapping_add(1);
    core::array::from_fn(|index| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        F(match index % 7 {
            0 => 0,
            1 => 1,
            2 => GOLDILOCKS_MODULUS_V1 - 1,
            _ => state % GOLDILOCKS_MODULUS_V1,
        })
    })
}
fn extension<const N: usize>(seed: u64) -> [E; N] {
    let columns = [
        words::<N>(seed),
        words(seed + 1),
        words(seed + 2),
        words(seed + 3),
    ];
    core::array::from_fn(|index| {
        E::from_coefficients(core::array::from_fn(|lane| columns[lane][index])).unwrap()
    })
}

#[test]
fn arithmetic_convolution_complete_native_and_fp4_residues_match_original() {
    for seed in 0..64 {
        let current = words(seed * 5);
        let next = words(seed * 5 + 1);
        let current_aux = words(seed * 5 + 2);
        let next_aux = words(seed * 5 + 3);
        let fixed = words(seed * 5 + 4);
        let expected = arithmetic_residues_original_for_test_v1(
            &current,
            &next,
            &current_aux,
            &next_aux,
            &fixed,
        )
        .unwrap();
        let actual = evaluate_p256_arithmetic_stark_residues_over_field_v1(
            &current,
            &next,
            &current_aux,
            &next_aux,
            &fixed,
        )
        .unwrap();
        assert_eq!(actual.len(), P256_ARITHMETIC_STARK_CONSTRAINT_COUNT_V1);
        assert_eq!(actual, expected, "base seed={seed}");
        if seed < 8 {
            let current = extension(seed * 5);
            let next = extension(seed * 5 + 1);
            let current_aux = extension(seed * 5 + 2);
            let next_aux = extension(seed * 5 + 3);
            let fixed = extension(seed * 5 + 4);
            assert_eq!(
                evaluate_p256_arithmetic_stark_residues_over_field_v1(
                    &current,
                    &next,
                    &current_aux,
                    &next_aux,
                    &fixed
                ),
                arithmetic_residues_original_for_test_v1(
                    &current,
                    &next,
                    &current_aux,
                    &next_aux,
                    &fixed
                ),
                "extension seed={seed}"
            );
        }
    }
}

#[test]
fn arithmetic_convolution_every_noncanonical_coordinate_preserves_refusal() {
    for kind in 0..5 {
        let width = match kind {
            0 | 1 => P256_ARITHMETIC_BASE_WIDTH_V1,
            2 | 3 => P256_ARITHMETIC_STARK_AUX_WIDTH_V1,
            _ => P256_ARITHMETIC_STARK_FIXED_WIDTH_V1,
        };
        for index in 0..width {
            let mut current = words(1);
            let mut next = words(2);
            let mut current_aux = words(3);
            let mut next_aux = words(4);
            let mut fixed = words(5);
            let target: &mut [F] = match kind {
                0 => &mut current,
                1 => &mut next,
                2 => &mut current_aux,
                3 => &mut next_aux,
                _ => &mut fixed,
            };
            target[index] = F(GOLDILOCKS_MODULUS_V1);
            let expected = arithmetic_residues_original_for_test_v1(
                &current,
                &next,
                &current_aux,
                &next_aux,
                &fixed,
            );
            assert!(expected.is_err());
            assert_eq!(
                evaluate_p256_arithmetic_stark_residues_over_field_v1(
                    &current,
                    &next,
                    &current_aux,
                    &next_aux,
                    &fixed
                ),
                expected
            );
            assert_eq!(
                target_value_v1(
                    kind,
                    index,
                    &current,
                    &next,
                    &current_aux,
                    &next_aux,
                    &fixed
                ),
                F(GOLDILOCKS_MODULUS_V1)
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn target_value_v1(
    kind: usize,
    index: usize,
    current: &[F],
    next: &[F],
    current_aux: &[F],
    next_aux: &[F],
    fixed: &[F],
) -> F {
    match kind {
        0 => current[index],
        1 => next[index],
        2 => current_aux[index],
        3 => next_aux[index],
        _ => fixed[index],
    }
}
