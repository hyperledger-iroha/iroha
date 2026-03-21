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
    sorafs::pin_registry::ManifestDigest,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_torii::sorafs::{
    EndpointKind, ProviderAdvertCache, ReplicationOrderV1, TransportProtocol,
    api::StorageManifestResponseDto,
};
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
use sorafs_node::store::StoredManifest;
use tokio::{sync::RwLock as AsyncRwLock, task::JoinHandle};

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
    sorafs_node: Option<sorafs_node::NodeHandle>,
    sorafs_provider_cache: Option<Arc<AsyncRwLock<ProviderAdvertCache>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteHydrationSource {
    manifest_digest_hex: String,
    manifest_cid_hex: String,
    chunker_handle: Option<String>,
    provider_ids: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteHydrationPlan {
    manifest_id_hex: String,
    chunker_handle: String,
    content_length: u64,
    chunks: Vec<RemoteHydrationChunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteHydrationChunk {
    offset: u64,
    length: u32,
    digest_hex: String,
}

impl SoracloudRuntimeManager {
    /// Construct the runtime manager for the supplied node state.
    #[must_use]
    pub fn new(config: SoracloudRuntimeManagerConfig, state: Arc<State>) -> Self {
        Self {
            config,
            state,
            snapshot: Arc::new(RwLock::new(SoracloudRuntimeSnapshot::default())),
            sorafs_node: None,
            sorafs_provider_cache: None,
        }
    }

    /// Attach the embedded SoraFS storage handle used for authoritative hydration.
    #[must_use]
    pub fn with_sorafs_node(mut self, sorafs_node: sorafs_node::NodeHandle) -> Self {
        self.sorafs_node = Some(sorafs_node);
        self
    }

    /// Attach the shared SoraFS provider-discovery cache used for remote hydration.
    #[must_use]
    pub fn with_sorafs_provider_cache(
        mut self,
        sorafs_provider_cache: Arc<AsyncRwLock<ProviderAdvertCache>>,
    ) -> Self {
        self.sorafs_provider_cache = Some(sorafs_provider_cache);
        self
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
        let initial_snapshot = build_runtime_snapshot(
            &view,
            &bundle_registry,
            &self.config.state_dir,
            self.artifacts_root(),
        )?;

        self.write_service_materializations(&initial_snapshot, &bundle_registry)?;
        self.write_apartment_materializations(&initial_snapshot, &view)?;
        self.prune_stale_service_materializations(&initial_snapshot)?;
        self.prune_stale_apartment_materializations(&initial_snapshot)?;
        self.hydrate_missing_artifacts(&view, &initial_snapshot)?;
        self.enforce_cache_budgets(&view, &initial_snapshot)?;
        let snapshot = build_runtime_snapshot(
            &view,
            &bundle_registry,
            &self.config.state_dir,
            self.artifacts_root(),
        )?;
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

    fn hydrate_missing_artifacts(
        &self,
        view: &StateView<'_>,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let stored_manifests = if let Some(sorafs_node) = self.sorafs_node.as_ref() {
            if sorafs_node.is_enabled() {
                sorafs_node
                    .stored_manifests()
                    .wrap_err("list stored SoraFS manifests for Soracloud hydration")?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let remote_sources = collect_remote_hydration_sources(view, &self.state);
        let mut missing = BTreeMap::<String, (Hash, String)>::new();
        let mut hydrated_payloads = BTreeMap::<Hash, Option<Vec<u8>>>::new();
        for versions in snapshot.services.values() {
            for plan in versions.values() {
                for artifact in &plan.artifacts {
                    if artifact.available_locally {
                        continue;
                    }
                    let artifact_hash = Hash::from_str(&artifact.artifact_hash).wrap_err_with(|| {
                        format!("parse Soracloud artifact hash `{}`", artifact.artifact_hash)
                    })?;
                    missing
                        .entry(artifact.local_cache_path.clone())
                        .or_insert((artifact_hash, artifact.artifact_path.clone()));
                }
            }
        }

        for (local_cache_path, (artifact_hash, artifact_path)) in missing {
            let cache_path = PathBuf::from(&local_cache_path);
            if cache_path.exists() {
                continue;
            }
            let payload = if let Some(cached) = hydrated_payloads.get(&artifact_hash) {
                cached.clone()
            } else {
                let resolved = self.read_committed_sorafs_payload(
                    view,
                    &stored_manifests,
                    &remote_sources,
                    artifact_hash,
                )?;
                hydrated_payloads.insert(artifact_hash, resolved.clone());
                resolved
            };
            let Some(payload) = payload else {
                continue;
            };
            write_bytes_atomic(&cache_path, &payload).wrap_err_with(|| {
                format!(
                    "persist hydrated Soracloud artifact `{artifact_path}` at {}",
                    cache_path.display()
                )
            })?;
        }

        Ok(())
    }

    fn read_committed_sorafs_payload(
        &self,
        view: &StateView<'_>,
        stored_manifests: &[StoredManifest],
        remote_sources: &[RemoteHydrationSource],
        expected_hash: Hash,
    ) -> eyre::Result<Option<Vec<u8>>> {
        if let Some(sorafs_node) = self.sorafs_node.as_ref() {
            for manifest in stored_manifests {
                if !manifest_is_committed(view, &self.state, manifest.manifest_digest()) {
                    continue;
                }
                let Ok(content_length) = usize::try_from(manifest.content_length()) else {
                    iroha_logger::warn!(
                        manifest_id = %manifest.manifest_id(),
                        content_length = manifest.content_length(),
                        "skipping Soracloud hydration candidate with oversized SoraFS payload"
                    );
                    continue;
                };
                let payload =
                    match sorafs_node.read_payload_range(manifest.manifest_id(), 0, content_length) {
                        Ok(payload) => payload,
                        Err(error) => {
                            iroha_logger::warn!(
                                ?error,
                                manifest_id = %manifest.manifest_id(),
                                "failed to read committed SoraFS payload during Soracloud hydration"
                            );
                            continue;
                        }
                    };
                if Hash::new(&payload) == expected_hash {
                    return Ok(Some(payload));
                }
            }
        }
        if let Some(payload) =
            self.read_committed_remote_sorafs_payload(remote_sources, expected_hash)?
        {
            return Ok(Some(payload));
        }
        Ok(None)
    }

    fn read_committed_remote_sorafs_payload(
        &self,
        remote_sources: &[RemoteHydrationSource],
        expected_hash: Hash,
    ) -> eyre::Result<Option<Vec<u8>>> {
        let Some(_cache) = self.sorafs_provider_cache.as_ref() else {
            return Ok(None);
        };
        if remote_sources.is_empty() {
            return Ok(None);
        }

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .wrap_err("build Soracloud remote hydration HTTP client")?;

        for source in remote_sources {
            for provider_id in &source.provider_ids {
                let Some(base_url) = self.remote_provider_base_url(provider_id) else {
                    continue;
                };
                let Some(manifest) =
                    self.fetch_remote_manifest_metadata(&client, &base_url, source)
                else {
                    continue;
                };
                if !manifest
                    .manifest_digest_hex
                    .eq_ignore_ascii_case(&source.manifest_digest_hex)
                {
                    continue;
                }
                if let Some(expected_chunker) = source.chunker_handle.as_ref()
                    && !manifest
                        .chunk_profile_handle
                        .eq_ignore_ascii_case(expected_chunker)
                {
                    continue;
                }

                let Some(plan) =
                    self.fetch_remote_hydration_plan(&client, &base_url, &source.manifest_cid_hex)
                else {
                    continue;
                };
                if !plan
                    .chunker_handle
                    .eq_ignore_ascii_case(&manifest.chunk_profile_handle)
                {
                    continue;
                }

                let client_id = "soracloud-runtime-hydration";
                let nonce = remote_hydration_nonce(&source.manifest_cid_hex, provider_id, expected_hash);
                let Some(stream_token) = self.fetch_remote_stream_token(
                    &client,
                    &base_url,
                    &source.manifest_cid_hex,
                    provider_id,
                    &plan,
                    client_id,
                    &nonce,
                ) else {
                    continue;
                };

                let Ok(capacity) = usize::try_from(plan.content_length) else {
                    iroha_logger::warn!(
                        manifest_digest = %source.manifest_digest_hex,
                        manifest_cid = %source.manifest_cid_hex,
                        provider_id_hex = %hex::encode(provider_id),
                        content_length = plan.content_length,
                        "skipping remote Soracloud hydration candidate with oversized payload"
                    );
                    continue;
                };

                let mut payload = Vec::with_capacity(capacity);
                let mut cursor = 0_u64;
                let mut fetch_failed = false;
                for chunk in &plan.chunks {
                    if chunk.offset != cursor {
                        fetch_failed = true;
                        break;
                    }
                    let Some(bytes) = self.fetch_remote_chunk(
                        &client,
                        &base_url,
                        &plan,
                        chunk,
                        &stream_token,
                        client_id,
                        &nonce,
                    ) else {
                        fetch_failed = true;
                        break;
                    };
                    if bytes.len() != usize::try_from(chunk.length).unwrap_or(usize::MAX) {
                        fetch_failed = true;
                        break;
                    }
                    cursor = cursor.saturating_add(bytes.len() as u64);
                    payload.extend_from_slice(&bytes);
                }
                if fetch_failed || cursor != plan.content_length {
                    continue;
                }
                if Hash::new(&payload) == expected_hash {
                    return Ok(Some(payload));
                }
            }
        }

        Ok(None)
    }

    fn remote_provider_base_url(&self, provider_id: &[u8; 32]) -> Option<reqwest::Url> {
        let cache = self.sorafs_provider_cache.as_ref()?;
        let guard = cache.try_read().ok()?;
        let record = guard.record_by_provider(provider_id)?;
        let advert = record.advert();
        let supports_torii_http_range = advert
            .body
            .transport_hints
            .as_ref()
            .map_or(true, |hints| {
                hints.iter()
                    .any(|hint| hint.protocol == TransportProtocol::ToriiHttpRange)
            });
        if !supports_torii_http_range {
            return None;
        }
        let endpoint = advert
            .body
            .endpoints
            .iter()
            .find(|endpoint| endpoint.kind == EndpointKind::Torii)?;
        normalize_provider_base_url(&endpoint.host_pattern)
    }

    fn fetch_remote_manifest_metadata(
        &self,
        client: &reqwest::blocking::Client,
        base_url: &reqwest::Url,
        source: &RemoteHydrationSource,
    ) -> Option<StorageManifestResponseDto> {
        let url = match base_url.join(&format!(
            "v1/sorafs/storage/manifest/{}",
            source.manifest_cid_hex
        )) {
            Ok(url) => url,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    base_url = %base_url,
                    manifest_cid = %source.manifest_cid_hex,
                    "failed to build remote Soracloud hydration manifest URL"
                );
                return None;
            }
        };
        let response = match client.get(url.clone()).send() {
            Ok(response) => response,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "remote Soracloud hydration manifest request failed"
                );
                return None;
            }
        };
        if !response.status().is_success() {
            return None;
        }
        let body = match response.bytes() {
            Ok(body) => body,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to read remote Soracloud hydration manifest body"
                );
                return None;
            }
        };
        match norito::json::from_slice::<StorageManifestResponseDto>(&body) {
            Ok(dto) => Some(dto),
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to decode remote Soracloud hydration manifest response"
                );
                None
            }
        }
    }

    fn fetch_remote_hydration_plan(
        &self,
        client: &reqwest::blocking::Client,
        base_url: &reqwest::Url,
        manifest_id_hex: &str,
    ) -> Option<RemoteHydrationPlan> {
        let url = match base_url.join(&format!("v1/sorafs/storage/plan/{manifest_id_hex}")) {
            Ok(url) => url,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    base_url = %base_url,
                    manifest_id = %manifest_id_hex,
                    "failed to build remote Soracloud hydration plan URL"
                );
                return None;
            }
        };
        let response = match client.get(url.clone()).send() {
            Ok(response) => response,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "remote Soracloud hydration plan request failed"
                );
                return None;
            }
        };
        if !response.status().is_success() {
            return None;
        }
        let body = match response.bytes() {
            Ok(body) => body,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to read remote Soracloud hydration plan body"
                );
                return None;
            }
        };
        parse_remote_hydration_plan(manifest_id_hex, &body).inspect_err(|error| {
            iroha_logger::debug!(
                ?error,
                url = %url,
                "failed to decode remote Soracloud hydration plan response"
            );
        }).ok()
    }

    fn fetch_remote_stream_token(
        &self,
        client: &reqwest::blocking::Client,
        base_url: &reqwest::Url,
        manifest_id_hex: &str,
        provider_id: &[u8; 32],
        plan: &RemoteHydrationPlan,
        client_id: &str,
        nonce: &str,
    ) -> Option<String> {
        let url = match base_url.join("v1/sorafs/storage/token") {
            Ok(url) => url,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    base_url = %base_url,
                    "failed to build remote Soracloud hydration token URL"
                );
                return None;
            }
        };
        let max_chunk_len = plan
            .chunks
            .iter()
            .map(|chunk| u64::from(chunk.length))
            .max()
            .unwrap_or(0);
        let mut request_body = norito::json::native::Map::new();
        request_body.insert(
            "manifest_id_hex".into(),
            norito::json::Value::from(manifest_id_hex),
        );
        request_body.insert(
            "provider_id_hex".into(),
            norito::json::Value::from(hex::encode(provider_id)),
        );
        request_body.insert("ttl_secs".into(), norito::json::Value::from(60_u64));
        request_body.insert("max_streams".into(), norito::json::Value::from(1_u16));
        request_body.insert(
            "rate_limit_bytes".into(),
            norito::json::Value::from(max_chunk_len.max(1)),
        );
        request_body.insert(
            "requests_per_minute".into(),
            norito::json::Value::from(
                u32::try_from(plan.chunks.len().saturating_add(8)).unwrap_or(u32::MAX),
            ),
        );
        let request_body = norito::json::Value::Object(request_body);
        let request_body = match norito::json::to_vec(&request_body) {
            Ok(body) => body,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to encode remote Soracloud hydration token request"
                );
                return None;
            }
        };
        let response = match client
            .post(url.clone())
            .header("X-SoraFS-Client", client_id)
            .header("X-SoraFS-Nonce", nonce)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(request_body)
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "remote Soracloud hydration token request failed"
                );
                return None;
            }
        };
        if !response.status().is_success() {
            return None;
        }
        let body = match response.bytes() {
            Ok(body) => body,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to read remote Soracloud hydration token body"
                );
                return None;
            }
        };
        let value: norito::json::Value = match norito::json::from_slice(&body) {
            Ok(value) => value,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to decode remote Soracloud hydration token response"
                );
                return None;
            }
        };
        value
            .get("token_base64")
            .and_then(norito::json::Value::as_str)
            .map(ToOwned::to_owned)
    }

    fn fetch_remote_chunk(
        &self,
        client: &reqwest::blocking::Client,
        base_url: &reqwest::Url,
        plan: &RemoteHydrationPlan,
        chunk: &RemoteHydrationChunk,
        stream_token: &str,
        client_id: &str,
        nonce: &str,
    ) -> Option<Vec<u8>> {
        let url = match base_url.join(&format!(
            "v1/sorafs/storage/chunk/{}/{}",
            plan.manifest_id_hex, chunk.digest_hex
        )) {
            Ok(url) => url,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    base_url = %base_url,
                    manifest_id = %plan.manifest_id_hex,
                    chunk_digest = %chunk.digest_hex,
                    "failed to build remote Soracloud hydration chunk URL"
                );
                return None;
            }
        };
        let response = match client
            .get(url.clone())
            .header("X-SoraFS-Stream-Token", stream_token)
            .header("X-SoraFS-Chunker", &plan.chunker_handle)
            .header("X-SoraFS-Client", client_id)
            .header("X-SoraFS-Nonce", nonce)
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "remote Soracloud hydration chunk request failed"
                );
                return None;
            }
        };
        if !response.status().is_success() {
            return None;
        }
        match response.bytes() {
            Ok(bytes) => Some(bytes.to_vec()),
            Err(error) => {
                iroha_logger::debug!(
                    ?error,
                    url = %url,
                    "failed to read remote Soracloud hydration chunk body"
                );
                None
            }
        }
    }

    fn enforce_cache_budgets(
        &self,
        view: &StateView<'_>,
        snapshot: &SoracloudRuntimeSnapshot,
    ) -> eyre::Result<()> {
        let artifact_observations = collect_artifact_cache_observations(view, snapshot);
        let artifact_candidates =
            collect_artifact_cache_candidates(self.artifacts_root().as_path(), &artifact_observations)?;
        let journal_sequences =
            collect_runtime_receipt_artifact_sequences(view, |receipt| receipt.journal_artifact_hash);
        let checkpoint_sequences = collect_runtime_receipt_artifact_sequences(view, |receipt| {
            receipt.checkpoint_artifact_hash
        });

        prune_cache_bucket(
            artifact_candidates.bundle,
            self.config.cache_budgets.bundle_bytes.get(),
        )?;
        prune_cache_bucket(
            artifact_candidates.static_asset,
            self.config.cache_budgets.static_asset_bytes.get(),
        )?;
        let mut journal_candidates = artifact_candidates.journal;
        journal_candidates.extend(collect_fixed_bucket_candidates(
            self.journals_root().as_path(),
            "journals",
            &journal_sequences,
        )?);
        prune_cache_bucket(
            journal_candidates,
            self.config.cache_budgets.journal_bytes.get(),
        )?;
        let mut checkpoint_candidates = artifact_candidates.checkpoint;
        checkpoint_candidates.extend(collect_fixed_bucket_candidates(
            self.checkpoints_root().as_path(),
            "checkpoints",
            &checkpoint_sequences,
        )?);
        prune_cache_bucket(
            checkpoint_candidates,
            self.config.cache_budgets.checkpoint_bytes.get(),
        )?;
        prune_cache_bucket(
            artifact_candidates.model_artifact,
            self.config.cache_budgets.model_artifact_bytes.get(),
        )?;
        prune_cache_bucket(
            artifact_candidates.model_weight,
            self.config.cache_budgets.model_weight_bytes.get(),
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CacheObservationMetadata {
    bucket: RuntimeCacheBucket,
    observation_sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RuntimeCacheBucket {
    Bundle,
    StaticAsset,
    Journal,
    Checkpoint,
    ModelArtifact,
    ModelWeight,
}

impl RuntimeCacheBucket {
    const fn priority(self) -> u8 {
        match self {
            Self::Bundle => 5,
            Self::ModelWeight => 4,
            Self::ModelArtifact => 3,
            Self::Journal => 2,
            Self::Checkpoint => 2,
            Self::StaticAsset => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CachePruneCandidate {
    path: PathBuf,
    stable_key: String,
    bytes: u64,
    observation_sequence: u64,
}

#[derive(Default)]
struct ArtifactCacheCandidates {
    bundle: Vec<CachePruneCandidate>,
    static_asset: Vec<CachePruneCandidate>,
    journal: Vec<CachePruneCandidate>,
    checkpoint: Vec<CachePruneCandidate>,
    model_artifact: Vec<CachePruneCandidate>,
    model_weight: Vec<CachePruneCandidate>,
}

impl ArtifactCacheCandidates {
    fn bucket_mut(&mut self, bucket: RuntimeCacheBucket) -> &mut Vec<CachePruneCandidate> {
        match bucket {
            RuntimeCacheBucket::Bundle => &mut self.bundle,
            RuntimeCacheBucket::StaticAsset => &mut self.static_asset,
            RuntimeCacheBucket::Journal => &mut self.journal,
            RuntimeCacheBucket::Checkpoint => &mut self.checkpoint,
            RuntimeCacheBucket::ModelArtifact => &mut self.model_artifact,
            RuntimeCacheBucket::ModelWeight => &mut self.model_weight,
        }
    }
}

fn collect_artifact_cache_observations(
    view: &StateView<'_>,
    snapshot: &SoracloudRuntimeSnapshot,
) -> BTreeMap<String, CacheObservationMetadata> {
    let world = view.world();
    let mut observations = BTreeMap::new();

    for (service_name, deployment) in world.soracloud_service_deployments().iter() {
        let service_name = service_name.to_string();
        let Some(versions) = snapshot.services.get(&service_name) else {
            continue;
        };
        for (_service_version, plan) in versions {
            let observation_sequence = match plan.role {
                SoracloudRuntimeRevisionRole::Active => deployment.process_started_sequence,
                SoracloudRuntimeRevisionRole::CanaryCandidate => deployment
                    .active_rollout
                    .as_ref()
                    .map_or(deployment.process_started_sequence, |rollout| {
                        rollout.updated_sequence
                    }),
            };
            for artifact in &plan.artifacts {
                upsert_cache_observation(
                    &mut observations,
                    sanitize_path_component(&artifact.artifact_hash),
                    runtime_cache_bucket_for_kind(artifact.kind),
                    observation_sequence,
                );
            }
        }
    }

    for (_, record) in world.soracloud_model_weight_versions().iter() {
        upsert_cache_observation(
            &mut observations,
            hash_cache_name(record.weight_artifact_hash),
            RuntimeCacheBucket::ModelWeight,
            record.promoted_sequence.unwrap_or(record.registered_sequence),
        );
    }

    for (_, record) in world.soracloud_model_artifacts().iter() {
        upsert_cache_observation(
            &mut observations,
            hash_cache_name(record.weight_artifact_hash),
            RuntimeCacheBucket::ModelArtifact,
            record.registered_sequence,
        );
    }

    observations
}

fn runtime_cache_bucket_for_kind(kind: SoraArtifactKindV1) -> RuntimeCacheBucket {
    match kind {
        SoraArtifactKindV1::Bundle => RuntimeCacheBucket::Bundle,
        SoraArtifactKindV1::StaticAsset => RuntimeCacheBucket::StaticAsset,
        SoraArtifactKindV1::Journal => RuntimeCacheBucket::Journal,
        SoraArtifactKindV1::Checkpoint => RuntimeCacheBucket::Checkpoint,
        SoraArtifactKindV1::ModelArtifact => RuntimeCacheBucket::ModelArtifact,
        SoraArtifactKindV1::ModelWeights => RuntimeCacheBucket::ModelWeight,
    }
}

fn upsert_cache_observation(
    observations: &mut BTreeMap<String, CacheObservationMetadata>,
    key: String,
    bucket: RuntimeCacheBucket,
    observation_sequence: u64,
) {
    match observations.entry(key) {
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let existing = entry.get_mut();
            existing.observation_sequence =
                existing.observation_sequence.max(observation_sequence);
            if bucket.priority() > existing.bucket.priority() {
                existing.bucket = bucket;
            }
        }
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(CacheObservationMetadata {
                bucket,
                observation_sequence,
            });
        }
    }
}

fn collect_artifact_cache_candidates(
    root: &Path,
    observations: &BTreeMap<String, CacheObservationMetadata>,
) -> eyre::Result<ArtifactCacheCandidates> {
    let mut candidates = ArtifactCacheCandidates::default();
    if !root.exists() {
        return Ok(candidates);
    }

    for entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let observation = observations
            .get(&file_name)
            .copied()
            .unwrap_or(CacheObservationMetadata {
                bucket: RuntimeCacheBucket::StaticAsset,
                observation_sequence: 0,
            });
        candidates
            .bucket_mut(observation.bucket)
            .push(CachePruneCandidate {
                path: entry.path(),
                stable_key: format!("artifacts/{file_name}"),
                bytes: entry.metadata()?.len(),
                observation_sequence: observation.observation_sequence,
            });
    }

    Ok(candidates)
}

fn collect_runtime_receipt_artifact_sequences(
    view: &StateView<'_>,
    select_hash: impl Fn(&SoraRuntimeReceiptV1) -> Option<Hash>,
) -> BTreeMap<String, u64> {
    let mut sequences = BTreeMap::new();
    for (_, receipt) in view.world().soracloud_runtime_receipts().iter() {
        let Some(hash) = select_hash(receipt) else {
            continue;
        };
        let key = hash_cache_name(hash);
        sequences
            .entry(key)
            .and_modify(|sequence: &mut u64| {
                *sequence = (*sequence).max(receipt.emitted_sequence);
            })
            .or_insert(receipt.emitted_sequence);
    }
    sequences
}

fn collect_fixed_bucket_candidates(
    root: &Path,
    bucket_name: &str,
    observation_sequences: &BTreeMap<String, u64>,
) -> eyre::Result<Vec<CachePruneCandidate>> {
    let mut candidates = Vec::new();
    if !root.exists() {
        return Ok(candidates);
    }

    for entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        candidates.push(CachePruneCandidate {
            path: entry.path(),
            stable_key: format!("{bucket_name}/{file_name}"),
            bytes: entry.metadata()?.len(),
            observation_sequence: observation_sequences.get(&file_name).copied().unwrap_or(0),
        });
    }

    Ok(candidates)
}

fn prune_cache_bucket(mut candidates: Vec<CachePruneCandidate>, budget_bytes: u64) -> eyre::Result<()> {
    let mut retained_bytes = candidates.iter().fold(0u64, |total, candidate| {
        total.saturating_add(candidate.bytes)
    });
    if retained_bytes <= budget_bytes {
        return Ok(());
    }

    candidates.sort_by(|left, right| {
        left.observation_sequence
            .cmp(&right.observation_sequence)
            .then_with(|| left.stable_key.cmp(&right.stable_key))
    });

    for candidate in candidates {
        if retained_bytes <= budget_bytes {
            break;
        }
        fs::remove_file(&candidate.path)
            .wrap_err_with(|| format!("prune Soracloud runtime cache {}", candidate.path.display()))?;
        retained_bytes = retained_bytes.saturating_sub(candidate.bytes);
    }

    Ok(())
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
            let active_runtime_state = runtime_state
                .as_ref()
                .filter(|state| state.active_service_version == service_version);
            let artifact_plans = build_artifact_plans(bundle, &artifacts_root);
            let hydration_complete = artifact_plans.iter().all(|artifact| artifact.available_locally);
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
                health_status: if hydration_complete {
                    active_runtime_state.map_or(SoraServiceHealthStatusV1::Hydrating, |state| {
                        state.health_status
                    })
                } else {
                    SoraServiceHealthStatusV1::Hydrating
                },
                load_factor_bps: active_runtime_state.map_or(0, |state| state.load_factor_bps),
                reported_pending_mailbox_messages: active_runtime_state
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
                artifacts: artifact_plans,
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

fn normalize_provider_base_url(raw: &str) -> Option<reqwest::Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('*') {
        return None;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = reqwest::Url::parse(&with_scheme).ok()?;
    let normalized_path = match url.path().trim_end_matches('/') {
        "" => "/".to_owned(),
        path => format!("{path}/"),
    };
    url.set_path(&normalized_path);
    Some(url)
}

fn remote_hydration_nonce(
    manifest_cid_hex: &str,
    provider_id: &[u8; 32],
    expected_hash: Hash,
) -> String {
    let manifest_prefix = manifest_cid_hex
        .chars()
        .take(16)
        .collect::<String>()
        .to_ascii_lowercase();
    let provider_prefix = hex::encode(provider_id).chars().take(8).collect::<String>();
    let hash_prefix = expected_hash
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("sc-{manifest_prefix}-{provider_prefix}-{hash_prefix}")
}

fn parse_remote_hydration_plan(
    manifest_id_hex: &str,
    body: &[u8],
) -> eyre::Result<RemoteHydrationPlan> {
    let value: norito::json::Value =
        norito::json::from_slice(body).wrap_err("decode remote plan response as JSON")?;
    let plan = value
        .get("plan")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre::eyre!("remote plan response missing `plan` object"))?;
    let chunker_handle = plan
        .get("chunk_profile_handle")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre::eyre!("remote plan response missing `chunk_profile_handle`"))?
        .to_owned();
    let content_length = plan
        .get("content_length")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre::eyre!("remote plan response missing `content_length`"))?;
    let chunks_value = plan
        .get("chunks")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre::eyre!("remote plan response missing `chunks` array"))?;
    if chunks_value.is_empty() && content_length != 0 {
        return Err(eyre::eyre!(
            "remote plan response returned zero chunks for non-empty payload"
        ));
    }

    let mut chunks = Vec::with_capacity(chunks_value.len());
    for (index, chunk) in chunks_value.iter().enumerate() {
        let chunk = chunk
            .as_object()
            .ok_or_else(|| eyre::eyre!("remote plan chunk {index} is not an object"))?;
        let offset = chunk
            .get("offset")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre::eyre!("remote plan chunk {index} missing `offset`"))?;
        let length = chunk
            .get("length")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre::eyre!("remote plan chunk {index} missing `length`"))?;
        let digest_hex = chunk
            .get("digest_blake3")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre::eyre!("remote plan chunk {index} missing `digest_blake3`"))?
            .to_ascii_lowercase();
        let digest_bytes = hex::decode(&digest_hex)
            .wrap_err_with(|| format!("decode remote plan chunk {index} digest"))?;
        if digest_bytes.len() != 32 {
            return Err(eyre::eyre!(
                "remote plan chunk {index} digest must decode to 32 bytes"
            ));
        }
        let length = u32::try_from(length)
            .wrap_err_with(|| format!("convert remote plan chunk {index} length to u32"))?;
        chunks.push(RemoteHydrationChunk {
            offset,
            length,
            digest_hex,
        });
    }

    Ok(RemoteHydrationPlan {
        manifest_id_hex: manifest_id_hex.to_owned(),
        chunker_handle,
        content_length,
        chunks,
    })
}

fn collect_remote_hydration_sources(
    view: &StateView<'_>,
    state: &State,
) -> Vec<RemoteHydrationSource> {
    let mut sources = BTreeMap::<(u8, u64, String, String), RemoteHydrationSource>::new();
    for (_order_id, record) in view.world().replication_orders().iter() {
        if !manifest_is_committed(view, state, record.manifest_digest.as_bytes()) {
            continue;
        }
        let order = match norito::decode_from_bytes::<ReplicationOrderV1>(&record.canonical_order) {
            Ok(order) => order,
            Err(error) => {
                iroha_logger::warn!(
                    ?error,
                    manifest_digest = %hex::encode(record.manifest_digest.as_bytes()),
                    "failed to decode canonical SoraFS replication order during Soracloud hydration"
                );
                continue;
            }
        };
        if order.manifest_cid.is_empty() || order.providers.is_empty() {
            continue;
        }

        let manifest_digest_hex = hex::encode(record.manifest_digest.as_bytes());
        let manifest_cid_hex = hex::encode(&order.manifest_cid);
        let chunker_handle = view
            .world()
            .pin_manifests()
            .get(&record.manifest_digest)
            .map(|manifest| manifest.chunker.to_handle());
        let status_rank = match record.status {
            iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Completed(_) => 0,
            iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Pending => 1,
            iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Expired(_) => 2,
        };
        let key = (
            status_rank,
            record.issued_epoch,
            manifest_digest_hex.clone(),
            manifest_cid_hex.clone(),
        );
        let entry = sources.entry(key).or_insert_with(|| RemoteHydrationSource {
            manifest_digest_hex,
            manifest_cid_hex,
            chunker_handle,
            provider_ids: Vec::new(),
        });
        for provider_id in order.providers {
            if !entry.provider_ids.contains(&provider_id) {
                entry.provider_ids.push(provider_id);
            }
        }
        entry.provider_ids.sort();
    }

    sources.into_values().collect()
}

fn manifest_is_committed(
    view: &StateView<'_>,
    state: &State,
    manifest_digest: &[u8; 32],
) -> bool {
    let digest = ManifestDigest::new(*manifest_digest);
    let has_active_pin = view
        .world()
        .pin_manifests()
        .get(&digest)
        .is_some_and(|record| record.status.is_active());
    has_active_pin || state.find_da_commitment_by_manifest(&digest).is_some()
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

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path must have a parent"))?;
    fs::create_dir_all(parent)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, bytes)?;
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
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
        num::NonZeroU64,
        sync::{Arc, mpsc},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    use eyre::Result;
    use iroha_crypto::{Algorithm, PrivateKey, PublicKey, Signature};
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_data_model::{
        block::BlockHeader,
        metadata::Metadata,
        smart_contract::manifest::EntryPointKind,
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestDigest, PinManifestRecord, PinPolicy,
            ReplicationOrderId, ReplicationOrderRecord, ReplicationOrderStatus,
        },
        soracloud::{
            AgentApartmentManifestV1, SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
            SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1, SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            SORA_SERVICE_RUNTIME_STATE_VERSION_V1, SoraAgentPersistentStateV1,
            SoraAgentRuntimeStatusV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
            SoraRolloutStageV1, SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1,
            SoraServiceHealthStatusV1, SoraServiceMailboxMessageV1, SoraServiceRolloutStateV1,
            SoraServiceRuntimeStateV1,
        },
    };
    use iroha_test_samples::ALICE_ID;
    use iroha_torii::sorafs::AdmissionRegistry;
    use sorafs_car::CarBuildPlan;
    use sorafs_chunker::ChunkProfile;
    use sorafs_manifest::{
        BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder,
        PinPolicy as ManifestPinPolicy, PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
        PROVIDER_ADMISSION_PROPOSAL_VERSION_V1, PROVIDER_ADVERT_VERSION_V1, AdvertEndpoint,
        AvailabilityTier, CapabilityTlv, CapabilityType, CouncilSignature,
        EndpointAdmissionV1, EndpointAttestationKind, EndpointAttestationV1, EndpointMetadata,
        EndpointMetadataKey, PathDiversityPolicy, ProviderAdmissionEnvelopeV1,
        ProviderAdmissionProposalV1, ProviderAdvertBodyV1, ProviderAdvertV1,
        ProviderCapabilityRangeV1, QosHints, RendezvousTopic, SignatureAlgorithm, StakePointer,
        StreamBudgetV1, TransportHintV1, compute_advert_body_digest, compute_proposal_digest,
    };
    use sorafs_node::{NodeHandle, config::StorageConfig};

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

    fn spawn_http_fixture(body: Vec<u8>) -> Result<(String, std::thread::JoinHandle<()>)> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 1024];
                let _ = std::io::Read::read(&mut stream, &mut request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::write_all(&mut stream, &body);
            }
        });
        Ok((format!("http://{address}/fixture"), handle))
    }

    #[derive(Clone)]
    struct RemoteManifestFixture {
        manifest_digest: ManifestDigest,
        order_id: ReplicationOrderId,
        issued_epoch: u64,
        canonical_order: Vec<u8>,
        manifest_id_hex: String,
        manifest_response_body: Vec<u8>,
        plan_response_body: Vec<u8>,
        chunk_path: String,
        payload: Vec<u8>,
    }

    #[derive(Clone)]
    struct HttpFixtureResponse {
        status_code: u16,
        content_type: &'static str,
        body: Vec<u8>,
    }

    impl HttpFixtureResponse {
        fn json(body: Vec<u8>) -> Self {
            Self {
                status_code: 200,
                content_type: "application/json",
                body,
            }
        }

        fn binary(body: Vec<u8>) -> Self {
            Self {
                status_code: 200,
                content_type: "application/octet-stream",
                body,
            }
        }

        fn not_found() -> Self {
            Self {
                status_code: 404,
                content_type: "text/plain; charset=utf-8",
                body: b"not found".to_vec(),
            }
        }
    }

    struct HttpRouteFixture {
        base_url: String,
        stop_tx: mpsc::Sender<()>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for HttpRouteFixture {
        fn drop(&mut self) {
            let _ = self.stop_tx.send(());
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn fixed_chunker_handle() -> ChunkerProfileHandle {
        ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: BLAKE3_256_MULTIHASH_CODE,
        }
    }

    fn build_remote_manifest_fixture(
        payload: &[u8],
        provider_id: [u8; 32],
        order_seed: u8,
    ) -> Result<RemoteManifestFixture> {
        let (_plan, manifest) = build_sorafs_manifest(payload)?;
        let manifest_digest = ManifestDigest::from_manifest(&manifest)?;
        let manifest_id_hex = hex::encode(&manifest.root_cid);
        let chunk_profile_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        let chunk_digest_hex = hex::encode(blake3::hash(payload).as_bytes());
        let manifest_response = StorageManifestResponseDto {
            manifest_id_hex: manifest_id_hex.clone(),
            manifest_b64: String::new(),
            manifest_digest_hex: hex::encode(manifest_digest.as_bytes()),
            payload_digest_hex: chunk_digest_hex.clone(),
            content_length: payload.len() as u64,
            chunk_count: 1,
            chunk_profile_handle: chunk_profile_handle.clone(),
            stored_at_unix_secs: 1,
        };
        let manifest_response_body = norito::json::to_vec(&manifest_response)?;
        let mut chunk_entry = norito::json::native::Map::new();
        chunk_entry.insert("chunk_index".into(), norito::json::Value::from(0_u64));
        chunk_entry.insert("offset".into(), norito::json::Value::from(0_u64));
        chunk_entry.insert(
            "length".into(),
            norito::json::Value::from(payload.len() as u64),
        );
        chunk_entry.insert(
            "digest_blake3".into(),
            norito::json::Value::from(chunk_digest_hex.clone()),
        );
        let mut plan = norito::json::native::Map::new();
        plan.insert("chunk_count".into(), norito::json::Value::from(1_u64));
        plan.insert(
            "content_length".into(),
            norito::json::Value::from(payload.len() as u64),
        );
        plan.insert(
            "payload_digest_blake3".into(),
            norito::json::Value::from(chunk_digest_hex.clone()),
        );
        plan.insert(
            "chunk_profile_handle".into(),
            norito::json::Value::from(chunk_profile_handle),
        );
        plan.insert(
            "chunk_digests_blake3".into(),
            norito::json::Value::Array(vec![norito::json::Value::from(
                chunk_digest_hex.clone(),
            )]),
        );
        plan.insert(
            "chunks".into(),
            norito::json::Value::Array(vec![norito::json::Value::Object(chunk_entry)]),
        );
        let mut plan_response = norito::json::native::Map::new();
        plan_response.insert(
            "manifest_id_hex".into(),
            norito::json::Value::from(manifest_id_hex.clone()),
        );
        plan_response.insert("plan".into(), norito::json::Value::Object(plan));
        let plan_response_body =
            norito::json::to_vec(&norito::json::Value::Object(plan_response))?;
        let order_id = ReplicationOrderId::new([order_seed; 32]);
        let canonical_order = norito::to_bytes(&ReplicationOrderV1 {
            order_id: *order_id.as_bytes(),
            manifest_cid: manifest.root_cid.clone(),
            providers: vec![provider_id],
            redundancy: 1,
            deadline: u64::from(order_seed) + 600,
            policy_hash: [0x91; 32],
        })?;
        Ok(RemoteManifestFixture {
            manifest_digest,
            order_id,
            issued_epoch: u64::from(order_seed),
            canonical_order,
            manifest_id_hex: manifest_id_hex.clone(),
            manifest_response_body,
            plan_response_body,
            chunk_path: format!(
                "/v1/sorafs/storage/chunk/{manifest_id_hex}/{chunk_digest_hex}"
            ),
            payload: payload.to_vec(),
        })
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> Result<(String, String)> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    buffer.extend_from_slice(&chunk[..read]);
                    if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(error) => return Err(error.into()),
            }
        }
        let request = String::from_utf8_lossy(&buffer);
        let request_line = request.lines().next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        Ok((
            parts.next().unwrap_or_default().to_owned(),
            parts.next().unwrap_or_default().to_owned(),
        ))
    }

    fn write_http_response(
        stream: &mut std::net::TcpStream,
        response: &HttpFixtureResponse,
    ) -> Result<()> {
        let reason = match response.status_code {
            200 => "OK",
            404 => "Not Found",
            _ => "Error",
        };
        let headers = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
            response.status_code,
            reason,
            response.body.len(),
            response.content_type,
        );
        stream.write_all(headers.as_bytes())?;
        stream.write_all(&response.body)?;
        Ok(())
    }

    fn spawn_remote_hydration_fixture(fixtures: &[RemoteManifestFixture]) -> Result<HttpRouteFixture> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let base_url = format!("http://{}", listener.local_addr()?);
        let mut routes = BTreeMap::<(String, String), HttpFixtureResponse>::new();
        let mut token_response = norito::json::native::Map::new();
        token_response.insert(
            "token_base64".into(),
            norito::json::Value::from("fixture-stream-token"),
        );
        routes.insert(
            ("POST".to_owned(), "/v1/sorafs/storage/token".to_owned()),
            HttpFixtureResponse::json(norito::json::to_vec(&norito::json::Value::Object(
                token_response,
            ))?),
        );
        for fixture in fixtures {
            routes.insert(
                (
                    "GET".to_owned(),
                    format!(
                        "/v1/sorafs/storage/manifest/{}",
                        fixture.manifest_id_hex
                    ),
                ),
                HttpFixtureResponse::json(fixture.manifest_response_body.clone()),
            );
            routes.insert(
                (
                    "GET".to_owned(),
                    format!("/v1/sorafs/storage/plan/{}", fixture.manifest_id_hex),
                ),
                HttpFixtureResponse::json(fixture.plan_response_body.clone()),
            );
            routes.insert(
                ("GET".to_owned(), fixture.chunk_path.clone()),
                HttpFixtureResponse::binary(fixture.payload.clone()),
            );
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let response = match read_http_request(&mut stream) {
                            Ok((method, path)) => routes
                                .get(&(method, path))
                                .cloned()
                                .unwrap_or_else(HttpFixtureResponse::not_found),
                            Err(_) => HttpFixtureResponse::not_found(),
                        };
                        let _ = write_http_response(&mut stream, &response);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(HttpRouteFixture {
            base_url,
            stop_tx,
            handle: Some(handle),
        })
    }

    fn test_provider_cache(
        base_url: &str,
        provider_id: [u8; 32],
    ) -> Result<Arc<AsyncRwLock<ProviderAdvertCache>>> {
        let advert_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0xA5; 32])?;
        let advert_public = PublicKey::from(advert_key.clone());
        let council_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x42; 32])?;
        let council_public = PublicKey::from(council_key.clone());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let issued_at = now.saturating_sub(60);
        let expires_at = issued_at + 600;

        let capabilities = vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: ProviderCapabilityRangeV1 {
                    max_chunk_span: 32,
                    min_granularity: 8,
                    supports_sparse_offsets: true,
                    requires_alignment: false,
                    supports_merkle_proof: true,
                }
                .to_bytes()?,
            },
        ];
        let stream_budget = Some(StreamBudgetV1 {
            max_in_flight: 8,
            max_bytes_per_sec: 8_388_608,
            burst_bytes: Some(1_048_576),
        });
        let transport_hints = Some(vec![TransportHintV1 {
            protocol: TransportProtocol::ToriiHttpRange,
            priority: 0,
        }]);
        let endpoint = AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: base_url.to_owned(),
            metadata: vec![EndpointMetadata {
                key: EndpointMetadataKey::Region,
                value: b"global".to_vec(),
            }],
        };
        let body = ProviderAdvertBodyV1 {
            provider_id,
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: Some(vec![
                "sorafs.sf1@1.0.0".to_owned(),
                "sorafs-sf1".to_owned(),
            ]),
            stake: StakePointer {
                pool_id: [0x21; 32],
                stake_amount: 1_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 1_000,
                max_concurrent_streams: 8,
            },
            capabilities: capabilities.clone(),
            endpoints: vec![endpoint.clone()],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".to_owned(),
                region: "global".to_owned(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 5,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: stream_budget.clone(),
            transport_hints: transport_hints.clone(),
        };
        let body_bytes = norito::to_bytes(&body)?;
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at,
            expires_at,
            body: body.clone(),
            signature: sorafs_manifest::AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: advert_public.to_bytes().1.to_vec(),
                signature: Signature::new(&advert_key, &body_bytes).payload().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let proposal = ProviderAdmissionProposalV1 {
            version: PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
            provider_id,
            profile_id: body.profile_id.clone(),
            profile_aliases: body.profile_aliases.clone(),
            stake: body.stake.clone(),
            capabilities: body.capabilities.clone(),
            endpoints: vec![EndpointAdmissionV1 {
                endpoint,
                attestation: EndpointAttestationV1 {
                    version: sorafs_manifest::ENDPOINT_ATTESTATION_VERSION_V1,
                    kind: EndpointAttestationKind::Mtls,
                    attested_at: issued_at.saturating_sub(10),
                    expires_at: expires_at + 60,
                    leaf_certificate: vec![0xAA],
                    intermediate_certificates: Vec::new(),
                    alpn_ids: vec!["h2".to_owned()],
                    report: Vec::new(),
                },
            }],
            advert_key: advert_public
                .to_bytes()
                .1
                .try_into()
                .expect("ed25519 key is 32 bytes"),
            jurisdiction_code: "US".to_owned(),
            contact_uri: Some("mailto:ops@example.test".to_owned()),
            stream_budget,
            transport_hints,
        };
        let proposal_digest = compute_proposal_digest(&proposal)?;
        let envelope = ProviderAdmissionEnvelopeV1 {
            version: PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
            proposal,
            proposal_digest,
            advert_body: body.clone(),
            advert_body_digest: compute_advert_body_digest(&body)?,
            issued_at,
            retention_epoch: expires_at + 600,
            council_signatures: vec![CouncilSignature {
                signer: council_public
                    .to_bytes()
                    .1
                    .try_into()
                    .expect("ed25519 key is 32 bytes"),
                signature: Signature::new(&council_key, &proposal_digest)
                    .payload()
                    .to_vec(),
            }],
            notes: None,
        };
        let admission = AdmissionRegistry::from_envelopes([envelope])?;
        let mut cache = ProviderAdvertCache::new(
            vec![
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
            Arc::new(admission),
        );
        cache
            .ingest(advert, issued_at.saturating_add(1))
            .map_err(|error| eyre::eyre!(error.to_string()))?;
        Ok(Arc::new(AsyncRwLock::new(cache)))
    }

    fn approve_remote_hydration_sources(
        state: &Arc<State>,
        fixtures: &[RemoteManifestFixture],
    ) -> Result<()> {
        let view = state.view();
        let next_height = NonZeroU64::new(
            u64::try_from(view.height())
                .unwrap_or(u64::MAX.saturating_sub(1))
                .saturating_add(1),
        )
        .expect("nonzero block height");
        let header = BlockHeader::new(next_height, view.latest_block_hash(), None, None, 0, 0);
        drop(view);

        let mut block = state.block(header);
        {
            let mut pin_manifests = block.world.pin_manifests_mut_for_testing().transaction();
            for fixture in fixtures {
                let mut record = PinManifestRecord::new(
                    fixture.manifest_digest,
                    fixed_chunker_handle(),
                    [0; 32],
                    PinPolicy::default(),
                    (*ALICE_ID).clone(),
                    fixture.issued_epoch,
                    None,
                    None,
                    Metadata::default(),
                );
                record.approve(fixture.issued_epoch, None);
                pin_manifests.insert(fixture.manifest_digest, record);
            }
            pin_manifests.apply();
        }
        {
            let mut replication_orders = block
                .world
                .replication_orders_mut_for_testing()
                .transaction();
            for fixture in fixtures {
                replication_orders.insert(
                    fixture.order_id,
                    ReplicationOrderRecord {
                        order_id: fixture.order_id,
                        manifest_digest: fixture.manifest_digest,
                        issued_by: (*ALICE_ID).clone(),
                        issued_epoch: fixture.issued_epoch,
                        deadline_epoch: fixture.issued_epoch + 600,
                        canonical_order: fixture.canonical_order.clone(),
                        status: ReplicationOrderStatus::Completed(fixture.issued_epoch + 1),
                    },
                );
            }
            replication_orders.apply();
        }
        block.commit()?;
        Ok(())
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

    fn assign_fixture_artifact_hashes(
        bundle: &mut SoraDeploymentBundleV1,
        bundle_bytes: &[u8],
        label: &str,
    ) -> Vec<Vec<u8>> {
        let service_name = bundle.service.service_name.to_string();
        bundle.container.bundle_hash = Hash::new(bundle_bytes);
        let mut payloads = Vec::with_capacity(bundle.service.artifacts.len());
        for (index, artifact) in bundle.service.artifacts.iter_mut().enumerate() {
            let payload = format!(
                "{label}:{service_name}:{index}:{}",
                artifact.artifact_path
            )
            .into_bytes();
            artifact.artifact_hash = Hash::new(&payload);
            payloads.push(payload);
        }
        payloads
    }

    fn seed_local_artifact_cache(
        artifacts_root: &Path,
        bundle_hash: Hash,
        bundle_bytes: &[u8],
        artifact_hashes_and_bytes: impl IntoIterator<Item = (Hash, Vec<u8>)>,
    ) -> Result<()> {
        fs::create_dir_all(artifacts_root)?;
        fs::write(artifacts_root.join(hash_cache_name(bundle_hash)), bundle_bytes)?;
        for (artifact_hash, payload) in artifact_hashes_and_bytes {
            fs::write(artifacts_root.join(hash_cache_name(artifact_hash)), payload)?;
        }
        Ok(())
    }

    fn test_sorafs_node(temp_dir: &tempfile::TempDir) -> NodeHandle {
        NodeHandle::new(
            StorageConfig::builder()
                .enabled(true)
                .data_dir(temp_dir.path().join("sorafs-storage"))
                .build(),
        )
    }

    fn build_sorafs_manifest(payload: &[u8]) -> Result<(CarBuildPlan, sorafs_manifest::ManifestV1)> {
        let plan = CarBuildPlan::single_file(payload)?;
        let digest = blake3::hash(payload);
        let manifest = ManifestBuilder::new()
            .root_cid(digest.as_bytes().to_vec())
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
            .content_length(plan.content_length)
            .car_digest(digest.into())
            .car_size(plan.content_length)
            .pin_policy(ManifestPinPolicy::default())
            .build()?;
        Ok((plan, manifest))
    }

    fn ingest_sorafs_payload(node: &NodeHandle, payload: &[u8]) -> Result<StoredManifest> {
        let (plan, manifest) = build_sorafs_manifest(payload)?;
        let mut reader = payload;
        let manifest_id = node.ingest_manifest(&manifest, &plan, &mut reader)?;
        node.manifest_metadata(&manifest_id).map_err(Into::into)
    }

    fn approve_sorafs_manifests(
        state: &Arc<State>,
        manifests: &[StoredManifest],
    ) -> Result<()> {
        let view = state.view();
        let next_height = NonZeroU64::new(
            u64::try_from(view.height())
                .unwrap_or(u64::MAX.saturating_sub(1))
                .saturating_add(1),
        )
        .expect("nonzero block height");
        let header = BlockHeader::new(next_height, view.latest_block_hash(), None, None, 0, 0);
        drop(view);

        let mut block = state.block(header);
        let mut pin_manifests = block.world.pin_manifests_mut_for_testing().transaction();
        for manifest in manifests {
            let digest = ManifestDigest::new(*manifest.manifest_digest());
            let mut record = PinManifestRecord::new(
                ManifestDigest::new(*manifest.manifest_digest()),
                ChunkerProfileHandle {
                    profile_id: 1,
                    namespace: "sorafs".to_owned(),
                    name: "sf1".to_owned(),
                    semver: "1.0.0".to_owned(),
                    multihash_code: BLAKE3_256_MULTIHASH_CODE,
                },
                [0; 32],
                PinPolicy::default(),
                (*ALICE_ID).clone(),
                1,
                None,
                None,
                Metadata::default(),
            );
            record.approve(1, None);
            pin_manifests.insert(digest, record);
        }
        pin_manifests.apply();
        block.commit()?;
        Ok(())
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
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "private_update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "persist-materialization");
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
        seed_local_artifact_cache(
            &temp_dir.path().join("artifacts"),
            bundle.container.bundle_hash,
            &bundle_bytes,
            bundle
                .service
                .artifacts
                .iter()
                .zip(artifact_payloads)
                .map(|(artifact, payload)| (artifact.artifact_hash, payload)),
        )?;
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
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update"]);
        let _artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "missing-bundle");
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
        assert_eq!(bundle_plan.health_status, SoraServiceHealthStatusV1::Hydrating);
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
    fn reconcile_once_prunes_cache_buckets_by_authoritative_sequence_and_refreshes_snapshot(
    ) -> Result<()> {
        let mut state = test_state()?;
        let mut active_bundle = load_deployment_bundle_fixture()?;
        let mut canary_bundle = active_bundle.clone();
        canary_bundle.service.service_version = "2026.03.0".to_string();
        canary_bundle.container.bundle_path = "/bundles/web_portal_canary.to".to_string();

        let active_bundle_bytes = simple_soracloud_contract_artifact(&["entry_active"]);
        let canary_bundle_bytes = simple_soracloud_contract_artifact(&["entry_canary"]);
        active_bundle.container.bundle_hash = Hash::new(&active_bundle_bytes);
        canary_bundle.container.bundle_hash = Hash::new(&canary_bundle_bytes);

        let active_asset_bytes = b"active-asset".to_vec();
        let canary_asset_bytes = b"canary-asset".to_vec();
        active_bundle.service.artifacts[0].artifact_hash = Hash::new(&active_asset_bytes);
        active_bundle.service.artifacts[0].artifact_path = "/public/active.html".to_string();
        canary_bundle.service.artifacts[0].artifact_hash = Hash::new(&canary_asset_bytes);
        canary_bundle.service.artifacts[0].artifact_path = "/public/canary.html".to_string();

        let mut deployment = sample_deployment_state(&active_bundle);
        deployment.revision_count = 2;
        deployment.active_rollout = Some(SoraServiceRolloutStateV1 {
            schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            rollout_handle: "rollout-2026-03".to_string(),
            baseline_version: Some(active_bundle.service.service_version.clone()),
            candidate_version: canary_bundle.service.service_version.clone(),
            canary_percent: 20,
            traffic_percent: 20,
            stage: SoraRolloutStageV1::Canary,
            health_failures: 0,
            max_health_failures: 3,
            health_window_secs: 60,
            created_sequence: 17,
            updated_sequence: 29,
        });
        deployment.last_rollout = deployment.active_rollout.clone();

        let mut runtime = sample_runtime_state(&active_bundle);
        runtime.rollout_handle = Some("rollout-2026-03".to_string());

        {
            let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
                    (
                        active_bundle.service.service_name.to_string(),
                        active_bundle.service.service_version.clone(),
                    ),
                    active_bundle.clone(),
                );
            world
                .soracloud_service_revisions_mut_for_testing()
                .insert(
                    (
                        canary_bundle.service.service_name.to_string(),
                        canary_bundle.service.service_version.clone(),
                    ),
                    canary_bundle.clone(),
                );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(active_bundle.service.service_name.clone(), deployment);
            world
                .soracloud_service_runtime_mut_for_testing()
                .insert(active_bundle.service.service_name.clone(), runtime);
        }

        let temp_dir = tempfile::tempdir()?;
        let artifacts_root = temp_dir.path().join("artifacts");
        fs::create_dir_all(&artifacts_root)?;
        fs::write(
            artifacts_root.join(hash_cache_name(active_bundle.container.bundle_hash)),
            &active_bundle_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(canary_bundle.container.bundle_hash)),
            &canary_bundle_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(active_bundle.service.artifacts[0].artifact_hash)),
            &active_asset_bytes,
        )?;
        fs::write(
            artifacts_root.join(hash_cache_name(canary_bundle.service.artifacts[0].artifact_hash)),
            &canary_asset_bytes,
        )?;

        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.cache_budgets.bundle_bytes =
            std::num::NonZeroU64::new(
                u64::try_from(active_bundle_bytes.len().max(canary_bundle_bytes.len()))
                    .expect("bundle size fits in u64"),
            )
            .expect("nonzero bundle budget");
        config.cache_budgets.static_asset_bytes =
            std::num::NonZeroU64::new(
                u64::try_from(active_asset_bytes.len().max(canary_asset_bytes.len()))
                    .expect("asset size fits in u64"),
            )
            .expect("nonzero asset budget");

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let active_plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("active service plan present");
        let canary_plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.03.0"))
            .expect("canary service plan present");
        let active_asset_plan = active_plan
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == SoraArtifactKindV1::StaticAsset)
            .expect("active static asset plan");
        let canary_asset_plan = canary_plan
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == SoraArtifactKindV1::StaticAsset)
            .expect("canary static asset plan");

        assert!(!active_plan.bundle_available_locally);
        assert!(canary_plan.bundle_available_locally);
        assert!(!active_asset_plan.available_locally);
        assert!(canary_asset_plan.available_locally);
        assert!(
            !artifacts_root
                .join(hash_cache_name(active_bundle.container.bundle_hash))
                .exists()
        );
        assert!(
            artifacts_root
                .join(hash_cache_name(canary_bundle.container.bundle_hash))
                .exists()
        );
        assert!(
            !artifacts_root
                .join(hash_cache_name(active_bundle.service.artifacts[0].artifact_hash))
                .exists()
        );
        assert!(
            artifacts_root
                .join(hash_cache_name(canary_bundle.service.artifacts[0].artifact_hash))
                .exists()
        );
        Ok(())
    }

    #[test]
    fn reconcile_once_prunes_tied_cache_candidates_by_stable_key() -> Result<()> {
        let state = test_state()?;
        let temp_dir = tempfile::tempdir()?;
        let journals_root = temp_dir.path().join("journals");
        fs::create_dir_all(&journals_root)?;

        let first_hash = Hash::new(b"journal-alpha");
        let second_hash = Hash::new(b"journal-omega");
        let payload = b"journal-entry".to_vec();
        let first_name = hash_cache_name(first_hash);
        let second_name = hash_cache_name(second_hash);
        let first_path = journals_root.join(&first_name);
        let second_path = journals_root.join(&second_name);
        fs::write(&first_path, &payload)?;
        fs::write(&second_path, &payload)?;

        let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf());
        config.cache_budgets.journal_bytes =
            std::num::NonZeroU64::new(u64::try_from(payload.len()).expect("payload size fits"))
                .expect("nonzero journal budget");

        let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
        manager.reconcile_once()?;

        let (removed, retained) = if first_name <= second_name {
            (first_path, second_path)
        } else {
            (second_path, first_path)
        };
        assert!(!removed.exists());
        assert!(retained.exists());
        Ok(())
    }

    #[test]
    fn reconcile_once_hydrates_missing_artifacts_from_committed_sorafs_store() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "private_update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "hydrated-from-sorafs");
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
        let sorafs_node = test_sorafs_node(&temp_dir);
        let mut committed_manifests = vec![ingest_sorafs_payload(&sorafs_node, &bundle_bytes)?];
        for payload in &artifact_payloads {
            committed_manifests.push(ingest_sorafs_payload(&sorafs_node, payload)?);
        }
        approve_sorafs_manifests(&state, &committed_manifests)?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node);
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("hydrated service plan");
        assert!(plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Healthy);
        assert!(plan.artifacts.iter().all(|artifact| artifact.available_locally));
        assert_eq!(
            fs::read(temp_dir.path().join("artifacts").join(hash_cache_name(bundle.container.bundle_hash)))?,
            bundle_bytes
        );
        for (artifact, payload) in bundle.service.artifacts.iter().zip(artifact_payloads) {
            assert_eq!(
                fs::read(temp_dir.path().join("artifacts").join(hash_cache_name(artifact.artifact_hash)))?,
                payload
            );
        }
        Ok(())
    }

    #[test]
    fn reconcile_once_hydrates_missing_artifacts_from_committed_remote_sorafs_provider(
    ) -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.service.artifacts.truncate(1);
        let bundle_bytes = simple_soracloud_contract_artifact(&["update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "remote-provider");
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

        let provider_id = [0x11; 32];
        let remote_payloads = std::iter::once(bundle_bytes.clone())
            .chain(artifact_payloads.iter().cloned())
            .collect::<Vec<_>>();
        let remote_fixtures = remote_payloads
            .iter()
            .enumerate()
            .map(|(index, payload)| {
                build_remote_manifest_fixture(
                    payload,
                    provider_id,
                    u8::try_from(index + 1).expect("fixture index fits in u8"),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        approve_remote_hydration_sources(&state, &remote_fixtures)?;

        let server = spawn_remote_hydration_fixture(&remote_fixtures)?;
        let provider_cache = test_provider_cache(&server.base_url, provider_id)?;

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_provider_cache(provider_cache);
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("hydrated service plan");
        assert!(plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Healthy);
        assert!(plan.artifacts.iter().all(|artifact| artifact.available_locally));
        assert_eq!(
            fs::read(
                temp_dir
                    .path()
                    .join("artifacts")
                    .join(hash_cache_name(bundle.container.bundle_hash))
            )?,
            bundle_bytes
        );
        for (artifact, payload) in bundle.service.artifacts.iter().zip(artifact_payloads) {
            assert_eq!(
                fs::read(
                    temp_dir
                        .path()
                        .join("artifacts")
                        .join(hash_cache_name(artifact.artifact_hash))
                )?,
                payload
            );
        }
        Ok(())
    }

    #[test]
    fn reconcile_once_skips_remote_sorafs_payloads_that_do_not_match_expected_hash() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.service.artifacts.clear();
        let bundle_bytes = simple_soracloud_contract_artifact(&["update"]);
        bundle.container.bundle_hash = Hash::new(&bundle_bytes);
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

        let provider_id = [0x11; 32];
        let remote_fixture =
            build_remote_manifest_fixture(b"wrong-remote-bundle", provider_id, 1)?;
        approve_remote_hydration_sources(&state, std::slice::from_ref(&remote_fixture))?;

        let server = spawn_remote_hydration_fixture(std::slice::from_ref(&remote_fixture))?;
        let provider_cache = test_provider_cache(&server.base_url, provider_id)?;

        let temp_dir = tempfile::tempdir()?;
        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_provider_cache(provider_cache);
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("service plan");
        assert!(!plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Hydrating);
        assert!(!temp_dir
            .path()
            .join("artifacts")
            .join(hash_cache_name(bundle.container.bundle_hash))
            .exists());
        Ok(())
    }

    #[test]
    fn reconcile_once_skips_uncommitted_sorafs_artifacts_and_keeps_service_hydrating() -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "uncommitted-sorafs");
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
        let sorafs_node = test_sorafs_node(&temp_dir);
        let _bundle_manifest = ingest_sorafs_payload(&sorafs_node, &bundle_bytes)?;
        for payload in &artifact_payloads {
            let _stored = ingest_sorafs_payload(&sorafs_node, payload)?;
        }

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node);
        manager.reconcile_once()?;

        let snapshot = manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("service plan");
        assert!(!plan.bundle_available_locally);
        assert_eq!(plan.health_status, SoraServiceHealthStatusV1::Hydrating);
        assert!(plan.artifacts.iter().all(|artifact| !artifact.available_locally));
        assert!(!temp_dir
            .path()
            .join("artifacts")
            .join(hash_cache_name(bundle.container.bundle_hash))
            .exists());
        Ok(())
    }

    #[test]
    fn restore_persisted_snapshot_rehydrates_missing_artifacts_from_committed_sorafs_store(
    ) -> Result<()> {
        let mut state = test_state()?;
        let mut bundle = load_deployment_bundle_fixture()?;
        let bundle_bytes = simple_soracloud_contract_artifact(&["update", "private_update"]);
        let artifact_payloads =
            assign_fixture_artifact_hashes(&mut bundle, &bundle_bytes, "restart-rehydrate");
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
        let sorafs_node = test_sorafs_node(&temp_dir);
        let mut committed_manifests = vec![ingest_sorafs_payload(&sorafs_node, &bundle_bytes)?];
        for payload in &artifact_payloads {
            committed_manifests.push(ingest_sorafs_payload(&sorafs_node, payload)?);
        }
        approve_sorafs_manifests(&state, &committed_manifests)?;

        let manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node.clone());
        manager.reconcile_once()?;

        let bundle_cache_path = temp_dir
            .path()
            .join("artifacts")
            .join(hash_cache_name(bundle.container.bundle_hash));
        let artifact_cache_paths = bundle
            .service
            .artifacts
            .iter()
            .map(|artifact| {
                temp_dir
                    .path()
                    .join("artifacts")
                    .join(hash_cache_name(artifact.artifact_hash))
            })
            .collect::<Vec<_>>();
        fs::remove_file(&bundle_cache_path)?;
        for path in &artifact_cache_paths {
            fs::remove_file(path)?;
        }

        let restarted_manager = SoracloudRuntimeManager::new(
            test_runtime_manager_config(temp_dir.path().to_path_buf()),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node);
        assert!(restarted_manager.restore_persisted_snapshot()?);
        restarted_manager.reconcile_once()?;

        let snapshot = restarted_manager.snapshot.read().clone();
        let plan = snapshot
            .services
            .get("web_portal")
            .and_then(|versions| versions.get("2026.02.0"))
            .expect("restarted service plan");
        assert!(plan.bundle_available_locally);
        assert!(plan.artifacts.iter().all(|artifact| artifact.available_locally));
        assert_eq!(fs::read(bundle_cache_path)?, bundle_bytes);
        for (path, payload) in artifact_cache_paths.iter().zip(artifact_payloads) {
            assert_eq!(fs::read(path)?, payload);
        }
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
    fn ivm_host_private_runtime_reads_secret_and_credential_material() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.capabilities.network = SoraNetworkPolicyV1::Allowlist(Vec::new());
        let temp_dir = tempfile::tempdir()?;
        let service_root = temp_dir
            .path()
            .join("secrets")
            .join(bundle.service.service_name.to_string())
            .join(bundle.service.service_version.clone())
            .join("db");
        fs::create_dir_all(&service_root)?;
        fs::write(service_root.join("password"), b"super-secret")?;
        let credential_root = temp_dir
            .path()
            .join("credentials")
            .join(bundle.service.service_name.to_string())
            .join(bundle.service.service_version.clone())
            .join("vault");
        fs::create_dir_all(&credential_root)?;
        fs::write(credential_root.join("token"), br#"{"token":"abc"}"#)?;

        let private_request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
        );
        let private_host = SoracloudIvmHost::new(
            private_request,
            temp_dir.path().to_path_buf(),
            test_runtime_manager_config(temp_dir.path().to_path_buf()).egress,
            BTreeMap::new(),
        );
        private_host.require_private_runtime(SYSCALL_SORACLOUD_READ_SECRET)?;
        private_host.require_private_runtime(SYSCALL_SORACLOUD_READ_CREDENTIAL)?;
        assert_eq!(
            private_host.read_material("secrets", "db/password")?,
            Some(b"super-secret".to_vec())
        );
        assert_eq!(
            private_host.read_material("credentials", "vault/token")?,
            Some(br#"{"token":"abc"}"#.to_vec())
        );

        let public_request = sample_ordered_mailbox_request(
            &bundle,
            "update",
            sample_mailbox_message(&bundle, "update", b"public".to_vec()),
        );
        let public_host = SoracloudIvmHost::new(
            public_request,
            temp_dir.path().to_path_buf(),
            test_runtime_manager_config(temp_dir.path().to_path_buf()).egress,
            BTreeMap::new(),
        );
        assert!(matches!(
            public_host.require_private_runtime(SYSCALL_SORACLOUD_READ_SECRET),
            Err(VMError::NotImplemented {
                syscall: SYSCALL_SORACLOUD_READ_SECRET
            })
        ));
        assert!(matches!(
            public_host.require_private_runtime(SYSCALL_SORACLOUD_READ_CREDENTIAL),
            Err(VMError::NotImplemented {
                syscall: SYSCALL_SORACLOUD_READ_CREDENTIAL
            })
        ));
        Ok(())
    }

    #[test]
    fn ivm_host_egress_fetch_enforces_allowlist_rate_and_byte_limits() -> Result<()> {
        let mut bundle = load_deployment_bundle_fixture()?;
        bundle.container.capabilities.network =
            SoraNetworkPolicyV1::Allowlist(vec!["127.0.0.1".to_owned()]);
        let temp_dir = tempfile::tempdir()?;
        let private_request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
        );
        let mut host = SoracloudIvmHost::new(
            private_request,
            temp_dir.path().to_path_buf(),
            iroha_config::parameters::actual::SoracloudRuntimeEgress {
                default_allow: false,
                allowed_hosts: vec!["127.0.0.1".to_owned()],
                rate_per_minute: std::num::NonZeroU32::new(1),
                max_bytes_per_minute: std::num::NonZeroU64::new(32),
            },
            BTreeMap::new(),
        );
        let body = b"hello-egress".to_vec();
        let expected_hash = Hash::new(&body);
        let (url, server) = spawn_http_fixture(body.clone())?;
        let response = host.egress_fetch(SoracloudEgressFetchRequestV1 {
            url: url.clone(),
            expected_hash: Some(expected_hash),
            max_bytes: 32,
        })?;
        server.join().expect("fixture server should complete");
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, body);
        assert_eq!(response.body_hash, expected_hash);

        let rate_limited = host
            .egress_fetch(SoracloudEgressFetchRequestV1 {
                url,
                expected_hash: Some(expected_hash),
                max_bytes: 32,
            })
            .expect_err("second request must exceed the per-minute rate limit");
        assert_eq!(rate_limited, VMError::PermissionDenied);

        let disallowed = host
            .egress_fetch(SoracloudEgressFetchRequestV1 {
                url: "http://example.com/blocked".to_owned(),
                expected_hash: Some(Hash::new(b"blocked")),
                max_bytes: 32,
            })
            .expect_err("disallowed hosts must be rejected before fetch");
        assert_eq!(disallowed, VMError::PermissionDenied);

        let private_request = sample_ordered_mailbox_request(
            &bundle,
            "private_update",
            sample_mailbox_message(&bundle, "private_update", b"private-2".to_vec()),
        );
        let mut byte_limited_host = SoracloudIvmHost::new(
            private_request,
            temp_dir.path().to_path_buf(),
            iroha_config::parameters::actual::SoracloudRuntimeEgress {
                default_allow: false,
                allowed_hosts: vec!["127.0.0.1".to_owned()],
                rate_per_minute: std::num::NonZeroU32::new(5),
                max_bytes_per_minute: std::num::NonZeroU64::new(4),
            },
            BTreeMap::new(),
        );
        let (url, server) = spawn_http_fixture(b"too-large".to_vec())?;
        let byte_limited = byte_limited_host
            .egress_fetch(SoracloudEgressFetchRequestV1 {
                url,
                expected_hash: Some(Hash::new(b"too-large")),
                max_bytes: 16,
            })
            .expect_err("responses above the byte budget must be rejected");
        server.join().expect("fixture server should complete");
        assert_eq!(byte_limited, VMError::PermissionDenied);
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
