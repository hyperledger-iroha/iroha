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

use iroha_crypto::{Hash, Signature};
use iroha_data_model::nexus::{
    AssetHandle as ModelAssetHandle, AxtBinding, AxtDescriptor as ModelAxtDescriptor,
    AxtHandleFragment, AxtHandleIssuerContextV1, AxtPolicyEntry as ModelAxtPolicyEntry,
    AxtPolicySnapshot as ModelAxtPolicySnapshot,
    AxtPolicySnapshotValidationError as ModelAxtPolicySnapshotValidationError,
    AxtProofEnvelope as ModelAxtProofEnvelope, AxtTouchSpec as ModelAxtTouchSpec, DataSpaceId,
    GroupBinding as ModelGroupBinding, HandleBudget as ModelHandleBudget,
    HandleSubject as ModelHandleSubject, LaneId, ProofBlob as ModelProofBlob,
    RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
    TouchManifest as ModelTouchManifest, compute_descriptor_binding,
    validate_descriptor as validate_model_descriptor,
};
use iroha_data_model::prelude::Quantity;
use norito::codec::{Decode, Encode};

use crate::{
    codec::{decode_canonical_norito, encode_canonical_norito},
    error::VMError,
};

/// Alias for the Norito proof envelope used in AXT proof verification.
pub type AxtProofEnvelope = ModelAxtProofEnvelope;

const AMOUNT_COMMITMENT_DOMAIN_SEPARATOR: &[u8] = b"iroha.axt.amount-commitment.v1";

/// Effective handle amount resolved from the intent/proof pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedHandleAmount {
    /// Non-zero amount used for budget checks and settlement.
    pub amount: Quantity,
    /// Optional amount commitment retained in block fragments.
    pub amount_commitment: Option<[u8; 32]>,
}

/// Errors returned by [`resolve_handle_amount`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleAmountResolutionError {
    /// No cleartext amount was provided and no committed amount could be loaded from proof.
    MissingAmount,
    /// The supplied proof payload is not a canonical AXT proof envelope.
    InvalidProofEnvelope,
    /// Cleartext and committed amounts disagree.
    Mismatch,
    /// Resolved amount is zero and therefore invalid for handle usage.
    ZeroAmount,
    /// A business quantity cannot be represented exactly by the V1 proof scalar.
    InvalidProofScalar,
    /// Proof-supplied amount commitment does not bind the canonical proof statement.
    CommitmentMismatch,
}

impl HandleAmountResolutionError {
    /// Convert the resolver error to the syscall-level VM error used by hosts.
    #[must_use]
    pub const fn to_vm_error(self) -> VMError {
        match self {
            Self::MissingAmount | Self::InvalidProofEnvelope => VMError::NoritoInvalid,
            Self::Mismatch
            | Self::ZeroAmount
            | Self::InvalidProofScalar
            | Self::CommitmentMismatch => VMError::PermissionDenied,
        }
    }
}

fn quantity_to_proof_scalar(amount: &Quantity) -> Result<u128, HandleAmountResolutionError> {
    if amount.scale() != 0 {
        return Err(HandleAmountResolutionError::InvalidProofScalar);
    }
    amount
        .as_numeric()
        .try_mantissa_u128()
        .ok_or(HandleAmountResolutionError::InvalidProofScalar)
}

fn proof_scalar_to_quantity(amount: u128) -> Quantity {
    amount
        .to_string()
        .parse()
        .expect("every u128 is an exact scale-zero Quantity")
}

/// Build a deterministic amount commitment used for hidden-amount fragments.
///
/// Canonical AXT proof envelopes are hashed with their `amount_commitment`
/// field cleared. This makes the commitment non-circular and lets validators
/// recompute it from an envelope that already carries the claimed value.
#[must_use]
pub fn derive_amount_commitment(
    dsid: DataSpaceId,
    amount: &Quantity,
    proof_payload: Option<&[u8]>,
) -> [u8; 32] {
    let normalized_proof_payload = proof_payload.map(|payload| {
        decode_canonical_norito::<AxtProofEnvelope>(payload).map_or_else(
            |_| payload.to_vec(),
            |mut envelope| {
                envelope.amount_commitment = None;
                encode_canonical_norito(&envelope)
                    .expect("a decoded canonical AXT proof envelope always re-encodes")
            },
        )
    });
    let proof_payload = normalized_proof_payload.as_deref();
    let amount_text = amount.to_string();
    let amount_len =
        u16::try_from(amount_text.len()).expect("bounded Quantity text length always fits in u16");
    let mut message = Vec::with_capacity(
        AMOUNT_COMMITMENT_DOMAIN_SEPARATOR.len()
            + core::mem::size_of::<u64>()
            + core::mem::size_of::<u16>()
            + amount_text.len()
            + proof_payload.map_or(0, |payload| payload.len()),
    );
    message.extend_from_slice(AMOUNT_COMMITMENT_DOMAIN_SEPARATOR);
    message.extend_from_slice(&dsid.as_u64().to_be_bytes());
    message.extend_from_slice(&amount_len.to_be_bytes());
    message.extend_from_slice(amount_text.as_bytes());
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
    resolve_handle_amount_components(
        intent.asset_dsid,
        intent.op.amount.as_ref(),
        proof.map(|blob| blob.payload.as_slice()),
    )
}

/// Resolve an effective amount from the canonical model components shared by
/// VM-host and block-admission validation.
///
/// Keeping this conversion in one place prevents the two consensus-critical
/// validation layers from disagreeing about fractional quantities, proof
/// scalar bounds, hidden amounts, or commitment derivation.
///
/// # Errors
///
/// Returns [`HandleAmountResolutionError`] when the amount is absent, zero,
/// inconsistent with the proof statement, or cannot be represented exactly by
/// the V1 proof scalar.
pub fn resolve_handle_amount_components(
    asset_dsid: DataSpaceId,
    intent_amount: Option<&Quantity>,
    proof_payload: Option<&[u8]>,
) -> Result<ResolvedHandleAmount, HandleAmountResolutionError> {
    let envelope = proof_payload
        .map(|payload| {
            decode_canonical_norito::<AxtProofEnvelope>(payload)
                .map_err(|_| HandleAmountResolutionError::InvalidProofEnvelope)
        })
        .transpose()?;
    let committed_amount = envelope.as_ref().and_then(|env| env.committed_amount);

    let amount = match (intent_amount, &committed_amount) {
        (Some(intent_amount), Some(committed_amount)) => {
            if quantity_to_proof_scalar(intent_amount)? != *committed_amount {
                return Err(HandleAmountResolutionError::Mismatch);
            }
            intent_amount.clone()
        }
        (Some(intent_amount), None) => intent_amount.clone(),
        (None, Some(committed_amount)) => proof_scalar_to_quantity(*committed_amount),
        (None, None) => return Err(HandleAmountResolutionError::MissingAmount),
    };
    if amount.is_zero() {
        return Err(HandleAmountResolutionError::ZeroAmount);
    }

    let supplied_commitment = envelope
        .as_ref()
        .and_then(|proof_envelope| proof_envelope.amount_commitment);
    let commitment_required =
        intent_amount.is_none() || committed_amount.is_some() || supplied_commitment.is_some();
    let amount_commitment =
        commitment_required.then(|| derive_amount_commitment(asset_dsid, &amount, proof_payload));
    if supplied_commitment.is_some() && supplied_commitment != amount_commitment {
        return Err(HandleAmountResolutionError::CommitmentMismatch);
    }

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
    validate_model_descriptor(&model_descriptor(descriptor)).map_err(|_| VMError::PermissionDenied)
}

fn model_descriptor(descriptor: &AxtDescriptor) -> ModelAxtDescriptor {
    ModelAxtDescriptor {
        dsids: descriptor.dsids.clone(),
        touches: descriptor
            .touches
            .iter()
            .map(|touch| ModelAxtTouchSpec {
                dsid: touch.dsid,
                read: touch.read.clone(),
                write: touch.write.clone(),
            })
            .collect(),
    }
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
    ///
    /// # Errors
    ///
    /// Returns [`ModelAxtPolicySnapshotValidationError`] when the snapshot is
    /// not canonically ordered or its version does not bind its exact entries.
    pub fn new(
        snapshot: &ModelAxtPolicySnapshot,
    ) -> Result<Self, ModelAxtPolicySnapshotValidationError> {
        Self::new_with_timing(
            snapshot,
            NonZeroU64::new(1).expect("slot length must be non-zero"),
            0,
        )
    }

    /// Construct a policy from a snapshot and explicit timing parameters.
    ///
    /// # Errors
    ///
    /// Returns [`ModelAxtPolicySnapshotValidationError`] when the snapshot is
    /// not canonically ordered or its version does not bind its exact entries.
    pub fn new_with_timing(
        snapshot: &ModelAxtPolicySnapshot,
        slot_length_ms: NonZeroU64,
        max_clock_skew_ms: u64,
    ) -> Result<Self, ModelAxtPolicySnapshotValidationError> {
        snapshot.validate()?;
        let entries = snapshot
            .entries
            .iter()
            .map(|binding| (binding.dsid, binding.policy))
            .collect();
        Ok(Self {
            entries,
            slot_length_ms,
            max_clock_skew_ms,
        })
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
        if usage.handle.handle_era != entry.active_handle_era {
            return Err(VMError::PermissionDenied);
        }
        if usage.handle.sub_nonce != entry.next_handle_counter {
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

/// Validate the semantic canonical form of an AXT touch manifest.
///
/// Empty read/write sets are valid, but every present key must be non-empty,
/// trimmed, strictly sorted, and unique. In particular, an empty prefix is
/// forbidden because every key starts with it.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] when a key list is not canonical.
pub fn validate_touch_manifest(manifest: &TouchManifest) -> Result<(), VMError> {
    if !canonical_nonempty_strings(&manifest.read) || !canonical_nonempty_strings(&manifest.write) {
        return Err(VMError::NoritoInvalid);
    }
    Ok(())
}

/// Validate a persisted data-model touch manifest with the runtime invariants.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] when a key list is not canonical.
pub fn validate_model_touch_manifest(manifest: &ModelTouchManifest) -> Result<(), VMError> {
    validate_touch_manifest(&TouchManifest {
        read: manifest.read.clone(),
        write: manifest.write.clone(),
    })
}

fn canonical_nonempty_strings(values: &[String]) -> bool {
    values
        .iter()
        .all(|value| !value.is_empty() && value.trim() == value)
        && values.windows(2).all(|pair| pair[0] < pair[1])
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
    /// Exact active policy era selected by the issuer.
    pub handle_era: u64,
    /// Exact next per-dataspace counter selected by the issuer.
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
    /// Immutable network, issuer, code, and ABI context authenticated by the signature.
    pub issuer_context: AxtHandleIssuerContextV1,
    /// Mandatory signature made by the issuer key resolved from committed policy.
    pub issuer_signature: Signature,
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

/// Validate the context-free shape and value invariants of an asset handle.
///
/// Policy bindings, current slots, descriptor identity, subjects, operations,
/// and cumulative budgets require host context and are intentionally checked
/// by [`AxtPolicy`] and [`HostAxtState`].
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] for malformed fixed-width fields and
/// [`VMError::PermissionDenied`] for unusable zero/empty capability values.
pub fn validate_asset_handle(handle: &AssetHandle) -> Result<(), VMError> {
    if handle.axt_binding.len() != 32
        || handle.manifest_view_root.len() != 32
        || handle.group_binding.composability_group_id.is_empty()
        || !canonical_nonempty_strings(&handle.scope)
        || handle.subject.account.trim() != handle.subject.account
    {
        return Err(VMError::NoritoInvalid);
    }
    if handle.scope.is_empty()
        || handle.subject.account.is_empty()
        || handle.budget.remaining.is_zero()
        || handle
            .budget
            .per_use
            .as_ref()
            .is_some_and(Quantity::is_zero)
        || handle.handle_era == 0
        || handle.sub_nonce == 0
        || handle.group_binding.epoch_id == 0
        || handle.expiry_slot == 0
        || handle
            .issuer_context
            .issuer_manifest_root
            .iter()
            .all(|byte| *byte == 0)
        || handle.issuer_context.abi_version != 1
        || handle.issuer_context.abi_hash.iter().all(|byte| *byte == 0)
    {
        return Err(VMError::PermissionDenied);
    }
    Ok(())
}

/// Validate a persisted data-model handle with the pointer-runtime invariants.
///
/// # Errors
///
/// Returns the same error classification as [`validate_asset_handle`].
pub fn validate_model_asset_handle(handle: &ModelAssetHandle) -> Result<(), VMError> {
    validate_asset_handle(&AssetHandle {
        scope: handle.scope.clone(),
        subject: HandleSubject {
            account: handle.subject.account.clone(),
            origin_dsid: handle.subject.origin_dsid,
        },
        budget: HandleBudget {
            remaining: handle.budget.remaining.clone(),
            per_use: handle.budget.per_use.clone(),
        },
        handle_era: handle.handle_era,
        sub_nonce: handle.sub_nonce,
        group_binding: GroupBinding {
            composability_group_id: handle.group_binding.composability_group_id.clone(),
            epoch_id: handle.group_binding.epoch_id,
        },
        target_lane: handle.target_lane,
        axt_binding: handle.axt_binding.as_bytes().to_vec(),
        manifest_view_root: handle.manifest_view_root.to_vec(),
        expiry_slot: handle.expiry_slot,
        max_clock_skew_ms: handle.max_clock_skew_ms,
        issuer_context: handle.issuer_context,
        issuer_signature: handle.issuer_signature.clone(),
    })
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
    pub remaining: Quantity,
    /// Optional per-use cap.
    pub per_use: Option<Quantity>,
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
    /// Cleartext amount, or `None` when the proof carries a hidden amount.
    pub amount: Option<Quantity>,
}

/// Validate context-free invariants of a remote spend intent.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] for empty or non-canonical operation and
/// account strings, and [`VMError::PermissionDenied`] for an explicit zero
/// amount.
pub fn validate_remote_spend_intent(intent: &RemoteSpendIntent) -> Result<(), VMError> {
    for value in [&intent.op.kind, &intent.op.from, &intent.op.to] {
        if value.is_empty() || value.trim() != value {
            return Err(VMError::NoritoInvalid);
        }
    }
    if intent.op.amount.as_ref().is_some_and(Quantity::is_zero) {
        return Err(VMError::PermissionDenied);
    }
    Ok(())
}

/// Validate a persisted data-model intent with the pointer-runtime invariants.
///
/// # Errors
///
/// Returns the same error classification as [`validate_remote_spend_intent`].
pub fn validate_model_remote_spend_intent(intent: &ModelRemoteSpendIntent) -> Result<(), VMError> {
    validate_remote_spend_intent(&RemoteSpendIntent {
        asset_dsid: intent.asset_dsid,
        op: SpendOp {
            kind: intent.op.kind.clone(),
            from: intent.op.from.clone(),
            to: intent.op.to.clone(),
            amount: intent.op.amount.clone(),
        },
    })
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

/// Validate context-free proof-blob invariants.
///
/// Proof schema, dataspace binding, manifest binding, freshness relative to a
/// current slot, and cryptographic validity require host context.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] when proof bytes are empty or an explicit
/// expiry uses the forbidden zero sentinel.
pub fn validate_proof_blob(proof: &ProofBlob) -> Result<(), VMError> {
    if proof.payload.is_empty() || proof.expiry_slot == Some(0) {
        return Err(VMError::NoritoInvalid);
    }
    Ok(())
}

/// Preflight the structural FastPQ V1 binding carried by an AXT proof envelope.
///
/// This checks only envelope routing and metadata. It does not verify FastPQ
/// proof contents and must never be treated as proof acceptance.
///
/// # Errors
///
/// Returns [`VMError::PermissionDenied`] when the envelope does not bind to the
/// expected dataspace/manifest or does not advertise FastPQ V1 proof material.
pub fn preflight_fastpq_v1_proof_envelope_for_manifest(
    envelope: &AxtProofEnvelope,
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
) -> Result<(), VMError> {
    preflight_fastpq_v1_proof_envelope(envelope, dsid)?;
    if envelope.manifest_root != manifest_root {
        return Err(VMError::PermissionDenied);
    }
    Ok(())
}

/// Preflight an AXT proof envelope as FastPQ V1 material without pinning a
/// manifest root.
///
/// This is diagnostic routing/metadata validation only. A host must still call
/// a real FastPQ verifier before accepting the envelope as proof material.
///
/// # Errors
///
/// Returns [`VMError::PermissionDenied`] when the envelope does not bind to the
/// expected dataspace or does not advertise FastPQ V1 proof material.
pub fn preflight_fastpq_v1_proof_envelope(
    envelope: &AxtProofEnvelope,
    dsid: DataSpaceId,
) -> Result<(), VMError> {
    let Some(binding) = envelope.fastpq_binding.as_ref() else {
        return Err(VMError::PermissionDenied);
    };
    if envelope.dsid != dsid
        || envelope.manifest_root.iter().all(|byte| *byte == 0)
        || envelope.proof.is_empty()
        || binding.source_dsid != dsid.as_u64()
        || binding.verifier_id != "fastpq"
        || binding.verifier_version != "v1"
        || !fastpq_binding_shape_is_concrete(binding)
    {
        return Err(VMError::PermissionDenied);
    }
    Ok(())
}

fn fastpq_binding_shape_is_concrete(binding: &iroha_data_model::nexus::AxtFastpqBinding) -> bool {
    binding_string_is_present(&binding.parameter)
        && binding_string_is_present(&binding.source_dataspace)
        && binding_string_is_present(&binding.source_receipt_id)
        && binding_hex_digest_is_present(&binding.source_tx_commitment)
        && fastpq_claim_type_is_supported(&binding.claim_type)
        && binding_hex_digest_is_present(&binding.claim_digest)
        && binding_hex_digest_is_present(&binding.witness_commitment)
        && binding_hex_digest_is_present(&binding.policy_commitment)
        && binding_string_is_present(&binding.verified_effect_type)
        && binding_string_is_present(&binding.corridor)
        && !binding.target_dsids.is_empty()
        && binding
            .target_dsids
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && binding.effect_binding.as_ref().is_none_or(|effect| {
            [
                &effect.destination_domain,
                &effect.destination_account_id,
                &effect.vault_account_id,
                &effect.issuance_account_id,
                &effect.source_asset_definition_id,
                &effect.destination_asset_definition_id,
            ]
            .into_iter()
            .all(|value| value.as_deref().is_none_or(binding_string_is_present))
        })
}

fn binding_string_is_present(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn binding_hex_digest_is_present(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn fastpq_claim_type_is_supported(value: &str) -> bool {
    matches!(
        value,
        "authorization" | "compliance" | "tx_predicate" | "value_conservation"
    )
}

/// Compute the canonical descriptor binding used by asset handles.
///
/// The current implementation prefixes the descriptor bytes with a stable
/// domain separator and hashes the concatenation using the Poseidon2 sponge
/// (rate 2, capacity 1, +1 padding). This matches the normative definition
/// documented in `nexus.md`.
pub fn compute_binding(descriptor: &AxtDescriptor) -> Result<[u8; 32], norito::Error> {
    compute_descriptor_binding(&model_descriptor(descriptor))
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
        validate_touch_manifest(&manifest)?;
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
            validate_proof_blob(&p)?;
            if let Some(expiry) = p.expiry_slot
                && let Some(slot) = current_slot
                && slot > 0
                && slot > expiry
            {
                return Err(VMError::PermissionDenied);
            }
            self.proofs.insert(dsid, p);
        } else {
            self.proofs.remove(&dsid);
        }
        Ok(())
    }

    pub fn record_handle(&mut self, usage: HandleUsage) -> Result<(), VMError> {
        if usage.amount.is_zero() {
            return Err(VMError::PermissionDenied);
        }
        validate_asset_handle(&usage.handle)?;
        validate_remote_spend_intent(&usage.intent)?;
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
        if let Some(per_use) = usage.handle.budget.per_use.as_ref()
            && &usage.amount > per_use
        {
            return Err(VMError::PermissionDenied);
        }
        if let Some(proof) = &usage.proof {
            validate_proof_blob(proof)?;
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
            total: Quantity,
            per_dsid: BTreeMap<DataSpaceId, Quantity>,
        }

        impl HandleAccumulator {
            fn new(key: HandleBudgetKey) -> Self {
                Self {
                    key,
                    total: Quantity::zero(),
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
                .checked_add(&usage.amount)
                .map_err(|_| VMError::PermissionDenied)?;

            let ds_total = accumulator
                .per_dsid
                .entry(usage.intent.asset_dsid)
                .or_insert_with(Quantity::zero);
            *ds_total = ds_total
                .checked_add(&usage.amount)
                .map_err(|_| VMError::PermissionDenied)?;

            if accumulator.total > accumulator.key.budget_remaining {
                return Err(VMError::PermissionDenied);
            }
            if let Some(per_use) = accumulator.key.budget_per_use.as_ref()
                && &*ds_total > per_use
            {
                return Err(VMError::PermissionDenied);
            }
        }

        for accumulator in &accumulators {
            if accumulator.total > accumulator.key.budget_remaining {
                return Err(VMError::PermissionDenied);
            }
            if let Some(per_use) = accumulator.key.budget_per_use.as_ref()
                && accumulator.per_dsid.values().any(|total| total > per_use)
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
    pub amount: Quantity,
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
                remaining: usage.handle.budget.remaining.clone(),
                per_use: usage.handle.budget.per_use.clone(),
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
            issuer_context: usage.handle.issuer_context,
            issuer_signature: usage.handle.issuer_signature.clone(),
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
        let amount_hidden = usage.intent.op.amount.is_none();
        let amount_commitment = if amount_hidden {
            usage.amount_commitment.or_else(|| {
                Some(derive_amount_commitment(
                    usage.intent.asset_dsid,
                    &usage.amount,
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
            amount: (!amount_hidden).then(|| usage.amount.clone()),
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
    budget_remaining: Quantity,
    budget_per_use: Option<Quantity>,
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
            budget_remaining: handle.budget.remaining.clone(),
            budget_per_use: handle.budget.per_use.clone(),
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

    const ACCOUNT_FROM_LITERAL: &str = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    const ACCOUNT_TO_LITERAL: &str = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";

    fn quantity(value: u128) -> Quantity {
        value
            .to_string()
            .parse()
            .expect("test amount is a canonical quantity")
    }

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

    #[test]
    fn descriptor_validation_rejects_noncanonical_order_and_paths() {
        let first = DataSpaceId::new(1);
        let second = DataSpaceId::new(2);
        let valid = AxtDescriptor {
            dsids: vec![first, second],
            touches: vec![
                AxtTouchSpec {
                    dsid: first,
                    read: vec!["orders".into()],
                    write: vec!["ledger".into()],
                },
                AxtTouchSpec {
                    dsid: second,
                    read: Vec::new(),
                    write: Vec::new(),
                },
            ],
        };
        assert_eq!(validate_descriptor(&valid), Ok(()));

        let mut cases = Vec::new();
        let mut descriptor = valid.clone();
        descriptor.dsids.swap(0, 1);
        cases.push(descriptor);
        let mut descriptor = valid.clone();
        descriptor.touches.swap(0, 1);
        cases.push(descriptor);
        for paths in [
            vec!["".to_owned()],
            vec![" orders".to_owned()],
            vec!["orders".to_owned(), "orders".to_owned()],
            vec!["z".to_owned(), "a".to_owned()],
        ] {
            let mut descriptor = valid.clone();
            descriptor.touches[0].read = paths;
            cases.push(descriptor);
        }

        for descriptor in cases {
            assert_eq!(
                validate_descriptor(&descriptor),
                Err(VMError::PermissionDenied),
                "noncanonical descriptor must fail: {descriptor:?}"
            );
        }
    }

    fn sample_touch_manifest() -> TouchManifest {
        TouchManifest {
            read: vec!["orders/item".into()],
            write: vec!["ledger/item".into()],
        }
    }

    #[test]
    fn touch_manifest_rejects_empty_whitespace_duplicate_and_unsorted_keys() {
        assert_eq!(validate_touch_manifest(&sample_touch_manifest()), Ok(()));
        assert_eq!(
            validate_touch_manifest(&TouchManifest {
                read: Vec::new(),
                write: Vec::new(),
            }),
            Ok(()),
            "an explicitly empty runtime manifest is valid"
        );
        for keys in [
            vec!["".to_owned()],
            vec![" key".to_owned()],
            vec!["key".to_owned(), "key".to_owned()],
            vec!["z".to_owned(), "a".to_owned()],
        ] {
            assert_eq!(
                validate_touch_manifest(&TouchManifest {
                    read: keys,
                    write: Vec::new(),
                }),
                Err(VMError::NoritoInvalid)
            );
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
            budget: HandleBudget {
                remaining: quantity(remaining),
                per_use: per_use.map(quantity),
            },
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
            issuer_context: Default::default(),
            issuer_signature: Signature::from_bytes(&[1_u8; 64]),
        }
    }

    #[test]
    fn standalone_handle_validation_rejects_every_context_free_fault() {
        let dsid = DataSpaceId::new(7);
        let valid = sample_handle(dsid, [0x11; 32], 10, Some(5));
        assert_eq!(validate_asset_handle(&valid), Ok(()));

        let mut malformed = valid.clone();
        malformed.axt_binding.pop();
        assert_eq!(
            validate_asset_handle(&malformed),
            Err(VMError::NoritoInvalid)
        );

        let mut malformed = valid.clone();
        malformed.manifest_view_root.push(0);
        assert_eq!(
            validate_asset_handle(&malformed),
            Err(VMError::NoritoInvalid)
        );

        let mut malformed = valid.clone();
        malformed.group_binding.composability_group_id.clear();
        assert_eq!(
            validate_asset_handle(&malformed),
            Err(VMError::NoritoInvalid)
        );

        let mut unusable = valid.clone();
        unusable.scope.clear();
        assert_eq!(
            validate_asset_handle(&unusable),
            Err(VMError::PermissionDenied)
        );

        let mut unusable = valid.clone();
        unusable.budget.remaining = Quantity::zero();
        assert_eq!(
            validate_asset_handle(&unusable),
            Err(VMError::PermissionDenied)
        );

        let mut unusable = valid.clone();
        unusable.budget.per_use = Some(Quantity::zero());
        assert_eq!(
            validate_asset_handle(&unusable),
            Err(VMError::PermissionDenied)
        );

        for field in ["handle era", "sub nonce", "group epoch", "expiry slot"] {
            let mut unusable = valid.clone();
            match field {
                "handle era" => unusable.handle_era = 0,
                "sub nonce" => unusable.sub_nonce = 0,
                "group epoch" => unusable.group_binding.epoch_id = 0,
                "expiry slot" => unusable.expiry_slot = 0,
                _ => unreachable!(),
            }
            assert_eq!(
                validate_asset_handle(&unusable),
                Err(VMError::PermissionDenied),
                "zero {field} must fail validation"
            );
        }

        let malformed_mutations: [fn(&mut AssetHandle); 4] = [
            |handle: &mut AssetHandle| handle.scope[0].push(' '),
            |handle: &mut AssetHandle| handle.scope.push("transfer".to_owned()),
            |handle: &mut AssetHandle| {
                handle.scope = vec!["withdraw".to_owned(), "transfer".to_owned()];
            },
            |handle: &mut AssetHandle| handle.subject.account.push(' '),
        ];
        for mutate in malformed_mutations {
            let mut malformed = valid.clone();
            mutate(&mut malformed);
            assert_eq!(
                validate_asset_handle(&malformed),
                Err(VMError::NoritoInvalid)
            );
        }

        let mut unusable = valid;
        unusable.subject.account.clear();
        assert_eq!(
            validate_asset_handle(&unusable),
            Err(VMError::PermissionDenied)
        );
    }

    #[test]
    fn standalone_proof_blob_validation_rejects_empty_and_zero_expiry() {
        let valid = ProofBlob {
            payload: vec![1],
            expiry_slot: Some(1),
        };
        assert_eq!(validate_proof_blob(&valid), Ok(()));
        assert_eq!(
            validate_proof_blob(&ProofBlob {
                payload: Vec::new(),
                expiry_slot: None,
            }),
            Err(VMError::NoritoInvalid)
        );
        assert_eq!(
            validate_proof_blob(&ProofBlob {
                payload: vec![1],
                expiry_slot: Some(0),
            }),
            Err(VMError::NoritoInvalid)
        );
    }

    fn sample_intent(dsid: DataSpaceId, amount: Option<u128>) -> RemoteSpendIntent {
        RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: ACCOUNT_FROM_LITERAL.into(),
                to: ACCOUNT_TO_LITERAL.into(),
                amount: amount.map(|value| {
                    value
                        .to_string()
                        .parse::<Quantity>()
                        .expect("test amount is a canonical quantity")
                }),
            },
        }
    }

    #[test]
    fn remote_spend_intent_rejects_empty_whitespace_and_zero_values() {
        let dsid = DataSpaceId::new(7);
        let valid = sample_intent(dsid, Some(1));
        assert_eq!(validate_remote_spend_intent(&valid), Ok(()));

        for field in ["kind", "from", "to"] {
            let mut invalid = valid.clone();
            match field {
                "kind" => invalid.op.kind.clear(),
                "from" => invalid.op.from.push(' '),
                "to" => invalid.op.to.clear(),
                _ => unreachable!(),
            }
            assert_eq!(
                validate_remote_spend_intent(&invalid),
                Err(VMError::NoritoInvalid),
                "invalid {field} must fail"
            );
        }
        assert_eq!(
            validate_remote_spend_intent(&sample_intent(dsid, Some(0))),
            Err(VMError::PermissionDenied)
        );
    }

    fn sample_fastpq_binding(dsid: DataSpaceId) -> iroha_data_model::nexus::AxtFastpqBinding {
        iroha_data_model::nexus::AxtFastpqBinding {
            parameter: "fastpq-lane-balanced".to_string(),
            source_dsid: dsid.as_u64(),
            source_dataspace: "ivm-abi-test".to_string(),
            source_receipt_id: format!("receipt-{}", dsid.as_u64()),
            source_tx_commitment: "aa".repeat(32),
            claim_type: "authorization".to_string(),
            claim_digest: "bb".repeat(32),
            witness_commitment: "cc".repeat(32),
            policy_commitment: "dd".repeat(32),
            verified_effect_type: "test_effect".to_string(),
            corridor: "ivm-abi-test".to_string(),
            verifier_id: "fastpq".to_string(),
            verifier_version: "v1".to_string(),
            target_dsids: vec![dsid.as_u64()],
            effect_binding: None,
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
            fastpq_binding: Some(sample_fastpq_binding(dsid)),
            committed_amount,
            amount_commitment,
        })
        .expect("encode proof envelope");
        ProofBlob {
            payload,
            expiry_slot: Some(10),
        }
    }

    fn proof_with_derived_amount_commitment(
        dsid: DataSpaceId,
        committed_amount: u128,
    ) -> ProofBlob {
        let mut proof = proof_with_amount(dsid, Some(committed_amount), None);
        let amount = quantity(committed_amount);
        let commitment = derive_amount_commitment(dsid, &amount, Some(&proof.payload));
        let mut envelope = norito::decode_from_bytes::<AxtProofEnvelope>(&proof.payload)
            .expect("decode test proof envelope");
        envelope.amount_commitment = Some(commitment);
        proof.payload = norito::to_bytes(&envelope).expect("encode committed test proof envelope");
        proof
    }

    #[test]
    fn preflight_fastpq_v1_proof_envelope_rejects_mislabeled_binding() {
        let dsid = DataSpaceId::new(90);
        let manifest_root = [0xAB; 32];
        let mut envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0x01, 0x02],
            fastpq_binding: Some(sample_fastpq_binding(dsid)),
            committed_amount: None,
            amount_commitment: None,
        };
        preflight_fastpq_v1_proof_envelope_for_manifest(&envelope, dsid, manifest_root)
            .expect("valid FastPQ V1 envelope preflight");

        envelope
            .fastpq_binding
            .as_mut()
            .expect("binding")
            .verifier_id = "synthetic".to_string();
        assert!(matches!(
            preflight_fastpq_v1_proof_envelope_for_manifest(&envelope, dsid, manifest_root),
            Err(VMError::PermissionDenied)
        ));

        envelope
            .fastpq_binding
            .as_mut()
            .expect("binding")
            .verifier_id = "fastpq".to_string();
        envelope
            .fastpq_binding
            .as_mut()
            .expect("binding")
            .verifier_version = "v2".to_string();
        assert!(matches!(
            preflight_fastpq_v1_proof_envelope_for_manifest(&envelope, dsid, manifest_root),
            Err(VMError::PermissionDenied)
        ));
    }

    #[test]
    fn preflight_fastpq_v1_proof_envelope_rejects_synthetic_binding() {
        let dsid = DataSpaceId::new(91);
        let manifest_root = [0xAC; 32];
        let mut envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0x01, 0x02],
            fastpq_binding: Some(sample_fastpq_binding(dsid)),
            committed_amount: None,
            amount_commitment: None,
        };
        envelope
            .fastpq_binding
            .as_mut()
            .expect("binding")
            .claim_digest = String::new();
        assert!(matches!(
            preflight_fastpq_v1_proof_envelope_for_manifest(&envelope, dsid, manifest_root),
            Err(VMError::PermissionDenied)
        ));

        envelope.fastpq_binding = Some(sample_fastpq_binding(dsid));
        envelope
            .fastpq_binding
            .as_mut()
            .expect("binding")
            .claim_type = "synthetic".to_string();
        assert!(matches!(
            preflight_fastpq_v1_proof_envelope_for_manifest(&envelope, dsid, manifest_root),
            Err(VMError::PermissionDenied)
        ));

        envelope.fastpq_binding = Some(sample_fastpq_binding(dsid));
        envelope
            .fastpq_binding
            .as_mut()
            .expect("binding")
            .target_dsids
            .clear();
        assert!(matches!(
            preflight_fastpq_v1_proof_envelope_for_manifest(&envelope, dsid, manifest_root),
            Err(VMError::PermissionDenied)
        ));
    }

    #[test]
    fn preflight_fastpq_v1_proof_envelope_rejects_noncanonical_binding_fields() {
        let dsid = DataSpaceId::new(92);
        let manifest_root = [0xAD; 32];
        let mut uppercase_digest = sample_fastpq_binding(dsid);
        uppercase_digest.claim_digest.make_ascii_uppercase();
        let mut unordered_targets = sample_fastpq_binding(dsid);
        unordered_targets.target_dsids = vec![dsid.as_u64() + 1, dsid.as_u64()];
        let mut untrimmed_corridor = sample_fastpq_binding(dsid);
        untrimmed_corridor.corridor.push(' ');

        for binding in [uppercase_digest, unordered_targets, untrimmed_corridor] {
            let envelope = AxtProofEnvelope {
                dsid,
                manifest_root,
                da_commitment: None,
                proof: vec![0x01, 0x02],
                fastpq_binding: Some(binding),
                committed_amount: None,
                amount_commitment: None,
            };
            assert_eq!(
                preflight_fastpq_v1_proof_envelope_for_manifest(&envelope, dsid, manifest_root),
                Err(VMError::PermissionDenied)
            );
        }
    }

    #[test]
    fn resolve_handle_amount_accepts_cleartext_intent() {
        let dsid = DataSpaceId::new(90);
        let intent = sample_intent(dsid, Some(42));
        let resolved = resolve_handle_amount(&intent, None).expect("resolve amount");
        assert_eq!(resolved.amount, quantity(42));
        assert_eq!(resolved.amount_commitment, None);
    }

    #[test]
    fn amount_commitment_is_independent_of_ambient_norito_layout() {
        let dsid = DataSpaceId::new(90);
        let proof = proof_with_amount(dsid, Some(42), None);
        let amount = quantity(42);
        let expected = derive_amount_commitment(dsid, &amount, Some(&proof.payload));

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let ambient_before = norito::to_bytes(&AxtProofEnvelope {
            dsid,
            manifest_root: [0xAB; 32],
            da_commitment: None,
            proof: vec![0x01, 0x02],
            fastpq_binding: Some(sample_fastpq_binding(dsid)),
            committed_amount: Some(42),
            amount_commitment: None,
        })
        .expect("encode ambient proof");
        assert_eq!(
            derive_amount_commitment(dsid, &amount, Some(&proof.payload)),
            expected
        );
        assert_eq!(
            norito::to_bytes(
                &decode_canonical_norito::<AxtProofEnvelope>(&proof.payload)
                    .expect("decode canonical proof")
            )
            .expect("re-encode under ambient flags"),
            ambient_before,
            "canonical commitment derivation must restore the caller's ambient flags"
        );
    }

    #[test]
    fn resolve_handle_amount_rejects_malformed_and_alternate_proof_envelopes() {
        let dsid = DataSpaceId::new(90);
        let intent = sample_intent(dsid, Some(42));
        let malformed = ProofBlob {
            payload: vec![1, 2, 3],
            expiry_slot: None,
        };
        assert_eq!(
            resolve_handle_amount(&intent, Some(&malformed)),
            Err(HandleAmountResolutionError::InvalidProofEnvelope)
        );

        let canonical = proof_with_amount(dsid, Some(42), None);
        let envelope = decode_canonical_norito::<AxtProofEnvelope>(&canonical.payload)
            .expect("decode canonical proof");
        let alternate = {
            let flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _alternate = norito::core::DecodeFlagsGuard::enter(flags);
            norito::to_bytes(&envelope).expect("encode alternate proof")
        };
        assert_ne!(alternate, canonical.payload);
        assert_eq!(
            resolve_handle_amount(
                &intent,
                Some(&ProofBlob {
                    payload: alternate,
                    expiry_slot: canonical.expiry_slot,
                })
            ),
            Err(HandleAmountResolutionError::InvalidProofEnvelope)
        );
    }

    #[test]
    fn resolve_handle_amount_uses_proof_commit_when_intent_hidden() {
        let dsid = DataSpaceId::new(91);
        let intent = sample_intent(dsid, None);
        let proof = proof_with_amount(dsid, Some(77), None);
        let resolved = resolve_handle_amount(&intent, Some(&proof)).expect("resolve amount");
        assert_eq!(resolved.amount, quantity(77));
        assert_eq!(
            resolved.amount_commitment,
            Some(derive_amount_commitment(
                dsid,
                &quantity(77),
                Some(proof.payload.as_slice())
            ))
        );
    }

    #[test]
    fn resolve_handle_amount_authenticates_supplied_commitment_without_circular_hashing() {
        let dsid = DataSpaceId::new(97);
        let intent = sample_intent(dsid, None);
        let proof = proof_with_derived_amount_commitment(dsid, 77);
        let resolved = resolve_handle_amount(&intent, Some(&proof))
            .expect("canonical supplied commitment must resolve");
        let expected = derive_amount_commitment(dsid, &quantity(77), Some(&proof.payload));
        assert_eq!(resolved.amount_commitment, Some(expected));

        let mut envelope = norito::decode_from_bytes::<AxtProofEnvelope>(&proof.payload)
            .expect("decode committed proof envelope");
        assert_eq!(envelope.amount_commitment, Some(expected));
        envelope.proof.push(0xFF);
        let mutated = ProofBlob {
            payload: norito::to_bytes(&envelope).expect("encode mutated proof envelope"),
            expiry_slot: proof.expiry_slot,
        };
        assert_eq!(
            resolve_handle_amount(&intent, Some(&mutated)),
            Err(HandleAmountResolutionError::CommitmentMismatch)
        );
    }

    #[test]
    fn resolve_handle_amount_rejects_attacker_supplied_commitment() {
        let dsid = DataSpaceId::new(98);
        let intent = sample_intent(dsid, None);
        let proof = proof_with_amount(dsid, Some(9), Some([0xA5; 32]));
        assert_eq!(
            resolve_handle_amount(&intent, Some(&proof)),
            Err(HandleAmountResolutionError::CommitmentMismatch)
        );
    }

    #[test]
    fn component_amount_resolver_matches_host_amount_resolver() {
        let dsid = DataSpaceId::new(96);
        let intent = sample_intent(dsid, Some(31));
        let proof = proof_with_amount(dsid, Some(31), None);
        let host = resolve_handle_amount(&intent, Some(&proof)).expect("host resolution");
        let components = resolve_handle_amount_components(
            dsid,
            intent.op.amount.as_ref(),
            Some(proof.payload.as_slice()),
        )
        .expect("component resolution");
        assert_eq!(components, host);
    }

    #[test]
    fn resolve_handle_amount_rejects_intent_proof_mismatch() {
        let dsid = DataSpaceId::new(92);
        let intent = sample_intent(dsid, Some(11));
        let proof = proof_with_amount(dsid, Some(12), None);
        assert_eq!(
            resolve_handle_amount(&intent, Some(&proof)),
            Err(HandleAmountResolutionError::Mismatch)
        );
    }

    #[test]
    fn resolve_handle_amount_rejects_fractional_proof_scalar() {
        let dsid = DataSpaceId::new(93);
        let mut intent = sample_intent(dsid, None);
        intent.op.amount = Some("1.5".parse().expect("canonical fractional quantity"));
        let proof = proof_with_amount(dsid, Some(1), None);
        assert_eq!(
            resolve_handle_amount(&intent, Some(&proof)),
            Err(HandleAmountResolutionError::InvalidProofScalar)
        );
    }

    #[test]
    fn resolve_handle_amount_rejects_proof_scalar_wider_than_u128() {
        let dsid = DataSpaceId::new(94);
        let mut intent = sample_intent(dsid, None);
        intent.op.amount = Some(
            "340282366920938463463374607431768211456"
                .parse()
                .expect("u128 maximum plus one fits the Quantity domain"),
        );
        let proof = proof_with_amount(dsid, Some(u128::MAX), None);
        assert_eq!(
            resolve_handle_amount(&intent, Some(&proof)),
            Err(HandleAmountResolutionError::InvalidProofScalar)
        );
    }

    #[test]
    fn resolve_handle_amount_rejects_zero_committed_scalar() {
        let dsid = DataSpaceId::new(95);
        let intent = sample_intent(dsid, None);
        let proof = proof_with_amount(dsid, Some(0), None);
        assert_eq!(
            resolve_handle_amount(&intent, Some(&proof)),
            Err(HandleAmountResolutionError::ZeroAmount)
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
        let intent = sample_intent(dsid, None);
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
        assert_eq!(fragment.amount, None);
        assert_eq!(fragment.amount_commitment, resolved.amount_commitment);
    }

    #[test]
    fn snapshot_policy_rejects_excess_skew_request() {
        let dsid = DataSpaceId::new(8);
        let entry = ModelAxtPolicyEntry {
            manifest_root: [0xAB; 32],
            target_lane: LaneId::new(0),
            active_handle_era: 1,
            next_handle_counter: 1,
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
        )
        .expect("canonical policy snapshot");

        let binding = [0x11; 32];
        let mut handle = sample_handle(dsid, binding, 25, None);
        handle.manifest_view_root = entry.manifest_root.to_vec();
        handle.target_lane = entry.target_lane;
        handle.handle_era = entry.active_handle_era;
        handle.sub_nonce = entry.next_handle_counter;
        handle.max_clock_skew_ms = Some(6);
        let intent = sample_intent(dsid, Some(1));
        let usage = HandleUsage {
            handle,
            intent,
            proof: None,
            amount: quantity(1),
            amount_commitment: None,
        };

        assert!(matches!(
            policy.allow_handle(&usage),
            Err(VMError::PermissionDenied)
        ));
    }

    #[test]
    fn snapshot_policy_rejects_noncanonical_snapshot_without_panicking() {
        let dsid = DataSpaceId::new(8);
        let entry = ModelAxtPolicyEntry {
            manifest_root: [0xAB; 32],
            target_lane: LaneId::new(0),
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 10,
        };
        let snapshot = ModelAxtPolicySnapshot {
            version: 1,
            entries: vec![iroha_data_model::nexus::AxtPolicyBinding {
                dsid,
                policy: entry,
            }],
        };

        assert!(matches!(
            SnapshotAxtPolicy::new(&snapshot),
            Err(ModelAxtPolicySnapshotValidationError::VersionMismatch { .. })
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
                intent: sample_intent(dsid, Some(60)),
                proof: proof.clone(),
                amount: quantity(60),
                amount_commitment: None,
            })
            .expect("first usage within budget");
        handle.sub_nonce = handle.sub_nonce.saturating_add(1);
        state
            .record_handle(HandleUsage {
                handle,
                intent: sample_intent(dsid, Some(50)),
                proof,
                amount: quantity(50),
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
                intent: sample_intent(dsid, Some(60)),
                proof: proof.clone(),
                amount: quantity(60),
                amount_commitment: None,
            })
            .expect("first usage within budget");
        handle.sub_nonce = handle.sub_nonce.saturating_add(1);
        state
            .record_handle(HandleUsage {
                handle,
                intent: sample_intent(dsid, Some(60)),
                proof,
                amount: quantity(60),
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
                intent: sample_intent(dsid, Some(50)),
                proof: proof.clone(),
                amount: quantity(50),
                amount_commitment: None,
            })
            .expect("first usage within budget");
        handle.sub_nonce = handle.sub_nonce.saturating_add(1);
        state
            .record_handle(HandleUsage {
                handle,
                intent: sample_intent(dsid, Some(50)),
                proof,
                amount: quantity(50),
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
        let intent = sample_intent(dsid, Some(10));
        let usage = HandleUsage {
            handle,
            intent,
            proof: None,
            amount: quantity(10),
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
                intent: sample_intent(dsid, Some(10)),
                proof: proof.clone(),
                amount: quantity(10),
                amount_commitment: None,
            })
            .expect("first usage accepted");
        state
            .record_handle(HandleUsage {
                handle: handle_low,
                intent: sample_intent(dsid, Some(10)),
                proof,
                amount: quantity(10),
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
                intent: sample_intent(ds_a, Some(10)),
                proof: proof.clone(),
                amount: quantity(10),
                amount_commitment: None,
            })
            .expect("first usage accepted");
        state
            .record_handle(HandleUsage {
                handle: handle_b,
                intent: sample_intent(ds_b, Some(10)),
                proof,
                amount: quantity(10),
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
        let intent = sample_intent(dsid, Some(10));

        let mut zero_era = base_handle.clone();
        zero_era.handle_era = 0;
        let err = state
            .record_handle(HandleUsage {
                handle: zero_era,
                intent: intent.clone(),
                proof: None,
                amount: quantity(10),
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
                amount: quantity(10),
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
            intent: sample_intent(dsid, Some(5)),
            proof: None,
            amount: quantity(5),
            amount_commitment: None,
        };
        state.record_handle(usage).expect("handle usage recorded");

        let fragment = state
            .handle_fragments()
            .first()
            .expect("handle fragment recorded");
        assert_eq!(fragment.handle.axt_binding.as_bytes(), &binding);
        assert_eq!(fragment.intent.asset_dsid, dsid);
        assert_eq!(fragment.amount, Some(Quantity::from(5_u64)));
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
                intent: sample_intent(dsid, Some(10)),
                proof: None,
                amount: quantity(10),
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
            intent: sample_intent(dsid, Some(25)),
            proof: proof.clone(),
            amount: quantity(25),
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
