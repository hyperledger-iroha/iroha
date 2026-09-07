//! Canonical Torii status response and capability snapshots.
//!
//! Named Norito schema identifiers are explicit protocol identities, independent
//! of the private module grouping used to maintain these DTOs.
mod common;
mod consensus;
mod gossip;
mod governance;
mod nexus;
mod taikai;

pub use common::{BuildStatus, CryptoStatus, Halo2Status, StackStatus, Status, Uptime};
pub use consensus::SumeragiConsensusStatus;
pub use gossip::{DaReceiptCursorStatus, TxGossipCaps, TxGossipSnapshot, TxGossipStatus};
pub use governance::{
    GovernanceManifestActivation, GovernanceManifestAdmissionCounters,
    GovernanceManifestQuorumCounters, GovernanceProposalCounters,
    GovernanceProtectedNamespaceCounters, GovernanceStatus,
};
pub use nexus::{
    NexusDataspaceCatalogStatus, NexusDataspaceTeuStatus, NexusLaneManifestValidatorBindingStatus,
    NexusLaneRuntimeUpgradeHookStatus, NexusLaneTeuBuckets, NexusLaneTeuDeferrals,
    NexusLaneTeuStatus, NexusRoutingMatcherStatus, NexusRoutingPolicyStatus,
    NexusRoutingRuleStatus, NexusStatus, SchedulerLayerWidthBuckets,
};
pub use taikai::{TaikaiAliasRotationStatus, TaikaiIngestErrorCounter, TaikaiIngestStatus};

#[cfg(test)]
mod manifest_tests;
