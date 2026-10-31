//! Independent ordered-output and full-ring phase coverage for bounded refresh.
//!
//! This unselected arithmetic fixture is not an R9 logical-refresh compiler,
//! a production profile, circuit privacy or a source/key/execution proof.

use super::*;
use crate::ram_lfe::{
    HiddenRamFheInstruction as Op, HiddenRamFheProgram, RamLfeClassV1, RamLfeReferenceInputV1,
    ram_lfe_reference_execute_v1, ram_lfe_reference_normalize_v1,
};
use iroha_fhe::{
    bgv::{
        BgvParameters, BootstrapBudget, Ciphertext, CoefficientSlotBudget,
        LocalBootstrapSetupBudget, SecretKey, UpperDigitsBudget, fixtures,
    },
    natural::Natural,
    prime_power::{PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan},
};
use num_bigint::{BigInt, Sign};
use zeroize::Zeroizing;

fn distinct_program() -> HiddenRamFheProgram {
    let mut instructions = vec![
        Op::LoadConst(3, 3),
        Op::Mul(3, 3, 3), // Dead, nonzero nonlinear gate keeps its original ordinal.
        Op::LoadInput(0, 1),
        Op::LoadState(1, 0),
        Op::LoadConst(2, 142),
        Op::LoadState(3, 31),
        Op::SelectEqZero(0, 1, 0, 2),
    ];
    instructions.extend(std::iter::repeat_n(Op::Mul(0, 0, 3), 6));
    instructions.extend([Op::StoreState(4, 0), Op::LoadState(0, 4)]);
    for index in 0..49 {
        instructions.push(Op::LoadConst(2, (index * 43 + 19) % 257));
    }
    for slot in 0..64 {
        instructions.extend([Op::LoadInput(1, slot), Op::Add(0, 0, 1), Op::Output(0)]);
    }
    assert_eq!(instructions.len(), 256);
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for instruction in instructions {
        builder.push(instruction).unwrap();
    }
    builder.finish().unwrap()
}

fn distinct_input() -> RamLfeReferenceInputV1 {
    RamLfeReferenceInputV1::from_bytes(&[1; 63]).unwrap()
}

fn distinct_memory() -> RamLfeInitialMemoryV1 {
    let mut lanes = std::array::from_fn::<_, 32, _>(|i| ((i * 19 + 197) % 257) as u16);
    lanes[0] = 256;
    lanes[31] = 7;
    RamLfeInitialMemoryV1::from_lanes(&lanes).unwrap()
}

fn expected_slots() -> [u64; 128] {
    // 7^6=200 and142*200=130 modulo257. Nonzero condition256 selects142.
    // The length slot contributes63, then each of63 byte slots contributes1.
    std::array::from_fn(|i| if i < 64 { 193 + i as u64 } else { 0 })
}

fn power(mut base: u64, mut exponent: usize) -> u64 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = result * base % 257;
        }
        base = base * base % 257;
        exponent >>= 1;
    }
    result
}

fn expected_coefficients(slots: &[u64; 128]) -> [u64; 128] {
    // Direct inverse Vandermonde over F257: beta_j=3^(2j+1),128^-1=255.
    // This uses neither the production SlotEncoding nor its inverse routine.
    std::array::from_fn(|k| {
        slots.iter().enumerate().fold(0, |sum, (j, &value)| {
            let beta = power(3, 2 * j + 1);
            let inverse_power = power(beta, (256 - k) % 256);
            (sum + value * inverse_power * 255) % 257
        })
    })
}

fn assert_independent_phase(
    profile: &BgvProfile,
    secret: &SecretKey,
    ciphertext: &Ciphertext,
    expected: &[u64; 128],
) {
    assert_eq!(profile.degree(), 128);
    assert_eq!(ciphertext.component_count(), 2);
    let primes = &profile.parameters().main_primes[..=ciphertext.level()];
    let q = primes.iter().fold(BigInt::from(1), |value, &p| value * p);
    let basis: Vec<_> = primes
        .iter()
        .map(|&p| {
            let partial = &q / p;
            let inverse = partial.modpow(&BigInt::from(p - 2), &BigInt::from(p));
            partial * inverse
        })
        .collect();
    let words = Zeroizing::new(ciphertext.canonical_words());
    let component_words = 128 * primes.len();
    assert_eq!(words.len(), component_words * 2);
    let lift = |component: usize, coefficient: usize| {
        basis
            .iter()
            .enumerate()
            .fold(BigInt::from(0), |sum, (limb, basis)| {
                sum + basis * words[component * component_words + limb * 128 + coefficient]
            })
            % &q
    };
    let c0: Vec<_> = (0..128).map(|i| lift(0, i)).collect();
    let c1: Vec<_> = (0..128).map(|i| lift(1, i)).collect();
    let coefficients = Zeroizing::new(fixtures::secret_coefficients(secret));
    let bound = ciphertext
        .noise_bound()
        .as_natural()
        .words_le()
        .iter()
        .rev()
        .fold(BigInt::from(0), |value, &word| (value << 64) + word);
    let half = (&q - 1_u32) / 2_u32;
    assert!(bound <= half);
    for i in 0..128 {
        let mut phase = c0[i].clone();
        for (j, &s) in coefficients.iter().enumerate() {
            if j <= i {
                phase += &c1[i - j] * i64::from(s);
            } else {
                phase -= &c1[i + 128 - j] * i64::from(s);
            }
        }
        let reduced = ((phase % &q) + &q) % &q;
        let centered = if reduced > half {
            reduced - &q
        } else {
            reduced
        };
        let magnitude = if centered.sign() == Sign::Minus {
            -&centered
        } else {
            centered.clone()
        };
        assert!(
            magnitude <= bound,
            "independent full-ring phase bound at{i}"
        );
        let message = ((&centered % 257_u32) + 257_u32) % 257_u32;
        assert_eq!(
            message,
            BigInt::from(expected[i]),
            "independent coefficient{i}"
        );
    }
}

#[test]
fn distinct_output_fixture_has_exact_original_shape_and_independent_clear_outputs() {
    let program = distinct_program();
    let input = distinct_input();
    let memory = distinct_memory();
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    assert_eq!(normal.report().peak_rank(), 16);
    assert_eq!(normal.gate_count(), 8);
    let reference =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Bounded, &program, &memory, &input).unwrap();
    let expected: Vec<_> = expected_slots()[..64].iter().map(|&v| v as u16).collect();
    assert_eq!(reference.output().scalars(), expected.as_slice());
    assert_eq!(
        normal
            .evaluate(&memory, &input)
            .unwrap()
            .padded_for_opening(),
        reference.output().padded_for_opening()
    );
    let coefficients = expected_coefficients(&expected_slots());
    for (j, &wanted) in expected_slots().iter().enumerate() {
        let beta = power(3, 2 * j + 1);
        let actual = coefficients
            .iter()
            .rev()
            .fold(0, |value, &c| (value * beta + c) % 257);
        assert_eq!(actual, wanted);
    }
}

fn physical_refresh_schedule() -> RamLfeBgvBoundedScheduleV1 {
    let context = PrimePowerContext::new(257, 3).unwrap();
    let geometry = UpperDigitsPlan::required(context, 1).unwrap();
    let program = UpperDigitsPlan::new(
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
    let transform = CoefficientSlotBudget {
        routing: SlotLinearBudget {
            coefficient_bytes: 128 * 129 * 4,
            layout_metadata_bytes: 16 << 10,
            preparation_scratch_bytes: 129 * 4,
            output_residue_bytes: 1 << 20,
            scratch_residue_bytes: 16 << 20,
        },
        owned_residue_bytes: 8 << 20,
        trace_plaintext_bytes: 128 * 8,
    };
    RamLfeBgvBoundedScheduleV1 {
        refresh: Some(RamLfeBgvBoundedRefreshChoiceV1 {
            source_level: 0,
            program,
            levels: vec![
                18, 17, 16, 15, 14, 13, 12, 11, 11, 10, 9, 8, 7, 6, 5, 4, 4, 4,
            ],
            budget: BootstrapBudget {
                decomposition: transform,
                recomposition: transform,
                upper_digits: UpperDigitsBudget {
                    compiler_steps: 1_000_000,
                    metadata_bytes: 16 << 20,
                    owned_residue_bytes: 8 << 20,
                    plaintext_bytes: 128 * 8,
                    backend_scratch_bytes: 8 << 20,
                    canonical_arithmetic_units: 1_u64 << 40,
                    scalar_word_operations: 1_u64 << 36,
                },
                retained_preparation_bytes: 32 << 20,
                input_owned_bytes: 1 << 20,
                input_plaintext_bytes: 2048,
                input_arithmetic_units: 1_u64 << 32,
                input_addition_words: 1_u64 << 24,
                stream_owned_residue_bytes: 696_320,
                stream_metadata_bytes: 1 << 20,
                canonical_arithmetic_units: 22_275_456,
                scalar_word_operations: 1_821_952,
            },
        }),
        layers: [RamLfeBgvBoundedLayerChoiceV1 {
            routing_level: 4,
            multiplication_level: 1,
            routing_scratch_bytes: 317_440,
        }; 16],
        projection_level: 4,
        mask_level: 0,
        projection_scratch_bytes: 317_440,
    }
}

#[test]
#[ignore = "genuine unselected N128 F257/E3 complete357-bootstrap executor; explicit scoped native batch"]
fn refreshed_bounded_preserves_64_distinct_ordered_outputs_and_independent_full_ring_phase() {
    let profile =
        BgvProfile::new(BgvParameters::derive(7, 257, 3, (60, 20), (60, 2), 2).unwrap()).unwrap();
    let precision = profile.plaintext_precision(1).unwrap();
    let budget = RamLfeBgvBoundedBudgetV1 {
        maximum_refresh_plans: 21,
        preparation_payload_bytes: 400 << 20,
        preparation_metadata_bytes: 64 << 20,
        canonical_arithmetic_units: 16_638_667_264,
        scalar_word_operations: 2_059_780_864,
        matrix_payload_bytes: RamLfeBgvBoundedPlanV1::matrix_payload_bytes(),
        layout_metadata_bytes: RamLfeBgvBoundedPlanV1::layout_metadata_bytes().unwrap(),
        preparation_scratch_bytes: RamLfeBgvNonlinearShapeV1::Bounded.preparation_scratch_bytes(),
        owned_residue_bytes: 1_146_880,
    };
    let plan =
        RamLfeBgvBoundedPlanV1::new(&profile, precision, &physical_refresh_schedule(), budget)
            .expect("complete fixed physical schedule before any private preparation");
    assert_eq!(plan.prepared_refresh_plans(), 21);
    assert_eq!(plan.operations().bootstrap_calls, 357);
    assert_eq!(plan.operations().ciphertext_products, 320);
    assert_eq!(plan.operations().routing_calls, 17);
    assert_eq!(plan.canonical_arithmetic_units(), 16_638_667_264);
    assert_eq!(plan.scalar_word_operations(), 2_059_780_864);
    assert_eq!(plan.owned_residue_bytes(), 1_146_880);
    assert_eq!(plan.output_shape().level(), 0);
    assert_eq!(plan.output_shape().plaintext_precision(), precision);
    assert_eq!(
        plan.output_shape().bound(),
        &Natural::from_u64(4_920_306_448_512)
    );
    let requirement = plan.preparation_requirement();
    assert!(requirement.peak_payload_bytes <= budget.preparation_payload_bytes);
    assert!(requirement.peak_metadata_bytes <= budget.preparation_metadata_bytes);

    let program = distinct_program();
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    assert_eq!(normal.report().peak_rank(), 16);
    assert_eq!(normal.gate_count(), 8);
    let prepared = RamLfeBgvBoundedProgramV1::from_normal_form(&normal, &plan).unwrap();
    let (secret, key) = LocalBootstrapKey::generate(
        &profile,
        plan.galois_elements(),
        LocalBootstrapSetupBudget {
            retained_residue_bytes: 3_686_400,
            returned_secret_bytes: 23_552,
            plaintext_bytes: 1024,
        },
    )
    .expect("actual OS-entropy local setup after complete public admission");
    let input = distinct_input();
    let memory = distinct_memory();
    let packed =
        RamLfeBgvPackedInputV1::encrypt(&profile, precision, profile.top_level(), &key, &input)
            .unwrap();
    let input_slots = std::array::from_fn(|i| {
        if i == 0 {
            63
        } else if i < 64 {
            1
        } else {
            0
        }
    });
    assert_independent_phase(
        &profile,
        &secret,
        packed.ciphertext(),
        &expected_coefficients(&input_slots),
    );
    super::tests::reset_counts();
    let output = prepared
        .evaluate(&profile, &plan, &memory, packed, &key)
        .expect("all sixteen complete layers and all357 physical refreshes");
    assert_eq!(super::tests::actual_counts(), *plan.operations());
    assert_eq!(&output.shape(), plan.output_shape());
    let expected = expected_slots();
    assert_independent_phase(
        &profile,
        &secret,
        output.ciphertext(),
        &expected_coefficients(&expected),
    );
    let opened = secret.decrypt(&profile, output.ciphertext()).unwrap();
    let decoded = SlotEncoding::new(&profile, precision)
        .unwrap()
        .decode(&opened)
        .unwrap();
    assert_eq!(decoded.as_slice(), expected.as_slice());
    let reference =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Bounded, &program, &memory, &input).unwrap();
    assert_eq!(
        decoded[..64],
        reference
            .output()
            .scalars()
            .iter()
            .map(|&x| u64::from(x))
            .collect::<Vec<_>>()
    );
    assert_eq!(&decoded[64..], &[0; 64]);
}
