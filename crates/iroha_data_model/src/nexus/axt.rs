//! Atomic cross-transaction (AXT) envelope and fragment types for Nexus lanes.
//!
//! These structures mirror the IVM syscall surface while providing Norito-compatible
//! schemas for WSV/block persistence and gossip replication.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
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
    /// Construct a canonical touch manifest from read/write key prefixes.
    ///
    /// Paths are trimmed, empty paths are discarded, and the remaining paths
    /// are sorted and deduplicated.
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
            let mut values: Vec<String> = iter
                .into_iter()
                .map(Into::into)
                .map(|path: String| path.trim().to_owned())
                .filter(|path| !path.is_empty())
                .collect();
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
/// The descriptor's bare Norito payload is prefixed with a domain separator and
/// hashed using Poseidon2 (rate 2, capacity 1, +1 padding) to produce a 32-byte
/// digest. The header-framed encoding is intentionally excluded so the binding
/// stays stable across feature-sensitive schema hashes.
///
/// # Errors
/// Returns an error if the descriptor cannot be encoded using Norito.
pub fn compute_descriptor_binding(descriptor: &AxtDescriptor) -> Result<[u8; 32], norito::Error> {
    let mut buf = b"iroha:axt:desc:v1\0".to_vec();
    let encoded = encode_adaptive(descriptor);
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
        let manifest = TouchManifest::from_read_write(read, write);
        let touch = AxtTouchSpec {
            dsid,
            read: manifest.read,
            write: manifest.write,
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
    /// Norito-encoded AXT proof envelope bytes.
    pub payload: Vec<u8>,
    /// Optional expiry slot advertised by the prover.
    #[norito(default)]
    pub expiry_slot: Option<u64>,
}

/// Check whether the proof envelope binds to the expected dataspace, manifest root,
/// and V1 `FastPQ` verifier binding.
#[must_use]
pub fn proof_matches_manifest(
    proof: &ProofBlob,
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
) -> bool {
    if manifest_root.iter().all(|byte| *byte == 0) {
        return false;
    }
    let Ok(envelope) = norito::decode_canonical::<AxtProofEnvelope>(&proof.payload) else {
        return false;
    };
    let Some(binding) = envelope.fastpq_binding.as_ref() else {
        return false;
    };
    envelope.dsid == dsid
        && envelope.manifest_root == manifest_root
        && envelope.manifest_root.iter().any(|byte| *byte != 0)
        && !envelope.proof.is_empty()
        && binding.source_dsid == dsid.as_u64()
        && binding.verifier_id == "fastpq"
        && binding.verifier_version == "v1"
        && fastpq_binding_shape_is_concrete(binding)
}

fn fastpq_binding_shape_is_concrete(binding: &AxtFastpqBinding) -> bool {
    binding_string_is_present(&binding.parameter)
        && binding_string_is_present(&binding.source_dataspace)
        && binding_string_is_present(&binding.source_receipt_id)
        && binding_hex_digest_is_present(&binding.source_tx_commitment)
        && fastpq_claim_type_is_supported(&binding.claim_type)
        && binding_hex_digest_is_present(&binding.claim_digest)
        && binding_hex_digest_is_present(&binding.witness_commitment)
        && binding_hex_digest_is_present(&binding.policy_commitment)
        && binding_string_is_present(&binding.verified_effect_type)
        && !binding.target_dsids.is_empty()
        && binding
            .target_dsids
            .windows(2)
            .all(|pair| pair[0] < pair[1])
}

fn binding_string_is_present(value: &str) -> bool {
    !value.trim().is_empty()
}

fn binding_hex_digest_is_present(value: &str) -> bool {
    let value = value.trim();
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn fastpq_claim_type_is_supported(value: &str) -> bool {
    let value = value.trim();
    value.eq_ignore_ascii_case("authorization")
        || value.eq_ignore_ascii_case("compliance")
        || value.eq_ignore_ascii_case("tx_predicate")
        || value.eq_ignore_ascii_case("value_conservation")
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
    /// Structured FASTPQ binding used to reconstruct the verified batch.
    #[norito(default)]
    pub fastpq_binding: Option<AxtFastpqBinding>,
    /// Optional non-zero scalar committed by the versioned FASTPQ proof statement.
    ///
    /// This is deliberately a fixed-width proof field, not a business-facing
    /// monetary quantity. Callers must convert a clear [`Quantity`] exactly at
    /// scale zero and reject values outside the `u128` statement domain.
    #[norito(default)]
    pub committed_amount: Option<u128>,
    /// Optional commitment for hidden-amount intents.
    #[norito(default)]
    pub amount_commitment: Option<[u8; 32]>,
}

/// Structured FASTPQ receipt/effect binding embedded in AXT proof envelopes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtFastpqBinding {
    /// Canonical FASTPQ parameter set.
    pub parameter: String,
    /// Source dataspace identifier.
    pub source_dsid: u64,
    /// Source dataspace alias.
    pub source_dataspace: String,
    /// Canonical receipt identifier bound to the proof.
    pub source_receipt_id: String,
    /// Canonical source transaction commitment.
    pub source_tx_commitment: String,
    /// Claim family proven by FASTPQ.
    pub claim_type: String,
    /// Canonical claim digest.
    pub claim_digest: String,
    /// Canonical witness commitment.
    pub witness_commitment: String,
    /// Canonical policy commitment.
    pub policy_commitment: String,
    /// Business effect type verified by the proof.
    pub verified_effect_type: String,
    /// Optional corridor label used by maintained flows.
    #[norito(default)]
    pub corridor: String,
    /// Verifier identifier.
    #[norito(default)]
    pub verifier_id: String,
    /// Verifier version.
    #[norito(default)]
    pub verifier_version: String,
    /// Non-empty, strictly increasing target dataspace ids committed by the proof.
    #[norito(default)]
    pub target_dsids: Vec<u64>,
    /// Business-effect bindings that maintained contracts compare on-ledger.
    #[norito(default)]
    pub effect_binding: Option<AxtEffectBinding>,
}

/// Business-effect bindings committed by a FASTPQ proof envelope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AxtEffectBinding {
    /// Destination dataspace/domain label when applicable.
    #[norito(default)]
    pub destination_domain: Option<String>,
    /// Destination account id in canonical encoded form.
    #[norito(default)]
    pub destination_account_id: Option<String>,
    /// Vault account id in canonical encoded form.
    #[norito(default)]
    pub vault_account_id: Option<String>,
    /// Issuance account id in canonical encoded form.
    #[norito(default)]
    pub issuance_account_id: Option<String>,
    /// Source asset definition id in canonical literal form.
    #[norito(default)]
    pub source_asset_definition_id: Option<String>,
    /// Destination asset definition id in canonical literal form.
    #[norito(default)]
    pub destination_asset_definition_id: Option<String>,
    /// Source scalar in the versioned FASTPQ circuit statement, when present.
    ///
    /// This is not a ledger amount; business quantities must be converted
    /// exactly before constructing the proof witness.
    #[norito(default)]
    pub source_amount_i64: Option<i64>,
    /// Destination scalar in the versioned FASTPQ circuit statement, when present.
    ///
    /// This is not a ledger amount; business quantities must be converted
    /// exactly before constructing the proof witness.
    #[norito(default)]
    pub destination_amount_i64: Option<i64>,
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct HandleBudget {
    /// Remaining allowance for the capability.
    pub remaining: Quantity,
    /// Optional per-use cap.
    #[norito(default)]
    pub per_use: Option<Quantity>,
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
    /// Cleartext amount, or `None` when the proof carries a hidden amount.
    #[norito(default)]
    pub amount: Option<Quantity>,
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
    /// Cleartext amount associated with the intent, or `None` for a hidden amount.
    #[norito(default)]
    pub amount: Option<Quantity>,
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
    /// Exact height of the block that persists this envelope.
    pub commit_height: u64,
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
    pub version: u64,
    /// Ordered bindings for each dataspace.
    pub entries: Vec<AxtPolicyBinding>,
}

/// Errors returned when validating an AXT policy snapshot.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum AxtPolicySnapshotValidationError {
    /// Snapshot repeats a dataspace identifier.
    #[error("duplicate policy binding for dataspace {0}")]
    DuplicateDataspaceId(DataSpaceId),
    /// Snapshot bindings are not strictly increasing by dataspace identifier.
    #[error(
        "policy bindings must be strictly ordered: dataspace {previous} appears before {current}"
    )]
    EntriesNotStrictlyOrdered {
        /// Dataspace identifier immediately before the ordering violation.
        previous: DataSpaceId,
        /// Dataspace identifier at the ordering violation.
        current: DataSpaceId,
    },
    /// Snapshot version does not bind the exact canonical entries.
    #[error("policy snapshot version mismatch: expected {expected}, found {actual}")]
    VersionMismatch {
        /// Version computed from the snapshot entries.
        expected: u64,
        /// Version advertised by the snapshot.
        actual: u64,
    },
}

impl AxtPolicySnapshot {
    /// Compute a stable, truncated hash version for a policy snapshot.
    #[must_use]
    pub fn compute_version(entries: &[AxtPolicyBinding]) -> u64 {
        if entries.is_empty() {
            return 0;
        }
        let canonical_entries = entries.to_vec();
        let encoded = encode_adaptive(&canonical_entries);
        let hash = Hash::new(&encoded);
        let mut truncated = [0u8; 8];
        truncated.copy_from_slice(&hash.as_ref()[..8]);
        u64::from_le_bytes(truncated)
    }

    /// Populate the version field and reject non-canonical snapshot entries.
    ///
    /// # Errors
    ///
    /// Returns [`AxtPolicySnapshotValidationError`] when entries are duplicated
    /// or not strictly ordered.
    pub fn with_computed_version(mut self) -> Result<Self, AxtPolicySnapshotValidationError> {
        self.version = Self::compute_version(&self.entries);
        self.validate()?;
        Ok(self)
    }

    /// Validate canonical binding order and the exact derived snapshot version.
    ///
    /// # Errors
    ///
    /// Returns [`AxtPolicySnapshotValidationError`] when entries are duplicated,
    /// not strictly ordered, or do not match the advertised version.
    pub fn validate(&self) -> Result<(), AxtPolicySnapshotValidationError> {
        for pair in self.entries.windows(2) {
            let previous = pair[0].dsid;
            let current = pair[1].dsid;
            if previous == current {
                return Err(AxtPolicySnapshotValidationError::DuplicateDataspaceId(
                    current,
                ));
            }
            if previous > current {
                return Err(
                    AxtPolicySnapshotValidationError::EntriesNotStrictlyOrdered {
                        previous,
                        current,
                    },
                );
            }
        }

        let expected = Self::compute_version(&self.entries);
        if self.version != expected {
            return Err(AxtPolicySnapshotValidationError::VersionMismatch {
                expected,
                actual: self.version,
            });
        }
        Ok(())
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
    /// Snapshot version advertised by the policy map used for validation, when one was installed.
    #[norito(default)]
    pub snapshot_version: Option<u64>,
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
            "{} (lane={:?}, dsid={:?}",
            self.detail, self.lane, self.dataspace
        )?;
        if let Some(snapshot_version) = self.snapshot_version {
            write!(f, ", snapshot={snapshot_version}")?;
        }
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
    /// Descriptor dataspace identifiers are not strictly increasing.
    #[error("dataspace ids must be strictly ordered: {previous} appears before {current}")]
    DataspaceIdsNotStrictlyOrdered {
        /// Dataspace identifier immediately before the ordering violation.
        previous: DataSpaceId,
        /// Dataspace identifier at the ordering violation.
        current: DataSpaceId,
    },
    /// Touch specification references a dataspace not present in `dsids`.
    #[error("touch references undeclared dataspace {0}")]
    TouchUndeclaredDataspace(DataSpaceId),
    /// Touch specification is duplicated for the same dataspace.
    #[error("duplicate touch entry for dataspace {0}")]
    DuplicateTouch(DataSpaceId),
    /// Touch specifications are not strictly increasing by dataspace identifier.
    #[error(
        "touch entries must be strictly ordered: dataspace {previous} appears before {current}"
    )]
    TouchesNotStrictlyOrdered {
        /// Dataspace identifier immediately before the ordering violation.
        previous: DataSpaceId,
        /// Dataspace identifier at the ordering violation.
        current: DataSpaceId,
    },
    /// A declared read path is empty or contains only whitespace.
    #[error("read path {index} for dataspace {dsid} must not be empty")]
    EmptyReadPath {
        /// Dataspace containing the invalid path.
        dsid: DataSpaceId,
        /// Zero-based path index.
        index: usize,
    },
    /// A declared read path has leading or trailing whitespace.
    #[error("read path {index} for dataspace {dsid} must be trimmed")]
    UntrimmedReadPath {
        /// Dataspace containing the invalid path.
        dsid: DataSpaceId,
        /// Zero-based path index.
        index: usize,
    },
    /// A declared read path duplicates an earlier path.
    #[error("read path {duplicate_index} for dataspace {dsid} duplicates path {first_index}")]
    DuplicateReadPath {
        /// Dataspace containing the duplicate path.
        dsid: DataSpaceId,
        /// Zero-based index of the first occurrence.
        first_index: usize,
        /// Zero-based index of the duplicate occurrence.
        duplicate_index: usize,
    },
    /// Declared read paths are not strictly lexicographically increasing.
    #[error(
        "read paths for dataspace {dsid} must be strictly ordered: path {previous_index} appears before path {current_index}"
    )]
    ReadPathsNotStrictlyOrdered {
        /// Dataspace containing the ordering violation.
        dsid: DataSpaceId,
        /// Zero-based index immediately before the ordering violation.
        previous_index: usize,
        /// Zero-based index at the ordering violation.
        current_index: usize,
    },
    /// A declared write path is empty or contains only whitespace.
    #[error("write path {index} for dataspace {dsid} must not be empty")]
    EmptyWritePath {
        /// Dataspace containing the invalid path.
        dsid: DataSpaceId,
        /// Zero-based path index.
        index: usize,
    },
    /// A declared write path has leading or trailing whitespace.
    #[error("write path {index} for dataspace {dsid} must be trimmed")]
    UntrimmedWritePath {
        /// Dataspace containing the invalid path.
        dsid: DataSpaceId,
        /// Zero-based path index.
        index: usize,
    },
    /// A declared write path duplicates an earlier path.
    #[error("write path {duplicate_index} for dataspace {dsid} duplicates path {first_index}")]
    DuplicateWritePath {
        /// Dataspace containing the duplicate path.
        dsid: DataSpaceId,
        /// Zero-based index of the first occurrence.
        first_index: usize,
        /// Zero-based index of the duplicate occurrence.
        duplicate_index: usize,
    },
    /// Declared write paths are not strictly lexicographically increasing.
    #[error(
        "write paths for dataspace {dsid} must be strictly ordered: path {previous_index} appears before path {current_index}"
    )]
    WritePathsNotStrictlyOrdered {
        /// Dataspace containing the ordering violation.
        dsid: DataSpaceId,
        /// Zero-based index immediately before the ordering violation.
        previous_index: usize,
        /// Zero-based index at the ordering violation.
        current_index: usize,
    },
}

/// Validate the canonical invariants of an AXT descriptor.
///
/// # Errors
///
/// Returns [`AxtValidationError`] when dataspace or touch entries are empty,
/// undeclared, duplicated, or out of order, or when read/write paths are empty,
/// untrimmed, duplicated, or out of order.
pub fn validate_descriptor(descriptor: &AxtDescriptor) -> Result<(), AxtValidationError> {
    if descriptor.dsids.is_empty() {
        return Err(AxtValidationError::EmptyDataspaceList);
    }

    let mut seen_dsids = BTreeSet::new();
    for dsid in &descriptor.dsids {
        if !seen_dsids.insert(*dsid) {
            return Err(AxtValidationError::DuplicateDataspaceId(*dsid));
        }
    }
    for pair in descriptor.dsids.windows(2) {
        if pair[0] >= pair[1] {
            return Err(AxtValidationError::DataspaceIdsNotStrictlyOrdered {
                previous: pair[0],
                current: pair[1],
            });
        }
    }

    let mut seen_touches = BTreeSet::new();
    for touch in &descriptor.touches {
        if !seen_dsids.contains(&touch.dsid) {
            return Err(AxtValidationError::TouchUndeclaredDataspace(touch.dsid));
        }
        if !seen_touches.insert(touch.dsid) {
            return Err(AxtValidationError::DuplicateTouch(touch.dsid));
        }
    }
    for pair in descriptor.touches.windows(2) {
        if pair[0].dsid >= pair[1].dsid {
            return Err(AxtValidationError::TouchesNotStrictlyOrdered {
                previous: pair[0].dsid,
                current: pair[1].dsid,
            });
        }
    }
    for touch in &descriptor.touches {
        validate_read_paths(touch.dsid, &touch.read)?;
        validate_write_paths(touch.dsid, &touch.write)?;
    }

    Ok(())
}

fn validate_read_paths(dsid: DataSpaceId, paths: &[String]) -> Result<(), AxtValidationError> {
    let mut first_indices = BTreeMap::new();
    for (index, path) in paths.iter().enumerate() {
        if path.trim().is_empty() {
            return Err(AxtValidationError::EmptyReadPath { dsid, index });
        }
        if path.trim() != path {
            return Err(AxtValidationError::UntrimmedReadPath { dsid, index });
        }
        if let Some(first_index) = first_indices.insert(path.as_str(), index) {
            return Err(AxtValidationError::DuplicateReadPath {
                dsid,
                first_index,
                duplicate_index: index,
            });
        }
    }
    for (previous_index, pair) in paths.windows(2).enumerate() {
        if pair[0] >= pair[1] {
            return Err(AxtValidationError::ReadPathsNotStrictlyOrdered {
                dsid,
                previous_index,
                current_index: previous_index + 1,
            });
        }
    }
    Ok(())
}

fn validate_write_paths(dsid: DataSpaceId, paths: &[String]) -> Result<(), AxtValidationError> {
    let mut first_indices = BTreeMap::new();
    for (index, path) in paths.iter().enumerate() {
        if path.trim().is_empty() {
            return Err(AxtValidationError::EmptyWritePath { dsid, index });
        }
        if path.trim() != path {
            return Err(AxtValidationError::UntrimmedWritePath { dsid, index });
        }
        if let Some(first_index) = first_indices.insert(path.as_str(), index) {
            return Err(AxtValidationError::DuplicateWritePath {
                dsid,
                first_index,
                duplicate_index: index,
            });
        }
    }
    for (previous_index, pair) in paths.windows(2).enumerate() {
        if pair[0] >= pair[1] {
            return Err(AxtValidationError::WritePathsNotStrictlyOrdered {
                dsid,
                previous_index,
                current_index: previous_index + 1,
            });
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

    fn sample_fastpq_binding(dsid: DataSpaceId) -> AxtFastpqBinding {
        AxtFastpqBinding {
            parameter: "fastpq-lane-balanced".to_string(),
            source_dsid: dsid.as_u64(),
            source_dataspace: format!("test-dataspace-{}", dsid.as_u64()),
            source_receipt_id: format!("receipt-{}", dsid.as_u64()),
            source_tx_commitment: "aa".repeat(32),
            claim_type: "authorization".to_string(),
            claim_digest: "bb".repeat(32),
            witness_commitment: "cc".repeat(32),
            policy_commitment: "dd".repeat(32),
            verified_effect_type: "test_effect".to_string(),
            corridor: "test-corridor".to_string(),
            verifier_id: "fastpq".to_string(),
            verifier_version: "v1".to_string(),
            target_dsids: vec![dsid.as_u64()],
            effect_binding: None,
        }
    }

    fn descriptor_with_paths(read: &[&str], write: &[&str]) -> AxtDescriptor {
        let dsid = DataSpaceId::new(1);
        AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: read.iter().map(|path| (*path).to_owned()).collect(),
                write: write.iter().map(|path| (*path).to_owned()).collect(),
            }],
        }
    }

    #[test]
    fn touch_manifest_constructor_canonicalizes_paths() {
        let manifest = TouchManifest::from_read_write(
            [" zebra ", "", "alpha", "alpha", " \t "],
            [" zeta", "\n", "beta ", "beta", "alpha"],
        );

        assert_eq!(manifest.read, vec!["alpha".to_owned(), "zebra".to_owned()]);
        assert_eq!(
            manifest.write,
            vec!["alpha".to_owned(), "beta".to_owned(), "zeta".to_owned()]
        );
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
    fn descriptor_validation_rejects_noncanonical_entry_order() {
        let first = DataSpaceId::new(2);
        let second = DataSpaceId::new(1);
        let unsorted_dsids = AxtDescriptor {
            dsids: vec![first, second],
            touches: Vec::new(),
        };
        assert_eq!(
            validate_descriptor(&unsorted_dsids),
            Err(AxtValidationError::DataspaceIdsNotStrictlyOrdered {
                previous: first,
                current: second,
            })
        );

        let unsorted_touches = AxtDescriptor {
            dsids: vec![second, first],
            touches: vec![
                AxtTouchSpec {
                    dsid: first,
                    read: Vec::new(),
                    write: Vec::new(),
                },
                AxtTouchSpec {
                    dsid: second,
                    read: Vec::new(),
                    write: Vec::new(),
                },
            ],
        };
        assert_eq!(
            validate_descriptor(&unsorted_touches),
            Err(AxtValidationError::TouchesNotStrictlyOrdered {
                previous: first,
                current: second,
            })
        );
    }

    #[test]
    fn descriptor_validation_rejects_noncanonical_read_paths() {
        assert_eq!(
            validate_descriptor(&descriptor_with_paths(&[" \t "], &[])),
            Err(AxtValidationError::EmptyReadPath {
                dsid: DataSpaceId::new(1),
                index: 0,
            })
        );
        assert_eq!(
            validate_descriptor(&descriptor_with_paths(&[" orders "], &[])),
            Err(AxtValidationError::UntrimmedReadPath {
                dsid: DataSpaceId::new(1),
                index: 0,
            })
        );
        assert_eq!(
            validate_descriptor(&descriptor_with_paths(&["orders", "orders"], &[])),
            Err(AxtValidationError::DuplicateReadPath {
                dsid: DataSpaceId::new(1),
                first_index: 0,
                duplicate_index: 1,
            })
        );
        assert_eq!(
            validate_descriptor(&descriptor_with_paths(&["zebra", "alpha"], &[])),
            Err(AxtValidationError::ReadPathsNotStrictlyOrdered {
                dsid: DataSpaceId::new(1),
                previous_index: 0,
                current_index: 1,
            })
        );
    }

    #[test]
    fn descriptor_validation_rejects_noncanonical_write_paths() {
        assert_eq!(
            validate_descriptor(&descriptor_with_paths(&[], &["\n"])),
            Err(AxtValidationError::EmptyWritePath {
                dsid: DataSpaceId::new(1),
                index: 0,
            })
        );
        assert_eq!(
            validate_descriptor(&descriptor_with_paths(&[], &[" ledger "])),
            Err(AxtValidationError::UntrimmedWritePath {
                dsid: DataSpaceId::new(1),
                index: 0,
            })
        );
        assert_eq!(
            validate_descriptor(&descriptor_with_paths(&[], &["ledger", "ledger"])),
            Err(AxtValidationError::DuplicateWritePath {
                dsid: DataSpaceId::new(1),
                first_index: 0,
                duplicate_index: 1,
            })
        );
        assert_eq!(
            validate_descriptor(&descriptor_with_paths(&[], &["zebra", "alpha"])),
            Err(AxtValidationError::WritePathsNotStrictlyOrdered {
                dsid: DataSpaceId::new(1),
                previous_index: 0,
                current_index: 1,
            })
        );
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
    fn descriptor_binding_hashes_bare_norito_payload() {
        let descriptor = sample_descriptor(DataSpaceId::new(9));
        let mut expected_preimage = b"iroha:axt:desc:v1\0".to_vec();
        expected_preimage.extend_from_slice(&encode_adaptive(&descriptor));

        let binding = compute_descriptor_binding(&descriptor).expect("binding");
        assert_eq!(binding, poseidon_hash_bytes(&expected_preimage));
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
        #[derive(Encode)]
        struct EnvelopeWithoutCommitHeight {
            binding: AxtBinding,
            lane: LaneId,
            descriptor: AxtDescriptor,
            touches: Vec<AxtTouchFragment>,
            proofs: Vec<AxtProofFragment>,
            handles: Vec<AxtHandleFragment>,
        }

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
                        remaining: Quantity::from(500_u64),
                        per_use: Some(Quantity::from(300_u64)),
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
                        amount: Some(Quantity::from(200_u64)),
                    },
                },
                proof: Some(ProofBlob {
                    payload: vec![0xCC],
                    expiry_slot: None,
                }),
                amount: Some(Quantity::from(200_u64)),
                amount_commitment: None,
            }],
            commit_height: 5,
        };

        let bytes = to_bytes(&envelope).expect("encode envelope");
        let decoded: AxtEnvelopeRecord = decode_from_bytes(&bytes).expect("decode envelope");
        assert_eq!(decoded, envelope);
        assert_eq!(decoded.binding.as_bytes(), &binding.into_array());
        assert_eq!(decoded.descriptor, descriptor);

        let missing_commit_height = EnvelopeWithoutCommitHeight {
            binding: envelope.binding,
            lane: envelope.lane,
            descriptor: envelope.descriptor.clone(),
            touches: envelope.touches.clone(),
            proofs: envelope.proofs.clone(),
            handles: envelope.handles.clone(),
        };
        let missing_commit_height_bytes =
            to_bytes(&missing_commit_height).expect("encode omitted-height fixture");
        assert!(
            decode_from_bytes::<AxtEnvelopeRecord>(&missing_commit_height_bytes).is_err(),
            "commit_height is a required V1 wire field"
        );
    }

    #[test]
    fn policy_snapshot_validation_rejects_order_duplicates_and_stale_versions() {
        #[derive(Encode)]
        struct SnapshotWithoutVersion {
            entries: Vec<AxtPolicyBinding>,
        }

        #[derive(Encode)]
        struct SnapshotWithoutEntries {
            version: u64,
        }

        let policy = AxtPolicyEntry {
            manifest_root: [0x42; 32],
            target_lane: LaneId::new(1),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 1,
        };
        let first = AxtPolicyBinding {
            dsid: DataSpaceId::new(1),
            policy,
        };
        let second = AxtPolicyBinding {
            dsid: DataSpaceId::new(2),
            policy,
        };

        let entries = vec![first, second];
        let canonical = AxtPolicySnapshot {
            version: AxtPolicySnapshot::compute_version(&entries),
            entries,
        };
        assert_eq!(canonical.validate(), Ok(()));
        assert_eq!(AxtPolicySnapshot::default().validate(), Ok(()));
        assert!(
            decode_from_bytes::<AxtPolicySnapshot>(
                &to_bytes(&SnapshotWithoutVersion {
                    entries: canonical.entries.clone(),
                })
                .expect("encode missing-version snapshot")
            )
            .is_err(),
            "snapshot version is a required V1 wire field"
        );
        assert!(
            decode_from_bytes::<AxtPolicySnapshot>(
                &to_bytes(&SnapshotWithoutEntries {
                    version: canonical.version,
                })
                .expect("encode missing-entries snapshot")
            )
            .is_err(),
            "snapshot entries are a required V1 wire field"
        );

        let duplicate_entries = vec![first, first];
        let duplicate = AxtPolicySnapshot {
            version: AxtPolicySnapshot::compute_version(&duplicate_entries),
            entries: duplicate_entries,
        };
        assert_eq!(
            duplicate.validate(),
            Err(AxtPolicySnapshotValidationError::DuplicateDataspaceId(
                first.dsid
            ))
        );

        let reversed_entries = vec![second, first];
        let reversed = AxtPolicySnapshot {
            version: AxtPolicySnapshot::compute_version(&reversed_entries),
            entries: reversed_entries,
        };
        assert_ne!(
            canonical.version, reversed.version,
            "snapshot versions must bind the exact entry order"
        );
        assert_eq!(
            reversed.validate(),
            Err(
                AxtPolicySnapshotValidationError::EntriesNotStrictlyOrdered {
                    previous: second.dsid,
                    current: first.dsid,
                }
            )
        );
        assert!(matches!(
            reversed.clone().with_computed_version(),
            Err(AxtPolicySnapshotValidationError::EntriesNotStrictlyOrdered { .. })
        ));

        let zero_version = AxtPolicySnapshot {
            version: 0,
            entries: canonical.entries.clone(),
        };
        assert_eq!(
            zero_version.validate(),
            Err(AxtPolicySnapshotValidationError::VersionMismatch {
                expected: canonical.version,
                actual: 0,
            })
        );

        let stale = AxtPolicySnapshot {
            version: canonical.version.wrapping_add(1),
            entries: canonical.entries.clone(),
        };
        assert_eq!(
            stale.validate(),
            Err(AxtPolicySnapshotValidationError::VersionMismatch {
                expected: canonical.version,
                actual: stale.version,
            })
        );
    }

    #[test]
    fn proof_matches_manifest_accepts_envelope_and_rejects_raw_root() {
        let dsid = DataSpaceId::new(17);
        let manifest_root = [0xA5; 32];
        let envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0xCC],
            fastpq_binding: Some(sample_fastpq_binding(dsid)),
            committed_amount: None,
            amount_commitment: None,
        };
        let encoded = norito::to_bytes(&envelope).expect("encode envelope");
        let proof = ProofBlob {
            payload: encoded,
            expiry_slot: None,
        };
        assert!(proof_matches_manifest(&proof, dsid, manifest_root));

        let missing_binding = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0xCC],
            fastpq_binding: None,
            committed_amount: None,
            amount_commitment: None,
        };
        let missing_binding_proof = ProofBlob {
            payload: norito::to_bytes(&missing_binding).expect("encode envelope"),
            expiry_slot: None,
        };
        assert!(!proof_matches_manifest(
            &missing_binding_proof,
            dsid,
            manifest_root
        ));

        let raw_proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(5),
        };
        assert!(!proof_matches_manifest(&raw_proof, dsid, manifest_root));
    }

    #[test]
    fn fastpq_binding_shape_requires_strictly_increasing_target_dsids() {
        let mut binding = sample_fastpq_binding(DataSpaceId::new(17));
        binding.target_dsids = vec![1, 2, 3];
        assert!(fastpq_binding_shape_is_concrete(&binding));

        binding.target_dsids = vec![1, 1, 2];
        assert!(!fastpq_binding_shape_is_concrete(&binding));

        binding.target_dsids = vec![3, 1, 2];
        assert!(
            !fastpq_binding_shape_is_concrete(&binding),
            "unique but non-canonical target order must fail closed"
        );
    }

    #[test]
    fn proof_matches_manifest_rejects_alternate_layout_and_restores_flags() {
        let dsid = DataSpaceId::new(21);
        let manifest_root = [0xD2; 32];
        let envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0xCC],
            fastpq_binding: Some(sample_fastpq_binding(dsid)),
            committed_amount: None,
            amount_commitment: None,
        };
        let default_flags = norito::default_encode_flags();
        let alternate_flags = default_flags & !norito::core::header_flags::COMPACT_LEN;
        assert_ne!(alternate_flags, default_flags);
        let prior_flags = norito::core::effective_decode_flags();

        let canonical_payload = {
            let _guard = norito::core::DecodeFlagsGuard::enter(default_flags);
            norito::to_bytes(&envelope).expect("encode canonical envelope")
        };
        let alternate_payload = {
            let _guard = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&envelope).expect("encode alternate-layout envelope")
        };
        assert_ne!(alternate_payload, canonical_payload);

        let canonical_proof = ProofBlob {
            payload: canonical_payload,
            expiry_slot: None,
        };
        let alternate_proof = ProofBlob {
            payload: alternate_payload,
            expiry_slot: None,
        };
        {
            let _caller_guard = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            assert_eq!(
                norito::core::effective_decode_flags(),
                Some(alternate_flags)
            );
            assert!(proof_matches_manifest(
                &canonical_proof,
                dsid,
                manifest_root
            ));
            assert_eq!(
                norito::core::effective_decode_flags(),
                Some(alternate_flags)
            );
            assert!(!proof_matches_manifest(
                &alternate_proof,
                dsid,
                manifest_root
            ));
            assert_eq!(
                norito::core::effective_decode_flags(),
                Some(alternate_flags)
            );
        }
        assert_eq!(norito::core::effective_decode_flags(), prior_flags);
    }

    #[test]
    fn proof_matches_manifest_rejects_synthetic_binding_shape() {
        let dsid = DataSpaceId::new(20);
        let manifest_root = [0xC1; 32];
        let mut binding = sample_fastpq_binding(dsid);
        binding.claim_digest.clear();
        let envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0xCC],
            fastpq_binding: Some(binding),
            committed_amount: None,
            amount_commitment: None,
        };
        let proof = ProofBlob {
            payload: norito::to_bytes(&envelope).expect("encode envelope"),
            expiry_slot: None,
        };
        assert!(!proof_matches_manifest(&proof, dsid, manifest_root));

        let mut binding = sample_fastpq_binding(dsid);
        binding.claim_type = "synthetic".to_string();
        let envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0xCC],
            fastpq_binding: Some(binding),
            committed_amount: None,
            amount_commitment: None,
        };
        let proof = ProofBlob {
            payload: norito::to_bytes(&envelope).expect("encode envelope"),
            expiry_slot: None,
        };
        assert!(!proof_matches_manifest(&proof, dsid, manifest_root));

        let mut binding = sample_fastpq_binding(dsid);
        binding.target_dsids.clear();
        let envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: vec![0xCC],
            fastpq_binding: Some(binding),
            committed_amount: None,
            amount_commitment: None,
        };
        let proof = ProofBlob {
            payload: norito::to_bytes(&envelope).expect("encode envelope"),
            expiry_slot: None,
        };
        assert!(!proof_matches_manifest(&proof, dsid, manifest_root));
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
            fastpq_binding: Some(sample_fastpq_binding(other)),
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
