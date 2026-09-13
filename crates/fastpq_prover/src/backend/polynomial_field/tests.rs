//! Arithmetic and canonicality checks for the shared polynomial field owner.

use super::super::polynomial_reference;
use super::*;

#[test]
fn field_inverses_preserve_the_existing_basis_and_all_coordinates() {
    assert_eq!(0_u64.inverse(), None);
    assert_eq!(GoldilocksFp4V1::ZERO.inverse(), None);
    for value in polynomial_reference::points()
        .into_iter()
        .filter(|value| *value != GoldilocksFp4V1::ZERO)
    {
        let inverse = value.inverse().unwrap();
        assert_eq!(value.mul(inverse), GoldilocksFp4V1::ONE);
        assert_eq!(inverse.mul(value), GoldilocksFp4V1::ONE);
        assert_eq!(inverse.inverse(), Some(value));
    }
    for value in [1, 7, 1 << 32, GOLDILOCKS_MODULUS - 1] {
        let embedded = GoldilocksFp4V1::embed_base(value);
        assert_eq!(
            embedded.inverse(),
            Some(GoldilocksFp4V1::embed_base(field_inverse(value)))
        );
        for power in [0, 1, 2, 31, GOLDILOCKS_MODULUS] {
            assert_eq!(
                embedded.power(power),
                GoldilocksFp4V1::embed_base(super::super::field_pow(value, power))
            );
        }
    }
}

#[test]
fn malformed_field_coordinates_are_rejected_before_inversion() {
    for value in [GOLDILOCKS_MODULUS, u64::MAX] {
        assert_eq!(value.inverse(), None);
        assert!(
            matches!(value.validate("point", &[9]), Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [9])
        );
        for lane in 0..4 {
            let mut words = [0; 4];
            words[lane] = value;
            let point = GoldilocksFp4V1::from_coefficients_unchecked_for_test(words);
            assert_eq!(point.inverse(), None);
            assert!(
                matches!(point.validate("point", &[9]), Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [9, lane])
            );
        }
    }
}

#[test]
fn fp4_inversion_matches_independent_big_exponent_vectors() {
    let vectors: [([u64; 4], [u64; 4]); 4] = [
        (
            [19, 31, 0, 0],
            [
                10449979523578133997,
                16930877747294120912,
                13152949513120778590,
                870193615252173847,
            ],
        ),
        (
            [19, 0, 31, 0],
            [14068322723375401646, 0, 1318452490038271421, 0],
        ),
        (
            [19, 0, 0, 31],
            [
                7508680214721908129,
                12178771568672285613,
                6244347143616962327,
                10079264575797699336,
            ],
        ),
        (
            [13, 17, 23, 29],
            [
                6445649592975166559,
                12198359575640447885,
                157916207088556095,
                2145492762898322422,
            ],
        ),
    ];
    for (point, inverse) in vectors {
        assert_eq!(
            GoldilocksFp4V1::new(point).unwrap().inverse(),
            GoldilocksFp4V1::new(inverse)
        );
    }
}
