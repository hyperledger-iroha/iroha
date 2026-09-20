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
use std::path::PathBuf;

pub(crate) use super::launcher::prepare::PrepareOutputCaps;

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
pub(super) struct SuppliedEvidenceBundleV1 {
    pub(super) version: u16,
    pub(super) heights: Vec<SuppliedEvidenceHeightV1>,
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::SuppliedEvidenceHeightV1")]
pub(super) struct SuppliedEvidenceHeightV1 {
    pub(super) height: u64,
    pub(super) finality: Vec<u8>,
    pub(super) contexts: Vec<u8>,
    pub(super) queries: Vec<Vec<u8>>,
}

/// Decoded launch authority with its original file descriptor and namespace retained.
///
/// Only secure request admission constructs this value. Export or replay consumes
/// it without exposing decoded facts separately from their input lease.
pub(crate) struct RetainedLauncherRequest {
    input_lease: InputPublicationLease,
    request: super::launcher::BoundLauncherRequest,
    reserved_bytes: u64,
}

/// Identity calculated from the exact canonical proof bytes under retained inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalProofIdentity {
    /// Raw SHA-256 for the independently pinned file role.
    pub(crate) raw_sha256: [u8; 32],
    /// Marked Iroha hash calculated independently with `Hash::new`.
    pub(crate) iroha_hash: Hash,
    /// Exact canonical frame length, not the admitted maximum.
    pub(crate) byte_length: u64,
}

/// Raw file identity of one canonical preparation transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreparedTransportIdentity {
    /// Raw SHA-256 of the full canonical frame, without Iroha hash marking.
    pub(crate) raw_sha256: [u8; 32],
    /// Actual byte length of the full frame.
    pub(crate) byte_length: u64,
}

/// Both transport identities under the retained original-facts and output owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreparedLaunchIdentity {
    /// Original canonical raw-facts file admitted by the caller.
    pub(crate) facts: PreparedTransportIdentity,
    /// Exact canonical launcher request identity.
    pub(crate) request: PreparedTransportIdentity,
    /// Exact canonical supplied-evidence bundle identity.
    pub(crate) bundle: PreparedTransportIdentity,
}

/// Structural stopped-height observation tied to the exact original signed genesis.
///
/// This is not authority for later carriers, finality certificates or execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StoppedTipIdentity {
    /// Raw identity of the independently authenticated generated genesis file.
    pub(crate) genesis: PreparedTransportIdentity,
    /// Exact genesis header network identity, distinct from raw file SHA-256.
    pub(crate) network_id: iroha_data_model::NetworkId,
    /// Whole committed marker height observed through the retained Core reader.
    pub(crate) committed_height: u64,
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
}

/// Receipt created only after file and retained parent durability completed.
pub(crate) struct PublishedProof {
    sha256: [u8; 32],
    byte_length: u64,
    proof: RetainedProof,
}
impl PublishedProof {
    /// Recheck original inputs and the identity certified at publication.
    ///
    /// The output descriptor is no longer retained here. This identifies the
    /// published bytes; a later output-file census and replay remain required.
    pub(crate) fn identity(&self) -> Result<CanonicalProofIdentity> {
        let identity = self.proof.identity()?;
        ensure!(
            identity.raw_sha256 == self.sha256 && identity.byte_length == self.byte_length,
            "publication identity differs from retained canonical proof"
        );
        Ok(identity)
    }
    /// Raw digest of the exact published canonical artifact.
    pub(crate) fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    /// Actual canonical artifact bytes, not its admitted maximum.
    pub(crate) fn byte_length(&self) -> u64 {
        self.byte_length
    }
    /// Derived complete projection; never a substitute for retained canonical proof.
    /// The independent maximum includes array delimiters and commas.
    pub(crate) fn json_projection(&self, maximum: u64) -> Result<Vec<u8>> {
        self.proof.json_projection(maximum)
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
pub(crate) use supported::{
    PreparedLaunch, PreparedOutputPair, ProofOutput, RetainedStoppedTip, export_bound_request,
    observe_stopped_tip, open_launcher, prepare_bound, replay_bound_request,
};

#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
mod unsupported {
    use super::*;
    use std::path::Path;

    // TODO: implement retained directory/file handles and atomic NOREPLACE on
    // other targets. Pinned rustix does not provide this full API set everywhere.
    pub(crate) fn open_launcher(_: ProofInputBinding) -> Result<RetainedLauncherRequest> {
        Err(eyre!("secure canonical proof filesystem is unsupported"))
    }
    pub(crate) fn replay_bound_request(
        _: RetainedLauncherRequest,
        _: Hash,
        _: ProofInputBinding,
    ) -> Result<RetainedProof> {
        Err(eyre!("secure canonical proof filesystem is unsupported"))
    }
    pub(crate) fn export_bound_request(
        _: RetainedLauncherRequest,
        _: &Path,
        _: &Path,
        _: CanonicalKuraEvidenceLimits,
        _: ProofInputBinding,
    ) -> Result<RetainedProof> {
        Err(eyre!("secure canonical proof filesystem is unsupported"))
    }
    impl RetainedProof {
        pub(crate) fn identity(&self) -> Result<CanonicalProofIdentity> {
            Err(eyre!("secure canonical proof filesystem is unsupported"))
        }
        pub(crate) fn json_projection(&self, _maximum: u64) -> Result<Vec<u8>> {
            Err(eyre!("secure canonical proof filesystem is unsupported"))
        }
    }
    pub(super) struct InputPublicationLease;
    impl InputPublicationLease {
        pub(super) fn check(&self) -> Result<()> {
            Err(eyre!("secure canonical proof filesystem is unsupported"))
        }
    }
    /// Unavailable retained preparation owner on this target.
    pub(crate) struct PreparedLaunch;
    impl PreparedLaunch {
        /// Report that no secure retained preparation identity is available.
        pub(crate) fn identity(&self) -> Result<PreparedLaunchIdentity> {
            Err(eyre!("secure preparation filesystem is unsupported"))
        }
    }
    /// Unavailable pair publication capability on this target.
    pub(crate) struct PreparedOutputPair;
    impl PreparedOutputPair {
        /// Reject targets without retained directory and NOREPLACE support.
        pub(crate) fn admit(_: &Path, _: &Path, _: PrepareOutputCaps) -> Result<Self> {
            Err(eyre!("secure preparation filesystem is unsupported"))
        }
    }
    /// Reject preparation without the secure retained filesystem implementation.
    pub(crate) fn prepare_bound(
        _: ProofInputBinding,
        _: PreparedOutputPair,
    ) -> Result<PreparedLaunch> {
        Err(eyre!("secure preparation filesystem is unsupported"))
    }
    /// Unsupported platforms cannot retain a stopped-tip observation.
    pub(crate) struct RetainedStoppedTip;
    impl RetainedStoppedTip {
        /// Fail without invoking a reply callback on unsupported platforms.
        pub(crate) fn finish_reply(
            self,
            _: impl FnOnce(StoppedTipIdentity) -> Result<()>,
        ) -> Result<StoppedTipIdentity> {
            Err(eyre!("secure stopped-tip filesystem is unsupported"))
        }
    }
    /// Reject stopped-tip observation before accessing files on unsupported platforms.
    pub(crate) fn observe_stopped_tip(
        _: ProofInputBinding,
        _: iroha_data_model::NetworkId,
        _: &Path,
        _: &Path,
        _: CanonicalKuraEvidenceLimits,
    ) -> Result<RetainedStoppedTip> {
        Err(eyre!("secure stopped-tip filesystem is unsupported"))
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
pub(crate) use unsupported::{
    PreparedLaunch, PreparedOutputPair, ProofOutput, RetainedStoppedTip, export_bound_request,
    observe_stopped_tip, open_launcher, prepare_bound, replay_bound_request,
};

#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
use unsupported::InputPublicationLease;

/// Exact original file roles, before any parser or semantic assembly runs.
pub(in crate::kura::scaling_evidence::export) struct FactsInputBindings {
    /// Original manifested genesis transaction JSON.
    pub(in crate::kura::scaling_evidence::export) manifest: ProofInputBinding,
    /// Original signed canonical genesis block.
    pub(in crate::kura::scaling_evidence::export) signed_genesis: ProofInputBinding,
    /// Four final peer configurations in independent validator order.
    pub(in crate::kura::scaling_evidence::export) peer_configs: [ProofInputBinding; 4],
    /// Original canonical revision-four genesis context.
    pub(in crate::kura::scaling_evidence::export) context: ProofInputBinding,
    /// Complete original physical signed-request event journal.
    pub(in crate::kura::scaling_evidence::export) journal: ProofInputBinding,
    /// Original canonical finality/context-witness vector for the entire selected interval.
    pub(in crate::kura::scaling_evidence::export) finality: ProofInputBinding,
    /// Original canonical committed-query vector in complete merge order.
    pub(in crate::kura::scaling_evidence::export) queries: ProofInputBinding,
}

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
pub(in crate::kura::scaling_evidence::export) use supported::{PublishedFacts, produce_facts};

#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
mod unsupported_facts {
    use super::*;
    use crate::kura::scaling_evidence::export::launcher::{
        journal::JournalExpectations,
        prepare::assemble::{FactsAssemblyCaps, GenesisExpectations},
    };

    /// Unsupported platforms never construct a facts publication capability.
    pub(in crate::kura::scaling_evidence::export) struct PublishedFacts;
    impl PublishedFacts {
        /// Unsupported platforms cannot return a checked facts identity.
        pub(in crate::kura::scaling_evidence::export) fn identity(
            &self,
        ) -> Result<PreparedTransportIdentity> {
            Err(eyre!("secure facts filesystem is unsupported"))
        }
        /// Fail without invoking a reply writer on unsupported platforms.
        pub(in crate::kura::scaling_evidence::export) fn finish_reply(
            self,
            _: impl FnOnce(PreparedTransportIdentity) -> Result<()>,
        ) -> Result<PreparedTransportIdentity> {
            Err(eyre!("secure facts filesystem is unsupported"))
        }
    }
    /// Reject facts publication before opening any file on unsupported platforms.
    #[expect(
        clippy::too_many_arguments,
        reason = "the unsupported boundary preserves the same typed launch inputs"
    )]
    pub(in crate::kura::scaling_evidence::export) fn produce_facts(
        _: FactsInputBindings,
        _: &Path,
        _: GenesisExpectations,
        _: JournalExpectations,
        _: VerificationLimits,
        _: &Path,
        _: &Path,
        _: CanonicalKuraEvidenceLimits,
        _: FactsAssemblyCaps,
    ) -> Result<PublishedFacts> {
        Err(eyre!("secure facts filesystem is unsupported"))
    }
}
#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
pub(in crate::kura::scaling_evidence::export) use unsupported_facts::{
    PublishedFacts, produce_facts,
};
