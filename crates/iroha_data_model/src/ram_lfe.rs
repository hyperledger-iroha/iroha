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
            resolver_public_key,
            active: false,
            note: None,
        }
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
    /// Hash of the plaintext output bytes returned to the caller.
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

/// Self-contained generic RAM-LFE execution receipt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RamLfeExecutionReceipt {
    /// Canonical receipt payload.
    pub payload: RamLfeExecutionReceiptPayload,
    /// Signature over [`payload`] when `verification_mode == signed`.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub signature: Option<Signature>,
    /// Proof payload when `verification_mode == proof`.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub proof: Option<ProofBox>,
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
            self.signature
                .clone()
                .ok_or(iroha_crypto::Error::BadSignature)?,
        )
        .verify(public_key, &self.payload)
    }
}

/// Prelude exports for RAM-LFE program-policy consumers.
pub mod prelude {
    pub use super::{
        RamLfeExecutionReceipt, RamLfeExecutionReceiptPayload, RamLfeProgramId,
        RamLfeProgramIdParseError, RamLfeProgramPolicy,
    };
}
