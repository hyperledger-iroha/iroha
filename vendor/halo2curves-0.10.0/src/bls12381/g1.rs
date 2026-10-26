#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

use core::{
    cmp,
    iter::Sum,
    ops::{Add, Mul, Neg, Sub},
};

use ff::{PrimeField, WithSmallOrderMulGroup};
use group::{
    cofactor::CofactorGroup, ff::Field, prime::PrimeCurveAffine, Curve, Group, GroupEncoding,
};
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use super::{fq::Fq, Fr};
use crate::{
    impl_binops_additive, impl_binops_additive_specify_output, impl_binops_multiplicative,
    impl_binops_multiplicative_mixed, new_curve_impl,
    serde::{Compressed, CompressedFlagConfig},
    Coordinates, CurveAffine, CurveExt,
};

new_curve_impl!(
    (pub),
    G1,
    G1Affine,
    Fq,
    Fr,
    (GENERATOR_X, GENERATOR_Y),
    A,
    B,
    "bls12381_g1",
    |domain_prefix| hash_to_curve(domain_prefix, hash_to_curve_suite(b"BLS12381G1_XMD:SHA-256_SSWU_RO_")),
    crate::serde::CompressedFlagConfig::ThreeSpare
);

impl Compressed<G1Affine> for G1Compressed {
    const CONFIG: CompressedFlagConfig = CompressedFlagConfig::ThreeSpare;
    fn sign(c: &G1Affine) -> Choice {
        c.y.lexicographically_largest() & !c.is_identity()
    }
    fn resolve(x: Fq, sign_set: Choice) -> CtOption<G1Affine> {
        G1Affine::y2(x).sqrt().map(|y| {
            let y = Fq::conditional_select(&y, &-y, sign_set ^ y.lexicographically_largest());
            G1Affine { x, y }
        })
    }
}

impl group::cofactor::CofactorGroup for G1 {
    type Subgroup = G1;

    fn clear_cofactor(&self) -> Self {
        self - self.mul_by_x()
    }

    fn into_subgroup(self) -> CtOption<Self::Subgroup> {
        CtOption::new(self, 1.into())
    }

    fn is_torsion_free(&self) -> Choice {
        self.mul_by_x().mul_by_x().neg().endo().ct_eq(self)
    }
}

const GENERATOR_X: Fq = Fq([
    0x5cb38790fd530c16,
    0x7817fc679976fff5,
    0x154f95c7143ba1c1,
    0xf0ae6acdf3d0e747,
    0xedce6ecc21dbf440,
    0x120177419e0bfb75,
]);

const GENERATOR_Y: Fq = Fq([
    0xbaac93d50ce72271,
    0x8c22631a7918fd8e,
    0xdd595f13570725ce,
    0x51ac582950405194,
    0x0e1c8c3fad0059c0,
    0x0bbc3efc5008a26a,
]);

const A: Fq = Fq::ZERO;
const B: Fq = Fq::from_raw([4, 0, 0, 0, 0, 0]);

impl G1 {
    fn mul_by_x(&self) -> G1 {
        let mut acc = G1::identity();
        for (i, b) in super::BLS_X.into_iter().enumerate() {
            (i != 0).then(|| acc = acc.double());
            (b == 1).then(|| acc += self);
        }
        acc.neg()
    }
}

fn hash_to_curve_suite(domain: &[u8]) -> crate::hash_to_curve::Suite<G1, sha2::Sha256, 64> {
    const SSWU_Z: Fq = Fq::from_raw([11, 0, 0, 0, 0, 0]);

    pub const ISO_A: Fq = Fq([
        0x2f65_aa0e_9af5_aa51,
        0x8646_4c2d_1e84_16c3,
        0xb85c_e591_b7bd_31e2,
        0x27e1_1c91_b5f2_4e7c,
        0x2837_6eda_6bfc_1835,
        0x1554_55c3_e507_1d85,
    ]);

    pub const ISO_B: Fq = Fq([
        0xfb99_6971_fe22_a1e0,
        0x9aa9_3eb3_5b74_2d6f,
        0x8c47_6013_de99_c5c4,
        0x873e_27c3_a221_e571,
        0xca72_b5e4_5a52_d888,
        0x0682_4061_418a_386b,
    ]);

    let iso_map = crate::hash_to_curve::Iso {
        a: ISO_A,
        b: ISO_B,
        map: Box::new(iso_map),
    };

    crate::hash_to_curve::Suite::new(domain, SSWU_Z, crate::hash_to_curve::Method::SSWU(iso_map))
}

/// Maps an iso-G1 point to a G1 point.
fn iso_map(x: Fq, y: Fq, z: Fq) -> G1 {
    const COEFFS: [&[Fq]; 4] = [&ISO11_XNUM, &ISO11_XDEN, &ISO11_YNUM, &ISO11_YDEN];

    // xnum, xden, ynum, yden
    let mut mapvals = [Fq::zero(); 4];

    // pre-compute powers of z
    let zpows = {
        let mut zpows = [Fq::zero(); 15];
        zpows[0] = z;
        for idx in 1..zpows.len() {
            zpows[idx] = zpows[idx - 1] * z;
        }
        zpows
    };

    // compute map value by Horner's rule
    for idx in 0..4 {
        let coeff = COEFFS[idx];
        let clast = coeff.len() - 1;
        mapvals[idx] = coeff[clast];
        for jdx in 0..clast {
            mapvals[idx] = mapvals[idx] * x + zpows[jdx] * coeff[clast - 1 - jdx];
        }
    }

    // x denominator is order 1 less than x numerator, so we need an extra factor of
    // z
    mapvals[1] *= z;

    // multiply result of Y map by the y-coord, y / z
    mapvals[2] *= y;
    mapvals[3] *= z;

    G1 {
        x: mapvals[0] * mapvals[3], // xnum * yden,
        y: mapvals[2] * mapvals[1], // ynum * xden,
        z: mapvals[1] * mapvals[3], // xden * yden
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn hash_to_curve<'a>(
    domain_prefix: &'a str,
    suite: crate::hash_to_curve::Suite<G1, sha2::Sha256, 64>,
) -> Box<dyn Fn(&[u8]) -> G1 + 'a> {
    Box::new(move |message| suite.hash_to_curve(domain_prefix, message).clear_cofactor())
}

#[cfg(test)]
mod test {
    use group::UncompressedEncoding;
    use rand_core::OsRng;

    use super::*;
    #[cfg(feature = "std")]
    use crate::serde::SerdeObject;
    use crate::{arithmetic::CurveEndo, tests::curve::TestH2C};
    crate::curve_testing_suite!(G1);
    crate::curve_testing_suite!(G1, "endo_consistency");
    crate::curve_testing_suite!(G1, "endo");

    #[test]
    fn test_cofactor() {
        assert!(bool::from(
            G1Affine::identity().to_curve().is_torsion_free()
        ));
        assert!(bool::from(
            G1Affine::generator().to_curve().is_torsion_free()
        ));
    }

    #[test]
    fn test_hash_to_curve() {
        [
        TestH2C::<G1Affine>::new(
            b"",
            crate::tests::point_from_hex(
                "052926add2207b76ca4fa57a8734416c8dc95e24501772c814278700eed6d1e4e8cf62d9c09db0fac349612b759e79a1",
                "08ba738453bfed09cb546dbb0783dbb3a5f1f566ed67bb6be0e8c67e2e81a4cc68ee29813bb7994998f3eae0c9c6a265",
            ),
        ),
        TestH2C::<G1Affine>::new(
            b"abc",
            crate::tests::point_from_hex(
                "03567bc5ef9c690c2ab2ecdf6a96ef1c139cc0b2f284dca0a9a7943388a49a3aee664ba5379a7655d3c68900be2f6903",
                "0b9c15f3fe6e5cf4211f346271d7b01c8f3b28be689c8429c85b67af215533311f0b8dfaaa154fa6b88176c229f2885d",
            ),
        ),
        TestH2C::<G1Affine>::new(
            b"abcdef0123456789",
            crate::tests::point_from_hex(
                "11e0b079dea29a68f0383ee94fed1b940995272407e3bb916bbf268c263ddd57a6a27200a784cbc248e84f357ce82d98",
                "03a87ae2caf14e8ee52e51fa2ed8eefe80f02457004ba4d486d6aa1f517c0889501dc7413753f9599b099ebcbbd2d709",
            ),
        ),
        TestH2C::<G1Affine>::new(
            b"q128_qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
            crate::tests::point_from_hex(
                "15f68eaa693b95ccb85215dc65fa81038d69629f70aeee0d0f677cf22285e7bf58d7cb86eefe8f2e9bc3f8cb84fac488",
                "1807a1d50c29f430b8cafc4f8638dfeeadf51211e1602a5f184443076715f91bb90a48ba1e370edce6ae1062f5e6dd38",
            ),
        ),
        TestH2C::<G1Affine>::new(
            b"a512_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            crate::tests::point_from_hex(
                "082aabae8b7dedb0e78aeb619ad3bfd9277a2f77ba7fad20ef6aabdc6c31d19ba5a6d12283553294c1825c4b3ca2dcfe",
                "05b84ae5a942248eea39e1d91030458c40153f3b654ab7872d779ad1e942856a20c438e8d99bc8abfbf74729ce1f7ac8",
            ),
        )
    ].iter().for_each(|test| {
        test.run("QUUX-V01-CS02-with-");
        });
    }
}

/// Coefficients of the 11-isogeny x map's numerator
const ISO11_XNUM: [Fq; 12] = [
    Fq::from_raw([
        0x4d18_b6f3_af00_131c,
        0x19fa_2197_93fe_e28c,
        0x3f28_85f1_467f_19ae,
        0x23dc_ea34_f2ff_b304,
        0xd15b_58d2_ffc0_0054,
        0x0913_be20_0a20_bef4,
    ]),
    Fq::from_raw([
        0x8989_8538_5cdb_bd8b,
        0x3c79_e43c_c7d9_66aa,
        0x1597_e193_f4cd_233a,
        0x8637_ef1e_4d66_23ad,
        0x11b2_2dee_d20d_827b,
        0x0709_7bc5_9987_84ad,
    ]),
    Fq::from_raw([
        0xa542_583a_480b_664b,
        0xfc71_69c0_26e5_68c6,
        0x5ba2_ef31_4ed8_b5a6,
        0x5b54_91c0_5102_f0e7,
        0xdf6e_9970_7d2a_0079,
        0x0784_151e_d760_5524,
    ]),
    Fq::from_raw([
        0x494e_2128_70f7_2741,
        0xab9b_e52f_bda4_3021,
        0x26f5_5779_94e3_4c3d,
        0x049d_fee8_2aef_bd60,
        0x65da_dd78_2850_5289,
        0x0e93_d431_ea01_1aeb,
    ]),
    Fq::from_raw([
        0x90ee_774b_d6a7_4d45,
        0x7ada_1c8a_41bf_b185,
        0x0f1a_8953_b325_f464,
        0x104c_2421_1be4_805c,
        0x1691_39d3_19ea_7a8f,
        0x09f2_0ead_8e53_2bf6,
    ]),
    Fq::from_raw([
        0x6ddd_93e2_f436_26b7,
        0xa548_2c9a_a1cc_d7bd,
        0x1432_4563_1883_f4bd,
        0x2e0a_94cc_f77e_c0db,
        0xb028_2d48_0e56_489f,
        0x18f4_bfcb_b436_8929,
    ]),
    Fq::from_raw([
        0x23c5_f0c9_5340_2dfd,
        0x7a43_ff69_58ce_4fe9,
        0x2c39_0d3d_2da5_df63,
        0xd0df_5c98_e1f9_d70f,
        0xffd8_9869_a572_b297,
        0x1277_ffc7_2f25_e8fe,
    ]),
    Fq::from_raw([
        0x79f4_f049_0f06_a8a6,
        0x85f8_94a8_8030_fd81,
        0x12da_3054_b18b_6410,
        0xe2a5_7f65_0588_0d65,
        0xbba0_74f2_60e4_00f1,
        0x08b7_6279_f621_d028,
    ]),
    Fq::from_raw([
        0xe672_45ba_78d5_b00b,
        0x8456_ba9a_1f18_6475,
        0x7888_bff6_e6b3_3bb4,
        0xe215_85b9_a30f_86cb,
        0x05a6_9cdc_ef55_feee,
        0x09e6_99dd_9adf_a5ac,
    ]),
    Fq::from_raw([
        0x0de5_c357_bff5_7107,
        0x0a0d_b4ae_6b1a_10b2,
        0xe256_bb67_b3b3_cd8d,
        0x8ad4_5657_4e9d_b24f,
        0x0443_915f_50fd_4179,
        0x098c_4bf7_de8b_6375,
    ]),
    Fq::from_raw([
        0xe6b0_617e_7dd9_29c7,
        0xfe6e_37d4_4253_7375,
        0x1daf_deda_137a_489e,
        0xe4ef_d1ad_3f76_7ceb,
        0x4a51_d866_7f0f_e1cf,
        0x054f_df4b_bf1d_821c,
    ]),
    Fq::from_raw([
        0x72db_2a50_658d_767b,
        0x8abf_91fa_a257_b3d5,
        0xe969_d683_3764_ab47,
        0x4641_7014_2a10_09eb,
        0xb14f_01aa_db30_be2f,
        0x18ae_6a85_6f40_715d,
    ]),
];

/// Coefficients of the 11-isogeny x map's denominator
const ISO11_XDEN: [Fq; 11] = [
    Fq::from_raw([
        0xb962_a077_fdb0_f945,
        0xa6a9_740f_efda_13a0,
        0xc14d_568c_3ed6_c544,
        0xb43f_c37b_908b_133e,
        0x9c0b_3ac9_2959_9016,
        0x0165_aa6c_93ad_115f,
    ]),
    Fq::from_raw([
        0x2327_9a3b_a506_c1d9,
        0x92cf_ca0a_9465_176a,
        0x3b29_4ab1_3755_f0ff,
        0x116d_da1c_5070_ae93,
        0xed45_3092_4cec_2045,
        0x0833_83d6_ed81_f1ce,
    ]),
    Fq::from_raw([
        0x9885_c2a6_449f_ecfc,
        0x4a2b_54cc_d377_33f0,
        0x17da_9ffd_8738_c142,
        0xa0fb_a727_32b3_fafd,
        0xff36_4f36_e54b_6812,
        0x0f29_c13c_6605_23e2,
    ]),
    Fq::from_raw([
        0xe349_cc11_8278_f041,
        0xd487_228f_2f32_04fb,
        0xc9d3_2584_9ade_5150,
        0x43a9_2bd6_9c15_c2df,
        0x1c2c_7844_bc41_7be4,
        0x1202_5184_f407_440c,
    ]),
    Fq::from_raw([
        0x587f_65ae_6acb_057b,
        0x1444_ef32_5140_201f,
        0xfbf9_95e7_1270_da49,
        0xccda_0660_7243_6a42,
        0x7408_904f_0f18_6bb2,
        0x13b9_3c63_edf6_c015,
    ]),
    Fq::from_raw([
        0xfb91_8622_cd14_1920,
        0x4a4c_6442_3eca_ddb4,
        0x0beb_2329_27f7_fb26,
        0x30f9_4df6_f83a_3dc2,
        0xaeed_d424_d780_f388,
        0x06cc_402d_d594_bbeb,
    ]),
    Fq::from_raw([
        0xd41f_7611_51b2_3f8f,
        0x32a9_2465_4357_19b3,
        0x64f4_36e8_88c6_2cb9,
        0xdf70_a9a1_f757_c6e4,
        0x6933_a38d_5b59_4c81,
        0x0c6f_7f72_37b4_6606,
    ]),
    Fq::from_raw([
        0x693c_0874_7876_c8f7,
        0x22c9_850b_f9cf_80f0,
        0x8e90_71da_b950_c124,
        0x89bc_62d6_1c7b_af23,
        0xbc6b_e2d8_dad5_7c23,
        0x1791_6987_aa14_a122,
    ]),
    Fq::from_raw([
        0x1be3_ff43_9c13_16fd,
        0x9965_243a_7571_dfa7,
        0xc7f7_f629_62f5_cd81,
        0x32c6_aa9a_f394_361c,
        0xbbc2_ee18_e1c2_27f4,
        0x0c10_2cba_c531_bb34,
    ]),
    Fq::from_raw([
        0x9976_14c9_7bac_bf07,
        0x61f8_6372_b991_92c0,
        0x5b8c_95fc_1435_3fc3,
        0xca2b_066c_2a87_492f,
        0x1617_8f5b_bf69_8711,
        0x12a6_dcd7_f0f4_e0e8,
    ]),
    Fq::from_raw([
        0x7609_0000_0002_fffd,
        0xebf4_000b_c40c_0002,
        0x5f48_9857_53c7_58ba,
        0x77ce_5853_7052_5745,
        0x5c07_1a97_a256_ec6d,
        0x15f6_5ec3_fa80_e493,
    ]),
];

/// Coefficients of the 11-isogeny y map's numerator
const ISO11_YNUM: [Fq; 16] = [
    Fq::from_raw([
        0x2b56_7ff3_e283_7267,
        0x1d4d_9e57_b958_a767,
        0xce02_8fea_04bd_7373,
        0xcc31_a30a_0b6c_d3df,
        0x7d7b_18a6_8269_2693,
        0x0d30_0744_d42a_0310,
    ]),
    Fq::from_raw([
        0x99c2_555f_a542_493f,
        0xfe7f_53cc_4874_f878,
        0x5df0_608b_8f97_608a,
        0x14e0_3832_052b_49c8,
        0x7063_26a6_957d_d5a4,
        0x0a8d_add9_c241_4555,
    ]),
    Fq::from_raw([
        0x13d9_4292_2a5c_f63a,
        0x357e_33e3_6e26_1e7d,
        0xcf05_a27c_8456_088d,
        0x0000_bd1d_e7ba_50f0,
        0x83d0_c753_2f8c_1fde,
        0x13f7_0bf3_8bbf_2905,
    ]),
    Fq::from_raw([
        0x5c57_fd95_bfaf_bdbb,
        0x28a3_59a6_5e54_1707,
        0x3983_ceb4_f636_0b6d,
        0xafe1_9ff6_f97e_6d53,
        0xb346_8f45_5019_2bf7,
        0x0bb6_cde4_9d8b_a257,
    ]),
    Fq::from_raw([
        0x590b_62c7_ff8a_513f,
        0x314b_4ce3_72ca_cefd,
        0x6bef_32ce_94b8_a800,
        0x6ddf_84a0_9571_3d5f,
        0x64ea_ce4c_b098_2191,
        0x0386_213c_651b_888d,
    ]),
    Fq::from_raw([
        0xa531_0a31_111b_bcdd,
        0xa14a_c0f5_da14_8982,
        0xf9ad_9cc9_5423_d2e9,
        0xaa6e_c095_283e_e4a7,
        0xcf5b_1f02_2e1c_9107,
        0x01fd_df5a_ed88_1793,
    ]),
    Fq::from_raw([
        0x65a5_72b0_d7a7_d950,
        0xe25c_2d81_8347_3a19,
        0xc2fc_ebe7_cb87_7dbd,
        0x05b2_d36c_769a_89b0,
        0xba12_961b_e86e_9efb,
        0x07eb_1b29_c1df_de1f,
    ]),
    Fq::from_raw([
        0x93e0_9572_f7c4_cd24,
        0x364e_9290_7679_5091,
        0x8569_467e_68af_51b5,
        0xa47d_a894_39f5_340f,
        0xf4fa_9180_82e4_4d64,
        0x0ad5_2ba3_e669_5a79,
    ]),
    Fq::from_raw([
        0x9114_2984_4e0d_5f54,
        0xd03f_51a3_516b_b233,
        0x3d58_7e56_4053_6e66,
        0xfa86_d2a3_a9a7_3482,
        0xa90e_d5ad_f1ed_5537,
        0x149c_9c32_6a5e_7393,
    ]),
    Fq::from_raw([
        0x462b_beb0_3c12_921a,
        0xdc9a_f5fa_0a27_4a17,
        0x9a55_8ebd_e836_ebed,
        0x649e_f8f1_1a4f_ae46,
        0x8100_e165_2b3c_dc62,
        0x1862_bd62_c291_dacb,
    ]),
    Fq::from_raw([
        0x05c9_b8ca_89f1_2c26,
        0x0194_160f_a9b9_ac4f,
        0x6a64_3d5a_6879_fa2c,
        0x1466_5bdd_8846_e19d,
        0xbb1d_0d53_af3f_f6bf,
        0x12c7_e1c3_b289_62e5,
    ]),
    Fq::from_raw([
        0xb55e_bf90_0b8a_3e17,
        0xfedc_77ec_1a92_01c4,
        0x1f07_db10_ea1a_4df4,
        0x0dfb_d15d_c41a_594d,
        0x3895_47f2_334a_5391,
        0x0241_9f98_1658_71a4,
    ]),
    Fq::from_raw([
        0xb416_af00_0745_fc20,
        0x8e56_3e9d_1ea6_d0f5,
        0x7c76_3e17_763a_0652,
        0x0145_8ef0_159e_bbef,
        0x8346_fe42_1f96_bb13,
        0x0d2d_7b82_9ce3_24d2,
    ]),
    Fq::from_raw([
        0x9309_6bb5_38d6_4615,
        0x6f2a_2619_951d_823a,
        0x8f66_b3ea_5951_4fa4,
        0xf563_e637_04f7_092f,
        0x724b_136c_4cf2_d9fa,
        0x0469_59cf_cfd0_bf49,
    ]),
    Fq::from_raw([
        0xea74_8d4b_6e40_5346,
        0x91e9_079c_2c02_d58f,
        0x4106_4965_946d_9b59,
        0xa067_31f1_d2bb_e1ee,
        0x07f8_97e2_67a3_3f1b,
        0x1017_2909_1921_0e5f,
    ]),
    Fq::from_raw([
        0x872a_a6c1_7d98_5097,
        0xeecc_5316_1264_562a,
        0x07af_e37a_fff5_5002,
        0x5475_9078_e5be_6838,
        0xc4b9_2d15_db8a_cca8,
        0x106d_87d1_b51d_13b9,
    ]),
];

/// Coefficients of the 11-isogeny y map's denominator
const ISO11_YDEN: [Fq; 16] = [
    Fq::from_raw([
        0xeb6c_359d_47e5_2b1c,
        0x18ef_5f8a_1063_4d60,
        0xddfa_71a0_889d_5b7e,
        0x723e_71dc_c5fc_1323,
        0x52f4_5700_b70d_5c69,
        0x0a8b_981e_e476_91f1,
    ]),
    Fq::from_raw([
        0x616a_3c4f_5535_b9fb,
        0x6f5f_0373_95db_d911,
        0xf25f_4cc5_e35c_65da,
        0x3e50_dffe_a3c6_2658,
        0x6a33_dca5_2356_0776,
        0x0fad_eff7_7b6b_fe3e,
    ]),
    Fq::from_raw([
        0x2be9_b66d_f470_059c,
        0x24a2_c159_a3d3_6742,
        0x115d_be7a_d10c_2a37,
        0xb663_4a65_2ee5_884d,
        0x04fe_8bb2_b8d8_1af4,
        0x01c2_a7a2_56fe_9c41,
    ]),
    Fq::from_raw([
        0xf27b_f8ef_3b75_a386,
        0x898b_3674_76c9_073f,
        0x2448_2e6b_8c2f_4e5f,
        0xc8e0_bbd6_fe11_0806,
        0x59b0_c17f_7631_448a,
        0x1103_7cd5_8b3d_bfbd,
    ]),
    Fq::from_raw([
        0x31c7_912e_a267_eec6,
        0x1dbf_6f1c_5fcd_b700,
        0xd30d_4fe3_ba86_fdb1,
        0x3cae_528f_bee9_a2a4,
        0xb1cc_e69b_6aa9_ad9a,
        0x0443_93bb_632d_94fb,
    ]),
    Fq::from_raw([
        0xc66e_f6ef_eeb5_c7e8,
        0x9824_c289_dd72_bb55,
        0x71b1_a4d2_f119_981d,
        0x104f_c1aa_fb09_19cc,
        0x0e49_df01_d942_a628,
        0x096c_3a09_7732_72d4,
    ]),
    Fq::from_raw([
        0x9abc_11eb_5fad_eff4,
        0x32dc_a50a_8857_28f0,
        0xfb1f_a372_1569_734c,
        0xc4b7_6271_ea65_06b3,
        0xd466_a755_99ce_728e,
        0x0c81_d464_5f4c_b6ed,
    ]),
    Fq::from_raw([
        0x4199_f10e_5b8b_e45b,
        0xda64_e495_b1e8_7930,
        0xcb35_3efe_9b33_e4ff,
        0x9e9e_fb24_aa64_24c6,
        0xf08d_3368_0a23_7465,
        0x0d33_7802_3e4c_7406,
    ]),
    Fq::from_raw([
        0x7eb4_ae92_ec74_d3a5,
        0xc341_b4aa_9fac_3497,
        0x5be6_0389_9e90_7687,
        0x03bf_d9cc_a75c_bdeb,
        0x564c_2935_a96b_fa93,
        0x0ef3_c333_71e2_fdb5,
    ]),
    Fq::from_raw([
        0x7ee9_1fd4_49f6_ac2e,
        0xe5d5_bd5c_b935_7a30,
        0x773a_8ca5_196b_1380,
        0xd0fd_a172_174e_d023,
        0x6cb9_5e0f_a776_aead,
        0x0d22_d5a4_0cec_7cff,
    ]),
    Fq::from_raw([
        0xf727_e092_85fd_8519,
        0xdc9d_55a8_3017_897b,
        0x7549_d8bd_0578_94ae,
        0x1784_1961_3d90_d8f8,
        0xfce9_5ebd_eb5b_490a,
        0x0467_ffae_f23f_c49e,
    ]),
    Fq::from_raw([
        0xc176_9e6a_7c38_5f1b,
        0x79bc_930d_eac0_1c03,
        0x5461_c75a_23ed_e3b5,
        0x6e20_829e_5c23_0c45,
        0x828e_0f1e_772a_53cd,
        0x116a_efa7_4912_7bff,
    ]),
    Fq::from_raw([
        0x101c_10bf_2744_c10a,
        0xbbf1_8d05_3a6a_3154,
        0xa0ec_f39e_f026_f602,
        0xfc00_9d49_96dc_5153,
        0xb900_0209_d5bd_08d3,
        0x189e_5fe4_470c_d73c,
    ]),
    Fq::from_raw([
        0x7ebd_546c_a157_5ed2,
        0xe47d_5a98_1d08_1b55,
        0x57b2_b625_b6d4_ca21,
        0xb0a1_ba04_2285_20cc,
        0x9873_8983_c210_7ff3,
        0x13dd_dbc4_799d_81d6,
    ]),
    Fq::from_raw([
        0x0931_9f2e_3983_4935,
        0x039e_952c_bdb0_5c21,
        0x55ba_77a9_a2f7_6493,
        0xfd04_e3df_c608_6467,
        0xfb95_832e_7d78_742e,
        0x0ef9_c24e_ccaf_5e0e,
    ]),
    Fq::from_raw([
        0x7609_0000_0002_fffd,
        0xebf4_000b_c40c_0002,
        0x5f48_9857_53c7_58ba,
        0x77ce_5853_7052_5745,
        0x5c07_1a97_a256_ec6d,
        0x15f6_5ec3_fa80_e493,
    ]),
];
