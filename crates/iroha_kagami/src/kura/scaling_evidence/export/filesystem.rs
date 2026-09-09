//! Bounded canonical-proof files with retained namespace and consuming completion.
//!
//! Bindings come from independent launcher authority, never from capture content.
//! Only the exact export/replay owners can create `RetainedProof`; there is no
//! caller callback or arbitrary-bytes publication capability. The final evidence
//! census remains required. These finite byte/work limits do not bound kernel I/O
//! latency or claim an atomic filesystem snapshot against a privileged writer.

use super::{
    HeightInputBinding, SuppliedHeightEvidence, TrustedRunPlan, VerificationLimits, VerifiedExport,
};
use color_eyre::eyre::{Result, ensure, eyre};
use iroha_core::kura::CanonicalKuraEvidenceLimits;
use iroha_crypto::Hash;
use std::path::{Path, PathBuf};

const MAX_INPUT_FILES: usize = 64;
const MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;

/// One independently admitted file role, including raw SHA-256 and a smaller cap.
pub(crate) struct ProofInputBinding {
    /// Exact absolute lexical path; symlinked ancestors are never resolved.
    pub(crate) path: PathBuf,
    /// Raw SHA-256, distinct from the adapter's marked Iroha Hash.
    pub(crate) sha256: [u8; 32],
    /// Maximum bytes admitted before allocation; zero and values over 256 MiB fail.
    pub(crate) max_bytes: u64,
}

// One mandatory canonical, uncompressed Norito V1 transport. These decoded
// fields are untrusted until the separately supplied adapter bindings and fresh
// anchored plan succeed. A file descriptor is not allocated per height or leaf.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::SuppliedEvidenceBundleV1")]
struct SuppliedEvidenceBundleV1 {
    version: u16,
    heights: Vec<SuppliedEvidenceHeightV1>,
}
#[derive(norito::Encode, norito::Decode)]
struct SuppliedEvidenceHeightV1 {
    height: u64,
    finality: Vec<u8>,
    queries: Vec<Vec<u8>>,
}

/// Canonical proof whose exact input read, semantic owner and final census finished.
pub(crate) struct RetainedProof {
    input_lease: InputPublicationLease,
    proof: VerifiedExport,
}
impl RetainedProof {
    fn recheck_sources(&self) -> Result<()> {
        self.input_lease.check()?;
        if let Some(completed) = &self.proof.disk_completion {
            completed.recheck_sources()?;
        }
        Ok(())
    }
    /// Derived JSON only after the complete input/semantic boundary succeeded.
    pub(crate) fn json_projection(&self) -> Result<Vec<u8>> {
        self.recheck_sources()?;
        self.proof.json_projection()
    }
}

/// Receipt created only after file and retained parent durability completed.
pub(crate) struct PublishedProof {
    sha256: [u8; 32],
    byte_length: u64,
    proof: RetainedProof,
}
impl PublishedProof {
    /// Raw digest of the exact published canonical artifact.
    pub(crate) fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    /// Actual canonical artifact bytes, not its admitted maximum.
    pub(crate) fn byte_length(&self) -> u64 {
        self.byte_length
    }
    /// Derived complete projection; never a substitute for retained canonical proof.
    pub(crate) fn json_projection(&self) -> Result<Vec<u8>> {
        self.proof.json_projection()
    }
}

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
mod supported;
#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
use supported::InputPublicationLease;
#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
pub(crate) use supported::{ProofOutput, export_bound_kura, replay_bound_export};

#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
mod unsupported {
    use super::*;

    // TODO: implement retained directory/file handles and atomic NOREPLACE on
    // other targets. Pinned rustix does not provide this full API set everywhere.
    pub(crate) fn replay_bound_export(
        _: TrustedRunPlan,
        _: VerificationLimits,
        _: &[HeightInputBinding],
        _: Hash,
        _: ProofInputBinding,
    ) -> Result<RetainedProof> {
        Err(eyre!("secure canonical proof filesystem is unsupported"))
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn export_bound_kura(
        _: TrustedRunPlan,
        _: VerificationLimits,
        _: &Path,
        _: &Path,
        _: CanonicalKuraEvidenceLimits,
        _: &[HeightInputBinding],
        _: ProofInputBinding,
    ) -> Result<RetainedProof> {
        Err(eyre!("secure canonical proof filesystem is unsupported"))
    }
    pub(super) struct InputPublicationLease;
    impl InputPublicationLease {
        pub(super) fn check(&self) -> Result<()> {
            Err(eyre!("secure canonical proof filesystem is unsupported"))
        }
    }
    pub(crate) struct ProofOutput;
    impl ProofOutput {
        pub(crate) fn admit(_: &Path, _: u64) -> Result<Self> {
            Err(eyre!("secure canonical proof filesystem is unsupported"))
        }
        pub(crate) fn publish(self, _: RetainedProof) -> Result<PublishedProof> {
            Err(eyre!("secure canonical proof filesystem is unsupported"))
        }
    }
}
#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
pub(crate) use unsupported::{ProofOutput, export_bound_kura, replay_bound_export};

#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
use unsupported::InputPublicationLease;
