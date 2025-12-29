use ivm::{IVM, encoding, instruction};

fn push_word(code: &mut Vec<u8>, word: u32) {
    code.extend_from_slice(&word.to_le_bytes());
}

fn halt(code: &mut Vec<u8>) {
    push_word(code, encoding::wide::encode_halt());
}

fn rr(op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
    encoding::wide::encode_rr(op, rd, rs1, rs2)
}

fn ri(op: u8, rd: u8, rs1: u8, imm: i8) -> u32 {
    encoding::wide::encode_ri(op, rd, rs1, imm)
}

fn load64(rd: u8, base: u8, imm: i8) -> u32 {
    encoding::wide::encode_load(instruction::wide::memory::LOAD64, rd, base, imm)
}

fn store64(base: u8, rs: u8, imm: i8) -> u32 {
    encoding::wide::encode_store(instruction::wide::memory::STORE64, base, rs, imm)
}

#[test]
fn rr_and_or_xor_shifts() {
    let mut code = Vec::new();
    push_word(&mut code, rr(instruction::wide::arithmetic::AND, 1, 2, 3));
    push_word(&mut code, rr(instruction::wide::arithmetic::OR, 4, 1, 5));
    push_word(&mut code, rr(instruction::wide::arithmetic::SLL, 6, 4, 7));
    push_word(&mut code, rr(instruction::wide::arithmetic::SRL, 8, 6, 9));
    push_word(&mut code, rr(instruction::wide::arithmetic::SRA, 10, 8, 11));
    halt(&mut code);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0xFF00_FF00_FF00_FF00);
    vm.registers.set(3, 0x0F0F_0F0F_0F0F_0F0F);
    vm.registers.set(5, 0x00FF_00FF_00FF_00FF);
    vm.registers.set(7, 4);
    vm.registers.set(9, 8);
    vm.registers.set(11, 8);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), 0x0F00_0F00_0F00_0F00);
    assert_eq!(vm.registers.get(4), 0x0FFF_0FFF_0FFF_0FFF);
    assert_eq!(vm.registers.get(6), 0xFFF0_FFF0_FFF0_FFF0);
    assert_eq!(vm.registers.get(8), 0x00FF_F0FF_F0FF_F0FF);
    assert_eq!(vm.registers.get(10), 0x0000_FFF0_FFF0_FFF0);
}

#[test]
fn shift_operands_are_masked() {
    let mut code = Vec::new();
    push_word(&mut code, rr(instruction::wide::arithmetic::SRA, 1, 2, 3));
    halt(&mut code);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0xF000_0000_0000_0000u64);
    vm.registers.set(3, 70); // masked to 6
    vm.run().unwrap();

    let expected = ((0xF000_0000_0000_0000u64 as i64) >> 6) as u64;
    assert_eq!(vm.registers.get(1), expected);
}

#[test]
fn immediate_ops_cover_small_constants() {
    let mut code = Vec::new();
    push_word(&mut code, ri(instruction::wide::arithmetic::ADDI, 1, 2, -5));
    push_word(
        &mut code,
        ri(instruction::wide::arithmetic::ORI, 3, 1, 0x3C),
    );
    push_word(
        &mut code,
        ri(instruction::wide::arithmetic::ANDI, 4, 3, 0x7F),
    );
    push_word(
        &mut code,
        ri(instruction::wide::arithmetic::XORI, 5, 4, 0x30),
    );
    push_word(
        &mut code,
        ri(instruction::wide::arithmetic::ROTL_IMM, 6, 5, 4),
    );
    push_word(
        &mut code,
        ri(instruction::wide::arithmetic::ROTR_IMM, 7, 6, 8),
    );
    halt(&mut code);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 100);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), 95);
    assert_eq!(vm.registers.get(3), 95 | 0x3C);
    assert_eq!(vm.registers.get(4), (95 | 0x3C) & 0x7F);
    assert_eq!(vm.registers.get(5), ((95 | 0x3C) & 0x7F) ^ 0x30);
    assert_eq!(vm.registers.get(6), vm.registers.get(5).rotate_left(4));
    assert_eq!(vm.registers.get(7), vm.registers.get(6).rotate_right(8));
}

#[test]
fn load_store_64_roundtrip() {
    let mut code = Vec::new();
    push_word(&mut code, store64(2, 3, 8));
    push_word(&mut code, load64(1, 2, 8));
    halt(&mut code);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    let base = ivm::Memory::HEAP_START;
    vm.registers.set(2, base);
    vm.registers.set(3, 0xDEAD_BEEF_F00D_BAAF);
    vm.run().unwrap();
    assert_eq!(vm.registers.get(1), 0xDEAD_BEEF_F00D_BAAF);
}

#[test]
fn branch_offset_counts_words() {
    let mut code = Vec::new();
    // If r2 == r2, skip the next add.
    push_word(
        &mut code,
        encoding::wide::encode_branch(instruction::wide::control::BEQ, 2, 2, 2),
    );
    // Would double r5 if executed.
    push_word(&mut code, rr(instruction::wide::arithmetic::ADD, 5, 5, 5));
    // Always doubles r6.
    push_word(&mut code, rr(instruction::wide::arithmetic::ADD, 6, 6, 6));
    halt(&mut code);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 1);
    vm.registers.set(5, 1);
    vm.registers.set(6, 1);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(5), 1, "branch should skip doubling r5");
    assert_eq!(vm.registers.get(6), 2, "r6 should be doubled");
}

#[test]
fn jalr_masks_low_bit_and_records_return_address() {
    let mut code = Vec::new();
    push_word(
        &mut code,
        encoding::wide::encode_ri(instruction::wide::control::JALR, 1, 2, 0),
    );
    push_word(
        &mut code,
        rr(instruction::wide::arithmetic::XOR, 10, 10, 10),
    );
    halt(&mut code);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    // Target address with low bit set should be masked to the XOR word (address 4).
    vm.registers.set(2, 5);
    vm.registers.set(10, 0xABCD);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(10), 0);
    let return_pc = vm.registers.get(1);
    assert_eq!(
        return_pc, 4,
        "return PC should point to instruction after JALR"
    );
}
