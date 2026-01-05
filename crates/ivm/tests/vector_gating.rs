//! Tests for header feature gating of vector instructions.

use ivm::{IVM, Memory, ProgramMetadata, VMError, encoding, instruction};

fn build_prog_with_vector(vmode: bool) -> Vec<u8> {
    // Emit a single VADD32 between vector registers v0 <- v0 + v1, then HALT.
    let mut code = Vec::new();
    // opcode=VADD32, rd=0, rs1=0, rs2=1 in the wide encoding.
    let word = encoding::wide::encode_rr(instruction::wide::crypto::VADD32, 0, 0, 1);
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut meta = ProgramMetadata::default();
    if vmode {
        meta.mode |= ivm::ivm_mode::VECTOR;
    }
    meta.abi_version = 1;
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

fn build_prog_with_load128(vmode: bool) -> Vec<u8> {
    let mut code = Vec::new();
    let word = encoding::wide::encode_load128(instruction::wide::memory::LOAD128, 2, 1, 3);
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut meta = ProgramMetadata::default();
    if vmode {
        meta.mode |= ivm::ivm_mode::VECTOR;
    }
    meta.abi_version = 1;
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

fn build_prog_with_store128(vmode: bool) -> Vec<u8> {
    let mut code = Vec::new();
    let word = encoding::wide::encode_store128(instruction::wide::memory::STORE128, 1, 2, 3);
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut meta = ProgramMetadata::default();
    if vmode {
        meta.mode |= ivm::ivm_mode::VECTOR;
    }
    meta.abi_version = 1;
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

#[test]
fn vector_ops_rejected_without_header_bit() {
    let prog = build_prog_with_vector(false);
    let mut vm = IVM::new(10_000);
    // Initialize vector registers used
    vm.set_vector_register(0, [1, 2, 3, 4]);
    vm.set_vector_register(1, [5, 6, 7, 8]);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    match err {
        ivm::VMError::VectorExtensionDisabled => {}
        other => panic!("expected VectorExtensionDisabled, got {other:?}"),
    }
}

#[test]
fn vector_ops_allowed_with_header_bit() {
    let prog = build_prog_with_vector(true);
    let mut vm = IVM::new(10_000);
    vm.set_vector_register(0, [1, 2, 3, 4]);
    vm.set_vector_register(1, [10, 20, 30, 40]);
    vm.load_program(&prog).unwrap();
    let _ = vm.run();
    // When header enables vector, VM should successfully execute and update v0
    let out = vm.vector_register(0);
    assert_eq!(out, [11, 22, 33, 44]);
}

#[test]
fn load128_rejected_without_header_bit() {
    let prog = build_prog_with_load128(false);
    let mut vm = IVM::new(10_000);
    vm.set_register(1, Memory::HEAP_START);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::VectorExtensionDisabled));
}

#[test]
fn store128_rejected_without_header_bit() {
    let prog = build_prog_with_store128(false);
    let mut vm = IVM::new(10_000);
    vm.set_register(1, Memory::HEAP_START);
    vm.set_register(2, 0xDEAD_BEEF_CAFE_BABE);
    vm.set_register(3, 0x0123_4567_89AB_CDEF);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::VectorExtensionDisabled));
}
