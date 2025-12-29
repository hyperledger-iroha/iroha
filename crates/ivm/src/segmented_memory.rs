//! Simple segmented memory model with fixed-size segments.
//!
//! Addresses are flat u64 values where the high 16 bits select the
//! segment and the low 16 bits are an offset within that segment.
//! The address ranges are:
//!   0x0000_0000..0x003F_FFFF -> Code (read-only)
//!   0x0040_0000..0x007F_FFFF -> Stack
//!   0x0080_0000..0x00BF_FFFF -> Heap
//! Each segment is currently 4 MB in size.

use crate::error::VMError;

/// Identifier for a memory segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    Code,
    Stack,
    Heap,
}

/// Segmented memory with code, stack and heap regions.
pub struct Memory {
    pub code: Vec<u8>,
    pub stack: Vec<u8>,
    pub heap: Vec<u8>,
}

impl Memory {
    /// Size of each segment in bytes (4 MB).
    pub const SEGMENT_SIZE: usize = 0x40_0000;
    /// Base addresses for each segment.
    pub const CODE_START: u64 = 0x0000_0000;
    pub const STACK_START: u64 = 0x0040_0000;
    pub const HEAP_START: u64 = 0x0080_0000;

    /// Create a new memory instance with default segment sizes.
    pub fn new() -> Self {
        Self {
            code: vec![0u8; Self::SEGMENT_SIZE],
            stack: vec![0u8; Self::SEGMENT_SIZE],
            heap: vec![0u8; Self::SEGMENT_SIZE],
        }
    }

    /// Map a flat address to a segment and offset.
    fn map_addr(&self, addr: u64, size: usize) -> Result<(Segment, usize), VMError> {
        // Compute end with overflow check to avoid wrapping comparisons
        let end = addr
            .checked_add(size as u64)
            .ok_or(VMError::MemoryOutOfBounds)?;
        let seg_sz = Self::SEGMENT_SIZE as u64;

        // Determine segment based on address range
        // NOTE: CODE_START is 0, so the lower-bound check would be tautological; only check upper bound.
        if end <= Self::CODE_START + seg_sz {
            Ok((Segment::Code, (addr - Self::CODE_START) as usize))
        } else if addr >= Self::STACK_START && end <= Self::STACK_START + seg_sz {
            Ok((Segment::Stack, (addr - Self::STACK_START) as usize))
        } else if addr >= Self::HEAP_START && end <= Self::HEAP_START + seg_sz {
            Ok((Segment::Heap, (addr - Self::HEAP_START) as usize))
        } else {
            Err(VMError::MemoryOutOfBounds)
        }
    }

    /// Load a u64 from memory (little-endian).
    pub fn load_u64(&self, addr: u64) -> Result<u64, VMError> {
        if !addr.is_multiple_of(8) {
            return Err(VMError::UnalignedAccess);
        }
        let (seg, off) = self.map_addr(addr, 8)?;
        let slice = match seg {
            Segment::Code => &self.code,
            Segment::Stack => &self.stack,
            Segment::Heap => &self.heap,
        };
        let bytes: [u8; 8] = slice[off..off + 8].try_into().unwrap();
        Ok(u64::from_le_bytes(bytes))
    }

    /// Store a u64 into memory (little-endian).
    pub fn store_u64(&mut self, addr: u64, value: u64) -> Result<(), VMError> {
        if !addr.is_multiple_of(8) {
            return Err(VMError::UnalignedAccess);
        }
        let (seg, off) = self.map_addr(addr, 8)?;
        let bytes = value.to_le_bytes();
        match seg {
            Segment::Code => Err(VMError::MemoryPermissionDenied),
            Segment::Stack => {
                self.stack[off..off + 8].copy_from_slice(&bytes);
                Ok(())
            }
            Segment::Heap => {
                self.heap[off..off + 8].copy_from_slice(&bytes);
                Ok(())
            }
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
