//! Reservation bounds checked against ordinary interpreter register observations.

use super::*;
use crate::{
    IVM, ProgramMetadata,
    encoding::wide::{encode_halt, encode_ri, encode_rr, encode_sys, encode_syscallx},
    instruction::wide::{
        arithmetic as a, control as c, crypto as v, memory as m, system as s, zk as z,
    },
};
use iroha_allocation::AllocationBudget;

fn observed(word: u32, lanes: u8, initialize: impl FnOnce(&mut IVM)) -> usize {
    let mut program = ProgramMetadata {
        mode: crate::ivm_mode::ZK | crate::ivm_mode::VECTOR,
        vector_length: lanes,
        max_cycles: 64,
        ..ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(&word.to_le_bytes());
    program.extend_from_slice(&encode_halt().to_le_bytes());
    let mut vm = IVM::try_new(10_000).unwrap();
    vm.load_program(&program).unwrap();
    vm.set_zk_trace_enabled(true);
    initialize(&mut vm);
    vm.run().unwrap();
    // Use the public borrowed diagnostic boundary, independent of the rows
    // owner's representation. HALT and cycle padding add no register events.
    let snapshot = vm
        .try_diagnostic_snapshot(&AllocationBudget::new(128 * 1024 * 1024))
        .unwrap();
    snapshot.register_event_count()
}

#[test]
fn every_admitted_opcode_has_a_finite_public_geometry_bound() {
    for opcode in u8::MIN..=u8::MAX {
        let result = instruction(encode_rr(opcode, 7, 5, 6), 64, true, true);
        assert_eq!(
            result.is_ok(),
            wide::is_valid_opcode(opcode),
            "opcode {opcode:#04x}"
        );
        if let Ok(rows) = result {
            // Current maximum is the bounded syscall host write publication;
            // ordinary 64-lane vector work occupies exactly 256 rows.
            assert!(rows <= 280, "opcode {opcode:#04x}: {rows}");
        }
    }
}

#[test]
fn scalar_bounds_match_real_value_reads_and_separate_tag_writes() {
    let binary = [
        a::ADD,
        a::SUB,
        a::AND,
        a::OR,
        a::XOR,
        a::SLL,
        a::SRL,
        a::SRA,
        a::MUL,
        a::MULH,
        a::MULHU,
        a::MULHSU,
        a::DIV,
        a::DIVU,
        a::REM,
        a::REMU,
        a::SLT,
        a::SLTU,
        a::SEQ,
        a::SNE,
        a::ROTL,
        a::ROTR,
        a::MIN,
        a::MAX,
        a::DIV_CEIL,
        a::GCD,
        a::MEAN,
        z::FADD,
        z::FSUB,
        z::FMUL,
    ];
    let unary = [
        a::ADDI,
        a::ANDI,
        a::ORI,
        a::XORI,
        a::NEG,
        a::NOT,
        a::ROTL_IMM,
        a::ROTR_IMM,
        a::POPCNT,
        a::CLZ,
        a::CTZ,
        a::ISQRT,
        a::ABS,
        z::FINV,
    ];
    for destination in [0, 5, 6, 255] {
        for (opcodes, reads) in [(&binary[..], 2), (&unary[..], 1)] {
            for &opcode in opcodes {
                let word = encode_rr(opcode, destination, 5, 6);
                let actual = observed(word, 4, |vm| {
                    vm.set_register(5, 9);
                    vm.set_register(6, 3);
                });
                assert_eq!(
                    actual,
                    reads + if destination == 0 { 0 } else { 2 },
                    "opcode {opcode:#04x}, r{destination}"
                );
                assert_eq!(instruction(word, 4, false, false).unwrap(), actual);
            }
        }
    }
}

#[test]
fn conditional_moves_reserve_the_public_maximum_before_the_condition_is_read() {
    for opcode in [a::CMOV, a::CMOVI] {
        for condition in [0, 1] {
            for destination in [0, 5, 6] {
                let word = if opcode == a::CMOV {
                    encode_rr(opcode, destination, 5, 6)
                } else {
                    encode_ri(opcode, destination, 6, -7)
                };
                let actual = observed(word, 4, |vm| {
                    vm.set_register(5, 13);
                    vm.set_register(6, condition);
                });
                let maximum = instruction(word, 4, false, false).unwrap();
                if condition == 0 {
                    assert_eq!(actual, 1);
                    assert!(actual <= maximum);
                } else {
                    assert_eq!(actual, maximum);
                }
            }
        }
    }
}

#[test]
fn vector_bounds_cover_scalar_tails_and_the_maximum_public_lane_count() {
    for lanes in [1u8, 2, 4, 6, 64] {
        for opcode in [v::VADD32, v::VADD64, v::VAND, v::VXOR, v::VOR, v::VROT32] {
            if opcode == v::VADD64 && !lanes.is_multiple_of(2) {
                continue;
            }
            let word = encode_rr(opcode, 2, 0, 1);
            let actual = observed(word, lanes, |vm| {
                for register in 32..32 + 2 * usize::from(lanes) {
                    vm.set_register(register, register as u64);
                }
            });
            assert_eq!(
                actual,
                usize::from(lanes) * if opcode == v::VROT32 { 3 } else { 4 }
            );
            assert_eq!(
                instruction(word, usize::from(lanes), false, false).unwrap(),
                actual
            );
        }
    }
}

#[test]
fn sha_state_lanes_are_bounded_independently_of_the_public_stride() {
    for lanes in [1u8, 2, 4, 64] {
        let word = encode_rr(v::SHA256BLOCK, 0, 5, 0);
        let actual = observed(word, lanes, |vm| {
            vm.set_register(5, crate::Memory::HEAP_START);
            vm.memory
                .store_bytes(crate::Memory::HEAP_START, &[0x5a; 64])
                .unwrap();
        });
        assert_eq!(actual, 1 + 6 * usize::from(lanes.min(4)));
        assert_eq!(
            instruction(word, usize::from(lanes), false, false).unwrap(),
            actual
        );
    }
}

#[test]
fn strict_call_and_root_subtrees_include_all_descriptor_reads() {
    assert_eq!(root(RootArguments::Empty, false), 16);
    assert_eq!(root(RootArguments::Prepared, false), 16);
    assert_eq!(
        root(RootArguments::DefaultHost, false),
        18 + syscall(syscalls::SYSCALL_GET_PUBLIC_INPUT)
    );
    for route in [
        RootArguments::Empty,
        RootArguments::Prepared,
        RootArguments::DefaultHost,
    ] {
        assert_eq!(root(route, true), root(route, false) + 6);
    }
    for (word, loose, strict) in [
        (encode_rr(c::JAL, 1, 0, 1), 2, 11),
        (encode_rr(c::JAL, 7, 0, 1), 2, 2),
        (encode_rr(c::JALS, 0, 0, 1), 2, 11),
        (encode_rr(c::JALR, 0, 1, 0), 1, 6),
    ] {
        assert_eq!(instruction(word, 4, false, false).unwrap(), loose);
        assert_eq!(instruction(word, 4, true, false).unwrap(), strict);
    }
}

#[test]
fn syscall_budget_contains_both_restore_and_host_net_change_paths() {
    for &number in crate::syscalls::abi_syscall_list() {
        let inputs = crate::ivm::syscall_public_input_registers(number);
        let outputs = crate::ivm::syscall_public_output_registers(number);
        let output_only = outputs
            .iter()
            .filter(|register| !inputs.contains(register))
            .count();
        let initial = if number == syscalls::SYSCALL_PRIVATE_NUMERIC_VALCOM {
            0
        } else {
            inputs.len()
        };
        let bound = syscall(number);
        // Each saved output is read, cleared, then retagged before the host;
        // quote rejection restores its value and tag instead of calling it.
        assert!(bound >= initial + 5 * output_only);
        assert!(bound >= initial + 3 * output_only + 255);
        assert_eq!(
            instruction(encode_syscallx(number), 4, false, false).unwrap(),
            bound
        );
        if let Ok(number8) = u8::try_from(number) {
            assert_eq!(
                instruction(encode_sys(s::SCALL, number8), 4, false, false).unwrap(),
                bound
            );
        }
    }
    assert_eq!(syscall(syscalls::SYSCALL_PRIVATE_NUMERIC_VALCOM), 256);
    let private_input = syscall(syscalls::SYSCALL_GET_PRIVATE_INPUT);
    assert!(private_input >= 256);
}

#[test]
fn native_observations_count_original_before_and_delayed_after_reads_even_for_r0() {
    for destination in [0, 5, 255] {
        for (opcode, extra) in [
            (a::ADD, 4),
            (a::MULHSU, 4),
            (a::ADDI, 3),
            (a::CLZ, 3),
            (m::LDI64, 2),
            (m::LOAD64, 5),
            (m::STORE64, 4),
            (c::JALR, 4),
        ] {
            let word = encode_rr(opcode, destination, 5, 6);
            assert_eq!(
                instruction(word, 4, true, true).unwrap(),
                instruction(word, 4, true, false).unwrap() + extra
            );
        }
    }
    // Unsupported native observations do not invent private packet work.
    for opcode in [a::DIV, a::CMOV, s::GETGAS, c::BEQ] {
        assert_eq!(native_observations(encode_rr(opcode, 7, 5, 6)), 0);
    }
}

#[test]
fn malformed_or_unrepresentable_geometry_cannot_wrap_a_reservation() {
    assert!(matches!(
        instruction(encode_rr(0xff, 7, 5, 6), 4, false, false),
        Err(VMError::InvalidOpcode(_))
    ));
    for opcode in [v::VADD32, v::VROT32] {
        assert!(matches!(
            instruction(encode_rr(opcode, 2, 0, 1), usize::MAX, false, false),
            Err(VMError::AllocationDeferred(
                AllocationRefusal::DemandOverflow
            ))
        ));
    }
    assert_eq!(
        instruction(encode_rr(v::VADD32, 2, 0, 1), 0, false, false).unwrap(),
        4
    );
    for (bits, rows) in [(0, 1), (64, 1), (65, 0), (255, 0)] {
        assert_eq!(
            instruction(encode_rr(z::ASSERT_RANGE, 0, 5, bits), 4, false, false).unwrap(),
            rows
        );
    }
}
