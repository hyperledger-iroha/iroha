//! Generic hidden-program RAM-LFE policy and receipt types.
//!
//! [`RamLfeReceiptV1`] and [`RamLfeOpeningV1`] are the canonical V1 execution
//! receipt and opening. Together with `iroha_crypto::RamLfePolicyV1` they are
//! specified in `specs/ram_lfe_execution_proof.md`. The signed policy, receipt
//! and opening types beside them are superseded and stay only until the tasks
//! named at each `TODO` switch their consumers.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{NetworkId, account::AccountId, proof::ProofBox};
use iroha_crypto::{
    Algorithm, Hash, PolicyCommitment, PublicKey, RamLfeAssociatedDataHashV1, RamLfeBackend,
    RamLfeError, RamLfeExecutionContextIdV1, RamLfeInitializedMemoryCommitmentV1,
    RamLfeInputCiphertextCommitmentV1, RamLfeOpeningKeyCommitmentV1,
    RamLfeOutputCiphertextCommitmentV1, RamLfeOutputCommitmentV1, RamLfePolicyCommitmentV1,
    RamLfePolicyV1, RamLfePrfKeyCommitmentV1, RamLfeVerificationMode, Signature, SignatureOf,
};
use iroha_model_base::name::Name;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{fmt, str::FromStr, string::String, vec::Vec};
pub(crate) fn signature_for_public_key_algorithm(
    public_key: &PublicKey,
    signature: &Signature,
) -> Result<Signature, iroha_crypto::Error> {
    let algorithm = public_key
        .try_algorithm()
        .map_err(|_| iroha_crypto::Error::BadSignature)?;
    let signature = match algorithm {
        Algorithm::Ed25519 => iroha_crypto::ed25519_parse_signature(signature.payload())
            .map_err(|_| iroha_crypto::Error::BadSignature)?,
        Algorithm::MlDsa => iroha_crypto::mldsa65_parse_signature(signature.payload())
            .map_err(|_| iroha_crypto::Error::BadSignature)?,
        _ => Signature::try_from_bytes(signature.payload())
            .map_err(|_| iroha_crypto::Error::BadSignature)?,
    };
    Ok(signature)
}
/// Error returned while parsing [`RamLfeProgramId`] literals.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum RamLfeProgramIdParseError {
    /// The supplied literal is not a valid [`Name`].
    #[error("{0}")]
    InvalidName(String),
}
/// Stable on-chain identifier for a hidden RAM-LFE program policy.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeProgramId")]
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
// TODO(R.12): register `iroha_crypto::RamLfePolicyV1` in place of this record.
// R.6 removes its Signed mode and resolver signing key.
/// Public metadata for a globally registered hidden RAM-LFE program.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeProgramPolicy")]
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
// TODO(R.12): replace with `RamLfeReceiptV1`. This payload binds neither the
// full policy, the network, the beneficiary, the initialized memory nor a replay nonce.
/// Canonical stateless RAM-LFE execution receipt payload.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeExecutionReceiptPayload")]
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
        norito::encode_canonical(self)
    }
    /// Hash the canonical payload bytes for proof-binding circuits.
    ///
    /// # Errors
    /// Returns the underlying Norito encoding error when serialization fails.
    pub fn payload_hash(&self) -> Result<Hash, norito::core::Error> {
        self.to_bytes().map(Hash::new)
    }
}
// TODO(R.12): replace with `RamLfeOpeningV1` once R.8 supplies the decryption
// and PRF proof. A signature over this payload is an attestation, not a proof.
/// Canonical payload signed by an external RAM-LFE output-opening authority.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeOutputOpeningPayload")]
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
// TODO(R.12): retire with `RamLfeOutputOpeningPayload`.
/// Externally attested opening of a RAM-LFE encrypted output.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeOutputOpening")]
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
        let signature = signature_for_public_key_algorithm(public_key, &self.signature)?;
        SignatureOf::<RamLfeOutputOpeningPayload>::from_signature(signature)
            .verify(public_key, &self.payload)
    }
}
// TODO(R.6): remove the Signed branch with the obsolete Signed execution mode;
// R.12 retires the remaining wrapper with `RamLfeExecutionReceiptPayload`.
/// Explicit attestation attached to a RAM-LFE receipt payload.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeReceiptAttestation")]
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
// TODO(R.12): retire with `RamLfeExecutionReceiptPayload`.
/// Self-contained generic RAM-LFE execution receipt.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeExecutionReceipt")]
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
        let signature = self
            .attestation
            .signature()
            .ok_or(iroha_crypto::Error::BadSignature)?;
        let signature = signature_for_public_key_algorithm(public_key, signature)?;
        SignatureOf::<RamLfeExecutionReceiptPayload>::from_signature(signature)
            .verify(public_key, &self.payload)
    }
}
const RECEIPT_DOMAIN: &[u8] = b"iroha.ram_lfe.v1.receipt";
const OPENING_DOMAIN: &[u8] = b"iroha.ram_lfe.v1.opening";

/// Blake2b domains owned by the data model for the canonical V1 contract.
///
/// They extend `iroha_crypto::RAM_LFE_V1_PUBLIC_DOMAINS`; no domain of either
/// table is a prefix of another.
pub const RAM_LFE_V1_MODEL_DOMAINS: [&[u8]; 2] = [RECEIPT_DOMAIN, OPENING_DOMAIN];

fn canonical_commitment<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<Hash, RamLfeError> {
    let frame = norito::encode_canonical(value)
        .map_err(|error| RamLfeError::TranscriptEncoding(error.to_string()))?;
    Ok(Hash::new_from_chunks(&[domain, &frame]))
}

/// Consumer scope in which a receipt's replay nonce must be unused.
///
/// A receipt made for one scope is never valid in another.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeReplayDomainV1")]
pub enum RamLfeReplayDomainV1 {
    /// General encrypted execution result.
    Execution,
    /// Identifier claim made from the opened output.
    IdentifierClaim,
}

/// Replay scope and nonce bound into a canonical V1 receipt.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeReplayV1")]
pub struct RamLfeReplayV1 {
    /// Scope in which the nonce must be unused.
    pub domain: RamLfeReplayDomainV1,
    /// Request nonce, unique within the scope, network and program.
    pub nonce: [u8; 32],
}

/// Execution fields that make one execution unique: the network, the program
/// and the replay scope and nonce.
///
/// Its digest blinds the initialized-memory commitment of the execution. It
/// is built from execution fields and never from the receipt commitment,
/// because the receipt contains the commitment the digest blinds.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeExecutionContextV1")]
pub struct RamLfeExecutionContextV1 {
    /// Exact genesis-derived deployment identity.
    pub network: NetworkId,
    /// Registered program the execution runs under.
    pub program_id: RamLfeProgramId,
    /// Replay scope and nonce of the execution.
    pub replay: RamLfeReplayV1,
}

impl RamLfeExecutionContextV1 {
    /// Return the execution-context identity: the commitment to this record's
    /// canonical frame.
    ///
    /// # Errors
    /// Reports a canonical encoding failure.
    pub fn id(&self) -> Result<RamLfeExecutionContextIdV1, RamLfeError> {
        let frame = norito::encode_canonical(self)
            .map_err(|error| RamLfeError::TranscriptEncoding(error.to_string()))?;
        RamLfeExecutionContextIdV1::commit(&frame)
    }
}

/// Commitment to one complete [`RamLfeReceiptV1`].
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeReceiptCommitmentV1")]
pub struct RamLfeReceiptCommitmentV1(Hash);

impl RamLfeReceiptCommitmentV1 {
    /// Borrow the marked Iroha Blake2b digest.
    #[must_use]
    pub const fn as_hash(&self) -> &Hash {
        &self.0
    }
}

/// Commitment to one complete [`RamLfeOpeningV1`].
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeOpeningCommitmentV1")]
pub struct RamLfeOpeningCommitmentV1(Hash);

impl RamLfeOpeningCommitmentV1 {
    /// Borrow the marked Iroha Blake2b digest.
    #[must_use]
    pub const fn as_hash(&self) -> &Hash {
        &self.0
    }
}

/// Canonical V1 execution receipt: the public statement of one execution.
///
/// It binds the complete authoritative policy, the network, the program, the
/// beneficiary, the request context, the initialized memory, both ciphertexts,
/// a mandatory expiry and a replay scope and nonce. A validator derives the
/// policy commitment, the network and the program from committed State and
/// compares them with [`Self::verify_authority`]; a prover supplies none of
/// them. Only the beneficiary may request the opening of a receipt
/// ([`Self::verify_opening_requester`]).
///
/// The receipt is a statement, not evidence. It proves nothing by itself.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeReceiptV1")]
pub struct RamLfeReceiptV1 {
    /// Commitment to the complete authoritative policy.
    pub policy: RamLfePolicyCommitmentV1,
    /// Exact genesis-derived deployment identity.
    pub network: NetworkId,
    /// Registered program the policy belongs to.
    pub program_id: RamLfeProgramId,
    /// Account the result is produced for.
    pub beneficiary: AccountId,
    /// Digest of the public associated data of the request.
    pub associated_data: RamLfeAssociatedDataHashV1,
    /// State lanes initialized for this execution.
    pub initialized_memory: RamLfeInitializedMemoryCommitmentV1,
    /// Exact submitted input ciphertext.
    pub input_ciphertext: RamLfeInputCiphertextCommitmentV1,
    /// Exact evaluated output ciphertext.
    pub output_ciphertext: RamLfeOutputCiphertextCommitmentV1,
    /// Expiry in milliseconds since the Unix epoch.
    pub expires_at_ms: u64,
    /// Replay scope and nonce.
    pub replay: RamLfeReplayV1,
}

impl RamLfeReceiptV1 {
    /// Check the receipt against the authoritative policy, network and program.
    ///
    /// All three come from committed State: the program the transaction names,
    /// the policy registered for it and the network the validator runs on. The
    /// policy record carries no program identifier, so two programs registered
    /// with the same policy are told apart only by the program checked here.
    ///
    /// # Errors
    /// Rejects a receipt bound to any other policy, network or program.
    pub fn verify_authority(
        &self,
        policy: &RamLfePolicyV1,
        network: &NetworkId,
        program_id: &RamLfeProgramId,
    ) -> Result<(), RamLfeError> {
        if self.policy != policy.commitment()? {
            return Err(RamLfeError::InvalidCanonicalValue(
                "receipt is bound to another policy",
            ));
        }
        if &self.network != network {
            return Err(RamLfeError::InvalidCanonicalValue(
                "receipt is bound to another network",
            ));
        }
        if &self.program_id != program_id {
            return Err(RamLfeError::InvalidCanonicalValue(
                "receipt is bound to another program",
            ));
        }
        Ok(())
    }

    /// Check that an account may request the opening of this receipt.
    ///
    /// `authority` is the authenticated authority of the transaction that
    /// requests the opening. An account named by the prover or by the opener
    /// is not sufficient. Only the beneficiary may request the opening, and
    /// the opened result is delivered to that account only.
    ///
    /// # Errors
    /// Returns [`RamLfeError::UnauthorizedOpeningRequester`] for any account
    /// other than the beneficiary.
    pub fn verify_opening_requester(&self, authority: &AccountId) -> Result<(), RamLfeError> {
        if &self.beneficiary != authority {
            return Err(RamLfeError::UnauthorizedOpeningRequester);
        }
        Ok(())
    }

    /// Return the fields that make this execution unique.
    ///
    /// The result does not depend on the initialized-memory commitment, so the
    /// evaluator can compute it before it commits the memory.
    #[must_use]
    pub fn execution_context(&self) -> RamLfeExecutionContextV1 {
        RamLfeExecutionContextV1 {
            network: self.network,
            program_id: self.program_id.clone(),
            replay: self.replay,
        }
    }

    /// Commit the complete receipt.
    ///
    /// # Errors
    /// Reports a canonical encoding failure.
    pub fn commitment(&self) -> Result<RamLfeReceiptCommitmentV1, RamLfeError> {
        canonical_commitment(RECEIPT_DOMAIN, self).map(RamLfeReceiptCommitmentV1)
    }
}

/// Canonical V1 opening: the public statement of one authorized opening.
///
/// It binds the receipt, the ordered output and the opening and PRF keys the
/// policy registered. The statement is not evidence: no opener, resolver or
/// committee signature over it stands in for the decryption and PRF proofs.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ram_lfe::RamLfeOpeningV1")]
pub struct RamLfeOpeningV1 {
    /// Receipt whose output ciphertext is opened.
    pub receipt: RamLfeReceiptCommitmentV1,
    /// Hiding commitment to the ordered opened output.
    pub output: RamLfeOutputCommitmentV1,
    /// Opening key the policy registered.
    pub opening_key: RamLfeOpeningKeyCommitmentV1,
    /// PRF key the policy registered.
    pub prf_key: RamLfePrfKeyCommitmentV1,
}

impl RamLfeOpeningV1 {
    /// Build the opening statement for a receipt under its authoritative policy.
    ///
    /// The opening and PRF keys are taken from the policy, never from the opener.
    ///
    /// # Errors
    /// Rejects a receipt bound to another policy and reports an encoding failure.
    pub fn new(
        policy: &RamLfePolicyV1,
        receipt: &RamLfeReceiptV1,
        output: RamLfeOutputCommitmentV1,
    ) -> Result<Self, RamLfeError> {
        if receipt.policy != policy.commitment()? {
            return Err(RamLfeError::InvalidCanonicalValue(
                "receipt is bound to another policy",
            ));
        }
        Ok(Self {
            receipt: receipt.commitment()?,
            output,
            opening_key: policy.opening_key,
            prf_key: policy.prf_key,
        })
    }

    /// Check the opening against the authoritative policy and receipt.
    ///
    /// # Errors
    /// Rejects an opening of another receipt or under any other opening or PRF key.
    pub fn verify_authority(
        &self,
        policy: &RamLfePolicyV1,
        receipt: &RamLfeReceiptV1,
    ) -> Result<(), RamLfeError> {
        if *self != Self::new(policy, receipt, self.output)? {
            return Err(RamLfeError::InvalidCanonicalValue(
                "opening is bound to another receipt or key",
            ));
        }
        Ok(())
    }

    /// Commit the complete opening.
    ///
    /// # Errors
    /// Reports a canonical encoding failure.
    pub fn commitment(&self) -> Result<RamLfeOpeningCommitmentV1, RamLfeError> {
        canonical_commitment(OPENING_DOMAIN, self).map(RamLfeOpeningCommitmentV1)
    }
}
/// Prelude exports for RAM-LFE program-policy consumers.
pub mod prelude {
    pub use super::{
        RamLfeExecutionContextV1, RamLfeExecutionReceipt, RamLfeExecutionReceiptPayload,
        RamLfeOpeningCommitmentV1, RamLfeOpeningV1, RamLfeOutputOpening,
        RamLfeOutputOpeningPayload, RamLfeProgramId, RamLfeProgramIdParseError,
        RamLfeProgramPolicy, RamLfeReceiptAttestation, RamLfeReceiptCommitmentV1, RamLfeReceiptV1,
        RamLfeReplayDomainV1, RamLfeReplayV1,
    };
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, RamLfeBackend, RamLfeVerificationMode};
    const NONCANONICAL_ED25519_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    const SMALL_ORDER_ED25519_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked RAM-LFE fixture keypair")
    }
    fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
            panic!("{algorithm:?} RAM-LFE fixture key generation should succeed: {err}")
        })
    }
    fn checked_ed25519_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked RAM-LFE Ed25519 fixture keypair")
    }
    fn with_malformed_ed25519_r(signature: &Signature, replacement_r: &[u8; 32]) -> Signature {
        let mut bytes = signature.payload().to_vec();
        bytes[..replacement_r.len()].copy_from_slice(replacement_r);
        Signature::from_bytes(&bytes)
    }
    fn receipt_payload() -> RamLfeExecutionReceiptPayload {
        RamLfeExecutionReceiptPayload {
            program_id: "email_retail".parse().expect("valid program id"),
            program_digest: Hash::new(b"program"),
            backend: RamLfeBackend::BfvProgrammedV1,
            verification_mode: RamLfeVerificationMode::Signed,
            input_ciphertext_hash: Hash::new(b"input-ciphertext"),
            output_ciphertext_hash: Hash::new(b"output-ciphertext"),
            parameter_digest: Hash::new(b"parameters"),
            evaluation_key_digest: Hash::new(b"evaluation-keys"),
            output_hash: Hash::new(b"output"),
            associated_data_hash: Hash::new(b"associated-data"),
            executed_at_ms: 1_777_777_777_000,
            expires_at_ms: Some(1_777_777_877_000),
        }
    }
    fn signed_receipt(
        signer: &KeyPair,
        payload: RamLfeExecutionReceiptPayload,
    ) -> RamLfeExecutionReceipt {
        let signature = SignatureOf::try_new(signer.private_key(), &payload)
            .expect("sign checked RAM-LFE receipt fixture");
        signature
            .verify(signer.public_key(), &payload)
            .expect("checked RAM-LFE receipt fixture verifies");
        RamLfeExecutionReceipt {
            attestation: RamLfeReceiptAttestation::Signed(signature.into()),
            payload,
        }
    }
    fn opening_payload() -> RamLfeOutputOpeningPayload {
        RamLfeOutputOpeningPayload {
            program_id: "email_retail".parse().expect("valid program id"),
            input_ciphertext_hash: Hash::new(b"input-ciphertext"),
            output_ciphertext_hash: Hash::new(b"output-ciphertext"),
            parameter_digest: Hash::new(b"parameters"),
            evaluation_key_digest: Hash::new(b"evaluation-keys"),
            opened_output_hash: Hash::new(b"opened-output"),
            opened_at_ms: 1_777_777_777_001,
            expires_at_ms: Some(1_777_777_877_000),
        }
    }
    fn signed_opening(
        signer: &KeyPair,
        payload: RamLfeOutputOpeningPayload,
    ) -> RamLfeOutputOpening {
        let signature = SignatureOf::try_new(signer.private_key(), &payload)
            .expect("sign checked RAM-LFE output-opening fixture");
        signature
            .verify(signer.public_key(), &payload)
            .expect("checked RAM-LFE output-opening fixture verifies");
        RamLfeOutputOpening {
            signature: signature.into(),
            payload,
        }
    }
    #[test]
    fn signed_receipt_verifies_only_signed_attestation() {
        let signer = checked_random_keypair();
        let receipt = signed_receipt(&signer, receipt_payload());
        receipt
            .verify_signature(signer.public_key())
            .expect("signed attestation should verify");
    }
    #[test]
    fn receipt_payload_identity_ignores_ambient_norito_layout() {
        let payload = receipt_payload();
        let canonical = payload
            .to_bytes()
            .expect("encode canonical receipt payload");
        let canonical_hash = payload
            .payload_hash()
            .expect("hash canonical receipt payload");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_ne!(
            norito::to_bytes(&payload).expect("encode alternate-layout receipt payload"),
            canonical,
            "fixture must exercise a distinct ambient Norito layout"
        );
        assert_eq!(
            payload
                .to_bytes()
                .expect("encode receipt payload canonically under alternate layout"),
            canonical
        );
        assert_eq!(
            payload
                .payload_hash()
                .expect("hash receipt payload under alternate layout"),
            canonical_hash
        );
    }
    #[test]
    fn signed_receipt_rejects_wrong_key_and_proof_attestation() {
        let signer = checked_random_keypair();
        let wrong_signer = checked_random_keypair();
        let payload = receipt_payload();
        let receipt = signed_receipt(&signer, payload.clone());
        receipt
            .verify_signature(wrong_signer.public_key())
            .expect_err("receipt signatures must reject unrelated verifier keys");
        let proof_receipt = RamLfeExecutionReceipt {
            payload,
            attestation: RamLfeReceiptAttestation::Proof(crate::proof::ProofBox::new(
                "halo2/ipa".into(),
                vec![1, 2, 3],
            )),
        };
        proof_receipt
            .verify_signature(signer.public_key())
            .expect_err("signature verification must reject proof attestations");
    }
    #[test]
    fn signed_receipt_rejects_all_zero_signature_material() {
        let signer = checked_random_keypair();
        let mut receipt = signed_receipt(&signer, receipt_payload());
        receipt.attestation = RamLfeReceiptAttestation::Signed(Signature::from_bytes(&[0u8; 64]));
        assert_eq!(
            receipt
                .verify_signature(signer.public_key())
                .expect_err("all-zero RAM-LFE receipt signature must fail admission"),
            iroha_crypto::Error::BadSignature
        );
    }
    #[test]
    fn signed_receipt_rejects_short_signature_material() {
        let signer = checked_ed25519_keypair(0x33);
        let mut receipt = signed_receipt(&signer, receipt_payload());
        receipt.attestation = RamLfeReceiptAttestation::Signed(Signature::from_bytes(&[0x44; 63]));
        assert_eq!(
            receipt
                .verify_signature(signer.public_key())
                .expect_err("short RAM-LFE receipt signature must fail admission"),
            iroha_crypto::Error::BadSignature
        );
    }
    #[test]
    fn signed_receipt_rejects_malformed_ed25519_signature_r() {
        let signer = checked_ed25519_keypair(0x31);
        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_ED25519_R),
            ("noncanonical", NONCANONICAL_ED25519_R),
        ] {
            let mut receipt = signed_receipt(&signer, receipt_payload());
            let signature = receipt
                .attestation
                .signature()
                .expect("signed fixture carries a signature");
            receipt.attestation = RamLfeReceiptAttestation::Signed(with_malformed_ed25519_r(
                signature,
                &replacement_r,
            ));
            assert_eq!(
                receipt.verify_signature(signer.public_key()).unwrap_err(),
                iroha_crypto::Error::BadSignature,
                "{label} RAM-LFE receipt Ed25519 R must fail admission"
            );
        }
    }
    #[test]
    fn signed_receipt_rejects_malformed_mldsa_signature_lengths() {
        let signer = checked_random_keypair_with_algorithm(Algorithm::MlDsa);
        let receipt = signed_receipt(&signer, receipt_payload());
        receipt
            .verify_signature(signer.public_key())
            .expect("valid ML-DSA RAM-LFE receipt signature verifies");
        let valid_signature = receipt
            .attestation
            .signature()
            .expect("signed fixture carries a signature")
            .payload()
            .to_vec();
        for (label, replacement_signature) in [
            (
                "short",
                valid_signature[..valid_signature.len() - 1].to_vec(),
            ),
            ("overlong", {
                let mut payload = valid_signature.clone();
                payload.push(0x61);
                payload
            }),
        ] {
            let mut invalid_receipt = receipt.clone();
            invalid_receipt.attestation =
                RamLfeReceiptAttestation::Signed(Signature::from_bytes(&replacement_signature));
            assert_eq!(
                invalid_receipt
                    .verify_signature(signer.public_key())
                    .unwrap_err(),
                iroha_crypto::Error::BadSignature,
                "{label} RAM-LFE receipt ML-DSA signature length was not rejected"
            );
        }
    }
    #[test]
    fn signed_receipt_rejects_tampered_ciphertext_binding() {
        let signer = checked_random_keypair();
        let mut receipt = signed_receipt(&signer, receipt_payload());
        receipt.payload.output_ciphertext_hash = Hash::new(b"tampered-output-ciphertext");
        receipt
            .verify_signature(signer.public_key())
            .expect_err("mutating ciphertext-bound receipt fields must invalidate signature");
    }
    #[test]
    fn signed_receipt_rejects_mutation_of_security_bindings() {
        macro_rules! assert_rejected {
            ($label:literal, |$payload:ident| $body:block) => {{
                let signer = checked_random_keypair();
                let mut receipt = signed_receipt(&signer, receipt_payload());
                let $payload = &mut receipt.payload;
                $body
                assert!(
                    receipt.verify_signature(signer.public_key()).is_err(),
                    "mutating {} must invalidate receipt signature",
                    $label
                );
            }};
        }
        assert_rejected!("program_id", |payload| {
            payload.program_id = "email_other".parse().expect("valid program id");
        });
        assert_rejected!("program_digest", |payload| {
            payload.program_digest = Hash::new(b"tampered-program");
        });
        assert_rejected!("backend", |payload| {
            payload.backend = RamLfeBackend::BfvAffineV1;
        });
        assert_rejected!("verification_mode", |payload| {
            payload.verification_mode = RamLfeVerificationMode::Proof;
        });
        assert_rejected!("input_ciphertext_hash", |payload| {
            payload.input_ciphertext_hash = Hash::new(b"tampered-input");
        });
        assert_rejected!("output_ciphertext_hash", |payload| {
            payload.output_ciphertext_hash = Hash::new(b"tampered-output");
        });
        assert_rejected!("parameter_digest", |payload| {
            payload.parameter_digest = Hash::new(b"tampered-parameters");
        });
        assert_rejected!("evaluation_key_digest", |payload| {
            payload.evaluation_key_digest = Hash::new(b"tampered-eval-keys");
        });
        assert_rejected!("output_hash", |payload| {
            payload.output_hash = Hash::new(b"tampered-output-hash");
        });
        assert_rejected!("associated_data_hash", |payload| {
            payload.associated_data_hash = Hash::new(b"tampered-associated-data");
        });
        assert_rejected!("executed_at_ms", |payload| {
            payload.executed_at_ms += 1;
        });
        assert_rejected!("expires_at_ms", |payload| {
            payload.expires_at_ms = None;
        });
    }
    #[test]
    fn output_opening_rejects_wrong_key_and_tampered_payload() {
        let signer = checked_random_keypair();
        let wrong_signer = checked_random_keypair();
        let mut opening = signed_opening(&signer, opening_payload());
        opening
            .verify_signature(wrong_signer.public_key())
            .expect_err("output openings must reject unrelated verifier keys");
        opening.payload.opened_output_hash = Hash::new(b"tampered-opened-output");
        opening
            .verify_signature(signer.public_key())
            .expect_err("mutating opened output binding must invalidate opening signature");
    }
    #[test]
    fn output_opening_rejects_all_zero_signature_material() {
        let signer = checked_random_keypair();
        let mut opening = signed_opening(&signer, opening_payload());
        opening.signature = Signature::from_bytes(&[0u8; 64]);
        assert_eq!(
            opening
                .verify_signature(signer.public_key())
                .expect_err("all-zero RAM-LFE opening signature must fail admission"),
            iroha_crypto::Error::BadSignature
        );
    }
    #[test]
    fn output_opening_rejects_short_signature_material() {
        let signer = checked_ed25519_keypair(0x34);
        let mut opening = signed_opening(&signer, opening_payload());
        opening.signature = Signature::from_bytes(&[0x55; 63]);
        assert_eq!(
            opening
                .verify_signature(signer.public_key())
                .expect_err("short RAM-LFE opening signature must fail admission"),
            iroha_crypto::Error::BadSignature
        );
    }
    #[test]
    fn output_opening_rejects_malformed_ed25519_signature_r() {
        let signer = checked_ed25519_keypair(0x32);
        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_ED25519_R),
            ("noncanonical", NONCANONICAL_ED25519_R),
        ] {
            let mut opening = signed_opening(&signer, opening_payload());
            opening.signature = with_malformed_ed25519_r(&opening.signature, &replacement_r);
            assert_eq!(
                opening.verify_signature(signer.public_key()).unwrap_err(),
                iroha_crypto::Error::BadSignature,
                "{label} RAM-LFE opening Ed25519 R must fail admission"
            );
        }
    }
    #[test]
    fn output_opening_rejects_malformed_mldsa_signature_lengths() {
        let signer = checked_random_keypair_with_algorithm(Algorithm::MlDsa);
        let opening = signed_opening(&signer, opening_payload());
        opening
            .verify_signature(signer.public_key())
            .expect("valid ML-DSA RAM-LFE opening signature verifies");
        let valid_signature = opening.signature.payload().to_vec();
        for (label, replacement_signature) in [
            (
                "short",
                valid_signature[..valid_signature.len() - 1].to_vec(),
            ),
            ("overlong", {
                let mut payload = valid_signature.clone();
                payload.push(0x62);
                payload
            }),
        ] {
            let mut invalid_opening = opening.clone();
            invalid_opening.signature = Signature::from_bytes(&replacement_signature);
            assert_eq!(
                invalid_opening
                    .verify_signature(signer.public_key())
                    .unwrap_err(),
                iroha_crypto::Error::BadSignature,
                "{label} RAM-LFE opening ML-DSA signature length was not rejected"
            );
        }
    }
    #[test]
    fn output_opening_rejects_mutation_of_security_bindings() {
        macro_rules! assert_rejected {
            ($label:literal, |$payload:ident| $body:block) => {{
                let signer = checked_random_keypair();
                let mut opening = signed_opening(&signer, opening_payload());
                let $payload = &mut opening.payload;
                $body
                assert!(
                    opening.verify_signature(signer.public_key()).is_err(),
                    "mutating {} must invalidate opening signature",
                    $label
                );
            }};
        }
        assert_rejected!("program_id", |payload| {
            payload.program_id = "email_other".parse().expect("valid program id");
        });
        assert_rejected!("input_ciphertext_hash", |payload| {
            payload.input_ciphertext_hash = Hash::new(b"tampered-input");
        });
        assert_rejected!("output_ciphertext_hash", |payload| {
            payload.output_ciphertext_hash = Hash::new(b"tampered-output");
        });
        assert_rejected!("parameter_digest", |payload| {
            payload.parameter_digest = Hash::new(b"tampered-parameters");
        });
        assert_rejected!("evaluation_key_digest", |payload| {
            payload.evaluation_key_digest = Hash::new(b"tampered-eval-keys");
        });
        assert_rejected!("opened_output_hash", |payload| {
            payload.opened_output_hash = Hash::new(b"tampered-opened-output");
        });
        assert_rejected!("opened_at_ms", |payload| {
            payload.opened_at_ms += 1;
        });
        assert_rejected!("expires_at_ms", |payload| {
            payload.expires_at_ms = None;
        });
    }
}

#[cfg(test)]
mod captured_ram_lfe_schema_tests;
