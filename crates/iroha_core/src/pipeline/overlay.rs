//! Transaction overlay scaffolding.
//!
//! A `TxOverlay` represents the sequence of stateful operations (ISIs) that a
//! transaction intends to perform. In the future, overlays will be created in a
//! read-only execution prepass and later committed in a deterministic order.
//! For now, this module provides a thin wrapper around a list of
//! `InstructionBox` and an `apply` method that executes them via the executor.
//!
//! Future work will extend overlays to be produced by IVM prepasses (draining
//! queued ISIs without mutating state) and to incorporate trigger side effects.
//! For now the type is mostly a thin wrapper that keeps chunking logic and
//! admission limits (`pipeline.overlay_max_*`) in one place.

use core::str::FromStr;
#[cfg(feature = "telemetry")]
use std::time::Instant;
#[cfg(test)]
use std::{
    collections::VecDeque,
    sync::{LazyLock, Mutex},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    mem,
    num::NonZeroU64,
    sync::{Arc, OnceLock},
};

use iroha_config::parameters::actual::QueryCursorMode;
use iroha_crypto::{Hash, streaming::TransportCapabilityResolutionSnapshot};
use iroha_data_model::{
    block::BlockHeader,
    errors::CanonicalErrorKind,
    executor::IvmAdmissionError,
    executor::{ManifestAbiHashMismatchInfo, ManifestCodeHashMismatchInfo},
    isi::{
        InstructionBox,
        settlement::{DvpIsi, PvpIsi},
        smart_contract_code::{
            ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
        },
    },
    metadata::Metadata,
    name::Name,
    nexus::AxtRejectContext,
    prelude::{AccountId, ValidationFail},
    proof::VerifyingKeyId,
    smart_contract::ContractAddress,
    smart_contract::manifest::{ContractManifest, MANIFEST_METADATA_KEY},
    transaction::{Executable, SignedTransaction, executable::ContractInvocation},
    zk::{
        BackendTag as ZkBackendTag, OpenVerifyEnvelope as ZkOpenVerifyEnvelope,
        OpenVerifyEnvelopeBounds as ZkOpenVerifyEnvelopeBounds, StarkFriOpenProofV1,
    },
};
use ivm::host::IVMHost;
use ivm::{VMError as IvmError, analysis::ProgramAnalysisError};
use mv::storage::StorageReadOnly;
use norito::{codec::Encode as NoritoEncode, streaming::CapabilityFlags};
use sha2::{Digest as _, Sha256};

use crate::{
    executor::{
        ContractEntrypointAuthorizationSnapshot, ensure_asset_definition_registration_allowed,
        extract_register_asset_definition, parse_gas_limit,
    },
    smartcontracts::{
        code,
        isi::settlement::{admission_validate_dvp, admission_validate_pvp},
        ivm::{
            cache::{IvmCache, ProgramSummary},
            host::{AmxBudgetViolation, QueryStateSource},
        },
    },
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
    streaming,
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StreamingOverlayMetadata {
    transport: Option<TransportCapabilityResolutionSnapshot>,
    negotiated: Option<CapabilityFlags>,
}

#[cfg(test)]
#[derive(Default)]
struct ProgramHashCache {
    map: BTreeMap<Hash, Hash>,
    order: VecDeque<Hash>,
    cap: usize,
}

#[cfg(test)]
impl ProgramHashCache {
    const DEFAULT_CAP: usize = 64;

    fn new(cap: usize) -> Self {
        Self {
            map: BTreeMap::new(),
            order: VecDeque::new(),
            cap,
        }
    }

    fn get_or_insert(&mut self, code_hash: Hash, abi_hash: Hash) -> Hash {
        if let Some(stored) = self.map.get(&code_hash) {
            return *stored;
        }
        self.map.insert(code_hash, abi_hash);
        self.order.push_back(code_hash);
        if self.order.len() > self.cap {
            if let Some(evicted) = self.order.pop_front() {
                self.map.remove(&evicted);
            }
        }
        abi_hash
    }
}

#[cfg(test)]
static PROGRAM_HASH_CACHE: LazyLock<Mutex<ProgramHashCache>> =
    LazyLock::new(|| Mutex::new(ProgramHashCache::new(ProgramHashCache::DEFAULT_CAP)));

#[derive(Clone, Debug)]
struct ContractCallExecutionContext {
    entrypoint: Option<String>,
    entrypoint_pc: Option<u64>,
    argument_record: Option<ivm::PreparedArgumentRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OverlayLifecycleCompletion {
    contract_address: ContractAddress,
    pending: code::PendingContractLifecycle,
}

fn validate_overlay_contract_runtime_context(
    world: &impl WorldReadOnly,
    context: &crate::executor::ContractRuntimeExecutionContext,
) -> Result<(), ValidationFail> {
    let live_subject = world
        .contract_subject_bindings()
        .get(&context.contract_address)
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!(
                "contract instance `{}` has no subject binding",
                context.contract_address
            ))
        })?;
    live_subject
        .validate_for(&context.contract_address)
        .map_err(ValidationFail::NotPermitted)?;
    if context.contract_subject != live_subject.subject {
        return Err(ValidationFail::NotPermitted(
            "prepared contract runtime context has an invalid subject binding".to_owned(),
        ));
    }
    if world
        .contract_instances()
        .get(&context.contract_address)
        .is_none()
    {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{}` is no longer active",
            context.contract_address
        )));
    }
    let live_alias = world
        .contract_alias_bindings()
        .get(&context.contract_address)
        .map(|binding| binding.alias.clone());
    if live_alias != context.contract_alias {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{}` changed alias binding while its effects were prepared",
            context.contract_address
        )));
    }
    if let Some(alias) = live_alias
        && world.contract_aliases().get(&alias) != Some(&context.contract_address)
    {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{}` has an inconsistent live alias binding",
            context.contract_address
        )));
    }
    Ok(())
}

enum ContractDispatchSource<'a> {
    Bytecode(&'a [u8]),
    Prepared(&'a ivm::PreparedContract),
}

impl ContractDispatchSource<'_> {
    fn is_self_describing(&self) -> Result<bool, OverlayBuildError> {
        match self {
            Self::Bytecode(bytecode) => ivm::ProgramMetadata::parse(bytecode)
                .map(|parsed| parsed.contract_interface.is_some())
                .map_err(|err| {
                    OverlayBuildError::ContractCall(format!(
                        "invalid contract artifact for contract call dispatch: {err}"
                    ))
                }),
            Self::Prepared(_) => Ok(true),
        }
    }

    fn callable_entrypoint(
        &self,
        selector: &str,
    ) -> Result<(u64, Option<String>, Option<ivm::EntrypointArgumentSchemaV1>), OverlayBuildError>
    {
        match self {
            Self::Bytecode(bytecode) => {
                let parsed = ivm::ProgramMetadata::parse(bytecode).map_err(|err| {
                    OverlayBuildError::ContractCall(format!(
                        "invalid contract artifact for contract call dispatch: {err}"
                    ))
                })?;
                let prefix_len = parsed.prefix_len() as u64;
                let contract_interface = parsed.contract_interface.as_ref().ok_or_else(|| {
                    OverlayBuildError::ContractCall(
                        "contract call entrypoint metadata requires a self-describing contract artifact"
                            .to_owned(),
                    )
                })?;
                let descriptor = contract_interface
                    .entrypoints
                    .iter()
                    .find(|candidate| candidate.name == selector)
                    .ok_or_else(|| {
                        OverlayBuildError::ContractCall(format!(
                            "unknown contract entrypoint `{selector}`"
                        ))
                    })?;
                let permission =
                    crate::executor::raw_contract_entrypoint_permission(descriptor, selector)
                        .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
                Ok((
                    prefix_len + descriptor.entry_pc,
                    permission,
                    descriptor.argument_schema.clone(),
                ))
            }
            Self::Prepared(contract) => {
                let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "unknown contract entrypoint `{selector}`"
                    ))
                })?;
                let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract entrypoint `{selector}` has no validated program counter"
                    ))
                })?;
                let permission =
                    crate::executor::raw_contract_entrypoint_permission(descriptor, selector)
                        .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
                Ok((
                    entrypoint_pc,
                    permission,
                    descriptor.argument_schema.clone(),
                ))
            }
        }
    }
}

fn parse_raw_contract_call_execution_context(
    metadata: &iroha_data_model::metadata::Metadata,
    bytecode: &[u8],
    gas_limit: u64,
) -> Result<Option<ContractCallExecutionContext>, OverlayBuildError> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Bytecode(bytecode),
        gas_limit,
    )
}

fn parse_prepared_contract_call_execution_context(
    metadata: &Metadata,
    contract: &ivm::PreparedContract,
    gas_limit: u64,
) -> Result<Option<ContractCallExecutionContext>, OverlayBuildError> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Prepared(contract),
        gas_limit,
    )
}

fn reject_raw_contract_without_state(bytecode: &[u8]) -> Result<(), OverlayBuildError> {
    let parsed = ivm::ProgramMetadata::parse(bytecode).map_err(|error| {
        OverlayBuildError::ContractCall(format!(
            "invalid contract artifact for contract call dispatch: {error}"
        ))
    })?;
    if parsed.contract_interface.is_some() {
        return Err(OverlayBuildError::ContractCall(
            "raw-IVM contract entrypoint dispatch requires a full state view and live contract binding"
                .to_owned(),
        ));
    }
    Ok(())
}

fn parse_contract_call_execution_context_from_source(
    metadata: &Metadata,
    source: ContractDispatchSource<'_>,
    gas_limit: u64,
) -> Result<Option<ContractCallExecutionContext>, OverlayBuildError> {
    let entrypoint = metadata
        .get("contract_entrypoint")
        .map(|raw| {
            raw.try_into_any_norito::<String>().map_err(|err| {
                OverlayBuildError::ContractCall(format!(
                    "invalid contract_entrypoint metadata: {err}"
                ))
            })
        })
        .transpose()?
        .map(|value| value.trim().to_owned());
    if entrypoint.as_deref().is_some_and(str::is_empty) {
        return Err(OverlayBuildError::ContractCall(
            "contract_entrypoint must not be empty".to_owned(),
        ));
    }

    let payload = metadata.get("contract_payload").cloned();
    if entrypoint.is_none() {
        if source.is_self_describing()? {
            return Err(OverlayBuildError::ContractCall(
                "self-describing contract calls require explicit contract_entrypoint metadata"
                    .to_owned(),
            ));
        }
        if payload.is_none() {
            return Ok(None);
        }
    }

    let (entrypoint_pc, argument_schema) = if let Some(selector) = entrypoint.as_deref() {
        let (entrypoint_pc, _entrypoint_permission, argument_schema) =
            source.callable_entrypoint(selector)?;
        (Some(entrypoint_pc), argument_schema)
    } else {
        (None, None)
    };
    let canonical_record = crate::executor::encode_contract_argument_record(
        argument_schema.as_ref(),
        payload.as_ref(),
    )
    .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
    let argument_record = match (argument_schema.as_ref(), canonical_record) {
        (None, None) => None,
        (Some(schema), Some(record)) => Some(
            ivm::prepare_argument_record_with_gas_limit(schema, Arc::from(record), gas_limit)
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?,
        ),
        _ => {
            return Err(OverlayBuildError::ContractCall(
                "contract argument schema and canonical record diverged".to_owned(),
            ));
        }
    };
    Ok(Some(ContractCallExecutionContext {
        entrypoint,
        entrypoint_pc,
        argument_record,
    }))
}

fn parse_prepared_contract_invocation_execution_context(
    invocation: &ContractInvocation,
    contract: &ivm::PreparedContract,
    gas_limit: u64,
    reused_argument_record: Option<&ivm::PreparedArgumentRecord>,
) -> Result<ContractCallExecutionContext, OverlayBuildError> {
    let selector = invocation.entrypoint.trim();
    if selector.is_empty() {
        return Err(OverlayBuildError::ContractCall(
            "contract entrypoint must not be empty".to_owned(),
        ));
    }

    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        OverlayBuildError::ContractCall(format!("unknown contract entrypoint `{selector}`"))
    })?;
    let _permission =
        crate::executor::callable_contract_entrypoint_permission(descriptor, selector)
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
    let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
        OverlayBuildError::ContractCall(format!(
            "contract entrypoint `{selector}` has no validated program counter"
        ))
    })?;

    let argument_record = match (
        descriptor.argument_schema.as_ref(),
        invocation.arguments.as_deref(),
    ) {
        (None, None) => None,
        (None, Some(_)) => {
            return Err(OverlayBuildError::ContractCall(
                "zero-parameter entrypoint must not carry an argument record".to_owned(),
            ));
        }
        (Some(_), None) => {
            return Err(OverlayBuildError::ContractCall(
                "parameterized entrypoint requires an argument record".to_owned(),
            ));
        }
        (Some(schema), Some(arguments)) => {
            let schema_bytes = norito::to_bytes(schema)
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            if let Some(reused) = reused_argument_record
                && reused.canonical_bytes() == arguments
                && reused.schema_bytes() == schema_bytes.as_slice()
            {
                Some(reused.clone())
            } else {
                Some(
                    ivm::prepare_argument_record_with_gas_limit(
                        schema,
                        Arc::<[u8]>::from(arguments),
                        gas_limit,
                    )
                    .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?,
                )
            }
        }
    };
    Ok(ContractCallExecutionContext {
        entrypoint: Some(selector.to_owned()),
        entrypoint_pc: Some(entrypoint_pc),
        argument_record,
    })
}

fn authorize_and_prepare_raw_contract_dispatch<R: StateReadOnly>(
    state_ro: &R,
    tx: &SignedTransaction,
    summary: &ProgramSummary,
    gas_limit: u64,
) -> Result<
    (
        ContractCallExecutionContext,
        crate::executor::ContractRuntimeExecutionContext,
        ContractEntrypointAuthorizationSnapshot,
    ),
    OverlayBuildError,
> {
    let selector = crate::executor::requested_contract_entrypoint(tx.metadata())
        .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?
        .ok_or_else(|| {
            OverlayBuildError::ContractCall(
                "self-describing raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                    .to_owned(),
            )
        })?;
    let identity = crate::executor::require_raw_contract_runtime_identity(
        state_ro.world(),
        summary.code_hash,
        tx.metadata(),
    )
    .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
    let authorization = crate::executor::authorize_prepared_raw_contract_selector(
        state_ro.world(),
        tx.authority(),
        summary.prepared_contract(),
        &selector,
        &identity,
    )
    .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
    let contract_subject = code::fetch_bound_contract_subject(state_ro, &identity.contract_address)
        .ok_or_else(|| {
            OverlayBuildError::ContractCall(format!(
                "contract instance `{}` has no valid subject binding",
                identity.contract_address
            ))
        })?;
    let call_context = parse_prepared_contract_call_execution_context(
        tx.metadata(),
        summary.prepared_contract(),
        gas_limit,
    )?
    .ok_or_else(|| {
        OverlayBuildError::ContractCall(
            "raw-IVM contract dispatch did not materialize its selected entrypoint".to_owned(),
        )
    })?;
    let runtime_context = crate::executor::ContractRuntimeExecutionContext {
        contract_subject,
        contract_address: identity.contract_address,
        contract_alias: identity.contract_alias,
        entrypoint: selector,
    };
    Ok((call_context, runtime_context, authorization))
}

fn validate_bound_contract_manifest(
    manifest: &ContractManifest,
    summary: &ProgramSummary,
) -> Result<(), OverlayBuildError> {
    if let Some(expected) = manifest.code_hash
        && expected != summary.code_hash
    {
        return Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo {
                expected,
                actual: summary.code_hash,
            }),
        ));
    }
    if let Some(expected) = manifest.abi_hash
        && expected != summary.abi_hash
    {
        return Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::ManifestAbiHashMismatch(ManifestAbiHashMismatchInfo {
                expected,
                actual: summary.abi_hash,
            }),
        ));
    }
    Ok(())
}

fn map_program_analysis_error(err: ProgramAnalysisError) -> OverlayBuildError {
    match err {
        ProgramAnalysisError::Metadata(_) => OverlayBuildError::IvmHeaderParse,
        ProgramAnalysisError::Decode(decode_err) => OverlayBuildError::IvmLoad(decode_err),
    }
}

fn cached_amx_analysis(
    ivm_cache: &mut IvmCache,
    summary: &ProgramSummary,
    bytecode: &[u8],
) -> Result<ivm::analysis::ProgramAnalysis, OverlayBuildError> {
    ivm_cache
        .analyze_program(summary, bytecode)
        .map_err(map_program_analysis_error)
}

#[cfg(feature = "telemetry")]
fn observe_overlay_stage_ms<R>(state_ro: &R, stage: &'static str, started_at: Instant)
where
    R: StateReadOnly,
{
    let aggregate_lane = state_ro.nexus().routing_policy.default_lane;
    state_ro.metrics().observe_pipeline_stage_ms(
        aggregate_lane,
        stage,
        started_at.elapsed().as_secs_f64() * 1_000.0,
    );
}

fn apply_contract_call_execution_context(
    vm: &mut ivm::IVM,
    context: Option<&ContractCallExecutionContext>,
) -> Result<(), OverlayBuildError> {
    if let Some(argument_record) = context.and_then(|context| context.argument_record.as_ref()) {
        argument_record
            .precharge_vm(vm)
            .map_err(OverlayBuildError::IvmRun)?;
    }
    if let Some(context) = context
        && let Some(entrypoint_pc) = context.entrypoint_pc
    {
        // Public by-call entrypoints are compiled as regular functions, not as
        // the artifact's top-level `main`. Seed RA with the end-of-code
        // sentinel so `return` exits execution instead of falling through to pc=0.
        vm.set_register(1, vm.memory.code_len());
        vm.set_program_counter(entrypoint_pc).map_err(|err| {
            OverlayBuildError::ContractCall(format!(
                "contract entrypoint `{}` resolved to invalid pc: {err}",
                context.entrypoint.as_deref().unwrap_or("main")
            ))
        })?;
    }
    Ok(())
}

fn begin_overlay_access_log<QS>(
    host: &mut crate::smartcontracts::ivm::host::CoreHostImpl<QS>,
    capture_access_log: bool,
) -> Result<(), OverlayBuildError>
where
    QS: crate::smartcontracts::ivm::host::QueryStateAccess + Default,
{
    if capture_access_log {
        host.begin_tx(&ivm::parallel::StateAccessSet::default())
            .map_err(OverlayBuildError::IvmRun)?;
    }
    Ok(())
}

fn finish_overlay_access_log<QS>(
    host: &mut crate::smartcontracts::ivm::host::CoreHostImpl<QS>,
    capture_access_log: bool,
) -> Result<Option<ivm::host::AccessLog>, OverlayBuildError>
where
    QS: crate::smartcontracts::ivm::host::QueryStateAccess + Default,
{
    if capture_access_log && host.access_logging_supported() {
        host.finish_tx()
            .map(Some)
            .map_err(OverlayBuildError::IvmRun)
    } else {
        Ok(None)
    }
}

fn default_pipeline_config() -> iroha_config::parameters::actual::Pipeline {
    use iroha_config::parameters::{actual, defaults};

    actual::Pipeline {
        ivm_proved: actual::IvmProvedExecution {
            enabled: defaults::pipeline::ivm_proved::ENABLED,
            skip_replay: defaults::pipeline::ivm_proved::SKIP_REPLAY,
            allowed_circuits: Vec::new(),
        },
        dynamic_prepass: defaults::pipeline::DYNAMIC_PREPASS,
        access_set_cache_enabled: defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
        parallel_overlay: defaults::pipeline::PARALLEL_OVERLAY,
        workers: defaults::pipeline::WORKERS,
        stateless_cache_cap: defaults::pipeline::STATELESS_CACHE_CAP,
        parallel_apply: defaults::pipeline::PARALLEL_APPLY,
        ready_queue_heap: defaults::pipeline::READY_QUEUE_HEAP,
        gpu_key_bucket: defaults::pipeline::GPU_KEY_BUCKET,
        debug_trace_scheduler_inputs: defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS,
        debug_trace_tx_eval: defaults::pipeline::DEBUG_TRACE_TX_EVAL,
        signature_batch_max: defaults::pipeline::SIGNATURE_BATCH_MAX,
        signature_batch_max_ed25519: defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
        signature_batch_max_secp256k1: defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
        signature_batch_max_pqc: defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
        signature_batch_max_bls: defaults::pipeline::SIGNATURE_BATCH_MAX_BLS,
        cache_size: defaults::pipeline::CACHE_SIZE,
        ivm_cache_max_decoded_ops: defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
        ivm_cache_max_bytes: defaults::pipeline::IVM_CACHE_MAX_BYTES,
        ivm_prover_threads: defaults::pipeline::IVM_PROVER_THREADS,
        overlay_max_instructions: defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
        overlay_max_bytes: defaults::pipeline::OVERLAY_MAX_BYTES,
        overlay_chunk_instructions: defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
        gas: actual::Gas {
            tech_account_id: defaults::pipeline::GAS_TECH_ACCOUNT_ID.to_string(),
            accepted_assets: Vec::new(),
            units_per_gas: Vec::new(),
        },
        ivm_max_cycles_upper_bound: defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
        ivm_max_decoded_instructions: defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
        ivm_max_decoded_bytes: defaults::pipeline::IVM_MAX_DECODED_BYTES,
        quarantine_max_txs_per_block: defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
        quarantine_tx_max_cycles: defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
        quarantine_tx_max_millis: defaults::pipeline::QUARANTINE_TX_MAX_MILLIS,
        query_default_cursor_mode: QueryCursorMode::Ephemeral,
        query_max_fetch_size: defaults::pipeline::QUERY_MAX_FETCH_SIZE,
        query_stored_min_gas_units: defaults::pipeline::QUERY_STORED_MIN_GAS_UNITS,
        amx_per_dataspace_budget_ms: defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
        amx_group_budget_ms: defaults::pipeline::AMX_GROUP_BUDGET_MS,
        amx_per_instruction_ns: defaults::pipeline::AMX_PER_INSTRUCTION_NS,
        amx_per_memory_access_ns: defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
        amx_per_syscall_ns: defaults::pipeline::AMX_PER_SYSCALL_NS,
    }
}

pub(crate) fn resolve_streaming_metadata<R: StateReadOnly>(
    state_ro: &R,
    authority: &AccountId,
) -> StreamingOverlayMetadata {
    let mut metadata = StreamingOverlayMetadata::default();
    let handle = match streaming::global_handle() {
        Some(handle) => handle,
        None => return metadata,
    };

    let mut candidate_keys: Vec<iroha_crypto::PublicKey> = Vec::new();
    if let Some(single) = authority.controller().single_signatory() {
        candidate_keys.push(single.clone());
    } else if let Some(policy) = authority.controller().multisig_policy() {
        candidate_keys.extend(
            policy
                .members()
                .iter()
                .map(|member| member.public_key().clone()),
        );
    }

    if candidate_keys.is_empty() {
        return metadata;
    }

    let peers = state_ro.world().peers();
    for key in candidate_keys {
        if let Some(peer) = peers.iter().find(|peer| peer.public_key() == &key).cloned() {
            metadata.transport = handle
                .transport_capabilities(&peer)
                .map(|resolution| TransportCapabilityResolutionSnapshot::from(&resolution));
            metadata.negotiated = handle.negotiated_capabilities(&peer);
            if metadata.transport.is_some() || metadata.negotiated.is_some() {
                break;
            }
        }
    }

    metadata
}

fn apply_streaming_metadata<QS: Default + crate::smartcontracts::ivm::host::QueryStateAccess>(
    host: &mut crate::smartcontracts::ivm::host::CoreHostImpl<QS>,
    metadata: StreamingOverlayMetadata,
) {
    if let Some(snapshot) = metadata.transport {
        host.record_transport_caps_snapshot(snapshot);
    }
    if let Some(flags) = metadata.negotiated {
        host.record_negotiated_caps_snapshot(flags);
    }
}

fn require_tx_gas_limit(tx: &SignedTransaction) -> Result<u64, OverlayBuildError> {
    let gas_limit = parse_gas_limit(tx.metadata()).map_err(|err| {
        let message = match err {
            ValidationFail::NotPermitted(msg) => msg,
            other => other.to_string(),
        };
        OverlayBuildError::GasLimit(message)
    })?;
    gas_limit.ok_or_else(|| {
        OverlayBuildError::GasLimit("missing gas_limit in transaction metadata".to_owned())
    })
}

#[cfg(test)]
const TEST_GAS_LIMIT: u64 = 50_000_000;

#[cfg(test)]
fn insert_gas_limit(metadata: &mut iroha_data_model::metadata::Metadata) {
    metadata.insert(
        Name::from_str("gas_limit").expect("static gas_limit key"),
        iroha_primitives::json::Json::new(TEST_GAS_LIMIT),
    );
}

#[cfg(test)]
fn compute_program_hashes(
    meta: &ivm::ProgramMetadata,
    _header_len: usize,
    bytecode: &[u8],
) -> (Hash, Hash) {
    let code_hash = ivm::contract_code_hash(bytecode);
    debug_assert_eq!(meta.abi_version, 1, "only ABI v1 is supported");
    let policy = ivm::SyscallPolicy::AbiV1;
    let computed = Hash::prehashed(ivm::syscalls::compute_abi_hash(policy));
    let abi_hash = PROGRAM_HASH_CACHE
        .lock()
        .expect("program hash cache poisoned")
        .get_or_insert(code_hash, computed);
    (code_hash, abi_hash)
}

const PREEXEC_OPCODE_DENYLIST: &[u8] = &[];

pub(crate) fn enforce_pre_execution_policy(
    ivm_max_cycles_upper_bound: NonZeroU64,
    meta: &ivm::ProgramMetadata,
    code_offset: usize,
    bytecode: &[u8],
) -> Result<(), OverlayBuildError> {
    crate::smartcontracts::ivm::validate_cycle_ceiling(meta, ivm_max_cycles_upper_bound)
        .map_err(OverlayBuildError::HeaderPolicy)?;

    if code_offset > bytecode.len() {
        return Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::BytecodeDecodingFailed(
                "IVM code offset exceeds bytecode length".into(),
            ),
        ));
    }

    for chunk in bytecode[code_offset..].chunks(4) {
        if chunk.len() < 4 {
            return Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::BytecodeDecodingFailed(
                    "IVM bytecode body not 4-byte aligned".into(),
                ),
            ));
        }
        let mut buf = [0u8; 4];
        buf.copy_from_slice(chunk);
        let word = u32::from_le_bytes(buf);
        let opcode = ivm::instruction::wide::opcode(word);
        if PREEXEC_OPCODE_DENYLIST.contains(&opcode) {
            return Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::BytecodeDecodingFailed(format!(
                    "opcode 0x{opcode:02x} denied by pre-execution policy"
                )),
            ));
        }
    }

    Ok(())
}

pub(crate) fn validate_contract_binding<R: StateReadOnly>(
    state_ro: &R,
    tx: &SignedTransaction,
    summary: &ProgramSummary,
) -> Result<(), OverlayBuildError> {
    let code_hash = summary.code_hash;
    let abi_hash = summary.abi_hash;
    let runtime_identity = crate::executor::resolve_raw_contract_runtime_identity(
        state_ro.world(),
        code_hash,
        tx.metadata(),
    )
    .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
    let mut contract_address = runtime_identity.map(|identity| identity.contract_address);
    if contract_address.is_none() {
        contract_address = tx
            .metadata()
            .get(&Name::from_str("gov_contract_address").expect("static name"))
            .map(|value| {
                value
                    .clone()
                    .try_into_any_norito::<String>()
                    .map_err(|error| {
                        OverlayBuildError::ContractCall(format!(
                            "invalid gov_contract_address metadata: {error}"
                        ))
                    })
            })
            .transpose()?
            .map(|raw| {
                raw.parse::<ContractAddress>().map_err(|error| {
                    OverlayBuildError::ContractCall(format!(
                        "invalid gov_contract_address metadata literal `{raw}`: {error}"
                    ))
                })
            })
            .transpose()?;
    }

    let artifacts = code::fetch_artifacts(state_ro, &code_hash, contract_address.as_ref());
    let manifest_opt = artifacts.manifest.as_ref();

    // Enforce any stored manifest constraints for this code hash.
    if let Some(manifest) = manifest_opt {
        if let Some(expected) = manifest.code_hash {
            if expected != code_hash {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo {
                        expected,
                        actual: code_hash,
                    }),
                ));
            }
        }
        if let Some(expected) = manifest.abi_hash {
            if expected != abi_hash {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::ManifestAbiHashMismatch(ManifestAbiHashMismatchInfo {
                        expected,
                        actual: abi_hash,
                    }),
                ));
            }
        }
    }

    // If contract-address metadata is present, ensure the instance binding matches.
    if let Some(contract_address) = contract_address.as_ref() {
        let bound_hash = artifacts.bound_code_hash.ok_or_else(|| {
            OverlayBuildError::HeaderPolicy(IvmAdmissionError::BytecodeDecodingFailed(format!(
                "contract instance `{contract_address}` not found in WSV"
            )))
        })?;

        if bound_hash != code_hash {
            return Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo {
                    expected: bound_hash,
                    actual: code_hash,
                }),
            ));
        }
        let manifest = manifest_opt.ok_or_else(|| {
            OverlayBuildError::HeaderPolicy(IvmAdmissionError::BytecodeDecodingFailed(
                "contract manifest missing for bound instance".into(),
            ))
        })?;
        let Some(expected_abi) = manifest.abi_hash else {
            return Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::BytecodeDecodingFailed(
                    "contract manifest missing abi_hash".into(),
                ),
            ));
        };
        if expected_abi != abi_hash {
            return Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::ManifestAbiHashMismatch(ManifestAbiHashMismatchInfo {
                    expected: expected_abi,
                    actual: abi_hash,
                }),
            ));
        }
    }

    Ok(())
}

fn metadata_contract_manifest(tx: &SignedTransaction) -> Option<ContractManifest> {
    tx.metadata()
        .get(&Name::from_str(MANIFEST_METADATA_KEY).expect("static manifest metadata key"))
        .and_then(|json| json.clone().try_into_any_norito::<ContractManifest>().ok())
}

fn queued_contract_bytes_match(
    queued: &[InstructionBox],
    code_hash: &Hash,
    bytecode: &[u8],
) -> bool {
    queued.iter().any(|instr| {
        instr
            .as_any()
            .downcast_ref::<RegisterSmartContractBytes>()
            .is_some_and(|bytes| {
                bytes.code_hash() == code_hash && bytes.code().as_slice() == bytecode
            })
    })
}

fn queued_manifest_matches(queued: &[InstructionBox], manifest: &ContractManifest) -> bool {
    queued.iter().any(|instr| {
        instr
            .as_any()
            .downcast_ref::<RegisterSmartContractCode>()
            .is_some_and(|registered| registered.manifest() == manifest)
    })
}

fn append_verified_contract_metadata_registration<R: StateReadOnly>(
    state_ro: &R,
    tx: &SignedTransaction,
    summary: &ProgramSummary,
    bytecode: &[u8],
    queued: &mut Vec<InstructionBox>,
) -> Result<(), OverlayBuildError> {
    let Some(manifest) = metadata_contract_manifest(tx) else {
        return Ok(());
    };
    let verified = ivm::verify_contract_artifact(bytecode).map_err(|err| {
        OverlayBuildError::HeaderPolicy(IvmAdmissionError::BytecodeDecodingFailed(err.to_string()))
    })?;
    if verified.code_hash != summary.code_hash {
        return Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo {
                expected: verified.code_hash,
                actual: summary.code_hash,
            }),
        ));
    }
    if manifest.signature_payload() != verified.manifest.signature_payload() {
        return Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::BytecodeDecodingFailed(
                "contract manifest metadata does not match embedded CNTR section".into(),
            ),
        ));
    }

    let code_hash = verified.code_hash;
    let code_is_registered = state_ro.world().contract_code().get(&code_hash).is_some()
        || queued_contract_bytes_match(queued, &code_hash, bytecode);
    if !code_is_registered {
        queued.push(
            RegisterSmartContractBytes {
                code_hash,
                code: bytecode.to_vec(),
            }
            .into(),
        );
    }

    let manifest_is_registered = state_ro
        .world()
        .contract_manifests()
        .get(&code_hash)
        .is_some()
        || queued_manifest_matches(queued, &manifest);
    if !manifest_is_registered {
        queued.push(RegisterSmartContractCode { manifest }.into());
    }
    Ok(())
}

fn append_verified_contract_metadata_registration_to_queued<R: StateReadOnly>(
    state_ro: &R,
    tx: &SignedTransaction,
    summary: &ProgramSummary,
    bytecode: &[u8],
    queued: &mut Vec<crate::smartcontracts::ivm::host::QueuedInstruction>,
    contract_runtime_context: Option<&crate::executor::ContractRuntimeExecutionContext>,
    entrypoint_authorization: &ContractEntrypointAuthorizationSnapshot,
) -> Result<(), OverlayBuildError> {
    let mut instructions = queued
        .iter()
        .map(|queued| queued.instruction.clone())
        .collect::<Vec<_>>();
    let original_len = instructions.len();
    append_verified_contract_metadata_registration(
        state_ro,
        tx,
        summary,
        bytecode,
        &mut instructions,
    )?;
    queued.extend(
        instructions
            .into_iter()
            .skip(original_len)
            .map(
                |instruction| crate::smartcontracts::ivm::host::QueuedInstruction {
                    instruction,
                    authority: tx.authority().clone(),
                    contract_runtime_context: contract_runtime_context.cloned(),
                    entrypoint_authorization: Some(entrypoint_authorization.clone()),
                },
            ),
    );
    Ok(())
}

fn append_verified_contract_metadata_registration_without_state(
    tx: &SignedTransaction,
    bytecode: &[u8],
    queued: &mut Vec<InstructionBox>,
) -> Result<(), OverlayBuildError> {
    let Some(manifest) = metadata_contract_manifest(tx) else {
        return Ok(());
    };
    let verified = ivm::verify_contract_artifact(bytecode).map_err(|err| {
        OverlayBuildError::HeaderPolicy(IvmAdmissionError::BytecodeDecodingFailed(err.to_string()))
    })?;
    if manifest.signature_payload() != verified.manifest.signature_payload() {
        return Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::BytecodeDecodingFailed(
                "contract manifest metadata does not match embedded CNTR section".into(),
            ),
        ));
    }

    let code_hash = verified.code_hash;
    if !queued_contract_bytes_match(queued, &code_hash, bytecode) {
        queued.push(
            RegisterSmartContractBytes {
                code_hash,
                code: bytecode.to_vec(),
            }
            .into(),
        );
    }
    if !queued_manifest_matches(queued, &manifest) {
        queued.push(RegisterSmartContractCode { manifest }.into());
    }
    Ok(())
}

pub(crate) fn prune_redundant_contract_ops<R: StateReadOnly>(
    state_ro: &R,
    queued: &mut Vec<InstructionBox>,
) {
    prune_redundant_contract_ops_with_metadata::<R, ()>(state_ro, queued, None);
}

fn prune_redundant_contract_ops_with_metadata<R, M>(
    state_ro: &R,
    queued: &mut Vec<InstructionBox>,
    metadata: Option<&mut Vec<M>>,
) where
    R: StateReadOnly,
{
    if queued.is_empty() {
        return;
    }
    if let Some(metadata) = metadata.as_ref() {
        debug_assert_eq!(
            metadata.len(),
            queued.len(),
            "overlay execution metadata must align with queued instructions",
        );
    }
    let mut manifest_cache: BTreeMap<Hash, Option<ContractManifest>> = BTreeMap::new();
    let mut code_cache: BTreeMap<Hash, Option<Vec<u8>>> = BTreeMap::new();
    let mut binding_cache: BTreeMap<ContractAddress, Option<Hash>> = BTreeMap::new();
    let retain: Vec<bool> = queued
        .iter()
        .map(|instr| {
            if let Some(reg) = instr.as_any().downcast_ref::<RegisterSmartContractCode>() {
                if let Some(hash) = reg.manifest().code_hash {
                    let existing = manifest_cache.entry(hash).or_insert_with(|| {
                        state_ro.world().contract_manifests().get(&hash).cloned()
                    });
                    if let Some(existing) = existing {
                        if existing == reg.manifest() {
                            return false;
                        }
                    }
                }
            } else if let Some(bytes) = instr.as_any().downcast_ref::<RegisterSmartContractBytes>()
            {
                let cached = code_cache.entry(*bytes.code_hash()).or_insert_with(|| {
                    state_ro
                        .world()
                        .contract_code()
                        .get(bytes.code_hash())
                        .cloned()
                });
                if cached
                    .as_ref()
                    .is_some_and(|existing| existing.as_slice() == bytes.code().as_slice())
                {
                    return false;
                }
            } else if let Some(activate) = instr.as_any().downcast_ref::<ActivateContractInstance>()
            {
                let key = activate.contract_address().clone();
                let bound = binding_cache
                    .entry(key.clone())
                    .or_insert_with(|| state_ro.world().contract_instances().get(&key).copied());
                if bound.is_some_and(|hash| hash == *activate.code_hash()) {
                    return false;
                }
            }
            true
        })
        .collect();
    if retain.iter().all(|keep| *keep) {
        return;
    }
    let prior = mem::take(queued);
    *queued = prior
        .into_iter()
        .zip(retain.iter().copied())
        .filter_map(|(instr, keep)| keep.then_some(instr))
        .collect();
    if let Some(metadata) = metadata {
        let prior = mem::take(metadata);
        *metadata = prior
            .into_iter()
            .zip(retain.into_iter())
            .filter_map(|(entry, keep)| keep.then_some(entry))
            .collect();
    }
}

#[derive(Debug, Clone)]
struct OverlayInstructionExecutionContext {
    authority: AccountId,
    contract_runtime_context: Option<crate::executor::ContractRuntimeExecutionContext>,
    entrypoint_authorization: Option<ContractEntrypointAuthorizationSnapshot>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TxOverlaySource {
    #[default]
    Instructions,
    ContractCall,
    Ivm,
    IvmProved,
}

/// Overlay of a transaction's intended operations.
#[derive(Debug, Clone, Default)]
pub struct TxOverlay {
    instructions: Vec<InstructionBox>,
    execution_contexts: Option<Vec<OverlayInstructionExecutionContext>>,
    entrypoint_authorization: Option<ContractEntrypointAuthorizationSnapshot>,
    lifecycle_completion: Option<OverlayLifecycleCompletion>,
    ivm_gas_used: Option<u64>,
    completed_axt: Vec<ivm::axt::HostAxtState>,
    durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
    durable_state_authorizations: BTreeMap<Name, Option<ContractEntrypointAuthorizationSnapshot>>,
    source: TxOverlaySource,
    byte_size: OnceLock<usize>,
}

/// Overlay plus optional host access log captured during the same VM run.
#[derive(Debug, Clone)]
pub(crate) struct PreparedTxOverlay {
    /// Built transaction overlay.
    pub(crate) overlay: TxOverlay,
    /// Dynamic state access log captured while building the overlay.
    pub(crate) access_log: Option<ivm::host::AccessLog>,
    /// Bytecode-derived scheduler fence for accesses whose concrete target is
    /// not proven by the instruction scanner.
    pub(crate) access_fence: VmAccessFence,
    /// Whether an opaque/nested or ledger-read syscall requires execution
    /// against the live scheduler state rather than the block-start snapshot.
    pub(crate) force_live_rebuild: bool,
    /// Canonical argument plan retained across a selective live-state rebuild.
    pub(crate) prepared_argument_record: Option<ivm::PreparedArgumentRecord>,
}

/// Conservative scheduler scope required by the reachable syscall surface.
///
/// Concrete host logs are useful for diagnostics and conflict precision, but
/// they describe only the block-start execution. If a predecessor changes a
/// value used for control flow, selective re-execution may choose a different
/// target. This bytecode-derived fence keeps that target change inside the DAG
/// relation established before any overlay is applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum VmAccessFence {
    /// Every reachable syscall is VM-local.
    #[default]
    None,
    /// Reachable syscalls are limited to contract-owned durable state.
    State,
    /// Ledger, nested-contract, opaque, or unclassified access is reachable.
    Global,
}

impl VmAccessFence {
    /// Derive a fail-closed fence from decoded bytecode rather than CNTR claims.
    #[must_use]
    pub(crate) fn from_program_analysis(analysis: &ivm::analysis::ProgramAnalysis) -> Self {
        let mut fence = Self::None;
        for syscall in &analysis.syscalls {
            match ivm::syscalls::syscall_access(syscall.number) {
                ivm::syscalls::SyscallAccess::None => {}
                ivm::syscalls::SyscallAccess::StateRead
                | ivm::syscalls::SyscallAccess::StateWrite => {
                    if fence == Self::None {
                        fence = Self::State;
                    }
                }
                ivm::syscalls::SyscallAccess::LedgerRead
                | ivm::syscalls::SyscallAccess::LedgerWrite
                | ivm::syscalls::SyscallAccess::Dynamic => return Self::Global,
            }
        }
        fence
    }

    /// Return whether the bytecode can observe world state not represented by
    /// the durable-state read fingerprint.
    #[must_use]
    pub(crate) fn requires_live_rebuild(analysis: &ivm::analysis::ProgramAnalysis) -> bool {
        analysis.syscalls.iter().any(|syscall| {
            matches!(
                ivm::syscalls::syscall_access(syscall.number),
                ivm::syscalls::SyscallAccess::LedgerRead
                    | ivm::syscalls::SyscallAccess::LedgerWrite
                    | ivm::syscalls::SyscallAccess::Dynamic
            )
        })
    }

    /// Scheduler write key which serializes every access in the required scope.
    #[must_use]
    pub(crate) const fn scheduler_write_key(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::State => Some("state:*"),
            Self::Global => Some("*"),
        }
    }
}

/// Snapshot of the durable-state prefixes read while preparing a VM overlay.
///
/// Overlay construction runs before the block scheduler applies predecessors. A
/// later transaction must therefore re-run its VM when one of the durable paths
/// it observed has changed in the meantime; otherwise a read-modify-write can
/// commit the value computed from the stale block-start snapshot.
#[derive(Clone, Debug)]
pub(crate) struct DurableStateReadSnapshot {
    /// `None` means an invalid/unrepresentable host key forced a fail-closed
    /// fingerprint of the complete durable-state map.
    prefixes: Option<Vec<Name>>,
    fingerprint: [u8; 32],
}

impl DurableStateReadSnapshot {
    /// Capture all exact values and descendants covered by the host read log.
    ///
    /// The host uses the same logical key for exact reads and prefix operations
    /// such as `STATE_KEYS`. Fingerprinting the whole prefix is conservative for
    /// exact reads and complete for both forms. Deployed contracts report the
    /// concrete contract-instance namespace; raw IVM execution reports the
    /// unscoped path it actually uses.
    pub(crate) fn capture<R>(
        tx: &SignedTransaction,
        access_log: Option<&ivm::host::AccessLog>,
        state_ro: &R,
    ) -> Option<Self>
    where
        R: StateReadOnly,
    {
        if !matches!(
            tx.instructions(),
            Executable::ContractCall(_) | Executable::Ivm(_)
        ) {
            return None;
        }
        let access_log = access_log?;
        if !access_log.durable_read_paths_complete {
            let fingerprint = durable_state_prefix_fingerprint(None, state_ro);
            return Some(Self {
                prefixes: None,
                fingerprint,
            });
        }
        if access_log.durable_read_paths.is_empty() {
            return None;
        }

        let mut prefixes = BTreeSet::new();
        for concrete_path in &access_log.durable_read_paths {
            let Ok(concrete_path) = Name::from_str(concrete_path) else {
                let fingerprint = durable_state_prefix_fingerprint(None, state_ro);
                return Some(Self {
                    prefixes: None,
                    fingerprint,
                });
            };
            prefixes.insert(concrete_path);
        }

        let prefixes = Some(prefixes.into_iter().collect::<Vec<_>>());
        let fingerprint = durable_state_prefix_fingerprint(prefixes.as_deref(), state_ro);
        Some(Self {
            prefixes,
            fingerprint,
        })
    }

    /// Return whether every observed durable-state prefix still has its
    /// block-preparation value.
    pub(crate) fn is_current<R>(&self, state_ro: &R) -> bool
    where
        R: StateReadOnly,
    {
        durable_state_prefix_fingerprint(self.prefixes.as_deref(), state_ro) == self.fingerprint
    }
}

fn durable_state_prefix_fingerprint<R>(prefixes: Option<&[Name]>, state_ro: &R) -> [u8; 32]
where
    R: StateReadOnly,
{
    fn frame(hasher: &mut Sha256, bytes: &[u8]) {
        let len = u64::try_from(bytes.len()).expect("slice length fits u64");
        hasher.update(len.to_le_bytes());
        hasher.update(bytes);
    }

    let mut hasher = Sha256::new();
    hasher.update(b"iroha:durable-state-read-snapshot:v1");
    let state = state_ro.world().smart_contract_state();
    let Some(prefixes) = prefixes else {
        hasher.update(u64::MAX.to_le_bytes());
        for (path, value) in state.iter() {
            let path_raw: &str = path.as_ref();
            frame(&mut hasher, path_raw.as_bytes());
            frame(&mut hasher, value);
        }
        return hasher.finalize().into();
    };
    let prefix_count = u64::try_from(prefixes.len()).expect("prefix count fits u64");
    hasher.update(prefix_count.to_le_bytes());
    for prefix in prefixes {
        let prefix_raw: &str = prefix.as_ref();
        frame(&mut hasher, prefix_raw.as_bytes());
        if let Some(value) = state.get(prefix) {
            hasher.update([1]);
            frame(&mut hasher, prefix_raw.as_bytes());
            frame(&mut hasher, value);
        } else {
            hasher.update([0]);
        }

        // Start directly at `prefix/`. Other valid names such as `prefix-`
        // can sort between `prefix` and `prefix/`; beginning at `prefix` and
        // breaking on the first non-match would therefore miss descendants.
        let descendant_prefix_raw = format!("{prefix_raw}/");
        let descendant_prefix = Name::from_str(&descendant_prefix_raw)
            .expect("a durable-state name with a slash suffix remains a valid Name");
        for (path, value) in state.range(descendant_prefix..) {
            let path_raw: &str = path.as_ref();
            if !path_raw.starts_with(&descendant_prefix_raw) {
                break;
            }
            hasher.update([1]);
            frame(&mut hasher, path_raw.as_bytes());
            frame(&mut hasher, value);
        }
        hasher.update([0]);
    }
    hasher.finalize().into()
}

impl PreparedTxOverlay {
    fn new(
        overlay: TxOverlay,
        access_log: Option<ivm::host::AccessLog>,
        access_fence: VmAccessFence,
        force_live_rebuild: bool,
    ) -> Self {
        Self {
            overlay,
            access_log,
            access_fence,
            force_live_rebuild,
            prepared_argument_record: None,
        }
    }

    fn with_prepared_argument_record(
        mut self,
        prepared_argument_record: Option<ivm::PreparedArgumentRecord>,
    ) -> Self {
        self.prepared_argument_record = prepared_argument_record;
        self
    }
}

impl TxOverlay {
    /// Create an overlay from a list of instructions.
    pub fn from_instructions(instrs: Vec<InstructionBox>) -> Self {
        Self {
            instructions: instrs,
            execution_contexts: None,
            entrypoint_authorization: None,
            lifecycle_completion: None,
            ivm_gas_used: None,
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::new(),
            durable_state_authorizations: BTreeMap::new(),
            source: TxOverlaySource::Instructions,
            byte_size: OnceLock::new(),
        }
    }

    fn from_ivm_proved_instructions(
        instrs: Vec<InstructionBox>,
        authority: &AccountId,
        contract_runtime_context: crate::executor::ContractRuntimeExecutionContext,
        entrypoint_authorization: ContractEntrypointAuthorizationSnapshot,
    ) -> Self {
        let execution_contexts = instrs
            .iter()
            .map(|_| OverlayInstructionExecutionContext {
                authority: authority.clone(),
                contract_runtime_context: Some(contract_runtime_context.clone()),
                entrypoint_authorization: Some(entrypoint_authorization.clone()),
            })
            .collect();
        Self {
            instructions: instrs,
            execution_contexts: Some(execution_contexts),
            entrypoint_authorization: Some(entrypoint_authorization),
            lifecycle_completion: None,
            ivm_gas_used: None,
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::new(),
            durable_state_authorizations: BTreeMap::new(),
            source: TxOverlaySource::IvmProved,
            byte_size: OnceLock::new(),
        }
    }

    /// Create an overlay from IVM-produced instructions and observed IVM gas usage.
    pub fn from_ivm_instructions(instrs: Vec<InstructionBox>, ivm_gas_used: u64) -> Self {
        Self {
            instructions: instrs,
            execution_contexts: None,
            entrypoint_authorization: None,
            lifecycle_completion: None,
            ivm_gas_used: Some(ivm_gas_used),
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::new(),
            durable_state_authorizations: BTreeMap::new(),
            source: TxOverlaySource::Ivm,
            byte_size: OnceLock::new(),
        }
    }

    /// Create an overlay from IVM-produced artifacts including durable state writes.
    pub fn from_ivm_execution(
        instrs: Vec<InstructionBox>,
        ivm_gas_used: u64,
        durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
    ) -> Self {
        let durable_state_authorizations = durable_state_overlay
            .keys()
            .cloned()
            .map(|path| (path, None))
            .collect();
        Self {
            instructions: instrs,
            execution_contexts: None,
            entrypoint_authorization: None,
            lifecycle_completion: None,
            ivm_gas_used: Some(ivm_gas_used),
            completed_axt: Vec::new(),
            durable_state_overlay,
            durable_state_authorizations,
            source: TxOverlaySource::Ivm,
            byte_size: OnceLock::new(),
        }
    }

    fn from_host_execution(
        instructions: Vec<InstructionBox>,
        execution_contexts: Vec<OverlayInstructionExecutionContext>,
        ivm_gas_used: u64,
        completed_axt: Vec<ivm::axt::HostAxtState>,
        durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
        durable_state_authorizations: BTreeMap<
            Name,
            Option<ContractEntrypointAuthorizationSnapshot>,
        >,
    ) -> Self {
        debug_assert_eq!(instructions.len(), execution_contexts.len());
        Self {
            instructions,
            execution_contexts: Some(execution_contexts),
            entrypoint_authorization: None,
            lifecycle_completion: None,
            ivm_gas_used: Some(ivm_gas_used),
            completed_axt,
            durable_state_overlay,
            durable_state_authorizations,
            source: TxOverlaySource::ContractCall,
            byte_size: OnceLock::new(),
        }
    }

    fn from_queued_execution(
        queued: Vec<crate::smartcontracts::ivm::host::QueuedInstruction>,
        ivm_gas_used: u64,
        completed_axt: Vec<ivm::axt::HostAxtState>,
        durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
        source: TxOverlaySource,
    ) -> Self {
        let mut instructions = Vec::with_capacity(queued.len());
        let mut execution_contexts = Vec::with_capacity(queued.len());
        for queued in queued {
            instructions.push(queued.instruction);
            execution_contexts.push(OverlayInstructionExecutionContext {
                authority: queued.authority,
                contract_runtime_context: queued.contract_runtime_context,
                entrypoint_authorization: queued.entrypoint_authorization,
            });
        }
        let durable_state_authorizations = durable_state_overlay
            .keys()
            .cloned()
            .map(|path| (path, None))
            .collect();
        Self {
            instructions,
            execution_contexts: Some(execution_contexts),
            entrypoint_authorization: None,
            lifecycle_completion: None,
            ivm_gas_used: Some(ivm_gas_used),
            completed_axt,
            durable_state_overlay,
            durable_state_authorizations,
            source,
            byte_size: OnceLock::new(),
        }
    }

    fn from_ivm_proved_execution(
        queued: Vec<crate::smartcontracts::ivm::host::QueuedInstruction>,
        ivm_gas_used: u64,
        completed_axt: Vec<ivm::axt::HostAxtState>,
        durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
    ) -> Self {
        Self::from_queued_execution(
            queued,
            ivm_gas_used,
            completed_axt,
            durable_state_overlay,
            TxOverlaySource::IvmProved,
        )
    }

    /// Is this overlay empty?
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
            && self.completed_axt.is_empty()
            && self.durable_state_overlay.is_empty()
    }

    /// Number of instructions in this overlay.
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Whether this overlay carries durable smart-contract state changes.
    pub fn has_durable_state_changes(&self) -> bool {
        !self.completed_axt.is_empty()
            || !self.durable_state_overlay.is_empty()
            || self.instructions.iter().any(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::bridge::RecordSccpMessage>()
                    .is_some()
            })
    }

    /// Iterate over instructions in this overlay.
    pub fn instructions(&self) -> impl ExactSizeIterator<Item = &InstructionBox> {
        self.instructions.iter()
    }

    /// Borrow the overlay instructions as a slice.
    pub fn instruction_slice(&self) -> &[InstructionBox] {
        &self.instructions
    }

    /// Borrow the durable smart-contract state overlay accumulated during IVM execution.
    pub fn durable_state_overlay(&self) -> &BTreeMap<Name, Option<Vec<u8>>> {
        &self.durable_state_overlay
    }

    /// Return IVM gas used during overlay prepass, when the source executable was `Executable::Ivm`.
    pub fn ivm_gas_used(&self) -> Option<u64> {
        self.ivm_gas_used
    }

    /// Approximate byte size of this overlay when serialized via Norito TLV.
    pub fn byte_size(&self) -> usize {
        *self.byte_size.get_or_init(|| {
            self.instructions
                .iter()
                .map(|i| NoritoEncode::encode(i).len())
                .sum()
        })
    }

    /// Apply the overlay to the given state transaction via the runtime executor.
    /// Executes instructions in chunks to bound peak working memory.
    ///
    /// # Errors
    /// Returns an error if executing any instruction fails validation or the executor rejects it.
    pub fn apply(
        &self,
        state_tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<(), ValidationFail> {
        self.apply_inner(state_tx, authority, self.instructions.len().max(1))
    }

    /// Apply the overlay with a specific chunk size (number of instructions per chunk).
    ///
    /// # Errors
    /// Returns an error if executing any instruction fails validation or the executor rejects it.
    pub fn apply_with_chunk(
        &self,
        state_tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        chunk_size: usize,
    ) -> Result<(), ValidationFail> {
        self.apply_inner(state_tx, authority, chunk_size.max(1))
    }

    fn validate_execution_context(
        world: &impl WorldReadOnly,
        execution_context: &OverlayInstructionExecutionContext,
    ) -> Result<(), ValidationFail> {
        match (
            execution_context.contract_runtime_context.as_ref(),
            execution_context.entrypoint_authorization.as_ref(),
        ) {
            (Some(runtime_context), Some(authorization)) => {
                validate_overlay_contract_runtime_context(world, runtime_context)?;
                if runtime_context.contract_address != authorization.contract_address
                    || runtime_context.contract_alias != authorization.contract_alias
                    || runtime_context.entrypoint != authorization.entrypoint
                    || execution_context.authority != authorization.authority
                {
                    return Err(ValidationFail::NotPermitted(
                        "prepared contract effect does not match its immutable authorization snapshot"
                            .to_owned(),
                    ));
                }
                authorization.validate_for_authority(world, &execution_context.authority)
            }
            (Some(_), None) => Err(ValidationFail::NotPermitted(
                "prepared contract effect is missing its entrypoint authorization snapshot"
                    .to_owned(),
            )),
            (None, Some(_)) => Err(ValidationFail::InternalError(
                "overlay entrypoint authorization has no runtime contract context".to_owned(),
            )),
            (None, None) => Ok(()),
        }
    }

    fn durable_path_requires_authorization(path: &Name) -> bool {
        let path = path.as_ref();
        path.starts_with("sc/")
            || (path.starts_with(code::CONTRACT_LIFECYCLE_STATE_PREFIX)
                && path
                    .as_bytes()
                    .get(code::CONTRACT_LIFECYCLE_STATE_PREFIX.len())
                    == Some(&b'/'))
    }

    fn validate_durable_authorizations(
        &self,
        world: &impl WorldReadOnly,
    ) -> Result<(), ValidationFail> {
        if self.durable_state_overlay.len() != self.durable_state_authorizations.len()
            || !self
                .durable_state_overlay
                .keys()
                .eq(self.durable_state_authorizations.keys())
        {
            return Err(ValidationFail::InternalError(
                "durable state overlay authorization keys are structurally inconsistent".to_owned(),
            ));
        }
        for (path, authorization) in &self.durable_state_authorizations {
            if Self::durable_path_requires_authorization(path) && authorization.is_none() {
                return Err(ValidationFail::NotPermitted(format!(
                    "scoped durable state path `{path}` is missing its contract authorization snapshot"
                )));
            }
            if let Some(authorization) = authorization {
                authorization.validate(world)?;
            }
        }
        Ok(())
    }

    fn apply_inner(
        &self,
        state_tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        chunk: usize,
    ) -> Result<(), ValidationFail> {
        let prior_sccp_recording_proof_verified = state_tx.sccp_recording_proof_verified;
        state_tx.sccp_recording_proof_verified = self.source == TxOverlaySource::IvmProved;
        let result = (|| -> Result<(), ValidationFail> {
            if self.source == TxOverlaySource::IvmProved {
                crate::validation_fee::enforce_ivm_proved_completed_axt_admission(
                    self.completed_axt.len(),
                    state_tx,
                )?;
            }
            let has_contract_effect = self.lifecycle_completion.is_some()
                || self.execution_contexts.as_ref().is_some_and(|contexts| {
                    contexts.iter().any(|context| {
                        context.contract_runtime_context.is_some()
                            || context.entrypoint_authorization.is_some()
                    })
                })
                || self
                    .durable_state_authorizations
                    .values()
                    .any(Option::is_some);
            if has_contract_effect && self.entrypoint_authorization.is_none() {
                return Err(ValidationFail::NotPermitted(
                    "contract overlay is missing its root authorization snapshot".to_owned(),
                ));
            }
            if let Some(completion) = self.lifecycle_completion.as_ref() {
                code::validate_contract_lifecycle_completion(
                    &state_tx.world,
                    &completion.contract_address,
                    completion.pending,
                )?;
            }
            if let Some(authorization) = self.entrypoint_authorization.as_ref() {
                if !authorization.is_root() {
                    return Err(ValidationFail::NotPermitted(
                        "contract overlay root authorization contains a parent invocation"
                            .to_owned(),
                    ));
                }
                authorization.validate_for_authority(&state_tx.world, authority)?;
                let retains_root = self
                    .execution_contexts
                    .iter()
                    .flat_map(|contexts| contexts.iter())
                    .filter_map(|context| context.entrypoint_authorization.as_ref())
                    .chain(
                        self.durable_state_authorizations
                            .values()
                            .filter_map(Option::as_ref),
                    )
                    .all(|effect| effect.descends_from(authorization));
                if !retains_root {
                    return Err(ValidationFail::NotPermitted(
                        "contract overlay effect does not retain the root invocation chain"
                            .to_owned(),
                    ));
                }
            }
            if let Some(execution_contexts) = self.execution_contexts.as_ref() {
                if execution_contexts.len() != self.instructions.len() {
                    return Err(ValidationFail::InternalError(
                        "overlay execution context count does not match its instruction count"
                            .to_owned(),
                    ));
                }
                for execution_context in execution_contexts {
                    Self::validate_execution_context(&state_tx.world, execution_context)?;
                }
            }
            self.validate_durable_authorizations(&state_tx.world)?;
            let executor = state_tx.world.executor.clone();
            let mut instruction_index = 0usize;
            for chunk_instrs in self.instructions.chunks(chunk) {
                for instr in chunk_instrs {
                    if let Some(authorization) = self.entrypoint_authorization.as_ref() {
                        authorization.validate_for_authority(&state_tx.world, authority)?;
                    }
                    let execution_context = self
                        .execution_contexts
                        .as_ref()
                        .map(|contexts| &contexts[instruction_index]);
                    if let Some(execution_context) = execution_context {
                        Self::validate_execution_context(&state_tx.world, execution_context)?;
                    }
                    let effect_authority =
                        execution_context.map_or(authority, |context| &context.authority);
                    if let Some(dvp) = instr.as_any().downcast_ref::<DvpIsi>() {
                        admission_validate_dvp(effect_authority, state_tx, dvp)
                            .map_err(ValidationFail::from)?;
                    } else if let Some(pvp) = instr.as_any().downcast_ref::<PvpIsi>() {
                        admission_validate_pvp(effect_authority, state_tx, pvp)
                            .map_err(ValidationFail::from)?;
                    }
                    if let Some(reg_asset_definition) = extract_register_asset_definition(instr) {
                        ensure_asset_definition_registration_allowed(
                            state_tx,
                            effect_authority,
                            &reg_asset_definition,
                        )?;
                    }
                    if let Some(execution_context) = execution_context {
                        executor.execute_borrowed_overlay_instruction(
                            state_tx,
                            &execution_context.authority,
                            instr,
                            execution_context.contract_runtime_context.as_ref(),
                        )?;
                    } else {
                        executor.execute_borrowed_overlay_instruction(
                            state_tx, authority, instr, None,
                        )?;
                    }
                    // The just-executed leaf may revoke its own permission or mutate its own live
                    // binding. Revalidate both the selected root and this exact leaf immediately,
                    // including after the final queued effect.
                    if let Some(authorization) = self.entrypoint_authorization.as_ref() {
                        authorization.validate_for_authority(&state_tx.world, authority)?;
                    }
                    if let Some(execution_context) = execution_context {
                        Self::validate_execution_context(&state_tx.world, execution_context)?;
                    }
                    instruction_index = instruction_index.saturating_add(1);
                }
            }
            // Revalidate immediately before committing the lifecycle tombstone. Queued effects
            // can invoke helper contracts, so the pre-execution check alone cannot detect a
            // deactivate/reactivate ABA staged while hajimari or kaizen is running.
            if let Some(completion) = self.lifecycle_completion.as_ref() {
                code::validate_contract_lifecycle_completion(
                    &state_tx.world,
                    &completion.contract_address,
                    completion.pending,
                )?;
            }
            // Queued instructions may revoke the selected permission or change the live contract
            // binding. Recheck after they finish so a stale authorization cannot guard durable
            // writes merely because it was valid at the start of overlay application.
            if let Some(authorization) = self.entrypoint_authorization.as_ref() {
                authorization.validate_for_authority(&state_tx.world, authority)?;
            }
            self.validate_durable_authorizations(&state_tx.world)?;
            crate::smartcontracts::ivm::host::HostExecutionArtifacts::record_completed_axt_states(
                state_tx,
                self.completed_axt.clone(),
            );
            for (path, value) in &self.durable_state_overlay {
                if let Some(authorization) = self
                    .durable_state_authorizations
                    .get(path)
                    .and_then(Option::as_ref)
                {
                    authorization.validate(&state_tx.world)?;
                }
                if let Some(stored) = value {
                    state_tx
                        .world
                        .smart_contract_state
                        .insert(path.clone(), stored.clone());
                } else {
                    state_tx.world.smart_contract_state.remove(path.clone());
                }
            }
            Ok(())
        })();
        state_tx.sccp_recording_proof_verified = prior_sccp_recording_proof_verified;
        result
    }

    fn with_entrypoint_authorization(
        mut self,
        authorization: Option<ContractEntrypointAuthorizationSnapshot>,
    ) -> Self {
        self.entrypoint_authorization = authorization;
        self
    }

    fn with_lifecycle_completion(
        mut self,
        contract_address: &ContractAddress,
        pending: Option<code::PendingContractLifecycle>,
    ) -> Self {
        self.lifecycle_completion = pending.map(|pending| OverlayLifecycleCompletion {
            contract_address: contract_address.clone(),
            pending,
        });
        self
    }
}

fn tx_overlay_from_host_queued<R: StateReadOnly>(
    state_ro: &R,
    queued: Vec<crate::smartcontracts::ivm::host::QueuedInstruction>,
    ivm_gas_used: u64,
    durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
    durable_state_authorizations: BTreeMap<Name, Option<ContractEntrypointAuthorizationSnapshot>>,
) -> TxOverlay {
    let mut queued_instructions: Vec<_> = queued
        .iter()
        .map(|queued| queued.instruction.clone())
        .collect();
    let mut execution_contexts: Vec<_> = queued
        .into_iter()
        .map(|queued| OverlayInstructionExecutionContext {
            authority: queued.authority,
            contract_runtime_context: queued.contract_runtime_context,
            entrypoint_authorization: queued.entrypoint_authorization,
        })
        .collect();
    prune_redundant_contract_ops_with_metadata(
        state_ro,
        &mut queued_instructions,
        Some(&mut execution_contexts),
    );
    TxOverlay::from_host_execution(
        queued_instructions,
        execution_contexts,
        ivm_gas_used,
        Vec::new(),
        durable_state_overlay,
        durable_state_authorizations,
    )
}

fn tx_overlay_from_ivm_proved_replay<R: StateReadOnly>(
    state_ro: &R,
    replay: IvmProvedReplay,
) -> TxOverlay {
    let IvmProvedReplay {
        queued: replay_queued,
        completed_axt,
        durable_state_overlay,
        gas_used,
        events_commitment: _,
        trace_hash: _,
    } = replay;
    let mut queued_instructions: Vec<_> = replay_queued
        .iter()
        .map(|queued| queued.instruction.clone())
        .collect();
    let mut execution_contexts: Vec<_> = replay_queued
        .into_iter()
        .map(|queued| OverlayInstructionExecutionContext {
            authority: queued.authority,
            contract_runtime_context: queued.contract_runtime_context,
            entrypoint_authorization: queued.entrypoint_authorization,
        })
        .collect();
    prune_redundant_contract_ops_with_metadata(
        state_ro,
        &mut queued_instructions,
        Some(&mut execution_contexts),
    );
    let queued = queued_instructions
        .into_iter()
        .zip(execution_contexts)
        .map(|(instruction, execution_context)| {
            crate::smartcontracts::ivm::host::QueuedInstruction {
                instruction,
                authority: execution_context.authority,
                contract_runtime_context: execution_context.contract_runtime_context,
                entrypoint_authorization: execution_context.entrypoint_authorization,
            }
        })
        .collect();
    TxOverlay::from_ivm_proved_execution(queued, gas_used, completed_axt, durable_state_overlay)
}

/// Build an overlay for a signed transaction without mutating state.
///
/// # Errors
/// Returns an error when the IVM header fails policy checks, loading fails, or VM execution fails.
pub fn build_overlay_for_transaction<R>(
    tx: &SignedTransaction,
    state_ro: &R,
) -> Result<TxOverlay, OverlayBuildError>
where
    R: StateReadOnly + QueryStateSource,
{
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    build_overlay_for_transaction_with_cache(tx, state_ro, &mut ivm_cache)
}

/// Build an overlay for a signed transaction using a caller-provided IVM cache.
///
/// # Errors
/// Returns an error when the IVM header fails policy checks, loading fails, or VM execution fails.
#[allow(clippy::too_many_lines)]
pub fn build_overlay_for_transaction_with_cache<R>(
    tx: &SignedTransaction,
    state_ro: &R,
    ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
) -> Result<TxOverlay, OverlayBuildError>
where
    R: StateReadOnly + QueryStateSource,
{
    match tx.instructions() {
        Executable::Instructions(batch) => {
            // We already have fully-formed owned instructions; just clone boxes.
            let mut instrs: Vec<InstructionBox> = batch.iter().cloned().collect();
            prune_redundant_contract_ops(state_ro, &mut instrs);
            Ok(TxOverlay::from_instructions(instrs))
        }
        Executable::ContractCall(call) => {
            let identity = code::fetch_bound_contract_identity(state_ro, &call.contract_address)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` not found in WSV",
                        call.contract_address
                    ))
                })?;
            let code_hash = identity.code_hash;
            let manifest = state_ro
                .world()
                .contract_manifests()
                .get(&code_hash)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` has no manifest",
                        call.contract_address
                    ))
                })?;
            let code_bytes = state_ro
                .world()
                .contract_code()
                .get(&code_hash)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` has no bytecode",
                        call.contract_address
                    ))
                })?;
            let summary = ivm_cache
                .summarize_program_with_hash(code_hash, code_bytes.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let gas_limit = require_tx_gas_limit(tx)?;
            let meta = summary.metadata.clone();
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;

            let code_offset = summary.code_offset;
            let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
            if wants_zk && !(state_ro.zk().halo2.enabled || state_ro.zk().stark.enabled) {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }

            enforce_pre_execution_policy(
                state_ro.pipeline().ivm_max_cycles_upper_bound,
                &meta,
                code_offset,
                code_bytes.as_ref(),
            )?;
            validate_bound_contract_manifest(manifest, &summary)?;

            let amx_analysis = cached_amx_analysis(ivm_cache, &summary, code_bytes.as_ref())?;
            let lifecycle_transition = crate::executor::validate_prepared_contract_lifecycle_call(
                state_ro.world(),
                &call.contract_address,
                summary.code_hash,
                summary.prepared_contract(),
                &call.entrypoint,
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let entrypoint_authorization = crate::executor::authorize_prepared_contract_selector(
                state_ro.world(),
                tx.authority(),
                summary.prepared_contract(),
                &call.entrypoint,
                &identity,
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let contract_call_context = parse_prepared_contract_invocation_execution_context(
                call,
                summary.prepared_contract(),
                gas_limit,
                None,
            )?;
            let mut vm = summary
                .checkout_runtime(gas_limit)
                .map_err(OverlayBuildError::IvmLoad)?;
            let contract_subject =
                code::fetch_bound_contract_subject(state_ro, &call.contract_address).ok_or_else(
                    || {
                        OverlayBuildError::ContractCall(format!(
                            "contract instance `{}` has no valid subject binding",
                            call.contract_address
                        ))
                    },
                )?;
            let contract_runtime_context = Some(crate::executor::ContractRuntimeExecutionContext {
                contract_subject,
                contract_address: call.contract_address.clone(),
                contract_alias: identity.contract_alias.clone(),
                entrypoint: contract_call_context
                    .entrypoint
                    .clone()
                    .expect("contract invocation parser must set entrypoint"),
            });

            let accounts = state_ro.accounts_snapshot();
            let streaming_meta = resolve_streaming_metadata(state_ro, tx.authority());
            let mut host =
                crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts_and_argument_record(
                    tx.authority().clone(),
                    Arc::clone(&accounts),
                    contract_call_context.argument_record.clone(),
                );
            host.set_prepared_contract_cache(summary.prepared_contract_cache());
            host.set_amx_analysis(amx_analysis);
            let amx_limits = crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                state_ro.pipeline(),
            );
            host.set_amx_limits(amx_limits);
            host.set_axt_timing(state_ro.nexus().axt);
            host.hydrate_axt_replay_ledger(state_ro);
            host.set_public_inputs_from_parameters(state_ro.world().parameters());
            host.set_vrf_epoch_seeds_from_world(state_ro.world());
            host.set_query_state(state_ro);
            host.set_contract_runtime_context(contract_runtime_context.clone());
            host.set_contract_entrypoint_authorization(Some(entrypoint_authorization.clone()));
            if let Some(pending) = lifecycle_transition {
                host.set_contract_lifecycle_transition(&call.contract_address, pending);
            }
            let snapshot = state_ro.axt_policy_snapshot();
            host = host.with_axt_policy_snapshot(&snapshot);
            apply_streaming_metadata(&mut host, streaming_meta);
            #[cfg(feature = "telemetry")]
            host.set_telemetry(state_ro.metrics().clone());
            host.set_crypto_config(state_ro.crypto());
            host.set_zk_config(state_ro.zk());
            host.set_chain_id(state_ro.chain_id());
            host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
                .map_err(OverlayBuildError::IvmRun)?;
            vm.set_gas_limit(gas_limit);
            apply_contract_call_execution_context(&mut vm, Some(&contract_call_context))?;
            vm.set_zk_trace_enabled(false);
            run_vm_with_host(&mut vm, &mut host)?;
            let ivm_gas_used = gas_limit.saturating_sub(vm.remaining_gas());
            let transport_caps_snapshot = host.transport_caps_snapshot().copied();
            let negotiated_caps_snapshot = host.negotiated_caps_snapshot().copied();
            let queued = host.drain_queued_instructions_with_contract_runtime_context(
                contract_runtime_context.clone(),
            );
            let (durable_state_overlay, durable_state_authorizations) =
                host.drain_durable_state_overlay_with_authorizations();
            if state_ro.zk().halo2.enabled && vm.zk_mode_enabled() {
                let trace = vm.register_trace();
                if !trace.is_empty() {
                    let constraints = vm.constraints().to_vec();
                    let mem_log = vm.memory_log().to_vec();
                    let reg_log = vm.register_log().to_vec();
                    let step_log = vm.step_log().to_vec();
                    let code_hash = vm.code_hash();
                    let tx_hash = iroha_crypto::Hash::prehashed(*tx.hash().as_ref());
                    let job = crate::pipeline::zk_lane::ZkTask {
                        tx_hash: Some(tx_hash),
                        code_hash,
                        program: summary.prepared_contract().shared_artifact(),
                        header: None,
                        trace,
                        constraints,
                        mem_log,
                        reg_log,
                        step_log,
                        transport_capabilities: transport_caps_snapshot,
                        negotiated_capabilities: negotiated_caps_snapshot,
                    };
                    let _ = crate::pipeline::zk_lane::try_submit(job);
                }
            }

            Ok(tx_overlay_from_host_queued(
                state_ro,
                queued,
                ivm_gas_used,
                durable_state_overlay,
                durable_state_authorizations,
            )
            .with_entrypoint_authorization(Some(entrypoint_authorization))
            .with_lifecycle_completion(&call.contract_address, lifecycle_transition))
        }
        Executable::Ivm(bytecode) => {
            // Validate header against node policy
            let summary = ivm_cache
                .summarize_program(bytecode.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let gas_limit = require_tx_gas_limit(tx)?;
            let meta = summary.metadata.clone();
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            // ABI gating is handled in validate_header_policy (v1-only release).

            let code_offset = summary.code_offset;
            let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
            if wants_zk && !(state_ro.zk().halo2.enabled || state_ro.zk().stark.enabled) {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }

            enforce_pre_execution_policy(
                state_ro.pipeline().ivm_max_cycles_upper_bound,
                &meta,
                code_offset,
                bytecode.as_ref(),
            )?;
            validate_contract_binding(state_ro, tx, &summary)?;

            let amx_analysis = cached_amx_analysis(ivm_cache, &summary, bytecode.as_ref())?;
            let selector = crate::executor::requested_contract_entrypoint(tx.metadata())
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(
                        "self-describing raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                            .to_owned(),
                    )
                })?;
            let identity = crate::executor::require_raw_contract_runtime_identity(
                state_ro.world(),
                summary.code_hash,
                tx.metadata(),
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let entrypoint_authorization =
                crate::executor::authorize_prepared_raw_contract_selector(
                    state_ro.world(),
                    tx.authority(),
                    summary.prepared_contract(),
                    &selector,
                    &identity,
                )
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let contract_call_context = parse_prepared_contract_call_execution_context(
                tx.metadata(),
                summary.prepared_contract(),
                gas_limit,
            )?;
            let contract_subject =
                code::fetch_bound_contract_subject(state_ro, &identity.contract_address)
                    .ok_or_else(|| {
                        OverlayBuildError::ContractCall(format!(
                            "contract instance `{}` has no valid subject binding",
                            identity.contract_address
                        ))
                    })?;
            let contract_runtime_context = Some(crate::executor::ContractRuntimeExecutionContext {
                contract_subject,
                contract_address: identity.contract_address.clone(),
                contract_alias: identity.contract_alias.clone(),
                entrypoint: selector,
            });
            let mut vm = summary
                .checkout_runtime(gas_limit)
                .map_err(OverlayBuildError::IvmLoad)?;

            // Run CoreHost to collect queued ISIs
            // Snapshot of accounts for deterministic helpers
            let accounts = state_ro.accounts_snapshot();
            let streaming_meta = resolve_streaming_metadata(state_ro, tx.authority());
            let mut host = if let Some(context) = contract_call_context.as_ref() {
                crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts_and_argument_record(
                    tx.authority().clone(),
                    Arc::clone(&accounts),
                    context.argument_record.clone(),
                )
            } else {
                crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts(
                    tx.authority().clone(),
                    Arc::clone(&accounts),
                )
            };
            host.set_prepared_contract_cache(summary.prepared_contract_cache());
            host.set_amx_analysis(amx_analysis);
            let amx_limits = crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                state_ro.pipeline(),
            );
            host.set_amx_limits(amx_limits);
            host.set_axt_timing(state_ro.nexus().axt);
            host.hydrate_axt_replay_ledger(state_ro);
            host.set_public_inputs_from_parameters(state_ro.world().parameters());
            host.set_vrf_epoch_seeds_from_world(state_ro.world());
            host.set_query_state(state_ro);
            host.set_contract_runtime_context(contract_runtime_context.clone());
            host.set_contract_entrypoint_authorization(Some(entrypoint_authorization.clone()));
            host.set_bound_contract_records_by_subject_snapshot(
                code::snapshot_bound_contract_records_by_subject(state_ro),
            );
            let snapshot = state_ro.axt_policy_snapshot();
            host = host.with_axt_policy_snapshot(&snapshot);
            apply_streaming_metadata(&mut host, streaming_meta);
            #[cfg(feature = "telemetry")]
            host.set_telemetry(state_ro.metrics().clone());
            host.set_crypto_config(state_ro.crypto());
            host.set_zk_config(state_ro.zk());
            host.set_chain_id(state_ro.chain_id());
            host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
                .map_err(OverlayBuildError::IvmRun)?;
            vm.set_gas_limit(gas_limit);
            apply_contract_call_execution_context(&mut vm, contract_call_context.as_ref())?;
            vm.set_zk_trace_enabled(false);
            run_vm_with_host(&mut vm, &mut host)?;
            let ivm_gas_used = gas_limit.saturating_sub(vm.remaining_gas());
            let transport_caps_snapshot = host.transport_caps_snapshot().copied();
            let negotiated_caps_snapshot = host.negotiated_caps_snapshot().copied();
            let mut queued = host.drain_queued_instructions_with_contract_runtime_context(
                contract_runtime_context.clone(),
            );
            let (durable_state_overlay, durable_state_authorizations) =
                host.drain_durable_state_overlay_with_authorizations();
            // Emit a ZK-lane job with the formal trace (non-forking background verification)
            if state_ro.zk().halo2.enabled && vm.zk_mode_enabled() {
                let trace = vm.register_trace();
                if !trace.is_empty() {
                    let constraints = vm.constraints().to_vec();
                    let mem_log = vm.memory_log().to_vec();
                    let reg_log = vm.register_log().to_vec();
                    let step_log = vm.step_log().to_vec();
                    let code_hash = vm.code_hash();
                    let tx_hash = iroha_crypto::Hash::prehashed(*tx.hash().as_ref());
                    let job = crate::pipeline::zk_lane::ZkTask {
                        tx_hash: Some(tx_hash),
                        code_hash,
                        program: summary.prepared_contract().shared_artifact(),
                        header: None,
                        trace,
                        constraints,
                        mem_log,
                        reg_log,
                        step_log,
                        transport_capabilities: transport_caps_snapshot,
                        negotiated_capabilities: negotiated_caps_snapshot,
                    };
                    let _ = crate::pipeline::zk_lane::try_submit(job);
                }
            }

            append_verified_contract_metadata_registration_to_queued(
                state_ro,
                tx,
                &summary,
                bytecode.as_ref(),
                &mut queued,
                contract_runtime_context.as_ref(),
                &entrypoint_authorization,
            )?;
            Ok(tx_overlay_from_host_queued(
                state_ro,
                queued,
                ivm_gas_used,
                durable_state_overlay,
                durable_state_authorizations,
            )
            .with_entrypoint_authorization(Some(entrypoint_authorization)))
        }
        Executable::IvmProved(proved) => {
            // Validate header against node policy (same checks as `Executable::Ivm`).
            let summary = ivm_cache
                .summarize_program(proved.bytecode.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let gas_limit = require_tx_gas_limit(tx)?;
            let meta = summary.metadata.clone();
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;

            let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
            if !wants_zk {
                return Err(OverlayBuildError::ZkProof(
                    "Executable::IvmProved requires IVM ZK mode bit (mode & ZK != 0)".to_owned(),
                ));
            }
            if wants_zk && !(state_ro.zk().halo2.enabled || state_ro.zk().stark.enabled) {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }

            enforce_pre_execution_policy(
                state_ro.pipeline().ivm_max_cycles_upper_bound,
                &meta,
                summary.code_offset,
                proved.bytecode.as_ref(),
            )?;
            validate_contract_binding(state_ro, tx, &summary)?;
            let selector = crate::executor::requested_contract_entrypoint(tx.metadata())
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(
                        "self-describing proved raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                            .to_owned(),
                    )
                })?;
            let identity = crate::executor::require_raw_contract_runtime_identity(
                state_ro.world(),
                summary.code_hash,
                tx.metadata(),
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let entrypoint_authorization =
                crate::executor::authorize_prepared_raw_contract_selector(
                    state_ro.world(),
                    tx.authority(),
                    summary.prepared_contract(),
                    &selector,
                    &identity,
                )
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;

            // Proved executions do not support the implicit manifest registration append;
            // if a manifest is attached and missing from WSV, reject deterministically.
            enforce_manifest_is_pre_registered(state_ro, tx, summary.code_hash)?;

            let replay = verify_ivm_proved_execution(state_ro, tx, proved, &summary)?;
            let _ = gas_limit; // still required for admission (fees), even when skipping VM.
            Ok(tx_overlay_from_ivm_proved_replay(state_ro, replay)
                .with_entrypoint_authorization(Some(entrypoint_authorization)))
        }
    }
}

/// Build an overlay for a transaction using a pre-captured accounts snapshot.
/// Build an overlay for a signed transaction, using a provided snapshot of accounts.
///
/// # Errors
/// Returns an error if the IVM header fails policy checks or running the VM fails.
pub fn build_overlay_for_transaction_with_accounts(
    tx: &SignedTransaction,
    accounts: &[AccountId],
) -> Result<TxOverlay, OverlayBuildError> {
    match tx.instructions() {
        Executable::Instructions(batch) => {
            let instrs: Vec<InstructionBox> = batch.iter().cloned().collect();
            Ok(TxOverlay::from_instructions(instrs))
        }
        Executable::ContractCall(_) => Err(OverlayBuildError::ContractCall(
            "Executable::ContractCall requires a full state view for overlay building".to_owned(),
        )),
        Executable::Ivm(bytecode) => {
            let parsed = ivm::ProgramMetadata::parse(bytecode.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let meta = parsed.metadata;
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            let code_offset = parsed.code_offset;
            let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
            if wants_zk {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }
            let pipeline = default_pipeline_config();
            enforce_pre_execution_policy(
                pipeline.ivm_max_cycles_upper_bound,
                &meta,
                code_offset,
                bytecode.as_ref(),
            )?;
            let tx_gas_limit = require_tx_gas_limit(tx)?;
            reject_raw_contract_without_state(bytecode.as_ref())?;
            let mut vm = ivm::IVM::new(tx_gas_limit);
            let contract_call_context = parse_raw_contract_call_execution_context(
                tx.metadata(),
                bytecode.as_ref(),
                tx_gas_limit,
            )?;
            let mut host = if let Some(context) = contract_call_context.as_ref() {
                crate::smartcontracts::ivm::host::CoreHost::with_accounts_and_argument_record(
                    tx.authority().clone(),
                    Arc::new(accounts.to_vec()),
                    context.argument_record.clone(),
                )
            } else {
                crate::smartcontracts::ivm::host::CoreHost::with_accounts(
                    tx.authority().clone(),
                    Arc::new(accounts.to_vec()),
                )
            };
            apply_streaming_metadata(&mut host, StreamingOverlayMetadata::default());
            vm.set_host(host);
            vm.load_program(bytecode.as_ref())
                .map_err(OverlayBuildError::IvmLoad)?;
            vm.set_gas_limit(tx_gas_limit);
            apply_contract_call_execution_context(&mut vm, contract_call_context.as_ref())?;
            run_vm(&mut vm)?;
            let ivm_gas_used = tx_gas_limit.saturating_sub(vm.remaining_gas());
            let (mut queued, durable_state_overlay) = if let Some(h) = vm.host_mut_any()
                && let Some(host) = h.downcast_mut::<crate::smartcontracts::ivm::host::CoreHost>()
            {
                (
                    host.drain_instructions(),
                    host.drain_durable_state_overlay(),
                )
            } else {
                (Vec::new(), BTreeMap::new())
            };
            append_verified_contract_metadata_registration_without_state(
                tx,
                bytecode.as_ref(),
                &mut queued,
            )?;
            Ok(TxOverlay::from_ivm_execution(
                queued,
                ivm_gas_used,
                durable_state_overlay,
            ))
        }
        Executable::IvmProved(_) => Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved requires a full state view for proof verification".to_owned(),
        )),
    }
}

/// Build an overlay and optionally capture dynamic state access in the same VM run.
///
/// # Errors
/// Returns an error if the IVM header fails policy checks or running the VM fails.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_prepared_overlay_for_transaction_with_accounts_zk<R>(
    tx: &SignedTransaction,
    accounts: Arc<Vec<AccountId>>,
    state_ro: &R,
    zk_enabled: bool,
    header: &BlockHeader,
    streaming_meta: StreamingOverlayMetadata,
    ivm_cache: &mut IvmCache,
    capture_access_log: bool,
    reused_argument_record: Option<ivm::PreparedArgumentRecord>,
) -> Result<PreparedTxOverlay, OverlayBuildError>
where
    R: StateReadOnly + QueryStateSource,
{
    match tx.instructions() {
        Executable::Instructions(batch) => {
            let instrs: Vec<InstructionBox> = batch.iter().cloned().collect();
            Ok(PreparedTxOverlay::new(
                TxOverlay::from_instructions(instrs),
                None,
                VmAccessFence::None,
                false,
            ))
        }
        Executable::ContractCall(call) => {
            #[cfg(feature = "telemetry")]
            let program_prepare_start = Instant::now();
            let identity = code::fetch_bound_contract_identity(state_ro, &call.contract_address)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` not found in WSV",
                        call.contract_address
                    ))
                })?;
            let code_hash = identity.code_hash;
            let manifest = state_ro
                .world()
                .contract_manifests()
                .get(&code_hash)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` has no manifest",
                        call.contract_address
                    ))
                })?;
            let code_bytes = state_ro
                .world()
                .contract_code()
                .get(&code_hash)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` has no bytecode",
                        call.contract_address
                    ))
                })?;
            let summary = ivm_cache
                .summarize_program_with_hash(code_hash, code_bytes.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let meta = summary.metadata.clone();
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            let code_offset = summary.code_offset;
            let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
            if wants_zk && !zk_enabled {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }
            enforce_pre_execution_policy(
                state_ro.pipeline().ivm_max_cycles_upper_bound,
                &meta,
                code_offset,
                code_bytes.as_ref(),
            )?;
            validate_bound_contract_manifest(manifest, &summary)?;
            let tx_gas_limit = require_tx_gas_limit(tx)?;
            let amx_analysis = cached_amx_analysis(ivm_cache, &summary, code_bytes.as_ref())?;
            let access_fence = VmAccessFence::from_program_analysis(&amx_analysis);
            let force_live_rebuild = VmAccessFence::requires_live_rebuild(&amx_analysis);
            let lifecycle_transition = crate::executor::validate_prepared_contract_lifecycle_call(
                state_ro.world(),
                &call.contract_address,
                summary.code_hash,
                summary.prepared_contract(),
                &call.entrypoint,
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let entrypoint_authorization = crate::executor::authorize_prepared_contract_selector(
                state_ro.world(),
                tx.authority(),
                summary.prepared_contract(),
                &call.entrypoint,
                &identity,
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let contract_call_context = parse_prepared_contract_invocation_execution_context(
                call,
                summary.prepared_contract(),
                tx_gas_limit,
                reused_argument_record.as_ref(),
            )?;
            let mut vm = summary
                .checkout_runtime(tx_gas_limit)
                .map_err(OverlayBuildError::IvmLoad)?;
            #[cfg(feature = "telemetry")]
            observe_overlay_stage_ms(state_ro, "overlay_program_prepare", program_prepare_start);
            let contract_subject =
                code::fetch_bound_contract_subject(state_ro, &call.contract_address).ok_or_else(
                    || {
                        OverlayBuildError::ContractCall(format!(
                            "contract instance `{}` has no valid subject binding",
                            call.contract_address
                        ))
                    },
                )?;
            let contract_runtime_context = Some(crate::executor::ContractRuntimeExecutionContext {
                contract_subject,
                contract_address: call.contract_address.clone(),
                contract_alias: identity.contract_alias.clone(),
                entrypoint: contract_call_context
                    .entrypoint
                    .clone()
                    .expect("contract invocation parser must set entrypoint"),
            });
            let mut host: crate::smartcontracts::ivm::host::CoreHostImpl<
                crate::smartcontracts::ivm::host::QueryStateSlot<_>,
            > = crate::smartcontracts::ivm::host::CoreHostImpl::<
                crate::smartcontracts::ivm::host::QueryStateSlot<_>,
            >::with_accounts_and_argument_record(
                tx.authority().clone(),
                Arc::clone(&accounts),
                contract_call_context.argument_record.clone(),
            );
            host.set_prepared_contract_cache(summary.prepared_contract_cache());
            host.set_amx_analysis(amx_analysis);
            #[cfg(feature = "telemetry")]
            let host_hydrate_start = Instant::now();
            let amx_limits = crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                state_ro.pipeline(),
            );
            host.set_amx_limits(amx_limits);
            host.set_axt_timing(state_ro.nexus().axt);
            host.hydrate_axt_replay_ledger(state_ro);
            host.set_public_inputs_from_parameters(state_ro.world().parameters());
            host.set_vrf_epoch_seeds_from_world(state_ro.world());
            host.set_query_state(state_ro);
            host.set_contract_runtime_context(contract_runtime_context.clone());
            host.set_contract_entrypoint_authorization(Some(entrypoint_authorization.clone()));
            let snapshot = state_ro.axt_policy_snapshot();
            host = host.with_axt_policy_snapshot(&snapshot);
            apply_streaming_metadata(&mut host, streaming_meta);
            #[cfg(feature = "telemetry")]
            host.set_telemetry(state_ro.metrics().clone());
            host.set_crypto_config(state_ro.crypto());
            host.set_zk_config(state_ro.zk());
            host.set_chain_id(state_ro.chain_id());
            host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
                .map_err(OverlayBuildError::IvmRun)?;
            if capture_access_log {
                host = host.with_access_logging();
            }
            begin_overlay_access_log(&mut host, capture_access_log)?;
            if let Some(pending) = lifecycle_transition {
                host.set_contract_lifecycle_transition(&call.contract_address, pending);
            }
            vm.set_gas_limit(tx_gas_limit);
            apply_contract_call_execution_context(&mut vm, Some(&contract_call_context))?;
            vm.set_zk_trace_enabled(false);
            #[cfg(feature = "telemetry")]
            observe_overlay_stage_ms(state_ro, "overlay_host_hydrate", host_hydrate_start);
            #[cfg(feature = "telemetry")]
            let vm_run_start = Instant::now();
            run_vm_with_host(&mut vm, &mut host)?;
            #[cfg(feature = "telemetry")]
            observe_overlay_stage_ms(state_ro, "overlay_vm_run", vm_run_start);
            let ivm_gas_used = tx_gas_limit.saturating_sub(vm.remaining_gas());
            let access_log = finish_overlay_access_log(&mut host, capture_access_log)?;
            let transport_caps_snapshot = host.transport_caps_snapshot().copied();
            let negotiated_caps_snapshot = host.negotiated_caps_snapshot().copied();
            let queued = host.drain_queued_instructions_with_contract_runtime_context(
                contract_runtime_context.clone(),
            );
            let (durable_state_overlay, durable_state_authorizations) =
                host.drain_durable_state_overlay_with_authorizations();
            if state_ro.zk().halo2.enabled && vm.zk_mode_enabled() {
                let trace = vm.register_trace();
                if !trace.is_empty() {
                    let constraints = vm.constraints().to_vec();
                    let mem_log = vm.memory_log().to_vec();
                    let reg_log = vm.register_log().to_vec();
                    let step_log = vm.step_log().to_vec();
                    let code_hash = vm.code_hash();
                    let tx_hash = iroha_crypto::Hash::prehashed(*tx.hash().as_ref());
                    let job = crate::pipeline::zk_lane::ZkTask {
                        tx_hash: Some(tx_hash),
                        code_hash,
                        program: summary.prepared_contract().shared_artifact(),
                        header: Some(*header),
                        trace,
                        constraints,
                        mem_log,
                        reg_log,
                        step_log,
                        transport_capabilities: transport_caps_snapshot,
                        negotiated_capabilities: negotiated_caps_snapshot,
                    };
                    let _ = crate::pipeline::zk_lane::try_submit(job);
                }
            }
            Ok(PreparedTxOverlay::new(
                tx_overlay_from_host_queued(
                    state_ro,
                    queued,
                    ivm_gas_used,
                    durable_state_overlay,
                    durable_state_authorizations,
                )
                .with_entrypoint_authorization(Some(entrypoint_authorization))
                .with_lifecycle_completion(&call.contract_address, lifecycle_transition),
                access_log,
                access_fence,
                force_live_rebuild,
            )
            .with_prepared_argument_record(contract_call_context.argument_record.clone()))
        }
        Executable::Ivm(bytecode) => {
            #[cfg(feature = "telemetry")]
            let program_prepare_start = Instant::now();
            let summary = ivm_cache
                .summarize_program(bytecode.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let meta = summary.metadata.clone();
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            let code_offset = summary.code_offset;
            let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
            if wants_zk && !zk_enabled {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }
            enforce_pre_execution_policy(
                state_ro.pipeline().ivm_max_cycles_upper_bound,
                &meta,
                code_offset,
                bytecode.as_ref(),
            )?;
            validate_contract_binding(state_ro, tx, &summary)?;
            let tx_gas_limit = require_tx_gas_limit(tx)?;
            let amx_analysis = cached_amx_analysis(ivm_cache, &summary, bytecode.as_ref())?;
            let access_fence = VmAccessFence::from_program_analysis(&amx_analysis);
            let force_live_rebuild = VmAccessFence::requires_live_rebuild(&amx_analysis);
            let selector = crate::executor::requested_contract_entrypoint(tx.metadata())
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(
                        "self-describing raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                            .to_owned(),
                    )
                })?;
            let identity = crate::executor::require_raw_contract_runtime_identity(
                state_ro.world(),
                summary.code_hash,
                tx.metadata(),
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let entrypoint_authorization =
                crate::executor::authorize_prepared_raw_contract_selector(
                    state_ro.world(),
                    tx.authority(),
                    summary.prepared_contract(),
                    &selector,
                    &identity,
                )
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let contract_call_context = parse_prepared_contract_call_execution_context(
                tx.metadata(),
                summary.prepared_contract(),
                tx_gas_limit,
            )?;
            let contract_subject =
                code::fetch_bound_contract_subject(state_ro, &identity.contract_address)
                    .ok_or_else(|| {
                        OverlayBuildError::ContractCall(format!(
                            "contract instance `{}` has no valid subject binding",
                            identity.contract_address
                        ))
                    })?;
            let contract_runtime_context = Some(crate::executor::ContractRuntimeExecutionContext {
                contract_subject,
                contract_address: identity.contract_address.clone(),
                contract_alias: identity.contract_alias.clone(),
                entrypoint: selector,
            });
            let mut vm = summary
                .checkout_runtime(tx_gas_limit)
                .map_err(OverlayBuildError::IvmLoad)?;
            #[cfg(feature = "telemetry")]
            observe_overlay_stage_ms(state_ro, "overlay_program_prepare", program_prepare_start);
            let mut host = if let Some(context) = contract_call_context.as_ref() {
                crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts_and_argument_record(
                    tx.authority().clone(),
                    Arc::clone(&accounts),
                    context.argument_record.clone(),
                )
            } else {
                crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts(
                    tx.authority().clone(),
                    Arc::clone(&accounts),
                )
            };
            host.set_prepared_contract_cache(summary.prepared_contract_cache());
            host.set_amx_analysis(amx_analysis);
            #[cfg(feature = "telemetry")]
            let host_hydrate_start = Instant::now();
            let amx_limits = crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                state_ro.pipeline(),
            );
            host.set_amx_limits(amx_limits);
            host.set_axt_timing(state_ro.nexus().axt);
            host.hydrate_axt_replay_ledger(state_ro);
            host.set_public_inputs_from_parameters(state_ro.world().parameters());
            host.set_vrf_epoch_seeds_from_world(state_ro.world());
            host.set_query_state(state_ro);
            host.set_contract_runtime_context(contract_runtime_context.clone());
            host.set_contract_entrypoint_authorization(Some(entrypoint_authorization.clone()));
            host.set_bound_contract_records_by_subject_snapshot(
                code::snapshot_bound_contract_records_by_subject(state_ro),
            );
            let snapshot = state_ro.axt_policy_snapshot();
            host = host.with_axt_policy_snapshot(&snapshot);
            apply_streaming_metadata(&mut host, streaming_meta);
            #[cfg(feature = "telemetry")]
            host.set_telemetry(state_ro.metrics().clone());
            host.set_crypto_config(state_ro.crypto());
            host.set_zk_config(state_ro.zk());
            host.set_chain_id(state_ro.chain_id());
            host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
                .map_err(OverlayBuildError::IvmRun)?;
            if capture_access_log {
                host = host.with_access_logging();
            }
            begin_overlay_access_log(&mut host, capture_access_log)?;
            vm.set_gas_limit(tx_gas_limit);
            apply_contract_call_execution_context(&mut vm, contract_call_context.as_ref())?;
            vm.set_zk_trace_enabled(false);
            #[cfg(feature = "telemetry")]
            observe_overlay_stage_ms(state_ro, "overlay_host_hydrate", host_hydrate_start);
            #[cfg(feature = "telemetry")]
            let vm_run_start = Instant::now();
            run_vm_with_host(&mut vm, &mut host)?;
            #[cfg(feature = "telemetry")]
            observe_overlay_stage_ms(state_ro, "overlay_vm_run", vm_run_start);
            let ivm_gas_used = tx_gas_limit.saturating_sub(vm.remaining_gas());
            let access_log = finish_overlay_access_log(&mut host, capture_access_log)?;
            let transport_caps_snapshot = host.transport_caps_snapshot().copied();
            let negotiated_caps_snapshot = host.negotiated_caps_snapshot().copied();
            let mut queued = host.drain_queued_instructions_with_contract_runtime_context(
                contract_runtime_context.clone(),
            );
            let (durable_state_overlay, durable_state_authorizations) =
                host.drain_durable_state_overlay_with_authorizations();
            if state_ro.zk().halo2.enabled && vm.zk_mode_enabled() {
                let trace = vm.register_trace();
                if !trace.is_empty() {
                    let constraints = vm.constraints().to_vec();
                    let mem_log = vm.memory_log().to_vec();
                    let reg_log = vm.register_log().to_vec();
                    let step_log = vm.step_log().to_vec();
                    let code_hash = vm.code_hash();
                    let tx_hash = iroha_crypto::Hash::prehashed(*tx.hash().as_ref());
                    let job = crate::pipeline::zk_lane::ZkTask {
                        tx_hash: Some(tx_hash),
                        code_hash,
                        program: summary.prepared_contract().shared_artifact(),
                        header: Some(*header),
                        trace,
                        constraints,
                        mem_log,
                        reg_log,
                        step_log,
                        transport_capabilities: transport_caps_snapshot,
                        negotiated_capabilities: negotiated_caps_snapshot,
                    };
                    let _ = crate::pipeline::zk_lane::try_submit(job);
                }
            }
            append_verified_contract_metadata_registration_to_queued(
                state_ro,
                tx,
                &summary,
                bytecode.as_ref(),
                &mut queued,
                contract_runtime_context.as_ref(),
                &entrypoint_authorization,
            )?;
            Ok(PreparedTxOverlay::new(
                tx_overlay_from_host_queued(
                    state_ro,
                    queued,
                    ivm_gas_used,
                    durable_state_overlay,
                    durable_state_authorizations,
                )
                .with_entrypoint_authorization(Some(entrypoint_authorization)),
                access_log,
                access_fence,
                force_live_rebuild,
            )
            .with_prepared_argument_record(
                contract_call_context
                    .as_ref()
                    .and_then(|context| context.argument_record.clone()),
            ))
        }
        Executable::IvmProved(proved) => {
            let summary = ivm_cache
                .summarize_program(proved.bytecode.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let meta = summary.metadata.clone();
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            let code_offset = summary.code_offset;
            let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
            if wants_zk && !zk_enabled {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }
            enforce_pre_execution_policy(
                state_ro.pipeline().ivm_max_cycles_upper_bound,
                &meta,
                code_offset,
                proved.bytecode.as_ref(),
            )?;
            validate_contract_binding(state_ro, tx, &summary)?;
            let selector = crate::executor::requested_contract_entrypoint(tx.metadata())
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(
                        "self-describing proved raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                            .to_owned(),
                    )
                })?;
            let identity = crate::executor::require_raw_contract_runtime_identity(
                state_ro.world(),
                summary.code_hash,
                tx.metadata(),
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let entrypoint_authorization =
                crate::executor::authorize_prepared_raw_contract_selector(
                    state_ro.world(),
                    tx.authority(),
                    summary.prepared_contract(),
                    &selector,
                    &identity,
                )
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;

            enforce_manifest_is_pre_registered(state_ro, tx, summary.code_hash)?;
            let replay = verify_ivm_proved_execution(state_ro, tx, proved, &summary)?;
            Ok(PreparedTxOverlay::new(
                tx_overlay_from_ivm_proved_replay(state_ro, replay)
                    .with_entrypoint_authorization(Some(entrypoint_authorization)),
                None,
                VmAccessFence::None,
                false,
            ))
        }
    }
}

/// Build an overlay for a transaction under quarantine limits.
///
/// Applies per-transaction execution caps when running IVM bytecode to collect queued ISIs:
/// - `max_cycles_cap`: if non-zero, caps VM cycles to `min(header.max_cycles, max_cycles_cap, upper_bound_cap)`.
/// - `max_millis_cap`: if non-zero, rejects the overlay if VM execution exceeds the wall-clock budget.
/// - `upper_bound_cap`: mandatory pipeline-wide upper bound on cycles.
///   Build an overlay for a transaction under quarantine caps (cycles/millis and upper bound).
///
/// # Errors
/// Returns an error if the IVM header fails policy checks or running the VM fails.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_overlay_for_transaction_quarantine(
    tx: &SignedTransaction,
    accounts: Arc<Vec<AccountId>>,
    state_ro: &(impl StateReadOnly + QueryStateSource),
    max_cycles_cap: u64,
    max_millis_cap: u64,
    upper_bound_cap: NonZeroU64,
    streaming_meta: StreamingOverlayMetadata,
    ivm_cache: &mut IvmCache,
) -> Result<TxOverlay, OverlayBuildError> {
    match tx.instructions() {
        Executable::Instructions(batch) => {
            // Built-in instruction batches do not use VM; return overlay directly.
            let instrs: Vec<InstructionBox> = batch.iter().cloned().collect();
            Ok(TxOverlay::from_instructions(instrs))
        }
        Executable::ContractCall(call) => {
            let identity = code::fetch_bound_contract_identity(state_ro, &call.contract_address)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` not found in WSV",
                        call.contract_address
                    ))
                })?;
            let code_hash = identity.code_hash;
            let manifest = state_ro
                .world()
                .contract_manifests()
                .get(&code_hash)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` has no manifest",
                        call.contract_address
                    ))
                })?;
            let code_bytes = state_ro
                .world()
                .contract_code()
                .get(&code_hash)
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(format!(
                        "contract instance `{}` has no bytecode",
                        call.contract_address
                    ))
                })?;
            let summary = ivm_cache
                .summarize_program_with_hash(code_hash, code_bytes.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let meta = summary.metadata.clone();
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            if meta.mode & ivm::ivm_mode::ZK != 0 {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }
            enforce_pre_execution_policy(
                state_ro.pipeline().ivm_max_cycles_upper_bound,
                &meta,
                summary.code_offset,
                code_bytes.as_ref(),
            )?;
            let tx_gas_limit = require_tx_gas_limit(tx)?;
            validate_bound_contract_manifest(manifest, &summary)?;
            let mut eff = meta.max_cycles.min(upper_bound_cap.get());
            if max_cycles_cap > 0 {
                eff = eff.min(max_cycles_cap);
            }
            let amx_analysis = cached_amx_analysis(ivm_cache, &summary, code_bytes.as_ref())?;
            let lifecycle_transition = crate::executor::validate_prepared_contract_lifecycle_call(
                state_ro.world(),
                &call.contract_address,
                summary.code_hash,
                summary.prepared_contract(),
                &call.entrypoint,
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let entrypoint_authorization = crate::executor::authorize_prepared_contract_selector(
                state_ro.world(),
                tx.authority(),
                summary.prepared_contract(),
                &call.entrypoint,
                &identity,
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let contract_call_context = parse_prepared_contract_invocation_execution_context(
                call,
                summary.prepared_contract(),
                tx_gas_limit,
                None,
            )?;
            let mut vm = summary
                .checkout_runtime(tx_gas_limit)
                .map_err(OverlayBuildError::IvmLoad)?;
            let contract_subject =
                code::fetch_bound_contract_subject(state_ro, &call.contract_address).ok_or_else(
                    || {
                        OverlayBuildError::ContractCall(format!(
                            "contract instance `{}` has no valid subject binding",
                            call.contract_address
                        ))
                    },
                )?;
            let contract_runtime_context = Some(crate::executor::ContractRuntimeExecutionContext {
                contract_subject,
                contract_address: call.contract_address.clone(),
                contract_alias: identity.contract_alias.clone(),
                entrypoint: contract_call_context
                    .entrypoint
                    .clone()
                    .expect("contract invocation parser must set entrypoint"),
            });
            let mut host: crate::smartcontracts::ivm::host::CoreHostImpl<
                crate::smartcontracts::ivm::host::QueryStateSlot<_>,
            > = crate::smartcontracts::ivm::host::CoreHostImpl::<
                crate::smartcontracts::ivm::host::QueryStateSlot<_>,
            >::with_accounts_and_argument_record(
                tx.authority().clone(),
                Arc::clone(&accounts),
                contract_call_context.argument_record.clone(),
            );
            host.set_prepared_contract_cache(summary.prepared_contract_cache());
            host.set_amx_analysis(amx_analysis);
            let amx_limits = crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                state_ro.pipeline(),
            );
            host.set_amx_limits(amx_limits);
            host.set_axt_timing(state_ro.nexus().axt);
            host.hydrate_axt_replay_ledger(state_ro);
            host.set_public_inputs_from_parameters(state_ro.world().parameters());
            host.set_vrf_epoch_seeds_from_world(state_ro.world());
            host.set_query_state(state_ro);
            host.set_contract_runtime_context(contract_runtime_context.clone());
            host.set_contract_entrypoint_authorization(Some(entrypoint_authorization.clone()));
            if let Some(pending) = lifecycle_transition {
                host.set_contract_lifecycle_transition(&call.contract_address, pending);
            }
            let snapshot = state_ro.axt_policy_snapshot();
            host = host.with_axt_policy_snapshot(&snapshot);
            apply_streaming_metadata(&mut host, streaming_meta);
            #[cfg(feature = "telemetry")]
            host.set_telemetry(state_ro.metrics().clone());
            host.set_crypto_config(state_ro.crypto());
            host.set_zk_config(state_ro.zk());
            host.set_chain_id(state_ro.chain_id());
            host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
                .map_err(OverlayBuildError::IvmRun)?;
            vm.set_max_cycles(eff);
            vm.set_gas_limit(tx_gas_limit);
            apply_contract_call_execution_context(&mut vm, Some(&contract_call_context))?;
            #[cfg(feature = "telemetry")]
            let t_start = std::time::Instant::now();
            let res = run_vm_with_host(&mut vm, &mut host);
            if max_millis_cap > 0 {
                let elapsed_ms = {
                    #[cfg(feature = "telemetry")]
                    {
                        t_start.elapsed().as_millis()
                    }
                    #[cfg(not(feature = "telemetry"))]
                    {
                        0
                    }
                };
                if elapsed_ms > u128::from(max_millis_cap) {
                    return Err(OverlayBuildError::IvmRun(ivm::VMError::ExceededMaxCycles));
                }
            }
            res?;
            let ivm_gas_used = tx_gas_limit.saturating_sub(vm.remaining_gas());
            let queued = host.drain_queued_instructions_with_contract_runtime_context(
                contract_runtime_context.clone(),
            );
            let (durable_state_overlay, durable_state_authorizations) =
                host.drain_durable_state_overlay_with_authorizations();
            Ok(tx_overlay_from_host_queued(
                state_ro,
                queued,
                ivm_gas_used,
                durable_state_overlay,
                durable_state_authorizations,
            )
            .with_entrypoint_authorization(Some(entrypoint_authorization))
            .with_lifecycle_completion(&call.contract_address, lifecycle_transition))
        }
        Executable::Ivm(bytecode) => {
            let summary = ivm_cache
                .summarize_program(bytecode.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let meta = summary.metadata.clone();
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            if meta.mode & ivm::ivm_mode::ZK != 0 {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }
            enforce_pre_execution_policy(
                state_ro.pipeline().ivm_max_cycles_upper_bound,
                &meta,
                summary.code_offset,
                bytecode.as_ref(),
            )?;
            validate_contract_binding(state_ro, tx, &summary)?;
            let tx_gas_limit = require_tx_gas_limit(tx)?;
            let mut eff = meta.max_cycles.min(upper_bound_cap.get());
            if max_cycles_cap > 0 {
                eff = eff.min(max_cycles_cap);
            }
            let selector = crate::executor::requested_contract_entrypoint(tx.metadata())
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?
                .ok_or_else(|| {
                    OverlayBuildError::ContractCall(
                        "self-describing raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                            .to_owned(),
                    )
                })?;
            let identity = crate::executor::require_raw_contract_runtime_identity(
                state_ro.world(),
                summary.code_hash,
                tx.metadata(),
            )
            .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let entrypoint_authorization =
                crate::executor::authorize_prepared_raw_contract_selector(
                    state_ro.world(),
                    tx.authority(),
                    summary.prepared_contract(),
                    &selector,
                    &identity,
                )
                .map_err(|error| OverlayBuildError::ContractCall(error.to_string()))?;
            let contract_call_context = parse_prepared_contract_call_execution_context(
                tx.metadata(),
                summary.prepared_contract(),
                tx_gas_limit,
            )?;
            let contract_subject =
                code::fetch_bound_contract_subject(state_ro, &identity.contract_address)
                    .ok_or_else(|| {
                        OverlayBuildError::ContractCall(format!(
                            "contract instance `{}` has no valid subject binding",
                            identity.contract_address
                        ))
                    })?;
            let contract_runtime_context = Some(crate::executor::ContractRuntimeExecutionContext {
                contract_subject,
                contract_address: identity.contract_address.clone(),
                contract_alias: identity.contract_alias.clone(),
                entrypoint: selector,
            });
            let mut vm = summary
                .checkout_runtime(tx_gas_limit)
                .map_err(OverlayBuildError::IvmLoad)?;
            let mut host = if let Some(context) = contract_call_context.as_ref() {
                crate::smartcontracts::ivm::host::CoreHost::with_accounts_and_argument_record(
                    tx.authority().clone(),
                    Arc::clone(&accounts),
                    context.argument_record.clone(),
                )
            } else {
                crate::smartcontracts::ivm::host::CoreHost::with_accounts(
                    tx.authority().clone(),
                    Arc::clone(&accounts),
                )
            };
            host.set_prepared_contract_cache(summary.prepared_contract_cache());
            host.set_contract_runtime_context(contract_runtime_context.clone());
            host.set_contract_entrypoint_authorization(Some(entrypoint_authorization.clone()));
            host.set_bound_contract_records_by_subject_snapshot(
                code::snapshot_bound_contract_records_by_subject(state_ro),
            );
            apply_streaming_metadata(&mut host, streaming_meta);
            vm.set_host(host);
            vm.set_max_cycles(eff);
            vm.set_gas_limit(tx_gas_limit);
            apply_contract_call_execution_context(&mut vm, contract_call_context.as_ref())?;
            // Run with a simple wall-clock budget check (post-hoc reject).
            #[cfg(feature = "telemetry")]
            let t_start = std::time::Instant::now();
            let res = run_vm(&mut vm);
            // Check wall-clock budget
            if max_millis_cap > 0 {
                let elapsed_ms = {
                    #[cfg(feature = "telemetry")]
                    {
                        t_start.elapsed().as_millis()
                    }
                    #[cfg(not(feature = "telemetry"))]
                    {
                        0
                    }
                };
                if elapsed_ms > u128::from(max_millis_cap) {
                    return Err(OverlayBuildError::IvmRun(ivm::VMError::ExceededMaxCycles));
                }
            }
            res?;
            let ivm_gas_used = tx_gas_limit.saturating_sub(vm.remaining_gas());
            let (queued, durable_state_overlay, durable_state_authorizations) = if let Some(h) =
                vm.host_mut_any()
                && let Some(host) = h.downcast_mut::<crate::smartcontracts::ivm::host::CoreHost>()
            {
                let queued = host.drain_queued_instructions_with_contract_runtime_context(
                    contract_runtime_context.clone(),
                );
                let (durable_state_overlay, durable_state_authorizations) =
                    host.drain_durable_state_overlay_with_authorizations();
                (queued, durable_state_overlay, durable_state_authorizations)
            } else {
                (Vec::new(), BTreeMap::new(), BTreeMap::new())
            };
            Ok(tx_overlay_from_host_queued(
                state_ro,
                queued,
                ivm_gas_used,
                durable_state_overlay,
                durable_state_authorizations,
            )
            .with_entrypoint_authorization(Some(entrypoint_authorization)))
        }
        Executable::IvmProved(_) => Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved is not supported in quarantine overlay building".to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests_overlay_manifest {
    use iroha_data_model::prelude::*;
    use iroha_primitives::json::Json;
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::state::State;

    fn build_wonderland_account(authority: &AccountId) -> iroha_data_model::account::Account {
        iroha_data_model::account::Account::new(authority.clone()).build(authority)
    }

    fn analysis_with_syscalls(numbers: &[u32]) -> ivm::analysis::ProgramAnalysis {
        ivm::analysis::ProgramAnalysis {
            metadata: ivm::ProgramMetadata::default(),
            instruction_count: numbers.len(),
            registers: ivm::analysis::RegisterUsage::default(),
            memory: ivm::analysis::MemoryAccesses::default(),
            syscalls: numbers
                .iter()
                .copied()
                .map(|number| ivm::analysis::SyscallUsage { number, count: 1 })
                .collect(),
        }
    }

    #[test]
    fn vm_access_fence_fails_closed_by_reachable_syscall_class() {
        assert_eq!(
            VmAccessFence::from_program_analysis(&analysis_with_syscalls(&[])),
            VmAccessFence::None
        );
        assert_eq!(
            VmAccessFence::from_program_analysis(&analysis_with_syscalls(&[
                ivm::syscalls::SYSCALL_STATE_GET,
                ivm::syscalls::SYSCALL_STATE_SET,
            ])),
            VmAccessFence::State
        );
        assert!(!VmAccessFence::requires_live_rebuild(
            &analysis_with_syscalls(&[ivm::syscalls::SYSCALL_STATE_GET])
        ));
        assert!(VmAccessFence::requires_live_rebuild(
            &analysis_with_syscalls(&[ivm::syscalls::SYSCALL_CORE_QUERY_GET])
        ));
        assert!(VmAccessFence::requires_live_rebuild(
            &analysis_with_syscalls(&[ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED])
        ));
        assert!(VmAccessFence::requires_live_rebuild(
            &analysis_with_syscalls(&[ivm::syscalls::SYSCALL_CALL_CONTRACT])
        ));
        for syscall in [
            ivm::syscalls::SYSCALL_CORE_QUERY_GET,
            ivm::syscalls::SYSCALL_CORE_QUERY_PAGE,
            ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED,
            ivm::syscalls::SYSCALL_CALL_CONTRACT,
            0x00ff_fffe,
        ] {
            assert_eq!(
                VmAccessFence::from_program_analysis(&analysis_with_syscalls(&[syscall])),
                VmAccessFence::Global,
                "syscall 0x{syscall:06x} must serialize globally"
            );
        }
    }

    #[test]
    fn overlay_error_retryability_excludes_state_invariant_failures() {
        assert!(
            OverlayBuildError::ContractCall("binding not found yet".to_owned())
                .may_change_with_live_state()
        );
        assert!(
            OverlayBuildError::IvmRun(ivm::VMError::PermissionDenied).may_change_with_live_state()
        );
        assert!(!OverlayBuildError::IvmHeaderParse.may_change_with_live_state());
        assert!(
            !OverlayBuildError::GasLimit("missing gas limit".to_owned())
                .may_change_with_live_state()
        );
        assert!(
            !OverlayBuildError::IvmLoad(ivm::VMError::InvalidMetadata).may_change_with_live_state()
        );
    }

    fn malformed_sccp_record_instruction() -> InstructionBox {
        crate::bridge::test_record_sccp_message(vec![0xFF]).into()
    }

    fn assert_sccp_proof_gate(error: ValidationFail) {
        assert!(
            matches!(
                &error,
                ValidationFail::InstructionFailed(
                    iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
                        message,
                    ),
                ) if message.contains("requires verified IVM proof")
            ),
            "unexpected SCCP proof-authority error: {error:?}"
        );
    }

    #[test]
    fn plain_overlay_rejects_sccp_recording_without_verified_proof() {
        let (authority, _) = gen_account_in("wonderland");
        let state = State::new_for_testing(
            crate::state::World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut state_tx = block.transaction();
        let overlay = TxOverlay::from_instructions(vec![malformed_sccp_record_instruction()]);

        let error = overlay
            .apply(&mut state_tx, &authority)
            .expect_err("plain overlays must not record SCCP messages");
        assert_sccp_proof_gate(error);
        assert!(!state_tx.sccp_recording_proof_verified);
    }

    #[test]
    fn proved_overlay_scopes_and_restores_sccp_recording_authority() {
        let (authority, _) = gen_account_in("wonderland");
        let state = State::new_for_testing(
            crate::state::World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut state_tx = block.transaction();
        let mut proved = TxOverlay::from_instructions(vec![malformed_sccp_record_instruction()]);
        proved.source = TxOverlaySource::IvmProved;

        let proved_error = proved
            .apply(&mut state_tx, &authority)
            .expect_err("proved SCCP record should pass the proof gate and reach lane admission");
        assert!(
            proved_error
                .to_string()
                .contains("requires nexus.enabled=true"),
            "proved overlay did not receive scoped SCCP proof authority: {proved_error:?}"
        );
        assert!(
            !state_tx.sccp_recording_proof_verified,
            "failed proved overlay must restore SCCP proof authority"
        );

        let plain = TxOverlay::from_instructions(vec![malformed_sccp_record_instruction()]);
        let plain_error = plain
            .apply(&mut state_tx, &authority)
            .expect_err("a later plain overlay must not inherit SCCP proof authority");
        assert_sccp_proof_gate(plain_error);
    }

    #[test]
    fn durable_state_read_snapshot_detects_value_and_descendant_changes() {
        fn state_with_entries(entries: &[(&str, &[u8])]) -> State {
            let mut world = crate::state::World::new();
            for (path, value) in entries {
                world.smart_contract_state_mut_for_testing().insert(
                    path.parse().expect("valid durable-state path"),
                    value.to_vec(),
                );
            }
            State::new_for_testing(
                world,
                crate::kura::Kura::blank_kura_for_testing(),
                crate::query::store::LiveQueryStore::start_test(),
            )
        }

        let (authority, keypair) = iroha_test_samples::gen_account_in("wonderland");
        let transaction = TransactionBuilder::new(ChainId::from("snapshot-test"), authority)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(Vec::new())))
            .sign(keypair.private_key());
        let mut access_log = ivm::host::AccessLog::default();
        access_log.read_keys.insert("counter".to_owned());
        access_log
            .durable_read_paths
            .extend(["counter".to_owned(), "sc/nested/counter".to_owned()]);
        access_log.durable_read_paths_complete = true;

        let initial = state_with_entries(&[
            ("counter", b"one"),
            ("counter-lexical-interloper", b"same"),
            ("sc/nested/counter", b"nested-one"),
            ("unrelated", b"stable"),
        ]);
        let snapshot =
            DurableStateReadSnapshot::capture(&transaction, Some(&access_log), &initial.view())
                .expect("non-empty VM read log creates a snapshot");
        assert!(snapshot.is_current(&initial.view()));

        let unrelated = state_with_entries(&[
            ("counter", b"one"),
            ("counter-lexical-interloper", b"same"),
            ("sc/nested/counter", b"nested-one"),
            ("unrelated", b"changed"),
        ]);
        assert!(snapshot.is_current(&unrelated.view()));

        let changed_value = state_with_entries(&[("counter", b"two")]);
        assert!(!snapshot.is_current(&changed_value.view()));

        let changed_concrete_scope = state_with_entries(&[
            ("counter", b"one"),
            ("counter-lexical-interloper", b"same"),
            ("sc/nested/counter", b"nested-two"),
        ]);
        assert!(!snapshot.is_current(&changed_concrete_scope.view()));

        let changed_descendant = state_with_entries(&[
            ("counter", b"one"),
            ("counter-lexical-interloper", b"same"),
            ("counter/child", b"new"),
            ("sc/nested/counter", b"nested-one"),
        ]);
        assert!(!snapshot.is_current(&changed_descendant.view()));
    }

    #[test]
    fn durable_state_read_snapshot_fails_closed_for_invalid_host_key() {
        let (authority, keypair) = iroha_test_samples::gen_account_in("wonderland");
        let transaction = TransactionBuilder::new(ChainId::from("snapshot-invalid"), authority)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(Vec::new())))
            .sign(keypair.private_key());
        let mut access_log = ivm::host::AccessLog::default();
        access_log
            .read_keys
            .insert("invalid state key with spaces".to_owned());

        let initial = {
            let mut world = crate::state::World::new();
            world
                .smart_contract_state_mut_for_testing()
                .insert("unrelated".parse().unwrap(), b"one".to_vec());
            State::new_for_testing(
                world,
                crate::kura::Kura::blank_kura_for_testing(),
                crate::query::store::LiveQueryStore::start_test(),
            )
        };
        let snapshot =
            DurableStateReadSnapshot::capture(&transaction, Some(&access_log), &initial.view())
                .expect("invalid VM read key still creates a fail-closed snapshot");

        let changed = {
            let mut world = crate::state::World::new();
            world
                .smart_contract_state_mut_for_testing()
                .insert("unrelated".parse().unwrap(), b"two".to_vec());
            State::new_for_testing(
                world,
                crate::kura::Kura::blank_kura_for_testing(),
                crate::query::store::LiveQueryStore::start_test(),
            )
        };
        assert!(
            !snapshot.is_current(&changed.view()),
            "an unrepresentable access key must conservatively observe the whole state map"
        );
    }

    #[test]
    fn durable_state_read_snapshot_fails_closed_for_partial_concrete_paths() {
        let (authority, keypair) = iroha_test_samples::gen_account_in("wonderland");
        let transaction = TransactionBuilder::new(ChainId::from("snapshot-partial"), authority)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(Vec::new())))
            .sign(keypair.private_key());
        let mut access_log = ivm::host::AccessLog::default();
        access_log
            .read_keys
            .extend(["alpha".to_owned(), "nested".to_owned()]);
        access_log.durable_read_paths.insert("alpha".to_owned());

        let make_state = |unrelated: &[u8]| {
            let mut world = crate::state::World::new();
            world
                .smart_contract_state_mut_for_testing()
                .insert("alpha".parse().unwrap(), b"stable".to_vec());
            world
                .smart_contract_state_mut_for_testing()
                .insert("unrelated".parse().unwrap(), unrelated.to_vec());
            State::new_for_testing(
                world,
                crate::kura::Kura::blank_kura_for_testing(),
                crate::query::store::LiveQueryStore::start_test(),
            )
        };
        let initial = make_state(b"one");
        let empty_incomplete_snapshot = DurableStateReadSnapshot::capture(
            &transaction,
            Some(&ivm::host::AccessLog::default()),
            &initial.view(),
        )
        .expect("an unattested empty log creates a fail-closed snapshot");
        assert!(
            !empty_incomplete_snapshot.is_current(&make_state(b"two").view()),
            "an empty custom-host log must not claim that no durable reads occurred"
        );

        let snapshot =
            DurableStateReadSnapshot::capture(&transaction, Some(&access_log), &initial.view())
                .expect("partial concrete log creates a fail-closed snapshot");
        assert!(snapshot.is_current(&initial.view()));
        assert!(
            !snapshot.is_current(&make_state(b"two").view()),
            "missing one logical read path must force a complete-map fingerprint"
        );
    }

    #[test]
    fn lifecycle_overlay_compare_and_consume_rejects_a_stale_second_apply_before_effects() {
        let (authority, _) = gen_account_in("wonderland");
        let domain = iroha_data_model::domain::Domain::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
        )
        .build(&authority);
        let account = build_wonderland_account(&authority);
        let world = crate::state::World::with([domain], [account], []);
        let state = State::new_for_testing(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            71,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let code_hash = Hash::new(b"lifecycle-overlay-code");
        let pending = code::PendingContractLifecycle::Hajimari {
            transition_id: Hash::new(b"lifecycle-overlay-transition"),
            code_hash,
        };
        let marker = code::contract_lifecycle_state_key(&contract_address);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        {
            let mut seed = block.transaction();
            seed.world
                .contract_instances
                .insert(contract_address.clone(), code_hash);
            code::set_pending_contract_lifecycle(&mut seed, &contract_address, Some(pending));
            seed.apply();
        }

        let mut first_writes = BTreeMap::new();
        first_writes.insert(marker.clone(), None);
        let authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "hajimari".to_owned(),
            None,
            &code::BoundContractIdentity {
                contract_address: contract_address.clone(),
                contract_alias: None,
                code_hash,
            },
        );
        let mut first = TxOverlay::from_ivm_execution(Vec::new(), 0, first_writes)
            .with_entrypoint_authorization(Some(authorization.clone()))
            .with_lifecycle_completion(&contract_address, Some(pending));
        first
            .durable_state_authorizations
            .insert(marker.clone(), Some(authorization.clone()));
        {
            let mut transaction = block.transaction();
            first
                .apply(&mut transaction, &authority)
                .expect("first lifecycle completion consumes the exact marker");
            transaction.apply();
        }
        assert!(block.world.smart_contract_state.get(&marker).is_none());

        let forbidden_effect: Name = "must-not-apply".parse().expect("state key");
        let mut stale_writes = BTreeMap::new();
        stale_writes.insert(marker.clone(), None);
        stale_writes.insert(forbidden_effect.clone(), Some(vec![0xA5]));
        let mut stale = TxOverlay::from_ivm_execution(Vec::new(), 0, stale_writes)
            .with_entrypoint_authorization(Some(authorization.clone()))
            .with_lifecycle_completion(&contract_address, Some(pending));
        stale
            .durable_state_authorizations
            .insert(marker, Some(authorization.clone()));
        stale
            .durable_state_authorizations
            .insert(forbidden_effect.clone(), Some(authorization));
        let mut transaction = block.transaction();
        let error = stale
            .apply(&mut transaction, &authority)
            .expect_err("a consumed lifecycle marker cannot be replayed");
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
        assert!(
            transaction
                .world
                .smart_contract_state
                .get(&forbidden_effect)
                .is_none(),
            "live lifecycle validation must run before every prepared effect"
        );
    }

    #[test]
    fn deployed_hajimari_call_builds_and_atomically_consumes_its_pending_transition() {
        use iroha_data_model::{
            permission::{Permission, Permissions},
            transaction::executable::ContractInvocation,
        };

        let (authority, keypair) = gen_account_in("wonderland");
        let domain = iroha_data_model::domain::Domain::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
        )
        .build(&authority);
        let account = build_wonderland_account(&authority);
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            72,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let metadata = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        };
        let interface = ivm::EmbeddedContractInterfaceV1 {
            seiyaku_name: "HajimariGuard".to_owned(),
            compiler_fingerprint: "iroha-core-lifecycle-overlay-test".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: "hajimari".to_owned(),
                kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Hajimari,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        let mut artifact = metadata.encode();
        artifact.extend_from_slice(&interface.encode_section());
        artifact.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let verified = ivm::verify_contract_artifact(&artifact).expect("valid hajimari artifact");
        let code_hash = verified.code_hash;
        let manifest = verified.manifest;

        let mut world = crate::state::World::with([domain], [account], []);
        world.contract_code.insert(code_hash, artifact);
        world.contract_manifests.insert(code_hash, manifest);
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let mut permissions = Permissions::new();
        assert!(permissions.insert(Permission::new(
            iroha_data_model::smart_contract::CONTRACT_HAJIMARI_PERMISSION_NAME.to_owned(),
            Json::new(()),
        )));
        world
            .account_permissions_mut_for_testing()
            .insert(authority.clone(), permissions);
        let state = State::new_with_chain(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            ChainId::from("hajimari-overlay"),
        );
        let pending = code::PendingContractLifecycle::Hajimari {
            transition_id: Hash::new(b"hajimari-overlay-transition"),
            code_hash,
        };
        let marker = code::contract_lifecycle_state_key(&contract_address);
        {
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
            let mut transaction = block.transaction();
            code::set_pending_contract_lifecycle(
                &mut transaction,
                &contract_address,
                Some(pending),
            );
            transaction.apply();
            block.commit().expect("commit pending hajimari transition");
        }

        let mut tx_metadata = Metadata::default();
        insert_gas_limit(&mut tx_metadata);
        let transaction =
            TransactionBuilder::new(ChainId::from("hajimari-overlay"), authority.clone())
                .with_metadata(tx_metadata)
                .with_executable(Executable::ContractCall(ContractInvocation {
                    contract_address: contract_address.clone(),
                    entrypoint: "hajimari".to_owned(),
                    arguments: None,
                }))
                .sign(keypair.private_key());

        let overlay = build_overlay_for_transaction(&transaction, &state.view())
            .expect("the exact pending hajimari call must prepare");
        assert_eq!(
            overlay
                .lifecycle_completion
                .as_ref()
                .map(|completion| completion.pending),
            Some(pending)
        );
        assert_eq!(overlay.durable_state_overlay.get(&marker), Some(&None));

        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
        let mut state_transaction = block.transaction();
        overlay
            .apply(&mut state_transaction, &authority)
            .expect("live hajimari transition remains valid at apply");
        state_transaction.apply();
        block.commit().expect("commit hajimari call");

        let view = state.view();
        assert!(view.world().smart_contract_state().get(&marker).is_none());
        assert!(
            code::validate_contract_lifecycle_call(
                view.world(),
                &contract_address,
                code_hash,
                iroha_data_model::smart_contract::manifest::EntryPointKind::Hajimari,
            )
            .is_err(),
            "hajimari/始まり must be single-use"
        );
    }

    fn minimal_contract_artifact_with_permission(
        abi_version: u8,
        permission: Option<&str>,
    ) -> (Vec<u8>, ContractManifest) {
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version,
        };
        let interface = ivm::EmbeddedContractInterfaceV1 {
            seiyaku_name: "TestContract".to_owned(),
            compiler_fingerprint: "iroha-core-overlay-test".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: "main".to_owned(),
                kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: permission.map(str::to_owned),
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: None,
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        let mut artifact = meta.encode();
        artifact.extend_from_slice(&interface.encode_section());
        artifact.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let verified =
            ivm::verify_contract_artifact(&artifact).expect("valid overlay test artifact");
        (artifact, verified.manifest)
    }

    fn minimal_contract_artifact(abi_version: u8) -> (Vec<u8>, ContractManifest) {
        minimal_contract_artifact_with_permission(abi_version, None)
    }

    #[test]
    fn self_describing_raw_contract_requires_explicit_entrypoint_in_every_parser() {
        let (artifact, _) = minimal_contract_artifact_with_permission(1, Some("CanInvoke"));
        let metadata = Metadata::default();
        let bytecode_err =
            parse_raw_contract_call_execution_context(&metadata, &artifact, TEST_GAS_LIMIT)
                .expect_err("raw self-describing artifact must not fall through to pc zero");
        assert!(
            matches!(
                &bytecode_err,
                OverlayBuildError::ContractCall(message)
                    if message.contains("require explicit contract_entrypoint")
            ),
            "unexpected bytecode parser error: {bytecode_err:?}"
        );

        let mut cache = IvmCache::new();
        let summary = cache
            .summarize_program(&artifact)
            .expect("prepare self-describing artifact");
        let prepared_err = parse_prepared_contract_call_execution_context(
            &metadata,
            summary.prepared_contract(),
            TEST_GAS_LIMIT,
        )
        .expect_err("prepared self-describing artifact must not fall through to pc zero");
        assert!(
            matches!(
                &prepared_err,
                OverlayBuildError::ContractCall(message)
                    if message.contains("require explicit contract_entrypoint")
            ),
            "unexpected prepared parser error: {prepared_err:?}"
        );
    }

    #[test]
    fn selective_rebuild_reuses_the_prepared_argument_plan_without_redecoding() {
        let artifact = ivm::KotodamaCompiler::new()
            .compile_source(
                r#"
seiyaku RebuildArguments {
  view fn inspect(value: i64) -> i64 { return value; }
}
"#,
            )
            .expect("compile parameterized rebuild fixture");
        let prepared = ivm::prepare_contract(Arc::from(artifact))
            .expect("prepare parameterized rebuild fixture");
        let schema = prepared
            .entrypoint_descriptor("inspect")
            .and_then(|entrypoint| entrypoint.argument_schema.as_ref())
            .expect("inspect argument schema");
        let arguments = ivm::encode_argument_record_from_json(
            schema,
            &Json::from(norito::json!({ "value": 7 })),
        )
        .expect("encode canonical rebuild arguments");
        let arguments =
            iroha_data_model::transaction::executable::ContractArgumentRecord::try_new(arguments)
                .expect("bounded rebuild argument record");
        let (authority, _) = gen_account_in("wonderland");
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            91,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive rebuild contract address");
        let invocation = ContractInvocation {
            contract_address,
            entrypoint: "inspect".to_owned(),
            arguments: Some(arguments),
        };

        ivm::reset_argument_record_decode_count();
        let first = parse_prepared_contract_invocation_execution_context(
            &invocation,
            &prepared,
            TEST_GAS_LIMIT,
            None,
        )
        .expect("prepare the initial argument plan");
        assert_eq!(ivm::argument_record_decode_count(), 1);
        let reused = first
            .argument_record
            .as_ref()
            .expect("prepared argument record");

        let rebuilt = parse_prepared_contract_invocation_execution_context(
            &invocation,
            &prepared,
            TEST_GAS_LIMIT,
            Some(reused),
        )
        .expect("reuse the argument plan for a selective rebuild");
        assert_eq!(
            ivm::argument_record_decode_count(),
            1,
            "the access-prepass/live-state rebuild boundary must not decode the signed payload twice"
        );
        assert_eq!(
            rebuilt
                .argument_record
                .as_ref()
                .expect("rebuilt argument record")
                .canonical_bytes(),
            reused.canonical_bytes()
        );
    }

    #[test]
    fn state_free_raw_builder_rejects_protected_entrypoint_before_argument_decode() {
        let program = ivm::KotodamaCompiler::new()
            .compile_source(
                r#"
seiyaku ProtectedStateFreeOverlay {
  kotoage fn write(value: i64) authorize("CanWriteStateFreeOverlay") {
    let _value = value;
  }
}
"#,
            )
            .expect("compile protected state-free overlay fixture");
        let (authority, keypair) = gen_account_in("wonderland");
        let chain_id = ChainId::from("protected-state-free-overlay");
        let mut metadata = Metadata::default();
        insert_gas_limit(&mut metadata);
        metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            Json::new("write"),
        );
        metadata.insert(
            "contract_payload".parse().expect("metadata key"),
            Json::from(norito::json!({ "value": 7 })),
        );
        let transaction = TransactionBuilder::new(chain_id, authority)
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
            .sign(keypair.private_key());

        ivm::reset_argument_record_decode_count();
        let error = build_overlay_for_transaction_with_accounts(&transaction, &[])
            .expect_err("state-free builder must reject protected entrypoints");
        assert!(
            matches!(
                &error,
                OverlayBuildError::ContractCall(message)
                    if message.contains("requires a full state view")
            ),
            "unexpected protected state-free overlay error: {error:?}"
        );
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "state-free permission rejection must precede canonical record decoding"
        );
    }

    #[test]
    fn state_free_raw_builder_rejects_permissionless_entrypoint_before_argument_decode() {
        let program = ivm::KotodamaCompiler::new()
            .compile_source(
                r#"
seiyaku PermissionlessStateFreeOverlay {
  kotoage fn write(value: i64) {
    let _value = value;
  }
}
"#,
            )
            .expect("compile permissionless state-free overlay fixture");
        let (authority, keypair) = gen_account_in("wonderland");
        let mut metadata = Metadata::default();
        insert_gas_limit(&mut metadata);
        metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            Json::new("write"),
        );
        metadata.insert(
            "contract_payload".parse().expect("metadata key"),
            Json::from(norito::json!({ "value": 7 })),
        );
        let transaction = TransactionBuilder::new(
            ChainId::from("permissionless-state-free-overlay"),
            authority,
        )
        .with_metadata(metadata)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
        .sign(keypair.private_key());

        ivm::reset_argument_record_decode_count();
        let error = build_overlay_for_transaction_with_accounts(&transaction, &[])
            .expect_err("state-free builder must reject every selected contract entrypoint");
        assert!(
            matches!(
                &error,
                OverlayBuildError::ContractCall(message)
                    if message.contains("full state view")
                        && message.contains("live contract binding")
            ),
            "unexpected permissionless state-free overlay error: {error:?}"
        );
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "state-free identity rejection must precede canonical record decoding"
        );
    }

    #[test]
    fn parameterized_contract_call_denial_precedes_argument_record_decode() {
        use iroha_data_model::transaction::executable::{
            ContractArgumentRecord, ContractInvocation,
        };

        const REQUIRED_PERMISSION: &str = "CanWriteParameterizedOverlay";
        let program = ivm::KotodamaCompiler::new()
            .compile_source(
                r#"
seiyaku ProtectedParameterizedOverlay {
  kotoage fn write(value: i64) authorize("CanWriteParameterizedOverlay") {
    let _value = value;
  }
}
"#,
            )
            .expect("compile protected parameterized overlay fixture");
        let verified =
            ivm::verify_contract_artifact(&program).expect("verify parameterized overlay fixture");
        let prepared = ivm::prepare_contract(Arc::from(program.clone()))
            .expect("prepare parameterized overlay fixture");
        let schema = prepared
            .entrypoint_descriptor("write")
            .and_then(|entrypoint| entrypoint.argument_schema.as_ref())
            .expect("write argument schema");
        let arguments = ivm::encode_argument_record_from_json(
            schema,
            &Json::from(norito::json!({ "value": 7 })),
        )
        .expect("encode canonical parameterized arguments");
        let arguments = ContractArgumentRecord::try_new(arguments)
            .expect("bounded parameterized argument record");

        let (authority, keypair) = gen_account_in("wonderland");
        let domain = iroha_data_model::domain::Domain::new(
            DomainId::try_new("wonderland", "universal").expect("valid domain"),
        )
        .build(&authority);
        let account = build_wonderland_account(&authority);
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            93,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive parameterized contract address");
        let code_hash = verified.code_hash;
        let mut world = crate::state::World::with([domain], [account], []);
        world.contract_code.insert(code_hash, program);
        world
            .contract_manifests
            .insert(code_hash, verified.manifest);
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let chain_id = ChainId::from("parameterized-authorization-overlay");
        let state = State::new_with_chain(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            chain_id.clone(),
        );

        let mut metadata = Metadata::default();
        insert_gas_limit(&mut metadata);
        let transaction = TransactionBuilder::new(chain_id, authority)
            .with_metadata(metadata)
            .with_executable(Executable::ContractCall(ContractInvocation {
                contract_address,
                entrypoint: "write".to_owned(),
                arguments: Some(arguments),
            }))
            .sign(keypair.private_key());

        ivm::reset_argument_record_decode_count();
        let error = build_overlay_for_transaction(&transaction, &state.view())
            .expect_err("missing permission must reject the parameterized call");
        assert!(
            matches!(
                &error,
                OverlayBuildError::ContractCall(message)
                    if message.contains(REQUIRED_PERMISSION) && message.contains("write")
            ),
            "unexpected parameterized authorization error: {error:?}"
        );
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "ordinary ContractCall permission denial must precede canonical record decoding"
        );
    }

    #[test]
    fn protected_contract_call_is_checked_before_vm_and_again_before_overlay_apply() {
        use iroha_data_model::{
            permission::{Permission, Permissions},
            transaction::executable::ContractInvocation,
        };

        const REQUIRED_PERMISSION: &str = "CanWriteGuardedState";
        let (authority, keypair) = gen_account_in("wonderland");
        let contract_address: ContractAddress =
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4vp9ggff82m7"
                .parse()
                .expect("valid contract address");
        let (artifact, manifest) =
            minimal_contract_artifact_with_permission(1, Some(REQUIRED_PERMISSION));
        let code_hash = manifest.code_hash.expect("verified code hash");
        let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
            "guarded",
            Some("wonderland"),
            "universal",
        )
        .expect("valid contract alias");

        let make_state = |authorized: bool| {
            let domain = iroha_data_model::domain::Domain::new(
                DomainId::try_new("wonderland", "universal").expect("valid domain"),
            )
            .build(&authority);
            let account = build_wonderland_account(&authority);
            let mut world = crate::state::World::with([domain], [account], []);
            world.contract_code.insert(code_hash, artifact.clone());
            world.contract_manifests.insert(code_hash, manifest.clone());
            world
                .contract_instances
                .insert(contract_address.clone(), code_hash);
            world
                .bind_contract_alias(&contract_address, contract_alias.clone(), None, None, 0)
                .expect("bind guarded contract alias");
            if authorized {
                let mut permissions = Permissions::new();
                assert!(permissions.insert(Permission::new(
                    REQUIRED_PERMISSION.to_owned(),
                    Json::new(()),
                )));
                world
                    .account_permissions_mut_for_testing()
                    .insert(authority.clone(), permissions);
            }
            State::new_with_chain(
                world,
                crate::kura::Kura::blank_kura_for_testing(),
                crate::query::store::LiveQueryStore::start_test(),
                ChainId::from("authorization-overlay"),
            )
        };

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        let transaction =
            TransactionBuilder::new(ChainId::from("authorization-overlay"), authority.clone())
                .with_metadata(metadata)
                .with_executable(Executable::ContractCall(ContractInvocation {
                    contract_address: contract_address.clone(),
                    entrypoint: "main".to_owned(),
                    arguments: None,
                }))
                .sign(keypair.private_key());

        let unauthorized_state = make_state(false);
        let denied = build_overlay_for_transaction(&transaction, &unauthorized_state.view())
            .expect_err("missing named permission must reject before the VM runs");
        assert!(
            matches!(
                denied,
                OverlayBuildError::ContractCall(message)
                    if message.contains(REQUIRED_PERMISSION) && message.contains("main")
            ),
            "unexpected authorization error: {denied:?}"
        );

        let authorized_state = make_state(true);
        let mut overlay = build_overlay_for_transaction(&transaction, &authorized_state.view())
            .expect("granted caller may prepare the protected call");
        let guarded_path: Name = "guarded/write".parse().expect("valid state path");
        let queued_key: Name = "guarded_queued".parse().expect("valid metadata key");
        overlay.instructions.push(
            iroha_data_model::isi::SetKeyValue::account(
                authority.clone(),
                queued_key.clone(),
                Json::new("queued"),
            )
            .into(),
        );
        overlay
            .execution_contexts
            .get_or_insert_with(Vec::new)
            .push(OverlayInstructionExecutionContext {
                authority: authority.clone(),
                contract_runtime_context: Some(crate::executor::ContractRuntimeExecutionContext {
                    contract_subject: contract_address.subject_id(),
                    contract_address: contract_address.clone(),
                    contract_alias: Some(contract_alias.clone()),
                    entrypoint: "main".to_owned(),
                }),
                entrypoint_authorization: overlay.entrypoint_authorization.clone(),
            });
        overlay
            .durable_state_overlay
            .insert(guarded_path.clone(), Some(vec![0xA5]));
        overlay.durable_state_authorizations.insert(
            guarded_path.clone(),
            overlay.entrypoint_authorization.clone(),
        );
        let proved_overlay = TxOverlay::from_ivm_proved_instructions(
            overlay.instructions.clone(),
            &authority,
            crate::executor::ContractRuntimeExecutionContext {
                contract_subject: contract_address.subject_id(),
                contract_address: contract_address.clone(),
                contract_alias: Some(contract_alias.clone()),
                entrypoint: "main".to_owned(),
            },
            overlay
                .entrypoint_authorization
                .clone()
                .expect("protected overlay authorization"),
        );
        let mut context_only_overlay = overlay.clone();
        context_only_overlay.entrypoint_authorization = None;

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut missing_durable_authorization = overlay.clone();
        missing_durable_authorization
            .durable_state_authorizations
            .remove(&guarded_path);
        let mut malformed_block = authorized_state.block(header.clone());
        let mut malformed_transaction = malformed_block.transaction();
        let malformed_error = missing_durable_authorization
            .apply(&mut malformed_transaction, &authority)
            .expect_err("durable values and authorization snapshots must have identical keys");
        assert!(matches!(
            malformed_error,
            ValidationFail::InternalError(message)
                if message.contains("structurally inconsistent")
        ));
        assert!(
            malformed_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_none()
                && malformed_transaction
                    .world
                    .smart_contract_state
                    .get(&guarded_path)
                    .is_none(),
            "structurally malformed durable authorization must reject before all effects"
        );
        drop(malformed_transaction);
        drop(malformed_block);

        let mut revoked_block = unauthorized_state.block(header.clone());
        let mut revoked_transaction = revoked_block.transaction();
        let error = overlay
            .apply(&mut revoked_transaction, &authority)
            .expect_err("a revoked permission must invalidate the prepared overlay");
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
        assert!(
            revoked_transaction
                .world
                .smart_contract_state
                .get(&guarded_path)
                .is_none(),
            "authorization must be checked before any durable write is applied"
        );
        assert!(
            revoked_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_none(),
            "authorization must be checked before any queued instruction is applied"
        );

        let mut revoked_proved_block = unauthorized_state.block(header.clone());
        let mut revoked_proved_transaction = revoked_proved_block.transaction();
        proved_overlay
            .apply(&mut revoked_proved_transaction, &authority)
            .expect_err("a revoked permission must invalidate a proved replay overlay");
        assert!(
            revoked_proved_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_none(),
            "proved replay authorization must run before any queued instruction"
        );

        let mut deactivated_proved_block = authorized_state.block(header.clone());
        let mut deactivated_proved_transaction = deactivated_proved_block.transaction();
        deactivated_proved_transaction
            .world
            .contract_instances
            .remove(contract_address.clone());
        proved_overlay
            .apply(&mut deactivated_proved_transaction, &authority)
            .expect_err("a deactivated contract must invalidate a proved replay overlay");
        assert!(
            deactivated_proved_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_none(),
            "proved replay binding validation must run before any queued instruction"
        );

        let mut authorized_proved_block = authorized_state.block(header.clone());
        let mut authorized_proved_transaction = authorized_proved_block.transaction();
        proved_overlay
            .apply(&mut authorized_proved_transaction, &authority)
            .expect("live permission and binding must allow the proved replay overlay");
        assert!(
            authorized_proved_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_some(),
            "granted proved replay authorization must allow queued instructions"
        );

        let mut revoked_context_block = unauthorized_state.block(header.clone());
        let mut revoked_context_transaction = revoked_context_block.transaction();
        context_only_overlay
            .apply(&mut revoked_context_transaction, &authority)
            .expect_err("queued contract effects must carry their own permission snapshot");
        assert!(
            revoked_context_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_none()
                && revoked_context_transaction
                    .world
                    .smart_contract_state
                    .get(&guarded_path)
                    .is_none(),
            "queued-context authorization must reject before every prepared effect"
        );

        let mut deactivated_block = authorized_state.block(header.clone());
        let mut deactivated_transaction = deactivated_block.transaction();
        deactivated_transaction
            .world
            .contract_instances
            .remove(contract_address.clone());
        let error = overlay
            .apply(&mut deactivated_transaction, &authority)
            .expect_err("deactivation must invalidate a prepared contract overlay");
        assert!(
            matches!(
                &error,
                ValidationFail::NotPermitted(message)
                    if message.contains("no longer active")
            ),
            "unexpected stale-binding error: {error:?}"
        );
        assert!(
            deactivated_transaction
                .world
                .smart_contract_state
                .get(&guarded_path)
                .is_none(),
            "binding must be checked before any durable write is applied"
        );
        assert!(
            deactivated_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_none(),
            "deactivation must be checked before any queued instruction is applied"
        );

        let mut rebound_block = authorized_state.block(header.clone());
        let mut rebound_transaction = rebound_block.transaction();
        rebound_transaction
            .world
            .contract_instances
            .insert(contract_address.clone(), Hash::new(b"changed-guarded-code"));
        let error = overlay
            .apply(&mut rebound_transaction, &authority)
            .expect_err("a changed code binding must invalidate a prepared contract overlay");
        assert!(
            matches!(
                &error,
                ValidationFail::NotPermitted(message)
                    if message.contains("changed code binding")
            ),
            "unexpected changed-code error: {error:?}"
        );
        assert!(
            rebound_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_none()
                && rebound_transaction
                    .world
                    .smart_contract_state
                    .get(&guarded_path)
                    .is_none(),
            "a changed code binding must apply zero prepared effects"
        );

        let mut realias_block = authorized_state.block(header.clone());
        let mut realias_transaction = realias_block.transaction();
        let replacement_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
            "guarded2",
            Some("wonderland"),
            "universal",
        )
        .expect("valid replacement alias");
        realias_transaction
            .world
            .bind_contract_alias(&contract_address, replacement_alias, None, None, 1)
            .expect("replace guarded contract alias");
        let error = overlay
            .apply(&mut realias_transaction, &authority)
            .expect_err("a changed alias binding must invalidate a prepared contract overlay");
        assert!(
            matches!(
                &error,
                ValidationFail::NotPermitted(message)
                    if message.contains("changed alias binding")
            ),
            "unexpected changed-alias error: {error:?}"
        );
        assert!(
            realias_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_none()
                && realias_transaction
                    .world
                    .smart_contract_state
                    .get(&guarded_path)
                    .is_none(),
            "a changed alias binding must apply zero prepared effects"
        );

        let mut revoking_overlay = overlay.clone();
        revoking_overlay.instructions = vec![
            Revoke::account_permission(
                Permission::new(REQUIRED_PERMISSION.to_owned(), Json::new(())),
                authority.clone(),
            )
            .into(),
        ];
        revoking_overlay.execution_contexts = Some(vec![
            overlay
                .execution_contexts
                .as_ref()
                .and_then(|contexts| contexts.first())
                .expect("guarded overlay instruction context")
                .clone(),
        ]);
        let mut revoking_block = authorized_state.block(header.clone());
        let mut revoking_transaction = revoking_block.transaction();
        let error = revoking_overlay
            .apply(&mut revoking_transaction, &authority)
            .expect_err("queued permission revocation must invalidate later durable writes");
        assert!(
            matches!(
                &error,
                ValidationFail::NotPermitted(message)
                    if message.contains(REQUIRED_PERMISSION)
            ),
            "unexpected post-instruction authorization error: {error:?}"
        );
        assert!(
            revoking_transaction
                .world
                .smart_contract_state
                .get(&guarded_path)
                .is_none(),
            "authorization must be rechecked after queued instructions and before durable writes"
        );

        let mut authorized_block = authorized_state.block(header);
        let mut authorized_transaction = authorized_block.transaction();
        overlay
            .apply(&mut authorized_transaction, &authority)
            .expect("live permission recheck should preserve the authorized path");
        assert_eq!(
            authorized_transaction
                .world
                .smart_contract_state
                .get(&guarded_path)
                .map(Vec::as_slice),
            Some([0xA5].as_slice())
        );
        assert!(
            authorized_transaction
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&queued_key)
                .is_some(),
            "the granted live authorization must allow queued instructions"
        );
    }

    #[test]
    fn nested_overlay_effects_retain_and_revalidate_the_complete_authorization_chain() {
        use iroha_data_model::permission::{Permission, Permissions};

        const ROOT_PERMISSION: &str = "CanInvokeRoot";
        const CHILD_PERMISSION: &str = "CanInvokeChild";
        let (authority, _) = gen_account_in("wonderland");
        let root_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            82,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive root contract address");
        let child_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            83,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive child contract address");
        let root_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
            "root",
            Some("wonderland"),
            "universal",
        )
        .expect("root alias");
        let child_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
            "child",
            Some("wonderland"),
            "universal",
        )
        .expect("child alias");
        let root_code_hash = Hash::new(b"root-authorization-code");
        let child_code_hash = Hash::new(b"child-authorization-code");
        let root_contract_subject = root_address.subject_id();

        let make_state = |grant_root: bool, grant_child: bool, child_active: bool| {
            let domain = iroha_data_model::domain::Domain::new(
                DomainId::try_new("wonderland", "universal").expect("domain id"),
            )
            .build(&authority);
            let account = build_wonderland_account(&authority);
            let root_contract_account = build_wonderland_account(&root_contract_subject);
            let mut world =
                crate::state::World::with([domain], [account, root_contract_account], []);
            world
                .contract_instances
                .insert(root_address.clone(), root_code_hash);
            if child_active {
                world
                    .contract_instances
                    .insert(child_address.clone(), child_code_hash);
            }
            world
                .bind_contract_alias(&root_address, root_alias.clone(), None, None, 0)
                .expect("bind root alias");
            if child_active {
                world
                    .bind_contract_alias(&child_address, child_alias.clone(), None, None, 0)
                    .expect("bind child alias");
            }
            let mut root_permissions = Permissions::new();
            if grant_root {
                assert!(
                    root_permissions
                        .insert(Permission::new(ROOT_PERMISSION.to_owned(), Json::new(()),))
                );
            }
            let mut child_permissions = Permissions::new();
            if grant_child {
                assert!(
                    child_permissions
                        .insert(Permission::new(CHILD_PERMISSION.to_owned(), Json::new(()),))
                );
                assert!(child_permissions.insert(Permission::new(
                    iroha_data_model::smart_contract::CONTRACT_HAJIMARI_PERMISSION_NAME.to_owned(),
                    Json::new(()),
                )));
            }
            world
                .account_permissions_mut_for_testing()
                .insert(authority.clone(), root_permissions);
            world
                .account_permissions_mut_for_testing()
                .insert(root_contract_subject.clone(), child_permissions);
            State::new_for_testing(
                world,
                crate::kura::Kura::blank_kura_for_testing(),
                crate::query::store::LiveQueryStore::start_test(),
            )
        };

        let root_authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "root".to_owned(),
            Some(ROOT_PERMISSION.to_owned()),
            &code::BoundContractIdentity {
                contract_address: root_address.clone(),
                contract_alias: Some(root_alias.clone()),
                code_hash: root_code_hash,
            },
        );
        let child_leaf = ContractEntrypointAuthorizationSnapshot::new(
            root_contract_subject.clone(),
            "child".to_owned(),
            Some(CHILD_PERMISSION.to_owned()),
            &code::BoundContractIdentity {
                contract_address: child_address.clone(),
                contract_alias: Some(child_alias.clone()),
                code_hash: child_code_hash,
            },
        );
        let child_authorization = child_leaf
            .clone()
            .with_parent(Some(root_authorization.clone()));
        let metadata_key: Name = "nested_authorization_applied"
            .parse()
            .expect("metadata key");
        let durable_path: Name = format!(
            "sc/{}/nested",
            hex::encode(Hash::new(child_address.to_string().as_bytes()).as_ref())
        )
        .parse()
        .expect("scoped durable path");
        let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            root_contract_subject.clone(),
            metadata_key.clone(),
            Json::new("applied"),
        )
        .into();
        let build_overlay = |effect_authorization: ContractEntrypointAuthorizationSnapshot| {
            TxOverlay::from_host_execution(
                vec![instruction.clone()],
                vec![OverlayInstructionExecutionContext {
                    authority: root_contract_subject.clone(),
                    contract_runtime_context: Some(
                        crate::executor::ContractRuntimeExecutionContext {
                            contract_subject: child_address.subject_id(),
                            contract_address: child_address.clone(),
                            contract_alias: Some(child_alias.clone()),
                            entrypoint: "child".to_owned(),
                        },
                    ),
                    entrypoint_authorization: Some(effect_authorization.clone()),
                }],
                0,
                Vec::new(),
                BTreeMap::from([(durable_path.clone(), Some(vec![0xC1]))]),
                BTreeMap::from([(durable_path.clone(), Some(effect_authorization))]),
            )
            .with_entrypoint_authorization(Some(root_authorization.clone()))
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);

        let authorized_state = make_state(true, true, true);
        let mut authorized_block = authorized_state.block(header.clone());
        let mut authorized_tx = authorized_block.transaction();
        build_overlay(child_authorization.clone())
            .apply(&mut authorized_tx, &authority)
            .expect("complete live authorization chain permits nested effects");
        assert!(
            authorized_tx
                .world
                .account(&root_contract_subject)
                .expect("root contract account")
                .metadata()
                .get(&metadata_key)
                .is_some()
        );
        assert_eq!(
            authorized_tx
                .world
                .smart_contract_state
                .get(&durable_path)
                .map(Vec::as_slice),
            Some([0xC1].as_slice())
        );
        drop(authorized_tx);
        drop(authorized_block);

        for (label, state) in [
            ("revoked root", make_state(false, true, true)),
            ("revoked child", make_state(true, false, true)),
            ("deactivated child", make_state(true, true, false)),
        ] {
            let mut block = state.block(header.clone());
            let mut tx = block.transaction();
            build_overlay(child_authorization.clone())
                .apply(&mut tx, &authority)
                .expect_err(label);
            assert!(
                tx.world
                    .account(&root_contract_subject)
                    .expect("root contract account")
                    .metadata()
                    .get(&metadata_key)
                    .is_none()
                    && tx.world.smart_contract_state.get(&durable_path).is_none(),
                "{label} must reject before every nested effect"
            );
        }

        let mut missing_parent_block = authorized_state.block(header);
        let mut missing_parent_tx = missing_parent_block.transaction();
        let error = build_overlay(child_leaf)
            .apply(&mut missing_parent_tx, &authority)
            .expect_err("detached child snapshot must not shed the root authorization");
        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message) if message.contains("root invocation chain")
        ));
        assert!(
            missing_parent_tx
                .world
                .account(&root_contract_subject)
                .expect("root contract account")
                .metadata()
                .get(&metadata_key)
                .is_none()
                && missing_parent_tx
                    .world
                    .smart_contract_state
                    .get(&durable_path)
                    .is_none()
        );

        let forged_child = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "child".to_owned(),
            Some(CHILD_PERMISSION.to_owned()),
            &code::BoundContractIdentity {
                contract_address: child_address.clone(),
                contract_alias: Some(child_alias.clone()),
                code_hash: child_code_hash,
            },
        )
        .with_parent(Some(root_authorization.clone()));
        let forged_state = make_state(true, true, true);
        let mut forged_block =
            forged_state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut forged_tx = forged_block.transaction();
        let forged_overlay = TxOverlay::from_host_execution(
            vec![instruction.clone()],
            vec![OverlayInstructionExecutionContext {
                authority: authority.clone(),
                contract_runtime_context: Some(crate::executor::ContractRuntimeExecutionContext {
                    contract_subject: child_address.subject_id(),
                    contract_address: child_address.clone(),
                    contract_alias: Some(child_alias.clone()),
                    entrypoint: "child".to_owned(),
                }),
                entrypoint_authorization: Some(forged_child),
            }],
            0,
            Vec::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .with_entrypoint_authorization(Some(root_authorization.clone()));
        let error = forged_overlay
            .apply(&mut forged_tx, &authority)
            .expect_err("nested caller must be the immediate parent contract subject");
        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message)
                if message.contains("immediate parent contract")
        ));

        let build_single_effect_overlay = |instruction: InstructionBox| {
            TxOverlay::from_host_execution(
                vec![instruction],
                vec![OverlayInstructionExecutionContext {
                    authority: root_contract_subject.clone(),
                    contract_runtime_context: Some(
                        crate::executor::ContractRuntimeExecutionContext {
                            contract_subject: child_address.subject_id(),
                            contract_address: child_address.clone(),
                            contract_alias: Some(child_alias.clone()),
                            entrypoint: "child".to_owned(),
                        },
                    ),
                    entrypoint_authorization: Some(child_authorization.clone()),
                }],
                0,
                Vec::new(),
                BTreeMap::new(),
                BTreeMap::new(),
            )
            .with_entrypoint_authorization(Some(root_authorization.clone()))
        };
        let self_revoking_state = make_state(true, true, true);
        let mut self_revoking_block =
            self_revoking_state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut self_revoking_tx = self_revoking_block.transaction();
        let error = build_single_effect_overlay(
            Revoke::account_permission(
                Permission::new(CHILD_PERMISSION.to_owned(), Json::new(())),
                root_contract_subject.clone(),
            )
            .into(),
        )
        .apply(&mut self_revoking_tx, &authority)
        .expect_err("the final nested effect must not revoke its own selected permission");
        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message) if message.contains(CHILD_PERMISSION)
        ));

        let self_deactivating_state = make_state(true, true, true);
        let mut self_deactivating_block = self_deactivating_state.block(BlockHeader::new(
            nonzero!(1_u64),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut self_deactivating_tx = self_deactivating_block.transaction();
        let error = build_single_effect_overlay(
            iroha_data_model::isi::smart_contract_code::DeactivateContractInstance {
                contract_address: child_address.clone(),
                reason: Some("nested authorization regression".to_owned()),
            }
            .into(),
        )
        .apply(&mut self_deactivating_tx, &authority)
        .expect_err("the final nested effect must not deactivate its selected contract");
        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message) if message.contains("no longer active")
        ));
    }

    #[test]
    fn permissionless_contract_overlay_still_carries_and_revalidates_live_binding() {
        use iroha_data_model::transaction::executable::ContractInvocation;

        let (authority, keypair) = gen_account_in("wonderland");
        let domain = iroha_data_model::domain::Domain::new(
            DomainId::try_new("wonderland", "universal").expect("valid domain"),
        )
        .build(&authority);
        let account = build_wonderland_account(&authority);
        let (artifact, manifest) = minimal_contract_artifact_with_permission(1, None);
        let code_hash = manifest.code_hash.expect("verified code hash");
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            81,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
            "open",
            Some("wonderland"),
            "universal",
        )
        .expect("valid contract alias");
        let mut world = crate::state::World::with([domain], [account], []);
        world.contract_code.insert(code_hash, artifact);
        world.contract_manifests.insert(code_hash, manifest);
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        world
            .bind_contract_alias(&contract_address, contract_alias.clone(), None, None, 0)
            .expect("bind contract alias");
        let state = State::new_with_chain(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            ChainId::from("permissionless-binding-overlay"),
        );
        let mut metadata = Metadata::default();
        insert_gas_limit(&mut metadata);
        let transaction = TransactionBuilder::new(
            ChainId::from("permissionless-binding-overlay"),
            authority.clone(),
        )
        .with_metadata(metadata)
        .with_executable(Executable::ContractCall(ContractInvocation {
            contract_address: contract_address.clone(),
            entrypoint: "main".to_owned(),
            arguments: None,
        }))
        .sign(keypair.private_key());
        let mut overlay = build_overlay_for_transaction(&transaction, &state.view())
            .expect("permissionless call prepares");
        let authorization = overlay
            .entrypoint_authorization
            .as_ref()
            .expect("every selected entrypoint carries an apply-time authorization snapshot");
        assert_eq!(authorization.entrypoint, "main");
        assert_eq!(authorization.permission, None);
        assert_eq!(&authorization.contract_address, &contract_address);
        assert_eq!(authorization.contract_alias.as_ref(), Some(&contract_alias));
        assert_eq!(authorization.code_hash, code_hash);

        let forbidden_effect: Name = "permissionless/forbidden"
            .parse()
            .expect("valid durable-state path");
        overlay
            .durable_state_overlay
            .insert(forbidden_effect.clone(), Some(vec![0x5A]));
        overlay.durable_state_authorizations.insert(
            forbidden_effect.clone(),
            overlay.entrypoint_authorization.clone(),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut state_block = state.block(header);
        let mut state_tx = state_block.transaction();
        state_tx
            .world
            .contract_instances
            .remove(contract_address.clone());

        overlay
            .apply(&mut state_tx, &authority)
            .expect_err("deactivation must invalidate even a permissionless prepared call");
        assert!(
            state_tx
                .world
                .smart_contract_state
                .get(&forbidden_effect)
                .is_none(),
            "deactivation must be checked before permissionless durable effects"
        );
    }

    #[test]
    fn overlay_appends_manifest_only_when_missing() {
        // Build state with a domain/account and optionally pre-seeded manifest
        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain = iroha_data_model::domain::Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        )
        .build(&authority_id);
        let account = build_wonderland_account(&authority_id);
        let world = crate::state::World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("chain"));

        // Create a minimal contract artifact and attach its verified manifest to tx metadata.
        let (prog, verified_manifest) = minimal_contract_artifact(1);
        let code_hash = verified_manifest
            .code_hash
            .expect("verified manifest code hash");
        let manifest = verified_manifest.signed(&kp);
        let mut md = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut md);
        md.insert(
            iroha_data_model::smart_contract::manifest::MANIFEST_METADATA_KEY
                .parse::<iroha_data_model::name::Name>()
                .unwrap(),
            Json::new(manifest.clone()),
        );
        let tx = iroha_data_model::transaction::TransactionBuilder::new(
            ChainId::from("chain"),
            authority_id.clone(),
        )
        .with_metadata(md)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog.clone())))
        .sign(kp.private_key());

        // Case 1: WSV doesn't have the manifest yet → overlay contains registration
        let overlay = build_overlay_for_transaction(&tx, &state.view()).expect("overlay");
        assert_eq!(
            overlay.instruction_count(),
            2,
            "expected bytecode and manifest registration ISIs"
        );

        // Seed bytecode and manifest into WSV.
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        stx.world.contract_code.insert(code_hash, prog.clone());
        stx.world
            .contract_manifests
            .insert(code_hash, manifest.clone());
        stx.apply();
        let _ = block.commit();

        // Case 2: WSV already has manifest → overlay contains no registration
        let overlay2 = build_overlay_for_transaction(&tx, &state.view()).expect("overlay2");
        assert!(overlay2.is_empty(), "no registration when manifest exists");
    }
}

/// Validate IVM header policy and return a structured admission error.
pub(crate) fn validate_header_policy(meta: &ivm::ProgramMetadata) -> Result<(), IvmAdmissionError> {
    // Version: accept 1.x
    if meta.version_major != 1 {
        return Err(IvmAdmissionError::UnsupportedVersion(
            iroha_data_model::executor::UnsupportedVersionInfo {
                major: meta.version_major,
                minor: meta.version_minor,
            },
        ));
    }
    // Mode feature bits
    let known = ivm::ivm_mode::ZK | ivm::ivm_mode::VECTOR | ivm::ivm_mode::HTM;
    if meta.mode & !known != 0 {
        return Err(IvmAdmissionError::UnsupportedFeatureBits(
            meta.mode & !known,
        ));
    }
    // ABI version: first release supports only v1.
    if meta.abi_version != 1 {
        return Err(IvmAdmissionError::UnsupportedAbiVersion(meta.abi_version));
    }
    if meta.max_cycles == 0 {
        return Err(IvmAdmissionError::MissingMaxCycles);
    }
    // Vector length sanity
    if meta.vector_length != 0 && meta.vector_length > ivm::VECTOR_LENGTH_MAX {
        return Err(IvmAdmissionError::VectorLengthTooLarge(
            iroha_data_model::executor::VectorLengthTooLargeInfo {
                vector_length: meta.vector_length,
                max_allowed: ivm::VECTOR_LENGTH_MAX,
            },
        ));
    }
    Ok(())
}

// (Chunking and limit enforcement driven by caller: see block.rs)

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId, Registrable,
        domain::DomainId,
        isi::smart_contract_code::RemoveSmartContractBytes,
        nexus::DataSpaceId,
        prelude::{IvmBytecode, TransactionBuilder},
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::gen_account_in;

    use super::*;
    use crate::state::State;

    fn build_wonderland_account(authority: &AccountId) -> iroha_data_model::account::Account {
        iroha_data_model::account::Account::new(authority.clone()).build(authority)
    }

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("overlay fixture key generation should succeed")
    }

    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }

    #[test]
    fn pre_execution_cycle_ceiling_accepts_exact_bound() {
        let meta = ivm::ProgramMetadata {
            max_cycles: 42,
            ..ivm::ProgramMetadata::default()
        };
        let bytecode = ivm::encoding::wide::encode_halt().to_le_bytes();

        enforce_pre_execution_policy(
            NonZeroU64::new(42).expect("test ceiling is non-zero"),
            &meta,
            0,
            &bytecode,
        )
        .expect("artifact at the configured ceiling should be admitted");
    }

    #[test]
    fn header_policy_rejects_zero_cycle_limit() {
        let meta = ivm::ProgramMetadata {
            max_cycles: 0,
            ..ivm::ProgramMetadata::default()
        };

        assert!(matches!(
            validate_header_policy(&meta),
            Err(IvmAdmissionError::MissingMaxCycles)
        ));
    }

    #[test]
    fn pre_execution_cycle_ceiling_rejects_over_bound() {
        let meta = ivm::ProgramMetadata {
            max_cycles: 43,
            ..ivm::ProgramMetadata::default()
        };
        let bytecode = ivm::encoding::wide::encode_halt().to_le_bytes();

        let error = enforce_pre_execution_policy(
            NonZeroU64::new(42).expect("test ceiling is non-zero"),
            &meta,
            0,
            &bytecode,
        )
        .expect_err("artifact above the configured ceiling must fail closed");
        assert!(matches!(
            error,
            OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::MaxCyclesExceedsUpperBound(info)
            ) if info.max_cycles == 43 && info.upper_bound == 42
        ));
    }

    fn mutate_open_verify_envelope_proof_box(
        mut proof: iroha_data_model::proof::ProofBox,
        mutate: impl FnOnce(&mut ZkOpenVerifyEnvelope),
    ) -> iroha_data_model::proof::ProofBox {
        let mut envelope: ZkOpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode OpenVerifyEnvelope fixture");
        mutate(&mut envelope);
        proof.bytes = norito::to_bytes(&envelope).expect("encode mutated OpenVerifyEnvelope");
        proof
    }

    #[test]
    fn empty_overlay_is_noop() {
        let ovl = TxOverlay::default();
        assert!(ovl.is_empty());
    }

    #[test]
    fn ivm_proved_axt_only_replay_is_not_dropped() {
        use iroha_data_model::block::BlockHeader;
        use nonzero_ext::nonzero;

        let (descriptor, binding) = ivm::axt::AxtDescriptor::builder()
            .dataspace(iroha_data_model::nexus::DataSpaceId::UNIVERSAL)
            .build_with_binding()
            .expect("AXT descriptor");
        let mut completed = ivm::axt::HostAxtState::new(descriptor, binding);
        completed
            .record_proof(
                iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
                Some(ivm::axt::ProofBlob {
                    payload: vec![1],
                    expiry_slot: None,
                }),
                None,
            )
            .expect("record AXT proof");
        completed.validate_commit().expect("completed AXT fixture");

        let state = crate::state::State::new_for_testing(
            crate::state::World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let overlay = tx_overlay_from_ivm_proved_replay(
            &state.view(),
            IvmProvedReplay {
                queued: Vec::new(),
                completed_axt: vec![completed],
                durable_state_overlay: BTreeMap::new(),
                events_commitment: Hash::new(b"events"),
                gas_used: 1,
                trace_hash: Hash::new(b"trace"),
            },
        );
        assert!(
            !overlay.is_empty(),
            "AXT-only proved replay must not collapse into an empty overlay"
        );
        assert!(overlay.has_durable_state_changes());

        let authority = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        overlay
            .apply(&mut state_tx, &authority)
            .expect("AXT-only proved replay applies without active fee policy");
        state_tx.apply();
        let envelopes = block.axt_envelopes();
        assert_eq!(envelopes.len(), 1, "verified AXT envelope must persist");
        assert_eq!(envelopes[0].binding.as_bytes(), &binding);
        assert_eq!(envelopes[0].commit_height, Some(1));
    }

    #[test]
    fn overlay_byte_size_cache_matches_norito_instruction_sum() {
        let instructions: Vec<InstructionBox> = vec![
            iroha_data_model::isi::Log::new(iroha_logger::Level::INFO, "cached-size-a".to_owned())
                .into(),
            iroha_data_model::isi::Log::new(iroha_logger::Level::INFO, "cached-size-b".to_owned())
                .into(),
        ];
        let expected = instructions
            .iter()
            .map(|instruction| NoritoEncode::encode(instruction).len())
            .sum();
        let overlay = TxOverlay::from_instructions(instructions);

        assert_eq!(overlay.byte_size(), expected);
        assert_eq!(overlay.byte_size.get(), Some(&expected));
        assert_eq!(overlay.byte_size(), expected);
    }

    #[test]
    fn overlay_rejects_ivm_without_gas_limit() {
        use iroha_data_model::{
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
        };

        let (program, _header_len, _meta) = sample_program();
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let world = crate::state::World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let state =
            crate::state::State::new_with_chain(world, kura, query_handle, ChainId::from("chain"));

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
            .sign(kp.private_key());

        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("overlay should require gas_limit metadata");
        assert!(matches!(
            err,
            OverlayBuildError::GasLimit(msg) if msg.contains("missing gas_limit")
        ));
    }

    #[test]
    fn overlay_rejects_ivm_proved_overlay_bind_standin_circuit() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId, VerifyingKeyRecord},
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);

        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> = Vec::new().into();

        // Compute the (code_hash, overlay_hash) public inputs expected by `IvmProved`.
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let overlay_hash = {
            let bytes = norito::to_bytes(&overlay).expect("encode overlay");
            Hash::new(&bytes)
        };
        let fixture = crate::zk::test_utils::halo2_ivm_overlay_bind_envelope(
            Hash::prehashed(*summary.code_hash.as_ref()),
            overlay_hash,
        );

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm_overlay_bind");
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture provides vk bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture provides vk hash");

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-overlay-bind",
            BackendTag::Halo2IpaPasta,
            "pasta",
            fixture.schema_hash,
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box);

        // Minimal authority/world setup.
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 101);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;
        // Unit tests should validate overlay plumbing, not benchmark ZK verifiers. Disable
        // time-based rejection so slow debug builds don't flap.
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let attachment =
            ProofAttachment::new_ref("halo2/ipa".into(), fixture.proof_box("halo2/ipa"), vk_id);
        let attachments = ProofAttachmentList(vec![attachment]);

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let events_commitment = Hash::new(b"events");
        let gas_policy_commitment = Hash::new(b"gas-policy");

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode,
                overlay: overlay.clone(),
                events_commitment,
                gas_policy_commitment,
            }))
            .with_attachments(attachments)
            .sign(kp.private_key());

        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("overlay-bind stand-in must be rejected");
        assert!(matches!(
            err,
            OverlayBuildError::ZkProof(msg) if msg.contains("binding-only stand-in circuit")
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn proved_overlay_builder_carries_complete_entrypoint_authorization() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            permission::{Permission, Permissions},
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId, VerifyingKeyRecord},
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        const REQUIRED_PERMISSION: &str = "CanBuildProvedOverlay";
        let compiler =
            ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
                force_zk: true,
                max_cycles: 10_000,
                mode: ivm::kotodama::compiler::CompilerMode::Test,
                ..ivm::kotodama::compiler::CompilerOptions::default()
            });
        let (program, manifest) = compiler
            .compile_source_with_manifest(
                r#"
seiyaku ProtectedProvedOverlay {
  kotoage fn open() -> i64 authorize("CanBuildProvedOverlay") {
    set_account_detail(
      authority(),
      name("proved_overlay_applied"),
      json!{ source: "top_level" }
    );
    return 0;
  }
}
"#,
            )
            .expect("compile protected ZK-mode contract");
        let bytecode = IvmBytecode::from_compiled(program.clone());

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let code_hash = Hash::prehashed(*summary.code_hash.as_ref());
        let vk_fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
            code_hash,
            Hash::new(b"vk-seed-overlay"),
            Hash::new(b"vk-events"),
            Hash::new(b"vk-gas-policy"),
        );

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm_execution");
        let vk_box = vk_fixture
            .vk_box("halo2/ipa")
            .expect("fixture provides vk bytes");
        let vk_commitment = vk_fixture
            .vk_hash("halo2/ipa")
            .expect("fixture provides vk hash");

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            BackendTag::Halo2IpaPasta,
            "pasta",
            vk_fixture.schema_hash,
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box);

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            92,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("derive proved-overlay contract address");
        let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
            "proved",
            Some("wonderland"),
            "universal",
        )
        .expect("valid proved-overlay contract alias");
        world
            .contract_code
            .insert(summary.code_hash, program.clone());
        world
            .contract_manifests
            .insert(summary.code_hash, manifest.signed(&kp));
        world
            .contract_instances
            .insert(contract_address.clone(), summary.code_hash);
        world
            .bind_contract_alias(&contract_address, contract_alias.clone(), None, None, 0)
            .expect("bind proved-overlay contract alias");
        let mut permissions = Permissions::new();
        assert!(permissions.insert(Permission::new(
            REQUIRED_PERMISSION.to_owned(),
            iroha_primitives::json::Json::new(()),
        )));
        world
            .account_permissions_mut_for_testing()
            .insert(authority.clone(), permissions);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;
        // Unit tests should validate overlay plumbing, not benchmark ZK verifiers. Disable
        // time-based rejection so slow debug builds don't flap.
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            iroha_primitives::json::Json::new("open"),
        );
        metadata.insert(
            "contract_address".parse().expect("metadata key"),
            iroha_primitives::json::Json::new(contract_address.to_string()),
        );
        metadata.insert(
            "contract_alias".parse().expect("metadata key"),
            iroha_primitives::json::Json::new(contract_alias.to_string()),
        );
        let derivation_tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::Ivm(bytecode.clone()))
            .sign(kp.private_key());
        let proved =
            derive_ivm_proved_payload_from_ivm_execution(&state.view(), &derivation_tx, &vk_record)
                .expect("derive non-empty proved overlay payload");
        assert_eq!(
            proved.overlay.len(),
            1,
            "top-level set_account_detail must produce one proved instruction"
        );
        let overlay_hash = {
            let bytes = norito::to_bytes(&proved.overlay).expect("encode derived proved overlay");
            Hash::new(&bytes)
        };
        let fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
            code_hash,
            overlay_hash,
            proved.events_commitment,
            proved.gas_policy_commitment,
        );
        assert_eq!(
            fixture
                .vk_hash("halo2/ipa")
                .expect("derived fixture provides vk hash"),
            vk_commitment,
            "the real payload envelope must use the registered execution verifying key"
        );

        let attachment =
            ProofAttachment::new_ref("halo2/ipa".into(), fixture.proof_box("halo2/ipa"), vk_id);
        let attachments = ProofAttachmentList(vec![attachment]);

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode: proved.bytecode,
                overlay: proved.overlay.clone(),
                events_commitment: proved.events_commitment,
                gas_policy_commitment: proved.gas_policy_commitment,
            }))
            .with_attachments(attachments)
            .sign(kp.private_key());

        let overlay_built =
            build_overlay_for_transaction(&tx, &state.view()).expect("proved execution overlay");
        let marker: Name = "proved_overlay_applied"
            .parse()
            .expect("valid proved-overlay marker");
        let built: Vec<InstructionBox> = overlay_built.instructions().cloned().collect();
        let expected_instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            authority.clone(),
            marker.clone(),
            Json::from(norito::json!({ "source": "top_level" })),
        )
        .into();
        assert_eq!(built, vec![expected_instruction]);
        assert_eq!(built.as_slice(), proved.overlay.as_ref());
        assert!(
            overlay_built.durable_state_overlay.is_empty(),
            "the canonical proved overlay must contain no StateMap writes"
        );
        let authorization = overlay_built
            .entrypoint_authorization
            .as_ref()
            .expect("proved overlay must retain selected entrypoint authorization");
        assert_eq!(authorization.entrypoint, "open");
        assert_eq!(
            authorization.permission.as_deref(),
            Some(REQUIRED_PERMISSION)
        );
        assert_eq!(&authorization.contract_address, &contract_address);
        assert_eq!(authorization.contract_alias.as_ref(), Some(&contract_alias));
        assert_eq!(authorization.code_hash, summary.code_hash);

        let execution_contexts = overlay_built
            .execution_contexts
            .as_deref()
            .expect("proved host write must retain its execution context");
        assert_eq!(execution_contexts.len(), 1);
        assert_eq!(
            execution_contexts[0].entrypoint_authorization.as_ref(),
            Some(authorization),
            "the queued host write must retain the complete root authorization snapshot"
        );
        let runtime_context = execution_contexts[0]
            .contract_runtime_context
            .as_ref()
            .expect("proved host write must retain its contract runtime context");
        assert_eq!(&runtime_context.contract_address, &contract_address);
        assert_eq!(
            runtime_context.contract_alias.as_ref(),
            Some(&contract_alias)
        );
        assert_eq!(runtime_context.entrypoint, "open");

        let replacement_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
            "proved2",
            Some("wonderland"),
            "universal",
        )
        .expect("valid replacement proved-overlay alias");
        for (mutation, expected_error) in [
            ("permission", REQUIRED_PERMISSION),
            ("instance", "no longer active"),
            ("code", "changed code binding"),
            ("alias", "changed alias binding"),
        ] {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut state_tx = block.transaction();
            match mutation {
                "permission" => {
                    state_tx.world.account_permissions.remove(authority.clone());
                }
                "instance" => {
                    state_tx
                        .world
                        .contract_instances
                        .remove(contract_address.clone());
                }
                "code" => {
                    state_tx.world.contract_instances.insert(
                        contract_address.clone(),
                        Hash::new(b"changed-proved-overlay-code"),
                    );
                }
                "alias" => {
                    state_tx
                        .world
                        .bind_contract_alias(
                            &contract_address,
                            replacement_alias.clone(),
                            None,
                            None,
                            1,
                        )
                        .expect("replace proved-overlay contract alias");
                }
                _ => unreachable!("complete mutation fixture"),
            }

            let error = overlay_built
                .apply(&mut state_tx, &authority)
                .expect_err("stale proved authorization must reject before its host write");
            assert!(
                matches!(
                    &error,
                    ValidationFail::NotPermitted(message)
                        if message.contains(expected_error)
                ),
                "unexpected {mutation} mutation error: {error:?}"
            );
            assert!(
                state_tx
                    .world
                    .account(&authority)
                    .expect("authority account")
                    .metadata()
                    .get(&marker)
                    .is_none(),
                "{mutation} mutation must reject before the proved metadata write"
            );
        }

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        overlay_built
            .apply(&mut state_tx, &authority)
            .expect("unchanged permission and binding must apply the proved host write");
        assert!(
            state_tx
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&marker)
                .is_some(),
            "granted proved authorization must apply the queued metadata write"
        );
    }

    #[test]
    fn overlay_rejects_ivm_proved_backend_tag_mismatches_before_verify() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId, VerifyingKeyRecord},
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);
        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> = Vec::new().into();

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let code_hash = Hash::prehashed(*summary.code_hash.as_ref());
        let overlay_hash = {
            let bytes = norito::to_bytes(&overlay).expect("encode overlay");
            Hash::new(&bytes)
        };
        let events_commitment = Hash::new(b"events");
        let gas_policy_commitment = Hash::new(b"gas-policy");
        let fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
            code_hash,
            overlay_hash,
            events_commitment,
            gas_policy_commitment,
        );

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm_execution");
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture provides vk bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture provides vk hash");

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            BackendTag::Halo2IpaPasta,
            "pasta",
            fixture.schema_hash,
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box);

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 102);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let chain_id = state.chain_id.clone();
        let build_tx = |vk_ref: VerifyingKeyId| {
            let attachment = ProofAttachment::new_ref(
                "halo2/ipa".into(),
                fixture.proof_box("halo2/ipa"),
                vk_ref,
            );
            TransactionBuilder::new(chain_id.clone(), authority.clone())
                .with_metadata(metadata.clone())
                .with_executable(Executable::IvmProved(IvmProved {
                    bytecode: bytecode.clone(),
                    overlay: overlay.clone(),
                    events_commitment,
                    gas_policy_commitment,
                }))
                .with_attachments(ProofAttachmentList(vec![attachment]))
                .sign(kp.private_key())
        };

        let wrong_ref_tx = build_tx(VerifyingKeyId::new("stark/fri", "ivm_execution"));
        let err = build_overlay_for_transaction(&wrong_ref_tx, &state.view())
            .expect_err("mismatched attachment verifier-key backend must reject before lookup");
        assert!(matches!(
            err,
            OverlayBuildError::ZkProof(msg)
                if msg.contains("proof attachment verifier-key backend mismatch")
        ));

        let mut bad_record = vk_record;
        bad_record.backend = BackendTag::Stark;
        state.world.verifying_keys.insert(vk_id.clone(), bad_record);
        let bad_record_tx = build_tx(vk_id.clone());
        let err = build_overlay_for_transaction(&bad_record_tx, &state.view())
            .expect_err("mismatched verifier record backend tag must reject before verify");
        assert!(matches!(
            err,
            OverlayBuildError::ZkProof(msg) if msg.contains("verifying key backend tag mismatch")
        ));
    }

    #[test]
    #[cfg(feature = "zk-stark")]
    fn overlay_accepts_stark_ivm_proved_binding_air_proof() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{
                ProofAttachment, ProofAttachmentList, VerifyingKeyBox, VerifyingKeyId,
                VerifyingKeyRecord,
            },
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);
        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> = Vec::new().into();

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let code_hash = Hash::prehashed(*summary.code_hash.as_ref());
        let overlay_hash = {
            let bytes = norito::to_bytes(&overlay).expect("encode overlay");
            Hash::new(&bytes)
        };

        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = "stark/fri/sha256-goldilocks:ivm-execution-v1";
        let vk_id = VerifyingKeyId::new(backend, "ivm_execution_stark");
        let vk_payload = crate::zk_stark::StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.to_owned(),
            n_log2: crate::zk_stark::ZK_ACE_STARK_FRI_PRODUCTION_MIN_N_LOG2,
            blowup_log2: crate::zk_stark::ZK_ACE_STARK_FRI_PRODUCTION_MIN_BLOWUP_LOG2,
            fold_arity: 2,
            queries: crate::zk_stark::ZK_ACE_STARK_FRI_PRODUCTION_MIN_QUERIES,
            merkle_arity: 2,
            hash_fn: crate::zk_stark::STARK_HASH_SHA256_V1,
        };
        let vk_box = VerifyingKeyBox::new(
            backend.into(),
            norito::to_bytes(&vk_payload).expect("encode STARK VK payload"),
        );
        let vk_commitment = crate::zk::hash_vk(&vk_box);
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            circuit_id,
            BackendTag::Stark,
            "goldilocks",
            crate::zk::ivm_execution_public_inputs_schema_hash(),
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box.clone());

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 103);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.stark.enabled = true;
        state.zk.halo2.enabled = false;
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let replay_tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode: bytecode.clone(),
                overlay: overlay.clone(),
                events_commitment: Hash::new(b"replay-events"),
                gas_policy_commitment: Hash::new(b"replay-gas-policy"),
            }))
            .sign(kp.private_key());
        let replay = replay_ivm_proved_overlay(
            &state.view(),
            &replay_tx,
            &summary,
            TEST_GAS_LIMIT,
            overlay_hash,
        )
        .expect("ivm proved replay");
        let events_commitment = replay.events_commitment;
        let gas_policy_commitment = expected_ivm_gas_policy_commitment(
            summary.code_hash,
            overlay_hash,
            &vk_record.circuit_id,
            vk_record.version,
            vk_record
                .gas_schedule_id
                .as_deref()
                .expect("gas schedule id must be set"),
            TEST_GAS_LIMIT,
            replay.gas_used,
            replay.trace_hash,
        );

        let proof_box = crate::zk::prove_stark_fri_ivm_execution_envelope(
            backend,
            circuit_id,
            &vk_box,
            code_hash,
            overlay_hash,
            events_commitment,
            gas_policy_commitment,
        )
        .expect("STARK binding AIR proof");
        let attachment = ProofAttachment::new_ref(backend.into(), proof_box, vk_id);
        let attachments = ProofAttachmentList(vec![attachment]);

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode,
                overlay: overlay.clone(),
                events_commitment,
                gas_policy_commitment,
            }))
            .with_attachments(attachments)
            .sign(kp.private_key());

        let overlay_built =
            build_overlay_for_transaction(&tx, &state.view()).expect("proved execution overlay");
        let built: Vec<InstructionBox> = overlay_built.instructions().cloned().collect();
        assert_eq!(built.as_slice(), overlay.as_ref());
    }

    #[test]
    #[cfg(feature = "zk-stark")]
    fn overlay_stark_prover_rejects_circuit_mismatch() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);
        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> = Vec::new().into();

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let code_hash = Hash::prehashed(*summary.code_hash.as_ref());
        let overlay_hash = {
            let bytes = norito::to_bytes(&overlay).expect("encode overlay");
            Hash::new(&bytes)
        };

        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = "stark/fri/sha256-goldilocks:ivm-execution-v1";
        let vk_id = VerifyingKeyId::new(backend, "ivm_execution_stark");
        let vk_payload = crate::zk_stark::StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.to_owned(),
            n_log2: crate::zk_stark::ZK_ACE_STARK_FRI_PRODUCTION_MIN_N_LOG2,
            blowup_log2: crate::zk_stark::ZK_ACE_STARK_FRI_PRODUCTION_MIN_BLOWUP_LOG2,
            fold_arity: 2,
            queries: crate::zk_stark::ZK_ACE_STARK_FRI_PRODUCTION_MIN_QUERIES,
            merkle_arity: 2,
            hash_fn: crate::zk_stark::STARK_HASH_SHA256_V1,
        };
        let vk_box = VerifyingKeyBox::new(
            backend.into(),
            norito::to_bytes(&vk_payload).expect("encode STARK VK payload"),
        );
        let vk_commitment = crate::zk::hash_vk(&vk_box);
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            circuit_id,
            BackendTag::Stark,
            "goldilocks",
            crate::zk::ivm_execution_public_inputs_schema_hash(),
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box.clone());

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 104);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.stark.enabled = true;
        state.zk.halo2.enabled = false;
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let replay_tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode: bytecode.clone(),
                overlay: overlay.clone(),
                events_commitment: Hash::new(b"replay-events"),
                gas_policy_commitment: Hash::new(b"replay-gas-policy"),
            }))
            .sign(kp.private_key());
        let replay = replay_ivm_proved_overlay(
            &state.view(),
            &replay_tx,
            &summary,
            TEST_GAS_LIMIT,
            overlay_hash,
        )
        .expect("ivm proved replay");
        let events_commitment = replay.events_commitment;
        let gas_policy_commitment = expected_ivm_gas_policy_commitment(
            summary.code_hash,
            overlay_hash,
            &vk_record.circuit_id,
            vk_record.version,
            vk_record
                .gas_schedule_id
                .as_deref()
                .expect("gas schedule id must be set"),
            TEST_GAS_LIMIT,
            replay.gas_used,
            replay.trace_hash,
        );

        let err = crate::zk::prove_stark_fri_ivm_execution_envelope(
            backend,
            "stark/fri/sha256-goldilocks:not-ivm-execution-v1",
            &vk_box,
            code_hash,
            overlay_hash,
            events_commitment,
            gas_policy_commitment,
        )
        .expect_err("mismatched STARK circuit must be rejected");
        assert!(
            err.contains("circuit_id mismatch")
                || err.contains("circuit id mismatch")
                || err.contains("STARK IVM execution proving requires"),
            "unexpected mismatch error: {err}"
        );
    }

    #[test]
    fn overlay_rejects_ivm_proved_when_commitments_mismatch() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId, VerifyingKeyRecord},
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);
        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> = Vec::new().into();

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let code_hash = Hash::prehashed(*summary.code_hash.as_ref());
        let overlay_hash = {
            let bytes = norito::to_bytes(&overlay).expect("encode overlay");
            Hash::new(&bytes)
        };
        let vk_fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
            code_hash,
            overlay_hash,
            Hash::new(b"vk-events"),
            Hash::new(b"vk-gas-policy"),
        );

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm_execution");
        let vk_box = vk_fixture
            .vk_box("halo2/ipa")
            .expect("fixture provides vk bytes");
        let vk_commitment = vk_fixture
            .vk_hash("halo2/ipa")
            .expect("fixture provides vk hash");

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            BackendTag::Halo2IpaPasta,
            "pasta",
            vk_fixture.schema_hash,
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box);

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 105);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;
        // Unit tests should validate overlay plumbing, not benchmark ZK verifiers. Disable
        // time-based rejection so slow debug builds don't flap.
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let replay_tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode: bytecode.clone(),
                overlay: overlay.clone(),
                events_commitment: Hash::new(b"replay-events"),
                gas_policy_commitment: Hash::new(b"replay-gas-policy"),
            }))
            .sign(kp.private_key());
        let replay = replay_ivm_proved_overlay(
            &state.view(),
            &replay_tx,
            &summary,
            TEST_GAS_LIMIT,
            overlay_hash,
        )
        .expect("ivm proved replay");
        let expected_events_commitment = replay.events_commitment;
        let expected_gas_policy_commitment = expected_ivm_gas_policy_commitment(
            summary.code_hash,
            overlay_hash,
            &vk_record.circuit_id,
            vk_record.version,
            vk_record
                .gas_schedule_id
                .as_deref()
                .expect("gas schedule id must be set"),
            TEST_GAS_LIMIT,
            replay.gas_used,
            replay.trace_hash,
        );

        // Let oversized public-input metadata reach the shared envelope validator instead of
        // the existing outer proof-size guard.
        state.zk.halo2.max_envelope_bytes = usize::MAX;
        state.zk.halo2.max_proof_bytes = usize::MAX;

        let build_tx =
            |events_commitment: Hash,
             gas_policy_commitment: Hash,
             mutate_envelope: Option<fn(&mut ZkOpenVerifyEnvelope)>| {
                let fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
                    code_hash,
                    overlay_hash,
                    events_commitment,
                    gas_policy_commitment,
                );
                let mut proof_box = fixture.proof_box("halo2/ipa");
                if let Some(mutate) = mutate_envelope {
                    proof_box = mutate_open_verify_envelope_proof_box(proof_box, mutate);
                }
                let attachment =
                    ProofAttachment::new_ref("halo2/ipa".into(), proof_box, vk_id.clone());
                let attachments = ProofAttachmentList(vec![attachment]);
                TransactionBuilder::new(state.chain_id.clone(), authority.clone())
                    .with_metadata(metadata.clone())
                    .with_executable(Executable::IvmProved(IvmProved {
                        bytecode: bytecode.clone(),
                        overlay: overlay.clone(),
                        events_commitment,
                        gas_policy_commitment,
                    }))
                    .with_attachments(attachments)
                    .sign(kp.private_key())
            };

        let invalid_envelope_cases: [(&str, fn(&mut ZkOpenVerifyEnvelope), &str); 7] = [
            (
                "unsupported backend",
                |env| env.backend = BackendTag::Unsupported,
                "backend is unsupported",
            ),
            (
                "empty circuit id",
                |env| env.circuit_id.clear(),
                "circuit id is empty",
            ),
            (
                "zero verifier-key hash",
                |env| env.vk_hash = [0u8; 32],
                "verifier-key hash is zero",
            ),
            (
                "empty public inputs",
                |env| env.public_inputs.clear(),
                "public inputs are empty",
            ),
            (
                "oversized public inputs",
                |env| {
                    env.public_inputs = vec![
                        0xA5;
                        iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_PUBLIC_INPUT_BYTES
                            + 1
                    ];
                },
                "public inputs length",
            ),
            (
                "empty proof bytes",
                |env| env.proof_bytes.clear(),
                "proof bytes are empty",
            ),
            (
                "auxiliary bytes",
                |env| env.aux = b"ignored-hint".to_vec(),
                "auxiliary bytes must be empty",
            ),
        ];
        for (label, mutate, expected_msg) in invalid_envelope_cases {
            let tx = build_tx(
                expected_events_commitment,
                expected_gas_policy_commitment,
                Some(mutate),
            );
            let err = match build_overlay_for_transaction(&tx, &state.view()) {
                Ok(_) => panic!("{label} must be rejected"),
                Err(err) => err,
            };
            assert!(
                matches!(
                    &err,
                    OverlayBuildError::ZkProof(msg)
                        if msg.contains("invalid OpenVerifyEnvelope")
                            && msg.contains(expected_msg)
                ),
                "unexpected {label} error: {err:?}"
            );
        }

        let bad_events_tx = build_tx(
            Hash::new(b"bad-events"),
            expected_gas_policy_commitment,
            None,
        );
        let err = build_overlay_for_transaction(&bad_events_tx, &state.view())
            .expect_err("events commitment mismatch must be rejected");
        assert!(
            matches!(
                &err,
                OverlayBuildError::ZkProof(msg) if msg.contains("events commitment mismatch")
            ),
            "unexpected error: {err:?}"
        );

        let bad_gas_policy_tx = build_tx(
            expected_events_commitment,
            Hash::new(b"bad-gas-policy"),
            None,
        );
        let err = build_overlay_for_transaction(&bad_gas_policy_tx, &state.view())
            .expect_err("gas policy commitment mismatch must be rejected");
        assert!(
            matches!(
                &err,
                OverlayBuildError::ZkProof(msg) if msg.contains("gas policy commitment mismatch")
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn overlay_rejects_ivm_proved_when_disabled_in_pipeline() {
        use iroha_data_model::{
            domain::Domain,
            isi::Log,
            level::Level,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            transaction::{Executable, IvmProved},
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);

        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> =
            vec![InstructionBox::from(Log {
                level: Level::INFO,
                msg: "hello".to_owned(),
            })]
            .into();

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 106);
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let events_commitment = Hash::new(b"events");
        let gas_policy_commitment = Hash::new(b"gas-policy");

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode,
                overlay,
                events_commitment,
                gas_policy_commitment,
            }))
            .sign(kp.private_key());

        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("should reject proved execution when disabled");
        assert!(matches!(
            err,
            OverlayBuildError::ZkProof(msg) if msg.contains("disabled")
        ));
    }

    #[test]
    fn overlay_rejects_ivm_proved_when_allowlist_empty() {
        use iroha_data_model::{
            domain::Domain,
            isi::Log,
            level::Level,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            transaction::{Executable, IvmProved},
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);

        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> =
            vec![InstructionBox::from(Log {
                level: Level::INFO,
                msg: "hello".to_owned(),
            })]
            .into();

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 107);
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits.clear();
        state.zk.halo2.enabled = true;

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let events_commitment = Hash::new(b"events");
        let gas_policy_commitment = Hash::new(b"gas-policy");

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode,
                overlay,
                events_commitment,
                gas_policy_commitment,
            }))
            .sign(kp.private_key());

        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("should reject proved execution when allowlist is empty");
        assert!(matches!(
            err,
            OverlayBuildError::ZkProof(msg) if msg.contains("allowed_circuits")
        ));
    }

    #[test]
    fn overlay_rejects_ivm_proved_when_overlay_hash_mismatches() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            isi::Log,
            level::Level,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId, VerifyingKeyRecord},
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);

        let overlay_ok: iroha_primitives::const_vec::ConstVec<InstructionBox> = Vec::new().into();
        let overlay_bad: iroha_primitives::const_vec::ConstVec<InstructionBox> =
            vec![InstructionBox::from(Log {
                level: Level::INFO,
                msg: "tampered".to_owned(),
            })]
            .into();

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let code_hash = Hash::prehashed(*summary.code_hash.as_ref());
        let overlay_ok_hash = {
            let bytes = norito::to_bytes(&overlay_ok).expect("encode overlay");
            Hash::new(&bytes)
        };
        let overlay_bad_hash = {
            let bytes = norito::to_bytes(&overlay_bad).expect("encode overlay");
            Hash::new(&bytes)
        };
        let events_commitment = Hash::new(b"events");
        let gas_policy_commitment = Hash::new(b"gas-policy");
        let fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
            code_hash,
            overlay_ok_hash,
            events_commitment,
            gas_policy_commitment,
        );

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm_execution");
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture provides vk bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture provides vk hash");

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            BackendTag::Halo2IpaPasta,
            "pasta",
            fixture.schema_hash,
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box);

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 108);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;
        // Unit tests should validate overlay plumbing, not benchmark ZK verifiers. Disable
        // time-based rejection so slow debug builds don't flap.
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let attachment =
            ProofAttachment::new_ref("halo2/ipa".into(), fixture.proof_box("halo2/ipa"), vk_id);
        let attachments = ProofAttachmentList(vec![attachment]);

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let _ = overlay_bad_hash; // mismatch is exercised via `overlay_hash` in proof public inputs.

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode,
                overlay: overlay_bad,
                events_commitment,
                gas_policy_commitment,
            }))
            .with_attachments(attachments)
            .sign(kp.private_key());

        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("overlay hash mismatch must be rejected");
        assert!(matches!(
            err,
            OverlayBuildError::ZkProof(msg) if msg.contains("proof public inputs do not match")
        ));
    }

    #[test]
    fn overlay_rejects_ivm_proved_when_vk_schema_hash_mismatches() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId, VerifyingKeyRecord},
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);
        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> = Vec::new().into();

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let code_hash = Hash::prehashed(*summary.code_hash.as_ref());
        let overlay_hash = {
            let bytes = norito::to_bytes(&overlay).expect("encode overlay");
            Hash::new(&bytes)
        };
        let events_commitment = Hash::new(b"events");
        let gas_policy_commitment = Hash::new(b"gas-policy");
        let fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
            code_hash,
            overlay_hash,
            events_commitment,
            gas_policy_commitment,
        );

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm_execution");
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture provides vk bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture provides vk hash");

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            BackendTag::Halo2IpaPasta,
            "pasta",
            *Hash::new(b"wrong-schema").as_ref(),
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box);

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 109);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;
        // Unit tests should validate overlay plumbing, not benchmark ZK verifiers. Disable
        // time-based rejection so slow debug builds don't flap.
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let attachment =
            ProofAttachment::new_ref("halo2/ipa".into(), fixture.proof_box("halo2/ipa"), vk_id);
        let attachments = ProofAttachmentList(vec![attachment]);

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode,
                overlay,
                events_commitment,
                gas_policy_commitment,
            }))
            .with_attachments(attachments)
            .sign(kp.private_key());

        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("schema hash mismatch must be rejected");
        assert!(matches!(
            err,
            OverlayBuildError::ZkProof(msg) if msg.contains("verifying key schema hash mismatch")
        ));
    }

    #[test]
    fn overlay_rejects_ivm_proved_when_replay_overlay_mismatches() {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            domain::Domain,
            isi::Log,
            level::Level,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId, VerifyingKeyRecord},
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);

        let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> =
            vec![InstructionBox::from(Log {
                level: Level::INFO,
                msg: "tampered".to_owned(),
            })]
            .into();

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(bytecode.as_ref())
            .expect("summarize IVM program");
        let code_hash = Hash::prehashed(*summary.code_hash.as_ref());
        let overlay_hash = {
            let bytes = norito::to_bytes(&overlay).expect("encode overlay");
            Hash::new(&bytes)
        };
        let vk_fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
            code_hash,
            overlay_hash,
            Hash::new(b"vk-events"),
            Hash::new(b"vk-gas-policy"),
        );

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm_execution");
        let vk_box = vk_fixture
            .vk_box("halo2/ipa")
            .expect("fixture provides vk bytes");
        let vk_commitment = vk_fixture
            .vk_hash("halo2/ipa")
            .expect("fixture provides vk hash");

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            BackendTag::Halo2IpaPasta,
            "pasta",
            vk_fixture.schema_hash,
            vk_commitment,
        );
        vk_record.status = ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());
        vk_record.key = Some(vk_box);

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 110);
        world
            .verifying_keys
            .insert(vk_id.clone(), vk_record.clone());
        world.verifying_keys_by_circuit.insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;
        // Unit tests should validate overlay plumbing, not benchmark ZK verifiers. Disable
        // time-based rejection so slow debug builds don't flap.
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state.pipeline.ivm_proved.enabled = true;
        state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);
        let replay_tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode: bytecode.clone(),
                overlay: overlay.clone(),
                events_commitment: Hash::new(b"replay-events"),
                gas_policy_commitment: Hash::new(b"replay-gas-policy"),
            }))
            .sign(kp.private_key());
        let replay = replay_ivm_proved_overlay(
            &state.view(),
            &replay_tx,
            &summary,
            TEST_GAS_LIMIT,
            overlay_hash,
        )
        .expect("ivm proved replay");
        let events_commitment = replay.events_commitment;
        let gas_policy_commitment = expected_ivm_gas_policy_commitment(
            summary.code_hash,
            overlay_hash,
            &vk_record.circuit_id,
            vk_record.version,
            vk_record
                .gas_schedule_id
                .as_deref()
                .expect("gas schedule id must be set"),
            TEST_GAS_LIMIT,
            replay.gas_used,
            replay.trace_hash,
        );

        let fixture = crate::zk::test_utils::halo2_ivm_execution_envelope(
            code_hash,
            overlay_hash,
            events_commitment,
            gas_policy_commitment,
        );
        let attachment =
            ProofAttachment::new_ref("halo2/ipa".into(), fixture.proof_box("halo2/ipa"), vk_id);
        let attachments = ProofAttachmentList(vec![attachment]);

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode,
                overlay,
                events_commitment,
                gas_policy_commitment,
            }))
            .with_attachments(attachments)
            .sign(kp.private_key());

        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("overlay replay mismatch must be rejected");
        assert!(
            matches!(
                &err,
                OverlayBuildError::ZkProof(msg) if msg.contains("deterministic IVM replay")
            ),
            "unexpected error: {err:?}"
        );

        state.pipeline.ivm_proved.skip_replay = true;
        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("ABI V1 must reject replay skipping even for a full execution circuit");
        assert!(
            matches!(
                &err,
                OverlayBuildError::ZkProof(message)
                    if message.contains("deterministic replay is mandatory")
            ),
            "unexpected skip-replay error: {err:?}"
        );
    }

    #[test]
    fn derive_ivm_proved_payload_matches_replay_commitments() {
        use iroha_data_model::{
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::VerifyingKeyRecord,
            transaction::{Executable, IvmProved},
            zk::BackendTag,
        };
        use std::sync::Arc;

        let (program, _header_len, _meta) = sample_program_zk_mode();
        let bytecode = IvmBytecode::from_compiled(program);

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = bind_sample_raw_contract(&mut world, &authority, &bytecode, 111);

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        bind_sample_raw_metadata(&mut metadata, &contract_address);

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::Ivm(bytecode.clone()))
            .sign(kp.private_key());

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            BackendTag::Halo2IpaPasta,
            "pasta",
            crate::zk::ivm_execution_public_inputs_schema_hash(),
            [0u8; 32],
        );
        vk_record.gas_schedule_id = Some("sched_0".to_owned());

        let proved = derive_ivm_proved_payload_from_ivm_execution(&state.view(), &tx, &vk_record)
            .expect("derive proved payload");

        let tx_proved = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(IvmProved {
                bytecode: proved.bytecode.clone(),
                overlay: proved.overlay.clone(),
                events_commitment: proved.events_commitment,
                gas_policy_commitment: proved.gas_policy_commitment,
            }))
            .sign(kp.private_key());

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(proved.bytecode.as_ref())
            .expect("summarize IVM program");

        let overlay_hash = {
            let bytes = norito::to_bytes(&proved.overlay).expect("encode overlay");
            Hash::new(&bytes)
        };

        let replay = replay_ivm_proved_overlay(
            &state.view(),
            &tx_proved,
            &summary,
            TEST_GAS_LIMIT,
            overlay_hash,
        )
        .expect("replay proved overlay");

        assert_eq!(
            proved.events_commitment, replay.events_commitment,
            "events commitment should match deterministic replay"
        );

        let expected_gas_policy_commitment = expected_ivm_gas_policy_commitment(
            summary.code_hash,
            overlay_hash,
            &vk_record.circuit_id,
            vk_record.version,
            vk_record
                .gas_schedule_id
                .as_deref()
                .expect("gas schedule id"),
            TEST_GAS_LIMIT,
            replay.gas_used,
            replay.trace_hash,
        );
        assert_eq!(
            proved.gas_policy_commitment, expected_gas_policy_commitment,
            "gas policy commitment should match deterministic replay"
        );
    }

    #[test]
    fn derive_ivm_proved_payload_dispatches_contract_entrypoint_metadata() {
        use iroha_data_model::{
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            proof::VerifyingKeyRecord,
            transaction::Executable,
            zk::BackendTag,
        };
        use std::sync::Arc;

        let compiler =
            ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
                force_zk: true,
                max_cycles: 10_000,
                mode: ivm::kotodama::compiler::CompilerMode::Test,
                ..ivm::kotodama::compiler::CompilerOptions::default()
            });
        let (program, manifest) = compiler
            .compile_source_with_manifest(
                r#"
seiyaku DeriveDispatch {
  kotoage fn main() -> i64 authorize("DeriveDispatch") {
    assert(false);
    return 0;
  }

  kotoage fn open(amount: i64) -> i64 authorize("DeriveDispatch") {
    assert(amount == 7);
    return 0;
  }

  kotoage fn restricted() -> int permission(AssetOps) {
    return 0;
  }
}
"#,
            )
            .expect("compile ZK-mode contract artifact");
        let bytecode = IvmBytecode::from_compiled(program.clone());

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_wonderland_account(&authority);
        let mut world = crate::state::World::with([domain], [account], []);
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            94,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("derive proved-payload contract address");
        let code_hash = manifest.code_hash.expect("verified code hash");
        world.contract_code.insert(code_hash, program);
        world.contract_manifests.insert(code_hash, manifest);
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let mut permissions = iroha_data_model::permission::Permissions::new();
        assert!(
            permissions.insert(iroha_data_model::permission::Permission::new(
                "DeriveDispatch".to_owned(),
                iroha_primitives::json::Json::new(()),
            )),
            "fixture permission should be newly granted"
        );
        world
            .account_permissions_mut_for_testing()
            .insert(authority.clone(), permissions);

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        state.zk.halo2.enabled = true;

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            iroha_primitives::json::Json::new("open"),
        );
        metadata.insert(
            "contract_payload".parse().expect("metadata key"),
            iroha_primitives::json::Json::new(norito::json!({ "amount": 7 })),
        );
        metadata.insert(
            "contract_address".parse().expect("metadata key"),
            iroha_primitives::json::Json::new(contract_address.to_string()),
        );

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(bytecode.clone()))
            .sign(kp.private_key());

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            BackendTag::Halo2IpaPasta,
            "pasta",
            crate::zk::ivm_execution_public_inputs_schema_hash(),
            [0u8; 32],
        );
        vk_record.gas_schedule_id = Some("sched_0".to_owned());

        let proved = derive_ivm_proved_payload_from_ivm_execution(&state.view(), &tx, &vk_record)
            .expect("derive proved payload using contract entrypoint metadata");

        assert!(
            proved.overlay.is_empty(),
            "test entrypoint should execute without queuing instructions"
        );

        let mut restricted_metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut restricted_metadata);
        restricted_metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            iroha_primitives::json::Json::new("restricted"),
        );
        let restricted_tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(restricted_metadata)
            .with_executable(Executable::Ivm(bytecode))
            .sign(kp.private_key());
        let err =
            derive_ivm_proved_payload_from_ivm_execution(&state.view(), &restricted_tx, &vk_record)
                .expect_err("proved derivation must enforce protected entrypoint permissions");
        assert!(
            matches!(
                &err,
                OverlayBuildError::ContractCall(message)
                    if message.contains("requires permission `AssetOps`")
            ),
            "unexpected permission error: {err:?}"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn ivm_proved_replay_rejects_nested_authorization_context() {
        use iroha_data_model::{
            domain::Domain,
            nexus::DataSpaceId,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            transaction::{Executable, IvmProved},
        };
        use std::sync::Arc;

        let compiler =
            ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
                force_zk: true,
                max_cycles: 100_000,
                mode: ivm::kotodama::compiler::CompilerMode::Test,
                ..ivm::kotodama::compiler::CompilerOptions::default()
            });
        let outer_source = r#"
seiyaku ReplayOuter {
  state bytes CalleeContract;

  kotoage fn bind(callee_contract: bytes) {
    CalleeContract = callee_contract;
  }

  kotoage fn run(value: int) -> int permission(AssetOps) {
    let payload = json_object();
    let payload = json_set_int(payload, name("value"), value);
    return decode_int(call_contract(CalleeContract, "write", payload));
  }
}
"#;
        let (outer_code, _) = compiler
            .compile_source_with_manifest(outer_source)
            .expect("compile outer replay contract");
        let bind_compiler =
            ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
                force_zk: false,
                max_cycles: 100_000,
                mode: ivm::kotodama::compiler::CompilerMode::Test,
                ..ivm::kotodama::compiler::CompilerOptions::default()
            });
        let (outer_bind_code, _) = bind_compiler
            .compile_source_with_manifest(outer_source)
            .expect("compile non-ZK outer state initializer");
        let (callee_code, _) = compiler
            .compile_source_with_manifest(
                r#"
seiyaku ReplayCallee {
  kotoage fn write(value: int) -> int permission(AssetOps) {
    set_account_detail(
      authority(),
      name("proof_replay"),
      json!{ source: "nested" }
    );
    return value;
  }
}
"#,
            )
            .expect("compile nested replay contract");

        let outer_verified =
            ivm::verify_contract_artifact(&outer_code).expect("verify outer contract artifact");
        let outer_bind_verified = ivm::verify_contract_artifact(&outer_bind_code)
            .expect("verify non-ZK outer state initializer");
        let callee_verified =
            ivm::verify_contract_artifact(&callee_code).expect("verify callee contract artifact");
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let outer_address = ContractAddress::derive(0, &authority, 1, DataSpaceId::UNIVERSAL)
            .expect("derive outer contract address");
        let callee_address = ContractAddress::derive(0, &authority, 2, DataSpaceId::UNIVERSAL)
            .expect("derive callee contract address");

        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let accounts = [
            build_wonderland_account(&authority),
            build_wonderland_account(&outer_address.subject_id()),
            build_wonderland_account(&callee_address.subject_id()),
        ];
        let mut world = crate::state::World::with([domain], accounts, []);
        let asset_ops = iroha_data_model::permission::Permission::new(
            "AssetOps".to_owned(),
            iroha_primitives::json::Json::new(()),
        );
        world.account_permissions.insert(
            authority.clone(),
            std::collections::BTreeSet::from([asset_ops.clone()]),
        );
        world.account_permissions.insert(
            outer_address.subject_id(),
            std::collections::BTreeSet::from([asset_ops]),
        );
        world.contract_code.insert(
            outer_bind_verified.code_hash,
            outer_bind_code.as_slice().to_vec(),
        );
        world.contract_manifests.insert(
            outer_bind_verified.code_hash,
            outer_bind_verified.manifest.signed(&kp),
        );
        world
            .contract_instances
            .insert(outer_address.clone(), outer_bind_verified.code_hash);
        world
            .contract_code
            .insert(callee_verified.code_hash, callee_code.as_slice().to_vec());
        world.contract_manifests.insert(
            callee_verified.code_hash,
            callee_verified.manifest.signed(&kp),
        );
        world
            .contract_instances
            .insert(callee_address.clone(), callee_verified.code_hash);

        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);

        let mut bind_metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut bind_metadata);
        bind_metadata.insert(
            "contract_address".parse().expect("metadata key"),
            iroha_primitives::json::Json::new(outer_address.to_string()),
        );
        bind_metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            iroha_primitives::json::Json::new("bind"),
        );
        bind_metadata.insert(
            "contract_payload".parse().expect("metadata key"),
            iroha_primitives::json::Json::from_str_norito(&format!(
                r#"{{"callee_contract":"0x{}"}}"#,
                hex::encode(callee_address.as_ref())
            ))
            .expect("bind payload"),
        );
        let bind_tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(bind_metadata)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(outer_bind_code)))
            .sign(kp.private_key());
        let bind_overlay = build_overlay_for_transaction(&bind_tx, &state.view())
            .expect("build bound-state initialization overlay");
        assert!(
            bind_overlay.has_durable_state_changes(),
            "outer contract binding must be represented as durable state"
        );
        let mut block = state.block(BlockHeader::new(
            core::num::NonZeroU64::new(1).expect("non-zero block height"),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut state_tx = block.transaction();
        bind_overlay
            .apply(&mut state_tx, &authority)
            .expect("apply bound-state initialization overlay");
        // The initial state write must use a non-ZK executable because raw `Executable::Ivm`
        // correctly rejects the ZK mode bit. Rebind the same address to the proof-capable artifact
        // before deriving and replaying the proved call; durable state is address-scoped.
        state_tx
            .world
            .contract_code
            .insert(outer_verified.code_hash, outer_code.as_slice().to_vec());
        state_tx.world.contract_manifests.insert(
            outer_verified.code_hash,
            outer_verified.manifest.signed(&kp),
        );
        state_tx
            .world
            .contract_instances
            .insert(outer_address.clone(), outer_verified.code_hash);
        state_tx.apply();
        block.commit().expect("commit bound-state initialization");

        let mut replay_metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut replay_metadata);
        replay_metadata.insert(
            "contract_address".parse().expect("metadata key"),
            iroha_primitives::json::Json::new(outer_address.to_string()),
        );
        replay_metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            iroha_primitives::json::Json::new("run"),
        );
        replay_metadata.insert(
            "contract_payload".parse().expect("metadata key"),
            iroha_primitives::json::Json::from_str_norito(r#"{"value":9}"#).expect("run payload"),
        );
        let empty_overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> =
            Vec::new().into();
        let proved = IvmProved {
            bytecode: IvmBytecode::from_compiled(outer_code),
            overlay: empty_overlay.clone(),
            events_commitment: Hash::new(b"placeholder-events"),
            gas_policy_commitment: Hash::new(b"placeholder-gas"),
        };
        let replay_tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_metadata(replay_metadata)
            .with_executable(Executable::IvmProved(proved.clone()))
            .sign(kp.private_key());
        let overlay_hash =
            Hash::new(norito::to_bytes(&empty_overlay).expect("encode placeholder proved overlay"));
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let summary = ivm_cache
            .summarize_program(proved.bytecode.as_ref())
            .expect("summarize nested proved contract");
        let error = replay_ivm_proved_overlay(
            &state.view(),
            &replay_tx,
            &summary,
            TEST_GAS_LIMIT,
            overlay_hash,
        )
        .expect_err("ABI V1 must reject nested proved authorization contexts");
        assert!(
            matches!(
                &error,
                OverlayBuildError::ZkProof(message)
                    if message.contains("only exact top-level authorization")
                        && message.contains("nested or mismatched contexts are forbidden")
            ),
            "unexpected nested proved replay error: {error:?}"
        );

        let detail_key: Name = "proof_replay".parse().expect("detail key");
        assert!(
            state
                .view()
                .world()
                .account(&outer_address.subject_id())
                .expect("outer contract subject account")
                .metadata()
                .get(&detail_key)
                .is_none(),
            "rejected nested proved replay must apply no queued instruction"
        );
    }

    #[test]
    fn proved_contract_permission_denies_before_argument_decode_or_proof_validation() {
        let compiler =
            ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
                force_zk: true,
                max_cycles: 10_000,
                ..ivm::kotodama::compiler::CompilerOptions::default()
            });
        let (program, manifest) = compiler
            .compile_source_with_manifest(
                r#"
seiyaku ProtectedProved {
  kotoage fn write(value: i64) authorize("CanWriteProved") {
    let _value = value;
  }
}
"#,
            )
            .expect("compile protected ZK-mode contract");
        let (authority, keypair) = gen_account_in("wonderland");
        let domain = iroha_data_model::domain::Domain::new(
            DomainId::try_new("wonderland", "universal").expect("valid domain"),
        )
        .build(&authority);
        let account = build_wonderland_account(&authority);
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            93,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("derive protected proved-call contract address");
        let code_hash = manifest.code_hash.expect("verified code hash");
        let mut world = crate::state::World::with([domain], [account], []);
        world.contract_code.insert(code_hash, program.clone());
        world.contract_manifests.insert(code_hash, manifest);
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let mut state = State::new_with_chain(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            ChainId::from("protected-proved-overlay"),
        );
        state.zk.halo2.enabled = true;
        state.pipeline.ivm_proved.enabled = true;

        let mut metadata = Metadata::default();
        insert_gas_limit(&mut metadata);
        metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            Json::new("write"),
        );
        metadata.insert(
            "contract_payload".parse().expect("metadata key"),
            Json::from(norito::json!({ "value": 9 })),
        );
        metadata.insert(
            "contract_address".parse().expect("metadata key"),
            Json::new(contract_address.to_string()),
        );
        let transaction =
            TransactionBuilder::new(ChainId::from("protected-proved-overlay"), authority)
                .with_metadata(metadata)
                .with_executable(Executable::IvmProved(
                    iroha_data_model::transaction::IvmProved {
                        bytecode: IvmBytecode::from_compiled(program),
                        overlay: Vec::<InstructionBox>::new().into(),
                        events_commitment: Hash::new(b"unverified-events"),
                        gas_policy_commitment: Hash::new(b"unverified-gas"),
                    },
                ))
                .sign(keypair.private_key());

        ivm::reset_argument_record_decode_count();
        let error = build_overlay_for_transaction(&transaction, &state.view())
            .expect_err("missing permission must reject before inspecting the proof");
        assert!(
            matches!(
                &error,
                OverlayBuildError::ContractCall(message)
                    if message.contains("CanWriteProved")
            ),
            "permission denial must win over proof-validation errors: {error:?}"
        );
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "denied proved-call arguments must remain undecoded"
        );
    }

    fn sample_program() -> (Vec<u8>, usize, ivm::ProgramMetadata) {
        let meta = ivm::ProgramMetadata {
            max_cycles: 1,
            ..ivm::ProgramMetadata::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&sample_contract_interface().encode_section());
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let parsed = ivm::ProgramMetadata::parse(&program).expect("parse sample program");
        (program, parsed.header_len, parsed.metadata)
    }

    fn sample_program_zk_mode() -> (Vec<u8>, usize, ivm::ProgramMetadata) {
        let meta = ivm::ProgramMetadata {
            max_cycles: 1,
            mode: ivm::ivm_mode::ZK,
            ..ivm::ProgramMetadata::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&sample_contract_interface().encode_section());
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let parsed = ivm::ProgramMetadata::parse(&program).expect("parse sample program");
        (program, parsed.header_len, parsed.metadata)
    }

    fn sample_contract_interface() -> ivm::EmbeddedContractInterfaceV1 {
        ivm::EmbeddedContractInterfaceV1 {
            seiyaku_name: "OverlayFixture".to_owned(),
            compiler_fingerprint: "iroha-core-overlay-tests".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: "main".to_owned(),
                kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        }
    }

    fn bind_sample_raw_contract(
        world: &mut crate::state::World,
        authority: &AccountId,
        bytecode: &IvmBytecode,
        nonce: u64,
    ) -> ContractAddress {
        let verified = ivm::verify_contract_artifact(bytecode.as_ref())
            .expect("sample raw contract artifact must verify");
        let code_hash = verified.code_hash;
        let address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            authority,
            nonce,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive sample raw contract address");
        world
            .contract_code
            .insert(code_hash, bytecode.as_ref().to_vec());
        world
            .contract_manifests
            .insert(code_hash, verified.manifest);
        world.contract_instances.insert(address.clone(), code_hash);
        address
    }

    fn bind_sample_raw_metadata(metadata: &mut Metadata, address: &ContractAddress) {
        metadata.insert(
            "contract_entrypoint".parse().expect("metadata key"),
            Json::new("main"),
        );
        metadata.insert(
            "contract_address".parse().expect("metadata key"),
            Json::new(address.to_string()),
        );
    }

    fn norito_blob<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
        norito::to_bytes(value).expect("norito encode payload with header")
    }

    fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
        v.extend_from_slice(&type_id.to_be_bytes());
        v.push(1u8);
        let payload_len =
            u32::try_from(payload.len()).expect("payload length must fit into u32 for TLV");
        v.extend_from_slice(&payload_len.to_be_bytes());
        v.extend_from_slice(payload);
        let hash = Hash::new(payload);
        v.extend_from_slice(hash.as_ref());
        v
    }

    fn program_with_literals(code: &[u8], literal_data: &[u8]) -> Vec<u8> {
        let meta = ivm::ProgramMetadata {
            max_cycles: 10_000,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(b"LTLB");
        program.extend_from_slice(&0_u32.to_le_bytes());
        program.extend_from_slice(&0_u32.to_le_bytes());
        let data_len =
            u32::try_from(literal_data.len()).expect("literal data length fits into u32");
        program.extend_from_slice(&data_len.to_le_bytes());
        program.extend_from_slice(literal_data);
        program.extend_from_slice(code);
        program
    }

    #[test]
    fn overlay_rejects_manifest_abi_mismatch_before_execution() {
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, TransactionBuilder},
        };
        use iroha_primitives::json::Json;
        use std::sync::Arc;

        let (program, header_len, meta) = sample_program();
        let (code_hash, abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let contract_address: ContractAddress =
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address");
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());

        // Inject a manifest with a mismatched abi_hash into WSV plus the instance binding.
        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let mut wrong_bytes = [0u8; 32];
        wrong_bytes.copy_from_slice(abi_hash.as_ref());
        wrong_bytes[0] ^= 0xFF;
        let wrong_abi_hash = Hash::prehashed(wrong_bytes);
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: Some(wrong_abi_hash),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            }
            .signed(&kp),
        );
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);

        // Build a contract-call style transaction that references the instance.
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_address").expect("static name"),
            Json::new(contract_address.to_string()),
        );
        insert_gas_limit(&mut metadata);
        insert_gas_limit(&mut metadata);

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(
                iroha_data_model::prelude::IvmBytecode::from_compiled(program),
            ))
            .sign(kp.private_key());

        let res = build_overlay_for_transaction(&tx, &state.view());
        assert!(matches!(
            res,
            Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::ManifestAbiHashMismatch(info)
            )) if info.expected == wrong_abi_hash && info.actual == abi_hash
        ));
    }

    #[test]
    fn raw_and_proved_ivm_reject_spoofed_contract_alias_metadata() {
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            transaction::IvmProved,
        };
        use iroha_primitives::json::Json;
        use std::sync::Arc;

        let (program, header_len, meta) = sample_program_zk_mode();
        let (code_hash, abi_hash) = super::compute_program_hashes(&meta, header_len, &program);
        let bytecode = IvmBytecode::from_compiled(program);
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let contract_address = ContractAddress::derive(
            0,
            &authority,
            9,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let active_alias: iroha_data_model::smart_contract::ContractAlias =
            "router::universal".parse().expect("active alias");
        let spoofed_alias = "benefit::universal";

        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        world
            .contract_code
            .insert(code_hash, bytecode.as_ref().to_vec());
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: Some(abi_hash),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                provenance: None,
            }
            .signed(&kp),
        );
        world
            .bind_contract_alias(&contract_address, active_alias, None, None, 0)
            .expect("bind canonical alias");
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);
        let summary = IvmCache::new()
            .summarize_program(bytecode.as_ref())
            .expect("program summary");

        let executable_variants = [
            Executable::Ivm(bytecode.clone()),
            Executable::IvmProved(IvmProved {
                bytecode,
                overlay: Vec::<InstructionBox>::new().into(),
                events_commitment: Hash::new(b"events"),
                gas_policy_commitment: Hash::new(b"gas"),
            }),
        ];
        for executable in executable_variants {
            let mut metadata = Metadata::default();
            metadata.insert(
                "contract_address".parse().expect("metadata key"),
                Json::new(contract_address.to_string()),
            );
            metadata.insert(
                "contract_alias".parse().expect("metadata key"),
                Json::new(spoofed_alias),
            );
            insert_gas_limit(&mut metadata);
            let tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
                .with_metadata(metadata)
                .with_executable(executable)
                .sign(kp.private_key());
            let error = validate_contract_binding(&state.view(), &tx, &summary)
                .expect_err("spoofed alias must fail before VM/proof execution");
            assert!(
                matches!(
                    error,
                    OverlayBuildError::ContractCall(ref message)
                        if message.contains("does not match the active binding")
                ),
                "unexpected spoofed-alias error: {error:?}"
            );
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn overlay_rejects_axt_without_policy_entries() {
        use iroha_data_model::{
            nexus::{AxtRejectReason, DataSpaceId, LaneId},
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
            transaction::Executable,
        };
        use ivm::{
            axt::{
                self, AssetHandle, GroupBinding, HandleBudget, HandleSubject, RemoteSpendIntent,
            },
            encoding, instruction,
            pointer_abi::PointerType,
            syscalls as ivm_sys,
        };
        use std::sync::Arc;

        const LITERAL_HEADER_LEN: usize = 16;
        const POINTER_TABLE_LEN: usize = 32;

        let dsid = DataSpaceId::new(7);
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("binding");
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let authority_str = authority.to_string();
        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority_str.clone(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(1),
            axt_binding: binding.to_vec(),
            manifest_view_root: vec![0x11; 32],
            expiry_slot: 40,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: axt::SpendOp {
                kind: "transfer".into(),
                from: authority_str,
                to: "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76".into(),
                amount: "5".into(),
            },
        };

        let descriptor_tlv = make_tlv(PointerType::AxtDescriptor as u16, &norito_blob(&descriptor));
        let dsid_tlv = make_tlv(PointerType::DataSpaceId as u16, &norito_blob(&dsid));
        let handle_tlv = make_tlv(PointerType::AssetHandle as u16, &norito_blob(&handle));
        let intent_tlv = make_tlv(PointerType::NoritoBytes as u16, &norito_blob(&intent));

        let tlv_base = LITERAL_HEADER_LEN + POINTER_TABLE_LEN;
        let desc_ptr = tlv_base;
        let dsid_ptr = desc_ptr + descriptor_tlv.len();
        let handle_ptr = dsid_ptr + dsid_tlv.len();
        let intent_ptr = handle_ptr + handle_tlv.len();

        let mut literal_data = Vec::new();
        for ptr in [desc_ptr, dsid_ptr, handle_ptr, intent_ptr] {
            literal_data.extend_from_slice(&(ptr as u64).to_le_bytes());
        }
        literal_data.extend_from_slice(&descriptor_tlv);
        literal_data.extend_from_slice(&dsid_tlv);
        literal_data.extend_from_slice(&handle_tlv);
        literal_data.extend_from_slice(&intent_tlv);
        let pad = (4 - (literal_data.len() % 4)) % 4;
        if pad != 0 {
            literal_data.resize(literal_data.len() + pad, 0);
        }

        let mut code = Vec::new();
        let mut emit = |word: u32| code.extend_from_slice(&word.to_le_bytes());
        let base_imm = i8::try_from(LITERAL_HEADER_LEN).expect("literal header fits i8");
        emit(encoding::wide::encode_ri(
            instruction::wide::arithmetic::ADDI,
            1,
            0,
            base_imm,
        ));
        emit(encoding::wide::encode_load(
            instruction::wide::memory::LOAD64,
            20,
            1,
            0,
        ));
        emit(encoding::wide::encode_load(
            instruction::wide::memory::LOAD64,
            21,
            1,
            8,
        ));
        emit(encoding::wide::encode_load(
            instruction::wide::memory::LOAD64,
            22,
            1,
            16,
        ));
        emit(encoding::wide::encode_load(
            instruction::wide::memory::LOAD64,
            23,
            1,
            24,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            10,
            20,
            0,
        ));
        emit(encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV).expect("syscall fits in u8"),
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            40,
            10,
            0,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            10,
            21,
            0,
        ));
        emit(encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV).expect("syscall fits in u8"),
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            41,
            10,
            0,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            10,
            22,
            0,
        ));
        emit(encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV).expect("syscall fits in u8"),
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            42,
            10,
            0,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            10,
            23,
            0,
        ));
        emit(encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV).expect("syscall fits in u8"),
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            43,
            10,
            0,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            10,
            40,
            0,
        ));
        emit(encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_AXT_BEGIN).expect("syscall fits in u8"),
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            10,
            41,
            0,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            11,
            0,
            0,
        ));
        emit(encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_AXT_TOUCH).expect("syscall fits in u8"),
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            10,
            42,
            0,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            11,
            43,
            0,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            12,
            0,
            0,
        ));
        emit(encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_USE_ASSET_HANDLE).expect("syscall fits in u8"),
        ));
        emit(encoding::wide::encode_halt());

        let program = program_with_literals(&code, &literal_data);
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(
            crate::state::World::default(),
            Arc::clone(&kura),
            query,
        );
        assert!(
            state.view().axt_policy_snapshot().entries.is_empty(),
            "expected empty AXT policy snapshot"
        );

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
            .sign(kp.private_key());

        let err = build_overlay_for_transaction(&tx, &state.view())
            .expect_err("overlay should reject AXT handle without policy entry");
        match err {
            OverlayBuildError::AxtReject(ctx) => {
                assert_eq!(ctx.reason, AxtRejectReason::MissingPolicy);
                assert_eq!(ctx.dataspace, Some(dsid));
                assert_eq!(ctx.lane, Some(LaneId::new(1)));
            }
            other => panic!("expected AxtReject, got {other:?}"),
        }
    }

    #[test]
    fn overlay_rejects_contract_binding_code_hash_mismatch() {
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, TransactionBuilder},
        };
        use iroha_primitives::json::Json;
        use std::sync::Arc;

        let (program, header_len, meta) = sample_program();
        let (code_hash, abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let contract_address: ContractAddress =
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address");
        let wrong_binding = Hash::new(b"other-binding");
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());

        // Insert a manifest for the actual code, but bind the namespace to a different code hash.
        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert(contract_address.clone(), wrong_binding);
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: Some(abi_hash),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            }
            .signed(&kp),
        );
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_address").expect("static name"),
            Json::new(contract_address.to_string()),
        );
        insert_gas_limit(&mut metadata);

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(
                iroha_data_model::prelude::IvmBytecode::from_compiled(program),
            ))
            .sign(kp.private_key());

        let res = build_overlay_for_transaction(&tx, &state.view());
        assert!(matches!(
            res,
            Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::ManifestCodeHashMismatch(info)
            )) if info.expected == wrong_binding && info.actual == code_hash
        ));
    }

    #[test]
    fn overlay_requires_manifest_for_bound_instance() {
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, TransactionBuilder},
        };
        use iroha_primitives::json::Json;
        use std::sync::Arc;

        let (program, header_len, meta) = sample_program();
        let (code_hash, _abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let contract_address: ContractAddress =
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address");
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());

        // Bind namespace to code hash but do not seed manifest in WSV.
        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_address").expect("static name"),
            Json::new(contract_address.to_string()),
        );
        insert_gas_limit(&mut metadata);

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(
                iroha_data_model::prelude::IvmBytecode::from_compiled(program),
            ))
            .sign(kp.private_key());

        let res = build_overlay_for_transaction(&tx, &state.view());
        assert!(matches!(
            res,
            Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::BytecodeDecodingFailed(msg)
            )) if msg.contains("manifest missing")
        ));
    }

    #[test]
    fn overlay_requires_manifest_abi_for_bound_instance() {
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, TransactionBuilder},
        };
        use iroha_primitives::json::Json;
        use std::sync::Arc;

        let (program, header_len, meta) = sample_program();
        let (code_hash, _abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let contract_address: ContractAddress =
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address");
        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());

        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: None,
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            }
            .signed(&kp),
        );
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_address").expect("static name"),
            Json::new(contract_address.to_string()),
        );
        insert_gas_limit(&mut metadata);

        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(
                iroha_data_model::prelude::IvmBytecode::from_compiled(program),
            ))
            .sign(kp.private_key());

        let res = build_overlay_for_transaction(&tx, &state.view());
        assert!(matches!(
            res,
            Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::BytecodeDecodingFailed(msg)
            )) if msg.contains("manifest missing abi_hash")
        ));

        // Ensure ABI mismatch still reports the structured error when abi_hash is present.
        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: Some(Hash::prehashed([0u8; 32])),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            }
            .signed(&kp),
        );
        let state = crate::state::State::new_for_testing(
            world,
            Arc::clone(&kura),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let res = build_overlay_for_transaction(&tx, &state.view());
        assert!(matches!(
            res,
            Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::ManifestAbiHashMismatch(_)
            ))
        ));
    }

    #[test]
    fn pre_execution_policy_allows_scallx_opcode() {
        use iroha_data_model::prelude::{AccountId, TransactionBuilder};
        use std::sync::Arc;

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain: iroha_data_model::domain::Domain = iroha_data_model::domain::Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        )
        .build(&authority);
        let account = build_wonderland_account(&authority);
        let world = crate::state::World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_with_chain(
            world,
            Arc::clone(&kura),
            query_handle,
            ChainId::from("chain"),
        );

        let meta = ivm::ProgramMetadata {
            max_cycles: 8,
            ..ivm::ProgramMetadata::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(
            &ivm::encoding::wide::encode_syscallx(ivm::syscalls::SYSCALL_DEBUG_PRINT).to_le_bytes(),
        );
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(
                iroha_data_model::prelude::IvmBytecode::from_compiled(program),
            ))
            .sign(kp.private_key());

        let res = build_overlay_for_transaction(&tx, &state.view());
        assert!(res.is_ok(), "SCALLX is part of the first-release ABI");
    }

    #[test]
    fn pre_execution_policy_ignores_literal_table() {
        use iroha_data_model::prelude::{AccountId, TransactionBuilder};
        use std::sync::Arc;

        let kp = checked_keypair();
        let authority = AccountId::new(kp.public_key().clone());
        let domain: iroha_data_model::domain::Domain = iroha_data_model::domain::Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        )
        .build(&authority);
        let account = build_wonderland_account(&authority);
        let world = crate::state::World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_with_chain(
            world,
            Arc::clone(&kura),
            query_handle,
            ChainId::from("chain"),
        );

        let meta = ivm::ProgramMetadata {
            max_cycles: 8,
            ..ivm::ProgramMetadata::default()
        };
        let mut program = meta.encode();
        // Literal table with a 0x62 byte to ensure pre-exec scans skip it.
        program.extend_from_slice(b"LTLB");
        program.extend_from_slice(&0u32.to_le_bytes()); // literal count
        program.extend_from_slice(&0u32.to_le_bytes()); // post-pad
        program.extend_from_slice(&4u32.to_le_bytes()); // data length
        program.extend_from_slice(&[0x62, 0x00, 0x00, 0x00]);
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut metadata);
        let tx = TransactionBuilder::new(state.chain_id.clone(), authority)
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(
                iroha_data_model::prelude::IvmBytecode::from_compiled(program),
            ))
            .sign(kp.private_key());

        let res = build_overlay_for_transaction(&tx, &state.view());
        assert!(res.is_ok(), "literal table should not affect opcode scan");
    }

    #[test]
    fn redundant_contract_ops_are_pruned() {
        use std::sync::Arc;

        use iroha_data_model::smart_contract::manifest::ContractManifest;

        use crate::{kura::Kura, query::store::LiveQueryStore, state::State};

        let (program, header_len, meta) = sample_program();
        let (code_hash, abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let mut world = crate::state::World::default();
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: Some(abi_hash),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        };
        world.contract_manifests.insert(code_hash, manifest.clone());
        world.contract_code.insert(code_hash, program.clone());
        let contract_address: ContractAddress =
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address");
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, Arc::clone(&kura), query);

        let mut queued: Vec<InstructionBox> = vec![
            RegisterSmartContractBytes {
                code_hash,
                code: program.clone(),
            }
            .into(),
            RegisterSmartContractCode {
                manifest: manifest.clone(),
            }
            .into(),
            ActivateContractInstance {
                contract_address,
                code_hash,
            }
            .into(),
            RemoveSmartContractBytes {
                code_hash,
                reason: None,
            }
            .into(),
        ];
        prune_redundant_contract_ops(&state.view(), &mut queued);
        assert_eq!(queued.len(), 1);
        assert!(
            queued[0]
                .as_any()
                .downcast_ref::<RemoveSmartContractBytes>()
                .is_some()
        );
    }

    #[test]
    fn sample_smart_contract_overlay_executes() {
        use std::sync::Arc;

        use iroha_data_model::{
            metadata::Metadata, prelude::TransactionBuilder, transaction::Executable,
        };
        use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, load_sample_ivm};

        let chain: ChainId = "chain".parse().expect("valid chain id");
        let mut metadata = Metadata::default();
        insert_gas_limit(&mut metadata);
        let tx = TransactionBuilder::new(chain, ALICE_ID.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(load_sample_ivm(
                "smart_contract_can_filter_queries",
            )))
            .sign(ALICE_KEYPAIR.private_key());

        let accounts = vec![ALICE_ID.clone()];
        let bytes: Vec<u8> = match tx.instructions() {
            Executable::Ivm(code) => code.as_ref().to_vec(),
            _ => unreachable!("expected IVM executable"),
        };
        let parsed = ivm::ProgramMetadata::parse(&bytes).expect("metadata parses");
        let decoded =
            ivm::ivm_cache::global_get_with_meta(&bytes[parsed.code_offset..], &parsed.metadata)
                .expect("bytecode decodes before execution");
        let mut vm = ivm::IVM::new(TEST_GAS_LIMIT);
        let host = crate::smartcontracts::ivm::host::CoreHost::with_accounts(
            tx.authority().clone(),
            Arc::new(accounts),
        );
        vm.set_host(host);
        vm.load_program(&bytes).expect("program loads");
        vm.set_gas_limit(TEST_GAS_LIMIT);
        if let Err(err) = vm.run() {
            let code_bytes = vm.memory.read_code_bytes();
            let original_code = bytes[parsed.header_len..].to_vec();
            let diffs = code_bytes
                .iter()
                .zip(original_code.iter())
                .filter(|(a, b)| a != b)
                .count();
            let pc_usize = usize::try_from(vm.pc).ok();
            let word = pc_usize.and_then(|pc| {
                if pc + 4 <= code_bytes.len() {
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&code_bytes[pc..pc + 4]);
                    Some(u32::from_le_bytes(buf))
                } else {
                    None
                }
            });
            let target_pc = vm.pc.saturating_sub(parsed.prefix_len() as u64);
            let has_decoded = decoded.iter().any(|op| op.pc == target_pc);
            let decoded_inst = decoded
                .iter()
                .find(|op| op.pc == target_pc)
                .map(|op| op.inst);
            let r10 = vm.registers.get(10);
            let r11 = vm.registers.get(11);
            let r12 = vm.registers.get(12);
            let dump_tlv = |addr: u64, vm: &ivm::IVM| -> Option<String> {
                let mut buf = vec![0u8; 48];
                vm.memory
                    .load_bytes(addr, &mut buf)
                    .ok()
                    .map(|()| hex::encode(buf))
            };
            panic!(
                "vm.run failed: {err:?} pc=0x{:x} gas_remaining={} word={word:#?} decoded_entry={has_decoded} inst={decoded_inst:#?} code_diffs={diffs} r10=0x{r10:x} r11=0x{r11:x} r12=0x{r12:x} tlv10={:?} tlv11={:?} tlv12={:?}",
                vm.pc,
                vm.gas_remaining,
                dump_tlv(r10, &vm),
                dump_tlv(r11, &vm),
                dump_tlv(r12, &vm)
            );
        }
    }
}

fn extract_amx_budget_violation(vm: &mut ivm::IVM) -> Option<AmxBudgetViolation> {
    let host_any = vm.host_mut_any()?;
    host_any
        .downcast_mut::<crate::smartcontracts::ivm::host::CoreHost>()
        .and_then(crate::smartcontracts::ivm::host::CoreHost::take_amx_budget_violation)
}

fn clear_axt_reject(vm: &mut ivm::IVM) {
    if let Some(host_any) = vm.host_mut_any() {
        if let Some(host) = host_any.downcast_mut::<crate::smartcontracts::ivm::host::CoreHost>() {
            host.clear_axt_reject();
        }
    }
}

fn extract_axt_reject(vm: &mut ivm::IVM) -> Option<AxtRejectContext> {
    let host_any = vm.host_mut_any()?;
    host_any
        .downcast_mut::<crate::smartcontracts::ivm::host::CoreHost>()
        .and_then(crate::smartcontracts::ivm::host::CoreHost::take_axt_reject)
}

fn run_vm(vm: &mut ivm::IVM) -> Result<(), OverlayBuildError> {
    clear_axt_reject(vm);
    match vm.run() {
        Ok(()) => Ok(()),
        Err(ivm::VMError::AmxBudgetExceeded {
            dataspace,
            stage,
            elapsed_ms,
            budget_ms,
        }) => {
            let violation = AmxBudgetViolation {
                dataspace,
                stage,
                elapsed_ms: u32::try_from(elapsed_ms.min(u64::from(u32::MAX)))
                    .expect("elapsed_ms clamped to u32::MAX"),
                budget_ms: u32::try_from(budget_ms.min(u64::from(u32::MAX)))
                    .expect("budget_ms clamped to u32::MAX"),
            };
            Err(OverlayBuildError::AmxBudgetViolation(violation))
        }
        Err(err) => {
            if let Some(reject) = extract_axt_reject(vm) {
                return Err(OverlayBuildError::AxtReject(reject));
            }
            extract_amx_budget_violation(vm)
                .map(OverlayBuildError::AmxBudgetViolation)
                .map_or_else(|| Err(OverlayBuildError::IvmRun(err)), Err)
        }
    }
}

fn run_vm_with_host<QS: crate::smartcontracts::ivm::host::QueryStateAccess + Default>(
    vm: &mut ivm::IVM,
    host: &mut crate::smartcontracts::ivm::host::CoreHostImpl<QS>,
) -> Result<(), OverlayBuildError> {
    host.clear_axt_reject();
    match vm.run_with_host(host) {
        Ok(()) => Ok(()),
        Err(ivm::VMError::AmxBudgetExceeded {
            dataspace,
            stage,
            elapsed_ms,
            budget_ms,
        }) => {
            let violation = AmxBudgetViolation {
                dataspace,
                stage,
                elapsed_ms: u32::try_from(elapsed_ms.min(u64::from(u32::MAX)))
                    .expect("elapsed_ms clamped to u32::MAX"),
                budget_ms: u32::try_from(budget_ms.min(u64::from(u32::MAX)))
                    .expect("budget_ms clamped to u32::MAX"),
            };
            Err(OverlayBuildError::AmxBudgetViolation(violation))
        }
        Err(err) => {
            if let Some(reject) = host.take_axt_reject() {
                return Err(OverlayBuildError::AxtReject(reject));
            }
            host.take_amx_budget_violation()
                .map(OverlayBuildError::AmxBudgetViolation)
                .map_or_else(|| Err(OverlayBuildError::IvmRun(err)), Err)
        }
    }
}

pub(crate) fn amx_timeout_message(violation: &AmxBudgetViolation) -> String {
    match violation.as_canonical() {
        CanonicalErrorKind::AmxTimeout(detail) => format!(
            "AMX_TIMEOUT dataspace={} stage={:?} elapsed_ms={} budget_ms={}",
            detail.dataspace.as_u64(),
            detail.stage,
            detail.elapsed_ms,
            detail.budget_ms
        ),
        _ => "AMX_TIMEOUT".to_owned(),
    }
}

/// Structured error type for overlay construction failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayBuildError {
    /// Failed to parse IVM header metadata.
    IvmHeaderParse,
    /// IVM header violated node policy (structured admission error).
    HeaderPolicy(IvmAdmissionError),
    /// Contract-call metadata was malformed or could not be applied.
    ContractCall(String),
    /// Missing or invalid `gas_limit` transaction metadata.
    GasLimit(String),
    /// Loading the program into the VM failed.
    IvmLoad(IvmError),
    /// Running the VM to collect queued ISIs failed.
    IvmRun(IvmError),
    /// AXT policy rejected the envelope with structured context.
    AxtReject(AxtRejectContext),
    /// AMX budget violation during overlay execution.
    AmxBudgetViolation(AmxBudgetViolation),
    /// Transaction classified into quarantine but exceeded per-block cap.
    QuarantineOverflow,
    /// ZK proof-related rejection (missing/invalid/unsupported).
    ZkProof(String),
}

impl OverlayBuildError {
    /// Return whether rebuilding against a later serial state may change the
    /// result. Structural, policy, gas, proof, and quarantine failures are
    /// invariant and must remain rejected without another execution attempt.
    #[must_use]
    pub(crate) const fn may_change_with_live_state(&self) -> bool {
        matches!(
            self,
            Self::ContractCall(_) | Self::IvmRun(_) | Self::AxtReject(_)
        )
    }
}

impl core::fmt::Display for OverlayBuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OverlayBuildError::IvmHeaderParse => write!(f, "IVM header parse error"),
            OverlayBuildError::HeaderPolicy(e) => write!(f, "header policy: {e:?}"),
            OverlayBuildError::ContractCall(msg) => write!(f, "{msg}"),
            OverlayBuildError::GasLimit(msg) => write!(f, "{msg}"),
            OverlayBuildError::IvmLoad(e) => write!(f, "ivm.load_program: {e}"),
            OverlayBuildError::IvmRun(e) => write!(f, "ivm.run: {e}"),
            OverlayBuildError::AxtReject(ctx) => write!(f, "axt_reject: {ctx}"),
            OverlayBuildError::AmxBudgetViolation(v) => write!(f, "{}", amx_timeout_message(v)),
            OverlayBuildError::QuarantineOverflow => write!(f, "quarantine overflow"),
            OverlayBuildError::ZkProof(msg) => write!(f, "zk_proof: {msg}"),
        }
    }
}

pub(crate) fn enforce_manifest_is_pre_registered<R: StateReadOnly>(
    state_ro: &R,
    tx: &SignedTransaction,
    code_hash: Hash,
) -> Result<(), OverlayBuildError> {
    if tx
        .metadata()
        .get(&iroha_data_model::name::Name::from_str(MANIFEST_METADATA_KEY).unwrap())
        .is_none()
    {
        return Ok(());
    }
    if state_ro
        .world()
        .contract_manifests()
        .get(&code_hash)
        .is_some()
    {
        return Ok(());
    }
    Err(OverlayBuildError::ZkProof(
        "manifest metadata present but contract manifest is not registered in WSV; proved executions do not support implicit manifest append"
            .to_owned(),
    ))
}

fn normalize_halo2_ipa_circuit_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("halo2/pasta/ipa/") {
        return (!rest.is_empty()).then(|| trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("halo2/pasta/") {
        return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
    }
    if let Some(rest) = trimmed.strip_prefix(crate::zk::ZK_BACKEND_HALO2_IPA) {
        if let Some(rest) = rest.strip_prefix("::") {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        if let Some(rest) = rest.strip_prefix(':') {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        if let Some(rest) = rest.strip_prefix('/') {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
    }
    Some(format!("halo2/pasta/ipa/{trimmed}"))
}

fn normalize_stark_fri_circuit_id(backend: &str, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == backend {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix(backend) {
        if let Some(rest) = rest.strip_prefix(':') {
            return (!rest.is_empty()).then(|| trimmed.to_string());
        }
        if let Some(rest) = rest.strip_prefix('/') {
            return (!rest.is_empty()).then(|| format!("{backend}:{rest}"));
        }
    }
    Some(format!("{backend}:{trimmed}"))
}

fn circuit_id_matches(backend: &str, record_id: &str, env_id: &str) -> bool {
    if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
        match (
            normalize_halo2_ipa_circuit_id(record_id),
            normalize_halo2_ipa_circuit_id(env_id),
        ) {
            (Some(rec), Some(env)) => rec == env,
            _ => record_id == env_id,
        }
    } else if crate::zk::is_stark_fri_v1_backend(backend) {
        match (
            normalize_stark_fri_circuit_id(backend, record_id),
            normalize_stark_fri_circuit_id(backend, env_id),
        ) {
            (Some(rec), Some(env)) => rec == env,
            _ => record_id == env_id,
        }
    } else {
        record_id == env_id
    }
}

fn hash_to_u64_limbs_le(hash: &Hash) -> [u64; 4] {
    let bytes: &[u8; 32] = hash.as_ref();
    let mut limbs = [0u64; 4];
    for (i, limb) in limbs.iter_mut().enumerate() {
        let start = i * 8;
        let end = start + 8;
        *limb = u64::from_le_bytes(bytes[start..end].try_into().expect("slice len = 8"));
    }
    limbs
}

fn limb_as_instance_bytes(limb: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&limb.to_le_bytes());
    out
}

fn expected_ivm_exec_public_inputs(
    code_hash: Hash,
    overlay_hash: Hash,
    events_commitment: Hash,
    gas_policy_commitment: Hash,
) -> Vec<[u8; 32]> {
    let code_limbs = hash_to_u64_limbs_le(&code_hash);
    let overlay_limbs = hash_to_u64_limbs_le(&overlay_hash);
    let events_limbs = hash_to_u64_limbs_le(&events_commitment);
    let gas_limbs = hash_to_u64_limbs_le(&gas_policy_commitment);
    code_limbs
        .into_iter()
        .chain(overlay_limbs)
        .chain(events_limbs)
        .chain(gas_limbs)
        .map(limb_as_instance_bytes)
        .collect()
}

const IVM_EVENTS_COMMITMENT_DOMAIN: &[u8] = b"iroha.ivm_proved.events_commitment.v3";
const IVM_GAS_POLICY_COMMITMENT_DOMAIN: &[u8] = b"iroha.ivm_proved.gas_policy_commitment.v3";

fn sha256_to_hash(bytes: &[u8]) -> Hash {
    let digest = Sha256::digest(bytes);
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&digest);
    Hash::prehashed(arr)
}

#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
struct IvmTraceBundleV1 {
    register_trace: Vec<IvmRegisterStateV1>,
    constraints: Vec<IvmConstraintV1>,
    memory_log: Vec<IvmMemEventV1>,
    register_log: Vec<IvmRegEventV1>,
    step_log: Vec<IvmStepEntryV1>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
struct IvmRegisterStateV1 {
    pc: u64,
    gpr: Vec<u64>,
    tags: Vec<u8>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
enum IvmConstraintV1 {
    Zero { reg: u16, cycle: u64 },
    Eq { reg1: u16, reg2: u16, cycle: u64 },
    Range { reg: u16, bits: u8, cycle: u64 },
}

#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
enum IvmMemEventV1 {
    Load {
        addr: u64,
        value: u128,
        size: u8,
        path: Vec<[u8; 32]>,
        root: [u8; 32],
    },
    Store {
        addr: u64,
        value: u128,
        size: u8,
        path: Vec<[u8; 32]>,
        root: [u8; 32],
    },
}

#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
enum IvmRegEventV1 {
    Read {
        index: u16,
        value: u64,
        tag: bool,
        path: Vec<[u8; 32]>,
        root: [u8; 32],
    },
    Write {
        index: u16,
        value: u64,
        tag: bool,
        path: Vec<[u8; 32]>,
        root: [u8; 32],
    },
}

#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
struct IvmStepEntryV1 {
    pc: u64,
    reg_root: [u8; 32],
    mem_root: [u8; 32],
}

fn build_ivm_trace_bundle(vm: &ivm::IVM) -> IvmTraceBundleV1 {
    let register_trace = vm
        .register_trace()
        .into_iter()
        .map(|state| IvmRegisterStateV1 {
            pc: state.pc,
            gpr: state.gpr.to_vec(),
            tags: state
                .tags
                .iter()
                .map(|tag| u8::from(*tag))
                .collect::<Vec<_>>(),
        })
        .collect::<Vec<_>>();

    let constraints = vm
        .constraints()
        .iter()
        .map(|c| match *c {
            ivm::zk::Constraint::Zero { reg, cycle } => IvmConstraintV1::Zero {
                reg: u16::try_from(reg).unwrap_or(u16::MAX),
                cycle,
            },
            ivm::zk::Constraint::Eq { reg1, reg2, cycle } => IvmConstraintV1::Eq {
                reg1: u16::try_from(reg1).unwrap_or(u16::MAX),
                reg2: u16::try_from(reg2).unwrap_or(u16::MAX),
                cycle,
            },
            ivm::zk::Constraint::Range { reg, bits, cycle } => IvmConstraintV1::Range {
                reg: u16::try_from(reg).unwrap_or(u16::MAX),
                bits,
                cycle,
            },
        })
        .collect::<Vec<_>>();

    let memory_log = vm
        .memory_log()
        .iter()
        .map(|e| match e {
            ivm::zk::MemEvent::Load {
                addr,
                value,
                size,
                path,
                root,
            } => IvmMemEventV1::Load {
                addr: *addr,
                value: *value,
                size: *size,
                path: path.clone(),
                root: *root.as_ref(),
            },
            ivm::zk::MemEvent::Store {
                addr,
                value,
                size,
                path,
                root,
            } => IvmMemEventV1::Store {
                addr: *addr,
                value: *value,
                size: *size,
                path: path.clone(),
                root: *root.as_ref(),
            },
        })
        .collect::<Vec<_>>();

    let register_log = vm
        .register_log()
        .iter()
        .map(|e| match e {
            ivm::zk::RegEvent::Read {
                index,
                value,
                tag,
                path,
                root,
            } => IvmRegEventV1::Read {
                index: u16::try_from(*index).unwrap_or(u16::MAX),
                value: *value,
                tag: *tag,
                path: path.clone(),
                root: *root.as_ref(),
            },
            ivm::zk::RegEvent::Write {
                index,
                value,
                tag,
                path,
                root,
            } => IvmRegEventV1::Write {
                index: u16::try_from(*index).unwrap_or(u16::MAX),
                value: *value,
                tag: *tag,
                path: path.clone(),
                root: *root.as_ref(),
            },
        })
        .collect::<Vec<_>>();

    let step_log = vm
        .step_log()
        .iter()
        .map(|entry| IvmStepEntryV1 {
            pc: entry.pc,
            reg_root: *entry.reg_root.as_ref(),
            mem_root: *entry.mem_root.as_ref(),
        })
        .collect::<Vec<_>>();

    IvmTraceBundleV1 {
        register_trace,
        constraints,
        memory_log,
        register_log,
        step_log,
    }
}

fn append_len_prefixed_str(out: &mut Vec<u8>, value: &str) {
    let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn expected_ivm_trace_hash(trace_bundle: &IvmTraceBundleV1) -> Result<Hash, OverlayBuildError> {
    let trace_bytes = norito::to_bytes(trace_bundle)
        .map_err(|_| OverlayBuildError::ZkProof("failed to encode IVM trace bundle".to_owned()))?;
    Ok(sha256_to_hash(&trace_bytes))
}

fn expected_ivm_events_commitment(code_hash: Hash, overlay_hash: Hash, trace_hash: Hash) -> Hash {
    let mut preimage = Vec::with_capacity(
        IVM_EVENTS_COMMITMENT_DOMAIN.len()
            + code_hash.as_ref().len() * 2
            + trace_hash.as_ref().len(),
    );
    preimage.extend_from_slice(IVM_EVENTS_COMMITMENT_DOMAIN);
    preimage.extend_from_slice(code_hash.as_ref());
    preimage.extend_from_slice(overlay_hash.as_ref());
    preimage.extend_from_slice(trace_hash.as_ref());
    sha256_to_hash(&preimage)
}

fn expected_ivm_gas_policy_commitment(
    code_hash: Hash,
    overlay_hash: Hash,
    circuit_id: &str,
    circuit_version: u32,
    gas_schedule_id: &str,
    tx_gas_limit: u64,
    gas_used: u64,
    trace_hash: Hash,
) -> Hash {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(IVM_GAS_POLICY_COMMITMENT_DOMAIN);
    preimage.extend_from_slice(code_hash.as_ref());
    preimage.extend_from_slice(overlay_hash.as_ref());
    preimage.extend_from_slice(crate::smartcontracts::limits::ivm_gas_schedule_hash().as_ref());
    preimage.extend_from_slice(&circuit_version.to_le_bytes());
    preimage.extend_from_slice(&tx_gas_limit.to_le_bytes());
    preimage.extend_from_slice(&gas_used.to_le_bytes());
    // Include a commitment to the execution trace hash so that `gas_used` cannot be
    // brute-forced from the commitment without reproducing the VM trace.
    preimage.extend_from_slice(trace_hash.as_ref());
    append_len_prefixed_str(&mut preimage, circuit_id);
    append_len_prefixed_str(&mut preimage, gas_schedule_id);
    sha256_to_hash(&preimage)
}

fn extract_expected_single_row_columns(columns: Vec<Vec<[u8; 32]>>) -> Option<Vec<[u8; 32]>> {
    let mut out = Vec::with_capacity(columns.len());
    for mut col in columns {
        if col.len() != 1 {
            return None;
        }
        out.push(col.pop()?);
    }
    Some(out)
}

const IVM_OVERLAY_BIND_CIRCUIT_CANONICAL: &str = "halo2/pasta/ipa/ivm-overlay-bind";

fn is_legacy_ivm_overlay_bind_circuit(backend: &str, circuit_id: &str) -> bool {
    backend == crate::zk::ZK_BACKEND_HALO2_IPA
        && normalize_halo2_ipa_circuit_id(circuit_id)
            .as_deref()
            .is_some_and(|normalized| normalized == IVM_OVERLAY_BIND_CIRCUIT_CANONICAL)
}

fn replay_ivm_proved_overlay<R>(
    state_ro: &R,
    tx: &SignedTransaction,
    summary: &ProgramSummary,
    gas_limit: u64,
    overlay_hash: Hash,
) -> Result<IvmProvedReplay, OverlayBuildError>
where
    R: StateReadOnly + QueryStateSource,
{
    let (contract_call_context, contract_runtime_context, entrypoint_authorization) =
        authorize_and_prepare_raw_contract_dispatch(state_ro, tx, summary, gas_limit)?;
    let mut vm = summary
        .checkout_runtime(gas_limit)
        .map_err(OverlayBuildError::IvmLoad)?;
    vm.set_zk_trace_enabled(true);
    let accounts = state_ro.accounts_snapshot();
    let mut host =
        crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts_and_argument_record(
            tx.authority().clone(),
            Arc::clone(&accounts),
            contract_call_context.argument_record.clone(),
        );
    let amx_analysis = ivm::analysis::analyze_prepared(summary.prepared_contract());
    host.set_prepared_contract_cache(summary.prepared_contract_cache());
    host.set_amx_analysis(amx_analysis);
    let amx_limits =
        crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(state_ro.pipeline());
    host.set_amx_limits(amx_limits);
    host.set_axt_timing(state_ro.nexus().axt);
    host.hydrate_axt_replay_ledger(state_ro);
    host.set_public_inputs_from_parameters(state_ro.world().parameters());
    host.set_vrf_epoch_seeds_from_world(state_ro.world());
    host.set_query_state(state_ro);
    host.set_contract_runtime_context(Some(contract_runtime_context.clone()));
    host.set_contract_entrypoint_authorization(Some(entrypoint_authorization.clone()));
    host.set_bound_contract_records_by_subject_snapshot(
        code::snapshot_bound_contract_records_by_subject(state_ro),
    );
    let snapshot = state_ro.axt_policy_snapshot();
    host = host.with_axt_policy_snapshot(&snapshot);
    apply_streaming_metadata(
        &mut host,
        resolve_streaming_metadata(state_ro, tx.authority()),
    );
    #[cfg(feature = "telemetry")]
    host.set_telemetry(state_ro.metrics().clone());
    host.set_crypto_config(state_ro.crypto());
    host.set_zk_config(state_ro.zk());
    host.set_chain_id(state_ro.chain_id());
    host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
        .map_err(OverlayBuildError::IvmRun)?;
    vm.set_gas_limit(gas_limit);
    apply_contract_call_execution_context(&mut vm, Some(&contract_call_context))?;
    run_vm_with_host(&mut vm, &mut host)?;
    let gas_used = gas_limit.saturating_sub(vm.remaining_gas());
    let trace_bundle = build_ivm_trace_bundle(&vm);
    let trace_hash = expected_ivm_trace_hash(&trace_bundle)?;
    let events_commitment =
        expected_ivm_events_commitment(summary.code_hash, overlay_hash, trace_hash);
    let queued = host.drain_queued_instructions_with_contract_runtime_context(Some(
        contract_runtime_context.clone(),
    ));
    let (durable_state_overlay, durable_state_authorizations) =
        host.drain_durable_state_overlay_with_authorizations();
    let completed_axt = host.drain_completed_axt_states();
    if !durable_state_overlay.is_empty() {
        return Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved cannot carry durable StateMap writes in ABI V1".to_owned(),
        ));
    }
    debug_assert!(durable_state_authorizations.is_empty());
    if queued.iter().any(|queued| {
        queued.authority != *tx.authority()
            || queued.entrypoint_authorization.as_ref() != Some(&entrypoint_authorization)
            || queued
                .contract_runtime_context
                .as_ref()
                .is_none_or(|context| {
                    context.contract_subject != contract_runtime_context.contract_subject
                        || context.contract_address != contract_runtime_context.contract_address
                        || context.contract_alias != contract_runtime_context.contract_alias
                        || context.entrypoint != contract_runtime_context.entrypoint
                })
    }) {
        return Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved ABI V1 can preserve only exact top-level authorization for queued host writes; nested or mismatched contexts are forbidden"
                .to_owned(),
        ));
    }
    let mut queued_instructions = queued
        .iter()
        .map(|queued| queued.instruction.clone())
        .collect::<Vec<_>>();
    let mut execution_contexts = queued
        .into_iter()
        .map(|queued| OverlayInstructionExecutionContext {
            authority: queued.authority,
            contract_runtime_context: queued.contract_runtime_context,
            entrypoint_authorization: queued.entrypoint_authorization,
        })
        .collect::<Vec<_>>();
    prune_redundant_contract_ops_with_metadata(
        state_ro,
        &mut queued_instructions,
        Some(&mut execution_contexts),
    );
    let queued = queued_instructions
        .into_iter()
        .zip(execution_contexts)
        .map(
            |(instruction, context)| crate::smartcontracts::ivm::host::QueuedInstruction {
                instruction,
                authority: context.authority,
                contract_runtime_context: context.contract_runtime_context,
                entrypoint_authorization: context.entrypoint_authorization,
            },
        )
        .collect();
    Ok(IvmProvedReplay {
        queued,
        completed_axt,
        durable_state_overlay,
        events_commitment,
        gas_used,
        trace_hash,
    })
}

pub(crate) struct IvmProvedReplay {
    pub(crate) queued: Vec<crate::smartcontracts::ivm::host::QueuedInstruction>,
    pub(crate) completed_axt: Vec<ivm::axt::HostAxtState>,
    pub(crate) durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
    pub(crate) events_commitment: Hash,
    pub(crate) gas_used: u64,
    pub(crate) trace_hash: Hash,
}

pub(crate) fn verify_ivm_proved_execution<R>(
    state_ro: &R,
    tx: &SignedTransaction,
    proved: &iroha_data_model::transaction::IvmProved,
    summary: &ProgramSummary,
) -> Result<IvmProvedReplay, OverlayBuildError>
where
    R: StateReadOnly + QueryStateSource,
{
    if summary.metadata.mode & ivm::ivm_mode::ZK == 0 {
        return Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved requires IVM ZK mode bit (mode & ZK != 0)".to_owned(),
        ));
    }
    let tx_gas_limit = require_tx_gas_limit(tx)?;
    let _ = authorize_and_prepare_raw_contract_dispatch(state_ro, tx, summary, tx_gas_limit)?;
    let pipeline_cfg = state_ro.pipeline();
    if !pipeline_cfg.ivm_proved.enabled {
        return Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved is disabled in node configuration".to_owned(),
        ));
    }
    if pipeline_cfg.ivm_proved.skip_replay {
        return Err(OverlayBuildError::ZkProof(
            "pipeline.ivm_proved.skip_replay is unsafe in ABI V1 because the proof does not commit to the complete invocation; deterministic replay is mandatory"
                .to_owned(),
        ));
    }
    if pipeline_cfg
        .ivm_proved
        .allowed_circuits
        .iter()
        .all(|circuit_id| circuit_id.trim().is_empty())
    {
        return Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved is not enabled for any circuits (pipeline.ivm_proved.allowed_circuits is empty)"
                .to_owned(),
        ));
    }

    let zk_cfg = state_ro.zk();

    let attachments = tx
        .attachments()
        .ok_or_else(|| OverlayBuildError::ZkProof("missing proof attachments".to_owned()))?;
    let list = &attachments.0;
    if list.len() != 1 {
        return Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved expects exactly one proof attachment".to_owned(),
        ));
    }
    let attachment = &list[0];
    if attachment.backend != attachment.proof.backend {
        return Err(OverlayBuildError::ZkProof(
            "proof attachment backend mismatch".to_owned(),
        ));
    }
    if attachment.backend != attachment.vk_ref.backend {
        return Err(OverlayBuildError::ZkProof(
            "proof attachment verifier-key backend mismatch".to_owned(),
        ));
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum IvmProvedBackendKind {
        Halo2Ipa,
        StarkFriV1,
    }
    let backend_kind = if attachment.backend.as_str() == crate::zk::ZK_BACKEND_HALO2_IPA {
        IvmProvedBackendKind::Halo2Ipa
    } else if crate::zk::is_stark_fri_v1_backend(attachment.backend.as_str()) {
        IvmProvedBackendKind::StarkFriV1
    } else {
        return Err(OverlayBuildError::ZkProof(
            "unsupported backend for Executable::IvmProved (expected halo2/ipa or stark/fri)"
                .to_owned(),
        ));
    };

    let proof_len = attachment.proof.bytes.len();
    match backend_kind {
        IvmProvedBackendKind::Halo2Ipa => {
            if !zk_cfg.halo2.enabled {
                return Err(OverlayBuildError::ZkProof(
                    "halo2 verification is disabled in node configuration".to_owned(),
                ));
            }
            if proof_len > zk_cfg.halo2.max_proof_bytes {
                return Err(OverlayBuildError::ZkProof(
                    "proof exceeds node-configured halo2.max_proof_bytes".to_owned(),
                ));
            }
        }
        IvmProvedBackendKind::StarkFriV1 => {
            if !zk_cfg.stark.enabled {
                return Err(OverlayBuildError::ZkProof(
                    "stark verification is disabled in node configuration".to_owned(),
                ));
            }
            if proof_len > zk_cfg.stark.max_envelope_bytes {
                return Err(OverlayBuildError::ZkProof(
                    "proof exceeds node-configured stark.max_envelope_bytes".to_owned(),
                ));
            }
        }
    }

    // Require VK references for governance-controlled circuit selection.
    let vk_id: &VerifyingKeyId = &attachment.vk_ref;

    let vk_record = state_ro
        .world()
        .verifying_keys()
        .get(vk_id)
        .ok_or_else(|| {
            OverlayBuildError::ZkProof(format!(
                "verifying key not found: {}::{}",
                vk_id.backend, vk_id.name
            ))
        })?;

    if vk_record.status != iroha_data_model::confidential::ConfidentialStatus::Active {
        return Err(OverlayBuildError::ZkProof(
            "verifying key is not Active".to_owned(),
        ));
    }
    let expected_record_backend =
        crate::zk::production_verify_backend_tag(attachment.backend.as_str()).ok_or_else(|| {
            OverlayBuildError::ZkProof(
                "proof attachment backend is not admitted by the production verifier registry"
                    .to_owned(),
            )
        })?;
    if vk_record.backend != expected_record_backend {
        return Err(OverlayBuildError::ZkProof(
            "verifying key backend tag mismatch".to_owned(),
        ));
    }
    let gas_schedule_id = vk_record.gas_schedule_id.as_deref().ok_or_else(|| {
        OverlayBuildError::ZkProof("verifying key missing gas_schedule_id".to_owned())
    })?;
    if vk_record.max_proof_bytes > 0
        && proof_len > usize::try_from(vk_record.max_proof_bytes).unwrap_or(usize::MAX)
    {
        return Err(OverlayBuildError::ZkProof(
            "proof exceeds verifying key max_proof_bytes".to_owned(),
        ));
    }
    if !pipeline_cfg
        .ivm_proved
        .allowed_circuits
        .iter()
        .filter_map(|circuit_id| {
            let trimmed = circuit_id.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .any(|allowed| {
            circuit_id_matches(attachment.backend.as_str(), &vk_record.circuit_id, allowed)
        })
    {
        return Err(OverlayBuildError::ZkProof(
            "verifying key circuit_id is not allowlisted for Executable::IvmProved".to_owned(),
        ));
    }
    let circuit_key = (vk_record.circuit_id.clone(), vk_record.version);
    match state_ro
        .world()
        .verifying_keys_by_circuit()
        .get(&circuit_key)
    {
        Some(mapped) if mapped == vk_id => {}
        _ => {
            return Err(OverlayBuildError::ZkProof(
                "verifying key circuit/version not active".to_owned(),
            ));
        }
    }

    let vk_box = vk_record
        .key
        .as_ref()
        .ok_or_else(|| OverlayBuildError::ZkProof("verifying key bytes missing".to_owned()))?;
    let computed_commitment = crate::zk::hash_vk(vk_box);
    if vk_record.commitment != computed_commitment {
        return Err(OverlayBuildError::ZkProof(
            "verifying key commitment mismatch".to_owned(),
        ));
    }
    if vk_box.backend != attachment.backend {
        return Err(OverlayBuildError::ZkProof(
            "verifying key backend mismatch".to_owned(),
        ));
    }

    // Decode and sanity-check the OpenVerifyEnvelope carried in the proof box.
    let env: ZkOpenVerifyEnvelope = norito::decode_from_bytes(&attachment.proof.bytes)
        .map_err(|_| OverlayBuildError::ZkProof("malformed OpenVerifyEnvelope".to_owned()))?;
    let max_envelope_proof_bytes = match backend_kind {
        IvmProvedBackendKind::Halo2Ipa => zk_cfg.halo2.max_proof_bytes,
        IvmProvedBackendKind::StarkFriV1 => zk_cfg.stark.max_proof_bytes,
    };
    let max_envelope_proof_bytes = if vk_record.max_proof_bytes > 0 {
        max_envelope_proof_bytes
            .min(usize::try_from(vk_record.max_proof_bytes).unwrap_or(usize::MAX))
    } else {
        max_envelope_proof_bytes
    };
    env.validate_with_bounds(ZkOpenVerifyEnvelopeBounds {
        max_proof_bytes: max_envelope_proof_bytes,
        ..ZkOpenVerifyEnvelopeBounds::default()
    })
    .map_err(|err| OverlayBuildError::ZkProof(format!("invalid OpenVerifyEnvelope: {err}")))?;
    match backend_kind {
        IvmProvedBackendKind::Halo2Ipa => {
            if env.backend != ZkBackendTag::Halo2IpaPasta {
                return Err(OverlayBuildError::ZkProof(
                    "unsupported OpenVerifyEnvelope backend tag for IvmProved".to_owned(),
                ));
            }
        }
        IvmProvedBackendKind::StarkFriV1 => {
            if env.backend != ZkBackendTag::Stark {
                return Err(OverlayBuildError::ZkProof(
                    "unsupported OpenVerifyEnvelope backend tag for IvmProved".to_owned(),
                ));
            }
        }
    }
    if !circuit_id_matches(
        attachment.backend.as_str(),
        &vk_record.circuit_id,
        &env.circuit_id,
    ) {
        return Err(OverlayBuildError::ZkProof(
            "verifying key circuit mismatch".to_owned(),
        ));
    }
    if is_legacy_ivm_overlay_bind_circuit(attachment.backend.as_str(), &vk_record.circuit_id)
        || is_legacy_ivm_overlay_bind_circuit(attachment.backend.as_str(), &env.circuit_id)
    {
        return Err(OverlayBuildError::ZkProof(
            "Executable::IvmProved rejects `halo2/ipa:ivm-overlay-bind`: the binding-only stand-in circuit is no longer accepted; `ivm-execution-v1` proof attachments are required"
                .to_owned(),
        ));
    }
    let expected_schema_hash = crate::zk::ivm_execution_public_inputs_schema_hash();
    if vk_record.public_inputs_schema_hash != expected_schema_hash {
        return Err(OverlayBuildError::ZkProof(
            "verifying key schema hash mismatch for ivm-execution-v1".to_owned(),
        ));
    }
    let observed_schema_hash: [u8; 32] = *Hash::new(&env.public_inputs).as_ref();
    if observed_schema_hash != expected_schema_hash {
        return Err(OverlayBuildError::ZkProof(
            "proof public input schema hash mismatch".to_owned(),
        ));
    }
    if env.vk_hash != vk_record.commitment {
        return Err(OverlayBuildError::ZkProof(
            "verifying key commitment mismatch".to_owned(),
        ));
    }
    let overlay_hash = {
        let bytes = norito::to_bytes(&proved.overlay).map_err(|_| {
            OverlayBuildError::ZkProof("failed to encode proved overlay".to_owned())
        })?;
        Hash::new(&bytes)
    };
    let expected = expected_ivm_exec_public_inputs(
        summary.code_hash,
        overlay_hash,
        proved.events_commitment,
        proved.gas_policy_commitment,
    );
    let observed = match backend_kind {
        IvmProvedBackendKind::Halo2Ipa => {
            let instance_cols = crate::zk::extract_pasta_instance_columns_bytes(&env.proof_bytes)
                .ok_or_else(|| {
                OverlayBuildError::ZkProof("missing proof instances".to_owned())
            })?;
            extract_expected_single_row_columns(instance_cols).ok_or_else(|| {
                OverlayBuildError::ZkProof(
                    "expected instance columns layout: 1 row per column".to_owned(),
                )
            })?
        }
        IvmProvedBackendKind::StarkFriV1 => {
            let open: StarkFriOpenProofV1 = norito::decode_from_bytes(&env.proof_bytes)
                .map_err(|_| OverlayBuildError::ZkProof("malformed STARK open proof".to_owned()))?;
            if open.version != 1 {
                return Err(OverlayBuildError::ZkProof(
                    "unsupported STARK open proof version".to_owned(),
                ));
            }
            extract_expected_single_row_columns(open.public_inputs).ok_or_else(|| {
                OverlayBuildError::ZkProof(
                    "expected instance columns layout: 1 row per column".to_owned(),
                )
            })?
        }
    };
    if observed != expected {
        return Err(OverlayBuildError::ZkProof(
            "proof public inputs do not match (code_hash, overlay_hash, events_commitment, gas_policy_commitment)"
                .to_owned(),
        ));
    }
    let report = crate::zk::verify_backend_with_timing_checked(
        attachment.backend.as_str(),
        &attachment.proof,
        Some(vk_box),
        zk_cfg,
    );
    if !report.ok {
        return Err(OverlayBuildError::ZkProof("proof rejected".to_owned()));
    }
    let replay = replay_ivm_proved_overlay(state_ro, tx, summary, tx_gas_limit, overlay_hash)?;
    if proved.events_commitment != replay.events_commitment {
        return Err(OverlayBuildError::ZkProof(
            "events commitment mismatch".to_owned(),
        ));
    }
    let expected_gas_policy_commitment = expected_ivm_gas_policy_commitment(
        summary.code_hash,
        overlay_hash,
        &vk_record.circuit_id,
        vk_record.version,
        gas_schedule_id,
        tx_gas_limit,
        replay.gas_used,
        replay.trace_hash,
    );
    if proved.gas_policy_commitment != expected_gas_policy_commitment {
        return Err(OverlayBuildError::ZkProof(
            "gas policy commitment mismatch".to_owned(),
        ));
    }

    let replay_overlay: Vec<_> = replay
        .queued
        .iter()
        .map(|queued| queued.instruction.clone())
        .collect();
    let mut provided_overlay: Vec<InstructionBox> = proved.overlay.iter().cloned().collect();
    prune_redundant_contract_ops(state_ro, &mut provided_overlay);
    if replay_overlay != provided_overlay {
        return Err(OverlayBuildError::ZkProof(
            "proved overlay does not match deterministic IVM replay".to_owned(),
        ));
    }

    Ok(replay)
}

/// Execute an `Executable::Ivm` transaction in the local state view and derive the corresponding
/// [`iroha_data_model::transaction::IvmProved`] payload.
///
/// This helper is intended for Torii/operator tooling to construct the proved payload in a way
/// that matches node-side admission replay verification (`verify_ivm_proved_execution`).
///
/// Note: callers should treat `gas_used` as private; this function returns commitments only.
pub fn derive_ivm_proved_payload_from_ivm_execution<R>(
    state_ro: &R,
    tx: &SignedTransaction,
    vk_record: &iroha_data_model::proof::VerifyingKeyRecord,
) -> Result<iroha_data_model::transaction::IvmProved, OverlayBuildError>
where
    R: StateReadOnly + QueryStateSource,
{
    let bytecode = match tx.instructions() {
        Executable::Ivm(bytecode) => bytecode.clone(),
        other => {
            return Err(OverlayBuildError::ZkProof(format!(
                "expected Executable::Ivm for proved derivation, got {other:?}"
            )));
        }
    };

    let gas_limit = require_tx_gas_limit(tx)?;

    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    let summary = ivm_cache
        .summarize_program(bytecode.as_ref())
        .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
    let meta = summary.metadata.clone();
    validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;

    let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
    if !wants_zk {
        return Err(OverlayBuildError::ZkProof(
            "ivm proved derivation requires IVM ZK mode bit (mode & ZK != 0)".to_owned(),
        ));
    }
    if wants_zk && !(state_ro.zk().halo2.enabled || state_ro.zk().stark.enabled) {
        return Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
        ));
    }

    enforce_pre_execution_policy(
        state_ro.pipeline().ivm_max_cycles_upper_bound,
        &meta,
        summary.code_offset,
        bytecode.as_ref(),
    )?;
    validate_contract_binding(state_ro, tx, &summary)?;
    // Proved executions do not support implicit manifest registration append.
    enforce_manifest_is_pre_registered(state_ro, tx, summary.code_hash)?;

    let gas_schedule_id = vk_record.gas_schedule_id.as_deref().ok_or_else(|| {
        OverlayBuildError::ZkProof("verifying key missing gas_schedule_id".to_owned())
    })?;

    let amx_analysis = ivm_cache
        .analyze_program(&summary, bytecode.as_ref())
        .map_err(map_program_analysis_error)?;
    let (contract_call_context, contract_runtime_context, entrypoint_authorization) =
        authorize_and_prepare_raw_contract_dispatch(state_ro, tx, &summary, gas_limit)?;
    let mut vm = summary
        .checkout_runtime(gas_limit)
        .map_err(OverlayBuildError::IvmLoad)?;
    vm.set_zk_trace_enabled(true);

    let accounts = state_ro.accounts_snapshot();
    let streaming_meta = resolve_streaming_metadata(state_ro, tx.authority());
    let mut host =
        crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts_and_argument_record(
            tx.authority().clone(),
            Arc::clone(&accounts),
            contract_call_context.argument_record.clone(),
        );
    host.set_prepared_contract_cache(summary.prepared_contract_cache());
    host.set_amx_analysis(amx_analysis);
    let amx_limits =
        crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(state_ro.pipeline());
    host.set_amx_limits(amx_limits);
    host.set_axt_timing(state_ro.nexus().axt);
    host.hydrate_axt_replay_ledger(state_ro);
    host.set_public_inputs_from_parameters(state_ro.world().parameters());
    host.set_vrf_epoch_seeds_from_world(state_ro.world());
    host.set_query_state(state_ro);
    host.set_contract_runtime_context(Some(contract_runtime_context.clone()));
    host.set_contract_entrypoint_authorization(Some(entrypoint_authorization.clone()));
    host.set_bound_contract_records_by_subject_snapshot(
        code::snapshot_bound_contract_records_by_subject(state_ro),
    );
    let snapshot = state_ro.axt_policy_snapshot();
    host = host.with_axt_policy_snapshot(&snapshot);
    apply_streaming_metadata(&mut host, streaming_meta);
    #[cfg(feature = "telemetry")]
    host.set_telemetry(state_ro.metrics().clone());
    host.set_crypto_config(state_ro.crypto());
    host.set_zk_config(state_ro.zk());
    host.set_chain_id(state_ro.chain_id());
    host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
        .map_err(OverlayBuildError::IvmRun)?;

    vm.set_gas_limit(gas_limit);
    vm.set_zk_trace_enabled(true);
    apply_contract_call_execution_context(&mut vm, Some(&contract_call_context))?;
    run_vm_with_host(&mut vm, &mut host)?;

    let gas_used = gas_limit.saturating_sub(vm.remaining_gas());
    let trace_bundle = build_ivm_trace_bundle(&vm);
    let trace_hash = expected_ivm_trace_hash(&trace_bundle)?;

    let queued = host.drain_queued_instructions_with_contract_runtime_context(Some(
        contract_runtime_context.clone(),
    ));
    let (durable_state_overlay, durable_state_authorizations) =
        host.drain_durable_state_overlay_with_authorizations();
    if !durable_state_overlay.is_empty() {
        return Err(OverlayBuildError::ZkProof(
            "proved payload derivation cannot encode durable StateMap writes in ABI V1".to_owned(),
        ));
    }
    debug_assert!(durable_state_authorizations.is_empty());
    if queued.iter().any(|queued| {
        queued.authority != *tx.authority()
            || queued.entrypoint_authorization.as_ref() != Some(&entrypoint_authorization)
            || queued
                .contract_runtime_context
                .as_ref()
                .is_none_or(|context| {
                    context.contract_subject != contract_runtime_context.contract_subject
                        || context.contract_address != contract_runtime_context.contract_address
                        || context.contract_alias != contract_runtime_context.contract_alias
                        || context.entrypoint != contract_runtime_context.entrypoint
                })
    }) {
        return Err(OverlayBuildError::ZkProof(
            "proved payload derivation ABI V1 can encode only exact top-level authorization for queued host writes; nested or mismatched contexts are forbidden"
                .to_owned(),
        ));
    }
    let mut queued = queued
        .into_iter()
        .map(|queued| queued.instruction)
        .collect::<Vec<_>>();
    prune_redundant_contract_ops(state_ro, &mut queued);
    let overlay: iroha_primitives::const_vec::ConstVec<InstructionBox> = queued.into();

    let overlay_hash = {
        let bytes = norito::to_bytes(&overlay).map_err(|_| {
            OverlayBuildError::ZkProof("failed to encode proved overlay".to_owned())
        })?;
        Hash::new(&bytes)
    };

    let events_commitment =
        expected_ivm_events_commitment(summary.code_hash, overlay_hash, trace_hash);
    let gas_policy_commitment = expected_ivm_gas_policy_commitment(
        summary.code_hash,
        overlay_hash,
        &vk_record.circuit_id,
        vk_record.version,
        gas_schedule_id,
        gas_limit,
        gas_used,
        trace_hash,
    );

    Ok(iroha_data_model::transaction::IvmProved {
        bytecode,
        overlay,
        events_commitment,
        gas_policy_commitment,
    })
}
