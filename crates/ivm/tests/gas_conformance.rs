//! Gas conformance: runtime accounting charges exactly the executed instruction set.
use ivm::{IVM, ProgramMetadata, cost_of, encoding, instruction};
#[test]
fn branch_executed_set_gas_matches() {
    // addi x1, x0, 1; addi x2, x0, 2; beq x1,x2, +8; jal 0, +4; halt
    let a1 = kotodama_lang::compiler::encode_addi(1, 0, 1).expect("encode addi");
    let a2 = kotodama_lang::compiler::encode_addi(2, 0, 2).expect("encode addi");
    let beq = encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 2, 2);
    let jmp = encoding::wide::encode_offset24(instruction::wide::control::JMP, 1);
    let halt = encoding::wide::encode_halt();
    let words = [a1, a2, beq, jmp, halt];
    let mut code = ProgramMetadata::default().encode();
    for w in &words {
        code.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm = IVM::new(10_000);
    vm.load_program(&code).unwrap();
    vm.run().unwrap();
    // Executed-set: a1, a2, beq (not taken), jmp, halt
    let executed = [a1, a2, beq, jmp, halt];
    let expected: u64 = executed
        .iter()
        .map(|&w| cost_of(w).expect("valid opcode must have gas cost"))
        .sum();
    assert_eq!(10_000 - vm.remaining_gas(), expected);
}
