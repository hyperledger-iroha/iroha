//! Deterministic, non-shipping native privacy release evidence.
//!
//! This module is deliberately behind the non-default
//! `privacy-release-evidence` feature. It is compiled only into the isolated
//! Taira release runner, never into `irohad`. The deterministic entropy below
//! is suitable only for reproducible release fixtures; neither its seed nor
//! any witness byte is exposed by the public evidence types. Canonical proof
//! bytes do cross the release-evidence boundary so the isolated Taira runner
//! can authenticate, persist, and exact-compare what production verified.

use core::{
    fmt,
    num::{NonZeroU32, NonZeroU64},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use incrementalmerkletree::{Hashable as _, Level};
use iroha_crypto::{Algorithm, KeyPair};
pub use iroha_data_model::privacy::{
    PrivacyExact12FixtureErrorV1, PrivacyExact12TypedEnvelopeRowV1,
    privacy_exact12_matrix_bytes_v1, privacy_exact12_typed_envelope_rows_v1,
};
use iroha_data_model::{
    isi::privacy::SubmitPrivacyProofV1,
    metadata::Metadata,
    prelude::{AccountId, AssetDefinitionId, ChainId, DomainId, Name},
    privacy::{
        BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1,
        BOOTLE_LANTERN_MAX_DISCLOSED_ATTRIBUTES_V1, BootleLanternAllowedAttributeValuesV1,
        BootleLanternAttributeValueV1, BootleLanternDisclosedAttributeV1,
        BootleLanternIssuerPolicyLifecycleV1, BootleLanternIssuerPolicyV1,
        BootleLanternIssuerPublicMatrixV1, BootleLanternPolynomialV1,
        IrohaBootleLanternAnoncredStatementV1, IrohaZkAmsProofV1, IrohaZkAmsStatementV1,
        OrchardHalo2ActionsStatementV1, PrivacyActiveLifecycleV1,
        PrivacyBootleLanternIssuerPolicyDigestV1, PrivacyChallengeV1, PrivacyConsensusLimitsV1,
        PrivacyCredentialDocumentTypeV1, PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1,
        PrivacyJindoFieldElementV1, PrivacyNamespaceScopeV1, PrivacyNamespaceV1,
        PrivacyNativeConsensusBindingV1, PrivacyNullifierV1, PrivacyOrchardActionV1,
        PrivacyP256CiphertextV1, PrivacyP256PointV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyPgcAccountBootstrapV1, PrivacyPgcAccountV1,
        PrivacyPgcBootstrapProofBytesV1, PrivacyPolicyDigestV1, PrivacyPolicyIdV1, PrivacyPoolIdV1,
        PrivacyPoolNamespaceV1, PrivacyProofBytesV1, PrivacyProofEnvelopeV1, PrivacyProofV1,
        PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1, PrivacyRootV1,
        PrivacySessionTranscriptDigestV1, PrivacyStatementContextV1, PrivacyStatementDigestV1,
        PrivacyStatementSchemaDigestV1, PrivacyStatementV1, PrivacyTransactionIntentDigestV1,
        PrivacyValueBalanceDirectionV1, PrivacyValueBalanceV1,
        PrivacyVegaIssuerRecordLifecycleV1, PrivacyVegaIssuerRecordV1, PrivacyVegaMdlDateV1,
        PrivacyVegaMdlDigestAlgorithmV1,
        PrivacyVegaMdlNamespaceV1, PrivacyVegaMdlSignatureAlgorithmV1, PrivacyVerifierDigestV1,
        PrivacyZkAmsActionV1, PrivacyZkAmsAdmissionAnchorV1, PrivacyZkAmsBatchAdmissionV1,
        PrivacyZkAmsCredentialNonceV1, PrivacyZkAmsKeyImageV1, PrivacyZkAmsPersonhoodCredentialV1,
        PrivacyZkAmsProvisionAccountV1, PrivacyZkAmsRegistryIdV1,
        PrivacyZkAmsRegistryRecordDigestV1, PrivacyZkAmsSeedPublicKeyV1,
        PrivacyZkAmsSubjectCommitmentV1, TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
        VegaExistingCredentialStatementV1, ZK_AMS_PHC_VERSION_V1, ZkAcePqAuthorizationStatementV1,
        zk_ams_issuer_policy_record_digest_v1, zk_ams_registry_record_digest_v1,
    },
    transaction::{FeePaymentIntent, TransactionBuilder, TransactionPayload},
    zk::{ZkAcePrivacyPublicInputsV1, derive_zk_ace_privacy_authorization_digest},
};
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};
use orchard::{
    keys::FullViewingKey,
    note::{ExtractedNoteCommitment, NoteVersion, RandomSeed, Rho},
    tree::MerkleHashOrchard,
    value::NoteValue,
};
use p256::ecdsa::{
    Signature as P256Signature, SigningKey as P256SigningKey, signature::hazmat::PrehashSigner as _,
};
use p256::elliptic_curve::PrimeField as _;
use rand_core_06::{CryptoRng, Error as RngError06, RngCore};
use sha2::{Digest, Sha256};

use crate::privacy_engines::{
    anonymous_pgc::{
        AnonymousPgcParametersV1, AnonymousPgcPoolInvariantV1, TwistedElGamalCiphertextV1,
        TwistedElGamalKeyPairV1, TwistedElGamalPublicKeyV1,
        bootstrap::{
            AnonymousPgcBootstrapStatementV1, AnonymousPgcBootstrapWitnessV1,
            MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1, PGC_BOOTSTRAP_INITIAL_EPOCH_V1, prove_bootstrap,
            verify_bootstrap_encoded,
        },
        decrypt_u32, encrypt_with_randomness,
        payment::{
            AnonymousPgcPaymentStatementV1, AnonymousPgcPaymentWitnessV1,
            MAX_PGC_PAYMENT_PROOF_BYTES_V1, PGC_PAYMENT_MAX_RECIPIENTS_V1,
            encrypt_signed_with_randomness, prove_payment, verify_payment_encoded,
        },
    },
    bootle_lantern::{
        codec::PROOF_BYTES_V1 as BOOTLE_PROOF_BYTES_V1,
        prove_bound_presentation_v1,
        relation::BootleLanternPresentationWitnessV1,
        ring::ApplicationPolynomialV1,
        transcript::{
            MatrixRoleV1, expand_application_matrix_v1, matrix_seed_v1 as bootle_matrix_seed_v1,
        },
        verify_bound_presentation_encoded_v1,
    },
    fcmp_plus_plus::{
        FCMP_MAX_INPUTS_NATIVE_V1, FCMP_MAX_OUTPUTS_NATIVE_V1, FCMP_MAX_PROOF_WIRE_BYTES_V1,
        FCMP_MAX_TREE_LAYERS_V1, FCMP_MIN_PROOF_WIRE_BYTES_V1, FCMP_PROOF_INPUT_BYTES_V1,
        FCMP_PROOF_WIRE_HEADER_BYTES_V1, FcmpOutputTupleV1, FcmpProofInputPublicV1, FcmpTreeRootV1,
        build_fcmp_frontier_v1, fcmp_release_fixture_v1, fcmp_release_invalid_path_fixture_v1,
        prove_fcmp_plus_plus_v1, verify_fcmp_transaction_v1,
    },
    ivm_private_note::{
        IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1, PRIVATE_NOTE_MAX_INPUTS_V1,
        PRIVATE_NOTE_MAX_OUTPUTS_V1, PRIVATE_NOTE_TREE_DEPTH_V1,
        ivm_private_note_release_fixture_v1, ivm_private_note_release_invalid_path_fixture_v1,
        prove_ivm_private_note_v1_with_rng, verify_ivm_private_note_v1,
    },
    jindo::{
        JINDO_MAX_BATCH_SIZE_V1, JINDO_MAX_COEFFICIENTS_V1, JINDO_NATIVE_PROOF_BYTES_V1,
        JINDO_RING_DEGREE_V1, JindoPrivacyActionTransactionContextV1, JindoPrivacyActionWitnessV1,
        jindo_crs_digest_v1, prepare_jindo_privacy_action_with_rng_v1,
        verify_batched_evaluation_v1,
    },
    orchard::{
        MerklePath, Note, ORCHARD_MAX_ACTIONS_V1, ORCHARD_TREE_DEPTH_V1, OrchardBundleDraftV1,
        OrchardBundlePublicV1, OrchardChangeProverInputV1, OrchardSpendProverInputV1, Scope,
        SpendingKey, authorize_orchard_bundle_v1, orchard_authorization_wire_size_v1,
        orchard_empty_root_v1, prepare_orchard_bundle_v1_with_rng, verify_orchard_bundle_v1,
    },
    p256::{SecretScalarV1, TranscriptBindingV1},
    pq_masp::{
        PQ_MASP_AUTHORIZATION_HEADER_BYTES_V1, PQ_MASP_INPUT_BOUND_V1,
        PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1, PQ_MASP_OUTPUT_BOUND_V1, PQ_MASP_TREE_DEPTH_V1,
        pq_masp_release_fixture_v1, pq_masp_release_invalid_path_fixture_v1,
        prove_pq_masp_v1_with_rng, verify_pq_masp_v1,
    },
    vega::{
        VEGA_MDL_PUBLIC_INPUT_COUNT_V1, VEGA_PRIVACY_ACTION_INDEX_V1, VegaMdlConsensusBindingV1,
        VegaMdlWitnessV1, VegaPrivacyActionPublicInputV1, VegaPrivacyActionTransactionContextV1,
        VegaPrivacyActionWitnessMaterialV1, derive_device_authentication_digest_v1,
        prepare_vega_privacy_action_with_rng_v1, prove_mdl_figure9_v1, verify_mdl_figure9_v1,
    },
    verange::{
        MAX_VERANGE_TYPE1_BATCH_COMMITMENTS_V1, MAX_VERANGE_TYPE1_BATCH_PROOF_BYTES_V1,
        VeRangeBitLengthV1, VeRangeParametersV1, VeRangeType1BatchStatementV1, commit, prove_batch,
        verify_batch_encoded,
    },
    zk_ace::{
        ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1, ZkAcePrivacyWitnessV1, prove_zk_ace_privacy_v1_with_rng,
        verify_zk_ace_privacy_v1,
    },
    zk_ace_stark::{QUERY_COUNT as ZK_ACE_QUERY_COUNT_V1, TRACE_LOG2 as ZK_ACE_TRACE_LOG2_V1},
    zk_ams::{
        MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1, MAX_ZK_AMS_LSAG_PROOF_BYTES_V1,
        ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1, ZK_AMS_MAX_RING_SIZE_V1, ZK_AMS_MIN_RING_SIZE_V1,
        ZK_AMS_PRIVACY_ACTION_INDEX_V1, ZkAmsBatchCredentialWitnessV1,
        ZkAmsPrivacyActionGovernanceV1, ZkAmsPrivacyActionTransactionContextV1, ZkAmsSeedSecretV1,
        prepare_zk_ams_batch_admission_transaction_intent_v1,
        prepare_zk_ams_provision_account_transaction_intent_v1, prove_zk_ams_batch_admission_v1,
        sign_zk_ams_provision_statement_v1, validate_zk_ams_privacy_action_transaction_intent_v1,
        verify_zk_ams_batch_admission_v1, verify_zk_ams_provision_statement_v1,
        zk_ams_batch_admission_adversarial_wires_v1, zk_ams_generator_digest_v1,
        zk_ams_key_image_v1, zk_ams_registry_transition_root_v1, zk_ams_seed_public_key_v1,
    },
    zk_x509::profile::{
        ZK_X509_MAX_PROOF_BYTES_V1, ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1,
        ZK_X509_PROVER_TARGET_SECONDS_V1,
    },
};
use crate::{
    privacy_profiles::compiled_privacy_profile_v1,
    privacy_state::compute_privacy_pgc_account_state_root_v1,
    privacy_verifier::{
        PrivacyVerificationContextV1, PrivacyVerificationErrorV1,
        validate_vega_authoritative_issuer_binding_v1, verify_privacy_envelope_v1,
    },
};
use iroha_zkp_halo2::vega::{
    MAX_VEGA_PROOF_BYTES_V1, VegaMdlProverConfigV1, ZkAmsMaskedProverConfigV1,
    vega_mdl_proof_dimensions_v1,
};

/// Evidence schema version. Any incompatible change requires a new version.
pub const PRIVACY_RELEASE_EVIDENCE_SCHEMA_VERSION_V1: u16 = 1;
/// Four mandatory stages for each protocol in the exact-12 registry.
pub const PRIVACY_RELEASE_CASE_COUNT_V1: usize = 4;
/// Exact eagerly initialized Rayon worker count for every isolated stage.
pub const PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1: u16 = 4;
/// Exact number of mandatory first-release evidence stages.
pub const PRIVACY_RELEASE_STAGE_COUNT_V1: usize =
    PrivacyProtocolIdV1::COUNT * PRIVACY_RELEASE_CASE_COUNT_V1;
/// Largest ordered proof-artifact collection admitted by one release stage.
pub const PRIVACY_RELEASE_MAX_PROOF_ARTIFACTS_V1: usize = 2;
/// Exact number of proof artifacts in the complete exact-12 evidence matrix.
///
/// There is one artifact per stage plus a second, state-lineage artifact for
/// both positive/maximum anonymous-PGC stages and all four ZK-AMS stages.
pub const PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1: usize = PRIVACY_RELEASE_STAGE_COUNT_V1 + 6;

/// Canonical protocol-specific process profile for one isolated release stage.
///
/// `peak_rss_ceiling_bytes` is the operating-system resident-set high-water
/// ceiling. It deliberately does not describe virtual address space: the
/// release runner applies and records a separate `RLIMIT_AS` containment limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrivacyReleaseProcessProfileV1 {
    /// Protocol whose isolated stages must use this exact profile.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact wall-clock ceiling for one stage, in milliseconds.
    pub elapsed_ceiling_millis: u64,
    /// Exact resident-set high-water ceiling for one stage, in bytes.
    pub peak_rss_ceiling_bytes: u64,
}

/// Return the canonical fixed process profile for `protocol_id`, when present.
///
/// `None` means that the protocol uses the release runner's generic reviewed
/// stage limits. A returned profile is exact rather than merely an upper bound:
/// every case for that protocol must carry the same time and peak-RSS values.
pub const fn privacy_release_process_profile_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> Option<PrivacyReleaseProcessProfileV1> {
    match protocol_id {
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => {
            let elapsed_ceiling_millis = match ZK_X509_PROVER_TARGET_SECONDS_V1.checked_mul(1_000) {
                Some(value) => value,
                None => panic!("zk-X509 release target milliseconds overflow u64"),
            };
            Some(PrivacyReleaseProcessProfileV1 {
                protocol_id,
                elapsed_ceiling_millis,
                peak_rss_ceiling_bytes: ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1,
            })
        }
        _ => None,
    }
}

/// Failure to establish the one immutable release-evidence Rayon topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyReleaseRayonPoolErrorV1 {
    /// Another caller initialized the process-global pool, or a worker could
    /// not be created. Either condition makes the release topology ambiguous.
    InitializationRejected,
    /// The calling stage leader was already executing as a Rayon worker.
    LeaderIsWorker,
    /// The initialized pool did not expose the exact four-worker width.
    WorkerCountMismatch,
    /// Not every exact worker reached the post-initialization barrier once.
    WorkerBarrierMismatch,
}

impl fmt::Display for PrivacyReleaseRayonPoolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InitializationRejected => {
                "release Rayon global pool was already initialized or could not start"
            }
            Self::LeaderIsWorker => "release stage leader is already a Rayon worker",
            Self::WorkerCountMismatch => "release Rayon pool has a noncanonical worker count",
            Self::WorkerBarrierMismatch => {
                "release Rayon workers did not reach the exact initialization barrier"
            }
        })
    }
}

impl std::error::Error for PrivacyReleaseRayonPoolErrorV1 {}

/// Initialize and attest the one process-global pool used by release proofs.
///
/// This must be the first Rayon operation in a freshly executed hidden stage.
/// A second call fails closed even when the first call used the correct width,
/// preventing a caller from treating an inherited or preinitialized pool as
/// canonical. Successful return proves that all four distinct workers reached
/// a barrier and that the stage leader is outside the worker set.
pub fn initialize_privacy_release_rayon_pool_v1() -> Result<(), PrivacyReleaseRayonPoolErrorV1> {
    let expected_threads = usize::from(PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1);
    rayon::ThreadPoolBuilder::new()
        .num_threads(expected_threads)
        .build_global()
        .map_err(|_| PrivacyReleaseRayonPoolErrorV1::InitializationRejected)?;
    if rayon::current_thread_index().is_some() {
        return Err(PrivacyReleaseRayonPoolErrorV1::LeaderIsWorker);
    }
    if rayon::current_num_threads() != expected_threads {
        return Err(PrivacyReleaseRayonPoolErrorV1::WorkerCountMismatch);
    }
    let mut worker_indices = rayon::broadcast(|context| context.index());
    worker_indices.sort_unstable();
    if worker_indices != (0..expected_threads).collect::<Vec<_>>() {
        return Err(PrivacyReleaseRayonPoolErrorV1::WorkerBarrierMismatch);
    }
    Ok(())
}

/// Absolute fail-closed ceiling for any one proof artifact declared by evidence.
///
/// The widening conversion is lossless and intentionally binds release
/// evidence to the same consensus constant used by Taira action admission.
pub const PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1: u64 =
    TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as u64;
/// Maximum total canonical proof bytes in a complete exact-12 evidence matrix.
pub const PRIVACY_RELEASE_MAX_TOTAL_PROOF_ARTIFACT_BYTES_V1: u64 =
    match PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1
        .checked_mul(PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1 as u64)
    {
        Some(total) => total,
        None => panic!("privacy release aggregate proof-byte ceiling overflow"),
    };
/// Mandatory evidence cases, in canonical per-protocol order.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(
    tag = "case",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PrivacyReleaseCaseKindV1 {
    /// A canonical public statement is proved and independently verified.
    PositiveCanonicalEndToEnd,
    /// A structurally valid semantic public-input mutation rejects the proof.
    PublicStatementBindingMutation,
    /// Header corruption, interior corruption, and exact truncation all reject.
    ProofCorruptionAndTruncation,
    /// The closed first-release maximum relation shape is proved and verified.
    MaximumShapeResource,
}

impl PrivacyReleaseCaseKindV1 {
    /// Every mandatory case in frozen order.
    pub const ALL: [Self; PRIVACY_RELEASE_CASE_COUNT_V1] = [
        Self::PositiveCanonicalEndToEnd,
        Self::PublicStatementBindingMutation,
        Self::ProofCorruptionAndTruncation,
        Self::MaximumShapeResource,
    ];

    /// Exact stable case label used by release artifacts and child invocation.
    #[must_use]
    pub const fn canonical_label(self) -> &'static str {
        match self {
            Self::PositiveCanonicalEndToEnd => "positive-canonical-end-to-end",
            Self::PublicStatementBindingMutation => "public-statement-binding-mutation",
            Self::ProofCorruptionAndTruncation => "proof-corruption-and-truncation",
            Self::MaximumShapeResource => "maximum-shape-resource",
        }
    }

    /// Parse one exact case label. Aliases and case folding are rejected.
    #[must_use]
    pub const fn from_canonical_label(label: &str) -> Option<Self> {
        match label.as_bytes() {
            b"positive-canonical-end-to-end" => Some(Self::PositiveCanonicalEndToEnd),
            b"public-statement-binding-mutation" => Some(Self::PublicStatementBindingMutation),
            b"proof-corruption-and-truncation" => Some(Self::ProofCorruptionAndTruncation),
            b"maximum-shape-resource" => Some(Self::MaximumShapeResource),
            _ => None,
        }
    }
}

/// One frozen coordinate in the exact first-release evidence schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrivacyReleaseStageCoordinateV1 {
    /// Exact zero-based position in the 48-stage schedule.
    pub stage_ordinal: u16,
    /// Protocol exercised at this position.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Evidence case exercised at this position.
    pub case_kind: PrivacyReleaseCaseKindV1,
}

const fn privacy_release_stage_coordinate_v1(
    stage_ordinal: u16,
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
) -> PrivacyReleaseStageCoordinateV1 {
    PrivacyReleaseStageCoordinateV1 {
        stage_ordinal,
        protocol_id,
        case_kind,
    }
}

/// Sole explicit declaration of the canonical 48-stage release schedule.
///
/// The declaration is intentionally written out rather than reconstructed at
/// runtime. `validate_privacy_release_stage_coordinates_v1` independently
/// derives the protocol-by-case product from the closed enums and rejects any
/// drift in this frozen list.
pub const PRIVACY_RELEASE_STAGE_COORDINATES_V1: [PrivacyReleaseStageCoordinateV1;
    PRIVACY_RELEASE_STAGE_COUNT_V1] = [
    privacy_release_stage_coordinate_v1(
        0,
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        1,
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        2,
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        3,
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        4,
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        5,
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        6,
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        7,
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        8,
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        9,
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        10,
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        11,
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        12,
        PrivacyProtocolIdV1::IrohaZkAmsV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        13,
        PrivacyProtocolIdV1::IrohaZkAmsV1,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        14,
        PrivacyProtocolIdV1::IrohaZkAmsV1,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        15,
        PrivacyProtocolIdV1::IrohaZkAmsV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        16,
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        17,
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        18,
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        19,
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        20,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        21,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        22,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        23,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        24,
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        25,
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        26,
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        27,
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        28,
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        29,
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        30,
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        31,
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        32,
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        33,
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        34,
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        35,
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        36,
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        37,
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        38,
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        39,
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        40,
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        41,
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        42,
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        43,
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
    privacy_release_stage_coordinate_v1(
        44,
        PrivacyProtocolIdV1::PqMaspStarkV0,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ),
    privacy_release_stage_coordinate_v1(
        45,
        PrivacyProtocolIdV1::PqMaspStarkV0,
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation,
    ),
    privacy_release_stage_coordinate_v1(
        46,
        PrivacyProtocolIdV1::PqMaspStarkV0,
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
    ),
    privacy_release_stage_coordinate_v1(
        47,
        PrivacyProtocolIdV1::PqMaspStarkV0,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ),
];

const _: () = assert!(PRIVACY_RELEASE_STAGE_COUNT_V1 == 48);

/// Check a purported stage declaration against the independently derived
/// protocol-by-case enum product.
#[must_use]
pub fn validate_privacy_release_stage_coordinates_v1(
    coordinates: &[PrivacyReleaseStageCoordinateV1],
) -> bool {
    if coordinates.len() != PRIVACY_RELEASE_STAGE_COUNT_V1 {
        return false;
    }
    let mut index = 0_usize;
    for protocol_id in PrivacyProtocolIdV1::ALL {
        for case_kind in PrivacyReleaseCaseKindV1::ALL {
            let Some(coordinate) = coordinates.get(index) else {
                return false;
            };
            let Ok(stage_ordinal) = u16::try_from(index) else {
                return false;
            };
            if coordinate.stage_ordinal != stage_ordinal
                || coordinate.protocol_id != protocol_id
                || coordinate.case_kind != case_kind
            {
                return false;
            }
            index += 1;
        }
    }
    index == coordinates.len()
}

/// Stable classification of the expected verifier failure exercised by a
/// successful evidence stage.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(
    tag = "failure_class",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PrivacyReleaseFailureClassV1 {
    /// The positive or maximum-shape proof verified successfully.
    NotApplicable,
    /// A semantically bound, structurally valid public statement was rejected.
    PublicStatementBindingRejected,
    /// Header/interior corruption and a one-byte truncation were all rejected.
    CanonicalWireCorruptionAndTruncationRejected,
}

/// Closed numeric resource facts. Unit semantics are frozen in the protocol
/// descriptor; unbounded caller-selected labels are intentionally absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
pub struct PrivacyReleaseResourceFactsV1 {
    /// Primary relation dimension actually exercised.
    pub primary_units: u64,
    /// Governed ceiling for `primary_units`.
    pub primary_ceiling: u64,
    /// Secondary relation dimension actually exercised.
    pub secondary_units: u64,
    /// Governed ceiling for `secondary_units`.
    pub secondary_ceiling: u64,
    /// Tree, circuit, or recursion depth actually exercised.
    pub relation_depth: u64,
    /// Governed ceiling for `relation_depth`.
    pub relation_depth_ceiling: u64,
}

impl PrivacyReleaseResourceFactsV1 {
    fn validate(&self) -> bool {
        self.primary_units > 0
            && self.primary_units <= self.primary_ceiling
            && self.secondary_units <= self.secondary_ceiling
            && self.relation_depth <= self.relation_depth_ceiling
    }
}

/// Return the frozen resource facts for one implemented release stage.
///
/// `None` is reserved exclusively for zk-X509 while its complete native
/// release stage and measured resource facts remain unavailable. The runner
/// must fail that protocol closed rather than manufacture placeholder values.
#[must_use]
pub fn privacy_release_resource_facts_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
) -> Option<PrivacyReleaseResourceFactsV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let facts = match protocol_id {
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => PrivacyReleaseResourceFactsV1 {
            primary_units: ZK_ACE_RELEASE_TRACE_ROWS_V1,
            primary_ceiling: ZK_ACE_RELEASE_TRACE_ROWS_V1,
            secondary_units: ZK_ACE_RELEASE_QUERY_COUNT_V1,
            secondary_ceiling: ZK_ACE_RELEASE_QUERY_COUNT_V1,
            relation_depth: ZK_ACE_RELEASE_FRI_ROUNDS_V1,
            relation_depth_ceiling: ZK_ACE_RELEASE_FRI_ROUNDS_V1,
        },
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => PrivacyReleaseResourceFactsV1 {
            primary_units: if maximum { 64 } else { 16 },
            primary_ceiling: 64,
            secondary_units: if maximum {
                u64::try_from(PGC_PAYMENT_MAX_RECIPIENTS_V1).ok()?
            } else {
                2
            },
            secondary_ceiling: u64::try_from(PGC_PAYMENT_MAX_RECIPIENTS_V1).ok()?,
            relation_depth: 32,
            relation_depth_ceiling: 32,
        },
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => PrivacyReleaseResourceFactsV1 {
            primary_units: if maximum { 8 } else { 1 },
            primary_ceiling: u64::try_from(MAX_VERANGE_TYPE1_BATCH_COMMITMENTS_V1).ok()?,
            secondary_units: if maximum { 64 } else { 32 },
            secondary_ceiling: 64,
            relation_depth: if maximum { 8 } else { 6 },
            relation_depth_ceiling: 8,
        },
        PrivacyProtocolIdV1::IrohaZkAmsV1 => {
            let ring_size = if maximum {
                ZK_AMS_MAX_RING_SIZE_V1
            } else {
                ZK_AMS_MIN_RING_SIZE_V1
            };
            let ring_size = u64::try_from(ring_size).ok()?;
            let batch_ceiling = u64::try_from(ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1).ok()?;
            PrivacyReleaseResourceFactsV1 {
                primary_units: ring_size,
                primary_ceiling: u64::try_from(ZK_AMS_MAX_RING_SIZE_V1).ok()?,
                secondary_units: batch_ceiling,
                secondary_ceiling: batch_ceiling,
                relation_depth: ring_size,
                relation_depth_ceiling: u64::try_from(ZK_AMS_MAX_RING_SIZE_V1).ok()?,
            }
        }
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0 => PrivacyReleaseResourceFactsV1 {
            primary_units: VEGA_RELEASE_CONSTRAINT_COUNT_V1,
            primary_ceiling: VEGA_RELEASE_CONSTRAINT_COUNT_V1,
            secondary_units: VEGA_RELEASE_VARIABLE_COUNT_V1,
            secondary_ceiling: VEGA_RELEASE_VARIABLE_COUNT_V1,
            relation_depth: VEGA_RELEASE_COMBINED_SUMCHECK_ROUNDS_V1,
            relation_depth_ceiling: VEGA_RELEASE_COMBINED_SUMCHECK_ROUNDS_V1,
        },
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => return None,
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 => {
            let ring_degree = u64::try_from(JINDO_RING_DEGREE_V1).ok()?;
            PrivacyReleaseResourceFactsV1 {
                primary_units: if maximum {
                    u64::try_from(JINDO_MAX_BATCH_SIZE_V1).ok()?
                } else {
                    1
                },
                primary_ceiling: u64::try_from(JINDO_MAX_BATCH_SIZE_V1).ok()?,
                secondary_units: if maximum {
                    u64::try_from(JINDO_MAX_COEFFICIENTS_V1).ok()?
                } else {
                    4
                },
                secondary_ceiling: u64::try_from(JINDO_MAX_COEFFICIENTS_V1).ok()?,
                relation_depth: ring_degree,
                relation_depth_ceiling: ring_degree,
            }
        }
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => PrivacyReleaseResourceFactsV1 {
            primary_units: if maximum {
                u64::from(BOOTLE_LANTERN_MAX_DISCLOSED_ATTRIBUTES_V1)
            } else {
                1
            },
            primary_ceiling: u64::from(BOOTLE_LANTERN_MAX_DISCLOSED_ATTRIBUTES_V1),
            secondary_units: if maximum {
                u64::from(BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1)
            } else {
                1
            },
            secondary_ceiling: u64::from(BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1),
            relation_depth: 8,
            relation_depth_ceiling: 8,
        },
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => PrivacyReleaseResourceFactsV1 {
            primary_units: if maximum {
                u64::try_from(ORCHARD_MAX_ACTIONS_V1).ok()?
            } else {
                1
            },
            primary_ceiling: u64::try_from(ORCHARD_MAX_ACTIONS_V1).ok()?,
            secondary_units: if maximum {
                u64::try_from(ORCHARD_MAX_ACTIONS_V1).ok()?
            } else {
                0
            },
            secondary_ceiling: u64::try_from(ORCHARD_MAX_ACTIONS_V1).ok()?,
            relation_depth: u64::from(ORCHARD_TREE_DEPTH_V1),
            relation_depth_ceiling: u64::from(ORCHARD_TREE_DEPTH_V1),
        },
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => PrivacyReleaseResourceFactsV1 {
            primary_units: if maximum {
                u64::try_from(FCMP_MAX_INPUTS_NATIVE_V1).ok()?
            } else {
                1
            },
            primary_ceiling: u64::try_from(FCMP_MAX_INPUTS_NATIVE_V1).ok()?,
            secondary_units: if maximum {
                u64::try_from(FCMP_MAX_OUTPUTS_NATIVE_V1).ok()?
            } else {
                1
            },
            secondary_ceiling: u64::try_from(FCMP_MAX_OUTPUTS_NATIVE_V1).ok()?,
            relation_depth: if maximum {
                u64::from(FCMP_MAX_TREE_LAYERS_V1)
            } else {
                1
            },
            relation_depth_ceiling: u64::from(FCMP_MAX_TREE_LAYERS_V1),
        },
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
            let units = if maximum { 2 } else { 1 };
            PrivacyReleaseResourceFactsV1 {
                primary_units: units,
                primary_ceiling: u64::try_from(PRIVATE_NOTE_MAX_INPUTS_V1).ok()?,
                secondary_units: units,
                secondary_ceiling: u64::try_from(PRIVATE_NOTE_MAX_OUTPUTS_V1).ok()?,
                relation_depth: u64::try_from(PRIVATE_NOTE_TREE_DEPTH_V1).ok()?,
                relation_depth_ceiling: u64::try_from(PRIVATE_NOTE_TREE_DEPTH_V1).ok()?,
            }
        }
        PrivacyProtocolIdV1::PqMaspStarkV0 => {
            let units = if maximum { 2 } else { 1 };
            PrivacyReleaseResourceFactsV1 {
                primary_units: units,
                primary_ceiling: u64::try_from(PQ_MASP_INPUT_BOUND_V1).ok()?,
                secondary_units: units,
                secondary_ceiling: u64::try_from(PQ_MASP_OUTPUT_BOUND_V1).ok()?,
                relation_depth: u64::try_from(PQ_MASP_TREE_DEPTH_V1).ok()?,
                relation_depth_ceiling: u64::try_from(PQ_MASP_TREE_DEPTH_V1).ok()?,
            }
        }
    };
    Some(facts)
}

/// One canonical proof artifact produced and independently verified in a stage.
///
/// Artifact semantics and order are frozen by the typed protocol/case pair and
/// its protocol descriptor. No caller-selected label can alter that meaning.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct PrivacyReleaseProofArtifactEvidenceV1 {
    /// Zero-based ordinal, contiguous in collection order.
    pub artifact_ordinal: u8,
    /// Exact canonical proof bytes accepted by the production verifier.
    #[norito(with = "privacy_release_base64_bytes_v1")]
    pub canonical_proof_bytes: Vec<u8>,
    /// SHA-256 of the valid canonical proof before adversarial mutation.
    pub proof_sha256: [u8; 32],
    /// Governed canonical decoder ceiling for this artifact.
    pub proof_bytes_ceiling: u64,
}

mod privacy_release_base64_bytes_v1 {
    use super::*;
    use norito::json::{JsonSerialize as _, Parser};

    pub fn serialize(bytes: &[u8], out: &mut String) {
        BASE64_STANDARD.encode(bytes).json_serialize(out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<Vec<u8>, norito::json::Error> {
        let encoded = parser.parse_string()?;
        let maximum_encoded_bytes = PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1
            .div_ceil(3)
            .checked_mul(4)
            .ok_or_else(|| {
                norito::json::Error::Message("canonical proof base64 ceiling overflowed".to_owned())
            })?;
        let encoded_bytes = u64::try_from(encoded.len()).map_err(|_| {
            norito::json::Error::Message("canonical proof base64 length exceeds u64".to_owned())
        })?;
        if encoded_bytes > maximum_encoded_bytes {
            return Err(norito::json::Error::Message(
                "canonical proof base64 exceeds the Taira artifact ceiling".to_owned(),
            ));
        }
        let bytes = BASE64_STANDARD
            .decode(encoded.as_bytes())
            .map_err(|error| norito::json::Error::Message(error.to_string()))?;
        let decoded_bytes = u64::try_from(bytes.len()).map_err(|_| {
            norito::json::Error::Message("canonical proof byte length exceeds u64".to_owned())
        })?;
        if decoded_bytes > PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1 {
            return Err(norito::json::Error::Message(
                "canonical proof bytes exceed the Taira artifact ceiling".to_owned(),
            ));
        }
        if BASE64_STANDARD.encode(&bytes) != encoded {
            return Err(norito::json::Error::Message(
                "canonical proof bytes use padded standard base64".to_owned(),
            ));
        }
        Ok(bytes)
    }
}

/// One complete native stage result. It contains exact canonical proofs,
/// their hashes, and public resource facts; witness material never crosses
/// this API.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
pub struct PrivacyReleaseStageEvidenceV1 {
    /// Evidence schema version.
    pub schema_version: u16,
    /// Exact global ordinal: protocol discriminant order, then case order.
    pub stage_ordinal: u16,
    /// Protocol under test.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Mandatory case under test.
    pub case_kind: PrivacyReleaseCaseKindV1,
    /// Exact relation/prover/verifier/resource semantics for this protocol.
    pub protocol_descriptor: String,
    /// SHA-256 of the exact canonical public statement material exercised.
    pub public_statement_sha256: [u8; 32],
    /// Bounded ordered canonical proofs produced and independently verified.
    pub proof_artifacts: Vec<PrivacyReleaseProofArtifactEvidenceV1>,
    /// Expected verifier failure class, if this is an adversarial stage.
    pub failure_class: PrivacyReleaseFailureClassV1,
    /// Bounded relation resource facts; proof resources live per artifact.
    pub resources: PrivacyReleaseResourceFactsV1,
}

/// Stable fail-closed error category returned by the native evidence API.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(
    tag = "error_class",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PrivacyReleaseEvidenceErrorClassV1 {
    /// The production engine or its complete release fixture is not available.
    ProtocolUnavailable,
    /// Canonical public inputs or the maximum-shape witness could not be built.
    FixtureConstructionFailed,
    /// The public production prover rejected the valid fixture.
    NativeProverRejected,
    /// The independent public production verifier rejected the valid proof.
    NativeVerifierRejected,
    /// The verifier accepted a semantically mutated public statement.
    PublicStatementMutationAccepted,
    /// The public prover accepted a path that does not resolve to the supplied root.
    InvalidWitnessPathAccepted,
    /// The public prover accepted a malleable or otherwise non-canonical witness.
    NonCanonicalWitnessAccepted,
    /// The strict decoder/verifier accepted a corrupt proof wire.
    ProofCorruptionAccepted,
    /// The strict decoder/verifier accepted a truncated proof wire.
    ProofTruncationAccepted,
    /// A measured relation or proof dimension exceeded its governed ceiling.
    ResourceCeilingExceeded,
    /// Internal order, count, or evidence-shape invariant failed.
    EvidenceInvariant,
    /// Production envelope admission rejected a valid native fixture before
    /// reaching the selected protocol verifier.
    ProductionEnvelopeRejected,
}

/// Fail-closed native stage error without secret-bearing engine diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
pub struct PrivacyReleaseEvidenceErrorV1 {
    /// Protocol whose evidence failed.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Case whose evidence failed.
    pub case_kind: PrivacyReleaseCaseKindV1,
    /// Stable, non-secret failure category.
    pub class: PrivacyReleaseEvidenceErrorClassV1,
}

impl fmt::Display for PrivacyReleaseEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "privacy release stage {}/{} failed: {:?}",
            self.protocol_id.canonical_label(),
            self.case_kind.canonical_label(),
            self.class
        )
    }
}

impl std::error::Error for PrivacyReleaseEvidenceErrorV1 {}

/// Return the exact global stage ordinal from the frozen release declaration.
#[must_use]
pub fn privacy_release_stage_ordinal_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
) -> u16 {
    assert!(
        validate_privacy_release_stage_coordinates_v1(&PRIVACY_RELEASE_STAGE_COORDINATES_V1),
        "frozen release stages must equal the closed protocol-by-case product"
    );
    PRIVACY_RELEASE_STAGE_COORDINATES_V1
        .iter()
        .find(|coordinate| {
            coordinate.protocol_id == protocol_id && coordinate.case_kind == case_kind
        })
        .map(|coordinate| coordinate.stage_ordinal)
        .expect("closed protocol/case coordinate is present in the frozen release declaration")
}

/// Return the exact number of canonical proof artifacts required by a stage.
#[must_use]
pub const fn privacy_release_proof_artifact_count_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
) -> u8 {
    if matches!(protocol_id, PrivacyProtocolIdV1::IrohaZkAmsV1)
        || (matches!(protocol_id, PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            && matches!(
                case_kind,
                PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
                    | PrivacyReleaseCaseKindV1::MaximumShapeResource
            ))
    {
        2
    } else {
        1
    }
}

/// Return the sole canonical decoder ceiling for one ordered proof artifact.
///
/// `None` means that the ordinal is not part of the typed protocol/case stage.
/// The mapping deliberately repeats the production verifier's protocol-local
/// cap at the release boundary so a receipt cannot substitute a broader cap.
#[must_use]
pub fn privacy_release_proof_artifact_ceiling_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
    artifact_ordinal: u8,
) -> Option<u64> {
    match (protocol_id, case_kind, artifact_ordinal) {
        (
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
            | PrivacyReleaseCaseKindV1::MaximumShapeResource,
            0,
        ) => u64::try_from(MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1).ok(),
        (
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
            | PrivacyReleaseCaseKindV1::MaximumShapeResource,
            1,
        )
        | (PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1, _, 0) => {
            u64::try_from(MAX_PGC_PAYMENT_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::VeRangeTransparentRangeV1, _, 0) => {
            u64::try_from(MAX_VERANGE_TYPE1_BATCH_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::IrohaZkAmsV1, _, 0) => {
            u64::try_from(MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::IrohaZkAmsV1, _, 1) => {
            u64::try_from(MAX_ZK_AMS_LSAG_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::VegaExistingCredentialZkV0, _, 0) => {
            u64::try_from(MAX_VEGA_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0, _, 0) => {
            u64::try_from(JINDO_NATIVE_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1, _, 0) => {
            u64::try_from(BOOTLE_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1, _, 0) => {
            u64::try_from(FCMP_MAX_PROOF_WIRE_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1, _, 0) => {
            u64::try_from(IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::PqMaspStarkV0, _, 0) => {
            u64::try_from(PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1).ok()
        }
        (PrivacyProtocolIdV1::OrchardHalo2ActionsV1, _, 0) => {
            orchard_authorization_wire_size_v1(ORCHARD_MAX_ACTIONS_V1)
                .and_then(|ceiling| u64::try_from(ceiling).ok())
        }
        (PrivacyProtocolIdV1::ZkAcePqAuthorizationV0, _, 0) => {
            Some(u64::from(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1))
        }
        (PrivacyProtocolIdV1::IrohaZkX509StarkP256V0, _, 0) => {
            Some(u64::from(ZK_X509_MAX_PROOF_BYTES_V1))
        }
        _ => None,
    }
}

/// Validate the complete ordered proof-artifact shape for one typed stage.
///
/// This rejects missing, extra, reordered, non-contiguous, empty, hash-mismatched,
/// over-ceiling, substituted-ceiling, and globally unbounded artifacts.
#[must_use]
pub fn validate_privacy_release_proof_artifacts_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
    artifacts: &[PrivacyReleaseProofArtifactEvidenceV1],
) -> bool {
    let expected = usize::from(privacy_release_proof_artifact_count_v1(
        protocol_id,
        case_kind,
    ));
    if artifacts.len() != expected || artifacts.len() > PRIVACY_RELEASE_MAX_PROOF_ARTIFACTS_V1 {
        return false;
    }

    let mut total_bytes = 0_u64;
    for (index, artifact) in artifacts.iter().enumerate() {
        let Ok(artifact_ordinal) = u8::try_from(index) else {
            return false;
        };
        let Some(expected_ceiling) =
            privacy_release_proof_artifact_ceiling_v1(protocol_id, case_kind, artifact_ordinal)
        else {
            return false;
        };
        let Ok(proof_bytes) = u64::try_from(artifact.canonical_proof_bytes.len()) else {
            return false;
        };
        let Some(next_total_bytes) = total_bytes.checked_add(proof_bytes) else {
            return false;
        };
        total_bytes = next_total_bytes;

        if artifact.artifact_ordinal != artifact_ordinal
            || artifact.canonical_proof_bytes.is_empty()
            || artifact.proof_sha256 == [0; 32]
            || artifact.proof_sha256 != sha256_v1(&artifact.canonical_proof_bytes)
            || proof_bytes > expected_ceiling
            || artifact.proof_bytes_ceiling != expected_ceiling
            || expected_ceiling > PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1
        {
            return false;
        }
    }
    true
}

/// Execute one mandatory native prove/verify or adversarial stage.
///
/// The selected engine must perform its public production prover and verifier
/// path. Missing complete implementations fail closed; no placeholder result
/// can be encoded as passing evidence.
pub fn run_privacy_release_stage_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<PrivacyReleaseStageEvidenceV1, PrivacyReleaseEvidenceErrorV1> {
    let material = match protocol_id {
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => run_anonymous_pgc_stage_v1(case_kind)
            .map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?,
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
            run_verange_stage_v1(case_kind).map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?
        }
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
            run_orchard_stage_v1(case_kind).map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?
        }
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 => run_jindo_stage_v1(case_kind)
            .map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?,
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => run_bootle_lantern_stage_v1(case_kind)
            .map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?,
        PrivacyProtocolIdV1::IrohaZkAmsV1 => {
            run_zk_ams_stage_v1(case_kind).map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?
        }
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => {
            run_zk_ace_stage_v1(case_kind).map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?
        }
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => run_fcmp_plus_plus_stage_v1(case_kind)
            .map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?,
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => run_ivm_private_note_stage_v1(case_kind)
            .map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?,
        PrivacyProtocolIdV1::PqMaspStarkV0 => {
            run_pq_masp_stage_v1(case_kind).map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?
        }
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0 => {
            run_vega_stage_v1(case_kind).map_err(|class| PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class,
            })?
        }
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => {
            return Err(PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class: PrivacyReleaseEvidenceErrorClassV1::ProtocolUnavailable,
            });
        }
    };

    let expected_resources = privacy_release_resource_facts_v1(protocol_id, case_kind).ok_or(
        PrivacyReleaseEvidenceErrorV1 {
            protocol_id,
            case_kind,
            class: PrivacyReleaseEvidenceErrorClassV1::ProtocolUnavailable,
        },
    )?;
    if material.resources != expected_resources {
        return Err(PrivacyReleaseEvidenceErrorV1 {
            protocol_id,
            case_kind,
            class: PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant,
        });
    }
    if !material.resources.validate() {
        return Err(PrivacyReleaseEvidenceErrorV1 {
            protocol_id,
            case_kind,
            class: PrivacyReleaseEvidenceErrorClassV1::ResourceCeilingExceeded,
        });
    }
    let expected_artifact_count = usize::from(privacy_release_proof_artifact_count_v1(
        protocol_id,
        case_kind,
    ));
    if material.proof_artifacts.len() != expected_artifact_count
        || material.proof_artifacts.len() > PRIVACY_RELEASE_MAX_PROOF_ARTIFACTS_V1
        || material
            .proof_artifacts
            .iter()
            .any(|artifact| artifact.proof.is_empty() || artifact.proof_bytes_ceiling == 0)
    {
        return Err(PrivacyReleaseEvidenceErrorV1 {
            protocol_id,
            case_kind,
            class: PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant,
        });
    }
    let proof_artifacts = material
        .proof_artifacts
        .into_iter()
        .enumerate()
        .map(|(index, artifact)| {
            let proof_sha256 = sha256_v1(&artifact.proof);
            Ok(PrivacyReleaseProofArtifactEvidenceV1 {
                artifact_ordinal: u8::try_from(index)
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
                canonical_proof_bytes: artifact.proof,
                proof_sha256,
                proof_bytes_ceiling: artifact.proof_bytes_ceiling,
            })
        })
        .collect::<Result<Vec<_>, PrivacyReleaseEvidenceErrorClassV1>>()
        .map_err(|class| PrivacyReleaseEvidenceErrorV1 {
            protocol_id,
            case_kind,
            class,
        })?;
    if !validate_privacy_release_proof_artifacts_v1(protocol_id, case_kind, &proof_artifacts) {
        return Err(PrivacyReleaseEvidenceErrorV1 {
            protocol_id,
            case_kind,
            class: PrivacyReleaseEvidenceErrorClassV1::ResourceCeilingExceeded,
        });
    }
    let public_statement_sha256 = sha256_v1(&material.public_statement_material);
    if public_statement_sha256 == [0; 32] {
        return Err(PrivacyReleaseEvidenceErrorV1 {
            protocol_id,
            case_kind,
            class: PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant,
        });
    }
    Ok(PrivacyReleaseStageEvidenceV1 {
        schema_version: PRIVACY_RELEASE_EVIDENCE_SCHEMA_VERSION_V1,
        stage_ordinal: privacy_release_stage_ordinal_v1(protocol_id, case_kind),
        protocol_id,
        case_kind,
        protocol_descriptor: privacy_release_protocol_descriptor_v1(protocol_id).to_owned(),
        public_statement_sha256,
        proof_artifacts,
        failure_class: material.failure_class,
        resources: material.resources,
    })
}

struct ProofArtifactMaterialV1 {
    proof: Vec<u8>,
    proof_bytes_ceiling: u64,
}

struct StageMaterialV1 {
    public_statement_material: Vec<u8>,
    proof_artifacts: Vec<ProofArtifactMaterialV1>,
    failure_class: PrivacyReleaseFailureClassV1,
    resources: PrivacyReleaseResourceFactsV1,
}

fn single_proof_artifact_v1(
    proof: Vec<u8>,
    proof_bytes_ceiling: u64,
) -> Vec<ProofArtifactMaterialV1> {
    vec![ProofArtifactMaterialV1 {
        proof,
        proof_bytes_ceiling,
    }]
}

fn ordered_public_statement_material_v1(
    protocol_id: PrivacyProtocolIdV1,
    statements: &[&[u8]],
) -> Result<Vec<u8>, PrivacyReleaseEvidenceErrorClassV1> {
    if statements.len() != PRIVACY_RELEASE_MAX_PROOF_ARTIFACTS_V1
        || statements.iter().any(|statement| statement.is_empty())
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let mut material = Vec::new();
    material.extend_from_slice(b"iroha.privacy.release.ordered-public-statements.v1");
    let protocol_label = protocol_id.canonical_label().as_bytes();
    material.extend_from_slice(
        &u16::try_from(protocol_label.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            .to_be_bytes(),
    );
    material.extend_from_slice(protocol_label);
    material.push(
        u8::try_from(statements.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
    );
    for (index, statement) in statements.iter().enumerate() {
        material.push(
            u8::try_from(index)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        );
        material.extend_from_slice(
            &u64::try_from(statement.len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
                .to_be_bytes(),
        );
        material.extend_from_slice(statement);
    }
    Ok(material)
}

const ZK_ACE_RELEASE_TRACE_ROWS_V1: u64 = 4_096;
const ZK_ACE_RELEASE_QUERY_COUNT_V1: u64 = 108;
const ZK_ACE_RELEASE_FRI_ROUNDS_V1: u64 = 12;

fn run_zk_ace_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let (public_inputs, witness) = zk_ace_fixture_v1()?;
    let mut rng = EvidenceRng09::new(stage_seed_v1(
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
        case_kind,
    ));
    let proof = prove_zk_ace_privacy_v1_with_rng(&public_inputs, &witness, &mut rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    verify_zk_ace_privacy_v1(&public_inputs, &proof, ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;

    let trace_rows = 1_u64
        .checked_shl(u32::from(ZK_ACE_TRACE_LOG2_V1))
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let query_count = u64::try_from(ZK_ACE_QUERY_COUNT_V1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let fri_rounds = u64::from(ZK_ACE_TRACE_LOG2_V1);
    let proof_bytes = u64::try_from(proof.len())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let proof_ceiling = u64::from(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1);
    if trace_rows != ZK_ACE_RELEASE_TRACE_ROWS_V1
        || query_count != ZK_ACE_RELEASE_QUERY_COUNT_V1
        || fri_rounds != ZK_ACE_RELEASE_FRI_ROUNDS_V1
        || proof_bytes != proof_ceiling
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let original_material = norito::encode_canonical(&public_inputs)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut cross_chain = public_inputs.clone();
            cross_chain.statement.context.chain_id =
                ChainId::from("taira-privacy-release-evidence-cross-context-v1");

            let mut cross_genesis = public_inputs.clone();
            cross_genesis.genesis_hash[0] ^= 0x80;

            let mut wrong_policy_id = public_inputs.clone();
            let mut policy_id = *wrong_policy_id.statement.policy_id.as_bytes();
            policy_id[0] ^= 0x80;
            wrong_policy_id.statement.policy_id = PrivacyPolicyIdV1::new(policy_id);

            let mut wrong_policy_digest = public_inputs.clone();
            let mut policy_digest = *wrong_policy_digest.statement.policy_digest.as_bytes();
            policy_digest[0] ^= 0x80;
            wrong_policy_digest.statement.policy_digest = PrivacyPolicyDigestV1::new(policy_digest);

            let mut malformed_version = public_inputs.clone();
            malformed_version.version = 2;

            let mut malformed_statement = public_inputs.clone();
            malformed_statement.statement.amount = 0;

            for mutation in [
                &cross_chain,
                &cross_genesis,
                &wrong_policy_id,
                &wrong_policy_digest,
                &malformed_version,
                &malformed_statement,
            ] {
                if verify_zk_ace_privacy_v1(mutation, &proof, ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1)
                    .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }
            (
                norito::encode_canonical(&cross_chain)
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let mut corrupt_header = proof.clone();
            let first = corrupt_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_zk_ace_privacy_v1(
                &public_inputs,
                &corrupt_header,
                ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut tampered = proof.clone();
            let middle = tampered.len() / 2;
            let middle_byte = tampered
                .get_mut(middle)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *middle_byte ^= 0x01;
            if verify_zk_ace_privacy_v1(
                &public_inputs,
                &tampered,
                ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let truncated = proof
                .get(..proof.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_zk_ace_privacy_v1(
                &public_inputs,
                truncated,
                ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };

    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(proof, proof_ceiling),
        failure_class,
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: trace_rows,
            primary_ceiling: ZK_ACE_RELEASE_TRACE_ROWS_V1,
            secondary_units: query_count,
            secondary_ceiling: ZK_ACE_RELEASE_QUERY_COUNT_V1,
            relation_depth: fri_rounds,
            relation_depth_ceiling: ZK_ACE_RELEASE_FRI_ROUNDS_V1,
        },
    })
}

fn zk_ace_fixture_v1()
-> Result<(ZkAcePrivacyPublicInputsV1, ZkAcePrivacyWitnessV1), PrivacyReleaseEvidenceErrorClassV1> {
    let witness = ZkAcePrivacyWitnessV1::try_new([0x91; 32], [0x92; 32], [0x93; 32])
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let chain_id = ChainId::from("taira-privacy-release-evidence-v1");
    let domain_id = DomainId::try_new("privacy", "universal")
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let asset_name = "zkace"
        .parse()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let statement = ZkAcePqAuthorizationStatementV1 {
        context: PrivacyStatementContextV1 {
            chain_id: chain_id.clone(),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x94; 32]),
            parameter_id: PrivacyParameterIdV1::new([0x95; 32]),
            parameter_digest: PrivacyParameterDigestV1::new([0x96; 32]),
            verifier_digest: PrivacyVerifierDigestV1::new([0x97; 32]),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new([0x98; 32]),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new([0x99; 32]),
        },
        identity_commitment: witness.identity_commitment_v1(),
        policy_id: PrivacyPolicyIdV1::new([0x9A; 32]),
        policy_digest: PrivacyPolicyDigestV1::new([0x9B; 32]),
        source: privacy_release_account_v1(0x9C)?,
        destination: privacy_release_account_v1(0x9D)?,
        asset_definition_id: AssetDefinitionId::new(domain_id, asset_name),
        amount: 19,
        authorization_epoch: 7,
        replay_nullifier: PrivacyNullifierV1::new([0; 32]),
    };
    let mut public_inputs = ZkAcePrivacyPublicInputsV1::new(statement, [0x9E; 32]);
    let authorization_digest = derive_zk_ace_privacy_authorization_digest(&public_inputs)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    public_inputs.statement.replay_nullifier =
        witness.replay_nullifier_v1(&authorization_digest, &chain_id);
    Ok((public_inputs, witness))
}

fn run_fcmp_plus_plus_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let (inputs, output_openings, root) = fcmp_release_fixture_v1(maximum)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let new_outputs = output_openings
        .iter()
        .map(|opening| opening.output())
        .collect::<Vec<_>>();
    let context_hash = [0xA1; 32];
    let mut rng = EvidenceRng06::new(stage_seed_v1(
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
        case_kind,
    ));
    let bundle = prove_fcmp_plus_plus_v1(&mut rng, context_hash, &inputs, &output_openings, root)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    verify_fcmp_transaction_v1(
        context_hash,
        bundle.proof_wire(),
        bundle.public_inputs(),
        &new_outputs,
        root,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let (proof, public_inputs) = bundle.into_parts();

    let expected_inputs = if maximum {
        FCMP_MAX_INPUTS_NATIVE_V1
    } else {
        1
    };
    let expected_outputs = if maximum {
        FCMP_MAX_OUTPUTS_NATIVE_V1
    } else {
        1
    };
    let expected_layers = if maximum { FCMP_MAX_TREE_LAYERS_V1 } else { 1 };
    let expected_wire_bytes = if maximum {
        FCMP_MAX_PROOF_WIRE_BYTES_V1
    } else {
        FCMP_MIN_PROOF_WIRE_BYTES_V1
    };
    if inputs.len() != expected_inputs
        || public_inputs.len() != expected_inputs
        || new_outputs.len() != expected_outputs
        || root.layers() != expected_layers
        || proof.len() != expected_wire_bytes
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let original_material =
        fcmp_statement_material_v1(context_hash, &public_inputs, &new_outputs, root)?;
    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut cross_context = context_hash;
            cross_context[0] ^= 0x80;
            if verify_fcmp_transaction_v1(cross_context, &proof, &public_inputs, &new_outputs, root)
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }

            let mut changed_key_image = public_inputs.clone();
            let first_input = changed_key_image
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            first_input.key_image = if first_input.key_image != first_input.pseudo_out {
                first_input.pseudo_out
            } else {
                first_input.output_key_tilde
            };
            if verify_fcmp_transaction_v1(
                context_hash,
                &proof,
                &changed_key_image,
                &new_outputs,
                root,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }

            let alternate_root = build_fcmp_frontier_v1(&new_outputs)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?
                .root;
            if alternate_root == root {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            }
            if verify_fcmp_transaction_v1(
                context_hash,
                &proof,
                &public_inputs,
                &new_outputs,
                alternate_root,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }

            (
                fcmp_statement_material_v1(cross_context, &public_inputs, &new_outputs, root)?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let (invalid_inputs, invalid_outputs, invalid_root) =
                fcmp_release_invalid_path_fixture_v1()
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let mut invalid_path_rng = EvidenceRng06::new(stage_seed_v1(
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                PrivacyReleaseCaseKindV1::MaximumShapeResource,
            ));
            if prove_fcmp_plus_plus_v1(
                &mut invalid_path_rng,
                context_hash,
                &invalid_inputs,
                &invalid_outputs,
                invalid_root,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::InvalidWitnessPathAccepted);
            }

            let mut corrupt_header = proof.clone();
            let first = corrupt_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_fcmp_transaction_v1(
                context_hash,
                &corrupt_header,
                &public_inputs,
                &new_outputs,
                root,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let circuit_start = FCMP_PROOF_WIRE_HEADER_BYTES_V1
                .checked_add(
                    public_inputs
                        .len()
                        .checked_mul(FCMP_PROOF_INPUT_BYTES_V1)
                        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
                )
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let mut path_proof_tamper = proof.clone();
            let path_byte = path_proof_tamper
                .get_mut(circuit_start)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *path_byte ^= 0x01;
            if verify_fcmp_transaction_v1(
                context_hash,
                &path_proof_tamper,
                &public_inputs,
                &new_outputs,
                root,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut interior_tamper = proof.clone();
            let interior = interior_tamper.len() / 2;
            let interior_byte = interior_tamper
                .get_mut(interior)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior_byte ^= 0x80;
            if verify_fcmp_transaction_v1(
                context_hash,
                &interior_tamper,
                &public_inputs,
                &new_outputs,
                root,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let truncated = proof
                .get(..proof.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_fcmp_transaction_v1(
                context_hash,
                truncated,
                &public_inputs,
                &new_outputs,
                root,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };

    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof,
            u64::try_from(FCMP_MAX_PROOF_WIRE_BYTES_V1)
                .expect("closed FCMP++ proof ceiling fits u64"),
        ),
        failure_class,
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(inputs.len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(FCMP_MAX_INPUTS_NATIVE_V1)
                .expect("closed FCMP++ input ceiling fits u64"),
            secondary_units: u64::try_from(new_outputs.len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            secondary_ceiling: u64::try_from(FCMP_MAX_OUTPUTS_NATIVE_V1)
                .expect("closed FCMP++ output ceiling fits u64"),
            relation_depth: u64::from(root.layers()),
            relation_depth_ceiling: u64::from(FCMP_MAX_TREE_LAYERS_V1),
        },
    })
}

fn fcmp_statement_material_v1(
    context_hash: [u8; 32],
    public_inputs: &[FcmpProofInputPublicV1],
    new_outputs: &[FcmpOutputTupleV1],
    root: FcmpTreeRootV1,
) -> Result<Vec<u8>, PrivacyReleaseEvidenceErrorClassV1> {
    let mut material =
        Vec::with_capacity(128 + (public_inputs.len() * 5 * 32) + (new_outputs.len() * 3 * 32));
    material.extend_from_slice(b"iroha.privacy.release.fcmp-plus-plus.public-statement.v1");
    material.extend_from_slice(&context_hash);
    material.push(root.layers());
    material.extend_from_slice(&root.point());
    material.extend_from_slice(
        &u32::try_from(public_inputs.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            .to_be_bytes(),
    );
    for public_input in public_inputs {
        for point in [
            public_input.output_key_tilde,
            public_input.linking_tag_generator_tilde,
            public_input.rerandomization_commitment,
            public_input.pseudo_out,
            public_input.key_image,
        ] {
            material.extend_from_slice(&point);
        }
    }
    material.extend_from_slice(
        &u32::try_from(new_outputs.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            .to_be_bytes(),
    );
    for output in new_outputs {
        material.extend_from_slice(&output.encode());
    }
    Ok(material)
}

fn run_anonymous_pgc_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let anonymity_set_size = if maximum { 64_usize } else { 16 };
    let recipient_count = if maximum {
        PGC_PAYMENT_MAX_RECIPIENTS_V1
    } else {
        2
    };
    let base_secret = if maximum { 1_000_u64 } else { 2 };
    let mut key_pairs = (base_secret
        ..base_secret
            .checked_add(
                u64::try_from(anonymity_set_size)
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            )
            .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?)
        .map(|value| {
            TwistedElGamalKeyPairV1::from_secret(evidence_secret_scalar_v1(value))
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    key_pairs.sort_by_key(TwistedElGamalKeyPairV1::public_key);
    let public_keys = key_pairs
        .iter()
        .map(TwistedElGamalKeyPairV1::public_key)
        .collect::<Vec<_>>();
    let sender_index = if maximum { 31 } else { 7 };
    let recipient_indices: &[usize] = if maximum {
        &[0, 1, 2, 3, 4, 5, 6, 7]
    } else {
        &[2, 12]
    };
    let mut transfer_values = vec![0_i64; anonymity_set_size];
    if maximum {
        for index in recipient_indices {
            transfer_values[*index] = 1;
        }
        transfer_values[sender_index] = -i64::try_from(recipient_count)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    } else {
        transfer_values[recipient_indices[0]] = 20;
        transfer_values[recipient_indices[1]] = 30;
        transfer_values[sender_index] = -50;
    }
    let transfer_randomness = (0..anonymity_set_size)
        .map(|index| {
            evidence_secret_scalar_v1(
                (if maximum { 2_000 } else { 100 })
                    + u64::try_from(index).expect("closed PGC index fits u64"),
            )
        })
        .collect::<Vec<_>>();
    let transfers = public_keys
        .iter()
        .copied()
        .zip(&transfer_values)
        .zip(&transfer_randomness)
        .map(|((key, value), randomness)| {
            encrypt_signed_with_randomness(key, *value, randomness)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let opening_balance = if maximum { 10_u32 } else { 100 };
    let current_balance_randomness = (0..anonymity_set_size)
        .map(|index| {
            evidence_secret_scalar_v1(
                (if maximum { 3_000 } else { 200 })
                    + u64::try_from(index).expect("closed PGC index fits u64"),
            )
        })
        .collect::<Vec<_>>();
    let current_balances = public_keys
        .iter()
        .copied()
        .zip(&current_balance_randomness)
        .map(|(key, randomness)| {
            encrypt_with_randomness(key, opening_balance, randomness)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let opening_balances = vec![opening_balance; anonymity_set_size];
    let total_supply = opening_balance
        .checked_mul(
            u32::try_from(anonymity_set_size)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        )
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let bootstrap = anonymous_pgc_bootstrap_material_v1(
        &public_keys,
        &current_balances,
        &opening_balances,
        &current_balance_randomness,
        total_supply,
        case_kind,
    )?;
    let bootstrap_digest = bootstrap.bootstrap_digest;
    let bootstrap_proof_digest = bootstrap.bootstrap_proof_digest;
    let pool_invariant =
        AnonymousPgcPoolInvariantV1::new(total_supply, bootstrap_digest, bootstrap_proof_digest)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if pool_invariant.total_supply() != total_supply
        || pool_invariant.bootstrap_digest() != bootstrap_digest
        || pool_invariant.bootstrap_proof_digest() != bootstrap_proof_digest
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let binding = anonymous_pgc_binding_v1([0x82; 32])?;
    let statement = AnonymousPgcPaymentStatementV1::new(
        &public_keys,
        &transfers,
        &current_balances,
        recipient_count,
        pool_invariant,
        binding,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let witness = AnonymousPgcPaymentWitnessV1 {
        transfer_values: &transfer_values,
        transfer_randomness: &transfer_randomness,
        sender_index,
        sender_secret: key_pairs[sender_index].secret_scalar(),
    };
    let mut rng = EvidenceRng06::new(stage_seed_v1(
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        case_kind,
    ));
    let proof = prove_payment(&statement, &witness, &mut rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let proof_bytes = proof.encode();
    let verified_payment = verify_payment_encoded(&statement, &proof_bytes)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    if verified_payment.next_balance_ciphertexts().len() != anonymity_set_size {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    for (index, (key_pair, ciphertext)) in key_pairs
        .iter()
        .zip(verified_payment.next_balance_ciphertexts())
        .enumerate()
    {
        let expected = i64::from(opening_balance)
            .checked_add(transfer_values[index])
            .and_then(|balance| u32::try_from(balance).ok())
            .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
        let observed = decrypt_u32(key_pair.secret_scalar(), *ciphertext)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
        if observed != expected {
            return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
        }
    }

    let authoritative_current_accounts =
        anonymous_pgc_account_table_v1(&public_keys, &current_balances)?;
    if authoritative_current_accounts != bootstrap.accounts {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let successor_accounts =
        anonymous_pgc_account_table_v1(&public_keys, verified_payment.next_balance_ciphertexts())?;
    let successor_epoch = bootstrap
        .initial_epoch
        .checked_add(1)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let successor_root = compute_privacy_pgc_account_state_root_v1(
        bootstrap.namespace,
        successor_epoch,
        total_supply,
        &successor_accounts,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    if successor_root.is_zero() || successor_root == bootstrap.initial_root {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let mut payment_statement_material = anonymous_pgc_statement_material_v1(&statement, &binding);
    let namespace_encoding = norito::encode_canonical(&bootstrap.namespace)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    payment_statement_material
        .extend_from_slice(b"iroha.privacy.release.anonymous-pgc.account-root-effect.v1");
    payment_statement_material.extend_from_slice(
        &u32::try_from(namespace_encoding.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            .to_be_bytes(),
    );
    payment_statement_material.extend_from_slice(&namespace_encoding);
    payment_statement_material.extend_from_slice(&bootstrap.initial_epoch.to_be_bytes());
    payment_statement_material.extend_from_slice(bootstrap.initial_root.as_bytes());
    payment_statement_material.extend_from_slice(&successor_epoch.to_be_bytes());
    payment_statement_material.extend_from_slice(successor_root.as_bytes());
    payment_statement_material.extend_from_slice(&total_supply.to_be_bytes());
    let payment_proof_ceiling = u64::try_from(MAX_PGC_PAYMENT_PROOF_BYTES_V1)
        .expect("closed PGC payment proof ceiling fits u64");
    let (public_statement_material, proof_artifacts, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => {
            let public_statement_material = ordered_public_statement_material_v1(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                &[
                    bootstrap.public_statement_material.as_slice(),
                    payment_statement_material.as_slice(),
                ],
            )?;
            (
                public_statement_material,
                vec![
                    ProofArtifactMaterialV1 {
                        proof: bootstrap.proof,
                        proof_bytes_ceiling: u64::try_from(MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1)
                            .expect("closed PGC bootstrap proof ceiling fits u64"),
                    },
                    ProofArtifactMaterialV1 {
                        proof: proof_bytes,
                        proof_bytes_ceiling: payment_proof_ceiling,
                    },
                ],
                PrivacyReleaseFailureClassV1::NotApplicable,
            )
        }
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut mutated_digest = binding.statement_digest;
            mutated_digest[0] ^= 0x80;
            let mutated_binding = anonymous_pgc_binding_v1(mutated_digest)?;
            let mutated = AnonymousPgcPaymentStatementV1::new(
                &public_keys,
                &transfers,
                &current_balances,
                recipient_count,
                pool_invariant,
                mutated_binding,
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            if verify_payment_encoded(&mutated, &proof_bytes).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                anonymous_pgc_statement_material_v1(&mutated, &mutated_binding),
                single_proof_artifact_v1(proof_bytes, payment_proof_ceiling),
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let mut corrupt = proof_bytes.clone();
            let first = corrupt
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_payment_encoded(&statement, &corrupt).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_interior = proof_bytes.clone();
            let interior_index = corrupt_interior.len() / 2;
            let interior = corrupt_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_payment_encoded(&statement, &corrupt_interior).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let truncated = proof_bytes
                .get(..proof_bytes.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_payment_encoded(&statement, truncated).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                payment_statement_material,
                single_proof_artifact_v1(proof_bytes, payment_proof_ceiling),
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };
    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts,
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(anonymity_set_size)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: 64,
            secondary_units: u64::try_from(recipient_count)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            secondary_ceiling: u64::try_from(PGC_PAYMENT_MAX_RECIPIENTS_V1)
                .expect("closed PGC recipient ceiling fits u64"),
            relation_depth: 32,
            relation_depth_ceiling: 32,
        },
        failure_class,
    })
}

struct AnonymousPgcBootstrapMaterialV1 {
    public_statement_material: Vec<u8>,
    proof: Vec<u8>,
    bootstrap_digest: [u8; 32],
    bootstrap_proof_digest: [u8; 32],
    namespace: PrivacyNamespaceV1,
    initial_epoch: u64,
    initial_root: PrivacyRootV1,
    accounts: Vec<PrivacyPgcAccountV1>,
}

fn anonymous_pgc_account_table_v1(
    public_keys: &[TwistedElGamalPublicKeyV1],
    encrypted_balances: &[TwistedElGamalCiphertextV1],
) -> Result<Vec<PrivacyPgcAccountV1>, PrivacyReleaseEvidenceErrorClassV1> {
    if public_keys.len() != encrypted_balances.len() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(public_keys
        .iter()
        .zip(encrypted_balances)
        .map(|(public_key, encrypted_balance)| PrivacyPgcAccountV1 {
            public_key: PrivacyP256PointV1::new(*public_key.as_point().as_bytes()),
            encrypted_balance: PrivacyP256CiphertextV1 {
                left: PrivacyP256PointV1::new(*encrypted_balance.left().as_bytes()),
                right: PrivacyP256PointV1::new(*encrypted_balance.right().as_bytes()),
            },
        })
        .collect())
}

fn anonymous_pgc_bootstrap_material_v1(
    public_keys: &[TwistedElGamalPublicKeyV1],
    encrypted_balances: &[TwistedElGamalCiphertextV1],
    balances: &[u32],
    randomness: &[SecretScalarV1],
    total_supply: u32,
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<AnonymousPgcBootstrapMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    const MAXIMUM_ACCOUNT_COUNT: usize = 64;
    let account_count = public_keys.len();
    if account_count == 0
        || account_count > MAXIMUM_ACCOUNT_COUNT
        || encrypted_balances.len() != account_count
        || balances.len() != account_count
        || randomness.len() != account_count
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let observed_supply = balances
        .iter()
        .try_fold(0_u64, |sum, balance| sum.checked_add(u64::from(*balance)))
        .and_then(|sum| u32::try_from(sum).ok())
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    if observed_supply != total_supply {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let namespace = PrivacyNamespaceV1::new(
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
            pool_id: PrivacyPoolIdV1::new([0x89; 32]),
        }),
    );
    let accounts = anonymous_pgc_account_table_v1(public_keys, encrypted_balances)?;
    let initial_root = compute_privacy_pgc_account_state_root_v1(
        namespace,
        PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
        total_supply,
        &accounts,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if initial_root.is_zero() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let bootstrap = PrivacyPgcAccountBootstrapV1 {
        namespace,
        initial_root,
        initial_epoch: PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
        total_supply,
        accounts: accounts.clone(),
    };
    bootstrap
        .validate()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let bootstrap_digest = bootstrap
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let namespace_encoding = norito::to_bytes(&namespace)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let compiled = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let parameters = AnonymousPgcParametersV1::get()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let binding = TranscriptBindingV1 {
        chain_id: b"taira-privacy-release-evidence-v1",
        genesis_hash: [0x81; 32],
        action_index: 0,
        statement_digest: *bootstrap_digest.as_bytes(),
        parameter_id: *compiled.parameter_id.as_bytes(),
        parameter_digest: *compiled.parameter_digest.as_bytes(),
        verifier_digest: *compiled.verifier_digest.as_bytes(),
        statement_schema_digest: *compiled.statement_schema_digest.as_bytes(),
        engine_manifest_digest: *compiled.engine_manifest_digest.as_bytes(),
        generator_digest: parameters.generator_digest(),
    };
    let statement = AnonymousPgcBootstrapStatementV1::new(
        &namespace_encoding,
        *initial_root.as_bytes(),
        PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
        total_supply,
        public_keys,
        encrypted_balances,
        binding,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let witness = AnonymousPgcBootstrapWitnessV1 {
        balances,
        randomness,
    };
    let mut rng = EvidenceRng06::new(stage_purpose_seed_v1(
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        case_kind,
        b"account-bootstrap-proof",
    )?);
    let proof = prove_bootstrap(&statement, &witness, &mut rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?
        .encode();
    let proof = PrivacyPgcBootstrapProofBytesV1::new(proof);
    let effect = verify_bootstrap_encoded(&statement, proof.as_bytes())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let bootstrap_proof_digest = proof
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let recomputed_root = compute_privacy_pgc_account_state_root_v1(
        bootstrap.namespace,
        bootstrap.initial_epoch,
        bootstrap.total_supply,
        &bootstrap.accounts,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    if recomputed_root != bootstrap.initial_root
        || statement.initial_root() != *bootstrap.initial_root.as_bytes()
        || statement.initial_epoch() != bootstrap.initial_epoch
        || statement.total_supply() != bootstrap.total_supply
        || statement.account_count() != bootstrap.accounts.len()
        || statement.bootstrap_table_digest() == [0; 32]
        || effect.total_supply() != bootstrap.total_supply
        || effect.account_count() != bootstrap.accounts.len()
        || effect.bootstrap_table_digest() != statement.bootstrap_table_digest()
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let canonical_bootstrap = norito::encode_canonical(&bootstrap)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let mut public_statement_material = Vec::new();
    public_statement_material
        .extend_from_slice(b"iroha.privacy.release.anonymous-pgc.bootstrap-public-statement.v1");
    public_statement_material.extend_from_slice(
        &u64::try_from(canonical_bootstrap.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            .to_be_bytes(),
    );
    public_statement_material.extend_from_slice(&canonical_bootstrap);
    append_p256_binding_material_v1(&mut public_statement_material, &binding);
    Ok(AnonymousPgcBootstrapMaterialV1 {
        public_statement_material,
        proof: proof.bytes,
        bootstrap_digest: *bootstrap_digest.as_bytes(),
        bootstrap_proof_digest: *bootstrap_proof_digest.as_bytes(),
        namespace,
        initial_epoch: PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
        initial_root,
        accounts,
    })
}

fn anonymous_pgc_binding_v1(
    statement_digest: [u8; 32],
) -> Result<TranscriptBindingV1<'static>, PrivacyReleaseEvidenceErrorClassV1> {
    let parameters = AnonymousPgcParametersV1::get()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let compiled = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if compiled.parameter_digest.as_bytes() != &parameters.parameter_digest() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(TranscriptBindingV1 {
        chain_id: b"taira-privacy-release-evidence-v1",
        genesis_hash: [0x81; 32],
        action_index: 0,
        statement_digest,
        parameter_id: *compiled.parameter_id.as_bytes(),
        parameter_digest: *compiled.parameter_digest.as_bytes(),
        verifier_digest: *compiled.verifier_digest.as_bytes(),
        statement_schema_digest: *compiled.statement_schema_digest.as_bytes(),
        engine_manifest_digest: *compiled.engine_manifest_digest.as_bytes(),
        generator_digest: parameters.generator_digest(),
    })
}

fn anonymous_pgc_statement_material_v1(
    statement: &AnonymousPgcPaymentStatementV1<'_>,
    binding: &TranscriptBindingV1<'_>,
) -> Vec<u8> {
    let mut material = Vec::with_capacity(384);
    material.extend_from_slice(b"iroha.privacy.release.anonymous-pgc.public-statement.v1");
    material.extend_from_slice(
        &u32::try_from(statement.anonymity_set_size())
            .expect("closed PGC anonymity-set size fits u32")
            .to_be_bytes(),
    );
    material.extend_from_slice(
        &u32::try_from(statement.recipient_count())
            .expect("closed PGC recipient count fits u32")
            .to_be_bytes(),
    );
    material.extend_from_slice(&statement.memo_and_ledger_digest());
    append_p256_binding_material_v1(&mut material, binding);
    material
}

fn evidence_secret_scalar_v1(value: u64) -> SecretScalarV1 {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    SecretScalarV1::from_bytes(bytes).expect("non-zero closed evidence scalar is canonical")
}

fn append_p256_binding_material_v1(material: &mut Vec<u8>, binding: &TranscriptBindingV1<'_>) {
    material.extend_from_slice(
        &u32::try_from(binding.chain_id.len())
            .expect("closed evidence chain ID fits u32")
            .to_be_bytes(),
    );
    material.extend_from_slice(binding.chain_id);
    material.extend_from_slice(&binding.genesis_hash);
    material.extend_from_slice(&binding.action_index.to_be_bytes());
    material.extend_from_slice(&binding.statement_digest);
    material.extend_from_slice(&binding.parameter_id);
    material.extend_from_slice(&binding.parameter_digest);
    material.extend_from_slice(&binding.verifier_digest);
    material.extend_from_slice(&binding.statement_schema_digest);
    material.extend_from_slice(&binding.engine_manifest_digest);
    material.extend_from_slice(&binding.generator_digest);
}

fn run_bootle_lantern_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let parameter_digest = [0x31; 32];
    let matrix_seed = bootle_matrix_seed_v1(parameter_digest)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let attribute_matrix =
        expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationAttributes)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let entries = attribute_matrix
        .entries()
        .iter()
        .map(|polynomial| BootleLanternPolynomialV1 {
            coefficients: polynomial.coefficients().to_vec(),
        })
        .collect();
    let mut attributes = [[0_u8; 8]; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1];
    if maximum {
        for (index, attribute) in attributes.iter_mut().enumerate() {
            *attribute = [u8::try_from(index + 1)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
                8];
        }
    } else {
        attributes[1] = [1; 8];
    }
    let required_disclosure_bitmap = if maximum { u8::MAX } else { 0b0000_0010 };
    let allowed_value_ceiling = usize::try_from(BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let allowed_values = attributes
        .iter()
        .copied()
        .enumerate()
        .map(|(index, attribute)| {
            let values = if required_disclosure_bitmap & (1_u8 << index) != 0 {
                if maximum {
                    (1..=allowed_value_ceiling)
                        .map(|ordinal| {
                            let byte = u8::try_from(ordinal).map_err(|_| {
                                PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant
                            })?;
                            Ok(BootleLanternAttributeValueV1::new([byte; 8]))
                        })
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    vec![BootleLanternAttributeValueV1::new(attribute)]
                }
            } else {
                Vec::new()
            };
            Ok(BootleLanternAllowedAttributeValuesV1 { values })
        })
        .collect::<Result<Vec<_>, PrivacyReleaseEvidenceErrorClassV1>>()?;
    let mut policy = BootleLanternIssuerPolicyV1 {
        issuer_id: PrivacyIssuerIdV1::new([11; 32]),
        policy_id: PrivacyPolicyIdV1::new([12; 32]),
        epoch: 1,
        lifecycle: BootleLanternIssuerPolicyLifecycleV1::Active,
        issuer_parameter_id: PrivacyParameterIdV1::new([13; 32]),
        issuer_parameter_digest: PrivacyParameterDigestV1::new([0; 32]),
        issuer_public_matrix: BootleLanternIssuerPublicMatrixV1 { entries },
        required_disclosure_bitmap,
        allowed_values,
        record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]),
    };
    policy.issuer_parameter_digest = policy
        .computed_issuer_parameter_digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    policy.record_digest = policy
        .computed_record_digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    policy
        .validate()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if maximum
        && (policy.allowed_values.len() != BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1
            || policy
                .allowed_values
                .iter()
                .any(|allowed| allowed.values.len() != allowed_value_ceiling))
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let disclosures = attributes
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| required_disclosure_bitmap & (1_u8 << index) != 0)
        .map(|(index, attribute)| BootleLanternDisclosedAttributeV1 {
            index: u8::try_from(index).expect("closed Bootle attribute index fits u8"),
            value: BootleLanternAttributeValueV1::new(attribute),
        })
        .collect::<Vec<_>>();
    let statement = IrohaBootleLanternAnoncredStatementV1 {
        context: PrivacyStatementContextV1 {
            chain_id: "taira-privacy-release-evidence-v1"
                .parse()
                .expect("closed evidence chain ID is canonical"),
            action_index: 3,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([1; 32]),
            parameter_id: PrivacyParameterIdV1::new([2; 32]),
            parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
            verifier_digest: PrivacyVerifierDigestV1::new([4; 32]),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new([5; 32]),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new([6; 32]),
        },
        issuer_id: policy.issuer_id,
        policy_id: policy.policy_id,
        issuer_policy_epoch: policy.epoch,
        issuer_policy_record_digest: policy.record_digest,
        issuer_parameter_id: policy.issuer_parameter_id,
        issuer_parameter_digest: policy.issuer_parameter_digest,
        disclosures,
    };
    let mut signature_two = [ApplicationPolynomialV1::ZERO; 8];
    for (output, attribute) in signature_two.iter_mut().zip(attributes) {
        *output = ApplicationPolynomialV1::from_direct_attribute(attribute);
    }
    let witness = BootleLanternPresentationWitnessV1 {
        randomness: [ApplicationPolynomialV1::ZERO; 16],
        tag: [ApplicationPolynomialV1::ZERO; 8],
        signature_one: [ApplicationPolynomialV1::ZERO; 8],
        signature_two,
        attributes,
    };
    let mut rng = EvidenceRng06::new(stage_seed_v1(
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
        case_kind,
    ));
    let proof = prove_bound_presentation_v1(&statement, &policy, [0x32; 32], &witness, &mut rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let proof_bytes = proof.encode();
    let proof_cap = u32::try_from(BOOTLE_PROOF_BYTES_V1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    verify_bound_presentation_encoded_v1(&statement, &policy, [0x32; 32], &proof_bytes, proof_cap)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let original_typed = PrivacyStatementV1::IrohaBootleLanternAnoncredV1(statement.clone());
    let original_material = norito::encode_canonical(&original_typed)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;

    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut mutated = statement.clone();
            let mut intent = *mutated.context.transaction_intent_digest.as_bytes();
            intent[0] ^= 0x80;
            mutated.context.transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new(intent);
            if verify_bound_presentation_encoded_v1(
                &mutated,
                &policy,
                [0x32; 32],
                &proof_bytes,
                proof_cap,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                norito::encode_canonical(&PrivacyStatementV1::IrohaBootleLanternAnoncredV1(
                    mutated,
                ))
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let mut corrupt_header = proof_bytes.clone();
            let first = corrupt_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_bound_presentation_encoded_v1(
                &statement,
                &policy,
                [0x32; 32],
                &corrupt_header,
                proof_cap,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_interior = proof_bytes.clone();
            let interior_index = corrupt_interior.len() / 2;
            let interior = corrupt_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_bound_presentation_encoded_v1(
                &statement,
                &policy,
                [0x32; 32],
                &corrupt_interior,
                proof_cap,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let truncated = proof_bytes
                .get(..proof_bytes.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_bound_presentation_encoded_v1(
                &statement, &policy, [0x32; 32], truncated, proof_cap,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };
    let disclosed_count = u64::try_from(statement.disclosures.len())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let maximum_allowed_value_count = policy
        .allowed_values
        .iter()
        .map(|allowed| allowed.values.len())
        .max()
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof_bytes,
            u64::try_from(BOOTLE_PROOF_BYTES_V1).expect("closed Bootle proof size fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: disclosed_count,
            primary_ceiling: u64::from(BOOTLE_LANTERN_MAX_DISCLOSED_ATTRIBUTES_V1),
            secondary_units: u64::try_from(maximum_allowed_value_count)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            secondary_ceiling: u64::from(BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1),
            relation_depth: 8,
            relation_depth_ceiling: 8,
        },
        failure_class,
    })
}

const ZK_AMS_RELEASE_CHAIN_ID_V1: &str = "taira-privacy-release-evidence-v1";
const ZK_AMS_RELEASE_GENESIS_HASH_V1: [u8; 32] = [0x11; 32];
const ZK_AMS_RELEASE_BLOCK_TIMESTAMP_MS_V1: u64 = 1_785_024_000_000;
const ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1: u32 = ZK_AMS_PRIVACY_ACTION_INDEX_V1;
const ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1: u32 = ZK_AMS_PRIVACY_ACTION_INDEX_V1;
const ZK_AMS_RELEASE_ADMISSION_CREATION_TIME_MS_V1: u64 = ZK_AMS_RELEASE_BLOCK_TIMESTAMP_MS_V1 - 2;
const ZK_AMS_RELEASE_PROVISION_CREATION_TIME_MS_V1: u64 = ZK_AMS_RELEASE_BLOCK_TIMESTAMP_MS_V1 - 1;
const ZK_AMS_RELEASE_ADMISSION_NONCE_V1: u32 = 21;
const ZK_AMS_RELEASE_PROVISION_NONCE_V1: u32 = 22;

fn run_zk_ams_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let ring_size = if maximum {
        ZK_AMS_MAX_RING_SIZE_V1
    } else {
        ZK_AMS_MIN_RING_SIZE_V1
    };
    let ring = zk_ams_sorted_ring_v1(ring_size)?;
    let admission =
        zk_ams_admission_lineage_material_v1(&ring, ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1, case_kind)?;
    let signer_index = if maximum { ring_size / 2 } else { 5 };
    let signer_secret = &ring
        .get(signer_index)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
        .1;
    let key_image = zk_ams_key_image_v1(signer_secret)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let statement = zk_ams_provision_statement_v1(
        &ring,
        key_image,
        admission.next_root,
        admission.next_epoch,
        admission.next_registry_record_digest,
    )?;
    let admission_intent = validate_zk_ams_privacy_action_transaction_intent_v1(
        &zk_ams_admission_transaction_context_v1()?,
        &admission.statement,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let provision_intent = validate_zk_ams_privacy_action_transaction_intent_v1(
        &zk_ams_provision_transaction_context_v1()?,
        &statement,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    if admission_intent == provision_intent {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let binding = zk_ams_binding_v1(&statement)?;
    let mut rng = EvidenceRng06::new(stage_seed_v1(PrivacyProtocolIdV1::IrohaZkAmsV1, case_kind));
    let provision_proof_bytes = sign_zk_ams_provision_statement_v1(
        &statement,
        &binding,
        signer_index,
        signer_secret,
        &mut rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let provision_effect =
        verify_zk_ams_provision_statement_v1(&statement, &binding, &provision_proof_bytes)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let authoritative_chain_id = ChainId::from(ZK_AMS_RELEASE_CHAIN_ID_V1);
    if statement.context.chain_id != authoritative_chain_id
        || statement.context.action_index != ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    verify_zk_ams_release_production_envelope_v1(
        &statement,
        &provision_proof_bytes,
        &authoritative_chain_id,
        ZK_AMS_RELEASE_GENESIS_HASH_V1,
        ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
    )?;
    let PrivacyZkAmsActionV1::ProvisionAccount(provision) = &statement.action else {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    };
    if provision.admitted_seed_key_ring.len() != ring_size
        || provision_effect.ring != provision.admitted_seed_key_ring
        || provision_effect.key_image != provision.key_image
        || provision.account_registry_root != admission.next_root
        || provision.account_registry_root_epoch != admission.next_epoch
        || statement.registry_record_digest != admission.next_registry_record_digest
        || provision.admitted_seed_key_ring != admission.admitted_seed_key_ring
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let provision_original_material = zk_ams_statement_material_v1(&statement, &binding)?;

    let secondary_units = u64::try_from(admission.anchor_count)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let (public_statement_material, proof_artifacts, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => {
            if provision.admitted_seed_key_ring.len() != ZK_AMS_MAX_RING_SIZE_V1 {
                if maximum {
                    return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
                }
            }
            if admission.anchor_count != ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1 {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            }
            let public_statement_material = ordered_public_statement_material_v1(
                PrivacyProtocolIdV1::IrohaZkAmsV1,
                &[
                    admission.public_statement_material.as_slice(),
                    provision_original_material.as_slice(),
                ],
            )?;
            (
                public_statement_material,
                zk_ams_release_proof_artifacts_v1(admission.proof, provision_proof_bytes),
                PrivacyReleaseFailureClassV1::NotApplicable,
            )
        }
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut mutated_admission = admission.statement.clone();
            let PrivacyZkAmsActionV1::BatchAdmission(batch) = &mut mutated_admission.action else {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            };
            batch.next_account_registry_root = PrivacyRootV1::new([0x39; 32]);
            let mutated_admission_binding = zk_ams_binding_v1(&mutated_admission)?;
            if verify_zk_ams_batch_admission_v1(
                &mutated_admission,
                &mutated_admission_binding,
                &admission.proof,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &mutated_admission,
                    &admission.proof,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }

            let mut mutated_provision = statement.clone();
            let PrivacyZkAmsActionV1::ProvisionAccount(provision) = &mut mutated_provision.action
            else {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            };
            provision.account_registry_root = PrivacyRootV1::new([0x38; 32]);
            let mutated_provision_binding = zk_ams_binding_v1(&mutated_provision)?;
            if verify_zk_ams_provision_statement_v1(
                &mutated_provision,
                &mutated_provision_binding,
                &provision_proof_bytes,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &mutated_provision,
                    &provision_proof_bytes,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }

            let mut wrong_chain = admission.statement.clone();
            wrong_chain.context.chain_id =
                ChainId::from("taira-privacy-release-evidence-zk-ams-wrong-chain");
            let wrong_chain_binding = zk_ams_binding_v1(&wrong_chain)?;
            if verify_zk_ams_batch_admission_v1(
                &wrong_chain,
                &wrong_chain_binding,
                &admission.proof,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &wrong_chain,
                    &admission.proof,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }

            let mut wrong_transaction = statement.clone();
            wrong_transaction.context.transaction_intent_digest =
                admission.statement.context.transaction_intent_digest;
            let wrong_transaction_binding = zk_ams_binding_v1(&wrong_transaction)?;
            if verify_zk_ams_provision_statement_v1(
                &wrong_transaction,
                &wrong_transaction_binding,
                &provision_proof_bytes,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &wrong_transaction,
                    &provision_proof_bytes,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }

            let mutated_admission_material =
                zk_ams_statement_material_v1(&mutated_admission, &mutated_admission_binding)?;
            let mutated_provision_material =
                zk_ams_statement_material_v1(&mutated_provision, &mutated_provision_binding)?;
            (
                ordered_public_statement_material_v1(
                    PrivacyProtocolIdV1::IrohaZkAmsV1,
                    &[
                        mutated_admission_material.as_slice(),
                        mutated_provision_material.as_slice(),
                    ],
                )?,
                zk_ams_release_proof_artifacts_v1(admission.proof, provision_proof_bytes),
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let admission_binding = zk_ams_binding_v1(&admission.statement)?;
            let mut corrupt_batch_header = admission.proof.clone();
            let first = corrupt_batch_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_zk_ams_batch_admission_v1(
                &admission.statement,
                &admission_binding,
                &corrupt_batch_header,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &admission.statement,
                    &corrupt_batch_header,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut corrupt_batch_interior = admission.proof.clone();
            let interior_index = corrupt_batch_interior.len() / 2;
            let interior = corrupt_batch_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_zk_ams_batch_admission_v1(
                &admission.statement,
                &admission_binding,
                &corrupt_batch_interior,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &admission.statement,
                    &corrupt_batch_interior,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let truncated_batch = admission
                .proof
                .get(..admission.proof.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_zk_ams_batch_admission_v1(
                &admission.statement,
                &admission_binding,
                truncated_batch,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &admission.statement,
                    truncated_batch,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }

            for malformed_batch in zk_ams_batch_admission_adversarial_wires_v1(
                &admission.proof,
                admission.anchor_count,
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            {
                if verify_zk_ams_batch_admission_v1(
                    &admission.statement,
                    &admission_binding,
                    &malformed_batch,
                )
                .is_ok()
                    || verify_zk_ams_release_production_envelope_v1(
                        &admission.statement,
                        &malformed_batch,
                        &authoritative_chain_id,
                        ZK_AMS_RELEASE_GENESIS_HASH_V1,
                        ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
                    )
                    .is_ok()
                {
                    return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
                }
            }
            let submax_admission = zk_ams_admission_lineage_material_v1(
                &ring,
                ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1 - 1,
                case_kind,
            )?;
            if submax_admission.anchor_count != ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1 - 1 {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            }
            let submax_binding = zk_ams_binding_v1(&submax_admission.statement)?;
            let submax_malformed = zk_ams_batch_admission_adversarial_wires_v1(
                &submax_admission.proof,
                submax_admission.anchor_count,
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if submax_malformed.len() != 5 {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            }
            for malformed_batch in submax_malformed {
                if verify_zk_ams_batch_admission_v1(
                    &submax_admission.statement,
                    &submax_binding,
                    &malformed_batch,
                )
                .is_ok()
                    || verify_zk_ams_release_production_envelope_v1(
                        &submax_admission.statement,
                        &malformed_batch,
                        &authoritative_chain_id,
                        ZK_AMS_RELEASE_GENESIS_HASH_V1,
                        ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
                    )
                    .is_ok()
                {
                    return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
                }
            }
            let oversized_batch = vec![0_u8; MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1 + 1];
            if verify_zk_ams_batch_admission_v1(
                &admission.statement,
                &admission_binding,
                &oversized_batch,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &admission.statement,
                    &oversized_batch,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut corrupt_provision_header = provision_proof_bytes.clone();
            let first = corrupt_provision_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_zk_ams_provision_statement_v1(&statement, &binding, &corrupt_provision_header)
                .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &statement,
                    &corrupt_provision_header,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut corrupt_provision_interior = provision_proof_bytes.clone();
            let interior_index = corrupt_provision_interior.len() / 2;
            let interior = corrupt_provision_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_zk_ams_provision_statement_v1(
                &statement,
                &binding,
                &corrupt_provision_interior,
            )
            .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &statement,
                    &corrupt_provision_interior,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let truncated_provision = provision_proof_bytes
                .get(..provision_proof_bytes.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_zk_ams_provision_statement_v1(&statement, &binding, truncated_provision)
                .is_ok()
                || verify_zk_ams_release_production_envelope_v1(
                    &statement,
                    truncated_provision,
                    &authoritative_chain_id,
                    ZK_AMS_RELEASE_GENESIS_HASH_V1,
                    ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                ordered_public_statement_material_v1(
                    PrivacyProtocolIdV1::IrohaZkAmsV1,
                    &[
                        admission.public_statement_material.as_slice(),
                        provision_original_material.as_slice(),
                    ],
                )?,
                zk_ams_release_proof_artifacts_v1(admission.proof, provision_proof_bytes),
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };

    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts,
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(ring_size)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(ZK_AMS_MAX_RING_SIZE_V1)
                .expect("closed ZK-AMS ring ceiling fits u64"),
            secondary_units,
            secondary_ceiling: u64::try_from(ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1)
                .expect("closed ZK-AMS admission-batch ceiling fits u64"),
            relation_depth: u64::try_from(ring_size)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            relation_depth_ceiling: u64::try_from(ZK_AMS_MAX_RING_SIZE_V1)
                .expect("closed ZK-AMS cyclic-response ceiling fits u64"),
        },
        failure_class,
    })
}

fn zk_ams_release_proof_artifacts_v1(
    admission_proof: Vec<u8>,
    provision_proof: Vec<u8>,
) -> Vec<ProofArtifactMaterialV1> {
    vec![
        ProofArtifactMaterialV1 {
            proof: admission_proof,
            proof_bytes_ceiling: u64::try_from(MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1)
                .expect("closed ZK-AMS batch proof ceiling fits u64"),
        },
        ProofArtifactMaterialV1 {
            proof: provision_proof,
            proof_bytes_ceiling: u64::try_from(MAX_ZK_AMS_LSAG_PROOF_BYTES_V1)
                .expect("closed ZK-AMS LSAG proof ceiling fits u64"),
        },
    ]
}

struct ZkAmsAdmissionLineageMaterialV1 {
    statement: IrohaZkAmsStatementV1,
    public_statement_material: Vec<u8>,
    proof: Vec<u8>,
    anchor_count: usize,
    next_root: PrivacyRootV1,
    next_epoch: u64,
    next_registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
    admitted_seed_key_ring: Vec<PrivacyZkAmsSeedPublicKeyV1>,
}

fn zk_ams_admission_lineage_material_v1(
    ring: &[([u8; 32], ZkAmsSeedSecretV1)],
    admission_batch_size: usize,
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<ZkAmsAdmissionLineageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    if ring.len() < ZK_AMS_MIN_RING_SIZE_V1
        || ring.len() > ZK_AMS_MAX_RING_SIZE_V1
        || admission_batch_size == 0
        || admission_batch_size > ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1
        || ring.len() < admission_batch_size
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let issuer_signing_key = P256SigningKey::from_bytes((&[7_u8; 32]).into())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let issuer_id = PrivacyIssuerIdV1::new([0x31; 32]);
    let policy_id = PrivacyPolicyIdV1::new([0x35; 32]);
    let registry_id = PrivacyZkAmsRegistryIdV1::new([0x33; 32]);
    let issuer_public_key = zk_ams_issuer_key_v1()?;
    let policy_digest = PrivacyPolicyDigestV1::new([0x36; 32]);
    let issuer_policy_record_digest = zk_ams_issuer_policy_record_digest_v1(
        issuer_id,
        policy_id,
        issuer_public_key,
        policy_digest,
    );
    let credentials = ring
        .iter()
        .enumerate()
        .map(|(index, (public, _))| {
            let index = u8::try_from(index)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let subject_byte = 0x41_u8
                .checked_add(index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let nonce_byte = 0x51_u8
                .checked_add(index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            Ok(PrivacyZkAmsPersonhoodCredentialV1 {
                version: ZK_AMS_PHC_VERSION_V1,
                issuer_id,
                policy_id,
                subject_commitment: PrivacyZkAmsSubjectCommitmentV1::new([subject_byte; 32]),
                seed_public_key: PrivacyZkAmsSeedPublicKeyV1::new(*public),
                credential_nonce: PrivacyZkAmsCredentialNonceV1::new([nonce_byte; 32]),
            })
        })
        .collect::<Result<Vec<_>, PrivacyReleaseEvidenceErrorClassV1>>()?;
    let all_anchors = credentials
        .iter()
        .map(|credential| PrivacyZkAmsAdmissionAnchorV1 {
            phc_hash: credential.digest(),
            seed_public_key: credential.seed_public_key,
        })
        .collect::<Vec<_>>();
    let batch_start = ring
        .len()
        .checked_sub(admission_batch_size)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let prestate_anchors = &all_anchors[..batch_start];
    let anchors = &all_anchors[batch_start..];
    let mut current_root = PrivacyRootV1::new([0x37; 32]);
    let mut current_epoch = 1_u64;
    for prior_batch in prestate_anchors.chunks(ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1) {
        let next_epoch = current_epoch
            .checked_add(1)
            .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
        let batch_size = u32::try_from(prior_batch.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
        current_root = prior_batch.iter().copied().enumerate().fold(
            current_root,
            |prior_root, (index, anchor)| {
                zk_ams_registry_transition_root_v1(
                    registry_id,
                    prior_root,
                    current_epoch,
                    next_epoch,
                    batch_size,
                    u32::try_from(index).expect("closed ZK-AMS admission index fits u32"),
                    anchor,
                )
            },
        );
        current_epoch = next_epoch;
    }
    let registry_record_digest = zk_ams_registry_record_digest_v1(
        issuer_id,
        registry_id,
        policy_id,
        issuer_policy_record_digest,
        policy_digest,
        current_root,
        current_epoch,
    );
    let next_epoch = current_epoch
        .checked_add(1)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let signatures = credentials[batch_start..]
        .iter()
        .map(|credential| {
            let signature: P256Signature = issuer_signing_key
                .sign_prehash(credential.digest().as_bytes())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let signature = signature.normalize_s().unwrap_or(signature);
            Ok(<[u8; 64]>::from(signature.to_bytes()))
        })
        .collect::<Result<Vec<_>, PrivacyReleaseEvidenceErrorClassV1>>()?;
    if anchors.len() != admission_batch_size || signatures.len() != admission_batch_size {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let batch_size = u32::try_from(anchors.len())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let next_root =
        anchors
            .iter()
            .copied()
            .enumerate()
            .fold(current_root, |prior_root, (index, anchor)| {
                zk_ams_registry_transition_root_v1(
                    registry_id,
                    prior_root,
                    current_epoch,
                    next_epoch,
                    batch_size,
                    u32::try_from(index).expect("closed ZK-AMS admission index fits u32"),
                    anchor,
                )
            });
    let governance = ZkAmsPrivacyActionGovernanceV1 {
        issuer_id,
        issuer_public_key,
        issuer_policy_record_digest,
        registry_id,
        registry_record_digest,
        policy_id,
        policy_digest,
    };
    let action = PrivacyZkAmsBatchAdmissionV1 {
        account_registry_root: current_root,
        account_registry_root_epoch: current_epoch,
        next_account_registry_root: next_root,
        next_account_registry_root_epoch: next_epoch,
        anchors: anchors.to_vec(),
    };
    let statement = prepare_zk_ams_batch_admission_transaction_intent_v1(
        &zk_ams_admission_transaction_context_v1()?,
        governance,
        action,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let binding = zk_ams_binding_v1(&statement)?;
    let witnesses = credentials[batch_start..]
        .iter()
        .zip(&signatures)
        .zip(&ring[batch_start..])
        .map(|((credential, signature), (_, secret))| {
            ZkAmsBatchCredentialWitnessV1::new(credential, signature, secret)
        })
        .collect::<Vec<_>>();
    if witnesses.len() != admission_batch_size {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let config = ZkAmsMaskedProverConfigV1::new(1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let mut proof_purpose = Vec::from(b"state-lineage-batch-admission-proof-size".as_slice());
    proof_purpose.extend_from_slice(
        &u32::try_from(admission_batch_size)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            .to_be_bytes(),
    );
    let mut rng = EvidenceRng06::new(stage_purpose_seed_v1(
        PrivacyProtocolIdV1::IrohaZkAmsV1,
        case_kind,
        &proof_purpose,
    )?);
    let proof = prove_zk_ams_batch_admission_v1(&statement, &binding, &witnesses, config, &mut rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let effect = verify_zk_ams_batch_admission_v1(&statement, &binding, &proof)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let authoritative_chain_id = ChainId::from(ZK_AMS_RELEASE_CHAIN_ID_V1);
    if statement.context.chain_id != authoritative_chain_id
        || statement.context.action_index != ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    verify_zk_ams_release_production_envelope_v1(
        &statement,
        &proof,
        &authoritative_chain_id,
        ZK_AMS_RELEASE_GENESIS_HASH_V1,
        ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1,
    )?;
    if effect.issuer_id != issuer_id
        || effect.policy_id != policy_id
        || effect.registry_id != registry_id
        || effect.current_root != current_root
        || effect.current_epoch != current_epoch
        || effect.next_root != next_root
        || effect.next_epoch != next_epoch
        || effect.anchors != anchors
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let next_registry_record_digest = zk_ams_registry_record_digest_v1(
        issuer_id,
        registry_id,
        policy_id,
        issuer_policy_record_digest,
        policy_digest,
        next_root,
        next_epoch,
    );
    let mut public_statement_material = zk_ams_statement_material_v1(&statement, &binding)?;
    public_statement_material
        .extend_from_slice(b"iroha.privacy.release.zk-ams.authoritative-prestate-lineage.v1");
    public_statement_material.extend_from_slice(&1_u64.to_be_bytes());
    public_statement_material.extend_from_slice(PrivacyRootV1::new([0x37; 32]).as_bytes());
    public_statement_material.extend_from_slice(
        &u32::try_from(prestate_anchors.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            .to_be_bytes(),
    );
    for anchor in prestate_anchors {
        public_statement_material.extend_from_slice(anchor.phc_hash.as_bytes());
        public_statement_material.extend_from_slice(anchor.seed_public_key.as_bytes());
    }
    public_statement_material.extend_from_slice(&current_epoch.to_be_bytes());
    public_statement_material.extend_from_slice(current_root.as_bytes());
    let admitted_seed_key_ring = all_anchors
        .iter()
        .map(|anchor| anchor.seed_public_key)
        .collect::<Vec<_>>();
    if admitted_seed_key_ring
        != ring
            .iter()
            .map(|(public, _)| PrivacyZkAmsSeedPublicKeyV1::new(*public))
            .collect::<Vec<_>>()
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(ZkAmsAdmissionLineageMaterialV1 {
        statement,
        public_statement_material,
        proof,
        anchor_count: anchors.len(),
        next_root,
        next_epoch,
        next_registry_record_digest,
        admitted_seed_key_ring,
    })
}

fn zk_ams_sorted_ring_v1(
    size: usize,
) -> Result<Vec<([u8; 32], ZkAmsSeedSecretV1)>, PrivacyReleaseEvidenceErrorClassV1> {
    let mut ring = (1..=size)
        .map(|index| {
            let mut bytes = [0_u8; 32];
            bytes[0] = u8::try_from(index)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let secret = ZkAmsSeedSecretV1::from_bytes(bytes)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            Ok((zk_ams_seed_public_key_v1(&secret), secret))
        })
        .collect::<Result<Vec<_>, _>>()?;
    ring.sort_by_key(|(public, _)| *public);
    Ok(ring)
}

fn privacy_release_account_v1(seed: u8) -> Result<AccountId, PrivacyReleaseEvidenceErrorClassV1> {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(AccountId::new(key_pair.public_key().clone()))
}

fn zk_ams_release_transaction_context_v1(
    creation_time_ms: u64,
    nonce: u32,
) -> Result<ZkAmsPrivacyActionTransactionContextV1, PrivacyReleaseEvidenceErrorClassV1> {
    Ok(ZkAmsPrivacyActionTransactionContextV1 {
        chain_id: ChainId::from(ZK_AMS_RELEASE_CHAIN_ID_V1),
        authority: privacy_release_account_v1(39)?,
        creation_time: Duration::from_millis(creation_time_ms),
        time_to_live: Some(Duration::from_secs(60)),
        nonce: NonZeroU32::new(nonce),
        fee_payment: FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(5_000_000)),
        metadata: Metadata::default(),
    })
}

fn zk_ams_admission_transaction_context_v1()
-> Result<ZkAmsPrivacyActionTransactionContextV1, PrivacyReleaseEvidenceErrorClassV1> {
    zk_ams_release_transaction_context_v1(
        ZK_AMS_RELEASE_ADMISSION_CREATION_TIME_MS_V1,
        ZK_AMS_RELEASE_ADMISSION_NONCE_V1,
    )
}

fn zk_ams_provision_transaction_context_v1()
-> Result<ZkAmsPrivacyActionTransactionContextV1, PrivacyReleaseEvidenceErrorClassV1> {
    zk_ams_release_transaction_context_v1(
        ZK_AMS_RELEASE_PROVISION_CREATION_TIME_MS_V1,
        ZK_AMS_RELEASE_PROVISION_NONCE_V1,
    )
}

fn zk_ams_issuer_key_v1() -> Result<PrivacyP256PointV1, PrivacyReleaseEvidenceErrorClassV1> {
    let signing_key = P256SigningKey::from_bytes((&[7_u8; 32]).into())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let encoded = signing_key.verifying_key().to_encoded_point(true);
    let bytes: [u8; 33] = encoded
        .as_bytes()
        .try_into()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(PrivacyP256PointV1::new(bytes))
}

fn zk_ams_provision_statement_v1(
    ring: &[([u8; 32], ZkAmsSeedSecretV1)],
    key_image: [u8; 32],
    registry_root: PrivacyRootV1,
    registry_epoch: u64,
    registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
) -> Result<IrohaZkAmsStatementV1, PrivacyReleaseEvidenceErrorClassV1> {
    let issuer_id = PrivacyIssuerIdV1::new([0x31; 32]);
    let policy_id = PrivacyPolicyIdV1::new([0x35; 32]);
    let issuer_public_key = zk_ams_issuer_key_v1()?;
    let policy_digest = PrivacyPolicyDigestV1::new([0x36; 32]);
    let governance = ZkAmsPrivacyActionGovernanceV1 {
        issuer_id,
        issuer_public_key,
        issuer_policy_record_digest: zk_ams_issuer_policy_record_digest_v1(
            issuer_id,
            policy_id,
            issuer_public_key,
            policy_digest,
        ),
        registry_id: PrivacyZkAmsRegistryIdV1::new([0x33; 32]),
        registry_record_digest,
        policy_id,
        policy_digest,
    };
    let action = PrivacyZkAmsProvisionAccountV1 {
        account_registry_root: registry_root,
        account_registry_root_epoch: registry_epoch,
        admitted_seed_key_ring: ring
            .iter()
            .map(|(public, _)| PrivacyZkAmsSeedPublicKeyV1::new(*public))
            .collect(),
        account_id: privacy_release_account_v1(40)?,
        key_image: PrivacyZkAmsKeyImageV1::new(key_image),
    };
    prepare_zk_ams_provision_account_transaction_intent_v1(
        &zk_ams_provision_transaction_context_v1()?,
        governance,
        action,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
}

fn zk_ams_binding_v1(
    statement: &IrohaZkAmsStatementV1,
) -> Result<TranscriptBindingV1<'_>, PrivacyReleaseEvidenceErrorClassV1> {
    let statement_digest = PrivacyStatementV1::IrohaZkAmsV1(statement.clone())
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(TranscriptBindingV1 {
        chain_id: statement.context.chain_id.as_str().as_bytes(),
        genesis_hash: ZK_AMS_RELEASE_GENESIS_HASH_V1,
        action_index: statement.context.action_index,
        statement_digest: *statement_digest.as_bytes(),
        parameter_id: *statement.context.parameter_id.as_bytes(),
        parameter_digest: *statement.context.parameter_digest.as_bytes(),
        verifier_digest: *statement.context.verifier_digest.as_bytes(),
        statement_schema_digest: *statement.context.statement_schema_digest.as_bytes(),
        engine_manifest_digest: *statement.context.engine_manifest_digest.as_bytes(),
        generator_digest: zk_ams_generator_digest_v1(),
    })
}

fn verify_zk_ams_release_production_envelope_v1(
    statement: &IrohaZkAmsStatementV1,
    proof: &[u8],
    authoritative_chain_id: &ChainId,
    genesis_hash: [u8; 32],
    authoritative_action_index: u32,
) -> Result<(), PrivacyReleaseEvidenceErrorClassV1> {
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let activation = profile.activation_record(PrivacyProtocolLifecycleV1::Active(
        PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 2,
        },
    ));
    let typed_statement = PrivacyStatementV1::IrohaZkAmsV1(statement.clone());
    let statement_digest = typed_statement
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let proof_bytes = PrivacyProofBytesV1::new(proof.to_vec());
    let action_proof = match &statement.action {
        PrivacyZkAmsActionV1::BatchAdmission(_) => {
            IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(proof_bytes)
        }
        PrivacyZkAmsActionV1::ProvisionAccount(_) => {
            IrohaZkAmsProofV1::Ristretto255LsagProvisionAccount(proof_bytes)
        }
    };
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest,
        statement: typed_statement,
        proof: PrivacyProofV1::IrohaZkAmsV1(action_proof),
    };
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let effects = verify_privacy_envelope_v1(
        &envelope,
        PrivacyVerificationContextV1 {
            activation: &activation,
            consensus_limits: &limits,
            chain_id: authoritative_chain_id,
            genesis_hash,
            current_height: 2,
            expected_action_index: authoritative_action_index,
            block_timestamp_ms: ZK_AMS_RELEASE_BLOCK_TIMESTAMP_MS_V1,
            pgc_state: None,
            orchard_state: None,
            proof_managed_state: None,
            zk_x509_state: None,
            bootle_lantern_policy: None,
            vega_issuer_record: None,
        },
    )
    .map_err(|source| match source {
        PrivacyVerificationErrorV1::NativeZkAms(_) => {
            PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected
        }
        _ => PrivacyReleaseEvidenceErrorClassV1::ProductionEnvelopeRejected,
    })?;
    if effects.protocol_id() != PrivacyProtocolIdV1::IrohaZkAmsV1
        || effects.statement_digest() != statement_digest
        || effects.action_index() != authoritative_action_index
        || effects.encoded_action_bytes() == 0
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(())
}

fn zk_ams_statement_material_v1(
    statement: &IrohaZkAmsStatementV1,
    binding: &TranscriptBindingV1<'_>,
) -> Result<Vec<u8>, PrivacyReleaseEvidenceErrorClassV1> {
    let mut material =
        norito::encode_canonical(&PrivacyStatementV1::IrohaZkAmsV1(statement.clone()))
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    append_p256_binding_material_v1(&mut material, binding);
    Ok(material)
}

fn run_jindo_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let polynomials = if maximum {
        (0..JINDO_MAX_BATCH_SIZE_V1)
            .map(|polynomial_index| {
                (0..JINDO_MAX_COEFFICIENTS_V1)
                    .map(|coefficient_index| {
                        let value = 1_u64
                            + u64::try_from(polynomial_index)
                                .expect("closed Jindo polynomial index fits u64")
                                * u64::try_from(JINDO_MAX_COEFFICIENTS_V1 + 1)
                                    .expect("closed Jindo coefficient ceiling fits u64")
                            + u64::try_from(coefficient_index)
                                .expect("closed Jindo coefficient index fits u64");
                        jindo_field_v1(value)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    } else {
        vec![vec![
            jindo_field_v1(3),
            jindo_field_v1(5),
            jindo_field_v1(7),
            jindo_field_v1(11),
        ]]
    };
    let witness = JindoPrivacyActionWitnessV1::try_new(polynomials, jindo_field_v1(13))
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let context = JindoPrivacyActionTransactionContextV1 {
        chain_id: ChainId::from("taira-privacy-release-evidence-v1"),
        authority: AccountId::new(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("fixed Jindo evidence authority is canonical"),
        ),
        creation_time: Duration::from_millis(1_800_000_000_123),
        time_to_live: Some(Duration::from_secs(60)),
        nonce: NonZeroU32::new(7),
        fee_payment: FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(5_000_000)),
        metadata: Metadata::default(),
    };
    let mut rng = EvidenceRng06::new(stage_seed_v1(
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
        case_kind,
    ));
    let prepared = prepare_jindo_privacy_action_with_rng_v1(context, witness, [0xa7; 32], &mut rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let observed = prepared
        .release_evidence_payload_v1()
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    if observed.0.as_bytes() != &prepared.transaction_intent_digest() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected);
    }
    let envelope = &observed.1.envelope;
    let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &envelope.statement
    else {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    };
    let PrivacyProofV1::IrohaJindoPolynomialCommitmentV0(proof) = &envelope.proof else {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    };
    let statement = statement.clone();
    let proof_bytes = proof.as_bytes().to_vec();
    if proof_bytes.len()
        != usize::try_from(prepared.proof_bytes())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let profile = crate::privacy_profiles::compiled_privacy_profile_v1(
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let binding = TranscriptBindingV1 {
        chain_id: statement.context.chain_id.as_str().as_bytes(),
        genesis_hash: [0xa7; 32],
        action_index: statement.context.action_index,
        statement_digest: prepared.statement_digest(),
        parameter_id: *profile.parameter_id.as_bytes(),
        parameter_digest: *profile.parameter_digest.as_bytes(),
        verifier_digest: *profile.verifier_digest.as_bytes(),
        statement_schema_digest: *profile.statement_schema_digest.as_bytes(),
        engine_manifest_digest: *profile.engine_manifest_digest.as_bytes(),
        generator_digest: jindo_crs_digest_v1(),
    };
    let proof_ceiling =
        u32::try_from(JINDO_NATIVE_PROOF_BYTES_V1).expect("closed Jindo proof ceiling fits u32");
    verify_batched_evaluation_v1(&statement, &proof_bytes, &binding, proof_ceiling)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let original_typed = PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement.clone());
    let original_material = norito::encode_canonical(&original_typed)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;

    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut mutated = statement.clone();
            let mut mutated_intent = *mutated.context.transaction_intent_digest.as_bytes();
            mutated_intent[0] ^= 0x80;
            mutated.context.transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new(mutated_intent);
            let mutated_typed =
                PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(mutated.clone());
            let mutated_digest = mutated_typed
                .digest()
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let mutated_binding = TranscriptBindingV1 {
                statement_digest: *mutated_digest.as_bytes(),
                ..binding
            };
            if verify_batched_evaluation_v1(&mutated, &proof_bytes, &mutated_binding, proof_ceiling)
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                norito::encode_canonical(&mutated_typed)
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let mut corrupt = proof_bytes.clone();
            let first = corrupt
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_batched_evaluation_v1(&statement, &corrupt, &binding, proof_ceiling).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_interior = proof_bytes.clone();
            let interior_index = corrupt_interior.len() / 2;
            let interior = corrupt_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_batched_evaluation_v1(&statement, &corrupt_interior, &binding, proof_ceiling)
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let truncated = proof_bytes
                .get(..proof_bytes.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_batched_evaluation_v1(&statement, truncated, &binding, proof_ceiling).is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };
    let max_coefficient_count = prepared
        .coefficient_counts()
        .iter()
        .copied()
        .max()
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof_bytes,
            u64::try_from(JINDO_NATIVE_PROOF_BYTES_V1)
                .expect("closed Jindo proof ceiling fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::from(prepared.polynomial_count()),
            primary_ceiling: u64::try_from(JINDO_MAX_BATCH_SIZE_V1)
                .expect("closed Jindo batch ceiling fits u64"),
            secondary_units: u64::from(max_coefficient_count),
            secondary_ceiling: u64::try_from(JINDO_MAX_COEFFICIENTS_V1)
                .expect("closed Jindo coefficient ceiling fits u64"),
            relation_depth: u64::try_from(JINDO_RING_DEGREE_V1)
                .expect("closed Jindo ring degree fits u64"),
            relation_depth_ceiling: u64::try_from(JINDO_RING_DEGREE_V1)
                .expect("closed Jindo ring degree fits u64"),
        },
        failure_class,
    })
}

fn jindo_field_v1(value: u64) -> PrivacyJindoFieldElementV1 {
    let mut encoding = [0_u8; 32];
    encoding[..8].copy_from_slice(&value.to_le_bytes());
    PrivacyJindoFieldElementV1::new(encoding)
}

const ORCHARD_RELEASE_CHAIN_ID_V1: &str = "taira-privacy-release-evidence-orchard-v1";
const ORCHARD_RELEASE_GENESIS_HASH_V1: [u8; 32] = [0x4f; 32];

fn orchard_release_statement_v1(
    context: PrivacyStatementContextV1,
    draft: &OrchardBundleDraftV1,
) -> Result<OrchardHalo2ActionsStatementV1, PrivacyReleaseEvidenceErrorClassV1> {
    let domain_id = DomainId::try_new("privacy", "universal")
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let asset_name = "orchard_release"
        .parse::<Name>()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let value_balance = match draft.value_balance.cmp(&0) {
        core::cmp::Ordering::Equal => PrivacyValueBalanceV1::balanced(),
        core::cmp::Ordering::Less => PrivacyValueBalanceV1 {
            direction: PrivacyValueBalanceDirectionV1::IntoPool,
            amount: u128::from(draft.value_balance.unsigned_abs()),
        },
        core::cmp::Ordering::Greater => PrivacyValueBalanceV1 {
            direction: PrivacyValueBalanceDirectionV1::OutOfPool,
            amount: u128::from(draft.value_balance.unsigned_abs()),
        },
    };
    Ok(OrchardHalo2ActionsStatementV1 {
        context,
        asset_definition_id: AssetDefinitionId::new(domain_id, asset_name),
        pool_id: PrivacyPoolIdV1::new([0x4e; 32]),
        anchor: PrivacyRootV1::new(draft.anchor),
        anchor_epoch: 1,
        actions: draft
            .actions
            .iter()
            .map(|action| PrivacyOrchardActionV1 {
                nullifier: action.nullifier,
                randomized_key: action.randomized_key,
                note_commitment: action.note_commitment,
                ephemeral_key: action.ephemeral_key,
                encrypted_note: action.encrypted_note.to_vec(),
                outgoing_ciphertext: action.outgoing_ciphertext.to_vec(),
                value_commitment: action.value_commitment,
            })
            .collect(),
        value_balance,
        expiry_height: 100,
    })
}

fn orchard_release_transaction_payload_v1(
    envelope: PrivacyProofEnvelopeV1,
) -> Result<TransactionPayload, PrivacyReleaseEvidenceErrorClassV1> {
    let authority = privacy_release_account_v1(0x4d)?;
    let mut builder = TransactionBuilder::new(
        ChainId::from(ORCHARD_RELEASE_CHAIN_ID_V1),
        authority,
        FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(5_000_000)),
    )
    .with_instructions([SubmitPrivacyProofV1::new(envelope)])
    .with_metadata(Metadata::default());
    builder.set_creation_time(Duration::from_millis(1_800_000_000_321));
    builder.set_ttl(Duration::from_secs(60));
    builder.set_nonce(NonZeroU32::new(9).expect("fixture nonce is non-zero"));
    builder
        .into_payload()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
}

fn run_orchard_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let action_count = if maximum { ORCHARD_MAX_ACTIONS_V1 } else { 1 };
    let (anchor, spends, changes, spend_count, expected_value_balance) = if maximum {
        let fixture = orchard_maximum_spend_fixture_v1()?;
        let spend_count = fixture.spends.len();
        let expected_value_balance = i64::try_from(fixture.total_value)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
        (
            fixture.anchor,
            fixture.spends,
            Vec::new(),
            spend_count,
            expected_value_balance,
        )
    } else {
        let seed = 0x31_u8;
        let value = 17_u64;
        (
            orchard_empty_root_v1(),
            Vec::new(),
            vec![OrchardChangeProverInputV1::new(
                orchard_spending_key_v1(seed),
                Scope::External,
                u32::from(seed),
                value,
                [seed; 512],
            )],
            0,
            -i64::try_from(value)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        )
    };
    let mut rng = EvidenceRng09::new(stage_seed_v1(
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
        case_kind,
    ));
    let prepared = prepare_orchard_bundle_v1_with_rng(
        anchor,
        spends,
        changes,
        u8::try_from(action_count)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        &mut rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let draft_context = PrivacyStatementContextV1 {
        chain_id: ChainId::from(ORCHARD_RELEASE_CHAIN_ID_V1),
        action_index: 0,
        transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0; 32]),
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
    };
    let mut statement = orchard_release_statement_v1(draft_context, prepared.public_draft())?;
    let draft_typed_statement = PrivacyStatementV1::OrchardHalo2ActionsV1(statement.clone());
    let draft_envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: PrivacyStatementDigestV1::new([0; 32]),
        statement: draft_typed_statement,
        proof: PrivacyProofV1::OrchardHalo2ActionsV1(PrivacyProofBytesV1::new(Vec::new())),
    };
    let canonical_intent = orchard_release_transaction_payload_v1(draft_envelope)?
        .privacy_transaction_intent_digest_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if canonical_intent.is_zero() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    statement.context.transaction_intent_digest = canonical_intent;
    let consensus_limits = PrivacyConsensusLimitsV1::taira_default();
    let consensus_binding = PrivacyNativeConsensusBindingV1::new(
        &statement.context,
        ORCHARD_RELEASE_GENESIS_HASH_V1,
        &consensus_limits,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let proved =
        authorize_orchard_bundle_v1(prepared, consensus_binding.clone(), &consensus_limits)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let proof_bytes = proved.authorization;
    let public = proved.public;
    verify_orchard_bundle_v1(&public, &proof_bytes, &consensus_limits)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    if public.consensus_binding != consensus_binding
        || public.anchor != *statement.anchor.as_bytes()
        || public.actions.len() != statement.actions.len()
        || public
            .actions
            .iter()
            .zip(&statement.actions)
            .any(|(native, typed)| {
                native.nullifier != typed.nullifier
                    || native.randomized_key != typed.randomized_key
                    || native.note_commitment != typed.note_commitment
                    || native.ephemeral_key != typed.ephemeral_key
                    || native.encrypted_note.as_slice() != typed.encrypted_note.as_slice()
                    || native.outgoing_ciphertext.as_slice() != typed.outgoing_ciphertext.as_slice()
                    || native.value_commitment != typed.value_commitment
            })
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let final_typed_statement = PrivacyStatementV1::OrchardHalo2ActionsV1(statement);
    let final_statement_digest = final_typed_statement
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let final_envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: final_statement_digest,
        statement: final_typed_statement,
        proof: PrivacyProofV1::OrchardHalo2ActionsV1(PrivacyProofBytesV1::new(proof_bytes.clone())),
    };
    let final_payload = orchard_release_transaction_payload_v1(final_envelope.clone())?;
    let validated_intent = final_payload
        .validate_privacy_transaction_intent_binding_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    if validated_intent != canonical_intent
        || public.consensus_binding.transaction_intent_digest != canonical_intent
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    if public.actions.len() != action_count || public.value_balance != expected_value_balance {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    if maximum && (spend_count != ORCHARD_MAX_ACTIONS_V1 || u64::from(ORCHARD_TREE_DEPTH_V1) != 32)
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            orchard_public_material_v1(&public)?,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut mutated = public.clone();
            mutated.consensus_binding.genesis_hash[0] ^= 0x80;
            if verify_orchard_bundle_v1(&mutated, &proof_bytes, &consensus_limits).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            let mut changed_intent_envelope = final_envelope.clone();
            let PrivacyStatementV1::OrchardHalo2ActionsV1(changed_intent_statement) =
                &mut changed_intent_envelope.statement
            else {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            };
            let mut changed_intent = *changed_intent_statement
                .context
                .transaction_intent_digest
                .as_bytes();
            changed_intent[0] ^= 0x40;
            changed_intent_statement.context.transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new(changed_intent);
            changed_intent_envelope.statement_digest =
                changed_intent_envelope
                    .statement
                    .digest()
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if orchard_release_transaction_payload_v1(changed_intent_envelope)?
                .validate_privacy_transaction_intent_binding_v1()
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }

            let mut changed_action_envelope = final_envelope.clone();
            let PrivacyStatementV1::OrchardHalo2ActionsV1(changed_action_statement) =
                &mut changed_action_envelope.statement
            else {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            };
            let changed_ciphertext = changed_action_statement
                .actions
                .first_mut()
                .and_then(|action| action.encrypted_note.first_mut())
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *changed_ciphertext ^= 0x20;
            changed_action_envelope.statement_digest =
                changed_action_envelope
                    .statement
                    .digest()
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if orchard_release_transaction_payload_v1(changed_action_envelope)?
                .validate_privacy_transaction_intent_binding_v1()
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                orchard_public_material_v1(&mutated)?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let mut corrupt = proof_bytes.clone();
            let first = corrupt
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_orchard_bundle_v1(&public, &corrupt, &consensus_limits).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_interior = proof_bytes.clone();
            let interior_index = corrupt_interior.len() / 2;
            let interior = corrupt_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_orchard_bundle_v1(&public, &corrupt_interior, &consensus_limits).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let truncated = proof_bytes
                .get(..proof_bytes.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_orchard_bundle_v1(&public, truncated, &consensus_limits).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                orchard_public_material_v1(&public)?,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };
    let proof_bytes_ceiling = orchard_authorization_wire_size_v1(ORCHARD_MAX_ACTIONS_V1)
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof_bytes,
            u64::try_from(proof_bytes_ceiling).expect("closed Orchard proof ceiling fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(public.actions.len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(ORCHARD_MAX_ACTIONS_V1)
                .expect("closed Orchard action ceiling fits u64"),
            secondary_units: u64::try_from(spend_count)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            secondary_ceiling: u64::try_from(ORCHARD_MAX_ACTIONS_V1)
                .expect("closed Orchard wallet-input ceiling fits u64"),
            relation_depth: u64::from(ORCHARD_TREE_DEPTH_V1),
            relation_depth_ceiling: u64::from(ORCHARD_TREE_DEPTH_V1),
        },
        failure_class,
    })
}

struct OrchardMaximumSpendFixtureV1 {
    anchor: [u8; 32],
    spends: Vec<OrchardSpendProverInputV1>,
    total_value: u64,
}

fn orchard_maximum_spend_fixture_v1()
-> Result<OrchardMaximumSpendFixtureV1, PrivacyReleaseEvidenceErrorClassV1> {
    if usize::from(ORCHARD_TREE_DEPTH_V1) != 32 {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let spending_key = orchard_spending_key_v1(0x41);
    let viewing_key = FullViewingKey::from(&spending_key);
    let recipient = viewing_key.address_at(0_u32, Scope::External);
    let values = [17_u64, 19_u64];
    let notes = values
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| {
            let mut rho_bytes = [0_u8; 32];
            rho_bytes[0] = u8::try_from(index + 1)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let rho = Option::<Rho>::from(Rho::from_bytes(&rho_bytes))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let note_seed = 0x61_u8
                .checked_add(
                    u8::try_from(index)
                        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
                )
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            let random_seed = (1_u8..=u8::MAX)
                .find_map(|counter| {
                    let mut bytes = [note_seed; 32];
                    bytes[0] = counter;
                    Option::<RandomSeed>::from(RandomSeed::from_bytes(bytes, &rho))
                })
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            Option::<Note>::from(Note::from_parts(
                recipient,
                NoteValue::from_raw(value),
                rho,
                random_seed,
                NoteVersion::V2,
            ))
            .ok_or(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if notes.len() != ORCHARD_MAX_ACTIONS_V1 {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let leaves = notes
        .iter()
        .map(|note| {
            let commitment = ExtractedNoteCommitment::from(note.commitment());
            MerkleHashOrchard::from_cmx(&commitment)
        })
        .collect::<Vec<_>>();
    let tree_depth = usize::from(ORCHARD_TREE_DEPTH_V1);
    let mut levels = Vec::with_capacity(tree_depth + 1);
    levels.push(leaves);
    for level_index in 0..tree_depth {
        let level = Level::from(
            u8::try_from(level_index)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        );
        let current = &levels[level_index];
        let mut next = Vec::with_capacity(current.len().div_ceil(2));
        let mut position = 0;
        while position < current.len() {
            let left = current[position];
            let right = current
                .get(position + 1)
                .copied()
                .unwrap_or_else(|| MerkleHashOrchard::empty_root(level));
            next.push(MerkleHashOrchard::combine(level, &left, &right));
            position += 2;
        }
        levels.push(next);
    }

    let mut anchor = None;
    let mut spends = Vec::with_capacity(notes.len());
    for (index, note) in notes.into_iter().enumerate() {
        let auth_path: [MerkleHashOrchard; ORCHARD_TREE_DEPTH_V1 as usize] =
            core::array::from_fn(|level_index| {
                let level = Level::from(
                    u8::try_from(level_index).expect("closed Orchard path level fits u8"),
                );
                let sibling = (index >> level_index) ^ 1;
                levels[level_index]
                    .get(sibling)
                    .copied()
                    .unwrap_or_else(|| MerkleHashOrchard::empty_root(level))
            });
        let merkle_path = MerklePath::from_parts(
            u32::try_from(index)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            auth_path,
        );
        let derived_anchor = merkle_path
            .root(ExtractedNoteCommitment::from(note.commitment()))
            .to_bytes();
        if let Some(expected_anchor) = anchor {
            if expected_anchor != derived_anchor {
                return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
            }
        } else {
            anchor = Some(derived_anchor);
        }
        spends.push(OrchardSpendProverInputV1::new(
            spending_key,
            note,
            merkle_path,
        ));
    }
    let total_value = values.into_iter().try_fold(0_u64, |total, value| {
        total
            .checked_add(value)
            .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)
    })?;
    Ok(OrchardMaximumSpendFixtureV1 {
        anchor: anchor.ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
        spends,
        total_value,
    })
}

fn orchard_spending_key_v1(seed: u8) -> SpendingKey {
    (1_u8..=u8::MAX)
        .find_map(|counter| {
            let mut bytes = [seed; 32];
            bytes[0] = counter;
            bytes[15] = counter.rotate_left(3);
            bytes[31] = seed ^ counter.rotate_right(1);
            Option::<SpendingKey>::from(SpendingKey::from_bytes(bytes))
        })
        .expect("closed evidence seed admits an Orchard spending key")
}

fn orchard_public_material_v1(
    public: &OrchardBundlePublicV1,
) -> Result<Vec<u8>, PrivacyReleaseEvidenceErrorClassV1> {
    let mut material = Vec::with_capacity(2_048);
    material.extend_from_slice(b"iroha.privacy.release.orchard.public-statement.v1");
    let binding_digest = public
        .consensus_binding
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    material.extend_from_slice(binding_digest.as_bytes());
    material.extend_from_slice(&public.anchor);
    material.extend_from_slice(&public.value_balance.to_be_bytes());
    material.push(
        u8::try_from(public.actions.len())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
    );
    for action in &public.actions {
        material.extend_from_slice(&action.nullifier);
        material.extend_from_slice(&action.randomized_key);
        material.extend_from_slice(&action.note_commitment);
        material.extend_from_slice(&action.ephemeral_key);
        material.extend_from_slice(&action.encrypted_note);
        material.extend_from_slice(&action.outgoing_ciphertext);
        material.extend_from_slice(&action.value_commitment);
    }
    Ok(material)
}

fn run_verange_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let profile = if maximum {
        VeRangeBitLengthV1::Bits64
    } else {
        VeRangeBitLengthV1::Bits32
    };
    let values: Vec<u64> = if maximum {
        vec![0, 1, 2, 3, 4, 5, u32::MAX.into(), u64::MAX]
    } else {
        vec![42]
    };
    let scalar_values: &[u8] = if maximum {
        &[3, 5, 7, 11, 13, 17, 19, 23]
    } else {
        &[7]
    };
    let blindings: Vec<SecretScalarV1> = scalar_values
        .iter()
        .map(|value| {
            let mut bytes = [0_u8; 32];
            bytes[31] = *value;
            SecretScalarV1::from_bytes(bytes)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        })
        .collect::<Result<_, _>>()?;
    let commitments = values
        .iter()
        .zip(&blindings)
        .map(|(value, blinding)| {
            commit(profile, *value, blinding)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let binding = verange_binding_v1(profile, [0x22; 32])?;
    let statement = VeRangeType1BatchStatementV1::new(profile, commitments.clone(), binding)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let mut rng = EvidenceRng06::new(stage_seed_v1(
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        case_kind,
    ));
    let proof = prove_batch(&statement, &values, &blindings, &mut rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let proof_bytes = proof.encode();
    verify_batch_encoded(&statement, &proof_bytes)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;

    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            verange_statement_material_v1(profile, &commitments, &binding),
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut mutated_digest = binding.statement_digest;
            mutated_digest[0] ^= 0x80;
            let mutated_binding = verange_binding_v1(profile, mutated_digest)?;
            let mutated =
                VeRangeType1BatchStatementV1::new(profile, commitments.clone(), mutated_binding)
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            if verify_batch_encoded(&mutated, &proof_bytes).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                verange_statement_material_v1(profile, &commitments, &mutated_binding),
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let mut corrupt = proof_bytes.clone();
            let first = corrupt
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_batch_encoded(&statement, &corrupt).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_interior = proof_bytes.clone();
            let interior_index = corrupt_interior.len() / 2;
            let interior = corrupt_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_batch_encoded(&statement, &corrupt_interior).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let truncated = proof_bytes
                .get(..proof_bytes.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_batch_encoded(&statement, truncated).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                verange_statement_material_v1(profile, &commitments, &binding),
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };

    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof_bytes,
            u64::try_from(MAX_VERANGE_TYPE1_BATCH_PROOF_BYTES_V1)
                .expect("fixed VeRange proof ceiling fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(values.len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(MAX_VERANGE_TYPE1_BATCH_COMMITMENTS_V1)
                .expect("fixed VeRange batch ceiling fits u64"),
            secondary_units: u64::from(profile.bits()),
            secondary_ceiling: 64,
            relation_depth: u64::try_from(profile.rows())
                .expect("fixed VeRange matrix row count fits u64"),
            relation_depth_ceiling: 8,
        },
        failure_class,
    })
}

fn verange_binding_v1(
    profile: VeRangeBitLengthV1,
    statement_digest: [u8; 32],
) -> Result<TranscriptBindingV1<'static>, PrivacyReleaseEvidenceErrorClassV1> {
    let parameters = VeRangeParametersV1::for_profile(profile)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(TranscriptBindingV1 {
        chain_id: b"taira-privacy-release-evidence-v1",
        genesis_hash: [0x11; 32],
        action_index: 3,
        statement_digest,
        parameter_id: [0x23; 32],
        parameter_digest: parameters.parameter_digest(),
        verifier_digest: [0x24; 32],
        statement_schema_digest: [0x25; 32],
        engine_manifest_digest: [0x26; 32],
        generator_digest: parameters.generator_digest(),
    })
}

fn verange_statement_material_v1(
    profile: VeRangeBitLengthV1,
    commitments: &[crate::privacy_engines::p256::CompressedPointV1],
    binding: &TranscriptBindingV1<'_>,
) -> Vec<u8> {
    let mut material = Vec::with_capacity(384);
    material.extend_from_slice(b"iroha.privacy.release.verange.public-statement.v1");
    material.extend_from_slice(&profile.bits().to_be_bytes());
    material.extend_from_slice(
        &u32::try_from(commitments.len())
            .expect("VeRange commitment ceiling fits u32")
            .to_be_bytes(),
    );
    for commitment in commitments {
        material.extend_from_slice(commitment.as_bytes());
    }
    append_p256_binding_material_v1(&mut material, binding);
    material
}

fn redigest_ivm_release_statement_v1(
    statement: &mut iroha_data_model::privacy::IrohaIvmPrivateNoteStarkStatementV1,
) -> Result<(), PrivacyReleaseEvidenceErrorClassV1> {
    statement.action_digest = iroha_data_model::privacy::PrivacyActionDigestV1::new([0; 32]);
    statement.action_digest = statement
        .computed_action_digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(())
}

fn native_bound_statement_material_v1(
    domain: &[u8],
    statement: &PrivacyStatementV1,
    consensus_binding: &PrivacyNativeConsensusBindingV1,
) -> Result<Vec<u8>, PrivacyReleaseEvidenceErrorClassV1> {
    let statement_bytes = norito::encode_canonical(statement)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let statement_len = u64::try_from(statement_bytes.len())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let binding_digest = consensus_binding
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let mut material = Vec::new();
    material.extend_from_slice(domain);
    material.extend_from_slice(&statement_len.to_be_bytes());
    material.extend_from_slice(&statement_bytes);
    material.extend_from_slice(binding_digest.as_bytes());
    Ok(material)
}

fn run_ivm_private_note_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    const IVM_PRIVATE_NOTE_RELEASE_GENESIS_HASH_V1: [u8; 32] = [0x49; 32];
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let protocol_id = PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
    let fixture_seed =
        stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-encryption")?;
    let mut fixture_rng = EvidenceRng06::new(fixture_seed);
    let fixture = ivm_private_note_release_fixture_v1(maximum, &mut fixture_rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let statement = fixture.statement;
    let witness = fixture.witness;
    let expected_units = if maximum { 2 } else { 1 };
    if witness.inputs().len() != expected_units
        || witness.outputs().len() != expected_units
        || statement.nullifiers.len() != expected_units
        || statement.output_commitments.len() != expected_units
        || statement.encrypted_outputs.len() != expected_units
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let proof_seed = stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-proof")?;
    let mut proof_rng = EvidenceRng09::new(proof_seed);
    let consensus_limits = PrivacyConsensusLimitsV1::taira_default();
    let consensus_binding = PrivacyNativeConsensusBindingV1::new(
        &statement.context,
        IVM_PRIVATE_NOTE_RELEASE_GENESIS_HASH_V1,
        &consensus_limits,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let proof = prove_ivm_private_note_v1_with_rng(
        &statement,
        &consensus_binding,
        &consensus_limits,
        &witness,
        &mut proof_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    verify_ivm_private_note_v1(&statement, &consensus_binding, &consensus_limits, &proof)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let original_typed = PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement.clone());
    let original_material = native_bound_statement_material_v1(
        b"iroha.privacy.release.ivm-private-note.bound-statement.v1",
        &original_typed,
        &consensus_binding,
    )?;

    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut cross_context = statement.clone();
            cross_context.context.chain_id =
                ChainId::from("taira-private-note-release-cross-context-v1");
            redigest_ivm_release_statement_v1(&mut cross_context)?;

            let mut cross_intent = statement.clone();
            let mut intent = *cross_intent.context.transaction_intent_digest.as_bytes();
            intent[0] ^= 0x80;
            cross_intent.context.transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new(intent);
            redigest_ivm_release_statement_v1(&mut cross_intent)?;

            let mut cross_root = statement.clone();
            let mut root = *cross_root.state_root.as_bytes();
            root[0] ^= 0x80;
            cross_root.state_root = PrivacyRootV1::new(root);
            redigest_ivm_release_statement_v1(&mut cross_root)?;

            let mut cross_epoch = statement.clone();
            cross_epoch.root_epoch = cross_epoch
                .root_epoch
                .checked_add(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            cross_epoch.execution_epoch = cross_epoch.root_epoch;
            redigest_ivm_release_statement_v1(&mut cross_epoch)?;

            for mutation in [&cross_context, &cross_intent, &cross_root, &cross_epoch] {
                if verify_ivm_private_note_v1(
                    mutation,
                    &consensus_binding,
                    &consensus_limits,
                    &proof,
                )
                .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }
            let mut changed_genesis_binding = consensus_binding.clone();
            changed_genesis_binding.genesis_hash[0] ^= 0x80;
            if verify_ivm_private_note_v1(
                &statement,
                &changed_genesis_binding,
                &consensus_limits,
                &proof,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                native_bound_statement_material_v1(
                    b"iroha.privacy.release.ivm-private-note.bound-statement.v1",
                    &original_typed,
                    &changed_genesis_binding,
                )?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let invalid_fixture_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-fixture-encryption")?;
            let mut invalid_fixture_rng = EvidenceRng06::new(invalid_fixture_seed);
            let invalid =
                ivm_private_note_release_invalid_path_fixture_v1(&mut invalid_fixture_rng)
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let invalid_proof_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-proof")?;
            let mut invalid_proof_rng = EvidenceRng09::new(invalid_proof_seed);
            let invalid_consensus_binding = PrivacyNativeConsensusBindingV1::new(
                &invalid.statement.context,
                IVM_PRIVATE_NOTE_RELEASE_GENESIS_HASH_V1,
                &consensus_limits,
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            if prove_ivm_private_note_v1_with_rng(
                &invalid.statement,
                &invalid_consensus_binding,
                &consensus_limits,
                &invalid.witness,
                &mut invalid_proof_rng,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::InvalidWitnessPathAccepted);
            }

            let mut corrupt_header = proof.clone();
            let first = corrupt_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_ivm_private_note_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_header,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut corrupt_interior = proof.clone();
            let interior = corrupt_interior.len() / 2;
            let byte = corrupt_interior
                .get_mut(interior)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *byte ^= 0x01;
            if verify_ivm_private_note_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_interior,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let truncated_length = proof
                .len()
                .checked_sub(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_ivm_private_note_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &proof[..truncated_length],
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };

    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof,
            u64::try_from(IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1)
                .expect("closed private-note proof ceiling fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(witness.inputs().len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(PRIVATE_NOTE_MAX_INPUTS_V1)
                .expect("closed private-note input ceiling fits u64"),
            secondary_units: u64::try_from(witness.outputs().len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            secondary_ceiling: u64::try_from(PRIVATE_NOTE_MAX_OUTPUTS_V1)
                .expect("closed private-note output ceiling fits u64"),
            relation_depth: u64::try_from(PRIVATE_NOTE_TREE_DEPTH_V1)
                .expect("closed private-note tree depth fits u64"),
            relation_depth_ceiling: u64::try_from(PRIVATE_NOTE_TREE_DEPTH_V1)
                .expect("closed private-note tree depth fits u64"),
        },
        failure_class,
    })
}

fn run_pq_masp_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    const PQ_MASP_RELEASE_GENESIS_HASH_V1: [u8; 32] = [0x50; 32];
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let protocol_id = PrivacyProtocolIdV1::PqMaspStarkV0;
    let keygen_seed = stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-keygen")?;
    let fixture_seed =
        stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-encryption")?;
    let mut fixture_rng = EvidenceRng09::new(fixture_seed);
    let fixture = pq_masp_release_fixture_v1(maximum, keygen_seed, &mut fixture_rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let statement = fixture.statement;
    let witness = fixture.witness;
    let authorization_secret_key = fixture.authorization_secret_key;
    let expected_units = if maximum { 2 } else { 1 };
    if witness.inputs().len() != expected_units
        || witness.outputs().len() != expected_units
        || statement.nullifiers.len() != expected_units
        || statement.output_commitments.len() != expected_units
        || statement.encrypted_outputs.len() != expected_units
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let proof_seed = stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-proof")?;
    let mut proof_rng = EvidenceRng09::new(proof_seed);
    let consensus_limits = PrivacyConsensusLimitsV1::taira_default();
    let consensus_binding = PrivacyNativeConsensusBindingV1::new(
        &statement.context,
        PQ_MASP_RELEASE_GENESIS_HASH_V1,
        &consensus_limits,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let proof = prove_pq_masp_v1_with_rng(
        &statement,
        &consensus_binding,
        &consensus_limits,
        &witness,
        authorization_secret_key.as_slice(),
        &mut proof_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    verify_pq_masp_v1(&statement, &consensus_binding, &consensus_limits, &proof)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let original_typed = PrivacyStatementV1::PqMaspStarkV0(statement.clone());
    let original_material = native_bound_statement_material_v1(
        b"iroha.privacy.release.pq-masp.bound-statement.v1",
        &original_typed,
        &consensus_binding,
    )?;

    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut cross_context = statement.clone();
            cross_context.context.chain_id =
                ChainId::from("taira-pq-masp-release-cross-context-v1");

            let mut cross_intent = statement.clone();
            let mut intent = *cross_intent.context.transaction_intent_digest.as_bytes();
            intent[0] ^= 0x80;
            cross_intent.context.transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new(intent);

            let mut cross_anchor = statement.clone();
            let mut anchor = *cross_anchor.anchor.as_bytes();
            anchor[0] ^= 0x80;
            cross_anchor.anchor = PrivacyRootV1::new(anchor);

            let mut cross_epoch = statement.clone();
            cross_epoch.anchor_epoch = cross_epoch
                .anchor_epoch
                .checked_add(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            cross_epoch.authorization_epoch = cross_epoch.anchor_epoch;

            let mut cross_key = statement.clone();
            let mut key_digest = *cross_key.authorization_key_digest.as_bytes();
            key_digest[0] ^= 0x80;
            cross_key.authorization_key_digest =
                iroha_data_model::privacy::PrivacyAuthorizationKeyDigestV1::new(key_digest);

            for mutation in [
                &cross_context,
                &cross_intent,
                &cross_anchor,
                &cross_epoch,
                &cross_key,
            ] {
                if verify_pq_masp_v1(mutation, &consensus_binding, &consensus_limits, &proof)
                    .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }
            let mut changed_genesis_binding = consensus_binding.clone();
            changed_genesis_binding.genesis_hash[0] ^= 0x80;
            if verify_pq_masp_v1(
                &statement,
                &changed_genesis_binding,
                &consensus_limits,
                &proof,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                native_bound_statement_material_v1(
                    b"iroha.privacy.release.pq-masp.bound-statement.v1",
                    &original_typed,
                    &changed_genesis_binding,
                )?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let invalid_keygen_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-fixture-keygen")?;
            let invalid_fixture_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-fixture-encryption")?;
            let mut invalid_fixture_rng = EvidenceRng09::new(invalid_fixture_seed);
            let invalid = pq_masp_release_invalid_path_fixture_v1(
                invalid_keygen_seed,
                &mut invalid_fixture_rng,
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let invalid_proof_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-proof")?;
            let mut invalid_proof_rng = EvidenceRng09::new(invalid_proof_seed);
            let invalid_consensus_binding = PrivacyNativeConsensusBindingV1::new(
                &invalid.statement.context,
                PQ_MASP_RELEASE_GENESIS_HASH_V1,
                &consensus_limits,
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            if prove_pq_masp_v1_with_rng(
                &invalid.statement,
                &invalid_consensus_binding,
                &consensus_limits,
                &invalid.witness,
                invalid.authorization_secret_key.as_slice(),
                &mut invalid_proof_rng,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::InvalidWitnessPathAccepted);
            }

            let mut corrupt_header = proof.clone();
            let first = corrupt_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_pq_masp_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_header,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut corrupt_inner_header = proof.clone();
            let inner_header = corrupt_inner_header
                .get_mut(PQ_MASP_AUTHORIZATION_HEADER_BYTES_V1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *inner_header ^= 0x80;
            if verify_pq_masp_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_inner_header,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut corrupt_interior = proof.clone();
            let interior = corrupt_interior.len() / 2;
            let byte = corrupt_interior
                .get_mut(interior)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *byte ^= 0x01;
            if verify_pq_masp_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_interior,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let truncated_length = proof
                .len()
                .checked_sub(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_pq_masp_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &proof[..truncated_length],
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };

    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof,
            u64::try_from(PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1)
                .expect("closed PQ-MASP proof ceiling fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(witness.inputs().len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(PQ_MASP_INPUT_BOUND_V1)
                .expect("closed PQ-MASP input ceiling fits u64"),
            secondary_units: u64::try_from(witness.outputs().len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            secondary_ceiling: u64::try_from(PQ_MASP_OUTPUT_BOUND_V1)
                .expect("closed PQ-MASP output ceiling fits u64"),
            relation_depth: u64::try_from(PQ_MASP_TREE_DEPTH_V1)
                .expect("closed PQ-MASP tree depth fits u64"),
            relation_depth_ceiling: u64::try_from(PQ_MASP_TREE_DEPTH_V1)
                .expect("closed PQ-MASP tree depth fits u64"),
        },
        failure_class,
    })
}

const VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1: u64 = 1_785_024_000_000;
const VEGA_RELEASE_CHAIN_ID_V1: &str = "taira-privacy-release-evidence-vega-v1";
const VEGA_RELEASE_GENESIS_HASH_V1: [u8; 32] = [0xa7; 32];
const VEGA_RELEASE_ACTION_INDEX_V1: u32 = VEGA_PRIVACY_ACTION_INDEX_V1;
const VEGA_RELEASE_CREATION_TIME_MS_V1: u64 = VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1 - 1;
const VEGA_RELEASE_NONCE_V1: u32 = 26;
const VEGA_RELEASE_VARIABLE_COUNT_V1: u64 = 524_288;
const VEGA_RELEASE_CONSTRAINT_COUNT_V1: u64 = 1_048_576;
const VEGA_RELEASE_COMBINED_SUMCHECK_ROUNDS_V1: u64 = 40;
const VEGA_RELEASE_PUBLIC_INPUT_COUNT_V1: usize = 14;

struct VegaReleaseFixtureV1 {
    public_input: VegaPrivacyActionPublicInputV1,
    issuer_record: PrivacyVegaIssuerRecordV1,
    issuer_authentication_sig_structure: Vec<u8>,
    mobile_security_object_payload: Vec<u8>,
    birth_date_issuer_signed_item: Vec<u8>,
    issuer_signature: P256Signature,
    issuer_high_s_signature: P256Signature,
    device_signing_key: P256SigningKey,
    genesis_hash: [u8; 32],
}

fn vega_release_transaction_context_v1()
-> Result<VegaPrivacyActionTransactionContextV1, PrivacyReleaseEvidenceErrorClassV1> {
    Ok(VegaPrivacyActionTransactionContextV1 {
        chain_id: ChainId::from(VEGA_RELEASE_CHAIN_ID_V1),
        authority: privacy_release_account_v1(0x56)?,
        creation_time: Duration::from_millis(VEGA_RELEASE_CREATION_TIME_MS_V1),
        time_to_live: Some(Duration::from_secs(60)),
        nonce: NonZeroU32::new(VEGA_RELEASE_NONCE_V1),
        fee_payment: FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(5_000_000)),
        metadata: Metadata::default(),
    })
}

fn run_vega_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let fixture = vega_release_fixture_v1()?;
    let VegaReleaseFixtureV1 {
        public_input,
        issuer_record,
        issuer_authentication_sig_structure,
        mobile_security_object_payload,
        birth_date_issuer_signed_item,
        issuer_signature,
        issuer_high_s_signature,
        device_signing_key,
        genesis_hash,
    } = fixture;
    let witness_material = VegaPrivacyActionWitnessMaterialV1::new(
        issuer_authentication_sig_structure.clone(),
        mobile_security_object_payload.clone(),
        birth_date_issuer_signed_item.clone(),
        &issuer_signature.to_bytes(),
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let proof_seed = stage_purpose_seed_v1(
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
        case_kind,
        b"figure9-proof-randomness",
    )?;
    let mut proof_rng = EvidenceRng06::new(proof_seed);
    let prepared = prepare_vega_privacy_action_with_rng_v1(
        vega_release_transaction_context_v1()?,
        public_input,
        witness_material,
        &device_signing_key,
        genesis_hash,
        VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
        &mut proof_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let (statement, proof) = {
        let (intent, submission) = prepared
            .release_evidence_payload_v1()
            .privacy_transaction_intent_binding_if_present_v1()
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
            .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
        if intent.as_bytes() != &prepared.transaction_intent_digest()
            || submission.envelope.statement_digest.as_bytes() != &prepared.statement_digest()
        {
            return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
        }
        let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) =
            &submission.envelope.statement
        else {
            return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
        };
        let PrivacyProofV1::VegaExistingCredentialZkV0(proof) = &submission.envelope.proof else {
            return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
        };
        if proof.as_bytes().len()
            != usize::try_from(prepared.proof_bytes())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?
        {
            return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
        }
        (statement.clone(), proof.as_bytes().to_vec())
    };
    validate_vega_authoritative_issuer_binding_v1(&statement, &issuer_record)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let binding = VegaMdlConsensusBindingV1::from_context(&statement.context, genesis_hash);

    let device_signature: P256Signature = device_signing_key
        .sign_prehash(statement.device_authentication_digest.as_bytes())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let device_signature = device_signature.normalize_s().unwrap_or(device_signature);
    let (device_r, device_s) = device_signature.split_scalars();
    let device_high_s_signature =
        P256Signature::from_scalars(device_r.to_repr(), (-*device_s).to_repr())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if issuer_high_s_signature.normalize_s().is_none()
        || device_high_s_signature.normalize_s().is_none()
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let noncanonical_witnesses: [(&[u8], VegaMdlWitnessV1); 2] = [
        (
            b"figure9-issuer-high-s-rejection",
            VegaMdlWitnessV1::new(
                issuer_authentication_sig_structure.clone(),
                mobile_security_object_payload.clone(),
                birth_date_issuer_signed_item.clone(),
                &issuer_high_s_signature.to_bytes(),
                &device_signature.to_bytes(),
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?,
        ),
        (
            b"figure9-device-high-s-rejection",
            VegaMdlWitnessV1::new(
                issuer_authentication_sig_structure,
                mobile_security_object_payload,
                birth_date_issuer_signed_item,
                &issuer_signature.to_bytes(),
                &device_high_s_signature.to_bytes(),
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?,
        ),
    ];
    for (purpose, noncanonical_witness) in noncanonical_witnesses {
        let mut noncanonical_rng = EvidenceRng06::new(stage_purpose_seed_v1(
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            case_kind,
            purpose,
        )?);
        let noncanonical_config = VegaMdlProverConfigV1::new(1)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
        if prove_mdl_figure9_v1(
            &statement,
            &binding,
            VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
            noncanonical_witness,
            noncanonical_config,
            &mut noncanonical_rng,
        )
        .is_ok()
        {
            return Err(PrivacyReleaseEvidenceErrorClassV1::NonCanonicalWitnessAccepted);
        }
    }
    verify_mdl_figure9_v1(
        &statement,
        &binding,
        VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
        &proof,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let authoritative_chain_id = ChainId::from(VEGA_RELEASE_CHAIN_ID_V1);
    let authoritative_action_index = VEGA_RELEASE_ACTION_INDEX_V1;
    if statement.context.chain_id != authoritative_chain_id
        || statement.context.action_index != authoritative_action_index
        || genesis_hash != VEGA_RELEASE_GENESIS_HASH_V1
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    verify_vega_release_production_envelope_v1(
        &statement,
        Some(&issuer_record),
        &proof,
        &authoritative_chain_id,
        genesis_hash,
        authoritative_action_index,
        VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
    )?;

    let dimensions = vega_mdl_proof_dimensions_v1()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let variable_count = u64::try_from(dimensions.variable_count)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let constraint_count = u64::try_from(dimensions.constraint_count)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let combined_sumcheck_rounds = dimensions
        .outer_sumcheck_rounds
        .checked_add(dimensions.inner_sumcheck_rounds)
        .and_then(|rounds| u64::try_from(rounds).ok())
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    if VEGA_MDL_PUBLIC_INPUT_COUNT_V1 != VEGA_RELEASE_PUBLIC_INPUT_COUNT_V1
        || variable_count != VEGA_RELEASE_VARIABLE_COUNT_V1
        || constraint_count != VEGA_RELEASE_CONSTRAINT_COUNT_V1
        || combined_sumcheck_rounds != VEGA_RELEASE_COMBINED_SUMCHECK_ROUNDS_V1
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }

    let original_material = norito::encode_canonical(
        &PrivacyStatementV1::VegaExistingCredentialZkV0(statement.clone()),
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut stale_epoch = statement.clone();
            stale_epoch.issuer_record_epoch = stale_epoch
                .issuer_record_epoch
                .checked_add(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            refresh_vega_device_authentication_digest_v1(&mut stale_epoch, genesis_hash)?;

            let mut wrong_issuer = statement.clone();
            let mut issuer_id = *wrong_issuer.issuer_id.as_bytes();
            issuer_id[0] ^= 0x80;
            wrong_issuer.issuer_id = PrivacyIssuerIdV1::new(issuer_id);
            refresh_vega_device_authentication_digest_v1(&mut wrong_issuer, genesis_hash)?;

            let mut wrong_record_digest = statement.clone();
            let mut record_digest = *wrong_record_digest.issuer_record_digest.as_bytes();
            record_digest[0] ^= 0x80;
            wrong_record_digest.issuer_record_digest =
                iroha_data_model::privacy::PrivacyVegaIssuerRecordDigestV1::new(record_digest);
            refresh_vega_device_authentication_digest_v1(&mut wrong_record_digest, genesis_hash)?;

            let mut wrong_issuer_key = statement.clone();
            let substitute_signing_key = P256SigningKey::from_bytes((&[3_u8; 32]).into())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            wrong_issuer_key.issuer_public_key =
                vega_compressed_public_key_v1(&substitute_signing_key)?;
            refresh_vega_device_authentication_digest_v1(&mut wrong_issuer_key, genesis_hash)?;

            let mut wrong_chain = statement.clone();
            wrong_chain.context.chain_id =
                ChainId::from("taira-privacy-release-evidence-vega-wrong-chain");
            refresh_vega_device_authentication_digest_v1(&mut wrong_chain, genesis_hash)?;

            let mut wrong_action_index = statement.clone();
            wrong_action_index.context.action_index = wrong_action_index
                .context
                .action_index
                .checked_add(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            refresh_vega_device_authentication_digest_v1(&mut wrong_action_index, genesis_hash)?;

            for issuer_mutation in [
                &stale_epoch,
                &wrong_issuer,
                &wrong_record_digest,
                &wrong_issuer_key,
            ] {
                if validate_vega_authoritative_issuer_binding_v1(issuer_mutation, &issuer_record)
                    .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }

            for mutation in [
                &stale_epoch,
                &wrong_issuer,
                &wrong_record_digest,
                &wrong_issuer_key,
                &wrong_chain,
                &wrong_action_index,
            ] {
                let mutated_binding =
                    VegaMdlConsensusBindingV1::from_context(&mutation.context, genesis_hash);
                if verify_mdl_figure9_v1(
                    mutation,
                    &mutated_binding,
                    VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                    &proof,
                )
                .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
                if verify_vega_release_production_envelope_v1(
                    mutation,
                    Some(&issuer_record),
                    &proof,
                    &authoritative_chain_id,
                    genesis_hash,
                    authoritative_action_index,
                    VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                )
                .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }

            let revoked_record = PrivacyVegaIssuerRecordV1::new(
                issuer_record.issuer_id,
                issuer_record.record_epoch,
                issuer_record.issuer_public_key,
                issuer_record.document_type,
                issuer_record.namespace,
                issuer_record.digest_algorithm,
                issuer_record.issuer_authentication_algorithm,
                issuer_record.device_authentication_algorithm,
                issuer_record.previous_record_digest,
                PrivacyVegaIssuerRecordLifecycleV1::Revoked,
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            for issuer_state in [None, Some(&revoked_record)] {
                if verify_vega_release_production_envelope_v1(
                    &statement,
                    issuer_state,
                    &proof,
                    &authoritative_chain_id,
                    genesis_hash,
                    authoritative_action_index,
                    VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                )
                .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }

            let mut wrong_genesis_hash = genesis_hash;
            wrong_genesis_hash[0] ^= 0x80;
            for (wrong_genesis, wrong_timestamp) in [
                (wrong_genesis_hash, VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1),
                (genesis_hash, 0),
            ] {
                if verify_vega_release_production_envelope_v1(
                    &statement,
                    Some(&issuer_record),
                    &proof,
                    &authoritative_chain_id,
                    wrong_genesis,
                    authoritative_action_index,
                    wrong_timestamp,
                )
                .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }
            (
                norito::encode_canonical(&PrivacyStatementV1::VegaExistingCredentialZkV0(
                    stale_epoch,
                ))
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let mut corrupt_header = proof.clone();
            let first = corrupt_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_mdl_figure9_v1(
                &statement,
                &binding,
                VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                &corrupt_header,
            )
            .is_ok()
                || verify_vega_release_production_envelope_v1(
                    &statement,
                    Some(&issuer_record),
                    &corrupt_header,
                    &authoritative_chain_id,
                    genesis_hash,
                    authoritative_action_index,
                    VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let mut corrupt_interior = proof.clone();
            let interior_index = corrupt_interior.len() / 2;
            let interior = corrupt_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_mdl_figure9_v1(
                &statement,
                &binding,
                VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                &corrupt_interior,
            )
            .is_ok()
                || verify_vega_release_production_envelope_v1(
                    &statement,
                    Some(&issuer_record),
                    &corrupt_interior,
                    &authoritative_chain_id,
                    genesis_hash,
                    authoritative_action_index,
                    VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }

            let truncated_length = proof
                .len()
                .checked_sub(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_mdl_figure9_v1(
                &statement,
                &binding,
                VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                &proof[..truncated_length],
            )
            .is_ok()
                || verify_vega_release_production_envelope_v1(
                    &statement,
                    Some(&issuer_record),
                    &proof[..truncated_length],
                    &authoritative_chain_id,
                    genesis_hash,
                    authoritative_action_index,
                    VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                )
                .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };

    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof,
            u64::try_from(MAX_VEGA_PROOF_BYTES_V1).expect("closed Vega proof ceiling fits u64"),
        ),
        failure_class,
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: constraint_count,
            primary_ceiling: VEGA_RELEASE_CONSTRAINT_COUNT_V1,
            secondary_units: variable_count,
            secondary_ceiling: VEGA_RELEASE_VARIABLE_COUNT_V1,
            relation_depth: combined_sumcheck_rounds,
            relation_depth_ceiling: VEGA_RELEASE_COMBINED_SUMCHECK_ROUNDS_V1,
        },
    })
}

fn verify_vega_release_production_envelope_v1(
    statement: &VegaExistingCredentialStatementV1,
    issuer_record: Option<&PrivacyVegaIssuerRecordV1>,
    proof: &[u8],
    authoritative_chain_id: &ChainId,
    genesis_hash: [u8; 32],
    authoritative_action_index: u32,
    block_timestamp_ms: u64,
) -> Result<(), PrivacyReleaseEvidenceErrorClassV1> {
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let activation = profile.activation_record(PrivacyProtocolLifecycleV1::Active(
        PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 2,
        },
    ));
    let typed_statement = PrivacyStatementV1::VegaExistingCredentialZkV0(statement.clone());
    let statement_digest = typed_statement
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest,
        statement: typed_statement,
        proof: PrivacyProofV1::VegaExistingCredentialZkV0(PrivacyProofBytesV1::new(proof.to_vec())),
    };
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let effects = verify_privacy_envelope_v1(
        &envelope,
        PrivacyVerificationContextV1 {
            activation: &activation,
            consensus_limits: &limits,
            chain_id: authoritative_chain_id,
            genesis_hash,
            current_height: 2,
            expected_action_index: authoritative_action_index,
            block_timestamp_ms,
            pgc_state: None,
            orchard_state: None,
            proof_managed_state: None,
            zk_x509_state: None,
            bootle_lantern_policy: None,
            vega_issuer_record: issuer_record,
        },
    )
    .map_err(|source| match source {
        PrivacyVerificationErrorV1::NativeVega(_) => {
            PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected
        }
        _ => PrivacyReleaseEvidenceErrorClassV1::ProductionEnvelopeRejected,
    })?;
    if effects.protocol_id() != PrivacyProtocolIdV1::VegaExistingCredentialZkV0
        || effects.statement_digest() != statement_digest
        || effects.action_index() != authoritative_action_index
        || effects.encoded_action_bytes() == 0
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(())
}

fn refresh_vega_device_authentication_digest_v1(
    statement: &mut VegaExistingCredentialStatementV1,
    genesis_hash: [u8; 32],
) -> Result<(), PrivacyReleaseEvidenceErrorClassV1> {
    let binding = VegaMdlConsensusBindingV1::from_context(&statement.context, genesis_hash);
    statement.device_authentication_digest =
        derive_device_authentication_digest_v1(statement, &binding)
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(())
}

fn vega_release_fixture_v1() -> Result<VegaReleaseFixtureV1, PrivacyReleaseEvidenceErrorClassV1> {
    let issuer_signing_key = P256SigningKey::from_bytes((&[1_u8; 32]).into())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let device_signing_key = P256SigningKey::from_bytes((&[2_u8; 32]).into())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let issuer_public_key = vega_compressed_public_key_v1(&issuer_signing_key)?;
    let issuer_record = PrivacyVegaIssuerRecordV1::new(
        PrivacyIssuerIdV1::new([0x40; 32]),
        1,
        issuer_public_key,
        PrivacyCredentialDocumentTypeV1::Iso18013_5Mdl,
        PrivacyVegaMdlNamespaceV1::OrgIso18013_5_1,
        PrivacyVegaMdlDigestAlgorithmV1::Sha256,
        PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
        PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
        None,
        PrivacyVegaIssuerRecordLifecycleV1::Active,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;

    let device_uncompressed = device_signing_key.verifying_key().to_encoded_point(false);
    let device_x = device_uncompressed
        .x()
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let device_y = device_uncompressed
        .y()
        .ok_or(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let birth_inner = vega_cbor_map_v1(vec![
        (vega_cbor_text_v1("digestID"), vega_cbor_unsigned_v1(1)),
        (vega_cbor_text_v1("random"), vega_cbor_bytes_v1(&[0x42; 16])),
        (
            vega_cbor_text_v1("elementIdentifier"),
            vega_cbor_text_v1("birth_date"),
        ),
        (
            vega_cbor_text_v1("elementValue"),
            vega_cbor_text_v1("1980-06-15"),
        ),
    ]);
    let birth_item = vega_cbor_tag_v1(24, vega_cbor_bytes_v1(&birth_inner));
    let birth_digest: [u8; 32] = Sha256::digest(&birth_item).into();
    let device_key = vega_cbor_map_v1(vec![
        (vega_cbor_unsigned_v1(1), vega_cbor_unsigned_v1(2)),
        (vega_cbor_negative_v1(-1), vega_cbor_unsigned_v1(1)),
        (vega_cbor_negative_v1(-2), vega_cbor_bytes_v1(device_x)),
        (vega_cbor_negative_v1(-3), vega_cbor_bytes_v1(device_y)),
    ]);
    let validity_info = vega_cbor_map_v1(vec![
        (
            vega_cbor_text_v1("signed"),
            vega_cbor_tag_v1(0, vega_cbor_text_v1("2025-01-01T00:00:00Z")),
        ),
        (
            vega_cbor_text_v1("validFrom"),
            vega_cbor_tag_v1(0, vega_cbor_text_v1("2025-01-01T00:00:00Z")),
        ),
        (
            vega_cbor_text_v1("validUntil"),
            vega_cbor_tag_v1(0, vega_cbor_text_v1("2035-08-17T12:34:56Z")),
        ),
    ]);
    let value_digests = vega_cbor_map_v1(vec![(
        vega_cbor_text_v1("org.iso.18013.5.1"),
        vega_cbor_map_v1(vec![(
            vega_cbor_unsigned_v1(1),
            vega_cbor_bytes_v1(&birth_digest),
        )]),
    )]);
    let mso_inner = vega_cbor_map_v1(vec![
        (vega_cbor_text_v1("version"), vega_cbor_text_v1("1.0")),
        (
            vega_cbor_text_v1("digestAlgorithm"),
            vega_cbor_text_v1("SHA-256"),
        ),
        (vega_cbor_text_v1("valueDigests"), value_digests),
        (
            vega_cbor_text_v1("deviceKeyInfo"),
            vega_cbor_map_v1(vec![(vega_cbor_text_v1("deviceKey"), device_key)]),
        ),
        (
            vega_cbor_text_v1("docType"),
            vega_cbor_text_v1("org.iso.18013.5.1.mDL"),
        ),
        (vega_cbor_text_v1("validityInfo"), validity_info),
    ]);
    let mso_payload = vega_cbor_tag_v1(24, vega_cbor_bytes_v1(&mso_inner));
    let sig_structure = vega_cbor_array_v1(vec![
        vega_cbor_text_v1("Signature1"),
        vega_cbor_bytes_v1(&[0xa1, 0x01, 0x26]),
        vega_cbor_bytes_v1(&[]),
        vega_cbor_bytes_v1(&mso_payload),
    ]);

    let genesis_hash = VEGA_RELEASE_GENESIS_HASH_V1;
    let public_input = VegaPrivacyActionPublicInputV1 {
        issuer_record,
        presentation_date: PrivacyVegaMdlDateV1 {
            year: 2026,
            month: 7,
            day: 26,
        },
        minimum_age_years: 18,
        reader_challenge: PrivacyChallengeV1::new([0x31; 32]),
        session_transcript_digest: PrivacySessionTranscriptDigestV1::new([0x32; 32]),
    };
    let issuer_digest: [u8; 32] = Sha256::digest(&sig_structure).into();
    let issuer_signature: P256Signature = issuer_signing_key
        .sign_prehash(&issuer_digest)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let issuer_signature = issuer_signature.normalize_s().unwrap_or(issuer_signature);
    let (issuer_r, issuer_s) = issuer_signature.split_scalars();
    let issuer_high_s_signature =
        P256Signature::from_scalars(issuer_r.to_repr(), (-*issuer_s).to_repr())
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if issuer_high_s_signature.normalize_s().is_none() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(VegaReleaseFixtureV1 {
        public_input,
        issuer_record,
        issuer_authentication_sig_structure: sig_structure,
        mobile_security_object_payload: mso_payload,
        birth_date_issuer_signed_item: birth_item,
        issuer_signature,
        issuer_high_s_signature,
        device_signing_key,
        genesis_hash,
    })
}

fn vega_compressed_public_key_v1(
    signing_key: &P256SigningKey,
) -> Result<PrivacyP256PointV1, PrivacyReleaseEvidenceErrorClassV1> {
    let encoded = signing_key.verifying_key().to_encoded_point(true);
    let bytes: [u8; 33] = encoded
        .as_bytes()
        .try_into()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(PrivacyP256PointV1::new(bytes))
}

fn vega_cbor_head_v1(major: u8, argument: u64) -> Vec<u8> {
    let argument_bytes = argument.to_be_bytes();
    match argument {
        0..=23 => vec![
            (major << 5) | u8::try_from(argument).expect("CBOR immediate argument is at most 23"),
        ],
        24..=0xff => vec![
            (major << 5) | 24,
            u8::try_from(argument).expect("CBOR one-byte argument is at most 255"),
        ],
        0x100..=0xffff => vec![(major << 5) | 25, argument_bytes[6], argument_bytes[7]],
        0x1_0000..=0xffff_ffff => vec![
            (major << 5) | 26,
            argument_bytes[4],
            argument_bytes[5],
            argument_bytes[6],
            argument_bytes[7],
        ],
        _ => {
            let mut encoded = vec![(major << 5) | 27];
            encoded.extend_from_slice(&argument_bytes);
            encoded
        }
    }
}

fn vega_cbor_unsigned_v1(value: u64) -> Vec<u8> {
    vega_cbor_head_v1(0, value)
}

fn vega_cbor_negative_v1(value: i64) -> Vec<u8> {
    debug_assert!(value < 0);
    let argument = u64::try_from(-(i128::from(value)) - 1)
        .expect("negative i64 has a non-negative CBOR argument fitting u64");
    vega_cbor_head_v1(1, argument)
}

fn vega_cbor_bytes_v1(value: &[u8]) -> Vec<u8> {
    let mut encoded = vega_cbor_head_v1(
        2,
        u64::try_from(value.len()).expect("slice length fits CBOR u64"),
    );
    encoded.extend_from_slice(value);
    encoded
}

fn vega_cbor_text_v1(value: &str) -> Vec<u8> {
    let mut encoded = vega_cbor_head_v1(
        3,
        u64::try_from(value.len()).expect("string length fits CBOR u64"),
    );
    encoded.extend_from_slice(value.as_bytes());
    encoded
}

fn vega_cbor_array_v1(values: Vec<Vec<u8>>) -> Vec<u8> {
    let mut encoded = vega_cbor_head_v1(
        4,
        u64::try_from(values.len()).expect("array length fits CBOR u64"),
    );
    for value in values {
        encoded.extend_from_slice(&value);
    }
    encoded
}

fn vega_cbor_map_v1(mut entries: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
    entries.sort_by(|left, right| {
        left.0
            .len()
            .cmp(&right.0.len())
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut encoded = vega_cbor_head_v1(
        5,
        u64::try_from(entries.len()).expect("map length fits CBOR u64"),
    );
    for (key, value) in entries {
        encoded.extend_from_slice(&key);
        encoded.extend_from_slice(&value);
    }
    encoded
}

fn vega_cbor_tag_v1(tag: u64, value: Vec<u8>) -> Vec<u8> {
    let mut encoded = vega_cbor_head_v1(6, tag);
    encoded.extend_from_slice(&value);
    encoded
}

/// Return the exact canonical release descriptor for one closed protocol.
///
/// Runner-side aggregate validation compares this byte-for-byte; a stage
/// cannot substitute a self-consistent free-form description.
#[must_use]
pub const fn privacy_release_protocol_descriptor_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> &'static str {
    match protocol_id {
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => {
            "zk-ace-pq-authorization-v0; prover=zk_ace::prove_zk_ace_privacy_v1_with_rng; verifier=zk_ace::verify_zk_ace_privacy_v1; fixed-primary=4096 execution-trace rows; fixed-secondary=108 unique verifier queries; fixed-depth=12 Fp4 FRI rounds; proof-cap=1341142 exact bytes; theorem=classical-ROM work-normalized >=128 bits; qROM-not-claimed"
        }
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => {
            "anonymous-pgc-k-out-of-n-v1; prover=anonymous_pgc::bootstrap::prove_bootstrap+anonymous_pgc::payment::prove_payment; verifier=anonymous_pgc::bootstrap::verify_bootstrap_encoded+anonymous_pgc::payment::verify_payment_encoded; positive-and-maximum-artifact-order=account-bootstrap,payment; payment-invariant=verified-bootstrap-payload-and-proof-digests; authoritative-root-effect=canonical-epoch1-to-epoch2-complete-account-table-successor-validation; max-primary=64 anonymity-set members; max-secondary=8 recipients; max-depth=32 range-proof bits; artifact-caps=MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,MAX_PGC_PAYMENT_PROOF_BYTES_V1"
        }
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
            "verange-transparent-range-v1; prover=verange::prove_batch; verifier=verange::verify_batch_encoded; max-primary=8 commitments; max-secondary=64 range bits; max-depth=8 Figure-1 matrix rows; proof-cap=MAX_VERANGE_TYPE1_BATCH_PROOF_BYTES_V1"
        }
        PrivacyProtocolIdV1::IrohaZkAmsV1 => {
            "iroha-zk-ams-v1; prover=zk_ams::prove_zk_ams_batch_admission_v1+zk_ams::sign_zk_ams_provision_statement_v1; verifier=privacy_verifier::verify_privacy_envelope_v1; native-verifier=zk_ams::verify_zk_ams_batch_admission_v1+zk_ams::verify_zk_ams_provision_statement_v1; batch-wire=independent-version+exact-count+fixed-eight-slots+canonical-zero-unused-tail; all-case-artifact-order=batch8-admission,successor-root-provisioning; lineage=two-sequential-single-action-transactions+distinct-intent-digests+authoritative-prestate-record-digest-to-batch-successor-record-digest-to-full-admitted-ring; max-primary=64 admitted ring members; max-secondary=8 ordered admission anchors; max-depth=64 MLSAGS cyclic responses; artifact-caps=MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1,MAX_ZK_AMS_LSAG_PROOF_BYTES_V1"
        }
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0 => {
            "vega-existing-credential-zk-v0; prover=vega::prove_mdl_figure9_v1; verifier=privacy_verifier::verify_privacy_envelope_v1; verifier-state=privacy_verifier::validate_vega_authoritative_issuer_binding_v1+vega::verify_mdl_figure9_v1; signature-preflight=P1363-nonzero-scalars+low-S-required+reject-high-S-without-normalization+verify-prehash-before-inverse; fixed-primary=1048576 padded R1CS constraints; fixed-secondary=524288 padded private variables; fixed-public-inputs=14; fixed-depth=40 combined outer+inner sumcheck rounds; proof-cap=524288 canonical bytes; issuer-state=current active self-digested append-only revision"
        }
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => {
            "iroha-zk-x509-stark-p256-v0; native P-256 X.509 predicate STARK; primary=certificate bytes; secondary=predicate constraints; depth=certificate-chain depth"
        }
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 => {
            "iroha-jindo-polynomial-commitment-v0; prover-state=jindo::prepare_jindo_privacy_action_with_rng_v1; verifier=jindo::verify_batched_evaluation_v1; max-primary=4 polynomials; max-secondary=256 coefficients each; max-depth=256 ring degree; proof-cap=JINDO_NATIVE_PROOF_BYTES_V1"
        }
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
            "iroha-bootle-lantern-anoncred-v1; prover-state=bootle_lantern::prove_bound_presentation_v1; verifier-state=bootle_lantern::verify_bound_presentation_encoded_v1; max-primary=8 disclosed attributes; max-secondary=32 governed allowed values per required attribute; max-depth=8 module rank; proof-cap=bootle_lantern::codec::PROOF_BYTES_V1"
        }
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
            "orchard-halo2-actions-v1; prover=orchard::prepare_orchard_bundle_v1_with_rng+orchard::authorize_orchard_bundle_v1; verifier=orchard::verify_orchard_bundle_v1; authorization=consuming-two-phase+native-consensus-binding; max-primary=2 actions; max-secondary=2 spends; max-depth=32 note-tree levels; proof-cap=orchard::orchard_authorization_wire_size_v1(2)"
        }
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
            "monero-fcmp-plus-plus-v1; prover=fcmp_plus_plus::prove_fcmp_plus_plus_v1; verifier=fcmp_plus_plus::verify_fcmp_transaction_v1; max-primary=2 inputs; max-secondary=4 strictly-positive outputs; max-depth=32 alternating Selene/Helios curve-tree layers; proof-cap=12520 exact max-shape IFC1 bytes; bounded-challenge-and-full-proof-retry=128"
        }
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
            "iroha-ivm-private-note-stark-v1; prover=ivm_private_note::prove_ivm_private_note_v1_with_rng; verifier=ivm_private_note::verify_ivm_private_note_v1; public-input=statement+mandatory-native-consensus-binding; max-primary=2 consumed notes; max-secondary=2 created notes; fixed-depth=32 SHA-256 note-tree levels; fixed-trace=16384 rows; fixed-queries=60; proof-cap=8388608 IPS1 bytes; wallet=X25519+XChaCha20Poly1305"
        }
        PrivacyProtocolIdV1::PqMaspStarkV0 => {
            "pq-masp-stark-v0; prover=pq_masp::prove_pq_masp_v1_with_rng; verifier=pq_masp::verify_pq_masp_v1; public-input=statement+mandatory-native-consensus-binding; max-primary=2 consumed notes; max-secondary=2 created notes; fixed-depth=32 SHA-256 note-tree levels; fixed-trace=16384 rows; fixed-queries=60; proof-cap=9437184 complete PQA1 bytes; authorization=ML-DSA-65-over-statement+binding+inner-proof; wallet=ML-KEM-768+XChaCha20Poly1305"
        }
    }
}

fn stage_seed_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"iroha.privacy.release.evidence.internal-seed.v1");
    hash.update(protocol_id.canonical_label().as_bytes());
    hash.update(case_kind.canonical_label().as_bytes());
    hash.finalize().into()
}

fn stage_purpose_seed_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
    purpose: &[u8],
) -> Result<[u8; 32], PrivacyReleaseEvidenceErrorClassV1> {
    let purpose_length = u64::try_from(purpose.len())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let mut hash = Sha256::new();
    hash.update(b"iroha.privacy.release.evidence.purpose-seed.v1");
    hash.update(stage_seed_v1(protocol_id, case_kind));
    hash.update(purpose_length.to_be_bytes());
    hash.update(purpose);
    let seed: [u8; 32] = hash.finalize().into();
    if seed.iter().all(|byte| *byte == 0) {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(seed)
}

fn sha256_v1(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

struct EvidenceRng06 {
    seed: [u8; 32],
    counter: u64,
}

impl EvidenceRng06 {
    const fn new(seed: [u8; 32]) -> Self {
        Self { seed, counter: 0 }
    }
}

impl RngCore for EvidenceRng06 {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0_u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_be_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_be_bytes(bytes)
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        let mut offset = 0;
        while offset < destination.len() {
            let mut hash = Sha256::new();
            hash.update(b"iroha.privacy.release.evidence.rng06.v1");
            hash.update(self.seed);
            hash.update(self.counter.to_be_bytes());
            self.counter = self.counter.wrapping_add(1);
            let block: [u8; 32] = hash.finalize().into();
            let take = (destination.len() - offset).min(block.len());
            destination[offset..offset + take].copy_from_slice(&block[..take]);
            offset += take;
        }
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError06> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for EvidenceRng06 {}

struct EvidenceRng09 {
    seed: [u8; 32],
    counter: u64,
}

impl EvidenceRng09 {
    const fn new(seed: [u8; 32]) -> Self {
        Self { seed, counter: 0 }
    }
}

impl rand::TryRngCore for EvidenceRng09 {
    type Error = core::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bytes = [0_u8; 4];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u32::from_be_bytes(bytes))
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bytes = [0_u8; 8];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u64::from_be_bytes(bytes))
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        let mut offset = 0;
        while offset < destination.len() {
            let mut hash = Sha256::new();
            hash.update(b"iroha.privacy.release.evidence.rng09.v1");
            hash.update(self.seed);
            hash.update(self.counter.to_be_bytes());
            self.counter = self.counter.wrapping_add(1);
            let block: [u8; 32] = hash.finalize().into();
            let take = (destination.len() - offset).min(block.len());
            destination[offset..offset + take].copy_from_slice(&block[..take]);
            offset += take;
        }
        Ok(())
    }
}

impl rand::TryCryptoRng for EvidenceRng09 {}

#[cfg(test)]
mod tests {
    use iroha_primitives::json::Json;

    use super::*;
    use crate::privacy_engines::vega::{
        build_signed_vega_privacy_action_with_rng_v1, sign_prepared_vega_privacy_action_v1,
    };

    const RAYON_POOL_CHILD_MARKER_V1: &str = "IROHA_PRIVACY_RELEASE_RAYON_POOL_CHILD_V1";

    #[test]
    fn zk_ams_release_lineage_uses_distinct_single_action_transactions() {
        let admission =
            zk_ams_admission_transaction_context_v1().expect("admission transaction context");
        let provision =
            zk_ams_provision_transaction_context_v1().expect("provision transaction context");

        assert_eq!(ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1, 0);
        assert_eq!(ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1, 0);
        assert_eq!(admission.chain_id, provision.chain_id);
        assert_eq!(admission.authority, provision.authority);
        assert_eq!(admission.time_to_live, provision.time_to_live);
        assert_eq!(admission.fee_payment, provision.fee_payment);
        assert_eq!(admission.metadata, provision.metadata);
        assert!(
            admission.creation_time < provision.creation_time,
            "admission must precede provisioning"
        );
        assert!(
            admission.nonce.expect("admission nonce") < provision.nonce.expect("provision nonce"),
            "sequential transactions require ordered nonces"
        );
    }

    #[test]
    fn zk_ams_release_envelope_distinguishes_admission_from_native_rejection() {
        let ring = zk_ams_sorted_ring_v1(ZK_AMS_MIN_RING_SIZE_V1).expect("canonical minimum ring");
        let key_image = zk_ams_key_image_v1(&ring[5].1).expect("canonical key image");
        let statement = zk_ams_provision_statement_v1(
            &ring,
            key_image,
            PrivacyRootV1::new([0x41; 32]),
            2,
            PrivacyZkAmsRegistryRecordDigestV1::new([0x42; 32]),
        )
        .expect("canonical provisioning statement");
        let authoritative_chain_id = ChainId::from(ZK_AMS_RELEASE_CHAIN_ID_V1);

        assert_eq!(
            verify_zk_ams_release_production_envelope_v1(
                &statement,
                &[0x01],
                &authoritative_chain_id,
                ZK_AMS_RELEASE_GENESIS_HASH_V1,
                ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
            ),
            Err(PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected),
            "a canonical one-action envelope must reach the native ZK-AMS verifier"
        );

        let mut impossible_second_action = statement;
        impossible_second_action.context.action_index = 1;
        assert_eq!(
            verify_zk_ams_release_production_envelope_v1(
                &impossible_second_action,
                &[0x01],
                &authoritative_chain_id,
                ZK_AMS_RELEASE_GENESIS_HASH_V1,
                ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
            ),
            Err(PrivacyReleaseEvidenceErrorClassV1::ProductionEnvelopeRejected),
            "Taira's one-action transaction limit must reject before native verification"
        );
    }

    #[test]
    fn vega_release_fixture_uses_the_canonical_single_taira_action() {
        let fixture = vega_release_fixture_v1().expect("canonical Vega release fixture");
        let transaction =
            vega_release_transaction_context_v1().expect("canonical Vega transaction context");
        let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("compiled Vega profile");
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let context = PrivacyStatementContextV1 {
            chain_id: transaction.chain_id.clone(),
            action_index: VEGA_RELEASE_ACTION_INDEX_V1,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x27; 32]),
            parameter_id: profile.parameter_id,
            parameter_digest: profile.parameter_digest,
            verifier_digest: profile.verifier_digest,
            statement_schema_digest: profile.statement_schema_digest,
            engine_manifest_digest: profile.engine_manifest_digest,
        };

        fixture
            .public_input
            .issuer_record
            .validate()
            .expect("canonical active Vega issuer record");
        context
            .validate(&limits)
            .expect("Vega is the sole privacy action in its transaction");
        assert_eq!(VEGA_RELEASE_ACTION_INDEX_V1, 0);
        assert_eq!(
            transaction.chain_id,
            ChainId::from(VEGA_RELEASE_CHAIN_ID_V1)
        );
        assert_eq!(
            transaction.creation_time,
            Duration::from_millis(VEGA_RELEASE_CREATION_TIME_MS_V1)
        );
        assert_eq!(transaction.nonce, NonZeroU32::new(VEGA_RELEASE_NONCE_V1));

        let mut impossible_second_action = context;
        impossible_second_action.action_index = 1;
        assert!(matches!(
            impossible_second_action.validate(&limits),
            Err(
                iroha_data_model::privacy::PrivacyStatementValidationError::ActionIndexOutOfBounds {
                    index: 1,
                    max_actions: 1,
                }
            )
        ));
    }

    #[test]
    #[ignore = "release gate: proves the full native Vega Figure 9 action once"]
    fn vega_action_api_binds_signs_and_rejects_transaction_proof_and_statement_drift() {
        let fixture = vega_release_fixture_v1().expect("canonical Vega release fixture");
        let witness_material = VegaPrivacyActionWitnessMaterialV1::new(
            fixture.issuer_authentication_sig_structure.clone(),
            fixture.mobile_security_object_payload.clone(),
            fixture.birth_date_issuer_signed_item.clone(),
            &fixture.issuer_signature.to_bytes(),
        )
        .expect("canonical Vega action witness material");
        let mut rng = EvidenceRng06::new([0x91; 32]);
        let prepared = prepare_vega_privacy_action_with_rng_v1(
            vega_release_transaction_context_v1().expect("canonical transaction context"),
            fixture.public_input,
            witness_material,
            &fixture.device_signing_key,
            fixture.genesis_hash,
            VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
            &mut rng,
        )
        .expect("canonical two-pass Vega action");
        assert_ne!(prepared.transaction_intent_digest(), [0; 32]);
        assert_ne!(prepared.statement_digest(), [0; 32]);
        assert_ne!(prepared.proof_envelope_hash(), [0; 32]);
        assert_eq!(
            prepared.effect(),
            crate::privacy_engines::vega::VegaPrivacyActionEffectV1::
                ActionVerificationAndFinalityOnly
        );
        let prepared_debug = format!("{prepared:?}");
        assert!(!prepared_debug.contains("TransactionPayload"));
        assert!(!prepared_debug.contains("PrivacyProofBytes"));
        assert!(!prepared_debug.contains("issuer_authentication_sig_structure"));

        let payload = prepared.release_evidence_payload_v1().clone();
        match payload.instructions() {
            iroha_data_model::transaction::Executable::Instructions(instructions) => {
                assert_eq!(instructions.len(), 1, "exactly one direct Vega action");
                assert!(
                    instructions[0]
                        .as_any()
                        .downcast_ref::<SubmitPrivacyProofV1>()
                        .is_some(),
                    "the sole action must be the typed Vega submission"
                );
            }
            other => panic!("unexpected Vega executable form: {other:?}"),
        }
        assert!(
            payload.attachments.is_none(),
            "canonical Vega actions cannot carry proof attachments"
        );
        let (intent, submission) = payload
            .privacy_transaction_intent_binding_if_present_v1()
            .expect("canonical direct privacy scan")
            .expect("exactly one Vega submission");
        assert_eq!(intent.as_bytes(), &prepared.transaction_intent_digest());
        let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) =
            &submission.envelope.statement
        else {
            panic!("prepared Vega statement changed variant")
        };
        let PrivacyProofV1::VegaExistingCredentialZkV0(proof) = &submission.envelope.proof else {
            panic!("prepared Vega proof changed variant")
        };
        assert_eq!(statement.context.action_index, VEGA_PRIVACY_ACTION_INDEX_V1);
        assert!(!proof.as_bytes().is_empty());
        assert_eq!(
            prepared.statement_bytes(),
            u32::try_from(
                norito::to_bytes(&submission.envelope.statement)
                    .expect("typed Vega statement encodes")
                    .len()
            )
            .expect("bounded Vega statement")
        );
        assert_eq!(
            prepared.proof_bytes(),
            u32::try_from(proof.as_bytes().len()).expect("bounded Vega proof")
        );
        let encoded_envelope =
            norito::to_bytes(&submission.envelope).expect("typed Vega envelope encodes");
        assert_eq!(
            prepared.encoded_proof_envelope_bytes(),
            u32::try_from(encoded_envelope.len()).expect("bounded Vega envelope")
        );
        assert_eq!(
            prepared.proof_envelope_hash(),
            *iroha_crypto::Hash::new(&encoded_envelope).as_ref()
        );
        submission
            .envelope
            .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
            .expect("prepared envelope is intrinsically valid");
        let mut proof_empty_escape = submission.envelope.clone();
        proof_empty_escape.proof =
            PrivacyProofV1::VegaExistingCredentialZkV0(PrivacyProofBytesV1::new(Vec::new()));
        assert!(
            proof_empty_escape
                .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
                .is_err(),
            "the internal proof-empty projection must never be submittable"
        );

        let mut changed_chain = payload.clone();
        changed_chain.chain = ChainId::from("vega-signed-action-wrong-chain-v1");
        assert!(
            changed_chain
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "chain mutation must invalidate the signed intent"
        );
        let mut changed_authority = payload.clone();
        changed_authority.authority =
            privacy_release_account_v1(0x57).expect("fixed alternate authority");
        assert!(
            changed_authority
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "authority mutation must invalidate the signed intent"
        );
        let mut changed_creation_time = payload.clone();
        changed_creation_time.creation_time_ms += 1;
        assert!(
            changed_creation_time
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "creation-time mutation must invalidate the signed intent"
        );
        let mut changed_fee = payload.clone();
        changed_fee.fee_payment =
            FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(6_000_000));
        assert!(
            changed_fee
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "fee mutation must invalidate the signed intent"
        );
        let mut changed_ttl = payload.clone();
        changed_ttl.time_to_live_ms = NonZeroU64::new(61_000);
        assert!(
            changed_ttl
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "TTL mutation must invalidate the signed intent"
        );
        let mut changed_nonce = payload.clone();
        changed_nonce.nonce = NonZeroU32::new(VEGA_RELEASE_NONCE_V1 + 1);
        assert!(
            changed_nonce
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "nonce mutation must invalidate the signed intent"
        );
        let mut changed_metadata = payload.clone();
        changed_metadata.metadata.insert(
            "vega_intent_mutation"
                .parse()
                .expect("canonical metadata key"),
            Json::new(1_u32),
        );
        assert!(
            changed_metadata
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "metadata mutation must invalidate the signed intent"
        );

        let binding =
            VegaMdlConsensusBindingV1::from_context(&statement.context, fixture.genesis_hash);
        let mut changed_proof = proof.as_bytes().to_vec();
        let changed_proof_index = changed_proof.len() / 2;
        changed_proof[changed_proof_index] ^= 1;
        assert!(
            verify_mdl_figure9_v1(
                statement,
                &binding,
                VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                &changed_proof,
            )
            .is_err(),
            "proof drift must fail native verification"
        );
        let mut changed_statement = statement.clone();
        changed_statement.minimum_age_years += 1;
        refresh_vega_device_authentication_digest_v1(&mut changed_statement, fixture.genesis_hash)
            .expect("mutated statement has canonical H_dev");
        let changed_binding = VegaMdlConsensusBindingV1::from_context(
            &changed_statement.context,
            fixture.genesis_hash,
        );
        assert!(
            verify_mdl_figure9_v1(
                &changed_statement,
                &changed_binding,
                VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                proof.as_bytes(),
            )
            .is_err(),
            "statement drift must fail native verification"
        );
        let mut impossible_second_action = statement.clone();
        impossible_second_action.context.action_index = 1;
        assert!(matches!(
            PrivacyStatementV1::VegaExistingCredentialZkV0(impossible_second_action)
                .validate(&PrivacyConsensusLimitsV1::taira_default()),
            Err(
                iroha_data_model::privacy::PrivacyStatementValidationError::ActionIndexOutOfBounds {
                    index: 1,
                    max_actions: 1,
                }
            )
        ));

        let transaction_key_pair = KeyPair::try_from_seed(vec![0x56; 32], Algorithm::Ed25519)
            .expect("fixed Vega transaction key");
        let expected_intent = prepared.transaction_intent_digest();
        let signed =
            sign_prepared_vega_privacy_action_v1(prepared, transaction_key_pair.private_key())
                .expect("sign sealed Vega action");
        signed
            .signed_transaction()
            .verify_signature()
            .expect("signed Vega transaction verifies");
        assert_eq!(signed.transaction_intent_digest(), expected_intent);
        assert_eq!(
            signed.transaction_hash(),
            *signed.signed_transaction().hash().as_ref()
        );
        assert!(
            signed.signed_transaction().attachments().is_none(),
            "signed canonical Vega actions cannot carry attachments"
        );
        let signed_debug = format!("{signed:?}");
        assert!(!signed_debug.contains("SignedTransaction {"));
        assert!(!signed_debug.contains("PrivacyProofBytes"));
        let mut signed_intent_drift = signed.signed_transaction().payload().clone();
        signed_intent_drift.nonce = NonZeroU32::new(VEGA_RELEASE_NONCE_V1 + 2);
        let independently_resigned_drift = TransactionBuilder::from_payload(signed_intent_drift)
            .expect("otherwise canonical drifted payload")
            .try_sign(transaction_key_pair.private_key())
            .expect("transaction signature covers the drifted payload");
        independently_resigned_drift
            .verify_signature()
            .expect("drifted payload has an independently valid transaction signature");
        assert!(
            independently_resigned_drift
                .privacy_transaction_intent_binding_if_present_v1()
                .is_err(),
            "a valid transaction signature cannot redeem a stale Vega intent"
        );

        let wrong_key_fixture =
            vega_release_fixture_v1().expect("second canonical Vega release fixture");
        let wrong_key_material = VegaPrivacyActionWitnessMaterialV1::new(
            wrong_key_fixture
                .issuer_authentication_sig_structure
                .clone(),
            wrong_key_fixture.mobile_security_object_payload.clone(),
            wrong_key_fixture.birth_date_issuer_signed_item.clone(),
            &wrong_key_fixture.issuer_signature.to_bytes(),
        )
        .expect("canonical wrong-key witness material");
        let foreign_key_pair = KeyPair::try_from_seed(vec![0x57; 32], Algorithm::Ed25519)
            .expect("fixed foreign transaction key");
        let wrong_key = build_signed_vega_privacy_action_with_rng_v1(
            vega_release_transaction_context_v1().expect("canonical transaction context"),
            wrong_key_fixture.public_input,
            wrong_key_material,
            &wrong_key_fixture.device_signing_key,
            wrong_key_fixture.genesis_hash,
            VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
            foreign_key_pair.private_key(),
            &mut EvidenceRng06::new([0x92; 32]),
        );
        assert!(matches!(
            wrong_key,
            Err(crate::privacy_engines::vega::VegaPrivacyActionBuildErrorV1::AuthorityKeyMismatch)
        ));
    }

    #[test]
    fn canonical_process_profile_is_exact_and_has_one_authoritative_source() {
        let profiles = PrivacyProtocolIdV1::ALL
            .into_iter()
            .filter_map(privacy_release_process_profile_v1)
            .collect::<Vec<_>>();
        assert_eq!(
            profiles,
            vec![PrivacyReleaseProcessProfileV1 {
                protocol_id: PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                elapsed_ceiling_millis: 300_000,
                peak_rss_ceiling_bytes: 12_884_901_888,
            }]
        );
        assert_eq!(
            profiles[0].elapsed_ceiling_millis,
            ZK_X509_PROVER_TARGET_SECONDS_V1 * 1_000
        );
        assert_eq!(
            profiles[0].peak_rss_ceiling_bytes,
            ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
        );
    }

    #[test]
    fn privacy_release_rayon_pool_fresh_process_child_v1() {
        if std::env::var_os(RAYON_POOL_CHILD_MARKER_V1).is_none() {
            return;
        }
        assert_eq!(PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1, 4);
        initialize_privacy_release_rayon_pool_v1().expect("initialize exact release Rayon pool");
        assert_eq!(
            rayon::current_num_threads(),
            usize::from(PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1)
        );
        assert_eq!(
            initialize_privacy_release_rayon_pool_v1(),
            Err(PrivacyReleaseRayonPoolErrorV1::InitializationRejected),
            "a second global-pool initialization must fail closed"
        );
    }

    #[test]
    fn privacy_release_rayon_pool_is_one_time_and_exact_at_api_boundary_v1() {
        let executable = std::env::current_exe().expect("resolve core unit-test executable");
        let output = std::process::Command::new(executable)
            .arg("privacy_release_rayon_pool_fresh_process_child_v1")
            .arg("--nocapture")
            .env(RAYON_POOL_CHILD_MARKER_V1, "1")
            .output()
            .expect("execute release Rayon API child");
        assert!(
            output.status.success(),
            "release Rayon API child failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn frozen_stage_order_is_explicit_and_matches_the_enum_product() {
        assert!(validate_privacy_release_stage_coordinates_v1(
            &PRIVACY_RELEASE_STAGE_COORDINATES_V1
        ));
        let mut observed = Vec::new();
        for protocol_id in PrivacyProtocolIdV1::ALL {
            for case_kind in PrivacyReleaseCaseKindV1::ALL {
                observed.push(privacy_release_stage_ordinal_v1(protocol_id, case_kind));
            }
        }
        assert_eq!(observed.len(), PRIVACY_RELEASE_STAGE_COUNT_V1);
        assert_eq!(
            observed,
            (0..u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1).unwrap()).collect::<Vec<_>>()
        );
        assert_eq!(
            PRIVACY_RELEASE_STAGE_COORDINATES_V1
                .map(|coordinate| coordinate.stage_ordinal)
                .to_vec(),
            observed
        );
    }

    #[test]
    fn resource_facts_are_frozen_for_available_stages_and_x509_remains_pending() {
        for protocol_id in PrivacyProtocolIdV1::ALL {
            for case_kind in PrivacyReleaseCaseKindV1::ALL {
                let facts = privacy_release_resource_facts_v1(protocol_id, case_kind);
                if protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
                    assert_eq!(facts, None);
                    assert_eq!(
                        run_privacy_release_stage_v1(protocol_id, case_kind),
                        Err(PrivacyReleaseEvidenceErrorV1 {
                            protocol_id,
                            case_kind,
                            class: PrivacyReleaseEvidenceErrorClassV1::ProtocolUnavailable,
                        })
                    );
                } else {
                    let facts = facts.expect("implemented stage has frozen resource facts");
                    assert!(facts.validate());
                    if case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource {
                        assert_eq!(facts.primary_units, facts.primary_ceiling);
                        assert_eq!(facts.secondary_units, facts.secondary_ceiling);
                        assert_eq!(facts.relation_depth, facts.relation_depth_ceiling);
                    }
                }
            }
        }
    }

    #[test]
    fn exact_parsers_reject_aliases_and_case_folding() {
        for case_kind in PrivacyReleaseCaseKindV1::ALL {
            assert_eq!(
                PrivacyReleaseCaseKindV1::from_canonical_label(case_kind.canonical_label()),
                Some(case_kind)
            );
        }
        assert_eq!(
            PrivacyReleaseCaseKindV1::from_canonical_label("Positive-Canonical-End-To-End"),
            None
        );
        assert_eq!(
            PrivacyReleaseCaseKindV1::from_canonical_label("positive-canonical-end-to-end "),
            None
        );
        assert_eq!(
            PrivacyReleaseCaseKindV1::from_canonical_label("positive"),
            None
        );
    }

    #[test]
    fn evidence_seeds_are_deterministic_and_purpose_separated() {
        let case_kind = PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation;
        let purposes: [&[u8]; 6] = [
            b"canonical-fixture-keygen",
            b"canonical-fixture-encryption",
            b"canonical-proof",
            b"invalid-path-fixture-keygen",
            b"invalid-path-fixture-encryption",
            b"invalid-path-proof",
        ];
        for protocol_id in [
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
        ] {
            let seeds = purposes
                .iter()
                .map(|purpose| {
                    stage_purpose_seed_v1(protocol_id, case_kind, purpose)
                        .expect("fixed evidence purpose derives a seed")
                })
                .collect::<Vec<_>>();
            for (index, seed) in seeds.iter().enumerate() {
                assert_eq!(
                    *seed,
                    stage_purpose_seed_v1(protocol_id, case_kind, purposes[index])
                        .expect("same purpose derives the same seed")
                );
                for other in &seeds[index + 1..] {
                    assert_ne!(seed, other);
                }
            }
        }
        assert_ne!(
            stage_purpose_seed_v1(
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                case_kind,
                b"canonical-proof",
            )
            .expect("IVM proof seed"),
            stage_purpose_seed_v1(
                PrivacyProtocolIdV1::PqMaspStarkV0,
                case_kind,
                b"canonical-proof",
            )
            .expect("PQ-MASP proof seed"),
        );
    }

    #[test]
    fn unavailable_protocols_fail_closed_without_placeholder_evidence() {
        let error = run_privacy_release_stage_v1(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
        )
        .expect_err("incomplete X.509 release fixture must fail closed");
        assert_eq!(
            error.class,
            PrivacyReleaseEvidenceErrorClassV1::ProtocolUnavailable
        );
    }

    #[test]
    fn maximum_fixture_dimensions_equal_governed_first_release_caps() {
        assert_eq!(BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1, 32);
        assert_eq!(ZK_AMS_MAX_RING_SIZE_V1, 64);
        assert_eq!(ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1, 8);
        assert_eq!(ORCHARD_MAX_ACTIONS_V1, 2);
        assert_eq!(ORCHARD_TREE_DEPTH_V1, 32);

        let orchard = orchard_maximum_spend_fixture_v1()
            .expect("maximum Orchard fixture has two shared-anchor real spends");
        assert_eq!(orchard.spends.len(), ORCHARD_MAX_ACTIONS_V1);
        assert_eq!(orchard.total_value, 36);
        assert_ne!(orchard.anchor, orchard_empty_root_v1());
    }

    #[test]
    fn ordered_proof_artifact_cardinality_is_closed_and_fail_closed() {
        let artifact = |protocol_id: PrivacyProtocolIdV1,
                        case_kind: PrivacyReleaseCaseKindV1,
                        artifact_ordinal: u8| {
            let canonical_proof_bytes =
                vec![artifact_ordinal.saturating_add(1); usize::from(artifact_ordinal) + 1];
            PrivacyReleaseProofArtifactEvidenceV1 {
                artifact_ordinal,
                proof_sha256: sha256_v1(&canonical_proof_bytes),
                canonical_proof_bytes,
                proof_bytes_ceiling: privacy_release_proof_artifact_ceiling_v1(
                    protocol_id,
                    case_kind,
                    artifact_ordinal,
                )
                .expect("valid fixture artifact has a canonical ceiling"),
            }
        };
        let ordinary_protocol = PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1;
        let ordinary_case = PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd;
        let pgc_protocol = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
        let zk_ams_protocol = PrivacyProtocolIdV1::IrohaZkAmsV1;
        let maximum_case = PrivacyReleaseCaseKindV1::MaximumShapeResource;
        let adversarial_case = PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation;
        assert_eq!(
            privacy_release_proof_artifact_count_v1(ordinary_protocol, ordinary_case),
            1
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(pgc_protocol, maximum_case),
            2
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(pgc_protocol, ordinary_case),
            2
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(zk_ams_protocol, maximum_case),
            2
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(zk_ams_protocol, ordinary_case),
            2
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(zk_ams_protocol, adversarial_case),
            2
        );
        assert!(validate_privacy_release_proof_artifacts_v1(
            ordinary_protocol,
            ordinary_case,
            &[artifact(ordinary_protocol, ordinary_case, 0)],
        ));
        assert!(validate_privacy_release_proof_artifacts_v1(
            pgc_protocol,
            maximum_case,
            &[
                artifact(pgc_protocol, maximum_case, 0),
                artifact(pgc_protocol, maximum_case, 1),
            ],
        ));
        assert!(validate_privacy_release_proof_artifacts_v1(
            pgc_protocol,
            ordinary_case,
            &[
                artifact(pgc_protocol, ordinary_case, 0),
                artifact(pgc_protocol, ordinary_case, 1),
            ],
        ));
        assert!(validate_privacy_release_proof_artifacts_v1(
            zk_ams_protocol,
            ordinary_case,
            &[
                artifact(zk_ams_protocol, ordinary_case, 0),
                artifact(zk_ams_protocol, ordinary_case, 1),
            ],
        ));
        assert!(validate_privacy_release_proof_artifacts_v1(
            zk_ams_protocol,
            adversarial_case,
            &[
                artifact(zk_ams_protocol, adversarial_case, 0),
                artifact(zk_ams_protocol, adversarial_case, 1),
            ],
        ));
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(pgc_protocol, ordinary_case, 0),
            u64::try_from(MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(pgc_protocol, ordinary_case, 1),
            u64::try_from(MAX_PGC_PAYMENT_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(zk_ams_protocol, ordinary_case, 0),
            u64::try_from(MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(zk_ams_protocol, ordinary_case, 1),
            u64::try_from(MAX_ZK_AMS_LSAG_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(zk_ams_protocol, adversarial_case, 0),
            u64::try_from(MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(zk_ams_protocol, adversarial_case, 1),
            u64::try_from(MAX_ZK_AMS_LSAG_PROOF_BYTES_V1).ok()
        );

        let valid = artifact(ordinary_protocol, ordinary_case, 0);
        let mut hash_mismatch = valid.clone();
        hash_mismatch.proof_sha256[0] ^= 1;
        let mut empty = valid.clone();
        empty.canonical_proof_bytes.clear();
        empty.proof_sha256 = sha256_v1(&empty.canonical_proof_bytes);
        let mut over_ceiling = valid.clone();
        over_ceiling.canonical_proof_bytes = vec![
            7;
            usize::try_from(over_ceiling.proof_bytes_ceiling)
                .expect("FCMP++ ceiling fits usize")
                + 1
        ];
        over_ceiling.proof_sha256 = sha256_v1(&over_ceiling.canonical_proof_bytes);
        let mut zero_ceiling = valid.clone();
        zero_ceiling.proof_bytes_ceiling = 0;
        let mut substituted_ceiling = valid.clone();
        substituted_ceiling.proof_bytes_ceiling = substituted_ceiling
            .proof_bytes_ceiling
            .checked_sub(1)
            .expect("FCMP++ ceiling is nonzero");
        let mut unbounded_ceiling = valid.clone();
        unbounded_ceiling.proof_bytes_ceiling = PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1 + 1;
        let mut byte_mutation = valid.clone();
        byte_mutation.canonical_proof_bytes[0] ^= 1;
        let malformed = [
            Vec::new(),
            vec![valid.clone(), valid.clone()],
            vec![hash_mismatch],
            vec![empty],
            vec![over_ceiling],
            vec![zero_ceiling],
            vec![substituted_ceiling],
            vec![unbounded_ceiling],
            vec![byte_mutation],
        ];
        for artifacts in malformed {
            assert!(!validate_privacy_release_proof_artifacts_v1(
                ordinary_protocol,
                ordinary_case,
                &artifacts,
            ));
        }
        let pgc_artifact_zero = artifact(pgc_protocol, maximum_case, 0);
        let pgc_artifact_one = artifact(pgc_protocol, maximum_case, 1);
        for artifacts in [
            vec![pgc_artifact_zero.clone()],
            vec![pgc_artifact_one.clone(), pgc_artifact_zero.clone()],
            vec![pgc_artifact_zero.clone(), pgc_artifact_zero.clone()],
            vec![
                pgc_artifact_zero.clone(),
                PrivacyReleaseProofArtifactEvidenceV1 {
                    artifact_ordinal: 2,
                    ..pgc_artifact_one.clone()
                },
            ],
            vec![
                pgc_artifact_zero.clone(),
                pgc_artifact_one.clone(),
                pgc_artifact_one,
            ],
        ] {
            assert!(!validate_privacy_release_proof_artifacts_v1(
                pgc_protocol,
                maximum_case,
                &artifacts,
            ));
        }
    }

    #[test]
    fn proof_artifact_consensus_cap_is_exact_and_cap_plus_one_rejects() {
        assert_eq!(
            PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1,
            u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
        );
        assert_eq!(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1, 9 * 1024 * 1024);
        let protocol_id = PrivacyProtocolIdV1::PqMaspStarkV0;
        let case_kind = PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd;
        let ceiling = privacy_release_proof_artifact_ceiling_v1(protocol_id, case_kind, 0)
            .expect("PQ-MASP stage has one canonical ceiling");
        assert_eq!(ceiling, PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1);

        let canonical_proof_bytes =
            vec![0x5a; usize::try_from(ceiling).expect("Taira proof cap fits usize")];
        let mut artifact = PrivacyReleaseProofArtifactEvidenceV1 {
            artifact_ordinal: 0,
            proof_sha256: sha256_v1(&canonical_proof_bytes),
            canonical_proof_bytes,
            proof_bytes_ceiling: ceiling,
        };
        assert!(validate_privacy_release_proof_artifacts_v1(
            protocol_id,
            case_kind,
            core::slice::from_ref(&artifact),
        ));

        artifact.canonical_proof_bytes.push(0);
        artifact.proof_sha256 = sha256_v1(&artifact.canonical_proof_bytes);
        assert!(!validate_privacy_release_proof_artifacts_v1(
            protocol_id,
            case_kind,
            core::slice::from_ref(&artifact),
        ));
    }

    #[test]
    fn every_typed_artifact_has_one_protocol_ceiling_below_the_consensus_cap() {
        let mut artifact_count = 0_usize;
        for protocol_id in PrivacyProtocolIdV1::ALL {
            for case_kind in PrivacyReleaseCaseKindV1::ALL {
                let stage_count = usize::from(privacy_release_proof_artifact_count_v1(
                    protocol_id,
                    case_kind,
                ));
                artifact_count = artifact_count
                    .checked_add(stage_count)
                    .expect("closed artifact count fits usize");
                for ordinal in 0..stage_count {
                    let ceiling = privacy_release_proof_artifact_ceiling_v1(
                        protocol_id,
                        case_kind,
                        u8::try_from(ordinal).expect("at most two artifacts"),
                    )
                    .expect("every required artifact has one canonical ceiling");
                    assert!(ceiling > 0);
                    assert!(ceiling <= PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1);
                }
                assert!(
                    privacy_release_proof_artifact_ceiling_v1(
                        protocol_id,
                        case_kind,
                        u8::try_from(stage_count).expect("at most two artifacts"),
                    )
                    .is_none()
                );
            }
        }
        assert_eq!(artifact_count, PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1);
    }

    #[test]
    fn canonical_proof_bytes_use_json_base64_and_round_trip_exactly() {
        let protocol_id = PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1;
        let case_kind = PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd;
        let canonical_proof_bytes = vec![0x00, 0x01, 0xfe, 0xff];
        let artifact = PrivacyReleaseProofArtifactEvidenceV1 {
            artifact_ordinal: 0,
            proof_sha256: sha256_v1(&canonical_proof_bytes),
            canonical_proof_bytes,
            proof_bytes_ceiling: privacy_release_proof_artifact_ceiling_v1(
                protocol_id,
                case_kind,
                0,
            )
            .expect("FCMP++ stage has one canonical ceiling"),
        };
        let json = norito::json::to_json(&artifact).expect("artifact JSON encodes");
        assert!(json.contains("\"canonical_proof_bytes\":\"AAH+/w==\""));
        let decoded: PrivacyReleaseProofArtifactEvidenceV1 =
            norito::json::from_str(&json).expect("artifact JSON decodes");
        assert_eq!(decoded, artifact);
        let unpadded = json.replace("AAH+/w==", "AAH+/w");
        assert!(
            norito::json::from_str::<PrivacyReleaseProofArtifactEvidenceV1>(&unpadded).is_err(),
            "non-canonical base64 spelling must reject"
        );

        let mut legacy_json = json;
        let closing_brace = legacy_json
            .pop()
            .expect("canonical artifact JSON has a closing brace");
        assert_eq!(closing_brace, '}');
        legacy_json.push_str(",\"proof_bytes\":4}");
        assert!(
            norito::json::from_str::<PrivacyReleaseProofArtifactEvidenceV1>(&legacy_json).is_err(),
            "removed reported-length field must not be accepted as a compatibility alias"
        );
    }

    #[test]
    fn every_protocol_has_one_distinct_nonempty_canonical_descriptor() {
        let descriptors = PrivacyProtocolIdV1::ALL.map(privacy_release_protocol_descriptor_v1);
        assert!(descriptors.iter().all(|descriptor| !descriptor.is_empty()));
        for (index, descriptor) in descriptors.iter().enumerate() {
            assert!(!descriptors[index + 1..].contains(descriptor));
        }
    }

    #[test]
    #[ignore = "operator-only native proof construction for the complete ZK-AMS corruption stage"]
    fn zk_ams_corruption_stage_rejects_maximum_and_submaximum_wire_mutations() {
        let protocol_id = PrivacyProtocolIdV1::IrohaZkAmsV1;
        let case_kind = PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation;
        let evidence =
            run_privacy_release_stage_v1(protocol_id, case_kind).expect("ZK-AMS corruption stage");
        assert_eq!(evidence.protocol_id, protocol_id);
        assert_eq!(evidence.case_kind, case_kind);
        assert_eq!(
            evidence.failure_class,
            PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected
        );
        assert_eq!(
            evidence.proof_artifacts.len(),
            usize::from(privacy_release_proof_artifact_count_v1(
                protocol_id,
                case_kind
            ))
        );
        assert!(validate_privacy_release_proof_artifacts_v1(
            protocol_id,
            case_kind,
            &evidence.proof_artifacts,
        ));
    }
}
