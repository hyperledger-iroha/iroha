//! `SoraFS` data model scaffolding.
//!
//! This module hosts forward-looking types for the `SoraFS` pin registry and
//! related governance flows. The pin registry operates alongside the manifest
//! schema defined in `sorafs_manifest` and stores canonical manifest digests,
//! replication policies, and lifecycle metadata. The deal module extends this
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
            CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
            CapacityDisputeOutcome, CapacityDisputeRecord, CapacityDisputeResolution,
            CapacityDisputeStatus, CapacityFeeLedgerEntry, CapacityTelemetryRecord, ProviderId,
        },
        deal::{
            ClientId, DealId, DealProposal, DealRecord, DealSettlementRecord, DealStatus,
            DealTerms, DealUsageReport, MicropaymentTicket, ProviderBondLedgerEntry, TicketId,
        },
        gar::{
            GarCdnPolicyV1, GarEnforcementActionV1, GarEnforcementReceiptV1, GarLicenseSetV1,
            GarMetricsPolicyV1, GarModerationAction, GarModerationDirectiveV1, GarPolicyPayloadV1,
        },
        moderation::{
            MODERATION_REPRO_MANIFEST_VERSION_V1, ModerationModelFingerprintV1,
            ModerationReproBodyV1, ModerationReproManifestSummary, ModerationReproManifestV1,
            ModerationReproSignatureV1, ModerationReproValidationError, ModerationSeedMaterialV1,
            ModerationThresholdsV1, SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
            SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1, SoraFsModerationBallotCommitV1,
            SoraFsModerationBallotContextV1, SoraFsModerationBallotError,
            SoraFsModerationBallotRevealV1, SoraFsModerationVoteChoice,
        },
        pin_registry::{
            ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord,
            ManifestDigest, PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderId,
            ReplicationOrderRecord, ReplicationOrderStatus, StorageClass,
        },
        pricing::{
            CollateralPolicy, CommitmentDiscountTier, CreditPolicy, DiscountSchedule,
            PricingScheduleRecord, ProviderCreditRecord, TierRate,
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
