use ivm::{IVM, ProgramMetadata, cost_of as cost_of_opt, encoding, instruction};
const HALT_WORD: u32 = encoding::wide::encode_halt();
fn cost_of(word: u32) -> u64 {
    cost_of_opt(word).expect("valid opcode must have gas cost")
}
fn assemble_words(words: &[u32]) -> Vec<u8> {
    let mut bytes = header();
    for &word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}
fn header() -> Vec<u8> {
    ProgramMetadata::default().encode()
}
#[test]
fn nested_branches_executed_set_gas() {
    // Program:
    // addi x1, x0, 1
    // addi x2, x0, 1
    // beq x1,x2, +8   ; skip over next jmp if equal
    // jmp +4          ; else path
    // addi x3, x0, 7  ; then-path single op
    // halt
    let a1 = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 0, 1);
    let a2 = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 2, 0, 1);
    let beq = encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 2, 2);
    let jmp = encoding::wide::encode_offset24(instruction::wide::control::JMP, 2);
    let then = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 3, 0, 7);
    let halt = HALT_WORD;
    let bytes = assemble_words(&[a1, a2, beq, jmp, then, halt]);
    let mut vm = IVM::new(10_000);
    vm.load_program(&bytes).unwrap();
    let _ = vm.run();
    // Expected executed-set when equal: addi, addi, beq, then, halt
    let executed = [a1, a2, beq, then, halt];
    let expected: u64 = executed.iter().map(|w| cost_of(*w)).sum();
    assert_eq!(10_000 - vm.remaining_gas(), expected);
}
