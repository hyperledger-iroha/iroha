//! GETGAS binds its original zero-debit gas word without fabricated register reads.

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
fn getgas_rejects_substituted_gas_words_coherent_wrong_tariffs_tags_and_all_original_mutations() {
    let instruction = enc::encode_rr(wide::system::GETGAS, 4, 4, 4);
    let (program, fixture) = captured(instruction, (1 << 48) + 100);
    let gas_before = packet::half(&fixture.0.packets.fields[GAS_DEBIT], BEFORE, 0);
    let gas_after = packet::half(&fixture.0.packets.fields[GAS_DEBIT], AFTER, 0);
    // Native GETGAS charges zero, so pre-step and post-step gas are equal.
    // A distinct caller-supplied value on either side of that word must fail.
    assert_eq!(gas_before, gas_after);
    for wrong in [gas_before - 1, gas_before + 1] {
        let mut substituted = fixture.clone();
        for limb in 0..4 {
            substituted.0.packets.fields[SCALAR_DESTINATION][AFTER + limb] =
                constant_limb(wrong, limb);
        }
        assert!(!substituted.accepts(&program));
    }
    for cost in [1, 2, 3] {
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
fn getgas_at_zero_gas_commits_and_rejects_forged_debits_or_underflow() {
    for rd in [0, 4] {
        let instruction = enc::encode_rr(wide::system::GETGAS, rd, 2, 3);
        // Invocation setup is funded exactly. GETGAS then commits with zero
        // remaining gas; any later return-ABI instruction is a separate step.
        let (program, fixture) = captured(instruction, root_setup_gas());
        for offset in [BEFORE, AFTER] {
            assert_eq!(
                packet::half(&fixture.0.packets.fields[GAS_DEBIT], offset, 0),
                0
            );
        }
        assert_eq!(
            packet::half(&fixture.0.packets.fields[SCALAR_DESTINATION], AFTER, 0),
            0
        );
        for wrong in [1, u64::MAX] {
            let mut forged = fixture.clone();
            bits(&mut forged.0.row[WORDS + 128..WORDS + 192], wrong);
            // The wrapped subtraction is otherwise coherent with a fabricated
            // one-unit debit; the native zero tariff and final borrow reject it.
            carries(
                &mut forged.0.row[CARRIES..CARRIES + 4],
                0,
                u64::from(wrong == u64::MAX),
                true,
            );
            for limb in 0..4 {
                forged.0.packets.fields[GAS_DEBIT][AFTER + limb] = constant_limb(wrong, limb);
                if rd != 0 {
                    forged.0.packets.fields[SCALAR_DESTINATION][AFTER + limb] =
                        constant_limb(wrong, limb);
                }
            }
            assert!(!forged.accepts(&program));
        }
        // Keep real out-of-gas coverage at the authenticated invocation setup.
        // It produces no GETGAS attempt and cannot be called an opcode trap.
        for gas in 0..root_setup_gas() {
            let (program, recorder, outcome) =
                shifts::capture(&[instruction], &[(4, 17, true)], gas, 32);
            assert!(matches!(outcome, Err(ivm::VMError::OutOfGas)));
            assert_root_preflight_out_of_gas(&program, &recorder, gas);
        }
    }
}
