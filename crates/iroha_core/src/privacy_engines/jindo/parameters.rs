//! Closed, integer-only first-release Jindo parameter profile.
//!
//! Parameter selection follows the public 128-bit MSIS/MLWE heuristic described
//! in ePrint 2026/044 for a 256-coefficient polynomial and a worst-case batch of
//! four.  Floating-point parameter search is intentionally absent from
//! consensus code: its selected output is pinned here and covered by the engine
//! manifest and known-answer tests.

use super::{
    JINDO_ENCODING_EXPONENT_V1, JINDO_ENCODING_SLOTS_V1, JINDO_MAX_COEFFICIENTS_V1,
    JINDO_RING_DEGREE_V1,
    ring::{JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1},
};

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
    /// Absolute coefficient bound of every Fiat--Shamir challenge polynomial.
    pub(crate) challenge_coefficient_bound: u16,
}

/// The only native Jindo parameter profile compiled for the first release.
pub(crate) const JINDO_PARAMETERS_V1: JindoParametersV1 = JindoParametersV1 {
    max_batch_size: 4,
    rows: 17,
    columns: 1,
    inner_msis_rank: 15,
    outer_msis_rank: 13,
    mlwe_rank: 32,
    log_inner_cutoff: 40,
    log_outer_cutoff: 65,
    // Conservative integer ceilings around the paper's selected bounds:
    // 6.118692882274416e19 and 5.482137275941817e24 respectively.
    response_two_norm_bound: 1_u128 << 66,
    decomposed_commitment_two_norm_bound: 1_u128 << 83,
    // floor(min(60272, 2^(120/16)) / 2) = floor(2^7.5 / 2).
    challenge_coefficient_bound: 90,
};

/// Domain-separated, reviewable parameter manifest input.
pub(crate) const JINDO_PARAMETER_MANIFEST_V1: &[u8] = b"iroha-jindo-v0|paper=eprint-2026-044-figures-1-5|coefficient-field=p=60272^16+1,le32|ring=Zq[X]/(X^256+1)|degree-bound=256|max-batch=4|rows=17|columns=1|inner-msis-rank=15|outer-msis-rank=13|mlwe-rank=32|inner-primes=9007199254740481,9007199254746113|outer-primes=140737488357377,140737488360961|inner-cutoff=2^40|outer-cutoff=2^65|response-two-norm-lt=2^66|decomposed-two-norm-lt=2^83|challenge-coefficients=[-90,90]|crs=shake256-domain-separated|wire=canonical-norito-strict-v1|assurance=experimental";

#[cfg(test)]
mod tests {
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
        assert!((1..=4).contains(&profile.max_batch_size));
        assert_eq!(profile.inner_msis_rank * (profile.columns + 1), 30);
        assert!(profile.outer_msis_rank > 0);
        assert!(profile.mlwe_rank > profile.inner_msis_rank);
        assert!(profile.log_inner_cutoff < 107);
        assert!(profile.log_outer_cutoff < 95);
        assert!(profile.challenge_coefficient_bound > 0);
        assert!(profile.response_two_norm_bound > 0);
        assert!(profile.decomposed_commitment_two_norm_bound > 0);
        assert!(JINDO_PARAMETER_MANIFEST_V1.starts_with(b"iroha-jindo-v0|"));
    }
}
