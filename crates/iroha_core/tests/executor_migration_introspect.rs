//! Helper test to dump the syscalls touched by executor fixtures.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use core::num::NonZeroU64;
use std::{fs, path::PathBuf};

use iroha_data_model::{block::BlockHeader, smart_contract::payloads::ExecutorContext};
use iroha_test_samples::ALICE_ID;
use ivm::{IVM, Memory, VMError, host::IVMHost};

struct LoggingHost;

impl IVMHost for LoggingHost {
    fn syscall(&mut self, number: u32, _vm: &mut IVM) -> Result<u64, VMError> {
        println!("syscall {number:#x}");
        Err(VMError::UnknownSyscall(number))
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self
    }
}

#[test]
fn print_syscalls() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("..");
    path.push("integration_tests/fixtures/ivm/executor_with_custom_permission.to");
    let bytes = fs::read(path).unwrap();
    let mut vm = IVM::new(0);
    vm.load_program(&bytes).unwrap();
    vm.set_host(LoggingHost);

    let context = ExecutorContext {
        authority: ALICE_ID.clone(),
        curr_block: BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0),
    };
    let payload = norito::to_bytes(&context).unwrap();
    let len_size = core::mem::size_of::<usize>();
    let mut bytes_with_len = Vec::with_capacity(len_size + payload.len());
    bytes_with_len.extend_from_slice(&(len_size + payload.len()).to_le_bytes());
    bytes_with_len.extend_from_slice(&payload);
    vm.store_bytes(Memory::HEAP_START, &bytes_with_len).unwrap();
    vm.set_register(10, Memory::HEAP_START);
    vm.set_gas_limit(50_000_000);
    let _ = vm.run();
}
