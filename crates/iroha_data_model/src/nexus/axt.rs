//! Atomic cross-transaction (AXT) envelope and fragment types for Nexus lanes.
//!
//! These structures mirror the IVM syscall surface while providing Norito-compatible
//! schemas for WSV/block persistence and gossip replication.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use iroha_zkp_halo2::poseidon::hash_bytes as poseidon_hash_bytes;
use norito::codec::{Decode, Encode, encode_adaptive};
use thiserror::Error;

use crate::nexus::{DataSpaceId, LaneId};

/// Canonical 32-byte binding derived from an AXT descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[repr(transparent)]
pub struct AxtBinding([u8; 32]);

impl AxtBinding {
    /// Construct a binding from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the binding bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Consume the binding and return the inner array.
    #[must_use]
    pub const fn into_array(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical descriptor for an AXT envelope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtDescriptor {
    /// List of dataspace identifiers touched by the transaction.
    pub dsids: Vec<DataSpaceId>,
    /// Fine-grained access declarations for each dataspace.
    #[norito(default)]
    pub touches: Vec<AxtTouchSpec>,
}

/// Declared access set for a dataspace touched by an AXT envelope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtTouchSpec {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Logical read-set expressed as application key prefixes.
    #[norito(default)]
    pub read: Vec<String>,
    /// Logical write-set expressed as application key prefixes.
    #[norito(default)]
    pub write: Vec<String>,
}

/// Runtime manifest supplied via `AXT_TOUCH`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TouchManifest {
    /// Keys read within the dataspace during execution.
    #[norito(default)]
    pub read: Vec<String>,
    /// Keys written within the dataspace during execution.
    #[norito(default)]
    pub write: Vec<String>,
}

impl TouchManifest {
    /// Construct a touch manifest from read/write key prefixes with deterministic ordering.
    #[must_use]
    pub fn from_read_write<R, W>(read: R, write: W) -> Self
    where
        R: IntoIterator,
        R::Item: Into<String>,
        W: IntoIterator,
        W::Item: Into<String>,
    {
        fn collect_sorted<I>(iter: I) -> Vec<String>
        where
            I: IntoIterator,
            I::Item: Into<String>,
        {
            let mut values: Vec<String> = iter.into_iter().map(Into::into).collect();
            values.sort();
            values.dedup();
            values
        }

        Self {
            read: collect_sorted(read),
            write: collect_sorted(write),
        }
    }
}

/// Compute the canonical descriptor binding used by asset handles and manifests.
///
/// The descriptor bytes are prefixed with a domain separator and hashed using
/// Poseidon2 (rate 2, capacity 1, +1 padding) to produce a 32-byte digest.
///
/// # Errors
/// Returns an error if the descriptor cannot be encoded using Norito.
pub fn compute_descriptor_binding(descriptor: &AxtDescriptor) -> Result<[u8; 32], norito::Error> {
    let mut buf = b"iroha:axt:desc:v1\0".to_vec();
    let encoded = norito::to_bytes(descriptor)?;
    buf.extend_from_slice(&encoded);
    Ok(poseidon_hash_bytes(&buf))
}

impl AxtDescriptor {
    /// Deterministically compute the binding hash for this descriptor.
    ///
    /// # Errors
    /// Returns an error if the descriptor cannot be encoded.
    pub fn binding(&self) -> Result<AxtBinding, norito::Error> {
        compute_descriptor_binding(self).map(AxtBinding::new)
    }

    /// Build a descriptor with sorted dataspace/touch entries.
    #[must_use]
    pub fn builder() -> AxtDescriptorBuilder {
        AxtDescriptorBuilder::default()
    }
}

/// Deterministic builder for [`AxtDescriptor`].
#[derive(Debug, Default, Clone)]
pub struct AxtDescriptorBuilder {
    dsids: BTreeSet<DataSpaceId>,
    touches: BTreeMap<DataSpaceId, AxtTouchSpec>,
}

impl AxtDescriptorBuilder {
    /// Start an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dataspace to the descriptor.
    #[must_use]
    pub fn dataspace(mut self, dsid: DataSpaceId) -> Self {
        self.dsids.insert(dsid);
        self
    }

    /// Add or replace a touch declaration for a dataspace.
    #[must_use]
    pub fn touch<R, W>(mut self, dsid: DataSpaceId, read: R, write: W) -> Self
    where
        R: IntoIterator,
        R::Item: Into<String>,
        W: IntoIterator,
        W::Item: Into<String>,
    {
        let touch = AxtTouchSpec {
            dsid,
            read: TouchManifest::from_read_write(read, Vec::<String>::new()).read,
            write: TouchManifest::from_read_write(Vec::<String>::new(), write).write,
        };
        self.dsids.insert(dsid);
        self.touches.insert(dsid, touch);
        self
    }

    /// Build the descriptor, rejecting undeclared/duplicate dataspace or touch entries.
    ///
    /// # Errors
    /// Returns [`AxtValidationError`] if the descriptor is invalid.
    pub fn build(self) -> Result<AxtDescriptor, AxtValidationError> {
        let dsids: Vec<DataSpaceId> = self.dsids.into_iter().collect();
        let touches: Vec<AxtTouchSpec> = self.touches.into_values().collect();
        let descriptor = AxtDescriptor { dsids, touches };
        validate_descriptor(&descriptor)?;
        Ok(descriptor)
    }
}

/// Touch fragment emitted for a particular dataspace.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtTouchFragment {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Manifest captured during execution.
    pub manifest: TouchManifest,
}

/// Wrapper around proof artifacts provided by dataspace verifiers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofBlob {
    /// Raw proof bytes.
    pub payload: Vec<u8>,
    /// Optional expiry slot advertised by the prover.
    #[norito(default)]
    pub expiry_slot: Option<u64>,
}

/// Check whether the proof payload binds to the expected dataspace and manifest root.
#[must_use]
pub fn proof_matches_manifest(
    proof: &ProofBlob,
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
) -> bool {
    if manifest_root.iter().all(|byte| *byte == 0) {
        return false;
    }
    if let Ok(envelope) = norito::decode_from_bytes::<AxtProofEnvelope>(&proof.payload) {
        return envelope.dsid == dsid
            && envelope.manifest_root == manifest_root
            && envelope.manifest_root.iter().any(|byte| *byte != 0);
    }
    proof.payload.as_slice() == manifest_root
}

/// Norito envelope used to bind dataspace proofs to manifest roots and DA state.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(decode_from_slice)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtProofEnvelope {
    /// Dataspace the proof is intended for.
    pub dsid: DataSpaceId,
    /// Manifest root the proof commits to.
    pub manifest_root: [u8; 32],
    /// Optional DA commitment the proof is bound to.
    #[norito(default)]
    pub da_commitment: Option<[u8; 32]>,
    /// Backend-specific proof payload.
    #[norito(default)]
    pub proof: Vec<u8>,
    /// Optional cleartext amount committed by the proof envelope.
    #[norito(default)]
    pub committed_amount: Option<u128>,
    /// Optional commitment for hidden-amount intents.
    #[norito(default)]
    pub amount_commitment: Option<[u8; 32]>,
}

/// Proof fragment associated with a dataspace.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtProofFragment {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Proof payload provided by the dataspace.
    pub proof: ProofBlob,
}

/// Dataspace composability group binding advertised by the capability.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct GroupBinding {
    /// Domain or composability group identifier.
    pub composability_group_id: Vec<u8>,
    /// Epoch identifier linked to the handle.
    pub epoch_id: u64,
}

/// Handle budget parameters.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct HandleBudget {
    /// Remaining allowance for the capability.
    pub remaining: u128,
    /// Optional per-use cap.
    #[norito(default)]
    pub per_use: Option<u128>,
}

/// Capability subject metadata.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct HandleSubject {
    /// Account identifier of the spender (string form for now).
    pub account: String,
    /// Optional originating dataspace for cross-dataspace handles.
    #[norito(default)]
    pub origin_dsid: Option<DataSpaceId>,
}

/// Subset of the asset handle ticket encoded by dataspace capability issuers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Dataspace composability group binding.
    pub group_binding: GroupBinding,
    /// Lane the handle is authorised to execute on.
    pub target_lane: LaneId,
    /// Poseidon-style binding of this handle to a descriptor.
    pub axt_binding: AxtBinding,
    /// Dataspace manifest root observed by the issuer at handle time.
    pub manifest_view_root: [u8; 32],
    /// Expiry slot for freshness enforcement.
    pub expiry_slot: u64,
    /// Optional wall-clock skew allowance enforced by the host.
    #[norito(default)]
    pub max_clock_skew_ms: Option<u32>,
}

/// Simplified representation of spend operations.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Intent forwarded to a dataspace via `USE_ASSET_HANDLE`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RemoteSpendIntent {
    /// Target asset dataspace identifier.
    pub asset_dsid: DataSpaceId,
    /// Operation payload.
    pub op: SpendOp,
}

/// Recorded handle usage for commit validation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtHandleFragment {
    /// Handle presented by the caller.
    pub handle: AssetHandle,
    /// Intent bound to the handle and dataspace.
    pub intent: RemoteSpendIntent,
    /// Optional proof attached to the handle.
    #[norito(default)]
    pub proof: Option<ProofBlob>,
    /// Amount associated with the intent.
    pub amount: u128,
    /// Optional commitment corresponding to the effective amount.
    #[norito(default)]
    pub amount_commitment: Option<[u8; 32]>,
}

/// Canonical fingerprint for a handle usage recorded in the replay ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtHandleReplayKey {
    /// Descriptor binding that minted the handle.
    pub binding: AxtBinding,
    /// Handle era.
    pub handle_era: u64,
    /// Handle sub-nonce.
    pub sub_nonce: u64,
    /// Target lane for the handle.
    pub target_lane: LaneId,
}

impl AxtHandleReplayKey {
    /// Create a replay key from explicit parts.
    #[must_use]
    pub fn from_parts(
        binding: [u8; 32],
        handle_era: u64,
        sub_nonce: u64,
        target_lane: LaneId,
    ) -> Self {
        Self {
            binding: AxtBinding::new(binding),
            handle_era,
            sub_nonce,
            target_lane,
        }
    }

    /// Create a replay key from an [`AssetHandle`].
    #[must_use]
    pub fn from_handle(handle: &AssetHandle) -> Self {
        Self::from_parts(
            handle.axt_binding.into_array(),
            handle.handle_era,
            handle.sub_nonce,
            handle.target_lane,
        )
    }
}

/// Ledger entry capturing when a handle was consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtReplayRecord {
    /// Dataspace referenced by the handle.
    pub dataspace: DataSpaceId,
    /// Slot when the handle was observed.
    pub used_slot: u64,
    /// Slot after which the replay guard can be evicted.
    pub retain_until_slot: u64,
}

impl AxtReplayRecord {
    /// Determine whether the replay guard has expired for a given slot and retention window.
    ///
    /// Records with zeroed slots are treated as stale and expired.
    #[must_use]
    pub fn is_expired(&self, current_slot: u64, retention_slots: u64) -> bool {
        if self.used_slot == 0 && self.retain_until_slot == 0 {
            return true;
        }
        let retention_cutoff = self.used_slot.saturating_add(retention_slots);
        let effective_until = core::cmp::max(self.retain_until_slot, retention_cutoff);
        current_slot >= effective_until
    }
}

/// Aggregate record used to persist and replicate AXT envelopes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtEnvelopeRecord {
    /// Binding derived from the descriptor.
    pub binding: AxtBinding,
    /// Lane executing the AXT.
    pub lane: LaneId,
    /// Canonical descriptor.
    pub descriptor: AxtDescriptor,
    /// Touch fragments per dataspace.
    #[norito(default)]
    pub touches: Vec<AxtTouchFragment>,
    /// Proof fragments per dataspace.
    #[norito(default)]
    pub proofs: Vec<AxtProofFragment>,
    /// Handle fragments recorded during execution.
    #[norito(default)]
    pub handles: Vec<AxtHandleFragment>,
    /// Optional commit height marker for block persistence.
    #[norito(default)]
    pub commit_height: Option<u64>,
}

/// Per-dataspace policy snapshot sourced from the Space Directory/WSV.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtPolicyEntry {
    /// Manifest root the handle must reference.
    pub manifest_root: [u8; 32],
    /// Lane the handle must target.
    pub target_lane: LaneId,
    /// Minimum allowed handle era.
    pub min_handle_era: u64,
    /// Minimum allowed sub-nonce.
    pub min_sub_nonce: u64,
    /// Current slot used for expiry checks.
    pub current_slot: u64,
}

/// Binding between a dataspace id and its AXT policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtPolicyBinding {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Policy entry.
    pub policy: AxtPolicyEntry,
}

/// Collection of AXT policy bindings for deterministic replication.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtPolicySnapshot {
    /// Hash-derived snapshot version (truncated to u64 for gauges/telemetry).
    #[norito(default)]
    pub version: u64,
    /// Ordered bindings for each dataspace.
    #[norito(default)]
    pub entries: Vec<AxtPolicyBinding>,
}

impl AxtPolicySnapshot {
    /// Compute a stable, truncated hash version for a policy snapshot.
    #[must_use]
    pub fn compute_version(entries: &[AxtPolicyBinding]) -> u64 {
        if entries.is_empty() {
            return 0;
        }
        let mut sorted_entries = entries.to_vec();
        sorted_entries.sort_by_key(|binding| binding.dsid);
        let encoded = encode_adaptive(&sorted_entries);
        let hash = Hash::new(&encoded);
        let mut truncated = [0u8; 8];
        truncated.copy_from_slice(&hash.as_ref()[..8]);
        u64::from_le_bytes(truncated)
    }

    /// Populate the version field based on the snapshot entries.
    #[must_use]
    pub fn with_computed_version(mut self) -> Self {
        self.version = Self::compute_version(&self.entries);
        self
    }
}

/// Context captured when an AXT envelope fails policy checks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtRejectContext {
    /// Classified reason for the rejection.
    pub reason: AxtRejectReason,
    /// Dataspace associated with the rejection (if known).
    #[norito(default)]
    pub dataspace: Option<DataSpaceId>,
    /// Lane associated with the rejection (if known).
    #[norito(default)]
    pub lane: Option<LaneId>,
    /// Snapshot version advertised by the policy map used for validation.
    #[norito(default)]
    pub snapshot_version: u64,
    /// Human-readable detail string for operators.
    #[norito(default)]
    pub detail: String,
    /// Minimum handle era hinted by the policy, when available.
    #[norito(default)]
    pub next_min_handle_era: Option<u64>,
    /// Minimum sub-nonce hinted by the policy, when available.
    #[norito(default)]
    pub next_min_sub_nonce: Option<u64>,
}

impl core::fmt::Display for AxtRejectContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} (snapshot={}, lane={:?}, dsid={:?}",
            self.detail, self.snapshot_version, self.lane, self.dataspace
        )?;
        if let Some(era) = self.next_min_handle_era {
            write!(f, ", next_min_handle_era={era}")?;
        }
        if let Some(sub_nonce) = self.next_min_sub_nonce {
            write!(f, ", next_min_sub_nonce={sub_nonce}")?;
        }
        write!(f, ")")
    }
}

/// Canonical reason codes for AXT policy rejections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "reason", content = "detail"))]
#[repr(u8)]
pub enum AxtRejectReason {
    /// Dataspace or lane binding did not match the policy.
    Lane,
    /// Manifest root validation failed.
    Manifest,
    /// Handle era below the policy minimum.
    HandleEra,
    /// Sub-nonce below the policy minimum.
    SubNonce,
    /// Proof or handle expired relative to the current slot.
    Expiry,
    /// Dataspace policy missing for the referenced handle/proof.
    MissingPolicy,
    /// Policy denied the request for any other reason (for example, scope mismatch).
    PolicyDenied,
    /// Proof payload failed validation.
    Proof,
    /// Envelope or handle referenced undeclared/invalid descriptor bindings.
    Descriptor,
    /// Budget constraints were exceeded.
    Budget,
    /// Replay guard or cache validation failed.
    ReplayCache,
    /// Duplicate fragment encountered.
    Duplicate,
}

impl AxtRejectReason {
    /// Stable label used for telemetry and debug outputs.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lane => "lane",
            Self::Manifest => "manifest",
            Self::HandleEra => "era",
            Self::SubNonce => "sub_nonce",
            Self::Expiry => "expiry",
            Self::MissingPolicy => "missing_policy",
            Self::PolicyDenied => "policy_denied",
            Self::Proof => "proof",
            Self::Descriptor => "descriptor",
            Self::Budget => "budget",
            Self::ReplayCache => "replay_cache",
            Self::Duplicate => "duplicate",
        }
    }

    /// Stable machine-readable code suitable for APIs and telemetry.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Lane => "AXT_LANE",
            Self::Manifest => "AXT_MANIFEST",
            Self::HandleEra => "AXT_HANDLE_ERA",
            Self::SubNonce => "AXT_SUB_NONCE",
            Self::Expiry => "AXT_EXPIRY",
            Self::MissingPolicy => "AXT_MISSING_POLICY",
            Self::PolicyDenied => "AXT_POLICY_DENIED",
            Self::Proof => "AXT_PROOF",
            Self::Descriptor => "AXT_DESCRIPTOR",
            Self::Budget => "AXT_BUDGET",
            Self::ReplayCache => "AXT_REPLAY_CACHE",
            Self::Duplicate => "AXT_DUPLICATE",
        }
    }

    /// Alias for telemetry call sites.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        self.label()
    }

    /// Resolve a reason label (e.g., from telemetry) back into a structured enum.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "lane" => Some(Self::Lane),
            "manifest" => Some(Self::Manifest),
            "era" => Some(Self::HandleEra),
            "sub_nonce" => Some(Self::SubNonce),
            "expiry" => Some(Self::Expiry),
            "missing_policy" => Some(Self::MissingPolicy),
            "policy_denied" => Some(Self::PolicyDenied),
            "proof" => Some(Self::Proof),
            "descriptor" => Some(Self::Descriptor),
            "budget" => Some(Self::Budget),
            "replay_cache" => Some(Self::ReplayCache),
            "duplicate" => Some(Self::Duplicate),
            _ => None,
        }
    }
}

/// Errors returned when validating an AXT descriptor.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum AxtValidationError {
    /// Descriptor lists no dataspaces.
    #[error("descriptor must include at least one dataspace")]
    EmptyDataspaceList,
    /// Descriptor repeats a dataspace identifier.
    #[error("duplicate dataspace id {0}")]
    DuplicateDataspaceId(DataSpaceId),
    /// Touch specification references a dataspace not present in `dsids`.
    #[error("touch references undeclared dataspace {0}")]
    TouchUndeclaredDataspace(DataSpaceId),
    /// Touch specification is duplicated for the same dataspace.
    #[error("duplicate touch entry for dataspace {0}")]
    DuplicateTouch(DataSpaceId),
}

/// Validate basic invariants of an AXT descriptor.
///
/// # Errors
///
/// Returns [`AxtValidationError`] when the descriptor is empty, contains duplicate
/// dataspaces, or references undeclared/duplicate touches.
pub fn validate_descriptor(descriptor: &AxtDescriptor) -> Result<(), AxtValidationError> {
    if descriptor.dsids.is_empty() {
        return Err(AxtValidationError::EmptyDataspaceList);
    }

    let mut seen_dsids = std::collections::BTreeSet::new();
    for dsid in &descriptor.dsids {
        if !seen_dsids.insert(*dsid) {
            return Err(AxtValidationError::DuplicateDataspaceId(*dsid));
        }
    }

    let mut seen_touches = std::collections::BTreeSet::new();
    for touch in &descriptor.touches {
        if !seen_dsids.contains(&touch.dsid) {
            return Err(AxtValidationError::TouchUndeclaredDataspace(touch.dsid));
        }
        if !seen_touches.insert(touch.dsid) {
            return Err(AxtValidationError::DuplicateTouch(touch.dsid));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use norito::{decode_from_bytes, to_bytes};

    use super::*;

    fn sample_descriptor(dsid: DataSpaceId) -> AxtDescriptor {
        AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: vec!["orders".into()],
                write: vec!["ledger".into()],
            }],
        }
    }

    #[test]
    fn descriptor_validation_rejects_duplicates_and_missing() {
        let empty = AxtDescriptor {
            dsids: Vec::new(),
            touches: Vec::new(),
        };
        assert!(matches!(
            validate_descriptor(&empty),
            Err(AxtValidationError::EmptyDataspaceList)
        ));

        let dup_ds = AxtDescriptor {
            dsids: vec![DataSpaceId::new(1), DataSpaceId::new(1)],
            touches: Vec::new(),
        };
        assert!(matches!(
            validate_descriptor(&dup_ds),
            Err(AxtValidationError::DuplicateDataspaceId(_))
        ));

        let undeclared_touch = AxtDescriptor {
            dsids: vec![DataSpaceId::new(2)],
            touches: vec![AxtTouchSpec {
                dsid: DataSpaceId::new(99),
                read: Vec::new(),
                write: Vec::new(),
            }],
        };
        assert!(matches!(
            validate_descriptor(&undeclared_touch),
            Err(AxtValidationError::TouchUndeclaredDataspace(_))
        ));

        let dup_touch = AxtDescriptor {
            dsids: vec![DataSpaceId::new(3)],
            touches: vec![
                AxtTouchSpec {
                    dsid: DataSpaceId::new(3),
                    read: Vec::new(),
                    write: Vec::new(),
                },
                AxtTouchSpec {
                    dsid: DataSpaceId::new(3),
                    read: Vec::new(),
                    write: Vec::new(),
                },
            ],
        };
        assert!(matches!(
            validate_descriptor(&dup_touch),
            Err(AxtValidationError::DuplicateTouch(_))
        ));
    }

    #[test]
    fn replay_record_zeroed_slots_are_expired() {
        let record = AxtReplayRecord {
            dataspace: DataSpaceId::new(1),
            used_slot: 0,
            retain_until_slot: 0,
        };
        assert!(record.is_expired(0, 1));
        assert!(record.is_expired(5, 10));
    }

    #[test]
    fn descriptor_validation_accepts_valid_descriptor() {
        let descriptor = sample_descriptor(DataSpaceId::new(7));
        assert_eq!(validate_descriptor(&descriptor), Ok(()));
    }

    #[test]
    fn axt_reject_reason_roundtrips_label() {
        assert_eq!(
            AxtRejectReason::from_label(AxtRejectReason::HandleEra.label()),
            Some(AxtRejectReason::HandleEra)
        );
        assert_eq!(AxtRejectReason::from_label("unknown"), None);
    }

    #[test]
    fn envelope_roundtrips_through_norito() {
        let dsid = DataSpaceId::new(11);
        let descriptor = sample_descriptor(dsid);
        let binding = AxtBinding::new([0xAB; 32]);
        let alice_account = crate::account::AccountId::new(
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key"),
        )
        .to_string();
        let merchant_account = crate::account::AccountId::new(
            "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D"
                .parse()
                .expect("public key"),
        )
        .to_string();
        let envelope = AxtEnvelopeRecord {
            binding,
            lane: LaneId::new(1),
            descriptor: descriptor.clone(),
            touches: vec![AxtTouchFragment {
                dsid,
                manifest: TouchManifest {
                    read: vec!["orders/0".into()],
                    write: vec!["ledger/0".into()],
                },
            }],
            proofs: vec![AxtProofFragment {
                dsid,
                proof: ProofBlob {
                    payload: vec![0xA5, 0x5A],
                    expiry_slot: None,
                },
            }],
            handles: vec![AxtHandleFragment {
                handle: AssetHandle {
                    scope: vec!["transfer".into()],
                    subject: HandleSubject {
                        account: alice_account.clone(),
                        origin_dsid: Some(dsid),
                    },
                    budget: HandleBudget {
                        remaining: 500,
                        per_use: Some(300),
                    },
                    handle_era: 1,
                    sub_nonce: 42,
                    group_binding: GroupBinding {
                        composability_group_id: vec![0u8; 32],
                        epoch_id: 1,
                    },
                    target_lane: LaneId::new(0),
                    axt_binding: binding,
                    manifest_view_root: [1u8; 32],
                    expiry_slot: 10,
                    max_clock_skew_ms: Some(0),
                },
                intent: RemoteSpendIntent {
                    asset_dsid: dsid,
                    op: SpendOp {
                        kind: "transfer".into(),
                        from: alice_account,
                        to: merchant_account,
                        amount: "200".into(),
                    },
                },
                proof: Some(ProofBlob {
                    payload: vec![0xCC],
                    expiry_slot: None,
                }),
                amount: 200,
                amount_commitment: None,
            }],
            commit_height: Some(5),
        };

        let bytes = to_bytes(&envelope).expect("encode envelope");
        let decoded: AxtEnvelopeRecord = decode_from_bytes(&bytes).expect("decode envelope");
        assert_eq!(decoded, envelope);
        assert_eq!(decoded.binding.as_bytes(), &binding.into_array());
        assert_eq!(decoded.descriptor, descriptor);
    }

    #[test]
    fn proof_matches_manifest_accepts_envelope_and_raw_root() {
        let dsid = DataSpaceId::new(17);
        let manifest_root = [0xA5; 32];
        let envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0xCC],
            committed_amount: None,
            amount_commitment: None,
        };
        let encoded = norito::to_bytes(&envelope).expect("encode envelope");
        let proof = ProofBlob {
            payload: encoded,
            expiry_slot: None,
        };
        assert!(proof_matches_manifest(&proof, dsid, manifest_root));

        let raw_proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(5),
        };
        assert!(proof_matches_manifest(&raw_proof, dsid, manifest_root));
    }

    #[test]
    fn proof_matches_manifest_rejects_mismatch() {
        let dsid = DataSpaceId::new(18);
        let other = DataSpaceId::new(19);
        let manifest_root = [0xB4; 32];
        let bad_root = [0xB5; 32];
        let envelope = AxtProofEnvelope {
            dsid: other,
            manifest_root: bad_root,
            da_commitment: None,
            proof: vec![0xCC],
            committed_amount: None,
            amount_commitment: None,
        };
        let encoded = norito::to_bytes(&envelope).expect("encode envelope");
        let proof = ProofBlob {
            payload: encoded,
            expiry_slot: None,
        };
        assert!(!proof_matches_manifest(&proof, dsid, manifest_root));

        let raw_proof = ProofBlob {
            payload: bad_root.to_vec(),
            expiry_slot: Some(7),
        };
        assert!(!proof_matches_manifest(&raw_proof, dsid, manifest_root));

        let zero_root = [0u8; 32];
        let zero_proof = ProofBlob {
            payload: zero_root.to_vec(),
            expiry_slot: None,
        };
        assert!(!proof_matches_manifest(&zero_proof, dsid, zero_root));
    }
}
