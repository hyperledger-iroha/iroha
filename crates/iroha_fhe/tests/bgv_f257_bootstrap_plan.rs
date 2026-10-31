//! Public F257 bootstrap planner parity without ciphertexts or evaluation keys.
//!
//! This unselected diagnostic allocates public profile tables and two small
//! coefficient matrices. It does not execute encrypted F257 arithmetic, certify
//! security, admit external sources, or qualify application resource limits.

use iroha_fhe::{
    bgv::{
        BgvParameters, BgvProfile, BootstrapBudget, BootstrapError, BootstrapPlan, CiphertextShape,
        CoefficientSlotBudget, SlotLinearBudget, UpperDigitsBudget,
    },
    natural::Natural,
    prime_power::{
        PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan, UpperDigitsProgram,
    },
};

const LEVELS: [usize; 18] = [
    21, 20, 19, 18, 17, 16, 15, 14, 14, 12, 11, 10, 9, 8, 7, 6, 5, 4,
];

fn program() -> UpperDigitsProgram {
    let context = PrimePowerContext::new(257, 3).expect("F257 master precision three");
    let geometry = UpperDigitsPlan::required(context, 1).expect("complete E3 to E1 tableau");
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
    .expect("exact immutable polynomial budget");
    UpperDigitsProgram::prepare(&plan).expect("public polynomial preparation")
}

fn budget() -> BootstrapBudget {
    // Finite component ceilings for this unselected planning diagnostic. These
    // are not production policy limits and do not allocate ciphertext payloads.
    let transform = CoefficientSlotBudget {
        routing: SlotLinearBudget {
            coefficient_bytes: 128 * 129 * 4,
            layout_metadata_bytes: 16 * 1024,
            preparation_scratch_bytes: 129 * 4,
            output_residue_bytes: 32 * 1024 * 1024,
            scratch_residue_bytes: 512 * 1024 * 1024,
        },
        owned_residue_bytes: usize::try_from(14_u64 << 30).expect("64-bit diagnostic host"),
        trace_plaintext_bytes: 65_536 * 8,
    };
    BootstrapBudget {
        decomposition: transform,
        recomposition: transform,
        upper_digits: UpperDigitsBudget {
            compiler_steps: 1_000_000,
            metadata_bytes: 16 * 1024 * 1024,
            owned_residue_bytes: transform.owned_residue_bytes,
            plaintext_bytes: 65_536 * 8,
            backend_scratch_bytes: 128 * 1024 * 1024,
            canonical_arithmetic_units: 1_u64 << 60,
            scalar_word_operations: 1_u64 << 50,
        },
        canonical_arithmetic_units: 1_u64 << 60,
        scalar_word_operations: 1_u64 << 50,
        stream_owned_residue_bytes: 1 << 30,
        stream_metadata_bytes: 1 << 24,
        retained_preparation_bytes: 32 * 1024 * 1024,
        input_owned_bytes: 64 * 1024 * 1024,
        input_plaintext_bytes: 2 * 65_536 * 8,
        input_arithmetic_units: 1_u64 << 40,
        input_addition_words: 1_u64 << 30,
    }
}

fn lower_to_zero(profile: &BgvProfile, mut shape: CiphertextShape) -> CiphertextShape {
    while shape.level() > 0 {
        shape = profile
            .plan_switch_down(&shape)
            .expect("every individual canonical level transfer fits");
    }
    shape
}

#[test]
#[ignore = "public N65536 profile tables and planner only; no ciphertext or key generation"]
fn f257_complete_current_bootstrap_plan_and_level_zero_carry_match_integer_oracle() {
    let profile = BgvProfile::new(
        BgvParameters::derive(16, 257, 3, (60, 24), (60, 2), 2)
            .expect("unselected arithmetic profile geometry"),
    )
    .expect("valid public profile");
    let precision = profile.plaintext_precision(1).expect("effective F257");
    let source = lower_to_zero(
        &profile,
        profile
            .plan_fresh(precision)
            .expect("genuine fresh transfer"),
    );
    assert_eq!(source.bound(), &Natural::from_u64(556_231_950_720));
    let upper_program = program();
    let metadata_requirement = BootstrapPlan::metadata_requirement(
        &profile,
        precision,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        &upper_program.plan(),
    )
    .expect("complete public typed metadata bound before preparation");
    let plan = BootstrapPlan::new(
        &profile,
        &source,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        upper_program,
        &LEVELS,
        budget(),
    )
    .expect("complete current constructor, transforms and digit schedule");
    let retained_metadata = plan
        .retained_metadata()
        .expect("observe complete prepared N65536 plan metadata");
    assert_eq!(
        retained_metadata.inline_bytes,
        metadata_requirement.inline_bytes
    );
    assert!(retained_metadata.retained_heap_bytes <= metadata_requirement.retained_heap_bytes);
    assert_eq!(retained_metadata.preparation_scratch_bytes, 0);
    assert!(
        retained_metadata.retained_bytes().unwrap() <= metadata_requirement.peak_bytes().unwrap()
    );
    assert_eq!(plan.payload().stream_owned_residue_bytes, 654_311_424);
    assert_eq!(
        plan.payload().canonical_arithmetic_units,
        12_346_604_126_208
    );
    assert_eq!(plan.payload().scalar_word_operations, 573_104_324_608);
    let mut exact = budget();
    exact.stream_owned_residue_bytes = plan.payload().stream_owned_residue_bytes;
    exact.stream_metadata_bytes = plan.payload().stream_metadata_bytes;
    exact.canonical_arithmetic_units = plan.payload().canonical_arithmetic_units;
    exact.scalar_word_operations = plan.payload().scalar_word_operations;
    BootstrapPlan::new(
        &profile,
        &source,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        program(),
        &LEVELS,
        exact,
    )
    .expect("all four exact complete stream/work ceilings fit");
    for which in 0..4 {
        let mut insufficient = exact;
        let resource = match which {
            0 => {
                insufficient.stream_owned_residue_bytes -= 1;
                "stream residues"
            }
            1 => {
                insufficient.stream_metadata_bytes -= 1;
                "stream metadata"
            }
            2 => {
                insufficient.canonical_arithmetic_units -= 1;
                "complete arithmetic"
            }
            _ => {
                insufficient.scalar_word_operations -= 1;
                "complete scalar words"
            }
        };
        assert!(
            matches!(BootstrapPlan::new(&profile,&source, iroha_fhe::bgv::BootstrapMessageSpace::Ring,program(),&LEVELS,insufficient),Err(BootstrapError::Budget { resource: found }) if found==resource)
        );
    }

    assert_eq!(plan.input_shape(), &source);
    assert_eq!(
        plan.homomorphic_input_shape().bound(),
        &Natural::from_words_le(&[10_456_878_399_539_478_912, 1_408_839])
    );
    assert_eq!(plan.galois_elements().len(), 16);
    assert_eq!(plan.output_shape().level(), 4);
    assert_eq!(plan.output_shape().plaintext_precision(), precision);
    assert_eq!(
        plan.output_shape().bound(),
        &Natural::from_words_le(&[10_722_834_492_894_606_848, 16_843_206_870_962_163_400, 7])
    );
    assert!(!plan.reduces_phase_bound(), "a fresh source need not gain");
    let final_zero = lower_to_zero(&profile, plan.output_shape().clone());
    assert_eq!(final_zero.bound(), &Natural::from_u64(556_231_950_720));
    let square = profile
        .plan_multiply(plan.output_shape(), plan.output_shape())
        .expect("real canonical tensor and relinearization transfers");
    assert_eq!(
        square.bound(),
        &Natural::from_words_le(&[
            6_766_181_321_523_030_229,
            15_412_242_943_344_252_287,
            13_185_601_604_741_141_557,
            12_609_646_224_771_721_449,
            4_103_648
        ])
    );
    let square_zero = lower_to_zero(&profile, square);
    assert_eq!(square_zero.bound(), &Natural::from_u64(4_854_894_088_388));
    let repeated = BootstrapPlan::new(
        &profile,
        &square_zero,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        program(),
        &LEVELS,
        budget(),
    )
    .expect("same complete schedule accepts square followed by all level drops");
    assert_eq!(repeated.output_shape(), plan.output_shape());

    // This exact upper bound comes from the strict dense ternary carry theorem,
    // not an observed secret Hamming weight or a supplied ciphertext body.
    let scale = 66_049;
    let q = profile.parameters().main_primes[0];
    let limit = (Natural::from_u64(q)
        .mul_u64(scale - 65_536 - 1)
        .checked_sub(&Natural::one())
        .expect("positive strict carry numerator"))
    .div_floor_u64(2 * scale)
    .expect("nonzero public divisor");
    assert_eq!(limit, Natural::from_u64(2_234_440_864_497_664));
    let forbidden = profile
        .plan_claimed(precision, 0, 2, limit.add_u64(1))
        .expect("ordinary phase capacity still admits this invalid carry source");
    assert!(matches!(
        BootstrapPlan::new(
            &profile,
            &forbidden,
            iroha_fhe::bgv::BootstrapMessageSpace::Ring,
            program(),
            &LEVELS,
            budget()
        ),
        Err(BootstrapError::RoundingBound)
    ));
}
