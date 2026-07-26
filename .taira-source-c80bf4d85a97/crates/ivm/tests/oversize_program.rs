use ivm::{IVM, VMError};
mod common;
use common::assemble;

#[test]
fn test_program_too_large() {
    let mut vm = IVM::new(u64::MAX);
    let code = vec![0u8; (ivm::Memory::HEAP_START as usize) + 1];
    let prog = assemble(&code);
    let res = vm.load_program(&prog);
    assert!(matches!(res, Err(VMError::InvalidMetadata)));
}
