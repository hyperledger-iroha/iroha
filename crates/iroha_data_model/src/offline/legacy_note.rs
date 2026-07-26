//! BOI legacy offline-note compatibility models.
//!
//! These models preserve the pre-Kagemusha note issuance, optional audit, and
//! online redemption wire contract. They do not advertise or select a peer
//! payment mode; ABI-21/V4 `cash_handoff_v1` remains the sole peer-cash path.

use iroha_crypto::{Hash, Signature};
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};

use crate::{
    account::AccountId,
    asset::AssetId,
    proof::{ProofBox, VerifyingKeyId},
};

/// Canonical legacy key-certificate format marker.
pub const OFFLINE_NOTE_KEY_CERTIFICATE_VERSION: u16 = 1;
/// Domain-separation tag for wallet-derived legacy note commitments.
pub const OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN: &str = "iroha:offline-note:note-commitment";
/// Domain-separation tag for wallet-derived legacy input nullifiers.
pub const OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN: &str = "iroha:offline-note:input-nullifier";
/// Domain-separation tag for wallet-derived legacy payment-token identifiers.
pub const OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN: &str = "iroha:offline-note:payment-token-id";

const OFFLINE_NOTE_KEY_CERTIFICATE_PAYLOAD_DOMAIN: &str =
    "iroha:offline-note:key-certificate-payload";
const OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN: &str = "iroha:offline-note:issued-claim";
const OFFLINE_NOTE_REDEEM_PUBLIC_INPUTS_DOMAIN: &str = "iroha:offline-note:redeem-public-inputs";
const OFFLINE_NOTE_AUDIT_PUBLIC_INPUTS_DOMAIN: &str = "iroha:offline-note:audit-public-inputs";

/// Canonical public-input schema for the legacy recursive note circuit.
pub const OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"offline_note_recursive","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","proof_mode","input_count","output_count","input_amount_sum","output_amount_sum","input_nullifier_sum_limb0","output_commitment_sum_limb0","key_certificate_payload_hash_limb0","source_or_token_limb0","input_claim_hash_sum_limb0","output_claim_hash_sum_limb0","reserved_zero"]}"#;

/// Compact CA-issued certificate for a one-use legacy note key.
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
    /// Hardware assertion key algorithm.
    pub assertion_key_algorithm: String,
    /// Hardware assertion public key bytes.
    pub assertion_public_key: Vec<u8>,
    /// Hardware one-use limit when the platform exposes it.
    pub assertion_usage_count_limit: Option<u32>,
    /// True when the issuer verified hardware one-use semantics.
    pub one_use: bool,
    /// Offline CA signature over the compact certificate payload.
    pub issuer_signature: Signature,
}

/// Canonical payload signed by legacy key-certificate issuers.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteKeyCertificatePayload {
    /// Domain separator for the signed payload.
    pub domain: String,
    /// Certificate format marker.
    pub version: u16,
    /// Platform class.
    pub platform: String,
    /// Issuer-scoped one-use key identifier.
    pub key_id: String,
    /// Device identifier.
    pub device_id: String,
    /// Account authorized to control the note key.
    pub account_id: AccountId,
    /// Ed25519 note-key bytes.
    pub public_key: Vec<u8>,
    /// Hardware assertion scheme.
    pub assertion_scheme: String,
    /// Hardware assertion algorithm.
    pub assertion_key_algorithm: String,
    /// Hardware assertion public key.
    pub assertion_public_key: Vec<u8>,
    /// Hardware one-use limit.
    pub assertion_usage_count_limit: Option<u32>,
    /// Hardware one-use assurance.
    pub one_use: bool,
}

/// Verifier-key-backed proof carried by a legacy note token.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteRecursiveProof {
    /// Stable verifier key identifier stored in WSV.
    pub verifier_key_id: VerifyingKeyId,
    /// Public input commitment hash.
    pub public_inputs_hash: Hash,
    /// Halo2 IPA proof envelope.
    pub proof: ProofBox,
}

/// Issuer-side note issuance record for online load or consolidation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteIssue {
    /// Deterministic note commitment.
    pub note_commitment: Hash,
    /// Owner key certificate.
    pub key_certificate: OfflineNoteKeyCertificate,
    /// Asset held by the note.
    pub asset: AssetId,
    /// Note amount.
    pub amount: Numeric,
}

/// Ledger-recognized note claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteIssuedClaim {
    /// Domain separator.
    pub domain: String,
    /// Note commitment.
    pub note_commitment: Hash,
    /// Certificate payload hash.
    pub key_certificate_payload_hash: Hash,
    /// Asset held by the note.
    pub asset: AssetId,
    /// Amount reserved into offline escrow.
    pub amount: Numeric,
}

/// Redeemable note output observed during an optional legacy audit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteAuditOutputClaim {
    /// Output note commitment.
    pub note_commitment: Hash,
    /// Output owner certificate.
    pub key_certificate: OfflineNoteKeyCertificate,
    /// Asset held by the output.
    pub asset: AssetId,
    /// Output amount.
    pub amount: Numeric,
}

/// Online redemption payload for a ledger-recognized legacy bearer note.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteRedeem {
    /// Ledger-recognized source commitment.
    pub source_note_commitment: Hash,
    /// Nullifiers consumed by the token.
    pub input_nullifiers: Vec<Hash>,
    /// Sender one-use note-key certificate.
    pub sender_key_certificate: OfflineNoteKeyCertificate,
    /// Recipient credited online.
    pub recipient: AccountId,
    /// Asset being redeemed.
    pub asset: AssetId,
    /// Redeemed amount.
    pub amount: Numeric,
    /// Legacy recursive proof.
    pub recursive_proof: OfflineNoteRecursiveProof,
}

/// Public inputs bound by a legacy redemption proof.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteRedeemPublicInputs {
    /// Domain separator.
    pub domain: String,
    /// Source note commitment.
    pub source_note_commitment: Hash,
    /// Consumed nullifiers.
    pub input_nullifiers: Vec<Hash>,
    /// Note-key certificate payload hash.
    pub key_certificate_payload_hash: Hash,
    /// Online recipient.
    pub recipient: AccountId,
    /// Redeemed asset.
    pub asset: AssetId,
    /// Redeemed amount.
    pub amount: Numeric,
}

/// Optional audit bundle that anchors a legacy P2P bearer lineage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteAuditBundle {
    /// Payment-token identifier.
    pub token_id: Hash,
    /// Sender one-use note-key certificate.
    pub sender_key_certificate: OfflineNoteKeyCertificate,
    /// Input nullifiers observed in the token.
    pub input_nullifiers: Vec<Hash>,
    /// Ledger-recognized input claims consumed by the token.
    pub input_claims: Vec<OfflineNoteIssuedClaim>,
    /// Output commitments created by the token.
    pub output_commitments: Vec<Hash>,
    /// Redeemable output claims created by the token.
    pub output_claims: Vec<OfflineNoteAuditOutputClaim>,
    /// Legacy recursive audit proof.
    pub recursive_proof: OfflineNoteRecursiveProof,
}

/// Public inputs bound by an optional legacy audit proof.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineNoteAuditPublicInputs {
    /// Domain separator.
    pub domain: String,
    /// Payment-token identifier.
    pub token_id: Hash,
    /// Note-key certificate payload hash.
    pub key_certificate_payload_hash: Hash,
    /// Input nullifiers.
    pub input_nullifiers: Vec<Hash>,
    /// Input claims.
    pub input_claims: Vec<OfflineNoteIssuedClaim>,
    /// Output commitments.
    pub output_commitments: Vec<Hash>,
    /// Output claims in ledger-recognized form.
    pub output_claims: Vec<OfflineNoteIssuedClaim>,
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
    /// Canonical payload bytes signed by the certificate issuer.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        to_bytes(&OfflineNoteKeyCertificatePayload::from(self))
    }

    /// Deterministic certificate-payload hash.
    pub fn payload_hash(&self) -> Result<Hash, norito::Error> {
        self.signing_bytes().map(Hash::new)
    }
}

impl OfflineNoteIssuedClaim {
    /// Build the claim recorded when a note is issued.
    pub fn from_issue(issue: &OfflineNoteIssue) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: issue.note_commitment,
            key_certificate_payload_hash: issue.key_certificate.payload_hash()?,
            asset: issue.asset.clone(),
            amount: issue.amount.clone(),
        })
    }

    /// Build the claim expected by a redemption.
    pub fn from_redemption(redemption: &OfflineNoteRedeem) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: redemption.source_note_commitment,
            key_certificate_payload_hash: redemption.sender_key_certificate.payload_hash()?,
            asset: redemption.asset.clone(),
            amount: redemption.amount.clone(),
        })
    }

    /// Build the claim recorded for an accepted audit output.
    pub fn from_audit_output(output: &OfflineNoteAuditOutputClaim) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: output.note_commitment,
            key_certificate_payload_hash: output.key_certificate.payload_hash()?,
            asset: output.asset.clone(),
            amount: output.amount.clone(),
        })
    }

    /// Deterministic claim hash.
    pub fn claim_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteRedeemPublicInputs {
    /// Build the public inputs committed by a redemption proof.
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

    /// Deterministic public-input hash.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteAuditPublicInputs {
    /// Build the public inputs committed by an audit proof.
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

    /// Deterministic public-input hash.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteAuditBundle {
    /// Deterministic hash exposed by the audit proof.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        OfflineNoteAuditPublicInputs::from_audit(self)?.public_inputs_hash()
    }
}

impl OfflineNoteRedeem {
    /// Deterministic hash exposed by the redemption proof.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        OfflineNoteRedeemPublicInputs::from_redemption(self)?.public_inputs_hash()
    }
}

/// Registry schema hash required by the legacy recursive note verifier.
#[must_use]
pub fn offline_note_recursive_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    Hash::new(OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursive_schema_hash_is_stable_and_nonzero() {
        assert_ne!(
            offline_note_recursive_public_inputs_schema_hash(),
            [0; Hash::LENGTH]
        );
        assert_eq!(
            offline_note_recursive_public_inputs_schema_hash(),
            <[u8; Hash::LENGTH]>::from(Hash::new(OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA))
        );
    }
}
