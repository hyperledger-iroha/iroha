//! Fixed bounded matrix lowering against the independent canonical interpreter.

use super::*;
use crate::ram_lfe::{
    HiddenRamFheInstruction as Op, HiddenRamFheProgram, RamLfeError, RamLfeInitialMemoryV1,
    RamLfeReferenceInputV1, clearing::cleared_cells, ram_lfe_reference_execute_v1,
    ram_lfe_reference_normalize_v1,
};
use iroha_fhe::bgv::{BgvParameters, BgvProfile, SlotEncoding, SlotLinearBudget, cost::CostModel};
use std::cell::Cell;
use zeroize::Zeroizing;

thread_local! {
    pub(super) static ROW_VISITS: Cell<usize> = const { Cell::new(0) };
    pub(super) static WRITES: Cell<(usize, u64)> = const { Cell::new((0, 0)) };
}

pub(super) fn observe_row() {
    ROW_VISITS.with(|count| count.set(count.get() + 1));
}

pub(super) fn observe_write(index: usize) {
    WRITES.with(|state| {
        let (count, hash) = state.get();
        state.set((
            count + 1,
            hash.wrapping_mul(0x100_0000_01b3)
                .wrapping_add(index as u64 + 1),
        ));
    });
}

fn tape(ops: impl IntoIterator<Item = Op>) -> HiddenRamFheProgram {
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for op in ops {
        builder.push(op).unwrap();
    }
    builder.finish().unwrap()
}

fn compile(program: &HiddenRamFheProgram) -> RamLfeBgvNonlinearCompilerV1 {
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, program).unwrap();
    RamLfeBgvNonlinearCompilerV1::from_normal_form(
        &normal,
        RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes(),
    )
    .unwrap()
}

fn lanes(seed: u16) -> RamLfeInitialMemoryV1 {
    RamLfeInitialMemoryV1::from_lanes(&std::array::from_fn::<_, 32, _>(|i| {
        ((u32::from(seed) + u32::try_from(i).unwrap() * 29) % 257) as u16
    }))
    .unwrap()
}

fn input(seed: u8) -> RamLfeReferenceInputV1 {
    RamLfeReferenceInputV1::from_bytes(&std::array::from_fn::<_, 63, _>(|i| {
        seed.wrapping_add(u8::try_from(i * 31 % 256).unwrap())
    }))
    .unwrap()
}

/// Execute all physical A*B+C rows, using the exact emitter that fills matrices.
/// Expected results below come from the independently implemented tape VM.
fn scalar(
    compiler: &RamLfeBgvNonlinearCompilerV1,
    normal: &RamLfeNormalFormV1,
    input: &RamLfeReferenceInputV1,
    memory: &RamLfeInitialMemoryV1,
) -> [u16; 128] {
    let support = RamLfeBgvNonlinearShapeV1::Bounded
        .layer_layout::<21, 60>(RamLfeBgvNonlinearStageV1::BoundedLayer(1), 1 << 20)
        .unwrap();
    let projection = RamLfeBgvNonlinearShapeV1::Bounded
        .projection_layout::<21>(1 << 20)
        .unwrap();
    let mut banks = ClearingArray::<u16, ROW_CELLS>::try_zeroed().unwrap();
    banks[BASE_BANK..BASE_BANK + 64].copy_from_slice(input.slots());
    banks[BASE_BANK + 64..ONE].copy_from_slice(memory.lanes());
    banks[ONE] = 1;
    let mut next = ClearingArray::<u16, PHYSICAL>::try_zeroed().unwrap();
    ROW_VISITS.with(|count| count.set(0));
    for layer in 1..=16 {
        for physical in 0..PHYSICAL {
            let operands = std::array::from_fn::<_, 3, _>(|operand| {
                let mut sum = 0_u32;
                compiler.emit_layer_row(layer, physical, operand, |source, coefficient| {
                    if coefficient != 0 {
                        assert!(
                            support
                                .coefficient_index(operand * PHYSICAL + physical, source)
                                .is_some(),
                            "every emitted nonzero belongs to fixed public support"
                        );
                    }
                    if coefficient != 0 && source < PHYSICAL {
                        let ready = compiler.private[READY + source];
                        assert!(
                            ready != 0 && ready < layer,
                            "operand must be ready in old bank"
                        );
                    }
                    sum = (sum + u32::from(coefficient) * u32::from(banks[source])) % 257;
                });
                sum
            });
            next[physical] = ((operands[0] * operands[1] + operands[2]) % 257) as u16;
        }
        // Complete all routing/products before replacing any source bank.
        banks[..PHYSICAL].copy_from_slice(&next[..]);
    }
    assert_eq!(ROW_VISITS.with(Cell::get), 16 * PHYSICAL * 3);
    assert_all_boundaries(normal, &banks, input, memory);
    for physical in 0..PHYSICAL {
        if compiler.private[READY + physical] == 0 {
            assert_eq!(banks[physical], 0, "unused physical slot stays zero");
        }
    }
    std::array::from_fn(|row| {
        let mut sum = 0_u32;
        compiler.emit_projection_row(row, |source, coefficient| {
            if coefficient != 0 {
                assert!(projection.coefficient_index(row, source).is_some());
            }
            sum = (sum + u32::from(coefficient) * u32::from(banks[source])) % 257;
        });
        u16::try_from(sum).unwrap()
    })
}

/// Evaluate canonical affine operands in tape order, using a direct branch
/// for Select. This oracle does not use physical depths or compiler row logic.
fn assert_all_boundaries(
    normal: &RamLfeNormalFormV1,
    banks: &[u16; ROW_CELLS],
    input: &RamLfeReferenceInputV1,
    memory: &RamLfeInitialMemoryV1,
) {
    let mut values = ClearingArray::<u16, COLUMNS>::try_zeroed().unwrap();
    values[0] = 1;
    values[1..65].copy_from_slice(input.slots());
    values[65..97].copy_from_slice(memory.lanes());
    for gate in 0..normal.gate_count() {
        let operands = std::array::from_fn::<_, 3, _>(|operand| {
            normal.gate_operand(gate, operand).map_or(0, |form| {
                u16::try_from(form.iter().zip(values.iter()).fold(
                    0_u32,
                    |sum, (&coefficient, &value)| {
                        (sum + u32::from(coefficient) * u32::from(value)) % 257
                    },
                ))
                .unwrap()
            })
        });
        let kind = normal.gate_kind(gate).unwrap();
        let result = match kind {
            RamLfeNormalGateKindV1::Mul => {
                (u32::from(operands[0]) * u32::from(operands[1]) % 257) as u16
            }
            RamLfeNormalGateKindV1::SelectEqZero => {
                // Repeated scalar multiplication independently checks every
                // retained power and its product-by-one after all carry layers.
                for step in 0..9 {
                    let exponent = 1_usize << (step.min(7) + 1);
                    let expected =
                        (0..exponent).fold(1_u32, |value, _| value * u32::from(operands[0]) % 257);
                    assert_eq!(u32::from(banks[gate * 10 + step]), expected);
                }
                if operands[0] == 0 {
                    operands[1]
                } else {
                    operands[2]
                }
            }
            RamLfeNormalGateKindV1::Refresh => panic!("bounded form cannot refresh"),
        };
        values[97 + gate] = result;
        assert_eq!(
            banks[10 * gate + 9],
            result,
            "every boundary, including dead gates"
        );
    }
}

fn assert_matches(
    compiler: &RamLfeBgvNonlinearCompilerV1,
    program: &HiddenRamFheProgram,
    input: &RamLfeReferenceInputV1,
    memory: &RamLfeInitialMemoryV1,
) -> [u16; 128] {
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, program).unwrap();
    let actual = scalar(compiler, &normal, input, memory);
    let reference =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Bounded, program, memory, input).unwrap();
    let expected = reference.output().padded_for_opening();
    assert_eq!(&actual[..64], expected.scalars());
    assert_eq!(
        normal.evaluate(memory, input).unwrap().padded_for_opening(),
        expected
    );
    assert_eq!(&actual[64..], &[0; 64]);
    actual
}

fn profile(log_n: u32, modulus: u64) -> BgvProfile {
    BgvProfile::new(BgvParameters::derive(log_n, modulus, 1, (40, 4), (41, 2), 2).unwrap()).unwrap()
}

fn plan<const O: usize>(profile: &BgvProfile) -> SlotLinearPlan<21, O> {
    let slots = SlotEncoding::new(profile, profile.master_plaintext())
        .unwrap()
        .slots();
    let one = usize::try_from(
        CostModel::new(profile)
            .ciphertext_residue_bytes(profile.top_level(), 2)
            .unwrap(),
    )
    .unwrap();
    let layout = if slots == 128 && profile.master_plaintext_modulus() == 257 {
        RamLfeBgvNonlinearShapeV1::Bounded
            .build_layout::<21, O>(
                if O == 1 {
                    None
                } else {
                    Some(RamLfeBgvNonlinearStageV1::BoundedLayer(1))
                },
                1 << 20,
            )
            .unwrap()
    } else {
        SlotLinearLayout::<21, O>::dense(slots, 1 << 20).unwrap()
    };
    SlotLinearPlan::with_layout(
        profile,
        &std::array::from_fn(|_| profile.plan_fresh(profile.master_plaintext()).unwrap()),
        layout,
        SlotLinearBudget {
            coefficient_bytes: (21 * O * slots * slots + O * slots) * 4,
            layout_metadata_bytes: 1 << 20,
            preparation_scratch_bytes: (21 * slots + 1) * 4,
            output_residue_bytes: O * one,
            scratch_residue_bytes: 64 * one,
        },
    )
    .unwrap()
}

#[test]
fn exact_geometry_class_budget_and_original_rank_are_preserved() {
    assert_eq!(
        RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes(),
        589_246
    );
    assert_eq!(
        RamLfeBgvNonlinearShapeV1::Bounded.maximum_layer_payload_bytes(),
        1_177_080
    );
    assert_eq!(
        RamLfeBgvNonlinearShapeV1::Bounded.projection_payload_bytes(),
        90_112
    );
    assert_eq!(
        16 * RamLfeBgvNonlinearShapeV1::Bounded.maximum_layer_payload_bytes()
            + RamLfeBgvNonlinearShapeV1::Bounded.projection_payload_bytes(),
        18_923_392
    );
    let program = tape([Op::Output(0)]);
    let before = cleared_cells();
    let affine = ram_lfe_reference_normalize_v1(RamLfeClassV1::Affine, &program).unwrap();
    assert_eq!(
        RamLfeBgvNonlinearCompilerV1::from_normal_form(&affine, usize::MAX).unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::ClassNotNonlinear
    );
    assert_eq!(cleared_cells(), before);
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    let bytes = RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes();
    assert_eq!(
        RamLfeBgvNonlinearCompilerV1::from_normal_form(&normal, bytes - 1).unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::PrivateBudget {
            required: bytes,
            budget: bytes - 1
        }
    );
    assert_eq!(cleared_cells(), before);
    let mut operations = vec![Op::LoadInput(0, 1)];
    operations.extend(std::iter::repeat_n(Op::Mul(0, 0, 0), 16));
    operations.extend([Op::MulPlain(0, 0, 0), Op::Mul(0, 0, 0), Op::Output(0)]);
    assert!(matches!(
        ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &tape(operations)),
        Err(RamLfeError::ClassRankExceeded { rank: 17, .. })
    ));
}

#[test]
fn same_compiler_uses_current_lanes_for_all257_conditions_and_snapshots() {
    let program = tape([
        Op::LoadState(0, 0),
        Op::LoadInput(1, 63),
        Op::LoadState(2, 31),
        Op::Output(2),
        Op::SelectEqZero(3, 0, 1, 2),
        Op::StoreState(31, 3),
        Op::Output(3),
        Op::LoadConst(2, 256),
        Op::LoadState(0, 31),
        Op::Add(0, 0, 2),
        Op::Output(0),
    ]);
    let compiler = compile(&program);
    let input = input(251);
    let mut first = None;
    for condition in 0..=256 {
        let mut values =
            std::array::from_fn::<_, 32, _>(|i| u16::try_from((i * 37 + 256) % 257).unwrap());
        values[0] = condition;
        let memory = RamLfeInitialMemoryV1::from_lanes(&values).unwrap();
        let result = assert_matches(&compiler, &program, &input, &memory);
        if condition == 0 {
            first = Some(result);
        }
    }
    let mut values =
        std::array::from_fn::<_, 32, _>(|i| u16::try_from((i * 37 + 256) % 257).unwrap());
    values[0] = 0;
    assert_eq!(
        assert_matches(
            &compiler,
            &program,
            &input,
            &RamLfeInitialMemoryV1::from_lanes(&values).unwrap()
        ),
        first.unwrap()
    );
}

#[test]
fn select_waits_for_late_branches_and_condition_and_retains_product_by_one() {
    let mut ops = vec![Op::LoadState(0, 0)];
    ops.extend(std::iter::repeat_n(Op::Mul(0, 0, 0), 6));
    ops.push(Op::LoadInput(1, 1));
    ops.extend(std::iter::repeat_n(Op::Mul(1, 1, 1), 15));
    ops.extend([
        Op::LoadConst(2, 256),
        Op::SelectEqZero(3, 0, 1, 2),
        Op::Output(3),
    ]);
    let program = tape(ops);
    let compiler = compile(&program);
    let select = 21 * 10;
    assert_eq!(
        &compiler.private[READY + select..READY + select + 10],
        &[7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
    for condition in [0, 1, 2, 128, 256] {
        assert_matches(
            &compiler,
            &program,
            &input(u8::try_from(condition % 256).unwrap()),
            &lanes(condition),
        );
    }
}

#[test]
fn cancellation_lowers_physical_depth_without_deleting_or_renumbering_gates() {
    let mut ops = vec![Op::LoadInput(0, 1)];
    ops.extend(std::iter::repeat_n(Op::Mul(0, 0, 0), 15));
    ops.extend([Op::MulPlain(0, 0, 0), Op::Mul(0, 0, 0), Op::Output(0)]);
    let program = tape(ops);
    let compiler = compile(&program);
    assert_eq!(&compiler.private[KINDS..KINDS + 16], &[1; 16]);
    assert_eq!(compiler.private[READY + 14 * 10 + 9], 15);
    assert_eq!(compiler.private[READY + 15 * 10 + 9], 1);
    assert_matches(&compiler, &program, &input(7), &lanes(256));
}

#[test]
fn maximum_boundaries_full_outputs_and_permanent_padding_are_exact() {
    for operation in [Op::Mul(0, 1, 1), Op::SelectEqZero(0, 1, 2, 3)] {
        let program = tape(std::iter::repeat_n(operation, 255).chain([Op::Output(0)]));
        let compiler = compile(&program);
        assert!(
            compiler.private[KINDS..KINDS + GATES]
                .iter()
                .all(|&kind| kind != 0)
        );
        assert_ne!(compiler.private[READY + 2549], 0);
        assert!(
            compiler.private[READY + 2550..READY + PHYSICAL]
                .iter()
                .all(|&depth| depth == 0)
        );
        assert_matches(&compiler, &program, &input(13), &lanes(191));
    }
    // Nonzero dead boundaries must also survive when no output reads them.
    for operation in [Op::Mul(0, 1, 1), Op::SelectEqZero(0, 2, 1, 2)] {
        let program = tape(
            [Op::LoadConst(1, 256)]
                .into_iter()
                .chain(std::iter::repeat_n(operation, 254))
                .chain([Op::Output(3)]),
        );
        assert_matches(&compile(&program), &program, &input(17), &lanes(233));
    }
    let mut ops = vec![Op::LoadInput(0, 63)];
    ops.extend(std::iter::repeat_n(Op::Mul(0, 0, 0), 16));
    ops.extend(std::iter::repeat_n(Op::Mul(2, 3, 3), 175));
    ops.extend(std::iter::repeat_n(Op::Output(0), 64));
    let program = tape(ops);
    assert_matches(&compile(&program), &program, &input(255), &lanes(256));
}

#[test]
fn canonical_columns_all_inputs_lanes_and_constant_route_exactly() {
    let compiler = compile(&tape([Op::Output(0)]));
    assert_eq!(compiler.column_slot(0), ONE);
    for column in 1..97 {
        assert_eq!(compiler.column_slot(column), BASE_BANK + column - 1);
    }
    for gate in 0..255 {
        assert_eq!(compiler.column_slot(97 + gate), 10 * gate + 9);
    }
    let mut ops = vec![Op::LoadConst(0, 256)];
    for slot in 0..64 {
        ops.extend([
            Op::LoadInput(1, slot),
            Op::MulPlain(1, 1, (u64::from(slot) * 19) % 257),
            Op::Add(0, 0, 1),
        ]);
    }
    for lane in 0..30 {
        ops.extend([Op::LoadState(1, lane), Op::Add(0, 0, 1)]);
    }
    ops.extend([Op::Mul(0, 0, 0), Op::Output(0)]);
    assert_eq!(ops.len(), 255);
    let program = tape(ops);
    assert_matches(&compile(&program), &program, &input(91), &lanes(256));
}

#[test]
fn layer_and_encoding_refusals_precede_private_row_allocation() {
    let compiler = compile(&tape([Op::Output(0)]));
    let good = plan::<60>(&profile(7, 257));
    let small = plan::<60>(&profile(6, 257));
    let other_modulus = plan::<60>(&profile(7, 769));
    let before = cleared_cells();
    ROW_VISITS.with(|count| count.set(0));
    for layer in [0, 17, u16::MAX] {
        assert_eq!(
            compiler
                .prepare_layer(RamLfeBgvNonlinearStageV1::BoundedLayer(layer), &good)
                .unwrap_err(),
            RamLfeBgvNonlinearCompileErrorV1::Layer
        );
    }
    for bad in [&small, &other_modulus] {
        assert_eq!(
            compiler
                .prepare_layer(RamLfeBgvNonlinearStageV1::BoundedLayer(1), bad)
                .unwrap_err(),
            RamLfeBgvNonlinearCompileErrorV1::UnsupportedProfile
        );
    }
    assert_eq!(
        compiler
            .prepare_projection(&plan::<1>(&profile(7, 769)))
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::UnsupportedProfile
    );
    assert_eq!(ROW_VISITS.with(Cell::get), 0);
    assert_eq!(cleared_cells(), before);
}

#[test]
fn full_layer_materialization_visits_every_logical_row_and_uses_shared_owner() {
    let zero = compile(&tape([Op::Output(0)]));
    let program = tape([
        Op::LoadInput(0, 63),
        Op::LoadState(1, 31),
        Op::SelectEqZero(2, 0, 1, 0),
        Op::Output(2),
    ]);
    let compiler = compile(&program);
    let profile = profile(7, 257);
    let layer_plan = plan::<60>(&profile);
    let mut row = Zeroizing::new(vec![0_u32; ROW_CELLS + 1]);
    let mut coefficients = Zeroizing::new(vec![0_u32; LAYER_CELLS]);
    let fill =
        |compiler: &RamLfeBgvNonlinearCompilerV1, row: &mut [u32], coefficients: &mut [u32]| {
            for logical in 0..OPERAND_BANKS * SLOTS {
                row.fill(0);
                compiler.fill_layer_row(10, logical, row);
                for (column, &value) in row.iter().enumerate() {
                    match layer_plan.layout().coefficient_index(logical, column) {
                        Some(at) => coefficients[at] = value,
                        None => assert_eq!(value, 0, "outside fixed support"),
                    }
                }
            }
        };
    WRITES.with(|state| state.set((0, 0)));
    fill(&zero, &mut row, &mut coefficients);
    let zero_trace = WRITES.with(Cell::get);
    assert_eq!(zero_trace.0, OPERAND_BANKS * SLOTS * (ROW_CELLS + 1));
    assert!(coefficients.iter().all(|&x| x == 0));
    WRITES.with(|state| state.set((0, 0)));
    fill(&compiler, &mut row, &mut coefficients);
    assert_eq!(WRITES.with(Cell::get), zero_trace);
    let at = |operand: usize, physical: usize, source: usize| {
        layer_plan
            .layout()
            .coefficient_index(operand * PHYSICAL + physical, source)
            .unwrap()
    };
    assert_eq!(coefficients[at(0, 9, ONE)], 1);
    assert_eq!(coefficients[at(0, 9, 8)], 256);
    assert_eq!(coefficients[at(1, 9, BASE_BANK + 95)], 1);
    assert_eq!(coefficients[at(1, 9, BASE_BANK + 63)], 256);
    assert_eq!(coefficients[at(2, 9, BASE_BANK + 63)], 1);
    assert_eq!(coefficients[at(0, 0, 0)], 1);
    assert_eq!(coefficients[at(1, 0, ONE)], 1);
    drop(coefficients);
    WRITES.with(|state| state.set((0, 0)));
    let map = compiler
        .prepare_layer(RamLfeBgvNonlinearStageV1::BoundedLayer(10), &layer_plan)
        .unwrap();
    assert_eq!(WRITES.with(Cell::get), zero_trace);
    assert_eq!(format!("{map:?}"), "[REDACTED packed linear map]");
    drop(map);
    let projection_plan = plan::<1>(&profile);
    row.fill(0);
    compiler.fill_projection_row(0, &mut row);
    assert_eq!(row[9], 1);
    assert_eq!(row.iter().filter(|&&x| x != 0).count(), 1);
    WRITES.with(|state| state.set((0, 0)));
    let projection = compiler.prepare_projection(&projection_plan).unwrap();
    assert_eq!(WRITES.with(Cell::get).0, SLOTS * (ROW_CELLS + 1));
    drop(projection);
}

#[test]
fn public_support_is_complete_fixed_and_rejects_future_and_bias_coefficients() {
    let layer = RamLfeBgvNonlinearShapeV1::Bounded
        .layer_layout::<21, 60>(RamLfeBgvNonlinearStageV1::BoundedLayer(1), 1 << 20)
        .unwrap();
    let projection = RamLfeBgvNonlinearShapeV1::Bounded
        .projection_layout::<21>(1 << 20)
        .unwrap();
    assert_eq!(layer.coefficient_count(), 294_270);
    assert_eq!(projection.coefficient_count(), 22_528);
    let mut next = 0;
    for operand in 0..3 {
        for physical in 0..PHYSICAL {
            let gate = physical / 10;
            let step = physical % 10;
            let row = operand * PHYSICAL + physical;
            for column in 0..=ROW_CELLS {
                let form =
                    (BASE_BANK..=ONE).contains(&column) || column < 10 * gate && column % 10 == 9;
                let expected = gate < 255
                    && match (step, operand) {
                        (0, 0) => form || column == physical,
                        (0, 1) | (9, 1 | 2) => form,
                        (1..=8, 0) => column == physical - 1 || column == physical,
                        (1..=7, 1) => column == physical - 1 || column == ONE,
                        (8, 1) => column == ONE,
                        (9, 0) => form || column == physical - 1 || column == physical,
                        _ => false,
                    };
                let actual = layer.coefficient_index(row, column);
                assert_eq!(actual.is_some(), expected, "row {row}, column {column}");
                if expected {
                    assert_eq!(actual, Some(next));
                    next += 1;
                }
            }
        }
    }
    assert_eq!(next, layer.coefficient_count());
    next = 0;
    for row in 0..128 {
        for column in 0..=ROW_CELLS {
            let expected = row < 64
                && ((BASE_BANK..=ONE).contains(&column) || column < 2550 && column % 10 == 9);
            let actual = projection.coefficient_index(row, column);
            assert_eq!(actual.is_some(), expected);
            if expected {
                assert_eq!(actual, Some(next));
                next += 1;
            }
        }
    }
    assert_eq!(next, projection.coefficient_count());
    let profile = profile(7, 257);
    let normal = ram_lfe_reference_normalize_v1(
        RamLfeClassV1::Bounded,
        &tape([Op::LoadInput(0, 1), Op::Mul(0, 0, 0), Op::Output(0)]),
    )
    .unwrap();
    let mut compiler = RamLfeBgvNonlinearCompilerV1::from_normal_form(&normal, usize::MAX).unwrap();
    // Deliberately corrupt the opaque test owner: future gate1 must not disappear.
    compiler.private[97 + 1] = 1;
    let planned = plan::<60>(&profile);
    assert!(matches!(
        compiler.prepare_layer(RamLfeBgvNonlinearStageV1::BoundedLayer(1), &planned),
        Err(RamLfeBgvNonlinearCompileErrorV1::Routing(
            SlotLinearError::OutsideSupport { .. }
        ))
    ));
    let mut dense_budget = SlotLinearBudget {
        coefficient_bytes: 82_606_080,
        layout_metadata_bytes: 1 << 20,
        preparation_scratch_bytes: 10_756,
        output_residue_bytes: planned.output_residue_bytes(),
        scratch_residue_bytes: planned.scratch_residue_bytes(),
    };
    let dense: SlotLinearPlan<21, 60> = SlotLinearPlan::new(
        &profile,
        &std::array::from_fn(|_| profile.plan_fresh(profile.master_plaintext()).unwrap()),
        dense_budget,
    )
    .unwrap();
    ROW_VISITS.with(|x| x.set(0));
    assert_eq!(
        compiler
            .prepare_layer(RamLfeBgvNonlinearStageV1::BoundedLayer(1), &dense)
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::Routing(SlotLinearError::LayoutMismatch)
    );
    assert_eq!(ROW_VISITS.with(Cell::get), 0);
    dense_budget.preparation_scratch_bytes -= 1;
    assert!(matches!(
        SlotLinearPlan::<21, 60>::new(
            &profile,
            &std::array::from_fn(|_| profile.plan_fresh(profile.master_plaintext()).unwrap()),
            dense_budget
        ),
        Err(SlotLinearError::PreparationBudget { .. })
    ));
}

#[test]
fn compiler_private_owner_clears_on_drop_error_and_unwind() {
    let program = tape([Op::LoadConst(0, 256), Op::Mul(0, 0, 0), Op::Output(0)]);
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    let before = cleared_cells();
    let owner = RamLfeBgvNonlinearCompilerV1::from_normal_form(
        &normal,
        RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes(),
    )
    .unwrap();
    assert_eq!(
        format!("{owner:?}"),
        "[REDACTED RAM-LFE nonlinear compiler]"
    );
    drop(owner);
    assert_eq!(cleared_cells() - before, PRIVATE_CELLS);
    let failing = || -> Result<(), RamLfeBgvNonlinearCompileErrorV1> {
        let _owner = RamLfeBgvNonlinearCompilerV1::from_normal_form(
            &normal,
            RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes(),
        )?;
        Err(RamLfeBgvNonlinearCompileErrorV1::Layer)
    };
    assert!(failing().is_err());
    assert_eq!(cleared_cells() - before, 2 * PRIVATE_CELLS);
    assert!(
        std::panic::catch_unwind(|| {
            let _owner = RamLfeBgvNonlinearCompilerV1::from_normal_form(
                &normal,
                RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes(),
            )
            .unwrap();
            panic!("bounded compiler unwind control");
        })
        .is_err()
    );
    assert_eq!(cleared_cells() - before, 3 * PRIVATE_CELLS);
}
