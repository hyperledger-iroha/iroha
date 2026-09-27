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
fn explicit_deep_transform_binds_full_domain_existing_subgroup_and_payload_cap() {
    use super::super::deep_geometry::{DeepGeometry, LDE_ROWS, TRACE_ROWS};
    let geometry = DeepGeometry::new().unwrap();
    let bytes = 2 * LDE_ROWS * F::BYTES;
    let large = PolynomialDomain::for_deep(&geometry, bytes).unwrap();
    assert_eq!(large.rows(), LDE_ROWS);
    assert_eq!(large.workspace_bytes(), bytes);
    assert_eq!(large.generator(), geometry.domain().generator);
    assert_eq!(large.numerator_rotation(TRACE_ROWS).unwrap(), 128);
    assert!(matches!(PolynomialDomain::for_deep(&geometry, bytes-1),
        Err(Error::VerifierLimitExceeded { limit: "max_polynomial_transform_bytes", actual, max }) if actual==bytes && max==bytes-1));
    // The old source-root constructor cannot be relabeled as this larger domain.
    assert!(PolynomialDomain::new(LDE_ROWS, F::ONE, LDE_ROWS, bytes).is_err());
    let old = domain(1 << 19, large.point(0).unwrap());
    for index in [0, 1, 7, 255, 65535, (1 << 19) - 1] {
        assert_eq!(large.point(16 * index).unwrap(), old.point(index).unwrap());
    }
    for index in [0, 1, 65535, LDE_ROWS - 128, LDE_ROWS - 1] {
        assert_eq!(
            large.point((index + 128) % LDE_ROWS).unwrap(),
            large
                .point(index)
                .unwrap()
                .mul_base(geometry.trace_generator())
        );
    }
    assert!(large.point(LDE_ROWS).is_err());
}

#[test]
#[ignore = "full 8M-point transform; explicitly selected by the DEEP producer qualification"]
fn explicit_deep_transform_full_four_lane_fft_matches_coefficient_oracle() {
    use super::super::deep_geometry::{DeepGeometry, LDE_ROWS, TRACE_ROWS};
    let geometry = DeepGeometry::new().unwrap();
    let domain = PolynomialDomain::for_deep(&geometry, 2 * LDE_ROWS * F::BYTES).unwrap();
    let mut coefficients = vec![F::ZERO; TRACE_ROWS];
    coefficients[0] = f(3);
    coefficients[1] = f(7);
    coefficients[TRACE_ROWS - 1] = f(11);
    let evaluated = domain.evaluate(&coefficients, TRACE_ROWS).unwrap();
    for index in (0..128).chain([65535, 65536, 1048575, LDE_ROWS - 1]) {
        let point = domain.point(index).unwrap();
        let expected = coefficients[0]
            .add(coefficients[1].mul(point))
            .add(coefficients[TRACE_ROWS - 1].mul(point.power((TRACE_ROWS - 1) as u64)));
        assert_eq!(evaluated.value(index).unwrap(), expected);
    }
    // This checks actual full-domain FFT output. The inverse produces every
    // coefficient and must recover all high zero padding as well.
    let recovered = domain.interpolate_lanes(evaluated).unwrap();
    assert_eq!(&recovered[..TRACE_ROWS], &coefficients);
    assert!(
        recovered[TRACE_ROWS..]
            .iter()
            .all(|&value| value == F::ZERO)
    );
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
