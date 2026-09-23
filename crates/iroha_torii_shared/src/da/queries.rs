//! Canonical HTTP requests, responses, and cursors for DA ledger reads and proofs.
//!
//! These records have one shared owner for Torii and SDK consumers. Their explicit
//! Norito schema identities and field ordering are unchanged by the owner move.

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    da::{
        commitment::{
            DaCommitmentKey, DaCommitmentLocation, DaCommitmentProof, DaCommitmentWithLocation,
            DaProofPolicyBundle,
        },
        pin_intent::{DaPinIntentWithLocation, MAX_DA_PIN_INTENT_ALIAS_BYTES},
        types::StorageTicketId,
    },
    sorafs::pin_registry::ManifestDigest,
};
use std::num::NonZeroU64;

/// Maximum encoded HTTP body size for DA list, proof, and verification requests.
pub const DA_QUERY_REQUEST_MAX_BYTES: usize = 64 * 1024;
/// Raw index rows examined when a DA list request omits its page size.
pub const DEFAULT_DA_QUERY_PAGE_SIZE: usize = 100;
/// Maximum raw index rows a single DA list request may examine.
pub const MAX_DA_QUERY_PAGE_SIZE: usize = 1_000;

/// State-independent failure in a DA list or proof request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaQueryValidationError {
    /// The requested scan budget exceeds the protocol maximum.
    LimitOutOfRange {
        /// Requested number of raw index rows.
        provided: u64,
    },
    /// The cursor's ledger height and optional hash cannot identify a chain tip.
    NonCanonicalSnapshot,
    /// A pin cursor does not refer to a committed location within its snapshot.
    CursorOutsideSnapshot,
    /// A commitment lookup has neither a manifest hash nor a complete sequence key.
    MissingCommitmentSelector,
    /// A pin lookup has neither a named selector nor a complete sequence key.
    MissingPinIntentSelector,
    /// A pin alias exceeds the protocol's UTF-8 byte limit.
    AliasTooLong {
        /// Encoded UTF-8 byte length of the supplied alias.
        provided: usize,
    },
}
impl std::fmt::Display for DaQueryValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LimitOutOfRange { provided } => write!(
                formatter,
                "DA list limit is {provided}; maximum is {MAX_DA_QUERY_PAGE_SIZE}",
            ),
            Self::NonCanonicalSnapshot => formatter.write_str(
                "DA cursor snapshot must contain no block hash at height 0 and exactly one block hash at non-zero height",
            ),
            Self::CursorOutsideSnapshot => formatter.write_str(
                "DA pin cursor location must have a positive block height no greater than its snapshot height",
            ),
            Self::MissingCommitmentSelector => formatter.write_str(
                "DA commitment proof requires a manifest hash or complete lane_id, epoch and sequence",
            ),
            Self::MissingPinIntentSelector => formatter.write_str(
                "DA pin-intent proof requires a manifest hash, storage ticket, alias or complete lane_id, epoch and sequence",
            ),
            Self::AliasTooLong { provided } => write!(
                formatter,
                "DA pin-intent alias is {provided} UTF-8 bytes; maximum is {MAX_DA_PIN_INTENT_ALIAS_BYTES}",
            ),
        }
    }
}
impl std::error::Error for DaQueryValidationError {}

fn validate_page(
    limit: Option<NonZeroU64>,
    snapshot: Option<DaListSnapshot>,
) -> Result<usize, DaQueryValidationError> {
    let limit = limit.map_or(Ok(DEFAULT_DA_QUERY_PAGE_SIZE), |limit| {
        let provided = limit.get();
        usize::try_from(provided)
            .ok()
            .filter(|&limit| limit <= MAX_DA_QUERY_PAGE_SIZE)
            .ok_or(DaQueryValidationError::LimitOutOfRange { provided })
    })?;
    if snapshot.is_some_and(|snapshot| !snapshot.is_canonical()) {
        return Err(DaQueryValidationError::NonCanonicalSnapshot);
    }
    Ok(limit)
}

/// Canonical ledger tip that binds a DA list cursor to one immutable view.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::commitments::DaListSnapshot")]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct DaListSnapshot {
    /// Committed chain height observed while constructing the page.
    pub block_height: u64,
    /// Hash of the block at `block_height`, absent only for the empty chain.
    #[norito(required)]
    pub block_hash: Option<HashOf<BlockHeader>>,
}
impl DaListSnapshot {
    /// Return whether the height and optional block hash describe one ledger tip.
    ///
    /// The empty chain has no hash; every nonzero height must name its block.
    #[must_use]
    pub const fn is_canonical(self) -> bool {
        (self.block_height == 0) == self.block_hash.is_none()
    }
}
/// Forward-only cursor for canonically ordered DA commitments.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::commitments::DaCommitmentListCursor")]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct DaCommitmentListCursor {
    /// Immutable ledger view this cursor was issued against.
    pub snapshot: DaListSnapshot,
    /// Last raw commitment examined in `(lane_id, epoch, sequence)` order.
    pub after: DaCommitmentKey,
}
/// Request payload for bounded DA commitment traversal.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::commitments::DaCommitmentListRequest")]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaCommitmentListRequest {
    /// Maximum raw index rows to inspect; values above 1,000 are rejected.
    #[norito(default)]
    pub limit: Option<NonZeroU64>,
    /// Server-issued continuation cursor from the preceding page.
    #[norito(default)]
    pub cursor: Option<DaCommitmentListCursor>,
}
impl DaCommitmentListRequest {
    /// Validate the bounded scan budget and state-independent cursor shape.
    ///
    /// # Errors
    ///
    /// Rejects a limit above [`MAX_DA_QUERY_PAGE_SIZE`] or an inconsistent snapshot.
    pub fn validate(&self) -> Result<(), DaQueryValidationError> {
        self.page_size().map(|_| ())
    }

    /// Return the validated number of raw index rows to examine.
    ///
    /// # Errors
    ///
    /// Rejects a limit above [`MAX_DA_QUERY_PAGE_SIZE`] or an inconsistent snapshot.
    pub fn page_size(&self) -> Result<usize, DaQueryValidationError> {
        validate_page(self.limit, self.cursor.map(|cursor| cursor.snapshot))
    }
}

/// Exact selector used to generate one DA commitment proof.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::commitments::DaCommitmentProofRequest")]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaCommitmentProofRequest {
    /// Exact manifest digest to locate or constrain the returned record.
    #[norito(default)]
    pub manifest_hash: Option<ManifestDigest>,
    /// Exact numeric lane to locate or constrain the returned record.
    #[norito(default)]
    pub lane_id: Option<u32>,
    /// Exact lane epoch to locate or constrain the returned record.
    #[norito(default)]
    pub epoch: Option<u64>,
    /// Exact sequence within the lane and epoch.
    #[norito(default)]
    pub sequence: Option<u64>,
}
impl DaCommitmentProofRequest {
    /// Validate that the request identifies a record without an unbounded search.
    ///
    /// Additional selectors constrain the same record and are checked together.
    ///
    /// # Errors
    ///
    /// Rejects a request without a manifest hash or a complete lane/epoch/sequence key.
    pub fn validate(&self) -> Result<(), DaQueryValidationError> {
        if self.manifest_hash.is_some()
            || (self.lane_id.is_some() && self.epoch.is_some() && self.sequence.is_some())
        {
            Ok(())
        } else {
            Err(DaQueryValidationError::MissingCommitmentSelector)
        }
    }
}

/// Response surface for DA commitment listings.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::commitments::DaCommitmentListResponse")]
#[derive(
    Debug,
    Clone,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaCommitmentListResponse {
    /// Proof-policy bundle associated with this response.
    pub policies: DaProofPolicyBundle,
    /// Visible commitments among the bounded raw index rows examined.
    pub commitments: Vec<DaCommitmentWithLocation>,
    /// Cursor for the next bounded scan, or `None` when the ordered index is exhausted.
    #[norito(required)]
    pub next_cursor: Option<DaCommitmentListCursor>,
}
/// Response surface for DA commitment proofs.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::commitments::DaCommitmentProofResponse")]
#[derive(
    Debug,
    Clone,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaCommitmentProofResponse {
    /// Proof-policy bundle associated with this response.
    pub policies: DaProofPolicyBundle,
    /// Membership proof and its referenced block location.
    pub proof: DaCommitmentProof,
}
/// Verification response for a DA commitment Merkle proof.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::commitments::DaCommitmentVerifyResponse")]
#[derive(
    Debug,
    Clone,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaCommitmentVerifyResponse {
    /// Whether the supplied proof verifies against the node's committed block.
    pub valid: bool,
    /// Verification failure when `valid` is false; absent on success.
    #[norito(required)]
    pub error: Option<String>,
}
/// Forward-only cursor for canonically ordered DA pin intents.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::pin_intents::DaPinIntentListCursor")]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct DaPinIntentListCursor {
    /// Immutable ledger view this cursor was issued against.
    pub snapshot: DaListSnapshot,
    /// Last raw pin intent examined in canonical block-location order.
    pub after: DaCommitmentLocation,
}
impl DaPinIntentListCursor {
    /// Validate that the cursor identifies a scanned location within its ledger view.
    ///
    /// # Errors
    /// Rejects an inconsistent snapshot or a location outside its committed height.
    pub fn validate(self) -> Result<(), DaQueryValidationError> {
        if !self.snapshot.is_canonical() {
            return Err(DaQueryValidationError::NonCanonicalSnapshot);
        }
        if self.after.block_height == 0 || self.after.block_height > self.snapshot.block_height {
            return Err(DaQueryValidationError::CursorOutsideSnapshot);
        }
        Ok(())
    }
}
/// Request payload for bounded DA pin-intent traversal.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::pin_intents::DaPinIntentListRequest")]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaPinIntentListRequest {
    /// Maximum raw index rows to inspect; values above 1,000 are rejected.
    #[norito(default)]
    pub limit: Option<NonZeroU64>,
    /// Server-issued continuation cursor from the preceding page.
    #[norito(default)]
    pub cursor: Option<DaPinIntentListCursor>,
}
impl DaPinIntentListRequest {
    /// Validate the bounded scan budget and state-independent cursor shape.
    ///
    /// # Errors
    ///
    /// Rejects an excessive limit, inconsistent snapshot or cursor outside its snapshot.
    pub fn validate(&self) -> Result<(), DaQueryValidationError> {
        self.page_size().map(|_| ())
    }

    /// Return the validated number of raw index rows to examine.
    ///
    /// # Errors
    ///
    /// Rejects an excessive limit, inconsistent snapshot or cursor outside its snapshot.
    pub fn page_size(&self) -> Result<usize, DaQueryValidationError> {
        let limit = validate_page(self.limit, self.cursor.map(|cursor| cursor.snapshot))?;
        if let Some(cursor) = self.cursor {
            cursor.validate()?;
        }
        Ok(limit)
    }
}

/// Exact selector used to generate one DA pin-intent proof.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::pin_intents::DaPinIntentQueryRequest")]
#[derive(
    Debug,
    Default,
    Clone,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaPinIntentQueryRequest {
    /// Exact manifest digest to locate or constrain the returned record.
    #[norito(default)]
    pub manifest_hash: Option<ManifestDigest>,
    /// Exact durable storage ticket to locate or constrain the pin intent.
    #[norito(default)]
    pub storage_ticket: Option<StorageTicketId>,
    /// Exact UTF-8 pin alias to locate or constrain the pin intent.
    #[norito(default)]
    pub alias: Option<String>,
    /// Exact numeric lane to locate or constrain the returned record.
    #[norito(default)]
    pub lane_id: Option<u32>,
    /// Exact lane epoch to locate or constrain the returned record.
    #[norito(default)]
    pub epoch: Option<u64>,
    /// Exact sequence within the lane and epoch.
    #[norito(default)]
    pub sequence: Option<u64>,
}
impl DaPinIntentQueryRequest {
    /// Validate the exact lookup selector and the protocol alias byte bound.
    ///
    /// Additional selectors constrain the same record and are checked together.
    ///
    /// # Errors
    ///
    /// Rejects an oversized alias or a request without a named selector or a
    /// complete lane/epoch/sequence key.
    pub fn validate(&self) -> Result<(), DaQueryValidationError> {
        if let Some(alias) = &self.alias
            && alias.len() > MAX_DA_PIN_INTENT_ALIAS_BYTES
        {
            return Err(DaQueryValidationError::AliasTooLong {
                provided: alias.len(),
            });
        }
        if self.manifest_hash.is_some()
            || self.storage_ticket.is_some()
            || self.alias.is_some()
            || (self.lane_id.is_some() && self.epoch.is_some() && self.sequence.is_some())
        {
            Ok(())
        } else {
            Err(DaQueryValidationError::MissingPinIntentSelector)
        }
    }
}

/// Response surface for bounded DA pin-intent traversal.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::pin_intents::DaPinIntentListResponse")]
#[derive(
    Debug,
    Clone,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaPinIntentListResponse {
    /// Visible intents among the bounded raw index rows examined for this page.
    pub intents: Vec<DaPinIntentWithLocation>,
    /// Cursor for the next bounded scan, or `None` when the ordered index is exhausted.
    #[norito(required)]
    pub next_cursor: Option<DaPinIntentListCursor>,
}
/// Verification response for indexed DA pin intent location data.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::da::pin_intents::DaPinIntentVerifyResponse")]
#[derive(
    Debug,
    Clone,
    norito::derive::JsonDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct DaPinIntentVerifyResponse {
    /// Whether the supplied proof verifies against the node's committed block.
    pub valid: bool,
    /// Deterministic verification failure when `valid` is false.
    #[norito(required)]
    pub error: Option<String>,
}

#[cfg(test)]
#[path = "queries_tests.rs"]
mod tests;
