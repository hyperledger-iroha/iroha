//! Finalized, chain-authoritative SoraFS PDP and PoTR outcome projections.
//!
//! The ledger deliberately does not retain PDP witness payloads.  It keeps the
//! terminal, payload-free projection and the detached provider attestation that
//! authenticates the canonical proof digest.  PoTR keeps the exact canonical
//! dual-signed receipt because that receipt is the exactly-once identity used by
//! latency repair and downstream audit consumers.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    sorafs::{capacity::ProviderId, pin_registry::ManifestDigest},
};

/// First-release proof-outcome projection version.
pub const PROOF_OUTCOME_RECORD_VERSION_V1: u16 = 1;
/// Hard item ceiling for one committed proof-outcome event query.
pub const PROOF_OUTCOME_QUERY_MAX_ITEMS_V1: usize = 64;
/// Hard encoded-byte ceiling for one committed proof-outcome event page.
pub const PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1: usize = 1024 * 1024;
/// Hard byte ceiling for one canonical dual-signed PoTR receipt.
pub const PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1: usize = 64 * 1024;
/// First-release governed proof-signer policy version.
pub const PROOF_OUTCOME_SIGNER_POLICY_VERSION_V1: u16 = 1;
/// Maximum canonical ML-DSA provider public-key bytes retained by signer policy.
pub const PROOF_OUTCOME_MAX_PROVIDER_KEY_BYTES_V1: usize = 8 * 1024;

/// Provider-scoped governed keys used to validate relayed PDP and PoTR outcomes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeSignerPolicyV1 {
    /// Policy schema version.
    pub version: u16,
    /// Provider governed by this policy.
    pub provider_id: ProviderId,
    /// Monotonic provider-scoped policy revision beginning at one.
    pub revision: u64,
    /// Digest of the previous canonical policy, absent only at revision one.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub predecessor_digest: Option<[u8; 32]>,
    /// Active council-verified admission envelope identity.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub admission_envelope_digest: [u8; 32],
    /// Admission-governed PDP Ed25519 public key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub pdp_public_key: [u8; 32],
    /// Admission-governed PoTR ML-DSA public key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub potr_mldsa_public_key: Vec<u8>,
    /// Governed gateway Ed25519 public key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub gateway_public_key: [u8; 32],
    /// Inclusive Unix timestamp at which this key set becomes active.
    pub valid_from_unix: u64,
    /// Inclusive Unix timestamp after which this key set may not authorize new outcomes.
    pub valid_until_unix: u64,
}

/// Activated provider-scoped proof signer policy with governance provenance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeSignerPolicyRecordV1 {
    /// Canonical governed key policy.
    pub policy: ProofOutcomeSignerPolicyV1,
    /// BLAKE3 digest of the exact canonical policy bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Governance authority that activated this revision.
    pub activated_by: AccountId,
    /// Committing block timestamp in milliseconds since Unix epoch.
    pub activated_at_unix_ms: u64,
}

/// Stable proof protocol discriminator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum ProofOutcomeKindV1 {
    /// Proof-of-data-possession terminal outcome.
    Pdp,
    /// Proof-of-timed-retrieval terminal outcome.
    Potr,
}

/// Payload-free stable PDP terminal classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "status", content = "detail", rename_all = "snake_case")]
pub enum PdpOutcomeStatusV1 {
    /// Exhaustive admission-bound verification succeeded.
    Accepted,
    /// No proof arrived before the response deadline.
    DeadlineExpired,
    /// A proof arrived after the response deadline.
    SubmissionLate,
    /// The proof claimed a timestamp beyond governed clock skew.
    FutureTimestamp,
    /// An authenticated proof failed binding, coverage, or Merkle verification.
    InvalidProof,
    /// The provider admission disappeared while the challenge was pending.
    AdmissionRevoked,
    /// The active admission no longer authorized the provider key or challenge.
    AdmissionInactive,
    /// Retained storage was unavailable for safe proof generation.
    StorageUnavailable,
}

impl PdpOutcomeStatusV1 {
    /// Whether the terminal archive must carry an authenticated proof.
    #[must_use]
    pub const fn requires_proof(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::SubmissionLate | Self::FutureTimestamp | Self::InvalidProof
        )
    }
}

/// Payload-free stable PoTR terminal classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "status", content = "detail", rename_all = "snake_case")]
pub enum PotrOutcomeStatusV1 {
    /// Retrieval completed within the governed deadline.
    Success,
    /// Retrieval exceeded the governed deadline.
    MissedDeadline,
    /// The provider returned an explicit failure.
    ProviderError,
    /// The gateway failed to complete the retrieval.
    GatewayError,
    /// The client cancelled the retrieval.
    ClientCancelled,
}

/// Detached Ed25519 provider attestation over a canonical PDP proof digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeEd25519AttestationV1 {
    /// Admission-governed provider public key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub public_key: [u8; 32],
    /// Strict Ed25519 signature from the canonical PDP proof.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub signature: [u8; 64],
}

/// Payload-free PDP-specific terminal projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PdpOutcomeProjectionV1 {
    /// Monotonic sequence assigned by the provider challenge protocol.
    pub source_sequence: u64,
    /// Challenge epoch.
    pub epoch_id: u64,
    /// Stable terminal classification.
    pub status: PdpOutcomeStatusV1,
    /// Canonical provider-signed proof digest, absent when no proof was submitted.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub proof_digest: Option<[u8; 32]>,
    /// Detached provider attestation, present exactly when `proof_digest` is present.
    pub provider_attestation: Option<ProofOutcomeEd25519AttestationV1>,
    /// Challenged segment count.
    pub sampled_segments: u16,
    /// Challenged hot-leaf count.
    pub sampled_hot_leaves: u16,
    /// Verified payload byte count; non-zero only for accepted proofs.
    pub sampled_bytes: u64,
    /// Challenge issuance time in seconds since Unix epoch.
    pub issued_at_unix: u64,
    /// Challenge response deadline in seconds since Unix epoch.
    pub response_deadline_unix: u64,
    /// Terminal decision time in seconds since Unix epoch.
    pub decided_at_unix: u64,
}

/// PoTR-specific terminal projection retaining the canonical signed receipt.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PotrOutcomeProjectionV1 {
    /// Stable receipt classification.
    pub status: PotrOutcomeStatusV1,
    /// Retrieval deadline in milliseconds.
    pub deadline_ms: u32,
    /// Observed retrieval latency in milliseconds.
    pub latency_ms: u32,
    /// Request issuance time in milliseconds since Unix epoch.
    pub requested_at_ms: u64,
    /// Response completion time in milliseconds since Unix epoch.
    pub responded_at_ms: u64,
    /// Receipt recording time in milliseconds since Unix epoch.
    pub recorded_at_ms: u64,
    /// Inclusive range start.
    pub range_start: u64,
    /// Inclusive range end.
    pub range_end: u64,
    /// Runtime-governed gateway Ed25519 public key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub gateway_public_key: [u8; 32],
    /// Digest of the runtime-governed provider ML-DSA public key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub governed_provider_key_digest: [u8; 32],
    /// Exact canonical dual-signed `sorafs_manifest::PotrReceiptV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_signed_receipt: Vec<u8>,
}

/// Protocol-specific terminal projection.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "projection", rename_all = "snake_case")]
pub enum ProofOutcomeProjectionV1 {
    /// PDP terminal metadata and detached proof attestation.
    Pdp(PdpOutcomeProjectionV1),
    /// PoTR metadata and exact dual-signed receipt.
    Potr(PotrOutcomeProjectionV1),
}

impl ProofOutcomeProjectionV1 {
    /// Return the stable protocol discriminator.
    #[must_use]
    pub const fn kind(&self) -> ProofOutcomeKindV1 {
        match self {
            Self::Pdp(_) => ProofOutcomeKindV1::Pdp,
            Self::Potr(_) => ProofOutcomeKindV1::Potr,
        }
    }
}

/// One chain-authoritative proof terminal outcome.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeRecordV1 {
    /// Projection schema version.
    pub version: u16,
    /// Protocol-scoped exactly-once identity: challenge ID for PDP and request scope for PoTR.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub identity_digest: [u8; 32],
    /// Digest of the canonical governance archive or final signed receipt.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub outcome_digest: [u8; 32],
    /// Provider named by the canonical proof material.
    pub provider_id: ProviderId,
    /// Manifest named by the canonical proof material.
    pub manifest_digest: ManifestDigest,
    /// Active council-verified admission envelope captured during runtime validation.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub admission_envelope_digest: [u8; 32],
    /// Transaction authority that committed the validated projection.
    pub submitted_by: AccountId,
    /// Committing block timestamp in milliseconds since Unix epoch.
    pub committed_at_unix_ms: u64,
    /// Protocol-specific terminal projection.
    pub projection: ProofOutcomeProjectionV1,
}

impl ProofOutcomeRecordV1 {
    /// Return the stable proof protocol discriminator.
    #[must_use]
    pub const fn kind(&self) -> ProofOutcomeKindV1 {
        self.projection.kind()
    }
}

/// Finalized block anchor for one coherent proof-outcome query result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeFinalizedCursorV1 {
    /// Finalized block height observed by the immutable state view.
    pub height: u64,
    /// Finalized block hash resolved from that same immutable state view.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
}

/// One authoritative proof outcome anchored to finalized chain state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeFinalizedRecordV1 {
    /// Finalized state anchor at which the outcome was read.
    pub finalized_cursor: ProofOutcomeFinalizedCursorV1,
    /// Chain-authoritative PDP or PoTR outcome.
    pub outcome: ProofOutcomeRecordV1,
}

/// Exclusive cursor for one committed proof-outcome event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeFinalizedEventCursorV1 {
    /// Monotonic event sequence beginning at one.
    pub sequence: u64,
    /// Finalized block height containing the event.
    pub block_height: u64,
    /// Finalized block hash resolved only after the block commits.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Proof-outcome event index within the committing block.
    pub event_index: u32,
}

/// Typed proof-outcome event with an unambiguous finalized-chain cursor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeFinalizedEventV1 {
    /// Monotonic proof-outcome event sequence beginning at one.
    pub sequence: u64,
    /// Committing block height.
    pub block_height: u64,
    /// Committing block hash resolved from finalized state.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Proof-outcome event index within the committing block.
    pub event_index: u32,
    /// Chain-authoritative proof outcome committed by this event.
    pub outcome: ProofOutcomeRecordV1,
}

impl ProofOutcomeFinalizedEventV1 {
    /// Return the exclusive cursor identifying this event.
    #[must_use]
    pub const fn cursor(&self) -> ProofOutcomeFinalizedEventCursorV1 {
        ProofOutcomeFinalizedEventCursorV1 {
            sequence: self.sequence,
            block_height: self.block_height,
            block_hash: self.block_hash,
            event_index: self.event_index,
        }
    }
}

/// Cursor-bounded page of typed committed proof-outcome events.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofOutcomeFinalizedEventPageV1 {
    /// Finalized state anchor shared by every event in the page.
    pub finalized_cursor: ProofOutcomeFinalizedCursorV1,
    /// Events in strictly increasing sequence and block/index order.
    pub events: Vec<ProofOutcomeFinalizedEventV1>,
    /// Whether at least one later committed event exists at this anchor.
    pub has_more: bool,
    /// Exclusive continuation cursor, present only when `has_more` is true.
    pub next_after: Option<ProofOutcomeFinalizedEventCursorV1>,
}
