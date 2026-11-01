//! Genuine bounded F257/E3 encrypted bootstrap and repeated square/refresh.
//!
//! The unselected N256 fixture exercises both coefficient banks under one
//! honestly generated local key. Independent `BigInt` CRT/convolution checks
//! every message coefficient and phase bound. This is not a production profile,
//! full-size N65536 execution, source authentication or security qualification.
mod support;

use iroha_fhe::{
    bgv::{
        BgvParameters, BgvProfile, BootstrapBudget, BootstrapPlan, Ciphertext,
        CoefficientSlotBudget, LocalBootstrapKey, LocalBootstrapSetupBudget, Plaintext, SecretKey,
        SlotEncoding, SlotLinearBudget, UpperDigitsBudget, fixtures,
    },
    natural::Natural,
    prime_power::{
        PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan, UpperDigitsProgram,
    },
};
use num_bigint::{BigInt, Sign};
use support::oracle;
use zeroize::Zeroizing;

const LEVELS: [usize; 18] = [
    19, 18, 17, 16, 15, 14, 13, 12, 12, 10, 9, 8, 7, 6, 5, 4, 3, 3,
];

fn program() -> UpperDigitsProgram {
    let context = PrimePowerContext::new(257, 3).unwrap();
    let required = UpperDigitsPlan::required(context, 1).unwrap();
    let plan = UpperDigitsPlan::new(
        context,
        1,
        ProgramBudget {
            coefficient_bytes: required.coefficient_bytes,
            preparation_scratch_bytes: required.preparation_scratch_bytes,
            program_bytes: required.program_bytes,
            preparation_operations: required.preparation_operations,
            live_values: required.live_values,
        },
    )
    .unwrap();
    UpperDigitsProgram::prepare(&plan).unwrap()
}

fn budget() -> BootstrapBudget {
    let decomposition = CoefficientSlotBudget {
        routing: SlotLinearBudget {
            coefficient_bytes: 128 * 129 * 4,
            layout_metadata_bytes: 16 * 1024,
            preparation_scratch_bytes: 129 * 4,
            output_residue_bytes: 1 << 20,
            // Finite public cap admits the witnessed B8/G16 split.
            scratch_residue_bytes: 2 << 20,
        },
        owned_residue_bytes: 8 << 20,
        trace_plaintext_bytes: 256 * 8,
    };
    let mut recomposition = decomposition;
    recomposition.routing.scratch_residue_bytes = 256 << 10;
    BootstrapBudget {
        decomposition,
        recomposition,
        upper_digits: UpperDigitsBudget {
            compiler_steps: 1_000_000,
            metadata_bytes: 16 << 20,
            owned_residue_bytes: 8 << 20,
            plaintext_bytes: 256 * 8,
            backend_scratch_bytes: 8 << 20,
            canonical_arithmetic_units: 1_u64 << 40,
            scalar_word_operations: 1_u64 << 36,
        },
        retained_preparation_bytes: 32 << 20,
        input_owned_bytes: 1 << 20,
        input_plaintext_bytes: 4096,
        input_arithmetic_units: 1_u64 << 32,
        input_addition_words: 1_u64 << 24,
        stream_owned_residue_bytes: 1_622_016,
        stream_metadata_bytes: 1 << 20,
        // Independent successor replay: only public-scalar multiply work changes.
        canonical_arithmetic_units: 102_490_880,
        scalar_word_operations: 7_559_680,
    }
}

fn integer(value: &Natural) -> BigInt {
    let bytes: Vec<_> = value
        .words_le()
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    BigInt::from_bytes_le(Sign::Plus, &bytes)
}

/// Independently reconstruct c0+c1*s over Z[X]/(X^N+1), then center modulo Q.
fn check_phase(
    profile: &BgvProfile,
    secret: &SecretKey,
    ciphertext: &Ciphertext,
    expected: &[u64],
) {
    assert_eq!(ciphertext.component_count(), 2);
    assert_eq!(expected.len(), profile.degree());
    let primes = &profile.parameters().main_primes[..=ciphertext.level()];
    let modulus = oracle::product(primes);
    let bound = integer(ciphertext.noise_bound().as_natural());
    assert!(
        &bound * 2_u8 < modulus,
        "declared phase fits the actual modulus"
    );
    let component_words = profile.degree() * primes.len();
    let words = Zeroizing::new(ciphertext.canonical_words());
    assert_eq!(words.len(), 2 * component_words);
    let c0 = oracle::lift(&words[..component_words], primes);
    let c1 = oracle::lift(&words[component_words..], primes);
    let coefficients = Zeroizing::new(fixtures::secret_coefficients(secret));
    let secret_polynomial = oracle::small(&coefficients);
    let values = oracle::poly_add(
        &c0,
        &oracle::poly_mul(&c1, &secret_polynomial, &modulus),
        &modulus,
    );
    for (value, &wanted) in values.iter().zip(expected) {
        let value = oracle::centered(value, &modulus);
        assert!(-&bound <= value && value <= bound, "actual phase bound");
        assert_eq!(
            oracle::word_mod(&value, ciphertext.plaintext_precision().modulus()),
            wanted,
            "full-ring coefficient from independent CRT and convolution"
        );
    }
}

fn clear_square(values: &[u64]) -> Vec<u64> {
    let polynomial: Vec<_> = values.iter().map(|&value| oracle::big(value)).collect();
    oracle::poly_mul(&polynomial, &polynomial, &oracle::big(257))
        .iter()
        .map(|value| oracle::word_mod(value, 257))
        .collect()
}

#[test]
#[ignore = "genuine unselected N256 F257 bootstrap; run explicitly with the scoped native batch"]
fn f257_both_banks_preserve_full_ring_phases_through_two_square_refresh_cycles() {
    let profile =
        BgvProfile::new(BgvParameters::derive(8, 257, 3, (50, 22), (60, 2), 2).unwrap()).unwrap();
    assert_eq!(
        profile.parameters().main_primes.as_slice(),
        &[
            562_967_672_918_017,
            563_019_818_867_713,
            563_202_329_691_649,
            563_619_497_289_217,
            563_758_553_155_073,
            564_140_956_786_177,
            564_488_596_450_817,
            564_610_270_333_441,
            565_027_437_931_009,
            565_218_639_746_561,
            565_270_785_696_257,
            565_349_004_620_801,
            565_418_532_553_729,
            565_653_189_327_361,
            565_740_099_243_521,
            565_757_481_226_753,
            565_766_172_218_369,
            565_844_391_142_913,
            565_939_992_050_689,
            566_105_120_891_393,
            566_261_558_740_481,
            566_417_996_589_569
        ]
    );
    assert_eq!(
        profile.parameters().special_primes.as_slice(),
        &[576_460_785_041_520_641, 576_460_828_496_478_721]
    );
    let precision = profile.plaintext_precision(1).unwrap();
    assert_eq!(SlotEncoding::new(&profile, precision).unwrap().slots(), 128);
    assert_eq!(profile.degree() / 128, 2);
    let fresh = profile.plan_fresh(precision).unwrap();
    let source = profile.plan_switch_to_level(&fresh, 0).unwrap();
    assert_eq!(source.bound(), &Natural::from_u64(2_181_235_200));
    let plan = BootstrapPlan::new(
        &profile,
        &source,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        program(),
        &LEVELS,
        budget(),
    )
    .unwrap();
    assert_eq!(
        plan.galois_elements(),
        &[5, 25, 113, 257, 385, 449, 481, 511]
    );
    assert_eq!(
        plan.homomorphic_input_shape().bound(),
        &Natural::from_words_le(&[9_961_313_701_462_081_920_u64, 21_u64])
    );
    assert_eq!(
        plan.output_shape().bound(),
        &Natural::from_words_le(&[6_045_943_472_966_474_940_u64, 1_210_605_716_911_226_152_u64])
    );
    assert_eq!(plan.output_shape().level(), 3);
    assert_eq!(plan.payload().stream_owned_residue_bytes, 1_622_016);
    assert_eq!(plan.payload().canonical_arithmetic_units, 102_490_880);
    assert_eq!(plan.payload().scalar_word_operations, 7_559_680);
    let at_two = profile.plan_switch_down(plan.output_shape()).unwrap();
    assert_eq!(
        at_two.bound(),
        &Natural::from_words_le(&[16_844_221_733_282_439_330_u64, 2147_u64])
    );
    let at_one = profile.plan_switch_down(&at_two).unwrap();
    assert_eq!(at_one.bound(), &Natural::from_u64(2_251_586_478));
    let squared = profile.plan_multiply(&at_one, &at_one).unwrap();
    assert_eq!(
        squared.bound(),
        &Natural::from_words_le(&[6_556_181_828_098_453_505_u64, 70_u64])
    );
    let repeat_input = profile.plan_switch_down(&squared).unwrap();
    assert_eq!(repeat_input.bound(), &Natural::from_u64(2_183_540_320));
    let repeat = BootstrapPlan::new(
        &profile,
        &repeat_input,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        program(),
        &LEVELS,
        budget(),
    )
    .unwrap();
    assert_eq!(repeat.output_shape(), plan.output_shape());
    assert_eq!(
        repeat.homomorphic_input_shape(),
        plan.homomorphic_input_shape()
    );
    assert_eq!(repeat.payload(), plan.payload());
    for shape in [
        &source,
        plan.output_shape(),
        &at_one,
        &squared,
        &repeat_input,
    ] {
        let q = profile.level_modulus(shape.level()).unwrap();
        assert!(shape.bound().mul_u64(2 * 66_049).add(&q.mul_u64(257)) < q.mul_u64(66_049));
    }
    let (secret, key) = LocalBootstrapKey::generate(
        &profile,
        plan.galois_elements(),
        LocalBootstrapSetupBudget {
            retained_residue_bytes: 9_912_320,
            returned_secret_bytes: 51_200,
            plaintext_bytes: 2048,
        },
    )
    .expect("honest OS-entropy key generation after complete public admission");
    let mut edges = vec![0; profile.degree()];
    edges[1] = 1;
    edges[255] = 256;
    let mut dense: Vec<u64> = (0..256).collect();
    dense[0] = 256;
    // Zero plus dense cover every field residue and both banks. The edge message
    // spans both ends of the ring so squaring exercises negacyclic wraparound.
    for mut expected in [vec![0; profile.degree()], edges, dense] {
        let plaintext = Plaintext::new(&profile, precision, expected.clone()).unwrap();
        let value = key.encrypt(&profile, &plaintext).unwrap();
        assert_eq!(value.shape(), fresh);
        check_phase(&profile, &secret, value.ciphertext(), &expected);
        let value = key.switch_to(&profile, value, 0).unwrap();
        assert_eq!(value.shape(), source);
        check_phase(&profile, &secret, value.ciphertext(), &expected);
        let mut value = key.refresh(&profile, &plan, value).unwrap();
        assert_eq!(&value.shape(), plan.output_shape());
        check_phase(&profile, &secret, value.ciphertext(), &expected);
        for _ in 0..2 {
            value = key.switch_to(&profile, value, 2).unwrap();
            assert_eq!(value.shape(), at_two);
            check_phase(&profile, &secret, value.ciphertext(), &expected);
            value = key.switch_to(&profile, value, 1).unwrap();
            assert_eq!(value.shape(), at_one);
            check_phase(&profile, &secret, value.ciphertext(), &expected);
            expected = clear_square(&expected);
            value = key.square(&profile, value).unwrap();
            assert_eq!(value.shape(), squared);
            check_phase(&profile, &secret, value.ciphertext(), &expected);
            value = key.switch_to(&profile, value, 0).unwrap();
            assert_eq!(value.shape(), repeat_input);
            check_phase(&profile, &secret, value.ciphertext(), &expected);
            value = key.refresh(&profile, &repeat, value).unwrap();
            assert_eq!(&value.shape(), plan.output_shape());
            check_phase(&profile, &secret, value.ciphertext(), &expected);
        }
    }
}
