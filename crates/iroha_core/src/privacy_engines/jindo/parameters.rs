//! Closed, integer-only first-release Jindo parameter profile.
//!
//! Parameter selection follows the public 128-bit MSIS/MLWE heuristic described
//! in ePrint 2026/044 for a 256-coefficient polynomial and a worst-case batch of
//! four.  Floating-point parameter search is intentionally absent from
//! consensus code: its selected output is pinned here and covered by the engine
//! manifest and known-answer tests.

use iroha_data_model::privacy::IROHA_JINDO_OUTER_COMMITMENT_RANK_V1;

use super::{JINDO_ENCODING_EXPONENT_V1, JINDO_MAX_BATCH_SIZE_V1};

/// Closed discrete-Gaussian widths used by the native Jindo prover.
///
/// Every width is the exact unsigned Q64 integer selected for this profile.
/// The sampler accepts this enum instead of an open numeric parameter so a
/// caller cannot silently instantiate an unreviewed distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JindoGaussianWidthV1 {
    /// Ordinary randomized coefficient encoding.
    Ecd,
    /// Evaluation-sensitive first coefficient row.
    EcdBlind,
    /// Ordinary randomized mask-row encoding.
    Mask,
    /// Evaluation-sensitive first mask row.
    MaskBlind,
    /// Ordinary MLWE hiding polynomial.
    Mlwe,
    /// Mask-column MLWE hiding polynomial.
    MaskMlwe,
}

impl JindoGaussianWidthV1 {
    /// Every width in canonical profile order.
    pub(crate) const ALL: [Self; 6] = [
        Self::Ecd,
        Self::EcdBlind,
        Self::Mask,
        Self::MaskBlind,
        Self::Mlwe,
        Self::MaskMlwe,
    ];

    /// Exact standard deviation multiplied by `2^64`.
    pub(crate) const fn sigma_q64(self) -> u128 {
        match self {
            Self::Ecd => 88_313_164_199_611_793_408,
            Self::EcdBlind => 21_291_244_130_556_007_189_315_584,
            Self::Mask => 31_972_780_684_076_312_952_832,
            Self::MaskBlind => 7_708_253_749_562_589_273_347_063_808,
            Self::Mlwe => 124_889_530_281_098_821_632,
            Self::MaskMlwe => 45_214_839_685_615_447_965_696,
        }
    }

    /// Integer proposal radius `ceil(14 * sigma)`.
    pub(crate) const fn tail_radius(self) -> u64 {
        let scaled = self.sigma_q64().saturating_mul(14);
        let integer = scaled >> 64;
        let has_fraction = scaled & ((1_u128 << 64) - 1) != 0;
        integer as u64 + has_fraction as u64
    }
}

/// Exact compiled Jindo parameter tuple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JindoParametersV1 {
    /// Largest Fiat--Shamir batched polynomial count.
    pub(crate) max_batch_size: usize,
    /// Matrix rows in the coefficient decomposition, including the randomized
    /// first and bridge rows.
    pub(crate) rows: usize,
    /// Matrix columns before the masking column.
    pub(crate) columns: usize,
    /// Inner MSIS matrix row count.
    pub(crate) inner_msis_rank: usize,
    /// Outer MSIS matrix row count.
    pub(crate) outer_msis_rank: usize,
    /// MLWE hiding rank.
    pub(crate) mlwe_rank: usize,
    /// Power-of-two inner commitment rounding divisor.
    pub(crate) log_inner_cutoff: u32,
    /// Power-of-two outer commitment rounding divisor.
    pub(crate) log_outer_cutoff: u32,
    /// Strict two-norm ceiling for the inner relation.
    pub(crate) response_two_norm_bound: u128,
    /// Strict two-norm ceiling for the outer decomposed commitment relation.
    pub(crate) decomposed_commitment_two_norm_bound: u128,
    /// Maximum absolute coefficient in a Fiat--Shamir challenge polynomial.
    pub(crate) challenge_coefficient_bound: u16,
}

/// The only native Jindo parameter profile compiled for the first release.
pub(crate) const JINDO_PARAMETERS_V1: JindoParametersV1 = JindoParametersV1 {
    max_batch_size: JINDO_MAX_BATCH_SIZE_V1,
    rows: JINDO_ENCODING_EXPONENT_V1 + 1,
    columns: 1,
    inner_msis_rank: 15,
    outer_msis_rank: IROHA_JINDO_OUTER_COMMITMENT_RANK_V1,
    mlwe_rank: 32,
    log_inner_cutoff: 40,
    log_outer_cutoff: 65,
    // Exact integers represented by the selected binary64 parameter output.
    response_two_norm_bound: 61_186_928_822_744_162_304,
    decomposed_commitment_two_norm_bound: 5_482_137_275_941_817_004_589_056,
    // The first mixed-radix coefficient reaches 91; the other fifteen reach
    // 90. Their maximum squared two-norm is 129_781 < 131_072, the bound used
    // by the published parameter search.
    challenge_coefficient_bound: 91,
};

/// Domain-separated, reviewable parameter manifest input.
///
/// For every named width, `tail_radius()` is at least fourteen times the
/// exact Q64 width.  The standard two-sided Gaussian tail estimate, with the
/// discrete correction for the minimum width (`Ecd`), is below `2^-140`.
/// The widest support has fewer than `2^34` integer candidates.  Comparing a
/// big-endian U256 draw directly with the Q256 weight therefore contributes
/// less than `2^(34-256) = 2^-222` even before normalization.  Center rounding
/// is at most `2^-129`; the range-reduced Q256 series and table accumulation
/// contribute less than `2^-240` per weight, or `2^-206` over the whole
/// support.  Thus the manifest's conservative total statistical-distance
/// ceiling is `2^-118`, dominated by its tail allowance.
pub const JINDO_PARAMETER_MANIFEST_V1: &[u8] = b"iroha-jindo-v0|paper=eprint-2026-044-v1-figures-1-5|coefficient-field=p=60272^16+1,le32|ring=Zq[X]/(X^256+1)|degree-bound=256|max-batch=4|rows=17|columns=1|inner-msis-rank=15|outer-msis-rank=13|mlwe-rank=32|inner-primes=9007199254740481,9007199254746113|outer-primes=140737488357377,140737488360961|inner-cutoff=2^40|outer-cutoff=2^65|response-two-norm-lt=61186928822744162304|decomposed-two-norm-lt=5482137275941817004589056|challenge=mixed-radix-120-bit-injective,c0=[-91,91],c1..15=[-90,90],two-norm-squared-lt=131072|gaussian-widths-q64=88313164199611793408,21291244130556007189315584,31972780684076312952832,7708253749562589273347063808,124889530281098821632,45214839685615447965696|gaussian-center=exact-signed-sum-b^i-over-p-to-q128-nearest|gaussian-sampler=uniform-exact-14sigma-interval-rejection+q256-range-reduced-integer-decay+one-u256be-threshold|max-statistical-distance-lt=2^-118|rng-health=distinct-256bit-blocks|crs=shake256-domain-separated|wire=IJP1-fixed-rns-le-v1|assurance=experimental-testnet-only";

#[cfg(test)]
mod tests {
    use super::super::{
        JINDO_ENCODING_EXPONENT_V1, JINDO_ENCODING_SLOTS_V1, JINDO_MAX_COEFFICIENTS_V1,
        JINDO_RING_DEGREE_V1,
        ring::{JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1},
    };
    use super::*;

    #[test]
    fn fixed_matrix_shape_covers_exactly_256_coefficients() {
        assert_eq!(JINDO_ENCODING_EXPONENT_V1, 16);
        assert_eq!(JINDO_ENCODING_SLOTS_V1, 16);
        assert_eq!(
            JINDO_PARAMETERS_V1.columns * (JINDO_PARAMETERS_V1.rows - 1) * JINDO_ENCODING_SLOTS_V1,
            JINDO_MAX_COEFFICIENTS_V1
        );
        assert_eq!(JINDO_RING_DEGREE_V1, 256);
    }

    #[test]
    fn fixed_rns_products_cover_selected_modulus_bits() {
        let inner = u128::from(JINDO_INNER_MODULI_V1[0].modulus())
            * u128::from(JINDO_INNER_MODULI_V1[1].modulus());
        let outer = u128::from(JINDO_OUTER_MODULI_V1[0].modulus())
            * u128::from(JINDO_OUTER_MODULI_V1[1].modulus());
        assert_eq!(inner, 81_129_638_414_648_204_884_353_358_500_353);
        assert_eq!(outer, 19_807_040_629_647_229_783_943_159_297);
        assert_eq!(128 - inner.leading_zeros(), 107);
        assert_eq!(128 - outer.leading_zeros(), 95);
    }

    #[test]
    fn all_profile_counts_and_bounds_are_nonzero_and_bounded() {
        let profile = JINDO_PARAMETERS_V1;
        assert_eq!(profile.max_batch_size, JINDO_MAX_BATCH_SIZE_V1);
        assert_eq!(
            profile.outer_msis_rank,
            IROHA_JINDO_OUTER_COMMITMENT_RANK_V1
        );
        assert_eq!(profile.inner_msis_rank * (profile.columns + 1), 30);
        assert!(profile.outer_msis_rank > 0);
        assert!(profile.mlwe_rank > profile.inner_msis_rank);
        assert!(profile.log_inner_cutoff < 107);
        assert!(profile.log_outer_cutoff < 95);
        assert!(profile.challenge_coefficient_bound > 0);
        assert!(profile.response_two_norm_bound > 0);
        assert!(profile.decomposed_commitment_two_norm_bound > 0);
        assert_eq!(profile.challenge_coefficient_bound, 91);
        assert!(JINDO_PARAMETER_MANIFEST_V1.starts_with(b"iroha-jindo-v0|"));
    }

    #[test]
    fn gaussian_widths_are_closed_and_tail_radii_are_exact() {
        let expected = [
            (88_313_164_199_611_793_408, 68),
            (21_291_244_130_556_007_189_315_584, 16_158_810),
            (31_972_780_684_076_312_952_832, 24_266),
            (7_708_253_749_562_589_273_347_063_808, 5_850_113_823),
            (124_889_530_281_098_821_632, 95),
            (45_214_839_685_615_447_965_696, 34_316),
        ];
        for (width, (sigma_q64, tail_radius)) in JindoGaussianWidthV1::ALL.into_iter().zip(expected)
        {
            assert_eq!(width.sigma_q64(), sigma_q64);
            assert_eq!(width.tail_radius(), tail_radius);
            assert!(
                u128::from(width.tail_radius()) << 64 >= width.sigma_q64() * 14,
                "the integer radius must cover fourteen exact Q64 widths"
            );
            assert!(
                2 * width.tail_radius() + 1 < (1_u64 << 34),
                "the complete proposal support must stay below 2^34"
            );
        }
    }
}
