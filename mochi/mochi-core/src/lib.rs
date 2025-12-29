//! Core orchestration primitives for the MOCHI local-network supervisor.
//!
//! This crate owns configuration templating, process lifecycle management,
//! and Torii client plumbing shared by every MOCHI front end.

pub mod compose;
pub mod config;
mod genesis;
pub mod logs;
pub mod state;
pub mod supervisor;
pub mod torii;
pub mod vault;

pub use compose::{
    ComposeError, InstructionDraft, InstructionPermission, SigningAuthority,
    TransactionComposeOptions, TransactionPreview, compose_preview, compose_preview_with_authority,
    compose_preview_with_options, development_signing_authorities, drafts_from_json_str,
    drafts_to_pretty_json, mint_numeric_preview,
};
pub use config::{GenesisProfile, NetworkProfile, NetworkTopology, ProfilePreset};
pub use genesis::default_manifest;
pub use iroha_crypto::{ExposedPrivateKey, KeyPair, PrivateKey};
pub use iroha_telemetry::metrics::{Status as TelemetryStatus, TxGossipSnapshot};
pub use logs::{LifecycleEvent, LogStreamKind, PeerLogEvent, PeerLogStream};
pub use state::{
    StateCursor, StateEntry, StatePage, StateQueryError, StateQueryKind, run_state_query,
};
pub use supervisor::{
    BinaryPaths, BinaryVersionInfo, CompatibilityReport, KagamiVerifyReport, PeerHandle, PeerState,
    Result as SupervisorResult, Supervisor, SupervisorBuilder, SupervisorError,
};
pub use torii::{
    BlockDecodeStage, BlockStream, BlockStreamDecodeError, BlockStreamEvent, BlockSummary,
    EventCategory, EventDecodeStage, EventStream, EventStreamDecodeError, EventStreamEvent,
    EventSummary, ManagedBlockStream, ManagedEventStream, ManagedStatusStream, ReadinessOptions,
    ReadinessSmokeBuildError, ReadinessSmokeOutcome, ReadinessSmokePlan, SmokeCommitOptions,
    SmokeCommitSnapshot, StatusMetrics, StatusStreamEvent, ToriiClient, ToriiError, ToriiErrorInfo,
    ToriiErrorKind, ToriiMetricsSnapshot, ToriiResult, ToriiStatusSnapshot, TriggerListPage,
    TriggerListQuery, TriggerRecord, WsFrame, WsSubscription, decode_norito_with_alignment,
};
pub use vault::{SIGNERS_FILE_NAME, SignerVault, SignerVaultError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_builder_uses_requested_profile() {
        let builder = SupervisorBuilder::new(ProfilePreset::SinglePeer);
        assert_eq!(
            builder.profile(),
            &NetworkProfile {
                preset: Some(ProfilePreset::SinglePeer),
                topology: NetworkTopology::single_peer(),
                consensus_mode:
                    iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned,
            }
        );
    }
}
