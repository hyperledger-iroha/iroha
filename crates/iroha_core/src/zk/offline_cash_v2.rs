//! Non-authorizing Offline Cash k=17 profile scaffold.
//!
//! This module records the reviewed V2 hard-cut ledger without changing the
//! Offline Cash V1 wire, release, protocol, artifact, or verifier contracts.
//! It deliberately has no artifact loader, terminal dispatch, verifier trait
//! implementation, protocol digest, or success path. In particular, the Eq
//! and Ep P-256 signature entries below are role metadata only. They cannot
//! authenticate a key or authorize proof acceptance.
//!
//! The private GuardBundle and STATE child modules freeze one acyclic source
//! contract: a
//! parity-typed 576-byte public value is an aggregate predecessor lineage, not
//! the current proof's terminal accumulator. It also freezes the exact
//! 336-word/48-cell GuardBundle ABI, 237-word/34-cell STATE ABI, ordered helper
//! and STATE folds, and fail-closed terminal order. A private native relation
//! kernel verifies only the exact six-input BGH19 Eq-then-Ep fold and retains
//! the complete provenance-bound candidate in a move-only seal. It does not
//! implement recursive circuits, ordinary child-proof verification, terminal
//! accumulator decisions, artifacts, persistence, or a production backend.
//!
//! Session framing, final-STATE pair qualification, V2 wire/release types,
//! compact SHA-256, DER/KeyMint/root closure, recursion bootstrap,
//! `GuardBundle`, authenticated artifacts/RSS, device qualification, and the
//! P-256 V3 interval/slope bounds remain open. The production backend therefore
//! remains `VerificationUnavailable`.

use core::fmt;

#[path = "offline_cash_v2/attestation_registration.rs"]
mod attestation_registration;
#[cfg(test)]
#[path = "offline_cash_v2/attestation_registration_tests.rs"]
mod attestation_registration_tests;
#[path = "offline_cash_v2/guard_bundle_provenance.rs"]
mod guard_bundle_provenance;
#[path = "offline_cash_v2/registered_platform_p256_circuit_source.rs"]
mod registered_platform_p256_circuit_source;
#[path = "offline_cash_v2/registered_platform_p256_statement.rs"]
mod registered_platform_p256_statement;
#[path = "offline_cash_v2_state_lineage.rs"]
mod state_lineage;
#[path = "offline_cash_v2/state_recursive_fold.rs"]
mod state_recursive_fold;
#[path = "offline_cash_v2/state_recursive_fold_native.rs"]
mod state_recursive_fold_native;
#[path = "offline_cash_v2/state_semantic_parent_provenance.rs"]
mod state_semantic_parent_provenance;
#[path = "offline_cash_v2/state_terminal_candidate.rs"]
mod state_terminal_candidate;

/// Halo2 domain exponent selected by the non-authorizing V2 profile.
pub(super) const OFFLINE_CASH_HALO2_K_V2: u32 = 17;
/// Exact raw transcript bytes for the reviewed P-256 signature shape at k=17.
pub(super) const OFFLINE_CASH_PARITY_RAW_PROOF_BYTES_V2: u32 = 3_232;
/// Exact augmented transcript bytes, including the folded-generator suffix.
pub(super) const OFFLINE_CASH_PARITY_AUGMENTED_PROOF_BYTES_V2: u32 = 3_264;
/// Absolute V2 child-proof cap, met exactly by the reviewed P-256 child.
pub(super) const OFFLINE_CASH_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2: u32 = 3_264;
/// Unresolved qualification target for the two final STATE proofs.
///
/// This value is telemetry until the final STATE pair-target decision is
/// governed. It is deliberately distinct from the 3,264-byte child-proof cap
/// and does not qualify the P-256 children below.
pub(super) const OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_QUALIFICATION_TARGET_BYTES_V2: u32 = 6_272;
/// Absolute byte limit reserved for the two final STATE proofs.
pub(super) const OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_ABSOLUTE_MAX_BYTES_V2: u32 = 6_528;
/// Exact bytes in the two reviewed augmented P-256 transcripts.
pub(super) const OFFLINE_CASH_P256_PAIRED_AUGMENTED_PROOF_BYTES_V2: u32 =
    2 * OFFLINE_CASH_PARITY_AUGMENTED_PROOF_BYTES_V2;
/// Arithmetic difference between the P-256 pair and unresolved final-STATE target.
///
/// The P-256 proofs are children, not evidence about final STATE proof size.
pub(super) const OFFLINE_CASH_P256_PAIRED_FINAL_STATE_TARGET_MISS_BYTES_V2: u32 =
    OFFLINE_CASH_P256_PAIRED_AUGMENTED_PROOF_BYTES_V2
        - OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_QUALIFICATION_TARGET_BYTES_V2;
/// A P-256 child pair does not establish final-STATE pair qualification.
pub(super) const OFFLINE_CASH_P256_PAIR_ESTABLISHES_FINAL_STATE_QUALIFICATION_V2: bool = false;
/// Exact aggregate parent-lineage accumulator bytes for one k=17 Pasta parity.
pub(super) const OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2: u32 = 576;
/// Exact bytes occupied by the Eq and Ep aggregate parent lineages together.
pub(super) const OFFLINE_CASH_PAIRED_PARENT_LINEAGE_BYTES_V2: u32 =
    2 * OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2;

/// Exact field-neutral V2 STATE public ABI words.
///
/// The final 144 words encode one aggregate predecessor lineage. The current
/// proof accumulator is expressly absent from these words.
pub(super) const OFFLINE_CASH_STATE_ABI_WORDS_V2: u32 = 237;
/// Canonical words packed into one Pasta public-instance cell.
pub(super) const OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V2: u32 = 7;
/// Exact public-instance cells for the 237-word V2 STATE ABI.
pub(super) const OFFLINE_CASH_STATE_INSTANCE_CELLS_V2: u32 = 34;
/// Mandatory zero words in the final STATE instance cell.
pub(super) const OFFLINE_CASH_STATE_FINAL_CELL_ZERO_PADDING_WORDS_V2: u32 = 1;

/// Unchanged receiver-request ceiling assumed by the hard-cut session ledger.
pub(super) const OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V2: u32 = 768;
/// Fixed payment bytes excluding the paired STATE proofs and parent lineages.
pub(super) const OFFLINE_CASH_PAYMENT_FIXED_ENVELOPE_BYTES_V2: u32 = 448;
/// Exact sender-payment ceiling after the proof and parent-lineage k=17 deltas.
pub(super) const OFFLINE_CASH_PAYMENT_MAX_BYTES_V2: u32 = 8_128;
/// Unchanged acknowledgement ceiling assumed by the hard-cut session ledger.
pub(super) const OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V2: u32 = 256;
/// Exact sum of the three independently bounded raw message components.
pub(super) const OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2: u32 = 9_152;
/// Exact three-frame text size at the three component maxima.
pub(super) const OFFLINE_CASH_EXACT_COMPONENT_THREE_FRAME_TEXT_SESSION_BYTES_V2: u32 = 12_219;
/// Unresolved independent aggregate raw-session policy ceiling.
///
/// This is not an active wire maximum: the bounded components currently sum to
/// 9,152 bytes, and the framing/profile decision remains open.
pub(super) const OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2: u32 = 9_403;
/// Safe text ceiling for any three-frame partition of the unresolved raw ceiling.
pub(super) const OFFLINE_CASH_UNRESOLVED_AGGREGATE_THREE_FRAME_TEXT_CEILING_BYTES_V2: u32 = 12_554;

/// Exact serialized transparent ParamsIPA bytes for either k=17 parity.
pub(super) const OFFLINE_CASH_PARAMS_BYTES_V2: u64 = 8_388_676;
/// Exact reviewed processed verifying-key bytes for one P-256 role/parity.
pub(super) const OFFLINE_CASH_P256_PROCESSED_VERIFYING_KEY_BYTES_V2: u64 = 394;
/// Exact reviewed processed proving-key bytes for one P-256 role/parity.
pub(super) const OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2: u64 = 113_246_726;
/// Named 32-MiB live-workspace component in the straightforward residence model.
pub(super) const OFFLINE_CASH_STRAIGHTFORWARD_RESIDENCE_WORKSPACE_BYTES_V2: u64 = 32 * 1024 * 1024;
/// Straightforward non-streaming implementation's computed residence lower bound.
///
/// This is an arithmetic lower bound, not a measured peak-RSS qualification.
pub(super) const OFFLINE_CASH_MINIMUM_STRAIGHTFORWARD_RESIDENCE_BYTES_V2: u64 = 155_189_834;
/// Qualification ceiling for measured whole-process proving and verification RSS.
pub(super) const OFFLINE_CASH_PROCESS_RSS_QUALIFICATION_BYTES_V2: u64 = 268_435_456;
/// Absolute complete preinstalled V2 artifact-package ceiling.
///
/// Only the P-256 subset is currently enumerated; this does not assert that a
/// complete V2 artifact package fits the ceiling.
pub(super) const OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V2: u64 = 536_870_912;
/// Exact bytes in the currently enumerated Eq/Ep P-256 artifact subset.
pub(super) const OFFLINE_CASH_P256_ARTIFACT_SUBSET_BYTES_V2: u64 = 2
    * (OFFLINE_CASH_PARAMS_BYTES_V2
        + OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2
        + OFFLINE_CASH_P256_PROCESSED_VERIFYING_KEY_BYTES_V2);
/// Existing Core archive ceiling, which the reviewed P-256 proving key exceeds.
pub(super) const OFFLINE_CASH_EXISTING_PROVING_KEY_ARCHIVE_MAX_BYTES_V2: u64 =
    super::HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES as u64;

/// The current k=17 helper scaffold transcript; it exceeds the parity cap.
pub(super) const OFFLINE_CASH_HELPER_SCAFFOLD_AUGMENTED_PROOF_BYTES_V2: u32 = 4_736;

const SCALAR_BYTES: u32 = 32;
const FOLDED_GENERATOR_BYTES: u32 = 32;
const STATE_NON_LINEAGE_WORDS: u32 = 93;
const TEXT_PREFIX_BYTES: u32 = 5;

const fn unpadded_base64url_len(raw_bytes: u32) -> u32 {
    raw_bytes / 3 * 4
        + match raw_bytes % 3 {
            0 => 0,
            1 => 2,
            _ => 3,
        }
}

/// Maximum sum of three unpadded-base64url payload lengths with a fixed raw total.
///
/// For totals of at least three bytes, distributing residue bytes across three
/// frames adds two encoded bytes for residue zero and one otherwise compared
/// with encoding the same total as one frame.
const fn maximum_three_frame_unpadded_base64url_len(raw_bytes: u32) -> u32 {
    assert!(raw_bytes >= 3);
    unpadded_base64url_len(raw_bytes)
        + match raw_bytes % 3 {
            0 => 2,
            _ => 1,
        }
}

const _: () = assert!(
    OFFLINE_CASH_PARITY_AUGMENTED_PROOF_BYTES_V2
        == OFFLINE_CASH_PARITY_RAW_PROOF_BYTES_V2 + FOLDED_GENERATOR_BYTES
);
const _: () = assert!(
    OFFLINE_CASH_PARITY_AUGMENTED_PROOF_BYTES_V2 == OFFLINE_CASH_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2
);
const _: () = assert!(
    OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_ABSOLUTE_MAX_BYTES_V2
        == OFFLINE_CASH_P256_PAIRED_AUGMENTED_PROOF_BYTES_V2
);
const _: () = assert!(
    OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_QUALIFICATION_TARGET_BYTES_V2
        <= OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_ABSOLUTE_MAX_BYTES_V2
);
const _: () = assert!(OFFLINE_CASH_P256_PAIRED_FINAL_STATE_TARGET_MISS_BYTES_V2 == 256);
const _: () = assert!(!OFFLINE_CASH_P256_PAIR_ESTABLISHES_FINAL_STATE_QUALIFICATION_V2);
const _: () = assert!(
    OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2
        == OFFLINE_CASH_HALO2_K_V2 * SCALAR_BYTES + FOLDED_GENERATOR_BYTES
);
const _: () = assert!(OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2 % 4 == 0);
const _: () = assert!(
    OFFLINE_CASH_PAIRED_PARENT_LINEAGE_BYTES_V2
        == 2 * OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2
);
const _: () = assert!(
    OFFLINE_CASH_STATE_ABI_WORDS_V2
        == STATE_NON_LINEAGE_WORDS + OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2 / 4
);
const _: () = assert!(
    OFFLINE_CASH_STATE_INSTANCE_CELLS_V2
        == OFFLINE_CASH_STATE_ABI_WORDS_V2.div_ceil(OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V2)
);
const _: () = assert!(
    OFFLINE_CASH_STATE_FINAL_CELL_ZERO_PADDING_WORDS_V2
        == OFFLINE_CASH_STATE_INSTANCE_CELLS_V2 * OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V2
            - OFFLINE_CASH_STATE_ABI_WORDS_V2
);
const _: () = assert!(
    OFFLINE_CASH_PAYMENT_MAX_BYTES_V2
        == OFFLINE_CASH_PAYMENT_FIXED_ENVELOPE_BYTES_V2
            + OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_ABSOLUTE_MAX_BYTES_V2
            + OFFLINE_CASH_PAIRED_PARENT_LINEAGE_BYTES_V2
);
const _: () = assert!(
    OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2
        == OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V2
            + OFFLINE_CASH_PAYMENT_MAX_BYTES_V2
            + OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V2
);
const _: () = assert!(
    OFFLINE_CASH_EXACT_COMPONENT_THREE_FRAME_TEXT_SESSION_BYTES_V2
        == 3 * TEXT_PREFIX_BYTES
            + unpadded_base64url_len(OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V2)
            + unpadded_base64url_len(OFFLINE_CASH_PAYMENT_MAX_BYTES_V2)
            + unpadded_base64url_len(OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V2)
);
const _: () = assert!(
    OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2
        < OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2
);
const _: () = assert!(
    OFFLINE_CASH_UNRESOLVED_AGGREGATE_THREE_FRAME_TEXT_CEILING_BYTES_V2
        == 3 * TEXT_PREFIX_BYTES
            + maximum_three_frame_unpadded_base64url_len(
                OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2,
            )
);
const _: () = assert!(
    OFFLINE_CASH_MINIMUM_STRAIGHTFORWARD_RESIDENCE_BYTES_V2
        == OFFLINE_CASH_PARAMS_BYTES_V2
            + OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2
            + OFFLINE_CASH_STRAIGHTFORWARD_RESIDENCE_WORKSPACE_BYTES_V2
);
// A computed lower bound below the ceiling is necessary but is not measured RSS evidence.
const _: () = assert!(
    OFFLINE_CASH_MINIMUM_STRAIGHTFORWARD_RESIDENCE_BYTES_V2
        <= OFFLINE_CASH_PROCESS_RSS_QUALIFICATION_BYTES_V2
);
const _: () =
    assert!(OFFLINE_CASH_P256_ARTIFACT_SUBSET_BYTES_V2 <= OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V2);
const _: () = assert!(
    OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2
        > OFFLINE_CASH_EXISTING_PROVING_KEY_ARCHIVE_MAX_BYTES_V2
);
const _: () = assert!(
    OFFLINE_CASH_HELPER_SCAFFOLD_AUGMENTED_PROOF_BYTES_V2
        > OFFLINE_CASH_PARITY_AUGMENTED_PROOF_BYTES_V2
);

/// Pasta parity attached to non-authorizing P-256 signature metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum OfflineCashHalo2ParityV2 {
    /// Eq/Fp proof metadata.
    Eq = 1,
    /// Ep/Fq proof metadata.
    Ep = 2,
}

/// Finite k=17 circuit-role namespace recorded by the private V2 source contract.
///
/// These tags are not a compiled protocol inventory and cannot identify an
/// artifact. Every corresponding compiler, artifact, backend, and release gate
/// remains closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum OfflineCashHalo2CircuitRoleV2 {
    /// Recursive balance-state relation.
    State = 1,
    /// Exact-next guard-use relation.
    GuardUse = 2,
    /// Platform binding relation.
    PlatformBind = 3,
    /// Optional Android hardware-key certificate relation.
    AndroidKeyCert = 4,
    /// Ordered recursive helper aggregation.
    GuardBundle = 5,
    /// P-256 signature verification child; metadata only.
    P256Signature = 6,
}

impl OfflineCashHalo2CircuitRoleV2 {
    /// Exact canonical V2 source-role order.
    pub(super) const ALL: [Self; 6] = [
        Self::State,
        Self::GuardUse,
        Self::PlatformBind,
        Self::AndroidKeyCert,
        Self::GuardBundle,
        Self::P256Signature,
    ];
}

/// Private artifact labels for the two P-256 parity records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum OfflineCashP256ArtifactRoleV2 {
    /// Eq P-256 signature proving key.
    P256SignaturePkEq = 1,
    /// Eq P-256 signature verifying key.
    P256SignatureVkEq = 2,
    /// Ep P-256 signature proving key.
    P256SignaturePkEp = 3,
    /// Ep P-256 signature verifying key.
    P256SignatureVkEp = 4,
}

/// Role-specific P-256 shape and artifact-size metadata.
///
/// Digest fields are intentionally absent. No value of this type can name or
/// authenticate a real artifact, and `activation_eligible` is pinned false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OfflineCashP256SignatureMetadataV2 {
    /// Selected Pasta parity.
    pub(super) parity: OfflineCashHalo2ParityV2,
    /// Metadata-only circuit role.
    pub(super) circuit_role: OfflineCashHalo2CircuitRoleV2,
    /// Parity-specific proving-key label.
    pub(super) proving_key_role: OfflineCashP256ArtifactRoleV2,
    /// Parity-specific verifying-key label.
    pub(super) verifying_key_role: OfflineCashP256ArtifactRoleV2,
    /// Exact Halo2 domain exponent.
    pub(super) k: u32,
    /// Exact raw transcript bytes.
    pub(super) raw_proof_bytes: u32,
    /// Exact augmented transcript bytes.
    pub(super) augmented_proof_bytes: u32,
    /// Exact processed proving-key bytes from the audit.
    pub(super) processed_proving_key_bytes: u64,
    /// Exact processed verifying-key bytes from the audit.
    pub(super) processed_verifying_key_bytes: u64,
    /// Always false until every closure and production-evidence gate passes.
    pub(super) activation_eligible: bool,
}

/// Exact Eq/Ep P-256 records. These records are not an artifact inventory.
pub(super) const OFFLINE_CASH_P256_SIGNATURE_METADATA_V2: [OfflineCashP256SignatureMetadataV2; 2] = [
    OfflineCashP256SignatureMetadataV2 {
        parity: OfflineCashHalo2ParityV2::Eq,
        circuit_role: OfflineCashHalo2CircuitRoleV2::P256Signature,
        proving_key_role: OfflineCashP256ArtifactRoleV2::P256SignaturePkEq,
        verifying_key_role: OfflineCashP256ArtifactRoleV2::P256SignatureVkEq,
        k: OFFLINE_CASH_HALO2_K_V2,
        raw_proof_bytes: OFFLINE_CASH_PARITY_RAW_PROOF_BYTES_V2,
        augmented_proof_bytes: OFFLINE_CASH_PARITY_AUGMENTED_PROOF_BYTES_V2,
        processed_proving_key_bytes: OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2,
        processed_verifying_key_bytes: OFFLINE_CASH_P256_PROCESSED_VERIFYING_KEY_BYTES_V2,
        activation_eligible: false,
    },
    OfflineCashP256SignatureMetadataV2 {
        parity: OfflineCashHalo2ParityV2::Ep,
        circuit_role: OfflineCashHalo2CircuitRoleV2::P256Signature,
        proving_key_role: OfflineCashP256ArtifactRoleV2::P256SignaturePkEp,
        verifying_key_role: OfflineCashP256ArtifactRoleV2::P256SignatureVkEp,
        k: OFFLINE_CASH_HALO2_K_V2,
        raw_proof_bytes: OFFLINE_CASH_PARITY_RAW_PROOF_BYTES_V2,
        augmented_proof_bytes: OFFLINE_CASH_PARITY_AUGMENTED_PROOF_BYTES_V2,
        processed_proving_key_bytes: OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2,
        processed_verifying_key_bytes: OFFLINE_CASH_P256_PROCESSED_VERIFYING_KEY_BYTES_V2,
        activation_eligible: false,
    },
];

/// P-256 V3 interval-bound qualification is not available.
pub(super) const OFFLINE_CASH_P256_V3_INTERVAL_EVIDENCE_AVAILABLE_V2: bool = false;
/// P-256 V3 slope-bound qualification is not available.
pub(super) const OFFLINE_CASH_P256_V3_SLOPE_EVIDENCE_AVAILABLE_V2: bool = false;
/// The three-frame session framing and independent aggregate policy are not frozen.
pub(super) const OFFLINE_CASH_SESSION_FRAMING_PROFILE_FROZEN_V2: bool = false;
/// The final-STATE pair qualification target has not been governed.
pub(super) const OFFLINE_CASH_FINAL_STATE_PAIR_TARGET_DECISION_AVAILABLE_V2: bool = false;
/// The frozen parent-lineage dependency order is acyclic by construction.
pub(super) const OFFLINE_CASH_STATE_PARENT_LINEAGE_CONTRACT_IS_ACYCLIC_V2: bool = true;
/// No production direct-instance STATE verifier is available.
pub(super) const OFFLINE_CASH_STATE_DIRECT_INSTANCE_VERIFIER_AVAILABLE_V2: bool = false;
/// No production recursive parent-lineage fold is available.
pub(super) const OFFLINE_CASH_STATE_RECURSIVE_FOLD_AVAILABLE_V2: bool = false;
/// No production STATE terminal can issue a verified receipt.
pub(super) const OFFLINE_CASH_STATE_TERMINAL_RECEIPT_AVAILABLE_V2: bool = false;
/// No distinct V2 wire and authenticated-release types are available.
pub(super) const OFFLINE_CASH_V2_WIRE_RELEASE_TYPES_AVAILABLE_V2: bool = false;
/// Compact SHA-256 qualification is not available.
pub(super) const OFFLINE_CASH_COMPACT_SHA_EVIDENCE_AVAILABLE_V2: bool = false;
/// Android DER, KeyMint, and governed-root proof closure is not available.
pub(super) const OFFLINE_CASH_DER_KEYMINT_GOVERNED_ROOT_CLOSURE_AVAILABLE_V2: bool = false;
/// Recursion bootstrap and compiled-protocol identity closure is not available.
pub(super) const OFFLINE_CASH_RECURSION_BOOTSTRAP_PROTOCOL_IDENTITY_AVAILABLE_V2: bool = false;
/// Recursive GuardBundle qualification is not available.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_EVIDENCE_AVAILABLE_V2: bool = false;
/// Complete STATE qualification is not available.
pub(super) const OFFLINE_CASH_STATE_EVIDENCE_AVAILABLE_V2: bool = false;
/// A complete V2 artifact-role inventory is not available.
pub(super) const OFFLINE_CASH_COMPLETE_ARTIFACT_INVENTORY_AVAILABLE_V2: bool = false;
/// Complete artifact-set size evidence against the package cap is not available.
pub(super) const OFFLINE_CASH_COMPLETE_ARTIFACT_SET_SIZE_EVIDENCE_AVAILABLE_V2: bool = false;
/// Authenticated production artifact evidence is not available.
pub(super) const OFFLINE_CASH_ARTIFACT_EVIDENCE_AVAILABLE_V2: bool = false;
/// Measured whole-process RSS evidence is not available.
pub(super) const OFFLINE_CASH_MEASURED_PROCESS_RSS_EVIDENCE_AVAILABLE_V2: bool = false;
/// Representative production-device evidence is not available.
pub(super) const OFFLINE_CASH_DEVICE_EVIDENCE_AVAILABLE_V2: bool = false;
/// No V2 proof-verification backend is available.
pub(super) const OFFLINE_CASH_VERIFICATION_BACKEND_AVAILABLE_V2: bool = false;

/// Exact reason the V2 profile cannot pass activation preflight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashActivationPreflightErrorV2 {
    /// The P-256 V3 interval proof has no qualifying bound evidence.
    P256V3IntervalEvidenceUnavailable,
    /// The P-256 V3 slope proof has no qualifying bound evidence.
    P256V3SlopeEvidenceUnavailable,
    /// The three-frame and independent aggregate session policies are unresolved.
    SessionFramingProfileUnresolved,
    /// The final-STATE qualification target has not been governed.
    FinalStatePairTargetDecisionUnavailable {
        /// Current unresolved final-STATE qualification target.
        qualification_target: u32,
        /// Absolute final-STATE proof-pair ceiling.
        absolute_maximum: u32,
    },
    /// No direct-instance STATE verifier implements the frozen acyclic ABI.
    StateDirectInstanceVerifierUnavailable,
    /// No recursive fold implements aggregate predecessor lineage production.
    StateRecursiveFoldUnavailable,
    /// No complete STATE terminal can cross the verified-receipt boundary.
    StateTerminalReceiptUnavailable,
    /// Distinct V2 wire and authenticated-release types are unavailable.
    V2WireReleaseTypesUnavailable,
    /// The current helper transcript exceeds the parity proof cap.
    HelperScaffoldProofSizeExceeded {
        /// Exact audited helper proof bytes.
        actual: u32,
        /// Exact V2 parity cap.
        maximum: u32,
    },
    /// Compact SHA-256 closure evidence is unavailable.
    CompactShaEvidenceUnavailable,
    /// Android DER, KeyMint, and governed-root closure is unavailable.
    DerKeyMintGovernedRootClosureUnavailable,
    /// Recursion bootstrap and compiled-protocol identity closure is unavailable.
    RecursionBootstrapProtocolIdentityUnavailable,
    /// Recursive GuardBundle closure evidence is unavailable.
    GuardBundleEvidenceUnavailable,
    /// Complete STATE closure evidence is unavailable.
    StateEvidenceUnavailable,
    /// The complete V2 artifact-role inventory is unavailable.
    CompleteArtifactInventoryUnavailable,
    /// Complete artifact-set size evidence is unavailable.
    CompleteArtifactSetSizeEvidenceUnavailable,
    /// Authenticated production artifact evidence is unavailable.
    ArtifactEvidenceUnavailable,
    /// The reviewed P-256 proving key exceeds Core's existing archive ceiling.
    ProvingKeyArchiveCapExceeded {
        /// Exact reviewed processed proving-key bytes.
        actual: u64,
        /// Existing maximum bytes accepted by Core's proving-key archive.
        maximum: u64,
    },
    /// Measured whole-process RSS evidence is unavailable.
    MeasuredProcessRssEvidenceUnavailable,
    /// Representative production-device evidence is unavailable.
    DeviceEvidenceUnavailable,
    /// Production proof verification is deliberately unavailable.
    VerificationUnavailable,
}

impl fmt::Display for OfflineCashActivationPreflightErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::P256V3IntervalEvidenceUnavailable => {
                formatter.write_str("offline-cash V2 P-256 V3 interval evidence is unavailable")
            }
            Self::P256V3SlopeEvidenceUnavailable => {
                formatter.write_str("offline-cash V2 P-256 V3 slope evidence is unavailable")
            }
            Self::SessionFramingProfileUnresolved => formatter.write_str(
                "offline-cash V2 three-frame and aggregate session policy is unresolved",
            ),
            Self::FinalStatePairTargetDecisionUnavailable {
                qualification_target,
                absolute_maximum,
            } => write!(
                formatter,
                "offline-cash V2 final-STATE pair target {qualification_target} is unresolved under absolute maximum {absolute_maximum}"
            ),
            Self::StateDirectInstanceVerifierUnavailable => formatter.write_str(
                "offline-cash V2 direct-instance STATE verifier is unavailable",
            ),
            Self::StateRecursiveFoldUnavailable => formatter.write_str(
                "offline-cash V2 recursive parent-lineage fold is unavailable",
            ),
            Self::StateTerminalReceiptUnavailable => formatter.write_str(
                "offline-cash V2 verified STATE receipt boundary is unavailable",
            ),
            Self::V2WireReleaseTypesUnavailable => formatter
                .write_str("offline-cash V2 wire and authenticated-release types are unavailable"),
            Self::HelperScaffoldProofSizeExceeded { actual, maximum } => write!(
                formatter,
                "offline-cash V2 helper scaffold proof size {actual} exceeds the {maximum}-byte parity cap"
            ),
            Self::CompactShaEvidenceUnavailable => {
                formatter.write_str("offline-cash V2 compact SHA-256 evidence is unavailable")
            }
            Self::DerKeyMintGovernedRootClosureUnavailable => formatter.write_str(
                "offline-cash V2 Android DER, KeyMint, and governed-root closure is unavailable",
            ),
            Self::RecursionBootstrapProtocolIdentityUnavailable => formatter.write_str(
                "offline-cash V2 recursion bootstrap and compiled-protocol identity are unavailable",
            ),
            Self::GuardBundleEvidenceUnavailable => {
                formatter.write_str("offline-cash V2 GuardBundle evidence is unavailable")
            }
            Self::StateEvidenceUnavailable => {
                formatter.write_str("offline-cash V2 STATE evidence is unavailable")
            }
            Self::CompleteArtifactInventoryUnavailable => {
                formatter.write_str("offline-cash V2 complete artifact inventory is unavailable")
            }
            Self::CompleteArtifactSetSizeEvidenceUnavailable => formatter.write_str(
                "offline-cash V2 complete artifact-set size evidence is unavailable",
            ),
            Self::ArtifactEvidenceUnavailable => {
                formatter.write_str("offline-cash V2 authenticated artifact evidence is unavailable")
            }
            Self::ProvingKeyArchiveCapExceeded { actual, maximum } => write!(
                formatter,
                "offline-cash V2 P-256 proving key size {actual} exceeds Core's existing {maximum}-byte archive cap"
            ),
            Self::MeasuredProcessRssEvidenceUnavailable => {
                formatter.write_str("offline-cash V2 measured process-RSS evidence is unavailable")
            }
            Self::DeviceEvidenceUnavailable => {
                formatter.write_str("offline-cash V2 device evidence is unavailable")
            }
            Self::VerificationUnavailable => {
                formatter.write_str("offline-cash V2 verification is unavailable")
            }
        }
    }
}

impl std::error::Error for OfflineCashActivationPreflightErrorV2 {}

/// Complete, ordered blocker inventory for the non-authorizing scaffold.
pub(super) const OFFLINE_CASH_ACTIVATION_BLOCKERS_V2: [OfflineCashActivationPreflightErrorV2; 21] = [
    OfflineCashActivationPreflightErrorV2::P256V3IntervalEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::P256V3SlopeEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::SessionFramingProfileUnresolved,
    OfflineCashActivationPreflightErrorV2::FinalStatePairTargetDecisionUnavailable {
        qualification_target: OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_QUALIFICATION_TARGET_BYTES_V2,
        absolute_maximum: OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_ABSOLUTE_MAX_BYTES_V2,
    },
    OfflineCashActivationPreflightErrorV2::StateDirectInstanceVerifierUnavailable,
    OfflineCashActivationPreflightErrorV2::StateRecursiveFoldUnavailable,
    OfflineCashActivationPreflightErrorV2::StateTerminalReceiptUnavailable,
    OfflineCashActivationPreflightErrorV2::V2WireReleaseTypesUnavailable,
    OfflineCashActivationPreflightErrorV2::HelperScaffoldProofSizeExceeded {
        actual: OFFLINE_CASH_HELPER_SCAFFOLD_AUGMENTED_PROOF_BYTES_V2,
        maximum: OFFLINE_CASH_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2,
    },
    OfflineCashActivationPreflightErrorV2::CompactShaEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::DerKeyMintGovernedRootClosureUnavailable,
    OfflineCashActivationPreflightErrorV2::RecursionBootstrapProtocolIdentityUnavailable,
    OfflineCashActivationPreflightErrorV2::GuardBundleEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::StateEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::CompleteArtifactInventoryUnavailable,
    OfflineCashActivationPreflightErrorV2::ProvingKeyArchiveCapExceeded {
        actual: OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2,
        maximum: OFFLINE_CASH_EXISTING_PROVING_KEY_ARCHIVE_MAX_BYTES_V2,
    },
    OfflineCashActivationPreflightErrorV2::CompleteArtifactSetSizeEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::ArtifactEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::MeasuredProcessRssEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::DeviceEvidenceUnavailable,
    OfflineCashActivationPreflightErrorV2::VerificationUnavailable,
];

const _: () = assert!(!OFFLINE_CASH_P256_V3_INTERVAL_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_P256_V3_SLOPE_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_SESSION_FRAMING_PROFILE_FROZEN_V2);
const _: () = assert!(!OFFLINE_CASH_FINAL_STATE_PAIR_TARGET_DECISION_AVAILABLE_V2);
const _: () = assert!(OFFLINE_CASH_STATE_PARENT_LINEAGE_CONTRACT_IS_ACYCLIC_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_DIRECT_INSTANCE_VERIFIER_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_RECURSIVE_FOLD_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_TERMINAL_RECEIPT_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_V2_WIRE_RELEASE_TYPES_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_COMPACT_SHA_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_DER_KEYMINT_GOVERNED_ROOT_CLOSURE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_RECURSION_BOOTSTRAP_PROTOCOL_IDENTITY_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_COMPLETE_ARTIFACT_INVENTORY_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_COMPLETE_ARTIFACT_SET_SIZE_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_ARTIFACT_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_MEASURED_PROCESS_RSS_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_DEVICE_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_VERIFICATION_BACKEND_AVAILABLE_V2);

/// Fail-closed activation preflight for the metadata-only V2 profile.
///
/// There is intentionally no success path. Returning the first entry from the
/// complete blocker inventory prevents this scaffold from being mistaken for
/// proof authority while preserving every reviewed failure for diagnostics.
pub(super) const fn preflight_offline_cash_activation_v2()
-> Result<(), OfflineCashActivationPreflightErrorV2> {
    Err(OFFLINE_CASH_ACTIVATION_BLOCKERS_V2[0])
}

#[cfg(test)]
#[path = "offline_cash_v2_tests.rs"]
mod tests;
