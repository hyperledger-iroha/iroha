//! Destination spans checked against original ordinary register transitions.

use super::*;
use crate::{
    IVM, ProgramMetadata,
    encoding::wide::{encode_halt, encode_ri, encode_rr, encode_sys, encode_scall},
    execution_step_recorder::{DiagnosticStepOutcome, DiagnosticStepRecorder},
    host::DefaultHost,
    instruction::wide::{arithmetic as a, control as c, memory as m, system as s},
};
use iroha_allocation::AllocationBudget;

fn observed(word: u32, initialize: impl FnOnce(&mut IVM)) -> usize {
    let mut image = ProgramMetadata {
        mode: crate::ivm_mode::ZK,
        max_cycles: 64,
        ..ProgramMetadata::default()
    }
    .encode();
    image.extend(word.to_le_bytes());
    image.extend(encode_halt().to_le_bytes());
    let mut vm = IVM::try_new(10_000).unwrap();
    vm.load_program(&image).unwrap();
    vm.set_zk_trace_enabled(false);
    initialize(&mut vm);
    let original = AllocationBudget::new(1 << 20);
    let mut recorder = DiagnosticStepRecorder::try_new(2, &original).unwrap();
    vm.run_with_host_diagnostic_steps(&mut DefaultHost::default(), &mut recorder)
        .unwrap();
    let record = &recorder.records()[0];
    assert_eq!(record.instruction, Some(word));
    assert_eq!(record.outcome, DiagnosticStepOutcome::Completed);
    record.changed_registers().count()
}

#[test]
fn every_admitted_opcode_has_a_bounded_public_destination_span() {
    for opcode in u8::MIN..=u8::MAX {
        let result = instruction(encode_rr(opcode, 7, 5, 6));
        assert_eq!(
            result.is_ok(),
            wide::is_valid_opcode(opcode),
            "{opcode:#04x}"
        );
        match result {
            Ok(changes) => assert!(changes <= 255, "{opcode:#04x}: {changes}"),
            // The rejection names the opcode byte, never the operand bits (0x0506 here).
            Err(error) => assert!(
                matches!(error, VMError::InvalidOpcode(reported) if reported == opcode),
                "{opcode:#04x}: {error:?}"
            ),
        }
    }
}

#[test]
fn aliases_zero_and_ignored_operand_bytes_cannot_invent_extra_destinations() {
    for destination in [0, 5, 6, 255] {
        for opcode in [a::ADD, a::CMOV, a::NOT, a::CLZ, m::LOAD64, m::LDI64] {
            for ignored in [0, 5, 255] {
                assert_eq!(
                    instruction(encode_rr(opcode, destination, 5, ignored)).unwrap(),
                    usize::from(destination != 0)
                );
            }
        }
    }
}

#[test]
fn actual_scalar_conditional_and_tag_only_changes_fit_the_same_public_bound() {
    for destination in [0, 5, 255] {
        for opcode in [a::ADD, a::ADDI, a::MULHSU, a::CLZ, a::CMOV] {
            let word = encode_rr(opcode, destination, 5, 6);
            for value in [0, 19] {
                let actual = observed(word, |vm| {
                    vm.registers.set(5, value);
                    vm.registers.set(6, 3);
                });
                assert!(actual <= instruction(word).unwrap());
            }
        }
    }
    // Value remains nine; the inherited source tag changes the destination.
    let word = encode_ri(a::ADDI, 7, 5, 0);
    assert_eq!(
        observed(word, |vm| {
            vm.registers.set(5, 9);
            vm.registers.set_tag(5, true);
            vm.registers.set(7, 9);
        }),
        1
    );
    assert_eq!(instruction(word).unwrap(), 1);
}

#[test]
fn both_syscall_encodings_fund_the_entire_architectural_net_change_set() {
    for &number in crate::syscalls::abi_syscall_list() {
        assert_eq!(instruction(encode_scall(number)).unwrap(), 255);
        if let Ok(number) = u8::try_from(number) {
            assert_eq!(instruction(encode_scall(u32::from(number))).unwrap(), 255);
        }
    }
}

#[test]
fn control_and_memory_reads_do_not_become_register_changes() {
    for opcode in [
        m::STORE64,
        c::BEQ,
        c::BNE,
        c::BLT,
        c::BGE,
        c::BLTU,
        c::BGEU,
        c::JMP,
        c::HALT,
    ] {
        assert_eq!(instruction(encode_rr(opcode, 7, 5, 6)).unwrap(), 0);
    }
    assert_eq!(instruction(encode_rr(c::JALS, 0, 0, 1)).unwrap(), 1);
    assert_eq!(instruction(encode_rr(c::JALR, 0, 1, 0)).unwrap(), 0);
}
