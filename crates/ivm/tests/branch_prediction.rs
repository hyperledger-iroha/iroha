use ivm::{IVM, encoding, instruction};
mod common;
use common::assemble;

const HALT_WORD: u32 = encoding::wide::encode_halt();

fn assemble_words(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for &word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    assemble(&bytes)
}

#[test]
fn branch_prediction_accuracy_loop() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 0);
    vm.set_register(2, 5);
    let prog = assemble_words(&[
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 1, 1),
        encoding::wide::encode_branch(instruction::wide::control::BLT, 1, 2, -1),
        HALT_WORD,
    ]);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    assert!(vm.branch_prediction_accuracy() > 0.5);
}
