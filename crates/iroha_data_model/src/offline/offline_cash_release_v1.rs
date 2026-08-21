//! Authenticated artifact and internal-validation contract for offline cash V1.

use super::{
    OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1, OFFLINE_CASH_SESSION_TARGET_BYTES_V1,
    OFFLINE_CASH_WIRE_VERSION_V1,
};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Fixed Halo2 domain exponent for every offline-cash proof role.
pub const OFFLINE_CASH_HALO2_K_V1: u32 = 16;
/// Exact serialized transparent IPA parameters for either Pasta parity.
pub const OFFLINE_CASH_PARAMS_BYTES_V1: u64 = 4_194_372;
/// Maximum processed state proving-key bytes for either parity.
pub const OFFLINE_CASH_STATE_PROVING_KEY_MAX_BYTES_V1: u64 = 48_234_934;
/// Maximum processed helper proving-key bytes for either parity.
pub const OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Maximum processed verifying-key bytes for one role and parity.
pub const OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1: u64 = 64 * 1024;
/// Maximum complete preinstalled offline artifact package.
pub const OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1: u64 = 512 * 1024 * 1024;
/// Maximum whole-process resident memory during proving or verification.
pub const OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1: u64 = 128 * 1024 * 1024;
/// Minimum distinct reproducible builds required by validation.
pub const OFFLINE_CASH_REPRODUCIBLE_BUILD_COUNT_V1: u8 = 2;
/// Minimum alternating send/receive handoffs in the invariant-size KAT.
pub const OFFLINE_CASH_MIN_QUALIFIED_HANDOFFS_V1: u32 = 1_024;
/// Minimum parser-fuzz cases represented by the validation report.
pub const OFFLINE_CASH_MIN_FUZZ_CASES_V1: u64 = 10_000_000;
/// Balanced-baseline p95 proving ceiling in milliseconds.
pub const OFFLINE_CASH_PROVE_P95_MAX_MS_V1: u32 = 10_000;
/// Balanced-baseline p95 verification ceiling in milliseconds.
pub const OFFLINE_CASH_VERIFY_P95_MAX_MS_V1: u32 = 1_000;
/// Balanced-baseline p95 complete handoff ceiling in milliseconds.
pub const OFFLINE_CASH_HANDOFF_P95_MAX_MS_V1: u32 = 30_000;
/// Required validator count for activation/restart/replay qualification.
pub const OFFLINE_CASH_VALIDATOR_COUNT_V1: u8 = 4;
/// Maximum trusted authorities in one locally configured release policy.
pub const OFFLINE_CASH_RELEASE_AUTHORITY_MAX_SIGNERS_V1: usize = 32;
/// Maximum canonical bytes accepted for one Offline Cash V1 release manifest.
pub const OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1: usize = 16 * 1024;
/// Maximum canonical bytes accepted for one locally selected authority policy.
pub const OFFLINE_CASH_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1: usize = 512 * 1024;
/// Maximum canonical bytes accepted for one threshold release attestation.
pub const OFFLINE_CASH_RELEASE_ATTESTATION_MAX_BYTES_V1: usize = 1024 * 1024;

const ARTIFACT_SET_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:artifact-set";
const RECEIPT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:validation-receipt";
const RELEASE_ID_DOMAIN: &[u8] = b"iroha:offline-cash:v1:release";
const MANIFEST_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:manifest";
const AUTHORITY_POLICY_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:release-authority-policy";
const RELEASE_APPROVAL_DOMAIN: &str = "iroha:offline-cash:v1:release-approval";
const RELEASE_ATTESTATION_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:release-attestation";

/// Canonical role of one preinstalled transparent Halo2 artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "role", content = "value", rename_all = "snake_case")]
#[repr(u8)]
pub enum OfflineCashArtifactRoleV1 {
    /// Eq/Fp transparent IPA parameters.
    ParamsEq,
    /// Ep/Fq transparent IPA parameters.
    ParamsEp,
    /// Eq/Fp state proving key.
    StatePkEq,
    /// Eq/Fp state verifying key.
    StateVkEq,
    /// Ep/Fq state proving key.
    StatePkEp,
    /// Ep/Fq state verifying key.
    StateVkEp,
    /// Eq/Fp GuardUse proving key.
    GuardUsePkEq,
    /// Eq/Fp GuardUse verifying key.
    GuardUseVkEq,
    /// Ep/Fq GuardUse proving key.
    GuardUsePkEp,
    /// Ep/Fq GuardUse verifying key.
    GuardUseVkEp,
    /// Eq/Fp PlatformBind proving key.
    PlatformBindPkEq,
    /// Eq/Fp PlatformBind verifying key.
    PlatformBindVkEq,
    /// Ep/Fq PlatformBind proving key.
    PlatformBindPkEp,
    /// Ep/Fq PlatformBind verifying key.
    PlatformBindVkEp,
    /// Eq/Fp AndroidKeyCert proving key.
    AndroidKeyCertPkEq,
    /// Eq/Fp AndroidKeyCert verifying key.
    AndroidKeyCertVkEq,
    /// Ep/Fq AndroidKeyCert proving key.
    AndroidKeyCertPkEp,
    /// Ep/Fq AndroidKeyCert verifying key.
    AndroidKeyCertVkEp,
    /// Eq/Fp GuardBundle proving key.
    GuardBundlePkEq,
    /// Eq/Fp GuardBundle verifying key.
    GuardBundleVkEq,
    /// Ep/Fq GuardBundle proving key.
    GuardBundlePkEp,
    /// Ep/Fq GuardBundle verifying key.
    GuardBundleVkEp,
}

impl OfflineCashArtifactRoleV1 {
    /// Exact canonically ordered release inventory.
    pub const ALL: [Self; 22] = [
        Self::ParamsEq,
        Self::ParamsEp,
        Self::StatePkEq,
        Self::StateVkEq,
        Self::StatePkEp,
        Self::StateVkEp,
        Self::GuardUsePkEq,
        Self::GuardUseVkEq,
        Self::GuardUsePkEp,
        Self::GuardUseVkEp,
        Self::PlatformBindPkEq,
        Self::PlatformBindVkEq,
        Self::PlatformBindPkEp,
        Self::PlatformBindVkEp,
        Self::AndroidKeyCertPkEq,
        Self::AndroidKeyCertVkEq,
        Self::AndroidKeyCertPkEp,
        Self::AndroidKeyCertVkEp,
        Self::GuardBundlePkEq,
        Self::GuardBundleVkEq,
        Self::GuardBundlePkEp,
        Self::GuardBundleVkEp,
    ];

    const fn is_params(self) -> bool {
        matches!(self, Self::ParamsEq | Self::ParamsEp)
    }

    const fn is_state_pk(self) -> bool {
        matches!(self, Self::StatePkEq | Self::StatePkEp)
    }

    const fn is_helper_pk(self) -> bool {
        matches!(
            self,
            Self::GuardUsePkEq
                | Self::GuardUsePkEp
                | Self::PlatformBindPkEq
                | Self::PlatformBindPkEp
                | Self::AndroidKeyCertPkEq
                | Self::AndroidKeyCertPkEp
                | Self::GuardBundlePkEq
                | Self::GuardBundlePkEp
        )
    }

    const fn is_vk(self) -> bool {
        matches!(
            self,
            Self::StateVkEq
                | Self::StateVkEp
                | Self::GuardUseVkEq
                | Self::GuardUseVkEp
                | Self::PlatformBindVkEq
                | Self::PlatformBindVkEp
                | Self::AndroidKeyCertVkEq
                | Self::AndroidKeyCertVkEp
                | Self::GuardBundleVkEq
                | Self::GuardBundleVkEp
        )
    }
}

/// Digest and byte length of one authenticated artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineCashArtifactBindingV1 {
    /// Artifact role.
    pub role: OfflineCashArtifactRoleV1,
    /// SHA-256 of the exact file bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sha256: [u8; 32],
    /// Exact file length.
    pub byte_len: u64,
}

/// Evidence produced by the internal release-qualification pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashInternalValidationReceiptV1 {
    /// Receipt version.
    pub version: u16,
    /// SHA-256 identity of the reviewed source tree and commit metadata.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub source_tree_digest: [u8; 32],
    /// SHA-256 of the unchanged root `Cargo.lock`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub cargo_lock_digest: [u8; 32],
    /// Exact circuit/profile digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub profile_digest: [u8; 32],
    /// Compiled Eq/Fp protocol digest exercised by qualification.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep/Fq protocol digest exercised by qualification.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Digest of the canonically ordered artifact inventory.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_set_digest: [u8; 32],
    /// Authenticated hardware allowlist/policy root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_policy_digest: [u8; 32],
    /// Circuit-shape synthesis report digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub circuit_shape_report_digest: [u8; 32],
    /// Cryptographic review report digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub security_review_digest: [u8; 32],
    /// Positive and adversarial known-answer report digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub kat_report_digest: [u8; 32],
    /// Parser and topology fuzz report digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub fuzz_report_digest: [u8; 32],
    /// Whole-process resource report digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub resource_report_digest: [u8; 32],
    /// Physical iPhone 12/iOS 15 qualification report digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ios_device_report_digest: [u8; 32],
    /// Physical eligible Pixel 6/Android 12 qualification report digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub android_device_report_digest: [u8; 32],
    /// Four-validator activation/restart/replay report digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub four_peer_report_digest: [u8; 32],
    /// Largest observed Eq/Ep current-proof pair.
    pub max_proof_pair_bytes: u32,
    /// Largest observed complete raw handoff session.
    pub max_session_bytes: u32,
    /// Largest observed whole-process RSS.
    pub max_process_rss_bytes: u64,
    /// Slowest qualifying p95 proof generation.
    pub prove_p95_ms: u32,
    /// Slowest qualifying p95 proof verification.
    pub verify_p95_ms: u32,
    /// Slowest qualifying p95 complete handoff.
    pub handoff_p95_ms: u32,
    /// Alternating offline handoffs completed with invariant wire size.
    pub qualified_handoffs: u32,
    /// Bounded decoder fuzz cases represented by the report.
    pub fuzz_cases: u64,
    /// Clean builds that reproduced every governed artifact byte-for-byte.
    pub reproducible_builds: u8,
    /// Validators used by activation/restart/replay qualification.
    pub validator_count: u8,
}

/// Canonical release manifest accepted by offline-cash runtime code.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashReleaseManifestV1 {
    /// Manifest version.
    pub version: u16,
    /// Digest-derived release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// SHA-256 identity of the reviewed source tree and commit metadata.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub source_tree_digest: [u8; 32],
    /// SHA-256 of the unchanged root `Cargo.lock`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub cargo_lock_digest: [u8; 32],
    /// Exact circuit/profile digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub profile_digest: [u8; 32],
    /// Compiled Eq/Fp protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep/Fq protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Authenticated hardware allowlist/policy root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_policy_digest: [u8; 32],
    /// SHA-256 of the canonical internal-validation receipt.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub validation_receipt_digest: [u8; 32],
    /// Fixed Halo2 domain exponent.
    pub halo2_k: u32,
    /// Canonically ordered complete artifact inventory.
    pub artifacts: Vec<OfflineCashArtifactBindingV1>,
}

/// Locally trusted threshold policy for Offline Cash V1 release authorities.
///
/// This policy is deployment configuration, not evidence supplied by an
/// untrusted release bundle. Callers must select the trusted policy before
/// authenticating any manifest or attestation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashReleaseAuthorityPolicyV1 {
    /// Policy format version.
    pub version: u16,
    /// Deployment-selected identity for this authority set.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub authority_set_id: [u8; 32],
    /// Minimum number of distinct authorized approvals.
    pub threshold: u16,
    /// Strictly ordered, unique trusted signing keys.
    pub authorized_signers: Vec<PublicKey>,
}

/// Immutable release subject approved by every Offline Cash V1 authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashReleaseAttestationSubjectV1 {
    /// Subject format version.
    pub version: u16,
    /// Digest of the locally selected authority policy.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub authority_policy_digest: [u8; 32],
    /// Digest-derived release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Digest of the complete canonical release manifest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub manifest_digest: [u8; 32],
    /// Digest of the exact internal-validation receipt.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub validation_receipt_digest: [u8; 32],
    /// Digest of the ordered complete artifact inventory.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_set_digest: [u8; 32],
}

/// Domain-separated payload signed by one Offline Cash V1 release authority.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashReleaseApprovalPayloadV1 {
    /// Cross-protocol replay separator.
    pub domain: String,
    /// Exact release subject approved by the authority.
    pub subject: OfflineCashReleaseAttestationSubjectV1,
}

/// One authority signature in an Offline Cash V1 release attestation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashReleaseApprovalV1 {
    /// Exact key selected by the locally trusted authority policy.
    pub public_key: PublicKey,
    /// Signature over the domain-separated complete release subject.
    pub signature: SignatureOf<OfflineCashReleaseApprovalPayloadV1>,
}

/// Threshold-signed Offline Cash V1 release attestation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashReleaseAttestationV1 {
    /// Attestation format version.
    pub version: u16,
    /// Exact subject shared by every approval.
    pub subject: OfflineCashReleaseAttestationSubjectV1,
    /// Strictly ordered, unique threshold approvals.
    pub approvals: Vec<OfflineCashReleaseApprovalV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
struct OfflineCashReleaseSubjectV1 {
    version: u16,
    source_tree_digest: [u8; 32],
    cargo_lock_digest: [u8; 32],
    profile_digest: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    hardware_policy_digest: [u8; 32],
    validation_receipt_digest: [u8; 32],
    halo2_k: u32,
    artifacts: Vec<OfflineCashArtifactBindingV1>,
}

/// Structural or evidence failure while authenticating an offline-cash release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashReleaseErrorV1 {
    /// Canonical Norito encoding failed.
    Encode,
    /// Manifest identity or fixed profile fields are invalid.
    InvalidManifest,
    /// Artifact inventory is missing, duplicated, unordered, corrupt, or oversized.
    InvalidArtifactSet,
    /// Validation evidence is missing, mismatched, or below a release threshold.
    InvalidValidationReceipt,
    /// Locally selected release-authority policy is malformed.
    InvalidAuthorityPolicy,
    /// Signed attestation does not bind the exact authenticated release subject.
    InvalidAttestation,
    /// An attestation approval does not belong to the trusted authority set.
    UnknownSigner,
    /// Attestation approvals contain a duplicate or are not strictly ordered.
    DuplicateOrUnorderedSigner,
    /// An authorized approval signature is invalid for the exact subject.
    InvalidSignature,
    /// Fewer distinct valid approvals were supplied than policy requires.
    InsufficientThreshold {
        /// Valid approvals collected.
        collected: u16,
        /// Approvals required by policy.
        required: u16,
    },
}

impl core::fmt::Display for OfflineCashReleaseErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Encode => "canonical offline-cash encoding failed",
            Self::InvalidManifest => "invalid offline-cash release manifest",
            Self::InvalidArtifactSet => "invalid offline-cash artifact set",
            Self::InvalidValidationReceipt => "invalid offline-cash validation receipt",
            Self::InvalidAuthorityPolicy => "invalid offline-cash release authority policy",
            Self::InvalidAttestation => "invalid offline-cash release attestation",
            Self::UnknownSigner => "unknown offline-cash release authority",
            Self::DuplicateOrUnorderedSigner => {
                "duplicate or unordered offline-cash release authority"
            }
            Self::InvalidSignature => "invalid offline-cash release authority signature",
            Self::InsufficientThreshold { .. } => {
                "insufficient offline-cash release authority threshold"
            }
        })
    }
}

impl std::error::Error for OfflineCashReleaseErrorV1 {}

/// Runtime capability created only after manifest and evidence authentication.
#[derive(Debug, PartialEq, Eq)]
pub struct OfflineCashAuthenticatedReleaseV1 {
    manifest: OfflineCashReleaseManifestV1,
    manifest_digest: [u8; 32],
    receipt_digest: [u8; 32],
    authority_policy_digest: [u8; 32],
    attestation_digest: [u8; 32],
    approved_signers: Vec<PublicKey>,
}

fn digest_encoded<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], OfflineCashReleaseErrorV1> {
    let bytes = norito::encode_canonical(value).map_err(|_| OfflineCashReleaseErrorV1::Encode)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

fn digest_is_nonzero(value: [u8; 32]) -> bool {
    value.iter().any(|byte| *byte != 0)
}

/// Decode one exact canonical release frame only after its complete outer byte
/// length is known to be within the type-specific admission cap.
fn decode_release_bounded_canonical<T>(
    bytes: &[u8],
    max: usize,
    invalid: OfflineCashReleaseErrorV1,
) -> Result<T, OfflineCashReleaseErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > max {
        return Err(invalid);
    }
    let limits = norito::canonical_decode_limits(bytes.len());
    norito::decode_canonical_with_limits(bytes, limits).map_err(|_| invalid)
}

/// Return the digest of a complete, canonically ordered artifact inventory.
///
/// # Errors
///
/// Returns an error when the inventory is incomplete, unordered, duplicated, or oversized.
pub fn offline_cash_artifact_set_digest_v1(
    artifacts: &[OfflineCashArtifactBindingV1],
) -> Result<[u8; 32], OfflineCashReleaseErrorV1> {
    validate_artifacts(artifacts)?;
    digest_encoded(ARTIFACT_SET_DIGEST_DOMAIN, &artifacts.to_vec())
}

fn validate_artifacts(
    artifacts: &[OfflineCashArtifactBindingV1],
) -> Result<(), OfflineCashReleaseErrorV1> {
    if artifacts.len() != OfflineCashArtifactRoleV1::ALL.len() {
        return Err(OfflineCashReleaseErrorV1::InvalidArtifactSet);
    }
    let mut total = 0_u64;
    for (artifact, expected_role) in artifacts
        .iter()
        .zip(OfflineCashArtifactRoleV1::ALL.iter().copied())
    {
        if artifact.role != expected_role || !digest_is_nonzero(artifact.sha256) {
            return Err(OfflineCashReleaseErrorV1::InvalidArtifactSet);
        }
        let valid_size = if artifact.role.is_params() {
            artifact.byte_len == OFFLINE_CASH_PARAMS_BYTES_V1
        } else if artifact.role.is_state_pk() {
            artifact.byte_len != 0
                && artifact.byte_len <= OFFLINE_CASH_STATE_PROVING_KEY_MAX_BYTES_V1
        } else if artifact.role.is_helper_pk() {
            artifact.byte_len != 0
                && artifact.byte_len <= OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1
        } else if artifact.role.is_vk() {
            artifact.byte_len != 0 && artifact.byte_len <= OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1
        } else {
            false
        };
        if !valid_size {
            return Err(OfflineCashReleaseErrorV1::InvalidArtifactSet);
        }
        total = total
            .checked_add(artifact.byte_len)
            .ok_or(OfflineCashReleaseErrorV1::InvalidArtifactSet)?;
    }
    if total > OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1 {
        return Err(OfflineCashReleaseErrorV1::InvalidArtifactSet);
    }
    Ok(())
}

impl OfflineCashInternalValidationReceiptV1 {
    /// Validate all evidence identities and numeric release thresholds.
    ///
    /// # Errors
    ///
    /// Returns an error when evidence is absent or any measured result exceeds its release limit.
    pub fn validate(&self) -> Result<(), OfflineCashReleaseErrorV1> {
        let digests = [
            self.source_tree_digest,
            self.cargo_lock_digest,
            self.profile_digest,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            self.artifact_set_digest,
            self.hardware_policy_digest,
            self.circuit_shape_report_digest,
            self.security_review_digest,
            self.kat_report_digest,
            self.fuzz_report_digest,
            self.resource_report_digest,
            self.ios_device_report_digest,
            self.android_device_report_digest,
            self.four_peer_report_digest,
        ];
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || digests.into_iter().any(|digest| !digest_is_nonzero(digest))
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.max_proof_pair_bytes
                > u32::try_from(OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1)
                    .expect("proof target fits u32")
            || self.max_session_bytes
                > u32::try_from(OFFLINE_CASH_SESSION_TARGET_BYTES_V1)
                    .expect("session target fits u32")
            || self.max_process_rss_bytes > OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1
            || self.prove_p95_ms > OFFLINE_CASH_PROVE_P95_MAX_MS_V1
            || self.verify_p95_ms > OFFLINE_CASH_VERIFY_P95_MAX_MS_V1
            || self.handoff_p95_ms > OFFLINE_CASH_HANDOFF_P95_MAX_MS_V1
            || self.qualified_handoffs < OFFLINE_CASH_MIN_QUALIFIED_HANDOFFS_V1
            || self.fuzz_cases < OFFLINE_CASH_MIN_FUZZ_CASES_V1
            || self.reproducible_builds < OFFLINE_CASH_REPRODUCIBLE_BUILD_COUNT_V1
            || self.validator_count != OFFLINE_CASH_VALIDATOR_COUNT_V1
        {
            return Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt);
        }
        Ok(())
    }

    /// Return the SHA-256 identity pinned by a release manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the receipt is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashReleaseErrorV1> {
        self.validate()?;
        digest_encoded(RECEIPT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashReleaseAuthorityPolicyV1 {
    /// Decode and validate one exact bounded trusted authority policy.
    ///
    /// The complete byte cap is enforced before Norito reads a header or any
    /// attacker-declared collection length.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, malformed, non-canonical, or
    /// structurally invalid policy.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, OfflineCashReleaseErrorV1> {
        let policy: Self = decode_release_bounded_canonical(
            bytes,
            OFFLINE_CASH_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
            OfflineCashReleaseErrorV1::InvalidAuthorityPolicy,
        )?;
        policy.validate()?;
        Ok(policy)
    }

    /// Validate the trusted authority set, signer order, and threshold.
    ///
    /// # Errors
    ///
    /// Returns an error when the policy is empty, oversized, duplicated,
    /// unordered, or has an impossible threshold.
    pub fn validate(&self) -> Result<(), OfflineCashReleaseErrorV1> {
        let signer_count = self.authorized_signers.len();
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || !digest_is_nonzero(self.authority_set_id)
            || signer_count == 0
            || signer_count > OFFLINE_CASH_RELEASE_AUTHORITY_MAX_SIGNERS_V1
            || self.threshold == 0
            || usize::from(self.threshold) > signer_count
            || !self
                .authorized_signers
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return Err(OfflineCashReleaseErrorV1::InvalidAuthorityPolicy);
        }
        Ok(())
    }

    /// Return the canonical identity bound into every release attestation.
    ///
    /// # Errors
    ///
    /// Returns an error when the policy is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashReleaseErrorV1> {
        self.validate()?;
        digest_encoded(AUTHORITY_POLICY_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashReleaseAttestationSubjectV1 {
    /// Return the exact domain-separated value signed by each authority.
    #[must_use]
    pub fn approval_payload(&self) -> OfflineCashReleaseApprovalPayloadV1 {
        OfflineCashReleaseApprovalPayloadV1 {
            domain: RELEASE_APPROVAL_DOMAIN.to_owned(),
            subject: *self,
        }
    }
}

impl OfflineCashReleaseAttestationV1 {
    /// Decode and structurally validate one exact bounded release attestation.
    ///
    /// Signature and trusted-policy verification still occur only through
    /// [`OfflineCashReleaseManifestV1::authenticate`]. The complete byte cap
    /// is enforced before Norito reads a header or any declared collection
    /// length.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, malformed, non-canonical, or
    /// structurally invalid attestation.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, OfflineCashReleaseErrorV1> {
        let attestation: Self = decode_release_bounded_canonical(
            bytes,
            OFFLINE_CASH_RELEASE_ATTESTATION_MAX_BYTES_V1,
            OfflineCashReleaseErrorV1::InvalidAttestation,
        )?;
        attestation.validate_standalone()?;
        Ok(attestation)
    }

    fn validate_standalone(&self) -> Result<(), OfflineCashReleaseErrorV1> {
        let subject_digests = [
            self.subject.authority_policy_digest,
            self.subject.release_id,
            self.subject.manifest_digest,
            self.subject.validation_receipt_digest,
            self.subject.artifact_set_digest,
        ];
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.subject.version != OFFLINE_CASH_WIRE_VERSION_V1
            || subject_digests
                .into_iter()
                .any(|digest| !digest_is_nonzero(digest))
            || self.approvals.is_empty()
            || self.approvals.len() > OFFLINE_CASH_RELEASE_AUTHORITY_MAX_SIGNERS_V1
        {
            return Err(OfflineCashReleaseErrorV1::InvalidAttestation);
        }
        if !self
            .approvals
            .windows(2)
            .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(OfflineCashReleaseErrorV1::DuplicateOrUnorderedSigner);
        }
        Ok(())
    }
}

impl OfflineCashReleaseManifestV1 {
    /// Decode and structurally validate one exact bounded release manifest.
    ///
    /// Receipt and authority evidence must subsequently be verified through
    /// [`Self::authenticate`]. The complete byte cap is enforced before
    /// Norito reads a header or any declared collection length.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, malformed, non-canonical, or
    /// structurally invalid manifest.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, OfflineCashReleaseErrorV1> {
        let manifest: Self = decode_release_bounded_canonical(
            bytes,
            OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1,
            OfflineCashReleaseErrorV1::InvalidManifest,
        )?;
        manifest.validate_standalone()?;
        Ok(manifest)
    }

    fn validate_standalone(&self) -> Result<(), OfflineCashReleaseErrorV1> {
        validate_artifacts(&self.artifacts)?;
        let digests = [
            self.release_id,
            self.source_tree_digest,
            self.cargo_lock_digest,
            self.profile_digest,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            self.hardware_policy_digest,
            self.validation_receipt_digest,
        ];
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.halo2_k != OFFLINE_CASH_HALO2_K_V1
            || digests.into_iter().any(|digest| !digest_is_nonzero(digest))
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.release_id != self.expected_release_id()?
        {
            return Err(OfflineCashReleaseErrorV1::InvalidManifest);
        }
        Ok(())
    }

    fn subject(&self) -> OfflineCashReleaseSubjectV1 {
        OfflineCashReleaseSubjectV1 {
            version: self.version,
            source_tree_digest: self.source_tree_digest,
            cargo_lock_digest: self.cargo_lock_digest,
            profile_digest: self.profile_digest,
            eq_protocol_digest: self.eq_protocol_digest,
            ep_protocol_digest: self.ep_protocol_digest,
            hardware_policy_digest: self.hardware_policy_digest,
            validation_receipt_digest: self.validation_receipt_digest,
            halo2_k: self.halo2_k,
            artifacts: self.artifacts.clone(),
        }
    }

    /// Compute the digest-derived release identifier from every authoritative field.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_release_id(&self) -> Result<[u8; 32], OfflineCashReleaseErrorV1> {
        digest_encoded(RELEASE_ID_DOMAIN, &self.subject())
    }

    /// Populate the digest-derived release identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal(mut self) -> Result<Self, OfflineCashReleaseErrorV1> {
        self.release_id = self.expected_release_id()?;
        Ok(self)
    }

    /// Build the immutable subject that trusted release authorities must sign.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest, receipt, artifact inventory, or
    /// locally selected authority policy is invalid or mismatched.
    pub fn release_attestation_subject(
        &self,
        receipt: &OfflineCashInternalValidationReceiptV1,
        policy: &OfflineCashReleaseAuthorityPolicyV1,
    ) -> Result<OfflineCashReleaseAttestationSubjectV1, OfflineCashReleaseErrorV1> {
        validate_artifacts(&self.artifacts)?;
        receipt.validate()?;
        let authority_policy_digest = policy.canonical_digest()?;
        let artifact_set_digest = offline_cash_artifact_set_digest_v1(&self.artifacts)?;
        let receipt_digest = receipt.canonical_digest()?;
        let manifest_digest = digest_encoded(MANIFEST_DIGEST_DOMAIN, self)?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.halo2_k != OFFLINE_CASH_HALO2_K_V1
            || !digest_is_nonzero(self.release_id)
            || self.release_id != self.expected_release_id()?
            || self.source_tree_digest != receipt.source_tree_digest
            || self.cargo_lock_digest != receipt.cargo_lock_digest
            || self.profile_digest != receipt.profile_digest
            || self.eq_protocol_digest != receipt.eq_protocol_digest
            || self.ep_protocol_digest != receipt.ep_protocol_digest
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.hardware_policy_digest != receipt.hardware_policy_digest
            || self.validation_receipt_digest != receipt_digest
            || receipt.artifact_set_digest != artifact_set_digest
        {
            return Err(OfflineCashReleaseErrorV1::InvalidManifest);
        }
        Ok(OfflineCashReleaseAttestationSubjectV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            authority_policy_digest,
            release_id: self.release_id,
            manifest_digest,
            validation_receipt_digest: receipt_digest,
            artifact_set_digest,
        })
    }

    /// Authenticate the manifest, receipt, artifacts, and threshold attestation.
    ///
    /// # Errors
    ///
    /// Returns an error when any identity, inventory, policy, signature,
    /// threshold, or subject binding fails.
    pub fn authenticate(
        &self,
        receipt: &OfflineCashInternalValidationReceiptV1,
        policy: &OfflineCashReleaseAuthorityPolicyV1,
        attestation: &OfflineCashReleaseAttestationV1,
    ) -> Result<OfflineCashAuthenticatedReleaseV1, OfflineCashReleaseErrorV1> {
        let expected_subject = self.release_attestation_subject(receipt, policy)?;
        if attestation.version != OFFLINE_CASH_WIRE_VERSION_V1
            || attestation.subject != expected_subject
            || attestation.approvals.len() > policy.authorized_signers.len()
        {
            return Err(OfflineCashReleaseErrorV1::InvalidAttestation);
        }
        if !attestation
            .approvals
            .windows(2)
            .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(OfflineCashReleaseErrorV1::DuplicateOrUnorderedSigner);
        }
        let payload = expected_subject.approval_payload();
        let mut approved_signers = Vec::with_capacity(attestation.approvals.len());
        for approval in &attestation.approvals {
            if policy
                .authorized_signers
                .binary_search(&approval.public_key)
                .is_err()
            {
                return Err(OfflineCashReleaseErrorV1::UnknownSigner);
            }
            approval
                .signature
                .verify(&approval.public_key, &payload)
                .map_err(|_| OfflineCashReleaseErrorV1::InvalidSignature)?;
            approved_signers.push(approval.public_key.clone());
        }
        let collected = u16::try_from(approved_signers.len())
            .map_err(|_| OfflineCashReleaseErrorV1::InvalidAttestation)?;
        if collected < policy.threshold {
            return Err(OfflineCashReleaseErrorV1::InsufficientThreshold {
                collected,
                required: policy.threshold,
            });
        }
        let attestation_digest = digest_encoded(RELEASE_ATTESTATION_DIGEST_DOMAIN, attestation)?;
        Ok(OfflineCashAuthenticatedReleaseV1 {
            manifest: self.clone(),
            manifest_digest: expected_subject.manifest_digest,
            receipt_digest: expected_subject.validation_receipt_digest,
            authority_policy_digest: expected_subject.authority_policy_digest,
            attestation_digest,
            approved_signers,
        })
    }
}

impl OfflineCashAuthenticatedReleaseV1 {
    /// Return the authenticated release identifier.
    #[must_use]
    pub fn release_id(&self) -> [u8; 32] {
        self.manifest.release_id
    }

    /// Return the authenticated profile digest.
    #[must_use]
    pub fn profile_digest(&self) -> [u8; 32] {
        self.manifest.profile_digest
    }

    /// Return the authenticated Eq/Fp compiled-protocol digest.
    #[must_use]
    pub fn eq_protocol_digest(&self) -> [u8; 32] {
        self.manifest.eq_protocol_digest
    }

    /// Return the authenticated Ep/Fq compiled-protocol digest.
    #[must_use]
    pub fn ep_protocol_digest(&self) -> [u8; 32] {
        self.manifest.ep_protocol_digest
    }

    /// Return the digest of the complete authenticated manifest bytes.
    #[must_use]
    pub fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }

    /// Return the authenticated validation-receipt digest.
    #[must_use]
    pub fn receipt_digest(&self) -> [u8; 32] {
        self.receipt_digest
    }

    /// Return the exact locally trusted authority-policy digest.
    #[must_use]
    pub fn authority_policy_digest(&self) -> [u8; 32] {
        self.authority_policy_digest
    }

    /// Return the digest of the verified threshold-signed attestation.
    #[must_use]
    pub fn attestation_digest(&self) -> [u8; 32] {
        self.attestation_digest
    }

    /// Return the strictly ordered authorities whose signatures were verified.
    #[must_use]
    pub fn approved_signers(&self) -> &[PublicKey] {
        &self.approved_signers
    }

    /// Resolve one required artifact binding by its unique canonical role.
    #[must_use]
    pub fn artifact(&self, role: OfflineCashArtifactRoleV1) -> OfflineCashArtifactBindingV1 {
        let index = OfflineCashArtifactRoleV1::ALL
            .iter()
            .position(|candidate| *candidate == role)
            .expect("every role belongs to the authenticated inventory");
        self.manifest.artifacts[index]
    }
}

#[cfg(test)]
#[path = "offline_cash_release_v1_tests.rs"]
mod tests;
