//! Public streamed-bootstrap budget and every-basis regression with independent CRT phases.
//!
//! These OS-entropy, p5/E3/N16 arithmetic fixtures qualify neither production
//! parameters nor external input/key proofs, circuit privacy or resource ledgers.
//! No runtime decryption or raw-ciphertext provenance constructor is used.
mod support;

use iroha_fhe::{
    bgv::{
        BgvParameters, BgvProfile, BootstrapBudget, BootstrapError, BootstrapPlan, Ciphertext,
        CoefficientSlotBudget, LocalBootstrapKey, LocalBootstrapSetupBudget, Plaintext, SecretKey,
        SlotLinearBudget, UpperDigitsBudget, fixtures,
    },
    natural::Natural,
    prime_power::{
        PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan, UpperDigitsProgram,
    },
};
use num_bigint::{BigInt, Sign};
use support::oracle;
use zeroize::Zeroizing;

const LEVELS: [usize; 6] = [3, 3, 3, 2, 1, 1];
const OUTPUT_BOUND: u64 = 4_710_378_958_336;

fn profile() -> BgvProfile {
    BgvProfile::new(
        BgvParameters::derive(4, 5, 3, (50, 5), (60, 2), 2)
            .expect("unselected p5 arithmetic geometry"),
    )
    .expect("master profile")
}

fn program() -> UpperDigitsProgram {
    let context = PrimePowerContext::new(5, 3).expect("p5 cubed");
    let geometry = UpperDigitsPlan::required(context, 1).expect("complete tableau geometry");
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
    .expect("exact public polynomial/program budget");
    UpperDigitsProgram::prepare(&plan).expect("immutable upper-digit program")
}

fn budget() -> BootstrapBudget {
    let transform = CoefficientSlotBudget {
        routing: SlotLinearBudget {
            coefficient_bytes: 1024,
            layout_metadata_bytes: 4096,
            preparation_scratch_bytes: 4096,
            output_residue_bytes: 1 << 20,
            scratch_residue_bytes: 1 << 20,
        },
        owned_residue_bytes: 1 << 20,
        trace_plaintext_bytes: 4096,
    };
    BootstrapBudget {
        decomposition: transform,
        recomposition: transform,
        upper_digits: UpperDigitsBudget {
            compiler_steps: 1_000_000,
            metadata_bytes: 1 << 20,
            owned_residue_bytes: 1 << 20,
            plaintext_bytes: 4096,
            backend_scratch_bytes: 1 << 20,
            canonical_arithmetic_units: 1_000_000_000,
            scalar_word_operations: 1_000_000_000,
        },
        retained_preparation_bytes: 1 << 22,
        input_owned_bytes: 1 << 20,
        input_plaintext_bytes: 4096,
        input_arithmetic_units: 1_000_000_000,
        input_addition_words: 1_000_000,
        stream_owned_residue_bytes: 1 << 20,
        stream_metadata_bytes: 1 << 20,
        canonical_arithmetic_units: 10_000_000_000,
        scalar_word_operations: 10_000_000_000,
    }
}

fn setup_budget() -> LocalBootstrapSetupBudget {
    LocalBootstrapSetupBudget {
        retained_residue_bytes: 1 << 20,
        returned_secret_bytes: 1 << 20,
        plaintext_bytes: 4096,
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
    oracle::poly_mul(&polynomial, &polynomial, &oracle::big(5))
        .iter()
        .map(|value| oracle::word_mod(value, 5))
        .collect()
}

#[test]
fn exact_stream_budgets_and_every_full_ring_basis_preserve_physical_phases() {
    let profile = profile();
    let precision = profile
        .plaintext_precision(1)
        .expect("effective message precision");
    let fresh = profile.plan_fresh(precision).expect("fresh source shape");
    let diagnostic = BootstrapPlan::new(&profile, &fresh, program(), &LEVELS, budget())
        .expect("complete public pipeline before any encryption or key generation");
    let payload = diagnostic.payload();
    assert!(payload.stream_owned_residue_bytes > 0);
    assert!(payload.stream_metadata_bytes > 0);
    assert!(payload.canonical_arithmetic_units > 0);
    assert!(payload.scalar_word_operations > 0);
    let mut exact = budget();
    exact.stream_owned_residue_bytes = payload.stream_owned_residue_bytes;
    exact.stream_metadata_bytes = payload.stream_metadata_bytes;
    exact.canonical_arithmetic_units = payload.canonical_arithmetic_units;
    exact.scalar_word_operations = payload.scalar_word_operations;
    for resource in 0..4 {
        let mut short = exact;
        match resource {
            0 => short.stream_owned_residue_bytes -= 1,
            1 => short.stream_metadata_bytes -= 1,
            2 => short.canonical_arithmetic_units -= 1,
            _ => short.scalar_word_operations -= 1,
        }
        assert!(matches!(
            BootstrapPlan::new(&profile, &fresh, program(), &LEVELS, short),
            Err(BootstrapError::Budget { .. })
        ));
    }
    let plan = BootstrapPlan::new(&profile, &fresh, program(), &LEVELS, exact)
        .expect("exact stream and aggregate work ceilings admit the complete plan");
    assert_eq!(plan.payload(), payload);
    assert_eq!(plan.output_shape(), diagnostic.output_shape());
    assert_eq!(plan.galois_elements(), diagnostic.galois_elements());
    let square_shape = profile
        .plan_multiply(plan.output_shape(), plan.output_shape())
        .expect("actual squared-output shape");
    let repeat = BootstrapPlan::new(&profile, &square_shape, program(), &LEVELS, budget())
        .expect("complete repeated bootstrap");
    assert!(repeat.reduces_phase_bound());
    assert_eq!(
        repeat.output_shape().bound(),
        &Natural::from_u64(OUTPUT_BOUND)
    );
    let mut galois = plan.galois_elements().to_vec();
    galois.extend_from_slice(repeat.galois_elements());
    galois.sort_unstable();
    galois.dedup();
    let (secret, key) = LocalBootstrapKey::generate(&profile, &galois, setup_budget())
        .expect("honest local setup; separate secret retained only by this test oracle");
    let n = profile.degree();
    for case in 0..n + 3 {
        let expected: Vec<u64> = (0..n)
            .map(|j| match case {
                i if i < n => u64::from(j == i),
                i if i == n => 0,
                i if i == n + 1 => 4,
                _ => 4 * (j % 2) as u64,
            })
            .collect();
        let plaintext = Plaintext::new(&profile, precision, expected.clone())
            .expect("full-ring basis or dense endpoint coefficients");
        let source = key
            .encrypt(&profile, &plaintext)
            .expect("honest local encryption");
        check_phase(&profile, &secret, source.ciphertext(), &expected);
        let refreshed = key
            .refresh(&profile, &plan, source)
            .expect("stream every coefficient bank with exact public ceilings");
        assert_eq!(&refreshed.shape(), plan.output_shape());
        check_phase(&profile, &secret, refreshed.ciphertext(), &expected);
        let squared = key
            .square(&profile, refreshed)
            .expect("canonical local square");
        let expected_square = clear_square(&expected);
        check_phase(&profile, &secret, squared.ciphertext(), &expected_square);
        let refreshed = key
            .refresh(&profile, &repeat, squared)
            .expect("repeat through the same single streaming implementation");
        assert_eq!(&refreshed.shape(), repeat.output_shape());
        check_phase(&profile, &secret, refreshed.ciphertext(), &expected_square);
    }
}
