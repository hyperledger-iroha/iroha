use ff::Field;

use super::{fq::Fq, fq2::Fq2};
use crate::ff_ext::{
    cubic::{CubicExtField, CubicExtFieldArith, CubicSparseMul},
    ExtField,
};

// -BETA is a cubic non-residue in Fp2. Fp6 = Fp2[X]/(X^3 + BETA)
// We introduce the variable v such that v^3 = -BETA
// BETA = - (u + 9)
// An element of Fq6, represented by c0 + c1 * v + c2 * v^2.Â
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
    // Fq2(u + 9)**(((q^0) - 1) / 3)
    Fq2 {
        c0: Fq([
            0xd35d438dc58f0d9d,
            0x0a78eb28f5c70b3d,
            0x666ea36f7879462c,
            0x0e0a77c19a07df2f,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^1) - 1) / 3)
    // taken from go-ethereum and also re-calculated manually
    Fq2 {
        c0: Fq([
            0xb5773b104563ab30,
            0x347f91c8a9aa6454,
            0x7a007127242e0991,
            0x1956bcd8118214ec,
        ]),
        c1: Fq([
            0x6e849f1ea0aa4757,
            0xaa1c7b6d89f89141,
            0xb6e713cdfae0ca3a,
            0x26694fbb4e82ebc3,
        ]),
    },
    // Fq2(u + 9)**(((q^2) - 1) / 3)
    // this one and other below are recalculated manually
    Fq2 {
        c0: Fq([
            0x3350c88e13e80b9c,
            0x7dce557cdb5e56b9,
            0x6001b4b8b615564a,
            0x2682e617020217e0,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^3) - 1) / 3)
    Fq2 {
        c0: Fq([
            0xc9af22f716ad6bad,
            0xb311782a4aa662b2,
            0x19eeaf64e248c7f4,
            0x20273e77e3439f82,
        ]),
        c1: Fq([
            0xacc02860f7ce93ac,
            0x3933d5817ba76b4c,
            0x69e6188b446c8467,
            0x0a46036d4417cc55,
        ]),
    },
    // Fq2(u + 9)**(((q^4) - 1) / 3)
    Fq2 {
        c0: Fq([
            0x71930c11d782e155,
            0xa6bb947cffbe3323,
            0xaa303344d4741444,
            0x2c3b3f0d26594943,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^5) - 1) / 3)
    Fq2 {
        c0: Fq([
            0xf91aba2654e8e3b1,
            0x4771cb2fdc92ce12,
            0xdcb16ae0fc8bdf35,
            0x274aa195cd9d8be4,
        ]),
        c1: Fq([
            0x5cfc50ae18811f8b,
            0x4bb28433cb43988c,
            0x4fd35f13c3b56219,
            0x301949bd2fc8883a,
        ]),
    },
];

pub const FROBENIUS_COEFF_FQ6_C2: [Fq2; 6] = [
    // Fq2(u + 9)**(((2q^0) - 2) / 3)
    Fq2 {
        c0: Fq([
            0xd35d438dc58f0d9d,
            0x0a78eb28f5c70b3d,
            0x666ea36f7879462c,
            0x0e0a77c19a07df2f,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((2q^1) - 2) / 3)
    Fq2 {
        c0: Fq([
            0x7361d77f843abe92,
            0xa5bb2bd3273411fb,
            0x9c941f314b3e2399,
            0x15df9cddbb9fd3ec,
        ]),
        c1: Fq([
            0x5dddfd154bd8c949,
            0x62cb29a5a4445b60,
            0x37bc870a0c7dd2b9,
            0x24830a9d3171f0fd,
        ]),
    },
    // Fq2(u + 9)**(((2q^2) - 2) / 3)
    Fq2 {
        c0: Fq([
            0x71930c11d782e155,
            0xa6bb947cffbe3323,
            0xaa303344d4741444,
            0x2c3b3f0d26594943,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((2q^3) - 2) / 3)
    Fq2 {
        c0: Fq([
            0x448a93a57b6762df,
            0xbfd62df528fdeadf,
            0xd858f5d00e9bd47a,
            0x06b03d4d3476ec58,
        ]),
        c1: Fq([
            0x2b19daf4bcc936d1,
            0xa1a54e7a56f4299f,
            0xb533eee05adeaef1,
            0x170c812b84dda0b2,
        ]),
    },
    // Fq2(u + 9)**(((2q^4) - 2) / 3)
    Fq2 {
        c0: Fq([
            0x3350c88e13e80b9c,
            0x7dce557cdb5e56b9,
            0x6001b4b8b615564a,
            0x2682e617020217e0,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((2q^5) - 2) / 3)
    Fq2 {
        c0: Fq([
            0x843420f1d8dadbd6,
            0x31f010c9183fcdb2,
            0x436330b527a76049,
            0x13d47447f11adfe4,
        ]),
        c1: Fq([
            0xef494023a857fa74,
            0x2a925d02d5ab101a,
            0x83b015829ba62f10,
            0x2539111d0c13aea3,
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
                crate::bn256::fq6::test::$test(&mut rng, $size);
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
}
