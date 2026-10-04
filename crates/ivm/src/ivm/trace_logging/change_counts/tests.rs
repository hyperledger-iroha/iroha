//! Destination spans checked against original ordinary register transitions.

use super::*;
use crate::{
    IVM, ProgramMetadata,
    encoding::wide::{encode_halt, encode_ri, encode_rr, encode_sys, encode_syscallx},
    execution_step_recorder::{DiagnosticStepOutcome, DiagnosticStepRecorder},
    host::DefaultHost,
    instruction::wide::{arithmetic as a, control as c, crypto as v, memory as m, system as s},
};
use iroha_allocation::AllocationBudget;

fn observed(word: u32, lanes: u8, initialize: impl FnOnce(&mut IVM)) -> usize {
    let mut image = ProgramMetadata {
        mode: crate::ivm_mode::ZK | crate::ivm_mode::VECTOR,
        vector_length: lanes,
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
        for lanes in [0, 1, 64, usize::MAX] {
            let result = instruction(encode_rr(opcode, 7, 5, 6), lanes);
            assert_eq!(
                result.is_ok(),
                wide::is_valid_opcode(opcode),
                "{opcode:#04x}"
            );
            if let Ok(changes) = result {
                assert!(changes <= 255, "{opcode:#04x}: {changes}");
            }
        }
    }
}

#[test]
fn aliases_zero_and_ignored_operand_bytes_cannot_invent_extra_destinations() {
    for destination in [0, 5, 6, 255] {
        for opcode in [a::ADD, a::CMOV, a::NOT, a::CLZ, s::GETGAS, m::LOAD64] {
            for ignored in [0, 5, 255] {
                assert_eq!(
                    instruction(encode_rr(opcode, destination, 5, ignored), 64).unwrap(),
                    usize::from(destination != 0)
                );
            }
        }
        for high in [0, 5, 6, 255] {
            for opcode in [m::LOAD128, v::ED25519BATCHVERIFY] {
                let expected =
                    usize::from(destination != 0) + usize::from(high != 0 && high != destination);
                assert_eq!(
                    instruction(encode_rr(opcode, destination, 5, high), 4).unwrap(),
                    expected
                );
            }
        }
        for opcode in [v::AESENC, v::AESDEC, v::BLAKE2S] {
            assert_eq!(
                instruction(encode_rr(opcode, destination, 5, 6), 4).unwrap(),
                usize::from(destination != 0) + usize::from(destination < 255)
            );
        }
    }
}

#[test]
fn actual_scalar_conditional_and_tag_only_changes_fit_the_same_public_bound() {
    for destination in [0, 5, 255] {
        for opcode in [a::ADD, a::ADDI, a::MULHSU, a::CLZ, a::CMOV] {
            let word = encode_rr(opcode, destination, 5, 6);
            for value in [0, 19] {
                let actual = observed(word, 4, |vm| {
                    vm.registers.set(5, value);
                    vm.registers.set(6, 3);
                });
                assert!(actual <= instruction(word, 4).unwrap());
            }
        }
    }
    // Value remains nine; the inherited source tag changes the destination.
    let word = encode_ri(a::ADDI, 7, 5, 0);
    assert_eq!(
        observed(word, 4, |vm| {
            vm.registers.set(5, 9);
            vm.registers.set_tag(5, true);
            vm.registers.set(7, 9);
        }),
        1
    );
    assert_eq!(instruction(word, 4).unwrap(), 1);
}

#[test]
fn actual_vector_tails_and_sha_state_strides_fit_public_geometry() {
    for lanes in [1u8, 2, 6, 64] {
        for opcode in [v::VADD32, v::VADD64, v::VAND, v::VXOR, v::VOR, v::VROT32] {
            if opcode == v::VADD64 && !lanes.is_multiple_of(2) {
                continue;
            }
            let word = encode_rr(opcode, 2, 0, 1);
            let actual = observed(word, lanes, |vm| {
                for register in 32..32 + 2 * usize::from(lanes) {
                    vm.registers.set(register, register as u64);
                }
            });
            assert!(actual <= instruction(word, usize::from(lanes)).unwrap());
            assert_eq!(
                instruction(word, usize::from(lanes)).unwrap(),
                usize::from(lanes)
            );
        }
        let word = encode_rr(v::SHA256BLOCK, 0, 5, 0);
        let actual = observed(word, lanes, |vm| {
            vm.registers.set(5, crate::Memory::HEAP_START);
            vm.memory
                .store_bytes(crate::Memory::HEAP_START, &[0x5a; 64])
                .unwrap();
        });
        let maximum = 2 * usize::from(lanes.min(4));
        assert_eq!(instruction(word, usize::from(lanes)).unwrap(), maximum);
        assert!(actual <= maximum);
    }
}

#[test]
fn both_syscall_encodings_fund_the_entire_architectural_net_change_set() {
    for &number in crate::syscalls::abi_syscall_list() {
        assert_eq!(instruction(encode_syscallx(number), 4).unwrap(), 255);
        if let Ok(number) = u8::try_from(number) {
            assert_eq!(instruction(encode_sys(s::SCALL, number), 4).unwrap(), 255);
        }
    }
}

#[test]
fn control_memory_and_assertion_reads_do_not_become_register_changes() {
    use crate::instruction::wide::zk as z;
    for opcode in [
        m::STORE64,
        m::STORE128,
        c::BEQ,
        c::BNE,
        c::BLT,
        c::BGE,
        c::BLTU,
        c::BGEU,
        c::JR,
        c::JMP,
        c::HALT,
        v::SHA3BLOCK,
        v::SETVL,
        v::PARBEGIN,
        v::PAREND,
        z::ASSERT,
        z::ASSERT_EQ,
        z::ASSERT_RANGE,
    ] {
        assert_eq!(instruction(encode_rr(opcode, 7, 5, 6), 64).unwrap(), 0);
    }
    assert_eq!(instruction(encode_rr(c::JALS, 0, 0, 1), 4).unwrap(), 1);
    assert_eq!(instruction(encode_rr(c::JALR, 0, 1, 0), 4).unwrap(), 0);
}
