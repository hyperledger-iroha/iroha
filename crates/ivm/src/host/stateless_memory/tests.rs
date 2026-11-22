//! Shared native/carrier memory-call parity and retained original scratch refusal.

use super::*;
use crate::{IVM, IvmProfileId, ProgramMetadata, host::IVMHost, pointer_abi::PointerType};

fn program() -> Vec<u8> {
    let mut bytes =
        ProgramMetadata::default().encode_for_profile(IvmProfileId::C10_GENERIC_CARRIER, 0);
    bytes.extend_from_slice(&ivm_abi::encoding::wide::encode_halt().to_le_bytes());
    bytes
}

fn carrier(budget: &AllocationBudget) -> IVM {
    let mut vm = IVM::try_new_for_external_host_with_memory_budget(100_000, budget).unwrap();
    vm.load_program_for_profile(&program(), IvmProfileId::C10_GENERIC_CARRIER)
        .unwrap();
    vm
}

#[test]
fn shared_alloc_and_owned_input_publication_preserve_native_results_and_gas() {
    let budget = AllocationBudget::new(512 * 1024 * 1024);
    let mut native = IVM::new(100_000);
    let mut selected = carrier(&budget);
    let mut host = DefaultHost::new();
    native.set_register(10, 96);
    selected.set_register(10, 96);
    let native_gas = host.syscall(syscalls::SYSCALL_ALLOC, &mut native).unwrap();
    let selected_gas = execute_carrier_memory_syscall(syscalls::SYSCALL_ALLOC, &mut selected)
        .unwrap()
        .unwrap();
    assert_eq!(selected_gas, native_gas);
    assert_eq!(selected.register(10), native.register(10));
    let envelope =
        ivm_abi::pointer_abi::encode_tlv(PointerType::NoritoBytes, b"shared memory kernel")
            .unwrap();
    for vm in [&mut native, &mut selected] {
        vm.store_bytes(vm.register(10), &envelope).unwrap();
    }
    let before = budget.reserved_bytes();
    let pointer = selected.register(10);
    let quoted = quote_carrier_memory_syscall(
        syscalls::SYSCALL_INPUT_PUBLISH_TLV,
        selected.syscall_quote(),
    )
    .unwrap()
    .unwrap();
    let actual = execute_carrier_memory_syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut selected)
        .unwrap()
        .unwrap();
    let original = host
        .syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut native)
        .unwrap();
    assert_eq!((actual, quoted), (original, original));
    assert_eq!(selected.register(10), pointer);
    assert_eq!(
        budget.reserved_bytes(),
        before,
        "owned public heap TLVs need no copy"
    );
    selected.set_register(10, 0);
    assert_eq!(
        execute_carrier_memory_syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut selected).unwrap(),
        Some(DefaultHost::input_publish_gas(0))
    );
    assert_eq!(selected.register(10), 0);
}

#[test]
fn funded_tlv_copy_keeps_exact_original_pool_until_drop_and_refuses_short_credit() {
    let budget = AllocationBudget::new(512 * 1024 * 1024);
    let mut vm = carrier(&budget);
    let envelope =
        ivm_abi::pointer_abi::encode_tlv(PointerType::NoritoBytes, b"original copy owner").unwrap();
    let pointer = vm.alloc_input_tlv(&envelope).unwrap();
    let before = budget.reserved_bytes();
    let limit = budget.limit_bytes();
    budget.set_limit_bytes(before + envelope.len() - 1);
    let refusal = vm
        .clone_tlv_with_original_memory(pointer)
        .err()
        .expect("one missing byte must refuse");
    assert!(matches!(
        refusal,
        VMError::Deferred(VmDeferral::AllocationDeferred(_))
    ));
    assert_eq!(budget.reserved_bytes(), before);
    budget.set_limit_bytes(before + envelope.len());
    let retained = vm.clone_tlv_with_original_memory(pointer).unwrap();
    assert_eq!(retained.as_ref(), envelope.as_slice());
    assert_eq!(budget.reserved_bytes(), before + envelope.len());
    drop(retained);
    assert_eq!(budget.reserved_bytes(), before);
    budget.set_limit_bytes(limit);
}

#[test]
fn carrier_memory_helpers_reject_another_profile_and_missing_original_pool() {
    let mut vm = IVM::new(100_000);
    assert!(matches!(
        quote_carrier_memory_syscall(syscalls::SYSCALL_ALLOC, vm.syscall_quote()),
        Err(VMError::Trap(VmTrap::PermissionDenied))
    ));
    assert!(matches!(
        execute_carrier_memory_syscall(syscalls::SYSCALL_ALLOC, &mut vm),
        Err(VMError::Trap(VmTrap::PermissionDenied))
    ));
    let envelope =
        ivm_abi::pointer_abi::encode_tlv(PointerType::NoritoBytes, b"standalone").unwrap();
    let pointer = vm.alloc_input_tlv(&envelope).unwrap();
    assert!(matches!(
        vm.clone_tlv_with_original_memory(pointer),
        Err(VMError::Trap(VmTrap::PermissionDenied))
    ));
}

fn literal_program(profile: Option<IvmProfileId>, envelope: &[u8]) -> Vec<u8> {
    let mut bytes = profile.map_or_else(
        || ProgramMetadata::default().encode(),
        |profile| ProgramMetadata::default().encode_for_profile(profile, 0),
    );
    let padding = (4 - (24 + envelope.len()) % 4) % 4;
    bytes.extend_from_slice(&crate::metadata::LITERAL_SECTION_MAGIC);
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&(padding as u32).to_le_bytes());
    bytes.extend_from_slice(&(envelope.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&24_u64.to_le_bytes());
    bytes.extend_from_slice(envelope);
    bytes.extend(std::iter::repeat_n(0, padding));
    bytes.extend_from_slice(&ivm_abi::encoding::wide::encode_halt().to_le_bytes());
    bytes
}

#[test]
fn literal_publication_uses_shared_pointer_kernel_and_original_temporary_credit() {
    let envelope =
        ivm_abi::pointer_abi::encode_tlv(PointerType::NoritoBytes, b"literal publication").unwrap();
    let budget = AllocationBudget::new(512 * 1024 * 1024);
    let mut selected = IVM::try_new_for_external_host_with_memory_budget(100_000, &budget).unwrap();
    selected
        .load_program_for_profile(
            &literal_program(Some(IvmProfileId::C10_GENERIC_CARRIER), &envelope),
            IvmProfileId::C10_GENERIC_CARRIER,
        )
        .unwrap();
    let mut native = IVM::new(100_000);
    native
        .load_program(&literal_program(None, &envelope))
        .unwrap();
    let mut host = DefaultHost::new();
    selected.set_register(10, 24);
    native.set_register(10, 24);
    let before = budget.reserved_bytes();
    let limit = budget.limit_bytes();
    budget.set_limit_bytes(before + envelope.len() - 1);
    let refused =
        execute_carrier_memory_syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut selected);
    assert!(matches!(
        refused,
        Err(VMError::Deferred(VmDeferral::AllocationDeferred(_)))
    ));
    assert_eq!(
        selected.register(10),
        24,
        "refusal cannot publish a replacement pointer"
    );
    assert_eq!(budget.reserved_bytes(), before);
    budget.set_limit_bytes(limit);
    let quote = quote_carrier_memory_syscall(
        syscalls::SYSCALL_INPUT_PUBLISH_TLV,
        selected.syscall_quote(),
    )
    .unwrap()
    .unwrap();
    let actual = execute_carrier_memory_syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut selected)
        .unwrap()
        .unwrap();
    let original = host
        .syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut native)
        .unwrap();
    assert_eq!((actual, quote), (original, original));
    assert_ne!(selected.register(10), 24);
    assert_eq!(selected.clone_tlv(selected.register(10)).unwrap(), envelope);
    assert_eq!(native.clone_tlv(native.register(10)).unwrap(), envelope);
}
