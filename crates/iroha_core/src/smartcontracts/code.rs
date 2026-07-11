//! On-chain smart contract registry helpers backed by the world state.
//!
//! This module exposes thin helpers that wrap the canonical ISI instructions
//! for registering manifests, storing bytecode, and binding contract
//! instances. Read APIs query the authenticated world-state view so callers
//! never rely on process-local caches. This replaces the historical
//! process-global map and ensures every node observes the same registry
//! contents.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::BlockHeader,
    isi::smart_contract_code::{
        ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
    },
    name::Name,
    prelude::ValidationFail,
    smart_contract::manifest::{ContractManifest, EntryPointKind},
    smart_contract::{ContractAddress, ContractAlias},
};
use mv::storage::StorageReadOnly;
use thiserror::Error;

use crate::{
    smartcontracts::Execute,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

/// Legacy reserved durable-state namespace used by the first subject-signing mitigation.
///
/// This namespace remains guest-inaccessible so old snapshots cannot regain a write surface, but
/// it is never authoritative: pre-upgrade guests could have populated arbitrary keys here. The
/// typed [`ContractSubjectBinding`] ledger is the only source used for runtime identity and direct
/// signature rejection.
pub(crate) const CONTRACT_SUBJECT_HISTORY_PREFIX: &str = "contract_subject_history_";

/// Derive the consensus-persisted history key for a contract subject.
#[cfg(test)]
pub(crate) fn contract_subject_history_key(subject: &AccountId) -> Name {
    let digest = Hash::new(subject.to_string().as_bytes());
    format!(
        "{CONTRACT_SUBJECT_HISTORY_PREFIX}{}",
        hex::encode(digest.as_ref())
    )
    .parse()
    .expect("contract subject history key must be a valid Name")
}

/// Return whether a durable-state key belongs to the contract-subject history namespace.
pub(crate) fn is_contract_subject_history_key(key: &Name) -> bool {
    key.as_ref().starts_with(CONTRACT_SUBJECT_HISTORY_PREFIX)
}

/// Reconstruct the legacy v1 subject whose signing key was publicly derivable from the address.
pub(crate) fn legacy_contract_subject_id_v1(address: &ContractAddress) -> AccountId {
    let mut seed = Vec::with_capacity(b"iroha:contract-subject:v1:".len() + address.as_ref().len());
    seed.extend_from_slice(b"iroha:contract-subject:v1:");
    seed.extend_from_slice(address.as_ref().as_bytes());
    let keypair = KeyPair::try_from_seed(seed, Algorithm::Ed25519)
        .expect("legacy contract subject seed derives Ed25519 keypair");
    AccountId::new(keypair.public_key().clone())
}

/// Legacy subject derivation used by deployments created before the hash-to-point v2 upgrade.
pub(crate) const CONTRACT_SUBJECT_VERSION_V1: u8 = 1;
/// Hash-to-point subject derivation used for addresses first activated after the v2 upgrade.
pub(crate) const CONTRACT_SUBJECT_VERSION_V2: u8 = 2;

/// Environment variable pointing at the exhaustive, offline-reviewed legacy activation manifest.
pub const CONTRACT_SUBJECT_V2_MIGRATION_MANIFEST_ENV: &str =
    "IROHA_CONTRACT_SUBJECT_V2_MIGRATION_MANIFEST";

/// Strict input produced by the offline contract-subject history audit tool.
#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
pub struct LegacyContractSubjectMigrationManifest {
    /// Manifest schema. Only version 1 is accepted.
    pub schema_version: u8,
    /// Exact chain whose finalized history was audited.
    pub chain_id: ChainId,
    /// Finalized height covered by the exhaustive event export.
    pub audited_through_height: u64,
    /// Canonical block hash at `audited_through_height`, or `None` for genesis height zero.
    pub audited_tip_hash: Option<iroha_crypto::HashOf<BlockHeader>>,
    /// Must assert that the source contained the complete finalized smart-contract event stream.
    pub complete_finalized_contract_event_export: bool,
    /// Hash of the exact reviewed event-export bytes.
    pub source_export_hash: Hash,
    /// Number of activation events in the source (including repeat activations).
    pub activation_event_count: u64,
    /// Sorted, unique set of every contract address ever activated through the audited height.
    pub historical_contract_addresses: Vec<ContractAddress>,
}

/// Load and authenticate the operator-reviewed legacy activation manifest against chain state.
pub(crate) fn load_legacy_contract_subject_migration_manifest(
    chain_id: &ChainId,
    block_hashes: &[iroha_crypto::HashOf<BlockHeader>],
) -> Result<(LegacyContractSubjectMigrationManifest, Hash), String> {
    let path = std::env::var_os(CONTRACT_SUBJECT_V2_MIGRATION_MANIFEST_ENV).ok_or_else(|| {
        format!(
            "legacy snapshot has no typed contract-subject ledger; set `{CONTRACT_SUBJECT_V2_MIGRATION_MANIFEST_ENV}` to the exhaustive offline audit manifest"
        )
    })?;
    let path = std::path::PathBuf::from(path);
    let bytes = std::fs::read(&path)
        .map_err(|err| format!("failed to read `{}`: {err}", path.display()))?;
    let text = core::str::from_utf8(&bytes).map_err(|err| {
        format!(
            "migration manifest `{}` is not UTF-8: {err}",
            path.display()
        )
    })?;
    let manifest: LegacyContractSubjectMigrationManifest = norito::json::from_json(text)
        .map_err(|err| format!("invalid migration manifest `{}`: {err}", path.display()))?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported contract-subject migration manifest schema {}",
            manifest.schema_version
        ));
    }
    if &manifest.chain_id != chain_id {
        return Err(format!(
            "contract-subject migration manifest chain id `{}` does not match `{chain_id}`",
            manifest.chain_id
        ));
    }
    let expected_height =
        u64::try_from(block_hashes.len()).map_err(|_| "snapshot height exceeds u64".to_owned())?;
    if manifest.audited_through_height != expected_height {
        return Err(format!(
            "contract-subject migration audit height {} does not match snapshot height {expected_height}",
            manifest.audited_through_height
        ));
    }
    let expected_tip = block_hashes.last().copied();
    if manifest.audited_tip_hash != expected_tip {
        return Err("contract-subject migration audit tip hash does not match snapshot tip".into());
    }
    if !manifest.complete_finalized_contract_event_export {
        return Err(
            "contract-subject migration manifest does not attest exhaustive finalized events"
                .into(),
        );
    }
    if manifest.activation_event_count
        < u64::try_from(manifest.historical_contract_addresses.len()).unwrap_or(u64::MAX)
    {
        return Err(
            "activation_event_count cannot be smaller than the unique historical address count"
                .into(),
        );
    }
    if manifest
        .historical_contract_addresses
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err("historical_contract_addresses must be strictly sorted and unique".into());
    }
    Ok((manifest, Hash::new(bytes)))
}

/// Consensus-persisted, irreversible subject identity for one contract address.
///
/// Bindings are retained after deactivation. This both preserves the runtime authority of
/// pre-upgrade instances and makes every historical contract subject permanently non-signing.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractSubjectBinding {
    /// Exact account authority used while this contract executes.
    pub(crate) subject: AccountId,
    /// Consensus derivation version (`1` for seeded-key legacy, `2` for hash-to-point).
    pub(crate) version: u8,
    /// Hash of the exhaustive offline history manifest that recovered this legacy address.
    /// Active legacy bindings reconstructed directly from authenticated WSV may omit it.
    pub(crate) legacy_audit_manifest_hash: Option<Hash>,
}

impl ContractSubjectBinding {
    /// Construct a binding for a pre-upgrade address without changing its authority graph.
    #[must_use]
    pub(crate) fn legacy_v1(
        address: &ContractAddress,
        legacy_audit_manifest_hash: Option<Hash>,
    ) -> Self {
        Self {
            subject: legacy_contract_subject_id_v1(address),
            version: CONTRACT_SUBJECT_VERSION_V1,
            legacy_audit_manifest_hash,
        }
    }

    /// Construct a binding for an address first activated under v2 rules.
    #[must_use]
    pub(crate) fn current_v2(address: &ContractAddress) -> Self {
        Self {
            subject: address.subject_id(),
            version: CONTRACT_SUBJECT_VERSION_V2,
            legacy_audit_manifest_hash: None,
        }
    }

    /// Validate that the persisted subject matches the declared consensus derivation version.
    pub(crate) fn validate_for(&self, address: &ContractAddress) -> Result<(), String> {
        let expected = match self.version {
            CONTRACT_SUBJECT_VERSION_V1 => legacy_contract_subject_id_v1(address),
            CONTRACT_SUBJECT_VERSION_V2 => address.subject_id(),
            other => return Err(format!("unsupported contract subject version {other}")),
        };
        if self.subject != expected {
            return Err(format!(
                "contract subject binding mismatch for `{address}`: version {} derives `{expected}`, stored `{}`",
                self.version, self.subject
            ));
        }
        if self.version == CONTRACT_SUBJECT_VERSION_V2 && self.legacy_audit_manifest_hash.is_some()
        {
            return Err(format!(
                "v2 contract subject binding for `{address}` must not carry a legacy audit hash"
            ));
        }
        Ok(())
    }
}

/// Remove all values from the legacy guest-writable marker namespace.
///
/// No security decision consumes these values. Clearing them during the trusted schema migration
/// prevents a pre-upgrade contract from leaving forged victim markers that resemble audit data.
pub(crate) fn clear_legacy_contract_subject_markers(world: &mut crate::state::World) {
    let retained: BTreeMap<_, _> = world
        .smart_contract_state
        .view()
        .iter()
        .filter(|(key, _)| !is_contract_subject_history_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    world.smart_contract_state = retained.into_iter().collect();
}

/// Initialize bindings for a newly constructed v2 world.
///
/// This path must not be used to load a legacy snapshot. Snapshot decoding separately detects a
/// missing typed field and requires the exhaustive offline history manifest.
pub(crate) fn initialize_current_contract_subject_bindings(
    world: &mut crate::state::World,
) -> Result<(), String> {
    clear_legacy_contract_subject_markers(world);
    let addresses: Vec<_> = world
        .contract_instances
        .view()
        .iter()
        .map(|(address, _)| address.clone())
        .collect();
    for address in addresses {
        if let Some(binding) = world.contract_subject_bindings.view().get(&address) {
            binding.validate_for(&address)?;
        } else {
            world.contract_subject_bindings.insert(
                address.clone(),
                ContractSubjectBinding::current_v2(&address),
            );
        }
    }
    rebuild_contract_subject_addresses(world)?;
    validate_contract_subject_bindings(world)
}

/// Apply the one-time legacy migration using an exhaustive finalized-history audit.
///
/// Every active instance and every address in `historical_addresses` is pinned to its original v1
/// authority. Nothing in the ownership graph is rewritten: balances, metadata, permissions,
/// roles, durable state, trigger references, and nested-call authority therefore remain exact.
/// Reapplying the same inputs is idempotent; a conflicting preexisting typed binding fails closed.
pub(crate) fn migrate_legacy_contract_subject_bindings(
    world: &mut crate::state::World,
    historical_addresses: impl IntoIterator<Item = ContractAddress>,
    manifest_hash: Hash,
) -> Result<(), String> {
    clear_legacy_contract_subject_markers(world);
    let mut addresses: std::collections::BTreeSet<_> = historical_addresses.into_iter().collect();
    addresses.extend(
        world
            .contract_instances
            .view()
            .iter()
            .map(|(address, _)| address.clone()),
    );
    for address in addresses {
        let expected = ContractSubjectBinding::legacy_v1(&address, Some(manifest_hash));
        match world.contract_subject_bindings.view().get(&address) {
            Some(existing) if existing != &expected => {
                return Err(format!(
                    "conflicting contract subject binding for legacy address `{address}`"
                ));
            }
            Some(_) => {}
            None => {
                world.contract_subject_bindings.insert(address, expected);
            }
        }
    }
    rebuild_contract_subject_addresses(world)?;
    validate_contract_subject_bindings(world)
}

/// Rebuild the reverse subject index exclusively from authenticated versioned bindings.
///
/// The index is deliberately omitted from snapshots/state roots. Rebuilding rejects duplicate
/// subjects instead of allowing one historical contract to shadow another at admission time.
pub(crate) fn rebuild_contract_subject_addresses(
    world: &mut crate::state::World,
) -> Result<(), String> {
    let mut by_subject = BTreeMap::new();
    for (address, binding) in world.contract_subject_bindings.view().iter() {
        binding.validate_for(address)?;
        if let Some(existing) = by_subject.insert(binding.subject.clone(), address.clone()) {
            return Err(format!(
                "contract subject `{}` is bound to both `{existing}` and `{address}`",
                binding.subject
            ));
        }
    }
    world.contract_subject_addresses = by_subject.into_iter().collect();
    Ok(())
}

/// Validate the complete typed subject ledger and require every active instance to have a binding.
pub(crate) fn validate_contract_subject_bindings(
    world: &crate::state::World,
) -> Result<(), String> {
    let bindings = world.contract_subject_bindings.view();
    for (address, binding) in bindings.iter() {
        binding.validate_for(address)?;
    }
    for (address, _) in world.contract_instances.view().iter() {
        if bindings.get(address).is_none() {
            return Err(format!(
                "active contract instance `{address}` has no versioned subject binding"
            ));
        }
    }
    if world.contract_subject_addresses.view().len() != bindings.len() {
        return Err("contract subject reverse index cardinality mismatch".into());
    }
    for (address, binding) in bindings.iter() {
        if world
            .contract_subject_addresses
            .view()
            .get(&binding.subject)
            != Some(address)
        {
            return Err(format!(
                "contract subject reverse index mismatch for `{address}` and `{}`",
                binding.subject
            ));
        }
    }
    Ok(())
}

/// Return whether an account is an irreversible historical contract subject.
pub(crate) fn is_historical_contract_subject(
    world: &impl WorldReadOnly,
    subject: &AccountId,
) -> bool {
    world.contract_subject_addresses().get(subject).is_some()
}

/// Smart contract registry errors.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Underlying instruction execution failed.
    #[error("instruction failed: {0}")]
    Instruction(#[from] crate::smartcontracts::Error),
    /// Contract manifest must declare `code_hash`.
    #[error("manifest.code_hash missing")]
    MissingCodeHash,
    /// Bytecode image is not a valid self-describing IVM contract artifact.
    #[error("invalid contract bytecode: {0}")]
    InvalidCode(String),
}

/// Reserved physical durable-state namespace for consensus-managed contract lifecycle markers.
///
/// Raw IVM state syscalls reject this namespace and deployed contracts are always scoped below
/// `sc/<contract-address-digest>/`, so only the runtime can create or consume these records.
pub(crate) const CONTRACT_LIFECYCLE_STATE_PREFIX: &str = "lc";

const CONTRACT_LIFECYCLE_RECORD_MAGIC: [u8; 4] = *b"KLC1";

/// Consensus-bound lifecycle transition awaiting its branded hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::codec::Decode, norito::codec::Encode)]
pub(crate) enum PendingContractLifecycle {
    /// A newly activated instance must execute its `hajimari`/`始まり` hook once.
    Hajimari {
        /// Unique deterministic identity of this activation transition.
        transition_id: Hash,
        /// Code hash activated for the new instance.
        code_hash: Hash,
    },
    /// An existing instance was rebound from one code hash to another and must execute
    /// its `kaizen`/`改善` hook once.
    Kaizen {
        /// Unique deterministic identity of this replacement transition.
        transition_id: Hash,
        /// Code hash that was active immediately before `kaizen`/`改善`.
        previous_code_hash: Hash,
        /// Newly activated code hash whose hook must execute.
        code_hash: Hash,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::codec::Decode, norito::codec::Encode)]
struct ContractLifecycleRecordV1 {
    domain: [u8; 4],
    pending: PendingContractLifecycle,
}

impl PendingContractLifecycle {
    /// Code hash whose entrypoint is allowed to consume this transition.
    #[must_use]
    pub(crate) const fn code_hash(self) -> Hash {
        match self {
            Self::Hajimari { code_hash, .. } | Self::Kaizen { code_hash, .. } => code_hash,
        }
    }

    /// Branded entrypoint kind required to consume this transition.
    #[must_use]
    pub(crate) const fn entrypoint_kind(self) -> EntryPointKind {
        match self {
            Self::Hajimari { .. } => EntryPointKind::Hajimari,
            Self::Kaizen { .. } => EntryPointKind::Kaizen,
        }
    }

    fn encode(self) -> Vec<u8> {
        norito::codec::Encode::encode(&ContractLifecycleRecordV1 {
            domain: CONTRACT_LIFECYCLE_RECORD_MAGIC,
            pending: self,
        })
    }

    fn decode(encoded: &[u8]) -> Result<Self, &'static str> {
        let record: ContractLifecycleRecordV1 = norito::decode_from_bytes(encoded)
            .map_err(|_| "lifecycle record is not canonical Norito")?;
        if norito::codec::Encode::encode(&record).as_slice() != encoded {
            return Err("lifecycle record is not canonical Norito");
        }
        if record.domain != CONTRACT_LIFECYCLE_RECORD_MAGIC {
            return Err("lifecycle record has an invalid domain tag");
        }
        Ok(record.pending)
    }
}

/// Build a pending lifecycle transition with an ABA-resistant deterministic identity.
///
/// The identity binds the current transaction/trigger execution, its transition ordinal, the
/// exact address and lifecycle kind, and both old and new code hashes. Consequently a later
/// deactivate/reactivate or A→B→A sequence cannot recreate the bytes observed by a stale
/// prepared lifecycle call.
pub(crate) fn new_pending_contract_lifecycle(
    state_transaction: &mut StateTransaction<'_, '_>,
    contract_address: &ContractAddress,
    previous_code_hash: Option<Hash>,
    code_hash: Hash,
    kind: EntryPointKind,
) -> Result<PendingContractLifecycle, &'static str> {
    let (execution_identity, ordinal) =
        state_transaction.next_contract_lifecycle_transition_seed()?;
    let mut preimage = Vec::from(&b"iroha:contract-lifecycle-transition:v1\0"[..]);
    preimage.extend_from_slice(execution_identity.as_ref());
    preimage.extend_from_slice(&ordinal.to_le_bytes());
    preimage.extend_from_slice(
        &u64::try_from(contract_address.as_str().len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    preimage.extend_from_slice(contract_address.as_str().as_bytes());
    preimage.push(match kind {
        EntryPointKind::Hajimari => 0,
        EntryPointKind::Kaizen => 1,
        EntryPointKind::Kotoage | EntryPointKind::View => {
            return Err("ordinary entrypoint kind cannot stage a lifecycle transition");
        }
    });
    if let Some(previous_code_hash) = previous_code_hash {
        preimage.push(1);
        preimage.extend_from_slice(previous_code_hash.as_ref());
    } else {
        preimage.push(0);
    }
    preimage.extend_from_slice(code_hash.as_ref());
    let transition_id = Hash::new(preimage);

    match (kind, previous_code_hash) {
        (EntryPointKind::Hajimari, None) => Ok(PendingContractLifecycle::Hajimari {
            transition_id,
            code_hash,
        }),
        (EntryPointKind::Kaizen, Some(previous_code_hash)) => {
            Ok(PendingContractLifecycle::Kaizen {
                transition_id,
                previous_code_hash,
                code_hash,
            })
        }
        (EntryPointKind::Hajimari, Some(_)) => {
            Err("hajimari/始まり cannot stage a previous code hash")
        }
        (EntryPointKind::Kaizen, None) => Err("kaizen/改善 requires a previous code hash"),
        (EntryPointKind::Kotoage | EntryPointKind::View, _) => {
            Err("ordinary entrypoint kind cannot stage a lifecycle transition")
        }
    }
}

/// Return the reserved physical durable-state key for an instance lifecycle marker.
#[must_use]
pub(crate) fn contract_lifecycle_state_key(contract_address: &ContractAddress) -> Name {
    let digest = hex::encode(Hash::new(contract_address.as_str().as_bytes()).as_ref());
    format!("{CONTRACT_LIFECYCLE_STATE_PREFIX}/{digest}")
        .parse()
        .expect("contract lifecycle state key is a valid Name")
}

/// Read and validate the pending lifecycle transition for `contract_address`.
///
/// Corrupt records fail closed because they are consensus state, not user input.
pub(crate) fn pending_contract_lifecycle(
    world: &impl WorldReadOnly,
    contract_address: &ContractAddress,
) -> Result<Option<PendingContractLifecycle>, ValidationFail> {
    let key = contract_lifecycle_state_key(contract_address);
    world
        .smart_contract_state()
        .get(&key)
        .map(|encoded| {
            PendingContractLifecycle::decode(encoded).map_err(|reason| {
                ValidationFail::InternalError(format!(
                    "invalid lifecycle state for contract `{contract_address}`: {reason}"
                ))
            })
        })
        .transpose()
}

/// Stage or clear the runtime-owned pending lifecycle marker.
pub(crate) fn set_pending_contract_lifecycle(
    state_transaction: &mut StateTransaction<'_, '_>,
    contract_address: &ContractAddress,
    pending: Option<PendingContractLifecycle>,
) {
    let key = contract_lifecycle_state_key(contract_address);
    if let Some(pending) = pending {
        state_transaction
            .world
            .smart_contract_state
            .insert(key, pending.encode());
    } else {
        state_transaction.world.smart_contract_state.remove(key);
    }
}

/// Validate lifecycle availability for a top-level call against the live binding.
///
/// A pending transition blocks all other entrypoints, including views, until its exact branded
/// hook succeeds. The returned transition is attached to the runtime context so successful VM
/// execution can consume the marker atomically with the rest of its overlay.
pub(crate) fn validate_contract_lifecycle_call(
    world: &impl WorldReadOnly,
    contract_address: &ContractAddress,
    executing_code_hash: Hash,
    kind: EntryPointKind,
) -> Result<Option<PendingContractLifecycle>, ValidationFail> {
    let bound_code_hash = world
        .contract_instances()
        .get(contract_address)
        .copied()
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!(
                "contract instance `{contract_address}` is not active"
            ))
        })?;
    if bound_code_hash != executing_code_hash {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{contract_address}` changed from code `{executing_code_hash}` to `{bound_code_hash}`"
        )));
    }

    let pending = pending_contract_lifecycle(world, contract_address)?;
    match (kind, pending) {
        (EntryPointKind::Hajimari, Some(pending @ PendingContractLifecycle::Hajimari { .. }))
            if pending.code_hash() == executing_code_hash =>
        {
            Ok(Some(pending))
        }
        (EntryPointKind::Kaizen, Some(pending @ PendingContractLifecycle::Kaizen { .. }))
            if pending.code_hash() == executing_code_hash =>
        {
            Ok(Some(pending))
        }
        (EntryPointKind::Hajimari, Some(other)) => Err(ValidationFail::NotPermitted(format!(
            "contract `{contract_address}` is awaiting {:?}, not hajimari/始まり",
            other.entrypoint_kind()
        ))),
        (EntryPointKind::Kaizen, Some(other)) => Err(ValidationFail::NotPermitted(format!(
            "contract `{contract_address}` is awaiting {:?}, not kaizen/改善",
            other.entrypoint_kind()
        ))),
        (EntryPointKind::Hajimari, None) => Err(ValidationFail::NotPermitted(format!(
            "contract `{contract_address}` has no pending hajimari/始まり transition"
        ))),
        (EntryPointKind::Kaizen, None) => Err(ValidationFail::NotPermitted(format!(
            "contract `{contract_address}` has no pending kaizen/改善 transition"
        ))),
        (EntryPointKind::Kotoage | EntryPointKind::View, Some(pending)) => {
            Err(ValidationFail::NotPermitted(format!(
                "contract `{contract_address}` must complete {:?} before other entrypoints are callable",
                pending.entrypoint_kind()
            )))
        }
        (EntryPointKind::Kotoage | EntryPointKind::View, None) => Ok(None),
    }
}

/// Recheck a prepared lifecycle completion against current state immediately before apply.
pub(crate) fn validate_contract_lifecycle_completion(
    world: &impl WorldReadOnly,
    contract_address: &ContractAddress,
    expected: PendingContractLifecycle,
) -> Result<(), ValidationFail> {
    let current = pending_contract_lifecycle(world, contract_address)?;
    if current != Some(expected) {
        return Err(ValidationFail::NotPermitted(format!(
            "pending hajimari/始まり or kaizen/改善 transition for `{contract_address}` changed before apply"
        )));
    }
    if world.contract_instances().get(contract_address).copied() != Some(expected.code_hash()) {
        return Err(ValidationFail::NotPermitted(format!(
            "contract binding for `{contract_address}` changed before lifecycle apply"
        )));
    }
    Ok(())
}

/// Reject a view while its instance is awaiting `hajimari`/`始まり` or `kaizen`/`改善`.
///
/// # Errors
/// Returns an error when the binding changed, the lifecycle marker is corrupt, or a transition
/// is still pending.
pub fn ensure_contract_ready_for_view(
    world: &impl WorldReadOnly,
    contract_address: &ContractAddress,
    executing_code_hash: Hash,
) -> Result<(), ValidationFail> {
    validate_contract_lifecycle_call(
        world,
        contract_address,
        executing_code_hash,
        EntryPointKind::View,
    )
    .map(|_| ())
}

/// Validate lifecycle availability for a non-mutating simulation of a top-level entrypoint.
///
/// # Errors
/// Returns an error when the entrypoint does not match the exact pending activation transition.
pub fn ensure_contract_entrypoint_lifecycle(
    world: &impl WorldReadOnly,
    contract_address: &ContractAddress,
    executing_code_hash: Hash,
    kind: EntryPointKind,
) -> Result<(), ValidationFail> {
    validate_contract_lifecycle_call(world, contract_address, executing_code_hash, kind).map(|_| ())
}

/// Record combining a contract manifest with optional bytecode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractCodeRecord {
    /// Manifest stored under the `code_hash` key.
    pub manifest: ContractManifest,
    /// Optional compiled bytecode bytes (entire `.to` image).
    pub code_bytes: Option<Vec<u8>>,
}

/// Register a smart contract manifest on-chain via the canonical ISI.
///
/// The authority must hold `CanRegisterSmartContractCode`. Networks can add
/// `CanEnactGovernance` for specific namespaces via `gov_protected_namespaces`.
/// The manifest must include `code_hash`, and the corresponding bytecode must
/// already be stored as a verified self-describing artifact.
///
/// # Errors
///
/// Returns [`RegistryError`] when the manifest is missing a `code_hash` or the
/// underlying `RegisterSmartContractCode` instruction fails during execution.
pub fn register_manifest(
    authority: &AccountId,
    manifest: ContractManifest,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), RegistryError> {
    if manifest.code_hash.is_none() {
        return Err(RegistryError::MissingCodeHash);
    }
    RegisterSmartContractCode { manifest }.execute(authority, state_transaction)?;
    Ok(())
}

/// Register compiled contract bytecode on-chain and return its `code_hash`.
///
/// The helper verifies the self-describing `CNTR` artifact, uses its canonical
/// artifact hash, and submits the [`RegisterSmartContractBytes`] instruction.
/// The authority must hold `CanRegisterSmartContractCode`.
///
/// # Errors
///
/// Returns [`RegistryError`] when the bytecode header is invalid or when the
/// underlying instruction execution fails.
pub fn register_code_bytes(
    authority: &AccountId,
    code: Vec<u8>,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<Hash, RegistryError> {
    let verified = ivm::verify_contract_artifact(&code)
        .map_err(|err| RegistryError::InvalidCode(err.to_string()))?;
    let code_hash = verified.code_hash;
    RegisterSmartContractBytes { code_hash, code }.execute(authority, state_transaction)?;
    Ok(code_hash)
}

/// Bind `contract_address` to a `code_hash` to activate or perform `kaizen`/`改善` on an
/// instance.
///
/// The authority must hold `CanRegisterSmartContractCode`, including for an
/// idempotent request. Rebinding an active address to different verified code
/// additionally requires `CanEnactGovernance`; it is a genuine in-place
/// `kaizen`/`改善` and stages its declared `kaizen`/`改善` hook.
///
/// # Errors
///
/// Returns [`RegistryError`] when the activation instruction fails during
/// execution.
pub fn activate_instance(
    authority: &AccountId,
    contract_address: ContractAddress,
    code_hash: Hash,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), RegistryError> {
    ActivateContractInstance {
        contract_address,
        code_hash,
    }
    .execute(authority, state_transaction)?;
    Ok(())
}

/// Fetch the manifest stored for `code_hash`, if any.
pub fn fetch_manifest(state: &impl StateReadOnly, code_hash: &Hash) -> Option<ContractManifest> {
    state.world().contract_manifests().get(code_hash).cloned()
}

/// Fetch the stored bytecode for `code_hash`, if any.
pub fn fetch_code_bytes(state: &impl StateReadOnly, code_hash: &Hash) -> Option<Vec<u8>> {
    state.world().contract_code().get(code_hash).cloned()
}

/// Retrieve a combined record (manifest + optional bytecode) for `code_hash`.
pub fn fetch_record(state: &impl StateReadOnly, code_hash: &Hash) -> Option<ContractCodeRecord> {
    let manifest = fetch_manifest(state, code_hash)?;
    let code_bytes = fetch_code_bytes(state, code_hash);
    Some(ContractCodeRecord {
        manifest,
        code_bytes,
    })
}

/// Batched contract lookup combining manifest, bytecode, and optional binding lookup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContractArtifacts {
    /// Stored manifest for `code_hash`, if any.
    pub manifest: Option<ContractManifest>,
    /// Stored bytecode for `code_hash`, if any.
    pub code_bytes: Option<Vec<u8>>,
    /// Code hash bound to `contract_address`, if a binding exists.
    pub bound_code_hash: Option<Hash>,
}

/// Fully resolved on-chain contract instance record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundContractRecord {
    /// Canonical instance address used to resolve the binding.
    pub contract_address: ContractAddress,
    /// Consensus-persisted runtime authority for this exact address.
    pub contract_subject: AccountId,
    /// Optional stable alias currently bound to the instance.
    pub contract_alias: Option<ContractAlias>,
    /// Code hash currently bound to the instance.
    pub code_hash: Hash,
    /// Stored manifest for the bound code hash.
    pub manifest: ContractManifest,
    /// Stored bytecode for the bound code hash.
    pub code_bytes: Vec<u8>,
}

/// Lightweight bound-instance identity that never copies contract bytecode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundContractIdentity {
    /// Canonical instance address used to resolve the binding.
    pub contract_address: ContractAddress,
    /// Optional stable alias currently bound to the instance.
    pub contract_alias: Option<ContractAlias>,
    /// Code hash currently bound to the instance.
    pub code_hash: Hash,
}

/// Fetch manifest, code bytes, and instance binding in a single pass.
#[must_use]
pub fn fetch_artifacts(
    state: &impl StateReadOnly,
    code_hash: &Hash,
    binding: Option<&ContractAddress>,
) -> ContractArtifacts {
    let manifest = fetch_manifest(state, code_hash);
    let code_bytes = fetch_code_bytes(state, code_hash);
    let bound_code_hash = binding.and_then(|contract_address| {
        state
            .world()
            .contract_instances()
            .get(contract_address)
            .copied()
    });

    ContractArtifacts {
        manifest,
        code_bytes,
        bound_code_hash,
    }
}

/// Return the code hash bound to `contract_address`, if any.
pub fn fetch_instance_binding(
    state: &impl StateReadOnly,
    contract_address: &ContractAddress,
) -> Option<Hash> {
    state
        .world()
        .contract_instances()
        .get(contract_address)
        .copied()
}

/// Resolve the consensus-persisted runtime authority for an active contract instance.
#[must_use]
pub fn fetch_bound_contract_subject(
    state: &impl StateReadOnly,
    contract_address: &ContractAddress,
) -> Option<AccountId> {
    bound_contract_subject_from_world(state.world(), contract_address)
}

/// Resolve a validated active contract authority directly from a world-state view.
#[must_use]
pub(crate) fn bound_contract_subject_from_world(
    world: &impl WorldReadOnly,
    contract_address: &ContractAddress,
) -> Option<AccountId> {
    world.contract_instances().get(contract_address)?;
    let binding = world.contract_subject_bindings().get(contract_address)?;
    binding.validate_for(contract_address).ok()?;
    Some(binding.subject.clone())
}

/// Resolve a bound instance without cloning its manifest or bytecode.
#[must_use]
pub fn fetch_bound_contract_identity(
    state: &impl StateReadOnly,
    contract_address: &ContractAddress,
) -> Option<BoundContractIdentity> {
    let code_hash = fetch_instance_binding(state, contract_address)?;
    fetch_bound_contract_subject(state, contract_address)?;
    let contract_alias = state
        .world()
        .contract_alias_bindings()
        .get(contract_address)
        .map(|binding| binding.alias.clone());
    if let Some(alias) = contract_alias.as_ref()
        && state.world().contract_aliases().get(alias) != Some(contract_address)
    {
        return None;
    }
    Some(BoundContractIdentity {
        contract_address: contract_address.clone(),
        contract_alias,
        code_hash,
    })
}

/// Borrow stored bytecode only for the duration of `use_bytes`.
///
/// This lets content-addressed cache misses prepare directly from world state
/// without first cloning the complete deployable image.
pub fn with_code_bytes<T>(
    state: &impl StateReadOnly,
    code_hash: &Hash,
    use_bytes: impl FnOnce(&[u8]) -> T,
) -> Option<T> {
    state
        .world()
        .contract_code()
        .get(code_hash)
        .map(|bytes| use_bytes(bytes.as_ref()))
}

/// Resolve the fully bound contract instance record for `contract_address`.
#[must_use]
pub fn fetch_bound_contract_record(
    state: &impl StateReadOnly,
    contract_address: &ContractAddress,
) -> Option<BoundContractRecord> {
    let code_hash = fetch_instance_binding(state, contract_address)?;
    let subject_binding = state
        .world()
        .contract_subject_bindings()
        .get(contract_address)?;
    subject_binding.validate_for(contract_address).ok()?;
    let manifest = fetch_manifest(state, &code_hash)?;
    let code_bytes = fetch_code_bytes(state, &code_hash)?;
    let contract_alias = state
        .world()
        .contract_alias_bindings()
        .get(contract_address)
        .map(|binding| binding.alias.clone());

    Some(BoundContractRecord {
        contract_address: contract_address.clone(),
        contract_subject: subject_binding.subject.clone(),
        contract_alias,
        code_hash,
        manifest,
        code_bytes,
    })
}

/// Resolve the fully bound contract instance record for a deterministic contract subject.
#[must_use]
pub fn fetch_bound_contract_record_by_subject(
    state: &impl StateReadOnly,
    contract_subject: &AccountId,
) -> Option<BoundContractRecord> {
    let contract_address = state
        .world()
        .contract_subject_addresses()
        .get(contract_subject)?;
    state.world().contract_instances().get(contract_address)?;
    fetch_bound_contract_record(state, contract_address)
}

/// Snapshot all deployed contract instance records keyed by deterministic contract subject.
#[must_use]
pub fn snapshot_bound_contract_records_by_subject(
    state: &impl StateReadOnly,
) -> BTreeMap<AccountId, BoundContractRecord> {
    state
        .world()
        .contract_instances()
        .iter()
        .filter_map(|(contract_address, _)| {
            fetch_bound_contract_record(state, contract_address)
                .map(|record| (record.contract_subject.clone(), record))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        isi::{Grant, SetParameter, smart_contract_code::DeactivateContractInstance},
        nexus::DataSpaceId,
        parameter::custom::{CustomParameter, CustomParameterId},
        permission,
        prelude::*,
        smart_contract::manifest::{EntryPointKind, EntrypointDescriptor},
    };
    use iroha_executor_data_model::permission::parameter::CanSetParameters;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("smart contract code fixture key generation should succeed")
    }

    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }

    fn minimal_contract_artifact(
        abi_version: u8,
    ) -> (
        Vec<u8>,
        iroha_data_model::smart_contract::manifest::ContractManifest,
    ) {
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version,
        };
        let entrypoint = EntrypointDescriptor {
            name: "main".to_owned(),
            kind: EntryPointKind::View,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: None,
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        };
        let interface = ivm::EmbeddedContractInterfaceV1 {
            seiyaku_name: "TestContract".to_owned(),
            compiler_fingerprint: "iroha-core-test".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
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
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut out = meta.encode();
        out.extend_from_slice(&interface.encode_section());
        out.extend_from_slice(&code);
        let verified = ivm::verify_contract_artifact(&out).expect("valid test contract artifact");
        (out, verified.manifest)
    }

    fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
        minimal_contract_artifact(abi_version).0
    }

    fn lifecycle_contract(
        source: &str,
    ) -> (
        Vec<u8>,
        iroha_data_model::smart_contract::manifest::ContractManifest,
    ) {
        ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(source)
            .expect("compile lifecycle contract")
    }

    fn test_state() -> (State, AccountId, KeyPair) {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let kp = checked_keypair();
        let (pubkey, _) = kp.clone().into_parts();
        let dom: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let auth = AccountId::of(pubkey);
        let domain = Domain::new(dom.clone()).build(&auth);
        let account = Account::new(auth.clone()).build(&auth);
        let mut world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
        let mut permissions = permission::Permissions::new();
        assert!(permissions.insert(permission::Permission::new(
            iroha_data_model::smart_contract::CONTRACT_HAJIMARI_PERMISSION_NAME.to_owned(),
            Json::new(()),
        )));
        world
            .account_permissions_mut_for_testing()
            .insert(auth.clone(), permissions);
        let state = State::new_for_testing(world, kura, query);
        (state, auth, kp)
    }

    fn default_header(height: u64) -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(
            core::num::NonZeroU64::new(height).expect("block height must be non-zero"),
            None,
            None,
            None,
            0,
            0,
        )
    }

    #[test]
    fn registry_roundtrip_manifest_and_code() {
        let (state, authority, kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();

        // Register bytecode and manifest, then activate an authorized namespace binding.
        let (code, manifest) = minimal_contract_artifact(1);
        let code_hash =
            register_code_bytes(&authority, code.clone(), &mut stx).expect("register bytecode");

        let manifest = manifest.signed(&kp);
        register_manifest(&authority, manifest.clone(), &mut stx).expect("register manifest");
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        activate_instance(&authority, contract_address.clone(), code_hash, &mut stx)
            .expect("activate instance");
        assert_eq!(
            pending_contract_lifecycle(&stx.world, &contract_address)
                .expect("valid lifecycle state"),
            None,
            "contracts without hajimari remain immediately callable"
        );

        let alias = ContractAlias::from_components("registry", Some("wonderland"), "universal")
            .expect("valid contract alias");
        stx.world
            .bind_contract_alias(&contract_address, alias.clone(), None, None, 0)
            .expect("bind contract alias");
        assert_eq!(
            fetch_bound_contract_identity(&stx, &contract_address)
                .expect("consistent alias binding is callable")
                .contract_alias,
            Some(alias.clone())
        );
        stx.world.contract_aliases.remove(alias.clone());
        assert!(
            fetch_bound_contract_identity(&stx, &contract_address).is_none(),
            "a forward-only alias binding must fail closed before contract execution"
        );
        stx.world
            .contract_aliases
            .insert(alias, contract_address.clone());
        stx.world.clear_contract_alias(&contract_address);

        stx.apply();
        block.commit().expect("commit block");

        let view = state.view();
        // Manifest fetch
        let got_manifest = fetch_manifest(&view, &code_hash).expect("manifest stored");
        assert_eq!(got_manifest, manifest);
        // Bytecode fetch
        let got_code = fetch_code_bytes(&view, &code_hash).expect("code stored");
        assert_eq!(got_code, code);
        // Combined record fetch
        let record = fetch_record(&view, &code_hash).expect("record exists");
        assert_eq!(record.manifest, manifest);
        assert_eq!(record.code_bytes.as_deref(), Some(code.as_slice()));
        // Instance binding fetch
        let bound = fetch_instance_binding(&view, &contract_address).expect("binding exists");
        assert_eq!(bound, code_hash);
        let identity = fetch_bound_contract_identity(&view, &contract_address)
            .expect("lightweight binding exists");
        assert_eq!(identity.contract_address, contract_address);
        assert_eq!(identity.code_hash, code_hash);
        assert_eq!(identity.contract_alias, None);
        let borrowed = with_code_bytes(&view, &code_hash, |bytes| (bytes.as_ptr(), bytes.to_vec()))
            .expect("borrow stored bytes");
        assert_eq!(borrowed.1, code);
        let stored_ptr = view
            .world()
            .contract_code()
            .get(&code_hash)
            .expect("stored bytes")
            .as_ptr();
        assert_eq!(borrowed.0, stored_ptr, "borrow helper must not clone bytes");
    }

    #[test]
    fn protected_contract_activation_succeeds_with_governance_permission() {
        let (state, authority, kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();

        // Grant only the governance/parameter permissions needed to protect a namespace.
        let enact: permission::Permission =
            iroha_executor_data_model::permission::governance::CanEnactGovernance.into();
        Grant::account_permission(enact, authority.clone())
            .execute(&authority, &mut stx)
            .expect("grant CanEnactGovernance");
        let set_params: permission::Permission = CanSetParameters.into();
        Grant::account_permission(set_params, authority.clone())
            .execute(&authority, &mut stx)
            .expect("grant CanSetParameters");

        // Protect the `apps` namespace.
        let id = CustomParameterId("gov_protected_namespaces".parse().unwrap());
        let payload = iroha_primitives::json::Json::from(
            norito::json::array(["apps"]).expect("serialize protected namespaces"),
        );
        let custom = CustomParameter::new(id, payload);
        SetParameter::new(Parameter::Custom(custom))
            .execute(&authority, &mut stx)
            .expect("set protected namespaces");

        // Register code + manifest and activate under governance protection.
        let (code, manifest) = minimal_contract_artifact(1);
        let code_hash =
            register_code_bytes(&authority, code.clone(), &mut stx).expect("register bytecode");
        let manifest = manifest.signed(&kp);
        register_manifest(&authority, manifest, &mut stx).expect("register manifest");
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            1,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        activate_instance(&authority, contract_address.clone(), code_hash, &mut stx)
            .expect("governed activation");
        stx.apply();
        block.commit().expect("commit block");

        let view = state.view();
        let bound = fetch_instance_binding(&view, &contract_address).expect("binding exists");
        assert_eq!(bound, code_hash);
    }

    #[test]
    fn lifecycle_hooks_are_single_use_and_bound_to_real_activation_transitions() {
        let (state, authority, keypair) = test_state();
        let mut block = state.block(default_header(1));
        let mut transaction = block.transaction();
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            7,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");

        let (v1_code, v1_manifest) = lifecycle_contract(
            r#"
seiyaku LifecycleOne {
  hajimari() {}
  kaizen() {}
  kotoage fn run() authorize("CanRunLifecycleOne") {}
}
"#,
        );
        let v1_hash = register_code_bytes(&authority, v1_code, &mut transaction)
            .expect("register v1 bytecode");
        register_manifest(&authority, v1_manifest.signed(&keypair), &mut transaction)
            .expect("register v1 manifest");
        activate_instance(
            &authority,
            contract_address.clone(),
            v1_hash,
            &mut transaction,
        )
        .expect("activate v1");

        let hajimari = pending_contract_lifecycle(&transaction.world, &contract_address)
            .expect("valid lifecycle state")
            .expect("activation staged hajimari");
        assert!(matches!(
            hajimari,
            PendingContractLifecycle::Hajimari { code_hash, .. } if code_hash == v1_hash
        ));
        assert_eq!(
            validate_contract_lifecycle_call(
                &transaction.world,
                &contract_address,
                v1_hash,
                EntryPointKind::Hajimari,
            )
            .expect("new activation admits hajimari"),
            Some(hajimari)
        );
        assert!(
            validate_contract_lifecycle_call(
                &transaction.world,
                &contract_address,
                v1_hash,
                EntryPointKind::Kotoage,
            )
            .is_err(),
            "ordinary calls must wait for hajimari"
        );
        assert!(
            validate_contract_lifecycle_call(
                &transaction.world,
                &contract_address,
                v1_hash,
                EntryPointKind::Kaizen,
            )
            .is_err(),
            "a fresh activation is not a kaizen transition"
        );

        validate_contract_lifecycle_completion(&transaction.world, &contract_address, hajimari)
            .expect("live completion matches pending hajimari");
        set_pending_contract_lifecycle(&mut transaction, &contract_address, None);
        assert!(
            validate_contract_lifecycle_call(
                &transaction.world,
                &contract_address,
                v1_hash,
                EntryPointKind::Hajimari,
            )
            .is_err(),
            "hajimari cannot replay after consumption"
        );
        validate_contract_lifecycle_call(
            &transaction.world,
            &contract_address,
            v1_hash,
            EntryPointKind::Kotoage,
        )
        .expect("ordinary calls open after hajimari");

        activate_instance(
            &authority,
            contract_address.clone(),
            v1_hash,
            &mut transaction,
        )
        .expect("same-code activation is idempotent");
        assert_eq!(
            pending_contract_lifecycle(&transaction.world, &contract_address)
                .expect("valid lifecycle state"),
            None,
            "idempotent activation must not recreate a consumed hajimari"
        );

        let (v2_code, v2_manifest) = lifecycle_contract(
            r#"
seiyaku LifecycleTwo {
  hajimari() {}
  kaizen() {}
  kotoage fn run() authorize("CanRunLifecycleTwo") {}
}
"#,
        );
        let v2_hash = register_code_bytes(&authority, v2_code, &mut transaction)
            .expect("register v2 bytecode");
        register_manifest(&authority, v2_manifest.signed(&keypair), &mut transaction)
            .expect("register v2 manifest");
        let unauthorized_kaizen = activate_instance(
            &authority,
            contract_address.clone(),
            v2_hash,
            &mut transaction,
        )
        .expect_err("in-place replacement requires governance authorization");
        assert!(
            unauthorized_kaizen
                .to_string()
                .contains("CanEnactGovernance")
        );
        assert_eq!(
            transaction
                .world
                .contract_instances
                .get(&contract_address)
                .copied(),
            Some(v1_hash),
            "rejected kaizen must preserve the old binding"
        );
        Grant::account_permission(
            iroha_data_model::permission::Permission::new(
                "CanEnactGovernance".to_owned(),
                Json::new(()),
            ),
            authority.clone(),
        )
        .execute(&authority, &mut transaction)
        .expect("grant in-place kaizen governance permission");
        activate_instance(
            &authority,
            contract_address.clone(),
            v2_hash,
            &mut transaction,
        )
        .expect("replace the active binding");

        let kaizen = pending_contract_lifecycle(&transaction.world, &contract_address)
            .expect("valid lifecycle state")
            .expect("replacement staged kaizen");
        assert!(matches!(
            kaizen,
            PendingContractLifecycle::Kaizen {
                previous_code_hash,
                code_hash,
                ..
            } if previous_code_hash == v1_hash && code_hash == v2_hash
        ));
        assert_eq!(
            validate_contract_lifecycle_call(
                &transaction.world,
                &contract_address,
                v2_hash,
                EntryPointKind::Kaizen,
            )
            .expect("genuine code replacement admits kaizen"),
            Some(kaizen)
        );
        assert!(
            validate_contract_lifecycle_call(
                &transaction.world,
                &contract_address,
                v2_hash,
                EntryPointKind::Hajimari,
            )
            .is_err(),
            "an in-place kaizen is not a new hajimari activation"
        );
        assert!(
            validate_contract_lifecycle_call(
                &transaction.world,
                &contract_address,
                v1_hash,
                EntryPointKind::Kaizen,
            )
            .is_err(),
            "old code cannot consume the new code's kaizen transition"
        );

        validate_contract_lifecycle_completion(&transaction.world, &contract_address, kaizen)
            .expect("live completion matches pending kaizen");
        set_pending_contract_lifecycle(&mut transaction, &contract_address, None);
        assert!(
            validate_contract_lifecycle_call(
                &transaction.world,
                &contract_address,
                v2_hash,
                EntryPointKind::Kaizen,
            )
            .is_err(),
            "kaizen cannot replay after consumption"
        );
    }

    #[test]
    fn lifecycle_transitions_in_one_execution_have_distinct_ordinals() {
        let (state, authority, _) = test_state();
        let mut block = state.block(default_header(1));
        let mut transaction = block.transaction();
        transaction.tx_call_hash = Some(Hash::new(b"one-execution-two-lifecycle-transitions"));
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            80,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let code_hash = Hash::new(b"ordinal-regression-code");

        let first = new_pending_contract_lifecycle(
            &mut transaction,
            &contract_address,
            None,
            code_hash,
            EntryPointKind::Hajimari,
        )
        .expect("first transition");
        let second = new_pending_contract_lifecycle(
            &mut transaction,
            &contract_address,
            None,
            code_hash,
            EntryPointKind::Hajimari,
        )
        .expect("second transition");

        assert_ne!(
            first, second,
            "the per-execution ordinal must distinguish otherwise identical lifecycle mutations"
        );
    }

    #[test]
    fn stale_hajimari_completion_rejects_deactivate_reactivate_aba() {
        let (state, authority, keypair) = test_state();
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            81,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let (code, manifest) = lifecycle_contract(
            r#"
seiyaku LifecycleAba {
  hajimari() {}
  kotoage fn run() authorize("CanRunLifecycleAba") {}
}
"#,
        );

        let mut first_block = state.block(default_header(1));
        let mut first_transaction = first_block.transaction();
        first_transaction.tx_call_hash = Some(Hash::new(b"lifecycle-aba-first-activation"));
        let code_hash = register_code_bytes(&authority, code, &mut first_transaction)
            .expect("register lifecycle bytecode");
        register_manifest(
            &authority,
            manifest.signed(&keypair),
            &mut first_transaction,
        )
        .expect("register lifecycle manifest");
        activate_instance(
            &authority,
            contract_address.clone(),
            code_hash,
            &mut first_transaction,
        )
        .expect("first activation");
        let stale_transition =
            pending_contract_lifecycle(&first_transaction.world, &contract_address)
                .expect("valid first lifecycle state")
                .expect("first hajimari transition");
        first_transaction.apply();
        first_block.commit().expect("commit first activation");

        let mut completion_block = state.block(default_header(2));
        let mut completion = completion_block.transaction();
        set_pending_contract_lifecycle(&mut completion, &contract_address, None);
        completion.apply();
        completion_block.commit().expect("commit first completion");

        let mut deactivate_block = state.block(default_header(3));
        let mut deactivate = deactivate_block.transaction();
        deactivate.tx_call_hash = Some(Hash::new(b"lifecycle-aba-deactivation"));
        DeactivateContractInstance {
            contract_address: contract_address.clone(),
            reason: Some("ABA regression fixture".to_owned()),
        }
        .execute(&authority, &mut deactivate)
        .expect("authorized deactivation");
        deactivate.apply();
        deactivate_block.commit().expect("commit deactivation");

        let mut second_block = state.block(default_header(4));
        let mut second_transaction = second_block.transaction();
        second_transaction.tx_call_hash = Some(Hash::new(b"lifecycle-aba-second-activation"));
        activate_instance(
            &authority,
            contract_address.clone(),
            code_hash,
            &mut second_transaction,
        )
        .expect("second activation");
        let current_transition =
            pending_contract_lifecycle(&second_transaction.world, &contract_address)
                .expect("valid second lifecycle state")
                .expect("second hajimari transition");

        assert_ne!(
            current_transition, stale_transition,
            "a new activation must never recreate an earlier KLC1 record"
        );
        assert!(matches!(
            validate_contract_lifecycle_completion(
                &second_transaction.world,
                &contract_address,
                stale_transition,
            ),
            Err(ValidationFail::NotPermitted(_))
        ));
        validate_contract_lifecycle_completion(
            &second_transaction.world,
            &contract_address,
            current_transition,
        )
        .expect("the exact current activation remains completable");
    }

    #[test]
    fn corrupt_lifecycle_marker_fails_closed() {
        let (state, authority, _) = test_state();
        let mut block = state.block(default_header(1));
        let mut transaction = block.transaction();
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            8,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        transaction.world.smart_contract_state.insert(
            contract_lifecycle_state_key(&contract_address),
            b"not-a-lifecycle-record".to_vec(),
        );

        assert!(matches!(
            pending_contract_lifecycle(&transaction.world, &contract_address),
            Err(ValidationFail::InternalError(message))
                if message.contains("invalid lifecycle state")
        ));

        let mut trailing = PendingContractLifecycle::Hajimari {
            transition_id: Hash::new(b"noncanonical-lifecycle-transition"),
            code_hash: Hash::new(b"noncanonical-lifecycle-record"),
        }
        .encode();
        trailing.push(0);
        transaction
            .world
            .smart_contract_state
            .insert(contract_lifecycle_state_key(&contract_address), trailing);
        assert!(matches!(
            pending_contract_lifecycle(&transaction.world, &contract_address),
            Err(ValidationFail::InternalError(message))
                if message.contains("not canonical Norito")
        ));
    }

    #[test]
    fn register_code_obeys_size_cap() {
        let (state, authority, _kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();

        // Set very small cap via custom parameter to ensure registration fails.
        let id = CustomParameterId("max_contract_code_bytes".parse().unwrap());
        let cap = CustomParameter::new(id, iroha_primitives::json::Json::new(8u64));
        SetParameter::new(Parameter::Custom(cap))
            .execute(&authority, &mut stx)
            .expect("set cap");

        let code = minimal_ivm_program(1);
        let err = register_code_bytes(&authority, code, &mut stx).unwrap_err();
        match err {
            RegistryError::Instruction(inner) => {
                let msg = inner.to_string();
                assert!(
                    msg.contains("code bytes exceed cap"),
                    "unexpected instruction error: {msg}"
                );
            }
            other => panic!("expected instruction error, got {other:?}"),
        }
    }

    #[test]
    fn register_code_rejects_zero_cycle_ceiling() {
        let (state, authority, _kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();
        let (mut code, _) = minimal_contract_artifact(1);
        code[8..16].copy_from_slice(&0_u64.to_le_bytes());
        let code_hash = ivm::contract_code_hash(&code);

        let error = register_code_bytes(&authority, code, &mut stx)
            .expect_err("zero-cycle artifact registration must fail closed");
        assert!(
            error.to_string().contains("omits a non-zero `max_cycles`"),
            "unexpected registration error: {error}"
        );
        assert!(
            stx.world.contract_code.get(&code_hash).is_none(),
            "rejected artifact must not enter world state"
        );
    }

    #[test]
    fn register_code_rejects_cycle_ceiling_above_node_policy() {
        let (mut state, authority, _kp) = test_state();
        let mut pipeline = state.pipeline.clone();
        pipeline.ivm_max_cycles_upper_bound =
            core::num::NonZeroU64::new(1).expect("test ceiling is non-zero");
        state.set_pipeline(pipeline);
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();
        let (mut code, _) = minimal_contract_artifact(1);
        code[8..16].copy_from_slice(&2_u64.to_le_bytes());
        let code_hash = ivm::contract_code_hash(&code);

        let error = register_code_bytes(&authority, code, &mut stx)
            .expect_err("over-ceiling artifact registration must fail closed");
        let message = error.to_string();
        assert!(
            message.contains("`max_cycles` exceeds upper bound"),
            "unexpected registration error: {message}"
        );
        assert!(
            stx.world.contract_code.get(&code_hash).is_none(),
            "rejected artifact must not enter world state"
        );
    }

    #[test]
    fn register_manifest_requires_code_hash() {
        let (state, authority, _kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();

        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: None,
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        };
        let err = register_manifest(&authority, manifest, &mut stx).unwrap_err();
        matches!(err, RegistryError::MissingCodeHash);
    }

    #[test]
    fn current_subject_binding_initialization_builds_reverse_index_and_purges_legacy_markers() {
        let authority = AccountId::new(checked_keypair().public_key().clone());
        let address = ContractAddress::derive(0, &authority, 7, DataSpaceId::UNIVERSAL)
            .expect("contract address");
        let mut world = World::default();
        world
            .contract_instances
            .insert(address.clone(), Hash::new(b"active-contract"));
        let poisoned_key = contract_subject_history_key(&authority);
        world
            .smart_contract_state
            .insert(poisoned_key.clone(), b"guest-forged".to_vec());

        initialize_current_contract_subject_bindings(&mut world)
            .expect("initialize v2 subject ledger");

        let bindings = world.contract_subject_bindings.view();
        let binding = bindings.get(&address).expect("binding");
        assert_eq!(binding.version, CONTRACT_SUBJECT_VERSION_V2);
        assert_eq!(binding.subject, address.subject_id());
        assert_eq!(
            world
                .contract_subject_addresses
                .view()
                .get(&binding.subject),
            Some(&address)
        );
        assert!(
            world
                .smart_contract_state
                .view()
                .get(&poisoned_key)
                .is_none()
        );
        validate_contract_subject_bindings(&world).expect("validated v2 subject ledger");
    }

    #[test]
    fn legacy_subject_migration_is_idempotent_and_retains_deactivated_history() {
        let authority = AccountId::new(checked_keypair().public_key().clone());
        let active = ContractAddress::derive(0, &authority, 8, DataSpaceId::UNIVERSAL)
            .expect("active address");
        let retired = ContractAddress::derive(0, &authority, 9, DataSpaceId::UNIVERSAL)
            .expect("retired address");
        let manifest_hash = Hash::new(b"reviewed-finalized-history");
        let mut world = World::default();
        world
            .contract_instances
            .insert(active.clone(), Hash::new(b"active-contract"));

        migrate_legacy_contract_subject_bindings(
            &mut world,
            [active.clone(), retired.clone()],
            manifest_hash,
        )
        .expect("migrate legacy subject ledger");
        migrate_legacy_contract_subject_bindings(
            &mut world,
            [active.clone(), retired.clone()],
            manifest_hash,
        )
        .expect("reapplying identical migration is idempotent");

        let bindings = world.contract_subject_bindings.view();
        for address in [&active, &retired] {
            let binding = bindings.get(address).expect("historical binding");
            assert_eq!(binding.version, CONTRACT_SUBJECT_VERSION_V1);
            assert_eq!(binding.subject, legacy_contract_subject_id_v1(address));
            assert_eq!(binding.legacy_audit_manifest_hash, Some(manifest_hash));
            assert_eq!(
                world
                    .contract_subject_addresses
                    .view()
                    .get(&binding.subject),
                Some(address),
                "reverse index must retain active and deactivated subjects"
            );
        }
        assert!(
            world.contract_instances.view().get(&retired).is_none(),
            "retired address must remain historical rather than active"
        );
        validate_contract_subject_bindings(&world).expect("validated legacy subject ledger");
    }
}
