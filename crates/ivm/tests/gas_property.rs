//! Gas accounting property tests for IVM
//!
//! These tests assert that summing the static gas costs for a sequence of
//! instructions equals the gas consumed by the VM during execution for simple
//! programs without branches or syscalls. This guards the contract that the
//! canonical gas table in `ivm::gas::cost_of[_with_params]` matches the
//! interpreter’s runtime accounting across scalar, memory, and vector-heavy
//! programs.

#[test]
fn gas_matches_sum_for_simple_arith_sequence() {
    // Build a tiny program: header + 4x ADDI + HALT.
    // Use RV‑compat ADDI encoding accepted by the IVM decoder.
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let mut prog = meta.encode();

    // Four ADDI instructions (cost 1 each) and a HALT (cost 0)
    let a1 = ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 1, 0, 1);
    let a2 = ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 2, 0, 2);
    let a3 = ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 3, 0, 3);
    let a4 = ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 4, 0, 4);
    let halt = HALT_WORD;

    for w in [a1, a2, a3, a4, halt] {
        prog.extend_from_slice(&w.to_le_bytes());
    }

    // Expected gas is the sum of static costs for the words (vector_len = 1)
    let expected: u64 = [a1, a2, a3, a4, halt].into_iter().map(ivm::cost_of).sum();

    let gas_limit = 10_000;
    let mut vm = ivm::IVM::new(gas_limit);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("execute");
    let used = gas_limit - vm.gas_remaining;
    assert_eq!(used, expected, "gas used must equal sum of cost_of(words)");
}

#[test]
fn gas_matches_sum_for_mixed_scalar_sequence() {
    // Program: store → load → add → mul → div → halt.
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let mut prog = meta.encode();

    let store = ivm::encoding::wide::encode_store(ivm::instruction::wide::memory::STORE64, 2, 3, 0);
    let load = ivm::encoding::wide::encode_load(ivm::instruction::wide::memory::LOAD64, 6, 2, 0);
    let add = ivm::encoding::wide::encode_rr(ivm::instruction::wide::arithmetic::ADD, 7, 4, 5);
    let mul = ivm::encoding::wide::encode_rr(ivm::instruction::wide::arithmetic::MUL, 8, 5, 7);
    let div = ivm::encoding::wide::encode_rr(ivm::instruction::wide::arithmetic::DIV, 9, 8, 5);
    let halt = HALT_WORD;

    let words = [store, load, add, mul, div, halt];
    for &word in &words {
        prog.extend_from_slice(&word.to_le_bytes());
    }

    let expected: u64 = words.iter().map(|&word| ivm::cost_of(word)).sum();

    let gas_limit = 10_000;
    let mut vm = ivm::IVM::new(gas_limit);
    vm.load_program(&prog).expect("load program");

    // Seed registers so arithmetic, load, and store execute without traps.
    let base = ivm::Memory::HEAP_START + 0x200;
    vm.registers.set(2, base);
    vm.registers.set(3, 0xDEAD_BEEF_CAFE_BABE);
    vm.registers.set(4, 120);
    vm.registers.set(5, 5);

    vm.run().expect("execute mixed scalar program");

    let used = gas_limit - vm.gas_remaining;
    assert_eq!(
        used, expected,
        "gas must match static schedule for mixed sequence"
    );

    // Validate side-effects to ensure every instruction executed.
    let mut stored = [0u8; 8];
    vm.memory
        .load_bytes(base, &mut stored)
        .expect("load stored bytes");
    assert_eq!(stored, 0xDEAD_BEEF_CAFE_BABE_u64.to_le_bytes());
    assert_eq!(vm.registers.get(6), 0xDEAD_BEEF_CAFE_BABE);
    assert_eq!(vm.registers.get(7), 125);
    assert_eq!(vm.registers.get(8), 625);
    assert_eq!(vm.registers.get(9), 125);
}

#[test]
fn gas_scales_with_vector_length_for_vadd32() {
    // Program: header (VECTOR enabled, logical VL=2) + 3x VADD32 + HALT
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: ivm::ivm_mode::VECTOR,
        // Use VL=2 to match the interpreter's base lanes consistently across
        // hosts. Larger VL gets clamped to the host maximum and may vary.
        vector_length: 2,
        max_cycles: 0,
        abi_version: 1,
    };
    let mut prog = meta.encode();

    // Encode three VADD32 words using the native 8‑bit primary opcode form.
    let v1 = ivm::encoding::wide::encode_rr(ivm::instruction::wide::crypto::VADD32, 10, 11, 12);
    let v2 = ivm::encoding::wide::encode_rr(ivm::instruction::wide::crypto::VADD32, 13, 14, 15);
    let v3 = ivm::encoding::wide::encode_rr(ivm::instruction::wide::crypto::VADD32, 16, 17, 18);
    let halt = HALT_WORD;

    for w in [v1, v2, v3, halt] {
        prog.extend_from_slice(&w.to_le_bytes());
    }

    // Compute expected gas using the canonical helper with vector_len=4.
    let vl = 2usize;
    let expected: u64 = [v1, v2, v3, halt]
        .into_iter()
        .map(|w| ivm::cost_of_with_params(w, vl, 0))
        .sum();

    let gas_limit = 10_000;
    let mut vm = ivm::IVM::new(gas_limit);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("execute");
    let used = gas_limit - vm.gas_remaining;
    assert_eq!(used, expected, "vector op gas must scale with VL");
}
const HALT_WORD: u32 = ivm::encoding::wide::encode_halt();
