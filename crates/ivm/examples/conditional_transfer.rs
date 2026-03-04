//! Demonstrates a conditional asset transfer using IVM host syscalls.
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ivm::{IVM, VMError, encoding, host::IVMHost, instruction, kotodama::wide as kwide, syscalls};

#[derive(Clone)]
struct AssetHost {
    state: Arc<Mutex<HashMap<u64, u64>>>,
}

impl AssetHost {
    fn new(initial_balances: &[(u64, u64)]) -> Self {
        let mut map = HashMap::new();
        for (id, bal) in initial_balances {
            map.insert(*id, *bal);
        }
        AssetHost {
            state: Arc::new(Mutex::new(map)),
        }
    }

    fn balance(&self, id: u64) -> u64 {
        let state = self.state.lock().expect("state mutex poisoned");
        *state.get(&id).unwrap_or(&0)
    }
}

impl IVMHost for AssetHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        let mut state = self.state.lock().expect("state mutex poisoned");
        match number {
            syscalls::SYSCALL_TRANSFER_ASSET => {
                let from = vm.register(10);
                let to = vm.register(11);
                let amount = vm.register(12);
                let from_bal = state.entry(from).or_default();
                if *from_bal < amount {
                    return Err(VMError::UnknownSyscall(number));
                }
                *from_bal -= amount;
                *state.entry(to).or_default() += amount;
                Ok(0)
            }
            syscalls::SYSCALL_ABORT => {
                println!("Transaction aborted");
                Ok(0)
            }
            _ => Err(VMError::UnknownSyscall(number)),
        }
    }

    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

fn build_program() -> Vec<u8> {
    let mut prog = ivm::ProgramMetadata::default().encode();
    // if x13 < x12 jump to abort (skip next two instructions)
    let branch = kwide::encode_branch_checked(instruction::wide::control::BLTU, 13, 12, 2)
        .expect("branch offset");
    prog.extend_from_slice(&branch.to_le_bytes());
    // perform transfer
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_TRANSFER_ASSET as u8,
        )
        .to_le_bytes(),
    );
    // halt after success
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    // abort branch target
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_ABORT as u8,
        )
        .to_le_bytes(),
    );
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    prog
}

fn main() {
    let host = AssetHost::new(&[(1, 100), (2, 10)]);
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host.clone());
    vm.set_register(10, 1); // sender
    vm.set_register(11, 2); // receiver
    vm.set_register(12, 30); // amount
    vm.set_register(13, host.balance(1));
    let prog = build_program();
    vm.load_program(&prog).unwrap();
    vm.run().expect("VM execution failed");
    println!(
        "Balances after transfer: acc1={}, acc2={}",
        host.balance(1),
        host.balance(2)
    );
}
