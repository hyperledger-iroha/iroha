//! Explicit borrowed-host construction, original pool custody and ordinary host parity.

use super::*;
use crate::{
    encoding::wide,
    error::VmDeferral,
    host::{SyscallContext, SyscallQuote},
    instruction::wide as isa,
};

const LIMIT: usize = 512 * 1024 * 1024;

fn artifact() -> Vec<u8> {
    let mut bytes =
        ProgramMetadata::default().encode_for_profile(IvmProfileId::C10_GENERIC_CARRIER, 0);
    for word in [
        wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 64),
        wide::encode_scall(crate::syscalls::SYSCALL_ALLOC),
        wide::encode_halt(),
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

#[derive(Default)]
struct MemoryHost {
    calls: usize,
}

impl IVMHost for MemoryHost {
    fn prepare_syscall(&self, number: u32, vm: SyscallQuote<'_>) -> Result<u64, VMError> {
        crate::host::quote_carrier_memory_syscall(number, vm)?
            .ok_or(VMError::Trap(crate::error::VmTrap::UnknownSyscall(number)))
    }

    fn syscall(&mut self, number: u32, vm: &mut dyn SyscallContext) -> Result<u64, VMError> {
        self.calls += 1;
        crate::host::execute_carrier_memory_syscall(number, vm)?
            .ok_or(VMError::Trap(crate::error::VmTrap::UnknownSyscall(number)))
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[test]
fn explicit_host_runs_same_engine_and_gas_without_installing_an_owned_host() {
    let budget = AllocationBudget::new(LIMIT);
    let mut external = IVM::try_new_for_external_host_with_memory_budget(10_000, &budget)
        .expect("original construction credit");
    let mut ordinary = IVM::try_new_with_memory_budget(10_000, &budget).unwrap();
    let original_host = ordinary
        .host_mut_any()
        .unwrap()
        .downcast_mut::<DefaultHost>()
        .unwrap() as *mut DefaultHost;
    for vm in [&mut external, &mut ordinary] {
        vm.load_program_for_profile(&artifact(), IvmProfileId::C10_GENERIC_CARRIER)
            .unwrap();
    }
    assert!(external.host_mut_any().is_none());
    assert!(external.allocation_budget().unwrap().same_pool(&budget));
    assert!(external.prepared.as_ref().unwrap().belongs_to(&budget));
    let before = (
        external.pc,
        external.remaining_gas(),
        budget.reserved_bytes(),
    );
    assert!(matches!(
        external.run(),
        Err(VMError::Deferred(VmDeferral::HostUnavailable))
    ));
    assert_eq!(
        (
            external.pc,
            external.remaining_gas(),
            budget.reserved_bytes()
        ),
        before
    );

    let mut host = MemoryHost::default();
    external.run_with_host(&mut host).unwrap();
    ordinary.run().unwrap();
    assert_eq!(host.calls, 1);
    assert_eq!(external.engine(), ordinary.engine());
    assert_eq!(external.register(10), ordinary.register(10));
    assert_eq!(external.remaining_gas(), ordinary.remaining_gas());
    assert!(external.host_mut_any().is_none());
    assert_eq!(
        ordinary
            .host_mut_any()
            .unwrap()
            .downcast_mut::<DefaultHost>()
            .unwrap() as *mut DefaultHost,
        original_host
    );
    assert!(matches!(
        external.run(),
        Err(VMError::Deferred(VmDeferral::HostUnavailable))
    ));
    drop((external, ordinary));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn external_construction_has_exact_original_fixed_credit_and_unwinds_refusal() {
    let probe = AllocationBudget::new(LIMIT);
    let vm = IVM::try_new_for_external_host_with_memory_budget(257, &probe).unwrap();
    let exact = probe.reserved_bytes();
    assert!(exact > 0);
    drop(vm);
    assert_eq!(probe.reserved_bytes(), 0);

    let short = AllocationBudget::new(exact - 1);
    assert!(matches!(
        IVM::try_new_for_external_host_with_memory_budget(257, &short),
        Err(VMError::Deferred(VmDeferral::AllocationDeferred(_)))
    ));
    assert_eq!(short.reserved_bytes(), 0);
    let budget = AllocationBudget::new(exact);
    let vm = IVM::try_new_for_external_host_with_memory_budget(257, &budget).unwrap();
    assert!(vm.allocation_budget().unwrap().same_pool(&budget));
    assert_eq!(budget.reserved_bytes(), exact);
    assert_eq!(vm.remaining_gas(), 257);
    drop(vm);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn ordinary_constructors_and_same_pool_reconstruction_keep_the_default_host() {
    let budget = AllocationBudget::new(LIMIT);
    let external = IVM::try_new_for_external_host_with_memory_budget(100, &budget).unwrap();
    for mut ordinary in [
        IVM::new(100),
        IVM::try_new(100).unwrap(),
        IVM::try_new_with_memory_budget(100, &budget).unwrap(),
        external.try_new_in_same_memory_pool(100).unwrap(),
    ] {
        assert!(
            ordinary
                .host_mut_any()
                .unwrap()
                .downcast_mut::<DefaultHost>()
                .is_some()
        );
        ordinary
            .load_code(&wide::encode_halt().to_le_bytes())
            .unwrap();
        ordinary.run().unwrap();
    }
    drop(external);
    assert_eq!(budget.reserved_bytes(), 0);
}
