//! Operations available to the host body after the VM reserves syscall gas.

use crate::error::VmTrap;
use crate::{IVM, Memory, VMError};
use iroha_crypto::{CompactMerkleProof, HashOf, MerkleTree};

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::IVM {}
}

/// Host execution capabilities for the current VM invocation.
///
/// Memory proof output is constructed by the VM in its authenticated format.
/// The host supplies no proof bytes or arbitrary memory writer.
pub trait SyscallContext: sealed::Sealed {
    /// Write the canonical memory path and optional root at the public ABI destinations.
    fn write_memory_merkle_path(&mut self) -> Result<usize, VMError>;

    /// Write a depth-capped memory proof and optional root at the public ABI destinations.
    fn write_compact_memory_merkle_path(&mut self) -> Result<usize, VMError>;

    /// Write a depth-capped register proof and optional root at the public ABI destinations.
    fn write_compact_register_merkle_path(&mut self) -> Result<usize, VMError>;
}

impl SyscallContext for IVM {
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
