//! Genuine encrypted affine normalization, fixed padding and refusal controls.

use super::*;
use crate::ram_lfe::{
    HiddenRamFheInstruction as Op, HiddenRamFheProgram, RamLfeReferenceInputV1,
    clearing::cleared_cells, ram_lfe_reference_execute_v1, ram_lfe_reference_normalize_v1,
};
use iroha_fhe::bgv::{
    BgvCandidate, BgvParameters, KeySet, Plaintext,
    fixtures::{self, FixtureEntropy},
};
use zeroize::Zeroizing;

fn arithmetic_profile(primes: usize) -> BgvProfile {
    BgvProfile::new(BgvParameters::derive(8, 257, 1, (40, primes), (41, 2), primes.min(2)).unwrap())
        .unwrap()
}

fn tape(instructions: impl IntoIterator<Item = Op>) -> HiddenRamFheProgram {
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for instruction in instructions {
        builder.push(instruction).unwrap();
    }
    builder.finish().unwrap()
}

fn prepared(
    plan: &RamLfeBgvAffinePlanV1,
    program: &HiddenRamFheProgram,
) -> RamLfeBgvAffineProgramV1 {
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Affine, program).unwrap();
    RamLfeBgvAffineProgramV1::from_normal_form(&normal, plan).unwrap()
}

fn keys(profile: &BgvProfile, plan: &RamLfeBgvAffinePlanV1) -> KeySet {
    fixtures::key_set(
        profile,
        plan.galois_elements(),
        &mut FixtureEntropy::new([29; 64]),
    )
    .unwrap()
}

fn encrypted_input(
    profile: &BgvProfile,
    keys: &KeySet,
    input: &RamLfeReferenceInputV1,
) -> Ciphertext {
    let encoding = SlotEncoding::new(profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let mut slots = Zeroizing::new([0_u64; SLOTS]);
    for (to, &from) in slots.iter_mut().zip(input.slots()) {
        *to = u64::from(from);
    }
    fixtures::encrypt(
        profile,
        &keys.public,
        &encoding.encode(profile, &*slots).unwrap(),
        &mut FixtureEntropy::new([31; 64]),
    )
    .unwrap()
}

fn assert_encrypted_matches_reference(
    profile: &BgvProfile,
    plan: &RamLfeBgvAffinePlanV1,
    keys: &KeySet,
    program: &HiddenRamFheProgram,
    input: &RamLfeReferenceInputV1,
    memory: &RamLfeInitialMemoryV1,
) -> CiphertextShape {
    let cipher = encrypted_input(profile, keys, input);
    let output = prepared(plan, program)
        .evaluate(profile, plan, memory, &cipher, &keys.evaluation)
        .unwrap();
    let opened = keys.secret.decrypt(profile, &output).unwrap();
    let slots = SlotEncoding::new(profile, profile.plaintext_precision(1).unwrap())
        .unwrap()
        .decode(&opened)
        .unwrap();
    let reference =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Affine, program, memory, input).unwrap();
    let expected = reference.output().padded_for_opening();
    for (&actual, &expected) in slots[..RAM_LFE_V1_MAX_OUTPUTS]
        .iter()
        .zip(expected.scalars())
    {
        assert_eq!(actual, u64::from(expected));
    }
    assert!(slots[RAM_LFE_V1_MAX_OUTPUTS..].iter().all(|&x| x == 0));
    assert_eq!(&output.shape(), plan.output_shape());
    output.shape()
}

#[test]
fn encrypted_maximum_scale_chain_folds_without_sequential_noise_growth() {
    let profile = arithmetic_profile(3);
    let plan =
        RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let keys = keys(&profile, &plan);
    let input = RamLfeReferenceInputV1::from_bytes(&[37; 63]).unwrap();
    let memory = RamLfeInitialMemoryV1::from_lanes(&[0; 32]).unwrap();
    let long = tape(
        [Op::LoadInput(0, 1)]
            .into_iter()
            .chain(std::iter::repeat_n(Op::MulPlain(0, 0, 128), 254))
            .chain([Op::Output(0)]),
    );
    let reduced = (0..254).fold(1_u64, |a, _| a * 128 % 257);
    let short = tape([
        Op::LoadInput(0, 1),
        Op::MulPlain(0, 0, reduced),
        Op::Output(0),
    ]);
    let normal_long = ram_lfe_reference_normalize_v1(RamLfeClassV1::Affine, &long).unwrap();
    let normal_short = ram_lfe_reference_normalize_v1(RamLfeClassV1::Affine, &short).unwrap();
    assert_eq!(normal_long.output_form(0), normal_short.output_form(0));
    let output = prepared(&plan, &long)
        .evaluate(
            &profile,
            &plan,
            &memory,
            &encrypted_input(&profile, &keys, &input),
            &keys.evaluation,
        )
        .unwrap();
    let reference = prepared(&plan, &short)
        .evaluate(
            &profile,
            &plan,
            &memory,
            &encrypted_input(&profile, &keys, &input),
            &keys.evaluation,
        )
        .unwrap();
    // Equal normalized programs and original ciphertext produce identical
    // complete output ciphertexts, not merely equal cleartext projections.
    assert_eq!(output, reference);
    assert_encrypted_matches_reference(&profile, &plan, &keys, &long, &input, &memory);
    let mut sequential = profile
        .plan_fresh(profile.plaintext_precision(1).unwrap())
        .unwrap();
    let mut rejected = false;
    for _ in 0..254 {
        match profile.plan_mul_plain(
            &sequential,
            Plaintext::constant_bound(&profile, profile.plaintext_precision(1).unwrap()),
        ) {
            Ok(next) => sequential = next,
            Err(BgvError::NoiseBudgetExceeded { .. }) => {
                rejected = true;
                break;
            }
            other => panic!("unexpected planning outcome: {other:?}"),
        }
    }
    assert!(
        rejected,
        "the instruction-by-instruction counterexample remains real"
    );
}

#[test]
fn encrypted_state_aliasing_output_snapshots_and_full_padding_match_reference() {
    let profile = arithmetic_profile(3);
    let plan =
        RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let keys = keys(&profile, &plan);
    let input = RamLfeReferenceInputV1::from_bytes(&[0, 255, 1, 113]).unwrap();
    let memory = RamLfeInitialMemoryV1::from_lanes(&std::array::from_fn::<_, 32, _>(|i| {
        ((i * 31 + 5) % 257) as u16
    }))
    .unwrap();
    let program = tape([
        Op::LoadInput(0, 2),
        Op::Output(0),
        Op::LoadState(1, 31),
        Op::Add(1, 1, 0),
        Op::StoreState(0, 1),
        Op::LoadConst(2, 256),
        Op::SubPlain(2, 2, 256),
        Op::AddPlain(2, 2, 256),
        Op::MulPlain(1, 1, 128),
        Op::LoadState(3, 0),
        Op::Add(3, 3, 1),
        Op::StoreState(31, 3),
        Op::LoadState(0, 31),
        Op::Output(0),
        Op::Output(2),
        Op::SubPlain(0, 0, 19),
        Op::Output(0),
    ]);
    let shape =
        assert_encrypted_matches_reference(&profile, &plan, &keys, &program, &input, &memory);
    let all_outputs = tape(
        [Op::LoadInput(0, 1)]
            .into_iter()
            .chain(std::iter::repeat_n(Op::Output(0), 64)),
    );
    assert_eq!(
        shape,
        assert_encrypted_matches_reference(&profile, &plan, &keys, &all_outputs, &input, &memory)
    );
    let zero = tape([Op::Output(0)]);
    assert_eq!(
        shape,
        assert_encrypted_matches_reference(&profile, &plan, &keys, &zero, &input, &memory)
    );
    assert_eq!(plan.galois_applications(), 22);
    assert_eq!(plan.galois_elements().len(), 7);
}

#[test]
fn public_plan_matches_independent_closed_noise_recurrence_at_candidate_size() {
    let current = BgvProfile::candidate(BgvCandidate::AffineF257).unwrap();
    let proposed =
        BgvProfile::new(BgvParameters::derive(14, 257, 1, (50, 4), (60, 2), 2).unwrap()).unwrap();
    // The existing C.3 four-prime proposal is tested arithmetically only. It
    // is not selected, and the missing sanitizer has no noise allowance here.
    assert_eq!(current.top_level(), 5);
    assert_eq!(proposed.top_level(), 3);
    assert_ne!(current.identity(), proposed.identity());
    for profile in [current, proposed] {
        let plan =
            RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
        let encoding =
            SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
        assert_eq!(
            (plan.routing.baby_count(), plan.routing.giant_count()),
            (8, 16)
        );
        assert_eq!(plan.galois_applications(), 22);
        assert_eq!(plan.routing.plaintext_products() + 2, 130);
        let l = encoding.vector_bound().l1;
        let mask = encoding.public_prefix_mask_bound(64).unwrap();
        assert_eq!((mask.linf, mask.l1), (128, 4494));
        let k = profile.key_switch_noise(profile.top_level()).unwrap();
        let augmented = profile.fresh_noise().mul_u64(mask.l1).add_u64(128);
        let expected = augmented
            .mul_u64(128)
            .add(&k.mul_u64(192))
            .mul_u64(l)
            .add(&k.mul_u64(15))
            .add_u64(128)
            .mul_u64(mask.l1);
        assert_eq!(plan.output_shape().bound(), &expected);
        assert!(plan.output_shape().bound() < profile.capacity(profile.top_level()).unwrap());
        assert_eq!(plan.output_shape().level(), profile.top_level());
        assert_eq!(plan.output_shape().components(), 2);
    }
}

#[test]
fn non_affine_profiles_and_short_chains_refuse_before_private_evaluation() {
    let profile = arithmetic_profile(3);
    let plan =
        RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let program = tape([Op::LoadInput(0, 1), Op::Mul(0, 0, 0), Op::Output(0)]);
    let bounded = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    let before = cleared_cells();
    assert_eq!(
        RamLfeBgvAffineProgramV1::from_normal_form(&bounded, &plan).unwrap_err(),
        RamLfeBgvAffineErrorV1::ClassNotAffine
    );
    assert_eq!(cleared_cells(), before);
    let small =
        BgvProfile::new(BgvParameters::derive(6, 257, 1, (40, 3), (41, 2), 2).unwrap()).unwrap();
    assert_eq!(
        RamLfeBgvAffinePlanV1::new(&small, small.plaintext_precision(1).unwrap()).unwrap_err(),
        RamLfeBgvAffineErrorV1::UnsupportedProfile
    );
    let other =
        BgvProfile::new(BgvParameters::derive(8, 65537, 1, (40, 3), (41, 2), 2).unwrap()).unwrap();
    assert_eq!(
        RamLfeBgvAffinePlanV1::new(&other, other.plaintext_precision(1).unwrap()).unwrap_err(),
        RamLfeBgvAffineErrorV1::UnsupportedProfile
    );
    assert!(matches!(
        {
            let short = arithmetic_profile(1);
            RamLfeBgvAffinePlanV1::new(&short, short.plaintext_precision(1).unwrap())
        },
        Err(RamLfeBgvAffineErrorV1::Routing(SlotLinearError::Bgv(
            BgvError::NoiseBudgetExceeded { .. }
        )))
    ));
}

#[test]
fn wrong_input_shape_profile_and_missing_keys_refuse() {
    let profile = arithmetic_profile(3);
    let plan =
        RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let keys = keys(&profile, &plan);
    let input = RamLfeReferenceInputV1::from_bytes(&[8]).unwrap();
    let cipher = encrypted_input(&profile, &keys, &input);
    let memory = RamLfeInitialMemoryV1::from_lanes(&[1; 32]).unwrap();
    let program = prepared(&plan, &tape([Op::LoadInput(0, 1), Op::Output(0)]));
    let doubled = profile.add(&cipher, &cipher).unwrap();
    assert_eq!(
        program
            .evaluate(&profile, &plan, &memory, &doubled, &keys.evaluation)
            .unwrap_err(),
        RamLfeBgvAffineErrorV1::InputShape
    );
    let empty = fixtures::evaluation_keys(
        &profile,
        &keys.secret,
        &[],
        &mut FixtureEntropy::new([41; 64]),
    )
    .unwrap();
    assert_eq!(
        program
            .evaluate(&profile, &plan, &memory, &cipher, &empty)
            .unwrap_err(),
        RamLfeBgvAffineErrorV1::Bgv(BgvError::WrongSwitchingKey)
    );
    let another = arithmetic_profile(4);
    assert_eq!(
        program
            .evaluate(&another, &plan, &memory, &cipher, &keys.evaluation)
            .unwrap_err(),
        RamLfeBgvAffineErrorV1::Bgv(BgvError::ProfileMismatch)
    );
}

#[test]
fn prepared_mapping_redacts_owner_and_clears_its_temporary_copy() {
    let profile = arithmetic_profile(3);
    let plan =
        RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let program = tape([Op::LoadConst(0, 197), Op::Output(0)]);
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Affine, &program).unwrap();
    let before = cleared_cells();
    let owner = RamLfeBgvAffineProgramV1::from_normal_form(&normal, &plan).unwrap();
    assert_eq!(
        format!("{owner:?}"),
        "[REDACTED RAM-LFE affine BGV program]"
    );
    assert_eq!(plan.routing.coefficient_count(), WORDS);
    // The wrapper temporary is cleared before returning. The sole persistent
    // owner's drop/error/unwind behavior is exercised by shared routing tests.
    assert_eq!(cleared_cells() - before, WORDS);
    drop(owner);
    assert_eq!(cleared_cells() - before, WORDS);
    let failure = || -> Result<(), RamLfeBgvAffineErrorV1> {
        let _owner = RamLfeBgvAffineProgramV1::from_normal_form(&normal, &plan)?;
        Err(RamLfeBgvAffineErrorV1::InputShape)
    };
    assert!(failure().is_err());
    assert_eq!(cleared_cells() - before, WORDS * 2);
    assert!(
        std::panic::catch_unwind(|| {
            let _owner = RamLfeBgvAffineProgramV1::from_normal_form(&normal, &plan).unwrap();
            panic!("test private affine owner unwind");
        })
        .is_err()
    );
    assert_eq!(cleared_cells() - before, WORDS * 3);
}

#[test]
fn dense_maximum_tape_uses_every_input_slot_and_reduced_lane_constants() {
    let profile = arithmetic_profile(3);
    let plan =
        RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let keys = keys(&profile, &plan);
    let bytes = std::array::from_fn::<_, 63, _>(|i| ((i * 43 + 7) % 256) as u8);
    let input = RamLfeReferenceInputV1::from_bytes(&bytes).unwrap();
    let memory = RamLfeInitialMemoryV1::from_lanes(&std::array::from_fn::<_, 32, _>(|i| {
        ((i * 19 + 197) % 257) as u16
    }))
    .unwrap();
    let mut instructions = vec![Op::LoadConst(0, 0)];
    let mut expected = 256_u64;
    for slot in 0..64_u16 {
        let coefficient = (u64::from(slot) * 37 + 256) % 257;
        instructions.extend([
            Op::LoadInput(1, slot),
            Op::MulPlain(1, 1, coefficient),
            Op::Add(0, 0, 1),
        ]);
        expected = (expected + coefficient * u64::from(input.slots()[usize::from(slot)])) % 257;
    }
    for lane in [0_u16, 1, 16, 31] {
        instructions.extend([Op::LoadState(2, lane), Op::Add(0, 0, 2)]);
        expected = (expected + u64::from(memory.lanes()[usize::from(lane)])) % 257;
    }
    instructions.extend([
        Op::AddPlain(0, 0, 256),
        Op::MulPlain(0, 0, 256),
        Op::MulPlain(0, 0, 256),
    ]);
    while instructions.len() < 255 {
        instructions.push(Op::AddPlain(0, 0, 0));
    }
    instructions.push(Op::Output(0));
    assert_eq!(instructions.len(), 256);
    let program = tape(instructions);
    let clear =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Affine, &program, &memory, &input).unwrap();
    assert_eq!(clear.output().scalars(), &[expected as u16]);
    assert_encrypted_matches_reference(&profile, &plan, &keys, &program, &input, &memory);
}

#[test]
fn one_prepared_map_uses_current_lanes_and_ignores_unused_input_slots() {
    let profile = arithmetic_profile(3);
    let plan =
        RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let keys = keys(&profile, &plan);
    let input = RamLfeReferenceInputV1::from_bytes(&[37, 255, 0]).unwrap();
    let program = tape([
        Op::LoadState(0, 31),
        Op::LoadInput(1, 1),
        Op::Add(0, 0, 1),
        Op::Output(0),
        Op::LoadState(0, 0),
        Op::Output(0),
        Op::LoadConst(0, 256),
        Op::Output(0),
    ]);
    let owner = prepared(&plan, &program);
    let mut slots = Zeroizing::new([0_u64; SLOTS]);
    for (slot, &value) in slots[..64].iter_mut().zip(input.slots()) {
        *slot = u64::from(value);
    }
    for (index, slot) in slots[64..].iter_mut().enumerate() {
        *slot = ((index * 29 + 256) % 257) as u64;
    }
    let encoding = SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let cipher = fixtures::encrypt(
        &profile,
        &keys.public,
        &encoding.encode(&profile, &*slots).unwrap(),
        &mut FixtureEntropy::new([149; 64]),
    )
    .unwrap();
    let memory_a = RamLfeInitialMemoryV1::from_lanes(&std::array::from_fn::<_, 32, _>(|i| {
        ((i * 13 + 256) % 257) as u16
    }))
    .unwrap();
    let memory_b = RamLfeInitialMemoryV1::from_lanes(&std::array::from_fn::<_, 32, _>(|i| {
        ((i * 31 + 17) % 257) as u16
    }))
    .unwrap();
    let mut first = None;
    for (run, memory) in [&memory_a, &memory_b, &memory_a].into_iter().enumerate() {
        let result = owner
            .evaluate(&profile, &plan, memory, &cipher, &keys.evaluation)
            .unwrap();
        let opened = keys.secret.decrypt(&profile, &result).unwrap();
        let actual = encoding.decode(&opened).unwrap();
        let reference =
            ram_lfe_reference_execute_v1(RamLfeClassV1::Affine, &program, memory, &input).unwrap();
        let padded = reference.output().padded_for_opening();
        for (&value, &expected) in actual[..64].iter().zip(padded.scalars()) {
            assert_eq!(value, u64::from(expected));
        }
        assert!(actual[64..].iter().all(|&x| x == 0));
        assert_eq!(actual[0], (u64::from(memory.lanes()[31]) + 37) % 257);
        assert_eq!(actual[1], u64::from(memory.lanes()[0]));
        assert_eq!(actual[2], 256);
        assert_eq!(&result.shape(), plan.output_shape());
        if run == 0 {
            first = Some(result);
        } else if run == 2 {
            assert_eq!(Some(result), first);
        }
    }
}

#[test]
fn prepared_map_profile_mismatch_refuses_before_reading_current_lanes() {
    let profile = arithmetic_profile(3);
    let plan =
        RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let other = arithmetic_profile(4);
    let other_plan =
        RamLfeBgvAffinePlanV1::new(&other, other.plaintext_precision(1).unwrap()).unwrap();
    let program = tape([Op::LoadState(0, 31), Op::Output(0)]);
    let owner = prepared(&other_plan, &program);
    let keys = keys(&profile, &plan);
    let input = RamLfeReferenceInputV1::from_bytes(&[17]).unwrap();
    let cipher = encrypted_input(&profile, &keys, &input);
    let memory = RamLfeInitialMemoryV1::from_lanes(&[256; 32]).unwrap();
    let before = cleared_cells();
    assert_eq!(
        owner
            .evaluate(&profile, &plan, &memory, &cipher, &keys.evaluation)
            .unwrap_err(),
        RamLfeBgvAffineErrorV1::Bgv(BgvError::ProfileMismatch)
    );
    assert_eq!(
        cleared_cells(),
        before,
        "no request initialization buffer existed"
    );
}

#[test]
fn effective_f257_affine_under_prime_power_masters_matches_current_memory_and_reference() {
    let program = tape([
        Op::LoadInput(0, 1),
        Op::Output(0),
        Op::LoadState(1, 31),
        Op::Add(1, 1, 0),
        Op::StoreState(0, 1),
        Op::MulPlain(1, 1, 128),
        Op::LoadState(2, 0),
        Op::Add(2, 2, 1),
        Op::Output(2),
        Op::SubPlain(1, 1, 256),
        Op::Output(1),
    ]);
    let input = RamLfeReferenceInputV1::from_bytes(&[37, 255, 0, 128]).unwrap();
    for master_exponent in [2, 3] {
        let profile = BgvProfile::new(
            BgvParameters::derive(8, 257, master_exponent, (50, 4), (60, 2), 2).unwrap(),
        )
        .unwrap();
        let precision = profile.plaintext_precision(1).unwrap();
        let plan = RamLfeBgvAffinePlanV1::new(&profile, precision).unwrap();
        assert_eq!(plan.input.plaintext_precision(), precision);
        assert_eq!(
            plan.input.bound(),
            &iroha_fhe::natural::Natural::from_u64(
                128 + profile.master_plaintext_modulus() * 21 * (2 * profile.degree() as u64 + 1)
            )
        );
        let keys = keys(&profile, &plan);
        for seed in [7_u16, 191, 7] {
            let lanes = std::array::from_fn::<_, 32, _>(|i| (seed + i as u16 * 3) % 257);
            let memory = RamLfeInitialMemoryV1::from_lanes(&lanes).unwrap();
            let shape = assert_encrypted_matches_reference(
                &profile, &plan, &keys, &program, &input, &memory,
            );
            assert_eq!(shape.plaintext_precision(), precision);
        }
        let before = cleared_cells();
        assert_eq!(
            RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(2).unwrap())
                .unwrap_err(),
            RamLfeBgvAffineErrorV1::UnsupportedProfile
        );
        let other = arithmetic_profile(3);
        assert_eq!(
            RamLfeBgvAffinePlanV1::new(&profile, other.plaintext_precision(1).unwrap())
                .unwrap_err(),
            RamLfeBgvAffineErrorV1::Bgv(BgvError::ProfileMismatch)
        );
        assert_eq!(cleared_cells(), before);
    }
}
