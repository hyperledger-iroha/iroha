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
crate::impl_tower2_from_uniform_bytes!(Fq, Fq2, 96);

pub type Fq2 = QuadExtField<Fq>;
impl QuadExtFieldArith for Fq2 {
    type Base = Fq;
    const SQRT: SQRT<Fq> = SQRT::Algorithm9 {
        q_minus_3_over_4: &[
            0x4f082305b61f3f51,
            0x65e05aa45a1c72a3,
            0x6e14116da0605617,
            0x0c19139cb84c680a,
        ],
        q_minus_1_over_2: &[
            0x9e10460b6c3e7ea3,
            0xcbc0b548b438e546,
            0xdc2822db40c0ac2e,
            0x183227397098d014,
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
    const NON_RESIDUE: Self = Fq2::new(Fq::from_raw([9u64, 0, 0, 0]), Fq::ONE);

    fn mul_by_nonresidue(&self) -> Self {
        // (xu+y)(u+9) = (9x+y)u+(9y-x)
        let t0 = self.c0;
        let t1 = self.c1;
        // 8*x*i + 8*y
        let t = self.double().double().double();
        Self {
            // 9*y
            c0: t.c0 + t0 - t1,
            // (9*x + y)
            c1: t.c1 + t0 + t1,
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
