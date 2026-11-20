//! Instruction decoding for the single-word (32-bit) IVM encoding.
//!
//! Every instruction occupies one 32-bit little-endian word aligned on a
//! 4-byte boundary, so decoding reduces to a bounds check followed by a single
//! load.  Compressed forms predating the wide encoding are no longer supported.
use crate::error::VmTrap;
use crate::{
    error::{Perm, VMError},
    memory::Memory,
};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
pub static DECODE_CALLS: AtomicU64 = AtomicU64::new(0);
/// Decode the instruction at `pc` and return its raw word.
pub fn decode(memory: &Memory, pc: u64) -> Result<u32, VMError> {
    check_fetch(memory.code_len(), pc)?;
    memory.load_instruction_u32(pc)
}

/// Decode through exclusive read logging while the interpreter owns the guest image.
pub(crate) fn decode_guest(memory: &mut Memory, pc: u64) -> Result<u32, VMError> {
    check_fetch(memory.code_len(), pc)?;
    memory
        .load_guest_word(
            pc,
            crate::execution_memory_recorder::DiagnosticMemoryAccessKind::InstructionFetch,
        )
        .map(u32::from_le_bytes)
}

fn check_fetch(code_len: u64, pc: u64) -> Result<(), VMError> {
    #[cfg(test)]
    {
        DECODE_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    if !pc.is_multiple_of(4) {
        return Err(VMError::Trap(VmTrap::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        }));
    }
    let fetch_end = pc
        .checked_add(4)
        .ok_or(VMError::Trap(VmTrap::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        }))?;
    if fetch_end > code_len {
        return Err(VMError::Trap(VmTrap::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        }));
    }
    Ok(())
}
/// Decode one instruction directly from a code slice.
///
/// This is the allocation-free admission/predecode path. It deliberately has the same alignment,
/// bounds, byte order, errors, and fixed width as [`decode`], without constructing the VM's
/// multi-megabyte memory and Merkle image merely to inspect immutable code.
pub fn decode_slice(code: &[u8], pc: u64) -> Result<u32, VMError> {
    #[cfg(test)]
    {
        DECODE_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    if !pc.is_multiple_of(4) {
        return Err(VMError::Trap(VmTrap::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        }));
    }
    let start = usize::try_from(pc).map_err(|_| {
        VMError::Trap(VmTrap::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        })
    })?;
    let end = start
        .checked_add(4)
        .ok_or(VMError::Trap(VmTrap::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        }))?;
    let bytes = code
        .get(start..end)
        .ok_or(VMError::Trap(VmTrap::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        }))?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("four-byte slice"),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::VmTrap;
    #[test]
    fn shared_and_exclusive_fetch_keep_the_same_read_set_and_fault_order() {
        let mut memory = Memory::new();
        memory.load_code(&0x01234567u32.to_le_bytes()).unwrap();
        assert_eq!(decode(&memory, 0).unwrap(), 0x01234567);
        let expected = memory.try_read_log_snapshot().unwrap();
        memory.clear_tracking();
        assert!(matches!(
            decode_guest(&mut memory, 2),
            Err(VMError::Trap(VmTrap::MemoryAccessViolation {
                perm: Perm::EXECUTE,
                ..
            }))
        ));
        assert!(matches!(
            decode_guest(&mut memory, 4),
            Err(VMError::Trap(VmTrap::MemoryAccessViolation {
                perm: Perm::EXECUTE,
                ..
            }))
        ));
        assert!(memory.try_read_log_snapshot().unwrap().is_empty());
        assert_eq!(decode_guest(&mut memory, 0).unwrap(), 0x01234567);
        assert_eq!(&*memory.try_read_log_snapshot().unwrap(), &*expected);
    }

    #[test]
    fn slice_decoder_matches_memory_decoder_without_vm_memory() {
        let words = [0x1122_3344_u32, 0xAABB_CCDD];
        let code = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        let mut memory = Memory::new();
        memory
            .load_code(&code)
            .expect("decoder test program fits code region");
        for pc in [0, 4] {
            assert_eq!(
                decode_slice(&code, pc).unwrap(),
                decode(&memory, pc).unwrap()
            );
        }
        for pc in [1, 8, u64::MAX] {
            for result in [decode_slice(&code, pc), decode(&memory, pc)] {
                assert!(
                    matches!(result, Err(VMError::Trap(VmTrap::MemoryAccessViolation { addr, perm })) if addr == pc as u32 && perm == Perm::EXECUTE)
                );
            }
        }
    }
}
