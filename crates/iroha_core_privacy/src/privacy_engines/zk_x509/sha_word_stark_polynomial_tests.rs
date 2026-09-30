//! Independent polynomial checks for fixed-capacity SHA selectors and degree.

use super::*;
use crate::privacy_engines::transparent_stark::{
    GoldilocksFp4V1 as E, goldilocks_ifft_v1, goldilocks_primitive_root_v1,
};

fn challenges() -> ZkX509ShaWordStarkChallengesV1 {
    ZkX509ShaWordStarkChallengesV1 {
        memory: ZkX509WordMemoryChallengesV1 {
            lanes: core::array::from_fn(|lane| {
                let start = 3 + 8 * lane as u64;
                ZkX509WordMemoryLaneChallengesV1 {
                    beta: F(start),
                    address: F(start + 2),
                    value: F(start + 4),
                    is_write: F(start + 6),
                }
            }),
        },
        base_folding: [F(101), F(103), F(107), F(109)],
    }
}

fn evaluate<A: PolynomialAirFieldV1>(rows: &[Vec<A>; 5]) -> Vec<A> {
    evaluate_zk_x509_sha_word_capacity_residues_over_field_v1(
        rows[0].as_slice().try_into().unwrap(),
        rows[1].as_slice().try_into().unwrap(),
        rows[2].as_slice().try_into().unwrap(),
        rows[3].as_slice().try_into().unwrap(),
        rows[4].as_slice().try_into().unwrap(),
        challenges(),
    )
    .unwrap()
}

fn rows<A: PolynomialAirFieldV1>(mut cell: impl FnMut(usize) -> A) -> [Vec<A>; 5] {
    let mut index = 0;
    [
        SHA_WORD_CAPACITY_BASE_WIDTH_V1,
        SHA_WORD_CAPACITY_BASE_WIDTH_V1,
        SHA_WORD_CAPACITY_AUX_WIDTH_V1,
        SHA_WORD_CAPACITY_AUX_WIDTH_V1,
        SHA_WORD_CAPACITY_FIXED_WIDTH_V1,
    ]
    .map(|width| {
        (0..width)
            .map(|_| {
                index += 1;
                cell(index)
            })
            .collect()
    })
}

fn horner(coefficients: &[F], point: E) -> E {
    coefficients.iter().rev().fold(E::ZERO, |sum, &value| {
        sum.mul(point).add(E::from_base(value))
    })
}

#[test]
fn digest_address_selection_agrees_with_native_schedule_and_interpolated_lde() {
    // A verifier-owned one-hot native selector; independent IFFT and polynomial
    // multiplication construct its off-domain event-pair equation.
    let native = [
        F::ZERO,
        F::ONE,
        F::ZERO,
        F::ZERO,
        F::ONE,
        F::ZERO,
        F::ZERO,
        F::ZERO,
    ];
    let root = goldilocks_primitive_root_v1(3).unwrap();
    let mut selector_coefficients = native;
    goldilocks_ifft_v1(&mut selector_coefficients, root).unwrap();
    let challenge = challenges().memory.lanes[0];
    let constant = challenge
        .beta
        .add(challenge.address.mul(F(19)))
        .add(challenge.value.mul(F(11)));
    let quadratic = challenge.address.mul(F(37).sub(F(19)));
    let mut expected_coefficients = vec![F::ZERO; 15];
    for (i, &left) in selector_coefficients.iter().enumerate() {
        expected_coefficients[i] = expected_coefficients[i].sub(constant.mul(left));
        for (j, &right) in selector_coefficients.iter().enumerate() {
            expected_coefficients[i + j] =
                expected_coefficients[i + j].sub(quadratic.mul(left).mul(right));
        }
    }
    let mut points = (0..8)
        .map(|index| E::from_base(root.pow(index)))
        .collect::<Vec<_>>();
    points.extend([E::from_base(F(7)), E::canonical([5, 1, 2, 3]).unwrap()]);
    for (index, point) in points.into_iter().enumerate() {
        let digest = horner(&selector_coefficients, point);
        let mut input = rows(|_| E::ZERO);
        input[0][0] = E::from_base(F(11));
        input[0][SHA_WORD_CAPACITY_ROW_ACTIVE_V1] = E::ONE;
        input[0][SHA_WORD_CAPACITY_DYNAMIC_ADDRESS_V1] = E::from_base(F(37));
        input[4][FIX_DIGEST] = digest;
        input[4][FIX_EVENT_ADDRESS] = E::from_base(F(19));
        let actual = evaluate(&input)[SHA_WORD_COPY_LANES_V1];
        assert_eq!(actual, horner(&expected_coefficients, point));
        if index < native.len() {
            assert_eq!(digest, E::from_base(native[index]));
            let address = if native[index] == F::ONE {
                F(37)
            } else {
                F(19)
            };
            let expected = F::ZERO.sub(
                native[index].mul(
                    challenge
                        .beta
                        .add(challenge.address.mul(address))
                        .add(challenge.value.mul(F(11))),
                ),
            );
            assert_eq!(actual, E::from_base(expected));
        } else {
            assert_ne!(digest, E::ZERO);
            assert_ne!(digest, E::ONE);
            let former_branch = E::ZERO.sub(digest.mul_base(constant));
            assert_ne!(
                actual, former_branch,
                "non-Boolean selector cannot use the fixed-address branch"
            );
        }
    }
}

#[test]
fn complete_capacity_fp4_residues_match_independent_polynomial_lifting() {
    let input = rows(|index| {
        let i = index as u64;
        E::canonical([i + 1, i + 3, i + 5, i + 7]).unwrap()
    });
    let actual = evaluate(&input);
    assert_eq!(actual.len(), SHA_WORD_CAPACITY_CONSTRAINT_COUNT_V1);
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut expected = vec![E::ZERO; actual.len()];
    // Each input is cubic in w and every residue has total degree at most six.
    // Nineteen independent F evaluations determine that degree-18 polynomial.
    for sample in 0..19 {
        let t = F(sample);
        let lifted = input.each_ref().map(|row| {
            row.iter()
                .map(|value| {
                    value
                        .coefficients()
                        .iter()
                        .rev()
                        .fold(F::ZERO, |sum, &value| sum.mul(t).add(value))
                })
                .collect::<Vec<_>>()
        });
        let scalar = evaluate_zk_x509_sha_word_capacity_residues_v1(
            lifted[0].as_slice().try_into().unwrap(),
            lifted[1].as_slice().try_into().unwrap(),
            lifted[2].as_slice().try_into().unwrap(),
            lifted[3].as_slice().try_into().unwrap(),
            lifted[4].as_slice().try_into().unwrap(),
            challenges(),
        )
        .unwrap();
        let mut numerator = E::ONE;
        let mut denominator = F::ONE;
        for other in 0..19 {
            if other != sample {
                numerator = numerator.mul(w.sub(E::from_base(F(other))));
                denominator = denominator.mul(t.sub(F(other)));
            }
        }
        let weight = numerator.mul_base(denominator.inv().unwrap());
        for (sum, &value) in expected.iter_mut().zip(&scalar) {
            *sum = sum.add(weight.mul_base(value));
        }
        if sample == 0 {
            let embedded = lifted
                .each_ref()
                .map(|row| row.iter().copied().map(E::from_base).collect());
            assert_eq!(
                evaluate(&embedded),
                scalar.into_iter().map(E::from_base).collect::<Vec<_>>()
            );
        }
    }
    assert_eq!(actual, expected);
    assert!(
        actual
            .iter()
            .any(|value| value.coefficients()[1] != F::ZERO)
    );
}

#[test]
fn capacity_total_degree_is_six_including_fixed_selectors() {
    assert_eq!(SHA_WORD_CAPACITY_CONSTRAINT_DEGREE_V1, 6);
    // A line through every private and fixed coordinate detects hidden selector
    // branches as well as degrees larger than the declared bound.
    let mut samples = (0..9)
        .map(|sample| {
            let t = F(sample);
            let mut input = rows(|index| F(index as u64 + 1).add(t.mul(F(index as u64 + 3))));
            input[4][FIX_DIGEST] = t;
            evaluate(&input)
        })
        .collect::<Vec<_>>();
    for _ in 0..7 {
        samples = samples
            .windows(2)
            .map(|pair| {
                pair[1]
                    .iter()
                    .zip(&pair[0])
                    .map(|(&right, &left)| right.sub(left))
                    .collect()
            })
            .collect();
    }
    assert!(samples.iter().flatten().all(|&value| value == F::ZERO));

    // The Boolean first-pair equation has leading coefficient -address^2:
    // (choose*active*first) * -(beta+address*digest*dynamic)*(beta+address*addr1).
    let mut pair_samples = (0..7)
        .map(|sample| {
            let t = F(sample);
            let mut input = rows(|_| F::ZERO);
            input[0][SHA_WORD_CAPACITY_ROW_ACTIVE_V1] = t;
            input[0][SHA_WORD_CAPACITY_DYNAMIC_ADDRESS_V1] = t;
            input[4][FIX_DIGEST] = t;
            input[4][FIX_CHOOSE] = t;
            input[4][FIX_BOOLEAN_FIRST] = t;
            input[4][FIX_EVENT_ADDRESS + 1] = t;
            evaluate(&input)[SHA_WORD_COPY_LANES_V1]
        })
        .collect::<Vec<_>>();
    for _ in 0..6 {
        pair_samples = pair_samples
            .windows(2)
            .map(|pair| pair[1].sub(pair[0]))
            .collect();
    }
    let address = challenges().memory.lanes[0].address;
    assert_eq!(
        pair_samples,
        [F::ZERO.sub(F(720).mul(address).mul(address))]
    );
    assert_ne!(pair_samples[0], F::ZERO);
}

#[test]
fn capacity_digest_address_stays_bound_when_local_event_pair_is_rewritten() {
    let challenges = challenges();
    let mut trace =
        build_sha_word_capacity_trace_v1(b"abc", 127, false, challenges.memory).unwrap();
    let digest_row = trace.maximum_local_rows - SHA_WORD_CAPACITY_LOCAL_ROWS_PER_CALL_V1;
    let next = digest_row + 1;
    let evaluate_trace = |trace: &ZkX509ShaWordCapacityTraceV1| {
        evaluate_zk_x509_sha_word_capacity_residues_v1(
            &trace.base_rows[digest_row],
            &trace.base_rows[next],
            &trace.aux_rows[digest_row],
            &trace.aux_rows[next],
            &trace.fixed_rows[digest_row],
            challenges,
        )
        .unwrap()
    };
    assert!(evaluate_trace(&trace).iter().all(|&value| value == F::ZERO));
    let wrong_address =
        trace.base_rows[digest_row][SHA_WORD_CAPACITY_DYNAMIC_ADDRESS_V1].add(F::ONE);
    trace.base_rows[digest_row][SHA_WORD_CAPACITY_DYNAMIC_ADDRESS_V1] = wrong_address;
    for (lane, challenge) in challenges.memory.lanes.iter().enumerate() {
        trace.aux_rows[digest_row][LOCAL_PAIR_01 + lane] = challenge
            .beta
            .add(challenge.address.mul(wrong_address))
            .add(challenge.value.mul(trace.base_rows[digest_row][0]));
    }
    let malformed = evaluate_trace(&trace);
    assert_eq!(
        malformed[SHA_WORD_COPY_LANES_V1],
        F::ZERO,
        "rewritten local pair alone accepts the false address"
    );
    assert!(
        malformed[SHA_WORD_STARK_CONSTRAINT_COUNT_V1..]
            .iter()
            .any(|&value| value != F::ZERO),
        "capacity relation independently binds the address to the final active block"
    );
    let mut malformed_fixed = trace.fixed_rows[digest_row];
    malformed_fixed[FIX_DIGEST] = F(GOLDILOCKS_MODULUS_V1);
    assert_eq!(
        evaluate_zk_x509_sha_word_capacity_residues_v1(
            &trace.base_rows[digest_row],
            &trace.base_rows[next],
            &trace.aux_rows[digest_row],
            &trace.aux_rows[next],
            &malformed_fixed,
            challenges,
        ),
        Err(ZkX509ShaWordStarkErrorV1::Topology)
    );
}
