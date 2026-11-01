//! Public full-size scalar-bootstrap planning, resource and native metadata parity.
//!
//! This ignored unselected diagnostic generates no key or ciphertext. It checks
//! current source-derived q23 work/phase vectors and architecture-local metadata;
//! it establishes no encrypted execution, security or application qualification.

use std::mem::size_of;

use iroha_fhe::{
    bgv::{
        BgvParameters, BgvProfile, BootstrapBudget, BootstrapError, BootstrapMessageSpace,
        BootstrapPlan, CiphertextShape, CoefficientSlotBudget, SlotEncoding, SlotLinearBudget,
        UpperDigitsBudget, cost::CostModel,
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
    let context = PrimePowerContext::new(257, 3).expect("F257 master precision");
    let geometry = UpperDigitsPlan::required(context, 1).expect("complete E3-to-E1 tableau");
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
    .expect("exact immutable program budget");
    UpperDigitsProgram::prepare(&plan).expect("public program preparation")
}

fn budget() -> BootstrapBudget {
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
        stream_owned_residue_bytes: 1 << 30,
        stream_metadata_bytes: 1 << 20,
        canonical_arithmetic_units: 1_u64 << 46,
        scalar_word_operations: 1_u64 << 40,
    }
}

fn check_carry(profile: &BgvProfile, source: &CiphertextShape) {
    let modulus = profile
        .level_modulus(source.level())
        .expect("source modulus");
    assert!(
        source
            .bound()
            .mul_u64(132_098)
            .add(&modulus.mul_u64(65_537))
            < modulus.mul_u64(66_049)
    );
}

struct BudgetCase {
    name: &'static str,
    get: fn(BootstrapBudget) -> u64,
    set: fn(&mut BootstrapBudget, u64),
    expected: Option<u64>,
}

fn resource_cases() -> [BudgetCase; 30] {
    macro_rules! case {
        ($($field:ident).+, $expected:expr) => {
            BudgetCase {
                name: stringify!($($field).+),
                get: |budget| budget.$($field).+ as u64,
                set: |budget, value| budget.$($field).+ = value.try_into().expect("native resource range"),
                expected: $expected,
            }
        };
    }
    [
        case!(canonical_arithmetic_units, Some(24_584_388_608)),
        case!(scalar_word_operations, Some(1_075_380_224)),
        case!(stream_owned_residue_bytes, Some(409_993_216)),
        case!(stream_metadata_bytes, None),
        case!(retained_preparation_bytes, Some(16_942_080)),
        case!(input_owned_bytes, Some(48_758_784)),
        case!(input_plaintext_bytes, Some(1_048_576)),
        case!(input_arithmetic_units, Some(66_715_648)),
        case!(input_addition_words, Some(1_507_328)),
        case!(decomposition.routing.coefficient_bytes, Some(66_048)),
        case!(decomposition.routing.layout_metadata_bytes, None),
        case!(decomposition.routing.preparation_scratch_bytes, Some(516)),
        case!(decomposition.routing.output_residue_bytes, Some(24_117_248)),
        case!(
            decomposition.routing.scratch_residue_bytes,
            Some(361_758_720)
        ),
        case!(decomposition.owned_residue_bytes, Some(409_993_216)),
        case!(decomposition.trace_plaintext_bytes, Some(524_288)),
        case!(recomposition.routing.coefficient_bytes, Some(66_048)),
        case!(recomposition.routing.layout_metadata_bytes, None),
        case!(recomposition.routing.preparation_scratch_bytes, Some(516)),
        case!(recomposition.routing.output_residue_bytes, Some(5_242_880)),
        case!(
            recomposition.routing.scratch_residue_bytes,
            Some(78_643_200)
        ),
        case!(recomposition.owned_residue_bytes, Some(89_128_960)),
        case!(recomposition.trace_plaintext_bytes, Some(524_288)),
        case!(upper_digits.compiler_steps, None),
        case!(upper_digits.metadata_bytes, None),
        case!(upper_digits.owned_residue_bytes, Some(347_602_944)),
        case!(upper_digits.plaintext_bytes, Some(524_288)),
        case!(upper_digits.backend_scratch_bytes, Some(47_710_208)),
        case!(
            upper_digits.canonical_arithmetic_units,
            Some(193_861_451_776)
        ),
        case!(upper_digits.scalar_word_operations, Some(1_136_525_312)),
    ]
}

fn exact_budget(
    plan: &BootstrapPlan,
    profile: &BgvProfile,
    source: &CiphertextShape,
) -> BootstrapBudget {
    // Public immutable source readmission exposes exact required caps without
    // assuming opaque Rust layouts or adding implementation-only API getters.
    // Every numeric field with an independent integer model has an exact vector;
    // native metadata and compiler visits are observed, then boundary tested.
    let mut exact = budget();
    for case in resource_cases() {
        let mut low = 0;
        let mut high = (case.get)(budget());
        while low < high {
            let middle = low + (high - low) / 2;
            let mut trial = budget();
            (case.set)(&mut trial, middle);
            match plan.admit_source(profile, source, trial) {
                Ok(_) => high = middle,
                Err(BootstrapError::Budget { .. }) => low = middle + 1,
                Err(error) => panic!("unexpected {} refusal: {error}", case.name),
            }
        }
        assert!(low > 0, "{} has actual work/storage", case.name);
        if let Some(expected) = case.expected {
            assert_eq!(low, expected, "{} independent vector", case.name);
        }
        (case.set)(&mut exact, low);
        eprintln!("scalar_q23_resource {}={low}", case.name);
    }
    let accepted = plan
        .admit_source(profile, source, exact)
        .expect("all thirty exact caps");
    assert_eq!(accepted.payload(), plan.payload());
    assert!(plan.shares_preparation(&accepted));
    for case in resource_cases() {
        let mut short = exact;
        (case.set)(&mut short, (case.get)(exact) - 1);
        assert!(
            matches!(
                plan.admit_source(profile, source, short),
                Err(BootstrapError::Budget { .. })
            ),
            "{} one-under cap must refuse",
            case.name
        );
    }
    exact
}

#[test]
#[ignore = "public unselected N65536 scalar/Ring planning only; no key or ciphertext generation"]
fn scalar_q23_fullsize_public_work_carry_caps_and_native_metadata() {
    let profile = BgvProfile::new(
        BgvParameters::derive(16, 257, 3, (60, 23), (60, 2), 2).expect("unselected q23 profile"),
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
    let effective = profile.plaintext_precision(1).expect("effective F257");
    assert_eq!(profile.degree(), 65_536);
    let encoding = SlotEncoding::new(&profile, effective).expect("canonical scalar geometry");
    assert_eq!(encoding.slots(), 128);
    assert_eq!(encoding.stride(), 512);
    assert_eq!(
        profile.degree() / 128,
        512,
        "geometric banks remain unchanged"
    );
    let mut source = profile.plan_fresh(effective).expect("honest fresh shape");
    while source.level() > 0 {
        source = profile
            .plan_switch_down(&source)
            .expect("actual fresh source lowering");
        assert_eq!(source.bound(), &Natural::from_u64(556_231_950_720));
    }
    check_carry(&profile, &source);
    let prepared = program();
    let selected_requirement = BootstrapPlan::split_metadata_requirement(
        &profile,
        effective,
        BootstrapMessageSpace::ScalarSlots,
        &prepared.plan(),
    )
    .expect("native typed metadata before preparation");
    let ring_requirement = BootstrapPlan::split_metadata_requirement(
        &profile,
        effective,
        BootstrapMessageSpace::Ring,
        &prepared.plan(),
    )
    .expect("unchanged full-Ring native metadata");
    assert_eq!(
        selected_requirement.source_descriptor.inline_bytes,
        size_of::<BootstrapPlan>()
    );
    assert_eq!(
        selected_requirement.source_descriptor.retained_heap_bytes,
        360
    );
    assert_eq!(
        selected_requirement.shared_preparation.retained_heap_bytes,
        5_988 + size_of::<CiphertextShape>()
    );
    assert_eq!(
        ring_requirement.shared_preparation.retained_heap_bytes
            - selected_requirement.shared_preparation.retained_heap_bytes,
        511 * (360 + size_of::<CiphertextShape>())
    );
    assert_eq!(
        selected_requirement.shared_preparation.inline_bytes,
        ring_requirement.shared_preparation.inline_bytes
    );
    let selected = BootstrapPlan::new(
        &profile,
        &source,
        BootstrapMessageSpace::ScalarSlots,
        prepared,
        &LEVELS,
        budget(),
    )
    .expect("selected fullsize public plan");
    let ring = BootstrapPlan::new(
        &profile,
        &source,
        BootstrapMessageSpace::Ring,
        program(),
        &LEVELS,
        budget(),
    )
    .expect("complete Ring remains admitted");
    assert_eq!(selected.message_space(), BootstrapMessageSpace::ScalarSlots);
    assert_eq!(ring.message_space(), BootstrapMessageSpace::Ring);
    assert!(!selected.shares_preparation(&ring));
    assert_eq!(selected.input_shape(), &source);
    assert_eq!(
        selected.homomorphic_input_shape(),
        ring.homomorphic_input_shape()
    );
    assert_eq!(
        selected.homomorphic_input_shape().bound(),
        &Natural::from_words_le(&[10_456_878_399_539_478_912, 1_408_839])
    );
    assert_eq!(selected.output_shape().level(), 4);
    assert_eq!(selected.output_shape().plaintext_precision(), effective);
    assert_eq!(
        selected.output_shape().bound(),
        &Natural::from_words_le(&[
            12_424_068_366_968_892_569,
            15_571_601_069_614_700_364,
            9_643_227_763_710_943,
        ])
    );
    assert_eq!(
        ring.output_shape().bound(),
        &selected.output_shape().bound().mul_u64(512)
    );
    assert_eq!(selected.galois_elements(), ring.galois_elements());
    assert_eq!(selected.galois_elements().len(), 16);
    assert_eq!(
        selected.payload().canonical_arithmetic_units,
        24_584_388_608
    );
    assert_eq!(selected.payload().scalar_word_operations, 1_075_380_224);
    assert_eq!(
        ring.payload().canonical_arithmetic_units,
        11_423_263_096_832
    );
    assert_eq!(ring.payload().scalar_word_operations, 539_348_762_624);
    assert_eq!(
        selected.payload().input_arithmetic_units,
        ring.payload().input_arithmetic_units
    );
    let exact = exact_budget(&selected, &profile, &source);
    // Reprepare under the observed exact stage caps. The supplied preparation
    // ceiling can shrink for this new owner; the original shared owner cannot.
    let reprepared = BootstrapPlan::new(
        &profile,
        &source,
        BootstrapMessageSpace::ScalarSlots,
        program(),
        &LEVELS,
        exact,
    )
    .expect("exact-cap independent preparation");
    assert_eq!(reprepared.output_shape(), selected.output_shape());
    assert_eq!(
        reprepared.payload().canonical_arithmetic_units,
        selected.payload().canonical_arithmetic_units
    );
    assert_eq!(
        reprepared.payload().stream_owned_residue_bytes,
        selected.payload().stream_owned_residue_bytes
    );
    assert!(!reprepared.shares_preparation(&selected));
    let observed = selected
        .retained_metadata_split()
        .expect("actual retained capacities");
    for (actual, required) in [
        (
            observed.shared_preparation,
            selected_requirement.shared_preparation,
        ),
        (
            observed.source_descriptor,
            selected_requirement.source_descriptor,
        ),
    ] {
        assert_eq!(actual.inline_bytes, required.inline_bytes);
        assert!(actual.retained_heap_bytes <= required.retained_heap_bytes);
        assert_eq!(actual.preparation_scratch_bytes, 0);
        assert!(actual.retained_bytes().unwrap() <= required.peak_bytes().unwrap());
    }
    eprintln!("scalar_q23_metadata requirement={selected_requirement:?} observed={observed:?}");
    let mut descriptors = Vec::with_capacity(SOURCE_BOUNDS.len());
    let mut observed_source_heap = 0;
    assert_eq!(SOURCE_BOUNDS.len(), 640);
    assert!(SOURCE_BOUNDS.windows(2).all(|pair| pair[0] < pair[1]));
    for bound in SOURCE_BOUNDS {
        // Public planning descriptors only; plan_claimed creates no ciphertext
        // or local/scalar capability and authorizes no encrypted execution.
        let shape = profile
            .plan_claimed(effective, 0, 2, Natural::from_u64(bound))
            .expect("exact public recurrence source shape");
        check_carry(&profile, &shape);
        let descriptor = selected
            .admit_source(&profile, &shape, exact)
            .expect("exact source admitted");
        assert_eq!(descriptor.input_shape(), &shape);
        assert_eq!(descriptor.output_shape(), selected.output_shape());
        assert_eq!(descriptor.payload(), selected.payload());
        assert!(descriptor.shares_preparation(&selected));
        let actual = descriptor.retained_metadata_split().unwrap();
        assert_eq!(actual.shared_preparation, observed.shared_preparation);
        observed_source_heap += actual.source_descriptor.retained_heap_bytes;
        descriptors.push(descriptor);
    }
    let all_sources = selected_requirement
        .source_descriptor
        .repeated(640)
        .unwrap();
    assert_eq!(all_sources.inline_bytes, 640 * size_of::<BootstrapPlan>());
    assert_eq!(all_sources.retained_heap_bytes, 640 * 360);
    assert!(observed_source_heap <= all_sources.retained_heap_bytes);
    eprintln!(
        "scalar_q23_metadata exact_descriptors=640 observed_source_heap={observed_source_heap} all_sources={all_sources:?}"
    );
    let last_carry = profile
        .plan_claimed(effective, 0, 2, Natural::from_u64(2_234_440_864_497_664))
        .unwrap();
    let first_bad = profile
        .plan_claimed(effective, 0, 2, Natural::from_u64(2_234_440_864_497_665))
        .unwrap();
    check_carry(&profile, &last_carry);
    assert!(selected.admit_source(&profile, &last_carry, exact).is_ok());
    assert!(matches!(
        selected.admit_source(&profile, &first_bad, exact),
        Err(BootstrapError::RoundingBound)
    ));
    let mut lower = selected.output_shape().clone();
    for expected in [
        Natural::from_words_le(&[5_858_627_133_677_207_255, 308_425_971_600_934_456]),
        Natural::from_u64(9_867_264_557_614_130_258),
        Natural::from_u64(556_231_950_737),
    ] {
        lower = profile
            .plan_switch_down(&lower)
            .expect("actual output drop");
        assert_eq!(lower.bound(), &expected);
        check_carry(&profile, &lower);
    }
    assert_eq!(lower.level(), 1);
    let square = profile
        .plan_multiply(&lower, &lower)
        .expect("actual public product transfer");
    assert_eq!(
        square.bound(),
        &Natural::from_words_le(&[11_366_334_896_302_408_044, 1_099_188_235])
    );
    let next = profile
        .plan_switch_down(&square)
        .expect("source-zero product drop");
    assert_eq!(next.bound(), &Natural::from_u64(591_403_102_513));
    check_carry(&profile, &next);
    let repeated = selected
        .admit_source(&profile, &next, exact)
        .expect("repeat from derived source");
    assert_eq!(repeated.output_shape(), selected.output_shape());
    assert!(repeated.shares_preparation(&selected));
    assert_eq!(
        CostModel::new(&profile).ciphertext_bytes(0, 2).unwrap(),
        983_252
    );
}

// Exact source bounds of the independent complete 2871-step target-bank R9
// recurrence: 2872 physical bootstraps, routing4/products1/source0. This test
// checks every derived descriptor; it does not execute that encrypted schedule.
const SOURCE_BOUNDS: [u64; 640] = [
    556_231_950_848,
    591_403_137_970,
    591_406_542_161,
    591_406_559_891,
    591_406_577_622,
    591_406_595_353,
    591_406_613_083,
    591_406_630_814,
    591_406_648_544,
    591_406_666_275,
    591_406_684_006,
    591_406_701_736,
    591_406_719_467,
    591_406_737_198,
    591_406_754_928,
    591_406_772_659,
    591_406_790_390,
    591_406_808_120,
    591_406_825_851,
    591_406_843_582,
    591_406_861_312,
    591_406_879_043,
    591_406_896_774,
    591_406_914_504,
    591_406_932_235,
    591_406_949_966,
    591_406_967_696,
    591_406_985_427,
    591_407_003_158,
    591_407_020_888,
    591_407_038_619,
    591_407_056_350,
    591_407_074_081,
    591_407_091_811,
    591_407_109_542,
    591_407_127_273,
    591_407_145_003,
    591_407_162_734,
    591_407_180_465,
    591_407_198_196,
    591_407_215_927,
    591_407_233_657,
    591_407_251_388,
    591_407_269_119,
    591_407_286_850,
    591_407_304_580,
    591_407_322_311,
    591_407_340_042,
    591_407_357_773,
    591_407_375_504,
    591_407_393_234,
    591_407_410_965,
    591_407_428_696,
    591_407_446_427,
    591_407_464_158,
    591_407_481_888,
    591_407_499_619,
    591_407_517_350,
    591_407_535_081,
    591_407_552_812,
    591_407_570_543,
    591_407_588_273,
    591_407_606_004,
    591_407_623_735,
    591_407_641_466,
    591_407_659_197,
    591_407_676_928,
    591_407_694_659,
    591_407_712_390,
    591_407_730_120,
    591_407_747_851,
    591_407_765_582,
    591_407_783_313,
    591_407_801_044,
    591_407_818_775,
    591_407_836_506,
    591_407_854_237,
    591_407_871_968,
    591_407_889_699,
    591_407_907_429,
    591_407_925_160,
    591_407_942_891,
    591_407_960_622,
    591_407_978_353,
    591_407_996_084,
    591_408_013_815,
    591_408_031_546,
    591_408_049_277,
    591_408_067_008,
    591_408_084_739,
    591_408_102_470,
    591_408_120_201,
    591_408_137_932,
    591_408_155_663,
    591_408_173_394,
    591_408_191_125,
    591_408_208_856,
    591_408_226_587,
    591_408_244_318,
    591_408_262_049,
    591_408_279_780,
    591_408_297_511,
    591_408_315_242,
    591_408_332_973,
    591_408_350_704,
    591_408_368_435,
    591_408_386_166,
    591_408_403_897,
    591_408_421_628,
    591_408_439_359,
    591_408_457_091,
    591_408_474_822,
    591_408_492_553,
    591_408_510_284,
    591_408_528_015,
    591_408_545_746,
    591_408_563_477,
    591_408_581_208,
    591_408_598_939,
    591_408_616_670,
    591_408_634_401,
    591_408_652_133,
    591_408_669_864,
    591_408_687_595,
    591_408_705_326,
    591_408_723_057,
    591_408_740_788,
    591_408_758_519,
    591_408_776_251,
    591_408_793_982,
    591_408_811_713,
    591_408_829_444,
    591_408_847_175,
    591_408_864_906,
    591_408_882_638,
    591_408_900_369,
    591_408_918_100,
    591_408_935_831,
    591_408_953_562,
    591_408_971_294,
    591_408_989_025,
    591_409_006_756,
    591_409_024_487,
    591_409_042_218,
    591_409_059_950,
    591_409_077_681,
    591_409_095_412,
    591_409_113_143,
    591_409_130_874,
    591_409_148_606,
    591_409_166_337,
    591_409_184_068,
    591_409_201_800,
    591_409_219_531,
    591_409_237_262,
    591_409_254_993,
    591_409_272_725,
    591_409_290_456,
    591_409_308_187,
    591_409_325_918,
    591_409_343_650,
    591_409_361_381,
    591_409_379_112,
    591_409_396_844,
    591_409_414_575,
    591_409_432_306,
    591_409_450_038,
    591_409_467_769,
    591_409_485_500,
    591_409_503_232,
    591_409_520_963,
    591_409_538_694,
    591_409_556_426,
    591_409_574_157,
    591_409_591_888,
    591_409_609_620,
    591_409_627_351,
    591_409_645_082,
    591_409_662_814,
    591_409_680_545,
    591_409_698_277,
    591_409_716_008,
    591_409_733_739,
    591_409_751_471,
    591_409_769_202,
    591_409_786_933,
    591_409_804_665,
    591_409_822_396,
    591_409_840_128,
    591_409_857_859,
    591_409_875_591,
    591_409_893_322,
    591_409_911_053,
    591_409_928_785,
    591_409_946_516,
    591_409_964_248,
    591_409_981_979,
    591_409_999_711,
    591_410_017_442,
    591_410_035_174,
    591_410_052_905,
    591_410_070_637,
    591_410_088_368,
    591_410_106_100,
    591_410_123_831,
    591_410_141_562,
    591_410_159_294,
    591_410_177_026,
    591_410_194_757,
    591_410_212_489,
    591_410_230_220,
    591_410_247_952,
    591_410_265_683,
    591_410_283_415,
    591_410_301_146,
    591_410_318_878,
    591_410_336_609,
    591_410_354_341,
    591_410_372_072,
    591_410_389_804,
    591_410_407_535,
    591_410_425_267,
    591_410_442_999,
    591_410_460_730,
    591_410_478_462,
    591_410_496_193,
    591_410_513_925,
    591_410_531_656,
    591_410_549_388,
    591_410_567_120,
    591_410_584_851,
    591_410_602_583,
    591_410_620_314,
    591_410_638_046,
    591_410_655_778,
    591_410_673_509,
    591_410_691_241,
    591_410_708_973,
    591_410_726_704,
    591_410_744_436,
    591_410_762_168,
    591_410_779_899,
    591_410_797_631,
    591_410_815_363,
    591_410_833_094,
    591_410_850_826,
    591_410_868_558,
    591_410_886_289,
    591_410_904_021,
    591_410_921_753,
    591_410_939_484,
    591_410_957_216,
    591_410_974_948,
    591_410_992_679,
    591_411_010_411,
    591_411_028_143,
    591_411_045_875,
    591_411_063_606,
    591_411_081_338,
    591_411_099_070,
    591_411_116_801,
    591_411_134_533,
    591_411_152_265,
    591_411_169_997,
    591_411_187_729,
    591_411_205_460,
    591_411_223_192,
    591_411_240_924,
    591_411_258_656,
    591_411_276_387,
    591_411_294_119,
    591_411_311_851,
    591_411_329_583,
    591_411_347_315,
    591_411_365_046,
    591_411_382_778,
    591_411_400_510,
    591_411_418_242,
    591_411_435_974,
    591_411_453_705,
    591_411_471_437,
    591_411_489_169,
    591_411_506_901,
    591_411_524_633,
    591_411_542_365,
    591_411_560_096,
    591_411_577_828,
    591_411_595_560,
    591_411_613_292,
    591_411_631_024,
    591_411_648_756,
    591_411_666_488,
    591_411_684_220,
    591_411_701_951,
    591_411_719_683,
    591_411_737_415,
    591_411_755_147,
    591_411_772_879,
    591_411_790_611,
    591_411_808_343,
    591_411_826_075,
    591_411_843_807,
    591_411_861_539,
    591_411_879_271,
    591_411_897_002,
    591_411_914_734,
    591_411_932_466,
    591_411_950_198,
    591_411_967_930,
    591_411_985_662,
    591_412_003_394,
    591_412_021_126,
    591_412_038_858,
    591_412_056_590,
    591_412_074_322,
    591_412_092_054,
    591_412_109_786,
    591_412_127_518,
    591_412_145_250,
    591_412_162_982,
    591_412_180_714,
    591_412_198_446,
    591_412_216_178,
    591_412_233_910,
    591_412_251_642,
    591_412_269_374,
    591_412_287_106,
    591_412_304_838,
    591_412_322_570,
    591_412_340_302,
    591_412_358_034,
    591_412_375_766,
    591_412_393_498,
    591_412_411_231,
    591_412_428_963,
    591_412_446_695,
    591_412_464_427,
    591_412_482_159,
    591_412_499_891,
    591_412_517_623,
    591_412_535_355,
    591_412_553_087,
    591_412_570_819,
    591_412_588_551,
    591_412_606_284,
    591_412_624_016,
    591_412_641_748,
    591_412_659_480,
    591_412_677_212,
    591_412_694_944,
    591_412_712_676,
    591_412_730_409,
    591_412_748_141,
    591_412_765_873,
    591_412_783_605,
    591_412_801_337,
    591_412_819_069,
    591_412_836_801,
    591_412_854_534,
    591_412_872_266,
    591_412_889_998,
    591_412_907_730,
    591_412_925_462,
    591_412_943_195,
    591_412_960_927,
    591_412_978_659,
    591_412_996_391,
    591_413_014_123,
    591_413_031_856,
    591_413_049_588,
    591_413_067_320,
    591_413_085_052,
    591_413_102_785,
    591_413_120_517,
    591_413_138_249,
    591_413_155_981,
    591_413_173_714,
    591_413_191_446,
    591_413_209_178,
    591_413_226_910,
    591_413_244_643,
    591_413_262_375,
    591_413_280_107,
    591_413_297_840,
    591_413_315_572,
    591_413_333_304,
    591_413_351_036,
    591_413_368_769,
    591_413_386_501,
    591_413_404_233,
    591_413_421_966,
    591_413_439_698,
    591_413_457_430,
    591_413_475_163,
    591_413_492_895,
    591_413_510_627,
    591_413_528_360,
    591_413_546_092,
    591_413_563_824,
    591_413_581_557,
    591_413_599_289,
    591_413_617_021,
    591_413_634_754,
    591_413_652_486,
    591_413_670_219,
    591_413_687_951,
    591_413_705_683,
    591_413_723_416,
    591_413_741_148,
    591_413_758_881,
    591_413_776_613,
    591_413_794_345,
    591_413_812_078,
    591_413_829_810,
    591_413_847_543,
    591_413_865_275,
    591_413_883_008,
    591_413_900_740,
    591_413_918_472,
    591_413_936_205,
    591_413_953_937,
    591_413_971_670,
    591_413_989_402,
    591_414_007_135,
    591_414_024_867,
    591_414_042_600,
    591_414_060_332,
    591_414_078_065,
    591_414_095_797,
    591_414_113_530,
    591_414_131_262,
    591_414_148_995,
    591_414_166_727,
    591_414_184_460,
    591_414_202_192,
    591_414_219_925,
    591_414_237_657,
    591_414_255_390,
    591_414_273_122,
    591_414_290_855,
    591_414_308_587,
    591_414_326_320,
    591_414_344_053,
    591_414_361_785,
    591_414_379_518,
    591_414_397_250,
    591_414_414_983,
    591_414_432_715,
    591_414_450_448,
    591_414_468_180,
    591_414_485_913,
    591_414_503_646,
    591_414_521_378,
    591_414_539_111,
    591_414_556_844,
    591_414_574_576,
    591_414_592_309,
    591_414_610_041,
    591_414_627_774,
    591_414_645_507,
    591_414_663_239,
    591_414_680_972,
    591_414_698_704,
    591_414_716_437,
    591_414_734_170,
    591_414_751_902,
    591_414_769_635,
    591_414_787_368,
    591_414_805_100,
    591_414_822_833,
    591_414_840_566,
    591_414_858_298,
    591_414_876_031,
    591_414_893_764,
    591_414_911_497,
    591_414_929_229,
    591_414_946_962,
    591_414_964_695,
    591_414_982_427,
    591_415_000_160,
    591_415_017_893,
    591_415_035_626,
    591_415_053_358,
    591_415_071_091,
    591_415_088_824,
    591_415_106_556,
    591_415_124_289,
    591_415_142_022,
    591_415_159_755,
    591_415_177_488,
    591_415_195_220,
    591_415_212_953,
    591_415_230_686,
    591_415_248_419,
    591_415_266_151,
    591_415_283_884,
    591_415_301_617,
    591_415_319_350,
    591_415_337_083,
    591_415_354_815,
    591_415_372_548,
    591_415_390_281,
    591_415_408_014,
    591_415_425_747,
    591_415_443_480,
    591_415_461_212,
    591_415_478_945,
    591_415_496_678,
    591_415_514_411,
    591_415_532_144,
    591_415_549_877,
    591_415_567_609,
    591_415_585_342,
    591_415_603_075,
    591_415_620_808,
    591_415_638_541,
    591_415_656_274,
    591_415_674_007,
    591_415_691_740,
    591_415_709_472,
    591_415_727_205,
    591_415_744_938,
    591_415_762_671,
    591_415_780_404,
    591_415_798_137,
    591_415_815_870,
    591_415_833_603,
    591_415_851_336,
    591_415_869_069,
    591_415_886_802,
    591_415_904_535,
    591_415_922_268,
    591_415_940_001,
    591_415_957_734,
    591_415_975_466,
    591_415_993_199,
    591_416_010_932,
    591_416_028_665,
    591_416_046_398,
    591_416_064_131,
    591_416_081_864,
    591_416_099_597,
    591_416_117_330,
    591_416_135_063,
    591_416_152_796,
    591_416_170_529,
    591_416_188_262,
    591_416_205_995,
    591_416_223_728,
    591_416_241_461,
    591_416_259_194,
    591_416_276_928,
    591_416_294_661,
    591_416_312_394,
    591_416_330_127,
    591_416_347_860,
    591_416_365_593,
    591_416_383_326,
    591_416_401_059,
    591_416_418_792,
    591_416_436_525,
    591_416_454_258,
    591_416_471_991,
    591_416_489_724,
    591_416_507_457,
    591_416_525_191,
    591_416_542_924,
    591_416_560_657,
    591_416_578_390,
    591_416_596_123,
    591_416_613_856,
    591_416_631_589,
    591_416_649_322,
    591_416_667_056,
    591_416_684_789,
    591_416_702_522,
    591_416_720_255,
    591_416_737_988,
    591_416_755_721,
    591_416_773_454,
    591_416_791_188,
    591_416_808_921,
    591_416_826_654,
    591_416_844_387,
    591_416_862_120,
    591_416_879_854,
    591_416_897_587,
    591_416_915_320,
    591_416_933_053,
    591_416_950_786,
    591_416_968_520,
    591_416_986_253,
    591_417_003_986,
    591_417_021_719,
    591_417_039_453,
    591_417_057_186,
    591_417_074_919,
    591_417_092_652,
    591_417_110_385,
    591_417_128_119,
    591_417_145_852,
    591_417_163_585,
    591_417_181_318,
    591_417_199_052,
    591_417_216_785,
    591_417_234_518,
    591_417_252_252,
    591_417_269_985,
    591_417_287_718,
    591_417_305_452,
    591_417_323_185,
    591_417_340_918,
    591_417_358_651,
    591_417_376_385,
    591_417_394_118,
    591_417_411_851,
    591_417_429_585,
    591_417_447_318,
    591_417_465_051,
    591_417_482_785,
    591_417_500_518,
    591_417_518_251,
    591_417_535_985,
    591_417_553_718,
    591_417_571_451,
    591_417_589_185,
    591_417_606_918,
    591_417_624_652,
    591_417_642_385,
    591_417_660_118,
    591_417_677_852,
    591_417_695_585,
    591_417_713_319,
    591_417_731_052,
    591_417_748_785,
    591_417_766_519,
    591_417_784_252,
    591_417_801_986,
    591_417_819_719,
    591_417_837_453,
];
