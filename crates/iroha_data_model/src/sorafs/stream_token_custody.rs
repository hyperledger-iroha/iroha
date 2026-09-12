//! Bounded native StreamToken custody controls and immutable execution provenance.
//!
//! The control payload is the existing Manifest-owned canonical frame. Public role custody
//! history excludes token bodies and per-token operation, audit, reservation and completion state.
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId, sorafs::capacity::ProviderId,
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Total immutable history capacity per provider, including emergency revocations.
pub const STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1: u64 = 8_194;
/// Configure/enroll ceiling, reserving two final transitions for both revocation flags.
///
/// Earlier revocations consume this same revision budget. At daily renewal this admits roughly
/// 22 years; exhaustion fails closed and is not a disk-retention or per-token cardinality policy.
pub const STREAM_TOKEN_CUSTODY_NORMAL_REVISIONS_V1: u64 = 8_192;
/// Hard complete native record size, including header and payload padding.
pub const STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1: usize = 16 * 1024;
/// Domain separating native control history digests from statements and operation receipts.
pub const STREAM_TOKEN_CUSTODY_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.stream-token.custody-control.v1\0";

/// Current-generation signer and attester revocations committed by one custody mutation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_custody::SorafsStreamTokenCustodyRevocationV1"
)]
pub struct SorafsStreamTokenCustodyRevocationV1 {
    /// Revoke the governed signer key generation.
    pub signer: bool,
    /// Revoke the governed independent attester key generation.
    pub attester: bool,
}

/// One governed mutation with no caller-selected execution provenance.
///
/// JSON uses the `action` discriminator and `value` payload, with snake-case action names.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_custody::SorafsStreamTokenCustodyActionV1"
)]
#[norito(tag = "action", content = "value", rename_all = "snake_case")]
pub enum SorafsStreamTokenCustodyActionV1 {
    /// Exact canonical Manifest `StreamTokenCustodyPolicyV1` frame.
    #[codec(index = 0)]
    Configure(Vec<u8>),
    /// Exact signed Manifest `SignerCustodyRecordV1` frame, fully verified before admission.
    #[codec(index = 1)]
    Enroll(Vec<u8>),
    /// Strictly set one or both current-generation revocation flags; never clear a flag.
    #[codec(index = 2)]
    Revoke(SorafsStreamTokenCustodyRevocationV1),
}

/// Immutable native control transition, hashed without a self-referential current block hash.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_custody::StreamTokenCustodyControlRecordV1"
)]
pub struct StreamTokenCustodyControlRecordV1 {
    /// Stable provider scope, independent of key/policy rotations.
    pub provider_id: ProviderId,
    /// Strict one-based revision, bounded by the total retention policy.
    pub revision: u64,
    /// Exact previous canonical native record digest; zero only at revision one.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub predecessor_digest: [u8; 32],
    /// Exact canonical request and original authority commitment for bounded historical retries.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub request_digest: [u8; 32],
    /// Actual block height executing the mutation, never submitted by its caller.
    pub execution_height: u64,
    /// Zero-based provider control transition ordinal within that block.
    pub ordinal: u32,
    /// Actual deterministic execution block timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// Canonical registered transaction authority holding the exact provider permission.
    pub authority: AccountId,
    /// Canonical Manifest `StreamTokenCustodyControlStateV1`, not a competing signed schema.
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub control_state: Vec<u8>,
}

#[cfg(test)]
mod tests;
