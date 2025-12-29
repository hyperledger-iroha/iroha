use ivm::{IVM, encoding};
use sha2::{Digest, Sha256};
mod common;
use common::assemble;

#[test]
fn test_program_code_hash() {
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let prog = assemble(&halt);
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&prog).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(halt);
    let expected: [u8; 32] = hasher.finalize().into();
    assert_eq!(vm.code_hash(), expected);
    vm.run().unwrap();
    assert_eq!(vm.code_hash(), expected);
}
