use ff::Field;

use super::{fp::Fp, fp2::Fp2, fp6::Fp6};
use crate::ff_ext::{
    quadratic::{QuadExtField, QuadExtFieldArith, QuadSparseMul},
    ExtField,
};

// -GAMMA is a quadratic non-residue in Fp6. Fp12 = Fp6[X]/(X^2 + GAMMA)
// We introduce the variable w such that w^2 = -GAMMA
// GAMMA = - v
/// An element of Fp12, represented by c0 + c1 * v.
pub type Fp12 = QuadExtField<Fp6>;

impl QuadExtFieldArith for Fp12 {
    type Base = Fp6;
}

impl QuadSparseMul for Fp12 {
    type Base = Fp2;
}

impl ExtField for Fp12 {
    const NON_RESIDUE: Self = Fp12::zero(); // no needs

    fn frobenius_map(&mut self, power: usize) {
        self.c0.frobenius_map(power);
        self.c1.frobenius_map(power);
        self.c1.c0.mul_assign(&FROBENIUS_COEFF_FP12_C1[power % 12]);
        self.c1.c1.mul_assign(&FROBENIUS_COEFF_FP12_C1[power % 12]);
        self.c1.c2.mul_assign(&FROBENIUS_COEFF_FP12_C1[power % 12]);
    }
}

crate::impl_binops_additive!(Fp12, Fp12);
crate::impl_binops_multiplicative!(Fp12, Fp12);
crate::impl_binops_calls!(Fp12);
crate::impl_sum_prod!(Fp12);
crate::impl_cyclotomic_square!(Fp2, Fp12);

/// Fp2(v)^((p^i-1)/6) for i=0,...,11
pub const FROBENIUS_COEFF_FP12_C1: [Fp2; 12] = [
    // Fp2(v)**(((p^0) - 1) / 6)
    Fp2::ONE,
    // Fp2(v)**(((p^1) - 1) / 6)
    Fp2 {
        // 0x3c3ad3da8b99cb1df0709dc343113ccd9892dedd51f30695d89c647b90de8f41df055384b9e6cfd4e70648622c750f32ee965dfef2303d3
        c0: Fp::from_raw([
            0x2ee965dfef2303d3,
            0x4e70648622c750f3,
            0x1df055384b9e6cfd,
            0x5d89c647b90de8f4,
            0xd9892dedd51f3069,
            0xdf0709dc343113cc,
            0x03c3ad3da8b99cb1,
        ]),
        // 0x149fd9ed2c7affe7aaa3b912182da22dccb29838628f04b6f333d052540294889f03876b2ddb143559f9373f4cf44e6afa0be24ad758a5ff
        c1: Fp::from_raw([
            0xfa0be24ad758a5ff,
            0x59f9373f4cf44e6a,
            0x9f03876b2ddb1435,
            0xf333d05254029488,
            0xccb29838628f04b6,
            0xaaa3b912182da22d,
            0x149fd9ed2c7affe7,
        ]),
    },
    // Fp2(v)**(((p^2) - 1) / 6)
    Fp2 {
        // 0x480000000000360001c950000d7ee0e4a803c956d01c903d720dc8ad8b38dffaf50c100004c37fffffff
        c0: Fp::from_raw([
            0x100004c37fffffff,
            0xc8ad8b38dffaf50c,
            0xc956d01c903d720d,
            0x50000d7ee0e4a803,
            0x00000000360001c9,
            0x0000000000004800,
            0x0000000000000000,
        ]),
        c1: Fp::ZERO,
    },
    // Fp2(v)**(((p^3) - 1) / 6)
    Fp2 {
        // 0x1baee9e044d94d205764b80089c40010af5ca1e56a2a81e6a5d8739325984fc889d390efef216fe4f4af912a897f60a128a3be71be4995ca
        c0: Fp::from_raw([
            0x28a3be71be4995ca,
            0xf4af912a897f60a1,
            0x89d390efef216fe4,
            0xa5d8739325984fc8,
            0xaf5ca1e56a2a81e6,
            0x5764b80089c40010,
            0x1baee9e044d94d20,
        ]),
        // 0x20d4c11700e832829b26f1795339413be65e47a7716bc8bc07cd6b44b03ef1130b3c35a77291b29d6f45d28e4ef1ecb9678f4479a1151232
        c1: Fp::from_raw([
            0x678f4479a1151232,
            0x6f45d28e4ef1ecb9,
            0x0b3c35a77291b29d,
            0x07cd6b44b03ef113,
            0xe65e47a7716bc8bc,
            0x9b26f1795339413b,
            0x20d4c11700e83282,
        ]),
    },
    // Fp2(v)**(((p^4) - 1) / 6)
    Fp2 {
        // 0x480000000000360001c950000d7ee0e4a803c956d01c903d720dc8ad8b38dffaf50c100004c37ffffffe
        c0: Fp::from_raw([
            0x100004c37ffffffe,
            0xc8ad8b38dffaf50c,
            0xc956d01c903d720d,
            0x50000d7ee0e4a803,
            0x00000000360001c9,
            0x0000000000004800,
            0x0000000000000000,
        ]),
        c1: Fp::ZERO,
    },
    // Fp2(v)**(((p^5) - 1) / 6)
    Fp2 {
        // 0x17eb3ca29c1fb06e785dae245592ec43d5d373f7950b517d484ead4b6c8a66d46be33bb7a38302e7a63f2ca466b80fadf9ba5891cf2691f7
        c0: Fp::from_raw([
            0xf9ba5891cf2691f7,
            0xa63f2ca466b80fad,
            0x6be33bb7a38302e7,
            0x484ead4b6c8a66d4,
            0xd5d373f7950b517d,
            0x785dae245592ec43,
            0x17eb3ca29c1fb06e,
        ]),
        // 0xc34e729d46d329af08338673b0b9f0e19abaf6f0edcc40514999af25c3c5c8a6c38ae3c44b69e68154c9b4f01fd9e4e6d83622ec9bc6c33
        c1: Fp::from_raw([
            0x6d83622ec9bc6c33,
            0x154c9b4f01fd9e4e,
            0x6c38ae3c44b69e68,
            0x14999af25c3c5c8a,
            0x19abaf6f0edcc405,
            0xf08338673b0b9f0e,
            0x0c34e729d46d329a,
        ]),
    },
    // Fp2(v)**(((p^6) - 1) / 6)
    Fp2 {
        // 0x24000000000024000130e0000d7f70e4a803ca76f439266f443f9a5cda8a6c7be4a7a5fe8fadffd6a2a7e8c30006b9459ffffcd300000000
        c0: Fp::from_raw([
            0x9ffffcd300000000,
            0xa2a7e8c30006b945,
            0xe4a7a5fe8fadffd6,
            0x443f9a5cda8a6c7b,
            0xa803ca76f439266f,
            0x0130e0000d7f70e4,
            0x2400000000002400,
        ]),
        c1: Fp::ZERO,
    },
    // Fp2(v)**(((p^7) - 1) / 6)
    Fp2 {
        // 0x203c52c25746874e2229d623d94e5d17ce7a9c891f19f605e6b5d415217c8387c6b750c6440f92d95437843cdd3f6852711696f310dcfc2e
        c0: Fp::from_raw([
            0x711696f310dcfc2e,
            0x5437843cdd3f6852,
            0xc6b750c6440f92d9,
            0xe6b5d415217c8387,
            0xce7a9c891f19f605,
            0x2229d623d94e5d17,
            0x203c52c25746874e,
        ]),
        // 0xf602612d3852418568d26edf551ceb6db51323e91aa21b8510bca0a8687d7f345a41e9361d2eba148aeb183b3126adaa5f41a8828a75a02
        c1: Fp::from_raw([
            0xa5f41a8828a75a02,
            0x48aeb183b3126ada,
            0x45a41e9361d2eba1,
            0x510bca0a8687d7f3,
            0xdb51323e91aa21b8,
            0x568d26edf551ceb6,
            0x0f602612d3852418,
        ]),
    },
    // Fp2(v)**(((p^8) - 1) / 6)
    Fp2 {
        // 0x24000000000024000130e0000d7f28e4a803ca76be3924a5f43f8cddf9a5c4781b50d5e1ff708dc8d9fa5d8a200bc4398ffff80f80000002
        c0: Fp::from_raw([
            0x8ffff80f80000002,
            0xd9fa5d8a200bc439,
            0x1b50d5e1ff708dc8,
            0xf43f8cddf9a5c478,
            0xa803ca76be3924a5,
            0x0130e0000d7f28e4,
            0x2400000000002400,
        ]),
        c1: Fp::ZERO,
    },
    // Fp2(v)**(((p^9) - 1) / 6)
    Fp2 {
        // 0x851161fbb26d6dfa9cc27ff83bb70d3f8a728918a0ea4889e6726c9b4f21cb35ad4150ea08c8ff1adf85798768758a4775c3e6141b66a37
        c0: Fp::from_raw([
            0x775c3e6141b66a37,
            0xadf85798768758a4,
            0x5ad4150ea08c8ff1,
            0x9e6726c9b4f21cb3,
            0xf8a728918a0ea488,
            0xa9cc27ff83bb70d3,
            0x0851161fbb26d6df,
        ]),
        // 0x32b3ee8ff17f17d6609ee86ba462fa8c1a582cf82cd5db33c722f182a4b7b68d96b70571d1c4d3933621634b114cc8c3870b8595eeaedcf
        c1: Fp::from_raw([
            0x3870b8595eeaedcf,
            0x33621634b114cc8c,
            0xd96b70571d1c4d39,
            0x3c722f182a4b7b68,
            0xc1a582cf82cd5db3,
            0x6609ee86ba462fa8,
            0x032b3ee8ff17f17d,
        ]),
    },
    // Fp2(v)**(((p^10) - 1) / 6)
    Fp2 {
        // 0x24000000000024000130e0000d7f28e4a803ca76be3924a5f43f8cddf9a5c4781b50d5e1ff708dc8d9fa5d8a200bc4398ffff80f80000003
        c0: Fp::from_raw([
            0x8ffff80f80000003,
            0xd9fa5d8a200bc439,
            0x1b50d5e1ff708dc8,
            0xf43f8cddf9a5c478,
            0xa803ca76be3924a5,
            0x0130e0000d7f28e4,
            0x2400000000002400,
        ]),
        c1: Fp::ZERO,
    },
    // Fp2(v)**(((p^11) - 1) / 6)
    Fp2 {
        // 0xc14c35d63e0739188d331dbb7ec84a0d230567f5f2dd4f1fbf0ed116e0005a778c46a46ec2afceefc68bc1e994ea997a645a44130d96e0a
        c0: Fp::from_raw([
            0xa645a44130d96e0a,
            0xfc68bc1e994ea997,
            0x78c46a46ec2afcee,
            0xfbf0ed116e0005a7,
            0xd230567f5f2dd4f1,
            0x88d331dbb7ec84a0,
            0x0c14c35d63e07391,
        ]),
        // 0x17cb18d62b92f16510ada798d273d1d68e581b07e55c626a2fa5ff6a7e4e0ff1786ef7c24af7616e8d5b4d73fe091af7327c9aa4364393ce
        c1: Fp::from_raw([
            0x327c9aa4364393ce,
            0x8d5b4d73fe091af7,
            0x786ef7c24af7616e,
            0x2fa5ff6a7e4e0ff1,
            0x8e581b07e55c626a,
            0x10ada798d273d1d6,
            0x17cb18d62b92f165,
        ]),
    },
];

#[cfg(test)]
mod test {
    macro_rules! test_fp12 {
        ($test:ident, $size: expr) => {
            paste::paste! {
            #[test]
            fn [< $test test >]() {
                use rand_core::SeedableRng;
                use rand_xorshift::XorShiftRng;
                let mut rng = XorShiftRng::from_seed(crate::tests::SEED);
                crate::pluto_eris::fp12::test::$test(&mut rng, $size);
            }
            }
        };
    }
    use ff::Field;
    use rand_core::RngCore;

    use super::*;
    use crate::{arith_test, frobenius_test, setup_f12_test_funcs, test};

    arith_test!(Fp12);

    // F12 specific
    setup_f12_test_funcs!(Fp12, Fp6, Fp2);
    test_fp12!(f12_mul_by_014_, 500);
    test_fp12!(f12_mul_by_034_, 500);
    frobenius_test!(Fp12, Fp, 8);
}
