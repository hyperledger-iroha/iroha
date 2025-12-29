//! SoraFS data model scaffolding.
//!
//! This module hosts forward-looking types for the SoraFS pin registry and
//! related governance flows. The pin registry operates alongside the manifest
//! schema defined in `sorafs_manifest` and stores canonical manifest digests,
//! replication policies, and lifecycle metadata. The deal module extends this
//! surface with storage market accounting (contracts, micropayments, bonds),
//! while the pricing module captures governance-controlled tariffs and credit
//! policy so ISI definitions can coordinate incentives deterministically. The
//! repair module models audit-driven repair queues that tie proof failures to
//! remediation workflows.

/// Capacity marketplace records (provider declarations, telemetry, fees).
pub mod capacity;

/// Gateway Authorization Record policy payload types.
pub mod gar;

/// Moderation reproducibility manifests and helpers.
pub mod moderation;

/// Pin registry manifest metadata and lifecycle records.
pub mod pin_registry;

/// Storage deal contracts, micropayment tickets, and settlement ledgers.
pub mod deal;

/// Governance-controlled pricing schedule and credit policy.
pub mod pricing;

/// Reserve + rent policy and lifecycle quoting.
pub mod reserve;

/// Re-export commonly used `SoraFS` types.
pub mod prelude {
    pub use super::capacity::{
        CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
        CapacityDisputeOutcome, CapacityDisputeRecord, CapacityDisputeResolution,
        CapacityDisputeStatus, CapacityFeeLedgerEntry, CapacityTelemetryRecord, ProviderId,
    };
    pub use super::deal::{
        ClientId, DealId, DealProposal, DealRecord, DealSettlementRecord, DealStatus, DealTerms,
        DealUsageReport, MicropaymentTicket, ProviderBondLedgerEntry, TicketId,
    };
    pub use super::gar::{
        GarCdnPolicyV1, GarEnforcementActionV1, GarEnforcementReceiptV1, GarLicenseSetV1,
        GarMetricsPolicyV1, GarModerationAction, GarModerationDirectiveV1, GarPolicyPayloadV1,
    };
    pub use super::moderation::{
        MODERATION_REPRO_MANIFEST_VERSION_V1, ModerationModelFingerprintV1, ModerationReproBodyV1,
        ModerationReproManifestSummary, ModerationReproManifestV1, ModerationReproSignatureV1,
        ModerationReproValidationError, ModerationSeedMaterialV1, ModerationThresholdsV1,
    };
    pub use super::pin_registry::{
        ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord,
        ManifestDigest, PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderId,
        ReplicationOrderRecord, ReplicationOrderStatus, ReplicationReceiptRecord,
        ReplicationReceiptStatus, StorageClass,
    };
    pub use super::pricing::{
        CollateralPolicy, CommitmentDiscountTier, CreditPolicy, DiscountSchedule,
        PricingScheduleRecord, ProviderCreditRecord, TierRate,
    };
    pub use super::reserve::{
        ClassRentRate, ReserveDuration, ReserveLedgerProjection, ReservePolicyError,
        ReservePolicyV1, ReserveQuote, ReserveTier, ReserveTierConfig,
    };
}
