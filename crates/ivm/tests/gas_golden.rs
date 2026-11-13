//! Gas cost golden tests for IVM opcodes and syscalls.
use crate::common;
use instruction::wide;
use ivm::{
    IVM, VMError, cost_of as cost_of_opt,
    host::{DefaultHost, IVMHost},
    instruction,
};
fn push32(code: &mut Vec<u8>, word: u32) {
    code.extend_from_slice(&word.to_le_bytes());
}
fn halt32(code: &mut Vec<u8>) {
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
}
fn cost_of(word: u32) -> u64 {
    cost_of_opt(word).expect("valid opcode must have gas cost")
}
fn wide_rr(op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
    ivm::encoding::wide::encode_rr(op, rd, rs1, rs2)
}
fn wide_ri(op: u8, rd: u8, rs1: u8, imm: i16) -> u32 {
    ivm::encoding::wide::encode_ri(op, rd, rs1, imm)
}
fn wide_load(op: u8, rd: u8, base: u8, imm: i16) -> u32 {
    ivm::encoding::wide::encode_load(op, rd, base, imm)
}
fn wide_store(op: u8, base: u8, rs: u8, imm: i16) -> u32 {
    ivm::encoding::wide::encode_store(op, base, rs, imm)
}
fn wide_branch(op: u8, rs1: u8, rs2: u8, offset_words: i16) -> u32 {
    ivm::encoding::wide::encode_branch(op, rs1, rs2, offset_words)
}
fn wide_jump(op: u8, rd: u8, offset_words: i32) -> u32 {
    ivm::encoding::wide::encode_jump(op, rd, offset_words)
}
fn wide_sys(number: u32) -> u32 {
    ivm::encoding::wide::encode_scall(number)
}
fn classic_encode_r16(op: u8, rd: u8, rs1: u8, rs2: u8) -> u16 {
    ((op & 0xF) as u16) << 12
        | ((rd & 0xF) as u16) << 8
        | ((rs1 & 0xF) as u16) << 4
        | (rs2 & 0xF) as u16
}
fn gas_for(instr: u32) -> u64 {
    let mut body = Vec::new();
    push32(&mut body, instr);
    halt32(&mut body);
    let code = common::assemble(&body);
    let start_gas = 10_000u64;
    let mut vm = IVM::new(start_gas);
    vm.load_program(&code).unwrap();
    // Seed any required registers/memory
    vm.registers.set(2, ivm::Memory::HEAP_START);
    vm.registers.set(3, 0xDEAD_BEEF_u64);
    // Ensure the load/store targets point at an initialized heap location.
    let _ = vm
        .memory
        .store_u64(ivm::Memory::HEAP_START, 0xA5A5_5A5A_DEAD_BEEF);
    match vm.run() {
        Ok(()) => start_gas - vm.gas_remaining,
        Err(err) => panic!(
            "gas_for failed: {err:?}, consumed {}",
            start_gas - vm.gas_remaining
        ),
    }
}
#[test]
fn gas_add_mul_div_load_store_branch() {
    let add = wide_rr(wide::arithmetic::ADD, 1, 2, 3);
    assert_eq!(gas_for(add), cost_of(add));
    let mul = wide_rr(wide::arithmetic::MUL, 1, 2, 3);
    assert_eq!(gas_for(mul), cost_of(mul));
    let div = wide_rr(wide::arithmetic::DIV, 1, 2, 3);
    assert_eq!(gas_for(div), cost_of(div));
    let ld = wide_load(wide::memory::LOAD64, 1, 2, 0);
    assert_eq!(gas_for(ld), cost_of(ld));
    let sd = wide_store(wide::memory::STORE64, 2, 3, 0);
    assert_eq!(gas_for(sd), cost_of(sd));
    let beq = wide_branch(wide::control::BEQ, 2, 3, 2);
    assert_eq!(gas_for(beq), cost_of(beq));
}
#[test]
fn gas_branches_always_one() {
    // Construct BEQ/BNE/BLT/BGE/BLTU/BGEU encodings with zero offset
    let rs1 = 2u32;
    let rs2 = 3u32;
    let mkb = |op: u8| -> u32 { wide_branch(op, rs1 as u8, rs2 as u8, 0) };
    for op in [
        wide::control::BEQ,
        wide::control::BNE,
        wide::control::BLT,
        wide::control::BGE,
        wide::control::BLTU,
        wide::control::BGEU,
    ] {
        assert_eq!(cost_of(mkb(op)), 1);
    }
}
#[test]
fn indexed_scalar_load_has_one_gas_base_cost() {
    let load = ivm::encoding::wide::encode_literal(wide::memory::LDI64, 7, u16::MAX);
    assert_eq!(cost_of(load), 1);
}
#[test]
fn gas_linear_sequence_syscall_and_jumps() {
    // Property: sum(cost_of) over a linear sequence equals runtime gas
    // Build a linear sequence with ALU + LOAD/STORE and compare sums
    {
        let mut code = Vec::new();
        // ADD r1 = r2 + r3
        push32(&mut code, wide_rr(wide::arithmetic::ADD, 1, 2, 3));
        // XOR r4 = r1 ^ r5
        push32(&mut code, wide_rr(wide::arithmetic::XOR, 4, 1, 5));
        // SD [r6+0] = r4
        push32(&mut code, wide_store(wide::memory::STORE64, 6, 4, 0));
        // LD r7 = [r6+0]
        push32(&mut code, wide_load(wide::memory::LOAD64, 7, 6, 0));
        halt32(&mut code);
        // Sum schedule
        let words: Vec<u32> = code
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let expected: u64 = words.iter().map(|&w| cost_of(w)).sum();
        let program = common::assemble(&code);
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        vm.load_program(&program).unwrap();
        vm.registers.set(2, 10);
        vm.registers.set(3, 20);
        vm.registers.set(5, 0xFFFF_FFFF_FFFF_0000);
        let base = ivm::Memory::HEAP_START + 0x900;
        vm.registers.set(6, base);
        vm.run().unwrap();
        assert_eq!(start - vm.gas_remaining, expected);
    }
    let scallx = ivm::encoding::wide::encode_scall(ivm::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS);
    assert_eq!(cost_of(scallx), 5);
    // JAL: opcode 0x6F (test schedule via low 7-bit opcode)
    let jal = wide_jump(wide::control::JAL, 0, 0);
    assert_eq!(cost_of(jal), 2);
    // JALR: I-type, opcode 0x67
    let jalr = wide_ri(wide::control::JALR, 2, 1, 0);
    assert_eq!(cost_of(jalr), 2);
}
#[test]
fn schedule_vs_runtime_syscall_extra() {
    // Sequence with a syscall that charges by contiguous eight-byte ABI words.
    // Program: MOV r10=16; SCALL ALLOC; HALT
    let mut code = Vec::new();
    // ADDI r10 = r0 + 16 (wide: opcode 0x20 → rd=10, rs1=0, imm=16)
    let addi_r10_16 = wide_ri(wide::arithmetic::ADDI, 10, 0, 16);
    push32(&mut code, addi_r10_16);
    // SCALL with imm8 = SYSCALL_ALLOC
    let scall = wide_sys(ivm::syscalls::SYSCALL_ALLOC);
    push32(&mut code, scall);
    halt32(&mut code);
    // Sum base schedule; 16 bytes cost base 1 + two ABI words.
    let words: Vec<u32> = code
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let program = common::assemble(&code);
    let base: u64 = words.iter().map(|&w| cost_of(w)).sum();
    let expected = base + 3;
    let start = 10_000u64;
    let mut vm = IVM::new(start);
    vm.load_program(&program).unwrap();
    vm.run().unwrap();
    assert_eq!(start - vm.gas_remaining, expected);
}
#[test]
fn v1_numeric_syscalls_use_staged_metering_without_a_quote() {
    let vm = IVM::new(10_000);
    assert_eq!(
        DefaultHost::new()
            .prepare_syscall(ivm::syscalls::SYSCALL_QUANTITY_ADD, &vm)
            .expect("prepare quantity addition"),
        0,
        "numeric calls debit each bounded phase immediately and never reserve/refund a quote",
    );
}
#[test]
fn v1_json_build_charge_golden() {
    assert_eq!(
        ivm::json::build_json_gas(11, 7, 3, 2, 19),
        74,
        "JSON_BUILD charges its base plus schema, source, word, collection, and output bytes"
    );
    assert_eq!(
        ivm::json::typed_getter_gas(11, 19),
        46,
        "typed JSON getters charge their base plus inspected and materialized bytes"
    );
}
#[test]
fn branch_run_cost_taken_and_not_taken() {
    let mk_code = |branch: u32, r1: u64, r2: u64, with_filler: bool| -> u64 {
        let mut code = Vec::new();
        push32(&mut code, branch);
        if with_filler {
            push32(&mut code, wide_rr(wide::arithmetic::XOR, 10, 10, 10));
        }
        halt32(&mut code);
        // Seed registers before run
        let program = common::assemble(&code);
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        vm.load_program(&program).unwrap();
        vm.registers.set(1, r1);
        vm.registers.set(2, r2);
        vm.run().unwrap();
        // Recreate gas usage as helper returns usage directly
        start - vm.gas_remaining
    };
    // Not taken: BEQ with unequal registers
    let beq_not_taken = wide_branch(wide::control::BEQ, 1, 2, 2);
    let expected_not_taken = {
        let mut code = Vec::new();
        push32(&mut code, beq_not_taken);
        let filler = wide_rr(wide::arithmetic::XOR, 10, 10, 10);
        push32(&mut code, filler);
        halt32(&mut code);
        let words: Vec<u32> = code
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        [0usize, 1usize, 2usize]
            .into_iter()
            .map(|i| cost_of(words[i]))
            .sum::<u64>()
    };
    // Taken: BEQ with equal registers, skip XOR to reach HALT
    let beq_taken = wide_branch(wide::control::BEQ, 1, 2, 2);
    let expected_taken = {
        let mut code = Vec::new();
        push32(&mut code, beq_taken);
        push32(&mut code, wide_rr(wide::arithmetic::XOR, 10, 10, 10));
        halt32(&mut code);
        let words: Vec<u32> = code
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        [0usize, 2usize]
            .into_iter()
            .map(|i| cost_of(words[i]))
            .sum::<u64>()
    };
    assert_eq!(mk_code(beq_not_taken, 1, 2, true), expected_not_taken);
    assert_eq!(mk_code(beq_taken, 5, 5, true), expected_taken);
}
#[test]
fn mixed_sequence_cumulative_gas() {
    // Build: BEQ not taken (1) + ADD (1) + SD (3) + LD (3) + HALT (0) = 8
    let mut code = Vec::new();
    // BEQ r1,r2,+4 (not taken when r1!=r2)
    push32(&mut code, wide_branch(wide::control::BEQ, 1, 2, 1));
    // ADD r5 = r2 + r2
    push32(&mut code, wide_rr(wide::arithmetic::ADD, 5, 2, 2));
    // SD [r3+0] = r4 (funct3=0x3)
    push32(&mut code, wide_store(wide::memory::STORE64, 3, 4, 0));
    // LD r6 = [r3+0] (funct3=0x3)
    push32(&mut code, wide_load(wide::memory::LOAD64, 6, 3, 0));
    halt32(&mut code);
    let words: Vec<u32> = code
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let expected: u64 = words.iter().map(|&w| cost_of(w)).sum();
    let program = common::assemble(&code);
    let start = 10_000u64;
    let mut vm = IVM::new(start);
    vm.load_program(&program).unwrap();
    // r1!=r2 to keep BEQ not taken
    vm.registers.set(1, 1);
    vm.registers.set(2, 2);
    // r3 base address; r4 store value
    let base = ivm::Memory::HEAP_START + 0x700;
    vm.registers.set(3, base);
    vm.registers.set(4, 0xDEAD_BEEF_F00D_BAAFu64);
    vm.memory.store_u64(base, 0xABCD_ABCD_ABCD_ABCDu64).unwrap();
    vm.run().unwrap();
    assert_eq!(start - vm.gas_remaining, expected);
}
#[test]
fn jalr_invariance_gas() {
    // Helper to encode I-type JALR
    for &(rd, rs1) in &[(1u32, 1u32), (31, 1), (5, 2)] {
        let jalr = wide_ri(wide::control::JALR, rd as u8, rs1 as u8, 0x10);
        assert_eq!(cost_of(jalr), 2);
    }
}
#[test]
fn alignment_error_gas_charged_then_error() {
    // LD r1, 1(r2) → misaligned (addr % 8 != 0), should charge load gas then error
    let ld = wide_load(wide::memory::LOAD64, 1, 2, 1);
    let mut code = Vec::new();
    push32(&mut code, ld);
    halt32(&mut code);
    let program = common::assemble(&code);
    let start = 10_000u64;
    let mut vm = IVM::new(start);
    vm.load_program(&program).unwrap();
    // r2 base aligned; add 1 offset makes misalignment
    vm.registers.set(2, ivm::Memory::HEAP_START);
    let res = vm.run();
    assert!(matches!(res, Err(VMError::MisalignedAccess { .. })));
    assert_eq!(start - vm.gas_remaining, cost_of(ld));
}
#[test]
fn alignment_error_misaligned_offsets() {
    // STORE64 with offset 1 should misalign and still deduct cost
    {
        let store = wide_store(wide::memory::STORE64, 2, 3, 1);
        let mut code = Vec::new();
        push32(&mut code, store);
        halt32(&mut code);
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        let program = common::assemble(&code);
        vm.load_program(&program).unwrap();
        vm.registers.set(2, ivm::Memory::HEAP_START);
        vm.registers.set(3, 0xABCD_EF01_2345_6789);
        let res = vm.run();
        assert!(matches!(res, Err(VMError::MisalignedAccess { .. })));
        assert_eq!(start - vm.gas_remaining, cost_of(store));
    }
    // LOAD64 with offset 2 should misalign and charge cost
    {
        let load = wide_load(wide::memory::LOAD64, 1, 2, 2);
        let mut code = Vec::new();
        push32(&mut code, load);
        halt32(&mut code);
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        let program = common::assemble(&code);
        vm.load_program(&program).unwrap();
        vm.registers.set(2, ivm::Memory::HEAP_START);
        let res = vm.run();
        assert!(matches!(res, Err(VMError::MisalignedAccess { .. })));
        assert_eq!(start - vm.gas_remaining, cost_of(load));
    }
}
#[test]
fn alignment_error_store_doubleword() {
    // SD misaligned: funct3=0x3, offset=4
    let mut code = Vec::new();
    push32(&mut code, wide_store(wide::memory::STORE64, 2, 3, 4));
    halt32(&mut code);
    let start = 10_000u64;
    let mut vm = IVM::new(start);
    let program = common::assemble(&code);
    vm.load_program(&program).unwrap();
    vm.registers.set(2, ivm::Memory::HEAP_START);
    vm.registers.set(3, 0xDEAD_BEEF_F00D_BAAFu64);
    let res = vm.run();
    assert!(matches!(res, Err(VMError::MisalignedAccess { .. })));
    let store = wide_store(wide::memory::STORE64, 2, 3, 4);
    assert_eq!(start - vm.gas_remaining, cost_of(store));
}
#[test]
fn aligned_load_store_ok_gas() {
    // STORE64 followed by LOAD64 at aligned base should succeed.
    let store = wide_store(wide::memory::STORE64, 2, 3, 0);
    let load = wide_load(wide::memory::LOAD64, 4, 2, 0);
    let mut code = Vec::new();
    push32(&mut code, store);
    push32(&mut code, load);
    halt32(&mut code);
    let start = 10_000u64;
    let mut vm = IVM::new(start);
    let program = common::assemble(&code);
    vm.load_program(&program).unwrap();
    let base = ivm::Memory::HEAP_START + 0x80;
    vm.registers.set(2, base);
    vm.registers.set(3, 0x0000_0000_0000_0080u64);
    vm.run().unwrap();
    let expected = cost_of(store) + cost_of(load);
    assert_eq!(start - vm.gas_remaining, expected);
    assert_eq!(vm.registers.get(4), 0x0000_0000_0000_0080u64);
}
#[test]
fn jal_and_jalr_offsets_gas() {
    // JAL forward by 4 bytes (skip filler); schedule gas = 2
    let jal = wide_jump(wide::control::JAL, 1, 1);
    assert_eq!(cost_of(jal), 2);
    // JALR with imm offsets 0 and 0x20; gas 2
    for &imm in &[0i32, 0x20] {
        let jalr = wide_ri(wide::control::JALR, 1, 2, imm as i16);
        assert_eq!(cost_of(jalr), 2);
    }
}
#[test]
fn executed_set_all_branch_variants() {
    // For each branch funct3 variant, test both taken and not taken
    let cases = [
        (wide::control::BEQ, 1u64, 1u64, true), // BEQ taken when equal
        (wide::control::BEQ, 1u64, 2u64, false), // BEQ not taken
        (wide::control::BNE, 1u64, 2u64, true), // BNE taken when not equal
        (wide::control::BNE, 1u64, 1u64, false), // BNE not taken
        (wide::control::BLT, (-1i64) as u64, 1u64, true), // BLT signed: -1 < 1
        (wide::control::BLT, 1u64, 1u64, false), // BLT not taken when equal
        (wide::control::BGE, (-1i64) as u64, (-2i64) as u64, true), // BGE signed: -1 >= -2
        (wide::control::BGE, (-2i64) as u64, (-1i64) as u64, false), // BGE not taken
        (wide::control::BLTU, 1u64, 2u64, true), // BLTU unsigned: 1 < 2
        (wide::control::BLTU, 2u64, 1u64, false), // BLTU not taken
        (wide::control::BGEU, 2u64, 1u64, true), // BGEU unsigned: 2 >= 1
        (wide::control::BGEU, 1u64, 2u64, false), // BGEU not taken
    ];
    for &(op, v1, v2, taken) in &cases {
        let mut code = Vec::new();
        // Branch skips over filler when taken.
        // With PC+len base, imm=+4 skips one 32-bit instruction.
        push32(&mut code, wide_branch(op, 1, 2, 2));
        // filler: XOR r10=r10^r10
        push32(&mut code, wide_rr(wide::arithmetic::XOR, 10, 10, 10));
        halt32(&mut code);
        let words: Vec<u32> = code
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let executed: &[usize] = if taken { &[0, 2] } else { &[0, 1, 2] };
        let expected: u64 = executed.iter().map(|&i| cost_of(words[i])).sum();
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        let program = common::assemble(&code);
        vm.load_program(&program).unwrap();
        vm.registers.set(1, v1);
        vm.registers.set(2, v2);
        vm.run().unwrap();
        assert_eq!(start - vm.gas_remaining, expected);
    }
}
#[test]
fn mixed_16_32_sequence_cumulative_gas() {
    // Mixed-width sequences are no longer supported: decoder rejects 16-bit words.
    let mut code = Vec::new();
    let add16 = classic_encode_r16(0x1, 1, 2, 3);
    code.extend_from_slice(&add16.to_le_bytes());
    push32(&mut code, wide_rr(wide::arithmetic::ADD, 4, 4, 4));
    push32(&mut code, ivm::encoding::wide::encode_halt());
    let start = 10_000u64;
    let mut vm = IVM::new(start);
    let program = common::assemble(&code);
    let load_res = vm.load_program(&program);
    match load_res {
        Err(VMError::MemoryAccessViolation { .. }) => {}
        Err(other) => panic!("expected loader to reject mixed-width stream, got {other:?}"),
        Ok(()) => {
            let res = vm.run();
            assert!(
                matches!(res, Err(VMError::MemoryAccessViolation { .. })),
                "expected mixed stream to be rejected at execution, got {res:?}"
            );
        }
    }
}
#[test]
fn nested_branches_executed_set_property() {
    // Layout:
    // 0: BEQ r1,r2,+4  -> to 8 when taken (PC+len base)
    // 4: XOR r10=r10^r10 (filler A)
    // 8: BNE r3,r4,+0  -> to 12 when taken (PC+len base)
    // 12: HALT
    let mut code = Vec::new();
    // BEQ r1,r2,+8; target = PC + (2*4) = 8
    push32(&mut code, wide_branch(wide::control::BEQ, 1, 2, 2));
    // XOR filler A
    push32(&mut code, wide_rr(wide::arithmetic::XOR, 10, 10, 10));
    // BNE r3,r4,+4; target = PC + (1*4) = 12
    push32(&mut code, wide_branch(wide::control::BNE, 3, 4, 1));
    // HALT
    halt32(&mut code);
    let words: Vec<u32> = code
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let program = common::assemble(&code);
    // Case A: BEQ taken (r1==r2), BNE executed then HALT
    {
        let executed = [0usize, 2usize, 3usize];
        let expected: u64 = executed.iter().map(|&i| cost_of(words[i])).sum();
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        vm.load_program(&program).unwrap();
        vm.registers.set(1, 5);
        vm.registers.set(2, 5);
        vm.registers.set(3, 7);
        vm.registers.set(4, 9);
        vm.run().unwrap();
        assert_eq!(start - vm.gas_remaining, expected);
    }
    // Case B: BEQ not taken (r1!=r2), executes XOR, BNE, HALT
    {
        let executed = [0usize, 1usize, 2usize, 3usize];
        let expected: u64 = executed.iter().map(|&i| cost_of(words[i])).sum();
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        vm.load_program(&program).unwrap();
        vm.registers.set(1, 1);
        vm.registers.set(2, 2);
        vm.registers.set(3, 1);
        vm.registers.set(4, 1);
        vm.run().unwrap();
        assert_eq!(start - vm.gas_remaining, expected);
    }
}
#[test]
fn back_to_back_branches_executed_set() {
    // Program layout (all 32-bit):
    // 0: BEQ r1,r2,+8  (skip over next when taken; current-PC relative)
    // 4: BNE r3,r4,+8  (skip over filler when taken; current-PC relative)
    // 8: XOR r10=r10^r10 (filler)
    // 12: HALT
    let mut code = Vec::new();
    // BEQ r1,r2,+8
    push32(&mut code, wide_branch(wide::control::BEQ, 1, 2, 2));
    // BNE r3,r4,+8
    push32(&mut code, wide_branch(wide::control::BNE, 3, 4, 2));
    // XOR filler
    push32(&mut code, wide_rr(wide::arithmetic::XOR, 10, 10, 10));
    // HALT
    halt32(&mut code);
    let words: Vec<u32> = code
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let program = common::assemble(&code);
    // Case 1: BEQ taken (r1==r2), then BNE executes and is taken (r3!=r4): executed {0,1,3}
    {
        let executed = [0usize, 1usize, 3usize];
        let expected: u64 = executed.iter().map(|&i| cost_of(words[i])).sum();
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        vm.load_program(&program).unwrap();
        vm.registers.set(1, 42);
        vm.registers.set(2, 42);
        vm.registers.set(3, 1);
        vm.registers.set(4, 2);
        vm.run().unwrap();
        assert_eq!(start - vm.gas_remaining, expected);
    }
    // Case 2: BEQ not taken (r1!=r2), BNE not taken (r3==r4): executed {0,1,2,3}
    {
        let executed = [0usize, 1usize, 2usize, 3usize];
        let expected: u64 = executed.iter().map(|&i| cost_of(words[i])).sum();
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        vm.load_program(&program).unwrap();
        vm.registers.set(1, 0);
        vm.registers.set(2, 1);
        vm.registers.set(3, 7);
        vm.registers.set(4, 7);
        vm.run().unwrap();
        assert_eq!(start - vm.gas_remaining, expected);
    }
}
#[test]
fn load_store_pair_executed_set() {
    use ivm::ProgramMetadata;
    // Program: LOAD64 r5 <- [r1+0]; STORE64 [r1+8] <- r5; HALT
    let mut program = ProgramMetadata::default().encode();
    let load = ivm::encoding::wide::encode_load(wide::memory::LOAD64, 5, 1, 0);
    let store = ivm::encoding::wide::encode_store(wide::memory::STORE64, 1, 5, 8);
    program.extend_from_slice(&load.to_le_bytes());
    program.extend_from_slice(&store.to_le_bytes());
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let words: Vec<u32> = program[ProgramMetadata::default().encode().len()..]
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let expected: u64 = words.iter().map(|&w| cost_of(w)).sum();
    let start = 10_000u64;
    let mut vm = IVM::new(start);
    vm.load_program(&program).unwrap();
    let base = ivm::Memory::HEAP_START + 0xC00;
    vm.registers.set(1, base);
    vm.memory.store_u64(base, 0x0102_0304_0506_0708u64).unwrap();
    vm.run().unwrap();
    assert_eq!(start - vm.gas_remaining, expected);
    let stored = vm.memory.load_u64(base + 8).expect("store should succeed");
    assert_eq!(stored, 0x0102_0304_0506_0708u64);
}
#[test]
fn executed_set_branch_taken_and_not_taken() {
    // Helper encoders
    // Case 1: taken branch skips filler and lands on HALT
    {
        let mut code = Vec::new();
        // BEQ r1,r2,+8 → taken jumps two words ahead (skip filler)
        push32(&mut code, wide_branch(wide::control::BEQ, 1, 2, 2));
        // filler: XOR r10=r10^r10 (should be skipped)
        push32(&mut code, wide_rr(wide::arithmetic::XOR, 10, 10, 10));
        halt32(&mut code);
        let words: Vec<u32> = code
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        // Executed indices: 0 (branch), 2 (halt)
        let expected: u64 = [0usize, 2usize].iter().map(|&i| cost_of(words[i])).sum();
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        let program = common::assemble(&code);
        vm.load_program(&program).unwrap();
        vm.registers.set(1, 7);
        vm.registers.set(2, 7);
        vm.run().unwrap();
        assert_eq!(start - vm.gas_remaining, expected);
    }
    // Case 2: not-taken branch executes filler then HALT
    {
        let mut code = Vec::new();
        // BEQ r1,r2,+8 → not taken, so filler executes before HALT
        push32(&mut code, wide_branch(wide::control::BEQ, 1, 2, 2));
        // filler: XOR r10=r10^r10
        push32(&mut code, wide_rr(wide::arithmetic::XOR, 10, 10, 10));
        halt32(&mut code);
        let words: Vec<u32> = code
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        // Executed indices: 0 (branch), 1 (xor), 2 (halt)
        let expected: u64 = [0usize, 1usize, 2usize]
            .iter()
            .map(|&i| cost_of(words[i]))
            .sum();
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        let program = common::assemble(&code);
        vm.load_program(&program).unwrap();
        vm.registers.set(1, 1);
        vm.registers.set(2, 2);
        vm.run().unwrap();
        assert_eq!(start - vm.gas_remaining, expected);
    }
}
#[test]
fn executed_set_jal_and_jalr_alignment() {
    // JAL to skip filler to HALT: executed = {JAL, HALT}
    {
        let mut code = Vec::new();
        // JAL rd=1, imm=+8 (architectural target at HALT, skipping filler)
        let jal = wide_jump(wide::control::JAL, 1, 2);
        push32(&mut code, jal);
        // filler that should be skipped
        push32(
            &mut code,
            wide_rr(wide::arithmetic::XOR, 10, 10, 10), // XOR r10,r10,r10
        );
        halt32(&mut code);
        let words: Vec<u32> = code
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let base: u64 = cost_of(words[0]) + cost_of(words[2]);
        let penalty: u64 = cost_of(words[1]);
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        let program = common::assemble(&code);
        vm.load_program(&program).unwrap();
        vm.run().unwrap();
        let actual = start - vm.gas_remaining;
        assert!(
            actual == base || actual == base + penalty,
            "unexpected gas for JAL: got {actual}, expected {base} or {}",
            base + penalty
        );
    }
    // JALR with odd target aligns to even: target=5 becomes 4; executed = {JALR, HALT at 4}
    {
        let mut code = Vec::new();
        // JALR rd=1, rs1=2, imm=0 (opcode 0x67)
        push32(&mut code, wide_ri(wide::control::JALR, 1, 2, 0));
        // HALT at address 4
        halt32(&mut code);
        // Unreached filler at 8
        push32(&mut code, wide_rr(wide::arithmetic::XOR, 10, 10, 10));
        let words: Vec<u32> = code
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let base: u64 = cost_of(words[0]) + cost_of(words[1]);
        let penalty: u64 = cost_of(words[2]);
        let start = 10_000u64;
        let mut vm = IVM::new(start);
        let program = common::assemble(&code);
        vm.load_program(&program).unwrap();
        // Set r2 to odd address 5 so alignment mask applies → 4
        vm.registers.set(2, 5);
        vm.run().unwrap();
        let actual = start - vm.gas_remaining;
        assert!(
            actual == base || actual == base + penalty,
            "unexpected gas for JALR: got {actual}, expected {base} or {}",
            base + penalty
        );
    }
}
