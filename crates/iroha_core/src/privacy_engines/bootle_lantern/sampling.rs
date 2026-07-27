//! Bounded native randomness and discrete-Gaussian sampling.
//!
//! Prover randomness enters through a fallible operating-system compatible
//! `CryptoRng`.  One seed is collected and then expanded with domain-separated
//! SHAKE256 invocations.  All proposal loops have fixed public work ceilings;
//! failure is returned instead of silently biasing a proof.

use rand_core_06::{CryptoRng, RngCore};
use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use thiserror::Error;
use zeroize::Zeroize;

use super::{
    params::{
        APPLICATION_RING_DEGREE_V1, GAUSSIAN_TRUNCATION_BOUNDS_V1,
        MAX_GAUSSIAN_COEFFICIENT_ATTEMPTS_V1, PROOF_MODULUS_V1, REJECTION_M_LIMBS_V1,
    },
    ring::ProofPolynomialV1,
};

const RANDOMNESS_DOMAIN_V1: &[u8] = b"iroha.privacy.bootle-lantern.prover-randomness.v1";
const CDF155: [(u64, u64); 21] = [
    (10_894_764_499_197_476_522, 10_804_844_707_381_617_341),
    (4_761_708_367_981_796_450, 6_209_732_027_000_382_074),
    (1_476_784_279_527_800_432, 14_108_379_346_150_813_303),
    (316_388_870_594_767_345, 17_827_298_407_763_885_637),
    (46_043_503_515_468_600, 18_385_899_657_892_021_654),
    (4_503_729_779_335_039, 3_860_889_375_818_664_979),
    (294_122_444_862_326, 13_947_176_349_836_216_550),
    (12_769_598_070_895, 14_321_894_682_751_135_119),
    (367_552_986_472, 10_286_761_328_368_440_884),
    (7_001_273_393, 2_287_787_188_970_898_528),
    (88_153_536, 17_843_977_990_435_837_663),
    (733_119, 12_174_894_787_802_692_461),
    (4_024, 18_067_426_722_645_776_197),
    (14, 10_764_017_821_655_913_055),
    (0, 643_125_733_022_530_080),
    (0, 1_014_291_014_134_832),
    (0, 1_055_215_183_460),
    (0, 724_109_373),
    (0, 327_744),
    (0, 98),
    (0, 0),
];

/// Domain-separated deterministic expansion of one caller-provided seed.
pub struct ProofRandomnessV1 {
    seed: [u8; 32],
    stream: u64,
}

impl core::fmt::Debug for ProofRandomnessV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ProofRandomnessV1")
            .field("seed", &"<redacted>")
            .field("stream", &self.stream)
            .finish()
    }
}

impl Drop for ProofRandomnessV1 {
    fn drop(&mut self) {
        self.seed.zeroize();
        self.stream.zeroize();
    }
}

impl ProofRandomnessV1 {
    /// Collect a fresh seed from a fallible cryptographic RNG.
    ///
    /// # Errors
    ///
    /// Propagates RNG failure and rejects the all-zero or one-repeated-byte
    /// catastrophic-health sentinels.
    pub fn from_rng<R: CryptoRng + RngCore>(rng: &mut R) -> Result<Self, SamplingErrorV1> {
        let mut seed = [0_u8; 32];
        rng.try_fill_bytes(&mut seed)
            .map_err(|_| SamplingErrorV1::RandomnessUnavailable)?;
        if seed == [0; 32] || seed.iter().all(|byte| *byte == seed[0]) {
            seed.zeroize();
            return Err(SamplingErrorV1::RandomnessHealthCheckFailed);
        }
        Ok(Self { seed, stream: 0 })
    }

    /// Construct a deterministic stream for known-answer and differential
    /// tests.  This is deliberately crate-private so production callers must
    /// supply a `CryptoRng`.
    #[cfg(test)]
    pub(crate) fn for_test(seed: [u8; 32]) -> Result<Self, SamplingErrorV1> {
        if seed == [0; 32] || seed.iter().all(|byte| *byte == seed[0]) {
            return Err(SamplingErrorV1::RandomnessHealthCheckFailed);
        }
        Ok(Self { seed, stream: 0 })
    }

    /// Fill bytes from a separately framed SHAKE256 stream.
    pub fn fill_bytes(&mut self, domain: &[u8], output: &mut [u8]) {
        let mut state = Shake256::default();
        absorb_frame(&mut state, RANDOMNESS_DOMAIN_V1);
        absorb_frame(&mut state, &self.seed);
        absorb_frame(&mut state, domain);
        absorb_frame(&mut state, &self.stream.to_be_bytes());
        self.stream = self
            .stream
            .checked_add(1)
            .expect("fixed proof work cannot exhaust u64 streams");
        let mut reader = state.finalize_xof();
        reader.read(output);
    }

    /// Draw one unbiased sign in `{ -1, +1 }`.
    pub fn sign(&mut self, domain: &[u8]) -> i64 {
        let mut byte = [0_u8; 1];
        self.fill_bytes(domain, &mut byte);
        if byte[0] & 1 == 0 { 1 } else { -1 }
    }

    /// Draw one unbiased ternary coefficient within the fixed work budget.
    ///
    /// # Errors
    ///
    /// Returns [`SamplingErrorV1::UniformSamplingExhausted`] if every bounded
    /// proposal is the sole rejected byte value.
    pub fn ternary(&mut self, domain: &[u8]) -> Result<i64, SamplingErrorV1> {
        for _ in 0..MAX_GAUSSIAN_COEFFICIENT_ATTEMPTS_V1 {
            let mut byte = [0_u8; 1];
            self.fill_bytes(domain, &mut byte);
            if byte[0] < 255 {
                return Ok(i64::from(byte[0] % 3) - 1);
            }
        }
        Err(SamplingErrorV1::UniformSamplingExhausted)
    }

    /// Draw one uniform proof-ring polynomial.
    pub fn uniform_polynomial(
        &mut self,
        domain: &[u8],
    ) -> Result<ProofPolynomialV1, SamplingErrorV1> {
        let mut coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
        for coefficient in &mut coefficients {
            *coefficient = self.uniform_modulus(domain, PROOF_MODULUS_V1)?;
        }
        ProofPolynomialV1::new(coefficients).map_err(|_| SamplingErrorV1::InternalInvariant)
    }

    /// Draw one polynomial with independent ternary coefficients.
    pub fn ternary_polynomial(
        &mut self,
        domain: &[u8],
    ) -> Result<ProofPolynomialV1, SamplingErrorV1> {
        let mut coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
        for coefficient in &mut coefficients {
            *coefficient = self.ternary(domain)?;
        }
        Ok(ProofPolynomialV1::from_centered_coefficients(coefficients))
    }

    /// Draw one centered discrete-Gaussian polynomial with standard deviation
    /// `1.55 * 2^log2_sigma`.
    pub fn gaussian_polynomial(
        &mut self,
        log2_sigma: u8,
        parameter_index: usize,
        domain: &[u8],
    ) -> Result<ProofPolynomialV1, SamplingErrorV1> {
        if parameter_index >= GAUSSIAN_TRUNCATION_BOUNDS_V1.len() || log2_sigma >= 63 {
            return Err(SamplingErrorV1::InvalidGaussianParameter);
        }
        let mut coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
        for coefficient in &mut coefficients {
            *coefficient = self.gaussian_coefficient(log2_sigma, parameter_index, domain)?;
        }
        Ok(ProofPolynomialV1::from_centered_coefficients(coefficients))
    }

    /// Apply the standard Lyubashevsky rejection decision.
    pub fn accept_standard(
        &mut self,
        z: &[i64],
        shift: &[i64],
        parameter_index: usize,
        variance: u64,
    ) -> Result<bool, SamplingErrorV1> {
        if z.len() != shift.len() || parameter_index >= REJECTION_M_LIMBS_V1.len() || variance == 0
        {
            return Err(SamplingErrorV1::InvalidRejectionParameter);
        }
        let (dot, norm) = dot_and_norm(z, shift)?;
        let numerator = norm
            .checked_sub(
                dot.checked_mul(2)
                    .ok_or(SamplingErrorV1::ArithmeticOverflow)?,
            )
            .ok_or(SamplingErrorV1::ArithmeticOverflow)?;
        let exponent = (numerator as f64) / (2.0 * variance as f64);
        let probability = (exponent.exp() / rejection_m(parameter_index)).min(1.0);
        Ok(self.uniform_unit(b"standard-rejection") < probability)
    }

    /// Apply the bimodal rejection decision used by the projected norm proofs.
    pub fn accept_bimodal(
        &mut self,
        z: &[i64],
        shift: &[i64],
        parameter_index: usize,
        variance: u64,
    ) -> Result<bool, SamplingErrorV1> {
        if z.len() != shift.len() || parameter_index >= REJECTION_M_LIMBS_V1.len() || variance == 0
        {
            return Err(SamplingErrorV1::InvalidRejectionParameter);
        }
        let (dot, norm) = dot_and_norm(z, shift)?;
        let denominator = rejection_m(parameter_index)
            * (-(norm as f64) / (2.0 * variance as f64)).exp()
            * ((dot as f64) / variance as f64).cosh();
        let probability = (1.0 / denominator).min(1.0);
        Ok(self.uniform_unit(b"bimodal-rejection") < probability)
    }

    fn gaussian_coefficient(
        &mut self,
        log2_sigma: u8,
        parameter_index: usize,
        domain: &[u8],
    ) -> Result<i64, SamplingErrorV1> {
        let scale = 1_u64 << log2_sigma;
        let fractional = self.uniform_modulus(domain, scale)?;
        let center = fractional as f64 / scale as f64;
        for _ in 0..MAX_GAUSSIAN_COEFFICIENT_ATTEMPTS_V1 {
            let sign_bit = self.sign(b"gaussian-sign") < 0;
            let magnitude = i64::from(self.cdf155_sample());
            let candidate = if sign_bit { magnitude + 1 } else { -magnitude };
            let sign_as_integer = i64::from(sign_bit);
            let x = (((candidate as f64 - center).powi(2)
                - (candidate - sign_as_integer) as f64 * (candidate - sign_as_integer) as f64)
                / (2.0 * 1.55 * 1.55))
                .max(0.0);
            if self.ber_exp(x) {
                let sample = candidate
                    .checked_mul(i64::try_from(scale).expect("scale fits i64"))
                    .and_then(|scaled| {
                        scaled.checked_sub(
                            i64::try_from(fractional).expect("fractional value fits i64"),
                        )
                    })
                    .ok_or(SamplingErrorV1::ArithmeticOverflow)?;
                if sample.unsigned_abs()
                    <= u64::try_from(GAUSSIAN_TRUNCATION_BOUNDS_V1[parameter_index])
                        .expect("positive truncation bound")
                {
                    return Ok(sample);
                }
            }
        }
        Err(SamplingErrorV1::GaussianSamplingExhausted)
    }

    fn cdf155_sample(&mut self) -> u32 {
        let mut bytes = [0_u8; 16];
        self.fill_bytes(b"gaussian-cdf", &mut bytes);
        let high = u64::from_le_bytes(bytes[..8].try_into().expect("exact half"));
        let low = u64::from_le_bytes(bytes[8..].try_into().expect("exact half"));
        let mut sample = 0_usize;
        while sample + 1 < CDF155.len()
            && (high < CDF155[sample].0 || (high == CDF155[sample].0 && low < CDF155[sample].1))
        {
            sample += 1;
        }
        u32::try_from(sample).expect("fixed CDF index fits u32")
    }

    fn ber_exp(&mut self, exponent: f64) -> bool {
        debug_assert!(exponent.is_finite() && exponent >= 0.0);
        self.uniform_unit(b"gaussian-ber-exp") < (-exponent).exp()
    }

    fn uniform_unit(&mut self, domain: &[u8]) -> f64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(domain, &mut bytes);
        let value = u64::from_le_bytes(bytes) >> 11;
        value as f64 / ((1_u64 << 53) as f64)
    }

    fn uniform_modulus(&mut self, domain: &[u8], modulus: u64) -> Result<u64, SamplingErrorV1> {
        if modulus == 0 {
            return Err(SamplingErrorV1::InternalInvariant);
        }
        let limit = u64::MAX - (u64::MAX % modulus);
        for _ in 0..MAX_GAUSSIAN_COEFFICIENT_ATTEMPTS_V1 {
            let mut bytes = [0_u8; 8];
            self.fill_bytes(domain, &mut bytes);
            let candidate = u64::from_le_bytes(bytes);
            if candidate < limit {
                return Ok(candidate % modulus);
            }
        }
        Err(SamplingErrorV1::UniformSamplingExhausted)
    }
}

fn absorb_frame(state: &mut Shake256, bytes: &[u8]) {
    let length = u32::try_from(bytes.len()).expect("fixed randomness frame fits u32");
    state.update(&length.to_be_bytes());
    state.update(bytes);
}

fn rejection_m(index: usize) -> f64 {
    const TWO64: f64 = 18_446_744_073_709_551_616.0;
    const TWO128: f64 = TWO64 * TWO64;
    let limbs = REJECTION_M_LIMBS_V1[index];
    limbs[2] as f64 + limbs[1] as f64 / TWO64 + limbs[0] as f64 / TWO128
}

fn dot_and_norm(lhs: &[i64], rhs: &[i64]) -> Result<(i128, i128), SamplingErrorV1> {
    let mut dot = 0_i128;
    let mut norm = 0_i128;
    for (lhs, rhs) in lhs.iter().copied().zip(rhs.iter().copied()) {
        dot = dot
            .checked_add(i128::from(lhs) * i128::from(rhs))
            .ok_or(SamplingErrorV1::ArithmeticOverflow)?;
        norm = norm
            .checked_add(i128::from(rhs) * i128::from(rhs))
            .ok_or(SamplingErrorV1::ArithmeticOverflow)?;
    }
    Ok((dot, norm))
}

/// Bounded sampling failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum SamplingErrorV1 {
    /// The external cryptographic RNG failed.
    #[error("Bootle/Lantern cryptographic randomness is unavailable")]
    RandomnessUnavailable,
    /// The seed matched a catastrophic stuck-RNG sentinel.
    #[error("Bootle/Lantern cryptographic randomness failed its health check")]
    RandomnessHealthCheckFailed,
    /// A Gaussian selector was outside the fixed profile.
    #[error("Bootle/Lantern Gaussian parameter is outside the fixed profile")]
    InvalidGaussianParameter,
    /// A rejection-sampling selector or shape was invalid.
    #[error("Bootle/Lantern rejection parameter is outside the fixed profile")]
    InvalidRejectionParameter,
    /// Uniform rejection exceeded its fixed work budget.
    #[error("Bootle/Lantern uniform sampling exhausted its fixed work budget")]
    UniformSamplingExhausted,
    /// Gaussian rejection exceeded its fixed work budget.
    #[error("Bootle/Lantern Gaussian sampling exhausted its fixed work budget")]
    GaussianSamplingExhausted,
    /// Checked integer arithmetic overflowed.
    #[error("Bootle/Lantern sampling arithmetic overflowed")]
    ArithmeticOverflow,
    /// A fixed internal invariant failed.
    #[error("Bootle/Lantern sampling internal invariant failed")]
    InternalInvariant,
}

#[cfg(test)]
mod tests {
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};

    use super::*;

    struct TestRng {
        state: u64,
        fail: bool,
        stuck: Option<u8>,
    }

    impl RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0_u8; 4];
            self.fill_bytes(&mut bytes);
            u32::from_le_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0_u8; 8];
            self.fill_bytes(&mut bytes);
            u64::from_le_bytes(bytes)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            self.try_fill_bytes(destination)
                .expect("infallible test invocation");
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            if self.fail {
                return Err(RngError::new("injected failure"));
            }
            if let Some(byte) = self.stuck {
                destination.fill(byte);
                return Ok(());
            }
            for byte in destination {
                self.state ^= self.state << 13;
                self.state ^= self.state >> 7;
                self.state ^= self.state << 17;
                *byte = self.state as u8;
            }
            Ok(())
        }
    }

    impl CryptoRng for TestRng {}

    fn test_randomness() -> ProofRandomnessV1 {
        ProofRandomnessV1::for_test(core::array::from_fn(|index| {
            u8::try_from(index + 1).expect("index fits")
        }))
        .expect("healthy seed")
    }

    #[test]
    fn external_rng_failure_and_stuck_sentinels_fail_closed() {
        let mut failed = TestRng {
            state: 1,
            fail: true,
            stuck: None,
        };
        assert!(matches!(
            ProofRandomnessV1::from_rng(&mut failed),
            Err(SamplingErrorV1::RandomnessUnavailable)
        ));
        for byte in [0, 1, 0xA5, u8::MAX] {
            let mut stuck = TestRng {
                state: 1,
                fail: false,
                stuck: Some(byte),
            };
            assert!(matches!(
                ProofRandomnessV1::from_rng(&mut stuck),
                Err(SamplingErrorV1::RandomnessHealthCheckFailed)
            ));
        }
    }

    #[test]
    fn deterministic_stream_is_domain_and_position_separated() {
        let mut first = test_randomness();
        let mut second = test_randomness();
        let mut a = [0_u8; 64];
        let mut b = [0_u8; 64];
        let mut c = [0_u8; 64];
        first.fill_bytes(b"a", &mut a);
        second.fill_bytes(b"a", &mut b);
        second.fill_bytes(b"a", &mut c);
        assert_eq!(a, b);
        assert_ne!(b, c);

        let mut changed = test_randomness();
        changed.fill_bytes(b"b", &mut c);
        assert_ne!(a, c);
    }

    #[test]
    fn gaussian_and_ternary_outputs_are_canonical_and_bounded() {
        let mut randomness = test_randomness();
        for (parameter_index, log2_sigma) in [23_u8, 12, 18, 29].into_iter().enumerate() {
            let polynomial = randomness
                .gaussian_polynomial(log2_sigma, parameter_index, b"gaussian-test")
                .expect("bounded Gaussian");
            assert!(
                polynomial
                    .coefficients()
                    .iter()
                    .all(|coefficient| *coefficient < PROOF_MODULUS_V1)
            );
            assert!((0..APPLICATION_RING_DEGREE_V1).all(|index| {
                polynomial.centered_coefficient(index).unsigned_abs()
                    <= u64::try_from(GAUSSIAN_TRUNCATION_BOUNDS_V1[parameter_index])
                        .expect("positive")
            }));
        }
        let ternary = randomness
            .ternary_polynomial(b"ternary-test")
            .expect("bounded ternary polynomial");
        assert!(
            (0..APPLICATION_RING_DEGREE_V1)
                .all(|index| (-1..=1).contains(&ternary.centered_coefficient(index)))
        );
    }

    #[test]
    fn rejection_decisions_validate_shapes_and_remain_probabilistic() {
        let mut randomness = test_randomness();
        assert_eq!(
            randomness.accept_standard(&[0], &[], 0, 1),
            Err(SamplingErrorV1::InvalidRejectionParameter)
        );
        assert_eq!(
            randomness.accept_bimodal(&[0], &[0], 4, 1),
            Err(SamplingErrorV1::InvalidRejectionParameter)
        );
        let mut accepted = 0;
        for _ in 0..256 {
            accepted += usize::from(
                randomness
                    .accept_standard(&[0, 0], &[1, -1], 0, 1_000)
                    .expect("decision"),
            );
        }
        assert!(accepted > 0 && accepted < 256);
    }
}
