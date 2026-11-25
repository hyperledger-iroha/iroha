//! Shared ALLOC and INPUT publication kernels with an original-pool carrier copy owner.

use super::{DefaultHost, SyscallContext, SyscallQuote, allocation_gas};
use crate::{
    VMError,
    error::{VmDeferral, VmTrap},
    pointer_abi::Tlv,
    syscalls,
};
use iroha_allocation::{AllocationBudget, AllocationReservation, AllocatorRefusal};

/// An authenticated public TLV copy retaining its original VM pool until disposal.
/// No API extracts or grows its backing allocation.
pub struct FundedPublicTlv {
    bytes: Vec<u8>,
    _allocation: AllocationReservation,
}

impl FundedPublicTlv {
    pub(crate) fn from_validated(tlv: Tlv<'_>, budget: &AllocationBudget) -> Result<Self, VMError> {
        let bytes = tlv
            .payload
            .len()
            .checked_add(super::TLV_ENVELOPE_OVERHEAD)
            .ok_or(VMError::Deferred(VmDeferral::AllocationDeferred(
                iroha_allocation::AllocationRefusal::DemandOverflow,
            )))?;
        let layout = std::alloc::Layout::array::<u8>(bytes).map_err(|_| {
            VMError::Deferred(VmDeferral::AllocationDeferred(
                iroha_allocation::AllocationRefusal::DemandOverflow,
            ))
        })?;
        let allocation = budget
            .try_reserve_bytes(bytes)
            .map_err(|original| VMError::Deferred(VmDeferral::AllocationDeferred(original)))?;
        let mut output = Vec::new();
        output.try_reserve_exact(bytes).map_err(|original| {
            VMError::Deferred(VmDeferral::AllocatorDeferred(
                AllocatorRefusal::from_vec_reserve(original, layout),
            ))
        })?;
        output.extend_from_slice(&(tlv.type_id as u16).to_be_bytes());
        output.push(tlv.version);
        output.extend_from_slice(&(tlv.payload.len() as u32).to_be_bytes());
        output.extend_from_slice(tlv.payload);
        let hash: [u8; 32] = iroha_crypto::Hash::new(tlv.payload).into();
        output.extend_from_slice(&hash);
        Ok(Self {
            bytes: output,
            _allocation: allocation,
        })
    }
}

impl AsRef<[u8]> for FundedPublicTlv {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

pub(super) fn allocate(vm: &mut dyn SyscallContext) -> Result<u64, VMError> {
    let size = vm.public_register(10)?;
    let address = vm.alloc_heap(size)?;
    vm.write_register(10, address)?;
    Ok(allocation_gas(size))
}

pub(super) fn publish_input<B: AsRef<[u8]>>(
    vm: &mut dyn SyscallContext,
    copy: impl FnOnce(&dyn SyscallContext, u64) -> Result<B, VMError>,
) -> Result<u64, VMError> {
    let source = vm.public_register(10)?;
    if source == 0 {
        vm.write_register(10, 0)?;
        return Ok(DefaultHost::input_publish_gas(0));
    }
    if source >= crate::memory::Memory::HEAP_START {
        let tlv = vm.public_tlv(source)?;
        let extent = 7usize.saturating_add(tlv.payload.len()).saturating_add(32);
        return Ok(DefaultHost::input_publish_gas(extent));
    }
    let resolved = DefaultHost::resolve_literal_pointer(vm, source)
        .ok_or(VMError::Trap(VmTrap::NoritoInvalid))?;
    let original = copy(vm, resolved)?;
    let bytes = original.as_ref();
    let total = bytes.len();
    let destination = vm.alloc_encoded_host_tlv(bytes)?;
    vm.write_register(10, destination)?;
    Ok(DefaultHost::input_publish_gas(total))
}

/// Quote only the shared memory calls of the exact carrier profile.
/// The caller separately owns the original carrier invocation capability.
/// # Errors
/// Rejects another selected profile or preserves native pointer/gas validation errors.
pub fn quote_carrier_memory_syscall(
    number: u32,
    vm: SyscallQuote<'_>,
) -> Result<Option<u64>, VMError> {
    if vm.syscall_policy() != crate::SyscallPolicy::AbiV1GenericCarrier {
        return Err(VMError::Trap(VmTrap::PermissionDenied));
    }
    if matches!(
        number,
        syscalls::SYSCALL_ALLOC | syscalls::SYSCALL_INPUT_PUBLISH_TLV
    ) {
        vm.common_syscall_gas_quote(number)
    } else {
        Ok(None)
    }
}

/// Execute the same memory kernels with the carrier's original temporary-copy owner.
/// This supplies no instruction, storage or caller authority.
/// # Errors
/// Rejects another profile, missing original VM pool, malformed pointers or original refusal.
pub fn execute_carrier_memory_syscall(
    number: u32,
    vm: &mut dyn SyscallContext,
) -> Result<Option<u64>, VMError> {
    if vm.quote().syscall_policy() != crate::SyscallPolicy::AbiV1GenericCarrier {
        return Err(VMError::Trap(VmTrap::PermissionDenied));
    }
    match number {
        syscalls::SYSCALL_ALLOC => allocate(vm).map(Some),
        syscalls::SYSCALL_INPUT_PUBLISH_TLV => {
            publish_input(vm, |vm, pointer| vm.clone_tlv_with_original_memory(pointer)).map(Some)
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests;
