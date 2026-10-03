//! GETGAS binds the exact original post-debit word without fabricated register reads.

use super::*;

fn captured(instruction: u32, gas: u64) -> (Program, ScalarFixture) {
    let (program, recorder, _) = shifts::capture(
        &[instruction],
        &[(2, u64::MAX, true), (3, 17, false), (4, 29, true)],
        gas,
        32,
    );
    let record = &recorder.records()[0];
    assert_eq!(record.instruction, Some(instruction));
    assert_eq!(record.opcode_gas, Some(0));
    assert_eq!(record.outcome, DiagnosticStepOutcome::Completed);
    assert_eq!(record.before.gas_remaining, gas - root_setup_gas());
    assert_eq!(record.after.gas_remaining, record.before.gas_remaining);
    assert_eq!(record.after.cycles, record.before.cycles + 1);
    assert_eq!(record.after.pc, record.before.pc + 4);
    assert!(!record.after.halted);
    for register in 0..256 {
        if register == wide::rd(instruction) && register != 0 {
            assert_eq!(record.after.registers[register], record.after.gas_remaining);
            assert!(!record.after.tags[register]);
        } else {
            assert_eq!(
                record.after.registers[register],
                record.before.registers[register]
            );
            assert_eq!(record.after.tags[register], record.before.tags[register]);
        }
    }
    let fixture = ScalarFixture::from_record(&program, record);
    assert!(fixture.accepts(&program));
    for slot in [SCALAR_LEFT, SCALAR_RIGHT] {
        assert!(
            fixture.0.packets.fields[slot]
                .iter()
                .all(|cell| *cell == F::ZERO)
        );
    }
    (program, fixture)
}

#[test]
fn native_getgas_binds_all_word_limbs_destination_zero_and_public_tag() {
    for gas in [
        root_setup_gas(),
        root_setup_gas() + 1,
        1 << 16,
        1 << 32,
        1 << 48,
        u64::MAX,
    ] {
        for rd in [0, 1, 2, 4, 10, 11, 12, 13, 31, 255] {
            captured(enc::encode_rr(wide::system::GETGAS, rd, 2, 3), gas);
        }
    }
}

#[test]
fn native_getgas_unused_operand_bytes_never_authorize_register_reads() {
    for (rs1, rs2) in [(0, 0), (2, 3), (4, 4), (127, 128), (255, 255)] {
        let instruction = enc::encode_rr(wide::system::GETGAS, 4, rs1, rs2);
        let (program, fixture) = captured(instruction, 100);
        for (slot, register) in [(SCALAR_LEFT, rs1), (SCALAR_RIGHT, rs2)] {
            let mut forged = fixture.clone();
            forged.0.packets.fields[slot] = event(
                Space::Register,
                0,
                u32::from(register),
                0,
                0,
                false,
                fixture.0.schedule.clocks[slot],
                false,
                false,
            );
            assert!(!forged.accepts(&program));
        }
    }
}

#[test]
fn getgas_rejects_supplied_values_coherent_nonzero_tariffs_tags_and_all_original_mutations() {
    let instruction = enc::encode_rr(wide::system::GETGAS, 4, 4, 4);
    let (program, fixture) = captured(instruction, (1 << 48) + 100);
    let gas_before = packet::half(&fixture.0.packets.fields[GAS_DEBIT], BEFORE, 0);
    let gas_after = gas_before;
    let mut supplied = fixture.clone();
    for limb in 0..4 {
        supplied.0.packets.fields[SCALAR_DESTINATION][AFTER + limb] =
            constant_limb(gas_before + 1, limb);
    }
    assert!(!supplied.accepts(&program));
    for cost in [1, 2, 3, 1 << 16, 1 << 32] {
        let mut forged = fixture.clone();
        let wrong = gas_before - cost;
        bits(&mut forged.0.row[WORDS + 128..WORDS + 192], wrong);
        carries(
            &mut forged.0.row[CARRIES..CARRIES + 4],
            gas_before,
            cost,
            true,
        );
        for limb in 0..4 {
            forged.0.packets.fields[GAS_DEBIT][AFTER + limb] = constant_limb(wrong, limb);
            forged.0.packets.fields[SCALAR_DESTINATION][AFTER + limb] = constant_limb(wrong, limb);
        }
        assert!(!forged.accepts(&program));
    }
    assert_eq!(
        packet::half(&fixture.0.packets.fields[SCALAR_DESTINATION], AFTER, 0),
        gas_after
    );
    for slot in 0..PORTS {
        for column in 0..packet::WIDTH {
            // The original history, not an operand alias, owns the prior
            // destination. GETGAS ignores both encoded source bytes even when
            // they spell rd. No other original column is unconstrained here.
            if slot == SCALAR_DESTINATION
                && ((BEFORE..BEFORE + 4).contains(&column) || column == BEFORE_TAG)
            {
                continue;
            }
            let mut forged = fixture.clone();
            forged.0.packets.fields[slot][column] =
                forged.0.packets.fields[slot][column].add(F::ONE);
            assert!(!forged.accepts(&program), "slot {slot} column {column}");
        }
    }
    for index in 0..super::super::super::WIDTH {
        let mut forged = fixture.clone();
        forged.0.row[index] = forged.0.row[index].add(F::ONE);
        assert!(!forged.accepts(&program), "workspace {index}");
    }
}

#[test]
fn native_getgas_completes_with_zero_remaining_gas() {
    let instruction = enc::encode_rr(wide::system::GETGAS, 4, 2, 3);
    let needs_gas = enc::encode_ri(wide::arithmetic::ADDI, 5, 0, 1);
    let (program, recorder, outcome) = shifts::capture(
        &[instruction, needs_gas],
        &[(4, 17, true)],
        root_setup_gas(),
        32,
    );
    // Invocation setup is funded. GETGAS itself succeeds with no remaining
    // gas; the following one-gas instruction is the only attempted opcode
    // that traps. No invocation-success claim is made from this partial row.
    assert!(matches!(outcome, Err(ivm::VMError::OutOfGas)));
    assert_eq!(recorder.records().len(), 2);
    let record = &recorder.records()[0];
    assert_eq!(record.instruction, Some(instruction));
    assert_eq!(record.opcode_gas, Some(0));
    assert_eq!(record.outcome, DiagnosticStepOutcome::Completed);
    assert_eq!(record.before.gas_remaining, 0);
    assert_eq!(record.after.gas_remaining, 0);
    assert_eq!(record.before.registers[4], 17);
    assert!(record.before.tags[4]);
    assert_eq!(record.after.registers[4], 0);
    assert!(!record.after.tags[4]);
    assert_eq!(record.after.pc, record.before.pc + 4);
    assert_eq!(record.after.cycles, record.before.cycles + 1);
    let fixture = ScalarFixture::from_record(&program, record);
    assert!(fixture.accepts(&program));
    let next = &recorder.records()[1];
    assert_eq!(next.instruction, Some(needs_gas));
    assert_eq!(next.opcode_gas, Some(1));
    assert!(matches!(next.outcome, DiagnosticStepOutcome::Trapped(_)));
    assert_eq!(next.before.registers, next.after.registers);
    assert_eq!(next.before.tags, next.after.tags);
    assert_eq!(next.before.pc, next.after.pc);
    assert_eq!(next.before.cycles, next.after.cycles);

    // A wrapped subtraction is not a way to turn a zero-cost read into a
    // fabricated maximum gas value. Bind the exact original zero-gas input.
    let mut forged = fixture.clone();
    bits(&mut forged.0.row[WORDS + 128..WORDS + 192], u64::MAX);
    carries(&mut forged.0.row[CARRIES..CARRIES + 4], 0, 1, true);
    for limb in 0..4 {
        forged.0.packets.fields[GAS_DEBIT][AFTER + limb] = constant_limb(u64::MAX, limb);
        forged.0.packets.fields[SCALAR_DESTINATION][AFTER + limb] = constant_limb(u64::MAX, limb);
    }
    assert!(!forged.accepts(&program));
}
