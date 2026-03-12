use core::str::FromStr;
use std::{
    any::Any,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    num::{NonZeroU16, NonZeroU64},
    path::PathBuf,
    sync::Arc,
};

use iroha_crypto::{Hash as CryptoHash, HashOf, PublicKey};
pub use iroha_data_model::account::AccountId;
pub use iroha_data_model::prelude::{
    AssetDefinitionId, DomainId, Mintable, Name, NftId, Peer, ScopedAccountId,
};
use iroha_data_model::{
    isi::{smart_contract_code as scode, transfer::TransferAssetBatch},
    nexus::{AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot, DataSpaceId, LaneId},
    proof::{ProofAttachment, VerifyingKeyId},
};
use iroha_primitives::json::Json;
use iroha_primitives::numeric::{Numeric, NumericSpec};
use norito::{
    decode_from_bytes,
    derive::{Decode, Encode},
    json::{self as njson},
};
use sha2::{Digest as _, Sha256};

use crate::{
    VMError,
    axt::{self, AssetHandle, AxtPolicy, ProofBlob, RemoteSpendIntent, TouchManifest},
    host::IVMHost,
    ivm::IVM,
    parallel::StateUpdate,
    pointer_abi::{self, PointerType},
    schema_registry::SchemaRegistry,
    state_overlay::{DurableStateOverlay, DurableStateSnapshot},
    syscalls,
};

/// Definition of an asset type.
#[derive(Clone, Debug)]
struct AssetDefinition {
    mintable: Mintable,
    total_supply: Numeric,
}

impl AssetDefinition {
    fn new(mintable: Mintable) -> Self {
        Self {
            mintable,
            total_supply: Numeric::zero(),
        }
    }
}

/// NFT state tracking the current owner, stored metadata, and the issuing authority.
#[derive(Clone, Debug)]
struct NftRecord {
    owner: AccountId,
    data: Vec<u8>,
    issuer: AccountId,
}

/// Per-dataspace policy sourced from Space Directory/WSV for AXT enforcement.
#[derive(Clone, Debug, Default, Encode, Decode)]
pub struct DataspaceAxtPolicy {
    pub manifest_root: [u8; 32],
    pub target_lane: LaneId,
    pub min_handle_era: u64,
    pub min_sub_nonce: u64,
    pub current_slot: u64,
}

impl DataspaceAxtPolicy {
    fn to_model_entry(&self) -> AxtPolicyEntry {
        AxtPolicyEntry {
            manifest_root: self.manifest_root,
            target_lane: self.target_lane,
            min_handle_era: self.min_handle_era,
            min_sub_nonce: self.min_sub_nonce,
            current_slot: self.current_slot,
        }
    }

    fn from_model_entry(entry: &AxtPolicyEntry) -> Self {
        Self {
            manifest_root: entry.manifest_root,
            target_lane: entry.target_lane,
            min_handle_era: entry.min_handle_era,
            min_sub_nonce: entry.min_sub_nonce,
            current_slot: entry.current_slot,
        }
    }
}

/// Space Directory-backed AXT policy used by WsvHost (and injectable into CoreHost in tests).
#[derive(Clone)]
pub struct SpaceDirectoryAxtPolicy {
    policies: HashMap<DataSpaceId, DataspaceAxtPolicy>,
    slot_length_ms: NonZeroU64,
    max_clock_skew_ms: u64,
}

impl Default for SpaceDirectoryAxtPolicy {
    fn default() -> Self {
        Self {
            policies: HashMap::new(),
            slot_length_ms: NonZeroU64::new(1).expect("default slot length must be non-zero"),
            max_clock_skew_ms: 0,
        }
    }
}

impl SpaceDirectoryAxtPolicy {
    pub fn from_snapshot(policies: HashMap<DataSpaceId, DataspaceAxtPolicy>) -> Self {
        Self::from_snapshot_with_timing(
            policies,
            NonZeroU64::new(1).expect("non-zero slot length"),
            0,
        )
    }

    pub fn from_snapshot_with_timing(
        policies: HashMap<DataSpaceId, DataspaceAxtPolicy>,
        slot_length_ms: NonZeroU64,
        max_clock_skew_ms: u64,
    ) -> Self {
        Self {
            policies,
            slot_length_ms,
            max_clock_skew_ms,
        }
    }

    pub fn from_policy_snapshot(snapshot: &AxtPolicySnapshot) -> Self {
        Self::from_policy_snapshot_with_timing(
            snapshot,
            NonZeroU64::new(1).expect("non-zero slot length"),
            0,
        )
    }

    pub fn from_policy_snapshot_with_timing(
        snapshot: &AxtPolicySnapshot,
        slot_length_ms: NonZeroU64,
        max_clock_skew_ms: u64,
    ) -> Self {
        let mut policies = HashMap::new();
        for binding in &snapshot.entries {
            policies.insert(
                binding.dsid,
                DataspaceAxtPolicy::from_model_entry(&binding.policy),
            );
        }
        Self::from_snapshot_with_timing(policies, slot_length_ms, max_clock_skew_ms)
    }

    pub fn with_current_slot(mut self, slot: u64) -> Self {
        for policy in self.policies.values_mut() {
            policy.current_slot = slot;
        }
        self
    }
}

impl AxtPolicy for SpaceDirectoryAxtPolicy {
    fn allow_touch(&self, _dsid: DataSpaceId, _manifest: &TouchManifest) -> Result<(), VMError> {
        Ok(())
    }

    fn allow_handle(&self, usage: &axt::HandleUsage) -> Result<(), VMError> {
        let dsid = usage.intent.asset_dsid;
        let Some(policy) = self.policies.get(&dsid) else {
            return Err(VMError::PermissionDenied);
        };
        if policy.manifest_root.iter().all(|b| *b == 0) {
            return Err(VMError::PermissionDenied);
        }
        if usage
            .handle
            .manifest_view_root
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(VMError::PermissionDenied);
        }
        if let Some(requested) = usage.handle.max_clock_skew_ms
            && u64::from(requested) > self.max_clock_skew_ms
        {
            return Err(VMError::PermissionDenied);
        }
        let expiry_slot = axt::expiry_slot_with_skew(
            usage.handle.expiry_slot,
            self.slot_length_ms,
            self.max_clock_skew_ms,
            usage.handle.max_clock_skew_ms,
        );
        if policy.current_slot > 0 && policy.current_slot > expiry_slot {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.target_lane != policy.target_lane {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.manifest_view_root.as_slice() != policy.manifest_root.as_slice() {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.handle_era < policy.min_handle_era {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.sub_nonce < policy.min_sub_nonce {
            return Err(VMError::PermissionDenied);
        }
        Ok(())
    }
}

/// Permission tokens used for authorising operations.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum PermissionToken {
    RegisterDomain,
    RegisterAccount,
    RegisterAssetDefinition,
    /// Permission to register ZK policy for an asset
    RegisterZkAsset(AssetDefinitionId),
    MintAsset(AssetDefinitionId),
    BurnAsset(AssetDefinitionId),
    TransferAsset(AssetDefinitionId),
    /// Permission to add a signatory for the given account
    AddSignatory(AccountId),
    /// Permission to remove a signatory for the given account
    RemoveSignatory(AccountId),
    /// Permission to update the quorum for the given account
    SetAccountQuorum(AccountId),
    /// Permission to set account detail for the given account
    SetAccountDetail(AccountId),
    /// Permission to shield public funds for an asset
    Shield(AssetDefinitionId),
    /// Permission to unshield private funds for an asset
    Unshield(AssetDefinitionId),
    /// Permission to read balances of the given account
    ReadAccountAssets(AccountId),
    /// Permission to create, delete, grant, and revoke roles.
    ManageRoles,
    /// Permission to grant and revoke direct permissions.
    ManagePermissions,
    /// Permission to create, mutate, and remove triggers.
    ManageTriggers,
    /// Permission to register and unregister peers.
    ManagePeers,
}

/// Minimal account representation tracking signatories, quorum, and metadata.
#[derive(Clone, Debug)]
struct Account {
    /// Public keys (or opaque identifiers) authorised to sign for the account.
    signatories: HashSet<String>,
    /// Required number of signatures for multisig operations. `1` by default.
    quorum: u32,
    /// Account detail entries keyed by `Name`.
    detail: HashMap<String, Vec<u8>>,
}

impl Account {
    fn insert_signatory(&mut self, key: String) -> bool {
        self.signatories.insert(key)
    }

    fn remove_signatory(&mut self, key: &str) -> bool {
        self.signatories.remove(key)
    }

    fn set_quorum(&mut self, quorum: u32) {
        self.quorum = quorum.max(1);
    }

    fn set_detail(&mut self, key: &str, value: Vec<u8>) {
        self.detail.insert(key.to_string(), value);
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            signatories: HashSet::new(),
            quorum: 1,
            detail: HashMap::new(),
        }
    }
}

/// A very small in-memory mock of Iroha's World State View (WSV).
///
/// Scope and purpose
/// - Provides minimal primitives for tests (domain/account/asset/nft/roles/triggers/peers) and
///   a compact shielded (ZK) asset state with recent-root windows.
/// - Implements a developer JSON-envelope path for a subset of admin-style operations via
///   `SMARTCONTRACT_EXECUTE_QUERY (0xA1)` and `SMARTCONTRACT_EXECUTE_INSTRUCTION (0xA0)`.
///   Supported envelopes include:
///   - Queries: `wsv.get_balance`, `wsv.list_triggers`, `wsv.has_permission`,
///     `wsv.list_domains_for_subject`, `wsv.list_accounts_for_domain`.
///   - Admin: `wsv.create_role`, `wsv.grant_role`, `wsv.revoke_role`,
///     `wsv.grant_permission`, `wsv.revoke_permission`, `wsv.create_trigger`,
///     `wsv.set_trigger_enabled`, `wsv.remove_trigger`.
/// - Production hosts should prefer Norito TLVs; the JSON envelope path is intended for tests
///   and dev tooling and mirrors ISI/syscall behavior under pointer‑ABI validation.
#[derive(Clone, Default)]
pub struct MockWorldStateView {
    domains: HashMap<DomainId, ()>,
    domain_accounts: HashMap<DomainId, HashSet<AccountId>>,
    accounts: HashMap<AccountId, Account>,
    permissions: HashMap<AccountId, HashSet<PermissionToken>>,
    asset_definitions: HashMap<AssetDefinitionId, AssetDefinition>,
    balances: HashMap<(AccountId, AssetDefinitionId), Numeric>,
    nfts: HashMap<NftId, NftRecord>,
    peers: HashSet<Peer>,
    triggers: HashMap<String, bool>,
    roles: HashMap<String, HashSet<PermissionToken>>,
    role_assignments: HashMap<AccountId, HashSet<String>>, // account subject -> role names
    // ZK (shielded) state
    zk_assets: HashMap<AssetDefinitionId, ZkAssetState>,
    elections: HashMap<String, ElectionState>,
    /// Events emitted by ZK operations for test visibility
    zk_events: Vec<ZkEvent>,
    /// Durable smart-contract state (path -> NoritoBytes payload TLVs)
    state_overlay: DurableStateOverlay,
    /// Manifest registry keyed by code hash (presence-only for gating).
    contract_manifests: HashSet<CryptoHash>,
    /// Stored contract bytecode keyed by code hash.
    contract_code: HashMap<CryptoHash, Vec<u8>>,
    /// Active contract instances keyed by (namespace, contract_id).
    contract_instances: HashMap<(String, String), CryptoHash>,
    /// Logical wall-clock timestamp used for time-gated operations (ms since epoch).
    current_time_ms: u64,
    /// Slot length (ms) used to derive current slot for expiry checks.
    slot_length_ms: u64,
    /// Maximum wall-clock skew (ms) tolerated for AXT expiry calculations.
    axt_max_clock_skew_ms: u64,
    verifying_keys: BTreeMap<VerifyingKeyId, MockVerifyingKeyRecord>,
    axt_policies: HashMap<DataSpaceId, DataspaceAxtPolicy>,
}

pub struct ZkPolicyConfig {
    pub mode: ZkAssetMode,
    pub allow_shield: bool,
    pub allow_unshield: bool,
    pub vk_transfer: Option<VerifyingKeyId>,
    pub vk_unshield: Option<VerifyingKeyId>,
    pub vk_shield: Option<VerifyingKeyId>,
}

impl MockWorldStateView {
    /// Create an empty mock WSV.
    pub fn new() -> Self {
        Self {
            domains: HashMap::new(),
            domain_accounts: HashMap::new(),
            accounts: HashMap::new(),
            permissions: HashMap::new(),
            asset_definitions: HashMap::new(),
            balances: HashMap::new(),
            nfts: HashMap::new(),
            peers: HashSet::new(),
            triggers: HashMap::new(),
            roles: HashMap::new(),
            role_assignments: HashMap::new(),
            zk_assets: HashMap::new(),
            elections: HashMap::new(),
            zk_events: Vec::new(),
            state_overlay: DurableStateOverlay::in_memory(),
            contract_manifests: HashSet::new(),
            contract_code: HashMap::new(),
            contract_instances: HashMap::new(),
            current_time_ms: 0,
            slot_length_ms: 1,
            axt_max_clock_skew_ms: 0,
            verifying_keys: BTreeMap::new(),
            axt_policies: HashMap::new(),
        }
    }

    /// Create a mock WSV whose contract state persists to the provided path.
    pub fn with_state_store(path: PathBuf) -> Result<Self, VMError> {
        let mut base = Self::new();
        base.state_overlay = DurableStateOverlay::with_persist_path(path)?;
        Ok(base)
    }

    /// Reconfigure the contract-state persistence path after construction.
    pub fn set_state_store_path(&mut self, path: PathBuf) -> Result<(), VMError> {
        self.state_overlay = DurableStateOverlay::with_persist_path(path)?;
        Ok(())
    }

    /// Override the logical wall-clock timestamp (milliseconds since epoch).
    ///
    /// Tests should set this to exercise election time windows deterministically.
    pub fn set_current_time_ms(&mut self, ts: u64) {
        self.current_time_ms = ts;
    }

    /// Configure slot length (ms) used for deriving current slot in AXT checks.
    pub fn set_slot_length_ms(&mut self, len: u64) {
        self.slot_length_ms = len.max(1);
    }

    /// Configure the maximum wall-clock skew (ms) tolerated for AXT expiry checks.
    pub fn set_max_clock_skew_ms(&mut self, skew_ms: u64) {
        self.axt_max_clock_skew_ms = skew_ms;
    }

    /// Expose the configured slot length used for AXT calculations.
    pub fn slot_length_ms(&self) -> NonZeroU64 {
        NonZeroU64::new(self.slot_length_ms.max(1)).expect("slot length is clamped to non-zero")
    }

    /// Expose the configured wall-clock skew allowance for AXT expiry checks.
    pub fn max_clock_skew_ms(&self) -> u64 {
        self.axt_max_clock_skew_ms
    }

    /// Derive the current slot from logical time and slot length.
    pub fn current_slot(&self) -> u64 {
        let len = self.slot_length_ms.max(1);
        self.current_time_ms / len
    }

    /// Install or update an AXT policy entry for a dataspace.
    pub fn set_axt_policy(&mut self, dsid: DataSpaceId, policy: DataspaceAxtPolicy) {
        let mut policy = policy;
        if policy.current_slot == 0 {
            policy.current_slot = self.current_slot();
        }
        self.axt_policies.insert(dsid, policy);
    }

    /// Snapshot all configured AXT policy entries.
    pub fn axt_policy_snapshot(&self) -> HashMap<DataSpaceId, DataspaceAxtPolicy> {
        self.axt_policies.clone()
    }

    /// Emit a data-model AXT policy snapshot for block/replication plumbing.
    pub fn axt_policy_snapshot_model(&self) -> AxtPolicySnapshot {
        let fallback_slot = if self
            .axt_policies
            .values()
            .any(|policy| policy.current_slot != 0)
        {
            None
        } else {
            Some(self.current_slot())
        };
        let mut entries: Vec<_> = self
            .axt_policies
            .iter()
            .map(|(dsid, policy)| {
                let mut entry = policy.to_model_entry();
                if let Some(slot) = fallback_slot {
                    entry.current_slot = slot;
                }
                AxtPolicyBinding {
                    dsid: *dsid,
                    policy: entry,
                }
            })
            .collect();
        entries.sort_by_key(|binding| binding.dsid);
        let version = AxtPolicySnapshot::compute_version(&entries);
        AxtPolicySnapshot { version, entries }
    }

    /// Load AXT policies from a data-model snapshot.
    pub fn load_axt_policy_snapshot_model(&mut self, snapshot: &AxtPolicySnapshot) {
        let map = snapshot
            .entries
            .iter()
            .map(|binding| {
                (
                    binding.dsid,
                    DataspaceAxtPolicy::from_model_entry(&binding.policy),
                )
            })
            .collect();
        self.axt_policies = map;
    }

    /// Return the logical wall-clock timestamp used for gating elections.
    pub fn current_time_ms(&self) -> u64 {
        self.current_time_ms
    }

    // -----------------------------
    // Smart-contract durable state (mock)
    // -----------------------------

    pub fn sc_get(&self, path: &str) -> Option<Vec<u8>> {
        let out = self.state_overlay.get(path);
        println!(
            "sc_get: {path} -> {}",
            if out.is_some() { "hit" } else { "miss" }
        );
        out
    }

    pub fn sc_set(&mut self, path: &str, value: Vec<u8>) -> Result<(), VMError> {
        println!("sc_set: {path} -> {value:?}");
        self.state_overlay.set(path, value)
    }

    pub fn sc_del(&mut self, path: &str) -> Result<(), VMError> {
        self.state_overlay.del(path)
    }

    pub fn sc_snapshot(&self) -> DurableStateSnapshot {
        self.state_overlay.checkpoint()
    }

    pub fn sc_restore(&mut self, snapshot: &DurableStateSnapshot) -> Result<(), VMError> {
        self.state_overlay.restore(snapshot)
    }

    pub fn sc_flush(&self) -> Result<(), VMError> {
        self.state_overlay.flush()
    }

    /// Record a manifest keyed by the supplied `code_hash`.
    pub fn insert_contract_manifest(&mut self, code_hash: CryptoHash) {
        self.contract_manifests.insert(code_hash);
    }

    /// Store contract bytecode for tests that exercise removal flows.
    pub fn insert_contract_code(&mut self, code_hash: CryptoHash, code: Vec<u8>) {
        self.contract_code.insert(code_hash, code);
    }

    /// Bind a contract instance in the mock registry.
    pub fn bind_contract_instance(
        &mut self,
        namespace: impl Into<String>,
        contract_id: impl Into<String>,
        code_hash: CryptoHash,
    ) {
        self.contract_instances
            .insert((namespace.into(), contract_id.into()), code_hash);
    }

    // -----------------------------
    // ZK shielded ledger handlers (permissions and full Merkle enforcement outstanding)
    // -----------------------------

    /// Register a ZK policy for an existing asset definition.
    pub fn register_zk_asset(&mut self, asset: AssetDefinitionId, policy: ZkPolicyConfig) -> bool {
        if !self.asset_definitions.contains_key(&asset) {
            return false;
        }
        let vk_transfer_binding = policy
            .vk_transfer
            .as_ref()
            .map(|id| self.binding_from_registry(id));
        let vk_unshield_binding = policy
            .vk_unshield
            .as_ref()
            .map(|id| self.binding_from_registry(id));
        let vk_shield_binding = policy
            .vk_shield
            .as_ref()
            .map(|id| self.binding_from_registry(id));
        let st = self.zk_assets.entry(asset.clone()).or_default();
        st.mode = policy.mode;
        st.allow_shield = policy.allow_shield;
        st.allow_unshield = policy.allow_unshield;
        st.vk_transfer = vk_transfer_binding;
        st.vk_unshield = vk_unshield_binding;
        st.vk_shield = vk_shield_binding;
        // Emit a policy-updated event
        self.zk_events.push(ZkEvent::ZkPolicyUpdated {
            asset: asset.clone(),
            mode: policy.mode,
            allow_shield: policy.allow_shield,
            allow_unshield: policy.allow_unshield,
        });
        true
    }

    /// Shield `amount` from `from` by appending a note commitment for `asset`.
    /// Permissions, proof validation, and Merkle verification are intentionally omitted in this mock.
    pub fn shield(
        &mut self,
        from: &ScopedAccountId,
        asset: &AssetDefinitionId,
        amount: Numeric,
        note_commitment: [u8; 32],
    ) -> bool {
        if !self.asset_definitions.contains_key(asset) {
            return false;
        }
        if !self.account_is_linked(from) {
            return false;
        }
        if !Self::is_unsigned_scale0(&amount) {
            return false;
        }
        let from_subject = Self::account_subject(from);
        let st = self.zk_assets.entry(asset.clone()).or_default();
        match st.mode {
            ZkAssetMode::Hybrid => {
                if !st.allow_shield {
                    return false;
                }
                // Debit public balance
                let key = (from_subject, asset.clone());
                let current = self
                    .balances
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(Numeric::zero);
                let remaining = match current.checked_sub(amount) {
                    Some(val) => val,
                    None => return false,
                };
                if remaining.mantissa().is_negative() {
                    return false;
                }
                if remaining.is_zero() {
                    self.balances.remove(&key);
                } else {
                    self.balances.insert(key, remaining);
                }
                let root = st.push_commitment(note_commitment);
                self.zk_events.push(ZkEvent::CommitmentAdded {
                    asset: asset.clone(),
                    commitment: note_commitment,
                    new_root: *root.as_ref(),
                });
                true
            }
            ZkAssetMode::ZkNative => {
                // No public balance to debit in native mode
                let root = st.push_commitment(note_commitment);
                self.zk_events.push(ZkEvent::CommitmentAdded {
                    asset: asset.clone(),
                    commitment: note_commitment,
                    new_root: *root.as_ref(),
                });
                true
            }
        }
    }

    /// Private transfer within shielded ledger: consume nullifiers and append output commitments.
    /// Proof verification and Merkle root binding are not modelled in this mock implementation.
    pub fn zk_transfer(
        &mut self,
        asset: &AssetDefinitionId,
        inputs: &[[u8; 32]],
        outputs: &[[u8; 32]],
        proof: &ProofAttachment,
    ) -> bool {
        let st = self.zk_assets.entry(asset.clone()).or_default();
        if let Some(binding) = st.vk_transfer.as_ref()
            && !binding.matches(proof)
        {
            return false;
        }
        // Consume nullifiers (fail if any already used)
        for n in inputs {
            if !st.nullifiers.insert(*n) {
                return false;
            }
        }
        for c in outputs {
            let root = st.push_commitment(*c);
            self.zk_events.push(ZkEvent::CommitmentAdded {
                asset: asset.clone(),
                commitment: *c,
                new_root: *root.as_ref(),
            });
        }
        true
    }

    /// Unshield: consume nullifiers and credit public balance.
    /// Change output accounting and full proof semantics remain unimplemented in this mock.
    pub fn unshield(
        &mut self,
        to: &ScopedAccountId,
        asset: &AssetDefinitionId,
        public_amount: Numeric,
        inputs: &[[u8; 32]],
        proof: &ProofAttachment,
    ) -> bool {
        if !self.account_is_linked(to) {
            return false;
        }
        let st = self.zk_assets.entry(asset.clone()).or_default();
        if st.mode != ZkAssetMode::Hybrid || !st.allow_unshield {
            return false;
        }
        if !Self::is_unsigned_scale0(&public_amount) {
            return false;
        }
        if let Some(binding) = st.vk_unshield.as_ref()
            && !binding.matches(proof)
        {
            return false;
        }
        for n in inputs {
            if !st.nullifiers.insert(*n) {
                return false;
            }
        }
        // Credit public balance
        let to_subject = Self::account_subject(to);
        let key = (to_subject.clone(), asset.clone());
        let current = self
            .balances
            .get(&key)
            .cloned()
            .unwrap_or_else(Numeric::zero);
        let next = match current.checked_add(public_amount.clone()) {
            Some(val) => val,
            None => return false,
        };
        self.balances.insert(key, next);
        // Emit an unshield event (no new root)
        self.zk_events.push(ZkEvent::Unshielded {
            asset: asset.clone(),
            to: to_subject,
            public_amount,
        });
        true
    }

    /// Return latest and recent roots for the asset's shielded ledger.
    pub fn get_roots(
        &self,
        asset: &AssetDefinitionId,
        max: usize,
    ) -> ([u8; 32], Vec<[u8; 32]>, u32) {
        if let Some(st) = self.zk_assets.get(asset) {
            let latest = st
                .root_history
                .last()
                .map(|h| *h.as_ref())
                .unwrap_or([0u8; 32]);
            let list: Vec<[u8; 32]> = if max == 0 || st.root_history.len() <= max {
                st.root_history.iter().map(|h| *h.as_ref()).collect()
            } else {
                st.root_history[st.root_history.len() - max..]
                    .iter()
                    .map(|h| *h.as_ref())
                    .collect()
            };
            (latest, list, st.root_history.len() as u32)
        } else {
            ([0u8; 32], Vec::new(), 0)
        }
    }

    /// Test helper: drain and return accumulated ZK events.
    pub fn drain_zk_events(&mut self) -> Vec<ZkEvent> {
        core::mem::take(&mut self.zk_events)
    }

    /// Create an election with parameters.
    pub fn create_election(
        &mut self,
        election_id: String,
        options: u32,
        eligible_root: [u8; 32],
        start_ts: u64,
        end_ts: u64,
    ) -> bool {
        let e = ElectionState {
            options,
            eligible_root,
            start_ts,
            end_ts,
            finalized: false,
            tally: vec![0; options as usize],
            ballot_nullifiers: HashSet::new(),
            ciphertexts: Vec::new(),
        };
        self.elections.insert(election_id, e).is_none()
    }

    /// Submit a ballot ciphertext with a unique nullifier.
    /// Enforces the election time window and basic proof structure checks.
    pub fn submit_ballot(
        &mut self,
        election_id: &str,
        ciphertext: Vec<u8>,
        nullifier: [u8; 32],
        proof: ProofAttachment,
    ) -> bool {
        if !self.validate_vote_proof(&proof) {
            return false;
        }
        let Some(e) = self.elections.get_mut(election_id) else {
            return false;
        };
        if e.finalized {
            return false;
        }
        if self.current_time_ms < e.start_ts || self.current_time_ms > e.end_ts {
            return false;
        }
        if !e.ballot_nullifiers.insert(nullifier) {
            return false;
        }
        e.ciphertexts.push(ciphertext);
        true
    }

    fn validate_vote_proof(&self, proof: &ProofAttachment) -> bool {
        if proof.backend != proof.proof.backend {
            return false;
        }
        if proof.proof.bytes.is_empty() {
            return false;
        }
        if proof.vk_inline.is_none() && proof.vk_ref.is_none() {
            return false;
        }
        if let Some(vk) = &proof.vk_inline
            && (vk.backend != proof.backend || vk.bytes.is_empty())
        {
            return false;
        }
        if let Some(id) = &proof.vk_ref
            && !self.verifying_keys.contains_key(id)
        {
            return false;
        }
        proof.envelope_hash.is_some()
    }

    /// Finalize an election with a provided tally.
    pub fn finalize_election(
        &mut self,
        election_id: &str,
        tally: Vec<u64>,
        proof: ProofAttachment,
    ) -> bool {
        if !self.validate_vote_proof(&proof) {
            return false;
        }
        let Some(e) = self.elections.get_mut(election_id) else {
            return false;
        };
        if e.finalized {
            return false;
        }
        if tally.len() != e.options as usize {
            return false;
        }
        e.tally = tally;
        e.finalized = true;
        true
    }

    fn account_subject(account: &ScopedAccountId) -> AccountId {
        account.subject_id()
    }

    fn account_is_linked(&self, account: &ScopedAccountId) -> bool {
        let subject = Self::account_subject(account);
        self.domain_accounts
            .get(account.domain())
            .is_some_and(|subjects| subjects.contains(&subject))
    }

    fn subject_has_any_domain(&self, subject: &AccountId) -> bool {
        self.domain_accounts
            .values()
            .any(|subjects| subjects.contains(subject))
    }

    /// List all domains currently linked to the supplied account subject.
    ///
    /// The returned list is sorted for deterministic test assertions.
    #[must_use]
    pub fn linked_domains_for_subject(&self, subject: &AccountId) -> Vec<DomainId> {
        let mut domains: Vec<DomainId> = self
            .domain_accounts
            .iter()
            .filter_map(|(domain, subjects)| {
                if subjects.contains(subject) {
                    Some(domain.clone())
                } else {
                    None
                }
            })
            .collect();
        domains.sort();
        domains
    }

    /// List all account subjects currently linked to a domain.
    ///
    /// The returned list is sorted for deterministic test assertions.
    #[must_use]
    pub fn linked_subjects_for_domain(&self, domain: &DomainId) -> Vec<AccountId> {
        let mut subjects: Vec<AccountId> = self
            .domain_accounts
            .get(domain)
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        subjects.sort();
        subjects
    }

    /// Link an existing account subject into a domain.
    ///
    /// Returns `true` when the link is newly created and `false` when the
    /// domain does not exist or the subject is already linked.
    pub fn link_subject_to_domain(&mut self, subject: AccountId, domain: DomainId) -> bool {
        if !self.domains.contains_key(&domain) {
            return false;
        }
        self.accounts.entry(subject.clone()).or_default();
        self.domain_accounts
            .entry(domain)
            .or_default()
            .insert(subject)
    }

    /// Unlink an account subject from a specific domain.
    ///
    /// If this is the final domain link for the subject, non-zero balances and
    /// NFT ownership still prevent unlinking so resources are not orphaned.
    /// Subject-level account state is otherwise preserved.
    pub fn unlink_subject_from_domain(&mut self, subject: &AccountId, domain: &DomainId) -> bool {
        let Some(subjects) = self.domain_accounts.get_mut(domain) else {
            return false;
        };
        if !subjects.remove(subject) {
            return false;
        }
        if subjects.is_empty() {
            self.domain_accounts.remove(domain);
        }
        if self.subject_has_any_domain(subject) {
            return true;
        }
        let has_bal = self
            .balances
            .iter()
            .any(|((acc, _), amount)| acc == subject && !amount.is_zero());
        let has_nfts = self.nfts.values().any(|rec| rec.owner == *subject);
        if has_bal || has_nfts {
            self.domain_accounts
                .entry(domain.clone())
                .or_default()
                .insert(subject.clone());
            return false;
        }
        true
    }

    fn canonical_account_id_for_subject(&self, subject: &AccountId) -> Option<ScopedAccountId> {
        let domain = self
            .domain_accounts
            .iter()
            .filter(|(_, subjects)| subjects.contains(subject))
            .map(|(domain, _)| domain.clone())
            .min()?;
        Some(subject.to_account_id(domain))
    }

    /// Test helper: register an account without permission checks or domain validation.
    /// Intended for unit tests that need to seed the mock quickly.
    pub fn add_account_unchecked(&mut self, id: ScopedAccountId) {
        let subject = Self::account_subject(&id);
        self.domains.entry(id.domain().clone()).or_default();
        self.domain_accounts
            .entry(id.domain().clone())
            .or_default()
            .insert(subject.clone());
        self.accounts.entry(subject).or_default();
    }

    /// Insert a verifying key record for ZK bindings.
    pub fn insert_verifying_key(&mut self, id: VerifyingKeyId, bytes: Vec<u8>) {
        let commitment = hash_vk_bytes(&id.backend.to_string(), &bytes);
        self.verifying_keys
            .insert(id, MockVerifyingKeyRecord { commitment });
    }

    /// Grant a permission token to `account`.
    pub fn grant_permission(&mut self, account: &ScopedAccountId, token: PermissionToken) {
        self.permissions
            .entry(Self::account_subject(account))
            .or_default()
            .insert(token);
    }

    /// Revoke a permission token from `account`.
    pub fn revoke_permission(&mut self, account: &ScopedAccountId, token: &PermissionToken) {
        if let Some(set) = self.permissions.get_mut(&Self::account_subject(account)) {
            set.remove(token);
        }
    }

    /// Add a signatory to `account`. Caller must be the account owner or hold `AddSignatory`.
    pub fn add_signatory(
        &mut self,
        caller: &ScopedAccountId,
        account: &ScopedAccountId,
        public_key: String,
    ) -> bool {
        let caller_subject = Self::account_subject(caller);
        let account_subject = Self::account_subject(account);
        if caller_subject != account_subject {
            let token = PermissionToken::AddSignatory(account_subject.clone());
            if !self.has_permission(caller, &token) {
                return false;
            }
        }
        if !self.account_is_linked(account) {
            return false;
        }
        let Some(acc) = self.accounts.get_mut(&account_subject) else {
            return false;
        };
        acc.insert_signatory(public_key)
    }

    /// Remove a signatory from `account`. Caller must be owner or hold `RemoveSignatory`.
    pub fn remove_signatory(
        &mut self,
        caller: &ScopedAccountId,
        account: &ScopedAccountId,
        public_key: &str,
    ) -> bool {
        let caller_subject = Self::account_subject(caller);
        let account_subject = Self::account_subject(account);
        if caller_subject != account_subject {
            let token = PermissionToken::RemoveSignatory(account_subject.clone());
            if !self.has_permission(caller, &token) {
                return false;
            }
        }
        if !self.account_is_linked(account) {
            return false;
        }
        let Some(acc) = self.accounts.get_mut(&account_subject) else {
            return false;
        };
        acc.remove_signatory(public_key)
    }

    /// Update quorum for `account`. Caller must be owner or hold `SetAccountQuorum`.
    pub fn set_account_quorum(
        &mut self,
        caller: &ScopedAccountId,
        account: &ScopedAccountId,
        quorum: u32,
    ) -> bool {
        if quorum == 0 {
            return false;
        }
        let caller_subject = Self::account_subject(caller);
        let account_subject = Self::account_subject(account);
        if caller_subject != account_subject {
            let token = PermissionToken::SetAccountQuorum(account_subject.clone());
            if !self.has_permission(caller, &token) {
                return false;
            }
        }
        if !self.account_is_linked(account) {
            return false;
        }
        let Some(acc) = self.accounts.get_mut(&account_subject) else {
            return false;
        };
        acc.set_quorum(quorum);
        true
    }

    /// Store account detail (metadata) under `key`. Caller must be owner or hold `SetAccountDetail`.
    pub fn set_account_detail(
        &mut self,
        caller: &ScopedAccountId,
        account: &ScopedAccountId,
        key: &str,
        value: Vec<u8>,
    ) -> bool {
        if key.is_empty() {
            return false;
        }
        let caller_subject = Self::account_subject(caller);
        let account_subject = Self::account_subject(account);
        if caller_subject != account_subject {
            let token = PermissionToken::SetAccountDetail(account_subject.clone());
            if !self.has_permission(caller, &token) {
                return false;
            }
        }
        if !self.account_is_linked(account) {
            return false;
        }
        let Some(acc) = self.accounts.get_mut(&account_subject) else {
            return false;
        };
        acc.set_detail(key, value);
        true
    }

    /// Read back account quorum.
    pub fn account_quorum(&self, account: &ScopedAccountId) -> Option<u32> {
        if !self.account_is_linked(account) {
            return None;
        }
        self.accounts
            .get(&Self::account_subject(account))
            .map(|a| a.quorum)
    }

    /// Read back account signatories.
    pub fn account_signatories(&self, account: &ScopedAccountId) -> Option<Vec<String>> {
        if !self.account_is_linked(account) {
            return None;
        }
        self.accounts
            .get(&Self::account_subject(account))
            .map(|a| a.signatories.iter().cloned().collect())
    }

    /// Read back an account detail entry.
    pub fn account_detail_value(&self, account: &ScopedAccountId, key: &str) -> Option<Vec<u8>> {
        if !self.account_is_linked(account) {
            return None;
        }
        self.accounts
            .get(&Self::account_subject(account))
            .and_then(|a| a.detail.get(key).cloned())
    }

    pub fn has_permission(&self, account: &ScopedAccountId, token: &PermissionToken) -> bool {
        let subject = Self::account_subject(account);
        if !self.subject_has_any_domain(&subject) {
            return false;
        }
        // Direct permission
        if self
            .permissions
            .get(&subject)
            .map(|set| set.contains(token))
            .unwrap_or(false)
        {
            return true;
        }
        // Role-derived permissions
        if let Some(role_names) = self.role_assignments.get(&subject) {
            for r in role_names {
                if let Some(perms) = self.roles.get(r)
                    && perms.contains(token)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Initialize with a list of balances.
    pub fn with_balances(entries: &[((ScopedAccountId, AssetDefinitionId), Numeric)]) -> Self {
        let mut wsv = Self::new();
        for ((account, asset), amount) in entries.iter().cloned() {
            assert!(
                Self::is_unsigned_scale0(&amount),
                "mock WSV balances must be unsigned scale=0"
            );
            let subject = Self::account_subject(&account);
            wsv.domains.entry(account.domain().clone()).or_default();
            wsv.domains.entry(asset.domain().clone()).or_default();
            wsv.domain_accounts
                .entry(account.domain().clone())
                .or_default()
                .insert(subject.clone());
            wsv.accounts.entry(subject.clone()).or_default();
            wsv.asset_definitions
                .entry(asset.clone())
                .or_insert_with(|| AssetDefinition::new(Mintable::Infinitely));
            wsv.balances
                .insert((subject, asset.clone()), amount.clone());
            if let Some(def) = wsv.asset_definitions.get_mut(&asset) {
                def.total_supply = def
                    .total_supply
                    .clone()
                    .checked_add(amount)
                    .expect("mock total supply overflow");
            }
        }
        wsv
    }

    /// Readback: check if a peer entry exists.
    pub fn has_peer(&self, peer: &Peer) -> bool {
        self.peers.contains(peer)
    }

    /// Readback: return trigger enabled state if present.
    pub fn trigger_state(&self, name: &str) -> Option<bool> {
        self.triggers.get(name).copied()
    }

    /// Create a role with the given permission set if it doesn't exist.
    pub fn create_role(&mut self, name: &str, perms: HashSet<PermissionToken>) -> bool {
        if self.roles.contains_key(name) {
            false
        } else {
            self.roles.insert(name.to_string(), perms);
            true
        }
    }

    /// Delete a role if it has no assignees.
    pub fn delete_role(&mut self, name: &str) -> bool {
        // Ensure no assignments reference this role
        let assigned = self.role_assignments.values().any(|set| set.contains(name));
        if assigned {
            return false;
        }
        self.roles.remove(name).is_some()
    }

    /// Grant a role to an account if the role exists.
    pub fn grant_role(&mut self, account: &ScopedAccountId, role: &str) -> bool {
        if !self.roles.contains_key(role) {
            return false;
        }
        let subject = Self::account_subject(account);
        if !self.subject_has_any_domain(&subject) {
            return false;
        }
        self.role_assignments
            .entry(subject)
            .or_default()
            .insert(role.to_string())
    }

    /// Revoke a role from an account.
    pub fn revoke_role(&mut self, account: &ScopedAccountId, role: &str) -> bool {
        let subject = Self::account_subject(account);
        if let Some(set) = self.role_assignments.get_mut(&subject) {
            set.remove(role)
        } else {
            false
        }
    }

    /// Register a new domain. Caller must hold `RegisterDomain`.
    pub fn register_domain(&mut self, caller: &ScopedAccountId, id: DomainId) -> bool {
        if !self.has_permission(caller, &PermissionToken::RegisterDomain) {
            return false;
        }
        self.domains.insert(id, ()).is_none()
    }

    /// Unregister a domain if it exists and has no accounts, assets, or NFTs.
    pub fn unregister_domain(&mut self, id: &DomainId) -> bool {
        // deny removal if any account or asset belongs to the domain
        let has_accounts = self
            .domain_accounts
            .get(id)
            .is_some_and(|subjects| !subjects.is_empty());
        let has_assets = self.asset_definitions.keys().any(|ad| ad.domain() == id);
        let has_nfts = self.nfts.keys().any(|nft_id| nft_id.domain() == id);
        if has_accounts || has_assets || has_nfts {
            return false;
        }
        self.domain_accounts.remove(id);
        self.domains.remove(id).is_some()
    }

    /// Register a new account. Returns `true` if it didn't exist before and the domain exists.
    pub fn register_account(&mut self, caller: &ScopedAccountId, id: ScopedAccountId) -> bool {
        if !self.domains.contains_key(id.domain()) {
            return false;
        }
        if !self.has_permission(caller, &PermissionToken::RegisterAccount) {
            return false;
        }
        let subject = Self::account_subject(&id);
        self.link_subject_to_domain(subject, id.domain().clone())
    }

    /// Attempt to unregister an account from the selected domain.
    ///
    /// If the account subject is linked to multiple domains, only the current
    /// domain link is removed. When the final domain link is removed, subject
    /// state remains detached (account metadata/permissions/roles are preserved).
    ///
    /// Unlinking the final domain while non-zero balances or NFT ownership
    /// remain is rejected to avoid leaving owned resources without a canonical
    /// scoped account link in this mock world-state model.
    pub fn unregister_account(&mut self, id: &ScopedAccountId) -> bool {
        let subject = Self::account_subject(id);
        self.unlink_subject_from_domain(&subject, id.domain())
    }

    /// Register a new asset definition with given mintability.
    /// Returns `true` if the definition was added.
    pub fn register_asset_definition(
        &mut self,
        caller: &ScopedAccountId,
        id: AssetDefinitionId,
        mintable: Mintable,
    ) -> bool {
        if !self.domains.contains_key(id.domain()) {
            return false;
        }
        if !self.has_permission(caller, &PermissionToken::RegisterAssetDefinition) {
            return false;
        }
        self.asset_definitions
            .insert(id, AssetDefinition::new(mintable))
            .is_none()
    }

    /// Unregister an asset definition when no non-zero balances exist for it.
    pub fn unregister_asset_definition(&mut self, id: &AssetDefinitionId) -> bool {
        let has_bal = self
            .balances
            .iter()
            .any(|((_, ad), amount)| ad == id && !amount.is_zero());
        if has_bal {
            return false;
        }
        self.asset_definitions.remove(id).is_some()
    }

    /// Get the balance of `account_id` for `asset_id`.
    pub fn balance(&self, account_id: ScopedAccountId, asset_id: AssetDefinitionId) -> Numeric {
        if !self.account_is_linked(&account_id) {
            return Numeric::zero();
        }
        let subject = Self::account_subject(&account_id);
        self.balances
            .get(&(subject, asset_id))
            .cloned()
            .unwrap_or_else(Numeric::zero)
    }

    fn is_unsigned_scale0(amount: &Numeric) -> bool {
        amount.scale() == 0 && !amount.mantissa().is_negative()
    }

    /// Get the balance of `account_id` for `asset_id` if `caller` is allowed to
    /// view it. Returns `None` if the caller lacks permission.
    pub fn balance_checked(
        &self,
        caller: &ScopedAccountId,
        account_id: &ScopedAccountId,
        asset_id: &AssetDefinitionId,
    ) -> Option<Numeric> {
        if Self::account_subject(caller) == Self::account_subject(account_id)
            || self.has_permission(
                caller,
                &PermissionToken::ReadAccountAssets(Self::account_subject(account_id)),
            )
        {
            Some(self.balance(account_id.clone(), asset_id.clone()))
        } else {
            None
        }
    }

    /// Transfer `amount` of `asset_id` from `from` to `to`.
    /// Returns `true` on success or `false` if `from` lacks funds.
    pub fn transfer(
        &mut self,
        caller: &ScopedAccountId,
        from: ScopedAccountId,
        to: ScopedAccountId,
        asset_id: AssetDefinitionId,
        amount: Numeric,
    ) -> bool {
        if !self.account_is_linked(&from) || !self.account_is_linked(&to) {
            return false;
        }
        if !Self::is_unsigned_scale0(&amount) {
            return false;
        }
        if Self::account_subject(caller) != Self::account_subject(&from) {
            let token = PermissionToken::TransferAsset(asset_id.clone());
            if !self.has_permission(caller, &token) {
                return false;
            }
        }
        let from_subject = Self::account_subject(&from);
        let to_subject = Self::account_subject(&to);
        let from_key = (from_subject, asset_id.clone());
        let to_key = (to_subject, asset_id);
        if from_key == to_key {
            let current = self
                .balances
                .get(&from_key)
                .cloned()
                .unwrap_or_else(Numeric::zero);
            let remaining = match current.clone().checked_sub(amount) {
                Some(val) => val,
                None => return false,
            };
            if remaining.mantissa().is_negative() {
                return false;
            }
            return true;
        }
        let from_current = self
            .balances
            .get(&from_key)
            .cloned()
            .unwrap_or_else(Numeric::zero);
        let from_remaining = match from_current.checked_sub(amount.clone()) {
            Some(val) => val,
            None => return false,
        };
        if from_remaining.mantissa().is_negative() {
            return false;
        }
        let to_current = self
            .balances
            .get(&to_key)
            .cloned()
            .unwrap_or_else(Numeric::zero);
        let to_next = match to_current.checked_add(amount) {
            Some(val) => val,
            None => return false,
        };
        if from_remaining.is_zero() {
            self.balances.remove(&from_key);
        } else {
            self.balances.insert(from_key, from_remaining);
        }
        self.balances.insert(to_key, to_next);
        true
    }

    /// Mint `amount` of `asset_id` into `account_id`.
    pub fn mint(
        &mut self,
        caller: &ScopedAccountId,
        account_id: ScopedAccountId,
        asset_id: AssetDefinitionId,
        amount: Numeric,
    ) -> bool {
        if !self.account_is_linked(&account_id) {
            return false;
        }
        if !Self::is_unsigned_scale0(&amount) {
            return false;
        }
        let token = PermissionToken::MintAsset(asset_id.clone());
        if !self.has_permission(caller, &token) {
            return false;
        }
        let Some(def) = self.asset_definitions.get_mut(&asset_id) else {
            return false;
        };
        if def.mintable.consume_one().is_err() {
            return false;
        }
        let balance_key = (Self::account_subject(&account_id), asset_id.clone());
        let current = self
            .balances
            .get(&balance_key)
            .cloned()
            .unwrap_or_else(Numeric::zero);
        let next = match current.checked_add(amount.clone()) {
            Some(val) => val,
            None => return false,
        };
        let total = match def.total_supply.clone().checked_add(amount) {
            Some(val) => val,
            None => return false,
        };
        self.balances.insert(balance_key, next);
        def.total_supply = total;
        true
    }

    /// Burn `amount` of `asset_id` from `account_id`. Returns `true` if the
    /// balance was sufficient and the burn succeeded.
    pub fn burn(
        &mut self,
        caller: &ScopedAccountId,
        account_id: ScopedAccountId,
        asset_id: AssetDefinitionId,
        amount: Numeric,
    ) -> bool {
        if !self.account_is_linked(&account_id) {
            return false;
        }
        if !Self::is_unsigned_scale0(&amount) {
            return false;
        }
        if Self::account_subject(caller) != Self::account_subject(&account_id) {
            let token = PermissionToken::BurnAsset(asset_id.clone());
            if !self.has_permission(caller, &token) {
                return false;
            }
        }
        let Some(def) = self.asset_definitions.get_mut(&asset_id) else {
            return false;
        };
        let balance_key = (Self::account_subject(&account_id), asset_id.clone());
        let current = self
            .balances
            .get(&balance_key)
            .cloned()
            .unwrap_or_else(Numeric::zero);
        let remaining = match current.checked_sub(amount.clone()) {
            Some(val) => val,
            None => return false,
        };
        if remaining.mantissa().is_negative() {
            return false;
        }
        let total = match def.total_supply.clone().checked_sub(amount) {
            Some(val) => val,
            None => return false,
        };
        if total.mantissa().is_negative() {
            return false;
        }
        if remaining.is_zero() {
            self.balances.remove(&balance_key);
        } else {
            self.balances.insert(balance_key, remaining);
        }
        def.total_supply = total;
        true
    }

    /// Create an NFT with `owner` and `issuer` if it does not already exist.
    pub fn create_nft(
        &mut self,
        owner: ScopedAccountId,
        issuer: ScopedAccountId,
        id: NftId,
    ) -> bool {
        if !self.account_is_linked(&owner) || !self.account_is_linked(&issuer) {
            return false;
        }
        let owner_subject = Self::account_subject(&owner);
        let issuer_subject = Self::account_subject(&issuer);
        self.nfts
            .insert(
                id,
                NftRecord {
                    owner: owner_subject,
                    data: Vec::new(),
                    issuer: issuer_subject,
                },
            )
            .is_none()
    }

    /// Transfer an NFT from `from` to `to`. Caller must be the owner or issuer.
    pub fn transfer_nft(
        &mut self,
        caller: &ScopedAccountId,
        from: ScopedAccountId,
        to: ScopedAccountId,
        id: &NftId,
    ) -> bool {
        if !self.account_is_linked(&from) || !self.account_is_linked(&to) {
            return false;
        }
        let caller_subject = Self::account_subject(caller);
        let from_subject = Self::account_subject(&from);
        let to_subject = Self::account_subject(&to);
        let Some(rec) = self.nfts.get_mut(id) else {
            return false;
        };
        if caller_subject != rec.owner && caller_subject != rec.issuer {
            return false;
        }
        if rec.owner != from_subject && caller_subject != rec.owner {
            return false;
        }
        rec.owner = to_subject;
        true
    }

    /// Set data for an NFT. Caller must be owner or issuer.
    pub fn set_nft_data(&mut self, caller: &ScopedAccountId, id: &NftId, json: Vec<u8>) -> bool {
        let caller_subject = Self::account_subject(caller);
        let Some(rec) = self.nfts.get_mut(id) else {
            return false;
        };
        if rec.owner != caller_subject && rec.issuer != caller_subject {
            return false;
        }
        rec.data = json;
        true
    }

    /// Burn (remove) an NFT. Caller must be owner or issuer.
    pub fn burn_nft(&mut self, caller: &ScopedAccountId, id: &NftId) -> bool {
        let caller_subject = Self::account_subject(caller);
        if let Some(rec) = self.nfts.get(id) {
            if rec.owner != caller_subject && rec.issuer != caller_subject {
                return false;
            }
        } else {
            return false;
        }
        self.nfts.remove(id).is_some()
    }

    /// Return the current owner of an NFT if it exists.
    pub fn nft_owner(&self, id: &NftId) -> Option<ScopedAccountId> {
        let subject = self.nfts.get(id).map(|rec| &rec.owner)?;
        self.canonical_account_id_for_subject(subject)
    }
}

#[derive(Clone, Debug)]
struct MockVerifyingKeyRecord {
    commitment: [u8; 32],
}

impl MockWorldStateView {
    fn binding_from_registry(&self, id: &VerifyingKeyId) -> ZkAssetVerifierBinding {
        let commitment = self.verifying_keys.get(id).map(|rec| rec.commitment);
        ZkAssetVerifierBinding {
            id: id.clone(),
            commitment,
        }
    }
}

fn hash_vk_bytes(backend: &str, bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(backend.as_bytes());
    h.update(bytes);
    h.finalize().into()
}

// NOTE: These items are already imported at the top of the module. The
// duplicate import caused E0252 (name defined multiple times). Remove it.
// use crate::{error::VMError, host::IVMHost, ivm::IVM, syscalls};
use core::str;

use iroha_data_model::isi::{InstructionBox as DMInstructionBox, zk as DMZk};

struct EnvelopeInstructionHandler {
    aliases: &'static [&'static str],
    decode: fn(norito::json::Value) -> Result<DMInstructionBox, VMError>,
}

fn decode_dm_instruction<T>(payload: norito::json::Value) -> Result<DMInstructionBox, VMError>
where
    T: iroha_data_model::isi::Instruction + norito::json::JsonDeserializeOwned,
{
    let instruction: T = norito::json::from_value(payload).map_err(|_| VMError::NoritoInvalid)?;
    Ok(iroha_data_model::isi::Instruction::into_instruction_box(
        Box::new(instruction),
    ))
}

const ENVELOPE_INSTRUCTION_HANDLERS: &[EnvelopeInstructionHandler] = &[
    EnvelopeInstructionHandler {
        aliases: &[
            "zk.RegisterZkAsset",
            "zk::RegisterZkAsset",
            "iroha_data_model::isi::zk::RegisterZkAsset",
        ],
        decode: decode_dm_instruction::<DMZk::RegisterZkAsset>,
    },
    EnvelopeInstructionHandler {
        aliases: &[
            "zk.Shield",
            "zk::Shield",
            "iroha_data_model::isi::zk::Shield",
        ],
        decode: decode_dm_instruction::<DMZk::Shield>,
    },
    EnvelopeInstructionHandler {
        aliases: &[
            "zk.ZkTransfer",
            "zk::ZkTransfer",
            "iroha_data_model::isi::zk::ZkTransfer",
        ],
        decode: decode_dm_instruction::<DMZk::ZkTransfer>,
    },
    EnvelopeInstructionHandler {
        aliases: &[
            "zk.Unshield",
            "zk::Unshield",
            "iroha_data_model::isi::zk::Unshield",
        ],
        decode: decode_dm_instruction::<DMZk::Unshield>,
    },
    EnvelopeInstructionHandler {
        aliases: &[
            "zk.CreateElection",
            "zk::CreateElection",
            "iroha_data_model::isi::zk::CreateElection",
        ],
        decode: decode_dm_instruction::<DMZk::CreateElection>,
    },
    EnvelopeInstructionHandler {
        aliases: &[
            "zk.SubmitBallot",
            "zk::SubmitBallot",
            "iroha_data_model::isi::zk::SubmitBallot",
        ],
        decode: decode_dm_instruction::<DMZk::SubmitBallot>,
    },
    EnvelopeInstructionHandler {
        aliases: &[
            "zk.FinalizeElection",
            "zk::FinalizeElection",
            "iroha_data_model::isi::zk::FinalizeElection",
        ],
        decode: decode_dm_instruction::<DMZk::FinalizeElection>,
    },
];

// -----------------------------
// ZK shielded ledger structures
// -----------------------------

/// Shielded asset mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ZkAssetMode {
    /// Only shielded ledger (no public account balances).
    ZkNative,
    /// Hybrid: public balances plus shielded ledger; allows shield/unshield when policy permits.
    #[default]
    Hybrid,
}

// Default is derived (Hybrid)

/// Verifying-key binding enforced for a ZK asset operation.
#[derive(Clone, Debug)]
pub struct ZkAssetVerifierBinding {
    pub id: VerifyingKeyId,
    pub commitment: Option<[u8; 32]>,
}

impl ZkAssetVerifierBinding {
    fn matches(&self, proof: &ProofAttachment) -> bool {
        if proof.backend.as_str() != self.id.backend {
            return false;
        }
        let mut saw_binding = false;
        if let Some(vk_id) = proof.vk_ref.as_ref() {
            if vk_id != &self.id {
                return false;
            }
            saw_binding = true;
        }
        if let Some(vk_inline) = proof.vk_inline.as_ref() {
            if vk_inline.backend != self.id.backend {
                return false;
            }
            if let Some(expected) = self.commitment {
                let backend = proof.backend.to_string();
                let digest = hash_vk_bytes(&backend, &vk_inline.bytes);
                if digest != expected {
                    return false;
                }
            }
            saw_binding = true;
        }
        if let Some(commitment) = proof.vk_commitment
            && let Some(expected) = self.commitment
            && commitment != expected
        {
            return false;
        }
        saw_binding
    }
}

/// Policy and state for a shielded asset.
#[derive(Clone, Debug, Default)]
pub struct ZkAssetState {
    pub mode: ZkAssetMode,
    pub allow_shield: bool,
    pub allow_unshield: bool,
    pub commitments: Vec<[u8; 32]>,
    pub root_history: Vec<HashOf<iroha_crypto::MerkleTree<[u8; 32]>>>,
    pub nullifiers: HashSet<[u8; 32]>,
    pub vk_transfer: Option<ZkAssetVerifierBinding>,
    pub vk_unshield: Option<ZkAssetVerifierBinding>,
    pub vk_shield: Option<ZkAssetVerifierBinding>,
    /// Canonical Merkle tree over 32-byte commitments (SHA-256 inner nodes, prehashed leaves)
    tree: iroha_crypto::MerkleTree<[u8; 32]>,
}

/// Maximum number of shielded Merkle roots retained in tests.
const ROOT_HISTORY_MAX: usize = 1024;

impl ZkAssetState {
    fn push_commitment(&mut self, c: [u8; 32]) -> HashOf<iroha_crypto::MerkleTree<[u8; 32]>> {
        self.commitments.push(c);
        // Domain‑tagged leaf and update root
        let leaf = iroha_crypto::MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment(c);
        self.tree.add(leaf);
        let root = self
            .tree
            .root()
            .unwrap_or_else(|| HashOf::from_untyped_unchecked(CryptoHash::prehashed([0u8; 32])));
        self.root_history.push(root);
        if self.root_history.len() > ROOT_HISTORY_MAX {
            let overflow = self.root_history.len() - ROOT_HISTORY_MAX;
            self.root_history.drain(0..overflow);
        }
        root
    }
}

/// Election state for anonymous voting.
#[derive(Clone, Debug, Default)]
pub struct ElectionState {
    pub options: u32,
    pub eligible_root: [u8; 32],
    pub start_ts: u64,
    pub end_ts: u64,
    pub finalized: bool,
    pub tally: Vec<u64>,
    pub ballot_nullifiers: HashSet<[u8; 32]>,
    pub ciphertexts: Vec<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zk_root_history_is_bounded() {
        let mut state = ZkAssetState::default();
        for i in 0..(ROOT_HISTORY_MAX + 32) {
            let mut commitment = [0u8; 32];
            commitment[..8].copy_from_slice(&(i as u64).to_le_bytes());
            state.push_commitment(commitment);
        }
        assert_eq!(state.root_history.len(), ROOT_HISTORY_MAX);
        assert_eq!(state.commitments.len(), ROOT_HISTORY_MAX + 32);
    }
}

#[cfg(test)]
fn test_account_id(signatory: &str, domain: &str) -> ScopedAccountId {
    ScopedAccountId::new(
        domain.parse().expect("test domain id must parse"),
        signatory
            .parse()
            .expect("test public key literal must parse"),
    )
}

/// ZK event stream for tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZkEvent {
    /// ZK policy was updated for an asset.
    ZkPolicyUpdated {
        asset: AssetDefinitionId,
        mode: ZkAssetMode,
        allow_shield: bool,
        allow_unshield: bool,
    },
    /// New commitment was appended and root updated.
    CommitmentAdded {
        asset: AssetDefinitionId,
        commitment: [u8; 32],
        new_root: [u8; 32],
    },
    /// Unshield operation credited public balance.
    Unshielded {
        asset: AssetDefinitionId,
        to: AccountId,
        public_amount: Numeric,
    },
}

/// Host environment exposing WSV operations via syscalls and enforcing permissions.
pub struct WsvHost {
    pub wsv: MockWorldStateView,
    pub caller: ScopedAccountId,
    account_map: HashMap<u64, ScopedAccountId>,
    asset_map: HashMap<u64, AssetDefinitionId>,
    // ZK verify gating and configuration
    zk_verified_transfer: bool,
    zk_verified_unshield: bool,
    zk_verified_ballot: VecDeque<[u8; 32]>,
    zk_verified_tally: Option<[u8; 32]>,
    zk_cfg: crate::host::ZkHalo2Config,
    axt_state: Option<axt::HostAxtState>,
    axt_policy: Arc<dyn AxtPolicy>,
    axt_policy_overridden: bool,
    sm_enabled: bool,
    fastpq_batch_entries:
        Option<Vec<(ScopedAccountId, ScopedAccountId, AssetDefinitionId, Numeric)>>,
    actual_access: crate::host::AccessLog,
    state_overlay: HashMap<String, Option<Vec<u8>>>,
    tx_active: bool,
    /// Optional pluggable schema registry for typed Norito encode/decode.
    schema: Option<std::sync::Arc<dyn SchemaRegistry + Send + Sync>>,
}

#[derive(Clone)]
struct WsvHostSnapshot {
    wsv: MockWorldStateView,
    state_snapshot: DurableStateSnapshot,
    caller: ScopedAccountId,
    account_map: HashMap<u64, ScopedAccountId>,
    asset_map: HashMap<u64, AssetDefinitionId>,
    zk_verified_transfer: bool,
    zk_verified_unshield: bool,
    zk_verified_ballot: VecDeque<[u8; 32]>,
    zk_verified_tally: Option<[u8; 32]>,
    zk_cfg: crate::host::ZkHalo2Config,
    axt_state: Option<axt::HostAxtState>,
    axt_policy: Arc<dyn AxtPolicy>,
    axt_policy_overridden: bool,
    sm_enabled: bool,
    fastpq_batch_entries:
        Option<Vec<(ScopedAccountId, ScopedAccountId, AssetDefinitionId, Numeric)>>,
    actual_access: crate::host::AccessLog,
    state_overlay: HashMap<String, Option<Vec<u8>>>,
    tx_active: bool,
    schema: Option<std::sync::Arc<dyn SchemaRegistry + Send + Sync>>,
}

impl WsvHost {
    fn default_domain_id() -> DomainId {
        iroha_data_model::account::address::default_domain_name()
            .parse()
            .expect("default domain name must parse as DomainId")
    }

    fn materialize_subject_account(
        wsv: &mut MockWorldStateView,
        subject: &AccountId,
    ) -> ScopedAccountId {
        if let Some(existing) = wsv.canonical_account_id_for_subject(subject) {
            return existing;
        }

        let account_id = subject.to_account_id(Self::default_domain_id());
        wsv.add_account_unchecked(account_id.clone());
        account_id
    }

    fn new_scoped(
        wsv: MockWorldStateView,
        caller: ScopedAccountId,
        account_map: HashMap<u64, ScopedAccountId>,
        asset_map: HashMap<u64, AssetDefinitionId>,
    ) -> Self {
        let policy = Self::build_wsv_axt_policy(&wsv);
        Self {
            wsv,
            caller,
            account_map,
            asset_map,
            zk_verified_transfer: false,
            zk_verified_unshield: false,
            zk_verified_ballot: VecDeque::new(),
            zk_verified_tally: None,
            zk_cfg: crate::host::ZkHalo2Config::default(),
            axt_state: None,
            axt_policy: policy,
            axt_policy_overridden: false,
            sm_enabled: false,
            fastpq_batch_entries: None,
            actual_access: crate::host::AccessLog::default(),
            state_overlay: HashMap::new(),
            tx_active: false,
            schema: None,
        }
    }

    /// Construct a host from a domainless caller/account index map.
    ///
    /// Subjects already linked in the world keep their canonical domain linkage.
    /// Subjects without any domain linkage are materialized under the configured
    /// default domain and linked into the world.
    pub fn new_with_subject_map(
        mut wsv: MockWorldStateView,
        caller: AccountId,
        account_map: HashMap<u64, AccountId>,
        asset_map: HashMap<u64, AssetDefinitionId>,
    ) -> Self {
        let caller_account = Self::materialize_subject_account(&mut wsv, &caller);
        let scoped_map = account_map
            .into_iter()
            .map(|(idx, subject)| (idx, Self::materialize_subject_account(&mut wsv, &subject)))
            .collect();
        Self::new_scoped(wsv, caller_account, scoped_map, asset_map)
    }

    /// Construct a host from a single domainless caller with no account index map.
    pub fn new_with_subject(
        wsv: MockWorldStateView,
        caller: AccountId,
        asset_map: HashMap<u64, AssetDefinitionId>,
    ) -> Self {
        Self::new_with_subject_map(wsv, caller, HashMap::new(), asset_map)
    }

    /// Return the current caller as a domainless account subject.
    #[must_use]
    pub fn caller_subject(&self) -> AccountId {
        AccountId::from(&self.caller)
    }

    /// Switch the caller using a domainless account subject.
    ///
    /// If the subject has no existing domain linkage in the world, it is
    /// materialized under the configured default domain first.
    pub fn set_caller_subject(&mut self, caller: AccountId) {
        self.caller = Self::materialize_subject_account(&mut self.wsv, &caller);
    }

    fn build_wsv_axt_policy(wsv: &MockWorldStateView) -> Arc<SpaceDirectoryAxtPolicy> {
        let slot_length_ms = wsv.slot_length_ms();
        let max_clock_skew_ms = wsv.max_clock_skew_ms();
        let has_explicit_slot = wsv
            .axt_policies
            .values()
            .any(|policy| policy.current_slot != 0);
        let mut policy = SpaceDirectoryAxtPolicy::from_snapshot_with_timing(
            wsv.axt_policy_snapshot(),
            slot_length_ms,
            max_clock_skew_ms,
        );
        if !has_explicit_slot {
            policy = policy.with_current_slot(wsv.current_slot());
        }
        Arc::new(policy)
    }

    fn refresh_axt_policy(&mut self) {
        if !self.axt_policy_overridden {
            self.axt_policy = Self::build_wsv_axt_policy(&self.wsv);
        }
    }

    fn checkpoint_state(&self) -> WsvHostSnapshot {
        WsvHostSnapshot {
            wsv: self.wsv.clone(),
            state_snapshot: self.wsv.sc_snapshot(),
            caller: self.caller.clone(),
            account_map: self.account_map.clone(),
            asset_map: self.asset_map.clone(),
            zk_verified_transfer: self.zk_verified_transfer,
            zk_verified_unshield: self.zk_verified_unshield,
            zk_verified_ballot: self.zk_verified_ballot.clone(),
            zk_verified_tally: self.zk_verified_tally,
            zk_cfg: self.zk_cfg,
            axt_state: self.axt_state.clone(),
            axt_policy: Arc::clone(&self.axt_policy),
            axt_policy_overridden: self.axt_policy_overridden,
            sm_enabled: self.sm_enabled,
            fastpq_batch_entries: self.fastpq_batch_entries.clone(),
            actual_access: self.actual_access.clone(),
            state_overlay: self.state_overlay.clone(),
            tx_active: self.tx_active,
            schema: self.schema.clone(),
        }
    }

    fn restore_state(&mut self, snapshot: &WsvHostSnapshot) {
        self.wsv = snapshot.wsv.clone();
        self.wsv
            .sc_restore(&snapshot.state_snapshot)
            .expect("restore durable state snapshot");
        self.caller = snapshot.caller.clone();
        self.account_map = snapshot.account_map.clone();
        self.asset_map = snapshot.asset_map.clone();
        self.zk_verified_transfer = snapshot.zk_verified_transfer;
        self.zk_verified_unshield = snapshot.zk_verified_unshield;
        self.zk_verified_ballot = snapshot.zk_verified_ballot.clone();
        self.zk_verified_tally = snapshot.zk_verified_tally;
        self.zk_cfg = snapshot.zk_cfg;
        self.axt_state = snapshot.axt_state.clone();
        self.axt_policy = Arc::clone(&snapshot.axt_policy);
        self.axt_policy_overridden = snapshot.axt_policy_overridden;
        self.sm_enabled = snapshot.sm_enabled;
        self.fastpq_batch_entries = snapshot.fastpq_batch_entries.clone();
        self.actual_access = snapshot.actual_access.clone();
        self.state_overlay = snapshot.state_overlay.clone();
        self.tx_active = snapshot.tx_active;
        self.schema = snapshot.schema.clone();
        self.refresh_axt_policy();
    }

    /// Configure Halo2 verification limits for this host.
    pub fn with_zk_halo2_config(mut self, cfg: crate::host::ZkHalo2Config) -> Self {
        self.zk_cfg = cfg;
        self
    }

    /// Override the default allow-all AXT policy (e.g., when wiring UAID manifests in tests).
    pub fn with_axt_policy(mut self, policy: Arc<dyn AxtPolicy>) -> Self {
        self.axt_policy = policy;
        self.axt_policy_overridden = true;
        self
    }

    /// Configure the expected manifest root for a dataspace (Space Directory policy).
    pub fn set_axt_manifest_root(&mut self, dsid: DataSpaceId, root: [u8; 32]) {
        let entry = self.wsv.axt_policies.entry(dsid).or_default();
        entry.manifest_root = root;
        self.refresh_axt_policy();
    }

    /// Configure the expected lane for a dataspace (Space Directory policy).
    pub fn set_axt_target_lane(&mut self, dsid: DataSpaceId, lane: u8) {
        let entry = self.wsv.axt_policies.entry(dsid).or_default();
        entry.target_lane = LaneId::new(u32::from(lane));
        self.refresh_axt_policy();
    }

    /// Configure the current slot used for expiry checks (Space Directory policy).
    pub fn set_axt_current_slot(&mut self, slot: u64) {
        self.wsv
            .set_current_time_ms(slot * self.wsv.slot_length_ms.max(1));
        self.refresh_axt_policy();
    }

    /// Configure the minimum allowed handle era for a dataspace (Space Directory policy).
    pub fn set_axt_min_handle_era(&mut self, dsid: DataSpaceId, era: u64) {
        let entry = self.wsv.axt_policies.entry(dsid).or_default();
        entry.min_handle_era = era;
        self.refresh_axt_policy();
    }

    /// Configure the minimum allowed sub-nonce for a dataspace (Space Directory policy).
    pub fn set_axt_min_sub_nonce(&mut self, dsid: DataSpaceId, sub_nonce: u64) {
        let entry = self.wsv.axt_policies.entry(dsid).or_default();
        entry.min_sub_nonce = sub_nonce;
        self.refresh_axt_policy();
    }

    /// Builder-style helper to set a manifest root expectation.
    pub fn with_axt_manifest_root(mut self, dsid: DataSpaceId, root: [u8; 32]) -> Self {
        self.set_axt_manifest_root(dsid, root);
        self
    }

    /// Builder-style helper to seed AXT policies from a Space Directory snapshot.
    pub fn with_axt_policy_snapshot(mut self, snapshot: AxtPolicySnapshot) -> Self {
        self.wsv.load_axt_policy_snapshot_model(&snapshot);
        self.refresh_axt_policy();
        self
    }

    /// Builder-style helper to set a target lane expectation.
    pub fn with_axt_target_lane(mut self, dsid: DataSpaceId, lane: u8) -> Self {
        self.set_axt_target_lane(dsid, lane);
        self
    }

    /// Builder-style helper to set the current slot for expiry checks.
    pub fn with_axt_current_slot(mut self, slot: u64) -> Self {
        self.set_axt_current_slot(slot);
        self
    }

    /// Builder-style helper to set the minimum handle era.
    pub fn with_axt_min_handle_era(mut self, dsid: DataSpaceId, era: u64) -> Self {
        self.set_axt_min_handle_era(dsid, era);
        self
    }

    /// Builder-style helper to set the minimum sub-nonce.
    pub fn with_axt_min_sub_nonce(mut self, dsid: DataSpaceId, sub_nonce: u64) -> Self {
        self.set_axt_min_sub_nonce(dsid, sub_nonce);
        self
    }

    /// Override the logical wall-clock timestamp and propagate to AXT expiry slot checks.
    pub fn set_current_time_ms(&mut self, ts: u64) {
        self.wsv.set_current_time_ms(ts);
        self.refresh_axt_policy();
    }

    /// Attach a schema registry implementation.
    pub fn with_schema_registry(
        mut self,
        reg: std::sync::Arc<dyn SchemaRegistry + Send + Sync>,
    ) -> Self {
        self.schema = Some(reg);
        self
    }

    fn log_read_key(&mut self, key: &str) {
        self.actual_access.read_keys.insert(key.to_string());
    }

    fn log_write_key(&mut self, key: &str) {
        self.actual_access.write_keys.insert(key.to_string());
        self.actual_access.state_writes.push(StateUpdate {
            key: key.to_string(),
            value: 1,
        });
    }

    /// Enable or disable SM helper syscalls.
    pub fn with_sm_enabled(mut self, enabled: bool) -> Self {
        self.sm_enabled = enabled;
        self
    }

    /// Toggle SM helper support at runtime.
    pub fn set_sm_enabled(&mut self, enabled: bool) {
        self.sm_enabled = enabled;
    }

    fn load_state_value(vm: &mut IVM, stored: &[u8]) -> Result<(), VMError> {
        let mut env = stored.to_vec();
        if let Ok(inner) = pointer_abi::validate_tlv_bytes(&env) {
            if inner.type_id != PointerType::NoritoBytes {
                return Err(VMError::NoritoInvalid);
            }
        } else {
            let mut out = Vec::with_capacity(7 + env.len() + iroha_crypto::Hash::LENGTH);
            out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
            out.push(1);
            out.extend_from_slice(&(env.len() as u32).to_be_bytes());
            out.extend_from_slice(&env);
            let h: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(&env).into();
            out.extend_from_slice(&h);
            env = out;
        }
        let p = vm.alloc_input_tlv(&env)?;
        vm.set_register(10, p);
        Ok(())
    }

    #[cfg(test)]
    pub fn __test_push_verified_ballot(&mut self, hash: [u8; 32]) {
        self.zk_verified_ballot.push_back(hash);
    }

    #[cfg(test)]
    pub fn __test_set_verified_tally(&mut self, hash: [u8; 32]) {
        self.zk_verified_tally = Some(hash);
    }

    #[cfg(test)]
    pub fn __test_verified_tally(&self) -> Option<[u8; 32]> {
        self.zk_verified_tally
    }

    fn account(&self, idx: u64) -> Option<ScopedAccountId> {
        self.account_map.get(&idx).cloned()
    }

    fn asset(&self, idx: u64) -> Option<AssetDefinitionId> {
        self.asset_map.get(&idx).cloned()
    }

    fn decode_account_payload(&self, payload: &[u8]) -> Result<ScopedAccountId, VMError> {
        decode_from_bytes::<ScopedAccountId>(payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_asset_payload(&self, payload: &[u8]) -> Result<AssetDefinitionId, VMError> {
        decode_from_bytes::<AssetDefinitionId>(payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_domain_payload(&self, payload: &[u8]) -> Result<DomainId, VMError> {
        decode_from_bytes::<DomainId>(payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_nft_payload(&self, payload: &[u8]) -> Result<NftId, VMError> {
        decode_from_bytes::<NftId>(payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_instruction_envelope(
        ty: &str,
        payload: norito::json::Value,
    ) -> Result<Option<DMInstructionBox>, VMError> {
        for handler in ENVELOPE_INSTRUCTION_HANDLERS {
            if handler.aliases.contains(&ty) {
                return (handler.decode)(payload).map(Some);
            }
        }
        Ok(None)
    }

    /// Decode a ScopedAccountId from a register which may contain either an index
    /// into `account_map` (older tests) or a pointer to a TLV in INPUT.
    fn decode_account_reg(&self, vm: &IVM, reg: usize) -> Result<ScopedAccountId, VMError> {
        let v = vm.register(reg);
        if crate::dev_env::debug_wsv_enabled() {
            eprintln!("[wsv.decode_account_reg] reg=r{reg} ptr=0x{v:08x}");
        }
        if let Some(id) = self.account(v) {
            return Ok(id);
        }
        // Treat as TLV pointer
        let tlv = vm.memory.validate_tlv(v)?;
        if crate::dev_env::debug_wsv_enabled() {
            eprintln!(
                "[wsv.decode_account_reg] tlv type={:?} len={}",
                tlv.type_id,
                tlv.payload.len()
            );
        }
        if tlv.type_id != PointerType::AccountId {
            return Err(VMError::NoritoInvalid);
        }
        self.decode_account_payload(tlv.payload)
    }

    /// Decode an AssetDefinitionId from a register which may contain either an
    /// index into `asset_map` or a pointer to a TLV in INPUT.
    fn decode_asset_reg(&self, vm: &IVM, reg: usize) -> Result<AssetDefinitionId, VMError> {
        let v = vm.register(reg);
        if let Some(id) = self.asset(v) {
            return Ok(id);
        }
        match vm.memory.validate_tlv(v) {
            Ok(tlv) => {
                if tlv.type_id != PointerType::AssetDefinitionId {
                    return Err(VMError::NoritoInvalid);
                }
                self.decode_asset_payload(tlv.payload)
            }
            Err(_) => self.decode_tlv_from_code(vm, v, PointerType::AssetDefinitionId),
        }
    }

    fn ensure_unsigned_scale0(numeric: Numeric) -> Result<Numeric, VMError> {
        if numeric.scale() != 0 || numeric.mantissa().is_negative() {
            return Err(VMError::AssertionFailed);
        }
        Ok(numeric)
    }

    /// Decode a Numeric from a register that points at NoritoBytes TLV.
    fn decode_numeric_reg(&self, vm: &IVM, reg: usize) -> Result<Numeric, VMError> {
        let v = vm.register(reg);
        match vm.memory.validate_tlv(v) {
            Ok(tlv) => {
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let numeric =
                    decode_from_bytes::<Numeric>(tlv.payload).map_err(|_| VMError::DecodeError)?;
                Self::ensure_unsigned_scale0(numeric)
            }
            Err(_) => {
                let numeric = self.decode_tlv_from_code(vm, v, PointerType::NoritoBytes)?;
                Self::ensure_unsigned_scale0(numeric)
            }
        }
    }

    fn decode_numeric_ptr(&self, vm: &IVM, ptr: u64) -> Result<Numeric, VMError> {
        match vm.memory.validate_tlv(ptr) {
            Ok(tlv) => {
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let numeric =
                    decode_from_bytes::<Numeric>(tlv.payload).map_err(|_| VMError::DecodeError)?;
                Self::ensure_unsigned_scale0(numeric)
            }
            Err(_) => {
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, PointerType::NoritoBytes) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: PointerType::NoritoBytes as u16,
                    });
                }
                let numeric = self.decode_tlv_from_code(vm, ptr, PointerType::NoritoBytes)?;
                Self::ensure_unsigned_scale0(numeric)
            }
        }
    }

    /// Decode NftId from a register that may be an INPUT TLV pointer.
    fn decode_nft_reg(&self, vm: &IVM, reg: usize) -> Result<NftId, VMError> {
        let v = vm.register(reg);
        if crate::dev_env::debug_wsv_enabled() {
            eprintln!("[wsv.decode_nft_reg] reg=r{reg} ptr=0x{v:08x}");
        }
        // No index map for NftId in this mock; require TLV pointer
        let tlv = vm.memory.validate_tlv(v)?;
        if crate::dev_env::debug_wsv_enabled() {
            eprintln!(
                "[wsv.decode_nft_reg] tlv type={:?} len={}",
                tlv.type_id,
                tlv.payload.len()
            );
        }
        if tlv.type_id != PointerType::NftId {
            return Err(VMError::NoritoInvalid);
        }
        self.decode_nft_payload(tlv.payload)
    }

    fn begin_fastpq_batch(&mut self) -> Result<u64, VMError> {
        if self.fastpq_batch_entries.is_some() {
            return Err(VMError::PermissionDenied);
        }
        self.fastpq_batch_entries = Some(Vec::new());
        Ok(0)
    }

    fn push_fastpq_batch_entry(&mut self, vm: &IVM) -> Result<u64, VMError> {
        if self.fastpq_batch_entries.is_none() {
            return Err(VMError::PermissionDenied);
        }
        let from = self.decode_account_reg(vm, 10)?;
        let to = self.decode_account_reg(vm, 11)?;
        let asset = self.decode_asset_reg(vm, 12)?;
        let amount = self.decode_numeric_reg(vm, 13)?;
        self.fastpq_batch_entries
            .as_mut()
            .expect("batch presence checked above")
            .push((from, to, asset, amount));
        Ok(0)
    }

    fn finish_fastpq_batch(&mut self) -> Result<u64, VMError> {
        let Some(entries) = self.fastpq_batch_entries.take() else {
            return Err(VMError::PermissionDenied);
        };
        if entries.is_empty() {
            return Err(VMError::DecodeError);
        }
        for (from, to, asset, amount) in entries {
            if !self.wsv.transfer(
                &self.caller,
                from.clone(),
                to.clone(),
                asset.clone(),
                amount,
            ) {
                return Err(VMError::PermissionDenied);
            }
        }
        Ok(0)
    }

    fn apply_fastpq_batch_tlv(&mut self, vm: &IVM) -> Result<u64, VMError> {
        if self.fastpq_batch_entries.is_some() {
            return Err(VMError::PermissionDenied);
        }
        let ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(ptr)?;
        if tlv.type_id != PointerType::NoritoBytes {
            return Err(VMError::NoritoInvalid);
        }
        let batch: TransferAssetBatch =
            decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
        if batch.entries().is_empty() {
            return Err(VMError::DecodeError);
        }
        for entry in batch.entries() {
            let from = Self::materialize_subject_account(&mut self.wsv, entry.from());
            let to = Self::materialize_subject_account(&mut self.wsv, entry.to());
            if !self.wsv.transfer(
                &self.caller,
                from,
                to,
                entry.asset_definition().clone(),
                entry.amount().clone(),
            ) {
                return Err(VMError::PermissionDenied);
            }
        }
        Ok(0)
    }

    fn axt_expiry_slot_with_skew(&self, expiry_slot: u64) -> u64 {
        axt::expiry_slot_with_skew(
            expiry_slot,
            self.wsv.slot_length_ms(),
            self.wsv.max_clock_skew_ms(),
            None,
        )
    }

    fn validate_axt_proof(&self, dsid: DataSpaceId, proof: &ProofBlob) -> Result<(), VMError> {
        let Some(policy) = self.wsv.axt_policies.get(&dsid) else {
            return Err(VMError::PermissionDenied);
        };
        if policy.manifest_root.iter().all(|byte| *byte == 0) {
            return Err(VMError::PermissionDenied);
        }
        if proof.payload.is_empty() {
            return Err(VMError::NoritoInvalid);
        }
        if proof.expiry_slot == Some(0) {
            return Err(VMError::NoritoInvalid);
        }
        if let Some(expiry_slot) = proof.expiry_slot {
            let expiry_with_skew = self.axt_expiry_slot_with_skew(expiry_slot);
            let current_slot = policy.current_slot;
            if current_slot > 0 && current_slot > expiry_with_skew {
                return Err(VMError::PermissionDenied);
            }
        }
        match decode_from_bytes::<axt::AxtProofEnvelope>(&proof.payload) {
            Ok(envelope) => {
                if envelope.dsid != dsid || envelope.manifest_root != policy.manifest_root {
                    return Err(VMError::PermissionDenied);
                }
            }
            Err(_) => {
                if proof.payload.as_slice() != policy.manifest_root.as_slice() {
                    return Err(VMError::PermissionDenied);
                }
            }
        }
        Ok(())
    }

    fn handle_axt_begin(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        self.refresh_axt_policy();
        let tlv = vm.memory.validate_tlv(vm.register(10))?;
        if tlv.type_id != PointerType::AxtDescriptor {
            return Err(VMError::NoritoInvalid);
        }
        let descriptor: axt::AxtDescriptor =
            norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        axt::validate_descriptor(&descriptor)?;
        let binding = axt::compute_binding(&descriptor).map_err(|_| VMError::NoritoInvalid)?;
        self.axt_state = Some(axt::HostAxtState::new(descriptor, binding));
        Ok(0)
    }

    fn handle_axt_touch(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let state = self.axt_state.as_mut().ok_or(VMError::PermissionDenied)?;
        let ds_tlv = vm.memory.validate_tlv(vm.register(10))?;
        if ds_tlv.type_id != PointerType::DataSpaceId {
            return Err(VMError::NoritoInvalid);
        }
        let dsid: DataSpaceId =
            norito::decode_from_bytes(ds_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        if !state.expected_dsids().contains(&dsid) {
            return Err(VMError::PermissionDenied);
        }
        let manifest_ptr = vm.register(11);
        let manifest = if manifest_ptr == 0 {
            TouchManifest {
                read: Vec::new(),
                write: Vec::new(),
            }
        } else {
            let manifest_tlv = vm.memory.validate_tlv(manifest_ptr)?;
            if manifest_tlv.type_id != PointerType::NoritoBytes {
                return Err(VMError::NoritoInvalid);
            }
            norito::decode_from_bytes(manifest_tlv.payload).map_err(|_| VMError::NoritoInvalid)?
        };
        self.axt_policy.allow_touch(dsid, &manifest)?;
        state.record_touch(dsid, manifest)?;
        Ok(0)
    }

    fn handle_axt_verify_ds_proof(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let state_view = self.axt_state.as_ref().ok_or(VMError::PermissionDenied)?;
        let ds_tlv = vm.memory.validate_tlv(vm.register(10))?;
        if ds_tlv.type_id != PointerType::DataSpaceId {
            return Err(VMError::NoritoInvalid);
        }
        let dsid: DataSpaceId =
            norito::decode_from_bytes(ds_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        if !state_view.expected_dsids().contains(&dsid) {
            return Err(VMError::PermissionDenied);
        }
        let proof_ptr = vm.register(11);
        if proof_ptr == 0 {
            if !self.wsv.axt_policies.contains_key(&dsid) {
                return Err(VMError::PermissionDenied);
            }
            let state = self.axt_state.as_mut().expect("axt_state checked above");
            state.record_proof(dsid, None, None)?;
            return Ok(0);
        }
        let proof_tlv = vm.memory.validate_tlv(proof_ptr)?;
        if proof_tlv.type_id != PointerType::ProofBlob {
            return Err(VMError::NoritoInvalid);
        }
        let proof: ProofBlob =
            norito::decode_from_bytes(proof_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        self.validate_axt_proof(dsid, &proof)?;
        let state = self.axt_state.as_mut().expect("axt_state checked above");
        state.record_proof(dsid, Some(proof), None)?;
        Ok(0)
    }

    fn handle_axt_use_asset_handle(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let handle_tlv = vm.memory.validate_tlv(vm.register(10))?;
        if handle_tlv.type_id != PointerType::AssetHandle {
            return Err(VMError::NoritoInvalid);
        }
        let handle: AssetHandle =
            norito::decode_from_bytes(handle_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        let Some(binding) = handle.binding_array() else {
            return Err(VMError::NoritoInvalid);
        };

        let op_tlv = vm.memory.validate_tlv(vm.register(11))?;
        if op_tlv.type_id != PointerType::NoritoBytes {
            return Err(VMError::NoritoInvalid);
        }
        let intent: RemoteSpendIntent =
            norito::decode_from_bytes(op_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        {
            let state = self.axt_state.as_ref().ok_or(VMError::PermissionDenied)?;
            if binding != state.binding() {
                return Err(VMError::PermissionDenied);
            }
            if !state.expected_dsids().contains(&intent.asset_dsid) {
                return Err(VMError::PermissionDenied);
            }
            if !state.has_touch(&intent.asset_dsid) {
                return Err(VMError::PermissionDenied);
            }
        }

        let proof = match vm.register(12) {
            0 => None,
            ptr => {
                let proof_tlv = vm.memory.validate_tlv(ptr)?;
                if proof_tlv.type_id != PointerType::ProofBlob {
                    return Err(VMError::NoritoInvalid);
                }
                Some(
                    norito::decode_from_bytes(proof_tlv.payload)
                        .map_err(|_| VMError::NoritoInvalid)?,
                )
            }
        };
        if let Some(proof_blob) = proof.as_ref() {
            self.validate_axt_proof(intent.asset_dsid, proof_blob)?;
        }
        let resolved_amount = axt::resolve_handle_amount(&intent, proof.as_ref())
            .map_err(axt::HandleAmountResolutionError::to_vm_error)?;
        if resolved_amount.amount > handle.budget.remaining {
            return Err(VMError::PermissionDenied);
        }
        if let Some(per_use) = handle.budget.per_use
            && resolved_amount.amount > per_use
        {
            return Err(VMError::PermissionDenied);
        }

        let usage = axt::HandleUsage {
            handle,
            intent,
            proof,
            amount: resolved_amount.amount,
            amount_commitment: resolved_amount.amount_commitment,
        };
        self.axt_policy.allow_handle(&usage)?;
        let state = self.axt_state.as_mut().expect("axt_state checked above");
        state.record_handle(usage)?;
        Ok(0)
    }

    fn handle_axt_commit(&mut self) -> Result<u64, VMError> {
        let state = self.axt_state.take().ok_or(VMError::PermissionDenied)?;
        match state.validate_commit() {
            Ok(()) => Ok(0),
            Err(err) => {
                self.axt_state = Some(state);
                Err(err)
            }
        }
    }

    fn handle_submit_ballot(&mut self, instr: &DMZk::SubmitBallot) -> Result<u64, VMError> {
        let Some(expected_hash) = self.zk_verified_ballot.pop_front() else {
            return Err(VMError::PermissionDenied);
        };
        let mut proof = instr.ballot_proof().clone();
        match proof.envelope_hash {
            Some(hash) if hash != expected_hash => {
                return Err(VMError::PermissionDenied);
            }
            None => {
                proof.envelope_hash = Some(expected_hash);
            }
            _ => {}
        }
        let ok = self.wsv.submit_ballot(
            instr.election_id(),
            instr.ciphertext().clone(),
            *instr.nullifier(),
            proof,
        );
        if ok {
            Ok(0)
        } else {
            Err(VMError::PermissionDenied)
        }
    }

    fn handle_finalize_election(&mut self, instr: &DMZk::FinalizeElection) -> Result<u64, VMError> {
        let Some(expected_hash) = self.zk_verified_tally.take() else {
            return Err(VMError::PermissionDenied);
        };
        let mut proof = instr.tally_proof().clone();
        match proof.envelope_hash {
            Some(hash) if hash != expected_hash => {
                return Err(VMError::PermissionDenied);
            }
            None => {
                proof.envelope_hash = Some(expected_hash);
            }
            _ => {}
        }
        let ok = self
            .wsv
            .finalize_election(instr.election_id(), instr.tally().clone(), proof);
        if ok {
            Ok(0)
        } else {
            Err(VMError::PermissionDenied)
        }
    }

    /// Decode a DomainId from a register which must be a pointer to a TLV in INPUT.
    fn decode_domain_reg(&self, vm: &IVM, reg: usize) -> Result<DomainId, VMError> {
        let v = vm.register(reg);
        if crate::dev_env::debug_wsv_enabled() {
            eprintln!("[wsv] decode_domain_reg: r{reg}=0x{v:x}");
        }
        let tlv = vm.memory.validate_tlv(v)?;
        if crate::dev_env::debug_wsv_enabled() {
            eprintln!(
                "[wsv] TLV: type=0x{:04x} len={}",
                tlv.type_id as u16,
                tlv.payload.len()
            );
        }
        if tlv.type_id != PointerType::DomainId {
            return Err(VMError::NoritoInvalid);
        }
        self.decode_domain_payload(tlv.payload)
    }

    fn decode_tlv_from_code<T>(
        &self,
        vm: &IVM,
        ptr: u64,
        expected: PointerType,
    ) -> Result<T, VMError>
    where
        T: for<'de> norito::NoritoDeserialize<'de>,
    {
        let mut hdr = [0u8; 7];
        vm.memory
            .load_bytes(ptr, &mut hdr)
            .map_err(|_| VMError::NoritoInvalid)?;
        let type_id = u16::from_be_bytes([hdr[0], hdr[1]]);
        if type_id != expected as u16 {
            return Err(VMError::NoritoInvalid);
        }
        if hdr[2] != 1 {
            return Err(VMError::NoritoInvalid);
        }
        let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
        let total = 7usize
            .checked_add(len)
            .and_then(|s| s.checked_add(iroha_crypto::Hash::LENGTH))
            .ok_or(VMError::NoritoInvalid)?;
        let code_len = vm.memory.code_len() as usize;
        if ptr as usize + total > code_len {
            return Err(VMError::NoritoInvalid);
        }
        let bytes = vm
            .memory
            .load_region(ptr, total as u64)
            .map_err(|_| VMError::NoritoInvalid)?;
        let payload = &bytes[7..7 + len];
        let expected_hash = &bytes[7 + len..total];
        let computed: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
        if computed.as_ref() != expected_hash {
            return Err(VMError::NoritoInvalid);
        }
        decode_from_bytes(payload).map_err(|_| VMError::DecodeError)
    }

    fn alloc_norito_bytes_tlv(vm: &mut IVM, payload: &[u8]) -> Result<u64, VMError> {
        let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
        out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        out.push(1);
        let len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(payload);
        let h: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
        out.extend_from_slice(&h);
        vm.alloc_input_tlv(&out)
    }

    fn decode_name_payload(&self, payload: &[u8]) -> Result<Name, VMError> {
        decode_from_bytes(payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_name_reg(&self, vm: &IVM, reg: usize) -> Result<Name, VMError> {
        let v = vm.register(reg);
        match vm.memory.validate_tlv(v) {
            Ok(tlv) => {
                if tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                self.decode_name_payload(tlv.payload)
            }
            Err(_) => self.decode_tlv_from_code(vm, v, PointerType::Name),
        }
    }
}

/// Parse a permission token from a compact Name string.
/// Supported formats:
/// - "register_domain"
/// - "register_account"
/// - "register_asset_definition"
/// - "read_assets:<account_id>"
/// - "mint_asset:<asset_def_id>"
/// - "burn_asset:<asset_def_id>"
/// - "transfer_asset:<asset_def_id>"
fn parse_permission_name(s: &str) -> Result<PermissionToken, VMError> {
    if s == "register_domain" {
        return Ok(PermissionToken::RegisterDomain);
    }
    if s == "register_account" {
        return Ok(PermissionToken::RegisterAccount);
    }
    if s == "register_asset_definition" {
        return Ok(PermissionToken::RegisterAssetDefinition);
    }
    if let Some(rest) = s.strip_prefix("read_assets:") {
        let id = parse_account_id_literal(rest)?;
        return Ok(PermissionToken::ReadAccountAssets(id.subject_id()));
    }
    if let Some(rest) = s.strip_prefix("add_signatory:") {
        let id = parse_account_id_literal(rest)?;
        return Ok(PermissionToken::AddSignatory(id.subject_id()));
    }
    if let Some(rest) = s.strip_prefix("remove_signatory:") {
        let id = parse_account_id_literal(rest)?;
        return Ok(PermissionToken::RemoveSignatory(id.subject_id()));
    }
    if let Some(rest) = s.strip_prefix("set_account_quorum:") {
        let id = parse_account_id_literal(rest)?;
        return Ok(PermissionToken::SetAccountQuorum(id.subject_id()));
    }
    if let Some(rest) = s.strip_prefix("set_account_detail:") {
        let id = parse_account_id_literal(rest)?;
        return Ok(PermissionToken::SetAccountDetail(id.subject_id()));
    }
    if let Some(rest) = s.strip_prefix("register_zk_asset:") {
        let id: AssetDefinitionId = rest.parse().map_err(|_| VMError::NoritoInvalid)?;
        return Ok(PermissionToken::RegisterZkAsset(id));
    }
    if let Some(rest) = s.strip_prefix("shield:") {
        let id: AssetDefinitionId = rest.parse().map_err(|_| VMError::NoritoInvalid)?;
        return Ok(PermissionToken::Shield(id));
    }
    if let Some(rest) = s.strip_prefix("unshield:") {
        let id: AssetDefinitionId = rest.parse().map_err(|_| VMError::NoritoInvalid)?;
        return Ok(PermissionToken::Unshield(id));
    }
    if let Some(rest) = s.strip_prefix("mint_asset:") {
        let id: AssetDefinitionId = rest.parse().map_err(|_| VMError::NoritoInvalid)?;
        return Ok(PermissionToken::MintAsset(id));
    }
    if let Some(rest) = s.strip_prefix("burn_asset:") {
        let id: AssetDefinitionId = rest.parse().map_err(|_| VMError::NoritoInvalid)?;
        return Ok(PermissionToken::BurnAsset(id));
    }
    if let Some(rest) = s.strip_prefix("transfer_asset:") {
        let id: AssetDefinitionId = rest.parse().map_err(|_| VMError::NoritoInvalid)?;
        return Ok(PermissionToken::TransferAsset(id));
    }
    if s == "manage_roles" {
        return Ok(PermissionToken::ManageRoles);
    }
    if s == "manage_permissions" {
        return Ok(PermissionToken::ManagePermissions);
    }
    if s == "manage_triggers" {
        return Ok(PermissionToken::ManageTriggers);
    }
    if s == "manage_peers" {
        return Ok(PermissionToken::ManagePeers);
    }
    Err(VMError::NoritoInvalid)
}

/// Parse a JSON payload and return either the raw string value or selected field contents.
fn parse_json_value_any(bytes: &[u8]) -> Result<njson::Value, VMError> {
    if let Ok(raw) = core::str::from_utf8(bytes)
        && let Ok(value) = njson::from_str(raw)
    {
        return Ok(value);
    }
    let json: Json = decode_from_bytes(bytes).map_err(|_| VMError::NoritoInvalid)?;
    njson::from_str(json.get()).map_err(|_| VMError::NoritoInvalid)
}

/// Parse a JSON payload and return either the raw string value or selected field contents.
fn parse_json_string_any(bytes: &[u8], keys: &[&str]) -> Result<String, VMError> {
    let value = parse_json_value_any(bytes)?;
    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }
    for key in keys {
        if let Some(str_val) = value.get(*key).and_then(|v| v.as_str()) {
            return Ok(str_val.to_string());
        }
    }
    Err(VMError::NoritoInvalid)
}

/// Parse a JSON payload and extract a string array from the top-level value or one of the provided keys.
fn parse_json_string_array_any(bytes: &[u8], keys: &[&str]) -> Result<Vec<String>, VMError> {
    let value = parse_json_value_any(bytes)?;

    let array = if let Some(arr) = value.as_array() {
        arr
    } else if let Some(map) = value.as_object() {
        keys.iter()
            .find_map(|key| map.get(*key))
            .and_then(njson::Value::as_array)
            .ok_or(VMError::NoritoInvalid)?
    } else {
        return Err(VMError::NoritoInvalid);
    };

    let mut out = Vec::with_capacity(array.len());
    for item in array {
        let s = item.as_str().ok_or(VMError::NoritoInvalid)?;
        out.push(s.to_string());
    }
    Ok(out)
}

fn parse_account_id_literal(raw: &str) -> Result<ScopedAccountId, VMError> {
    ScopedAccountId::parse_encoded(raw).map_err(|_| VMError::NoritoInvalid)
}

/// Parse a peer identifier from a JSON payload that may be either a raw string or an
/// object containing a `peer` field.
fn parse_peer_any(bytes: &[u8]) -> Result<Peer, VMError> {
    let peer = parse_json_string_any(bytes, &["peer"])?;
    Peer::from_str(&peer).map_err(|_| VMError::NoritoInvalid)
}

fn parse_permission_value(value: &njson::Value) -> Result<PermissionToken, VMError> {
    fn parse_target<T>(map: &njson::Map) -> Result<T, VMError>
    where
        T: core::str::FromStr,
    {
        let target = map
            .get("target")
            .and_then(njson::Value::as_str)
            .ok_or(VMError::NoritoInvalid)?;
        target.parse().map_err(|_| VMError::NoritoInvalid)
    }
    fn parse_account_target(map: &njson::Map) -> Result<ScopedAccountId, VMError> {
        let target = map
            .get("target")
            .and_then(njson::Value::as_str)
            .ok_or(VMError::NoritoInvalid)?;
        parse_account_id_literal(target)
    }

    if let Some(name) = value.as_str() {
        return parse_permission_name(name);
    }

    let map = value.as_object().ok_or(VMError::NoritoInvalid)?;
    let kind = map
        .get("type")
        .and_then(njson::Value::as_str)
        .ok_or(VMError::NoritoInvalid)?;

    match kind {
        "register_domain" => Ok(PermissionToken::RegisterDomain),
        "register_account" => Ok(PermissionToken::RegisterAccount),
        "register_asset_definition" => Ok(PermissionToken::RegisterAssetDefinition),
        "register_zk_asset" => {
            let id: AssetDefinitionId = parse_target(map)?;
            Ok(PermissionToken::RegisterZkAsset(id))
        }
        "read_assets" => {
            let account = parse_account_target(map)?;
            Ok(PermissionToken::ReadAccountAssets(account.subject_id()))
        }
        "add_signatory" => {
            let account = parse_account_target(map)?;
            Ok(PermissionToken::AddSignatory(account.subject_id()))
        }
        "remove_signatory" => {
            let account = parse_account_target(map)?;
            Ok(PermissionToken::RemoveSignatory(account.subject_id()))
        }
        "set_account_quorum" => {
            let account = parse_account_target(map)?;
            Ok(PermissionToken::SetAccountQuorum(account.subject_id()))
        }
        "set_account_detail" => {
            let account = parse_account_target(map)?;
            Ok(PermissionToken::SetAccountDetail(account.subject_id()))
        }
        "shield" => {
            let id: AssetDefinitionId = parse_target(map)?;
            Ok(PermissionToken::Shield(id))
        }
        "unshield" => {
            let id: AssetDefinitionId = parse_target(map)?;
            Ok(PermissionToken::Unshield(id))
        }
        "mint_asset" => {
            let id: AssetDefinitionId = parse_target(map)?;
            Ok(PermissionToken::MintAsset(id))
        }
        "burn_asset" => {
            let id: AssetDefinitionId = parse_target(map)?;
            Ok(PermissionToken::BurnAsset(id))
        }
        "transfer_asset" => {
            let id: AssetDefinitionId = parse_target(map)?;
            Ok(PermissionToken::TransferAsset(id))
        }
        "manage_roles" => Ok(PermissionToken::ManageRoles),
        "manage_permissions" => Ok(PermissionToken::ManagePermissions),
        "manage_triggers" => Ok(PermissionToken::ManageTriggers),
        "manage_peers" => Ok(PermissionToken::ManagePeers),
        _ => Err(VMError::NoritoInvalid),
    }
}

/// Parse permission JSON into a PermissionToken.
///
/// DevEx: accept the string forms used by `iroha_cli` and unit tests, rather
/// than requiring a full typed Norito JSON representation for IDs.
fn parse_permission_json(s: &str) -> Result<PermissionToken, VMError> {
    let value: njson::Value =
        njson::from_slice(s.as_bytes()).map_err(|_| VMError::NoritoInvalid)?;
    parse_permission_value(&value)
}

fn parse_permission_json_any(bytes: &[u8]) -> Result<PermissionToken, VMError> {
    let value = parse_json_value_any(bytes)?;
    parse_permission_value(&value)
}

fn parse_permission_name_any(bytes: &[u8]) -> Result<PermissionToken, VMError> {
    if let Ok(raw) = core::str::from_utf8(bytes) {
        return parse_permission_name(raw);
    }
    let name: Name = decode_from_bytes(bytes).map_err(|_| VMError::NoritoInvalid)?;
    parse_permission_name(name.as_ref())
}

// Keep tests at the end of the file to satisfy clippy::items_after_test_module
// without requiring an allow attribute.
/* tests moved to EOF */

impl IVMHost for WsvHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        if !crate::syscalls::is_syscall_allowed(vm.syscall_policy(), number) {
            return Err(VMError::UnknownSyscall(number));
        }
        match number {
            // Durable smart-contract state syscalls
            crate::syscalls::SYSCALL_STATE_GET => {
                // r10 = &Name path -> return r10 = &NoritoBytes value in INPUT (or 0 if none)
                let name = self.decode_name_reg(vm, 10)?;
                let path = name.as_ref();
                self.log_read_key(path);
                if self.tx_active
                    && let Some(entry) = self.state_overlay.get(path)
                {
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!(
                            "[WsvHost] overlay STATE_GET path='{path}' staged={}",
                            entry.is_some()
                        );
                    }
                    match entry {
                        Some(val) => Self::load_state_value(vm, val)?,
                        None => vm.set_register(10, 0),
                    }
                    return Ok(0);
                }
                if let Some(env) = self.wsv.sc_get(path) {
                    Self::load_state_value(vm, &env)?;
                } else {
                    vm.set_register(10, 0);
                }
                Ok(0)
            }
            crate::syscalls::SYSCALL_STATE_SET => {
                // r10 = &Name path; r11 = &NoritoBytes value
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!(
                        "[WsvHost] STATE_SET regs r10=0x{path:08x} r11=0x{val:08x}",
                        path = vm.register(10),
                        val = vm.register(11)
                    );
                }
                let p_path = vm.memory.validate_tlv(vm.register(10))?;
                let p_val = vm.memory.validate_tlv(vm.register(11))?;
                if p_path.type_id != PointerType::Name || p_val.type_id != PointerType::NoritoBytes
                {
                    return Err(VMError::NoritoInvalid);
                }
                // Enforce pointer-ABI policy for the value type
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, p_val.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: p_val.type_id as u16,
                    });
                }
                let path_name = self.decode_name_payload(p_path.payload)?;
                let path = path_name.as_ref();
                self.log_write_key(path);
                let mut stored =
                    Vec::with_capacity(7 + p_val.payload.len() + iroha_crypto::Hash::LENGTH);
                stored.extend_from_slice(&(p_val.type_id as u16).to_be_bytes());
                stored.push(p_val.version);
                stored.extend_from_slice(&(p_val.payload.len() as u32).to_be_bytes());
                stored.extend_from_slice(p_val.payload);
                let h: [u8; iroha_crypto::Hash::LENGTH] =
                    iroha_crypto::Hash::new(p_val.payload).into();
                stored.extend_from_slice(&h);
                if self.tx_active {
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!(
                            "[WsvHost] overlay STATE_SET path='{path}' bytes={}",
                            stored.len()
                        );
                    }
                    self.state_overlay.insert(path.to_string(), Some(stored));
                } else {
                    self.wsv.sc_set(path, stored)?;
                }
                Ok(0)
            }
            crate::syscalls::SYSCALL_STATE_DEL => {
                // r10 = &Name path
                let p_path = vm.memory.validate_tlv(vm.register(10))?;
                if p_path.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let path = self.decode_name_payload(p_path.payload)?;
                self.log_write_key(path.as_ref());
                if self.tx_active {
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!("[WsvHost] overlay STATE_DEL path='{}'", path.as_ref());
                    }
                    self.state_overlay.insert(path.to_string(), None);
                } else {
                    self.wsv.sc_del(path.as_ref())?;
                }
                Ok(0)
            }
            crate::syscalls::SYSCALL_DECODE_INT => {
                // r10 = &NoritoBytes (Norito-framed i64) -> r10 = parsed i64
                let addr = vm.register(10);
                if addr == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(addr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let val: i64 = decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                vm.set_register(10, val as u64);
                Ok(0)
            }
            crate::syscalls::SYSCALL_BUILD_PATH_MAP_KEY => {
                // r10 = &Name base; r11 = key (int) -> r10 = &Name("<base>/<key>")
                let base_name = self.decode_name_reg(vm, 10)?;
                let key = vm.register(11) as i64;
                let base = base_name.as_ref();
                let mut s = String::with_capacity(base.len() + 1 + 20);
                s.push_str(base);
                s.push('/');
                use core::fmt::Write as _;
                let _ = write!(&mut s, "{key}");
                let path_name = Name::from_str(&s).map_err(|_| VMError::NoritoInvalid)?;
                let body = norito::to_bytes(&path_name).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_ALLOC => {
                let size = vm.register(10);
                let addr = vm.alloc_heap(size)?;
                vm.set_register(10, addr);
                Ok(1)
            }
            crate::syscalls::SYSCALL_ENCODE_INT => {
                // r10 = value (i64) -> r10 = &NoritoBytes (Norito-framed i64)
                let val = vm.register(10) as i64;
                let body = norito::to_bytes(&val).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_FROM_INT => {
                let val = vm.register(10) as i64;
                if val < 0 {
                    return Err(VMError::AssertionFailed);
                }
                let payload =
                    norito::to_bytes(&Numeric::new(val, 0)).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_TO_INT => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let numeric = self.decode_numeric_ptr(vm, ptr)?;
                let value = numeric
                    .try_mantissa_i128()
                    .ok_or(VMError::AssertionFailed)?;
                if value > i64::MAX as i128 {
                    return Err(VMError::AssertionFailed);
                }
                vm.set_register(10, (value as i64) as u64);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_ADD => {
                let lhs = self.decode_numeric_ptr(vm, vm.register(10))?;
                let rhs = self.decode_numeric_ptr(vm, vm.register(11))?;
                let out = lhs.checked_add(rhs).ok_or(VMError::AssertionFailed)?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_SUB => {
                let lhs = self.decode_numeric_ptr(vm, vm.register(10))?;
                let rhs = self.decode_numeric_ptr(vm, vm.register(11))?;
                let out = lhs.checked_sub(rhs).ok_or(VMError::AssertionFailed)?;
                if out.mantissa().is_negative() {
                    return Err(VMError::AssertionFailed);
                }
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_MUL => {
                let lhs = self.decode_numeric_ptr(vm, vm.register(10))?;
                let rhs = self.decode_numeric_ptr(vm, vm.register(11))?;
                let out = lhs
                    .checked_mul(rhs, NumericSpec::unconstrained())
                    .ok_or(VMError::AssertionFailed)?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_DIV => {
                let lhs = self.decode_numeric_ptr(vm, vm.register(10))?;
                let rhs = self.decode_numeric_ptr(vm, vm.register(11))?;
                let out = lhs
                    .checked_div(rhs, NumericSpec::unconstrained())
                    .ok_or(VMError::AssertionFailed)?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_REM => {
                let lhs = self.decode_numeric_ptr(vm, vm.register(10))?;
                let rhs = self.decode_numeric_ptr(vm, vm.register(11))?;
                let out = lhs
                    .checked_rem(rhs, NumericSpec::unconstrained())
                    .ok_or(VMError::AssertionFailed)?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_NEG => {
                let val = self.decode_numeric_ptr(vm, vm.register(10))?;
                if !val.is_zero() {
                    return Err(VMError::AssertionFailed);
                }
                let payload = norito::to_bytes(&val).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_NUMERIC_EQ
            | crate::syscalls::SYSCALL_NUMERIC_NE
            | crate::syscalls::SYSCALL_NUMERIC_LT
            | crate::syscalls::SYSCALL_NUMERIC_LE
            | crate::syscalls::SYSCALL_NUMERIC_GT
            | crate::syscalls::SYSCALL_NUMERIC_GE => {
                let lhs = self.decode_numeric_ptr(vm, vm.register(10))?;
                let rhs = self.decode_numeric_ptr(vm, vm.register(11))?;
                let cmp = lhs.cmp(&rhs);
                let result = match number {
                    crate::syscalls::SYSCALL_NUMERIC_EQ => cmp == core::cmp::Ordering::Equal,
                    crate::syscalls::SYSCALL_NUMERIC_NE => cmp != core::cmp::Ordering::Equal,
                    crate::syscalls::SYSCALL_NUMERIC_LT => cmp == core::cmp::Ordering::Less,
                    crate::syscalls::SYSCALL_NUMERIC_LE => {
                        cmp == core::cmp::Ordering::Less || cmp == core::cmp::Ordering::Equal
                    }
                    crate::syscalls::SYSCALL_NUMERIC_GT => cmp == core::cmp::Ordering::Greater,
                    crate::syscalls::SYSCALL_NUMERIC_GE => {
                        cmp == core::cmp::Ordering::Greater || cmp == core::cmp::Ordering::Equal
                    }
                    _ => false,
                };
                vm.set_register(10, if result { 1 } else { 0 });
                Ok(0)
            }
            crate::syscalls::SYSCALL_JSON_ENCODE => {
                let tlv = vm.memory.validate_tlv(vm.register(10))?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let json: iroha_primitives::json::Json =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_JSON_DECODE => {
                let addr = vm.register(10);
                if addr == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(addr)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let json: iroha_primitives::json::Json = match tlv.type_id {
                    PointerType::NoritoBytes | PointerType::Json => {
                        decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?
                    }
                    // Devex: accept raw JSON bytes too (validated and canonicalised).
                    PointerType::Blob => {
                        let raw =
                            core::str::from_utf8(tlv.payload).map_err(|_| VMError::DecodeError)?;
                        let value = njson::parse_value(raw).map_err(|_| VMError::DecodeError)?;
                        iroha_primitives::json::Json::from(value)
                    }
                    _ => return Err(VMError::NoritoInvalid),
                };
                let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_TLV_LEN => {
                let addr = vm.register(10);
                if addr == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(addr)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                vm.set_register(10, tlv.payload.len() as u64);
                Ok(0)
            }
            crate::syscalls::SYSCALL_JSON_GET_I64
            | crate::syscalls::SYSCALL_JSON_GET_JSON
            | crate::syscalls::SYSCALL_JSON_GET_NAME
            | crate::syscalls::SYSCALL_JSON_GET_ACCOUNT_ID
            | crate::syscalls::SYSCALL_JSON_GET_NFT_ID
            | crate::syscalls::SYSCALL_JSON_GET_BLOB_HEX => {
                let json_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let key_tlv = vm.memory.validate_tlv(vm.register(11))?;
                if json_tlv.type_id != PointerType::Json || key_tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, json_tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: json_tlv.type_id as u16,
                    });
                }
                if !pointer_abi::is_type_allowed_for_policy(policy, key_tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: key_tlv.type_id as u16,
                    });
                }

                let json: Json =
                    decode_from_bytes(json_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let value: njson::Value = json
                    .try_into_any_norito()
                    .map_err(|_| VMError::DecodeError)?;
                let obj = value.as_object().ok_or(VMError::DecodeError)?;
                let key_name: Name =
                    decode_from_bytes(key_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let field = obj.get(key_name.as_ref()).ok_or(VMError::DecodeError)?;

                match number {
                    crate::syscalls::SYSCALL_JSON_GET_I64 => {
                        let n = match field {
                            njson::Value::Number(njson::native::Number::I64(v)) => *v,
                            njson::Value::Number(njson::native::Number::U64(v)) => {
                                i64::try_from(*v).map_err(|_| VMError::DecodeError)?
                            }
                            _ => return Err(VMError::DecodeError),
                        };
                        vm.set_register(10, n as u64);
                        Ok(0)
                    }
                    crate::syscalls::SYSCALL_JSON_GET_JSON => {
                        let out_json =
                            Json::from_norito_value_ref(field).map_err(|_| VMError::DecodeError)?;
                        let body =
                            norito::to_bytes(&out_json).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = CryptoHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    crate::syscalls::SYSCALL_JSON_GET_NAME => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let nm = Name::from_str(raw).map_err(|_| VMError::DecodeError)?;
                        let body = norito::to_bytes(&nm).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = CryptoHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    crate::syscalls::SYSCALL_JSON_GET_ACCOUNT_ID => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let acct = ScopedAccountId::parse_encoded(raw)
                            .map_err(|_| VMError::DecodeError)?;
                        let body = norito::to_bytes(&acct).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = CryptoHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    crate::syscalls::SYSCALL_JSON_GET_NFT_ID => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let nft = NftId::from_str(raw).map_err(|_| VMError::DecodeError)?;
                        let body = norito::to_bytes(&nft).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::NftId as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = CryptoHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    crate::syscalls::SYSCALL_JSON_GET_BLOB_HEX => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let raw = raw.strip_prefix("0x").unwrap_or(raw);
                        if raw.len() % 2 != 0 {
                            return Err(VMError::DecodeError);
                        }
                        let bytes = hex::decode(raw).map_err(|_| VMError::DecodeError)?;
                        let mut out = Vec::with_capacity(7 + bytes.len() + 32);
                        out.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                        out.extend_from_slice(&bytes);
                        let h: [u8; 32] = CryptoHash::new(&bytes).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    _ => Err(VMError::UnknownSyscall(number)),
                }
            }

            crate::syscalls::SYSCALL_NAME_DECODE => {
                // r10 = &NoritoBytes (prefer Norito Name; legacy UTF-8 string is also accepted)
                // -> r10 = &Name
                let addr = vm.register(10);
                if addr == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(addr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let nm: iroha_data_model::name::Name =
                    if let Ok(name) = decode_from_bytes(tlv.payload) {
                        name
                    } else if let Ok(raw) = core::str::from_utf8(tlv.payload) {
                        Name::from_str(raw).map_err(|_| VMError::DecodeError)?
                    } else if let Ok(raw) = decode_from_bytes::<String>(tlv.payload) {
                        Name::from_str(&raw).map_err(|_| VMError::DecodeError)?
                    } else {
                        return Err(VMError::DecodeError);
                    };
                let body = norito::to_bytes(&nm).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_POINTER_TO_NORITO => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = vm.memory.validate_tlv(ptr)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let mut body = Vec::with_capacity(2 + 1 + 4 + tlv.payload.len() + 32);
                body.extend_from_slice(&(tlv.type_id_raw().to_be_bytes()));
                body.push(tlv.version);
                body.extend_from_slice(&(tlv.payload.len() as u32).to_be_bytes());
                body.extend_from_slice(tlv.payload);
                let inner_hash: [u8; 32] = iroha_crypto::Hash::new(tlv.payload).into();
                body.extend_from_slice(&inner_hash);
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let outer_hash: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&outer_hash);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_POINTER_FROM_NORITO => {
                let addr = vm.register(10);
                if addr == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(addr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                let expected = vm.register(11) as u16;
                let inner = pointer_abi::validate_tlv_bytes(tlv.payload)
                    .map_err(|_| VMError::NoritoInvalid)?;
                if expected != 0 && expected != inner.type_id as u16 {
                    return Err(VMError::NoritoInvalid);
                }
                if !pointer_abi::is_type_allowed_for_policy(policy, inner.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: inner.type_id as u16,
                    });
                }
                let mut out = Vec::with_capacity(7 + inner.payload.len() + 32);
                out.extend_from_slice(&(inner.type_id as u16).to_be_bytes());
                out.push(inner.version);
                out.extend_from_slice(&(inner.payload.len() as u32).to_be_bytes());
                out.extend_from_slice(inner.payload);
                let h: [u8; 32] = iroha_crypto::Hash::new(inner.payload).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_TLV_EQ => {
                let ptr1 = vm.register(10);
                let ptr2 = vm.register(11);
                if ptr1 == ptr2 {
                    vm.set_register(10, 1);
                    return Ok(0);
                }
                if ptr1 == 0 || ptr2 == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                // Validate both TLVs
                let tlv1 = match vm.memory.validate_tlv(ptr1) {
                    Ok(t) => t,
                    Err(_) => {
                        vm.set_register(10, 0);
                        return Ok(0);
                    }
                };
                let tlv2 = match vm.memory.validate_tlv(ptr2) {
                    Ok(t) => t,
                    Err(_) => {
                        vm.set_register(10, 0);
                        return Ok(0);
                    }
                };
                // Check headers and payload
                let eq = tlv1.type_id == tlv2.type_id
                    && tlv1.version == tlv2.version
                    && tlv1.payload == tlv2.payload;
                vm.set_register(10, if eq { 1 } else { 0 });
                Ok(0)
            }
            crate::syscalls::SYSCALL_SCHEMA_ENCODE => {
                let s_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let v_tlv = vm.memory.validate_tlv(vm.register(11))?;
                if s_tlv.type_id != PointerType::Name || v_tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let schema = self.decode_name_payload(s_tlv.payload)?.to_string();
                let json: Json =
                    decode_from_bytes(v_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let json_bytes = json.get().as_bytes();
                if let Some(reg) = &self.schema
                    && let Some(bytes) = reg.encode_json(&schema, json_bytes)
                {
                    let mut out = Vec::with_capacity(7 + bytes.len() + 32);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                    out.extend_from_slice(&bytes);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&bytes).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, p);
                    return Ok(0);
                }
                match schema.as_str() {
                    "Order" => {
                        #[derive(norito::Decode, norito::Encode, Clone, Debug)]
                        struct Order {
                            qty: i64,
                            side: String,
                        }
                        let val: njson::Value =
                            njson::from_slice(json_bytes).map_err(|_| VMError::NoritoInvalid)?;
                        let qty = val
                            .get("qty")
                            .and_then(|v| v.as_i64())
                            .ok_or(VMError::NoritoInvalid)?;
                        let side = val
                            .get("side")
                            .and_then(|v| v.as_str())
                            .ok_or(VMError::NoritoInvalid)?
                            .to_string();
                        let order = Order { qty, side };
                        let bytes = norito::to_bytes(&order).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + bytes.len() + 32);
                        out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                        out.extend_from_slice(&bytes);
                        let h: [u8; 32] = iroha_crypto::Hash::new(&bytes).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    "TradeV1" => {
                        #[derive(norito::Decode, norito::Encode, Clone, Debug)]
                        struct TradeV1 {
                            qty: i64,
                            price: i64,
                            side: String,
                        }
                        let val: njson::Value =
                            njson::from_slice(json_bytes).map_err(|_| VMError::NoritoInvalid)?;
                        let qty = val
                            .get("qty")
                            .and_then(|v| v.as_i64())
                            .ok_or(VMError::NoritoInvalid)?;
                        let price = val
                            .get("price")
                            .and_then(|v| v.as_i64())
                            .ok_or(VMError::NoritoInvalid)?;
                        let side = val
                            .get("side")
                            .and_then(|v| v.as_str())
                            .ok_or(VMError::NoritoInvalid)?
                            .to_string();
                        let trade = TradeV1 { qty, price, side };
                        let bytes = norito::to_bytes(&trade).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + bytes.len() + 32);
                        out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                        out.extend_from_slice(&bytes);
                        let h: [u8; 32] = iroha_crypto::Hash::new(&bytes).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    _ => {
                        let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                }
            }
            crate::syscalls::SYSCALL_SCHEMA_DECODE => {
                let s_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let b_tlv = vm.memory.validate_tlv(vm.register(11))?;
                if s_tlv.type_id != PointerType::Name || b_tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, s_tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: s_tlv.type_id as u16,
                    });
                }
                if !pointer_abi::is_type_allowed_for_policy(policy, b_tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: b_tlv.type_id as u16,
                    });
                }
                let schema = self.decode_name_payload(s_tlv.payload)?.to_string();
                if let Some(reg) = &self.schema
                    && let Some(min) = reg.decode_to_json(&schema, b_tlv.payload)
                {
                    let json_str =
                        core::str::from_utf8(&min).map_err(|_| VMError::NoritoInvalid)?;
                    let json =
                        Json::from_str_norito(json_str).map_err(|_| VMError::NoritoInvalid)?;
                    let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, p);
                    return Ok(0);
                }
                match schema.as_str() {
                    "Order" => {
                        #[derive(norito::Decode, norito::Encode, Clone, Debug)]
                        struct Order {
                            qty: i64,
                            side: String,
                        }
                        let order: Order = norito::decode_from_bytes(b_tlv.payload)
                            .map_err(|_| VMError::NoritoInvalid)?;
                        let val = {
                            let mut map = njson::Map::new();
                            map.insert("qty".to_owned(), njson::Value::from(order.qty));
                            map.insert("side".to_owned(), njson::Value::from(order.side));
                            njson::Value::Object(map)
                        };
                        let json = Json::from(&val);
                        let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    "TradeV1" => {
                        #[derive(norito::Decode, norito::Encode, Clone, Debug)]
                        struct TradeV1 {
                            qty: i64,
                            price: i64,
                            side: String,
                        }
                        let trade: TradeV1 = norito::decode_from_bytes(b_tlv.payload)
                            .map_err(|_| VMError::NoritoInvalid)?;
                        let val = {
                            let mut map = njson::Map::new();
                            map.insert("qty".to_owned(), njson::Value::from(trade.qty));
                            map.insert("price".to_owned(), njson::Value::from(trade.price));
                            map.insert("side".to_owned(), njson::Value::from(trade.side));
                            njson::Value::Object(map)
                        };
                        let json = Json::from(&val);
                        let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    _ => {
                        let json: Json = norito::decode_from_bytes(b_tlv.payload)
                            .map_err(|_| VMError::NoritoInvalid)?;
                        let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                }
            }
            crate::syscalls::SYSCALL_SCHEMA_INFO => {
                let tlv = vm.memory.validate_tlv(vm.register(10))?;
                if tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let name = self.decode_name_payload(tlv.payload)?;
                let name_str = name.as_ref();
                let (id_hex, version) = match name_str {
                    "Order" => {
                        let id: [u8; 32] = iroha_crypto::Hash::new(b"Order@1").into();
                        (hex::encode(id), 1u64)
                    }
                    "TradeV1" => {
                        let id: [u8; 32] = iroha_crypto::Hash::new(b"TradeV1@1").into();
                        (hex::encode(id), 1u64)
                    }
                    _ => (String::new(), 0),
                };
                if version == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let body_value = {
                    let mut map = njson::Map::new();
                    map.insert("id".to_owned(), njson::Value::from(id_hex));
                    map.insert("version".to_owned(), njson::Value::from(version));
                    njson::Value::Object(map)
                };
                let json = Json::from(&body_value);
                let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO => {
                let base_tlv = vm.memory.validate_tlv(vm.register(10))?;
                if base_tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let key_tlv = vm.memory.validate_tlv(vm.register(11))?;
                if key_tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let base_name = self.decode_name_payload(base_tlv.payload)?;
                let base = base_name.as_ref();
                let h: [u8; 32] = iroha_crypto::Hash::new(key_tlv.payload).into();
                let mut s = String::with_capacity(base.len() + 1 + 64);
                use core::fmt::Write as _;
                let _ = write!(&mut s, "{base}/");
                for b in &h {
                    let _ = write!(&mut s, "{b:02x}");
                }
                let path_name = Name::from_str(&s).map_err(|_| VMError::NoritoInvalid)?;
                let body = norito::to_bytes(&path_name).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let hh: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&hh);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            crate::syscalls::SYSCALL_SHA256_HASH | crate::syscalls::SYSCALL_SHA3_HASH => {
                let mut default = crate::host::DefaultHost::new();
                default.syscall(number, vm)
            }
            crate::syscalls::SYSCALL_SM3_HASH
            | crate::syscalls::SYSCALL_SM2_VERIFY
            | crate::syscalls::SYSCALL_SM4_GCM_SEAL
            | crate::syscalls::SYSCALL_SM4_GCM_OPEN
            | crate::syscalls::SYSCALL_SM4_CCM_SEAL
            | crate::syscalls::SYSCALL_SM4_CCM_OPEN => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let mut default = crate::host::DefaultHost::new().with_sm_enabled(true);
                default.syscall(number, vm)
            }
            // Developer helper used by Kotodama-compiled programs to mirror a TLV
            // into the INPUT region (no-op if already INPUT) with envelope/policy
            // validation so pointer‑ABI handlers can rely on INPUT-resident TLVs.
            crate::syscalls::SYSCALL_INPUT_PUBLISH_TLV => {
                let src = vm.register(10);
                if crate::dev_env::debug_wsv_enabled() {
                    eprintln!("[wsv] INPUT_PUBLISH_TLV src=0x{src:x}");
                }
                if src == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let input_lo = crate::memory::Memory::INPUT_START;
                let input_hi =
                    crate::memory::Memory::INPUT_START + crate::memory::Memory::INPUT_SIZE;
                if src >= input_lo && src < input_hi {
                    let tlv = vm.memory.validate_tlv(src)?;
                    if tlv.payload.len() > self.zk_cfg.max_envelope_bytes {
                        return Err(VMError::PermissionDenied);
                    }
                    let policy = vm.syscall_policy();
                    if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                        return Err(VMError::AbiTypeNotAllowed {
                            abi: vm.abi_version(),
                            type_id: tlv.type_id as u16,
                        });
                    }
                    return Ok(0);
                }
                // Read TLV header to determine total envelope length: 2(type) + 1(ver) + 4(len) + payload + 32(hash)
                let hdr = vm
                    .memory
                    .load_region(src, 7)
                    .map_err(|_| VMError::NoritoInvalid)?;
                let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
                let total = 7usize
                    .checked_add(len)
                    .and_then(|v| v.checked_add(32))
                    .ok_or(VMError::NoritoInvalid)?;
                if total > self.zk_cfg.max_envelope_bytes {
                    return Err(VMError::PermissionDenied);
                }
                let bytes_vec = vm
                    .memory
                    .load_region(src, total as u64)
                    .map_err(|_| VMError::NoritoInvalid)?
                    .to_vec();
                let tlv = pointer_abi::validate_tlv_bytes(&bytes_vec)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let dst = vm.alloc_input_tlv(&bytes_vec)?;
                vm.set_register(10, dst);
                Ok(0)
            }
            // Development JSON envelope: execute read-only queries and return a pointer-ABI TLV
            // with the response JSON in the INPUT region.
            crate::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY => {
                // r10 = &Json envelope:
                // {"type":"wsv.get_balance"|"wsv.list_triggers"|"wsv.has_permission"|
                //          "wsv.list_domains_for_subject"|"wsv.list_accounts_for_domain",
                //  "payload": {...}}
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let json: Json =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                let v: norito::json::Value =
                    norito::json::from_str(json.get()).map_err(|_| VMError::NoritoInvalid)?;
                let ty = v
                    .get("type")
                    .and_then(|v| v.as_str())
                    .ok_or(VMError::NoritoInvalid)?;
                let payload = v.get("payload").cloned().ok_or(VMError::NoritoInvalid)?;

                // Helper to produce a JSON TLV in INPUT and return pointer in x10.
                let mut return_json = |val: norito::json::Value| -> Result<u64, VMError> {
                    let json = Json::from(&val);
                    let body = norito::to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                    out.extend_from_slice(&h);
                    vm.alloc_input_tlv(&out)
                };

                // Dispatch simple queries
                match ty {
                    // Get account balance (permission-checked): {account_id, asset_id} -> {balance}
                    "wsv.get_balance" => {
                        let acc = parse_account_id_literal(
                            payload
                                .get("account_id")
                                .and_then(|v| v.as_str())
                                .ok_or(VMError::NoritoInvalid)?,
                        )?;
                        let asset = payload
                            .get("asset_id")
                            .and_then(|v| v.as_str())
                            .ok_or(VMError::NoritoInvalid)?
                            .parse()
                            .map_err(|_| VMError::NoritoInvalid)?;
                        if let Some(bal) = self.wsv.balance_checked(&self.caller, &acc, &asset) {
                            let mut map = njson::Map::new();
                            map.insert("balance".to_owned(), njson::Value::String(bal.to_string()));
                            let p = return_json(njson::Value::Object(map))?;
                            vm.set_register(10, p);
                            Ok(0)
                        } else {
                            Err(VMError::PermissionDenied)
                        }
                    }
                    // List triggers: -> {triggers: [{name, enabled}, ...]}
                    "wsv.list_triggers" => {
                        let mut items = Vec::new();
                        for (name, en) in self.wsv.triggers.iter() {
                            let mut map = njson::Map::new();
                            map.insert("name".to_owned(), njson::Value::from(name.clone()));
                            map.insert("enabled".to_owned(), njson::Value::from(*en));
                            items.push(njson::Value::Object(map));
                        }
                        let mut map = njson::Map::new();
                        map.insert("triggers".to_owned(), njson::Value::Array(items));
                        let p = return_json(njson::Value::Object(map))?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    // Has permission: {account_id, permission} -> {ok: bool}
                    "wsv.has_permission" => {
                        let acc = parse_account_id_literal(
                            payload
                                .get("account_id")
                                .and_then(|v| v.as_str())
                                .ok_or(VMError::NoritoInvalid)?,
                        )?;
                        // Permission can be a string or a JSON object with {type,target}
                        let ok = if let Some(s) = payload.get("permission").and_then(|v| v.as_str())
                        {
                            if let Ok(tok) = parse_permission_name(s) {
                                self.wsv.has_permission(&acc, &tok)
                            } else {
                                false
                            }
                        } else if let Some(obj) =
                            payload.get("permission").and_then(|v| v.as_object())
                        {
                            let s =
                                norito::json::to_json(&norito::json::Value::Object(obj.clone()))
                                    .map_err(|_| VMError::NoritoInvalid)?;
                            if let Ok(tok) = parse_permission_json(&s) {
                                self.wsv.has_permission(&acc, &tok)
                            } else {
                                false
                            }
                        } else {
                            return Err(VMError::NoritoInvalid);
                        };
                        let mut map = njson::Map::new();
                        map.insert("ok".to_owned(), njson::Value::from(ok));
                        let p = return_json(njson::Value::Object(map))?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    // List domains linked to a subject identified by account literal:
                    // {account_id} -> {domains:[...]}
                    "wsv.list_domains_for_subject" => {
                        let acc = parse_account_id_literal(
                            payload
                                .get("account_id")
                                .and_then(|v| v.as_str())
                                .ok_or(VMError::NoritoInvalid)?,
                        )?;
                        let subject = MockWorldStateView::account_subject(&acc);
                        let domains = self.wsv.linked_domains_for_subject(&subject);
                        let mut map = njson::Map::new();
                        map.insert(
                            "domains".to_owned(),
                            njson::Value::Array(
                                domains
                                    .into_iter()
                                    .map(|domain| njson::Value::from(domain.to_string()))
                                    .collect(),
                            ),
                        );
                        let p = return_json(njson::Value::Object(map))?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    // List account literals for all subjects linked to a domain:
                    // {domain_id} -> {domain_id, account_ids:[...]}
                    "wsv.list_accounts_for_domain" => {
                        let domain: DomainId = payload
                            .get("domain_id")
                            .and_then(|v| v.as_str())
                            .ok_or(VMError::NoritoInvalid)?
                            .parse()
                            .map_err(|_| VMError::NoritoInvalid)?;
                        let account_ids = self
                            .wsv
                            .linked_subjects_for_domain(&domain)
                            .into_iter()
                            .map(|subject| {
                                njson::Value::from(
                                    subject.to_account_id(domain.clone()).to_string(),
                                )
                            })
                            .collect::<Vec<_>>();
                        let mut map = njson::Map::new();
                        map.insert(
                            "domain_id".to_owned(),
                            njson::Value::from(domain.to_string()),
                        );
                        map.insert("account_ids".to_owned(), njson::Value::Array(account_ids));
                        let p = return_json(njson::Value::Object(map))?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    _ => Err(VMError::NoritoInvalid),
                }
            }
            // Link ZK_VERIFY syscalls: decode Norito envelope and set per-op verified flags.
            syscalls::SYSCALL_ZK_VERIFY_TRANSFER
            | syscalls::SYSCALL_ZK_VERIFY_UNSHIELD
            | syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT
            | syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => {
                // Expect NoritoBytes TLV in r10 for a `iroha_data_model::zk::OpenVerifyEnvelope`.
                //
                // Note: `WsvHost` is a development/mock host. It does not perform full
                // cryptographic proof verification. The production node host (CoreHost)
                // verifies the proof end-to-end.
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                if tlv.payload.len() > self.zk_cfg.max_envelope_bytes {
                    vm.set_register(10, 0);
                    vm.set_register(11, crate::host::ERR_ENVELOPE_SIZE);
                    return Ok(0);
                }
                let env_hash: [u8; 32] = CryptoHash::new(tlv.payload).into();
                if !self.zk_cfg.enabled {
                    vm.set_register(10, 0);
                    vm.set_register(11, crate::host::ERR_DISABLED);
                    return Ok(0);
                }
                if self.zk_cfg.backend != crate::host::ZkHalo2Backend::Ipa {
                    vm.set_register(10, 0);
                    vm.set_register(11, crate::host::ERR_BACKEND);
                    return Ok(0);
                }
                let env: iroha_data_model::zk::OpenVerifyEnvelope =
                    match norito::decode_from_bytes(tlv.payload) {
                        Ok(env) => env,
                        Err(_) => {
                            vm.set_register(10, 0);
                            vm.set_register(11, crate::host::ERR_DECODE);
                            return Ok(0);
                        }
                    };
                if env.backend != iroha_data_model::zk::BackendTag::Halo2IpaPasta {
                    vm.set_register(10, 0);
                    vm.set_register(11, crate::host::ERR_BACKEND);
                    return Ok(0);
                }
                if env.proof_bytes.len() > self.zk_cfg.max_proof_bytes {
                    vm.set_register(10, 0);
                    vm.set_register(11, crate::host::ERR_PROOF_LEN);
                    return Ok(0);
                }

                // Mock host treats the envelope as verified if it passes basic gating.
                vm.set_register(10, 1);
                vm.set_register(11, 0);
                match number {
                    syscalls::SYSCALL_ZK_VERIFY_TRANSFER => self.zk_verified_transfer = true,
                    syscalls::SYSCALL_ZK_VERIFY_UNSHIELD => self.zk_verified_unshield = true,
                    syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT => {
                        self.zk_verified_ballot.push_back(env_hash);
                    }
                    syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => {
                        self.zk_verified_tally = Some(env_hash);
                    }
                    _ => {}
                }
                Ok(0)
            }
            syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION => {
                // r10 = &NoritoBytes or &Json (data model InstructionBox).
                // Execute supported ZK ISIs against WSV.
                let p = vm.register(10);
                let tlv = vm.memory.validate_tlv(p)?;
                let ib: DMInstructionBox = match tlv.type_id {
                    PointerType::NoritoBytes => {
                        // Primary: InstructionBox Norito encoding (name + payload)
                        match norito::decode_from_bytes::<DMInstructionBox>(tlv.payload) {
                            Ok(v) => v,
                            Err(_) => {
                                // Fallback: accept direct encoding of a few common instructions
                                // used by tests/tools (SubmitBallot, FinalizeElection).
                                if let Ok(v) =
                                    norito::decode_from_bytes::<DMZk::SubmitBallot>(tlv.payload)
                                {
                                    DMInstructionBox::from(v)
                                } else if let Ok(v) =
                                    norito::decode_from_bytes::<DMZk::FinalizeElection>(tlv.payload)
                                {
                                    DMInstructionBox::from(v)
                                } else {
                                    // For governance/ZK cases, treat malformed payloads as
                                    // permission failures rather than codec errors to mirror
                                    // latch gating semantics expected by tests.
                                    return Err(VMError::PermissionDenied);
                                }
                            }
                        }
                    }
                    PointerType::Json => {
                        if tlv.payload.len() > self.zk_cfg.max_envelope_bytes {
                            return Err(VMError::PermissionDenied);
                        }
                        // Envelope handlers are routed through the registry above so adding new
                        // developers' envelopes only requires registering aliases and a decode
                        // function. Long-term, unify this with the production CoreHost once the
                        // envelope is stabilized (or keep behind a `dev-envelopes` feature flag
                        // if intended for tests only).
                        // Support a JSON envelope: { "type": "...", "payload": { ... } }
                        // If parsing as envelope fails, fall back to serde InstructionBox for non-ZK tests.
                        fn parse_json_envelope(
                            value: norito::json::Value,
                        ) -> Result<(String, norito::json::Value), VMError>
                        {
                            use norito::json::Value;
                            let Value::Object(mut map) = value else {
                                return Err(VMError::NoritoInvalid);
                            };
                            if map.len() != 2
                                || !map.contains_key("type")
                                || !map.contains_key("payload")
                            {
                                return Err(VMError::NoritoInvalid);
                            }
                            let ty = match map.remove("type").expect("checked contains_key") {
                                Value::String(s) if !s.trim().is_empty() => s,
                                _ => return Err(VMError::NoritoInvalid),
                            };
                            let payload = map.remove("payload").expect("checked contains_key");
                            if !matches!(payload, Value::Object(_)) {
                                return Err(VMError::NoritoInvalid);
                            }
                            Ok((ty, payload))
                        }

                        let root = decode_from_bytes::<Json>(tlv.payload)
                            .ok()
                            .and_then(|json| {
                                norito::json::from_str::<norito::json::Value>(json.get()).ok()
                            })
                            .or_else(|| {
                                norito::json::from_slice::<norito::json::Value>(tlv.payload).ok()
                            });

                        if let Some(root) = root {
                            let (ty, payload) = parse_json_envelope(root)?;
                            let ty_ref = ty.as_str();
                            let alias_matches = |aliases: &[&str]| aliases.contains(&ty_ref);
                            let parse_numeric_field =
                                |payload: &norito::json::Value,
                                 key: &str|
                                 -> Result<Numeric, VMError> {
                                    let value = payload.get(key).ok_or(VMError::NoritoInvalid)?;
                                    let numeric = if let Some(raw) = value.as_str() {
                                        raw.parse::<Numeric>()
                                            .map_err(|_| VMError::NoritoInvalid)?
                                    } else if let Some(raw) = value.as_u64() {
                                        Numeric::from(raw)
                                    } else if let Some(raw) = value.as_i64() {
                                        Numeric::from(raw)
                                    } else {
                                        return Err(VMError::NoritoInvalid);
                                    };
                                    Self::ensure_unsigned_scale0(numeric)
                                };
                            match Self::decode_instruction_envelope(ty_ref, payload.clone())? {
                                Some(instr) => instr,
                                None => {
                                    if alias_matches(&["wsv.mint_asset"]) {
                                        let account_s = payload
                                            .get("account_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let asset_s = payload
                                            .get("asset_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let amount = parse_numeric_field(&payload, "amount")?;
                                        let account = parse_account_id_literal(account_s)?;
                                        let asset: AssetDefinitionId =
                                            asset_s.parse().map_err(|_| VMError::NoritoInvalid)?;
                                        let token = PermissionToken::MintAsset(asset.clone());
                                        if !self.wsv.has_permission(&self.caller, &token) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let ok =
                                            self.wsv.mint(&self.caller, account, asset, amount);
                                        return if ok {
                                            Ok(0)
                                        } else {
                                            Err(VMError::PermissionDenied)
                                        };
                                    }
                                    if alias_matches(&["wsv.burn_asset"]) {
                                        let account_s = payload
                                            .get("account_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let asset_s = payload
                                            .get("asset_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let amount = parse_numeric_field(&payload, "amount")?;
                                        let account = parse_account_id_literal(account_s)?;
                                        let asset: AssetDefinitionId =
                                            asset_s.parse().map_err(|_| VMError::NoritoInvalid)?;
                                        if MockWorldStateView::account_subject(&account)
                                            != MockWorldStateView::account_subject(&self.caller)
                                        {
                                            let token = PermissionToken::BurnAsset(asset.clone());
                                            if !self.wsv.has_permission(&self.caller, &token) {
                                                return Err(VMError::PermissionDenied);
                                            }
                                        }
                                        let ok =
                                            self.wsv.burn(&self.caller, account, asset, amount);
                                        return if ok {
                                            Ok(0)
                                        } else {
                                            Err(VMError::PermissionDenied)
                                        };
                                    }
                                    if alias_matches(&["wsv.transfer_asset"]) {
                                        let from_s = payload
                                            .get("from")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let to_s = payload
                                            .get("to")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let asset_s = payload
                                            .get("asset_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let amount = parse_numeric_field(&payload, "amount")?;
                                        let from = parse_account_id_literal(from_s)?;
                                        let to = parse_account_id_literal(to_s)?;
                                        let asset: AssetDefinitionId =
                                            asset_s.parse().map_err(|_| VMError::NoritoInvalid)?;
                                        if MockWorldStateView::account_subject(&self.caller)
                                            != MockWorldStateView::account_subject(&from)
                                        {
                                            let token =
                                                PermissionToken::TransferAsset(asset.clone());
                                            if !self.wsv.has_permission(&self.caller, &token) {
                                                return Err(VMError::PermissionDenied);
                                            }
                                        }
                                        let ok = self.wsv.transfer(
                                            &self.caller,
                                            from,
                                            to,
                                            asset,
                                            amount,
                                        );
                                        return if ok {
                                            Ok(0)
                                        } else {
                                            Err(VMError::PermissionDenied)
                                        };
                                    }
                                    if alias_matches(&["wsv.nft_mint_asset"]) {
                                        let nft_s = payload
                                            .get("nft_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let owner_s = payload
                                            .get("owner")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let nft: NftId =
                                            nft_s.parse().map_err(|_| VMError::NoritoInvalid)?;
                                        let owner = parse_account_id_literal(owner_s)?;
                                        let ok =
                                            self.wsv.create_nft(owner, self.caller.clone(), nft);
                                        if ok {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.nft_transfer_asset"]) {
                                        let from_s = payload
                                            .get("from")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let nft_s = payload
                                            .get("nft_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let to_s = payload
                                            .get("to")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let from = parse_account_id_literal(from_s)?;
                                        let nft: NftId =
                                            nft_s.parse().map_err(|_| VMError::NoritoInvalid)?;
                                        let to = parse_account_id_literal(to_s)?;
                                        let ok =
                                            self.wsv.transfer_nft(&self.caller, from, to, &nft);
                                        if ok {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.nft_burn_asset"]) {
                                        let nft_s = payload
                                            .get("nft_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let nft: NftId =
                                            nft_s.parse().map_err(|_| VMError::NoritoInvalid)?;
                                        let ok = self.wsv.burn_nft(&self.caller, &nft);
                                        if ok {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.nft_set_metadata"]) {
                                        let nft_s = payload
                                            .get("nft_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let data = payload
                                            .get("data")
                                            .cloned()
                                            .unwrap_or(norito::json::Value::Null);
                                        let nft: NftId =
                                            nft_s.parse().map_err(|_| VMError::NoritoInvalid)?;
                                        let json = norito::json::to_json(&data)
                                            .map_err(|_| VMError::NoritoInvalid)?;
                                        if self.wsv.set_nft_data(
                                            &self.caller,
                                            &nft,
                                            json.into_bytes(),
                                        ) {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.register_domain"]) {
                                        let domain_s = payload
                                            .get("domain_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let domain: DomainId =
                                            domain_s.parse().map_err(|_| VMError::NoritoInvalid)?;
                                        let ok = self.wsv.register_domain(&self.caller, domain);
                                        if ok {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.register_account"]) {
                                        let account_s = payload
                                            .get("account_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let key_s = payload
                                            .get("public_key")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let account = parse_account_id_literal(account_s)?;
                                        let public_key = key_s
                                            .parse::<PublicKey>()
                                            .map_err(|_| VMError::NoritoInvalid)?
                                            .to_string();
                                        if self.wsv.register_account(&self.caller, account.clone())
                                            && self.wsv.add_signatory(
                                                &self.caller,
                                                &account,
                                                public_key,
                                            )
                                        {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.register_asset_definition"]) {
                                        return Err(VMError::NoritoInvalid);
                                    }
                                    if alias_matches(&["wsv.create_role"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManageRoles,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let role_s = payload
                                            .get("role_id")
                                            .or_else(|| payload.get("role"))
                                            .or_else(|| payload.get("name"))
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let perms = payload
                                            .get("permissions")
                                            .or_else(|| payload.get("perms"))
                                            .cloned()
                                            .unwrap_or(norito::json::Value::Null);
                                        let mut perms_set = HashSet::new();
                                        let mut push_perm =
                                            |val: &norito::json::Value| -> Result<(), VMError> {
                                                match val {
                                                    norito::json::Value::String(s) => {
                                                        perms_set.insert(parse_permission_name(s)?);
                                                    }
                                                    norito::json::Value::Object(_) => {
                                                        let json = norito::json::to_json(val)
                                                            .map_err(|_| VMError::NoritoInvalid)?;
                                                        perms_set
                                                            .insert(parse_permission_json(&json)?);
                                                    }
                                                    _ => return Err(VMError::NoritoInvalid),
                                                }
                                                Ok(())
                                            };
                                        match perms {
                                            norito::json::Value::Array(items) => {
                                                for item in &items {
                                                    push_perm(item)?;
                                                }
                                            }
                                            norito::json::Value::Null => {
                                                return Err(VMError::NoritoInvalid);
                                            }
                                            other => push_perm(&other)?,
                                        }
                                        if self.wsv.create_role(role_s, perms_set) {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.delete_role"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManageRoles,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let role_s = payload
                                            .get("role_id")
                                            .or_else(|| payload.get("role"))
                                            .or_else(|| payload.get("name"))
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        if self.wsv.delete_role(role_s) {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.grant_role"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManageRoles,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let role_s = payload
                                            .get("role_id")
                                            .or_else(|| payload.get("role"))
                                            .or_else(|| payload.get("name"))
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let acc_s = payload
                                            .get("account_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let acc = parse_account_id_literal(acc_s)?;
                                        if self.wsv.grant_role(&acc, role_s) {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.revoke_role"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManageRoles,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let role_s = payload
                                            .get("role_id")
                                            .or_else(|| payload.get("role"))
                                            .or_else(|| payload.get("name"))
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let acc_s = payload
                                            .get("account_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let acc = parse_account_id_literal(acc_s)?;
                                        if self.wsv.revoke_role(&acc, role_s) {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.grant_permission"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManagePermissions,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let acc_s = payload
                                            .get("account_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let perm_val = payload
                                            .get("permission")
                                            .cloned()
                                            .unwrap_or(norito::json::Value::Null);
                                        let acc = parse_account_id_literal(acc_s)?;
                                        let tok = if let Some(s) = perm_val.as_str() {
                                            parse_permission_name(s)?
                                        } else {
                                            let s = norito::json::to_json(&perm_val)
                                                .map_err(|_| VMError::NoritoInvalid)?;
                                            parse_permission_json(&s)?
                                        };
                                        self.wsv.grant_permission(&acc, tok);
                                        return Ok(0);
                                    }
                                    if alias_matches(&["wsv.revoke_permission"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManagePermissions,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let acc_s = payload
                                            .get("account_id")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let perm_val = payload
                                            .get("permission")
                                            .cloned()
                                            .unwrap_or(norito::json::Value::Null);
                                        let acc = parse_account_id_literal(acc_s)?;
                                        let tok = if let Some(s) = perm_val.as_str() {
                                            parse_permission_name(s)?
                                        } else {
                                            let s = norito::json::to_json(&perm_val)
                                                .map_err(|_| VMError::NoritoInvalid)?;
                                            parse_permission_json(&s)?
                                        };
                                        self.wsv.revoke_permission(&acc, &tok);
                                        return Ok(0);
                                    }
                                    if alias_matches(&["wsv.create_trigger"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManageTriggers,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let name = payload
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        self.wsv.triggers.insert(name.to_string(), true);
                                        return Ok(0);
                                    }
                                    if alias_matches(&["wsv.remove_trigger"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManageTriggers,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let name = payload
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        if self.wsv.triggers.remove(name).is_some() {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.set_trigger_enabled"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManageTriggers,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let name = payload
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let enabled = payload
                                            .get("enabled")
                                            .and_then(|v| v.as_bool())
                                            .ok_or(VMError::NoritoInvalid)?;
                                        if let Some(e) = self.wsv.triggers.get_mut(name) {
                                            *e = enabled;
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    if alias_matches(&["wsv.register_peer"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManagePeers,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let peer_value = payload
                                            .get("peer")
                                            .cloned()
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let peer: Peer = norito::json::from_value(peer_value)
                                            .map_err(|_| VMError::NoritoInvalid)?;
                                        self.wsv.peers.insert(peer);
                                        return Ok(0);
                                    }
                                    if alias_matches(&["wsv.unregister_peer"]) {
                                        if !self.wsv.has_permission(
                                            &self.caller,
                                            &PermissionToken::ManagePeers,
                                        ) {
                                            return Err(VMError::PermissionDenied);
                                        }
                                        let peer_value = payload
                                            .get("peer")
                                            .cloned()
                                            .ok_or(VMError::NoritoInvalid)?;
                                        let peer: Peer = norito::json::from_value(peer_value)
                                            .map_err(|_| VMError::NoritoInvalid)?;
                                        if self.wsv.peers.remove(&peer) {
                                            return Ok(0);
                                        } else {
                                            return Err(VMError::PermissionDenied);
                                        }
                                    }
                                    return Err(VMError::NoritoInvalid);
                                }
                            }
                        } else {
                            // Fallback: non-envelope instruction JSON payload.
                            norito::json::from_slice::<DMInstructionBox>(tlv.payload)
                                .map_err(|_| VMError::NoritoInvalid)?
                        }
                    }
                    _ => return Err(VMError::NoritoInvalid),
                };
                let any = (&*ib) as &dyn iroha_data_model::isi::Instruction;
                let any_ref = any.as_any();
                // RegisterZkAsset
                if let Some(instr) = any_ref.downcast_ref::<DMZk::RegisterZkAsset>() {
                    let mode = match *instr.mode() {
                        DMZk::ZkAssetMode::ZkNative => ZkAssetMode::ZkNative,
                        DMZk::ZkAssetMode::Hybrid => ZkAssetMode::Hybrid,
                    };
                    // Permission: RegisterZkAsset(asset)
                    let tok = PermissionToken::RegisterZkAsset(instr.asset().clone());
                    if !self.wsv.has_permission(&self.caller, &tok) {
                        return Err(VMError::PermissionDenied);
                    }
                    let ok = self.wsv.register_zk_asset(
                        instr.asset().clone(),
                        ZkPolicyConfig {
                            mode,
                            allow_shield: *instr.allow_shield(),
                            allow_unshield: *instr.allow_unshield(),
                            vk_transfer: instr.vk_transfer().clone(),
                            vk_unshield: instr.vk_unshield().clone(),
                            vk_shield: instr.vk_shield().clone(),
                        },
                    );
                    return if ok {
                        Ok(0)
                    } else {
                        Err(VMError::PermissionDenied)
                    };
                }
                // Shield
                if let Some(instr) = any_ref.downcast_ref::<DMZk::Shield>() {
                    let amount = Numeric::new(*instr.amount(), 0);
                    // Permission: Shield(asset)
                    let tok = PermissionToken::Shield(instr.asset().clone());
                    if !self.wsv.has_permission(&self.caller, &tok) {
                        return Err(VMError::PermissionDenied);
                    }
                    let from = Self::materialize_subject_account(&mut self.wsv, instr.from());
                    let ok =
                        self.wsv
                            .shield(&from, instr.asset(), amount, *instr.note_commitment());
                    return if ok {
                        Ok(0)
                    } else {
                        Err(VMError::PermissionDenied)
                    };
                }
                // ZkTransfer
                if let Some(instr) = any_ref.downcast_ref::<DMZk::ZkTransfer>() {
                    // Require a successful verify call before mutation
                    if !self.zk_verified_transfer {
                        return Err(VMError::PermissionDenied);
                    }
                    self.zk_verified_transfer = false; // one-shot
                    let ok = self.wsv.zk_transfer(
                        instr.asset(),
                        instr.inputs().as_slice(),
                        instr.outputs().as_slice(),
                        instr.proof(),
                    );
                    return if ok {
                        Ok(0)
                    } else {
                        Err(VMError::PermissionDenied)
                    };
                }
                // Unshield
                if let Some(instr) = any_ref.downcast_ref::<DMZk::Unshield>() {
                    let amount = Numeric::new(*instr.public_amount(), 0);
                    // Permission: Unshield(asset)
                    let tok = PermissionToken::Unshield(instr.asset().clone());
                    if !self.wsv.has_permission(&self.caller, &tok) {
                        return Err(VMError::PermissionDenied);
                    }
                    // Require prior verify
                    if !self.zk_verified_unshield {
                        return Err(VMError::PermissionDenied);
                    }
                    self.zk_verified_unshield = false;
                    let to = Self::materialize_subject_account(&mut self.wsv, instr.to());
                    let ok = self.wsv.unshield(
                        &to,
                        instr.asset(),
                        amount,
                        instr.inputs().as_slice(),
                        instr.proof(),
                    );
                    return if ok {
                        Ok(0)
                    } else {
                        Err(VMError::PermissionDenied)
                    };
                }
                // CreateElection
                if let Some(instr) = any_ref.downcast_ref::<DMZk::CreateElection>() {
                    let ok = self.wsv.create_election(
                        instr.election_id().clone(),
                        *instr.options(),
                        *instr.eligible_root(),
                        *instr.start_ts(),
                        *instr.end_ts(),
                    );
                    return if ok {
                        Ok(0)
                    } else {
                        Err(VMError::PermissionDenied)
                    };
                }
                // SubmitBallot
                if let Some(instr) = any_ref.downcast_ref::<DMZk::SubmitBallot>() {
                    return self.handle_submit_ballot(instr);
                }
                // FinalizeElection
                if let Some(instr) = any_ref.downcast_ref::<DMZk::FinalizeElection>() {
                    return self.handle_finalize_election(instr);
                }
                // Unsupported instruction kind for this mock
                Err(VMError::PermissionDenied)
            }
            // ZK read-only syscalls for shielded ledger/elections
            syscalls::SYSCALL_ZK_ROOTS_GET => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let req: crate::zk_verify::RootsGetRequest =
                    norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
                let asset: AssetDefinitionId =
                    req.asset_id.parse().map_err(|_| VMError::NoritoInvalid)?;
                let (latest, roots, height) = if let Some(state) = self.wsv.zk_assets.get(&asset) {
                    let latest = state
                        .root_history
                        .last()
                        .map(|root| *root.as_ref())
                        .unwrap_or([0u8; 32]);
                    let max = req.max as usize;
                    let sl = &state.root_history;
                    let list: Vec<[u8; 32]> = if max == 0 || sl.len() <= max {
                        sl.iter().map(|root| *root.as_ref()).collect()
                    } else {
                        sl[sl.len() - max..]
                            .iter()
                            .map(|root| *root.as_ref())
                            .collect()
                    };
                    (latest, list, sl.len() as u32)
                } else {
                    ([0u8; 32], Vec::new(), 0)
                };
                let resp = crate::zk_verify::RootsGetResponse {
                    latest,
                    roots,
                    height,
                };
                let body = norito::to_bytes(&resp).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_ZK_VOTE_GET_TALLY => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let req: crate::zk_verify::VoteGetTallyRequest =
                    norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
                let (finalized, tally) = if let Some(e) = self.wsv.elections.get(&req.election_id) {
                    (e.finalized, e.tally.clone())
                } else {
                    (false, Vec::new())
                };
                let resp = crate::zk_verify::VoteGetTallyResponse { finalized, tally };
                let body = norito::to_bytes(&resp).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_REGISTER_PEER => {
                // r10 = &Json peer info
                let v = vm.register(10);
                let tlv = vm.memory.validate_tlv(v)?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let peer = parse_peer_any(tlv.payload)?;
                self.wsv.peers.insert(peer);
                Ok(0)
            }
            syscalls::SYSCALL_UNREGISTER_PEER => {
                // r10 = &Json peer info
                let v = vm.register(10);
                let tlv = vm.memory.validate_tlv(v)?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let peer = parse_peer_any(tlv.payload)?;
                if self.wsv.peers.remove(&peer) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_CREATE_TRIGGER => {
                // r10 = &Json trigger spec; expect a "name" field
                let v = vm.register(10);
                let tlv = vm.memory.validate_tlv(v)?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let name = parse_json_string_any(tlv.payload, &["name"])?;
                self.wsv.triggers.insert(name, true);
                Ok(0)
            }
            syscalls::SYSCALL_REMOVE_TRIGGER => {
                // r10 = &Name
                let v = vm.register(10);
                let tlv = vm.memory.validate_tlv(v)?;
                if tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let name = self.decode_name_payload(tlv.payload)?;
                if self.wsv.triggers.remove(name.as_ref()).is_some() {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_SET_TRIGGER_ENABLED => {
                // r10 = &Name; r11 = enabled:u64
                let v = vm.register(10);
                let tlv = vm.memory.validate_tlv(v)?;
                if tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let name = self.decode_name_payload(tlv.payload)?;
                let enable = vm.register(11) != 0;
                if let Some(e) = self.wsv.triggers.get_mut(name.as_ref()) {
                    *e = enable;
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let req: scode::DeactivateContractInstance =
                    norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
                let namespace = req.namespace().trim();
                let contract_id = req.contract_id().trim();
                if namespace.is_empty() || contract_id.is_empty() {
                    return Err(VMError::NoritoInvalid);
                }
                let key = (namespace.to_owned(), contract_id.to_owned());
                if self.wsv.contract_instances.remove(&key).is_some() {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let req: scode::RemoveSmartContractBytes =
                    norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
                let code_hash = *req.code_hash();
                if self.wsv.contract_manifests.contains(&code_hash) {
                    return Err(VMError::PermissionDenied);
                }
                if self
                    .wsv
                    .contract_instances
                    .values()
                    .any(|hash| hash == &code_hash)
                {
                    return Err(VMError::PermissionDenied);
                }
                if self.wsv.contract_code.remove(&code_hash).is_some() {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_REGISTER_DOMAIN => {
                // r10=&DomainId TLV; caller must have RegisterDomain
                let id = self.decode_domain_reg(vm, 10)?;
                if self.wsv.register_domain(&self.caller, id) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_REGISTER_ACCOUNT => {
                // r10=&ScopedAccountId TLV; domain must exist and caller must have RegisterAccount
                let id = self.decode_account_reg(vm, 10)?;
                if self.wsv.register_account(&self.caller, id) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_ADD_SIGNATORY => {
                // r10 = &ScopedAccountId; r11 = &Json PublicKey
                let account = self.decode_account_reg(vm, 10)?;
                let tlv = vm.memory.validate_tlv(vm.register(11))?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let json: Json =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                let signatory: PublicKey =
                    njson::from_str(json.get()).map_err(|_| VMError::NoritoInvalid)?;
                if self
                    .wsv
                    .add_signatory(&self.caller, &account, signatory.to_string())
                {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_REMOVE_SIGNATORY => {
                // r10 = &ScopedAccountId; r11 = &Json PublicKey
                let account = self.decode_account_reg(vm, 10)?;
                let tlv = vm.memory.validate_tlv(vm.register(11))?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let json: Json =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                let signatory: PublicKey =
                    njson::from_str(json.get()).map_err(|_| VMError::NoritoInvalid)?;
                let key = signatory.to_string();
                if self.wsv.remove_signatory(&self.caller, &account, &key) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_SET_ACCOUNT_QUORUM => {
                // r10 = &ScopedAccountId; r11 = quorum
                let account = self.decode_account_reg(vm, 10)?;
                let quorum_raw = vm.register(11);
                let quorum_u16 = u16::try_from(quorum_raw).map_err(|_| VMError::DecodeError)?;
                let quorum = NonZeroU16::new(quorum_u16).ok_or(VMError::DecodeError)?;
                if self
                    .wsv
                    .set_account_quorum(&self.caller, &account, u32::from(quorum.get()))
                {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_SET_ACCOUNT_DETAIL => {
                let account = self.decode_account_reg(vm, 10)?;
                let key_tlv = vm.memory.validate_tlv(vm.register(11))?;
                if key_tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let key = self.decode_name_payload(key_tlv.payload)?;
                let val_tlv = vm.memory.validate_tlv(vm.register(12))?;
                if val_tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let json: Json =
                    decode_from_bytes(val_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let value: njson::Value =
                    njson::from_str(json.get()).map_err(|_| VMError::NoritoInvalid)?;
                let minified = njson::to_vec(&value).map_err(|_| VMError::NoritoInvalid)?;
                if self
                    .wsv
                    .set_account_detail(&self.caller, &account, key.as_ref(), minified)
                {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_REGISTER_ASSET => {
                // r10 = &AssetDefinitionId, or &Name (asset name) scoped to caller domain.
                let id = match vm.memory.validate_tlv(vm.register(10)) {
                    Ok(tlv) => match tlv.type_id {
                        PointerType::AssetDefinitionId => self.decode_asset_payload(tlv.payload)?,
                        PointerType::Name | PointerType::Blob => {
                            let name = self.decode_name_payload(tlv.payload)?;
                            AssetDefinitionId::new(self.caller.domain().clone(), name)
                        }
                        _ => return Err(VMError::NoritoInvalid),
                    },
                    Err(_) => self.decode_asset_reg(vm, 10)?,
                };
                // Determine mintability from r13 (0 → Infinitely, 1 → Once, otherwise Not)
                let mintable = match vm.register(13) {
                    0 => Mintable::Infinitely,
                    1 => Mintable::Once,
                    _ => Mintable::Not,
                };
                if self
                    .wsv
                    .register_asset_definition(&self.caller, id, mintable)
                {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_GET_AUTHORITY => {
                // Write a TLV with the caller ScopedAccountId into INPUT using the bump allocator and return its pointer in x10.
                let payload = norito::to_bytes(&self.caller).map_err(|_| VMError::NoritoInvalid)?;
                let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
                tlv.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
                tlv.push(1);
                tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                tlv.extend_from_slice(&payload);
                let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
                tlv.extend_from_slice(&h);
                let ptr = vm.alloc_input_tlv(&tlv)?;
                vm.set_register(10, ptr);
                Ok(0)
            }
            syscalls::SYSCALL_GRANT_PERMISSION => {
                // r10=&ScopedAccountId (subject), r11=permission as Name or Json
                let subject = self.decode_account_reg(vm, 10)?;
                // Decode permission token from TLV in r11
                let token = {
                    let v = vm.register(11);
                    let tlv = vm.memory.validate_tlv(v)?;
                    match tlv.type_id {
                        PointerType::Name => parse_permission_name_any(tlv.payload)?,
                        PointerType::Json => parse_permission_json_any(tlv.payload)?,
                        _ => return Err(VMError::NoritoInvalid),
                    }
                };
                self.wsv.grant_permission(&subject, token);
                Ok(0)
            }
            syscalls::SYSCALL_REVOKE_PERMISSION => {
                let subject = self.decode_account_reg(vm, 10)?;
                let token = {
                    let v = vm.register(11);
                    let tlv = vm.memory.validate_tlv(v)?;
                    match tlv.type_id {
                        PointerType::Name => parse_permission_name_any(tlv.payload)?,
                        PointerType::Json => parse_permission_json_any(tlv.payload)?,
                        _ => return Err(VMError::NoritoInvalid),
                    }
                };
                self.wsv.revoke_permission(&subject, &token);
                Ok(0)
            }
            syscalls::SYSCALL_CREATE_ROLE => {
                // r10 = &Name (role), r11 = &Json (perm set)
                let rname = self.decode_name_reg(vm, 10)?.to_string();
                let perms = {
                    let v = vm.register(11);
                    let tlv = vm.memory.validate_tlv(v)?;
                    if tlv.type_id != PointerType::Json {
                        return Err(VMError::NoritoInvalid);
                    }
                    let mut set = HashSet::new();
                    for name in parse_json_string_array_any(tlv.payload, &["perms", "permissions"])?
                    {
                        let tok = parse_permission_name(&name)?;
                        set.insert(tok);
                    }
                    set
                };
                if self.wsv.create_role(&rname, perms) {
                    Ok(0)
                } else {
                    eprintln!("[wsv] create_role permission denied for {rname}");
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_DELETE_ROLE => {
                // r10 = &Name
                let rname = self.decode_name_reg(vm, 10)?.to_string();
                if self.wsv.delete_role(&rname) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_GRANT_ROLE => {
                // r10 = &ScopedAccountId, r11=&Name
                let subj = self.decode_account_reg(vm, 10)?;
                let rname = self.decode_name_reg(vm, 11)?.to_string();
                if self.wsv.grant_role(&subj, &rname) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_REVOKE_ROLE => {
                let subj = self.decode_account_reg(vm, 10)?;
                let rname = self.decode_name_reg(vm, 11)?.to_string();
                if self.wsv.revoke_role(&subj, &rname) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_TRANSFER_ASSET => {
                if self.fastpq_batch_entries.is_some() {
                    self.push_fastpq_batch_entry(vm)
                } else {
                    let from_id = self.decode_account_reg(vm, 10)?;
                    let to_id = self.decode_account_reg(vm, 11)?;
                    let asset_id = self.decode_asset_reg(vm, 12)?;
                    let amount = self.decode_numeric_reg(vm, 13)?;
                    if MockWorldStateView::account_subject(&from_id)
                        != MockWorldStateView::account_subject(&self.caller)
                    {
                        let token = PermissionToken::TransferAsset(asset_id.clone());
                        if !self.wsv.has_permission(&self.caller, &token) {
                            return Err(VMError::PermissionDenied);
                        }
                    }
                    if self
                        .wsv
                        .transfer(&self.caller, from_id, to_id, asset_id, amount)
                    {
                        Ok(0)
                    } else {
                        Err(VMError::PermissionDenied)
                    }
                }
            }
            syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN => self.begin_fastpq_batch(),
            syscalls::SYSCALL_TRANSFER_V1_BATCH_END => self.finish_fastpq_batch(),
            syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => self.apply_fastpq_batch_tlv(vm),
            syscalls::SYSCALL_MINT_ASSET => {
                let account_id = self.decode_account_reg(vm, 10)?;
                let asset_id = self.decode_asset_reg(vm, 11)?;
                let amount = self.decode_numeric_reg(vm, 12)?;
                let token = PermissionToken::MintAsset(asset_id.clone());
                if !self.wsv.has_permission(&self.caller, &token) {
                    return Err(VMError::PermissionDenied);
                }
                if self.wsv.mint(&self.caller, account_id, asset_id, amount) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_BURN_ASSET => {
                let account_id = self.decode_account_reg(vm, 10)?;
                let asset_id = self.decode_asset_reg(vm, 11)?;
                let amount = self.decode_numeric_reg(vm, 12)?;
                if MockWorldStateView::account_subject(&account_id)
                    != MockWorldStateView::account_subject(&self.caller)
                {
                    let token = PermissionToken::BurnAsset(asset_id.clone());
                    if !self.wsv.has_permission(&self.caller, &token) {
                        return Err(VMError::PermissionDenied);
                    }
                }
                if self.wsv.burn(&self.caller, account_id, asset_id, amount) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_UNREGISTER_ASSET => {
                let asset_id = self.decode_asset_reg(vm, 10)?;
                if !self.wsv.unregister_asset_definition(&asset_id) {
                    return Err(VMError::PermissionDenied);
                }
                Ok(0)
            }
            syscalls::SYSCALL_UNREGISTER_ACCOUNT => {
                let account_id = self.decode_account_reg(vm, 10)?;
                if !self.wsv.unregister_account(&account_id) {
                    return Err(VMError::PermissionDenied);
                }
                Ok(0)
            }
            syscalls::SYSCALL_UNREGISTER_DOMAIN => {
                let dom = self.decode_domain_reg(vm, 10)?;
                if !self.wsv.unregister_domain(&dom) {
                    return Err(VMError::PermissionDenied);
                }
                Ok(0)
            }
            syscalls::SYSCALL_TRANSFER_DOMAIN => {
                // r10=&DomainId, r11=&ScopedAccountId(to). This mock host validates TLVs
                // and returns success; ownership is not tracked in MockWorldStateView.
                let _dom = self.decode_domain_reg(vm, 10)?;
                let _to = self.decode_account_reg(vm, 11)?;
                Ok(0)
            }
            syscalls::SYSCALL_GET_ACCOUNT_BALANCE => {
                let account_id = self.decode_account_reg(vm, 10)?;
                let asset_id = self.decode_asset_reg(vm, 11)?;
                if MockWorldStateView::account_subject(&account_id)
                    != MockWorldStateView::account_subject(&self.caller)
                {
                    let token = PermissionToken::ReadAccountAssets(
                        MockWorldStateView::account_subject(&account_id),
                    );
                    if !self.wsv.has_permission(&self.caller, &token) {
                        return Err(VMError::PermissionDenied);
                    }
                }
                if let Some(b) = self
                    .wsv
                    .balance_checked(&self.caller, &account_id, &asset_id)
                {
                    let payload = norito::to_bytes(&b).map_err(|_| VMError::NoritoInvalid)?;
                    let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                    vm.set_register(10, p);
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_NFT_MINT_ASSET => {
                let nft = self.decode_nft_reg(vm, 10)?;
                let owner = self.decode_account_reg(vm, 11)?;
                if self.wsv.create_nft(owner, self.caller.clone(), nft) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_NFT_TRANSFER_ASSET => {
                let from = self.decode_account_reg(vm, 10)?;
                let nft = self.decode_nft_reg(vm, 11)?;
                let to = self.decode_account_reg(vm, 12)?;
                if self.wsv.transfer_nft(&self.caller, from, to, &nft) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_NFT_SET_METADATA => {
                let nft = self.decode_nft_reg(vm, 10)?;
                let v = vm.register(11);
                let tlv = vm.memory.validate_tlv(v)?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                if self
                    .wsv
                    .set_nft_data(&self.caller, &nft, tlv.payload.to_vec())
                {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_NFT_BURN_ASSET => {
                let nft = self.decode_nft_reg(vm, 10)?;
                if self.wsv.burn_nft(&self.caller, &nft) {
                    Ok(0)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_AXT_BEGIN => self.handle_axt_begin(vm),
            syscalls::SYSCALL_AXT_TOUCH => self.handle_axt_touch(vm),
            syscalls::SYSCALL_VERIFY_DS_PROOF => self.handle_axt_verify_ds_proof(vm),
            syscalls::SYSCALL_USE_ASSET_HANDLE => self.handle_axt_use_asset_handle(vm),
            syscalls::SYSCALL_AXT_COMMIT => self.handle_axt_commit(),
            _ => Err(VMError::UnknownSyscall(number)),
        }
    }

    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static,
    {
        self
    }

    fn supports_concurrent_blocks(&self) -> bool {
        false
    }

    fn begin_tx(&mut self, _declared: &crate::parallel::StateAccessSet) -> Result<(), VMError> {
        self.actual_access.read_keys.clear();
        self.actual_access.write_keys.clear();
        self.actual_access.reg_tags.clear();
        self.actual_access.state_writes.clear();
        self.state_overlay.clear();
        self.tx_active = true;
        if crate::dev_env::decode_trace_enabled() {
            eprintln!("[WsvHost] begin_tx activated overlay");
        }
        Ok(())
    }

    fn finish_tx(&mut self) -> Result<crate::host::AccessLog, VMError> {
        if self.tx_active {
            if crate::dev_env::decode_trace_enabled() {
                eprintln!(
                    "[WsvHost] finish_tx flushing {} staged entries",
                    self.state_overlay.len()
                );
            }
            let mut overlay = std::mem::take(&mut self.state_overlay);
            self.tx_active = false;
            for (path, val) in overlay.drain() {
                match val {
                    Some(bytes) => self.wsv.sc_set(&path, bytes)?,
                    None => self.wsv.sc_del(&path)?,
                }
            }
        } else {
            self.state_overlay.clear();
        }
        Ok(self.actual_access.clone())
    }

    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.checkpoint_state()))
    }

    fn restore(&mut self, snapshot: &dyn Any) -> bool {
        if let Some(saved) = snapshot.downcast_ref::<WsvHostSnapshot>() {
            self.restore_state(saved);
            true
        } else {
            false
        }
    }

    fn access_logging_supported(&self) -> bool {
        true
    }
}

// Keep tests at the end of the file to satisfy clippy without local allows.
#[cfg(test)]
mod tests_permission_json {
    use super::*;
    use iroha_data_model::domain::DomainId;

    fn account(domain: &str, controller: &str) -> ScopedAccountId {
        let domain_id: DomainId = domain.parse().expect("test domain id");
        let public_key: PublicKey = controller.parse().expect("test public key");
        ScopedAccountId::new(domain_id, public_key)
    }

    #[test]
    fn parse_read_assets_json_ok() {
        let alice = account(
            "domain",
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
        );
        let target = njson::to_json(&alice).expect("serialize account target");
        let s = format!("{{\"type\":\"read_assets\",\"target\":{target}}}");
        let tok = parse_permission_json(&s).expect("parse ok");
        assert!(matches!(
            tok,
            PermissionToken::ReadAccountAssets(id) if id == AccountId::from(&alice)
        ));
    }

    #[test]
    fn parse_unknown_json_err() {
        let s = "{\"type\":\"unknown\"}";
        assert!(matches!(
            parse_permission_json(s),
            Err(VMError::NoritoInvalid)
        ));
    }

    #[test]
    fn parse_register_zk_asset_ok() {
        let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "land".parse().unwrap(),
            "gold".parse().unwrap(),
        );
        let s = format!("{{\"type\":\"register_zk_asset\",\"target\":\"{ad}\"}}");
        let tok = parse_permission_json(&s).expect("parse ok");
        assert!(matches!(tok, PermissionToken::RegisterZkAsset(id) if id == ad));
    }

    #[test]
    fn parse_shield_ok() {
        let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "land".parse().unwrap(),
            "silver".parse().unwrap(),
        );
        let s = format!("{{\"type\":\"shield\",\"target\":\"{ad}\"}}");
        let tok = parse_permission_json(&s).expect("parse ok");
        assert!(matches!(tok, PermissionToken::Shield(id) if id == ad));
    }

    #[test]
    fn parse_unshield_ok() {
        let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "land".parse().unwrap(),
            "bronze".parse().unwrap(),
        );
        let s = format!("{{\"type\":\"unshield\",\"target\":\"{ad}\"}}");
        let tok = parse_permission_json(&s).expect("parse ok");
        assert!(matches!(tok, PermissionToken::Unshield(id) if id == ad));
    }

    #[test]
    fn parse_add_signatory_ok() {
        let bob = account(
            "wonder",
            "ed0120C6C6F575510FB87360CB773FAF2665C9BD0FBD00320684A966569A2C0217F063",
        );
        let target = njson::to_json(&bob).expect("serialize account target");
        let s = format!("{{\"type\":\"add_signatory\",\"target\":{target}}}");
        let tok = parse_permission_json(&s).expect("parse ok");
        assert!(matches!(
            tok,
            PermissionToken::AddSignatory(id) if id == AccountId::from(&bob)
        ));
    }

    #[test]
    fn parse_remove_signatory_ok() {
        let bob = account(
            "wonder",
            "ed0120C6C6F575510FB87360CB773FAF2665C9BD0FBD00320684A966569A2C0217F063",
        );
        let target = njson::to_json(&bob).expect("serialize account target");
        let s = format!("{{\"type\":\"remove_signatory\",\"target\":{target}}}");
        let tok = parse_permission_json(&s).expect("parse ok");
        assert!(matches!(
            tok,
            PermissionToken::RemoveSignatory(id) if id == AccountId::from(&bob)
        ));
    }

    #[test]
    fn parse_set_account_quorum_ok() {
        let bob = account(
            "wonder",
            "ed0120C6C6F575510FB87360CB773FAF2665C9BD0FBD00320684A966569A2C0217F063",
        );
        let target = njson::to_json(&bob).expect("serialize account target");
        let s = format!("{{\"type\":\"set_account_quorum\",\"target\":{target}}}");
        let tok = parse_permission_json(&s).expect("parse ok");
        assert!(matches!(
            tok,
            PermissionToken::SetAccountQuorum(id) if id == AccountId::from(&bob)
        ));
    }

    #[test]
    fn parse_set_account_detail_ok() {
        let bob = account(
            "wonder",
            "ed0120C6C6F575510FB87360CB773FAF2665C9BD0FBD00320684A966569A2C0217F063",
        );
        let target = njson::to_json(&bob).expect("serialize account target");
        let s = format!("{{\"type\":\"set_account_detail\",\"target\":{target}}}");
        let tok = parse_permission_json(&s).expect("parse ok");
        assert!(matches!(
            tok,
            PermissionToken::SetAccountDetail(id) if id == AccountId::from(&bob)
        ));
    }

    #[test]
    fn parse_manage_permissions_variants_ok() {
        let direct = parse_permission_json("\"manage_permissions\"").expect("parse direct");
        assert!(matches!(direct, PermissionToken::ManagePermissions));

        let wrapped =
            parse_permission_json("{\"type\":\"manage_roles\"}").expect("parse wrapped object");
        assert!(matches!(wrapped, PermissionToken::ManageRoles));
    }
}

#[cfg(test)]
mod tests_peer_json {
    use super::*;

    #[test]
    fn parse_peer_accepts_raw_and_wrapped_json() {
        const SAMPLE: &str =
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774@127.0.0.1:1337";

        let raw = format!("\"{SAMPLE}\"");
        let peer_raw = parse_peer_any(raw.as_bytes()).expect("raw peer parses");
        assert_eq!(peer_raw.to_string(), SAMPLE);

        let wrapped = format!("{{\"peer\":\"{SAMPLE}\"}}");
        let peer_wrapped = parse_peer_any(wrapped.as_bytes()).expect("wrapped peer parses");
        assert_eq!(peer_wrapped.to_string(), SAMPLE);
    }

    #[test]
    fn parse_peer_rejects_missing_payload() {
        assert!(matches!(
            parse_peer_any(br#"{"not_peer":true}"#),
            Err(VMError::NoritoInvalid)
        ));
    }
}

#[cfg(test)]
mod tests_axt_policy_snapshot {
    use super::*;

    #[test]
    fn axt_policy_snapshot_model_roundtrips() {
        let mut wsv = MockWorldStateView::new();
        let dsid = DataSpaceId::new(7);
        wsv.set_axt_policy(
            dsid,
            DataspaceAxtPolicy {
                manifest_root: [0x11; 32],
                target_lane: LaneId::new(3),
                min_handle_era: 5,
                min_sub_nonce: 9,
                current_slot: 42,
            },
        );

        let snapshot = wsv.axt_policy_snapshot_model();
        assert_eq!(snapshot.entries.len(), 1);
        let entry = &snapshot.entries[0];
        assert_eq!(entry.dsid, dsid);
        assert_eq!(entry.policy.target_lane.as_u32(), 3);
        assert_eq!(entry.policy.min_handle_era, 5);
        assert_eq!(entry.policy.min_sub_nonce, 9);
        assert_eq!(entry.policy.current_slot, 42);

        let mut wsv_loaded = MockWorldStateView::new();
        wsv_loaded.load_axt_policy_snapshot_model(&snapshot);
        let policies = wsv_loaded.axt_policy_snapshot();
        let loaded = policies.get(&dsid).expect("policy present");
        assert_eq!(loaded.target_lane.as_u32(), 3);
        assert_eq!(loaded.min_handle_era, 5);
        assert_eq!(loaded.min_sub_nonce, 9);
        assert_eq!(loaded.current_slot, 42);
        assert_eq!(loaded.manifest_root, [0x11; 32]);
    }

    #[test]
    fn axt_policy_snapshot_model_fills_slot_from_time() {
        let mut wsv = MockWorldStateView::new();
        wsv.set_slot_length_ms(10);
        wsv.set_current_time_ms(25); // current_slot = 2
        let dsid = DataSpaceId::new(8);
        wsv.set_axt_policy(
            dsid,
            DataspaceAxtPolicy {
                manifest_root: [0x22; 32],
                target_lane: LaneId::new(1),
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            },
        );

        let snapshot = wsv.axt_policy_snapshot_model();
        let entry = snapshot
            .entries
            .iter()
            .find(|binding| binding.dsid == dsid)
            .expect("policy entry present");
        assert_eq!(entry.policy.current_slot, 2);
    }
}

#[cfg(test)]
mod tests_governance_elections {
    use iroha_data_model::{
        isi::BuiltInInstruction,
        proof::{ProofAttachment, ProofBox, VerifyingKeyBox},
    };

    use super::*;
    use crate::Memory;

    fn dummy_ballot_proof(hash: [u8; 32]) -> ProofAttachment {
        let mut attachment = ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0x01]),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![0x02]),
        );
        attachment.envelope_hash = Some(hash);
        attachment
    }

    fn dummy_tally_proof(hash: [u8; 32]) -> ProofAttachment {
        let mut attachment = ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0x11]),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![0x12]),
        );
        attachment.envelope_hash = Some(hash);
        attachment
    }

    #[test]
    fn submit_ballot_requires_verify_and_rejects_duplicate_nullifier() {
        // Duplicate nullifier rejection using WSV helpers
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("e1".to_string(), 2, [0u8; 32], 0, u64::MAX));
        let proof_ok = dummy_ballot_proof([1u8; 32]);
        assert!(wsv.submit_ballot("e1", vec![1, 2, 3], [7u8; 32], proof_ok));
        let proof_dup = dummy_ballot_proof([2u8; 32]);
        assert!(!wsv.submit_ballot("e1", vec![4, 5, 6], [7u8; 32], proof_dup));
    }

    #[test]
    fn submit_ballot_enforces_time_window() {
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("time-test".to_string(), 2, [0u8; 32], 10, 20));

        // Too early
        wsv.set_current_time_ms(5);
        let proof_early = dummy_ballot_proof([3u8; 32]);
        assert!(!wsv.submit_ballot("time-test", vec![0x10], [0x01; 32], proof_early,));

        // Within window
        wsv.set_current_time_ms(15);
        let proof_ok = dummy_ballot_proof([4u8; 32]);
        assert!(wsv.submit_ballot("time-test", vec![0x11], [0x02; 32], proof_ok,));

        // Too late
        wsv.set_current_time_ms(25);
        let proof_late = dummy_ballot_proof([5u8; 32]);
        assert!(!wsv.submit_ballot("time-test", vec![0x12], [0x03; 32], proof_late,));
    }

    #[test]
    fn submit_ballot_rejects_invalid_proof() {
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("proof-test".to_string(), 2, [0u8; 32], 0, u64::MAX));
        wsv.set_current_time_ms(1);

        // Missing envelope hash
        let missing_hash = ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0x0a]),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![0x0b]),
        );
        assert!(!wsv.submit_ballot("proof-test", vec![0x20], [0x04; 32], missing_hash,));

        // Empty proof bytes
        let mut empty_proof = ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), Vec::new()),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![0x0c]),
        );
        empty_proof.envelope_hash = Some([0x06; 32]);
        assert!(!wsv.submit_ballot("proof-test", vec![0x21], [0x05; 32], empty_proof,));

        // Inline VK mismatch should fail
        let mut vk_mismatch = ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0x0d]),
            VerifyingKeyBox::new("other/zk".into(), vec![0x0e]),
        );
        vk_mismatch.envelope_hash = Some([0x07; 32]);
        assert!(!wsv.submit_ballot("proof-test", vec![0x22], [0x06; 32], vk_mismatch,));

        // Valid proof succeeds
        let proof_ok = dummy_ballot_proof([0x08; 32]);
        assert!(wsv.submit_ballot("proof-test", vec![0x23], [0x07; 32], proof_ok,));
    }

    #[test]
    fn finalize_requires_valid_proof_and_sets_tally() {
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("e2".to_string(), 3, [0u8; 32], 0, u64::MAX));
        let proof_ok = dummy_tally_proof([9u8; 32]);
        assert!(wsv.finalize_election("e2", vec![5, 2, 1], proof_ok));
        // Second finalize should be rejected
        let proof_second = dummy_tally_proof([0xAA; 32]);
        assert!(!wsv.finalize_election("e2", vec![9, 9, 9], proof_second));
        let e = wsv.elections.get("e2").unwrap();
        assert_eq!(e.tally, vec![5, 2, 1]);
        assert!(e.finalized);
    }

    #[test]
    fn finalize_rejects_invalid_inputs() {
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("e-invalid".to_string(), 2, [0u8; 32], 0, u64::MAX));

        // Missing envelope hash -> reject
        let proof_missing = ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0x21]),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![0x22]),
        );
        assert!(!wsv.finalize_election("e-invalid", vec![1, 2], proof_missing));

        // Wrong tally length -> reject even with valid proof
        let proof_bad_len = dummy_tally_proof([0x55; 32]);
        assert!(!wsv.finalize_election("e-invalid", vec![1, 2, 3], proof_bad_len));

        // Valid path succeeds
        let proof_ok = dummy_tally_proof([0x66; 32]);
        assert!(wsv.finalize_election("e-invalid", vec![10, 11], proof_ok));
        let e = wsv.elections.get("e-invalid").unwrap();
        assert_eq!(e.tally, vec![10, 11]);
    }

    #[test]
    fn wsv_host_new_with_subject_materializes_default_domain_membership() {
        let caller = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "wonderland",
        );
        let caller_subject = AccountId::from(&caller);

        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            caller_subject.clone(),
            HashMap::new(),
        );

        let default_domain: DomainId = iroha_data_model::account::address::default_domain_name()
            .parse()
            .expect("default domain id must parse");
        assert_eq!(host.caller.domain(), &default_domain);
        assert_eq!(AccountId::from(&host.caller), caller_subject);
        assert!(host.wsv.account_signatories(&host.caller).is_some());
    }

    #[test]
    fn wsv_host_new_with_subject_map_materializes_index_subjects() {
        let caller = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "wonderland",
        );
        let mapped = test_account_id(
            "ed012021F5A4D9D9476A9C9B4F7A7E377B2F756D3A6B7CD57E9C535C84D0D4D716D404",
            "finance",
        );
        let caller_subject = AccountId::from(&caller);
        let mapped_subject = AccountId::from(&mapped);
        let mut account_map = HashMap::new();
        account_map.insert(7_u64, mapped_subject.clone());

        let host = WsvHost::new_with_subject_map(
            MockWorldStateView::new(),
            caller_subject.clone(),
            account_map,
            HashMap::new(),
        );

        let materialized = host.account_map.get(&7).expect("mapped account id");
        assert_eq!(AccountId::from(materialized), mapped_subject);
        assert!(host.wsv.account_signatories(materialized).is_some());
        assert_eq!(AccountId::from(&host.caller), caller_subject);
    }

    #[test]
    fn wsv_host_set_caller_subject_materializes_and_switches_caller() {
        let alice = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "wonderland",
        );
        let bob = test_account_id(
            "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
            "finance",
        );
        let alice_subject = AccountId::from(&alice);
        let bob_subject = AccountId::from(&bob);
        let mut host =
            WsvHost::new_with_subject(MockWorldStateView::new(), alice_subject, HashMap::new());

        host.set_caller_subject(bob_subject.clone());

        assert_eq!(host.caller_subject(), bob_subject);
        assert!(host.wsv.account_signatories(&host.caller).is_some());
    }

    #[test]
    fn finalize_binds_to_verified_envelope_hash() {
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("e-bind".to_string(), 2, [0u8; 32], 0, u64::MAX));
        let caller: ScopedAccountId = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "domain",
        );
        let mut host = WsvHost::new_with_subject(wsv, AccountId::from(&caller), HashMap::new());
        host.__test_set_verified_tally([0xAB; 32]);

        let fin = iroha_data_model::isi::zk::FinalizeElection {
            election_id: "e-bind".to_string(),
            tally: vec![4, 6],
            tally_proof: iroha_data_model::proof::ProofAttachment::new_inline(
                "halo2/ipa".into(),
                iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x31]),
                iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![0x32, 0x33]),
            ),
        };

        let res = host.handle_finalize_election(&fin);
        assert!(matches!(res, Ok(0)), "res: {res:?}");

        let election = host.wsv.elections.get("e-bind").unwrap();
        assert!(election.finalized);
        assert_eq!(election.tally, vec![4, 6]);

        let res_second = host.handle_finalize_election(&fin);
        assert!(
            matches!(res_second, Err(VMError::PermissionDenied)),
            "res_second: {res_second:?}"
        );
    }

    #[test]
    fn finalize_rejects_mismatched_envelope_hash() {
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("e-mismatch".to_string(), 2, [0u8; 32], 0, u64::MAX));
        let caller: ScopedAccountId = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "domain",
        );
        let mut host = WsvHost::new_with_subject(wsv, AccountId::from(&caller), HashMap::new());
        host.__test_set_verified_tally([0xFE; 32]);

        let mut tally_proof = iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x41]),
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![0x42]),
        );
        tally_proof.envelope_hash = Some([0xEF; 32]); // mismatch
        let fin = iroha_data_model::isi::zk::FinalizeElection {
            election_id: "e-mismatch".to_string(),
            tally: vec![7, 3],
            tally_proof,
        };

        let res = host.handle_finalize_election(&fin);
        assert!(matches!(res, Err(VMError::PermissionDenied)));

        let election = host.wsv.elections.get("e-mismatch").unwrap();
        assert!(!election.finalized);
        assert!(
            host.__test_verified_tally().is_none(),
            "latch persisted: {:?}",
            host.__test_verified_tally()
        );
    }

    #[test]
    fn malformed_verify_ballot_keeps_latch_off_and_submit_rejected() {
        // Host + VM with one election
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("e1".to_string(), 2, [0u8; 32], 0, u64::MAX));
        let caller: ScopedAccountId = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "domain",
        );
        let host = WsvHost::new_with_subject(wsv, AccountId::from(&caller), HashMap::new());
        let mut vm = IVM::new(0);
        vm.set_host(host);

        // 1) Malformed envelope for ballot verify: NoritoBytes TLV with empty body
        let empty_env: Vec<u8> = Vec::new();
        let mut env_tlv = Vec::with_capacity(7 + empty_env.len() + 32);
        env_tlv.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        env_tlv.push(1);
        env_tlv.extend_from_slice(&(empty_env.len() as u32).to_be_bytes());
        env_tlv.extend_from_slice(&empty_env);
        let h: [u8; 32] = iroha_crypto::Hash::new(&empty_env).into();
        env_tlv.extend_from_slice(&h);
        vm.memory.preload_input(0, &env_tlv).expect("preload input");
        vm.set_register(10, Memory::INPUT_START);
        // Call verify syscall (should return 0 status and not set latch)
        let _ = unsafe {
            let host_ptr = vm
                .host_mut_any()
                .unwrap()
                .downcast_mut::<WsvHost>()
                .unwrap() as *mut WsvHost;
            (*host_ptr)
                .syscall(syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT, &mut vm)
                .unwrap_or(0)
        };

        // 2) Try to submit a ballot; should be PermissionDenied and no ciphertexts
        let sb = iroha_data_model::isi::zk::SubmitBallot {
            election_id: "e1".to_string(),
            ciphertext: vec![1, 2, 3],
            ballot_proof: iroha_data_model::proof::ProofAttachment::new_inline(
                "halo2/ipa".into(),
                iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x01]),
                iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![0x02]),
            ),
            nullifier: [7u8; 32],
        };
        let ib_bytes = sb.encode_as_instruction_box();
        let mut tlv = Vec::with_capacity(7 + ib_bytes.len() + 32);
        tlv.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        tlv.push(1);
        tlv.extend_from_slice(&(ib_bytes.len() as u32).to_be_bytes());
        tlv.extend_from_slice(&ib_bytes);
        let hh: [u8; 32] = iroha_crypto::Hash::new(&ib_bytes).into();
        tlv.extend_from_slice(&hh);
        vm.memory.preload_input(0, &tlv).expect("preload input");
        vm.set_register(10, Memory::INPUT_START);
        let res = unsafe {
            let host_ptr = vm
                .host_mut_any()
                .unwrap()
                .downcast_mut::<WsvHost>()
                .unwrap() as *mut WsvHost;
            (*host_ptr).syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        };
        assert!(matches!(res, Err(VMError::PermissionDenied)));
        let host_ref = vm.host_mut_any().unwrap();
        let host = host_ref.downcast_ref::<WsvHost>().unwrap();
        assert_eq!(host.wsv.elections.get("e1").unwrap().ciphertexts.len(), 0);
    }

    #[test]
    fn host_submit_ballot_requires_matching_envelope_hash() {
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("gov1".to_string(), 2, [0u8; 32], 0, u64::MAX));
        wsv.set_current_time_ms(100);
        let caller: ScopedAccountId = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "domain",
        );
        let host = WsvHost::new_with_subject(wsv, AccountId::from(&caller), HashMap::new());
        let mut vm = IVM::new(0);
        vm.set_host(host);

        {
            let host_ref = vm.host_mut_any().unwrap();
            let host = host_ref.downcast_mut::<WsvHost>().unwrap();
            host.wsv.set_current_time_ms(150);
            host.__test_push_verified_ballot([0x11; 32]);
        }

        // Submit instruction without envelope hash; host should inject expected hash and succeed.
        let submit = iroha_data_model::isi::zk::SubmitBallot {
            election_id: "gov1".to_string(),
            ciphertext: vec![0xaa, 0xbb, 0xcc],
            ballot_proof: ProofAttachment::new_inline(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), vec![0x01]),
                VerifyingKeyBox::new("halo2/ipa".into(), vec![0x02]),
            ),
            nullifier: [0x09; 32],
        };
        let res = {
            let host_ref = vm.host_mut_any().unwrap();
            let host = host_ref.downcast_mut::<WsvHost>().unwrap();
            host.handle_submit_ballot(&submit)
        };
        assert!(matches!(res, Ok(0)));
        {
            let host_ref = vm.host_mut_any().unwrap();
            let host = host_ref.downcast_ref::<WsvHost>().unwrap();
            let election = host.wsv.elections.get("gov1").unwrap();
            assert_eq!(election.ciphertexts.len(), 1);
        }

        // Push verified hash but provide mismatching envelope hash in the proof; should be rejected.
        {
            let host_ref = vm.host_mut_any().unwrap();
            let host = host_ref.downcast_mut::<WsvHost>().unwrap();
            host.__test_push_verified_ballot([0x22; 32]);
        }
        let mut mismatch_proof = ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0x03]),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![0x04]),
        );
        mismatch_proof.envelope_hash = Some([0x33; 32]);
        let submit_bad = iroha_data_model::isi::zk::SubmitBallot {
            election_id: "gov1".to_string(),
            ciphertext: vec![0xdd, 0xee, 0xff],
            ballot_proof: mismatch_proof,
            nullifier: [0x10; 32],
        };
        let res_bad = {
            let host_ref = vm.host_mut_any().unwrap();
            let host = host_ref.downcast_mut::<WsvHost>().unwrap();
            host.handle_submit_ballot(&submit_bad)
        };
        assert!(matches!(res_bad, Err(VMError::PermissionDenied)));
        {
            let host_ref = vm.host_mut_any().unwrap();
            let host = host_ref.downcast_ref::<WsvHost>().unwrap();
            let election = host.wsv.elections.get("gov1").unwrap();
            assert_eq!(election.ciphertexts.len(), 1);
        }
    }

    #[test]
    fn malformed_verify_tally_keeps_latch_off_and_finalize_rejected() {
        let mut wsv = MockWorldStateView::new();
        assert!(wsv.create_election("e2".to_string(), 3, [0u8; 32], 0, u64::MAX));
        let caller: ScopedAccountId = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "domain",
        );
        let host = WsvHost::new_with_subject(wsv, AccountId::from(&caller), HashMap::new());
        let mut vm = IVM::new(0);
        vm.set_host(host);

        // Malformed tally verify
        let empty_env: Vec<u8> = Vec::new();
        let mut env_tlv = Vec::with_capacity(7 + empty_env.len() + 32);
        env_tlv.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        env_tlv.push(1);
        env_tlv.extend_from_slice(&(empty_env.len() as u32).to_be_bytes());
        env_tlv.extend_from_slice(&empty_env);
        let h: [u8; 32] = iroha_crypto::Hash::new(&empty_env).into();
        env_tlv.extend_from_slice(&h);
        vm.memory.preload_input(0, &env_tlv).expect("preload input");
        vm.set_register(10, Memory::INPUT_START);
        let _ = unsafe {
            let host_ptr = vm
                .host_mut_any()
                .unwrap()
                .downcast_mut::<WsvHost>()
                .unwrap() as *mut WsvHost;
            (*host_ptr)
                .syscall(syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY, &mut vm)
                .unwrap_or(0)
        };

        // Finalize should be rejected
        let fe = iroha_data_model::isi::zk::FinalizeElection {
            election_id: "e2".to_string(),
            tally: vec![5, 2, 1],
            tally_proof: iroha_data_model::proof::ProofAttachment::new_inline(
                "halo2/ipa".into(),
                iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x03]),
                iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![0x04]),
            ),
        };
        let ib_bytes = fe.encode_as_instruction_box();
        let mut tlv = Vec::with_capacity(7 + ib_bytes.len() + 32);
        tlv.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        tlv.push(1);
        tlv.extend_from_slice(&(ib_bytes.len() as u32).to_be_bytes());
        tlv.extend_from_slice(&ib_bytes);
        let hh: [u8; 32] = iroha_crypto::Hash::new(&ib_bytes).into();
        tlv.extend_from_slice(&hh);
        vm.memory.preload_input(0, &tlv).expect("preload input");
        vm.set_register(10, Memory::INPUT_START);
        let res = unsafe {
            let host_ptr = vm
                .host_mut_any()
                .unwrap()
                .downcast_mut::<WsvHost>()
                .unwrap() as *mut WsvHost;
            (*host_ptr).syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        };
        assert!(matches!(res, Err(VMError::PermissionDenied)));
    }
}

#[cfg(test)]
mod tests_zk_asset_bindings {
    use super::*;

    #[test]
    fn register_without_vk_allows_shield() {
        let caller: ScopedAccountId = test_account_id(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
            "domain",
        );
        let domain: DomainId = "domain".parse().unwrap();
        let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "domain".parse().unwrap(),
            "rose".parse().unwrap(),
        );
        let mut wsv = MockWorldStateView::new();
        wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
        assert!(wsv.register_domain(&caller, domain.clone()));
        wsv.add_account_unchecked(caller.clone());
        wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
        assert!(wsv.register_asset_definition(&caller, asset.clone(), Mintable::Infinitely));
        wsv.grant_permission(&caller, PermissionToken::RegisterZkAsset(asset.clone()));
        wsv.grant_permission(&caller, PermissionToken::Shield(asset.clone()));
        wsv.grant_permission(&caller, PermissionToken::MintAsset(asset.clone()));
        assert!(wsv.register_zk_asset(
            asset.clone(),
            ZkPolicyConfig {
                mode: ZkAssetMode::Hybrid,
                allow_shield: true,
                allow_unshield: true,
                vk_transfer: None,
                vk_unshield: None,
                vk_shield: None,
            },
        ));
        wsv.mint(
            &caller,
            caller.clone(),
            asset.clone(),
            Numeric::from(10_u64),
        );
        assert!(wsv.shield(&caller, &asset, Numeric::from(3_u64), [7u8; 32]));
    }
}

#[cfg(test)]
mod tests_nft_decode {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn decode_nft_payload_accepts_norito_encoded_bytes() {
        let nft_id: NftId = "n0$wonderland".parse().unwrap();
        let payload = norito::to_bytes(&nft_id).expect("encode nft id");
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            AccountId::from(&caller),
            HashMap::new(),
        );

        let decoded = host.decode_nft_payload(&payload).expect("decode ok");
        assert_eq!(decoded, nft_id);
    }
}

#[cfg(test)]
mod tests_null_decode {
    use std::collections::HashMap;

    use super::*;
    use iroha_data_model::prelude::Name;
    use iroha_primitives::json::Json;

    fn make_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
        out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
        out.push(1);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(payload);
        let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
        out.extend_from_slice(&h);
        out
    }

    fn call_syscall(vm: &mut IVM, number: u32) -> Result<u64, VMError> {
        unsafe {
            let host_ptr = vm
                .host_mut_any()
                .unwrap()
                .downcast_mut::<WsvHost>()
                .unwrap() as *mut WsvHost;
            (*host_ptr).syscall(number, vm)
        }
    }

    #[test]
    fn decode_syscalls_accept_null_pointers() {
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            AccountId::from(&caller),
            HashMap::new(),
        );
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);

        let cases = [
            syscalls::SYSCALL_DECODE_INT,
            syscalls::SYSCALL_JSON_DECODE,
            syscalls::SYSCALL_NAME_DECODE,
            syscalls::SYSCALL_POINTER_FROM_NORITO,
            syscalls::SYSCALL_INPUT_PUBLISH_TLV,
        ];

        for &number in &cases {
            vm.set_register(10, 0);
            vm.set_register(11, 0);
            call_syscall(&mut vm, number).expect("syscall should accept null");
            assert_eq!(vm.register(10), 0);
        }
    }

    #[test]
    fn input_publish_tlv_rejects_oversized_envelope() {
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            AccountId::from(&caller),
            HashMap::new(),
        )
        .with_zk_halo2_config(crate::host::ZkHalo2Config {
            max_envelope_bytes: 64,
            ..Default::default()
        });
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);

        let payload = vec![b'x'; 128];
        let ptr = vm
            .alloc_input_tlv(&make_tlv(PointerType::Json, &payload))
            .expect("alloc oversized json tlv");
        vm.set_register(10, ptr);
        let err = call_syscall(&mut vm, syscalls::SYSCALL_INPUT_PUBLISH_TLV)
            .expect_err("oversized tlv should be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
    }

    #[test]
    fn execute_instruction_rejects_oversized_json_envelope() {
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            AccountId::from(&caller),
            HashMap::new(),
        )
        .with_zk_halo2_config(crate::host::ZkHalo2Config {
            max_envelope_bytes: 96,
            ..Default::default()
        });
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);

        let oversized_env = norito::json::object([
            (
                "type",
                norito::json::to_value("wsv.create_role").expect("serialize type"),
            ),
            (
                "payload",
                norito::json::object([(
                    "name",
                    norito::json::to_value(&"a".repeat(256)).expect("serialize oversized name"),
                )])
                .expect("serialize payload"),
            ),
        ])
        .expect("serialize envelope");
        let envelope_bytes =
            norito::json::to_vec(&oversized_env).expect("serialize envelope bytes");
        let ptr = vm
            .alloc_input_tlv(&make_tlv(PointerType::Json, &envelope_bytes))
            .expect("alloc oversized envelope");
        vm.set_register(10, ptr);
        let err = call_syscall(&mut vm, syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION)
            .expect_err("oversized json envelope should be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
    }

    #[test]
    fn decode_int_accepts_norito_i64() {
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let payload = norito::to_bytes(&29_i64).expect("encode i64");
        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            AccountId::from(&caller),
            HashMap::new(),
        );
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);
        let ptr = vm
            .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, &payload))
            .expect("alloc tlv");
        vm.set_register(10, ptr);
        call_syscall(&mut vm, syscalls::SYSCALL_DECODE_INT).expect("decode int");
        assert_eq!(vm.register(10) as i64, 29);
    }

    #[test]
    fn decode_int_rejects_non_norito_i64_payloads() {
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let cases = vec![
            ("utf8-decimal", b"-41".to_vec()),
            (
                "norito-string",
                norito::to_bytes(&"-8".to_string()).expect("encode string"),
            ),
        ];

        for (label, payload) in cases {
            let host = WsvHost::new_with_subject(
                MockWorldStateView::new(),
                AccountId::from(&caller.clone()),
                HashMap::new(),
            );
            let mut vm = IVM::new(u64::MAX);
            vm.set_host(host);
            let ptr = vm
                .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, &payload))
                .expect("alloc tlv");
            vm.set_register(10, ptr);
            let err = call_syscall(&mut vm, syscalls::SYSCALL_DECODE_INT)
                .expect_err("decode_int should reject non-i64 payload");
            assert!(
                matches!(err, VMError::DecodeError),
                "decode_int payload variant {label} should yield DecodeError, got {err:?}"
            );
        }
    }

    #[test]
    fn name_decode_rejects_non_norito_payload() {
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            AccountId::from(&caller),
            HashMap::new(),
        );
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);

        let bad = b"not-norito-encoded-name";
        let ptr = vm
            .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, bad))
            .expect("alloc tlv");
        vm.set_register(10, ptr);

        let err = call_syscall(&mut vm, syscalls::SYSCALL_NAME_DECODE)
            .expect_err("expected non-Norito payload to be rejected");
        assert!(matches!(err, VMError::DecodeError));
    }

    #[test]
    fn json_decode_accepts_blob() {
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            AccountId::from(&caller),
            HashMap::new(),
        );
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);

        let json = br#"{"a":1,"b":[2,3]}"#;
        let ptr = vm
            .alloc_input_tlv(&make_tlv(PointerType::Blob, json))
            .expect("alloc tlv");
        vm.set_register(10, ptr);
        call_syscall(&mut vm, syscalls::SYSCALL_JSON_DECODE).expect("blob should be accepted");
        let out_ptr = vm.register(10);
        assert_ne!(out_ptr, 0);
        let out = vm.memory.validate_tlv(out_ptr).expect("validate tlv");
        assert_eq!(out.type_id, PointerType::Json);
    }

    #[test]
    fn schema_decode_rejects_blob() {
        let caller: ScopedAccountId = test_account_id(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "wonderland",
        );
        let host = WsvHost::new_with_subject(
            MockWorldStateView::new(),
            AccountId::from(&caller),
            HashMap::new(),
        );
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(host);

        let schema: Name = "Order".parse().expect("schema name");
        let json = Json::from_str_norito(r#"{"qty":10, "side":"buy"}"#).expect("json");
        let schema_bytes = norito::to_bytes(&schema).expect("encode schema");
        let json_bytes = norito::to_bytes(&json).expect("encode json");
        let p_schema = vm
            .alloc_input_tlv(&make_tlv(PointerType::Name, &schema_bytes))
            .expect("alloc schema");
        let p_json = vm
            .alloc_input_tlv(&make_tlv(PointerType::Json, &json_bytes))
            .expect("alloc json");
        vm.set_register(10, p_schema);
        vm.set_register(11, p_json);
        call_syscall(&mut vm, syscalls::SYSCALL_SCHEMA_ENCODE).expect("encode ok");
        let encoded = vm.memory.validate_tlv(vm.register(10)).expect("encoded");
        let p_blob = vm
            .alloc_input_tlv(&make_tlv(PointerType::Blob, encoded.payload))
            .expect("alloc blob");
        vm.set_register(10, p_schema);
        vm.set_register(11, p_blob);
        let err = call_syscall(&mut vm, syscalls::SYSCALL_SCHEMA_DECODE)
            .expect_err("blob should be rejected");
        assert!(matches!(err, VMError::NoritoInvalid));
    }
}
