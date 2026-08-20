//! Atomic cross-transaction (AXT) envelope and fragment types for Nexus lanes.
//!
//! These structures mirror the IVM syscall surface while providing Norito-compatible
//! schemas for WSV/block persistence and gossip replication.
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    asset::id::AssetDefinitionId,
    block::BlockHeader,
    nexus::{DataSpaceId, LaneId, UniversalAccountId},
};
use iroha_crypto::{Hash, HashOf, PrivateKey, PublicKey, Signature};
use iroha_primitives::numeric::{NumericOperationError, Quantity};
use iroha_schema::IntoSchema;
use iroha_zkp_halo2::poseidon::hash_bytes as poseidon_hash_bytes;
use norito::codec::{Decode, Encode, encode_adaptive};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
/// Maximum number of proof-bound remote-spend statements in one V1 FASTPQ binding.
///
/// This matches the consensus ceiling for authenticated AXT handles in one
/// block, so one proof can legitimately cover every same-dataspace handle
/// without permitting an unbounded outer proof envelope.
pub const MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1: usize = 65_536;
/// Canonical 32-byte binding derived from an AXT descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtDescriptor {
    /// List of dataspace identifiers touched by the transaction.
    pub dsids: Vec<DataSpaceId>,
    /// Fine-grained access declarations for each dataspace.
    pub touches: Vec<AxtTouchSpec>,
}
/// Declared access set for a dataspace touched by an AXT envelope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtTouchSpec {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Logical read-set expressed as application key prefixes.
    pub read: Vec<String>,
    /// Logical write-set expressed as application key prefixes.
    pub write: Vec<String>,
}
/// Runtime manifest supplied via `AXT_TOUCH`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct TouchManifest {
    /// Keys read within the dataspace during execution.
    pub read: Vec<String>,
    /// Keys written within the dataspace during execution.
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
/// hashed using Poseidon2 (rate 2, capacity 1) to produce a 32-byte digest. Byte
/// packing appends `0x01` and zero-pads to an eight-byte boundary before the
/// sponge's field-level +1 padding. The header-framed encoding is intentionally
/// excluded so the binding stays stable across feature-sensitive schema hashes.
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtTouchFragment {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Manifest captured during execution.
    pub manifest: TouchManifest,
}
/// Wrapper around proof artifacts provided by dataspace verifiers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ProofBlob {
    /// Norito-encoded AXT proof envelope bytes.
    pub payload: Vec<u8>,
    /// Outer mirror of the proof-bound expiry slot.
    ///
    /// `None` is an authenticated no-expiry sentinel, not an omitted
    /// verification field. Consensus consumers must exact-compare this value
    /// with the proof metadata.
    #[norito(required)]
    pub expiry_slot: Option<u64>,
}
/// Check whether the decoded proof envelope has the expected structural shape.
///
/// This helper performs canonical decoding and compares untrusted outer fields;
/// it does **not** verify the `FastPQ` proof or cryptographically authenticate
/// the dataspace, manifest root, expiry, DA commitment, or binding. Consensus
/// and host admission must use the `fastpq_prover` verifier instead.
#[must_use]
pub fn proof_envelope_shape_matches_manifest(
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
        && binding.remote_spend_intent_commitments.len() <= MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1
        && binding
            .remote_spend_intent_commitments
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtProofEnvelope {
    /// Dataspace the proof is intended for.
    pub dsid: DataSpaceId,
    /// Manifest root the proof commits to.
    pub manifest_root: [u8; 32],
    /// Optional DA commitment the proof is bound to.
    #[norito(required)]
    pub da_commitment: Option<[u8; 32]>,
    /// Backend-specific proof payload.
    pub proof: Vec<u8>,
    /// Structured FASTPQ binding used to reconstruct the verified batch.
    #[norito(required)]
    pub fastpq_binding: Option<AxtFastpqBinding>,
    /// Optional non-zero scalar committed by the versioned FASTPQ proof statement.
    ///
    /// This is deliberately a fixed-width proof field, not a business-facing
    /// monetary quantity. Callers must convert a clear [`Quantity`] exactly at
    /// scale zero and reject values outside the `u128` statement domain. When
    /// present, the value must exactly match the proof-bound AXT batch metadata.
    #[norito(required)]
    pub committed_amount: Option<u128>,
    /// Optional commitment for hidden-amount intents.
    #[norito(required)]
    pub amount_commitment: Option<[u8; 32]>,
}
/// Structured FASTPQ receipt/effect binding embedded in AXT proof envelopes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
    pub corridor: String,
    /// Verifier identifier.
    pub verifier_id: String,
    /// Verifier version.
    pub verifier_version: String,
    /// Non-empty, strictly increasing target dataspace ids committed by the proof.
    pub target_dsids: Vec<u64>,
    /// Business-effect bindings that maintained contracts compare on-ledger.
    #[norito(required)]
    pub effect_binding: Option<AxtEffectBinding>,
    /// Canonical sorted commitments linking this proof's exact transfer
    /// statements to independently authenticated [`RemoteSpendIntent`] handles.
    ///
    /// Generic proofs that are not consumed by `USE_ASSET_HANDLE` leave this
    /// empty. Handle-bound proofs must include every exact replay identity,
    /// descriptor, asset, dataspace, operation, accounts, and effective amount
    /// tuple that may use the proof. The proof does not itself grant authority.
    /// Duplicates, non-canonical ordering, and sets larger than
    /// [`MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1`] are rejected.
    pub remote_spend_intent_commitments: Vec<[u8; 32]>,
}
/// Business-effect bindings committed by a FASTPQ proof envelope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtEffectBinding {
    /// Destination dataspace/domain label when applicable.
    #[norito(required)]
    pub destination_domain: Option<String>,
    /// Destination account id in canonical encoded form.
    #[norito(required)]
    pub destination_account_id: Option<String>,
    /// Vault account id in canonical encoded form.
    #[norito(required)]
    pub vault_account_id: Option<String>,
    /// Issuance account id in canonical encoded form.
    #[norito(required)]
    pub issuance_account_id: Option<String>,
    /// Source asset definition id in canonical literal form.
    #[norito(required)]
    pub source_asset_definition_id: Option<String>,
    /// Destination asset definition id in canonical literal form.
    #[norito(required)]
    pub destination_asset_definition_id: Option<String>,
    /// Source scalar in the versioned FASTPQ circuit statement, when present.
    ///
    /// This is not a ledger amount; business quantities must be converted
    /// exactly before constructing the proof witness.
    #[norito(required)]
    pub source_amount_i64: Option<i64>,
    /// Destination scalar in the versioned FASTPQ circuit statement, when present.
    ///
    /// This is not a ledger amount; business quantities must be converted
    /// exactly before constructing the proof witness.
    #[norito(required)]
    pub destination_amount_i64: Option<i64>,
}
/// Proof fragment associated with a dataspace.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtProofFragment {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Proof payload provided by the dataspace.
    pub proof: ProofBlob,
}
/// Dataspace composability group binding advertised by the capability.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct GroupBinding {
    /// Domain or composability group identifier.
    pub composability_group_id: Vec<u8>,
    /// Epoch identifier linked to the handle.
    pub epoch_id: u64,
}
/// Handle budget parameters.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct HandleBudget {
    /// Remaining allowance for the capability.
    pub remaining: Quantity,
    /// Optional per-use cap.
    #[norito(required)]
    pub per_use: Option<Quantity>,
}
/// Capability subject metadata.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct HandleSubject {
    /// Canonical I105 account identifier of the spender.
    pub account: String,
    /// Optional originating dataspace for cross-dataspace handles.
    #[norito(required)]
    pub origin_dsid: Option<DataSpaceId>,
}
/// Domain separator for V1 issuer signatures over asset handles.
pub const AXT_HANDLE_ISSUER_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha:axt:asset-handle-issuer:v1\0";
/// Domain separator for V1 asset-definition incarnation commitments.
pub const AXT_ASSET_INCARNATION_DOMAIN_V1: &[u8] = b"iroha:axt:asset-incarnation:v1\0";
/// Exact non-zero lifecycle incarnation of one registered asset definition.
///
/// Core derives this value from the network, canonical asset identifier,
/// registration header, deterministic execution identity, and lifecycle
/// ordinal. An absent-to-present registration (including
/// re-registration) creates a distinct authority context without revoking
/// handles for unrelated assets. Ordinary updates to a registered definition
/// do not rotate this token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[repr(transparent)]
#[schema(transparent)]
pub struct AxtAssetIncarnationV1(Hash);
/// Failure returned while validating raw V1 asset-incarnation bytes.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AxtAssetIncarnationValidationError {
    /// The all-zero token is reserved for absence.
    #[error("AXT asset-definition incarnation is zero")]
    Zero,
    /// Raw bytes do not satisfy the canonical Iroha hash marker invariant.
    #[error("AXT asset-definition incarnation has an invalid hash marker")]
    InvalidHashMarker,
}
impl AxtAssetIncarnationV1 {
    /// Derive the exact V1 incarnation installed by an asset registry event.
    ///
    /// `registration_header_hash` is `StateTransaction::_curr_block.hash()` at
    /// the absent-to-present registration boundary. `execution_identity` and
    /// `lifecycle_ordinal` identify the exact deterministic registration event
    /// when multiple autonomous executions share that header context.
    #[must_use]
    pub fn derive(
        network_id: &NetworkId,
        asset_definition_id: &AssetDefinitionId,
        registration_header_hash: &HashOf<BlockHeader>,
        execution_identity: &Hash,
        lifecycle_ordinal: u64,
    ) -> Self {
        let asset_bytes = asset_definition_id.aid_bytes();
        let ordinal_bytes = lifecycle_ordinal.to_be_bytes();
        Self(Hash::new_from_chunks(&[
            AXT_ASSET_INCARNATION_DOMAIN_V1,
            network_id.as_bytes(),
            &asset_bytes,
            registration_header_hash.as_ref(),
            execution_identity.as_ref(),
            &ordinal_bytes,
        ]))
    }

    /// Validate and wrap canonical raw incarnation bytes.
    ///
    /// # Errors
    ///
    /// Rejects the absence sentinel and bytes that do not carry the canonical
    /// Iroha hash marker.
    pub fn try_from_bytes(
        bytes: [u8; Hash::LENGTH],
    ) -> Result<Self, AxtAssetIncarnationValidationError> {
        let logical_payload_is_zero = bytes[..Hash::LENGTH - 1].iter().all(|byte| *byte == 0)
            && bytes[Hash::LENGTH - 1] & !1 == 0;
        if logical_payload_is_zero {
            return Err(AxtAssetIncarnationValidationError::Zero);
        }
        let hash = Hash::prehashed(bytes);
        if hash.as_ref() != &bytes {
            return Err(AxtAssetIncarnationValidationError::InvalidHashMarker);
        }
        Ok(Self(hash))
    }

    /// Validate this token's non-zero canonical hash invariant.
    ///
    /// # Errors
    ///
    /// Returns the corresponding validation error for corrupt in-memory or
    /// decoded state.
    pub fn validate(&self) -> Result<(), AxtAssetIncarnationValidationError> {
        Self::try_from_bytes(*self.as_bytes()).map(|_| ())
    }

    /// Borrow the canonical 32-byte incarnation token.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; Hash::LENGTH] {
        self.0.as_ref()
    }

    /// Recover the typed hash backing this token.
    #[must_use]
    pub const fn into_hash(self) -> Hash {
        self.0
    }
}
/// Immutable admission context for one V1 AXT issuer signature.
///
/// None of these values is selected by the submitted handle. Validators
/// reconstruct the context from the exact network, committed issuer policy,
/// and currently executing IVM image before checking the signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtHandleIssuerContextV1 {
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Dataspace whose committed policy authorizes the handle.
    pub asset_dsid: DataSpaceId,
    /// Exact registered incarnation of the asset definition authorized by the handle.
    pub asset_definition_incarnation: AxtAssetIncarnationV1,
    /// Committed UAID authorized to issue handles for the dataspace.
    pub issuer: UniversalAccountId,
    /// Exact committed issuer/permission-manifest root.
    pub issuer_manifest_root: [u8; 32],
    /// Hash of the exact IVM program image allowed to exercise the handle.
    pub code_root: [u8; 32],
    /// Pointer/syscall ABI version whose semantics are authorized.
    pub abi_version: u16,
    /// Canonical hash of the authorized ABI surface.
    pub abi_hash: [u8; 32],
}
impl AxtHandleIssuerContextV1 {
    /// Validate the exact asset-registration incarnation in this context.
    ///
    /// # Errors
    ///
    /// Rejects the absence sentinel or a non-canonical hash marker.
    pub fn validate(&self) -> Result<(), AxtAssetIncarnationValidationError> {
        self.asset_definition_incarnation.validate()
    }
}
impl Default for AxtHandleIssuerContextV1 {
    /// Return a syntactic fixture context that cannot match committed policy.
    ///
    /// This exists for context-free codec/shape fixtures. Issuers must replace
    /// every field with exact committed values before signing; admission always
    /// reconstructs and compares the complete context.
    fn default() -> Self {
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xF1; 32])),
        );
        let asset_definition_id = AssetDefinitionId::from_uuid_bytes([
            0xF0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 0xF2,
        ])
        .expect("fixed fixture asset identifier is canonical UUIDv4");
        let registration_header_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xF4; 32]));
        let execution_identity = Hash::new(b"axt-default-asset-registration-execution");
        Self {
            network_id,
            asset_dsid: DataSpaceId::UNIVERSAL,
            asset_definition_incarnation: AxtAssetIncarnationV1::derive(
                &network_id,
                &asset_definition_id,
                &registration_header_hash,
                &execution_identity,
                0,
            ),
            issuer: UniversalAccountId::from_hash(Hash::prehashed([0xF3; 32])),
            issuer_manifest_root: [0xF5; 32],
            code_root: [0xF7; 32],
            abi_version: 1,
            abi_hash: [0xF9; 32],
        }
    }
}
/// Canonical V1 statement authenticated by an AXT capability issuer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AssetHandleIssuerPayloadV1 {
    /// Immutable admission context reconstructed by the validating host.
    pub context: AxtHandleIssuerContextV1,
    /// Exact asset definition this capability authorizes.
    pub asset_definition_id: AssetDefinitionId,
    /// Declared operations permitted by the capability.
    pub scope: Vec<String>,
    /// Capability subject.
    pub subject: HandleSubject,
    /// Capability budget.
    pub budget: HandleBudget,
    /// Exact active policy era.
    pub active_handle_era: u64,
    /// Exact next per-dataspace capability counter.
    pub next_handle_counter: u64,
    /// Composability group and epoch.
    pub group_binding: GroupBinding,
    /// Authorized execution lane.
    pub target_lane: LaneId,
    /// Descriptor/AXT execution context.
    pub axt_binding: AxtBinding,
    /// Exact active manifest root.
    pub manifest_view_root: [u8; 32],
    /// Capability expiry slot.
    pub expiry_slot: u64,
    /// Requested clock-skew allowance, if any.
    #[norito(required)]
    pub max_clock_skew_ms: Option<u32>,
}
/// Unsigned AXT capability claims prepared by an issuer.
///
/// This type cannot enter an AXT envelope. Signing consumes it and returns the
/// admission-ready [`AssetHandle`] whose signature is mandatory on the wire.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AssetHandleDraft {
    /// Exact asset definition authorized by the capability.
    pub asset_definition_id: AssetDefinitionId,
    /// Declared permissions (example values such as "transfer").
    pub scope: Vec<String>,
    /// Subject bound to the capability.
    pub subject: HandleSubject,
    /// Budget parameters controlling single-/multi-use semantics.
    pub budget: HandleBudget,
    /// Exact active policy era selected for this draft.
    pub handle_era: u64,
    /// Exact next per-dataspace counter selected for this draft.
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
    #[norito(required)]
    pub max_clock_skew_ms: Option<u32>,
}
impl AssetHandleDraft {
    /// Build the exact statement authenticated by the dataspace issuer.
    #[must_use]
    pub fn issuer_payload_v1(
        &self,
        context: AxtHandleIssuerContextV1,
    ) -> AssetHandleIssuerPayloadV1 {
        AssetHandleIssuerPayloadV1 {
            context,
            asset_definition_id: self.asset_definition_id.clone(),
            scope: self.scope.clone(),
            subject: self.subject.clone(),
            budget: self.budget.clone(),
            active_handle_era: self.handle_era,
            next_handle_counter: self.sub_nonce,
            group_binding: self.group_binding.clone(),
            target_lane: self.target_lane,
            axt_binding: self.axt_binding,
            manifest_view_root: self.manifest_view_root,
            expiry_slot: self.expiry_slot,
            max_clock_skew_ms: self.max_clock_skew_ms,
        }
    }
    /// Encode the domain-separated canonical V1 issuer-signature preimage.
    #[must_use]
    pub fn issuer_signature_preimage_v1(&self, context: AxtHandleIssuerContextV1) -> Vec<u8> {
        let payload = self.issuer_payload_v1(context);
        let encoded = encode_adaptive(&payload);
        let mut preimage = Vec::with_capacity(
            AXT_HANDLE_ISSUER_SIGNATURE_DOMAIN_V1
                .len()
                .saturating_add(encoded.len()),
        );
        preimage.extend_from_slice(AXT_HANDLE_ISSUER_SIGNATURE_DOMAIN_V1);
        preimage.extend_from_slice(&encoded);
        preimage
    }
    /// Authenticate this handle with the committed dataspace issuer's key.
    ///
    /// # Errors
    ///
    /// Returns a cryptographic signing error when the private key is invalid.
    pub fn sign_by_issuer_v1(
        self,
        context: AxtHandleIssuerContextV1,
        private_key: &PrivateKey,
    ) -> Result<AssetHandle, iroha_crypto::Error> {
        let preimage = self.issuer_signature_preimage_v1(context);
        let issuer_signature = Signature::try_new(private_key, &preimage)?;
        Ok(AssetHandle::from_signed_draft(
            self,
            context,
            issuer_signature,
        ))
    }
}
/// Admission-ready AXT capability with a mandatory issuer signature.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AssetHandle {
    /// Exact asset definition authorized by the issuer signature.
    pub asset_definition_id: AssetDefinitionId,
    /// Declared permissions (example values such as "transfer").
    pub scope: Vec<String>,
    /// Subject bound to the capability.
    pub subject: HandleSubject,
    /// Budget parameters controlling single-/multi-use semantics.
    pub budget: HandleBudget,
    /// Exact active era selected by committed issuer policy.
    pub handle_era: u64,
    /// Exact next per-dataspace counter selected by committed issuer policy.
    pub sub_nonce: u64,
    /// Dataspace composability group binding.
    pub group_binding: GroupBinding,
    /// Lane the handle is authorized to execute on.
    pub target_lane: LaneId,
    /// Poseidon-style binding of this handle to a descriptor.
    pub axt_binding: AxtBinding,
    /// Exact committed manifest root observed by the issuer.
    pub manifest_view_root: [u8; 32],
    /// Expiry slot for freshness enforcement.
    pub expiry_slot: u64,
    /// Optional wall-clock skew allowance enforced by the host.
    #[norito(required)]
    pub max_clock_skew_ms: Option<u32>,
    /// Immutable network, issuer, code, and ABI context authenticated by the signature.
    pub issuer_context: AxtHandleIssuerContextV1,
    /// Issuer signature over the canonical V1 handle statement.
    pub issuer_signature: Signature,
}
impl AssetHandle {
    fn from_signed_draft(
        draft: AssetHandleDraft,
        issuer_context: AxtHandleIssuerContextV1,
        issuer_signature: Signature,
    ) -> Self {
        Self {
            asset_definition_id: draft.asset_definition_id,
            scope: draft.scope,
            subject: draft.subject,
            budget: draft.budget,
            handle_era: draft.handle_era,
            sub_nonce: draft.sub_nonce,
            group_binding: draft.group_binding,
            target_lane: draft.target_lane,
            axt_binding: draft.axt_binding,
            manifest_view_root: draft.manifest_view_root,
            expiry_slot: draft.expiry_slot,
            max_clock_skew_ms: draft.max_clock_skew_ms,
            issuer_context,
            issuer_signature,
        }
    }
    /// Recover the unsigned claims for canonical signature verification.
    #[must_use]
    pub fn draft(&self) -> AssetHandleDraft {
        AssetHandleDraft {
            asset_definition_id: self.asset_definition_id.clone(),
            scope: self.scope.clone(),
            subject: self.subject.clone(),
            budget: self.budget.clone(),
            handle_era: self.handle_era,
            sub_nonce: self.sub_nonce,
            group_binding: self.group_binding.clone(),
            target_lane: self.target_lane,
            axt_binding: self.axt_binding,
            manifest_view_root: self.manifest_view_root,
            expiry_slot: self.expiry_slot,
            max_clock_skew_ms: self.max_clock_skew_ms,
        }
    }
    /// Verify this handle against the issuer key resolved from committed policy.
    ///
    /// # Errors
    ///
    /// Returns [`iroha_crypto::Error::BadSignature`] when the carried context
    /// differs from the authoritative context or the signature is invalid.
    pub fn verify_issuer_signature_v1(
        &self,
        context: AxtHandleIssuerContextV1,
        issuer: &PublicKey,
    ) -> Result<(), iroha_crypto::Error> {
        if self.issuer_context.validate().is_err()
            || context.validate().is_err()
            || self.issuer_context != context
        {
            return Err(iroha_crypto::Error::BadSignature);
        }
        self.issuer_signature.verify(
            issuer,
            &self
                .draft()
                .issuer_signature_preimage_v1(self.issuer_context),
        )
    }
}
/// Canonical issuer-signed handle family used for cumulative budget accounting.
///
/// The key contains every field in [`AssetHandleIssuerPayloadV1`] except
/// `next_handle_counter`. Sequential sub-nonces therefore spend one shared
/// allowance, while a different signed capability statement remains a distinct
/// family. The signature bytes are deliberately absent because they authenticate
/// the statement but are not part of its identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtHandleBudgetKey {
    issuer_context: AxtHandleIssuerContextV1,
    asset_definition_id: AssetDefinitionId,
    scope: Vec<String>,
    subject: HandleSubject,
    budget: HandleBudget,
    active_handle_era: u64,
    group_binding: GroupBinding,
    target_lane: LaneId,
    axt_binding: AxtBinding,
    manifest_view_root: [u8; 32],
    expiry_slot: u64,
    #[norito(required)]
    max_clock_skew_ms: Option<u32>,
}
impl AxtHandleBudgetKey {
    /// Derive a family key from the exact V1 statement authenticated by an issuer.
    #[must_use]
    pub fn from_issuer_payload_v1(payload: &AssetHandleIssuerPayloadV1) -> Self {
        Self {
            issuer_context: payload.context,
            asset_definition_id: payload.asset_definition_id.clone(),
            scope: payload.scope.clone(),
            subject: payload.subject.clone(),
            budget: payload.budget.clone(),
            active_handle_era: payload.active_handle_era,
            group_binding: payload.group_binding.clone(),
            target_lane: payload.target_lane,
            axt_binding: payload.axt_binding,
            manifest_view_root: payload.manifest_view_root,
            expiry_slot: payload.expiry_slot,
            max_clock_skew_ms: payload.max_clock_skew_ms,
        }
    }

    /// Derive the canonical family key for an admitted signed handle.
    #[must_use]
    pub fn from_handle(handle: &AssetHandle) -> Self {
        Self::from_issuer_payload_v1(&handle.draft().issuer_payload_v1(handle.issuer_context))
    }

    /// Return the authenticated dataspace that issued this handle family.
    #[must_use]
    pub const fn asset_dsid(&self) -> DataSpaceId {
        self.issuer_context.asset_dsid
    }

    /// Return the permanent authorization generation signed into this family.
    #[must_use]
    pub const fn authorization_generation(&self) -> u64 {
        self.active_handle_era
    }

    /// Return the asset-registration incarnation authenticated by this family.
    #[must_use]
    pub const fn asset_definition_incarnation(&self) -> AxtAssetIncarnationV1 {
        self.issuer_context.asset_definition_incarnation
    }

    /// Validate structural invariants carried by this family key.
    ///
    /// # Errors
    ///
    /// Rejects an absent or non-canonical asset-registration incarnation.
    pub fn validate(&self) -> Result<(), AxtAssetIncarnationValidationError> {
        self.issuer_context.validate()
    }

    /// Return the issuer-authorized execution lane for this handle family.
    #[must_use]
    pub const fn target_lane(&self) -> LaneId {
        self.target_lane
    }

    /// Return a conservative count of heap bytes owned by this family key.
    ///
    /// This excludes the inline size of [`Self`] so callers can combine it
    /// with their own container accounting without double-counting. Capacity,
    /// rather than length, is used for owned collections because the WSV hot
    /// tier budgets resident allocation.
    #[must_use]
    pub fn allocated_heap_bytes(&self) -> usize {
        fn quantity_heap_bytes(quantity: &Quantity) -> usize {
            quantity.mantissa().bit_len().saturating_add(7) / 8
        }

        let mut total = self.scope.capacity().saturating_mul(core::mem::size_of::<String>());
        for item in &self.scope {
            total = total.saturating_add(item.capacity());
        }
        total = total.saturating_add(self.subject.account.capacity());
        total = total.saturating_add(self.group_binding.composability_group_id.capacity());
        total = total.saturating_add(quantity_heap_bytes(&self.budget.remaining));
        if let Some(per_use) = &self.budget.per_use {
            total = total.saturating_add(quantity_heap_bytes(per_use));
        }
        total
    }
}
/// Consensus-persisted cumulative consumption for one handle budget family.
///
/// `retain_until_slot` is monotonic audit metadata. V1 deliberately exposes no
/// pruning predicate: slot-configuration changes could otherwise make a removed
/// family usable again and reset its budget.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtHandleBudgetRecord {
    consumed: Quantity,
    retain_until_slot: u64,
}
/// Failure returned while consuming a persisted handle-family budget.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AxtHandleBudgetConsumeError {
    /// The signed family carries an absent or malformed asset incarnation.
    #[error("handle budget family has invalid asset incarnation: {0}")]
    InvalidAssetIncarnation(#[from] AxtAssetIncarnationValidationError),
    /// A handle use must consume a non-zero amount.
    #[error("handle budget consumption amount is zero")]
    ZeroAmount,
    /// Exact decimal accumulation exceeded the canonical quantity domain.
    #[error("handle budget arithmetic failed: {0}")]
    Arithmetic(#[from] NumericOperationError),
    /// Cumulative consumption exceeded the issuer-signed remaining allowance.
    #[error("handle budget cumulative consumption exceeds remaining allowance")]
    RemainingExceeded,
    /// Cumulative consumption exceeded the issuer-signed per-use allowance.
    #[error("handle budget cumulative consumption exceeds per-use allowance")]
    PerUseExceeded,
}
impl AxtHandleBudgetRecord {
    /// Construct an empty cumulative record.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            consumed: Quantity::zero(),
            retain_until_slot: 0,
        }
    }

    /// Return the amount consumed by this family across committed blocks.
    #[must_use]
    pub const fn consumed(&self) -> &Quantity {
        &self.consumed
    }

    /// Return the greatest consensus retention deadline observed for the family.
    #[must_use]
    pub const fn retain_until_slot(&self) -> u64 {
        self.retain_until_slot
    }

    /// Validate a decoded persisted record against its authenticated family key.
    ///
    /// An empty record is only a transient accumulator created by [`Self::empty`]
    /// and must never appear in committed state. `retain_until_slot` is audit
    /// metadata and may be zero for a capability accepted at the genesis slot;
    /// V1 therefore imposes no independent validity rule on it.
    ///
    /// # Errors
    ///
    /// Returns [`AxtHandleBudgetConsumeError::ZeroAmount`] for an empty persisted
    /// record, or the corresponding limit error when cumulative consumption
    /// exceeds the issuer-signed `remaining` or `per_use` allowance.
    pub fn validate_for_key(
        &self,
        key: &AxtHandleBudgetKey,
    ) -> Result<(), AxtHandleBudgetConsumeError> {
        Self::validate_consumed_for_key(&self.consumed, key)
    }

    /// Add one non-zero use while enforcing the issuer-signed aggregate limits.
    ///
    /// The record is unchanged on error. A successful update monotonically
    /// retains the greatest supplied audit deadline.
    ///
    /// # Errors
    ///
    /// Returns [`AxtHandleBudgetConsumeError`] for a zero amount, exact-decimal
    /// overflow, or an issuer-signed `remaining`/`per_use` limit violation.
    pub fn try_consume(
        &mut self,
        key: &AxtHandleBudgetKey,
        amount: &Quantity,
        retain_until_slot: u64,
    ) -> Result<(), AxtHandleBudgetConsumeError> {
        if amount.is_zero() {
            return Err(AxtHandleBudgetConsumeError::ZeroAmount);
        }
        let consumed = self.consumed.checked_add(amount)?;
        Self::validate_consumed_for_key(&consumed, key)?;
        self.consumed = consumed;
        self.retain_until_slot = self.retain_until_slot.max(retain_until_slot);
        Ok(())
    }

    fn validate_consumed_for_key(
        consumed: &Quantity,
        key: &AxtHandleBudgetKey,
    ) -> Result<(), AxtHandleBudgetConsumeError> {
        key.validate()?;
        if consumed.is_zero() {
            return Err(AxtHandleBudgetConsumeError::ZeroAmount);
        }
        if consumed > &key.budget.remaining {
            return Err(AxtHandleBudgetConsumeError::RemainingExceeded);
        }
        if key
            .budget
            .per_use
            .as_ref()
            .is_some_and(|limit| consumed > limit)
        {
            return Err(AxtHandleBudgetConsumeError::PerUseExceeded);
        }
        Ok(())
    }
}
/// Permanent per-dataspace ratchet for AXT authorization generations and sub-nonces.
///
/// The record is consensus state independent of manifest, era, lane, and slot
/// configuration. Once created it must never be removed or reset: policy
/// snapshots project both fields so a previously issued handle can never become
/// current again after an authorization-identity cycle or node restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtHandleCounterRecord {
    next: u64,
    authorization_generation: u64,
}
/// Failure returned while validating or advancing an AXT handle counter ratchet.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AxtHandleCounterError {
    /// A persisted ratchet may never contain the reserved zero value.
    #[error("AXT handle counter ratchet contains reserved zero value")]
    ZeroNextCounter,
    /// A persisted ratchet may never contain the inactive generation sentinel.
    #[error("AXT authorization generation contains reserved zero value")]
    ZeroAuthorizationGeneration,
    /// The presented sub-nonce is not the exact permanent next value.
    #[error("handle sub-nonce mismatch: expected {expected}, found {actual}")]
    SubNonceMismatch {
        /// Exact next admissible sub-nonce.
        expected: u64,
        /// Caller-presented sub-nonce.
        actual: u64,
    },
    /// The presented handle era is not the exact permanent authorization generation.
    #[error("handle authorization generation mismatch: expected {expected}, found {actual}")]
    AuthorizationGenerationMismatch {
        /// Exact generation projected into the active policy.
        expected: u64,
        /// Caller-presented handle era.
        actual: u64,
    },
    /// The permanent counter cannot advance without wrapping.
    #[error("AXT handle counter ratchet is exhausted")]
    CounterExhausted,
    /// The permanent authorization generation cannot advance without wrapping.
    #[error("AXT authorization generation is exhausted")]
    AuthorizationGenerationExhausted,
}
impl AxtHandleCounterRecord {
    /// Construct the first permanent record for a dataspace.
    ///
    /// `authorization_generation` is derived from the first active manifest's
    /// activation era. Zero is the absent/inactive policy sentinel, so an
    /// active record normalizes it to one.
    #[must_use]
    pub const fn initial(authorization_generation: u64) -> Self {
        Self {
            next: 1,
            authorization_generation: if authorization_generation == 0 {
                1
            } else {
                authorization_generation
            },
        }
    }

    /// Construct a validated record from authoritative persisted/setup state.
    ///
    /// Live handle consumption must use [`Self::try_advance`]; this constructor
    /// exists for bounded snapshot decoding and explicit policy installation.
    ///
    /// # Errors
    ///
    /// Returns the corresponding zero-value error for a reserved next counter
    /// or inactive authorization generation.
    pub const fn try_from_parts(
        next: u64,
        authorization_generation: u64,
    ) -> Result<Self, AxtHandleCounterError> {
        if next == 0 {
            return Err(AxtHandleCounterError::ZeroNextCounter);
        }
        if authorization_generation == 0 {
            return Err(AxtHandleCounterError::ZeroAuthorizationGeneration);
        }
        Ok(Self {
            next,
            authorization_generation,
        })
    }

    /// Return the exact next admissible handle sub-nonce.
    #[must_use]
    pub const fn next(&self) -> u64 {
        self.next
    }

    /// Return the permanent generation projected as `active_handle_era`.
    #[must_use]
    pub const fn authorization_generation(&self) -> u64 {
        self.authorization_generation
    }

    /// Validate a decoded persisted counter record.
    ///
    /// # Errors
    ///
    /// Returns the corresponding zero-value error when snapshot data contains
    /// a reserved counter or inactive generation sentinel.
    pub const fn validate(&self) -> Result<(), AxtHandleCounterError> {
        if self.next == 0 {
            return Err(AxtHandleCounterError::ZeroNextCounter);
        }
        if self.authorization_generation == 0 {
            return Err(AxtHandleCounterError::ZeroAuthorizationGeneration);
        }
        Ok(())
    }

    /// Consume a handle at the exact generation and next sub-nonce.
    ///
    /// The record is unchanged on error.
    ///
    /// # Errors
    ///
    /// Returns [`AxtHandleCounterError`] when the persisted value is invalid,
    /// the presented generation/sub-nonce is stale or caller-selected future
    /// state, or the counter is exhausted.
    pub fn try_advance(
        &mut self,
        presented_generation: u64,
        presented_sub_nonce: u64,
    ) -> Result<(), AxtHandleCounterError> {
        self.validate()?;
        if presented_generation != self.authorization_generation {
            return Err(AxtHandleCounterError::AuthorizationGenerationMismatch {
                expected: self.authorization_generation,
                actual: presented_generation,
            });
        }
        if presented_sub_nonce != self.next {
            return Err(AxtHandleCounterError::SubNonceMismatch {
                expected: self.next,
                actual: presented_sub_nonce,
            });
        }
        let advanced = self
            .next
            .checked_add(1)
            .ok_or(AxtHandleCounterError::CounterExhausted)?;
        self.next = advanced;
        Ok(())
    }

    /// Revoke the current generation and next sub-nonce during a policy transition.
    ///
    /// The next generation is `max(current + 1, minimum_generation)`, where the
    /// minimum is the newly derived manifest activation era (or zero on
    /// removal). The record is unchanged on error.
    ///
    /// # Errors
    ///
    /// Returns [`AxtHandleCounterError`] for invalid persisted state or when
    /// either permanent dimension cannot advance without wrapping.
    pub fn try_revoke_for_policy_transition(
        &mut self,
        minimum_generation: u64,
    ) -> Result<(), AxtHandleCounterError> {
        self.validate()?;
        let advanced_next = self
            .next
            .checked_add(1)
            .ok_or(AxtHandleCounterError::CounterExhausted)?;
        let advanced_generation = self
            .authorization_generation
            .checked_add(1)
            .ok_or(AxtHandleCounterError::AuthorizationGenerationExhausted)?
            .max(minimum_generation);
        self.next = advanced_next;
        self.authorization_generation = advanced_generation;
        Ok(())
    }
}
/// Error returned when a handle does not represent the one allowed ratchet step.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AxtHandleSequenceError {
    /// The handle does not use the exact active manifest era.
    #[error("handle era mismatch: expected {expected}, found {actual}")]
    EraMismatch {
        /// Active manifest era.
        expected: u64,
        /// Caller-supplied era.
        actual: u64,
    },
    /// The handle does not use the exact next counter.
    #[error("handle sub-nonce mismatch: expected {expected}, found {actual}")]
    SubNonceMismatch {
        /// Next admissible counter.
        expected: u64,
        /// Caller-supplied counter.
        actual: u64,
    },
    /// The counter cannot advance without wrapping.
    #[error("handle sub-nonce counter is exhausted")]
    CounterExhausted,
}
/// Validate one exact era/counter transition and return the next counter.
///
/// This deliberately rejects both stale values and caller-selected future values. The active
/// manifest controls the era; accepted handles advance only the per-dataspace counter by one.
///
/// # Errors
///
/// Returns [`AxtHandleSequenceError::EraMismatch`] or
/// [`AxtHandleSequenceError::SubNonceMismatch`] when the handle does not match the active policy,
/// and [`AxtHandleSequenceError::CounterExhausted`] when the accepted counter cannot advance.
pub fn next_axt_handle_sub_nonce(
    policy: &AxtPolicyEntry,
    handle: &AssetHandle,
) -> Result<u64, AxtHandleSequenceError> {
    if handle.handle_era != policy.active_handle_era {
        return Err(AxtHandleSequenceError::EraMismatch {
            expected: policy.active_handle_era,
            actual: handle.handle_era,
        });
    }
    if handle.sub_nonce != policy.next_handle_counter {
        return Err(AxtHandleSequenceError::SubNonceMismatch {
            expected: policy.next_handle_counter,
            actual: handle.sub_nonce,
        });
    }
    handle
        .sub_nonce
        .checked_add(1)
        .ok_or(AxtHandleSequenceError::CounterExhausted)
}
/// Simplified representation of spend operations.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SpendOp {
    /// Exact asset definition authorized by the handle and proven by FASTPQ.
    pub asset_definition_id: AssetDefinitionId,
    /// Operation kind (e.g., "transfer").
    pub kind: String,
    /// Origin account id in canonical I105 form.
    pub from: String,
    /// Destination account id in canonical I105 form.
    pub to: String,
    /// Cleartext amount, or `None` when the proof carries a hidden amount.
    #[norito(required)]
    pub amount: Option<Quantity>,
}
/// Intent forwarded to a dataspace via `USE_ASSET_HANDLE`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RemoteSpendIntent {
    /// Target asset dataspace identifier.
    pub asset_dsid: DataSpaceId,
    /// Operation payload.
    pub op: SpendOp,
}
/// Canonical claim binding one proof-resolved remote spend to one authenticated handle use.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtRemoteSpendClaimV1 {
    /// Exact authenticated handle use that is allowed to consume this claim.
    ///
    /// Including the replay identity prevents a proof for one handle from
    /// authorizing a different handle with otherwise identical transfer data.
    pub handle_replay_key: AxtHandleReplayKey,
    /// Exact asset definition transferred by the proof transcript.
    pub asset_definition_id: AssetDefinitionId,
    /// Exact operation kind. V1 FASTPQ transcript linkage accepts only `transfer`.
    pub kind: String,
    /// Canonical I105 source account.
    pub from: String,
    /// Canonical I105 destination account.
    pub to: String,
    /// Effective clear or proof-resolved amount.
    pub effective_amount: Quantity,
}
impl AxtRemoteSpendClaimV1 {
    /// Construct the canonical preimage committed for one remote spend.
    #[must_use]
    pub fn new(
        handle_replay_key: AxtHandleReplayKey,
        asset_definition_id: AssetDefinitionId,
        kind: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        effective_amount: Quantity,
    ) -> Self {
        Self {
            handle_replay_key,
            asset_definition_id,
            kind: kind.into(),
            from: from.into(),
            to: to.into(),
            effective_amount,
        }
    }
}
/// Compute the canonical V1 commitment that binds a FASTPQ proof to one remote spend.
///
/// The commitment covers the exact authenticated handle replay identity, asset
/// definition, operation kind, canonical accounts, and effective amount. The
/// handle identity includes its descriptor binding, asset dataspace, exact
/// asset-definition incarnation, era, sub-nonce, and target lane, so a proof
/// cannot be replayed for another handle or a later registration of the asset.
/// The domain separator and canonical framed Norito statement encoding make
/// the commitment deterministic and distinct from other AXT and FASTPQ digests.
#[must_use]
pub fn compute_remote_spend_intent_commitment_v1(
    handle_replay_key: AxtHandleReplayKey,
    asset_definition_id: &AssetDefinitionId,
    kind: &str,
    from: &str,
    to: &str,
    effective_amount: &Quantity,
) -> [u8; 32] {
    let statement = AxtRemoteSpendClaimV1::new(
        handle_replay_key,
        asset_definition_id.clone(),
        kind,
        from,
        to,
        effective_amount.clone(),
    );
    compute_remote_spend_claim_commitment_v1(&statement)
}
/// Compute the canonical V1 commitment for an already materialized remote-spend claim.
#[must_use]
pub fn compute_remote_spend_claim_commitment_v1(statement: &AxtRemoteSpendClaimV1) -> [u8; 32] {
    let mut payload = b"iroha:axt:remote-spend-intent:v1\0".to_vec();
    payload.extend_from_slice(
        &norito::encode_canonical(statement)
            .expect("fixed remote-spend commitment statement must encode canonically"),
    );
    Hash::new(payload).into()
}
/// Recorded handle usage for commit validation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtHandleFragment {
    /// Handle presented by the caller.
    pub handle: AssetHandle,
    /// Intent bound to the handle and dataspace.
    pub intent: RemoteSpendIntent,
    /// Optional proof attached to the handle.
    #[norito(required)]
    pub proof: Option<ProofBlob>,
    /// Cleartext amount associated with the intent, or `None` for a hidden amount.
    #[norito(required)]
    pub amount: Option<Quantity>,
    /// Optional commitment corresponding to the effective amount.
    #[norito(required)]
    pub amount_commitment: Option<[u8; 32]>,
}
/// Canonical fingerprint for a handle usage recorded in the replay ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtHandleReplayKey {
    /// Dataspace whose committed policy issued the handle.
    pub asset_dsid: DataSpaceId,
    /// Exact registered incarnation of the asset definition authorized by the handle.
    pub asset_definition_incarnation: AxtAssetIncarnationV1,
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
        asset_dsid: DataSpaceId,
        asset_definition_incarnation: AxtAssetIncarnationV1,
        binding: [u8; 32],
        handle_era: u64,
        sub_nonce: u64,
        target_lane: LaneId,
    ) -> Self {
        Self {
            asset_dsid,
            asset_definition_incarnation,
            binding: AxtBinding::new(binding),
            handle_era,
            sub_nonce,
            target_lane,
        }
    }
    /// Create a replay key from an [`AssetHandle`] and its authenticated policy dataspace.
    #[must_use]
    pub fn from_handle(asset_dsid: DataSpaceId, handle: &AssetHandle) -> Self {
        Self::from_parts(
            asset_dsid,
            handle.issuer_context.asset_definition_incarnation,
            handle.axt_binding.into_array(),
            handle.handle_era,
            handle.sub_nonce,
            handle.target_lane,
        )
    }

    /// Return the exact asset-definition incarnation authenticated by this replay key.
    #[must_use]
    pub const fn asset_definition_incarnation(&self) -> AxtAssetIncarnationV1 {
        self.asset_definition_incarnation
    }

    /// Validate the exact asset incarnation carried by a decoded replay key.
    ///
    /// # Errors
    ///
    /// Rejects the absence sentinel or a non-canonical hash marker.
    pub fn validate(&self) -> Result<(), AxtHandleReplayKeyValidationError> {
        self.asset_definition_incarnation
            .validate()
            .map_err(AxtHandleReplayKeyValidationError::InvalidAssetIncarnation)?;
        if self.handle_era == 0 {
            return Err(AxtHandleReplayKeyValidationError::ZeroHandleEra);
        }
        if self.sub_nonce == 0 {
            return Err(AxtHandleReplayKeyValidationError::ZeroSubNonce);
        }
        Ok(())
    }
}
/// Failure returned while validating a persisted AXT handle replay key.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AxtHandleReplayKeyValidationError {
    /// The key carries an absent or malformed asset-definition incarnation.
    #[error("replay key has an invalid asset-definition incarnation: {0}")]
    InvalidAssetIncarnation(AxtAssetIncarnationValidationError),
    /// V1 handles never authenticate era zero.
    #[error("replay key handle era must be non-zero")]
    ZeroHandleEra,
    /// V1 handles never authenticate sub-nonce zero.
    #[error("replay key sub-nonce must be non-zero")]
    ZeroSubNonce,
}
/// Ledger entry capturing when a handle was consumed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtReplayRecord {
    /// Redundant observational dataspace recorded with the handle use.
    ///
    /// Replay and cleanup authority comes from [`AxtHandleReplayKey::asset_dsid`];
    /// consumers must not use this copy to select the replay scope.
    pub dataspace: DataSpaceId,
    /// Exact issuer-authenticated family whose durable budget proves this use.
    ///
    /// The replay key intentionally carries only the compact nonce identity, so
    /// this field is required to preserve a fail-closed link to the complete
    /// signed capability family across snapshots and Kura replay.
    pub budget_key: AxtHandleBudgetKey,
    /// Slot when the handle was observed.
    pub used_slot: u64,
    /// Slot after which the replay guard can be evicted.
    pub retain_until_slot: u64,
}
/// Failure returned while validating a persisted AXT replay-ledger entry.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AxtReplayRecordValidationError {
    /// The authoritative replay key carries an absent or malformed asset incarnation.
    #[error("invalid authoritative replay key: {0}")]
    InvalidReplayKey(AxtHandleReplayKeyValidationError),
    /// The referenced family key carries an absent or malformed asset incarnation.
    #[error("replay record references an invalid handle budget family: {0}")]
    InvalidBudgetKey(AxtAssetIncarnationValidationError),
    /// The redundant record dataspace differs from the authoritative key.
    #[error("replay record dataspace does not match its authoritative key")]
    DataspaceMismatch,
    /// The compact replay identity and referenced family authenticate different incarnations.
    #[error("replay record budget family has a different asset incarnation")]
    BudgetIncarnationMismatch,
    /// The compact replay identity and referenced family authenticate different generations.
    #[error("replay record budget family has a different authorization generation")]
    BudgetGenerationMismatch,
    /// The compact replay identity and referenced family authenticate different lanes.
    #[error("replay record budget family has a different target lane")]
    BudgetLaneMismatch,
    /// The compact replay identity and referenced family authenticate different bindings.
    #[error("replay record budget family has a different AXT binding")]
    BudgetBindingMismatch,
    /// A zeroed record cannot represent an accepted handle use.
    #[error("replay record has zero use and retention slots")]
    ZeroedSlots,
    /// The retention deadline precedes the slot at which the handle was used.
    #[error("replay record retention deadline precedes its use slot")]
    RetentionBeforeUse,
}
impl AxtReplayRecord {
    /// Validate a decoded persisted record against its authoritative replay key.
    ///
    /// The dataspace carried by the record is observational redundancy only;
    /// authorization and lookup always use the key.
    ///
    /// # Errors
    ///
    /// Returns [`AxtReplayRecordValidationError`] when the authoritative key is
    /// invalid, the redundant dataspace or signed budget family disagrees with
    /// the key, both slots are zero, or retention ends before the recorded use.
    pub fn validate_for_key(
        &self,
        key: &AxtHandleReplayKey,
    ) -> Result<(), AxtReplayRecordValidationError> {
        key.validate()
            .map_err(AxtReplayRecordValidationError::InvalidReplayKey)?;
        self.budget_key
            .validate()
            .map_err(AxtReplayRecordValidationError::InvalidBudgetKey)?;
        if self.dataspace != key.asset_dsid || self.budget_key.asset_dsid() != key.asset_dsid {
            return Err(AxtReplayRecordValidationError::DataspaceMismatch);
        }
        if self.budget_key.asset_definition_incarnation() != key.asset_definition_incarnation {
            return Err(AxtReplayRecordValidationError::BudgetIncarnationMismatch);
        }
        if self.budget_key.authorization_generation() != key.handle_era {
            return Err(AxtReplayRecordValidationError::BudgetGenerationMismatch);
        }
        if self.budget_key.target_lane != key.target_lane {
            return Err(AxtReplayRecordValidationError::BudgetLaneMismatch);
        }
        if self.budget_key.axt_binding != key.binding {
            return Err(AxtReplayRecordValidationError::BudgetBindingMismatch);
        }
        if self.used_slot == 0 && self.retain_until_slot == 0 {
            return Err(AxtReplayRecordValidationError::ZeroedSlots);
        }
        if self.retain_until_slot < self.used_slot {
            return Err(AxtReplayRecordValidationError::RetentionBeforeUse);
        }
        Ok(())
    }

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
        current_slot > effective_until
    }
}
/// Aggregate record used to persist and replicate AXT envelopes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtEnvelopeRecord {
    /// Binding derived from the descriptor.
    pub binding: AxtBinding,
    /// Lane executing the AXT.
    pub lane: LaneId,
    /// Canonical descriptor.
    pub descriptor: AxtDescriptor,
    /// Touch fragments per dataspace.
    pub touches: Vec<AxtTouchFragment>,
    /// Proof fragments per dataspace.
    pub proofs: Vec<AxtProofFragment>,
    /// Handle fragments recorded during execution.
    pub handles: Vec<AxtHandleFragment>,
    /// Exact height of the block that persists this envelope.
    pub commit_height: u64,
}
/// Per-dataspace policy snapshot sourced from the Space Directory/WSV.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtPolicyEntry {
    /// Manifest root the handle must reference.
    pub manifest_root: [u8; 32],
    /// Lane the handle must target.
    pub target_lane: LaneId,
    /// Exact active handle era.
    pub active_handle_era: u64,
    /// Exact next admissible handle counter.
    pub next_handle_counter: u64,
    /// Current slot used for expiry checks.
    pub current_slot: u64,
}
/// Binding between a dataspace id and its AXT policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtPolicyBinding {
    /// Dataspace identifier.
    pub dsid: DataSpaceId,
    /// Policy entry.
    pub policy: AxtPolicyEntry,
}
/// Collection of AXT policy bindings for deterministic replication.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
    /// A policy projection disagrees with the permanent dataspace counter ratchet.
    #[error(
        "policy counter for dataspace {dataspace} is {policy_next}, permanent ratchet requires {ratchet_next}"
    )]
    CounterRatchetMismatch {
        /// Dataspace whose projected counter is inconsistent.
        dataspace: DataSpaceId,
        /// Next counter advertised by the policy snapshot.
        policy_next: u64,
        /// Exact next counter held by permanent consensus state.
        ratchet_next: u64,
    },
    /// A projected handle era disagrees with the permanent authorization generation.
    #[error(
        "policy authorization generation for dataspace {dataspace} is {policy_generation}, permanent ratchet requires {ratchet_generation}"
    )]
    AuthorizationGenerationRatchetMismatch {
        /// Dataspace whose projected generation is inconsistent.
        dataspace: DataSpaceId,
        /// Generation advertised as `active_handle_era` by the policy snapshot.
        policy_generation: u64,
        /// Exact generation held by permanent consensus state.
        ratchet_generation: u64,
    },
    /// A policy transition cannot revoke the final counter value without wrapping.
    #[error("AXT handle counter ratchet is exhausted for dataspace {dataspace}")]
    CounterRatchetExhausted {
        /// Dataspace whose permanent ratchet reached `u64::MAX`.
        dataspace: DataSpaceId,
    },
    /// A finalized local policy projection differs from the advertised snapshot.
    #[error("finalized AXT policy projection differs from advertised snapshot")]
    FinalizedPolicyMismatch,
    /// Non-genesis committed state did not expose consensus-authenticated time.
    #[error("committed non-genesis AXT state lacks authenticated ledger time")]
    AuthenticatedLedgerTimeUnavailable,
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AxtRejectContext {
    /// Classified reason for the rejection.
    pub reason: AxtRejectReason,
    /// Dataspace associated with the rejection (if known).
    #[norito(required)]
    pub dataspace: Option<DataSpaceId>,
    /// Lane associated with the rejection (if known).
    #[norito(required)]
    pub lane: Option<LaneId>,
    /// Snapshot version advertised by the policy map used for validation, when one was installed.
    #[norito(required)]
    pub snapshot_version: Option<u64>,
    /// Human-readable detail string for operators.
    pub detail: String,
    /// Exact active handle era, when available.
    #[norito(required)]
    pub active_handle_era: Option<u64>,
    /// Exact next handle counter, when available.
    #[norito(required)]
    pub next_handle_counter: Option<u64>,
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
        if let Some(era) = self.active_handle_era {
            write!(f, ", active_handle_era={era}")?;
        }
        if let Some(sub_nonce) = self.next_handle_counter {
            write!(f, ", next_handle_counter={sub_nonce}")?;
        }
        write!(f, ")")
    }
}
/// Canonical reason codes for AXT policy rejections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "reason", content = "detail"))]
#[repr(u8)]
pub enum AxtRejectReason {
    /// Dataspace or lane binding did not match the policy.
    Lane,
    /// Manifest root validation failed.
    Manifest,
    /// Handle era differs from the exact active era.
    HandleEra,
    /// Handle counter differs from the exact next counter.
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
    use super::*;
    use crate::domain::DomainId;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::{bigint::BigInt, numeric::Numeric};
    #[cfg(feature = "json")]
    use mv::json::JsonKeyCodec;
    use norito::{decode_from_bytes, to_bytes};
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
    fn test_network_id(seed: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            seed,
        )))
    }
    fn test_asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("domain"),
            "rose".parse().expect("asset name"),
        )
    }
    fn test_asset_incarnation(seed: &[u8]) -> AxtAssetIncarnationV1 {
        let network_id = test_network_id(seed);
        let registration_header_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            [b"axt-test-replay-registration:".as_slice(), seed].concat(),
        ));
        AxtAssetIncarnationV1::derive(
            &network_id,
            &test_asset_definition_id(),
            &registration_header_hash,
            &Hash::new([b"axt-test-replay-execution:".as_slice(), seed].concat()),
            0,
        )
    }
    fn issuer_context(network_id: NetworkId, asset_dsid: DataSpaceId) -> AxtHandleIssuerContextV1 {
        let asset_definition_id = test_asset_definition_id();
        let registration_header_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"axt-test-asset-registration-header",
        ));
        let execution_identity = Hash::new(b"axt-test-asset-registration-execution");
        AxtHandleIssuerContextV1 {
            network_id,
            asset_dsid,
            asset_definition_incarnation: AxtAssetIncarnationV1::derive(
                &network_id,
                &asset_definition_id,
                &registration_header_hash,
                &execution_identity,
                0,
            ),
            issuer: UniversalAccountId::from_hash(Hash::new(b"axt-test-issuer")),
            issuer_manifest_root: [0x5A; 32],
            code_root: [0xC0; 32],
            abi_version: 1,
            abi_hash: [0xAB; 32],
        }
    }
    fn sample_asset_handle_draft() -> AssetHandleDraft {
        AssetHandleDraft {
            asset_definition_id: test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".into(),
                origin_dsid: Some(DataSpaceId::new(7)),
            },
            budget: HandleBudget {
                remaining: Quantity::from(50_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 9,
            sub_nonce: 3,
            group_binding: GroupBinding {
                composability_group_id: b"settlement".to_vec(),
                epoch_id: 4,
            },
            target_lane: LaneId::new(2),
            axt_binding: AxtBinding::new([0xA5; 32]),
            manifest_view_root: [0x5A; 32],
            expiry_slot: 100,
            max_clock_skew_ms: Some(25),
        }
    }
    fn sample_asset_handle() -> AssetHandle {
        let issuer = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);
        sample_asset_handle_draft()
            .sign_by_issuer_v1(
                issuer_context(test_network_id(b"sequence-network"), DataSpaceId::new(7)),
                issuer.private_key(),
            )
            .expect("sign sample handle")
    }
    fn budget_key_for_replay_key(key: &AxtHandleReplayKey) -> AxtHandleBudgetKey {
        let mut handle = sample_asset_handle();
        handle.issuer_context.asset_dsid = key.asset_dsid;
        handle.issuer_context.asset_definition_incarnation = key.asset_definition_incarnation;
        handle.handle_era = key.handle_era;
        handle.target_lane = key.target_lane;
        handle.axt_binding = key.binding;
        AxtHandleBudgetKey::from_handle(&handle)
    }
    #[test]
    fn unsigned_draft_cannot_decode_as_admission_handle() {
        let encoded = to_bytes(&sample_asset_handle_draft()).expect("encode unsigned draft");
        assert!(
            decode_from_bytes::<AssetHandle>(&encoded).is_err(),
            "the admission wire type must require its issuer context and signature"
        );
    }
    #[test]
    fn handle_replay_key_scopes_identical_ticket_by_dataspace_and_asset_incarnation() {
        let handle = sample_asset_handle();
        let key_a = AxtHandleReplayKey::from_handle(DataSpaceId::new(7), &handle);
        let key_b = AxtHandleReplayKey::from_handle(DataSpaceId::new(8), &handle);
        assert_ne!(key_a, key_b);
        assert_eq!(key_a.binding, key_b.binding);
        assert_eq!(key_a.handle_era, key_b.handle_era);
        assert_eq!(key_a.sub_nonce, key_b.sub_nonce);
        assert_eq!(key_a.target_lane, key_b.target_lane);
        assert_eq!(
            key_a.asset_definition_incarnation,
            key_b.asset_definition_incarnation
        );

        let mut retired_handle = handle.clone();
        retired_handle.issuer_context.asset_definition_incarnation =
            test_asset_incarnation(b"retired-replay-incarnation");
        let retired_key = AxtHandleReplayKey::from_handle(DataSpaceId::new(7), &retired_handle);
        assert_ne!(key_a, retired_key);
        assert_eq!(key_a.binding, retired_key.binding);
        assert_eq!(key_a.handle_era, retired_key.handle_era);
        assert_eq!(key_a.sub_nonce, retired_key.sub_nonce);
        assert_eq!(key_a.target_lane, retired_key.target_lane);

        let encoded = to_bytes(&key_b).expect("encode dataspace-scoped replay key");
        let decoded: AxtHandleReplayKey =
            decode_from_bytes(&encoded).expect("decode dataspace-scoped replay key");
        assert_eq!(decoded, key_b);
        assert_eq!(decoded.asset_dsid, DataSpaceId::new(8));
    }
    #[test]
    fn remote_spend_intent_commitment_binds_every_runtime_field() {
        let dsid = DataSpaceId::new(7);
        let incarnation = test_asset_incarnation(b"remote-spend-current");
        let replay_key =
            AxtHandleReplayKey::from_parts(dsid, incarnation, [0xA5; 32], 11, 12, LaneId::new(3));
        let amount = Quantity::from(5_u64);
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("domain"),
            "rose".parse().expect("asset name"),
        );
        let expected = compute_remote_spend_intent_commitment_v1(
            replay_key,
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        );
        assert_eq!(
            hex::encode(expected),
            "95d9bb334cb47eab805b80f1b18c94747c49b1d6bb51ec1e01c6cde688cca281",
            "V1 remote-spend commitment wire preimage changed"
        );
        assert_eq!(
            expected,
            compute_remote_spend_intent_commitment_v1(
                replay_key,
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            )
        );
        let mutations = [
            compute_remote_spend_intent_commitment_v1(
                AxtHandleReplayKey::from_parts(
                    dsid,
                    incarnation,
                    [0xA4; 32],
                    11,
                    12,
                    LaneId::new(3),
                ),
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                AxtHandleReplayKey::from_parts(
                    DataSpaceId::new(8),
                    incarnation,
                    [0xA5; 32],
                    11,
                    12,
                    LaneId::new(3),
                ),
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                AxtHandleReplayKey::from_parts(
                    dsid,
                    test_asset_incarnation(b"remote-spend-retired"),
                    [0xA5; 32],
                    11,
                    12,
                    LaneId::new(3),
                ),
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                AxtHandleReplayKey::from_parts(
                    dsid,
                    incarnation,
                    [0xA5; 32],
                    10,
                    12,
                    LaneId::new(3),
                ),
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                AxtHandleReplayKey::from_parts(
                    dsid,
                    incarnation,
                    [0xA5; 32],
                    11,
                    13,
                    LaneId::new(3),
                ),
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                AxtHandleReplayKey::from_parts(
                    dsid,
                    incarnation,
                    [0xA5; 32],
                    11,
                    12,
                    LaneId::new(4),
                ),
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                replay_key,
                &AssetDefinitionId::derive_from_components(
                    DomainId::try_new("axt", "universal").expect("domain"),
                    "iris".parse().expect("asset name"),
                ),
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                replay_key,
                &asset_definition,
                "mint",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                replay_key,
                &asset_definition,
                "transfer",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                replay_key,
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                &amount,
            ),
            compute_remote_spend_intent_commitment_v1(
                replay_key,
                &asset_definition,
                "transfer",
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
                &Quantity::from(6_u64),
            ),
        ];
        assert!(
            mutations
                .into_iter()
                .all(|commitment| commitment != expected)
        );
    }
    #[test]
    fn remote_spend_claim_roundtrips_and_matches_component_commitment() {
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("domain"),
            "rose".parse().expect("asset name"),
        );
        let replay_key = AxtHandleReplayKey::from_parts(
            DataSpaceId::new(7),
            test_asset_incarnation(b"remote-spend-claim"),
            [0xA5; 32],
            11,
            12,
            LaneId::new(3),
        );
        let claim = AxtRemoteSpendClaimV1::new(
            replay_key,
            asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            Quantity::from(5_u64),
        );
        let encoded = to_bytes(&claim).expect("encode remote-spend claim");
        let decoded: AxtRemoteSpendClaimV1 =
            decode_from_bytes(&encoded).expect("decode remote-spend claim");
        assert_eq!(decoded, claim);
        assert_eq!(
            compute_remote_spend_claim_commitment_v1(&claim),
            compute_remote_spend_intent_commitment_v1(
                claim.handle_replay_key,
                &claim.asset_definition_id,
                &claim.kind,
                &claim.from,
                &claim.to,
                &claim.effective_amount,
            )
        );
    }
    #[test]
    fn asset_incarnation_is_nonzero_canonical_and_binds_registration_identity() {
        let golden_network = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32])),
        );
        let golden_asset = AssetDefinitionId::from_uuid_bytes([
            0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
        ])
        .expect("golden asset identifier is canonical UUIDv4");
        let golden_header =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x33; 32]));
        let golden_execution = Hash::prehashed([0x55; 32]);
        let golden = AxtAssetIncarnationV1::derive(
            &golden_network,
            &golden_asset,
            &golden_header,
            &golden_execution,
            0x0102_0304_0506_0708,
        );
        assert_eq!(
            hex::encode(golden.as_bytes()),
            "a744e8a34aacfa4cdc9ae4407b88d3710c594bf2f5cbf7c68308353e33b4992d",
            "V1 domain/chunk ordering and big-endian ordinal are wire commitments"
        );

        let network = test_network_id(b"asset-incarnation-network");
        let asset = test_asset_definition_id();
        let header = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"asset-incarnation-registration-header",
        ));
        let execution_identity = Hash::new(b"asset-incarnation-registration-execution");
        let incarnation =
            AxtAssetIncarnationV1::derive(&network, &asset, &header, &execution_identity, 3);
        assert_eq!(incarnation.validate(), Ok(()));
        assert!(incarnation.as_bytes().iter().any(|byte| *byte != 0));
        assert_eq!(
            AxtAssetIncarnationV1::try_from_bytes(*incarnation.as_bytes()),
            Ok(incarnation)
        );
        assert_eq!(
            AxtAssetIncarnationV1::try_from_bytes([0; Hash::LENGTH]),
            Err(AxtAssetIncarnationValidationError::Zero)
        );
        assert_eq!(
            AxtAssetIncarnationV1::try_from_bytes(Hash::prehashed([0; Hash::LENGTH]).into()),
            Err(AxtAssetIncarnationValidationError::Zero),
            "the hash marker alone is still the logical absence sentinel"
        );
        let mut invalid_marker = [0; Hash::LENGTH];
        invalid_marker[0] = 1;
        assert_eq!(
            AxtAssetIncarnationV1::try_from_bytes(invalid_marker),
            Err(AxtAssetIncarnationValidationError::InvalidHashMarker)
        );

        let other_network = test_network_id(b"other-asset-incarnation-network");
        assert_ne!(
            incarnation,
            AxtAssetIncarnationV1::derive(&other_network, &asset, &header, &execution_identity, 3,)
        );
        let other_asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("domain"),
            "iris".parse().expect("asset name"),
        );
        assert_ne!(
            incarnation,
            AxtAssetIncarnationV1::derive(&network, &other_asset, &header, &execution_identity, 3,)
        );
        let other_header = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"other-asset-incarnation-registration-header",
        ));
        assert_ne!(
            incarnation,
            AxtAssetIncarnationV1::derive(&network, &asset, &other_header, &execution_identity, 3,)
        );
        let other_execution = Hash::new(b"other-asset-incarnation-registration-execution");
        assert_ne!(
            incarnation,
            AxtAssetIncarnationV1::derive(&network, &asset, &header, &other_execution, 3)
        );
        assert_ne!(
            incarnation,
            AxtAssetIncarnationV1::derive(&network, &asset, &header, &execution_identity, 4)
        );

        let encoded = to_bytes(&incarnation).expect("encode asset incarnation");
        assert_eq!(
            decode_from_bytes::<AxtAssetIncarnationV1>(&encoded).expect("decode asset incarnation"),
            incarnation
        );
        #[cfg(feature = "json")]
        {
            let context = issuer_context(network, DataSpaceId::new(7));
            let mut value = norito::json::to_value(&context).expect("encode issuer context JSON");
            let mut logical_zero = norito::json::to_value(&context.asset_definition_incarnation)
                .expect("encode incarnation JSON");
            logical_zero
                .as_array_mut()
                .expect("transparent incarnation JSON tuple")[0] =
                norito::json::to_value(&Hash::prehashed([0; Hash::LENGTH]))
                    .expect("encode logical-zero hash");
            value
                .as_object_mut()
                .expect("issuer context JSON object")
                .insert("asset_definition_incarnation".to_owned(), logical_zero);
            let decoded = norito::json::from_value::<AxtHandleIssuerContextV1>(value)
                .expect("the typed hash marker is syntactically valid");
            assert_eq!(
                decoded.validate(),
                Err(AxtAssetIncarnationValidationError::Zero),
                "contextual validation must reject the logical-zero token"
            );
        }
    }
    #[test]
    fn asset_handle_issuer_signature_binds_every_policy_field_and_network() {
        let issuer = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let impostor = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
        let dsid = DataSpaceId::new(7);
        let context = issuer_context(test_network_id(b"iroha-test-network"), dsid);
        let signed = sample_asset_handle_draft()
            .sign_by_issuer_v1(context, issuer.private_key())
            .expect("sign fixture handle");
        assert!(
            signed
                .verify_issuer_signature_v1(context, issuer.public_key())
                .is_ok()
        );
        assert!(
            signed
                .verify_issuer_signature_v1(context, impostor.public_key())
                .is_err(),
            "a forged issuer must not authenticate"
        );
        let mut wrong_contexts = Vec::new();
        let mut wrong = context;
        wrong.network_id = test_network_id(b"other-network");
        wrong_contexts.push(wrong);
        let mut wrong = context;
        wrong.asset_dsid = DataSpaceId::new(8);
        wrong_contexts.push(wrong);
        let mut wrong = context;
        wrong.asset_definition_incarnation = AxtAssetIncarnationV1::derive(
            &context.network_id,
            &test_asset_definition_id(),
            &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"other-asset-incarnation-header",
            )),
            &Hash::new(b"other-asset-incarnation-execution"),
            0,
        );
        wrong_contexts.push(wrong);
        let mut wrong = context;
        wrong.issuer = UniversalAccountId::from_hash(Hash::new(b"other-issuer"));
        wrong_contexts.push(wrong);
        let mut wrong = context;
        wrong.issuer_manifest_root[0] ^= 1;
        wrong_contexts.push(wrong);
        let mut wrong = context;
        wrong.code_root[0] ^= 1;
        wrong_contexts.push(wrong);
        let mut wrong = context;
        wrong.abi_version += 1;
        wrong_contexts.push(wrong);
        let mut wrong = context;
        wrong.abi_hash[0] ^= 1;
        wrong_contexts.push(wrong);
        for wrong in wrong_contexts {
            assert!(
                signed
                    .verify_issuer_signature_v1(wrong, issuer.public_key())
                    .is_err(),
                "issuer signatures must bind the exact external admission context"
            );
        }
        let mut altered = Vec::new();
        let mut handle = signed.clone();
        handle.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("domain"),
            "iris".parse().expect("asset name"),
        );
        altered.push(handle);
        let mut handle = signed.clone();
        handle.scope.push("mint".into());
        altered.push(handle);
        let mut handle = signed.clone();
        handle.subject.account.push_str("-altered");
        altered.push(handle);
        let mut handle = signed.clone();
        handle.budget.remaining = Quantity::from(51_u64);
        altered.push(handle);
        let mut handle = signed.clone();
        handle.handle_era += 1;
        altered.push(handle);
        let mut handle = signed.clone();
        handle.sub_nonce += 1;
        altered.push(handle);
        let mut handle = signed.clone();
        handle.group_binding.epoch_id += 1;
        altered.push(handle);
        let mut handle = signed.clone();
        handle.target_lane = LaneId::new(3);
        altered.push(handle);
        let mut handle = signed.clone();
        handle.axt_binding = AxtBinding::new([0xA4; 32]);
        altered.push(handle);
        let mut handle = signed.clone();
        handle.manifest_view_root[0] ^= 1;
        altered.push(handle);
        let mut handle = signed.clone();
        handle.expiry_slot += 1;
        altered.push(handle);
        let mut handle = signed.clone();
        handle.max_clock_skew_ms = Some(26);
        altered.push(handle);
        for altered in altered {
            assert!(
                altered
                    .verify_issuer_signature_v1(context, issuer.public_key())
                    .is_err(),
                "altering an issuer-bound handle field must invalidate the signature"
            );
        }
    }
    #[test]
    fn handle_budget_key_omits_only_counter_and_signature() {
        let signed = sample_asset_handle();
        let payload = signed.draft().issuer_payload_v1(signed.issuer_context);
        let expected = AxtHandleBudgetKey::from_handle(&signed);
        assert_eq!(
            expected,
            AxtHandleBudgetKey::from_issuer_payload_v1(&payload)
        );
        assert_eq!(expected.asset_dsid(), signed.issuer_context.asset_dsid);
        assert_eq!(expected.target_lane(), signed.target_lane);
        assert_eq!(expected.authorization_generation(), signed.handle_era);

        let mut next_counter = payload.clone();
        next_counter.next_handle_counter = next_counter.next_handle_counter.saturating_add(1);
        assert_eq!(
            expected,
            AxtHandleBudgetKey::from_issuer_payload_v1(&next_counter),
            "sequential sub-nonces must share one cumulative family"
        );
        let mut other_signature = signed.clone();
        other_signature.issuer_signature = Signature::from_bytes(&[0xA7; 64]);
        assert_eq!(
            expected,
            AxtHandleBudgetKey::from_handle(&other_signature),
            "signature encoding authenticates but does not identify the family"
        );

        let mut mutations = Vec::new();
        let mut changed = payload.clone();
        changed.context.network_id = test_network_id(b"other-budget-network");
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.context.asset_definition_incarnation = AxtAssetIncarnationV1::derive(
            &changed.context.network_id,
            &changed.asset_definition_id,
            &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"other-budget-asset-registration",
            )),
            &Hash::new(b"other-budget-asset-registration-execution"),
            0,
        );
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("axt", "universal").expect("domain"),
            "iris".parse().expect("asset name"),
        );
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.scope.push("mint".to_owned());
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.subject.origin_dsid = Some(DataSpaceId::new(99));
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.budget.remaining = Quantity::from(51_u64);
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.active_handle_era = changed.active_handle_era.saturating_add(1);
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.group_binding.epoch_id = changed.group_binding.epoch_id.saturating_add(1);
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.target_lane = LaneId::new(changed.target_lane.as_u32().saturating_add(1));
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.axt_binding = AxtBinding::new([0xA4; 32]);
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.manifest_view_root[0] ^= 1;
        mutations.push(changed);
        let mut changed = payload.clone();
        changed.expiry_slot = changed.expiry_slot.saturating_add(1);
        mutations.push(changed);
        let mut changed = payload;
        changed.max_clock_skew_ms = Some(26);
        mutations.push(changed);
        for changed in mutations {
            assert_ne!(
                expected,
                AxtHandleBudgetKey::from_issuer_payload_v1(&changed),
                "every other issuer-signed field must identify the budget family"
            );
        }
    }
    #[test]
    fn handle_budget_record_enforces_limits_atomically_and_roundtrips() {
        let mut handle = sample_asset_handle();
        handle.budget.remaining = Quantity::from(50_u64);
        handle.budget.per_use = Some(Quantity::from(10_u64));
        let key = AxtHandleBudgetKey::from_handle(&handle);
        let mut record = AxtHandleBudgetRecord::empty();
        let empty = record.clone();
        assert_eq!(
            record.try_consume(&key, &Quantity::zero(), 20),
            Err(AxtHandleBudgetConsumeError::ZeroAmount)
        );
        assert_eq!(record, empty, "failed consumption must be atomic");
        record
            .try_consume(&key, &Quantity::from(6_u64), 20)
            .expect("first consumption");
        record
            .try_consume(&key, &Quantity::from(4_u64), 10)
            .expect("exact per-use aggregate cap");
        record
            .validate_for_key(&key)
            .expect("valid persisted record");
        assert_eq!(record.consumed(), &Quantity::from(10_u64));
        assert_eq!(record.retain_until_slot(), 20, "retention is monotonic");
        let at_limit = record.clone();
        assert_eq!(
            record.try_consume(&key, &Quantity::from(1_u64), 30),
            Err(AxtHandleBudgetConsumeError::PerUseExceeded)
        );
        assert_eq!(
            record, at_limit,
            "limit rejection must not mutate retention"
        );

        handle.budget.per_use = None;
        let remaining_key = AxtHandleBudgetKey::from_handle(&handle);
        let mut remaining = AxtHandleBudgetRecord::empty();
        remaining
            .try_consume(&remaining_key, &Quantity::from(50_u64), 40)
            .expect("exact remaining cap");
        let at_remaining = remaining.clone();
        assert_eq!(
            remaining.try_consume(&remaining_key, &Quantity::from(1_u64), 50),
            Err(AxtHandleBudgetConsumeError::RemainingExceeded)
        );
        assert_eq!(remaining, at_remaining);

        let mut fractional_handle = handle.clone();
        fractional_handle.budget.remaining = Quantity::from(100_u64);
        let fractional_key = AxtHandleBudgetKey::from_handle(&fractional_handle);
        let mut fractional = AxtHandleBudgetRecord::empty();
        fractional
            .try_consume(
                &fractional_key,
                &"0.5".parse().expect("fractional quantity"),
                60,
            )
            .expect("canonical exact decimals need not have equal scales");

        let mut maximum_bytes = vec![0xFF_u8; 63];
        maximum_bytes.push(0x7F);
        let maximum = Quantity::from_canonical_numeric(Numeric::new(
            BigInt::from_twos_bytes(&maximum_bytes).expect("signed maximum"),
            0,
        ))
        .expect("signed maximum is a quantity");
        let mut overflow_handle = handle;
        overflow_handle.budget.remaining = maximum.clone();
        let overflow_key = AxtHandleBudgetKey::from_handle(&overflow_handle);
        let mut overflow = AxtHandleBudgetRecord {
            consumed: maximum,
            retain_until_slot: 70,
        };
        let before_overflow = overflow.clone();
        assert_eq!(
            overflow.try_consume(&overflow_key, &Quantity::from(1_u64), 80),
            Err(AxtHandleBudgetConsumeError::Arithmetic(
                NumericOperationError::MantissaOverflow
            ))
        );
        assert_eq!(overflow, before_overflow);

        assert_eq!(
            AxtHandleBudgetRecord::empty().validate_for_key(&key),
            Err(AxtHandleBudgetConsumeError::ZeroAmount),
            "committed state must not contain empty accumulator records"
        );
        let over_remaining = AxtHandleBudgetRecord {
            consumed: Quantity::from(51_u64),
            retain_until_slot: 90,
        };
        assert_eq!(
            over_remaining.validate_for_key(&remaining_key),
            Err(AxtHandleBudgetConsumeError::RemainingExceeded)
        );
        let over_per_use = AxtHandleBudgetRecord {
            consumed: Quantity::from(11_u64),
            retain_until_slot: 90,
        };
        assert_eq!(
            over_per_use.validate_for_key(&key),
            Err(AxtHandleBudgetConsumeError::PerUseExceeded)
        );

        let key_bytes = to_bytes(&key).expect("encode budget key");
        assert_eq!(
            decode_from_bytes::<AxtHandleBudgetKey>(&key_bytes).expect("decode budget key"),
            key
        );
        let record_bytes = to_bytes(&record).expect("encode budget record");
        assert_eq!(
            decode_from_bytes::<AxtHandleBudgetRecord>(&record_bytes)
                .expect("decode budget record"),
            record
        );
    }
    #[test]
    fn handle_counter_record_is_permanent_exact_and_checked() {
        let mut record = AxtHandleCounterRecord::initial(4);
        assert_eq!(record.next(), 1);
        assert_eq!(record.authorization_generation(), 4);
        assert_eq!(record.validate(), Ok(()));
        let installed = AxtHandleCounterRecord::try_from_parts(7, 9)
            .expect("validated authoritative setup ratchet");
        assert_eq!(installed.next(), 7);
        assert_eq!(installed.authorization_generation(), 9);
        assert_eq!(
            AxtHandleCounterRecord::try_from_parts(0, 9),
            Err(AxtHandleCounterError::ZeroNextCounter)
        );

        let before = record;
        assert_eq!(
            record.try_advance(4, 2),
            Err(AxtHandleCounterError::SubNonceMismatch {
                expected: 1,
                actual: 2,
            })
        );
        assert_eq!(record, before, "future-value rejection must be atomic");
        assert_eq!(
            record.try_advance(3, 1),
            Err(AxtHandleCounterError::AuthorizationGenerationMismatch {
                expected: 4,
                actual: 3,
            })
        );
        assert_eq!(record, before, "generation rejection must be atomic");
        assert_eq!(record.try_advance(4, 1), Ok(()));
        assert_eq!(record.next(), 2);
        assert_eq!(record.authorization_generation(), 4);
        assert_eq!(
            record.try_revoke_for_policy_transition(9),
            Ok(()),
            "policy transition must revoke both signed dimensions"
        );
        assert_eq!(record.next(), 3);
        assert_eq!(record.authorization_generation(), 9);
        let mut incremented =
            AxtHandleCounterRecord::try_from_parts(4, 9).expect("valid transition fixture");
        incremented
            .try_revoke_for_policy_transition(3)
            .expect("lower derived era still advances the generation");
        assert_eq!(incremented.next(), 5);
        assert_eq!(incremented.authorization_generation(), 10);
        assert_eq!(
            AxtHandleCounterRecord::initial(0).authorization_generation(),
            1,
            "zero is reserved for an absent/inactive policy"
        );
        assert_eq!(
            AxtHandleCounterRecord::try_from_parts(1, 0),
            Err(AxtHandleCounterError::ZeroAuthorizationGeneration)
        );
        assert_eq!(
            record.try_advance(9, 1),
            Err(AxtHandleCounterError::SubNonceMismatch {
                expected: 3,
                actual: 1,
            })
        );

        let invalid = AxtHandleCounterRecord {
            next: 0,
            authorization_generation: 9,
        };
        assert_eq!(
            invalid.validate(),
            Err(AxtHandleCounterError::ZeroNextCounter)
        );
        let inactive = AxtHandleCounterRecord {
            next: 1,
            authorization_generation: 0,
        };
        assert_eq!(
            inactive.validate(),
            Err(AxtHandleCounterError::ZeroAuthorizationGeneration)
        );
        let mut exhausted = AxtHandleCounterRecord {
            next: u64::MAX,
            authorization_generation: 9,
        };
        let before = exhausted;
        assert_eq!(
            exhausted.try_advance(9, u64::MAX),
            Err(AxtHandleCounterError::CounterExhausted)
        );
        assert_eq!(exhausted, before, "overflow rejection must be atomic");
        assert_eq!(
            exhausted.try_revoke_for_policy_transition(10),
            Err(AxtHandleCounterError::CounterExhausted)
        );
        assert_eq!(exhausted, before, "revocation overflow must be atomic");
        let mut generation_exhausted = AxtHandleCounterRecord {
            next: 3,
            authorization_generation: u64::MAX,
        };
        let before = generation_exhausted;
        assert_eq!(
            generation_exhausted.try_revoke_for_policy_transition(u64::MAX),
            Err(AxtHandleCounterError::AuthorizationGenerationExhausted)
        );
        assert_eq!(
            generation_exhausted, before,
            "generation overflow must leave both dimensions unchanged"
        );

        let encoded = to_bytes(&record).expect("encode handle counter ratchet");
        assert_eq!(
            decode_from_bytes::<AxtHandleCounterRecord>(&encoded)
                .expect("decode handle counter ratchet"),
            record
        );
        #[cfg(feature = "json")]
        {
            let value = norito::json::to_value(&record).expect("encode counter JSON");
            assert_eq!(
                norito::json::from_value::<AxtHandleCounterRecord>(value)
                    .expect("decode counter JSON"),
                record
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn handle_budget_key_json_storage_key_roundtrips() {
        let key = AxtHandleBudgetKey::from_handle(&sample_asset_handle());
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        let mut parser = norito::json::Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse JSON storage key");
        assert_eq!(
            AxtHandleBudgetKey::decode_json_key(&raw_key).expect("decode JSON storage key"),
            key
        );
        assert!(
            AxtHandleBudgetKey::decode_json_key(&(raw_key.clone() + " ")).is_err(),
            "non-canonical whitespace must not alias the canonical snapshot key"
        );
        assert!(
            AxtHandleBudgetKey::decode_json_key(&(raw_key + "true")).is_err(),
            "trailing JSON must not alias the canonical snapshot key"
        );
    }
    #[test]
    fn handle_sequence_accepts_only_exact_checked_progression() {
        let mut policy = AxtPolicyEntry {
            manifest_root: [0x5A; 32],
            target_lane: LaneId::new(2),
            active_handle_era: 9,
            next_handle_counter: 3,
            current_slot: 1,
        };
        let mut handle = sample_asset_handle();
        assert_eq!(next_axt_handle_sub_nonce(&policy, &handle), Ok(4));
        policy.next_handle_counter = 4;
        handle.sub_nonce = 4;
        assert_eq!(next_axt_handle_sub_nonce(&policy, &handle), Ok(5));
        handle.sub_nonce = 3;
        assert!(matches!(
            next_axt_handle_sub_nonce(&policy, &handle),
            Err(AxtHandleSequenceError::SubNonceMismatch {
                expected: 4,
                actual: 3
            })
        ));
        handle.sub_nonce = u64::MAX;
        assert!(matches!(
            next_axt_handle_sub_nonce(&policy, &handle),
            Err(AxtHandleSequenceError::SubNonceMismatch { .. })
        ));
        handle.sub_nonce = policy.next_handle_counter;
        handle.handle_era = u64::MAX;
        assert!(matches!(
            next_axt_handle_sub_nonce(&policy, &handle),
            Err(AxtHandleSequenceError::EraMismatch {
                expected: 9,
                actual: u64::MAX
            })
        ));
        policy.active_handle_era = u64::MAX;
        policy.next_handle_counter = u64::MAX;
        handle.handle_era = u64::MAX;
        handle.sub_nonce = u64::MAX;
        assert_eq!(
            next_axt_handle_sub_nonce(&policy, &handle),
            Err(AxtHandleSequenceError::CounterExhausted)
        );
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
            remote_spend_intent_commitments: Vec::new(),
        }
    }
    #[test]
    fn axt_v1_rejects_pre_release_binary_layouts_with_defaulted_fields() {
        #[derive(Encode)]
        struct PreReleaseDescriptor {
            dsids: Vec<DataSpaceId>,
        }
        #[derive(Encode)]
        struct PreReleaseProofEnvelope {
            dsid: DataSpaceId,
            manifest_root: [u8; 32],
            da_commitment: Option<[u8; 32]>,
            proof: Vec<u8>,
            fastpq_binding: Option<AxtFastpqBinding>,
        }
        #[derive(Encode)]
        struct PreReleaseProofBlob {
            payload: Vec<u8>,
        }
        #[derive(Encode)]
        struct PreReleaseHandleBudget {
            remaining: Quantity,
        }
        #[derive(Encode)]
        struct PreReleaseHandleSubject {
            account: String,
        }
        #[derive(Encode)]
        struct PreReleaseAssetHandleDraft {
            scope: Vec<String>,
            subject: HandleSubject,
            budget: HandleBudget,
            handle_era: u64,
            sub_nonce: u64,
            group_binding: GroupBinding,
            target_lane: LaneId,
            axt_binding: AxtBinding,
            manifest_view_root: [u8; 32],
            expiry_slot: u64,
            max_clock_skew_ms: Option<u32>,
        }
        #[derive(Encode)]
        struct PreReleaseSpendOp {
            kind: String,
            from: String,
            to: String,
            amount: Option<Quantity>,
        }

        let dsid = DataSpaceId::new(19);
        let descriptor = to_bytes(&PreReleaseDescriptor { dsids: vec![dsid] })
            .expect("encode pre-release AXT descriptor");
        assert!(
            decode_from_bytes::<AxtDescriptor>(&descriptor).is_err(),
            "the V1 descriptor must require its exact touch collection"
        );

        let envelope = to_bytes(&PreReleaseProofEnvelope {
            dsid,
            manifest_root: [0xA5; 32],
            da_commitment: None,
            proof: vec![0xC3],
            fastpq_binding: Some(sample_fastpq_binding(dsid)),
        })
        .expect("encode pre-release AXT proof envelope");
        assert!(
            decode_from_bytes::<AxtProofEnvelope>(&envelope).is_err(),
            "the V1 proof envelope must require amount and commitment slots"
        );

        let shortened = to_bytes(&PreReleaseProofBlob {
            payload: vec![0xC5],
        })
        .expect("encode pre-release proof blob");
        assert!(decode_from_bytes::<ProofBlob>(&shortened).is_err());
        let shortened = to_bytes(&PreReleaseHandleBudget {
            remaining: Quantity::from(5_u64),
        })
        .expect("encode pre-release handle budget");
        assert!(decode_from_bytes::<HandleBudget>(&shortened).is_err());
        let shortened = to_bytes(&PreReleaseHandleSubject {
            account: "sorau fixture".to_owned(),
        })
        .expect("encode pre-release handle subject");
        assert!(decode_from_bytes::<HandleSubject>(&shortened).is_err());
        let draft = sample_asset_handle_draft();
        let shortened = to_bytes(&PreReleaseAssetHandleDraft {
            scope: draft.scope,
            subject: draft.subject,
            budget: draft.budget,
            handle_era: draft.handle_era,
            sub_nonce: draft.sub_nonce,
            group_binding: draft.group_binding,
            target_lane: draft.target_lane,
            axt_binding: draft.axt_binding,
            manifest_view_root: draft.manifest_view_root,
            expiry_slot: draft.expiry_slot,
            max_clock_skew_ms: draft.max_clock_skew_ms,
        })
        .expect("encode pre-asset-binding handle draft");
        assert!(
            decode_from_bytes::<AssetHandleDraft>(&shortened).is_err(),
            "the V1 handle draft must require its issuer-signed asset definition"
        );
        let shortened = to_bytes(&PreReleaseSpendOp {
            kind: "transfer".to_owned(),
            from: "sorau source".to_owned(),
            to: "sorau destination".to_owned(),
            amount: None,
        })
        .expect("encode pre-asset-binding spend operation");
        assert!(
            decode_from_bytes::<SpendOp>(&shortened).is_err(),
            "the V1 spend operation must require its exact asset definition"
        );
    }
    #[test]
    fn axt_v1_json_requires_nullable_slots_and_rejects_unknown_fields() {
        let dsid = DataSpaceId::new(20);
        let envelope = AxtProofEnvelope {
            dsid,
            manifest_root: [0xA6; 32],
            da_commitment: None,
            proof: vec![0xC4],
            fastpq_binding: Some(sample_fastpq_binding(dsid)),
            committed_amount: None,
            amount_commitment: None,
        };
        for field in [
            "da_commitment",
            "proof",
            "fastpq_binding",
            "committed_amount",
            "amount_commitment",
        ] {
            let mut value =
                norito::json::to_value(&envelope).expect("serialize AXT proof envelope");
            assert!(
                value
                    .as_object_mut()
                    .expect("AXT proof envelope JSON object")
                    .remove(field)
                    .is_some(),
                "fixture must contain field {field}"
            );
            assert!(
                norito::json::from_value::<AxtProofEnvelope>(value).is_err(),
                "the V1 proof envelope must require {field}"
            );
        }
        let mut unknown = norito::json::to_value(&envelope).expect("serialize AXT proof envelope");
        unknown
            .as_object_mut()
            .expect("AXT proof envelope JSON object")
            .insert("pre_release_field".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<AxtProofEnvelope>(unknown).is_err(),
            "the V1 proof envelope must reject unknown fields"
        );

        let binding = envelope
            .fastpq_binding
            .as_ref()
            .expect("fixture has FASTPQ binding");
        let mut missing_effect = norito::json::to_value(binding).expect("serialize FASTPQ binding");
        missing_effect
            .as_object_mut()
            .expect("FASTPQ binding JSON object")
            .remove("effect_binding");
        assert!(
            norito::json::from_value::<AxtFastpqBinding>(missing_effect).is_err(),
            "the V1 FASTPQ binding must require its nullable effect slot"
        );

        let proof = ProofBlob {
            payload: vec![0xC5],
            expiry_slot: None,
        };
        let mut missing_expiry = norito::json::to_value(&proof).expect("serialize proof blob");
        missing_expiry
            .as_object_mut()
            .expect("proof blob JSON object")
            .remove("expiry_slot");
        assert!(
            norito::json::from_value::<ProofBlob>(missing_expiry).is_err(),
            "the V1 proof blob must require its nullable expiry slot"
        );
    }
    #[test]
    fn axt_v1_json_requires_handle_and_envelope_collections() {
        let handle = sample_asset_handle();
        let mut missing_asset = norito::json::to_value(&handle).expect("serialize asset handle");
        missing_asset
            .as_object_mut()
            .expect("asset handle JSON object")
            .remove("asset_definition_id");
        assert!(
            norito::json::from_value::<AssetHandle>(missing_asset).is_err(),
            "the V1 asset handle must require its issuer-signed asset definition"
        );
        let mut missing_skew = norito::json::to_value(&handle).expect("serialize asset handle");
        missing_skew
            .as_object_mut()
            .expect("asset handle JSON object")
            .remove("max_clock_skew_ms");
        assert!(
            norito::json::from_value::<AssetHandle>(missing_skew).is_err(),
            "the V1 asset handle must require its nullable clock-skew slot"
        );

        let dsid = DataSpaceId::new(21);
        let record = AxtEnvelopeRecord {
            binding: AxtBinding::new([0xD1; 32]),
            lane: LaneId::new(2),
            descriptor: sample_descriptor(dsid),
            touches: Vec::new(),
            proofs: Vec::new(),
            handles: Vec::new(),
            commit_height: 1,
        };
        for field in ["touches", "proofs", "handles"] {
            let mut value = norito::json::to_value(&record).expect("serialize AXT envelope");
            value
                .as_object_mut()
                .expect("AXT envelope JSON object")
                .remove(field);
            assert!(
                norito::json::from_value::<AxtEnvelopeRecord>(value).is_err(),
                "the V1 AXT envelope must require {field}"
            );
        }
    }
    #[test]
    fn axt_v1_json_requires_every_nested_nullable_slot() {
        macro_rules! assert_required_json_fields {
            ($value:expr, $ty:ty, [$($field:literal),+ $(,)?]) => {{
                let canonical = norito::json::to_value(&$value)
                    .expect("serialize canonical AXT JSON value");
                $(
                    let mut missing = canonical.clone();
                    assert!(
                        missing
                            .as_object_mut()
                            .expect("canonical AXT JSON object")
                            .remove($field)
                            .is_some(),
                        "fixture must contain field {}",
                        $field,
                    );
                    assert!(
                        norito::json::from_value::<$ty>(missing).is_err(),
                        "V1 AXT JSON must require field {}",
                        $field,
                    );
                )+
            }};
        }

        let effect = AxtEffectBinding {
            destination_domain: None,
            destination_account_id: None,
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: None,
            destination_asset_definition_id: None,
            source_amount_i64: None,
            destination_amount_i64: None,
        };
        assert_required_json_fields!(
            effect,
            AxtEffectBinding,
            [
                "destination_domain",
                "destination_account_id",
                "vault_account_id",
                "issuance_account_id",
                "source_asset_definition_id",
                "destination_asset_definition_id",
                "source_amount_i64",
                "destination_amount_i64",
            ]
        );

        let draft = sample_asset_handle_draft();
        assert_required_json_fields!(draft.subject, HandleSubject, ["origin_dsid"]);
        assert_required_json_fields!(draft.budget, HandleBudget, ["per_use"]);
        assert_required_json_fields!(
            draft,
            AssetHandleDraft,
            ["asset_definition_id", "max_clock_skew_ms"]
        );
        let context = issuer_context(test_network_id(b"json-v1-network"), DataSpaceId::new(7));
        assert_required_json_fields!(
            context,
            AxtHandleIssuerContextV1,
            ["asset_definition_incarnation"]
        );
        assert_required_json_fields!(
            draft.issuer_payload_v1(context),
            AssetHandleIssuerPayloadV1,
            ["asset_definition_id", "max_clock_skew_ms"]
        );
        let budget_key = AxtHandleBudgetKey::from_handle(&sample_asset_handle());
        assert_required_json_fields!(
            budget_key,
            AxtHandleBudgetKey,
            [
                "issuer_context",
                "asset_definition_id",
                "scope",
                "subject",
                "budget",
                "active_handle_era",
                "group_binding",
                "target_lane",
                "axt_binding",
                "manifest_view_root",
                "expiry_slot",
                "max_clock_skew_ms",
            ]
        );
        assert_required_json_fields!(
            AxtHandleBudgetRecord::empty(),
            AxtHandleBudgetRecord,
            ["consumed", "retain_until_slot"]
        );
        assert_required_json_fields!(
            AxtHandleCounterRecord::initial(1),
            AxtHandleCounterRecord,
            ["next", "authorization_generation"]
        );

        let op = SpendOp {
            asset_definition_id: test_asset_definition_id(),
            kind: "transfer".to_owned(),
            from: draft.subject.account.clone(),
            to: draft.subject.account.clone(),
            amount: None,
        };
        assert_required_json_fields!(op, SpendOp, ["asset_definition_id", "amount"]);
        assert_required_json_fields!(
            AxtHandleReplayKey::from_handle(DataSpaceId::new(7), &sample_asset_handle()),
            AxtHandleReplayKey,
            ["asset_definition_incarnation"]
        );
        assert_required_json_fields!(
            AxtReplayRecord {
                dataspace: DataSpaceId::new(7),
                budget_key: AxtHandleBudgetKey::from_handle(&sample_asset_handle()),
                used_slot: 1,
                retain_until_slot: 1,
            },
            AxtReplayRecord,
            ["budget_key"]
        );
        let fragment = AxtHandleFragment {
            handle: sample_asset_handle(),
            intent: RemoteSpendIntent {
                asset_dsid: DataSpaceId::new(7),
                op,
            },
            proof: None,
            amount: None,
            amount_commitment: None,
        };
        assert_required_json_fields!(
            fragment,
            AxtHandleFragment,
            ["proof", "amount", "amount_commitment"]
        );

        let reject = AxtRejectContext {
            reason: AxtRejectReason::PolicyDenied,
            dataspace: None,
            lane: None,
            snapshot_version: None,
            detail: "policy rejected".to_owned(),
            active_handle_era: None,
            next_handle_counter: None,
        };
        assert_required_json_fields!(
            reject,
            AxtRejectContext,
            [
                "dataspace",
                "lane",
                "snapshot_version",
                "active_handle_era",
                "next_handle_counter",
            ]
        );
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
            budget_key: AxtHandleBudgetKey::from_handle(&sample_asset_handle()),
            used_slot: 0,
            retain_until_slot: 0,
        };
        assert!(record.is_expired(0, 1));
        assert!(record.is_expired(5, 10));
    }
    #[test]
    fn replay_record_expires_strictly_after_effective_deadline() {
        let record = AxtReplayRecord {
            dataspace: DataSpaceId::new(1),
            budget_key: AxtHandleBudgetKey::from_handle(&sample_asset_handle()),
            used_slot: 10,
            retain_until_slot: 20,
        };
        assert!(!record.is_expired(19, 5));
        assert!(
            !record.is_expired(20, 5),
            "the handle remains valid through its inclusive expiry slot"
        );
        assert!(record.is_expired(21, 5));

        assert!(
            !record.is_expired(25, 15),
            "the configured retention window is also inclusive"
        );
        assert!(record.is_expired(26, 15));
    }
    #[test]
    fn replay_record_validation_uses_authoritative_key_and_canonical_storage_key() {
        let key = AxtHandleReplayKey::from_parts(
            DataSpaceId::new(7),
            test_asset_incarnation(b"persisted-replay-key"),
            [0xA5; 32],
            3,
            4,
            LaneId::new(2),
        );
        let record = AxtReplayRecord {
            dataspace: DataSpaceId::new(7),
            budget_key: budget_key_for_replay_key(&key),
            used_slot: 10,
            retain_until_slot: 10,
        };
        assert_eq!(record.validate_for_key(&key), Ok(()));

        let mut zero_era = key;
        zero_era.handle_era = 0;
        assert_eq!(
            record.validate_for_key(&zero_era),
            Err(AxtReplayRecordValidationError::InvalidReplayKey(
                AxtHandleReplayKeyValidationError::ZeroHandleEra
            ))
        );
        let mut zero_sub_nonce = key;
        zero_sub_nonce.sub_nonce = 0;
        assert_eq!(
            record.validate_for_key(&zero_sub_nonce),
            Err(AxtReplayRecordValidationError::InvalidReplayKey(
                AxtHandleReplayKeyValidationError::ZeroSubNonce
            ))
        );

        let mut invalid = record.clone();
        invalid.dataspace = DataSpaceId::new(8);
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::DataspaceMismatch)
        );
        let mut invalid = record.clone();
        invalid.budget_key.issuer_context.asset_dsid = DataSpaceId::new(8);
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::DataspaceMismatch)
        );
        let mut invalid = record.clone();
        invalid
            .budget_key
            .issuer_context
            .asset_definition_incarnation =
            AxtAssetIncarnationV1(Hash::prehashed([0; Hash::LENGTH]));
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::InvalidBudgetKey(
                AxtAssetIncarnationValidationError::Zero
            ))
        );
        let mut different_incarnation = key;
        different_incarnation.asset_definition_incarnation =
            test_asset_incarnation(b"different-replay-incarnation");
        let mut invalid = record.clone();
        invalid.budget_key = budget_key_for_replay_key(&different_incarnation);
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::BudgetIncarnationMismatch)
        );
        let mut invalid = record.clone();
        invalid.budget_key.active_handle_era = key.handle_era + 1;
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::BudgetGenerationMismatch)
        );
        let mut invalid = record.clone();
        invalid.budget_key.target_lane = LaneId::new(3);
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::BudgetLaneMismatch)
        );
        let mut different_binding = key;
        different_binding.binding = AxtBinding::new([0xD6; 32]);
        let mut invalid = record.clone();
        invalid.budget_key = budget_key_for_replay_key(&different_binding);
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::BudgetBindingMismatch)
        );
        let invalid = AxtReplayRecord {
            dataspace: DataSpaceId::new(7),
            budget_key: record.budget_key.clone(),
            used_slot: 0,
            retain_until_slot: 0,
        };
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::ZeroedSlots)
        );
        let invalid = AxtReplayRecord {
            dataspace: DataSpaceId::new(7),
            budget_key: record.budget_key.clone(),
            used_slot: 11,
            retain_until_slot: 10,
        };
        assert_eq!(
            invalid.validate_for_key(&key),
            Err(AxtReplayRecordValidationError::RetentionBeforeUse)
        );

        #[cfg(feature = "json")]
        {
            let mut encoded = String::new();
            key.encode_json_key(&mut encoded);
            let mut parser = norito::json::Parser::new(&encoded);
            let raw_key = parser.parse_string().expect("parse JSON replay key");
            assert_eq!(
                AxtHandleReplayKey::decode_json_key(&raw_key)
                    .expect("decode canonical JSON replay key"),
                key
            );
            assert!(AxtHandleReplayKey::decode_json_key(&(raw_key + " ")).is_err());

            let mut missing_incarnation =
                norito::json::to_value(&key).expect("encode replay key JSON");
            missing_incarnation
                .as_object_mut()
                .expect("replay key JSON object")
                .remove("asset_definition_incarnation");
            assert!(
                norito::json::from_value::<AxtHandleReplayKey>(missing_incarnation).is_err(),
                "the first-release replay key must require its asset incarnation"
            );

            let mut invalid_key_json =
                norito::json::to_value(&key).expect("encode replay key JSON");
            let mut logical_zero = norito::json::to_value(&key.asset_definition_incarnation)
                .expect("encode incarnation JSON");
            logical_zero
                .as_array_mut()
                .expect("transparent incarnation JSON tuple")[0] =
                norito::json::to_value(&Hash::prehashed([0; Hash::LENGTH]))
                    .expect("encode logical-zero hash");
            invalid_key_json
                .as_object_mut()
                .expect("replay key JSON object")
                .insert("asset_definition_incarnation".to_owned(), logical_zero);
            let invalid_key_json =
                norito::json::to_string(&invalid_key_json).expect("serialize invalid replay key");
            let invalid_key: AxtHandleReplayKey = norito::json::from_str(&invalid_key_json)
                .expect("logical-zero hash is syntactically decodable");
            assert_eq!(
                invalid_key.validate(),
                Err(AxtHandleReplayKeyValidationError::InvalidAssetIncarnation(
                    AxtAssetIncarnationValidationError::Zero
                ))
            );
            assert_eq!(
                record.validate_for_key(&invalid_key),
                Err(AxtReplayRecordValidationError::InvalidReplayKey(
                    AxtHandleReplayKeyValidationError::InvalidAssetIncarnation(
                        AxtAssetIncarnationValidationError::Zero
                    )
                ))
            );
            assert!(AxtHandleReplayKey::decode_json_key(&invalid_key_json).is_err());
        }
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
    #[expect(
        clippy::too_many_lines,
        reason = "one canonical envelope fixture verifies the full nested wire shape and required commit height"
    )]
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
                    asset_definition_id: test_asset_definition_id(),
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
                    issuer_context: AxtHandleIssuerContextV1 {
                        network_id: test_network_id(b"envelope-roundtrip-network"),
                        asset_dsid: dsid,
                        asset_definition_incarnation: AxtAssetIncarnationV1::derive(
                            &test_network_id(b"envelope-roundtrip-network"),
                            &test_asset_definition_id(),
                            &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                                b"envelope-roundtrip-asset-registration-header",
                            )),
                            &Hash::new(b"envelope-roundtrip-asset-registration-execution"),
                            0,
                        ),
                        issuer: UniversalAccountId::from_hash(Hash::new(
                            b"envelope-roundtrip-issuer",
                        )),
                        issuer_manifest_root: [1u8; 32],
                        code_root: Hash::new(b"envelope-roundtrip-code").into(),
                        abi_version: 1,
                        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
                    },
                    issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
                },
                intent: RemoteSpendIntent {
                    asset_dsid: dsid,
                    op: SpendOp {
                        asset_definition_id: test_asset_definition_id(),
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
    #[expect(
        clippy::too_many_lines,
        reason = "one policy-snapshot matrix covers canonical order, required fields, duplicates, and version binding"
    )]
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
            active_handle_era: 1,
            next_handle_counter: 1,
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
    fn proof_envelope_shape_matches_manifest_accepts_envelope_and_rejects_raw_root() {
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
        assert!(proof_envelope_shape_matches_manifest(
            &proof,
            dsid,
            manifest_root
        ));
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
        assert!(!proof_envelope_shape_matches_manifest(
            &missing_binding_proof,
            dsid,
            manifest_root
        ));
        let raw_proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(5),
        };
        assert!(!proof_envelope_shape_matches_manifest(
            &raw_proof,
            dsid,
            manifest_root
        ));
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
    fn fastpq_binding_shape_requires_canonical_remote_spend_commitment_set() {
        let mut binding = sample_fastpq_binding(DataSpaceId::new(18));
        binding.remote_spend_intent_commitments = vec![[0x11; 32], [0x22; 32]];
        assert!(fastpq_binding_shape_is_concrete(&binding));
        binding.remote_spend_intent_commitments = vec![[0x11; 32], [0x11; 32]];
        assert!(!fastpq_binding_shape_is_concrete(&binding));
        binding.remote_spend_intent_commitments = vec![[0x22; 32], [0x11; 32]];
        assert!(!fastpq_binding_shape_is_concrete(&binding));

        binding.remote_spend_intent_commitments = (0_u64
            ..=u64::try_from(MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1)
                .expect("V1 commitment limit fits u64"))
            .map(|index| {
                let mut commitment = [0_u8; 32];
                commitment[24..].copy_from_slice(&index.to_be_bytes());
                commitment
            })
            .collect();
        assert!(
            !fastpq_binding_shape_is_concrete(&binding),
            "an ordered but oversized commitment set must fail closed"
        );
    }
    #[test]
    fn proof_envelope_shape_matches_manifest_rejects_alternate_layout_and_restores_flags() {
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
            assert!(proof_envelope_shape_matches_manifest(
                &canonical_proof,
                dsid,
                manifest_root
            ));
            assert_eq!(
                norito::core::effective_decode_flags(),
                Some(alternate_flags)
            );
            assert!(!proof_envelope_shape_matches_manifest(
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
    fn proof_envelope_shape_matches_manifest_rejects_synthetic_binding_shape() {
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
        assert!(!proof_envelope_shape_matches_manifest(
            &proof,
            dsid,
            manifest_root
        ));
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
        assert!(!proof_envelope_shape_matches_manifest(
            &proof,
            dsid,
            manifest_root
        ));
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
        assert!(!proof_envelope_shape_matches_manifest(
            &proof,
            dsid,
            manifest_root
        ));
    }
    #[test]
    fn proof_envelope_shape_matches_manifest_rejects_mismatch() {
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
        assert!(!proof_envelope_shape_matches_manifest(
            &proof,
            dsid,
            manifest_root
        ));
        let raw_proof = ProofBlob {
            payload: bad_root.to_vec(),
            expiry_slot: Some(7),
        };
        assert!(!proof_envelope_shape_matches_manifest(
            &raw_proof,
            dsid,
            manifest_root
        ));
        let zero_root = [0u8; 32];
        let zero_proof = ProofBlob {
            payload: zero_root.to_vec(),
            expiry_slot: None,
        };
        assert!(!proof_envelope_shape_matches_manifest(
            &zero_proof,
            dsid,
            zero_root
        ));
    }
}
