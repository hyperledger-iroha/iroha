//! Independent coefficient-oracle, canonicality and scheduling checks for FRI.

use super::*;
use fastpq_isi::FASTPQ_FINAL_V1;

fn root(length: usize) -> u64 {
    assert!(length.is_power_of_two());
    field_pow(
        FASTPQ_FINAL_V1.lde_root,
        1_u64 << (FASTPQ_FINAL_V1.lde_log_size - length.ilog2()),
    )
}

fn coefficients(length: usize) -> Vec<GoldilocksFp4V1> {
    (0..length)
        .map(|index| {
            let index = index as u64;
            GoldilocksFp4V1::new([
                7 + index * 13,
                11 + index * 17,
                19 + index * 23,
                29 + index * 31,
            ])
            .unwrap()
        })
        .collect()
}

fn evaluate(coefficients: &[GoldilocksFp4V1], point: u64) -> GoldilocksFp4V1 {
    coefficients
        .iter()
        .rev()
        .fold(GoldilocksFp4V1::ZERO, |value, &coefficient| {
            value.mul_base(point).add(coefficient)
        })
}

// This oracle starts from polynomial coefficients and never interpolates the
// fiber or uses a domain inverse, twiddle table, bit reversal or butterfly.
fn folded_coefficients(
    coefficients: &[GoldilocksFp4V1],
    arity: usize,
    challenge: GoldilocksFp4V1,
) -> Vec<GoldilocksFp4V1> {
    coefficients
        .chunks(arity)
        .map(|chunk| {
            let mut result = GoldilocksFp4V1::ZERO;
            let mut power = GoldilocksFp4V1::ONE;
            for &coefficient in chunk {
                result = result.add(coefficient.mul(power));
                power = power.mul(challenge);
            }
            result
        })
        .collect()
}

#[test]
fn every_arity_matches_independent_coefficients_with_all_challenge_coordinates() {
    for arity in [2, 4, 8, 16] {
        let generator = root(arity);
        let plan = FriFoldPlan::new(arity, generator).unwrap();
        for length in [1, arity - 1, arity, arity + 1, 4 * arity + 3] {
            let coefficients = coefficients(length);
            for x in [1, 7, 1_u64 << 32, GOLDILOCKS_MODULUS - 1] {
                let values: Vec<_> = (0..arity)
                    .map(|index| {
                        evaluate(
                            &coefficients,
                            mul_mod(x, field_pow(generator, index as u64)),
                        )
                    })
                    .collect();
                for challenge in [
                    GoldilocksFp4V1::ZERO,
                    GoldilocksFp4V1::ONE,
                    GoldilocksFp4V1::new([0, 1, 0, 0]).unwrap(),
                    GoldilocksFp4V1::new([0, 0, 1, 0]).unwrap(),
                    GoldilocksFp4V1::new([0, 0, 0, 1]).unwrap(),
                    GoldilocksFp4V1::new([GOLDILOCKS_MODULUS - 1, 37, 41, 43]).unwrap(),
                ] {
                    let expected = evaluate(
                        &folded_coefficients(&coefficients, arity, challenge),
                        field_pow(x, arity as u64),
                    );
                    assert_eq!(plan.fold_coset(&values, challenge, x).unwrap(), expected);
                }
            }
        }
    }
}

#[test]
fn binary_folding_preserves_the_existing_owner() {
    let plan = FriFoldPlan::new(2, GOLDILOCKS_MODULUS - 1).unwrap();
    let values = coefficients(2);
    for challenge in coefficients(8) {
        for x in [1, 7, 101, GOLDILOCKS_MODULUS - 1] {
            assert_eq!(
                plan.fold_coset(&values, challenge, x).unwrap(),
                crate::backend::fold_fri_coset(&values, challenge, x, GOLDILOCKS_MODULUS - 1,)
                    .unwrap()
            );
        }
    }
}

#[test]
fn root_order_and_fiber_shape_are_checked_before_arithmetic() {
    for arity in [0, 1, 3, 5, 32, usize::MAX] {
        assert!(FriFoldPlan::new(arity, 1).is_err());
    }
    for arity in [2, 4, 8, 16] {
        for generator in [0, 1, GOLDILOCKS_MODULUS, u64::MAX] {
            assert!(FriFoldPlan::new(arity, generator).is_err());
        }
        if arity > 2 {
            assert!(FriFoldPlan::new(arity, root(arity / 2)).is_err());
        }
        let plan = FriFoldPlan::new(arity, root(arity)).unwrap();
        for length in [0, arity - 1, arity + 1] {
            assert!(
                plan.fold_coset(&coefficients(length), GoldilocksFp4V1::ONE, 7)
                    .is_err()
            );
        }
        for x in [0, GOLDILOCKS_MODULUS, u64::MAX] {
            assert!(
                plan.fold_coset(&coefficients(arity), GoldilocksFp4V1::ONE, x)
                    .is_err()
            );
        }
    }
}

#[test]
fn each_malformed_field_coordinate_is_rejected() {
    let plan = FriFoldPlan::new(16, root(16)).unwrap();
    for lane in 0..4 {
        let mut words = [1; 4];
        words[lane] = GOLDILOCKS_MODULUS;
        let bad = GoldilocksFp4V1::from_coefficients_unchecked_for_test(words);
        assert!(matches!(
            plan.fold_coset(&coefficients(16), bad, 7),
            Err(Error::NonCanonicalGoldilocksElement { context: "fri_fold_challenge", indices }) if indices == [lane]
        ));
        for position in 0..16 {
            let mut values = coefficients(16);
            values[position] = bad;
            assert!(matches!(
                plan.fold_coset(&values, GoldilocksFp4V1::ONE, 7),
                Err(Error::NonCanonicalGoldilocksElement { context: "fri_fold_value", indices }) if indices == [position, lane]
            ));
        }
    }
}

#[test]
fn layer_fibers_and_degree_reduction_match_independent_coefficients() {
    for arity in [2, 4, 8, 16] {
        let length = 16 * arity;
        let domain = FriDomain {
            generator: root(length),
            offset: 7,
        };
        let plan = FriFoldPlan::new(arity, domain.coset_generator(16)).unwrap();
        let coefficients = coefficients(9 * arity - 1);
        let challenge = GoldilocksFp4V1::new([31, 37, 41, 43]).unwrap();
        let input: Vec<_> = (0..length)
            .map(|index| evaluate(&coefficients, domain.point(index)))
            .collect();
        let mut output = vec![GoldilocksFp4V1::ZERO; 16];
        plan.fold_layer_into(&input, challenge, domain, &mut output)
            .unwrap();
        let expected = folded_coefficients(&coefficients, arity, challenge);
        assert_eq!(expected.len(), coefficients.len().div_ceil(arity));
        let next_domain = domain.folded(arity);
        for (index, actual) in output.iter().enumerate() {
            assert_eq!(*actual, evaluate(&expected, next_domain.point(index)));
        }
        assert!(
            next_domain
                .evaluations_have_degree_below(&output, expected.len())
                .unwrap()
        );
        assert!(
            !next_domain
                .evaluations_have_degree_below(&output, expected.len() - 1)
                .unwrap()
        );
    }
}

#[test]
fn supported_fold_schedule_reaches_the_constant_terminal_code() {
    let mut domain = FriDomain {
        generator: root(1 << 17),
        offset: 7,
    };
    let mut coefficients = coefficients(7);
    let mut values: Vec<_> = (0..1 << 17)
        .map(|index| evaluate(&coefficients, domain.point(index)))
        .collect();
    for arity in [16, 16, 8, 8, 4] {
        let plan = FriFoldPlan::new(arity, domain.coset_generator(values.len() / arity)).unwrap();
        let challenge = GoldilocksFp4V1::new([arity as u64, 37, 41, 43]).unwrap();
        let mut next = vec![GoldilocksFp4V1::ZERO; values.len() / arity];
        plan.fold_layer_into(&values, challenge, domain, &mut next)
            .unwrap();
        domain = domain.folded(arity);
        coefficients = folded_coefficients(&coefficients, arity, challenge);
        for (index, actual) in next.iter().enumerate() {
            assert_eq!(*actual, evaluate(&coefficients, domain.point(index)));
        }
        values = next;
    }
    assert_eq!(coefficients.len(), 1);
    assert_eq!(values.len(), 2);
    assert_eq!(values, vec![coefficients[0]; 2]);
}

#[test]
fn layer_refusal_leaves_caller_storage_unchanged() {
    let domain = FriDomain {
        generator: root(64),
        offset: 7,
    };
    let plan = FriFoldPlan::new(4, domain.coset_generator(16)).unwrap();
    let values = coefficients(64);
    let sentinel = GoldilocksFp4V1::new([13, 17, 19, 23]).unwrap();
    let mut output = vec![sentinel; 16];
    let mut reject = |input: &[GoldilocksFp4V1], challenge, domain| {
        assert!(
            plan.fold_layer_into(input, challenge, domain, &mut output)
                .is_err()
        );
        assert_eq!(output, vec![sentinel; 16]);
    };
    reject(&[], GoldilocksFp4V1::ONE, domain);
    reject(&values[..63], GoldilocksFp4V1::ONE, domain);
    reject(&values[..32], GoldilocksFp4V1::ONE, domain);
    for bad in [0, GOLDILOCKS_MODULUS, u64::MAX] {
        reject(
            &values,
            GoldilocksFp4V1::ONE,
            FriDomain {
                offset: bad,
                ..domain
            },
        );
        reject(
            &values,
            GoldilocksFp4V1::ONE,
            FriDomain {
                generator: bad,
                ..domain
            },
        );
    }
    reject(
        &values,
        GoldilocksFp4V1::ONE,
        FriDomain {
            generator: root(32),
            ..domain
        },
    );
    reject(
        &values,
        GoldilocksFp4V1::ONE,
        FriDomain {
            generator: field_inverse(domain.generator),
            ..domain
        },
    );
    for lane in 0..4 {
        let mut words = [1; 4];
        words[lane] = GOLDILOCKS_MODULUS;
        let bad = GoldilocksFp4V1::from_coefficients_unchecked_for_test(words);
        reject(&values, bad, domain);
        let mut input = values.clone();
        input[63] = bad;
        reject(&input, GoldilocksFp4V1::ONE, domain);
    }
}

#[test]
fn parallel_folds_are_identical_on_one_and_four_workers() {
    for arity in [2, 4, 8, 16] {
        let length = arity * PARALLEL_OUTPUT_THRESHOLD;
        let domain = FriDomain {
            generator: root(length),
            offset: 7,
        };
        let plan = FriFoldPlan::new(arity, domain.coset_generator(length / arity)).unwrap();
        let values = coefficients(length);
        let challenge = GoldilocksFp4V1::new([31, 37, 41, 43]).unwrap();
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let mut output = vec![GoldilocksFp4V1::ZERO; length / arity];
                    plan.fold_layer_into(&values, challenge, domain, &mut output)
                        .unwrap();
                    output
                })
        };
        let output = run(1);
        assert_eq!(output, run(4));
        for (index, &actual) in output.iter().enumerate() {
            let fiber: Vec<_> = (0..arity)
                .map(|position| values[index + position * output.len()])
                .collect();
            assert_eq!(
                actual,
                plan.fold_coset(&fiber, challenge, domain.point(index))
                    .unwrap()
            );
        }
    }
}
