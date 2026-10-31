//! Canonical mixed-gate boundaries, copied snapshots and fixed R9 row supports.

use super::*;
use crate::ram_lfe::{
    HiddenRamFheInstruction as Op, HiddenRamFheProgram, RamLfeError, RamLfeInitialMemoryV1,
    RamLfeReferenceInputV1, clearing::cleared_cells, ram_lfe_reference_execute_v1,
    ram_lfe_reference_normalize_v1,
};
use iroha_fhe::bgv::{BgvParameters, BgvProfile, SlotLinearBudget, cost::CostModel};
use std::cell::Cell;

const SHAPE: RamLfeBgvNonlinearShapeV1 = RamLfeBgvNonlinearShapeV1::Refresh;
const MODES: [RamLfeBgvRefreshMapV1; 3] = [
    RamLfeBgvRefreshMapV1::Initial,
    RamLfeBgvRefreshMapV1::Square,
    RamLfeBgvRefreshMapV1::Final,
];

fn stage(gate: usize, mode: RamLfeBgvRefreshMapV1) -> RamLfeBgvNonlinearStageV1 {
    RamLfeBgvNonlinearStageV1::RefreshGate {
        gate: u16::try_from(gate).unwrap(),
        mode,
    }
}

fn mode(step: usize) -> RamLfeBgvRefreshMapV1 {
    match step {
        0 => MODES[0],
        1..=7 => MODES[1],
        8 => MODES[2],
        _ => unreachable!(),
    }
}

fn tape(operations: impl IntoIterator<Item = Op>) -> HiddenRamFheProgram {
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for operation in operations {
        builder.push(operation).unwrap();
    }
    builder.finish().unwrap()
}

fn compile(program: &HiddenRamFheProgram) -> (RamLfeNormalFormV1, RamLfeBgvNonlinearCompilerV1) {
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Refresh, program).unwrap();
    let compiler =
        RamLfeBgvNonlinearCompilerV1::from_normal_form(&normal, SHAPE.private_payload_bytes())
            .unwrap();
    assert_eq!(compiler.shape(), SHAPE);
    (normal, compiler)
}

fn memory(seed: u16) -> RamLfeInitialMemoryV1 {
    RamLfeInitialMemoryV1::from_lanes(&std::array::from_fn::<_, 32, _>(|i| {
        ((u16::try_from(i).unwrap() * 19) + seed) % 257
    }))
    .unwrap()
}

fn dot(form: &[u16], values: &[u16]) -> u16 {
    u16::try_from(form.iter().zip(values).fold(0_u32, |sum, (&a, &b)| {
        (sum + u32::from(a) * u32::from(b)) % 257
    }))
    .unwrap()
}

/// Direct canonical gate evaluation uses a branch for Select and identity for
/// Refresh, independently of physical placement or the compiler's row logic.
fn boundary_values(
    normal: &RamLfeNormalFormV1,
    input: &RamLfeReferenceInputV1,
    memory: &RamLfeInitialMemoryV1,
) -> ClearingArray<u16, REFRESH_COLUMNS> {
    let mut values = ClearingArray::try_zeroed().unwrap();
    values[0] = 1;
    values[1..65].copy_from_slice(input.slots());
    values[65..97].copy_from_slice(memory.lanes());
    for gate in 0..normal.gate_count() {
        let operands = std::array::from_fn::<_, 3, _>(|operand| {
            normal
                .gate_operand(gate, operand)
                .map_or(0, |form| dot(form, &values[..]))
        });
        values[97 + gate] = match normal.gate_kind(gate).unwrap() {
            RamLfeNormalGateKindV1::Mul => {
                (u32::from(operands[0]) * u32::from(operands[1]) % 257) as u16
            }
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
    values
}

/// All 2,871 steps visit every target bank's 128 outputs and all A/B/C rows.
/// Every mixed boundary, including dead gates, is compared with the direct
/// canonical oracle; all prior results and structural-zero futures are checked.
fn scalar(
    program: &HiddenRamFheProgram,
    input: &RamLfeReferenceInputV1,
    memory: &RamLfeInitialMemoryV1,
) -> [u16; 128] {
    let (normal, compiler) = compile(program);
    let expected = boundary_values(&normal, input, memory);
    let mut state = ClearingArray::<u16, 512>::try_zeroed().unwrap();
    state[384..448].copy_from_slice(input.slots());
    state[448..480].copy_from_slice(memory.lanes());
    state[480] = 1;
    let mut next = ClearingArray::<u16, 128>::try_zeroed().unwrap();
    tests::ROW_VISITS.with(|x| x.set(0));
    for gate in 0..REFRESH_GATES {
        for step in 0..9 {
            let target_bank = gate / SLOTS;
            let untouched = std::array::from_fn::<_, 512, _>(|index| state[index]);
            for slot in 0..SLOTS {
                let operands = std::array::from_fn::<_, 3, _>(|operand| {
                    let mut sum = 0_u32;
                    let (segments, count) =
                        SHAPE.support(Some(stage(gate, mode(step))), operand * SLOTS + slot);
                    compiler.emit_refresh_row(
                        gate,
                        mode(step),
                        slot,
                        operand,
                        |source, coefficient| {
                            if coefficient != 0 {
                                assert!(segments[..count].iter().any(|segment| source
                                    >= segment.start()
                                    && source
                                        < segment.start() + segment.count() * segment.stride()
                                    && (source - segment.start()) % segment.stride() == 0));
                            }
                            sum = (sum + u32::from(coefficient) * u32::from(state[source])) % 257;
                        },
                    );
                    sum
                });
                next[slot] = ((operands[0] * operands[1] + operands[2]) % 257) as u16;
            }
            state[target_bank * SLOTS..(target_bank + 1) * SLOTS].copy_from_slice(&next[..]);
            for bank in 0..3 {
                if bank != target_bank {
                    assert_eq!(
                        &state[bank * SLOTS..(bank + 1) * SLOTS],
                        &untouched[bank * SLOTS..(bank + 1) * SLOTS]
                    );
                }
            }
            assert_eq!(&state[384..], &untouched[384..]);
            assert_eq!(&state[..gate], &expected[97..97 + gate]);
            assert!(state[gate + 1..384].iter().all(|&x| x == 0));
        }
        assert_eq!(
            state[gate],
            expected[97 + gate],
            "canonical mixed boundary {gate}"
        );
    }
    assert_eq!(tests::ROW_VISITS.with(Cell::get), 319 * 9 * 128 * 3);
    let actual = std::array::from_fn(|row| {
        let mut sum = 0_u32;
        compiler.emit_projection_row(row, |source, coefficient| {
            sum = (sum + u32::from(coefficient) * u32::from(state[source])) % 257
        });
        u16::try_from(sum).unwrap()
    });
    let reference =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Refresh, program, memory, input).unwrap();
    assert_eq!(
        &actual[..64],
        reference.output().padded_for_opening().scalars()
    );
    assert_eq!(
        normal.evaluate(memory, input).unwrap().padded_for_opening(),
        reference.output().padded_for_opening()
    );
    assert_eq!(&actual[64..], &[0; 64]);
    actual
}

#[test]
fn fixed_refresh_geometry_support_and_all_public_stage_indices() {
    assert_eq!(SHAPE.class(), RamLfeClassV1::Refresh);
    assert_eq!(SHAPE.private_payload_bytes(), 850_110);
    assert_eq!(SHAPE.maximum_layer_payload_bytes(), 5_232);
    assert_eq!(SHAPE.projection_payload_bytes(), 106_496);
    assert_eq!(SHAPE.preparation_scratch_bytes(), 2_052);
    let mut cells = 0;
    let mut metadata = 0;
    for gate in 0..319 {
        for mode in MODES {
            let stage = stage(gate, mode);
            let bytes = SHAPE.layer_layout_metadata_bytes::<4, 3>(stage).unwrap();
            let layout = SHAPE.layer_layout::<4, 3>(stage, bytes).unwrap();
            assert!(matches!(
                SHAPE.layer_layout::<4, 3>(stage, bytes - 1),
                Err(RamLfeBgvNonlinearCompileErrorV1::Routing(
                    SlotLinearError::LayoutBudget { .. }
                ))
            ));
            let expected = match mode {
                RamLfeBgvRefreshMapV1::Initial => 2 * gate + 194 + gate % 128,
                RamLfeBgvRefreshMapV1::Square => 2 + gate % 128,
                RamLfeBgvRefreshMapV1::Final => 3 * gate + 292 + gate % 128,
            };
            assert_eq!(layout.coefficient_count(), expected);
            for row in 0..384 {
                let slot = row % 128;
                if slot > gate % 128 {
                    assert!(layout.row_segments(row).unwrap().is_empty());
                }
                for segment in layout.row_segments(row).unwrap() {
                    assert!(segment.start() + segment.count() * segment.stride() <= 512);
                }
            }
            cells += expected;
            metadata += bytes;
        }
    }
    let bytes = SHAPE.projection_layout_metadata_bytes::<4>().unwrap();
    let projection = SHAPE.projection_layout::<4>(bytes).unwrap();
    assert_eq!(cells * 4 + projection.coefficient_count() * 4, 1_962_112);
    assert_eq!(metadata + bytes, 5_098_896);
    for row in 64..128 {
        assert!(projection.row_segments(row).unwrap().is_empty());
    }
    for gate in [319, u16::MAX] {
        assert_eq!(
            SHAPE
                .layer_layout_metadata_bytes::<4, 3>(RamLfeBgvNonlinearStageV1::RefreshGate {
                    gate,
                    mode: MODES[0]
                })
                .unwrap_err(),
            RamLfeBgvNonlinearCompileErrorV1::Layer
        );
    }
    assert_eq!(
        SHAPE
            .layer_layout_metadata_bytes::<21, 60>(stage(0, MODES[0]))
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
    );
    assert_eq!(
        SHAPE
            .layer_layout_metadata_bytes::<4, 3>(RamLfeBgvNonlinearStageV1::BoundedLayer(1))
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
    );
    assert_eq!(
        SHAPE.projection_layout_metadata_bytes::<0>().unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
    );
}

#[test]
fn mixed_refreshes_preserve_copied_lanes_aliases_early_outputs_and_dead_gates() {
    let mut ops = vec![Op::LoadInput(0, 1)];
    ops.extend(std::iter::repeat_n(Op::Mul(0, 0, 0), 16));
    ops.extend([
        Op::StoreState(3, 0),
        Op::Output(0),
        Op::Mul(1, 0, 0),
        Op::LoadState(2, 3),
        Op::Mul(2, 2, 1),
        Op::Output(2),
        Op::Output(0),
        Op::LoadState(3, 3),
        Op::Output(3),
        Op::Mul(2, 1, 1),
    ]);
    let program = tape(ops);
    let (normal, compiler) = compile(&program);
    assert_eq!(normal.report().refresh_count(), 3);
    assert_eq!(normal.gate_operand(16, 0).unwrap()[112], 1);
    assert_eq!(
        normal.gate_operand(18, 0).unwrap()[112],
        1,
        "stored lane keeps original column"
    );
    for gate in 0..normal.gate_count() {
        for operand in 0..3 {
            if let Some(form) = normal.gate_operand(gate, operand) {
                assert_eq!(&compiler.operand(gate, operand)[..form.len()], form);
                assert!(
                    compiler.operand(gate, operand)[form.len()..]
                        .iter()
                        .all(|&x| x == 0)
                );
            }
        }
    }
    for data in [vec![], vec![3], vec![255; 63]] {
        scalar(
            &program,
            &RamLfeReferenceInputV1::from_bytes(&data).unwrap(),
            &memory(17),
        );
    }
    let mut cancelled = vec![Op::LoadInput(0, 1)];
    cancelled.extend(std::iter::repeat_n(Op::Mul(0, 0, 0), 16));
    cancelled.extend([Op::MulPlain(0, 0, 0), Op::Mul(0, 0, 0), Op::Output(0)]);
    let cancelled = tape(cancelled);
    assert_eq!(compile(&cancelled).0.report().refresh_count(), 1);
    assert!(matches!(
        ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &cancelled),
        Err(RamLfeError::ClassRankExceeded { .. })
    ));
    scalar(
        &cancelled,
        &RamLfeReferenceInputV1::from_bytes(&[3]).unwrap(),
        &memory(256),
    );

    let mut joint = vec![
        Op::LoadConst(3, 7),
        Op::LoadConst(0, 2),
        Op::LoadConst(1, 3),
        Op::LoadConst(2, 5),
    ];
    for register in 0..3 {
        joint.extend(std::iter::repeat_n(Op::Mul(register, register, 3), 16));
        joint.extend([Op::StoreState(register, register), Op::Output(register)]);
    }
    joint.extend([
        Op::SelectEqZero(0, 2, 0, 1),
        Op::Output(0),
        Op::LoadState(2, 2),
        Op::SelectEqZero(1, 2, 2, 2),
        Op::Output(1),
    ]);
    let joint = tape(joint);
    let (normal, _) = compile(&joint);
    let points: Vec<_> = normal.report().refresh_points().collect();
    assert_eq!(
        points[..3].iter().map(|p| p.register).collect::<Vec<_>>(),
        [2, 0, 1]
    );
    assert_eq!(points[0].instruction, points[1].instruction);
    assert_eq!(points[1].instruction, points[2].instruction);
    scalar(
        &joint,
        &RamLfeReferenceInputV1::from_bytes(&[3]).unwrap(),
        &memory(17),
    );
}

#[test]
fn maximum319_mixed_boundaries_and64_ordered_output_snapshots() {
    let mut ops = vec![Op::SelectEqZero(0, 0, 0, 0); 65];
    ops.extend(std::iter::repeat_n(Op::SelectEqZero(0, 1, 0, 0), 6));
    ops.extend(std::iter::repeat_n(Op::SelectEqZero(2, 3, 3, 3), 184));
    ops.push(Op::Output(0));
    let maximum = tape(ops);
    let (normal, _) = compile(&maximum);
    assert_eq!(
        (
            normal.gate_count(),
            normal.report().refresh_count(),
            normal.depth()
        ),
        (319, 64, 16)
    );
    scalar(
        &maximum,
        &RamLfeReferenceInputV1::from_bytes(&[255; 63]).unwrap(),
        &memory(123),
    );
    // Nonzero mixed boundaries cross both bank boundaries, with dead results
    // that cannot be validated by the output alone.
    let mut nonzero = vec![Op::LoadState(0, 31), Op::LoadState(1, 0)];
    nonzero.extend(std::iter::repeat_n(Op::SelectEqZero(0, 0, 0, 0), 65));
    nonzero.extend(std::iter::repeat_n(Op::SelectEqZero(0, 2, 0, 0), 6));
    nonzero.extend(std::iter::repeat_n(Op::SelectEqZero(3, 2, 1, 1), 182));
    nonzero.push(Op::Output(0));
    let nonzero = tape(nonzero);
    let input = RamLfeReferenceInputV1::from_bytes(&[7]).unwrap();
    let lanes = memory(7);
    let expected = boundary_values(&compile(&nonzero).0, &input, &lanes);
    for gate in [127, 128, 255, 256, 316] {
        assert_ne!(expected[97 + gate], 0);
    }
    scalar(&nonzero, &input, &lanes);
    let mut snapshots = vec![Op::LoadState(0, 31)];
    for _ in 0..64 {
        snapshots.extend([Op::Output(0), Op::AddPlain(0, 0, 1)]);
    }
    let actual = scalar(
        &tape(snapshots),
        &RamLfeReferenceInputV1::from_bytes(&[]).unwrap(),
        &memory(17),
    );
    for (i, &value) in actual[..64].iter().enumerate() {
        assert_eq!(
            value,
            (memory(17).lanes()[31] + u16::try_from(i).unwrap()) % 257
        );
    }
    let invalid =
        tape(std::iter::repeat_n(Op::SelectEqZero(0, 0, 0, 0), 66).chain([Op::Output(0)]));
    assert!(ram_lfe_reference_normalize_v1(RamLfeClassV1::Refresh, &invalid).is_err());
}

#[test]
fn every_select_condition_and_operand_alias_uses_eight_squares_and_copied_forms() {
    // This targeted first-gate replay covers all 257 conditions without
    // pretending to replace the complete padded schedule tests above.
    let program = tape([
        Op::LoadState(0, 0),
        Op::LoadState(1, 1),
        Op::LoadState(2, 2),
        Op::SelectEqZero(0, 0, 1, 2),
        Op::Output(0),
    ]);
    let (_, compiler) = compile(&program);
    for condition in 0..257 {
        for (zero, nonzero) in [(0, 256), (17, 99), (256, 1)] {
            let mut state = [0_u16; 512];
            state[448] = condition;
            state[449] = zero;
            state[450] = nonzero;
            state[480] = 1;
            for step in 0..9 {
                let operands = std::array::from_fn::<_, 3, _>(|operand| {
                    let mut sum = 0_u32;
                    compiler.emit_refresh_row(0, mode(step), 0, operand, |source, coefficient| {
                        sum = (sum + u32::from(coefficient) * u32::from(state[source])) % 257
                    });
                    sum
                });
                state[0] = ((operands[0] * operands[1] + operands[2]) % 257) as u16;
            }
            assert_eq!(state[0], if condition == 0 { zero } else { nonzero });
        }
    }
    for dst in 0..4 {
        for c in 0..4 {
            for z in 0..4 {
                for n in 0..4 {
                    let program = tape([
                        Op::LoadState(0, 0),
                        Op::LoadState(1, 1),
                        Op::LoadState(2, 2),
                        Op::LoadState(3, 3),
                        Op::SelectEqZero(dst, c, z, n),
                        Op::Output(dst),
                    ]);
                    let (normal, compiler) = compile(&program);
                    for operand in 0..3 {
                        assert_eq!(
                            &compiler.operand(0, operand)[..97],
                            normal.gate_operand(0, operand).unwrap()
                        );
                    }
                }
            }
        }
    }
    for value in [0, 3, 256] {
        scalar(
            &program,
            &RamLfeReferenceInputV1::from_bytes(&[1]).unwrap(),
            &memory(value),
        );
    }
}

#[test]
fn uniform_unit_elision_requires_reachable_prefix_future_zero_and_base_one() {
    // Independent clear algebra only. This does not inspect an encrypted state
    // or provide an arbitrary-state constructor to the runtime. The class's
    // Select condition rank remains ten despite nine physical update steps.
    assert_eq!(crate::ram_lfe::RAM_LFE_V1_SELECT_CONDITION_RANK, 10);
    for gate in [0, 127, 128, 255, 256, 318] {
        for condition in 0_u16..257 {
            let mut power = u32::from(condition);
            for _ in 0..8 {
                power = power * power % 257;
            }
            assert_eq!(power, u32::from(condition != 0));
            for kind in [
                RamLfeNormalGateKindV1::Mul,
                RamLfeNormalGateKindV1::SelectEqZero,
                RamLfeNormalGateKindV1::Refresh,
            ] {
                let mut before = [0_u16; 384];
                for (index, value) in before[..gate].iter_mut().enumerate() {
                    *value = ((u32::try_from(index).unwrap() * 17 + u32::from(condition) + 3) % 257)
                        as u16;
                }
                before[gate] = if kind == RamLfeNormalGateKindV1::SelectEqZero {
                    u16::try_from(power).unwrap()
                } else {
                    0
                };
                // The removed global Unit maps represented this scalar
                // expression on every coefficient, including absent futures.
                let base_one = 1_u32;
                let with_unit = std::array::from_fn::<_, 384, _>(|index| {
                    if index <= gate {
                        (u32::from(before[index]) * base_one % 257) as u16
                    } else {
                        0
                    }
                });
                assert_eq!(
                    with_unit, before,
                    "gate {gate} kind {kind:?} condition {condition}"
                );
            }
        }
        let mut nonzero_future = [0_u16; 384];
        nonzero_future[gate + 1] = 7;
        let filtered = std::array::from_fn::<_, 384, _>(|index| {
            if index <= gate {
                nonzero_future[index]
            } else {
                0
            }
        });
        assert_ne!(
            filtered, nonzero_future,
            "future-zero invariant is necessary"
        );
        let mut wrong_one = [0_u16; 384];
        wrong_one[gate] = 1;
        let with_nonunit_base = std::array::from_fn::<_, 384, _>(|index| {
            if index <= gate {
                wrong_one[index] * 2 % 257
            } else {
                0
            }
        });
        assert_ne!(
            with_nonunit_base, wrong_one,
            "base-one invariant is necessary"
        );
    }
}

fn plan(stage: RamLfeBgvNonlinearStageV1) -> SlotLinearPlan<4, 3> {
    let profile =
        BgvProfile::new(BgvParameters::derive(7, 257, 1, (40, 4), (41, 2), 2).unwrap()).unwrap();
    let shape = profile.plan_fresh(profile.master_plaintext()).unwrap();
    let one = usize::try_from(
        CostModel::new(&profile)
            .ciphertext_residue_bytes(shape.level(), 2)
            .unwrap(),
    )
    .unwrap();
    SlotLinearPlan::with_layout(
        &profile,
        &std::array::from_fn(|_| shape.clone()),
        SHAPE.layer_layout(stage, 1 << 20).unwrap(),
        SlotLinearBudget {
            coefficient_bytes: SHAPE.maximum_layer_payload_bytes(),
            layout_metadata_bytes: 1 << 20,
            preparation_scratch_bytes: SHAPE.preparation_scratch_bytes(),
            output_residue_bytes: 3 * one,
            scratch_residue_bytes: 128 * one,
        },
    )
    .unwrap()
}

#[test]
fn exact_stage_support_and_class_preflight_precede_private_rows() {
    let program = tape([
        Op::LoadState(0, 0),
        Op::SelectEqZero(1, 0, 0, 0),
        Op::Output(1),
    ]);
    let (normal, compiler) = compile(&program);
    let bytes = SHAPE.private_payload_bytes();
    let before = cleared_cells();
    assert!(matches!(
        RamLfeBgvNonlinearCompilerV1::from_normal_form(&normal, bytes - 1),
        Err(RamLfeBgvNonlinearCompileErrorV1::PrivateBudget { .. })
    ));
    assert_eq!(cleared_cells(), before);
    let initial = plan(stage(0, MODES[0]));
    let square = plan(stage(0, MODES[1]));
    tests::ROW_VISITS.with(|x| x.set(0));
    assert_eq!(
        compiler
            .prepare_layer(stage(0, MODES[0]), &square)
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::Routing(SlotLinearError::LayoutMismatch)
    );
    assert_eq!(
        compiler
            .prepare_layer(RamLfeBgvNonlinearStageV1::BoundedLayer(1), &initial)
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
    );
    let bounded = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    let bounded = RamLfeBgvNonlinearCompilerV1::from_normal_form(&bounded, usize::MAX).unwrap();
    assert_eq!(
        bounded
            .prepare_layer(stage(0, MODES[0]), &initial)
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
    );
    assert_eq!(tests::ROW_VISITS.with(Cell::get), 0);
    tests::WRITES.with(|x| x.set((0, 0)));
    let prepared = compiler
        .prepare_layer(stage(0, MODES[0]), &initial)
        .unwrap();
    assert_eq!(tests::ROW_VISITS.with(Cell::get), 384);
    assert_eq!(tests::WRITES.with(Cell::get).0, 384 * 513);
    let private_trace = tests::WRITES.with(Cell::get);
    drop(prepared);
    tests::WRITES.with(|x| x.set((0, 0)));
    let (_, empty) = compile(&tape([Op::Output(0)]));
    empty.prepare_layer(stage(0, MODES[0]), &initial).unwrap();
    assert_eq!(tests::WRITES.with(Cell::get), private_trace);
    let mut corrupted = RamLfeBgvNonlinearCompilerV1::from_normal_form(&normal, bytes).unwrap();
    corrupted.private[97] = 1; // Future/current target is outside initial operand support.
    assert!(matches!(
        corrupted.prepare_layer(stage(0, MODES[0]), &initial),
        Err(RamLfeBgvNonlinearCompileErrorV1::Routing(
            SlotLinearError::OutsideSupport { .. }
        ))
    ));
    let last = plan(stage(318, MODES[2]));
    compiler.prepare_layer(stage(318, MODES[2]), &last).unwrap();
    let profile =
        BgvProfile::new(BgvParameters::derive(7, 257, 1, (40, 4), (41, 2), 2).unwrap()).unwrap();
    let shape = profile.plan_fresh(profile.master_plaintext()).unwrap();
    let one = usize::try_from(
        CostModel::new(&profile)
            .ciphertext_residue_bytes(shape.level(), 2)
            .unwrap(),
    )
    .unwrap();
    let projection = SlotLinearPlan::with_layout(
        &profile,
        &std::array::from_fn(|_| shape.clone()),
        SHAPE.projection_layout::<4>(1 << 20).unwrap(),
        SlotLinearBudget {
            coefficient_bytes: SHAPE.projection_payload_bytes(),
            layout_metadata_bytes: 1 << 20,
            preparation_scratch_bytes: SHAPE.preparation_scratch_bytes(),
            output_residue_bytes: one,
            scratch_residue_bytes: 128 * one,
        },
    )
    .unwrap();
    let mut row = [0_u32; 513];
    compiler.fill_projection_row(0, &mut row);
    assert_eq!(row[0], 1);
    assert_eq!(row.iter().filter(|&&x| x != 0).count(), 1);
    tests::WRITES.with(|x| x.set((0, 0)));
    compiler.prepare_projection(&projection).unwrap();
    assert_eq!(tests::WRITES.with(Cell::get).0, 128 * 513);
}

#[test]
fn refresh_compiler_clears_class_sized_forms_on_success_error_and_unwind() {
    let program = tape([Op::LoadState(0, 31), Op::Mul(1, 0, 0), Op::Output(1)]);
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Refresh, &program).unwrap();
    let before = cleared_cells();
    let owner = RamLfeBgvNonlinearCompilerV1::from_normal_form(&normal, usize::MAX).unwrap();
    assert_eq!(
        format!("{owner:?}"),
        "[REDACTED RAM-LFE nonlinear compiler]"
    );
    assert!(owner.private.iter().any(|&x| x != 0));
    drop(owner);
    assert_eq!(cleared_cells() - before, REFRESH_PRIVATE_CELLS);
    let before = cleared_cells();
    let failure = || -> Result<(), RamLfeBgvNonlinearCompileErrorV1> {
        let _owner = RamLfeBgvNonlinearCompilerV1::from_normal_form(&normal, usize::MAX)?;
        Err(RamLfeBgvNonlinearCompileErrorV1::Layer)
    };
    assert!(failure().is_err());
    assert_eq!(cleared_cells() - before, REFRESH_PRIVATE_CELLS);
    let before = cleared_cells();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owner =
                RamLfeBgvNonlinearCompilerV1::from_normal_form(&normal, usize::MAX).unwrap();
            panic!("refresh compiler unwind");
        }))
        .is_err()
    );
    assert_eq!(cleared_cells() - before, REFRESH_PRIVATE_CELLS);
}

#[test]
fn public_refresh_rows_cover_every_gate_without_reading_private_forms() {
    let (_, mut compiler) = compile(&tape([Op::Output(0)]));
    // Invalid private values must be irrelevant to the wholly public modes.
    compiler.private.fill(u16::MAX);
    for gate in 0..REFRESH_GATES {
        for mode in [RamLfeBgvRefreshMapV1::Square] {
            for slot in 0..SLOTS {
                for operand in 0..3 {
                    let expected = if slot < gate % SLOTS && operand == 2 {
                        Some(gate / SLOTS * SLOTS + slot)
                    } else if slot == gate % SLOTS && operand <= 1 {
                        Some(gate)
                    } else {
                        None
                    };
                    let mut actual = None;
                    assert!(emit_public_refresh_row(
                        gate,
                        mode,
                        slot,
                        operand,
                        &mut |column, coefficient| {
                            assert!(actual.replace((column, coefficient)).is_none());
                        }
                    ));
                    assert_eq!(actual, expected.map(|column| (column, 1)));
                    let mut from_private_owner = None;
                    compiler.emit_refresh_row(gate, mode, slot, operand, |column, coefficient| {
                        assert!(from_private_owner.replace((column, coefficient)).is_none());
                    });
                    assert_eq!(from_private_owner, actual);
                }
            }
        }
        for mode in [RamLfeBgvRefreshMapV1::Initial, RamLfeBgvRefreshMapV1::Final] {
            assert!(!emit_public_refresh_row(
                gate,
                mode,
                gate % SLOTS,
                0,
                &mut |_, _| panic!("private target emitted")
            ));
        }
    }
}

#[test]
fn public_refresh_preparation_refuses_private_modes_and_mismatches_before_rows() {
    let square = plan(stage(0, RamLfeBgvRefreshMapV1::Square));
    tests::ROW_VISITS.with(|x| x.set(0));
    tests::WRITES.with(|x| x.set((0, 0)));
    let before = cleared_cells();
    for mode in [RamLfeBgvRefreshMapV1::Initial, RamLfeBgvRefreshMapV1::Final] {
        assert_eq!(
            SHAPE
                .prepare_public_layer(stage(0, mode), &square)
                .unwrap_err(),
            RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
        );
    }
    for (shape, stage) in [
        (
            RamLfeBgvNonlinearShapeV1::Bounded,
            stage(0, RamLfeBgvRefreshMapV1::Square),
        ),
        (SHAPE, RamLfeBgvNonlinearStageV1::BoundedLayer(1)),
    ] {
        assert_eq!(
            shape.prepare_public_layer(stage, &square).unwrap_err(),
            RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
        );
    }
    for gate in [319, usize::from(u16::MAX)] {
        assert_eq!(
            SHAPE
                .prepare_public_layer(stage(gate, RamLfeBgvRefreshMapV1::Square), &square)
                .unwrap_err(),
            RamLfeBgvNonlinearCompileErrorV1::Layer
        );
    }
    for other in [
        stage(1, RamLfeBgvRefreshMapV1::Square),
        stage(128, RamLfeBgvRefreshMapV1::Square),
    ] {
        assert_eq!(
            SHAPE.prepare_public_layer(other, &square).unwrap_err(),
            RamLfeBgvNonlinearCompileErrorV1::Routing(SlotLinearError::LayoutMismatch)
        );
    }
    let profile =
        BgvProfile::new(BgvParameters::derive(7, 769, 1, (40, 4), (41, 2), 2).unwrap()).unwrap();
    let source = profile.plan_fresh(profile.master_plaintext()).unwrap();
    let one = usize::try_from(
        CostModel::new(&profile)
            .ciphertext_residue_bytes(source.level(), 2)
            .unwrap(),
    )
    .unwrap();
    let alien = SlotLinearPlan::with_layout(
        &profile,
        &std::array::from_fn(|_| source.clone()),
        SHAPE
            .layer_layout::<4, 3>(stage(0, MODES[1]), 1 << 20)
            .unwrap(),
        SlotLinearBudget {
            coefficient_bytes: SHAPE.maximum_layer_payload_bytes(),
            layout_metadata_bytes: 1 << 20,
            preparation_scratch_bytes: SHAPE.preparation_scratch_bytes(),
            output_residue_bytes: 3 * one,
            scratch_residue_bytes: 128 * one,
        },
    )
    .unwrap();
    assert_eq!(
        SHAPE
            .prepare_public_layer(stage(0, MODES[1]), &alien)
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::UnsupportedProfile
    );
    let projection = SlotLinearPlan::with_layout(
        &profile,
        &std::array::from_fn(|_| source.clone()),
        SHAPE.projection_layout::<4>(1 << 20).unwrap(),
        SlotLinearBudget {
            coefficient_bytes: SHAPE.projection_payload_bytes(),
            layout_metadata_bytes: 1 << 20,
            preparation_scratch_bytes: SHAPE.preparation_scratch_bytes(),
            output_residue_bytes: one,
            scratch_residue_bytes: 128 * one,
        },
    )
    .unwrap();
    assert_eq!(
        SHAPE
            .prepare_public_layer(stage(0, MODES[1]), &projection)
            .unwrap_err(),
        RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
    );
    assert_eq!(tests::ROW_VISITS.with(Cell::get), 0);
    assert_eq!(tests::WRITES.with(Cell::get), (0, 0));
    assert_eq!(cleared_cells(), before);
}

#[test]
fn public_refresh_maps_match_private_preparation_and_direct_slot_values() {
    use iroha_fhe::bgv::{
        SlotEncoding,
        fixtures::{self, FixtureEntropy},
    };
    let profile =
        BgvProfile::new(BgvParameters::derive(7, 257, 1, (40, 4), (41, 2), 2).unwrap()).unwrap();
    let encoding = SlotEncoding::new(&profile, profile.master_plaintext()).unwrap();
    let (_, compiler) = compile(&tape([
        Op::LoadState(0, 0),
        Op::SelectEqZero(1, 0, 0, 0),
        Op::Output(1),
    ]));
    for gate in [0, 128, 318] {
        for mode in [RamLfeBgvRefreshMapV1::Square] {
            let stage = stage(gate, mode);
            let plan = plan(stage);
            let before = cleared_cells();
            tests::ROW_VISITS.with(|x| x.set(0));
            tests::WRITES.with(|x| x.set((0, 0)));
            let public = SHAPE.prepare_public_layer(stage, &plan).unwrap();
            assert_eq!(cleared_cells(), before);
            assert_eq!(tests::ROW_VISITS.with(Cell::get), 384);
            assert_eq!(tests::WRITES.with(Cell::get).0, 384 * 513);
            let public_trace = tests::WRITES.with(Cell::get);
            tests::WRITES.with(|x| x.set((0, 0)));
            let private = compiler.prepare_layer(stage, &plan).unwrap();
            assert_eq!(tests::WRITES.with(Cell::get), public_trace);
            let keys = fixtures::key_set(
                &profile,
                plan.galois_elements(),
                &mut FixtureEntropy::new([74; 64]),
            )
            .unwrap();
            let values = std::array::from_fn::<_, 4, _>(|bank| {
                std::array::from_fn::<_, 128, _>(|slot| ((bank * 71 + slot * 19 + 3) % 257) as u64)
            });
            let inputs = std::array::from_fn(|bank| {
                fixtures::encrypt(
                    &profile,
                    &keys.public,
                    &encoding.encode(&profile, &values[bank]).unwrap(),
                    &mut FixtureEntropy::new([u8::try_from(bank).unwrap() + 25; 64]),
                )
                .unwrap()
            });
            let actual = public
                .evaluate(&profile, &plan, &inputs, &keys.evaluation)
                .unwrap();
            let comparison = private
                .evaluate(&profile, &plan, &inputs, &keys.evaluation)
                .unwrap();
            for (bank, (left, right)) in actual.iter().zip(&comparison).enumerate() {
                assert_eq!(
                    left.to_bytes(&profile).unwrap(),
                    right.to_bytes(&profile).unwrap()
                );
                let decoded = encoding
                    .decode(&keys.secret.decrypt(&profile, left).unwrap())
                    .unwrap();
                let operand = bank;
                for (slot, value) in decoded.iter().enumerate() {
                    let expected = if slot < gate % 128 && operand == 2 {
                        values[gate / 128][slot]
                    } else if slot == gate % 128 && operand <= 1 {
                        values[gate / 128][gate % 128]
                    } else {
                        0
                    };
                    assert_eq!(
                        *value, expected,
                        "gate {gate} mode {mode:?} bank {bank} slot {slot}"
                    );
                }
            }
        }
    }
}
