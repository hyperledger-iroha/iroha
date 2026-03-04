//! Instruction decoding for the single-word (32-bit) IVM encoding.
//!
//! Every instruction occupies one 32-bit little-endian word aligned on a
//! 4-byte boundary, so decoding reduces to a bounds check followed by a single
//! load.  Compressed forms predating the wide encoding are no longer supported.
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    error::{Perm, VMError},
    memory::Memory,
};

#[cfg(test)]
pub static DECODE_CALLS: AtomicU64 = AtomicU64::new(0);

/// Decode the instruction at `pc`, returning the raw word and its length (always 4).
pub fn decode(memory: &Memory, pc: u64) -> Result<(u32, u32), VMError> {
    decode_wide(memory, pc)
}

/// Decode assuming the stream contains only 32-bit words aligned on 4-byte boundaries.
/// This helper bypasses the compressed-instruction logic and is intended for the
/// upcoming wide-encoding pipeline.
pub fn decode_wide(memory: &Memory, pc: u64) -> Result<(u32, u32), VMError> {
    #[cfg(test)]
    {
        DECODE_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    if !pc.is_multiple_of(4) {
        return Err(VMError::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        });
    }
    if pc + 4 > memory.code_len() {
        return Err(VMError::MemoryAccessViolation {
            addr: pc as u32,
            perm: Perm::EXECUTE,
        });
    }
    let word = memory.load_u32(pc)?;
    Ok((word, 4))
}
