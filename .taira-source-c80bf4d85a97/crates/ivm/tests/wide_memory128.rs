//! Wide opcode 128-bit memory operation tests.

use ivm::{
    IVM, Memory, ProgramMetadata, VMError, encoding, instruction, ivm_mode,
    kotodama::wide::{encode_addi, encode_load128, encode_store128},
};

const HALT: u32 = encoding::wide::encode_halt();

fn program_header() -> Vec<u8> {
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: ivm_mode::VECTOR,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    meta.encode()
}

fn append_word(buf: &mut Vec<u8>, word: u32) {
    buf.extend_from_slice(&word.to_le_bytes());
}

#[test]
fn wide_store_load128_roundtrip() {
    let target_addr = Memory::HEAP_START + 0x80;
    let adjust = encode_addi(4, 4, 16);
    let store = encode_store128(4, 5, 6);
    let load = encode_load128(4, 8, 9);

    let mut program = program_header();
    for word in [adjust, store, load, HALT] {
        append_word(&mut program, word);
    }

    let mut vm = IVM::new(u64::MAX);
    // Seed registers: base register points 16 bytes behind the target so the
    // ADDI brings it to the aligned chunk boundary required by the 128-bit ops.
    vm.registers.set(4, target_addr - 16);
    vm.registers.set(5, 0xDEAD_BEEF_CAFE_BABE);
    vm.registers.set(6, 0x0123_4567_89AB_CDEF);

    vm.load_program(&program).expect("load program");
    vm.run().expect("execute program");

    let lo = vm.registers.get(8);
    let hi = vm.registers.get(9);
    assert_eq!(lo, 0xDEAD_BEEF_CAFE_BABE);
    assert_eq!(hi, 0x0123_4567_89AB_CDEF);

    // Ensure the memory footprint matches the reconstructed 128-bit lane.
    let bytes = vm
        .memory
        .load_u128(target_addr)
        .expect("load stored 128-bit value");
    assert_eq!(bytes, ((hi as u128) << 64) | lo as u128);

    // Sanity-check that the high destination register really came from the
    // third operand slot.
    assert_eq!(instruction::wide::rs2(load), 9);
}

#[test]
fn wide_store_load128_requires_alignment() {
    let target_addr = Memory::HEAP_START + 0x88;
    let adjust = encode_addi(4, 4, 16);
    let store = encode_store128(4, 5, 6);

    let mut program = program_header();
    for word in [adjust, store, HALT] {
        append_word(&mut program, word);
    }

    let mut vm = IVM::new(u64::MAX);
    vm.registers.set(4, target_addr - 16);
    vm.registers.set(5, 0);
    vm.registers.set(6, 0);

    vm.load_program(&program).expect("load program");
    let err = vm.run().expect_err("misaligned 128-bit store should trap");
    assert!(matches!(err, VMError::MisalignedAccess { .. }));
}

#[test]
fn wide_chunked_frame_updates_step_in_16_byte_chunks() {
    let first_addr = Memory::HEAP_START + 0xA0;
    let mut program = program_header();

    let adjust_to_first = encode_addi(4, 4, 16);
    let store_first = encode_store128(4, 5, 6);
    let advance_chunk = encode_addi(4, 4, 16);
    let store_second = encode_store128(4, 7, 8);

    for word in [
        adjust_to_first,
        store_first,
        advance_chunk,
        store_second,
        HALT,
    ] {
        append_word(&mut program, word);
    }

    let mut vm = IVM::new(u64::MAX);
    vm.registers.set(4, first_addr - 16);
    vm.registers.set(5, 0x0102_0304_0506_0708);
    vm.registers.set(6, 0x1112_1314_1516_1718);
    vm.registers.set(7, 0x2122_2324_2526_2728);
    vm.registers.set(8, 0x3132_3334_3536_3738);

    vm.load_program(&program).expect("load program");
    vm.run().expect("execute program");

    let first_chunk = vm.memory.load_u128(first_addr).expect("load first chunk");
    let second_chunk = vm
        .memory
        .load_u128(first_addr + 16)
        .expect("load second chunk");

    assert_eq!(
        first_chunk,
        ((vm.registers.get(6) as u128) << 64) | vm.registers.get(5) as u128
    );
    assert_eq!(
        second_chunk,
        ((vm.registers.get(8) as u128) << 64) | vm.registers.get(7) as u128
    );
}
