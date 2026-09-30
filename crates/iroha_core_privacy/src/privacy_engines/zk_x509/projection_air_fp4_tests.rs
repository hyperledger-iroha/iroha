//! Independent polynomial lifting, degree and malformed-projection checks.

use super::*;
use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;

fn challenges() -> ZkX509ProjectionChallengesV1 {
    ZkX509ProjectionChallengesV1 {
        copy: core::array::from_fn(|lane| ZkX509ProjectionCopyChallengesV1 {
            beta: F(17 * lane as u64 + 2),
            gamma: F(17 * lane as u64 + 3),
        }),
        compaction: core::array::from_fn(|lane| ZkX509ProjectionCompactionChallengesV1 {
            active: F(17 * lane as u64 + 5),
            invocation: F(17 * lane as u64 + 7),
            position: F(17 * lane as u64 + 11),
            value: F(17 * lane as u64 + 13),
            gamma: F(17 * lane as u64 + 17),
        }),
    }
}

fn rows() -> [Vec<E>; 5] {
    let mut index = 1;
    [
        ZK_X509_PROJECTION_BASE_WIDTH_V1,
        ZK_X509_PROJECTION_BASE_WIDTH_V1,
        ZK_X509_PROJECTION_AUX_WIDTH_V1,
        ZK_X509_PROJECTION_AUX_WIDTH_V1,
        ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1,
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
    evaluate_zk_x509_projection_stark_residues_v1(
        rows[0].as_slice().try_into().unwrap(),
        rows[1].as_slice().try_into().unwrap(),
        rows[2].as_slice().try_into().unwrap(),
        rows[3].as_slice().try_into().unwrap(),
        rows[4].as_slice().try_into().unwrap(),
        challenges(),
    )
    .unwrap()
}

#[test]
fn projection_fp4_matches_independent_base_polynomial_interpolation() {
    let rows = rows();
    let actual = evaluate(&rows);
    assert_eq!(actual.len(), ZK_X509_PROJECTION_STARK_CONSTRAINT_COUNT_V1);
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut expected = vec![E::ZERO; actual.len()];
    // Cubic input polynomials and degree-four AIR require thirteen samples.
    // Base-only evaluation followed by Lagrange interpolation avoids using the
    // extension AIR's multiplication as the reference calculation.
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
        let evaluated = evaluate(&base);
        let mut numerator = E::ONE;
        let mut denominator = F::ONE;
        for other in 0..13 {
            if other != sample {
                numerator = numerator.mul(w.sub(E::from_base(F(other))));
                denominator = denominator.mul(t.sub(F(other)));
            }
        }
        let weight = numerator.mul_base(denominator.inv().unwrap());
        for (sum, value) in expected.iter_mut().zip(&evaluated) {
            *sum = sum.add(weight.mul_base(*value));
        }
        if sample == 0 {
            let embedded = base
                .each_ref()
                .map(|row| row.iter().copied().map(E::from_base).collect());
            assert_eq!(
                evaluate(&embedded),
                evaluated.into_iter().map(E::from_base).collect::<Vec<_>>()
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
fn projection_total_degree_is_exactly_four_including_fixed_selectors() {
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
    for order in 1..=5 {
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
        if order == 4 {
            assert!(samples.iter().flatten().any(|value| *value != F::ZERO));
        }
    }
    assert!(samples[0].iter().all(|value| *value == F::ZERO));
    assert_eq!(ZK_X509_PROJECTION_STARK_CONSTRAINT_DEGREE_V1, 4);
}

#[test]
fn projection_rejects_noncanonical_fields_and_changed_public_digest() {
    let mut current = [F::ZERO; ZK_X509_PROJECTION_BASE_WIDTH_V1];
    let next = current;
    let aux = [F::ZERO; ZK_X509_PROJECTION_AUX_WIDTH_V1];
    let mut fixed = [F::ZERO; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1];
    fixed[FIX_ACTIVE] = F::ONE;
    fixed[FIX_DIGEST] = F::ONE;
    fixed[FIX_EXPECTED_BYTE] = F(5);
    current[USED] = F::ONE;
    current[VALUE] = F(5);
    current[VALUE_BITS] = F::ONE;
    current[VALUE_BITS + 2] = F::ONE;
    let honest = evaluate_zk_x509_projection_stark_residues_v1(
        &current,
        &next,
        &aux,
        &aux,
        &fixed,
        challenges(),
    )
    .unwrap();
    assert!(honest.iter().all(|residue| *residue == F::ZERO));
    fixed[FIX_EXPECTED_BYTE] = F(6);
    let changed = evaluate_zk_x509_projection_stark_residues_v1(
        &current,
        &next,
        &aux,
        &aux,
        &fixed,
        challenges(),
    )
    .unwrap();
    assert!(changed.iter().any(|residue| *residue != F::ZERO));
    current[VALUE] = F(GOLDILOCKS_MODULUS_V1);
    assert_eq!(
        evaluate_zk_x509_projection_stark_residues_v1(
            &current,
            &next,
            &aux,
            &aux,
            &fixed,
            challenges(),
        ),
        Err(ZkX509ProjectionAirErrorV1::NonCanonicalField)
    );
}
