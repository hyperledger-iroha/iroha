//! Borrowed fixed-header admission before any owned contract-section decoding.

use super::*;

/// Fixed-header validation tied to the original immutable artifact borrow.
/// This is a dispatch hint, not contract, section, literal or opcode admission.
#[derive(Debug)]
pub struct ParsedProgramHeader<'a> {
    bytes: &'a [u8],
    metadata: ProgramMetadata,
    syscall_policy: crate::SyscallPolicy,
}
impl<'a> ParsedProgramHeader<'a> {
    /// Exact named policy authenticated by this original artifact's fixed header.
    #[must_use]
    pub const fn syscall_policy(&self) -> crate::SyscallPolicy {
        self.syscall_policy
    }

    /// Whether the next section starts with the sole canonical CNTR marker.
    /// A true value does not establish canonical payload or contract validity.
    #[must_use]
    pub fn declares_contract_interface(&self) -> bool {
        self.bytes.get(HEADER_SIZE..HEADER_SIZE + 4)
            == Some(CONTRACT_INTERFACE_SECTION_MAGIC.as_slice())
    }

    /// Decode the ordered sections from the exact original artifact.
    /// All section, codec-limit and malformed-input checks retain their canonical ordering.
    /// This consumes the borrowed header; it accepts no replacement bytes or offsets.
    pub fn parse_sections(self) -> Result<ParsedProgramMetadata, VMError> {
        super::parse_program_sections(self.bytes, self.metadata, self.syscall_policy)
    }
}

pub(super) fn parse(
    bytes: &[u8],
    syscall_policy: crate::SyscallPolicy,
) -> Result<ParsedProgramHeader<'_>, VMError> {
    if bytes.len() < HEADER_SIZE {
        return Err(VMError::InvalidMetadata);
    }
    let magic = &bytes[0..4];
    let version_major = bytes[4];
    if magic != MAGIC {
        return Err(VMError::InvalidMetadata);
    }
    let abi_version = bytes[HEADER_ABI_VERSION_OFFSET];
    let abi_hash: [u8; 32] = bytes[HEADER_ABI_HASH_OFFSET..HEADER_SIZE]
        .try_into()
        .map_err(|_| VMError::InvalidMetadata)?;
    let version_minor = bytes[5];
    let mode = bytes[6];
    let max_cycles_bytes: [u8; 8] = bytes[HEADER_BUDGET_OFFSET..HEADER_ABI_VERSION_OFFSET]
        .try_into()
        .map_err(|_| VMError::InvalidMetadata)?;
    let max_cycles = u64::from_le_bytes(max_cycles_bytes);
    // Validate consensus-visible header policy in stable precedence order:
    // version, unknown feature bits, ABI version, ZK cycle bound, ABI hash.
    // Structural length and magic failures necessarily precede these.
    //
    // Validate header fields according to the current implementation policy.
    // - The sole current program header is 1.1 for every execution profile.
    // - CNTR presence selects contract admission; test capabilities are separate.
    // - Mode must not contain unknown bits (only ZK).
    // - ABI V1 is the only first-release ABI.
    // - `max_cycles` is the ZK trace bound: nonzero exactly when the ZK bit is set.
    //   Gas is the only execution budget and is supplied by the caller.
    const KNOWN_MODE_BITS: u8 = mode::ZK;
    if version_major != 1 || version_minor != 1 {
        return Err(VMError::UnsupportedProgramVersion {
            major: version_major,
            minor: version_minor,
        });
    }
    let unsupported_feature_bits = mode & !KNOWN_MODE_BITS;
    if unsupported_feature_bits != 0 {
        return Err(VMError::UnsupportedProgramFeatureBits {
            bits: unsupported_feature_bits,
        });
    }
    if abi_version != 1 {
        return Err(VMError::UnsupportedProgramAbiVersion {
            version: abi_version,
        });
    }
    match (mode & mode::ZK != 0, max_cycles) {
        (true, 0) => return Err(VMError::MissingMaxCycles),
        (false, max_cycles) if max_cycles != 0 => {
            return Err(VMError::MaxCyclesWithoutZk { max_cycles });
        }
        _ => {}
    }
    let expected = crate::syscalls::compute_abi_hash(syscall_policy);
    if abi_hash != expected {
        return Err(VMError::ArtifactAbiHashMismatch {
            expected,
            actual: abi_hash,
        });
    }
    let image_len = bytes
        .len()
        .checked_sub(HEADER_SIZE)
        .ok_or(VMError::InvalidMetadata)?;
    if image_len > MAX_PROGRAM_IMAGE_BYTES_V1 {
        return Err(VMError::InvalidMetadata);
    }
    Ok(ParsedProgramHeader {
        bytes,
        syscall_policy,
        metadata: ProgramMetadata {
            version_major,
            version_minor,
            mode,
            max_cycles,
            abi_version,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_borrows_original_bytes_and_does_not_admit_a_truncated_cntr() {
        let mut bytes = ProgramMetadata::default().encode();
        bytes.extend_from_slice(&CONTRACT_INTERFACE_SECTION_MAGIC);
        let header = ProgramMetadata::parse_header(&bytes).unwrap();
        assert!(core::ptr::eq(header.bytes.as_ptr(), bytes.as_ptr()));
        assert!(header.declares_contract_interface());
        assert!(matches!(
            header.parse_sections().unwrap_err(),
            VMError::InvalidMetadata
        ));
        let header = ProgramMetadata::default().encode();
        let parsed = ProgramMetadata::parse_header(&header).unwrap();
        assert!(!parsed.declares_contract_interface());
        let parsed = parsed.parse_sections().unwrap();
        assert_eq!(parsed.header_len, HEADER_SIZE);
        assert_eq!(parsed.code_offset, HEADER_SIZE);
        assert!(parsed.contract_interface.is_none());
    }

    #[test]
    fn fixed_header_policy_precedes_section_claims_and_enclosing_decode_pressure() {
        let mut bytes = ProgramMetadata::default().encode();
        bytes.extend_from_slice(&CONTRACT_INTERFACE_SECTION_MAGIC);
        bytes[4] = 2;
        let limits = norito::DecodeLimits::new(0, 0, 0, 0, 0);
        let outcome = norito::core::with_decode_limits_scope(limits, || {
            ProgramMetadata::parse_header(&bytes)
        });
        assert!(
            matches!((&(outcome.unwrap_err()), &(VMError::UnsupportedProgramVersion { major: 2, minor: 1 })), (VMError::UnsupportedProgramVersion { major: __actual_major, minor: __actual_minor }, VMError::UnsupportedProgramVersion { major: __expected_major, minor: __expected_minor }) if __actual_major == __expected_major && __actual_minor == __expected_minor)
        );
    }
    #[test]
    fn cycle_bound_is_present_exactly_for_zk_headers() {
        let zk_without_bound = ProgramMetadata {
            mode: mode::ZK,
            ..ProgramMetadata::default()
        }
        .encode();
        assert!(matches!(
            ProgramMetadata::parse_header(&zk_without_bound),
            Err(VMError::MissingMaxCycles)
        ));
        let bound_without_zk = ProgramMetadata {
            max_cycles: 7,
            ..ProgramMetadata::default()
        }
        .encode();
        assert!(matches!(
            ProgramMetadata::parse_header(&bound_without_zk),
            Err(VMError::MaxCyclesWithoutZk { max_cycles: 7 })
        ));
        for metadata in [
            ProgramMetadata::default(),
            ProgramMetadata {
                mode: mode::ZK,
                max_cycles: 7,
                ..ProgramMetadata::default()
            },
        ] {
            assert!(ProgramMetadata::parse_header(&metadata.encode()).is_ok());
        }
    }
    #[test]
    fn retired_header_rejects_every_prefix_before_decode_or_local_capacity() {
        for suffix in [b"".as_slice(), b"CNTR", b"DBG1", b"LTLB"] {
            let mut bytes = ProgramMetadata::default().encode();
            bytes[5] = 0;
            bytes.extend_from_slice(suffix);
            let limits = norito::DecodeLimits::new(0, 0, 0, 0, 0);
            for corrupt_other_policy in [false, true] {
                if corrupt_other_policy {
                    bytes[6] = 0xff;
                    bytes[HEADER_ABI_VERSION_OFFSET] = 2;
                    bytes[HEADER_ABI_HASH_OFFSET] ^= 1;
                }
                let (header, sections) = norito::core::with_decode_limits_scope(limits, || {
                    (
                        ProgramMetadata::parse_header(&bytes),
                        ProgramMetadata::parse(&bytes),
                    )
                });
                assert!(matches!(
                    header.unwrap_err(),
                    VMError::UnsupportedProgramVersion { major: 1, minor: 0 }
                ));
                assert!(matches!(
                    sections.unwrap_err(),
                    VMError::UnsupportedProgramVersion { major: 1, minor: 0 }
                ));
            }
        }
    }
}
