//! Native field and curve adapters for the pinned Microsoft Vega profile.
//!
//! Vega's canonical engine uses the T256 group. Its scalar field is exactly
//! the P-256 base field, which lets the mDL circuit ingest issuer-key
//! coordinates without non-native reduction. This module keeps that identity
//! explicit and exposes only canonical, non-reducing encodings to callers.
//!
//! The protocol source is Microsoft `vega-prover` commit
//! `c0ee259053cd12eaf43ed71b5cde375452b3ee4d`, licensed under MIT.

use core::{
    fmt,
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};

use halo2curves::{
    ff::{Field, FromUniformBytes, PrimeField},
    t256::Fq,
};
use thiserror::Error;

#[path = "vega/algebra.rs"]
mod algebra;
#[path = "vega/bulletproof_t256.rs"]
mod bulletproof_t256;
#[path = "vega/canonical_mc.rs"]
mod canonical_mc;
#[path = "vega/circuit.rs"]
mod circuit;
#[path = "vega/commitment.rs"]
mod commitment;
#[path = "vega/curve.rs"]
mod curve;
#[path = "vega/date.rs"]
mod date;
#[path = "vega/engine.rs"]
mod engine;
#[path = "vega/figure9.rs"]
mod figure9;
#[path = "vega/figure9_layout.rs"]
mod figure9_layout;
#[path = "vega/hyrax.rs"]
mod hyrax;
#[path = "vega/masked_relaxed.rs"]
mod masked_relaxed;
#[path = "vega/nifs.rs"]
mod nifs;
#[path = "vega/p256.rs"]
mod p256;
#[path = "vega/r1cs.rs"]
mod r1cs;
#[path = "vega/sha256.rs"]
mod sha256;
#[path = "vega/spartan.rs"]
mod spartan;
#[path = "vega/sponge.rs"]
mod sponge;
#[path = "vega/sumcheck.rs"]
mod sumcheck;
#[path = "vega/transcript.rs"]
mod transcript;
#[path = "vega/wire.rs"]
mod wire;
#[path = "vega/zk_ams.rs"]
mod zk_ams;

pub(super) use curve::{
    VEGA_T256_BASE_MODULUS_BE_V1, VegaCurveError, VegaT256PointV1, derive_t256_generators_v1,
};
pub use engine::{
    MAX_VEGA_PROVER_RELEASE_MEMORY_CEILING_BYTES_V1, MAX_VEGA_PROVER_WORKERS_V1,
    VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1, VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
    VEGA_MDL_ACTION_INDEX_V1, VEGA_MDL_CANONICAL_RELATION_DIGEST_V1,
    VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, VEGA_MDL_COMPILED_PROFILE_DIGEST_V1,
    VEGA_MDL_COMPILED_PROFILE_MANIFEST_V1, VEGA_MDL_MC_STEP_COUNT_V1,
    VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1, VegaMdlProofContextV1, VegaMdlProofDimensionsV1,
    VegaMdlProofErrorV1, VegaMdlProverConfigV1, VegaRandomSourceErrorV1, VegaRandomSourceV1,
    prove_vega_mdl_figure9_v1, vega_mdl_canonical_relation_digest_v1,
    vega_mdl_compiled_profile_digest_v1, vega_mdl_proof_dimensions_v1, vega_mdl_verifier_digest_v1,
    verify_vega_mdl_figure9_v1,
};
pub use figure9::{
    VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaMdlFigure9ErrorV1, VegaMdlFigure9WitnessV1,
    validate_vega_mdl_figure9_encoding_v1, validate_vega_mdl_figure9_relation_v1,
};
pub use masked_relaxed::{MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1};
pub(super) use transcript::{VegaTranscriptError, VegaTranscriptV1};
pub(super) use wire::{VegaPointWireV1, VegaScalarWireV1};
pub use zk_ams::{
    MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1, ZK_AMS_ACTION_INDEX_V1,
    ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1, ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1,
    ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1, ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1,
    ZK_AMS_MKHE_MAX_PROOF_BYTES_V1, ZK_AMS_PHASE3_MAX_TERMINAL_PROOF_BYTES_V1,
    ZK_AMS_PHASE23_FRESHNESS_CERTIFIES_HIDDEN_MASK_SHARES_V1,
    ZK_AMS_PHASE23_FRESHNESS_COMMIT_WIRE_BYTES_V1, ZK_AMS_PHASE23_FRESHNESS_RECEIPT_WIRE_BYTES_V1,
    ZK_AMS_PHASE23_FRESHNESS_REVEAL_WIRE_BYTES_V1, ZK_AMS_PHASE23_MAX_CANONICAL_SPARSE_ENTRIES_V1,
    ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1, ZK_AMS_PHASE23_RELEASE_MAP_SET_KAT_DIGEST_V1,
    ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1,
    ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1, ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1,
    ZK_AMS_T256_GALOIS_KEY_COUNT_V1, ZK_AMS_T256_GALOIS_KEY_SCHEDULE_DIGEST_V1,
    ZK_AMS_T256_MAX_LOGICAL_VALUES_V1, ZK_AMS_T256_RELEASE_PACKED_INPUT_KAT_DIGEST_V1,
    ZK_AMS_T256_RELEASE_PACKED_OUTPUT_KAT_DIGEST_V1,
    ZK_AMS_T256_RELEASE_PACKING_NEGATIVE_CASE_COUNT_V1,
    ZK_AMS_T256_RELEASE_PACKING_NEGATIVE_KAT_DIGEST_V1,
    ZK_AMS_T256_RELEASE_ROTATION_CERTIFICATE_KAT_DIGEST_V1,
    ZK_AMS_T256_RELEASE_TRANSFORMED_RNS_KAT_DIGEST_V1, ZkAmsAdmissionPublicInputV1,
    ZkAmsAdmissionRelationDimensionsV1, ZkAmsAdmissionRelationErrorV1,
    ZkAmsAdmissionRelationWitnessV1, ZkAmsMaskedProverConfigV1, ZkAmsMkheAbortReasonV1,
    ZkAmsMkheActiveCollectivePublicKeyStatementV1, ZkAmsMkheActiveCollectivePublicKeyWitnessV1,
    ZkAmsMkheActiveContributionV1, ZkAmsMkheActivePartySecretV1,
    ZkAmsMkheActiveRkgLinearProofSecurityV1, ZkAmsMkheActiveRkgProofV1,
    ZkAmsMkheActiveRkgRoundOneStatementV1, ZkAmsMkheActiveRkgRoundOneWitnessV1,
    ZkAmsMkheActiveRkgRoundTwoStatementV1, ZkAmsMkheActiveRkgRoundTwoWitnessV1,
    ZkAmsMkheActiveRoundReceiptV1, ZkAmsMkheActiveRoundV1, ZkAmsMkheAuthenticatedCksContributionV1,
    ZkAmsMkheAuthenticatedDecryptionShareV1, ZkAmsMkheAuthenticationWireV1,
    ZkAmsMkheCksAbortReasonV1, ZkAmsMkheCksContributionWireV1, ZkAmsMkheCksProofV1,
    ZkAmsMkheCksResourceEvidenceV1, ZkAmsMkheCksSourceCiphertextV1, ZkAmsMkheCksStatementV1,
    ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheCollectiveCiphertextWireV1,
    ZkAmsMkheCollectiveCksDigitEvidenceV1, ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1, ZkAmsMkheCollectiveEvaluatedKeyManifestV1,
    ZkAmsMkheCollectiveEvaluatedKeyProviderV1, ZkAmsMkheCollectiveEvaluatedKeyPublicationFooterV1,
    ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1,
    ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1, ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ZkAmsMkheCollectiveEvidenceRecordFooterV1, ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    ZkAmsMkheCollectiveEvidenceRecordKindV1, ZkAmsMkheCollectiveEvidenceSetFooterV1,
    ZkAmsMkheCollectiveEvidenceSetHeaderV1, ZkAmsMkheCollectiveEvidenceSetKindV1,
    ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheCollectivePartyStateV1,
    ZkAmsMkheCollectivePublicKeyShareV1, ZkAmsMkheCollectivePublicKeyV1,
    ZkAmsMkheCollectiveSourceProofEvidenceV1, ZkAmsMkheCollectiveSourceStatementEvidenceV1,
    ZkAmsMkheDecryptedPlaintextV1, ZkAmsMkheDecryptionAbortReasonV1, ZkAmsMkheDecryptionProofV1,
    ZkAmsMkheDecryptionResourceEvidenceV1, ZkAmsMkheDecryptionSplitTransportV1,
    ZkAmsMkheDecryptionStatementV1, ZkAmsMkheDecryptionTransportComponentKindV1,
    ZkAmsMkheDecryptionTransportManifestV1, ZkAmsMkheDecryptionTransportPointerV1,
    ZkAmsMkheDirectAdmittedContributionSetV1, ZkAmsMkheDirectCeremonyContextV1,
    ZkAmsMkheDirectCeremonyRoundV1, ZkAmsMkheDirectCoordinatorV1,
    ZkAmsMkheDirectEvaluatedKeySetAdmissionV1, ZkAmsMkheDirectEvaluatedKeyTargetV1,
    ZkAmsMkheDirectNoiseCertificateV1, ZkAmsMkheDirectNoiseIntegrationCertificateV1,
    ZkAmsMkheDirectPolynomialRoleV1, ZkAmsMkheDirectPolynomialStreamReceiptV1,
    ZkAmsMkheDirectPolynomialStreamV1, ZkAmsMkheDirectProofAuditV1,
    ZkAmsMkheDirectResourceCertificateV1, ZkAmsMkheDirectVerifiedContributionProviderV1,
    ZkAmsMkheDirectVerifiedContributionV1, ZkAmsMkheErrorV1, ZkAmsMkheEvaluatedKeySorafsPointerV1,
    ZkAmsMkheFullRosterDecryptionResultV1, ZkAmsMkheGovernedActiveRosterV1,
    ZkAmsMkheGovernedCollectiveKeyMaterialIdentityV1, ZkAmsMkheGovernedParticipantV1,
    ZkAmsMkheGovernedRosterWireV1, ZkAmsMkheIdentifiableAbortV1, ZkAmsMkheIdentifiableCksAbortV1,
    ZkAmsMkheIdentifiableDecryptionAbortV1, ZkAmsMkheNoiseCertificateV1,
    ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1, ZkAmsMkheOwnedCollectiveSourceProofEvidenceV1,
    ZkAmsMkheOwnedCollectiveSourceStatementEvidenceV1, ZkAmsMkheProofEnvelopeWireV1,
    ZkAmsMkheProofKindV1, ZkAmsMkheReadinessV1, ZkAmsMkheReleaseManifestV1,
    ZkAmsMkheResourceCertificateV1, ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheRosterKeyProofV1,
    ZkAmsMkheSecurityAttackRecordV1, ZkAmsMkheSecurityAttackV1, ZkAmsMkheSecurityCandidateV1,
    ZkAmsMkheSecurityCertificateV1, ZkAmsMkheSecurityEstimatorSuiteV1, ZkAmsMkheSeededRkgKeyWireV1,
    ZkAmsMkheSeekableEvaluatedKeyAccountingV1, ZkAmsMkheWireBindingV1, ZkAmsPhase3BatchAnchorV1,
    ZkAmsPhase3FoldHistoryV1, ZkAmsPhase3GovernedBatchV1, ZkAmsPhase3TerminalContextV1,
    ZkAmsPhase3TerminalImplementationV1, ZkAmsPhase3TerminalProverOutputV1,
    ZkAmsPhase3TerminalReceiptV1, ZkAmsPhase23AccumulatorShapeV1,
    ZkAmsPhase23CommitmentPreimageLayoutV1, ZkAmsPhase23CrossTermCommitmentV1,
    ZkAmsPhase23EncryptedBindingV1, ZkAmsPhase23EncryptedImplementationV1,
    ZkAmsPhase23EquationCertificateV1, ZkAmsPhase23FreshnessCommitV1,
    ZkAmsPhase23FreshnessContextV1, ZkAmsPhase23FreshnessPhaseV1, ZkAmsPhase23FreshnessReceiptV1,
    ZkAmsPhase23FreshnessRevealV1, ZkAmsPhase23MapKindV1, ZkAmsPhase23MaterializedAccumulatorsV1,
    ZkAmsPhase23PackedAccumulatorSetV1, ZkAmsPhase23PendingRevealV1,
    ZkAmsPhase23PublicAccumulatorV1, ZkAmsPhase23PublicChallengeFamilyV1,
    ZkAmsPhase23PublicChallengeRoleV1, ZkAmsPhase23PublicChallengeV1,
    ZkAmsPhase23PublicFoldHistoryV1, ZkAmsPhase23PublicFoldRecordV1, ZkAmsPhase23ReleaseMapsV1,
    ZkAmsPhase23SparseMapV1, ZkAmsPhase23StrictPublicInstanceV1, ZkAmsPhase23VerifiedCommitSetV1,
    ZkAmsProofContextV1, ZkAmsT256GaloisKeyScheduleEntryV1, ZkAmsT256GaloisKeyScheduleV1,
    ZkAmsT256PackedPlaintextV1, ZkAmsT256PackingLayoutV1, ZkAmsT256ReleasePackingCertificateV1,
    ZkAmsT256RotationCertificateV1, ZkAmsT256RotationDirectionV1, ZkAmsT256RotationV1,
    admit_zk_ams_mkhe_direct_contribution_set_v1, aggregate_zk_ams_mkhe_collective_public_key_v1,
    combine_zk_ams_mkhe_cks_v1, commit_zk_ams_phase23_freshness_v1,
    decode_zk_ams_t256_packed_plaintext_v1, encode_zk_ams_t256_packed_plaintext_v1,
    encrypt_zk_ams_mkhe_collective_packed_v1, finalize_zk_ams_phase23_freshness_v1,
    generate_zk_ams_mkhe_collective_party_state_v1, open_zk_ams_phase23_freshness_reveal_v1,
    permute_zk_ams_t256_slots_v1, prove_zk_ams_admission_relation_v1,
    prove_zk_ams_mkhe_active_collective_public_key_v1, prove_zk_ams_mkhe_active_rkg_round_one_v1,
    prove_zk_ams_mkhe_active_rkg_round_two_v1, prove_zk_ams_mkhe_cks_contribution_v1,
    prove_zk_ams_mkhe_decryption_share_v1, prove_zk_ams_phase3_terminal_v1,
    reconstruct_zk_ams_mkhe_decryption_share_v1, rotate_zk_ams_t256_packed_plaintext_v1,
    split_zk_ams_mkhe_decryption_share_v1, validate_zk_ams_t256_galois_key_exponents_v1,
    validate_zk_ams_t256_galois_key_schedule_v1, verify_combine_decode_zk_ams_mkhe_decryption_v1,
    verify_zk_ams_admission_relation_v1, verify_zk_ams_mkhe_active_collective_public_key_v1,
    verify_zk_ams_mkhe_active_rkg_round_one_v1, verify_zk_ams_mkhe_active_rkg_round_two_v1,
    verify_zk_ams_mkhe_cks_contribution_v1, verify_zk_ams_mkhe_decryption_share_v1,
    verify_zk_ams_phase3_terminal_v1, zk_ams_admission_relation_dimensions_v1,
    zk_ams_compiled_profile_digest_v1, zk_ams_mkhe_active_collective_public_a_v1,
    zk_ams_mkhe_active_rkg_linear_proof_security_v1, zk_ams_mkhe_cks_resource_evidence_v1,
    zk_ams_mkhe_cks_statement_digest_v1, zk_ams_mkhe_collect_active_round_v1,
    zk_ams_mkhe_compact_key_switch_ring_multiplications_v1,
    zk_ams_mkhe_decryption_resource_evidence_v1, zk_ams_mkhe_direct_noise_certificate_v1,
    zk_ams_mkhe_direct_noise_integration_certificate_v1,
    zk_ams_mkhe_direct_noise_integration_for_admitted_keys_v1, zk_ams_mkhe_direct_proof_audit_v1,
    zk_ams_mkhe_direct_resource_certificate_v1, zk_ams_mkhe_manifest_digest_v1,
    zk_ams_mkhe_noise_certificate_v1, zk_ams_mkhe_readiness_digest_v1, zk_ams_mkhe_readiness_v1,
    zk_ams_mkhe_release_manifest_v1, zk_ams_mkhe_resource_certificate_digest_v1,
    zk_ams_mkhe_resource_certificate_v1, zk_ams_mkhe_security_candidate_input_digest_v1,
    zk_ams_mkhe_security_candidate_v1, zk_ams_mkhe_security_certificate_v1,
    zk_ams_mkhe_seekable_evaluated_key_accounting_v1, zk_ams_phase3_nifs_verifier_digest_v1,
    zk_ams_phase3_ordered_public_inputs_digest_v1, zk_ams_phase3_terminal_implementation_v1,
    zk_ams_phase23_cross_term_v1, zk_ams_phase23_encrypted_implementation_v1,
    zk_ams_phase23_equation_certificate_digest_v1, zk_ams_phase23_equation_certificate_v1,
    zk_ams_phase23_fold_linear_v1, zk_ams_phase23_fold_quadratic_v1,
    zk_ams_phase23_materialize_release_accumulators_v1, zk_ams_phase23_release_map_set_digest_v1,
    zk_ams_phase23_release_maps_v1, zk_ams_release_candidate_profile_digest_v1,
    zk_ams_t256_galois_key_schedule_v1, zk_ams_t256_generator_digest_v1,
    zk_ams_t256_packed_subfield_conjugation_exponent_v1, zk_ams_t256_packing_layout_v1,
    zk_ams_t256_release_packing_certificate_v1, zk_ams_t256_rotation_certificate_v1,
    zk_ams_t256_rotation_exponent_for_direction_v1, zk_ams_t256_rotation_exponent_v1,
    zk_ams_t256_rotation_key_plan_v1, zk_ams_t256_rotation_v1,
};

/// Exact canonical COSE `Sig_structure` width in the released Figure 9 relation.
pub const VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1: usize = 368;
/// Exact tagged ISO 18013-5 MSO payload width embedded in the `Sig_structure`.
pub const VEGA_MDL_MSO_PAYLOAD_BYTES_V1: usize = 348;
/// Exact tagged `IssuerSignedItemBytes` width for the private birth date.
pub const VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1: usize = 92;
/// Exact randomizer width inside the birth-date signed item.
pub const VEGA_MDL_BIRTH_RANDOM_BYTES_V1: usize = 16;
/// Exact `YYYY-MM-DD` text width parsed by the released relation.
pub const VEGA_MDL_FULL_DATE_TEXT_BYTES_V1: usize = 10;
/// Exact `YYYY-MM-DDTHH:MM:SSZ` text width parsed by the released relation.
pub const VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1: usize = 20;
/// Lowest trusted UTC presentation year admitted by the released relation.
pub const VEGA_MDL_MIN_PRESENTATION_YEAR_V1: u16 = 1_970;
/// Highest presentation year for which a later four-digit `validUntil` exists.
pub const VEGA_MDL_MAX_PRESENTATION_YEAR_V1: u16 = 9_998;
/// Lowest non-degenerate public age threshold admitted by the released relation.
pub const VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1: u8 = 1;
/// Highest achievable public age threshold admitted by the released relation.
pub const VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1: u8 = 150;

/// Tight first-release cap for one canonical Norito Vega proof.
///
/// A 512 KiB ceiling leaves room for the exact 368-byte Figure 9 relation and
/// Norito framing while preventing this engine from inheriting the much
/// broader per-action opaque-byte allowance.
pub const MAX_VEGA_PROOF_BYTES_V1: usize = 512 * 1024;

/// Big-endian modulus of the canonical T256 scalar field.
///
/// This is also the base-field modulus of NIST P-256.
pub const VEGA_T256_SCALAR_MODULUS_BE_V1: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];

/// Failure while translating canonical Vega field material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaFieldError {
    /// The supplied big-endian integer is not smaller than the T256 scalar
    /// modulus.
    #[error("integer is not a canonical T256 scalar")]
    NonCanonicalScalar,
    /// The zero scalar does not have a multiplicative inverse.
    #[error("cannot invert the zero T256 scalar")]
    InversionOfZero,
}

/// Canonical T256 scalar used by Vega public inputs and proof-system algebra.
///
/// Construction is deliberately non-reducing: byte strings at or above the
/// modulus are rejected rather than silently mapped into the field.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VegaT256ScalarV1(Fq);

impl VegaT256ScalarV1 {
    /// Return the additive identity.
    #[must_use]
    pub const fn zero() -> Self {
        Self(Fq::ZERO)
    }

    /// Return the multiplicative identity.
    #[must_use]
    pub const fn one() -> Self {
        Self(Fq::ONE)
    }

    /// Parse one canonical 32-byte big-endian scalar without modular reduction.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::NonCanonicalScalar`] when `bytes` represents
    /// an integer greater than or equal to the scalar modulus.
    pub fn from_be_bytes_exact(bytes: [u8; 32]) -> Result<Self, VegaFieldError> {
        if bytes >= VEGA_T256_SCALAR_MODULUS_BE_V1 {
            return Err(VegaFieldError::NonCanonicalScalar);
        }
        // `halo2curves` 0.9 exposes the P-256 base-field representation in
        // little-endian order. Keep that implementation detail behind this
        // explicitly big-endian Vega boundary.
        let mut repr = bytes;
        repr.reverse();
        let value = Option::<Fq>::from(Fq::from_repr(repr.into()))
            .ok_or(VegaFieldError::NonCanonicalScalar)?;
        Ok(Self(value))
    }

    /// Parse one canonical 32-byte little-endian proof scalar without modular
    /// reduction.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::NonCanonicalScalar`] when `bytes` represents
    /// an integer greater than or equal to the scalar modulus.
    pub fn from_le_bytes_exact(mut bytes: [u8; 32]) -> Result<Self, VegaFieldError> {
        bytes.reverse();
        Self::from_be_bytes_exact(bytes)
    }

    /// Reduce an exact 64-byte little-endian uniform string as specified by
    /// the pinned Vega Fiat--Shamir transcript.
    #[must_use]
    pub fn from_uniform_le_bytes(bytes: [u8; 64]) -> Self {
        Self(Fq::from_uniform_bytes(&bytes))
    }

    /// Construct a scalar from an unsigned 64-bit integer.
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        Self(Fq::from(value))
    }

    /// Return the exact canonical 32-byte big-endian representation.
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 32] {
        let mut bytes: [u8; 32] = self.0.to_repr().into();
        bytes.reverse();
        bytes
    }

    /// Return the exact canonical 32-byte little-endian proof encoding.
    #[must_use]
    pub fn to_le_bytes(self) -> [u8; 32] {
        let mut bytes = self.to_be_bytes();
        bytes.reverse();
        bytes
    }

    /// Return whether this field element is zero.
    #[must_use]
    pub fn is_zero(self) -> bool {
        bool::from(self.0.is_zero())
    }

    /// Return the multiplicative inverse.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::InversionOfZero`] for the additive identity.
    pub fn inverse(self) -> Result<Self, VegaFieldError> {
        Option::<Fq>::from(self.0.invert())
            .map(Self)
            .ok_or(VegaFieldError::InversionOfZero)
    }

    /// Square this scalar.
    #[must_use]
    pub fn square(self) -> Self {
        Self(self.0.square())
    }
}

impl Default for VegaT256ScalarV1 {
    fn default() -> Self {
        Self::zero()
    }
}

impl Add for VegaT256ScalarV1 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for VegaT256ScalarV1 {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for VegaT256ScalarV1 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for VegaT256ScalarV1 {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul for VegaT256ScalarV1 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl MulAssign for VegaT256ScalarV1 {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0;
    }
}

impl Neg for VegaT256ScalarV1 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl fmt::Debug for VegaT256ScalarV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("VegaT256ScalarV1")
            .field(&hex::encode(self.to_be_bytes()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use halo2curves::ff::PrimeField;

    use super::*;

    #[test]
    fn t256_scalar_modulus_is_exactly_the_p256_base_modulus() {
        assert_eq!(
            Fq::MODULUS,
            "0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff"
        );
        let mut below = VEGA_T256_SCALAR_MODULUS_BE_V1;
        below[31] -= 1;
        let parsed = VegaT256ScalarV1::from_be_bytes_exact(below).expect("q - 1 is canonical");
        assert_eq!(parsed.to_be_bytes(), below);
        assert_eq!(
            VegaT256ScalarV1::from_be_bytes_exact(VEGA_T256_SCALAR_MODULUS_BE_V1),
            Err(VegaFieldError::NonCanonicalScalar)
        );
        assert_eq!(
            VegaT256ScalarV1::from_be_bytes_exact([0xff; 32]),
            Err(VegaFieldError::NonCanonicalScalar)
        );
    }

    #[test]
    fn t256_scalar_big_endian_boundary_does_not_reduce() {
        for value in [0_u64, 1, 255, 256, u32::MAX.into(), u64::MAX] {
            let scalar = VegaT256ScalarV1::from_u64(value);
            let mut expected = [0_u8; 32];
            expected[24..].copy_from_slice(&value.to_be_bytes());
            assert_eq!(scalar.to_be_bytes(), expected);
            assert_eq!(VegaT256ScalarV1::from_be_bytes_exact(expected), Ok(scalar));
        }
    }
}
