use ff::PrimeField;
pub(crate) fn primefield_constants_test<F: PrimeField>() {
    assert_eq!(F::ROOT_OF_UNITY_INV, F::ROOT_OF_UNITY.invert().unwrap());
    assert_eq!(F::from(2) * F::TWO_INV, F::ONE);
    if F::S != 0 {
        assert_eq!(F::ROOT_OF_UNITY.pow_vartime([1 << F::S]), F::ONE);
        assert_eq!(F::DELTA, F::MULTIPLICATIVE_GENERATOR.pow([1u64 << F::S]));
    }
}

use ff::WithSmallOrderMulGroup;
pub(crate) fn zeta_test<F: WithSmallOrderMulGroup<3>>() {
    assert_eq!(F::ZETA * F::ZETA * F::ZETA, F::ONE);
    assert_ne!(F::ZETA * F::ZETA, F::ONE);
}

#[macro_export]
macro_rules! constants_test {
    ($field:ident) => {
        #[test]
        fn primefield_constants_test() {
            use super::*;
            $crate::tests::field::constants::primefield_constants_test::<$field>();
        }

        #[test]
        fn zeta_test() {
            use super::*;
            $crate::tests::field::constants::zeta_test::<$field>();
        }
    };
}
