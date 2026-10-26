use core::{cmp::Ordering, convert::TryInto};

use subtle::{Choice, CtOption};

use super::fp::Fp;
use crate::{
    ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup},
    ff_ext::{
        quadratic::{QuadExtField, QuadExtFieldArith, SQRT},
        ExtField, Legendre,
    },
};

crate::impl_binops_additive!(Fp2, Fp2);
crate::impl_binops_multiplicative!(Fp2, Fp2);
crate::impl_binops_calls!(Fp2);
crate::impl_sum_prod!(Fp2);
crate::impl_tower2!(Fp, Fp2);
crate::impl_tower2_from_uniform_bytes!(Fp, Fp2, 128);

pub type Fp2 = QuadExtField<Fp>;

impl QuadExtFieldArith for Fp2 {
    type Base = Fp;
    const SQRT: SQRT<Fp> = SQRT::Algorithm10 {
        precompute_e: Fp2 {
            c0: Fp::ZERO,
            c1: Fp::from_raw([
                0x67153f9701e19938,
                0x5d232408689b4c6c,
                0x021848271d63f087,
                0x03b21f15823a404a,
                0x667c70cf991a36e6,
                0x7a82a3d83bc9e63a,
                0x13e275a1fa6a13af,
            ]),
        },
        precompute_f: Fp2 {
            c0: Fp::from_raw([0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            c1: Fp::ZERO,
        },
        q_minus_1_over_4: &[
            0x67ffff34c0000000,
            0xa8a9fa30c001ae51,
            0xf929e97fa3eb7ff5,
            0xd10fe69736a29b1e,
            0x2a00f29dbd0e499b,
            0x004c3800035fdc39,
            0x0900000000000900,
        ],
    };
}

impl ExtField for Fp2 {
    const NON_RESIDUE: Self = Fp2 {
        c0: Fp::from_raw([
            0xddb6da4b5b6db6e8,
            0x833bf7b35b701d98,
            0x3f6072240ebe2483,
            0x73cd928ee056022c,
            0xce4a7f2a7bcb4495,
            0xdbda9924971b3a9a,
            0x0cdb6db6db6dc3b6,
        ]),

        c1: Fp::from_raw([
            0xeb6db62d36db6db3,
            0xb523fb0536dcde8e,
            0x8c6d1148d5a5491b,
            0x457b57ef5366ce1a,
            0x489319197d79f5f3,
            0xb71cc2492776bcc3,
            0x07b6db6db6db756d,
        ]),
    };
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
    use crate::{arith_test, constants_test, legendre_test, serde_test, test};

    constants_test!(Fp2);

    arith_test!(Fp2);
    legendre_test!(Fp2);
    test!(arith, Fp2, sqrt_test, 1000);

    serde_test!(Fp2);

    crate::f2_test!(Fp2, Fp);
    crate::frobenius_test!(Fp2, Fp, 20);

    #[test]
    fn test_fp2_squaring() {
        // u + 1
        let mut a = Fp2 {
            c0: Fp::one(),
            c1: Fp::one(),
        };
        // (u + 1) ^2 = 1 + u^2 + 2u = -4 + 2u
        a.square_assign();
        let minus_4 = -Fp::from(4u64);
        assert_eq!(
            a,
            Fp2 {
                c0: minus_4,
                c1: Fp::one() + Fp::one(),
            }
        );

        // u
        let mut a = Fp2 {
            c0: Fp::zero(),
            c1: Fp::one(),
        };
        // u^2
        a.square_assign();
        assert_eq!(
            a,
            Fp2 {
                c0: Fp::NON_RESIDUE,
                c1: Fp::zero(),
            }
        );
    }

    #[test]
    fn test_fp2_mul_nonresidue() {
        use rand_core::SeedableRng;
        use rand_xorshift::XorShiftRng;
        let mut rng = XorShiftRng::from_seed(crate::tests::SEED);
        for _ in 0..1000 {
            let mut a = Fp2::random(&mut rng);
            let mut b = a;
            a = a.mul_by_nonresidue();
            b.mul_assign(&Fp2::NON_RESIDUE);

            assert_eq!(a, b);
        }
    }
}
