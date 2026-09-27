//! Polynomial extension and adversarial checks for the complete MAIN byte-memory AIR.

use super::*;
use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
use crate::privacy_engines::transparent_stark::PolynomialAirFieldV1;

fn challenges() -> ZkX509IoChallengesV1 {
    ZkX509IoChallengesV1 {
        lanes: core::array::from_fn(|lane| super::super::io_air::ZkX509IoLaneChallengesV1 {
            beta: F(13 * lane as u64 + 2),
            channel: F(13 * lane as u64 + 3),
            offset: F(13 * lane as u64 + 5),
            value: F(13 * lane as u64 + 7),
            is_write: F(13 * lane as u64 + 11),
        }),
    }
}
fn rows() -> [Vec<E>; 5] {
    let mut index = 1;
    [
        IO_BASE_WIDTH,
        IO_BASE_WIDTH,
        IO_AUX_WIDTH,
        IO_AUX_WIDTH,
        IO_FIXED_WIDTH,
    ]
    .map(|width| {
        (0..width)
            .map(|_| {
                index += 1;
                E::canonical([index, index + 3, index + 5, index + 7]).unwrap()
            })
            .collect()
    })
}
fn evaluate<A: PolynomialAirFieldV1>(rows: &[Vec<A>; 5]) -> Vec<A> {
    io_constraint_residues_v1(
        SegmentLayoutV1::for_full_io().unwrap(),
        17,
        &rows[0],
        &rows[1],
        &rows[2],
        &rows[3],
        &rows[4],
        challenges(),
    )
    .unwrap()
}

#[test]
fn io_fp4_matches_independent_base_interpolation_and_embedding() {
    let rows = rows();
    let actual = evaluate(&rows);
    assert_eq!(actual.len(), IO_CONSTRAINT_COUNT);
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut expected = vec![E::ZERO; IO_CONSTRAINT_COUNT];
    // Use the unchanged conservative degree-four profile, including fixed
    // selectors, and cubic coefficient inputs: thirteen base samples suffice.
    for sample in 0..13 {
        let t = F(sample);
        let base = rows.each_ref().map(|row| {
            row.iter()
                .map(|value| {
                    value
                        .coefficients()
                        .iter()
                        .rev()
                        .fold(F::ZERO, |sum, c| sum.mul(t).add(*c))
                })
                .collect::<Vec<_>>()
        });
        let values = evaluate(&base);
        let mut numerator = E::ONE;
        let mut denominator = F::ONE;
        for other in 0..13 {
            if other != sample {
                numerator = numerator.mul(w.sub(E::from_base(F(other))));
                denominator = denominator.mul(t.sub(F(other)));
            }
        }
        let weight = numerator.mul_base(denominator.inv().unwrap());
        for (sum, value) in expected.iter_mut().zip(&values) {
            *sum = sum.add(weight.mul_base(*value));
        }
        if sample == 0 {
            let embedded = base
                .each_ref()
                .map(|row| row.iter().copied().map(E::from_base).collect());
            assert_eq!(
                evaluate(&embedded),
                values.into_iter().map(E::from_base).collect::<Vec<_>>()
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
fn io_fp4_rejects_bad_dimensions_and_binds_logical_extent() {
    let rows = rows();
    let layout = SegmentLayoutV1::for_full_io().unwrap();
    let baseline = evaluate(&rows);
    let changed = io_constraint_residues_v1(
        layout,
        18,
        &rows[0],
        &rows[1],
        &rows[2],
        &rows[3],
        &rows[4],
        challenges(),
    )
    .unwrap();
    assert_ne!(baseline, changed);
    for width in 0..5 {
        let mut malformed = rows.clone();
        malformed[width].pop();
        assert!(
            io_constraint_residues_v1(
                layout,
                17,
                &malformed[0],
                &malformed[1],
                &malformed[2],
                &malformed[3],
                &malformed[4],
                challenges()
            )
            .is_err()
        );
    }
    for logical in [0, ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1] {
        assert!(
            io_constraint_residues_v1(
                layout,
                logical,
                &rows[0],
                &rows[1],
                &rows[2],
                &rows[3],
                &rows[4],
                challenges()
            )
            .is_err()
        );
    }
    let mut invalid_challenges = challenges();
    invalid_challenges.lanes[1] = invalid_challenges.lanes[0];
    assert!(
        io_constraint_residues_v1(
            layout,
            17,
            &rows[0],
            &rows[1],
            &rows[2],
            &rows[3],
            &rows[4],
            invalid_challenges
        )
        .is_err()
    );
    let mut base = rows.each_ref().map(|row| {
        row.iter()
            .map(|value| value.coefficients()[0])
            .collect::<Vec<_>>()
    });
    base[0][EXEC_VALUE] = F(GOLDILOCKS_MODULUS_V1);
    assert!(
        io_constraint_residues_v1(
            layout,
            17,
            &base[0],
            &base[1],
            &base[2],
            &base[3],
            &base[4],
            challenges()
        )
        .is_err()
    );
}

#[test]
fn io_degree_bound_covers_fixed_selectors_and_all_continuations() {
    let rows = rows();
    let mut samples = (0..6)
        .map(|sample| {
            let t = F(sample);
            let base = rows.each_ref().map(|row| {
                row.iter()
                    .map(|value| {
                        let c = value.coefficients();
                        c[0].add(c[1].mul(t))
                    })
                    .collect::<Vec<_>>()
            });
            evaluate(&base)
        })
        .collect::<Vec<_>>();
    for _ in 0..=IO_CONSTRAINT_DEGREE {
        samples = samples
            .windows(2)
            .map(|pair| {
                pair[0]
                    .iter()
                    .zip(&pair[1])
                    .map(|(left, right)| right.sub(*left))
                    .collect()
            })
            .collect();
    }
    assert_eq!(IO_CONSTRAINT_DEGREE, 4);
    assert!(samples[0].iter().all(|value| *value == F::ZERO));
}
