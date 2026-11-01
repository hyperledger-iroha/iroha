//! Genuine full-size F257 scalar bootstrap and one square/refresh repetition.
//!
//! This ignored unselected N65536 diagnostic uses honest local key generation.
//! Every coefficient has a production phase calculation followed by independent
//! `BigInt` CRT, message and bound checks. Independent direct secret convolution
//! covers all 128 scalar-stride coefficients plus seven off-stride/boundary
//! probes, not every full-size coefficient. Small tests validate that oracle
//! against full schoolbook convolution. No full application, proof, parameter,
//! circular/KDM security or original application-resource qualification follows.
//! This test intentionally opens phases under separate secret custody. `BigInt`
//! oracle buffers are not guaranteed cleared, so this fixture does not certify
//! production custody. Raw phase/ciphertext words and copied secret coefficients
//! use owners that clear on drop.

mod support;

use std::{mem::size_of, time::Instant};

use iroha_fhe::{
    bgv::{
        BgvParameters, BgvProfile, BootstrapBudget, BootstrapError, BootstrapMessageSpace,
        BootstrapPlan, Ciphertext, CoefficientSlotBudget, LocalBootstrapKey,
        LocalBootstrapSetupBudget, SecretKey, SlotEncoding, SlotLinearBudget, UpperDigitsBudget,
        fixtures,
    },
    natural::Natural,
    prime_power::{
        PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan, UpperDigitsProgram,
    },
};
use num_bigint::{BigInt, Sign};
use support::oracle;
use zeroize::Zeroizing;

const SLOTS: usize = 128;
const LEVELS: [usize; 18] = [
    20, 19, 18, 17, 16, 15, 14, 13, 13, 11, 10, 9, 8, 7, 6, 5, 4, 4,
];

fn program() -> UpperDigitsProgram {
    let context = PrimePowerContext::new(257, 3).unwrap();
    let geometry = UpperDigitsPlan::required(context, 1).unwrap();
    let plan = UpperDigitsPlan::new(
        context,
        1,
        ProgramBudget {
            coefficient_bytes: geometry.coefficient_bytes,
            preparation_scratch_bytes: geometry.preparation_scratch_bytes,
            program_bytes: geometry.program_bytes,
            preparation_operations: geometry.preparation_operations,
            live_values: geometry.live_values,
        },
    )
    .unwrap();
    UpperDigitsProgram::prepare(&plan).unwrap()
}

fn budget() -> BootstrapBudget {
    // The existing q23 public planner's finite stage ceilings retain its exact
    // routing split. Outer work/stream caps use its independently checked values.
    let transform = CoefficientSlotBudget {
        routing: SlotLinearBudget {
            coefficient_bytes: 128 * 129 * 4,
            layout_metadata_bytes: 16 << 10,
            preparation_scratch_bytes: 129 * 4,
            output_residue_bytes: 32 << 20,
            scratch_residue_bytes: 512 << 20,
        },
        owned_residue_bytes: 1 << 30,
        trace_plaintext_bytes: 65_536 * 8,
    };
    BootstrapBudget {
        decomposition: transform,
        recomposition: transform,
        upper_digits: UpperDigitsBudget {
            compiler_steps: 1_000_000,
            metadata_bytes: 16 << 20,
            owned_residue_bytes: 1 << 30,
            plaintext_bytes: 65_536 * 8,
            backend_scratch_bytes: 128 << 20,
            canonical_arithmetic_units: 1_u64 << 60,
            scalar_word_operations: 1_u64 << 50,
        },
        retained_preparation_bytes: 32 << 20,
        input_owned_bytes: 64 << 20,
        input_plaintext_bytes: 2 * 65_536 * 8,
        input_arithmetic_units: 1_u64 << 40,
        input_addition_words: 1_u64 << 30,
        stream_owned_residue_bytes: 409_993_216,
        stream_metadata_bytes: 1 << 20,
        canonical_arithmetic_units: 24_584_388_608,
        scalar_word_operations: 1_075_380_224,
    }
}

fn setup_budget(profile: &BgvProfile, galois: &[u32]) -> LocalBootstrapSetupBudget {
    let coefficient_bytes = profile.degree() * size_of::<u64>();
    let main_limbs = profile.top_level() + 1;
    let full_limbs = main_limbs + profile.special_count();
    let switch_limbs = (galois.len() + 1) * 2 * profile.digit_count() * full_limbs;
    LocalBootstrapSetupBudget {
        retained_residue_bytes: coefficient_bytes * (4 * main_limbs + switch_limbs),
        returned_secret_bytes: coefficient_bytes * (full_limbs + 1),
        plaintext_bytes: coefficient_bytes,
    }
}

fn power(mut base: u64, mut exponent: usize) -> u64 {
    let mut value = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            value = value * base % 257;
        }
        base = base * base % 257;
        exponent >>= 1;
    }
    value
}

fn expected_coefficients(degree: usize, slots: &[u64; SLOTS]) -> Vec<u64> {
    assert_eq!(degree % SLOTS, 0);
    let mut scalar = [0_u64; SLOTS];
    for (slot, &value) in slots.iter().enumerate() {
        let inverse_root = power(3, 256 - (2 * slot + 1));
        let mut factor = 1;
        for coefficient in &mut scalar {
            *coefficient = (*coefficient + value * factor) % 257;
            factor = factor * inverse_root % 257;
        }
    }
    // 128^-1 mod 257 = 255; direct inverse Vandermonde, independent of encoding.
    let mut coefficients = vec![0; degree];
    for (index, coefficient) in scalar.into_iter().enumerate() {
        coefficients[index * (degree / SLOTS)] = coefficient * 255 % 257;
    }
    coefficients
}

fn probes(degree: usize, stride: usize) -> Vec<usize> {
    let mut indices: Vec<_> = (0..SLOTS).map(|i| i * stride).collect();
    indices.extend([
        1,
        stride - 1,
        stride + 1,
        degree / 2 - 1,
        degree / 2 + 1,
        degree - 2,
        degree - 1,
    ]);
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn direct_phase_residue(c0: &[u64], c1: &[u64], secret: &[i64], prime: u64, at: usize) -> u64 {
    assert_eq!(c0.len(), secret.len());
    assert_eq!(c1.len(), secret.len());
    // Ternary secret, N=65536 and primes below 2^60 bound the signed sum by 2^77.
    // Reduce once after the direct negacyclic sum; no NTT or production kernel.
    let mut sum = i128::from(c0[at]);
    for (index, &coefficient) in secret.iter().enumerate() {
        let (source, sign) = if index <= at {
            (at - index, 1_i128)
        } else {
            (at + secret.len() - index, -1_i128)
        };
        sum += sign * i128::from(coefficient) * i128::from(c1[source]);
    }
    u64::try_from(sum.rem_euclid(i128::from(prime))).unwrap()
}

fn integer(value: &Natural) -> BigInt {
    let bytes: Vec<_> = value
        .words_le()
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    BigInt::from_bytes_le(Sign::Plus, &bytes)
}

fn check_phase(
    profile: &BgvProfile,
    secret: &SecretKey,
    ciphertext: &Ciphertext,
    slots: &[u64; SLOTS],
    label: &str,
) {
    let started = Instant::now();
    let degree = profile.degree();
    assert_eq!(degree, 65_536);
    let expected = expected_coefficients(degree, slots);
    assert_eq!(ciphertext.component_count(), 2);
    assert_eq!(ciphertext.plaintext_precision().modulus(), 257);
    let primes = &profile.parameters().main_primes[..=ciphertext.level()];
    assert!(primes.iter().all(|&prime| prime < 1_u64 << 60));
    let phase_words =
        Zeroizing::new(fixtures::inner_product_words(profile, secret, ciphertext).unwrap());
    assert_eq!(phase_words.len(), degree * primes.len());
    let modulus = oracle::product(primes);
    let bound = integer(ciphertext.noise_bound().as_natural());
    assert!(&bound * 2_u8 < modulus);
    let phase = oracle::lift(&phase_words, primes);
    assert_eq!(phase.len(), degree);
    for (index, value) in phase.iter().enumerate() {
        let centered = oracle::centered(value, &modulus);
        assert!(
            -&bound <= centered && centered <= bound,
            "{label}: coefficient {index} phase bound"
        );
        assert_eq!(
            oracle::word_mod(&centered, 257),
            expected[index],
            "{label}: coefficient {index}"
        );
        if index % (degree / SLOTS) != 0 {
            assert_eq!(expected[index], 0);
        }
    }
    assert_eq!(
        secret.decrypt(profile, ciphertext).unwrap().coefficients(),
        expected
    );
    let words = Zeroizing::new(ciphertext.canonical_words());
    let component_words = degree * primes.len();
    assert_eq!(words.len(), 2 * component_words);
    let coefficients = Zeroizing::new(fixtures::secret_coefficients(secret));
    assert_eq!(coefficients.len(), degree);
    assert!(coefficients.iter().all(|value| (-1..=1).contains(value)));
    let indices = probes(degree, degree / SLOTS);
    assert_eq!(indices.len(), 135);
    assert!((0..SLOTS).all(|i| indices.contains(&(i * (degree / SLOTS)))));
    for (limb, &prime) in primes.iter().enumerate() {
        let c0 = &words[limb * degree..(limb + 1) * degree];
        let c1 = &words[component_words + limb * degree..component_words + (limb + 1) * degree];
        for &index in &indices {
            assert_eq!(
                direct_phase_residue(c0, c1, &coefficients, prime, index),
                phase_words[limb * degree + index],
                "{label}: independent probe {index}, limb {limb}"
            );
        }
    }
    eprintln!(
        "scalar_fullsize_phase label={label} level={} full_crt_coefficients={degree} independent_convolution_probes={} elapsed_ms={}",
        ciphertext.level(),
        indices.len(),
        started.elapsed().as_millis()
    );
}

#[test]
fn independent_scalar_vandermonde_roundtrip_and_small_full_convolution() {
    let slots = std::array::from_fn(|i| u64::try_from(i + 1).unwrap());
    let coefficients = expected_coefficients(256, &slots);
    assert!(
        coefficients
            .iter()
            .enumerate()
            .all(|(i, &value)| i % 2 == 0 || value == 0)
    );
    for (slot, &expected) in slots.iter().enumerate() {
        let root = power(3, 2 * slot + 1);
        let (actual, _) = coefficients
            .iter()
            .step_by(2)
            .fold((0, 1), |(sum, factor), &value| {
                ((sum + value * factor) % 257, factor * root % 257)
            });
        assert_eq!(actual, expected);
    }
    let degree = 16;
    let primes = [97_u64, 193];
    let secret: Vec<_> = (0..degree)
        .map(|i| i64::try_from(i % 3).unwrap() - 1)
        .collect();
    let c0: Vec<_> = primes
        .iter()
        .flat_map(|&prime| (0..degree).map(move |i| (u64::try_from(i).unwrap() * 17 + 29) % prime))
        .collect();
    let c1: Vec<_> = primes
        .iter()
        .flat_map(|&prime| (0..degree).map(move |i| (u64::try_from(i).unwrap() * 37 + 5) % prime))
        .collect();
    let modulus = oracle::product(&primes);
    let expected = oracle::poly_add(
        &oracle::lift(&c0, &primes),
        &oracle::poly_mul(
            &oracle::lift(&c1, &primes),
            &oracle::small(&secret),
            &modulus,
        ),
        &modulus,
    );
    for (limb, &prime) in primes.iter().enumerate() {
        for (at, expected) in expected.iter().enumerate() {
            assert_eq!(
                direct_phase_residue(
                    &c0[limb * degree..(limb + 1) * degree],
                    &c1[limb * degree..(limb + 1) * degree],
                    &secret,
                    prime,
                    at
                ),
                oracle::word_mod(expected, prime)
            );
        }
    }
}

#[test]
#[ignore = "genuine unselected N65536 scalar bootstrap and repeated square/refresh; exact key residues about 5 GiB; explicitly scheduled native diagnostic"]
fn scalar_q23_fullsize_honest_bootstrap_and_repeat_preserve_all_messages_and_phase_bounds() {
    let started = Instant::now();
    let profile =
        BgvProfile::new(BgvParameters::derive(16, 257, 3, (60, 23), (60, 2), 2).unwrap()).unwrap();
    let effective = profile.plaintext_precision(1).unwrap();
    let encoding = SlotEncoding::new(&profile, effective).unwrap();
    assert_eq!(profile.degree(), 65_536);
    assert_eq!(profile.parameters().main_primes.len(), 23);
    assert_eq!(profile.special_count(), 2);
    assert_eq!(profile.digit_count(), 12);
    assert_eq!(encoding.slots(), SLOTS);
    assert_eq!(encoding.stride(), 512);
    assert_eq!(encoding.root(), 3);
    let fresh = profile.plan_fresh(effective).unwrap();
    assert_eq!(fresh.level(), 22);
    assert_eq!(fresh.bound(), &Natural::from_u64(46_723_127_394_197));
    let source = profile.plan_switch_to_level(&fresh, 0).unwrap();
    assert_eq!(source.level(), 0);
    assert_eq!(source.bound(), &Natural::from_u64(556_231_950_720));
    let plan = BootstrapPlan::new(
        &profile,
        &source,
        BootstrapMessageSpace::ScalarSlots,
        program(),
        &LEVELS,
        budget(),
    )
    .unwrap();
    assert_eq!(plan.message_space(), BootstrapMessageSpace::ScalarSlots);
    assert_eq!(plan.input_shape(), &source);
    assert_eq!(
        plan.homomorphic_input_shape().plaintext_precision(),
        profile.master_plaintext()
    );
    assert_eq!(
        plan.homomorphic_input_shape().bound(),
        &Natural::from_words_le(&[10_456_878_399_539_478_912, 1_408_839])
    );
    assert_eq!(plan.output_shape().level(), 4);
    assert_eq!(
        plan.output_shape().bound(),
        &Natural::from_words_le(&[
            12_424_068_366_968_892_569,
            15_571_601_069_614_700_364,
            9_643_227_763_710_943,
        ])
    );
    assert_eq!(plan.payload().canonical_arithmetic_units, 24_584_388_608);
    assert_eq!(plan.payload().scalar_word_operations, 1_075_380_224);
    assert_eq!(plan.payload().stream_owned_residue_bytes, 409_993_216);
    let lower = profile
        .plan_switch_to_level(plan.output_shape(), 1)
        .unwrap();
    assert_eq!(lower.level(), 1);
    assert_eq!(lower.bound(), &Natural::from_u64(556_231_950_737));
    let square = profile.plan_multiply(&lower, &lower).unwrap();
    assert_eq!(square.level(), 1);
    assert_eq!(
        square.bound(),
        &Natural::from_words_le(&[11_366_334_896_302_408_044, 1_099_188_235])
    );
    let repeat_source = profile.plan_switch_down(&square).unwrap();
    assert_eq!(repeat_source.level(), 0);
    assert_eq!(repeat_source.bound(), &Natural::from_u64(591_403_102_513));
    let repeat = plan
        .admit_source(&profile, &repeat_source, budget())
        .unwrap();
    assert!(plan.shares_preparation(&repeat));
    assert_eq!(repeat.message_space(), BootstrapMessageSpace::ScalarSlots);
    assert_eq!(repeat.output_shape(), plan.output_shape());
    assert_eq!(
        repeat.homomorphic_input_shape(),
        plan.homomorphic_input_shape()
    );
    assert_ne!(repeat.input_shape(), plan.input_shape());
    for shape in [&source, &repeat_source] {
        let modulus = profile.level_modulus(shape.level()).unwrap();
        assert!(
            shape.bound().mul_u64(132_098).add(&modulus.mul_u64(65_537)) < modulus.mul_u64(66_049),
            "full dense-secret strict carry"
        );
    }
    let mut galois = plan.galois_elements().to_vec();
    galois.extend_from_slice(repeat.galois_elements());
    galois.sort_unstable();
    galois.dedup();
    assert_eq!(galois.len(), 16);
    let setup = setup_budget(&profile, &galois);
    assert_eq!(setup.retained_residue_bytes, 5_395_972_096);
    assert_eq!(setup.returned_secret_bytes, 13_631_488);
    assert_eq!(setup.plaintext_bytes, 524_288);
    for (case, resource) in [
        "setup retained residues",
        "setup returned secret",
        "setup plaintext",
    ]
    .into_iter()
    .enumerate()
    {
        let mut short = setup;
        match case {
            0 => short.retained_residue_bytes -= 1,
            1 => short.returned_secret_bytes -= 1,
            _ => short.plaintext_bytes -= 1,
        }
        assert!(
            matches!(LocalBootstrapKey::generate(&profile, &galois, short),
            Err(BootstrapError::Budget { resource: actual }) if actual == resource)
        );
    }
    let key_started = Instant::now();
    let (secret, key) = LocalBootstrapKey::generate(&profile, &galois, setup).unwrap();
    eprintln!(
        "scalar_fullsize_setup retained={} secret={} plaintext={} key_ms={}",
        setup.retained_residue_bytes,
        setup.returned_secret_bytes,
        setup.plaintext_bytes,
        key_started.elapsed().as_millis()
    );
    let slots: [u64; SLOTS] = std::array::from_fn(|i| u64::try_from(i + 1).unwrap());
    let squared_slots = slots.map(|value| value * value % 257);
    for values in [&slots, &squared_slots] {
        let mut distinct = values.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), SLOTS);
    }
    let plaintext = encoding.encode(&profile, &slots).unwrap();
    assert_eq!(
        plaintext.coefficients(),
        expected_coefficients(profile.degree(), &slots)
    );
    let encrypted = key.encrypt_scalar(&profile, &encoding, &plaintext).unwrap();
    assert_eq!(encrypted.shape(), fresh);
    check_phase(&profile, &secret, encrypted.ciphertext(), &slots, "fresh22");
    let input = key.switch_scalar(&profile, encrypted, 0).unwrap();
    assert_eq!(input.shape(), source);
    check_phase(&profile, &secret, input.ciphertext(), &slots, "source0");
    let output = key.refresh_scalar(&profile, &plan, input).unwrap();
    assert_eq!(&output.shape(), plan.output_shape());
    check_phase(&profile, &secret, output.ciphertext(), &slots, "refresh4");
    let lowered = key.switch_scalar(&profile, output, 1).unwrap();
    assert_eq!(lowered.shape(), lower);
    check_phase(&profile, &secret, lowered.ciphertext(), &slots, "lower1");
    let product = key.square_scalar(&profile, lowered).unwrap();
    assert_eq!(product.shape(), square);
    check_phase(
        &profile,
        &secret,
        product.ciphertext(),
        &squared_slots,
        "square1",
    );
    let repeated_input = key.switch_scalar(&profile, product, 0).unwrap();
    assert_eq!(repeated_input.shape(), repeat_source);
    check_phase(
        &profile,
        &secret,
        repeated_input.ciphertext(),
        &squared_slots,
        "repeat_source0",
    );
    let repeated = key
        .refresh_scalar(&profile, &repeat, repeated_input)
        .unwrap();
    assert_eq!(&repeated.shape(), repeat.output_shape());
    check_phase(
        &profile,
        &secret,
        repeated.ciphertext(),
        &squared_slots,
        "repeat_refresh4",
    );
    eprintln!(
        "scalar_fullsize_complete bootstraps=2 full_crt_checkpoints=7 sampled_convolution_probes=135 total_ms={}",
        started.elapsed().as_millis()
    );
}
