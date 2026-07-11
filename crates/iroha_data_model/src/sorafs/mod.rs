//! `SoraFS` data model scaffolding.
//!
//! This module hosts forward-looking types for the `SoraFS` pin registry and
//! related governance flows. The pin registry operates alongside the manifest
//! schema defined in `sorafs_manifest` and stores both canonical manifest
//! digests (the envelope identity) and exact binary root CIDs (the content DAG
//! identity), plus replication policies and lifecycle metadata. These two
//! commitments are distinct and replication/alias records bind both where
//! applicable. The deal module extends this
//! surface with storage market accounting (contracts, micropayments, bonds),
//! while the pricing module captures governance-controlled tariffs and credit
//! policy so ISI definitions can coordinate incentives deterministically. The
//! repair module models audit-driven repair queues that tie proof failures to
//! remediation workflows, and the transparency module defines canonical
//! moderation ledger payloads/proofs for public SFM-4c verifiers.

/// Capacity marketplace records (provider declarations, telemetry, fees).
pub mod capacity;

/// Gateway Authorization Record policy payload types.
pub mod gar;

/// Moderation reproducibility manifests, `SoraFS` ballot payloads, and helpers.
pub mod moderation;

/// Pin registry manifest metadata and lifecycle records.
pub mod pin_registry;

/// Storage deal contracts, micropayment tickets, and settlement ledgers.
pub mod deal;

/// Governance-controlled pricing schedule and credit policy.
pub mod pricing;

/// Reserve + rent policy and lifecycle quoting.
pub mod reserve;

/// Transparency ledger entries, cycle headers, and inclusion proofs.
pub mod transparency;

/// Re-export commonly used `SoraFS` types.
pub mod prelude {
    pub use super::{
        capacity::{
            CapacityAccrual, CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
            CapacityDisputeOutcome, CapacityDisputeRecord, CapacityDisputeResolution,
            CapacityDisputeStatus, CapacityFeeLedgerEntry, CapacityLedgerMutationError,
            CapacityTelemetryRecord, ProviderId,
        },
        deal::{
            BondLedgerMutationError, ClientId, DealComputationError, DealId, DealProposal,
            DealProposalValidationError, DealRecord, DealSettlementRecord,
            DealSettlementValidationError, DealStatus, DealTerms, DealTermsValidationError,
            DealUsageReport, DealUsageValidationError, MicropaymentTicket,
            MicropaymentTicketValidationError, ProviderBondLedgerEntry, TicketId,
        },
        gar::{
            GarCdnPolicyV1, GarEnforcementActionV1, GarEnforcementReceiptV1, GarLicenseSetV1,
            GarMetricsPolicyV1, GarModerationAction, GarModerationDirectiveV1, GarPolicyPayloadV1,
        },
        moderation::{
            MODERATION_COMMITTEE_AGGREGATE_VERSION_V1, MODERATION_COMMITTEE_MAX_RESULTS_V1,
            MODERATION_MODEL_ARTIFACT_VERSION_V1, MODERATION_MODEL_FEATURE_COUNT_V1,
            MODERATION_MODEL_MAX_ARTIFACT_BYTES_V1, MODERATION_MODEL_MAX_ARTIFACT_PATH_BYTES_V1,
            MODERATION_MODEL_MAX_CALIBRATION_KNOTS_V1, MODERATION_MODEL_MAX_INPUT_BYTES_V1,
            MODERATION_MODEL_MAX_MODELS_V1, MODERATION_MODEL_MAX_TOTAL_ARTIFACT_BYTES_V1,
            MODERATION_MODEL_WORKING_MEMORY_BYTES_V1, MODERATION_PROVENANCE_LOG_VERSION_V1,
            MODERATION_PROVENANCE_MAX_ENTRIES_V1, MODERATION_REPRO_MANIFEST_VERSION_V1,
            MODERATION_REPRO_MAX_BPS, MODERATION_REPRO_MAX_NOTES_BYTES_V1,
            MODERATION_REPRO_MAX_RUNTIME_VERSION_BYTES_V1,
            MODERATION_REPRO_MAX_SEED_DOMAIN_BYTES_V1,
            MODERATION_REPRO_MAX_SIGNATURE_ROLE_BYTES_V1, MODERATION_REPRO_MAX_SIGNATURES_V1,
            MODERATION_SIGNED_RESULT_MAX_SUBJECT_BYTES_V1, MODERATION_SIGNED_RESULT_VERSION_V1,
            MODERATION_TRUST_MAX_CLOCK_SKEW_SECS_V1, MODERATION_TRUST_MAX_RESULT_AGE_SECS_V1,
            MODERATION_TRUST_MAX_RESULT_TTL_SECS_V1, MODERATION_TRUST_MAX_SIGNATURES_V1,
            MODERATION_TRUST_MAX_SIGNERS_V1, MODERATION_TRUST_POLICY_VERSION_V1,
            ModerationCalibrationKnotV1, ModerationCommitteeAggregateError,
            ModerationCommitteeAggregateV1, ModerationCommitteeMemberV1,
            ModerationFeatureProfileV1, ModerationModelArtifactError, ModerationModelArtifactV1,
            ModerationModelEngineV1, ModerationModelFingerprintV1, ModerationModelScoreV1,
            ModerationProvenanceEntryV1, ModerationProvenanceError, ModerationProvenanceLogV1,
            ModerationProvenancePayloadV1, ModerationReproBodyV1, ModerationReproManifestSummary,
            ModerationReproManifestV1, ModerationReproSignatureV1, ModerationReproValidationError,
            ModerationSeedMaterialV1, ModerationSignedResultError, ModerationSignedScreeningBodyV1,
            ModerationSignedScreeningResultV1, ModerationThresholdsV1, ModerationTrustPolicyBodyV1,
            ModerationTrustPolicyError, ModerationTrustPolicySignatureV1,
            ModerationTrustPolicySummaryV1, ModerationTrustPolicyV1, ModerationTrustedSignerV1,
            SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
            SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1, SoraFsModerationBallotCommitV1,
            SoraFsModerationBallotContextV1, SoraFsModerationBallotError,
            SoraFsModerationBallotRevealV1, SoraFsModerationVoteChoice,
            is_canonical_moderation_artifact_path_v1, moderation_model_required_operations_v1,
        },
        pin_registry::{
            ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord,
            ManifestDigest, ManifestRootCid, ManifestRootCidError, ManifestRootCidErrorKind,
            PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderId, ReplicationOrderRecord,
            ReplicationOrderStatus, StorageClass,
        },
        pricing::{
            CollateralPolicy, CommitmentDiscountTier, CreditMutationError, CreditPolicy,
            DiscountSchedule, PricingComputationError, PricingScheduleRecord,
            PricingValidationError, ProviderCreditRecord, TierRate, checked_mul_div_floor_u128,
            checked_mul_div_round_u128,
        },
        reserve::{
            ClassRentRate, ReserveDuration, ReserveLedgerProjection, ReserveLifecycleProjection,
            ReserveLifecycleStage, ReservePolicyError, ReservePolicyV1, ReserveQuote, ReserveTier,
            ReserveTierConfig,
        },
        transparency::{
            MODERATION_LEDGER_BLOCK_VERSION_V1, MODERATION_LEDGER_ENTRY_VERSION_V1,
            MODERATION_LEDGER_PROOF_VERSION_V1, MODERATION_LEDGER_PUBLICATION_VERSION_V1,
            MODERATION_PRIVACY_AGGREGATE_VERSION_V1, MODERATION_PRIVACY_DELTA_PPB_MAX,
            MODERATION_PRIVACY_PARAMETERS_VERSION_V1, ModerationLedgerBlockV1,
            ModerationLedgerCyclePublicationV1, ModerationLedgerEntryKindV1,
            ModerationLedgerEntryV1, ModerationLedgerMetadataV1, ModerationLedgerProofNodeV1,
            ModerationLedgerProofSideV1, ModerationLedgerProofV1,
            ModerationPrivacyAggregateMetricV1, ModerationPrivacyAggregateV1,
            ModerationPrivacyModeV1, ModerationPrivacyParametersV1,
            PROOF_TOKEN_ISSUANCE_VERSION_V1, ProofTokenIssuanceV1, TransparencyLedgerError,
        },
    };
}
