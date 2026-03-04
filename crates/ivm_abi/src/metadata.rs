//! Program metadata parser used when loading bytecode.
//!
//! Each compiled program begins with a small header describing the VM version,
//! enabled features and optional cycle limit.  This module defines a
//! [`ProgramMetadata`] structure and helpers for encoding and decoding this
//! header.
//!
//! The metadata header encodes the VM version, execution mode flags, optional
//! vector length and cycle limit.  It also reserves bits for hardware
//! transactional memory (HTM) support.

use crate::error::VMError;

/// Maximum accepted logical vector length for admission.
pub const VECTOR_LENGTH_MAX: u8 = 64;

/// Magic prefix identifying IVM bytecode.
pub const MAGIC: &[u8; 4] = b"IVM\0";
pub const HEADER_SIZE: usize = 17;

/// Literal table section marker placed immediately after the metadata header
/// when compiled bytecode includes literal fixups.
pub const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";

/// Execution mode flags used in the metadata header.
pub mod mode {
    /// Zero-knowledge proof mode enabled.
    #[allow(dead_code)]
    pub const ZK: u8 = 0x01;
    /// Vector extension (SIMD/crypto ops) enabled.
    pub const VECTOR: u8 = 0x02;
    /// Hardware transactional memory enabled.
    #[allow(dead_code)]
    pub const HTM: u8 = 0x04;
}

#[derive(Clone, Debug)]
pub struct ProgramMetadata {
    pub version_major: u8,
    pub version_minor: u8,
    pub mode: u8,
    /// Logical vector length in lanes. `0` selects the maximum supported value.
    pub vector_length: u8,
    pub max_cycles: u64,
    /// ABI version for syscall table and pointer-ABI schema.
    pub abi_version: u8,
}

/// Result of parsing metadata and locating the code segment inside a program artifact.
#[derive(Clone, Debug)]
pub struct ParsedProgramMetadata {
    pub metadata: ProgramMetadata,
    /// Number of bytes occupied by the metadata header.
    pub header_len: usize,
    /// Absolute offset within the artifact where executable code begins.
    pub code_offset: usize,
}

impl ParsedProgramMetadata {
    /// Length of the literal/padding block placed between the header and executable code.
    pub fn literal_prefix_len(&self) -> usize {
        self.code_offset.saturating_sub(self.header_len)
    }
}

impl ProgramMetadata {
    pub fn parse(bytes: &[u8]) -> Result<ParsedProgramMetadata, VMError> {
        if bytes.len() < HEADER_SIZE {
            return Err(VMError::InvalidMetadata);
        }
        let magic = &bytes[0..4];
        let version_major = bytes[4];
        if magic != MAGIC {
            return Err(VMError::InvalidMetadata);
        }
        let abi_version = bytes[16];
        let header_len = HEADER_SIZE;
        let version_minor = bytes[5];
        let mode = bytes[6];
        let vector_length = bytes[7];
        let max_cycles_bytes: [u8; 8] = bytes[8..16]
            .try_into()
            .map_err(|_| VMError::InvalidMetadata)?;
        let max_cycles = u64::from_le_bytes(max_cycles_bytes);

        // Validate header fields according to the current implementation policy.
        // - Accept version 1.0 headers (first-release layout).
        // - Mode must not contain unknown bits (only ZK, VECTOR, HTM).
        // - `vector_length` is advisory and may be set regardless of the VECTOR bit.
        // - ABI version is carried as-is; admission enforces allowed values.
        const KNOWN_MODE_BITS: u8 = mode::ZK | mode::VECTOR | mode::HTM;
        if version_major != 1 {
            return Err(VMError::InvalidMetadata);
        }
        if version_minor != 0 {
            return Err(VMError::InvalidMetadata);
        }
        if mode & !KNOWN_MODE_BITS != 0 {
            return Err(VMError::InvalidMetadata);
        }
        // Note: vector_length may be non-zero even if VECTOR flag is off; the
        // host/runtime may clamp or ignore it depending on policy.
        let mut code_offset = header_len;
        // Optional literal section begins with `LTLB` magic immediately after the header.
        if bytes.len() >= header_len + 4
            && bytes[header_len..header_len + 4] == LITERAL_SECTION_MAGIC
        {
            let start = header_len;
            if bytes.len() < start + 16 {
                return Err(VMError::InvalidMetadata);
            }
            let count_bytes: [u8; 4] = bytes[start + 4..start + 8]
                .try_into()
                .map_err(|_| VMError::InvalidMetadata)?;
            let post_bytes: [u8; 4] = bytes[start + 8..start + 12]
                .try_into()
                .map_err(|_| VMError::InvalidMetadata)?;
            let data_bytes: [u8; 4] = bytes[start + 12..start + 16]
                .try_into()
                .map_err(|_| VMError::InvalidMetadata)?;
            let lit_count = u32::from_le_bytes(count_bytes) as usize;
            let post_pad = u32::from_le_bytes(post_bytes) as usize;
            let data_len = u32::from_le_bytes(data_bytes) as usize;
            let lit_len = lit_count
                .checked_mul(8)
                .and_then(|n| n.checked_add(16))
                .and_then(|n| n.checked_add(post_pad))
                .and_then(|n| n.checked_add(data_len))
                .ok_or(VMError::InvalidMetadata)?;
            code_offset = start.checked_add(lit_len).ok_or(VMError::InvalidMetadata)?;
            if code_offset > bytes.len() {
                return Err(VMError::InvalidMetadata);
            }
        } else if bytes.len() >= header_len + 4 {
            // Reject layouts that insert zero padding before the literal table marker.
            let max_scan = header_len + 32;
            let limit = bytes.len().saturating_sub(4);
            let end = max_scan.min(limit);
            let mut idx = header_len;
            while idx <= end {
                if bytes[idx..idx + 4] == LITERAL_SECTION_MAGIC {
                    let pad = &bytes[header_len..idx];
                    if pad.iter().all(|b| *b == 0) {
                        return Err(VMError::InvalidMetadata);
                    }
                    break;
                } else if bytes[idx] != 0 {
                    break;
                }
                idx += 1;
            }
        }
        Ok(ParsedProgramMetadata {
            metadata: Self {
                version_major,
                version_minor,
                mode,
                vector_length,
                max_cycles,
                abi_version,
            },
            header_len,
            code_offset,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(MAGIC);
        v.push(self.version_major);
        v.push(self.version_minor);
        v.push(self.mode);
        v.push(self.vector_length);
        v.extend_from_slice(&self.max_cycles.to_le_bytes());
        v.push(self.abi_version);
        v
    }

    /// Construct a default header for a specific `version_major.version_minor`
    /// and `abi_version`. Other fields are set to zero.
    pub fn default_for(version_major: u8, version_minor: u8, abi_version: u8) -> Self {
        Self {
            version_major,
            version_minor,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version,
        }
    }
}

impl Default for ProgramMetadata {
    fn default() -> Self {
        Self {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        }
    }
}
