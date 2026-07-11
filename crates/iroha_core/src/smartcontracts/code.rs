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
    smart_contract::manifest::ContractManifest,
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
/// Manifest registration is public. Networks can still protect specific
/// namespaces at activation time via `gov_protected_namespaces`. The manifest
/// must include `code_hash`, and the corresponding bytecode must already be
/// stored as a verified self-describing artifact.
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
/// Bytecode registration is public; namespace protection applies when
/// instances are activated.
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

/// Bind `contract_address` to a `code_hash` to activate an instance.
///
/// The binding is idempotent: calling this helper with the same mapping is a
/// no-op, while conflicting mappings result in an error from the underlying ISI.
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
        isi::{Grant, SetParameter},
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
            kind: EntryPointKind::Public,
            params: Vec::new(),
            return_type: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: None,
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        };
        let interface = ivm::EmbeddedContractInterfaceV1 {
            compiler_fingerprint: "iroha-core-test".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: entrypoint.name.clone(),
                kind: entrypoint.kind,
                params: entrypoint.params.clone(),
                return_type: entrypoint.return_type.clone(),
                permission: entrypoint.permission.clone(),
                read_keys: entrypoint.read_keys.clone(),
                write_keys: entrypoint.write_keys.clone(),
                access_hints_complete: entrypoint.access_hints_complete,
                access_hints_skipped: entrypoint.access_hints_skipped.clone(),
                triggers: entrypoint.triggers.clone(),
                entry_pc: 0,
            }],
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

    fn test_state() -> (State, AccountId, KeyPair) {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let kp = checked_keypair();
        let (pubkey, _) = kp.clone().into_parts();
        let dom: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let auth = AccountId::of(pubkey);
        let domain = Domain::new(dom.clone()).build(&auth);
        let account = Account::new(auth.clone()).build(&auth);
        let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
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

        // Register bytecode and manifest, then activate a public namespace binding.
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
    fn register_manifest_requires_code_hash() {
        let (state, authority, _kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();

        let manifest = ContractManifest {
            code_hash: None,
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
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
