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
                10_449_979_523_578_133_997,
                16_930_877_747_294_120_912,
                13_152_949_513_120_778_590,
                870_193_615_252_173_847,
            ],
        ),
        (
            [19, 0, 31, 0],
            [14_068_322_723_375_401_646, 0, 1_318_452_490_038_271_421, 0],
        ),
        (
            [19, 0, 0, 31],
            [
                7_508_680_214_721_908_129,
                12_178_771_568_672_285_613,
                6_244_347_143_616_962_327,
                10_079_264_575_797_699_336,
            ],
        ),
        (
            [13, 17, 23, 29],
            [
                6_445_649_592_975_166_559,
                12_198_359_575_640_447_885,
                157_916_207_088_556_095,
                2_145_492_762_898_322_422,
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
