use ivm::{IVM, encoding, instruction};

fn push32(code: &mut Vec<u8>, word: u32) {
    code.extend_from_slice(&word.to_le_bytes());
}

fn halt32(code: &mut Vec<u8>) {
    push32(code, encoding::wide::encode_halt());
}

#[test]
fn add_sub_xor_wide() {
    let mut code = Vec::new();
    // add r1 = r2 + r3
    push32(
        &mut code,
        encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 1, 2, 3),
    );
    // sub r4 = r1 - r5
    push32(
        &mut code,
        encoding::wide::encode_rr(instruction::wide::arithmetic::SUB, 4, 1, 5),
    );
    // xor r6 = r4 ^ r7
    push32(
        &mut code,
        encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 6, 4, 7),
    );
    halt32(&mut code);

    let mut vm = IVM::new(1_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 10);
    vm.registers.set(3, 20);
    vm.registers.set(5, 7);
    vm.registers.set(7, 0xFF);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), 30);
    assert_eq!(vm.registers.get(4), 23);
    assert_eq!(vm.registers.get(6), 23 ^ 0xFF);
}

#[test]
fn beq_wide_offset() {
    let mut code = Vec::new();
    // beq r1, r2, +2 (skip the next instruction when equal)
    push32(
        &mut code,
        encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 2, 2),
    );
    // xor r8 = r8 ^ r8 (will be skipped if branch taken)
    push32(
        &mut code,
        encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 8, 8, 8),
    );
    halt32(&mut code);

    let mut vm = IVM::new(1_000);
    vm.memory.load_code(&code);
    vm.registers.set(1, 42);
    vm.registers.set(2, 42);
    vm.registers.set(8, 0xAA);
    vm.run().unwrap();
    assert_eq!(vm.registers.get(8), 0xAA);
}
