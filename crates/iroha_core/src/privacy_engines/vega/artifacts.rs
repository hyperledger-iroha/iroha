//! Explicit governed-artifact sources for the native Vega Figure 9 engine.
//!
//! This module owns only source qualification and one-shot installation. It
//! performs no filesystem, environment, network, setup, or key-generation
//! lookup. Runtime owners must supply a manifest and lend the exact artifact
//! bytes through one of the role-separated callback interfaces below.

use core::fmt;

use iroha_zkp_halo2::vega::{
    VEGA_MDL_CANONICAL_RELATION_DIGEST_V1, VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1,
    VEGA_MDL_COMPILED_PROFILE_DIGEST_V1, VEGA_MDL_COMPILED_PROFILE_MANIFEST_V1,
    install_vega_mdl_figure9_prover_artifacts_v1, install_vega_mdl_figure9_verifier_key_v1,
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::VEGA_PINNED_SOURCE_COMMIT_V1;

/// Schema name of the exact Figure 9 artifact manifest.
pub const VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_V1: &str =
    "iroha.vega.figure9.microsoft-mc.artifacts";
/// Schema version of the exact Figure 9 artifact manifest.
pub const VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_VERSION_V1: u16 = 1;
/// Absolute byte ceiling for either canonical Figure 9 key artifact.
///
/// This mirrors the independent bounded decoders at the cryptographic engine
/// boundary. It is an input-safety limit, not evidence of an actual artifact's
/// released size or of prover peak memory.
pub const VEGA_MDL_FIGURE9_KEY_ARTIFACT_MAX_BYTES_V1: u64 = 512 * 1024 * 1024;

const ARTIFACT_MANIFEST_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.vega.figure9.microsoft-mc.artifact-manifest.v1\0";
const UPSTREAM_SOURCE_COMMIT_V1: [u8; 40] = *b"c0ee259053cd12eaf43ed71b5cde375452b3ee4d";
const UPSTREAM_SOURCE_TREE_V1: [u8; 40] = *b"7226b6cbfbfe8613dd2d5ee831096b7578a5c115";
const VENDOR_MANIFEST_SHA256_V1: [u8; 32] = [
    0x53, 0x9c, 0x54, 0x25, 0x1c, 0x88, 0x53, 0xfa, 0x99, 0x67, 0x3e, 0x71, 0xd7, 0x77, 0x96, 0x6a,
    0x3e, 0x3e, 0x23, 0x8e, 0x64, 0x02, 0x8d, 0x47, 0xb3, 0xe6, 0x83, 0x32, 0x90, 0x23, 0x23, 0x6f,
];
const UPSTREAM_COMMIT_MANIFEST_LINE_V1: &[u8] =
    b"upstream_commit=c0ee259053cd12eaf43ed71b5cde375452b3ee4d\n";
const UPSTREAM_TREE_MANIFEST_LINE_V1: &[u8] =
    b"upstream_tree=7226b6cbfbfe8613dd2d5ee831096b7578a5c115\n";
const VENDOR_MANIFEST_DIGEST_LINE_V1: &[u8] =
    b"vendor_manifest_sha256=539c54251c8853fa99673e71d777966a3e3e238e64028d47b3e683329023236f\n";

/// Role of one canonical Figure 9 setup artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum VegaMdlFigure9ArtifactRoleV1 {
    /// Canonical Microsoft proving key.
    ProvingKey = 1,
    /// Canonical Microsoft verifier key.
    VerifierKey = 2,
}

/// Exact public identity of one canonical artifact file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VegaMdlFigure9ArtifactBindingV1 {
    role: VegaMdlFigure9ArtifactRoleV1,
    exact_byte_len: u64,
    raw_canonical_sha256: [u8; 32],
}

impl VegaMdlFigure9ArtifactBindingV1 {
    /// Construct one bounded raw-artifact binding.
    ///
    /// `raw_canonical_sha256` is SHA-256 over the complete canonical artifact
    /// file. It is deliberately distinct from the logical verifier-key digest.
    ///
    /// # Errors
    ///
    /// Rejects zero or over-ceiling lengths and an all-zero raw SHA-256.
    pub fn new(
        role: VegaMdlFigure9ArtifactRoleV1,
        exact_byte_len: u64,
        raw_canonical_sha256: [u8; 32],
    ) -> Result<Self, VegaMdlFigure9ArtifactQualificationErrorV1> {
        let binding = Self {
            role,
            exact_byte_len,
            raw_canonical_sha256,
        };
        binding.validate()?;
        Ok(binding)
    }

    /// Artifact role selected by this binding.
    #[must_use]
    pub const fn role(&self) -> VegaMdlFigure9ArtifactRoleV1 {
        self.role
    }

    /// Exact complete-file byte length.
    #[must_use]
    pub const fn exact_byte_len(&self) -> u64 {
        self.exact_byte_len
    }

    /// Raw SHA-256 over the complete canonical artifact file.
    #[must_use]
    pub const fn raw_canonical_sha256(&self) -> [u8; 32] {
        self.raw_canonical_sha256
    }

    fn validate(&self) -> Result<(), VegaMdlFigure9ArtifactQualificationErrorV1> {
        if self.exact_byte_len == 0
            || self.exact_byte_len > VEGA_MDL_FIGURE9_KEY_ARTIFACT_MAX_BYTES_V1
            || self.raw_canonical_sha256 == [0; 32]
        {
            return Err(
                VegaMdlFigure9ArtifactQualificationErrorV1::InvalidArtifactBinding {
                    role: self.role,
                },
            );
        }
        Ok(())
    }
}

/// Complete public identity of the governed Figure 9 PK/VK release.
///
/// The fields are private so callers cannot construct a manifest that omits a
/// profile or provenance binding. This value contains no key bytes or path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VegaMdlFigure9ArtifactManifestV1 {
    schema: &'static str,
    schema_version: u16,
    compiled_profile_digest: [u8; 32],
    canonical_relation_digest: [u8; 32],
    upstream_source_commit: [u8; 40],
    upstream_source_tree: [u8; 40],
    vendor_manifest_sha256: [u8; 32],
    logical_governed_verifier_digest: [u8; 32],
    proving_key: VegaMdlFigure9ArtifactBindingV1,
    verifier_key: VegaMdlFigure9ArtifactBindingV1,
}

impl VegaMdlFigure9ArtifactManifestV1 {
    /// Construct the exact compiled Figure 9 artifact manifest.
    ///
    /// The caller supplies only the two raw canonical file identities. All
    /// schema, relation, profile, logical-key, and upstream provenance fields
    /// come from the compiled engine profile and are revalidated at install.
    ///
    /// # Errors
    ///
    /// Rejects reversed roles, invalid bindings, or ambiguous raw identities.
    pub fn new(
        proving_key: VegaMdlFigure9ArtifactBindingV1,
        verifier_key: VegaMdlFigure9ArtifactBindingV1,
    ) -> Result<Self, VegaMdlFigure9ArtifactQualificationErrorV1> {
        let manifest = Self {
            schema: VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_V1,
            schema_version: VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_VERSION_V1,
            compiled_profile_digest: VEGA_MDL_COMPILED_PROFILE_DIGEST_V1,
            canonical_relation_digest: VEGA_MDL_CANONICAL_RELATION_DIGEST_V1,
            upstream_source_commit: UPSTREAM_SOURCE_COMMIT_V1,
            upstream_source_tree: UPSTREAM_SOURCE_TREE_V1,
            vendor_manifest_sha256: VENDOR_MANIFEST_SHA256_V1,
            logical_governed_verifier_digest: VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1,
            proving_key,
            verifier_key,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Manifest schema name.
    #[must_use]
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    /// Manifest schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    /// Digest of the complete compiled Microsoft-MC adapter profile.
    #[must_use]
    pub const fn compiled_profile_digest(&self) -> [u8; 32] {
        self.compiled_profile_digest
    }

    /// Digest of the complete canonical Figure 9 relation.
    #[must_use]
    pub const fn canonical_relation_digest(&self) -> [u8; 32] {
        self.canonical_relation_digest
    }

    /// Exact pinned upstream source commit as lowercase ASCII hexadecimal.
    #[must_use]
    pub const fn upstream_source_commit(&self) -> &[u8; 40] {
        &self.upstream_source_commit
    }

    /// Exact pinned upstream source tree as lowercase ASCII hexadecimal.
    #[must_use]
    pub const fn upstream_source_tree(&self) -> &[u8; 40] {
        &self.upstream_source_tree
    }

    /// SHA-256 of the pinned vendored-source manifest.
    #[must_use]
    pub const fn vendor_manifest_sha256(&self) -> [u8; 32] {
        self.vendor_manifest_sha256
    }

    /// Logical governed verifier-key digest used by the Microsoft protocol.
    ///
    /// This is not a raw artifact-file SHA-256. See the verifier binding's
    /// [`VegaMdlFigure9ArtifactBindingV1::raw_canonical_sha256`] instead.
    #[must_use]
    pub const fn logical_governed_verifier_digest(&self) -> [u8; 32] {
        self.logical_governed_verifier_digest
    }

    /// Exact binding for one setup-artifact role.
    #[must_use]
    pub const fn artifact(
        &self,
        role: VegaMdlFigure9ArtifactRoleV1,
    ) -> VegaMdlFigure9ArtifactBindingV1 {
        match role {
            VegaMdlFigure9ArtifactRoleV1::ProvingKey => self.proving_key,
            VegaMdlFigure9ArtifactRoleV1::VerifierKey => self.verifier_key,
        }
    }

    /// Stable SHA-256 identity of every public manifest field.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(ARTIFACT_MANIFEST_DIGEST_DOMAIN_V1);
        hasher.update((self.schema.len() as u64).to_be_bytes());
        hasher.update(self.schema.as_bytes());
        hasher.update(self.schema_version.to_be_bytes());
        hasher.update(self.compiled_profile_digest);
        hasher.update(self.canonical_relation_digest);
        hasher.update(self.upstream_source_commit);
        hasher.update(self.upstream_source_tree);
        hasher.update(self.vendor_manifest_sha256);
        hasher.update(self.logical_governed_verifier_digest);
        for binding in [self.proving_key, self.verifier_key] {
            hasher.update([binding.role as u8]);
            hasher.update(binding.exact_byte_len.to_be_bytes());
            hasher.update(binding.raw_canonical_sha256);
        }
        hasher.finalize().into()
    }

    fn validate(&self) -> Result<(), VegaMdlFigure9ArtifactQualificationErrorV1> {
        if self.schema != VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_V1
            || self.schema_version != VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_VERSION_V1
        {
            return Err(VegaMdlFigure9ArtifactQualificationErrorV1::SchemaMismatch);
        }
        if self.compiled_profile_digest != VEGA_MDL_COMPILED_PROFILE_DIGEST_V1 {
            return Err(VegaMdlFigure9ArtifactQualificationErrorV1::ProfileMismatch);
        }
        if self.canonical_relation_digest != VEGA_MDL_CANONICAL_RELATION_DIGEST_V1 {
            return Err(VegaMdlFigure9ArtifactQualificationErrorV1::RelationMismatch);
        }
        if self.upstream_source_commit != UPSTREAM_SOURCE_COMMIT_V1
            || self.upstream_source_commit.as_slice() != VEGA_PINNED_SOURCE_COMMIT_V1
            || self.upstream_source_tree != UPSTREAM_SOURCE_TREE_V1
            || self.vendor_manifest_sha256 != VENDOR_MANIFEST_SHA256_V1
            || !compiled_profile_contains_once(UPSTREAM_COMMIT_MANIFEST_LINE_V1)
            || !compiled_profile_contains_once(UPSTREAM_TREE_MANIFEST_LINE_V1)
            || !compiled_profile_contains_once(VENDOR_MANIFEST_DIGEST_LINE_V1)
        {
            return Err(VegaMdlFigure9ArtifactQualificationErrorV1::ProvenanceMismatch);
        }
        if self.logical_governed_verifier_digest != VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1
            || self.logical_governed_verifier_digest == [0; 32]
        {
            return Err(VegaMdlFigure9ArtifactQualificationErrorV1::LogicalVerifierDigestMismatch);
        }
        self.proving_key.validate()?;
        self.verifier_key.validate()?;
        if self.proving_key.role != VegaMdlFigure9ArtifactRoleV1::ProvingKey {
            return Err(
                VegaMdlFigure9ArtifactQualificationErrorV1::InvalidArtifactBinding {
                    role: self.proving_key.role,
                },
            );
        }
        if self.verifier_key.role != VegaMdlFigure9ArtifactRoleV1::VerifierKey {
            return Err(
                VegaMdlFigure9ArtifactQualificationErrorV1::InvalidArtifactBinding {
                    role: self.verifier_key.role,
                },
            );
        }
        if self.proving_key.raw_canonical_sha256 == self.verifier_key.raw_canonical_sha256 {
            return Err(VegaMdlFigure9ArtifactQualificationErrorV1::AmbiguousArtifactBindings);
        }
        Ok(())
    }
}

fn compiled_profile_contains_once(needle: &[u8]) -> bool {
    let mut matches = VEGA_MDL_COMPILED_PROFILE_MANIFEST_V1
        .windows(needle.len())
        .filter(|window| *window == needle);
    matches.next().is_some() && matches.next().is_none()
}

/// Error returned by a deployment-owned artifact source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaMdlFigure9ArtifactSourceErrorV1 {
    /// The provider could not lend the requested immutable artifact.
    #[error("Vega Figure 9 artifact source is unavailable")]
    Unavailable,
    /// The Core-owned consumer rejected the lent artifact.
    #[error("Vega Figure 9 artifact consumer rejected the source callback")]
    CallbackRejected,
}

/// Fail-closed manifest, source, authentication, or installation failure.
// Keep exact length diagnostics inline and allocation-free on this failure path.
#[allow(variant_size_differences)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaMdlFigure9ArtifactQualificationErrorV1 {
    /// Manifest schema name or version differs from the sole released schema.
    #[error("Vega Figure 9 artifact manifest schema mismatch")]
    SchemaMismatch,
    /// Compiled Microsoft-MC profile digest differs from the released profile.
    #[error("Vega Figure 9 artifact compiled-profile mismatch")]
    ProfileMismatch,
    /// Canonical Figure 9 relation digest differs from the released relation.
    #[error("Vega Figure 9 artifact relation mismatch")]
    RelationMismatch,
    /// Upstream commit, tree, or vendored-source manifest identity differs.
    #[error("Vega Figure 9 artifact upstream provenance mismatch")]
    ProvenanceMismatch,
    /// Logical governed VK digest differs from the released Microsoft key.
    #[error("Vega Figure 9 logical governed verifier digest mismatch")]
    LogicalVerifierDigestMismatch,
    /// A role, exact length, or raw canonical SHA-256 binding is invalid.
    #[error("invalid Vega Figure 9 {role:?} artifact binding")]
    InvalidArtifactBinding {
        /// Role whose binding is invalid.
        role: VegaMdlFigure9ArtifactRoleV1,
    },
    /// PK and VK claimed the same raw canonical artifact identity.
    #[error("Vega Figure 9 PK and VK artifact bindings are ambiguous")]
    AmbiguousArtifactBindings,
    /// Source returned an explicit provider failure.
    #[error("Vega Figure 9 artifact provider failed: {0}")]
    SourceFailure(VegaMdlFigure9ArtifactSourceErrorV1),
    /// Source omitted, repeated, or swallowed its callback result.
    #[error("Vega Figure 9 artifact source callback contract violation")]
    SourceContractViolation,
    /// Source changed its manifest during qualification.
    #[error("Vega Figure 9 artifact source changed its manifest during qualification")]
    ManifestChanged,
    /// Lent artifact length differed from its exact manifest binding.
    #[error("Vega Figure 9 {role:?} artifact length {actual} differs from exact length {expected}")]
    LengthMismatch {
        /// Artifact role.
        role: VegaMdlFigure9ArtifactRoleV1,
        /// Exact manifest length.
        expected: u64,
        /// Lent byte length.
        actual: u64,
    },
    /// Raw SHA-256 of the complete canonical artifact file differed.
    #[error("Vega Figure 9 {role:?} raw canonical artifact SHA-256 mismatch")]
    RawCanonicalSha256Mismatch {
        /// Artifact role.
        role: VegaMdlFigure9ArtifactRoleV1,
    },
    /// Temporary bounded ownership needed to defer installation was unavailable.
    #[error("Vega Figure 9 {role:?} artifact allocation failed")]
    AllocationFailed {
        /// Artifact role.
        role: VegaMdlFigure9ArtifactRoleV1,
    },
    /// The strict canonical Microsoft key decoder or pairing check rejected.
    #[error("Vega Figure 9 cryptographic artifact installer rejected the candidate")]
    InstallerRejected,
}

/// Verifier-only source of one immutable Figure 9 VK artifact.
///
/// The source must invoke `consume` exactly once and propagate its result. This
/// interface has no proving-key argument or callback, so verifier runtimes are
/// never asked to receive or lend PK bytes.
pub trait VegaMdlFigure9VerifierArtifactSourceV1: Send + Sync {
    /// Complete public release manifest served by this source.
    fn artifact_manifest(&self) -> &VegaMdlFigure9ArtifactManifestV1;

    /// Lend the exact canonical verifier-key bytes for one callback.
    fn with_verifier_key(
        &self,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1>,
    ) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1>;
}

/// Prover source of one immutable, mutually bound Figure 9 PK/VK pair.
///
/// The source must invoke `consume` exactly once and propagate its result. Core
/// authenticates both complete files before the process-global pair installer
/// can run.
pub trait VegaMdlFigure9ProverArtifactSourceV1: Send + Sync {
    /// Complete public release manifest served by this source.
    fn artifact_manifest(&self) -> &VegaMdlFigure9ArtifactManifestV1;

    /// Lend the exact canonical proving-key and verifier-key bytes together.
    fn with_prover_artifacts(
        &self,
        consume: &mut dyn FnMut(&[u8], &[u8]) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1>,
    ) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1>;
}

struct SourceCallbackStateV1<T> {
    callback_count: u8,
    outcome: Option<Result<T, VegaMdlFigure9ArtifactQualificationErrorV1>>,
}

impl<T> SourceCallbackStateV1<T> {
    const fn new() -> Self {
        Self {
            callback_count: 0,
            outcome: None,
        }
    }

    fn enter(&mut self) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1> {
        self.callback_count = self.callback_count.saturating_add(1);
        if self.callback_count != 1 {
            return Err(VegaMdlFigure9ArtifactSourceErrorV1::CallbackRejected);
        }
        Ok(())
    }

    fn record(
        &mut self,
        outcome: Result<T, VegaMdlFigure9ArtifactQualificationErrorV1>,
    ) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1> {
        let callback_result = if outcome.is_ok() {
            Ok(())
        } else {
            Err(VegaMdlFigure9ArtifactSourceErrorV1::CallbackRejected)
        };
        self.outcome = Some(outcome);
        callback_result
    }

    fn finish(
        self,
        source_result: Result<(), VegaMdlFigure9ArtifactSourceErrorV1>,
    ) -> Result<T, VegaMdlFigure9ArtifactQualificationErrorV1> {
        if self.callback_count == 0 {
            return match source_result {
                Ok(()) | Err(VegaMdlFigure9ArtifactSourceErrorV1::CallbackRejected) => {
                    Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceContractViolation)
                }
                Err(provider_error) => Err(
                    VegaMdlFigure9ArtifactQualificationErrorV1::SourceFailure(provider_error),
                ),
            };
        }
        if self.callback_count != 1 {
            return Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceContractViolation);
        }
        match self.outcome {
            Some(Err(callback_error)) => match source_result {
                Ok(()) | Err(VegaMdlFigure9ArtifactSourceErrorV1::CallbackRejected) => {
                    Err(callback_error)
                }
                Err(provider_error) => Err(
                    VegaMdlFigure9ArtifactQualificationErrorV1::SourceFailure(provider_error),
                ),
            },
            Some(Ok(value)) => match source_result {
                Ok(()) => Ok(value),
                Err(VegaMdlFigure9ArtifactSourceErrorV1::CallbackRejected) => {
                    Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceContractViolation)
                }
                Err(provider_error) => Err(
                    VegaMdlFigure9ArtifactQualificationErrorV1::SourceFailure(provider_error),
                ),
            },
            None => Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceContractViolation),
        }
    }
}

fn authenticate_and_copy_artifact(
    bytes: &[u8],
    expected: VegaMdlFigure9ArtifactBindingV1,
) -> Result<Vec<u8>, VegaMdlFigure9ArtifactQualificationErrorV1> {
    let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual != expected.exact_byte_len
        || actual == 0
        || actual > VEGA_MDL_FIGURE9_KEY_ARTIFACT_MAX_BYTES_V1
    {
        return Err(VegaMdlFigure9ArtifactQualificationErrorV1::LengthMismatch {
            role: expected.role,
            expected: expected.exact_byte_len,
            actual,
        });
    }
    if <[u8; 32]>::from(Sha256::digest(bytes)) != expected.raw_canonical_sha256 {
        return Err(
            VegaMdlFigure9ArtifactQualificationErrorV1::RawCanonicalSha256Mismatch {
                role: expected.role,
            },
        );
    }
    let mut owned = Vec::new();
    owned.try_reserve_exact(bytes.len()).map_err(|_| {
        VegaMdlFigure9ArtifactQualificationErrorV1::AllocationFailed {
            role: expected.role,
        }
    })?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

/// Opaque proof that the exact verifier artifact was installed successfully.
///
/// This receipt is intentionally move-only. It retains public manifest metadata
/// only and exposes no artifact bytes, provider, handle, or path.
#[must_use = "the verifier-artifact install receipt must remain bound to its runtime owner"]
pub struct VegaMdlFigure9VerifierArtifactInstallReceiptV1 {
    manifest: VegaMdlFigure9ArtifactManifestV1,
}

impl fmt::Debug for VegaMdlFigure9VerifierArtifactInstallReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VegaMdlFigure9VerifierArtifactInstallReceiptV1")
            .field("manifest_sha256", &self.manifest.manifest_sha256())
            .field(
                "logical_governed_verifier_digest",
                &self.manifest.logical_governed_verifier_digest,
            )
            .finish_non_exhaustive()
    }
}

impl VegaMdlFigure9VerifierArtifactInstallReceiptV1 {
    /// Borrow the public manifest identity retained after installation.
    #[must_use]
    pub const fn manifest(&self) -> &VegaMdlFigure9ArtifactManifestV1 {
        &self.manifest
    }

    /// Stable SHA-256 of the complete public artifact manifest.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest.manifest_sha256()
    }
}

/// Opaque proof that the exact mutually bound PK/VK pair installed successfully.
///
/// This receipt is intentionally move-only. It retains public manifest metadata
/// only and exposes no artifact bytes, provider, handle, or path.
#[must_use = "the prover-artifact install receipt must remain bound to its runtime owner"]
pub struct VegaMdlFigure9ProverArtifactInstallReceiptV1 {
    manifest: VegaMdlFigure9ArtifactManifestV1,
}

impl fmt::Debug for VegaMdlFigure9ProverArtifactInstallReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VegaMdlFigure9ProverArtifactInstallReceiptV1")
            .field("manifest_sha256", &self.manifest.manifest_sha256())
            .field(
                "logical_governed_verifier_digest",
                &self.manifest.logical_governed_verifier_digest,
            )
            .finish_non_exhaustive()
    }
}

impl VegaMdlFigure9ProverArtifactInstallReceiptV1 {
    /// Borrow the public manifest identity retained after installation.
    #[must_use]
    pub const fn manifest(&self) -> &VegaMdlFigure9ArtifactManifestV1 {
        &self.manifest
    }

    /// Stable SHA-256 of the complete public artifact manifest.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest.manifest_sha256()
    }
}

/// Qualify and install one verifier-only artifact from an explicit source.
///
/// The source callback is completed and audited before the irreversible
/// process-global installer runs. Temporary bytes are dropped before return.
///
/// # Errors
///
/// Fails on any manifest, provider, callback, raw-file, allocation, or strict
/// cryptographic-installer mismatch.
pub fn qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(
    source: &dyn VegaMdlFigure9VerifierArtifactSourceV1,
) -> Result<
    VegaMdlFigure9VerifierArtifactInstallReceiptV1,
    VegaMdlFigure9ArtifactQualificationErrorV1,
> {
    let manifest = source.artifact_manifest().clone();
    manifest.validate()?;
    let expected = manifest.artifact(VegaMdlFigure9ArtifactRoleV1::VerifierKey);
    let mut state = SourceCallbackStateV1::new();
    let source_result = source.with_verifier_key(&mut |verifier_key| {
        state.enter()?;
        let artifact = authenticate_and_copy_artifact(verifier_key, expected);
        state.record(artifact)
    });
    let verifier_key = state.finish(source_result)?;
    if source.artifact_manifest() != &manifest {
        return Err(VegaMdlFigure9ArtifactQualificationErrorV1::ManifestChanged);
    }
    install_vega_mdl_figure9_verifier_key_v1(&verifier_key)
        .map_err(|_| VegaMdlFigure9ArtifactQualificationErrorV1::InstallerRejected)?;
    drop(verifier_key);
    Ok(VegaMdlFigure9VerifierArtifactInstallReceiptV1 { manifest })
}

/// Qualify and install one mutually bound prover PK/VK pair from an explicit source.
///
/// The source callback is completed and audited before the irreversible
/// process-global pair installer runs. Temporary bytes are dropped before
/// return.
///
/// # Errors
///
/// Fails on any manifest, provider, callback, raw-file, allocation, or strict
/// cryptographic-installer mismatch.
pub fn qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(
    source: &dyn VegaMdlFigure9ProverArtifactSourceV1,
) -> Result<VegaMdlFigure9ProverArtifactInstallReceiptV1, VegaMdlFigure9ArtifactQualificationErrorV1>
{
    let manifest = source.artifact_manifest().clone();
    manifest.validate()?;
    let expected_proving_key = manifest.artifact(VegaMdlFigure9ArtifactRoleV1::ProvingKey);
    let expected_verifier_key = manifest.artifact(VegaMdlFigure9ArtifactRoleV1::VerifierKey);
    let mut state = SourceCallbackStateV1::new();
    let source_result = source.with_prover_artifacts(&mut |proving_key, verifier_key| {
        state.enter()?;
        let artifacts = (|| {
            let proving_key = authenticate_and_copy_artifact(proving_key, expected_proving_key)?;
            let verifier_key = authenticate_and_copy_artifact(verifier_key, expected_verifier_key)?;
            Ok((proving_key, verifier_key))
        })();
        state.record(artifacts)
    });
    let (proving_key, verifier_key) = state.finish(source_result)?;
    if source.artifact_manifest() != &manifest {
        return Err(VegaMdlFigure9ArtifactQualificationErrorV1::ManifestChanged);
    }
    install_vega_mdl_figure9_prover_artifacts_v1(&proving_key, &verifier_key)
        .map_err(|_| VegaMdlFigure9ArtifactQualificationErrorV1::InstallerRejected)?;
    drop((proving_key, verifier_key));
    Ok(VegaMdlFigure9ProverArtifactInstallReceiptV1 { manifest })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;

    const ARTIFACT_SOURCE: &str = include_str!("artifacts.rs");
    const SYNTHETIC_PROVING_KEY: &[u8] = b"not-a-canonical-figure9-proving-key";
    const SYNTHETIC_VERIFIER_KEY: &[u8] = b"not-a-canonical-figure9-verifier-key";

    fn raw_sha256(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    fn binding(
        role: VegaMdlFigure9ArtifactRoleV1,
        bytes: &[u8],
    ) -> VegaMdlFigure9ArtifactBindingV1 {
        VegaMdlFigure9ArtifactBindingV1::new(
            role,
            u64::try_from(bytes.len()).expect("synthetic length fits u64"),
            raw_sha256(bytes),
        )
        .expect("bounded nonzero synthetic binding")
    }

    fn manifest() -> VegaMdlFigure9ArtifactManifestV1 {
        VegaMdlFigure9ArtifactManifestV1::new(
            binding(
                VegaMdlFigure9ArtifactRoleV1::ProvingKey,
                SYNTHETIC_PROVING_KEY,
            ),
            binding(
                VegaMdlFigure9ArtifactRoleV1::VerifierKey,
                SYNTHETIC_VERIFIER_KEY,
            ),
        )
        .expect("metadata-only synthetic manifest")
    }

    #[derive(Clone, Copy)]
    enum SourceBehavior {
        Once,
        OnceThenFail,
        Omit,
        FailBeforeCallback,
        TwiceAndSwallow,
        SwallowCallbackFailure,
        CallbackFailureThenProviderFailure,
    }

    struct VerifierSource {
        manifest: VegaMdlFigure9ArtifactManifestV1,
        verifier_key: Vec<u8>,
        behavior: SourceBehavior,
        callback_count: AtomicUsize,
    }

    impl VerifierSource {
        fn new(behavior: SourceBehavior, verifier_key: Vec<u8>) -> Self {
            Self {
                manifest: manifest(),
                verifier_key,
                behavior,
                callback_count: AtomicUsize::new(0),
            }
        }
    }

    impl VegaMdlFigure9VerifierArtifactSourceV1 for VerifierSource {
        fn artifact_manifest(&self) -> &VegaMdlFigure9ArtifactManifestV1 {
            &self.manifest
        }

        fn with_verifier_key(
            &self,
            consume: &mut dyn FnMut(&[u8]) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1>,
        ) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1> {
            match self.behavior {
                SourceBehavior::Once => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    consume(&self.verifier_key)
                }
                SourceBehavior::OnceThenFail => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    consume(&self.verifier_key)?;
                    Err(VegaMdlFigure9ArtifactSourceErrorV1::Unavailable)
                }
                SourceBehavior::Omit => Ok(()),
                SourceBehavior::FailBeforeCallback => {
                    Err(VegaMdlFigure9ArtifactSourceErrorV1::Unavailable)
                }
                SourceBehavior::TwiceAndSwallow => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    let _ = consume(&self.verifier_key);
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    let _ = consume(&self.verifier_key);
                    Ok(())
                }
                SourceBehavior::SwallowCallbackFailure => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    let _ = consume(&self.verifier_key);
                    Ok(())
                }
                SourceBehavior::CallbackFailureThenProviderFailure => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    let _ = consume(&self.verifier_key);
                    Err(VegaMdlFigure9ArtifactSourceErrorV1::Unavailable)
                }
            }
        }
    }

    struct ProverSource {
        manifest: VegaMdlFigure9ArtifactManifestV1,
        proving_key: Vec<u8>,
        verifier_key: Vec<u8>,
        behavior: SourceBehavior,
        callback_count: AtomicUsize,
    }

    impl ProverSource {
        fn new(behavior: SourceBehavior, proving_key: Vec<u8>, verifier_key: Vec<u8>) -> Self {
            Self {
                manifest: manifest(),
                proving_key,
                verifier_key,
                behavior,
                callback_count: AtomicUsize::new(0),
            }
        }
    }

    impl VegaMdlFigure9ProverArtifactSourceV1 for ProverSource {
        fn artifact_manifest(&self) -> &VegaMdlFigure9ArtifactManifestV1 {
            &self.manifest
        }

        fn with_prover_artifacts(
            &self,
            consume: &mut dyn FnMut(
                &[u8],
                &[u8],
            ) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1>,
        ) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1> {
            match self.behavior {
                SourceBehavior::Once => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    consume(&self.proving_key, &self.verifier_key)
                }
                SourceBehavior::OnceThenFail => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    consume(&self.proving_key, &self.verifier_key)?;
                    Err(VegaMdlFigure9ArtifactSourceErrorV1::Unavailable)
                }
                SourceBehavior::Omit => Ok(()),
                SourceBehavior::FailBeforeCallback => {
                    Err(VegaMdlFigure9ArtifactSourceErrorV1::Unavailable)
                }
                SourceBehavior::TwiceAndSwallow => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    let _ = consume(&self.proving_key, &self.verifier_key);
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    let _ = consume(&self.proving_key, &self.verifier_key);
                    Ok(())
                }
                SourceBehavior::SwallowCallbackFailure => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    let _ = consume(&self.proving_key, &self.verifier_key);
                    Ok(())
                }
                SourceBehavior::CallbackFailureThenProviderFailure => {
                    self.callback_count.fetch_add(1, Ordering::SeqCst);
                    let _ = consume(&self.proving_key, &self.verifier_key);
                    Err(VegaMdlFigure9ArtifactSourceErrorV1::Unavailable)
                }
            }
        }
    }

    struct ChangingManifestVerifierSource {
        before: VegaMdlFigure9ArtifactManifestV1,
        after: VegaMdlFigure9ArtifactManifestV1,
        changed: AtomicBool,
    }

    impl VegaMdlFigure9VerifierArtifactSourceV1 for ChangingManifestVerifierSource {
        fn artifact_manifest(&self) -> &VegaMdlFigure9ArtifactManifestV1 {
            if self.changed.load(Ordering::SeqCst) {
                &self.after
            } else {
                &self.before
            }
        }

        fn with_verifier_key(
            &self,
            consume: &mut dyn FnMut(&[u8]) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1>,
        ) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1> {
            let result = consume(SYNTHETIC_VERIFIER_KEY);
            self.changed.store(true, Ordering::SeqCst);
            result
        }
    }

    #[test]
    fn manifest_keeps_logical_and_raw_verifier_identities_distinct() {
        let manifest = manifest();
        assert_eq!(
            manifest.logical_governed_verifier_digest(),
            VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1
        );
        assert_eq!(
            manifest
                .artifact(VegaMdlFigure9ArtifactRoleV1::VerifierKey)
                .raw_canonical_sha256(),
            raw_sha256(SYNTHETIC_VERIFIER_KEY)
        );
        assert_ne!(
            manifest.logical_governed_verifier_digest(),
            manifest
                .artifact(VegaMdlFigure9ArtifactRoleV1::VerifierKey)
                .raw_canonical_sha256()
        );
        assert_ne!(manifest.manifest_sha256(), [0; 32]);
        assert_eq!(
            manifest.upstream_source_commit().as_slice(),
            VEGA_PINNED_SOURCE_COMMIT_V1
        );
    }

    #[test]
    fn production_source_has_no_ambient_loader_and_verifier_trait_has_no_pk_bytes() {
        let production = ARTIFACT_SOURCE
            .split_once("#[cfg(test)]")
            .expect("test-module marker")
            .0;
        for forbidden in [
            "std::fs",
            "std::env",
            "std::net",
            "File::",
            "PathBuf",
            "TcpStream",
            "UdpSocket",
        ] {
            assert!(
                !production.contains(forbidden),
                "ambient artifact loader escaped into production: {forbidden}"
            );
        }
        let verifier_trait = production
            .split_once("pub trait VegaMdlFigure9VerifierArtifactSourceV1")
            .expect("verifier source trait")
            .1
            .split_once("/// Prover source")
            .expect("prover source boundary")
            .0;
        assert!(!verifier_trait.contains("proving_key"));
        assert!(!verifier_trait.contains("with_prover_artifacts"));

        for receipt in [
            "VegaMdlFigure9VerifierArtifactInstallReceiptV1",
            "VegaMdlFigure9ProverArtifactInstallReceiptV1",
        ] {
            let marker = format!("pub struct {receipt} {{");
            let position = production
                .find(&marker)
                .expect("opaque receipt declaration");
            let declaration_prefix = &production[position.saturating_sub(160)..position];
            assert!(!declaration_prefix.contains("#[derive"));
            let body = production[position + marker.len()..]
                .split_once('}')
                .expect("opaque receipt body")
                .0;
            assert_eq!(body.trim(), "manifest: VegaMdlFigure9ArtifactManifestV1,");
            assert!(!production.contains(&format!("impl Clone for {receipt}")));
            assert!(!production.contains(&format!("impl Copy for {receipt}")));
        }
    }

    #[test]
    fn manifest_rejects_every_compiled_identity_and_binding_drift() {
        let mut malformed = manifest();
        malformed.schema = "wrong";
        assert_eq!(
            malformed.validate(),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SchemaMismatch)
        );

        let mut malformed = manifest();
        malformed.compiled_profile_digest[0] ^= 1;
        assert_eq!(
            malformed.validate(),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::ProfileMismatch)
        );

        let mut malformed = manifest();
        malformed.canonical_relation_digest[0] ^= 1;
        assert_eq!(
            malformed.validate(),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::RelationMismatch)
        );

        let mut malformed = manifest();
        malformed.upstream_source_tree[0] ^= 1;
        assert_eq!(
            malformed.validate(),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::ProvenanceMismatch)
        );

        let mut malformed = manifest();
        malformed.logical_governed_verifier_digest[0] ^= 1;
        assert_eq!(
            malformed.validate(),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::LogicalVerifierDigestMismatch)
        );

        for exact_byte_len in [0, VEGA_MDL_FIGURE9_KEY_ARTIFACT_MAX_BYTES_V1 + 1] {
            assert_eq!(
                VegaMdlFigure9ArtifactBindingV1::new(
                    VegaMdlFigure9ArtifactRoleV1::VerifierKey,
                    exact_byte_len,
                    [1; 32],
                ),
                Err(
                    VegaMdlFigure9ArtifactQualificationErrorV1::InvalidArtifactBinding {
                        role: VegaMdlFigure9ArtifactRoleV1::VerifierKey,
                    }
                )
            );
        }
        assert!(matches!(
            VegaMdlFigure9ArtifactBindingV1::new(
                VegaMdlFigure9ArtifactRoleV1::ProvingKey,
                1,
                [0; 32],
            ),
            Err(
                VegaMdlFigure9ArtifactQualificationErrorV1::InvalidArtifactBinding {
                    role: VegaMdlFigure9ArtifactRoleV1::ProvingKey,
                }
            )
        ));
    }

    #[test]
    fn verifier_source_contract_rejects_omission_provider_failure_and_repetition() {
        let omitted = VerifierSource::new(SourceBehavior::Omit, SYNTHETIC_VERIFIER_KEY.to_vec());
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&omitted),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceContractViolation)
        ));

        let failed = VerifierSource::new(
            SourceBehavior::FailBeforeCallback,
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&failed),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceFailure(
                VegaMdlFigure9ArtifactSourceErrorV1::Unavailable
            ))
        ));

        let repeated = VerifierSource::new(
            SourceBehavior::TwiceAndSwallow,
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&repeated),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceContractViolation)
        ));
        assert_eq!(repeated.callback_count.load(Ordering::SeqCst), 2);

        let late_failure = VerifierSource::new(
            SourceBehavior::OnceThenFail,
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&late_failure),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceFailure(
                VegaMdlFigure9ArtifactSourceErrorV1::Unavailable
            ))
        ));
    }

    #[test]
    fn callback_rejection_cannot_be_swallowed_or_reach_the_installer() {
        let propagated = VerifierSource::new(SourceBehavior::Once, b"wrong-length".to_vec());
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&propagated),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::LengthMismatch {
                role: VegaMdlFigure9ArtifactRoleV1::VerifierKey,
                ..
            })
        ));

        let source = VerifierSource::new(
            SourceBehavior::SwallowCallbackFailure,
            b"wrong-length".to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&source),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::LengthMismatch {
                role: VegaMdlFigure9ArtifactRoleV1::VerifierKey,
                ..
            })
        ));
        assert_eq!(source.callback_count.load(Ordering::SeqCst), 1);

        let mut same_length_wrong_digest = SYNTHETIC_VERIFIER_KEY.to_vec();
        same_length_wrong_digest[0] ^= 1;
        let source = VerifierSource::new(
            SourceBehavior::SwallowCallbackFailure,
            same_length_wrong_digest,
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&source),
            Err(
                VegaMdlFigure9ArtifactQualificationErrorV1::RawCanonicalSha256Mismatch {
                    role: VegaMdlFigure9ArtifactRoleV1::VerifierKey,
                }
            )
        ));

        let provider_failure = VerifierSource::new(
            SourceBehavior::CallbackFailureThenProviderFailure,
            b"wrong-length".to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&provider_failure),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceFailure(
                VegaMdlFigure9ArtifactSourceErrorV1::Unavailable
            ))
        ));
    }

    #[test]
    fn prover_source_contract_and_error_precedence_are_independently_enforced() {
        let omitted = ProverSource::new(
            SourceBehavior::Omit,
            SYNTHETIC_PROVING_KEY.to_vec(),
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(&omitted),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceContractViolation)
        ));

        let repeated = ProverSource::new(
            SourceBehavior::TwiceAndSwallow,
            SYNTHETIC_PROVING_KEY.to_vec(),
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(&repeated),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceContractViolation)
        ));
        assert_eq!(repeated.callback_count.load(Ordering::SeqCst), 2);

        let propagated = ProverSource::new(
            SourceBehavior::Once,
            b"wrong-length".to_vec(),
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(&propagated),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::LengthMismatch {
                role: VegaMdlFigure9ArtifactRoleV1::ProvingKey,
                ..
            })
        ));

        let swallowed = ProverSource::new(
            SourceBehavior::SwallowCallbackFailure,
            b"wrong-length".to_vec(),
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(&swallowed),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::LengthMismatch {
                role: VegaMdlFigure9ArtifactRoleV1::ProvingKey,
                ..
            })
        ));

        let callback_then_provider_failure = ProverSource::new(
            SourceBehavior::CallbackFailureThenProviderFailure,
            b"wrong-length".to_vec(),
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(
                &callback_then_provider_failure
            ),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceFailure(
                VegaMdlFigure9ArtifactSourceErrorV1::Unavailable
            ))
        ));

        let callback_success_then_provider_failure = ProverSource::new(
            SourceBehavior::OnceThenFail,
            SYNTHETIC_PROVING_KEY.to_vec(),
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(
                &callback_success_then_provider_failure
            ),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::SourceFailure(
                VegaMdlFigure9ArtifactSourceErrorV1::Unavailable
            ))
        ));
    }

    #[test]
    fn changing_source_manifest_is_rejected_before_installer() {
        let before = manifest();
        let mut after = before.clone();
        after.verifier_key.raw_canonical_sha256[0] ^= 1;
        let source = ChangingManifestVerifierSource {
            before,
            after,
            changed: AtomicBool::new(false),
        };
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&source),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::ManifestChanged)
        ));
    }

    #[test]
    fn matching_synthetic_files_reach_only_the_strict_rejecting_installers() {
        let verifier_source =
            VerifierSource::new(SourceBehavior::Once, SYNTHETIC_VERIFIER_KEY.to_vec());
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_verifier_artifact_v1(&verifier_source),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::InstallerRejected)
        ));

        let prover_source = ProverSource::new(
            SourceBehavior::Once,
            SYNTHETIC_PROVING_KEY.to_vec(),
            SYNTHETIC_VERIFIER_KEY.to_vec(),
        );
        assert!(matches!(
            qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(&prover_source),
            Err(VegaMdlFigure9ArtifactQualificationErrorV1::InstallerRejected)
        ));
        assert_eq!(prover_source.callback_count.load(Ordering::SeqCst), 1);
    }
}
