use ivm::{IVM, encoding, instruction};

const HALT_WORD: u32 = encoding::wide::encode_halt();

fn assemble(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for &word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

#[test]
fn srai_masks_excess_shamt() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0xF000_0000_0000_0000u64);
    vm.registers.set(4, 70);
    vm.run().unwrap();

    let expected = ((0xF000_0000_0000_0000u64 as i64) >> 6) as u64;
    assert_eq!(vm.registers.get(1), expected);
}

#[test]
fn slli_shamt_0_no_change() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0x0123_4567_89AB_CDEFu64);
    vm.registers.set(4, 0);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), vm.registers.get(2));
}

#[test]
fn srli_shamt_63_boundary() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0x8000_0000_0000_0001u64);
    vm.registers.set(4, 63);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), 1);
}

#[test]
fn srai_shamt_63_sign_fill() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0x8000_0000_0000_0001u64);
    vm.registers.set(4, 63);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), u64::MAX);
}

#[test]
fn srli_masks_64_to_zero_shift() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0xDEAD_BEEF_F00D_BAAFu64);
    vm.registers.set(4, 64);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), 0xDEAD_BEEF_F00D_BAAF);
}

#[test]
fn slli_masks_excess_shamt() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0x0123_4567_89AB_CDEFu64);
    vm.registers.set(4, 66);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), 0x0123_4567_89AB_CDEFu64 << 2);
}

#[test]
fn srli_shamt_1_logical() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0x8000_0000_0000_0001u64);
    vm.registers.set(4, 1);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), 0x4000_0000_0000_0000u64);
}

#[test]
fn srai_shamt_1_sign() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0x8000_0000_0000_0001u64);
    vm.registers.set(4, 1);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), 0xC000_0000_0000_0000u64);
}

#[test]
fn srli_shamt_0_no_change() {
    let code = assemble(&[
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 1, 2, 4),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);
    vm.registers.set(2, 0xDEAD_BEEF_F00D_BAAFu64);
    vm.registers.set(4, 0);
    vm.run().unwrap();

    assert_eq!(vm.registers.get(1), vm.registers.get(2));
}

#[test]
fn slli_boundaries_positive_and_negative() {
    let code = assemble(&[
        // Positive source shifts.
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 10, 2, 4),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 11, 2, 5),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 12, 2, 6),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 13, 2, 7),
        // Negative source shifts.
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 14, 3, 4),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 15, 3, 5),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 16, 3, 6),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SLL, 17, 3, 7),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);

    let pos = 0x0123_4567_89AB_CDEFu64;
    let neg = 0xF123_4567_89AB_CDEFu64;
    vm.registers.set(2, pos);
    vm.registers.set(3, neg);
    vm.registers.set(4, 0);
    vm.registers.set(5, 1);
    vm.registers.set(6, 63);
    vm.registers.set(7, 64);

    vm.run().unwrap();

    assert_eq!(vm.registers.get(10), pos);
    assert_eq!(vm.registers.get(11), pos << 1);
    assert_eq!(vm.registers.get(12), pos << 63);
    assert_eq!(vm.registers.get(13), pos);
    assert_eq!(vm.registers.get(14), neg);
    assert_eq!(vm.registers.get(15), neg << 1);
    assert_eq!(vm.registers.get(16), neg << 63);
    assert_eq!(vm.registers.get(17), neg);
}

#[test]
fn srli_srai_boundaries_positive_and_negative() {
    let code = assemble(&[
        // SRL positive.
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 10, 2, 4),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 11, 2, 5),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 12, 2, 6),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 13, 2, 7),
        // SRA positive.
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 14, 2, 4),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 15, 2, 5),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 16, 2, 6),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 17, 2, 7),
        // SRL negative.
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 18, 3, 4),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 19, 3, 5),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 20, 3, 6),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRL, 21, 3, 7),
        // SRA negative.
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 22, 3, 4),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 23, 3, 5),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 24, 3, 6),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SRA, 25, 3, 7),
        HALT_WORD,
    ]);

    let mut vm = IVM::new(10_000);
    vm.memory.load_code(&code);

    let pos = 0x0123_4567_89AB_CDEFu64;
    let neg = 0xF123_4567_89AB_CDEFu64;
    vm.registers.set(2, pos);
    vm.registers.set(3, neg);
    vm.registers.set(4, 0);
    vm.registers.set(5, 1);
    vm.registers.set(6, 63);
    vm.registers.set(7, 64);

    vm.run().unwrap();

    assert_eq!(vm.registers.get(10), pos);
    assert_eq!(vm.registers.get(11), pos >> 1);
    assert_eq!(vm.registers.get(12), pos >> 63);
    assert_eq!(vm.registers.get(13), pos);
    assert_eq!(vm.registers.get(14), pos);
    assert_eq!(vm.registers.get(15), pos >> 1);
    assert_eq!(vm.registers.get(16), pos >> 63);
    assert_eq!(vm.registers.get(17), pos);

    assert_eq!(vm.registers.get(18), neg);
    assert_eq!(vm.registers.get(19), neg >> 1);
    assert_eq!(vm.registers.get(20), neg >> 63);
    assert_eq!(vm.registers.get(21), neg);

    assert_eq!(vm.registers.get(22), neg);
    assert_eq!(vm.registers.get(23), ((neg as i64) >> 1) as u64);
    assert_eq!(vm.registers.get(24), u64::MAX);
    assert_eq!(vm.registers.get(25), neg);
}
