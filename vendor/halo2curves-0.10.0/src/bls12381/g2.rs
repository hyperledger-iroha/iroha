#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec::Vec};

use core::{
    cmp,
    fmt::Debug,
    iter::Sum,
    ops::{Add, Mul, Neg, Sub},
};

use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use crate::{
    bls12381::{fq::Fq, fq2::Fq2, fr::Fr},
    ff::{Field, PrimeField, WithSmallOrderMulGroup},
    ff_ext::ExtField,
    group::{cofactor::CofactorGroup, prime::PrimeCurveAffine, Curve, Group, GroupEncoding},
    impl_binops_additive, impl_binops_additive_specify_output, impl_binops_multiplicative,
    impl_binops_multiplicative_mixed, new_curve_impl,
    serde::{Compressed, CompressedFlagConfig},
    Coordinates, CurveAffine, CurveExt,
};

const G2_GENERATOR_X: Fq2 = Fq2 {
    c0: Fq([
        0xf5f2_8fa2_0294_0a10,
        0xb3f5_fb26_87b4_961a,
        0xa1a8_93b5_3e2a_e580,
        0x9894_999d_1a3c_aee9,
        0x6f67_b763_1863_366b,
        0x0581_9192_4350_bcd7,
    ]),
    c1: Fq([
        0xa5a9_c075_9e23_f606,
        0xaaa0_c59d_bccd_60c3,
        0x3bb1_7e18_e286_7806,
        0x1b1a_b6cc_8541_b367,
        0xc2b6_ed0e_f215_8547,
        0x1192_2a09_7360_edf3,
    ]),
};

const G2_GENERATOR_Y: Fq2 = Fq2 {
    c0: Fq([
        0x4c73_0af8_6049_4c4a,
        0x597c_fa1f_5e36_9c5a,
        0xe7e6_856c_aa0a_635a,
        0xbbef_b5e9_6e0d_495f,
        0x07d3_a975_f0ef_25a2,
        0x0083_fd8e_7e80_dae5,
    ]),
    c1: Fq([
        0xadc0_fc92_df64_b05d,
        0x18aa_270a_2b14_61dc,
        0x86ad_ac6a_3be4_eba0,
        0x7949_5c4e_c93d_a33a,
        0xe717_5850_a43c_caed,
        0x0b2b_c2a1_63de_1bf2,
    ]),
};

const G2_B: Fq2 = Fq2 {
    c0: Fq::from_raw([4, 0, 0, 0, 0, 0]),
    c1: Fq::from_raw([4, 0, 0, 0, 0, 0]),
};

const G2_A: Fq2 = Fq2::ZERO;

new_curve_impl!(
    (pub),
    G2,
    G2Affine,
    Fq2,
    Fr,
    (G2_GENERATOR_X, G2_GENERATOR_Y),
    G2_A,
    G2_B,
    "bls12381_g2",
    |domain_prefix| hash_to_curve(domain_prefix, hash_to_curve_suite(b"BLS12381G2_XMD:SHA-256_SSWU_RO_")),
    crate::serde::CompressedFlagConfig::ThreeSpare

);

impl crate::serde::endian::EndianRepr for Fq2 {
    const ENDIAN: crate::serde::endian::Endian = Fq::ENDIAN;

    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> subtle::CtOption<Self> {
        Fq2::from_bytes(bytes[..Fq2::SIZE].try_into().unwrap())
    }
}

impl Compressed<G2Affine> for G2Compressed {
    const CONFIG: CompressedFlagConfig = CompressedFlagConfig::ThreeSpare;
    fn sign(c: &G2Affine) -> Choice {
        c.y.lexicographically_largest() & !c.is_identity()
    }
    fn resolve(x: Fq2, sign_set: Choice) -> CtOption<G2Affine> {
        G2Affine::y2(x).sqrt().map(|y| {
            let y = Fq2::conditional_select(&y, &-y, sign_set ^ y.lexicographically_largest());
            G2Affine { x, y }
        })
    }
}

impl group::cofactor::CofactorGroup for G2 {
    type Subgroup = G2;

    /// Clears the cofactor, using [Budroni-Pintore](https://ia.cr/2017/419).
    /// This is equivalent to multiplying by $h\_\textrm{eff} = 3(z^2 - 1) \cdot
    /// h_2$, where $h_2$ is the cofactor of $\mathbb{G}\_2$ and $z$ is the
    /// parameter of BLS12-381.
    fn clear_cofactor(&self) -> G2 {
        let t1 = self.mul_by_x(); // [x] P
        let t2 = self.psi(); // psi(P)

        self.double().psi2() // psi^2(2P)
            + (t1 + t2).mul_by_x() // psi^2(2P) + [x^2] P + [x] psi(P)
            - t1 // psi^2(2P) + [x^2 - x] P + [x] psi(P)
            - t2 // psi^2(2P) + [x^2 - x] P + [x - 1] psi(P)
            - self // psi^2(2P) + [x^2 - x - 1] P + [x - 1] psi(P)
    }

    fn into_subgroup(self) -> CtOption<Self::Subgroup> {
        CtOption::new(self, 1.into())
    }

    /// Returns true if this point is free of an $h$-torsion component, and so
    /// it exists within the $q$-order subgroup $\mathbb{G}_2$. This should
    /// always return true unless an "unchecked" API was used.
    fn is_torsion_free(&self) -> Choice {
        // Algorithm from Section 4 of https://eprint.iacr.org/2021/1130
        // Updated proof of correctness in https://eprint.iacr.org/2022/352
        //
        // Check that psi(P) == [x] P
        self.psi().ct_eq(&self.mul_by_x())
    }
}

impl G2 {
    /// Multiply `self` by `crate::BLS_X`, using double and add.

    fn mul_by_x(&self) -> G2 {
        let mut acc = G2::identity();
        for (i, b) in super::BLS_X.into_iter().enumerate() {
            (i != 0).then(|| acc = acc.double());
            (b == 1).then(|| acc += self);
        }
        acc.neg()
    }

    fn psi(&self) -> G2 {
        // 1 / ((u+1) ^ ((q-1)/3))
        let psi_coeff_x = Fq2 {
            c0: Fq::zero(),
            c1: Fq([
                0x890dc9e4867545c3,
                0x2af322533285a5d5,
                0x50880866309b7e2c,
                0xa20d1b8c7e881024,
                0x14e4f04fe2db9068,
                0x14e56d3f1564853a,
            ]),
        };
        // 1 / ((u+1) ^ (p-1)/2)
        let psi_coeff_y = Fq2 {
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
        };

        // x = frobenius(x)/((u+1)^((p-1)/3))
        let mut x = self.x;
        x.frobenius_map(1);
        x.mul_assign(&psi_coeff_x);

        // y = frobenius(y)/(u+1)^((p-1)/2)
        let mut y = self.y;
        y.frobenius_map(1);
        y.mul_assign(&psi_coeff_y);

        // z = frobenius(z)
        let mut z = self.z;
        z.frobenius_map(1);

        G2 { x, y, z }
    }

    fn psi2(&self) -> G2 {
        // 1 / 2 ^ ((q-1)/3)
        let psi2_coeff_x = Fq2 {
            c0: Fq([
                0xcd03c9e48671f071,
                0x5dab22461fcda5d2,
                0x587042afd3851b95,
                0x8eb60ebe01bacb9e,
                0x03f97d6e83d050d2,
                0x18f0206554638741,
            ]),
            c1: Fq::zero(),
        };

        G2 {
            // x = frobenius^2(x)/2^((p-1)/3); note that q^2 is the order of the field.
            x: self.x * psi2_coeff_x,
            // y = -frobenius^2(y); note that q^2 is the order of the field.
            y: self.y.neg(),
            // z = z
            z: self.z,
        }
    }
}

fn hash_to_curve_suite(domain: &[u8]) -> crate::hash_to_curve::Suite<G2, sha2::Sha256, 128> {
    const SSWU_Z: Fq2 = Fq2 {
        c0: Fq([
            0x87eb_ffff_fff9_555c,
            0x656f_ffe5_da8f_fffa,
            0x0fd0_7493_45d3_3ad2,
            0xd951_e663_0665_76f4,
            0xde29_1a3d_41e9_80d3,
            0x0815_664c_7dfe_040d,
        ]),
        c1: Fq([
            0x43f5_ffff_fffc_aaae,
            0x32b7_fff2_ed47_fffd,
            0x07e8_3a49_a2e9_9d69,
            0xeca8_f331_8332_bb7a,
            0xef14_8d1e_a0f4_c069,
            0x040a_b326_3eff_0206,
        ]),
    };

    const ISO_A: Fq2 = Fq2 {
        c0: Fq::zero(),
        c1: Fq([
            0xe53a_0000_0313_5242,
            0x0108_0c0f_def8_0285,
            0xe788_9edb_e340_f6bd,
            0x0b51_3751_2631_0601,
            0x02d6_9857_17c7_44ab,
            0x1220_b4e9_79ea_5467,
        ]),
    };

    const ISO_B: Fq2 = Fq2 {
        c0: Fq([
            0x22ea_0000_0cf8_9db2,
            0x6ec8_32df_7138_0aa4,
            0x6e1b_9440_3db5_a66e,
            0x75bf_3c53_a794_73ba,
            0x3dd3_a569_412c_0a34,
            0x125c_db5e_74dc_4fd1,
        ]),
        c1: Fq([
            0x22ea_0000_0cf8_9db2,
            0x6ec8_32df_7138_0aa4,
            0x6e1b_9440_3db5_a66e,
            0x75bf_3c53_a794_73ba,
            0x3dd3_a569_412c_0a34,
            0x125c_db5e_74dc_4fd1,
        ]),
    };
    let iso_map = crate::hash_to_curve::Iso {
        a: ISO_A,
        b: ISO_B,
        map: Box::new(iso_map),
    };

    crate::hash_to_curve::Suite::new(domain, SSWU_Z, crate::hash_to_curve::Method::SSWU(iso_map))
}

/// Maps an iso-G1 point to a G1 point.
fn iso_map(x: Fq2, y: Fq2, z: Fq2) -> G2 {
    const COEFFS: [&[Fq2]; 4] = [&ISO3_XNUM, &ISO3_XDEN, &ISO3_YNUM, &ISO3_YDEN];

    // xnum, xden, ynum, yden
    let mut mapvals = [Fq2::ZERO; 4];

    // compute powers of z
    let zsq = z.square();
    let zpows = [z, zsq, zsq * z];

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

    G2 {
        x: mapvals[0] * mapvals[3], // xnum * yden,
        y: mapvals[2] * mapvals[1], // ynum * xden,
        z: mapvals[1] * mapvals[3], // xden * yden
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn hash_to_curve<'a>(
    domain_prefix: &'a str,
    suite: crate::hash_to_curve::Suite<G2, sha2::Sha256, 128>,
) -> Box<dyn Fn(&[u8]) -> G2 + 'a> {
    Box::new(move |message| suite.hash_to_curve(domain_prefix, message).clear_cofactor())
}

#[cfg(test)]
mod test {
    use group::UncompressedEncoding;
    use rand_core::OsRng;

    use super::*;
    use crate::arithmetic::CurveEndo;
    #[cfg(feature = "std")]
    use crate::serde::SerdeObject;

    crate::curve_testing_suite!(G2);
    crate::curve_testing_suite!(G2, "endo_consistency");
    crate::curve_testing_suite!(G2, "endo");

    #[test]
    fn test_cofactor() {
        assert!(bool::from(
            G2Affine::identity().to_curve().is_torsion_free()
        ));
        assert!(bool::from(
            G2Affine::generator().to_curve().is_torsion_free()
        ));
    }

    #[test]
    fn test_hash_to_curve() {
        pub(crate) fn point_from_hex(x0: &str, x1: &str, y0: &str, y1: &str) -> G2Affine {
            let x0: Fq = crate::tests::hex_to_field(x0);
            let x1: Fq = crate::tests::hex_to_field(x1);
            let x = Fq2 { c0: x0, c1: x1 };
            let y0: Fq = crate::tests::hex_to_field(y0);
            let y1: Fq = crate::tests::hex_to_field(y1);
            let y = Fq2 { c0: y0, c1: y1 };
            G2Affine::from_xy(x, y).unwrap()
        }

        struct Test {
            msg: &'static [u8],
            expect: G2Affine,
        }

        impl Test {
            fn new(msg: &'static [u8], expect: G2Affine) -> Self {
                Self { msg, expect }
            }

            fn run(&self, domain_prefix: &str) {
                let r0 = G2::hash_to_curve(domain_prefix)(self.msg);
                assert_eq!(r0.to_affine(), self.expect);
            }
        }

        let tests = [
        Test::new(
            b"",
            point_from_hex(
                "0141ebfbdca40eb85b87142e130ab689c673cf60f1a3e98d69335266f30d9b8d4ac44c1038e9dcdd5393faf5c41fb78a",
                "05cb8437535e20ecffaef7752baddf98034139c38452458baeefab379ba13dff5bf5dd71b72418717047f5b0f37da03d",
                "0503921d7f6a12805e72940b963c0cf3471c7b2a524950ca195d11062ee75ec076daf2d4bc358c4b190c0c98064fdd92",
                "12424ac32561493f3fe3c260708a12b7c620e7be00099a974e259ddc7d1f6395c3c811cdd19f1e8dbf3e9ecfdcbab8d6",
            ),
        ),
        Test::new(
            b"abc",
            point_from_hex(
                "02c2d18e033b960562aae3cab37a27ce00d80ccd5ba4b7fe0e7a210245129dbec7780ccc7954725f4168aff2787776e6",
                "139cddbccdc5e91b9623efd38c49f81a6f83f175e80b06fc374de9eb4b41dfe4ca3a230ed250fbe3a2acf73a41177fd8",
                "1787327b68159716a37440985269cf584bcb1e621d3a7202be6ea05c4cfe244aeb197642555a0645fb87bf7466b2ba48",
                "00aa65dae3c8d732d10ecd2c50f8a1baf3001578f71c694e03866e9f3d49ac1e1ce70dd94a733534f106d4cec0eddd16",
            ),
        ),
        Test::new(
            b"abcdef0123456789",
            point_from_hex(
                "121982811d2491fde9ba7ed31ef9ca474f0e1501297f68c298e9f4c0028add35aea8bb83d53c08cfc007c1e005723cd0",
                "190d119345b94fbd15497bcba94ecf7db2cbfd1e1fe7da034d26cbba169fb3968288b3fafb265f9ebd380512a71c3f2c",
                "05571a0f8d3c08d094576981f4a3b8eda0a8e771fcdcc8ecceaf1356a6acf17574518acb506e435b639353c2e14827c8",
                "0bb5e7572275c567462d91807de765611490205a941a5a6af3b1691bfe596c31225d3aabdf15faff860cb4ef17c7c3be",
            ),
        ),
        Test::new(
            b"q128_qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
            point_from_hex(
                "19a84dd7248a1066f737cc34502ee5555bd3c19f2ecdb3c7d9e24dc65d4e25e50d83f0f77105e955d78f4762d33c17da",
                "0934aba516a52d8ae479939a91998299c76d39cc0c035cd18813bec433f587e2d7a4fef038260eef0cef4d02aae3eb91",
                "14f81cd421617428bc3b9fe25afbb751d934a00493524bc4e065635b0555084dd54679df1536101b2c979c0152d09192",
                "09bcccfa036b4847c9950780733633f13619994394c23ff0b32fa6b795844f4a0673e20282d07bc69641cee04f5e5662",
            ),
        ),
        Test::new(
            b"a512_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            point_from_hex(
                "01a6ba2f9a11fa5598b2d8ace0fbe0a0eacb65deceb476fbbcb64fd24557c2f4b18ecfc5663e54ae16a84f5ab7f62534",
                "11fca2ff525572795a801eed17eb12785887c7b63fb77a42be46ce4a34131d71f7a73e95fee3f812aea3de78b4d01569",
                "0b6798718c8aed24bc19cb27f866f1c9effcdbf92397ad6448b5c9db90d2b9da6cbabf48adc1adf59a1a28344e79d57e",
                "03a47f8e6d1763ba0cad63d6114c0accbef65707825a511b251a660a9b3994249ae4e63fac38b23da0c398689ee2ab52",
            ),
        ),
    ];

        tests.iter().for_each(|test| {
            test.run("QUUX-V01-CS02-with-");
        });
    }
}

/// Coefficients of the 3-isogeny x map's numerator
const ISO3_XNUM: [Fq2; 4] = [
    Fq2 {
        c0: Fq([
            0x47f6_71c7_1ce0_5e62,
            0x06dd_5707_1206_393e,
            0x7c80_cd2a_f3fd_71a2,
            0x0481_03ea_9e6c_d062,
            0xc545_16ac_c8d0_37f6,
            0x1380_8f55_0920_ea41,
        ]),
        c1: Fq([
            0x47f6_71c7_1ce0_5e62,
            0x06dd_5707_1206_393e,
            0x7c80_cd2a_f3fd_71a2,
            0x0481_03ea_9e6c_d062,
            0xc545_16ac_c8d0_37f6,
            0x1380_8f55_0920_ea41,
        ]),
    },
    Fq2 {
        c0: Fq::zero(),
        c1: Fq([
            0x5fe5_5555_554c_71d0,
            0x873f_ffdd_236a_aaa3,
            0x6a6b_4619_b26e_f918,
            0x21c2_8884_0887_4945,
            0x2836_cda7_028c_abc5,
            0x0ac7_3310_a7fd_5abd,
        ]),
    },
    Fq2 {
        c0: Fq([
            0x0a0c_5555_5559_71c3,
            0xdb0c_0010_1f9e_aaae,
            0xb1fb_2f94_1d79_7997,
            0xd396_0742_ef41_6e1c,
            0xb700_40e2_c205_56f4,
            0x149d_7861_e581_393b,
        ]),
        c1: Fq([
            0xaff2_aaaa_aaa6_38e8,
            0x439f_ffee_91b5_5551,
            0xb535_a30c_d937_7c8c,
            0x90e1_4442_0443_a4a2,
            0x941b_66d3_8146_55e2,
            0x0563_9988_53fe_ad5e,
        ]),
    },
    Fq2 {
        c0: Fq([
            0x40aa_c71c_71c7_25ed,
            0x1909_5555_7a84_e38e,
            0xd817_050a_8f41_abc3,
            0xd864_85d4_c87f_6fb1,
            0x696e_b479_f885_d059,
            0x198e_1a74_3280_02d2,
        ]),
        c1: Fq::zero(),
    },
];

/// Coefficients of the 3-isogeny x map's denominator
const ISO3_XDEN: [Fq2; 3] = [
    Fq2 {
        c0: Fq::zero(),
        c1: Fq([
            0x1f3a_ffff_ff13_ab97,
            0xf25b_fc61_1da3_ff3e,
            0xca37_57cb_3819_b208,
            0x3e64_2736_6f8c_ec18,
            0x0397_7bc8_6095_b089,
            0x04f6_9db1_3f39_a952,
        ]),
    },
    Fq2 {
        c0: Fq([
            0x4476_0000_0027_552e,
            0xdcb8_009a_4348_0020,
            0x6f7e_e9ce_4a6e_8b59,
            0xb103_30b7_c0a9_5bc6,
            0x6140_b1fc_fb1e_54b7,
            0x0381_be09_7f0b_b4e1,
        ]),
        c1: Fq([
            0x7588_ffff_ffd8_557d,
            0x41f3_ff64_6e0b_ffdf,
            0xf7b1_e8d2_ac42_6aca,
            0xb374_1acd_32db_b6f8,
            0xe9da_f5b9_482d_581f,
            0x167f_53e0_ba74_31b8,
        ]),
    },
    Fq2::one(),
];

/// Coefficients of the 3-isogeny y map's numerator
const ISO3_YNUM: [Fq2; 4] = [
    Fq2 {
        c0: Fq([
            0x96d8_f684_bdfc_77be,
            0xb530_e4f4_3b66_d0e2,
            0x184a_88ff_3796_52fd,
            0x57cb_23ec_fae8_04e1,
            0x0fd2_e39e_ada3_eba9,
            0x08c8_055e_31c5_d5c3,
        ]),
        c1: Fq([
            0x96d8_f684_bdfc_77be,
            0xb530_e4f4_3b66_d0e2,
            0x184a_88ff_3796_52fd,
            0x57cb_23ec_fae8_04e1,
            0x0fd2_e39e_ada3_eba9,
            0x08c8_055e_31c5_d5c3,
        ]),
    },
    Fq2 {
        c0: Fq::zero(),
        c1: Fq([
            0xbf0a_71c7_1c91_b406,
            0x4d6d_55d2_8b76_38fd,
            0x9d82_f98e_5f20_5aee,
            0xa27a_a27b_1d1a_18d5,
            0x02c3_b2b2_d293_8e86,
            0x0c7d_1342_0b09_807f,
        ]),
    },
    Fq2 {
        c0: Fq([
            0xd7f9_5555_5553_1c74,
            0x21cf_fff7_48da_aaa8,
            0x5a9a_d186_6c9b_be46,
            0x4870_a221_0221_d251,
            0x4a0d_b369_c0a3_2af1,
            0x02b1_ccc4_29ff_56af,
        ]),
        c1: Fq([
            0xe205_aaaa_aaac_8e37,
            0xfcdc_0007_6879_5556,
            0x0c96_011a_8a15_37dd,
            0x1c06_a963_f163_406e,
            0x010d_f44c_82a8_81e6,
            0x174f_4526_0f80_8feb,
        ]),
    },
    Fq2 {
        c0: Fq([
            0xa470_bda1_2f67_f35c,
            0xc0fe_38e2_3327_b425,
            0xc9d3_d0f2_c6f0_678d,
            0x1c55_c993_5b5a_982e,
            0x27f6_c0e2_f074_6764,
            0x117c_5e6e_28aa_9054,
        ]),
        c1: Fq::zero(),
    },
];

/// Coefficients of the 3-isogeny y map's denominator
const ISO3_YDEN: [Fq2; 4] = [
    Fq2 {
        c0: Fq([
            0x0162_ffff_fa76_5adf,
            0x8f7b_ea48_0083_fb75,
            0x561b_3c22_59e9_3611,
            0x11e1_9fc1_a9c8_75d5,
            0xca71_3efc_0036_7660,
            0x03c6_a03d_41da_1151,
        ]),
        c1: Fq([
            0x0162_ffff_fa76_5adf,
            0x8f7b_ea48_0083_fb75,
            0x561b_3c22_59e9_3611,
            0x11e1_9fc1_a9c8_75d5,
            0xca71_3efc_0036_7660,
            0x03c6_a03d_41da_1151,
        ]),
    },
    Fq2 {
        c0: Fq::zero(),
        c1: Fq([
            0x5db0_ffff_fd3b_02c5,
            0xd713_f523_58eb_fdba,
            0x5ea6_0761_a84d_161a,
            0xbb2c_75a3_4ea6_c44a,
            0x0ac6_7359_21c1_119b,
            0x0ee3_d913_bdac_fbf6,
        ]),
    },
    Fq2 {
        c0: Fq([
            0x66b1_0000_003a_ffc5,
            0xcb14_00e7_64ec_0030,
            0xa73e_5eb5_6fa5_d106,
            0x8984_c913_a0fe_09a9,
            0x11e1_0afb_78ad_7f13,
            0x0542_9d0e_3e91_8f52,
        ]),
        c1: Fq([
            0x534d_ffff_ffc4_aae6,
            0x5397_ff17_4c67_ffcf,
            0xbff2_73eb_870b_251d,
            0xdaf2_8271_5287_0915,
            0x393a_9cba_ca9e_2dc3,
            0x14be_74db_faee_5748,
        ]),
    },
    Fq2::one(),
];
