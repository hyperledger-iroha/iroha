//! Independent native custody of the final-promotion transaction account.
//!
//! The stable authority is network plus deployment plus the account-signing role, never the
//! rotating account key. This history is independent of role-14 receipt custody and operations.
//! Policy, control and enrollment payloads retain their sole canonical Manifest byteframes.
//! Decoded public claims do not establish hardware custody or a fresh authorized observation.
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Total immutable custody revisions, including two emergency revocations.
pub const FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1: u64 = 8_194;
/// Configure/enroll ceiling, reserving capacity to revoke both current key generations.
pub const FINAL_PROMOTION_ACCOUNT_CUSTODY_NORMAL_REVISIONS_V1: u64 = 8_192;
/// Maximum complete native instruction or record, including canonical Norito framing.
pub const FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1: usize = 32 * 1024;
/// Domain of immutable account-custody control records, separate from receipt custody.
pub const FINAL_PROMOTION_ACCOUNT_CUSTODY_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.final-promotion-account.custody-control.v1\0";
/// Domain of the exact reviewed unsigned account transaction payload commitment.
///
/// The digest is SHA-256 of this prefix followed by the complete header-bearing canonical
/// Norito `TransactionPayload` frame, with no projection or excluded payload fields. The sole
/// computation belongs to the prepared account-transaction owner, not this public DTO module.
pub const FINAL_PROMOTION_ACCOUNT_TRANSACTION_PAYLOAD_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.final-promotion-account.transaction-payload.v1\0";

/// Monotonic revocation of the current signer or independent attester generation.
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
    name = "iroha_data_model::sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyRevocationV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionAccountCustodyRevocationV1 {
    /// Revoke the governed account-signing key generation without its cooperation.
    pub signer: bool,
    /// Revoke the independent attestation-key generation without its cooperation.
    pub attester: bool,
}

/// Sole account-custody action inventory; checks cannot create operation or reservation state.
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
    name = "iroha_data_model::sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyActionV1"
)]
#[norito(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FinalPromotionAccountCustodyActionV1 {
    /// Canonical Manifest `SignerCustodyPolicyV1`, with role 15, Ed25519 and this deployment.
    #[codec(index = 0)]
    Configure(Vec<u8>),
    /// Independently signed canonical `SignerCustodyRecordV1` for the committed predecessor.
    #[codec(index = 1)]
    Enroll(Vec<u8>),
    /// Set one or both current-generation revocations; never reset a revocation flag.
    #[codec(index = 2)]
    Revoke(FinalPromotionAccountCustodyRevocationV1),
    /// Check current enrolled account custody without mutating any history or replay counter.
    #[codec(index = 3)]
    Check(FinalPromotionAccountCustodyCheckV1),
}

/// One challenged current-custody predicate for an independently reviewed account transaction.
///
/// The outer instruction's account-custody CAS commits the complete governed binding and active
/// enrollment. An independent observer submits this Check; it is not signed by the target key.
/// Native execution and its finalized consumer must validate the target's key-derived universal
/// account and exact deployment Operate permission, as well as the observer's Check permission.
/// This is not a reusable approval, a role-14 operation request or a hardware-use capability.
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
    name = "iroha_data_model::sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyCheckV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionAccountCustodyCheckV1 {
    /// Fresh unpredictable 256-bit challenge, retired on every terminal consumer outcome.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub challenge: [u8; 32],
    /// Independently expected genesis-derived network identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub network_id: [u8; 32],
    /// Independently retained positive finalized floor, preceding this Check's execution.
    pub minimum_height: u64,
    /// Exact block hash at the independently retained finalized floor.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub minimum_block_hash: [u8; 32],
    /// Exact target account derived from the currently governed role-15 public key.
    pub expected_account: AccountId,
    /// Nonzero exact reviewed payload commitment under the sole account-payload domain above.
    ///
    /// The consumer independently retains the reviewed payload and this digest before Check
    /// submission. A candidate-selected digest cannot create transaction or custody authority.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub transaction_payload_digest: [u8; 32],
}

/// Actual deterministic account-custody execution, with no self-referential block hash.
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
    name = "iroha_data_model::sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyExecutionV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionAccountCustodyExecutionV1 {
    /// Actual executing block height; never a caller-submitted finality assertion.
    pub height: u64,
    /// Zero-based transition ordinal within this deployment's account-custody history and block.
    pub ordinal: u32,
    /// Actual execution block's logical timestamp in Unix milliseconds, not a UTC clock proof.
    pub recorded_at_unix_ms: u64,
    /// Registered universal governance account holding this deployment's management permission.
    pub authority: AccountId,
}

/// Immutable account-custody transition, retained across target-account and policy rotation.
///
/// Native storage additionally retains permanent first-use indexes for signer and attester keys.
/// Check execution does not append this record, consume revision capacity or authorize pruning.
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
    name = "iroha_data_model::sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyRecordV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionAccountCustodyRecordV1 {
    /// Stable deployment identity within the native network and sole account-signing role.
    pub deployment_id: String,
    /// Strictly one-based account-custody revision, independent of receipt custody history.
    pub revision: u64,
    /// Exact previous account-custody digest; zero only for the first configuration.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub predecessor_digest: [u8; 32],
    /// Exact canonical custody mutation and original submitting authority commitment.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub request_digest: [u8; 32],
    /// Deterministic native execution provenance supplied by the ledger.
    pub execution: FinalPromotionAccountCustodyExecutionV1,
    /// Canonical Manifest `SignerCustodyControlStateV1`, with the role-15 policy and active head.
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub control_state: Vec<u8>,
    /// Exact admitted custody frame; cleared by configuration and preserved by revocation.
    pub enrollment: Option<Vec<u8>>,
}

#[cfg(test)]
mod tests;
