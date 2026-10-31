//! Public local-bootstrap API regression with independent full-ring CRT phases.
//!
//! These OS-entropy, p5/E3/N16 arithmetic fixtures qualify neither production
//! parameters nor external input/key proofs, circuit privacy or resource ledgers.
//! No runtime decryption or raw-ciphertext provenance constructor is used.
mod support;

use iroha_fhe::{
    bgv::{
        BgvParameters, BgvProfile, BootstrapBudget, BootstrapError, BootstrapPlan, Ciphertext,
        CoefficientSlotBudget, LocalBootstrapKey, LocalBootstrapSetupBudget, Plaintext, SecretKey,
        SlotEncoding, SlotLinearBudget, SlotLinearLayout, SlotLinearMap, SlotLinearPlan,
        SlotLinearSegment, UpperDigitsBudget, fixtures,
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
        canonical_arithmetic_units: 1_000_000_000_000,
        scalar_word_operations: 1_000_000_000_000,
        stream_owned_residue_bytes: 1 << 20,
        stream_metadata_bytes: 1 << 20,
        retained_preparation_bytes: 1 << 22,
        input_owned_bytes: 1 << 20,
        input_plaintext_bytes: 4096,
        input_arithmetic_units: 1_000_000_000,
        input_addition_words: 1_000_000,
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
fn public_api_preserves_zero_x_dense_and_three_square_refresh_cycles() {
    let profile = profile();
    let precision = profile.plaintext_precision(1).expect("message precision");
    let fresh = profile.plan_fresh(precision).expect("fresh shape");
    let source_levels = [4, 2, 1];
    let plans: Vec<_> = source_levels
        .iter()
        .map(|&level| {
            let shape = profile
                .plan_switch_to_level(&fresh, level)
                .expect("public lowering");
            BootstrapPlan::new(&profile, &shape, program(), &LEVELS, budget())
                .expect("first honest refresh plan")
        })
        .collect();
    let square_shape = profile
        .plan_multiply(plans[0].output_shape(), plans[0].output_shape())
        .expect("same-level square after bootstrap");
    let repeat = BootstrapPlan::new(&profile, &square_shape, program(), &LEVELS, budget())
        .expect("repeat plan from the exact squared bound");
    assert!(repeat.reduces_phase_bound());
    assert_eq!(
        repeat.output_shape().bound(),
        &Natural::from_u64(OUTPUT_BOUND)
    );
    let mut galois = Vec::new();
    for plan in plans.iter().chain(core::iter::once(&repeat)) {
        assert_eq!(
            plan.homomorphic_input_shape().bound(),
            &Natural::from_u64(85_993_566)
        );
        galois.extend_from_slice(plan.galois_elements());
    }
    galois.sort_unstable();
    galois.dedup();
    let (secret, key) = LocalBootstrapKey::generate(&profile, &galois, setup_budget())
        .expect("honest local key and separate secret custody from OS entropy");
    let mut x = vec![0_u64; profile.degree()];
    x[1] = 1;
    let dense: Vec<u64> = (0..profile.degree())
        .map(|i| ((3 * i + i / 3) % 4 + 1) as u64)
        .collect();
    let cases = [vec![0; profile.degree()], x, dense];
    for ((mut expected, &level), plan) in cases.into_iter().zip(&source_levels).zip(&plans) {
        let plaintext =
            Plaintext::new(&profile, precision, expected.clone()).expect("full-ring message");
        let value = key.encrypt(&profile, &plaintext).expect("local encryption");
        assert_eq!(value.shape(), fresh);
        check_phase(&profile, &secret, value.ciphertext(), &expected);
        let value = key
            .switch_to(&profile, value, level)
            .expect("public source lowering");
        assert_eq!(&value.shape(), plan.input_shape());
        check_phase(&profile, &secret, value.ciphertext(), &expected);
        assert!(
            !plan.reduces_phase_bound(),
            "the first refresh is allowed to increase a fresh bound"
        );
        let mut value = key
            .refresh(&profile, plan, value)
            .expect("complete first bootstrap");
        assert_eq!(&value.shape(), plan.output_shape());
        assert_eq!(value.shape().bound(), &Natural::from_u64(OUTPUT_BOUND));
        assert_eq!(value.shape().level(), 1);
        assert_eq!(value.shape().plaintext_precision(), precision);
        check_phase(&profile, &secret, value.ciphertext(), &expected);
        for _ in 0..3 {
            expected = clear_square(&expected);
            value = key.square(&profile, value).expect("canonical local square");
            assert_eq!(&value.shape(), repeat.input_shape());
            check_phase(&profile, &secret, value.ciphertext(), &expected);
            let squared_bound = value.shape().bound().clone();
            value = key
                .refresh(&profile, &repeat, value)
                .expect("complete repeated bootstrap");
            let shape = value.shape();
            assert!(
                shape.bound() < &squared_bound,
                "each square-refresh strictly reduces its bound"
            );
            assert_eq!(shape.bound(), &Natural::from_u64(OUTPUT_BOUND));
            assert_eq!(shape.level(), 1);
            assert_eq!(shape.plaintext_precision(), precision);
            check_phase(&profile, &secret, value.ciphertext(), &expected);
        }
    }
}

#[test]
fn public_api_rejects_same_profile_foreign_key_origins() {
    let profile = profile();
    let precision = profile.plaintext_precision(1).expect("message precision");
    let fresh = profile.plan_fresh(precision).expect("fresh shape");
    let plan = BootstrapPlan::new(&profile, &fresh, program(), &LEVELS, budget()).expect("plan");
    let (secret, first) =
        LocalBootstrapKey::generate(&profile, plan.galois_elements(), setup_budget())
            .expect("first local key owner");
    let (_, other) = LocalBootstrapKey::generate(&profile, plan.galois_elements(), setup_budget())
        .expect("independent same-profile key owner");
    let plaintext =
        Plaintext::new(&profile, precision, vec![1; profile.degree()]).expect("message");
    let left = first
        .encrypt(&profile, &plaintext)
        .expect("first same-origin operand");
    let right = first
        .encrypt(&profile, &plaintext)
        .expect("second same-origin operand");
    let product = first
        .multiply(&profile, left, right)
        .expect("same-origin multiplication");
    check_phase(
        &profile,
        &secret,
        product.ciphertext(),
        &clear_square(&vec![1; profile.degree()]),
    );
    let source = first.encrypt(&profile, &plaintext).expect("source");
    assert_eq!(
        other.refresh(&profile, &plan, source).unwrap_err(),
        BootstrapError::Origin
    );
    for foreign_left in [false, true] {
        let local = first.encrypt(&profile, &plaintext).expect("local source");
        let foreign = other.encrypt(&profile, &plaintext).expect("foreign source");
        let result = if foreign_left {
            first.multiply(&profile, foreign, local)
        } else {
            first.multiply(&profile, local, foreign)
        };
        assert_eq!(result.unwrap_err(), BootstrapError::Origin);
    }
}

/// Independent inverse of the two-point negacyclic scalar evaluation modulo 5.
fn scalar_coefficients(root: u64, values: [u64; 2]) -> Vec<u64> {
    assert_eq!(root * root % 5, 4);
    let inverse = (1..5).find(|x| (2 * root * x) % 5 == 1).unwrap();
    let mut coefficients = vec![0; 16];
    coefficients[0] = (values[0] + values[1]) * 3 % 5;
    coefficients[8] = (values[0] + 5 - values[1]) * inverse % 5;
    coefficients
}

#[test]
fn public_route_preserves_origin_through_mixed_bounds_and_repeated_refresh() {
    let profile = profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let encoding = SlotEncoding::new(&profile, precision).unwrap();
    assert_eq!(encoding.slots(), 2);
    let fresh = profile.plan_fresh(precision).unwrap();
    let squared = profile.plan_multiply(&fresh, &fresh).unwrap();
    // Bank 0 is bias only, bank 1 selects the first input, bank 2 mixes both.
    // Public support is fixed before any private message or map values exist.
    let mut segments = Vec::new();
    let mut offsets = vec![0];
    for row in 0..6 {
        match row / 2 {
            0 => segments.push(SlotLinearSegment::new(4, 1, 1)),
            1 => segments.push(SlotLinearSegment::new(row % 2, 1, 1)),
            _ => segments.push(SlotLinearSegment::new(0, 5, 1)),
        }
        offsets.push(segments.len());
    }
    let layout = SlotLinearLayout::<2, 3>::new(2, offsets, segments, 4096).unwrap();
    let route = SlotLinearPlan::with_layout(
        &profile,
        &[fresh.clone(), squared.clone()],
        layout,
        budget().decomposition.routing,
    )
    .unwrap();
    assert_ne!(route.output_shapes()[0], route.output_shapes()[1]);
    assert_ne!(route.output_shapes()[1], route.output_shapes()[2]);
    let map = SlotLinearMap::prepare_rows(&route, |row, values| {
        match row / 2 {
            0 => values[4] = (row + 1) as u32,
            1 => values[row % 2] = 1,
            _ => values.copy_from_slice(if row % 2 == 0 {
                &[1, 2, 3, 4, 1]
            } else {
                &[4, 3, 2, 1, 2]
            }),
        }
        Ok(())
    })
    .unwrap();
    let plans: Vec<_> = route
        .output_shapes()
        .iter()
        .map(|shape| BootstrapPlan::new(&profile, shape, program(), &LEVELS, budget()).unwrap())
        .collect();
    let repeat_shape = profile
        .plan_multiply(plans[0].output_shape(), plans[0].output_shape())
        .unwrap();
    let repeat = BootstrapPlan::new(&profile, &repeat_shape, program(), &LEVELS, budget()).unwrap();
    assert!(repeat.reduces_phase_bound());
    let mut galois = route.galois_elements().to_vec();
    for plan in plans.iter().chain(core::iter::once(&repeat)) {
        galois.extend_from_slice(plan.galois_elements());
    }
    galois.sort_unstable();
    galois.dedup();
    let (secret, key) = LocalBootstrapKey::generate(&profile, &galois, setup_budget()).unwrap();
    let input_bytes = [&fresh, &squared]
        .into_iter()
        .map(|shape| 2 * profile.degree() * (shape.level() + 1) * core::mem::size_of::<u64>())
        .sum::<usize>();
    let ceiling = input_bytes + route.output_residue_bytes() + route.scratch_residue_bytes();
    for (a, b) in [([0, 0], [0, 0]), ([1, 2], [3, 4])] {
        let pa = encoding.encode(&profile, &a).unwrap();
        let pb = encoding.encode(&profile, &b).unwrap();
        let lhs = key.encrypt(&profile, &pa).unwrap();
        let rhs = key
            .square(&profile, key.encrypt(&profile, &pb).unwrap())
            .unwrap();
        check_phase(
            &profile,
            &secret,
            lhs.ciphertext(),
            &scalar_coefficients(encoding.root(), a),
        );
        let b = [b[0] * b[0] % 5, b[1] * b[1] % 5];
        check_phase(
            &profile,
            &secret,
            rhs.ciphertext(),
            &scalar_coefficients(encoding.root(), b),
        );
        let input_words = [
            lhs.ciphertext().canonical_words(),
            rhs.ciphertext().canonical_words(),
        ];
        let (returned, outputs) = key
            .route(&profile, &route, &map, [lhs, rhs], ceiling)
            .unwrap();
        for (value, expected) in returned.iter().zip(&input_words) {
            assert_eq!(&value.ciphertext().canonical_words(), expected);
            key.preflight_evaluation(&profile, value, &value.shape(), route.galois_elements())
                .unwrap();
        }
        drop(returned);
        let expected_slots = [
            [1, 2],
            a,
            [
                (a[0] + 2 * a[1] + 3 * b[0] + 4 * b[1] + 1) % 5,
                (4 * a[0] + 3 * a[1] + 2 * b[0] + b[1] + 2) % 5,
            ],
        ];
        for (((output, plan), slots), shape) in outputs
            .into_iter()
            .zip(&plans)
            .zip(expected_slots)
            .zip(route.output_shapes())
        {
            assert_eq!(&output.shape(), shape);
            let mut expected = scalar_coefficients(encoding.root(), slots);
            check_phase(&profile, &secret, output.ciphertext(), &expected);
            let mut value = key.refresh(&profile, plan, output).unwrap();
            check_phase(&profile, &secret, value.ciphertext(), &expected);
            for _ in 0..2 {
                value = key.square(&profile, value).unwrap();
                expected = clear_square(&expected);
                check_phase(&profile, &secret, value.ciphertext(), &expected);
                value = key.refresh(&profile, &repeat, value).unwrap();
                check_phase(&profile, &secret, value.ciphertext(), &expected);
                assert_eq!(value.shape().bound(), &Natural::from_u64(OUTPUT_BOUND));
            }
        }
    }
}
