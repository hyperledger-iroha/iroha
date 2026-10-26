use ff::PrimeField;
use rand_core::RngCore;

use crate::ff_ext::Legendre;

pub(crate) fn legendre_symbol_test<F: PrimeField + Legendre>(mut rng: impl RngCore, n: usize) {
    assert_eq!(F::ZERO.legendre(), 0);
    for _ in 0..n {
        let a = F::random(&mut rng);
        if a.legendre() == -1 {
            assert!(bool::from(a.sqrt().is_none()));
        }
        let b = a.square();
        assert_eq!(b.legendre(), 1);
    }
}

pub(crate) fn quadratic_residue_test<F: PrimeField + Legendre>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
        let elem = F::random(&mut rng);
        let is_quad_non_res: bool = elem.ct_quadratic_non_residue().into();
        let is_quad_res_or_zero: bool = elem.ct_quadratic_residue().into();
        assert_eq!(!is_quad_non_res, is_quad_res_or_zero)
    }
}

#[macro_export]
macro_rules! legendre_test {
    ($field:ident) => {
        test!(legendre, $field, legendre_symbol_test, 1000);
        test!(legendre, $field, quadratic_residue_test, 1000);
    };
}
