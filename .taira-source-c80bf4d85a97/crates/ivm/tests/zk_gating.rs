//! Tests for header feature gating of ZK instructions.

use ivm::{IVM, ProgramMetadata, VMError, encoding, instruction};

fn build_prog_with_assert(zk_mode: bool) -> Vec<u8> {
    let mut code = Vec::new();
    let word = encoding::wide::encode_rr(instruction::wide::zk::ASSERT, 0, 1, 0);
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut meta = ProgramMetadata::default();
    if zk_mode {
        meta.mode |= ivm::ivm_mode::ZK;
        meta.max_cycles = 2;
    }
    meta.abi_version = 1;
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

#[test]
fn zk_ops_rejected_without_header_bit() {
    let prog = build_prog_with_assert(false);
    let mut vm = IVM::new(10_000);
    vm.set_register(1, 0);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::ZkExtensionDisabled));
}

#[test]
fn zk_ops_allowed_with_header_bit() {
    let prog = build_prog_with_assert(true);
    let mut vm = IVM::new(10_000);
    vm.set_register(1, 0);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
}
