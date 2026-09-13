//! Dense independent transform oracles and exact geometry admission tests.
use super::super::GOLDILOCKS_MODULUS;
use super::*;

fn f(seed: u64) -> F {
    F::new([seed, seed + 3, seed + 7, seed + 11]).unwrap()
}
fn domain(rows: usize, offset: F) -> PolynomialDomain {
    PolynomialDomain::new(rows, offset, 524_288, usize::MAX).unwrap()
}
fn horner(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |sum, &value| sum.mul(point).add(value))
}

#[test]
fn full_extension_fft_and_inverse_match_dense_horner_at_every_small_point() {
    for rows in [1, 2, 4, 8, 16, 32] {
        for offset in [
            F::ONE,
            F::new([3, 0, 0, 0]).unwrap(),
            F::new([0, 1, 0, 0]).unwrap(),
            f(5),
        ] {
            let domain = domain(rows, offset);
            assert_eq!(domain.workspace_bytes(), 2 * rows * F::BYTES);
            let coefficients: Vec<_> = (0..rows).map(|i| f(17 + i as u64)).collect();
            let original = coefficients.clone();
            let lanes = domain.evaluate(&coefficients, rows).unwrap();
            let values: Vec<_> = (0..rows)
                .map(|i| {
                    let expected = horner(&coefficients, domain.point(i).unwrap());
                    assert_eq!(lanes.value(i).unwrap(), expected);
                    expected
                })
                .collect();
            let inverse = domain.interpolate(&values).unwrap();
            assert_eq!(&*inverse, &coefficients);
            let inverse_lanes = domain.interpolate_lanes(lanes).unwrap();
            assert_eq!(&*inverse_lanes, &coefficients);
            assert_eq!(coefficients, original);
        }
    }
}

#[test]
fn dense_inverse_dft_authenticates_each_extension_coordinate_and_padding() {
    let domain = domain(16, f(3));
    let values: Vec<_> = (0..16).map(|i| f(31 + 7 * i)).collect();
    let result = domain.interpolate(&values).unwrap();
    let inverse_size = F::embed_base(16).inverse().unwrap();
    let inverse_generator = F::embed_base(domain.generator()).inverse().unwrap();
    let inverse_offset = domain.point(0).unwrap().inverse().unwrap();
    for degree in 0..16 {
        let coefficient = values
            .iter()
            .enumerate()
            .fold(F::ZERO, |sum, (i, &value)| {
                sum.add(value.mul(inverse_generator.power((degree * i) as u64)))
            })
            .mul(inverse_size)
            .mul(inverse_offset.power(degree as u64));
        assert_eq!(result[degree], coefficient);
    }
    let mut coefficients = vec![F::ZERO; 16];
    coefficients[0] = f(7);
    coefficients[2] = f(11);
    let recovered = domain
        .interpolate_lanes(domain.evaluate(&coefficients, 3).unwrap())
        .unwrap();
    assert_eq!(&*recovered, &coefficients);
    assert!(recovered[3..].iter().all(|&v| v == F::ZERO));
}

#[test]
fn exact_domain_binding_and_rotated_next_points_include_wraparound() {
    let domain = domain(32, f(7));
    let rotation = domain.numerator_rotation(8).unwrap();
    assert_eq!(rotation, 4);
    let omega = FixedTraceDomain::new(&FASTPQ_FINAL_V1, 8)
        .unwrap()
        .generator;
    for index in 0..32 {
        assert_eq!(
            domain.point((index + rotation) % 32).unwrap(),
            domain.point(index).unwrap().mul_base(omega)
        );
    }
    let coefficients = [f(2), f(3)];
    let lanes = domain.evaluate(&coefficients, 2).unwrap();
    assert!(self::domain(32, f(8)).interpolate_lanes(lanes).is_err());
    assert!(self::domain(32, F::ONE).numerator_rotation(8).is_err());
    assert!(domain.numerator_rotation(64).is_err());
    assert!(domain.numerator_rotation(3).is_err());
    assert!(domain.point(32).is_err());
    assert!(
        domain
            .evaluate(&coefficients, 2)
            .unwrap()
            .value(32)
            .is_err()
    );
}

#[test]
fn malformed_coordinates_extent_high_terms_and_policy_fail_before_transform() {
    let domain = domain(8, f(3));
    assert!(domain.evaluate(&[], 0).is_err());
    assert!(domain.evaluate(&[F::ZERO; 9], 0).is_err());
    assert!(domain.evaluate(&[F::ZERO; 8], 9).is_err());
    assert!(domain.interpolate(&[F::ZERO; 7]).is_err());
    let mut values = [F::ZERO; 8];
    values[7] = F::ONE;
    assert!(domain.evaluate(&values, 7).is_err());
    assert_eq!(
        domain.evaluate(&values, 8).unwrap().value(0).unwrap(),
        domain.point(0).unwrap().power(7)
    );
    for coordinate in 0..4 {
        let mut words = [0; 4];
        words[coordinate] = GOLDILOCKS_MODULUS;
        let bad = F::from_coefficients_unchecked_for_test(words);
        let mut values = [F::ZERO; 8];
        values[7] = bad;
        assert!(domain.evaluate(&values, 8).is_err());
        assert!(domain.interpolate(&values).is_err());
        assert!(PolynomialDomain::new(8, bad, 8, usize::MAX).is_err());
    }
    for rows in [0, 3, 1_048_576] {
        assert!(PolynomialDomain::new(rows, F::ONE, usize::MAX, usize::MAX).is_err());
    }
    assert!(PolynomialDomain::new(8, F::ZERO, 8, usize::MAX).is_err());
    assert!(PolynomialDomain::new(8, f(3), 7, usize::MAX).is_err());
    assert!(PolynomialDomain::new(8, f(3), 8, 8 * 2 * F::BYTES - 1).is_err());
    assert!(reserved::<F>(usize::MAX).is_err());
}
