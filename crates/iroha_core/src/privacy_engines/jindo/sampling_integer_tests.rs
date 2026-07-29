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

struct CountingRng<R> {
    inner: R,
    bytes: usize,
}

impl<R> CountingRng<R> {
    fn new(inner: R) -> Self {
        Self { inner, bytes: 0 }
    }
}

impl<R: RngCore> RngCore for CountingRng<R> {
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
        self.bytes += destination.len();
        self.inner.fill_bytes(destination);
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
        self.bytes += destination.len();
        self.inner.try_fill_bytes(destination)
    }
}

impl<R: CryptoRng> CryptoRng for CountingRng<R> {}

struct ConstantRng(u8);

impl RngCore for ConstantRng {
    fn next_u32(&mut self) -> u32 {
        u32::from(self.0) * 0x0101_0101
    }

    fn next_u64(&mut self) -> u64 {
        u64::from(self.0) * 0x0101_0101_0101_0101
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        destination.fill(self.0);
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for ConstantRng {}

struct FailingRng;

impl RngCore for FailingRng {
    fn next_u32(&mut self) -> u32 {
        panic!("Jindo must use the fallible RNG interface")
    }

    fn next_u64(&mut self) -> u64 {
        panic!("Jindo must use the fallible RNG interface")
    }

    fn fill_bytes(&mut self, _destination: &mut [u8]) {
        panic!("Jindo must use the fallible RNG interface")
    }

    fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), RngError> {
        Err(RngError::new("injected Jindo RNG failure"))
    }
}

impl CryptoRng for FailingRng {}

struct TapeRng {
    bytes: Vec<u8>,
    offset: usize,
}

impl TapeRng {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, offset: 0 }
    }
}

impl RngCore for TapeRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0_u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_be_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_be_bytes(bytes)
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        let end = self.offset + destination.len();
        destination.copy_from_slice(&self.bytes[self.offset..end]);
        self.offset = end;
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for TapeRng {}

struct RejectAcceptanceRng {
    fills: usize,
    bytes: usize,
}

impl RejectAcceptanceRng {
    fn new() -> Self {
        Self { fills: 0, bytes: 0 }
    }
}

impl RngCore for RejectAcceptanceRng {
    fn next_u32(&mut self) -> u32 {
        0
    }

    fn next_u64(&mut self) -> u64 {
        0
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        self.bytes += destination.len();
        match self.fills {
            0 => destination.fill(0),
            1 => {
                destination.fill(0);
                let last = destination.len() - 1;
                destination[last] = 1;
            }
            _ if destination.len() == 8 => destination.fill(0),
            _ => destination.fill(u8::MAX),
        }
        self.fills += 1;
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for RejectAcceptanceRng {}

struct RejectProposalRng {
    fills: usize,
}

impl RngCore for RejectProposalRng {
    fn next_u32(&mut self) -> u32 {
        u32::MAX
    }

    fn next_u64(&mut self) -> u64 {
        u64::MAX
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        match self.fills {
            0 => destination.fill(0),
            1 => {
                destination.fill(0);
                let last = destination.len() - 1;
                destination[last] = 1;
            }
            _ => destination.fill(u8::MAX),
        }
        self.fills += 1;
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for RejectProposalRng {}

#[test]
fn exact_center_interval_and_maximum_exponent_are_pinned() {
    let centers = [
        SignedQ128V1::ZERO,
        SignedQ128V1::new(false, U256::ONE.shl_vartime(126)),
        SignedQ128V1::new(true, U256::ONE.shl_vartime(126)),
    ];
    let exponent_ceiling = U512::from_u64(99).shl_vartime(Q256_FRACTION_BITS_V1);
    for width in JindoGaussianWidthV1::ALL {
        let radius = SignedQ128V1::new(
            false,
            U256::from_u128(width.sigma_q64() * 14).shl_vartime(64),
        );
        for center in centers {
            let lower = add_signed_q128_v1(center, radius.negated())
                .ceil_i128()
                .expect("lower");
            let upper = add_signed_q128_v1(center, radius)
                .floor_i128()
                .expect("upper");
            let lower = i64::try_from(lower).expect("profile lower fits i64");
            let upper = i64::try_from(upper).expect("profile upper fits i64");
            assert!(absolute_delta_q128_v1(lower, center) <= radius.magnitude);
            assert!(absolute_delta_q128_v1(upper, center) <= radius.magnitude);
            assert!(absolute_delta_q128_v1(lower - 1, center) > radius.magnitude);
            assert!(absolute_delta_q128_v1(upper + 1, center) > radius.magnitude);
            assert!(
                gaussian_exponent_q256_v1(lower, center, width).expect("lower exponent")
                    < exponent_ceiling
            );
            assert!(
                gaussian_exponent_q256_v1(upper, center, width).expect("upper exponent")
                    < exponent_ceiling
            );
        }
    }
}

#[test]
fn exact_rational_centers_round_once_and_wrap_with_the_right_sign() {
    for source in [0, JINDO_RING_DEGREE_V1 - 1] {
        let mut coefficients = [0_i128; JINDO_RING_DEGREE_V1];
        coefficients[source] = 1;
        let centers = randomized_encoding_centers_v1(&coefficients).expect("centers");
        for (digit, power) in encoding_base_powers_v1().iter().copied().enumerate() {
            let shift = (digit + 1) * JINDO_ENCODING_SLOTS_V1;
            let remaining = JINDO_RING_DEGREE_V1 - shift;
            let (target, negative) = if source < remaining {
                (source + shift, false)
            } else {
                (source - remaining, true)
            };
            let center = centers[target];
            assert_eq!(center.negative, negative && center.magnitude != U256::ZERO);

            let scaled = power.shl_vartime(Q128_FRACTION_BITS_V1);
            let rounded_product =
                U512::from(&center.magnitude).wrapping_mul(&coefficient_modulus_v1());
            let error = if scaled >= rounded_product {
                scaled.wrapping_sub(&rounded_product)
            } else {
                rounded_product.wrapping_sub(&scaled)
            };
            assert!(
                error.shl_vartime(1) < coefficient_modulus_v1(),
                "Q128 center is not the nearest exact b^i/p value for digit {digit}"
            );
        }
    }
}

#[test]
fn encoded_center_extrema_and_all_six_widths_fit_signed_endpoints() {
    // Every deterministic digit is in [0, b].  Therefore a center numerator
    // is bounded by b * (1 + ... + b^15) < 2p, so |center| < 2.
    let two_q128 = U256::from_u64(2).shl_vartime(Q128_FRACTION_BITS_V1);
    let coefficients = [i128::from(JINDO_ENCODING_BASE_V1); JINDO_RING_DEGREE_V1];
    let centers = randomized_encoding_centers_v1(&coefficients).expect("maximum digit centers");
    assert!(centers.iter().all(|center| center.magnitude < two_q128));
    for width in JindoGaussianWidthV1::ALL {
        let endpoint_bound = u128::from(width.tail_radius()) + 2;
        assert!(endpoint_bound < i64::MAX as u128);
    }

    for invalid in [-1_i128, i128::from(JINDO_ENCODING_BASE_V1) + 1] {
        let mut coefficients = [0_i128; JINDO_RING_DEGREE_V1];
        coefficients[0] = invalid;
        assert_eq!(
            randomized_encoding_centers_v1(&coefficients),
            Err(JindoSamplingErrorV1::InvalidEncodedCoefficient)
        );
    }
}

#[test]
fn q256_decay_thresholds_match_independent_high_precision_vectors() {
    let one = U512::ONE.shl_vartime(Q256_FRACTION_BITS_V1);
    assert_eq!(decay_threshold_u256_v1(U512::ZERO), None);
    let cases = [
        (
            one.shr_vartime(12),
            "fff0007ffd555fffddde38e2be2d82d549cb030fde4780c8ba5d4c942f9675d9",
            "fff0007ffd555fffddde38e2be2d82d549cb030fde4780c8ba5d4c942f9675d8",
        ),
        (
            one.shr_vartime(2),
            "c75f7cf564105743415cbc9d6368f3b96071095abeaf430dbc067f714a3c19b1",
            "c75f7cf564105743415cbc9d6368f3b96071095abeaf430dbc067f714a3c1787",
        ),
        (
            one.shr_vartime(1),
            "9b4597e37cb04ff3d675a35530cdd767e347bf8ad0e80abbce4ae95861014678",
            "9b4597e37cb04ff3d675a35530cdd767e347bf8ad0e80abbce4ae95861014318",
        ),
        (
            one,
            "5e2d58d8b3bcdf1abadec7829054f90dda9805aab56c77333024b9d0a507daed",
            "5e2d58d8b3bcdf1abadec7829054f90dda9805aab56c77333024b9d0a507daee",
        ),
        (
            U512::from_u64(2).shl_vartime(Q256_FRACTION_BITS_V1),
            "22a555477f03973fb6edd5c25a052ae3f0dd961da28ac9959e1329cdbcb21c09",
            "22a555477f03973fb6edd5c25a052ae3f0dd961da28ac9959e1329cdbcb21c09",
        ),
        (
            U512::from_u64(10).shl_vartime(Q256_FRACTION_BITS_V1),
            "0002f9af36ac8f93538b648eaa1310e5f2bdf1d29cb282668b9a7147cc4dc89b",
            "0002f9af36ac8f93538b648eaa1310e5f2bdf1d29cb282668b9a7147cc4dc89a",
        ),
        (
            U512::from_u64(113).shl_vartime(Q256_FRACTION_BITS_V1 - 3),
            "00000c4fbd4af9c4eb8deb00d317a478df79b300319072d68ae2d53da4f4d06c",
            "00000c4fbd4af9c4eb8deb00d317a478df79b300319072d68ae2d53da4f4d06c",
        ),
        (
            U512::from_u64(98).shl_vartime(Q256_FRACTION_BITS_V1),
            "0000000000000000000000000000000000062147610462421d607550d3923590",
            "0000000000000000000000000000000000062147610462421d607550d3923590",
        ),
    ];
    let error_bound = U256::from_u64(1 << 16);
    for (input, expected, independent_nearest) in cases {
        let actual = decay_threshold_u256_v1(input).expect("positive decay threshold");
        assert_eq!(actual, U256::from_be_hex(expected));
        let independent_nearest = U256::from_be_hex(independent_nearest);
        let error = if actual >= independent_nearest {
            actual.wrapping_sub(&independent_nearest)
        } else {
            independent_nearest.wrapping_sub(&actual)
        };
        assert!(error <= error_bound);
    }

    let mut previous = decay_q256_v1(U512::ZERO);
    for eighths in 1..=(98 * 8) {
        let input = U512::from_u64(eighths).shl_vartime(Q256_FRACTION_BITS_V1 - 3);
        let current = decay_q256_v1(input);
        assert!(
            current < previous,
            "decay is not strict at eighth {eighths}"
        );
        assert!(current != U512::ZERO, "profile-range decay became zero");
        previous = current;
    }
}

#[test]
fn stuck_zero_ff_and_bounded_rejection_sources_fail_closed() {
    assert_eq!(
        sample_uniform_field_element_v1(&mut FailingRng),
        Err(JindoSamplingErrorV1::RandomnessUnavailable)
    );
    assert_eq!(
        sample_discrete_gaussian_v1(
            SignedQ128V1::ZERO,
            JindoGaussianWidthV1::Ecd,
            &mut FailingRng,
        ),
        Err(JindoSamplingErrorV1::RandomnessUnavailable)
    );
    for pattern in [0, u8::MAX] {
        assert_eq!(
            sample_discrete_gaussian_v1(
                SignedQ128V1::ZERO,
                JindoGaussianWidthV1::Ecd,
                &mut ConstantRng(pattern),
            ),
            Err(JindoSamplingErrorV1::RandomnessHealthCheckFailed)
        );
    }
    assert_eq!(
        sample_uniform_field_element_v1(&mut ConstantRng(u8::MAX)),
        Err(JindoSamplingErrorV1::FieldRejectionBudgetExhausted)
    );
    assert_eq!(
        sample_discrete_gaussian_v1(
            SignedQ128V1::ZERO,
            JindoGaussianWidthV1::Ecd,
            &mut RejectProposalRng { fills: 0 },
        ),
        Err(JindoSamplingErrorV1::RejectionBudgetExhausted)
    );

    let mut rejection_rng = RejectAcceptanceRng::new();
    assert_eq!(
        sample_discrete_gaussian_v1(
            SignedQ128V1::ZERO,
            JindoGaussianWidthV1::Ecd,
            &mut rejection_rng,
        ),
        Err(JindoSamplingErrorV1::RejectionBudgetExhausted)
    );
    assert_eq!(
        rejection_rng.bytes,
        2 * RNG_HEALTH_BLOCK_BYTES_V1 + MAX_GAUSSIAN_ATTEMPTS_V1 * (8 + 32)
    );
}

#[test]
fn proposal_and_acceptance_draws_are_big_endian_and_fixed_width() {
    let mut tape = vec![0_u8; 2 * RNG_HEALTH_BLOCK_BYTES_V1];
    tape[2 * RNG_HEALTH_BLOCK_BYTES_V1 - 1] = 1;
    // The Ecd interval at center zero is [-67, 67].  Big-endian offset 67
    // therefore proposes zero and has unit weight.
    tape.extend_from_slice(&67_u64.to_be_bytes());
    tape.extend_from_slice(&[0xa5; 32]);
    let mut rng = TapeRng::new(tape);
    assert_eq!(
        sample_discrete_gaussian_v1(SignedQ128V1::ZERO, JindoGaussianWidthV1::Ecd, &mut rng,),
        Ok(0)
    );
    assert_eq!(rng.offset, 2 * RNG_HEALTH_BLOCK_BYTES_V1 + 8 + 32);
}

#[test]
fn deterministic_sampler_tape_is_reproducible_seed_separated_and_bounded() {
    let mut first = CountingRng::new(TestRng::new(0x1234_5678_9abc_def0));
    let mut replay = CountingRng::new(TestRng::new(0x1234_5678_9abc_def0));
    let mut distinct = CountingRng::new(TestRng::new(0x1234_5678_9abc_def1));
    let first_samples: Vec<_> = (0..32)
        .map(|_| {
            sample_discrete_gaussian_v1(SignedQ128V1::ZERO, JindoGaussianWidthV1::Ecd, &mut first)
                .expect("sample")
        })
        .collect();
    let replay_samples: Vec<_> = (0..32)
        .map(|_| {
            sample_discrete_gaussian_v1(SignedQ128V1::ZERO, JindoGaussianWidthV1::Ecd, &mut replay)
                .expect("sample")
        })
        .collect();
    let distinct_samples: Vec<_> = (0..32)
        .map(|_| {
            sample_discrete_gaussian_v1(
                SignedQ128V1::ZERO,
                JindoGaussianWidthV1::Ecd,
                &mut distinct,
            )
            .expect("sample")
        })
        .collect();
    assert_eq!(first_samples, replay_samples);
    assert_eq!(first.bytes, replay.bytes);
    assert_eq!(
        first_samples,
        [
            -2, 3, -5, 5, 3, -9, -8, 2, 1, 0, -3, 5, -1, 1, -4, -4, 1, -5, 5, 2, 5, 0, 6, 4, 2, -8,
            0, -7, 3, -5, 2, -4,
        ]
    );
    assert_eq!(first.bytes, 20_488);
    assert_ne!(first_samples, distinct_samples);
    assert_eq!(distinct.bytes, 17_128);
    assert!(
        first_samples
            .iter()
            .all(|sample| sample.unsigned_abs() <= JindoGaussianWidthV1::Ecd.tail_radius())
    );
}

#[test]
fn integer_moments_and_symmetry_match_the_small_profile_width() {
    let mut rng = TestRng::new(0xd00d_f00d_cafe_babe);
    let sample_count = 8_192_i128;
    let mut sum = 0_i128;
    let mut squares = 0_u128;
    let mut positive = 0_i128;
    let mut negative = 0_i128;
    for _ in 0..sample_count {
        let sample =
            sample_discrete_gaussian_v1(SignedQ128V1::ZERO, JindoGaussianWidthV1::Mlwe, &mut rng)
                .expect("sample");
        sum += i128::from(sample);
        squares += u128::from(sample.unsigned_abs()).pow(2);
        positive += i128::from(sample > 0);
        negative += i128::from(sample < 0);
    }
    assert!(sum.unsigned_abs() * 100 < sample_count.unsigned_abs() * 20);
    assert!((positive - negative).unsigned_abs() * 100 < sample_count.unsigned_abs() * 5);
    let count = sample_count.unsigned_abs();
    let variance_numerator = squares * count - sum.unsigned_abs().pow(2);
    assert!(44 * count.pow(2) < variance_numerator);
    assert!(48 * count.pow(2) > variance_numerator);
}

#[test]
fn randomized_encoding_preserves_every_slot_for_all_encoding_widths() {
    let values: [JindoFieldElementV1; JINDO_ENCODING_SLOTS_V1] =
        core::array::from_fn(|index| JindoFieldElementV1::from_u64(index as u64 * 1_000_003 + 17));
    for (index, width) in [
        JindoGaussianWidthV1::Ecd,
        JindoGaussianWidthV1::EcdBlind,
        JindoGaussianWidthV1::Mask,
        JindoGaussianWidthV1::MaskBlind,
    ]
    .into_iter()
    .enumerate()
    {
        let randomized = randomized_encode_coefficient_slots_v1(
            &values,
            width,
            &mut TestRng::new(index as u64 + 1),
        )
        .expect("randomized encoding");
        assert_eq!(decode_coefficient_slots_v1(&randomized), values);
    }
    assert_eq!(
        randomized_encode_coefficient_slots_v1(
            &values[..JINDO_ENCODING_SLOTS_V1 - 1],
            JindoGaussianWidthV1::Ecd,
            &mut TestRng::new(9),
        ),
        Err(JindoSamplingErrorV1::InvalidEncodingLength)
    );
}

#[test]
fn gaussian_polynomial_is_canonical_nonzero_and_reproducible() {
    let mut first_rng = TestRng::new(77);
    let mut replay_rng = TestRng::new(77);
    let first = sample_gaussian_polynomial_v1(
        JindoGaussianWidthV1::MaskMlwe,
        JINDO_INNER_MODULI_V1,
        &mut first_rng,
    )
    .expect("sample polynomial");
    let replay = sample_gaussian_polynomial_v1(
        JindoGaussianWidthV1::MaskMlwe,
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

#[test]
fn production_sampler_sources_exclude_native_float_and_transcendental_paths() {
    let sampling = include_str!("sampling.rs")
        .split("// INTEGER_ONLY_PRODUCTION_END")
        .next()
        .expect("production marker");
    let parameters = include_str!("parameters.rs");
    let protocol = include_str!("protocol.rs");
    let forbidden = [
        ["f", "32"].concat(),
        ["f", "64"].concat(),
        [".", "exp("].concat(),
        ["pow", "f("].concat(),
        ["libm", "::"].concat(),
    ];
    for (name, source) in [
        ("sampling", sampling),
        ("parameters", parameters),
        ("protocol", protocol),
    ] {
        for token in &forbidden {
            assert!(
                !source.contains(token),
                "{name} contains forbidden token {token}"
            );
        }
    }
}
