//! Prover-only Gaussian and randomized CELPC encoding support.
//!
//! No floating-point value produced here is admitted by consensus.  Verifiers
//! reconstruct exact integer coefficients and enforce pinned integer norms.
//! The prover accepts only a caller-supplied `CryptoRng`, bounds every rejection
//! loop, and returns an error instead of hanging on a failed or adversarial
//! randomness source.

use rand_core_06::{CryptoRng, RngCore};
use thiserror::Error;

use super::{
    JINDO_ENCODING_BASE_V1, JINDO_ENCODING_EXPONENT_V1, JINDO_ENCODING_SLOTS_V1,
    JINDO_RING_DEGREE_V1,
    encoding::encode_coefficient_slots_v1,
    field::JindoFieldElementV1,
    ring::{JINDO_INNER_MODULI_V1, JindoPrimeModulusV1, JindoRnsPolynomialV1},
};

/// Standard deviation for ordinary randomized coefficient encodings.
pub(crate) const JINDO_ECD_STD_DEV_V1: f64 = f64::from_bits(0x4013_265d_8bbe_d26c);
/// Standard deviation for the evaluation-sensitive first row.
pub(crate) const JINDO_ECD_BLIND_STD_DEV_V1: f64 = f64::from_bits(0x4131_9c98_a835_4ea5);
/// Standard deviation for mask-row encodings.
pub(crate) const JINDO_MASK_STD_DEV_V1: f64 = f64::from_bits(0x409b_14fd_dd24_75fe);
/// Standard deviation for the evaluation-sensitive mask row.
pub(crate) const JINDO_MASK_BLIND_STD_DEV_V1: f64 = f64::from_bits(0x41b8_e81e_3911_3843);
/// Standard deviation for ordinary MLWE hiding polynomials.
pub(crate) const JINDO_MLWE_STD_DEV_V1: f64 = f64::from_bits(0x401b_14c2_f863_e925);
/// Standard deviation for mask-column MLWE polynomials.
pub(crate) const JINDO_MASK_MLWE_STD_DEV_V1: f64 = f64::from_bits(0x40a3_2633_e6df_28bf);

const MAX_GAUSSIAN_ATTEMPTS_V1: usize = 4_096;
const GAUSSIAN_TAIL_STANDARD_DEVIATIONS_V1: f64 = 14.0;

// Only the non-negligible `-b^i / p` entries survive the paper's threshold.
const DELTA_INVERSE_V1: [f64; JINDO_ENCODING_EXPONENT_V1] = [
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    f64::from_bits(0xbbf6_5d8c_f5c1_c3be),
    f64::from_bits(0xbcf4_91a9_5344_6375),
    f64::from_bits(0xbdf2_eab6_2984_3538),
    f64::from_bits(0xbef1_65bb_e7ce_86b2),
];

/// Bounded prover-side sampling failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum JindoSamplingErrorV1 {
    /// The supplied randomness stream failed every bounded rejection attempt.
    #[error("Jindo Gaussian rejection sampler exhausted its fixed attempt budget")]
    RejectionBudgetExhausted,
    /// The requested Gaussian parameters are outside the closed profile.
    #[error("invalid Jindo Gaussian parameters")]
    InvalidGaussianParameters,
    /// A sampled coefficient exceeded the fixed signed representation.
    #[error("Jindo Gaussian sample exceeded the fixed signed coefficient range")]
    SampleOutOfRange,
}

/// Sample one discrete Gaussian integer by bounded uniform rejection.
///
/// The proposal covers `center ± 14σ`; omitted mass is below `2^-140`.
/// Rejection against `exp(-(x-center)^2/(2σ^2))` leaves the exact discrete
/// Gaussian weights within that pinned tail.  An initial two-word continuous
/// RNG health test rejects a stuck stream with a false-positive probability of
/// `2^-64`.
pub(crate) fn sample_discrete_gaussian_v1<R>(
    center: f64,
    standard_deviation: f64,
    rng: &mut R,
) -> Result<i64, JindoSamplingErrorV1>
where
    R: CryptoRng + RngCore,
{
    if !center.is_finite() || !standard_deviation.is_finite() || standard_deviation <= 0.0 {
        return Err(JindoSamplingErrorV1::InvalidGaussianParameters);
    }

    // Treat two identical consecutive 64-bit outputs as a catastrophic source
    // failure. This makes a constant/zero RNG fail closed instead of silently
    // producing a deterministic mask.
    if rng.next_u64() == rng.next_u64() {
        return Err(JindoSamplingErrorV1::RejectionBudgetExhausted);
    }

    let radius = (GAUSSIAN_TAIL_STANDARD_DEVIATIONS_V1 * standard_deviation).ceil();
    let lower = f64_to_i64_v1((center - radius).ceil())?;
    let upper = f64_to_i64_v1((center + radius).floor())?;
    let width = u64::try_from(i128::from(upper) - i128::from(lower) + 1)
        .map_err(|_| JindoSamplingErrorV1::SampleOutOfRange)?;
    if width == 0 {
        return Err(JindoSamplingErrorV1::SampleOutOfRange);
    }

    for _ in 0..MAX_GAUSSIAN_ATTEMPTS_V1 {
        let offset = sample_bounded_u64_v1(width, rng);
        let candidate = i128::from(lower) + i128::from(offset);
        let candidate =
            i64::try_from(candidate).map_err(|_| JindoSamplingErrorV1::SampleOutOfRange)?;
        let delta = candidate as f64 - center;
        let acceptance = (-(delta * delta) / (2.0 * standard_deviation * standard_deviation)).exp();
        if uniform_open_v1(rng) < acceptance {
            return Ok(candidate);
        }
    }
    Err(JindoSamplingErrorV1::RejectionBudgetExhausted)
}

/// Randomized encoding in the exact coset of the deterministic CELPC encoding.
pub(crate) fn randomized_encode_coefficient_slots_v1<R>(
    values: &[JindoFieldElementV1],
    standard_deviation: f64,
    rng: &mut R,
) -> Result<JindoRnsPolynomialV1, JindoSamplingErrorV1>
where
    R: CryptoRng + RngCore,
{
    let deterministic = encode_coefficient_slots_v1(values)
        .ok_or(JindoSamplingErrorV1::InvalidGaussianParameters)?;
    let mut encoded_coefficients = [0_i128; JINDO_RING_DEGREE_V1];
    for (index, coefficient) in encoded_coefficients.iter_mut().enumerate() {
        *coefficient = deterministic.balanced_coefficient(index, JINDO_INNER_MODULI_V1);
    }

    let mut centers = [0_f64; JINDO_RING_DEGREE_V1];
    for (digit, delta_inverse) in DELTA_INVERSE_V1.into_iter().enumerate() {
        if delta_inverse == 0.0 {
            continue;
        }
        let shift = (digit + 1) * JINDO_ENCODING_SLOTS_V1;
        let remaining = JINDO_RING_DEGREE_V1 - shift;
        for source in 0..remaining {
            centers[source + shift] += delta_inverse * encoded_coefficients[source] as f64;
        }
        for source in remaining..JINDO_RING_DEGREE_V1 {
            centers[source - remaining] -= delta_inverse * encoded_coefficients[source] as f64;
        }
    }

    let mut lattice = [0_i128; JINDO_RING_DEGREE_V1];
    for (sample, center) in lattice.iter_mut().zip(centers) {
        *sample = i128::from(sample_discrete_gaussian_v1(
            -center,
            standard_deviation,
            rng,
        )?);
    }

    let mut randomized = encoded_coefficients;
    for index in 0..JINDO_RING_DEGREE_V1 {
        randomized[index] -= i128::from(JINDO_ENCODING_BASE_V1) * lattice[index];
        if index >= JINDO_ENCODING_SLOTS_V1 {
            randomized[index] += lattice[index - JINDO_ENCODING_SLOTS_V1];
        } else {
            randomized[index] -= lattice[JINDO_RING_DEGREE_V1 - JINDO_ENCODING_SLOTS_V1 + index];
        }
    }
    Ok(JindoRnsPolynomialV1::from_balanced_coefficients(
        randomized,
        JINDO_INNER_MODULI_V1,
    ))
}

/// Sample one independent small Gaussian application-ring polynomial.
pub(crate) fn sample_gaussian_polynomial_v1<R>(
    standard_deviation: f64,
    moduli: [JindoPrimeModulusV1; 2],
    rng: &mut R,
) -> Result<JindoRnsPolynomialV1, JindoSamplingErrorV1>
where
    R: CryptoRng + RngCore,
{
    let mut coefficients = [0_i128; JINDO_RING_DEGREE_V1];
    for coefficient in &mut coefficients {
        *coefficient = i128::from(sample_discrete_gaussian_v1(0.0, standard_deviation, rng)?);
    }
    Ok(JindoRnsPolynomialV1::from_balanced_coefficients(
        coefficients,
        moduli,
    ))
}

fn sample_bounded_u64_v1(bound: u64, rng: &mut impl RngCore) -> u64 {
    debug_assert!(bound > 0);
    let acceptance_limit = u64::MAX - (u64::MAX % bound);
    loop {
        let candidate = rng.next_u64();
        if candidate < acceptance_limit {
            return candidate % bound;
        }
    }
}

fn uniform_open_v1(rng: &mut impl RngCore) -> f64 {
    const DENOMINATOR: f64 = (1_u64 << 53) as f64;
    let mantissa = rng.next_u64() >> 11;
    (mantissa as f64 + 0.5) / DENOMINATOR
}

fn f64_to_i64_v1(value: f64) -> Result<i64, JindoSamplingErrorV1> {
    const TWO_TO_63: f64 = 9_223_372_036_854_775_808.0;
    if value < -TWO_TO_63 || value >= TWO_TO_63 {
        return Err(JindoSamplingErrorV1::SampleOutOfRange);
    }
    Ok(value as i64)
}

#[cfg(test)]
mod tests {
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};

    use super::super::encoding::decode_coefficient_slots_v1;
    use super::*;

    #[derive(Clone)]
    struct TestRng(u64);

    impl TestRng {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
    }

    impl RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }

        fn next_u64(&mut self) -> u64 {
            let mut value = self.0;
            value ^= value >> 12;
            value ^= value << 25;
            value ^= value >> 27;
            self.0 = value;
            value.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            for chunk in destination.chunks_mut(8) {
                let bytes = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            self.fill_bytes(destination);
            Ok(())
        }
    }

    impl CryptoRng for TestRng {}

    struct StuckRng;

    impl RngCore for StuckRng {
        fn next_u32(&mut self) -> u32 {
            0
        }

        fn next_u64(&mut self) -> u64 {
            0
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(0);
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            self.fill_bytes(destination);
            Ok(())
        }
    }

    impl CryptoRng for StuckRng {}

    #[test]
    fn invalid_parameters_and_failed_randomness_are_rejected() {
        let mut rng = TestRng::new(1);
        assert_eq!(
            sample_discrete_gaussian_v1(0.0, 0.0, &mut rng),
            Err(JindoSamplingErrorV1::InvalidGaussianParameters)
        );
        assert_eq!(
            sample_discrete_gaussian_v1(f64::NAN, 1.0, &mut rng),
            Err(JindoSamplingErrorV1::InvalidGaussianParameters)
        );
        assert_eq!(
            sample_discrete_gaussian_v1(0.0, 1.0, &mut StuckRng),
            Err(JindoSamplingErrorV1::RejectionBudgetExhausted)
        );
    }

    #[test]
    fn deterministic_randomness_tape_is_byte_stable_and_seed_separated() {
        let mut first = TestRng::new(0x1234_5678_9abc_def0);
        let mut replay = first.clone();
        let mut distinct = TestRng::new(0x1234_5678_9abc_def1);
        let first_samples: Vec<_> = (0..64)
            .map(|_| {
                sample_discrete_gaussian_v1(0.25, JINDO_ECD_STD_DEV_V1, &mut first).expect("sample")
            })
            .collect();
        let replay_samples: Vec<_> = (0..64)
            .map(|_| {
                sample_discrete_gaussian_v1(0.25, JINDO_ECD_STD_DEV_V1, &mut replay)
                    .expect("sample")
            })
            .collect();
        let distinct_samples: Vec<_> = (0..64)
            .map(|_| {
                sample_discrete_gaussian_v1(0.25, JINDO_ECD_STD_DEV_V1, &mut distinct)
                    .expect("sample")
            })
            .collect();
        assert_eq!(first_samples, replay_samples);
        assert_ne!(first_samples, distinct_samples);
    }

    #[test]
    fn sampler_moments_match_the_pinned_small_gaussian() {
        let mut rng = TestRng::new(0xd00d_f00d_cafe_babe);
        let sample_count = 50_000_u64;
        let mut sum = 0_f64;
        let mut squares = 0_f64;
        for _ in 0..sample_count {
            let sample = sample_discrete_gaussian_v1(0.0, JINDO_MLWE_STD_DEV_V1, &mut rng)
                .expect("sample") as f64;
            sum += sample;
            squares += sample * sample;
        }
        let mean = sum / sample_count as f64;
        let variance = squares / sample_count as f64 - mean * mean;
        assert!(mean.abs() < 0.12, "mean {mean}");
        assert!(
            (variance - JINDO_MLWE_STD_DEV_V1.powi(2)).abs() < 1.0,
            "variance {variance}"
        );
    }

    #[test]
    fn randomized_encoding_preserves_every_decoded_slot() {
        let values: [JindoFieldElementV1; JINDO_ENCODING_SLOTS_V1] =
            core::array::from_fn(|index| {
                JindoFieldElementV1::from_u64(index as u64 * 1_000_003 + 17)
            });
        for (index, standard_deviation) in [
            JINDO_ECD_STD_DEV_V1,
            JINDO_ECD_BLIND_STD_DEV_V1,
            JINDO_MASK_STD_DEV_V1,
            JINDO_MASK_BLIND_STD_DEV_V1,
        ]
        .into_iter()
        .enumerate()
        {
            let randomized = randomized_encode_coefficient_slots_v1(
                &values,
                standard_deviation,
                &mut TestRng::new(index as u64 + 1),
            )
            .expect("randomized encoding");
            assert_eq!(decode_coefficient_slots_v1(&randomized), values);
        }
    }

    #[test]
    fn gaussian_polynomial_is_canonical_nonzero_and_reproducible() {
        let mut first_rng = TestRng::new(77);
        let mut replay_rng = TestRng::new(77);
        let first = sample_gaussian_polynomial_v1(
            JINDO_MASK_MLWE_STD_DEV_V1,
            JINDO_INNER_MODULI_V1,
            &mut first_rng,
        )
        .expect("sample polynomial");
        let replay = sample_gaussian_polynomial_v1(
            JINDO_MASK_MLWE_STD_DEV_V1,
            JINDO_INNER_MODULI_V1,
            &mut replay_rng,
        )
        .expect("sample polynomial");
        assert_eq!(first, replay);
        assert!(
            first
                .residues()
                .iter()
                .flatten()
                .any(|coefficient| *coefficient != 0)
        );
    }
}
