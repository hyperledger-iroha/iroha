//! Native direct jumps joined to the original private control and history columns.

use super::*;

fn instruction(wide_offset: bool, offset: i32) -> u32 {
    if wide_offset {
        enc::encode_offset24(wide::control::JMP, offset)
    } else {
        enc::encode_jump(wide::control::JAL, 0, i16::try_from(offset).unwrap())
    }
}

fn first_jump(wide_offset: bool, offset: i32, gas: u64) -> (Program, ScalarFixture) {
    let prefix = usize::try_from((-offset).max(0)).unwrap();
    let jump = instruction(wide_offset, offset);
    let mut body = vec![enc::encode_ri(wide::arithmetic::ADDI, 0, 0, 0); prefix];
    body.push(jump);
    let (program, recorder, _later_outcome) = shifts::capture(
        &body,
        &[
            (0, u64::MAX, true),
            (2, u64::MAX, true),
            (3, 17, false),
            (255, 23, true),
        ],
        gas,
        64,
    );
    let record = &recorder.records()[prefix];
    assert_eq!(record.instruction, Some(jump));
    assert_eq!(record.opcode_gas, Some(2));
    assert_eq!(record.outcome, DiagnosticStepOutcome::Completed);
    assert_eq!(
        record.after.pc,
        record.before.pc.wrapping_add_signed(i64::from(offset) * 4)
    );
    assert_eq!(record.before.gas_remaining - record.after.gas_remaining, 2);
    assert_eq!(record.after.cycles, record.before.cycles + 1);
    assert_eq!(record.before.registers, record.after.registers);
    assert_eq!(record.before.tags, record.after.tags);
    assert_eq!(record.changed_registers().count(), 0);
    assert_eq!(record.after.registers[0], 0);
    assert!(!record.after.tags[0]);
    assert!(!record.after.halted);
    assert_eq!(record.before.vector_length, record.after.vector_length);
    assert_eq!(
        record.before.constraint_failed,
        record.after.constraint_failed
    );
    let fixture = ScalarFixture::from_record(&program, record);
    assert!(fixture.accepts(&program));
    for slot in 0..PORTS {
        if ![
            PC_READ,
            GAS_DEBIT,
            CALL_DEPTH,
            PC_WRITE,
            CYCLE_WRITE,
            RUNNING_WRITE,
        ]
        .contains(&slot)
        {
            assert!(
                fixture.0.packets.fields[slot]
                    .iter()
                    .all(|field| *field == F::ZERO)
            );
        }
    }
    (program, fixture)
}

#[test]
fn native_direct_jumps_bind_signed_offsets_and_no_register_or_lifecycle_effects() {
    for wide_offset in [false, true] {
        for offset in [-3, -2, -1, 0, 1, 2, 4] {
            let (program, fixture) = first_jump(wide_offset, offset, 128);
            for depth in [0, 1, 1023, 1024] {
                let mut preserved = fixture.clone();
                preserved.0.set_depth(depth, depth);
                assert!(preserved.accepts(&program));
                assert_eq!(preserved.0.packets.fields[CALL_DEPTH][WRITE], F::ZERO);
                preserved.0.set_depth(depth, depth + 1);
                assert!(!preserved.accepts(&program));
            }
        }
    }
    assert!(role(enc::encode_jump(wide::control::JAL, 1, 1)) == Some(Role::Child));
    assert!(role(enc::encode_offset24(wide::control::JALS, 1)) == Some(Role::Child));
}

#[test]
fn every_direct_jump_original_field_and_workspace_mutation_rejects() {
    for wide_offset in [false, true] {
        let (program, fixture) = first_jump(wide_offset, 2, 128);
        for slot in 0..PORTS {
            for column in 0..packet::WIDTH {
                let mut forged = fixture.clone();
                forged.0.packets.fields[slot][column] =
                    forged.0.packets.fields[slot][column].add(F::ONE);
                assert!(
                    !forged.accepts(&program),
                    "jump {wide_offset} port {slot} column {column}"
                );
            }
        }
        for column in 0..super::super::super::WIDTH {
            let mut forged = fixture.clone();
            forged.0.row[column] = forged.0.row[column].add(F::ONE);
            assert!(
                !forged.accepts(&program),
                "jump {wide_offset} workspace {column}"
            );
        }
        let mut halted = fixture.clone();
        halted.0.row[HALT] = F::ONE;
        halted.0.packets.fields[RUNNING_WRITE][AFTER] = F::ZERO;
        assert!(!halted.accepts(&program));
        for replacement in [instruction(wide_offset, 1), instruction(wide_offset, 3)] {
            let other = Program::new(contract(&[replacement], 64, ivm::ivm_mode::ZK)).unwrap();
            assert!(!fixture.accepts(&other));
        }
    }
}

#[test]
fn direct_jump_gas_binds_two_full_width_units_and_forbids_underflow() {
    for wide_offset in [false, true] {
        let jump = instruction(wide_offset, 0);
        for available in [0, 1] {
            let (_, recorder, outcome) =
                shifts::capture(&[jump], &[], root_setup_gas() + available, 32);
            assert!(matches!(outcome, Err(ivm::VMError::OutOfGas)));
            let trapped = &recorder.records()[0];
            assert_eq!(trapped.opcode_gas, Some(2));
            assert!(matches!(trapped.outcome, DiagnosticStepOutcome::Trapped(_)));
            assert_eq!(trapped.before, trapped.after);
        }
        for gas in [root_setup_gas() + 2, 1 << 16, 1 << 32, 1 << 48, u64::MAX] {
            first_jump(wide_offset, 0, gas);
        }
        let (program, fixture) = first_jump(wide_offset, 0, (1 << 48) + root_setup_gas());
        let before = packet::half(&fixture.0.packets.fields[GAS_DEBIT], BEFORE, 0);
        for wrong_cost in [0, 1, 3, 1 << 16, 1 << 32] {
            let mut forged = fixture.clone();
            let after = before - wrong_cost;
            bits(&mut forged.0.row[WORDS + 128..WORDS + 192], after);
            carries(
                &mut forged.0.row[CARRIES..CARRIES + 4],
                before,
                wrong_cost,
                true,
            );
            for limb in 0..4 {
                forged.0.packets.fields[GAS_DEBIT][AFTER + limb] = constant_limb(after, limb);
            }
            assert!(!forged.accepts(&program));
        }
        let mut underflow = fixture.clone();
        for (word, value) in [(1, 1), (2, u64::MAX)] {
            bits(
                &mut underflow.0.row[WORDS + word * 64..WORDS + (word + 1) * 64],
                value,
            );
        }
        for limb in 0..4 {
            underflow.0.packets.fields[GAS_DEBIT][BEFORE + limb] = constant_limb(1, limb);
            underflow.0.packets.fields[GAS_DEBIT][AFTER + limb] = constant_limb(u64::MAX, limb);
        }
        carries(&mut underflow.0.row[CARRIES..CARRIES + 4], 1, 2, true);
        assert!(!underflow.accepts(&program));
    }
}

#[test]
fn direct_jump_cycle_limit_and_one_cycle_commit_are_exact() {
    for wide_offset in [false, true] {
        let jump = instruction(wide_offset, 0);
        let (program, recorder, outcome) = shifts::capture(&[jump], &[], 128, 1);
        assert!(matches!(outcome, Err(ivm::VMError::ExceededMaxCycles)));
        assert_eq!(recorder.records().len(), 1);
        let fixture = ScalarFixture::from_record(&program, &recorder.records()[0]);
        assert!(fixture.accepts(&program));
        for wrong_after in [0, 2, 1 << 16, u64::MAX] {
            let mut forged = fixture.clone();
            bits(&mut forged.0.row[WORDS + 256..WORDS + 320], wrong_after);
            carries(
                &mut forged.0.row[CARRIES + 4..CARRIES + 8],
                0,
                wrong_after,
                false,
            );
            for limb in 0..4 {
                forged.0.packets.fields[CYCLE_WRITE][AFTER + limb] =
                    constant_limb(wrong_after, limb);
            }
            assert!(!forged.accepts(&program));
        }
        let mut exhausted = fixture.clone();
        for (word, value) in [(3, 1), (4, 2), (9, u64::MAX)] {
            bits(
                &mut exhausted.0.row[WORDS + word * 64..WORDS + (word + 1) * 64],
                value,
            );
        }
        exhausted.0.packets.fields[CYCLE_WRITE][BEFORE] = F::ONE;
        exhausted.0.packets.fields[CYCLE_WRITE][AFTER] = F(2);
        carries(&mut exhausted.0.row[CARRIES + 4..CARRIES + 8], 1, 1, false);
        carries(&mut exhausted.0.row[CARRIES + 16..CARRIES + 20], 0, 1, true);
        assert!(!exhausted.accepts(&program));
    }
}

#[test]
fn direct_jump_original_control_ports_join_every_private_history_stage() {
    for wide_offset in [false, true] {
        let (_, fixture) = first_jump(wide_offset, 2, 128);
        assert!(history::accepts(&fixture, None));
        for slot in [
            PC_READ,
            GAS_DEBIT,
            CALL_DEPTH,
            PC_WRITE,
            CYCLE_WRITE,
            RUNNING_WRITE,
        ] {
            assert!(!history::accepts(&fixture, Some((slot, false))));
            assert!(!history::accepts(&fixture, Some((slot, true))));
        }
    }
}

#[test]
fn native_direct_jumps_and_conditional_loop_preserve_consecutive_original_boundaries() {
    let body = [
        enc::encode_ri(wide::arithmetic::ADDI, 2, 0, 2),
        instruction(false, 2),
        enc::encode_ri(wide::arithmetic::ADDI, 4, 0, 7),
        enc::encode_ri(wide::arithmetic::ADDI, 2, 2, -1),
        enc::encode_branch(wide::control::BNE, 2, 0, -2),
        instruction(true, 1),
        enc::encode_ri(wide::arithmetic::ADDI, 5, 4, 1),
    ];
    let (program, recorder, _later_outcome) = shifts::capture(&body, &[], 128, 64);
    let executed = [0, 1, 3, 4, 2, 3, 4, 5, 6];
    let records = &recorder.records()[..executed.len()];
    for (slot, record) in executed.into_iter().zip(records) {
        assert_eq!(record.instruction, Some(body[slot]));
        assert!(ScalarFixture::from_record(&program, record).accepts(&program));
    }
    for pair in records.windows(2) {
        assert_eq!(pair[0].after, pair[1].before);
    }
    assert_eq!(records.last().unwrap().after.registers[2], 0);
    assert_eq!(records.last().unwrap().after.registers[5], 8);
}

#[test]
fn direct_jumps_reach_both_bounded_image_edges_and_reject_non_boundaries() {
    let nop = enc::encode_ri(wide::arithmetic::ADDI, 0, 0, 0);
    for wide_offset in [false, true] {
        for backward in [false, true] {
            let mut body = vec![nop; 60];
            let slot = if backward { 59 } else { 0 };
            body[slot] = instruction(wide_offset, if backward { -59 } else { 63 });
            let (program, recorder, _later_outcome) = shifts::capture(&body, &[], 256, 64);
            assert_eq!(program.words.len(), MAX_WORDS);
            let record = &recorder.records()[slot];
            assert_eq!(record.instruction, Some(body[slot]));
            assert_eq!(
                record.after.pc,
                u64::from(program.first_pc) + if backward { 0 } else { 63 * 4 }
            );
            assert!(ScalarFixture::from_record(&program, record).accepts(&program));
        }
        // Unlike a conditional branch, a direct jump at the last instruction
        // needs no fallthrough. Execute both edges of this admitted image.
        let mut body = vec![nop; 60];
        body[0] = instruction(wide_offset, 63);
        let artifact = contract(&body, 64, ivm::ivm_mode::ZK);
        let mut bytes = artifact.artifact().to_vec();
        let last = bytes.len() - 4;
        bytes[last..].copy_from_slice(&instruction(wide_offset, -63).to_le_bytes());
        let program = Program::new(ivm::prepare_contract(bytes.into()).unwrap()).unwrap();
        let (program, recorder, _) = shifts::capture_program(program, &[], 256);
        for (index, record) in recorder.records().iter().take(4).enumerate() {
            assert_eq!(
                record.before.pc,
                u64::from(program.first_pc) + if index % 2 == 0 { 0 } else { 63 * 4 }
            );
            assert!(ScalarFixture::from_record(&program, record).accepts(&program));
        }
        let artifact = contract(&[instruction(wide_offset, 0)], 64, ivm::ivm_mode::ZK);
        let (minimum, maximum) = if wide_offset {
            (-(1 << 23), (1 << 23) - 1)
        } else {
            (i32::from(i16::MIN), i32::from(i16::MAX))
        };
        for offset in [minimum, -1, 5, maximum] {
            let mut invalid = artifact.artifact().to_vec();
            invalid[artifact.code_offset()..artifact.code_offset() + 4]
                .copy_from_slice(&instruction(wide_offset, offset).to_le_bytes());
            assert!(ivm::prepare_contract(invalid.into()).is_err());
        }
    }
}

#[test]
fn direct_jump_composition_preserves_control_degree_two_and_full_degree_four() {
    use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
    let program = Program::new(contract(
        &[
            instruction(false, 1),
            instruction(true, 1),
            enc::encode_rr(wide::arithmetic::CMOV, 4, 2, 3),
        ],
        64,
        ivm::ivm_mode::ZK,
    ))
    .unwrap();
    let width = super::super::super::WIDTH;
    let schedule = Schedule::new(7, core::array::from_fn(|i| i as u32)).unwrap();
    for composed in [false, true] {
        let degree = if composed { 4 } else { 2 };
        let measured = measured_maximum_affine_degree_v1(
            [0x7b; 32],
            [width + PORTS * packet::WIDTH, 0, 0, 0, 0],
            8,
            degree,
            |row, _, _, _, _| {
                let original = OriginalPackets::candidate(core::array::from_fn(|slot| {
                    row[width + slot * packet::WIDTH..width + (slot + 1) * packet::WIDTH]
                        .try_into()
                        .unwrap()
                }));
                let mut residues = Vec::new();
                if composed {
                    super::super::super::append_residues(
                        &mut residues,
                        &program,
                        schedule,
                        row[..width].try_into().unwrap(),
                        &original,
                    );
                } else {
                    super::super::super::append_control_residues(
                        &mut residues,
                        &program,
                        schedule,
                        row[..width].try_into().unwrap(),
                        &original,
                    );
                }
                Ok::<_, core::convert::Infallible>(residues)
            },
        );
        assert_eq!(measured, usize::from(degree));
    }
}

#[test]
fn direct_jump_extension_keeps_other_destinations_and_indirect_forms_rejected() {
    let artifact = contract(&[instruction(false, 0)], 64, ivm::ivm_mode::ZK);
    for forbidden in [
        enc::encode_jump(wide::control::JAL, 2, 0),
        enc::encode_jump(wide::control::JAL, 255, 0),
        enc::encode_ri(wide::control::JALR, 1, 1, 0),
        enc::encode_ri(wide::control::JALR, 0, 2, 0),
        enc::encode_ri(wide::control::JALR, 0, 1, 1),
    ] {
        assert!(role(forbidden).is_none());
        let mut invalid = artifact.artifact().to_vec();
        invalid[artifact.code_offset()..artifact.code_offset() + 4]
            .copy_from_slice(&forbidden.to_le_bytes());
        assert!(ivm::prepare_contract(invalid.into()).is_err());
    }
}
