//! Demonstrates mint and burn operations using IVM host syscalls.
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ivm::{IVM, VMError, encoding, host::IVMHost, instruction, kotodama::wide as kwide, syscalls};

#[derive(Clone)]
struct AssetHost {
    balances: Arc<Mutex<HashMap<u64, u64>>>,
    supply: Arc<Mutex<u64>>,
}

impl AssetHost {
    fn new() -> Self {
        AssetHost {
            balances: Arc::new(Mutex::new(HashMap::new())),
            supply: Arc::new(Mutex::new(0)),
        }
    }

    fn balance(&self, id: u64) -> u64 {
        let balances = self.balances.lock().expect("balance mutex poisoned");
        *balances.get(&id).unwrap_or(&0)
    }

    fn supply(&self) -> u64 {
        *self.supply.lock().expect("supply mutex poisoned")
    }
}

impl IVMHost for AssetHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        let mut balances = self.balances.lock().expect("balance mutex poisoned");
        let mut supply = self.supply.lock().expect("supply mutex poisoned");
        match number {
            syscalls::SYSCALL_MINT_ASSET => {
                let acct = vm.register(10);
                let amt = vm.register(11);
                *balances.entry(acct).or_default() += amt;
                *supply += amt;
                Ok(0)
            }
            syscalls::SYSCALL_BURN_ASSET => {
                let acct = vm.register(10);
                let amt = vm.register(11);
                let bal = balances.entry(acct).or_default();
                if *bal < amt {
                    return Err(VMError::UnknownSyscall(number));
                }
                *bal -= amt;
                *supply -= amt;
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
    // if current_supply >= cap -> burn instead of mint (skip next two instructions)
    let branch = kwide::encode_branch_checked(instruction::wide::control::BGEU, 12, 13, 2)
        .expect("branch offset");
    prog.extend_from_slice(&branch.to_le_bytes());
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_MINT_ASSET as u8,
        )
        .to_le_bytes(),
    );
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_BURN_ASSET as u8,
        )
        .to_le_bytes(),
    );
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    prog
}

fn main() {
    let host = AssetHost::new();
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host.clone());
    vm.set_register(10, 1); // account id
    vm.set_register(11, 50); // amount
    vm.set_register(12, host.supply()); // current supply
    vm.set_register(13, 100); // cap
    let prog = build_program();
    vm.load_program(&prog).unwrap();
    vm.run().expect("VM execution failed");
    println!(
        "Balance after op: {}  Total supply: {}",
        host.balance(1),
        host.supply()
    );
}
