//! Public F257 23-prime repeated-bootstrap schedule parity without ciphertexts or evaluation keys.
//!
//! This unselected diagnostic allocates public profile tables and two small
//! coefficient matrices. It does not execute encrypted F257 arithmetic, certify
//! security, admit external sources, or qualify application resource limits.

use iroha_fhe::{
    bgv::{
        BgvParameters, BgvProfile, BootstrapBudget, BootstrapError, BootstrapPlan, CiphertextShape,
        CoefficientSlotBudget, SlotLinearBudget, UpperDigitsBudget, cost::CostModel,
    },
    natural::Natural,
    prime_power::{
        PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan, UpperDigitsProgram,
    },
};

const LEVELS: [usize; 18] = [
    20, 19, 18, 17, 16, 15, 14, 13, 13, 11, 10, 9, 8, 7, 6, 5, 4, 4,
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
        owned_residue_bytes: 1 << 30,
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
        retained_preparation_bytes: 32 * 1024 * 1024,
        input_owned_bytes: 64 * 1024 * 1024,
        input_plaintext_bytes: 2 * 65_536 * 8,
        input_arithmetic_units: 1_u64 << 40,
        input_addition_words: 1_u64 << 30,
        stream_owned_residue_bytes: 1 << 30,
        stream_metadata_bytes: 1 << 20,
        canonical_arithmetic_units: 1_u64 << 46,
        scalar_word_operations: 1_u64 << 40,
    }
}

fn check_carry(profile: &BgvProfile, shape: &CiphertextShape) {
    let q = profile
        .level_modulus(shape.level())
        .expect("actual current modulus");
    let left = shape.bound().mul_u64(2 * 66_049).add(&q.mul_u64(65_537));
    assert!(left < q.mul_u64(66_049), "strict dense-secret carry");
}

#[test]
#[ignore = "public unselected N65536 planner only; no ciphertext or key generation"]
fn q23_output_lowering_before_square_forms_a_repeatable_current_arithmetic_cycle() {
    let profile = BgvProfile::new(
        BgvParameters::derive(16, 257, 3, (60, 23), (60, 2), 2).expect("unselected q23 geometry"),
    )
    .expect("public profile tables");
    assert_eq!(
        profile.parameters().main_primes.as_slice(),
        &[
            576_494_471_325_024_257_u64,
            576_507_820_688_146_433_u64,
            576_561_218_140_635_137_u64,
            576_599_041_336_147_969_u64,
            576_754_783_905_906_689_u64,
            576_799_281_782_980_609_u64,
            576_814_856_039_956_481_u64,
            576_839_329_872_347_137_u64,
            576_908_301_581_811_713_u64,
            576_932_775_414_202_369_u64,
            576_988_397_760_544_769_u64,
            577_072_943_726_985_217_u64,
            577_206_437_358_206_977_u64,
            577_242_035_659_866_113_u64,
            577_299_882_900_062_209_u64,
            577_306_557_581_623_297_u64,
            577_359_955_034_112_001_u64,
            577_382_203_972_648_961_u64,
            577_400_003_123_478_529_u64,
            577_422_252_062_015_489_u64,
            577_428_926_743_576_577_u64,
            577_433_376_531_283_969_u64,
            577_473_424_620_650_497_u64
        ]
    );
    assert_eq!(
        profile.parameters().special_primes.as_slice(),
        &[577_475_649_514_504_193_u64, 577_586_894_207_188_993_u64]
    );
    let precision = profile.plaintext_precision(1).expect("effective F257");
    let mut source = profile
        .plan_fresh(precision)
        .expect("honest fresh transfer");
    while source.level() > 0 {
        source = profile
            .plan_switch_down(&source)
            .expect("each source drop admitted");
        assert_eq!(source.bound(), &Natural::from_u64(556_231_950_720));
    }
    check_carry(&profile, &source);
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
    .expect("all current stage and combined budgets admit q23 schedule");
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
    assert_eq!(plan.input_shape(), &source);
    assert_eq!(plan.output_shape().level(), 4);
    assert_eq!(plan.output_shape().plaintext_precision(), precision);
    assert_eq!(plan.galois_elements().len(), 16);
    assert_eq!(
        plan.homomorphic_input_shape().bound(),
        &Natural::from_words_le(&[10_456_878_399_539_478_912_u64, 1_408_839_u64])
    );
    assert_eq!(
        plan.output_shape().bound(),
        &Natural::from_words_le(&[
            15_443_042_531_987_239_424_u64,
            3_666_307_800_200_288_600_u64,
            4_937_332_615_020_003_248_u64
        ])
    );
    assert!(
        profile
            .plan_multiply(plan.output_shape(), plan.output_shape())
            .is_err(),
        "this witness requires ordinary lowering before the next square"
    );
    let payload = plan.payload();
    assert_eq!(payload.canonical_arithmetic_units, 11_423_263_096_832);
    assert_eq!(payload.scalar_word_operations, 539_348_762_624);
    let mut exact = budget();
    exact.stream_owned_residue_bytes = payload.stream_owned_residue_bytes;
    exact.stream_metadata_bytes = payload.stream_metadata_bytes;
    exact.canonical_arithmetic_units = payload.canonical_arithmetic_units;
    exact.scalar_word_operations = payload.scalar_word_operations;
    let exact_plan = BootstrapPlan::new(
        &profile,
        &source,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        program(),
        &LEVELS,
        exact,
    )
    .expect("four exact resource ceilings");
    assert_eq!(exact_plan.payload(), payload);
    for resource in 0..4 {
        let mut short = exact;
        match resource {
            0 => short.stream_owned_residue_bytes -= 1,
            1 => short.stream_metadata_bytes -= 1,
            2 => short.canonical_arithmetic_units -= 1,
            _ => short.scalar_word_operations -= 1,
        }
        assert!(matches!(
            BootstrapPlan::new(
                &profile,
                &source,
                iroha_fhe::bgv::BootstrapMessageSpace::Ring,
                program(),
                &LEVELS,
                short
            ),
            Err(BootstrapError::Budget { .. })
        ));
    }
    let model = CostModel::new(&profile);
    let mut extra_work = 0_u64;
    let mut lower = plan.output_shape().clone();
    for (from_level, expected) in [
        (
            4,
            Natural::from_words_le(&[
                11_244_268_267_255_934_702_u64,
                10_340_144_870_002_028_706_u64,
                8_u64,
            ]),
        ),
        (
            3,
            Natural::from_words_le(&[16_078_037_141_200_283_143_u64, 273_u64]),
        ),
    ] {
        assert_eq!(lower.level(), from_level);
        extra_work += model.switch_down_units(from_level).unwrap();
        lower = profile.plan_switch_down(&lower).expect("output lowering");
        assert_eq!(lower.bound(), &expected);
        check_carry(&profile, &lower);
    }
    assert_eq!(lower.level(), 2);
    extra_work += model.multiplication_units(2).unwrap();
    let mut square = profile
        .plan_multiply(&lower, &lower)
        .expect("canonical square after lowering");
    assert_eq!(
        square.bound(),
        &Natural::from_words_le(&[
            3_517_145_997_904_658_796_u64,
            11_702_910_996_898_082_204_u64,
            4_915_570_211_u64
        ])
    );
    check_carry(&profile, &square);
    let direct = BootstrapPlan::new(
        &profile,
        &square,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        program(),
        &LEVELS,
        budget(),
    )
    .expect("actual squared source directly satisfies carry");
    assert_eq!(direct.output_shape(), plan.output_shape());
    for (from_level, expected) in [
        (
            2,
            Natural::from_words_le(&[9_919_605_623_255_712_960_u64, 157_270_837_540_u64]),
        ),
        (1, Natural::from_words_le(&[5_588_487_865_142_u64])),
    ] {
        assert_eq!(square.level(), from_level);
        extra_work += model.switch_down_units(from_level).unwrap();
        square = profile
            .plan_switch_down(&square)
            .expect("squared source lowering");
        assert_eq!(square.bound(), &expected);
        check_carry(&profile, &square);
    }
    assert_eq!(square.level(), 0);
    assert_eq!(extra_work, 32_964_608);
    assert_eq!(
        payload.canonical_arithmetic_units + extra_work,
        11_423_296_061_440
    );
    let repeated = BootstrapPlan::new(
        &profile,
        &square,
        iroha_fhe::bgv::BootstrapMessageSpace::Ring,
        program(),
        &LEVELS,
        budget(),
    )
    .expect("same complete schedule repeats from the actual lowered square");
    assert_eq!(repeated.output_shape(), plan.output_shape());
    assert_eq!(
        repeated.homomorphic_input_shape(),
        plan.homomorphic_input_shape()
    );
    assert_eq!(repeated.payload(), plan.payload());
    assert_eq!(model.ciphertext_bytes(0, 2).unwrap(), 983_252);
}
