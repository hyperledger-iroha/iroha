use std::{
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use iroha_core::{
    queue::Queue,
    soracloud_runtime::{
        SoracloudApartmentExecutionRequest, SoracloudApartmentExecutionResult,
        SoracloudLocalReadRequest, SoracloudLocalReadResponse,
        SoracloudOrderedMailboxExecutionRequest,
        SoracloudOrderedMailboxExecutionResult, SoracloudPrivateInferenceExecutionRequest,
        SoracloudPrivateInferenceExecutionResult, SoracloudRuntime,
        SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
        SoracloudRuntimeReadHandle, SoracloudRuntimeSnapshot,
    },
    state::State,
};
use iroha_crypto::KeyPair;
use iroha_data_model::prelude::{AccountId, ChainId};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use parking_lot::RwLock;
use tokio::task;

#[derive(Clone)]
pub(crate) struct SoracloudRuntimeManagerConfig {
    pub state_dir: PathBuf,
    pub local_peer_id: Option<String>,
}

impl SoracloudRuntimeManagerConfig {
    #[must_use]
    pub fn from_runtime_config(
        config: &iroha_config::parameters::actual::SoracloudRuntime,
    ) -> Self {
        Self {
            state_dir: config.state_dir.clone(),
            local_peer_id: None,
        }
    }

    #[must_use]
    pub fn with_local_host_identity(
        mut self,
        _validator_account_id: AccountId,
        peer_id: impl Into<String>,
    ) -> Self {
        self.local_peer_id = Some(peer_id.into());
        self
    }
}

#[derive(Clone)]
pub(crate) struct QueuedSoracloudRuntimeMutationSink;

impl QueuedSoracloudRuntimeMutationSink {
    #[must_use]
    pub(crate) fn new(
        _chain_id: Arc<ChainId>,
        _queue: Arc<Queue>,
        _state: Arc<State>,
        _authority: AccountId,
        _key_pair: KeyPair,
    ) -> Self {
        Self
    }
}

pub struct SoracloudRuntimeManager {
    config: SoracloudRuntimeManagerConfig,
}

impl SoracloudRuntimeManager {
    #[must_use]
    pub fn new(config: SoracloudRuntimeManagerConfig, _state: Arc<State>) -> Self {
        Self { config }
    }

    #[must_use]
    pub(crate) fn with_mutation_sink(self, _mutation_sink: Arc<QueuedSoracloudRuntimeMutationSink>) -> Self {
        self
    }

    #[must_use]
    pub fn with_sorafs_node(self, _sorafs_node: sorafs_node::NodeHandle) -> Self {
        self
    }

    #[must_use]
    pub fn with_sorafs_provider_cache(
        self,
        _sorafs_provider_cache: Arc<tokio::sync::RwLock<iroha_torii::sorafs::ProviderAdvertCache>>,
    ) -> Self {
        self
    }

    pub fn start(self, shutdown_signal: ShutdownSignal) -> (SoracloudRuntimeManagerHandle, Child) {
        let handle = SoracloudRuntimeManagerHandle {
            snapshot: Arc::new(RwLock::new(SoracloudRuntimeSnapshot::default())),
            state_dir: Arc::new(self.config.state_dir),
            local_peer_id: self.config.local_peer_id,
        };
        let task = task::spawn(async move {
            shutdown_signal.receive().await;
        });
        (handle, Child::new(task, OnShutdown::Wait(Duration::from_secs(1))))
    }
}

#[derive(Clone)]
pub struct SoracloudRuntimeManagerHandle {
    snapshot: Arc<RwLock<SoracloudRuntimeSnapshot>>,
    state_dir: Arc<PathBuf>,
    local_peer_id: Option<String>,
}

impl SoracloudRuntimeManagerHandle {
    #[must_use]
    pub fn snapshot(&self) -> SoracloudRuntimeSnapshot {
        self.snapshot.read().clone()
    }

    #[must_use]
    pub fn state_dir(&self) -> PathBuf {
        self.state_dir.as_ref().clone()
    }
}

fn unavailable(message: &str) -> SoracloudRuntimeExecutionError {
    SoracloudRuntimeExecutionError::new(
        SoracloudRuntimeExecutionErrorKind::Unavailable,
        message.to_owned(),
    )
}

impl SoracloudRuntimeReadHandle for SoracloudRuntimeManagerHandle {
    fn snapshot(&self) -> SoracloudRuntimeSnapshot {
        Self::snapshot(self)
    }

    fn state_dir(&self) -> PathBuf {
        Self::state_dir(self)
    }

    fn local_peer_id(&self) -> Option<String> {
        self.local_peer_id.clone()
    }
}

impl SoracloudRuntime for SoracloudRuntimeManagerHandle {
    fn execute_local_read(
        &self,
        _request: SoracloudLocalReadRequest,
    ) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
        Err(unavailable("embedded Soracloud runtime is disabled for this build"))
    }

    fn execute_ordered_mailbox(
        &self,
        _request: SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError> {
        Err(unavailable("embedded Soracloud runtime is disabled for this build"))
    }

    fn execute_apartment(
        &self,
        _request: SoracloudApartmentExecutionRequest,
    ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError> {
        Err(unavailable("embedded Soracloud runtime is disabled for this build"))
    }

    fn execute_private_inference(
        &self,
        _request: SoracloudPrivateInferenceExecutionRequest,
    ) -> Result<SoracloudPrivateInferenceExecutionResult, SoracloudRuntimeExecutionError> {
        Err(unavailable("embedded Soracloud runtime is disabled for this build"))
    }
}
