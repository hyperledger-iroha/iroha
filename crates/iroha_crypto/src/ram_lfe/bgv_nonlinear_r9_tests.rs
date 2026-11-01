//! Fixed Refresh-class public plans, admission controls and genuine encrypted runs.
//!
//! The ignored diagnostics use unselected N128/F257/E3 parameters. They do not
//! establish a selected profile, KDM security, execution proof or production cost.

use super::*;
use crate::ram_lfe::{
    HiddenRamFheInstruction as Op, HiddenRamFheProgram, RamLfeClassV1, RamLfeNormalGateKindV1,
    RamLfeReferenceInputV1, clearing::cleared_cells, ram_lfe_reference_execute_v1,
    ram_lfe_reference_normalize_v1,
};
use iroha_fhe::{
    bgv::{LocalBootstrapSetupBudget, SecretKey},
    natural::Natural,
};
use std::{cell::RefCell, time::Instant};
use zeroize::Zeroizing;

fn schedule() -> RamLfeBgvNonlinearScheduleV1 {
    let mut schedule = tests::refresh_fixture_schedule();
    schedule.input_level = 0;
    schedule.layers = RamLfeBgvNonlinearLayersV1::Refresh(Box::new(
        [RamLfeBgvNonlinearLayerChoiceV1 {
            routing_level: 4,
            multiplication_level: 1,
            routing_scratch_bytes: 143_360,
        }; 3],
    ));
    schedule.projection_scratch_bytes = 143_360;
    schedule
}
fn budget() -> RamLfeBgvNonlinearBudgetV1 {
    RamLfeBgvNonlinearBudgetV1 {
        maximum_refresh_plans: 640,
        preparation_payload_bytes: 64 << 20,
        preparation_metadata_bytes: 64 << 20,
        canonical_arithmetic_units: 73_234_637_760,
        scalar_word_operations: 5_767_183_744,
        matrix_payload_bytes: RamLfeBgvNonlinearPlanV1::matrix_payload_bytes(
            RamLfeBgvNonlinearShapeV1::Refresh,
        ),
        layout_metadata_bytes: RamLfeBgvNonlinearPlanV1::layout_metadata_bytes(
            RamLfeBgvNonlinearShapeV1::Refresh,
        )
        .unwrap(),
        preparation_scratch_bytes: RamLfeBgvNonlinearShapeV1::Refresh.preparation_scratch_bytes(),
        owned_residue_bytes: 16 << 20,
    }
}
fn tape(operations: impl IntoIterator<Item = Op>) -> HiddenRamFheProgram {
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for op in operations {
        builder.push(op).unwrap();
    }
    builder.finish().unwrap()
}
fn maximum() -> HiddenRamFheProgram {
    let mut ops = vec![Op::SelectEqZero(0, 0, 0, 0); 65];
    ops.extend(std::iter::repeat_n(Op::SelectEqZero(0, 1, 0, 0), 6));
    ops.extend(std::iter::repeat_n(Op::SelectEqZero(2, 3, 3, 3), 184));
    ops.push(Op::Output(0));
    tape(ops)
}
fn nonzero() -> HiddenRamFheProgram {
    let mut ops = vec![Op::LoadState(0, 31), Op::LoadState(1, 0)];
    ops.extend(std::iter::repeat_n(Op::SelectEqZero(0, 0, 0, 0), 65));
    ops.extend(std::iter::repeat_n(Op::SelectEqZero(0, 2, 0, 0), 6));
    ops.extend(std::iter::repeat_n(Op::SelectEqZero(3, 2, 1, 1), 182));
    ops.push(Op::Output(0));
    tape(ops)
}
fn plan_data(plan: &RamLfeBgvNonlinearPlanV1) -> &PlanData<4, 3, 3, 1> {
    match &plan.data {
        PlanKind::Refresh(data) => data,
        _ => panic!("refresh fixture plan"),
    }
}
fn setup(profile: &BgvProfile, plan: &RamLfeBgvNonlinearPlanV1) -> (SecretKey, LocalBootstrapKey) {
    // Explicit finite setup ceilings for the original N128/q20 fixture and the
    // separate unselected N256/q23 diagnostic. Neither is a protocol policy.
    let budget = if profile.degree() == 128 {
        LocalBootstrapSetupBudget {
            retained_residue_bytes: 3_686_400,
            returned_secret_bytes: 23_552,
            plaintext_bytes: 1024,
        }
    } else {
        LocalBootstrapSetupBudget {
            retained_residue_bytes: 64 << 20,
            returned_secret_bytes: 1 << 20,
            plaintext_bytes: 2048,
        }
    };
    LocalBootstrapKey::generate(profile, plan.galois_elements(), budget).unwrap()
}

fn memory() -> RamLfeInitialMemoryV1 {
    RamLfeInitialMemoryV1::from_lanes(&std::array::from_fn::<_, 32, _>(|i| {
        u16::try_from((i * 19 + 7) % 257).unwrap()
    }))
    .unwrap()
}
fn assert_phase(
    profile: &BgvProfile,
    secret: &SecretKey,
    value: &LocalBootstrapCiphertext<'_>,
    slots: &[u64; 128],
) {
    let scalar_coefficients = independent_refresh_tests::expected_coefficients(slots);
    let degree = profile.degree();
    let stride = degree / 128;
    let coefficients: Vec<_> = (0..degree)
        .map(|i| {
            if i % stride == 0 {
                scalar_coefficients[i / stride]
            } else {
                0
            }
        })
        .collect();
    if degree == 128 {
        independent_refresh_tests::assert_independent_phase(
            profile,
            secret,
            value.ciphertext(),
            &scalar_coefficients,
        );
    } else {
        independent_refresh_tests::assert_independent_phase_at_degree(
            profile,
            secret,
            value.ciphertext(),
            &coefficients,
        );
    }
    assert_eq!(
        secret
            .decrypt(profile, value.ciphertext())
            .unwrap()
            .coefficients(),
        coefficients
    );
}

// The full fixture moves its synthetic secret into one test-only owner. A
// panic/error drops that owner through BoundaryGuard; no secret is cloned or
// exposed to production evaluation. The hook observes the canonical evaluator.
struct BoundaryOracle {
    secret: SecretKey,
    gates: Zeroizing<Vec<u64>>,
    base: Zeroizing<[u64; 128]>,
    seen: Vec<u16>,
}
thread_local! { static BOUNDARIES: RefCell<Option<BoundaryOracle>> = const { RefCell::new(None) }; }
struct BoundaryGuard;
impl BoundaryGuard {
    fn install(
        secret: SecretKey,
        normal: &RamLfeNormalFormV1,
        memory: &RamLfeInitialMemoryV1,
        input: &RamLfeReferenceInputV1,
    ) -> Self {
        let mut values = Zeroizing::new(vec![0_u64; 416]);
        values[0] = 1;
        for (to, &from) in values[1..65].iter_mut().zip(input.slots()) {
            *to = u64::from(from);
        }
        for (to, &from) in values[65..97].iter_mut().zip(memory.lanes()) {
            *to = u64::from(from);
        }
        for gate in 0..normal.gate_count() {
            let operands = std::array::from_fn::<_, 3, _>(|operand| {
                normal.gate_operand(gate, operand).map_or(0, |form| {
                    form.iter()
                        .zip(&*values)
                        .fold(0, |sum, (&coefficient, &value)| {
                            (sum + u64::from(coefficient) * value) % 257
                        })
                })
            });
            values[97 + gate] = match normal.gate_kind(gate).unwrap() {
                RamLfeNormalGateKindV1::Mul => operands[0] * operands[1] % 257,
                RamLfeNormalGateKindV1::SelectEqZero => {
                    if operands[0] == 0 {
                        operands[1]
                    } else {
                        operands[2]
                    }
                }
                RamLfeNormalGateKindV1::Refresh => operands[0],
            };
        }
        let base = Zeroizing::new(std::array::from_fn::<_, 128, _>(|i| {
            if i < 64 {
                u64::from(input.slots()[i])
            } else if i < 96 {
                u64::from(memory.lanes()[i - 64])
            } else {
                u64::from(i == 96)
            }
        }));
        BOUNDARIES.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(slot.is_none());
            *slot = Some(BoundaryOracle {
                secret,
                gates: values,
                base,
                seen: Vec::new(),
            });
        });
        Self
    }
    #[expect(
        clippy::unused_self,
        reason = "Consuming this Drop guard keeps its cleanup lifetime through oracle extraction and prevents a second finish."
    )]
    fn finish(self) -> (SecretKey, Vec<u16>) {
        let BoundaryOracle {
            secret,
            gates,
            base,
            seen,
        } = BOUNDARIES.with(|slot| slot.borrow_mut().take().unwrap());
        drop(gates);
        drop(base);
        (secret, seen)
    }
}
impl Drop for BoundaryGuard {
    fn drop(&mut self) {
        BOUNDARIES.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}
pub(super) fn observe_boundary(
    profile: &BgvProfile,
    stage: RamLfeBgvNonlinearStageV1,
    banks: &[StateBank<'_>],
) {
    let RamLfeBgvNonlinearStageV1::RefreshGate {
        gate,
        mode: RamLfeBgvRefreshMapV1::Final,
    } = stage
    else {
        return;
    };
    if ![127, 128, 255, 256, 318].contains(&gate) {
        return;
    }
    BOUNDARIES.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(oracle) = slot.as_mut() else {
            return;
        };
        assert_eq!(banks.len(), 4);
        for (bank, value) in banks.iter().take(3).enumerate() {
            let expected = Zeroizing::new(std::array::from_fn::<_, 128, _>(|i| {
                let at = bank * 128 + i;
                if at <= usize::from(gate) {
                    oracle.gates[97 + at]
                } else {
                    0
                }
            }));
            assert_phase(profile, &oracle.secret, value.as_local(), &expected);
        }
        assert_phase(profile, &oracle.secret, banks[3].as_local(), &oracle.base);
        oracle.seen.push(gate);
    });
}

#[test]
fn refresh_public_plan_reuses_one_core_with640_exact_sources_and_all2871_steps() {
    let profile = tests::refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    tests::reset_counts();
    let plan = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule(), budget()).unwrap();
    let data = plan_data(&plan);
    let pool = data.refresh.as_ref().unwrap();
    assert_eq!(plan.shape(), RamLfeBgvNonlinearShapeV1::Refresh);
    assert_eq!(pool.plans.len(), 640);
    assert_eq!(tests::public_preparations(), 1);
    assert_eq!(data.layers.len(), 957);
    assert_eq!(
        data.layers.iter().map(|p| p.repetitions).sum::<usize>(),
        2871
    );
    assert_eq!(
        data.layers
            .iter()
            .filter(|p| matches!(p.coefficients, Coefficients::Public(_)))
            .count(),
        319
    );
    assert_eq!(
        data.layers
            .iter()
            .filter(|p| matches!(p.coefficients, Coefficients::Private(_)))
            .count(),
        638
    );
    assert_eq!(plan.operations().bootstrap_calls, 2872);
    assert_eq!(plan.operations().ciphertext_products, 2871);
    assert_eq!(plan.operations().routing_calls, 2872);
    assert_eq!(plan.operations().modulus_switches, 28_714);
    assert_eq!(data.initial_refresh, [None, None, None, Some(0)]);
    for (ordinal, layer) in data.layers.iter().enumerate() {
        let target = (ordinal / 3) / 128;
        assert_eq!(layer.products.updates, [target]);
        assert_eq!(
            layer
                .after_refresh
                .iter()
                .filter(|index| index.is_some())
                .count(),
            1
        );
        assert!(layer.after_refresh[target].is_some());
        // The canonical source descriptors remain mixed: untouched zero banks
        // have their actual trivial bound until first used as a target.
        for untouched_future in &layer.products.state_outputs[target + 1..] {
            assert_eq!(untouched_future.bound(), &Natural::from_u64(128));
            assert_eq!(untouched_future.level(), 4);
        }
    }
    assert_eq!(plan.canonical_arithmetic_units(), 73_234_637_760);
    assert_eq!(plan.scalar_word_operations(), 5_767_183_744);
    assert_eq!(plan.output_shape().level(), 0);
    assert_eq!(
        plan.output_shape().bound(),
        &Natural::from_u64(1_094_861_248)
    );
    assert_eq!(
        plan.preparation_requirement().retained_payload_bytes,
        24_003_088
    );
    assert_eq!(
        plan.preparation_requirement().peak_payload_bytes,
        24_855_250
    );
    for (index, descriptor) in pool.plans.iter().enumerate() {
        assert!(pool.plans[0].shares_preparation(descriptor));
        assert_eq!(
            descriptor.message_space(),
            iroha_fhe::bgv::BootstrapMessageSpace::ScalarSlots
        );
        for earlier in &pool.plans[..index] {
            assert_ne!(descriptor.input_shape(), earlier.input_shape());
        }
    }
    assert_eq!(
        tests::actual_counts(),
        RamLfeBgvNonlinearOperationsV1::default()
    );
    tests::reset_counts();
}

#[test]
fn refresh_exact_aggregate_caps_and_source_count_refuse_one_under_before_private_work() {
    let profile = tests::refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let schedule = schedule();
    let plan = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, budget()).unwrap();
    let exact = RamLfeBgvNonlinearBudgetV1 {
        preparation_payload_bytes: plan.preparation_requirement().peak_payload_bytes,
        preparation_metadata_bytes: plan.preparation_requirement().peak_metadata_bytes,
        owned_residue_bytes: plan.owned_residue_bytes(),
        ..budget()
    };
    RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, exact).unwrap();
    let before = cleared_cells();
    for (case, mut cap) in (0..6).map(|case| (case, exact)) {
        match case {
            0 => cap.preparation_payload_bytes -= 1,
            1 => cap.preparation_metadata_bytes -= 1,
            2 => cap.canonical_arithmetic_units -= 1,
            3 => cap.scalar_word_operations -= 1,
            4 => cap.maximum_refresh_plans -= 1,
            _ => cap.owned_residue_bytes -= 1,
        }
        tests::reset_counts();
        let error = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, cap).unwrap_err();
        match case {
            0 | 1 => assert!(matches!(
                error,
                RamLfeBgvNonlinearErrorV1::PreparationBudget { .. }
            )),
            2 | 3 => assert!(matches!(
                error,
                RamLfeBgvNonlinearErrorV1::WorkBudget { .. }
            )),
            4 => assert_eq!(
                error,
                RamLfeBgvNonlinearErrorV1::RefreshPlanBudget {
                    required: 640,
                    budget: 639
                }
            ),
            _ => assert!(matches!(
                error,
                RamLfeBgvNonlinearErrorV1::ResidueBudget { .. }
            )),
        }
        assert_eq!(cleared_cells(), before);
        assert_eq!(
            tests::actual_counts(),
            RamLfeBgvNonlinearOperationsV1::default()
        );
        assert_eq!(tests::public_preparations(), usize::from(case > 1));
    }
    for mutate in [0, 1, 2] {
        let mut invalid = schedule.clone();
        let mut cap = exact;
        match mutate {
            0 => {
                invalid.refresh = None;
                cap.maximum_refresh_plans = 0
            }
            1 => cap.maximum_refresh_plans = 2873,
            _ => invalid.input_level = usize::MAX,
        }
        tests::reset_counts();
        assert!(RamLfeBgvNonlinearPlanV1::new(&profile, precision, &invalid, cap).is_err());
        assert_eq!(tests::public_preparations(), 0);
        assert_eq!(cleared_cells(), before);
    }
    tests::reset_counts();
}

#[test]
fn prepared_program_binds_original_class_and_exact_plan_before_input_or_memory_reads() {
    let profile = tests::refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let schedule = schedule();
    let first = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, budget()).unwrap();
    let second = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, budget()).unwrap();
    assert_eq!(first.output_shape(), second.output_shape());
    assert_eq!(first.operations(), second.operations());
    let program = tape([Op::Output(0)]);
    let wrong = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    let before = cleared_cells();
    assert!(matches!(
        RamLfeBgvNonlinearProgramV1::from_normal_form(&wrong, &first),
        Err(RamLfeBgvNonlinearErrorV1::Compile(
            RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
        ))
    ));
    assert_eq!(cleared_cells(), before);
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Refresh, &program).unwrap();
    let prepared = RamLfeBgvNonlinearProgramV1::from_normal_form(&normal, &first).unwrap();
    let (_, key) = setup(&profile, &first);
    let input = RamLfeReferenceInputV1::from_bytes(&[255; 63]).unwrap();
    let packed = RamLfeBgvPackedInputV1::encrypt(&profile, precision, 0, &key, &input).unwrap();
    // Keep caller-owned memory alive across the refusal observation; dropping
    // an inline temporary would itself clear 32 cells after the call.
    let lanes = memory();
    let before = cleared_cells();
    tests::reset_counts();
    assert_eq!(
        prepared
            .evaluate(&profile, &second, &lanes, packed, &key)
            .unwrap_err(),
        RamLfeBgvNonlinearErrorV1::ProgramPlan
    );
    assert_eq!(cleared_cells(), before);
    assert_eq!(
        tests::actual_counts(),
        RamLfeBgvNonlinearOperationsV1::default()
    );
    tests::reset_counts();
}

#[test]
fn refresh_stage_target_and_bank_order_refuse_before_arithmetic_or_private_rows() {
    let profile = tests::refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let mut plan =
        RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule(), budget()).unwrap();
    let normal =
        ram_lfe_reference_normalize_v1(RamLfeClassV1::Refresh, &tape([Op::Output(0)])).unwrap();
    let mut prepared = RamLfeBgvNonlinearProgramV1::from_normal_form(&normal, &plan).unwrap();
    let (_, key) = setup(&profile, &plan);
    let input = RamLfeReferenceInputV1::from_bytes(&[7]).unwrap();
    let memory = memory();
    let ProgramKind::Refresh(private) = &mut prepared.data else {
        unreachable!()
    };
    let PlanKind::Refresh(data) = &mut plan.data else {
        unreachable!()
    };
    let original_stage = private.layers[0].stage;
    for case in 0..3 {
        match case {
            0 => private.layers[0].stage = stage(RamLfeBgvNonlinearShapeV1::Refresh, 3),
            1 => data.layers[0].stage = stage(RamLfeBgvNonlinearShapeV1::Refresh, 3),
            _ => data.layers[0].products.updates[0] = 1,
        }
        let packed = RamLfeBgvPackedInputV1::encrypt(&profile, precision, 0, &key, &input).unwrap();
        let before = cleared_cells();
        tests::reset_counts();
        let error = private
            .evaluate(&profile, data, &memory, packed, &key)
            .unwrap_err();
        assert_eq!(
            error,
            if case == 2 {
                RamLfeBgvNonlinearErrorV1::Geometry
            } else {
                RamLfeBgvNonlinearErrorV1::ProgramPlan
            }
        );
        assert_eq!(cleared_cells(), before);
        assert_eq!(
            tests::actual_counts(),
            RamLfeBgvNonlinearOperationsV1::default()
        );
        private.layers[0].stage = original_stage;
        data.layers[0].stage = original_stage;
        data.layers[0].products.updates[0] = 0;
    }
    private.check_layers(data).unwrap();
    let packed = RamLfeBgvPackedInputV1::encrypt(&profile, precision, 0, &key, &input).unwrap();
    let (original, mut banks) = initialize_state(&profile, data, &memory, packed, &key).unwrap();
    drop(original);
    assert_eq!(banks[0].shape(), banks[1].shape());
    banks.swap(0, 1);
    let before = cleared_cells();
    tests::reset_counts();
    assert_eq!(
        execute_layer(&profile, &data.layers[0], &private.layers[0], banks, &key).unwrap_err(),
        RamLfeBgvNonlinearErrorV1::Geometry
    );
    assert_eq!(cleared_cells(), before);
    assert_eq!(
        tests::actual_counts(),
        RamLfeBgvNonlinearOperationsV1::default()
    );
    tests::reset_counts();
}

#[test]
#[ignore = "two encrypted Refresh first-gate cases: 20 bootstraps/72 phase checks; run before full2871step diagnostics"]
fn refresh_class_first_gate_matches_select_and_independent_full_ring_phases() {
    let profile = tests::refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let plan = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule(), budget()).unwrap();
    first_gate_phase_witness(&profile, &plan);
}

fn scalar_subalgebra_fixture() -> (BgvProfile, RamLfeBgvNonlinearPlanV1) {
    let profile = BgvProfile::new(
        iroha_fhe::bgv::BgvParameters::derive(8, 257, 3, (60, 23), (60, 2), 2).unwrap(),
    )
    .unwrap();
    let precision = profile.plaintext_precision(1).unwrap();
    let mut schedule = schedule();
    let choice = schedule.refresh.as_mut().unwrap();
    // Independently replayed full 2871-step public recurrence; this is an
    // unselected diagnostic profile, not a parameter qualification.
    choice.levels = vec![
        20, 19, 18, 17, 16, 15, 14, 13, 13, 11, 10, 9, 8, 7, 6, 5, 4, 4,
    ];
    choice.budget.decomposition.trace_plaintext_bytes *= 2;
    choice.budget.recomposition.trace_plaintext_bytes *= 2;
    choice.budget.upper_digits.plaintext_bytes *= 2;
    choice.budget.input_plaintext_bytes *= 2;
    choice.budget.stream_owned_residue_bytes = 64 << 20;
    choice.budget.input_owned_bytes = 16 << 20;
    choice.budget.input_arithmetic_units = 1_u64 << 40;
    choice.budget.input_addition_words = 1_u64 << 30;
    choice.budget.canonical_arithmetic_units = 1_u64 << 50;
    choice.budget.scalar_word_operations = 1_u64 << 40;
    if let RamLfeBgvNonlinearLayersV1::Refresh(layers) = &mut schedule.layers {
        for layer in layers.iter_mut() {
            layer.routing_scratch_bytes *= 2;
        }
    }
    schedule.projection_scratch_bytes *= 2;
    let mut cap = budget();
    cap.canonical_arithmetic_units = 1_u64 << 60;
    cap.scalar_word_operations = 1_u64 << 50;
    cap.preparation_payload_bytes *= 2;
    cap.preparation_metadata_bytes *= 2;
    cap.maximum_refresh_plans = 2;
    cap.owned_residue_bytes = 128 << 20;
    let plan = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, cap).unwrap();
    assert_eq!(plan.prepared_refresh_plans(), 2);
    assert_eq!(plan.operations().bootstrap_calls, 2872);
    assert_eq!(plan.operations().routing_calls, 2872);
    assert_eq!(plan.operations().ciphertext_products, 2871);
    assert_eq!(plan.operations().modulus_switches, 28_714);
    assert_eq!(
        plan.output_shape().bound(),
        &Natural::from_u64(2_181_235_200)
    );
    let pool = plan_data(&plan).refresh.as_ref().unwrap();
    let mut source_bounds = pool
        .plans
        .iter()
        .map(|descriptor| descriptor.input_shape().bound().clone())
        .collect::<Vec<_>>();
    source_bounds.sort();
    assert_eq!(
        source_bounds,
        [2_181_235_328, 2_181_237_313].map(Natural::from_u64)
    );
    for descriptor in &pool.plans {
        assert_eq!(
            descriptor.message_space(),
            iroha_fhe::bgv::BootstrapMessageSpace::ScalarSlots
        );
        assert!(pool.plans[0].shares_preparation(descriptor));
    }
    (profile, plan)
}

#[test]
#[ignore = "N256 scalar first-gate cases: 20 bootstraps and 72 all-coefficient phase checks"]
fn refresh_scalar_subalgebra_first_gate_preserves_zero_off_stride_and_every_bank() {
    let (profile, plan) = scalar_subalgebra_fixture();
    first_gate_phase_witness(&profile, &plan);
}

fn first_gate_phase_witness(profile: &BgvProfile, plan: &RamLfeBgvNonlinearPlanV1) {
    let started = Instant::now();
    let precision = profile.plaintext_precision(1).unwrap();
    let data = plan_data(plan);
    let tape = tape([
        Op::LoadState(0, 0),
        Op::LoadState(1, 1),
        Op::LoadState(2, 2),
        Op::SelectEqZero(0, 0, 1, 2),
        Op::Output(0),
    ]);
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Refresh, &tape).unwrap();
    let prepared = RamLfeBgvNonlinearProgramV1::from_normal_form(&normal, plan).unwrap();
    let ProgramKind::Refresh(private) = &prepared.data else {
        unreachable!()
    };
    let (secret, key) = setup(profile, plan);
    let input = RamLfeReferenceInputV1::from_bytes(&[255, 17]).unwrap();
    // Reuse the exact admitted plan, key and program for both distinct branches.
    // Keep the original nonzero condition and add a zero condition with unequal
    // branches so either broken Select branch changes an observed ciphertext.
    for condition in [256_u16, 0] {
        let mut lanes = [0_u16; 32];
        lanes[..3].copy_from_slice(&[condition, 17, 99]);
        let memory = RamLfeInitialMemoryV1::from_lanes(&lanes).unwrap();
        let packed = RamLfeBgvPackedInputV1::encrypt(profile, precision, 0, &key, &input).unwrap();
        tests::reset_counts();
        let (original, mut banks) = initialize_state(profile, data, &memory, packed, &key).unwrap();
        drop(original);
        let pool = data.refresh.as_ref().unwrap();
        banks = refresh_banks(profile, &key, pool, &data.initial_refresh, banks).unwrap();
        let base: [u64; 128] = std::array::from_fn(|i| {
            if i < 64 {
                u64::from(input.slots()[i])
            } else if i < 96 {
                u64::from(lanes[i - 64])
            } else {
                u64::from(i == 96)
            }
        });
        let mut target = u64::from(condition);
        let mut steps = 0;
        for public in &data.layers[..3] {
            let map = match &public.coefficients {
                Coefficients::Public(map) => map,
                Coefficients::Private(index) => &private.layers[*index],
            };
            for _ in 0..public.repetitions {
                // Compare complete ciphertext residues and bounds across the
                // canonical operation, in addition to independently decrypted phases.
                let carried = [1, 2, 3].map(|bank| banks[bank].ciphertext().clone());
                banks = execute_layer(profile, public, map, banks, &key).unwrap();
                banks = refresh_banks(profile, &key, pool, &public.after_refresh, banks).unwrap();
                for (bank, expected) in [1, 2, 3].into_iter().zip(&carried) {
                    assert_eq!(banks[bank].ciphertext(), expected);
                }
                if steps < 8 {
                    target = target * target % 257
                } else if steps == 8 {
                    target = ((258 - target) % 257 * (17 + 257 - 99) + 99) % 257;
                }
                let expected = std::array::from_fn(|i| if i == 0 { target } else { 0 });
                assert_phase(profile, &secret, banks[0].as_local(), &expected);
                assert_phase(profile, &secret, banks[1].as_local(), &[0; 128]);
                assert_phase(profile, &secret, banks[2].as_local(), &[0; 128]);
                assert_phase(profile, &secret, banks[3].as_local(), &base);
                steps += 1;
            }
        }
        assert_eq!(steps, 9);
        assert_eq!(target, if condition == 0 { 17 } else { 99 });
        assert_eq!(tests::actual_counts().bootstrap_calls, 10);
        assert_eq!(tests::actual_counts().ciphertext_products, 9);
        assert_eq!(tests::actual_counts().routing_calls, 9);
        eprintln!(
            "Refresh first gate condition={condition} 10 bootstraps/36 independent phase checks elapsed_ms={}",
            started.elapsed().as_millis()
        );
        tests::reset_counts();
    }
}

#[test]
#[ignore = "three complete Refresh-class runs, each 2871 steps/2872 genuine bootstraps; parent schedules native diagnostics"]
fn refresh_class_full_maximum_shapes_nonzero_banks_and_ordered_alias_outputs() {
    let started = Instant::now();
    let profile = tests::refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let plan = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule(), budget()).unwrap();
    let (secret, key) = setup(&profile, &plan);
    let mut secret = Some(secret);
    for (case, program, input, memory) in [
        (
            "maximum319",
            maximum(),
            RamLfeReferenceInputV1::from_bytes(&[255; 63]).unwrap(),
            memory(),
        ),
        (
            "nonzero-cross-banks",
            nonzero(),
            RamLfeReferenceInputV1::from_bytes(&[7]).unwrap(),
            memory(),
        ),
        (
            "stored-alias64snapshots",
            independent_refresh_tests::distinct_program(),
            independent_refresh_tests::distinct_input(),
            independent_refresh_tests::distinct_memory(),
        ),
    ] {
        let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Refresh, &program).unwrap();
        if case == "maximum319" {
            assert_eq!(normal.gate_count(), 319);
            assert_eq!(normal.report().refresh_count(), 64);
            assert_eq!(normal.depth(), 16);
        }
        if case == "nonzero-cross-banks" {
            assert!(normal.gate_count() > 256);
            assert_eq!(normal.report().refresh_count(), 64);
        }
        let expected =
            ram_lfe_reference_execute_v1(RamLfeClassV1::Refresh, &program, &memory, &input)
                .unwrap();
        assert_eq!(
            normal
                .evaluate(&memory, &input)
                .unwrap()
                .padded_for_opening(),
            expected.output().padded_for_opening()
        );
        let slots: [u64; 128] = std::array::from_fn(|i| {
            if i < 64 {
                u64::from(expected.output().padded_for_opening().scalars()[i])
            } else {
                0
            }
        });
        if case == "stored-alias64snapshots" {
            assert_eq!(slots, independent_refresh_tests::expected_slots());
        } else if case == "nonzero-cross-banks" {
            assert_eq!(slots[0], u64::from(memory.lanes()[31]));
            assert_ne!(slots[0], 0);
        }
        let prepared = RamLfeBgvNonlinearProgramV1::from_normal_form(&normal, &plan).unwrap();
        let packed = RamLfeBgvPackedInputV1::encrypt(&profile, precision, 0, &key, &input).unwrap();
        tests::reset_counts();
        let guard = BoundaryGuard::install(secret.take().unwrap(), &normal, &memory, &input);
        let run = Instant::now();
        let output = prepared
            .evaluate(&profile, &plan, &memory, packed, &key)
            .unwrap();
        assert_eq!(tests::actual_counts(), *plan.operations());
        assert_eq!(&output.shape(), plan.output_shape());
        let (returned, seen) = guard.finish();
        secret = Some(returned);
        assert_eq!(seen, [127, 128, 255, 256, 318]);
        assert_phase(&profile, secret.as_ref().unwrap(), &output, &slots);
        eprintln!(
            "Refresh case={case} full steps 2871 bootstraps 2872 run_ms={} total_ms={}",
            run.elapsed().as_millis(),
            started.elapsed().as_millis()
        );
    }
    tests::reset_counts();
}

#[test]
fn refresh_class_faults_retire_completed_and_pending_banks_on_error_and_unwind() {
    let profile = tests::refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let plan = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule(), budget()).unwrap();
    let data = plan_data(&plan);
    let (_, key) = setup(&profile, &plan);
    let input = RamLfeReferenceInputV1::from_bytes(&[7]).unwrap();
    for unwind in [false, true] {
        tests::reset_counts();
        let packed = RamLfeBgvPackedInputV1::encrypt(&profile, precision, 0, &key, &input).unwrap();
        let (original, banks) = initialize_state(&profile, data, &memory(), packed, &key).unwrap();
        drop(original);
        tests::arm_fault("refreshed bank", 1, unwind);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            refresh_banks(
                &profile,
                &key,
                data.refresh.as_ref().unwrap(),
                &data.initial_refresh,
                banks,
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RamLfeBgvNonlinearErrorV1::Allocation)
            ));
        }
        assert_eq!(tests::actual_counts().bootstrap_calls, 1);
        // Three honest routing-level zeros and one completed refreshed base.
        // All four owners still retire on either error or unwind.
        assert_eq!(
            iroha_fhe::bgv::fixtures::cleared_residue_words() - tests::retired_at_fault(),
            4 * 2 * profile.degree() * 5
        );
    }
    tests::reset_counts();
}

#[test]
#[ignore = "complete N256 scalar-subalgebra Refresh run: 2871 steps/2872 bootstraps and cross-bank full-ring phases"]
fn refresh_scalar_subalgebra_full_cross_bank_run_preserves_phases_and_projection() {
    let started = Instant::now();
    let (profile, plan) = scalar_subalgebra_fixture();
    let precision = profile.plaintext_precision(1).unwrap();
    let program = nonzero();
    let input = RamLfeReferenceInputV1::from_bytes(&[7]).unwrap();
    let memory = memory();
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Refresh, &program).unwrap();
    assert!(normal.gate_count() > 256);
    assert_eq!(normal.report().refresh_count(), 64);
    let expected =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Refresh, &program, &memory, &input).unwrap();
    assert_eq!(
        normal
            .evaluate(&memory, &input)
            .unwrap()
            .padded_for_opening(),
        expected.output().padded_for_opening()
    );
    let slots: [u64; 128] = std::array::from_fn(|i| {
        if i < 64 {
            u64::from(expected.output().padded_for_opening().scalars()[i])
        } else {
            0
        }
    });
    assert_eq!(slots[0], u64::from(memory.lanes()[31]));
    assert_ne!(slots[0], 0);
    let prepared = RamLfeBgvNonlinearProgramV1::from_normal_form(&normal, &plan).unwrap();
    let (secret, key) = setup(&profile, &plan);
    let packed = RamLfeBgvPackedInputV1::encrypt(&profile, precision, 0, &key, &input).unwrap();
    tests::reset_counts();
    let guard = BoundaryGuard::install(secret, &normal, &memory, &input);
    let output = prepared
        .evaluate(&profile, &plan, &memory, packed, &key)
        .unwrap();
    assert_eq!(tests::actual_counts(), *plan.operations());
    assert_eq!(&output.shape(), plan.output_shape());
    let (secret, seen) = guard.finish();
    // The same canonical evaluator and independent full-degree CRT oracle see
    // both target-bank changes and all four banks at every selected boundary.
    assert_eq!(seen, [127, 128, 255, 256, 318]);
    assert_phase(&profile, &secret, &output, &slots);
    eprintln!(
        "N256 Refresh full cross-bank steps2871 bootstraps2872 elapsed_ms={}",
        started.elapsed().as_millis()
    );
    tests::reset_counts();
}
