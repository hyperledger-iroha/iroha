//! Offline note models.
//!
//! Offline is the first production offline note surface. The legacy
//! allowance, witness-lineage, plaintext receipt, and aggregate proof models are
//! intentionally absent from this module.

use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model_derive::model;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};

pub use self::model::*;
use crate::{
    ChainId,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    proof::ProofBox,
    proof::VerifyingKeyId,
};

/// Prefix embedded into offline instruction rejection messages.
///
/// Mobile SDKs parse the label after this prefix up to the first `:` to recover
/// stable machine-readable error codes.
pub const OFFLINE_REJECTION_REASON_PREFIX: &str = "offline_reason::";
/// Asset-definition metadata key that enables Offline escrow tracking.
pub const OFFLINE_ASSET_ENABLED_METADATA_KEY: &str = "offline.enabled";
/// Domain-separation tag for deterministic offline escrow derivation.
pub const OFFLINE_ESCROW_SEED_LABEL: &str = "iroha.offline.escrow";
/// Canonical Offline key-certificate format marker for the first release.
pub const OFFLINE_NOTE_KEY_CERTIFICATE_VERSION: u16 = 1;
/// Domain-separation tag for wallet-derived Offline Note note commitments.
pub const OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN: &str = "iroha:offline-note:note-commitment";
/// Domain-separation tag for wallet-derived Offline Note input nullifiers.
pub const OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN: &str = "iroha:offline-note:input-nullifier";
/// Domain-separation tag for wallet-derived Offline Note payment token identifiers.
pub const OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN: &str = "iroha:offline-note:payment-token-id";
/// Signature algorithm label for Ed25519 Offline Bearer v2 signatures.
pub const OFFLINE_BEARER_SIGNATURE_ALGORITHM_ED25519: &str = "ed25519";
/// Signature algorithm label for ECDSA P-256 with SHA-256 Offline Bearer v2 signatures.
pub const OFFLINE_BEARER_SIGNATURE_ALGORITHM_ECDSA_P256_SHA256: &str = "ecdsa_p256_sha256";
/// Raw Ed25519 public-key encoding label for Offline Bearer v2 certificates.
pub const OFFLINE_BEARER_PUBLIC_KEY_ENCODING_RAW_ED25519: &str = "raw_ed25519";
/// X9.62 uncompressed P-256 public-key encoding label for Offline Bearer v2 certificates.
pub const OFFLINE_BEARER_PUBLIC_KEY_ENCODING_X963_P256: &str = "x963_uncompressed_p256";

/// Error returned when Offline Note canonical derivation inputs are invalid.
#[derive(Debug)]
pub enum OfflineNoteDerivationError {
    /// Random secret material must be exactly 32 bytes.
    InvalidRandomBytesLength {
        /// Name of the invalid field.
        field: &'static str,
        /// Expected byte count.
        expected: usize,
        /// Actual byte count.
        actual: usize,
    },
    /// Canonical Norito encoding failed.
    Encode(norito::Error),
}

impl core::fmt::Display for OfflineNoteDerivationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidRandomBytesLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "Offline Note {field} must be exactly {expected} bytes (found {actual})"
            ),
            Self::Encode(err) => write!(f, "failed to encode Offline Note preimage: {err}"),
        }
    }
}

impl std::error::Error for OfflineNoteDerivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidRandomBytesLength { .. } => None,
            Self::Encode(err) => Some(err),
        }
    }
}

impl From<norito::Error> for OfflineNoteDerivationError {
    fn from(err: norito::Error) -> Self {
        Self::Encode(err)
    }
}

/// Derive the deterministic Offline escrow account for an asset definition.
#[must_use]
pub fn offline_escrow_account_id(
    chain_id: &ChainId,
    definition_id: &AssetDefinitionId,
) -> AccountId {
    let seed_material = format!(
        "{OFFLINE_ESCROW_SEED_LABEL}|{}|{definition_id}",
        chain_id.as_str()
    );
    let seed: [u8; Hash::LENGTH] = Hash::new(seed_material).into();
    let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
    AccountId::new(keypair.public_key().clone())
}

#[model]
mod model {
    use super::*;

    /// Compact CA-issued certificate for an Offline one-use note key.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteKeyCertificate {
        /// Certificate format marker.
        pub version: u16,
        /// Platform class, for example `ios-appattest` or `android-keymint`.
        pub platform: String,
        /// Issuer-scoped one-use key identifier.
        pub key_id: String,
        /// Device identifier bound by the offline CA.
        pub device_id: String,
        /// Account authorized to control the note key.
        pub account_id: AccountId,
        /// Ed25519 public key bytes for local note/proof signatures.
        pub public_key: Vec<u8>,
        /// Hardware assertion scheme bound to this note key.
        pub assertion_scheme: String,
        /// Hardware assertion key algorithm, for example `ecdsa-p256-sha256`.
        pub assertion_key_algorithm: String,
        /// Hardware assertion public key bytes, for example SEC1 P-256.
        pub assertion_public_key: Vec<u8>,
        /// Hardware one-use limit when the platform exposes it.
        pub assertion_usage_count_limit: Option<u32>,
        /// True when the issuer verified hardware one-use semantics.
        pub one_use: bool,
        /// Offline CA signature over the compact certificate payload.
        pub issuer_signature: Signature,
    }

    /// Canonical payload signed by Offline key-certificate issuers.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteKeyCertificatePayload {
        /// Domain separator for the signed payload.
        pub domain: String,
        /// Certificate format marker.
        pub version: u16,
        /// Platform class, for example `ios-appattest` or `android-keymint`.
        pub platform: String,
        /// Issuer-scoped one-use key identifier.
        pub key_id: String,
        /// Device identifier bound by the offline CA.
        pub device_id: String,
        /// Account authorized to control the note key.
        pub account_id: AccountId,
        /// Ed25519 public key bytes for local note/proof signatures.
        pub public_key: Vec<u8>,
        /// Hardware assertion scheme bound to this note key.
        pub assertion_scheme: String,
        /// Hardware assertion key algorithm, for example `ecdsa-p256-sha256`.
        pub assertion_key_algorithm: String,
        /// Hardware assertion public key bytes, for example SEC1 P-256.
        pub assertion_public_key: Vec<u8>,
        /// Hardware one-use limit when the platform exposes it.
        pub assertion_usage_count_limit: Option<u32>,
        /// True when the issuer verified hardware one-use semantics.
        pub one_use: bool,
    }

    /// Verifier-key-backed recursive proof carried by Offline note tokens.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRecursiveProof {
        /// Stable verifier key identifier selected by the operator and stored in WSV.
        pub verifier_key_id: VerifyingKeyId,
        /// Public input commitment hash.
        pub public_inputs_hash: Hash,
        /// Compact recursive proof payload encoded as an `OpenVerifyEnvelope`.
        pub proof: ProofBox,
    }

    /// Issuer-side note issuance record for online load/consolidation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssue {
        /// Deterministic note commitment.
        pub note_commitment: Hash,
        /// Owner key certificate for this note.
        pub key_certificate: OfflineNoteKeyCertificate,
        /// Asset held by the note.
        pub asset: AssetId,
        /// Note amount.
        pub amount: Numeric,
    }

    /// Ledger-recognized note claim bound to one compact Offline note certificate.
    ///
    /// Issuer loads create this claim directly; P2P bearer outputs create the same claim only
    /// when their audit lineage is submitted, either before redemption or earlier in the same
    /// transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssuedClaim {
        /// Domain separator for the issued-note claim.
        pub domain: String,
        /// Deterministic note commitment recorded at issuance.
        pub note_commitment: Hash,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Asset held by the issued note.
        pub asset: AssetId,
        /// Note amount reserved into offline escrow.
        pub amount: Numeric,
    }

    /// Redeemable note output observed during Offline audit.
    ///
    /// The output is final for offline bearers when received locally. The ledger recognizes it
    /// after the corresponding audit is committed.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditOutputClaim {
        /// Deterministic note commitment created by the audited transfer.
        pub note_commitment: Hash,
        /// Owner key certificate for this output note.
        pub key_certificate: OfflineNoteKeyCertificate,
        /// Asset held by this output note.
        pub asset: AssetId,
        /// Output amount reserved in offline escrow.
        pub amount: Numeric,
    }

    /// Redemption payload submitted online when defunding a bearer note.
    ///
    /// The source claim must already be ledger-recognized. For unanchored P2P bearer outputs,
    /// submit their ordered audit lineage before this redeem instruction in the same transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRedeem {
        /// Ledger-recognized note commitment consumed by this redemption.
        pub source_note_commitment: Hash,
        /// Nullifiers consumed by the redeeming token.
        pub input_nullifiers: Vec<Hash>,
        /// Compact certificate for the one-use note key that signed the proof.
        pub sender_key_certificate: OfflineNoteKeyCertificate,
        /// Recipient account credited online.
        pub recipient: AccountId,
        /// Asset being redeemed.
        pub asset: AssetId,
        /// Redeemed amount.
        pub amount: Numeric,
        /// Compact recursive proof for the final note state.
        pub recursive_proof: OfflineNoteRecursiveProof,
    }

    /// Public inputs bound by an Offline redemption proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRedeemPublicInputs {
        /// Domain separator for the redemption public inputs.
        pub domain: String,
        /// Ledger-recognized note commitment consumed by this redemption.
        pub source_note_commitment: Hash,
        /// Nullifiers consumed by the redeeming token.
        pub input_nullifiers: Vec<Hash>,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Recipient account credited online.
        pub recipient: AccountId,
        /// Asset being redeemed.
        pub asset: AssetId,
        /// Redeemed amount.
        pub amount: Numeric,
    }

    /// Audit bundle for Offline P2P bearer lineage.
    ///
    /// It is not required for offline transfer finality, but it anchors P2P output claims so the
    /// ledger can later redeem them from offline escrow.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditBundle {
        /// Payment token identifier.
        pub token_id: Hash,
        /// Compact certificate for the one-use note key that signed the proof.
        pub sender_key_certificate: OfflineNoteKeyCertificate,
        /// Input nullifiers observed in the token.
        pub input_nullifiers: Vec<Hash>,
        /// Issued input claims consumed by the token.
        pub input_claims: Vec<OfflineNoteIssuedClaim>,
        /// Output note commitments created by the token.
        pub output_commitments: Vec<Hash>,
        /// Redeemable output claims created by the token.
        pub output_claims: Vec<OfflineNoteAuditOutputClaim>,
        /// Optional recursive proof for audit/replay checks.
        pub recursive_proof: OfflineNoteRecursiveProof,
    }

    /// Public inputs bound by an Offline optional audit proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditPublicInputs {
        /// Domain separator for the audit public inputs.
        pub domain: String,
        /// Payment token identifier.
        pub token_id: Hash,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Input nullifiers observed in the token.
        pub input_nullifiers: Vec<Hash>,
        /// Issued input claims consumed by the token.
        pub input_claims: Vec<OfflineNoteIssuedClaim>,
        /// Output note commitments created by the token.
        pub output_commitments: Vec<Hash>,
        /// Redeemable output claims created by the token.
        pub output_claims: Vec<OfflineNoteIssuedClaim>,
    }

    /// Origin of a wallet-derived Offline Note note commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssuerLoadOrigin {
        /// Wallet operation id sent to Torii.
        pub operation_id: String,
        /// Issuer lineage id updated by Torii.
        pub lineage_id: String,
        /// Local lineage revision after issuing the note.
        pub local_revision: u64,
    }

    /// Origin data for an offline peer-to-peer payment token output.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteP2pOutputOrigin {
        /// Recipient payment request id.
        pub payment_request_id: String,
        /// Output index inside the payment token.
        pub output_index: u32,
    }

    /// Canonical preimage used to derive an Offline Note note commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteCommitmentPreimage {
        /// Domain separator for note commitments.
        pub domain: String,
        /// Chain id that scopes this note.
        pub chain_id: ChainId,
        /// Hash of the owner key certificate payload.
        pub owner_key_certificate_payload_hash: Hash,
        /// Asset held by the note.
        pub asset: AssetId,
        /// Note amount.
        pub amount: Numeric,
        /// Wallet-generated 32-byte note secret.
        pub note_secret: Vec<u8>,
        /// Origin metadata that separates issuer loads from P2P outputs.
        pub origin: OfflineNoteCommitmentOrigin,
    }

    /// Canonical preimage used to derive an Offline Note input nullifier.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteInputNullifierPreimage {
        /// Domain separator for input nullifiers.
        pub domain: String,
        /// Chain id that scopes this nullifier.
        pub chain_id: ChainId,
        /// Commitment of the note being spent.
        pub source_note_commitment: Hash,
        /// Hash of the owner key certificate payload.
        pub owner_key_certificate_payload_hash: Hash,
        /// Wallet-generated 32-byte note secret.
        pub note_secret: Vec<u8>,
    }

    /// Canonical preimage used to derive an Offline Note payment token id.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNotePaymentTokenIdPreimage {
        /// Domain separator for payment token ids.
        pub domain: String,
        /// Chain id that scopes this payment token.
        pub chain_id: ChainId,
        /// Wallet-local payment request id that binds this token to one receive request.
        pub payment_request_id: String,
        /// Wallet-local token creation time in Unix milliseconds.
        pub created_at_ms: u64,
        /// Wallet-generated 32-byte payment token nonce.
        pub token_nonce: Vec<u8>,
        /// Hash of the sender key certificate payload.
        pub sender_key_certificate_payload_hash: Hash,
        /// Input nullifiers consumed by the token.
        pub input_nullifiers: Vec<Hash>,
        /// Output commitments created by the token.
        pub output_commitments: Vec<Hash>,
    }

    /// Per-asset Offline Bearer send limits distributed in policy bundles.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerAssetSendLimitV2 {
        /// Asset definition governed by this limit.
        pub asset_definition_id: String,
        /// Maximum amount allowed for a single transfer of this asset.
        pub max_transaction_amount: String,
        /// Maximum amount allowed per local day.
        pub daily_send_limit: String,
        /// Maximum amount allowed per local month.
        pub monthly_send_limit: String,
    }

    /// Signed Offline Bearer v2 issuer policy bundle.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerPolicyBundleV2 {
        /// Issuer-scoped policy identifier.
        pub policy_id: String,
        /// Hash of the current policy contents as published by the issuer.
        pub policy_hash_hex: String,
        /// Settlement issuer identifier.
        pub issuer_id: String,
        /// Policy issue time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Policy expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Maximum certificate age accepted at event time.
        pub max_certificate_age_ms: u64,
        /// Maximum policy age accepted at event time.
        pub max_policy_age_ms: u64,
        /// Maximum receive/debit token age.
        pub max_token_age_ms: u64,
        /// Maximum offline balance a purse may hold.
        pub max_offline_balance: String,
        /// Maximum amount allowed for one transfer.
        pub max_transaction_amount: String,
        /// Hardware classes allowed by this policy.
        pub allowed_hardware_classes: Vec<String>,
        /// Blacklisted canonical account identifiers.
        pub blacklisted_account_ids: Vec<String>,
        /// Blacklisted device identifiers.
        pub blacklisted_device_ids: Vec<String>,
        /// Blacklisted key identifiers.
        pub blacklisted_key_ids: Vec<String>,
        /// Signature algorithm used for the issuer signature.
        pub signature_algorithm: String,
        /// Issuer signature over the canonical policy payload.
        pub issuer_signature: Vec<u8>,
        /// Monotonic policy epoch.
        pub policy_epoch: u64,
        /// Human-readable policy source.
        pub policy_source: String,
        /// Revoked certificate identifiers.
        pub revoked_certificate_ids: Vec<String>,
        /// Revoked transfer or receive-request identifiers.
        pub revoked_transfer_ids: Vec<String>,
        /// Per-asset send limits.
        pub asset_send_limits: Vec<OfflineBearerAssetSendLimitV2>,
    }

    /// Canonical payload signed by Offline Bearer policy issuers.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerPolicyBundlePayloadV2 {
        /// Domain separator for the signed payload.
        pub domain: String,
        /// Issuer-scoped policy identifier.
        pub policy_id: String,
        /// Hash of the current policy contents as published by the issuer.
        pub policy_hash_hex: String,
        /// Settlement issuer identifier.
        pub issuer_id: String,
        /// Policy issue time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Policy expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Maximum certificate age accepted at event time.
        pub max_certificate_age_ms: u64,
        /// Maximum policy age accepted at event time.
        pub max_policy_age_ms: u64,
        /// Maximum receive/debit token age.
        pub max_token_age_ms: u64,
        /// Maximum offline balance a purse may hold.
        pub max_offline_balance: String,
        /// Maximum amount allowed for one transfer.
        pub max_transaction_amount: String,
        /// Hardware classes allowed by this policy.
        pub allowed_hardware_classes: Vec<String>,
        /// Blacklisted canonical account identifiers.
        pub blacklisted_account_ids: Vec<String>,
        /// Blacklisted device identifiers.
        pub blacklisted_device_ids: Vec<String>,
        /// Blacklisted key identifiers.
        pub blacklisted_key_ids: Vec<String>,
        /// Signature algorithm used for the issuer signature.
        pub signature_algorithm: String,
        /// Monotonic policy epoch.
        pub policy_epoch: u64,
        /// Human-readable policy source.
        pub policy_source: String,
        /// Revoked certificate identifiers.
        pub revoked_certificate_ids: Vec<String>,
        /// Revoked transfer or receive-request identifiers.
        pub revoked_transfer_ids: Vec<String>,
        /// Per-asset send limits.
        pub asset_send_limits: Vec<OfflineBearerAssetSendLimitV2>,
    }

    /// Signed Offline Bearer v2 purse certificate.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerCertificateV2 {
        /// Issuer-scoped certificate identifier.
        pub certificate_id: String,
        /// Chain identifier that scopes the purse.
        pub chain_id: String,
        /// Settlement issuer identifier.
        pub issuer_id: String,
        /// Hardware purse identifier.
        pub purse_id: String,
        /// Canonical account identifier.
        pub account_id: String,
        /// Asset definition held by this purse.
        pub asset_definition_id: String,
        /// Device identifier certified by the issuer.
        pub device_id: String,
        /// Signing key identifier certified by the issuer.
        pub key_id: String,
        /// Hardware class certified by the issuer.
        pub hardware_class: String,
        /// Signature algorithm used by this purse key.
        pub signature_algorithm: String,
        /// Public-key encoding for this purse key.
        pub public_key_encoding: String,
        /// Purse public key bytes.
        pub public_key: Vec<u8>,
        /// Certificate issue time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Certificate expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Policy identifier bound by this certificate.
        pub policy_id: String,
        /// Policy hash bound by this certificate.
        pub policy_hash_hex: String,
        /// Issuer signature over the canonical certificate payload.
        pub issuer_signature: Vec<u8>,
    }

    /// Canonical payload signed by Offline Bearer certificate issuers.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerCertificatePayloadV2 {
        /// Domain separator for the signed payload.
        pub domain: String,
        /// Issuer-scoped certificate identifier.
        pub certificate_id: String,
        /// Chain identifier that scopes the purse.
        pub chain_id: String,
        /// Settlement issuer identifier.
        pub issuer_id: String,
        /// Hardware purse identifier.
        pub purse_id: String,
        /// Canonical account identifier.
        pub account_id: String,
        /// Asset definition held by this purse.
        pub asset_definition_id: String,
        /// Device identifier certified by the issuer.
        pub device_id: String,
        /// Signing key identifier certified by the issuer.
        pub key_id: String,
        /// Hardware class certified by the issuer.
        pub hardware_class: String,
        /// Signature algorithm used by this purse key.
        pub signature_algorithm: String,
        /// Public-key encoding for this purse key.
        pub public_key_encoding: String,
        /// Purse public key bytes.
        pub public_key: Vec<u8>,
        /// Certificate issue time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Certificate expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Policy identifier bound by this certificate.
        pub policy_id: String,
        /// Policy hash bound by this certificate.
        pub policy_hash_hex: String,
    }

    /// Recipient challenge for an Offline Bearer v2 payment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerReceiveRequestV2 {
        /// Payload version.
        pub version: u16,
        /// Chain identifier that scopes the request.
        pub chain_id: String,
        /// Wallet-local receive request identifier.
        pub payment_request_id: String,
        /// Recipient purse certificate.
        pub recipient_certificate: OfflineBearerCertificateV2,
        /// Asset requested.
        pub asset_definition_id: String,
        /// Canonical decimal amount requested.
        pub amount: String,
        /// Request creation time in Unix milliseconds.
        pub created_at_ms: u64,
        /// Request expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Policy hash bound by this request.
        pub policy_hash_hex: String,
        /// Signature algorithm used for `challenge_signature`.
        pub signature_algorithm: String,
        /// Recipient purse signature over the canonical receive-request payload.
        pub challenge_signature: Vec<u8>,
    }

    /// Canonical payload signed by Offline Bearer receive request recipients.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerReceiveRequestPayloadV2 {
        /// Domain separator for the signed payload.
        pub domain: String,
        /// Payload version.
        pub version: u16,
        /// Chain identifier that scopes the request.
        pub chain_id: String,
        /// Wallet-local receive request identifier.
        pub payment_request_id: String,
        /// Hash of the signed recipient certificate.
        pub recipient_certificate_hash: Hash,
        /// Asset requested.
        pub asset_definition_id: String,
        /// Canonical decimal amount requested.
        pub amount: String,
        /// Request creation time in Unix milliseconds.
        pub created_at_ms: u64,
        /// Request expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Policy hash bound by this request.
        pub policy_hash_hex: String,
        /// Signature algorithm used for `challenge_signature`.
        pub signature_algorithm: String,
    }

    /// Sender debit receipt transferred to the recipient.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerDebitReceiptV2 {
        /// Payload version.
        pub version: u16,
        /// Transfer identifier.
        pub transfer_id: String,
        /// Chain identifier that scopes the transfer.
        pub chain_id: String,
        /// Receive request identifier satisfied by this debit.
        pub payment_request_id: String,
        /// Sender purse certificate.
        pub sender_certificate: OfflineBearerCertificateV2,
        /// Recipient purse certificate.
        pub recipient_certificate: OfflineBearerCertificateV2,
        /// Asset transferred.
        pub asset_definition_id: String,
        /// Canonical decimal amount transferred.
        pub amount: String,
        /// Sender balance before debit.
        pub sender_pre_balance: String,
        /// Sender balance after debit.
        pub sender_post_balance: String,
        /// Sender purse sequence after debit.
        pub sender_sequence: u64,
        /// Debit creation time in Unix milliseconds.
        pub created_at_ms: u64,
        /// Debit expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Policy hash bound by this debit.
        pub policy_hash_hex: String,
        /// Recipient receive-request signature copied into the debit.
        pub receive_challenge_signature: Vec<u8>,
        /// Signature algorithm used for `debit_signature`.
        pub signature_algorithm: String,
        /// Sender purse signature over the canonical debit payload.
        pub debit_signature: Vec<u8>,
    }

    /// Canonical payload signed by Offline Bearer debit senders.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerDebitReceiptPayloadV2 {
        /// Domain separator for the signed payload.
        pub domain: String,
        /// Payload version.
        pub version: u16,
        /// Transfer identifier.
        pub transfer_id: String,
        /// Chain identifier that scopes the transfer.
        pub chain_id: String,
        /// Receive request identifier satisfied by this debit.
        pub payment_request_id: String,
        /// Hash of the signed sender certificate.
        pub sender_certificate_hash: Hash,
        /// Hash of the signed recipient certificate.
        pub recipient_certificate_hash: Hash,
        /// Asset transferred.
        pub asset_definition_id: String,
        /// Canonical decimal amount transferred.
        pub amount: String,
        /// Sender balance before debit.
        pub sender_pre_balance: String,
        /// Sender balance after debit.
        pub sender_post_balance: String,
        /// Sender purse sequence after debit.
        pub sender_sequence: u64,
        /// Debit creation time in Unix milliseconds.
        pub created_at_ms: u64,
        /// Debit expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Policy hash bound by this debit.
        pub policy_hash_hex: String,
        /// Recipient receive-request signature copied into the debit.
        pub receive_challenge_signature: Vec<u8>,
        /// Signature algorithm used for `debit_signature`.
        pub signature_algorithm: String,
    }

    /// Recipient credit receipt retained for settlement.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerCreditReceiptV2 {
        /// Payload version.
        pub version: u16,
        /// Transfer identifier.
        pub transfer_id: String,
        /// Chain identifier that scopes the transfer.
        pub chain_id: String,
        /// Recipient purse certificate.
        pub recipient_certificate: OfflineBearerCertificateV2,
        /// Canonical decimal amount credited.
        pub amount: String,
        /// Recipient balance before credit.
        pub recipient_pre_balance: String,
        /// Recipient balance after credit.
        pub recipient_post_balance: String,
        /// Recipient purse sequence after credit.
        pub recipient_sequence: u64,
        /// Credit acceptance time in Unix milliseconds.
        pub accepted_at_ms: u64,
        /// Signature algorithm used for `credit_signature`.
        pub signature_algorithm: String,
        /// Recipient purse signature over the canonical credit payload.
        pub credit_signature: Vec<u8>,
    }

    /// Canonical payload signed by Offline Bearer credit recipients.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerCreditReceiptPayloadV2 {
        /// Domain separator for the signed payload.
        pub domain: String,
        /// Payload version.
        pub version: u16,
        /// Transfer identifier.
        pub transfer_id: String,
        /// Chain identifier that scopes the transfer.
        pub chain_id: String,
        /// Hash of the signed recipient certificate.
        pub recipient_certificate_hash: Hash,
        /// Canonical decimal amount credited.
        pub amount: String,
        /// Recipient balance before credit.
        pub recipient_pre_balance: String,
        /// Recipient balance after credit.
        pub recipient_post_balance: String,
        /// Recipient purse sequence after credit.
        pub recipient_sequence: u64,
        /// Credit acceptance time in Unix milliseconds.
        pub accepted_at_ms: u64,
        /// Signature algorithm used for `credit_signature`.
        pub signature_algorithm: String,
    }

    /// Compact settlement batch exported from a local Offline Bearer v2 purse journal.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerSettlementBatchV2 {
        /// Payload version.
        pub version: u16,
        /// Chain identifier that scopes the settlement.
        pub chain_id: String,
        /// Exporting purse identifier.
        pub purse_id: String,
        /// Accepted debit receipts.
        pub debit_receipts: Vec<OfflineBearerDebitReceiptV2>,
        /// Accepted credit receipts.
        pub credit_receipts: Vec<OfflineBearerCreditReceiptV2>,
    }

    /// Canonical payload representing a settlement batch digest set.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineBearerSettlementBatchPayloadV2 {
        /// Domain separator for the payload.
        pub domain: String,
        /// Payload version.
        pub version: u16,
        /// Chain identifier that scopes the settlement.
        pub chain_id: String,
        /// Exporting purse identifier.
        pub purse_id: String,
        /// Hashes of signed debit receipts.
        pub debit_receipt_hashes: Vec<Hash>,
        /// Hashes of signed credit receipts.
        pub credit_receipt_hashes: Vec<Hash>,
    }
}

/// Origin of a wallet-derived Offline Note note commitment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OfflineNoteCommitmentOrigin {
    /// Note created by an issuer load operation.
    IssuerLoad(OfflineNoteIssuerLoadOrigin),
    /// Note created as an output of an offline peer-to-peer payment token.
    P2pOutput(OfflineNoteP2pOutputOrigin),
}

const OFFLINE_NOTE_KEY_CERTIFICATE_PAYLOAD_DOMAIN: &str =
    "iroha:offline-note:key-certificate-payload";
const OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN: &str = "iroha:offline-note:issued-claim";
const OFFLINE_NOTE_REDEEM_PUBLIC_INPUTS_DOMAIN: &str = "iroha:offline-note:redeem-public-inputs";
const OFFLINE_NOTE_AUDIT_PUBLIC_INPUTS_DOMAIN: &str = "iroha:offline-note:audit-public-inputs";
const OFFLINE_BEARER_POLICY_BUNDLE_PAYLOAD_DOMAIN: &str = "iroha:offline-bearer-v2:policy-bundle";
const OFFLINE_BEARER_CERTIFICATE_PAYLOAD_DOMAIN: &str = "iroha:offline-bearer-v2:certificate";
const OFFLINE_BEARER_RECEIVE_REQUEST_PAYLOAD_DOMAIN: &str =
    "iroha:offline-bearer-v2:receive-request";
const OFFLINE_BEARER_DEBIT_RECEIPT_PAYLOAD_DOMAIN: &str = "iroha:offline-bearer-v2:debit-receipt";
const OFFLINE_BEARER_CREDIT_RECEIPT_PAYLOAD_DOMAIN: &str = "iroha:offline-bearer-v2:credit-receipt";
const OFFLINE_BEARER_SETTLEMENT_BATCH_PAYLOAD_DOMAIN: &str =
    "iroha:offline-bearer-v2:settlement-batch";
/// Canonical public-input schema descriptor for Offline recursive note proofs.
pub const OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"offline_note_recursive","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","proof_mode","input_count","output_count","input_amount_sum","output_amount_sum","input_nullifier_sum_limb0","output_commitment_sum_limb0","key_certificate_payload_hash_limb0","source_or_token_limb0","input_claim_hash_sum_limb0","output_claim_hash_sum_limb0","reserved_zero"]}"#;

/// Return the registry schema hash required for Offline recursive note verifiers.
#[must_use]
pub fn offline_note_recursive_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    Hash::new(OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA).into()
}

impl From<&OfflineNoteKeyCertificate> for OfflineNoteKeyCertificatePayload {
    fn from(certificate: &OfflineNoteKeyCertificate) -> Self {
        Self {
            domain: OFFLINE_NOTE_KEY_CERTIFICATE_PAYLOAD_DOMAIN.to_owned(),
            version: certificate.version,
            platform: certificate.platform.clone(),
            key_id: certificate.key_id.clone(),
            device_id: certificate.device_id.clone(),
            account_id: certificate.account_id.clone(),
            public_key: certificate.public_key.clone(),
            assertion_scheme: certificate.assertion_scheme.clone(),
            assertion_key_algorithm: certificate.assertion_key_algorithm.clone(),
            assertion_public_key: certificate.assertion_public_key.clone(),
            assertion_usage_count_limit: certificate.assertion_usage_count_limit,
            one_use: certificate.one_use,
        }
    }
}

impl OfflineNoteKeyCertificate {
    /// Canonical payload bytes signed by the Offline certificate issuer.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        let payload = OfflineNoteKeyCertificatePayload::from(self);
        to_bytes(&payload)
    }

    /// Deterministic hash of the canonical certificate payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn payload_hash(&self) -> Result<Hash, norito::Error> {
        self.signing_bytes().map(Hash::new)
    }
}

impl OfflineNoteIssuedClaim {
    /// Build the claim recorded when an Offline note is issued.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_issue(issue: &OfflineNoteIssue) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: issue.note_commitment,
            key_certificate_payload_hash: issue.key_certificate.payload_hash()?,
            asset: issue.asset.clone(),
            amount: issue.amount.clone(),
        })
    }

    /// Build the claim expected when an Offline note is redeemed.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_redemption(redemption: &OfflineNoteRedeem) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: redemption.source_note_commitment,
            key_certificate_payload_hash: redemption.sender_key_certificate.payload_hash()?,
            asset: redemption.asset.clone(),
            amount: redemption.amount.clone(),
        })
    }

    /// Build the claim recorded when an Offline audited output is accepted.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_audit_output(output: &OfflineNoteAuditOutputClaim) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: output.note_commitment,
            key_certificate_payload_hash: output.key_certificate.payload_hash()?,
            asset: output.asset.clone(),
            amount: output.amount.clone(),
        })
    }

    /// Deterministic hash of the issued-note claim.
    ///
    /// # Errors
    ///
    /// Returns an error when the claim cannot be serialized with Norito.
    pub fn claim_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteRedeemPublicInputs {
    /// Build the public inputs committed by an Offline redemption proof.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_redemption(redemption: &OfflineNoteRedeem) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_REDEEM_PUBLIC_INPUTS_DOMAIN.to_owned(),
            source_note_commitment: redemption.source_note_commitment,
            input_nullifiers: redemption.input_nullifiers.clone(),
            key_certificate_payload_hash: redemption.sender_key_certificate.payload_hash()?,
            recipient: redemption.recipient.clone(),
            asset: redemption.asset.clone(),
            amount: redemption.amount.clone(),
        })
    }

    /// Deterministic hash of the redemption public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public inputs cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteAuditPublicInputs {
    /// Build the public inputs committed by an Offline optional audit proof.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_audit(audit: &OfflineNoteAuditBundle) -> Result<Self, norito::Error> {
        let output_claims = audit
            .output_claims
            .iter()
            .map(OfflineNoteIssuedClaim::from_audit_output)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            domain: OFFLINE_NOTE_AUDIT_PUBLIC_INPUTS_DOMAIN.to_owned(),
            token_id: audit.token_id,
            key_certificate_payload_hash: audit.sender_key_certificate.payload_hash()?,
            input_nullifiers: audit.input_nullifiers.clone(),
            input_claims: audit.input_claims.clone(),
            output_commitments: audit.output_commitments.clone(),
            output_claims,
        })
    }

    /// Deterministic hash of the audit public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public inputs cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteAuditBundle {
    /// Deterministic hash that the optional audit proof must expose as public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        OfflineNoteAuditPublicInputs::from_audit(self)?.public_inputs_hash()
    }
}

impl OfflineNoteRedeem {
    /// Deterministic hash that the recursive proof must expose as public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        OfflineNoteRedeemPublicInputs::from_redemption(self)?.public_inputs_hash()
    }
}

impl From<&OfflineBearerPolicyBundleV2> for OfflineBearerPolicyBundlePayloadV2 {
    fn from(policy: &OfflineBearerPolicyBundleV2) -> Self {
        Self {
            domain: OFFLINE_BEARER_POLICY_BUNDLE_PAYLOAD_DOMAIN.to_owned(),
            policy_id: policy.policy_id.clone(),
            policy_hash_hex: policy.policy_hash_hex.clone(),
            issuer_id: policy.issuer_id.clone(),
            issued_at_ms: policy.issued_at_ms,
            expires_at_ms: policy.expires_at_ms,
            max_certificate_age_ms: policy.max_certificate_age_ms,
            max_policy_age_ms: policy.max_policy_age_ms,
            max_token_age_ms: policy.max_token_age_ms,
            max_offline_balance: policy.max_offline_balance.clone(),
            max_transaction_amount: policy.max_transaction_amount.clone(),
            allowed_hardware_classes: sorted_strings(&policy.allowed_hardware_classes),
            blacklisted_account_ids: sorted_strings(&policy.blacklisted_account_ids),
            blacklisted_device_ids: sorted_strings(&policy.blacklisted_device_ids),
            blacklisted_key_ids: sorted_strings(&policy.blacklisted_key_ids),
            signature_algorithm: policy.signature_algorithm.clone(),
            policy_epoch: policy.policy_epoch,
            policy_source: policy.policy_source.clone(),
            revoked_certificate_ids: sorted_strings(&policy.revoked_certificate_ids),
            revoked_transfer_ids: sorted_strings(&policy.revoked_transfer_ids),
            asset_send_limits: sorted_asset_send_limits(&policy.asset_send_limits),
        }
    }
}

impl OfflineBearerPolicyBundleV2 {
    /// Canonical payload bytes signed by the Offline Bearer policy issuer.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        to_bytes(&OfflineBearerPolicyBundlePayloadV2::from(self))
    }
}

impl From<&OfflineBearerCertificateV2> for OfflineBearerCertificatePayloadV2 {
    fn from(certificate: &OfflineBearerCertificateV2) -> Self {
        Self {
            domain: OFFLINE_BEARER_CERTIFICATE_PAYLOAD_DOMAIN.to_owned(),
            certificate_id: certificate.certificate_id.clone(),
            chain_id: certificate.chain_id.clone(),
            issuer_id: certificate.issuer_id.clone(),
            purse_id: certificate.purse_id.clone(),
            account_id: certificate.account_id.clone(),
            asset_definition_id: certificate.asset_definition_id.clone(),
            device_id: certificate.device_id.clone(),
            key_id: certificate.key_id.clone(),
            hardware_class: certificate.hardware_class.clone(),
            signature_algorithm: certificate.signature_algorithm.clone(),
            public_key_encoding: certificate.public_key_encoding.clone(),
            public_key: certificate.public_key.clone(),
            issued_at_ms: certificate.issued_at_ms,
            expires_at_ms: certificate.expires_at_ms,
            policy_id: certificate.policy_id.clone(),
            policy_hash_hex: certificate.policy_hash_hex.clone(),
        }
    }
}

impl OfflineBearerCertificateV2 {
    /// Canonical payload bytes signed by the Offline Bearer certificate issuer.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        to_bytes(&OfflineBearerCertificatePayloadV2::from(self))
    }

    /// Deterministic hash of the signed certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate cannot be serialized with Norito.
    pub fn certificate_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineBearerReceiveRequestV2 {
    /// Canonical payload bytes signed by the recipient purse.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        to_bytes(&OfflineBearerReceiveRequestPayloadV2 {
            domain: OFFLINE_BEARER_RECEIVE_REQUEST_PAYLOAD_DOMAIN.to_owned(),
            version: self.version,
            chain_id: self.chain_id.clone(),
            payment_request_id: self.payment_request_id.clone(),
            recipient_certificate_hash: self.recipient_certificate.certificate_hash()?,
            asset_definition_id: self.asset_definition_id.clone(),
            amount: self.amount.clone(),
            created_at_ms: self.created_at_ms,
            expires_at_ms: self.expires_at_ms,
            policy_hash_hex: self.policy_hash_hex.clone(),
            signature_algorithm: self.signature_algorithm.clone(),
        })
    }
}

impl OfflineBearerDebitReceiptV2 {
    /// Canonical payload bytes signed by the sender purse.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        to_bytes(&OfflineBearerDebitReceiptPayloadV2 {
            domain: OFFLINE_BEARER_DEBIT_RECEIPT_PAYLOAD_DOMAIN.to_owned(),
            version: self.version,
            transfer_id: self.transfer_id.clone(),
            chain_id: self.chain_id.clone(),
            payment_request_id: self.payment_request_id.clone(),
            sender_certificate_hash: self.sender_certificate.certificate_hash()?,
            recipient_certificate_hash: self.recipient_certificate.certificate_hash()?,
            asset_definition_id: self.asset_definition_id.clone(),
            amount: self.amount.clone(),
            sender_pre_balance: self.sender_pre_balance.clone(),
            sender_post_balance: self.sender_post_balance.clone(),
            sender_sequence: self.sender_sequence,
            created_at_ms: self.created_at_ms,
            expires_at_ms: self.expires_at_ms,
            policy_hash_hex: self.policy_hash_hex.clone(),
            receive_challenge_signature: self.receive_challenge_signature.clone(),
            signature_algorithm: self.signature_algorithm.clone(),
        })
    }

    /// Deterministic hash of the signed debit receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when the receipt cannot be serialized with Norito.
    pub fn receipt_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineBearerCreditReceiptV2 {
    /// Canonical payload bytes signed by the recipient purse.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        to_bytes(&OfflineBearerCreditReceiptPayloadV2 {
            domain: OFFLINE_BEARER_CREDIT_RECEIPT_PAYLOAD_DOMAIN.to_owned(),
            version: self.version,
            transfer_id: self.transfer_id.clone(),
            chain_id: self.chain_id.clone(),
            recipient_certificate_hash: self.recipient_certificate.certificate_hash()?,
            amount: self.amount.clone(),
            recipient_pre_balance: self.recipient_pre_balance.clone(),
            recipient_post_balance: self.recipient_post_balance.clone(),
            recipient_sequence: self.recipient_sequence,
            accepted_at_ms: self.accepted_at_ms,
            signature_algorithm: self.signature_algorithm.clone(),
        })
    }

    /// Deterministic hash of the signed credit receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when the receipt cannot be serialized with Norito.
    pub fn receipt_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineBearerSettlementBatchV2 {
    /// Canonical settlement batch digest payload bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when a receipt or payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        let debit_receipt_hashes = self
            .debit_receipts
            .iter()
            .map(OfflineBearerDebitReceiptV2::receipt_hash)
            .collect::<Result<Vec<_>, _>>()?;
        let credit_receipt_hashes = self
            .credit_receipts
            .iter()
            .map(OfflineBearerCreditReceiptV2::receipt_hash)
            .collect::<Result<Vec<_>, _>>()?;
        to_bytes(&OfflineBearerSettlementBatchPayloadV2 {
            domain: OFFLINE_BEARER_SETTLEMENT_BATCH_PAYLOAD_DOMAIN.to_owned(),
            version: self.version,
            chain_id: self.chain_id.clone(),
            purse_id: self.purse_id.clone(),
            debit_receipt_hashes,
            credit_receipt_hashes,
        })
    }
}

fn sorted_strings(values: &[String]) -> Vec<String> {
    let mut normalized = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn sorted_asset_send_limits(
    limits: &[OfflineBearerAssetSendLimitV2],
) -> Vec<OfflineBearerAssetSendLimitV2> {
    let mut sorted = limits.to_vec();
    sorted.sort_by(|lhs, rhs| lhs.asset_definition_id.cmp(&rhs.asset_definition_id));
    sorted
}

fn validate_offline_note_random_bytes(
    field: &'static str,
    bytes: &[u8],
) -> Result<(), OfflineNoteDerivationError> {
    if bytes.len() != Hash::LENGTH {
        return Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
            field,
            expected: Hash::LENGTH,
            actual: bytes.len(),
        });
    }
    Ok(())
}

/// Derive the canonical Offline Note note commitment from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `note_secret` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_note_commitment(
    preimage: &OfflineNoteCommitmentPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("note_secret", &preimage.note_secret)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

/// Derive the canonical Offline Note input nullifier from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `note_secret` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_input_nullifier(
    preimage: &OfflineNoteInputNullifierPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("note_secret", &preimage.note_secret)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

/// Derive the canonical Offline Note payment token id from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `token_nonce` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_payment_token_id(
    preimage: &OfflineNotePaymentTokenIdPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("token_nonce", &preimage.token_nonce)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

#[cfg(test)]
mod offline_note_tests {
    use iroha_crypto::{Algorithm, KeyPair, PublicKey};

    use super::*;
    use crate::{asset::AssetDefinitionId, domain::DomainId};

    fn sample_signature(seed: u8) -> Signature {
        let mut payload = [0u8; 64];
        for (idx, byte) in payload.iter_mut().enumerate() {
            let offset = u8::try_from(idx).expect("index fits into u8");
            *byte = seed.wrapping_add(offset);
        }
        Signature::from_bytes(&payload)
    }

    fn sample_public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        key_pair.public_key().clone()
    }

    fn sample_account(seed: u8, domain: &str) -> AccountId {
        let key = sample_public_key(seed);
        let _domain_id = DomainId::try_new(domain, "universal").expect("domain id");
        AccountId::new(key)
    }

    #[test]
    fn offline_escrow_account_derivation_binds_chain_and_asset_definition() {
        let chain_id: ChainId = "offline-escrow-testnet".parse().expect("chain id");
        let other_chain_id: ChainId = "offline-escrow-mainnet".parse().expect("chain id");
        let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
        let definition_id = AssetDefinitionId::new(
            domain_id.clone(),
            "usd".parse().expect("asset definition name"),
        );
        let other_definition_id =
            AssetDefinitionId::new(domain_id, "eur".parse().expect("asset definition name"));

        let escrow = offline_escrow_account_id(&chain_id, &definition_id);

        assert_eq!(escrow, offline_escrow_account_id(&chain_id, &definition_id));
        assert_ne!(
            escrow,
            offline_escrow_account_id(&other_chain_id, &definition_id)
        );
        assert_ne!(
            escrow,
            offline_escrow_account_id(&chain_id, &other_definition_id)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn offline_note_claims_and_public_inputs_bind_payload_fields() {
        let account_id = sample_account(0xD4, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id.clone());
        let note_public_key = sample_public_key(0xA8);
        let (_algorithm, note_key) = note_public_key.to_bytes();
        let certificate = OfflineNoteKeyCertificate {
            version: OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
            platform: "ios-appattest".to_owned(),
            key_id: "one-use-key".to_owned(),
            device_id: "device-1".to_owned(),
            account_id: account_id.clone(),
            public_key: note_key.to_vec(),
            assertion_scheme: "apple-appattest-counter".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: sample_signature(0xAB),
        };
        let proof = OfflineNoteRecursiveProof {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-recursive"),
            public_inputs_hash: Hash::new(b"offline-public-inputs"),
            proof: ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
        };
        let note_commitment = Hash::new(b"offline-note-issued-note");
        let issue = OfflineNoteIssue {
            note_commitment,
            key_certificate: certificate.clone(),
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
        };
        let mut redemption = OfflineNoteRedeem {
            source_note_commitment: note_commitment,
            input_nullifiers: vec![Hash::new(b"offline-note-nullifier")],
            sender_key_certificate: certificate.clone(),
            recipient: account_id,
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
            recursive_proof: proof.clone(),
        };

        let issue_claim = OfflineNoteIssuedClaim::from_issue(&issue)
            .expect("issue claim")
            .claim_hash()
            .expect("issue claim hash");
        let redeem_claim = OfflineNoteIssuedClaim::from_redemption(&redemption)
            .expect("redemption claim")
            .claim_hash()
            .expect("redemption claim hash");
        assert_eq!(issue_claim, redeem_claim);
        let redemption_inputs = redemption
            .public_inputs_hash()
            .expect("redemption public inputs hash");
        redemption.source_note_commitment = Hash::new(b"offline-note-other-note");
        assert_ne!(
            redemption_inputs,
            redemption
                .public_inputs_hash()
                .expect("changed redemption public inputs hash")
        );
        assert_ne!(
            issue_claim,
            OfflineNoteIssuedClaim::from_redemption(&redemption)
                .expect("changed redemption claim")
                .claim_hash()
                .expect("changed redemption claim hash")
        );

        let mut audit = OfflineNoteAuditBundle {
            token_id: Hash::new(b"offline-note-audit-token"),
            sender_key_certificate: certificate.clone(),
            input_nullifiers: vec![Hash::new(b"offline-note-audit-nullifier")],
            input_claims: vec![
                OfflineNoteIssuedClaim::from_issue(&issue).expect("audit input claim"),
            ],
            output_commitments: vec![Hash::new(b"offline-note-output-note")],
            output_claims: vec![OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-output-note"),
                key_certificate: certificate,
                asset,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: proof,
        };
        let audit_inputs = audit
            .public_inputs_hash()
            .expect("audit public inputs hash");
        audit.output_commitments = vec![Hash::new(b"offline-note-other-output")];
        assert_ne!(
            audit_inputs,
            audit
                .public_inputs_hash()
                .expect("changed audit public inputs hash")
        );
        audit.output_commitments = vec![Hash::new(b"offline-note-output-note")];
        audit.input_claims[0].amount = Numeric::new(9, 0);
        assert_ne!(
            audit_inputs,
            audit
                .public_inputs_hash()
                .expect("changed audit input claim public inputs hash")
        );
    }

    #[test]
    fn offline_note_wallet_derivations_bind_preimages() {
        let chain_id: ChainId = "offline-note-derivation-chain".parse().expect("chain id");
        let account_id = sample_account(0xD5, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id);
        let owner_key_certificate_payload_hash = Hash::new(b"offline-note-owner-cert");
        let note_secret = vec![0xA5; Hash::LENGTH];
        let commitment_preimage = OfflineNoteCommitmentPreimage {
            domain: OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            owner_key_certificate_payload_hash,
            asset: asset.clone(),
            amount: Numeric::new(42, 0),
            note_secret: note_secret.clone(),
            origin: OfflineNoteCommitmentOrigin::IssuerLoad(OfflineNoteIssuerLoadOrigin {
                operation_id: "operation-1".to_owned(),
                lineage_id: "lineage-1".to_owned(),
                local_revision: 7,
            }),
        };
        let commitment =
            derive_offline_note_note_commitment(&commitment_preimage).expect("commitment");

        assert_eq!(
            commitment,
            derive_offline_note_note_commitment(&commitment_preimage).expect("repeat commitment")
        );
        let mut changed_commitment_preimage = commitment_preimage.clone();
        changed_commitment_preimage.origin =
            OfflineNoteCommitmentOrigin::P2pOutput(OfflineNoteP2pOutputOrigin {
                payment_request_id: "payment-request-1".to_owned(),
                output_index: 0,
            });
        assert_ne!(
            commitment,
            derive_offline_note_note_commitment(&changed_commitment_preimage)
                .expect("changed origin commitment")
        );

        let nullifier_preimage = OfflineNoteInputNullifierPreimage {
            domain: OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            source_note_commitment: commitment,
            owner_key_certificate_payload_hash,
            note_secret: note_secret.clone(),
        };
        let nullifier =
            derive_offline_note_input_nullifier(&nullifier_preimage).expect("nullifier");
        assert_eq!(
            nullifier,
            derive_offline_note_input_nullifier(&nullifier_preimage).expect("repeat nullifier")
        );
        let mut changed_nullifier_preimage = nullifier_preimage.clone();
        changed_nullifier_preimage.note_secret[0] ^= 0x01;
        assert_ne!(
            nullifier,
            derive_offline_note_input_nullifier(&changed_nullifier_preimage)
                .expect("changed secret nullifier")
        );

        let token_preimage = OfflineNotePaymentTokenIdPreimage {
            domain: OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN.to_owned(),
            chain_id,
            payment_request_id: "payment-request-fixture".to_owned(),
            created_at_ms: 1_700_000_001_000,
            token_nonce: vec![0xC6; Hash::LENGTH],
            sender_key_certificate_payload_hash: owner_key_certificate_payload_hash,
            input_nullifiers: vec![nullifier],
            output_commitments: vec![commitment],
        };
        let token_id =
            derive_offline_note_payment_token_id(&token_preimage).expect("payment token id");
        assert_eq!(
            token_id,
            derive_offline_note_payment_token_id(&token_preimage).expect("repeat payment token id")
        );
        let mut changed_token_preimage = token_preimage.clone();
        changed_token_preimage.token_nonce[0] ^= 0x01;
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_token_preimage)
                .expect("changed nonce payment token id")
        );
        let mut changed_request_token_preimage = token_preimage.clone();
        changed_request_token_preimage.payment_request_id = "payment-request-other".to_owned();
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_request_token_preimage)
                .expect("changed request payment token id")
        );
        let mut changed_created_at_token_preimage = token_preimage.clone();
        changed_created_at_token_preimage.created_at_ms += 1;
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_created_at_token_preimage)
                .expect("changed created_at payment token id")
        );
    }

    #[test]
    fn offline_note_wallet_derivations_reject_short_random_material() {
        let chain_id: ChainId = "offline-note-derivation-chain".parse().expect("chain id");
        let account_id = sample_account(0xD6, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id);
        let owner_key_certificate_payload_hash = Hash::new(b"offline-note-owner-cert");
        let commitment_preimage = OfflineNoteCommitmentPreimage {
            domain: OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            owner_key_certificate_payload_hash,
            asset,
            amount: Numeric::new(42, 0),
            note_secret: vec![0xA5; Hash::LENGTH - 1],
            origin: OfflineNoteCommitmentOrigin::IssuerLoad(OfflineNoteIssuerLoadOrigin {
                operation_id: "operation-1".to_owned(),
                lineage_id: "lineage-1".to_owned(),
                local_revision: 7,
            }),
        };
        assert!(matches!(
            derive_offline_note_note_commitment(&commitment_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "note_secret",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));

        let nullifier_preimage = OfflineNoteInputNullifierPreimage {
            domain: OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            source_note_commitment: Hash::new(b"source-note"),
            owner_key_certificate_payload_hash,
            note_secret: vec![0xB6; Hash::LENGTH - 1],
        };
        assert!(matches!(
            derive_offline_note_input_nullifier(&nullifier_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "note_secret",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));

        let token_preimage = OfflineNotePaymentTokenIdPreimage {
            domain: OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN.to_owned(),
            chain_id,
            payment_request_id: "payment-request-fixture".to_owned(),
            created_at_ms: 1_700_000_001_000,
            token_nonce: vec![0xC7; Hash::LENGTH - 1],
            sender_key_certificate_payload_hash: owner_key_certificate_payload_hash,
            input_nullifiers: vec![Hash::new(b"nullifier")],
            output_commitments: vec![Hash::new(b"commitment")],
        };
        assert!(matches!(
            derive_offline_note_payment_token_id(&token_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "token_nonce",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));
    }
}
