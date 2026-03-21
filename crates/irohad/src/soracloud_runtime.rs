//! Embedded Soracloud runtime-manager reconciliation for `irohad`.
//!
//! This subsystem continuously projects authoritative Soracloud world state
//! into a node-local materialization plan and now serves deterministic local
//! reads/apartment observations directly from the committed snapshot plus the
//! hydrated artifact cache. Soracloud runtime v1 is currently `Ivm`-only;
//! `NativeProcess` deployments are rejected during admission and runtime
//! activation.
//!
//! Ordered mailbox execution now runs admitted IVM bundles directly through
//! the Soracloud host surface while local reads continue to resolve from the
//! committed snapshot plus hydrated artifact cache.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use eyre::WrapErr;
use iroha_core::soracloud_runtime::{
    SoracloudApartmentExecutionRequest, SoracloudApartmentExecutionResult,
    SoracloudLocalReadRequest, SoracloudLocalReadResponse,
    SoracloudOrderedMailboxExecutionRequest, SoracloudOrderedMailboxExecutionResult,
    SoracloudRuntime, SoracloudRuntimeApartmentPlan, SoracloudRuntimeArtifactPlan,
    SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
    SoracloudRuntimeMailboxPlan, SoracloudRuntimeReadHandle, SoracloudRuntimeRevisionRole,
    SoracloudRuntimeServicePlan, SoracloudRuntimeSnapshot,
};
use iroha_core::state::{State, StateView, WorldReadOnly};
use iroha_crypto::Hash;
use iroha_data_model::{
    Encode,
    name::Name,
    soracloud::{
        SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1, SORA_RUNTIME_RECEIPT_VERSION_V1,
        SORACLOUD_HOST_RESPONSE_VERSION_V1, SoraAgentApartmentRecordV1, SoraArtifactKindV1,
        SoraCapabilityPolicyV1, SoraCertifiedResponsePolicyV1, SoraDeploymentBundleV1,
        SoraNetworkPolicyV1, SoraRuntimeReceiptV1, SoraServiceDeploymentStateV1,
        SoraServiceHandlerClassV1, SoraServiceHandlerV1, SoraServiceHealthStatusV1,
        SoraServiceLifecycleActionV1, SoraServiceMailboxMessageV1, SoraServiceRuntimeStateV1,
        SoraServiceStateEntryV1, SoraStateBindingV1,
        SoraStateMutationOperationV1, SoracloudAppendJournalResponseV1,
        SoracloudEgressFetchRequestV1, SoracloudEgressFetchResponseV1,
        SoracloudEmitMailboxMessageRequestV1, SoracloudEmitMailboxMessageResponseV1,
        SoracloudEmitStateMutationRequestV1, SoracloudEmitStateMutationResponseV1,
        SoracloudHostOperationV1, SoracloudHostRequestEnvelopeV1,
        SoracloudHostRequestPayloadV1, SoracloudHostResponseEnvelopeV1,
        SoracloudHostResponsePayloadV1, SoracloudPublishCheckpointResponseV1,
        SoracloudReadCommittedStateResponseV1, SoracloudReadCredentialResponseV1,
        SoracloudReadSecretResponseV1,
    },
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use ivm::{
    IVM, IVMHost, PointerType, VMError, verify_contract_artifact,
    syscalls::{
        SYSCALL_SORACLOUD_APPEND_JOURNAL, SYSCALL_SORACLOUD_EGRESS_FETCH,
        SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE, SYSCALL_SORACLOUD_EMIT_STATE_MUTATION,
        SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT, SYSCALL_SORACLOUD_READ_COMMITTED_STATE,
        SYSCALL_SORACLOUD_READ_CREDENTIAL, SYSCALL_SORACLOUD_READ_SECRET,
    },
};
use mv::storage::StorageReadOnly;
use parking_lot::RwLock;
use tokio::task::JoinHandle;

/// Runtime-manager configuration derived from the explicit Soracloud runtime settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SoracloudRuntimeManagerConfig {
    /// Root directory for local runtime materialization state.
    pub state_dir: PathBuf,
    /// Reconciliation cadence against authoritative state.
    pub reconcile_interval: Duration,
    /// Reserved concurrency budget for future hydration workers.
    pub hydration_concurrency: NonZeroUsize,
    /// Configured artifact-cache budgets for the embedded runtime manager.
    pub cache_budgets: iroha_config::parameters::actual::SoracloudRuntimeCacheBudgets,
    /// Deterministic `NativeProcess` hosting limits.
    pub native_process: iroha_config::parameters::actual::SoracloudRuntimeNativeProcess,
    /// Outbound egress policy for embedded runtimes.
    pub egress: iroha_config::parameters::actual::SoracloudRuntimeEgress,
}

impl SoracloudRuntimeManagerConfig {
    /// Build a runtime-manager configuration from the parsed Soracloud runtime settings.
    #[must_use]
    pub fn from_runtime_config(config: &iroha_config::parameters::actual::SoracloudRuntime) -> Self {
        Self {
            state_dir: config.state_dir.clone(),
            reconcile_interval: config.reconcile_interval,
            hydration_concurrency: config.hydration_concurrency,
            cache_budgets: config.cache_budgets.clone(),
            native_process: config.native_process.clone(),
            egress: config.egress.clone(),
        }
    }
}

/// Executable handle to the embedded Soracloud runtime manager.
#[derive(Clone)]
pub struct SoracloudRuntimeManagerHandle {
    snapshot: Arc<RwLock<SoracloudRuntimeSnapshot>>,
    config: Arc<SoracloudRuntimeManagerConfig>,
    state_dir: Arc<PathBuf>,
    state: Arc<State>,
}

impl SoracloudRuntimeManagerHandle {
    /// Return the latest materialization snapshot.
    #[must_use]
    pub fn snapshot(&self) -> SoracloudRuntimeSnapshot {
        self.snapshot.read().clone()
    }

    /// Return the runtime-manager state directory.
    #[must_use]
    pub fn state_dir(&self) -> PathBuf {
        self.state_dir.as_ref().clone()
    }
}

impl SoracloudRuntimeReadHandle for SoracloudRuntimeManagerHandle {
    fn snapshot(&self) -> SoracloudRuntimeSnapshot {
        SoracloudRuntimeManagerHandle::snapshot(self)
    }

    fn state_dir(&self) -> PathBuf {
        SoracloudRuntimeManagerHandle::state_dir(self)
    }
}

impl SoracloudRuntime for SoracloudRuntimeManagerHandle {
    fn execute_local_read(
        &self,
        request: SoracloudLocalReadRequest,
    ) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
        let view = self.state.view();
        let snapshot = self.snapshot();
        validate_local_runtime_snapshot(&view, &snapshot, &request)?;
        let context = resolve_local_read_context(&view, &request)?;

        match request.handler_class {
            iroha_core::soracloud_runtime::SoracloudLocalReadKind::Asset => {
                execute_asset_local_read(&request, &context, self.state_dir.as_ref())
            }
            iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query => {
                execute_query_local_read(&view, &request, &context)
            }
        }
    }

    fn execute_ordered_mailbox(
        &self,
        request: SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError> {
        if request.handler.is_none() {
            return Ok(deterministic_mailbox_failure_result(
                request,
                "missing_handler",
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
        if let Err(message) = ensure_ivm_runtime(
            request.bundle.container.runtime,
            request.deployment.service_name.as_ref(),
            &request.deployment.current_service_version,
        ) {
            return Ok(deterministic_mailbox_failure_result_with_message(
                request,
                "invalid_runtime",
                message,
                SoraServiceHealthStatusV1::Degraded,
            ));
        }

        let bundle_cache_path = self
            .state_dir
            .join("artifacts")
            .join(hash_cache_name(request.bundle.container.bundle_hash));
        let bundle_bytes = match read_and_verify_cached_artifact(
            &bundle_cache_path,
            request.bundle.container.bundle_hash,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result_with_message(
                    request,
                    "bundle_unavailable",
                    error.message,
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        };

        let verified = match verify_contract_artifact(&bundle_bytes) {
            Ok(verified) => verified,
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result_with_message(
                    request,
                    "invalid_bundle",
                    error.to_string(),
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        };
        let Some(entrypoint) = verified
            .contract_interface
            .entrypoints
            .iter()
            .find(|entrypoint| {
                request
                    .handler
                    .as_ref()
                    .is_some_and(|handler| entrypoint.name == handler.entrypoint)
            })
        else {
            return Ok(deterministic_mailbox_failure_result(
                request,
                "missing_entrypoint",
                SoraServiceHealthStatusV1::Degraded,
            ));
        };

        let committed_entries = collect_committed_service_state_entries(
            &self.state.view(),
            request.deployment.service_name.as_ref(),
        );
        let host = SoracloudIvmHost::new(
            request.clone(),
            self.state_dir(),
            self.config.egress.clone(),
            committed_entries,
        );
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);
        if let Err(error) = vm.load_program(&bundle_bytes) {
            return Ok(deterministic_mailbox_failure_result(
                request,
                vm_error_label(&error),
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
        let entry_pc = u64::try_from(verified.code_offset.saturating_sub(verified.header_len))
            .unwrap_or(u64::MAX)
            .saturating_add(entrypoint.entry_pc);
        if let Err(error) = vm.set_program_counter(entry_pc) {
            return Ok(deterministic_mailbox_failure_result(
                request,
                vm_error_label(&error),
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
        match mailbox_payload_tlv_bytes(&request.mailbox_message.payload_bytes) {
            Ok(tlv_bytes) => match vm.alloc_input_tlv(&tlv_bytes) {
                Ok(ptr) => vm.set_register(10, ptr),
                Err(error) => {
                    return Ok(deterministic_mailbox_failure_result(
                        request,
                        vm_error_label(&error),
                        SoraServiceHealthStatusV1::Degraded,
                    ));
                }
            },
            Err(error) => {
                return Ok(deterministic_mailbox_failure_result(
                    request,
                    vm_error_label(&error),
                    SoraServiceHealthStatusV1::Degraded,
                ));
            }
        }
        vm.set_register(11, request.execution_sequence);
        vm.set_register(12, request.observed_height);
        if let Err(error) = vm.run() {
            return Ok(deterministic_mailbox_failure_result(
                request,
                vm_error_label(&error),
                SoraServiceHealthStatusV1::Degraded,
            ));
        }
        let Some(host) = vm.host_mut_any().and_then(|host| host.downcast_mut::<SoracloudIvmHost>())
        else {
            return Ok(deterministic_mailbox_failure_result(
                request,
                "host_unavailable",
                SoraServiceHealthStatusV1::Degraded,
            ));
        };
        match std::mem::replace(
            host,
            SoracloudIvmHost::new(
                request.clone(),
                self.state_dir(),
                self.config.egress.clone(),
                BTreeMap::new(),
            ),
        )
        .into_execution_result()
        {
            Ok(result) => Ok(result),
            Err(error) => Ok(deterministic_mailbox_failure_result_with_message(
                request,
                "materialization_failure",
                error.message,
                SoraServiceHealthStatusV1::Degraded,
            )),
        }
    }

    fn execute_apartment(
        &self,
        request: SoracloudApartmentExecutionRequest,
    ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError> {
        let view = self.state.view();
        let snapshot = self.snapshot();
        validate_apartment_snapshot(&view, &snapshot, &request)?;
        let Some(record) = view
            .world()
            .soracloud_agent_apartments()
            .get(&request.apartment_name)
        else {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!("unknown Soracloud apartment `{}`", request.apartment_name),
            ));
        };
        if record.process_generation != request.process_generation {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                format!(
                    "apartment `{}` process generation {} does not match committed generation {}",
                    request.apartment_name, request.process_generation, record.process_generation
                ),
            ));
        }

        Ok(SoracloudApartmentExecutionResult {
            status: record.status,
            checkpoint_artifact_hash: None,
            journal_artifact_hash: None,
            result_commitment: apartment_result_commitment(
                &request.apartment_name,
                request.process_generation,
                &request.operation,
                request.request_commitment,
                record.status,
            ),
        })
    }
}

#[derive(Clone, Debug)]
struct StagedRuntimeArtifact {
    artifact_path: String,
    bytes: Vec<u8>,
    artifact_hash: Hash,
}

struct SoracloudIvmHost {
    request: SoracloudOrderedMailboxExecutionRequest,
    state_dir: PathBuf,
    egress: iroha_config::parameters::actual::SoracloudRuntimeEgress,
    committed_entries: BTreeMap<(String, String), SoraServiceStateEntryV1>,
    binding_totals: BTreeMap<String, u64>,
    staged_state_mutations: Vec<iroha_core::soracloud_runtime::SoracloudDeterministicStateMutation>,
    staged_outbound_mailbox_messages: Vec<SoraServiceMailboxMessageV1>,
    staged_journal: Option<StagedRuntimeArtifact>,
    staged_checkpoint: Option<StagedRuntimeArtifact>,
    egress_requests: u32,
    egress_bytes: u64,
}

impl SoracloudIvmHost {
    fn new(
        request: SoracloudOrderedMailboxExecutionRequest,
        state_dir: PathBuf,
        egress: iroha_config::parameters::actual::SoracloudRuntimeEgress,
        committed_entries: BTreeMap<(String, String), SoraServiceStateEntryV1>,
    ) -> Self {
        let mut binding_totals = BTreeMap::new();
        for entry in committed_entries.values() {
            let total = binding_totals
                .entry(entry.binding_name.to_string())
                .or_insert(0u64);
            *total = total.saturating_add(entry.payload_bytes.get());
        }
        Self {
            request,
            state_dir,
            egress,
            committed_entries,
            binding_totals,
            staged_state_mutations: Vec::new(),
            staged_outbound_mailbox_messages: Vec::new(),
            staged_journal: None,
            staged_checkpoint: None,
            egress_requests: 0,
            egress_bytes: 0,
        }
    }

    fn handler_class(&self) -> SoraServiceHandlerClassV1 {
        self.request
            .handler
            .as_ref()
            .map(|handler| handler.class)
            .unwrap_or(SoraServiceHandlerClassV1::Update)
    }

    fn service_name(&self) -> &Name {
        &self.request.deployment.service_name
    }

    fn service_version(&self) -> &str {
        &self.request.deployment.current_service_version
    }

    fn require_private_runtime(&self, syscall: u32) -> Result<(), VMError> {
        if self.handler_class() == SoraServiceHandlerClassV1::PrivateUpdate {
            Ok(())
        } else {
            Err(VMError::NotImplemented { syscall })
        }
    }

    fn read_request_payload(
        &self,
        vm: &mut IVM,
        expected_operation: SoracloudHostOperationV1,
        syscall: u32,
    ) -> Result<SoracloudHostRequestPayloadV1, VMError> {
        let tlv = vm.memory.validate_tlv(vm.register(10))?;
        if tlv.type_id != PointerType::SoracloudRequest {
            return Err(VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: tlv.type_id_raw(),
            });
        }
        let envelope = norito::decode_from_bytes::<SoracloudHostRequestEnvelopeV1>(tlv.payload)
            .map_err(|_| VMError::NoritoInvalid)?;
        envelope.validate().map_err(|_| VMError::NoritoInvalid)?;
        if envelope.operation != expected_operation {
            return Err(VMError::NotImplemented { syscall });
        }
        Ok(envelope.payload)
    }

    fn write_response(
        &self,
        vm: &mut IVM,
        operation: SoracloudHostOperationV1,
        payload: SoracloudHostResponsePayloadV1,
    ) -> Result<(), VMError> {
        let envelope = SoracloudHostResponseEnvelopeV1 {
            schema_version: SORACLOUD_HOST_RESPONSE_VERSION_V1,
            operation,
            payload,
        };
        envelope.validate().map_err(|_| VMError::NoritoInvalid)?;
        let payload_bytes = norito::to_bytes(&envelope).map_err(|_| VMError::NoritoInvalid)?;
        let tlv = make_pointer_tlv(PointerType::SoracloudResponse, &payload_bytes);
        let ptr = vm.alloc_input_tlv(&tlv)?;
        vm.set_register(10, ptr);
        Ok(())
    }

    fn binding(&self, binding_name: &Name) -> Result<&SoraStateBindingV1, VMError> {
        self.request
            .bundle
            .service
            .state_bindings
            .iter()
            .find(|binding| binding.binding_name == *binding_name)
            .ok_or(VMError::PermissionDenied)
    }

    fn state_entry_key(binding_name: &Name, state_key: &str) -> (String, String) {
        (binding_name.to_string(), state_key.to_owned())
    }

    fn current_entry_size(&self, binding_name: &Name, state_key: &str) -> u64 {
        let key = Self::state_entry_key(binding_name, state_key);
        self.committed_entries
            .get(&key)
            .map(|entry| entry.payload_bytes.get())
            .unwrap_or(0)
    }

    fn stage_state_mutation(
        &mut self,
        request: SoracloudEmitStateMutationRequestV1,
    ) -> Result<SoracloudEmitStateMutationResponseV1, VMError> {
        if !self.request.bundle.container.capabilities.allow_state_writes {
            return Err(VMError::PermissionDenied);
        }
        if request.state_key.trim().is_empty() || !request.state_key.starts_with('/') {
            return Err(VMError::PermissionDenied);
        }
        let binding = self.binding(&request.binding_name)?.clone();
        if !request.state_key.starts_with(&binding.key_prefix) {
            return Err(VMError::PermissionDenied);
        }
        if binding.encryption != request.encryption {
            return Err(VMError::PermissionDenied);
        }
        let binding_name = request.binding_name.to_string();
        let current_size = self.current_entry_size(&request.binding_name, &request.state_key);
        match request.operation {
            SoraStateMutationOperationV1::Upsert => {
                let Some(payload_bytes) = request.payload_bytes else {
                    return Err(VMError::NoritoInvalid);
                };
                if payload_bytes == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let Some(payload_commitment) = request.payload_commitment else {
                    return Err(VMError::NoritoInvalid);
                };
                if payload_bytes > binding.max_item_bytes.get() {
                    return Err(VMError::PermissionDenied);
                }
                if !matches!(binding.mutability, iroha_data_model::soracloud::SoraStateMutabilityV1::AppendOnly | iroha_data_model::soracloud::SoraStateMutabilityV1::ReadWrite) {
                    return Err(VMError::PermissionDenied);
                }
                if binding.mutability
                    == iroha_data_model::soracloud::SoraStateMutabilityV1::AppendOnly
                    && current_size > 0
                {
                    return Err(VMError::PermissionDenied);
                }
                let current_total = self.binding_totals.get(&binding_name).copied().unwrap_or(0);
                let next_total = current_total
                    .saturating_sub(current_size)
                    .saturating_add(payload_bytes);
                if next_total > binding.max_total_bytes.get() {
                    return Err(VMError::PermissionDenied);
                }
                self.binding_totals.insert(binding_name.clone(), next_total);
                self.committed_entries.insert(
                    Self::state_entry_key(&request.binding_name, &request.state_key),
                    SoraServiceStateEntryV1 {
                        schema_version: iroha_data_model::soracloud::SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                        service_name: self.request.deployment.service_name.clone(),
                        service_version: self.request.deployment.current_service_version.clone(),
                        binding_name: request.binding_name.clone(),
                        state_key: request.state_key.clone(),
                        encryption: request.encryption,
                        payload_bytes: std::num::NonZeroU64::new(payload_bytes)
                            .ok_or(VMError::NoritoInvalid)?,
                        payload_commitment,
                        last_update_sequence: self.request.execution_sequence,
                        governance_tx_hash: self.request.mailbox_message.payload_commitment,
                        source_action: SoraServiceLifecycleActionV1::StateMutation,
                    },
                );
            }
            SoraStateMutationOperationV1::Delete => {
                if request.payload_bytes.is_some() || request.payload_commitment.is_some() {
                    return Err(VMError::NoritoInvalid);
                }
                if binding.mutability != iroha_data_model::soracloud::SoraStateMutabilityV1::ReadWrite {
                    return Err(VMError::PermissionDenied);
                }
                let current_total = self.binding_totals.get(&binding_name).copied().unwrap_or(0);
                self.binding_totals.insert(
                    binding_name.clone(),
                    current_total.saturating_sub(current_size),
                );
                self.committed_entries
                    .remove(&Self::state_entry_key(&request.binding_name, &request.state_key));
            }
        }

        let mutation = iroha_core::soracloud_runtime::SoracloudDeterministicStateMutation {
            binding_name,
            state_key: request.state_key.clone(),
            operation: request.operation,
            encryption: request.encryption,
            payload_bytes: request.payload_bytes,
            payload_commitment: request.payload_commitment,
        };
        let mutation_commitment = Hash::new(Encode::encode(&(
            "soracloud.host.state-mutation.v1",
            self.request.mailbox_message.message_id,
            mutation.binding_name.as_str(),
            mutation.state_key.as_str(),
            mutation.operation,
            mutation.encryption,
            mutation.payload_bytes,
            mutation.payload_commitment,
            u64::try_from(self.staged_state_mutations.len()).unwrap_or(u64::MAX),
        )));
        self.staged_state_mutations.push(mutation);
        Ok(SoracloudEmitStateMutationResponseV1 { mutation_commitment })
    }

    fn stage_outbound_mailbox_message(
        &mut self,
        request: SoracloudEmitMailboxMessageRequestV1,
    ) -> SoracloudEmitMailboxMessageResponseV1 {
        let payload_commitment = Hash::new(&request.payload_bytes);
        let message_id = Hash::new(Encode::encode(&(
            "soracloud.host.mailbox.v1",
            self.request.mailbox_message.message_id,
            self.request.deployment.service_name.as_ref(),
            self.request.mailbox_message.to_handler.as_ref(),
            request.to_service.as_ref(),
            request.to_handler.as_ref(),
            payload_commitment,
            request.available_after_sequence,
            request.expires_at_sequence,
            u64::try_from(self.staged_outbound_mailbox_messages.len()).unwrap_or(u64::MAX),
        )));
        self.staged_outbound_mailbox_messages.push(SoraServiceMailboxMessageV1 {
            schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id,
            from_service: self.request.deployment.service_name.clone(),
            from_handler: self.request.mailbox_message.to_handler.clone(),
            to_service: request.to_service,
            to_handler: request.to_handler,
            payload_bytes: request.payload_bytes,
            payload_commitment,
            enqueue_sequence: self.request.execution_sequence,
            available_after_sequence: request
                .available_after_sequence
                .max(self.request.execution_sequence),
            expires_at_sequence: request.expires_at_sequence,
        });
        SoracloudEmitMailboxMessageResponseV1 {
            message_id,
            payload_commitment,
        }
    }

    fn stage_artifact(
        slot: &mut Option<StagedRuntimeArtifact>,
        request: String,
        bytes: Vec<u8>,
    ) -> Hash {
        let artifact_hash = Hash::new(&bytes);
        *slot = Some(StagedRuntimeArtifact {
            artifact_path: request,
            bytes,
            artifact_hash,
        });
        artifact_hash
    }

    fn read_material(
        &self,
        root_name: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, VMError> {
        let relative = sanitized_relative_material_path(key)?;
        let path = self
            .state_dir
            .join(root_name)
            .join(sanitize_path_component(self.service_name().as_ref()))
            .join(sanitize_path_component(self.service_version()))
            .join(relative);
        match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(VMError::PermissionDenied),
        }
    }

    fn host_network_allows(&self, host: &str) -> bool {
        let container_policy: &SoraCapabilityPolicyV1 = &self.request.bundle.container.capabilities;
        let container_allows = match &container_policy.network {
            SoraNetworkPolicyV1::Isolated => false,
            SoraNetworkPolicyV1::Allowlist(hosts) => hosts.iter().any(|allowed| allowed == host),
        };
        if !container_allows {
            return false;
        }
        if self.egress.default_allow {
            true
        } else {
            self.egress.allowed_hosts.iter().any(|allowed| allowed == host)
        }
    }

    fn egress_fetch(
        &mut self,
        request: SoracloudEgressFetchRequestV1,
    ) -> Result<SoracloudEgressFetchResponseV1, VMError> {
        self.require_private_runtime(SYSCALL_SORACLOUD_EGRESS_FETCH)?;
        let Some(expected_hash) = request.expected_hash else {
            return Err(VMError::PermissionDenied);
        };
        let Some(host) = url_host(&request.url) else {
            return Err(VMError::PermissionDenied);
        };
        if !self.host_network_allows(host) {
            return Err(VMError::PermissionDenied);
        }
        let max_requests = self.egress.rate_per_minute.map(|value| value.get()).unwrap_or(u32::MAX);
        if self.egress_requests >= max_requests {
            return Err(VMError::PermissionDenied);
        }
        let remaining_budget = self
            .egress
            .max_bytes_per_minute
            .map(|value| value.get())
            .unwrap_or(u64::MAX)
            .saturating_sub(self.egress_bytes);
        let response_cap = remaining_budget.min(request.max_bytes);
        if response_cap == 0 {
            return Err(VMError::PermissionDenied);
        }
        let response = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| VMError::PermissionDenied)?
            .get(&request.url)
            .send()
            .map_err(|_| VMError::PermissionDenied)?;
        let status_code = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let body = response
            .bytes()
            .map_err(|_| VMError::PermissionDenied)?
            .to_vec();
        if u64::try_from(body.len()).unwrap_or(u64::MAX) > response_cap {
            return Err(VMError::PermissionDenied);
        }
        let body_hash = Hash::new(&body);
        if body_hash != expected_hash {
            return Err(VMError::PermissionDenied);
        }
        self.egress_requests = self.egress_requests.saturating_add(1);
        self.egress_bytes = self
            .egress_bytes
            .saturating_add(u64::try_from(body.len()).unwrap_or(u64::MAX));
        Ok(SoracloudEgressFetchResponseV1 {
            status_code,
            content_type,
            body,
            body_hash,
        })
    }

    fn into_execution_result(
        self,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError> {
        let handler_class = self.handler_class();
        let journal_artifact_hash = persist_staged_runtime_artifact(
            self.state_dir.join("journals"),
            self.staged_journal.as_ref(),
        )?;
        let checkpoint_artifact_hash = persist_staged_runtime_artifact(
            self.state_dir.join("checkpoints"),
            self.staged_checkpoint.as_ref(),
        )?;
        let runtime_state = updated_runtime_state_with_outbound_mailbox(
            self.request.runtime_state.clone(),
            &self.request,
            SoraServiceHealthStatusV1::Healthy,
            &self.staged_outbound_mailbox_messages,
        );
        let result_commitment = authoritative_mailbox_result_commitment(
            &self.request,
            &self.staged_state_mutations,
            &self.staged_outbound_mailbox_messages,
            &runtime_state,
            journal_artifact_hash,
            checkpoint_artifact_hash,
        );
        let receipt_id = Hash::new(Encode::encode(&(
            "soracloud:runtime-receipt:v1",
            self.request.mailbox_message.message_id,
            self.request.deployment.service_name.as_ref(),
            self.request.deployment.current_service_version.as_str(),
            self.request.mailbox_message.to_handler.as_ref(),
            self.request.execution_sequence,
            result_commitment,
        )));
        Ok(SoracloudOrderedMailboxExecutionResult {
            state_mutations: self.staged_state_mutations,
            outbound_mailbox_messages: self.staged_outbound_mailbox_messages,
            runtime_state: Some(runtime_state),
            runtime_receipt: SoraRuntimeReceiptV1 {
                schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id,
                service_name: self.request.deployment.service_name,
                service_version: self.request.deployment.current_service_version,
                handler_name: self.request.mailbox_message.to_handler.clone(),
                handler_class,
                request_commitment: self.request.mailbox_message.payload_commitment,
                result_commitment,
                certified_by: SoraCertifiedResponsePolicyV1::None,
                emitted_sequence: self.request.execution_sequence,
                mailbox_message_id: Some(self.request.mailbox_message.message_id),
                journal_artifact_hash,
                checkpoint_artifact_hash,
            },
        })
    }
}

impl IVMHost for SoracloudIvmHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        match number {
            SYSCALL_SORACLOUD_READ_COMMITTED_STATE => {
                let SoracloudHostRequestPayloadV1::ReadCommittedState(request) = self
                    .read_request_payload(
                        vm,
                        SoracloudHostOperationV1::ReadCommittedState,
                        number,
                    )?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let entry = self
                    .committed_entries
                    .get(&Self::state_entry_key(&request.binding_name, &request.state_key))
                    .cloned();
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadCommittedState,
                    SoracloudHostResponsePayloadV1::ReadCommittedState(
                        SoracloudReadCommittedStateResponseV1 { entry },
                    ),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_EMIT_STATE_MUTATION => {
                let SoracloudHostRequestPayloadV1::EmitStateMutation(request) = self
                    .read_request_payload(
                        vm,
                        SoracloudHostOperationV1::EmitStateMutation,
                        number,
                    )?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let response = self.stage_state_mutation(request)?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EmitStateMutation,
                    SoracloudHostResponsePayloadV1::EmitStateMutation(response),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE => {
                let SoracloudHostRequestPayloadV1::EmitMailboxMessage(request) = self
                    .read_request_payload(
                        vm,
                        SoracloudHostOperationV1::EmitMailboxMessage,
                        number,
                    )?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let response = self.stage_outbound_mailbox_message(request);
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EmitMailboxMessage,
                    SoracloudHostResponsePayloadV1::EmitMailboxMessage(response),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_APPEND_JOURNAL => {
                let SoracloudHostRequestPayloadV1::AppendJournal(request) = self
                    .read_request_payload(vm, SoracloudHostOperationV1::AppendJournal, number)?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let artifact_hash = Self::stage_artifact(
                    &mut self.staged_journal,
                    request.artifact_path,
                    request.payload_bytes,
                );
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::AppendJournal,
                    SoracloudHostResponsePayloadV1::AppendJournal(
                        SoracloudAppendJournalResponseV1 { artifact_hash },
                    ),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT => {
                let SoracloudHostRequestPayloadV1::PublishCheckpoint(request) = self
                    .read_request_payload(
                        vm,
                        SoracloudHostOperationV1::PublishCheckpoint,
                        number,
                    )?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let artifact_hash = Self::stage_artifact(
                    &mut self.staged_checkpoint,
                    request.artifact_path,
                    request.payload_bytes,
                );
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::PublishCheckpoint,
                    SoracloudHostResponsePayloadV1::PublishCheckpoint(
                        SoracloudPublishCheckpointResponseV1 { artifact_hash },
                    ),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_READ_SECRET => {
                self.require_private_runtime(number)?;
                let SoracloudHostRequestPayloadV1::ReadSecret(request) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::ReadSecret, number)?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let payload_bytes = self.read_material("secrets", &request.secret_name)?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadSecret,
                    SoracloudHostResponsePayloadV1::ReadSecret(SoracloudReadSecretResponseV1 {
                        found: payload_bytes.is_some(),
                        payload_bytes: payload_bytes.unwrap_or_default(),
                    }),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_READ_CREDENTIAL => {
                self.require_private_runtime(number)?;
                let SoracloudHostRequestPayloadV1::ReadCredential(request) = self
                    .read_request_payload(vm, SoracloudHostOperationV1::ReadCredential, number)?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let payload_bytes = self.read_material("credentials", &request.credential_name)?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::ReadCredential,
                    SoracloudHostResponsePayloadV1::ReadCredential(
                        SoracloudReadCredentialResponseV1 {
                            found: payload_bytes.is_some(),
                            payload_bytes: payload_bytes.unwrap_or_default(),
                        },
                    ),
                )?;
                Ok(0)
            }
            SYSCALL_SORACLOUD_EGRESS_FETCH => {
                let SoracloudHostRequestPayloadV1::EgressFetch(request) =
                    self.read_request_payload(vm, SoracloudHostOperationV1::EgressFetch, number)?
                else {
                    return Err(VMError::NoritoInvalid);
                };
                let response = self.egress_fetch(request)?;
                self.write_response(
                    vm,
                    SoracloudHostOperationV1::EgressFetch,
                    SoracloudHostResponsePayloadV1::EgressFetch(response),
                )?;
                Ok(0)
            }
            _ => Err(VMError::UnknownSyscall(number)),
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any
    where
        Self: 'static,
    {
        self
    }
}

#[derive(Clone)]
struct ResolvedLocalReadContext {
    deployment: SoraServiceDeploymentStateV1,
    bundle: SoraDeploymentBundleV1,
    handler: SoraServiceHandlerV1,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize
)]
struct LocalQueryResponse {
    schema_version: u16,
    service_name: String,
    service_version: String,
    handler_name: String,
    observed_height: u64,
    observed_block_hash: Option<String>,
    entries: Vec<LocalQueryEntry>,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize
)]
struct LocalQueryEntry {
    binding_name: String,
    state_key: String,
    service_version: String,
    encryption: iroha_data_model::soracloud::SoraStateEncryptionV1,
    payload_bytes: u64,
    payload_commitment: Hash,
    last_update_sequence: u64,
    governance_tx_hash: Hash,
    source_action: SoraServiceLifecycleActionV1,
}

/// Embedded `irohad` Soracloud runtime-manager actor.
pub(crate) struct SoracloudRuntimeManager {
    config: SoracloudRuntimeManagerConfig,
    state: Arc<State>,
    snapshot: Arc<RwLock<SoracloudRuntimeSnapshot>>,
}

impl SoracloudRuntimeManager {
    /// Construct the runtime manager for the supplied node state.
    #[must_use]
    pub fn new(config: SoracloudRuntimeManagerConfig, state: Arc<State>) -> Self {
        Self {
            config,
            state,
            snapshot: Arc::new(RwLock::new(SoracloudRuntimeSnapshot::default())),
        }
    }

    /// Start the background reconciliation loop.
    pub fn start(self, shutdown_signal: ShutdownSignal) -> (SoracloudRuntimeManagerHandle, Child) {
        let manager = Arc::new(self);
        if let Err(error) = manager.restore_persisted_snapshot() {
            iroha_logger::warn!(
                ?error,
                state_dir = %manager.config.state_dir.display(),
                "failed to restore persisted Soracloud runtime-manager snapshot"
            );
        }
        if let Err(error) = manager.reconcile_once() {
            iroha_logger::warn!(
                ?error,
                state_dir = %manager.config.state_dir.display(),
                "initial Soracloud runtime-manager reconciliation failed"
            );
        }
        let handle = SoracloudRuntimeManagerHandle {
            snapshot: Arc::clone(&manager.snapshot),
            config: Arc::new(manager.config.clone()),
            state_dir: Arc::new(manager.config.state_dir.clone()),
            state: Arc::clone(&manager.state),
        };
        let task = Arc::clone(&manager).spawn_reconcile_task(shutdown_signal);
        (
            handle,
            Child::new(task, OnShutdown::Wait(Duration::from_secs(1))),
        )
    }

    fn spawn_reconcile_task(self: Arc<Self>, shutdown_signal: ShutdownSignal) -> JoinHandle<()> {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(self.config.reconcile_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(error) = self.reconcile_once() {
                            iroha_logger::warn!(
                                ?error,
                                state_dir = %self.config.state_dir.display(),
                                "Soracloud runtime-manager reconciliation failed"
                            );
                        }
                    }
                    () = shutdown_signal.receive() => {
                        iroha_logger::debug!("Soracloud runtime manager is being shut down.");
                        break;
                    }
                    else => break,
                }
            }
        })
    }

    /// Reconcile the node-local materialization plan against authoritative state once.
    pub(crate) fn reconcile_once(&self) -> eyre::Result<()> {
        fs::create_dir_all(self.services_root())
            .wrap_err_with(|| format!("create {}", self.services_root().display()))?;
        fs::create_dir_all(self.apartments_root())
            .wrap_err_with(|| format!("create {}", self.apartments_root().display()))?;
        fs::create_dir_all(self.artifacts_root())
            .wrap_err_with(|| format!("create {}", self.artifacts_root().display()))?;
        fs::create_dir_all(self.journals_root())
            .wrap_err_with(|| format!("create {}", self.journals_root().display()))?;
        fs::create_dir_all(self.checkpoints_root())
            .wrap_err_with(|| format!("create {}", self.checkpoints_root().display()))?;
        fs::create_dir_all(self.secrets_root())
            .wrap_err_with(|| format!("create {}", self.secrets_root().display()))?;
        fs::create_dir_all(self.credentials_root())
            .wrap_err_with(|| format!("create {}", self.credentials_root().display()))?;

        let view = self.state.view();
        let bundle_registry = collect_service_revision_registry(&view);
        let snapshot = build_runtime_snapshot(
            &view,
            &bundle_registry,
            &self.config.state_dir,
            self.artifacts_root(),
        )?;

        self.write_service_materializations(&snapshot, &bundle_registry)?;
        self.write_apartment_materializations(&snapshot, &view)?;
        self.prune_stale_service_materializations(&snapshot)?;
        self.prune_stale_apartment_materializations(&snapshot)?;
        write_json_atomic(
            &self.config.state_dir.join("runtime_snapshot.json"),
            &snapshot,
        )?;
        *self.snapshot.write() = snapshot;
        Ok(())
    }

    fn runtime_snapshot_path(&self) -> PathBuf {
        self.config.state_dir.join("runtime_snapshot.json")
    }

    fn restore_persisted_snapshot(&self) -> eyre::Result<bool> {
        let path = self.runtime_snapshot_path();
        let Some(snapshot) = read_json_optional::<SoracloudRuntimeSnapshot>(&path)
            .wrap_err_with(|| format!("read {}", path.display()))?
        else {
            return Ok(false);
        };
        *self.snapshot.write() = snapshot;
        Ok(true)
    }

    fn services_root(&self) -> PathBuf {
        self.config.state_dir.join("services")
    }

    fn apartments_root(&self) -> PathBuf {
        self.config.state_dir.join("apartments")
    }

    fn artifacts_root(&self) -> PathBuf {
        self.config.state_dir.join("artifacts")
    }

    fn journals_root(&self) -> PathBuf {
        self.config.state_dir.join("journals")
    }

    fn checkpoints_root(&self) -> PathBuf {
        self.config.state_dir.join("checkpoints")
    }

    fn secrets_root(&self) -> PathBuf {
        self.config.state_dir.join("secrets")
    }

    fn credentials_root(&self) -> PathBuf {
        self.config.state_dir.join("credentials")
    }

    fn write_service_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
        bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    ) -> eyre::Result<()> {
        for (service_name, versions) in &snapshot.services {
            let service_dir_name = sanitize_path_component(service_name);
            let service_root = self.services_root().join(service_dir_name);
            fs::create_dir_all(&service_root)
                .wrap_err_with(|| format!("create {}", service_root.display()))?;
            for (service_version, plan) in versions {
                let version_dir = service_root.join(sanitize_path_component(service_version));
                fs::create_dir_all(&version_dir)
                    .wrap_err_with(|| format!("create {}", version_dir.display()))?;
                write_json_atomic(&version_dir.join("runtime_plan.json"), plan)?;
                if let Some(bundle) =
                    bundle_registry.get(&(service_name.clone(), service_version.clone()))
                {
                    write_json_atomic(&version_dir.join("deployment_bundle.json"), bundle)?;
                }
            }
        }
        Ok(())
    }

    fn write_apartment_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
        view: &StateView<'_>,
    ) -> eyre::Result<()> {
        for (apartment_name, plan) in &snapshot.apartments {
            let apartment_root = self
                .apartments_root()
                .join(sanitize_path_component(apartment_name));
            fs::create_dir_all(&apartment_root)
                .wrap_err_with(|| format!("create {}", apartment_root.display()))?;
            write_json_atomic(&apartment_root.join("runtime_plan.json"), plan)?;
            if let Some(record) = view
                .world()
                .soracloud_agent_apartments()
                .get(apartment_name)
            {
                write_json_atomic(&apartment_root.join("apartment_manifest.json"), &record.manifest)?;
            }
        }
        Ok(())
    }

    fn prune_stale_service_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let desired: BTreeMap<String, BTreeSet<String>> = snapshot
            .services
            .iter()
            .map(|(service_name, versions)| {
                let desired_versions = versions
                    .keys()
                    .map(|version| sanitize_path_component(version))
                    .collect();
                (sanitize_path_component(service_name), desired_versions)
            })
            .collect();

        prune_nested_directory_tree(self.services_root().as_path(), &desired)?;
        Ok(())
    }

    fn prune_stale_apartment_materializations(
        &self,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let desired: BTreeSet<String> = snapshot
            .apartments
            .keys()
            .map(|name| sanitize_path_component(name))
            .collect();
        prune_flat_directory_tree(self.apartments_root().as_path(), &desired)?;
        Ok(())
    }
}

fn execute_asset_local_read(
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
    state_dir: &Path,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    let Some(artifact) = resolve_asset_artifact(&context.bundle, &context.handler, &request.handler_path)
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "asset handler `{}` on service `{}` cannot resolve request path `{}`",
                context.handler.handler_name, request.service_name, request.handler_path
            ),
        ));
    };
    let cache_path = state_dir
        .join("artifacts")
        .join(hash_cache_name(artifact.artifact_hash));
    let response_bytes = read_and_verify_cached_artifact(&cache_path, artifact.artifact_hash)?;
    let result_commitment = asset_result_commitment(artifact.artifact_hash, &response_bytes);
    let runtime_receipt = match context.handler.certified_response {
        SoraCertifiedResponsePolicyV1::AuditReceipt => Some(local_read_receipt(
            request,
            &context.deployment,
            &context.handler,
            result_commitment,
            context.handler.certified_response,
            None,
        )),
        SoraCertifiedResponsePolicyV1::StateCommitment => None,
        SoraCertifiedResponsePolicyV1::None => {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "asset handler `{}` cannot serve an uncertified fast-path response",
                    context.handler.handler_name
                ),
            ));
        }
    };

    Ok(SoracloudLocalReadResponse {
        response_bytes,
        content_type: Some(content_type_for_path(&artifact.artifact_path).to_owned()),
        content_encoding: None,
        cache_control: Some("public, max-age=60".to_owned()),
        bindings: vec![iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
            binding_name: None,
            state_key: None,
            payload_commitment: None,
            artifact_hash: Some(artifact.artifact_hash),
        }],
        result_commitment,
        certified_by: context.handler.certified_response,
        runtime_receipt,
    })
}

fn execute_query_local_read(
    view: &StateView<'_>,
    request: &SoracloudLocalReadRequest,
    context: &ResolvedLocalReadContext,
) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
    let filters = parse_query_params(request.request_query.as_deref());
    let binding_filter = filters.get("binding").map(String::as_str);
    let key_filter = filters.get("key").map(String::as_str);
    let prefix_filter = filters.get("prefix").map(String::as_str);
    let limit = filters
        .get("limit")
        .and_then(|limit| limit.parse::<usize>().ok())
        .unwrap_or(256)
        .max(1);

    let mut rows = view
        .world()
        .soracloud_service_state_entries()
        .iter()
        .filter(|((_service_name, binding_name, state_key), entry)| {
            entry.service_name.as_ref() == request.service_name
                && binding_filter.is_none_or(|filter| filter == binding_name.as_str())
                && key_filter.is_none_or(|filter| filter == state_key.as_str())
                && prefix_filter.is_none_or(|filter| state_key.starts_with(filter))
        })
        .map(|((_service_name, _binding_name, _state_key), entry)| entry.clone())
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.binding_name
            .cmp(&right.binding_name)
            .then_with(|| left.state_key.cmp(&right.state_key))
    });
    rows.truncate(limit);

    let entries = rows
        .iter()
        .map(|entry| LocalQueryEntry {
            binding_name: entry.binding_name.to_string(),
            state_key: entry.state_key.clone(),
            service_version: entry.service_version.clone(),
            encryption: entry.encryption,
            payload_bytes: entry.payload_bytes.get(),
            payload_commitment: entry.payload_commitment,
            last_update_sequence: entry.last_update_sequence,
            governance_tx_hash: entry.governance_tx_hash,
            source_action: entry.source_action,
        })
        .collect::<Vec<_>>();
    let response = LocalQueryResponse {
        schema_version: 1,
        service_name: request.service_name.clone(),
        service_version: context.deployment.current_service_version.clone(),
        handler_name: context.handler.handler_name.to_string(),
        observed_height: request.observed_height,
        observed_block_hash: request.observed_block_hash.map(|hash| hash.to_string()),
        entries,
    };
    let response_bytes = norito::json::to_json(&response)
        .map(|json| json.into_bytes())
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!("serialize Soracloud query response: {error}"),
            )
        })?;
    let bindings = rows
        .iter()
        .map(state_entry_binding)
        .collect::<Vec<_>>();
    let result_commitment = Hash::new(&response_bytes);
    let runtime_receipt = match context.handler.certified_response {
        SoraCertifiedResponsePolicyV1::AuditReceipt => Some(local_read_receipt(
            request,
            &context.deployment,
            &context.handler,
            result_commitment,
            context.handler.certified_response,
            None,
        )),
        SoraCertifiedResponsePolicyV1::StateCommitment => None,
        SoraCertifiedResponsePolicyV1::None => {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!(
                    "query handler `{}` cannot serve an uncertified fast-path response",
                    context.handler.handler_name
                ),
            ));
        }
    };

    Ok(SoracloudLocalReadResponse {
        response_bytes,
        content_type: Some("application/json".to_owned()),
        content_encoding: None,
        cache_control: Some("no-store".to_owned()),
        bindings,
        result_commitment,
        certified_by: context.handler.certified_response,
        runtime_receipt,
    })
}

fn validate_local_runtime_snapshot(
    view: &StateView<'_>,
    snapshot: &SoracloudRuntimeSnapshot,
    request: &SoracloudLocalReadRequest,
) -> Result<(), SoracloudRuntimeExecutionError> {
    let committed_height = committed_height(view);
    let committed_block_hash = committed_block_hash(view);
    if request.observed_height != committed_height || request.observed_block_hash != committed_block_hash
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "local read snapshot is stale: request observed height/hash {:?}/{:?}, committed {:?}/{:?}",
                request.observed_height,
                request.observed_block_hash,
                committed_height,
                committed_block_hash
            ),
        ));
    }
    if snapshot.observed_height != committed_height
        || parse_snapshot_hash(snapshot.observed_block_hash.as_deref())? != committed_block_hash
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "runtime-manager hydration is behind committed state for service `{}`",
                request.service_name
            ),
        ));
    }
    let Some(service_versions) = snapshot.services.get(&request.service_name) else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "service `{}` is not materialized in the node-local runtime snapshot",
                request.service_name
            ),
        ));
    };
    let Some(plan) = service_versions.get(&request.service_version) else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "service `{}` revision `{}` is not materialized locally",
                request.service_name, request.service_version
            ),
        ));
    };
    if !plan.bundle_available_locally {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "service `{}` revision `{}` is not hydrated locally",
                request.service_name, request.service_version
            ),
        ));
    }
    Ok(())
}

fn validate_apartment_snapshot(
    view: &StateView<'_>,
    snapshot: &SoracloudRuntimeSnapshot,
    request: &SoracloudApartmentExecutionRequest,
) -> Result<(), SoracloudRuntimeExecutionError> {
    let committed_height = committed_height(view);
    let committed_block_hash = committed_block_hash(view);
    if request.observed_height != committed_height || request.observed_block_hash != committed_block_hash
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "apartment execution snapshot is stale: request observed height/hash {:?}/{:?}, committed {:?}/{:?}",
                request.observed_height,
                request.observed_block_hash,
                committed_height,
                committed_block_hash
            ),
        ));
    }
    if snapshot.observed_height != committed_height
        || parse_snapshot_hash(snapshot.observed_block_hash.as_deref())? != committed_block_hash
    {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "runtime-manager apartment snapshot is behind committed state for `{}`",
                request.apartment_name
            ),
        ));
    }
    if !snapshot.apartments.contains_key(&request.apartment_name) {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "apartment `{}` is not materialized in the node-local runtime snapshot",
                request.apartment_name
            ),
        ));
    }
    Ok(())
}

fn resolve_local_read_context(
    view: &StateView<'_>,
    request: &SoracloudLocalReadRequest,
) -> Result<ResolvedLocalReadContext, SoracloudRuntimeExecutionError> {
    let service_id: Name = request
        .service_name
        .parse()
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                format!("invalid Soracloud service name `{}`: {error}", request.service_name),
            )
        })?;
    let Some(deployment) = view
        .world()
        .soracloud_service_deployments()
        .get(&service_id)
        .cloned()
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!("unknown Soracloud service `{}`", request.service_name),
        ));
    };
    if deployment.current_service_version != request.service_version {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!(
                "service `{}` active version `{}` does not match requested local-read version `{}`",
                request.service_name,
                deployment.current_service_version,
                request.service_version
            ),
        ));
    }
    let Some(bundle) = view
        .world()
        .soracloud_service_revisions()
        .get(&(request.service_name.clone(), request.service_version.clone()))
        .cloned()
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "missing admitted Soracloud revision `{}` for service `{}`",
                request.service_version, request.service_name
            ),
        ));
    };
    ensure_ivm_runtime(
        bundle.container.runtime,
        request.service_name.as_str(),
        request.service_version.as_str(),
    )
    .map_err(|message| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            message,
        )
    })?;
    let Some(handler) = bundle
        .service
        .handlers
        .iter()
        .find(|handler| {
            handler.handler_name.as_ref() == request.handler_name
                && handler.class == request.handler_class.handler_class()
        })
        .cloned()
    else {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            format!(
                "service `{}` revision `{}` does not expose handler `{}` for {:?}",
                request.service_name,
                request.service_version,
                request.handler_name,
                request.handler_class
            ),
        ));
    };

    Ok(ResolvedLocalReadContext {
        deployment,
        bundle,
        handler,
    })
}

fn resolve_asset_artifact<'a>(
    bundle: &'a SoraDeploymentBundleV1,
    handler: &SoraServiceHandlerV1,
    handler_path: &str,
) -> Option<&'a iroha_data_model::soracloud::SoraArtifactRefV1> {
    let normalized_handler_path = if handler_path.is_empty() {
        "/"
    } else {
        handler_path
    };
    let mut candidates = bundle
        .service
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == SoraArtifactKindV1::StaticAsset
                && artifact
                    .handler_name
                    .as_ref()
                    .is_some_and(|name| name == &handler.handler_name)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.artifact_path.cmp(&right.artifact_path));
    if normalized_handler_path == "/" {
        return candidates
            .iter()
            .copied()
            .find(|artifact| artifact.artifact_path.ends_with("/index.html"))
            .or_else(|| candidates.into_iter().next());
    }

    candidates
        .iter()
        .copied()
        .find(|artifact| artifact.artifact_path == normalized_handler_path)
        .or_else(|| {
            candidates
                .iter()
                .copied()
                .find(|artifact| artifact.artifact_path.ends_with(normalized_handler_path))
        })
}

fn read_and_verify_cached_artifact(
    cache_path: &Path,
    expected_hash: Hash,
) -> Result<Vec<u8>, SoracloudRuntimeExecutionError> {
    let response_bytes = fs::read(cache_path).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            format!("read hydrated Soracloud artifact cache {}: {error}", cache_path.display()),
        )
    })?;
    let actual_hash = Hash::new(&response_bytes);
    if actual_hash != expected_hash {
        return Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "hydrated Soracloud artifact cache {} failed hash verification: expected {}, found {}",
                cache_path.display(),
                expected_hash,
                actual_hash
            ),
        ));
    }
    Ok(response_bytes)
}

fn asset_result_commitment(artifact_hash: Hash, response_bytes: &[u8]) -> Hash {
    let mut payload = Vec::with_capacity(Hash::LENGTH + response_bytes.len());
    payload.extend_from_slice(artifact_hash.as_ref());
    payload.extend_from_slice(response_bytes);
    Hash::new(payload)
}

fn state_entry_binding(
    entry: &SoraServiceStateEntryV1,
) -> iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
    iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
        binding_name: Some(entry.binding_name.to_string()),
        state_key: Some(entry.state_key.clone()),
        payload_commitment: Some(entry.payload_commitment),
        artifact_hash: None,
    }
}

fn local_read_receipt(
    request: &SoracloudLocalReadRequest,
    deployment: &SoraServiceDeploymentStateV1,
    handler: &SoraServiceHandlerV1,
    result_commitment: Hash,
    certified_by: SoraCertifiedResponsePolicyV1,
    mailbox_message_id: Option<Hash>,
) -> SoraRuntimeReceiptV1 {
    let emitted_sequence = next_authoritative_observation_sequence_from_view(deployment.service_name.as_ref(), request.observed_height);
    SoraRuntimeReceiptV1 {
        schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
        receipt_id: Hash::new(
            Encode::encode(&(
                "soracloud:local-read",
                deployment.service_name.as_ref(),
                deployment.current_service_version.as_str(),
                handler.handler_name.as_ref(),
                request.request_commitment,
                result_commitment,
                certified_by,
            )),
        ),
        service_name: deployment.service_name.clone(),
        service_version: deployment.current_service_version.clone(),
        handler_name: handler.handler_name.clone(),
        handler_class: handler.class,
        request_commitment: request.request_commitment,
        result_commitment,
        certified_by,
        emitted_sequence,
        mailbox_message_id,
        journal_artifact_hash: None,
        checkpoint_artifact_hash: None,
    }
}

fn next_authoritative_observation_sequence_from_view(_service_name: &str, observed_height: u64) -> u64 {
    observed_height.max(1)
}

fn apartment_result_commitment(
    apartment_name: &str,
    process_generation: u64,
    operation: &str,
    request_commitment: Hash,
    status: iroha_data_model::soracloud::SoraAgentRuntimeStatusV1,
) -> Hash {
    Hash::new(Encode::encode(&(
        "soracloud:apartment",
        apartment_name,
        process_generation,
        operation,
        request_commitment,
        status,
    )))
}

fn committed_height(view: &StateView<'_>) -> u64 {
    u64::try_from(view.height()).unwrap_or(u64::MAX)
}

fn committed_block_hash(view: &StateView<'_>) -> Option<Hash> {
    view.latest_block_hash().map(Hash::from)
}

fn parse_snapshot_hash(
    snapshot_hash: Option<&str>,
) -> Result<Option<Hash>, SoracloudRuntimeExecutionError> {
    snapshot_hash
        .map(Hash::from_str)
        .transpose()
        .map_err(|error| {
            SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Internal,
                format!("invalid Soracloud runtime snapshot block hash: {error}"),
            )
        })
}

fn parse_query_params(query: Option<&str>) -> BTreeMap<String, String> {
    query
        .unwrap_or_default()
        .split('&')
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let (key, value) = entry.split_once('=').unwrap_or((entry, ""));
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

fn content_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("css") => "text/css; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("mjs") => "application/javascript; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("xml") => "application/xml",
        _ => "application/octet-stream",
    }
}

fn build_runtime_snapshot(
    view: &StateView<'_>,
    bundle_registry: &BTreeMap<(String, String), SoraDeploymentBundleV1>,
    state_dir: &Path,
    artifacts_root: PathBuf,
) -> eyre::Result<SoracloudRuntimeSnapshot> {
    let mut services = BTreeMap::new();
    let world = view.world();

    for (service_name, deployment) in world.soracloud_service_deployments().iter() {
        let service_name_key = service_name.clone();
        let service_name = service_name_key.to_string();
        let versions = collect_active_versions(deployment);
        let runtime_state = world.soracloud_service_runtime().get(&service_name_key);
        let authoritative_pending = authoritative_mailbox_counts(
            world.soracloud_mailbox_messages(),
            world.soracloud_runtime_receipts(),
        );

        let mut version_plans = BTreeMap::new();
        for (service_version, role, traffic_percent) in versions {
            let bundle = bundle_registry
                .get(&(service_name.clone(), service_version.clone()))
                .ok_or_else(|| {
                    eyre::eyre!(
                        "deployment for service `{service_name}` references missing admitted revision `{service_version}`"
                    )
                })?;
            ensure_ivm_runtime(bundle.container.runtime, &service_name, &service_version)
                .map_err(eyre::Report::msg)?;
            let is_runtime_active = runtime_state
                .as_ref()
                .is_some_and(|state| state.active_service_version == service_version);
            let service_dir = state_dir
                .join("services")
                .join(sanitize_path_component(&service_name))
                .join(sanitize_path_component(&service_version));
            let bundle_cache_path = artifacts_root.join(hash_cache_name(bundle.container.bundle_hash));
            let plan = SoracloudRuntimeServicePlan {
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                role,
                traffic_percent,
                runtime: bundle.container.runtime,
                bundle_hash: bundle.container.bundle_hash.to_string(),
                bundle_path: bundle.container.bundle_path.clone(),
                entrypoint: bundle.container.entrypoint.clone(),
                bundle_cache_path: bundle_cache_path.display().to_string(),
                bundle_available_locally: bundle_cache_path.exists(),
                process_generation: is_runtime_active.then_some(deployment.process_generation),
                health_status: runtime_state
                    .as_ref()
                    .filter(|state| state.active_service_version == service_version)
                    .map_or(SoraServiceHealthStatusV1::Hydrating, |state| state.health_status),
                load_factor_bps: runtime_state
                    .as_ref()
                    .filter(|state| state.active_service_version == service_version)
                    .map_or(0, |state| state.load_factor_bps),
                reported_pending_mailbox_messages: runtime_state
                    .as_ref()
                    .filter(|state| state.active_service_version == service_version)
                    .map_or(0, |state| state.pending_mailbox_message_count),
                authoritative_pending_mailbox_messages: authoritative_pending
                    .get(&service_name)
                    .copied()
                    .unwrap_or_default(),
                rollout_handle: deployment
                    .active_rollout
                    .as_ref()
                    .map(|rollout| rollout.rollout_handle.clone()),
                materialization_dir: service_dir.display().to_string(),
                mailboxes: bundle
                    .service
                    .handlers
                    .iter()
                    .filter_map(|handler| {
                        handler.mailbox.as_ref().map(|mailbox| SoracloudRuntimeMailboxPlan {
                            handler_name: handler.handler_name.to_string(),
                            queue_name: mailbox.queue_name.to_string(),
                            max_pending_messages: mailbox.max_pending_messages.get(),
                            max_message_bytes: mailbox.max_message_bytes.get(),
                            retention_blocks: mailbox.retention_blocks.get(),
                        })
                    })
                    .collect(),
                artifacts: build_artifact_plans(bundle, &artifacts_root),
            };
            version_plans.insert(service_version, plan);
        }
        services.insert(service_name, version_plans);
    }

    let apartments = world
        .soracloud_agent_apartments()
        .iter()
        .map(|(apartment_name, record)| {
            (
                apartment_name.clone(),
                build_apartment_plan(apartment_name, record, state_dir),
            )
        })
        .collect();

    Ok(SoracloudRuntimeSnapshot {
        schema_version: SoracloudRuntimeSnapshot::default().schema_version,
        observed_height: u64::try_from(view.height()).unwrap_or(u64::MAX),
        observed_block_hash: view.latest_block_hash().map(|hash| hash.to_string()),
        services,
        apartments,
    })
}

fn build_apartment_plan(
    apartment_name: &str,
    record: &SoraAgentApartmentRecordV1,
    state_dir: &Path,
) -> SoracloudRuntimeApartmentPlan {
    let apartment_root = state_dir
        .join("apartments")
        .join(sanitize_path_component(apartment_name));
    SoracloudRuntimeApartmentPlan {
        apartment_name: apartment_name.to_string(),
        manifest_hash: record.manifest_hash.to_string(),
        status: record.status,
        process_generation: record.process_generation,
        lease_expires_sequence: record.lease_expires_sequence,
        last_active_sequence: record.last_active_sequence,
        materialization_dir: apartment_root.display().to_string(),
        pending_wallet_request_count: u32::try_from(record.pending_wallet_requests.len())
            .unwrap_or(u32::MAX),
        pending_mailbox_message_count: u32::try_from(record.mailbox_queue.len())
            .unwrap_or(u32::MAX),
        autonomy_budget_remaining_units: record.autonomy_budget_remaining_units,
        approved_artifact_count: u32::try_from(record.artifact_allowlist.len()).unwrap_or(u32::MAX),
        autonomy_run_count: u32::try_from(record.autonomy_run_history.len()).unwrap_or(u32::MAX),
        revoked_policy_capability_count: u32::try_from(record.revoked_policy_capabilities.len())
            .unwrap_or(u32::MAX),
    }
}

fn build_artifact_plans(
    bundle: &SoraDeploymentBundleV1,
    artifacts_root: &Path,
) -> Vec<SoracloudRuntimeArtifactPlan> {
    let mut artifacts = Vec::with_capacity(bundle.service.artifacts.len().saturating_add(1));
    let bundle_cache_path = artifacts_root.join(hash_cache_name(bundle.container.bundle_hash));
    artifacts.push(SoracloudRuntimeArtifactPlan {
        kind: SoraArtifactKindV1::Bundle,
        artifact_hash: bundle.container.bundle_hash.to_string(),
        artifact_path: bundle.container.bundle_path.clone(),
        handler_name: None,
        local_cache_path: bundle_cache_path.display().to_string(),
        available_locally: bundle_cache_path.exists(),
    });
    artifacts.extend(bundle.service.artifacts.iter().map(|artifact| {
        let cache_path = artifacts_root.join(hash_cache_name(artifact.artifact_hash));
        SoracloudRuntimeArtifactPlan {
            kind: artifact.kind,
            artifact_hash: artifact.artifact_hash.to_string(),
            artifact_path: artifact.artifact_path.clone(),
            handler_name: artifact.handler_name.as_ref().map(ToString::to_string),
            local_cache_path: cache_path.display().to_string(),
            available_locally: cache_path.exists(),
        }
    }));
    artifacts
}

fn collect_service_revision_registry(
    view: &StateView<'_>,
) -> BTreeMap<(String, String), SoraDeploymentBundleV1> {
    view.world()
        .soracloud_service_revisions()
        .iter()
        .map(|((service_name, service_version), bundle)| {
            ((service_name.clone(), service_version.clone()), bundle.clone())
        })
        .collect()
}

fn collect_active_versions(
    deployment: &SoraServiceDeploymentStateV1,
) -> Vec<(String, SoracloudRuntimeRevisionRole, u8)> {
    let mut versions = Vec::new();
    if let Some(rollout) = deployment.active_rollout.as_ref() {
        let traffic_percent = rollout.traffic_percent.min(100);
        let baseline_percent = 100u8.saturating_sub(traffic_percent);
        versions.push((
            deployment.current_service_version.clone(),
            SoracloudRuntimeRevisionRole::Active,
            baseline_percent,
        ));
        if rollout.candidate_version != deployment.current_service_version {
            versions.push((
                rollout.candidate_version.clone(),
                SoracloudRuntimeRevisionRole::CanaryCandidate,
                traffic_percent,
            ));
        }
    } else {
        versions.push((
            deployment.current_service_version.clone(),
            SoracloudRuntimeRevisionRole::Active,
            100,
        ));
    }
    versions
}

fn authoritative_mailbox_counts(
    messages: &impl StorageReadOnly<Hash, SoraServiceMailboxMessageV1>,
    receipts: &impl StorageReadOnly<Hash, SoraRuntimeReceiptV1>,
) -> BTreeMap<String, u32> {
    let consumed: BTreeSet<Hash> = receipts
        .iter()
        .filter_map(|(_receipt_id, receipt)| receipt.mailbox_message_id)
        .collect();
    let mut counts = BTreeMap::new();
    for (_, message) in messages.iter() {
        if consumed.contains(&message.message_id) {
            continue;
        }
        let entry = counts.entry(message.to_service.to_string()).or_insert(0u32);
        *entry = entry.saturating_add(1);
    }
    counts
}

fn collect_committed_service_state_entries(
    view: &StateView<'_>,
    service_name: &str,
) -> BTreeMap<(String, String), SoraServiceStateEntryV1> {
    view.world()
        .soracloud_service_state_entries()
        .iter()
        .filter(|((_service, _binding, _key), entry)| entry.service_name.as_ref() == service_name)
        .map(|((_service, binding, key), entry)| ((binding.clone(), key.clone()), entry.clone()))
        .collect()
}

fn deterministic_mailbox_failure_result(
    request: SoracloudOrderedMailboxExecutionRequest,
    outcome_label: &str,
    health_status: SoraServiceHealthStatusV1,
) -> SoracloudOrderedMailboxExecutionResult {
    deterministic_mailbox_failure_result_with_message(
        request,
        outcome_label,
        outcome_label.to_owned(),
        health_status,
    )
}

fn deterministic_mailbox_failure_result_with_message(
    request: SoracloudOrderedMailboxExecutionRequest,
    outcome_label: &str,
    detail: String,
    health_status: SoraServiceHealthStatusV1,
) -> SoracloudOrderedMailboxExecutionResult {
    let result_commitment = Hash::new(Encode::encode(&(
        "soracloud:runtime-failure:v1",
        request.mailbox_message.message_id,
        request.deployment.service_name.as_ref(),
        request.deployment.current_service_version.as_str(),
        request.mailbox_message.to_handler.as_ref(),
        request.execution_sequence,
        outcome_label,
        detail,
    )));
    let receipt_id = mailbox_receipt_id(
        request.mailbox_message.message_id,
        request.deployment.service_name.as_ref(),
        &request.deployment.current_service_version,
        request.execution_sequence,
        outcome_label,
    );
    SoracloudOrderedMailboxExecutionResult {
        state_mutations: Vec::new(),
        outbound_mailbox_messages: Vec::new(),
        runtime_state: Some(updated_runtime_state_with_outbound_mailbox(
            request.runtime_state.clone(),
            &request,
            health_status,
            &[],
        )),
        runtime_receipt: SoraRuntimeReceiptV1 {
            schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
            receipt_id,
            service_name: request.deployment.service_name,
            service_version: request.deployment.current_service_version,
            handler_name: request.mailbox_message.to_handler.clone(),
            handler_class: request
                .handler
                .as_ref()
                .map(|handler| handler.class)
                .unwrap_or(SoraServiceHandlerClassV1::Update),
            request_commitment: request.mailbox_message.payload_commitment,
            result_commitment,
            certified_by: SoraCertifiedResponsePolicyV1::None,
            emitted_sequence: request.execution_sequence,
            mailbox_message_id: Some(request.mailbox_message.message_id),
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
        },
    }
}

fn mailbox_receipt_id(
    message_id: Hash,
    service_name: &str,
    service_version: &str,
    execution_sequence: u64,
    outcome_label: &str,
) -> Hash {
    Hash::new(
        format!(
            "soracloud:runtime-receipt:{message_id}:{service_name}:{service_version}:{execution_sequence}:{outcome_label}"
        )
        .as_bytes(),
    )
}

fn synthetic_runtime_state(
    request: &SoracloudOrderedMailboxExecutionRequest,
    health_status: SoraServiceHealthStatusV1,
) -> iroha_data_model::soracloud::SoraServiceRuntimeStateV1 {
    iroha_data_model::soracloud::SoraServiceRuntimeStateV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
        service_name: request.deployment.service_name.clone(),
        active_service_version: request.deployment.current_service_version.clone(),
        health_status,
        load_factor_bps: 0,
        materialized_bundle_hash: request.bundle.container.bundle_hash,
        rollout_handle: request
            .deployment
            .active_rollout
            .as_ref()
            .map(|rollout| rollout.rollout_handle.clone()),
        pending_mailbox_message_count: request.authoritative_pending_mailbox_messages,
        last_receipt_id: None,
    }
}

fn updated_runtime_state_with_outbound_mailbox(
    runtime_state: Option<iroha_data_model::soracloud::SoraServiceRuntimeStateV1>,
    request: &SoracloudOrderedMailboxExecutionRequest,
    health_status: SoraServiceHealthStatusV1,
    outbound_mailbox_messages: &[SoraServiceMailboxMessageV1],
) -> iroha_data_model::soracloud::SoraServiceRuntimeStateV1 {
    let mut runtime_state =
        runtime_state.unwrap_or_else(|| synthetic_runtime_state(request, health_status));
    let self_requeued = outbound_mailbox_messages
        .iter()
        .filter(|message| message.to_service == request.deployment.service_name)
        .count();
    runtime_state.health_status = health_status;
    runtime_state.pending_mailbox_message_count = request
        .authoritative_pending_mailbox_messages
        .saturating_sub(1)
        .saturating_add(u32::try_from(self_requeued).unwrap_or(u32::MAX));
    runtime_state
}

fn ensure_ivm_runtime(
    runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1,
    service_name: &str,
    service_version: &str,
) -> Result<(), String> {
    match runtime {
        iroha_data_model::soracloud::SoraContainerRuntimeV1::Ivm => Ok(()),
        iroha_data_model::soracloud::SoraContainerRuntimeV1::NativeProcess => Err(format!(
            "service `{service_name}` revision `{service_version}` targets unsupported Soracloud runtime `NativeProcess`; v1 currently admits only `Ivm`"
        )),
    }
}

fn authoritative_mailbox_result_commitment(
    request: &SoracloudOrderedMailboxExecutionRequest,
    state_mutations: &[iroha_core::soracloud_runtime::SoracloudDeterministicStateMutation],
    outbound_mailbox_messages: &[SoraServiceMailboxMessageV1],
    runtime_state: &SoraServiceRuntimeStateV1,
    journal_artifact_hash: Option<Hash>,
    checkpoint_artifact_hash: Option<Hash>,
) -> Hash {
    let mutation_fingerprints = state_mutations
        .iter()
        .map(|mutation| {
            (
                mutation.binding_name.as_str(),
                mutation.state_key.as_str(),
                mutation.operation,
                mutation.encryption,
                mutation.payload_bytes,
                mutation.payload_commitment,
            )
        })
        .collect::<Vec<_>>();
    let outbound_fingerprints = outbound_mailbox_messages
        .iter()
        .map(|message| {
            (
                message.message_id,
                message.from_service.as_ref(),
                message.from_handler.as_ref(),
                message.to_service.as_ref(),
                message.to_handler.as_ref(),
                message.payload_commitment,
                message.available_after_sequence,
                message.expires_at_sequence,
            )
        })
        .collect::<Vec<_>>();
    Hash::new(Encode::encode(&(
        "soracloud:runtime-result:v1",
        request.mailbox_message.message_id,
        request.deployment.service_name.as_ref(),
        request.deployment.current_service_version.as_str(),
        request.mailbox_message.to_handler.as_ref(),
        request.execution_sequence,
        mutation_fingerprints,
        outbound_fingerprints,
        runtime_state.clone(),
        journal_artifact_hash,
        checkpoint_artifact_hash,
    )))
}

fn mailbox_payload_tlv_bytes(payload_bytes: &[u8]) -> Result<Vec<u8>, VMError> {
    if payload_bytes.is_empty() {
        return Ok(make_pointer_tlv(PointerType::Blob, &[]));
    }
    if ivm::pointer_abi::validate_tlv_bytes(payload_bytes).is_ok() {
        return Ok(payload_bytes.to_vec());
    }
    Ok(make_pointer_tlv(PointerType::Blob, payload_bytes))
}

fn make_pointer_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(u32::try_from(payload.len()).unwrap_or(u32::MAX)).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(Hash::new(payload).as_ref());
    out
}

fn vm_error_label(error: &VMError) -> &'static str {
    match error {
        VMError::OutOfGas => "out_of_gas",
        VMError::OutOfMemory => "out_of_memory",
        VMError::MemoryAccessViolation { .. } => "memory_access_violation",
        VMError::MisalignedAccess { .. } => "misaligned_access",
        VMError::MemoryOutOfBounds => "memory_out_of_bounds",
        VMError::UnalignedAccess => "unaligned_access",
        VMError::MemoryPermissionDenied => "memory_permission_denied",
        VMError::DecodeError => "decode_error",
        VMError::InvalidOpcode(_) => "invalid_opcode",
        VMError::UnknownSyscall(_) => "unknown_syscall",
        VMError::HostUnavailable => "host_unavailable",
        VMError::NotImplemented { .. } => "not_implemented",
        VMError::AssertionFailed => "assertion_failed",
        VMError::ExceededMaxCycles => "exceeded_max_cycles",
        VMError::InvalidMetadata => "invalid_metadata",
        VMError::VectorExtensionDisabled => "vector_disabled",
        VMError::ZkExtensionDisabled => "zk_disabled",
        VMError::NullifierAlreadyUsed => "nullifier_used",
        VMError::PermissionDenied => "permission_denied",
        VMError::PrivacyViolation => "privacy_violation",
        VMError::RegisterOutOfBounds => "register_out_of_bounds",
        VMError::HTMAbort => "htm_abort",
        VMError::NoritoInvalid => "norito_invalid",
        VMError::AbiTypeNotAllowed { .. } => "abi_type_not_allowed",
        VMError::AmxBudgetExceeded { .. } => "amx_budget_exceeded",
    }
}

fn persist_staged_runtime_artifact(
    root: PathBuf,
    artifact: Option<&StagedRuntimeArtifact>,
) -> Result<Option<Hash>, SoracloudRuntimeExecutionError> {
    let Some(artifact) = artifact else {
        return Ok(None);
    };
    fs::create_dir_all(&root).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!("create Soracloud runtime artifact root {}: {error}", root.display()),
        )
    })?;
    let path = root.join(hash_cache_name(artifact.artifact_hash));
    fs::write(&path, &artifact.bytes).map_err(|error| {
        SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Internal,
            format!(
                "persist Soracloud runtime artifact `{}` at {}: {error}",
                artifact.artifact_path,
                path.display()
            ),
        )
    })?;
    Ok(Some(artifact.artifact_hash))
}

fn sanitized_relative_material_path(key: &str) -> Result<PathBuf, VMError> {
    if key.trim().is_empty() {
        return Err(VMError::PermissionDenied);
    }
    let mut path = PathBuf::new();
    for component in key.split('/') {
        if component.is_empty() || matches!(component, "." | "..") {
            return Err(VMError::PermissionDenied);
        }
        path.push(sanitize_path_component(component));
    }
    Ok(path)
}

fn url_host(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split('/').next()?;
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = authority
        .strip_prefix('[')
        .and_then(|value| value.split_once(']').map(|(host, _)| host))
        .unwrap_or_else(|| authority.split(':').next().unwrap_or(authority));
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn hash_cache_name(hash: Hash) -> String {
    sanitize_path_component(&hash.to_string())
}

fn sanitize_path_component(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '_',
        })
        .collect()
}

fn prune_nested_directory_tree(
    root: &Path,
    desired: &BTreeMap<String, BTreeSet<String>>,
) -> eyre::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for service_entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let service_entry = service_entry?;
        if !service_entry.file_type()?.is_dir() {
            continue;
        }
        let service_name = service_entry.file_name().to_string_lossy().into_owned();
        let service_path = service_entry.path();
        let Some(desired_versions) = desired.get(&service_name) else {
            fs::remove_dir_all(&service_path)
                .wrap_err_with(|| format!("remove stale {}", service_path.display()))?;
            continue;
        };
        for version_entry in
            fs::read_dir(&service_path).wrap_err_with(|| format!("read {}", service_path.display()))?
        {
            let version_entry = version_entry?;
            if !version_entry.file_type()?.is_dir() {
                continue;
            }
            let version_name = version_entry.file_name().to_string_lossy().into_owned();
            if !desired_versions.contains(&version_name) {
                let version_path = version_entry.path();
                fs::remove_dir_all(&version_path)
                    .wrap_err_with(|| format!("remove stale {}", version_path.display()))?;
            }
        }
        let mut remaining = fs::read_dir(&service_path)?;
        if remaining.next().is_none() {
            fs::remove_dir_all(&service_path)
                .wrap_err_with(|| format!("remove empty {}", service_path.display()))?;
        }
    }
    Ok(())
}

fn prune_flat_directory_tree(root: &Path, desired: &BTreeSet<String>) -> eyre::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !desired.contains(&name) {
            let path = entry.path();
            fs::remove_dir_all(&path)
                .wrap_err_with(|| format!("remove stale {}", path.display()))?;
        }
    }
    Ok(())
}

fn write_json_atomic<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: norito::json::JsonSerialize + ?Sized,
{
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path must have a parent"))?;
    fs::create_dir_all(parent)?;
    let payload = norito::json::to_json(value)
        .map_err(|error| io::Error::other(format!("serialize json: {error}")))?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, payload)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn read_json_optional<T>(path: &Path) -> io::Result<Option<T>>
where
    T: norito::json::JsonDeserialize,
{
    let payload = match fs::read(path) {
        Ok(payload) => payload,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if payload.is_empty() {
        return Ok(None);
    }
    norito::json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::other(format!("deserialize json: {error}")))
}

#[cfg(test)]
mod tests {
    //! Tests for the embedded Soracloud runtime manager.

    use super::*;
    use std::sync::Arc;

    use eyre::Result;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_data_model::{
        smart_contract::manifest::EntryPointKind,
        soracloud::{
            AgentApartmentManifestV1, SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1, SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            SORA_SERVICE_RUNTIME_STATE_VERSION_V1, SoraAgentPersistentStateV1,
            SoraAgentRuntimeStatusV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
            SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1, SoraServiceHealthStatusV1,
            SoraServiceMailboxMessageV1, SoraServiceRuntimeStateV1,
        },
    };

    fn load_deployment_bundle_fixture() -> Result<SoraDeploymentBundleV1> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/sora_deployment_bundle_v1.json");
        let raw = fs::read_to_string(path)?;
        Ok(norito::json::from_str(&raw)?)
    }

    fn load_agent_manifest_fixture() -> Result<AgentApartmentManifestV1> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/agent_apartment_manifest_v1.json");
        let raw = fs::read_to_string(path)?;
        Ok(norito::json::from_str(&raw)?)
    }

    fn test_state() -> Result<Arc<State>> {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        Ok(Arc::new(State::new_for_testing(World::new(), kura, query)))
    }

    fn sample_agent_record() -> Result<SoraAgentApartmentRecordV1> {
        let manifest = load_agent_manifest_fixture()?;
        Ok(SoraAgentApartmentRecordV1 {
            schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            manifest_hash: Hash::prehashed([0xAA; Hash::LENGTH]),
            status: SoraAgentRuntimeStatusV1::Running,
            deployed_sequence: 1,
            lease_started_sequence: 1,
            lease_expires_sequence: 42,
            last_renewed_sequence: 1,
            restart_count: 0,
            last_restart_sequence: None,
            last_restart_reason: None,
            process_generation: 7,
            process_started_sequence: 1,
            last_active_sequence: 9,
            last_checkpoint_sequence: None,
            checkpoint_count: 0,
            persistent_state: SoraAgentPersistentStateV1 {
                total_bytes: 0,
                key_sizes: BTreeMap::new(),
            },
            revoked_policy_capabilities: BTreeSet::from(["wallet.sign".to_string()]),
            pending_wallet_requests: BTreeMap::new(),
            wallet_daily_spend: BTreeMap::new(),
            mailbox_queue: Vec::new(),
            autonomy_budget_ceiling_units: 500,
            autonomy_budget_remaining_units: 325,
            artifact_allowlist: BTreeMap::new(),
            autonomy_run_history: Vec::new(),
            manifest,
        })
    }

    fn sample_runtime_state(bundle: &SoraDeploymentBundleV1) -> SoraServiceRuntimeStateV1 {
        SoraServiceRuntimeStateV1 {
            schema_version: SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            service_name: bundle.service.service_name.clone(),
            active_service_version: bundle.service.service_version.clone(),
            health_status: SoraServiceHealthStatusV1::Healthy,
            load_factor_bps: 425,
            materialized_bundle_hash: bundle.container.bundle_hash,
            rollout_handle: None,
            pending_mailbox_message_count: 3,
            last_receipt_id: None,
        }
    }

    fn sample_deployment_state(bundle: &SoraDeploymentBundleV1) -> SoraServiceDeploymentStateV1 {
        SoraServiceDeploymentStateV1 {
            schema_version: SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
            service_name: bundle.service.service_name.clone(),
            current_service_version: bundle.service.service_version.clone(),
            current_service_manifest_hash: bundle.service.container.manifest_hash,
            current_container_manifest_hash: bundle.service.container.manifest_hash,
            revision_count: 1,
            process_generation: 5,
            process_started_sequence: 11,
            active_rollout: None,
            last_rollout: None,
        }
    }

    fn soracloud_entrypoint(
        name: &str,
        entry_pc: u64,
    ) -> ivm::EmbeddedEntrypointDescriptor {
        ivm::EmbeddedEntrypointDescriptor {
            name: name.to_owned(),
            kind: EntryPointKind::Public,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc,
        }
    }

    fn simple_soracloud_contract_artifact(entrypoints: &[&str]) -> Vec<u8> {
        let metadata = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        };
        let contract_interface = ivm::EmbeddedContractInterfaceV1 {
            compiler_fingerprint: "irohad-soracloud-tests".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: entrypoints
                .iter()
                .map(|name| soracloud_entrypoint(name, 0))
                .collect(),
        };
        let mut bytes = metadata.encode();
        bytes.extend_from_slice(&contract_interface.encode_section());
        bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        bytes
    }

    fn bundle_handler(
        bundle: &SoraDeploymentBundleV1,
        handler_name: &str,
    ) -> iroha_data_model::soracloud::SoraServiceHandlerV1 {
        bundle
            .service
            .handlers
            .iter()
            .find(|handler| handler.handler_name.as_ref() == handler_name)
            .cloned()
            .expect("fixture handler must exist")
    }

    fn sample_mailbox_message(
        bundle: &SoraDeploymentBundleV1,
        handler_name: &str,
        payload_bytes: Vec<u8>,
    ) -> SoraServiceMailboxMessageV1 {
        let payload_commitment = Hash::new(&payload_bytes);
        SoraServiceMailboxMessageV1 {
            schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id: Hash::new(Encode::encode(&(
                "soracloud.runtime.tests.mailbox",
                bundle.service.service_name.as_ref(),
                handler_name,
                payload_commitment,
            ))),
            from_service: "scheduler".parse().expect("literal name"),
            from_handler: "dispatch".parse().expect("literal name"),
            to_service: bundle.service.service_name.clone(),
            to_handler: handler_name.parse().expect("fixture handler name"),
            payload_bytes,
            payload_commitment,
            enqueue_sequence: 6,
            available_after_sequence: 6,
            expires_at_sequence: None,
        }
    }

    fn sample_ordered_mailbox_request(
        bundle: &SoraDeploymentBundleV1,
        handler_name: &str,
        mailbox_message: SoraServiceMailboxMessageV1,
    ) -> SoracloudOrderedMailboxExecutionRequest {
        SoracloudOrderedMailboxExecutionRequest {
            observed_height: 0,
            observed_block_hash: None,
            execution_sequence: 7,
            deployment: sample_deployment_state(bundle),
            bundle: bundle.clone(),
            handler: Some(bundle_handler(bundle, handler_name)),
            mailbox_message,
            runtime_state: Some(sample_runtime_state(bundle)),
            authoritative_pending_mailbox_messages: 1,
        }
    }

    fn test_runtime_manager_config(state_dir: PathBuf) -> SoracloudRuntimeManagerConfig {
        let runtime = iroha_config::parameters::actual::SoracloudRuntime {
            state_dir,
            ..Default::default()
        };
        SoracloudRuntimeManagerConfig::from_runtime_config(&runtime)
    }

    fn test_runtime_handle(
        manager: &SoracloudRuntimeManager,
        state: Arc<State>,
    ) -> SoracloudRuntimeManagerHandle {
        SoracloudRuntimeManagerHandle {
            snapshot: Arc::clone(&manager.snapshot),
            config: Arc::new(manager.config.clone()),
            state_dir: Arc::new(manager.config.state_dir.clone()),
            state,
        }
    }

    #[test]
    fn manager_config_uses_explicit_soracloud_runtime_settings() {
        let runtime = iroha_config::parameters::actual::SoracloudRuntime {
            state_dir: PathBuf::from("/tmp/iroha-soracloud-runtime-config"),
            reconcile_interval: Duration::from_secs(17),
            hydration_concurrency: std::num::NonZeroUsize::new(9)
                .expect("nonzero hydration concurrency"),
            cache_budgets: iroha_config::parameters::actual::SoracloudRuntimeCacheBudgets {
                bundle_bytes: std::num::NonZeroU64::new(1_024).expect("nonzero"),
                static_asset_bytes: std::num::NonZeroU64::new(2_048).expect("nonzero"),
                journal_bytes: std::num::NonZeroU64::new(3_072).expect("nonzero"),
                checkpoint_bytes: std::num::NonZeroU64::new(4_096).expect("nonzero"),
                model_artifact_bytes: std::num::NonZeroU64::new(5_120).expect("nonzero"),
                model_weight_bytes: std::num::NonZeroU64::new(6_144).expect("nonzero"),
            },
            native_process: iroha_config::parameters::actual::SoracloudRuntimeNativeProcess {
                max_concurrent_processes: std::num::NonZeroUsize::new(3)
                    .expect("nonzero concurrent processes"),
                cpu_millis: std::num::NonZeroU32::new(1_500).expect("nonzero cpu"),
                memory_bytes: std::num::NonZeroU64::new(65_536).expect("nonzero memory"),
                ephemeral_storage_bytes: std::num::NonZeroU64::new(131_072)
                    .expect("nonzero storage"),
                max_open_files: std::num::NonZeroU32::new(64).expect("nonzero files"),
                max_tasks: std::num::NonZeroU16::new(32).expect("nonzero tasks"),
                start_grace: Duration::from_secs(3),
                stop_grace: Duration::from_secs(5),
            },
            egress: iroha_config::parameters::actual::SoracloudRuntimeEgress {
                default_allow: true,
                allowed_hosts: vec!["cdn.sora.test".to_string()],
                rate_per_minute: std::num::NonZeroU32::new(120),
                max_bytes_per_minute: std::num::NonZeroU64::new(262_144),
            },
        };

        let manager = SoracloudRuntimeManagerConfig::from_runtime_config(&runtime);
        assert_eq!(manager.state_dir, runtime.state_dir);
        assert_eq!(manager.reconcile_interval, runtime.reconcile_interval);
        assert_eq!(manager.hydration_concurrency, runtime.hydration_concurrency);
        assert_eq!(manager.cache_budgets, runtime.cache_budgets);
        assert_eq!(manager.native_process, runtime.native_process);
        assert_eq!(manager.egress, runtime.egress);
    }

    #[test]
    fn reconcile_once_persists_active_service_and_apartment_materializations() -> Result<()> {
        let mut state = test_state()?;
        let bundle = load_deployment_bundle_fixture()?;
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let apartment = sample_agent_record()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
                    (
                        bundle.service.service_name.to_string(),
                        bundle.service.service_version.clone(),
                    ),
                    bundle.clone(),
                );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
            world
                .soracloud_agent_apartments_mut_for_testing()
                .insert(apartment.manifest.apartment_name.to_string(), apartment);
        }

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let service_versions = snapshot
            .services
            .get("web_portal")
            .expect("service snapshot present");
        let plan = service_versions
            .get("2026.02.0")
            .expect("service version snapshot present");
        assert_eq!(plan.runtime, SoraContainerRuntimeV1::Ivm);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Healthy);
        assert_eq!(plan.authoritative_pending_mailbox_messages, 0);
        assert_eq!(
            snapshot
                .apartments
                .get("ops_agent")
                .expect("apartment snapshot present")
                .process_generation,
            7
        );
        assert!(temp_dir
            .path()
            .join("runtime_snapshot.json")
            .exists());
        assert!(temp_dir
            .path()
            .join("services/web_portal/2026.02.0/runtime_plan.json")
            .exists());
        assert!(temp_dir
            .path()
            .join("services/web_portal/2026.02.0/deployment_bundle.json")
            .exists());
        assert!(temp_dir.path().join("journals").exists());
        assert!(temp_dir.path().join("checkpoints").exists());
        assert!(temp_dir.path().join("secrets").exists());
        assert!(temp_dir
            .path()
            .join("apartments/ops_agent/runtime_plan.json")
            .exists());
        assert!(temp_dir
            .path()
            .join("apartments/ops_agent/apartment_manifest.json")
            .exists());
        Ok(())
    }

    #[test]
    fn reconcile_once_prunes_stale_materializations_and_reports_missing_bundle_cache() -> Result<()> {
        let mut state = test_state()?;
        let bundle = load_deployment_bundle_fixture()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
                    (
                        bundle.service.service_name.to_string(),
                        bundle.service.service_version.clone(),
                    ),
                    bundle.clone(),
                );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    bundle.service.service_name.clone(),
                    sample_deployment_state(&bundle),
                );
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), sample_runtime_state(&bundle));
        }

        let temp_dir = tempfile::tempdir()?;
        let stale_dir = temp_dir.path().join("services/stale_service/stale_version");
        fs::create_dir_all(&stale_dir)?;
        fs::write(stale_dir.join("runtime_plan.json"), "{}")?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let bundle_plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("bundle plan present");
        assert!(!bundle_plan.bundle_available_locally);
        assert!(bundle_plan
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == SoraArtifactKindV1::Bundle && !artifact.available_locally));
        assert!(!temp_dir.path().join("services/stale_service").exists());
        Ok(())
    }

    #[test]
    fn reconcile_once_rejects_unsupported_native_process_runtime() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.runtime = SoraContainerRuntimeV1::NativeProcess;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
                    (
                        bundle.service.service_name.to_string(),
                        bundle.service.service_version.clone(),
                    ),
                    bundle.clone(),
                );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    bundle.service.service_name.clone(),
                    sample_deployment_state(&bundle),
                );
        }

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        let error = manager
            .reconcile_once()
            .expect_err("native-process revisions must be rejected during activation");
        assert!(
            error.to_string().contains("admits only `Ivm`"),
            "unexpected reconcile error: {error:?}"
        );
        Ok(())
    }

    #[test]
    fn restore_persisted_snapshot_preserves_last_snapshot_if_reconcile_fails() -> Result<()> {
        let mut state = test_state()?;
        let bundle = load_deployment_bundle_fixture()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    bundle.service.service_name.clone(),
                    sample_deployment_state(&bundle),
                );
        }

        let temp_dir = tempfile::tempdir()?;
        let expected_snapshot = SoracloudRuntimeSnapshot {
            schema_version: SoracloudRuntimeSnapshot::default().schema_version,
            observed_height: 77,
            observed_block_hash: Some(Hash::prehashed([0x55; Hash::LENGTH]).to_string()),
            services: BTreeMap::from([(
                "restored_service".to_string(),
                BTreeMap::from([(
                    "2026.03.0".to_string(),
                    SoracloudRuntimeServicePlan {
                        service_name: "restored_service".to_string(),
                        service_version: "2026.03.0".to_string(),
                        role: SoracloudRuntimeRevisionRole::Active,
                        traffic_percent: 100,
                        runtime: SoraContainerRuntimeV1::Ivm,
                        bundle_hash: Hash::prehashed([0x33; Hash::LENGTH]).to_string(),
                        bundle_path: "sorafs://restored.bundle".to_string(),
                        entrypoint: "main".to_string(),
                        bundle_cache_path: temp_dir
                            .path()
                            .join("artifacts/restored_bundle")
                            .display()
                            .to_string(),
                        bundle_available_locally: true,
                        process_generation: Some(9),
                        health_status: SoraServiceHealthStatusV1::Healthy,
                        load_factor_bps: 250,
                        reported_pending_mailbox_messages: 2,
                        authoritative_pending_mailbox_messages: 2,
                        rollout_handle: None,
                        materialization_dir: temp_dir
                            .path()
                            .join("services/restored_service/2026.03.0")
                            .display()
                            .to_string(),
                        mailboxes: Vec::new(),
                        artifacts: Vec::new(),
                    },
                )]),
            )]),
            apartments: BTreeMap::new(),
        };
        write_json_atomic(
            temp_dir.path().join("runtime_snapshot.json").as_path(),
            &expected_snapshot,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        assert!(manager.restore_persisted_snapshot()?);
        let error = manager
            .reconcile_once()
            .expect_err("reconcile should fail without the admitted revision bundle");
        assert!(
            error
                .to_string()
                .contains("references missing admitted revision"),
            "unexpected reconcile error: {error:?}"
        );
        assert_eq!(manager.snapshot.read().clone(), expected_snapshot);
        Ok(())
    }

    #[test]
    fn execute_local_read_serves_hydrated_asset_with_committed_binding() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = b"ivm bundle bytes".to_vec();
        let asset_bytes = b"<html><body>portal</body></html>".to_vec();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        bundle.service.artifacts[0].artifact_hash = Hash::new(&asset_bytes);
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.service.artifacts[0].artifact_hash)),
            &asset_bytes,
        )?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
                    (
                        bundle.service.service_name.to_string(),
                        bundle.service.service_version.clone(),
                    ),
                    bundle.clone(),
                );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let response = handle.execute_local_read(SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: bundle.service.service_name.to_string(),
            service_version: bundle.service.service_version.clone(),
            handler_name: "assets".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Asset,
            request_method: "GET".to_owned(),
            request_path: "/app/assets".to_owned(),
            handler_path: "/".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: Vec::new(),
            request_commitment: Hash::new(b"asset-request"),
        })
        .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert_eq!(response.response_bytes, asset_bytes);
        assert_eq!(
            response.content_type.as_deref(),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(response.certified_by, SoraCertifiedResponsePolicyV1::StateCommitment);
        assert!(response.runtime_receipt.is_none());
        assert_eq!(response.bindings.len(), 1);
        assert_eq!(
            response.bindings[0].artifact_hash,
            Some(bundle.service.artifacts[0].artifact_hash)
        );
        Ok(())
    }

    #[test]
    fn execute_local_read_returns_query_metadata_and_audit_receipt() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = b"ivm bundle bytes".to_vec();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        let deployment = sample_deployment_state(&bundle);
        let runtime = sample_runtime_state(&bundle);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
                    (
                        bundle.service.service_name.to_string(),
                        bundle.service.service_version.clone(),
                    ),
                    bundle.clone(),
                );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), runtime);
            world
                .soracloud_service_state_entries_mut_for_testing()
                .insert(
                    (
                        bundle.service.service_name.to_string(),
                        "session_store".to_owned(),
                        "/state/session/alice".to_owned(),
                    ),
                    SoraServiceStateEntryV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                        service_name: bundle.service.service_name.clone(),
                        service_version: bundle.service.service_version.clone(),
                        binding_name: "session_store".parse().expect("valid binding"),
                        state_key: "/state/session/alice".to_owned(),
                        encryption: iroha_data_model::soracloud::SoraStateEncryptionV1::ClientCiphertext,
                        payload_bytes: std::num::NonZeroU64::new(64).expect("nonzero"),
                        payload_commitment: Hash::new(b"alice-session"),
                        last_update_sequence: 4,
                        governance_tx_hash: Hash::new(b"gov-session"),
                        source_action: SoraServiceLifecycleActionV1::StateMutation,
                    },
                );
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let response = handle.execute_local_read(SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: bundle.service.service_name.to_string(),
            service_version: bundle.service.service_version.clone(),
            handler_name: "query".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "GET".to_owned(),
            request_path: "/app/query".to_owned(),
            handler_path: "/".to_owned(),
            request_query: Some("binding=session_store".to_owned()),
            request_headers: BTreeMap::new(),
            request_body: Vec::new(),
            request_commitment: Hash::new(b"query-request"),
        })
        .map_err(|error| eyre::eyre!("{error:?}"))?;

        let decoded: LocalQueryResponse = norito::json::from_slice(&response.response_bytes)?;
        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0].binding_name, "session_store");
        assert_eq!(decoded.entries[0].state_key, "/state/session/alice");
        assert_eq!(response.certified_by, SoraCertifiedResponsePolicyV1::AuditReceipt);
        assert!(response.runtime_receipt.is_some());
        assert_eq!(response.bindings.len(), 1);
        assert_eq!(
            response.bindings[0].state_key.as_deref(),
            Some("/state/session/alice")
        );
        Ok(())
    }

    #[test]
    fn execute_ordered_mailbox_runs_update_handler_from_admitted_ivm_bundle() -> Result<()> {
        let state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let artifact_bytes =
            simple_soracloud_contract_artifact(&["apply_update", "apply_private_update"]);
        bundle.container.bundle_hash = Hash::new(&artifact_bytes);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &artifact_bytes,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = sample_ordered_mailbox_request(
            &bundle,
            "update",
            sample_mailbox_message(&bundle, "update", b"hello-update".to_vec()),
        );

        let result = handle
            .execute_ordered_mailbox(request.clone())
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert!(result.state_mutations.is_empty());
        assert!(result.outbound_mailbox_messages.is_empty());
        let runtime_state = result.runtime_state.expect("runtime state");
        assert_eq!(runtime_state.health_status, SoraServiceHealthStatusV1::Healthy);
        assert_eq!(runtime_state.pending_mailbox_message_count, 0);
        assert_eq!(
            result.runtime_receipt.handler_class,
            SoraServiceHandlerClassV1::Update
        );
        assert_eq!(
            result.runtime_receipt.request_commitment,
            request.mailbox_message.payload_commitment
        );
        assert_eq!(
            result.runtime_receipt.mailbox_message_id,
            Some(request.mailbox_message.message_id)
        );
        assert_ne!(result.runtime_receipt.result_commitment, Hash::prehashed([0; Hash::LENGTH]));
        Ok(())
    }

    #[test]
    fn execute_ordered_mailbox_runs_private_update_handler_from_admitted_ivm_bundle() -> Result<()> {
        let state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let artifact_bytes =
            simple_soracloud_contract_artifact(&["apply_update", "apply_private_update"]);
        bundle.container.bundle_hash = Hash::new(&artifact_bytes);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &artifact_bytes,
        )?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"secret-input".to_vec()),
        );

        let result = handle
            .execute_ordered_mailbox(request)
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert!(result.state_mutations.is_empty());
        assert!(result.outbound_mailbox_messages.is_empty());
        assert_eq!(
            result.runtime_receipt.handler_class,
            SoraServiceHandlerClassV1::PrivateUpdate
        );
        assert_eq!(
            result.runtime_state.expect("runtime state").health_status,
            SoraServiceHealthStatusV1::Healthy
        );
        Ok(())
    }

    #[test]
    fn execute_ordered_mailbox_returns_deterministic_failure_for_missing_bundle_cache() -> Result<()> {
        let state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let artifact_bytes = simple_soracloud_contract_artifact(&["apply_update"]);
        bundle.container.bundle_hash = Hash::new(&artifact_bytes);
        let temp_dir = tempfile::tempdir()?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        let handle = test_runtime_handle(&manager, Arc::clone(&state));
        let request = sample_ordered_mailbox_request(
            &bundle,
            "update",
            sample_mailbox_message(&bundle, "update", b"missing-bundle".to_vec()),
        );

        let result = handle
            .execute_ordered_mailbox(request)
            .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert!(result.state_mutations.is_empty());
        assert!(result.outbound_mailbox_messages.is_empty());
        assert_eq!(
            result.runtime_state.expect("runtime state").health_status,
            SoraServiceHealthStatusV1::Degraded
        );
        assert_eq!(result.runtime_receipt.journal_artifact_hash, None);
        assert_eq!(result.runtime_receipt.checkpoint_artifact_hash, None);
        Ok(())
    }

    #[test]
    fn execute_local_read_fails_closed_when_runtime_snapshot_is_behind() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = b"ivm bundle bytes".to_vec();
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
            &bundle_bytes,
        )?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
                    (
                        bundle.service.service_name.to_string(),
                        bundle.service.service_version.clone(),
                    ),
                    bundle.clone(),
                );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    bundle.service.service_name.clone(),
                    sample_deployment_state(&bundle),
                );
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(bundle.service.service_name.clone(), sample_runtime_state(&bundle));
        }
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        manager.snapshot.write().observed_height = 99;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let error = handle
            .execute_local_read(SoracloudLocalReadRequest {
                observed_height: 0,
                observed_block_hash: None,
                service_name: bundle.service.service_name.to_string(),
                service_version: bundle.service.service_version.clone(),
                handler_name: "query".to_owned(),
                handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
                request_method: "GET".to_owned(),
                request_path: "/app/query".to_owned(),
                handler_path: "/".to_owned(),
                request_query: None,
                request_headers: BTreeMap::new(),
                request_body: Vec::new(),
                request_commitment: Hash::new(b"stale-query"),
            })
            .expect_err("stale runtime snapshots must fail closed");
        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        Ok(())
    }

    #[test]
    fn execute_apartment_returns_authoritative_status_and_commitment() -> Result<()> {
        let mut state = test_state()?;
        let apartment = sample_agent_record()?;
        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_agent_apartments_mut_for_testing()
                .insert(apartment.manifest.apartment_name.to_string(), apartment.clone());
        }
        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        );
        manager.reconcile_once()?;
        let handle = test_runtime_handle(&manager, Arc::clone(&state));

        let result = handle.execute_apartment(SoracloudApartmentExecutionRequest {
            observed_height: 0,
            observed_block_hash: None,
            apartment_name: apartment.manifest.apartment_name.to_string(),
            process_generation: apartment.process_generation,
            operation: "checkpoint".to_owned(),
            request_commitment: Hash::new(b"checkpoint-request"),
        })
        .map_err(|error| eyre::eyre!("{error:?}"))?;

        assert_eq!(result.status, apartment.status);
        assert!(result.checkpoint_artifact_hash.is_none());
        assert!(result.journal_artifact_hash.is_none());
        assert_ne!(result.result_commitment, Hash::new(b"checkpoint-request"));
        Ok(())
    }
}
