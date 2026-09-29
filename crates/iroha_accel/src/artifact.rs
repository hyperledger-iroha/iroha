//! Exact immutable CUDA loader identity; labels are not cache keys.

use std::ffi::CStr;

/// Immutable PTX admitted by the consumer's artifact/provenance verifier.
///
/// The owner compares every byte, including the terminating NUL. It exposes no
/// loader options: all modules use the same no-options Driver API load. This
/// type preserves identity; constructing it does not establish qualification or
/// provenance. No source compilation, file lookup, or download occurs here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PtxArtifact {
    bytes: &'static CStr,
}

impl PtxArtifact {
    /// Bind exact immutable NUL-terminated PTX bytes to this load request.
    pub const fn new(bytes: &'static CStr) -> Self {
        Self { bytes }
    }

    /// Exact bytes used both for cache equality and the native load.
    pub fn bytes(self) -> &'static CStr {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_uses_the_complete_artifact() {
        let first = PtxArtifact::new(c".version 8.0\n.target sm_80\n");
        assert_eq!(first, PtxArtifact::new(c".version 8.0\n.target sm_80\n"));
        assert_ne!(first, PtxArtifact::new(c".version 8.0\n.target sm_90\n"));
        assert_eq!(first.bytes().to_bytes_with_nul().last(), Some(&0));
    }
}
