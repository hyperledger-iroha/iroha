//! Poseidon2 permutation over the Goldilocks field (p = 2^64 - 2^32 + 1).
//!
//! The constants are generated from the reference Grain LFSR as implemented in
//! `poseidon-primitives` (commit `0.2.0`, parameters pinned to
//! `ark-poseidon2` commit `3f2b7fe`). Generation script lives under
//! `target-codex/poseidon_gen` and mirrors the canonical Poseidon2 parameter
//! tables for width 3, rate 2.

use core::convert::TryFrom;

const MODULUS: u64 = 0xffff_ffff_0000_0001;
/// Goldilocks field modulus (2^64 - 2^32 + 1).
pub const FIELD_MODULUS: u64 = MODULUS;
const MODULUS_U128: u128 = MODULUS as u128;
/// Poseidon state width (t = 3).
pub const STATE_WIDTH: usize = 3;
/// Poseidon rate (r = 2).
pub const RATE: usize = 2;
const FULL_ROUNDS_HALF: usize = 4;
const PARTIAL_ROUNDS: usize = 57;

/// Round constants for the canonical Poseidon2 permutation.
pub const ROUND_CONSTANTS: [[u64; STATE_WIDTH]; FULL_ROUNDS_HALF * 2 + PARTIAL_ROUNDS] = [
    [
        0x0355_defb_099b_ed96,
        0xfcc5_5fb9_6813_55e6,
        0x9313_68d7_25f8_720a,
    ],
    [
        0x3b87_52f8_1e4e_100a,
        0x53b8_64ee_53de_1e7c,
        0xfccb_110a_3a83_9800,
    ],
    [
        0xfff0_e990_12e3_2773,
        0xfca2_8ab2_898e_ff49,
        0xb02b_1c85_4e99_b411,
    ],
    [
        0x9c45_a0b2_aded_dcdd,
        0xab6b_d8d6_b495_9cfa,
        0x32eb_07eb_92be_cca5,
    ],
    [
        0x0269_8cda_7aa8_674a,
        0xa691_4454_ffbb_c85a,
        0x3304_02af_7562_de69,
    ],
    [
        0x6b9d_eb68_9731_670a,
        0xb6ac_ba18_2405_2a6e,
        0x3ad1_81a6_aaed_99ec,
    ],
    [
        0x871f_2534_5b28_b8e9,
        0xcd88_d358_315e_eec1,
        0xe051_da57_7c86_be1d,
    ],
    [
        0x153a_d181_7c0c_125a,
        0x9746_1b44_2e8a_be8c,
        0x8659_1af5_1639_f5a9,
    ],
    [
        0xb5bb_c2a4_45ce_1d3c,
        0x3cf8_7df3_7950_3557,
        0x69c8_7ef4_b926_f914,
    ],
    [
        0x941b_ba1e_4ef7_e9f5,
        0x2f9b_29b5_8901_fe31,
        0x49c9_5be0_66d4_e00b,
    ],
    [
        0xcdb4_fa57_1ed1_6b7e,
        0x8152_6ee1_686c_1130,
        0x9afe_7622_fd18_7b9d,
    ],
    [
        0x1c6d_3d32_f566_6a4a,
        0xbc71_4a50_9ed0_c9e2,
        0xdbd4_4f43_3d9b_b989,
    ],
    [
        0x07f9_a90b_7e43_d95c,
        0xbf08_30bc_8e11_be74,
        0x97f8_2d76_58d4_24af,
    ],
    [
        0x232d_735b_d912_e717,
        0x24c5_847c_a749_1e7a,
        0x050e_45f9_ad01_8df7,
    ],
    [
        0x3d87_5e31_3e44_5623,
        0xf0e7_65b8_e635_0e3a,
        0x24e1_f73a_acb2_a5d8,
    ],
    [
        0xa260_8cac_4edd_cbf6,
        0xbb84_373a_1401_2579,
        0x153f_f026_357d_db2e,
    ],
    [
        0x31c5_1990_051b_2fce,
        0xc432_6a6d_f134_2061,
        0x110a_b731_271e_081d,
    ],
    [
        0xd7d4_ed98_3f73_8f7d,
        0x04c2_243a_9d2d_2dd8,
        0xec84_bd9f_09f4_a4ee,
    ],
    [
        0x459c_6a91_e831_0f7d,
        0xbd54_48db_7b31_2560,
        0x5540_5bc1_d484_1791,
    ],
    [
        0x9b29_aa29_d32f_f2ef,
        0x19b4_882d_7d91_817d,
        0xff96_eeff_0d35_d395,
    ],
    [
        0x725b_ee8d_2e75_2eba,
        0xb778_a21c_6431_0114,
        0x9991_cf61_41e2_878a,
    ],
    [
        0x5baf_8b23_f6d3_0359,
        0x17d0_b954_a0c2_4e6d,
        0x3050_c407_8053_b0d5,
    ],
    [
        0xf9f5_2a30_90ef_a474,
        0xa598_d529_15f9_9806,
        0xbf48_572b_618f_bd61,
    ],
    [
        0xa34b_fa5c_0b92_ec3e,
        0xab11_1573_6057_c63c,
        0x2f7d_4168_4601_4aaa,
    ],
    [
        0xe19f_2142_92d3_a546,
        0xb48c_ed43_6104_2203,
        0xb64f_6bfc_1cfc_bd92,
    ],
    [
        0xe0f4_8eec_d39b_7cbf,
        0xc438_4583_c0d6_450d,
        0x7460_cad3_3f43_6aca,
    ],
    [
        0xa69f_9eff_5caf_7223,
        0x82ac_d34e_811e_340a,
        0xecb3_d78f_9881_bedf,
    ],
    [
        0x9767_60db_1c85_079a,
        0xa0d5_2712_8f52_31ca,
        0xa882_3b80_503c_432b,
    ],
    [
        0xc41a_fca1_9eb7_8258,
        0x33b3_0793_338e_26ee,
        0x3369_d80a_26f2_6384,
    ],
    [
        0x4f52_37b4_3fe7_c081,
        0x3e2e_28d3_7379_26d1,
        0x9e9f_d944_9eaf_c854,
    ],
    [
        0xf6db_f8e5_0187_050f,
        0x0883_a737_2f01_a7d2,
        0xe85c_8cbe_6053_fa56,
    ],
    [
        0x43e6_6dea_535d_ce9d,
        0xff74_45db_745c_243b,
        0xde5d_71d4_d770_9458,
    ],
    [
        0xb14f_0f45_451d_99f9,
        0xfcdf_81f9_7120_6fc3,
        0x2508_429e_d0bb_abfa,
    ],
    [
        0x4081_15a6_f42a_57e8,
        0xfcde_3356_9c03_693d,
        0x8392_6cc6_0624_1abd,
    ],
    [
        0xfe61_9141_9d8e_0a84,
        0x358d_7136_2d23_4258,
        0x3bfb_29c6_4195_c104,
    ],
    [
        0x0ed6_b527_fe03_4ad7,
        0xcd19_98fe_841a_698a,
        0x6ab8_90eb_bfe7_72a9,
    ],
    [
        0x099a_748e_d541_5721,
        0x254b_906a_5a91_bc8a,
        0x4d51_30a3_0f0c_bb72,
    ],
    [
        0xd093_2ea3_fa64_d467,
        0xbe72_249d_94ae_a218,
        0x0621_5750_fd6a_29c5,
    ],
    [
        0xd790_3148_4db8_9846,
        0xdd89_cad9_59ba_24a4,
        0xdc04_6dfa_4487_1254,
    ],
    [
        0xb36f_522b_f4af_2ef6,
        0x9279_6eae_3863_2589,
        0x9796_6381_78a0_1e39,
    ],
    [
        0xbe2c_6a4f_4312_1eff,
        0x0c20_942e_a145_116b,
        0xb6d2_1c3a_f9d7_af98,
    ],
    [
        0x953a_f8c7_9c3e_074e,
        0xc039_1530_65b4_a5c5,
        0x9f51_afaf_415b_8a55,
    ],
    [
        0x6198_a829_2756_9707,
        0xa093_66e5_bb6a_66c7,
        0x68e7_10be_9451_ff59,
    ],
    [
        0x9226_cca1_e498_8ddd,
        0x27e1_5510_153c_0224,
        0x8d6a_3e1f_c7c1_97f2,
    ],
    [
        0xfaa9_29e2_5257_bc60,
        0x6b37_c307_ac47_aa83,
        0x907b_20a3_5cfe_4fc2,
    ],
    [
        0x5035_0676_383d_204e,
        0x446c_05dd_253b_59e8,
        0x0f31_889e_23c5_d5ea,
    ],
    [
        0x38ec_3e1c_2e66_7720,
        0x026f_df27_b554_ff10,
        0x1ca8_2d95_c6fa_fd39,
    ],
    [
        0xddd0_bc05_6342_e191,
        0x2d31_5fb4_4b04_ac30,
        0xfd4b_193b_ae11_b94b,
    ],
    [
        0x086d_f800_b348_9093,
        0xf30f_ae76_04b9_87e2,
        0x10a1_9793_fe69_ebe8,
    ],
    [
        0x0c77_141b_7edf_0449,
        0x5ea8_7577_9328_deb1,
        0x0b67_4c79_767e_421f,
    ],
    [
        0x4a5a_726d_82f3_258e,
        0x09e8_c7a8_be47_b1de,
        0xfffa_2ac0_8527_6013,
    ],
    [
        0xc091_3932_4aa9_d4f9,
        0xb51a_2320_5924_fd17,
        0xdcc8_8949_49e0_2da4,
    ],
    [
        0x2274_8352_9222_ad6e,
        0x6533_7b21_01df_c3d8,
        0xe19d_fd7c_78aa_acc7,
    ],
    [
        0xf1b9_8df3_40a1_6ca6,
        0x18d5_4312_3d27_950d,
        0xc0c1_6b9b_6007_e396,
    ],
    [
        0xb90b_c3c0_d060_5c8c,
        0xde10_8ac1_d6c0_bdca,
        0x5ee8_196b_6114_6783,
    ],
    [
        0x2aab_a85c_6bbc_f12f,
        0x02ff_605b_7384_3154,
        0x2c37_fc5e_4dfe_bb1e,
    ],
    [
        0xcb1c_a352_a46e_4d33,
        0x187d_0f76_fc0f_b4a3,
        0xaead_cdfe_0a66_b1f1,
    ],
    [
        0xc125_32bf_b67e_3436,
        0x4b40_3b0b_f220_033e,
        0xe0de_c101_5d69_cd0b,
    ],
    [
        0x5d3e_b71f_9ef3_0c4a,
        0xf82d_9fbc_df53_2aeb,
        0xa362_551d_86be_bd87,
    ],
    [
        0x6823_e258_0285_1126,
        0x189c_e62b_7765_0805,
        0xefc2_61ff_a36b_f041,
    ],
    [
        0x4788_6351_1cba_f173,
        0x4f4b_d042_c56b_7936,
        0x5b4c_f8cc_8584_ca9a,
    ],
    [
        0xcc94_4e5f_7606_3e0c,
        0xd29a_0b00_2c78_3ca7,
        0x2f59_efce_5cde_182b,
    ],
    [
        0x93a0_767d_9186_685c,
        0xee25_01a7_61ec_f4e5,
        0xe751_4fa4_8b14_5686,
    ],
    [
        0x5e0f_189e_8c61_d97c,
        0xebe9_bd9b_ef0f_5443,
        0xd6cf_e2a7_6189_672a,
    ],
    [
        0x38ff_8f40_252c_1ab7,
        0x3cf9_c4c3_804d_8166,
        0x512f_1e3a_c8d0_ffe5,
    ],
];

/// MDS matrix for the canonical Poseidon2 permutation.
pub const MDS: [[u64; STATE_WIDTH]; STATE_WIDTH] = [
    [
        0x9825_13a2_3d22_b592,
        0xa311_5db8_cf1d_9c90,
        0x46ba_684b_9eee_84b7,
    ],
    [
        0xbe3d_ce25_491d_b768,
        0xfb0a_6f73_1943_519f,
        0xfce5_bd95_3cde_1896,
    ],
    [
        0xe624_719c_41eb_1a09,
        0xd222_1b0f_1aa2_ebc4,
        0x1ab5_e60d_03ad_44bc,
    ],
];

#[inline]
fn add(a: u64, b: u64) -> u64 {
    let sum = a.wrapping_add(b);
    let mut result = sum;
    if sum < a {
        result = result.wrapping_sub(MODULUS);
    }
    if result >= MODULUS {
        result - MODULUS
    } else {
        result
    }
}

#[inline]
fn reduce_wide(wide_lo: u64, wide_hi: u64) -> u64 {
    let hi_lo = i128::from(wide_hi & 0xffff_ffff);
    let hi_hi = i128::from(wide_hi >> 32);
    let mut acc = i128::from(wide_lo);
    acc += hi_lo << 32;
    acc -= hi_lo;
    acc -= hi_hi;

    let modulus = i128::from(MODULUS);
    if acc < 0 {
        acc += modulus;
        if acc < 0 {
            acc += modulus;
        }
    }
    if acc >= modulus {
        acc -= modulus;
        if acc >= modulus {
            acc -= modulus;
        }
    }
    u64::try_from(acc).expect("Goldilocks reduction must stay within field bounds")
}

#[inline]
fn mul(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let lo = u64::try_from(product & u128::from(u64::MAX))
        .expect("low 64 bits of the product must fit into u64");
    let hi = u64::try_from(product >> 64).expect("high 64 bits of the product must fit into u64");
    let reduced = reduce_wide(lo, hi);
    debug_assert_eq!(
        reduced,
        (product % MODULUS_U128) as u64,
        "Goldilocks multiplication reduction diverged for a={a:#x}, b={b:#x}"
    );
    reduced
}

#[inline]
fn pow5(x: u64) -> u64 {
    let x2 = mul(x, x);
    let x4 = mul(x2, x2);
    mul(x4, x)
}

fn apply_mds(state: &mut [u64; STATE_WIDTH]) {
    let mut new_state = [0u64; STATE_WIDTH];
    for (out_word, row) in new_state.iter_mut().zip(MDS.iter()) {
        let mut acc = 0u64;
        for (&coef, &value) in row.iter().zip(state.iter()) {
            acc = add(acc, mul(coef, value));
        }
        *out_word = acc;
    }
    *state = new_state;
}

fn full_round(state: &mut [u64; STATE_WIDTH], rc: &[u64; STATE_WIDTH]) {
    for (word, constant) in state.iter_mut().zip(rc.iter()) {
        *word = pow5(add(*word, *constant));
    }
    apply_mds(state);
}

fn partial_round(state: &mut [u64; STATE_WIDTH], rc: &[u64; STATE_WIDTH]) {
    for (word, constant) in state.iter_mut().zip(rc.iter()) {
        *word = add(*word, *constant);
    }
    state[0] = pow5(state[0]);
    apply_mds(state);
}

fn permute(state: &mut [u64; STATE_WIDTH]) {
    let mut round = 0;
    for _ in 0..FULL_ROUNDS_HALF {
        full_round(state, &ROUND_CONSTANTS[round]);
        round += 1;
    }
    for _ in 0..PARTIAL_ROUNDS {
        partial_round(state, &ROUND_CONSTANTS[round]);
        round += 1;
    }
    for _ in 0..FULL_ROUNDS_HALF {
        full_round(state, &ROUND_CONSTANTS[round]);
        round += 1;
    }
}

/// Compute a Poseidon2 hash over the provided Goldilocks field elements.
///
/// The sponge uses rate 2, capacity 1, and absorbs the message using classical
/// +1 padding (a single `1` element appended after the payload).
pub fn hash_field_elements(elements: &[u64]) -> u64 {
    let mut state = [0u64; STATE_WIDTH];
    let mut chunks = elements.chunks_exact(RATE);
    for chunk in &mut chunks {
        for (idx, &value) in chunk.iter().enumerate() {
            state[idx] = add(state[idx], value);
        }
        permute(&mut state);
    }

    let remainder = chunks.remainder();
    let mut block = [0u64; RATE];
    block[..remainder.len()].copy_from_slice(remainder);
    block[remainder.len()] = 1;
    for (idx, &value) in block.iter().enumerate() {
        state[idx] = add(state[idx], value);
    }
    permute(&mut state);

    state[0]
}

/// Apply the canonical Poseidon permutation to the supplied state.
pub fn permute_state(state: &mut [u64; STATE_WIDTH]) {
    permute(state);
}

/// Poseidon sponge used to derive deterministic field elements.
#[derive(Debug, Clone, Copy)]
pub struct PoseidonSponge {
    state: [u64; STATE_WIDTH],
    rate_index: usize,
    finalised: bool,
}

impl PoseidonSponge {
    #[must_use]
    /// Create a new sponge in the zero state.
    pub fn new() -> Self {
        Self {
            state: [0u64; STATE_WIDTH],
            rate_index: 0,
            finalised: false,
        }
    }

    /// Reset the sponge back to the initial zeroed state.
    pub fn reset(&mut self) {
        self.state = [0u64; STATE_WIDTH];
        self.rate_index = 0;
        self.finalised = false;
    }

    /// Absorb a single field element into the sponge.
    pub fn absorb(&mut self, element: u64) {
        debug_assert!(
            !self.finalised,
            "cannot absorb into a finalised sponge; start a new instance"
        );
        debug_assert!(
            self.rate_index < RATE,
            "rate index must stay within sponge capacity"
        );
        self.state[self.rate_index] = add(self.state[self.rate_index], element);
        self.rate_index += 1;
        if self.rate_index == RATE {
            permute(&mut self.state);
            self.rate_index = 0;
        }
    }

    /// Absorb a slice of field elements into the sponge.
    pub fn absorb_slice(&mut self, elements: &[u64]) {
        for &element in elements {
            self.absorb(element);
        }
    }

    fn ensure_finalised(&mut self) {
        if self.finalised {
            return;
        }
        self.absorb(1);
        while self.rate_index != 0 {
            self.absorb(0);
        }
        self.finalised = true;
    }

    /// Squeeze a single field element while keeping the sponge ready for the next output.
    #[must_use]
    pub fn squeeze_element(&mut self) -> u64 {
        self.ensure_finalised();
        let element = self.state[0];
        permute(&mut self.state);
        element
    }

    #[must_use]
    /// Finalise the sponge and return the first output element.
    pub fn squeeze(mut self) -> u64 {
        self.ensure_finalised();
        self.state[0]
    }
}

impl Default for PoseidonSponge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poseidon_hash_known_vector() {
        let digest = hash_field_elements(&[1, 2, 3]);
        assert_eq!(digest, 0x42ea_af13_b5f9_03b1);
    }

    #[test]
    fn squeeze_multiple_elements() {
        let mut sponge = PoseidonSponge::new();
        sponge.absorb_slice(&[1, 2, 3]);
        let first = sponge.squeeze_element();
        let second = sponge.squeeze_element();
        assert_ne!(first, second);
    }

    #[test]
    fn field_addition_matches_reference() {
        let cases = [
            (0u64, 0u64),
            (1, FIELD_MODULUS - 1),
            (FIELD_MODULUS - 1, FIELD_MODULUS - 1),
            (FIELD_MODULUS - 2, 3),
            (0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210),
        ];
        for (a, b) in cases {
            let expected = ((u128::from(a) + u128::from(b)) % MODULUS_U128) as u64;
            assert_eq!(add(a, b), expected, "addition diverged for {a:#x} + {b:#x}");
        }
    }

    #[test]
    fn field_multiplication_matches_reference() {
        let cases = [
            (0u64, 0u64),
            (1, FIELD_MODULUS - 1),
            (FIELD_MODULUS - 1, FIELD_MODULUS - 1),
            (123_456_789, 987_654_321),
            (0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210),
        ];
        for (a, b) in cases {
            let expected = ((u128::from(a) * u128::from(b)) % MODULUS_U128) as u64;
            assert_eq!(
                mul(a, b),
                expected,
                "multiplication diverged for {a:#x} * {b:#x}"
            );
        }
    }
}
