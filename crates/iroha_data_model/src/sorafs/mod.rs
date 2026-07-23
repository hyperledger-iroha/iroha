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
//! moderation ledger payloads/proofs for public SFM-4c verifiers. The
//! `pop_registry` module defines the consensus-owned, payload-free credential
//! issuer commitments and signed root/revocation publications used by SFM-4b1.

/// Capacity marketplace records (provider declarations, telemetry, fees).
pub mod capacity;

/// Gateway Authorization Record policy payload types.
pub mod gar;

/// Moderation reproducibility manifests, `SoraFS` ballot payloads, and helpers.
pub mod moderation;

/// Authoritative on-chain moderation commit/reveal policy and records.
pub mod moderation_ledger;

/// Authoritative orderbook policy and on-chain audit records.
pub mod orderbook;

/// Pin registry manifest metadata and lifecycle records.
pub mod pin_registry;

/// Authoritative proof-of-personhood issuer and registry records.
pub mod pop_registry;

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
        moderation_ledger::{
            MODERATION_APPEAL_INTAKE_DIGEST_DOMAIN_V1, MODERATION_APPEAL_INTAKE_VERSION_V1,
            MODERATION_LEDGER_CASE_VERSION_V1, MODERATION_LEDGER_MAX_CANDIDATE_POOL_SIZE_V1,
            MODERATION_LEDGER_MAX_CHALLENGES_V1, MODERATION_LEDGER_MAX_EVIDENCE_URI_BYTES_V1,
            MODERATION_LEDGER_MAX_EXCLUSIONS_V1, MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1,
            MODERATION_LEDGER_MAX_NONCE_BYTES_V1, MODERATION_LEDGER_MAX_PANEL_SIZE_V1,
            MODERATION_LEDGER_MAX_PENALTY_POINTS_V1, MODERATION_LEDGER_MAX_REASON_BYTES_V1,
            MODERATION_LEDGER_MAX_TOTAL_WINDOW_MS_V1, MODERATION_LEDGER_MAX_WAITLIST_SIZE_V1,
            MODERATION_LEDGER_POLICY_DIGEST_DOMAIN_V1, MODERATION_LEDGER_POLICY_VERSION_V1,
            MODERATION_LEDGER_ROSTER_HASH_DOMAIN_V1, MODERATION_POP_CHALLENGE_DOMAIN_V1,
            MODERATION_POP_SNAPSHOT_DIGEST_DOMAIN_V1, MODERATION_SORTITION_DIGEST_DOMAIN_V1,
            MODERATION_SORTITION_SCORE_DOMAIN_V1, MODERATION_SORTITION_SEED_DOMAIN_V1,
            ModerationAppealIntakeError, ModerationAppealIntakeV1, ModerationAppealRecordV1,
            ModerationAppealStatusV1, ModerationCaseRecordV1, ModerationCaseSpecError,
            ModerationCaseSpecV1, ModerationCaseStatusV1, ModerationChallengeDecisionV1,
            ModerationChallengeKindV1, ModerationChallengeRecordV1, ModerationCommitRecordV1,
            ModerationJurorEligibilityClassV1, ModerationJurorEligibilityRecordV1,
            ModerationJurorReplacementV1, ModerationLedgerPolicyError,
            ModerationLedgerPolicyRecord, ModerationLedgerPolicyV1, ModerationLedgerStatusV1,
            ModerationNoShowKindV1, ModerationNoShowRecordV1, ModerationOutcomeKindV1,
            ModerationOutcomeRecordV1, ModerationPanelSelectionV1,
            ModerationPoPRegistrySnapshotError, ModerationPoPRegistrySnapshotV1,
            ModerationRevealRecordV1, ModerationSortitionError, ModerationVoteCountsV1,
            REPAIR_LEDGER_APPEAL_ID_DOMAIN_V1, REPAIR_LEDGER_IDEMPOTENCY_DOMAIN_V1,
            REPAIR_LEDGER_MAX_APPEAL_REASON_BYTES_V1, REPAIR_LEDGER_MAX_IDEMPOTENCY_KEY_BYTES_V1,
            REPAIR_LEDGER_MAX_LEASE_MS_V1, REPAIR_LEDGER_MAX_RECEIPTS_V1,
            REPAIR_LEDGER_MIN_LEASE_MS_V1, REPAIR_LEDGER_TASK_ID_DOMAIN_V1,
            REPAIR_LEDGER_TASK_VERSION_V1, RepairLedgerActionReceiptV1, RepairLedgerAppealRecordV1,
            RepairLedgerCompletedV1, RepairLedgerEscalatedV1, RepairLedgerFailedV1,
            RepairLedgerLeaseV1, RepairLedgerSlashRecordV1, RepairLedgerStatusV1,
            RepairLedgerTaskV1, RepairLedgerTerminalKindV1, RepairLedgerTerminalOutcomeV1,
            sorafs_moderation_panel_roster_hash_v1, sorafs_moderation_pop_challenge_v1,
            sorafs_moderation_pop_verifier_context_v1, sorafs_moderation_select_panel_v1,
            sorafs_moderation_sortition_digest_v1, sorafs_moderation_sortition_seed_v1,
            sorafs_repair_appeal_id_v1, sorafs_repair_idempotency_digest_v1,
            sorafs_repair_task_id_v1,
        },
        orderbook::{
            ORDERBOOK_ADMISSION_POLICY_DIGEST_DOMAIN_V1, ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            ORDERBOOK_MAX_CLOCK_SKEW_SECS_V1, ORDERBOOK_MAX_ORDER_LIFETIME_SECS_V1,
            ORDERBOOK_MAX_RECEIPT_AGE_SECS_V1, ORDERBOOK_MAX_RECEIPT_BYTES_V1,
            ORDERBOOK_MAX_RECEIPTS_PER_CHANNEL_V1, ORDERBOOK_QUERY_MAX_ITEMS_V1,
            ORDERBOOK_SETTLEMENT_ESCROW_ID_DOMAIN_V1, OrderbookAdmissionPolicyRecord,
            OrderbookAdmissionPolicyV1, OrderbookCancellationRecord, OrderbookLedgerStatusV1,
            OrderbookOrderPageV1, OrderbookOrderRecord, OrderbookOrderStatusV1,
            OrderbookOwnerNonceRecord, OrderbookPolicyValidationError,
            OrderbookSettlementIndexRecord, OrderbookSettlementRangeRecord,
            OrderbookSettlementReceiptPageV1, OrderbookSettlementReceiptRecord,
            orderbook_settlement_escrow_id,
        },
        pin_registry::{
            ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord,
            ManifestDigest, ManifestRootCid, ManifestRootCidError, ManifestRootCidErrorKind,
            PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderId, ReplicationOrderRecord,
            ReplicationOrderStatus, StorageClass,
        },
        pop_registry::{
            POP_COMMITMENT_ROOT_PAYLOAD_MAX_BYTES_V1, POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1,
            POP_CREDENTIAL_COMMITMENTS_MAX_V1, POP_CREDENTIAL_LIFETIME_MAX_SECS_V1,
            POP_CREDENTIAL_PAYLOAD_COMMITMENT_DOMAIN_V1, POP_ISSUER_ID_MAX_BYTES_V1,
            POP_ISSUER_POLICY_DIGEST_DOMAIN_V1, POP_ISSUER_POLICY_VERSION_V1,
            POP_PUBLICATION_CLOCK_SKEW_MAX_SECS_V1, POP_REGISTRY_AUDIT_DIGEST_DOMAIN_V1,
            POP_REGISTRY_PAYLOAD_DIGEST_DOMAIN_V1, POP_REVOCATION_LIST_PAYLOAD_MAX_BYTES_V1,
            POP_REVOCATION_NONCE_COMMITMENT_DOMAIN_V1, POP_REVOCATIONS_PER_PUBLICATION_MAX_V1,
            PopCommitmentRootRecordV1, PopCredentialCommitmentBatchV1,
            PopCredentialCommitmentBatchValidationError, PopCredentialCommitmentRecordV1,
            PopCredentialCommitmentV1, PopCredentialCommitmentValidationError,
            PopIssuerPolicyRecordV1, PopIssuerPolicyV1, PopIssuerPolicyValidationError,
            PopRegistryAuditDigestRecordV1, PopRegistryAuditEventKindV1,
            PopRegistryRevocationReasonV1, PopRegistryStatusV1, PopRevocationPublicationRecordV1,
            PopRevocationRecordV1, pop_credential_payload_commitment_v1,
            pop_registry_payload_digest_v1, pop_revocation_nonce_commitment_v1,
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
