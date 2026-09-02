//! Authenticated artifact and internal-validation contract for Kagemusha V1.

use super::{
    KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1, KAGEMUSHA_SESSION_MAX_BYTES_V1,
    KAGEMUSHA_TEXT_SESSION_MAX_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1,
};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Fixed Halo2 domain exponent for every kagemusha proof role.
pub const KAGEMUSHA_HALO2_K_V1: u32 = 16;
/// Exact serialized transparent IPA parameters for either Pasta parity.
pub const KAGEMUSHA_PARAMS_BYTES_V1: u64 = 4_194_372;
/// Maximum processed state proving-key bytes for either parity.
pub const KAGEMUSHA_STATE_PROVING_KEY_MAX_BYTES_V1: u64 = 48_234_934;
/// Maximum processed helper proving-key bytes for either parity.
pub const KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Maximum processed verifying-key bytes for one role and parity.
pub const KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1: u64 = 64 * 1024;
/// Maximum raw proof-evidence bytes for one internal helper parity.
///
/// This is a release-evidence resource ceiling, not a Kagemusha payment
/// transport allowance. The authenticated compiled protocol fixes the exact
/// admissible length below this ceiling.
pub const KAGEMUSHA_INTERNAL_HELPER_PROOF_EVIDENCE_MAX_BYTES_V1: u32 = 64 * 1024 * 1024;
/// Maximum complete preinstalled Kagemusha artifact package.
pub const KAGEMUSHA_ARTIFACT_SET_MAX_BYTES_V1: u64 = 512 * 1024 * 1024;
/// Maximum whole-process resident memory during proving or verification.
pub const KAGEMUSHA_PROCESS_RSS_MAX_BYTES_V1: u64 = 128 * 1024 * 1024;
/// Minimum distinct reproducible builds required by validation.
pub const KAGEMUSHA_REPRODUCIBLE_BUILD_COUNT_V1: u8 = 2;
/// Minimum alternating send/receive handoffs in the invariant-size KAT.
pub const KAGEMUSHA_MIN_QUALIFIED_HANDOFFS_V1: u32 = 1_024;
/// Minimum independent credits folded and spent by the aggregate-balance KAT.
pub const KAGEMUSHA_MIN_QUALIFIED_AGGREGATED_CREDITS_V1: u32 = 1_000;
/// Minimum credits folded during the thermal-throttling qualification run.
pub const KAGEMUSHA_MIN_THERMAL_FOLDED_CREDITS_V1: u32 = 1_000;
/// Minimum parser-fuzz cases represented by the validation report.
pub const KAGEMUSHA_MIN_FUZZ_CASES_V1: u64 = 10_000_000;
/// Balanced-baseline p95 proving ceiling in milliseconds.
pub const KAGEMUSHA_PROVE_P95_MAX_MS_V1: u32 = 10_000;
/// Balanced-baseline p95 verification ceiling in milliseconds.
pub const KAGEMUSHA_VERIFY_P95_MAX_MS_V1: u32 = 1_000;
/// Balanced-baseline p95 complete handoff ceiling in milliseconds.
pub const KAGEMUSHA_HANDOFF_P95_MAX_MS_V1: u32 = 30_000;
/// Required validator count for activation/restart/replay qualification.
pub const KAGEMUSHA_VALIDATOR_COUNT_V1: u8 = 4;
/// Maximum trusted authorities in one locally configured release policy.
pub const KAGEMUSHA_RELEASE_AUTHORITY_MAX_SIGNERS_V1: usize = 32;
/// Maximum canonical bytes accepted for one Kagemusha V1 release manifest.
///
/// The 64-KiB admission budget accommodates the complete authenticated records
/// for all 64 embedded hardware profiles; it is enforced before decoding.
pub const KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum canonical bytes accepted for one internal qualification receipt.
pub const KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1: usize = 1024 * 1024;
/// Maximum canonical bytes accepted for one locally selected authority policy.
pub const KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1: usize = 512 * 1024;
/// Maximum canonical bytes accepted for one threshold release attestation.
pub const KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1: usize = 1024 * 1024;
/// Maximum enabled hardware profiles represented by one bounded V1 receipt.
pub const KAGEMUSHA_RELEASE_MAX_ENABLED_PROFILES_V1: usize = 64;
/// Maximum reproducible-build records represented by one bounded V1 receipt.
pub const KAGEMUSHA_RELEASE_MAX_REPRODUCIBLE_BUILDS_V1: usize = 8;
/// Maximum byte length represented by one external evidence-file binding.
pub const KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1: u64 = 4 * 1024 * 1024 * 1024;
/// Maximum closed evidence-root bytes accepted by the V1 authority projection.
pub const KAGEMUSHA_RELEASE_EVIDENCE_TOTAL_MAX_BYTES_V1: u64 = 6 * 1024 * 1024 * 1024;
/// Maximum signed verifier observations in one V1 evidence closure.
pub const KAGEMUSHA_RELEASE_VERIFICATION_RECORD_MAX_COUNT_V1: u32 = 8_192;
/// Maximum aggregate trusted-verifier transcript bytes in one V1 closure.
pub const KAGEMUSHA_RELEASE_TRANSCRIPT_TOTAL_MAX_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Maximum aggregate verifier-input bytes counted across V1 observations.
pub const KAGEMUSHA_RELEASE_COMMAND_INPUT_TOTAL_MAX_BYTES_V1: u64 = 48 * 1024 * 1024 * 1024;
/// Maximum aggregate observed wall time or CPU time in milliseconds.
pub const KAGEMUSHA_RELEASE_OBSERVED_TIME_TOTAL_MAX_MS_V1: u64 = 24 * 60 * 60 * 1_000;

const ARTIFACT_SET_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:artifact-set";
const VK_SET_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:vk-set";
const PROFILE_QUALIFICATION_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:profile-qualification";
const HARDWARE_POLICY_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-policy";
const SUITE_COMMITMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:suite-commitment";
const RECEIPT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:validation-receipt";
const RELEASE_ID_DOMAIN: &[u8] = b"iroha:kagemusha:v1:release";
const MANIFEST_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:manifest";
const AUTHORITY_POLICY_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:release-authority-policy";
const RELEASE_APPROVAL_DOMAIN: &str = "iroha:kagemusha:v1:release-approval";
const RELEASE_ATTESTATION_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:release-attestation";
const RELEASE_PROFILE_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:release-profile";

/// Canonical role of one preinstalled transparent Halo2 artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "role", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
#[repr(u8)]
pub enum KagemushaArtifactRoleV1 {
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
    /// Eq/Fp mint-authorization helper proving key.
    MintAuthorizationPkEq,
    /// Eq/Fp mint-authorization helper verifying key.
    MintAuthorizationVkEq,
    /// Ep/Fq mint-authorization helper proving key.
    MintAuthorizationPkEp,
    /// Ep/Fq mint-authorization helper verifying key.
    MintAuthorizationVkEp,
    /// Eq/Fp finalized mint-credit helper proving key.
    MintCreditPkEq,
    /// Eq/Fp finalized mint-credit helper verifying key.
    MintCreditVkEq,
    /// Ep/Fq finalized mint-credit helper proving key.
    MintCreditPkEp,
    /// Ep/Fq finalized mint-credit helper verifying key.
    MintCreditVkEp,
    /// Eq/Fp provider-neutral hardware-credential proving key.
    PlatformCredentialPkEq,
    /// Eq/Fp provider-neutral hardware-credential verifying key.
    PlatformCredentialVkEq,
    /// Ep/Fq provider-neutral hardware-credential proving key.
    PlatformCredentialPkEp,
    /// Ep/Fq provider-neutral hardware-credential verifying key.
    PlatformCredentialVkEp,
    /// Eq/Fp `GuardBundle` proving key.
    GuardBundlePkEq,
    /// Eq/Fp `GuardBundle` verifying key.
    GuardBundleVkEq,
    /// Ep/Fq `GuardBundle` proving key.
    GuardBundlePkEp,
    /// Ep/Fq `GuardBundle` verifying key.
    GuardBundleVkEp,
}

impl KagemushaArtifactRoleV1 {
    /// Exact canonically ordered release inventory.
    pub const ALL: [Self; 22] = [
        Self::ParamsEq,
        Self::ParamsEp,
        Self::StatePkEq,
        Self::StateVkEq,
        Self::StatePkEp,
        Self::StateVkEp,
        Self::MintAuthorizationPkEq,
        Self::MintAuthorizationVkEq,
        Self::MintAuthorizationPkEp,
        Self::MintAuthorizationVkEp,
        Self::MintCreditPkEq,
        Self::MintCreditVkEq,
        Self::MintCreditPkEp,
        Self::MintCreditVkEp,
        Self::PlatformCredentialPkEq,
        Self::PlatformCredentialVkEq,
        Self::PlatformCredentialPkEp,
        Self::PlatformCredentialVkEp,
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
            Self::MintAuthorizationPkEq
                | Self::MintAuthorizationPkEp
                | Self::MintCreditPkEq
                | Self::MintCreditPkEp
                | Self::PlatformCredentialPkEq
                | Self::PlatformCredentialPkEp
                | Self::GuardBundlePkEq
                | Self::GuardBundlePkEp
        )
    }

    const fn is_vk(self) -> bool {
        matches!(
            self,
            Self::StateVkEq
                | Self::StateVkEp
                | Self::MintAuthorizationVkEq
                | Self::MintAuthorizationVkEp
                | Self::MintCreditVkEq
                | Self::MintCreditVkEp
                | Self::PlatformCredentialVkEq
                | Self::PlatformCredentialVkEp
                | Self::GuardBundleVkEq
                | Self::GuardBundleVkEp
        )
    }
}

/// Digest and byte length of one authenticated artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaArtifactBindingV1 {
    /// Artifact role.
    pub role: KagemushaArtifactRoleV1,
    /// SHA-256 of the exact file bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sha256: [u8; 32],
    /// Exact file length.
    pub byte_len: u64,
}

/// SHA-256 and length of one external qualification artifact.
///
/// This binding is provenance, not proof that the referenced bytes satisfy a
/// qualification case. Immutable-candidate release tooling must read the
/// complete file, verify its length and SHA-256, validate its typed contents,
/// and only then construct and sign a receipt. Runtime code authenticates that
/// signed projection; it cannot demonstrate the external file's semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaEvidenceFileV1 {
    /// SHA-256 of the exact evidence-file bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sha256: [u8; 32],
    /// Exact file length.
    pub byte_len: u64,
}

/// One hardware profile enabled by a release manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaEnabledProfileV1 {
    /// Complete governed non-forking hardware-service profile.
    pub hardware_profile: super::KagemushaHardwareProfileV1,
    /// Digest-derived [`super::KagemushaHardwareProfileV1`] identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Exact proof suite admitted for credentials under this profile.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub suite_id: [u8; 32],
    /// Digest of every authenticated verifier artifact and compiled protocol identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub vk_digest: [u8; 32],
    /// Digest of this profile's exact typed qualification matrix.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub qualification_digest: [u8; 32],
    /// Exact governed policy epoch.
    pub policy_epoch: u64,
    /// Physical qualification report approved for this profile.
    pub qualification_report: KagemushaEvidenceFileV1,
}

/// Recursive relation qualified for each enabled hardware profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "relation", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
#[repr(u8)]
pub enum KagemushaQualifiedRelationV1 {
    /// Create the first aggregate-balance state.
    Bootstrap,
    /// Fold an authenticated online mint into the aggregate balance.
    MintFold,
    /// Split one payment from the aggregate balance.
    SendSplit,
    /// Fold one received credit into the aggregate balance.
    ReceiveFold,
    /// Split an online redemption from the aggregate balance.
    RedeemSplit,
    /// Rotate the hardware credential and, when governed, the verifier suite without changing
    /// value.
    Rotate,
}

/// Helper circuit qualified alongside the six aggregate-state relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "helper", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
#[repr(u8)]
pub enum KagemushaQualifiedHelperCircuitV1 {
    /// Authenticate a ledger-issued mint authorization before mint finalization.
    MintAuthorization,
    /// Authenticate finalized online mint credits before folding.
    MintCredit,
    /// Verify the compact governance-issued hardware credential.
    PlatformCredential,
    /// Verify the complete non-forking hardware guard bundle.
    GuardBundle,
}

impl KagemushaQualifiedHelperCircuitV1 {
    /// Exact canonically ordered helper-circuit set.
    pub const ALL: [Self; 4] = [
        Self::MintAuthorization,
        Self::MintCredit,
        Self::PlatformCredential,
        Self::GuardBundle,
    ];

    const fn expected_vk_roles(self) -> (KagemushaArtifactRoleV1, KagemushaArtifactRoleV1) {
        match self {
            Self::MintAuthorization => (
                KagemushaArtifactRoleV1::MintAuthorizationVkEq,
                KagemushaArtifactRoleV1::MintAuthorizationVkEp,
            ),
            Self::MintCredit => (
                KagemushaArtifactRoleV1::MintCreditVkEq,
                KagemushaArtifactRoleV1::MintCreditVkEp,
            ),
            Self::PlatformCredential => (
                KagemushaArtifactRoleV1::PlatformCredentialVkEq,
                KagemushaArtifactRoleV1::PlatformCredentialVkEp,
            ),
            Self::GuardBundle => (
                KagemushaArtifactRoleV1::GuardBundleVkEq,
                KagemushaArtifactRoleV1::GuardBundleVkEp,
            ),
        }
    }

    const fn uses_internal_proof_evidence(self) -> bool {
        matches!(self, Self::PlatformCredential | Self::GuardBundle)
    }
}

/// Release-bound compiled protocol identity for one helper circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaHelperProtocolV1 {
    /// Helper circuit whose compiled protocols are identified.
    pub helper: KagemushaQualifiedHelperCircuitV1,
    /// Compiled Eq/Fp protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep/Fq protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Exact raw Eq/Fp proof bytes for an internal-only helper.
    ///
    /// This is zero for `MintAuthorization` and `MintCredit`, whose public wire
    /// values remain governed by the V1 parity, current-proof, and canonical
    /// paired-proof transport ceilings.
    pub eq_proof_bytes: u32,
    /// Exact raw Ep/Fq proof bytes for an internal-only helper.
    ///
    /// This is zero for `MintAuthorization` and `MintCredit`. A nonzero value
    /// is an authenticated compiled-protocol property, not a transport cap.
    pub ep_proof_bytes: u32,
}

/// Real-circuit qualification for one non-state helper circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaHelperQualificationV1 {
    /// Helper circuit measured by this record.
    pub helper: KagemushaQualifiedHelperCircuitV1,
    /// Compiled Eq/Fp protocol digest exercised by the measurement.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep/Fq protocol digest exercised by the measurement.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Exact Eq/Fp verifier artifact exercised by the measurement.
    pub eq_verifying_key: KagemushaArtifactBindingV1,
    /// Exact Ep/Fq verifier artifact exercised by the measurement.
    pub ep_verifying_key: KagemushaArtifactBindingV1,
    /// Eq/Fp rows synthesized with `k = 16`.
    pub eq_circuit_rows: u32,
    /// Ep/Fq rows synthesized with `k = 16`.
    pub ep_circuit_rows: u32,
    /// Exact raw Eq/Fp proof bytes observed for an internal-only helper.
    ///
    /// This is zero when `complete_proof_bytes` measures a canonical paired
    /// wire value for `MintAuthorization` or `MintCredit`.
    pub eq_proof_bytes: u32,
    /// Exact raw Ep/Fq proof bytes observed for an internal-only helper.
    ///
    /// This is zero when `complete_proof_bytes` measures a canonical paired
    /// wire value for `MintAuthorization` or `MintCredit`.
    pub ep_proof_bytes: u32,
    /// Complete proof bytes observed for the helper.
    ///
    /// For `MintAuthorization` and `MintCredit`, this is the canonical paired
    /// wire value and must fit the 6,528-byte transport ceiling. For
    /// `PlatformCredential` and `GuardBundle`, this is exactly the checked sum
    /// of the separately observed raw Eq/Fp and Ep/Fq internal proof lengths;
    /// it is deliberately not interpreted as a paired-wire length.
    pub complete_proof_bytes: u32,
    /// Slowest qualifying p95 proof generation, in milliseconds.
    pub prove_p95_ms: u32,
    /// Slowest qualifying p95 proof verification, in milliseconds.
    pub verify_p95_ms: u32,
    /// Largest whole-process RSS observed, in bytes.
    pub process_rss_bytes: u64,
    /// Largest measured operation energy, in millijoules.
    pub operation_energy_millijoules: u64,
    /// Exact helper-circuit measurement report.
    pub report: KagemushaEvidenceFileV1,
}

impl KagemushaQualifiedRelationV1 {
    /// Exact ordered relation set required for every enabled profile.
    pub const ALL: [Self; 6] = [
        Self::Bootstrap,
        Self::MintFold,
        Self::SendSplit,
        Self::ReceiveFold,
        Self::RedeemSplit,
        Self::Rotate,
    ];

    const fn expected_vk_roles(self) -> (KagemushaArtifactRoleV1, KagemushaArtifactRoleV1) {
        (
            KagemushaArtifactRoleV1::StateVkEq,
            KagemushaArtifactRoleV1::StateVkEp,
        )
    }
}

/// Per-relation real-circuit measurements for one enabled hardware profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaRelationQualificationV1 {
    /// Relation measured by this record.
    pub relation: KagemushaQualifiedRelationV1,
    /// Compiled Eq/Fp protocol digest exercised by this relation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep/Fq protocol digest exercised by this relation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Exact Eq/Fp verifier artifact exercised by the measurement.
    pub eq_verifying_key: KagemushaArtifactBindingV1,
    /// Exact Ep/Fq verifier artifact exercised by the measurement.
    pub ep_verifying_key: KagemushaArtifactBindingV1,
    /// Eq/Fp rows synthesized with `k = 16`.
    pub eq_circuit_rows: u32,
    /// Ep/Fq rows synthesized with `k = 16`.
    pub ep_circuit_rows: u32,
    /// Exact complete paired-proof bytes observed for the relation.
    pub complete_proof_bytes: u32,
    /// Slowest qualifying p95 proof generation, in milliseconds.
    pub prove_p95_ms: u32,
    /// Slowest qualifying p95 proof verification, in milliseconds.
    pub verify_p95_ms: u32,
    /// Largest whole-process RSS observed, in bytes.
    pub process_rss_bytes: u64,
    /// Largest measured operation energy, in millijoules.
    pub operation_energy_millijoules: u64,
    /// Exact per-relation measurement report.
    pub report: KagemushaEvidenceFileV1,
}

/// One real recursive-depth qualification run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaRecursiveDepthQualificationV1 {
    /// Recursive transition depth exercised with real proofs.
    pub depth: u32,
    /// Handoffs actually verified; it must equal `depth`.
    pub verified_handoffs: u32,
    /// Exact complete paired-proof bytes at this depth.
    pub complete_proof_bytes: u32,
    /// Exact complete raw session bytes at this depth.
    pub raw_session_bytes: u32,
    /// Exact complete text session bytes at this depth.
    pub text_session_bytes: u32,
    /// Exact depth-run report.
    pub report: KagemushaEvidenceFileV1,
}

/// Quantitative aggregate-balance qualification for one profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaAggregateBalanceQualificationV1 {
    /// Independent payments created for the run.
    pub independent_payments: u32,
    /// Independent credits folded into one aggregate state.
    pub folded_credits: u32,
    /// Payments emitted from the aggregate state after folding; exactly one is required.
    pub spend_payments: u32,
    /// Exact aggregate-fold-and-spend report.
    pub report: KagemushaEvidenceFileV1,
}

/// Sustained thermally throttled receive-fold qualification for one profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaThermalQualificationV1 {
    /// Credits folded during the sustained run.
    pub folded_credits: u32,
    /// Slowest qualifying p95 single-credit receive-fold proof, in milliseconds.
    pub fold_p95_ms: u32,
    /// Largest whole-process RSS observed, in bytes.
    pub process_rss_bytes: u64,
    /// Largest single-credit receive-fold energy observed, in millijoules.
    pub operation_energy_millijoules: u64,
    /// Exact thermal-run report.
    pub report: KagemushaEvidenceFileV1,
}

/// Complete session-size and handoff measurement for one profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaEnvelopeQualificationV1 {
    /// Largest complete raw handoff session.
    pub raw_session_bytes: u32,
    /// Largest complete text handoff session.
    pub text_session_bytes: u32,
    /// Slowest qualifying p95 complete handoff, in milliseconds.
    pub handoff_p95_ms: u32,
    /// Exact transport measurement report.
    pub report: KagemushaEvidenceFileV1,
}

/// Complete real-circuit and physical-device evidence for one enabled profile.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaProfileQualificationV1 {
    /// Exact enabled profile qualified by this record.
    pub profile: KagemushaEnabledProfileV1,
    /// Exactly the nine required relations in protocol order.
    pub relations: Vec<KagemushaRelationQualificationV1>,
    /// Exactly the four required helper circuits in protocol order.
    pub helper_circuits: Vec<KagemushaHelperQualificationV1>,
    /// Exactly depths 8, 64, 1,024, and one greater depth, in ascending order.
    pub recursive_depths: Vec<KagemushaRecursiveDepthQualificationV1>,
    /// At least 1,000 independent credits folded and spent as one payment.
    pub aggregate_balance: KagemushaAggregateBalanceQualificationV1,
    /// Sustained thermal-fold qualification.
    pub thermal: KagemushaThermalQualificationV1,
    /// Raw/text transport and end-to-end handoff qualification.
    pub envelope: KagemushaEnvelopeQualificationV1,
    /// Exact closed acceptance-case evidence for this enabled profile.
    pub acceptance_cases: Vec<KagemushaAcceptanceCaseEvidenceV1>,
}

/// Closed release-acceptance case set. Every case is mandatory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "case", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
#[repr(u8)]
pub enum KagemushaAcceptanceCaseV1 {
    /// Receiver hardware remains recoverable under sustained inbox pressure.
    ReceiverInboxPressure,
    /// Sender outbox capacity is reserved before commitment.
    SenderOutboxCapacityExhaustion,
    /// Crash after durable transition-intent staging.
    CrashAfterTransitionIntentStage,
    /// Crash during the one authoritative hardware commit.
    CrashDuringHardwareCommit,
    /// Crash after hardware commit and before proof completion.
    CrashAfterHardwareCommitBeforeProof,
    /// Crash during recursive proof generation.
    CrashDuringProof,
    /// Crash after proof completion and before successor-state persistence.
    CrashAfterProofBeforeStatePersistence,
    /// Crash after successor-state persistence and before envelope persistence.
    CrashAfterStatePersistenceBeforeEnvelope,
    /// Crash after canonical-envelope persistence and before exposure.
    CrashAfterEnvelopeBeforeExposure,
    /// Crash while irreversibly staging an inbound credit.
    CrashDuringInboxStage,
    /// Crash after inbound staging and before acknowledgement exposure.
    CrashAfterInboxStageBeforeAck,
    /// Repeated recovery reproduces the same terminal envelope and acknowledgement.
    RecoveryIdempotence,
    /// Shuffled simultaneous receiver requests remain independent.
    ShuffledConcurrentRequests,
    /// A credit committed in-window remains valid after request expiry.
    DelayedDeliveryAfterRequestExpiry,
    /// Exact duplicate delivery returns the same durable acknowledgement.
    DuplicateTransport,
    /// Reuse of a credit ID with different bytes fails closed.
    SameCreditReplay,
    /// A transition from a stale aggregate state fails closed.
    StaleState,
    /// Two successors from one predecessor fail closed.
    TwoSuccessorsFromOnePredecessor,
    /// Rollback of authenticated monetary state fails closed.
    Rollback,
    /// Rollback of trusted hardware time fails closed.
    ClockRollback,
    /// Hardware counter reuse or skipping fails closed.
    CounterReuseOrSkip,
    /// Forged hardware-epoch rotation fails closed.
    ForgedEpochRotation,
    /// Every checked asset-arithmetic overflow fails closed.
    ArithmeticOverflow,
    /// Request, payment, opening, and proof bind the same positive exact amount.
    ExactAmountBinding,
    /// Any exact-amount mismatch fails closed.
    WrongAmountRejection,
    /// Distinct valid payments against the same request are all accepted.
    DistinctPaymentsSameRequest,
    /// Concurrent distinct payments against the same request remain independently valid.
    ConcurrentPaymentsSameRequest,
    /// Invoice deduplication remains outside protocol admission.
    InvoiceDeduplicationApplicationPolicy,
    /// Proof or public-output substitution fails closed.
    ProofOutputSubstitution,
    /// Reserve underflow fails atomically.
    ReserveUnderflow,
    /// Duplicate redemption is idempotent and never debits twice.
    DuplicateRedemption,
    /// Concurrent redemption consumes each terminal nullifier once.
    ConcurrentRedemption,
    /// Top-up recovery conserves payer balance and reserve liability.
    TopUpRecovery,
    /// Full redemption produces a valid zero-balance successor.
    FullRedemption,
    /// Partial redemption preserves the exact spendable remainder.
    PartialRedemption,
    /// A zero-balance successor can later receive and spend.
    ZeroBalanceContinuation,
    /// Ordinary verifier rotation carries balance and replay state offline.
    OrdinaryVerifierRotation,
    /// Hardware-counter rollover never strands funds.
    HardwareCounterRollover,
    /// Emergency profile suspension preserves online redemption and recovery.
    EmergencySuspensionOnlineRecovery,
    /// Animated QR recovers from qualified frame loss.
    AnimatedQrLossRecovery,
    /// Animated QR recovers from qualified frame reordering.
    AnimatedQrReorderingRecovery,
    /// Static QR is admitted only for messages proven to fit.
    StaticQrSizeGuard,
    /// Four validators pass activation, restart, and replay.
    FourPeerActivationRestartReplay,
    /// Qualified devices complete end-to-end operation in airplane mode.
    PhysicalAirplaneMode,
    /// Qualified devices recover across process and operating-system restart.
    PhysicalRestart,
    /// Qualified devices recover across physical power loss.
    PhysicalPowerLoss,
    /// Physical trusted-time rollback fails closed.
    PhysicalClockRollback,
    /// Backup/restore cannot fork rollback-resistant state.
    PhysicalBackupRestoreRejection,
    /// Physical memory and latency measurements satisfy the release caps.
    PhysicalMemoryAndLatency,
    /// Physical sustained thermal folding satisfies the release caps.
    PhysicalThermalFolding,
    /// No software implementation may replace the qualified non-forking service.
    NoSoftwareFallback,
    /// Swift consumes native-core canonical fixtures without independent cryptography.
    NativeFixtureSwift,
    /// Kotlin consumes native-core canonical fixtures without independent cryptography.
    NativeFixtureKotlin,
    /// Mirrored Java consumes native-core canonical fixtures without independent cryptography.
    NativeFixtureJava,
    /// JavaScript consumes native-core canonical fixtures without independent cryptography.
    NativeFixtureJavaScript,
    /// Python consumes native-core canonical fixtures without independent cryptography.
    NativeFixturePython,
    /// C# consumes native-core canonical fixtures without independent cryptography.
    NativeFixtureCSharp,
    /// JNI consumes native-core canonical fixtures without independent cryptography.
    NativeFixtureJni,
    /// QR orchestration consumes the same native-core canonical fixtures.
    NativeFixtureQr,
    /// NFC orchestration consumes the same native-core canonical fixtures.
    NativeFixtureNfc,
}

impl KagemushaAcceptanceCaseV1 {
    /// Exact canonically ordered acceptance case set.
    pub const ALL: [Self; 60] = [
        Self::ReceiverInboxPressure,
        Self::SenderOutboxCapacityExhaustion,
        Self::CrashAfterTransitionIntentStage,
        Self::CrashDuringHardwareCommit,
        Self::CrashAfterHardwareCommitBeforeProof,
        Self::CrashDuringProof,
        Self::CrashAfterProofBeforeStatePersistence,
        Self::CrashAfterStatePersistenceBeforeEnvelope,
        Self::CrashAfterEnvelopeBeforeExposure,
        Self::CrashDuringInboxStage,
        Self::CrashAfterInboxStageBeforeAck,
        Self::RecoveryIdempotence,
        Self::ShuffledConcurrentRequests,
        Self::DelayedDeliveryAfterRequestExpiry,
        Self::DuplicateTransport,
        Self::SameCreditReplay,
        Self::StaleState,
        Self::TwoSuccessorsFromOnePredecessor,
        Self::Rollback,
        Self::ClockRollback,
        Self::CounterReuseOrSkip,
        Self::ForgedEpochRotation,
        Self::ArithmeticOverflow,
        Self::ExactAmountBinding,
        Self::WrongAmountRejection,
        Self::DistinctPaymentsSameRequest,
        Self::ConcurrentPaymentsSameRequest,
        Self::InvoiceDeduplicationApplicationPolicy,
        Self::ProofOutputSubstitution,
        Self::ReserveUnderflow,
        Self::DuplicateRedemption,
        Self::ConcurrentRedemption,
        Self::TopUpRecovery,
        Self::FullRedemption,
        Self::PartialRedemption,
        Self::ZeroBalanceContinuation,
        Self::OrdinaryVerifierRotation,
        Self::HardwareCounterRollover,
        Self::EmergencySuspensionOnlineRecovery,
        Self::AnimatedQrLossRecovery,
        Self::AnimatedQrReorderingRecovery,
        Self::StaticQrSizeGuard,
        Self::FourPeerActivationRestartReplay,
        Self::PhysicalAirplaneMode,
        Self::PhysicalRestart,
        Self::PhysicalPowerLoss,
        Self::PhysicalClockRollback,
        Self::PhysicalBackupRestoreRejection,
        Self::PhysicalMemoryAndLatency,
        Self::PhysicalThermalFolding,
        Self::NoSoftwareFallback,
        Self::NativeFixtureSwift,
        Self::NativeFixtureKotlin,
        Self::NativeFixtureJava,
        Self::NativeFixtureJavaScript,
        Self::NativeFixturePython,
        Self::NativeFixtureCSharp,
        Self::NativeFixtureJni,
        Self::NativeFixtureQr,
        Self::NativeFixtureNfc,
    ];
}
/// Evidence binding for one mandatory closed acceptance case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaAcceptanceCaseEvidenceV1 {
    /// Mandatory acceptance case.
    pub case: KagemushaAcceptanceCaseV1,
    /// Validator count for the four-peer case; zero for every device-local case.
    pub validator_count: u8,
    /// Exact report for the case.
    pub report: KagemushaEvidenceFileV1,
}

/// One independent byte-for-byte reproducible artifact build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReproducibleBuildV1 {
    /// Stable identity of the independent builder/environment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub builder_id: [u8; 32],
    /// Artifact inventory digest reproduced by this build.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_set_digest: [u8; 32],
    /// Exact reproducibility report.
    pub report: KagemushaEvidenceFileV1,
}

/// Closed, separately trusted verifier-observation projection reviewed by release authorities.
///
/// The manifest and observer policy are immutable external files. The records
/// digest covers the canonically ordered verifier identity, exact arguments,
/// stdout/stderr bindings, resource observations, and threshold approvals for
/// every typed report. Candidate-selected executables are never part of this
/// corridor. The receipt digest, and therefore every release-authority
/// signature, authenticates this complete projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaEvidenceClosureV1 {
    /// Exact immutable release-evidence manifest.
    pub evidence_manifest: KagemushaEvidenceFileV1,
    /// Exact separately selected trusted observer/verifier policy.
    pub observer_policy: KagemushaEvidenceFileV1,
    /// Domain-separated SHA-256 of the canonical ordered signed-observation projection.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub verification_records_digest: [u8; 32],
    /// Domain-separated identity of the source, lockfile, artifacts, protocols, profiles, and observer policy.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub candidate_context_digest: [u8; 32],
    /// Number of distinct signed observations represented by the digest.
    pub verification_record_count: u32,
    /// Total bytes in the closed evidence root.
    pub total_evidence_bytes: u64,
    /// Total exact stdout and stderr bytes across all observations.
    pub total_transcript_bytes: u64,
    /// Total verifier input bytes counted across all observations.
    pub total_command_input_bytes: u64,
    /// Sum of trusted wall-time observations.
    pub total_observed_duration_ms: u64,
    /// Sum of trusted CPU-time observations.
    pub total_observed_cpu_ms: u64,
}

/// Evidence produced by the internal release-qualification pipeline.
///
/// Every report hash/length below is only a provenance binding. The release
/// tool must verify each referenced file and the semantics projected into this
/// bounded typed receipt before asking authorities to sign it.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaInternalValidationReceiptV1 {
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
    /// Canonical little-endian Fp Poseidon digest of the compiled Eq protocol exercised by qualification.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Canonical little-endian Fq Poseidon digest of the compiled Ep protocol exercised by qualification.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Digest of the canonically ordered artifact inventory.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_set_digest: [u8; 32],
    /// Authenticated hardware allowlist/policy root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_policy_digest: [u8; 32],
    /// Immutable manifest, local trust policy, and threshold-signed verifier observations.
    pub evidence_closure: KagemushaEvidenceClosureV1,
    /// Circuit-shape synthesis report.
    pub circuit_shape_report: KagemushaEvidenceFileV1,
    /// Independent cryptographic review report.
    pub security_review_report: KagemushaEvidenceFileV1,
    /// Positive and adversarial known-answer report.
    pub kat_report: KagemushaEvidenceFileV1,
    /// Parser and topology fuzz report.
    pub fuzz_report: KagemushaEvidenceFileV1,
    /// Whole-process resource report.
    pub resource_report: KagemushaEvidenceFileV1,
    /// Exact qualified evidence for every enabled manifest profile.
    pub profile_qualifications: Vec<KagemushaProfileQualificationV1>,
    /// Exact helper-circuit compiled protocol identities in protocol order.
    pub helper_protocols: Vec<KagemushaHelperProtocolV1>,
    /// Independent byte-for-byte reproducible builds in builder-id order.
    pub reproducible_builds: Vec<KagemushaReproducibleBuildV1>,
    /// Bounded decoder fuzz cases represented by the report.
    pub fuzz_cases: u64,
}

/// Canonical release manifest accepted by kagemusha runtime code.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReleaseManifestV1 {
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
    /// Canonical little-endian Fp Poseidon digest of the compiled Eq protocol.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Canonical little-endian Fq Poseidon digest of the compiled Ep protocol.
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
    /// Exact helper-circuit compiled protocol identities in protocol order.
    pub helper_protocols: Vec<KagemushaHelperProtocolV1>,
    /// Strictly ordered, unique hardware profiles enabled by this release.
    pub enabled_profiles: Vec<KagemushaEnabledProfileV1>,
    /// Canonically ordered complete artifact inventory.
    pub artifacts: Vec<KagemushaArtifactBindingV1>,
}

/// Locally trusted threshold policy for Kagemusha V1 release authorities.
///
/// This policy is deployment configuration, not evidence supplied by an
/// untrusted release bundle. Callers must select the trusted policy before
/// authenticating any manifest or attestation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReleaseAuthorityPolicyV1 {
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

/// Immutable release subject approved by every Kagemusha V1 authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReleaseAttestationSubjectV1 {
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

/// Domain-separated payload signed by one Kagemusha V1 release authority.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReleaseApprovalPayloadV1 {
    /// Cross-protocol replay separator.
    pub domain: String,
    /// Exact release subject approved by the authority.
    pub subject: KagemushaReleaseAttestationSubjectV1,
}

/// One authority signature in an Kagemusha V1 release attestation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReleaseApprovalV1 {
    /// Exact key selected by the locally trusted authority policy.
    pub public_key: PublicKey,
    /// Signature over the domain-separated complete release subject.
    pub signature: SignatureOf<KagemushaReleaseApprovalPayloadV1>,
}

/// Threshold-signed Kagemusha V1 release attestation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReleaseAttestationV1 {
    /// Attestation format version.
    pub version: u16,
    /// Exact subject shared by every approval.
    pub subject: KagemushaReleaseAttestationSubjectV1,
    /// Strictly ordered, unique threshold approvals.
    pub approvals: Vec<KagemushaReleaseApprovalV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
struct KagemushaReleaseSubjectV1 {
    version: u16,
    source_tree_digest: [u8; 32],
    cargo_lock_digest: [u8; 32],
    profile_digest: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    hardware_policy_digest: [u8; 32],
    validation_receipt_digest: [u8; 32],
    halo2_k: u32,
    helper_protocols: Vec<KagemushaHelperProtocolV1>,
    enabled_profiles: Vec<KagemushaEnabledProfileV1>,
    artifacts: Vec<KagemushaArtifactBindingV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.release-vk-set-digest-subject")]
struct KagemushaVkSetSubjectV1 {
    version: u16,
    state_eq_protocol_digest: [u8; 32],
    state_ep_protocol_digest: [u8; 32],
    helper_protocols: Vec<KagemushaHelperProtocolV1>,
    verifying_keys: Vec<KagemushaArtifactBindingV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.release-artifact-set-digest-subject")]
struct KagemushaArtifactSetDigestSubjectV1 {
    artifacts: Vec<KagemushaArtifactBindingV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.release-hardware-policy-digest-subject")]
struct KagemushaHardwarePolicyDigestSubjectV1 {
    enabled_profiles: Vec<KagemushaEnabledProfileV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.release-profile-qualification-digest-subject")]
struct KagemushaProfileQualificationDigestSubjectV1 {
    qualification: KagemushaProfileQualificationV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.release-profile-digest-subject")]
struct KagemushaReleaseProfileDigestSubjectV1 {
    version: u16,
    halo2_k: u32,
    circuit_shape_report: KagemushaEvidenceFileV1,
    state_eq_protocol_digest: [u8; 32],
    state_ep_protocol_digest: [u8; 32],
    helper_protocols: Vec<KagemushaHelperProtocolV1>,
}

/// Structural or evidence failure while authenticating an kagemusha release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaReleaseErrorV1 {
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

impl core::fmt::Display for KagemushaReleaseErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Encode => "canonical kagemusha encoding failed",
            Self::InvalidManifest => "invalid kagemusha release manifest",
            Self::InvalidArtifactSet => "invalid kagemusha artifact set",
            Self::InvalidValidationReceipt => "invalid kagemusha validation receipt",
            Self::InvalidAuthorityPolicy => "invalid kagemusha release authority policy",
            Self::InvalidAttestation => "invalid kagemusha release attestation",
            Self::UnknownSigner => "unknown kagemusha release authority",
            Self::DuplicateOrUnorderedSigner => {
                "duplicate or unordered kagemusha release authority"
            }
            Self::InvalidSignature => "invalid kagemusha release authority signature",
            Self::InsufficientThreshold { .. } => {
                "insufficient kagemusha release authority threshold"
            }
        })
    }
}

impl std::error::Error for KagemushaReleaseErrorV1 {}

/// Runtime capability created only after manifest and evidence authentication.
#[derive(Debug, PartialEq, Eq)]
pub struct KagemushaAuthenticatedReleaseV1 {
    manifest: KagemushaReleaseManifestV1,
    manifest_digest: [u8; 32],
    receipt_digest: [u8; 32],
    authority_policy_digest: [u8; 32],
    attestation_digest: [u8; 32],
    approved_signers: Vec<PublicKey>,
}

fn digest_encoded<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
    let bytes = norito::encode_canonical(value).map_err(|_| KagemushaReleaseErrorV1::Encode)?;
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

/// Derive the exact suite commitment used by governed hardware profiles.
#[must_use]
pub fn kagemusha_suite_commitment_v1(suite_id: [u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SUITE_COMMITMENT_DOMAIN);
    hasher.update([0]);
    hasher.update(
        u64::try_from(suite_id.len())
            .expect("suite identity length fits u64")
            .to_le_bytes(),
    );
    hasher.update(suite_id);
    hasher.finalize().into()
}

fn validate_evidence_file(file: KagemushaEvidenceFileV1) -> bool {
    digest_is_nonzero(file.sha256)
        && file.byte_len != 0
        && file.byte_len <= KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1
}

fn validate_evidence_closure(closure: KagemushaEvidenceClosureV1) -> bool {
    validate_evidence_file(closure.evidence_manifest)
        && validate_evidence_file(closure.observer_policy)
        && closure.evidence_manifest.sha256 != closure.observer_policy.sha256
        && digest_is_nonzero(closure.verification_records_digest)
        && digest_is_nonzero(closure.candidate_context_digest)
        && closure.verification_record_count != 0
        && closure.verification_record_count <= KAGEMUSHA_RELEASE_VERIFICATION_RECORD_MAX_COUNT_V1
        && closure.total_evidence_bytes != 0
        && closure.total_evidence_bytes <= KAGEMUSHA_RELEASE_EVIDENCE_TOTAL_MAX_BYTES_V1
        && closure.total_transcript_bytes != 0
        && closure.total_transcript_bytes <= KAGEMUSHA_RELEASE_TRANSCRIPT_TOTAL_MAX_BYTES_V1
        && closure.total_command_input_bytes != 0
        && closure.total_command_input_bytes <= KAGEMUSHA_RELEASE_COMMAND_INPUT_TOTAL_MAX_BYTES_V1
        && closure.total_observed_duration_ms != 0
        && closure.total_observed_duration_ms <= KAGEMUSHA_RELEASE_OBSERVED_TIME_TOTAL_MAX_MS_V1
        && closure.total_observed_cpu_ms != 0
        && closure.total_observed_cpu_ms <= KAGEMUSHA_RELEASE_OBSERVED_TIME_TOTAL_MAX_MS_V1
}

fn validate_enabled_profile(profile: KagemushaEnabledProfileV1) -> bool {
    profile.hardware_profile.validate().is_ok()
        && digest_is_nonzero(profile.hardware_profile_id)
        && digest_is_nonzero(profile.suite_id)
        && digest_is_nonzero(profile.vk_digest)
        && digest_is_nonzero(profile.qualification_digest)
        && profile.policy_epoch != 0
        && profile.hardware_profile_id == profile.hardware_profile.hardware_profile_id
        && profile.policy_epoch == profile.hardware_profile.policy_epoch
        && kagemusha_suite_commitment_v1(profile.suite_id)
            == profile.hardware_profile.allowed_suite_commitment
        && validate_evidence_file(profile.qualification_report)
        && profile.qualification_report.sha256
            == profile.hardware_profile.qualification_report_digest
}

fn validate_helper_protocols(protocols: &[KagemushaHelperProtocolV1]) -> bool {
    protocols.len() == KagemushaQualifiedHelperCircuitV1::ALL.len()
        && protocols
            .iter()
            .zip(KagemushaQualifiedHelperCircuitV1::ALL)
            .all(|(protocol, expected)| {
                protocol.helper == expected
                    && digest_is_nonzero(protocol.eq_protocol_digest)
                    && digest_is_nonzero(protocol.ep_protocol_digest)
                    && protocol.eq_protocol_digest != protocol.ep_protocol_digest
                    && if expected.uses_internal_proof_evidence() {
                        valid_internal_helper_proof_length(protocol.eq_proof_bytes)
                            && valid_internal_helper_proof_length(protocol.ep_proof_bytes)
                            && protocol
                                .eq_proof_bytes
                                .checked_add(protocol.ep_proof_bytes)
                                .is_some()
                    } else {
                        protocol.eq_proof_bytes == 0 && protocol.ep_proof_bytes == 0
                    }
            })
}

const fn valid_internal_helper_proof_length(length: u32) -> bool {
    length != 0
        && length <= KAGEMUSHA_INTERNAL_HELPER_PROOF_EVIDENCE_MAX_BYTES_V1
        && length % 32 == 0
}

fn protocol_digests_are_unique(
    state_eq: [u8; 32],
    state_ep: [u8; 32],
    helpers: &[KagemushaHelperProtocolV1],
) -> bool {
    let mut digests = Vec::with_capacity(2 + 2 * helpers.len());
    digests.extend([state_eq, state_ep]);
    for helper in helpers {
        digests.extend([helper.eq_protocol_digest, helper.ep_protocol_digest]);
    }
    digests
        .iter()
        .enumerate()
        .all(|(index, digest)| digest_is_nonzero(*digest) && !digests[index + 1..].contains(digest))
}

fn validate_enabled_profiles(
    profiles: &[KagemushaEnabledProfileV1],
) -> Result<(), KagemushaReleaseErrorV1> {
    if profiles.is_empty()
        || profiles.len() > KAGEMUSHA_RELEASE_MAX_ENABLED_PROFILES_V1
        || profiles
            .iter()
            .copied()
            .any(|profile| !validate_enabled_profile(profile))
        || !profiles
            .windows(2)
            .all(|pair| pair[0].hardware_profile_id < pair[1].hardware_profile_id)
    {
        return Err(KagemushaReleaseErrorV1::InvalidManifest);
    }
    Ok(())
}

/// Decode one exact canonical release frame only after its complete outer byte
/// length is known to be within the type-specific admission cap.
fn decode_release_bounded_canonical<T>(
    bytes: &[u8],
    max: usize,
    invalid: KagemushaReleaseErrorV1,
) -> Result<T, KagemushaReleaseErrorV1>
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
pub fn kagemusha_artifact_set_digest_v1(
    artifacts: &[KagemushaArtifactBindingV1],
) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
    validate_artifacts(artifacts)?;
    digest_encoded(
        ARTIFACT_SET_DIGEST_DOMAIN,
        &KagemushaArtifactSetDigestSubjectV1 {
            artifacts: artifacts.to_vec(),
        },
    )
}

/// Derive the authenticated verifier-set identity for one release.
///
/// The digest binds every VK role, exact VK file SHA-256 and length, and every
/// state and helper compiled-protocol identity. It does not claim
/// that a file hash alone demonstrates circuit semantics; immutable release
/// tooling must verify the exact artifact bytes before signing the manifest.
///
/// # Errors
///
/// Returns an error for an invalid artifact inventory, incomplete helper
/// protocol set, reused protocol identity, or canonical encoding failure.
pub fn kagemusha_vk_set_digest_v1(
    artifacts: &[KagemushaArtifactBindingV1],
    state_eq_protocol_digest: [u8; 32],
    state_ep_protocol_digest: [u8; 32],
    helper_protocols: &[KagemushaHelperProtocolV1],
) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
    validate_artifacts(artifacts)?;
    if !validate_helper_protocols(helper_protocols)
        || !protocol_digests_are_unique(
            state_eq_protocol_digest,
            state_ep_protocol_digest,
            helper_protocols,
        )
    {
        return Err(KagemushaReleaseErrorV1::InvalidManifest);
    }
    let verifying_keys = artifacts
        .iter()
        .copied()
        .filter(|artifact| artifact.role.is_vk())
        .collect();
    digest_encoded(
        VK_SET_DIGEST_DOMAIN,
        &KagemushaVkSetSubjectV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            state_eq_protocol_digest,
            state_ep_protocol_digest,
            helper_protocols: helper_protocols.to_vec(),
            verifying_keys,
        },
    )
}

/// Derive the hardware-policy identity from the exact canonical enabled set.
///
/// # Errors
///
/// Returns an error when the set is empty, oversized, unordered, duplicated,
/// contains an invalid embedded hardware profile, or cannot be encoded.
pub fn kagemusha_hardware_policy_digest_v1(
    enabled_profiles: &[KagemushaEnabledProfileV1],
) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
    validate_enabled_profiles(enabled_profiles)?;
    digest_encoded(
        HARDWARE_POLICY_DIGEST_DOMAIN,
        &KagemushaHardwarePolicyDigestSubjectV1 {
            enabled_profiles: enabled_profiles.to_vec(),
        },
    )
}

/// Derive the identity of one profile's complete typed qualification matrix.
///
/// The digest is computed with the embedded `qualification_digest` field set
/// to zero, avoiding self-reference while binding every other profile,
/// relation, helper, depth, capacity, lifecycle, transport, and report field.
///
/// # Errors
///
/// Returns an error when canonical encoding fails.
pub fn kagemusha_profile_qualification_digest_v1(
    qualification: &KagemushaProfileQualificationV1,
) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
    let mut subject = qualification.clone();
    subject.profile.qualification_digest = [0; 32];
    digest_encoded(
        PROFILE_QUALIFICATION_DIGEST_DOMAIN,
        &KagemushaProfileQualificationDigestSubjectV1 {
            qualification: subject,
        },
    )
}

/// Derive the exact circuit/profile identity projected by release evidence.
///
/// # Errors
///
/// Returns an error when helper protocols are incomplete, protocol identities
/// are reused, the shape report is invalid, or canonical encoding fails.
pub fn kagemusha_release_profile_digest_v1(
    circuit_shape_report: KagemushaEvidenceFileV1,
    state_eq_protocol_digest: [u8; 32],
    state_ep_protocol_digest: [u8; 32],
    helper_protocols: &[KagemushaHelperProtocolV1],
) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
    if !validate_evidence_file(circuit_shape_report)
        || !validate_helper_protocols(helper_protocols)
        || !protocol_digests_are_unique(
            state_eq_protocol_digest,
            state_ep_protocol_digest,
            helper_protocols,
        )
    {
        return Err(KagemushaReleaseErrorV1::InvalidValidationReceipt);
    }
    digest_encoded(
        RELEASE_PROFILE_DIGEST_DOMAIN,
        &KagemushaReleaseProfileDigestSubjectV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            halo2_k: KAGEMUSHA_HALO2_K_V1,
            circuit_shape_report,
            state_eq_protocol_digest,
            state_ep_protocol_digest,
            helper_protocols: helper_protocols.to_vec(),
        },
    )
}

fn validate_artifacts(
    artifacts: &[KagemushaArtifactBindingV1],
) -> Result<(), KagemushaReleaseErrorV1> {
    if artifacts.len() != KagemushaArtifactRoleV1::ALL.len() {
        return Err(KagemushaReleaseErrorV1::InvalidArtifactSet);
    }
    let mut total = 0_u64;
    for (artifact, expected_role) in artifacts
        .iter()
        .zip(KagemushaArtifactRoleV1::ALL.iter().copied())
    {
        if artifact.role != expected_role || !digest_is_nonzero(artifact.sha256) {
            return Err(KagemushaReleaseErrorV1::InvalidArtifactSet);
        }
        let valid_size = if artifact.role.is_params() {
            artifact.byte_len == KAGEMUSHA_PARAMS_BYTES_V1
        } else if artifact.role.is_state_pk() {
            artifact.byte_len != 0 && artifact.byte_len <= KAGEMUSHA_STATE_PROVING_KEY_MAX_BYTES_V1
        } else if artifact.role.is_helper_pk() {
            artifact.byte_len != 0 && artifact.byte_len <= KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1
        } else if artifact.role.is_vk() {
            artifact.byte_len != 0 && artifact.byte_len <= KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1
        } else {
            false
        };
        if !valid_size {
            return Err(KagemushaReleaseErrorV1::InvalidArtifactSet);
        }
        total = total
            .checked_add(artifact.byte_len)
            .ok_or(KagemushaReleaseErrorV1::InvalidArtifactSet)?;
    }
    if total > KAGEMUSHA_ARTIFACT_SET_MAX_BYTES_V1 {
        return Err(KagemushaReleaseErrorV1::InvalidArtifactSet);
    }
    if artifacts.iter().enumerate().any(|(index, artifact)| {
        artifacts[index + 1..]
            .iter()
            .any(|other| artifact.sha256 == other.sha256)
    }) {
        return Err(KagemushaReleaseErrorV1::InvalidArtifactSet);
    }
    Ok(())
}

fn validate_vk_reference(
    binding: KagemushaArtifactBindingV1,
    expected_role: KagemushaArtifactRoleV1,
) -> bool {
    binding.role == expected_role
        && digest_is_nonzero(binding.sha256)
        && binding.byte_len != 0
        && binding.byte_len <= KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1
}

impl KagemushaProfileQualificationV1 {
    /// Compute the digest bound by the manifest's enabled-profile record.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_qualification_digest(&self) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
        kagemusha_profile_qualification_digest_v1(self)
    }

    /// Populate the digest of this exact typed profile qualification.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal_qualification_digest(mut self) -> Result<Self, KagemushaReleaseErrorV1> {
        self.profile.qualification_digest = self.expected_qualification_digest()?;
        Ok(self)
    }
}

fn validate_profile_qualification(
    qualification: &KagemushaProfileQualificationV1,
    state_eq_protocol_digest: [u8; 32],
    state_ep_protocol_digest: [u8; 32],
    helper_protocols: &[KagemushaHelperProtocolV1],
) -> Result<(), KagemushaReleaseErrorV1> {
    let invalid = || KagemushaReleaseErrorV1::InvalidValidationReceipt;
    let maximum_circuit_rows = 1_u32
        .checked_shl(KAGEMUSHA_HALO2_K_V1)
        .ok_or_else(invalid)?;
    let proof_max =
        u32::try_from(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1).expect("proof maximum fits u32");
    let raw_session_max =
        u32::try_from(KAGEMUSHA_SESSION_MAX_BYTES_V1).expect("raw-session maximum fits u32");
    let text_session_max =
        u32::try_from(KAGEMUSHA_TEXT_SESSION_MAX_BYTES_V1).expect("text-session maximum fits u32");
    if !validate_enabled_profile(qualification.profile)
        || qualification.relations.len() != KagemushaQualifiedRelationV1::ALL.len()
        || qualification.helper_circuits.len() != KagemushaQualifiedHelperCircuitV1::ALL.len()
        || qualification.profile.qualification_digest
            != qualification
                .expected_qualification_digest()
                .map_err(|_| invalid())?
    {
        return Err(invalid());
    }
    for (relation, expected_relation) in qualification
        .relations
        .iter()
        .zip(KagemushaQualifiedRelationV1::ALL.iter().copied())
    {
        let (expected_eq_vk, expected_ep_vk) = expected_relation.expected_vk_roles();
        let (expected_eq_protocol, expected_ep_protocol) =
            (state_eq_protocol_digest, state_ep_protocol_digest);
        if relation.relation != expected_relation
            || relation.eq_protocol_digest != expected_eq_protocol
            || relation.ep_protocol_digest != expected_ep_protocol
            || !validate_vk_reference(relation.eq_verifying_key, expected_eq_vk)
            || !validate_vk_reference(relation.ep_verifying_key, expected_ep_vk)
            || relation.eq_verifying_key.sha256 == relation.ep_verifying_key.sha256
            || relation.eq_circuit_rows == 0
            || relation.eq_circuit_rows > maximum_circuit_rows
            || relation.ep_circuit_rows == 0
            || relation.ep_circuit_rows > maximum_circuit_rows
            || relation.complete_proof_bytes == 0
            || relation.complete_proof_bytes > proof_max
            || relation.prove_p95_ms == 0
            || relation.prove_p95_ms > KAGEMUSHA_PROVE_P95_MAX_MS_V1
            || relation.verify_p95_ms == 0
            || relation.verify_p95_ms > KAGEMUSHA_VERIFY_P95_MAX_MS_V1
            || relation.process_rss_bytes == 0
            || relation.process_rss_bytes > KAGEMUSHA_PROCESS_RSS_MAX_BYTES_V1
            || relation.operation_energy_millijoules == 0
            || !validate_evidence_file(relation.report)
        {
            return Err(invalid());
        }
    }

    for ((helper, protocol), expected_helper) in qualification
        .helper_circuits
        .iter()
        .zip(helper_protocols)
        .zip(KagemushaQualifiedHelperCircuitV1::ALL)
    {
        let (expected_eq_vk, expected_ep_vk) = expected_helper.expected_vk_roles();
        let valid_proof_measurement = if expected_helper.uses_internal_proof_evidence() {
            helper.eq_proof_bytes == protocol.eq_proof_bytes
                && helper.ep_proof_bytes == protocol.ep_proof_bytes
                && helper.eq_proof_bytes.checked_add(helper.ep_proof_bytes)
                    == Some(helper.complete_proof_bytes)
        } else {
            helper.eq_proof_bytes == 0
                && helper.ep_proof_bytes == 0
                && helper.complete_proof_bytes != 0
                && helper.complete_proof_bytes <= proof_max
        };
        if helper.helper != expected_helper
            || protocol.helper != expected_helper
            || helper.eq_protocol_digest != protocol.eq_protocol_digest
            || helper.ep_protocol_digest != protocol.ep_protocol_digest
            || !validate_vk_reference(helper.eq_verifying_key, expected_eq_vk)
            || !validate_vk_reference(helper.ep_verifying_key, expected_ep_vk)
            || helper.eq_verifying_key.sha256 == helper.ep_verifying_key.sha256
            || helper.eq_circuit_rows == 0
            || helper.eq_circuit_rows > maximum_circuit_rows
            || helper.ep_circuit_rows == 0
            || helper.ep_circuit_rows > maximum_circuit_rows
            || !valid_proof_measurement
            || helper.prove_p95_ms == 0
            || helper.prove_p95_ms > KAGEMUSHA_PROVE_P95_MAX_MS_V1
            || helper.verify_p95_ms == 0
            || helper.verify_p95_ms > KAGEMUSHA_VERIFY_P95_MAX_MS_V1
            || helper.process_rss_bytes == 0
            || helper.process_rss_bytes > KAGEMUSHA_PROCESS_RSS_MAX_BYTES_V1
            || helper.operation_energy_millijoules == 0
            || !validate_evidence_file(helper.report)
        {
            return Err(invalid());
        }
    }

    if qualification.recursive_depths.len() != 4 {
        return Err(invalid());
    }
    let invariant_depth_sizes = (
        qualification.recursive_depths[0].complete_proof_bytes,
        qualification.recursive_depths[0].raw_session_bytes,
        qualification.recursive_depths[0].text_session_bytes,
    );
    for depth in &qualification.recursive_depths {
        if depth.depth == 0
            || depth.verified_handoffs != depth.depth
            || depth.complete_proof_bytes == 0
            || depth.complete_proof_bytes > proof_max
            || depth.raw_session_bytes == 0
            || depth.raw_session_bytes > raw_session_max
            || depth.text_session_bytes == 0
            || depth.text_session_bytes > text_session_max
            || (
                depth.complete_proof_bytes,
                depth.raw_session_bytes,
                depth.text_session_bytes,
            ) != invariant_depth_sizes
            || !validate_evidence_file(depth.report)
        {
            return Err(invalid());
        }
    }
    if qualification.recursive_depths[0].depth != 8
        || qualification.recursive_depths[1].depth != 64
        || qualification.recursive_depths[2].depth != KAGEMUSHA_MIN_QUALIFIED_HANDOFFS_V1
        || qualification.recursive_depths[3].depth <= KAGEMUSHA_MIN_QUALIFIED_HANDOFFS_V1
    {
        return Err(invalid());
    }

    let aggregate = qualification.aggregate_balance;
    if aggregate.independent_payments < KAGEMUSHA_MIN_QUALIFIED_AGGREGATED_CREDITS_V1
        || aggregate.folded_credits != aggregate.independent_payments
        || aggregate.spend_payments != 1
        || !validate_evidence_file(aggregate.report)
    {
        return Err(invalid());
    }
    let thermal = qualification.thermal;
    if thermal.folded_credits < KAGEMUSHA_MIN_THERMAL_FOLDED_CREDITS_V1
        || thermal.fold_p95_ms == 0
        || thermal.fold_p95_ms > KAGEMUSHA_PROVE_P95_MAX_MS_V1
        || thermal.process_rss_bytes == 0
        || thermal.process_rss_bytes > KAGEMUSHA_PROCESS_RSS_MAX_BYTES_V1
        || thermal.operation_energy_millijoules == 0
        || !validate_evidence_file(thermal.report)
    {
        return Err(invalid());
    }
    let envelope = qualification.envelope;
    if envelope.raw_session_bytes == 0
        || envelope.raw_session_bytes > raw_session_max
        || envelope.text_session_bytes == 0
        || envelope.text_session_bytes > text_session_max
        || envelope.raw_session_bytes != invariant_depth_sizes.1
        || envelope.text_session_bytes != invariant_depth_sizes.2
        || envelope.handoff_p95_ms == 0
        || envelope.handoff_p95_ms > KAGEMUSHA_HANDOFF_P95_MAX_MS_V1
        || !validate_evidence_file(envelope.report)
    {
        return Err(invalid());
    }
    if qualification.acceptance_cases.len() != KagemushaAcceptanceCaseV1::ALL.len() {
        return Err(invalid());
    }
    for (evidence, expected_case) in qualification
        .acceptance_cases
        .iter()
        .zip(KagemushaAcceptanceCaseV1::ALL)
    {
        let expected_validator_count = if matches!(
            expected_case,
            KagemushaAcceptanceCaseV1::FourPeerActivationRestartReplay
        ) {
            KAGEMUSHA_VALIDATOR_COUNT_V1
        } else {
            0
        };
        if evidence.case != expected_case
            || evidence.validator_count != expected_validator_count
            || !validate_evidence_file(evidence.report)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

impl KagemushaInternalValidationReceiptV1 {
    /// Decode and validate one exact bounded qualification receipt.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, malformed, non-canonical, or invalid receipt.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaReleaseErrorV1> {
        let receipt: Self = decode_release_bounded_canonical(
            bytes,
            KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
            KagemushaReleaseErrorV1::InvalidValidationReceipt,
        )?;
        receipt.validate()?;
        Ok(receipt)
    }

    /// Validate all evidence identities and numeric release thresholds.
    ///
    /// # Errors
    ///
    /// Returns an error when evidence is absent or any measured result exceeds its release limit.
    pub fn validate(&self) -> Result<(), KagemushaReleaseErrorV1> {
        let enabled_profiles: Vec<_> = self
            .profile_qualifications
            .iter()
            .map(|qualification| qualification.profile)
            .collect();
        let expected_hardware_policy_digest =
            kagemusha_hardware_policy_digest_v1(&enabled_profiles)
                .map_err(|_| KagemushaReleaseErrorV1::InvalidValidationReceipt)?;
        let expected_vk_digest = self
            .profile_qualifications
            .first()
            .map(|qualification| qualification.profile.vk_digest)
            .ok_or(KagemushaReleaseErrorV1::InvalidValidationReceipt)?;
        let expected_profile_digest = kagemusha_release_profile_digest_v1(
            self.circuit_shape_report,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            &self.helper_protocols,
        )?;
        let digests = [
            self.source_tree_digest,
            self.cargo_lock_digest,
            self.profile_digest,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            self.artifact_set_digest,
            self.hardware_policy_digest,
        ];
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || digests.into_iter().any(|digest| !digest_is_nonzero(digest))
            || !validate_helper_protocols(&self.helper_protocols)
            || !protocol_digests_are_unique(
                self.eq_protocol_digest,
                self.ep_protocol_digest,
                &self.helper_protocols,
            )
            || self.hardware_policy_digest != expected_hardware_policy_digest
            || self.profile_digest != expected_profile_digest
            || !validate_evidence_closure(self.evidence_closure)
            || self
                .profile_qualifications
                .iter()
                .any(|qualification| qualification.profile.vk_digest != expected_vk_digest)
            || !validate_evidence_file(self.circuit_shape_report)
            || !validate_evidence_file(self.security_review_report)
            || !validate_evidence_file(self.kat_report)
            || !validate_evidence_file(self.fuzz_report)
            || !validate_evidence_file(self.resource_report)
            || self.fuzz_cases < KAGEMUSHA_MIN_FUZZ_CASES_V1
            || self.profile_qualifications.is_empty()
            || self.profile_qualifications.len() > KAGEMUSHA_RELEASE_MAX_ENABLED_PROFILES_V1
            || !self.profile_qualifications.windows(2).all(|pair| {
                pair[0].profile.hardware_profile_id < pair[1].profile.hardware_profile_id
            })
            || self.reproducible_builds.len() < usize::from(KAGEMUSHA_REPRODUCIBLE_BUILD_COUNT_V1)
            || self.reproducible_builds.len() > KAGEMUSHA_RELEASE_MAX_REPRODUCIBLE_BUILDS_V1
            || !self
                .reproducible_builds
                .windows(2)
                .all(|pair| pair[0].builder_id < pair[1].builder_id)
        {
            return Err(KagemushaReleaseErrorV1::InvalidValidationReceipt);
        }
        for qualification in &self.profile_qualifications {
            validate_profile_qualification(
                qualification,
                self.eq_protocol_digest,
                self.ep_protocol_digest,
                &self.helper_protocols,
            )?;
        }
        for build in &self.reproducible_builds {
            if !digest_is_nonzero(build.builder_id)
                || build.artifact_set_digest != self.artifact_set_digest
                || !validate_evidence_file(build.report)
            {
                return Err(KagemushaReleaseErrorV1::InvalidValidationReceipt);
            }
        }
        let encoded = norito::encode_canonical(self)
            .map_err(|_| KagemushaReleaseErrorV1::InvalidValidationReceipt)?;
        if encoded.len() > KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1 {
            return Err(KagemushaReleaseErrorV1::InvalidValidationReceipt);
        }
        Ok(())
    }

    /// Return the SHA-256 identity pinned by a release manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the receipt is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
        self.validate()?;
        digest_encoded(RECEIPT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaReleaseAuthorityPolicyV1 {
    /// Decode and validate one exact bounded trusted authority policy.
    ///
    /// The complete byte cap is enforced before Norito reads a header or any
    /// attacker-declared collection length.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, malformed, non-canonical, or
    /// structurally invalid policy.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaReleaseErrorV1> {
        let policy: Self = decode_release_bounded_canonical(
            bytes,
            KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
            KagemushaReleaseErrorV1::InvalidAuthorityPolicy,
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
    pub fn validate(&self) -> Result<(), KagemushaReleaseErrorV1> {
        let signer_count = self.authorized_signers.len();
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || !digest_is_nonzero(self.authority_set_id)
            || signer_count == 0
            || signer_count > KAGEMUSHA_RELEASE_AUTHORITY_MAX_SIGNERS_V1
            || self.threshold == 0
            || usize::from(self.threshold) > signer_count
            || !self
                .authorized_signers
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return Err(KagemushaReleaseErrorV1::InvalidAuthorityPolicy);
        }
        Ok(())
    }

    /// Return the canonical identity bound into every release attestation.
    ///
    /// # Errors
    ///
    /// Returns an error when the policy is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
        self.validate()?;
        digest_encoded(AUTHORITY_POLICY_DIGEST_DOMAIN, self)
    }
}

impl KagemushaReleaseAttestationSubjectV1 {
    /// Return the exact domain-separated value signed by each authority.
    #[must_use]
    pub fn approval_payload(&self) -> KagemushaReleaseApprovalPayloadV1 {
        KagemushaReleaseApprovalPayloadV1 {
            domain: RELEASE_APPROVAL_DOMAIN.to_owned(),
            subject: *self,
        }
    }
}

impl KagemushaReleaseAttestationV1 {
    /// Decode and structurally validate one exact bounded release attestation.
    ///
    /// Signature and trusted-policy verification still occur only through
    /// [`KagemushaReleaseManifestV1::authenticate`]. The complete byte cap
    /// is enforced before Norito reads a header or any declared collection
    /// length.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, malformed, non-canonical, or
    /// structurally invalid attestation.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaReleaseErrorV1> {
        let attestation: Self = decode_release_bounded_canonical(
            bytes,
            KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1,
            KagemushaReleaseErrorV1::InvalidAttestation,
        )?;
        attestation.validate_standalone()?;
        Ok(attestation)
    }

    fn validate_standalone(&self) -> Result<(), KagemushaReleaseErrorV1> {
        let subject_digests = [
            self.subject.authority_policy_digest,
            self.subject.release_id,
            self.subject.manifest_digest,
            self.subject.validation_receipt_digest,
            self.subject.artifact_set_digest,
        ];
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.subject.version != KAGEMUSHA_WIRE_VERSION_V1
            || subject_digests
                .into_iter()
                .any(|digest| !digest_is_nonzero(digest))
            || self.approvals.is_empty()
            || self.approvals.len() > KAGEMUSHA_RELEASE_AUTHORITY_MAX_SIGNERS_V1
        {
            return Err(KagemushaReleaseErrorV1::InvalidAttestation);
        }
        if !self
            .approvals
            .windows(2)
            .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(KagemushaReleaseErrorV1::DuplicateOrUnorderedSigner);
        }
        Ok(())
    }
}

impl KagemushaReleaseManifestV1 {
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
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaReleaseErrorV1> {
        let manifest: Self = decode_release_bounded_canonical(
            bytes,
            KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
            KagemushaReleaseErrorV1::InvalidManifest,
        )?;
        manifest.validate_standalone()?;
        Ok(manifest)
    }

    fn validate_standalone(&self) -> Result<(), KagemushaReleaseErrorV1> {
        validate_artifacts(&self.artifacts)?;
        validate_enabled_profiles(&self.enabled_profiles)?;
        let expected_vk_digest = kagemusha_vk_set_digest_v1(
            &self.artifacts,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            &self.helper_protocols,
        )?;
        let expected_hardware_policy_digest =
            kagemusha_hardware_policy_digest_v1(&self.enabled_profiles)?;
        let encoded =
            norito::encode_canonical(self).map_err(|_| KagemushaReleaseErrorV1::InvalidManifest)?;
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
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.halo2_k != KAGEMUSHA_HALO2_K_V1
            || digests.into_iter().any(|digest| !digest_is_nonzero(digest))
            || !validate_helper_protocols(&self.helper_protocols)
            || !protocol_digests_are_unique(
                self.eq_protocol_digest,
                self.ep_protocol_digest,
                &self.helper_protocols,
            )
            || self.hardware_policy_digest != expected_hardware_policy_digest
            || self
                .enabled_profiles
                .iter()
                .any(|profile| profile.vk_digest != expected_vk_digest)
            || encoded.len() > KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1
            || self.release_id != self.expected_release_id()?
        {
            return Err(KagemushaReleaseErrorV1::InvalidManifest);
        }
        Ok(())
    }

    fn subject(&self) -> KagemushaReleaseSubjectV1 {
        KagemushaReleaseSubjectV1 {
            version: self.version,
            source_tree_digest: self.source_tree_digest,
            cargo_lock_digest: self.cargo_lock_digest,
            profile_digest: self.profile_digest,
            eq_protocol_digest: self.eq_protocol_digest,
            ep_protocol_digest: self.ep_protocol_digest,
            hardware_policy_digest: self.hardware_policy_digest,
            validation_receipt_digest: self.validation_receipt_digest,
            halo2_k: self.halo2_k,
            helper_protocols: self.helper_protocols.clone(),
            enabled_profiles: self.enabled_profiles.clone(),
            artifacts: self.artifacts.clone(),
        }
    }

    /// Compute the digest-derived release identifier from every authoritative field.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_release_id(&self) -> Result<[u8; 32], KagemushaReleaseErrorV1> {
        digest_encoded(RELEASE_ID_DOMAIN, &self.subject())
    }

    /// Populate the digest-derived release identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal(mut self) -> Result<Self, KagemushaReleaseErrorV1> {
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
        receipt: &KagemushaInternalValidationReceiptV1,
        policy: &KagemushaReleaseAuthorityPolicyV1,
    ) -> Result<KagemushaReleaseAttestationSubjectV1, KagemushaReleaseErrorV1> {
        self.validate_standalone()?;
        receipt.validate()?;
        let authority_policy_digest = policy.canonical_digest()?;
        let artifact_set_digest = kagemusha_artifact_set_digest_v1(&self.artifacts)?;
        let receipt_digest = receipt.canonical_digest()?;
        let manifest_digest = digest_encoded(MANIFEST_DIGEST_DOMAIN, self)?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.halo2_k != KAGEMUSHA_HALO2_K_V1
            || !digest_is_nonzero(self.release_id)
            || self.release_id != self.expected_release_id()?
            || self.source_tree_digest != receipt.source_tree_digest
            || self.cargo_lock_digest != receipt.cargo_lock_digest
            || self.profile_digest != receipt.profile_digest
            || self.eq_protocol_digest != receipt.eq_protocol_digest
            || self.ep_protocol_digest != receipt.ep_protocol_digest
            || self.helper_protocols != receipt.helper_protocols
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.hardware_policy_digest != receipt.hardware_policy_digest
            || self.validation_receipt_digest != receipt_digest
            || receipt.artifact_set_digest != artifact_set_digest
            || self.enabled_profiles.len() != receipt.profile_qualifications.len()
            || !self
                .enabled_profiles
                .iter()
                .zip(&receipt.profile_qualifications)
                .all(|(enabled, qualification)| *enabled == qualification.profile)
        {
            return Err(KagemushaReleaseErrorV1::InvalidManifest);
        }
        for qualification in &receipt.profile_qualifications {
            for relation in &qualification.relations {
                let (eq_role, ep_role) = relation.relation.expected_vk_roles();
                let (eq_protocol_digest, ep_protocol_digest) =
                    (self.eq_protocol_digest, self.ep_protocol_digest);
                let eq_artifact = self
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.role == eq_role)
                    .ok_or(KagemushaReleaseErrorV1::InvalidArtifactSet)?;
                let ep_artifact = self
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.role == ep_role)
                    .ok_or(KagemushaReleaseErrorV1::InvalidArtifactSet)?;
                if relation.eq_verifying_key != *eq_artifact
                    || relation.ep_verifying_key != *ep_artifact
                    || relation.eq_protocol_digest != eq_protocol_digest
                    || relation.ep_protocol_digest != ep_protocol_digest
                {
                    return Err(KagemushaReleaseErrorV1::InvalidManifest);
                }
            }
            for (helper, protocol) in qualification
                .helper_circuits
                .iter()
                .zip(&self.helper_protocols)
            {
                let (eq_role, ep_role) = helper.helper.expected_vk_roles();
                let eq_artifact = self
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.role == eq_role)
                    .ok_or(KagemushaReleaseErrorV1::InvalidArtifactSet)?;
                let ep_artifact = self
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.role == ep_role)
                    .ok_or(KagemushaReleaseErrorV1::InvalidArtifactSet)?;
                if helper.helper != protocol.helper
                    || helper.eq_verifying_key != *eq_artifact
                    || helper.ep_verifying_key != *ep_artifact
                    || helper.eq_protocol_digest != protocol.eq_protocol_digest
                    || helper.ep_protocol_digest != protocol.ep_protocol_digest
                {
                    return Err(KagemushaReleaseErrorV1::InvalidManifest);
                }
            }
        }
        Ok(KagemushaReleaseAttestationSubjectV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
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
        receipt: &KagemushaInternalValidationReceiptV1,
        policy: &KagemushaReleaseAuthorityPolicyV1,
        attestation: &KagemushaReleaseAttestationV1,
    ) -> Result<KagemushaAuthenticatedReleaseV1, KagemushaReleaseErrorV1> {
        let expected_subject = self.release_attestation_subject(receipt, policy)?;
        if attestation.version != KAGEMUSHA_WIRE_VERSION_V1
            || attestation.subject != expected_subject
            || attestation.approvals.len() > policy.authorized_signers.len()
        {
            return Err(KagemushaReleaseErrorV1::InvalidAttestation);
        }
        if !attestation
            .approvals
            .windows(2)
            .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(KagemushaReleaseErrorV1::DuplicateOrUnorderedSigner);
        }
        let payload = expected_subject.approval_payload();
        let mut approved_signers = Vec::with_capacity(attestation.approvals.len());
        for approval in &attestation.approvals {
            if policy
                .authorized_signers
                .binary_search(&approval.public_key)
                .is_err()
            {
                return Err(KagemushaReleaseErrorV1::UnknownSigner);
            }
            approval
                .signature
                .verify(&approval.public_key, &payload)
                .map_err(|_| KagemushaReleaseErrorV1::InvalidSignature)?;
            approved_signers.push(approval.public_key.clone());
        }
        let collected = u16::try_from(approved_signers.len())
            .map_err(|_| KagemushaReleaseErrorV1::InvalidAttestation)?;
        if collected < policy.threshold {
            return Err(KagemushaReleaseErrorV1::InsufficientThreshold {
                collected,
                required: policy.threshold,
            });
        }
        let attestation_digest = digest_encoded(RELEASE_ATTESTATION_DIGEST_DOMAIN, attestation)?;
        Ok(KagemushaAuthenticatedReleaseV1 {
            manifest: self.clone(),
            manifest_digest: expected_subject.manifest_digest,
            receipt_digest: expected_subject.validation_receipt_digest,
            authority_policy_digest: expected_subject.authority_policy_digest,
            attestation_digest,
            approved_signers,
        })
    }
}

impl KagemushaAuthenticatedReleaseV1 {
    /// Return the authenticated release identifier.
    #[must_use]
    pub fn release_id(&self) -> [u8; 32] {
        self.manifest.release_id
    }

    /// Return the exact sorted enabled hardware-profile set.
    #[must_use]
    pub fn enabled_profiles(&self) -> &[KagemushaEnabledProfileV1] {
        &self.manifest.enabled_profiles
    }

    /// Resolve one enabled profile by its digest-derived hardware profile ID.
    #[must_use]
    pub fn enabled_profile(
        &self,
        hardware_profile_id: [u8; 32],
    ) -> Option<&KagemushaEnabledProfileV1> {
        self.manifest
            .enabled_profiles
            .binary_search_by_key(&hardware_profile_id, |profile| profile.hardware_profile_id)
            .ok()
            .map(|index| &self.manifest.enabled_profiles[index])
    }

    /// Return the authenticated complete verifier-set digest.
    #[must_use]
    pub fn vk_set_digest(&self) -> [u8; 32] {
        self.manifest.enabled_profiles[0].vk_digest
    }

    /// Return the deterministic authenticated enabled-profile policy digest.
    #[must_use]
    pub fn hardware_policy_digest(&self) -> [u8; 32] {
        self.manifest.hardware_policy_digest
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

    /// Return the exact authenticated helper-protocol set in canonical order.
    #[must_use]
    pub fn helper_protocols(&self) -> &[KagemushaHelperProtocolV1] {
        &self.manifest.helper_protocols
    }

    /// Resolve the authenticated compiled protocol identities for one helper circuit.
    #[must_use]
    pub fn helper_protocol(
        &self,
        helper: KagemushaQualifiedHelperCircuitV1,
    ) -> Option<&KagemushaHelperProtocolV1> {
        self.manifest
            .helper_protocols
            .binary_search_by_key(&helper, |protocol| protocol.helper)
            .ok()
            .map(|index| &self.manifest.helper_protocols[index])
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
    pub fn artifact(&self, role: KagemushaArtifactRoleV1) -> KagemushaArtifactBindingV1 {
        let index = KagemushaArtifactRoleV1::ALL
            .iter()
            .position(|candidate| *candidate == role)
            .expect("every role belongs to the authenticated inventory");
        self.manifest.artifacts[index]
    }
}

#[cfg(test)]
#[path = "kagemusha_release_v1_tests.rs"]
mod tests;
