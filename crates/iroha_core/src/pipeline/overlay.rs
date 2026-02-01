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
use std::{collections::BTreeMap, sync::Arc};
#[cfg(test)]
use std::{
    collections::VecDeque,
    sync::{LazyLock, Mutex},
};

use iroha_config::parameters::actual::QueryCursorMode;
use iroha_crypto::{Hash, streaming::TransportCapabilityResolutionSnapshot};
use iroha_data_model::{
    block::BlockHeader,
    errors::CanonicalErrorKind,
    executor::IvmAdmissionError,
    executor::{
        ManifestAbiHashMismatchInfo, ManifestCodeHashMismatchInfo, MaxCyclesExceedsUpperBoundInfo,
    },
    isi::{
        InstructionBox,
        settlement::{DvpIsi, PvpIsi},
        smart_contract_code::{
            ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
        },
    },
    name::Name,
    nexus::AxtRejectContext,
    prelude::{AccountId, ValidationFail},
    smart_contract::manifest::{ContractManifest, MANIFEST_METADATA_KEY},
    transaction::{Executable, SignedTransaction},
};
use ivm::{VMError as IvmError, analysis::ProgramAnalysisError};
use mv::storage::StorageReadOnly;
use norito::{codec::Encode as NoritoEncode, streaming::CapabilityFlags};

use crate::{
    executor::{
        ensure_asset_definition_registration_allowed, extract_register_asset_definition,
        parse_gas_limit,
    },
    smartcontracts::{
        code,
        isi::settlement::{admission_validate_dvp, admission_validate_pvp},
        ivm::{
            cache::ProgramSummary,
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

fn default_pipeline_config() -> iroha_config::parameters::actual::Pipeline {
    use iroha_config::parameters::{actual, defaults};

    actual::Pipeline {
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
    header_len: usize,
    bytecode: &[u8],
) -> (Hash, Hash) {
    let code_hash = Hash::new(&bytecode[header_len..]);
    debug_assert_eq!(meta.abi_version, 1, "only ABI v1 is supported");
    let policy = ivm::SyscallPolicy::AbiV1;
    let computed = Hash::prehashed(ivm::syscalls::compute_abi_hash(policy));
    let abi_hash = PROGRAM_HASH_CACHE
        .lock()
        .expect("program hash cache poisoned")
        .get_or_insert(code_hash, computed);
    (code_hash, abi_hash)
}

const PREEXEC_OPCODE_DENYLIST: &[u8] = &[ivm::instruction::wide::system::SYSTEM];

fn enforce_pre_execution_policy(
    ivm_max_cycles_upper_bound: u64,
    meta: &ivm::ProgramMetadata,
    code_offset: usize,
    bytecode: &[u8],
) -> Result<(), OverlayBuildError> {
    let provided_cycles = meta.max_cycles;
    let upper = ivm_max_cycles_upper_bound;
    if upper > 0 && provided_cycles > upper {
        return Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::MaxCyclesExceedsUpperBound(MaxCyclesExceedsUpperBoundInfo {
                max_cycles: provided_cycles,
                upper_bound: upper,
            }),
        ));
    }

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

fn validate_contract_binding<R: StateReadOnly>(
    state_ro: &R,
    tx: &SignedTransaction,
    summary: &ProgramSummary,
) -> Result<(), OverlayBuildError> {
    let code_hash = summary.code_hash;
    let abi_hash = summary.abi_hash;
    let namespace = tx
        .metadata()
        .get(&Name::from_str("contract_namespace").expect("static name"))
        .and_then(|value| value.clone().try_into_any_norito::<String>().ok());
    let contract_id = tx
        .metadata()
        .get(&Name::from_str("contract_id").expect("static name"))
        .and_then(|value| value.clone().try_into_any_norito::<String>().ok());

    let artifacts = code::fetch_artifacts(
        state_ro,
        &code_hash,
        namespace.as_deref().zip(contract_id.as_deref()),
    );
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

    // If namespace/contract_id metadata is present, ensure the instance binding matches.
    if let (Some(namespace), Some(contract_id)) = (namespace, contract_id) {
        let bound_hash = artifacts.bound_code_hash.ok_or_else(|| {
            OverlayBuildError::HeaderPolicy(IvmAdmissionError::BytecodeDecodingFailed(format!(
                "contract instance `{namespace}/{contract_id}` not found in WSV"
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

fn prune_redundant_contract_ops<R: StateReadOnly>(state_ro: &R, queued: &mut Vec<InstructionBox>) {
    if queued.is_empty() {
        return;
    }
    let mut manifest_cache: BTreeMap<Hash, Option<ContractManifest>> = BTreeMap::new();
    let mut code_cache: BTreeMap<Hash, Option<Vec<u8>>> = BTreeMap::new();
    let mut binding_cache: BTreeMap<(String, String), Option<Hash>> = BTreeMap::new();
    queued.retain(|instr| {
        if let Some(reg) = instr.as_any().downcast_ref::<RegisterSmartContractCode>() {
            if let Some(hash) = reg.manifest().code_hash {
                let existing = manifest_cache
                    .entry(hash)
                    .or_insert_with(|| state_ro.world().contract_manifests().get(&hash).cloned());
                if let Some(existing) = existing {
                    if existing == reg.manifest() {
                        return false;
                    }
                }
            }
        } else if let Some(bytes) = instr.as_any().downcast_ref::<RegisterSmartContractBytes>() {
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
        } else if let Some(activate) = instr.as_any().downcast_ref::<ActivateContractInstance>() {
            let key = (activate.namespace().clone(), activate.contract_id().clone());
            let bound = binding_cache
                .entry(key.clone())
                .or_insert_with(|| state_ro.world().contract_instances().get(&key).copied());
            if bound.is_some_and(|hash| hash == *activate.code_hash()) {
                return false;
            }
        }
        true
    });
}

/// Overlay of a transaction's intended operations.
#[derive(Debug, Clone, Default)]
pub struct TxOverlay {
    instructions: Vec<InstructionBox>,
}

impl TxOverlay {
    /// Create an overlay from a list of instructions.
    pub fn from_instructions(instrs: Vec<InstructionBox>) -> Self {
        Self {
            instructions: instrs,
        }
    }

    /// Is this overlay empty?
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Number of instructions in this overlay.
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Iterate over instructions in this overlay.
    pub fn instructions(&self) -> impl ExactSizeIterator<Item = &InstructionBox> {
        self.instructions.iter()
    }

    /// Approximate byte size of this overlay when serialized via Norito TLV.
    pub fn byte_size(&self) -> usize {
        self.instructions
            .iter()
            .map(|i| NoritoEncode::encode(i).len())
            .sum()
    }

    /// Apply the overlay to the given state transaction via the runtime executor.
    /// Executes instructions in chunks to bound peak working memory.
    /// Apply the overlay to the given state transaction via the runtime executor.
    ///
    /// # Errors
    /// Returns an error if executing any instruction fails validation or the executor rejects it.
    pub fn apply(
        &self,
        state_tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<(), ValidationFail> {
        let executor = state_tx.world.executor.clone();
        // Execute instructions directly; avoid registry-based roundtrips.
        for chunk_instrs in self.instructions.chunks(self.instructions.len().max(1)) {
            for instr in chunk_instrs {
                if let Some(dvp) = instr.as_any().downcast_ref::<DvpIsi>() {
                    admission_validate_dvp(authority, state_tx, dvp)
                        .map_err(ValidationFail::from)?;
                } else if let Some(pvp) = instr.as_any().downcast_ref::<PvpIsi>() {
                    admission_validate_pvp(authority, state_tx, pvp)
                        .map_err(ValidationFail::from)?;
                }
                if let Some(reg_asset_definition) = extract_register_asset_definition(instr) {
                    ensure_asset_definition_registration_allowed(
                        state_tx,
                        authority,
                        &reg_asset_definition,
                    )?;
                }
                executor.execute_instruction(state_tx, authority, instr.clone())?;
            }
        }
        Ok(())
    }

    /// Apply the overlay with a specific chunk size (number of instructions per chunk).
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
        let executor = state_tx.world.executor.clone();
        let chunk = chunk_size.max(1);
        for chunk_instrs in self.instructions.chunks(chunk) {
            for instr in chunk_instrs {
                if let Some(dvp) = instr.as_any().downcast_ref::<DvpIsi>() {
                    admission_validate_dvp(authority, state_tx, dvp)
                        .map_err(ValidationFail::from)?;
                } else if let Some(pvp) = instr.as_any().downcast_ref::<PvpIsi>() {
                    admission_validate_pvp(authority, state_tx, pvp)
                        .map_err(ValidationFail::from)?;
                }
                if let Some(reg_asset_definition) = extract_register_asset_definition(instr) {
                    ensure_asset_definition_registration_allowed(
                        state_tx,
                        authority,
                        &reg_asset_definition,
                    )?;
                }
                executor.execute_instruction(state_tx, authority, instr.clone())?;
            }
        }
        Ok(())
    }
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
            if wants_zk && !state_ro.zk().halo2.enabled {
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

            let mut vm = ivm_cache
                .clone_runtime(&summary, bytecode.as_ref(), gas_limit)
                .map_err(OverlayBuildError::IvmLoad)?;

            // Run CoreHost to collect queued ISIs
            // Snapshot of accounts for deterministic helpers
            let accounts = Arc::new(
                state_ro
                    .world()
                    .accounts_iter()
                    .map(|e| e.id.clone())
                    .collect::<Vec<_>>(),
            );
            let streaming_meta = resolve_streaming_metadata(state_ro, tx.authority());
            let mut host = crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts(
                tx.authority().clone(),
                Arc::clone(&accounts),
            );
            let amx_analysis =
                ivm::analysis::analyze_program(bytecode.as_ref()).map_err(|err| match err {
                    ProgramAnalysisError::Metadata(_) => OverlayBuildError::IvmHeaderParse,
                    ProgramAnalysisError::Decode(decode_err) => {
                        OverlayBuildError::IvmLoad(decode_err)
                    }
                })?;
            host.set_amx_analysis(amx_analysis);
            let amx_limits = crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                state_ro.pipeline(),
            );
            host.set_amx_limits(amx_limits);
            host.set_axt_timing(state_ro.nexus().axt);
            host.hydrate_axt_replay_ledger(state_ro);
            host.set_durable_state_snapshot_from_world(state_ro.world());
            host.set_public_inputs_from_parameters(state_ro.world().parameters());
            host.set_query_state(state_ro);
            let snapshot = state_ro.axt_policy_snapshot();
            host = host.with_axt_policy_snapshot(&snapshot);
            apply_streaming_metadata(&mut host, streaming_meta);
            #[cfg(feature = "telemetry")]
            host.set_telemetry(state_ro.metrics().clone());
            host.set_crypto_config(state_ro.crypto());
            host.set_halo2_config(&state_ro.zk().halo2);
            host.set_chain_id(state_ro.chain_id());
            host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
                .map_err(OverlayBuildError::IvmRun)?;
            vm.set_gas_limit(gas_limit);
            run_vm_with_host(&mut vm, &mut host)?;
            let transport_caps_snapshot = host.transport_caps_snapshot().copied();
            let negotiated_caps_snapshot = host.negotiated_caps_snapshot().copied();
            let mut queued = host.drain_instructions();
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
                        program: Arc::new(bytecode.as_ref().to_vec()),
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

            prune_redundant_contract_ops(state_ro, &mut queued);
            // If transaction metadata carries a ContractManifest, append a registration ISI
            // when the manifest isn't present in WSV yet.
            if let Some(json) = tx
                .metadata()
                .get(&iroha_data_model::name::Name::from_str(MANIFEST_METADATA_KEY).unwrap())
                && let Ok(manifest) = json.clone().try_into_any_norito::<ContractManifest>()
            {
                // Compute code hash from program body
                let code_hash = summary.code_hash;
                // Persist only if not present
                if state_ro
                    .world()
                    .contract_manifests()
                    .get(&code_hash)
                    .is_none()
                {
                    let isi = RegisterSmartContractCode { manifest };
                    queued.push(InstructionBox::from(isi));
                }
            }
            Ok(TxOverlay::from_instructions(queued))
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
            let mut vm = ivm::IVM::new(tx_gas_limit);
            let mut host = crate::smartcontracts::ivm::host::CoreHost::with_accounts(
                tx.authority().clone(),
                Arc::new(accounts.to_vec()),
            );
            apply_streaming_metadata(&mut host, StreamingOverlayMetadata::default());
            vm.set_host(host);
            vm.load_program(bytecode.as_ref())
                .map_err(OverlayBuildError::IvmLoad)?;
            vm.set_gas_limit(tx_gas_limit);
            run_vm(&mut vm)?;
            let mut queued = if let Some(h) = vm.host_mut_any()
                && let Some(host) = h.downcast_mut::<crate::smartcontracts::ivm::host::CoreHost>()
            {
                host.drain_instructions()
            } else {
                Vec::new()
            };
            // Append manifest registration if attached in metadata
            if let Some(json) = tx
                .metadata()
                .get(&iroha_data_model::name::Name::from_str(MANIFEST_METADATA_KEY).unwrap())
                && let Ok(manifest) = json.clone().try_into_any_norito::<ContractManifest>()
            {
                queued.push(InstructionBox::from(RegisterSmartContractCode { manifest }));
            }
            Ok(TxOverlay::from_instructions(queued))
        }
    }
}

/// Build an overlay for a transaction using a pre-captured accounts snapshot, and
/// optionally emit a ZK-lane verification task with the given block header.
/// Build an overlay for a signed transaction, with ZK mode hint and block header context.
///
/// # Errors
/// Returns an error if the IVM header fails policy checks or running the VM fails.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_overlay_for_transaction_with_accounts_zk<R>(
    tx: &SignedTransaction,
    accounts: &[AccountId],
    state_ro: &R,
    zk_enabled: bool,
    header: &BlockHeader,
    streaming_meta: StreamingOverlayMetadata,
) -> Result<TxOverlay, OverlayBuildError>
where
    R: StateReadOnly + QueryStateSource,
{
    match tx.instructions() {
        Executable::Instructions(batch) => {
            let instrs: Vec<InstructionBox> = batch.iter().cloned().collect();
            Ok(TxOverlay::from_instructions(instrs))
        }
        Executable::Ivm(bytecode) => {
            let parsed = ivm::ProgramMetadata::parse(bytecode.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let meta = parsed.metadata;
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            let code_offset = parsed.code_offset;
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
            let tx_gas_limit = require_tx_gas_limit(tx)?;
            let mut vm = ivm::IVM::new(tx_gas_limit);
            let mut host = crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts(
                tx.authority().clone(),
                Arc::new(accounts.to_vec()),
            );
            let amx_analysis =
                ivm::analysis::analyze_program(bytecode.as_ref()).map_err(|err| match err {
                    ProgramAnalysisError::Metadata(_) => OverlayBuildError::IvmHeaderParse,
                    ProgramAnalysisError::Decode(decode_err) => {
                        OverlayBuildError::IvmLoad(decode_err)
                    }
                })?;
            host.set_amx_analysis(amx_analysis);
            let amx_limits = crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                state_ro.pipeline(),
            );
            host.set_amx_limits(amx_limits);
            host.set_axt_timing(state_ro.nexus().axt);
            host.hydrate_axt_replay_ledger(state_ro);
            host.set_durable_state_snapshot_from_world(state_ro.world());
            host.set_public_inputs_from_parameters(state_ro.world().parameters());
            host.set_query_state(state_ro);
            let snapshot = state_ro.axt_policy_snapshot();
            host = host.with_axt_policy_snapshot(&snapshot);
            apply_streaming_metadata(&mut host, streaming_meta);
            #[cfg(feature = "telemetry")]
            host.set_telemetry(state_ro.metrics().clone());
            host.set_crypto_config(state_ro.crypto());
            host.set_halo2_config(&state_ro.zk().halo2);
            host.set_chain_id(state_ro.chain_id());
            host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
                .map_err(OverlayBuildError::IvmRun)?;
            vm.load_program(bytecode.as_ref())
                .map_err(OverlayBuildError::IvmLoad)?;
            vm.set_gas_limit(tx_gas_limit);
            run_vm_with_host(&mut vm, &mut host)?;
            let transport_caps_snapshot = host.transport_caps_snapshot().copied();
            let negotiated_caps_snapshot = host.negotiated_caps_snapshot().copied();
            let mut queued = host.drain_instructions();
            if zk_enabled && vm.zk_mode_enabled() {
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
                        program: Arc::new(bytecode.as_ref().to_vec()),
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
            if let Some(json) = tx
                .metadata()
                .get(&iroha_data_model::name::Name::from_str(MANIFEST_METADATA_KEY).unwrap())
                && let Ok(manifest) = json.clone().try_into_any_norito::<ContractManifest>()
            {
                let code_hash = iroha_crypto::Hash::new(&bytecode.as_ref()[parsed.header_len..]);
                queued.push(InstructionBox::from(RegisterSmartContractCode { manifest }));
                let _ = code_hash;
            }
            prune_redundant_contract_ops(state_ro, &mut queued);
            Ok(TxOverlay::from_instructions(queued))
        }
    }
}

/// Build an overlay for a transaction under quarantine limits.
///
/// Applies per-transaction execution caps when running IVM bytecode to collect queued ISIs:
/// - `max_cycles_cap`: if non-zero, caps VM cycles to `min(header.max_cycles, max_cycles_cap, upper_bound_cap)`.
/// - `max_millis_cap`: if non-zero, rejects the overlay if VM execution exceeds the wall-clock budget.
/// - `upper_bound_cap`: pipeline-wide upper bound on cycles; 0 means no additional cap.
///   Build an overlay for a transaction under quarantine caps (cycles/millis and upper bound).
///
/// # Errors
/// Returns an error if the IVM header fails policy checks or running the VM fails.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_overlay_for_transaction_quarantine(
    tx: &SignedTransaction,
    accounts: &[AccountId],
    max_cycles_cap: u64,
    max_millis_cap: u64,
    upper_bound_cap: u64,
    streaming_meta: StreamingOverlayMetadata,
) -> Result<TxOverlay, OverlayBuildError> {
    match tx.instructions() {
        Executable::Instructions(batch) => {
            // Built-in instruction batches do not use VM; return overlay directly.
            let instrs: Vec<InstructionBox> = batch.iter().cloned().collect();
            Ok(TxOverlay::from_instructions(instrs))
        }
        Executable::Ivm(bytecode) => {
            let parsed = ivm::ProgramMetadata::parse(bytecode.as_ref())
                .map_err(|_| OverlayBuildError::IvmHeaderParse)?;
            let meta = parsed.metadata;
            validate_header_policy(&meta).map_err(OverlayBuildError::HeaderPolicy)?;
            if meta.mode & ivm::ivm_mode::ZK != 0 {
                return Err(OverlayBuildError::HeaderPolicy(
                    IvmAdmissionError::UnsupportedFeatureBits(ivm::ivm_mode::ZK),
                ));
            }
            let tx_gas_limit = require_tx_gas_limit(tx)?;
            let mut eff = meta.max_cycles;
            if eff == 0 {
                eff = u64::MAX;
            }
            if upper_bound_cap > 0 {
                eff = eff.min(upper_bound_cap);
            }
            if max_cycles_cap > 0 {
                eff = eff.min(max_cycles_cap);
            }
            if eff == u64::MAX {
                eff = 0; // no cap
            }
            let mut vm = ivm::IVM::new(tx_gas_limit);
            let mut host = crate::smartcontracts::ivm::host::CoreHost::with_accounts(
                tx.authority().clone(),
                Arc::new(accounts.to_vec()),
            );
            apply_streaming_metadata(&mut host, streaming_meta);
            vm.set_host(host);
            vm.load_program(bytecode.as_ref())
                .map_err(OverlayBuildError::IvmLoad)?;
            if eff > 0 {
                vm.set_max_cycles(eff);
            }
            vm.set_gas_limit(tx_gas_limit);
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
            let queued = if let Some(h) = vm.host_mut_any()
                && let Some(host) = h.downcast_mut::<crate::smartcontracts::ivm::host::CoreHost>()
            {
                host.drain_instructions()
            } else {
                Vec::new()
            };
            Ok(TxOverlay::from_instructions(queued))
        }
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

    const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";

    fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version,
        };
        let mut v = meta.encode();
        v.extend_from_slice(&LITERAL_SECTION_MAGIC);
        v.extend_from_slice(&0u32.to_le_bytes()); // literal entries
        v.extend_from_slice(&0u32.to_le_bytes()); // post-pad
        v.extend_from_slice(&0u32.to_le_bytes()); // literal size
        v.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        v
    }

    #[test]
    fn overlay_appends_manifest_only_when_missing() {
        // Build state with a domain/account and optionally pre-seeded manifest
        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = crate::state::World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("chain"));

        // Create a minimal program and its hashes
        let prog = minimal_ivm_program(1);
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);

        // Build a manifest JSON and attach to tx metadata
        let manifest = iroha_data_model::smart_contract::manifest::ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);
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
            1,
            "expected one registration ISI"
        );

        // Seed manifest into WSV
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
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
fn validate_header_policy(meta: &ivm::ProgramMetadata) -> Result<(), IvmAdmissionError> {
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
    use iroha_data_model::{
        ChainId, Registrable, isi::smart_contract_code::RemoveSmartContractBytes,
    };

    use super::*;

    #[test]
    fn empty_overlay_is_noop() {
        let ovl = TxOverlay::default();
        assert!(ovl.is_empty());
    }

    #[test]
    fn overlay_rejects_ivm_without_gas_limit() {
        use iroha_crypto::KeyPair;
        use iroha_data_model::{
            account::Account,
            domain::Domain,
            prelude::{AccountId, IvmBytecode, TransactionBuilder},
        };

        let (program, _header_len, _meta) = sample_program();
        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );
        let domain = Domain::new("wonderland".parse().unwrap()).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
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

    fn sample_program() -> (Vec<u8>, usize, ivm::ProgramMetadata) {
        let meta = ivm::ProgramMetadata {
            max_cycles: 1,
            ..ivm::ProgramMetadata::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let parsed = ivm::ProgramMetadata::parse(&program).expect("parse sample program");
        (program, parsed.header_len, parsed.metadata)
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
        use std::sync::Arc;

        use iroha_crypto::KeyPair;
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, TransactionBuilder},
        };
        use iroha_primitives::json::Json;

        let (program, header_len, meta) = sample_program();
        let (code_hash, abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let namespace = "apps".to_string();
        let contract_id = "calc".to_string();
        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );

        // Inject a manifest with a mismatched abi_hash into WSV plus the instance binding.
        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert((namespace.clone(), contract_id.clone()), code_hash);
        let mut wrong_bytes = [0u8; 32];
        wrong_bytes.copy_from_slice(abi_hash.as_ref());
        wrong_bytes[0] ^= 0xFF;
        let wrong_abi_hash = Hash::prehashed(wrong_bytes);
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: Some(wrong_abi_hash),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                kotoba: None,
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
            Name::from_str("contract_namespace").expect("static name"),
            Json::new(namespace.clone()),
        );
        metadata.insert(
            Name::from_str("contract_id").expect("static name"),
            Json::new(contract_id.clone()),
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
    #[allow(clippy::too_many_lines)]
    fn overlay_rejects_axt_without_policy_entries() {
        use std::sync::Arc;

        use iroha_crypto::KeyPair;
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

        const LITERAL_HEADER_LEN: usize = 16;
        const POINTER_TABLE_LEN: usize = 32;

        let dsid = DataSpaceId::new(7);
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("binding");
        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );
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
                to: "bob@wonderland".into(),
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
        use std::sync::Arc;

        use iroha_crypto::KeyPair;
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, TransactionBuilder},
        };
        use iroha_primitives::json::Json;

        let (program, header_len, meta) = sample_program();
        let (code_hash, abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let namespace = "apps".to_string();
        let contract_id = "calc".to_string();
        let wrong_binding = Hash::new(b"other-binding");
        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );

        // Insert a manifest for the actual code, but bind the namespace to a different code hash.
        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert((namespace.clone(), contract_id.clone()), wrong_binding);
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: Some(abi_hash),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                kotoba: None,
                provenance: None,
            }
            .signed(&kp),
        );
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_namespace").expect("static name"),
            Json::new(namespace.clone()),
        );
        metadata.insert(
            Name::from_str("contract_id").expect("static name"),
            Json::new(contract_id.clone()),
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
        use std::sync::Arc;

        use iroha_crypto::KeyPair;
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, TransactionBuilder},
        };
        use iroha_primitives::json::Json;

        let (program, header_len, meta) = sample_program();
        let (code_hash, _abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let namespace = "apps".to_string();
        let contract_id = "calc".to_string();
        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );

        // Bind namespace to code hash but do not seed manifest in WSV.
        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert((namespace.clone(), contract_id.clone()), code_hash);
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_namespace").expect("static name"),
            Json::new(namespace.clone()),
        );
        metadata.insert(
            Name::from_str("contract_id").expect("static name"),
            Json::new(contract_id.clone()),
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
        use std::sync::Arc;

        use iroha_crypto::KeyPair;
        use iroha_data_model::{
            metadata::Metadata,
            prelude::{AccountId, TransactionBuilder},
        };
        use iroha_primitives::json::Json;

        let (program, header_len, meta) = sample_program();
        let (code_hash, _abi_hash) = super::compute_program_hashes(&meta, header_len, &program);

        let namespace = "apps".to_string();
        let contract_id = "calc".to_string();
        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );

        let mut world = crate::state::World::default();
        world
            .contract_instances
            .insert((namespace.clone(), contract_id.clone()), code_hash);
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: None,
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                kotoba: None,
                provenance: None,
            }
            .signed(&kp),
        );
        let kura = Arc::new(crate::kura::Kura::blank_kura_for_testing());
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = crate::state::State::new_for_testing(world, Arc::clone(&kura), query);

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_namespace").expect("static name"),
            Json::new(namespace.clone()),
        );
        metadata.insert(
            Name::from_str("contract_id").expect("static name"),
            Json::new(contract_id.clone()),
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
            .insert((namespace.clone(), contract_id.clone()), code_hash);
        world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: Some(Hash::prehashed([0u8; 32])),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                kotoba: None,
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
    fn pre_execution_policy_denies_system_opcode() {
        use std::sync::Arc;

        use iroha_crypto::KeyPair;
        use iroha_data_model::prelude::{AccountId, TransactionBuilder};

        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap()).build(&authority);
        let account = iroha_data_model::account::Account::new(authority.clone()).build(&authority);
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
            &ivm::encoding::wide::encode_sys(ivm::instruction::wide::system::SYSTEM, 0)
                .to_le_bytes(),
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
        assert!(matches!(
            res,
            Err(OverlayBuildError::HeaderPolicy(
                IvmAdmissionError::BytecodeDecodingFailed(msg)
            )) if msg.contains("denied by pre-execution policy")
        ));
    }

    #[test]
    fn pre_execution_policy_ignores_literal_table() {
        use std::sync::Arc;

        use iroha_crypto::KeyPair;
        use iroha_data_model::prelude::{AccountId, TransactionBuilder};

        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap()).build(&authority);
        let account = iroha_data_model::account::Account::new(authority.clone()).build(&authority);
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
            code_hash: Some(code_hash),
            abi_hash: Some(abi_hash),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        };
        world.contract_manifests.insert(code_hash, manifest.clone());
        world.contract_code.insert(code_hash, program.clone());
        world
            .contract_instances
            .insert(("apps".to_string(), "calc".to_string()), code_hash);
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
                namespace: "apps".to_string(),
                contract_id: "calc".to_string(),
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
            let target_pc = vm.pc.saturating_sub(parsed.literal_prefix_len() as u64);
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
}

impl core::fmt::Display for OverlayBuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OverlayBuildError::IvmHeaderParse => write!(f, "IVM header parse error"),
            OverlayBuildError::HeaderPolicy(e) => write!(f, "header policy: {e:?}"),
            OverlayBuildError::GasLimit(msg) => write!(f, "{msg}"),
            OverlayBuildError::IvmLoad(e) => write!(f, "ivm.load_program: {e}"),
            OverlayBuildError::IvmRun(e) => write!(f, "ivm.run: {e}"),
            OverlayBuildError::AxtReject(ctx) => write!(f, "axt_reject: {ctx}"),
            OverlayBuildError::AmxBudgetViolation(v) => write!(f, "{}", amx_timeout_message(v)),
            OverlayBuildError::QuarantineOverflow => write!(f, "quarantine overflow"),
        }
    }
}
