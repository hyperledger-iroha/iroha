use ff::Field;

use super::{fp::Fp, fp2::Fp2};
use crate::ff_ext::{
    cubic::{CubicExtField, CubicExtFieldArith, CubicSparseMul},
    ExtField,
};

// -BETA is a cubic non-residue in Fp2. Fp6 = Fp2[X]/(X^3 + BETA)
// We introduce the variable v such that v^3 = -BETA
// BETA = - 57/(z+3)
crate::impl_binops_additive!(Fp6, Fp6);
crate::impl_binops_multiplicative!(Fp6, Fp6);
crate::impl_binops_calls!(Fp6);
crate::impl_sum_prod!(Fp6);
pub type Fp6 = CubicExtField<Fp2>;

impl CubicExtFieldArith for Fp6 {
    type Base = Fp2;
}

impl CubicSparseMul for Fp6 {
    type Base = Fp2;
}

impl ExtField for Fp6 {
    const NON_RESIDUE: Self = Fp6::new(Fp2::ZERO, Fp2::ONE, Fp2::ZERO);

    fn frobenius_map(&mut self, power: usize) {
        self.c0.frobenius_map(power);
        self.c1.frobenius_map(power);
        self.c2.frobenius_map(power);
        self.c1.mul_assign(&FROBENIUS_COEFF_FP6_C1[power % 6]);
        self.c2.mul_assign(&FROBENIUS_COEFF_FP6_C2[power % 6]);
    }

    fn mul_by_nonresidue(self: &Fp6) -> Fp6 {
        let c0 = self.c2.mul_by_nonresidue();
        let c1 = self.c0;
        let c2 = self.c1;
        Self { c0, c1, c2 }
    }
}

/// Fp2 coefficients for the efficient computation of Frobenius Endomorphism in
/// Fp6.
pub(crate) const FROBENIUS_COEFF_FP6_C1: [Fp2; 6] = [
    // Fp2(v^3)**(((p^0) - 1) / 3)
    Fp2::ONE,
    // Fp2(v^3)**(((p^1) - 1) / 3)
    Fp2 {
        // 0x120de97f024c55bc3bc0d351f4c70da1e3886170077a50986f93678bc921dcd5041bc4bb14cc42dc52e787634eccc335a001825382850d03
        c0: Fp::from_raw([
            0xa001825382850d03,
            0x52e787634eccc335,
            0x041bc4bb14cc42dc,
            0x6f93678bc921dcd5,
            0xe3886170077a5098,
            0x3bc0d351f4c70da1,
            0x120de97f024c55bc,
        ]),
        // 0x2096f3f804d973afd82becc2ef081b76132461908eadbe3da1a7f5502b7091965efa1ddf4658080413be1b7cd3c9ea0e2772fea378a9b322
        c1: Fp::from_raw([
            0x2772fea378a9b322,
            0x13be1b7cd3c9ea0e,
            0x5efa1ddf46580804,
            0xa1a7f5502b709196,
            0x132461908eadbe3d,
            0xd82becc2ef081b76,
            0x2096f3f804d973af,
        ]),
    },
    // Fp2(v^3)**(((p^2) - 1) / 3)
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
    // Fp2(v^3)**(((p^3) - 1) / 3)
    Fp2 {
        // 0x1f9cd069c59f50a72511749de232911d833b798e78bd98c02913e38315a71c287cd52ae30d09b78a8b43b17b4c3ea938a04518fa783eb497
        c0: Fp::from_raw([
            0xa04518fa783eb497,
            0x8b43b17b4c3ea938,
            0x7cd52ae30d09b78a,
            0x2913e38315a71c28,
            0x833b798e78bd98c0,
            0x2511749de232911d,
            0x1f9cd069c59f50a7,
        ]),
        // 0x23affd628747cbaec26943f93dc9eab63f4af36699fe6d74c0aa2122aa7cb689e8faacb3479a973a4a728fcb77b150ee77240d4066e42ac5
        c1: Fp::from_raw([
            0x77240d4066e42ac5,
            0x4a728fcb77b150ee,
            0xe8faacb3479a973a,
            0xc0aa2122aa7cb689,
            0x3f4af36699fe6d74,
            0xc26943f93dc9eab6,
            0x23affd628747cbae,
        ]),
    },
    // Fp2(v^3)**(((p^4) - 1) / 3)
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
    // Fp2(v^3)**(((p^5) - 1) / 3)
    Fp2 {
        // 0x165546173814a19ca18f781044054309e943b9ef683a6385efd7e9aad64bdffa485e5c5efd860546672498a76502061cffb95e58053c3e68
        c0: Fp::from_raw([
            0xffb95e58053c3e68,
            0x672498a76502061c,
            0x485e5c5efd860546,
            0xefd7e9aad64bdffa,
            0xe943b9ef683a6385,
            0xa18f781044054309,
            0x165546173814a19c,
        ]),
        // 0x3b90ea573df08a167cc8f43ee2cdb9cfd983ff6bfc6212c262d1e46df2790d7815a816a9169606ee71f263db492378ea168edc22072221b
        c1: Fp::from_raw([
            0xa168edc22072221b,
            0xe71f263db492378e,
            0x815a816a9169606e,
            0x262d1e46df2790d7,
            0xfd983ff6bfc6212c,
            0x67cc8f43ee2cdb9c,
            0x03b90ea573df08a1,
        ]),
    },
];

/// Fp2 coefficients for the efficient computation of Frobenius Endomorphism in
/// Fp6.
pub(crate) const FROBENIUS_COEFF_FP6_C2: [Fp2; 6] = [
    // Fp2(v^3)**(((2p^0) - 2) / 3)
    Fp2::ONE,
    // Fp2(v^3)**(((2p^1) - 2) / 3)
    Fp2 {
        // 0x93733692ce3cdcfc34610bac6bd22c4dc590efb038c82998c9549048e7b424cc00e17ffb4a61950d0ec132a7b38f09db0a818e422737f7c
        c0: Fp::from_raw([
            0xb0a818e422737f7c,
            0xd0ec132a7b38f09d,
            0xc00e17ffb4a61950,
            0x8c9549048e7b424c,
            0xdc590efb038c8299,
            0xc34610bac6bd22c4,
            0x093733692ce3cdcf,
        ]),
        // 0x12cb19daadc92882ba3593aa6f3e6bf426f29bd46039e3036f61d0bd35f39ebecdac3209d9df546061c90b4940d9031c240ce398421dc7dc
        c1: Fp::from_raw([
            0x240ce398421dc7dc,
            0x61c90b4940d9031c,
            0xcdac3209d9df5460,
            0x6f61d0bd35f39ebe,
            0x26f29bd46039e303,
            0xba3593aa6f3e6bf4,
            0x12cb19daadc92882,
        ]),
    },
    // Fp2(v^3)**(((2p^2) - 2) / 3)
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
    // Fp2(v^3)**(((2p^3) - 2) / 3)
    Fp2 {
        // 0x85cc83a7eeba2ef5f7dd2f9f1405312b2ce0cbc85b8561e1657aaf1e85b82299aa5ace8b26b78d88f57e1c7a87f75556885980d6c8d2186
        c0: Fp::from_raw([
            0x6885980d6c8d2186,
            0x8f57e1c7a87f7555,
            0x9aa5ace8b26b78d8,
            0x1657aaf1e85b8229,
            0xb2ce0cbc85b8561e,
            0x5f7dd2f9f1405312,
            0x085cc83a7eeba2ef,
        ]),
        // 0xda3357ee4e6a9836af75e8ec0dbd23e7abc03d404620899ee0ea8b684b9400d58d5ebe487e523680bbe8a0dd9ea1d312bca2a953ab51c9b
        c1: Fp::from_raw([
            0x2bca2a953ab51c9b,
            0x0bbe8a0dd9ea1d31,
            0x58d5ebe487e52368,
            0xee0ea8b684b9400d,
            0x7abc03d404620899,
            0x6af75e8ec0dbd23e,
            0x0da3357ee4e6a983,
        ]),
    },
    // Fp2(v^3)**(((2p^4) - 2) / 3)
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
    // Fp2(v^3)**(((2p^5) - 2) / 3)
    Fp2 {
        // 0x126c045c5430b340de6cfc4b5581fb0d18dcaebf6af44db7a152a66663b3a80589f3e116289c6dad4263f3d0dc4e535286d24be170ff5eff
        c0: Fp::from_raw([
            0x86d24be170ff5eff,
            0x4263f3d0dc4e5352,
            0x89f3e116289c6dad,
            0xa152a66663b3a805,
            0x18dcaebf6af44db7,
            0xde6cfc4b5581fb0d,
            0x126c045c5430b340,
        ]),
        // 0x391b0a66d5051f9dc03edc6dd6532b206552ace8f9d3ad1e6cf20e91fdd8dafbe2588102de9880e3520536be54398f85028eea5832d1b8a
        c1: Fp::from_raw([
            0x5028eea5832d1b8a,
            0x3520536be54398f8,
            0xbe2588102de9880e,
            0xe6cf20e91fdd8daf,
            0x06552ace8f9d3ad1,
            0xdc03edc6dd6532b2,
            0x0391b0a66d5051f9,
        ]),
    },
];

#[cfg(test)]
mod test {
    use rand_core::RngCore;

    use super::*;
    use crate::{arith_test, frobenius_test, setup_f6_test_funcs, test};

    macro_rules! test_fp6 {
        ($test:ident, $size: expr) => {
            paste::paste! {
            #[test]
            fn [< $test test >]() {
                use rand_core::SeedableRng;
                use rand_xorshift::XorShiftRng;
                let mut rng = XorShiftRng::from_seed(crate::tests::SEED);
                crate::pluto_eris::fp6::test::$test(&mut rng, $size);
            }
            }
        };
    }

    arith_test!(Fp6);
    setup_f6_test_funcs!(Fp6, Fp2);
    test_fp6!(f6_mul_nonresidue_, 1000);
    test_fp6!(f6_mul_by_1_, 1000);
    test_fp6!(f6_mul_by_01_, 1000);
    frobenius_test!(Fp6, Fp, 10);

    #[test]
    fn test_fp6_mul_nonresidue() {
        let e = Fp6::random(rand_core::OsRng);
        let a0 = e.mul_by_nonresidue();
        let a1 = e * Fp6::NON_RESIDUE;

        assert_eq!(a0, a1);
    }
}
