//! Core orchestration primitives for the MOCHI local-network supervisor.
//!
//! This crate owns configuration templating, process lifecycle management,
//! and Torii client plumbing shared by every MOCHI front end.
pub mod bootstrap;
pub mod chaos;
pub mod compose;
pub mod config;
pub mod dashboard;
mod generation;
mod genesis;
pub mod logs;
mod path_safety;
mod secret;
pub mod state;
pub mod supervisor;
pub mod torii;
pub mod vault;
pub use bootstrap::{
    BootstrapArtifact, BootstrapBundle, BootstrapInputs, BootstrapWriteError, ENV_LOCAL_FILE,
    KOTLIN_SAMPLE_FILE, RUST_SAMPLE_FILE, TYPESCRIPT_SAMPLE_FILE, ensure_http_base, shell_quote,
    write_bootstrap_bundle,
};
pub use chaos::{
    ChaosError, ChaosEvent, ChaosPreset, ChaosReport, ChaosRunRequest, ChaosRunResult,
    run_chaos_preset,
};
pub use compose::{
    ComposeError, InstructionDraft, InstructionPermission, SigningAuthority,
    TransactionComposeOptions, TransactionPreview, compose_preview_with_options,
    development_signing_authorities, drafts_from_json_str, drafts_to_pretty_json,
};
pub use config::{
    GenesisProfile, NetworkProfile, NetworkTopology, ProfilePreset,
    infer_workspace_root_from_sandbox_root, sandbox_root_for_workspace,
};
pub use dashboard::{
    DashboardAccountCard, DashboardAccountInput, DashboardAssetBalance, DashboardRecentBlock,
    DashboardSnapshot, fetch_dashboard_snapshot,
};
pub use genesis::{sample_cabbage_definition_id, sample_rose_definition_id};
pub use iroha_crypto::{ExposedPrivateKey, KeyPair, PrivateKey};
pub use iroha_telemetry::metrics::{Status as TelemetryStatus, TxGossipSnapshot};
pub use logs::{LifecycleEvent, LogStreamKind, PeerLogEvent, PeerLogStream};
pub use secret::SecretString;
pub use state::{
    StateCursor, StateEntry, StatePage, StateQueryError, StateQueryKind, run_state_query,
};
pub use supervisor::{
    BinaryPaths, PeerHandle, PeerState, SelectedPeerStoragePaths, Supervisor, SupervisorBuilder,
    SupervisorError, SupervisorSessionInfo, resolve_selected_peer_storage_paths,
};
#[cfg(any(test, feature = "test"))]
pub use supervisor::{
    kagami_stub_genesis_policies_from_config, sign_kagami_stub_genesis_from_config,
};
pub use torii::{
    BlockDecodeStage, BlockStream, BlockStreamDecodeError, BlockStreamEvent, BlockSummary,
    EventCategory, EventDecodeStage, EventStream, EventStreamDecodeError, EventStreamEvent,
    EventSummary, LocalMcpProbeResult, ManagedBlockStream, ManagedEventStream,
    ManagedPeerGenesisFailure, ManagedPeerGenesisReadinessError, ManagedStatusStream,
    OperatorSigningContext, ReadinessOptions, ReadinessSmokeBuildError, ReadinessSmokeOutcome,
    ReadinessSmokePlan, SmokeCommitOptions, SmokeCommitSnapshot, StatusMetrics, StatusStreamEvent,
    ToriiClient, ToriiError, ToriiErrorInfo, ToriiErrorKind, ToriiMetricsSnapshot, ToriiResult,
    ToriiStatusSnapshot, WsFrame, WsSubscription, decode_norito,
    wait_for_all_managed_peers_genesis,
};
pub use vault::{SIGNERS_FILE_NAME, SignerVault, SignerVaultError};
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supervisor_builder_uses_requested_profile() {
        let builder = SupervisorBuilder::new(ProfilePreset::FourPeerBft);
        assert_eq!(
            builder.profile(),
            &NetworkProfile {
                preset: Some(ProfilePreset::FourPeerBft),
                topology: NetworkTopology::four_peer_bft(),
                consensus_mode:
                    iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned,
            }
        );
    }
}
