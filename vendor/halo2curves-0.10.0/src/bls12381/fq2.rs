use core::{cmp::Ordering, convert::TryInto};
use subtle::{Choice, CtOption};

use super::fq::Fq;
use crate::{
    ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup},
    ff_ext::{
        quadratic::{QuadExtField, QuadExtFieldArith, SQRT},
        ExtField, Legendre,
    },
};

crate::impl_binops_additive!(Fq2, Fq2);
crate::impl_binops_multiplicative!(Fq2, Fq2);
crate::impl_binops_calls!(Fq2);
crate::impl_sum_prod!(Fq2);
crate::impl_tower2!(Fq, Fq2);
crate::impl_tower2_from_uniform_bytes!(Fq, Fq2, 128);

pub type Fq2 = QuadExtField<Fq>;
impl QuadExtFieldArith for Fq2 {
    type Base = Fq;
    const SQRT: SQRT<Fq> = SQRT::Algorithm9 {
        q_minus_3_over_4: &[
            0xee7fbfffffffeaaa,
            0x07aaffffac54ffff,
            0xd9cc34a83dac3d89,
            0xd91dd2e13ce144af,
            0x92c6e9ed90d2eb35,
            0x0680447a8e5ff9a6,
        ],
        q_minus_1_over_2: &[
            0xdcff7fffffffd555,
            0x0f55ffff58a9ffff,
            0xb39869507b587b12,
            0xb23ba5c279c2895f,
            0x258dd3db21a5d66b,
            0x0d0088f51cbff34d,
        ],
    };

    fn square_assign(el: &mut QuadExtField<Self::Base>) {
        let a = el.c0 + el.c1;
        let b = el.c0 - el.c1;
        let c = el.c0.double();
        el.c0 = a * b;
        el.c1 = c * el.c1;
    }
}

impl ExtField for Fq2 {
    const NON_RESIDUE: Self = Fq2::new(Fq::ONE, Fq::ONE);

    fn mul_by_nonresidue(&self) -> Self {
        Self {
            c0: self.c0 - self.c1,
            c1: self.c0 + self.c1,
        }
    }

    fn frobenius_map(&mut self, power: usize) {
        if power % 2 != 0 {
            self.conjugate();
        }
    }
}

#[cfg(test)]
mod test {

    use rand_core::RngCore;

    use super::*;
    use crate::{
        arith_test, constants_test, f2_test, frobenius_test, legendre_test, serde_test, test,
    };

    constants_test!(Fq2);

    arith_test!(Fq2);
    legendre_test!(Fq2);
    test!(arith, Fq2, sqrt_test, 1000);

    serde_test!(Fq2);

    f2_test!(Fq2, Fq);
    frobenius_test!(Fq2, Fq, 20);

    #[test]
    fn test_fq2_mul_nonresidue() {
        let e = Fq2::random(rand_core::OsRng);
        let a0 = e.mul_by_nonresidue();
        let a1 = e * Fq2::NON_RESIDUE;
        assert_eq!(a0, a1);
    }
}
