use std::{path::PathBuf, sync::Arc, time::Duration};

use iroha_core::{
    queue::Queue,
    soracloud_runtime::{
        SoracloudApartmentExecutionRequest, SoracloudApartmentExecutionResult,
        SoracloudLocalReadRequest, SoracloudLocalReadResponse,
        SoracloudOrderedMailboxExecutionRequest, SoracloudOrderedMailboxExecutionResult,
        SoracloudRuntime, SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
        SoracloudRuntimeReadHandle, SoracloudRuntimeSnapshot,
    },
    state::State,
};
use iroha_data_model::prelude::{AccountId, ChainId};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use parking_lot::RwLock;
use tokio::task;

#[derive(Clone)]
pub struct SoracloudRuntimeManagerConfig {
    pub production_mode: bool,
    pub state_dir: PathBuf,
    pub local_peer_id: Option<String>,
}

impl SoracloudRuntimeManagerConfig {
    #[must_use]
    pub fn from_runtime_config(
        config: &iroha_config::parameters::actual::SoracloudRuntime,
    ) -> Self {
        assert!(
            !config.production_mode,
            "soracloud_runtime.production_mode requires building irohad with the `embedded-soracloud-runtime` feature"
        );
        Self {
            production_mode: config.production_mode,
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
pub struct QueuedSoracloudRuntimeMutationSink;

impl QueuedSoracloudRuntimeMutationSink {
    /// Construct the inert non-production sink used by builds without the
    /// embedded Soracloud runtime.
    ///
    /// # Errors
    ///
    /// This constructor currently cannot fail. It keeps the production
    /// launcher's fallible signature so feature selection cannot bypass its
    /// signer checks.
    pub(crate) fn new(
        _chain_id: Arc<ChainId>,
        _queue: Arc<Queue>,
        _state: Arc<State>,
        _signer: Arc<dyn crate::soracloud_runtime_signer::SoracloudRuntimeMutationSignerV1>,
        _submission: iroha_config::parameters::actual::SoracloudRuntimeSubmission,
    ) -> eyre::Result<Self> {
        Ok(Self)
    }
}

pub struct SoracloudRuntimeManager {
    config: SoracloudRuntimeManagerConfig,
}

impl SoracloudRuntimeManager {
    #[must_use]
    pub fn new(config: SoracloudRuntimeManagerConfig, _state: Arc<State>) -> Self {
        assert!(
            !config.production_mode,
            "soracloud_runtime.production_mode requires building irohad with the `embedded-soracloud-runtime` feature"
        );
        Self { config }
    }

    #[must_use]
    pub(crate) fn with_mutation_sink(
        self,
        _mutation_sink: Arc<QueuedSoracloudRuntimeMutationSink>,
    ) -> Self {
        self
    }

    /// Attach the inert runtime-only HF credential boundary.
    #[must_use]
    pub(crate) fn with_hf_inference_credential_provider(
        self,
        _provider: Arc<
            dyn crate::soracloud_hf_credential::SoracloudHfInferenceCredentialProviderV1,
        >,
    ) -> Self {
        self
    }

    /// Attach the inert remote stream-token operator boundary.
    #[must_use]
    pub(crate) fn with_remote_stream_token_operator_from_config(
        self,
        _config: &iroha_config::parameters::actual::Root,
    ) -> Self {
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

    /// Start the disabled-runtime shutdown waiter.
    ///
    /// # Errors
    ///
    /// The stub currently has no fallible initialization, but returns the same
    /// result shape as the embedded runtime so launcher startup remains
    /// feature-independent and fail-closed.
    pub fn start(
        self,
        shutdown_signal: ShutdownSignal,
    ) -> eyre::Result<(SoracloudRuntimeManagerHandle, Child)> {
        let handle = SoracloudRuntimeManagerHandle {
            snapshot: Arc::new(RwLock::new(SoracloudRuntimeSnapshot::default())),
            state_dir: Arc::new(self.config.state_dir),
            local_peer_id: self.config.local_peer_id,
        };
        let task = task::spawn(async move {
            shutdown_signal.receive().await;
        });
        Ok((
            handle,
            Child::new(task, OnShutdown::Wait(Duration::from_secs(1))),
        ))
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
    fn materialization_available(&self) -> bool {
        false
    }

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
        Err(unavailable(
            "embedded Soracloud runtime is disabled for this build",
        ))
    }

    fn execute_ordered_mailbox(
        &self,
        _request: SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError> {
        Err(unavailable(
            "embedded Soracloud runtime is disabled for this build",
        ))
    }

    fn execute_apartment(
        &self,
        _request: SoracloudApartmentExecutionRequest,
    ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError> {
        Err(unavailable(
            "embedded Soracloud runtime is disabled for this build",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn production_runtime_config() -> iroha_config::parameters::actual::SoracloudRuntime {
        iroha_config::parameters::actual::SoracloudRuntime {
            production_mode: true,
            ..Default::default()
        }
    }

    #[test]
    fn stub_runtime_config_rejects_production_mode() {
        let runtime = production_runtime_config();
        let result = std::panic::catch_unwind(|| {
            let _ = SoracloudRuntimeManagerConfig::from_runtime_config(&runtime);
        });
        assert!(result.is_err());
    }

    #[test]
    fn stub_runtime_manager_rejects_production_mode() {
        let config = SoracloudRuntimeManagerConfig {
            production_mode: true,
            state_dir: PathBuf::from("runtime"),
            local_peer_id: None,
        };
        let state = {
            let kura = iroha_core::kura::Kura::blank_kura_for_testing();
            let query = iroha_core::query::store::LiveQueryStore::start_test();
            Arc::new(State::new_for_testing(
                iroha_core::state::World::new(),
                kura,
                query,
            ))
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = SoracloudRuntimeManager::new(config, state);
        }));
        assert!(result.is_err());
    }
}
