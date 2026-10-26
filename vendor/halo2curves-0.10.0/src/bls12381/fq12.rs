use super::{fq::Fq, fq2::Fq2, fq6::Fq6};
use crate::ff_ext::{
    quadratic::{QuadExtField, QuadExtFieldArith, QuadSparseMul},
    ExtField,
};

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

pub const FROBENIUS_COEFF_FQ12_C1: [Fq2; 12] = [
    // z = u + 1
    // z ^ ((p ^ 0 - 1) / 6)
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
    // z ^ ((p ^ 1 - 1) / 6)
    Fq2 {
        c0: Fq([
            0x07089552b319d465,
            0xc6695f92b50a8313,
            0x97e83cccd117228f,
            0xa35baecab2dc29ee,
            0x1ce393ea5daace4d,
            0x08f2220fb0fb66eb,
        ]),
        c1: Fq([
            0xb2f66aad4ce5d646,
            0x5842a06bfc497cec,
            0xcf4895d42599d394,
            0xc11b9cba40a8e8d0,
            0x2e3813cbe5a0de89,
            0x110eefda88847faf,
        ]),
    },
    // z ^ ((p ^ 2 - 1) / 6)
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
    // z ^ ((p ^ 3 - 1) / 6)
    Fq2 {
        c0: Fq([
            0x3e2f585da55c9ad1,
            0x4294213d86c18183,
            0x382844c88b623732,
            0x92ad2afd19103e18,
            0x1d794e4fac7cf0b9,
            0x0bd592fc7d825ec8,
        ]),
        c1: Fq([
            0x7bcfa7a25aa30fda,
            0xdc17dec12a927e7c,
            0x2f088dd86b4ebef1,
            0xd1ca2087da74d4a7,
            0x2da2596696cebc1d,
            0x0e2b7eedbbfd87d2,
        ]),
    },
    // z ^ ((p ^ 4 - 1) / 6)
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
    // z ^ ((p ^ 5 - 1) / 6)
    Fq2 {
        c0: Fq([
            0x3726c30af242c66c,
            0x7c2ac1aad1b6fe70,
            0xa04007fbba4b14a2,
            0xef517c3266341429,
            0x0095ba654ed2226b,
            0x02e370eccc86f7dd,
        ]),
        c1: Fq([
            0x82d83cf50dbce43f,
            0xa2813e53df9d018f,
            0xc6f0caa53c65e181,
            0x7525cf528d50fe95,
            0x4a85ed50f4798a6b,
            0x171da0fd6cf8eebd,
        ]),
    },
    // z ^ ((p ^ 6 - 1) / 6)
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
    // z ^ ((p ^ 7 - 1) / 6)
    Fq2 {
        c0: Fq([
            0xb2f66aad4ce5d646,
            0x5842a06bfc497cec,
            0xcf4895d42599d394,
            0xc11b9cba40a8e8d0,
            0x2e3813cbe5a0de89,
            0x110eefda88847faf,
        ]),
        c1: Fq([
            0x07089552b319d465,
            0xc6695f92b50a8313,
            0x97e83cccd117228f,
            0xa35baecab2dc29ee,
            0x1ce393ea5daace4d,
            0x08f2220fb0fb66eb,
        ]),
    },
    // z ^ ((p ^ 8 - 1) / 6)
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
    // z ^ ((p ^ 9 - 1) / 6)
    Fq2 {
        c0: Fq([
            0x7bcfa7a25aa30fda,
            0xdc17dec12a927e7c,
            0x2f088dd86b4ebef1,
            0xd1ca2087da74d4a7,
            0x2da2596696cebc1d,
            0x0e2b7eedbbfd87d2,
        ]),
        c1: Fq([
            0x3e2f585da55c9ad1,
            0x4294213d86c18183,
            0x382844c88b623732,
            0x92ad2afd19103e18,
            0x1d794e4fac7cf0b9,
            0x0bd592fc7d825ec8,
        ]),
    },
    // z ^ ((p ^ 10 - 1) / 6)
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
    // z ^ ((p ^ 11 - 1) / 6)
    Fq2 {
        c0: Fq([
            0x82d83cf50dbce43f,
            0xa2813e53df9d018f,
            0xc6f0caa53c65e181,
            0x7525cf528d50fe95,
            0x4a85ed50f4798a6b,
            0x171da0fd6cf8eebd,
        ]),
        c1: Fq([
            0x3726c30af242c66c,
            0x7c2ac1aad1b6fe70,
            0xa04007fbba4b14a2,
            0xef517c3266341429,
            0x0095ba654ed2226b,
            0x02e370eccc86f7dd,
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
                crate::bls12381::fq12::test::$test(&mut rng, $size);
            }
            }
        };
    }
    use ff::Field;
    use rand_core::RngCore;

    use super::*;
    use crate::{arith_test, frobenius_test, setup_f12_test_funcs, test};

    arith_test!(Fq12);
    // TODO Compile problems with derive_serde feature
    // serde_test!(fq12);

    // F12 specific
    setup_f12_test_funcs!(Fq12, Fq6, Fq2);
    test_fq12!(f12_mul_by_014_, 500);
    test_fq12!(f12_mul_by_034_, 500);
    frobenius_test!(Fq12, Fq, 8);
}
