use ff::Field;

use super::{fq::Fq, fq2::Fq2};
use crate::ff_ext::{
    cubic::{CubicExtField, CubicExtFieldArith, CubicSparseMul},
    ExtField,
};

crate::impl_binops_additive!(Fq6, Fq6);
crate::impl_binops_multiplicative!(Fq6, Fq6);
crate::impl_binops_calls!(Fq6);
crate::impl_sum_prod!(Fq6);
pub type Fq6 = CubicExtField<Fq2>;

impl CubicExtFieldArith for Fq6 {
    type Base = Fq2;
}

impl CubicSparseMul for Fq6 {
    type Base = Fq2;
}

impl ExtField for Fq6 {
    const NON_RESIDUE: Self = Fq6::new(Fq2::ZERO, Fq2::ONE, Fq2::ZERO);

    fn frobenius_map(&mut self, power: usize) {
        self.c0.frobenius_map(power);
        self.c1.frobenius_map(power);
        self.c2.frobenius_map(power);
        self.c1.mul_assign(&FROBENIUS_COEFF_FQ6_C1[power % 6]);
        self.c2.mul_assign(&FROBENIUS_COEFF_FQ6_C2[power % 6]);
    }

    fn mul_by_nonresidue(self: &Fq6) -> Fq6 {
        let c0 = self.c2.mul_by_nonresidue();
        let c1 = self.c0;
        let c2 = self.c1;
        Self { c0, c1, c2 }
    }
}

pub const FROBENIUS_COEFF_FQ6_C1: [Fq2; 6] = [
    // z ^ (( p ^ 0 - 1) / 3)
    Fq2 {
        c0: Fq([
            0x760900000002fffd,
            0xebf4000bc40c0002,
            0x5f48985753c758ba,
            0x77ce585370525745,
            0x5c071a97a256ec6d,
            0x15f65ec3fa80e493,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
    // z ^ (( p ^ 1 - 1) / 3)
    Fq2 {
        c0: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
        c1: Fq([
            0xcd03c9e48671f071,
            0x5dab22461fcda5d2,
            0x587042afd3851b95,
            0x8eb60ebe01bacb9e,
            0x03f97d6e83d050d2,
            0x18f0206554638741,
        ]),
    },
    // z ^ (( p ^ 2 - 1) / 3)
    Fq2 {
        c0: Fq([
            0x30f1361b798a64e8,
            0xf3b8ddab7ece5a2a,
            0x16a8ca3ac61577f7,
            0xc26a2ff874fd029b,
            0x3636b76660701c6e,
            0x051ba4ab241b6160,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
    // z ^ (( p ^ 3 - 1) / 3)
    Fq2 {
        c0: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
        c1: Fq([
            0x760900000002fffd,
            0xebf4000bc40c0002,
            0x5f48985753c758ba,
            0x77ce585370525745,
            0x5c071a97a256ec6d,
            0x15f65ec3fa80e493,
        ]),
    },
    // z ^ (( p ^ 4 - 1) / 3)
    Fq2 {
        c0: Fq([
            0xcd03c9e48671f071,
            0x5dab22461fcda5d2,
            0x587042afd3851b95,
            0x8eb60ebe01bacb9e,
            0x03f97d6e83d050d2,
            0x18f0206554638741,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
    // z ^ (( p ^ 5 - 1) / 3)
    Fq2 {
        c0: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
        c1: Fq([
            0x30f1361b798a64e8,
            0xf3b8ddab7ece5a2a,
            0x16a8ca3ac61577f7,
            0xc26a2ff874fd029b,
            0x3636b76660701c6e,
            0x051ba4ab241b6160,
        ]),
    },
];

// z = u + 1
pub const FROBENIUS_COEFF_FQ6_C2: [Fq2; 6] = [
    // z ^ (( 2 * p ^ 0 - 2) / 3)
    Fq2 {
        c0: Fq([
            0x760900000002fffd,
            0xebf4000bc40c0002,
            0x5f48985753c758ba,
            0x77ce585370525745,
            0x5c071a97a256ec6d,
            0x15f65ec3fa80e493,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
    // z ^ (( 2 * p ^ 1 - 2) / 3)
    Fq2 {
        c0: Fq([
            0x890dc9e4867545c3,
            0x2af322533285a5d5,
            0x50880866309b7e2c,
            0xa20d1b8c7e881024,
            0x14e4f04fe2db9068,
            0x14e56d3f1564853a,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
    // z ^ (( 2 * p ^ 2 - 2) / 3)
    Fq2 {
        c0: Fq([
            0xcd03c9e48671f071,
            0x5dab22461fcda5d2,
            0x587042afd3851b95,
            0x8eb60ebe01bacb9e,
            0x03f97d6e83d050d2,
            0x18f0206554638741,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
    // z ^ (( 2 * p ^ 3 - 2) / 3)
    Fq2 {
        c0: Fq([
            0x43f5fffffffcaaae,
            0x32b7fff2ed47fffd,
            0x07e83a49a2e99d69,
            0xeca8f3318332bb7a,
            0xef148d1ea0f4c069,
            0x040ab3263eff0206,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
    // z ^ (( 2 * p ^ 4 - 2) / 3)
    Fq2 {
        c0: Fq([
            0x30f1361b798a64e8,
            0xf3b8ddab7ece5a2a,
            0x16a8ca3ac61577f7,
            0xc26a2ff874fd029b,
            0x3636b76660701c6e,
            0x051ba4ab241b6160,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
    // z ^ (( 2 * p ^ 5 - 2) / 3)
    Fq2 {
        c0: Fq([
            0xecfb361b798dba3a,
            0xc100ddb891865a2c,
            0x0ec08ff1232bda8e,
            0xd5c13cc6f1ca4721,
            0x47222a47bf7b5c04,
            0x0110f184e51c5f59,
        ]),
        c1: Fq([
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ]),
    },
];

#[cfg(test)]
mod test {
    use rand_core::RngCore;

    use super::*;
    use crate::{arith_test, frobenius_test, setup_f6_test_funcs, test};

    macro_rules! test_fq6 {
        ($test:ident, $size: expr) => {
            paste::paste! {
            #[test]
            fn [< $test test >]() {
                use rand_core::SeedableRng;
                use rand_xorshift::XorShiftRng;
                let mut rng = XorShiftRng::from_seed(crate::tests::SEED);
                crate::bls12381::fq6::test::$test(&mut rng, $size);
            }
            }
        };
    }

    arith_test!(Fq6);
    setup_f6_test_funcs!(Fq6, Fq2);
    test_fq6!(f6_mul_nonresidue_, 1000);
    test_fq6!(f6_mul_by_1_, 1000);
    test_fq6!(f6_mul_by_01_, 1000);
    frobenius_test!(Fq6, Fq, 10);

    #[test]
    fn test_fq6_mul_nonresidue() {
        let e = Fq6::random(rand_core::OsRng);
        let a0 = e.mul_by_nonresidue();
        let a1 = e * Fq6::NON_RESIDUE;
        assert_eq!(a0, a1);
    }
}
