//! Closed consensus parameters for the first Bootle/Lantern profile.

/// Application ring degree.
pub const APPLICATION_RING_DEGREE_V1: usize = 64;
/// Application ring modulus.
pub const APPLICATION_MODULUS_V1: u16 = 12_289;
/// Number of application equations.
pub const APPLICATION_ROWS_V1: usize = 8;
/// Number of credential attributes.
pub const ATTRIBUTE_COUNT_V1: usize = 8;
/// Number of application witness polynomials before norm padding.
pub const APPLICATION_WITNESS_POLYNOMIALS_V1: usize = 48;

/// Internal Lantern/LNP22 proof modulus.
pub const PROOF_MODULUS_V1: u64 = 1_125_899_906_843_221;
/// Canonical inverse of two modulo the internal proof modulus.
pub const PROOF_INVERSE_TWO_V1: u64 = 562_949_953_421_611;
/// Canonical inverse of four modulo the internal proof modulus.
pub const PROOF_INVERSE_FOUR_V1: u64 = 844_424_930_132_416;
/// Canonical inverse of the application modulus modulo the proof modulus.
pub const APPLICATION_MODULUS_INVERSE_IN_PROOF_V1: u64 = 305_914_215_066_280;
/// Exact quotient bound used when lifting the eight application equations
/// from `R_p` into `R_q`.
pub const APPLICATION_RELATION_QUOTIENT_BOUND_V1: u64 = 30_064;
/// Encoded bit width of an internal proof residue.
pub const PROOF_MODULUS_BITS_V1: u8 = 51;
/// Canonical wire width of one proof residue.
pub const PROOF_RESIDUE_BYTES_V1: usize = 7;
/// Compression exponent used by the mathematical ABDLOP relation.
pub const DECOMPOSITION_BITS_V1: u8 = 15;
/// Compression gamma.
pub const COMPRESSION_GAMMA_V1: u64 = 16_531_490;
/// Compression modulus.
pub const COMPRESSION_MODULUS_V1: u64 = 68_106_378;

/// Internal short-message dimension including two norm-padding polynomials.
pub const TBOX_M1_V1: usize = 50;
/// Internal mask dimension.
pub const TBOX_M2_V1: usize = 64;
/// Large-message dimension.
pub const TBOX_L_V1: usize = 0;
/// Extended-message dimension.
pub const TBOX_LEXT_V1: usize = 12;
/// Module-SIS commitment dimension.
pub const TBOX_KMSIS_V1: usize = 20;
/// Number of binary witness polynomials.
pub const BINARY_POLYNOMIALS_V1: usize = 16;
/// Number of exact application relations.
pub const EXACT_RELATIONS_V1: usize = 8;
/// Number of norm statements.
pub const NORM_STATEMENTS_V1: usize = 2;
/// Number of exact-coordinate relations.
pub const EXACT_COORDINATE_RELATIONS_V1: usize = 50;
/// Schwartz-Zippel repetition parameter.
pub const LAMBDA_V1: usize = 4;

/// Challenge coefficient magnitude.
pub const CHALLENGE_OMEGA_V1: i64 = 8;
/// Conservative base-two challenge-set size used by the proof theorem.
pub const CHALLENGE_SET_BITS_V1: u16 = 129;
/// Challenge rejection parameter.
pub const CHALLENGE_ETA_V1: u16 = 140;

/// Exact squared bound on the credential randomness vector.
pub const RANDOMNESS_NORM_SQUARED_BOUND_V1: u64 = 11_881;
/// Exact squared bound on the signature preimage.
pub const SIGNATURE_NORM_SQUARED_BOUND_V1: u64 = 34_034_726;
/// Response squared-norm bound.
pub const RESPONSE_NORM_SQUARED_BOUND_V1: u64 = 143_158_532_224_272_924;
/// `z1` squared-norm bound computed as
/// `floor(2 * 50 * 64 * 962 * 2^(2*23) / 400)`.
pub const Z1_NORM_SQUARED_BOUND_V1: u64 = 1_083_115_710_382_604_288;
/// `z3` squared-norm bound.
pub const Z3_NORM_SQUARED_BOUND_V1: u64 = 113_676_554_463_109;
/// `z4` infinity-norm bound.
pub const Z4_INFINITY_NORM_BOUND_V1: u64 = 13_314_398_617;

/// Rounded square of `1.55 * 2^23`.
pub const GAUSSIAN_1_VARIANCE_V1: u64 = 169_060_907_886_838;
/// Rounded square of `1.55 * 2^12`.
pub const GAUSSIAN_2_VARIANCE_V1: u64 = 40_307_261;
/// Rounded square of `1.55 * 2^18`.
pub const GAUSSIAN_3_VARIANCE_V1: u64 = 165_098_542_858;
/// Rounded square of `1.55 * 2^29`.
pub const GAUSSIAN_4_VARIANCE_V1: u64 = 692_473_478_704_487_465;

/// Exact truncation bounds `20 * sigma = 31 * 2^k` for native sampling.
pub const GAUSSIAN_TRUNCATION_BOUNDS_V1: [i64; 4] =
    [260_046_848, 126_976, 8_126_464, 16_642_998_272];

/// Fixed-point `M * 2^128` limbs for the four rejection decisions.
pub const REJECTION_M_LIMBS_V1: [[u64; 3]; 4] = [
    [9_859_465_049_745_963_634, 5_934_122_506_364_705_697, 2],
    [15_002_606_599_518_648_986, 13_043_305_028_329_312_333, 2],
    [7_569_140_089_676_268_920, 652_277_607_672_955_118, 1],
    [1_037_701_719_389_014_861, 363_797_679_677_738_827, 1],
];

/// Internal CRT primes. These are an implementation optimization and never
/// appear on the wire or define consensus residues.
pub const INTERNAL_CRT_PRIMES_V1: [u64; 3] = [
    1_125_899_906_840_833,
    1_125_899_906_839_937,
    1_125_899_906_837_633,
];

/// Maximum number of whole-proof rejection attempts.
pub const MAX_PROOF_SAMPLING_ATTEMPTS_V1: u32 = 4_096;
/// Maximum Gaussian proposal attempts for one coefficient.
pub const MAX_GAUSSIAN_COEFFICIENT_ATTEMPTS_V1: u32 = 4_096;

/// Pinned mathematical source profile.
pub const SOURCE_PROFILE_V1: &[u8] =
    b"BLNS-CRYPTO-2023-eprint-2023-560:LaZeR-10eafeca4cd53ff4fc54193dce904dbd0026fefd";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modular_inverse_constants_are_exact() {
        assert_eq!(
            (u128::from(PROOF_INVERSE_TWO_V1) * 2) % u128::from(PROOF_MODULUS_V1),
            1
        );
        assert_eq!(
            (u128::from(PROOF_INVERSE_FOUR_V1) * 4) % u128::from(PROOF_MODULUS_V1),
            1
        );
        assert_eq!(
            (u128::from(APPLICATION_MODULUS_INVERSE_IN_PROOF_V1)
                * u128::from(APPLICATION_MODULUS_V1))
                % u128::from(PROOF_MODULUS_V1),
            1
        );
    }

    #[test]
    fn compression_identity_is_exact() {
        assert_eq!(
            u128::from(COMPRESSION_GAMMA_V1) * u128::from(COMPRESSION_MODULUS_V1) + 1,
            u128::from(PROOF_MODULUS_V1)
        );
    }

    #[test]
    fn profile_dimensions_are_internally_consistent() {
        assert_eq!(APPLICATION_ROWS_V1, EXACT_RELATIONS_V1);
        assert_eq!(
            APPLICATION_WITNESS_POLYNOMIALS_V1 + NORM_STATEMENTS_V1,
            TBOX_M1_V1
        );
        assert_eq!(TBOX_M2_V1 - TBOX_KMSIS_V1, 44);
        assert_eq!(BINARY_POLYNOMIALS_V1, ATTRIBUTE_COUNT_V1 * 2);
    }
}
