//! Generic hidden-program RAM-LFE policy and receipt types.

use std::{fmt, str::FromStr, string::String, vec::Vec};

use iroha_crypto::{
    Hash, PolicyCommitment, PublicKey, RamLfeBackend, RamLfeVerificationMode, Signature,
    SignatureOf,
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, name::Name, proof::ProofBox};

/// Error returned while parsing [`RamLfeProgramId`] literals.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum RamLfeProgramIdParseError {
    /// The supplied literal is not a valid [`Name`].
    #[error("{0}")]
    InvalidName(String),
}

/// Stable on-chain identifier for a hidden RAM-LFE program policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RamLfeProgramId {
    /// Canonical program name.
    pub name: Name,
}

impl RamLfeProgramId {
    /// Construct a new program identifier.
    #[must_use]
    pub const fn new(name: Name) -> Self {
        Self { name }
    }
}

impl fmt::Display for RamLfeProgramId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}

impl FromStr for RamLfeProgramId {
    type Err = RamLfeProgramIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Name::from_str(s.trim())
            .map(Self::new)
            .map_err(|err| RamLfeProgramIdParseError::InvalidName(err.to_string()))
    }
}

/// Public metadata for a globally registered hidden RAM-LFE program.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RamLfeProgramPolicy {
    /// Stable on-chain program identifier.
    pub program_id: RamLfeProgramId,
    /// Account that registered and controls the policy.
    pub owner: AccountId,
    /// Evaluator backend exposed to wallets and clients.
    pub backend: RamLfeBackend,
    /// Receipt attestation mode enforced for this program.
    pub verification_mode: RamLfeVerificationMode,
    /// Commitment to the hidden program metadata and resolver secret.
    pub commitment: PolicyCommitment,
    /// Public key used to verify signed receipts.
    pub resolver_public_key: PublicKey,
    /// Public key used to verify externally opened RAM-LFE output commitments.
    pub output_opening_public_key: PublicKey,
    /// Whether the program policy is active for new execution receipts.
    pub active: bool,
    /// Optional human-readable note.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub note: Option<String>,
}

impl RamLfeProgramPolicy {
    /// Construct a new inactive RAM-LFE program policy.
    #[must_use]
    pub fn new(
        program_id: RamLfeProgramId,
        owner: AccountId,
        backend: RamLfeBackend,
        verification_mode: RamLfeVerificationMode,
        commitment: PolicyCommitment,
        resolver_public_key: PublicKey,
    ) -> Self {
        Self {
            program_id,
            owner,
            backend,
            verification_mode,
            commitment,
            resolver_public_key: resolver_public_key.clone(),
            output_opening_public_key: resolver_public_key,
            active: false,
            note: None,
        }
    }

    /// Override the output-opening verifier key.
    #[must_use]
    pub fn with_output_opening_public_key(mut self, public_key: PublicKey) -> Self {
        self.output_opening_public_key = public_key;
        self
    }

    /// Attach an optional operator note.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// Canonical stateless RAM-LFE execution receipt payload.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RamLfeExecutionReceiptPayload {
    /// Program policy used for the execution.
    pub program_id: RamLfeProgramId,
    /// Published digest of the hidden compiled program.
    pub program_digest: Hash,
    /// Backend used to evaluate the request.
    pub backend: RamLfeBackend,
    /// Receipt verification mode.
    pub verification_mode: RamLfeVerificationMode,
    /// Hash of the encrypted input envelope evaluated by the program.
    pub input_ciphertext_hash: Hash,
    /// Hash of the encrypted output envelope produced by the program.
    pub output_ciphertext_hash: Hash,
    /// Digest of the registered BFV parameter set used for execution.
    pub parameter_digest: Hash,
    /// Digest of the BFV evaluation-key bundle used for execution.
    pub evaluation_key_digest: Hash,
    /// Hash of the encrypted output bytes returned to the caller.
    pub output_hash: Hash,
    /// Hash of the associated-data blob bound into the execution.
    pub associated_data_hash: Hash,
    /// Execution timestamp in milliseconds since Unix epoch.
    pub executed_at_ms: u64,
    /// Optional receipt expiry timestamp in milliseconds since Unix epoch.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub expires_at_ms: Option<u64>,
}

impl RamLfeExecutionReceiptPayload {
    /// Encode the canonical signed/proved payload bytes.
    ///
    /// # Errors
    /// Returns the underlying Norito encoding error when serialization fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(self)
    }

    /// Hash the canonical payload bytes for proof-binding circuits.
    ///
    /// # Errors
    /// Returns the underlying Norito encoding error when serialization fails.
    pub fn payload_hash(&self) -> Result<Hash, norito::core::Error> {
        self.to_bytes().map(Hash::new)
    }
}

/// Canonical payload signed by an external RAM-LFE output-opening authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RamLfeOutputOpeningPayload {
    /// Program policy whose output ciphertext was opened.
    pub program_id: RamLfeProgramId,
    /// Hash of the encrypted input envelope evaluated by the program.
    pub input_ciphertext_hash: Hash,
    /// Hash of the encrypted output envelope produced by the program.
    pub output_ciphertext_hash: Hash,
    /// Digest of the registered BFV parameter set used for execution.
    pub parameter_digest: Hash,
    /// Digest of the BFV evaluation-key bundle used for execution.
    pub evaluation_key_digest: Hash,
    /// Hash of the externally opened plaintext output bytes.
    pub opened_output_hash: Hash,
    /// Opening timestamp in milliseconds since Unix epoch.
    pub opened_at_ms: u64,
    /// Optional opening expiry timestamp in milliseconds since Unix epoch.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub expires_at_ms: Option<u64>,
}

/// Externally attested opening of a RAM-LFE encrypted output.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RamLfeOutputOpening {
    /// Canonical opening payload.
    pub payload: RamLfeOutputOpeningPayload,
    /// Signature over the canonical opening payload bytes.
    pub signature: Signature,
}

impl RamLfeOutputOpening {
    /// Verify the opening authority signature.
    ///
    /// # Errors
    /// Returns an error when the signature is invalid for the opening payload.
    pub fn verify_signature(&self, public_key: &PublicKey) -> Result<(), iroha_crypto::Error> {
        SignatureOf::<RamLfeOutputOpeningPayload>::from_signature(self.signature.clone())
            .verify(public_key, &self.payload)
    }
}

/// Explicit attestation attached to a RAM-LFE receipt payload.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RamLfeReceiptAttestation {
    /// Resolver signature over the canonical payload bytes.
    Signed(Signature),
    /// Proof payload over the canonical payload bytes.
    Proof(ProofBox),
}

impl RamLfeReceiptAttestation {
    /// Return the signature when this attestation is signed.
    #[must_use]
    pub const fn signature(&self) -> Option<&Signature> {
        match self {
            Self::Signed(signature) => Some(signature),
            Self::Proof(_) => None,
        }
    }

    /// Return the proof when this attestation is proof based.
    #[must_use]
    pub const fn proof(&self) -> Option<&ProofBox> {
        match self {
            Self::Signed(_) => None,
            Self::Proof(proof) => Some(proof),
        }
    }
}

/// Self-contained generic RAM-LFE execution receipt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RamLfeExecutionReceipt {
    /// Canonical receipt payload.
    pub payload: RamLfeExecutionReceiptPayload,
    /// Explicit receipt attestation.
    pub attestation: RamLfeReceiptAttestation,
}

impl RamLfeExecutionReceipt {
    /// Encode the canonical attested payload bytes.
    ///
    /// # Errors
    /// Returns the underlying Norito encoding error when serialization fails.
    pub fn payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        self.payload.to_bytes()
    }

    /// Verify the signature over the payload.
    ///
    /// # Errors
    /// Returns an error when the signature is missing or invalid.
    pub fn verify_signature(&self, public_key: &PublicKey) -> Result<(), iroha_crypto::Error> {
        SignatureOf::<RamLfeExecutionReceiptPayload>::from_signature(
            self.attestation
                .signature()
                .cloned()
                .ok_or(iroha_crypto::Error::BadSignature)?,
        )
        .verify(public_key, &self.payload)
    }
}

/// Prelude exports for RAM-LFE program-policy consumers.
pub mod prelude {
    pub use super::{
        RamLfeExecutionReceipt, RamLfeExecutionReceiptPayload, RamLfeOutputOpening,
        RamLfeOutputOpeningPayload, RamLfeProgramId, RamLfeProgramIdParseError,
        RamLfeProgramPolicy, RamLfeReceiptAttestation,
    };
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{KeyPair, RamLfeBackend, RamLfeVerificationMode};

    use super::*;

    #[test]
    fn signed_receipt_verifies_only_signed_attestation() {
        let signer = KeyPair::random();
        let payload = RamLfeExecutionReceiptPayload {
            program_id: "email_retail".parse().expect("valid program id"),
            program_digest: Hash::new(b"program"),
            backend: RamLfeBackend::BfvProgrammedSha3_256V1,
            verification_mode: RamLfeVerificationMode::Signed,
            input_ciphertext_hash: Hash::new(b"input-ciphertext"),
            output_ciphertext_hash: Hash::new(b"output-ciphertext"),
            parameter_digest: Hash::new(b"parameters"),
            evaluation_key_digest: Hash::new(b"evaluation-keys"),
            output_hash: Hash::new(b"output"),
            associated_data_hash: Hash::new(b"associated-data"),
            executed_at_ms: 1_777_777_777_000,
            expires_at_ms: Some(1_777_777_877_000),
        };
        let signature = SignatureOf::new(signer.private_key(), &payload);
        let receipt = RamLfeExecutionReceipt {
            payload,
            attestation: RamLfeReceiptAttestation::Signed(Signature::from_bytes(
                signature.payload(),
            )),
        };

        receipt
            .verify_signature(signer.public_key())
            .expect("signed attestation should verify");
    }
}
