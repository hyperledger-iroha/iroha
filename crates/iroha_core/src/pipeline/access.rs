//! Access-set derivation for transactions and instructions.
//!
//! Produces deterministic read/write key sets to feed the conflict-aware
//! scheduler described in `new_pipeline.md`.

use core::fmt::Write as _;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
};

use iroha_crypto::Hash as IrohaHash;
// ZK ISIs live in the data model; import the module for pattern matches
use iroha_data_model::isi::ExecuteTrigger;
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{
        BurnBox, GrantBox, InstructionBox, Log, MintBox, RegisterBox, RemoveKeyValueBox, RevokeBox,
        SetKeyValueBox, TransferBox, UnregisterBox, zk,
    },
    metadata::Metadata,
    nexus::LaneId,
    nft::NftId,
    permission,
    prelude::*,
    role::RoleId,
    rwa::RwaId,
    smart_contract::manifest::{
        ContractManifest, DynamicAccessHint, EntrypointDescriptor, MANIFEST_METADATA_KEY,
    },
    state::{
        AccountMetadataKey, AccountRoleKey, AssetDefinitionMetadataKey, AssetMetadataKey,
        CanonicalStateKey, DomainMetadataKey, NftMetadataKey, RwaMetadataKey,
        StateAccessSetAdvisory, TriggerMetadataKey, TxQueueKey,
    },
    transaction::{SignedTransaction, executable::ContractInvocation},
};
use ivm::host::IVMHost;
use mv::storage::StorageReadOnly; // bring trait into scope for .get()
use parking_lot::RwLock;

use crate::{
    executor::parse_gas_limit,
    smartcontracts::triggers::set::{ExecutableRef, SetReadOnly},
    smartcontracts::{code, ivm::host::QueryStateSource},
    state::{StateReadOnly, WorldReadOnly},
};

/// Canonical string key used for conflict detection (Norito-like ordering).
///
/// Keys are generated deterministically from data model identifiers such as
/// `AccountId`, `DomainId`, `AssetDefinitionId`, `AssetId`, `NftId`, and `RwaId`.
pub type AccessKey = String;

const AUTHORITY_ACCOUNT_KEY: &str = "account:$authority";
/// Synthetic scheduler epoch covering every change that can affect a named
/// entrypoint permission check (direct grants, role bindings, and role grants).
const AUTHORIZATION_EPOCH_KEY: &str = "authorization:*";
const ACCOUNT_WILDCARD_KEY: &str = "account:*";
const ASSET_WILDCARD_KEY: &str = "asset:*";
const ASSET_DEF_WILDCARD_KEY: &str = "asset_def:*";
const NEXUS_ACTIVE_LANE_CATALOG_KEY: &str = "nexus.active_lane_catalog";
const SCCP_ON_CHAIN_REGISTRY_KEY: &str = "parameter.custom:sccp_registry_v1";

/// Access set with separate read and write collections.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AccessSet {
    /// Set of keys read by a transaction or instruction batch.
    pub read_keys: BTreeSet<AccessKey>,
    /// Set of keys written by a transaction or instruction batch.
    pub write_keys: BTreeSet<AccessKey>,
}

impl AccessSet {
    /// Create an empty access set.
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a single read key.
    pub fn add_read(&mut self, k: AccessKey) {
        self.read_keys.insert(k);
    }
    /// Add a single write key.
    pub fn add_write(&mut self, k: AccessKey) {
        self.write_keys.insert(k);
    }
    /// Merge another access set into this one.
    pub fn union_with(&mut self, other: AccessSet) {
        self.read_keys.extend(other.read_keys);
        self.write_keys.extend(other.write_keys);
    }
    /// Conservative set that conflicts with everything (serializes the tx).
    pub fn global() -> Self {
        let mut s = Self::new();
        s.add_write("*".to_string());
        s
    }
}

/// Origin of an IVM access set used by the scheduler.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum AccessSetSource {
    /// Derived from manifest-level `access_set_hints`.
    ManifestHints,
    /// Derived from entrypoint-level hints on the manifest.
    EntrypointHints,
    /// Derived from a dynamic prepass that merged ISI targets and state access logs.
    PrepassMerge,
    /// Conservative fallback (global conflicts).
    ConservativeFallback,
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct AccessSetCacheKey {
    code_hash: IrohaHash,
    entrypoint: Option<String>,
}

struct AccessSetCacheEntry {
    manifest_hash: IrohaHash,
    set: AccessSet,
}

fn access_set_cache() -> &'static RwLock<BTreeMap<AccessSetCacheKey, AccessSetCacheEntry>> {
    static ACCESS_SET_CACHE: OnceLock<RwLock<BTreeMap<AccessSetCacheKey, AccessSetCacheEntry>>> =
        OnceLock::new();
    ACCESS_SET_CACHE.get_or_init(|| RwLock::new(BTreeMap::new()))
}

fn access_set_cache_get(key: &AccessSetCacheKey, manifest_hash: &IrohaHash) -> Option<AccessSet> {
    let cache = access_set_cache();
    {
        let guard = cache.read();
        if let Some(entry) = guard.get(key) {
            if entry.manifest_hash == *manifest_hash {
                return Some(entry.set.clone());
            }
        } else {
            return None;
        }
    }
    let mut guard = cache.write();
    if let Some(entry) = guard.get(key) {
        if entry.manifest_hash == *manifest_hash {
            return Some(entry.set.clone());
        }
        guard.remove(key);
    }
    None
}

fn access_set_cache_put(key: AccessSetCacheKey, manifest_hash: IrohaHash, set: AccessSet) {
    let mut guard = access_set_cache().write();
    guard.insert(key, AccessSetCacheEntry { manifest_hash, set });
}

#[cfg(test)]
fn access_set_cache_clear() {
    access_set_cache().write().clear();
}

fn manifest_signature_hash(manifest: &ContractManifest) -> IrohaHash {
    IrohaHash::new(manifest.signature_payload_bytes())
}

fn manifest_from_metadata(tx: &SignedTransaction) -> Option<ContractManifest> {
    let key: Name = MANIFEST_METADATA_KEY.parse().ok()?;
    tx.metadata()
        .get(&key)
        .and_then(|json| json.clone().try_into_any_norito::<ContractManifest>().ok())
}

#[derive(Clone, Debug)]
struct ContractCallExecutionContext {
    entrypoint: Option<String>,
    entrypoint_pc: Option<u64>,
    entrypoint_permission: Option<String>,
    argument_record: Option<ivm::PreparedArgumentRecord>,
    authorization: Option<crate::executor::ContractEntrypointAuthorizationSnapshot>,
}

fn requested_contract_entrypoint(metadata: &Metadata) -> Option<String> {
    metadata
        .get("contract_entrypoint")
        .and_then(|raw| raw.clone().try_into_any_norito::<String>().ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn add_embedded_entrypoint_authorization_read(
    set: &mut AccessSet,
    bytecode: &[u8],
    metadata: &Metadata,
) -> bool {
    let Some(selector) = requested_contract_entrypoint(metadata) else {
        return true;
    };
    let Ok(parsed) = ivm::ProgramMetadata::parse(bytecode) else {
        return false;
    };
    let Some(interface) = parsed.contract_interface.as_ref() else {
        return false;
    };
    let Some(descriptor) = interface
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == selector)
    else {
        return false;
    };
    let Ok(permission) = crate::executor::raw_contract_entrypoint_permission(descriptor, &selector)
    else {
        return false;
    };
    if permission.is_some() {
        set.add_read(AUTHORIZATION_EPOCH_KEY.to_owned());
    }
    true
}

fn resolve_callable_contract_entrypoint(
    bytecode: &[u8],
    selector: &str,
    interface_required_message: &'static str,
    raw_ivm: bool,
) -> Result<(u64, Option<String>, Option<ivm::EntrypointArgumentSchemaV1>), String> {
    let parsed = ivm::ProgramMetadata::parse(bytecode)
        .map_err(|err| format!("invalid contract artifact for contract call dispatch: {err}"))?;
    let prefix_len = parsed.prefix_len() as u64;
    let contract_interface = parsed
        .contract_interface
        .as_ref()
        .ok_or_else(|| interface_required_message.to_owned())?;
    let descriptor = contract_interface
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == selector)
        .ok_or_else(|| format!("unknown contract entrypoint `{selector}`"))?;
    let permission = if raw_ivm {
        crate::executor::raw_contract_entrypoint_permission(descriptor, selector)
    } else {
        crate::executor::callable_contract_entrypoint_permission(descriptor, selector)
    }
    .map_err(|error| error.to_string())?;
    Ok((
        prefix_len + descriptor.entry_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}

fn is_self_describing_contract(bytecode: &[u8]) -> bool {
    ivm::ProgramMetadata::parse(bytecode)
        .ok()
        .and_then(|parsed| parsed.contract_interface)
        .is_some()
}

fn parse_contract_call_execution_context(
    metadata: &Metadata,
    bytecode: &[u8],
    gas_limit: u64,
    authorization: Option<crate::executor::ContractEntrypointAuthorizationSnapshot>,
) -> Result<Option<ContractCallExecutionContext>, String> {
    let entrypoint = requested_contract_entrypoint(metadata);
    let payload = metadata.get("contract_payload").cloned();

    let (entrypoint, entrypoint_pc, entrypoint_permission, argument_schema) = if let Some(
        selector,
    ) =
        entrypoint.as_deref()
    {
        let (entrypoint_pc, entrypoint_permission, argument_schema) =
            resolve_callable_contract_entrypoint(
                bytecode,
                selector,
                "contract call entrypoint metadata requires a self-describing contract artifact",
                true,
            )?;
        let selected = authorization.as_ref().ok_or_else(|| {
            "raw-IVM contract entrypoint prepass requires an authorized live contract binding"
                .to_owned()
        })?;
        if selected.entrypoint != selector || selected.permission != entrypoint_permission {
            return Err(
                "raw-IVM contract entrypoint authorization changed before argument preparation"
                    .to_owned(),
            );
        }
        (
            Some(selector.to_owned()),
            Some(entrypoint_pc),
            entrypoint_permission,
            argument_schema,
        )
    } else if is_self_describing_contract(bytecode) {
        return Err(
            "self-describing contract calls require explicit contract_entrypoint metadata"
                .to_owned(),
        );
    } else if payload.is_none() {
        return Ok(None);
    } else {
        (None, None, None, None)
    };

    let canonical_record = crate::executor::encode_contract_argument_record(
        argument_schema.as_ref(),
        payload.as_ref(),
    )
    .map_err(|error| error.to_string())?;
    let argument_record = match (argument_schema.as_ref(), canonical_record) {
        (None, None) => None,
        (Some(schema), Some(record)) => Some(
            ivm::prepare_argument_record_with_gas_limit(schema, Arc::from(record), gas_limit)
                .map_err(|error| error.to_string())?,
        ),
        _ => return Err("contract argument schema and canonical record diverged".to_owned()),
    };
    Ok(Some(ContractCallExecutionContext {
        entrypoint,
        entrypoint_pc,
        entrypoint_permission,
        argument_record,
        authorization,
    }))
}

fn parse_contract_invocation_execution_context(
    invocation: &ContractInvocation,
    bytecode: &[u8],
    gas_limit: u64,
    authorization: crate::executor::ContractEntrypointAuthorizationSnapshot,
) -> Result<ContractCallExecutionContext, String> {
    let selector = invocation.entrypoint.trim();
    if selector.is_empty() {
        return Err("contract entrypoint must not be empty".to_owned());
    }

    let (entrypoint_pc, entrypoint_permission, argument_schema) =
        resolve_callable_contract_entrypoint(
            bytecode,
            selector,
            "contract call requires a self-describing contract artifact",
            false,
        )?;
    if authorization.entrypoint != selector || authorization.permission != entrypoint_permission {
        return Err(
            "deployed contract entrypoint authorization changed before argument preparation"
                .to_owned(),
        );
    }

    let argument_record = match (argument_schema.as_ref(), invocation.arguments.as_deref()) {
        (None, None) => None,
        (None, Some(_)) => {
            return Err("zero-parameter entrypoint must not carry an argument record".to_owned());
        }
        (Some(_), None) => {
            return Err("parameterized entrypoint requires an argument record".to_owned());
        }
        (Some(schema), Some(arguments)) => Some(
            ivm::prepare_argument_record_with_gas_limit(
                schema,
                Arc::<[u8]>::from(arguments),
                gas_limit,
            )
            .map_err(|error| error.to_string())?,
        ),
    };
    Ok(ContractCallExecutionContext {
        entrypoint: Some(selector.to_owned()),
        entrypoint_pc: Some(entrypoint_pc),
        entrypoint_permission,
        argument_record,
        authorization: Some(authorization),
    })
}

fn apply_contract_call_execution_context(
    vm: &mut ivm::IVM,
    context: Option<&ContractCallExecutionContext>,
) -> Result<(), String> {
    if let Some(context) = context
        && let Some(entrypoint_pc) = context.entrypoint_pc
    {
        // Match runtime contract-call semantics during access derivation so
        // non-`main` entrypoints can return cleanly to the VM end-of-stream.
        vm.set_register(1, vm.memory.code_len());
        vm.set_program_counter(entrypoint_pc).map_err(|err| {
            format!(
                "contract entrypoint `{}` resolved to invalid pc: {err}",
                context.entrypoint.as_deref().unwrap_or("<unspecified>")
            )
        })?;
    }
    Ok(())
}

fn manifest_access_set(
    manifest: &ContractManifest,
    code_hash: IrohaHash,
    bytecode: &[u8],
    cache_enabled: bool,
    requested_entrypoint: Option<&str>,
) -> Option<(AccessSet, AccessSetSource)> {
    let manifest_hash = cache_enabled.then(|| manifest_signature_hash(manifest));
    let mut selected_entrypoint_name = None;
    let mut authorization_read_required = false;
    if let Some(entrypoints) = manifest.entrypoints.as_deref() {
        let entrypoint = select_entrypoint(entrypoints, requested_entrypoint)?;
        selected_entrypoint_name = Some(entrypoint.name.clone());
        authorization_read_required = entrypoint_requires_authorization_read(entrypoint);
        if !entrypoint_access_hints_are_complete(entrypoint) {
            return None;
        }
        let key = AccessSetCacheKey {
            code_hash,
            entrypoint: Some(entrypoint.name.clone()),
        };
        if let Some(hash) = manifest_hash.as_ref() {
            if let Some(mut set) = access_set_cache_get(&key, hash) {
                if authorization_read_required {
                    set.add_read(AUTHORIZATION_EPOCH_KEY.to_owned());
                }
                return Some((set, AccessSetSource::EntrypointHints));
            }
        }
        if let Some(set) = entrypoint_access_set_if_safe(bytecode, entrypoint) {
            if let Some(hash) = manifest_hash.as_ref() {
                access_set_cache_put(key, hash.clone(), set.clone());
            }
            return Some((set, AccessSetSource::EntrypointHints));
        }

        // Entrypoint metadata is the most precise description available. If it is
        // incomplete or otherwise unsafe, do not mask that failure by falling back
        // to the contract-wide hints: those may contain the same under-approximation.
        if !entrypoint.read_keys.is_empty() || !entrypoint.write_keys.is_empty() {
            return None;
        }
        // A complete, explicitly empty entrypoint set may use the wider static
        // contract hints as a conservative over-approximation.
    }
    if let Some(hints) = manifest.access_set_hints.as_ref() {
        if !hints.dynamic_reads.is_empty() || !hints.dynamic_writes.is_empty() {
            return None;
        }
        let key = AccessSetCacheKey {
            code_hash,
            entrypoint: selected_entrypoint_name,
        };
        if let Some(hash) = manifest_hash.as_ref() {
            if let Some(mut set) = access_set_cache_get(&key, hash) {
                if authorization_read_required {
                    set.add_read(AUTHORIZATION_EPOCH_KEY.to_owned());
                }
                return Some((set, AccessSetSource::ManifestHints));
            }
        }
        if let Some(mut set) = manifest_hint_access_set_if_safe(bytecode, hints) {
            if authorization_read_required {
                set.add_read(AUTHORIZATION_EPOCH_KEY.to_owned());
            }
            if let Some(hash) = manifest_hash.as_ref() {
                access_set_cache_put(key, hash.clone(), set.clone());
            }
            return Some((set, AccessSetSource::ManifestHints));
        }
    }
    None
}

/// Derivation strategy for IVM executables.
#[derive(Debug, Copy, Clone)]
pub enum IvmStrategy {
    /// Attempt a dynamic prepass by executing the program with a read-only host and
    /// deriving keys from the queued ISIs. Fallback to conservative on error.
    DynamicThenConservative,
    /// Always conservative (serializes contracts).
    Conservative,
}

/// Derive access set for a signed transaction.
///
/// - ISI batches are analyzed statically by inspecting instruction targets.
/// - IVM contracts: when `ivm_strategy` is `DynamicThenConservative` and `state_view` is provided,
///   a read-only prepass is performed to derive keys from queued ISIs; otherwise conservative.
pub fn derive_for_transaction<R>(
    tx: &SignedTransaction,
    state_ro: Option<&R>,
    ivm_strategy: IvmStrategy,
) -> AccessSet
where
    R: StateReadOnly + QueryStateSource,
{
    derive_for_transaction_with_source(tx, state_ro, ivm_strategy).0
}

/// Derive access set for a signed transaction and report the IVM source, if any.
pub(crate) fn derive_for_transaction_with_source<R>(
    tx: &SignedTransaction,
    state_ro: Option<&R>,
    ivm_strategy: IvmStrategy,
) -> (AccessSet, Option<AccessSetSource>)
where
    R: StateReadOnly + QueryStateSource,
{
    match tx.instructions() {
        Executable::Instructions(batch) => with_stateful_admission_keys(
            tx,
            derive_from_isi_batch_with_state(batch.as_ref(), state_ro),
            None,
        ),
        Executable::ContractCall(call) => {
            if let Some(view) = state_ro
                && let Some(record) =
                    code::fetch_bound_contract_record(view, &call.contract_address)
                && record.code_hash == call.expected_code_hash
            {
                if let Some((set, source)) = manifest_access_set(
                    &record.manifest,
                    record.code_hash,
                    record.code_bytes.as_ref(),
                    view.pipeline().access_set_cache_enabled,
                    Some(call.entrypoint.as_str()),
                ) {
                    return with_stateful_admission_keys(tx, set, Some(source));
                }

                if matches!(ivm_strategy, IvmStrategy::DynamicThenConservative) {
                    let mut set = tx_gas_limit(tx)
                        .and_then(|gas_limit| {
                            let prepared =
                                ivm::prepare_contract(Arc::<[u8]>::from(record.code_bytes.clone()))
                                    .map_err(|error| {
                                        format!(
                                            "failed to prepare deployed contract artifact: {error}"
                                        )
                                    })?;
                            if prepared.code_hash() != record.code_hash {
                                return Err(
                                    "deployed contract bytecode no longer matches its live binding"
                                        .to_owned(),
                                );
                            }
                            let identity = code::BoundContractIdentity {
                                contract_address: record.contract_address.clone(),
                                contract_alias: record.contract_alias.clone(),
                                contract_alias_binding: record.contract_alias_binding.clone(),
                                code_hash: record.code_hash,
                            };
                            let authorization =
                                crate::executor::authorize_prepared_contract_selector(
                                    view.world(),
                                    tx.authority(),
                                    &prepared,
                                    &call.entrypoint,
                                    &identity,
                                )
                                .map_err(|error| error.to_string())?;
                            let context = parse_contract_invocation_execution_context(
                                call,
                                record.code_bytes.as_ref(),
                                gas_limit,
                                authorization,
                            )?;
                            derive_from_ivm_dynamic_with_context(
                                record.code_bytes.as_ref(),
                                tx.authority(),
                                Some(context),
                                view,
                                gas_limit,
                            )
                        })
                        .unwrap_or_else(|_| AccessSet::global());
                    let fenced =
                        apply_unverified_ivm_access_fence(record.code_bytes.as_ref(), &mut set);
                    let source = if fenced || is_conservative_global(&set) {
                        AccessSetSource::ConservativeFallback
                    } else {
                        AccessSetSource::PrepassMerge
                    };
                    return with_stateful_admission_keys(tx, set, Some(source));
                }
            }

            with_stateful_admission_keys(
                tx,
                AccessSet::global(),
                Some(AccessSetSource::ConservativeFallback),
            )
        }
        Executable::IvmProved(proved) => {
            let mut set = derive_from_isi_batch_with_state(proved.overlay.as_ref(), state_ro);
            if !add_embedded_entrypoint_authorization_read(
                &mut set,
                proved.bytecode.as_ref(),
                tx.metadata(),
            ) {
                set = AccessSet::global();
            }
            let fenced = apply_unverified_ivm_access_fence(proved.bytecode.as_ref(), &mut set);
            let source = (fenced || is_conservative_global(&set))
                .then_some(AccessSetSource::ConservativeFallback);
            with_stateful_admission_keys(tx, set, source)
        }
        Executable::Ivm(bytecode) => {
            let bytecode_ref = bytecode.as_ref();
            let requested_entrypoint = requested_contract_entrypoint(tx.metadata());
            if ivm::ProgramMetadata::parse(bytecode_ref).is_ok() {
                let code_hash = ivm::contract_code_hash(bytecode_ref);
                // 1) Try static hints from on-chain manifest (by code_hash)
                if let Some(view) = state_ro {
                    if let Some(manifest) = view.world().contract_manifests().get(&code_hash) {
                        if let Some((set, source)) = manifest_access_set(
                            manifest,
                            code_hash,
                            bytecode_ref,
                            view.pipeline().access_set_cache_enabled,
                            requested_entrypoint.as_deref(),
                        ) {
                            return with_stateful_admission_keys(tx, set, Some(source));
                        }
                    }
                }
                // 1b) Fallback to manifest provided in transaction metadata.
                if let Some(manifest) = manifest_from_metadata(tx) {
                    if manifest.code_hash == Some(code_hash)
                        && manifest_matches_embedded_contract(bytecode_ref, &manifest)
                    {
                        if let Some((set, source)) = manifest_access_set(
                            &manifest,
                            code_hash,
                            bytecode_ref,
                            false,
                            requested_entrypoint.as_deref(),
                        ) {
                            return with_stateful_admission_keys(tx, set, Some(source));
                        }
                    }
                }
            }
            // 2) Otherwise, use dynamic prepass if enabled with view, else conservative
            let (set, source) = match (ivm_strategy, state_ro) {
                (IvmStrategy::DynamicThenConservative, Some(view)) => {
                    let mut set = tx_gas_limit(tx)
                        .and_then(|gas_limit| {
                            derive_from_ivm_dynamic(
                                bytecode_ref,
                                tx.authority(),
                                tx.metadata(),
                                view,
                                gas_limit,
                            )
                        })
                        .unwrap_or_else(|_| AccessSet::global());
                    let fenced = apply_unverified_ivm_access_fence(bytecode_ref, &mut set);
                    let source = if fenced || is_conservative_global(&set) {
                        AccessSetSource::ConservativeFallback
                    } else {
                        AccessSetSource::PrepassMerge
                    };
                    (set, Some(source))
                }
                _ => (
                    AccessSet::global(),
                    Some(AccessSetSource::ConservativeFallback),
                ),
            };
            with_stateful_admission_keys(tx, set, source)
        }
    }
}

/// Derive access for a transaction whose overlay has already been built.
pub(crate) fn derive_for_prepared_overlay_with_source<R>(
    tx: &SignedTransaction,
    state_ro: &R,
    overlay: &crate::pipeline::overlay::TxOverlay,
    access_log: Option<&ivm::host::AccessLog>,
    dynamic_prepass: bool,
) -> (AccessSet, Option<AccessSetSource>)
where
    R: StateReadOnly + QueryStateSource,
{
    match tx.instructions() {
        Executable::Instructions(_) => with_stateful_admission_keys(
            tx,
            derive_from_overlay_artifacts(overlay, None, Some(state_ro), false),
            None,
        ),
        Executable::IvmProved(proved) => {
            let mut set = derive_from_overlay_artifacts(overlay, None, Some(state_ro), false);
            if !add_embedded_entrypoint_authorization_read(
                &mut set,
                proved.bytecode.as_ref(),
                tx.metadata(),
            ) {
                set = AccessSet::global();
            }
            let fenced = apply_unverified_ivm_access_fence(proved.bytecode.as_ref(), &mut set);
            let source = (fenced || is_conservative_global(&set))
                .then_some(AccessSetSource::ConservativeFallback);
            with_stateful_admission_keys(tx, set, source)
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {
            let (hint_set, hint_source) =
                derive_for_transaction_with_source(tx, Some(state_ro), IvmStrategy::Conservative);
            if matches!(
                hint_source,
                Some(AccessSetSource::ManifestHints | AccessSetSource::EntrypointHints)
            ) {
                return (hint_set, hint_source);
            }
            if !dynamic_prepass {
                return (hint_set, hint_source);
            }

            let set = derive_from_overlay_artifacts(overlay, access_log, Some(state_ro), true);
            let source = if is_conservative_global(&set) {
                AccessSetSource::ConservativeFallback
            } else {
                AccessSetSource::PrepassMerge
            };
            with_stateful_admission_keys(tx, set, Some(source))
        }
    }
}

fn derive_from_overlay_artifacts<R>(
    overlay: &crate::pipeline::overlay::TxOverlay,
    access_log: Option<&ivm::host::AccessLog>,
    state_ro: Option<&R>,
    conservative_if_empty: bool,
) -> AccessSet
where
    R: StateReadOnly + QueryStateSource,
{
    let mut set = AccessSet::new();
    let max_depth = state_ro
        .map(|state| {
            state
                .world()
                .parameters()
                .smart_contract()
                .execution_depth()
        })
        .unwrap_or(0);
    let mut visited_triggers = BTreeSet::new();
    for isi in overlay.instructions() {
        set.union_with(derive_from_instruction(
            isi,
            state_ro,
            &mut visited_triggers,
            0,
            max_depth,
        ));
    }
    if let Some(log) = access_log {
        merge_access_log(&mut set, log);
    }
    for path in overlay.durable_state_overlay().keys() {
        set.add_write(access_key_from_state_log(&path.to_string()));
    }
    if conservative_if_empty && set.read_keys.is_empty() && set.write_keys.is_empty() {
        AccessSet::global()
    } else {
        set
    }
}

fn is_conservative_global(set: &AccessSet) -> bool {
    set.read_keys.is_empty() && set.write_keys.len() == 1 && set.write_keys.contains("*")
}

fn apply_unverified_ivm_access_fence(bytecode: &[u8], set: &mut AccessSet) -> bool {
    let fence = ivm::analysis::analyze_program(bytecode).map_or(
        crate::pipeline::overlay::VmAccessFence::Global,
        |analysis| crate::pipeline::overlay::VmAccessFence::from_program_analysis(&analysis),
    );
    if let Some(key) = fence.scheduler_write_key() {
        set.add_write(key.to_owned());
        true
    } else {
        false
    }
}

fn manifest_matches_embedded_contract(bytecode: &[u8], manifest: &ContractManifest) -> bool {
    ivm::verify_contract_artifact(bytecode)
        .map(|verified| manifest.signature_payload() == verified.manifest.signature_payload())
        .unwrap_or(false)
}

fn key_tx_sequence(account: &AccountId) -> AccessKey {
    format!("tx.sequence:{account}")
}

fn key_sccp_outbound_message(
    key: &iroha_data_model::bridge::SccpOutboundMessageKeyV1,
) -> AccessKey {
    let mut out = format!(
        "sccp.outbound.v1:{}:{}:",
        key.lane.source.profile_key(),
        key.lane.target.profile_key()
    );
    for byte in key.message_id {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn key_bridge_proof_hash(proof_hash: &[u8; 32]) -> AccessKey {
    let mut out = "bridge.proof:".to_owned();
    for byte in proof_hash {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn key_bridge_backend(backend: &str) -> AccessKey {
    format!("bridge.backend:{backend}")
}

fn key_sccp_native_bridge_message(
    lane: iroha_data_model::bridge::SccpLaneIdV1,
    message_id: [u8; 32],
) -> AccessKey {
    let mut out = format!(
        "sccp.bridge.native:{}:{}:",
        lane.source.profile_key(),
        lane.target.profile_key()
    );
    for byte in message_id {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
std::thread_local! {
    static BRIDGE_PROOF_HASH_ATTEMPTS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_bridge_proof_hash_attempts() {
    BRIDGE_PROOF_HASH_ATTEMPTS.with(|count| count.set(0));
}

#[cfg(test)]
fn bridge_proof_hash_attempts() -> usize {
    BRIDGE_PROOF_HASH_ATTEMPTS.with(core::cell::Cell::get)
}

fn bridge_proof_hash(proof: &iroha_data_model::bridge::BridgeProof) -> Option<[u8; 32]> {
    #[cfg(test)]
    BRIDGE_PROOF_HASH_ATTEMPTS.with(|count| count.set(count.get().saturating_add(1)));
    let backend = proof.backend_label();
    let encoded = norito::to_bytes(proof).ok()?;
    Some(crate::zk::hash_proof(
        &iroha_data_model::proof::ProofBox::new(backend, encoded),
    ))
}

fn sccp_bridge_message_access_key(
    proof: &iroha_data_model::bridge::BridgeProof,
) -> Option<Option<AccessKey>> {
    match &proof.payload {
        // Extracting the exact message key requires trusting nested,
        // proof-controlled framing. Scheduler derivation must not decode that
        // archive or run any curve/pairing work, so destination submissions use
        // the conservative global access set. Execution later resolves the
        // exact key from its single fully validated owned context.
        iroha_data_model::bridge::BridgeProofPayload::SccpDestination(_) => None,
        iroha_data_model::bridge::BridgeProofPayload::NativeProtocol(native) => {
            let decoded = iroha_sccp::decode_bridge_native_protocol_proof_v1(native).ok()?;
            Some(Some(key_sccp_native_bridge_message(
                decoded.source.lane,
                decoded.source.message_id,
            )))
        }
        iroha_data_model::bridge::BridgeProofPayload::Ics(_)
        | iroha_data_model::bridge::BridgeProofPayload::TransparentZk(_) => Some(None),
    }
}

fn derive_submit_bridge_proof_access(
    submit: &iroha_data_model::isi::bridge::SubmitBridgeProof,
) -> AccessSet {
    let Some(sccp_message_key) = sccp_bridge_message_access_key(&submit.proof) else {
        return AccessSet::global();
    };
    let Some(proof_hash) = bridge_proof_hash(&submit.proof) else {
        return AccessSet::global();
    };
    let mut set = AccessSet::new();
    set.add_write(key_bridge_proof_hash(&proof_hash));
    set.add_write(key_bridge_backend(&submit.proof.backend_label()));
    if let Some(sccp_message_key) = sccp_message_key {
        set.add_write(sccp_message_key);
    }
    set
}

fn derive_record_bridge_receipt_access(
    record: &iroha_data_model::isi::bridge::RecordBridgeReceipt,
) -> AccessSet {
    let mut set = AccessSet::new();
    set.add_read(NEXUS_ACTIVE_LANE_CATALOG_KEY.to_owned());
    set.add_write(key_bridge_proof_hash(&record.receipt.proof_hash));
    set
}

fn derive_sccp_outbound_message_access(
    record: &iroha_data_model::isi::bridge::RecordSccpMessage,
) -> AccessSet {
    let Ok(validated) = crate::bridge::validate_recorded_sccp_message_payload_bytes(
        record.context,
        &record.payload_bytes,
    ) else {
        return AccessSet::global();
    };
    let mut set = AccessSet::new();
    set.add_read(NEXUS_ACTIVE_LANE_CATALOG_KEY.to_owned());
    set.add_read(SCCP_ON_CHAIN_REGISTRY_KEY.to_owned());
    set.add_write(key_sccp_outbound_message(&validated.key));
    set
}

fn with_stateful_admission_keys(
    tx: &SignedTransaction,
    mut set: AccessSet,
    source: Option<AccessSetSource>,
) -> (AccessSet, Option<AccessSetSource>) {
    expand_authority_placeholders(&mut set, tx.authority());
    set.add_read(key_account(tx.authority()));
    set.add_write(key_tx_sequence(tx.authority()));
    if let Executable::ContractCall(invocation) = tx.instructions() {
        set.add_read(format!("contract.instance:{}", invocation.contract_address));
        let lifecycle_marker =
            code::contract_lifecycle_state_key(&invocation.contract_address).to_string();
        let lifecycle_key = access_key_from_state_log(&lifecycle_marker);
        set.add_read(lifecycle_key.clone());
        if matches!(
            invocation.entrypoint.as_str(),
            "hajimari" | "始まり" | "kaizen" | "改善"
        ) {
            set.add_write(lifecycle_key);
        }
    }
    (set, source)
}

fn expand_authority_placeholders(set: &mut AccessSet, authority: &AccountId) {
    set.read_keys = set
        .read_keys
        .iter()
        .map(|key| expand_authority_placeholder_key(key, authority))
        .collect();
    set.write_keys = set
        .write_keys
        .iter()
        .map(|key| expand_authority_placeholder_key(key, authority))
        .collect();
}

fn expand_authority_placeholder_key(key: &str, authority: &AccountId) -> String {
    if key == AUTHORITY_ACCOUNT_KEY {
        return key_account(authority);
    }
    if let Some(rest) = key.strip_prefix("asset:")
        && let Some(definition_raw) = rest.strip_suffix(":$authority")
        && let Ok(definition) = AssetDefinitionId::parse_address_literal(definition_raw)
    {
        return key_asset(&AssetId::of(definition, authority.clone()));
    }
    if let Some(key_raw) = key.strip_prefix("account.detail:$authority:")
        && let Ok(name) = key_raw.parse::<Name>()
    {
        return key_account_detail(authority, &name);
    }
    if let Some(role_raw) = key.strip_prefix("role.binding:$authority:")
        && let Ok(role) = role_raw.parse::<RoleId>()
    {
        return key_role_binding(authority, &role);
    }
    if let Some(permission) = key.strip_prefix("perm.account:$authority:")
        && !permission.is_empty()
    {
        return format!("perm.account:{authority}:{permission}");
    }
    key.to_owned()
}

fn is_authority_placeholder_key(key: &str) -> bool {
    if key == AUTHORITY_ACCOUNT_KEY {
        return true;
    }
    if let Some(rest) = key.strip_prefix("asset:")
        && let Some(definition_raw) = rest.strip_suffix(":$authority")
    {
        return AssetDefinitionId::parse_address_literal(definition_raw).is_ok();
    }
    if let Some(key_raw) = key.strip_prefix("account.detail:$authority:") {
        return key_raw.parse::<Name>().is_ok();
    }
    if let Some(role_raw) = key.strip_prefix("role.binding:$authority:") {
        return role_raw.parse::<RoleId>().is_ok();
    }
    if let Some(permission) = key.strip_prefix("perm.account:$authority:") {
        return !permission.is_empty();
    }
    false
}

fn entrypoint_access_set_if_safe(
    bytecode: &[u8],
    entrypoint: &EntrypointDescriptor,
) -> Option<AccessSet> {
    if !entrypoint_access_hints_are_complete(entrypoint) {
        return None;
    }
    let mut set = hint_access_set_if_safe(bytecode, &entrypoint.read_keys, &entrypoint.write_keys)?;
    if entrypoint_requires_authorization_read(entrypoint) {
        set.add_read(AUTHORIZATION_EPOCH_KEY.to_owned());
    }
    Some(set)
}

fn entrypoint_requires_authorization_read(entrypoint: &EntrypointDescriptor) -> bool {
    entrypoint.permission.is_some()
        || matches!(
            entrypoint.kind,
            iroha_data_model::smart_contract::manifest::EntryPointKind::Hajimari
                | iroha_data_model::smart_contract::manifest::EntryPointKind::Kaizen
        )
}

fn entrypoint_access_hints_are_complete(entrypoint: &EntrypointDescriptor) -> bool {
    entrypoint.access_hints_complete == Some(true) && entrypoint.access_hints_skipped.is_empty()
}

fn hint_access_set_if_safe(
    bytecode: &[u8],
    read_keys: &[String],
    write_keys: &[String],
) -> Option<AccessSet> {
    hint_access_set_with_dynamic_if_safe(bytecode, read_keys, write_keys, &[], &[])
}

fn manifest_hint_access_set_if_safe(
    bytecode: &[u8],
    hints: &iroha_data_model::smart_contract::manifest::AccessSetHints,
) -> Option<AccessSet> {
    // Dynamic hints currently identify only a base key and do not carry enough
    // information to prove that every concrete state key conflicts with it.
    // Keep them advisory until that relationship is represented explicitly.
    if !hints.dynamic_reads.is_empty() || !hints.dynamic_writes.is_empty() {
        return None;
    }
    hint_access_set_with_dynamic_if_safe(
        bytecode,
        &hints.read_keys,
        &hints.write_keys,
        &hints.dynamic_reads,
        &hints.dynamic_writes,
    )
}

fn hint_access_set_with_dynamic_if_safe(
    bytecode: &[u8],
    read_keys: &[String],
    write_keys: &[String],
    dynamic_reads: &[DynamicAccessHint],
    dynamic_writes: &[DynamicAccessHint],
) -> Option<AccessSet> {
    if read_keys.is_empty() && write_keys.is_empty() {
        if dynamic_reads.is_empty() && dynamic_writes.is_empty() {
            return None;
        }
    }
    let set = access_set_from_hint_keys(read_keys, write_keys, dynamic_reads, dynamic_writes)?;
    let global_read = read_keys.iter().any(|key| key == "*");
    let global_write = write_keys.iter().any(|key| key == "*");
    if global_write {
        return Some(set);
    }
    let report = match ivm::analysis::analyze_program(bytecode) {
        Ok(report) => report,
        Err(_) => return None,
    };
    let state_read_wildcard = read_keys.iter().any(|key| key == "state:*")
        || write_keys.iter().any(|key| key == "state:*");
    let state_write_wildcard = write_keys.iter().any(|key| key == "state:*");
    for syscall in &report.syscalls {
        use ivm::syscalls::SyscallAccess;

        let covered = match ivm::syscalls::syscall_access(syscall.number) {
            SyscallAccess::None => true,
            SyscallAccess::StateRead => state_read_wildcard || global_read,
            SyscallAccess::StateWrite => state_write_wildcard,
            SyscallAccess::LedgerRead => global_read,
            SyscallAccess::LedgerWrite | SyscallAccess::Dynamic => false,
        };
        if !covered {
            return None;
        }
    }
    Some(set)
}

fn select_entrypoint<'a>(
    entrypoints: &'a [EntrypointDescriptor],
    requested_entrypoint: Option<&str>,
) -> Option<&'a EntrypointDescriptor> {
    if entrypoints.is_empty() {
        return None;
    }
    if let Some(requested) = requested_entrypoint {
        return entrypoints.iter().find(|entry| entry.name == requested);
    }
    None
}

/// Normalize manifest/entrypoint hint keys into canonical WSV keys plus state keys.
#[allow(clippy::too_many_lines)]
fn access_set_from_hint_keys(
    read_keys: &[String],
    write_keys: &[String],
    dynamic_reads: &[DynamicAccessHint],
    dynamic_writes: &[DynamicAccessHint],
) -> Option<AccessSet> {
    let mut advisory = StateAccessSetAdvisory::default();
    let mut state_reads: BTreeSet<String> = BTreeSet::new();
    let mut state_writes: BTreeSet<String> = BTreeSet::new();
    let ingest = |raw: &str,
                  canonical: &mut Vec<CanonicalStateKey>,
                  state_keys: &mut BTreeSet<String>|
     -> Option<()> {
        if raw == "*" {
            state_keys.insert(raw.to_owned());
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("state:") {
            if rest.is_empty() {
                return None;
            }
            state_keys.insert(raw.to_owned());
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("zk:election:") {
            if rest.is_empty() || rest.contains('*') {
                return None;
            }
            state_keys.insert(raw.to_owned());
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("zk_asset:") {
            AssetDefinitionId::parse_address_literal(rest).ok()?;
            state_keys.insert(raw.to_owned());
            return Some(());
        }
        if is_authority_placeholder_key(raw) {
            state_keys.insert(raw.to_owned());
            return Some(());
        }
        if raw == ACCOUNT_WILDCARD_KEY {
            state_keys.insert(raw.to_owned());
            return Some(());
        }
        if raw == ASSET_WILDCARD_KEY || raw == ASSET_DEF_WILDCARD_KEY {
            state_keys.insert(raw.to_owned());
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("account.detail:") {
            let mut parsed: Option<AccountMetadataKey> = None;
            for split in [rest.split_once(':'), rest.rsplit_once(':')] {
                let Some((id_raw, key_raw)) = split else {
                    continue;
                };
                let Ok(key) = key_raw.parse::<Name>() else {
                    continue;
                };
                match AccountId::parse_encoded(id_raw)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                {
                    Ok(id) => {
                        parsed = Some(AccountMetadataKey { id, key });
                        break;
                    }
                    Err(_) => continue,
                }
            }
            match parsed {
                Some(key) => {
                    canonical.push(CanonicalStateKey::AccountMetadata(key));
                }
                None => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("domain.detail:") {
            let (id, key) = rest.split_once(':')?;
            let id = DomainId::parse_fully_qualified(id).ok()?;
            let key: Name = key.parse().ok()?;
            canonical.push(CanonicalStateKey::DomainMetadata(DomainMetadataKey {
                id,
                key,
            }));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("asset_def.detail:") {
            let (id, key) = rest.split_once(':')?;
            let id = AssetDefinitionId::parse_address_literal(id).ok()?;
            let key: Name = key.parse().ok()?;
            canonical.push(CanonicalStateKey::AssetDefinitionMetadata(
                AssetDefinitionMetadataKey { id, key },
            ));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("asset.detail:") {
            let mut parsed: Option<AssetMetadataKey> = None;
            for split in [rest.split_once(':'), rest.rsplit_once(':')] {
                let Some((id_raw, key_raw)) = split else {
                    continue;
                };
                let Ok(key) = key_raw.parse::<Name>() else {
                    continue;
                };
                match AssetId::parse_literal(id_raw) {
                    Ok(id) => {
                        parsed = Some(AssetMetadataKey { id, key });
                        break;
                    }
                    Err(_) => continue,
                }
            }
            match parsed {
                Some(key) => {
                    canonical.push(CanonicalStateKey::AssetMetadata(key));
                }
                None => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("nft.detail:") {
            let (id, key) = rest.split_once(':')?;
            let id: NftId = id.parse().ok()?;
            let key: Name = key.parse().ok()?;
            canonical.push(CanonicalStateKey::NftMetadata(NftMetadataKey { id, key }));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("rwa.detail:") {
            let (id, key) = rest.split_once(':')?;
            let id: RwaId = id.parse().ok()?;
            let key: Name = key.parse().ok()?;
            canonical.push(CanonicalStateKey::RwaMetadata(RwaMetadataKey { id, key }));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("trigger.detail:") {
            let (id, key) = rest.split_once(':')?;
            let id: TriggerId = id.parse().ok()?;
            let key: Name = key.parse().ok()?;
            canonical.push(CanonicalStateKey::TriggerMetadata(TriggerMetadataKey {
                id,
                key,
            }));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("role.binding:") {
            let mut parsed: Option<AccountRoleKey> = None;
            for split in [rest.split_once(':'), rest.rsplit_once(':')] {
                let Some((account_raw, role_raw)) = split else {
                    continue;
                };
                let Ok(role) = role_raw.parse::<RoleId>() else {
                    continue;
                };
                match AccountId::parse_encoded(account_raw)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                {
                    Ok(account) => {
                        parsed = Some(AccountRoleKey { account, role });
                        break;
                    }
                    Err(_) => continue,
                }
            }
            match parsed {
                Some(key) => {
                    canonical.push(CanonicalStateKey::AccountRole(key));
                }
                None => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("account:") {
            match AccountId::parse_encoded(rest)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            {
                Ok(id) => canonical.push(CanonicalStateKey::Account(id)),
                Err(_) => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("domain:") {
            let id = DomainId::parse_fully_qualified(rest).ok()?;
            canonical.push(CanonicalStateKey::Domain(id));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("asset_def:") {
            if let Ok(id) = AssetDefinitionId::parse_address_literal(rest) {
                canonical.push(CanonicalStateKey::AssetDefinition(id));
            } else {
                return None;
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("asset:") {
            match AssetId::parse_literal(rest) {
                Ok(id) => canonical.push(CanonicalStateKey::Asset(id)),
                Err(_) => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("nft:") {
            let id: NftId = rest.parse().ok()?;
            canonical.push(CanonicalStateKey::Nft(id));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("rwa:") {
            let id: RwaId = rest.parse().ok()?;
            canonical.push(CanonicalStateKey::Rwa(id));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("trigger:") {
            let id: TriggerId = rest.parse().ok()?;
            canonical.push(CanonicalStateKey::Trigger(id));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("role:") {
            let id: RoleId = rest.parse().ok()?;
            canonical.push(CanonicalStateKey::Role(id));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("txqueue:") {
            let hash: iroha_crypto::HashOf<SignedTransaction> = rest.parse().ok()?;
            canonical.push(CanonicalStateKey::TxQueue(TxQueueKey { hash }));
            return Some(());
        }
        None
    };

    for key in read_keys {
        ingest(key, &mut advisory.reads, &mut state_reads)?;
    }
    for key in write_keys {
        ingest(key, &mut advisory.writes, &mut state_writes)?;
    }
    for hint in dynamic_reads {
        ingest_dynamic_hint(hint, &mut state_reads)?;
    }
    for hint in dynamic_writes {
        ingest_dynamic_hint(hint, &mut state_writes)?;
        ingest_dynamic_hint(hint, &mut state_reads)?;
    }

    advisory.canonicalize();

    let render = |key: &CanonicalStateKey| -> AccessKey {
        match key {
            CanonicalStateKey::Domain(id) => format!("domain:{id}"),
            CanonicalStateKey::Account(id) => format!("account:{id}"),
            CanonicalStateKey::Asset(id) => format!("asset:{id}"),
            CanonicalStateKey::AssetDefinition(id) => format!("asset_def:{id}"),
            CanonicalStateKey::Nft(id) => format!("nft:{id}"),
            CanonicalStateKey::Rwa(id) => format!("rwa:{id}"),
            CanonicalStateKey::Trigger(id) => format!("trigger:{id}"),
            CanonicalStateKey::Role(id) => format!("role:{id}"),
            CanonicalStateKey::AccountPermissions(id) => format!("perm.account:{id}"),
            CanonicalStateKey::AccountRole(key) => {
                format!("role.binding:{}:{}", key.account, key.role)
            }
            CanonicalStateKey::TxQueue(key) => format!("txqueue:{}", key.hash),
            CanonicalStateKey::DomainMetadata(key) => {
                format!("domain.detail:{}:{}", key.id, key.key)
            }
            CanonicalStateKey::AccountMetadata(key) => {
                format!("account.detail:{}:{}", key.id, key.key)
            }
            CanonicalStateKey::AssetDefinitionMetadata(key) => {
                format!("asset_def.detail:{}:{}", key.id, key.key)
            }
            CanonicalStateKey::AssetMetadata(key) => format!("asset.detail:{}:{}", key.id, key.key),
            CanonicalStateKey::NftMetadata(key) => format!("nft.detail:{}:{}", key.id, key.key),
            CanonicalStateKey::RwaMetadata(key) => format!("rwa.detail:{}:{}", key.id, key.key),
            CanonicalStateKey::TriggerMetadata(key) => {
                format!("trigger.detail:{}:{}", key.id, key.key)
            }
        }
    };

    let mut set = AccessSet::new();
    for key in advisory.reads {
        set.add_read(render(&key));
    }
    for key in advisory.writes {
        set.add_write(render(&key));
    }
    for key in state_reads {
        set.add_read(key);
    }
    for key in state_writes {
        set.add_write(key);
    }
    Some(set)
}

fn ingest_dynamic_hint(hint: &DynamicAccessHint, state_keys: &mut BTreeSet<String>) -> Option<()> {
    if hint.max_keys == 0 {
        return None;
    }
    let rest = hint.base_key.strip_prefix("state:")?;
    if rest.is_empty() || rest.contains('/') || rest == "*" {
        return None;
    }
    state_keys.insert(hint.base_key.clone());
    Some(())
}

fn derive_from_isi_batch_with_state<R>(batch: &[InstructionBox], state_ro: Option<&R>) -> AccessSet
where
    R: StateReadOnly + QueryStateSource,
{
    if let Some(set) = derive_simple_asset_transfer_batch(batch) {
        return set;
    }

    let mut set = AccessSet::new();
    let max_depth = state_ro
        .map(|view| view.world().parameters().smart_contract().execution_depth())
        .unwrap_or(0);
    let mut visited_triggers = BTreeSet::new();
    for instr in batch {
        set.union_with(derive_from_instruction(
            instr,
            state_ro,
            &mut visited_triggers,
            0,
            max_depth,
        ));
    }
    set
}

fn derive_simple_asset_transfer_batch(batch: &[InstructionBox]) -> Option<AccessSet> {
    let mut set = AccessSet::new();
    for instr in batch {
        let transfer = instr.as_any().downcast_ref::<TransferBox>()?;
        let TransferBox::Asset(transfer) = transfer else {
            return None;
        };
        let source_id = transfer.source.clone();
        let destination_id = AssetId::of(
            transfer.source.definition.clone(),
            transfer.destination.clone(),
        );
        add_asset_rw(&mut set, &source_id);
        add_asset_rw(&mut set, &destination_id);
    }
    Some(set)
}

#[allow(clippy::too_many_lines)]
fn derive_from_instruction<R>(
    instr: &InstructionBox,
    state_ro: Option<&R>,
    visited_triggers: &mut BTreeSet<TriggerId>,
    depth: u8,
    max_depth: u8,
) -> AccessSet
where
    R: StateReadOnly + QueryStateSource,
{
    let mut set = AccessSet::new();
    let any = instr.as_any();

    // Logging is side-effect-free; keep it conflict-free.
    if any.downcast_ref::<Log>().is_some() {
        return set;
    }
    if let Some(submit) = any.downcast_ref::<iroha_data_model::isi::bridge::SubmitBridgeProof>() {
        return derive_submit_bridge_proof_access(submit);
    }
    if let Some(record) = any.downcast_ref::<iroha_data_model::isi::bridge::RecordBridgeReceipt>() {
        return derive_record_bridge_receipt_access(record);
    }
    if let Some(record) = any.downcast_ref::<iroha_data_model::isi::bridge::RecordSccpMessage>() {
        return derive_sccp_outbound_message_access(record);
    }

    // Transfers
    if let Some(tb) = any.downcast_ref::<TransferBox>() {
        match tb {
            TransferBox::Asset(t) => {
                // source: AssetId, destination: AccountId
                let src = t.source.clone();
                let dst = AssetId::of(t.source.definition.clone(), t.destination.clone());
                add_asset_rw(&mut set, &src);
                add_asset_rw(&mut set, &dst);
            }
            TransferBox::Domain(t) => {
                add_domain_rw(&mut set, &t.object);
                add_account_r(&mut set, &t.source);
                add_account_r(&mut set, &t.destination);
            }
            TransferBox::AssetDefinition(t) => {
                add_asset_def_rw(&mut set, &t.object);
                add_account_r(&mut set, &t.source);
                add_account_r(&mut set, &t.destination);
            }
            TransferBox::Nft(t) => {
                add_nft_rw(&mut set, &t.object);
                add_account_r(&mut set, &t.source);
                add_account_r(&mut set, &t.destination);
            }
        }
        return set;
    }

    if let Some(rb) = any.downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>() {
        use iroha_data_model::isi::rwa::RwaInstructionBox;

        match rb {
            RwaInstructionBox::Register(r) => {
                add_domain_rw(&mut set, r.rwa.domain());
            }
            RwaInstructionBox::Transfer(t) => {
                add_rwa_rw(&mut set, t.rwa());
                add_account_r(&mut set, t.source());
                add_account_r(&mut set, t.destination());
            }
            RwaInstructionBox::Merge(m) => {
                for parent in m.parents() {
                    add_rwa_rw(&mut set, parent.rwa());
                }
            }
            RwaInstructionBox::Redeem(r) => add_rwa_rw(&mut set, r.rwa()),
            RwaInstructionBox::Freeze(r) => add_rwa_rw(&mut set, r.rwa()),
            RwaInstructionBox::Unfreeze(r) => add_rwa_rw(&mut set, r.rwa()),
            RwaInstructionBox::Hold(r) => add_rwa_rw(&mut set, r.rwa()),
            RwaInstructionBox::Release(r) => add_rwa_rw(&mut set, r.rwa()),
            RwaInstructionBox::ForceTransfer(r) => {
                add_rwa_rw(&mut set, r.rwa());
                add_account_r(&mut set, r.destination());
            }
            RwaInstructionBox::SetControls(r) => add_rwa_rw(&mut set, r.rwa()),
            RwaInstructionBox::SetKeyValue(r) => add_rwa_detail_rw(&mut set, &r.object, &r.key),
            RwaInstructionBox::RemoveKeyValue(r) => add_rwa_detail_rw(&mut set, &r.object, &r.key),
        }
        return set;
    }

    // Mint
    if let Some(mb) = any.downcast_ref::<MintBox>() {
        match mb {
            MintBox::Asset(m) => {
                add_asset_rw(&mut set, &m.destination);
                add_asset_def_rw(&mut set, m.destination.definition());
            }
            MintBox::TriggerRepetitions(m) => {
                add_trigger_rw(&mut set, &m.destination);
            }
        }
        return set;
    }

    // Burn
    if let Some(bb) = any.downcast_ref::<BurnBox>() {
        match bb {
            BurnBox::Asset(b) => {
                add_asset_rw(&mut set, &b.destination);
                add_asset_def_rw(&mut set, b.destination.definition());
            }
            BurnBox::TriggerRepetitions(b) => {
                add_trigger_rw(&mut set, &b.destination);
            }
        }
        return set;
    }

    if any
        .downcast_ref::<iroha_data_model::isi::SetAssetDefinitionBalancePolicy>()
        .is_some()
    {
        return AccessSet::global();
    }

    // Set / Remove key-values
    if let Some(sb) = any.downcast_ref::<SetKeyValueBox>() {
        match sb {
            SetKeyValueBox::Account(s) => {
                add_account_detail_rw(&mut set, &s.object, &s.key);
            }
            SetKeyValueBox::Domain(s) => {
                add_domain_detail_rw(&mut set, &s.object, &s.key);
            }
            SetKeyValueBox::AssetDefinition(s) => {
                add_asset_def_detail_rw(&mut set, &s.object, &s.key);
            }
            SetKeyValueBox::Nft(s) => {
                add_nft_detail_rw(&mut set, &s.object, &s.key);
            }
            SetKeyValueBox::Trigger(s) => {
                set.add_read(key_trigger(&s.object));
                set.add_write(format!("trigger.detail:{}:{}", &s.object, &s.key));
            }
        }
        return set;
    }
    if let Some(rb) = any.downcast_ref::<RemoveKeyValueBox>() {
        match rb {
            RemoveKeyValueBox::Account(r) => {
                add_account_detail_rw(&mut set, &r.object, &r.key);
            }
            RemoveKeyValueBox::Domain(r) => {
                add_domain_detail_rw(&mut set, &r.object, &r.key);
            }
            RemoveKeyValueBox::AssetDefinition(r) => {
                add_asset_def_detail_rw(&mut set, &r.object, &r.key);
            }
            RemoveKeyValueBox::Nft(r) => {
                add_nft_detail_rw(&mut set, &r.object, &r.key);
            }
            RemoveKeyValueBox::Trigger(r) => {
                set.add_read(key_trigger(&r.object));
                set.add_write(format!("trigger.detail:{}:{}", &r.object, &r.key));
            }
        }
        return set;
    }

    // Register / Unregister
    if let Some(rb) = any.downcast_ref::<RegisterBox>() {
        match rb {
            RegisterBox::Domain(r) => add_domain_rw(&mut set, &r.object.id().clone()),
            RegisterBox::Account(r) => add_account_rw(&mut set, r.object.id()),
            RegisterBox::AssetDefinition(r) => {
                add_asset_def_rw(&mut set, &r.object.id().clone());
            }
            RegisterBox::Nft(r) => add_nft_rw(&mut set, r.object.id()),
            RegisterBox::Peer(_) => set = AccessSet::global(),
            RegisterBox::Trigger(r) => add_trigger_rw(&mut set, r.object.id()),
            RegisterBox::Role(r) => {
                add_role_rw(&mut set, r.object.id());
                set.add_write(AUTHORIZATION_EPOCH_KEY.to_owned());
            }
        }
        return set;
    }

    // ZK Voting
    if let Some(instr) = any.downcast_ref::<zk::CreateElection>() {
        // Single election record write
        set.add_write(format!("zk:election:{}", instr.election_id()));
        return set;
    }
    if let Some(instr) = any.downcast_ref::<zk::SubmitBallot>() {
        // Write ciphertext history and nullifiers for this election
        let id = instr.election_id();
        set.add_write(format!("zk:election:{id}:ciphertexts"));
        set.add_write(format!("zk:election:{id}:nullifiers"));
        return set;
    }
    if let Some(instr) = any.downcast_ref::<zk::FinalizeElection>() {
        // Write finalized tally for this election
        set.add_write(format!("zk:election:{}:tally", instr.election_id()));
        return set;
    }
    if let Some(instr) = any.downcast_ref::<zk::Unshield>() {
        let asset_id = AssetId::of(instr.asset().clone(), instr.to().clone());
        add_asset_rw(&mut set, &asset_id);
        add_zk_asset_rw(&mut set, instr.asset());
        let Ok(key) = "zk.unshield.last".parse::<Name>() else {
            return AccessSet::global();
        };
        add_asset_def_detail_rw(&mut set, instr.asset(), &key);
        return set;
    }
    if let Some(ub) = any.downcast_ref::<UnregisterBox>() {
        match ub {
            UnregisterBox::Domain(u) => add_domain_rw(&mut set, &u.object),
            UnregisterBox::Account(u) => add_account_rw(&mut set, &u.object),
            UnregisterBox::AssetDefinition(u) => add_asset_def_rw(&mut set, &u.object),
            UnregisterBox::Nft(u) => add_nft_rw(&mut set, &u.object),
            UnregisterBox::Peer(_) => set = AccessSet::global(),
            UnregisterBox::Trigger(u) => add_trigger_rw(&mut set, &u.object),
            UnregisterBox::Role(u) => {
                add_role_rw(&mut set, &u.object);
                set.add_write(AUTHORIZATION_EPOCH_KEY.to_owned());
            }
        }
        return set;
    }

    // Grant
    if let Some(gb) = any.downcast_ref::<GrantBox>() {
        set.add_write(AUTHORIZATION_EPOCH_KEY.to_owned());
        match gb {
            GrantBox::Permission(g) => {
                add_account_rw(&mut set, &g.destination);
                set.add_write(key_perm_account(&g.destination, &g.object));
            }
            GrantBox::Role(g) => {
                add_account_rw(&mut set, &g.destination);
                set.add_read(key_role(&g.object));
                set.add_write(key_role_binding(&g.destination, &g.object));
            }
            GrantBox::RolePermission(g) => {
                add_role_rw(&mut set, &g.destination);
                set.add_write(key_perm_role(&g.destination, &g.object));
            }
        }
        return set;
    }

    // Revoke
    if let Some(rb) = any.downcast_ref::<RevokeBox>() {
        set.add_write(AUTHORIZATION_EPOCH_KEY.to_owned());
        match rb {
            RevokeBox::Permission(r) => {
                add_account_rw(&mut set, &r.destination);
                set.add_write(key_perm_account(&r.destination, &r.object));
            }
            RevokeBox::Role(r) => {
                add_account_rw(&mut set, &r.destination);
                set.add_read(key_role(&r.object));
                set.add_write(key_role_binding(&r.destination, &r.object));
            }
            RevokeBox::RolePermission(r) => {
                add_role_rw(&mut set, &r.destination);
                set.add_write(key_perm_role(&r.destination, &r.object));
            }
        }
        return set;
    }

    // Execute trigger
    if let Some(exe) = any.downcast_ref::<ExecuteTrigger>() {
        // Executing a trigger can mutate its own action (e.g., via Mint::trigger_repetitions or metadata updates),
        // so treat it as a full trigger write to avoid under-reporting conflicts.
        add_trigger_rw(&mut set, &exe.trigger);
        if let Some(view) = state_ro {
            let can_recurse = depth < max_depth && !visited_triggers.contains(&exe.trigger);
            if can_recurse {
                visited_triggers.insert(exe.trigger.clone());
                set.union_with(derive_from_trigger_executable(
                    &exe.trigger,
                    view,
                    visited_triggers,
                    depth.saturating_add(1),
                    max_depth,
                ));
            }
        }
        return set;
    }
    if let Some(act) =
        any.downcast_ref::<iroha_data_model::isi::staking::ActivatePublicLaneValidator>()
    {
        add_public_lane_validator_rw(&mut set, act.lane_id, &act.validator);
        return set;
    }
    if let Some(exit) =
        any.downcast_ref::<iroha_data_model::isi::staking::ExitPublicLaneValidator>()
    {
        add_public_lane_validator_rw(&mut set, exit.lane_id, &exit.validator);
        return set;
    }

    // Fallback: unknown instruction kind — be conservative.
    AccessSet::global()
}

fn derive_from_trigger_executable<R>(
    trigger_id: &TriggerId,
    state_ro: &R,
    visited_triggers: &mut BTreeSet<TriggerId>,
    depth: u8,
    max_depth: u8,
) -> AccessSet
where
    R: StateReadOnly + QueryStateSource,
{
    let mut set = AccessSet::new();
    let triggers = state_ro.world().triggers();
    let Some(executable) = triggers.inspect_by_id(trigger_id, |action| action.executable().clone())
    else {
        return set;
    };
    match executable {
        ExecutableRef::Instructions(instructions) => {
            for instr in instructions.as_ref() {
                set.union_with(derive_from_instruction(
                    instr,
                    Some(state_ro),
                    visited_triggers,
                    depth,
                    max_depth,
                ));
            }
        }
        ExecutableRef::ContractCall(invocation) => {
            if let Some(record) =
                code::fetch_bound_contract_record(state_ro, &invocation.contract_address)
                && record.code_hash == invocation.expected_code_hash
                && let Some((hinted, _source)) = manifest_access_set(
                    &record.manifest,
                    record.code_hash,
                    record.code_bytes.as_ref(),
                    state_ro.pipeline().access_set_cache_enabled,
                    Some(invocation.entrypoint.as_str()),
                )
            {
                set.union_with(hinted);
            } else {
                set.union_with(AccessSet::global());
            }
        }
        ExecutableRef::Ivm(hash) => {
            let Some(code) = triggers.get_original_contract(&hash) else {
                return set;
            };
            if let Some(hinted) = derive_access_from_ivm_trigger(code, state_ro) {
                set.union_with(hinted);
            }
        }
    }
    set
}

fn derive_access_from_ivm_trigger<R>(
    bytecode: &iroha_data_model::transaction::IvmBytecode,
    state_ro: &R,
) -> Option<AccessSet>
where
    R: StateReadOnly + QueryStateSource,
{
    let bytecode_ref = bytecode.as_ref();
    ivm::ProgramMetadata::parse(bytecode_ref).ok()?;
    let code_hash = ivm::contract_code_hash(bytecode_ref);
    let manifest = state_ro.world().contract_manifests().get(&code_hash)?;
    manifest_access_set(
        manifest,
        code_hash,
        bytecode_ref,
        state_ro.pipeline().access_set_cache_enabled,
        None,
    )
    .map(|(set, _source)| set)
}

fn key_account(id: &AccountId) -> AccessKey {
    format!("account:{id}")
}
fn key_account_detail(id: &AccountId, key: &Name) -> AccessKey {
    let mut s = String::new();
    let _ = write!(s, "account.detail:{id}:{key}");
    s
}
fn key_domain(id: &DomainId) -> AccessKey {
    format!("domain:{id}")
}
fn key_domain_detail(id: &DomainId, key: &Name) -> AccessKey {
    format!("domain.detail:{id}:{key}")
}
fn key_asset_def(id: &AssetDefinitionId) -> AccessKey {
    format!("asset_def:{id}")
}
fn key_asset_def_detail(id: &AssetDefinitionId, key: &Name) -> AccessKey {
    format!("asset_def.detail:{id}:{key}")
}
fn key_asset(id: &AssetId) -> AccessKey {
    format!("asset:{id}")
}
fn key_zk_asset(id: &AssetDefinitionId) -> AccessKey {
    format!("zk_asset:{id}")
}
fn key_nft(id: &NftId) -> AccessKey {
    format!("nft:{id}")
}
fn key_nft_detail(id: &NftId, key: &Name) -> AccessKey {
    format!("nft.detail:{id}:{key}")
}
fn key_rwa(id: &RwaId) -> AccessKey {
    format!("rwa:{id}")
}
fn key_rwa_detail(id: &RwaId, key: &Name) -> AccessKey {
    format!("rwa.detail:{id}:{key}")
}

fn add_account_r(set: &mut AccessSet, id: &AccountId) {
    set.add_read(ACCOUNT_WILDCARD_KEY.to_owned());
    set.add_read(key_account(id));
}
fn add_domain_r(set: &mut AccessSet, id: &DomainId) {
    set.add_read(key_domain(id));
}
fn add_account_rw(set: &mut AccessSet, id: &AccountId) {
    set.add_read(ACCOUNT_WILDCARD_KEY.to_owned());
    set.add_write(ACCOUNT_WILDCARD_KEY.to_owned());
    let k = key_account(id);
    set.add_read(k.clone());
    set.add_write(k);
}
fn add_account_detail_rw(set: &mut AccessSet, id: &AccountId, key: &Name) {
    set.add_read(key_account(id));
    let d = key_account_detail(id, key);
    set.add_read(d.clone());
    set.add_write(d);
}
fn add_domain_rw(set: &mut AccessSet, id: &DomainId) {
    let k = key_domain(id);
    set.add_read(k.clone());
    set.add_write(k);
}
fn add_domain_detail_rw(set: &mut AccessSet, id: &DomainId, key: &Name) {
    set.add_read(key_domain(id));
    let d = key_domain_detail(id, key);
    set.add_read(d.clone());
    set.add_write(d);
}
fn add_asset_def_rw(set: &mut AccessSet, id: &AssetDefinitionId) {
    set.add_read(ASSET_DEF_WILDCARD_KEY.to_owned());
    set.add_write(ASSET_DEF_WILDCARD_KEY.to_owned());
    if let Some(domain) = id.try_domain() {
        add_domain_r(set, domain);
    }
    let k = key_asset_def(id);
    set.add_read(k.clone());
    set.add_write(k);
}
fn add_asset_def_r(set: &mut AccessSet, id: &AssetDefinitionId) {
    set.add_read(ASSET_DEF_WILDCARD_KEY.to_owned());
    if let Some(domain) = id.try_domain() {
        add_domain_r(set, domain);
    }
    set.add_read(key_asset_def(id));
}
fn add_asset_def_detail_rw(set: &mut AccessSet, id: &AssetDefinitionId, key: &Name) {
    if let Some(domain) = id.try_domain() {
        add_domain_r(set, domain);
    }
    set.add_read(key_asset_def(id));
    let d = key_asset_def_detail(id, key);
    set.add_read(d.clone());
    set.add_write(d);
}
fn add_asset_rw(set: &mut AccessSet, id: &AssetId) {
    set.add_read(ASSET_WILDCARD_KEY.to_owned());
    set.add_write(ASSET_WILDCARD_KEY.to_owned());
    let k = key_asset(id);
    set.add_read(k.clone());
    set.add_write(k);
    // Asset operations rely on the owning account/domain and definition state.
    add_account_r(set, id.account());
    if let Some(domain) = id.definition().try_domain() {
        add_domain_r(set, domain);
    }
    add_asset_def_r(set, id.definition());
}
fn add_zk_asset_rw(set: &mut AccessSet, id: &AssetDefinitionId) {
    let k = key_zk_asset(id);
    set.add_read(k.clone());
    set.add_write(k);
}
fn add_nft_rw(set: &mut AccessSet, id: &NftId) {
    let k = key_nft(id);
    set.add_read(k.clone());
    set.add_write(k);
}
fn add_nft_detail_rw(set: &mut AccessSet, id: &NftId, key: &Name) {
    set.add_read(key_nft(id));
    let d = key_nft_detail(id, key);
    set.add_read(d.clone());
    set.add_write(d);
}
fn add_rwa_rw(set: &mut AccessSet, id: &RwaId) {
    let k = key_rwa(id);
    set.add_read(k.clone());
    set.add_write(k);
}
fn add_rwa_detail_rw(set: &mut AccessSet, id: &RwaId, key: &Name) {
    set.add_read(key_rwa(id));
    let d = key_rwa_detail(id, key);
    set.add_read(d.clone());
    set.add_write(d);
}

fn key_role(id: &RoleId) -> AccessKey {
    format!("role:{id}")
}
fn key_role_binding(account: &AccountId, role: &RoleId) -> AccessKey {
    format!("role.binding:{account}:{role}")
}
fn key_perm_account(account: &AccountId, perm: &permission::Permission) -> AccessKey {
    format!("perm.account:{}:{}", account, perm.name())
}
fn key_perm_role(role: &RoleId, perm: &permission::Permission) -> AccessKey {
    format!("perm.role:{}:{}", role, perm.name())
}
fn add_role_rw(set: &mut AccessSet, id: &RoleId) {
    let k = key_role(id);
    set.add_read(k.clone());
    set.add_write(k);
}
fn key_trigger(id: &TriggerId) -> AccessKey {
    format!("trigger:{id}")
}
fn key_trigger_repetitions(id: &TriggerId) -> AccessKey {
    format!("trigger.repetitions:{id}")
}
fn key_public_lane_validator(lane: LaneId, validator: &AccountId) -> AccessKey {
    format!("nexus.validator:{lane}:{validator}")
}
fn add_public_lane_validator_rw(set: &mut AccessSet, lane: LaneId, validator: &AccountId) {
    let k = key_public_lane_validator(lane, validator);
    set.add_read(k.clone());
    set.add_write(k);
}
fn add_trigger_rw(set: &mut AccessSet, id: &TriggerId) {
    let key = key_trigger(id);
    set.add_read(key.clone());
    set.add_write(key);
    set.add_write(key_trigger_repetitions(id));
}

fn tx_gas_limit(tx: &SignedTransaction) -> Result<u64, String> {
    let gas_limit = parse_gas_limit(tx.metadata()).map_err(|err| err.to_string())?;
    gas_limit.ok_or_else(|| "missing gas_limit in transaction metadata".to_owned())
}

fn derive_from_ivm_dynamic<R>(
    bytecode: &[u8],
    authority: &AccountId,
    metadata: &Metadata,
    state_ro: &R,
    gas_limit: u64,
) -> Result<AccessSet, String>
where
    R: StateReadOnly + QueryStateSource,
{
    let selector = crate::executor::requested_contract_entrypoint(metadata)
        .map_err(|error| error.to_string())?;
    let authorization = if let Some(selector) = selector.as_deref() {
        let code_hash = ivm::contract_code_hash(bytecode);
        let identity = crate::executor::require_raw_contract_runtime_identity(
            state_ro.world(),
            code_hash,
            metadata,
        )
        .map_err(|error| error.to_string())?;
        let prepared = ivm::prepare_contract(Arc::<[u8]>::from(bytecode))
            .map_err(|error| format!("failed to prepare raw contract artifact: {error}"))?;
        if prepared.code_hash() != identity.code_hash {
            return Err("raw contract bytecode no longer matches its live binding".to_owned());
        }
        Some(
            crate::executor::authorize_prepared_raw_contract_selector(
                state_ro.world(),
                authority,
                &prepared,
                selector,
                &identity,
            )
            .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };
    let contract_call_context =
        parse_contract_call_execution_context(metadata, bytecode, gas_limit, authorization)?;
    derive_from_ivm_dynamic_with_context(
        bytecode,
        authority,
        contract_call_context,
        state_ro,
        gas_limit,
    )
}

fn derive_from_ivm_dynamic_with_context<R>(
    bytecode: &[u8],
    authority: &AccountId,
    contract_call_context: Option<ContractCallExecutionContext>,
    state_ro: &R,
    gas_limit: u64,
) -> Result<AccessSet, String>
where
    R: StateReadOnly + QueryStateSource,
{
    // Execute VM with CoreHost to collect queued ISIs; do not apply.
    ivm::ProgramMetadata::parse(bytecode).map_err(|e| format!("ivm.metadata: {e}"))?;
    if let Some(context) = contract_call_context.as_ref() {
        match (&context.entrypoint, &context.authorization) {
            (Some(entrypoint), Some(authorization)) => {
                if authorization.entrypoint != *entrypoint
                    || authorization.permission != context.entrypoint_permission
                {
                    return Err(
                        "contract prepass authorization does not match the selected entrypoint"
                            .to_owned(),
                    );
                }
                authorization
                    .validate_for_authority(state_ro.world(), authority)
                    .map_err(|error| error.to_string())?;
            }
            (Some(_), None) => {
                return Err(
                    "contract entrypoint prepass requires an authorized live contract binding"
                        .to_owned(),
                );
            }
            (None, Some(_)) => {
                return Err(
                    "legacy IVM prepass must not carry contract entrypoint authorization"
                        .to_owned(),
                );
            }
            (None, None) => {}
        }
    }
    let mut vm = ivm::IVM::new(gas_limit);
    // Supply accounts snapshot for vendor helpers to become deterministic.
    let accounts = state_ro.accounts_snapshot();
    let mut host = if let Some(context) = contract_call_context.as_ref() {
        crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts_and_argument_record(
            authority.clone(),
            Arc::clone(&accounts),
            context.argument_record.clone(),
        )
    } else {
        crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts(
            authority.clone(),
            Arc::clone(&accounts),
        )
    }
    .with_access_logging();
    #[cfg(feature = "telemetry")]
    host.set_telemetry(state_ro.metrics().clone());
    host.set_crypto_config(state_ro.crypto());
    host.set_zk_config(state_ro.zk());
    host.set_public_inputs_from_parameters(state_ro.world().parameters());
    host.set_vrf_epoch_seeds_from_world(state_ro.world());
    host.set_query_state(state_ro);
    host.set_chain_id(state_ro.chain_id());
    if let Some(authorization) = contract_call_context
        .as_ref()
        .and_then(|context| context.authorization.as_ref())
    {
        let contract_subject = crate::smartcontracts::code::bound_contract_subject_from_world(
            state_ro.world(),
            &authorization.contract_address,
        )
        .ok_or_else(|| {
            format!(
                "contract instance `{}` has no valid subject binding",
                authorization.contract_address
            )
        })?;
        host.bind_contract_runtime_context(contract_subject, authorization.clone());
    }
    host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
        .map_err(|e| format!("ivm.zk_snapshots: {e}"))?;
    host.begin_tx(&ivm::parallel::StateAccessSet::default())
        .map_err(|e| format!("ivm.begin_tx: {e}"))?;
    vm.load_program(bytecode)
        .map_err(|e| format!("ivm.load_program: {e}"))?;
    vm.set_gas_limit(gas_limit);
    apply_contract_call_execution_context(&mut vm, contract_call_context.as_ref())
        .map_err(|e| format!("ivm.contract_call: {e}"))?;
    vm.run_with_host(&mut host)
        .map_err(|e| format!("ivm.run: {e}"))?;
    let mut set = AccessSet::new();
    let mut access_log: Option<ivm::host::AccessLog> = None;
    let max_depth = state_ro
        .world()
        .parameters()
        .smart_contract()
        .execution_depth();
    let mut visited_triggers = BTreeSet::new();
    for isi in host.drain_instructions() {
        set.union_with(derive_from_instruction(
            &isi,
            Some(state_ro),
            &mut visited_triggers,
            0,
            max_depth,
        ));
    }
    if host.access_logging_supported() {
        access_log = Some(
            host.finish_tx()
                .map_err(|e| format!("ivm.finish_tx: {e}"))?,
        );
    }
    if let Some(log) = access_log {
        merge_access_log(&mut set, &log);
    }
    if contract_call_context
        .as_ref()
        .and_then(|context| context.entrypoint_permission.as_ref())
        .is_some()
    {
        set.add_read(AUTHORIZATION_EPOCH_KEY.to_owned());
    }
    if set.read_keys.is_empty() && set.write_keys.is_empty() {
        // No syscalls or only helper syscalls: be conservative.
        return Ok(AccessSet::global());
    }
    Ok(set)
}

fn merge_access_log(set: &mut AccessSet, log: &ivm::host::AccessLog) {
    for key in &log.read_keys {
        set.add_read(access_key_from_state_log(key));
    }
    for key in &log.write_keys {
        set.add_write(access_key_from_state_log(key));
    }
}

fn access_key_from_state_log(key: &str) -> AccessKey {
    if key.starts_with("state:") {
        key.to_owned()
    } else {
        format!("state:{key}")
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use iroha_data_model::{
        isi::Log,
        level::Level,
        transaction::{Executable, IvmBytecode, TransactionBuilder},
    };

    use super::*;
    use crate::smartcontracts::Execute;
    use crate::state::{State, World};

    const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";
    const TEST_GAS_LIMIT: u64 = 50_000_000;

    fn canonical_test_sccp_payload_bytes(payload: &iroha_sccp::SccpPayloadV1) -> Vec<u8> {
        iroha_sccp::canonical_sccp_payload_bytes(payload)
            .expect("valid SCCP access-set fixture payload encodes")
    }

    fn wonderland_domain_id() -> DomainId {
        DomainId::try_new("wonderland", "universal").expect("static domain id")
    }

    fn new_wonderland_account(account_id: &AccountId) -> iroha_data_model::account::NewAccount {
        Account::new(account_id.clone())
    }

    fn build_wonderland_account(account_id: &AccountId) -> Account {
        new_wonderland_account(account_id).build(account_id)
    }

    fn insert_gas_limit(metadata: &mut iroha_data_model::metadata::Metadata) {
        metadata.insert(
            Name::from_str("gas_limit").expect("static gas_limit key"),
            iroha_primitives::json::Json::new(TEST_GAS_LIMIT),
        );
    }

    fn sccp_transfer_payload(
        nonce: u64,
        source_domain: u32,
        target_domain: u32,
    ) -> iroha_sccp::SccpPayloadV1 {
        iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain,
            dest_domain: target_domain,
            nonce,
            route_revision: 1,
            asset_home_domain: source_domain,
            asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            asset_id: b"xor".to_vec(),
            amount: 5,
            sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
            recipient: vec![0x22; 20],
            route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
                .as_bytes()
                .to_vec(),
        })
    }

    fn bridge_proof_fixture(seed: u8) -> iroha_data_model::bridge::BridgeProof {
        iroha_data_model::bridge::BridgeProof {
            range: iroha_data_model::bridge::BridgeProofRange {
                start_height: 20 + u64::from(seed),
                end_height: 20 + u64::from(seed),
            },
            payload: iroha_data_model::bridge::BridgeProofPayload::TransparentZk(
                iroha_data_model::bridge::BridgeTransparentProof {
                    verifier_manifest_hash: [0xB0 | (seed & 0x0F); 32],
                    proof: iroha_data_model::proof::ProofBox::new(
                        format!("halo2/mock/{seed}").into(),
                        vec![0xCA, 0xFE, seed],
                    ),
                    recursion_depth: Some(1),
                },
            ),
        }
    }

    fn bridge_receipt_fixture(proof_hash: [u8; 32]) -> iroha_data_model::bridge::BridgeReceipt {
        iroha_data_model::bridge::BridgeReceipt {
            lane: LaneId::SINGLE,
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: None,
            proof_hash,
            amount: 1,
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        }
    }

    fn sccp_bridge_proof_fixture(
        _nonce: u64,
        proof_seed: u8,
    ) -> iroha_data_model::bridge::BridgeProof {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        iroha_data_model::bridge::BridgeProof {
            range: iroha_data_model::bridge::BridgeProofRange {
                start_height: fixture.request.public_inputs.finality_height,
                // Vary only the outer wrapper. Scheduler derivation must stay
                // conservative without inspecting either nested artifact.
                end_height: fixture
                    .request
                    .public_inputs
                    .finality_height
                    .saturating_add(u64::from(proof_seed)),
            },
            payload: iroha_data_model::bridge::BridgeProofPayload::SccpDestination(
                fixture.bridge_proof,
            ),
        }
    }
    fn native_sccp_bridge_proof_fixture() -> (
        iroha_data_model::bridge::BridgeProof,
        iroha_data_model::bridge::SccpLaneIdV1,
        [u8; 32],
        AccessKey,
    ) {
        let (native, identity, anchor) = iroha_sccp::sccp_native_ethereum_inbound_test_fixture_v1();
        let validated =
            iroha_sccp::verify_sccp_native_inbound_message_proof_v1(&native, &identity, anchor)
                .expect("native access fixture must validate");
        let backend = native.source.proof.backend();
        let encoded = iroha_sccp::encode_sccp_native_inbound_message_proof_v1(&native)
            .expect("native access fixture must encode");
        let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
            iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
            iroha_data_model::bridge::SccpRouteActivationV1::Staged,
        );
        let route_configuration_hash = route
            .route_configuration_hash()
            .expect("exact native access route configuration");
        let proof = iroha_data_model::bridge::BridgeProof {
            range: iroha_data_model::bridge::BridgeProofRange {
                start_height: validated.source_finality.height,
                end_height: validated.source_finality.height,
            },
            payload: iroha_data_model::bridge::BridgeProofPayload::NativeProtocol(
                iroha_data_model::bridge::BridgeNativeProtocolProofV1 {
                    backend,
                    route_configuration_hash,
                    encoded_envelope: encoded,
                },
            ),
        };
        let key = key_sccp_native_bridge_message(
            validated.message_key.lane,
            validated.message_key.message_id,
        );
        (
            proof,
            validated.message_key.lane,
            validated.message_key.message_id,
            key,
        )
    }

    fn test_contract_artifact(
        code: Vec<u8>,
        access_set_hints: Option<iroha_data_model::smart_contract::manifest::AccessSetHints>,
        entrypoints: Vec<EntrypointDescriptor>,
    ) -> (Vec<u8>, IrohaHash, ContractManifest) {
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 10_000,
            abi_version: 1,
        };
        let embedded_entrypoints = entrypoints
            .iter()
            .map(|entrypoint| ivm::EmbeddedEntrypointDescriptor {
                name: entrypoint.name.clone(),
                kind: entrypoint.kind,
                params: entrypoint.params.clone(),
                argument_schema: entrypoint.argument_schema.clone(),
                return_type: entrypoint.return_type.clone(),
                return_schema: entrypoint.return_schema.clone(),
                permission: entrypoint.permission.clone(),
                read_keys: entrypoint.read_keys.clone(),
                write_keys: entrypoint.write_keys.clone(),
                access_hints_complete: entrypoint.access_hints_complete,
                access_hints_skipped: entrypoint.access_hints_skipped.clone(),
                triggers: entrypoint.triggers.clone(),
                entry_pc: 0,
            })
            .collect();
        let interface = ivm::EmbeddedContractInterfaceV1 {
            seiyaku_name: "TestContract".to_owned(),
            compiler_fingerprint: "access-test".to_owned(),
            abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints,
            kotoba: Vec::new(),
            entrypoints: embedded_entrypoints,
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        let mut artifact = meta.encode();
        artifact.extend_from_slice(&interface.encode_section());
        artifact.extend_from_slice(&code);
        let verified = ivm::verify_contract_artifact(&artifact).expect("valid test artifact");
        (artifact, verified.code_hash, verified.manifest)
    }

    fn default_test_entrypoint() -> EntrypointDescriptor {
        EntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: Some("ExecuteContract".to_owned()),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        }
    }

    #[test]
    fn select_entrypoint_requires_an_explicit_selector() {
        let mut main = default_test_entrypoint();
        main.read_keys = vec!["state:main".to_owned()];
        let mut run = default_test_entrypoint();
        run.name = "run".to_owned();
        run.read_keys = vec!["state:run".to_owned()];
        let mut hajimari = default_test_entrypoint();
        hajimari.name = "hajimari".to_owned();
        hajimari.kind = iroha_data_model::smart_contract::manifest::EntryPointKind::Hajimari;
        hajimari.read_keys = vec!["state:hajimari".to_owned()];
        let mut view_main = default_test_entrypoint();
        view_main.kind = iroha_data_model::smart_contract::manifest::EntryPointKind::View;

        let entrypoints = vec![run.clone(), hajimari.clone(), main.clone()];
        assert!(select_entrypoint(&entrypoints, None).is_none());
        assert_eq!(
            select_entrypoint(&entrypoints, Some("run")).map(|entrypoint| entrypoint.name.as_str()),
            Some("run")
        );

        let non_main_entrypoints = vec![run, hajimari];
        assert!(select_entrypoint(&non_main_entrypoints, None).is_none());
        assert!(select_entrypoint(&[view_main], None).is_none());
    }

    #[test]
    fn lifecycle_calls_write_the_instance_marker_scheduler_key() {
        let (authority, keypair) = iroha_test_samples::gen_account_in("wonderland");
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            91,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let marker_key = access_key_from_state_log(
            crate::smartcontracts::code::contract_lifecycle_state_key(&contract_address).as_ref(),
        );

        for (entrypoint, writes_marker) in [
            ("run", false),
            ("hajimari", true),
            ("始まり", true),
            ("kaizen", true),
            ("改善", true),
        ] {
            let transaction = TransactionBuilder::new("chain".parse().unwrap(), authority.clone())
                .with_executable(Executable::ContractCall(ContractInvocation {
                    contract_address: contract_address.clone(),
                    expected_code_hash: iroha_crypto::Hash::new(entrypoint.as_bytes()),
                    entrypoint: entrypoint.to_owned(),
                    arguments: None,
                }))
                .sign(keypair.private_key());
            let (set, _) = with_stateful_admission_keys(&transaction, AccessSet::new(), None);
            assert!(set.read_keys.contains(&marker_key));
            assert_eq!(
                set.write_keys.contains(&marker_key),
                writes_marker,
                "unexpected lifecycle marker mode for `{entrypoint}`"
            );
        }
    }

    fn state_get_test_program() -> Vec<u8> {
        let mut program = ivm::ProgramMetadata::default().encode();
        program.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_STATE_GET)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        program
    }

    #[test]
    fn entrypoint_hints_require_explicit_complete_unskipped_attestation() {
        let program = state_get_test_program();
        let mut entrypoint = default_test_entrypoint();
        entrypoint.read_keys = vec!["state:alpha".to_owned()];
        entrypoint.write_keys = vec!["state:beta".to_owned()];

        assert!(
            entrypoint_access_set_if_safe(&program, &entrypoint).is_none(),
            "exact CNTR keys are not a bytecode proof of the runtime state path"
        );

        entrypoint.read_keys = vec!["state:*".to_owned()];
        entrypoint.write_keys.clear();
        assert!(entrypoint_access_set_if_safe(&program, &entrypoint).is_some());

        for completion in [None, Some(false)] {
            entrypoint.access_hints_complete = completion;
            assert!(entrypoint_access_set_if_safe(&program, &entrypoint).is_none());
        }

        entrypoint.access_hints_complete = Some(true);
        entrypoint.access_hints_skipped = vec!["dynamic state path".to_owned()];
        assert!(entrypoint_access_set_if_safe(&program, &entrypoint).is_none());
    }

    #[test]
    fn protected_entrypoint_reads_the_authorization_scheduler_epoch() {
        let mut program = ivm::ProgramMetadata::default().encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut entrypoint = default_test_entrypoint();
        entrypoint.permission = Some("CanRunGuardedEntrypoint".to_owned());

        let set = entrypoint_access_set_if_safe(&program, &entrypoint)
            .expect("a complete local entrypoint has a static access set");
        assert!(
            set.read_keys.contains(AUTHORIZATION_EPOCH_KEY),
            "permission checks must conflict with every grant, revoke, and role mutation"
        );
    }

    #[test]
    fn dynamic_raw_contract_prepass_rejects_identityless_dispatch_before_argument_decode() {
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let state = State::new(
            World::with([domain], [account], []),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let mut entrypoint = default_test_entrypoint();
        entrypoint.permission = None;
        let code = ivm::encoding::wide::encode_halt().to_le_bytes().to_vec();
        let (artifact, _, _) = test_contract_artifact(code, None, vec![entrypoint]);
        let mut metadata = Metadata::default();
        metadata.insert(
            "contract_entrypoint".parse().unwrap(),
            iroha_primitives::json::Json::new("main"),
        );
        metadata.insert(
            "contract_payload".parse().unwrap(),
            iroha_primitives::json::Json::new(1_u64),
        );

        ivm::reset_argument_record_decode_count();
        let error =
            derive_from_ivm_dynamic(&artifact, &alice, &metadata, &state.view(), TEST_GAS_LIMIT)
                .expect_err("selected raw contract entrypoints require a live instance identity");

        assert!(
            error.contains("requires a live contract_address or contract_alias binding"),
            "unexpected prepass rejection: {error}"
        );
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "authorization must fail before canonical argument decoding"
        );
    }

    #[test]
    fn incomplete_entrypoint_hints_do_not_fall_through_to_contract_hints() {
        use iroha_data_model::smart_contract::manifest::AccessSetHints;

        let program = state_get_test_program();
        let code_hash = ivm::contract_code_hash(&program);
        let contract_hints = AccessSetHints {
            read_keys: vec!["state:contract-read".to_owned()],
            write_keys: vec!["state:contract-write".to_owned()],
            dynamic_reads: Vec::new(),
            dynamic_writes: Vec::new(),
        };

        for (completion, skipped) in [
            (None, Vec::new()),
            (Some(false), Vec::new()),
            (Some(true), vec!["dynamic state path".to_owned()]),
        ] {
            let mut entrypoint = default_test_entrypoint();
            entrypoint.read_keys = vec!["state:entry-read".to_owned()];
            entrypoint.write_keys = vec!["state:entry-write".to_owned()];
            entrypoint.access_hints_complete = completion;
            entrypoint.access_hints_skipped = skipped;
            let manifest = ContractManifest {
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: None,
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: Some(contract_hints.clone()),
                entrypoints: Some(vec![entrypoint]),
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            };

            assert!(
                manifest_access_set(&manifest, code_hash, &program, false, Some("main")).is_none()
            );
        }
    }

    #[test]
    fn dynamic_manifest_hints_are_not_scheduler_authoritative() {
        use iroha_data_model::smart_contract::manifest::{AccessSetHints, DynamicAccessHint};

        let program = state_get_test_program();
        let code_hash = ivm::contract_code_hash(&program);
        for dynamic_hint in [
            DynamicAccessHint {
                base_key: "state:Victim".to_owned(),
                key_type: "forged-key-type".to_owned(),
                bound_kind: "forged-exact-bound".to_owned(),
                max_keys: 1,
            },
            DynamicAccessHint {
                base_key: "state:Unrelated/forged-child".to_owned(),
                key_type: String::new(),
                bound_kind: String::new(),
                max_keys: u32::MAX,
            },
        ] {
            for hints in [
                AccessSetHints {
                    read_keys: Vec::new(),
                    write_keys: Vec::new(),
                    dynamic_reads: vec![dynamic_hint.clone()],
                    dynamic_writes: Vec::new(),
                },
                AccessSetHints {
                    read_keys: Vec::new(),
                    write_keys: Vec::new(),
                    dynamic_reads: Vec::new(),
                    dynamic_writes: vec![dynamic_hint.clone()],
                },
            ] {
                assert!(manifest_hint_access_set_if_safe(&program, &hints).is_none());

                let manifest = ContractManifest {
                    seiyaku_name: Some("DynamicHintsAreAdvisory".to_owned()),
                    code_hash: Some(code_hash),
                    abi_hash: None,
                    compiler_fingerprint: Some("malicious-cntr".to_owned()),
                    features_bitmap: Some(0),
                    access_set_hints: Some(hints),
                    entrypoints: None,
                    states: None,
                    kotoba: None,
                    error_codes: None,
                    provenance: None,
                };
                assert!(
                    manifest_access_set(&manifest, code_hash, &program, false, None).is_none(),
                    "dynamic base/key/bound claims must never become a scheduler access set"
                );
            }
        }
    }

    #[test]
    fn compiler_dynamic_state_writes_and_helper_writes_fall_back_to_global() {
        let source = r#"
seiyaku DynamicAccessCounter {
  state StateMap<int, int> Counters;

  fn bump_hidden(int key, int delta) {
    let current = Counters.get(key).unwrap_or(0);
    Counters[key] = current + delta;
  }

  kotoage fn bump_direct(int key, int delta) authorize("CanEnactGovernance") {
    let current = Counters.get(key).unwrap_or(0);
    Counters[key] = current + delta;
  }

  kotoage fn bump_via_helper(int key, int delta) authorize("CanEnactGovernance") {
    bump_hidden(key, delta);
  }
}
"#;
        let (program, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(source)
            .expect("compile dynamic-access contract");
        let code_hash = ivm::contract_code_hash(&program);
        let entrypoints = manifest
            .entrypoints
            .as_deref()
            .expect("compiler manifest entrypoints");

        for entrypoint_name in ["bump_direct", "bump_via_helper"] {
            let entrypoint = entrypoints
                .iter()
                .find(|entrypoint| entrypoint.name == entrypoint_name)
                .unwrap_or_else(|| panic!("missing `{entrypoint_name}` entrypoint"));
            assert_eq!(entrypoint.access_hints_complete, Some(true));
            assert!(entrypoint.access_hints_skipped.is_empty());
            assert!(entrypoint.read_keys.contains(&"state:Counters".to_owned()));
            assert!(entrypoint.write_keys.contains(&"state:Counters".to_owned()));
            assert!(
                manifest_access_set(&manifest, code_hash, &program, false, Some(entrypoint_name),)
                    .is_none(),
                "dynamic StateMap base hints must not be trusted as exact scheduler keys"
            );
        }

        let (alice, key_pair) = iroha_test_samples::gen_account_in("wonderland");
        for entrypoint_name in ["bump_direct", "bump_via_helper"] {
            let mut metadata = Metadata::default();
            metadata.insert(
                MANIFEST_METADATA_KEY
                    .parse()
                    .expect("manifest metadata key"),
                iroha_primitives::json::Json::new(manifest.clone()),
            );
            metadata.insert(
                "contract_entrypoint"
                    .parse()
                    .expect("contract entrypoint metadata key"),
                iroha_primitives::json::Json::new(entrypoint_name.to_owned()),
            );
            insert_gas_limit(&mut metadata);
            let transaction = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
                .with_metadata(metadata)
                .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program.clone())))
                .sign(key_pair.private_key());

            let (set, source) = derive_for_transaction_with_source::<crate::state::StateView<'_>>(
                &transaction,
                None,
                IvmStrategy::Conservative,
            );
            assert!(set.write_keys.contains("*"));
            assert!(!set.write_keys.contains("state:Counters"));
            assert_eq!(source, Some(AccessSetSource::ConservativeFallback));
        }
    }

    fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
        v.extend_from_slice(&type_id.to_be_bytes());
        v.push(1u8); // version
        let payload_len =
            u32::try_from(payload.len()).expect("payload length must fit into u32 for TLV");
        v.extend_from_slice(&payload_len.to_be_bytes());
        v.extend_from_slice(payload);
        let h: [u8; 32] = IrohaHash::new(payload).into();
        v.extend_from_slice(&h);
        v
    }

    #[test]
    fn isi_access_transfer_and_mint() {
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let (bob, _) = iroha_test_samples::gen_account_in("wonderland");
        let domain_id = wonderland_domain_id();
        let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let src = AssetId::of(ad.clone(), alice.clone());

        let isis: Vec<iroha_data_model::isi::InstructionBox> = vec![
            Mint::asset_numeric(10u32, src.clone()).into(),
            Transfer::asset_numeric(src.clone(), 5u32, bob.clone()).into(),
        ];
        let exec = Executable::from_iter(isis);
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(exec)
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());

        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            None,
            IvmStrategy::Conservative,
        );
        let a_src = key_asset(&src);
        let a_dst = key_asset(&AssetId::of(ad, bob.clone()));
        let k_account_alice = key_account(&alice);
        let k_account_bob = key_account(&bob);
        let k_asset_def = key_asset_def(src.definition());
        let k_domain = key_domain(&domain_id);
        assert!(set.read_keys.contains(&a_src));
        assert!(set.write_keys.contains(&a_src));
        assert!(set.read_keys.contains(&a_dst));
        assert!(set.write_keys.contains(&a_dst));
        assert!(set.read_keys.contains(&k_account_alice));
        assert!(set.read_keys.contains(&k_account_bob));
        assert!(set.read_keys.contains(&k_asset_def));
        assert!(set.write_keys.contains(&k_asset_def));
        assert!(set.read_keys.contains(&k_domain));
    }

    #[test]
    fn simple_asset_transfer_batch_fast_path_matches_generic_walker() {
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let (bob, _) = iroha_test_samples::gen_account_in("wonderland");
        let (carol, _) = iroha_test_samples::gen_account_in("wonderland");
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let alice_asset = AssetId::of(asset_definition.clone(), alice.clone());
        let bob_asset = AssetId::of(asset_definition, bob.clone());
        let batch: Vec<InstructionBox> = vec![
            Transfer::asset_numeric(alice_asset, 5_u32, bob).into(),
            Transfer::asset_numeric(bob_asset, 2_u32, carol).into(),
        ];

        let fast = derive_simple_asset_transfer_batch(&batch)
            .expect("simple asset transfers should use the fast path");
        let mut generic = AccessSet::new();
        let mut visited_triggers = BTreeSet::new();
        for instruction in &batch {
            generic.union_with(derive_from_instruction(
                instruction,
                None::<&crate::state::StateView<'_>>,
                &mut visited_triggers,
                0,
                0,
            ));
        }

        assert_eq!(fast, generic);
        assert!(
            derive_simple_asset_transfer_batch(&[Log::new(Level::INFO, "noop".into()).into()])
                .is_none()
        );
    }

    #[test]
    fn log_instruction_has_no_access_keys() {
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_instructions([Log::new(Level::INFO, "hello".to_owned())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let authority = tx.authority().clone();

        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            None,
            IvmStrategy::Conservative,
        );

        assert_eq!(set.read_keys, [format!("account:{authority}")].into());
        assert_eq!(set.write_keys, [format!("tx.sequence:{authority}")].into());
    }

    #[test]
    fn record_sccp_message_access_uses_outbound_message_key() {
        let payload =
            sccp_transfer_payload(1, iroha_sccp::SCCP_DOMAIN_SORA, iroha_sccp::SCCP_DOMAIN_ETH);
        let instruction = InstructionBox::from(crate::bridge::test_record_sccp_message(
            canonical_test_sccp_payload_bytes(&payload),
        ));
        let mut visited_triggers = BTreeSet::new();

        let set = derive_from_instruction(
            &instruction,
            None::<&crate::state::StateView<'_>>,
            &mut visited_triggers,
            0,
            0,
        );

        let expected =
            key_sccp_outbound_message(&crate::bridge::test_sccp_outbound_message_key(&payload));
        assert_eq!(
            set.read_keys,
            BTreeSet::from([
                NEXUS_ACTIVE_LANE_CATALOG_KEY.to_owned(),
                SCCP_ON_CHAIN_REGISTRY_KEY.to_owned(),
            ])
        );
        assert_eq!(set.write_keys, BTreeSet::from([expected]));
    }

    #[test]
    fn record_sccp_message_access_separates_profiles_but_not_binding_rotations() {
        use iroha_data_model::bridge::{SccpLaneIdV1, SccpNetworkV1, SccpOutboundMessageContextV1};

        let payload =
            sccp_transfer_payload(9, iroha_sccp::SCCP_DOMAIN_SORA, iroha_sccp::SCCP_DOMAIN_ETH);
        let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
        let mainnet = SccpOutboundMessageContextV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::EthereumMainnet,
            },
            [0x31; 32],
            [0x41; 32],
        )
        .expect("mainnet context");
        let sepolia = SccpOutboundMessageContextV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::EthereumSepolia,
            },
            [0x32; 32],
            [0x42; 32],
        )
        .expect("Sepolia context");
        let rotated = SccpOutboundMessageContextV1::new(
            mainnet.lane,
            [0x33; 32],
            mainnet.route_configuration_hash,
        )
        .expect("rotated mainnet binding");

        let access_for = |context| {
            let instruction =
                InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                    context,
                    payload_bytes.clone(),
                ));
            let mut visited_triggers = BTreeSet::new();
            derive_from_instruction(
                &instruction,
                None::<&crate::state::StateView<'_>>,
                &mut visited_triggers,
                0,
                0,
            )
        };

        let mainnet_access = access_for(mainnet);
        let sepolia_access = access_for(sepolia);
        let rotated_access = access_for(rotated);
        assert_ne!(
            mainnet_access.write_keys, sepolia_access.write_keys,
            "same-domain exact profiles must not alias one scheduler key"
        );
        assert_eq!(
            mainnet_access.write_keys, rotated_access.write_keys,
            "binding rotation must not create a replay-distinct scheduler key"
        );
    }

    #[test]
    fn record_sccp_message_access_serializes_canonical_payload_only() {
        let payload =
            sccp_transfer_payload(3, iroha_sccp::SCCP_DOMAIN_SORA, iroha_sccp::SCCP_DOMAIN_ETH);
        let canonical_payload = canonical_test_sccp_payload_bytes(&payload);
        let binary =
            InstructionBox::from(crate::bridge::test_record_sccp_message(canonical_payload));
        let expected =
            key_sccp_outbound_message(&crate::bridge::test_sccp_outbound_message_key(&payload));

        let mut visited_triggers = BTreeSet::new();
        let set = derive_from_instruction(
            &binary,
            None::<&crate::state::StateView<'_>>,
            &mut visited_triggers,
            0,
            0,
        );
        assert_eq!(
            set.read_keys,
            BTreeSet::from([
                NEXUS_ACTIVE_LANE_CATALOG_KEY.to_owned(),
                SCCP_ON_CHAIN_REGISTRY_KEY.to_owned(),
            ])
        );
        assert_eq!(set.write_keys, BTreeSet::from([expected]));
    }

    #[test]
    fn record_sccp_message_access_serializes_invalid_or_non_sora_payloads() {
        let invalid = InstructionBox::from(crate::bridge::test_record_sccp_message(vec![0xFF]));
        let inbound_payload =
            sccp_transfer_payload(2, iroha_sccp::SCCP_DOMAIN_ETH, iroha_sccp::SCCP_DOMAIN_SORA);
        let inbound = InstructionBox::from(crate::bridge::test_record_sccp_message(
            canonical_test_sccp_payload_bytes(&inbound_payload),
        ));
        let hex_alias_payload =
            sccp_transfer_payload(4, iroha_sccp::SCCP_DOMAIN_SORA, iroha_sccp::SCCP_DOMAIN_ETH);
        let hex_alias = InstructionBox::from(crate::bridge::test_record_sccp_message(
            format!(
                "0x{}",
                hex::encode(canonical_test_sccp_payload_bytes(&hex_alias_payload))
            )
            .into_bytes(),
        ));

        for instruction in [&invalid, &inbound, &hex_alias] {
            let mut visited_triggers = BTreeSet::new();
            let set = derive_from_instruction(
                instruction,
                None::<&crate::state::StateView<'_>>,
                &mut visited_triggers,
                0,
                0,
            );
            assert_eq!(set, AccessSet::global());
        }
    }

    #[test]
    fn submit_bridge_proof_access_uses_canonical_proof_hash() {
        let proof = bridge_proof_fixture(5);
        let expected_hash = bridge_proof_hash(&proof).expect("fixture proof should encode");
        let instruction =
            InstructionBox::from(iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof));
        let mut visited_triggers = BTreeSet::new();

        let set = derive_from_instruction(
            &instruction,
            None::<&crate::state::StateView<'_>>,
            &mut visited_triggers,
            0,
            0,
        );

        assert!(set.read_keys.is_empty());
        assert_eq!(
            set.write_keys,
            BTreeSet::from([
                key_bridge_proof_hash(&expected_hash),
                key_bridge_backend(&bridge_proof_fixture(5).backend_label())
            ])
        );
    }

    #[test]
    fn bridge_receipt_access_conflicts_with_submitted_proof_hash() {
        let proof = bridge_proof_fixture(6);
        let proof_hash = bridge_proof_hash(&proof).expect("fixture proof should encode");
        let submit =
            InstructionBox::from(iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof));
        let receipt =
            InstructionBox::from(iroha_data_model::isi::bridge::RecordBridgeReceipt::new(
                bridge_receipt_fixture(proof_hash),
            ));
        let expected_key = key_bridge_proof_hash(&proof_hash);

        let mut visited_triggers = BTreeSet::new();
        let submit_set = derive_from_instruction(
            &submit,
            None::<&crate::state::StateView<'_>>,
            &mut visited_triggers,
            0,
            0,
        );
        assert!(!submit_set.read_keys.contains(NEXUS_ACTIVE_LANE_CATALOG_KEY));
        assert!(submit_set.write_keys.contains(&expected_key));

        let mut visited_triggers = BTreeSet::new();
        let receipt_set = derive_from_instruction(
            &receipt,
            None::<&crate::state::StateView<'_>>,
            &mut visited_triggers,
            0,
            0,
        );
        assert!(
            receipt_set
                .read_keys
                .contains(NEXUS_ACTIVE_LANE_CATALOG_KEY)
        );
        assert!(receipt_set.write_keys.contains(&expected_key));
    }

    #[test]
    fn submit_sccp_bridge_proof_access_is_global_and_performs_zero_crypto() {
        let first_proof = sccp_bridge_proof_fixture(7, 1);
        let second_proof = sccp_bridge_proof_fixture(7, 2);
        assert_ne!(
            bridge_proof_hash(&first_proof),
            bridge_proof_hash(&second_proof)
        );
        iroha_sccp::reset_sccp_destination_proof_work_counters_v1();
        reset_bridge_proof_hash_attempts();

        for proof in [first_proof, second_proof] {
            let instruction =
                InstructionBox::from(iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof));
            let mut visited_triggers = BTreeSet::new();
            let set = derive_from_instruction(
                &instruction,
                None::<&crate::state::StateView<'_>>,
                &mut visited_triggers,
                0,
                0,
            );
            assert_eq!(set, AccessSet::global());
        }
        assert_eq!(
            iroha_sccp::sccp_destination_proof_work_counters_v1(),
            iroha_sccp::SccpDestinationProofWorkCountersV1::default(),
        );
        assert_eq!(bridge_proof_hash_attempts(), 0);
    }

    #[test]
    fn submit_sccp_bridge_proof_access_routes_malformed_artifacts_globally_without_work() {
        let valid_proof = sccp_bridge_proof_fixture(8, 1);
        iroha_sccp::reset_sccp_destination_proof_work_counters_v1();
        reset_bridge_proof_hash_attempts();
        for (index, mut proof) in [valid_proof.clone(), valid_proof].into_iter().enumerate() {
            let iroha_data_model::bridge::BridgeProofPayload::SccpDestination(destination) =
                &mut proof.payload
            else {
                panic!("SCCP fixture must use the closed destination variant");
            };
            if index == 0 {
                destination.encoded_artifact = vec![0xFF];
            } else {
                destination.backend =
                    iroha_data_model::bridge::BridgeSccpDestinationProofBackendV1::TronGroth16Bn254;
            }
            let instruction =
                InstructionBox::from(iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof));
            let mut visited_triggers = BTreeSet::new();
            let set = derive_from_instruction(
                &instruction,
                None::<&crate::state::StateView<'_>>,
                &mut visited_triggers,
                0,
                0,
            );
            assert_eq!(set, AccessSet::global());
        }
        assert_eq!(
            iroha_sccp::sccp_destination_proof_work_counters_v1(),
            iroha_sccp::SccpDestinationProofWorkCountersV1::default(),
        );
        assert_eq!(bridge_proof_hash_attempts(), 0);
    }

    #[test]
    fn submit_native_sccp_proof_access_uses_exact_lane_and_message_id() {
        let (proof, lane, message_id, expected_key) = native_sccp_bridge_proof_fixture();
        let instruction = InstructionBox::from(
            iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()),
        );
        let mut visited_triggers = BTreeSet::new();
        let set = derive_from_instruction(
            &instruction,
            None::<&crate::state::StateView<'_>>,
            &mut visited_triggers,
            0,
            0,
        );

        assert_eq!(
            expected_key,
            format!(
                "sccp.bridge.native:{}:{}:{}",
                lane.source.profile_key(),
                lane.target.profile_key(),
                hex::encode(message_id)
            )
        );
        assert!(set.write_keys.contains(&expected_key));
        assert!(set.write_keys.contains(&key_bridge_proof_hash(
            &bridge_proof_hash(&proof).expect("native proof hash")
        )));
        assert!(
            set.write_keys
                .contains(&key_bridge_backend(&proof.backend_label()))
        );
    }

    #[test]
    fn submit_native_sccp_access_serializes_alternate_wrappers_but_not_other_lanes() {
        let (proof, lane, message_id, expected_key) = native_sccp_bridge_proof_fixture();
        let mut alternate = proof.clone();
        alternate.range.start_height = alternate.range.start_height.saturating_add(1);
        alternate.range.end_height = alternate.range.end_height.saturating_add(1);
        for proof in [proof, alternate] {
            let instruction =
                InstructionBox::from(iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof));
            let mut visited_triggers = BTreeSet::new();
            let set = derive_from_instruction(
                &instruction,
                None::<&crate::state::StateView<'_>>,
                &mut visited_triggers,
                0,
                0,
            );
            assert!(set.write_keys.contains(&expected_key));
        }

        let other_lane = iroha_data_model::bridge::SccpLaneIdV1 {
            source: iroha_data_model::bridge::SccpNetworkV1::EthereumSepolia,
            target: lane.target,
        };
        let other_key = key_sccp_native_bridge_message(other_lane, message_id);
        assert_ne!(expected_key, other_key);
    }

    #[test]
    fn submit_malformed_native_sccp_proof_access_is_global() {
        let (mut proof, _, _, _) = native_sccp_bridge_proof_fixture();
        let iroha_data_model::bridge::BridgeProofPayload::NativeProtocol(native) =
            &mut proof.payload
        else {
            panic!("native fixture payload")
        };
        native.encoded_envelope.push(0x00);
        let instruction =
            InstructionBox::from(iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof));
        let mut visited_triggers = BTreeSet::new();
        let set = derive_from_instruction(
            &instruction,
            None::<&crate::state::StateView<'_>>,
            &mut visited_triggers,
            0,
            0,
        );
        assert_eq!(set, AccessSet::global());
    }

    #[test]
    fn register_access_includes_domain_reads() {
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let domain_id = wonderland_domain_id();
        let account = new_wonderland_account(&alice);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let asset_def = AssetDefinition::numeric(asset_def_id.clone());

        let isis: Vec<iroha_data_model::isi::InstructionBox> = vec![
            Register::account(account).into(),
            Register::asset_definition(asset_def).into(),
        ];
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(Executable::from_iter(isis))
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());

        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            None,
            IvmStrategy::Conservative,
        );

        let k_domain = key_domain(&domain_id);
        let k_account = key_account(&alice);
        let k_asset_def = key_asset_def(&asset_def_id);
        assert!(set.read_keys.contains(&k_domain));
        assert!(set.read_keys.contains(&k_account));
        assert!(set.write_keys.contains(&k_account));
        assert!(set.read_keys.contains(&k_asset_def));
        assert!(set.write_keys.contains(&k_asset_def));
    }

    #[test]
    fn ivm_access_dynamic_prepass_set_account_detail_sentinel() {
        // World and state for view
        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);
        let view = state.view();

        // Program: GET_AUTHORITY; INPUT_PUBLISH_TLV (key/value); SET_ACCOUNT_DETAIL; HALT
        const LITERAL_DATA_START: i16 = 16;
        let key: Name = "cursor".parse().expect("key name");
        let key_payload = norito::to_bytes(&key).expect("encode key");
        let value_json = iroha_primitives::json::Json::new(1u64);
        let value_payload = norito::to_bytes(&value_json).expect("encode value");
        let key_tlv = make_tlv(ivm::PointerType::Name as u16, &key_payload);
        let value_tlv = make_tlv(ivm::PointerType::Json as u16, &value_payload);
        let value_ptr = LITERAL_DATA_START
            + i16::try_from(key_tlv.len()).expect("literal data offset fits in i16");

        let mut code = Vec::new();
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_GET_AUTHORITY)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(
            &ivm::kotodama::compiler::encode_addi(13, 10, 0)
                .expect("encode addi")
                .to_le_bytes(),
        ); // save account ptr
        code.extend_from_slice(
            &ivm::kotodama::compiler::encode_addi(10, 0, LITERAL_DATA_START)
                .expect("encode addi")
                .to_le_bytes(),
        );
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(
            &ivm::kotodama::compiler::encode_addi(11, 10, 0)
                .expect("encode addi")
                .to_le_bytes(),
        ); // r11 = key ptr
        code.extend_from_slice(
            &ivm::kotodama::compiler::encode_addi(10, 0, value_ptr)
                .expect("encode addi")
                .to_le_bytes(),
        );
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(
            &ivm::kotodama::compiler::encode_addi(12, 10, 0)
                .expect("encode addi")
                .to_le_bytes(),
        ); // r12 = value ptr
        code.extend_from_slice(
            &ivm::kotodama::compiler::encode_addi(10, 13, 0)
                .expect("encode addi")
                .to_le_bytes(),
        ); // r10 = account ptr
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_SET_ACCOUNT_DETAIL)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 10_000,
            abi_version: 1,
        };
        let mut prog = meta.encode();
        let mut literal_data = Vec::with_capacity(key_tlv.len() + value_tlv.len());
        literal_data.extend_from_slice(&key_tlv);
        literal_data.extend_from_slice(&value_tlv);
        prog.extend_from_slice(&LITERAL_SECTION_MAGIC);
        prog.extend_from_slice(&0u32.to_le_bytes()); // literal entries
        prog.extend_from_slice(&0u32.to_le_bytes()); // post-pad bytes
        prog.extend_from_slice(&(literal_data.len() as u32).to_le_bytes()); // literal size
        prog.extend_from_slice(&literal_data);
        prog.extend_from_slice(&code);

        let mut md = iroha_data_model::metadata::Metadata::default();
        insert_gas_limit(&mut md);
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_metadata(md)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let (set, source) = derive_for_transaction_with_source(
            &tx,
            Some(&view),
            IvmStrategy::DynamicThenConservative,
        );
        // Expect an account.detail access for the authority under key "cursor".
        let k = key_account_detail(&alice, &"cursor".parse().unwrap());
        assert!(set.read_keys.contains(&k) && set.write_keys.contains(&k));
        assert!(
            set.write_keys.contains("*"),
            "a concrete prepass target cannot prove that a ledger-write target is stable after re-execution"
        );
        assert_eq!(source, Some(AccessSetSource::ConservativeFallback));
    }

    #[test]
    fn ivm_access_dynamic_prepass_requires_gas_limit() {
        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);
        let view = state.view();

        let mut code = Vec::new();
        for rd in [10_u8, 11, 12] {
            code.extend_from_slice(
                &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, rd, 0, 0)
                    .to_le_bytes(),
            );
        }
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_SET_ACCOUNT_DETAIL)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 10_000,
            abi_version: 1,
        };
        let mut prog = meta.encode();
        prog.extend_from_slice(&LITERAL_SECTION_MAGIC);
        prog.extend_from_slice(&0u32.to_le_bytes()); // literal entries
        prog.extend_from_slice(&0u32.to_le_bytes()); // post-pad bytes
        prog.extend_from_slice(&0u32.to_le_bytes()); // literal size
        prog.extend_from_slice(&code);

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());
        let authority = tx.authority().clone();

        let (set, source) = derive_for_transaction_with_source(
            &tx,
            Some(&view),
            IvmStrategy::DynamicThenConservative,
        );
        assert!(set.write_keys.contains("*"));
        assert!(set.read_keys.contains(&format!("account:{authority}")));
        assert!(
            set.write_keys.contains(&format!("tx.sequence:{authority}")),
            "stateful admission sequence key must serialize same-authority transactions"
        );
        assert_eq!(source, Some(AccessSetSource::ConservativeFallback));
    }

    #[test]
    fn access_log_state_keys_are_prefixed() {
        let mut log = ivm::host::AccessLog::default();
        log.read_keys.insert("counter".to_owned());
        log.read_keys.insert("state:already".to_owned());
        log.write_keys.insert("items/1".to_owned());
        let mut set = AccessSet::new();
        merge_access_log(&mut set, &log);
        assert!(set.read_keys.contains("state:counter"));
        assert!(set.read_keys.contains("state:already"));
        assert!(set.write_keys.contains("state:items/1"));
    }

    #[test]
    fn bytecode_access_fence_serializes_state_and_nested_targets_conservatively() {
        use iroha_data_model::transaction::IvmProved;

        fn program_with_syscall(number: u32) -> Vec<u8> {
            let mut program = ivm::ProgramMetadata::default().encode();
            program.extend_from_slice(
                &ivm::encoding::wide::encode_sys(
                    ivm::instruction::wide::system::SCALL,
                    u8::try_from(number).expect("test syscall fits in the encoded immediate"),
                )
                .to_le_bytes(),
            );
            program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
            program
        }

        let mut state_set = AccessSet::new();
        state_set.add_write("state:Map/01".to_owned());
        assert!(apply_unverified_ivm_access_fence(
            &program_with_syscall(ivm::syscalls::SYSCALL_STATE_SET),
            &mut state_set,
        ));
        assert!(state_set.write_keys.contains("state:*"));
        assert!(!state_set.write_keys.contains("*"));

        let mut nested_set = AccessSet::new();
        nested_set.add_write("state:Map/01".to_owned());
        assert!(apply_unverified_ivm_access_fence(
            &program_with_syscall(ivm::syscalls::SYSCALL_CALL_CONTRACT),
            &mut nested_set,
        ));
        assert!(nested_set.write_keys.contains("*"));

        let nested_program = program_with_syscall(ivm::syscalls::SYSCALL_CALL_CONTRACT);
        let (authority, key_pair) = iroha_test_samples::gen_account_in("wonderland");
        let proved = IvmProved {
            bytecode: IvmBytecode::from_compiled(nested_program),
            overlay: Vec::<InstructionBox>::new().into(),
            events_commitment: IrohaHash::new(b"proved-events"),
            gas_policy_commitment: IrohaHash::new(b"proved-gas-policy"),
        };
        let transaction = TransactionBuilder::new("chain".parse().unwrap(), authority)
            .with_executable(Executable::IvmProved(proved))
            .sign(key_pair.private_key());
        let (proved_set, source) = derive_for_transaction_with_source::<crate::state::StateView<'_>>(
            &transaction,
            None,
            IvmStrategy::Conservative,
        );
        assert!(
            proved_set.write_keys.contains("*"),
            "proved overlays must retain the bytecode-derived nested-call fence"
        );
        assert_eq!(source, Some(AccessSetSource::ConservativeFallback));

        let state = State::new(
            World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let empty_overlay = crate::pipeline::overlay::TxOverlay::from_instructions(Vec::new());
        let (prepared_set, prepared_source) = derive_for_prepared_overlay_with_source(
            &transaction,
            &state.view(),
            &empty_overlay,
            None,
            false,
        );
        assert!(prepared_set.write_keys.contains("*"));
        assert_eq!(prepared_source, Some(AccessSetSource::ConservativeFallback));
    }

    #[test]
    fn syscall_access_registry_fails_closed_for_unknown_numbers() {
        use ivm::syscalls::SyscallAccess;

        assert_eq!(
            ivm::syscalls::syscall_access(ivm::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT),
            SyscallAccess::None
        );
        assert_eq!(
            ivm::syscalls::syscall_access(ivm::syscalls::SYSCALL_STATE_GET),
            SyscallAccess::StateRead
        );
        assert_eq!(
            ivm::syscalls::syscall_access(ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED),
            SyscallAccess::LedgerWrite
        );
        assert_eq!(
            ivm::syscalls::syscall_access(0x00ff_fffe),
            SyscallAccess::Dynamic
        );
    }

    #[test]
    fn helper_hidden_privileged_and_dynamic_syscalls_force_global_serialization() {
        use ivm::instruction::wide;

        for (label, syscall) in [
            ("ledger write", ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED),
            ("dynamic nested call", ivm::syscalls::SYSCALL_CALL_CONTRACT),
        ] {
            let code = [
                ivm::encoding::wide::encode_offset24(wide::control::JALS, 2),
                ivm::encoding::wide::encode_halt(),
                ivm::encoding::wide::encode_syscallx(syscall),
                ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
            ];
            let mut program = ivm::ProgramMetadata::default().encode();
            program.extend(code.into_iter().flat_map(u32::to_le_bytes));

            assert!(
                hint_access_set_if_safe(
                    &program,
                    &["state:forged-read".to_owned()],
                    &["state:forged-write".to_owned()],
                )
                .is_none(),
                "{label} hidden behind a helper trusted forged exact CNTR keys"
            );

            let mut dynamic_prepass_claim = AccessSet::new();
            dynamic_prepass_claim.add_read("state:forged-read".to_owned());
            assert!(
                apply_unverified_ivm_access_fence(&program, &mut dynamic_prepass_claim),
                "{label} did not activate the bytecode-derived access fence"
            );
            assert!(
                dynamic_prepass_claim.write_keys.contains("*"),
                "{label} did not force global serialization"
            );
        }
    }

    #[test]
    fn access_set_hints_accept_state_and_canonical_keys() {
        let alice = iroha_test_samples::ALICE_ID.clone();
        let reads = vec![
            "state:alpha".to_owned(),
            format!("account:{alice}"),
            format!("account.detail:{alice}:cursor"),
        ];
        let writes = vec![
            "state:beta".to_owned(),
            "asset_def:62Fk4FPcMuLvW5QjDGNF2a4jAmjM".to_owned(),
        ];
        let set = access_set_from_hint_keys(&reads, &writes, &[], &[])
            .expect("expected valid access set hints");
        assert!(set.read_keys.contains("state:alpha"));
        assert!(set.read_keys.contains(&format!("account:{alice}")));
        assert!(
            set.read_keys
                .contains(&format!("account.detail:{alice}:cursor"))
        );
        assert!(set.write_keys.contains("state:beta"));
        assert!(
            set.write_keys
                .contains("asset_def:62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        );
    }

    #[test]
    fn access_set_hints_accept_zk_state_keys() {
        let asset_def = AssetDefinitionId::parse_address_literal("6pEP9RjNoZ7beWkT3pLfKoM1dyfi")
            .expect("asset definition");
        let reads = vec![format!("zk_asset:{asset_def}")];
        let writes = vec![
            "zk:election:election-1:ciphertexts".to_owned(),
            "zk:election:election-1:nullifiers".to_owned(),
        ];
        let set = access_set_from_hint_keys(&reads, &writes, &[], &[])
            .expect("expected zk access set hints to normalize");
        assert!(set.read_keys.contains(&format!("zk_asset:{asset_def}")));
        assert!(
            set.write_keys
                .contains("zk:election:election-1:ciphertexts")
        );
        assert!(set.write_keys.contains("zk:election:election-1:nullifiers"));
    }

    #[test]
    fn access_set_hints_accept_and_expand_authority_placeholders() {
        let authority = iroha_test_samples::ALICE_ID.clone();
        let mut set = AccessSet {
            read_keys: [
                AUTHORITY_ACCOUNT_KEY.to_owned(),
                "asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM:$authority".to_owned(),
                "account.detail:$authority:cursor".to_owned(),
            ]
            .into(),
            write_keys: [
                "role.binding:$authority:minter".to_owned(),
                "perm.account:$authority:BenefitSpend".to_owned(),
            ]
            .into(),
        };
        expand_authority_placeholders(&mut set, &authority);
        let asset_def =
            AssetDefinitionId::parse_address_literal("62Fk4FPcMuLvW5QjDGNF2a4jAmjM").unwrap();
        let asset = AssetId::of(asset_def, authority.clone());
        assert!(set.read_keys.contains(&format!("account:{authority}")));
        assert!(set.read_keys.contains(&format!("asset:{asset}")));
        assert!(
            set.read_keys
                .contains(&format!("account.detail:{authority}:cursor"))
        );
        assert!(
            set.write_keys
                .contains(&format!("role.binding:{authority}:minter"))
        );
        assert!(
            set.write_keys
                .contains(&format!("perm.account:{authority}:BenefitSpend"))
        );

        let reads = vec![AUTHORITY_ACCOUNT_KEY.to_owned()];
        let writes = vec!["role.binding:$authority:minter".to_owned()];
        assert!(access_set_from_hint_keys(&reads, &writes, &[], &[]).is_some());
    }

    #[test]
    fn access_set_hints_accept_dynamic_state_hints() {
        let dynamic_reads = vec![
            iroha_data_model::smart_contract::manifest::DynamicAccessHint {
                base_key: "state:Orders".to_owned(),
                key_type: "int".to_owned(),
                bound_kind: "range".to_owned(),
                max_keys: 64,
            },
        ];
        let set = access_set_from_hint_keys(&[], &[], &dynamic_reads, &[])
            .expect("expected dynamic read hint to normalize");
        assert!(set.read_keys.contains("state:Orders"));
        assert!(!set.write_keys.contains("state:Orders"));

        let dynamic_writes = vec![
            iroha_data_model::smart_contract::manifest::DynamicAccessHint {
                base_key: "state:Balances".to_owned(),
                key_type: "int".to_owned(),
                bound_kind: "take".to_owned(),
                max_keys: 64,
            },
        ];
        let set = access_set_from_hint_keys(&[], &[], &[], &dynamic_writes)
            .expect("expected dynamic write hint to normalize");
        assert!(set.read_keys.contains("state:Balances"));
        assert!(set.write_keys.contains("state:Balances"));
    }

    #[test]
    fn access_set_hints_accept_coarse_dynamic_account_key() {
        let reads = vec![ACCOUNT_WILDCARD_KEY.to_owned()];
        let set = access_set_from_hint_keys(&reads, &[], &[], &[])
            .expect("expected account wildcard hint to normalize");
        assert!(set.read_keys.contains(ACCOUNT_WILDCARD_KEY));
        assert!(!set.write_keys.contains(ACCOUNT_WILDCARD_KEY));

        let writes = vec![ACCOUNT_WILDCARD_KEY.to_owned()];
        let set = access_set_from_hint_keys(&[], &writes, &[], &[])
            .expect("expected account wildcard write hint to normalize");
        assert!(set.write_keys.contains(ACCOUNT_WILDCARD_KEY));
    }

    #[test]
    fn access_set_hints_accept_coarse_dynamic_asset_keys() {
        let reads = vec![
            ASSET_WILDCARD_KEY.to_owned(),
            ASSET_DEF_WILDCARD_KEY.to_owned(),
        ];
        let writes = reads.clone();
        let set = access_set_from_hint_keys(&reads, &writes, &[], &[])
            .expect("expected asset wildcard hints to normalize");
        assert!(set.read_keys.contains(ASSET_WILDCARD_KEY));
        assert!(set.write_keys.contains(ASSET_WILDCARD_KEY));
        assert!(set.read_keys.contains(ASSET_DEF_WILDCARD_KEY));
        assert!(set.write_keys.contains(ASSET_DEF_WILDCARD_KEY));
    }

    #[test]
    fn access_set_hints_reject_invalid_dynamic_state_hints() {
        let zero_bound = vec![
            iroha_data_model::smart_contract::manifest::DynamicAccessHint {
                base_key: "state:Orders".to_owned(),
                key_type: "int".to_owned(),
                bound_kind: "range".to_owned(),
                max_keys: 0,
            },
        ];
        assert!(access_set_from_hint_keys(&[], &[], &zero_bound, &[]).is_none());

        let wildcard = vec![
            iroha_data_model::smart_contract::manifest::DynamicAccessHint {
                base_key: "state:*".to_owned(),
                key_type: "int".to_owned(),
                bound_kind: "take".to_owned(),
                max_keys: 64,
            },
        ];
        assert!(access_set_from_hint_keys(&[], &[], &wildcard, &[]).is_none());
    }

    #[test]
    fn access_set_hints_reject_unknown_keys() {
        let reads = vec!["perm.account:historical-scoped-literal:can_transfer".to_owned()];
        assert!(access_set_from_hint_keys(&reads, &[], &[], &[]).is_none());
    }

    #[test]
    fn access_set_hints_accept_wildcards() {
        let reads = vec!["*".to_owned()];
        let set = access_set_from_hint_keys(&reads, &[], &[], &[]).expect("global wildcard hint");
        assert!(set.read_keys.contains("*"));

        let writes = vec!["state:*".to_owned()];
        let set = access_set_from_hint_keys(&[], &writes, &[], &[]).expect("state wildcard hint");
        assert!(set.write_keys.contains("state:*"));
    }

    #[test]
    fn ivm_access_uses_manifest_hints_when_present() {
        use iroha_data_model::{
            asset::{AssetDefinitionId, AssetId},
            smart_contract::manifest::{AccessSetHints, MANIFEST_METADATA_KEY},
        };
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        // World/state setup with one account to own the manifest
        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        // Insert manifest with access-set hints into WSV
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_def, alice.clone());
        let hints = AccessSetHints {
            read_keys: vec![format!("account:{alice}")],
            write_keys: vec![format!("asset:{asset_id}")],
            dynamic_reads: Vec::new(),
            dynamic_writes: Vec::new(),
        };
        let code = vec![ivm::encoding::wide::encode_halt().to_le_bytes()]
            .into_iter()
            .flatten()
            .collect();
        let (prog, code_hash, manifest) =
            test_contract_artifact(code, Some(hints.clone()), vec![default_test_entrypoint()]);
        let manifest = manifest.signed(&kp);
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        let mut stx = st_block.transaction();
        stx.world
            .contract_manifests
            .insert(code_hash, manifest.clone());
        stx.apply();
        let _ = st_block.commit();

        // Build a tx carrying this program; add manifest copy into metadata as well (optional)
        let mut md = iroha_data_model::metadata::Metadata::default();
        md.insert(MANIFEST_METADATA_KEY.parse().unwrap(), Json::new(manifest));
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_metadata(md)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let (set, source) = derive_for_transaction_with_source(
            &tx,
            Some(&state.view()),
            IvmStrategy::DynamicThenConservative,
        );
        // Expect keys exactly from hints
        assert!(set.read_keys.contains(&hints.read_keys[0]));
        assert!(set.write_keys.contains(&hints.write_keys[0]));
        assert_eq!(source, Some(AccessSetSource::ManifestHints));
    }

    #[test]
    fn ivm_access_uses_manifest_hints_from_metadata_when_missing_in_wsv() {
        use iroha_data_model::{
            asset::{AssetDefinitionId, AssetId},
            smart_contract::manifest::{AccessSetHints, MANIFEST_METADATA_KEY},
        };
        use iroha_primitives::json::Json;

        access_set_cache_clear();

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_def, alice.clone());
        let hints = AccessSetHints {
            read_keys: vec![format!("account:{alice}")],
            write_keys: vec![format!("asset:{asset_id}")],
            dynamic_reads: Vec::new(),
            dynamic_writes: Vec::new(),
        };
        let code = vec![ivm::encoding::wide::encode_halt().to_le_bytes()]
            .into_iter()
            .flatten()
            .collect();
        let (prog, _code_hash, manifest) =
            test_contract_artifact(code, Some(hints.clone()), vec![default_test_entrypoint()]);
        let manifest = manifest.signed(&kp);

        let mut md = iroha_data_model::metadata::Metadata::default();
        md.insert(MANIFEST_METADATA_KEY.parse().unwrap(), Json::new(manifest));
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_metadata(md)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let (set, source) = derive_for_transaction_with_source(
            &tx,
            Some(&state.view()),
            IvmStrategy::DynamicThenConservative,
        );
        assert!(set.read_keys.contains(&hints.read_keys[0]));
        assert!(set.write_keys.contains(&hints.write_keys[0]));
        assert_eq!(source, Some(AccessSetSource::ManifestHints));
    }

    #[test]
    fn access_set_cache_invalidates_on_manifest_update() {
        use iroha_data_model::smart_contract::manifest::AccessSetHints;
        use nonzero_ext::nonzero;

        access_set_cache_clear();

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = ivm::contract_code_hash(&prog);

        let hints_a = AccessSetHints {
            read_keys: vec!["state:alpha".to_owned()],
            write_keys: Vec::new(),
            dynamic_reads: Vec::new(),
            dynamic_writes: Vec::new(),
        };
        let manifest_a = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(hints_a.clone()),
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        let mut stx = st_block.transaction();
        stx.world.contract_manifests.insert(code_hash, manifest_a);
        stx.apply();
        let _ = st_block.commit();

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let set_a = derive_for_transaction(&tx, Some(&state.view()), IvmStrategy::Conservative);
        assert!(set_a.read_keys.contains("state:alpha"));
        assert!(!set_a.read_keys.contains("state:beta"));

        let hints_b = AccessSetHints {
            read_keys: vec!["state:beta".to_owned()],
            write_keys: Vec::new(),
            dynamic_reads: Vec::new(),
            dynamic_writes: Vec::new(),
        };
        let manifest_b = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(hints_b.clone()),
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        let mut stx = st_block.transaction();
        stx.world.contract_manifests.insert(code_hash, manifest_b);
        stx.apply();
        let _ = st_block.commit();

        let set_b = derive_for_transaction(&tx, Some(&state.view()), IvmStrategy::Conservative);
        assert!(set_b.read_keys.contains("state:beta"));
        assert!(!set_b.read_keys.contains("state:alpha"));
    }

    #[test]
    fn ivm_access_falls_back_when_manifest_hints_invalid() {
        use iroha_data_model::smart_contract::manifest::AccessSetHints;
        use nonzero_ext::nonzero;

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&[0x01, 0x00]); // dummy body
        ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = ivm::contract_code_hash(&prog);

        let hints = AccessSetHints {
            read_keys: vec!["perm.account:historical-scoped-literal:can_transfer".to_owned()],
            write_keys: Vec::new(),
            dynamic_reads: Vec::new(),
            dynamic_writes: Vec::new(),
        };
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(hints),
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        let mut stx = st_block.transaction();
        stx.world.contract_manifests.insert(code_hash, manifest);
        stx.apply();
        let _ = st_block.commit();

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let (set, source) =
            derive_for_transaction_with_source(&tx, Some(&state.view()), IvmStrategy::Conservative);
        assert!(set.write_keys.contains("*"));
        assert_eq!(source, Some(AccessSetSource::ConservativeFallback));
    }

    #[test]
    fn ivm_access_rejects_unproven_exact_state_entrypoint_hints() {
        use iroha_data_model::smart_contract::manifest::{
            AccessSetHints, ContractManifest, EntryPointKind, EntrypointDescriptor,
        };
        use nonzero_ext::nonzero;

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut code = Vec::new();
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_STATE_GET)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&code);
        ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = ivm::contract_code_hash(&prog);

        let entrypoints = vec![
            EntrypointDescriptor {
                name: "main".to_owned(),
                kind: EntryPointKind::Kotoage,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: Some("ExecuteContract".to_owned()),
                read_keys: vec!["state:alpha".to_owned()],
                write_keys: vec!["state:beta".to_owned()],
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
            },
            EntrypointDescriptor {
                name: "run".to_owned(),
                kind: EntryPointKind::Kotoage,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: Some("ExecuteContract".to_owned()),
                read_keys: vec!["state:run-read".to_owned()],
                write_keys: vec!["state:run-write".to_owned()],
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
            },
        ];
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(AccessSetHints {
                read_keys: vec!["state:manifest-read".to_owned()],
                write_keys: vec!["state:manifest-write".to_owned()],
                dynamic_reads: Vec::new(),
                dynamic_writes: Vec::new(),
            }),
            entrypoints: Some(entrypoints),
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        let mut stx = st_block.transaction();
        stx.world
            .contract_manifests
            .insert(code_hash, manifest.clone());
        stx.apply();
        let _ = st_block.commit();

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let (set, source) =
            derive_for_transaction_with_source(&tx, Some(&state.view()), IvmStrategy::Conservative);
        assert!(set.write_keys.contains("*"));
        assert!(!set.read_keys.contains("state:alpha"));
        assert!(!set.write_keys.contains("state:beta"));
        assert!(!set.read_keys.contains("state:manifest-read"));
        assert!(!set.write_keys.contains("state:manifest-write"));
        assert!(!set.read_keys.contains("state:run-read"));
        assert!(!set.write_keys.contains("state:run-write"));
        assert_eq!(source, Some(AccessSetSource::ConservativeFallback));
    }

    #[test]
    fn ivm_access_skips_entrypoint_hints_for_unsafe_syscalls() {
        use iroha_data_model::smart_contract::manifest::{
            ContractManifest, EntryPointKind, EntrypointDescriptor,
        };
        use nonzero_ext::nonzero;

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut code = Vec::new();
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&code);
        ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = ivm::contract_code_hash(&prog);

        let entrypoints = vec![EntrypointDescriptor {
            name: "main".to_owned(),
            kind: EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: Some("ExecuteContract".to_owned()),
            read_keys: vec!["state:alpha".to_owned()],
            write_keys: vec!["state:beta".to_owned()],
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        }];
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: Some(entrypoints),
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        let mut stx = st_block.transaction();
        stx.world
            .contract_manifests
            .insert(code_hash, manifest.clone());
        stx.apply();
        let _ = st_block.commit();

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let (set, source) =
            derive_for_transaction_with_source(&tx, Some(&state.view()), IvmStrategy::Conservative);
        assert!(set.write_keys.contains("*"));
        assert!(!set.read_keys.contains("state:alpha"));
        assert_eq!(source, Some(AccessSetSource::ConservativeFallback));
    }

    #[test]
    fn ivm_access_rejects_unproven_exact_ledger_entrypoint_hints() {
        use iroha_data_model::{
            asset::id::{AssetDefinitionId, AssetId},
            smart_contract::manifest::{ContractManifest, EntryPointKind, EntrypointDescriptor},
        };
        use nonzero_ext::nonzero;

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&alice);
        let account = build_wonderland_account(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut code = Vec::new();
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&code);
        ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = ivm::contract_code_hash(&prog);

        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_def, alice.clone());
        let entrypoints = vec![EntrypointDescriptor {
            name: "main".to_owned(),
            kind: EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: Some("ExecuteContract".to_owned()),
            read_keys: vec![format!("account:{alice}")],
            write_keys: vec![format!("asset:{asset_id}")],
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        }];
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: Some(entrypoints),
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        let mut stx = st_block.transaction();
        stx.world
            .contract_manifests
            .insert(code_hash, manifest.clone());
        stx.apply();
        let _ = st_block.commit();

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let (set, source) =
            derive_for_transaction_with_source(&tx, Some(&state.view()), IvmStrategy::Conservative);
        assert!(set.write_keys.contains("*"));
        assert!(!set.write_keys.contains(&format!("asset:{asset_id}")));
        assert_eq!(source, Some(AccessSetSource::ConservativeFallback));
    }

    #[test]
    fn grant_revoke_role_and_permission_have_static_keys() {
        use iroha_data_model::permission::Permission;
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let role_id: RoleId = "auditor".parse().unwrap();
        let perm = Permission::new(
            "CanMintAsset".to_string(),
            norito::json!({"asset":"coin#wonderland"}),
        );

        // Build ISI batch with grant/revoke combinations
        let isis: Vec<InstructionBox> = vec![
            Grant::account_role(role_id.clone(), alice.clone()).into(),
            Revoke::account_role(role_id.clone(), alice.clone()).into(),
            Grant::account_permission(perm.clone(), alice.clone()).into(),
            Revoke::account_permission(perm.clone(), alice.clone()).into(),
            Grant::role_permission(perm.clone(), role_id.clone()).into(),
            Revoke::role_permission(perm.clone(), role_id.clone()).into(),
        ];
        let exec = Executable::from_iter(isis);
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_executable(exec)
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            None,
            IvmStrategy::Conservative,
        );

        // Expect role registry touched and account-role binding keys written
        assert!(set.read_keys.contains(&format!("role:{}", &role_id)));
        assert!(
            set.write_keys
                .contains(&format!("role.binding:{}:{}", &alice, &role_id))
        );
        // Expect permission keys touched for account and role
        assert!(
            set.write_keys
                .contains(&format!("perm.account:{}:{}", &alice, perm.name()))
        );
        assert!(
            set.write_keys
                .contains(&format!("perm.role:{}:{}", &role_id, perm.name()))
        );
        assert!(
            set.write_keys.contains(AUTHORIZATION_EPOCH_KEY),
            "permission and role mutations must order protected contract calls"
        );
    }

    #[test]
    fn execute_trigger_keys_cover_definition_and_repetitions() {
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let trig: TriggerId = "t0".parse().unwrap();
        let isi: InstructionBox = ExecuteTrigger::new(trig.clone()).into();
        let exec = Executable::from_iter([isi]);
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice)
            .with_executable(exec)
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            None,
            IvmStrategy::Conservative,
        );
        assert!(set.read_keys.contains(&format!("trigger:{}", &trig)));
        assert!(set.write_keys.contains(&format!("trigger:{}", &trig)));
        assert!(
            set.write_keys
                .contains(&format!("trigger.repetitions:{}", &trig))
        );
    }

    #[test]
    fn execute_trigger_includes_access_from_trigger_instructions() {
        use nonzero_ext::nonzero;

        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query);
        let alice = iroha_test_samples::ALICE_ID.clone();

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        {
            let mut stx = st_block.transaction();
            let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&alice, &mut stx)
                .unwrap();
            Register::account(new_wonderland_account(&alice))
                .execute(&alice, &mut stx)
                .unwrap();
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            );
            Register::asset_definition({
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&alice, &mut stx)
            .unwrap();
            let asset_id = AssetId::of(asset_def_id.clone(), alice.clone());
            let trigger_id: TriggerId = "mint_asset_trigger".parse().unwrap();
            let trigger = Trigger::new(
                trigger_id.clone(),
                Action::new(
                    vec![InstructionBox::from(Mint::asset_numeric(
                        1_u32,
                        asset_id.clone(),
                    ))],
                    Repeats::Exactly(1),
                    alice.clone(),
                    iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id.clone())
                        .under_authority(alice.clone()),
                ),
            );
            Register::trigger(trigger)
                .execute(&alice, &mut stx)
                .unwrap();
            stx.apply();
        }
        st_block.commit().unwrap();

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_instructions([InstructionBox::from(ExecuteTrigger::new(
                "mint_asset_trigger".parse().unwrap(),
            ))])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            Some(&state.view()),
            IvmStrategy::Conservative,
        );

        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_def_id.clone(), alice.clone());
        let asset_key = key_asset(&asset_id);
        let asset_def_key = key_asset_def(&asset_def_id);
        assert!(set.write_keys.contains(&asset_key));
        assert!(set.write_keys.contains(&asset_def_key));
    }

    #[test]
    fn execute_trigger_includes_trigger_metadata_keys() {
        use nonzero_ext::nonzero;

        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query);
        let alice = iroha_test_samples::ALICE_ID.clone();

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        {
            let mut stx = st_block.transaction();
            Register::domain(Domain::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
            ))
            .execute(&alice, &mut stx)
            .unwrap();
            Register::account(new_wonderland_account(&alice))
                .execute(&alice, &mut stx)
                .unwrap();
            let trigger_id: TriggerId = "meta_trigger".parse().unwrap();
            let key: Name = "flag".parse().unwrap();
            let trigger = Trigger::new(
                trigger_id.clone(),
                Action::new(
                    vec![InstructionBox::from(SetKeyValue::trigger(
                        trigger_id.clone(),
                        key.clone(),
                        iroha_primitives::json::Json::from(norito::json!("ok")),
                    ))],
                    Repeats::Exactly(1),
                    alice.clone(),
                    iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id.clone())
                        .under_authority(alice.clone()),
                ),
            );
            Register::trigger(trigger)
                .execute(&alice, &mut stx)
                .unwrap();
            stx.apply();
        }
        st_block.commit().unwrap();

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_instructions([InstructionBox::from(ExecuteTrigger::new(
                "meta_trigger".parse().unwrap(),
            ))])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            Some(&state.view()),
            IvmStrategy::Conservative,
        );
        let detail_key = format!("trigger.detail:{}:{}", "meta_trigger", "flag");
        assert!(set.write_keys.contains(&detail_key));
    }

    #[test]
    fn execute_trigger_uses_manifest_hints_for_ivm_triggers() {
        use iroha_data_model::smart_contract::manifest::AccessSetHints;
        use nonzero_ext::nonzero;

        access_set_cache_clear();

        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query);
        let alice = iroha_test_samples::ALICE_ID.clone();

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut st_block = state.block(header);
        let (code_hash, trigger_id, hints) = {
            let mut stx = st_block.transaction();
            Register::domain(Domain::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
            ))
            .execute(&alice, &mut stx)
            .unwrap();
            Register::account(new_wonderland_account(&alice))
                .execute(&alice, &mut stx)
                .unwrap();

            let hints = AccessSetHints {
                read_keys: vec![format!("account:{alice}")],
                write_keys: vec![format!("state:trigger_hint")],
                dynamic_reads: Vec::new(),
                dynamic_writes: Vec::new(),
            };
            let code = vec![ivm::encoding::wide::encode_halt().to_le_bytes()]
                .into_iter()
                .flatten()
                .collect();
            let (prog, code_hash, manifest) =
                test_contract_artifact(code, Some(hints.clone()), vec![default_test_entrypoint()]);
            let manifest = manifest.signed(&iroha_test_samples::ALICE_KEYPAIR);
            stx.world.contract_manifests.insert(code_hash, manifest);

            let trigger_id: TriggerId = "ivm_trigger".parse().unwrap();
            let trigger = Trigger::new(
                trigger_id.clone(),
                Action::new(
                    Executable::Ivm(IvmBytecode::from_compiled(prog)),
                    Repeats::Exactly(1),
                    alice.clone(),
                    iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id.clone())
                        .under_authority(alice.clone()),
                ),
            );
            Register::trigger(trigger)
                .execute(&alice, &mut stx)
                .unwrap();
            stx.apply();

            (code_hash, trigger_id, hints)
        };
        st_block.commit().unwrap();

        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice.clone())
            .with_instructions([InstructionBox::from(ExecuteTrigger::new(
                trigger_id.clone(),
            ))])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            Some(&state.view()),
            IvmStrategy::Conservative,
        );

        assert!(set.read_keys.contains(&hints.read_keys[0]));
        assert!(set.write_keys.contains(&hints.write_keys[0]));
        assert!(
            state
                .view()
                .world()
                .contract_manifests()
                .get(&code_hash)
                .is_some()
        );
    }

    #[test]
    fn register_trigger_keys_cover_definition_and_repetitions() {
        use iroha_primitives::const_vec::ConstVec;

        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let trig: TriggerId = "t_reg".parse().unwrap();
        let trigger = Trigger::new(
            trig.clone(),
            Action::new(
                ConstVec::<InstructionBox>::new_empty(),
                Repeats::Exactly(1),
                alice.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trig.clone())
                    .under_authority(alice.clone()),
            ),
        );
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice)
            .with_instructions([InstructionBox::from(Register::trigger(trigger))])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            None,
            IvmStrategy::Conservative,
        );
        assert!(set.read_keys.contains(&format!("trigger:{trig}")));
        assert!(set.write_keys.contains(&format!("trigger:{trig}")));
        assert!(
            set.write_keys
                .contains(&format!("trigger.repetitions:{trig}"))
        );
    }
}
