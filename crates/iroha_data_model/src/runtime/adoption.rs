//! Purpose-separated validator attestations of an adoption record (§13).
//!
//! This module verifies signatures and exact all-member coverage only. It does not
//! authenticate the supplied committee, certify governance, verify a State import,
//! or authorize startup. In particular, all validator signatures do not replace
//! the independently authenticated authorizing governance record. A future startup
//! consumer must join those independent authorities and the complete-State proof.
//! No consumer or unchecked startup capability is introduced here.

use std::collections::BTreeSet;

use iroha_crypto::{PrivateKey, PublicKey, Signature};
use norito::codec::{Decode, Encode};

use super::lifecycle::{AdoptionRecordV1, LifecycleSignatureError, committee_digest};

/// Signature purpose distinct from release support, the record ID, and consensus votes.
pub const ADOPTION_ATTESTATION_DOMAIN: &[u8] = b"iroha:runtime:adoption-attestation:v1\0";

/// A validator's statement about one exact adoption record, including its governance
/// reference. The reference is bound by this statement, not authenticated by it.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::adoption::AdoptionAttestationV1")]
pub struct AdoptionAttestationV1 {
    /// [`AdoptionRecordV1::id`] of the exact statement being attested.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub adoption: [u8; 32],
    /// The validator that signs the statement.
    pub validator: PublicKey,
}

/// One validator's adoption statement and its original signature.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::adoption::SignedAdoptionAttestationV1")]
pub struct SignedAdoptionAttestationV1 {
    /// Exact statement; changing its record or validator invalidates the signature.
    pub attestation: AdoptionAttestationV1,
    /// Signature over the domain-separated canonical statement.
    pub signature: Signature,
}

/// Why the supplied adoption attestations do not establish exact signature coverage.
#[derive(Debug, thiserror::Error)]
pub enum AdoptionAttestationError {
    /// The supplied committee is outside Sumeragi's `3f+1`, `1 <= f <= 10` bound.
    #[error("adoption committee must have 3f+1 members for 1 <= f <= 10")]
    CommitteeSize,
    /// A duplicate supplied committee key cannot stand for another validator.
    #[error("adoption committee contains duplicate members")]
    DuplicateCommitteeMember,
    /// The record binds another committee than the supplied source.
    #[error("adoption record names another committee")]
    CommitteeMismatch,
    /// Missing or additional statements cannot replace exact all-n coverage.
    #[error("adoption needs exactly one statement per committee member")]
    StatementCount,
    /// The same signer supplied multiple statements.
    #[error("adoption statements repeat a validator")]
    DuplicateSigner,
    /// The statement's key is not a member of the supplied committee.
    #[error("adoption statement names a foreign validator")]
    ForeignSigner,
    /// The statement names another complete adoption record.
    #[error("adoption statement names another record")]
    RecordMismatch,
    /// Canonical encoding of the statement failed; retain the original failure.
    #[error(transparent)]
    Encoding(#[from] norito::Error),
    /// Canonical identity construction failed; no identity is substituted.
    #[error(transparent)]
    Identity(#[from] LifecycleSignatureError),
    /// The original signature operation failed, including a missing local verifier.
    #[error(transparent)]
    Crypto(#[from] iroha_crypto::Error),
}

impl AdoptionAttestationV1 {
    fn signing_payload(&self) -> Result<Vec<u8>, AdoptionAttestationError> {
        let wire = norito::encode_canonical(self)?;
        let mut payload = Vec::with_capacity(ADOPTION_ATTESTATION_DOMAIN.len() + wire.len());
        payload.extend_from_slice(ADOPTION_ATTESTATION_DOMAIN);
        payload.extend_from_slice(&wire);
        Ok(payload)
    }

    /// Sign this statement without claiming that its record is authorized or true.
    ///
    /// # Errors
    /// Retains the original canonical-encoding or local signing error.
    pub fn sign(
        self,
        key: &PrivateKey,
    ) -> Result<SignedAdoptionAttestationV1, AdoptionAttestationError> {
        let signature = Signature::try_new(key, &self.signing_payload()?)?;
        Ok(SignedAdoptionAttestationV1 {
            attestation: self,
            signature,
        })
    }
}

impl SignedAdoptionAttestationV1 {
    /// Verify the signature against the statement's own declared key and exact record.
    /// This grants no committee membership, governance, State or startup authority.
    ///
    /// # Errors
    /// Rejects a different record and retains encoding and signature-operation errors.
    pub fn verify_for(&self, record: &AdoptionRecordV1) -> Result<(), AdoptionAttestationError> {
        if self.attestation.adoption != record.id()? {
            return Err(AdoptionAttestationError::RecordMismatch);
        }
        self.signature.verify(
            &self.attestation.validator,
            &self.attestation.signing_payload()?,
        )?;
        Ok(())
    }
}

/// Verify exactly one original signature from every supplied committee member.
///
/// The caller must independently authenticate both the record and committee from
/// protocol state. Success means only that the supplied keys signed these bytes;
/// it is deliberately `()` rather than a startup/governance capability. In
/// particular, a signed arbitrary authorization digest is not a governance proof.
/// Inputs already decoded by the caller remain subject to that boundary's byte and
/// allocation admission; this verifier bounds work to Sumeragi's maximum 31 members.
///
/// # Errors
/// Refuses invalid or duplicated membership, committee/record substitution, missing,
/// repeated or foreign statements, and any original signature/encoding error.
pub fn verify_all_adoption_attestations(
    record: &AdoptionRecordV1,
    committee: &[PublicKey],
    statements: &[SignedAdoptionAttestationV1],
) -> Result<(), AdoptionAttestationError> {
    if !iroha_model_base::topology::is_valid_committee_size(committee.len()) {
        return Err(AdoptionAttestationError::CommitteeSize);
    }
    let members: BTreeSet<_> = committee.iter().collect();
    if members.len() != committee.len() {
        return Err(AdoptionAttestationError::DuplicateCommitteeMember);
    }
    if committee_digest(committee)? != record.committee {
        return Err(AdoptionAttestationError::CommitteeMismatch);
    }
    if statements.len() != committee.len() {
        return Err(AdoptionAttestationError::StatementCount);
    }
    let mut observed = BTreeSet::new();
    for signed in statements {
        let member = &signed.attestation.validator;
        if !members.contains(member) {
            return Err(AdoptionAttestationError::ForeignSigner);
        }
        if !observed.insert(member) {
            return Err(AdoptionAttestationError::DuplicateSigner);
        }
        signed.verify_for(record)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
