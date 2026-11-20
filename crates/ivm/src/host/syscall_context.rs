//! Operations available to the host body after the VM reserves syscall gas.

use crate::error::VmTrap;
use crate::{IVM, Memory, VMError};
use iroha_crypto::{CompactMerkleProof, HashOf, MerkleTree};
use ivm_abi::{
    pointer_abi::{PointerType, Tlv},
    private_input::PrivateInputKindV1,
};

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::IVM {}
}

/// Host execution capabilities for the current VM invocation.
///
/// Memory proof output is constructed by the VM in its authenticated format.
/// The host supplies no proof bytes or arbitrary memory writer.
pub trait SyscallContext: sealed::Sealed {
    /// Read a public guest register within the active register geometry.
    fn public_register(&self, index: usize) -> Result<u64, VMError>;

    /// Write one guest result register within the active register geometry.
    fn write_register(&mut self, index: usize, value: u64) -> Result<(), VMError>;

    /// Validate a whole guest-owned public TLV, including its provenance, type and digest.
    fn public_tlv(&self, address: u64) -> Result<Tlv<'_>, VMError>;

    /// Publish a canonical typed return in the VM's existing input or heap allocation.
    fn alloc_public_tlv(&mut self, kind: PointerType, payload: &[u8]) -> Result<u64, VMError>;

    /// Grow the heap by the size in register 10, publish the new limit and return the gas used.
    fn grow_selected_heap(&mut self) -> Result<u64, VMError>;

    /// Borrow the guest's public output prefix after the caller has prepaid the copy.
    fn public_output(&self) -> Result<&[u8], VMError>;

    /// Select and validate a host-supplied private input without disclosing either
    /// guest selector to the host, and publish only the resulting opaque pointer.
    fn install_selected_private_input(&mut self, records: &[Vec<u8>]) -> Result<(), VMError>;

    /// Compute the protocol commitment over the value and blinding input in registers 10 and 11.
    fn commit_private_numeric_inputs(&mut self) -> Result<(), VMError>;

    /// Write the canonical memory path and optional root at the public ABI destinations.
    fn write_memory_merkle_path(&mut self) -> Result<usize, VMError>;

    /// Write a depth-capped memory proof and optional root at the public ABI destinations.
    fn write_compact_memory_merkle_path(&mut self) -> Result<usize, VMError>;

    /// Write a depth-capped register proof and optional root at the public ABI destinations.
    fn write_compact_register_merkle_path(&mut self) -> Result<usize, VMError>;
}

impl SyscallContext for IVM {
    fn public_register(&self, index: usize) -> Result<u64, VMError> {
        self.syscall_quote().public_register(index)
    }

    fn write_register(&mut self, index: usize, value: u64) -> Result<(), VMError> {
        self.syscall_quote().check_register_index(index)?;
        self.set_register(index, value);
        Ok(())
    }

    fn public_tlv(&self, address: u64) -> Result<Tlv<'_>, VMError> {
        self.validate_tlv(address)
    }

    fn alloc_public_tlv(&mut self, kind: PointerType, payload: &[u8]) -> Result<u64, VMError> {
        let envelope = ivm_abi::pointer_abi::encode_tlv(kind, payload)?;
        self.alloc_host_tlv(&envelope)
    }

    fn grow_selected_heap(&mut self) -> Result<u64, VMError> {
        let size = self.register(10);
        let limit = self.grow_heap(size)?;
        self.set_register(10, limit);
        Ok(super::DefaultHost::grow_heap_gas(size))
    }

    fn public_output(&self) -> Result<&[u8], VMError> {
        self.ensure_public_memory(Memory::OUTPUT_START, self.output_used_len())?;
        Ok(self.read_output_used())
    }

    fn install_selected_private_input(&mut self, records: &[Vec<u8>]) -> Result<(), VMError> {
        let expected_kind = PrivateInputKindV1::from_tag(self.register(11))
            .ok_or(VMError::Trap(VmTrap::NoritoInvalid))?;
        let record = usize::try_from(self.register(10))
            .ok()
            .and_then(|index| records.get(index))
            .ok_or(VMError::Trap(VmTrap::PermissionDenied))?;
        let value = crate::private_input::decode_record(record, expected_kind)?;
        debug_assert_eq!(value.kind, expected_kind);
        let pointer = self.alloc_host_private_tlv(&value.envelope)?;
        self.set_register(10, pointer);
        Ok(())
    }

    fn commit_private_numeric_inputs(&mut self) -> Result<(), VMError> {
        use iroha_primitives::numeric_abi::MAX_QUANTITY_ENVELOPE_BYTES_V1;

        let value = self.snapshot_private_tlv(self.register(10), MAX_QUANTITY_ENVELOPE_BYTES_V1)?;
        let blind = self.snapshot_private_tlv(self.register(11), MAX_QUANTITY_ENVELOPE_BYTES_V1)?;
        let value_kind = crate::private_input::validate_private_numeric_envelope(&value)?;
        let blind_kind = crate::private_input::validate_private_numeric_envelope(&blind)?;
        let commitment = crate::private_input::valcom(value_kind, &value, blind_kind, &blind)?;
        let encoded = ivm_abi::numeric_tlv::encode_int(&commitment)?;
        let pointer = self.alloc_host_tlv(&encoded)?;
        self.set_register(10, pointer);
        Ok(())
    }

    fn write_memory_merkle_path(&mut self) -> Result<usize, VMError> {
        let addr = self.syscall_quote().public_register(10)?;
        let max_addr = self
            .memory
            .stack_top()
            .saturating_add(crate::Memory::STACK_SLOP);
        if addr >= max_addr {
            return Err(VMError::Trap(VmTrap::MemoryOutOfBounds));
        }
        let dest = self.syscall_quote().public_register(11)?;
        let root_out = self.syscall_quote().public_register(12)?;
        let (root, path) = self.memory.merkle_root_and_path(addr)?;
        for (i, node) in path.iter().enumerate() {
            self.memory.store_bytes(dest + (i as u64) * 32, node)?;
        }
        if root_out != 0 {
            self.memory.store_bytes(root_out, root.as_ref())?;
        }
        self.set_register(10, path.len() as u64);
        Ok(path.len())
    }

    fn write_compact_memory_merkle_path(&mut self) -> Result<usize, VMError> {
        let addr = self.syscall_quote().public_register(10)?;
        let max_addr = self
            .memory
            .stack_top()
            .saturating_add(crate::Memory::STACK_SLOP);
        if addr >= max_addr {
            return Err(VMError::Trap(VmTrap::MemoryOutOfBounds));
        }
        let dest = self.syscall_quote().public_register(11)?;
        let depth_cap_raw = self.syscall_quote().public_register(12)?;
        let depth_cap = if depth_cap_raw == 0 {
            None
        } else {
            Some(usize::try_from(depth_cap_raw.min(32)).expect("depth cap fits usize"))
        };
        let root_out = self.syscall_quote().public_register(13)?;
        let (proof, root) = self.memory.merkle_compact(addr, depth_cap)?;
        let depth = write_compact_proof(&mut self.memory, dest, root_out, proof, root)?;
        self.set_register(10, depth as u64);
        Ok(depth)
    }

    fn write_compact_register_merkle_path(&mut self) -> Result<usize, VMError> {
        let idx_raw = self.syscall_quote().public_register(10)?;
        let idx =
            usize::try_from(idx_raw).map_err(|_| VMError::Trap(VmTrap::RegisterOutOfBounds))?;
        if idx >= crate::parallel::REGISTER_COUNT {
            return Err(VMError::Trap(VmTrap::RegisterOutOfBounds));
        }
        let dest = self.syscall_quote().public_register(11)?;
        let depth_cap_raw = self.syscall_quote().public_register(12)?;
        let depth_cap = if depth_cap_raw == 0 {
            None
        } else {
            Some(usize::try_from(depth_cap_raw.min(32)).expect("depth cap fits usize"))
        };
        let root_out = self.syscall_quote().public_register(13)?;
        let (proof, root) = self.registers.merkle_compact(idx, depth_cap)?;
        let depth = write_compact_proof(&mut self.memory, dest, root_out, proof, root)?;
        self.set_register(10, depth as u64);
        Ok(depth)
    }
}

fn write_compact_proof(
    memory: &mut Memory,
    destination: u64,
    root_destination: u64,
    proof: CompactMerkleProof<[u8; 32]>,
    root: HashOf<MerkleTree<[u8; 32]>>,
) -> Result<usize, VMError> {
    let depth = proof.depth();
    memory.store_bytes(destination, &[depth])?;
    memory.store_bytes(destination + 1, &proof.dirs().to_le_bytes())?;
    memory.store_bytes(destination + 5, &u32::from(depth).to_le_bytes())?;
    let mut next = destination + 9;
    for sibling in proof.siblings() {
        let bytes = sibling.map(|hash| *hash.as_ref()).unwrap_or([0u8; 32]);
        memory.store_bytes(next, &bytes)?;
        next += 32;
    }
    if root_destination != 0 {
        memory.store_bytes(root_destination, root.as_ref())?;
    }
    Ok(usize::from(depth))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::VmTrap;

    #[test]
    fn register_access_checks_geometry_and_only_reads_public_values() {
        let mut vm = IVM::new(1000);
        vm.set_zk_mode(true).unwrap();
        vm.write_register(12, 41).unwrap();
        assert_eq!(vm.public_register(12).unwrap(), 41);
        vm.registers.set_tag(12, true);
        assert!(matches!(
            vm.public_register(12),
            Err(VMError::Trap(VmTrap::PrivacyViolation))
        ));
        assert!(matches!(
            vm.write_register(usize::MAX, 17),
            Err(VMError::Trap(VmTrap::RegisterOutOfBounds))
        ));
        assert_eq!(vm.register(12), 41);
    }

    #[test]
    fn public_tlv_round_trips_the_canonical_envelope_and_refuses_private_bytes() {
        let mut vm = IVM::new(1000);
        vm.set_zk_mode(true).unwrap();
        let payload = b"host return";
        let address = vm.alloc_public_tlv(PointerType::Blob, payload).unwrap();
        let value = vm.public_tlv(address).unwrap();
        assert_eq!(value.type_id, PointerType::Blob);
        assert_eq!(value.payload, payload);
        let envelope = ivm_abi::pointer_abi::encode_tlv(PointerType::Blob, payload).unwrap();
        let private = vm.alloc_host_private_tlv(&envelope).unwrap();
        assert!(matches!(
            vm.public_tlv(private),
            Err(VMError::Trap(VmTrap::PrivacyViolation))
        ));
        assert!(vm.public_tlv(Memory::STACK_START).is_err());
    }

    #[test]
    fn heap_growth_uses_the_selected_size_without_returning_it_to_the_host() {
        let mut vm = IVM::new(1000);
        vm.set_zk_mode(true).unwrap();
        vm.set_register(10, 4096);
        vm.registers.set_tag(10, true);
        let limit = Memory::HEAP_MAX_SIZE - 4096;
        vm.memory.set_heap_limit(limit).unwrap();
        assert_eq!(
            vm.grow_selected_heap().unwrap(),
            super::super::DefaultHost::grow_heap_gas(4096)
        );
        assert_eq!(vm.register(10), limit + 4096);
    }

    #[test]
    fn public_output_borrows_only_the_written_guest_prefix() {
        let mut vm = IVM::new(1000);
        assert!(vm.public_output().unwrap().is_empty());
        vm.store_u64(Memory::OUTPUT_START, 741).unwrap();
        assert_eq!(vm.public_output().unwrap(), 741u64.to_le_bytes());
    }

    #[test]
    fn private_input_selection_keeps_both_tagged_selectors_in_the_vm() {
        let mut vm = IVM::new(1000);
        vm.set_zk_mode(true).unwrap();
        let record = crate::private_input::int_record(42_u64.into()).unwrap();
        let inputs = [crate::private_input::encode_record(&record).unwrap()];
        vm.set_register(10, 0);
        vm.set_register(11, PrivateInputKindV1::Int.tag());
        vm.registers.set_tag(10, true);
        vm.registers.set_tag(11, true);
        assert!(matches!(
            vm.public_register(10),
            Err(VMError::Trap(VmTrap::PrivacyViolation))
        ));
        assert!(matches!(
            vm.public_register(11),
            Err(VMError::Trap(VmTrap::PrivacyViolation))
        ));
        vm.install_selected_private_input(&inputs).unwrap();
        assert!(matches!(
            vm.public_tlv(vm.register(10)),
            Err(VMError::Trap(VmTrap::PrivacyViolation))
        ));
    }

    #[test]
    fn refused_private_input_and_commitment_leave_output_untouched() {
        let mut vm = IVM::new(1000);
        vm.set_register(10, 42);
        vm.set_register(11, u64::MAX);
        assert!(matches!(
            vm.install_selected_private_input(&[]),
            Err(VMError::Trap(VmTrap::NoritoInvalid))
        ));
        vm.set_register(11, PrivateInputKindV1::Decimal.tag());
        assert!(matches!(
            vm.install_selected_private_input(&[]),
            Err(VMError::Trap(VmTrap::PermissionDenied))
        ));
        assert!(matches!(
            vm.commit_private_numeric_inputs(),
            Err(VMError::Trap(VmTrap::PrivacyViolation))
        ));
        assert_eq!(vm.register(10), 42);
        assert_eq!(vm.output_used_len(), 0);
    }

    #[test]
    fn merkle_outputs_check_public_destination_before_emitting_any_proof() {
        let mut vm = IVM::new(1000);
        vm.set_zk_mode(true).unwrap();
        vm.set_register(10, 0);
        vm.set_register(11, Memory::OUTPUT_START);
        vm.registers.set_tag(11, true);
        for write in [
            IVM::write_memory_merkle_path,
            IVM::write_compact_memory_merkle_path,
            IVM::write_compact_register_merkle_path,
        ] {
            assert!(matches!(
                write(&mut vm),
                Err(VMError::Trap(VmTrap::PrivacyViolation))
            ));
            assert_eq!(vm.memory.load_u64(Memory::OUTPUT_START).unwrap(), 0);
            assert_eq!(vm.register(10), 0);
        }
    }
}
