use super::{fq::Fq, fq2::Fq2, fq6::Fq6};
use crate::ff_ext::{
    quadratic::{QuadExtField, QuadExtFieldArith, QuadSparseMul},
    ExtField,
};

// -GAMMA is a quadratic non-residue in Fp6. Fp12 = Fp6[X] / (X^2 + GAMMA)
// We introduce the variable w such that w^2 = -GAMMA
// GAMMA = -v
/// An element of Fq12, represented by c0 + c1 * w.
pub type Fq12 = QuadExtField<Fq6>;

impl QuadExtFieldArith for Fq12 {
    type Base = Fq6;
}

impl QuadSparseMul for Fq12 {
    type Base = Fq2;
}

impl ExtField for Fq12 {
    const NON_RESIDUE: Self = Fq12::zero(); // no needs

    fn frobenius_map(&mut self, power: usize) {
        self.c0.frobenius_map(power);
        self.c1.frobenius_map(power);
        self.c1.c0.mul_assign(&FROBENIUS_COEFF_FQ12_C1[power % 12]);
        self.c1.c1.mul_assign(&FROBENIUS_COEFF_FQ12_C1[power % 12]);
        self.c1.c2.mul_assign(&FROBENIUS_COEFF_FQ12_C1[power % 12]);
    }
}

crate::impl_binops_additive!(Fq12, Fq12);
crate::impl_binops_multiplicative!(Fq12, Fq12);
crate::impl_binops_calls!(Fq12);
crate::impl_sum_prod!(Fq12);
crate::impl_cyclotomic_square!(Fq2, Fq12);

// non_residue^((modulus^i-1)/6) for i=0,...,11
pub const FROBENIUS_COEFF_FQ12_C1: [Fq2; 12] = [
    // Fq2(u + 9)**(((q^0) - 1) / 6)
    // Fq points are represented in Montgomery form with R = 2^256
    Fq2 {
        c0: Fq([
            0xd35d438dc58f0d9d,
            0x0a78eb28f5c70b3d,
            0x666ea36f7879462c,
            0x0e0a77c19a07df2f,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^1) - 1) / 6)
    Fq2 {
        c0: Fq([
            0xaf9ba69633144907,
            0xca6b1d7387afb78a,
            0x11bded5ef08a2087,
            0x02f34d751a1f3a7c,
        ]),
        c1: Fq([
            0xa222ae234c492d72,
            0xd00f02a4565de15b,
            0xdc2ff3a253dfc926,
            0x10a75716b3899551,
        ]),
    },
    // Fq2(u + 9)**(((q^2) - 1) / 6)
    Fq2 {
        c0: Fq([
            0xca8d800500fa1bf2,
            0xf0c5d61468b39769,
            0x0e201271ad0d4418,
            0x04290f65bad856e6,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^3) - 1) / 6)
    Fq2 {
        c0: Fq([
            0x365316184e46d97d,
            0x0af7129ed4c96d9f,
            0x659da72fca1009b5,
            0x08116d8983a20d23,
        ]),
        c1: Fq([
            0xb1df4af7c39c1939,
            0x3d9f02878a73bf7f,
            0x9b2220928caf0ae0,
            0x26684515eff054a6,
        ]),
    },
    // Fq2(u + 9)**(((q^4) - 1) / 6)
    Fq2 {
        c0: Fq([
            0x3350c88e13e80b9c,
            0x7dce557cdb5e56b9,
            0x6001b4b8b615564a,
            0x2682e617020217e0,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^5) - 1) / 6)
    Fq2 {
        c0: Fq([
            0x86b76f821b329076,
            0x408bf52b4d19b614,
            0x53dfb9d0d985e92d,
            0x051e20146982d2a7,
        ]),
        c1: Fq([
            0x0fbc9cd47752ebc7,
            0x6d8fffe33415de24,
            0xbef22cf038cf41b9,
            0x15c0edff3c66bf54,
        ]),
    },
    // Fq2(u + 9)**(((q^6) - 1) / 6)
    Fq2 {
        c0: Fq([
            0x68c3488912edefaa,
            0x8d087f6872aabf4f,
            0x51e1a24709081231,
            0x2259d6b14729c0fa,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^7) - 1) / 6)
    Fq2 {
        c0: Fq([
            0x8c84e580a568b440,
            0xcd164d1de0c21302,
            0xa692585790f737d5,
            0x2d7100fdc71265ad,
        ]),
        c1: Fq([
            0x99fdddf38c33cfd5,
            0xc77267ed1213e931,
            0xdc2052142da18f36,
            0x1fbcf75c2da80ad7,
        ]),
    },
    // Fq2(u + 9)**(((q^8) - 1) / 6)
    Fq2 {
        c0: Fq([
            0x71930c11d782e155,
            0xa6bb947cffbe3323,
            0xaa303344d4741444,
            0x2c3b3f0d26594943,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^9) - 1) / 6)
    Fq2 {
        c0: Fq([
            0x05cd75fe8a3623ca,
            0x8c8a57f293a85cee,
            0x52b29e86b7714ea8,
            0x2852e0e95d8f9306,
        ]),
        c1: Fq([
            0x8a41411f14e0e40e,
            0x59e26809ddfe0b0d,
            0x1d2e2523f4d24d7d,
            0x09fc095cf1414b83,
        ]),
    },
    // Fq2(u + 9)**(((q^10) - 1) / 6)
    Fq2 {
        c0: Fq([
            0x08cfc388c494f1ab,
            0x19b315148d1373d4,
            0x584e90fdcb6c0213,
            0x09e1685bdf2f8849,
        ]),
        c1: Fq([0x0, 0x0, 0x0, 0x0]),
    },
    // Fq2(u + 9)**(((q^11) - 1) / 6)
    Fq2 {
        c0: Fq([
            0xb5691c94bd4a6cd1,
            0x56f575661b581478,
            0x64708be5a7fb6f30,
            0x2b462e5e77aecd82,
        ]),
        c1: Fq([
            0x2c63ef42612a1180,
            0x29f16aae345bec69,
            0xf95e18c648b216a4,
            0x1aa36073a4cae0d4,
        ]),
    },
];

#[cfg(test)]
mod test {

    macro_rules! test_fq12 {
        ($test:ident, $size: expr) => {
            paste::paste! {
            #[test]
            fn [< $test test >]() {
                use rand_core::SeedableRng;
                use rand_xorshift::XorShiftRng;
                let mut rng = XorShiftRng::from_seed(crate::tests::SEED);
                crate::bn256::fq12::test::$test(&mut rng, $size);
            }
            }
        };
    }
    use ff::Field;
    use rand_core::RngCore;

    use super::*;
    use crate::{arith_test, frobenius_test, setup_f12_test_funcs, test};

    arith_test!(Fq12);

    // F12 specific
    setup_f12_test_funcs!(Fq12, Fq6, Fq2);
    test_fq12!(f12_mul_by_014_, 500);
    test_fq12!(f12_mul_by_034_, 500);
    frobenius_test!(Fq12, Fq, 8);
}
