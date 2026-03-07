//! Atomic cross-transaction (AXT) helper types.
//!
//! The structures defined here deliberately model only the subset of fields
//! exercised by the current host implementation. They provide a Norito-
//! compatible schema so test fixtures can round-trip through the pointer-ABI
//! TLVs exposed to the VM. As the end-to-end pipeline matures these models
//! should converge with the canonical data-model crate.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
};

use iroha_crypto::Hash;
use iroha_data_model::nexus::{
    AssetHandle as ModelAssetHandle, AxtBinding, AxtHandleFragment,
    AxtPolicyEntry as ModelAxtPolicyEntry, AxtPolicySnapshot as ModelAxtPolicySnapshot,
    AxtProofEnvelope as ModelAxtProofEnvelope, DataSpaceId, GroupBinding as ModelGroupBinding,
    HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject, LaneId,
    ProofBlob as ModelProofBlob, RemoteSpendIntent as ModelRemoteSpendIntent,
    SpendOp as ModelSpendOp, compute_descriptor_binding,
};
use norito::codec::{Decode, Encode};

use crate::error::VMError;

/// Alias for the Norito proof envelope used in AXT proof verification.
pub type AxtProofEnvelope = ModelAxtProofEnvelope;

const AMOUNT_COMMITMENT_DOMAIN_SEPARATOR: &[u8] = b"iroha.axt.amount-commitment.v1";

/// Effective handle amount resolved from the intent/proof pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedHandleAmount {
    /// Non-zero amount used for budget checks and settlement.
    pub amount: u128,
    /// Optional amount commitment retained in block fragments.
    pub amount_commitment: Option<[u8; 32]>,
}

/// Errors returned by [`resolve_handle_amount`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleAmountResolutionError {
    /// No cleartext amount was provided and no committed amount could be loaded from proof.
    MissingAmount,
    /// Cleartext and committed amounts disagree.
    Mismatch,
    /// Resolved amount is zero and therefore invalid for handle usage.
    ZeroAmount,
}

impl HandleAmountResolutionError {
    /// Convert the resolver error to the syscall-level VM error used by hosts.
    #[must_use]
    pub const fn to_vm_error(self) -> VMError {
        match self {
            Self::MissingAmount => VMError::NoritoInvalid,
            Self::Mismatch | Self::ZeroAmount => VMError::PermissionDenied,
        }
    }
}

/// Build a deterministic amount commitment used for hidden-amount fragments.
#[must_use]
pub fn derive_amount_commitment(
    dsid: DataSpaceId,
    amount: u128,
    proof_payload: Option<&[u8]>,
) -> [u8; 32] {
    let mut message = Vec::with_capacity(
        AMOUNT_COMMITMENT_DOMAIN_SEPARATOR.len()
            + core::mem::size_of::<u64>()
            + core::mem::size_of::<u128>()
            + proof_payload.map_or(0, |payload| payload.len()),
    );
    message.extend_from_slice(AMOUNT_COMMITMENT_DOMAIN_SEPARATOR);
    message.extend_from_slice(&dsid.as_u64().to_be_bytes());
    message.extend_from_slice(&amount.to_be_bytes());
    if let Some(payload) = proof_payload {
        message.extend_from_slice(payload);
    }
    Hash::new(&message).into()
}

/// Resolve an effective amount and commitment for a handle usage.
///
/// This supports both cleartext (`intent.op.amount`) and hidden modes where
/// the cleartext amount is redacted and a committed amount is carried in the
/// [`AxtProofEnvelope`].
pub fn resolve_handle_amount(
    intent: &RemoteSpendIntent,
    proof: Option<&ProofBlob>,
) -> Result<ResolvedHandleAmount, HandleAmountResolutionError> {
    let intent_amount = intent.op.amount.parse::<u128>().ok();
    let envelope =
        proof.and_then(|blob| norito::decode_from_bytes::<AxtProofEnvelope>(&blob.payload).ok());
    let committed_amount = envelope.as_ref().and_then(|env| env.committed_amount);

    let amount = match (intent_amount, committed_amount) {
        (Some(intent_amount), Some(committed_amount)) => {
            if intent_amount != committed_amount {
                return Err(HandleAmountResolutionError::Mismatch);
            }
            intent_amount
        }
        (Some(intent_amount), None) => intent_amount,
        (None, Some(committed_amount)) => committed_amount,
        (None, None) => return Err(HandleAmountResolutionError::MissingAmount),
    };
    if amount == 0 {
        return Err(HandleAmountResolutionError::ZeroAmount);
    }

    let amount_commitment = envelope
        .as_ref()
        .and_then(|proof_envelope| proof_envelope.amount_commitment)
        .or_else(|| {
            if intent_amount.is_none() || committed_amount.is_some() {
                Some(derive_amount_commitment(
                    intent.asset_dsid,
                    amount,
                    proof.map(|blob| blob.payload.as_slice()),
                ))
            } else {
                None
            }
        });

    Ok(ResolvedHandleAmount {
        amount,
        amount_commitment,
    })
}

/// Canonical descriptor for an AXT envelope.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, norito::json::Serialize, norito::json::Deserialize,
)]
pub struct AxtDescriptor {
    /// List of dataspace identifiers touched by the transaction.
    pub dsids: Vec<DataSpaceId>,
    /// Fine-grained access declarations for each DS.
    pub touches: Vec<AxtTouchSpec>,
}

impl AxtDescriptor {
    /// Start a deterministic descriptor builder.
    #[must_use]
    pub fn builder() -> AxtDescriptorBuilder {
        AxtDescriptorBuilder::default()
    }

    /// Collect the dataspace identifiers declared in the descriptor.
    #[must_use]
    pub fn dsid_set(&self) -> BTreeSet<DataSpaceId> {
        self.dsids.iter().copied().collect()
    }

    /// Locate the declared touch specification for a dataspace.
    #[must_use]
    pub fn touch_for(&self, dsid: &DataSpaceId) -> Option<&AxtTouchSpec> {
        self.touches.iter().find(|touch| &touch.dsid == dsid)
    }
}

/// Deterministic builder for [`AxtDescriptor`].
#[derive(Debug, Default)]
pub struct AxtDescriptorBuilder {
    dsids: BTreeSet<DataSpaceId>,
    touches: BTreeMap<DataSpaceId, AxtTouchSpec>,
}

impl AxtDescriptorBuilder {
    /// Declare that the descriptor touches the provided dataspace id.
    #[must_use]
    pub fn dataspace(mut self, dsid: DataSpaceId) -> Self {
        self.dsids.insert(dsid);
        self
    }

    /// Add or replace the touch specification for a dataspace.
    ///
    /// Paths are trimmed, sorted, and deduplicated for deterministic output.
    #[must_use]
    pub fn touch<R, W>(mut self, dsid: DataSpaceId, read: R, write: W) -> Self
    where
        R: IntoIterator,
        R::Item: Into<String>,
        W: IntoIterator,
        W::Item: Into<String>,
    {
        self.dsids.insert(dsid);
        self.touches.insert(
            dsid,
            AxtTouchSpec {
                dsid,
                read: canonicalize_paths(read),
                write: canonicalize_paths(write),
            },
        );
        self
    }

    /// Finalise the descriptor and validate pointer-ABI invariants.
    ///
    /// # Errors
    /// Returns an error if the descriptor is empty or contains mismatched
    /// dataspace/touch declarations.
    pub fn build(self) -> Result<AxtDescriptor, VMError> {
        let mut dsids = self.dsids;
        dsids.extend(self.touches.keys().copied());
        let descriptor = AxtDescriptor {
            dsids: dsids.into_iter().collect(),
            touches: self.touches.into_values().collect(),
        };
        validate_descriptor(&descriptor)?;
        Ok(descriptor)
    }

    /// Finalise the descriptor and compute its canonical binding.
    ///
    /// # Errors
    /// Returns an error when validation fails or the descriptor cannot be
    /// encoded to Norito bytes for hashing.
    pub fn build_with_binding(self) -> Result<(AxtDescriptor, [u8; 32]), VMError> {
        let descriptor = self.build()?;
        let binding = compute_binding(&descriptor).map_err(|_| VMError::NoritoInvalid)?;
        Ok((descriptor, binding))
    }
}

fn canonicalize_paths<I, S>(paths: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut paths: Vec<String> = paths
        .into_iter()
        .map(|p| p.into().trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Validate basic invariants of an AXT descriptor.
pub fn validate_descriptor(descriptor: &AxtDescriptor) -> Result<(), VMError> {
    if descriptor.dsids.is_empty() {
        return Err(VMError::PermissionDenied);
    }
    let dsid_set = descriptor.dsid_set();
    if dsid_set.len() != descriptor.dsids.len() {
        return Err(VMError::PermissionDenied);
    }
    let mut touch_dsids: BTreeSet<DataSpaceId> = BTreeSet::new();
    for touch in &descriptor.touches {
        if !dsid_set.contains(&touch.dsid) {
            return Err(VMError::PermissionDenied);
        }
        if !touch_dsids.insert(touch.dsid) {
            return Err(VMError::PermissionDenied);
        }
    }
    Ok(())
}

/// Compute the expiry slot after applying a wall-clock skew allowance.
#[must_use]
pub fn expiry_slot_with_skew(
    expiry_slot: u64,
    slot_length_ms: NonZeroU64,
    max_clock_skew_ms: u64,
    override_ms: Option<u32>,
) -> u64 {
    let effective_ms = override_ms
        .map(u64::from)
        .unwrap_or(max_clock_skew_ms)
        .min(max_clock_skew_ms);
    if effective_ms == 0 {
        return expiry_slot;
    }
    let slot_ms = slot_length_ms.get();
    let skew_slots = effective_ms.div_ceil(slot_ms);
    expiry_slot.saturating_add(skew_slots)
}

/// Policy hook for gating AXT touches and handle usage.
pub trait AxtPolicy: Send + Sync {
    /// Decide whether a touch manifest is allowed for the given dataspace.
    fn allow_touch(&self, dsid: DataSpaceId, manifest: &TouchManifest) -> Result<(), VMError>;

    /// Decide whether a handle usage is allowed.
    fn allow_handle(&self, usage: &HandleUsage) -> Result<(), VMError>;
}

/// Default AXT policy that allows all operations.
pub struct AllowAllAxtPolicy;

impl AxtPolicy for AllowAllAxtPolicy {
    fn allow_touch(&self, _dsid: DataSpaceId, _manifest: &TouchManifest) -> Result<(), VMError> {
        Ok(())
    }

    fn allow_handle(&self, _usage: &HandleUsage) -> Result<(), VMError> {
        Ok(())
    }
}

/// Simple policy implementation backed by an AXT policy snapshot.
#[derive(Clone, Debug)]
pub struct SnapshotAxtPolicy {
    entries: BTreeMap<DataSpaceId, ModelAxtPolicyEntry>,
    slot_length_ms: NonZeroU64,
    max_clock_skew_ms: u64,
}

impl SnapshotAxtPolicy {
    /// Construct a policy from a snapshot.
    #[must_use]
    pub fn new(snapshot: &ModelAxtPolicySnapshot) -> Self {
        Self::new_with_timing(
            snapshot,
            NonZeroU64::new(1).expect("slot length must be non-zero"),
            0,
        )
    }

    /// Construct a policy from a snapshot and explicit timing parameters.
    #[must_use]
    pub fn new_with_timing(
        snapshot: &ModelAxtPolicySnapshot,
        slot_length_ms: NonZeroU64,
        max_clock_skew_ms: u64,
    ) -> Self {
        let entries = snapshot
            .entries
            .iter()
            .map(|binding| (binding.dsid, binding.policy))
            .collect();
        Self {
            entries,
            slot_length_ms,
            max_clock_skew_ms,
        }
    }
}

impl AxtPolicy for SnapshotAxtPolicy {
    fn allow_touch(&self, _dsid: DataSpaceId, _manifest: &TouchManifest) -> Result<(), VMError> {
        Ok(())
    }

    fn allow_handle(&self, usage: &HandleUsage) -> Result<(), VMError> {
        let entry = self
            .entries
            .get(&usage.intent.asset_dsid)
            .ok_or(VMError::PermissionDenied)?;
        if entry.manifest_root.iter().all(|byte| *byte == 0) {
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
        let expiry_slot = expiry_slot_with_skew(
            usage.handle.expiry_slot,
            self.slot_length_ms,
            self.max_clock_skew_ms,
            usage.handle.max_clock_skew_ms,
        );
        if entry.current_slot > 0 && entry.current_slot > expiry_slot {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.target_lane != entry.target_lane {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.manifest_view_root.as_slice() != entry.manifest_root.as_slice() {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.handle_era < entry.min_handle_era {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.sub_nonce < entry.min_sub_nonce {
            return Err(VMError::PermissionDenied);
        }
        Ok(())
    }
}

/// Declared access set for a dataspace touched by an AXT envelope.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, norito::json::Serialize, norito::json::Deserialize,
)]
pub struct AxtTouchSpec {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Logical read-set expressed as application key prefixes.
    pub read: Vec<String>,
    /// Logical write-set expressed as application key prefixes.
    pub write: Vec<String>,
}

/// Runtime manifest supplied via `AXT_TOUCH`.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, norito::json::Serialize, norito::json::Deserialize,
)]
pub struct TouchManifest {
    /// Keys read within the dataspace during execution.
    pub read: Vec<String>,
    /// Keys written within the dataspace during execution.
    pub write: Vec<String>,
}

/// Subset of the AssetHandle ticket encoded by asset dataspace capability issuers.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct AssetHandle {
    /// Declared permissions (example values such as "transfer").
    pub scope: Vec<String>,
    /// Subject bound to the capability.
    pub subject: HandleSubject,
    /// Budget parameters controlling single-/multi-use semantics.
    pub budget: HandleBudget,
    /// Logical era counter for revocation sequencing.
    pub handle_era: u64,
    /// Per-use nonce guarding replay.
    pub sub_nonce: u64,
    /// Lane/group binding advertised by the issuer.
    pub group_binding: GroupBinding,
    /// Lane the handle is authorised to execute on.
    pub target_lane: LaneId,
    /// Poseidon-style binding of this handle to a descriptor (32 bytes).
    pub axt_binding: Vec<u8>,
    /// Dataspace manifest root observed by the issuer at handle time.
    pub manifest_view_root: Vec<u8>,
    /// Expiry slot for freshness enforcement.
    pub expiry_slot: u64,
    /// Optional wall-clock skew allowance enforced by the host.
    pub max_clock_skew_ms: Option<u32>,
}

impl AssetHandle {
    /// Returns the binding as a 32-byte array when present.
    #[must_use]
    pub fn binding_array(&self) -> Option<[u8; 32]> {
        if self.axt_binding.len() == 32 {
            let mut buf = [0u8; 32];
            buf.copy_from_slice(&self.axt_binding);
            Some(buf)
        } else {
            None
        }
    }
}

/// Capability subject metadata.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct HandleSubject {
    /// Account identifier of the spender (string form for now).
    pub account: String,
    /// Optional originating dataspace for cross-DS handles.
    pub origin_dsid: Option<DataSpaceId>,
}

/// Handle budget parameters.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct HandleBudget {
    /// Remaining allowance for the capability.
    pub remaining: u128,
    /// Optional per-use cap.
    pub per_use: Option<u128>,
}

/// Dataspace composability group binding advertised by the capability.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct GroupBinding {
    /// Domain or composability group identifier.
    pub composability_group_id: Vec<u8>,
    /// Epoch identifier linked to the handle.
    pub epoch_id: u64,
}

impl PartialOrd for GroupBinding {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GroupBinding {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.composability_group_id
            .cmp(&other.composability_group_id)
            .then_with(|| self.epoch_id.cmp(&other.epoch_id))
    }
}

/// Intent forwarded to an asset dataspace via `USE_ASSET_HANDLE`.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct RemoteSpendIntent {
    /// Target asset dataspace identifier.
    pub asset_dsid: DataSpaceId,
    /// Operation payload (e.g., transfer details) expressed as JSON-ish strings for now.
    pub op: SpendOp,
}

/// Simplified representation of spend operations.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct SpendOp {
    /// Operation kind (e.g., "transfer").
    pub kind: String,
    /// Origin account id (string form).
    pub from: String,
    /// Destination account id (string form).
    pub to: String,
    /// Amount encoded as decimal string to avoid precision issues in tests.
    pub amount: String,
}

/// Wrapper around proof artifacts provided by dataspace verifiers.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct ProofBlob {
    /// Raw proof bytes.
    pub payload: Vec<u8>,
    /// Optional expiry slot advertised by the prover.
    ///
    /// When present this slot is compared against the current AXT policy slot so
    /// hosts can reject stale proofs deterministically.
    #[norito(default)]
    pub expiry_slot: Option<u64>,
}

/// Compute the canonical descriptor binding used by asset handles.
///
/// The current implementation prefixes the descriptor bytes with a stable
/// domain separator and hashes the concatenation using the Poseidon2 sponge
/// (rate 2, capacity 1, +1 padding). This matches the normative definition
/// documented in `nexus.md`.
pub fn compute_binding(descriptor: &AxtDescriptor) -> Result<[u8; 32], norito::Error> {
    let model_descriptor = iroha_data_model::nexus::AxtDescriptor {
        dsids: descriptor.dsids.clone(),
        touches: descriptor
            .touches
            .iter()
            .map(|touch| iroha_data_model::nexus::AxtTouchSpec {
                dsid: touch.dsid,
                read: touch.read.clone(),
                write: touch.write.clone(),
            })
            .collect(),
    };
    compute_descriptor_binding(&model_descriptor)
}

/// Shared helper used by hosts to track in-flight AXT state.
#[derive(Clone, Debug)]
pub struct HostAxtState {
    descriptor: AxtDescriptor,
    binding: [u8; 32],
    expected_dsids: BTreeSet<DataSpaceId>,
    touches: BTreeMap<DataSpaceId, TouchManifest>,
    proofs: BTreeMap<DataSpaceId, ProofBlob>,
    handles: Vec<HandleUsage>,
    handle_fragments: Vec<AxtHandleFragment>,
}

impl HostAxtState {
    #[must_use]
    pub fn new(descriptor: AxtDescriptor, binding: [u8; 32]) -> Self {
        let expected_dsids = descriptor.dsid_set();
        Self {
            descriptor,
            binding,
            expected_dsids,
            touches: BTreeMap::new(),
            proofs: BTreeMap::new(),
            handles: Vec::new(),
            handle_fragments: Vec::new(),
        }
    }

    #[must_use]
    pub fn binding(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub fn descriptor(&self) -> &AxtDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub fn expected_dsids(&self) -> &BTreeSet<DataSpaceId> {
        &self.expected_dsids
    }

    pub fn record_touch(
        &mut self,
        dsid: DataSpaceId,
        manifest: TouchManifest,
    ) -> Result<(), VMError> {
        if self.touches.contains_key(&dsid) {
            return Err(VMError::PermissionDenied);
        }
        if !self.expected_dsids.contains(&dsid) {
            return Err(VMError::PermissionDenied);
        }
        if let Some(spec) = self.descriptor.touch_for(&dsid) {
            if !manifest
                .read
                .iter()
                .all(|entry| spec.read.iter().any(|prefix| entry.starts_with(prefix)))
            {
                return Err(VMError::PermissionDenied);
            }
            if !manifest
                .write
                .iter()
                .all(|entry| spec.write.iter().any(|prefix| entry.starts_with(prefix)))
            {
                return Err(VMError::PermissionDenied);
            }
        } else if !manifest.read.is_empty() || !manifest.write.is_empty() {
            return Err(VMError::PermissionDenied);
        }
        self.touches.insert(dsid, manifest);
        Ok(())
    }

    #[must_use]
    pub fn has_touch(&self, dsid: &DataSpaceId) -> bool {
        self.touches.contains_key(dsid)
    }

    pub fn record_proof(
        &mut self,
        dsid: DataSpaceId,
        proof: Option<ProofBlob>,
        current_slot: Option<u64>,
    ) -> Result<(), VMError> {
        if !self.expected_dsids.contains(&dsid) {
            return Err(VMError::PermissionDenied);
        }
        if let Some(p) = proof {
            if p.payload.is_empty() {
                return Err(VMError::NoritoInvalid);
            }
            if let Some(expiry) = p.expiry_slot {
                if expiry == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                if let Some(slot) = current_slot
                    && slot > 0
                    && slot > expiry
                {
                    return Err(VMError::PermissionDenied);
                }
            }
            self.proofs.insert(dsid, p);
        } else {
            self.proofs.remove(&dsid);
        }
        Ok(())
    }

    pub fn record_handle(&mut self, usage: HandleUsage) -> Result<(), VMError> {
        if usage.amount == 0 {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.scope.is_empty() {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.budget.remaining == 0 {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.handle_era == 0 || usage.handle.sub_nonce == 0 {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.expiry_slot == 0 {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.group_binding.composability_group_id.is_empty() {
            return Err(VMError::NoritoInvalid);
        }
        if usage.handle.manifest_view_root.len() != 32 {
            return Err(VMError::NoritoInvalid);
        }
        if !self.expected_dsids.contains(&usage.intent.asset_dsid) {
            return Err(VMError::PermissionDenied);
        }
        if !self.touches.contains_key(&usage.intent.asset_dsid) {
            return Err(VMError::PermissionDenied);
        }
        if usage
            .handle
            .scope
            .iter()
            .all(|scope| scope != &usage.intent.op.kind)
        {
            return Err(VMError::PermissionDenied);
        }
        if usage.intent.op.from != usage.handle.subject.account {
            return Err(VMError::PermissionDenied);
        }
        let binding = usage.handle.binding_array().ok_or(VMError::NoritoInvalid)?;
        if binding != self.binding {
            return Err(VMError::PermissionDenied);
        }
        // Ensure replay protection per handle/sub-nonce combination before budget checks.
        if self.handles.iter().any(|prev| {
            prev.handle.handle_era == usage.handle.handle_era
                && prev
                    .handle
                    .binding_array()
                    .is_some_and(|prev_binding| prev_binding == binding)
                && prev.handle.target_lane == usage.handle.target_lane
                && usage.handle.sub_nonce == prev.handle.sub_nonce
        }) {
            return Err(VMError::PermissionDenied);
        }
        if usage.amount > usage.handle.budget.remaining {
            return Err(VMError::PermissionDenied);
        }
        if let Some(per_use) = usage.handle.budget.per_use
            && usage.amount > per_use
        {
            return Err(VMError::PermissionDenied);
        }
        if let Some(proof) = &usage.proof
            && proof.payload.is_empty()
        {
            return Err(VMError::NoritoInvalid);
        }
        let fragment = AxtHandleFragment::try_from(&usage)?;
        self.handles.push(usage);
        self.handle_fragments.push(fragment);
        Ok(())
    }

    #[must_use]
    pub fn touches(&self) -> &BTreeMap<DataSpaceId, TouchManifest> {
        &self.touches
    }

    #[must_use]
    pub fn proofs(&self) -> &BTreeMap<DataSpaceId, ProofBlob> {
        &self.proofs
    }

    #[must_use]
    pub fn handles(&self) -> &[HandleUsage] {
        &self.handles
    }

    #[must_use]
    /// Return handle fragments recorded from accepted handle usages.
    pub fn handle_fragments(&self) -> &[AxtHandleFragment] {
        &self.handle_fragments
    }

    pub fn validate_commit(&self) -> Result<(), VMError> {
        struct HandleAccumulator {
            key: HandleBudgetKey,
            total: u128,
            per_dsid: BTreeMap<DataSpaceId, u128>,
        }

        impl HandleAccumulator {
            fn new(key: HandleBudgetKey) -> Self {
                Self {
                    key,
                    total: 0,
                    per_dsid: BTreeMap::new(),
                }
            }
        }

        for dsid in &self.expected_dsids {
            if self.descriptor.touch_for(dsid).is_some() && !self.touches.contains_key(dsid) {
                return Err(VMError::PermissionDenied);
            }
        }
        let mut seen_nonces: BTreeSet<([u8; 32], u64, LaneId, u64)> = BTreeSet::new();
        let mut accumulators: Vec<HandleAccumulator> = Vec::new();
        for usage in &self.handles {
            let binding = usage.handle.binding_array().ok_or(VMError::NoritoInvalid)?;
            let key = (
                binding,
                usage.handle.handle_era,
                usage.handle.target_lane,
                usage.handle.sub_nonce,
            );
            if !seen_nonces.insert(key) {
                return Err(VMError::PermissionDenied);
            }
            if usage.amount > usage.handle.budget.remaining {
                return Err(VMError::PermissionDenied);
            }
            if let Some(proof) = usage
                .proof
                .as_ref()
                .or_else(|| self.proofs.get(&usage.intent.asset_dsid))
                && let Some(expiry_slot) = proof.expiry_slot
                && (expiry_slot == 0 || usage.handle.expiry_slot > expiry_slot)
            {
                return Err(VMError::PermissionDenied);
            }
            if usage.proof.is_none() && !self.proofs.contains_key(&usage.intent.asset_dsid) {
                return Err(VMError::PermissionDenied);
            }

            let budget_key = HandleBudgetKey::try_from(&usage.handle)?;
            let accumulator = match accumulators.iter().position(|acc| acc.key == budget_key) {
                Some(existing) => &mut accumulators[existing],
                None => {
                    accumulators.push(HandleAccumulator::new(budget_key));
                    accumulators
                        .last_mut()
                        .expect("accumulator was just pushed")
                }
            };

            accumulator.total = accumulator
                .total
                .checked_add(usage.amount)
                .ok_or(VMError::PermissionDenied)?;

            let ds_total = accumulator
                .per_dsid
                .entry(usage.intent.asset_dsid)
                .or_insert(0);
            *ds_total = ds_total
                .checked_add(usage.amount)
                .ok_or(VMError::PermissionDenied)?;

            if accumulator.total > accumulator.key.budget_remaining {
                return Err(VMError::PermissionDenied);
            }
            if let Some(per_use) = accumulator.key.budget_per_use
                && *ds_total > per_use
            {
                return Err(VMError::PermissionDenied);
            }
        }

        for accumulator in &accumulators {
            if accumulator.total > accumulator.key.budget_remaining {
                return Err(VMError::PermissionDenied);
            }
            if let Some(per_use) = accumulator.key.budget_per_use
                && accumulator.per_dsid.values().any(|total| *total > per_use)
            {
                return Err(VMError::PermissionDenied);
            }
        }
        let mut dataspace_proofs_present: BTreeSet<DataSpaceId> =
            self.proofs.keys().copied().collect();
        for usage in &self.handles {
            if usage.proof.is_some() {
                dataspace_proofs_present.insert(usage.intent.asset_dsid);
            }
        }
        for dsid in &self.expected_dsids {
            if !dataspace_proofs_present.contains(dsid) {
                return Err(VMError::PermissionDenied);
            }
        }
        Ok(())
    }
}

/// Recorded handle usage for commit validation.
#[derive(Debug, Clone)]
pub struct HandleUsage {
    pub handle: AssetHandle,
    pub intent: RemoteSpendIntent,
    pub proof: Option<ProofBlob>,
    pub amount: u128,
    pub amount_commitment: Option<[u8; 32]>,
}

impl TryFrom<&HandleUsage> for AxtHandleFragment {
    type Error = VMError;

    fn try_from(usage: &HandleUsage) -> Result<Self, Self::Error> {
        let binding = usage.handle.binding_array().ok_or(VMError::NoritoInvalid)?;
        let manifest_view_root = manifest_root_array(&usage.handle)?;
        let handle = ModelAssetHandle {
            scope: usage.handle.scope.clone(),
            subject: ModelHandleSubject {
                account: usage.handle.subject.account.clone(),
                origin_dsid: usage.handle.subject.origin_dsid,
            },
            budget: ModelHandleBudget {
                remaining: usage.handle.budget.remaining,
                per_use: usage.handle.budget.per_use,
            },
            handle_era: usage.handle.handle_era,
            sub_nonce: usage.handle.sub_nonce,
            group_binding: ModelGroupBinding {
                composability_group_id: usage.handle.group_binding.composability_group_id.clone(),
                epoch_id: usage.handle.group_binding.epoch_id,
            },
            target_lane: usage.handle.target_lane,
            axt_binding: AxtBinding::new(binding),
            manifest_view_root,
            expiry_slot: usage.handle.expiry_slot,
            max_clock_skew_ms: usage.handle.max_clock_skew_ms,
        };
        let intent = ModelRemoteSpendIntent {
            asset_dsid: usage.intent.asset_dsid,
            op: ModelSpendOp {
                kind: usage.intent.op.kind.clone(),
                from: usage.intent.op.from.clone(),
                to: usage.intent.op.to.clone(),
                amount: usage.intent.op.amount.clone(),
            },
        };
        let proof = usage.proof.as_ref().map(|p| ModelProofBlob {
            payload: p.payload.clone(),
            expiry_slot: p.expiry_slot,
        });
        let amount_hidden = usage.intent.op.amount.parse::<u128>().is_err();
        let amount_commitment = if amount_hidden {
            usage.amount_commitment.or_else(|| {
                Some(derive_amount_commitment(
                    usage.intent.asset_dsid,
                    usage.amount,
                    usage.proof.as_ref().map(|blob| blob.payload.as_slice()),
                ))
            })
        } else {
            usage.amount_commitment
        };
        Ok(AxtHandleFragment {
            handle,
            intent,
            proof,
            amount: if amount_hidden { 0 } else { usage.amount },
            amount_commitment,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct HandleBudgetKey {
    binding: [u8; 32],
    handle_era: u64,
    target_lane: u32,
    manifest_root: [u8; 32],
    scope: Vec<String>,
    subject_account: String,
    subject_origin: Option<u64>,
    group_binding: GroupBinding,
    expiry_slot: u64,
    budget_remaining: u128,
    budget_per_use: Option<u128>,
    max_clock_skew_ms: Option<u32>,
}

impl TryFrom<&AssetHandle> for HandleBudgetKey {
    type Error = VMError;

    fn try_from(handle: &AssetHandle) -> Result<Self, Self::Error> {
        let manifest_root = manifest_root_array(handle)?;
        let binding = handle.binding_array().ok_or(VMError::NoritoInvalid)?;
        Ok(Self {
            binding,
            handle_era: handle.handle_era,
            target_lane: handle.target_lane.as_u32(),
            manifest_root,
            scope: handle.scope.clone(),
            subject_account: handle.subject.account.clone(),
            subject_origin: handle.subject.origin_dsid.map(DataSpaceId::as_u64),
            group_binding: handle.group_binding.clone(),
            expiry_slot: handle.expiry_slot,
            budget_remaining: handle.budget.remaining,
            budget_per_use: handle.budget.per_use,
            max_clock_skew_ms: handle.max_clock_skew_ms,
        })
    }
}

fn manifest_root_array(handle: &AssetHandle) -> Result<[u8; 32], VMError> {
    if handle.manifest_view_root.len() != 32 {
        return Err(VMError::NoritoInvalid);
    }
    let mut manifest_root = [0u8; 32];
    manifest_root.copy_from_slice(&handle.manifest_view_root);
    Ok(manifest_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT_FROM_LITERAL: &str = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
    const ACCOUNT_TO_LITERAL: &str = "6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU";

    #[test]
    fn expiry_slot_with_skew_respects_caps() {
        let slot = expiry_slot_with_skew(10, NonZeroU64::new(10).expect("slot length"), 5, Some(7));
        assert_eq!(slot, 11, "7ms skew rounds up into 1 slot of 10ms");

        let clamped =
            expiry_slot_with_skew(20, NonZeroU64::new(10).expect("slot length"), 5, Some(50));
        assert_eq!(
            clamped, 21,
            "override above config should clamp to config max"
        );

        let zero =
            expiry_slot_with_skew(5, NonZeroU64::new(10).expect("slot length"), 0, Some(100));
        assert_eq!(zero, 5, "zero skew leaves expiry unchanged");
    }

    #[test]
    fn binding_is_stable_for_descriptor() {
        let descriptor = AxtDescriptor {
            dsids: vec![DataSpaceId::new(1), DataSpaceId::new(2)],
            touches: vec![AxtTouchSpec {
                dsid: DataSpaceId::new(1),
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let first = compute_binding(&descriptor).expect("binding");
        let second = compute_binding(&descriptor).expect("binding");
        assert_eq!(first, second);
    }

    fn sample_touch_manifest() -> TouchManifest {
        TouchManifest {
            read: vec!["orders/item".into()],
            write: vec!["ledger/item".into()],
        }
    }

    fn sample_handle(
        dsid: DataSpaceId,
        binding: [u8; 32],
        remaining: u128,
        per_use: Option<u128>,
    ) -> AssetHandle {
        AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: ACCOUNT_FROM_LITERAL.into(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget { remaining, per_use },
            handle_era: 1,
            sub_nonce: 7,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 10,
            },
            target_lane: LaneId::new(0),
            axt_binding: binding.to_vec(),
            manifest_view_root: vec![1; 32],
            expiry_slot: 99,
            max_clock_skew_ms: Some(0),
        }
    }

    fn sample_intent(dsid: DataSpaceId, amount: &str) -> RemoteSpendIntent {
        RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: ACCOUNT_FROM_LITERAL.into(),
                to: ACCOUNT_TO_LITERAL.into(),
                amount: amount.into(),
            },
        }
    }

    fn proof_with_amount(
        dsid: DataSpaceId,
        committed_amount: Option<u128>,
        amount_commitment: Option<[u8; 32]>,
    ) -> ProofBlob {
        let payload = norito::to_bytes(&AxtProofEnvelope {
            dsid,
            manifest_root: [0xAB; 32],
            da_commitment: None,
            proof: vec![0x01, 0x02],
            committed_amount,
            amount_commitment,
        })
        .expect("encode proof envelope");
        ProofBlob {
            payload,
            expiry_slot: Some(10),
        }
    }

    #[test]
    fn resolve_handle_amount_accepts_cleartext_intent() {
        let dsid = DataSpaceId::new(90);
        let intent = sample_intent(dsid, "42");
        let resolved = resolve_handle_amount(&intent, None).expect("resolve amount");
        assert_eq!(resolved.amount, 42);
        assert_eq!(resolved.amount_commitment, None);
    }

    #[test]
    fn resolve_handle_amount_uses_proof_commit_when_intent_hidden() {
        let dsid = DataSpaceId::new(91);
        let intent = sample_intent(dsid, "hidden");
        let proof = proof_with_amount(dsid, Some(77), None);
        let resolved = resolve_handle_amount(&intent, Some(&proof)).expect("resolve amount");
        assert_eq!(resolved.amount, 77);
        assert_eq!(
            resolved.amount_commitment,
            Some(derive_amount_commitment(
                dsid,
                77,
                Some(proof.payload.as_slice())
            ))
        );
    }

    #[test]
    fn resolve_handle_amount_rejects_intent_proof_mismatch() {
        let dsid = DataSpaceId::new(92);
        let intent = sample_intent(dsid, "11");
        let proof = proof_with_amount(dsid, Some(12), None);
        assert_eq!(
            resolve_handle_amount(&intent, Some(&proof)),
            Err(HandleAmountResolutionError::Mismatch)
        );
    }

    #[test]
    fn try_from_usage_redacts_hidden_amount() {
        let dsid = DataSpaceId::new(93);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let intent = sample_intent(dsid, "hidden");
        let proof = proof_with_amount(dsid, Some(5), None);
        let resolved = resolve_handle_amount(&intent, Some(&proof)).expect("resolve amount");
        let usage = HandleUsage {
            handle: sample_handle(dsid, binding, 10, Some(10)),
            intent,
            proof: Some(proof),
            amount: resolved.amount,
            amount_commitment: resolved.amount_commitment,
        };
        let fragment = AxtHandleFragment::try_from(&usage).expect("fragment conversion");
        assert_eq!(fragment.amount, 0);
        assert_eq!(fragment.amount_commitment, resolved.amount_commitment);
    }

    #[test]
    fn snapshot_policy_rejects_excess_skew_request() {
        let dsid = DataSpaceId::new(8);
        let entry = ModelAxtPolicyEntry {
            manifest_root: [0xAB; 32],
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 10,
        };
        let entries = vec![iroha_data_model::nexus::AxtPolicyBinding {
            dsid,
            policy: entry,
        }];
        let snapshot = ModelAxtPolicySnapshot {
            version: ModelAxtPolicySnapshot::compute_version(&entries),
            entries,
        };
        let policy = SnapshotAxtPolicy::new_with_timing(
            &snapshot,
            NonZeroU64::new(10).expect("slot length"),
            5,
        );

        let binding = [0x11; 32];
        let mut handle = sample_handle(dsid, binding, 25, None);
        handle.manifest_view_root = entry.manifest_root.to_vec();
        handle.target_lane = entry.target_lane;
        handle.handle_era = entry.min_handle_era;
        handle.sub_nonce = entry.min_sub_nonce;
        handle.max_clock_skew_ms = Some(6);
        let intent = sample_intent(dsid, "1");
        let usage = HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 1,
            amount_commitment: None,
        };

        assert!(matches!(
            policy.allow_handle(&usage),
            Err(VMError::PermissionDenied)
        ));
    }

    #[test]
    fn commit_rejects_cumulative_budget_overspend() {
        let dsid = DataSpaceId::new(5);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch matches descriptor");

        let mut handle = sample_handle(dsid, binding, 100, None);
        let proof = Some(ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        });

        state
            .record_handle(HandleUsage {
                handle: handle.clone(),
                intent: sample_intent(dsid, "60"),
                proof: proof.clone(),
                amount: 60,
                amount_commitment: None,
            })
            .expect("first usage within budget");
        handle.sub_nonce = handle.sub_nonce.saturating_add(1);
        state
            .record_handle(HandleUsage {
                handle,
                intent: sample_intent(dsid, "50"),
                proof,
                amount: 50,
                amount_commitment: None,
            })
            .expect("second usage tracked");

        assert!(matches!(
            state.validate_commit(),
            Err(VMError::PermissionDenied)
        ));
    }

    #[test]
    fn commit_rejects_budget_overspend_across_sub_nonces() {
        let dsid = DataSpaceId::new(6);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch matches descriptor");

        let mut handle = sample_handle(dsid, binding, 100, None);
        let proof = Some(ProofBlob {
            payload: vec![9],
            expiry_slot: None,
        });

        state
            .record_handle(HandleUsage {
                handle: handle.clone(),
                intent: sample_intent(dsid, "60"),
                proof: proof.clone(),
                amount: 60,
                amount_commitment: None,
            })
            .expect("first usage within budget");
        handle.sub_nonce = handle.sub_nonce.saturating_add(1);
        state
            .record_handle(HandleUsage {
                handle,
                intent: sample_intent(dsid, "60"),
                proof,
                amount: 60,
                amount_commitment: None,
            })
            .expect("second usage recorded for different sub-nonce");

        assert!(matches!(
            state.validate_commit(),
            Err(VMError::PermissionDenied)
        ));
    }

    #[test]
    fn commit_rejects_per_use_overspend_per_dataspace() {
        let dsid = DataSpaceId::new(7);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch matches descriptor");

        let mut handle = sample_handle(dsid, binding, 200, Some(70));
        let proof = Some(ProofBlob {
            payload: vec![2],
            expiry_slot: None,
        });

        state
            .record_handle(HandleUsage {
                handle: handle.clone(),
                intent: sample_intent(dsid, "50"),
                proof: proof.clone(),
                amount: 50,
                amount_commitment: None,
            })
            .expect("first usage within budget");
        handle.sub_nonce = handle.sub_nonce.saturating_add(1);
        state
            .record_handle(HandleUsage {
                handle,
                intent: sample_intent(dsid, "50"),
                proof,
                amount: 50,
                amount_commitment: None,
            })
            .expect("second usage within budget");

        assert!(matches!(
            state.validate_commit(),
            Err(VMError::PermissionDenied)
        ));
    }

    #[test]
    fn record_handle_rejects_replay_same_sub_nonce() {
        let dsid = DataSpaceId::new(8);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch recorded");

        let handle = sample_handle(dsid, binding, 50, None);
        let intent = sample_intent(dsid, "10");
        let usage = HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 10,
            amount_commitment: None,
        };

        state
            .record_handle(usage.clone())
            .expect("first usage accepted");
        let err = state
            .record_handle(usage)
            .expect_err("duplicate sub-nonce must be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
    }

    #[test]
    fn record_handle_allows_out_of_order_sub_nonce() {
        let dsid = DataSpaceId::new(11);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch recorded");

        let mut handle_high = sample_handle(dsid, binding, 100, None);
        handle_high.sub_nonce = 2;
        let mut handle_low = handle_high.clone();
        handle_low.sub_nonce = 1;

        let proof = Some(ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        });
        state
            .record_handle(HandleUsage {
                handle: handle_high,
                intent: sample_intent(dsid, "10"),
                proof: proof.clone(),
                amount: 10,
                amount_commitment: None,
            })
            .expect("first usage accepted");
        state
            .record_handle(HandleUsage {
                handle: handle_low,
                intent: sample_intent(dsid, "10"),
                proof,
                amount: 10,
                amount_commitment: None,
            })
            .expect("second usage accepted");

        assert!(matches!(state.validate_commit(), Ok(())));
    }

    #[test]
    fn record_handle_allows_same_sub_nonce_across_lanes() {
        let ds_a = DataSpaceId::new(8);
        let ds_b = DataSpaceId::new(9);
        let descriptor = AxtDescriptor {
            dsids: vec![ds_a, ds_b],
            touches: vec![
                AxtTouchSpec {
                    dsid: ds_a,
                    read: vec!["orders".into()],
                    write: vec!["ledger".into()],
                },
                AxtTouchSpec {
                    dsid: ds_b,
                    read: vec!["orders".into()],
                    write: vec!["ledger".into()],
                },
            ],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(ds_a, sample_touch_manifest())
            .expect("touch recorded");
        state
            .record_touch(ds_b, sample_touch_manifest())
            .expect("touch recorded");

        let mut handle_a = sample_handle(ds_a, binding, 50, None);
        handle_a.target_lane = LaneId::new(1);
        let mut handle_b = sample_handle(ds_b, binding, 50, None);
        handle_b.target_lane = LaneId::new(2);
        handle_b.sub_nonce = handle_a.sub_nonce;

        let proof = Some(ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        });
        state
            .record_handle(HandleUsage {
                handle: handle_a,
                intent: sample_intent(ds_a, "10"),
                proof: proof.clone(),
                amount: 10,
                amount_commitment: None,
            })
            .expect("first usage accepted");
        state
            .record_handle(HandleUsage {
                handle: handle_b,
                intent: sample_intent(ds_b, "10"),
                proof,
                amount: 10,
                amount_commitment: None,
            })
            .expect("second usage accepted for different lane");

        assert!(matches!(state.validate_commit(), Ok(())));
    }

    #[test]
    fn record_handle_rejects_zero_era_or_sub_nonce() {
        let dsid = DataSpaceId::new(10);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch recorded");

        let base_handle = sample_handle(dsid, binding, 50, None);
        let intent = sample_intent(dsid, "10");

        let mut zero_era = base_handle.clone();
        zero_era.handle_era = 0;
        let err = state
            .record_handle(HandleUsage {
                handle: zero_era,
                intent: intent.clone(),
                proof: None,
                amount: 10,
                amount_commitment: None,
            })
            .expect_err("zero handle era must be rejected");
        assert!(matches!(err, VMError::PermissionDenied));

        let mut zero_nonce = base_handle;
        zero_nonce.sub_nonce = 0;
        let err = state
            .record_handle(HandleUsage {
                handle: zero_nonce,
                intent,
                proof: None,
                amount: 10,
                amount_commitment: None,
            })
            .expect_err("zero sub-nonce must be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
    }

    #[test]
    fn record_handle_populates_handle_fragments() {
        let dsid = DataSpaceId::new(7);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch matches descriptor");

        let handle = sample_handle(dsid, binding, 10, None);
        let usage = HandleUsage {
            handle,
            intent: sample_intent(dsid, "5"),
            proof: None,
            amount: 5,
            amount_commitment: None,
        };
        state.record_handle(usage).expect("handle usage recorded");

        let fragment = state
            .handle_fragments()
            .first()
            .expect("handle fragment recorded");
        assert_eq!(fragment.handle.axt_binding.as_bytes(), &binding);
        assert_eq!(fragment.intent.asset_dsid, dsid);
        assert_eq!(fragment.amount, 5);
    }

    #[test]
    fn record_proof_rejects_expired_slot() {
        let dsid = DataSpaceId::new(8);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch recorded");

        let proof = ProofBlob {
            payload: vec![0xA5],
            expiry_slot: Some(5),
        };
        let err = state
            .record_proof(dsid, Some(proof), Some(10))
            .expect_err("expired proof should be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
    }

    #[test]
    fn commit_rejects_proof_expiry_before_handle_expiry() {
        let dsid = DataSpaceId::new(9);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch recorded");
        state
            .record_proof(
                dsid,
                Some(ProofBlob {
                    payload: vec![0xBB],
                    expiry_slot: Some(50),
                }),
                Some(1),
            )
            .expect("proof accepted for current slot");

        let handle = sample_handle(dsid, binding, 100, None);
        state
            .record_handle(HandleUsage {
                handle: AssetHandle {
                    expiry_slot: 60,
                    ..handle
                },
                intent: sample_intent(dsid, "10"),
                proof: None,
                amount: 10,
                amount_commitment: None,
            })
            .expect("handle recorded");

        assert!(matches!(
            state.validate_commit(),
            Err(VMError::PermissionDenied)
        ));
    }

    #[test]
    fn commit_rejects_replayed_sub_nonce_for_same_binding() {
        let dsid = DataSpaceId::new(12);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        };
        let binding = compute_binding(&descriptor).expect("binding");
        let mut state = HostAxtState::new(descriptor, binding);
        state
            .record_touch(dsid, sample_touch_manifest())
            .expect("touch recorded");

        let handle = sample_handle(dsid, binding, 200, Some(200));
        let proof = Some(ProofBlob {
            payload: vec![0xA5],
            expiry_slot: None,
        });

        let usage = HandleUsage {
            handle: handle.clone(),
            intent: sample_intent(dsid, "25"),
            proof: proof.clone(),
            amount: 25,
            amount_commitment: None,
        };
        state
            .record_handle(usage.clone())
            .expect("first usage recorded");
        // Simulate a replayed handle injected from an external source (e.g., snapshot).
        state.handles.push(usage);

        assert!(matches!(
            state.validate_commit(),
            Err(VMError::PermissionDenied)
        ));
    }
}
