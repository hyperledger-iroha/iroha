//! `SoraNet`-specific data model extensions.
//!
//! This module hosts forward-looking types for privacy tickets, relay
//! incentives, and additional transport metadata surfaced by the `SoraNet`
//! anonymity layer. The initial implementation focused on zero-knowledge
//! privacy tickets so SoraFS streaming requests can remain anonymous while
//! remaining auditable by relays and gateways. The incentive scaffolding
//! introduced in SNNet-7 models relay bonding, bandwidth attestations, and
//! reward instructions so the treasury can remunerate SoraNet relays with
//! deterministic Norito payloads.

#![allow(clippy::module_name_repetitions)]

/// Canonical 32-byte digest type used across `SoraNet` payloads.
pub type Digest32 = [u8; 32];

/// Relay identifier derived from the directory fingerprint.
pub type RelayId = Digest32;

/// Incentive and payout scaffolding for SoraNet relays.
pub mod incentives;
/// Aggregated privacy-preserving telemetry buckets and summaries.
pub mod privacy_metrics;
/// Privacy ticket payloads and envelopes.
pub mod ticket;
/// VPN cell/control-plane payloads for the native SoraNet tunnel.
pub mod vpn;

/// Re-export commonly used `SoraNet` types.
pub mod prelude {
    pub use super::{
        Digest32, RelayId,
        incentives::{
            BandwidthConfidenceV1, RelayBandwidthProofV1, RelayBondLedgerEntryV1,
            RelayBondPolicyV1, RelayComplianceStatusV1, RelayEpochMetricsV1,
            RelayRewardInstructionV1,
        },
        privacy_metrics::{
            SoranetGarAbuseCountV1, SoranetGarAbuseShareV1, SoranetLatencyPercentileV1,
            SoranetPrivacyBucketMetricsV1, SoranetPrivacyEventActiveSampleV1,
            SoranetPrivacyEventGarAbuseCategoryV1, SoranetPrivacyEventHandshakeFailureV1,
            SoranetPrivacyEventHandshakeSuccessV1, SoranetPrivacyEventKindV1,
            SoranetPrivacyEventThrottleV1, SoranetPrivacyEventV1,
            SoranetPrivacyEventVerifiedBytesV1, SoranetPrivacyHandshakeFailureV1,
            SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1, SoranetPrivacyThrottleScopeV1,
        },
        ticket::{TicketBodyV1, TicketEnvelopeV1, TicketScopeV1},
        vpn::{
            VPN_CELL_LEN, VpnCellClassV1, VpnCellError, VpnCellFlagsV1, VpnCellHeaderV1, VpnCellV1,
            VpnControlPlaneV1, VpnCoverPlanEntryV1, VpnCoverScheduleV1, VpnExitClassV1,
            VpnFlowLabelV1, VpnPaddedCellV1, VpnRouteV1, VpnSessionReceiptV1,
        },
    };
}
