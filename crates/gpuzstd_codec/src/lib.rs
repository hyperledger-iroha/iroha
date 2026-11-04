//! Portable deterministic Zstd frame coding shared by the native GPU helpers.
//!
//! Backend crates own device execution and their separate plugin C exports. This
//! crate owns only the shared sequence layout, entropy coding and frame format.

pub mod bitstream;
pub mod fse;
pub mod huffman;
pub mod zstd_frame;

#[cfg(feature = "test-support")]
pub mod test_support;

/// Deterministic Zstd sequence emitted by GPU match-finding kernels.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuZstdSequence {
    /// Literal bytes preceding the match.
    pub lit_len: u32,
    /// Match length in bytes, or zero for the final literal-only tail.
    pub match_len: u32,
    /// Backward match offset in bytes.
    pub offset: u32,
    /// Reserved for ABI-compatible future extension; must be zero.
    pub reserved: u32,
}

#[cfg(test)]
mod tests {
    use super::GpuZstdSequence;

    #[test]
    fn sequence_layout_matches_both_device_abis() {
        assert_eq!(size_of::<GpuZstdSequence>(), 16);
        assert_eq!(align_of::<GpuZstdSequence>(), align_of::<u32>());
        assert_eq!(std::mem::offset_of!(GpuZstdSequence, lit_len), 0);
        assert_eq!(std::mem::offset_of!(GpuZstdSequence, match_len), 4);
        assert_eq!(std::mem::offset_of!(GpuZstdSequence, offset), 8);
        assert_eq!(std::mem::offset_of!(GpuZstdSequence, reserved), 12);
    }
}
