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
    nexus::LaneId,
    nft::NftId,
    permission,
    prelude::*,
    role::RoleId,
    smart_contract::manifest::{ContractManifest, EntrypointDescriptor, MANIFEST_METADATA_KEY},
    state::{
        AccountMetadataKey, AccountRoleKey, AssetDefinitionMetadataKey, AssetMetadataKey,
        CanonicalStateKey, DomainMetadataKey, NftMetadataKey, StateAccessSetAdvisory,
        TriggerMetadataKey, TxQueueKey,
    },
    transaction::SignedTransaction,
};
use ivm::host::IVMHost;
use mv::storage::StorageReadOnly; // bring trait into scope for .get()
use parking_lot::RwLock;

use crate::{
    executor::parse_gas_limit,
    smartcontracts::ivm::host::QueryStateSource,
    state::{StateReadOnly, WorldReadOnly},
};

/// Canonical string key used for conflict detection (Norito-like ordering).
///
/// Keys are generated deterministically from data model identifiers such as
/// `AccountId`, `DomainId`, `AssetDefinitionId`, `AssetId`, and `NftId`.
pub type AccessKey = String;

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

fn manifest_access_set(
    manifest: &ContractManifest,
    code_hash: IrohaHash,
    bytecode: &[u8],
    cache_enabled: bool,
) -> Option<(AccessSet, AccessSetSource)> {
    let manifest_hash = cache_enabled.then(|| manifest_signature_hash(manifest));
    if let Some(hints) = manifest.access_set_hints.as_ref() {
        let key = AccessSetCacheKey {
            code_hash,
            entrypoint: None,
        };
        if let Some(hash) = manifest_hash.as_ref() {
            if let Some(set) = access_set_cache_get(&key, hash) {
                return Some((set, AccessSetSource::ManifestHints));
            }
        }
        if let Some(set) = access_set_from_hint_keys(&hints.read_keys, &hints.write_keys) {
            if let Some(hash) = manifest_hash {
                access_set_cache_put(key, hash, set.clone());
            }
            return Some((set, AccessSetSource::ManifestHints));
        }
    }
    if let Some(entrypoints) = manifest.entrypoints.as_deref()
        && let Some(entrypoint) = select_entrypoint(entrypoints)
    {
        let key = AccessSetCacheKey {
            code_hash,
            entrypoint: Some(entrypoint.name.clone()),
        };
        if let Some(hash) = manifest_hash.as_ref() {
            if let Some(set) = access_set_cache_get(&key, hash) {
                return Some((set, AccessSetSource::EntrypointHints));
            }
        }
        if let Some(set) = entrypoint_access_set_if_safe(bytecode, entrypoint) {
            if let Some(hash) = manifest_hash {
                access_set_cache_put(key, hash, set.clone());
            }
            return Some((set, AccessSetSource::EntrypointHints));
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
        Executable::Instructions(batch) => (derive_from_isi_batch(batch.as_ref()), None),
        Executable::Ivm(bytecode) => {
            let bytecode_ref = bytecode.as_ref();
            if let Ok(parsed) = ivm::ProgramMetadata::parse(bytecode_ref) {
                let code_hash = IrohaHash::new(&bytecode_ref[parsed.header_len..]);
                // 1) Try static hints from on-chain manifest (by code_hash)
                if let Some(view) = state_ro {
                    if let Some(manifest) = view.world().contract_manifests().get(&code_hash) {
                        if let Some((set, source)) = manifest_access_set(
                            manifest,
                            code_hash,
                            bytecode_ref,
                            view.pipeline().access_set_cache_enabled,
                        ) {
                            return (set, Some(source));
                        }
                    }
                }
                // 1b) Fallback to manifest provided in transaction metadata.
                if let Some(manifest) = manifest_from_metadata(tx) {
                    if manifest.code_hash == Some(code_hash) {
                        if let Some((set, source)) =
                            manifest_access_set(&manifest, code_hash, bytecode_ref, false)
                        {
                            return (set, Some(source));
                        }
                    }
                }
            }
            // 2) Otherwise, use dynamic prepass if enabled with view, else conservative
            match (ivm_strategy, state_ro) {
                (IvmStrategy::DynamicThenConservative, Some(view)) => {
                    let set = tx_gas_limit(tx)
                        .and_then(|gas_limit| {
                            derive_from_ivm_dynamic(bytecode_ref, tx.authority(), view, gas_limit)
                        })
                        .unwrap_or_else(|_| AccessSet::global());
                    let source = if set.read_keys.is_empty()
                        && set.write_keys.len() == 1
                        && set.write_keys.contains("*")
                    {
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
            }
        }
    }
}

fn entrypoint_access_set_if_safe(
    bytecode: &[u8],
    entrypoint: &EntrypointDescriptor,
) -> Option<AccessSet> {
    if entrypoint.read_keys.is_empty() && entrypoint.write_keys.is_empty() {
        return None;
    }
    let report = match ivm::analysis::analyze_program(bytecode) {
        Ok(report) => report,
        Err(_) => return None,
    };
    if !report
        .syscalls
        .iter()
        .all(|entry| is_entrypoint_hint_safe_syscall(entry.number))
    {
        return None;
    }
    let has_non_state_syscall = report
        .syscalls
        .iter()
        .any(|entry| !is_state_only_syscall(entry.number));
    if has_non_state_syscall && entrypoint_keys_state_only(entrypoint) {
        return None;
    }
    access_set_from_hint_keys(&entrypoint.read_keys, &entrypoint.write_keys)
}

fn select_entrypoint(entrypoints: &[EntrypointDescriptor]) -> Option<&EntrypointDescriptor> {
    if entrypoints.is_empty() {
        return None;
    }
    if let Some(entrypoint) = entrypoints.iter().find(|entry| entry.name == "main") {
        return Some(entrypoint);
    }
    if let Some(entrypoint) = entrypoints.iter().find(|entry| entry.name == "hajimari") {
        return Some(entrypoint);
    }
    if entrypoints.len() == 1 {
        return entrypoints.first();
    }
    None
}

/// Normalize manifest/entrypoint hint keys into canonical WSV keys plus state keys,
/// preserving literal WSV keys when account selectors cannot be resolved.
#[allow(clippy::too_many_lines)]
fn access_set_from_hint_keys(read_keys: &[String], write_keys: &[String]) -> Option<AccessSet> {
    if read_keys.iter().any(|key| key == "*") || write_keys.iter().any(|key| key == "*") {
        return Some(AccessSet::global());
    }
    let mut advisory = StateAccessSetAdvisory::default();
    let mut state_reads: BTreeSet<String> = BTreeSet::new();
    let mut state_writes: BTreeSet<String> = BTreeSet::new();
    let account_parse_unresolved = |err: &iroha_data_model::error::ParseError| -> bool {
        matches!(
            err.reason(),
            "ERR_DOMAIN_SELECTOR_UNRESOLVED" | "ERR_UAID_UNRESOLVED" | "ERR_OPAQUE_ID_UNRESOLVED"
        )
    };
    let asset_has_unresolved_account = |raw_asset: &str| -> bool {
        let Some((_, account_part)) = raw_asset.rsplit_once('#') else {
            return false;
        };
        match account_part.parse::<AccountId>() {
            Ok(_) => false,
            Err(err) => account_parse_unresolved(&err),
        }
    };

    let ingest = |raw: &str,
                  canonical: &mut Vec<CanonicalStateKey>,
                  state_keys: &mut BTreeSet<String>|
     -> Option<()> {
        if let Some(rest) = raw.strip_prefix("state:") {
            if rest.is_empty() {
                return None;
            }
            state_keys.insert(raw.to_owned());
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("account.detail:") {
            let mut parsed = None;
            for split in [rest.split_once(':'), rest.rsplit_once(':')] {
                let Some((id_raw, key_raw)) = split else {
                    continue;
                };
                let Ok(key) = key_raw.parse::<Name>() else {
                    continue;
                };
                match id_raw.parse::<AccountId>() {
                    Ok(id) => {
                        parsed = Some(Ok(AccountMetadataKey { id, key }));
                        break;
                    }
                    Err(err) if account_parse_unresolved(&err) => {
                        parsed = Some(Err(()));
                        break;
                    }
                    Err(_) => continue,
                }
            }
            match parsed {
                Some(Ok(key)) => {
                    canonical.push(CanonicalStateKey::AccountMetadata(key));
                }
                Some(Err(())) => {
                    state_keys.insert(raw.to_owned());
                }
                None => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("domain.detail:") {
            let (id, key) = rest.split_once(':')?;
            let id: DomainId = id.parse().ok()?;
            let key: Name = key.parse().ok()?;
            canonical.push(CanonicalStateKey::DomainMetadata(DomainMetadataKey {
                id,
                key,
            }));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("asset_def.detail:") {
            let (id, key) = rest.split_once(':')?;
            let id: AssetDefinitionId = id.parse().ok()?;
            let key: Name = key.parse().ok()?;
            canonical.push(CanonicalStateKey::AssetDefinitionMetadata(
                AssetDefinitionMetadataKey { id, key },
            ));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("asset.detail:") {
            let mut parsed = None;
            for split in [rest.split_once(':'), rest.rsplit_once(':')] {
                let Some((id_raw, key_raw)) = split else {
                    continue;
                };
                let Ok(key) = key_raw.parse::<Name>() else {
                    continue;
                };
                match id_raw.parse::<AssetId>() {
                    Ok(id) => {
                        parsed = Some(Ok(AssetMetadataKey { id, key }));
                        break;
                    }
                    Err(_) if asset_has_unresolved_account(id_raw) => {
                        parsed = Some(Err(()));
                        break;
                    }
                    Err(_) => continue,
                }
            }
            match parsed {
                Some(Ok(key)) => {
                    canonical.push(CanonicalStateKey::AssetMetadata(key));
                }
                Some(Err(())) => {
                    state_keys.insert(raw.to_owned());
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
            let mut parsed = None;
            for split in [rest.split_once(':'), rest.rsplit_once(':')] {
                let Some((account_raw, role_raw)) = split else {
                    continue;
                };
                let Ok(role) = role_raw.parse::<RoleId>() else {
                    continue;
                };
                match account_raw.parse::<AccountId>() {
                    Ok(account) => {
                        parsed = Some(Ok(AccountRoleKey { account, role }));
                        break;
                    }
                    Err(err) if account_parse_unresolved(&err) => {
                        parsed = Some(Err(()));
                        break;
                    }
                    Err(_) => continue,
                }
            }
            match parsed {
                Some(Ok(key)) => {
                    canonical.push(CanonicalStateKey::AccountRole(key));
                }
                Some(Err(())) => {
                    state_keys.insert(raw.to_owned());
                }
                None => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("account:") {
            match rest.parse::<AccountId>() {
                Ok(id) => canonical.push(CanonicalStateKey::Account(id)),
                Err(err) if account_parse_unresolved(&err) => {
                    state_keys.insert(raw.to_owned());
                }
                Err(_) => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("domain:") {
            let id: DomainId = rest.parse().ok()?;
            canonical.push(CanonicalStateKey::Domain(id));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("asset_def:") {
            let id: AssetDefinitionId = rest.parse().ok()?;
            canonical.push(CanonicalStateKey::AssetDefinition(id));
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("asset:") {
            match rest.parse::<AssetId>() {
                Ok(id) => canonical.push(CanonicalStateKey::Asset(id)),
                Err(_) if asset_has_unresolved_account(rest) => {
                    state_keys.insert(raw.to_owned());
                }
                Err(_) => return None,
            }
            return Some(());
        }
        if let Some(rest) = raw.strip_prefix("nft:") {
            let id: NftId = rest.parse().ok()?;
            canonical.push(CanonicalStateKey::Nft(id));
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

    advisory.canonicalize();

    let render = |key: &CanonicalStateKey| -> AccessKey {
        match key {
            CanonicalStateKey::Domain(id) => format!("domain:{id}"),
            CanonicalStateKey::Account(id) => format!("account:{id}"),
            CanonicalStateKey::Asset(id) => format!("asset:{id}"),
            CanonicalStateKey::AssetDefinition(id) => format!("asset_def:{id}"),
            CanonicalStateKey::Nft(id) => format!("nft:{id}"),
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

fn entrypoint_keys_state_only(entrypoint: &EntrypointDescriptor) -> bool {
    let is_state_key = |key: &str| key.starts_with("state:");
    entrypoint.read_keys.iter().all(|key| is_state_key(key))
        && entrypoint.write_keys.iter().all(|key| is_state_key(key))
}

fn is_entrypoint_hint_safe_syscall(number: u8) -> bool {
    matches!(
        u32::from(number),
        ivm::syscalls::SYSCALL_REGISTER_DOMAIN
            | ivm::syscalls::SYSCALL_UNREGISTER_DOMAIN
            | ivm::syscalls::SYSCALL_TRANSFER_DOMAIN
            | ivm::syscalls::SYSCALL_REGISTER_ACCOUNT
            | ivm::syscalls::SYSCALL_UNREGISTER_ACCOUNT
            | ivm::syscalls::SYSCALL_REGISTER_PEER
            | ivm::syscalls::SYSCALL_UNREGISTER_PEER
            | ivm::syscalls::SYSCALL_REGISTER_ASSET
            | ivm::syscalls::SYSCALL_UNREGISTER_ASSET
            | ivm::syscalls::SYSCALL_MINT_ASSET
            | ivm::syscalls::SYSCALL_BURN_ASSET
            | ivm::syscalls::SYSCALL_TRANSFER_ASSET
            | ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN
            | ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_END
            | ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY
            | ivm::syscalls::SYSCALL_SET_ACCOUNT_DETAIL
            | ivm::syscalls::SYSCALL_NFT_MINT_ASSET
            | ivm::syscalls::SYSCALL_NFT_TRANSFER_ASSET
            | ivm::syscalls::SYSCALL_NFT_SET_METADATA
            | ivm::syscalls::SYSCALL_NFT_BURN_ASSET
            | ivm::syscalls::SYSCALL_CREATE_ROLE
            | ivm::syscalls::SYSCALL_DELETE_ROLE
            | ivm::syscalls::SYSCALL_GRANT_ROLE
            | ivm::syscalls::SYSCALL_REVOKE_ROLE
            | ivm::syscalls::SYSCALL_GRANT_PERMISSION
            | ivm::syscalls::SYSCALL_REVOKE_PERMISSION
            | ivm::syscalls::SYSCALL_CREATE_TRIGGER
            | ivm::syscalls::SYSCALL_REMOVE_TRIGGER
            | ivm::syscalls::SYSCALL_SET_TRIGGER_ENABLED
            | ivm::syscalls::SYSCALL_AXT_BEGIN
            | ivm::syscalls::SYSCALL_AXT_TOUCH
            | ivm::syscalls::SYSCALL_AXT_COMMIT
            | ivm::syscalls::SYSCALL_VERIFY_DS_PROOF
            | ivm::syscalls::SYSCALL_USE_ASSET_HANDLE
            | ivm::syscalls::SYSCALL_DEBUG_PRINT
            | ivm::syscalls::SYSCALL_EXIT
            | ivm::syscalls::SYSCALL_ABORT
            | ivm::syscalls::SYSCALL_DEBUG_LOG
            | ivm::syscalls::SYSCALL_ALLOC
            | ivm::syscalls::SYSCALL_GROW_HEAP
            | ivm::syscalls::SYSCALL_GET_PUBLIC_INPUT
            | ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT
            | ivm::syscalls::SYSCALL_VERIFY_SIGNATURE
            | ivm::syscalls::SYSCALL_COMMIT_OUTPUT
            | ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV
            | ivm::syscalls::SYSCALL_POINTER_TO_NORITO
            | ivm::syscalls::SYSCALL_POINTER_FROM_NORITO
            | ivm::syscalls::SYSCALL_TLV_EQ
            | ivm::syscalls::SYSCALL_NAME_DECODE
            | ivm::syscalls::SYSCALL_JSON_ENCODE
            | ivm::syscalls::SYSCALL_JSON_DECODE
            | ivm::syscalls::SYSCALL_SCHEMA_ENCODE
            | ivm::syscalls::SYSCALL_SCHEMA_DECODE
            | ivm::syscalls::SYSCALL_SCHEMA_INFO
            | ivm::syscalls::SYSCALL_DECODE_INT
            | ivm::syscalls::SYSCALL_ENCODE_INT
            | ivm::syscalls::SYSCALL_BUILD_PATH_MAP_KEY
            | ivm::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO
            | ivm::syscalls::SYSCALL_STATE_GET
            | ivm::syscalls::SYSCALL_STATE_SET
            | ivm::syscalls::SYSCALL_STATE_DEL
            | ivm::syscalls::SYSCALL_GET_AUTHORITY
            | ivm::syscalls::SYSCALL_SM3_HASH
            | ivm::syscalls::SYSCALL_SM2_VERIFY
            | ivm::syscalls::SYSCALL_SM4_GCM_SEAL
            | ivm::syscalls::SYSCALL_SM4_GCM_OPEN
            | ivm::syscalls::SYSCALL_SM4_CCM_SEAL
            | ivm::syscalls::SYSCALL_SM4_CCM_OPEN
            | ivm::syscalls::SYSCALL_VRF_VERIFY
            | ivm::syscalls::SYSCALL_VRF_VERIFY_BATCH
            | ivm::syscalls::SYSCALL_ZK_VERIFY_TRANSFER
            | ivm::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD
            | ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT
            | ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY
            | ivm::syscalls::SYSCALL_ZK_VERIFY_BATCH
            | ivm::syscalls::SYSCALL_PROVE_EXECUTION
            | ivm::syscalls::SYSCALL_VERIFY_PROOF
            | ivm::syscalls::SYSCALL_GET_MERKLE_PATH
            | ivm::syscalls::SYSCALL_GET_MERKLE_COMPACT
            | ivm::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT
    )
}

fn is_state_only_syscall(number: u8) -> bool {
    matches!(
        u32::from(number),
        ivm::syscalls::SYSCALL_DEBUG_PRINT
            | ivm::syscalls::SYSCALL_EXIT
            | ivm::syscalls::SYSCALL_ABORT
            | ivm::syscalls::SYSCALL_DEBUG_LOG
            | ivm::syscalls::SYSCALL_ALLOC
            | ivm::syscalls::SYSCALL_GROW_HEAP
            | ivm::syscalls::SYSCALL_GET_PUBLIC_INPUT
            | ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT
            | ivm::syscalls::SYSCALL_VERIFY_SIGNATURE
            | ivm::syscalls::SYSCALL_COMMIT_OUTPUT
            | ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV
            | ivm::syscalls::SYSCALL_POINTER_TO_NORITO
            | ivm::syscalls::SYSCALL_POINTER_FROM_NORITO
            | ivm::syscalls::SYSCALL_TLV_EQ
            | ivm::syscalls::SYSCALL_NAME_DECODE
            | ivm::syscalls::SYSCALL_JSON_ENCODE
            | ivm::syscalls::SYSCALL_JSON_DECODE
            | ivm::syscalls::SYSCALL_SCHEMA_ENCODE
            | ivm::syscalls::SYSCALL_SCHEMA_DECODE
            | ivm::syscalls::SYSCALL_SCHEMA_INFO
            | ivm::syscalls::SYSCALL_DECODE_INT
            | ivm::syscalls::SYSCALL_ENCODE_INT
            | ivm::syscalls::SYSCALL_BUILD_PATH_MAP_KEY
            | ivm::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO
            | ivm::syscalls::SYSCALL_STATE_GET
            | ivm::syscalls::SYSCALL_STATE_SET
            | ivm::syscalls::SYSCALL_STATE_DEL
            | ivm::syscalls::SYSCALL_GET_AUTHORITY
            | ivm::syscalls::SYSCALL_SM3_HASH
            | ivm::syscalls::SYSCALL_SM2_VERIFY
            | ivm::syscalls::SYSCALL_SM4_GCM_SEAL
            | ivm::syscalls::SYSCALL_SM4_GCM_OPEN
            | ivm::syscalls::SYSCALL_SM4_CCM_SEAL
            | ivm::syscalls::SYSCALL_SM4_CCM_OPEN
            | ivm::syscalls::SYSCALL_VRF_VERIFY
            | ivm::syscalls::SYSCALL_VRF_VERIFY_BATCH
            | ivm::syscalls::SYSCALL_ZK_VERIFY_TRANSFER
            | ivm::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD
            | ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT
            | ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY
            | ivm::syscalls::SYSCALL_ZK_VERIFY_BATCH
            | ivm::syscalls::SYSCALL_PROVE_EXECUTION
            | ivm::syscalls::SYSCALL_VERIFY_PROOF
            | ivm::syscalls::SYSCALL_GET_MERKLE_PATH
            | ivm::syscalls::SYSCALL_GET_MERKLE_COMPACT
            | ivm::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT
    )
}

fn derive_from_isi_batch(batch: &[InstructionBox]) -> AccessSet {
    let mut set = AccessSet::new();
    for instr in batch {
        set.union_with(derive_from_instruction(instr));
    }
    set
}

#[allow(clippy::too_many_lines)]
fn derive_from_instruction(instr: &InstructionBox) -> AccessSet {
    let mut set = AccessSet::new();
    let any = instr.as_any();

    // Logging is side-effect-free; keep it conflict-free.
    if any.downcast_ref::<Log>().is_some() {
        return set;
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
            RegisterBox::Account(r) => {
                add_domain_r(&mut set, r.object.id().domain());
                add_account_rw(&mut set, r.object.id());
            }
            RegisterBox::AssetDefinition(r) => {
                add_domain_r(&mut set, r.object.id().domain());
                add_asset_def_rw(&mut set, &r.object.id().clone());
            }
            RegisterBox::Nft(r) => add_nft_rw(&mut set, r.object.id()),
            RegisterBox::Peer(_) => set = AccessSet::global(),
            RegisterBox::Trigger(r) => add_trigger_rw(&mut set, r.object.id()),
            RegisterBox::Role(r) => add_role_rw(&mut set, r.object.id()),
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
    if let Some(ub) = any.downcast_ref::<UnregisterBox>() {
        match ub {
            UnregisterBox::Domain(u) => add_domain_rw(&mut set, &u.object),
            UnregisterBox::Account(u) => add_account_rw(&mut set, &u.object),
            UnregisterBox::AssetDefinition(u) => add_asset_def_rw(&mut set, &u.object),
            UnregisterBox::Nft(u) => add_nft_rw(&mut set, &u.object),
            UnregisterBox::Peer(_) => set = AccessSet::global(),
            UnregisterBox::Trigger(u) => add_trigger_rw(&mut set, &u.object),
            UnregisterBox::Role(u) => add_role_rw(&mut set, &u.object),
        }
        return set;
    }

    // Grant
    if let Some(gb) = any.downcast_ref::<GrantBox>() {
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
        set.add_read(key_trigger(&exe.trigger));
        set.add_write(key_trigger_repetitions(&exe.trigger));
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
fn key_nft(id: &NftId) -> AccessKey {
    format!("nft:{id}")
}
fn key_nft_detail(id: &NftId, key: &Name) -> AccessKey {
    format!("nft.detail:{id}:{key}")
}

fn add_account_r(set: &mut AccessSet, id: &AccountId) {
    set.add_read(key_account(id));
}
fn add_domain_r(set: &mut AccessSet, id: &DomainId) {
    set.add_read(key_domain(id));
}
fn add_account_rw(set: &mut AccessSet, id: &AccountId) {
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
    let k = key_asset_def(id);
    set.add_read(k.clone());
    set.add_write(k);
}
fn add_asset_def_r(set: &mut AccessSet, id: &AssetDefinitionId) {
    set.add_read(key_asset_def(id));
}
fn add_asset_def_detail_rw(set: &mut AccessSet, id: &AssetDefinitionId, key: &Name) {
    set.add_read(key_asset_def(id));
    let d = key_asset_def_detail(id, key);
    set.add_read(d.clone());
    set.add_write(d);
}
fn add_asset_rw(set: &mut AccessSet, id: &AssetId) {
    let k = key_asset(id);
    set.add_read(k.clone());
    set.add_write(k);
    // Asset operations rely on the owning account/domain and definition state.
    add_account_r(set, id.account());
    add_domain_r(set, id.account().domain());
    add_asset_def_r(set, id.definition());
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
    state_ro: &R,
    gas_limit: u64,
) -> Result<AccessSet, String>
where
    R: StateReadOnly + QueryStateSource,
{
    // Execute VM with CoreHost to collect queued ISIs; do not apply.
    ivm::ProgramMetadata::parse(bytecode).map_err(|e| format!("ivm.metadata: {e}"))?;
    let mut vm = ivm::IVM::new(gas_limit);
    // Supply accounts snapshot for vendor helpers to become deterministic.
    let accounts = state_ro.accounts_snapshot();
    let mut host = crate::smartcontracts::ivm::host::CoreHostImpl::with_accounts(
        authority.clone(),
        Arc::clone(&accounts),
    )
    .with_access_logging();
    #[cfg(feature = "telemetry")]
    host.set_telemetry(state_ro.metrics().clone());
    host.set_crypto_config(state_ro.crypto());
    host.set_halo2_config(&state_ro.zk().halo2);
    host.set_durable_state_snapshot_from_world(state_ro.world());
    host.set_public_inputs_from_parameters(state_ro.world().parameters());
    host.set_query_state(state_ro);
    host.set_chain_id(state_ro.chain_id());
    host.set_zk_snapshots_from_world(state_ro.world(), state_ro.zk())
        .map_err(|e| format!("ivm.zk_snapshots: {e}"))?;
    host.begin_tx(&ivm::parallel::StateAccessSet::default())
        .map_err(|e| format!("ivm.begin_tx: {e}"))?;
    vm.load_program(bytecode)
        .map_err(|e| format!("ivm.load_program: {e}"))?;
    vm.set_gas_limit(gas_limit);
    vm.run_with_host(&mut host)
        .map_err(|e| format!("ivm.run: {e}"))?;
    let mut set = AccessSet::new();
    let mut access_log: Option<ivm::host::AccessLog> = None;
    for isi in host.drain_instructions() {
        set.union_with(derive_from_instruction(&isi));
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
    use crate::state::{State, World};

    const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";
    const TEST_GAS_LIMIT: u64 = 50_000_000;

    fn insert_gas_limit(metadata: &mut iroha_data_model::metadata::Metadata) {
        metadata.insert(
            Name::from_str("gas_limit").expect("static gas_limit key"),
            iroha_primitives::json::Json::new(TEST_GAS_LIMIT),
        );
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
        let ad: AssetDefinitionId = "coin#wonderland".parse().unwrap();
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
        let k_domain = key_domain(alice.domain());
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
    fn log_instruction_has_no_access_keys() {
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let tx = TransactionBuilder::new("chain".parse().unwrap(), alice)
            .with_instructions([Log::new(Level::INFO, "hello".to_owned())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());

        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            None,
            IvmStrategy::Conservative,
        );

        assert!(set.read_keys.is_empty());
        assert!(set.write_keys.is_empty());
    }

    #[test]
    fn register_access_includes_domain_reads() {
        let (alice, _) = iroha_test_samples::gen_account_in("wonderland");
        let domain_id = alice.domain().clone();
        let account = Account::new(alice.clone());
        let asset_def_id: AssetDefinitionId = "coin#wonderland".parse().unwrap();
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
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
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
        assert_eq!(source, Some(AccessSetSource::PrepassMerge));
    }

    #[test]
    fn ivm_access_dynamic_prepass_requires_gas_limit() {
        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
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

        let (set, source) = derive_for_transaction_with_source(
            &tx,
            Some(&view),
            IvmStrategy::DynamicThenConservative,
        );
        assert!(set.write_keys.contains("*"));
        assert!(set.read_keys.is_empty());
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
    fn access_set_hints_accept_state_and_canonical_keys() {
        let alice = iroha_test_samples::ALICE_ID.clone();
        let reads = vec![
            "state:alpha".to_owned(),
            format!("account:{alice}"),
            format!("account.detail:{alice}:cursor"),
        ];
        let writes = vec![
            "state:beta".to_owned(),
            "asset_def:rose#wonderland".to_owned(),
        ];
        let set =
            access_set_from_hint_keys(&reads, &writes).expect("expected valid access set hints");
        assert!(set.read_keys.contains("state:alpha"));
        assert!(set.read_keys.contains(&format!("account:{alice}")));
        assert!(
            set.read_keys
                .contains(&format!("account.detail:{alice}:cursor"))
        );
        assert!(set.write_keys.contains("state:beta"));
        assert!(set.write_keys.contains("asset_def:rose#wonderland"));
    }

    #[test]
    fn access_set_hints_reject_unknown_keys() {
        let reads = vec!["perm.account:alice@wonderland:can_transfer".to_owned()];
        assert!(access_set_from_hint_keys(&reads, &[]).is_none());
    }

    #[test]
    fn access_set_hints_accept_global_wildcard() {
        let reads = vec!["*".to_owned()];
        let set = access_set_from_hint_keys(&reads, &[]).expect("expected global access set");
        assert!(set.read_keys.is_empty());
        assert!(set.write_keys.contains("*"));
    }

    #[test]
    fn ivm_access_uses_manifest_hints_when_present() {
        use iroha_data_model::{
            asset::{AssetDefinitionId, AssetId},
            smart_contract::manifest::{AccessSetHints, ContractManifest, MANIFEST_METADATA_KEY},
        };
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        // World/state setup with one account to own the manifest
        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        // Minimal program body to compute a code hash
        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&[0x01, 0x00]); // dummy body
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

        // Insert manifest with access-set hints into WSV
        let asset_def: AssetDefinitionId = "rose#wonderland".parse().expect("asset definition");
        let asset_id = AssetId::of(asset_def, alice.clone());
        let hints = AccessSetHints {
            read_keys: vec![format!("account:{alice}")],
            write_keys: vec![format!("asset:{asset_id}")],
        };
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(hints.clone()),
            entrypoints: None,
            kotoba: None,
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
            smart_contract::manifest::{AccessSetHints, ContractManifest, MANIFEST_METADATA_KEY},
        };
        use iroha_primitives::json::Json;

        access_set_cache_clear();

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(b"metadata-hints");
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

        let asset_def: AssetDefinitionId = "rose#wonderland".parse().expect("asset definition");
        let asset_id = AssetId::of(asset_def, alice.clone());
        let hints = AccessSetHints {
            read_keys: vec![format!("account:{alice}")],
            write_keys: vec![format!("asset:{asset_id}")],
        };
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(hints.clone()),
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);

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
        use iroha_data_model::smart_contract::manifest::{AccessSetHints, ContractManifest};
        use nonzero_ext::nonzero;

        access_set_cache_clear();

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(b"cache-hints");
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

        let hints_a = AccessSetHints {
            read_keys: vec!["state:alpha".to_owned()],
            write_keys: Vec::new(),
        };
        let manifest_a = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(hints_a.clone()),
            entrypoints: None,
            kotoba: None,
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
        };
        let manifest_b = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(hints_b.clone()),
            entrypoints: None,
            kotoba: None,
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
        use iroha_data_model::smart_contract::manifest::{AccessSetHints, ContractManifest};
        use nonzero_ext::nonzero;

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&[0x01, 0x00]); // dummy body
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

        let hints = AccessSetHints {
            read_keys: vec!["perm.account:alice@wonderland:can_transfer".to_owned()],
            write_keys: Vec::new(),
        };
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: Some(hints),
            entrypoints: None,
            kotoba: None,
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
    fn ivm_access_uses_manifest_entrypoint_hints_when_present() {
        use iroha_data_model::smart_contract::manifest::{
            ContractManifest, EntryPointKind, EntrypointDescriptor,
        };
        use nonzero_ext::nonzero;

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
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
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

        let entrypoints = vec![
            EntrypointDescriptor {
                name: "main".to_owned(),
                kind: EntryPointKind::Public,
                permission: None,
                read_keys: vec!["state:alpha".to_owned()],
                write_keys: vec!["state:beta".to_owned()],
                access_hints_complete: None,
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
            },
            EntrypointDescriptor {
                name: "run".to_owned(),
                kind: EntryPointKind::Public,
                permission: None,
                read_keys: vec!["state:run-read".to_owned()],
                write_keys: vec!["state:run-write".to_owned()],
                access_hints_complete: None,
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
            },
        ];
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: Some(entrypoints),
            kotoba: None,
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
        assert!(set.read_keys.contains("state:alpha"));
        assert!(set.write_keys.contains("state:beta"));
        assert!(!set.read_keys.contains("state:run-read"));
        assert!(!set.write_keys.contains("state:run-write"));
        assert_eq!(source, Some(AccessSetSource::EntrypointHints));
    }

    #[test]
    fn ivm_access_skips_entrypoint_hints_for_unsafe_syscalls() {
        use iroha_data_model::smart_contract::manifest::{
            ContractManifest, EntryPointKind, EntrypointDescriptor,
        };
        use nonzero_ext::nonzero;

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut code = Vec::new();
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_TRANSFER_ASSET)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&code);
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

        let entrypoints = vec![EntrypointDescriptor {
            name: "main".to_owned(),
            kind: EntryPointKind::Public,
            permission: None,
            read_keys: vec!["state:alpha".to_owned()],
            write_keys: vec!["state:beta".to_owned()],
            access_hints_complete: None,
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        }];
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: Some(entrypoints),
            kotoba: None,
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

        let set = derive_for_transaction(&tx, Some(&state.view()), IvmStrategy::Conservative);
        assert!(set.write_keys.contains("*"));
        assert!(!set.read_keys.contains("state:alpha"));
    }

    #[test]
    fn ivm_access_uses_entrypoint_hints_for_isi_syscalls_with_wsv_keys() {
        use iroha_data_model::{
            asset::id::{AssetDefinitionId, AssetId},
            smart_contract::manifest::{ContractManifest, EntryPointKind, EntrypointDescriptor},
        };
        use nonzero_ext::nonzero;

        let (alice, kp) = iroha_test_samples::gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let mut code = Vec::new();
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_TRANSFER_ASSET)
                    .expect("syscall identifier fits in 8 bits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut prog = ivm::ProgramMetadata::default().encode();
        prog.extend_from_slice(&code);
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

        let asset_def: AssetDefinitionId = "rose#wonderland".parse().unwrap();
        let asset_id = AssetId::of(asset_def, alice.clone());
        let entrypoints = vec![EntrypointDescriptor {
            name: "main".to_owned(),
            kind: EntryPointKind::Public,
            permission: None,
            read_keys: vec![format!("account:{alice}")],
            write_keys: vec![format!("asset:{asset_id}")],
            access_hints_complete: None,
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        }];
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: Some(entrypoints),
            kotoba: None,
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
        assert!(set.read_keys.contains(&format!("account:{alice}")));
        assert!(set.write_keys.contains(&format!("asset:{asset_id}")));
        assert_eq!(source, Some(AccessSetSource::EntrypointHints));
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
        assert!(
            set.write_keys
                .contains(&format!("trigger.repetitions:{}", &trig))
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
