//! Portable Exact12 release and network-bound deployment qualification manifests.

use super::{
    PRIVACY_EXACT12_CATALOG_ID_V1, PrivacyAuditBundleDigestV1, PrivacyCapabilityRowV1,
    PrivacyCompiledProfileResultV1, PrivacyEngineIdV1, PrivacyEngineManifestDigestV1,
    PrivacyExact12CatalogCommitmentV1, PrivacyExact12DeploymentQualificationDigestV1,
    PrivacyExact12ReleaseManifestDigestV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
    PrivacyProofSystemIdV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
    PrivacyReleaseArtifactDigestV1, PrivacySecurityClaimDigestV1, PrivacySecurityClaimV1,
    PrivacyStatementSchemaDigestV1, PrivacyVerifierDigestV1,
};
use crate::{ChainId, NetworkId};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{PublicKey, Signature};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

/// Exact portable release-manifest wire version.
pub const PRIVACY_EXACT12_RELEASE_MANIFEST_VERSION_V1: u16 = 1;
/// Exact deployment-qualification wire version.
pub const PRIVACY_EXACT12_DEPLOYMENT_QUALIFICATION_VERSION_V1: u16 = 1;
/// Sole IVM ABI version admitted by the first release.
pub const PRIVACY_EXACT12_ABI_VERSION_V1: u16 = 1;
/// Number of release stages required for every protocol.
pub const PRIVACY_EXACT12_RELEASE_STAGES_PER_PROTOCOL_V1: usize = 4;
/// Exact number of stage receipts in one release manifest.
pub const PRIVACY_EXACT12_RELEASE_STAGE_RECEIPTS_V1: usize =
    PrivacyProtocolIdV1::COUNT * PRIVACY_EXACT12_RELEASE_STAGES_PER_PROTOCOL_V1;
/// Exact number of proof artifacts in one release manifest.
pub const PRIVACY_EXACT12_RELEASE_PROOF_ARTIFACTS_V1: usize = 54;
/// Exact number of independent audit classes.
pub const PRIVACY_EXACT12_RELEASE_AUDIT_CLASSES_V1: usize = 5;
/// Exact number of validators in the first deployment qualification.
pub const PRIVACY_EXACT12_DEPLOYMENT_VALIDATORS_V1: usize = 4;
/// Exact `2f + 1` deployment-signature quorum for `3f + 1 = 4` validators.
pub const PRIVACY_EXACT12_DEPLOYMENT_SIGNATURES_V1: usize = 3;
/// Maximum accepted Medium dispositions in one audit class.
pub const PRIVACY_EXACT12_MAX_MEDIUM_DISPOSITIONS_PER_AUDIT_V1: usize = 256;
/// Maximum byte length of a release artifact name or version identifier.
pub const PRIVACY_EXACT12_RELEASE_TEXT_MAX_BYTES_V1: usize = 256;

const RELEASE_ARTIFACT_SET_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:exact12:release-artifact-set:v1";
const RELEASE_AUDIT_BUNDLE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:exact12:audit-bundle:v1";
const RELEASE_MANIFEST_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:exact12:release-manifest:v1";
const RELEASE_SYSCALL_LIST_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:exact12:abi-v1-syscall-list:v1";
const RELEASE_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha:privacy:exact12:release-signature:v1";
const AUDIT_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha:privacy:exact12:audit-signature:v1";
const MEDIUM_DISPOSITION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:privacy:exact12:medium-disposition:v1";
const DEPLOYMENT_ROSTER_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:exact12:deployment-roster:v1";
const DEPLOYMENT_QUALIFICATION_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:exact12:deployment-qualification:v1";
const DEPLOYMENT_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha:privacy:exact12:deployment-signature:v1";

/// One of the four mandatory release-evidence stages for every protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "stage", content = "value", deny_unknown_fields)
)]
pub enum PrivacyReleaseStageV1 {
    /// A canonical public statement is proved and independently verified.
    #[cfg_attr(feature = "json", norito(rename = "positive-canonical-end-to-end"))]
    PositiveCanonicalEndToEnd,
    /// A structurally valid semantic public-input mutation rejects the proof.
    #[cfg_attr(feature = "json", norito(rename = "public-statement-binding-mutation"))]
    PublicStatementBindingMutation,
    /// Header corruption, interior corruption, and exact truncation all reject.
    #[cfg_attr(feature = "json", norito(rename = "proof-corruption-and-truncation"))]
    ProofCorruptionAndTruncation,
    /// The closed first-release maximum relation shape is proved and verified.
    #[cfg_attr(feature = "json", norito(rename = "maximum-shape-resource"))]
    MaximumShapeResource,
}

impl PrivacyReleaseStageV1 {
    /// Every mandatory stage in canonical order.
    pub const ALL: [Self; PRIVACY_EXACT12_RELEASE_STAGES_PER_PROTOCOL_V1] = [
        Self::PositiveCanonicalEndToEnd,
        Self::PublicStatementBindingMutation,
        Self::ProofCorruptionAndTruncation,
        Self::MaximumShapeResource,
    ];
}

/// Return the exact proof-artifact cardinality for one frozen evidence stage.
#[must_use]
pub const fn privacy_exact12_release_proof_artifact_count_v1(
    protocol_id: PrivacyProtocolIdV1,
    stage: PrivacyReleaseStageV1,
) -> u8 {
    if matches!(protocol_id, PrivacyProtocolIdV1::IrohaZkAmsV1)
        || (matches!(protocol_id, PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            && matches!(
                stage,
                PrivacyReleaseStageV1::PositiveCanonicalEndToEnd
                    | PrivacyReleaseStageV1::MaximumShapeResource
            ))
    {
        2
    } else {
        1
    }
}

/// Kind of immutable executable artifact shipped by the release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum PrivacyReleaseExecutableKindV1 {
    /// Native executable or library package.
    #[cfg_attr(feature = "json", norito(rename = "binary"))]
    Binary,
    /// Immutable deployable container image.
    #[cfg_attr(feature = "json", norito(rename = "container_image"))]
    ContainerImage,
}

/// Closed SDK and tooling package matrix required by the release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "consumer", content = "value", deny_unknown_fields)
)]
pub enum PrivacyReleaseSdkConsumerV1 {
    /// Kotlin/JVM SDK.
    #[cfg_attr(feature = "json", norito(rename = "kotlin_jvm"))]
    KotlinJvm,
    /// Kotlin Android SDK.
    #[cfg_attr(feature = "json", norito(rename = "kotlin_android"))]
    KotlinAndroid,
    /// Mirrored Java Android SDK.
    #[cfg_attr(feature = "json", norito(rename = "java_android"))]
    JavaAndroid,
    /// Swift SDK and C bridge.
    #[cfg_attr(feature = "json", norito(rename = "swift_c_bridge"))]
    SwiftCBridge,
    /// JavaScript N-API package.
    #[cfg_attr(feature = "json", norito(rename = "javascript_napi"))]
    JavascriptNapi,
    /// Python PyO3 package.
    #[cfg_attr(feature = "json", norito(rename = "python_pyo3"))]
    PythonPyo3,
    /// C# package.
    #[cfg_attr(feature = "json", norito(rename = "csharp"))]
    CSharp,
    /// Command-line client.
    #[cfg_attr(feature = "json", norito(rename = "cli"))]
    Cli,
    /// OpenAPI schema package.
    #[cfg_attr(feature = "json", norito(rename = "openapi"))]
    OpenApi,
    /// Genesis authoring tooling.
    #[cfg_attr(feature = "json", norito(rename = "genesis_tooling"))]
    GenesisTooling,
}

impl PrivacyReleaseSdkConsumerV1 {
    /// Every required consumer in canonical order.
    pub const ALL: [Self; 10] = [
        Self::KotlinJvm,
        Self::KotlinAndroid,
        Self::JavaAndroid,
        Self::SwiftCBridge,
        Self::JavascriptNapi,
        Self::PythonPyo3,
        Self::CSharp,
        Self::Cli,
        Self::OpenApi,
        Self::GenesisTooling,
    ];
}

/// Closed deterministic hardware backend matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "backend", content = "value", deny_unknown_fields)
)]
pub enum PrivacyReleaseHardwareBackendV1 {
    /// Portable scalar implementation.
    #[cfg_attr(feature = "json", norito(rename = "scalar"))]
    Scalar,
    /// x86-64 AVX2 implementation.
    #[cfg_attr(feature = "json", norito(rename = "avx2"))]
    Avx2,
    /// x86-64 AVX-512 implementation.
    #[cfg_attr(feature = "json", norito(rename = "avx512"))]
    Avx512,
    /// AArch64 NEON implementation.
    #[cfg_attr(feature = "json", norito(rename = "neon"))]
    Neon,
    /// Apple Metal implementation.
    #[cfg_attr(feature = "json", norito(rename = "metal"))]
    Metal,
    /// NVIDIA CUDA implementation.
    #[cfg_attr(feature = "json", norito(rename = "cuda"))]
    Cuda,
}

impl PrivacyReleaseHardwareBackendV1 {
    /// Every required backend in canonical order.
    pub const ALL: [Self; 6] = [
        Self::Scalar,
        Self::Avx2,
        Self::Avx512,
        Self::Neon,
        Self::Metal,
        Self::Cuda,
    ];
}

/// Closed independent audit classes required for release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "audit_class", content = "value", deny_unknown_fields)
)]
pub enum PrivacyReleaseAuditClassV1 {
    /// Cryptographic assumptions, relations, and reductions.
    #[cfg_attr(feature = "json", norito(rename = "cryptographic"))]
    Cryptographic,
    /// Implementation, parser, and side-channel review.
    #[cfg_attr(
        feature = "json",
        norito(rename = "implementation_parser_side_channel")
    )]
    ImplementationParserSideChannel,
    /// Build reproducibility and supply-chain review.
    #[cfg_attr(feature = "json", norito(rename = "build_supply_chain"))]
    BuildSupplyChain,
    /// Deployment topology and resource qualification.
    #[cfg_attr(feature = "json", norito(rename = "deployment_resource"))]
    DeploymentResource,
    /// SDK trust-boundary and secret-locality review.
    #[cfg_attr(feature = "json", norito(rename = "sdk_boundary"))]
    SdkBoundary,
}

impl PrivacyReleaseAuditClassV1 {
    /// Every mandatory audit class in canonical order.
    pub const ALL: [Self; PRIVACY_EXACT12_RELEASE_AUDIT_CLASSES_V1] = [
        Self::Cryptographic,
        Self::ImplementationParserSideChannel,
        Self::BuildSupplyChain,
        Self::DeploymentResource,
        Self::SdkBoundary,
    ];
}

/// Role of a signature authorizing the portable release manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "role", content = "value", deny_unknown_fields)
)]
pub enum PrivacyReleaseSignatureRoleV1 {
    /// Release engineering owner.
    #[cfg_attr(feature = "json", norito(rename = "release_engineering"))]
    ReleaseEngineering,
    /// Cryptographic review owner.
    #[cfg_attr(feature = "json", norito(rename = "cryptographic_review"))]
    CryptographicReview,
    /// Implementation security owner.
    #[cfg_attr(feature = "json", norito(rename = "implementation_security"))]
    ImplementationSecurity,
    /// Build and supply-chain owner.
    #[cfg_attr(feature = "json", norito(rename = "build_supply_chain"))]
    BuildSupplyChain,
    /// Deployment and resource owner.
    #[cfg_attr(feature = "json", norito(rename = "deployment_resource"))]
    DeploymentResource,
    /// SDK boundary owner.
    #[cfg_attr(feature = "json", norito(rename = "sdk_boundary"))]
    SdkBoundary,
}

impl PrivacyReleaseSignatureRoleV1 {
    /// Every required role in canonical order.
    pub const ALL: [Self; 6] = [
        Self::ReleaseEngineering,
        Self::CryptographicReview,
        Self::ImplementationSecurity,
        Self::BuildSupplyChain,
        Self::DeploymentResource,
        Self::SdkBoundary,
    ];
}

/// Clean source, compiler, and lockfile identities used for the release build.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseSourceIdentityV1 {
    /// Digest of the exact source tree.
    pub source_tree_digest: PrivacyReleaseArtifactDigestV1,
    /// True only when the source tree contained no uncommitted changes.
    pub source_tree_clean: bool,
    /// Human-readable pinned toolchain identity.
    pub toolchain_id: String,
    /// Digest of the complete toolchain bundle.
    pub toolchain_digest: PrivacyReleaseArtifactDigestV1,
    /// Digest of the unchanged Cargo lockfile.
    pub cargo_lock_digest: PrivacyReleaseArtifactDigestV1,
}

/// One executable or container image identity.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseExecutableArtifactV1 {
    /// Closed artifact kind.
    pub kind: PrivacyReleaseExecutableKindV1,
    /// Canonical package or image name.
    pub name: String,
    /// Immutable content digest.
    pub artifact_digest: PrivacyReleaseArtifactDigestV1,
}

/// Final binding for one of the twelve protocol engines.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseProtocolBindingV1 {
    /// Exact protocol in catalog order.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact proof-system profile.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Exact native engine.
    pub engine_id: PrivacyEngineIdV1,
    /// Governed parameter-set identifier.
    pub parameter_id: PrivacyParameterIdV1,
    /// Governed parameter digest.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Native verifier digest.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Public-statement schema digest.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Native engine-manifest digest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Complete final security claim.
    pub security_claim: PrivacySecurityClaimV1,
    /// Canonical digest of `security_claim`.
    pub security_claim_digest: PrivacySecurityClaimDigestV1,
}

/// One of the exact 48 protocol-stage receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseStageReceiptV1 {
    /// Exact zero-based position in the 48-stage schedule.
    pub stage_ordinal: u16,
    /// Protocol covered by this receipt.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Mandatory stage covered by this receipt.
    pub stage: PrivacyReleaseStageV1,
    /// Security claim covered by the stage.
    pub security_claim_digest: PrivacySecurityClaimDigestV1,
    /// Parameters used by the stage.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Verifier used by the stage.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Engine used by the stage.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Immutable evidence receipt.
    pub receipt_digest: PrivacyReleaseArtifactDigestV1,
}

/// One of the exact 54 proof artifacts retained by the release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseProofArtifactV1 {
    /// Protocol covered by this proof artifact.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact stage whose production proof bytes this artifact records.
    pub stage: PrivacyReleaseStageV1,
    /// Zero-based contiguous ordinal within the stage.
    pub stage_artifact_ordinal: u8,
    /// Exact stage receipt that authenticates this proof artifact.
    pub stage_receipt_digest: PrivacyReleaseArtifactDigestV1,
    /// Security claim covered by this artifact.
    pub security_claim_digest: PrivacySecurityClaimDigestV1,
    /// Parameters used to produce or verify this artifact.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Verifier used by this artifact.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Engine used by this artifact.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Immutable artifact content digest.
    pub artifact_digest: PrivacyReleaseArtifactDigestV1,
}

/// One exact SDK or tooling package built from the Rust-owned fixture corpus.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseSdkPackageV1 {
    /// Closed consumer identity.
    pub consumer: PrivacyReleaseSdkConsumerV1,
    /// Canonical package name.
    pub package_name: String,
    /// Exact package version.
    pub package_version: String,
    /// Immutable package digest.
    pub package_digest: PrivacyReleaseArtifactDigestV1,
    /// Digest of the unchanged shared fixture corpus consumed by this package.
    pub fixture_corpus_digest: PrivacyReleaseArtifactDigestV1,
}

/// Deterministic output and runtime-self-test result for one hardware backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseHardwareResultV1 {
    /// Closed backend identity.
    pub backend: PrivacyReleaseHardwareBackendV1,
    /// Binary containing the tested path.
    pub tested_binary_digest: PrivacyReleaseArtifactDigestV1,
    /// Digest of the maximum-shape deterministic output.
    pub deterministic_output_digest: PrivacyReleaseArtifactDigestV1,
    /// Scalar output digest used for byte-parity comparison.
    pub scalar_reference_digest: PrivacyReleaseArtifactDigestV1,
    /// Immutable runtime self-test and measurement result.
    pub result_digest: PrivacyReleaseArtifactDigestV1,
    /// True only when runtime self-test succeeded before using the backend.
    pub runtime_self_test_passed: bool,
}

/// Signed acceptance of one remaining Medium audit finding.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyAcceptedMediumDispositionV1 {
    /// Immutable finding digest.
    pub finding_digest: PrivacyReleaseArtifactDigestV1,
    /// Immutable signed disposition digest.
    pub disposition_digest: PrivacyReleaseArtifactDigestV1,
    /// Exact final release artifact set covered by the disposition.
    pub release_artifact_set_digest: PrivacyReleaseArtifactDigestV1,
    /// Audit authority signature over the canonical disposition payload.
    pub signature: Signature,
}

/// One signed independent audit class.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseAuditV1 {
    /// Exact mandatory audit class.
    pub audit_class: PrivacyReleaseAuditClassV1,
    /// Digest of the complete independent report.
    pub report_digest: PrivacyReleaseArtifactDigestV1,
    /// Exact final release artifact set covered by the report.
    pub release_artifact_set_digest: PrivacyReleaseArtifactDigestV1,
    /// Number of open Critical findings; validation requires zero.
    pub open_critical_findings: u16,
    /// Number of open High findings; validation requires zero.
    pub open_high_findings: u16,
    /// Canonically ordered accepted Medium findings and signed dispositions.
    pub accepted_medium_dispositions: Vec<PrivacyAcceptedMediumDispositionV1>,
    /// Independent audit authority.
    pub auditor: PublicKey,
    /// Auditor signature over the canonical report binding.
    pub signature: Signature,
}

/// One role-separated signature over the portable manifest digest.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyReleaseSignatureV1 {
    /// Exact release-approval role.
    pub role: PrivacyReleaseSignatureRoleV1,
    /// Role owner public key.
    pub signer: PublicKey,
    /// Signature over the role and manifest digest.
    pub signature: Signature,
}

/// Portable, self-authenticating first-release Exact12 manifest.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.exact12-release-manifest.v1")]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyExact12ReleaseManifestV1 {
    /// Exact manifest version.
    pub version: u16,
    /// Exact final catalog identity.
    pub catalog_id: String,
    /// Exact final catalog commitment.
    pub catalog_commitment: PrivacyExact12CatalogCommitmentV1,
    /// Clean source, toolchain, and lockfile identity.
    pub source: PrivacyReleaseSourceIdentityV1,
    /// Sole first-release IVM ABI version; validation requires exactly one.
    pub abi_version: u16,
    /// Hash of the complete ABI descriptor for `abi_version`.
    pub abi_hash: PrivacyReleaseArtifactDigestV1,
    /// Digest of the canonical ordered V1 syscall list.
    pub syscall_list_digest: PrivacyReleaseArtifactDigestV1,
    /// Strictly ordered immutable binaries and container images.
    pub executables: Vec<PrivacyReleaseExecutableArtifactV1>,
    /// Exactly twelve protocol bindings in catalog order.
    pub protocols: Vec<PrivacyReleaseProtocolBindingV1>,
    /// Exactly 48 receipts in protocol-major, stage-minor order.
    pub stage_receipts: Vec<PrivacyReleaseStageReceiptV1>,
    /// Exactly 54 proof artifacts in protocol-major, stage-minor, ordinal order.
    pub proof_artifacts: Vec<PrivacyReleaseProofArtifactV1>,
    /// Exact SDK/tooling package matrix in canonical consumer order.
    pub sdk_packages: Vec<PrivacyReleaseSdkPackageV1>,
    /// Exact deterministic hardware matrix in canonical backend order.
    pub hardware_results: Vec<PrivacyReleaseHardwareResultV1>,
    /// Digest of all non-audit release artifacts.
    pub release_artifact_set_digest: PrivacyReleaseArtifactDigestV1,
    /// Exactly five signed independent audit classes.
    pub audits: Vec<PrivacyReleaseAuditV1>,
    /// Canonical digest of the complete audit vector.
    pub audit_bundle_digest: PrivacyAuditBundleDigestV1,
    /// Role-separated signatures in canonical role order.
    pub release_signatures: Vec<PrivacyReleaseSignatureV1>,
    /// Canonical self-digest with this field zeroed and signatures omitted.
    pub manifest_digest: PrivacyExact12ReleaseManifestDigestV1,
}

/// One protocol activation height bound to the deployment transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyDeploymentActivationV1 {
    /// Protocol in canonical catalog order.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Finalized activation height.
    pub activation_height: u64,
}

/// One validator's restart and adversarial-canary evidence.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyDeploymentValidatorCanaryV1 {
    /// Canonical zero-based validator seat.
    pub validator_index: u16,
    /// Validator consensus identity.
    pub validator: PublicKey,
    /// One-based rollout wave; exactly one validator belongs to every wave.
    pub rollout_wave: u8,
    /// Number of restarts in the wave; exactly one is required.
    pub restart_count: u8,
    /// Last finalized height before restart.
    pub pre_restart_height: u64,
    /// First finalized height after restart.
    pub post_restart_height: u64,
    /// Height at which the adversarial canary completed.
    pub canary_height: u64,
    /// Immutable restart/canary evidence digest.
    pub canary_digest: PrivacyReleaseArtifactDigestV1,
    /// Converged state digest observed by this validator.
    pub converged_state_digest: PrivacyReleaseArtifactDigestV1,
    /// Exact Torii/privacy endpoint version observed after restart.
    pub endpoint_version: String,
}

/// One validator signature over a deployment qualification digest.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyDeploymentValidatorSignatureV1 {
    /// Signer's canonical validator seat.
    pub validator_index: u16,
    /// Signature over the seat and deployment digest.
    pub signature: Signature,
}

/// Network-bound four-validator deployment qualification.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.exact12-deployment-qualification.v1")]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyExact12DeploymentQualificationV1 {
    /// Exact qualification version.
    pub version: u16,
    /// Canonical deployment-selected chain identifier.
    pub chain_id: ChainId,
    /// Target genesis-derived network identity.
    pub network_id: NetworkId,
    /// Exact target genesis hash; must equal the network identity bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub genesis_hash: [u8; 32],
    /// Portable release deployed to the target network.
    pub release_manifest_digest: PrivacyExact12ReleaseManifestDigestV1,
    /// Exact activation transaction wire digest.
    pub activation_transaction_digest: PrivacyReleaseArtifactDigestV1,
    /// Exactly twelve activation heights in catalog order.
    pub activations: Vec<PrivacyDeploymentActivationV1>,
    /// Exact four-validator roster digest.
    pub validator_roster_digest: PrivacyReleaseArtifactDigestV1,
    /// Exact endpoint version required from every validator.
    pub endpoint_version: String,
    /// Final height at which all validators converged after all waves.
    pub convergence_height: u64,
    /// Exact common converged state digest.
    pub converged_state_digest: PrivacyReleaseArtifactDigestV1,
    /// Exactly four canaries in validator-seat and rollout-wave order.
    pub validator_canaries: Vec<PrivacyDeploymentValidatorCanaryV1>,
    /// Exactly three distinct validator signatures in ascending seat order.
    pub validator_signatures: Vec<PrivacyDeploymentValidatorSignatureV1>,
    /// Canonical self-digest with this field zeroed and signatures omitted.
    pub qualification_digest: PrivacyExact12DeploymentQualificationDigestV1,
}

/// Complete immutable evidence registered before any Exact12 protocol is usable.
///
/// Keeping the portable release and network deployment manifests together
/// prevents a caller from attaching isolated claim digests to an activation.
/// The record is a singleton in world state and is accepted only after Core
/// validates it against the running chain, genesis, validator roster, ABI, and
/// all twelve committed activations.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.exact12-qualification-record.v1")]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyExact12QualificationRecordV1 {
    /// Full portable release manifest, including all twelve claims and artifacts.
    pub release_manifest: PrivacyExact12ReleaseManifestV1,
    /// Full deployment evidence for the exact target network.
    pub deployment_qualification: PrivacyExact12DeploymentQualificationV1,
}

impl Ord for PrivacyExact12QualificationRecordV1 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

impl PartialOrd for PrivacyExact12QualificationRecordV1 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Encode)]
struct ReleaseSignaturePayloadV1 {
    role: PrivacyReleaseSignatureRoleV1,
    manifest_digest: PrivacyExact12ReleaseManifestDigestV1,
}

#[derive(Encode)]
struct AuditSignaturePayloadV1 {
    audit_class: PrivacyReleaseAuditClassV1,
    report_digest: PrivacyReleaseArtifactDigestV1,
    release_artifact_set_digest: PrivacyReleaseArtifactDigestV1,
    open_critical_findings: u16,
    open_high_findings: u16,
}

#[derive(Encode)]
struct MediumDispositionSignaturePayloadV1 {
    finding_digest: PrivacyReleaseArtifactDigestV1,
    disposition_digest: PrivacyReleaseArtifactDigestV1,
    release_artifact_set_digest: PrivacyReleaseArtifactDigestV1,
}

#[derive(Encode)]
struct DeploymentSignaturePayloadV1 {
    validator_index: u16,
    qualification_digest: PrivacyExact12DeploymentQualificationDigestV1,
}

fn valid_release_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= PRIVACY_EXACT12_RELEASE_TEXT_MAX_BYTES_V1
        && value.trim() == value
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn digest_canonical<T: Encode>(domain: &[u8], value: &T) -> Result<[u8; 32], norito::Error> {
    let encoded = norito::encode_canonical(value)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(
        u64::try_from(encoded.len())
            .expect("Norito output length fits u64 on supported targets")
            .to_le_bytes(),
    );
    hasher.update(encoded);
    Ok(hasher.finalize().into())
}

fn signing_bytes<T: Encode>(domain: &[u8], value: &T) -> Result<Vec<u8>, norito::Error> {
    let encoded = norito::encode_canonical(value)?;
    let mut bytes = Vec::with_capacity(domain.len() + 8 + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(
        &u64::try_from(encoded.len())
            .expect("Norito output length fits u64 on supported targets")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

/// Compute the canonical digest of the ordered ABI-v1 syscall-number list.
///
/// Numbers are bound as a count followed by canonical little-endian `u32`
/// values. Callers must pass the exact sorted list exposed by the compiled IVM.
#[must_use]
pub fn privacy_exact12_syscall_list_digest_v1(syscalls: &[u32]) -> PrivacyReleaseArtifactDigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(RELEASE_SYSCALL_LIST_DIGEST_DOMAIN_V1);
    hasher.update(
        u64::try_from(syscalls.len())
            .expect("the compiled syscall list length fits u64")
            .to_le_bytes(),
    );
    for syscall in syscalls {
        hasher.update(syscall.to_le_bytes());
    }
    PrivacyReleaseArtifactDigestV1::new(hasher.finalize().into())
}

impl PrivacyReleaseSignatureV1 {
    /// Return the exact role-separated bytes that must be signed.
    pub fn signing_bytes(
        role: PrivacyReleaseSignatureRoleV1,
        manifest_digest: PrivacyExact12ReleaseManifestDigestV1,
    ) -> Result<Vec<u8>, norito::Error> {
        signing_bytes(
            RELEASE_SIGNATURE_DOMAIN_V1,
            &ReleaseSignaturePayloadV1 {
                role,
                manifest_digest,
            },
        )
    }
}

impl PrivacyAcceptedMediumDispositionV1 {
    /// Return the exact artifact-bound bytes that the audit authority must sign.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        signing_bytes(
            MEDIUM_DISPOSITION_SIGNATURE_DOMAIN_V1,
            &MediumDispositionSignaturePayloadV1 {
                finding_digest: self.finding_digest,
                disposition_digest: self.disposition_digest,
                release_artifact_set_digest: self.release_artifact_set_digest,
            },
        )
    }
}

impl PrivacyReleaseAuditV1 {
    /// Return the exact release-artifact-bound audit bytes that must be signed.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        signing_bytes(
            AUDIT_SIGNATURE_DOMAIN_V1,
            &AuditSignaturePayloadV1 {
                audit_class: self.audit_class,
                report_digest: self.report_digest,
                release_artifact_set_digest: self.release_artifact_set_digest,
                open_critical_findings: self.open_critical_findings,
                open_high_findings: self.open_high_findings,
            },
        )
    }

    fn validate(
        &self,
        artifact_set_digest: PrivacyReleaseArtifactDigestV1,
    ) -> Result<(), PrivacyReleaseAuditValidationErrorV1> {
        if self.report_digest.is_zero()
            || self.release_artifact_set_digest != artifact_set_digest
            || self.open_critical_findings != 0
            || self.open_high_findings != 0
            || self.accepted_medium_dispositions.len()
                > PRIVACY_EXACT12_MAX_MEDIUM_DISPOSITIONS_PER_AUDIT_V1
        {
            return Err(PrivacyReleaseAuditValidationErrorV1::ReportBinding);
        }
        let mut prior = None;
        let mut disposition_digests = BTreeSet::new();
        for disposition in &self.accepted_medium_dispositions {
            if disposition.finding_digest.is_zero()
                || disposition.disposition_digest.is_zero()
                || disposition.release_artifact_set_digest != artifact_set_digest
                || prior.is_some_and(|value| value >= disposition.finding_digest)
                || !disposition_digests.insert(disposition.disposition_digest)
            {
                return Err(PrivacyReleaseAuditValidationErrorV1::MediumDisposition);
            }
            let bytes = disposition
                .signing_bytes()
                .map_err(|_| PrivacyReleaseAuditValidationErrorV1::Encoding)?;
            disposition
                .signature
                .verify(&self.auditor, &bytes)
                .map_err(|_| PrivacyReleaseAuditValidationErrorV1::MediumSignature)?;
            prior = Some(disposition.finding_digest);
        }
        let bytes = self
            .signing_bytes()
            .map_err(|_| PrivacyReleaseAuditValidationErrorV1::Encoding)?;
        self.signature
            .verify(&self.auditor, &bytes)
            .map_err(|_| PrivacyReleaseAuditValidationErrorV1::AuditSignature)
    }
}

impl PrivacyExact12ReleaseManifestV1 {
    /// Compute the digest of all non-audit release artifacts.
    pub fn computed_release_artifact_set_digest(
        &self,
    ) -> Result<PrivacyReleaseArtifactDigestV1, norito::Error> {
        let mut normalized = self.clone();
        normalized.release_artifact_set_digest = PrivacyReleaseArtifactDigestV1::new([0; 32]);
        normalized.audit_bundle_digest = PrivacyAuditBundleDigestV1::new([0; 32]);
        normalized.audits.clear();
        normalized.release_signatures.clear();
        normalized.manifest_digest = PrivacyExact12ReleaseManifestDigestV1::new([0; 32]);
        for binding in &mut normalized.protocols {
            binding.security_claim.audit_bundle_digest = PrivacyAuditBundleDigestV1::new([0; 32]);
            binding.security_claim_digest = PrivacySecurityClaimDigestV1::new([0; 32]);
        }
        for receipt in &mut normalized.stage_receipts {
            receipt.security_claim_digest = PrivacySecurityClaimDigestV1::new([0; 32]);
        }
        for artifact in &mut normalized.proof_artifacts {
            artifact.security_claim_digest = PrivacySecurityClaimDigestV1::new([0; 32]);
        }
        digest_canonical(RELEASE_ARTIFACT_SET_DIGEST_DOMAIN_V1, &normalized)
            .map(PrivacyReleaseArtifactDigestV1::new)
    }

    /// Compute the canonical digest of all five signed audit records.
    pub fn computed_audit_bundle_digest(
        &self,
    ) -> Result<PrivacyAuditBundleDigestV1, norito::Error> {
        digest_canonical(RELEASE_AUDIT_BUNDLE_DIGEST_DOMAIN_V1, &self.audits)
            .map(PrivacyAuditBundleDigestV1::new)
    }

    /// Compute the portable manifest digest with its self-digest zeroed and approvals omitted.
    pub fn computed_manifest_digest(
        &self,
    ) -> Result<PrivacyExact12ReleaseManifestDigestV1, norito::Error> {
        let mut normalized = self.clone();
        normalized.manifest_digest = PrivacyExact12ReleaseManifestDigestV1::new([0; 32]);
        normalized.release_signatures.clear();
        digest_canonical(RELEASE_MANIFEST_DIGEST_DOMAIN_V1, &normalized)
            .map(PrivacyExact12ReleaseManifestDigestV1::new)
    }

    /// Validate every exact count, ordering, artifact, audit, and signature binding.
    ///
    /// # Errors
    ///
    /// Returns a closed validation error without converting absent evidence into qualification.
    pub fn validate(&self) -> Result<(), PrivacyExact12ReleaseManifestValidationErrorV1> {
        if self.version != PRIVACY_EXACT12_RELEASE_MANIFEST_VERSION_V1 {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::Version);
        }
        if self.catalog_id.as_bytes() != PRIVACY_EXACT12_CATALOG_ID_V1
            || self.catalog_commitment != PrivacyExact12CatalogCommitmentV1::canonical()
        {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::Catalog);
        }
        if !self.source.source_tree_clean
            || self.source.source_tree_digest.is_zero()
            || self.source.toolchain_digest.is_zero()
            || self.source.cargo_lock_digest.is_zero()
            || !valid_release_text(&self.source.toolchain_id)
        {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::SourceIdentity);
        }
        if self.abi_version != PRIVACY_EXACT12_ABI_VERSION_V1
            || self.abi_hash.is_zero()
            || self.syscall_list_digest.is_zero()
            || self.abi_hash == self.syscall_list_digest
        {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::AbiBinding);
        }
        self.validate_executables()?;
        if self.protocols.len() != PrivacyProtocolIdV1::COUNT {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProtocolCount);
        }
        if self.stage_receipts.len() != PRIVACY_EXACT12_RELEASE_STAGE_RECEIPTS_V1 {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::StageReceiptCount);
        }
        if self.proof_artifacts.len() != PRIVACY_EXACT12_RELEASE_PROOF_ARTIFACTS_V1 {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProofArtifactCount);
        }
        if self.sdk_packages.len() != PrivacyReleaseSdkConsumerV1::ALL.len() {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::SdkPackageCount);
        }
        if self.hardware_results.len() != PrivacyReleaseHardwareBackendV1::ALL.len() {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::HardwareResultCount);
        }
        if self.audits.len() != PRIVACY_EXACT12_RELEASE_AUDIT_CLASSES_V1 {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::AuditCount);
        }
        if self.release_signatures.len() != PrivacyReleaseSignatureRoleV1::ALL.len() {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ReleaseSignatureCount);
        }

        for (binding, expected) in self.protocols.iter().zip(PrivacyProtocolIdV1::ALL) {
            if binding.protocol_id != expected
                || binding.proof_system_id != expected.expected_proof_system()
                || binding.engine_id != expected.expected_engine()
                || binding.parameter_id.is_zero()
                || binding.parameter_digest.is_zero()
                || binding.verifier_digest.is_zero()
                || binding.statement_schema_digest.is_zero()
                || binding.engine_manifest_digest.is_zero()
                || binding.security_claim.audit_bundle_digest != self.audit_bundle_digest
                || binding
                    .security_claim
                    .validate_against(expected, binding.parameter_digest, binding.verifier_digest)
                    .is_err()
                || binding.security_claim.computed_digest().ok()
                    != Some(binding.security_claim_digest)
            {
                return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProtocolBinding);
            }
        }
        self.validate_stage_receipts()?;
        self.validate_proof_artifacts()?;
        self.validate_sdk_packages()?;
        self.validate_hardware_results()?;

        if self.release_artifact_set_digest.is_zero()
            || self.computed_release_artifact_set_digest().ok()
                != Some(self.release_artifact_set_digest)
        {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ArtifactSetDigest);
        }
        if self.audit_bundle_digest.is_zero()
            || self.computed_audit_bundle_digest().ok() != Some(self.audit_bundle_digest)
        {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::AuditBundleDigest);
        }
        let mut auditors = BTreeSet::new();
        let mut audit_reports = BTreeSet::new();
        for (audit, expected) in self.audits.iter().zip(PrivacyReleaseAuditClassV1::ALL) {
            if audit.audit_class != expected
                || !auditors.insert(audit.auditor.clone())
                || !audit_reports.insert(audit.report_digest)
                || audit.validate(self.release_artifact_set_digest).is_err()
            {
                return Err(PrivacyExact12ReleaseManifestValidationErrorV1::Audit);
            }
        }
        if self.manifest_digest.is_zero()
            || self.computed_manifest_digest().ok() != Some(self.manifest_digest)
        {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ManifestDigest);
        }
        let mut signers = BTreeSet::new();
        for (approval, expected) in self
            .release_signatures
            .iter()
            .zip(PrivacyReleaseSignatureRoleV1::ALL)
        {
            if approval.role != expected
                || auditors.contains(&approval.signer)
                || !signers.insert(approval.signer.clone())
            {
                return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ReleaseSignature);
            }
            let bytes = PrivacyReleaseSignatureV1::signing_bytes(expected, self.manifest_digest)
                .map_err(|_| PrivacyExact12ReleaseManifestValidationErrorV1::Encoding)?;
            approval
                .signature
                .verify(&approval.signer, &bytes)
                .map_err(|_| PrivacyExact12ReleaseManifestValidationErrorV1::ReleaseSignature)?;
        }
        Ok(())
    }

    fn validate_executables(&self) -> Result<(), PrivacyExact12ReleaseManifestValidationErrorV1> {
        if self.executables.len() < 2 {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ExecutableArtifact);
        }
        let mut has_binary = false;
        let mut has_image = false;
        let mut digests = BTreeSet::new();
        let mut prior: Option<(PrivacyReleaseExecutableKindV1, &str)> = None;
        for artifact in &self.executables {
            let key = (artifact.kind, artifact.name.as_str());
            if !valid_release_text(&artifact.name)
                || artifact.artifact_digest.is_zero()
                || prior.is_some_and(|value| value >= key)
                || !digests.insert(artifact.artifact_digest)
            {
                return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ExecutableArtifact);
            }
            has_binary |= artifact.kind == PrivacyReleaseExecutableKindV1::Binary;
            has_image |= artifact.kind == PrivacyReleaseExecutableKindV1::ContainerImage;
            prior = Some(key);
        }
        if !has_binary || !has_image {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ExecutableArtifact);
        }
        Ok(())
    }

    fn validate_stage_receipts(
        &self,
    ) -> Result<(), PrivacyExact12ReleaseManifestValidationErrorV1> {
        let mut receipt_digests = BTreeSet::new();
        for (index, receipt) in self.stage_receipts.iter().enumerate() {
            let binding = &self.protocols[index / PRIVACY_EXACT12_RELEASE_STAGES_PER_PROTOCOL_V1];
            let expected_stage =
                PrivacyReleaseStageV1::ALL[index % PRIVACY_EXACT12_RELEASE_STAGES_PER_PROTOCOL_V1];
            let expected_ordinal = u16::try_from(index).expect("48 release stages fit u16");
            if receipt.stage_ordinal != expected_ordinal
                || receipt.protocol_id != binding.protocol_id
                || receipt.stage != expected_stage
                || receipt.security_claim_digest != binding.security_claim_digest
                || receipt.parameter_digest != binding.parameter_digest
                || receipt.verifier_digest != binding.verifier_digest
                || receipt.engine_manifest_digest != binding.engine_manifest_digest
                || receipt.receipt_digest.is_zero()
                || !receipt_digests.insert(receipt.receipt_digest)
            {
                return Err(PrivacyExact12ReleaseManifestValidationErrorV1::StageReceipt);
            }
        }
        Ok(())
    }

    fn validate_proof_artifacts(
        &self,
    ) -> Result<(), PrivacyExact12ReleaseManifestValidationErrorV1> {
        let mut digests = BTreeSet::new();
        let mut artifact_index = 0_usize;
        for (protocol_index, protocol_id) in PrivacyProtocolIdV1::ALL.into_iter().enumerate() {
            let binding = &self.protocols[protocol_index];
            for (stage_index, stage) in PrivacyReleaseStageV1::ALL.into_iter().enumerate() {
                let receipt_index = protocol_index
                    .checked_mul(PRIVACY_EXACT12_RELEASE_STAGES_PER_PROTOCOL_V1)
                    .and_then(|base| base.checked_add(stage_index))
                    .expect("the fixed Exact12 stage index cannot overflow");
                let receipt = &self.stage_receipts[receipt_index];
                let count = privacy_exact12_release_proof_artifact_count_v1(protocol_id, stage);
                for stage_artifact_ordinal in 0..count {
                    let artifact = &self.proof_artifacts[artifact_index];
                    if artifact.protocol_id != protocol_id
                        || artifact.stage != stage
                        || artifact.stage_artifact_ordinal != stage_artifact_ordinal
                        || artifact.stage_receipt_digest != receipt.receipt_digest
                        || artifact.security_claim_digest != binding.security_claim_digest
                        || artifact.parameter_digest != binding.parameter_digest
                        || artifact.verifier_digest != binding.verifier_digest
                        || artifact.engine_manifest_digest != binding.engine_manifest_digest
                        || artifact.artifact_digest.is_zero()
                        || !digests.insert(artifact.artifact_digest)
                    {
                        return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProofArtifact);
                    }
                    artifact_index = artifact_index
                        .checked_add(1)
                        .ok_or(PrivacyExact12ReleaseManifestValidationErrorV1::ProofArtifact)?;
                }
            }
        }
        if artifact_index != self.proof_artifacts.len() {
            return Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProofArtifact);
        }
        Ok(())
    }

    fn validate_sdk_packages(&self) -> Result<(), PrivacyExact12ReleaseManifestValidationErrorV1> {
        let mut fixture_digest = None;
        let mut package_digests = BTreeSet::new();
        for (package, expected) in self
            .sdk_packages
            .iter()
            .zip(PrivacyReleaseSdkConsumerV1::ALL)
        {
            if package.consumer != expected
                || !valid_release_text(&package.package_name)
                || !valid_release_text(&package.package_version)
                || package.package_digest.is_zero()
                || package.fixture_corpus_digest.is_zero()
                || fixture_digest.is_some_and(|digest| digest != package.fixture_corpus_digest)
                || !package_digests.insert(package.package_digest)
            {
                return Err(PrivacyExact12ReleaseManifestValidationErrorV1::SdkPackage);
            }
            fixture_digest = Some(package.fixture_corpus_digest);
        }
        Ok(())
    }

    fn validate_hardware_results(
        &self,
    ) -> Result<(), PrivacyExact12ReleaseManifestValidationErrorV1> {
        let binary_digests = self
            .executables
            .iter()
            .filter(|artifact| artifact.kind == PrivacyReleaseExecutableKindV1::Binary)
            .map(|artifact| artifact.artifact_digest)
            .collect::<BTreeSet<_>>();
        let mut scalar_reference = None;
        let mut result_digests = BTreeSet::new();
        for (result, expected) in self
            .hardware_results
            .iter()
            .zip(PrivacyReleaseHardwareBackendV1::ALL)
        {
            if result.backend != expected
                || !result.runtime_self_test_passed
                || !binary_digests.contains(&result.tested_binary_digest)
                || result.deterministic_output_digest.is_zero()
                || result.scalar_reference_digest.is_zero()
                || result.result_digest.is_zero()
                || result.deterministic_output_digest != result.scalar_reference_digest
                || scalar_reference.is_some_and(|digest| digest != result.scalar_reference_digest)
                || !result_digests.insert(result.result_digest)
            {
                return Err(PrivacyExact12ReleaseManifestValidationErrorV1::HardwareResult);
            }
            scalar_reference = Some(result.scalar_reference_digest);
        }
        Ok(())
    }
}

impl PrivacyDeploymentValidatorSignatureV1 {
    /// Return the exact seat-bound bytes that a deployment validator must sign.
    pub fn signing_bytes(
        validator_index: u16,
        qualification_digest: PrivacyExact12DeploymentQualificationDigestV1,
    ) -> Result<Vec<u8>, norito::Error> {
        signing_bytes(
            DEPLOYMENT_SIGNATURE_DOMAIN_V1,
            &DeploymentSignaturePayloadV1 {
                validator_index,
                qualification_digest,
            },
        )
    }
}

impl PrivacyExact12DeploymentQualificationV1 {
    /// Compute the exact ordered validator-roster digest.
    pub fn computed_validator_roster_digest(
        &self,
    ) -> Result<PrivacyReleaseArtifactDigestV1, norito::Error> {
        let roster = self
            .validator_canaries
            .iter()
            .map(|canary| canary.validator.clone())
            .collect::<Vec<_>>();
        digest_canonical(DEPLOYMENT_ROSTER_DIGEST_DOMAIN_V1, &roster)
            .map(PrivacyReleaseArtifactDigestV1::new)
    }

    /// Compute the deployment digest with its self-digest zeroed and quorum signatures omitted.
    pub fn computed_qualification_digest(
        &self,
    ) -> Result<PrivacyExact12DeploymentQualificationDigestV1, norito::Error> {
        let mut normalized = self.clone();
        normalized.qualification_digest =
            PrivacyExact12DeploymentQualificationDigestV1::new([0; 32]);
        normalized.validator_signatures.clear();
        digest_canonical(DEPLOYMENT_QUALIFICATION_DIGEST_DOMAIN_V1, &normalized)
            .map(PrivacyExact12DeploymentQualificationDigestV1::new)
    }

    /// Validate the exact network, activation, restart, convergence, endpoint, and quorum bindings.
    ///
    /// # Errors
    ///
    /// Returns a closed error for any missing, duplicated, reordered, zero, or invalid binding.
    pub fn validate(&self) -> Result<(), PrivacyExact12DeploymentQualificationValidationErrorV1> {
        if self.version != PRIVACY_EXACT12_DEPLOYMENT_QUALIFICATION_VERSION_V1 {
            return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::Version);
        }
        if self.network_id.as_bytes() != &self.genesis_hash {
            return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::NetworkGenesis);
        }
        if self.release_manifest_digest.is_zero()
            || self.activation_transaction_digest.is_zero()
            || self.validator_roster_digest.is_zero()
            || self.converged_state_digest.is_zero()
            || !valid_release_text(&self.endpoint_version)
            || self.convergence_height == 0
        {
            return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::DeploymentBinding);
        }
        if self.activations.len() != PrivacyProtocolIdV1::COUNT {
            return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::ActivationCount);
        }
        let mut latest_activation_height = 0_u64;
        for (activation, expected) in self.activations.iter().zip(PrivacyProtocolIdV1::ALL) {
            if activation.protocol_id != expected
                || activation.activation_height == 0
                || activation.activation_height >= self.convergence_height
            {
                return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::Activation);
            }
            latest_activation_height = latest_activation_height.max(activation.activation_height);
        }
        if self.validator_canaries.len() != PRIVACY_EXACT12_DEPLOYMENT_VALIDATORS_V1 {
            return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::CanaryCount);
        }
        let mut validators = BTreeSet::new();
        let mut prior_wave_height = latest_activation_height;
        for (index, canary) in self.validator_canaries.iter().enumerate() {
            let expected_index = u16::try_from(index).expect("four validators fit u16");
            let expected_wave = u8::try_from(index + 1).expect("four rollout waves fit u8");
            if canary.validator_index != expected_index
                || canary.rollout_wave != expected_wave
                || canary.restart_count != 1
                || canary.pre_restart_height < prior_wave_height
                || canary.post_restart_height <= canary.pre_restart_height
                || canary.canary_height < canary.post_restart_height
                || canary.canary_height > self.convergence_height
                || canary.canary_digest.is_zero()
                || canary.converged_state_digest != self.converged_state_digest
                || canary.endpoint_version != self.endpoint_version
                || !validators.insert(canary.validator.clone())
            {
                return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::Canary);
            }
            prior_wave_height = canary.canary_height;
        }
        if self.computed_validator_roster_digest().ok() != Some(self.validator_roster_digest) {
            return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::RosterDigest);
        }
        if self.validator_signatures.len() != PRIVACY_EXACT12_DEPLOYMENT_SIGNATURES_V1 {
            return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::SignatureCount);
        }
        if self.qualification_digest.is_zero()
            || self.computed_qualification_digest().ok() != Some(self.qualification_digest)
        {
            return Err(
                PrivacyExact12DeploymentQualificationValidationErrorV1::QualificationDigest,
            );
        }
        let mut prior_signer = None;
        for signed in &self.validator_signatures {
            let index = usize::from(signed.validator_index);
            if index >= self.validator_canaries.len()
                || prior_signer.is_some_and(|prior| prior >= signed.validator_index)
            {
                return Err(PrivacyExact12DeploymentQualificationValidationErrorV1::Signature);
            }
            let bytes = PrivacyDeploymentValidatorSignatureV1::signing_bytes(
                signed.validator_index,
                self.qualification_digest,
            )
            .map_err(|_| PrivacyExact12DeploymentQualificationValidationErrorV1::Encoding)?;
            signed
                .signature
                .verify(&self.validator_canaries[index].validator, &bytes)
                .map_err(|_| PrivacyExact12DeploymentQualificationValidationErrorV1::Signature)?;
            prior_signer = Some(signed.validator_index);
        }
        Ok(())
    }
}

impl PrivacyExact12QualificationRecordV1 {
    /// Validate both manifests and their immutable cross-manifest link.
    ///
    /// Core performs the additional state-dependent checks against the target
    /// chain, compiled ABI/profile catalog, validator topology, and activation
    /// heights before storing this record.
    ///
    /// # Errors
    ///
    /// Rejects an invalid release, invalid deployment, or a deployment that
    /// names a different release digest.
    pub fn validate(&self) -> Result<(), PrivacyExact12QualificationRecordValidationErrorV1> {
        self.release_manifest
            .validate()
            .map_err(PrivacyExact12QualificationRecordValidationErrorV1::ReleaseManifest)?;
        self.deployment_qualification
            .validate()
            .map_err(PrivacyExact12QualificationRecordValidationErrorV1::DeploymentQualification)?;
        if self.deployment_qualification.release_manifest_digest
            != self.release_manifest.manifest_digest
        {
            return Err(PrivacyExact12QualificationRecordValidationErrorV1::ReleaseManifestDigest);
        }
        Ok(())
    }

    /// Validate this evidence against the complete committed capability snapshot.
    ///
    /// # Errors
    ///
    /// Rejects evidence that predates convergence, omits an active Exact12
    /// activation, or differs from any compiled/governed protocol tuple or
    /// finalized activation height.
    pub fn validate_against_snapshot(
        &self,
        committed_height: u64,
        protocols: &[PrivacyCapabilityRowV1],
    ) -> Result<(), PrivacyExact12QualificationRecordValidationErrorV1> {
        self.validate()?;
        if self.deployment_qualification.convergence_height > committed_height {
            return Err(PrivacyExact12QualificationRecordValidationErrorV1::NotConverged);
        }
        if protocols.len() != PrivacyProtocolIdV1::COUNT {
            return Err(PrivacyExact12QualificationRecordValidationErrorV1::ProtocolCount);
        }
        for (index, protocol_id) in PrivacyProtocolIdV1::ALL.into_iter().enumerate() {
            let row = &protocols[index];
            row.validate_at_committed_height(committed_height)
                .map_err(|_| PrivacyExact12QualificationRecordValidationErrorV1::ProtocolBinding)?;
            self.validate_protocol_binding(index, protocol_id, row)?;
        }
        Ok(())
    }

    /// Validate qualification for one committed capability row.
    ///
    /// # Errors
    ///
    /// Rejects invalid global evidence, pre-convergence state, or a protocol
    /// binding that differs from the exact compiled profile, activation, or
    /// deployment height.
    pub fn validate_protocol_at_snapshot(
        &self,
        committed_height: u64,
        row: &PrivacyCapabilityRowV1,
    ) -> Result<(), PrivacyExact12QualificationRecordValidationErrorV1> {
        self.validate()?;
        if self.deployment_qualification.convergence_height > committed_height {
            return Err(PrivacyExact12QualificationRecordValidationErrorV1::NotConverged);
        }
        row.validate_at_committed_height(committed_height)
            .map_err(|_| PrivacyExact12QualificationRecordValidationErrorV1::ProtocolBinding)?;
        let index = PrivacyProtocolIdV1::ALL
            .iter()
            .position(|protocol_id| *protocol_id == row.protocol_id)
            .ok_or(PrivacyExact12QualificationRecordValidationErrorV1::ProtocolBinding)?;
        self.validate_protocol_binding(index, row.protocol_id, row)
    }

    fn validate_protocol_binding(
        &self,
        index: usize,
        protocol_id: PrivacyProtocolIdV1,
        row: &PrivacyCapabilityRowV1,
    ) -> Result<(), PrivacyExact12QualificationRecordValidationErrorV1> {
        let release = &self.release_manifest.protocols[index];
        let deployment = &self.deployment_qualification.activations[index];
        let PrivacyCompiledProfileResultV1::Available(profile) = row.compiled_profile else {
            return Err(PrivacyExact12QualificationRecordValidationErrorV1::CompiledProfile);
        };
        let Some(activation) = row.activation else {
            return Err(PrivacyExact12QualificationRecordValidationErrorV1::Activation);
        };
        let PrivacyProtocolLifecycleV1::Active(lifecycle) = activation.lifecycle else {
            return Err(PrivacyExact12QualificationRecordValidationErrorV1::Activation);
        };
        if row.protocol_id != protocol_id
            || release.protocol_id != protocol_id
            || deployment.protocol_id != protocol_id
            || release.proof_system_id != activation.proof_system_id
            || release.proof_system_id != profile.proof_system_id
            || release.engine_id != activation.engine_id
            || release.engine_id != profile.engine_id
            || release.parameter_id != activation.parameter_id
            || release.parameter_id != profile.parameter_id
            || release.parameter_digest != activation.parameter_digest
            || release.parameter_digest != profile.parameter_digest
            || release.verifier_digest != activation.verifier_digest
            || release.verifier_digest != profile.verifier_digest
            || release.statement_schema_digest != activation.statement_schema_digest
            || release.statement_schema_digest != profile.statement_schema_digest
            || release.engine_manifest_digest != activation.engine_manifest_digest
            || release.engine_manifest_digest != profile.engine_manifest_digest
            || deployment.activation_height != lifecycle.activated_at_height
        {
            return Err(PrivacyExact12QualificationRecordValidationErrorV1::ProtocolBinding);
        }
        Ok(())
    }
}

/// Validation failure for one signed audit class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyReleaseAuditValidationErrorV1 {
    /// Report, artifact, severity, or disposition bound is invalid.
    #[error("privacy release audit report binding is invalid")]
    ReportBinding,
    /// Medium findings are zero, duplicated, reordered, or bound to another artifact set.
    #[error("privacy release Medium disposition is invalid")]
    MediumDisposition,
    /// Canonical signature payload encoding failed.
    #[error("privacy release audit signature payload encoding failed")]
    Encoding,
    /// A Medium disposition signature is invalid.
    #[error("privacy release Medium disposition signature is invalid")]
    MediumSignature,
    /// The audit report signature is invalid.
    #[error("privacy release audit report signature is invalid")]
    AuditSignature,
}

/// Validation failure for a portable Exact12 release manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyExact12ReleaseManifestValidationErrorV1 {
    /// Manifest version is not v1.
    #[error("privacy Exact12 release manifest version is not v1")]
    Version,
    /// Catalog identity or commitment is not the final Exact12 value.
    #[error("privacy Exact12 release catalog binding is invalid")]
    Catalog,
    /// Source tree, toolchain, or lockfile identity is invalid.
    #[error("privacy Exact12 release source identity is invalid")]
    SourceIdentity,
    /// ABI version, ABI hash, or ordered syscall-list digest is invalid.
    #[error("privacy Exact12 release ABI binding is invalid")]
    AbiBinding,
    /// Executable artifacts are absent, duplicated, unordered, or invalid.
    #[error("privacy Exact12 release executable artifact set is invalid")]
    ExecutableArtifact,
    /// Protocol binding count is not exactly twelve.
    #[error("privacy Exact12 release must contain exactly twelve protocol bindings")]
    ProtocolCount,
    /// One protocol binding or security claim is invalid.
    #[error("privacy Exact12 release protocol binding is invalid")]
    ProtocolBinding,
    /// Stage receipt count is not exactly 48.
    #[error("privacy Exact12 release must contain exactly 48 stage receipts")]
    StageReceiptCount,
    /// One stage receipt is duplicated, reordered, or cross-bound incorrectly.
    #[error("privacy Exact12 release stage receipt is invalid")]
    StageReceipt,
    /// Proof artifact count is not exactly 54.
    #[error("privacy Exact12 release must contain exactly 54 proof artifacts")]
    ProofArtifactCount,
    /// One proof artifact is duplicated, reordered, or cross-bound incorrectly.
    #[error("privacy Exact12 release proof artifact is invalid")]
    ProofArtifact,
    /// SDK package count differs from the closed consumer matrix.
    #[error("privacy Exact12 release SDK package count is invalid")]
    SdkPackageCount,
    /// One SDK package or shared fixture binding is invalid.
    #[error("privacy Exact12 release SDK package is invalid")]
    SdkPackage,
    /// Hardware result count differs from the closed backend matrix.
    #[error("privacy Exact12 release hardware result count is invalid")]
    HardwareResultCount,
    /// One hardware self-test or byte-parity result is invalid.
    #[error("privacy Exact12 release hardware result is invalid")]
    HardwareResult,
    /// Non-audit artifact-set digest is zero or inconsistent.
    #[error("privacy Exact12 release artifact-set digest is invalid")]
    ArtifactSetDigest,
    /// Audit count is not exactly five.
    #[error("privacy Exact12 release must contain exactly five audit classes")]
    AuditCount,
    /// One audit, auditor, finding, or signature is invalid.
    #[error("privacy Exact12 release audit is invalid")]
    Audit,
    /// Audit bundle digest is zero or inconsistent.
    #[error("privacy Exact12 release audit-bundle digest is invalid")]
    AuditBundleDigest,
    /// Release signature count differs from the closed role matrix.
    #[error("privacy Exact12 release signature count is invalid")]
    ReleaseSignatureCount,
    /// A release role, signer separation, or signature is invalid.
    #[error("privacy Exact12 release role signature is invalid")]
    ReleaseSignature,
    /// Manifest self-digest is zero or inconsistent.
    #[error("privacy Exact12 release manifest digest is invalid")]
    ManifestDigest,
    /// Canonical signature or content encoding failed.
    #[error("privacy Exact12 release canonical encoding failed")]
    Encoding,
}

/// Validation failure for a network-bound deployment qualification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyExact12DeploymentQualificationValidationErrorV1 {
    /// Qualification version is not v1.
    #[error("privacy Exact12 deployment qualification version is not v1")]
    Version,
    /// Network identity and genesis hash differ.
    #[error("privacy Exact12 deployment network/genesis binding is invalid")]
    NetworkGenesis,
    /// Release, activation, endpoint, convergence, or roster binding is absent.
    #[error("privacy Exact12 deployment binding is invalid")]
    DeploymentBinding,
    /// Activation count is not exactly twelve.
    #[error("privacy Exact12 deployment must bind exactly twelve activations")]
    ActivationCount,
    /// Activation order or height is invalid.
    #[error("privacy Exact12 deployment activation binding is invalid")]
    Activation,
    /// Canary count is not exactly four.
    #[error("privacy Exact12 deployment must contain exactly four validator canaries")]
    CanaryCount,
    /// A validator restart, canary, convergence, endpoint, or wave binding is invalid.
    #[error("privacy Exact12 deployment validator canary is invalid")]
    Canary,
    /// Validator roster digest differs from the exact ordered canary roster.
    #[error("privacy Exact12 deployment validator roster digest is invalid")]
    RosterDigest,
    /// Validator signature count is not exactly three.
    #[error("privacy Exact12 deployment must contain exactly three validator signatures")]
    SignatureCount,
    /// Deployment self-digest is zero or inconsistent.
    #[error("privacy Exact12 deployment qualification digest is invalid")]
    QualificationDigest,
    /// A validator seat, ordering, or signature is invalid.
    #[error("privacy Exact12 deployment validator signature is invalid")]
    Signature,
    /// Canonical signature or content encoding failed.
    #[error("privacy Exact12 deployment canonical encoding failed")]
    Encoding,
}

/// Validation failure for the complete Exact12 qualification singleton.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyExact12QualificationRecordValidationErrorV1 {
    /// The portable release manifest is invalid.
    #[error("privacy Exact12 qualification release manifest is invalid: {0}")]
    ReleaseManifest(PrivacyExact12ReleaseManifestValidationErrorV1),
    /// The network-bound deployment qualification is invalid.
    #[error("privacy Exact12 qualification deployment evidence is invalid: {0}")]
    DeploymentQualification(PrivacyExact12DeploymentQualificationValidationErrorV1),
    /// Deployment evidence names a different portable release.
    #[error("privacy Exact12 qualification release digest does not match deployment evidence")]
    ReleaseManifestDigest,
    /// Deployment convergence has not reached the committed snapshot height.
    #[error("privacy Exact12 qualification has not converged at the committed height")]
    NotConverged,
    /// Snapshot row count differs from the closed Exact12 catalog.
    #[error("privacy Exact12 qualification snapshot must contain exactly twelve rows")]
    ProtocolCount,
    /// At least one locally compiled profile is unavailable.
    #[error("privacy Exact12 qualification requires every compiled profile")]
    CompiledProfile,
    /// At least one Exact12 activation is absent or not active.
    #[error("privacy Exact12 qualification requires every activation to be active")]
    Activation,
    /// A release, compiled-profile, activation, or deployment tuple differs.
    #[error("privacy Exact12 qualification protocol binding is invalid")]
    ProtocolBinding,
}
