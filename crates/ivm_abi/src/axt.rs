//! Atomic cross-transaction (AXT) helper types.
//!
//! The structures defined here deliberately model only the subset of fields exercised by the
//! current host implementation. They provide a Norito- compatible schema so test fixtures can
//! round-trip through the pointer-ABI TLVs exposed to the VM. As the end-to-end pipeline matures
//! these models should converge with the canonical data-model crate.
use crate::{
    codec::{decode_canonical_norito, encode_canonical_norito},
    error::VMError,
};
use iroha_crypto::{Hash, Signature};
#[cfg(test)]
use iroha_data_model::nexus::AxtAssetIncarnationV1;
use iroha_data_model::nexus::{
    AssetHandle as ModelAssetHandle, AxtBinding, AxtDescriptor as ModelAxtDescriptor,
    AxtHandleBudgetKey as ModelAxtHandleBudgetKey,
    AxtHandleBudgetRecord as ModelAxtHandleBudgetRecord, AxtHandleFragment,
    AxtHandleIssuerContextV1, AxtHandleReplayKey, AxtPolicyEntry as ModelAxtPolicyEntry,
    AxtPolicySnapshot as ModelAxtPolicySnapshot,
    AxtPolicySnapshotValidationError as ModelAxtPolicySnapshotValidationError,
    AxtProofEnvelope as ModelAxtProofEnvelope, AxtTouchSpec as ModelAxtTouchSpec, DataSpaceId,
    GroupBinding as ModelGroupBinding, HandleBudget as ModelHandleBudget,
    HandleSubject as ModelHandleSubject, LaneId, MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES,
    ProofBlob as ModelProofBlob, RemoteSpendIntent as ModelRemoteSpendIntent,
    SpendOp as ModelSpendOp, TouchManifest as ModelTouchManifest, compute_descriptor_binding,
    compute_remote_spend_intent_commitment_v1, validate_descriptor as validate_model_descriptor,
};
use iroha_data_model::{
    asset::AssetBalanceScope,
    prelude::{AccountId, AssetDefinitionId, Quantity},
};
use norito::codec::{Decode, Encode};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
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
/// Dynamic AXT handle-use facts extracted from one canonically decoded and
/// cryptographically verified proof envelope.
///
/// Constructing this value does not itself verify the FASTPQ proof. Callers
/// must only build it after successful proof verification. The compact facts
/// let repeated handles enforce amount and intent membership without decoding
/// or scanning the full proof payload again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxtProofUseFacts {
    dsid: DataSpaceId,
    committed_amount: Option<u128>,
    supplied_amount_commitment: Option<[u8; 32]>,
    fixed_amount_commitment: Option<[u8; 32]>,
    variable_amount_commitment_payload: Option<Vec<u8>>,
    remote_spend_intent_commitments: Vec<[u8; 32]>,
}
impl AxtProofUseFacts {
    /// Extract reusable handle-use facts from an already verified canonical envelope.
    ///
    /// This consumes the decoded envelope so its potentially large remote-intent
    /// commitment vector can be retained without cloning it.
    #[must_use]
    pub fn from_verified_envelope(envelope: AxtProofEnvelope) -> Self {
        Self::from_canonical_envelope(envelope)
    }
    fn from_canonical_envelope(mut envelope: AxtProofEnvelope) -> Self {
        let dsid = envelope.dsid;
        let committed_amount = envelope.committed_amount;
        let supplied_amount_commitment = envelope.amount_commitment;
        let normalized_payload =
            (committed_amount.is_some() || supplied_amount_commitment.is_some()).then(|| {
                envelope.amount_commitment = None;
                encode_canonical_norito(&envelope)
                    .expect("a decoded canonical AXT proof envelope always re-encodes")
            });
        let fixed_amount_commitment = committed_amount.map(|amount| {
            derive_amount_commitment_from_normalized_payload(
                dsid,
                &proof_scalar_to_quantity(amount),
                normalized_payload.as_deref(),
            )
        });
        let variable_amount_commitment_payload =
            if committed_amount.is_none() && supplied_amount_commitment.is_some() {
                normalized_payload
            } else {
                None
            };
        let remote_spend_intent_commitments = envelope
            .fastpq_binding
            .take()
            .map_or_else(Vec::new, |binding| binding.remote_spend_intent_commitments);
        Self {
            dsid,
            committed_amount,
            supplied_amount_commitment,
            fixed_amount_commitment,
            variable_amount_commitment_payload,
            remote_spend_intent_commitments,
        }
    }

    /// Return the proof-bound, canonically ordered remote-spend commitments.
    #[must_use]
    pub fn remote_spend_intent_commitments(&self) -> &[[u8; 32]] {
        &self.remote_spend_intent_commitments
    }

    /// Require exact one-time consumption of every proof-bound remote-spend claim.
    ///
    /// The input may arrive in handle execution order. It is sorted without
    /// deduplication, so duplicate handle consumption and unconsumed proof
    /// claims both fail the exact comparison.
    ///
    /// # Errors
    ///
    /// Returns [`VMError::PermissionDenied`] unless the consumed commitment
    /// multiset exactly equals the proof's canonical commitment set.
    pub fn validate_remote_spend_consumption(
        &self,
        consumed_commitments: &[[u8; 32]],
    ) -> Result<(), VMError> {
        let mut consumed = consumed_commitments.to_vec();
        consumed.sort_unstable();
        if consumed == self.remote_spend_intent_commitments {
            Ok(())
        } else {
            Err(VMError::PermissionDenied)
        }
    }
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
/// This digest links the envelope and fragment copies for consistency; it does
/// not authenticate the amount. Amount authenticity comes from the
/// proof-bound FASTPQ batch metadata checked by the AXT proof verifier.
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
        if payload.len() > MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES {
            return Cow::Borrowed(payload);
        }
        decode_canonical_norito::<AxtProofEnvelope>(payload).map_or_else(
            |_| Cow::Borrowed(payload),
            |mut envelope| {
                envelope.amount_commitment = None;
                Cow::Owned(
                    encode_canonical_norito(&envelope)
                        .expect("a decoded canonical AXT proof envelope always re-encodes"),
                )
            },
        )
    });
    derive_amount_commitment_from_normalized_payload(
        dsid,
        amount,
        normalized_proof_payload.as_deref(),
    )
}
fn derive_amount_commitment_from_normalized_payload(
    dsid: DataSpaceId,
    amount: &Quantity,
    proof_payload: Option<&[u8]>,
) -> [u8; 32] {
    let amount_text = amount.to_string();
    let amount_len =
        u16::try_from(amount_text.len()).expect("bounded Quantity text length always fits in u16");
    let dsid_bytes = dsid.as_u64().to_be_bytes();
    let amount_len_bytes = amount_len.to_be_bytes();
    let fixed_chunks = [
        AMOUNT_COMMITMENT_DOMAIN_SEPARATOR,
        dsid_bytes.as_slice(),
        amount_len_bytes.as_slice(),
        amount_text.as_bytes(),
    ];
    proof_payload.map_or_else(
        || Hash::new_from_chunks(&fixed_chunks).into(),
        |payload| {
            Hash::new_from_chunks(&[
                fixed_chunks[0],
                fixed_chunks[1],
                fixed_chunks[2],
                fixed_chunks[3],
                payload,
            ])
            .into()
        },
    )
}
/// Resolve an effective amount and commitment for a handle usage.
///
/// This supports both cleartext (`intent.op.amount`) and hidden modes where the cleartext amount is
/// redacted and a committed amount is carried in the [`AxtProofEnvelope`].
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
/// Returns [`HandleAmountResolutionError`] when the amount is absent, zero, inconsistent with the
/// proof statement, or cannot be represented exactly by the V1 proof scalar.
pub fn resolve_handle_amount_components(
    asset_dsid: DataSpaceId,
    intent_amount: Option<&Quantity>,
    proof_payload: Option<&[u8]>,
) -> Result<ResolvedHandleAmount, HandleAmountResolutionError> {
    let Some(proof_payload) = proof_payload else {
        let amount = intent_amount
            .cloned()
            .ok_or(HandleAmountResolutionError::MissingAmount)?;
        if amount.is_zero() {
            return Err(HandleAmountResolutionError::ZeroAmount);
        }
        return Ok(ResolvedHandleAmount {
            amount,
            amount_commitment: None,
        });
    };
    if proof_payload.len() > MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES {
        return Err(HandleAmountResolutionError::InvalidProofEnvelope);
    }
    let envelope = decode_canonical_norito::<AxtProofEnvelope>(proof_payload)
        .map_err(|_| HandleAmountResolutionError::InvalidProofEnvelope)?;
    let facts = AxtProofUseFacts::from_canonical_envelope(envelope);
    resolve_handle_amount_components_from_proof_facts(asset_dsid, intent_amount, &facts)
}
/// Resolve an effective handle amount from cached, verified proof facts.
///
/// Unlike [`resolve_handle_amount_components`], this path performs no proof
/// decoding or full-payload scan and is suitable for repeated use of one proof.
///
/// # Errors
///
/// Returns [`HandleAmountResolutionError`] when the dataspace or amount does
/// not match the verified proof facts, or when the amount is absent, zero, or
/// not exactly representable by the V1 proof scalar.
pub fn resolve_handle_amount_components_from_proof_facts(
    asset_dsid: DataSpaceId,
    intent_amount: Option<&Quantity>,
    facts: &AxtProofUseFacts,
) -> Result<ResolvedHandleAmount, HandleAmountResolutionError> {
    if facts.dsid != asset_dsid {
        return Err(HandleAmountResolutionError::InvalidProofEnvelope);
    }
    let committed_amount = facts.committed_amount;
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
    let supplied_commitment = facts.supplied_amount_commitment;
    let commitment_required =
        intent_amount.is_none() || committed_amount.is_some() || supplied_commitment.is_some();
    let amount_commitment = commitment_required.then(|| {
        facts.fixed_amount_commitment.unwrap_or_else(|| {
            derive_amount_commitment_from_normalized_payload(
                asset_dsid,
                &amount,
                facts.variable_amount_commitment_payload.as_deref(),
            )
        })
    });
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
    /// Exact asset definition authorized by the issuer signature.
    pub asset_definition_id: AssetDefinitionId,
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
/// Returns [`VMError::NoritoInvalid`] for malformed fixed-width fields or a
/// non-canonical account identifier and [`VMError::PermissionDenied`] for
/// unusable zero/empty capability values.
pub fn validate_asset_handle(handle: &AssetHandle) -> Result<(), VMError> {
    if handle.axt_binding.len() != 32
        || handle.manifest_view_root.len() != 32
        || handle.group_binding.composability_group_id.is_empty()
        || !canonical_nonempty_strings(&handle.scope)
        || (!handle.subject.account.is_empty()
            && canonical_account_id(&handle.subject.account).is_none())
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
        || handle.issuer_context.validate().is_err()
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
        asset_definition_id: handle.asset_definition_id.clone(),
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
impl TryFrom<&AssetHandle> for ModelAssetHandle {
    type Error = VMError;

    fn try_from(handle: &AssetHandle) -> Result<Self, Self::Error> {
        let binding = handle.binding_array().ok_or(VMError::NoritoInvalid)?;
        let manifest_view_root = manifest_root_array(handle)?;
        Ok(Self {
            asset_definition_id: handle.asset_definition_id.clone(),
            scope: handle.scope.clone(),
            subject: ModelHandleSubject {
                account: handle.subject.account.clone(),
                origin_dsid: handle.subject.origin_dsid,
            },
            budget: ModelHandleBudget {
                remaining: handle.budget.remaining.clone(),
                per_use: handle.budget.per_use.clone(),
            },
            handle_era: handle.handle_era,
            sub_nonce: handle.sub_nonce,
            group_binding: ModelGroupBinding {
                composability_group_id: handle.group_binding.composability_group_id.clone(),
                epoch_id: handle.group_binding.epoch_id,
            },
            target_lane: handle.target_lane,
            axt_binding: AxtBinding::new(binding),
            manifest_view_root,
            expiry_slot: handle.expiry_slot,
            max_clock_skew_ms: handle.max_clock_skew_ms,
            issuer_context: handle.issuer_context,
            issuer_signature: handle.issuer_signature.clone(),
        })
    }
}
/// Capability subject metadata.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct HandleSubject {
    /// Canonical I105 account identifier of the spender.
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
    /// Exact asset definition authorized by the handle and proof statement.
    pub asset_definition_id: AssetDefinitionId,
    /// Operation kind (e.g., "transfer").
    pub kind: String,
    /// Origin account id in canonical I105 form.
    pub from: String,
    /// Destination account id in canonical I105 form.
    pub to: String,
    /// Cleartext amount, or `None` when the proof carries a hidden amount.
    pub amount: Option<Quantity>,
}
/// Validate context-free invariants of a remote spend intent.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] for empty or non-canonical operation and account strings, and
/// [`VMError::PermissionDenied`] for an explicit zero amount.
pub fn validate_remote_spend_intent(intent: &RemoteSpendIntent) -> Result<(), VMError> {
    if intent.op.kind != "transfer" {
        return Err(VMError::NoritoInvalid);
    }
    for account in [&intent.op.from, &intent.op.to] {
        if canonical_account_id(account).is_none() {
            return Err(VMError::NoritoInvalid);
        }
    }
    if intent.op.amount.as_ref().is_some_and(Quantity::is_zero) {
        return Err(VMError::PermissionDenied);
    }
    Ok(())
}

fn canonical_account_id(value: &str) -> Option<AccountId> {
    let parsed = AccountId::parse_encoded(value).ok()?;
    (parsed.canonical() == value).then(|| parsed.into_account_id())
}

/// Require a registered asset policy's balance scope to match the intent dataspace.
///
/// Globally scoped assets belong to the universal dataspace. A
/// dataspace-restricted definition selects the exact signed intent/proof
/// dataspace bucket. Callers must first establish that the asset definition is
/// registered in committed state and derive `resolved_scope` from its balance
/// policy; the opaque asset-definition identifier carries no routing meaning.
///
/// # Errors
///
/// Returns [`VMError::PermissionDenied`] when the policy-derived scope does not
/// match `asset_dsid`.
pub fn validate_remote_spend_asset_scope(
    asset_dsid: DataSpaceId,
    resolved_scope: AssetBalanceScope,
) -> Result<(), VMError> {
    let matches = match resolved_scope {
        AssetBalanceScope::Global => asset_dsid == DataSpaceId::UNIVERSAL,
        AssetBalanceScope::Dataspace(dataspace) => asset_dsid == dataspace,
    };
    matches.then_some(()).ok_or(VMError::PermissionDenied)
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
            asset_definition_id: intent.op.asset_definition_id.clone(),
            kind: intent.op.kind.clone(),
            from: intent.op.from.clone(),
            to: intent.op.to.clone(),
            amount: intent.op.amount.clone(),
        },
    })
}
/// Require a proof-bound commitment to the exact runtime remote-spend statement.
///
/// This is a semantic membership check only. The caller must first verify the
/// FASTPQ proof cryptographically. Keeping the check separate ensures it still
/// runs for every handle when a verified proof is reused from a per-dataspace
/// cache or proof fragment.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] when the proof is not a canonical AXT
/// envelope, and [`VMError::PermissionDenied`] when it lacks the exact
/// handle identity/asset/operation/account/amount commitment.
pub fn validate_remote_spend_intent_commitment(
    handle: &AssetHandle,
    intent: &RemoteSpendIntent,
    effective_amount: &Quantity,
    proof: &ProofBlob,
) -> Result<(), VMError> {
    if handle.asset_definition_id != intent.op.asset_definition_id {
        return Err(VMError::PermissionDenied);
    }
    validate_remote_spend_intent_commitment_components(
        expected_remote_spend_intent_commitment_v1(handle, intent, effective_amount)?,
        &proof.payload,
    )
}
/// Require a proof-bound commitment for a persisted data-model remote spend.
///
/// The caller must first verify the FASTPQ proof cryptographically.
///
/// # Errors
///
/// Returns the same error classification as
/// [`validate_remote_spend_intent_commitment`].
pub fn validate_model_remote_spend_intent_commitment(
    handle: &ModelAssetHandle,
    intent: &ModelRemoteSpendIntent,
    effective_amount: &Quantity,
    proof: &ModelProofBlob,
) -> Result<(), VMError> {
    if handle.asset_definition_id != intent.op.asset_definition_id {
        return Err(VMError::PermissionDenied);
    }
    validate_remote_spend_intent_commitment_components(
        expected_model_remote_spend_intent_commitment_v1(handle, intent, effective_amount),
        &proof.payload,
    )
}
/// Require an exact persisted remote-spend commitment from cached, verified proof facts.
///
/// This performs the same semantic membership check as
/// [`validate_model_remote_spend_intent_commitment`] without decoding or
/// scanning the proof payload again.
///
/// # Errors
///
/// Returns [`VMError::PermissionDenied`] when the proof facts do not contain
/// the exact handle identity/asset/operation/account/amount commitment.
pub fn validate_model_remote_spend_intent_commitment_from_proof_facts(
    handle: &ModelAssetHandle,
    intent: &ModelRemoteSpendIntent,
    effective_amount: &Quantity,
    facts: &AxtProofUseFacts,
) -> Result<(), VMError> {
    if facts.dsid != intent.asset_dsid
        || handle.asset_definition_id != intent.op.asset_definition_id
    {
        return Err(VMError::PermissionDenied);
    }
    validate_remote_spend_intent_commitment_components_from_commitments(
        expected_model_remote_spend_intent_commitment_v1(handle, intent, effective_amount),
        &facts.remote_spend_intent_commitments,
    )
}

/// Derive the commitment expected for one concrete pointer-ABI handle use.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] if the handle descriptor binding is not
/// exactly 32 bytes, and [`VMError::PermissionDenied`] if the intent names an
/// asset other than the one authenticated by the handle issuer.
pub fn expected_remote_spend_intent_commitment_v1(
    handle: &AssetHandle,
    intent: &RemoteSpendIntent,
    effective_amount: &Quantity,
) -> Result<[u8; 32], VMError> {
    if handle.asset_definition_id != intent.op.asset_definition_id {
        return Err(VMError::PermissionDenied);
    }
    let binding = handle.binding_array().ok_or(VMError::NoritoInvalid)?;
    let replay_key = AxtHandleReplayKey::from_parts(
        intent.asset_dsid,
        handle.issuer_context.asset_definition_incarnation,
        binding,
        handle.handle_era,
        handle.sub_nonce,
        handle.target_lane,
    );
    Ok(compute_remote_spend_intent_commitment_v1(
        replay_key,
        &handle.asset_definition_id,
        &intent.op.kind,
        &intent.op.from,
        &intent.op.to,
        effective_amount,
    ))
}

/// Derive the commitment expected for one concrete persisted handle use.
#[must_use]
pub fn expected_model_remote_spend_intent_commitment_v1(
    handle: &ModelAssetHandle,
    intent: &ModelRemoteSpendIntent,
    effective_amount: &Quantity,
) -> [u8; 32] {
    compute_remote_spend_intent_commitment_v1(
        AxtHandleReplayKey::from_handle(intent.asset_dsid, handle),
        &handle.asset_definition_id,
        &intent.op.kind,
        &intent.op.from,
        &intent.op.to,
        effective_amount,
    )
}
fn validate_remote_spend_intent_commitment_components(
    expected: [u8; 32],
    proof_payload: &[u8],
) -> Result<(), VMError> {
    if proof_payload.len() > MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES {
        return Err(VMError::NoritoInvalid);
    }
    let envelope = decode_canonical_norito::<ModelAxtProofEnvelope>(proof_payload)
        .map_err(|_| VMError::NoritoInvalid)?;
    let binding = envelope
        .fastpq_binding
        .as_ref()
        .ok_or(VMError::PermissionDenied)?;
    validate_remote_spend_intent_commitment_components_from_commitments(
        expected,
        &binding.remote_spend_intent_commitments,
    )
}
fn validate_remote_spend_intent_commitment_components_from_commitments(
    expected: [u8; 32],
    remote_spend_intent_commitments: &[[u8; 32]],
) -> Result<(), VMError> {
    remote_spend_intent_commitments
        .binary_search(&expected)
        .map(|_| ())
        .map_err(|_| VMError::PermissionDenied)
}
/// Wrapper around proof artifacts provided by dataspace verifiers.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct ProofBlob {
    /// Raw proof bytes, bounded by the shared AXT proof-envelope payload limit.
    pub payload: Vec<u8>,
    /// Outer mirror of the proof-bound optional expiry slot.
    ///
    /// `None` is an authenticated no-expiry sentinel. Proof-aware hosts must
    /// exact-compare this value with the proof metadata before applying the
    /// current AXT policy slot's freshness check.
    #[norito(required)]
    pub expiry_slot: Option<u64>,
}
/// Validate context-free proof-blob invariants.
///
/// Proof schema, dataspace binding, manifest binding, freshness relative to a
/// current slot, and cryptographic validity require host context.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] when proof bytes are empty or oversized,
/// or an explicit expiry uses the forbidden zero sentinel.
pub fn validate_proof_blob(proof: &ProofBlob) -> Result<(), VMError> {
    if proof.payload.is_empty()
        || proof.payload.len() > MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES
        || proof.expiry_slot == Some(0)
    {
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
/// Preflight an AXT proof envelope as FastPQ V1 material without pinning a manifest root.
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
        && binding.remote_spend_intent_commitments.len()
            <= iroha_data_model::nexus::MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1
        && binding
            .remote_spend_intent_commitments
            .windows(2)
            .all(|pair| pair[0] < pair[1])
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
/// The current implementation prefixes the descriptor bytes with a stable domain separator and
/// hashes the concatenation using the Poseidon2 sponge (rate 2, capacity 1). Byte packing appends
/// a `0x01` delimiter and zero-pads to an eight-byte boundary before the sponge's field-level +1
/// padding. This matches the normative definition documented in `nexus.md`.
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
        if usage.handle.asset_definition_id != usage.intent.op.asset_definition_id {
            return Err(VMError::PermissionDenied);
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
            prev.intent.asset_dsid == usage.intent.asset_dsid
                && prev.handle.handle_era == usage.handle.handle_era
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
        for dsid in &self.expected_dsids {
            if self.descriptor.touch_for(dsid).is_some() && !self.touches.contains_key(dsid) {
                return Err(VMError::PermissionDenied);
            }
        }
        let mut seen_nonces: BTreeSet<(DataSpaceId, [u8; 32], u64, LaneId, u64)> = BTreeSet::new();
        let mut accumulators: BTreeMap<HandleBudgetKey, ModelAxtHandleBudgetRecord> =
            BTreeMap::new();
        for usage in &self.handles {
            if usage.handle.asset_definition_id != usage.intent.op.asset_definition_id {
                return Err(VMError::PermissionDenied);
            }
            let binding = usage.handle.binding_array().ok_or(VMError::NoritoInvalid)?;
            let key = (
                usage.intent.asset_dsid,
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
            let budget_key = try_handle_budget_key(usage.intent.asset_dsid, &usage.handle)?;
            accumulators
                .entry(budget_key.clone())
                .or_insert_with(ModelAxtHandleBudgetRecord::empty)
                .try_consume(&budget_key, &usage.amount, 0)
                .map_err(|_| VMError::PermissionDenied)?;
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
        let handle = ModelAssetHandle::try_from(&usage.handle)?;
        let intent = ModelRemoteSpendIntent {
            asset_dsid: usage.intent.asset_dsid,
            op: ModelSpendOp {
                asset_definition_id: usage.intent.op.asset_definition_id.clone(),
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
/// Canonical consensus key used to aggregate an issuer-signed handle family.
pub type HandleBudgetKey = ModelAxtHandleBudgetKey;
/// Derive the canonical consensus budget key from a pointer-ABI handle.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] for malformed fixed-width fields and
/// [`VMError::PermissionDenied`] when `asset_dsid` is not the dataspace
/// authenticated by the issuer context.
pub fn try_handle_budget_key(
    asset_dsid: DataSpaceId,
    handle: &AssetHandle,
) -> Result<HandleBudgetKey, VMError> {
    if handle.issuer_context.asset_dsid != asset_dsid {
        return Err(VMError::PermissionDenied);
    }
    let model = ModelAssetHandle::try_from(handle)?;
    Ok(HandleBudgetKey::from_handle(&model))
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
    use iroha_data_model::domain::DomainId;
    const ACCOUNT_FROM_LITERAL: &str = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    const ACCOUNT_TO_LITERAL: &str = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";
    fn quantity(value: u128) -> Quantity {
        value
            .to_string()
            .parse()
            .expect("test amount is a canonical quantity")
    }
    fn test_asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("test asset domain"),
            "rose".parse().expect("test asset name"),
        )
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
            asset_definition_id: test_asset_definition_id(),
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
            issuer_context: AxtHandleIssuerContextV1 {
                asset_dsid: dsid,
                ..AxtHandleIssuerContextV1::default()
            },
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
    fn asset_handle_subject_requires_canonical_account_id() {
        let dsid = DataSpaceId::new(7);
        let valid = sample_handle(dsid, [0x11; 32], 10, Some(5));
        assert_eq!(validate_asset_handle(&valid), Ok(()));

        let mut malformed = ACCOUNT_FROM_LITERAL.to_owned();
        malformed.pop();
        let invalid_accounts = [
            ("alias", "spender@payments".to_owned()),
            ("malformed", malformed),
            (
                "noncanonical",
                ACCOUNT_FROM_LITERAL.replacen("sora", "ｓｏｒａ", 1),
            ),
            ("whitespace", format!(" {ACCOUNT_FROM_LITERAL}")),
        ];
        for (case, account) in invalid_accounts {
            let mut invalid = valid.clone();
            invalid.subject.account = account;
            assert_eq!(
                validate_asset_handle(&invalid),
                Err(VMError::NoritoInvalid),
                "{case} subject account must fail"
            );
        }
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
        assert_eq!(
            validate_proof_blob(&ProofBlob {
                payload: vec![0; MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES + 1],
                expiry_slot: None,
            }),
            Err(VMError::NoritoInvalid)
        );
    }
    #[test]
    fn proof_blob_requires_explicit_nullable_expiry_slot() {
        #[derive(Encode)]
        struct ProofBlobWithoutExpiry {
            payload: Vec<u8>,
        }

        let omitted = encode_canonical_norito(&ProofBlobWithoutExpiry { payload: vec![1] })
            .expect("encode pre-release proof blob without expiry slot");
        assert_eq!(
            decode_canonical_norito::<ProofBlob>(&omitted),
            Err(VMError::NoritoInvalid),
            "V1 must reject a proof blob that omits its nullable expiry slot"
        );

        let explicit_none = ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        };
        let encoded =
            encode_canonical_norito(&explicit_none).expect("encode explicit no-expiry proof blob");
        assert_eq!(
            decode_canonical_norito::<ProofBlob>(&encoded),
            Ok(explicit_none),
            "an explicit None remains the authenticated no-expiry value"
        );
    }
    fn sample_intent(dsid: DataSpaceId, amount: Option<u128>) -> RemoteSpendIntent {
        RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                asset_definition_id: test_asset_definition_id(),
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
        for kind in ["mint", "Transfer", " transfer", "transfer "] {
            let mut invalid = valid.clone();
            invalid.op.kind = kind.to_owned();
            assert_eq!(
                validate_remote_spend_intent(&invalid),
                Err(VMError::NoritoInvalid),
                "non-transfer operation {kind:?} must fail closed"
            );
        }
    }

    #[test]
    fn remote_spend_asset_scope_requires_exact_authoritative_dataspace() {
        let dsid = DataSpaceId::new(7);
        assert_eq!(
            validate_remote_spend_asset_scope(dsid, AssetBalanceScope::Dataspace(dsid)),
            Ok(())
        );
        assert_eq!(
            validate_remote_spend_asset_scope(DataSpaceId::UNIVERSAL, AssetBalanceScope::Global,),
            Ok(())
        );
        assert_eq!(
            validate_remote_spend_asset_scope(
                dsid,
                AssetBalanceScope::Dataspace(DataSpaceId::new(8)),
            ),
            Err(VMError::PermissionDenied)
        );
        assert_eq!(
            validate_remote_spend_asset_scope(dsid, AssetBalanceScope::Global),
            Err(VMError::PermissionDenied)
        );
    }
    #[test]
    fn remote_spend_intent_from_and_to_require_canonical_account_ids() {
        let dsid = DataSpaceId::new(7);
        let valid = sample_intent(dsid, Some(1));
        assert_eq!(validate_remote_spend_intent(&valid), Ok(()));

        let mut malformed = ACCOUNT_FROM_LITERAL.to_owned();
        malformed.pop();
        let invalid_accounts = [
            ("alias", "spender@payments".to_owned()),
            ("malformed", malformed),
            (
                "noncanonical",
                ACCOUNT_FROM_LITERAL.replacen("sora", "ｓｏｒａ", 1),
            ),
            ("whitespace", format!("{ACCOUNT_FROM_LITERAL} ")),
        ];
        for field in ["from", "to"] {
            for (case, account) in &invalid_accounts {
                let mut invalid = valid.clone();
                match field {
                    "from" => invalid.op.from.clone_from(account),
                    "to" => invalid.op.to.clone_from(account),
                    _ => unreachable!(),
                }
                assert_eq!(
                    validate_remote_spend_intent(&invalid),
                    Err(VMError::NoritoInvalid),
                    "{case} {field} account must fail"
                );
            }
        }
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
            remote_spend_intent_commitments: Vec::new(),
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
    fn proof_for_remote_spends(
        intents: &[(&AssetHandle, &RemoteSpendIntent, Quantity)],
    ) -> ProofBlob {
        let dsid = intents
            .first()
            .expect("remote-spend proof fixture is non-empty")
            .1
            .asset_dsid;
        let mut binding = sample_fastpq_binding(dsid);
        binding.remote_spend_intent_commitments = intents
            .iter()
            .map(|(handle, intent, amount)| {
                expected_remote_spend_intent_commitment_v1(handle, intent, amount)
                    .expect("fixture handle binding")
            })
            .collect();
        binding.remote_spend_intent_commitments.sort_unstable();
        binding.remote_spend_intent_commitments.dedup();
        ProofBlob {
            payload: norito::to_bytes(&AxtProofEnvelope {
                dsid,
                manifest_root: [0xAB; 32],
                da_commitment: None,
                proof: vec![0x01, 0x02],
                fastpq_binding: Some(binding),
                committed_amount: None,
                amount_commitment: None,
            })
            .expect("encode remote-spend proof envelope"),
            expiry_slot: Some(10),
        }
    }
    #[test]
    fn proof_payload_decode_helpers_reject_oversized_canonical_envelope() {
        let dsid = DataSpaceId::new(94);
        let descriptor_binding = [0x94; 32];
        let intent = sample_intent(dsid, Some(5));
        let amount = quantity(5);
        let handle = sample_handle(dsid, descriptor_binding, 10, Some(10));
        let proof = proof_for_remote_spends(&[(&handle, &intent, amount.clone())]);
        let mut envelope = decode_canonical_norito::<AxtProofEnvelope>(&proof.payload)
            .expect("decode canonical remote-spend proof");
        envelope.proof = vec![0xA5; MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES];
        envelope.committed_amount = Some(5);
        envelope.amount_commitment = Some([0x5A; 32]);
        let oversized_payload =
            encode_canonical_norito(&envelope).expect("encode oversized canonical proof envelope");
        assert!(oversized_payload.len() > MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES);
        let oversized = ProofBlob {
            payload: oversized_payload,
            expiry_slot: Some(10),
        };

        assert_eq!(validate_proof_blob(&oversized), Err(VMError::NoritoInvalid));
        assert_eq!(
            resolve_handle_amount(&intent, Some(&oversized)),
            Err(HandleAmountResolutionError::InvalidProofEnvelope)
        );
        assert_eq!(
            validate_remote_spend_intent_commitment(&handle, &intent, &amount, &oversized),
            Err(VMError::NoritoInvalid)
        );
        let expected_raw = derive_amount_commitment_from_normalized_payload(
            dsid,
            &amount,
            Some(&oversized.payload),
        );
        assert_eq!(
            derive_amount_commitment(dsid, &amount, Some(&oversized.payload)),
            expected_raw,
            "oversized envelopes must be treated as opaque commitment bytes without decoding"
        );
    }
    #[test]
    fn remote_spend_intent_commitment_rejects_substitution_and_supports_proof_reuse() {
        let dsid = DataSpaceId::new(93);
        let descriptor_binding = [0x93; 32];
        let clear = sample_intent(dsid, Some(5));
        let mut hidden = sample_intent(dsid, None);
        hidden.op.to = ACCOUNT_FROM_LITERAL.to_owned();
        let clear_amount = quantity(5);
        let hidden_amount = quantity(7);
        let clear_handle = sample_handle(dsid, descriptor_binding, 10, Some(10));
        let mut hidden_handle = clear_handle.clone();
        hidden_handle.sub_nonce += 1;
        let proof = proof_for_remote_spends(&[
            (&clear_handle, &clear, clear_amount.clone()),
            (&hidden_handle, &hidden, hidden_amount.clone()),
        ]);
        assert_eq!(
            validate_remote_spend_intent_commitment(&clear_handle, &clear, &clear_amount, &proof,),
            Ok(())
        );
        assert_eq!(
            validate_remote_spend_intent_commitment(
                &hidden_handle,
                &hidden,
                &hidden_amount,
                &proof,
            ),
            Ok(())
        );
        let envelope = decode_canonical_norito::<AxtProofEnvelope>(&proof.payload)
            .expect("decode reusable proof once");
        let facts = AxtProofUseFacts::from_verified_envelope(envelope);
        let model_clear = ModelRemoteSpendIntent {
            asset_dsid: clear.asset_dsid,
            op: ModelSpendOp {
                asset_definition_id: clear.op.asset_definition_id.clone(),
                kind: clear.op.kind.clone(),
                from: clear.op.from.clone(),
                to: clear.op.to.clone(),
                amount: clear.op.amount.clone(),
            },
        };
        let model_handle = AxtHandleFragment::try_from(&HandleUsage {
            handle: clear_handle.clone(),
            intent: clear.clone(),
            proof: None,
            amount: clear_amount.clone(),
            amount_commitment: None,
        })
        .expect("convert fixture handle")
        .handle;
        assert_eq!(
            validate_model_remote_spend_intent_commitment_from_proof_facts(
                &model_handle,
                &model_clear,
                &clear_amount,
                &facts,
            ),
            Ok(())
        );
        let mut reincarnated_model_handle = model_handle.clone();
        reincarnated_model_handle
            .issuer_context
            .asset_definition_incarnation = AxtAssetIncarnationV1::derive(
            &reincarnated_model_handle.issuer_context.network_id,
            &reincarnated_model_handle.asset_definition_id,
            &iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::new(b"ivm-abi-reincarnated-remote-spend-registration"),
            ),
            &Hash::new(b"ivm-abi-reincarnated-remote-spend-execution"),
            1,
        );
        assert_eq!(
            validate_model_remote_spend_intent_commitment_from_proof_facts(
                &reincarnated_model_handle,
                &model_clear,
                &clear_amount,
                &facts,
            ),
            Err(VMError::PermissionDenied),
            "a claim for a retired asset incarnation must not authorize the current handle"
        );
        let mut substituted_model = model_clear.clone();
        substituted_model.op.to = ACCOUNT_FROM_LITERAL.to_owned();
        assert_eq!(
            validate_model_remote_spend_intent_commitment_from_proof_facts(
                &model_handle,
                &substituted_model,
                &clear_amount,
                &facts,
            ),
            Err(VMError::PermissionDenied)
        );
        for field in ["kind", "from", "to"] {
            let mut substituted = clear.clone();
            match field {
                "kind" => substituted.op.kind = "mint".to_owned(),
                "from" => substituted.op.from = ACCOUNT_TO_LITERAL.to_owned(),
                "to" => substituted.op.to = ACCOUNT_FROM_LITERAL.to_owned(),
                _ => unreachable!(),
            }
            assert_eq!(
                validate_remote_spend_intent_commitment(
                    &clear_handle,
                    &substituted,
                    &clear_amount,
                    &proof,
                ),
                Err(VMError::PermissionDenied),
                "substituted {field} must not reuse the proof"
            );
        }
        let mut substituted_dsid = clear.clone();
        substituted_dsid.asset_dsid = DataSpaceId::new(94);
        assert_eq!(
            validate_remote_spend_intent_commitment(
                &clear_handle,
                &substituted_dsid,
                &clear_amount,
                &proof,
            ),
            Err(VMError::PermissionDenied),
            "substituted asset dataspace must not reuse the proof"
        );
        assert_eq!(
            validate_remote_spend_intent_commitment(&clear_handle, &clear, &quantity(6), &proof,),
            Err(VMError::PermissionDenied)
        );
        let mut substituted_asset = clear.clone();
        substituted_asset.op.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("test asset domain"),
            "iris".parse().expect("test asset name"),
        );
        assert_eq!(
            validate_remote_spend_intent_commitment(
                &clear_handle,
                &substituted_asset,
                &clear_amount,
                &proof,
            ),
            Err(VMError::PermissionDenied),
            "substituted asset definition must not reuse the proof"
        );
        let mut substituted_handle = clear_handle.clone();
        substituted_handle.axt_binding = vec![0x94; 32];
        assert_eq!(
            validate_remote_spend_intent_commitment(
                &substituted_handle,
                &clear,
                &clear_amount,
                &proof,
            ),
            Err(VMError::PermissionDenied)
        );
        let mut reincarnated_handle = clear_handle.clone();
        reincarnated_handle
            .issuer_context
            .asset_definition_incarnation = AxtAssetIncarnationV1::derive(
            &reincarnated_handle.issuer_context.network_id,
            &reincarnated_handle.asset_definition_id,
            &iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::new(b"ivm-abi-reincarnated-remote-spend-registration"),
            ),
            &Hash::new(b"ivm-abi-reincarnated-remote-spend-execution"),
            1,
        );
        assert_eq!(
            validate_remote_spend_intent_commitment(
                &reincarnated_handle,
                &clear,
                &clear_amount,
                &proof,
            ),
            Err(VMError::PermissionDenied),
            "a proof for the retired incarnation must not authorize a current-incarnation handle"
        );
        let mut second_handle = clear_handle.clone();
        second_handle.sub_nonce += 1;
        assert_eq!(
            validate_remote_spend_intent_commitment(&second_handle, &clear, &clear_amount, &proof,),
            Err(VMError::PermissionDenied),
            "a proof for one handle must not authorize a new handle identity"
        );

        let clear_commitment =
            expected_remote_spend_intent_commitment_v1(&clear_handle, &clear, &clear_amount)
                .expect("fixture commitment");
        let hidden_commitment =
            expected_remote_spend_intent_commitment_v1(&hidden_handle, &hidden, &hidden_amount)
                .expect("fixture commitment");
        assert_eq!(
            facts.validate_remote_spend_consumption(&[hidden_commitment, clear_commitment]),
            Ok(())
        );
        assert_eq!(
            facts.validate_remote_spend_consumption(&[clear_commitment, clear_commitment]),
            Err(VMError::PermissionDenied),
            "one proof claim cannot be consumed twice"
        );
        assert_eq!(
            facts.validate_remote_spend_consumption(&[clear_commitment]),
            Err(VMError::PermissionDenied),
            "an unconsumed proof claim must fail closed"
        );

        // An empty intent set remains valid metadata for generic VERIFY_DS
        // proof flows, but it must never authorize USE_ASSET_HANDLE.
        let empty_proof = proof_with_amount(dsid, None, None);
        let mut empty_envelope =
            norito::decode_from_bytes::<AxtProofEnvelope>(&empty_proof.payload)
                .expect("decode empty remote-spend binding proof");
        preflight_fastpq_v1_proof_envelope_for_manifest(
            &empty_envelope,
            dsid,
            empty_envelope.manifest_root,
        )
        .expect("generic FastPQ proof accepts an empty remote-spend binding");
        assert_eq!(
            validate_remote_spend_intent_commitment(
                &clear_handle,
                &clear,
                &clear_amount,
                &empty_proof,
            ),
            Err(VMError::PermissionDenied)
        );

        empty_envelope.fastpq_binding = None;
        let unbound_proof = ProofBlob {
            payload: norito::to_bytes(&empty_envelope).expect("encode unbound proof envelope"),
            expiry_slot: empty_proof.expiry_slot,
        };
        assert_eq!(
            validate_remote_spend_intent_commitment(
                &clear_handle,
                &clear,
                &clear_amount,
                &unbound_proof,
            ),
            Err(VMError::PermissionDenied)
        );
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
    fn cached_proof_facts_match_payload_amount_resolution() {
        let dsid = DataSpaceId::new(96);
        let intent = sample_intent(dsid, None);
        let proof = proof_with_derived_amount_commitment(dsid, 31);
        let expected = resolve_handle_amount(&intent, Some(&proof)).expect("payload resolution");
        let envelope = decode_canonical_norito::<AxtProofEnvelope>(&proof.payload)
            .expect("decode canonical proof once");
        let facts = AxtProofUseFacts::from_verified_envelope(envelope);
        assert_eq!(
            resolve_handle_amount_components_from_proof_facts(
                dsid,
                intent.op.amount.as_ref(),
                &facts,
            ),
            Ok(expected)
        );
        assert_eq!(
            resolve_handle_amount_components_from_proof_facts(
                DataSpaceId::new(97),
                intent.op.amount.as_ref(),
                &facts,
            ),
            Err(HandleAmountResolutionError::InvalidProofEnvelope)
        );
        assert_eq!(
            resolve_handle_amount_components_from_proof_facts(dsid, Some(&quantity(32)), &facts,),
            Err(HandleAmountResolutionError::Mismatch)
        );
    }
    #[test]
    fn cached_proof_facts_preserve_cleartext_supplied_commitment() {
        let dsid = DataSpaceId::new(97);
        let amount = quantity(23);
        let intent = sample_intent(dsid, Some(23));
        let mut proof = proof_with_amount(dsid, None, None);
        let commitment = derive_amount_commitment(dsid, &amount, Some(&proof.payload));
        let mut envelope = decode_canonical_norito::<AxtProofEnvelope>(&proof.payload)
            .expect("decode proof for commitment");
        envelope.amount_commitment = Some(commitment);
        proof.payload = encode_canonical_norito(&envelope).expect("encode committed proof");

        let expected = resolve_handle_amount(&intent, Some(&proof)).expect("payload resolution");
        let envelope = decode_canonical_norito::<AxtProofEnvelope>(&proof.payload)
            .expect("decode canonical proof once");
        let facts = AxtProofUseFacts::from_verified_envelope(envelope);
        assert_eq!(
            resolve_handle_amount_components_from_proof_facts(
                dsid,
                intent.op.amount.as_ref(),
                &facts,
            ),
            Ok(expected)
        );
        assert_eq!(
            resolve_handle_amount_components_from_proof_facts(dsid, Some(&quantity(24)), &facts,),
            Err(HandleAmountResolutionError::CommitmentMismatch)
        );
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
    fn commit_keeps_budgets_separate_for_distinct_signed_assets() {
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
        let first_handle = sample_handle(dsid, binding, 100, None);
        let mut second_handle = first_handle.clone();
        second_handle.sub_nonce = second_handle.sub_nonce.saturating_add(1);
        second_handle.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("test asset domain"),
            "iris".parse().expect("test asset name"),
        );
        let mut second_intent = sample_intent(dsid, Some(60));
        second_intent.op.asset_definition_id = second_handle.asset_definition_id.clone();
        let proof = Some(ProofBlob {
            payload: vec![9],
            expiry_slot: None,
        });
        state
            .record_handle(HandleUsage {
                handle: first_handle,
                intent: sample_intent(dsid, Some(60)),
                proof: proof.clone(),
                amount: quantity(60),
                amount_commitment: None,
            })
            .expect("first asset usage within its budget");
        state
            .record_handle(HandleUsage {
                handle: second_handle,
                intent: second_intent,
                proof,
                amount: quantity(60),
                amount_commitment: None,
            })
            .expect("second asset usage within its independent budget");

        assert_eq!(state.validate_commit(), Ok(()));
    }
    #[test]
    fn handle_budget_key_groups_sub_nonces_but_separates_signed_assets() {
        let dsid = DataSpaceId::new(6);
        let binding = [0x5A; 32];
        let first = sample_handle(dsid, binding, 100, None);
        let mut next_nonce = first.clone();
        next_nonce.sub_nonce = next_nonce.sub_nonce.saturating_add(1);

        let first_key = try_handle_budget_key(dsid, &first).expect("valid handle key");
        let next_nonce_key =
            try_handle_budget_key(dsid, &next_nonce).expect("valid next-nonce handle key");
        assert_eq!(
            first_key, next_nonce_key,
            "sub-nonces share the issuer-signed aggregate budget"
        );

        let mut other_asset = next_nonce;
        other_asset.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("test asset domain"),
            "iris".parse().expect("test asset name"),
        );
        let other_asset_key =
            try_handle_budget_key(dsid, &other_asset).expect("valid other-asset handle key");
        assert_ne!(
            first_key, other_asset_key,
            "distinct issuer-signed assets must not share a budget"
        );
    }
    #[test]
    fn handle_budget_key_is_identical_for_abi_and_model_handles() {
        let dsid = DataSpaceId::new(6);
        let mut abi_handle = sample_handle(dsid, [0x5A; 32], 123, Some(17));
        abi_handle.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("test asset domain"),
            "iris".parse().expect("test asset name"),
        );
        abi_handle.scope = vec!["burn".into(), "transfer".into()];
        abi_handle.subject.origin_dsid = Some(DataSpaceId::new(42));
        abi_handle.handle_era = 33;
        abi_handle.group_binding.composability_group_id = vec![0x44; 32];
        abi_handle.group_binding.epoch_id = 71;
        abi_handle.target_lane = LaneId::new(9);
        abi_handle.manifest_view_root = vec![0xA5; 32];
        abi_handle.expiry_slot = 456;
        abi_handle.max_clock_skew_ms = Some(987);
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            Hash::new(b"ivm-abi-budget-key-network"),
        ));
        let registration_header_hash =
            iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::new(b"ivm-abi-budget-key-asset-registration"),
            );
        let execution_identity = Hash::new(b"ivm-abi-budget-key-asset-execution");
        abi_handle.issuer_context = AxtHandleIssuerContextV1 {
            network_id,
            asset_dsid: dsid,
            asset_definition_incarnation: AxtAssetIncarnationV1::derive(
                &network_id,
                &abi_handle.asset_definition_id,
                &registration_header_hash,
                &execution_identity,
                0,
            ),
            issuer: iroha_data_model::nexus::UniversalAccountId::from_hash(Hash::new(
                b"ivm-abi-budget-key-issuer",
            )),
            issuer_manifest_root: [0xB1; 32],
            code_root: [0xB2; 32],
            abi_version: 1,
            abi_hash: [0xB3; 32],
        };
        let mut intent = sample_intent(dsid, Some(1));
        intent.op.asset_definition_id = abi_handle.asset_definition_id.clone();
        let model_handle = AxtHandleFragment::try_from(&HandleUsage {
            handle: abi_handle.clone(),
            intent,
            proof: None,
            amount: quantity(1),
            amount_commitment: None,
        })
        .expect("canonical model handle")
        .handle;

        let abi_key = try_handle_budget_key(dsid, &abi_handle).expect("canonical pointer-ABI key");
        let model_key = HandleBudgetKey::from_handle(&model_handle);
        assert_eq!(
            abi_key, model_key,
            "ABI and persisted handles must normalize to one budget identity"
        );

        let assert_model_mutation_changes_key = |mutated: ModelAssetHandle, field: &str| {
            assert_ne!(
                abi_key,
                HandleBudgetKey::from_handle(&mutated),
                "{field} must remain part of the normalized budget identity"
            );
        };
        let mut mutated = model_handle.clone();
        mutated.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("test asset domain"),
            "rose".parse().expect("test asset name"),
        );
        assert_model_mutation_changes_key(mutated, "asset definition");
        let mut mutated = model_handle.clone();
        mutated.subject.origin_dsid = Some(DataSpaceId::new(43));
        assert_model_mutation_changes_key(mutated, "subject origin");
        let mut mutated = model_handle.clone();
        mutated.group_binding.composability_group_id = vec![0x45; 32];
        assert_model_mutation_changes_key(mutated, "composability group");
        let mut mutated = model_handle.clone();
        mutated.group_binding.epoch_id = 72;
        assert_model_mutation_changes_key(mutated, "group epoch");
        let mut mutated = model_handle.clone();
        mutated.target_lane = LaneId::new(10);
        assert_model_mutation_changes_key(mutated, "target lane");
        let mut mutated = model_handle.clone();
        mutated.manifest_view_root = [0xA6; 32];
        assert_model_mutation_changes_key(mutated, "manifest root");
        let mut mutated = model_handle.clone();
        mutated.budget.remaining = quantity(124);
        assert_model_mutation_changes_key(mutated, "remaining budget");
        let mut mutated = model_handle.clone();
        mutated.budget.per_use = Some(quantity(18));
        assert_model_mutation_changes_key(mutated, "per-use budget");
        let mut mutated = model_handle.clone();
        mutated.expiry_slot = 457;
        assert_model_mutation_changes_key(mutated, "expiry slot");
        let mut mutated = model_handle.clone();
        mutated.max_clock_skew_ms = Some(988);
        assert_model_mutation_changes_key(mutated, "clock skew");
        let mut mutated = model_handle.clone();
        mutated.issuer_context.network_id =
            iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                iroha_data_model::block::BlockHeader,
            >::from_untyped_unchecked(
                Hash::new(b"ivm-abi-other-budget-key-network"),
            ));
        assert_model_mutation_changes_key(mutated, "issuer network");
        let mut mutated = model_handle.clone();
        mutated.issuer_context.asset_dsid = DataSpaceId::new(7);
        assert_model_mutation_changes_key(mutated, "issuer dataspace");
        let mut mutated = model_handle.clone();
        mutated.issuer_context.asset_definition_incarnation = AxtAssetIncarnationV1::derive(
            &mutated.issuer_context.network_id,
            &mutated.asset_definition_id,
            &iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::new(b"ivm-abi-other-asset-registration"),
            ),
            &Hash::new(b"ivm-abi-other-asset-execution"),
            0,
        );
        assert_model_mutation_changes_key(mutated, "asset-definition incarnation");
        let mut mutated = model_handle.clone();
        mutated.issuer_context.issuer = iroha_data_model::nexus::UniversalAccountId::from_hash(
            Hash::new(b"ivm-abi-other-budget-key-issuer"),
        );
        assert_model_mutation_changes_key(mutated, "issuer identity");
        let mut mutated = model_handle.clone();
        mutated.issuer_context.issuer_manifest_root = [0xB4; 32];
        assert_model_mutation_changes_key(mutated, "issuer manifest root");
        let mut mutated = model_handle.clone();
        mutated.issuer_context.code_root = [0xB5; 32];
        assert_model_mutation_changes_key(mutated, "issuer code root");
        let mut mutated = model_handle.clone();
        mutated.issuer_context.abi_version = 2;
        assert_model_mutation_changes_key(mutated, "issuer ABI version");
        let mut mutated = model_handle;
        mutated.issuer_context.abi_hash = [0xB6; 32];
        assert_model_mutation_changes_key(mutated, "issuer ABI hash");
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
    fn record_handle_rejects_asset_not_authenticated_by_handle_issuer() {
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
        let mut intent = sample_intent(dsid, Some(10));
        intent.op.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("test asset domain"),
            "iris".parse().expect("test asset name"),
        );

        assert_eq!(
            state.record_handle(HandleUsage {
                handle,
                intent,
                proof: None,
                amount: quantity(10),
                amount_commitment: None,
            }),
            Err(VMError::PermissionDenied),
            "one signed handle must not authorize a different asset in the same dataspace"
        );
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
    fn record_handle_allows_same_sub_nonce_across_dataspaces_on_same_lane() {
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
        let mut handle_a = sample_handle(ds_a, binding, 10, None);
        handle_a.target_lane = LaneId::new(1);
        let mut handle_b = sample_handle(ds_b, binding, 10, None);
        handle_b.subject.origin_dsid = handle_a.subject.origin_dsid;
        handle_b.target_lane = handle_a.target_lane;
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
            .expect("second usage accepted for different dataspace");
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
